# 命令兼容性矩阵（COMPATIBILITY.md）

> 最后更新：2026-06-25  
> 对应代码分支：`master`

本文档按命令分组列出 Redis 标准命令在 fire-redis 中的实现状态。

---

## 状态说明

| 标记 | 含义 |
|------|------|
| ✅ `supported` | 已实现，行为与 Redis 对齐 |
| 🟡 `partial`   | 已实现，但有功能限制（见备注列） |
| 🔲 `todo`      | 计划支持，当前未实现 |
| ❌ `n/a`       | 超出项目范围，不会实现 |

---

## 连接 / Connection

| 命令 | 状态 | 备注 |
|------|------|------|
| `PING` | ✅ supported | |
| `ECHO` | ✅ supported | |
| `QUIT` | ✅ supported | |
| `INFO` | ✅ supported | 返回 Server / Stats / Latency / Commandstats / Keyspace / Replication 共 6 个段 |
| `RESET` | 🔲 todo | |
| `AUTH` | ❌ n/a | 无 ACL / 鉴权机制 |
| `SELECT` | ❌ n/a | 不支持多 database |
| `HELLO` | ❌ n/a | 属于 RESP3 协议协商，超出范围 |
| `CLIENT *` | ❌ n/a | 客户端管理命令，超出范围 |

---

## 通用键命令 / Generic

| 命令 | 状态 | 备注 |
|------|------|------|
| `DEL` | ✅ supported | 支持批量删除多个 key |
| `EXISTS` | ✅ supported | 支持传入多个 key，返回存在数量 |
| `TYPE` | ✅ supported | |
| `EXPIRE` | ✅ supported | 秒级，内部转毫秒存储 |
| `TTL` | ✅ supported | |
| `PTTL` | ✅ supported | |
| `KEYS` | 🟡 partial | 仅支持 `*`（全量），不支持 `?`、`[ae]`、`user:*` 等 glob 模式 |
| `FLUSHALL` | ✅ supported | |
| `PERSIST` | 🔲 todo | 移除 key 的过期时间 |
| `PEXPIRE` | 🔲 todo | 毫秒级 EXPIRE |
| `EXPIREAT` | 🔲 todo | Unix 时间戳到期 |
| `PEXPIREAT` | 🔲 todo | 毫秒时间戳到期 |
| `EXPIRETIME` | 🔲 todo | 返回到期时间戳（Redis 7.0+） |
| `PEXPIRETIME` | 🔲 todo | 毫秒版本（Redis 7.0+） |
| `RENAME` | 🔲 todo | |
| `RENAMENX` | 🔲 todo | |
| `RANDOMKEY` | 🔲 todo | |
| `SCAN` | 🔲 todo | 渐进式迭代 |
| `UNLINK` | 🔲 todo | 异步 DEL |
| `COPY` | 🔲 todo | |
| `SORT` | 🔲 todo | |
| `TOUCH` | 🔲 todo | |
| `FLUSHDB` | 🔲 todo | 仅清当前 DB（当前只有 DB 0） |
| `OBJECT` | ❌ n/a | 内部对象调试，超出范围 |
| `DUMP` / `RESTORE` | ❌ n/a | 序列化格式不对齐 |
| `MOVE` | ❌ n/a | 不支持多 database |
| `WAIT` | ❌ n/a | 无复制机制 |

---

## 字符串 / String

| 命令 | 状态 | 备注 |
|------|------|------|
| `GET` | ✅ supported | |
| `SET` | 🟡 partial | 支持 `EX`/`PX`/`NX`/`XX`；不支持 `KEEPTTL`、`EXAT`、`PXAT`、`GET`（返回旧值）选项 |
| `MGET` | ✅ supported | |
| `MSET` | ✅ supported | |
| `APPEND` | ✅ supported | |
| `STRLEN` | ✅ supported | |
| `INCR` | ✅ supported | |
| `DECR` | ✅ supported | |
| `INCRBY` | 🔲 todo | |
| `DECRBY` | 🔲 todo | |
| `INCRBYFLOAT` | 🔲 todo | |
| `GETSET` | 🔲 todo | Redis 6.2 已弃用，可用 `SET ... GET` 替代 |
| `GETDEL` | 🔲 todo | |
| `GETEX` | 🔲 todo | |
| `SETNX` | 🔲 todo | 可用 `SET key val NX` 替代 |
| `SETEX` | 🔲 todo | 可用 `SET key val EX secs` 替代 |
| `PSETEX` | 🔲 todo | 可用 `SET key val PX ms` 替代 |
| `MSETNX` | 🔲 todo | |
| `GETRANGE` | 🔲 todo | |
| `SETRANGE` | 🔲 todo | |
| `SUBSTR` | ❌ n/a | `GETRANGE` 的旧别名，不单独实现 |

---

## 列表 / List

