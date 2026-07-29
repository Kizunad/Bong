# plan-bot-e2e-coverage-v1 — 协议级黑盒 Bot e2e 模块全覆盖

一句话主题：把 `scripts/bot/` 协议级玩家 Bot 的场景覆盖推到"每个 gameplay/网络模块都有对应 Bot e2e 场景"，让 CI 在无真人客户端条件下锁住玩家可感知行为（AGENTS.md §15）。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 框架 + 首批 4 场景 + CI Bot e2e stage | ✅ 2026-07-06（本骨架随 P0 PR 进库） |
| P1 | 修炼模块：realm/qi/meridian dev 命令 + breakthrough intent 链路 | ✅ 2026-07-29 |
| P2 | 战斗模块：attack NPC → typed combat/death；skill cast intent → cast/VFX/SFX | ✅ 2026-07-29 |
| P3 | 库存/物品：背包 intent、容器、装备、`/clearinv` 分支 | ✅ 2026-07-29 |
| P4 | 生产系统：炼丹 / 锻造 / 制作 / 灵田 / 采集 | ✅ 2026-07-29 |
| P5 | 多 bot 并发：可见性/共同 NPC/chat 隔离已落地；组队渡劫/贸易/Agent 回流待补 | ⏳ |
| P6 | `bong:server_data` 零依赖 protobuf 深解码已落地；剩余 HUD oneof 全覆盖 | ⏳ |

## P0 — 框架 + 首批场景（本 PR）

- `scripts/bot/mc_protocol.py` — MC 763 offline 传输层（帧/压缩/登录，`Connection._try_parse_frame` 半帧安全）
- `scripts/bot/bot.py` — `class Bot`（动作：`cmd/chat/intent/send_payload/move_to/attack_entity`；断言：`wait_for/expect_payload/expect_chat/assert_alive`）
- 场景 ×4：`terrain_join_chunk_delivery`（pin PR#846 ChunkCenter）/ `network_session_tolerance` / `network_client_request_tolerance` / `cmd_dev_give_feedback`
- `scripts/bot-e2e.sh` + e2e.yml「Bot e2e stage」（`BOT_E2E_KILL_STALE=1`）
- `scripts/e2e-redis.sh` cleanup 改 `kill_tree`（孤儿 server 修复，见问题记录 #2）

## P1 — 修炼模块 ✅ 2026-07-29

- `scripts/bot/scenarios/cultivation_realm_qi.py`：`/realm set`、`/qi set`、`/qi max`、`/meridian open` 的成功、拒绝、钳制与重复状态反馈。
- `scripts/bot/scenarios/cultivation_breakthrough.py`：`breakthrough_request` → typed `breakthrough_cinematic`，并锁定 production realm wire `Awaken → Induce`。
- `scripts/bot/scenarios/cultivation_pill_consume.py`：双入口吃丹、`qi_current` 权威回升、库存扣除与空丹宽容。
- 抓手：`server/src/cmd/dev/realm.rs`、`server/src/network/client_request_handler.rs`、`scripts/bot/proto_min.py`。

## P2 — 战斗模块 ✅ 2026-07-29

- `scripts/bot/scenarios/combat_attack_hit.py`：确定性 passive NPC、原版攻击包、typed outgoing hit 与生产死亡链路精确销毁。
- `scripts/bot/scenarios/combat_skill_cast.py`：`skill_bar_bind` / `skill_bar_cast` 的权威绑定、`cast_sync`、独立 VFX 与战斗反馈。
- `scripts/bot/scenarios/combat_weapon_equip_damage.py`：空手基线与满耐久铁剑的 outgoing damage 契约。
- `scripts/bot/scenarios/combat_respawn_stops_low_hp_heartbeat.py`、`combat_technique_sword_av.py`：重生收掉低血心跳，剑招三反馈与断脉拒因。

## P3 — 库存/物品 ✅ 2026-07-29

- `scripts/bot/scenarios/inventory_pack_move_intents.py`：权威 `inventory_snapshot`、动态 `pack_<owner_instance_id>`、`chest_worn` LIFO、同实例穿脱、容量与 `/clearinv pack|all|naked`。
- `scripts/bot/scenarios/inventory_container_open_minimal.py`：真实 `trade_crate` 放置、typed open/snapshot、双向 move、拒绝回滚、无丢失复制与 close。
- `scripts/bot/scenarios/inventory_equip_wearer_race_reject.py`、`inventory_supply_coffin_cross_dimension.py`：装备门拒因与跨维容器 session 门。
- 抓手：`scripts/bot/scenarios/_inventory_helpers.py`、`server/src/inventory/mod.rs`、`server/src/network/client_request_handler.rs`。

## P4 — 生产系统 ✅ 2026-07-29

- `production_alchemy_brew_pill.py`：真炉放置、点火、投料、丹成入包与数量错误负分支。
- `production_forge_station_real_place.py`：真砧放置、图谱、起炉拒因/受理、十次淬炼与成品入包。
- `production_handcraft_stone_knife.py`、`production_craft_cancel_full_inventory_refund.py`、`production_craft_disconnect_resume.py`：制作完成、满包取消落地、断线恢复与 exactly-once 退款。
- `production_lingtian_gathering_intents.py`、`production_spiritwood_full_inventory_drop.py`：权威地形开垦、深解码采集进度、240-tick 灵木采伐、freshness wire 保真与同实例拾回。
- `scripts/bot/make_novice_raster_fixture.py` 从 `server/zones.json` 生产 `spawn_distribution` 派生草地 tiles，避免用户名 hash 落入 Stone fallback。

