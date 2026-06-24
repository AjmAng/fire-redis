# Design Tradeoffs (Interview Notes)

## Why RESP2 only

- Keeps codec and debugging surface small
- Covers most demo and benchmark paths in this toy scope
- Avoids RESP3 edge cases that do not change core interview value

## Why not full Redis compatibility

- Full compatibility is expensive and hard to verify in limited time
- This project optimizes for explainable architecture and stable demo quality
- Focus is on representative command paths, tests, and measured behavior

## Why no cluster/replication/transactions now

- These features multiply state and failure complexity
- They would dilute time from fundamentals: protocol, command execution, store, persistence
- For interview value, one well-implemented single-node system is stronger than many shallow features

## What we prioritize instead

- Demo repeatability (`docs/DEMO.md`, replay script)
- Observability and recovery explanation (`docs/OBSERVABILITY.md`)
- Small but meaningful test + perf baseline

## Persistence tradeoffs

- **RDB + AOF startup semantics**: When both files exist, we load AOF only (Redis-compatible). RDB acts as a fallback when AOF is disabled or missing.
- **AOF rewrite TTL**: Rewrite emits an `EXPIRE` command after each key to preserve TTL. We use seconds granularity because `PEXPIRE` is not implemented; sub-second TTLs may be rounded up to 1 second.
- **AOF ordering**: Commands are logged before execution for simplicity. This means a failed conditional write (e.g., `SET NX` on an existing key) is still appended and will replay, which differs from Redis. Documented as a known toy-scope limitation.
- **Crash consistency**: We do not guarantee fsync ordering between RDB and AOF or partial-write recovery. The implementation favors explainability over production-grade durability.

