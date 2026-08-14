# plan-bughunt-dandao-pill-rush-dead-realm-guard-v1（骨架）

> **骨架（草案）**。一句话主题：删除 `resolve_pill_rush` 中对最低合法境界 `Realm::Awaken` 的恒假比较，保留“无 `Cultivation` 才按境界不足拒绝、六个合法境界均继续走经脉/真元门”的真实契约，并用边界测试锁住该契约。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 第一性验真 + 删除 `resolve_pill_rush` 的不可达 `RealmTooLow` 分支 | ⬜ |
| P1 | 六境界边界矩阵、无组件/经脉/真元回归与同类最低境界守卫审计 | ⬜ |

## 接入面

- **进料**：`server/src/cultivation/mod.rs:251-255` 在生产插件启动时插入 `skill_registry::init_registry()`；该统一初始化器在 `server/src/cultivation/skill_registry.rs:104-122` 调用 `crate::dandao::register_skills`，再由 `server/src/dandao/mod.rs:60-63` 把 `DANDAO_PILL_RUSH_SKILL_ID` 映射到 `resolve_pill_rush`。玩家公开入口虽由 `ClientRequestV1::SkillBarCast` 在 `server/src/network/client_request_handler.rs:2233-2244` 进入 `handle_skill_bar_cast`，但 `:14243-14260` 会先经过 `technique_definition` / `KnownTechniques` 门，之后才在 `:14356-14359,14441-14469` 执行 `SkillRegistry::lookup` 与 `SkillFn`；丹道三基础招当前缺 definition，故玩家入口尚不能到达 resolver。
- **出料**：本 plan 只锁 production `init_registry()` 查表后直接调用 `SkillFn` 的 server resolver 契约：`server/src/dandao/skills.rs:209-248` 读取施法者 `Cultivation`、`MeridianSeveredPermanent` 与真元余额；合法施法返回 `CastResult::Started { cooldown_ticks, anim_duration_ticks }`，缺 `Cultivation`、经脉永久断裂或真元不足返回既有 `CastRejectReason`。玩家 `SkillBarCast` 的 definition/ownership 断点及其 `CastSyncV1` 端到端闭环明确留给既有 bridge plan，本 plan 不伪造当前不存在的可达链。
- **共享类型 / event**：复用 `Cultivation`、`Realm`、`CastResult`、`CastRejectReason`、`MeridianSeveredPermanent`、`QiTransfer`；禁止为删除死代码另造 realm gate component/helper。
- **跨仓库契约**：纯 server resolver 清理；不改 agent schema、Redis key、proto 或 client handler。丹道三基础招的玩家技能栏/A/V 接线另归 `plan-bughunt-dandao-basic-skillbar-bridge-v1`，本 plan 不重复承接。
- **worldview 锚点**：`worldview.md §三 L67-L79` 固定六境界顺序并规定材料/丹药是强力辅助；`worldview.md §三 L136-L155` 规定丹药加速受丹毒限制。此修复不改变境界或丹药数值，只让代码与醒灵可用契约一致。
- **qi_physics 锚点**：`server/src/dandao/skills.rs:234-247` 仍由 `dandao_qi_cost_base` + `drain_dandao_qi` 扣费；`server/src/dandao/skills.rs:73-119,120-203` 仍完成 qi debit、`qi_release_to_zone` zone credit 与 overflow/audit `QiTransfer`。不得因清理境界分支绕过或改写守恒路径。

## 第一性验真（`origin/main @ aea2d9dcc89c540795018e86c3eb55bc340adb58`，2026-07-29）

