# plan-bughunt-heartbeat-pseudo-vein-qi-mint

> **BugHunt A3（server-qi 第三轮）**。一句话主题：核验 heartbeat 自动伪灵脉曾绕过账本铸造、销毁真元的历史缺陷；生产路径已由 PR #1152 修复，本轮补齐并验证拒绝无资金生成的契约测试。

> **阶段总览：** P0 ✅ 2026-09-23（PR #1152 落地，本轮验证）；P1 ✅ 2026-09-23（PR #1152 落地，本轮验证）；P2 ✅ 2026-09-23（PR #1152 落地，本轮验证）。

## 接入面

- 进料：heartbeat omen `OmenKind::PseudoVeinForming` 通过 `WorldQiAccount` 从 `pending_inflow_account()` 借入真元。
- 出料：`ZoneRegistry::register_runtime_zone` 注册动态伪灵脉 runtime zone；`QiTransfer` 记录借出、衰减归还与消散归还。
- 结算与换算：使用 `inject_zone_for_pseudo_vein_target`、`settle_ephemeral_pseudo_vein_zone_to_target`、`settle_ephemeral_pseudo_vein_zone`；zone 比率换算使用 `QI_ZONE_UNIT_CAPACITY`。
- 共享类型 / event：复用 `WorldQiAccount`、`ZoneRegistry` 资源与 `QiTransfer` event，不新增同义 component 或 event。
- 跨仓库契约：本 plan 仅改 server 测试与文档，不改 agent/client 契约；heartbeat 伪灵脉既有 omen/VFX 下发不在范围内。
- 世界观锚点：沿用 `worldview.md §一 L18`，全服灵气总量恒定且不会凭空产生。

> 范围声明：本文只处理 heartbeat 自动伪灵脉的账本借还，不消费或修改其他 plan。起草时已避开 #975 dormant 负灵域死亡释放 `.max(0.0)` 与 #989 灵物磨损 overflow；#899 的重启恢复/持久化不替代本计划的借还语义。

## Bug 摘要

本节记录 skeleton 起草时的历史缺陷。起草时，`world::register` 同时注册新的 `world::pseudo_vein_runtime` 和旧的 `world::heartbeat`；新路径已经把灵潮/伪灵脉接入 `pending_inflow_account`，但 heartbeat 自动 omen 路径仍走旧的 `spawn_pseudo_vein_from_omen`：

- 创建时注册 `pseudo_vein_heartbeat_*` runtime zone，直接设置 `spirit_qi: omen.intensity`。
- 同时把 `PseudoVeinRuntimeState.qi_current = omen.intensity`。
- 后续 `advance_active_pseudo_veins` 每次把 `zone.spirit_qi` 覆盖为旧 state 衰减后的 snapshot，耗尽时直接设成 `0.0`。
- 整条路径没有 `WorldQiAccount` 参数，没有 `pending_inflow_account`，没有真实 `QiTransfer`。

这违反 `worldview.md §一 L18` 的“全服灵气总量恒定，不会凭空产生”，也违反 `docs/CLAUDE.md §四 L58-L60` 对自定 qi 衰减/绕过守恒账本的红线。`worldview.md §二 L38` 与 `worldview.md §十三 L1277` 允许伪灵脉作为天道陷阱出现，但 `worldview.md §二 L50` 明确存在“代偿”负灵风暴语义，不是系统外创生豁免。当前实现与逐项验真结果见“验证结论 + 证据”。

## 对实际游玩体验的影响

玩家会遇到世界心跳自动刷出的伪灵脉，并把它当成真实高灵气点使用：在该 runtime zone 里静坐修炼会按 `Zone.spirit_qi` 获得真元，固元突破会把它当成 0.8 门槛环境，炼丹起炉也会按该 zone 灵气判断是否允许。

结果是：服务器会周期性给地图凭空投放可修炼、可突破、可炼丹的高灵气窗口。玩家越会踩节奏、越会围绕伪灵脉打坐/冲境/起炉，越能把这些未从全服预算扣出的灵气变成自己的 `qi_current`。伪灵脉消散时，未被吸收的那部分又直接清零，导致真元经济既能凭空发奖，也会凭空销毁，破坏“修炼消耗 = 别人少掉”的核心体验。

