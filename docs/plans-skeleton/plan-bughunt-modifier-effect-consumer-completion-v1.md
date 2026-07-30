# plan-bughunt-modifier-effect-consumer-completion-v1（骨架）

> **骨架（草案）**。一句话主题：只收束 r6/r7/r8 已映射的 11 条 canonical modifier/effect finding，并建立一套共享 anti-orphan gate；未进入 Canonical Finding Mapping 的观察不属于本 plan 交付物。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 完整类型库存、mapped producer manifest 与可独立合并的 pending-closure 门禁 | ⬜ |
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
- **跨仓库契约**：P0 与 P1-A/P1-B 以 server 为主；P1-C 按 §8.1 选择 server-authoritative 或 client-hook 路线。client 路线必须包含 server emitter → proto/generated → handler/store → 非 mixin helper → jump hook 的完整单向数据链。
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

- [ ] 建立 greppable `ModifierConsumerContract` / `MODIFIER_CONSUMER_MANIFEST`（名称可等义）。以 §8.1 选定的唯一声明宏、Rust AST 或同等机械来源，穷举 `DerivedAttrs` 字段、`InsightModifiers` 字段与 `StatusEffectKind` variant 全集及每个 production producer branch；另从 production 代码独立导出 typed consumer edge、consumer system 与 app 中实际 scheduler registration（排除 `cfg(test)`、测试调用和孤立 helper）。新增/删除/改名/新增 branch 或 edge 未登记即失败。
- [ ] 调度证据使用分层 typed graph：`ModifierMemberId → ModifierConsumerEdgeId(ModifierConsumerSiteId) → ModifierSystemId → ModifierScheduleSiteId`。member→consumer 表示某字段/variant 的真实 production read edge，system→schedule 表示 app 中真实加载的 production registration；一个 member 可有多个 consumer edge，多个 member/system 可共享同一真实 registration，禁止伪造逐字段 registration。Closed/Existing 必须至少存在一条从该 member 到真实 registration 的 production-reachable 完整路径；consumer 无 registration、registration 未实际加载或只在测试加载均失败。
- [ ] 使用互斥生命周期分类：`MappedPendingClosure`、`MappedGameplayClosed`、`MappedProducerRetired`、`ExistingGameplay`、`DormantNoProducer`、`UnmappedObservation`。P0 时本 plan 的未闭环成员合法登记为 `MappedPendingClosure`，只要求 canonical finding ID、稳定 producer-site/reachability 证据、owner、目标 P1 批次、预期 consumer domain 与专属测试 ID；**不要求尚不存在的 production caller、schedule 或差分测试先通过**。Mapped canonical 成员只能在 Pending、Closed、Retired 三类间转换，不得改标其他分类消项；`MappedProducerRetired` 必须由同一机械库存证明 mapped writer 与该 member 的 typed consumer edge均为空、因而该 member 不再存在到任何 scheduler registration 的可达路径；仍服务其他 production consumer 的共享 registration 不要求删除。Retired 还须绑定 producer 删除/禁用证据、迁移版本及持久状态迁移/水合测试 ID（无持久状态时绑定机械证明）。保留任一 writer/consumer edge 或缺迁移证据即失败。
- [ ] 各 P1 PR 必须在同一变更中接通 production consumer + schedule + observable differential test，并把本批项目原子转为 `MappedGameplayClosed`；若决议删除语义，则删除/禁用 producer、迁移持久状态后转为 `MappedProducerRetired`。`MappedGameplayClosed` 与 `ExistingGameplay` 都必须引用权威 producer、member→consumer edge、consumer system、真实 schedule site 与逐项 differential test ID；`DormantNoProducer` 必须由同一全量机械库存证明无 production-reachable writer，不能靠 manifest 自报。分类与 typed graph 必须和实际 scheduler 加载集合精确对账。
- [ ] P0 在 exact HEAD 冻结既有 `UnmappedObservation` stable-ID/producer-site allowlist；只有该基线内技术债可继续非阻塞。基线后新增字段、variant 或 production writer branch 默认 CI 失败，必须先由独立 PR 建立 canonical finding、owner 与 successor，再进入受控 pending/closed/retired；普通 manifest 更新不得扩大 allowlist。未映射观察不得生成 typed migration、gameplay consumer 或本 plan 归档前置，也不会自动扩张 P1。
- [ ] 每个 production writer 使用稳定 typed producer-site ID，同函数不同 branch 不得折叠；mapped 11 条 finding 的 producer 子集必须精确对账，r8 #5 必须与上表 13 项相等。P0 不要求把未映射动态字符串域做全仓 typed migration。
- [ ] P0 门禁测试放入 `server/src/test_coverage_guards.rs`（由 `server/src/lib.rs` 的 `#[cfg(test)] mod test_coverage_guards` 加载），并由 `.github/workflows/e2e.yml` 的 `Server stage (cargo test)` 实际执行；negative fixture/mutation-style 集成验收分别移除或错配一个 producer branch、consumer edge、production caller、scheduler registration、Retired 迁移证据及 observation allowlist，证明 `cargo test` 与 e2e job 必定失败。另 pin：多个字段共享一个真实 registration 必须通过；退役其中一个字段且共享 registration 仍服务其他 consumer 时必须通过；consumer edge 存在但 registration 缺失、registration 残留但 member 已无 consumer edge、以及虚构逐字段 registration 必须分别得到预期分类或失败。后续 P1 与归档均以该 job 为必过项。
- [ ] 可核验 symbol：`MODIFIER_CONSUMER_MANIFEST`、`ModifierMemberId`、`ModifierProducerSiteId`、`ModifierConsumerEdgeId`、`ModifierConsumerSiteId`、`ModifierSystemId`、`ModifierScheduleSiteId`、`modifier_consumer_manifest_stays_current`、`modifier_producer_sites_stay_current`、`modifier_consumer_sites_stay_current`、`modifier_schedule_sites_match_production_app`、`modifier_consumer_graph_supports_shared_schedule`、`retiring_one_member_preserves_shared_schedule`、`consumer_without_registration_fails`、`fabricated_per_member_registration_fails`、`mapped_pending_requires_closure_target`、`gameplay_closed_requires_scheduled_consumer`、`mapped_producer_retired_requires_no_reachability_and_migration`、`existing_gameplay_requires_full_evidence`、`dormant_no_producer_requires_empty_reachability`、`unmapped_observation_baseline_is_immutable`、`modifier_archive_rejects_pending_closure`、`unmapped_observation_requires_successor_before_consumption`（名称可等义）。

