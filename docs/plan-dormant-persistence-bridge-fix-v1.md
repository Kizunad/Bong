# Bong · plan-dormant-persistence-bridge-fix-v1

修复 **dormant NPC 快照 Redis 持久化在量级 ≥ ~250 时把整条出站 IPC 搞瘫** 的既有 bug——`NpcDormantHash` 走 inline 出站，单次 `HashReplace` 超时后 ① 永久 pin `pending_command` 饿死所有出站消息（world_state / combat / chat 全发不出去）② `tokio::timeout` 取消 future 时泄漏 `bong:npc/dormant:tmp:*` 临时 key，形成 Redis 膨胀死亡螺旋。本 plan 不改散布逻辑、不改 dormant 数据模型，只修出站桥的健壮性 + 写入效率，让默认 `BONG_DORMANT_ROGUE_SEED_COUNT=1000` 能冷启动端到端持久化。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 止血：失败的 dormant 写不再饿死 IPC + 不再泄漏 temp key | ⬜ |
| P1 | 减负：dormant 只在变化时写，去掉每 10s 全量重 serde + 整表替换 | ⬜ |
| P2 | 根治写慢：profile inline HashReplace ~200× 慢源 + 分块/增量写 | ⬜ |
| P3 | 验收：1000 dormant 空 Redis 冷启动 → 持久化 → 重启恢复 端到端 | ⬜ |

> **dev-only 备注**：本 bug 仅在 dormant store 从**空** Redis 全量 seed（`seed_initial_dormant_population_on_startup` 走非 `is_empty` 分支）时触发。日常运行因 Redis 里一直有存量（`is_empty()==false` → 跳过 seed），从未发生过全量 1000 冷写，所以这是潜伏 bug。`scripts/start.sh` 默认就是 1000，任何人 `redis-cli flushall` 后冷启动都会撞。

> **与散布改动的关系**：本 bug 与 `dormant_seed_scatter_position`（R2 散布）改动**完全无关**，是在那次改动后清空 Redis 强制重 seed 才暴露的。散布改动本身已验证正确（3 单测 + 落库坐标按 zone 均匀铺开）。

---

## 接入面 Checklist

纯 server 持久化 / IPC infra plan ——**不涉及** worldview 锚点、qi_physics、视听规格（按 `docs/CLAUDE.md` §70「纯 server 逻辑 plan 无此要求」豁免）。

- **进料**：`NpcDormantStore`（`server/src/npc/dormant/mod.rs`）的快照集合，经 `publish_world_state_to_redis`（`server/src/network/mod.rs:841`）每 `WORLD_STATE_PUBLISH_INTERVAL_TICKS=200` tick 拉取一次。
- **出料**：`RedisOutbound::NpcDormantHash(Vec<(String,String)>)` → 出站桥 `HashReplace` → Redis HASH `bong:npc/dormant`（`NPC_DORMANT_REDIS_KEY`）。回读路径：server 启动时 `load_dormant_snapshots_from_redis`（`dormant/mod.rs:375`）`HGETALL` 恢复。
- **共享类型 / event**：复用现有 `RedisOutbound` / `RedisIoCommand` enum、`NpcDormantStore`、`NPC_DORMANT_REDIS_KEY`、`to_redis_hash_payloads()`。**不新建** event 或 schema。
- **跨仓库契约**：
  - **server**：`bong:npc/dormant`（HASH 持久化，自产自消）；`RedisOutbound::NpcDormantHash` / `RedisIoCommand::HashReplace`。
  - **agent**：天道通过 `bong:world_state`（pub/sub）间接受影响——出站桥被 dormant 写 pin 住时，`WorldStateV1` 也发不出去（虽然 world_state 走后台连接，但 drain 循环在 `pending_command` 重试失败处提前 return，根本到不了 world_state 的 prepare），天道收不到世界快照 → narration 断流。
  - **client**：**不受影响**——客户端走 Minecraft 协议直连 Valence（:25565），不读 Redis。
- **worldview 锚点**：无（基础设施）。
- **qi_physics 锚点**：无（不涉及真元/灵气计算）。

---

## 一、背景与诊断证据（2026-05-30 实地复现）

### 1.1 复现步骤

