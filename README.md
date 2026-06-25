# fire-redis

A Redis-like server written in Rust with async I/O (`tokio`), RESP2 protocol support, in-memory data structures, optional persistence, and built-in observability.

## Highlights

- RESP2 codec with frame-safe decoding/encoding
- TCP server and CLI client binaries
- Core Redis-style commands for strings, lists, sets, hashes, and sorted sets
- Optional persistence manager (RDB snapshots and AOF logging)
- Runtime metrics (lock-free counters, latency histograms — via `INFO` and HTTP)
- OpenTelemetry tracing integration (via `OTEL_EXPORTER_OTLP_ENDPOINT`)
- Docker support and benchmark tooling

## Project Layout

- `src/bin/server.rs`: server entrypoint (`redis-server`)
- `src/bin/cli.rs`: CLI entrypoint (`redis-cli`)
- `src/resp.rs`: RESP codec and tests
- `src/commands/`: command parsing and execution
- `src/store/`: in-memory data structure implementations
- `src/metrics.rs`: lock-free atomic counters and latency histogram
- `src/observability.rs`: OpenTelemetry tracing integration
- `src/persistence/`: RDB/AOF persistence logic
- `perf/`: benchmark scripts and matrix runner
- `docs/`: architecture, roadmap, demo guide, tradeoffs, compatibility, observability

## Quick Start (Local)

### 1) Build

```bash
cargo build --release
```

### 2) Run server

```bash
cargo run --bin redis-server
```

By default, the server reads:

- `REDIS_BIND` (default: `127.0.0.1`)
- `REDIS_PORT` (default: `6379`)

Example:

```bash
REDIS_BIND=0.0.0.0 REDIS_PORT=6379 cargo run --bin redis-server
```

### 3) Use CLI

```bash
cargo run --bin redis-cli
```

Or run a one-shot command:

```bash
cargo run --bin redis-cli -- PING
cargo run --bin redis-cli -- SET hello world
cargo run --bin redis-cli -- GET hello
```

### 4) Interview demo replay

See `docs/DEMO.md` for the manual demo flow and command sequence.

## Docker

Build image:

```bash
docker build -t fire-redis:bench .
```

Run container:

```bash
docker run --rm -p 6379:6379 -e REDIS_BIND=0.0.0.0 -e REDIS_PORT=6379 fire-redis:bench
```

## Benchmarking

See `perf/README.md` for:

- Single-run benchmark (`perf/main.py`)
- Matrix benchmark across different scales (`perf/run_matrix.py`)
- Head-to-head image comparison (`perf/compare-images.sh`)

## Docs

- `docs/ROADMAP.md`
- `docs/ARCHITECTURE.md`
- `docs/DEMO.md`
- `docs/TRADEOFFS.md`
- `docs/OBSERVABILITY.md`
- `docs/COMPATIBILITY.md`
- `docs/PROGRESS.md`

## Notes

- Protocol target: RESP2
- This project is intended for learning, experimentation, and systems interview demos.
