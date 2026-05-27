# Architecture (Toy Scope)

## Request Path

1. `src/resp.rs`
   - Decodes RESP2 frames from TCP stream
   - Encodes command results back to RESP values
2. `src/commands/`
   - Parses command argv into typed command enum
   - Dispatches execution and returns RESP-friendly values
3. `src/store/`
   - In-memory data structures (string/hash/list/set/zset)
   - TTL checks and periodic expired-key eviction
4. `src/persistence/` (optional)
   - AOF write logging and RDB snapshot support
   - Startup load path for recovery

## Runtime Model

- Async TCP server (`tokio`) accepts connections concurrently
- Each connection is handled in an async task
- Shared store is accessed through concurrency-safe primitives in `Store`
- Background tasks handle expiration and persistence duties

## Intentional Toy Boundaries

- Protocol target is RESP2 only
- Compatibility goal is high-frequency interview commands, not full Redis parity
- Reliability focuses on explainability and repeatability, not production SLOs

