"""Core Redis benchmark runner — single case at a time, no subprocess."""

from __future__ import annotations

import random
import statistics
import string
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from typing import Any

import redis


SUPPORTED_OPS = [
    "set", "get", "mixed", "pipeline", "incr", "decr", "mget", "mset",
    "append", "strlen", "del", "exists",
    "hset", "hget", "hdel", "hlen", "hexists", "hkeys", "hvals", "hgetall",
    "lpush", "lpop", "rpush", "rpop", "llen", "lindex", "lrange",
    "sadd", "sismember", "srem", "smembers", "scard", "spop",
    "zadd", "zscore", "zrem", "zcard", "zcount", "zrange",
    "ping", "all-mixed",
]

OPS_REQUIRING_PREFILL = {
    "get", "mixed", "pipeline", "mget", "append", "del", "exists",
    "hget", "hdel", "hlen", "hexists", "hkeys", "hvals", "hgetall",
    "lpop", "rpop", "llen", "lindex", "lrange",
    "sismember", "srem", "smembers", "scard", "spop",
    "zscore", "zrem", "zcard", "zcount", "zrange",
    "all-mixed",
}

STRING_OPS = {"get", "mixed", "pipeline", "mget", "append", "del", "exists"}
HASH_OPS = {"hget", "hdel", "hlen", "hexists"}
MULTI_FIELD_HASH_OPS = {"hkeys", "hvals", "hgetall"}
LIST_OPS = {"lpop", "rpop", "llen", "lindex", "lrange"}
SET_OPS = {"sismember", "srem", "smembers", "scard", "spop"}
ZSET_OPS = {"zscore", "zrem", "zcard", "zcount", "zrange"}


@dataclass
class _BenchResult:
    latencies_ms: list[float]
    errors: int
    requests_done: int


def random_payload(size: int, rnd: random.Random) -> bytes:
    alphabet = string.ascii_letters + string.digits
    return "".join(rnd.choice(alphabet) for _ in range(size)).encode("utf-8")


def split_work(total: int, workers: int) -> list[int]:
    base = total // workers
    extra = total % workers
    return [base + (1 if i < extra else 0) for i in range(workers)]


# ── Pre-fill helpers ────────────────────────────────────────────────────


def _prefill_keys(client: redis.Redis, key_prefix: str, keyspace: int, payload: bytes) -> None:
    pipe = client.pipeline(transaction=False)
    for i in range(keyspace):
        pipe.set(f"{key_prefix}:{i}", payload)
        if i % 500 == 0:
            pipe.execute()
    pipe.execute()


def _prefill_hashes(client: redis.Redis, key_prefix: str, keyspace: int, payload: bytes) -> None:
    pipe = client.pipeline(transaction=False)
    for i in range(keyspace):
        pipe.hset(f"{key_prefix}:{i}", mapping={"field": payload})
        if i % 500 == 0:
            pipe.execute()
    pipe.execute()


def _prefill_multi_field_hashes(client: redis.Redis, key_prefix: str, keyspace: int, payload: bytes) -> None:
    pipe = client.pipeline(transaction=False)
    for i in range(keyspace):
        mapping = {f"f{j}": payload for j in range(10)}
        pipe.hset(f"{key_prefix}:{i}", mapping=mapping)
        if i % 200 == 0:
            pipe.execute()
    pipe.execute()


def _prefill_lists(client: redis.Redis, key_prefix: str, keyspace: int, payload: bytes) -> None:
    pipe = client.pipeline(transaction=False)
    for i in range(keyspace):
        key = f"{key_prefix}:{i}"
        pipe.delete(key)
        pipe.rpush(key, payload, payload, payload, payload)
        if i % 250 == 0:
            pipe.execute()
    pipe.execute()


def _prefill_sets(client: redis.Redis, key_prefix: str, keyspace: int) -> None:
    pipe = client.pipeline(transaction=False)
    for i in range(keyspace):
        pipe.sadd(f"{key_prefix}:{i}", f"member:{i}")
        if i % 500 == 0:
            pipe.execute()
    pipe.execute()


def _prefill_zsets(client: redis.Redis, key_prefix: str, keyspace: int) -> None:
    pipe = client.pipeline(transaction=False)
    for i in range(keyspace):
        pipe.zadd(f"{key_prefix}:{i}", {f"member:{i}": float(i)})
        if i % 500 == 0:
            pipe.execute()
    pipe.execute()


def _prefill(client: redis.Redis, op: str, key_prefix: str, keyspace: int, payload: bytes) -> None:
    if op not in OPS_REQUIRING_PREFILL:
        return
    if op in STRING_OPS or op == "all-mixed":
        _prefill_keys(client, key_prefix, keyspace, payload)
    if op in HASH_OPS or op == "all-mixed":
        _prefill_hashes(client, key_prefix, keyspace, payload)
    if op in MULTI_FIELD_HASH_OPS or op == "all-mixed":
        _prefill_multi_field_hashes(client, key_prefix, keyspace, payload)
    if op in LIST_OPS or op == "all-mixed":
        _prefill_lists(client, key_prefix, keyspace, payload)
    if op in SET_OPS or op == "all-mixed":
        _prefill_sets(client, key_prefix, keyspace)
    if op in ZSET_OPS or op == "all-mixed":
        _prefill_zsets(client, key_prefix, keyspace)


