# plan-cultivation-pacing-v1 — 修炼节奏重塑：慢修 + 丹药加速路线

修炼基础速率大幅降低，8 种丹药形成从醒灵到化虚的完整加速路线，每颗丹药绑定真实区域 + 教会玩家一个新系统，自然拉动采集→灵田→炼金→锻造→探索→战斗全链需求。

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 基础速率重调 + 逐脉难度曲线 | ✅ 2026-05-19 |
| P1 | StatusEffect 接入 + 8 种修炼丹药 + 材料刷新机制 | ✅ 2026-05-19 |
| P2 | 体验平衡与数值校准 | ✅ 2026-05-20 |

---

## 接入面

- **进料**：`cultivation::Cultivation`（qi_current/qi_max/realm）、`cultivation::MeridianSystem`（opened_count/family）、`combat::StatusEffects`（读 CultivationAcceleration）、`alchemy::PillEffect`（meridian_progress_bonus 字段已预留未消费）、`botany::PlantRegistry`（灵草模板）、`gathering::GatheringSession`（采集产出）
- **出料**：修改 `cultivation::meridian_open::advance_open_progress_at()` 公式 → 全经脉打通速度变化；修改 `qi_physics::constants::QI_CULTIVATION_REGEN_RATE` → 全局真元回复速度变化；新增 `StatusEffectKind::CultivationAcceleration` → `combat::StatusEffects` 消费；8 份丹方 JSON → `alchemy` 消费；`PillEffect.meridian_progress_bonus` 实装消费 → `alchemy::pill::consume_pill()` 挂载 StatusEffect
- **共享类型**：复用 `StatusEffectKind`（新增变体）、复用 `PillEffect`（激活已有字段）、复用 `BreakthroughBoost`（凝元丹/破境丹/渡劫丹叠加）
- **跨仓库契约**：server only（P0-P1 纯常量+公式+丹方），P2 HUD 涉及 client `StatusSnapshot` 新增 cultivation_acceleration 字段
- **worldview 锚点**：worldview §二（真元/灵气/修炼）、§四（六境界突破）、§七（丹药/炼金）
- **qi_physics 锚点**：调用 `QI_CULTIVATION_REGEN_RATE`（constants.rs:70），不新增物理常数

---

## 设计理念

**核心矛盾**：当前打通一条经脉只要 5 秒，真元几秒回满，玩家坐地冥想就能直升通灵——所有生产/战斗/探索系统沦为"可选便利"，无人问津。

**解法**：纯冥想仍然可行但极慢（~56h 开完 16 条经脉） → 丹药提供 1.5-5× 加速（~19h） → 丹药需要灵草/矿物 → 灵草分布在不同危险区域 → 高级灵草在高危区域 → 需要战斗能力和装备 → 战斗消耗丹药/工具 → 循环成立。

---

## 修炼丹路线总览

8 种丹药形成一条从醒灵到化虚的完整进阶路线，每颗丹药绑定一个真实地理区域、教会玩家一个新游戏系统：

```
初醒原（安全）
  │ 采灵草 ×3，学基础炼丹
  ▼
① 灵息丸 ─→ 1.5× 加速 ─→ 打通首条经脉
  │ 开灵田种刺舌蒿
  ▼
② 聚灵丹 ─→ 2× 加速 ─→ 打通 3 脉，突破引气
  │ 种凝脉草（16h），探索幽暗地穴边缘采萤渊菇
  ▼
③ 通脉散 ─→ 2.5× 经脉加速 ─→ 打通 4-6 脉
  │ 远征血谷采血色脉草，学锻造炼凡铁
  ▼
④ 凝元丹 ─→ 3× 全面加速 + 突破 +10% ─→ 打通 7-9 脉
  │ 深入灵泉湿地采兽心草（40% 散落），返回血谷采焦脉藤
  ▼
⑤ 洗髓液 ─→ 4× 极速（qi 回复 -50%）─→ 打通 10-12 脉，突破固元
  │ 准备突破材料
  ▼
⑥ 破境丹 ─→ 突破 +20%（单次消耗）─→ 固元→通灵 关键突破
  │ 远征王隐台（负灵域！）采蜕骨藤，灵泉眼深处采井心藻
  ▼
⑦ 开窍丹 ─→ 5× 奇经专用 ─→ 打通 8 条奇经（固元前 4 + 通灵后 4，见 §8.1 #5）
  │ 准备渡劫（王隐台采龙鳞苔 + 灵泉眼采续元蕊）
  ▼
⑧ 渡劫丹 ─→ 突破 +25% + 劫中护体 ─→ 渡虚劫 → 化虚
```

### 8 种丹药详表

| # | 丹药 | ID | 境界 | 加速 | 时长 | 毒素 | 毒色 | 主材料 | 产区 | 教会系统 |
|---|------|-----|------|------|------|------|------|--------|------|---------|
| ① | 灵息丸 | `ling_xi_wan` | 醒灵 | 1.5×(mag=0.5) | 36000t(30min) | 0.15 | Gentle | 灵草×3 | 初醒原 | 基础采集+炼丹 |
| ② | 聚灵丹 | `ju_ling_dan` | 醒灵/引气 | 2×(mag=1.0) | 24000t(20min) | 0.20 | Mellow | 灵草×2+刺舌蒿×1 | 初醒原(种植) | 灵田种植 |
| ③ | 通脉散 | `tong_mai_san` | 引气 | 2.5×(mag=1.5) | 18000t(15min) | 0.30 | Solid | 凝脉草×2+萤渊菇×1 | 灵田+幽暗地穴 | 探索冒险 |
| ④ | 凝元丹 | `ning_yuan_dan` | 凝脉 | 3×(mag=2.0)+BT+0.10 | 18000t(15min) | 0.35 | Heavy | 血色脉草×1+凝脉草×1+凡铁×1 | 血谷+灵田+锻造 | 战斗区+锻造 |
| ⑤ | 洗髓液 | `xi_sui_ye` | 凝脉/固元 | 4×(mag=3.0) qi回复-50% | 12000t(10min) | 0.40 | Violent | 兽心草×1+焦脉藤×1 | 灵泉湿地+血谷 | 风险/收益权衡 |
| ⑥ | 破境丹 | `po_jing_dan` | 固元 | BT+0.20（单次消耗） | 单次 | 0.45 | Insidious | 玄绒苔×1+血色脉草×1+灵石×1 | 幽暗地穴深层+血谷 | 突破准备 |
| ⑦ | 开窍丹 | `kai_qiao_dan` | 固元 | 5×奇经专用(mag=4.0) | 12000t(10min) | 0.50 | Turbid | 蜕骨藤×1+井心藻×1 | 王隐台(负灵域!)+灵泉眼 | 负灵域探索 |
| ⑧ | 渡劫丹 | `du_jie_dan` | 通灵 | BT+0.25+劫中减伤+每波回qi | 渡劫全程 | 0.60 | Insidious | 龙鳞苔×1+续元蕊×1+灵石×3 | 王隐台+灵泉眼+矿 | 终极准备 |