```bash
# 1. 清空 Redis（去掉 dormant 存量，强制全量冷 seed）
redis-cli flushall
# 2. 默认 1000 dormant 启动
bash scripts/start.sh           # 即 BONG_DORMANT_ROGUE_SEED_COUNT=1000
# 3. 观察 server pane（tmux attach -t bong）
#    ~10s 后开始无限刷：
#    [bong][redis] reconnect attempt=1 ... reason=outbound_retry_failed:
#      timed out replacing hash bong:npc/dormant after 3s
# 4. 验证 IPC 瘫痪
redis-cli hlen bong:npc/dormant         # → 0（写从未成功）
redis-cli --scan --pattern 'bong:npc/dormant:tmp:*' | wc -l   # → 持续增长（泄漏）
redis-cli dbsize                        # → 几百，全是泄漏 tmp key
redis-cli info memory | grep used_memory_human   # → 实测涨到 467MB
```

### 1.2 实测数据（已确认）

| 测量 | 结果 | 结论 |
|------|------|------|
| 单条 dormant 快照 JSON | **5478 字节**（`redis-cli hget` 实测） | 1000 条 ≈ 5.4MB |
| Redis 服务端写 250×3KB（EVAL，零网络） | **8ms** | Redis 本身不慢 |
| Redis 服务端写 1000×3KB（EVAL） | **13ms** | Redis 本身不慢 |
| Rust inline `HashReplace` 40 条（≈213KB） | **写成功**（<3s，hlen=40，0 重连） | bridge 对小量健康 |
| Rust inline `HashReplace` 250 条（≈1.35MB） | **3s 超时** | Rust 路径 ~200× 慢于裸 Redis |
| Rust inline `HashReplace` 1000 条（≈5.4MB） | **3s 超时** | 同上 |
| 泄漏 tmp key（一次会话累积） | **562 key / 467MB**；`del` 后 DBSIZE→0 | 全是泄漏，无真实数据 |

> **关键反差**：Redis 服务端 13ms 能写完 1000 条，Rust inline 路径却 3s 超时——慢源在 Rust 出站路径（`MultiplexedConnection` 大命令行为 / tokio socket / WSL2 网络？），**P2 需 profile 定位**。这是本 plan 唯一未根因定位的开放问题（见 §三）。

### 1.3 根因三连（均已读码坐实）

**① 失败的 inline 写永久 pin、饿死整条出站管道**
`runs_on_background_redis_connection`（`redis_bridge.rs`）**只对 `Publish{channel: CH_WORLD_STATE}` 返回 true**——world_state 走 clone 出来的后台连接 fire-and-forget；但 `NpcDormantHash → HashReplace` 走 **inline**。`dispatch_outbound_command` 对 inline 命令失败时返回 `Err((error, command))`，`drain_outbound_messages`（:376）把它存进 `*pending_command`（:402）并 `Reconnect`。而 `pending_command` 声明在重连循环**之外**（`redis_bridge.rs:342`），跨重连持久；每次新 session 的 `drain_outbound_messages` **先重试 `pending_command`**（:383-389），又超时，又 `Reconnect`……死循环，永远到不了 `while drained` 去 drain 新消息（含 world_state 的 prepare）。→ **所有出站 IPC 冻结**。

**② 超时泄漏 temp key**
`execute_hash_replace`（:1460）用 `tokio::time::timeout(REDIS_HASH_REPLACE_TIMEOUT=3s, execute_hash_replace_atomic(...))` 包裹。`execute_hash_replace_atomic`（:1486）流程：`DEL temp:tmp:<nonce>` → `HSET temp (全量)` → `RENAME temp→key`，清理 `DEL temp` **只在内部 op 返回 `Err` 时跑**。但 `tokio::timeout` 超时是**取消（drop）future**，不是返回 Err——HSET 进行到一半被 drop，`RENAME` 不执行、清理 `DEL temp` 也不执行，**temp key 残留**。`redis_temp_key_nonce()`（:1533）用纳秒做后缀，每次重试都是**新** key → 无限累积。

**③ 每 10s 全量重 serde + 整表替换，哪怕没变化**
`publish_world_state_to_redis`（`network/mod.rs:841`）每 200 tick 无条件调 `dormant_store.to_redis_hash_payloads()`（对全部快照 `serde_json::to_string`，主线程同步）→ 发 `NpcDormantHash(全量)` → `HashReplace` 整表 `DEL+HSET+RENAME`。dormant 老化间隔是 60s（`DORMANT_LIFECYCLE_TICK_INTERVAL`），绝大多数周期数据没变，却仍全量重写。

**①+②+③ 复合**：①把写卡死 → ②每次重试漏一个 ~5MB temp key → Redis 膨胀 → 更慢 → 更易超时 → 死亡螺旋。

---

## 二、涉及代码清单（实施直接抓这些入口）

