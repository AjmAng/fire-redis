from __future__ import annotations

import argparse
import csv
import json
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run perf/main.py across multiple data scales")
    parser.add_argument("--url", default="redis://127.0.0.1:6379/0", help="Redis URL")
    parser.add_argument("--main-script", default="main.py", help="Path to perf main script")
    parser.add_argument(
        "--ops",
233        default="set,get,mixed,pipeline,incr,mget,hset,hget,lpush,lpop,sadd,sismember,zadd,zscore",
        help="Comma-separated ops to run (must be supported by main.py --op)",
    )
    parser.add_argument(
        "--scales",
        default="1,2,4",
        help="Comma-separated scale factors applied to requests/keyspace",
    )
    parser.add_argument("--repeat", type=int, default=1, help="Runs per case")
    parser.add_argument("--base-requests", type=int, default=100_000)
    parser.add_argument("--base-workers", type=int, default=50)
    parser.add_argument("--base-keyspace", type=int, default=10_000)
    parser.add_argument("--value-size", type=int, default=128)
    parser.add_argument("--pipeline-size", type=int, default=10)
    parser.add_argument("--warmup", type=int, default=1_000)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--key-prefix", default="perf")
    parser.add_argument("--config", default="", help="Optional JSON file describing explicit cases")
    parser.add_argument("--output-json", default="matrix-results.json", help="Output JSON file")
    parser.add_argument("--output-csv", default="matrix-results.csv", help="Output CSV file")
    return parser


def parse_csv_ints(raw: str) -> list[int]:
    vals = [v.strip() for v in raw.split(",") if v.strip()]
    return [int(v) for v in vals]


def parse_csv_str(raw: str) -> list[str]:
    return [v.strip() for v in raw.split(",") if v.strip()]


def parse_json_line(stdout: str) -> dict[str, Any]:
    for line in reversed(stdout.splitlines()):
        line = line.strip()
        if not line:
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(obj, dict) and "throughput_rps" in obj:
            return obj
    raise RuntimeError("No benchmark JSON payload found in subprocess output")


def load_cases(args: argparse.Namespace) -> list[dict[str, Any]]:
    if args.config:
        with open(args.config, "r", encoding="utf-8") as f:
            data = json.load(f)
        cases = data.get("cases", data)
        if not isinstance(cases, list):
            raise SystemExit("config must contain a list or {\"cases\": [...]} structure")
        return cases

    scales = parse_csv_ints(args.scales)
    ops = parse_csv_str(args.ops)

    cases: list[dict[str, Any]] = []
    for scale in scales:
        for op in ops:
            cases.append(
                {
                    "name": f"scale{scale}-{op}",
                    "scale": scale,
                    "op": op,
                    "requests": args.base_requests * scale,
                    "workers": args.base_workers,
                    "keyspace": args.base_keyspace * scale,
                    "value_size": args.value_size,
                    "pipeline_size": args.pipeline_size,
                    "warmup": args.warmup,
                    "seed": args.seed,
                    "key_prefix": f"{args.key_prefix}-s{scale}-{op}",
                }
            )
    return cases


def run_case(main_script: str, url: str, case: dict[str, Any], repeat_idx: int) -> dict[str, Any]:
    label = f"{case['name']}-r{repeat_idx + 1}"
    cmd = [
        sys.executable,
        main_script,
        "--url",
        url,
        "--op",
        str(case["op"]),
        "--requests",
        str(case["requests"]),
        "--workers",
        str(case["workers"]),
        "--keyspace",
        str(case["keyspace"]),
        "--value-size",
        str(case["value_size"]),
        "--pipeline-size",
        str(case["pipeline_size"]),
        "--warmup",
        str(case["warmup"]),
        "--seed",
        str(case["seed"] + repeat_idx),
        "--key-prefix",
        str(case["key_prefix"]),
        "--cleanup",
        "--label",
        label,
        "--output-format",
        "json",
    ]

    print(f"[run] {label} op={case['op']} req={case['requests']} keyspace={case['keyspace']}")
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        raise RuntimeError(
            f"Case {label} failed with code {proc.returncode}\nSTDOUT:\n{proc.stdout}\nSTDERR:\n{proc.stderr}"
        )

    result = parse_json_line(proc.stdout)
    result["case_name"] = case["name"]
    result["scale"] = case.get("scale", "custom")
    result["repeat"] = repeat_idx + 1
    return result


def write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    if not rows:
        return
    fields = [
        "case_name",
        "scale",
        "repeat",
        "label",
        "op",
        "requests",
        "requests_done",
        "workers",
        "keyspace",
        "value_size",
        "pipeline_size",
        "errors",
        "duration_s",
        "throughput_rps",
        "latency_avg_ms",
        "latency_p50_ms",
        "latency_p95_ms",
        "latency_p99_ms",
        "url",
    ]
    with open(path, "w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=fields)
        writer.writeheader()
        for row in rows:
            writer.writerow({k: row.get(k) for k in fields})


def main() -> None:
    args = build_parser().parse_args()
    if args.repeat <= 0:
        raise SystemExit("repeat must be > 0")

    main_script = str(Path(args.main_script).resolve())
    cases = load_cases(args)

    started = time.time()
    rows: list[dict[str, Any]] = []

    for case in cases:
        for repeat_idx in range(args.repeat):
            rows.append(run_case(main_script, args.url, case, repeat_idx))

    out_json = Path(args.output_json).resolve()
    out_csv = Path(args.output_csv).resolve()

    out_json.write_text(
        json.dumps(
            {
                "url": args.url,
                "started_at": int(started),
                "duration_s": round(time.time() - started, 6),
                "cases": len(cases),
                "repeat": args.repeat,
                "results": rows,
            },
            ensure_ascii=True,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )

    write_csv(out_csv, rows)

    print("\n=== Matrix Done ===")
    print(f"cases            : {len(cases)}")
    print(f"repeat           : {args.repeat}")
    print(f"total_runs       : {len(rows)}")
    print(f"output_json      : {out_json}")
    print(f"output_csv       : {out_csv}")


if __name__ == "__main__":
    main()