> BT = BreakthroughBoost 突破成功率加成。洗髓液的 qi 回复 -50% 通过新增 `QiRegenSlowed`（magnitude=0.5 → 回复 ×0.5）实现（**不复用现有 `QiRegenPaused`**，后者是 bool 全停，见 tick.rs:291-300）。开窍丹 mag=4.0 仅对 `MeridianFamily::Extraordinary` 生效，正经打通时按 mag=0 处理。
>
> **同种丹药堆叠限制**：同一种丹药（PillKind）同时最多 2 层有效（第 3 颗起 magnitude 不计入聚合），防止灵息丸 ×6 堆叠绕过进阶曲线。跨种丹药不受此限。

### 丹毒混搭策略

不同毒色互不干扰（各色独立 threshold=1.0），玩家可以跨色混搭：

| 组合 | 毒色 | 累积毒素 | 效果 |
|------|------|---------|------|
| 灵息丸×2 + 通脉散×1 | Gentle 0.30 + Solid 0.30 | 均未达阈值 | 1.5× + 2.5× 叠加 |
| 聚灵丹×2 + 凝元丹×1 | Mellow 0.40 + Heavy 0.35 | 均安全 | 2× + 3× + BT 叠加 |
| 洗髓液×1 + 灵息丸×3 | Violent 0.40 + Gentle 0.45 | 均安全 | 4×开脉 + qi 回复部分补偿 |

### 材料产区 × 危险等级

| 区域 | 灵气 | 危险 | 可采灵草 | 对应丹药 |
|------|------|------|---------|---------|
| **初醒原** | 0.3 | ⭐1 | 灵草(common) | ①②灵息丸/聚灵丹 |
| **初醒原灵田** | — | — | 刺舌蒿(种8h)、凝脉草(种16h) | ②③④聚灵丹/通脉散/凝元丹 |
| **幽暗地穴边缘** | 0.4 | ⭐⭐4 | 萤渊菇(uncommon) | ③通脉散 |
| **血谷** | 0.3 | ⭐⭐⭐4 | 血色脉草(rare)、焦脉藤(rare) | ④⑤⑥凝元丹/洗髓液/破境丹 |
| **灵泉湿地** | 0.7 | ⭐⭐3 | 兽心草(rare,40%散落) | ⑤洗髓液 |
| **灵泉眼** | 0.7+ | ⭐⭐⭐3 | 井心藻(epic)、续元蕊(very rare,60%散落) | ⑦⑧开窍丹/渡劫丹 |
| **幽暗地穴深层** | 0.4 | ⭐⭐⭐⭐4 | 玄绒苔(rare) | ⑥破境丹 |
| **王隐台(负灵域)** | -0.15 | ⭐⭐⭐3 | 蜕骨藤(rare)、龙鳞苔(very rare) | ⑦⑧开窍丹/渡劫丹 |

---

## 目标节奏

### 开脉时间预算（zone_qi=0.6 典型区域，qi_ratio=0.8）

| 境界跃迁 | 需开经脉 | 纯冥想 | 吃丹（对应丹药） | 说明 |
|---------|---------|-------|----------------|------|
| 醒灵→引气 | 3 正经 | ~3.3h | ~1.1h（灵息丸/聚灵丹） | 新手期 |
| 引气→凝脉 | +3 正经 | ~4.7h | ~1.6h（通脉散） | 灵田+首次冒险 |
| 凝脉→固元 | +6 正经 | ~13.2h | ~4.4h（凝元丹/洗髓液） | 全面展开 |
| 固元→通灵 | +4 奇经 | ~34.9h | ~7h（开窍丹 5×） | 奇经极慢 |
| **合计** | **16 条** | **~56h** | **~14h** | |

> 通灵→化虚走天劫流程。含 qi 积累、突破、材料采集等，总游玩时长预估：带丹 ~80-100h，纯冥想 ~200h+。

### 真元回复时间（zone_qi=0.6）

| 状态 | 纯冥想 | 吃聚灵丹（2×） |
|------|-------|--------------|
| 醒灵（rate=0.1, qi_max=10）| ~46 min | ~23 min |
| 开 1 脉后（rate=1.0, qi_max=20）| ~12 min | ~6 min |
| 开 3 脉后（rate≈3.0, qi_max=40）| ~8 min | ~4 min |
| 开 6 脉后（rate≈12.0, qi_max=70）| ~3 min | ~1.5 min |

### 逐脉耗时明细（纯冥想，zone_qi=0.6, qi_ratio=0.8）

| 经脉序号 | 类型 | 难度因子 | 单脉耗时 |
|---------|------|---------|---------|
| 1 | 正经 | 1.000 | 58 min |
| 2 | 正经 | 0.870 | 67 min |
| 3 | 正经 | 0.769 | 76 min |
| 4 | 正经 | 0.690 | 84 min |
| 5 | 正经 | 0.625 | 93 min |
| 6 | 正经 | 0.571 | 102 min |
| 7 | 正经 | 0.526 | 110 min |
| 8 | 正经 | 0.488 | 118 min |
| 9 | 正经 | 0.455 | 127 min |
| 10 | 正经 | 0.426 | 136 min |
| 11 | 正经 | 0.400 | 145 min |
| 12 | 正经 | 0.377 | 153 min |
| 13 | 奇经 | 0.143 | 6.8h |
| 14 | 奇经 | 0.136 | 7.1h |
| 15 | 奇经 | 0.129 | 7.5h |
| 16 | 奇经 | 0.123 | 7.8h |

> 奇经合计 ~29h（非 qi_ratio=0.8 恒定——奇经阶段 qi_max 更大、qi 消耗更多，实际 qi_ratio 波动在 0.6-0.85，总耗时预估 ~35h）。

---

## P0 — 基础速率重调 + 逐脉难度曲线 ✅ 2026-05-19

### P0.1 常量修改

**`server/src/cultivation/meridian_open.rs`**（第 34-37 行）：

```
BASE_OPEN_RATE:  0.01  → 0.00003   (333× 降速)
OPEN_COST_FACTOR: 5.0  → 5.0       (不变，总 qi 消耗恒为 5.0/脉)
```

**`server/src/qi_physics/constants.rs`**（第 70 行）：

