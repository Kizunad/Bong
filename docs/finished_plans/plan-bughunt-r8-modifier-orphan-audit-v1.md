# plan-bughunt-r8-modifier-orphan-audit-v1（已归档）

> **归档说明（2026-07-28）**：除本说明与文末 Round bundle triage 外，下列正文完整保留本 plan 在冻结基线 `origin/main @ c625d5a5` 上的原始阶段、决议、测试与审计记录；正文里的 “Active / 骨架 / ⬜ / 开放问题” 是历史状态。当前唯一实施归属以文末 `Finding Mapping` 为准，移交 successor 的条目仍未实施，不因本 bundle 归档而视为完成。


> **Active（验证收口 + 后续实施入口）**。一句话主题：从 `docs/plans-skeleton/plan-bughunt-r8-findings-v1.md` 拆出 P0 modifier/derived-attr orphan 大簇，第一性原理复核 baomai-v4 `DerivedAttrs`、`InsightModifiers`、`jump_height_multiplier` 的“写入端齐全、消费端断裂”事实，并把后续修复拆成低风险消费层 PR + 需设计决策 PR，防止逐字段草率接线。
>
> 本 plan **不移动** `docs/plans-skeleton/plan-bughunt-r8-findings-v1.md`；该聚合 skeleton 保留 round8 全量 findings。本文只承接 P0 modifier/derived-attr orphan 主题。

## 接入面

- **进料**：`server/src/combat/components.rs::DerivedAttrs`、`server/src/combat/baomai_v4/scar_circuit.rs::scar_circuit_derive_system`、`server/src/combat/baomai_v4/iron_cocoon.rs::iron_cocoon_passive_system`、`server/src/cultivation/insight_apply.rs::InsightModifiers`、`server/src/combat/body_conditioning.rs::apply_guangbo_ticao_bonuses`。
- **出料**：近战 reach 判定 `server/src/combat/player_attack.rs`、真元吸收 `server/src/cultivation/tick.rs::qi_regen_and_zone_drain_tick`、污染排异 `server/src/cultivation/contamination.rs::contamination_tick`、伤口结算 `server/src/combat/resolve.rs::resolve_attack_intents`、经脉吞吐 `server/src/cultivation/components.rs::MeridianSystem::sum_rate`、movement/server-data/client jump 协议链路。
- **共享类型 / event**：复用 `DerivedAttrs`、`StatusEffects`、`InsightModifiers`、`ActiveScarCircuits`、`ScarHistory`、`KnownTechniques`、`DerivedAttrsSyncV1`；禁止另造 parallel modifier component。
- **跨仓库契约**：server 为主；`jump_height_multiplier` 若进入客户端跳跃，必须同步扩 `server/src/schema/combat_hud.rs::DerivedAttrsSyncV1`、proto convert、agent schema/generated、client `DerivedAttrsStore` 与跳跃输入/mixin 消费端。
- **worldview 锚点**：伤口档次与护甲降级见 `worldview.md §四 L250-L260`；经脉流量公式见 `worldview.md §四 L275-L288`；污染排异 10:15 亏损见 `worldview.md §四 L344-L351`；爆脉体修定位见 `worldview.md §五 L401-L405`。
- **qi_physics 锚点**：真元吸收必须继续走 `qi_physics::excretion::regen_from_zone` 与 `WorldQiAccount` ledger；污染排异必须继续走 `release_qi_amount_to_zone` / `QiTransfer`，任何倍率只能作用于 rate/cost，不得凭空增减真元。

## 阶段总览

| 阶段 | 内容 | 状态 |
|---|---|---|
| P0 | 验证矩阵：确认真 orphan、排除误报、标注不应立即修的字段 | ✅ 2026-07-07 |
| P1 | 低风险 server 消费层：baomai-v4 reach / qi_regen / contam_purge 的最小 hook 与守恒测试 | ✅ 2026-07-08 |
| P2 | 活茧伤口档次与茧灵 flow 设计门：确定降级算法、随机性、flow 临时倍率归属 | ⬜ needs design |
| P3 | InsightModifiers 正向/负向字段消费层：按 gameplay loop 拆 PR 接线 | ⬜ needs design |
| P4 | `jump_height_multiplier` 下游：上游练习事件已接通，仍需 server/client 权威端设计 | ⬜ needs design |
| P5 | 防孤岛回归：modifier 字段新增必须绑定 consumer/test/lint allowlist | ⬜ |

