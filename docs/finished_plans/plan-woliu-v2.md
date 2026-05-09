# Bong · plan-woliu-v2

涡流功法五招完整包：动画 / 特效 / 音效 / 伤害 / 真元消耗 / 反噬 / 客户端 UI 全流程。承接 `plan-woliu-v1` ✅ finished（基础持涡 toggle）—— v2 引入**搅拌器物理**（99/1 + 经脉 cap）+ **紊流场**（涡流流派专属 EnvField 边界）+ 五招完整规格（持涡 / 瞬涡 / 涡口 / 涡引 / 涡心），无境界 gate 只有威力门坎。

**世界观锚点**：`worldview.md §二 守恒律+压强法则+真元极易挥发` · `§三 化虚天道针对+×5 质变+维护成本` · `§四 距离衰减 0.03/格+流量公式+异体排斥+过载撕裂+距离衰减针对飞行真元` · `§五 涡流防御+P 物理推导(噬论 1/r²)+流派由组合涌现+末土后招原则+涡流非爆发型` · `§六 缜密色长期沉淀` · `§十一 灵物密度阈值天道注视` · `§十六 坍缩渊负灵域内涡流自动反噬` · `§K narration 沉默`

**library 锚点**：`cultivation-0002 烬灰子内观笔记 §噬论`（局部负压 1/r² 吸力的物理推导）

**前置依赖**（用户拍板"等 patch 后做"）：

- `plan-qi-physics-v1` P1 ship → API 冻结后接入
- `plan-qi-physics-patch-v1` P0/P2 → woliu.rs 现有硬编 const（`VORTEX_THEORETICAL_LIMIT_DELTA=0.8` / β=2.0 / K_drain≤0.5）全部走底盘
- `plan-skill-v1` ✅ + `plan-hotbar-modify-v1` ✅ → SkillRegistry / Casting / SkillBarBindings.cooldown_until_tick 直接复用
- `plan-multi-style-v1` ✅ → PracticeLog vector 接入缜密色累积
- `plan-HUD-v1` ✅ + `plan-input-binding-v1` ✅ → HUD 框架 + F 键触发

**反向被依赖**：

- `plan-style-balance-v1` 🆕 → 5 招的 W/β/K_drain 数值进矩阵
- `plan-color-v1` 🆕 → 缜密色加成 hook
- `plan-tribulation-v1` ⏳ → 化虚涡心绝壁劫触发条件
- `plan-tsy-zone-v1` ✅ → 坍缩渊负灵域内涡流自动反噬规则

---

## 接入面 Checklist

- **进料**：`cultivation::Cultivation { qi_current, qi_max, realm, meridian_flow_capacity, contamination }` / `qi_physics::ledger::QiTransfer` / `qi_physics::env::EnvField` / `qi_physics::collision::qi_collision` / `qi_physics::channeling::qi_channeling` / `qi_physics::constants::{QI_NEGATIVE_FIELD_K, QI_DRAIN_CLAMP}` / `SkillRegistry` / `SkillSet` / `Casting` / `PracticeLog` / `Realm`
- **出料**：5 招 `WoliuSkillId` enum 注册到 SkillRegistry / `VortexCastEvent`(扩展 v1 → 含 SkillId variant) / `VortexBackfireEvent`(扩展含分级) / `TurbulenceFieldSpawned` 🆕 / `TurbulenceFieldDecayed` 🆕 / `QiTransfer{from:zone,to:caster}`(1% 吸入) / `QiTransfer{from:caster_drain,to:surrounding_zones}`(99% 紊流甩出，按压强法则分发)
- **共享类型**：`StyleAttack` / `StyleDefense` trait（qi_physics::traits 已定义）/ 紊流场作为 `EnvField` 的局部子集（半径内 zone qi 状态被覆盖为紊流态 + decay 自然回归）
- **跨仓库契约**：
  - server: `combat::woliu_v2::*`（主实装）+ `schema::woliu_v2`（IPC payload）
  - agent: `tiandao::woliu_v2_runtime`（5 招 narration + 反噬叙事 + 紊流场叙事 + 化虚被动场天道注视）
  - client: 2 动画 + 1 粒子（VORTEX_SPIRAL）+ 2 音效 recipe + 4 HUD 组件
- **worldview 锚点**：见头部「世界观锚点」
- **qi_physics 锚点**：5 招全部走 qi_physics 算子，**禁止 plan 内自己写距离衰减/异体排斥/吸力公式**。本 plan 只声明 5 招的物理参数（注入率 / 吸取率 / 紊流半径 / 反噬阈值），底层公式归 qi_physics

---

## §0 设计轴心

- [x] **搅拌器物理（区别于黑洞）**：worldview §二 守恒律 + plan-qi-physics-v1 §0 第一公理。涡流不是吸光 zone qi，而是**搅拌**——99% 甩回环境，1% 吸入：
  ```
  total_drained = field_strength × radius² × duration
  rotational_swirl = total_drained × 0.99   → 紊流场（动态漩涡，非可吸收浓度）
  absorbed_raw = total_drained × absorption_rate  (1% 上限)

  absorption_rate = realm_factor × (1 - contam):
    醒灵 0.001 / 引气 0.002 / 凝脉 0.004 / 固元 0.006
    通灵 0.008 / 半步化虚 0.009 / 化虚 0.010

  meridian_cap = caster.meridian_flow_capacity × (1 - contam) × dt
  if absorbed_raw > meridian_cap:
    actual_absorbed = meridian_cap
    overflow = absorbed_raw - meridian_cap → 触发反噬阶梯
    contam += overflow / qi_max × 0.1
  else:
    actual_absorbed = absorbed_raw
    contam += absorbed_raw / qi_max × 0.01

  caster.qi_current += actual_absorbed
  ledger 写两条 QiTransfer:
    {from: zone, to: caster, amount: actual_absorbed}
    {from: zone, to: surrounding_zones, amount: rotational_swirl, distribution: pressure_law}
  ```