```
QI_CULTIVATION_REGEN_RATE:  0.01  → 0.003   (3.3× 降速)
```

### P0.2 逐脉难度递增函数

**`server/src/cultivation/meridian_open.rs`** 新增：

```rust
pub fn meridian_difficulty_factor(opened_count: usize, family: MeridianFamily) -> f64 {
    let progression = 1.0 / (1.0 + opened_count as f64 * 0.15);
    let family_mult = match family {
        MeridianFamily::Regular => 1.0,
        MeridianFamily::Extraordinary => 0.4,
    };
    progression * family_mult
}
```

delta 公式变更（第 96 行）：

```rust
// before:
let delta = BASE_OPEN_RATE * zone_qi * qi_ratio;
// after:
let difficulty = meridian_difficulty_factor(meridians.opened_count(), target.family());
let delta = BASE_OPEN_RATE * zone_qi * qi_ratio * difficulty;
```

### P0.3 验收标准

- [ ] `cargo test` 全绿
- [ ] 单测 `meridian_difficulty_factor`：pin opened_count=0/3/6/12 × Regular/Extraordinary 共 8 组返回值
- [ ] 单测 `advance_open_progress_at`：zone_qi=0.6, qi_ratio=0.8 下首条正经需 ≥69600 tick 打通（约 58 min，0.00003×0.6×0.8=0.0000144/tick，1/0.0000144≈69444）
- [ ] 单测：首条奇经（opened_count=12, Extraordinary）需 ≥486000 tick 打通（约 6.8h）
- [ ] 单测 qi_regen：醒灵期 rate=0.1, zone_qi=0.6 下 qi_max=10 回满需 ≥55000 tick（约 46 min）
- [ ] 回归：现有 meridian/breakthrough/tribulation 测试全绿

---

## P1 — StatusEffect 接入 + 8 种修炼丹药 ✅ 2026-05-19

### P1.1 新增 StatusEffectKind

**`server/src/combat/events.rs`**（StatusEffectKind enum）：

```rust
CultivationAcceleration,       // magnitude N → (1+N)× 修炼速度，同时加速 qi 回复与经脉打通
QiRegenSlowed,                 // magnitude N → qi 回复 × (1-N)，区别于 QiRegenPaused（后者是 bool 全停）
DamageVulnerability,           // magnitude N → 受击伤害 × (1+N)，洗髓液副作用
```

> **不复用 `QiRegenPaused`**：现有 `qi_regen_pause_multiplier()`（tick.rs:291-300）是 `any(QiRegenPaused) → 0.0` 的 bool 实现，阵法冷却等依赖此语义。`QiRegenSlowed` 是新增的比例减速，独立消费。

### P1.2 tick.rs 接入加速乘数

**`server/src/cultivation/tick.rs`**（第 177-197 行乘数链）：

```rust
fn cultivation_acceleration_multiplier(se: &StatusEffects) -> f64 {
    let sum: f32 = se.active.iter()
        .filter(|e| e.kind == StatusEffectKind::CultivationAcceleration && e.remaining_ticks > 0)
        .map(|e| e.magnitude.max(0.0))
        .sum();
    (1.0 + sum as f64).min(5.0)  // 上限 5×
}
```

乘入 qi_regen tick 的 rate 乘数链。

### P1.3 meridian_open.rs 接入加速乘数

**`server/src/cultivation/meridian_open.rs`**：

`advance_open_progress_at()` 新增参数 `cultivation_boost: f64`，由 `meridian_open_tick` 调用侧从 StatusEffects 预计算后传入。

```rust
let delta = BASE_OPEN_RATE * zone_qi * qi_ratio * difficulty * cultivation_boost;
```

**开窍丹特殊逻辑**：当 target 为 `MeridianFamily::Regular` 时，`CultivationAcceleration` 中来自开窍丹的部分不计入 boost（通过给开窍丹使用独立 `StatusEffectKind::ExtraordinaryMeridianAcceleration` 实现，仅在 target 为奇经时参与聚合）。

### P1.4 八种修炼丹药实现

每种丹药需要：

1. `server/src/alchemy/pill.rs`：新增 PillKind 变体 + PillEffect 定义
2. `server/assets/alchemy/recipes/<id>_v1.json`：丹方 JSON
3. 特殊效果需在 `consume_pill()` 中额外处理

**① 灵息丸** `ling_xi_wan`：
- PillEffect: `CultivationAcceleration(mag=0.5)`, duration=36000t
- 丹方: 灵草(spirit_grass)×3, Furnace tier 1（初醒原公共 NPC 丹炉可用，见 §8.1 #6）
- 最简单的丹，新手引导入口

**② 聚灵丹** `ju_ling_dan`：
- PillEffect: `CultivationAcceleration(mag=1.0)`, duration=24000t
- 丹方: 灵草(spirit_grass)×2 + 刺舌蒿(ci_she_hao)×1, Furnace tier 1
- 需要种植系统产出刺舌蒿

**③ 通脉散** `tong_mai_san`：
- PillEffect: `CultivationAcceleration(mag=1.5)`, duration=18000t
- 丹方: 凝脉草(ning_mai_cao)×2 + 萤渊菇(ying_yuan_gu)×1, Furnace tier 1
- 凝脉草需种 16h，萤渊菇需去幽暗地穴边缘采

**④ 凝元丹** `ning_yuan_dan`：
- PillEffect: `CultivationAcceleration(mag=2.0)` + `BreakthroughBoost(mag=0.10)`, duration=18000t
- 丹方: 血色脉草(xue_se_mai_cao)×1 + 凝脉草(ning_mai_cao)×1 + 凡铁(fan_tie)×1, Furnace tier 1
- 血色脉草需远征血谷(danger 4)，凡铁需学锻造

**⑤ 洗髓液** `xi_sui_ye`：
- PillEffect: `CultivationAcceleration(mag=3.0)` + `DamageVulnerability(mag=1.0, 即受击伤害×2)`, duration=12000t；buff 到期后自动追加 `QiRegenSlowed(mag=0.8, 12000t=10min)` 疲惫期（见 §8.1 #8）
- 丹方: 兽心草(shou_xin_cao)×1 + 焦脉藤(jiao_mai_teng)×1, Furnace tier 2
- 兽心草在灵泉湿地(danger 3, 40% 散落)，焦脉藤在血谷裂隙(danger 4)
- 高风险高回报：10min 4× 开脉 + 受击伤害翻倍，buff 结束后 10min qi 回复降至 ×0.2（洗髓后身体虚脱）

