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

`composure_recover_mul` 不属于 r8 #5：canonical r8 已确认其 sibling `Cultivation.composure_recover_rate` 有 gameplay consumer。`practices` 动态 marker 及本表外字段不因 P0 inventory 自动进入 mandatory scope。

## 第一性验真（`origin/main @ de75f14e43daf1105ea978c43d187acbb7f12f14`，2026-07-29）

1. `ContaminationBoost` 可生产、可 upsert、可显示、会到期，但 `server/src/cultivation/contamination.rs:97-205` 不读 `StatusEffects`。
2. JinZhongDan negative slot 在 `server/src/alchemy/pill.rs:632-642` 仍生产正向 `QiRegenBoost(0.001)`；现有 consumer 按 `1 + magnitude` 增益回气。
3. r7 五字段与 r8 #5 冻结的 13 个子字段由 `server/src/cultivation/insight_apply.rs:24-254` 写入；production reachability 与 reader 缺口须由 P0 逐项登记，不能以聚合名称略过。
4. Iron Cocoon 四字段由 `server/src/combat/baomai_v4/iron_cocoon.rs:99-143` 写入；`combat::resolve_attack_intents` 与 `MeridianSystem::sum_rate` 未消费。
5. `jump_height_multiplier` 有 producer/reset，但 `server/src/network/derived_attrs_emit.rs:76-90`、`DerivedAttrsSyncV1`、client store 与真实 jump 链均无 consumer。
6. `server/src/test_coverage_guards.rs` 现有 event reader/writer 扫描不能证明上述 mapped modifier 已有 production consumer。

## P0 — mapped anti-orphan contract

- [ ] 建立 greppable `ModifierConsumerContract` / `MODIFIER_CONSUMER_MANIFEST`（名称可等义）。机械穷举 `DerivedAttrs` 字段、`InsightModifiers` 字段与 `StatusEffectKind` variant 全集，再与 manifest 精确集合对账；新增/删除/改名未登记即失败。
- [ ] 使用互斥生命周期分类：`MappedPendingClosure`、`MappedGameplayClosed`、`MappedProducerRetired`、`ExistingGameplay`、`DormantNoProducer`、`UnmappedObservation`。P0 时本 plan 的未闭环成员合法登记为 `MappedPendingClosure`，只要求 canonical finding ID、稳定 producer-site/reachability 证据、owner、目标 P1 批次、预期 consumer domain 与测试 ID；**不要求尚不存在的 production caller、schedule 或差分测试先通过**。
- [ ] 各 P1 PR 必须在同一变更中接通 production consumer + schedule + observable differential test，并把本批项目原子转为 `MappedGameplayClosed`；若决议删除语义，则删除/禁用 producer、迁移持久状态后转为 `MappedProducerRetired`。`MappedGameplayClosed` 才启用 scheduled-consumer 强门禁，归档门拒绝任何 `MappedPendingClosure`。
- [ ] `UnmappedObservation` 只记录域、证据、triage owner 与“先建 canonical finding + successor”退出条件；不得生成 typed migration、gameplay consumer 或本 plan 归档前置。新增观察不会自动扩张 P1。
- [ ] 对 11 条 finding 的每个 writer 使用稳定 typed producer-site ID；同函数不同 branch 不得折叠，r8 #5 必须与上表 13 项精确相等。P0 不要求把未映射动态字符串域做全仓 typed migration。
- [ ] 可核验 symbol：`MODIFIER_CONSUMER_MANIFEST`、`ModifierProducerSiteId`、`modifier_consumer_manifest_stays_current`、`modifier_producer_sites_stay_current`、`mapped_pending_requires_closure_target`、`gameplay_closed_requires_scheduled_consumer`、`modifier_archive_rejects_pending_closure`、`unmapped_observation_requires_successor_before_consumption`（名称可等义）。

## P1 — mapped gameplay closure

### P1-A — Alchemy + mapped Insight