- [x] **紊流场（涡流流派专属边界）**：99% 甩出 ≠ 静态浓度溢出，是**动态高速漩涡**——其他玩家无法吸收（修炼靠静坐+稳定环境，紊流冲击经脉失败）。紊流场内效果：
  - 修炼吸收率 ×0（静坐被冲击 / 经脉无法稳定吸纳）
  - 战斗真元注入精度 ×0.5（招式失准）
  - shelflife 加速 ×3（worldview §二 末法分解加速）
  - 护体真气消耗 +20%（紊流持续撞击）
  - 自然耗散率 5%/s，化虚涡心紊流（30s 维持 × 50 格半径）→ 散尽 ~5min
  - **守恒律仍闭合**：紊流最终散回 zone qi 静态值，但过渡态对其他玩家不可用
  - **流派识别物理依据**：紊流场是涡流专属，其他流派创造不了。worldview §五:537 流派由组合涌现的尾迹

- [x] **反噬阶梯（worldview §四:354 过载撕裂物理）**：

  | overflow | 反噬级 | 经脉损伤 | 恢复 |
  |---|---|---|---|
  | < 10% qi_max | 微感 | contam +0.01/s | 静坐代谢 |
  | 10-30% | MICRO_TEAR | 手部 LU 经流量 ×0.85 / 5min | 凝脉散外敷 |
  | 30-60% | TORN | 手部 LU+LI ×0.5 / 30min，禁开涡流 | 凝脉散内服 |
  | ≥60% / 维持上限超 / 化虚绝壁劫触发 | SEVERED | 主经永久断 / 此手不能再开涡流 / 化虚跌境 | 上古"接经术" |

- [x] **无境界 gate，只有威力门坎**（worldview §五:537 流派由组合涌现）：任何境界都能 cast，三层物理自然惩罚：
  - qi_current 不足 → server `CastResult::Rejected{ QiInsufficient }`
  - 威力近零 → 触发但 Δ≈0 / K_drain≈0，HUD「掌心微凉」/「微风拂动」
  - 反噬过烈 → 低境硬开高招立即 SEVERED 一手

- [x] **化虚双场模型**（worldview §四 距离衰减针对飞行真元，不针对场延伸）：
  - **致命场**（≤10 格）：高强度负压压差驱动，1/r² 暴跌，worldview §四 0.03/格硬约束
  - **影响场**（10-300 格）：弱负压场延伸，触发"真元载体被吸引漂移"，触不致命但触发天道注视（worldview §十一 灵物密度阈值）
  - 化虚级涡口可远程定位创造低压点（不是飞行物，不受 0.03/格 衰减约束）
  - 化虚级涡心扩散到 zone 边界 = worldview §四:380「化虚老怪走过新人来不及看清袍角」物理依据

- [x] **化虚被动场可关**（worldview §四:506 末土后招原则物理化身）：化虚者可在 inspect 界面手动 toggle 被动场。关闭 = 不再搅拌 + 灵气分布回归正常 + 天道注视消退 / 代价 = 不再有持续 1% 吸入。**P0 决策：默认关**（§5 #1）

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ✅ 2026-05-09 | 决策门：5 招数值表锁定 + 搅拌器 99/1 比例 + 紊流场半径与耗散公式 + §5 七个开放问题决策 + qi_physics 接入面定稿 + ContainerKind/EnvField 紊流态扩展 design | 数值矩阵落 `combat::woliu_v2::skills`；99/1 与紊流常量落 `qi_physics::{constants,env,excretion}` |
| **P1** ✅ 2026-05-09 | server `combat::woliu_v2::*` 5 招 logic + 搅拌器物理 + 紊流场 EnvField 写入 + 反噬阶梯 + qi_physics 算子调用 + 化虚双场模型 + ≥100 单测 | `cargo test combat::woliu_v2` 127 passed；`grep -rcE '#\[test\]' server/src/combat/woliu_v2/` = `tests.rs:127` |
| **P2** ✅ 2026-05-09 | client 2 动画（vortex_palm_open / vortex_spiral_stance）+ VORTEX_SPIRAL 粒子 + 4 HUD 组件 + 5 招独立 hotbar icon | `render_animation.py` headless 两条动画通过；`./gradlew test build` 通过 |
| **P3** ✅ 2026-05-09 | 2 音效 recipe（vortex_low_hum / vortex_qi_siphon）+ agent 5 招 narration template + 反噬叙事（4 级）+ 紊流场叙事 + 化虚被动场天道注视 narration | `npm run build` + schema/tiandao tests 通过；server 音效/VFX bridge 发布差异化 recipe |
| **P4** ✅ 2026-05-09 | 坍缩渊负灵域内涡流自动反噬规则 + 化虚涡心绝壁劫 backfire surface + SkillXp/IPC telemetry surface；真实数值校准留给依赖 plan | 坍缩渊涡心强制 `SEVERED` 测试通过；`VoidHeartTribulation` cause / agent narration surface 已接 |

