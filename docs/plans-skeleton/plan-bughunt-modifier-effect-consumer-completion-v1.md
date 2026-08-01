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
- **inventory 不自动扩 scope**：P0 若发现未映射 orphan，只登记为 `UnmappedObservation`；必须先建立独立 canonical finding、owner 与 successor，后续 plan 才能消费。它不阻塞本 plan 完成，也不得被静默升级成本 plan 的迁移或 gameplay 实现。
- **不冒充闭环**：HUD、schema、persistence、client store、测试内读取或无 production caller 的 helper 都不算 gameplay consumer。

## 接入面

- **进料**：`server/src/alchemy/side_effect_apply.rs:20-83` 与 `server/src/alchemy/pill.rs:632-642` 生产 mapped `StatusEffectKind`；`server/src/cultivation/insight_apply.rs:24-254` 写 mapped `InsightModifiers`；`server/src/combat/baomai_v4/iron_cocoon.rs:99-143` 写四个 mapped `DerivedAttrs` 字段；`server/src/combat/body_conditioning.rs:157-167` 写 `jump_height_multiplier`。
- **出料**：污染排异、回气/突破/过载/颜色/涡流 gameplay loop、`combat::resolve_attack_intents` 的 canonical wound sink、effective meridian flow，以及唯一权威 jump runtime 必须读取对应 mapped modifier。
- **共享类型 / event**：复用 `StatusEffects`、`StatusEffectKind`、`InsightModifiers`、`DerivedAttrs`、`MeridianSystem`、`ActiveScarCircuits`、`Wound`/`WoundKindProfile`、`DerivedAttrsSyncV1`；禁止另造 parallel modifier component 或持久改写派生倍率。
- **跨仓库契约**：P0 与 P1-A/P1-B 以 server 为主，P0 不得要求任何 client ACK/wire；P1-C 按 §8.1 选择 server-authoritative 或 client-hook 路线。仅在选择 client-hook 路线时，才要求 server emitter → proto/generated → handler/store → 非 mixin helper → jump hook 的单向链，以及明确的 client → server 反向生产链：`DerivedAttrsHandler` 原子安装 authority tuple 后生成 `NeutralAppliedAck`，命名的 `ClientRequestProtocol` encoder 经 `ClientRequestSender` 的 `bong:client_request` send site 发出，Rust `ClientRequestV1` schema/decode/dispatch 进入 typed report/ACK event 或 authority resource，再由真实注册且排在 jump validation 之前的 server consumer 消费。`NeutralAppliedAck`（仅证明该 tuple 已安装）与 `JumpAuthorityTupleReport`（仅观测当前 tuple）是不同 wire/type/handler 语义，report 不得替代 ACK 或单独驱动 authority transition；删除任一 seam 必须命中 guard 自身稳定 diagnostic，端到端测试必须走真实 transport 而非 direct-call handler。server-authoritative 路线不得新增这些 client ACK/proto/store seam，并将对应 client-wire 验收明确记为 N/A。
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

`composure_recover_mul` 不属于 r8 #5：canonical r8 只确认其 sibling `Cultivation.composure_recover_rate` 有 gameplay consumer；`InsightModifiers.composure_recover_mul` 本身仍无 reader，P0 必须将它登记为 `UnmappedObservation`，不得误标 `ExistingGameplay`，消费或清理前须另立 canonical finding、owner 与 successor。`practices` 动态 marker 及本表外字段同样不因 P0 inventory 自动进入 mandatory scope。

## 第一性验真（`origin/main @ de75f14e43daf1105ea978c43d187acbb7f12f14`，2026-07-29）

1. `ContaminationBoost` 可生产、可 upsert、可显示、会到期，但 `server/src/cultivation/contamination.rs:97-205` 不读 `StatusEffects`。
2. JinZhongDan negative slot 在 `server/src/alchemy/pill.rs:632-642` 仍生产正向 `QiRegenBoost(0.001)`；现有 consumer 按 `1 + magnitude` 增益回气。
3. r7 五字段与 r8 #5 冻结的 13 个子字段由 `server/src/cultivation/insight_apply.rs:24-254` 写入；production reachability 与 reader 缺口须由 P0 逐项登记，不能以聚合名称略过。
4. Iron Cocoon 四字段由 `server/src/combat/baomai_v4/iron_cocoon.rs:99-143` 写入；`combat::resolve_attack_intents` 与 `MeridianSystem::sum_rate` 未消费。
5. `jump_height_multiplier` 有 producer/reset，但 `server/src/network/derived_attrs_emit.rs:76-90`、`DerivedAttrsSyncV1`、client store 与真实 jump 链均无 consumer。
6. `server/src/test_coverage_guards.rs` 现有 event reader/writer 扫描不能证明上述 mapped modifier 已有 production consumer。

## P0 — mapped anti-orphan contract