| 文件 | 符号 / 行 | 角色 |
|------|-----------|------|
| `server/src/network/redis_bridge.rs` | `REDIS_HASH_REPLACE_TIMEOUT`（:106，3s）、`REDIS_IO_TIMEOUT`（:104，100ms）、`OUTBOUND_DRAIN_BUDGET`（:109，16） | 超时 / 预算常量 |
| 同上 | `RedisOutbound`（:123）/ `NpcDormantHash`（:125）；`RedisIoCommand`（:228）/ `HashReplace`（:241） | 出站消息 / IO 命令 enum |
| 同上 | `pending_command`（:342 声明，跨重连持久）；`drain_outbound_messages`（:376，:383-389 先重试 pending） | **bug ① 落点** |
| 同上 | `prepare_outbound_command`（:430，`RedisOutbound`→`RedisIoCommand` 翻译；`NpcDormantHash`→`HashReplace` 在 :444）；`runs_on_background_redis_connection`（:1407，仅 `CH_WORLD_STATE` 返回 true）；`dispatch_outbound_command`（:1388，分 inline / 后台 clone 连接） | inline vs 后台分派（**bug ① 关键开关**：HashReplace 现走 inline） |
| 同上 | `execute_hash_replace`（:1460，timeout 包裹）；`execute_hash_replace_atomic`（:1486，DEL/HSET/RENAME，清理仅在 op `Err` 时跑）；`redis_temp_key_nonce`（:1533） | **bug ② 落点** |
| 同上 | `connect_bridge_session`（:1574）：`pub_conn` multiplexed（:1590）+ 独立 `pubsub`（:1600） | 连接拓扑（pub/sub 已分离，排除共用干扰） |
| `server/src/network/mod.rs` | `WORLD_STATE_PUBLISH_INTERVAL_TICKS`（:164，200≈10s）；`publish_world_state_to_redis`（:841，dormant 发布 :918-928） | **bug ③ 落点** |
| `server/src/npc/dormant/mod.rs` | `NPC_DORMANT_REDIS_KEY`（:41）；`NpcDormantStore`；`to_redis_hash_payloads`（:327）；`load_dormant_snapshots_from_redis`（:375）；`seed_initial_dormant_population_on_startup`（:515，`is_empty` 守卫） | dormant store + 持久化两端 |

---

## P0 — 止血：失败的 dormant 写不再饿死 IPC + 不再泄漏 temp key ⬜

**目标**：哪怕 dormant 写仍然慢/失败，也绝不冻结其他出站 IPC，且绝不泄漏 temp key。这是**最高优先**，先让服务器在 1000 dormant 下保持可用（world_state 持续发布、客户端/agent 不受 dormant 写拖累）。

**交付物**：

1. **bug ① — 解除 pin 饿死**（二选一或组合，实施时定，见 §三开放问题 #1）：
   - **方案 A（推荐）**：把 `HashReplace`（至少 `NPC_DORMANT_REDIS_KEY` 这条）纳入后台连接分派——扩 `runs_on_background_redis_connection` 让 `RedisIoCommand::HashReplace { key: NPC_DORMANT_REDIS_KEY, .. }` 返回 true，走 clone 连接 fire-and-forget，失败只 `warn!` 不 pin。
   - **方案 B**：保留 inline 但给 `pending_command` 加**重试上限**（如 2 次）+ **drop-and-warn** 策略，超限即丢弃该命令、继续 drain 后续消息，绝不无限 pin。
   - 不论 A/B：`drain_outbound_messages` 在 `pending_command` 失败后**必须能继续 drain 新消息**，不再提前 return 把 world_state 饿死。
2. **bug ② — temp key 不泄漏**：
   - `execute_hash_replace_atomic` 改用**确定性 temp key**（如 `format!("{key}:tmp")`，去掉 `redis_temp_key_nonce`），每次重试**覆盖**同一 temp key 而非累积；并在 `execute_hash_replace` 的**超时分支**显式补一次 `DEL {key}:tmp`（best-effort，带 `REDIS_IO_TIMEOUT`）。
   - 启动时加一次性 janitor：`load_dormant_store_from_redis_system` 或 bridge 建连后 `SCAN + DEL bong:npc/dormant:tmp*`，清历史泄漏。

**测试声明**（`server/src/network/redis_bridge.rs` 内 `#[cfg(test)]`，契约级，不绑实现）：
- `dormant_hash_replace_failure_does_not_starve_other_outbound`：构造一个会失败/超时的 `HashReplace` 入队，其后入队一条 `Publish`；断言 `Publish` 仍被分派（或 `pending_command` 在有限次后释放），**不无限 pin**。
- `hash_replace_timeout_cleans_temp_key`：模拟超时分支后断言不残留 `*:tmp*`（确定性 key 被覆盖/删除）。
- `startup_janitor_purges_leaked_tmp_keys`：预置若干 `bong:npc/dormant:tmp:*`，启动后断言归零。

