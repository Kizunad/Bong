# plan-bughunt-heartbeat-pseudo-vein-qi-mint

> **Skeleton / BugHunt A3（server-qi 第三轮）**。一句话主题：`world::heartbeat` 旧伪灵脉自动生成路径仍直接把 `Zone.spirit_qi` 写成 0.4-0.8 的高灵气 runtime zone，并在后续 tick 直接衰减/清零；该路径没有从 `pending_inflow_account` 借出、没有 `WorldQiAccount::transfer`、没有 `QiTransfer` 审计，导致伪灵脉自动刷新时凭空铸造灵气，消散时又把未被吸收的灵气直接销毁。

> 范围声明：本 skeleton 只记录 bug，不消费/归档 plan，不修改源码。已避开 #975 dormant 负灵域死亡释放 `.max(0.0)` 与 #989 灵物磨损 overflow 未落账；也核对 #899 仅覆盖 heartbeat 伪灵脉 runtime zone 重启恢复/持久化，不覆盖本条账本借还语义。

## Bug 摘要

`world::register` 同时注册了新的 `world::pseudo_vein_runtime` 和旧的 `world::heartbeat`。新路径已经把灵潮/伪灵脉接入 `pending_inflow_account`，通过 `inject_zone_for_pseudo_vein` 真实借出、消散时 `PseudoVeinSettle` 归还；但 heartbeat 自动 omen 路径仍走旧的 `spawn_pseudo_vein_from_omen`：

- 创建时注册 `pseudo_vein_heartbeat_*` runtime zone，直接设置 `spirit_qi: omen.intensity`。
- 同时把 `PseudoVeinRuntimeState.qi_current = omen.intensity`。
- 后续 `advance_active_pseudo_veins` 每次把 `zone.spirit_qi` 覆盖为旧 state 衰减后的 snapshot，耗尽时直接设成 `0.0`。
- 整条路径没有 `WorldQiAccount` 参数，没有 `pending_inflow_account`，没有真实 `QiTransfer`。

这违反 `worldview.md §一 L18` 的“全服灵气总量恒定，不会凭空产生”，也违反 `docs/CLAUDE.md §四 L58-L60` 对自定 qi 衰减/绕过守恒账本的红线。`worldview.md §二 L38` 与 `worldview.md §十三 L1277` 允许伪灵脉作为天道陷阱出现，但 `worldview.md §二 L50` 明确存在“代偿”负灵风暴语义，不是系统外创生豁免。

## 对实际游玩体验的影响

玩家会遇到世界心跳自动刷出的伪灵脉，并把它当成真实高灵气点使用：在该 runtime zone 里静坐修炼会按 `Zone.spirit_qi` 获得真元，固元突破会把它当成 0.8 门槛环境，炼丹起炉也会按该 zone 灵气判断是否允许。

结果是：服务器会周期性给地图凭空投放可修炼、可突破、可炼丹的高灵气窗口。玩家越会踩节奏、越会围绕伪灵脉打坐/冲境/起炉，越能把这些未从全服预算扣出的灵气变成自己的 `qi_current`。伪灵脉消散时，未被吸收的那部分又直接清零，导致真元经济既能凭空发奖，也会凭空销毁，破坏“修炼消耗 = 别人少掉”的核心体验。

## 证据定位

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

### P0 — 统一 heartbeat 自动伪灵脉入口

- 让 heartbeat 自动伪灵脉不再直接注册旧 `PseudoVeinRuntimeState` runtime zone。
- 优先复用 `world::pseudo_vein_runtime` 的 `PseudoVeinRuntime` component 与 `inject_zone_for_pseudo_vein` / settlement 路径。
- 若短期不能删旧 state，则旧 `spawn_pseudo_vein_from_omen` 至少必须拿到 `WorldQiAccount`，按 `pending_inflow_account -> zone` 真实借出后才能提高 `zone.spirit_qi`，并记录 `QiTransfer`。

### P1 — 收口旧衰减/消散语义

- 旧 `advance_active_pseudo_veins` 不能继续只按 `PseudoVeinRuntimeState.qi_current` 覆盖 `Zone.spirit_qi`。
- 消散时必须把未被玩家/NPC吸收的余额按 `QI_ZONE_UNIT_CAPACITY` 换算，转回 `pending_inflow_account`，等价于新 `PseudoVeinSettle` 语义。
- 链式事件 `PseudoVeinDissipated` 可保留，但 `redistributed_qi` 不能代替账本搬运。

### P2 — 删除或隔离旧世界生成 runtime

- 明确 `worldgen::pseudo_vein::PseudoVeinRuntimeState` 是 terrain/telemetry helper 还是生产 runtime。
- 如果不再作为生产 qi runtime 使用，移除 heartbeat 对它的依赖，避免下一次改动又绕回旧直写字段。
- 如果必须保留，补充注释说明它只计算展示/阶段，不拥有真元余额；真实余额以 `WorldQiAccount` 与 `Zone.spirit_qi` 同步路径为准。

## 验收测试计划

- 新增 heartbeat 自动路径单测：触发 `PseudoVeinForming` omen 前后，用 `summarize_world_qi` / `WorldQiAccount` 对拍，断言伪灵脉创建不会增加 `total_observed`，`pending_inflow_account` 按注入量下降。
- 新增旧回归测试：`spawn_pseudo_vein_from_omen` 或替代入口不得在无 `WorldQiAccount` 时把 `Zone.spirit_qi` 从低值直接抬高；缺账本应降级为零注入或拒绝生成真实高灵气。
- 新增消散测试：伪灵脉剩余未吸收余额消散后转回 `pending_inflow_account`，不能直接 `zone.spirit_qi = 0.0` 丢失。
- 新增玩家体验链路测试：玩家在 heartbeat 伪灵脉内修炼获得的 `qi_current` 必须对应 zone/pending pool 的等额减少，不能凭空增加 `total_observed`。
- 跑 server 栈：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。

## 风险

- #899 若先合并，会让 heartbeat 伪灵脉 runtime zone 被持久化；修本 bug 时要同时处理持久化字段中的已注入余额/借款额，否则重启后仍会出现“zone 恢复了，账本没恢复”的分叉。
- 直接切换到 `PseudoVeinRuntime` 可能影响 world heartbeat 的 omen/VFX/链式兽潮时序，需要保留 `PseudoVeinDissipated` 事件语义。
- `summarize_world_qi` 的 `zone_qi` 是分率口径，`WorldQiAccount` 是绝对量口径；测试需要沿用现有守恒测试的换算/对拍方式，避免把口径差误判成新 bug。
- 修复时不要把伪灵脉改成纯特效：它仍应是真实诱饵，只是必须从全服预算中借出并在消散时结算。
