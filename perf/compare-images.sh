#!/usr/bin/env bash
set -euo pipefail

# Compare fire-redis image and official redis image under identical runtime limits.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PERF_MAIN="${ROOT_DIR}/perf/main.py"
RESULT_DIR="${ROOT_DIR}/perf/results"

FIRE_IMAGE="fire-redis"
REDIS_IMAGE="redis"
CPUS="1.0"
MEMORY="512m"
PIDS_LIMIT="256"
PORT="6379"
WAIT_SECS="2"

BENCH_ARGS=(--op mixed --workers 50 --requests 200000 --keyspace 10000 --value-size 128 --seed 42)

usage() {
  cat <<'EOF'
Usage:
  ./scripts/compare-images.sh [options] [-- <extra perf args>]

Options:
  --fire-image <image>      fire-redis image tag (default: fire-redis:bench)
  --redis-image <image>     official redis image tag (default: redis:7-alpine)
  --cpus <num>              docker cpu limit (default: 1.0)
  --memory <size>           docker memory limit (default: 512m)
  --pids-limit <num>        docker pids limit (default: 256)
  --port <port>             host port for benchmark (default: 6379)
  --wait-secs <n>           startup wait seconds before bench (default: 2)
  -h, --help                show this help

Examples:
  ./scripts/compare-images.sh
  ./scripts/compare-images.sh --cpus 2 --memory 1g -- --op get --workers 100 --requests 300000
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --fire-image)
      FIRE_IMAGE="$2"; shift 2 ;;
    --redis-image)
      REDIS_IMAGE="$2"; shift 2 ;;
    --cpus)
      CPUS="$2"; shift 2 ;;
    --memory)
      MEMORY="$2"; shift 2 ;;
    --pids-limit)
      PIDS_LIMIT="$2"; shift 2 ;;
    --port)
      PORT="$2"; shift 2 ;;
    --wait-secs)
      WAIT_SECS="$2"; shift 2 ;;
    --)
      shift
      BENCH_ARGS=("$@")
      break ;;
    -h|--help)
      usage
      exit 0 ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 1 ;;
  esac
done

VENV_PYTHON="${ROOT_DIR}/perf/.venv/Scripts/python.exe"
if [[ -x "${VENV_PYTHON}" ]]; then
  PYTHON_BIN="${VENV_PYTHON}"
elif command -v python3 >/dev/null 2>&1; then
  PYTHON_BIN="python3"
elif command -v python >/dev/null 2>&1; then
  PYTHON_BIN="python"
else
  echo "Python not found. Please create .venv in perf or ensure python is in PATH." >&2
  exit 1
fi

mkdir -p "${RESULT_DIR}"
TS="$(date +%Y%m%d-%H%M%S)"

run_case() {
  local name="$1"
  local image="$2"
  local container="bench-${name}-${TS}"
  local out_file="${RESULT_DIR}/${TS}-${name}.json"

  docker run --rm -d \
    --name "${container}" \
    -p "${PORT}:6379" \
    --cpus "${CPUS}" \
    --memory "${MEMORY}" \
    --memory-swap "${MEMORY}" \
    --pids-limit "${PIDS_LIMIT}" \
    --ulimit nofile=65535:65535 \
    -e REDIS_BIND=0.0.0.0 \
    -e REDIS_PORT=6379 \
    "${image}" >/dev/null

  sleep "${WAIT_SECS}"

  set +e
  "${PYTHON_BIN}" "${PERF_MAIN}" \
    --url "redis://127.0.0.1:${PORT}/0" \
    --label "${name}" \
    --output-format json \
    --output-file "${out_file}" \
    "${BENCH_ARGS[@]}"
  local status=$?
  set -e

  docker stop "${container}" >/dev/null || true

  if [[ ${status} -ne 0 ]]; then
    echo "Benchmark failed for ${name}" >&2
    exit ${status}
  fi

  echo "Saved ${name} result: ${out_file}"
}

run_case "fire-redis" "${FIRE_IMAGE}"
run_case "redis-official" "${REDIS_IMAGE}"

echo "Done. Results are under ${RESULT_DIR}"