**P0 决策门**：完成前 §5 七个问题必须有答案，否则五招实装方向分裂。

---

## §2 五招完整规格

### ① 持涡 — 持续 toggle 防御伞

**用途**：拦截飞行真元载体（暗器骨刺/飞剑/法术弹射/毒针），载体真元归零后失效。化虚级整个山头的箭都在飞过半山腰时变普通木棍。

| 境界 | Δ | 致命半径 | 影响半径 | 紊流半径 | 维持上限 | qi/s | HUD |
|---|---|---|---|---|---|---|---|
| 醒灵 | 0.0 | 0.5 | 0.5 | 0 | 1s | 5 | 掌心微凉 |
| 引气 | 0.05 | 1.0 | 1.5 | 0.3 | 2s | 5 | 微风拂动 |
| 凝脉 | 0.25 | 1.5 | 3 | 1 | 5s | 6 | 涡流成形 ✅ |
| 固元 | 0.45 | 2.0 | 5 | 2 | 8s | 7 | 涡流深沉 |
| 通灵 | 0.65 | 3 | 15 | 8 | 12s | 9 | 涡流如渊 |
| 半步化虚 | 0.70 | 4 | 25 | 15 | 14s | 10 | 涡心初成 |
| 化虚 | 0.80 | 5 | 30-50 | 20-30 | 18s | 12 | 涡心如墟 |

**机制**：toggle 开关式。维持期间每 tick 调 `qi_physics::channeling::passive_drain(carrier_qi, env)` 拦截范围内飞行真元载体，载体 qi 归零 → 失效（已实装于 v1 的 vortex_intercept_tick）。搅拌器 99/1 应用：拦截到的飞行真元 99% 甩出加紊流场 + 1% 入 caster 池。

**冷却**：toggle 关闭后 0.5s 内不能再开（防止极快连按）。

### ② 瞬涡 — 200ms 弹反（melee 保命招）

**用途**：唯一能在 melee 中保命的涡流招（持涡只挡远程）。体修蓄力全力一击的瞬间瞬涡接 → 他打不进还输 30-40 qi。

| 境界 | qi | 窗口 | K_drain | 化虚专属 | HUD |
|---|---|---|---|---|---|
| 醒灵 | 8 | 200ms | 0.00 | — | 掌心一颤 / 失败 |
| 引气 | 8 | 200ms | 0.10 | — | 微反震 |
| 凝脉 | 8 | 200ms | 0.25 | — | 涡刃乍现 ✅ |
| 固元 | 8 | 200ms | 0.40 | — | 涡刃成势 |
| 通灵 | 8 | 200ms | 0.50（clamp 上限） | — | 涡刃如锋 |
| 半步化虚 | 8 | 220ms | 0.50 | — | 窗口稍宽 |
| 化虚 | 8 | 250ms | 0.50 | **链反**：弹回能量自动接力到周围 30 格内所有同源攻击 | 极限弹反 |

**机制**：window 内被命中触发 `qi_physics::collision::reverse_clamp(K_drain ≤ 0.5)` 反吸攻方真元。失败即被全额命中（worldview §四 异体排斥）。化虚链反走 `qi_physics::field::propagate_to_nearby(events, radius=30)`，需 qi_physics-patch-v1 P3 加新算子。

**冷却**：5s（worldview §四 截脉弹反窗口规范）。

**vs 截脉对比**：zhenmai 是肉接（自伤换免伤，皮下震爆），瞬涡是负压吃（不自伤但 5s 冷却 + 200ms 时机准）。两者互补不重叠。

### ③ 涡口 — 远程定位负压

**用途**：远程消耗、追猎逃兵。化虚老怪山顶一指，山脚敌人胸口凭空生涡口持续抽真元。

worldview §四 距离衰减针对**飞行真元**——涡口是「在敌方位置创造低压点」，不是飞行物，**不受 0.03/格 衰减约束**。这是涡流流派的物理特权（cultivation-0002 §噬论）。

| 境界 | 涡口距离 | 1 格吸取率 | 维持上限 | 启动 + qi/s | HUD |
|---|---|---|---|---|---|
| 醒灵 | — | — | — | — | Reject 真元不足（池 10 < 12） |
| 引气 | 1.5 格 | 1.0 qi/s | 2s | 12+3 | 涡口微张 |
| 凝脉 | 3 格 | 2.5 qi/s | 4s | 12+3 | 涡口张开 ✅ |
| 固元 | 5 格 | 4.0 qi/s | 6s | 12+3 | 涡口深啖 |
| 通灵 | 30 格 | 5.0 qi/s | 8s | 12+3 | 涡口如渊 |
| 半步化虚 | 50 格 | 5.5 qi/s | 9s | 12+3 | 涡口吞地 |
| 化虚 | 整 zone（100-300 格） | 6.0 qi/s | 10s | 12+3 | 涡口吞渊 |

**机制**：`qi_physics::channeling::active_drain(target_pos, dist, env)` 按 1/r² 计算吸取率。caster 自身持续 3/s 维持涡口形态。搅拌器 99/1：抽到的敌方 qi 99% 甩到敌方所在 zone 形成紊流场（敌方周围短暂修炼禁区）+ 1% 入 caster 池。

**冷却**：维持上限耗尽后 8s。

### ④ 涡引 — 拉拽有真元目标（AOE）

