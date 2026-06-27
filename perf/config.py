"""Configuration loader — reads TOML files for benchmark settings."""

from __future__ import annotations

import itertools
import tomllib
from pathlib import Path
from typing import Any


def load_toml(path: str | Path) -> dict[str, Any]:
    """Load a TOML file and return as a dict."""
    with open(path, "rb") as f:
        return tomllib.load(f)


def load_config(config_dir: str | Path, mode: str) -> dict[str, Any]:
    """Load perf.toml (docker/output settings) + mode-specific config (single/full).

    Returns a merged dict with keys:
      - docker: container settings
      - output: output directory, etc.
      - cases: list of case dicts (each has op, requests, workers, keyspace, ...)
      - warmup, key_prefix, etc.
    """
    config_dir = Path(config_dir)
    cfg = load_toml(config_dir / "perf.toml")

    if mode == "single":
        case_cfg = load_toml(config_dir / "single.toml")
        cfg["cases"] = _normalize_cases(case_cfg["cases"], case_cfg)
    elif mode == "full":
        case_cfg = load_toml(config_dir / "full.toml")
        cfg["cases"] = _expand_full_cases(case_cfg)
    else:
        raise ValueError(f"Unknown mode: {mode}")

    # Merge top-level keys from case config (warmup, key_prefix, etc.)
    for key in ("warmup", "key_prefix", "seed", "cleanup"):
        if key in case_cfg:
            cfg[key] = case_cfg[key]

    cfg.setdefault("warmup", 1000)
    cfg.setdefault("key_prefix", "perf")
    cfg.setdefault("cleanup", True)

    return cfg


def _normalize_cases(cases: list[dict], defaults: dict) -> list[dict]:
    """Fill in missing fields from defaults."""
    result = []
    for c in cases:
        case = dict(c)
        for key in ("requests", "workers", "keyspace", "value_size", "pipeline_size", "seed"):
            if key not in case and key in defaults:
                case[key] = defaults[key]
        case.setdefault("value_size", 128)
        case.setdefault("pipeline_size", 1)
        case.setdefault("seed", 42)
        case.setdefault("key_distribution", "uniform")
        result.append(case)
    return result


def _expand_full_cases(cfg: dict) -> list[dict]:
    """Generate cross-product of ops × scales for full mode."""
    ops: list[str] = cfg["ops"]
    scales: list[int] = cfg["scales"]
    repeat: int = cfg.get("repeat", 1)
    base_req: int = cfg.get("base_requests", 100_000)
    base_w: int = cfg.get("base_workers", 50)
    base_ks: int = cfg.get("base_keyspace", 10_000)
    base_vs: int = cfg.get("base_value_size", 128)
    base_pp: int = cfg.get("base_pipeline_size", 10)
    seed: int = cfg.get("seed", 42)

    cases: list[dict] = []
    for op, scale in itertools.product(ops, scales):
        if op in ("pipeline", "mget", "mset", "lrange", "zrange"):
            pp = base_pp
        else:
            pp = 1
        for r in range(repeat):
            s = seed + r
            cases.append({
                "op": op,
                "scale": scale,
                "requests": base_req * scale,
                "workers": base_w,
                "keyspace": base_ks * scale,
                "value_size": base_vs,
                "pipeline_size": pp,
                "warmup": cfg.get("warmup", 1000),
                "seed": s,
                "key_prefix": cfg.get("key_prefix", "perf-full"),
                "key_distribution": "uniform",
                "repeat_idx": r + 1,
            })
    return cases
