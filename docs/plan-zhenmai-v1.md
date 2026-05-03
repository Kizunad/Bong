# Bong · plan-zhenmai-v1

**截脉·震爆流**（防御）。受击瞬间皮下震爆中和异种真元——"以血保真元"。**v1 模式特殊**：P0 已落地（plan-combat-no_ui 一同验收），P1 是境界分级 + prep + 僵直 + 距离梯度 + 装备耦合的扩展。

**Primary Axis**（worldview §五:465 已正典）：**弹反窗口 + 污染真元中和效率**

## 阶段总览

| 阶段 | 状态 | 验收 |
|---|---|---|
| **P0** 基础弹反（200ms / 5 qi / 0.2x contam / 0.3 severity 体表伤）| ✅ **已落地（2026-04 plan-combat-no_ui）** | 1539 单测过 |
| **P1** 境界分级 + prep 1s + 触发僵直 + 距离梯度 + 装备耦合 + FOV + agent narration | ⬜ | — |
| P2 v1 收口（饱和 testing + 数值平衡） | ⬜ | — |

> **vN+1 (plan-zhenmai-v2)**：连环震爆 + 预判震爆（±0.10s 极限 timing）+ 化虚弹反一切（断绝）+ 与爆脉流深度 trade-off matrix

---

## 世界观 / library / 交叉引用

**worldview 锚点**：
- §五.防御.1 截脉/震爆流（line 432-436：血肉反应装甲 / 极限弹反 / 类动作游戏弹反 / 体修近战狂人匹配）
- §五:465 流派 primary axis 表（弹反窗口 + 污染真元中和效率）
- §五:386-389 "防御三流皆克制不了真正的体修爆脉"（爆脉求损 vs 截脉减损 — Q64 距离梯度自然实现）
- §四 异体排斥（line 342-349：侵染 + 排异反应 + 交换比亏损 — 中和的物理依据）
- §四 过载撕裂（line 351-358：经脉裂痕物理 — 震爆是"过载撕裂"小型化定向化）

**library 锚点**：
- `peoples-0006 战斗流派源流`（防御一·截脉/震爆流原文："**截脉者，皮下引爆真元中和异种真元，以血保真元；时机判断极苛**")

**交叉引用**：
- `plan-combat-no_ui-v1` ✅（已落地）— **P0 直接复用** `DefenseIntent` / `DefenseWindow` / `apply_defense_intents` / `JIEMAI_*` 常量；P1 扩展 `JIEMAI_DEFENSE_WINDOW_MS` + 境界分级
- `plan-cultivation-v1` ✅（已落地）— 经脉系统、`Cultivation.qi_current` 扣减
- `plan-armor-v1` ✅（已落地）— P1 接入 `Armor.weight_class`（轻/中/重）影响 prep window
- `plan-perception-v1.1` ✅（已落地）— FOV 方向判定接入 `attacker.eye_pos vs defender.facing`
- `plan-baomai-v1` ✅（已落地）— 爆脉攻击常态 reach 0.9 格，自动落入 jiemai 距离梯度的"贴脸残效"区（30% 效果）

## 接入面 checklist（防孤岛 — 严格按 docs/CLAUDE.md §二）

- **进料**：`Cultivation.qi_current` 扣 jiemai 成本 → `Combat.attacker.position` 计算 hit_distance → `Combat.defender.facing` 校验 FOV → `Armor.weight_class` 影响 prep window → `Cultivation.realm` 决定境界分级窗口/qi/effectiveness
- **出料**：`CombatEvent::DefenseTriggered { defender, kind: JieMai, effectiveness }`（已实装）→ `StatusEffectKind::ParryRecovery` 僵直 status（新增）→ `Contamination.entries.last_mut().amount *= contam_mul` → `Wounds.entries.push(Wound { kind: Concussion, severity: distance_graded })`
- **共享类型 / event**：复用 `DefenseIntent` / `DefenseWindow` / `CombatState.incoming_window` / `Contamination` / `Wounds` / `Cultivation`；扩展 `JIEMAI_DEFENSE_WINDOW_MS`（200 → 1000）+ 新增 `StatusEffectKind::ParryRecovery` variant
- **跨仓库契约**：
  - server: `combat::resolve.rs::apply_defense_intents` 升级（境界分级 + 僵直施加）/ `combat::resolve.rs::resolve_attack_intents` jiemai 分支扩展（距离梯度 + FOV）/ `combat::components.rs::JIEMAI_*` 常量重定义为函数 `jiemai_window_ms_for_realm` / `jiemai_qi_cost_for_realm` / `jiemai_contam_multiplier` / `jiemai_effectiveness_by_distance`
  - schema: `agent/packages/schema/src/combat-event.ts` 已有 `DefenseTriggeredV1`，扩展 `effectiveness: f32` 字段
  - client: `bong:combat/defense_stance` (inbound, plan-combat-no_ui §592 已定义) 复用；新增 prep 1s 视觉反馈 + 僵直 visual cue
