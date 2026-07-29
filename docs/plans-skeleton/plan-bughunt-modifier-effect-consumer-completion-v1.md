# plan-bughunt-modifier-effect-consumer-completion-v1（骨架）

> **骨架（草案）**。一句话主题：把 r6/r7/r8 已确认的 11 条 canonical finding 收束成可实施的 modifier/effect 消费闭环：先建立防孤岛 manifest，再按 alchemy/Insight、活茧伤口与 effective flow、jump 跨端三组接通真实 gameplay consumer，并以可观察差分测试证明“写入字段”不再只是 storage/HUD/test-only 孤岛。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | canonical consumer manifest + 11 条 finding / live 字段完整性门禁 | ⬜ |
| P1 | Alchemy effect 极性 + `InsightModifiers` 按 gameplay loop 分批消费 | ⬜ |
| P2 | Iron Cocoon wound-grade + ScarForged effective-flow 消费闭环 | ⬜ |
| P3 | `jump_height_multiplier` 唯一权威端 + 可观察 runtime 闭环（按选定路线互斥验收） | ⬜ |

## 接入面

- **进料**：alchemy 由 `server/src/alchemy/side_effect_apply.rs:20-83` 与 `server/src/alchemy/pill.rs:632-642` 生产 `StatusEffectKind`；顿悟由 `server/src/cultivation/insight_apply.rs:24-254` 写 `InsightModifiers`；活茧由 `server/src/combat/baomai_v4/iron_cocoon.rs:99-143` 写四个 `DerivedAttrs` 字段；广播体操由 `server/src/combat/body_conditioning.rs:157-167` 写 `jump_height_multiplier`。
- **出料**：污染排异 `cultivation::contamination_tick`、回气 `cultivation::qi_regen_and_zone_drain_tick`、突破/过载/经脉恢复/颜色与涡流 gameplay loop、伤口结算 `combat::resolve_attack_intents`、有效经脉流率、以及真实跳跃物理必须读取对应 modifier；单纯持久化、HUD、schema、client store 或测试内读取不算 gameplay consumer。
- **共享类型 / event**：复用 `StatusEffects`、`StatusEffectKind`、`InsightModifiers`、`DerivedAttrs`、`MeridianSystem`、`ActiveScarCircuits`、`Wound`/`WoundKindProfile`、`DerivedAttrsSyncV1`；禁止另造 parallel modifier component 或持久改写派生倍率。
- **跨仓库契约**：P0-P2 以 server 为主；P3 若采用 client jump hook，必须同 PR 扩 `DerivedAttrsSyncV1` → proto/generated → `DerivedAttrsHandler` / `DerivedAttrsStore` → 非 mixin 包 helper + jump input/mixin，并锁断线 reset。若采用 server-authoritative velocity，则仍需 server movement/e2e 证明客户端实际高度变化。
- **worldview 锚点**：`worldview.md §四 L250-L260`（伤口档次与护甲后果）、`§四 L275-L288`（经脉流量）、`§四 L344-L351`（污染排异亏损）、`§五 L401-L405`（爆脉体修）；顿悟收益/代价必须进入日常 gameplay loop，不能只留文字与存档。
- **qi_physics 锚点**：回气继续走 `qi_physics::excretion::regen_from_zone` / `WorldQiAccount` ledger，污染排异继续走 `release_qi_amount_to_zone` / `QiTransfer`；modifier 只能改变 canonical rate/threshold/cost，禁止直接无对端修改 `qi_current` 或 zone qi。`scar_forged_flow_bonus` 只改变 effective rate，不持久改 `Meridian.flow_rate`。

## Canonical Finding Mapping

当前 owner 以归档文末 Mapping 为准，r8 audit 是细化而不是第二套 finding：

| 来源 | Canonical finding | 本 plan 阶段 |
|---|---|---|
| r6 #0 | `ContaminationBoost` gameplay consumer 缺失 | P1 |
| r6 #1 | JinZhongDan negative slot 写正向 `QiRegenBoost` | P1 |
| r7 #3-#7 | `qi_regen_mul`、`next_breakthrough_bonus`、三个 vortex 字段 | P1 |
| r8 #2 | `bruise_threshold_multiplier` / `fracture_downgrade_chance` / `cut_pierce_downgrade` | P2 |
| r8 #3 | `scar_forged_flow_bonus` | P2 |
| r8 #5 | 其余 live `InsightModifiers` benefit/cost cluster | P1 |
| r8 #4 | `jump_height_multiplier` | P3 |