- [ ] 建立 greppable `ModifierConsumerContract` / `MODIFIER_CONSUMER_MANIFEST`（名称可等义）。以 §8.1 选定的唯一声明宏、Rust AST 或同等机械来源，穷举 `DerivedAttrs` 字段、`InsightModifiers` 字段、`StatusEffectKind` variant 与每个 production producer branch；另从 production 代码独立导出 producer reachability、typed consumer edge、consumer system 与 app 中实际 scheduler registration（排除 `cfg(test)`、测试调用和孤立 helper）。新增/删除/改名字段或 variant、新增/删除/改绑 writer branch、edge/caller/registration 未登记即失败。
- [ ] producer 与 consumer 分成两张可对账的 typed graph：`ModifierProducerSiteId → ModifierProducerCallerId/ModifierProducerSystemId → ModifierProductionEntryId/ModifierScheduleSiteId` 证明该精确 branch 能从真实生产入口触发；`ModifierMemberId → ModifierConsumerEdgeId(ModifierConsumerSiteId) → ModifierSystemId → ModifierScheduleSiteId` 证明 gameplay read 已由生产 app 装载。非测试源码中“存在 writer/helper”不等于 production reachable；缺 caller/entry/schedule 的 producer 不得进入 Closed/Existing。一个 member 可有多个 consumer edge，多个 member/system 可共享同一真实 registration，禁止伪造逐字段 registration。
- [ ] canonical lifecycle 记录必须显式区分 `ModifierContractSubject::Member` 与 `ModifierContractSubject::ProducerBinding`（名称可等义）。后者由稳定 `ModifierProducerBindingId` 绑定 `CanonicalFindingId + ModifierProducerSiteId + actual_member_id + expected_member_id + lifecycle + evidence`，同函数不同 branch 不得折叠。r6 #1 在 P0 精确登记 JinZhongDan negative branch 的 `actual=QiRegenBoost`、`expected=QiRegenSlowed`、`MappedPendingClosure`；P1-A 只能在代码 branch 与该记录同 PR 原子改绑且 expected member 的 consumer/schedule/differential 全链成立后转 Closed，或删除/禁用该精确 branch 后转 Retired。其他合法 `QiRegenBoost` producer site 与其共享 consumer/schedule 始终独立保持 `ExistingGameplay`，不得为关闭 r6 #1 全局退役该 member。
- [ ] **Baseline bootstrap 是 P0 的真实前置，不是本 PR 的自举副作用。** 在任何实现 PR 可进入 P0 前，必须先由独立、受保护主线上的人工审核 bootstrap 变更，从归档的 r6/r7/r8 canonical source blob、r8 #5 derivation source blob 与固定 derivation version 建立 `CanonicalLifecycleBaseline`；该 bootstrap 变更不属于本 plan PR。若 `github.event.pull_request.base.sha` 对应 tree 尚无该 baseline，P0 必须保持 `⬜` 并以稳定 `MODIFIER_GUARD_BASELINE_NOT_BOOTSTRAPPED` fail-closed，禁止从 current HEAD、plan 表格或本 PR 新增文件创建 baseline。baseline 存在后，P0 必须从每个 PR 的 exact `github.event.pull_request.base.sha` tree 读取不可变 `CanonicalLifecycleBaseline`，验证 base SHA 是目标分支历史中本 PR 的真实 parent/base，并记录 archived source blob、derivation version 与 tuple digest；current HEAD 只能提交待比较 manifest/evidence，不能修改、选择或同步重写 baseline。缺失 base object、base/parent 不匹配、authority tuple/hash 不匹配、删除旧 baseline 后重建、改名换 ID 或“旧 ID 删除 + 新 ID 新增”均由 transition guard 自身以稳定 `MODIFIER_GUARD_BASELINE_AUTHORITY_MISMATCH` / `MODIFIER_GUARD_LIFECYCLE_TRANSITION` diagnostic 拒绝。后续 P1/Closure 只允许既有 stable ID 发生 `MappedPendingClosure → MappedGameplayClosed|MappedProducerRetired` 状态与 evidence 转换；集合外 subject 不得计入 11 条完成数。`canonical_lifecycle_transitions_from_base_tree_are_pinned`、`canonical_stable_id_rekey_cannot_bypass_transition_gate` 与 `current_manifest_cannot_self_authorize_baseline`（名称可等义）必须分别 pin 合法 transition、rekey/delete+add 拒绝及 base/current authority mismatch。
- [ ] `MappedGameplayClosed` 与 `MappedProducerRetired` 必须按 subject 派生而非手写父级状态。Member subject 只有在 baseline stable ID/source tuple 未变、至少一条 production-reachable writer/binding、完整 `member → typed consumer edge → consumer system → actual scheduler registration` 链、专属 observable differential test 且其全部 baseline bindings 均闭合时才 Closed；ProducerBinding subject 还必须保留精确 branch/site、`actual_member_id == expected_member_id`、具备 branch→production-entry/caller/system/schedule 链、目标 Member 已闭合与专属差分证据。每个 Closed subject 的差分证据必须是 stable typed `DifferentialEvidence`（名称可等义）：一条记录固定绑定一个 `ModifierContractSubject` stable ID、一个 `ModifierConsumerEdgeId`、一个 observable domain、一个 test target 与一个不可被其他 subject 复用的 test selector/case ID；同一表驱动测试可覆盖多个 subject，但必须为每个 case 提供可独立解析且不可复用的 selector。它不是普通 manifest 字符串：guard 必须在 current exact HEAD 从实际 test inventory 解析 target/selector，确认测试存在、未 `#[ignore]`、未被 `cfg` 排除且会被目标 trusted step 收集；再与 trusted protected workflow 产生的 `DifferentialEvidenceReceipt` 对拍，该 receipt 必须绑定 current HEAD、protected verifier/workflow SHA、target/selector、实际执行成功结果及同一 subject/edge/domain。任一缺失、删除/改名、未收集、ignore/cfg 排除、陈旧 HEAD/receipt、跨 subject 复用，或 subject/member/binding/edge/domain 错绑均不得 Closed。Retired 的 Member 必须证明所有 production-reachable writer/binding 与 typed consumer edge 均为空，并有迁移/水合证据；Retired 的 ProducerBinding 只证明该精确 site 删除/禁用且 site→production 路径为空，并保留迁移证据，不得因此退役同 member 的其他合法 site 或共享 schedule。HUD、schema、persistence、client store、测试或无 production caller helper 均不满足 closure。
- [ ] r8 #5 只能由 `r8_5_closed(base,current)` 派生，禁止可手写 aggregate Closed flag：base-derived subject set 必须精确含 13 个 `InsightModifiers` Member 与 13 个 ProducerBinding（共 26 个），current set 与 base 精确相等；每个 child 必须是有效 Closed 或有效 Retired，每个非 Retired binding 保持 base 的 `actual_member_id`/`expected_member_id` mapping 且 `actual_member_id == expected_member_id`，Member/Binding 一对一 pairing invariant 成立。r8 #5 的 13 Member 必须恰为表列字段；13 Binding 只可来自 `apply_choice` 的三个指定 effect arm 与 `apply_tradeoff_cost` 的十个指定 scalar-writing arm，排除 practices marker、r7/阵法字段和 `composure_recover_mul`。任一 child 缺失、重复、额外、Pending、改绑或 parent Closed 而 child 未闭合均拒绝归档；其他 aggregate/member/producer-binding finding 同样必须由其完整 expected subject set 派生 closure。
- [ ] P1 checkpoint 必须在同一最终 PR 的对应顺序内接通 production producer/caller + consumer + schedule + observable differential，并把本批 subject 原子转为 `MappedGameplayClosed`；若决议删除语义，则删除/禁用精确 producer subject、迁移持久状态后转为 `MappedProducerRetired`。Closed/Existing 必须引用权威 producer site、producer reachability、member→consumer edge、consumer system、真实 schedule site与逐项 differential test ID；`DormantNoProducer` 必须由同一全量机械库存证明无 production-reachable writer。分类与两张 typed graph 必须和实际 production entry/scheduler 加载集合精确对账。
- [ ] P0 在 exact HEAD 冻结 exact `UnmappedObservation` stable-ID/producer-site allowlist；`InsightModifiers.composure_recover_mul` 是已知必须出现的独立精确条目。§“未映射域观察”六行只是人工 domain summary，不是六个可机械放行的 wildcard ID；动态字符串域也不因该表自动进入 typed inventory。基线后新增字段、variant 或 production writer branch 默认 CI 失败，普通 manifest/allowlist 更新仍必须失败；只有先由独立 PR 建立 canonical finding、owner 与 successor 后才能建立受控 subject。未映射观察不得生成 gameplay consumer/migration 或本 plan 归档前置。
- [ ] 每个 production writer 使用稳定 typed producer-site ID，同函数不同 branch 不得折叠；mapped 11 条 finding 的 producer subject 必须精确对账，r8 #5 必须与上表 13 项相等。P0 不要求把未映射动态字符串域做全仓 typed migration。
- [ ] P0 门禁测试放入 `server/src/test_coverage_guards.rs`（由 `server/src/lib.rs` 的 `#[cfg(test)] mod test_coverage_guards` 加载），并由**受保护主线上的 trusted gate workflow** 的 `Server stage (cargo test)` 实际执行。该 verifier 与 workflow 必须从 bootstrap 后的 protected main/ref 读取，而不是 checkout 本 PR 的 workflow/verifier 副本；PR under review 修改 `.github/workflows/**`、guard/verifier source 或测试 fixture 时，不得改变本次 gate 的判定逻辑。trusted workflow 必须以 `pull_request_target` 或等价的受保护 workflow 读取 PR 的 base SHA/HEAD object，并仅把 PR tree 当作待比较输入。若实现 PR 合法需要修改 verifier、workflow 或 baseline schema，先由独立 trusted-gate PR 在受保护主线更新并完成 bootstrap/兼容性审查，随后本 PR 以新的 trusted workflow/base authority 重跑；被审 PR 不得同时更新并自认证 gate。negative fixture/mutation-style 验收必须分别执行：字段与 enum variant 的新增/删除/改名；同函数新增 writer branch、branch actual-member 改绑、producer caller/production entry 删除；consumer edge、system 或 scheduler registration 删除；Retired 证据删除；差分测试删除/改名、`#[ignore]`、`cfg` 排除、未被 target runner 收集、陈旧 receipt、跨 subject 复用 selector/case ID、Member/ProducerBinding/edge/domain 错绑；新增 orphan 后仅做普通 manifest/allowlist 更新；r8 #5 缺一个 Member、缺一个 ProducerBinding、额外 binding、binding `actual_member_id`/`expected_member_id` 改写、child Pending 却手写 parent Closed、base/current authority tuple 不同、删除 baseline 后重建。每个 mutation 必须先证明 fixture 仍可编译/装载，再精确断言由 anti-orphan/transition guard 自身以稳定 diagnostic code（例如 `MODIFIER_GUARD_UNMAPPED_PRODUCER`、`MODIFIER_GUARD_LIFECYCLE_TRANSITION`、`MODIFIER_GUARD_BASELINE_AUTHORITY_MISMATCH`、`MODIFIER_GUARD_DIFFERENTIAL_EVIDENCE`，名称可等义）拒绝；普通 rustc/解析/链接/无关测试失败不计门禁命中，e2e 还须检查该 guard test 名与 diagnostic 出现在对应 step 输出。另 pin：多个字段共享一个真实 registration 通过；退役一个 member subject 时共享 registration 仍服务其他 consumer 通过；退役一个 producer-binding subject 时同 member 的其他合法 site 通过；consumer 无 registration、orphan writer 无 production caller、registration 残留但 member 无 consumer edge、虚构逐字段 registration、新 orphan+普通 allowlist 更新、aggregate parent Closed 而 child 缺失、current manifest 自授权 baseline，以及差分证据删除/改名/ignore/cfg 排除/未收集/陈旧 receipt/复用或错绑分别由预期 guard diagnostic 失败。另必须以真实 Active→Finished archive workflow/入口执行 lifecycle predicate，不得只挂在可直接调用的 helper 单测。
- [ ] 可核验 symbol：`MODIFIER_CONSUMER_MANIFEST`、`MODIFIER_EFFECT_COMPLETION_CANONICAL_FINDINGS`、`CanonicalLifecycleBaseline`、`CanonicalFindingId`、`ModifierContractSubject`、`ModifierMemberId`、`ModifierProducerBindingId`、`ModifierProducerSiteId`、`ModifierProducerCallerId`、`ModifierProductionEntryId`、`ModifierConsumerEdgeId`、`ModifierConsumerSiteId`、`ModifierSystemId`、`ModifierScheduleSiteId`、`DifferentialEvidence`、`DifferentialEvidenceReceipt`、`modifier_consumer_manifest_stays_current`、`modifier_producer_sites_stay_current`、`modifier_producer_reachability_matches_production_app`、`modifier_consumer_sites_stay_current`、`modifier_schedule_sites_match_production_app`、`modifier_consumer_graph_supports_shared_schedule`、`jinzhongdan_negative_binding_tracks_actual_and_expected_member`、`canonical_lifecycle_transitions_from_base_tree_are_pinned`、`canonical_stable_id_rekey_cannot_bypass_transition_gate`、`current_manifest_cannot_self_authorize_baseline`、`differential_evidence_is_current_and_subject_unique`、`r8_5_closed`、`r8_5_subject_set_is_exact`、`retiring_one_member_preserves_shared_schedule`、`retiring_one_binding_preserves_other_member_sites`、`consumer_without_registration_fails`、`producer_without_production_caller_fails`、`fabricated_per_member_registration_fails`、`mapped_pending_requires_closure_target`、`gameplay_closed_requires_scheduled_consumer`、`mapped_producer_retired_requires_subject_specific_unreachability_and_migration`、`existing_gameplay_requires_full_evidence`、`dormant_no_producer_requires_empty_reachability`、`unmapped_observation_baseline_is_immutable`、`ordinary_allowlist_update_cannot_admit_new_orphan`、`modifier_archive_rejects_plan_canonical_pending_closure`、`unmapped_observation_requires_successor_before_consumption`（名称可等义）。