- **特性接入面（v1 仅留 hook）**：worldview §五:466 "防御三流不绑染色"已正典 — jiemai 不与染色挂钩
  - **真元流速**（woliu primary axis 泛型化）→ jiemai prep window 延长 X% — vN+1 接入
  - **沉重色（Heavy）**（worldview §六）→ 体修近战 jiemai 自伤减半（仅 wound severity，不影响 contam mul）— vN+1 接入

**Hotbar 接入声明**（2026-05-03 user 正典化"所有技能走 hotbar"）：
- **`bong:combat/defense_stance`**（plan-combat-no_ui §592 已实装现有 packet）= **Technique** → P0 现状保留（已落地），**P1 改写时同步迁移到 hotbar 1-9**（`Technique::ZhenmaiParry` 绑战斗·修炼栏 + UseQuickSlot 触发）
- 详见 `plan-woliu-v1.md §8 跨 plan hotbar 同步修正备注`。

---

## §A 概览（设计导航）

> 截脉/震爆流 = 受击瞬间皮下震爆中和异种真元——以**0.5s 僵直**换取**80% 污染减免** + **轻量自伤**。物理本质 = 修士主动让本音"高频化"与入侵真元做"破坏性干涉"（音论）。**关键边界**：贴脸 jiemai 效率残（30%），无法克制爆脉；远距离 jiemai 全效（100%），克暗器/法术/远程武器。

### A.0 v1 实装范围（2026-05-03 拍板）

| 维度 | v1 实装 | 搁置 vN+1 |
|---|---|---|
| **基础弹反**（已落地）| ✅ 200ms / 5 qi / 0.2x contam / 0.3 severity（凝脉默认值，但实装其实是引气级——见 A.1）| — |
| 境界分级窗口/qi（**P1**）| 4 档（引气/凝脉/固元/通灵）| 化虚（已断绝）|
| prep 模式（**P1 Q61: C**）| 短期 prep 1s（按键瞬间预备）| 持续姿态 toggle（A 选项）|
| 触发僵直（**P1 Q62 reframe**）| `StatusEffectKind::ParryRecovery` 0.5s 通用 | 高境减僵直（通灵 0.3s / 化虚 0.1s）|
| 距离梯度（**P1 Q64 双轴**）| `jiemai_effectiveness(hit_distance)` 公式 — 远 1.0 / 贴脸 0.3 | 完整 trade-off matrix |
| 装备重量耦合（**P1 Q63: B**）| 轻 ×1.0 / 中 ×0.9 / 重 ×0.6 | 多档护甲细化 |
| FOV 方向判定（**P1 Q65: D**）| 引气 180° / 凝脉 200° / 固元 270° / 通灵 360° | 360° 环视感知（特殊神识）|
| 失败累积（**Q62 reframe 取消**）| ❌ 不做经脉损伤累积（取代为僵直）| — |
| vs 爆脉特判 | ❌ 不做（Q64 距离梯度自然实现）| — |
| agent narration（**P1**）| `DefenseTriggered` 推 narration（"X 弹反千钧之矛 / X 反应不及"）| LifeRecord "X 在 N 战中弹反 Y" |
| 染色加成 | ❌ 不绑染色（worldview §五:466 正典）| — |

### A.1 v1 P0 已实装 vs worldview 数值表 gap

worldview / skeleton §3 数值表 vs 当前 `JIEMAI_*` 常量：

| 境界 | worldview 窗口 | worldview qi | 当前实装 | 状态 |
|---|---|---|---|---|
| 引气 | ±0.05s (100ms) | 5 | 200ms / 5 / 0.2 / 0.3 | ❌ 窗口偏宽 |
| **凝脉** | ±0.10s (200ms) | 6 | **同上** | ⚠️ 窗口对，**qi 偏低** |
| 固元 | ±0.15s (300ms) | 8 | 同上 | ❌ 偏紧 + qi 偏低 |
| 通灵 | ±0.20s (400ms) | 10 | 同上 | ❌ 偏紧 + qi 偏低 |

**Q60: B 路径**：P1 实装时同时做 v1.1 修正 — 把"现实装"重新定位为**引气过渡值（窗口 200ms 偏宽 = 实际是 1s prep 简化版的产物）**；P1 落地境界分级时调整：
- prep window 全境界统一 1000ms（Q66 简化）
- qi cost 按 worldview：引气 5 / 凝脉 6 / 固元 8 / 通灵 10
- contam_multiplier 全境界 0.2（中和 80% 上限）
- distance graded effectiveness 公式（Q64 双轴）

> **v1 简化备注**：worldview "凝脉 ±0.10s 极限弹反" 的精确 timing 在 v1 实装为 1s prep 内任意命中即触发——实际是"宽容版弹反"。vN+1 引入 `±0.10s` timing window 区分"普通 parry"与"极限震爆"（高阶 timing 触发额外 contam 减免奖励）。

### A.2 v1 P1 数值表（提议）

#### 窗口 / qi（境界分级）

| 境界 | prep window | qi cost | contam mul | concussion 基础 severity |
|---|---|---|---|---|
| 醒灵 | 不可学（镜身太薄）| — | — | — |
| **引气** | 1000ms | 5 | 0.2 | 0.3 |
| **凝脉** | 1000ms | 6 | 0.2 | 0.3 |
| **固元** | 1000ms | 8 | 0.2 | 0.3 |
| **通灵** | 1000ms | 10 | 0.2 | 0.3 |