P0 anti-orphan manifest/lint 是本 skeleton 的共享实施门，但不是第 12 条 finding。

## 第一性验真（`origin/main @ de75f14e43daf1105ea978c43d187acbb7f12f14`，2026-07-29）

1. **Alchemy effect 仍断链**：`server/src/alchemy/side_effect_apply.rs:20-83` 把可达 `contam_boost` 变成带 magnitude/duration 的 `ContaminationBoost`；`server/src/combat/status.rs:17-58` 会 upsert，HUD 也会显示，但 `server/src/cultivation/contamination.rs:97-205` 的 query/公式不读 `StatusEffects`。它是“可生产、可显示、会到期，但不改变污染物理”的 behavioral orphan。
2. **金钟丹极性仍错**：`server/src/alchemy/pill.rs:632-642` 的 negative duration 仍 push 正向 `QiRegenBoost(0.001)`；`server/src/cultivation/tick.rs:223-226,445-455` 现有 consumer 按 `1 + magnitude` 增益回气。因此本项只修 negative kind/极性，不重做通用 `QiRegenBoost` consumer。
3. **r7 五字段仍只有 producer/storage**：`server/src/cultivation/insight_apply.rs:25-40,123-191` 写 `qi_regen_mul`、`next_breakthrough_bonus` 与三个 vortex 字段；回气 query `server/src/cultivation/tick.rs:96-132` 不含 `InsightModifiers`，breakthrough/woliu 主循环也不读这些字段。选择会真实发生并持久化（`cultivation/insight_flow.rs:251-344`、`persistence/mod.rs:6193-6229`），但 gameplay 数值不变。
4. **P1 live Insight 簇仍断链**：`server/src/cultivation/insight_apply.rs:24-254` 的 live benefit/cost 字段中，除阵法两字段已有 consumer、`composure_recover_mul` 有 sibling direct effect 外，当前生产读取 grep 仅命中 reset/fixture/offer schema。`generic_talents.json:173-188` 虽定义 `vortex_burst_damage` cost，且可转换为 `insight_apply.rs:248-251` 的 `practices` marker，但当前 `color_affinity.rs:76-105,138-165` 每 alignment 只选第一个 valid candidate，使它被更早的同 affinity 候选遮蔽，production offer 不可达；须登记 dormant selector-shadowed，不能误纳 live consumer scope。`observe_chance_bonus` 同样仅 default，`technique_observe.rs:64-134` 的 reader 上层无 production caller，必须登记 dormant，不可误报已闭环。
5. **活茧四字段仍只写不读**：`server/src/combat/baomai_v4/iron_cocoon.rs:99-143` 写三项 wound 字段与 `scar_forged_flow_bonus`；全仓其它命中只在 producer 单测。`server/src/cultivation/components.rs:522-524` 的 `MeridianSystem::sum_rate` 只加持久 `flow_rate`；`combat/resolve.rs` 也不读三项 wound 字段。
6. **jump 仍是跨栈 orphan**：`server/src/combat/body_conditioning.rs:157-167` 写入、`combat/status.rs:163-175` reset，但 `schema/combat_hud.rs:209-220`、`network/derived_attrs_emit.rs:76-90`、`proto/bong/envelope.proto:1706-1718`、client `DerivedAttrsStore` 和跳跃输入链均无该字段。兄弟字段 `move_speed_multiplier` 已消费，不能代替 jump consumer。
7. **P0 gate 仍不存在**：`server/src/test_coverage_guards.rs` 现有源码扫描面向 `EventWriter<T>`/`EventReader<T>`；仓库无 `MODIFIER_CONSUMER_MANIFEST` / `modifier_consumer_manifest_stays_current`。它无法区分 gameplay、HUD/persistence-only、test-only、shadow direct effect 或 dormant producer。

## P0 — Modifier/effect consumer contract（先于 gameplay PR）

