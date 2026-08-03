# plan-bot-e2e-coverage-v1 — 协议级黑盒 Bot e2e 模块全覆盖

一句话主题：把 `scripts/bot/` 协议级玩家 Bot 的场景覆盖推到"每个 gameplay/网络模块都有对应 Bot e2e 场景"，让 CI 在无真人客户端条件下锁住玩家可感知行为（AGENTS.md §15）。

## 接入面

- **进料**：MC 1.20.1 protocol 763 的 S2C/C2S 包、`bong:server_data` protobuf `Envelope`、dev 命令反馈、Redis `bong:player_chat`，以及现有修炼/战斗/库存/生产/交易/渡劫模块的权威事件与快照。
- **出料**：`scripts/bot/scenarios/` 的协议级黑盒断言、`scripts/bot/test_protocol.py` 的编解码/场景契约回归，以及 `.github/workflows/e2e.yml` 的 Bot e2e 证据；框架只观测并驱动生产链路，不另造 gameplay 状态。
- **复用类型 / event / schema**：复用 `Envelope` oneof、`player_state`、`breakthrough_cinematic`、`combat_event`、`cast_sync`、`inventory_snapshot`、`alchemy_outcome`、`Narration` 与 `Tribulation*` 契约；MC 包 ID 以 Valence pin `2b705351` 的 packet inspector 为唯一来源。
- **跨仓库契约**：server 负责 emit/消费协议与 Redis 消息；agent 通过 `bong:player_chat` 和 Narration schema 产出回流；client 与 Bot 共同消费 `bong:server_data`，其中 Bot 的 `proto_min.py` 必须与 proto oneof 标签及客户端 HUD handler 对拍。
- **worldview 锚点**：修炼/真元对应 `worldview.md` §三，战斗对应 §四/§五，天道 narration 对应 §八，交易与骨币对应 §九，玩家社交边界对应 §十一。
- **qi_physics 锚点**：本 plan 不定义物理公式或常数；涉及真元变动的场景只验证现有 `qi_physics::ledger::QiTransfer` 所驱动的外部可观察结果与守恒，不直接改写账户。
- **运行基础设施边界**：P0 的 Bot CI/preview 启停依赖全机共享 Cargo/Gradle 槽与 identity-safe server lifecycle；`scripts/build-token.sh`、`scripts/lib/bong-server-lifecycle.sh`、`scripts/preview/run-server-headless.sh` 只承载测试 harness 的并发和清理权限，不改变 gameplay。对应回归必须锁住跨 worktree 共享锁域、PID/starttime/executable identity、精确 listener owner 与 fail-closed cleanup。

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
- **玩家可感知验收**：Bot 必须按时序观察 `cast_sync(casting → complete)`，并分别匹配每招既有的 `vfx_id`、SFX/战斗事件与 HUD 反馈；不得以 raw bytes 或无类型聊天替代。动画/粒子/音效/icon 的具体视觉资产由各招式所属 plan 定义，本 plan 只锁已发布协议身份及先后顺序，不新增资产。

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
- **玩家可感知验收**：生产场景必须观察既有 typed 进度、成功/拒绝 outcome、库存数量与落地物；粒子、音效、HUD 和动画按炼丹/锻造/灵田/采集所属 finished plan 的既有 ID 对拍，本 plan 不以聊天提示代替这些可见结果，也不另立视觉规格。

## P5 — 多 bot 并发 ⏳

已落地：

- `scripts/bot/scenarios/multibot_chat_visibility.py`：两 Bot 互见 `PlayerSpawn`，并对共同观察到的同一 passive NPC 各自产生 typed outgoing hit。
- `scripts/bot/scenarios/network_chat_echo.py`：同 zone 广播（含发送者 echo）与跨 zone 隔离。
- `scripts/bot/scenarios/agent_ui_realm_gate_private_narration.py`：真实 `AgentUiRuntime` 境界门拒绝只向目标 Bot 发送 `system_warning`。

剩余验收（本 plan 保持 active）：

- 真实双玩家交易。
- 组队渡劫。
- 完整 `chat → bong:player_chat → Tiandao → narration` Agent 联跑回流。

- **玩家可感知验收**：双 Bot 的交易、组队渡劫和聊天回流必须分别证明“发起方/接收方/旁观者”的可见范围；Tiandao 回流使用现有 `Narration` scope/style，至少锁定 player/zone 隔离和一条符合 §八语调的真实 narration，不以 server echo 冒充 Agent 输出。

## P6 — server_data protobuf 深断言 ⏳

已落地：

- `scripts/bot/proto_min.py`：纯 stdlib 的 protobuf wire decoder，不新增 CI Python 依赖；按 `Envelope` oneof tag 分发 typed payload。
- `scripts/bot/server_data.py` 与 `scripts/bot/test_protocol.py`：数值级断言 `player_state.spirit_qi`、`breakthrough_cinematic`、`craft_session_state.elapsed_ticks`、库存/容器/战斗/生产 payload。
- 真实场景不以 raw bytes/chat 假阳性替代 typed payload。

