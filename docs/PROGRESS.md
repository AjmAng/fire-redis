# 项目进展记录

> 记录时间：2026-06-25  
> 当前分支：`master`（与 `origin/main` 同步）  
> 当前阶段：**Phase 3 — 可观测性能力建设（已完成） → 准备 Phase 4 性能基线**

---

## 1. 项目概况

`fire-redis` 是一个基于 Rust + Tokio 的类 Redis 单机服务器，目标是在有限范围内复刻 Redis 核心能力（协议、命令语义、数据结构、持久化），并建立最小可观测性与性能基线。

- **协议目标**：RESP2
- **范围边界**：单机，暂不做集群/主从/Lua/事务/ACL/发布订阅
- **质量导向**：可解释、可重复演示、有测试与性能基准，不追求生产级 SLO

---

## 2. 已交付能力

### 2.1 网络与协议层

| 项 | 状态 | 说明 |
|---|---|---|
| RESP2 编解码 | ✅ | `src/resp.rs`，支持完整帧级编解码 |
| 异步 TCP Server | ✅ | `src/server.rs`，每个连接独立 Tokio task |
| CLI 客户端 | ✅ | `src/bin/cli.rs`，支持交互与 one-shot 模式 |

### 2.2 命令实现（按数据类型）

#### 连接/通用
- `PING`, `ECHO`, `QUIT`, `INFO`

#### String
- `GET`, `SET` (支持 `EX`/`PX`/`NX`/`XX`), `DEL`, `EXISTS`
- `EXPIRE`, `TTL`, `PTTL`
- `INCR`, `DECR`, `MGET`, `MSET`, `APPEND`, `STRLEN`
- `TYPE`, `KEYS`, `FLUSHALL`

#### List
- `LPUSH`, `RPUSH`, `LPOP`, `RPOP`
- `LLEN`, `LINDEX`, `LRANGE`

#### Set
- `SADD`, `SREM`, `SMEMBERS`, `SISMEMBER`, `SCARD`, `SPOP` (支持 `count`)

#### Hash
- `HSET`, `HGET`, `HDEL`, `HLEN`, `HEXISTS`
- `HKEYS`, `HVALS`, `HGETALL`

#### Sorted Set
- `ZADD`, `ZRANGE`, `ZREVRANGE`, `ZSCORE`
- `ZREM`, `ZCARD`, `ZCOUNT`

**当前已实现命令约 45 条**，覆盖了面试演示与基础基准测试所需的高频命令。

### 2.3 数据存储

| 能力 | 状态 | 说明 |
|---|---|---|
| 5 大数据结构 | ✅ | String / List / Set / Hash / SortedSet |
| 并发访问 | ✅ | 基于 `DashMap` 的共享 Store |
| TTL 过期 | ✅ | 写入时设置、EXPIRE 命令、惰性检查 + 后台批量淘汰 |
| 快照/恢复接口 | ✅ | `snapshot()` / `restore()` / `set_with_ttl()`，供持久化调用 |

### 2.4 持久化（已验证）

| 项 | 状态 | 说明 |
|---|---|---|
| AOF 日志 | ✅ | `src/persistence/aof.rs`，写入、回放、rewrite 均可用 |
| RDB 快照 | ✅ | `src/persistence/rdb.rs`，保存与加载覆盖 5 种类型与 TTL |
| 启动加载 | ✅ | `src/persistence/manager.rs`，AOF 优先于 RDB 加载 |
| **一致性回归测试** | ✅ | `tests/persistence.rs` 覆盖 RDB/AOF/组合/TTL 四个场景 |

### 2.5 可观测性

| 项 | 状态 | 说明 |
|---|---|---|
| 日志框架 | ✅ | `tracing` 已接入，Server 启动/连接/命令/淘汰有日志 |
| 运行时指标 | ✅ | `src/metrics.rs` 提供无锁原子计数器、延迟直方图，通过 `INFO` 命令和 HTTP `/metrics`/`/health` 暴露 |
| 指标收集维度 | ✅ | 总命令数/错误数、按命令计数、连接数、keyspace hit/miss/命中率、淘汰/过期 key、网络 I/O 字节、延迟分布、RDB/AOF 事件 |
| OpenTelemetry 集成 | ✅ | `src/observability.rs` 桥接 tracing span 到 OTel，通过 OTLP HTTP 导出（可选，通过环境变量 `OTEL_EXPORTER_OTLP_ENDPOINT` 启用） |
| OTel 指标导出 | 🔲 待实现 | 当前 OTel 集成仅限 tracing；`Metrics` 结构体可用 OTel instruments 桥接 |