> 全境界 prep 同 1000ms — 这是 v1 简化。境界差异体现在 **qi cost** + **FOV** + **vN+1 的 timing 极限弹反**。

#### 距离梯度（Q64 双轴公式）

```rust
fn jiemai_effectiveness(hit_distance: f32) -> f32 {
    if hit_distance >= 2.0 { 1.0 }
    else if hit_distance <= 0.9 { 0.3 }
    else { 0.3 + (hit_distance - 0.9) / (2.0 - 0.9) * 0.7 }
}
// contam 实际减免： contam_mul = 1.0 - (1.0 - 0.2) * effectiveness
// concussion 实际自伤： severity = 0.3 / effectiveness
```

| 攻击距离 | 效率 | contam 减免 | 自伤 severity | 战术含义 |
|---|---|---|---|---|
| ≥ 2.0 格（剑/枪/远程）| 1.0 | 减 80% | 0.30 | 标准甜区 |
| 1.5 格（短刃/拳带步）| 0.68 | 减 54% | 0.44 | 效率打折 |
| 1.3 格（拳）| 0.55 | 减 44% | 0.55 | 越来越亏 |
| ≤ 0.9 格（贴脸/爆脉）| 0.3 | 减 24% | 1.0 | 几乎无效 + 自伤翻倍 |

#### FOV（Q65: D）

| 境界 | 可弹反角度 | server 校验 |
|---|---|---|
| 引气 | 180° | `defender.facing.dot(attacker_dir) >= 0` |
| 凝脉 | 200° | dot(approx) >= -0.17 (cos(100°)) |
| 固元 | 270° | dot(approx) >= -0.71 (cos(135°)) |
| 通灵 | 360° | 不检查 FOV |

#### 装备重量（Q63: B，接 plan-armor-v1）

```rust
fn jiemai_armor_modifier(weight_class: WeightClass) -> f32 {
    match weight_class {
        WeightClass::Light => 1.0,
        WeightClass::Medium => 0.9,
        WeightClass::Heavy => 0.6,
    }
}
// 应用：实际 prep window = 1000ms × armor_mod
//       Light: 1000ms / Medium: 900ms / Heavy: 600ms
```

### A.3 v1 实施阶梯

```
P0  已落地（2026-04 plan-combat-no_ui v1 一起验收）
       JIEMAI_DEFENSE_WINDOW_MS = 200 / qi 5 / contam 0.2 / severity 0.3
       apply_defense_intents 已实装 / resolve.rs jiemai 分支已实装

P1  本 plan 新做（v1 P1 ≈ 其他 plan 的 P0 + P1）
       ├── A. 境界分级 qi cost (5/6/8/10)
       ├── B. prep window 200ms → 1000ms 改写
       ├── C. StatusEffectKind::ParryRecovery 0.5s 僵直施加
       ├── D. jiemai_effectiveness(hit_distance) 双轴梯度公式落地
       ├── E. plan-armor-v1.weight_class 接入（prep × armor_mod）
       ├── F. FOV 境界分级（180/200/270/360）+ defender.facing 校验
       └── G. CombatEvent::DefenseTriggered 扩展 effectiveness 字段 → agent narration
       ↓ 饱和 testing
P2  v1 收口
       ├── 数值平衡（fight room 演练 + agent 推爆脉 vs 截脉对抗）
       └── LifeRecord "X 在 N 战中弹反 Y" 事件
```

### A.4 v1 已知偏离正典（vN+1 必须修复）

- [ ] **极限 timing 弹反**（worldview "凝脉 ±0.10s 极限"）—— v1 是 1s prep 宽容版；vN+1 引入 ±0.10s timing window
- [ ] **化虚弹反一切**（worldview line 392 "理论存在但化虚已断绝"）—— v1 不实装
- [ ] **预判震爆 / 连环震爆**（skeleton §2 高阶用法）—— v1 不实装
- [ ] **染色亲和**（沉重色 wound 减半 / 真元流速 prep 延长）—— v1 不实装
- [ ] **dugu 师 ParryRecovery 期间灌毒蛊免疫**？—— 跨 plan 设计 case，待 plan-dugu-v2 时拍

### A.5 v1 关键开放问题

**已闭合**（Q60-Q65 + Q66-Q69 推断，2026-05-03）：
- Q60 → B v1.1 修正路径（P1 落地时把"现实装"重定位为引气过渡）
- Q61 → C 短期 prep 1s
- Q62 reframe → 触发僵直取代失败累积自损
- Q63 → B 重甲 ×0.6 / 中 ×0.9 / 轻 ×1.0
- Q64 reframe → 双轴梯度 jiemai_effectiveness（远 1.0 / 贴脸 0.3）
- Q65 → D FOV 境界分级（180/200/270/360）
- Q66 → prep 1s 简化版
- Q67 → 僵直 0.5s 通用
- Q68 → Q64 双轴梯度
- Q69 → 距离过近不静默