- [ ] 建立 greppable `ModifierConsumerContract` / `MODIFIER_CONSUMER_MANIFEST`（名称可等义），以 Rust AST、唯一声明宏或同等可机械穷举源提取**完整** `DerivedAttrs` 字段、`InsightModifiers` 字段与 `StatusEffectKind` variant 集合，再与 manifest 做精确一一对账；展示/存储/internal 项也必须逐项显式分类并写理由，禁止在库存生成前用“非展示”或“gameplay”语义预筛选。新增、删除或改名未同步登记即 gate 失败。
- [ ] 每条登记明确 producer、production consumer、observable differential test 与分类：`Gameplay`、`ShadowDirectEffect`、`DisplayStorageOnly`、`DormantNoProducer`、`DormantSelectorShadowed`、`PlannedNoConsumer`。后三类必须写 owner、阻塞原因和解除条件；禁止把 HUD/schema/persistence/store/test-only read 填成 consumer。`PlannedNoConsumer` 只允许作为实施中的临时红色状态，不是可归档豁免。
- [ ] P0 必须登记 11 条 canonical finding 及 P1 的完整 live Insight 清单；短期设计未决字段可临时 `PlannedNoConsumer`，但不得静默遗漏：`qi_regen_mul`、`next_breakthrough_bonus`、`vortex_backfire_resist_mul`、`vortex_delta_bonus_add`、`vortex_flow_speed_mul`、`hunyuan_threshold_mul`、`chaotic_tolerance_add`、`overload_tolerance_add`、`opposite_color_efficiency_penalty`、`qi_volatility_add`、`shock_sensitivity_add`、`main_color_efficiency_penalty`、`overload_fragility_add`、`reaction_window_penalty`、`breakthrough_failure_penalty_mul`、`sense_exposure_add`、`meridian_heal_slowdown_mul`、`chaotic_tolerance_loss`。
- [ ] `InsightModifiers.practices` 必须收束到 typed enum/newtype registry 与唯一构造/解析入口；参数化 marker 使用结构化编码，禁止 production 直接插入任意字符串。门禁从该 registry 穷举全部 key/prefix 并逐 key 做 reachability 分类；未知、脏值、重复值与旧存档 key 必须有 fail-closed 或显式 migration 策略及 pin test，不能用一个 `HashSet` 字段条目掩盖新孤岛 marker。
- [ ] 已有真消费/特殊分类须显式防重复：`zhenfa_concealment`、`zhenfa_disenchant` 为 `Gameplay`；`composure_recover_mul` 为 `ShadowDirectEffect`（真实 `Cultivation.composure_recover_rate` 被消费）；`observe_chance_bonus` 为 `DormantNoProducer`；`woliu:vortex_burst_damage_mul:*` 为 `DormantSelectorShadowed`，除非同 PR 先补 production selector reachability，再新增 typed consumer 与差分测试。
- [ ] `Gameplay` 合同不能靠文本 grep 自证：每条必须绑定可 grep 的 production caller/system schedule 与 gameplay differential test；helper 只有 tests caller 必须失败。可核验测试：`modifier_consumer_manifest_stays_current`、`modifier_contract_rejects_unregistered_inventory_member`、`modifier_contract_rejects_storage_test_or_dead_helper_consumer`、`practices_registry_rejects_unknown_or_direct_string_marker`、`planned_modifier_contract_requires_owner_reason_and_exit_condition`。fixture 覆盖缺字段、未登记新字段/variant、仅测试引用、仅 HUD/serialization 引用、生产文件 helper read 但无 caller（仿 `evaluate_observe_attempt`）与未知 practices key；manifest 不能取代后续 gameplay 差分测试。

## P1 — Alchemy + Insight gameplay consumers

### P1a Alchemy effect / polarity

- [ ] pre-P0 决议先冻结 `ContaminationBoost.magnitude` 语义：一次性新增 `ContamSource`，或有效期内持续增加污染压力/降低 purge；只能二选一并写清 refresh/stack/expiry。接线不得绕过 contamination ledger、排异真元成本、crack 与死亡判据。
- [ ] 修正 JinZhongDan negative slot：选现有 `QiRegenSlowed` 或新增语义明确的负面 kind；强度必须乘 `neg_scale`，覆盖 0/默认/max scale 与到期恢复 neutral。禁止给 `QiRegenBoost` 传负 magnitude 依赖隐式极性。
- [ ] 完整链测试从配方/丹药效果生产到 `StatusEffects` 再到真实污染或回气差分；覆盖 inactive/active/expired、duration=0、重复 upsert、magnitude 边界、污染空/非空、qi 足/不足与 zone 回灌守恒。
- [ ] 可核验 symbol：`ContaminationBoost`、`contamination_tick`、`CombatPillKind::JinZhongDan`、`QiRegenSlowed`、`contamination_boost_changes_runtime_pressure`、`jinzhongdan_negative_slot_reduces_regen_until_expiry`。