## P1 — mapped gameplay closure

### P1-A — Alchemy + mapped Insight

- [ ] 冻结 `ContaminationBoost` 的 magnitude、duration、stack/refresh/expiry 与 ledger 接缝；§8.1 定义 magnitude 的单位、合法 finite 闭区间和负值/越界/NaN/±Infinity 策略，以及 duration 的 0/上下界/溢出策略。错误输入必须 reject 或 fail-closed，且 effect 状态与 qi ledger 均无部分副作用；`contamination_tick` 通过 canonical qi ledger 产生可观察差分。
- [ ] `contamination_boost_lifecycle_is_pinned`（名称可等义）以表驱动状态机覆盖 duration=0、合法最小/最大与等号、溢出、首次 apply、同源/异源重复 upsert、stack/cap、refresh 前后、expiry tick 前/等号/后、到期清理恰好一次及到期后回 neutral。每一步同时断言 effect 状态、`WorldQiAccount` 与 `QiTransfer`：非法输入或失败转换全零副作用，合法转换不得重复记账，到期后不得继续产生该 effect 的 ledger 差分。同源/异源只黑盒 pin §8.1 选定的既有 source identity 语义，不借此迁移 `source_pill` 或扩张 status-origin scope。
- [ ] JinZhongDan negative slot 改为语义明确的负面 regen effect；§8.1 冻结 `neg_scale` 的单位、合法 finite 闭区间、组合顺序及负值/越界/NaN/±Infinity 策略；测试覆盖下界/等号/上界、0/默认/max、非法值零副作用、到期回 neutral 与重复 upsert。
- [ ] r7 五字段与 r8 #5 的 13 个冻结字段按表中 gameplay domain 接入唯一 effective helper；P0 若证明某 writer production 不可达，也必须在本批删除/禁用该 writer并迁移状态，不得改标 `UnmappedObservation` 消项。
- [ ] §8.1 逐字段冻结单位、neutral、finite 区间、add/mul 顺序、累计上限、消费时点、持久化/水合/reset。表驱动 pin 覆盖合法下界/等号/上界、越界、NaN/±Infinity、组合顺序/cap、持久化往返、水合与 reset/到期回 neutral；r7 五字段与 r8 #5 十三字段各自绑定专属 `ModifierConsumerEdgeId` 与 differential test ID，`ModifierScheduleSiteId` 可由多个字段共享，但每个字段都须机械证明 member→consumer→system→真实 registration 的完整 production-reachable 链。逐字段以相同 gameplay 输入、仅该 modifier 在 neutral/非 neutral 间变化，断言目标 domain 可观察差分且非目标后果不变，测试表与 18 字段精确集合对账。qi gain/drain 继续断言 `WorldQiAccount` / `QiTransfer` 守恒。
- [ ] 可核验 symbol：`ContaminationBoost`、`contamination_tick`、`CombatPillKind::JinZhongDan`、`QiRegenSlowed`、`insight_qi_regen_multiplier`、`effective_breakthrough_bonus`、`effective_overload_threshold`、`effective_vortex_delta`、`effective_vortex_flow_speed`、`mapped_insight_modifier_contract_is_pinned`、`mapped_insight_modifier_changes_gameplay`（名称可等义）。