剩余验收（本 plan 保持 active）：

- 盘点并补齐所有仍未覆盖的 HUD oneof（含 `combat_hud_state`）。
- 若未来改用生成式 Python bindings，需单独决定依赖与构建产物策略；本阶段当前选择零依赖 decoder，不虚报“已生成 bindings”。

## 问题记录（开发中实际踩到，后续阶段留意）

1. **共享 target 的旧二进制不可直接跑**：`server/target/debug/bong-server` 可能来自其他或已删 worktree，`CARGO_MANIFEST_DIR` 编译期烙死后会指向错误资产路径。结论：bot-e2e.sh 必须从当前 checkout 经 build-token 完成 `cargo build`，在令牌保护的成功构建后复制本轮 immutable binary，再在令牌外运行该副本；禁止直接执行共享 target 里的旧二进制。
2. **e2e-redis.sh 孤儿 server**：cleanup 只 `kill` 子 shell，bash 不向子进程转发 SIGTERM（已实验证实），`cargo run`/`bong-server` 变孤儿继续占 25565——本地跑完 smoke 会漏进程，CI 里会卡死后续要用该端口的 stage。P0 已修（`kill_tree` 递归杀树 + bot-e2e.sh `BOT_E2E_KILL_STALE` 兜底）。
3. **763 命令包带签名字段**：`CommandExecution(0x04)` 尾部 timestamp/salt/签名数/message_count/20-bit BitSet 全零即可过 offline server，但字节布局错一位整包被丢（无反馈）。包 ID/布局唯一权威 = valence checkout `tools/packet_inspector/extracted/packets.json`，别信 wiki.vg 其他版本页。
4. **短 timeout 轮询下的半帧撕裂**：reader 用 0.5s socket timeout 时，timeout 可能落在帧长度前缀读一半处；朴素"边读边消费"实现会把已读字节丢掉导致整条流错位。框架已用"缓冲区攒够完整帧才消费"规避（`_try_parse_frame`），后续写新协议工具照抄这个模式。
5. **raster-less fallback 世界覆盖已闭环**：`server/src/world/mod.rs` 从有效 spawn distribution（空配置则 patrol/emergency anchor）构造非空 chunk union，并按最高境界 20-chunk view distance 填平所有可出生视域；启动只在 `anchors/chunks/view_distance_chunks` 均为正整数时发布结构化 `BOT_FALLBACK_FLAT_READY`。`terrain_join_chunk_delivery` 在专用 fallback ownership 下要求三簇真实出生及至少八个 chunk，不再跳过 chunk-delivery leg。
6. **Bot.wait_for 的 predicate 持锁回调死锁**：predicate 在事件锁内执行，回调 `events_of()` 等同样拿锁的方法时非重入锁直接死锁（连 SIGTERM 都收不干净）。已改 `RLock` 修复；写框架新等待原语时沿用。
7. **并发 orchestrator 环境下“端口开 ≠ server 就绪”**：本机共享 target 可能被别的 agent 构建；同时 25565 上可能出现其他集成测试的瞬时 listener。`bot-e2e.sh` 因此以“本轮 immutable binary 的结构化 bootstrap 锚点 + listener 属于本轮进程树”双门校验 ownership；专用 fallback/ambient 模式还使用本轮私有 Redis，拒绝共享状态假阳性。

## §9 开放问题（P5/P6 决策门）

1. P5 的真实交易以哪条现有交易协议作为首个 Bot 验收入口？
2. 组队渡劫怎样证明同一事件的参与者与旁观者 scope，而不靠脆弱聊天文案？
3. Tiandao 联跑在 CI 中使用真实 Agent 模型还是确定性 mock？怎样证明 Redis 往返而非 server echo？
4. P6 的 HUD oneof 完整清单以哪个生成物为唯一真相，如何防 proto 增字段后静默漏测？

## §9.1 决议（pre-P5/P6 收口，2026-07-29）

### #1 交易入口

**决议**：首个场景复用已落地的 `TradeOfferRequest` / `TradeOfferResponse`、typed `trade_offer` 与权威 `inventory_snapshot` revision；双方各交换一个现有 item instance，并以交换前后 instance 集合不变证明无复制/丢失。当前玩家交易不是骨币支付路径，不虚构余额断言，也不新建 Bot 专用旁路。

**落点**：`server/src/social/mod.rs:1020-1243`、`proto/bong/envelope.proto:2333-2344` + P5 §「真实双玩家交易」。

### #2 组队渡劫 scope

**决议**：用 `Tribulation*` schema 的 `char_id`、`zone_name` 与参与者列表作为身份锚；参与 Bot 必须收到 typed 阶段事件，zone 内旁观者只收到契约允许的 zone narration，跨 zone Bot 不收到。断言事件 identity/scope，不匹配自由文本。

**落点**：`agent/packages/schema/src/tribulation.ts:82-99` + P5 §「组队渡劫」。

### #3 Tiandao 联跑