**仍 open**（v1 实施时拍板）：
- [ ] **Q70. 网络延迟容忍**：client 输入时戳 vs server tick 的允许偏差 — 建议起手 ±150ms
- [ ] **Q71. 僵直状态可否打断**：被攻击命中是否清除 ParryRecovery？建议**否**（僵直 = 真实 0.5s）
- [ ] **Q72. prep 期间是否可移动**：玩家进入 prep 后能否走位？建议**慢速可移动**（速度 ×0.7）
- [ ] **Q73. ParryRecovery 与其他状态的叠加**：与 Slowed 是否互斥？建议**叠加**（按 stamina 各自结算）

---

## §0 设计轴心

- [ ] 震爆 = **修士主动在受击点制造一次微型过载**，让本音瞬间高频化与入侵真元相互抵消
- [ ] 极限弹反窗口 = 受击 ±0.1-0.2s（按境界）
- [ ] 代价：体表伤口 +1 档，但污染清零（"以血保真元"）
- [ ] 末法约束：失败 = 自伤白挨；连续失败经脉 MICRO_TEAR

## §1 第一性原理（烬灰子四论挂点）

- **音论·主动失谐对消**：受击瞬间在皮下主动让本音"高频化"——入侵的异种真元和高频本音相撞，**两个失谐音相互抵消**（音学的破坏性干涉）。这是为什么时机要苛——必须在异音"未深入"时叠音对消
- **缚论·镜身局部扣减**：动作本身让修士自己镜身在受击点局部龟裂——所以"以血保真元"
- **影论·镜面震荡**：震爆瞬间镜面剧烈震荡，能量从镜面流向皮下 → 体表伤口 +1 档
- **过载撕裂复用**：震爆其实是"过载撕裂"的小型化、定向化版本——不是为攻击，是为防御

## §2 招式 / 形态分级

| 形态 | 触发条件 | 真元成本 | 效果 |
|---|---|---|---|
| **皮下震爆**（标准）| 受击 ±窗口内主动 | 5-10 真元 | 污染 = 0 + 体表 +1 档 |
| **预判震爆**（高阶）| 看招式动作预先按 | 10-20 真元 | 窗口 +50%，但提前按浪费真元 |
| **连环震爆**（极险）| 连续受击全部弹 | 累计真元 | 第 N 次失败概率指数升 |

## §3 数值幅度梯度（按境界）

| 境界 | 弹反窗口 | 中和量上限 | 触发真元成本 |
|---|---|---|---|
| 醒灵 | 不可学（镜身太薄）| — | — |
| **引气** | ±0.05s | ≤ 5 | 5 |
| **凝脉** | ±0.10s | ≤ 15 | 6 |
| **固元** | ±0.15s | ≤ 30 | 8 |
| **通灵** | ±0.20s | ≤ 50 | 10 |
| **化虚**（理论）| ±0.3s "弹反一切" | ∞ | — |

**装备/姿态约束**：
- 必须**持械近战姿态**（不能盾牌 / 远程 / 退步姿态）
- 装备护甲过重（plan-armor-v1）→ 窗口 -30%

## §3.1 截脉·v1 规格（P1 阶段——P0 已落地）

> **本 plan 模式特殊**：P0（200ms / 5 qi / 0.2x contam / 0.3 severity）已随 plan-combat-no_ui v1 落地（2026-04 验收 1539 单测）。本节规格是 **P1 扩展**——把 P0 的"凝脉默认值"升级为完整境界分级 + prep + 僵直 + 距离梯度 + 装备耦合 + FOV。

### 3.1.A 境界分级窗口与 qi cost（Q60: B v1.1 修正路径）

**当前实装常量**（`server/src/combat/components.rs:23-27`）：
```rust
pub const JIEMAI_DEFENSE_WINDOW_MS: u32 = 200;   // ⚠️ P1 改 1000
pub const JIEMAI_DEFENSE_QI_COST: f64 = 5.0;     // ⚠️ P1 改函数
pub const JIEMAI_CONTAM_MULTIPLIER: f64 = 0.2;   // ✅ 保留
pub const JIEMAI_CONCUSSION_SEVERITY: f32 = 0.3; // ✅ 保留为基础值
```

**P1 改写**（常量 → 函数）：
```rust
// 全境界统一 prep
pub const JIEMAI_PREP_WINDOW_MS: u32 = 1000;     // P1 起 200 → 1000

// 境界分级 qi cost
pub fn jiemai_qi_cost_for_realm(realm: Realm) -> f64 {
    match realm {
        Realm::XingLing => f64::INFINITY,  // 醒灵不可学（worldview line 39 "不可修"）
        Realm::YinQi => 5.0,
        Realm::NingMai => 6.0,
        Realm::GuYuan => 8.0,
        Realm::TongLing => 10.0,
        Realm::HuaXu => 10.0,              // 化虚已断绝，沿用通灵值（vN+1 弹反一切）
    }
}

// 全境界统一 contam mul（中和上限 80%）
pub const JIEMAI_CONTAM_MULTIPLIER: f64 = 0.2;

// concussion 基础 severity
pub const JIEMAI_CONCUSSION_BASE_SEVERITY: f32 = 0.3;
```

