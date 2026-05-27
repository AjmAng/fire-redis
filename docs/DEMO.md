# Demo Guide (Interview)

This document gives a stable 5-8 minute demo flow for `redis-rs`.

## Fast Path (Manual)

Use two terminals:

### Terminal A: start server

```bash
cargo run --bin redis-server
```

### Terminal B: run demo commands

```bash
cargo run --bin redis-cli -- PING
cargo run --bin redis-cli -- SET k v
cargo run --bin redis-cli -- GET k
cargo run --bin redis-cli -- SET temp 1 EX 1
# wait about 2 seconds for the key to expire
cargo run --bin redis-cli -- GET temp
cargo run --bin redis-cli -- HSET h f v
cargo run --bin redis-cli -- HGET h f
```

If you prefer interactive mode, use:

```bash
cargo run --bin redis-cli
```

## Expected Talking Points

- RESP request path: `resp` -> `commands` -> `store` -> optional `persistence`
- TTL behavior is visible and easy to verify
- At least one structured type (`HASH`, `LIST`, or `ZSET`) is demonstrated
- Regression tests and perf scripts exist (`tests/`, `perf/`)

