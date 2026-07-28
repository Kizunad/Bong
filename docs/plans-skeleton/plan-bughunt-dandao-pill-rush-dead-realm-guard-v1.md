# plan-bughunt-dandao-pill-rush-dead-realm-guard-v1（骨架）

> **骨架（草案）**。一句话主题：删除 `resolve_pill_rush` 中对最低合法境界 `Realm::Awaken` 的恒假比较，保留“无 `Cultivation` 才按境界不足拒绝、六个合法境界均继续走经脉/真元门”的真实契约，并用边界测试防止死守卫回流。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 第一性验真 + 删除 `resolve_pill_rush` 的不可达 `RealmTooLow` 分支 | ⬜ |
| P1 | 六境界边界矩阵、无组件/经脉/真元回归与同类最低境界守卫审计 | ⬜ |

## 接入面

- **进料**：`server/src/dandao/mod.rs:60-63` 通过 `SkillRegistry` 注册 `DANDAO_PILL_RUSH_SKILL_ID`；`server/src/dandao/skills.rs:209-248` 的 `resolve_pill_rush` 读取施法者 `Cultivation`、`MeridianSeveredPermanent` 与真元余额。
- **出料**：合法施法继续返回 `CastResult::Started { cooldown_ticks, anim_duration_ticks }`；缺 `Cultivation`、经脉永久断裂或真元不足继续返回既有 `CastRejectReason`，不新增 event/schema。
- **共享类型 / event**：复用 `Cultivation`、`Realm`、`CastResult`、`CastRejectReason`、`MeridianSeveredPermanent`、`QiTransfer`；禁止为删除死代码另造 realm gate component/helper。
- **跨仓库契约**：纯 server resolver 清理；不改 agent schema、Redis key、proto 或 client handler。丹道三基础招的玩家技能栏/A/V 接线另归 `plan-bughunt-dandao-basic-skillbar-bridge-v1`，本 plan 不重复承接。
- **worldview 锚点**：`worldview.md §三 L67-L79` 固定六境界顺序并规定材料/丹药是强力辅助；`worldview.md §三 L136-L155` 规定丹药加速受丹毒限制。此修复不改变境界或丹药数值，只让代码与醒灵可用契约一致。
- **qi_physics 锚点**：`server/src/dandao/skills.rs:234-247` 仍由 `dandao_qi_cost_base` + `drain_dandao_qi` 扣费；`server/src/dandao/skills.rs:166-203` 仍生成 player→zone/overflow `QiTransfer`。不得因清理境界分支绕过或改写守恒路径。

## 第一性验真（`origin/main @ aea2d9dcc89c540795018e86c3eb55bc340adb58`，2026-07-29）

1. `server/src/cultivation/components.rs:16-23` 的 `Realm` 只有醒灵→化虚六个合法变体，`Awaken` 是最低变体；`Realm::rank()` 在 `server/src/cultivation/components.rs:58-69` 也明确醒灵为 1、其余依次递增。
2. `server/src/dandao/skills.rs:215-225` 已先用 `world.get::<Cultivation>(caster)` 拒绝无修为组件的实体；随后 `cultivation.realm as u8 < Realm::Awaken as u8` 对任一合法 `Realm` 都恒为 false，因此第二个 `RealmTooLow` 分支不可达。
3. `server/src/dandao/progression.rs:8-14` 明确 `Realm::Awaken` 即解锁 `dandao.pill_rush`，`server/src/dandao/progression.rs:145-148` 已有 `pill_rush_available_from_awaken` pin；该招不应提高到引气门槛。
4. `server/src/dandao/skills.rs:404-413` 已证明醒灵且真元充足时施法成功；`server/src/dandao/skills.rs:390-401` 证明缺 `Cultivation` 时仍以 `RealmTooLow` 拒绝。删除恒假比较不会放宽凡人、断脉或真元门。
5. 原 finding 成立但影响限于误导性死代码和未来维护风险：当前玩家可用结果没有被该分支错误拒绝。本 plan 不夸大为现有玩家施法故障，也不改变技能获得链路。

