# plan-bughunt-modifier-effect-consumer-completion-v1（骨架）

> 一句话主题：系统性补齐 `DerivedAttrs` / `InsightModifiers` / alchemy `StatusEffectKind` 的生产→消费→可观察玩法链，并以 consumer manifest 阻止新 modifier 再次成为孤岛。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | canonical 字段/效果清单 + producer/consumer/owner 矩阵 | ⬜ |
| P1 | 最小 anti-orphan manifest/gate + alchemy effect 极性与污染消费闭环 | ⬜ |
| P2 | 活茧伤口档次 + 茧灵 effective flow | ⬜ |
| P3 | Insight 正向/负向字段按 gameplay loop 分批接线 | ⬜ |
| P4 | `jump_height_multiplier` 依赖 R6 emit/bridge + R2 store/reset 冻结 API 后，仅落 gameplay consumer | ⬜ |
| P5 | manifest 全量覆盖、lint 强化与全栈 gate | ⬜ |

## 接入面

- **进料**：`server/src/combat/components.rs::DerivedAttrs`、`iron_cocoon_passive_system`、`server/src/cultivation/insight_apply.rs::InsightModifiers`、`server/src/alchemy/events.rs::StatusEffectKind`、`side_effect_apply.rs`、`pill.rs`、`apply_guangbo_ticao_bonuses`。
- **出料**：伤口结算、经脉 effective flow、回气/突破/颜色效率/过载/感知/恢复玩法循环、污染 tick、server-authoritative movement 与 client jump。
- **共享类型 / event**：复用 `DerivedAttrs`、`InsightModifiers`、`StatusEffects`、`DerivedAttrsSyncV1`、既有 qi ledger；禁止建立 parallel modifier component 或按玩法复制字段。
- **跨仓库契约 / 文件所有权**：P1-P3 以 server 领域调用方为主。P4 严格服从 master §4，不设 focused 例外：R6 独占并先交付 Rust schema/proto/generated、server `network/*_emit.rs` 与 `network/emit/` builder、`schema/proto_convert.rs`、client `network/` bridge/router 及 `BongNetworkHandler.register()` channel-registration 区段和对应 wire 契约 pin；R2 随后独占并交付 client `DerivedAttrsStore` adapter/登记、session reset API 与 `BongNetworkHandler.clearClientStateOnDisconnect` 区段的断线清理 pin；本 focused plan 不修改上述 R6/R2 独占区段，只在两者 API 合入冻结后消费它们，实施 server/client jump gameplay consumer、同一玩法差分与 bot/e2e 验收。
- **worldview 锚点**：伤口档次 `worldview.md §四 L250-L260`；经脉流量 §四 L275-L288；污染排异 §四 L344-L351；爆脉体修 §五 L401-L405。
- **qi_physics 锚点**：倍率只能作用于现有 rate/cost/effective value；吸收继续走 `regen_from_zone`，排异继续走 `release_qi_amount_to_zone`，不得铸造、蒸发或双重记账。

## P0 Canonical Scope

### 已完成/明确排除

- PR #1143 / commit `3e698151` 已接通 `DerivedAttrs.reach_bonus`、`qi_regen_multiplier`、`contam_purge_multiplier`；`healing_rate_multiplier` 原本已有 runtime consumer，均不重复。
- `composure_recover_mul` 虽是影子字段，但 `apply_choice` 已直改 `Cultivation.composure_recover_rate` 且 gameplay 生效，不按 no-op 修复。
- `zhenfa_concealment`、`zhenfa_disenchant` 与 lifespan enlightenment 已有 consumer，不纳入。

### 仍需闭环

1. **Alchemy**：`ContaminationBoost` 已生产/持久化/HUD 展示，但 `contamination_tick` 不读；`JinZhongDan` 的 negative slot 当前放入正向 `QiRegenBoost`，现有回气 consumer 会把“负面”变成真实增益。
2. **Iron cocoon**：`bruise_threshold_multiplier`、`fracture_downgrade_chance`、`cut_pierce_downgrade`、`scar_forged_flow_bonus`。
3. **Insight**：`qi_regen_mul`、`next_breakthrough_bonus`、`vortex_backfire_resist_mul`、`vortex_delta_bonus_add`、`vortex_flow_speed_mul`、`hunyuan_threshold_mul`、`chaotic_tolerance_add/loss`、`overload_tolerance_add`、`opposite_color_efficiency_penalty`、`main_color_efficiency_penalty`、`qi_volatility_add`、`shock_sensitivity_add`、`overload_fragility_add`、`reaction_window_penalty`、`breakthrough_failure_penalty_mul`、`sense_exposure_add`、`meridian_heal_slowdown_mul`。
4. **Observe partial chain**：`observe_chance_bonus` 当前仅有声明/默认值（`server/src/cultivation/insight_apply.rs:34,81`），没有 effect 写入；`server/src/cultivation/technique_observe.rs:64-87` 的 `observe_learn_chance` 会读取它，但 `server/src/cultivation/technique_observe.rs:90-134` 的 `evaluate_observe_attempt` 调用点仅在 tests（`server/src/cultivation/technique_observe.rs:253,280,315`），没有 production caller。必须同时接真实 producer 与 observe 入口，或明确退役，不能把 helper-level read 标成已修。
5. **Jump**：`jump_height_multiplier` 已由广播体操 proficiency 生产；`move_speed_multiplier` 兄弟字段已生效，但 jump 无 server/client runtime consumer 或 wire 字段。