**用途**：断敌路（撤离仪式中的敌人拽 1 格 = 撤离失败）/ 远距离捞 loot / 拉道伥过来打。化虚老怪伸手整个山谷有真元的物体向他漂移 5-10 格。

**只拉有真元的实体/物品**（spirit_quality > 0 / qi_current > 0）：玩家 / NPC / 道伥执念 / 飞行暗器 / 掉落的封灵骨币 / 未变质灵草。**不拉**：纯凡物（土/木/石）/ 退活骨币 / 原版怪物 / spirit_quality=0 物品。**凡器被拿着 → 拉拿刀的人，刀跟着来**（拉的是人）。

**位移公式**：`displacement_blocks = caster.qi_current × N / target.qi_current`
- 化虚拉醒灵：8.0 × 10000 / 10 = 拉飞了
- 醒灵拉化虚：1.0 × 10 / 10000 = 0 位移（推不动）
- 物理依据：worldview §二 压强法则——敌方真元在体内是高压（经脉维持），创造极低压 → 真元想流过来 → 拽身体一起过

| 境界 | qi | 半径 | 拉力 N | 紊流路径半径 | 冷却 | HUD |
|---|---|---|---|---|---|---|
| 醒灵 | 25 | — | — | — | — | Reject 池 10 < 25 |
| 引气 | 25 | 3 格 | 1.0 | 0 | 30s | 涡引微息 |
| 凝脉 | 25 | 5 格 | 2.5 | 1 | 30s | 涡引成形 ✅ |
| 固元 | 25 | 7 格 | 4.0 | 2 | 25s | 涡引拽动 |
| 通灵 | 25 | 30 格 | 6.0 | 5 | 20s | 涡引夺命 |
| 半步化虚 | 25 | 50 格 | 7.0 | 8 | 18s | |
| 化虚 | 25 | 100+ 格 zone 量级 | 8.0 | 沿途 5s 紊流尾迹 | 15s | 涡引拉天 |

**机制**：一次性 25 qi 投入，扫描半径内 `qi_current > 0 || spirit_quality > 0` 实体/物品 → 按公式计算位移 → emit `EntityDisplacedByVortexPull` event（client 端做位移动画）。**搅拌器对涡引不直接适用**——caster 投入的 25 qi 主要用于物理位移做工，但化虚级路径上的紊流尾迹仍然按 99/1 产生。

### ⑤ 涡心 — 被动场 + 主动模式（半步化虚以上专属能力质变）

**用途**：1v 多围攻时开，敌人 5 格内全部被慢性抽干 + 紊流场封锁修炼/战斗。化虚级整个山谷变成不可修炼 + 不可正常战斗的死区。

| 境界 | 启动+qi/s | 致命半径 | 影响半径 | 紊流半径 | 维持上限 | 反噬上限 | HUD |
|---|---|---|---|---|---|---|---|
| 醒灵/引气 | — | — | — | — | — | — | Reject |
| 凝脉 | 50+8 | 1.5 | 1.5 | 1 | 2s | TORN 立即 | 涡心初成 不稳 |
| 固元 | 50+8 | 2 | 10 | 5 | 4s | TORN 风险 | 涡心成形 |
| 通灵 | 50+8 | 3 | 30 | 15 | 6s | MICRO_TEAR 风险 | 涡心如渊 |
| 半步化虚 | 50+8 | 4 | 50 | 25 | 8s | 微感 | 涡心吞地 |
| **化虚** | 50+8 主动 / 0+0 被动 | 5 | 被动 30 / 主动 100-300 zone 边界 | 被动 20 / 主动 50-100 | 主动可至反噬 / 被动 ∞ | 微感（被动）/ 绝壁劫触发 SEVERED+跌境（主动 ≥30s）| 涡心如墟 地为之倾 |

**化虚专属**：

- **被动模式**（默认状态待 §5 #1 拍板）：30 格被动场常驻，1% 吸入持续，紊流 20 格
- **主动模式**：致命半径 5 格，影响半径全 zone，可手动扩半径，无维持上限直到反噬
- **关闭**：紊流 ~5min 自然散尽，期间 zone 仍异常
- **天道注视**：主动 ≥30s 触发绝壁劫（worldview §三:78 + §十一）→ emit `TribulationAnnounce{ trigger: VoidWalkerHeartVortex }`
- **叙事意象**：化虚老怪走过荒野，身后灵草枯萎、空气扭曲、噬元鼠尸横遍野的紊流尾迹。worldview §四:380「化虚老怪从醒灵村中走过新人来不及看清袍角」物理依据 —— 因为整个村子瞬间进入紊流场。

**坍缩渊内特殊行为**（worldview §十六.五 灵龛失效）：坍缩渊内开涡心 → 立即 SEVERED + 跌境，因为坍缩渊本身就是负灵域，再叠加涡心负压 = 物理失控。强制反噬，不允许玩家在副本内开涡心赌命。

---

## §3 数据契约