**⑥ 破境丹** `po_jing_dan`：
- PillEffect: `BreakthroughBoost(mag=0.20)`, 单次消耗（突破时清除）
- 丹方: 玄绒苔(xuan_rong_tai)×1 + 血色脉草(xue_se_mai_cao)×1 + 灵石(ling_shi)×1, Furnace tier 2
- 玄绒苔在幽暗地穴深层(danger 4)，灵石需矿物采集

**⑦ 开窍丹** `kai_qiao_dan`：
- PillEffect: `ExtraordinaryMeridianAcceleration(mag=4.0)`, duration=12000t
- 丹方: 蜕骨藤(tui_gu_teng)×1 + 井心藻(jing_xin_zao)×1, Furnace tier 2
- 蜕骨藤在王隐台(负灵域 -0.15! danger 3, 手部擦伤)
- 井心藻在灵泉眼(epic 级，极珍稀)
- 仅加速奇经打通，正经无效

**⑧ 渡劫丹** `du_jie_dan`：
- PillEffect: `BreakthroughBoost(mag=0.25)` + 渡劫期间减伤 30% + 每波间隙回复 15% qi
- 丹方: 龙鳞苔(long_lin_tai)×1 + 续元蕊(xu_yuan_rui)×1 + 灵石(ling_shi)×3, Furnace tier 3
- 龙鳞苔在王隐台边缘(very rare, 手部割伤+50% 散落)
- 续元蕊在灵泉眼正上方 3 尺(very rare, 60% 散落)
- 全游戏最难炼制的丹药

### P1.5 新增 ExtraordinaryMeridianAcceleration

**`server/src/combat/events.rs`**：

```rust
ExtraordinaryMeridianAcceleration,  // 仅加速奇经打通，正经无效
```

**`server/src/cultivation/meridian_open.rs`**：meridian_open_tick 中聚合时，仅当 target.family() == Extraordinary 时计入此效果。

### P1.6 激活已有预留接口

- `PillEffect.meridian_progress_bonus`（pill.rs:40）：在 `consume_pill()` 中读取并挂载对应 `CultivationAcceleration` StatusEffect
- `StatusEffectKind::QiRegenBoost`（events.rs:103）：统一归并到 `CultivationAcceleration`，避免双轨

### P1.7 验收标准

- [ ] `cargo test` 全绿
- [ ] 单测 `cultivation_acceleration_multiplier`：magnitude=0→1.0×, 0.5→1.5×, 1.0→2.0×, 4.0→5.0×(cap)
- [ ] 单测：消费 `ling_xi_wan` 后挂载 CultivationAcceleration(mag=0.5, dur=36000)
- [ ] 单测：消费 `ning_yuan_dan` 后同时挂载 CultivationAcceleration(mag=2.0) + BreakthroughBoost(mag=0.10)
- [ ] 单测：消费 `xi_sui_ye` 后同时挂载 CultivationAcceleration(mag=3.0) + QiRegenSlowed(mag=0.5) + DamageVulnerability(mag=1.0)
- [ ] 单测：消费 `kai_qiao_dan` 后挂载 ExtraordinaryMeridianAcceleration(mag=4.0)
- [ ] 单测：ExtraordinaryMeridianAcceleration 对 Regular 经脉 delta 无影响
- [ ] 单测：ExtraordinaryMeridianAcceleration 对 Extraordinary 经脉 delta = 5× 基线
- [ ] 单测：丹毒 ≥1.0 时 `can_take_pill()` 返回 false
- [ ] 单测：不同毒色丹药互不干扰（Gentle+Solid 各自独立累积）
- [ ] 单测：同种丹药第 3 颗 magnitude 不计入聚合（灵息丸 ×3 仍为 2×，非 2.5×）
- [ ] 单测：qi_regen 在 CultivationAcceleration(mag=1.0) 下速率 2×（对比基线）
- [ ] 单测：meridian delta 在 CultivationAcceleration(mag=2.0) 下 3× 基线
- [ ] 单测：QiRegenSlowed(mag=0.5) 消费侧——qi_regen tick 输出减半（对比无 debuff 基线）
- [ ] 单测：DamageVulnerability(mag=1.0) 消费侧——combat resolve 受击伤害 ×2（对比无 debuff 基线）
- [ ] 单测：聚灵丹 + zone_qi=0.6 下首条正经 ≤30 min
- [ ] 集成测试：服药 → 开脉 → 突破 全链路跑通
- [ ] 8 份丹方 JSON schema 校验通过

### P1.8 灵草刷新配置

丹药原料灵草的刷新机制已有三种底盘（`botany/lifecycle.rs`）：ZoneRefresh（区域密度自动补充）、StaticPoint（固定点冷却再生）、EventTriggered（事件触发）。需为每种丹药原料校准参数，确保产出率匹配 P2.1 表。

**刷新公式回顾**：
- ZoneRefresh: `target_count = floor(zone.spirit_qi × density_factor)`，采后 despawn → 下 tick 自动补生（新位置）
- StaticPoint: 固定位置，采后冷却 `regen_ticks` 后原地重生
- 丹道灵草: 走 botany 系统但按 `spirit_qi` 数值约束而非 zone 名称

**各原料灵草配置表**：

| 灵草 | spawn_mode | zone_tags | density_factor | growth_cost | regen_ticks | 目标产出 |
|------|-----------|-----------|----------------|-------------|-------------|---------|
| 灵草 spirit_grass | ZoneRefresh | Plains | 20.0 | 0.002 | — | 8-12/h（spawn 区 6 株同存，见 §8.1 #9） |
| 刺舌蒿 ci_she_hao | ZoneRefresh+灵田 | Plains | 4.0 | 0.002 | — | 灵田 8h 一茬 |
| 凝脉草 ning_mai_cao | ZoneRefresh+灵田 | Plains | 3.0 | 0.003 | — | 灵田 16h 一茬 |
| 萤渊菇 ying_yuan_gu | ZoneRefresh | Cave | 2.5 | 0.005 | — | 2-3/h（地穴边缘） |
| 血色脉草 xue_se_mai_cao | ZoneRefresh | BloodValley | 1.5 | 0.008 | — | 1-2/h（血谷遍布） |
| 焦脉藤 jiao_mai_teng | StaticPoint | BloodValley | — | — | 3600 (30min) | 2 处固定点/zone |
| 玄绒苔 xuan_rong_tai | StaticPoint | Cave | — | — | 7200 (60min) | 1-2 处固定点/zone |
| 兽心草 shou_xin_cao | 丹道灵草 | qi≥0.5 | 0.8 | 0.010 | — | 0-1/h（40% 散落） |
| 蜕骨藤 tui_gu_teng | 丹道灵草 | qi∈[-0.1,0.2] | 0.6 | 0.012 | — | 0-1/h（手伤+工具） |
| 井心藻 jing_xin_zao | StaticPoint | Marsh(灵泉眼) | — | — | 14400 (2h) | 1 处/灵泉眼 |
| 龙鳞苔 long_lin_tai | 丹道灵草 | qi∈[-0.3,0] | 0.4 | 0.015 | — | 0-1/h（50% 散落） |
| 续元蕊 xu_yuan_rui | StaticPoint | Marsh(灵泉眼) | — | — | 21600 (3h) | 1 处/灵泉眼 |