| 命令 | 状态 | 备注 |
|------|------|------|
| `LPUSH` | ✅ supported | 支持批量 push |
| `RPUSH` | ✅ supported | 支持批量 push |
| `LPOP` | ✅ supported | |
| `RPOP` | ✅ supported | |
| `LLEN` | ✅ supported | |
| `LINDEX` | ✅ supported | |
| `LRANGE` | ✅ supported | 支持负数索引 |
| `LSET` | 🔲 todo | |
| `LINSERT` | 🔲 todo | |
| `LREM` | 🔲 todo | |
| `LTRIM` | 🔲 todo | |
| `LPUSHX` | 🔲 todo | 仅当 key 存在时 push |
| `RPUSHX` | 🔲 todo | 仅当 key 存在时 push |
| `LPOS` | 🔲 todo | |
| `LMOVE` | 🔲 todo | |
| `LMPOP` | 🔲 todo | |
| `RPOPLPUSH` | 🔲 todo | Redis 6.2 已弃用，`LMOVE` 替代 |
| `BLPOP` | ❌ n/a | 阻塞命令，超出范围 |
| `BRPOP` | ❌ n/a | 阻塞命令，超出范围 |
| `BLMOVE` | ❌ n/a | 阻塞命令，超出范围 |
| `BLMPOP` | ❌ n/a | 阻塞命令，超出范围 |

---

## 集合 / Set

| 命令 | 状态 | 备注 |
|------|------|------|
| `SADD` | ✅ supported | 支持批量添加 |
| `SREM` | ✅ supported | 支持批量删除 |
| `SMEMBERS` | ✅ supported | |
| `SISMEMBER` | ✅ supported | |
| `SCARD` | ✅ supported | |
| `SPOP` | ✅ supported | 支持可选 count 参数 |
| `SRANDMEMBER` | 🔲 todo | |
| `SMISMEMBER` | 🔲 todo | 批量判断（Redis 6.2+） |
| `SUNION` | 🔲 todo | |
| `SUNIONSTORE` | 🔲 todo | |
| `SINTER` | 🔲 todo | |
| `SINTERSTORE` | 🔲 todo | |
| `SINTERCARD` | 🔲 todo | Redis 7.0+ |
| `SDIFF` | 🔲 todo | |
| `SDIFFSTORE` | 🔲 todo | |
| `SSCAN` | 🔲 todo | |

---

## 哈希 / Hash

| 命令 | 状态 | 备注 |
|------|------|------|
| `HSET` | ✅ supported | 支持批量设置多个 field-value 对 |
| `HGET` | ✅ supported | |
| `HDEL` | ✅ supported | 支持批量删除 field |
| `HLEN` | ✅ supported | |
| `HEXISTS` | ✅ supported | |
| `HKEYS` | ✅ supported | |
| `HVALS` | ✅ supported | |
| `HGETALL` | ✅ supported | |
| `HMGET` | 🔲 todo | 批量获取多个 field |
| `HINCRBY` | 🔲 todo | |
| `HINCRBYFLOAT` | 🔲 todo | |
| `HSETNX` | 🔲 todo | |
| `HRANDFIELD` | 🔲 todo | Redis 6.2+ |
| `HSCAN` | 🔲 todo | |
| `HMSET` | 🟡 partial | Redis 4.0 已弃用；`HSET` 已支持多 field-value，功能完全等价，但命令名 `HMSET` 未注册 |

---

## 有序集合 / Sorted Set

| 命令 | 状态 | 备注 |
|------|------|------|
| `ZADD` | ✅ supported | |
| `ZRANGE` | 🟡 partial | 仅支持按 index 范围；不支持 `BYSCORE`/`BYLEX`/`REV`/`LIMIT` 选项（Redis 6.2+） |
| `ZREVRANGE` | ✅ supported | |
| `ZSCORE` | ✅ supported | |
| `ZREM` | ✅ supported | 支持批量删除 |
| `ZCARD` | ✅ supported | |
| `ZCOUNT` | ✅ supported | |
| `ZRANK` | 🔲 todo | |
| `ZREVRANK` | 🔲 todo | |
| `ZINCRBY` | 🔲 todo | |
| `ZRANGEBYSCORE` | 🔲 todo | Redis 6.2 中被新版 `ZRANGE BYSCORE` 取代 |
| `ZREVRANGEBYSCORE` | 🔲 todo | |
| `ZRANGEBYLEX` | 🔲 todo | |
| `ZREVRANGEBYLEX` | 🔲 todo | |
| `ZRANGESTORE` | 🔲 todo | Redis 6.2+ |
| `ZREMRANGEBYRANK` | 🔲 todo | |
| `ZREMRANGEBYSCORE` | 🔲 todo | |
| `ZREMRANGEBYLEX` | 🔲 todo | |
| `ZLEXCOUNT` | 🔲 todo | |
| `ZPOPMIN` | 🔲 todo | |
| `ZPOPMAX` | 🔲 todo | |
| `ZRANDMEMBER` | 🔲 todo | Redis 6.2+ |
| `ZMSCORE` | 🔲 todo | Redis 6.2+ |
| `ZDIFF` | 🔲 todo | |
| `ZDIFFSTORE` | 🔲 todo | |
| `ZUNION` | 🔲 todo | |
| `ZUNIONSTORE` | 🔲 todo | |
| `ZINTER` | 🔲 todo | |
| `ZINTERSTORE` | 🔲 todo | |
| `ZINTERCARD` | 🔲 todo | Redis 7.0+ |
| `ZSCAN` | 🔲 todo | |
| `BZPOPMIN` | ❌ n/a | 阻塞命令，超出范围 |
| `BZPOPMAX` | ❌ n/a | 阻塞命令，超出范围 |
| `BZMPOP` | ❌ n/a | 阻塞命令，超出范围 |

