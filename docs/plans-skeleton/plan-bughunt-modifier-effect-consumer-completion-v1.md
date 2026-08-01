# plan-bughunt-modifier-effect-consumer-completion-v1（骨架）

> **骨架（草案）**。一句话主题：只收束 r6/r7/r8 已映射的 11 条 canonical modifier/effect finding，并建立一套共享 anti-orphan gate；未进入 Canonical Finding Mapping 的观察不属于本 plan 交付物。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 完整类型库存、mapped producer manifest 与 pending-closure 门禁 | ⬜ |
| P1 | 按 Alchemy/Insight、Iron Cocoon、jump 三批把 mapped pending 原子转换为 gameplay closure | ⬜ |

## 范围原则

- **唯一强制范围**：下表 11 条 canonical finding，以及为机械验收这些 finding 所必需的 P0 共享门禁。
- **finding 数不等于字段数**：11 是 r6/r7/r8 canonical finding 的计数；聚合 finding r8 #5 冻结为下表 13 个子字段，manifest 与验收按子字段精确对账。
- **inventory 不自动扩 scope**：P0 若发现未映射 orphan，只登记为 `UnmappedObservation`；它不属于本 plan 的迁移或 gameplay 实现，也不得被静默升级成本 plan 交付物。
- **不冒充闭环**：HUD、schema、persistence、client store、测试内读取或无 production caller 的 helper 都不算 gameplay consumer。

## 接入面

- **进料**：`server/src/alchemy/side_effect_apply.rs:20-83` 与 `server/src/alchemy/pill.rs:632-642` 生产 mapped `StatusEffectKind`；`server/src/cultivation/insight_apply.rs:24-254` 写 mapped `InsightModifiers`；`server/src/combat/baomai_v4/iron_cocoon.rs:99-143` 写四个 mapped `DerivedAttrs` 字段；`server/src/combat/body_conditioning.rs:157-167` 写 `jump_height_multiplier`。
- **出料**：污染排异、回气/突破/过载/颜色/涡流 gameplay loop、`combat::resolve_attack_intents` 的 canonical wound sink、effective meridian flow，以及唯一权威 jump runtime 必须读取对应 mapped modifier。
- **共享类型 / event**：复用 `StatusEffects`、`StatusEffectKind`、`InsightModifiers`、`DerivedAttrs`、`MeridianSystem`、`ActiveScarCircuits`、`Wound`/`WoundKindProfile`、`DerivedAttrsSyncV1`；禁止另造 parallel modifier component 或持久改写派生倍率。
- **跨仓库契约**：P0 与 P1-A/P1-B 以 server 为主，P0 不得要求任何 client ACK/wire。P1-C 只实现一条互斥路线：server-authoritative，或 client-hook + server-validation。仅后一条需要 `server emitter → proto/generated → handler/store → 非 mixin helper → jump hook` 的单向链，以及 `DerivedAttrsHandler` 原子安装 authority tuple 后产生 `NeutralAppliedAck`、经命名 `ClientRequestProtocol` / `ClientRequestSender` 的 `bong:client_request` send site、Rust `ClientRequestV1` decode/dispatch 和真实 server consumer 的反向链。`NeutralAppliedAck` 只证明 tuple 已安装；`JumpAuthorityTupleReport` 只观测当前 tuple，二者不得互相替代或单独驱动 authority transition。server-authoritative 路线不得新增这些 client ACK/proto/store seam，并将对应 client-wire 验收记为 N/A。
- **worldview 锚点**：`worldview.md §四 L250-L260`（伤口档次与护甲后果）、`§四 L275-L288`（经脉流量）、`§四 L344-L351`（污染排异亏损）、`§五 L401-L405`（爆脉体修）。
- **qi_physics 锚点**：回气继续走 `qi_physics::excretion::regen_from_zone` / `WorldQiAccount` ledger，污染排异继续走 `release_qi_amount_to_zone` / `QiTransfer`；modifier 只改变 canonical rate/threshold/cost，不得无对端修改 `qi_current` 或 zone qi。`scar_forged_flow_bonus` 只改变 effective rate，不持久改 `Meridian.flow_rate`。

## Canonical Finding Mapping（本 plan 的全部 mandatory scope）