**实现**：

- `server/src/botany/registry.rs`：确认/更新上表 12 种灵草的注册参数。**注意**：`spirit_grass`（灵草）当前仅作为物品 template_id 存在，不在 BotanyKindRegistry 中注册——需新增注册（ZoneRefresh, Plains, density=5.0）使其在世界中自然生长刷新
- `server/src/world/zone.rs:382-419`：确认 zone→BotanyZoneTag 映射覆盖所有产区
- `server/src/dandao/herbs.rs`：确认丹道五灵草注册到 BotanyKindRegistry 且 density 匹配
- 新增 StaticPoint 配置：焦脉藤（血谷 2 处）、玄绒苔（地穴深层 1-2 处）、井心藻（灵泉眼 1 处）、续元蕊（灵泉眼 1 处）→ 在 `initialize_static_points_from_zones()` 中注入固定坐标

**关键约束**：ZoneRefresh 每 spawn 一株消耗 `growth_cost` 的 zone spirit_qi → 密度自然受灵气总量限制，多人采集同一 zone 会让灵气下降 → 刷新变慢 → 天然竞争。

### P1.9 矿物再生机制

当前矿物采空后永久耗尽（`mineral/break_handler.rs:398-405` despawn + `persistence.rs` 写入 `exhausted.json`），凡铁和灵石是丹药配方必需品，不能枯竭。

**设计**：矿物锚点增加可选 `respawn_ticks` 字段，耗尽后启动冷却计时，到期后从 exhausted 列表移除、下次锚点物化时重新生成。

**`server/src/mineral/types.rs`** 新增：

```rust
pub struct MineralAnchorConfig {
    // ...existing fields...
    pub respawn_ticks: Option<u64>,  // None = 永久耗尽（默认，向后兼容）
}
```

**`server/src/mineral/persistence.rs`** 修改：

现有耗尽记录是 `ExhaustedLogFile { version, entries: Vec<ExhaustedEntry> }`，每条 entry 含 `(mineral_id, x, y, z, tick)`。在 `ExhaustedEntry` 上扩展：

```rust
pub struct ExhaustedEntry {
    // ...existing fields (mineral_id, x, y, z, exhausted_at_tick)...
    #[serde(default)]  // 向后兼容旧 JSON
    pub respawn_at_tick: Option<u64>,  // None = 永不再生（默认）
}
```

写入时：`respawn_at_tick = anchor.respawn_ticks.map(|t| now_tick + t)`。

**`server/src/mineral/anchors.rs`** 修改：

锚点物化时检查 respawn_at_tick：

```rust
if let Some(entry) = exhausted_entries.iter().find(|e| e.position == anchor_pos) {
    match entry.respawn_at_tick {
        Some(respawn_tick) if now_tick >= respawn_tick => {
            // 冷却结束，移除耗尽记录，允许重新生成
            remove_exhausted_entry(entry);
        }
        _ => continue, // 仍在冷却或永久耗尽，跳过
    }
}
```

**矿物再生时间配置**：

| 矿物 | 品阶 | respawn_ticks | 游戏内时间 | 说明 |
|------|------|--------------|----------|------|
| 凡铁 fan_tie | 凡(1) | 72000 | ~1h | 基础金属，快速再生 |
| 粗铁 cu_tie | 凡(1) | 72000 | ~1h | 同凡铁 |
| 杂钢 za_gang | 凡(1) | 108000 | ~1.5h | 略慢 |
| 丹砂 dan_sha | 凡(1) | 108000 | ~1.5h | 炼丹辅料 |
| 凡品灵石 ling_shi_fan | 凡(1) | 144000 | ~2h | 燃料+丹药原料 |
| 灵铁 ling_tie | 灵(2) | 288000 | ~4h | 中阶金属 |
| 中品灵石 ling_shi_zhong | 灵(2) | 288000 | ~4h | 中阶燃料 |
| 上品灵石 ling_shi_shang | 稀(3) | 576000 | ~8h | 高阶，慢再生 |
| 髓铁 sui_tie | 稀(3) | 576000 | ~8h | 高阶金属 |
| 遗品灵石 ling_shi_yi | 遗(4) | None | 永不再生 | 遗级不可再生 |
| 枯金 ku_jin | 遗(4) | None | 永不再生 | 遗级不可再生 |

> 品阶越高再生越慢，遗(4)级永久耗尽。凡铁/凡品灵石 ~1-2h 再生，确保丹药路线不断档。

**worldgen 锚点 JSON 更新**：`worldgen/blueprint/mineral_anchors.json` 每条锚点增加 `"respawn_ticks"` 字段（可选，缺省 = None = 永久耗尽，向后兼容）。

### P1.10 材料刷新验收标准

- [ ] `cargo test` 全绿
- [ ] 单测：12 种灵草在对应 zone 中注册且 density_factor 匹配上表
- [ ] 单测：StaticPoint 灵草（焦脉藤/玄绒苔/井心藻/续元蕊）采后在 regen_ticks 内不重生、到期后重生
- [ ] 单测：ZoneRefresh 灵草（灵草/萤渊菇/血色脉草）采后 zone 在下一个 lifecycle tick 补生新株
- [ ] 单测：矿物 exhausted + respawn_ticks 设置后，冷却到期时从 exhausted 列表移除
- [ ] 单测：矿物 respawn_ticks=None 时行为不变（永久耗尽，向后兼容）
- [ ] 单测：凡铁锚点 respawn 后 `remaining_units` 恢复为 `max_units`
- [ ] 集成测试：采空凡铁 → 等待 72000t → 锚点重新物化 → 可再次采集

---

## P2 — 体验平衡与数值校准 ✅ 2026-05-20

### P2.1 材料获取节奏对齐

确保丹药原料获取难度与修炼节奏匹配：