### P1-B — Iron Cocoon wound grade + effective flow

- [ ] `combat::resolve_attack_intents` 内建立唯一 typed `CanonicalWoundSink`（名称可等义）：该 sink 是本 pipeline 唯一允许最终构造/写入 `Wound` 与派生后果的位置；参与该 pipeline 的 damage producer 必须调用它。门禁 pin sink **恰好一个**、production 可达，并拒绝重复 sink 与 sink 外直接写入。
- [ ] sink 只处理**一个 `AttackIntent` event occurrence 对应的 primary wound**：`raw hit → armor → pure effective severity/grade → deterministic downgrade → health/bleeding/contamination/meridian/event consequences`。mapped 三个 wound modifier 只在这里消费；先完成 modifier 输入与目标状态的全部可失败校验，再以一个已计算的 effective result 驱动本范围后果，避免 grade/severity 与后果分叉。`EventReader<AttackIntent>::read_with_id()` 暴露的 world-local `EventId`（或等义的调用方注入 occurrence/roll input）只用于该事件实例的 deterministic fracture decision；同一事件不会在下一次 reader update 重放，两个 payload 相同但分别 send 的事件仍是两个合法 hit，禁止按 payload 去重。
- [ ] §8.1 只冻结本 sink 的 occurrence/roll 输入、effective-grade 纯函数、失败前置校验与本范围提交顺序；明确排除 durable hit identity、persistent settled-hit ledger、retention/cleanup、hydration/restart replay、cross-restart identity reuse、payload deduplication 与 resolver 全局 rollback。若未来需要这些能力，必须另立 combat-persistence/transaction canonical finding，不得借 r8 #2 偷渡。测试至少 pin：单 event 单次结算、同一 reader 后续 update 不重复结算、两个相同 payload 分别产生两个伤口、deterministic roll 对同一 occurrence 输入稳定、modifier 非法/目标缺失在 primary-wound mutation 前 fail-closed 且无本范围副作用。
- [ ] §8.1 分类型冻结合同：`bruise_threshold_multiplier` 与 `fracture_downgrade_chance` 定义单位、neutral、合法 finite 闭区间、非法值 fail-closed/reject、组合顺序/cap；`cut_pierce_downgrade` 与 `scar_forged_flow_bonus` 是 bool marker，只允许 `false/true`，并覆盖非法反序列化。四字段共同 pin 持久化/水合/reset/到期与 active→inactive 回 neutral；另单独冻结 ScarForged marker 生效时 effective-flow 数值倍率的依据、finite 区间与组合/cap。表驱动测试按实际类型覆盖浮点下界/等号/上界、越界、NaN/±Infinity，以及布尔两态和非法 wire 值。
- [ ] §8.1 的唯一 wound-grade 表对每个 wound kind/grade 的 finite threshold 使用相邻可表示浮点值 `next_down(threshold)`、`threshold`、`next_up(threshold)`（或等价 bit-level 生成）锁定等号归属，禁止使用未定义的 `threshold±ε`。测试先断言阈值 finite、相邻值严格有序且未跨越相邻 grade 阈值；若端点不存在某一侧相邻值，§8.1 必须明确该端点策略。三点均断言最终 grade 与 health、bleeding、contamination、meridian、event 的完整派生后果。
- [ ] `effective_meridian_sum_rate` 只在 `scar_forged_flow_bonus` active 时对 `ActiveScarCircuits` 涉及的去重经脉应用 §8.1 决定的倍率；共享经脉只加成一次，不持久改 `Meridian.flow_rate`。
- [ ] 本批不声称迁移全仓所有历史 wound/health writer；未经过 `resolve_attack_intents` 的旁路属于附录观察，须独立 canonical finding 才能扩 scope。
- [ ] 可核验 symbol：`CanonicalWoundSink`、`canonical_wound_sink_is_unique`、`effective_wound_grade`、`wound_grade_thresholds_are_pinned`、`cocoon_fracture_roll`、`effective_meridian_sum_rate`、`iron_cocoon_downgrade_changes_full_wound_consequences`、`event_occurrence_is_settled_once_without_payload_dedup`、`scar_forged_bonus_only_applies_to_active_circuits`（名称可等义）。

### P1-C — `jump_height_multiplier` authority