## P1 — mapped gameplay closure

### P1-A — Alchemy + mapped Insight

- [ ] 冻结 `ContaminationBoost` 的 magnitude、duration、stack/refresh/expiry 与 ledger 接缝；§8.1 定义 magnitude 的单位、合法 finite 闭区间和负值/越界/NaN/±Infinity 策略，以及 duration 的 0/上下界/溢出策略。错误输入必须 reject 或 fail-closed，且 effect 状态与 qi ledger 均无部分副作用；`contamination_tick` 通过 canonical qi ledger 产生可观察差分。
- [ ] `contamination_boost_lifecycle_is_pinned`（名称可等义）以表驱动状态机覆盖 duration=0、合法最小/最大与等号、溢出、首次 apply、同源/异源重复 upsert、stack/cap、refresh 前后、expiry tick 前/等号/后、到期清理恰好一次及到期后回 neutral。每一步同时断言 effect 状态、`WorldQiAccount` 与 `QiTransfer`：非法输入或失败转换全零副作用，合法转换不得重复记账，到期后不得继续产生该 effect 的 ledger 差分。同源/异源只黑盒 pin §8.1 选定的既有 source identity 语义，不借此迁移 `source_pill` 或扩张 status-origin scope。
- [ ] JinZhongDan negative slot 改为语义明确的负面 regen effect；§8.1 冻结 `neg_scale` 的单位、合法 finite 闭区间、组合顺序及负值/越界/NaN/±Infinity 策略；测试覆盖下界/等号/上界、0/默认/max、非法值零副作用、到期回 neutral 与重复 upsert。
- [ ] r7 五字段与 r8 #5 的 13 个冻结字段按表中 gameplay domain 接入唯一 effective helper；P0 若证明某 writer production 不可达，也必须在本批删除/禁用该 writer并迁移状态，不得改标 `UnmappedObservation` 消项。
- [ ] §8.1 逐字段冻结单位、neutral、finite 区间、add/mul 顺序、累计上限、消费时点、持久化/水合/reset。表驱动 pin 覆盖合法下界/等号/上界、越界、NaN/±Infinity、组合顺序/cap、持久化往返、水合与 reset/到期回 neutral；r7 五字段与 r8 #5 十三字段各自绑定专属 `ModifierConsumerEdgeId` 与 differential test ID，`ModifierScheduleSiteId` 可由多个字段共享，但每个字段都须机械证明 member→consumer→system→真实 registration 的完整 production-reachable 链。逐字段以相同 gameplay 输入、仅该 modifier 在 neutral/非 neutral 间变化，断言目标 domain 可观察差分且非目标后果不变，测试表与 18 字段精确集合对账。qi gain/drain 继续断言 `WorldQiAccount` / `QiTransfer` 守恒。
- [ ] 可核验 symbol：`ContaminationBoost`、`contamination_tick`、`CombatPillKind::JinZhongDan`、`QiRegenSlowed`、`insight_qi_regen_multiplier`、`effective_breakthrough_bonus`、`effective_overload_threshold`、`effective_vortex_delta`、`effective_vortex_flow_speed`、`mapped_insight_modifier_contract_is_pinned`、`mapped_insight_modifier_changes_gameplay`（名称可等义）。