**验收**：`redis-cli flushall && bash scripts/start.sh`（1000 dormant）→ ① server pane **无** `reconnect ... timed out replacing hash` 无限刷屏；② `redis-cli --scan --pattern 'bong:npc/dormant:tmp:*' | wc -l` 长期 ≤ 1；③ world_state 正常发布（天道 narration 不因 dormant 断流）。**注意**：P0 不要求 1000 一定写成功（那是 P2/P3），只要求失败不致瘫。

---

## P1 — 减负：dormant 只在变化时写，去掉每 10s 全量重 serde + 整表替换 ⬜

**目标**：消除 bug ③ 的无谓开销——dormant 变化稀疏（老化 60s 一次、死亡/hydrate 偶发），不该每 200 tick 全量重 serde + 整表替换。

**交付物**：

1. `NpcDormantStore` 加 **dirty 标记**（`dirty: bool` 或 `revision: u64`），在所有 mutator 处置位：seed、`advance_dormant_position` / 老化批 tick、death/release、hydrate/dehydrate 增删快照、`rebuild_indexes` 调用点。提供 `take_dirty()` / `is_dirty()`。
2. `publish_world_state_to_redis`（`network/mod.rs:918`）改为：**仅当 `dormant_store` dirty 时**才 `to_redis_hash_payloads()` + 发 `NpcDormantHash`，并清 dirty。clean 周期直接跳过。
3. （可选，见开放问题 #3）评估 `build_world_state_snapshot` 是否仍需内嵌全量 dormant 到 `WorldStateV1`——若 agent 只需摘要，改发 digest，进一步降 world_state 体积。

**测试声明**（`server/src/npc/dormant/mod.rs`）：
- `dormant_store_dirty_set_on_seed_age_death`：seed / 老化 / death 后 `is_dirty()` 为真。
- `dormant_store_clean_after_take_dirty`：`take_dirty()` 后再无变化则 `is_dirty()` 为假。
- `dormant_publish_skipped_when_clean`（`network/mod.rs` 或集成）：无变化的发布周期不产生 `NpcDormantHash` 出站消息。

**验收**：1000 dormant 稳态运行，`HashReplace` 写次数从「每 10s 一次」降到「仅 seed 后 + 偶发变化时」；server pane `syncing N dormant NPC snapshots` debug 行不再每周期出现。

---

## P2 — 根治写慢：profile inline HashReplace 的 ~200× 慢源 + 分块/增量写 ⬜

**目标**：定位并消除「Redis 服务端 13ms、Rust inline 3s 超时」的反差，让 1000 条（5.4MB）写入稳定 < 1s。

**交付物**：

1. **Profile（先做，结论写回本 plan §三 #2）**：在 `execute_hash_replace_atomic` 各 step（DEL temp / HSET / RENAME）打 `Instant` 计时日志，定位耗时落在哪一步；对比 `MultiplexedConnection` 单条巨型 `HSET`（250+ field-arg）vs 分批 `HSET` 的耗时。候选慢源：① redis-rs `MultiplexedConnection` 对单条超大命令的 framing/flush 行为；② tokio socket 在 WSL2 下大 buffer 写；③ `cmd("HSET").arg(field_pairs)` 构造 1000 对 `(&str,&str)` 的开销。
2. **修复（依 profile 结论）**，优先级：
   - **分块 HSET**：把全量 HSET 拆成多批（如每批 100-200 field）顺序 `HSET temp`，再 `RENAME`，规避单条巨命令。
   - **增量写**：diff 当前 dirty 快照 vs 上次已写集合，只 `HSET` 变化字段 + `HDEL` 删除字段，免整表替换（需在 store 侧记 `last_persisted` 指纹或复用 P1 的 revision）。
   - **超时兜底**：若分块/增量后仍需更长窗口，按实测把 `REDIS_HASH_REPLACE_TIMEOUT` 调到留足余量（但优先把写做快，不是单纯放宽超时）。

**测试声明**：
- `hash_replace_chunks_large_payload`（`redis_bridge.rs`）：≥ N 条时按批分块，断言批数 = ceil(N/chunk)。
- `dormant_incremental_write_only_changed_fields`（若走增量）：只改 1 条快照 → 只 `HSET` 1 field + 0 `HDEL`。

