# Redis Perf (Python)

Cross-platform Redis benchmark tool with automatic Docker comparison.

## Quick Start

```bash
cd perf
uv sync        # or: pip install -e .

# Quick comparison (single mode): fire-redis vs redis-official
python perf.py

# Full benchmark matrix
python perf.py --mode full

# Attach to existing Redis, skip Docker
python perf.py --url redis://192.168.1.5:6379
```

## Structure

| File | Role |
|------|------|
| `perf.py` | **唯一入口** — CLI 解析 + 主流程编排 |
| `config.py` | 加载 TOML 配置文件 |
| `runner.py` | 核心 benchmark 引擎（多线程压测 Redis） |
| `docker_.py` | Docker 容器启停管理 |
| `compare.py` | 结果对比逻辑 + 表格输出 |
| `plotter.py` | 生成对比柱状图（需 matplotlib） |

## Configuration

| File | Purpose |
|------|---------|
| `perf.toml` | Docker 设置（镜像、CPU、内存等） |
| `single.toml` | 快速模式 — 少量 ops，单规模 |
| `full.toml` | 完整模式 — 全部 ops × 多 scale × 3 轮重复 |

## Modes

| Mode | 用途 | 参数 |
|------|------|------|
| `single`（默认） | 快速 smoke-test, ~11 个 op | `python perf.py` |
| `full` | 全量矩阵, 全部 38 个 op × 3 个 scale × 3 轮 | `python perf.py --mode full` |

## Output

- `results/<timestamp>-fire-redis.json`
- `results/<timestamp>-redis-official.json`
- `results/compare-throughput.png`（需安装 matplotlib）

All tools are fully **cross-platform** (Windows / Linux / macOS).

## Install

```bash
cd perf
uv sync        # or: pip install -e .
```

After install, you can use the CLI entry points directly:

```bash
redis-perf                  # same as: python benchmark.py
redis-perf-matrix           # same as: python run_matrix.py
redis-perf-compare          # same as: python compare_results.py
redis-perf-compare-images   # same as: python compare_images.py
```

Dependencies are declared in `pyproject.toml`.

## Single Benchmark (`benchmark.py`)

Default mixed workload:

```bash
python benchmark.py
```

Use a specific Redis URL and higher concurrency:

```bash
python benchmark.py --url redis://127.0.0.1:6379/0 --workers 100 --requests 200000 --op mixed
```

### Supported Operations

**String ops:** `set`, `get`, `mixed` (50/50 set+get), `incr`, `decr`, `append`, `strlen`,
`mget`, `mset`, `del`, `exists`, `pipeline` (batched set+get)

```bash
# String comparison set
python benchmark.py --op set     --workers 50 --requests 100000 --value-size 256
python benchmark.py --op get     --workers 50 --requests 100000
python benchmark.py --op incr    --workers 50 --requests 100000
python benchmark.py --op decr    --workers 50 --requests 100000
python benchmark.py --op append  --workers 50 --requests 100000
python benchmark.py --op mset    --workers 50 --requests 100000 --pipeline-size 10
python benchmark.py --op del     --workers 50 --requests 100000
python benchmark.py --op exists  --workers 50 --requests 100000
python benchmark.py --op pipeline --pipeline-size 20 --workers 50 --requests 50000
```

**Hash ops:** `hset`, `hget`, `hdel`, `hlen`, `hexists`, `hkeys`, `hvals`, `hgetall`

```bash
python benchmark.py --op hset     --workers 50 --requests 100000
python benchmark.py --op hget     --workers 50 --requests 100000
python benchmark.py --op hdel     --workers 50 --requests 100000
python benchmark.py --op hlen     --workers 50 --requests 100000
python benchmark.py --op hexists  --workers 50 --requests 100000
python benchmark.py --op hkeys    --workers 50 --requests 100000
python benchmark.py --op hvals    --workers 50 --requests 100000
python benchmark.py --op hgetall  --workers 50 --requests 100000
```

**List ops:** `lpush`, `lpop`, `rpush`, `rpop`, `llen`, `lindex`, `lrange`