- [ ] 冻结 `ContaminationBoost` 的 magnitude、duration、stack/refresh/expiry 语义，并让 `contamination_tick` 通过 canonical qi ledger 产生可观察差分。
- [ ] JinZhongDan negative slot 改为语义明确的负面 regen effect；`neg_scale`、0/默认/max、到期回 neutral 与重复 upsert 均有完整链测试。
- [ ] r7 五字段与 r8 #5 的 13 个冻结字段按表中 gameplay domain 接入唯一 effective helper；P0 若证明某 writer production 不可达，也必须在本批删除/禁用该 writer并迁移状态，不得改标 `UnmappedObservation` 消项。
- [ ] §8.1 逐字段冻结单位、neutral、finite 区间、add/mul 顺序、累计上限、消费时点、持久化/水合/reset。表驱动 pin 覆盖合法下界/等号/上界、越界、NaN/±Infinity、组合顺序/cap、持久化往返、水合与 reset/到期回 neutral；另有“相同 gameplay 输入，仅 modifier 不同”的 production-schedule differential test。qi gain/drain 继续断言 `WorldQiAccount` / `QiTransfer` 守恒。
- [ ] 可核验 symbol：`ContaminationBoost`、`contamination_tick`、`CombatPillKind::JinZhongDan`、`QiRegenSlowed`、`insight_qi_regen_multiplier`、`effective_breakthrough_bonus`、`effective_overload_threshold`、`effective_vortex_delta`、`effective_vortex_flow_speed`、`mapped_insight_modifier_contract_is_pinned`、`mapped_insight_modifier_changes_gameplay`（名称可等义）。

### P1-B — Iron Cocoon wound grade + effective flow

- [ ] `combat::resolve_attack_intents` 内建立唯一 typed `CanonicalWoundSink`（名称可等义）：该 sink 是本 pipeline 唯一允许最终构造/写入 `Wound` 与派生后果的位置；参与该 pipeline 的 damage producer必须调用它。门禁 pin sink **恰好一个**、production 可达，并拒绝重复 sink 与 sink 外直接写入。
- [ ] sink 统一执行 `raw hit → armor → effective severity/grade → deterministic downgrade → health/bleeding/contamination/meridian/event consequences`，mapped 三个 wound modifier 只在这里消费。确定性 fracture roll 由稳定 attack/hit identity 派生，重复投递幂等；覆盖同 tick 多 hit、输入重排、0%/100% 与重放。
- [ ] §8.1 的唯一 wound-grade 表按每个 wound kind/grade 阈值测试 `threshold-ε`、`threshold`、`threshold+ε`，同时断言最终 grade 与 health、bleeding、contamination、meridian、event 全后果，锁定等号归属。
- [ ] `effective_meridian_sum_rate` 只在 `scar_forged_flow_bonus` active 时对 `ActiveScarCircuits` 涉及的去重经脉应用 §8.1 决定的倍率；共享经脉只加成一次，不持久改 `Meridian.flow_rate`。
- [ ] 本批不声称迁移全仓所有历史 wound/health writer；未经过 `resolve_attack_intents` 的旁路属于附录观察，须独立 canonical finding 才能扩 scope。
- [ ] 可核验 symbol：`CanonicalWoundSink`、`canonical_wound_sink_is_unique`、`effective_wound_grade`、`wound_grade_thresholds_are_pinned`、`cocoon_fracture_roll`、`effective_meridian_sum_rate`、`iron_cocoon_downgrade_changes_full_wound_consequences`、`scar_forged_bonus_only_applies_to_active_circuits`（名称可等义）。

### P1-C — `jump_height_multiplier` authority

- [ ] §8.1 先选择唯一互斥路线，并冻结字段表示 apex-height multiplier 还是 initial-velocity multiplier、合法 finite 区间与 apex 容差。MC 1.20.1 离散重力、阻力、碰撞和 tick 更新顺序必须锚定确切上游版本/映射/类/方法，不得凭经验自定；client/server 共享公式或同一组 golden tick/velocity/apex vectors。非法值统一 fail-closed 到 1.0 或拒绝。
- [ ] **server-authoritative 路线**：接 server movement/velocity、production schedule、非法纵向速度拒绝与真实 client/bot apex e2e；禁止新增无 runtime reader 的 proto/store 字段。
- [ ] **client-hook + server-validation 路线**：从 `server/src/network/derived_attrs_emit.rs` 的 `DerivedAttrs` query、payload 写入与 production send schedule 开始，贯通 `DerivedAttrsSyncV1`、proto/generated、handler/store、非 mixin helper、jump hook 与 disconnect reset。消息采用 session generation + 单调 revision/effective tick 的权威全量状态；同一 session 激活→停用/到期时 server 必须发送显式 `jump_height_multiplier=1.0` 的更高 revision，client 覆盖旧值，server 自 effective tick 起按 1.0 校验。字段缺失、旧 revision 或旧 session 不得清除/复活状态。
- [ ] 两路线共同测试 multiplier 1.0/中间/上限与非法输入，并以真实 velocity/apex 而不是 payload/store 值验收；client 路线另覆盖 active→neutral、旧 active 与 neutral 乱序、重复 neutral、旧 session neutral、断线/重登及起跳交错。
- [ ] 可核验 symbol：`sanitized_jump_height_multiplier`、`effective_jump_velocity`、`jump_physics_golden_vectors_match_mc_1_20_1`、`guangbo_jump_height_changes_observed_apex`；client 路线另含 `DerivedAttrsSyncV1`、`DerivedAttrsStore`、`jump_modifier_resets_on_disconnect`、`jump_modifier_neutral_revision_clears_same_session`（名称可等义）。