## P0 — 验证矩阵（已收口）

### 0.1 baomai-v4 疤纹回路

**写入端成立**：`scar_circuit_derive_system` 每 tick 从 `ActiveScarCircuits` 写 `reach_bonus` / `qi_regen_multiplier` / `contam_purge_multiplier` / `healing_rate_multiplier`，并在 `combat/mod.rs` 注册到 `CombatSystemSet::Physics`。

**消费端结论**：

- `healing_rate_multiplier` **非 orphan**：`server/src/combat/lifecycle.rs::health_regen_tick` 已读取并乘入血量恢复。
- `reach_bonus` **orphan**：`server/src/combat/player_attack.rs:82` 只用 `weapon_reach()` / `FIST_REACH`，不查 `DerivedAttrs`。
- `qi_regen_multiplier` **orphan**：`server/src/cultivation/tick.rs:117` 查询了 `Option<&DerivedAttrs>`，但 `tick.rs:191` 起只读 `qi_max_multiplier` 并乘 status/zone/territory 等倍率，没有读该字段。
- `contam_purge_multiplier` **orphan**：`server/src/cultivation/contamination.rs:93` 起的 query 不含 `DerivedAttrs`，`contamination.rs:129` purge rate 只由基础值和炼丹技能决定。

**不立即机械全修原因**：`reach_bonus` 是低风险候选；`qi_regen_multiplier` 与 `contam_purge_multiplier` 涉及真元守恒与污染排异代价，必须在 `regen_from_zone` / `release_qi_amount_to_zone` 既有路径内加倍率和守恒断言，不能只改数值。

### 0.2 baomai-v4 活茧

**写入端成立**：`iron_cocoon_passive_system` 写 `bruise_threshold_multiplier` / `fracture_downgrade_chance` / `cut_pierce_downgrade` / `scar_forged_flow_bonus`。

**消费端结论**：

- 伤口链 orphan：`server/src/combat/resolve.rs::wound_kind_profile` 是按 `WoundKind` 静态查表；`apply_armor_mitigation` 只消费 `defense_profile`，没有读取活茧三字段。
- 茧灵 flow orphan：`server/src/cultivation/components.rs:289` 的 `sum_rate()` 只累加持久 `flow_rate`；全仓没有读取 `scar_forged_flow_bonus` 改活跃回路经脉 flow。

**不立即机械修原因**：伤口档次降级需要定义作用点（命中前改 kind、命中后改 wound grade、还是防御矩阵叠加）、随机性来源和测试可重复性；茧灵 flow 需要决定是临时 effective rate、直接改 `Meridian.flow_rate`，还是在 baomai-v4 计算层加成。直接写入持久 `flow_rate` 会和锻造/Insight永久加成混在一起。

### 0.3 InsightModifiers

**写入端成立**：`InsightModifiers` 定义在 `server/src/cultivation/insight_apply.rs:24` 起；`apply_choice` 写 `qi_regen_mul`、`overload_tolerance_add`、`chaotic_tolerance_add`、`hunyuan_threshold_mul`、`next_breakthrough_bonus`、涡流/阵法字段等；`apply_tradeoff_cost` 写全部负向代价字段。

**消费端结论**：

- r7 已登记的五字段仍应归统一消费层，不在本 plan 重复实现：`qi_regen_mul`、`next_breakthrough_bonus`、`vortex_backfire_resist_mul`、`vortex_delta_bonus_add`、`vortex_flow_speed_mul`。
- 本轮新增确认 orphan：`hunyuan_threshold_mul`、`chaotic_tolerance_add`、`overload_tolerance_add`，以及 `opposite_color_efficiency_penalty`、`qi_volatility_add`、`shock_sensitivity_add`、`main_color_efficiency_penalty`、`overload_fragility_add`、`reaction_window_penalty`、`breakthrough_failure_penalty_mul`、`sense_exposure_add`、`meridian_heal_slowdown_mul`、`chaotic_tolerance_loss`。
- `composure_recover_mul` **不是真实 no-op**：`apply_choice` 同时直接改 `Cultivation.composure_recover_rate`，`server/src/cultivation/composure.rs:10` 起读取该 rate；字段本身是影子记录，但 gameplay 收益已生效。