### P1b Insight 按 gameplay loop 序列化

- [ ] **PR-Insight-A（回气/突破）**：接 `qi_regen_mul`、`next_breakthrough_bonus`、`breakthrough_failure_penalty_mul`；真实 choice→component→tick/breakthrough 流程对拍，明确 bonus 的一次性消费时点。回气 gain/drain 必须继续 ledger 守恒。
- [ ] **PR-Insight-B（颜色/混元/杂色）**：接 `hunyuan_threshold_mul`、`chaotic_tolerance_add/loss` 与两个 color penalty。现有 aggregate penalty 丢失受罚颜色 identity，实施前必须先改成能表达目标颜色的持久化形状并补 migration；不能把惩罚全局化。
- [ ] **PR-Insight-C（过载/经脉恢复）**：接 `overload_tolerance_add`、`overload_fragility_add`、`meridian_heal_slowdown_mul`；阈值、严重度、恢复时长分别做 neutral/benefit/cost 差分，避免同一 modifier 在 detection 与 event-reader 双算。
- [ ] **PR-Insight-D（涡流）**：接 `vortex_backfire_resist_mul`、`vortex_delta_bonus_add`、`vortex_flow_speed_mul`；先定义 flow speed 是 cast ticks、吸收 dt 还是维持周期，所有 Woliu 生产路径必须共用同一 effective helper。`woliu:vortex_burst_damage_mul:*` 当前 selector-shadowed，不是本批 live consumer 交付；只有先用 production offer 测试证明其可达，才允许同 PR 将 marker 迁成 typed cost、接真实爆发伤害并覆盖缺失/单值/累乘/脏值。
- [ ] **PR-Insight-E（感知/反应/冲击/波动）**：`sense_exposure_add`、`reaction_window_penalty`、`shock_sensitivity_add`、`qi_volatility_add` 在 pre-P0 找到或建立 canonical gameplay seam；不得为消除 manifest 红线随意挂错系统。若决议确认任一字段不应存在，须在本 plan 内删除/禁用其 production producer 并迁移旧状态；若确因外部领域依赖不能在本 plan 交付，须先用独立 docs PR 更新 canonical Finding Mapping，建立带 owner、阶段和可核验验收的 successor 后方可移出 scope。本 plan 保留的 live producer 不得以 `PlannedNoConsumer` 完成该阶段或归档。
- [ ] 每字段至少一条“相同 gameplay 输入，仅 modifier 不同”的 production differential integration test；同时覆盖 neutral、累计/组合、非有限或脏持久化值 fail-closed、重登水合后仍生效及 reset。只断言 `InsightModifiers` 字段变化不算验收。
- [ ] 可核验 symbol：`insight_qi_regen_multiplier`、`effective_breakthrough_bonus`、`effective_overload_threshold`、`effective_meridian_heal_rate`、`effective_vortex_delta`、`effective_vortex_flow_speed`（名称可等义）及 `insight_modifier_changes_*_gameplay` 测试族。

## P2 — Iron Cocoon wound-grade + effective flow

