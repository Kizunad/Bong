# plan-bughunt-npc-daily-life-qi-mint-v1

> Skeleton Plan / BugHunt server-qi r02 / 2026-07-07

## 0. 接入面

- 进料：`NpcDailySchedule` / `ReturnHomeAction` / `RestAction` / `GoToPoiAction` / `NpcLodTier::Far` / `ZoneRegistry` / `WorldQiAccount` / `Cultivation`。
- 出料：NPC 休息、到达 `QiSpring`、Far LOD 修炼时的 `qi_current` 回复必须走标准 `CultivationRegen` 守恒账。
- worldview 锚点：`worldview.md §一 L17-L20`、`worldview.md §十 L872-L876`，灵气总量固定、修炼消耗区域灵气。
- qi_physics 锚点：`docs/CLAUDE.md §四 L58-L60`；`qi_physics::excretion::regen_from_zone`；`qi_physics::ledger::{WorldQiAccount, QiTransfer, QiTransferReason::CultivationRegen}`。
- 非重复说明：不是 #1050 craft `qi_cost` 固定落 `zone:spawn`；本缺陷只覆盖 NPC daily-life 的休息 / QiSpring 到达 / Far LOD 修炼回气旁路。

## 1. 实际游玩体验影响

玩家正常游玩中会看到散修夜里回家休息、低真元时躲回据点、到灵泉点打坐，或在远离玩家后进入 Far LOD 继续日程推演。当前这些路径可以让 NPC `qi_current` 回升，但对应区域 `zone.spirit_qi` 不下降，`WorldQiAccount` 也没有 `CultivationRegen` 审计。

结果是 NPC 比玩家多了一条免费回气渠道：玩家在同一区域修炼会吸干灵气、改变灵地贫瘠反馈，而 NPC 休息/远场修炼不会让灵地变差。长期看，散修续航、突破准备和战后恢复都会偏向 NPC，破坏末法残土“你修炼消耗的灵气就是别人少掉的灵气”的资源争夺体验。

## 2. 复现路径

1. Near 休息：生成带 `NpcDailySchedule` / `NpcHomeBase` / `NpcPatrol` / `Cultivation` 的散修，`qi_current=0`，区域 `spirit_qi` 记录为 A，触发 `RestAction` 或 `ReturnHomeAction` 到家后跑满休息 tick。
   - 期望：NPC 回气量通过 `regen_from_zone` 从 home zone 扣除，并留下 `QiTransferReason::CultivationRegen`。
   - 实际：`qi_current` 增加，zone 字段和 ledger 不随之扣减。
2. Far LOD 修炼：生成 `NpcLodTier::Far` 的散修，日程命中 `ScheduleActivity::Cultivate`，记录 home zone `spirit_qi` 与 `WorldQiAccount`。
   - 期望：Far 简化推演仍遵守 zone -> NPC 的守恒搬运。
   - 实际：`far_activity_tick` 只把 `zone_qi` 当倍率，直接给 NPC 加真元。
3. QiSpring 到达：让 `GoToPoiAction` 抵达 `QiSpring`，首个 arrival tick 调 `finish_poi_arrival(ScheduleActivity::Cultivate)`。
   - 期望：按已归档 plan 描述转入 `CultivateAction` 或走同源守恒 helper。
   - 实际：一次性 `recover_current_qi(qi_max * 0.02)`，无 zone 扣减。

## 3. 根因证据

- `server/src/cultivation/components.rs:401` 的 `recover_current_qi` 只是组件加法和上限 clamp，本身不做 zone/ledger。
- 标准回气路径在 `server/src/cultivation/tick.rs:91` 标注零和，`server/src/cultivation/tick.rs:248` 通过 `compute_regen` 得出 gain/drain，`server/src/cultivation/tick.rs:268` 缺 `WorldQiAccount` 直接跳过，`server/src/cultivation/tick.rs:272` 追加 `CultivationRegen`，`server/src/cultivation/tick.rs:282` 同 tick 增加 `qi_current` 并扣 `zone.spirit_qi`。
- `server/src/npc/brain/actions_life.rs:666` 和 `server/src/npc/brain/actions_life.rs:725` 在 `ReturnHomeAction` / `RestAction` 中每 tick 调 `rest_tick`。
- `server/src/npc/schedule.rs:249` 的 `rest_tick` 直接调用 `recover_current_qi`。结合 `server/src/npc/brain/mod.rs:147` 的 `REST_MAX_TICKS = 20 * 120`，完整休息足以把低真元 NPC 拉回大量真元，不是装饰性数值。
- `server/src/npc/schedule.rs:382` 的 Far 系统会真实运行；`server/src/npc/schedule.rs:277` 的 `Cultivate` 分支直接 `recover_current_qi(cultivation, zone_qi.max(0.0) * 0.01)`。
- `server/src/npc/brain/actions_life.rs:433` 到达 POI 首 tick 调 `finish_poi_arrival`；`server/src/npc/brain/actions_life.rs:486` 的 `Cultivate` 分支一次性 `recover_current_qi(cultivation, cultivation.qi_max * 0.02)`。
- `docs/finished_plans/plan-npc-daily-life-v1.md:37` 写明 daily-life “不涉及 qi_physics（NPC 修炼吸收灵气已在 CultivateAction 中走 qi_physics）”，但实现里 P2/P3 日程层新增了直接回气旁路。
- `server/src/npc/lod.rs:262` / `server/src/npc/lod.rs:357` 已有 Mid/Drowsy NPC 用 ledger transfer + zone 写回的范本，说明背景 NPC 回气没有设计豁免。

