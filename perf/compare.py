"""Compare two sets of benchmark results and print a side-by-side table."""

from __future__ import annotations

from typing import Any

LATENCY_FIELDS = [
    "latency_min_ms", "latency_avg_ms", "latency_median_ms", "latency_p50_ms",
    "latency_p95_ms", "latency_p99_ms", "latency_p99_9_ms", "latency_p99_99_ms",
    "latency_max_ms", "latency_stdev_ms",
]

SCALAR_FIELDS = ["throughput_rps", "duration_s"]


def fmt(val: Any) -> str:
    if isinstance(val, float):
        return f"{val:>12.3f}"
    if isinstance(val, int):
        return f"{val:>12d}"
    return f"{str(val):>12}"


def pct_diff(a: float, b: float) -> str:
    if a == 0:
        return "   N/A%  "
    diff = ((b - a) / a) * 100
    sign = "+" if diff >= 0 else ""
    return f"{sign}{diff:>+7.2f}%"


def compare_single(
    base: dict[str, Any],
    target: dict[str, Any],
    label_a: str,
    label_b: str,
) -> list[dict[str, Any]]:
    """Compare two single-run results. Returns rows for print_table()."""
    rows: list[dict[str, Any]] = []

    for fname in ["op", "requests_done", "workers", "keyspace", "value_size", "pipeline_size", "errors"]:
        if fname in base or fname in target:
            rows.append({
                "field": fname,
                label_a: base.get(fname, "-"),
                label_b: target.get(fname, "-"),
                "diff": "",
            })

    for fname in SCALAR_FIELDS:
        va = base.get(fname)
        vb = target.get(fname)
        if va is None and vb is None:
            continue
        va = va if va is not None else 0
        vb = vb if vb is not None else 0
        rows.append({
            "field": fname,
            label_a: fmt(va),
            label_b: fmt(vb),
            "diff": pct_diff(float(va), float(vb)),
        })

    for fname in LATENCY_FIELDS:
        va = base.get(fname)
        vb = target.get(fname)
        if va is None and vb is None:
            continue
        va = va if va is not None else 0
        vb = vb if vb is not None else 0
        rows.append({
            "field": fname,
            label_a: fmt(va),
            label_b: fmt(vb),
            "diff": pct_diff(float(va), float(vb)),
        })

    return rows


def print_table(rows: list[dict[str, Any]], label_a: str, label_b: str) -> None:
    """Print a formatted comparison table to stdout."""
    if not rows:
        print("  (no comparable data)")
        return
    fields = list(rows[0].keys())
    col_widths: dict[str, int] = {}
    for k in fields:
        vals = [str(r.get(k, "")) for r in rows]
        vals.append(k)
        col_widths[k] = max(len(v) for v in vals)
    col_widths["field"] = max(col_widths["field"], 22)
    col_widths[label_a] = max(col_widths[label_a], 14)
    col_widths[label_b] = max(col_widths[label_b], 14)
    col_widths["diff"] = max(col_widths["diff"], 12)

    sep = " | ".join("-" * w for w in col_widths.values())
    fmt_row = " | ".join(f"{{{k}:<{col_widths[k]}}}" for k in fields)

    print(f"  {fmt_row.format(**{k: k for k in fields})}")
    print(f"  {sep}")
    for row in rows:
        print(f"  {fmt_row.format(**row)}")


def aggregate_results(results: list[dict[str, Any]]) -> dict[str, Any]:
    """Average multiple runs (same case, different repeats) into one result."""
    if not results:
        return {}
    avg: dict[str, Any] = {}
    for key in results[0]:
        vals = [r.get(key) for r in results if isinstance(r.get(key), (int, float))]
        if vals:
            avg[key] = sum(vals) / len(vals)
        else:
            avg[key] = results[0].get(key)
    return avg