- [ ] §8.1 先选择唯一互斥路线，并冻结字段表示 apex-height multiplier 还是 initial-velocity multiplier、合法 finite 区间与 apex 容差。MC 1.20.1 离散重力、阻力、碰撞和 tick 更新顺序必须锚定确切上游版本/映射/类/方法，不得凭经验自定；client/server 共享公式或同一组 golden tick/velocity/apex vectors。非法值统一 fail-closed 到 1.0 或拒绝。
- [ ] P1-C 的路线选择必须由 §8.1 Decision checkpoint 在进入 P1-C 前明确记录，并由人工/受保护主线 gate 批准；该决定是本 plan 的唯一 architecture authority。若选择 **server-authoritative**，P1-C 只交付 server movement/velocity authority、production schedule、非法纵向速度拒绝与真实 client/bot apex e2e，所有 client ACK/proto/store/反向生产链条均为 N/A，P0 不得以它们阻塞，且不得遗留无 runtime reader 的 client schema/store。若选择 **client-hook + server-validation**，P1-C 才必须交付下述 `NeutralAppliedAck`/`JumpAuthorityTupleReport` 与 client→server 真实 transport 反向链；这些 ACK seam 是 P1-C 条件交付，不属于 P0。
- [ ] **client-hook + server-validation 路线（仅在 §8.1 选择该路线时生效）**：从 `server/src/network/derived_attrs_emit.rs` 的 `DerivedAttrs` query、payload 写入与 production send schedule 开始，贯通 `DerivedAttrsSyncV1`、proto/generated、`DerivedAttrsHandler`/version-aware jump store、非 mixin helper、jump hook 与 disconnect reset；同时必须新增并实际加载 server movement validation consumer（例如 `server/src/movement/jump_validation.rs` 的 `validate_player_jump_motion`，名称可等义），在每个受影响 movement tick 读取 server-side authoritative `(session, revision, effective_tick, multiplier)` 与当前位置/速度/`OnGround` 状态，按 §8.1 锚定 MC 1.20.1 离散物理 envelope/tolerance，对超出 envelope、旧状态或缺失状态执行 reject/correct/fail-closed，并以 observable server outcome 而非只看 payload/store 证明生效。消息采用 session generation + 单调 revision/effective tick 的权威全量状态；同一 session 激活→停用/到期时 server 必须预发布显式 `jump_height_multiplier=1.0` 的更高 revision。client 反向生产链必须明确分离：`DerivedAttrsHandler` 仅在原子安装完整 authority tuple 后生成精确 `NeutralAppliedAck(session, revision, effective_tick)`；周期性 `JumpAuthorityTupleReport` 只报告当前观测 tuple，不是 receipt，不能单独驱动 server authority。ACK 必须经命名的 `ClientRequestProtocol` encoder、`ClientRequestSender` 的 `bong:client_request` send site、Rust `ClientRequestV1` schema/decoder、`CustomPayloadEvent` dispatch、typed ACK/report resource/event，再由排在 `validate_player_jump_motion` 之前的真实注册 server consumer 消费；删除任一 seam 必须命中 anti-orphan guard 自身稳定 diagnostic，wire e2e 必须经真实 ClientPlayNetworking→Valence transport→serde→handler→scheduled consumer，禁止 direct-call handler/state machine 代替。§8.1 只能选择以下可收敛交接之一：**ACK-staged 路线**中 `effective_tick` 仅作 server-labelled not-before/attribution，不是 client 本地 tick或双方同步时钟；server 发送显式 `NeutralPrepare(session, revision, effective_tick)` 并在收到精确 ACK 前继续按旧 active authority 验证、允许已 neutral 的 client 低轨迹并重传 prepare；client 安装 tuple 后回 ACK，但保持 jump fence；server 仅在真实 handler 消费到该 ACK 后，于明确的下一 `commit_server_tick` 原子提交 neutral，并发送可重传 `NeutralRelease(commit_server_tick)`，client 直到收到同一 release 才解除 fence。**禁止**声称丢包后双方在同一实际 tick commit。ACK 缺失、错 tuple、旧 revision/session、重复/乱序、超时或断线均不得使 server 单方面切 neutral：server 保持 active/reconciliation、拒绝新 jump、沿旧 envelope 校验/纠正并最终 fail-closed。未来 effective tick 只能标记不早于何时，不得单独驱动未确认 authority transition；不得在丢包时出现 server neutral 且 client 仍获准 active jump。字段缺失、旧 revision 或旧 session 不得清除/复活状态。本子合同仅在 §8.1 选择 client 路线时生效：§8.1 还必须冻结 session/revision/effective-tick/phase 的 wire 类型、位宽和比较算法，定义 server/client 进程重启后的 session 唯一性来源，并禁止 revision 静默回绕。revision 耗尽时只能在上述显式 neutral 交接下切换到不可复用的新 session，或停止发送并 fail-closed；ACK 仅在 `(session generation, revision, effective tick)` 全部精确匹配时有效，report 永远不得替代 ACK；跨 session 的旧 active、neutral、ACK 或 report 均不得改变当前状态。**future-effective-tick 仅作为历史兼容术语，不得再作为独立 authority 路线。**
- [ ] **client-hook + server-validation 的 TypeBox schema authority（仅在 §8.1 选择该路线时生效）**：`DerivedAttrsSyncV1` 必须以 `agent/packages/schema/src/server-data.ts` 为唯一 TypeBox source of truth，并在实际 server-data wrapper/registry 中注册；同一变更链必须更新 `agent/packages/schema/src/schema-registry.ts`，提交由 `agent/packages/schema/src/generate.ts` 派生的 `agent/packages/schema/generated/server-data-derived-attrs-sync-v1.json` 等生成 JSON Schema，提交 `agent/packages/schema/samples/` 下命名的 valid/neutral/reset/invalid shared samples，并运行 `npm run generate:check` 后用 `cd agent && npm run build -w @bong/schema` 重建 `dist/`。P1-C 必须逐项对拍 `TypeBox source → generated JSON Schema → shared samples → rebuilt @bong/schema export → server/src/schema/combat_hud.rs:205-220 → proto/bong/envelope.proto:1706-1718 与 generated proto → DerivedAttrsHandler/store/hook` 的字段、类型、位宽、unknown-field 规则；samples 必须同时通过 TypeBox 与 Rust serde round-trip，缺失/陈旧/漂移的 source、generated、sample、dist 或 Rust mirror 必须在 runtime chain 验收前由 guard 失败。可核验 symbol 至少包括 `derived_attrs_sync_typebox_contract_is_authoritative`、`derived_attrs_sync_generated_artifacts_are_fresh`、`derived_attrs_sync_samples_match_rust_wire`、`derived_attrs_sync_dist_export_is_current`（名称可等义）。若选择 server-authoritative 路线，该 client schema/store/ACK 链明确为 N/A，且不得留下无 runtime reader 的 client artifact。
- [ ] 两路线共同测试 multiplier 1.0/中间/上限与非法输入，并以真实 velocity/apex 而不是 payload/store 值验收；共同覆盖 active→neutral、到期/reset、断线/实体重建/重登、水合后旧状态不复活，以及转换边界前后起跳。server 路线另证明 production schedule 每次读取当前权威倍率且下一次起跳恢复 1.0；client 路线另覆盖旧 active 与 neutral 乱序、重复 neutral、旧 session neutral、neutral 延迟跨 tick、丢失/重传、ACK 迟到/丢失、`NeutralAppliedAck` 与 `JumpAuthorityTupleReport` 语义混用、反向 production wiring 任一 seam 缺失、handler 前 report、`effective_tick` 到点但 prepare/ACK 全丢失、`NeutralReconciliation` 禁止新 jump/旧 active envelope 校验、明确 `commit_server_tick` 后的 release/fence 解除、超时纠正/断开、effective tick 前后起跳交错、revision max-1/max/耗尽与回绕尝试、server/client 分别重启、session 标识冲突、旧 active/neutral 跨 session 到达及旧 ACK/report 抵达新 session。不得测试或断言“丢包后双方同一实际 tick neutral commit”；只能断言 server 在消费精确 ACK 后于明确 `commit_server_tick` 提交，并且 client 在收到对应 release 前保持 fence。选择 server-authoritative 路线时，client wire 测试明确记为 N/A，且不得遗留 proto/store 字段。
- [ ] 可核验 symbol：`sanitized_jump_height_multiplier`、`effective_jump_velocity`、`jump_physics_golden_vectors_match_mc_1_20_1`、`guangbo_jump_height_changes_observed_apex`；仅选择 client-hook + server-validation 路线时，另含 `DerivedAttrsSyncV1`、`DerivedAttrsStore`、`jump_modifier_resets_on_disconnect`、`jump_modifier_neutral_revision_clears_same_session`、`neutral_applied_ack_is_distinct_from_tuple_report`、`client_request_ack_reverse_wiring_is_production_reachable`（名称可等义）。