- [ ] pre-P0 决议冻结唯一 wound pipeline：`raw hit → armor mitigation → effective severity → grade threshold 判定（含等号归属）→ 每 hit 至多一次 deterministic fracture roll → downgrade → health/bleeding/contamination/肢体/meridian crack/LifeRecord/event 全后果派生`；若代码事实要求不同顺序，§8.1 必须给出 worldview 与当前调用链双锚点后统一改写本条。`bruise_threshold_multiplier`、`cut_pierce_downgrade` 必须统一作用于真实 effective severity/grade，不能把 Bruise/Abrasion/Laceration/Fracture 伪装成 `WoundKind`。
- [ ] 所有下游后果——health、bleeding、contamination、肢体状态、meridian crack、LifeRecord 与 emitted event——必须消费同一降档结果，避免“伤口显示降档但流血/裂脉仍按原档”。
- [ ] `fracture_downgrade_chance` 使用 deterministic、测试可固定的 combat RNG；覆盖 0%、阈值等号及两侧、20% 命中/未命中与 100%，并保证同一 hit 只 roll 一次；另以 armor 把 raw severity 推过档次边界的案例锁定上述唯一顺序。
- [ ] 新建 `effective_meridian_sum_rate`（或等义 helper）：只有 `DerivedAttrs.scar_forged_flow_bonus == true` 时，才以唯一常数 `SCAR_FORGED_FLOW_RATE_BONUS` 对 `ActiveScarCircuits` 涉及的经脉应用有效倍率；先把所有 active circuit 的 `meridian_pair()` 展开为去重后的 `HashSet<MeridianId>`，同一经脉被多个回路共享也只乘一次 1.05，禁止按回路叠乘。不持久改 `Meridian.flow_rate`、不逐 tick 累积；先修 producer query 需读取 active circuits，再替换所有需要派生流率的生产调用点。
- [ ] 测试覆盖五种 `WoundKind`、四档后果边界、armor 前后顺序、ScarForged flag false/true、无/单/多活跃回路、两个回路共享同一经脉时仅加成一次、常数值 pin、重登 neutral 与 qi regen/ledger 守恒；`IronCocoonStage` 的 49/50、119/120、249/250、499/500 四组跃迁都必须覆盖前值/边界值、累计继承及降回低阶段时字段 reset。
- [ ] 可核验 symbol：`effective_wound_grade`、`cocoon_fracture_roll`、`effective_meridian_sum_rate`、`iron_cocoon_downgrade_changes_full_wound_consequences`、`scar_forged_bonus_only_applies_to_active_circuits`（名称可等义）。

## P3 — `jump_height_multiplier` runtime authority

- [ ] **共同验收**：pre-P0 在 Valence/Fabric 1.20.1 实证后选择唯一权威路线，优先 server-authoritative jump velocity/attribute；两路线都必须保护现有 `GuangboTicaoPracticeEvent` 生产、真元扣费与 proficiency 消费链。先建立唯一 `sanitized_jump_height_multiplier`（或等义 helper）：仅接受 finite 且位于 §8.1 冻结的 `[min,max]` 区间，0、负数、超上限、NaN、±Infinity 与脏持久化 proficiency 一律 fail-closed 回 neutral 1.0，禁止把非有限值送入 velocity/proto/store。实际起跳初速度/最高高度覆盖 multiplier 1.0/中间/上限、各非法输入、inactive/active technique、重登、地面/空中/水中/攀爬边界。
- [ ] **server-authoritative 路线（与 client 路线互斥）**：同 PR 接通 server movement 注册/调度、倍率边界、重登 neutral 与非法纵向速度拒绝，并由真实客户端/bot e2e 观察 apex；此路线禁止新增没有 runtime reader 的 `DerivedAttrsSyncV1`、proto、client store 或 mixin 字段。
- [ ] **client hook + server validation 路线（与 server 路线互斥）**：同 PR 接通 `DerivedAttrsSyncV1`、proto/generated/Rust convert、`DerivedAttrsHandler`、`DerivedAttrsStore`、独立非 mixin helper、mixin 配置中的 jump input hook、server validation 与 `SessionScopedStoreRegistry` 断线 reset；覆盖 proto round-trip、重复 payload/tick 幂等、断线 reset、hook 实际加载和非法纵向速度拒绝。
- [ ] 可核验 symbol 以 §8.1 选定路线为准：共同 `jump_height_multiplier`、`guangbo_jump_height_changes_observed_apex`；server 路线 `effective_jump_velocity` / movement registration；client 路线另含 `DerivedAttrsSyncV1`、`DerivedAttrsStore`、`jump_modifier_resets_on_disconnect`（名称可等义）。

## 范围边界 / 已排除项

- 不重修 r8 #1 / audit P1：`reach_bonus`、`DerivedAttrs.qi_regen_multiplier`、`contam_purge_multiplier` 已由 PR #1143 接 consumer；`healing_rate_multiplier` 也已有 runtime read。
- 不重做 r4 通用 `QiCapPermMinus` / `QiRegenBoost` consumer；本 plan 只修 JinZhongDan 负面槽错误使用正向 kind。
- `zhenfa_concealment` / `zhenfa_disenchant` 已接消费；`composure_recover_mul` 的 gameplay effect 由 sibling direct mutation 生效；`observe_chance_bonus` 无 live producer/production caller。三类都进入 P0 manifest 分类，但不强行再接第二次 consumer。
- 不纳入映射给其他 owner 的 Freeze 容器、JueBi marker、Botany release、TSY hostile ghost 与 distance decay；不修改 `docs/plan-container-filter-and-completion-v1.md` 或 qi_physics 常数。
- 不以展示或协议字段代替玩法效果；P1-P3 的每个完成项都必须同时存在 producer、production consumer、系统调度顺序与 observable differential test。