**不立即机械修原因**：`plan-insight-alignment-v1` 要求代价在日常 gameplay loop 中可感知。各字段消费点分属回气、突破、污染/感知、阵法反应窗口、过载撕裂、经脉恢复、颜色效率；一次 PR 全接会混合平衡、UI、schema、测试，review 不可控。

### 0.4 jump_height_multiplier

**写入端成立**：`apply_guangbo_ticao_bonuses` 写 `move_speed_multiplier` 与 `jump_height_multiplier`。当前主干已在 `server/src/network/cast_emit.rs:235-240` 从 `body.guangbo_ticao` cast 自然完成发送 `GuangboTicaoPracticeEvent`，`server/src/combat/body_conditioning.rs::consume_guangbo_practice_events` 消费后扣真元并增长 proficiency。因此 r10 skeleton 里“生产端零结果”的旧描述已过期，不能再作为本 plan 的阻塞理由。

**消费端结论**：`move_speed_multiplier` 已被 `server/src/movement/mod.rs:273-274` 消费；`jump_height_multiplier` 仍只在 default / 写入 / 单元测试中出现，server movement 不读，`DerivedAttrsSyncV1` / proto convert / client `DerivedAttrsStore` 也没有 jump 字段。广播体操跳跃收益的上游已有机会生效，但下游跳跃权威端和跨端同步仍断裂。

**不立即机械修原因**：跳跃不是纯 server 数值乘区；必须先决定 MC 1.20.1 下 jump velocity/attribute 的权威端、是否需要 client input/mixin、断线 reset 与反作弊口径。直接把字段塞进 schema 但没有 jump consumer 仍会制造新孤岛。

## P1 — baomai-v4 疤纹回路低风险消费层（已落地）

目标只处理可独立验证的 server hook，不把活茧/Insight 一起塞入。P1 已按最小闭环落地：

1. `reach_bonus`：`server/src/combat/player_attack.rs` 查询 `Option<&DerivedAttrs>`，近战 reach 判定和 `AttackIntent.reach` 同步使用 `base + attrs.reach_bonus.max(0.0)`；测试覆盖无 attrs extension-only 距离拒绝、空手/武器加成、负值 clamp。
2. `qi_regen_multiplier`：`server/src/cultivation/tick.rs` 在 `regen_from_zone` rate 乘区内读取 `DerivedAttrs.qi_regen_multiplier`，按 `plan-baomai-v4 §2.4` 只在 `BloodBurnActive` 有效期内生效；使用 `CombatClock` 口径判断焚血过期，倍率只放大 gain/drain，不绕过 `WorldQiAccount` audit。
3. `contam_purge_multiplier`：`server/src/cultivation/contamination.rs` query 加 `Option<&DerivedAttrs>`，只乘 `purge_rate`，不改 `DRAIN_RATIO`；污染排异仍走 `release_qi_amount_to_zone` / `QiTransferReason::ReleaseToZone`。
4. 生产 wiring：`server/src/cultivation/mod.rs` 显式让 `qi_regen_and_zone_drain_tick` 排在 `attribute_aggregate_tick`、`scar_circuit_derive_system`、`body_conditioning_aggregate` 之后，避免消费上一帧或未派生的 `DerivedAttrs`。

**测试声明**：`cd server && CARGO_BUILD_JOBS=1 nice -n 10 cargo test player_attack`；`cd server && CARGO_BUILD_JOBS=1 nice -n 10 cargo test scar_qi_regen`；`cd server && CARGO_BUILD_JOBS=1 nice -n 10 cargo test scar_contam`；最终 gate 仍按 §10 跑完整 server 命令。

## P2 — 活茧伤口档次与茧灵 flow 设计门

实施前必须先追加活茧专项决议，明确：

1. `bruise_threshold_multiplier` 如何映射到现有 `WoundKindProfile` / wound grade；若仓库当前没有 BRUISE grade 结算入口，不得伪造“阈值已生效”。
2. `fracture_downgrade_chance` 的 RNG 来源：必须 deterministic，可在 combat resolve 测试中固定 seed；不能用不可复现随机。
3. `cut_pierce_downgrade` 是改 `intent.wound_kind` 还是改生成的 `Wound`；需保持 contamination/bleed/crack 与降级后 wound 一致。
4. `scar_forged_flow_bonus` 只能是 effective rate 加成，不能持久改 `Meridian.flow_rate`；建议新建纯函数 `effective_sum_rate(meridians, attrs, circuits)` 并逐步替换 `sum_rate()` 消费点。

