# Redis Perf (Python)

Python benchmark tools for Redis-compatible servers.

This folder provides:

- `main.py`: single benchmark run with latency and throughput metrics
- `run_matrix.py`: batch benchmark runner across multiple workload scales
- `../scripts/compare-images.sh`: compare `fire-redis` vs official `redis` under equal container limits

## Install

Dependencies are declared in `pyproject.toml`.

## Single Benchmark (`main.py`)

Default mixed workload:

```bash
python main.py
```

Use a specific Redis URL and higher concurrency:

```bash
python main.py --url redis://127.0.0.1:6379/0 --workers 100 --requests 200000 --op mixed
```

SET-only workload:

```bash
python main.py --op set --workers 50 --requests 100000 --value-size 256
```

GET-only workload (keys are auto-prefilled):

```bash
python main.py --op get --workers 50 --requests 100000
```

Pipeline workload:

```bash
python main.py --op pipeline --pipeline-size 20 --workers 50 --requests 50000
```

Delete generated keys after the run:

```bash
python main.py --cleanup
```

Machine-readable JSON output:

```bash
python main.py --output-format json --label fire-redis-baseline
```

On Windows, you can use your local venv explicitly:

```powershell
.venv\Scripts\python.exe main.py
```

## Common Arguments

- `--url`: Redis URL
- `--op`: `set|get|mixed|pipeline`
- `--requests`: total logical requests
- `--workers`: concurrent worker count
- `--key-prefix`: key prefix (default: `perf`)
- `--keyspace`: number of keys
- `--value-size`: value size in bytes
- `--pipeline-size`: per-request batch size for pipeline mode
- `--warmup`: warmup request count
- `--cleanup`: delete benchmark keys after run
- `--output-format`: `text|json`
- `--output-file`: optional result file (stdout is still printed)
- `--label`: optional label for automation/reporting

## Image-vs-Image Comparison

Run from the repository root:

```bash
./scripts/compare-images.sh \
  --fire-image fire-redis:bench \
  --redis-image redis:7-alpine \
  --cpus 1.0 \
  --memory 512m \
  -- --op mixed --workers 50 --requests 200000 --keyspace 10000 --value-size 128 --seed 42
```

Results are written to `perf/results/*.json`.

## Scale Matrix Benchmark (`run_matrix.py`)

Run predefined scale factors (both `requests` and `keyspace` are multiplied by scale):

```bash
python run_matrix.py \
  --url redis://127.0.0.1:6379/0 \
  --scales 1,2,4,8 \
  --ops set,get,mixed,pipeline \
  --repeat 3 \
  --output-json matrix-results.json \
  --output-csv matrix-results.csv
```

Run explicit cases from config:

```bash
python run_matrix.py --config scales.example.json --repeat 2
```

Windows example:

```powershell
.venv\Scripts\python.exe run_matrix.py --config scales.example.json --repeat 2
```

`run_matrix.py` internally calls `main.py --output-format json` and generates:

- `matrix-results.json`: full structured results
- `matrix-results.csv`: tabular output for spreadsheet analysis