## §8 开放问题（实施前须追加 §8.1 决议）

1. `ContaminationBoost` 是一次性污染注入，还是有效期内持续压力；magnitude 单位、refresh/stack 如何定义？
2. JinZhongDan 正确 negative kind 与基础强度是什么，`neg_scale` 是否线性乘入？
3. `next_breakthrough_bonus` 在尝试开始、成功、任意结算还是仅失败后清除？
4. 两个 color penalty 如何保留 target color identity并迁移旧存档？
5. `qi_volatility_add`、`shock_sensitivity_add`、`sense_exposure_add`、`reaction_window_penalty` 的 canonical gameplay seam 分别在哪里？
6. `vortex_flow_speed_mul` 改 cast ticks、吸收 dt 还是维持周期？
7. 活茧降档修改真实 `Wound.severity`，还是引入统一 `effective_wound_grade`；Bruise threshold 的精确定义是什么？
8. fracture deterministic RNG 复用哪一个 combat seed/context？
9. ScarForged “活跃回路”的唯一数据源是否为 `ActiveScarCircuits`，NPC/离屏路径是否适用？
10. jump 最终采用 server authority 还是 client hook + server validation？
11. P0 manifest 采用哪一个可机械穷举的权威源（Rust AST、唯一声明宏或同等方案）；如何对完整字段/variant inventory 精确集合对账，并避免把文本 grep 冒充 reachability？
12. `InsightModifiers.practices` 如何迁成 typed enum/newtype registry 与唯一构造/解析入口；参数化 key 如何编码，未知/脏/重复/旧存档 key 如何 fail-closed 或 migration？
13. `vortex_burst_damage` 是否应先改 selector 使 talent production-reachable；若启用，cost 是迁成 typed 字段还是保留 registry 中的结构化 marker？
14. 唯一 wound pipeline 是否采用 P2 所列 `raw hit → armor → effective severity → grade → roll → downgrade → consequences`；阈值等号归哪一档？
15. jump multiplier 的合法 finite `[min,max]` 是什么；遇到 0、负数、超上限、NaN、±Infinity 与脏 proficiency 时是否统一回 neutral 1.0？

> 在上述十五项全部按 `docs/CLAUDE.md §五` 追加当前代码 `file:line + plan 章节` 双锚点决议之前，P0-P3 均不得实施。决议完成后，canonical live finding 若仍保留 production producer，就必须在本 plan 内转为 `Gameplay` 或有等价真实效果的 `ShadowDirectEffect` 才能完成阶段和归档；`PlannedNoConsumer` 只能保持阶段未完成。若决议选择不再支持该语义，须删除/禁用 producer 并迁移旧状态；若确需移交外域，须先由独立 docs PR 更新 canonical Finding Mapping 并建立带 owner、阶段和验收测试的 successor，不能由自动消费 agent 拍脑袋消项。

## §10 实施工作流

### §10.1 串行 PR 边界

本 plan scope 明确超过 4 个 PR，必须在单 plan 内序列化，前一个 merge 后才开始下一个，禁止将 P1/P2/P3 混成 giant PR：

1. **PR-0 contract gate**：仅 P0 typed manifest、完整类型 inventory 精确对账、typed practices registry、production caller/schedule/test 绑定与 gate 自测；不改 gameplay 数值。
2. **PR-1 alchemy**：仅 P1a `ContaminationBoost` + JinZhongDan 极性；同 PR 完成 production status→污染/回气 caller 与 schedule，含完整链及 ledger 差分。
3. **PR-2..PR-6 Insight A-E**：严格按 P1b 五个 gameplay 域拆分；每 PR 同包完成 production App/system/event/registry 注册和由真实 schedule 驱动的差分测试。找不到 canonical seam 时该 PR/阶段保持未完成；只有按 §8.1 删除/禁用 producer或先完成 canonical owner 移交，才可消除对应 live finding，禁止以 `PlannedNoConsumer` 结案。
4. **PR-7 Iron Cocoon wound**：只接唯一 wound pipeline、确定性 RNG、production resolve 注册与全后果一致性。
5. **PR-8 ScarForged effective flow**：只接 active circuits、effective rate、production 调用点与 qi ledger。
6. **PR-9 jump authority**：P3 独立 PR，严格按 §8.1 选定的一条互斥路线同包完成 runtime consumer/注册/验证；server 路线不创建死 proto/store，client 路线必须完成协议、hook 加载与断线 reset。
7. **PR-10 integration closure**：只验证或调整已注册系统之间的跨域顺序，补全链 e2e 与 manifest 汇总；不得首次注册任何 production consumer、system、EventReader、handler、registry、mixin 或 hook，也不得在此首次决定字段语义。