### P1-B — Iron Cocoon wound grade + effective flow

- [ ] `combat::resolve_attack_intents` 内建立唯一 typed `CanonicalWoundSink`（名称可等义）：该 sink 是本 pipeline 唯一允许最终构造/写入 `Wound` 与派生后果的位置；参与该 pipeline 的 damage producer必须调用它。门禁 pin sink **恰好一个**、production 可达，并拒绝重复 sink 与 sink 外直接写入。
- [ ] sink 统一执行 `raw hit → armor → effective severity/grade → deterministic downgrade → health/bleeding/contamination/meridian/event consequences`，mapped 三个 wound modifier 只在这里消费。§8.1 冻结稳定 attack/hit identity，以及已结算 hit ledger 的 owner、持久化/保留与清理边界；sink 必须先完成全部可失败校验并构造不可部分失败的提交批次，或采用有明确回滚语义的事务，再在同一原子边界提交全部后果与 identity。确定性 fracture roll 只负责结果稳定，去重 ledger 负责重放幂等；故障注入覆盖缺失目标状态、非法后果输入与每个可失败边界，失败时 wound/health/bleeding/contamination/meridian/event/ledger 全部不变，修正后同一 hit 仍能且只能成功一次；另覆盖同 tick 多 hit、输入重排、水合后重放与 0%/100%。`canonical_hit_ledger_retention_is_pinned`（名称可等义）以 §8.1 选定的 monotonic tick/epoch 与 identity namespace 覆盖 retention-1、retention 等号、retention+1、清理前重放、重复/批量清理幂等、水合后继续保留、tick/clock 回退的 fail-closed 行为、容量上界，以及清理后 identity 是否允许复用及其 generation/epoch 条件。每个 case 同时断言 wound、health、bleeding、contamination、meridian、event 与 hit ledger 要么全不变、要么仅提交一次；批量清理不得误删仍在保留期内的 identity。
- [ ] §8.1 分类型冻结合同：`bruise_threshold_multiplier` 与 `fracture_downgrade_chance` 定义单位、neutral、合法 finite 闭区间、非法值 fail-closed/reject、组合顺序/cap；`cut_pierce_downgrade` 与 `scar_forged_flow_bonus` 是 bool marker，只允许 `false/true`，并覆盖非法反序列化。四字段共同 pin 持久化/水合/reset/到期与 active→inactive 回 neutral；另单独冻结 ScarForged marker 生效时 effective-flow 数值倍率的依据、finite 区间与组合/cap。表驱动测试按实际类型覆盖浮点下界/等号/上界、越界、NaN/±Infinity，以及布尔两态和非法 wire 值。
- [ ] §8.1 的唯一 wound-grade 表对每个 wound kind/grade 的 finite threshold 使用相邻可表示浮点值 `next_down(threshold)`、`threshold`、`next_up(threshold)`（或等价 bit-level 生成）锁定等号归属，禁止使用未定义的 `threshold±ε`。测试先断言阈值 finite、相邻值严格有序且未跨越相邻 grade 阈值；若端点不存在某一侧相邻值，§8.1 必须明确该端点策略。三点均断言最终 grade 与 health、bleeding、contamination、meridian、event 的完整派生后果。
- [ ] `effective_meridian_sum_rate` 只在 `scar_forged_flow_bonus` active 时对 `ActiveScarCircuits` 涉及的去重经脉应用 §8.1 决定的倍率；共享经脉只加成一次，不持久改 `Meridian.flow_rate`。
- [ ] 本批不声称迁移全仓所有历史 wound/health writer；未经过 `resolve_attack_intents` 的旁路属于附录观察，须独立 canonical finding 才能扩 scope。
- [ ] 可核验 symbol：`CanonicalWoundSink`、`canonical_wound_sink_is_unique`、`effective_wound_grade`、`wound_grade_thresholds_are_pinned`、`cocoon_fracture_roll`、`effective_meridian_sum_rate`、`iron_cocoon_downgrade_changes_full_wound_consequences`、`scar_forged_bonus_only_applies_to_active_circuits`（名称可等义）。