# ── Worker ──────────────────────────────────────────────────────────────


def _gaussian_key(rnd: random.Random, keyspace: int) -> int:
    mu = keyspace / 2.0
    sigma = keyspace / 6.0
    while True:
        idx = int(rnd.gauss(mu, sigma))
        if 0 <= idx < keyspace:
            return idx


def _pick_key_id(rnd: random.Random, keyspace: int, distribution: str) -> int:
    if distribution == "gaussian":
        return _gaussian_key(rnd, keyspace)
    return rnd.randrange(keyspace)


def _run_worker(
    pool: redis.ConnectionPool,
    op: str,
    key_prefix: str,
    keyspace: int,
    payload: bytes,
    requests: int,
    worker_seed: int,
    pipeline_size: int,
    key_distribution: str = "uniform",
    stop_event: threading.Event | None = None,
) -> _BenchResult:
    client = redis.Redis(connection_pool=pool)
    rnd = random.Random(worker_seed)
    latencies_ms: list[float] = []
    errors = 0
    done = 0

    for _ in range(requests):
        if stop_event and stop_event.is_set():
            break
        key_id = _pick_key_id(rnd, keyspace, key_distribution)
        key = f"{key_prefix}:{key_id}"
        started = time.perf_counter_ns()
        try:
            if op == "set":
                client.set(key, payload)
            elif op == "get":
                client.get(key)
            elif op == "mixed":
                if rnd.random() < 0.5:
                    client.set(key, payload)
                else:
                    client.get(key)
            elif op == "pipeline":
                pipe = client.pipeline(transaction=False)
                for _ in range(pipeline_size):
                    k = f"{key_prefix}:{rnd.randrange(keyspace)}"
                    pipe.set(k, payload)
                    pipe.get(k)
                pipe.execute()
            elif op == "incr":
                client.incr(key)
            elif op == "decr":
                client.decr(key)
            elif op == "mget":
                keys = [f"{key_prefix}:{rnd.randrange(keyspace)}" for _ in range(pipeline_size)]
                client.mget(keys)
            elif op == "mset":
                pairs = {}
                for _ in range(pipeline_size):
                    k = f"{key_prefix}:{rnd.randrange(keyspace)}"
                    pairs[k] = payload
                client.mset(pairs)
            elif op == "append":
                client.append(key, payload)
            elif op == "strlen":
                client.strlen(key)
            elif op == "del":
                client.delete(key)
            elif op == "exists":
                client.exists(key)
            elif op == "hset":
                client.hset(key, f"field:{rnd.randrange(16)}", payload)
            elif op == "hget":
                client.hget(key, "field")
            elif op == "hdel":
                client.hdel(key, f"field:{rnd.randrange(10)}")
            elif op == "hlen":
                client.hlen(key)
            elif op == "hexists":
                client.hexists(key, "field")
            elif op == "hkeys":
                client.hkeys(key)
            elif op == "hvals":
                client.hvals(key)
            elif op == "hgetall":
                client.hgetall(key)
            elif op == "lpush":
                client.lpush(key, payload)
            elif op == "lpop":
                client.lpop(key)
            elif op == "rpush":
                client.rpush(key, payload)
            elif op == "rpop":
                client.rpop(key)
            elif op == "llen":
                client.llen(key)
            elif op == "lindex":
                client.lindex(key, 0)
            elif op == "lrange":
                client.lrange(key, 0, pipeline_size - 1)
            elif op == "sadd":
                client.sadd(key, f"member:{rnd.randrange(max(1, keyspace * 2))}")
            elif op == "sismember":
                client.sismember(key, f"member:{key_id}")
            elif op == "srem":
                client.srem(key, f"member:{rnd.randrange(max(1, keyspace * 2))}")
            elif op == "smembers":
                client.smembers(key)
            elif op == "scard":
                client.scard(key)
            elif op == "spop":
                client.spop(key, 1)
            elif op == "zadd":
                member = f"member:{rnd.randrange(max(1, keyspace * 2))}"
                client.zadd(key, {member: rnd.random() * 10_000})
            elif op == "zscore":
                client.zscore(key, f"member:{key_id}")
            elif op == "zrem":
                client.zrem(key, f"member:{rnd.randrange(max(1, keyspace * 2))}")
            elif op == "zcard":
                client.zcard(key)
            elif op == "zcount":
                client.zcount(key, "-inf", "+inf")
            elif op == "zrange":
                client.zrange(key, 0, pipeline_size - 1)
            elif op == "ping":
                client.ping()
            elif op == "all-mixed":
                mod = rnd.randrange(12)
                if mod == 0:
                    client.set(key, payload)
                elif mod == 1:
                    client.get(key)
                elif mod == 2:
                    client.incr(key)
                elif mod == 3:
                    client.hset(key, "field", payload)
                elif mod == 4:
                    client.hget(key, "field")
                elif mod == 5:
                    client.lpush(key, payload)
                elif mod == 6:
                    client.lpop(key)
                elif mod == 7:
                    client.sadd(key, f"member:{rnd.randrange(max(1, keyspace * 2))}")
                elif mod == 8:
                    client.sismember(key, f"member:{key_id}")
                elif mod == 9:
                    zmember = f"member:{rnd.randrange(max(1, keyspace * 2))}"
                    client.zadd(key, {zmember: rnd.random() * 10_000})
                elif mod == 10:
                    client.ping()
                else:
                    client.exists(key)
            else:
                raise ValueError(f"unsupported op: {op}")
            done += 1
        except redis.RedisError:
            errors += 1
        finally:
            elapsed_ms = (time.perf_counter_ns() - started) / 1_000_000
            latencies_ms.append(elapsed_ms)

    return _BenchResult(latencies_ms=latencies_ms, errors=errors, requests_done=done)