**含义**：境界差异不在"窗口紧度"上（vN+1 才做 ±0.10s 极限弹反），v1 体现在 **qi cost** + **FOV** 两轴。高境玩家 jiemai 更贵，但视野更广（A.2 FOV 表）。

### 3.1.B Prep window（Q61: C 短期 1s）

**当前实装语义**：玩家发 `DefenseIntent` → server `apply_defense_intents` → 设 `incoming_window` 200ms → 命中事务读 `incoming_window` 判断是否仍 open。

**P1 改写**：
- `JIEMAI_DEFENSE_WINDOW_MS: 200 → 1000`（重命名为 `JIEMAI_PREP_WINDOW_MS`）
- 但 prep 实际生效时间 = `1000ms × jiemai_armor_modifier(armor.weight)`（轻 1000 / 中 900 / 重 600）
- prep 期内任意命中都触发 jiemai 结算（v1 不分 timing 等级；vN+1 引入 ±0.10s 极限弹反给 timing 准的玩家奖励）

```rust
pub fn apply_defense_intents(
    mut defenses: EventReader<DefenseIntent>,
    mut defenders: Query<(
        &mut CombatState,
        &Cultivation,
        Option<&Armor>,
        Option<&StatusEffects>,
    )>,
    // 新增：施加 ParryRecovery 僵直
    mut status_intents: EventWriter<ApplyStatusEffectIntent>,
) {
    for defense in defenses.read() {
        let Ok((mut combat_state, cult, armor, status_effects)) = defenders.get_mut(defense.defender) else { continue };

        // 已有：Stunned 不能 parry
        if status_effects.is_some_and(|se| has_active_status(se, StatusEffectKind::Stunned)) { continue; }

        // 新增：醒灵不能 parry
        if cult.realm == Realm::XingLing { continue; }

        // 新增：ParryRecovery 期间不能再次 parry（僵直内禁连续按）
        if status_effects.is_some_and(|se| has_active_status(se, StatusEffectKind::ParryRecovery)) { continue; }

        // 新增：装备重量影响 prep window
        let armor_mod = armor.map_or(1.0, |a| jiemai_armor_modifier(a.weight_class));
        let prep_ms = (JIEMAI_PREP_WINDOW_MS as f32 * armor_mod) as u32;

        combat_state.incoming_window = Some(DefenseWindow {
            opened_at_tick: defense.issued_at_tick,
            duration_ms: prep_ms,
        });

        // 新增：触发即施加 0.5s ParryRecovery 僵直（Q62 reframe）
        status_intents.send(ApplyStatusEffectIntent {
            target: defense.defender,
            kind: StatusEffectKind::ParryRecovery,
            duration_ms: 500,
            source: defense.source,
        });
    }
}
```

### 3.1.C 触发僵直 ParryRecovery（Q62 reframe）

**新增 status effect variant**（`server/src/combat/events.rs::StatusEffectKind`）：

```rust
pub enum StatusEffectKind {
    // ... existing variants ...
    /// 截脉触发僵直（worldview §五.防御.1 "类动作游戏弹反"）
    /// 期间不能再次 attack / parry / sprint，可慢速移动（×0.7）
    ParryRecovery,
}
```

**`StatusEffectRegistry` 行为定义**：
- `duration_ms`: 500 (Q67: 不分境界，统一 0.5s)
- `effects`:
  - `block_attack: true` — 不能再发 AttackIntent
  - `block_defense: true` — 不能再发 DefenseIntent
  - `move_speed_mul: 0.7` — Q72 慢速可移动
  - `block_sprint: true` — 不能 sprint
- `dispellable: false` — Q71 僵直不可被打断
- `stacks_with: [Slowed, Bleeding, ...]` — Q73 与其他 status 叠加

**注**：`PendingDuguInfusion`（plan-dugu-v1）若与 `ParryRecovery` 同时存在 → dugu 师选择"灌毒蛊但 parry 失败陷僵直" — 60s pending 内仍可恢复后出手。跨 plan 不冲突，留 plan-dugu-v2 时拍最终交互。

### 3.1.D 距离梯度（Q64 双轴公式）

**新增函数**（`server/src/combat/jiemai.rs` 新文件）：

```rust
/// jiemai 效率随 hit_distance 衰减：远距离 1.0，贴脸 0.3
/// worldview §五:386-389 "防御三流皆克制不了真正的体修爆脉" 的物理实现：
/// 爆脉常态 reach 0.9 格 → 永远落入 0.3 残效区
pub fn jiemai_effectiveness(hit_distance: f32) -> f32 {
    if hit_distance >= 2.0 { 1.0 }
    else if hit_distance <= 0.9 { 0.3 }
    else {
        // 0.9 - 2.0 线性插值：0.3 → 1.0
        0.3 + (hit_distance - 0.9) / (2.0 - 0.9) * 0.7
    }
}

/// 双轴：contam 减免效率 ↓ + concussion 自伤 ↑
pub fn jiemai_apply_effects(
    eff: f32,
    contam_amount: &mut f64,
    concussion_severity: &mut f32,
) {
    // contam mul = 1.0 - (1.0 - 0.2) * eff
    let contam_mul = 1.0 - (1.0 - JIEMAI_CONTAM_MULTIPLIER) * eff as f64;
    *contam_amount *= contam_mul;

    // wound severity = 0.3 / eff（贴脸自伤翻倍）
    *concussion_severity = JIEMAI_CONCUSSION_BASE_SEVERITY / eff;
}
```