### P1-C — `jump_height_multiplier` authority

- [ ] §8.1 先选择唯一互斥路线，并冻结字段表示 apex-height multiplier 还是 initial-velocity multiplier、合法 finite 区间与 apex 容差。MC 1.20.1 离散重力、阻力、碰撞和 tick 更新顺序必须锚定确切上游版本/映射/类/方法，不得凭经验自定；client/server 共享公式或同一组 golden tick/velocity/apex vectors。非法值统一 fail-closed 到 1.0 或拒绝。
- [ ] **server-authoritative 路线**：接 server movement/velocity、production schedule、非法纵向速度拒绝与真实 client/bot apex e2e；禁止新增无 runtime reader 的 proto/store 字段。
- [ ] **client-hook + server-validation 路线**：从 `server/src/network/derived_attrs_emit.rs` 的 `DerivedAttrs` query、payload 写入与 production send schedule 开始，贯通 `DerivedAttrsSyncV1`、proto/generated、handler/store、非 mixin helper、jump hook 与 disconnect reset。消息采用 session generation + 单调 revision/effective tick 的权威全量状态；同一 session 激活→停用/到期时 server 必须预发布显式 `jump_height_multiplier=1.0` 的更高 revision，并按 §8.1 选择 ACK 后生效或双方已知的有界 future effective tick，配套可靠重传、超时 fail-closed 与已确认 revision 校验规则；禁止 server 在 client 可能尚未收到 neutral 时单方面切到 1.0。字段缺失、旧 revision 或旧 session 不得清除/复活状态。本子合同仅在 §8.1 选择 client 路线时生效：§8.1 还必须冻结 session/revision/effective-tick 的 wire 类型、位宽和比较算法，定义 server/client 进程重启后的 session 唯一性来源，并禁止 revision 静默回绕。revision 耗尽时只能在显式 neutral 交接下切换到不可复用的新 session，或停止发送并 fail-closed；ACK 仅在 `(session generation, revision, effective tick)` 全部精确匹配时有效，跨 session 的旧 active、neutral 或 ACK 均不得改变当前状态。
- [ ] 两路线共同测试 multiplier 1.0/中间/上限与非法输入，并以真实 velocity/apex 而不是 payload/store 值验收；共同覆盖 active→neutral、到期/reset、断线/实体重建/重登、水合后旧状态不复活，以及转换边界前后起跳。server 路线另证明 production schedule 每次读取当前权威倍率且下一次起跳恢复 1.0；client 路线另覆盖旧 active 与 neutral 乱序、重复 neutral、旧 session neutral、neutral 延迟跨 tick、丢失/重传、ACK 迟到/丢失、超时、effective tick 前后起跳交错、revision max-1/max/耗尽与回绕尝试、server/client 分别重启、session 标识冲突、旧 active/neutral 跨 session 到达及旧 ACK 抵达新 session。选择 server-authoritative 路线时，client wire 测试明确记为 N/A，且不得遗留 proto/store 字段。
- [ ] 可核验 symbol：`sanitized_jump_height_multiplier`、`effective_jump_velocity`、`jump_physics_golden_vectors_match_mc_1_20_1`、`guangbo_jump_height_changes_observed_apex`；client 路线另含 `DerivedAttrsSyncV1`、`DerivedAttrsStore`、`jump_modifier_resets_on_disconnect`、`jump_modifier_neutral_revision_clears_same_session`（名称可等义）。

