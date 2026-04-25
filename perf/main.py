from __future__ import annotations

import argparse
import json
import random
import statistics
import string
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass

import redis


@dataclass
class BenchResult:
    latencies_ms: list[float]
    errors: int
    requests_done: int


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Simple Redis performance benchmark")
    parser.add_argument("--url", default="redis://127.0.0.1:6379/0", help="Redis URL")
    parser.add_argument("--op", choices=["set", "get", "mixed", "pipeline"], default="mixed")
    parser.add_argument("--requests", type=int, default=100_000, help="Total logical requests")
    parser.add_argument("--workers", type=int, default=50, help="Concurrent workers")
    parser.add_argument("--key-prefix", default="perf", help="Benchmark key prefix")
    parser.add_argument("--keyspace", type=int, default=10_000, help="Number of keys to target")
    parser.add_argument("--value-size", type=int, default=128, help="Value size in bytes")
    parser.add_argument("--pipeline-size", type=int, default=10, help="Batch size for pipeline op")
    parser.add_argument("--warmup", type=int, default=1_000, help="Warmup requests")
    parser.add_argument("--seed", type=int, default=42, help="Random seed")
    parser.add_argument("--cleanup", action="store_true", help="Delete test keys after benchmark")
    parser.add_argument("--label", default="", help="Optional label included in output")
    parser.add_argument(
        "--output-format",
        choices=["text", "json"],
        default="text",
        help="Result output format",
    )
    parser.add_argument(
        "--output-file",
        default="",
        help="Optional file to write results to (stdout is always used)",
    )
    return parser


def random_payload(size: int, rnd: random.Random) -> bytes:
    alphabet = string.ascii_letters + string.digits
    return "".join(rnd.choice(alphabet) for _ in range(size)).encode("utf-8")


def split_work(total: int, workers: int) -> list[int]:
    base = total // workers
    extra = total % workers
    return [base + (1 if i < extra else 0) for i in range(workers)]


def prefill_keys(client: redis.Redis, key_prefix: str, keyspace: int, payload: bytes) -> None:
    pipe = client.pipeline(transaction=False)
    for i in range(keyspace):
        pipe.set(f"{key_prefix}:{i}", payload)
        if i % 500 == 0:
            pipe.execute()
    pipe.execute()


def cleanup_keys(client: redis.Redis, key_prefix: str) -> None:
    cursor = 0
    pattern = f"{key_prefix}:*"
    while True:
        cursor, keys = client.scan(cursor=cursor, match=pattern, count=1000)
        if keys:
            client.delete(*keys)
        if cursor == 0:
            break


def run_worker(
    pool: redis.ConnectionPool,
    op: str,
    key_prefix: str,
    keyspace: int,
    payload: bytes,
    requests: int,
    worker_seed: int,
    pipeline_size: int,
) -> BenchResult:
    client = redis.Redis(connection_pool=pool)
    rnd = random.Random(worker_seed)
    latencies_ms: list[float] = []
    errors = 0
    done = 0

    for _ in range(requests):
        key_id = rnd.randrange(keyspace)
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
            else:  # pipeline
                pipe = client.pipeline(transaction=False)
                for _ in range(pipeline_size):
                    key_id = rnd.randrange(keyspace)
                    key = f"{key_prefix}:{key_id}"
                    pipe.set(key, payload)
                    pipe.get(key)
                pipe.execute()
            done += 1
        except redis.RedisError:
            errors += 1
        finally:
            elapsed_ms = (time.perf_counter_ns() - started) / 1_000_000
            latencies_ms.append(elapsed_ms)

    return BenchResult(latencies_ms=latencies_ms, errors=errors, requests_done=done)


def pct(latencies: list[float], p: float) -> float:
    if not latencies:
        return 0.0
    k = max(0, min(len(latencies) - 1, int(round((p / 100) * (len(latencies) - 1)))))
    return latencies[k]