**测试声明**：`cd server && cargo test combat::resolve combat::baomai_v4::tests::p2_iron_cocoon cultivation::tick`。

## P3 — InsightModifiers 消费层拆分

不要做“一个巨大 PR 全字段接线”。推荐拆分：

1. **PR-Insight-A 回气/突破已有主循环**：`qi_regen_mul`、`next_breakthrough_bonus`、`breakthrough_failure_penalty_mul`。这些已有明确 tick / breakthrough 消费点，最适合先接。
2. **PR-Insight-B 颜色/混元/杂色**：`hunyuan_threshold_mul`、`chaotic_tolerance_add`、`chaotic_tolerance_loss`、`opposite_color_efficiency_penalty`、`main_color_efficiency_penalty`。需要对齐 `QiColor` / `PracticeLog` 的现有计算，补 UI/inspect 展示。
3. **PR-Insight-C 过载/经脉恢复**：`overload_tolerance_add`、`overload_fragility_add`、`meridian_heal_slowdown_mul`。需要对齐 `cultivation::overload` 与经脉恢复系统，避免和 baomai-v4 过载史重复计数。
4. **PR-Insight-D 感知/反应窗口/冲击**：`sense_exposure_add`、`reaction_window_penalty`、`shock_sensitivity_add`。这组没有单一消费点，必须先设计 gameplay loop 与 UI 反馈。

**测试声明**：每个 PR 必须有“字段写入前后同一玩法 loop 差分”的 integration test，不能只测 `InsightModifiers` 数值累计。

## P4 — jump_height_multiplier 下游设计门

**当前结论**：不再阻塞于 `GuangboTicaoPracticeEvent` 生产端。当前代码已有 `cast_emit.rs` 发送事件 + `body_conditioning.rs` 消费事件的熟练度闭环；本 plan 的 jump 问题收窄为“`DerivedAttrs.jump_height_multiplier` 下游无 runtime consumer / 无跨端字段”。

实施前必须先追加 jump 专项决议，明确：

1. server：Valence/MC 1.20.1 是否有安全的 jump attribute / velocity hook；若 server authoritative，必须有 movement/e2e 差分测试。
2. client：若只能 client-side 处理，则 `DerivedAttrsSyncV1`、proto convert/generated、client `DerivedAttrsStore`、跳跃输入/mixin 必须同 PR 接通，断线 reset 测试必须覆盖。
3. anti-orphan：新增 `jump_height_multiplier` consumer 后必须进入 P5 manifest；不能只扩 schema 或只扩 client store。
4. playtest-fixes 保护：不得删除或弱化 `cast_emit.rs` 现有 `GuangboTicaoPracticeEvent` 生产端与 `consume_guangbo_practice_events` 守恒消费端。

## P5 — 防孤岛回归

新增一个轻量守门，不追求完美静态分析，但要防同类问题继续扩大。

1. 建立 modifier consumer 清单：`DerivedAttrs` / `InsightModifiers` / `StatusEffectKind` 每个非展示字段必须列出至少一个 runtime consumer 或显式 `planned_no_consumer` 理由。
2. 用 server 单测或脚本检查清单中的字段仍能在消费文件中 grep 到；测试名建议 `modifier_consumer_manifest_stays_current`。
3. 新增字段若只出现在 struct/default/write/test 中，测试必须红，并要求 plan 阶段写清 BLOCKED/设计归属。

## §7 开放问题

1. `qi_regen_multiplier` 是否必须限定 BloodBurnActive？`plan-baomai-v4 §2.4` 写“焚血激活期间”，但当前字段仅由回路写入，不携带焚血状态。
2. `bruise_threshold_multiplier` 对应的 BRUISE 阈值在现有 `resolve.rs` 是否有可插入口，还是需要先补 wound grade 阶梯。
3. Insight 颜色效率惩罚是否应落到技能伤害、修炼效率、还是 `PracticeLog` 权重变化。
4. `jump_height_multiplier` 的权威端是 server 物理还是 client jump input。