```bash
python benchmark.py --op lpush   --workers 50 --requests 100000
python benchmark.py --op lpop    --workers 50 --requests 100000
python benchmark.py --op rpush   --workers 50 --requests 100000
python benchmark.py --op rpop    --workers 50 --requests 100000
python benchmark.py --op llen    --workers 50 --requests 100000
python benchmark.py --op lrange  --workers 50 --requests 100000 --pipeline-size 10
```

**Set ops:** `sadd`, `sismember`, `srem`, `smembers`, `scard`, `spop`

```bash
python benchmark.py --op sadd       --workers 50 --requests 100000
python benchmark.py --op sismember  --workers 50 --requests 100000
python benchmark.py --op smembers   --workers 50 --requests 100000
python benchmark.py --op spop       --workers 50 --requests 100000
```

**Sorted-set ops:** `zadd`, `zscore`, `zrem`, `zcard`, `zcount`, `zrange`

```bash
python benchmark.py --op zadd    --workers 50 --requests 100000
python benchmark.py --op zscore  --workers 50 --requests 100000
python benchmark.py --op zrange  --workers 50 --requests 100000 --pipeline-size 10
python benchmark.py --op zcard   --workers 50 --requests 100000
```

**Connection:** `ping`

```bash
python benchmark.py --op ping --workers 50 --requests 100000
```

**Mixed data type:** `all-mixed` (round-robin across string, hash, list, set, zset, ping, exists)

```bash
python benchmark.py --op all-mixed --workers 80 --requests 200000 --keyspace 20000
```

**Cleanup after run:**

```bash
python benchmark.py --op set --cleanup
```

**Machine-readable JSON output:**

```bash
python benchmark.py --output-format json --label fire-redis-baseline
```

On Windows, using local venv:

```powershell
.venv\Scripts\python.exe benchmark.py
```

## Common Arguments

- `--url`: Redis URL
- `--op`: operation — see table above for all supported ops
- `--requests`: total logical requests (ignored if `--time` is set)
- `--time`: run duration in seconds (e.g. `--time 30` runs for 30 seconds, overrides `--requests`)
- `--workers`: concurrent worker count
- `--key-prefix`: key prefix (default: `perf`)
- `--keyspace`: number of keys
- `--value-size`: value size in bytes
- `--pipeline-size`: per-request batch size (used by `pipeline`, `mget`, `mset`, `lrange`, `zrange`)
- `--key-distribution`: `uniform` (default) or `gaussian` (hot-spot pattern)
- `--warmup`: warmup request count
- `--cleanup`: delete benchmark keys after run
- `--output-format`: `text|json`
- `--output-file`: optional result file (stdout is always printed)
- `--label`: optional label for automation/reporting

## Comparing Two Servers

### Step 1: Run benchmarks against each server

```bash
# Against server A (e.g. Redis official)
python benchmark.py --url redis://127.0.0.1:6379/0 --op mixed --workers 50 --requests 200000 \
  --output-format json --output-file results-redis.json --label redis-official

# Against server B (e.g. fire-redis)
python benchmark.py --url redis://127.0.0.2:6379/0 --op mixed --workers 50 --requests 200000 \
  --output-format json --output-file results-fire.json --label fire-redis
```

### Step 2: Generate comparison report

```bash
# Side-by-side comparison table
python compare_results.py \
  --baseline results-redis.json --baseline-label "Redis Official" \
  --target results-fire.json --target-label "Fire Redis"

# With HTML report
python compare_results.py \
  --baseline results-redis.json --baseline-label "Redis Official" \
  --target results-fire.json --target-label "Fire Redis" \
  --html comparison.html

# Focus on a specific metric
python compare_results.py --baseline a.json --target b.json --field throughput_rps

# Compare matrix results (aggregated across repeats)
python compare_results.py --baseline matrix-a.json --target matrix-b.json --matrix
```

### Output Metrics

The comparison shows for each metric:
- Values from both servers
- Percentage change: **positive = target is better** (higher throughput, lower latency)

Metrics reported:
- `throughput_rps`: requests per second
- `latency_min/avg/p50/p95/p99/p99.9/p99.99/max/stdev_ms`: full latency distribution

## Duration Mode