| 来源 | Canonical finding | 批次 |
|---|---|---|
| r6 #0 | `ContaminationBoost` gameplay consumer 缺失 | P1-A |
| r6 #1 | JinZhongDan negative slot 写正向 `QiRegenBoost` | P1-A |
| r7 #3-#7 | `qi_regen_mul`、`next_breakthrough_bonus`、`vortex_backfire_resist_mul`、`vortex_delta_bonus_add`、`vortex_flow_speed_mul`（5 条） | P1-A |
| r8 #2 | `bruise_threshold_multiplier` / `fracture_downgrade_chance` / `cut_pierce_downgrade` | P1-B |
| r8 #3 | `scar_forged_flow_bonus` | P1-B |
| r8 #5 | 下表冻结的 13 个 `InsightModifiers` benefit/cost 子字段（1 条聚合 finding） | P1-A |
| r8 #4 | `jump_height_multiplier` | P1-C |

计数：r6 两条 + r7 五条 + r8 四条 = **11 条 canonical finding**。P0 是共享实施门，不是第 12 条 finding；r8 #5 的 13 个字段不另增 finding 数。

### r8 #5 冻结子字段库存

Canonical 来源为 `docs/finished_plans/plan-bughunt-r8-findings-v1.md:21-27,61-69`；writer 均在 `server/src/cultivation/insight_apply.rs`。P0 仍须逐项复验 production reachability；不可达 writer 也不得从本表静默消失，只能在 P1-A 删除/禁用 producer 并迁移状态，或接通真实 consumer。

| 子字段 | writer | neutral | 目标 gameplay domain |
|---|---|---:|---|
| `hunyuan_threshold_mul` | `insight_apply.rs:155-156` | `1.0` | 混元成立阈值 |
| `chaotic_tolerance_add` | `insight_apply.rs:149-152` | `0.0` | 杂色/混沌容忍 |
| `overload_tolerance_add` | `insight_apply.rs:123-125` | `0.0` | 经脉过载阈值 |
| `opposite_color_efficiency_penalty` | `insight_apply.rs:230-232` | `0.0` | 对立色效率 |
| `qi_volatility_add` | `insight_apply.rs:233` | `0.0` | 真元波动后果 |
| `shock_sensitivity_add` | `insight_apply.rs:234` | `0.0` | 心境冲击 |
| `main_color_efficiency_penalty` | `insight_apply.rs:235-237` | `0.0` | 主色效率 |
| `overload_fragility_add` | `insight_apply.rs:238` | `0.0` | 经脉过载后果 |
| `meridian_heal_slowdown_mul` | `insight_apply.rs:239` | `1.0` | 经脉修复 |
| `breakthrough_failure_penalty_mul` | `insight_apply.rs:240-242` | `1.0` | 突破失败后果 |
| `sense_exposure_add` | `insight_apply.rs:243` | `0.0` | 感知暴露 |
| `reaction_window_penalty` | `insight_apply.rs:244-246` | `0.0` | 战斗反应窗口 |
| `chaotic_tolerance_loss` | `insight_apply.rs:247` | `0.0` | 杂色容忍扣减 |

`composure_recover_mul` 不属于 r8 #5：canonical r8 只确认其 sibling `Cultivation.composure_recover_rate` 有 gameplay consumer；`InsightModifiers.composure_recover_mul` 本身仍无 reader，P0 必须将它登记为 `UnmappedObservation`，不得误标 `ExistingGameplay`；其消费或清理不属于本 plan 范围。`practices` 动态 marker 及本表外字段同样不因 P0 inventory 自动进入 mandatory scope。

## 第一性验真（`origin/main @ de75f14e43daf1105ea978c43d187acbb7f12f14`，2026-07-29）

1. `ContaminationBoost` 可生产、可 upsert、可显示、会到期，但 `server/src/cultivation/contamination.rs:97-205` 不读 `StatusEffects`。
2. JinZhongDan negative slot 在 `server/src/alchemy/pill.rs:632-642` 仍生产正向 `QiRegenBoost(0.001)`；现有 consumer 按 `1 + magnitude` 增益回气。
3. r7 五字段与 r8 #5 冻结的 13 个子字段由 `server/src/cultivation/insight_apply.rs:24-254` 写入；production reachability 与 reader 缺口须由 P0 逐项登记，不能以聚合名称略过。
4. Iron Cocoon 四字段由 `server/src/combat/baomai_v4/iron_cocoon.rs:99-143` 写入；`combat::resolve_attack_intents` 与 `MeridianSystem::sum_rate` 未消费。
5. `jump_height_multiplier` 有 producer/reset，但 `server/src/network/derived_attrs_emit.rs:76-90`、`DerivedAttrsSyncV1`、client store 与真实 jump 链均无 consumer。
6. `server/src/test_coverage_guards.rs` 现有 event reader/writer 扫描不能证明上述 mapped modifier 已有 production consumer。