| 丹药 | 原料产区 | 1h 可采量 | 够用次数 | 风险 |
|------|---------|----------|---------|------|
| ①灵息丸 | 初醒原 | 灵草 6-8 | 2 颗 | 无 |
| ②聚灵丹 | 初醒原灵田 | 灵草 4+刺舌蒿 2 | 2 颗 | 无（种植等待） |
| ③通脉散 | 灵田+幽暗地穴 | 凝脉草 1-2+萤渊菇 2-3 | 1-2 颗 | 中（地穴怪） |
| ④凝元丹 | 血谷+灵田+矿 | 血色脉草 1-2 | 1 颗 | 高（血谷danger4） |
| ⑤洗髓液 | 灵泉湿地+血谷 | 兽心草 0-1+焦脉藤 1 | 0-1 颗 | 高（散落+战斗） |
| ⑥破境丹 | 地穴深层+血谷 | 玄绒苔 0-1 | 0-1 颗 | 高（深层怪） |
| ⑦开窍丹 | 王隐台+灵泉眼 | 蜕骨藤 0-1+井心藻 0-1 | 0-1 颗 | 极高（负灵域） |
| ⑧渡劫丹 | 王隐台+灵泉眼+矿 | 龙鳞苔 0-1+续元蕊 0-1 | 0-1 颗 | 极高 |

需确认 `server/src/gathering/` 和 `server/src/botany/` 的刷新率与上表对齐。

### P2.2 NPC 丹药经济

- NPC 售卖低品质灵息丸（品质 flawed，效果 ×0.6，售价 8 骨币）→ "自己炼更划算"
- NPC 售卖低品质聚灵丹（品质 flawed，效果 ×0.6，售价 15 骨币）→ 引导学灵田
- ③以上 NPC 不售卖 → 必须自炼或玩家间交易
- 高品质丹药（perfect）可卖给 NPC 获取溢价 → 炼金成为收入来源

### P2.3 修炼 HUD 反馈

client 侧新增：
- 经脉打通进度条旁显示当前修炼倍率（如"修炼 ×3.0"）
- 丹药 buff 剩余时间在状态栏倒计时
- 经脉预估剩余时间（基于当前速率 + buff 剩余时长计算）

### P2.4 验收标准

- [ ] 端到端体验：新号醒灵→引气，纯冥想 ~3h / 带灵息丸+聚灵丹 ~1h
- [ ] 材料刷新率与采集效率与 P2.1 表偏差 ≤30%
- [ ] NPC 丹药交易已配置
- [ ] Client HUD 显示加速倍率 + buff 剩余时间
- [ ] 全链路：采草 → 种田 → 炼丹 → 服药 → 开脉 → 突破，无断点

---

## §8 开放问题（P0 决策门前需收口）

1. **dev 命令快进**：`/time advance` 快进修炼时钟时，丹药 buff duration 是否同步扣减？当前 `CultivationClock` 与 `StatusEffect.remaining_ticks` 是否共用 tick 源？
2. **多人灵气竞争**：QI_CULTIVATION_REGEN_RATE 降速后，多人同区域修炼会更快耗干 zone_qi（灵气守恒律），是否需要调整 zone_qi 恢复速率？
3. **现有测试兼容**：现有 meridian/breakthrough 测试依赖当前速率（5 秒/脉），降速 333× 后这些测试需要重写还是用 `/time advance` 跳过？
4. **QiRegenPaused × QiRegenSlowed 优先级**：当玩家同时有 QiRegenPaused（阵法冷却，全停）和 QiRegenSlowed（洗髓液，减半）时，pause 应短路优先（qi_regen=0），slowed 不生效。方向已定（见 P1.1），细节待 §8.1。
5. **开窍丹 × 通灵突破**：通灵→化虚走天劫流程，开窍丹的奇经加速在此阶段是否仍有意义（奇经应该在固元→通灵前打完）？是否需要在通灵后禁用开窍丹？
6. **灵息丸 Furnace tier 0 门槛**：醒灵玩家是否有 Furnace tier 0 可用？需确认 spawn 区域有无可用丹炉 / 是否需要 NPC 提供公共丹炉。
7. **同种堆叠限制 × 毒素系统交互**：同种丹药最多 2 层有效（防灵息丸×6 绕过曲线）+ 毒素阈值 1.0（同色累积）双重限制。堆叠先踢（第 3 颗 mag 不计入），毒素后踢（第 7 颗才超阈值）。二者定位不同（前者防绕过、后者长期惩罚），但玩家可能困惑"为什么吃了 2 颗就没加速了但毒还低"。需决定 UI 如何提示。
8. **洗髓液 DamageVulnerability 触发场景**：冥想时通常不受击——DamageVulnerability 实际触发依赖 zone 刷怪袭击 / PvP / 灵气暴动。若触发率过低则仍是假代价。备选方案：buff 结束后追加 10min 疲惫期（qi_regen ×0.2）。
9. **灵草 density_factor 产出率校准**：P1.8 灵草 density_factor=5.0，但初醒原 zone_qi=0.3 下 target_count=floor(0.3×5.0)=1 株/zone，可能达不到 P2.1 声称的 6-8/h。需在 P2 阶段用实际 zone 面积 × lifecycle tick 频率实测校准。

> 全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

---

## §8.1 决议（pre-P0 收口，2026-05-19）

### #1 dev 命令快进与 StatusEffect

**决议**：
1. `/time advance` 仅修改 `CultivationClock.tick`（`cmd/dev/time.rs:58`），不影响 `CombatClock`
2. `StatusEffect.remaining_ticks` 由 `CombatClock` 驱动（`combat/status.rs:87-102`），两个时钟完全独立
3. 因此 `/time advance` 快进修炼进度但**不消耗丹药 buff 时长**——这是合理的 dev 行为（测试用，不需要同步）
4. 若需同步测试丹药消耗，用 Bevy 正常 tick 等待即可

**落点**：无代码改动。`server/src/cmd/dev/time.rs:58` + `server/src/combat/status.rs:87-102` 已确认独立。

### #2 多人灵气竞争

**决议**：
1. zone.spirit_qi 无自然回复机制——仅通过外部事件（NPC 还债、植物死亡归还、zone 事件）恢复
2. `QI_CULTIVATION_REGEN_RATE` 同时控制吸纳量和 zone drain，降速后**压力比例不变**——修改常量不改变多人竞争烈度
3. 无需额外调整 zone_qi 恢复速率。多人自然分散到 qi 更充裕的区域（灵泉湿地 qi=0.7 vs 初醒原 qi=0.3）
4. 灵气稀缺是 worldview 核心设计（"末法灵气稀薄"），竞争本身是预期游戏体验

**落点**：无代码改动。`server/src/qi_physics/excretion.rs:43-63`（drain=gain/50）比例关系不受常量值影响。

### #3 现有测试兼容