**验收**：本地 `redis-cli flushall && BONG_DORMANT_ROGUE_SEED_COUNT=1000 bash scripts/start.sh` → seed 后 `redis-cli hlen bong:npc/dormant` == 1000，且 server pane 出现 `replaced hash bong:npc/dormant; entries=1000`（成功日志），无超时。

---

## P3 — 验收：1000 dormant 空 Redis 冷启动端到端 ⬜

**目标**：把 §一 的复现步骤变成「全绿」，证明默认 1000 冷启动 + 重启恢复全链路健康。

**交付物 / 验收脚本**（可固化为 `scripts/` 下手动验收清单或 smoke 子项）：

1. `redis-cli flushall && BONG_DORMANT_ROGUE_SEED_COUNT=1000 BONG_ROGUE_SEED_COUNT=20 bash scripts/start.sh`
2. 等 ~30s，断言：
   - `redis-cli hlen bong:npc/dormant` → **1000**
   - `redis-cli --scan --pattern 'bong:npc/dormant:tmp:*' | wc -l` → **0**
   - server pane `reconnect ... timed out` 计数 → **0**
   - 天道 agent pane 能收到 world_state（narration 正常）
3. `tmux kill-session -t bong`（保留 Redis）→ 重启 → 断言 `[bong][npc] loaded 1000 dormant NPC snapshot(s) from Redis HASH`（恢复成功，不再 re-seed）。
4. 长跑 5 分钟，`redis-cli info memory` 内存平稳（无 temp key 死亡螺旋）。

**验收**：上述全过；`start.sh` 默认 1000 不再需要手动降级到 40/250。

---

## 三、开放问题（P0 决策门前需收口）

> 按 `docs/CLAUDE.md` §五：实施前应追加 `## 三.1 决议（pre-P0 收口，YYYY-MM-DD）`，每条落到「文件:行号 + plan 章节」双锚点。#2 必须靠 P2 profile 产出真实数据，不许拍脑袋。

- **#1（P0 必决）** bug ① 用方案 A（dormant HashReplace 走后台连接）还是方案 B（inline + 重试上限 + drop）？权衡：A 改动小但 dormant 写变成 fire-and-forget（失败静默，靠下周期重发 + P1 dirty 保证最终一致）；B 保留 inline 可见性但要小心别再无限 pin。**倾向 A**——dormant 持久化本就允许最终一致，且 A 天然不阻塞主管道。
- **#2（P2 必决）** inline `HashReplace` ~200× 慢于裸 Redis 的真因。需 P2 profile 定位是 `MultiplexedConnection` 单条巨命令、tokio/WSL socket、还是命令构造。结论决定 P2 是「分块」「增量」还是「换 API」。
- **#3（P1 可选）** `WorldStateV1` 是否仍需内嵌全量 dormant（`network/mod.rs:913` 传 `dormant_store` 进 `build_world_state_snapshot`）？若天道只用摘要，改 digest 可同时减小 world_state 体积。需查 agent 侧对 world_state.dormant 字段的消费。

---

## 四、实施工作流（多 PR 拆分建议）

scope 约 2-3 PR，按依赖顺序序列化（前一个 merge 后开下一个）：

1. **PR-1（P0）止血**：`redis_bridge.rs` 后台分派/重试策略 + temp key 确定性化 + 启动 janitor + 三条健壮性单测。**独立可 merge**，立即让默认 1000 不致瘫。
2. **PR-2（P1）dirty 减负**：`NpcDormantStore` dirty 标记 + 发布门控 + 单测。依赖 PR-1。
3. **PR-3（P2+P3）写快 + 验收**：profile → 分块/增量写 + 1000 冷启动验收。依赖 PR-1/2。

每 PR：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 全绿；按 memory `feedback_always_pr` 走 branch + PR + CodeRabbit/Pi agent review，两 bot 确认无阻塞再 merge（`feedback_wait_coderabbit_approve`）。

---

## Finish Evidence

> 全部 P ✅ + 本节填完后，由 `/consume-plan` 或人工 `git mv` 入 `docs/finished_plans/`。

**落地清单**：（每阶段对应真实模块/文件路径，待填）

**关键 commit**：（hash + 日期 + 一句话，待填）

**测试结果**：（跑过的命令 + 数量，待填）

**跨仓库核验**：（server `bong:npc/dormant` / `RedisOutbound::NpcDormantHash` 命中；agent world_state 不再断流；client 无关，待填）

**遗留 / 后续**：（如 #3 world_state digest 若未做，登记后续，待填）