**决议**：CI 使用确定性 Tiandao mock，但必须保留完整 `chat → bong:player_chat → agent consumer → bong:agent_narrate/Narration → server → Bot` Redis 往返；测试为每轮生成 token/timestamp，并断言回流携带该关联值，server 本地 echo 不能满足。

**落点**：`agent/packages/tiandao/src/redis-ipc.ts:927-940,983-991`、`agent/packages/schema/src/channels.ts:10-16`、`agent/packages/schema/src/narration.ts`、`agent/packages/schema/src/client-payload.ts` + P5 §「Agent 联跑回流」。

### #4 HUD oneof 真相源

**决议**：从 `proto/bong/envelope.proto` 的 `ServerDataPayload` oneof 自动提取 tag/name 清单，与 `SERVER_DATA_PAYLOAD_NAMES`、decoder 分派和场景覆盖矩阵做集合等价测试；新增 oneof 后测试必须先红。未知枚举值保留可诊断 identity，不折叠成缺省值。

**落点**：`proto/bong/envelope.proto:16-102`、`scripts/bot/proto_min.py`、`scripts/bot/test_protocol.py` + P6。

以上问题均以 §9.1 为实施基线；若生产 symbol/handler 在 P5/P6 开工前变化，先原地更新本节再实现。

## §10 实施工作流

### §10.1 单 plan 多 PR 序列化

本 plan 已跨 P0-P6，后续仍保持一个 active plan，按依赖序列逐 PR 落地：

1. **PR-P5a**：真实双玩家交易，含双方 revision/守恒与拒绝分支。
2. **PR-P5b**：组队渡劫参与者/旁观者 scope。
3. **PR-P5c**：Redis Tiandao narration 完整回流。
4. **PR-P6**：由 proto 真相源驱动的 HUD oneof 覆盖矩阵，补齐 `combat_hud_state` 等缺口。
5. **归档 PR**：仅当 P5/P6 全部 ✅、Finish Evidence 齐全后，原子迁入 `docs/finished_plans/`。

前一 PR merge 并通过 e2e、`/review` 与 CodeRabbit 后才开下一 PR；不得并行修改同一 Bot decoder/场景注册表。

### §10.2 实施与验证约束

- 每个 PR 使用独立实现上下文，先从 `origin/main` 核验 production handler/schema，不得凭本 plan 的旧 symbol 猜实现。
- 场景代码、protocol decoder、饱和单测和 CI 注册必须同 PR；禁止只加 helper 或只加 mock。
- 本 plan 无建筑/NBT/layout/新视觉资产交付，不适用 3 轮视觉打磨；若阶段意外引入资产，必须转由所属 gameplay plan 定义并遵守 `<PROMISE>` 纪律。
- 本地验证不得运行被安全隔离的 shutdown-order 信号测试；真实 shutdown-order 覆盖保留在 GitHub e2e，PR 测试说明必须披露本地跳过。
- 最终精确 HEAD 必须经 fresh-context read-only validator；HEAD 变化后重验。

### §10.3 Review 与自动归档

- Push 后发送独立 `/review` 评论；只以 `/review` 与 CodeRabbit 为 review gate，不等待 Codex。
- review 修改带来新 HEAD 后重跑受影响门禁并重发 `/review`。
- P5/P6 均完成后，由最后一轮实施补齐 `## Finish Evidence`（落地清单、commit、测试、跨仓 symbol、遗留），再 `git mv` 归档；任何阶段仍为 ⏳ 时禁止归档。
- 单次 consume-plan 只编排当前未完成阶段；用户无需手工拆 plan，但 merge 仍遵守仓库授权边界。

### §10.4 单次 consume-plan 全自动到 merge

1. 用户提交 `/consume-plan` 后，orchestrator 只选择 §10.1 中最前一个尚未完成的 PR，不跨阶段并行，也不提前改 P5/P6 状态。
2. 独立实施 subagent 从最新 `origin/main` 落地场景、decoder、饱和测试与 CI 注册；随后运行对应栈门禁，并对最终精确 HEAD 启动 fresh-context read-only validator。validator、门禁或 GitHub e2e 任一失败都回到实施步骤，HEAD 变化后旧结论作废。
3. subagent push 并创建该阶段 PR；orchestrator 发送独立 `/review`，持续处理 `/review`、CodeRabbit 与 e2e 结论。返工 push 后重新验证并重发 `/review`，直到没有仍成立的阻塞意见。
4. 仅 orchestrator 在既有授权边界内 merge 已收敛 PR；实施 subagent 不自行 merge。若当前会话无 merge 授权，则停在可合并状态交给获授权主体，不把“已开 PR”记成阶段完成。
5. 前一 PR merge 后再消费下一个条目。只有 PR-P5a/P5b/P5c/P6 全部 merge，才更新 P5/P6 为 `✅ YYYY-MM-DD`，追加完整 `## Finish Evidence`，并通过独立归档 PR 把 plan 迁入 `docs/finished_plans/`；此前本 plan 必须保持 active。
