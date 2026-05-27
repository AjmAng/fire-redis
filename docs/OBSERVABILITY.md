# Observability Notes

This toy project keeps observability lightweight and interview-friendly.

## Logging

Server logging is configured in `src/bin/server.rs` with tracing.

To run with explicit debug visibility:

```powershell
$env:RUST_LOG = "info,fire_redis=debug"
cargo run --bin redis-server
```

## What to Watch During Demo

- Startup binding and protocol banner
- Connection open/close events
- Expired key eviction events
- Persistence enablement and load/save messages

## Suggested Metrics (Next Iteration)

- Request count by command
- Success/error counters
- TTL eviction count
- AOF write count and recovery item count

Keep metrics minimal and tied to interview talking points, not dashboard complexity.