1. `server/src/cultivation/components.rs:16-23` 的 `Realm` 只有醒灵→化虚六个合法变体，`Awaken` 是最低变体；`Realm::rank()` 在 `server/src/cultivation/components.rs:58-69` 也明确醒灵为 1、其余依次递增。
2. `server/src/dandao/skills.rs:215-225` 已先用 `world.get::<Cultivation>(caster)` 拒绝无修为组件的实体；随后 `cultivation.realm as u8 < Realm::Awaken as u8` 对任一合法 `Realm` 都恒为 false，因此第二个 `RealmTooLow` 分支不可达。
3. `server/src/dandao/progression.rs:8-14` 明确 `Realm::Awaken` 即解锁 `dandao.pill_rush`，`server/src/dandao/progression.rs:145-148` 已有 `pill_rush_available_from_awaken` pin；该招不应提高到引气门槛。
4. `server/src/dandao/skills.rs:404-413` 已证明醒灵且真元充足时施法成功；`server/src/dandao/skills.rs:390-401` 证明缺 `Cultivation` 时仍以 `RealmTooLow` 拒绝。删除恒假比较不会放宽凡人、断脉或真元门。
5. 该 resolver 已进入生产 registry，但玩家公开入口存在另一个已登记断点：`server/src/cultivation/mod.rs:254` 把 production `init_registry()` 插入 App，`server/src/cultivation/skill_registry.rs:104-122` 把丹道注册纳入统一表；然而 `server/src/network/client_request_handler.rs:14243-14260` 在 registry lookup 前要求 `technique_definition` 与 `KnownTechniques`，而 `server/src/cultivation/known_techniques.rs:166-1115` 当前 49 条 `TECHNIQUE_DEFINITIONS` 无 `dandao.pill_rush`。`docs/plans-skeleton/plan-bughunt-dandao-basic-skillbar-bridge-v1.md:17-42,66-85` 已把 definition/ownership/A/V 与 network e2e 登记为唯一 owner；本 plan 只能验证 production registry→resolver，不能宣称已验证玩家技能栏运行行为。
6. 原 finding 成立但影响限于误导性死代码和未来维护风险：当前 registry resolver 的直接调用结果没有被该分支错误拒绝。本 plan 不夸大为现有玩家施法故障，也不改变技能获得链路。

## P0 — 删除不可达境界守卫

- [ ] 在 `server/src/dandao/skills.rs::resolve_pill_rush` 删除 `cultivation.realm < Realm::Awaken` 的恒假分支；保留缺 `Cultivation` 的 `RealmTooLow` 拒绝作为凡人/坏实体边界。
- [ ] 不把门槛改成 `Realm::Induce`，不新增平行 `PILL_RUSH_MIN_REALM` 抽象；权威解锁表 `abilities_unlocked_at(Realm::Awaken)` 与既有成功测试已经说明最低合法境界就是醒灵。
- [ ] 保持后续检查顺序和行为：`check_meridian_dependencies(PILL_RUSH_MERIDIANS, ...)` → `dandao_qi_cost_base(cultivation.realm)` → `drain_dandao_qi` → `CastResult::Started`。
- [ ] 可核验 symbol：`resolve_pill_rush`、`pill_rush_succeeds_at_awaken`、`pill_rush_rejects_without_cultivation`。

**P0 测试声明**：`cd server && cargo test dandao::skills::skill_tests`；该真实模块（`server/src/dandao/skills.rs:337-338`）至少覆盖无 `Cultivation` 拒绝、醒灵成功、真元不足、脾/肾经永久断裂、冷却、实际扣费，以及 `drain_qi_no_position_routes_to_overflow_not_destroyed` / `qi_transfer_to_account_is_zone_kind_when_in_zone` 两条 zone/overflow 守恒分支；现有测试不得因删分支弱化。

## P1 — 饱和边界与同类审计