## 初始证据定位（skeleton 起草时）

- 注册链：`server/src/world/mod.rs:145` 注册 `pseudo_vein_runtime::register`，`server/src/world/mod.rs:152` 随后仍注册 `heartbeat::register`。
- heartbeat 调度：`server/src/world/heartbeat.rs:454-468` 把 `heartbeat_tick`、`chain_reaction_tick`、`zone_qi_inflow_tick` 挂进 `Update`；`server/src/world/heartbeat.rs:577-668` 的 `heartbeat_tick` 会推进旧 `active_pseudo_veins`，触发 due omen，再继续 `maybe_queue_pseudo_vein`。
- 自动伪灵脉触发：`server/src/world/heartbeat.rs:1298-1359` 每 15 分钟 cadence 按季节强度排 `PseudoVeinForming` omen；强度来自 `server/src/world/heartbeat.rs:471-497` 和 `server/src/world/heartbeat.rs:1847-1861`，范围约 0.4-0.8。
- 凭空写入：`server/src/world/heartbeat.rs:1689-1748` 的 `spawn_pseudo_vein_from_omen` 注册 `Zone { spirit_qi: omen.intensity, qi_equilibrium: 0.0, qi_inflow_per_min: 0.0 }`，并设置 `state.qi_current = omen.intensity`。
- 直接衰减/清零：`server/src/world/heartbeat.rs:1002-1048` 的 `advance_active_pseudo_veins` 把 `zone.spirit_qi = advance.snapshot.spirit_qi_current`，dissipated 后直接 `zone.spirit_qi = 0.0`。
- 旧 state 自定衰减：`server/src/worldgen/pseudo_vein.rs:98-108` 直接扣 `self.qi_current`；`server/src/worldgen/pseudo_vein.rs:149-170` 自带 occupant 衰减公式。
- 新路径对比：`server/src/world/pseudo_vein_runtime.rs:455-509` 明确要求从 `pending_inflow_account` 真实借出；`server/src/world/pseudo_vein_runtime.rs:512-555` 消散时真实转回待分配池。
- ledger 语义锚点：`server/src/qi_physics/ledger.rs:326-345` 已把旧伪灵脉“凭空创生”类问题定性为缺陷，要求借还款守恒。
- 不是纯展示字段：`server/src/qi_physics/ledger.rs:641-650` 的 `summarize_world_qi` 把所有 `Zone.spirit_qi` 求和进 `zone_qi`；`server/src/cultivation/tick.rs:248-283` 用它算修炼 gain/drain；`server/src/cultivation/breakthrough.rs:382-400` 用它判突破环境；`server/src/network/client_request_handler.rs:12139-12164` 用它判炼丹起炉。

## 触发路径

1. 世界启动走 `world::register`，同时装入 `pseudo_vein_runtime` 与 `heartbeat`。
2. `heartbeat_tick` 达到 `WorldHeartbeat.pseudo_vein_cadence`，`maybe_queue_pseudo_vein` 排入 `OmenKind::PseudoVeinForming`。
3. omen 到期后，`fire_due_omens` 调用旧 `spawn_pseudo_vein_from_omen`，注册一个 `pseudo_vein_heartbeat_*` runtime zone。
4. 创建时该 zone 直接获得 `omen.intensity` 的 `spirit_qi`，没有从待分配池扣款。
5. 玩家/NPC 在该 zone 内修炼、突破或炼丹，真实消费这段高灵气。
6. 后续 heartbeat 用旧 `PseudoVeinRuntimeState` 自定衰减覆盖 `zone.spirit_qi`，耗尽时设为 `0.0`，没有把剩余余额归还待分配池。

## 反方审查记录

第一轮 subagent 反方结论：`SURVIVES`。

