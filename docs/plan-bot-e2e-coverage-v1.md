# plan-bot-e2e-coverage-v1 — 协议级黑盒 Bot e2e 模块全覆盖

一句话主题：把 `scripts/bot/` 协议级玩家 Bot 的场景覆盖推到"每个 gameplay/网络模块都有对应 Bot e2e 场景"，让 CI 在无真人客户端条件下锁住玩家可感知行为（AGENTS.md §15）。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 框架 + 首批 4 场景 + CI Bot e2e stage | ✅ 2026-07-06（本骨架随 P0 PR 进库） |
| P1 | 修炼模块：realm/qi/meridian dev 命令 + breakthrough intent 链路 | ⬜ |
| P2 | 战斗模块：attack NPC → combat_hit/死亡链路；skill cast intent → 招式专属通道 | ⬜ |
| P3 | 库存/物品：背包 intent、容器、装备、`/clearinv` 分支 | ⬜ |
| P4 | 生产系统：炼丹 / 锻造 / 种植（lingtian）/ 采集 | ⬜ |
| P5 | 多 bot 并发：同打一个 NPC、组队渡劫、贸易、chat→天道 narration | ⬜ |
| P6 | `bong:server_data` protobuf 深断言（Python pb 生成，数值级 qi/HUD 校验） | ⬜ |

## P0 — 框架 + 首批场景（本 PR）

- `scripts/bot/mc_protocol.py` — MC 763 offline 传输层（帧/压缩/登录，`Connection._try_parse_frame` 半帧安全）
- `scripts/bot/bot.py` — `class Bot`（动作：`cmd/chat/intent/send_payload/move_to/attack_entity`；断言：`wait_for/expect_payload/expect_chat/assert_alive`）
- 场景 ×4：`terrain_join_chunk_delivery`（pin PR#846 ChunkCenter）/ `network_session_tolerance` / `network_client_request_tolerance` / `cmd_dev_give_feedback`
- `scripts/bot-e2e.sh` + e2e.yml「Bot e2e stage」（`BOT_E2E_KILL_STALE=1`）
- `scripts/e2e-redis.sh` cleanup 改 `kill_tree`（孤儿 server 修复，见问题记录 #2）

## P1 — 修炼模块（下一步建议）

- 场景 `cultivation_realm_qi.py`：`/realm set` + `/qi set` → chat 反馈断言；非法值分支
- 场景 `cultivation_breakthrough.py`：dev 铺垫 → `intent({"type":"breakthrough"})` → `bong:breakthrough_*` payload 到达
- 抓手：`server/src/cmd/dev/realm.rs`、`schema/client_request.rs` breakthrough variant

## P2 — 战斗模块

- 场景：找最近 NPC（entity_spawn 观察）→ `attack_entity` → `bong:combat_hit` payload
- 技能：`/technique add` → skill cast intent → 招式专属 `bong:<skill>` 通道到达
- 依赖决策：cast intent 的精确 JSON 形状需从 `schema/client_request.rs` 逐 variant 对照

## P3 — 库存/物品

- `/clearinv pack|all|naked` 三分支、背包穿脱 intent（`bong:inventory_pack_*`）、容器双击打开

## P4 — 生产系统

- 炼丹（`bong:alchemy_*` 通道族）、锻造（`bong:forge_*`）、灵田（`bong:lingtian_*`）、采集 tick 通道

## P5 — 多 bot 并发

- 两 bot 同 server：互见 entity_spawn、同打 NPC 伤害归属、chat → `bong:player_chat` → 天道 narration 回流（需 agent 联跑，考虑挂在 smoke-e2e 后半）

## P6 — server_data protobuf 深断言

- 从 `proto/` 生成 Python bindings（CI 已装 protoc），对 `bong:server_data` 做数值级断言（qi 数值、HUD 状态）
- 决策门：CI 是否引入 protobuf Python 依赖（目前框架零依赖）

## 问题记录（开发中实际踩到，后续阶段留意）

1. **共享 target 的旧二进制不可直接跑**：`server/target/debug/bong-server` 是从已删 worktree（`.worktree/consume-tsy-search-cancel-v1`）编译的，`CARGO_MANIFEST_DIR` 编译期烙死 → 启动即 panic（loot_pools.json not found）。结论：bot-e2e.sh 必须 `cargo run` from 当前 checkout，禁止直接执行 target 里的二进制。
2. **e2e-redis.sh 孤儿 server**：cleanup 只 `kill` 子 shell，bash 不向子进程转发 SIGTERM（已实验证实），`cargo run`/`bong-server` 变孤儿继续占 25565——本地跑完 smoke 会漏进程，CI 里会卡死后续要用该端口的 stage。P0 已修（`kill_tree` 递归杀树 + bot-e2e.sh `BOT_E2E_KILL_STALE` 兜底）。
3. **763 命令包带签名字段**：`CommandExecution(0x04)` 尾部 timestamp/salt/签名数/message_count/20-bit BitSet 全零即可过 offline server，但字节布局错一位整包被丢（无反馈）。包 ID/布局唯一权威 = valence checkout `tools/packet_inspector/extracted/packets.json`，别信 wiki.vg 其他版本页。
4. **短 timeout 轮询下的半帧撕裂**：reader 用 0.5s socket timeout 时，timeout 可能落在帧长度前缀读一半处；朴素"边读边消费"实现会把已读字节丢掉导致整条流错位。框架已用"缓冲区攒够完整帧才消费"规避（`_try_parse_frame`），后续写新协议工具照抄这个模式。
5. **raster-less 世界盖不住 spawn 散布区（server 侧真缺口，建议后续修）**：`server/src/world/mod.rs` fallback 平台日志自称 "16x16 chunks centered on spawn"，实际 centered on **origin**；spawn 迁移（#808）+ zone "spawn" 散布后，玩家/bot 常出生在平台外纯虚空（实测三连 join：chunk(11,3) 34 chunk / chunk(-15,-15) 0 chunk ×2）。影响：CI e2e 与本地 raster-less dev server 的玩家出生即虚空 + 坠落回弹；`terrain_join_chunk_delivery` 场景的 chunk 投递 leg 只能自适应跳过（已显式打印）。修法候选：fallback 平台以真实 spawn 点为中心生成、或覆盖整个 spawn zone；修好后把场景下限收紧 ≥8。
6. **Bot.wait_for 的 predicate 持锁回调死锁**：predicate 在事件锁内执行，回调 `events_of()` 等同样拿锁的方法时非重入锁直接死锁（连 SIGTERM 都收不干净）。已改 `RLock` 修复；写框架新等待原语时沿用。
7. **并发 orchestrator 环境下"端口开 ≠ server 就绪"**：本机 CARGO_TARGET_DIR 全局指向共享 target，别的 agent 的 cargo 会占 build lock 把 `cargo run` 卡住；同时 25565 上可能出现别人集成测试的瞬时 listener（接受 TCP 几秒后断），单看端口会误判就绪、bot 连上直接 connection_lost。bot-e2e.sh 已改「自己 log 的 bootstrap 锚点 + 端口」双条件，并对 build lock 卡死给出显式提示。CI 单租户无此问题，本地多 agent 并发时留意。