## §8 开放问题（P0 前须追加 §8.1 决议）

1. `ContaminationBoost` 的 magnitude 单位/合法 finite 闭区间/非法值策略、duration 的 0/上下界/溢出策略、stack/refresh/expiry 与 ledger 接缝是什么？
2. JinZhongDan 的负面 kind、基础强度，以及 `neg_scale` 的单位、合法 finite 闭区间、非法值策略与组合公式是什么？
3. r7 五字段与 r8 #5 十三字段逐项的单位、neutral、finite 区间、组合/累计、消费时点、持久化/水合/reset 是什么？
4. canonical wound grade 的阈值/等号归属是什么；两个浮点 wound modifier 的单位、neutral、合法 finite 闭区间、非法值策略、组合/cap 与生命周期是什么；`cut_pierce_downgrade` bool marker 的两态、非法反序列化与生命周期是什么；一个 `AttackIntent` event occurrence 的 occurrence/roll input、deterministic fracture decision、effective-grade 纯函数与本范围失败前置校验/提交顺序如何定义？明确不在本 finding 冻结 durable hit identity、persistent settled-hit ledger、retention/cleanup、hydration/restart replay、payload dedup 或 resolver 全局 rollback；这些若需要必须另立 combat-persistence/transaction finding。
5. `scar_forged_flow_bonus` bool marker 的两态、非法反序列化与生命周期是什么；marker 生效时 effective-flow 倍率的 canonical 数值依据、合法 finite 闭区间、非法值策略、适用经脉、组合/cap 与持久化/水合/reset/到期语义是什么？当前代码注释中的 +5% 不能在无决议时自动视为正典。
6. jump 选择哪条 authority 路线；字段表示 apex 还是 velocity、合法范围、权威 MC 1.20.1 源码/映射锚点、离散公式/容差是什么？若选择 client 路线，还须冻结 session/revision/effective-tick 的 wire 类型与位宽、比较算法、revision 耗尽且禁止回绕的策略、server/client 重启后的 session 唯一性、explicit-neutral 的 ACK-staged prepare/ACK/commit_server_tick/release、重传/超时合同，以及 ACK 对 `(session, revision, effective tick)` 的精确匹配规则；future-effective-tick 仅作历史兼容术语，不得成为独立 authority 路线。
7. P0 的机械权威源采用 Rust AST、唯一声明宏还是同等方案；如何从 production 代码与真实 scheduler registration 导出全部 producer branch、member→consumer edge、consumer system 与 system→schedule edge，允许共享 registration 却禁止伪造逐字段 registration，并把 pending 检查接入实际归档命令/CI 入口？

> 七项全部以当前 `file:line + plan 章节` 双锚点追加到 §8.1 后，才能进入 P0。

## 未映射域观察（非本 plan 交付物）

以下是调研中发现的风险线索，**不是 P0/P1 mandatory deliverable，不阻塞本 plan 归档，也不得由本 plan 实施 agent 顺手迁移**。任何一项进入实现前，必须先建立独立 canonical finding、明确 owner、successor plan 与验收测试。

| 未映射域 | 当前证据 | triage owner / 消费前置 |
|---|---|---|
| Alchemy 动态 side-effect tags | `AlchemyBuff(String)` 隐藏 recipe 动态 tag；当前资产约 35 tag / 80 config site | Alchemy owner；先立独立 config-effect finding/successor |
| perception keys | `UnlockPerception.kind` / `UnlockedPerceptions` 使用自由字符串，producer/reader key 不配对 | Cultivation/Insight owner；先立 perception registry finding/successor |
| Insight trigger/fired keys 与 no-op variants | `trigger_id`、`fired_triggers` 及部分 apply arm 是字符串/no-op | Cultivation/Insight owner；按 trigger lifecycle 与 no-op effect 分别立 finding/successor |
| status origin | `ActiveStatusEffect.source_pill: Option<String>` 参与 stack/expiry/cleanup | Combat Status + Alchemy owner；先立 typed origin finding/successor |
| generic-talent discriminator | stat/op/group/color stringly config、unknown→Lung、转换 `.ok()` 静默过滤 | Cultivation/Insight owner；先立 config validation finding/successor |
| repo-wide wound writer migration | projectile/AoE/collision 与 healing/revival/init 等 writer 尚未统一分类 | Combat/Wound owner；先立 repo-wide wound pipeline finding/successor |

P0 只允许把这些六类 domain summary 标为“未映射域观察”并验证“未被本 plan 消费”；它们不是可放行的 wildcard。机械 manifest 还必须单独登记 `InsightModifiers.composure_recover_mul` 这一 exact-HEAD `UnmappedObservation` stable ID；不得以 inventory 名义把六类 summary 或该精确观察重新变成本 plan 交付物。

## §10 单 PR / P0-P1 实施边界

