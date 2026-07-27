# plan-bughunt-modifier-effect-consumer-completion-v1（骨架）

> 一句话主题：系统性补齐 `DerivedAttrs` / `InsightModifiers` / alchemy `StatusEffectKind` 的生产→消费→可观察玩法链，并以 consumer manifest 阻止新 modifier 再次成为孤岛。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | canonical 字段/效果清单 + producer/consumer/owner 矩阵 | ⬜ |
| P1 | alchemy effect 极性与污染消费闭环 | ⬜ |
| P2 | 活茧伤口档次 + 茧灵 effective flow | ⬜ |
| P3 | Insight 正向/负向字段按 gameplay loop 分批接线 | ⬜ |
| P4 | `jump_height_multiplier` server/client 权威闭环 | ⬜ |
| P5 | anti-orphan manifest/lint + 全栈 gate | ⬜ |

## 接入面

- **进料**：`server/src/combat/components.rs::DerivedAttrs`、`iron_cocoon_passive_system`、`server/src/cultivation/insight_apply.rs::InsightModifiers`、`server/src/alchemy/events.rs::StatusEffectKind`、`side_effect_apply.rs`、`pill.rs`、`apply_guangbo_ticao_bonuses`。
- **出料**：伤口结算、经脉 effective flow、回气/突破/颜色效率/过载/感知/恢复玩法循环、污染 tick、server-authoritative movement 与 client jump。
- **共享类型 / event**：复用 `DerivedAttrs`、`InsightModifiers`、`StatusEffects`、`DerivedAttrsSyncV1`、既有 qi ledger；禁止建立 parallel modifier component 或按玩法复制字段。
- **跨仓库契约**：P1-P3 以 server 为主；P4 若需 client jump，必须同 PR 扩 Rust schema/proto/generated/Java store 和 reset/consumer，禁止只加 wire 字段。
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
4. **Observe partial chain**：`observe_chance_bonus` 已被 `technique_observe.rs::evaluate_observe_attempt` 读取，但该 helper 无 production caller；必须接真实 observe 入口或明确退役，不能标已修。
5. **Jump**：`jump_height_multiplier` 已由广播体操 proficiency 生产；`move_speed_multiplier` 兄弟字段已生效，但 jump 无 server/client runtime consumer 或 wire 字段。

## 当前证据（origin/main @ c625d5a5）

- `server/src/alchemy/side_effect_apply.rs:18-31` 把 `contam_boost` 映射到 `ContaminationBoost`；`server/src/cultivation/contamination.rs` 的 tick query/公式不读 `StatusEffects`。
- `server/src/alchemy/pill.rs:632-642` 在 `JinZhongDan` negative duration 下 push `QiRegenBoost`；`server/src/cultivation/tick.rs::qi_regen_boost_multiplier` 已把它作为正向倍率消费。
- `server/src/combat/baomai_v4/iron_cocoon.rs:110-140` 写四个字段；伤口 resolve 与 `MeridianSystem::sum_rate` 不读。
- `server/src/cultivation/insight_apply.rs` 写上述 Insight 字段；除排除项外缺生产 gameplay consumer。
- `server/src/combat/body_conditioning.rs` 写 `jump_height_multiplier`；server movement、`DerivedAttrsSyncV1` 与 client store 均无 jump consumer。

## 设计门与验收

1. 每项必须以“同一 gameplay loop，写入前后可观察差分”验收；只测 struct 数值、helper 或 schema 不算完成。
2. 活茧伤口降级要冻结作用点、deterministic RNG 与 wound kind/grade 一致性；`scar_forged_flow_bonus` 只能进入 effective rate，不得永久改 `Meridian.flow_rate`。
3. Insight 按回气/突破、颜色/混元、过载/经脉、感知/反应四组拆 atomic PR；所有负向 cost 也必须真实可感知。
4. Jump 必须先拍板权威端；若跨端，server emit→proto/JSON bridge→client store→jump consumer→断线 reset 同 PR 闭环。
5. P5 manifest 对 `DerivedAttrs`、`InsightModifiers`、非展示 `StatusEffectKind` 逐字段记录至少一个 production consumer 或带 owner/reason 的 `planned_no_consumer`；新增只有 default/write/test 的字段时 gate 必须红。
6. 按触栈运行完整 server gate；涉及 schema/client 时重建 schema dist 并使用 Java 17 运行 client gate；补对应 bot/e2e 场景。

## 去重边界

- 本 plan 是 r6 P1、r7 P0、r8 modifier audit P2-P5 与 r8 findings #2/#3/#4/#5 的唯一 successor。
- 不重新实施 PR #1143；不接管距离衰减、Freeze 容器、JueBi marker、TSY cleanup 等独立 lifecycle/domain finding。
- 原 `plan-bughunt-r8-modifier-orphan-audit-v1` 作为验证与 P1 落地历史归档，不再作为 active implementation queue。