- [ ] 新增 `pill_rush_accepts_every_valid_realm_before_resource_gates`（或同义 greppable 测试）：遍历 `Realm::{Awaken, Induce, Condense, Solidify, Spirit, Void}`，给足对应 `dandao_qi_cost_base(realm)` 且经脉完好，断言均进入 `CastResult::Started`；失败信息必须带 realm 与实际结果。
- [ ] 保留 `pill_rush_rejects_without_cultivation`，明确 `RealmTooLow` 只代表缺失修为组件，不再暗示存在低于醒灵的合法 `Realm`。
- [ ] 审计 `server/src/dandao/skills.rs` 其余基础招 realm gate：`resolve_pill_bomb` 的 `Realm::Induce` 与 `resolve_pill_mist` 的 `Realm::Condense` 均有真实低阶反例；只补必要的正好低一境/恰达门槛测试，不顺手改数值或 resolver。
- [ ] 不新增源码字符串测试：六境界行为矩阵锁定外部契约，恒假比较本身由删除后的 diff review 与 `cargo clippy --all-targets -- -D warnings` 守门；本 plan 不宣称行为测试能检测一个行为中性的死比较重新出现。
- [ ] 在 `server/src/cultivation/skill_registry.rs`（或紧邻丹道测试）新增 `production_registry_dispatches_pill_rush_at_awaken`（或同义 greppable 测试）：必须调用 production `init_registry()` 后以 `DANDAO_PILL_RUSH_SKILL_ID` 执行 `SkillRegistry::lookup`，再调用返回的 `SkillFn`；禁止在测试内手工注册或直接调用 `resolve_pill_rush`。醒灵、经脉完好、真元充足时断言 `CastResult::Started` 与真元按 `dandao_qi_cost_base(Realm::Awaken)` 扣除，证明统一 registry 确实加载并消费该 resolver。
- [ ] 同一 production-registry dispatch fixture 增加缺 `Cultivation` 拒绝 case，断言 lookup 仍命中且返回 `CastRejectReason::RealmTooLow`；这只锁 server registry→resolver，不绕过 `technique_definition` 去伪造当前不可达的玩家 `SkillBarCast` e2e。
- [ ] 可核验 symbol：`skill_registry::init_registry`、`DANDAO_PILL_RUSH_SKILL_ID`、`SkillRegistry::lookup`、`production_registry_dispatches_pill_rush_at_awaken`、`pill_rush_accepts_every_valid_realm_before_resource_gates`、`pill_bomb_rejects_below_induce`、`pill_mist_rejects_below_condense`。

**P1 测试声明**：`cd server && cargo test dandao::skills::skill_tests` 与 `cd server && cargo test cultivation::skill_registry::tests::production_registry_dispatches_pill_rush`；后一个过滤器必须实际运行 production-registry 的成功/拒绝 case，禁止零测试假绿。最终 server gate 为 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。

## 范围边界

- 不修 `dandao.pill_*` 的 `KnownTechniques` / `TECHNIQUE_DEFINITIONS` / skill-bar / A/V/HUD/icon 断链；该主题已有 `plan-bughunt-dandao-basic-skillbar-bridge-v1`，且当前 `handle_skill_bar_cast` 会在 `SkillRegistry::lookup` 前因缺 definition 拒绝。只有该 owner 合入后，玩家 `ClientRequestV1::SkillBarCast` e2e 才成为可执行验收；本 plan 不复制或提前消费它。
- 不调整三招境界、真元费用、经脉依赖、冷却、动画时长或伤害/增益数值。
- 不改 `Realm` 枚举判别值，不引入“凡人境”或第七境界。
- 本 plan 是纯 server 逻辑清理，不需要新增客户端视听资产；既有玩家可感知行为应 bit-for-bit 不变。

## §8 开放问题（P0 决策门前需收口）

1. P1 是否需要源码形态 lint？推荐不为单个恒假比较引入源码字符串测试；六境界矩阵只锁行为契约，代码形态由 clippy 与实施 diff review 负责，验收证据不得把两者混称。
2. `RealmTooLow` 用于“缺 `Cultivation`”是否需要另拆更精确拒绝原因？推荐本 plan 不改 wire/UI 枚举，避免把纯死代码清理扩大成协议变更。

以上问题全部已在 §8.1 收口；实施时以 §8.1 决议为准。

## §8.1 决议（pre-P0 收口，2026-08-14）

### #1 不增加源码形态 lint

**决议**：
1. 不为单个恒假比较新增源码字符串测试；代码形态由删除该分支的 diff review 与 clippy 共同核验。
2. 六境界矩阵只声明并锁定外部行为：所有合法 `Realm` 均继续进入既有经脉与真元门。
3. 验收证据分别陈述行为测试与静态门禁，不把六境界矩阵误称为死代码回归 lint。

**落点**：`server/src/dandao/skills.rs:209-248`；plan P0、P1。

### #2 保留既有 RealmTooLow wire 语义

**决议**：
1. 缺 `Cultivation` 时继续返回 `CastRejectReason::RealmTooLow`。
2. 不新增拒绝枚举、不调整 `CastOutcomeV1` 映射，也不触碰 client/agent 协议。
3. 用 production registry dispatch 的缺组件用例锁住现有 server 契约。

**落点**：`server/src/dandao/skills.rs:215-225`、`server/src/cultivation/skill_registry.rs:104-122`；plan P0、P1。
