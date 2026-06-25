# Observability Notes

This project keeps observability lightweight and interview-friendly while providing production-oriented hooks.

## Logging

Server logging is configured via `tracing` + `tracing-subscriber`.  The entrypoint
in `src/bin/server.rs` calls `observability::init()` which sets up:

- Structured stdout logging (fmt layer)
- Optional OpenTelemetry span export via OTLP/HTTP

### Environment variables

| Variable | Default | Description |
|---|---|---|
| `RUST_LOG` | `info,fire_redis=debug` | Log filter (standard `tracing`/env-filter syntax) |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4318` | OTel collector endpoint for span export |

To start the server with OTel tracing enabled (pointing at a local collector):

```powershell
$env:RUST_LOG = "info,fire_redis=debug"
$env:OTEL_EXPORTER_OTLP_ENDPOINT = "http://localhost:4318"
cargo run --bin redis-server
```

If no OTel collector is reachable, the server logs to stdout only (no crash, no
hang) — a grace note is printed on stderr at startup:

```
[otel] Failed to create OTLP span exporter: ...
[otel] Span export disabled. Set OTEL_EXPORTER_OTLP_ENDPOINT to enable.
```

## Runtime Metrics

`src/metrics.rs` provides lock‑free atomic counters and a latency histogram.
Metrics are exposed through multiple channels:

| Channel | Port / Command | Format |
|---|---|---|
| Redis `INFO` command | 6379 (RESP2) | `key:value` pairs (sections: Server, Stats, Latency, Commandstats, Keyspace) |
| HTTP `/metrics` | 6380 | Prometheus text format |
| HTTP `/health` | 6380 | JSON |
| OTel spans (when enabled) | configured via `OTEL_EXPORTER_OTLP_ENDPOINT` | OTLP HTTP/protobuf |

### Metrics collected

- Total / error commands
- Per-command counters (e.g. `cmd_get:42`)
- Active / total connections
- Keyspace hits, misses, hit‑rate
- Evicted & expired keys
- Network I/O bytes (inbound & outbound)
- Command‑latency histogram (buckets: 10, 50, 100, 500, 1k, 5k, 10k, 50k µs)
- RDB saves, AOF writes & rewrites
- Uptime & total keys

## OpenTelemetry Integration

`src/observability.rs` wires the OTel Rust SDK into the `tracing` ecosystem.

### Architecture

```
Application (tracing spans)
       │
       ├──► tracing-subscriber (stdout)
       │
       └──► tracing-opentelemetry ──► OTLP HTTP exporter ──► OTel Collector ──► Jaeger / Tempo / etc.
```

### How spans flow

1. `tracing::info_span!("command", cmd, addr)` creates a span for each command
   (see `handle_connection` in `src/server.rs`).
2. `tracing-opentelemetry` bridges every `tracing` span into an OTel span.
3. The OTLP HTTP exporter sends completed spans to the configured collector.
4. `OtelGuard` (returned by `observability::init()`) flushes remaining spans on
   drop — keep it alive for the `main` function's lifetime.

### Adding custom attributes

Standard `tracing` attributes are automatically forwarded as OTel span
attributes:

```rust
tracing::info_span!("command", cmd = %cmd_name, addr = %addr);
```

## What to Watch During Demo

- Startup binding and protocol banner
- Connection open/close events
- Expired key eviction events
- Persistence enablement and load/save messages
- `GET /metrics` returning Prometheus counters with latency histograms
- `GET /health` returning JSON with uptime and active connections
- OTel spans appearing in Jaeger / Grafana Tempo (if collector configured)

## Next Steps (Not Implemented)

- OTel **metrics** export (currently the OTel integration is tracing‑only;
  the built-in `Metrics` struct can be bridged to OTel instruments in a
  follow‑up)
- Structured log shipping (Loki, Datadog, etc.)
- Grafana dashboard configuration (`grafana.json`)
- Alerting rules (webhook, Slack, etc.)