```
server/src/combat/woliu_v2/
├── mod.rs              — Plugin 注册 + re-export + register_skills(&mut SkillRegistry)
├── skills.rs           — WoliuSkillId enum (Hold/Burst/Mouth/Pull/Heart)
│                        + 5 个 resolve_fn (cast_hold / cast_burst / cast_mouth /
│                                          cast_pull / cast_heart)
├── state.rs            — VortexState component 扩展 v1 (含 SkillId variant +
│                        active_skill_kind + heart_passive_enabled)
│                        + TurbulenceField component (center, radius, intensity,
│                                                     decay_rate, spawned_at_tick)
│                        + PassiveVortex marker (化虚专属)
├── tick.rs             — vortex_intercept_tick / vortex_maintain_tick(已有 v1) +
│                        turbulence_decay_tick 🆕 + heart_passive_tick 🆕
├── physics.rs          — 搅拌器算子 stir_99_1(total_drained, caster) →
│                                  (absorbed, swirl, overflow)
│                        + 反向 ledger 写入 (reverse_flow_to_surrounding_zones)
│                        + 化虚双场模型 (lethal_field vs influence_field)
├── backfire.rs         — 反噬阶梯 (微感/MICRO_TEAR/TORN/SEVERED) +
│                        坍缩渊强制反噬 + 化虚绝壁劫触发
└── events.rs           — VortexCastEvent (扩展 v1 含 SkillId) /
                          VortexBackfireEvent (扩展含 BackfireLevel) /
                          TurbulenceFieldSpawned 🆕 / TurbulenceFieldDecayed 🆕 /
                          EntityDisplacedByVortexPull 🆕 (涡引专用)

server/src/schema/woliu_v2.rs  — 5 招 IPC payload (WoliuSkillCastV1 /
                                  WoliuBackfireV1 / TurbulenceFieldV1 /
                                  WoliuPullDisplaceV1)

agent/packages/schema/src/woliu_v2.ts  — TypeBox 对齐
agent/packages/tiandao/src/woliu_v2_runtime.ts  — 5 招 narration template +
                                                  反噬叙事（4 级）+
                                                  紊流场叙事 +
                                                  化虚被动场天道注视 +
                                                  绝壁劫触发叙事

client/src/main/java/.../combat/woliu/v2/
├── WoliuV2AnimationPlayer.java       — 2 动画播放
├── VortexSpiralParticle.java         — 黑洞螺旋粒子（继承 BongRibbonParticle
│                                       + 向心加速度）
├── VortexChargeProgressHud.java      — 蓄力进度环（复用 baomai-v2 ChargeRing 模板）
├── VortexCooldownOverlay.java        — 冷却灰显（复用 SkillBar cooldown 灰显）
├── BackfireWarningHud.java           — 手腕红光警告 + 反噬等级文字
└── TurbulenceFieldVisualizeHud.java  — 紊流场可见性（PVP 信息暴露，
                                        半径范围扭曲滤镜）

client/src/main/resources/assets/bong/
├── player_animation/vortex_palm_open.json
├── player_animation/vortex_spiral_stance.json
└── audio_recipes/vortex_low_hum.json + vortex_qi_siphon.json
```

**SkillRegistry 注册**（mod.rs）：

```rust
pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register("woliu.hold",  cast_hold);
    registry.register("woliu.burst", cast_burst);
    registry.register("woliu.mouth", cast_mouth);
    registry.register("woliu.pull",  cast_pull);
    registry.register("woliu.heart", cast_heart);
}
```

**PracticeLog 接入**（每招触发后）：

```rust
emit SkillXpGain {
    char: caster,
    skill: SkillId::Woliu,
    amount: per_skill_amount(skill_kind),  // hold 1 / burst 2 / mouth 2 / pull 3 / heart 5
    source: XpGainSource::Action {
        plan: "woliu_v2",
        action: skill_kind.as_str(),
    }
}
```

PracticeLog 累积驱动 QiColor 演化（缜密色），由 plan-multi-style-v1 ✅ 已通的机制接管。

---

## §4 客户端新建资产

| 类别 | ID | 来源 | 优先级 | 备注 |
|---|---|---|---|---|
| 动画 | `bong:vortex_palm_open` | 新建 JSON | P2 | 掌心向外翻转 + 手腕下压 + 身体微缩，priority 300（姿态层），render_animation.py headless 验证 |
| 动画 | `bong:vortex_spiral_stance` | 新建 JSON | P2 | 双手环绕身体螺旋盘旋姿态，priority 400，涡心专用 |
| 粒子 | `VORTEX_SPIRAL` ParticleType + Player | 新建 | P2 | 黑色/深紫 ribbon 状螺旋体，朝掌心向心收缩，alpha 随距离衰减。继承 BongRibbonParticle + 对每个 segment 施加向心加速度 |
| 音效 | `vortex_low_hum` | recipe 新建 | P3 | layers: `[{ sound: "block.beacon.activate", pitch: 0.3, volume: 0.5 }, { sound: "entity.wither.hurt", pitch: 0.4, volume: 0.2, delay_ticks: 2 }]`（layered 低沉嗡鸣） |
| 音效 | `vortex_qi_siphon` | recipe 新建 | P3 | layers: `[{ sound: "entity.enderman.teleport", pitch: 1.8, volume: 0.3 }]`（高频掠过感） |
| HUD | `VortexChargeProgressHud` | 复用 baomai-v2 ChargeRing 模板 | P2 | 涡口/涡引/涡心启动期 |
| HUD | `VortexCooldownOverlay` | 复用 SkillBar cooldown 灰显 | P2 | hotbar 自动接管，无需额外 |
| HUD | `BackfireWarningHud` | 新建 | P2 | 手腕红光（contam 累积可视化）+ 反噬等级文字（微感/MICRO_TEAR/TORN/SEVERED） |
| HUD | `TurbulenceFieldVisualizeHud` | 新建 | P2 | 紊流场半径范围内空气扭曲滤镜 + 灵气云团粒子，PVP 时其他玩家可见 = 信息暴露 |