## P0 — mapped anti-orphan contract

- [ ] 建立 greppable `ModifierConsumerContract` / `MODIFIER_CONSUMER_MANIFEST`（名称可等义）。以唯一声明宏、Rust AST 或同等机械来源，穷举 `DerivedAttrs` 字段、`InsightModifiers` 字段、`StatusEffectKind` variant 与每个 production producer branch；另从 production 代码独立导出 producer reachability、typed consumer edge、consumer system 与 app 中实际 scheduler registration（排除 `cfg(test)`、测试调用和孤立 helper）。新增/删除/改名字段或 variant、新增/删除/改绑 writer branch、edge/caller/registration 未登记即失败。
- [ ] producer 与 consumer 分成两张可对账的 typed graph：`ModifierProducerSiteId → ModifierProducerCallerId/ModifierProducerSystemId → ModifierProductionEntryId/ModifierScheduleSiteId` 证明该精确 branch 能从真实生产入口触发；`ModifierMemberId → ModifierConsumerEdgeId(ModifierConsumerSiteId) → ModifierSystemId → ModifierScheduleSiteId` 证明 gameplay read 已由生产 app 装载。非测试源码中“存在 writer/helper”不等于 production reachable；缺 caller/entry/schedule 的 producer 不得进入 Closed/Existing。一个 member 可有多个 consumer edge，多个 member/system 可共享同一真实 registration，禁止伪造逐字段 registration。
- [ ] canonical lifecycle 记录必须显式区分 `ModifierContractSubject::Member` 与 `ModifierContractSubject::ProducerBinding`（名称可等义）。后者由稳定 `ModifierProducerBindingId` 绑定 `CanonicalFindingId + ModifierProducerSiteId + actual_member_id + expected_member_id + lifecycle + evidence`，同函数不同 branch 不得折叠。r6 #1 在 P0 精确登记 JinZhongDan negative branch 的 `actual=QiRegenBoost`、`expected=QiRegenSlowed`、`MappedPendingClosure`；P1-A 只能在代码 branch 与该记录同 PR 原子改绑且 expected member 的 consumer/schedule/differential 全链成立后转 Closed，或删除/禁用该精确 branch 后转 Retired。其他合法 `QiRegenBoost` producer site 与其共享 consumer/schedule 始终独立保持 `ExistingGameplay`，不得为关闭 r6 #1 全局退役该 member。
- [ ] manifest 必须把本 plan 的 11 条 canonical finding 及各自完整 expected subject set 固化为稳定 ID inventory；父级状态只能由其 child subject 派生，禁止手写 Closed。P0 的 transition guard 必须拒绝缺失、重复、额外、rekey 或“删除旧 ID 再新增 ID”绕过；P1 只可把既有 `MappedPendingClosure` 转为 `MappedGameplayClosed` 或 `MappedProducerRetired`，集合外 observation 不得计入 11 条完成数。
- [ ] `MappedGameplayClosed` 与 `MappedProducerRetired` 必须按 subject 派生而非手写父级状态。Member subject 只有在至少一条 production-reachable writer/binding、完整 `member → typed consumer edge → consumer system → actual scheduler registration` 链、以及专属 observable differential test 均成立时才 Closed；ProducerBinding subject 还必须保留精确 branch/site、`actual_member_id == expected_member_id`、完整 branch→production-entry/caller/system/schedule 链和目标 Member 的 closure。每个 Closed subject 必须绑定不可复用的 `DifferentialEvidence` test target/case selector；表驱动测试可覆盖多个 subject，但每个 case 必须独立可解析。Retired 的 Member 必须证明所有 production-reachable writer/binding 与 typed consumer edge 均为空并具迁移/水合证据；Retired 的 ProducerBinding 只证明该精确 site 已删除/禁用且 site→production 路径为空，不得退役同 member 的其他合法 site 或共享 schedule。HUD、schema、persistence、client store、测试或无 production caller helper 均不满足 closure。
- [ ] r8 #5 只能由 `r8_5_closed`（名称可等义）从 inventory 派生，禁止 aggregate Closed flag：expected set 精确含 13 个 `InsightModifiers` Member 与 13 个 ProducerBinding（共 26 个），每个 child 必须是有效 Closed 或有效 Retired，Member/Binding 一对一 pairing invariant 成立。13 Binding 只可来自 `apply_choice` 的三个指定 effect arm 与 `apply_tradeoff_cost` 的十个指定 scalar-writing arm，排除 practices marker、r7/阵法字段和 `composure_recover_mul`。任一 child 缺失、重复、额外、Pending 或改绑均由 guard 拒绝。
- [ ] P1 在同一最终 PR 中按 Alchemy/Insight、Iron Cocoon、jump 三组完成 production producer/caller、consumer、schedule 与 observable differential，并把对应 subject 原子转为 `MappedGameplayClosed`；若删除语义，则删除/禁用精确 producer subject、迁移持久状态后转为 `MappedProducerRetired`。Closed/Existing 必须引用权威 producer site、producer reachability、member→consumer edge、consumer system、真实 schedule site 与逐项 differential test ID；`DormantNoProducer` 必须由同一全量机械库存证明无 production-reachable writer。
- [ ] P0 冻结 exact `UnmappedObservation` stable-ID/producer-site allowlist；`InsightModifiers.composure_recover_mul` 是已知必须出现的独立精确条目。§“未映射域观察”六行只是人工 domain summary，不是六个可机械放行的 wildcard ID；动态字符串域也不因该表自动进入 typed inventory。新增字段、variant 或 production writer branch 默认失败；普通 manifest/allowlist 更新不得接纳新 orphan。未映射观察不得生成 gameplay consumer/migration 或成为本 plan 的交付物。
- [ ] 每个 production writer 使用稳定 typed producer-site ID，同函数不同 branch 不得折叠；mapped 11 条 finding 的 producer subject 必须精确对账，r8 #5 必须与上表 13 项相等。P0 不要求把未映射动态字符串域做全仓 typed migration。
- [ ] P0 门禁测试放入 `server/src/test_coverage_guards.rs`（由 `server/src/lib.rs` 的 `#[cfg(test)] mod test_coverage_guards` 加载），并由普通 `cargo test` 执行。PR 验证只能运行在标准、非特权的 `pull_request` CI 语境（现有 Bong e2e 亦如此）；不得让具特权 token 或 secrets 的环境执行 PR 代码。negative fixture/mutation-style 验收至少覆盖字段/variant/branch 的新增删除改名、actual-member 改绑、producer caller/entry、consumer edge/system/scheduler registration 删除、Retired 证据删除、differential selector 删除/改名/忽略/错绑、普通 allowlist 更新接纳新 orphan，以及 r8 #5 child 缺失/额外/改绑/Pending。每个 mutation 必须由 anti-orphan/transition guard 自身以稳定 diagnostic 拒绝；普通 rustc、解析、链接或无关测试失败不计门禁命中。另 pin：多个字段共享一个真实 registration、退役一个 member 不影响共享 registration、退役一个 binding 不影响同 member 其他合法 site、consumer 无 registration、orphan writer 无 production caller、虚构逐字段 registration 与 aggregate parent 手写 Closed。
- [ ] 可核验 symbol：`MODIFIER_CONSUMER_MANIFEST`、`MODIFIER_EFFECT_COMPLETION_CANONICAL_FINDINGS`、`CanonicalFindingId`、`ModifierContractSubject`、`ModifierMemberId`、`ModifierProducerBindingId`、`ModifierProducerSiteId`、`ModifierProducerCallerId`、`ModifierProductionEntryId`、`ModifierConsumerEdgeId`、`ModifierConsumerSiteId`、`ModifierSystemId`、`ModifierScheduleSiteId`、`DifferentialEvidence`、`modifier_consumer_manifest_stays_current`、`modifier_producer_sites_stay_current`、`modifier_producer_reachability_matches_production_app`、`modifier_consumer_sites_stay_current`、`modifier_schedule_sites_match_production_app`、`modifier_consumer_graph_supports_shared_schedule`、`jinzhongdan_negative_binding_tracks_actual_and_expected_member`、`canonical_lifecycle_transitions_are_pinned`、`canonical_stable_id_rekey_cannot_bypass_transition_guard`、`differential_evidence_is_subject_unique`、`r8_5_closed`、`r8_5_subject_set_is_exact`、`retiring_one_member_preserves_shared_schedule`、`retiring_one_binding_preserves_other_member_sites`、`consumer_without_registration_fails`、`producer_without_production_caller_fails`、`fabricated_per_member_registration_fails`、`mapped_pending_requires_closure_target`、`gameplay_closed_requires_scheduled_consumer`、`mapped_producer_retired_requires_subject_specific_unreachability_and_migration`、`existing_gameplay_requires_full_evidence`、`dormant_no_producer_requires_empty_reachability`、`ordinary_allowlist_update_cannot_admit_new_orphan`、`unmapped_observation_is_not_consumed_by_this_plan`（名称可等义）。