- 试图证明旧路径是死代码，失败：`heartbeat::register` 仍把 `heartbeat_tick` 挂进 `Update`，`heartbeat_tick` 仍调用 `maybe_queue_pseudo_vein` 与 `spawn_pseudo_vein_from_omen`。
- 试图证明新 ledger runtime 已替代旧路径，失败：新 `PseudoVeinRuntime` 是另一套 ECS component；heartbeat 自动路径没有 `Commands`、没有 `PseudoVeinRuntime` query，也没有调用 `inject_zone_for_pseudo_vein`。
- 试图证明 `Zone.spirit_qi` 只是展示字段，失败：修炼、突破、炼丹、风险热力图和 `summarize_world_qi` 都直接读它。
- 重复性核对：不重复 #975/#989；#899 是伪灵脉 runtime zone 重启丢失/持久化，不修 pending pool 借还款。

第二轮 subagent 反方结论：`SURVIVES_ROUND2`。

- 反方论点“天道伪灵脉可系统外创生”被驳回：正典只允许天道陷阱和代偿，不允许跳过全服守恒。
- 反方论点“同文件 `zone_qi_inflow_tick` 会补账”被驳回：旧 runtime zone 创建时 `qi_equilibrium = 0.0` 且 `qi_inflow_per_min = 0.0`，`zone_qi_inflow_tick` 第一层就跳过。
- 反方论点“#899 已经修掉”被驳回：#899 open PR 的正文/范围是重启恢复和持久化，没有 `WorldQiAccount` / `QiTransfer` / `pending_inflow` 语义修复。
- 反方论点“只是审计漂移”被驳回：玩家可实际把该 `spirit_qi` 转成 `qi_current` 或突破/炼丹资格。

## Skeleton Fix Plan

### P0 ✅ 2026-09-23 — 统一 heartbeat 自动伪灵脉入口（PR #1152 落地，本轮验证）

- 让 heartbeat 自动伪灵脉不再直接注册旧 `PseudoVeinRuntimeState` runtime zone。
- 优先复用 `world::pseudo_vein_runtime` 的 `PseudoVeinRuntime` component 与 `inject_zone_for_pseudo_vein` / settlement 路径。
- 若短期不能删旧 state，则旧 `spawn_pseudo_vein_from_omen` 至少必须拿到 `WorldQiAccount`，按 `pending_inflow_account -> zone` 真实借出后才能提高 `zone.spirit_qi`，并记录 `QiTransfer`。

### P1 ✅ 2026-09-23 — 收口旧衰减/消散语义（PR #1152 落地，本轮验证）

- 旧 `advance_active_pseudo_veins` 不能继续只按 `PseudoVeinRuntimeState.qi_current` 覆盖 `Zone.spirit_qi`。
- 消散时必须把未被玩家/NPC吸收的余额按 `QI_ZONE_UNIT_CAPACITY` 换算，转回 `pending_inflow_account`，等价于新 `PseudoVeinSettle` 语义。
- 链式事件 `PseudoVeinDissipated` 可保留，但 `redistributed_qi` 不能代替账本搬运。

### P2 ✅ 2026-09-23 — 隔离旧生命周期状态的真元权威（PR #1152 落地，本轮验证）

- 明确 `worldgen::pseudo_vein::PseudoVeinRuntimeState` 是 terrain/telemetry helper 还是生产 runtime。
- 如果不再作为生产 qi runtime 使用，移除 heartbeat 对它的依赖，避免下一次改动又绕回旧直写字段。
- 如果必须保留，补充注释说明它只计算展示/阶段，不拥有真元余额；真实余额以 `WorldQiAccount` 与 `Zone.spirit_qi` 同步路径为准。

## 验收测试对照