---

## §4.5 P1 测试矩阵（饱和化测试）

CLAUDE.md `## Testing — 饱和化测试`：每函数测 ① happy ② 边界 ③ 错误分支 ④ 状态转换。下限 **100 单测**：

| 模块 | 测试组 | 下限 |
|---|---|---|
| `cast_hold` | 7 境界威力 + 拦截命中/miss + 维持上限触发 + qi 不足 reject + 关闭后紊流耗散 | 12 |
| `cast_burst` | 200ms 窗口内/外 + K_drain 7 档 + 化虚链反 propagate + 攻方真元清零 clamp + 5s 冷却 | 13 |
| `cast_mouth` | 1/r² 衰减 + 7 距离档 + 维持上限 + 自身持续消耗 + 99/1 守恒断言 | 15 |
| `cast_pull` | 位移公式 + 有 qi/无 qi 目标过滤 + 凡器跟人位移 + 同境界互拉 + 化虚拉醒灵/反向 | 15 |
| `cast_heart` | 7 境界数值 + 主动/被动模式切换 + 化虚扩半径 + 关闭后 5min 紊流耗散 + 30s 触发绝壁劫 + 坍缩渊内强制反噬 | 25 |
| `stir_99_1` | absorption_rate 7 境界 + 经脉 cap 触发 overflow + 99% swirl 分布 + 守恒断言（input == output 总量） | 10 |
| `backfire_escalation` | 4 级渐进 + overflow 阈值 + 化虚绝壁劫触发 + SEVERED 永久不可逆 | 10 |

**P1 验收**：`grep -rcE '#\[test\]' server/src/combat/woliu_v2/` ≥ 100。守恒不变量：跨 24h tick fixture 模拟，`Σall ≈ WorldQiBudget.current_total`，紊流 decay 必须最终归回 zone 静态值。

---

## §5 开放问题 / 决策门（P0 启动前必须收口）

### #1 化虚被动场默认开还是关？

- **A**：默认开（化虚者一登录就在搅拌 zone，存在即注视）
- **B**：默认关（worldview §四:506 末土后招原则，玩家选择何时暴露）
- **C**：玩家上次选择记忆（per-character config）

**默认推荐 B** —— 末土后招原则：化虚者主动暴露身份是战略选择，不应被强制。但化虚者一旦关闭就放弃 1% 吸入，需要主动开来吃 zone qi 维持化虚池子。这造成"暴露 vs 续命"的张力，符合 worldview §三:80 化虚维护成本极高的设计。

### #2 紊流场是否影响 caster 自身？

- **A**：不影响（漩涡中心稳定，物理上合理）
- **B**：影响但减半（中心也有少量紊流）
- **C**：完全影响（caster 自身也修炼受阻）

**默认推荐 A** —— 漩涡中心是稳定的（流体力学事实），caster 站在中心不被自身紊流冲击。但 caster 的 1% 吸入产生的反噬另算（不是紊流场效果，是经脉 overflow）。

### #3 紊流场跟阵法（zhenfa）冲突时谁覆盖谁？

- **A**：涡流紊流覆盖（紊流物理强度高）
- **B**：阵法覆盖（地师预埋 > 涡流即时）
- **C**：叠加效果（紊流 ×0.5 + 阵法 ×0.5）

**P0 决策：本 plan 不做覆盖仲裁** —— 需 plan-zhenfa-v1 配合定义阵法的 EnvField 写入边界。本 plan 只声明涡流紊流态和局部 multiplier，阵法冲突仲裁落点归 zhenfa-v1 vN+1。

### #4 99/1 比例是 v1 锁死还是留 telemetry 校准？

- **A**：v1 锁死 99/1（worldview 物理直觉）
- **B**：留 config 暴露 + telemetry 校准（符合 plan-style-balance-v1 §P.7 校准方法）

**默认推荐 B** —— 99/1 是设计直觉，但实际 PVP 数据可能显示 95/5 或 99.5/0.5 更平衡。暴露为 `qi_physics::constants::VORTEX_ABSORPTION_RATIO_BASE` config，P4 telemetry 校准。

### #5 涡引拉拽干尸（worldview §十六 死在坍缩渊的修士干尸 → 道伥）算不算？

- **A**：算（干尸有 spirit_quality 残留 → 拉得动）
- **B**：不算（干尸真元已被坍缩渊抽干，spirit_quality=0）
- **C**：拉得动但触发道伥激活（worldview §七 + §十六.六）

**默认推荐 C** —— 干尸残留低 spirit_quality（0.05-0.1），物理上能被涡引拉一点点（位移 ≤ 1 格）。但拉拽过程中触发激活道伥本能 → emit `TaoChangAwakened` event。这是有趣的负面副作用（你想拉宝 → 拉醒了道伥）。

### #6 涡心被动场常驻是否消耗 qi_current？

- **A**：消耗（即使被动也要付维持成本，0.5 qi/s）
- **B**：不消耗（化虚级被动是免费的，自然形态）
- **C**：从环境吸入（化虚 absorption_rate 0.01 持续 1% 入池）

**默认推荐 C** —— 被动场不主动扣自身 qi，但持续 1% 入池（搅拌器物理一致）。这跟主动模式区别在于：被动是"被环境喂"，主动是"主动搅"。化虚者只要站在有 zone qi 的地方就能慢慢续命，但代价是天道一直在看。