## §8 开放问题（P0 前须追加 §8.1 决议）

1. `ContaminationBoost` 的 magnitude 单位/合法 finite 闭区间/非法值策略、duration 的 0/上下界/溢出策略、stack/refresh/expiry 与 ledger 接缝是什么？
2. JinZhongDan 的负面 kind、基础强度，以及 `neg_scale` 的单位、合法 finite 闭区间、非法值策略与组合公式是什么？
3. r7 五字段与 r8 #5 十三字段逐项的单位、neutral、finite 区间、组合/累计、消费时点、持久化/水合/reset 是什么？
4. canonical wound grade 的阈值/等号归属是什么；两个浮点 wound modifier 的单位、neutral、合法 finite 闭区间、非法值策略、组合/cap 与生命周期是什么；`cut_pierce_downgrade` bool marker 的两态、非法反序列化与生命周期是什么；stable attack/hit identity、deterministic roll，以及已结算 hit ledger 的 owner、原子 check-and-record、持久化/保留/清理边界如何定义；保留边界采用何种 monotonic tick/epoch，清理后 identity 是否允许复用及其 generation/epoch 条件、容量上界与时钟回退策略是什么；提交采用“全部预校验后不可失败批次”还是可回滚事务，各失败边界如何保证全状态零副作用？
5. `scar_forged_flow_bonus` bool marker 的两态、非法反序列化与生命周期是什么；marker 生效时 effective-flow 倍率的 canonical 数值依据、合法 finite 闭区间、非法值策略、适用经脉、组合/cap 与持久化/水合/reset/到期语义是什么？当前代码注释中的 +5% 不能在无决议时自动视为正典。
6. jump 选择哪条 authority 路线；字段表示 apex 还是 velocity、合法范围、权威 MC 1.20.1 源码/映射锚点、离散公式/容差是什么？若选择 client 路线，还须冻结 session/revision/effective-tick 的 wire 类型与位宽、比较算法、revision 耗尽且禁止回绕的策略、server/client 重启后的 session 唯一性、explicit-neutral + ACK/future-effective-tick/重传/超时合同，以及 ACK 对 `(session, revision, effective tick)` 的精确匹配规则。
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