本 skeleton 必须服从现有 `/consume-plan <plan-name>` consumer：骨架 promotion 为 active plan 后，由一次普通消费在**同一 worktree、同一 implementation branch、同一个最终 PR**内按 P0 → P1 推进。consumer 只解析阶段总览中的 `P0`、`P1` 行；`Decision`、`P1-A`、`P1-B`、`P1-C`、`Closure` 是该单 PR 内的有序文内 checkpoint，不是独立 PR、分支或 merge boundary。该约束不授权修改 `skills/consume-plan/SKILL.md`、command loader/registry、workflow 状态存储或其他 plan，也不允许把附录观察混入实现。

本 plan 的机器可读阶段合同固定为：

```json
{"protocol":"consume-plan-single-pr-v1","phases":["P0","P1"],"checkpoints":["Decision","P1-A","P1-B","P1-C","Closure"]}
```

该合同与现有阶段解析器一致；不得再声明 consumer 不支持的多 PR stage、阶段分支或阶段 merge。

1. **Decision checkpoint**：promotion 后在 active plan exact HEAD 上完成 §8.1 七项双锚点决议与 §10.1 authority readiness checklist；缺任一决议、baseline 或 trusted authority 时停止，不进入 P0。该 checkpoint 不创建 PR，也不声称 P0 authority 已由当前 PR 自举。
2. **P0 phase**：在同一 implementation branch 只实现 inventory、producer-site、`MappedPendingClosure` manifest 与共享 gate；不得预填不存在的 consumer，不得迁移附录域。P0 完成后才允许在同一 branch 进入 P1-A checkpoint。
3. **P1-A checkpoint**：只闭合 r6、r7 与 r8 #5 mapped Alchemy/Insight finding；同一最终 PR 原子交付 production wiring/schedule/differential + pin tests，并转换本批 manifest 状态。
4. **P1-B checkpoint**：只闭合 r8 #2/#3；同一最终 PR 完成唯一 `CanonicalWoundSink`、effective flow、production tests 与本批状态转换，不迁移 repo-wide writer。
5. **P1-C checkpoint**：只闭合 r8 #4；按 §8.1 选定的一条 jump 路线完整交付并转换状态，另一条不得留下死 schema/store。client-hook 路线还必须满足 P1-C 的 TypeBox/generated/sample/dist/Rust/proto 对拍合同。
6. **Closure checkpoint**：仍在同一最终 PR 内，从该 PR 的 immutable base tree authority 派生精确 11 个 finding 及各自完整 expected subject set，机械拒绝任一 child 残留 `MappedPendingClosure`、缺失/重复/额外/改绑或手写 parent Closed；每个 Closed subject 还必须从 protected trusted run 取得同时绑定该 PR `base_sha`/`head_sha`、subject/edge/domain、target/selector 与成功结果的唯一 `DifferentialEvidenceReceipt`；r8 #5 另验证 13 Member + 13 ProducerBinding（26 subjects）的 `r8_5_closed(base,current)` predicate。随后在同一 PR 中填写 `## Finish Evidence` 并按现有归档入口迁入 `docs/finished_plans/`；Closure 不得首次新增 production wiring。

该单 PR 必须在 P0/P1 checkpoint 间保持顺序：fresh `origin/main` 复验 → Decision/authority readiness → P0 → P1-A → P1-B → P1-C → Closure → `git diff --check` + `python3 scripts/plans_progress.py --check` → 紧邻 `git fetch origin && git merge origin/main` → 若 HEAD 变化则对新 SHA 重跑 exact-HEAD validator 与受影响 gate → push → local/remote/PR HEAD SHA 对拍 → 当前 central-review gate 与 CodeRabbit 收敛。任何 blocker/major、测试失败、SHA 漂移或 merge conflict 均停在当前 checkpoint；修复后仍在同一 branch 对账并从该 checkpoint 续跑，不跳 checkpoint、不重复创建 PR、不并行消费附录域。本地严禁运行 `scripts/test-tmux-shutdown-order.sh` 或任何调用它的 suite；该覆盖只留给 GitHub e2e。

- [ ] `modifier_archive_rejects_plan_canonical_pending_closure` 必须升级为真实 PR archive lifecycle gate：它必须在该**单一实现 PR**的 active→finished rename 上运行，取得该 PR 的 base SHA 与 HEAD，读取 base tree authority 并验证 derived `all_11_closed` / `r8_5_closed(base,current)`，以及每个 Closed subject 的 `DifferentialEvidenceReceipt` 在当前 PR base/HEAD、protected verifier/workflow、target/selector、subject/edge/domain 和成功结果上精确对拍。不得仅挂在可绕过的 helper、`plan-finish.sh`、`plans_progress.py --check` 或 source-path e2e 中。工作流必须监听 active/finished plan rename、guard 实现及 workflow 本身，且 checkout/取 object 的方式必须可读取 base SHA 与对应 blob。集成测试必须证明直接 `git mv`、/consume-plan 内联 `git mv`、helper 外部调用和 docs-only closure patch 都不能绕过 lifecycle gate。归档检查只量化 base authority 冻结的精确 11 个 finding ID；每个 finding 的完整 expected subject set 必须逐项闭合，r8 #5 额外必须满足 13 Member + 13 ProducerBinding 的 26-subject predicate；集合外 pending 不计入也不能抵消/掩盖 mandatory subject。

### §10.1 Promotion 后的 authority readiness checklist（非 finding、非实施阶段）

本 gameplay plan 不授权修改 `skills/consume-plan/SKILL.md`、command loader/registry、workflow 状态存储或其测试；这些工作既不是第 12 条 canonical finding，也不是共享 P0 gate。现有入口只支持 active plan 的普通 `/consume-plan <plan-name>`，因此本骨架不得虚构未注册参数或 skeleton 路径调用。

骨架 → active promotion 后，runner 必须在该 active plan 的 exact HEAD 上完成并记录以下**人工只读 checklist**；它不创建 worktree、branch、commit 或 PR，不是新的 protocol stage，也不调用 `/consume-plan`：

1. 核对机器阶段合同仍精确为 `P0 → P1`，并记录 active plan HEAD 与当前 workflow HEAD；同时核对 `Decision` checkpoint 的七项 §8.1 决议已具备双锚点，但不得声称 P0 authority 已就绪。
2. 在同一 implementation branch 进入 P0 前、且不改变该 `decision_plan_blob_sha` 的前提下，读取并记录 `protected_ref`、`protected_base_sha`、`decision_plan_blob_sha`、baseline bootstrap merge SHA 与 baseline blob/object SHA、archived canonical source tuple、derivation version、tuple digest、trusted verifier SHA、trusted workflow SHA、baseline schema version，以及 verifier/schema/derivation compatibility verdict。
3. baseline/blob/tuple 缺失或不匹配、决议 plan blob 漂移、trusted verifier/workflow 不在 protected authority、verifier 旧于 baseline schema/derivation version、无法读取 protected base object，或 checklist evidence 不完整时，runner 必须停止，不得进入 P0/P1，也不得把 workflow 缺口计入 modifier manifest。bootstrap/trusted-gate 修复必须由独立 workflow blocker/PR 完成；其 merge 后在保留该 `decision_plan_blob_sha` 的 current protected base 重新执行本 checklist。