### #7 紊流场内别的玩家施法精度 ×0.5 是否包括防御招？

- **A**：包括（紊流不区分攻防）
- **B**：不包括（防御招仅修炼/攻击受影响）
- **C**：仅持续型防御（持涡 / 涡心）受影响，瞬时型（瞬涡 / 截脉）不受

**默认推荐 A** —— 紊流是物理事实，不区分攻防意图。所有招式精度 ×0.5。这是 worldview §五:467 涡流"算计型博弈"的物理化身——你创造紊流场迫使对手降效。

---

## §6 进度日志

- **2026-05-05** 骨架立项，承接 plan-woliu-v1 ✅ finished（PR #113 commit 06f6a726 + P0/P1 commit 121dbf70）。
  - 设计轴心：搅拌器物理（99/1）+ 紊流场（动态漩涡非浓度溢出）+ 反噬阶梯（4 级）+ 无境界 gate 只有威力门坎 + 化虚被动场可关 + 化虚双场模型（致命场+影响场）
  - 五招完整规格 7 档威力表锁定（持涡/瞬涡/涡口/涡引/涡心）
  - 化虚质变定调：远程定位涡口 + zone 量级涡引 + 山谷级紊流死区涡心，对应 worldview §四:380 物理依据
  - 紊流场作为涡流流派专属边界：其他流派创造不了，是流派识别物理依据（worldview §五:537 流派由组合涌现的尾迹）
  - worldview 锚点对齐：§二/§三:78/§四:380/§五:442+537/§十一/§十六.五/§K
  - qi_physics 锚点：等 patch P0/P2 完成后接入；本 plan **不动既有代码**（v1 模块），全部新建在 `combat::woliu_v2` 子模块内
  - SkillRegistry / PracticeLog / HUD / 音效 / 动画 全部底盘复用，无新建框架
  - 待补：与 plan-style-balance-v1 数值矩阵对齐 / plan-color-v1 缜密色加成 hook / plan-tribulation-v1 化虚绝壁劫触发条件 / plan-zhenfa-v1 紊流场 vs 阵法冲突仲裁
- **2026-05-09**：升 active（`git mv docs/plans-skeleton/plan-woliu-v2.md → docs/plan-woliu-v2.md`）。触发条件：
  - **plan-qi-physics-patch-v1 ✅ finished**（PR #162，2026-05-08）—— P0/P2 全部底盘就位，搅拌器 99/1 + 紊流场 EnvField + ρ 矩阵 + 1/r² drain + 反噬阶梯 SEVERED 写入路径全部可接
  - **plan-woliu-v1 ✅** + **plan-skill-v1 ✅** + **plan-multi-style-v1 ✅** —— 持涡 v1 + 熟练度生长 + 缜密色 PracticeLog 全前置 ✅
  - 用户 2026-05-09 拍板**音效/特效/HUD 区分硬约束 + 招式独立 icon**：5 招（持涡 / 瞬涡 / 涡口 / 涡引 / 涡心）cast 必须各自携带差异化 animation + particle + SFX + HUD 反馈（漩涡半径 / 紊流密度 / 化虚双场致命+影响视觉分层 视觉物理化）+ **hotbar/SkillBar 槽位 PNG icon 每招独立**（走 `client/.../hud/SkillSlotRenderer.java`，化虚涡心 icon 用 zone-level 紊流死区视觉锚定 worldview §四:380），紊流场 EnvField 边界视觉是另一通道（不挤占 hotbar icon），P4/P5 验收必须含视觉/听觉差异化回归 + icon 显示回归
  - 2026-05-09 consume-plan 收口：五招 server 物理、agent narration、client HUD/动画/粒子/icon、音效 recipe 与跨仓库 schema 已落地；数值 telemetry / 缜密色加成 / 阵法冲突仲裁保留给各自依赖 plan。

---

## Finish Evidence

- **落地清单**：
  - P0 物理与数值：`server/src/qi_physics/constants.rs`、`server/src/qi_physics/env.rs`、`server/src/qi_physics/excretion.rs`、`server/src/combat/woliu_v2/skills.rs`
  - P1 server 五招：`server/src/combat/woliu_v2/{events,state,physics,backfire,tick,skills,tests}.rs`、`server/src/combat/mod.rs`、`server/src/cultivation/skill_registry.rs`、`server/src/cultivation/known_techniques.rs`
  - P2 client 视听与 HUD：`client/src/main/java/com/bong/client/hud/{WoliuV2HudPlanner,VortexChargeProgressHud,VortexCooldownOverlay,BackfireWarningHud,TurbulenceFieldVisualizeHud}.java`、`client/src/main/java/com/bong/client/visual/particle/{VortexSpiralParticle,VortexSpiralPlayer}.java`、`client/src/main/resources/assets/bong/player_animation/{vortex_palm_open,vortex_spiral_stance}.json`、`client/src/main/resources/assets/bong/textures/gui/skill/woliu_*.png`
  - P3 契约/叙事/音效：`server/src/schema/woliu_v2.rs`、`server/src/network/woliu_event_bridge.rs`、`server/assets/audio/recipes/{vortex_low_hum,vortex_qi_siphon}.json`、`agent/packages/schema/src/woliu_v2.ts`、`agent/packages/tiandao/src/woliu_v2_runtime.ts`
  - P4 特殊规则 surface：`server/src/combat/woliu_v2/backfire.rs`、`server/src/combat/woliu_v2/tests.rs`、`agent/packages/tiandao/tests/woliu_v2_runtime.test.ts`