def main() -> None:
    args = build_parser().parse_args()
    if args.requests <= 0 or args.workers <= 0 or args.keyspace <= 0 or args.value_size <= 0:
        raise SystemExit("requests/workers/keyspace/value-size must be > 0")
    if args.op == "pipeline" and args.pipeline_size <= 0:
        raise SystemExit("pipeline-size must be > 0")

    random.seed(args.seed)
    payload = random_payload(args.value_size, random.Random(args.seed))

    pool = redis.ConnectionPool.from_url(args.url, max_connections=max(100, args.workers * 2))
    client = redis.Redis(connection_pool=pool)

    print(f"[setup] ping={client.ping()} url={args.url}")
    print(
        f"[setup] op={args.op} requests={args.requests} workers={args.workers} "
        f"keyspace={args.keyspace} value_size={args.value_size}"
    )

    if args.op in {"get", "mixed", "pipeline"}:
        print("[setup] pre-filling keys...")
        prefill_keys(client, args.key_prefix, args.keyspace, payload)

    if args.warmup > 0:
        print(f"[warmup] running {args.warmup} requests...")
        _ = run_worker(
            pool=pool,
            op=args.op,
            key_prefix=args.key_prefix,
            keyspace=args.keyspace,
            payload=payload,
            requests=args.warmup,
            worker_seed=args.seed + 100_000,
            pipeline_size=args.pipeline_size,
        )

    pieces = split_work(args.requests, args.workers)
    started = time.perf_counter()
    lock = threading.Lock()
    merged_latencies: list[float] = []
    total_errors = 0
    total_done = 0

    def consume(result: BenchResult) -> None:
        nonlocal total_errors, total_done
        with lock:
            merged_latencies.extend(result.latencies_ms)
            total_errors += result.errors
            total_done += result.requests_done

    with ThreadPoolExecutor(max_workers=args.workers) as executor:
        futures = []
        for idx, chunk in enumerate(pieces):
            if chunk == 0:
                continue
            futures.append(
                executor.submit(
                    run_worker,
                    pool,
                    args.op,
                    args.key_prefix,
                    args.keyspace,
                    payload,
                    chunk,
                    args.seed + idx + 1,
                    args.pipeline_size,
                )
            )
        for f in futures:
            consume(f.result())

    duration = time.perf_counter() - started
    merged_latencies.sort()
    throughput = total_done / duration if duration > 0 else 0.0
    avg = statistics.fmean(merged_latencies) if merged_latencies else 0.0

    result = {
        "label": args.label,
        "url": args.url,
        "op": args.op,
        "requests": args.requests,
        "requests_done": total_done,
        "workers": args.workers,
        "keyspace": args.keyspace,
        "value_size": args.value_size,
        "pipeline_size": args.pipeline_size,
        "warmup": args.warmup,
        "seed": args.seed,
        "errors": total_errors,
        "duration_s": round(duration, 6),
        "throughput_rps": round(throughput, 6),
        "latency_avg_ms": round(avg, 6),
        "latency_p50_ms": round(pct(merged_latencies, 50), 6),
        "latency_p95_ms": round(pct(merged_latencies, 95), 6),
        "latency_p99_ms": round(pct(merged_latencies, 99), 6),
    }

    if args.output_format == "json":
        payload = json.dumps(result, ensure_ascii=True)
        print(payload)
        if args.output_file:
            with open(args.output_file, "w", encoding="utf-8") as f:
                f.write(payload + "\n")
    else:
        print("\n=== Redis Perf Result ===")
        print(f"duration_s       : {duration:.4f}")
        print(f"requests_done    : {total_done}")
        print(f"errors           : {total_errors}")
        print(f"throughput_rps   : {throughput:.2f}")
        print(f"latency_avg_ms   : {avg:.3f}")
        print(f"latency_p50_ms   : {pct(merged_latencies, 50):.3f}")
        print(f"latency_p95_ms   : {pct(merged_latencies, 95):.3f}")
        print(f"latency_p99_ms   : {pct(merged_latencies, 99):.3f}")
        if args.output_file:
            with open(args.output_file, "w", encoding="utf-8") as f:
                f.write(json.dumps(result, ensure_ascii=True) + "\n")

    if args.cleanup:
        print("[cleanup] deleting benchmark keys...")
        cleanup_keys(client, args.key_prefix)
        print("[cleanup] done")


if __name__ == "__main__":
    main()