## 4. 修复计划骨架

### P0：统一 NPC 日程回气 helper

- 新增/抽取一个 server-only helper，输入 NPC entity、当前位置或 home zone、期望 rate、`ZoneRegistry`、`WorldQiAccount`、`Cultivation`。
- helper 复用 `regen_from_zone` 的 gain/drain 换算，成功时追加 `QiTransferReason::CultivationRegen`，同步 `cultivation.qi_current += gain` 与 `zone.spirit_qi -= drain`。
- 缺 `WorldQiAccount`、zone 不存在、zone_qi <= 0、room <= 0 时不回气，保持标准 `qi_regen_and_zone_drain_tick` 语义。

### P1：收敛 Near 休息路径

- `rest_tick` 保留 hunger 回复纯函数能力，移除直接 qi 增量，或改为返回“请求回气量”由 action system 走 P0 helper。
- `ReturnHomeAction` / `RestAction` 使用 home zone 真实扣账；低真元触发回家仍成立，但不再免费补真元。

### P2：收敛 Far LOD 和 QiSpring 到达路径

- `far_npc_schedule_tick_system` 对 `ScheduleActivity::Cultivate` 走 P0 helper，而不是直接调用 `far_activity_tick` 加真元。
- `finish_poi_arrival(ScheduleActivity::Cultivate)` 不再直接加 `qi_max * 0.02`；优先转入/触发 `CultivateAction`，如需一次性到达奖励也必须通过 P0 helper 从实际 zone 扣账。

### P3：饱和化测试

- Near 休息：正灵区完整休息后 `qi_current` 增量、`zone.spirit_qi` 降幅、`CultivationRegen` audit 三者精确匹配。
- Near 休息：缺 ledger / zone_qi <= 0 / qi room=0 时不回气。
- Far LOD：`NpcLodTier::Far + Cultivate` 不得凭空加真元，必须扣 home zone。
- QiSpring：到达 `QiSpring` 不得绕过 ledger；若切入 `CultivateAction`，断言首 tick 无瞬时免费 qi。
- 回归：`qi_regen_and_zone_drain_tick`、dormant/lod 既有守恒测试不退化。

## 5. 验证计划

- server 单测：新增 `npc::schedule`、`npc::brain::actions_life` 相关守恒测试，覆盖 happy path、负/零 zone、缺 ledger、满 qi、Far LOD、QiSpring 到达。
- server 命令：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。
- 联调：带 `BONG_SKIP_SKIN_PREFETCH=1` 跑 `bash scripts/smoke-test-e2e.sh`，观察散修夜间休息/灵泉修炼后 zone qi telemetry 有对应下降。
- Bot e2e 后续：如已有可观察 dev 命令，补一个 NPC daily-life 场景；若不可观察，先补 `bong:qi/ledger`/zone 信息观测入口再断言。

## 6. 对抗复核结论

- 候选证据：daily-life 三个生产调用点直接 `recover_current_qi`，对比标准 `qi_regen_and_zone_drain_tick` 少了 zone 扣减和 `WorldQiAccount` 审计。
- 反方质疑：`recover_current_qi` helper 本身无罪；Near NPC 已有标准回气主链；Far tick 频率较低且受正 zone_qi 门控；休息不是所有 NPC 自动触发。
- 修正/反驳：候选收窄为 NPC daily-life 的休息 / Far Cultivate / QiSpring arrival 三个旁路；按 `REST_MAX_TICKS` 计算，休息回气规模足以影响战斗和突破；既有 LOD 守恒实现证明背景回气不应豁免。
- 最终裁决：成立。定级为 NPC daily-life 生产路径的 major/high-risk 守恒漏洞；不是全局 critical，但必须修复后才能声称 NPC 日程与玩家修炼同守恒口径。