**改 `resolve_attack_intents` jiemai 分支**（`server/src/combat/resolve.rs:430-451`）：

```rust
// 现有代码：
//   if window_open && qi_current >= JIEMAI_DEFENSE_QI_COST {
//       qi_current -= JIEMAI_DEFENSE_QI_COST;
//       last_contam.amount *= JIEMAI_CONTAM_MULTIPLIER;
//       wounds.push(Wound { severity: JIEMAI_CONCUSSION_SEVERITY, ... });
//   }

// P1 改写：
let qi_cost = jiemai_qi_cost_for_realm(defender_cultivation.realm);
if window_open && defender_cultivation.qi_current + f64::EPSILON >= qi_cost {
    // FOV 校验（Q65: D，§3.1.E）
    if !jiemai_fov_check(&attacker_position, &defender_facing, defender_cultivation.realm) {
        // FOV 不通过：window 失效，但 qi 不扣，僵直已施加
        combat_state.incoming_window = None;
        continue 'resolve_jiemai;
    }

    defender_cultivation.qi_current -= qi_cost;

    // 距离梯度（Q64）
    let eff = jiemai_effectiveness(hit_distance);
    let mut contam_amount = last_contam.amount;
    let mut concussion_severity = JIEMAI_CONCUSSION_BASE_SEVERITY;
    jiemai_apply_effects(eff, &mut contam_amount, &mut concussion_severity);
    last_contam.amount = contam_amount;
    emitted_contam_delta = contam_amount;

    wounds.entries.push(Wound {
        location: hit_probe.body_part,
        kind: WoundKind::Concussion,
        severity: concussion_severity,
        bleeding_per_sec: JIEMAI_CONCUSSION_BLEEDING_PER_SEC,
        created_at_tick: clock.tick,
        inflicted_by: Some(attacker_id.clone()),
    });

    jiemai_success = true;
    jiemai_effectiveness_value = eff;  // 给 CombatEvent::DefenseTriggered
}

combat_state.incoming_window = None;
```

### 3.1.E FOV 方向判定（Q65: D 境界分级）

**新增函数**：

```rust
pub fn jiemai_fov_dot_threshold(realm: Realm) -> f32 {
    match realm {
        Realm::YinQi => 0.0,      // 180° (cos 90°)
        Realm::NingMai => -0.17,  // 200° (cos 100°)
        Realm::GuYuan => -0.71,   // 270° (cos 135°)
        Realm::TongLing => -1.0,  // 360° 不限
        Realm::HuaXu => -1.0,     // 同通灵
        _ => 0.0,                 // 醒灵 fallback (实际不会走到，前置已挡)
    }
}

pub fn jiemai_fov_check(
    attacker_pos: &Position,
    defender_pos: &Position,
    defender_facing: &Facing,
    realm: Realm,
) -> bool {
    let attacker_dir = (attacker_pos.0 - defender_pos.0).normalize();
    let dot = defender_facing.dir().dot(attacker_dir);
    dot >= jiemai_fov_dot_threshold(realm)
}
```

**含义**：
- 引气：仅前 180° 可弹反，背后偷袭无效
- 通灵：360° 全感知（worldview "敏锐如野兽"）

### 3.1.F 装备重量耦合（Q63: B，接 plan-armor-v1）

```rust
pub fn jiemai_armor_modifier(weight: WeightClass) -> f32 {
    match weight {
        WeightClass::Light => 1.0,
        WeightClass::Medium => 0.9,
        WeightClass::Heavy => 0.6,
    }
}

// 应用：见 §3.1.B 已嵌入 apply_defense_intents
```

`Armor.weight_class` 在 plan-armor-v1 已落地（轻/中/重）—— v1 不需要新增字段，直接 query `Option<&Armor>` 接入。

### 3.1.G CombatEvent 扩展 + agent narration

**扩展 `CombatEvent::DefenseTriggered`**（`server/src/combat/events.rs`）：

```rust
pub enum CombatEvent {
    // ...
    DefenseTriggered {
        defender: PlayerId,
        kind: DefenseKind,
        effectiveness: f32,    // 新增 — Q64 双轴效率
        contam_reduced: f64,   // 新增 — 实际减少的污染量
        wound_severity: f32,   // 新增 — 实际自伤
    },
}
```

**对应 schema**（`agent/packages/schema/src/combat-event.ts`）：

```typescript
export const DefenseTriggeredV1 = Type.Object({
    defender: PlayerIdV1,
    kind: Type.Union([Type.Literal("JieMai"), Type.Literal("TiShi"), Type.Literal("JueLing")]),
    effectiveness: Type.Number(),       // 新增 0.3 - 1.0
    contamReduced: Type.Number(),       // 新增
    woundSeverity: Type.Number(),       // 新增
});
```