def _pct(latencies: list[float], p: float) -> float:
    if not latencies:
        return 0.0
    k = max(0, min(len(latencies) - 1, int(round((p / 100) * (len(latencies) - 1)))))
    return latencies[k]


# ── Public API ──────────────────────────────────────────────────────────


def run_case(
    url: str,
    op: str,
    requests: int,
    workers: int,
    keyspace: int,
    value_size: int,
    *,
    pipeline_size: int = 1,
    warmup: int = 1000,
    seed: int = 42,
    key_prefix: str = "perf",
    key_distribution: str = "uniform",
    label: str = "",
    cleanup: bool = False,
) -> dict[str, Any]:
    """Run a single benchmark case. Returns a result dict compatible with compare.py."""

    payload = random_payload(value_size, random.Random(seed))
    pool = redis.ConnectionPool.from_url(url, max_connections=max(100, workers * 2))
    client = redis.Redis(connection_pool=pool)

    # Pre-fill if needed
    _prefill(client, op, key_prefix, keyspace, payload)

    # Warmup
    if warmup > 0:
        _run_worker(pool, op, key_prefix, keyspace, payload, warmup, seed + 100_000, pipeline_size, key_distribution)

    # Actual benchmark
    pieces = split_work(requests, workers)
    started = time.perf_counter()
    lock = threading.Lock()
    merged_latencies: list[float] = []
    total_errors = 0
    total_done = 0

    def consume(result: _BenchResult) -> None:
        nonlocal total_errors, total_done
        with lock:
            merged_latencies.extend(result.latencies_ms)
            total_errors += result.errors
            total_done += result.requests_done

    with ThreadPoolExecutor(max_workers=workers) as executor:
        futures = []
        for idx, chunk in enumerate(pieces):
            if chunk == 0:
                continue
            futures.append(
                executor.submit(
                    _run_worker, pool, op, key_prefix, keyspace, payload,
                    chunk, seed + idx + 1, pipeline_size, key_distribution,
                )
            )
        for f in futures:
            consume(f.result())
    duration = time.perf_counter() - started

    merged_latencies.sort()
    throughput = total_done / duration if duration > 0 else 0.0
    avg = statistics.fmean(merged_latencies) if merged_latencies else 0.0
    stdev = statistics.stdev(merged_latencies) if len(merged_latencies) > 1 else 0.0

    if cleanup:
        _cleanup_keys(client, key_prefix)

    return {
        "label": label,
        "url": url,
        "op": op,
        "requests": requests,
        "requests_done": total_done,
        "workers": workers,
        "keyspace": keyspace,
        "value_size": value_size,
        "pipeline_size": pipeline_size,
        "warmup": warmup,
        "seed": seed,
        "key_distribution": key_distribution,
        "errors": total_errors,
        "duration_s": round(duration, 6),
        "throughput_rps": round(throughput, 6),
        "latency_min_ms": round(merged_latencies[0], 6) if merged_latencies else 0.0,
        "latency_avg_ms": round(avg, 6),
        "latency_median_ms": round(_pct(merged_latencies, 50), 6),
        "latency_p50_ms": round(_pct(merged_latencies, 50), 6),
        "latency_p95_ms": round(_pct(merged_latencies, 95), 6),
        "latency_p99_ms": round(_pct(merged_latencies, 99), 6),
        "latency_p99_9_ms": round(_pct(merged_latencies, 99.9), 6),
        "latency_p99_99_ms": round(_pct(merged_latencies, 99.99), 6),
        "latency_max_ms": round(merged_latencies[-1], 6) if merged_latencies else 0.0,
        "latency_stdev_ms": round(stdev, 6),
    }


def _cleanup_keys(client: redis.Redis, key_prefix: str) -> None:
    cursor = 0
    pattern = f"{key_prefix}:*"
    while True:
        cursor, keys = client.scan(cursor=cursor, match=pattern, count=1000)
        if keys:
            client.delete(*keys)
        if cursor == 0:
            break