---

## 服务器 / Server

| 命令 | 状态 | 备注 |
|------|------|------|
| `DBSIZE` | 🔲 todo | |
| `TIME` | 🔲 todo | |
| `BGSAVE` | 🔲 todo | RDB 框架已就绪（`src/persistence/rdb.rs`），命令接口待接入 |
| `BGREWRITEAOF` | 🔲 todo | AOF 框架已就绪（`src/persistence/aof.rs`），命令接口待接入 |
| `SAVE` | 🔲 todo | |
| `LASTSAVE` | ❌ n/a | 超出当前范围 |
| `DEBUG` | ❌ n/a | 超出当前范围 |
| `CONFIG` | ❌ n/a | 超出当前范围 |
| `MONITOR` | ❌ n/a | 超出当前范围 |
| `SLOWLOG` | ❌ n/a | 超出当前范围 |
| `MEMORY` | ❌ n/a | 超出当前范围 |
| `COMMAND` | ❌ n/a | 超出当前范围 |
| `LATENCY` | ❌ n/a | 超出当前范围 |
| `MODULE` | ❌ n/a | 超出当前范围 |
| `LOLWUT` | ❌ n/a | |

---

## 整类不实现（Out-of-Scope Categories）

以下命令族在 ROADMAP 中明确标注为当前范围外，不计入 `todo`：

| 类别 | 典型命令 | 原因 |
|------|---------|------|
| **集群** | `CLUSTER *` | 单机项目 |
| **复制** | `REPLICAOF`, `SLAVEOF`, `WAIT` | 无主从复制 |
| **发布订阅** | `SUBSCRIBE`, `PUBLISH`, `PSUBSCRIBE` 等 | 超出范围 |
| **Lua 脚本** | `EVAL`, `EVALSHA`, `SCRIPT *` | 暂不做 Lua |
| **事务** | `MULTI`, `EXEC`, `DISCARD`, `WATCH` | 暂不做事务 |
| **ACL** | `ACL *` | 无访问控制 |
| **地理位置** | `GEOADD`, `GEODIST`, `GEORADIUS` 等 | 超出范围 |
| **Stream** | `XADD`, `XREAD`, `XGROUP` 等 | 超出范围 |
| **HyperLogLog** | `PFADD`, `PFCOUNT`, `PFMERGE` | 超出范围 |
| **位图** | `BITCOUNT`, `BITOP`, `BITPOS`, `GETBIT`, `SETBIT` | 超出范围 |

---

## 实现汇总

| 状态 | 数量 |
|------|------|
| ✅ `supported` | **46** |
| 🟡 `partial`   | **4**（`KEYS`、`SET`、`ZRANGE`、`HMSET`）|
| 🔲 `todo`      | ~55（通用键 + 各数据类型扩展命令 + Server 命令）|
| ❌ `n/a`       | 整类不实现（见上表）|

---

## Partial 实现说明

### `KEYS` — 仅支持 `*` 通配符
当前实现硬编码判断 `pattern == "*"`，非 `*` 的 pattern 会返回错误。  
计划引入 glob 匹配（如 `glob` crate）以支持 `user:*`、`h?llo`、`h[ae]llo` 等模式。

### `SET` — 缺少部分选项
支持：`EX`、`PX`、`NX`、`XX`  
不支持：`KEEPTTL`（保留现有 TTL）、`EXAT`/`PXAT`（绝对 Unix 时间戳到期）、`GET`（返回旧值）

### `ZRANGE` — 仅支持 index 范围
仅支持 `ZRANGE key start stop`（按排名区间）。  
Redis 6.2 新增的 `BYSCORE`、`BYLEX`、`REV`、`LIMIT` 选项未实现。  
`ZRANGEBYSCORE`、`ZRANGEBYLEX` 作为独立命令也未实现。

### `HMSET` — 已弃用
Redis 4.0 起 `HMSET` 被弃用，`HSET` 已支持传入多个 field-value 对，功能完全等价。  
当前 fire-redis 未注册 `HMSET` 命令名，但 `HSET` 行为已覆盖其所有用途。

---

*本文档应与 `src/commands/mod.rs` 中的命令注册保持同步更新。*

