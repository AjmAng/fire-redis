# redis-rs Roadmap（Redis 复刻 + 可观测性）

## 项目目标

`redis-rs` 的主目标调整为：

1. 复刻 Redis 单机核心能力（协议、命令语义、数据结构、持久化）。
2. 建立最小但实用的可观测性能力（日志、指标、运行状态）。

当前结构基线：`src/`（核心实现）、`tests/`（测试）、`perf/`（性能验证）、`docs/`（文档）。

---

## Phase 1（已完成）：协议与命令语义基线

### 目标
对齐 RESP2 和高频核心命令行为，形成可验证的兼容性基线。

### 交付物
- ✅ `docs/COMPATIBILITY.md`：按命令分组标注 `supported / partial / todo`。
- ✅ 命令语义对齐：48 条命令已实现，覆盖参数校验、类型错误、空值语义、TTL 行为。
- ✅ 正向/负向测试覆盖所有已实现命令。

### 验收标准
- ✅ 所有 `supported` 命令都有至少 1 个正向测试。
- ✅ 每个命令族至少有 1 个错误语义测试。
- ✅ `docs/COMPATIBILITY.md` 与实际实现一致。

---

## Phase 2（已完成）：数据结构完整性与持久化一致性

### 目标
提升单机语义一致性，确保重启恢复可预测。

### 交付物
- ✅ `src/store/` 覆盖 5 种数据结构边界行为（空 key、类型冲突、TTL 交互）。
- ✅ 8 个持久化集成测试覆盖：RDB 全量恢复、AOF 回放与 rewrite、RDB+AOF 组合加载、TTL 跨重启、无持久化、DEL/FLUSHALL 后保存。
- ✅ `docs/TRADEOFFS.md` 补充持久化保证范围与已知限制。

### 验收标准
- ✅ 冷启动/热重启后关键数据集一致。
- ✅ 持久化失败场景有明确错误行为和文档说明。
- ✅ 回归测试可稳定复现恢复流程。

---

## Phase 3（已完成）：可观测性能力建设

### 目标
让系统运行状态可见、可解释、可定位。

### 交付物
- ✅ 日志（`tracing`）标准化：启动/关闭、连接生命周期、命令执行、TTL 淘汰、持久化事件。
- ✅ 运行时指标：`src/metrics.rs` 提供无锁原子计数器 + 延迟直方图，通过 `INFO` 命令和 HTTP `/metrics`/`/health` 暴露。
- ✅ OpenTelemetry tracing 集成：`src/observability.rs` 桥接 tracing span → OTel，可选通过 OTLP HTTP 导出。
- ✅ `docs/OBSERVABILITY.md` 明确采集方式与解释口径。

### 验收标准
- ✅ 通过日志/span 可追踪一次请求的关键路径。
- ✅ 关键计数指标可通过 `INFO` 命令和 HTTP 端点查询。
- ✅ 文档可指导定位常见问题。

---

## Phase 4：性能回归与持续对比

### 目标
利用已有 `perf/` 一键脚本持续跟踪性能趋势，防止回归。

### 交付物
- 固定一套基准场景（如 `set/get/mixed` + 结构类操作）。
- 约定回归门槛（如吞吐下降或延迟上升阈值）。
- 将结果归档到 `perf/results/` 并在文档记录变化原因。

### 验收标准
- 能稳定复现基准数据。
- 每次较大变更后有可对比结果。
- 性能回归能被及时发现并解释。

---

## Scope Boundaries（阶段内不做）

1. 暂不做集群、主从复制、哨兵、分片。
2. 暂不做 Lua、事务（`MULTI/EXEC`）、ACL、发布订阅。
3. 暂不做平台化运维能力和复杂配置中心。
4. 暂不追求“全量 Redis 完整兼容”，优先高频命令与核心语义。

---

## 与仓库结构的落地映射

- `src/commands/`：命令兼容性和错误语义对齐。
- `src/store/`：数据结构与 TTL 边界一致性。
- `src/persistence/`：AOF/RDB 恢复和故障语义。
- `src/server.rs`：连接处理、生命周期日志、观测事件。
- `tests/`：兼容性、持久化、回归测试。
- `perf/`：持续性能基线与回归对比。
- `docs/`：`ROADMAP`、`COMPATIBILITY`、`OBSERVABILITY`、`TRADEOFFS`。

---

## 近期两周执行清单

- [x] 新建 `docs/COMPATIBILITY.md` 并完成第一版命令矩阵。
- [x] 为已支持命令补齐正向/负向测试（含 wrong arity/wrong type）。
- [x] 补充持久化恢复回归用例（8 个场景）。
- [x] 在 `docs/OBSERVABILITY.md` 固化指标口径与日志事件。
- [ ] 用现有 `perf/` 脚本完成一轮基线记录并归档（**待启动 — Phase 4**）。

---

## 决策关注点

1. **Phase 4 性能基线**：是否立即启动 `perf/` 基线记录并归档？
2. **OTel 指标导出**：是否将 `src/metrics.rs` 桥接到 OTel instruments 以实现统一指标出口？
3. **负向测试补齐**：是否为所有 `supported` 命令添加 wrong arity + wrong type 端到端测试？