## 当前证据（origin/main @ c625d5a5）

- `server/src/alchemy/side_effect_apply.rs:18-31` 把 `contam_boost` 映射到 `ContaminationBoost`；`server/src/cultivation/contamination.rs` 的 tick query/公式不读 `StatusEffects`。
- `server/src/alchemy/pill.rs:632-642` 在 `JinZhongDan` negative duration 下 push `QiRegenBoost`；`server/src/cultivation/tick.rs::qi_regen_boost_multiplier` 已把它作为正向倍率消费。
- `server/src/combat/baomai_v4/iron_cocoon.rs:110-140` 写四个字段；伤口 resolve 与 `MeridianSystem::sum_rate` 不读。
- `server/src/cultivation/insight_apply.rs` 写上述 Insight 字段；除排除项外缺生产 gameplay consumer。
- `server/src/combat/body_conditioning.rs` 写 `jump_height_multiplier`；server movement、`DerivedAttrsSyncV1` 与 client store 均无 jump consumer。

## 设计门与验收

1. **anti-orphan 前置门（继承 r8 audit 决议）**：P1-P4 的 implementation 不得在不存在 checked-in consumer manifest 和“新增字段只出现在 default/write/test 时必红”的最小 gate 时开始。优先先完成 P5；唯一例外是 P1 的首个 PR 可同包交付覆盖当前字段的最小 manifest/gate。后续 P1-P4 每个字段接线 PR 必须同步更新并通过该 gate；完整 lint 可在 P5 继续强化。manifest 不替代同一 gameplay loop 的可观察差分验收，`planned_no_consumer` 仍须有 owner/reason。
2. 每项必须以“同一 gameplay loop，写入前后可观察差分”验收；只测 struct 数值、helper 或 schema 不算完成。
3. 活茧伤口降级要冻结作用点、deterministic RNG 与 wound kind/grade 一致性；`scar_forged_flow_bonus` 只能进入 effective rate，不得永久改 `Meridian.flow_rate`。
4. Insight 按回气/突破、颜色/混元、过载/经脉、感知/反应四组拆 atomic PR；所有负向 cost 也必须真实可感知。
5. **Jump 三段依赖链（无文件所有权例外）**：
   1. R6 先交付并冻结 `jump_height_multiplier` 的 Rust schema/proto/generated、server `network/*_emit.rs` / `network/emit/` builder / `schema/proto_convert.rs`、client bridge/router 与 `BongNetworkHandler.register()` channel-registration API，契约 pin 证明 wire 值从 server 到 client bridge 可达；
   2. R2 在 R6 契约合入后交付并冻结 client store adapter/registry、session reset 与 `BongNetworkHandler.clearClientStateOnDisconnect` cleanup API，生命周期 pin 证明重连不保留陈旧倍率；
   3. 本 focused P4 等 R6、R2 两段均合入后，只消费冻结 API，落 jump gameplay consumer 与玩法/bot 差分；不得修改 `network/*_emit.rs`、`network/emit/`、`schema/proto_convert.rs`、client bridge/router、`BongNetworkHandler.register()` channel-registration、Store 生命周期或 `clearClientStateOnDisconnect` 独占区段。
   三段可分 PR 顺序合入，但 P4 在 R6/R2 API 未齐时保持 BLOCKED；任何阶段都不能只凭 schema/store 数值测试宣称 jump 已闭环。
6. P5 manifest 对 `DerivedAttrs`、`InsightModifiers`、非展示 `StatusEffectKind` 逐字段记录至少一个 production consumer 或带 owner/reason 的 `planned_no_consumer`；新增只有 default/write/test 的字段时 gate 必须红。
7. 按触栈运行完整 server gate；R6 修改 schema/bridge 时由 R6 重建 schema dist 并跑其跨端契约 gate，R2 修改 store/reset 时由 R2 使用 Java 17 跑 client lifecycle gate；focused P4 仅对自己消费的冻结 API 跑 server/client gameplay consumer 与 bot/e2e 场景。

## 去重边界

- 本 plan 是 r6 P1、r7 P0、r8 modifier audit P2-P5 与 r8 findings #2/#3/#4/#5 的唯一 successor。
- 不重新实施 PR #1143；不接管距离衰减、Freeze 容器、JueBi marker、TSY cleanup 等独立 lifecycle/domain finding。
- 原 `plan-bughunt-r8-modifier-orphan-audit-v1` 作为验证与 P1 落地历史归档，不再作为 active implementation queue。