## P0 — 删除不可达境界守卫

- [ ] 在 `server/src/dandao/skills.rs::resolve_pill_rush` 删除 `cultivation.realm < Realm::Awaken` 的恒假分支；保留缺 `Cultivation` 的 `RealmTooLow` 拒绝作为凡人/坏实体边界。
- [ ] 不把门槛改成 `Realm::Induce`，不新增平行 `PILL_RUSH_MIN_REALM` 抽象；权威解锁表 `abilities_unlocked_at(Realm::Awaken)` 与既有成功测试已经说明最低合法境界就是醒灵。
- [ ] 保持后续检查顺序和行为：`check_meridian_dependencies(PILL_RUSH_MERIDIANS, ...)` → `dandao_qi_cost_base(cultivation.realm)` → `drain_dandao_qi` → `CastResult::Started`。
- [ ] 可核验 symbol：`resolve_pill_rush`、`pill_rush_succeeds_at_awaken`、`pill_rush_rejects_without_cultivation`。

**P0 测试声明**：`cd server && cargo test dandao::skills::tests::pill_rush`；至少覆盖无 `Cultivation` 拒绝、醒灵成功、真元不足、脾/肾经永久断裂、冷却、实际扣费与 `QiTransfer` 入 zone/overflow，现有测试不得因删分支弱化。

## P1 — 饱和边界与同类审计

- [ ] 新增 `pill_rush_accepts_every_valid_realm_before_resource_gates`（或同义 greppable 测试）：遍历 `Realm::{Awaken, Induce, Condense, Solidify, Spirit, Void}`，给足对应 `dandao_qi_cost_base(realm)` 且经脉完好，断言均进入 `CastResult::Started`；失败信息必须带 realm 与实际结果。
- [ ] 保留 `pill_rush_rejects_without_cultivation`，明确 `RealmTooLow` 只代表缺失修为组件，不再暗示存在低于醒灵的合法 `Realm`。
- [ ] 审计 `server/src/dandao/skills.rs` 其余基础招 realm gate：`resolve_pill_bomb` 的 `Realm::Induce` 与 `resolve_pill_mist` 的 `Realm::Condense` 均有真实低阶反例；只补必要的正好低一境/恰达门槛测试，不顺手改数值或 resolver。
- [ ] 加静态回归断言或等价 review gate，确保 `resolve_pill_rush` 不再比较 `Realm::Awaken` 下界；优先用行为矩阵锁契约，不以脆弱源码字符串测试替代行为测试。
- [ ] 可核验 symbol：`pill_rush_accepts_every_valid_realm_before_resource_gates`、`pill_bomb_rejects_below_induce`、`pill_mist_rejects_below_condense`。

**P1 测试声明**：`cd server && cargo test dandao::skills::tests`；最终 server gate 为 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。

## 范围边界

- 不修 `dandao.pill_*` 的 `KnownTechniques` / skill-bar / A/V/HUD/icon 断链；该主题已有 `plan-bughunt-dandao-basic-skillbar-bridge-v1`。
- 不调整三招境界、真元费用、经脉依赖、冷却、动画时长或伤害/增益数值。
- 不改 `Realm` 枚举判别值，不引入“凡人境”或第七境界。
- 本 plan 是纯 server 逻辑清理，不需要新增客户端视听资产；既有玩家可感知行为应 bit-for-bit 不变。

## §8 开放问题（P0 决策门前需收口）

1. P1 是否需要源码形态 lint，还是六境界行为矩阵已足以防止恒假下界比较回流？推荐只保留行为矩阵，避免测试绑死实现文本。
2. `RealmTooLow` 用于“缺 `Cultivation`”是否需要另拆更精确拒绝原因？推荐本 plan 不改 wire/UI 枚举，避免把纯死代码清理扩大成协议变更。