`consume-plan-single-pr-v1` 的 checkpoint gate 必须机械证明：

1. promotion 后只允许一份 active plan、一个 implementation branch、一个最终 PR；普通 `/consume-plan <plan-name>` 只在 `docs/plan-<name>.md` 存在后启动，并由现有 consumer 在同一 worktree 顺序执行 P0 → P1。
2. Decision checkpoint 只能完成 §10.1 checklist，不得创建或声称已验证 P0 authority；缺失 `CanonicalLifecycleBaseline` authority tuple/digest、bootstrap merge/blob SHA、trusted verifier/workflow SHA 或 baseline schema/derivation compatibility 时，P0/P1 均 fail-closed。Decision 不创建/替代 bootstrap 或 trusted-gate PR，只作本 plan §8.1 决议。
3. 持久 execution ID、单 owner lease/CAS、checkpoint 幂等 operation key 与明确 linearization point。
4. P0 完成后才可进入 P1-A → P1-B → P1-C → Closure checkpoint；各 checkpoint 不产生独立 branch/PR/merge boundary。P1-C client-hook 路线另须满足 TypeBox source、generated JSON Schema、shared samples、rebuilt `@bong/schema` dist、Rust mirror 与 proto/generated 的 exact contract 对拍；server-authoritative 路线将该 client chain 记为 N/A。
5. 确定性故障注入覆盖重复命令、双 runner、派发前后崩溃、PR 创建后未登记、证据写入失败、陈旧开放 PR、central-review/e2e 失败、SHA 漂移与 merge conflict，以及 baseline 缺失、authority tuple/blob 不匹配、trusted verifier/workflow 非 protected authority、verifier/schema/derivation 不兼容；断言不越级、不产生第二个有效 PR、已完成 checkpoint 不重复执行且 Closure 只归档一次。
6. Closure 只能在同一最终 PR 的前序 P0/P1 checkpoint 均完成、`MODIFIER_EFFECT_COMPLETION_CANONICAL_FINDINGS` 的精确 11 个 ID 内，每个 finding 的完整 base-derived expected subject set 均逐项闭合，且 `MappedPendingClosure=0`、Retired 证据有效、每个 Closed subject 的 `DifferentialEvidenceReceipt` 对应当前 PR base/HEAD 与 trusted workflow 成功结果后执行；r8 #5 还必须满足 `r8_5_closed(base,current)` 的 13 Member + 13 ProducerBinding（26 subjects）精确集合 predicate。集合外 pending 不计入也不得掩盖本集合状态。

正常的真实 `/consume-plan <plan-name>` run 只在骨架 promotion 为 `docs/plan-<name>.md` 后作为 single-PR happy-path smoke evidence；暂停/恢复与并发安全由上述 workflow-owned deterministic fixture 证明，不要求本 gameplay plan 制造故障。

### §10.2 checkpoint 完成后的单次消费

仅在 promotion 后完成 §10.1 checklist，用户的一次普通 `/consume-plan <plan-name>` 才可从 P0 起按 P0 → P1 顺序运行。该次消费在一个 worktree、一个 implementation branch、一个最终 PR 内依次通过 Decision、P0、P1-A、P1-B、P1-C、Closure checkpoint；任一 blocker/major、测试失败、SHA 漂移或 merge conflict 均暂停在当前 checkpoint，修复后以同一 operation key 对账远端状态并从该 checkpoint exact HEAD 续跑，不跳 checkpoint、不重复派发、不并行消费附录域。Finish Evidence 记录 checklist 的 protected authority tuple、bootstrap/trusted verifier/workflow SHA、compatibility verdict、run ID、唯一 PR 的 base/head SHA，以及真实运行中的暂停/恢复事件；不得用一次无故障 run 冒充 workflow 故障分支证明。Closure 完成后，真实归档入口从该 PR 的 base tree authority 派生精确 11-ID 与每 finding 完整 expected subject set，确认全部 subject 已有效 Closed/Retired、集合内 `MappedPendingClosure=0`、Retired 证据有效、每个 Closed subject 的 current-HEAD trusted `DifferentialEvidenceReceipt` 可解析且唯一归属，并且 r8 #5 满足 26-subject predicate，才可迁入 `docs/finished_plans/`。

## 归档门

- 11 条 mapped finding 的父级状态（含 r8 #5 聚合 finding）不得手写，必须由各自 base-derived 完整 expected subject set 逐项派生：每个 Member subject 与 ProducerBinding subject 均转为有效 `MappedGameplayClosed`，或按 §8.1 删除/禁用精确 producer、迁移状态并以机械证据转为 `MappedProducerRetired`。仅对 `MODIFIER_EFFECT_COMPLETION_CANONICAL_FINDINGS` 冻结的精确 11 个 stable ID 要求 `MappedPendingClosure=0`，集合外条目不计入也不能掩盖。`MappedGameplayClosed` 遵循 §P0 的 production-reachable writer/binding、consumer edge、system、真实 registration 与 differential closure；只有选择 `MappedProducerRetired` 的 Member subject 才要求其全部 production-reachable writer/binding 与 consumer edge 均不可达，只有选择 `MappedProducerRetired` 的 ProducerBinding subject 才要求其精确 producer site 不可达。共享 scheduler 若仍服务其他 member/site 可以保留，迁移证据必须完整。r8 #5 还必须满足 13 Member + 13 ProducerBinding 的 26-subject predicate。不得以 rekey/delete+add、`UnmappedObservation`、HUD/storage read 或 planned 状态结案。
- P0 完整库存精确对账，exact-HEAD 冻结的附录 observation allowlist 未被普通变更扩大；基线内观察仍保持“未被本 plan 消费”且不阻塞本 plan，基线后新增 orphan 则 fail-closed。
- Alchemy/Insight、Iron Cocoon wound/effective flow、jump 三条 mapped production chain 均有 observable differential/e2e；数值、生命周期、wound 阈值与 jump neutral/物理 vectors 均有 pin，qi 路径有 ledger 守恒证据。
- Finish Evidence 只在该单一实现 PR 的 validator、按栈 gate、`git diff --check`、plan 进度检查、local/remote/PR SHA 对拍、更新后 exact-HEAD central-review gate、CodeRabbit 与 GitHub e2e 全部收敛后填写；记录 commit SHA、validator verdict、命令结果、run URL/ID 与遗留 successor。