- **关键 commit**：
  - `b2ae9e93e` · 2026-05-09 · `plan-woliu-v2: 接入 server 五招物理`
  - `86d177a8e` · 2026-05-09 · `plan-woliu-v2: 接入 agent 契约与叙事`
  - `9763c2e1c` · 2026-05-09 · `plan-woliu-v2: 接入 client 视听与 HUD`
  - `e7792511b` · 2026-05-09 · `fix(plan-woliu-v2): 接通紊流运行时与 HUD 状态`（PR review 修复：紊流投射、vortex_state HUD 字段、肺经依赖、涡心 30s 绝壁劫延迟触发、真实 zone 账本分发）
  - `e7326071a` · 2026-05-09 · `fix(plan-woliu-v2): 收紧涡流状态生命周期`（PR review 修复：搅拌污染持久化、VortexV2State/PassiveVortex 生命周期清理、HUD tick 换算、99/1 守恒 pin 测试）
  - `fe1e2bce0` · 2026-05-09 · `fix(plan-woliu-v2): 落实涡引位移结算`（PR review 修复：`woliu.pull` 结算直接改写目标 `Position`，事件保留为观测/动画信号）
  - `b30793066` · 2026-05-09 · `fix(plan-woliu-v2): 对齐 technique 数量`（rebase 修复：主线暗器技法与涡流技法合并后 `KnownTechniques` 固定数组长度更新为 22）
  - `3082efd11` · 2026-05-09 · `fix(plan-woliu-v2): 对齐 yidao rebase 注册格式`（rebase 修复：主线医道注册与涡流注册共存后保持 server 注册表格式化）
  - `92f960607` · 2026-05-09 · `fix(plan-woliu-v2): 收紧 pull 与紊流边界`（PR review 修复：`woliu.pull` 缺目标拒绝、只在真实位移后发事件、已耗尽紊流不再投影、涡流粒子共享中心点、冷却秒数极端值钳制）
- **测试结果**：
  - `cd server && cargo fmt --check` → passed
  - `cd server && CARGO_BUILD_JOBS=1 cargo clippy --all-targets -j 1 -- -D warnings` → passed
  - `cd server && CARGO_BUILD_JOBS=1 cargo test -j 1` → 3420 passed
  - `cd server && CARGO_BUILD_JOBS=1 cargo test -j 1 combat::woliu_v2` → 130 passed
  - `cd server && CARGO_BUILD_JOBS=1 cargo test -j 1 network::woliu_state_emit::tests` → 1 passed
  - `cd server && CARGO_BUILD_JOBS=1 cargo test -j 1 cultivation::tick::tests::turbulence_exposure_blocks_qi_regen` → 1 passed
  - `cd server && grep -rcE '#\[test\]' src/combat/woliu_v2` → `tests.rs:130`
  - `cd server && CARGO_BUILD_JOBS=1 cargo test -j 1 qi_physics` → 91 passed
  - `cd agent && npm run build && npm test -w @bong/schema && npm test -w @bong/tiandao` → schema 342 passed，tiandao 318 passed
  - `cd agent && npm run generate:check -w @bong/schema` → generated schema artifacts fresh (322 files)
  - `cd client && JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test build` → BUILD SUCCESSFUL
  - `python3 client/tools/render_animation.py client/src/main/resources/assets/bong/player_animation/vortex_palm_open.json -o /tmp/woliu_v2_anim_vortex_palm_open` → wrote grid
  - `python3 client/tools/render_animation.py client/src/main/resources/assets/bong/player_animation/vortex_spiral_stance.json -o /tmp/woliu_v2_anim_vortex_spiral_stance` → wrote grid
- **跨仓库核验**：
  - server：`WoliuSkillId::{Hold,Burst,Mouth,Pull,Heart}` / `WOLIU_*_SKILL_ID` 注册到 `SkillRegistry`；`WoliuSkillCastV1` / `WoliuBackfireV1` / `TurbulenceFieldV1` 通过 `CH_WOLIU_V2_{CAST,BACKFIRE,TURBULENCE}` 推送；VFX/audio trigger 使用各招 `animation_id` / `particle_id` / `sound_recipe_id`
  - agent：`agent/packages/schema/src/woliu_v2.ts` TypeBox 合约与 server serde payload 对齐；`WoliuV2NarrationRuntime` 订阅 cast/backfire/turbulence 三通道并输出五招/反噬/紊流叙事
  - client：`QuickBarHudPlanner` 支持 `iconTexture()`；`WoliuV2HudPlanner` 聚合 4 个 HUD planner；`VORTEX_SPIRAL` 粒子注册；2 条 player_animation JSON + 6 张 skill/particle PNG 资源入包
- **遗留 / 后续**：
  - `plan-style-balance-v1`：真实 5 招 × 7 流派 PVP telemetry 数值校准；本 plan 只暴露 SkillXp/IPC/visual telemetry surface
  - `plan-color-v1`：PracticeLog → QiColor 的缜密色数值加成；本 plan 只发 `SkillXpGain`
  - `plan-zhenfa-v1 vN+1`：紊流场与阵法 EnvField 覆盖/叠加仲裁
  - `plan-tribulation-v1`：`VoidHeartTribulation` 的正式 `TribulationAnnounce` 链；本 plan 已发 backfire cause 与 agent 叙事 surface