**agent narration 触发条件**（v1 P1 实装）：
- `effectiveness >= 0.7` → "X 弹反千钧之矛"（甜区）
- `0.3 < effectiveness < 0.7` → "X 勉强挡住但已露破绽"（中距亏）
- `effectiveness == 0.3` → "X 反应不及 / 震爆冲到自己"（贴脸残效）
- `jiemai_success == false` → "X 看似要 parry 却落空"（FOV/距离/qi 不足）

---

## §4 材料 / 资源链

**震爆流不依赖材料**——这是它的优势（书里"运营全在自身，不依外物"原本是说爆脉，震爆同理）。但有以下辅助：

| 辅助 | library 来源 | 用途 |
|---|---|---|
| 凝脉草 / 养经苔 | ecology-0002 | 失败后养皮下经脉 |
| 安神果 | ecology-0002 | 顿悟"反应敏锐"加成（worldview §六.3）|

## §5 触发 / 流程

```
准备阶段：玩家进入"截脉姿态"（消耗 2 真元 / 秒维持，或被动）
受击前 0.2s：HUD 显示弹反窗口（视觉/听觉提示）
玩家按 parry → 检查窗口 timing →
  ✅ 命中：异种真元 = 0 + 体表 wound +1 档
  ❌ 失败：异种真元正常 + 体表 wound +2 档（白挨自伤）
连续失败 3 次：受击点经脉 MICRO_TEAR
```

## §6 反噬 / 失败代价

- [ ] 反应失败 → 体表 +2 档 + 污染照常累积
- [ ] 连续触发 3 次失败 → 受击点经脉 MICRO_TEAR
- [ ] 维持"截脉姿态"持续耗真元（站桩等待 = 烧真元）
- [ ] 装备过重窗口缩短（与 plan-armor-v1 形成 trade-off）
- [ ] 化虚级"弹反一切"理论存在但化虚已断绝

## §7 克制关系

- **克**：暗器流（注入瞬间炸掉）；涡流流（涡流流不靠近，无可弹）
- **被克**：**爆脉流**（爆脉求损 → 弹反带来的自伤恰是爆脉所求；worldview "防御三流皆克制不了真正的体修爆脉"）；毒蛊流（毒不是即时威能，弹反等不到爆点）
- **染色关联**：世界观 §六.2 明确防御三流是战术选择而非长期修习，不绑定染色。体修（沉重色）天然顺手但非专属；丹师（温润色）反应慢弹反窗口几乎用不上

## §8 数据契约

### v1 P0 已落地清单（reverse engineered，2026-04 plan-combat-no_ui v1 验收）

| 模块 | 文件路径 | 状态 |
|---|---|---|
| Defense window component | `server/src/combat/components.rs:138-149` | ✅ `DefenseWindow { opened_at_tick, duration_ms }` |
| CombatState integration | `server/src/combat/components.rs:151-160` | ✅ `incoming_window: Option<DefenseWindow>` |
| Defense intent | `server/src/combat/events.rs:54` | ✅ `DefenseIntent { defender, issued_at_tick, source }` |
| Apply defense intents | `server/src/combat/resolve.rs:110-128` | ✅ `apply_defense_intents` 系统 |
| Jiemai 分支结算 | `server/src/combat/resolve.rs:422-454` | ✅ window 检查 + qi 扣减 + contam mul + concussion wound |
| 常量 | `server/src/combat/components.rs:23-27` | ✅ `JIEMAI_*` 5 个常量 |
| 单测 | `server/src/combat/resolve.rs:2200-2400+` | ✅ 多个 jiemai 场景测 |

### v1 P1 落地清单（按 §3.1 规格）

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| 常量 → 函数 | `server/src/combat/components.rs` | `JIEMAI_PREP_WINDOW_MS = 1000` (取代 200) / `jiemai_qi_cost_for_realm` / `JIEMAI_CONCUSSION_BASE_SEVERITY` |
| 距离梯度 | `server/src/combat/jiemai.rs` (新文件) | `jiemai_effectiveness(hit_distance)` / `jiemai_apply_effects(eff, &mut contam, &mut severity)` |
| FOV 校验 | `server/src/combat/jiemai.rs` | `jiemai_fov_dot_threshold(realm)` / `jiemai_fov_check(...)` |
| 装备耦合 | `server/src/combat/jiemai.rs` | `jiemai_armor_modifier(weight)` |
| Apply defense 升级 | `server/src/combat/resolve.rs` | `apply_defense_intents` 加 realm/armor/ParryRecovery 逻辑 |
| Resolve attack jiemai 分支 | `server/src/combat/resolve.rs:422+` | 嵌入 FOV check + 距离梯度 + effectiveness 字段输出 |
| ParryRecovery status | `server/src/combat/events.rs::StatusEffectKind` + `combat/status.rs::EFFECT_REGISTRY` | 新增 variant + 0.5s duration + block_attack/defense + speed×0.7 |
| CombatEvent 扩展 | `server/src/combat/events.rs` | `DefenseTriggered` 加 `effectiveness` / `contam_reduced` / `wound_severity` 字段 |
| Schema | `agent/packages/schema/src/combat-event.ts::DefenseTriggeredV1` | 加三个字段，sample 双端校验 |
| Agent narration | `agent/packages/tiandao/src/zhenmai-narration.ts` | 按 effectiveness 分档触发 |
| Client HUD prep visual | `client/.../hud/ParryHud.java` | 1s prep 进度条 + 0.5s 僵直 cue |
| 单测 | `server/src/combat/jiemai_tests.rs` | 距离梯度 / FOV / 装备 / 醒灵不可学 / ParryRecovery 阻塞 |