P0 只允许把这些条目标为 `UnmappedObservation` 并验证“未被本 plan 消费”；不得以 inventory 名义把它们重新变成本 plan 交付物。

## §10 串行多 PR 实施边界

本 skeleton 按当前 successor 调度授权采用**同一 active plan 下的串行多 PR**；前一 PR merge 后才开下一 PR。该授权只改变本 plan 的 PR 切分，不允许修改其他 plan，也不允许把附录观察混入实现。

本 plan 的机器可读阶段合同固定为：

```json
{"protocol":"consume-plan-serial-multi-pr-v1","stages":["Decision","P0","P1-A","P1-B","P1-C","Closure"]}
```

该精确列表不得由阶段总览中的 `P0/P1` 粗粒度解析替代。

1. **Decision PR**：骨架 promotion 后只追加完整 §8.1 双锚点决议；七项未收口不得启动 P0。
2. **P0 PR**：只实现 inventory、producer-site、`MappedPendingClosure` manifest 与分阶段 gate；不得预填不存在的 consumer，不得迁移附录域。
3. **P1-A PR**：只闭合 r6、r7 与 r8 #5 mapped Alchemy/Insight finding；同 PR 原子交付 production wiring/schedule/differential + pin tests，并转换本批 manifest 状态。
4. **P1-B PR**：只闭合 r8 #2/#3；同 PR 完成唯一 `CanonicalWoundSink`、effective flow、production tests 与本批状态转换，不迁移 repo-wide writer。
5. **P1-C PR**：只闭合 r8 #4；按 §8.1 选定的一条 jump 路线完整交付并转换状态，另一条不得留下死 schema/store。
6. **Closure/Archive PR**：机械拒绝残留 `MappedPendingClosure`，汇总 11 条 finding、各实现 PR exact HEAD、`/review` 与 GitHub e2e run URL/ID/result，填写 `## Finish Evidence` 并迁入 `docs/finished_plans/`；不得首次新增 production wiring。

每个实现 PR 都必须：fresh `origin/main` 复验 → production wiring + 饱和测试 → fresh-context exact-HEAD validator → 按所触栈完整 gate → `git diff --check` + `python3 scripts/plans_progress.py --check` → 紧邻 `git fetch origin && git merge origin/main` → HEAD 变化后对新 SHA 重跑 validator 与受影响 gate → push → local/remote/PR HEAD SHA 三方对拍 → 独立 `/review` 并等待 `/review` 与 CodeRabbit 收敛。任何 review 修改都必须对更新后的 exact HEAD 重走上述闭环。本地严禁运行 `scripts/test-tmux-shutdown-order.sh` 或任何调用它的 suite；该覆盖只留给 GitHub e2e。

`modifier_archive_rejects_pending_closure` 必须接入实际 Active→Finished 迁档入口及对应 CI（可扩展 `scripts/plans_progress.py --check` 或使用等价唯一入口），不得只存在为可直接调用的 helper 单测；集成测试必须证明含 pending、无效 Retired 证据或被擅自扩大的 observation allowlist 时真实迁档失败，仅在全部 closed 或具备完整退役证据时成功。

### §10.1 外部 `/consume-plan` capability preflight（非 finding、非实施阶段）

本 gameplay plan 不授权修改 `skills/consume-plan/SKILL.md`、command loader/registry、workflow 状态存储或其测试；这些工作既不是第 12 条 canonical finding，也不是共享 P0 gate。骨架 promotion 后、Decision PR 之前，runner 必须只读执行：

`/consume-plan --preflight bughunt-modifier-effect-consumer-completion-v1`