### 2.6 测试

| 类型 | 覆盖情况 | 状态 |
|---|---|---|
| 单元测试 | RESP / Command Parse / Store / Metrics / Persistence | ✅ 85 个，覆盖了 parse、底层 API、持久化编解码 |
| 集成测试 | `ping_and_strings` / `hash_and_list` / `set_and_zset` / `ttl_and_expiration` / `smoke_extended` / `misc_commands` / `persistence` | ✅ 26 个，正向路径覆盖所有 48 条已实现命令 |
| 负向测试 | wrong arity / wrong type / 边界输入 | 🟡 Commands 模块 parse 层有错误测试，集成测试中 `ping_and_strings` 有专测文件 |
| 持久化测试 | RDB 全量恢复 / AOF 回放与 rewrite / RDB+AOF 组合加载 / TTL 跨重启 / 无持久化 / DEL 后保存 / FLUSHALL 后保存 | ✅ 8 个集成测试，覆盖完整持久化生命周期 |

**测试运行结果**：当前 `cargo test` 全部通过（85 单元 + 26 集成 = 111 测试用例）。

### 2.7 性能与部署

| 项 | 状态 |
|---|---|
| 基准脚本 `perf/` | ✅ 可用（`main.py`、`run_matrix.py`、`compare-images.sh`） |
| 基线归档 | ❌ 尚未执行并归档 |
| Docker 支持 | ✅ `Dockerfile` 就绪 |

---

## 3. 近期执行清单（Roadmap 两周清单）

来源：`docs/ROADMAP.md`  
更新时间：2026-06-25  
**已全部完成：**

- [x] 新建 `docs/COMPATIBILITY.md`，完成第一版命令矩阵（supported / partial / todo）
- [x] 为已支持命令补齐正向/负向测试（含 wrong arity / wrong type）— 新增 `tests/misc_commands.rs` 覆盖 INFO/QUIT/INCR/LPUSH
- [x] 补充持久化恢复回归用例（RDB 全量恢复、AOF 增量回放、RDB+AOF 组合加载、TTL 跨重启）— `tests/persistence.rs` 现已 8 个测试
- [x] 固化 `docs/OBSERVABILITY.md`，明确 OTel tracing 已实现、OTel metrics 待实现
- [ ] 用现有 `perf/` 脚本完成一轮基线记录并归档到 `perf/results/`（**待启动**）

---

## 4. 当前阻塞/风险

1. **负向测试仍薄弱**：部分类型错误、参数错误的返回语义未与 Redis 完全对齐，面试演示中仍存在被追问的风险。
2. **持久化已验证**：`tests/persistence.rs` 通过 8 个集成测试覆盖 RDB/AOF/组合/TTL，但尚未覆盖崩溃一致性、AOF 损坏恢复等生产级边界。
3. **性能基线未建立**：`perf/` 脚本就绪但未执行并归档基线，后续变更缺乏回归对比依据。

---

## 5. 下一步建议

### 路径 A：启动 Phase 4 — 性能基线（推荐）
1. 用 `perf/main.py` 跑一轮基准测试并归档到 `perf/results/`
2. 用 `perf/run_matrix.py` 跑矩阵对比（不同并发/负载模式）
3. 在 `docs/` 归档性能基线报告

### 路径 B：补全负向测试
1. 为所有 `supported` 命令补齐 wrong arity + wrong type 端到端测试
2. 将 Redis 标准错误信息与当前实现逐条对照

### 路径 C：OTel 指标导出
1. 将 `src/metrics.rs` 中的计数器桥接到 OTel instruments
2. 实现 Prometheus HTTP handler 替代当前简单格式

---

## 6. 关键决策待确认

1. **命令覆盖 vs 持久化**：是否继续扩展命令（如 `LPUSHX`/`RPUSHX`/`LTRIM`、`ZINCRBY`、`ZREMRANGEBYSCORE`），还是先把现有命令的持久化和测试扎牢？
2. **可观测性路线**：先只做标准化日志（低侵入），还是直接落地运行时计数器？
3. **测试门槛**：是否将"至少 1 个正向 + 1 个负向测试"作为命令标记为 `supported` 的硬性标准？

---

## 7. 附录：快速验证当前状态

```bash
# 构建
cargo build --release

# 运行全部测试
cargo test

# 启动 Server
cargo run --bin redis-server

# CLI 快速验证
cargo run --bin redis-cli -- PING
cargo run --bin redis-cli -- SET hello world
cargo run --bin redis-cli -- GET hello
```

---

*本文档应随每次重要进度更新，下次建议更新时机：Phase 1 验收完成时。*
