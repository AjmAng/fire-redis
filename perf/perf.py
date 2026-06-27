"""
Redis Perf — unified benchmark tool.

Usage:
    python perf.py                  # quick single-mode comparison
    python perf.py --mode full      # full matrix comparison
    python perf.py --port 6380      # use different host port
    python perf.py --skip-docker    # skip Docker, attach to existing Redis
    python perf.py --url redis://192.168.1.5:6379
"""

from __future__ import annotations

import argparse
import json
import logging
import sys
import time
from pathlib import Path
from typing import Any

import config as cfg
import compare as cmp
import docker_ as docker
import plotter as plt_
import runner

logging.basicConfig(
    level=logging.INFO,
    format="[%(levelname)s] %(message)s",
    stream=sys.stderr,
)
log = logging.getLogger("perf")

HERE = Path(__file__).resolve().parent


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description="Redis Perf — unified benchmark & comparison tool")
    p.add_argument("--mode", choices=["single", "full"], default="single",
                   help="Benchmark scale (default: single)")
    p.add_argument("--port", type=int, default=6379,
                   help="Host port for Redis (default: 6379)")
    p.add_argument("--url", default="",
                   help="Redis URL (if set, skips Docker container management)")
    p.add_argument("--skip-docker", action="store_true",
                   help="Skip Docker lifecycle — attach to existing Redis at --url")
    p.add_argument("--config-dir", default=str(HERE),
                   help="Directory containing TOML config files (default: perf/)")
    p.add_argument("--no-plot", action="store_true",
                   help="Skip chart generation")
    return p


def run_benchmarks(
    url: str,
    cases: list[dict[str, Any]],
    label: str,
    cleanup: bool,
    base_key_prefix: str = "perf",
) -> list[dict[str, Any]]:
    """Run all benchmark cases serially against *url*. Returns list of result dicts."""
    results: list[dict[str, Any]] = []
    total = len(cases)
    for idx, case in enumerate(cases):
        case_label = f"{label}-{case['op']}"
        if "scale" in case:
            case_label += f"-s{case['scale']}"
        if "repeat_idx" in case:
            case_label += f"-r{case['repeat_idx']}"

        log.info("[%d/%d] %s op=%s ...", idx + 1, total, case_label, case["op"])
        t0 = time.time()
        # Use unique key prefix per case to avoid WRONGTYPE conflicts
        key_prefix = f"{base_key_prefix}-{case['op']}"
        result = runner.run_case(
            url=url,
            op=case["op"],
            requests=case["requests"],
            workers=case["workers"],
            keyspace=case["keyspace"],
            value_size=case.get("value_size", 128),
            pipeline_size=case.get("pipeline_size", 1),
            warmup=case.get("warmup", 1000),
            seed=case.get("seed", 42),
            key_prefix=key_prefix,
            key_distribution=case.get("key_distribution", "uniform"),
            label=case_label,
            cleanup=False,  # we clean at the end
        )
        elapsed = time.time() - t0
        tput = result["throughput_rps"]
        log.info("  done in %.1f s  —  %.0f rps  —  p99 %.3f ms",
                 elapsed, tput, result["latency_p99_ms"])
        results.append(result)

    if cleanup:
        log.info("Cleaning up benchmark keys ...")
        from runner import _cleanup_keys
        import redis
        pool = redis.ConnectionPool.from_url(url, max_connections=10)
        client = redis.Redis(connection_pool=pool)
        for case in cases:
            key_prefix = f"{base_key_prefix}-{case['op']}"
            _cleanup_keys(client, key_prefix)

    return results


def save_results(results: list[dict[str, Any]], path: str | Path) -> None:
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(results, indent=2, ensure_ascii=False), encoding="utf-8")
    log.info("Results saved to %s", path)


def main() -> None:
    args = build_parser().parse_args()
    config = cfg.load_config(args.config_dir, args.mode)
    skip_docker = args.skip_docker or bool(args.url)
    port = args.port
    url = args.url or f"redis://127.0.0.1:{port}/0"
    output_dir = Path(config.get("output", {}).get("dir", str(HERE / "results")))
    output_dir.mkdir(parents=True, exist_ok=True)

    docker_cfg = config.get("docker", {})
    cases = config["cases"]
    cleanup = config.get("cleanup", True)
    ts = time.strftime("%Y%m%d-%H%M%S")

    # Determine which images to run
    images: list[tuple[str, str]] = []
    if skip_docker:
        # Single run — just use the url as-is, label "redis"
        images = [("redis", "")]
    else:
        images = [
            ("fire-redis", docker_cfg.get("fire_image", "fire-redis")),
            ("redis-official", docker_cfg.get("redis_image", "redis")),
        ]

    all_results: dict[str, list[dict[str, Any]]] = {}
    started_containers: list[tuple[str, str, str]] = []  # (docker_path, container_name, label)

    try:
        for label, image_name in images:
            container_name = f"bench-{label}-{ts}"

            if not skip_docker:
                dkr = docker.find_docker()
                docker.start_container(
                    dkr, image_name, container_name, port,
                    docker_cfg.get("cpus", "1.0"),
                    docker_cfg.get("memory", "512m"),
                    docker_cfg.get("pids_limit", "256"),
                )
                started_containers.append((dkr, container_name, label))
                docker.wait_for_ready(port, docker_cfg.get("wait_secs", 2.0))

            results = run_benchmarks(url, cases, label, cleanup=False,
                                     base_key_prefix=config.get("key_prefix", "perf"))
            all_results[label] = results
            result_file = output_dir / f"{ts}-{label}.json"
            save_results(results, result_file)

            if not skip_docker:
                docker.stop_container(dkr, container_name)
                # Remove from tracking so we don't double-stop in finally
                started_containers.pop()

    except Exception:
        log.exception("Benchmark failed — cleaning up containers ...")
        raise
    finally:
        # Ensure any running containers are stopped
        for dkr, cname, clabel in started_containers:
            log.warning("Cleaning up container %s (%s) ...", cname, clabel)
            docker.stop_container(dkr, cname)

    # ── Compare ────────────────────────────────────────────────────────────
    if len(all_results) == 2:
        label_a, label_b = list(all_results.keys())
        res_a, res_b = all_results[label_a], all_results[label_b]

        # Group by op for per-op comparison
        a_by_op: dict[str, list[dict]] = {}
        b_by_op: dict[str, list[dict]] = {}
        for r in res_a:
            a_by_op.setdefault(r["op"], []).append(r)
        for r in res_b:
            b_by_op.setdefault(r["op"], []).append(r)

        common_ops = sorted(set(a_by_op.keys()) & set(b_by_op.keys()))

        print(f"\n{'='*60}")
        print(f"  Comparison: {label_a} vs {label_b}")
        print(f"{'='*60}")

        for op in common_ops:
            avg_a = cmp.aggregate_results(a_by_op[op])
            avg_b = cmp.aggregate_results(b_by_op[op])
            print(f"\n  --- {op} ---")
            rows = cmp.compare_single(avg_a, avg_b, label_a, label_b)
            cmp.print_table(rows, label_a, label_b)

        # ── Plot ────────────────────────────────────────────────────────────
        if not args.no_plot:
            plt_.plot_comparison(res_a, res_b, output_dir, label_a, label_b)

    elif not skip_docker:
        log.warning("Only one set of results — skipping comparison.")

    log.info("All done. Results are under %s", output_dir)


if __name__ == "__main__":
    main()
