"""Generate comparison charts from benchmark results."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from compare import aggregate_results


def plot_comparison(
    fire_results: list[dict[str, Any]],
    official_results: list[dict[str, Any]],
    output_dir: str | Path,
    label_a: str = "fire-redis",
    label_b: str = "redis-official",
) -> str | None:
    """Generate a throughput comparison bar chart.

    Returns the path to the saved PNG, or None if plotting fails.
    """
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        import numpy as np
    except ImportError:
        print("[plot] matplotlib not installed. Install with: pip install matplotlib")
        return None

    # Aggregate runs by op name
    fire_by_op: dict[str, list[dict]] = {}
    official_by_op: dict[str, list[dict]] = {}
    for r in fire_results:
        fire_by_op.setdefault(r["op"], []).append(r)
    for r in official_results:
        official_by_op.setdefault(r["op"], []).append(r)

    common_ops = sorted(set(fire_by_op.keys()) & set(official_by_op.keys()))
    if not common_ops:
        print("[plot] No common ops between the two result sets.")
        return None

    fire_avg = [aggregate_results(fire_by_op[op])["throughput_rps"] for op in common_ops]
    official_avg = [aggregate_results(official_by_op[op])["throughput_rps"] for op in common_ops]

    x = np.arange(len(common_ops))
    width = 0.35

    fig, ax = plt.subplots(figsize=(max(10, len(common_ops) * 0.8), 6))
    bars1 = ax.bar(x - width / 2, fire_avg, width, label=label_a, color="#e24a33")
    bars2 = ax.bar(x + width / 2, official_avg, width, label=label_b, color="#2c7bb6")

    ax.set_ylabel("Throughput (requests / sec)")
    ax.set_title("Redis Benchmark Comparison")
    ax.set_xticks(x)
    ax.set_xticklabels(common_ops, rotation=45, ha="right")
    ax.legend()
    ax.grid(axis="y", alpha=0.3)

    # Annotate values on bars
    for bar in bars1:
        ax.text(bar.get_x() + bar.get_width() / 2, bar.get_height(),
                f"{bar.get_height():,.0f}", ha="center", va="bottom", fontsize=8)
    for bar in bars2:
        ax.text(bar.get_x() + bar.get_width() / 2, bar.get_height(),
                f"{bar.get_height():,.0f}", ha="center", va="bottom", fontsize=8)

    fig.tight_layout()
    out_dir = Path(output_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    out_path = out_dir / "compare-throughput.png"
    fig.savefig(out_path, dpi=150)
    plt.close(fig)
    print(f"[plot] Comparison chart saved to {out_path}")
    return str(out_path)