## P1 — mapped gameplay closure

### P1-A — Alchemy + mapped Insight

- [ ] 冻结 `ContaminationBoost` 的 magnitude、duration、stack/refresh/expiry 与 ledger 接缝；实现必须定义 magnitude 的单位、合法 finite 闭区间和负值/越界/NaN/±Infinity 策略，以及 duration 的 0/上下界/溢出策略。错误输入必须 reject 或 fail-closed，且 effect 状态与 qi ledger 均无部分副作用；`contamination_tick` 通过 canonical qi ledger 产生可观察差分。
- [ ] `contamination_boost_lifecycle_is_pinned`（名称可等义）以表驱动状态机覆盖 duration=0、合法最小/最大与等号、溢出、首次 apply、同源/异源重复 upsert、stack/cap、refresh 前后、expiry tick 前/等号/后、到期清理恰好一次及到期后回 neutral。每一步同时断言 effect 状态、`WorldQiAccount` 与 `QiTransfer`：非法输入或失败转换全零副作用，合法转换不得重复记账，到期后不得继续产生该 effect 的 ledger 差分。实现只黑盒 pin 已确定的 source identity 语义，不借此迁移 `source_pill` 或扩张 status-origin scope。
- [ ] JinZhongDan negative slot 改为语义明确的负面 regen effect；实现须冻结 `neg_scale` 的单位、合法 finite 闭区间、组合顺序及负值/越界/NaN/±Infinity 策略；测试覆盖下界/等号/上界、0/默认/max、非法值零副作用、到期回 neutral 与重复 upsert。
- [ ] r7 五字段与 r8 #5 的 13 个冻结字段按表中 gameplay domain 接入唯一 effective helper；P0 若证明某 writer production 不可达，也必须在本批删除/禁用该 writer并迁移状态，不得改标 `UnmappedObservation` 消项。
- [ ] 实现逐字段冻结单位、neutral、finite 区间、add/mul 顺序、累计上限、消费时点、持久化/水合/reset。表驱动 pin 覆盖合法下界/等号/上界、越界、NaN/±Infinity、组合顺序/cap、持久化往返、水合与 reset/到期回 neutral；r7 五字段与 r8 #5 十三字段各自绑定专属 `ModifierConsumerEdgeId` 与 differential test ID，`ModifierScheduleSiteId` 可由多个字段共享，但每个字段都须机械证明 member→consumer→system→真实 registration 的完整 production-reachable 链。逐字段以相同 gameplay 输入、仅该 modifier 在 neutral/非 neutral 间变化，断言目标 domain 可观察差分且非目标后果不变，测试表与 18 字段精确集合对账。qi gain/drain 继续断言 `WorldQiAccount` / `QiTransfer` 守恒。
- [ ] 可核验 symbol：`ContaminationBoost`、`contamination_tick`、`CombatPillKind::JinZhongDan`、`QiRegenSlowed`、`insight_qi_regen_multiplier`、`effective_breakthrough_bonus`、`effective_overload_threshold`、`effective_vortex_delta`、`effective_vortex_flow_speed`、`mapped_insight_modifier_contract_is_pinned`、`mapped_insight_modifier_changes_gameplay`（名称可等义）。