## §7.1 决议（pre-P0 收口，2026-07-07）

### #1 P0 本轮是否直接大范围修字段

**决议**：
1. 不直接大范围修。P0 已验证孤岛为真，但主题跨度覆盖 combat resolve、cultivation ledger、Insight 平衡、client movement。
2. 后续只允许按 P1/P2/P3/P4 拆分 PR；每个 PR 必须证明“写入字段 → 消费系统 → 可观察 gameplay 差分 → 守恒/协议测试”闭环。
3. `healing_rate_multiplier` 与 `composure_recover_mul` 从 bug 清单剔除，避免重复修已生效或影子记录字段。

**落点**：本文 P0；证据来自 `server/src/combat/lifecycle.rs::health_regen_tick`、`server/src/cultivation/composure.rs::composure_tick`。

### #2 低风险局部项是否本 PR 小修

**决议**：
1. 本 PR 承接 P1，只修 baomai-v4 疤纹回路里能独立闭环的三项 server consumer：`reach_bonus`、`qi_regen_multiplier`、`contam_purge_multiplier`。
2. 不修活茧、Insight、jump 三组字段；这些字段的作用点、随机性、跨端权威或平衡口径仍未收口，继续留在 P2-P4。
3. 本 PR 的验收以代码差分 + 饱和测试 + gpt-5.5 xhigh read-only validator PASS 为准。

**落点**：本文 P1；代码落点 `server/src/combat/player_attack.rs`、`server/src/cultivation/tick.rs`、`server/src/cultivation/contamination.rs`、`server/src/cultivation/mod.rs`。

### #3 jump_height 处理顺序

**决议**：
1. 不再把 r10 P1 当作前置阻塞：当前主干 `cast_emit.rs:235-240` 已发送 `GuangboTicaoPracticeEvent`，`body_conditioning.rs::consume_guangbo_practice_events` 已消费并增长 proficiency。
2. 本 PR 仍不接 `jump_height_multiplier`，因为下游权威端跨 server/client，需要先做专项设计；只扩字段不接 jump consumer 仍是孤岛。
3. P4 从 BLOCKED 改为 needs design，后续 PR 必须同包提交 runtime consumer、跨端同步、断线 reset 与差分测试。

**落点**：本文 P4；当前代码锚点 `server/src/network/cast_emit.rs:235-240`、`server/src/combat/body_conditioning.rs::consume_guangbo_practice_events`。

### #4 防孤岛策略

**决议**：
1. 后续消费本 plan 时必须先做 P5 的 manifest/lint，或者在 P1 第一 PR 里至少引入最小 consumer 清单。
2. 清单不是替代测试；它只防“新增字段没有任何 runtime consumer”。每个字段仍需 gameplay 差分测试。
3. 允许把短期无法接线的字段标成 `planned_no_consumer`，但必须写明 owner plan 与阻塞原因。

**落点**：本文 P5。

## §10 实施工作流

- 一个 PR 只处理一个阶段或一个拆分 PR，不得把 P1/P2/P3 混在一起。
- 纯 server PR 跑 `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`；若只改 plan 文档，允许以 grep/validator 证据替代 cargo。
- 改 `agent/packages/schema/src/*.ts` 后必须 `cd agent && npm run build -w @bong/schema`。
- 涉及 client jump 时必须使用 JDK 17 跑 `cd client && ./gradlew test build`，不得用系统默认 JDK 21。
- 若后续开 PR，PR body 必须逐字段说明：写入端、消费端、因果链、测试证据、剩余风险。

---

## 2026-07-28 Round bundle finding triage

本节仅登记当前裁决与唯一 owner；不改写上文历史结论，也不把 P2-P5 标为完成。

## Finding Mapping