- 创建借款：`pseudo_vein_omen_borrows_from_pending_pool_without_creating_qi` 检查动态 zone 余额及 zone+ledger 总量；通用 pending-pool 注入测试检查真实余额扣减。
- 无资金拒绝：本轮新增 `pseudo_vein_omen_rejects_spawn_when_pending_pool_is_unfunded`，检查没有伪灵脉 zone/lifecycle、待分配池余额仍为零且物理总量不变。
- 衰减/消散：`heartbeat_tick_keeps_pseudo_vein_state_zone_and_ledger_in_lockstep` 检查逐 tick 对拍；`restored_pseudo_vein_first_tick_returns_dynamic_zone_balance_to_pending_pool` 检查剩余余额全额回池。
- 玩家吸收：`qi_regen_records_transfer_audit_without_mirroring_ledger_balance` 检查实际 `qi_current` 增长与 zone 扣款等额；heartbeat 创建测试确认其 zone 余额来自已扣款的池。两段同步守恒路径由同栈契约测试覆盖，无跨进程边界。
- server 门禁：`scripts/build-token.sh cargo fmt --check`、`scripts/build-token.sh cargo clippy --all-targets -- -D warnings`、`scripts/build-token.sh cargo test` 均通过。

## 验证结论 + 证据

- **创建与借款：已由 PR #1152 修复。** `spawn_pseudo_vein_from_omen` 先以 `spirit_qi: 0.0` 注册动态 zone，再调用 `inject_zone_for_pseudo_vein_target`；借款失败会移除 runtime zone，成功后 lifecycle 状态取真实 zone 余额。注入 helper 按 `pending_inflow_account` 的真实余额限额，并走 `ReleaseToZone` transfer。证据：`server/src/world/heartbeat.rs:2098`、`server/src/world/heartbeat.rs:2132`、`server/src/world/heartbeat.rs:2147`、`server/src/world/pseudo_vein_runtime.rs:509`。测试 `pseudo_vein_omen_borrows_from_pending_pool_without_creating_qi` 对拍 zone 与 ledger 总余额；`inject_zone_for_pseudo_vein_borrows_from_pending_pool_and_debits_it` 断言待分配池实际扣款。
- **衰减与消散：已由 PR #1152 修复。** heartbeat 每 tick 以 `Zone.spirit_qi` 校准 lifecycle，再通过 `settle_ephemeral_pseudo_vein_zone_to_target` 把衰减量真实转回待分配池；最终移除 zone 前会结清全部剩余余额，失败则保留 runtime 重试。证据：`server/src/world/heartbeat.rs:1349`、`server/src/world/heartbeat.rs:1379`、`server/src/world/heartbeat.rs:1412`、`server/src/world/pseudo_vein_runtime.rs:552`、`server/src/world/pseudo_vein_runtime.rs:564`。测试 `heartbeat_tick_keeps_pseudo_vein_state_zone_and_ledger_in_lockstep` 锁定持续衰减守恒；`restored_pseudo_vein_first_tick_returns_dynamic_zone_balance_to_pending_pool` 断言剩余余额全额回池。
- **缺少可用待分配余额：生产路径会拒绝生成；本轮补齐直接入口测试。** `heartbeat_tick` 要求 `ResMut<WorldQiAccount>`，缺少 resource 时系统不可运行；对存在但未注入 `pending_inflow_account` 余额的 ledger，heartbeat 入口必须返回 `None`、清除零余额 runtime zone 且不增加物理总量。新测试 `pseudo_vein_omen_rejects_spawn_when_pending_pool_is_unfunded` 断言实际 pool 余额和 zone+ledger 总量；通用 helper 的空池测试为 `inject_zone_for_pseudo_vein_is_a_noop_when_pool_is_empty`。证据：`server/src/world/heartbeat.rs:905`、`server/src/world/heartbeat.rs:2147`、`server/src/world/heartbeat_tests.rs:200`、`server/tests/unit/world/pseudo_vein_runtime_test.rs:242`。
- **玩家吸收链路：由组合契约覆盖。** `qi_regen_records_transfer_audit_without_mirroring_ledger_balance` 断言玩家实际 `qi_current` 增量、zone 实际扣款和 `CultivationRegen` transfer 等额；heartbeat 创建测试证明动态伪灵脉 zone 的余额来自真实池借款。两段为同一 server 内同步路径，不涉及跨进程时序。证据：`server/tests/unit/cultivation/tick_test.rs:219`、`server/src/world/heartbeat_tests.rs:133`。
- **修复来源：** PR #1152 已合并（2026-07-11）；生产守恒修复由 `264b80a43`（2026-07-10，生成/恢复/消散守恒）和 `59d1a8f9b`（2026-07-10，lifecycle 与账本同值）落地。`docs/plan-refactor-qi-ledger-v1.md:86` 也将其列为已修复项，本轮已独立复核实现。