### P1-B — Iron Cocoon wound grade + effective flow

- [ ] `combat::resolve_attack_intents` 内建立唯一 typed `CanonicalWoundSink`（名称可等义）：该 sink 是本 pipeline 唯一允许最终构造/写入 `Wound` 与派生后果的位置；参与该 pipeline 的 damage producer 必须调用它。门禁 pin sink **恰好一个**、production 可达，并拒绝重复 sink 与 sink 外直接写入。
- [ ] sink 只处理**一个 `AttackIntent` event occurrence 对应的 primary wound**：`raw hit → armor → pure effective severity/grade → deterministic downgrade → health/bleeding/contamination/meridian/event consequences`。mapped 三个 wound modifier 只在这里消费；先完成 modifier 输入与目标状态的全部可失败校验，再以一个已计算的 effective result 驱动本范围后果，避免 grade/severity 与后果分叉。`EventReader<AttackIntent>::read_with_id()` 暴露的 world-local `EventId`（或等义的调用方注入 occurrence/roll input）只用于该事件实例的 deterministic fracture decision；同一事件不会在下一次 reader update 重放，两个 payload 相同但分别 send 的事件仍是两个合法 hit，禁止按 payload 去重。
- [ ] 实现须冻结本 sink 的 occurrence/roll 输入、effective-grade 纯函数、失败前置校验与本范围提交顺序；明确排除 durable hit identity、persistent settled-hit ledger、retention/cleanup、hydration/restart replay、cross-restart identity reuse、payload deduplication 与 resolver 全局 rollback。若未来需要这些能力，必须另立 combat-persistence/transaction canonical finding，不得借 r8 #2 偷渡。测试至少 pin：单 event 单次结算、同一 reader 后续 update 不重复结算、两个相同 payload 分别产生两个伤口、deterministic roll 对同一 occurrence 输入稳定、modifier 非法/目标缺失在 primary-wound mutation 前 fail-closed 且无本范围副作用。
- [ ] 实现分类型冻结合同：`bruise_threshold_multiplier` 与 `fracture_downgrade_chance` 定义单位、neutral、合法 finite 闭区间、非法值 fail-closed/reject、组合顺序/cap；`cut_pierce_downgrade` 与 `scar_forged_flow_bonus` 是 bool marker，只允许 `false/true`，并覆盖非法反序列化。四字段共同 pin 持久化/水合/reset/到期与 active→inactive 回 neutral；另单独冻结 ScarForged marker 生效时 effective-flow 数值倍率的依据、finite 区间与组合/cap。表驱动测试按实际类型覆盖浮点下界/等号/上界、越界、NaN/±Infinity，以及布尔两态和非法 wire 值。
- [ ] 实现冻结唯一 wound-grade 表，对每个 wound kind/grade 的 finite threshold 使用相邻可表示浮点值 `next_down(threshold)`、`threshold`、`next_up(threshold)`（或等价 bit-level 生成）锁定等号归属，禁止使用未定义的 `threshold±ε`。测试先断言阈值 finite、相邻值严格有序且未跨越相邻 grade 阈值；若端点不存在某一侧相邻值，实现必须明确该端点策略。三点均断言最终 grade 与 health、bleeding、contamination、meridian、event 的完整派生后果。
- [ ] `effective_meridian_sum_rate` 只在 `scar_forged_flow_bonus` active 时对 `ActiveScarCircuits` 涉及的去重经脉应用实现确定的倍率；共享经脉只加成一次，不持久改 `Meridian.flow_rate`。
- [ ] 本批不声称迁移全仓所有历史 wound/health writer；未经过 `resolve_attack_intents` 的旁路属于附录观察，须独立 canonical finding 才能扩 scope。
- [ ] 可核验 symbol：`CanonicalWoundSink`、`canonical_wound_sink_is_unique`、`effective_wound_grade`、`wound_grade_thresholds_are_pinned`、`cocoon_fracture_roll`、`effective_meridian_sum_rate`、`iron_cocoon_downgrade_changes_full_wound_consequences`、`event_occurrence_is_settled_once_without_payload_dedup`、`scar_forged_bonus_only_applies_to_active_circuits`（名称可等义）。