| Finding / 阶段 | 当前裁决 / current `file:line` | 分类 | Canonical owner / merged evidence | 文档动作 |
|---|---|---|---|---|
| P0 验证矩阵 | `server/src/combat/components.rs:311-392`、`server/src/cultivation/insight_apply.rs:24-98` 与 `server/src/combat/body_conditioning.rs:157-167` 的 producer/consumer 第一性矩阵与误报剔除完整保留于上文 | audit-history（不是 finding row） | 本归档只保留验证史，不形成 implementation owner | 唯一 history row，不计入 60 个 finding rows |
| P1 scar circuit 三字段 | `server/src/combat/player_attack.rs:37-107` 消费 reach；`server/src/cultivation/tick.rs:240-269` 消费 regen；`server/src/cultivation/contamination.rs:97-211` 消费 purge | `already-fixed/invalid`（already-fixed） | `3e6981513` / PR #1143 | 不重复实施 |
| P2 wound-grade：`bruise_threshold_multiplier` / `fracture_downgrade_chance` / `cut_pierce_downgrade` | `server/src/combat/baomai_v4/iron_cocoon.rs:110-139` 写三项伤口字段，combat resolve 仍不读 | `independent-domain-fix` | `docs/plans-skeleton/plan-bughunt-modifier-effect-consumer-completion-v1.md` P2 | 独立 wound-grade finding row |
| P2 scar-forged flow：`scar_forged_flow_bonus` | `server/src/combat/baomai_v4/iron_cocoon.rs:106-143` 写 flag；`server/src/cultivation/components.rs:522-524` 的 `MeridianSystem::sum_rate` 不读 | `independent-domain-fix` | 同 successor P2 | 独立 effective-flow 设计/验收行 |
| P3 InsightModifiers | `observe_chance_bonus` 当前仅有默认值（`server/src/cultivation/insight_apply.rs:34,81`；无 effect 写入）；`server/src/cultivation/technique_observe.rs:64-87` 的 `observe_learn_chance` 读取该字段，但 `server/src/cultivation/technique_observe.rs:90-134` 的 `evaluate_observe_attempt` 仅由 tests（`server/src/cultivation/technique_observe.rs:253,280,315`）调用、无 production caller；其余 live 字段见 `server/src/cultivation/insight_apply.rs:24-254` | `independent-domain-fix` | 同 successor P3 | 仅迁 live 字段，排除已生效项 |
| P4 jump | `server/src/combat/status.rs:174` 有字段 reset、`server/src/combat/body_conditioning.rs:157-167` 有写入；`server/src/schema/combat_hud.rs:209-220`、`server/src/network/derived_attrs_emit.rs:76-90` 与 `client/src/main/java/com/bong/client/combat/store/DerivedAttrsStore.java:13-28` 均无 jump 字段/consumer | `independent-domain-fix` | 同 successor P4 | R6 emit/bridge → R2 store/reset → focused gameplay consumer 依赖链 |

## Successor implementation gate（非 finding row）

- P5 anti-orphan manifest/lint 是 `plan-bughunt-modifier-effect-consumer-completion-v1.md` 的共享实施门，不是 round bundle 的独立 finding，故不占 Finding Mapping 数据行；successor 仍必须在 P1-P4 implementation 前按其 anti-orphan 前置契约落地。

## Finish Evidence

> 本 plan 当前完成 P0 验证收口 + P1 低风险 server 修复；P2-P5 未完成，暂不归档。
>
> **2026-07-28 归档更新**：P0 验证与 P1 修复已经完成；P2-P5 从未在本 plan 实施。Round bundle triage 将其完整规格移交唯一 successor 后归档本 audit；移交只关闭重复队列，不代表 successor 已落地。

### 原 P0/P1 实施与测试证据（完整保留）