## 风险

以下风险为 skeleton 起草时的实施注意事项，不表示当前仍有未修生产缺陷；本轮复核未发现账本借还路径缺口。

- #899 若先合并，会让 heartbeat 伪灵脉 runtime zone 被持久化；修本 bug 时要同时处理持久化字段中的已注入余额/借款额，否则重启后仍会出现“zone 恢复了，账本没恢复”的分叉。
- 直接切换到 `PseudoVeinRuntime` 可能影响 world heartbeat 的 omen/VFX/链式兽潮时序，需要保留 `PseudoVeinDissipated` 事件语义。
- `summarize_world_qi` 的 `zone_qi` 是分率口径，`WorldQiAccount` 是绝对量口径；测试需要沿用现有守恒测试的换算/对拍方式，避免把口径差误判成新 bug。
- 修复时不要把伪灵脉改成纯特效：它仍应是真实诱饵，只是必须从全服预算中借出并在消散时结算。

## Finish Evidence

### 落地清单

- P0：`server/src/world/heartbeat.rs:2098` 创建时以零余额注册并从池注入；`server/src/world/pseudo_vein_runtime.rs:509` 扣减真实待分配池；无资金时回滚 runtime zone。入口契约由 `server/src/world/heartbeat_tests.rs:200` 覆盖。
- P1：`server/src/world/heartbeat.rs:1349` 每 tick 从真实 zone 余额推进 lifecycle 并结算衰减；消散前全额结算后再移除。由 `server/src/world/heartbeat_tests.rs:617`、`server/src/world/heartbeat_tests.rs:809` 覆盖。
- P2：保留 `PseudoVeinRuntimeState` 负责生命周期/年龄，真元余额以 `Zone.spirit_qi` 为权威；heartbeat 同步及账本结算代码见 `server/src/world/heartbeat.rs:1369`、`server/src/world/pseudo_vein_runtime.rs:564`。

### 关键 commit

- `264b80a439ea0f244075e44584944a53d504fd17`（2026-07-10）：守住动态伪灵脉生成、恢复与消散守恒。
- `59d1a8f9b882c20305ff99004c236f63108c9e86`（2026-07-10）：保持动态伪灵脉生命周期与账本同值。
- PR #1152 于 2026-07-11 合并；本轮 promotion 为 `4bcfb803e`（2026-09-23），缺池入口回归测试为 `17881dd9c`（2026-09-23）。

### 测试结果

- `scripts/build-token.sh cargo fmt --check`：PASS。
- `scripts/build-token.sh cargo clippy --all-targets -- -D warnings`：PASS。
- `scripts/build-token.sh cargo test`：12,588 passed，0 failed；另有 5 个文档测试标记为 ignored。
- 定向回归 `scripts/build-token.sh cargo test pseudo_vein_omen_rejects_spawn_when_pending_pool_is_unfunded`：1 passed，0 failed。

### 跨仓库核验

- Server：`spawn_pseudo_vein_from_omen`、`QiTransfer`、`PseudoVeinSettle` 与 heartbeat/ledger 契约测试命中本修复范围。
- Agent/schema：`agent/packages/schema/src/pseudo-vein.ts:13` 定义 `PseudoVeinSnapshotV1`，`:28` 定义 `PseudoVeinDissipateEventV1`；`agent/packages/schema/src/channels.ts:265`、`:268` 定义活动/消散频道，本轮未改动。
- Client：`client/src/main/java/com/bong/client/hud/OmenHudPlanner.java:38` 处理 `PSEUDO_VEIN` HUD 表现，本轮未改动。

### 遗留 / 后续

- 本计划范围内无生产代码待办；本轮仅补齐空待分配池时 heartbeat 入口拒绝生成的回归测试。
- Agent/schema 与 Client 的既有协议和表现契约未变，不需要跨栈改动。
