# plan-bughunt-modifier-effect-consumer-completion-v1（骨架）

> **骨架（草案）**。一句话主题：只收束 r6/r7/r8 已映射的 11 条 canonical modifier/effect finding，并建立一套共享 anti-orphan gate；未进入 Canonical Finding Mapping 的观察不属于本 plan 交付物。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 11 条 mapped finding 的 producer/consumer manifest、完整类型库存与防孤岛门禁 | ⬜ |
| P1 | 按 Alchemy/Insight、Iron Cocoon、jump 三批接通 mapped gameplay consumer | ⬜ |

## 范围原则

- **唯一强制范围**：下表 11 条 canonical finding，以及为机械验收这些 finding 所必需的 P0 共享门禁。
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
| r8 #5 | 其余 live `InsightModifiers` benefit/cost cluster | P1-A |
| r8 #4 | `jump_height_multiplier` | P1-C |

计数：r6 两条 + r7 五条 + r8 四条 = **11 条**。P0 是共享实施门，不是第 12 条 finding。

## 第一性验真（`origin/main @ de75f14e43daf1105ea978c43d187acbb7f12f14`，2026-07-29）

1. `ContaminationBoost` 可生产、可 upsert、可显示、会到期，但 `server/src/cultivation/contamination.rs:97-205` 不读 `StatusEffects`。
2. JinZhongDan negative slot 在 `server/src/alchemy/pill.rs:632-642` 仍生产正向 `QiRegenBoost(0.001)`；现有 consumer 按 `1 + magnitude` 增益回气。
3. r7 五字段与 r8 #5 live cluster 由 `server/src/cultivation/insight_apply.rs:24-254` 写入并持久化，但对应 gameplay loop 没有完整 production reader。
4. Iron Cocoon 四字段由 `server/src/combat/baomai_v4/iron_cocoon.rs:99-143` 写入；`combat::resolve_attack_intents` 与 `MeridianSystem::sum_rate` 未消费。
5. `jump_height_multiplier` 有 producer/reset，但 `server/src/network/derived_attrs_emit.rs:76-90`、`DerivedAttrsSyncV1`、client store 与真实 jump 链均无 consumer。
6. `server/src/test_coverage_guards.rs` 现有 event reader/writer 扫描不能证明上述 mapped modifier 已有 production consumer。

## P0 — mapped anti-orphan contract

- [ ] 建立 greppable `ModifierConsumerContract` / `MODIFIER_CONSUMER_MANIFEST`（名称可等义）。机械穷举 `DerivedAttrs` 字段、`InsightModifiers` 字段与 `StatusEffectKind` variant 全集，再与 manifest 精确集合对账；新增/删除/改名未登记即失败。
- [ ] 每个 inventory member 采用互斥分类：`MappedHere`、`ExistingGameplay`、`DormantNoProducer`、`UnmappedObservation`。`MappedHere` 必须绑定 production producer、production caller/system schedule 与 observable differential test；storage/HUD/test-only/dead-helper read 必须失败。
- [ ] `UnmappedObservation` 只记录域、证据、triage owner 与“先建 canonical finding + successor”退出条件；不得生成 typed migration、gameplay consumer 或本 plan 归档前置。新增观察不会自动扩张 P1。
- [ ] 对 11 条 finding 的每个 writer 使用稳定 typed producer-site ID；同函数不同 branch 不得折叠。P0 不要求把未映射动态字符串域做全仓 typed migration。
- [ ] 可核验 symbol：`MODIFIER_CONSUMER_MANIFEST`、`ModifierProducerSiteId`、`modifier_consumer_manifest_stays_current`、`modifier_producer_sites_stay_current`、`mapped_modifier_requires_scheduled_gameplay_consumer`、`unmapped_observation_requires_successor_before_consumption`（名称可等义）。

## P1 — mapped gameplay closure

### P1-A — Alchemy + mapped Insight

- [ ] 冻结 `ContaminationBoost` 的 magnitude、duration、stack/refresh/expiry 语义，并让 `contamination_tick` 通过 canonical qi ledger 产生可观察差分。
- [ ] JinZhongDan negative slot 改为语义明确的负面 regen effect；`neg_scale`、0/默认/max、到期回 neutral 与重复 upsert 均有完整链测试。
- [ ] r7 五字段与 r8 #5 mapped live cluster 按其既有 gameplay domain 接入唯一 effective helper。§8.1 逐字段冻结单位、neutral、finite 区间、add/mul 顺序、累计上限、消费时点、持久化/水合/reset；`composure_recover_mul` 必须选择单一权威状态，禁止保留会漂移的 sibling duplicate。
- [ ] 每个 mapped 字段都有“相同 gameplay 输入，仅 modifier 不同”的 production-schedule differential test；qi gain/drain 继续断言 `WorldQiAccount` / `QiTransfer` 守恒。
- [ ] 可核验 symbol：`ContaminationBoost`、`contamination_tick`、`CombatPillKind::JinZhongDan`、`QiRegenSlowed`、`insight_qi_regen_multiplier`、`effective_breakthrough_bonus`、`effective_overload_threshold`、`effective_vortex_delta`、`effective_vortex_flow_speed`、`mapped_insight_modifier_changes_gameplay`（名称可等义）。

### P1-B — Iron Cocoon wound grade + effective flow