preflight 不得创建 worktree、branch、commit 或 PR，必须针对 exact active-plan HEAD 返回机器可读 PASS，至少包含 `protocol`、`plan_head`、`workflow_head`、上述六阶段精确列表及 capability-test run ID。命令不存在、协议不支持、阶段列表不精确或测试证据缺失，一律返回 `WORKFLOW_CAPABILITY_BLOCKED`：不得开 Decision PR，不得进入 P0/P1，也不得把 workflow 缺口计入 modifier manifest。修复必须由独立 workflow blocker/PR 完成；其 merge 后在同一 exact plan HEAD 重新执行 preflight。

`consume-plan-serial-multi-pr-v1` 的外部 capability suite 必须机械证明：

1. 同一时刻至多一个阶段 PR；前一 PR 未 merge 不得派发下一阶段。
2. 持久 execution ID、单 owner lease/CAS、逐阶段幂等 operation key 与明确 linearization point。
3. 外部副作用采用“先写 intent → 以 operation key 创建/发现 PR → 对账 GitHub 状态 → CAS 提交阶段”的恢复协议，不宣称 PR 创建、merge 与本地游标具有不可能实现的跨系统原子事务。
4. 确定性故障注入覆盖重复命令、双 runner、派发前后崩溃、PR 创建后未登记、merge 后游标未更新、证据写入失败、陈旧开放 PR、review/e2e 失败、SHA 漂移与 merge conflict；断言不越级、不产生两个有效后继 PR、已 merge 阶段不重复执行且 Closure 只归档一次。
5. Closure 只能在五个前序 PR 已 merge、`MappedPendingClosure=0` 且 Retired 证据有效后启动。

正常的真实 `/consume-plan` run 只作为 happy-path smoke evidence；暂停/恢复与并发安全由上述 workflow-owned deterministic fixture 证明，不要求本 gameplay plan 制造故障。

### §10.2 preflight PASS 后的单次消费

仅在 §10.1 PASS 后，用户的一次 `/consume-plan` 才可按 Decision → P0 → P1-A → P1-B → P1-C → Closure 串行运行。每个阶段使用 fresh `origin/main`、独立实施/返工 agent 与独立 PR；前一 PR merge 后才创建下一阶段分支。任一 blocker/major、测试失败、SHA 漂移或 merge conflict 均暂停在当前阶段，修复后以同一 operation key 对账远端状态并从该阶段 exact HEAD 续跑，不跳阶段、不重复派发、不并行消费附录域。Finish Evidence 记录 preflight 的 `workflow_head`/run ID、每个阶段 PR 与 merge SHA，以及真实运行中的暂停/恢复事件；不得用一次无故障 run 冒充 workflow 故障分支证明。全部实现 PR 收敛且真实归档入口确认 pending 为 0、Retired 证据有效后，Closure PR 才可归档，merge 后流程结束。

## 归档门

- 11 条 mapped finding（含 r8 #5 冻结的 13 个子字段）全部转为 `MappedGameplayClosed`，或按 §8.1 删除/禁用 producer、迁移状态并以机械证据转为 `MappedProducerRetired`；`MappedPendingClosure` 必须为 0，Retired 的 writer/consumer/schedule 必须不可达且迁移证据完整，不得以 `UnmappedObservation`、HUD/storage read 或 planned 状态结案。
- P0 完整库存精确对账，exact-HEAD 冻结的附录 observation allowlist 未被普通变更扩大；基线内观察仍保持“未被本 plan 消费”且不阻塞本 plan，基线后新增 orphan 则 fail-closed。
- Alchemy/Insight、Iron Cocoon wound/effective flow、jump 三条 mapped production chain 均有 observable differential/e2e；数值、生命周期、wound 阈值与 jump neutral/物理 vectors 均有 pin，qi 路径有 ledger 守恒证据。
- Finish Evidence 只在所有实现 PR 的 validator、按栈 gate、`git diff --check`、plan 进度检查、local/remote/PR SHA 对拍、更新后 exact-HEAD `/review`、CodeRabbit 与 GitHub e2e 全部收敛后填写；记录 commit SHA、validator verdict、命令结果、run URL/ID 与遗留 successor。