Run for a fixed time instead of a fixed request count:

```bash
python benchmark.py --time 30 --workers 100 --op mixed  # 30-second sustained load
python benchmark.py --time 60 --workers 50 --op pipeline --pipeline-size 20
```

Duration mode is more stable for long-running performance comparisons.

## Key Distribution

Simulate real-world hot-spot access patterns:

```bash
# Gaussian distribution: ~68% of requests hit the middle 34% of keyspace
python benchmark.py --op get --key-distribution gaussian --keyspace 10000 --requests 200000
```

## Image-vs-Image Comparison (cross-platform)

Compare two Redis-compatible Docker images under identical resource limits:

```bash
# From the perf/ directory
python compare_images.py \
  --fire-image fire-redis:bench \
  --redis-image redis:7-alpine \
  --cpus 1.0 \
  --memory 512m \
  -- --op mixed --workers 50 --requests 200000 --keyspace 10000 --value-size 128 --seed 42
```

Or via the CLI entry point (after `uv sync` / `pip install -e .`):

```bash
redis-perf-compare-images \
  --fire-image fire-redis:bench \
  --redis-image redis:7-alpine \
  --cpus 2 --memory 1g \
  -- --op get --workers 100 --requests 300000
```

Results are written to `perf/results/*.json`.

All arguments are optional; defaults match the embedded example above.

**Available options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--fire-image` | `fire-redis` | fire-redis Docker image tag |
| `--redis-image` | `redis` | Official redis Docker image tag |
| `--cpus` | `1.0` | Docker CPU limit |
| `--memory` | `512m` | Docker memory limit |
| `--pids-limit` | `256` | Docker pids limit |
| `--port` | `6379` | Host port for benchmark |
| `--wait-secs` | `2.0` | Container startup wait time |
| `--` | — | Separator for extra `benchmark.py` arguments |

## Scale Matrix Benchmark (`run_matrix.py`)

### Scale Factor Sweep

Both `requests` and `keyspace` are multiplied by each scale:

```bash
python run_matrix.py \
  --url redis://127.0.0.1:6379/0 \
  --scales 1,2,4,8 \
  --ops set,get,mixed,incr,ping,hset,hget,lpush,sadd,zadd \
  --repeat 3 \
  --output-json matrix-results.json \
  --output-csv matrix-results.csv
```

### Dimension Sweep

Independently sweep value size, worker count, or pipeline size across chosen ops:

```bash
# Sweep value sizes: compare 64B, 1KB, 4KB payloads
python run_matrix.py \
  --ops set,get,mixed \
  --scales 1 \
  --sweep-value-size 64,1024,4096 \
  --repeat 3 \
  --output-json value-sweep.json \
  --output-csv value-sweep.csv

# Sweep worker counts: compare concurrency levels
python run_matrix.py \
  --ops mixed \
  --scales 1 \
  --sweep-workers 10,50,100,200 \
  --repeat 3 \
  --output-json worker-sweep.json \
  --output-csv worker-sweep.csv

# Sweep pipeline sizes
python run_matrix.py \
  --ops pipeline,mget \
  --scales 1 \
  --sweep-pipeline 1,5,10,20,50 \
  --repeat 3 \
  --output-json pipeline-sweep.json \
  --output-csv pipeline-sweep.csv

# Combine sweeps (cross-product)
python run_matrix.py \
  --ops set \
  --scales 1 \
  --sweep-value-size 64,256,1024 \
  --sweep-workers 10,50,100 \
  --repeat 2 \
  --output-json multi-sweep.json
```

### Explicit Config Cases

```bash
python run_matrix.py --config scales.example.json --repeat 2
```

Windows:

```powershell
.venv\Scripts\python.exe run_matrix.py --config scales.example.json --repeat 2
```

### Output Files

`run_matrix.py` internally calls `benchmark.py --output-format json` and generates:

- `matrix-results.json`: full structured results
- `matrix-results.csv`: tabular output for spreadsheet analysis
- `matrix-results.agg.json` / `.agg.csv`: **aggregated statistics** (mean/min/max/stdev across repeats)

When `--repeat > 1`, the aggregated files give you run-to-run stability metrics.