### v1 P2 落地清单

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| 数值平衡 | `server/src/combat/jiemai_balance.rs` (可选) | fight room 演练数据收集 |
| LifeRecord | `server/src/lore/life_record.rs` | "X 在 N 战中弹反 Y" 事件类型 |

## §9 实施节点

详见 §A.3 v1 实施阶梯。三阶段：

- [x] **P0** 基础弹反 — **已落地（2026-04 plan-combat-no_ui v1 验收）**
- [ ] **P1** 境界分级 + prep 1s + 触发僵直 + 距离梯度 + 装备耦合 + FOV + agent narration —— 见 §3.1
- [ ] **P2** v1 收口（数值平衡 + LifeRecord）

## §10 开放问题

### 已闭合（2026-05-03 拍板，10 个决策）

- [x] **Q60** → B v1.1 修正路径
- [x] **Q61** → C 短期 prep 1s
- [x] **Q62 reframe** → 触发僵直取代失败累积自损
- [x] **Q63** → B 重甲 ×0.6 / 中 ×0.9 / 轻 ×1.0
- [x] **Q64 reframe** → 双轴梯度 jiemai_effectiveness
- [x] **Q65** → D FOV 境界分级（180/200/270/360）
- [x] **Q66** → prep 1s 简化版
- [x] **Q67** → 僵直 0.5s 通用
- [x] **Q68** → Q64 双轴梯度
- [x] **Q69** → 距离过近不静默

### 仍 open（v1 实施时拍板）

- [ ] **Q70. 网络延迟容忍**：client 输入时戳 vs server tick 的允许偏差 — 建议起手 ±150ms（覆盖正常网络抖动），P1 实装时按 anti-cheat 标准调
- [ ] **Q71. 僵直状态可否打断**：建议**否**（僵直 = 真实 0.5s，被打也只能挨）—— P1 实装时确认 Stunned status 不清除 ParryRecovery
- [ ] **Q72. prep 期间是否可移动**：建议**慢速可移动**（速度 ×0.7，符合"备战姿态"）—— P1 实装时配 EFFECT_REGISTRY
- [ ] **Q73. ParryRecovery 与其他状态叠加**：与 Slowed 互斥还是叠加？建议**叠加**（按各自 stamina 结算）

### vN+1 留待问题（plan-zhenmai-v2 时拍）

- [ ] **极限 timing 弹反**（worldview "凝脉 ±0.10s 极限"）—— 引入 ±0.10s timing window；命中在 ±0.10s 内 → effectiveness × 1.3 奖励
- [ ] 化虚"弹反一切"是否在游戏内做暗示（NPC 传说 / library 书籍）？
- [ ] 预判震爆 / 连环震爆（skeleton §2 高阶用法）
- [ ] 染色亲和（沉重色 wound 减半 / 真元流速 prep 延长 / 飘逸色 prep 缩短惩罚）
- [ ] 高境减僵直（通灵 0.3s / 化虚 0.1s "弹反一切"）
- [ ] 完整 trade-off matrix（暗器流 / 法术 / 各类武器）

## §11 进度日志

- 2026-04-26：骨架创建。依赖 plan-combat-no_ui 受击窗口接口 + plan-armor-v1 重量耦合。无对应详写功法书，从 worldview + 战斗流派源流 推演。
- 2026-04（plan-combat-no_ui v1 验收同期）：**P0 已落地** —— `JIEMAI_DEFENSE_WINDOW_MS=200` / `JIEMAI_DEFENSE_QI_COST=5` / `JIEMAI_CONTAM_MULTIPLIER=0.2` / `JIEMAI_CONCUSSION_SEVERITY=0.3` 与 `apply_defense_intents` + `resolve.rs` jiemai 分支随 plan-combat-no_ui 1539 单测一同验收。
- 2026-05-03：从 skeleton 升 active。§A 概览 + §3.1 P1 截脉·v1 规格落地（10 个决策点闭环 Q60-Q69，4 个 v1 实装时拍板 Q70-Q73）。primary axis = 弹反窗口 + 污染真元中和效率（worldview §五:465）。**v1 模式特殊**：P0 已实装，本 plan 工作集中在 P1 扩展（境界分级 + prep 1s + 触发僵直 + 距离梯度 + 装备耦合 + FOV）。**Q64 距离梯度优雅实现** worldview "防御三流皆克制不了爆脉" — 爆脉常态 reach 0.9 格自然落入 0.3 残效区，不需 `AttackSource::BurstMeridian` 特判。