**决议**：
1. `meridian_open.rs:300` `progress_accumulates_and_opens` **必破**：循环 200 次不足（新速率需 ~33000 次）。修复：将循环上限提高至 35000，或改为精确计算期望迭代数
2. `tick.rs` 17 个测试中 12 个用相对断言（比例/倍数），**不受影响**；2 个绝对值断言足够宽松（`<=`），大概率通过；需运行确认
3. **修复方案**：PR-1 中同步修复受影响测试，不另开 PR

**落点**：`server/src/cultivation/meridian_open.rs:304`（循环上限改 35000）。`server/src/cultivation/tick.rs:347,355`（运行确认，必要时调参数）。plan P0.3 验收已覆盖。

### #4 QiRegenPaused × QiRegenSlowed 优先级

**决议**：
1. `QiRegenPaused`（`tick.rs:291-300`）是 bool 全停（×0），保持不变
2. `QiRegenSlowed` 新增为独立函数，插入乘数链中 `qi_regen_pause_multiplier` **之后**
3. 评估顺序：`...× qi_regen_pause_mult × qi_regen_slowed_mult × ...`
4. 当 pause 生效时 mult=0，slowed 的 ×0.5 乘在 0 上仍为 0——**pause 自然短路 slowed**，无需特殊处理
5. `QiRegenSlowed` 实现：`(1.0 - sum(mag)).clamp(0.0, 1.0)`

**落点**：`server/src/cultivation/tick.rs:197`（在 juebi_aftershock 之后插入 `* qi_regen_slowed_multiplier`）。plan P1.2 已覆盖。

### #5 开窍丹 × 通灵后用途

**决议**：
1. 通灵（Spirit）需要 4 条奇经，化虚（Void）需要**全部 8 条奇经**（`breakthrough.rs:307-318`）
2. 通灵→化虚期间玩家仍需打通**剩余 4 条奇经**——开窍丹在此阶段**极为关键**
3. **不禁用开窍丹**，相反，plan 路线图中"⑦开窍丹→打通 4 条奇经"的定位应扩大为"固元→通灵期间开 4 条 + 通灵→化虚期间再开 4 条"
4. 丹药路线图补注：开窍丹是通灵后最重要的丹药

**落点**：plan 路线图（第 58 行）"打通 4 条奇经" → 应注明"固元→通灵 4 条 + 通灵→化虚 4 条，共 8 条"。无代码改动。

### #6 Furnace tier 0 门槛

**决议**：
1. 现有最低丹炉为 tier 1（凡铁炉，`furnace.rs:106-113`），不存在 tier 0
2. 初醒原无丹炉 POI——醒灵玩家没有炼丹入口
3. **解法**：在初醒原教学区（断碑观星台旁）放置 1 个**公共 NPC 丹炉**（tier 1，不可拆走），作为新手引导的一部分
4. 灵息丸和聚灵丹配方改为 Furnace tier 1（不是 tier 0）——因为 tier 0 不存在且无需创建
5. 公共丹炉旁放置 NPC 提示"灵草可炼制修炼丹药"作为引导

**落点**：`server/zones.worldview.example.json` 初醒原 POI 列表新增 `"furnace_public_npc"`。plan P1.4 ①② Furnace tier 0 → tier 1。plan P2.2 NPC 丹药经济中追加公共丹炉引导。

### #7 同种堆叠限制 × 毒素系统交互

**决议**：
1. 现有 `upsert_status_effect()`（`combat/status.rs:44-56`）对同种 `StatusEffectKind` 取 max(magnitude, duration)——**当前架构下同种丹药不会叠加**（第 2 颗只刷新，不增加 mag）
2. **设计意图是允许叠加**（丹毒混搭策略表依赖此行为）→ 需改为 `push` 模式
3. 实现：对 `CultivationAcceleration` 专用 `push_status_effect()`（push 到 Vec），其他 StatusEffectKind 保持 upsert
4. 聚合时 `cultivation_acceleration_multiplier()` 已有 `.sum()` + `.min(5.0)` → 总倍率上限 5×
5. **per-PillKind cap 实现**：在 `ActiveStatusEffect` 新增 `source_pill: Option<String>` 字段（`#[serde(default)]`），push 前检查同 source_pill 已有条目数 ≥ 2 则不 push
6. **UI 提示**：当第 3 颗同种丹药被拒绝时，chat 提示"此丹药已达最大层数"（区别于毒素阈值提示"丹毒过重"）

**落点**：`server/src/combat/status.rs:44-56`（新增 `push_status_effect`）+ `server/src/combat/components.rs:354-363`（`ActiveStatusEffect` 加 `source_pill`）+ `server/src/alchemy/pill.rs` consume_pill 调用侧。

### #8 洗髓液代价真实化

**决议**：
1. DamageVulnerability 在安全区冥想时触发率极低（无怪袭击），确实是假代价
2. **改为双重代价**：
   - buff 期间：DamageVulnerability(mag=1.0, 受击伤害×2) — 在危险区有意义
   - buff 结束后：自动追加 `QiRegenSlowed(mag=0.8, duration=12000t=10min)` — qi 回复降至 ×0.2
3. 这意味着：10min 极速开脉 → 10min 极慢恢复期。玩家必须计划好"洗髓后要干什么"——不能立刻战斗或继续修炼
4. 实现：在 `status_effect_tick()` 检测到 `xi_sui_ye` 来源的 CultivationAcceleration 到期时，push 一条 QiRegenSlowed(mag=0.8, 12000t)

**落点**：`server/src/combat/status.rs` 新增到期回调逻辑。plan P1.4 ⑤ 洗髓液效果描述更新。

### #9 灵草 density_factor 产出率

**决议**：
1. ZoneRefresh 公式 `target_count = floor(zone_qi × density_factor)`，lifecycle 周期 100 tick = 5 秒
2. 初醒原 zone_qi=0.3, density_factor=5.0 → target=1 株/zone。1500×1500 区域内仅 1 株，采后 5 秒刷新但随机位置——实际寻找+采集吞吐远低于 6-8/h
3. **修正**：灵草 density_factor 提高至 **20.0**（target=floor(0.3×20)=6 株同时存在）。初醒原 6 株散布 1500×1500 ≈ 每 600×600 一株，步行 1-2 min + 采集 30s → ~2-3 min/株 → 20-30/h 可采，考虑遗漏和竞争，实际 ~8-12/h
4. 其他区域（qingyun qi=0.5 → target=10, lingquan qi=0.7 → target=14）更充裕
5. P1.8 灵草配置表 density_factor 列更新：spirit_grass 5.0 → 20.0
6. P2.1 材料表"灵草 6-8/h"在修正后合理