### §10.2 每 PR 验收门

- 每个功能 PR 必须在同 PR 完成并测试其 production App/system schedule、EventReader/事件链、registry/handler 或 mixin hook 加载；测试须从真实应用构建/调度入口驱动，直接调用 helper 只能作单元补充，不能满足该 PR 的闭环验收。PR-10 不得作为延迟 wiring 的兜底。
- 每个代码 PR 在修改前重新 fetch 并以 current `origin/main` 第一性验真；完成后用显式工作区绝对路径 + exact HEAD SHA 启动 fresh-context read-only validator，任何 HEAD 变化都重验。
- server 变更在 `server/` 下通过 `flock /tmp/bong-cargo.lock` 或 `scripts/build-token.sh` 运行 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`；严禁本地运行 `scripts/test-tmux-shutdown-order.sh` 或任何调用它的 suite，该隔离/关停覆盖留给 GitHub e2e。
- proto/schema 改动同步生成 Rust/proto artifacts 并跑 server gate；若触及 `agent/packages/schema/src`，先 `cd agent && npm run build -w @bong/schema` 再跑对应 `npm test`。
- client 变更固定 Java 17，并在 `client/` 下通过 `flock /tmp/bong-gradle.lock` 跑 `./gradlew test build`；P3 另跑真实 bot/e2e jump apex 场景。
- 每次 push 前紧邻执行 `git fetch origin && git merge origin/main`；merge 带进相关改动则重跑受影响完整门禁与 fresh validator。push 后对同一 PR 新发独立 `/review` 评论，等待 review/CodeRabbit 收敛。
- 所有提交使用中文原子 commit，带 `Model: <精确模型 id>` 与 `Co-Authored-By` trailer；最终归档前逐条核验 11 finding、P0 gate，并断言本 plan 保留 production producer 的 canonical live finding 中 `PlannedNoConsumer` 为 0；其余 dormant 条目须保留 owner/reason/exit condition，随后才填写 `## Finish Evidence`。

### §10.3 归档前跨域验收

- `modifier_consumer_manifest_stays_current` 必须从机械权威源穷举最新完整 `DerivedAttrs`、`InsightModifiers`、`StatusEffectKind` 集合，并从 typed practices registry 穷举全部 key/prefix，与 manifest 精确一一对账；每条 `Gameplay` 合同有 production caller/schedule + observable differential test，未知字段/variant/key 与 direct string marker 一律 fail-closed。
- 归档时本 plan 保留 production producer 的 11 条 canonical live finding 必须全部成为 `Gameplay` / 等价 `ShadowDirectEffect`，`PlannedNoConsumer` 数量必须为 0；删除/禁用 producer或移交外域只能按 §8.1 与独立 canonical Mapping 变更执行，不能用 manifest 备注代替交付。
- 运行 alchemy→status→污染/回气、Insight choice→持久化→gameplay loop、Iron Cocoon→resolve/flow、Guangbo→选定权威路线→实际 jump 四条完整链；每条链的 production 注册必须在所属功能 PR 已存在，PR-10 只验跨域顺序和 e2e，manifest 绿不能替代任何一条。
- qi 路径以 `WorldQiAccount` / `QiTransfer` 守恒断言验收；jump 共同以实际 apex/velocity 验收，server 路线验非法速度与重登 neutral，client 路线另验 proto 幂等与 disconnect reset。
- 全部阶段 ✅、上述 live-orphan 清零断言成立、review 无 blocker/major、GitHub e2e 覆盖本地隔离 suite 后，才填写 Finish Evidence 并迁入 `docs/finished_plans/`。