- **落地清单**：新增本文档；`server/src/combat/events.rs` 增加 `AttackReach::with_bonus`；`server/src/combat/player_attack.rs` 消费 `DerivedAttrs.reach_bonus`；`server/src/cultivation/tick.rs` 消费 `DerivedAttrs.qi_regen_multiplier` 并按 `BloodBurnActive` / `CombatClock` 限定；`server/src/cultivation/contamination.rs` 消费 `DerivedAttrs.contam_purge_multiplier`；`server/src/cultivation/mod.rs` 补生产调度顺序；未移动 `docs/plans-skeleton/plan-bughunt-r8-findings-v1.md`。
- **关键 commit**：本 PR 拆为近战 reach 消费、回气/排异守恒消费、审计证据三个提交；最终提交列表以 PR git log 为准。
- **测试结果**：
  - ✅ `cd server && CARGO_BUILD_JOBS=1 nice -n 10 cargo test player_attack`
  - ✅ `cd server && CARGO_BUILD_JOBS=1 nice -n 10 cargo test scar_qi_regen`
  - ✅ `cd server && CARGO_BUILD_JOBS=1 nice -n 10 cargo test scar_contam`
  - ✅ `cd server && CARGO_BUILD_JOBS=1 nice -n 10 cargo test qi_regen_skips_despawned_offline_cultivators`
  - ✅ `cd server && CARGO_BUILD_JOBS=1 nice -n 10 cargo test contamination_tick_skips_despawned_offline_players`
  - ✅ `cd server && CARGO_BUILD_JOBS=1 nice -n 10 cargo fmt --check`
  - ✅ `cd server && CARGO_BUILD_JOBS=1 nice -n 10 cargo test`（10926 passed / 1 ignored；main 11 passed；full_app 1 passed；backpack e2e 4 passed；doctest 5 ignored）
  - ⚠️ `cd server && CARGO_BUILD_JOBS=1 nice -n 10 cargo clippy --all-targets -- -D warnings` 当前被仓库级 Rust 1.96.1 clippy 新 lint 债阻塞：69 个错误分布在 botany / fauna / inventory / network / npc / world 等未触碰模块，主要为 `manual_is_multiple_of`、`derivable_impls`、`manual_checked_ops`、`unnecessary_sort_by` 等；本 PR 修改文件未出现在失败列表中。
- **grep/读码证据**：
  - `reach_bonus` / `qi_regen_multiplier` / `contam_purge_multiplier`：写入在 `server/src/combat/baomai_v4/scar_circuit.rs:213-228`；P0 时 `player_attack.rs` reach 判定只读 `weapon_reach()` / `FIST_REACH`，`cultivation/tick.rs` 只消费 `qi_max_multiplier`，`contamination.rs` query 不含 `DerivedAttrs`。P1 后三者分别由 `player_attack.rs`、`tick.rs`、`contamination.rs` 消费。
  - `bruise_threshold_multiplier` / `fracture_downgrade_chance` / `cut_pierce_downgrade` / `scar_forged_flow_bonus`：写入在 `server/src/combat/baomai_v4/iron_cocoon.rs:110-140`；`resolve.rs` wound profile 静态查表，`MeridianSystem::sum_rate()` 只累加持久 `flow_rate`。
  - `InsightModifiers`：收益/惩罚字段写入在 `server/src/cultivation/insight_apply.rs:124-247`；除 `composure_recover_mul` 的直接 rate 生效路径外，本轮列出的惩罚/收益字段没有 runtime consumer。
  - `jump_height_multiplier`：写入在 `server/src/combat/body_conditioning.rs:166-167`，`move_speed_multiplier` 兄弟字段在 `server/src/movement/mod.rs:273-274` 消费；`jump_height_multiplier` 不在 `DerivedAttrsSyncV1`、proto convert、client `DerivedAttrsStore` 中。
- **跨仓库核验**：server `DerivedAttrs` / `InsightModifiers` / `DerivedAttrsSyncV1` 已核对；client jump 下游未实施，列入 P4 needs design。
- **遗留 / 后续**：P2 活茧、P3 InsightModifiers、P4 jump、P5 防孤岛 manifest 均为后续实施阶段。

### 2026-07-28 Round bundle triage 证据

- **落地清单**：P0 结论与 P1 代码修复历史保留；P2-P5 全部迁到 `plan-bughunt-modifier-effect-consumer-completion-v1`，后者明确列 canonical 字段、排除项、设计门与跨栈验收；本文件迁入 finished。
- **关键 commit**：`3e6981513`（2026-07-09，PR #1143）接通 reach/regen/purge；已验证为 `origin/main @ c625d5a5` 祖先且当前 consumers 存在。
- **测试结果**：原 P1 Finish Evidence 记录 server tests/gate；本次只做 docs-only triage，以 docs static gate + exact-HEAD validator 验收，不复跑旧代码测试。
- **跨仓库核验**：P1 为 server-only；jump 的未实施 server/schema/client 链已明确迁 successor，不能以单端 schema 代替 runtime consumer。
- **遗留 / 后续**：唯一 successor 为 `plan-bughunt-modifier-effect-consumer-completion-v1` P2-P5；P2 wound-grade/flow、P3 Insight、P4 jump 分别见 Finding Mapping，P5 anti-orphan manifest/lint 作为共享 implementation gate 见紧随表后的非 finding 说明；本 audit 禁止再消费。