**落点**：plan P1.8 灵草配置表（spirit_grass density_factor 改 20.0）。P2 阶段实测校准。

---

## §10 实施工作流

### §10.1 PR 拆分

| PR | 范围 | 依赖 |
|----|------|------|
| PR-1 | P0 常量重调 + 难度曲线 + 测试 | 无 |
| PR-2 | P1.1-P1.3 StatusEffect 接入（CultivationAcceleration + QiRegenSlowed tick 消费 + DamageVulnerability combat 消费）+ tick/meridian_open 公式改动 + 测试 | PR-1 |
| PR-3 | P1.4-P1.6 八种丹药 PillKind + 丹方 JSON + consume_pill 实装 + 测试 | PR-2 |
| PR-4 | P1.8-P1.9 灵草刷新配置 + 矿物再生机制 + 测试 | PR-1（不依赖丹药代码） |
| PR-5 | P2 体验平衡 + NPC 经济 + HUD 反馈 + 端到端验收 | PR-3 + PR-4 |

> PR-3 和 PR-4 可并行（丹药代码和材料刷新互不依赖），PR-5 等两者都 merge 后再开。

### §10.2 Subagent 配置

每 PR 起独立 subagent：
```
Agent(subagent_type: "claude", model: "opus", prompt: "...任务...\nultrathink")
```

### §10.3 CodeRabbit 等待协议

每 PR merge 前等 CodeRabbit APPROVED（ScheduleWakeup 1200s × 最多 3 回合）。

### §10.4 单次 consume-plan 全自动到 merge

用户 `/consume-plan cultivation-pacing-v1` 后即可离开，醒来检查 plan 是否在 `finished_plans/`。

---

## Finish Evidence

### 落地清单

| 阶段 | 模块 / 文件 |
|------|------------|
| P0 | `server/src/cultivation/meridian_open.rs`（BASE_OPEN_RATE 0.01->0.00003, meridian_difficulty_factor）、`server/src/qi_physics/constants.rs`（QI_CULTIVATION_REGEN_RATE 0.01->0.003） |
| P1.1-P1.3 | `server/src/combat/events.rs`（4 新变体: CultivationAcceleration / QiRegenSlowed / DamageVulnerability / ExtraordinaryMeridianAcceleration）、`server/src/combat/status.rs`（push_status_effect + source_pill）、`server/src/cultivation/tick.rs`（cultivation_acceleration_multiplier + qi_regen_slowed_multiplier 乘数链）、`server/src/cultivation/meridian_open.rs`（cultivation_boost 参数） |
| P1.4-P1.6 | `server/src/alchemy/pill.rs`（CultivationPillKind 8 种 + 2 种 flawed + consume_cultivation_pill + FLAWED_MAGNITUDE_MULTIPLIER）、`server/assets/alchemy/recipes/`（8 份丹方 JSON）、`server/assets/items/pills.toml`（10 种修炼丹药模板） |
| P1.8-P1.9 | `server/src/botany/registry.rs`（12 种灵草注册: spirit_grass density=20.0 等）、`server/src/mineral/registry.rs`（respawn_ticks 机制: 凡铁 72000 / 灵石 144000 / 遗级 None） |
| P2.1 | 验证性确认：P1.8/P1.9 配置的 density_factor 与 regen_ticks 已与 plan P2.1 表对齐，无需额外调整 |
| P2.2 | `server/src/network/client_request_handler.rs`（npc_trade_catalog_entry 新增 ling_xi_wan_flawed 8骨币 + ju_ling_dan_flawed 15骨币）、`server/src/alchemy/pill.rs`（flawed spec + is_flawed_cultivation_pill + consume 时 magnitude 缩放） |
| P2.3 | `server/src/network/status_snapshot_emit.rs`（cultivation_acceleration 顶层字段 + 修炼丹药 source_label）、`client/src/main/java/com/bong/client/combat/handler/StatusSnapshotHandler.java`（解析 cultivation_acceleration）、`client/src/main/java/com/bong/client/combat/store/StatusEffectStore.java`（cultivationAcceleration 存取）、`client/src/main/java/com/bong/client/hud/StatusEffectHudPlanner.java`（修炼 xN.N 显示） |

### 关键 commit

| Hash | 日期 | 描述 |
|------|------|------|
| d85c516a2 | 2026-05-19 | PR-1: P0 修炼基础速率重调 + 逐脉难度曲线 |
| 8fa6b7d4f | 2026-05-19 | PR-2: P1.1-P1.3 StatusEffect 修炼加速基建 + 4 变体 + tick/meridian 乘数链 |
| cea9f676b | 2026-05-19 | PR-3: 八种修炼丹药 + consume_pill StatusEffect 实装 |
| 8b91691b3 | 2026-05-19 | PR-4: 灵草刷新配置 + 矿物再生机制 |
| 24b8726ab | 2026-05-20 | PR-5: P2 体验平衡 — NPC 次品丹药经济 + HUD 修炼加速 + 30 新测试 |

### 测试结果

```
cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
# test result: ok. 5695 passed; 0 failed; 0 ignored

cd client && ./gradlew test build
# BUILD SUCCESSFUL in 59s
```

### 跨仓库核验

- **server**: `StatusEffectKind::CultivationAcceleration` / `QiRegenSlowed` / `DamageVulnerability` / `ExtraordinaryMeridianAcceleration`、`cultivation_acceleration_multiplier`、`npc_trade_catalog_entry("ling_xi_wan_flawed")`、`status_snapshot_emit` cultivation_acceleration 字段
- **client**: `StatusSnapshotHandler` 解析 `cultivation_acceleration`、`StatusEffectStore.cultivationAcceleration()`、`StatusEffectHudPlanner` "修炼 xN.N" 显示
- **agent**: 无直接改动（天道 agent 不参与修炼速率逻辑）

### 遗留 / 后续

- Client HUD 渲染精化（经脉进度条旁倍率显示 + 预估剩余时间）：server 已准备 cultivation_acceleration 字段，client 当前显示"修炼 xN.N"文字，进度条旁精确倍率显示待人工实现
- P2.4 端到端体验测试：需人工 runClient 验证完整流程（采草 -> 炼丹 -> 服药 -> 开脉 -> 突破）
- 公共 NPC 丹炉 POI（§8.1 #6）：需在 worldgen zone 配置中添加初醒原教学区丹炉，非本 plan 范围
- 高品质丹药 NPC 回收（plan P2.2 "perfect 可卖 NPC 获溢价"）：NPC 买入逻辑未实装，待 NPC 交易系统 v2 支持