## §8 开放问题（P0 前须追加 §8.1 决议）

1. `ContaminationBoost` 的单位、stack/refresh/expiry 与 ledger 接缝是什么？
2. JinZhongDan 的负面 kind、基础强度与 `neg_scale` 公式是什么？
3. r7 五字段与 r8 #5 十三字段逐项的单位、neutral、finite 区间、组合/累计、消费时点、持久化/水合/reset 是什么？
4. canonical wound grade 的阈值/等号归属是什么；stable attack/hit identity 与 deterministic roll 如何定义？
5. ScarForged 倍率的 canonical 数值依据、单位、适用经脉与组合公式是什么？当前代码常数 1.05 不能在无决议时自动视为正典。
6. jump 选择哪条 authority 路线；字段表示 apex 还是 velocity、合法范围、权威 MC 1.20.1 源码/映射锚点、离散公式/容差及 client 路线 revision + explicit-neutral 合同是什么？
7. P0 inventory/producer-site 的机械权威源采用 Rust AST、唯一声明宏还是同等方案？

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

1. **Decision PR**：骨架 promotion 后只追加完整 §8.1 双锚点决议；七项未收口不得启动 P0。
2. **P0 PR**：只实现 inventory、producer-site、`MappedPendingClosure` manifest 与分阶段 gate；不得预填不存在的 consumer，不得迁移附录域。
3. **P1-A PR**：只闭合 r6、r7 与 r8 #5 mapped Alchemy/Insight finding；同 PR 原子交付 production wiring/schedule/differential + pin tests，并转换本批 manifest 状态。
4. **P1-B PR**：只闭合 r8 #2/#3；同 PR 完成唯一 `CanonicalWoundSink`、effective flow、production tests 与本批状态转换，不迁移 repo-wide writer。
5. **P1-C PR**：只闭合 r8 #4；按 §8.1 选定的一条 jump 路线完整交付并转换状态，另一条不得留下死 schema/store。
6. **Closure/Archive PR**：机械拒绝残留 `MappedPendingClosure`，汇总 11 条 finding、各实现 PR exact HEAD、`/review` 与 GitHub e2e run URL/ID/result，填写 `## Finish Evidence` 并迁入 `docs/finished_plans/`；不得首次新增 production wiring。

每个实现 PR 都必须：fresh `origin/main` 复验 → production wiring + 饱和测试 → fresh-context exact-HEAD validator → 按所触栈完整 gate → 紧邻 `git fetch origin && git merge origin/main` → HEAD 变化后重验 → push → 独立 `/review`。本地严禁运行 `scripts/test-tmux-shutdown-order.sh` 或任何调用它的 suite；该覆盖只留给 GitHub e2e。

## 归档门

- 11 条 mapped finding（含 r8 #5 冻结的 13 个子字段）全部转为 `MappedGameplayClosed`，或按 §8.1 删除/禁用 producer、迁移状态并转为 `MappedProducerRetired`；`MappedPendingClosure` 必须为 0，不得以 `UnmappedObservation`、HUD/storage read 或 planned 状态结案。
- P0 完整库存精确对账，且附录域仍保持“未被本 plan 消费”；它们的存在不冒充本 plan finding，也不阻塞本 plan。
- Alchemy/Insight、Iron Cocoon wound/effective flow、jump 三条 mapped production chain 均有 observable differential/e2e；数值、生命周期、wound 阈值与 jump neutral/物理 vectors 均有 pin，qi 路径有 ledger 守恒证据。
- Finish Evidence 只在所有实现 PR 的 `/review` 与 GitHub e2e 对对应 exact HEAD 收敛后填写；记录 commit SHA、validator verdict、命令结果、run URL/ID 与遗留 successor。