- [ ] `combat::resolve_attack_intents` 内建立唯一 typed `CanonicalWoundSink`（名称可等义）：该 sink 是本 pipeline 唯一允许最终构造/写入 `Wound` 与派生后果的位置；参与该 pipeline 的 damage producer 必须调用它。门禁 pin sink **恰好一个**、production 可达，并拒绝重复 sink 与 sink 外直接写入。
- [ ] sink 统一执行 `raw hit → armor → effective severity/grade → deterministic downgrade → health/bleeding/contamination/meridian/event consequences`，mapped 三个 wound modifier 只在这里消费。确定性 fracture roll 由稳定 attack/hit identity 派生，重复投递幂等；覆盖同 tick 多 hit、输入重排、0%/100% 与重放。
- [ ] `effective_meridian_sum_rate` 只在 `scar_forged_flow_bonus` active 时对 `ActiveScarCircuits` 涉及的去重经脉应用 §8.1 决定的倍率；共享经脉只加成一次，不持久改 `Meridian.flow_rate`。
- [ ] 本批不声称迁移全仓所有历史 wound/health writer；未经过 `resolve_attack_intents` 的旁路属于附录观察，须独立 canonical finding 才能扩 scope。
- [ ] 可核验 symbol：`CanonicalWoundSink`、`canonical_wound_sink_is_unique`、`effective_wound_grade`、`cocoon_fracture_roll`、`effective_meridian_sum_rate`、`iron_cocoon_downgrade_changes_full_wound_consequences`、`scar_forged_bonus_only_applies_to_active_circuits`（名称可等义）。

### P1-C — `jump_height_multiplier` authority

- [ ] §8.1 先选择唯一互斥路线，并冻结字段表示 apex-height multiplier 还是 initial-velocity multiplier、合法 finite 区间、MC 1.20.1 离散 gravity/drag/tick 换算与 apex 容差；非法值统一 fail-closed 到 1.0 或拒绝。
- [ ] **server-authoritative 路线**：接 server movement/velocity、production schedule、非法纵向速度拒绝与真实 client/bot apex e2e；禁止新增无 runtime reader 的 proto/store 字段。
- [ ] **client-hook + server-validation 路线**：必须从 `server/src/network/derived_attrs_emit.rs` 的 `DerivedAttrs` query、sanitized 非 neutral payload 写入与 production send schedule 开始，贯通 `DerivedAttrsSyncV1`、proto/generated、handler/store、非 mixin helper、jump hook 与 disconnect reset。同步携 session generation + 单调 revision/effective tick（或等价机制），服务端按同一版本校验；覆盖激活、停用、乱序、延迟、重复、旧 session、重登及起跳交错。
- [ ] 两路线共同测试 multiplier 1.0/中间/上限与非法输入，并以真实 velocity/apex 而不是 payload/store 值验收。
- [ ] 可核验 symbol：`sanitized_jump_height_multiplier`、`effective_jump_velocity`、`guangbo_jump_height_changes_observed_apex`；client 路线另含 `DerivedAttrsSyncV1`、`DerivedAttrsStore`、`jump_modifier_resets_on_disconnect`（名称可等义）。

## §8 开放问题（P0 前须追加 §8.1 决议）

1. `ContaminationBoost` 的单位、stack/refresh/expiry 与 ledger 接缝是什么？
2. JinZhongDan 的负面 kind、基础强度与 `neg_scale` 公式是什么？
3. mapped live Insight 字段逐项的单位、neutral、finite 区间、组合/累计、消费时点、持久化/水合/reset 是什么；`composure_recover_mul` 选择哪个唯一权威状态？
4. canonical wound grade 的阈值/等号归属是什么；stable attack/hit identity 与 deterministic roll 如何定义？
5. ScarForged 倍率的 canonical 数值依据、单位、适用经脉与组合公式是什么？当前代码常数 1.05 不能在无决议时自动视为正典。
6. jump 选择哪条 authority 路线；字段表示 apex 还是 velocity、合法范围、离散物理公式/容差及 client 路线 revision 合同是什么？
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
2. **P0 PR**：只实现 mapped manifest、producer-site 与 anti-orphan gate；不得迁移附录域。
3. **P1-A PR**：只闭合 r6、r7 与 r8 #5 mapped Alchemy/Insight finding，并在同 PR 完成 production schedule 与差分测试。
4. **P1-B PR**：只闭合 r8 #2/#3；同 PR完成唯一 `CanonicalWoundSink` 与 effective flow，不迁移 repo-wide writer。
5. **P1-C PR**：只闭合 r8 #4；按 §8.1 选定的一条 jump 路线完整交付，另一条不得留下死 schema/store。
6. **Closure/Archive PR**：只汇总 11 条 finding、各实现 PR exact HEAD、`/review` 与 GitHub e2e run URL/ID/result，填写 `## Finish Evidence` 并迁入 `docs/finished_plans/`；不得首次新增 production wiring。

每个实现 PR 都必须：fresh `origin/main` 复验 → production wiring + 饱和测试 → fresh-context exact-HEAD validator → 按所触栈完整 gate → 紧邻 `git fetch origin && git merge origin/main` → HEAD 变化后重验 → push → 独立 `/review`。本地严禁运行 `scripts/test-tmux-shutdown-order.sh` 或任何调用它的 suite；该覆盖只留给 GitHub e2e。

## 归档门

- 11 条 mapped finding 全部成为真实 `Gameplay` closure，或按 §8.1 删除/禁用其 producer并迁移；不得以 `UnmappedObservation`、HUD/storage read 或 planned 状态结案。
- P0 完整库存精确对账，且附录域仍保持“未被本 plan 消费”；它们的存在不冒充本 plan finding，也不阻塞本 plan。
- Alchemy/Insight、Iron Cocoon wound/effective flow、jump 三条 mapped production chain 均有 observable differential/e2e；qi 路径有 ledger 守恒证据。
- Finish Evidence 只在所有实现 PR 的 `/review` 与 GitHub e2e 对对应 exact HEAD 收敛后填写；记录 commit SHA、validator verdict、命令结果、run URL/ID 与遗留 successor。