### P1-C — `jump_height_multiplier` authority

- [ ] P1-C 只实现一条互斥 runtime 路线：server-authoritative，或 client-hook + server-validation。§8 的设计分析仅说明两条路线的物理和状态边界，不是 `/consume-plan` 的前置条件；无论选择哪条，字段都必须明确表示 apex-height multiplier 或 initial-velocity multiplier，冻结合法 finite 区间与 apex 容差，并锚定 MC 1.20.1 的确切上游版本、映射、类、方法与离散物理。非法值统一 fail-closed 到 `1.0` 或拒绝。
- [ ] 选择 **server-authoritative** 时，P1-C 交付 server movement/velocity authority、production schedule、非法纵向速度拒绝与真实 client/bot apex e2e；client ACK/proto/store/反向生产链均为 N/A，P0 不得以它们阻塞，也不得遗留无 runtime reader 的 client schema/store。
- [ ] 选择 **client-hook + server-validation** 时，从 `server/src/network/derived_attrs_emit.rs` 的 `DerivedAttrs` query、payload 写入与 production send schedule 开始，贯通 `DerivedAttrsSyncV1`、proto/generated、`DerivedAttrsHandler`/version-aware jump store、非 mixin helper、jump hook 与 disconnect reset；必须有实际加载的 server movement validation consumer，在每个受影响 movement tick 读取 authoritative tuple 与当前位置/速度/`OnGround`，按冻结的 MC 物理 envelope/tolerance reject/correct/fail-closed。消息采用 session generation、单调 revision/effective tick 的全量 authority state；active→neutral、到期或断线时必须发布更高 revision 的 `jump_height_multiplier=1.0`。`NeutralAppliedAck` 只能证明完整 authority tuple 已安装，`JumpAuthorityTupleReport` 只能报告观测 tuple；ACK 经命名 `ClientRequestProtocol` encoder、`ClientRequestSender` send site、Rust `ClientRequestV1` decoder、`CustomPayloadEvent` dispatch、typed ACK/report resource/event 与真实注册 server consumer 到达验证链，report 不得替代 ACK。丢包、错 tuple、旧 revision/session、重复/乱序、超时或断线都不得错误放行 active jump 或复活旧状态。
- [ ] 仅在 client-hook + server-validation 路线中，`DerivedAttrsSyncV1` 必须以 `agent/packages/schema/src/server-data.ts` 为唯一 TypeBox source of truth，并在实际 server-data wrapper/registry 中注册；同一变更链更新 `agent/packages/schema/src/schema-registry.ts`，提交由 `agent/packages/schema/src/generate.ts` 派生的 `agent/packages/schema/generated/server-data-derived-attrs-sync-v1.json` 等生成 JSON Schema，提交 `agent/packages/schema/samples/` 下命名的 valid/neutral/reset/invalid shared samples，并运行 `npm run generate:check` 后用 `cd agent && npm run build -w @bong/schema` 重建 `dist/`。必须逐项对拍 `TypeBox source → generated JSON Schema → shared samples → rebuilt @bong/schema export → server/src/schema/combat_hud.rs:205-220 → proto/bong/envelope.proto:1706-1718 与 generated proto → DerivedAttrsHandler/store/hook` 的字段、类型、位宽和 unknown-field 规则；samples 同时通过 TypeBox 与 Rust serde round-trip。选择 server-authoritative 路线时，该 client schema/store/ACK 链为 N/A，且不得留下无 runtime reader 的 artifact。
- [ ] 两路线共同以真实 velocity/apex 而不是 payload/store 值验收 multiplier `1.0`、中间、上限与非法输入，覆盖 active→neutral、到期/reset、断线/实体重建/重登、水合后旧状态不复活及转换边界前后起跳。server 路线另证明 production schedule 每次读取当前权威倍率且下一次起跳恢复 `1.0`；client 路线另覆盖旧 active/neutral 的乱序与重复、ACK/report 语义混用、反向 production wiring seam 缺失、丢失/重传、revision 耗尽/回绕尝试、server/client 分别重启与跨 session 旧消息。
- [ ] 可核验 symbol：`sanitized_jump_height_multiplier`、`effective_jump_velocity`、`jump_physics_golden_vectors_match_mc_1_20_1`、`guangbo_jump_height_changes_observed_apex`；仅选择 client-hook + server-validation 路线时，另含 `DerivedAttrsSyncV1`、`DerivedAttrsStore`、`jump_modifier_resets_on_disconnect`、`jump_modifier_neutral_revision_clears_same_session`、`neutral_applied_ack_is_distinct_from_tuple_report`、`client_request_ack_reverse_wiring_is_production_reachable`、`derived_attrs_sync_typebox_contract_is_authoritative`、`derived_attrs_sync_generated_artifacts_are_fresh`、`derived_attrs_sync_samples_match_rust_wire`、`derived_attrs_sync_dist_export_is_current`（名称可等义）。

