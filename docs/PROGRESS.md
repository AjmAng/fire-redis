# 项目进展记录

> 记录时间：2026-06-07  
> 当前分支：`master`（与 `origin/main` 同步）  
> 当前阶段：**Phase 1 — 协议与命令语义基线（收尾中）**

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
| 指标定义 | 🟡 文档化 | `docs/OBSERVABILITY.md` 列出了建议指标集，但未全部落地 |
| 指标采集 | ❌ 未实现 | 无请求计数器、错误计数器、淘汰计数器等运行时指标 |

### 2.6 测试

| 类型 | 覆盖情况 | 状态 |
|---|---|---|
| 单元测试 | RESP / Command Parse / Store 基础行为 | ✅ 有，但集中在 parse 与底层 API |
| 集成测试 | `ping_and_strings` / `hash_and_list` / `set_and_zset` / `ttl_and_expiration` / `smoke_extended` | ✅ 正向路径覆盖较好 |
| 负向测试 | wrong arity / wrong type / 边界输入 | 🟡 薄弱，Commands 模块有部分 parse 层错误测试，但缺少端到端错误语义测试 |
| 持久化测试 | 启动加载 / 故障恢复 / 文件损坏 | ❌ 无 |

**测试运行结果**：当前 `cargo test` 全部通过。

### 2.7 性能与部署

| 项 | 状态 |
|---|---|
| 基准脚本 `perf/` | ✅ 可用（`main.py`、`run_matrix.py`、`compare-images.sh`） |
| 基线归档 | ❌ 尚未执行并归档 |
| Docker 支持 | ✅ `Dockerfile` 就绪 |

---

## 3. 近期执行清单（Roadmap 两周清单）

来源：`docs/ROADMAP.md`  
更新时间：2026-06-07  
**全部待完成：**

- [x] 新建 `docs/COMPATIBILITY.md`，完成第一版命令矩阵（supported / partial / todo）
- [ ] 为已支持命令补齐正向/负向测试（含 wrong arity / wrong type）
- [x] 补充持久化恢复回归用例（RDB 全量恢复、AOF 增量回放、RDB+AOF 组合加载、TTL 跨重启）
- [ ] 在 `docs/OBSERVABILITY.md` 固化指标口径与日志事件（区分"已有"和"待实现"）
- [ ] 用现有 `perf/` 脚本完成一轮基线记录并归档到 `perf/results/`

---

## 4. 当前阻塞/风险

1. **负向测试缺失**：类型错误、参数错误的返回语义未与 Redis 严格对齐，存在面试演示中被追问的风险。
2. **持久化已验证**：`tests/persistence.rs` 通过 4 个集成测试验证 RDB/AOF/组合/TTL 恢复，但尚未覆盖崩溃一致性、AOF 损坏恢复等边界。
3. **缺少兼容性对照表**：没有 `COMPATIBILITY.md` 时，外部使用者（包括面试官）无法快速判断哪些命令可用、行为差异在哪。

---

## 5. 下一步建议

### 路径 A：保 Phase 1 验收（推荐本周做）
1. 建立 `docs/COMPATIBILITY.md`（1-2 小时）
2. 挑选 5-8 条高频命令，补 wrong arity + wrong type 集成测试（2-3 小时）
3. 运行 `cargo test` 确保全绿，更新本文档标记完成项

### 路径 B：提前启动 Phase 2
1. 写 2 个持久化回归测试：
   - 场景 A：写入数据 → 触发 RDB 保存 → 模拟重启加载 → 断言数据与 TTL 一致
   - 场景 B：写入数据 → AOF 记录 → 模拟重启回放 → 断言数据一致
2. 在 `docs/TRADEOFFS.md` 补充持久化保证范围与已知限制

### 路径 C：可观测性先行
1. 在 `src/server.rs` 与 `src/commands/` 关键路径插入计数器（可用原子变量或 tracing metrics）
2. 更新 `docs/OBSERVABILITY.md`，把"建议指标"改为"已实现的指标"

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