## P5 — 多 bot 并发 ⏳

已落地：

- `scripts/bot/scenarios/multibot_chat_visibility.py`：两 Bot 互见 `PlayerSpawn`，并对共同观察到的同一 passive NPC 各自产生 typed outgoing hit。
- `scripts/bot/scenarios/network_chat_echo.py`：同 zone 广播（含发送者 echo）与跨 zone 隔离。
- `scripts/bot/scenarios/agent_ui_realm_gate_private_narration.py`：真实 `AgentUiRuntime` 境界门拒绝只向目标 Bot 发送 `system_warning`。

剩余验收（本 plan 保持 active）：

- 真实双玩家交易。
- 组队渡劫。
- 完整 `chat → bong:player_chat → Tiandao → narration` Agent 联跑回流。

## P6 — server_data protobuf 深断言 ⏳

已落地：

- `scripts/bot/proto_min.py`：纯 stdlib 的 protobuf wire decoder，不新增 CI Python 依赖；按 `Envelope` oneof tag 分发 typed payload。
- `scripts/bot/server_data.py` 与 `scripts/bot/test_protocol.py`：数值级断言 `player_state.spirit_qi`、`breakthrough_cinematic`、`craft_session_state.elapsed_ticks`、库存/容器/战斗/生产 payload。
- 真实场景不以 raw bytes/chat 假阳性替代 typed payload。

剩余验收（本 plan 保持 active）：

- 盘点并补齐所有仍未覆盖的 HUD oneof（含 `combat_hud_state`）。
- 若未来改用生成式 Python bindings，需单独决定依赖与构建产物策略；本阶段当前选择零依赖 decoder，不虚报“已生成 bindings”。

## 问题记录（开发中实际踩到，后续阶段留意）

1. **共享 target 的旧二进制不可直接跑**：`server/target/debug/bong-server` 是从已删 worktree（`.worktree/consume-tsy-search-cancel-v1`）编译的，`CARGO_MANIFEST_DIR` 编译期烙死 → 启动即 panic（loot_pools.json not found）。结论：bot-e2e.sh 必须 `cargo run` from 当前 checkout，禁止直接执行 target 里的二进制。
2. **e2e-redis.sh 孤儿 server**：cleanup 只 `kill` 子 shell，bash 不向子进程转发 SIGTERM（已实验证实），`cargo run`/`bong-server` 变孤儿继续占 25565——本地跑完 smoke 会漏进程，CI 里会卡死后续要用该端口的 stage。P0 已修（`kill_tree` 递归杀树 + bot-e2e.sh `BOT_E2E_KILL_STALE` 兜底）。
3. **763 命令包带签名字段**：`CommandExecution(0x04)` 尾部 timestamp/salt/签名数/message_count/20-bit BitSet 全零即可过 offline server，但字节布局错一位整包被丢（无反馈）。包 ID/布局唯一权威 = valence checkout `tools/packet_inspector/extracted/packets.json`，别信 wiki.vg 其他版本页。
4. **短 timeout 轮询下的半帧撕裂**：reader 用 0.5s socket timeout 时，timeout 可能落在帧长度前缀读一半处；朴素"边读边消费"实现会把已读字节丢掉导致整条流错位。框架已用"缓冲区攒够完整帧才消费"规避（`_try_parse_frame`），后续写新协议工具照抄这个模式。
5. **raster-less 世界盖不住 spawn 散布区（server 侧真缺口，建议后续修）**：`server/src/world/mod.rs` fallback 平台日志自称 "16x16 chunks centered on spawn"，实际 centered on **origin**；spawn 迁移（#808）+ zone "spawn" 散布后，玩家/bot 常出生在平台外纯虚空（实测三连 join：chunk(11,3) 34 chunk / chunk(-15,-15) 0 chunk ×2）。影响：CI e2e 与本地 raster-less dev server 的玩家出生即虚空 + 坠落回弹；`terrain_join_chunk_delivery` 场景的 chunk 投递 leg 只能自适应跳过（已显式打印）。修法候选：fallback 平台以真实 spawn 点为中心生成、或覆盖整个 spawn zone；修好后把场景下限收紧 ≥8。
6. **Bot.wait_for 的 predicate 持锁回调死锁**：predicate 在事件锁内执行，回调 `events_of()` 等同样拿锁的方法时非重入锁直接死锁（连 SIGTERM 都收不干净）。已改 `RLock` 修复；写框架新等待原语时沿用。
7. **并发 orchestrator 环境下"端口开 ≠ server 就绪"**：本机 CARGO_TARGET_DIR 全局指向共享 target，别的 agent 的 cargo 会占 build lock 把 `cargo run` 卡住；同时 25565 上可能出现别人集成测试的瞬时 listener（接受 TCP 几秒后断），单看端口会误判就绪、bot 连上直接 connection_lost。bot-e2e.sh 已改「自己 log 的 bootstrap 锚点 + 端口」双条件，并对 build lock 卡死给出显式提示。CI 单租户无此问题，本地多 agent 并发时留意。