## §8 设计分析（供实现者与人工审阅参考）

以下条目记录 P0/P1 实现时需要明确的领域语义与边界。它们不是 promotion、`/consume-plan`、CI 或阶段推进的前置条件，不定义 PR、审批或外部流程。

1. `ContaminationBoost` 的 magnitude 单位/合法 finite 闭区间/非法值策略、duration 的 0/上下界/溢出策略、stack/refresh/expiry 与 ledger 接缝是什么？
2. JinZhongDan 的负面 kind、基础强度，以及 `neg_scale` 的单位、合法 finite 闭区间、非法值策略与组合公式是什么？
3. r7 五字段与 r8 #5 十三字段逐项的单位、neutral、finite 区间、组合/累计、消费时点、持久化/水合/reset 是什么？
4. canonical wound grade 的阈值/等号归属是什么；两个浮点 wound modifier 的单位、neutral、合法 finite 闭区间、非法值策略、组合/cap 与生命周期是什么；`cut_pierce_downgrade` bool marker 的两态、非法反序列化与生命周期是什么；一个 `AttackIntent` event occurrence 的 occurrence/roll input、deterministic fracture decision、effective-grade 纯函数与本范围失败前置校验/提交顺序如何定义？durable hit identity、persistent settled-hit ledger、retention/cleanup、hydration/restart replay、payload dedup 与 resolver 全局 rollback 不属于本 finding；如需这些能力，另立 combat-persistence/transaction finding。
5. `scar_forged_flow_bonus` bool marker 的两态、非法反序列化与生命周期是什么；marker 生效时 effective-flow 倍率的 canonical 数值依据、合法 finite 闭区间、非法值策略、适用经脉、组合/cap 与持久化/水合/reset/到期语义是什么？当前代码注释中的 +5% 不能在无明确设计时自动视为正典。
6. jump 选择哪条 authority 路线；字段表示 apex 还是 velocity、合法范围、权威 MC 1.20.1 源码/映射锚点、离散公式/容差是什么？若选择 client 路线，还须明确 session/revision/effective-tick 的 wire 类型与位宽、比较算法、revision 耗尽且禁止回绕的策略、server/client 重启后的 session 唯一性、explicit-neutral 的 ACK prepare/commit/release、重传/超时合同，以及 ACK 对 `(session, revision, effective tick)` 的精确匹配规则。
7. P0 使用声明宏、Rust AST 或何种同等机械来源；如何从 production 代码与真实 scheduler registration 导出 producer branch、member→consumer edge、consumer system 与 system→schedule edge，并防止伪造逐字段 registration？

## 未映射域观察（非本 plan 交付物）

以下是调研中发现的风险线索，**不是 P0/P1 mandatory deliverable，不阻塞本 plan 归档，也不得由本 plan 实施 agent 顺手迁移**。本 plan 不为这些观察定义任何实施流程。

| 未映射域 | 当前证据 | 本 plan 处置 |
|---|---|---|
| Alchemy 动态 side-effect tags | `AlchemyBuff(String)` 隐藏 recipe 动态 tag；当前资产约 35 tag / 80 config site | 本 plan 不处置 |
| perception keys | `UnlockPerception.kind` / `UnlockedPerceptions` 使用自由字符串，producer/reader key 不配对 | 本 plan 不处置 |
| Insight trigger/fired keys 与 no-op variants | `trigger_id`、`fired_triggers` 及部分 apply arm 是字符串/no-op | 本 plan 不处置 |
| status origin | `ActiveStatusEffect.source_pill: Option<String>` 参与 stack/expiry/cleanup | 本 plan 不处置 |
| generic-talent discriminator | stat/op/group/color stringly config、unknown→Lung、转换 `.ok()` 静默过滤 | 本 plan 不处置 |
| repo-wide wound writer migration | projectile/AoE/collision 与 healing/revival/init 等 writer 尚未统一分类 | 本 plan 不处置 |

P0 只允许把这些六类 domain summary 标为“未映射域观察”并验证“未被本 plan 消费”；它们不是可放行的 wildcard。机械 manifest 还必须单独登记 `InsightModifiers.composure_recover_mul` 这一 exact-HEAD `UnmappedObservation` stable ID；不得以 inventory 名义把六类 summary 或该精确观察重新变成本 plan 交付物。

## §10 实施边界

骨架 promotion 为 `docs/plan-<name>.md` 后，现有 `/consume-plan <plan-name>` 只按阶段总览中的 `P0`、`P1` 行工作：在一个 worktree、一个 implementation branch 和一个最终 PR 中，先完成 P0，再完成 P1。`P1-A`、`P1-B`、`P1-C` 只是 P1 内的技术分组，不是额外 phase row、独立 PR、分支或 merge boundary。

本 plan 不定义额外 checkpoint、消费协议、阶段状态机、决策 PR、阶段合并边界、外部协调机制、workflow/registry 义务或计划自定义归档门；也不授权修改 `skills/consume-plan/SKILL.md`、command loader、workflow 状态存储或其他 plan。§8 的设计分析只供实现者和人工审阅者阅读，相关技术选择必须在实际 P0/P1 代码和测试中体现，但不会阻止 consumer 按 P0/P1 执行。

P0/P1 完成后的 `## Finish Evidence` 与 active→finished 流转沿用现有 consumer 的普通收尾行为。所有 PR 代码验证保持标准、非特权的 `pull_request` CI 语境。
