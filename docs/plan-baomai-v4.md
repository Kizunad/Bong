# Bong · plan-baomai-v4

**爆脉肉搏 · 疤纹深度扩展**——将体修从"按钮式输出"升级为"战斗史塑角色"的被动成长/战术博弈体系。核心假说：**经脉裂纹不只是负债，也是资产**——过载产生的裂缝在已锻化的肌肉组织中形成次级通路（疤纹回路），长期累积的过载使肉身矿化（活茧），主动断脉可换来污染免疫（死脉甲），破损经脉是天然的探针（裂读），双体修贴身搏杀时裂纹共振会撕裂双方（互噬）。

**定位**：baomai_v3 的纵向延伸——不新增主动攻击技能（v3 五技已覆盖攻防全链路），而是在 v3 的"过载→裂纹→恢复"循环上叠加**被动成长层 + 战术信息层 + PvP 博弈层**。

---

## 阶段总览

| Phase | 内容 | 状态 | 验收日期 |
|-------|------|------|---------|
| P0 | 底盘扩展：ScarHistory + MeridianAdjacency + qi_physics 常数 | ⬜ | — |
| P1 | 疤纹回路：相邻微裂经脉自发形成被动回路 | ⬜ | — |
| P2 | 活茧：累计过载触发四阶肉身被动进化 | ⬜ | — |
| P3 | 死脉甲：主动绝脉技 + SEVERED 经脉污染免疫 | ⬜ | — |
| P4 | 裂读：近战命中时探测对方经脉损伤分布 | ⬜ | — |
| P5 | 互噬：双体修贴身搏杀触发共振锁定 | ⬜ | — |

---

## 世界观锚点

- `worldview.md §四:354-358` — 过载撕裂物理（流量上限 5/s → 强行 20 = 裂缝）——疤纹回路的物理基础
- `worldview.md §五:399-405` — 体修"破产狂战士"定义（所有资源强化肉体经脉韧性）——活茧的叙事根
- `worldview.md §五:466` — 经脉龟裂深度为体修主轴——本 plan 将"深度"从纯负面代价扩展为正负双面
- `worldview.md §三:205-207` — 锻造 tier 0-10（横向 flow_capacity 苦修）——活茧是锻造之外的第二条横向成长路
- `worldview.md §四:293-306` — 体表 ↔ 经脉联动（正经按肢分布）——死脉甲的肢体映射基础
- `worldview.md §四:326-330` — 异体排斥（攻击 = 污染+置换，10:15 交换比亏损）——裂读利用排斥瞬间的信号反馈
- `worldview.md §六:611` — 沉重色（真元浑厚下沉、密度极高）——活茧的真元矿化在沉重色修士身上最显著

---

## 交叉引用（已完成 plan）

- `plan-baomai-v1` ✅ — 崩拳基础 + 过载原型（本 plan 的 ScarHistory 直接承接 MeridianRippleScar）
- `plan-baomai-v2` ✅ — 全力一击 charge/release + Exhausted 状态
- `plan-baomai-v3` ✅ — 五技完整体系 + MeridianRippleScar + OverloadMeridianRippleEvent（本 plan 核心进料）
- `plan-meridian-severed-v1` ✅ — MeridianSeveredPermanent + SeverSource::VoluntarySever + SkillMeridianDependencies::declare()（P3 死脉甲直接复用）
- `plan-qi-physics-v1` ✅ — 守恒律 / QiTransfer / collision / constants（P0 扩展常数、P5 ρ override）
- `plan-combat-no_ui` ✅ — AttackIntent / CombatEvent / StatusEffectKind（P4/P5 新增事件变体）
- `plan-cultivation-v1` ✅ — MeridianSystem / Meridian / integrity / contamination（全 phase 进料）
- `plan-vfx-v1` ✅ — VfxEventRouter / VfxPlayer（P5 共振 VFX）
- `plan-style-vector-integration-v1` ✅ — PracticeLog 染色权重（活茧 stage-up 与沉重色练习量联动检查）

**交叉引用（active）**：
- `plan-dandao-path-v1` ⏳ — 丹体异化是另一条"被动身体改造"路线，与活茧平行但不冲突（丹道改外观+增部位，活茧改内质+增韧性）
- `plan-sword-path-v2` ⏳ — 裂读结果可推断对手剑道技能可用性（依赖经脉映射 LI/SI/TE）

---

## 接入面 Checklist

- **进料**：
  - `combat::baomai_v3::state::MeridianRippleScar` — 现有过载累积（P0 扩展为 ScarHistory）
  - `combat::baomai_v3::events::OverloadMeridianRippleEvent` — 过载事件触发回路/活茧判定
  - `cultivation::components::{MeridianSystem, Meridian, MeridianId}` — 经脉状态读取
  - `cultivation::meridian::severed::{MeridianSeveredPermanent, SeverSource, SkillMeridianDependencies}` — P3 绝脉复用
  - `combat::components::{Wounds, BodyPart, WoundGrade, DerivedAttrs}` — 活茧被动修改战斗属性
  - `combat::events::{CombatEvent, AttackIntent}` — 裂读/互噬的触发入口
  - `qi_physics::constants` — 新增物理常数（**先扩 qi_physics 再 import，不在本模块内自定义**）
  - `qi_physics::collision::qi_collision` — P5 互噬期间 ρ override
- **出料**：
  - `ScarCircuitFormedEvent` / `ScarCircuitBrokenEvent` → agent narration
  - `IronCocoonStageUpEvent` → agent narration + client event_flow
  - `CrackReadingResultEvent` → client HUD overlay（`bong:crack_reading` CustomPayload）
  - `ResonanceLockEvent` / `ResonanceLockEndEvent` → client VFX + HUD meter
  - `VoluntarySeverEvent` → `MeridianSeveredPermanent` 写入 + agent narration
- **共享类型 / event**：
  - 复用 `SeverSource::VoluntarySever`（已定义于 plan-meridian-severed-v1）
  - 复用 `OverloadMeridianRippleEvent`（只订阅，不重新定义）
  - 新增 `StatusEffectKind::ResonanceLocked`（互噬锁定状态）
  - 新增 `AttackSource::VoluntarySever`（绝脉主动技）
- **跨仓库契约**：
  - Server: `baomai_v4::*` 模块（纯 server 逻辑，P0-P3 无 client 依赖）
  - Client: `bong:crack_reading` CustomPayload type ID（P4）+ `bong:resonance_lock` CustomPayload type ID（P5）
  - Agent: `ScarCircuitFormedEvent` / `IronCocoonStageUpEvent` 可选叙事消费（非阻塞）
- **worldview 锚点**：见上"世界观锚点"节
- **qi_physics 锚点**：
  - `qi_physics::constants` — 新增 `SCAR_CIRCUIT_INTEGRITY_MIN` / `_MAX` / `RESONANCE_LOCK_RANGE` / `RESONANCE_RETREAT_INTEGRITY_PENALTY` / `IRON_COCOON_THRESHOLDS`
  - `qi_physics::collision` — P5 互噬期间临时 ρ=0 覆写（调用 `qi_collision` 时传入 `rho_override: Option<f64>`）

---

## §1 P0 — 底盘扩展

### §1.1 ScarHistory 组件

**与 MeridianRippleScar 的关系**：现有 `MeridianRippleScar`（baomai_v3）追踪单次/累计 severity 和 `accumulated_overloads`。`ScarHistory` 是**上层聚合器**——从 `MeridianRippleScar.accumulated_overloads` 读取初始值（首次插入时 migration），之后通过订阅 `OverloadMeridianRippleEvent` 同步递增。**不替代 MeridianRippleScar**（后者仍负责 severity 追踪和经脉 integrity 扣减），只是在其上层追加维度（分桶计数）。

**文件**：`server/src/combat/baomai_v4/scar_history.rs`

```rust
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct ScarHistory {
    /// 累计过载事件次数（只增不减，跨境界/死亡保留）。
    /// 首次插入时从 MeridianRippleScar.accumulated_overloads 同步。
    pub total_overloads: u32,
    /// 按技能分桶计数（崩拳 / 全力 / 撼山 / 焚血 / 散功）
    pub overloads_by_skill: HashMap<BaomaiSkillId, u32>,
    /// 按经脉分桶计数（哪条脉被过载最多）
    pub overloads_by_meridian: HashMap<MeridianId, u32>,
}

impl ScarHistory {
    /// 从现有 MeridianRippleScar 迁移构建
    pub fn from_ripple_scar(scar: &MeridianRippleScar) -> Self {
        Self {
            total_overloads: scar.accumulated_overloads,
            overloads_by_skill: HashMap::new(),
            overloads_by_meridian: HashMap::new(),
        }
    }
}
```

**IronCocoonStage 不存储在 ScarHistory 中**——改为实时计算 `IronCocoonStage::from_overloads(scar_history.total_overloads)`，`iron_cocoon_check_system` 做前后比较以检测阶段跨越。

**系统**：`scar_history_track_system` — 订阅 `OverloadMeridianRippleEvent`，更新 `ScarHistory` 计数器。对无 `ScarHistory` 但有 `MeridianRippleScar` 的 entity，首次插入时调用 `from_ripple_scar()` 迁移。

### §1.2 MeridianAdjacency 常量图

定义经脉间的物理邻接关系（基于 worldview §四:293 正经按肢分布 + 奇经连通）。

**文件**：`server/src/combat/baomai_v4/adjacency.rs`

```rust
/// 经脉邻接对。仅同肢经脉互为邻接（简化模型，不走 TCM 全拓扑）。
pub const MERIDIAN_ADJACENCY: &[(MeridianId, MeridianId)] = &[
    // 左臂·手三阴（气）
    (MeridianId::Lung, MeridianId::Heart),
    (MeridianId::Heart, MeridianId::Pericardium),
    (MeridianId::Lung, MeridianId::Pericardium),
    // 右臂·手三阳（力）
    (MeridianId::LargeIntestine, MeridianId::SmallIntestine),
    (MeridianId::SmallIntestine, MeridianId::TripleEnergizer),
    (MeridianId::LargeIntestine, MeridianId::TripleEnergizer),
    // 左腿·足三阴（韧）
    (MeridianId::Spleen, MeridianId::Kidney),
    (MeridianId::Kidney, MeridianId::Liver),
    (MeridianId::Spleen, MeridianId::Liver),
    // 右腿·足三阳（速）
    (MeridianId::Stomach, MeridianId::Bladder),
    (MeridianId::Bladder, MeridianId::Gallbladder),
    (MeridianId::Stomach, MeridianId::Gallbladder),
    // 任督桥（躯干纵轴，连通上下）
    (MeridianId::Ren, MeridianId::Du),
];

pub fn are_adjacent(a: MeridianId, b: MeridianId) -> bool { ... }
pub fn adjacent_pairs_for(id: MeridianId) -> Vec<MeridianId> { ... }
```

### §1.3 常数定义

常数严格分为两类：涉及真元物理交互的放 `qi_physics/constants.rs`，纯 gameplay 参数放本模块。

**文件**：`server/src/qi_physics/constants.rs`（追加，仅真元/经脉物理相关）

```rust
// ── 疤纹回路（经脉 integrity 物理阈值）──
/// 回路形成所需经脉 integrity 下限（含）
pub const SCAR_CIRCUIT_INTEGRITY_MIN: f64 = 0.50;
/// 回路形成所需经脉 integrity 上限（含）——超过此值回路断开
pub const SCAR_CIRCUIT_INTEGRITY_MAX: f64 = 0.85;

// ── 互噬（真元排斥/经脉损伤物理）──
/// 共振锁定期间 ρ 覆写值（排斥系数降为零）
pub const RESONANCE_LOCK_RHO_OVERRIDE: f64 = 0.0;
/// 锁定期间经脉 integrity 下降速率（per tick，仅已受损经脉）
pub const RESONANCE_LOCK_INTEGRITY_DRAIN: f64 = 0.005;
/// 脱离共振的 integrity 惩罚（per 受损经脉）
pub const RESONANCE_RETREAT_INTEGRITY_PENALTY: f64 = 0.08;
```

**文件**：`server/src/combat/baomai_v4/constants.rs`（新建，gameplay 参数）

```rust
// ── 活茧 ──
pub const IRON_COCOON_THRESHOLDS: [u32; 4] = [50, 120, 250, 500];

// ── 互噬（gameplay 参数）──
pub const RESONANCE_LOCK_RANGE: f64 = 2.0;
pub const RESONANCE_METER_HIT_BASE: f64 = 0.12;
pub const RESONANCE_LOCK_DURATION_TICKS: u64 = 60; // 20 tps × 3s
pub const RESONANCE_SCAR_THRESHOLD: u32 = 40;

// ── 裂读 ──
pub const CRACK_READING_RATE_PER_OVERLOAD: f64 = 0.004;
pub const CRACK_READING_RATE_CAP: f64 = 0.50;
pub const CRACK_READING_DISPLAY_TICKS: u64 = 60;
pub const CRACK_READING_DEEP_WINDOW_TICKS: u64 = 60;

// ── 疤纹回路 ──
pub const SCAR_CIRCUIT_MIN_OVERLOADS: u32 = 5;
pub const SCAR_CIRCUIT_CHECK_INTERVAL_TICKS: u64 = 40;

// ── 死脉甲 ──
pub const VOLUNTARY_SEVER_MIN_OVERLOADS: u32 = 80;
pub const VOLUNTARY_SEVER_MIN_MASTERY: f32 = 60.0;
pub const VOLUNTARY_SEVER_MAX_INTEGRITY: f64 = 0.70;
pub const VOLUNTARY_SEVER_CAST_TICKS: u64 = 100;
pub const VOLUNTARY_SEVER_COMBAT_COOLDOWN_TICKS: u64 = 60;
```

### §1.4 模块骨架

**文件**：`server/src/combat/baomai_v4/mod.rs`

```rust
pub mod adjacency;
pub mod constants;
pub mod crack_reading;
pub mod dead_armor;
pub mod events;
pub mod iron_cocoon;
pub mod resonance_lock;
pub mod scar_circuit;
pub mod scar_history;
#[cfg(test)]
mod tests;
```

Plugin 注册 6 个 event 类型 + 4 个 system（P0 仅 `scar_history_track_system`）。

### §1.5 测试（P0）

- `scar_history_increments_on_overload` — 崩拳触发后 `ScarHistory.total_overloads` +1
- `scar_history_per_skill_bucket` — 不同技能分别计入对应桶
- `scar_history_per_meridian_bucket` — 过载影响的每条经脉各自计入
- `scar_history_survives_death` — 死亡复活后 `ScarHistory` 保留
- `adjacency_same_limb` — 同肢经脉 `are_adjacent` = true
- `adjacency_cross_limb` — 不同肢经脉 `are_adjacent` = false（Ren/Du 除外）
- `adjacency_ren_du` — Ren ↔ Du `are_adjacent` = true
- `adjacency_symmetric` — `are_adjacent(a, b) == are_adjacent(b, a)`
- `qi_physics_constants_exist` — 新增常数可编译访问

---

## §2 P1 — 疤纹回路（Scar Circuit）

### §2.1 物理推导

当两条相邻正经都处于 MICRO_TEAR 状态（integrity ∈ [0.50, 0.85]），裂缝外溢的微量真元在已锻化的肌肉组织中形成压强差驱动的次级通路。这是 worldview §四:354 过载撕裂的自然推论——裂缝不是空洞，是未被封堵的微管道；末法环境的低灵压不足以自动密封它们。

### §2.2 组件

**文件**：`server/src/combat/baomai_v4/scar_circuit.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScarCircuitKind {
    TigerMouth,    // 虎口回路 LI↔SI（右臂）
    TripleYang,    // 三阳合流 SI↔TE（右臂）
    HeartLung,     // 心肺短路 LU↔HT（左臂）
    LiverKidney,   // 肝肾交汇 KI↔LR（左腿）
    GallStomach,   // 胆胃通路 ST↔GB（右腿）
    SpleenKidney,  // 脾肾固本 SP↔KI（左腿）
    RenDuBridge,   // 任督桥 Ren↔Du（躯干）
}

impl ScarCircuitKind {
    /// 组成该回路的两条经脉
    pub fn meridian_pair(&self) -> (MeridianId, MeridianId) { ... }
}

#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct ActiveScarCircuits {
    pub circuits: HashSet<ScarCircuitKind>,
    pub formed_at: HashMap<ScarCircuitKind, u64>, // tick
}
```

### §2.3 形成 / 断裂条件

**形成**（每 40 tick 检查一次，≈ 2s 周期）：

```
for each ScarCircuitKind:
    let (a, b) = kind.meridian_pair()
    let ma = meridian_system.get(a)
    let mb = meridian_system.get(b)
    if ma.opened && mb.opened
       && ma.integrity >= SCAR_CIRCUIT_INTEGRITY_MIN
       && ma.integrity <= SCAR_CIRCUIT_INTEGRITY_MAX
       && mb.integrity >= SCAR_CIRCUIT_INTEGRITY_MIN
       && mb.integrity <= SCAR_CIRCUIT_INTEGRITY_MAX
       && !ma.is_severed() && !mb.is_severed()
       && scar_history.total_overloads >= 5  // 最低门槛，防新手误触
    then:
       circuits.insert(kind)
       emit ScarCircuitFormedEvent
```

**断裂**（同一 40-tick 周期检查）：

```
for each active circuit:
    let (a, b) = circuit.meridian_pair()
    if ma.integrity > SCAR_CIRCUIT_INTEGRITY_MAX  // 治好了
       || ma.integrity < SCAR_CIRCUIT_INTEGRITY_MIN  // 裂得太深
       || mb.integrity > SCAR_CIRCUIT_INTEGRITY_MAX
       || mb.integrity < SCAR_CIRCUIT_INTEGRITY_MIN
       || ma.is_severed() || mb.is_severed()
    then:
       circuits.remove(circuit)
       emit ScarCircuitBrokenEvent
```

### §2.4 被动效果

回路提供的加成写入 `DerivedAttrs`（复用 `body_conditioning_aggregate` 的模式）。

| 回路 | 效果 | 数值 | 平衡理由 |
|------|------|------|---------|
| 虎口回路 | 崩拳命中时，`damage × 0.08` 转为 contamination 注入（不额外扣真元，只是伤害类型分流） | 8% | 不增加总伤害，只改变伤害构成——对重甲目标更有效，对高 ρ 目标更弱 |
| 三阳合流 | 近战 reach +0.3 blocks | +0.3 | 基础 FIST_REACH ≈ 1.5，+0.3 = +20% 距离，可感知但非决定性 |
| 心肺短路 | 焚血激活期间 qi_regen +15% | ×1.15 | 焚血本身是高风险状态（HP 换 qi），加速回蓝缓解风险但不消除 |
| 肝肾交汇 | contamination 自然排毒速率 +10% | ×1.10 | QoL 改善，对抗毒蛊/污染战术的被动缓解 |
| 胆胃通路 | 撼山 AOE 半径 +1 block | +1 | 引气基础 3 格 → 4 格，提升范围 33%；化虚基础 10 → 11，仅 10% |
| 脾肾固本 | 安全区域内经脉自愈速率 +20% | ×1.20 | 纯非战斗恢复加速——鼓励"打完去养伤"的体修循环 |
| 任督桥 | 全力一击充能速率 +25%（但充能被打断时 Ren/Du 各受 integrity -0.03） | +25% / -0.03 | 高风险高回报——充能更快但失败代价更大 |

**实现路径**（不同效果挂不同 hook 点）：

| 回路 | hook 点 | 说明 |
|------|---------|------|
| 虎口回路 | `resolve.rs` 的 `emitted_contam_delta` 赋值后 | 在现有 contamination 结算流程中追加：if TigerMouth active && source == BengQuan, `contam_delta += wound_damage × 0.08`（damage→contam 分流，总伤害不变） |
| 三阳合流 | `player_attack.rs` 的 reach 判定前 | 读 `DerivedAttrs.reach_bonus`（新增字段）添加 +0.3 |
| 心肺短路 | `qi_physics::excretion::regen_from_zone` | 读 `DerivedAttrs.qi_regen_multiplier`（新增字段），BloodBurnActive 期间 ×1.15 |
| 肝肾交汇 | `cultivation::contamination` tick 系统 | 读 `DerivedAttrs.contam_purge_multiplier`（新增字段）×1.10 |
| 胆胃通路 | `baomai_v3::skills::mountain_shake` 的 radius 参数 | 读 `ActiveScarCircuits` 直接 +1 |
| 脾肾固本 | `qi_physics::healing` tick 系统 | 读 `DerivedAttrs.healing_rate_multiplier`（新增字段）×1.20（仅 env qi > 0.3） |
| 任督桥 | `baomai_v3::skills::full_power_charge` 的 rate | 读 `ActiveScarCircuits` 直接 ×1.25 |

**系统**：`scar_circuit_derive_system` 在 `CombatSystemSet::Physics` 阶段运行，读 `ActiveScarCircuits` 写 DerivedAttrs 新增字段（reach_bonus / qi_regen_multiplier / contam_purge_multiplier / healing_rate_multiplier）。虎口/胆胃/任督桥的效果直接在各自技能施放函数中检查 `ActiveScarCircuits`，不走 DerivedAttrs。

**约束**：回路效果不与同类 buff 叠加（如果有其他 reach 加成，取 max 不叠加）。需在 DerivedAttrs 新增字段时设 default = 1.0（乘法中性元素）。

### §2.5 视听（内联）

**narration（回路形成时）**：
- scope: `player`, style: `perception`
- 示例 1: `"你隐约感到右臂大肠经与小肠经之间有一丝微妙的牵引——旧伤处的真元不再只是外溢，而是找到了一条新的通路。"`
- 示例 2: `"左腿肝经与肾经的裂纹似乎在深处连通了。血肉中有什么在蠕动，不痛，只是一种陌生的'通畅'感。"`
- 示例 3（任督桥）: `"任督二脉的旧伤在胸腔深处相互呼应，真元在裂缝间回流。这不是修复——是另一种连接。"`

**narration（回路断裂时）**：
- scope: `player`, style: `perception`
- 示例: `"右臂的虎口回路断了——经脉修复得太好，次级通路被封死了。"`

**HUD（经脉检视画面）**：
- 在现有 MeridianInspectScreen 上追加显示：活跃回路用 `#FFD700`（金色）虚线连接对应经脉节点
- 回路名称以 tooltip 形式显示在连线上

**无战斗时 VFX**——回路是内在被动，无粒子/音效

### §2.6 测试（P1）

- `circuit_forms_when_both_micro_tear` — 双经脉 integrity = 0.70 → 回路形成
- `circuit_not_formed_if_one_intact` — 一条 0.70 另一条 0.90 → 不形成
- `circuit_not_formed_if_one_too_damaged` — 一条 0.70 另一条 0.40 → 不形成
- `circuit_breaks_on_heal_past_max` — 治疗后 integrity > 0.85 → 回路断开
- `circuit_breaks_on_sever` — 经脉 SEVERED → 回路立即断开
- `circuit_passive_tiger_mouth_contam` — 虎口回路激活时崩拳 damage 8% 转 contamination
- `circuit_passive_reach_bonus` — 三阳合流激活时 reach = FIST_REACH + 0.3
- `circuit_passive_no_stack` — 两个 reach 加成取 max
- `circuit_requires_min_overloads` — `scar_history.total_overloads < 5` → 不形成
- `circuit_ren_du_bridge_interrupt_penalty` — 充能被打断时 Ren/Du integrity 各 -0.03
- `circuit_multiple_active` — 可同时激活多条回路（不同肢体）
- `circuit_check_period` — 40 tick 检查一次，中间状态变化不即时反映

---

## §3 P2 — 活茧（Iron Cocoon）

### §3.1 物理推导

体修的沉重色真元密度极高（worldview §六:611），常年过载导致微量真元渗入肌肉、骨膜。末法真元排他性意味着自体真元不被排斥——它们在组织中**矿化沉积**，如同河水矿物质在河岸沉积形成石灰层。这不是主动锻造（那走 forging tier），是被动的物理适应——打出来的，不是练出来的。

### §3.2 组件

**文件**：`server/src/combat/baomai_v4/iron_cocoon.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum IronCocoonStage {
    None,       // 0 — 普通肉身
    ToughSkin,  // 1 — 茧皮（50+ overloads）
    IronBone,   // 2 — 茧骨（120+ overloads）
    DullFlesh,  // 3 — 茧肉（250+ overloads）
    ScarForged, // 4 — 茧灵（500+ overloads）
}

impl IronCocoonStage {
    pub fn from_overloads(n: u32) -> Self {
        match n {
            0..50 => None,
            50..120 => ToughSkin,
            120..250 => IronBone,
            250..500 => DullFlesh,
            _ => ScarForged,
        }
    }
}
```

### §3.3 各阶被动效果

| 阶段 | 门槛 | 被动名 | 效果 | 数值 | 平衡理由 |
|------|------|--------|------|------|---------|
| 茧皮 | 50 | 厚皮 | BRUISE 伤害阈值 ×1.25（更难被打出淤伤） | ×1.25 | BRUISE 是最轻微伤，影响微乎其微——主要是"感知到自己变硬了"的心理反馈 |
| 茧骨 | 120 | 铁骨 | 受击时 FRACTURE 降级为 LACERATION 概率 20% | 20% | 概率制，非保底——降低极端伤害的频率但不消除风险 |
| 茧肉 | 250 | 钝感 | Cut/Pierce 类伤害降一档（LACERATION→ABRASION） | Cut/Pierce only | 仅对利器有效——钝击、灼烧、震荡不受影响。防的是"被砍"不是"被锤" |
| 茧灵 | 500 | 逆生 | 持有活跃疤纹回路的经脉 flow_rate +5%（永久） | +5% | 极晚期微幅加成——仅影响有回路的脉（最多 7 条中的 3-4 条），且回路本身需维护 |

**系统**：`iron_cocoon_check_system` — 订阅 `OverloadMeridianRippleEvent`，检查 `ScarHistory.total_overloads` 是否跨越阶段阈值 → 更新 `ScarHistory.iron_cocoon_stage` + emit `IronCocoonStageUpEvent`。

**系统**：`iron_cocoon_passive_system` — 在 `CombatSystemSet::Physics` 读 `IronCocoonStage` 修改 `DerivedAttrs`。

### §3.4 视听（内联）

**narration（阶段提升时）**：
- scope: `player`, style: `perception`
- 示例（茧皮）: `"你握拳时感到拳面的皮肤不再是柔软的——一层薄薄的硬壳不知何时长了出来，像老树皮。那些旧伤的痕迹，似乎渗进了骨肉深处。"`
- 示例（茧骨）: `"右腿胫骨上挨过的那道裂纹，现在摸上去比别处更硬。你试着用指节敲了敲，声音发闷——里面不是骨头了，更像石头。"`
- 示例（茧灵）: `"旧伤处的经脉在隐隐发热。不是痛，是流动——疤痕组织竟然在传导真元，甚至比原来更快。你开始理解：这些年挨的每一拳，都不是白挨的。"`

**HUD**：
- 无常驻 HUD 变化——活茧是内在被动，不显示额外 UI 元素
- 仅在经脉检视画面（MeridianInspectScreen）的角落标注当前活茧阶段文字（如 `[茧骨]`）
- event_flow 推一条 `"[活茧·茧骨] 肉身韧性再进一步"` 短通知

**音效（阶段提升瞬间）**：
- audio_recipe: `[{ "sound": "block.anvil.land", "pitch": 0.6, "volume": 0.4, "delay_ticks": 0 }, { "sound": "block.stone.break", "pitch": 0.8, "volume": 0.3, "delay_ticks": 5 }]`
- 低沉的铁砧+碎石声，暗示骨肉硬化

### §3.5 测试（P2）

- `cocoon_stage_none_below_threshold` — 49 overloads → `IronCocoonStage::None`
- `cocoon_stage_tough_skin_at_50` — 50 overloads → `ToughSkin`
- `cocoon_stage_iron_bone_at_120` — 120 overloads → `IronBone`
- `cocoon_stage_dull_flesh_at_250` — 250 overloads → `DullFlesh`
- `cocoon_stage_scar_forged_at_500` — 500 overloads → `ScarForged`
- `cocoon_bruise_threshold_at_tough_skin` — ToughSkin 时 BRUISE 阈值 = base × 1.25
- `cocoon_fracture_downgrade_probability` — IronBone 时 FRACTURE→LACERATION 20% 概率 sample（1000 次 ~200 ± 40）
- `cocoon_cut_wound_downgrade` — DullFlesh 时 Cut LACERATION → ABRASION
- `cocoon_blunt_not_downgraded` — DullFlesh 时 Blunt LACERATION 不变（只防 Cut/Pierce）
- `cocoon_scar_forged_flow_rate_bonus` — ScarForged 时有活跃回路的经脉 flow_rate × 1.05
- `cocoon_scar_forged_no_bonus_without_circuit` — 无回路的经脉 flow_rate 不变
- `cocoon_preserves_across_death` — 死亡复活后 IronCocoonStage 保留
- `cocoon_event_emitted_on_stage_up` — 跨越阈值时 emit IronCocoonStageUpEvent

---

## §4 P3 — 死脉甲（Dead Meridian Armor）

### §4.1 物理推导

worldview §四:326 写明攻击本质是"异种真元侵入对方经脉"。SEVERED 经脉是死通道——没有真元流动。异种真元注入死经脉 = 注入惰性组织：不吸收、不反应、不污染。代价：该经脉绑定的所有功法永久丢失（已由 plan-meridian-severed-v1 实装）。

### §4.2 绝脉技（VoluntarySever Skill）

**文件**：`server/src/combat/baomai_v4/dead_armor.rs`

```rust
pub const VOLUNTARY_SEVER_SKILL_ID: &str = "baomai.voluntary_sever";
```

**施放条件**：
- `ScarHistory.total_overloads >= 80`（大量过载经验才懂得"主动断脉"）
- baomai mastery（任一技能）≥ 60
- 目标经脉 ∈ `MeridianId::REGULAR ∪ {Ren, Du}`（共 14 条。其余 6 条奇经无体部映射，排除——见 §8.1 #3）
- 目标经脉 integrity < 0.70（不能断健康经脉——必须已有裂纹）
- 目标经脉 NOT SEVERED
- 非战斗状态（3s 内无 `CombatEvent`）——不是战中应急技，是战前策略选择
- 施放耗时 100 tick（5s 引导，可被打断）

**施放效果**：
1. 目标经脉 `integrity → 0.0`，写入 `MeridianSeveredPermanent { source: SeverSource::VoluntarySever }`
2. 所有依赖该经脉的 `SkillMeridianDependencies` 被拦截（已有机制，不需新代码）
3. 任何经过该经脉的 `ScarCircuit` 立即断裂
4. 该经脉对应的 `BodyPart` 获得 `ContaminationImmunity` 标记

**注册**：`SkillMeridianDependencies::declare(VOLUNTARY_SEVER_SKILL_ID, vec![])` — 绝脉技本身不依赖任何特定经脉（可以断任何一条）。

### §4.3 污染免疫实装

**文件**：`server/src/combat/baomai_v4/dead_armor.rs`

```rust
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct DeadMeridianArmor {
    /// 因主动绝脉获得污染免疫的肢体区域
    pub immune_regions: HashSet<BodyPart>,
}
```

**映射规则**（worldview §四:293 正经按肢分布）：

| SEVERED 经脉 | 免疫区域 |
|-------------|---------|
| LU / HT / PC 任一 | `BodyPart::ArmL`（左臂）|
| LI / SI / TE 任一 | `BodyPart::ArmR`（右臂）|
| SP / KI / LR 任一 | `BodyPart::LegL`（左腿）|
| ST / BL / GB 任一 | `BodyPart::LegR`（右腿）|
| Ren | `BodyPart::Chest`（胸）|
| Du | `BodyPart::Back`（背）|

**系统**：`dead_armor_contam_filter_system` — 在 `resolve.rs` 的 contamination 写入前拦截：如果 `CombatEvent.target_body_part ∈ DeadMeridianArmor.immune_regions`，被拦截的 `contamination_delta` 不写入目标经脉，**改为调用 `qi_release_to_zone(contam_delta, target_position, env)`** 将该部分异种真元散逸到环境中（守恒律：攻方真元已从池中扣除 → 未被目标吸收 → 归还环境，不凭空消失）。

**约束**：
- 仅过滤 contamination，不过滤物理伤害——死脉区域仍然会受切割/钝击/穿刺
- 仅对 `SeveredSource::VoluntarySever` 产生的 SEVERED 经脉生效——战斗中被打断的经脉不自动获得免疫（那是被动断裂，不是主动锻造的死脉）
- 被拦截的异种真元走 `qi_release_to_zone()` 归还环境（遵守 `qi_physics::ledger::QiTransfer` 守恒律）

### §4.4 视听（内联）

**narration（绝脉施放时）**：
- scope: `player`, style: `perception`
- 示例: `"你深吸一口气，真元如灼热的铁水灌入右臂小肠经——不是修复，是烧灼。经脉在痛觉中萎缩、封死。疼痛过后是异样的平静：那条脉死了，但你的右臂从此不怕任何人的真元侵入。"`

**粒子（绝脉瞬间）**：
- 基类: `BongLineParticle`
- 数量: 12
- lifetime: 30 ticks
- 速度: 沿断脉肢体轴向外扩散 0.05 block/tick
- 颜色: `#4A4A4A`（暗灰）→ `#1A1A1A`（黑）渐变
- spawn: burst
- 贴图: 复用 `bong:vfx/meridian_crack`（已有）

**音效**：
- audio_recipe: `[{ "sound": "block.glass.break", "pitch": 0.4, "volume": 0.6, "delay_ticks": 0 }, { "sound": "entity.wither.spawn", "pitch": 2.0, "volume": 0.2, "delay_ticks": 10 }]`

**HUD（持久）**：
- 经脉检视画面中，VoluntarySever 的经脉节点用 `#333333`（深灰）填充 + 白色骷髅小图标
- 与战斗 SEVERED（红色×标记）视觉区分

### §4.5 测试（P3）

- `voluntary_sever_requires_overloads` — `total_overloads < 80` → `CastRejectReason::InRecovery`（复用 InRecovery 表示"身体条件不满足"，语义上合理——没有足够的过载经历来"懂得"断脉）
- `voluntary_sever_requires_mastery` — baomai mastery < 60 → `CastRejectReason::InRecovery`
- `voluntary_sever_requires_damaged_meridian` — integrity > 0.70 → reject
- `voluntary_sever_requires_out_of_combat` — 3s 内有 CombatEvent → reject
- `voluntary_sever_marks_permanent` — 施放后 `MeridianSeveredPermanent` 含目标经脉
- `voluntary_sever_grants_contam_immunity` — 施放后 `DeadMeridianArmor.immune_regions` 含对应肢体
- `voluntary_sever_breaks_circuit` — 经过该经脉的活跃回路断裂
- `voluntary_sever_blocks_dependent_skills` — 依赖该经脉的技能 → CastRejectReason::MeridianSevered
- `dead_armor_blocks_contamination` — 免疫区域 contamination_delta = 0
- `dead_armor_allows_physical_damage` — 免疫区域 wound_grade 正常
- `dead_armor_only_voluntary` — 战斗 SEVERED 的经脉不获得免疫
- `voluntary_sever_interruptible` — 引导中被攻击 → 取消

---

## §5 P4 — 裂读（Crack Reading）

### §5.1 物理推导

体修 ρ=0.65（七流中最高排斥系数）意味着对异种真元极度敏感。近战接触时微量真元交换是物理必然——worldview §四:326 的"侵染"即使在轻微触碰时也会发生。体修的高 ρ 让这些微弱信号不被排斥反应淹没——反而像雷达回波一样被经脉网络接收。裂纹越多的经脉，表面积越大，接收灵敏度越高。

### §5.2 组件

**文件**：`server/src/combat/baomai_v4/crack_reading.rs`

```rust
#[derive(Component, Debug, Clone)]
pub struct CrackReadingState {
    /// 上次成功裂读的 tick（用于深读窗口判定）
    pub last_read_tick: Option<u64>,
    /// 上次裂读的目标
    pub last_read_target: Option<Entity>,
}

/// 裂读结果——发送给 client 渲染
#[derive(Debug, Clone, Serialize)]
pub struct CrackReadingResult {
    pub target: Entity,
    pub meridian_states: Vec<MeridianReadEntry>,
    pub is_deep_read: bool,
    pub display_until_tick: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeridianReadEntry {
    pub id: MeridianId,
    pub integrity_bracket: IntegrityBracket, // Intact/MicroTear/Torn/Severed（不给精确数值）
    pub has_circuit: bool,         // 深读才显示
    pub is_dead_armor: bool,       // 深读才显示
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum IntegrityBracket {
    Intact,    // 0.85+
    MicroTear, // 0.50-0.85
    Torn,      // 0.01-0.50
    Severed,   // 0.0
}

/// NPC 目标使用降级 3 档精度（§8.1 #4）
#[derive(Debug, Clone, Copy, Serialize)]
pub enum NpcIntegrityBracket {
    Intact,    // 0.85+
    Damaged,   // 0.01-0.85（合并 MicroTear + Torn）
    Severed,   // 0.0
}
```

### §5.3 判定流程

**触发**：`CombatEvent` 结算后，若 attacker 持有 `ScarHistory` 且攻击为近战命中：

```
success_rate = min(
    scar_history.total_overloads as f64 * CRACK_READING_RATE_PER_OVERLOAD,
    CRACK_READING_RATE_CAP
)
// 50 overloads → 20%，125 overloads → 50% cap

if rand() < success_rate:
    shallow_read → 显示所有 20 条经脉的 IntegrityBracket
    
    if last_read_target == target
       && (tick - last_read_tick) < CRACK_READING_DEEP_WINDOW_TICKS:
        deep_read → 额外显示 has_circuit + is_dead_armor
    
    emit CrackReadingResultEvent
    update CrackReadingState
```

**信息精度**：
- **玩家目标**：四档区间（Intact / MicroTear / Torn / Severed）
- **NPC 目标**：三档区间（Intact / Damaged / Severed），合并 MicroTear + Torn 为 Damaged（NPC 经脉模型精度不足以区分微裂与重裂——见 §8.1 #4）
- NPC 目标的深读字段 `has_circuit` / `is_dead_armor` 始终返回 `false`（NPC 无 `ActiveScarCircuits` / `DeadMeridianArmor` 组件）

### §5.4 视听（内联）

**HUD（裂读结果）**：
- HudRenderLayer: `CrackReadingOverlay`（新增，Z 优先级高于战斗 HUD，低于菜单）
- 布局: 目标身体轮廓（复用 MeridianInspectScreen 的迷你人形图），经脉按 IntegrityBracket 着色：
  - Intact: `#4CAF50`（绿）
  - MicroTear: `#FFC107`（琥珀）
  - Torn: `#FF5722`（红橙）
  - Severed: `#424242`（灰）
- 持续: `CRACK_READING_DISPLAY_TICKS`（60 tick = 3s），fade out 最后 10 tick
- 深读额外显示: 活跃回路用金色虚线、死脉甲用骷髅图标

**音效（裂读成功时）**：
- audio_recipe: `[{ "sound": "entity.experience_orb.pickup", "pitch": 1.8, "volume": 0.15, "delay_ticks": 0 }]`
- 极轻微的"叮"一声——不应干扰战斗节奏

**narration（首次裂读成功时，仅首次）**：
- scope: `player`, style: `perception`
- 示例: `"你的拳头触到他胸口的瞬间，一阵异样的酸麻从指尖传回——不是他的真元在排斥你，而是你自己的裂纹在'读'他。那些旧伤居然像触手一样，摸到了他经脉里的每一道暗伤。"`

### §5.5 Client 端

**CustomPayload**: `bong:crack_reading`

```json
{
  "target_entity_id": 12345,
  "entries": [
    { "meridian": "LargeIntestine", "bracket": "Torn", "has_circuit": false, "is_dead_armor": false },
    ...
  ],
  "is_deep": false,
  "display_ticks": 60
}
```

**Java 端**：新建 `CrackReadingOverlayRenderer` 类，在 `HudRenderLayer` 中注册，读 payload 渲染叠加图。

### §5.6 测试（P4）

- `crack_reading_probability_scales` — 50 overloads → ~20% / 125+ → 50% cap
- `crack_reading_requires_melee_hit` — 远程攻击不触发
- `crack_reading_shows_integrity_brackets` — 每条经脉返回正确的 IntegrityBracket
- `crack_reading_deep_on_consecutive` — 3s 内第二次命中同一目标 → `is_deep_read = true`
- `crack_reading_deep_shows_circuits` — 深读结果含 `has_circuit` 信息
- `crack_reading_deep_shows_dead_armor` — 深读结果含 `is_dead_armor` 信息
- `crack_reading_resets_on_target_change` — 换目标后深读窗口重置
- `crack_reading_no_exact_values` — 结果不含精确 integrity 数值
- `crack_reading_client_payload_format` — CustomPayload 序列化格式正确

---

## §6 P5 — 互噬（Resonance Lock）

### §6.1 物理推导

两个重度疤纹体修贴身搏杀时，双方裂纹经脉的真元振动频率在物理接触中产生干涉。worldview §四:354 描述的过载裂纹会在真元通过时"震颤"——这是微观尺度的压力波。当两套震颤系统通过肢体接触耦合，如果频率接近（即疤纹模式相似——都是体修），产生**共振放大**。共振使裂纹扩展速率急剧上升，形成双向的"互噬"——你碎我也碎。

### §6.2 组件

**文件**：`server/src/combat/baomai_v4/resonance_lock.rs`

```rust
#[derive(Component, Debug, Clone)]
pub struct ResonanceMeter {
    /// 当前计量 0.0..=1.0
    pub value: f64,
    /// 共振对手
    pub partner: Option<Entity>,
    /// 上次计量增长 tick
    pub last_hit_tick: u64,
}

#[derive(Component, Debug, Clone)]
pub struct ResonanceLocked {
    pub partner: Entity,
    pub started_at: u64,
    pub ends_at: u64,
}
```

### §6.3 触发条件

**前置**：双方都持有 `ScarHistory` 且 `total_overloads >= 40`。

**计量增长**：当 A 近战命中 B 且 B 也在最近 20 tick 内近战命中了 A（双向交战）：

```
let similarity = min(a.total_overloads, b.total_overloads) as f64
                / max(a.total_overloads, b.total_overloads) as f64;
let increment = RESONANCE_METER_HIT_BASE * similarity;
// 双方 scar 对称时 increment = 0.12，差距大时递减
// 例：A=200 B=50 → similarity=0.25 → increment=0.03（几乎不共振）

meter_a.value += increment;
meter_b.value += increment;
```

**衰减**：如果双方 5s 内没有互相命中，meter 以 0.02/tick 衰减至 0。

**触发**：任一方 meter ≥ 1.0 → 双方进入 `ResonanceLocked`。

### §6.4 锁定效果（3s 窗口）

1. **双向经脉碎裂**：双方所有 integrity < `SCAR_CIRCUIT_INTEGRITY_MAX`（即已受损）的经脉，每 tick integrity -= `RESONANCE_LOCK_INTEGRITY_DRAIN`（0.005）
2. **ρ 覆写**：双方之间的 `qi_collision` 调用时 `rho_override = Some(0.0)` — 排斥系数降为零，异种真元完全灌入
3. **经脉级联**：锁定期间任一方经脉 SEVERED → 对方**同一 MeridianId 的经脉** integrity -= 0.5 × 该经脉 SEVERED 前的剩余 integrity（不直接 SEVER 对方，但重伤。例：A 的 LI SEVERED → B 的 LI integrity 减半）
4. **状态效果**：双方获得 `StatusEffectKind::ResonanceLocked` — 移速 -30%，不可闪步（Dash 禁用）

### §6.5 脱离机制

- **距离脱离**：任一方移出 `RESONANCE_LOCK_RANGE`（2 格）→ 锁定立即结束
- **脱离惩罚**：先脱离的一方，所有 `integrity < SCAR_CIRCUIT_INTEGRITY_MAX` 的经脉 integrity -= `RESONANCE_RETREAT_INTEGRITY_PENALTY`（0.08）
- **自然到期**：`RESONANCE_LOCK_DURATION_TICKS`（60 tick = 3s）后自动结束，无惩罚
- **一方死亡**：锁定立即结束，无惩罚

### §6.6 数值平衡分析

| 场景 | 计算 | 结论 |
|------|------|------|
| 对称体修（双方 100 overloads）| similarity=1.0, increment=0.12, 需 9 次互中 ≈ 4-5s 对攻 | 合理——需要持续交战才能触发 |
| 不对称（200 vs 50）| similarity=0.25, increment=0.03, 需 34 次互中 | 实际不可能触发——保护低经验体修 |
| 锁定期间经脉损耗 | 60 tick × 0.005 = 0.30 per meridian | 从 MICRO_TEAR 推向 TORN 但不直接 SEVER——严重恶化，战后需长期休养 |
| 脱离惩罚 | 0.08 per 受损经脉 | 约等于 1-2 次崩拳的过载，痛但可恢复 |

### §6.7 视听（内联）

**HUD（共振计量条）**：
- HudRenderLayer: `ResonanceMeterOverlay`
- 位置: 屏幕下方中央，血条上方
- 样式: 双向填充条（从中间向两端填充），底色 `#1A1A1A`，填充色 `#FF4444`（红）→ `#FFD700`（金，满时）
- 触发显示条件: meter > 0.1 且 partner != None
- 锁定时: 条变为 `#FFD700` 常亮 + 边框脉冲闪烁

**粒子（锁定期间）**：
- 基类: `BongRibbonParticle`
- 数量: 8（双方各 4 条丝带连线）
- lifetime: 持续至锁定结束
- 速度: 0（固定连线，两端点跟踪双方位置）
- 颜色: `#FF4444` 到 `#FFD700` 渐变，随 tick 脉冲亮度（opacity 0.4-0.8 正弦振荡，周期 10 tick）
- spawn: continuous
- 贴图 ID: `bong:vfx/resonance_ribbon`（新增）
- VfxPlayer: `ResonanceLockVfxPlayer`

**粒子（脱离瞬间）**：
- 基类: `BongSpriteParticle`
- 数量: 20
- lifetime: 15 ticks
- 速度: 从脱离者向外爆发 0.15 block/tick，随机方向
- 颜色: `#FF4444`
- spawn: burst
- 贴图: 复用 `bong:vfx/meridian_crack`

**音效（锁定触发时）**：
- audio_recipe: `[{ "sound": "entity.warden.heartbeat", "pitch": 1.2, "volume": 0.5, "delay_ticks": 0 }, { "sound": "block.respawn_anchor.charge", "pitch": 0.6, "volume": 0.4, "delay_ticks": 5 }]`

**音效（锁定持续中，循环）**：
- audio_recipe: `[{ "sound": "entity.warden.heartbeat", "pitch": 1.4, "volume": 0.3, "delay_ticks": 0 }]` — 每 20 tick 播放一次

**音效（脱离时）**：
- audio_recipe: `[{ "sound": "block.glass.break", "pitch": 0.5, "volume": 0.6, "delay_ticks": 0 }]`

**narration（首次触发互噬时，仅首次）**：
- scope: `zone`（附近修士能感知到共振），style: `perception`
- 示例 1: `"空气在两人之间猛然凝滞——不是风停了，是他们的经脉在共振。真元的频率穿过肌肤彼此纠缠，像两头困兽咬在一起撕不开。"`
- 示例 2: `"两个体修的裂纹在同一个频率上颤抖。每一次碰撞都让双方的经脉更加动摇——这不是打斗，是互相毁灭。"`

### §6.8 测试（P5）

- `resonance_requires_both_high_scar` — 一方 total_overloads < 40 → meter 不增长
- `resonance_meter_scales_with_similarity` — 对称时 increment=0.12，差距大时递减
- `resonance_meter_requires_mutual_hits` — 单方面打不涨 meter
- `resonance_meter_decays_without_combat` — 5s 无互中 → meter 归零
- `resonance_lock_triggers_at_full` — meter ≥ 1.0 → 双方进入 ResonanceLocked
- `resonance_lock_drains_integrity` — 锁定中每 tick damaged 经脉 integrity -= 0.005
- `resonance_lock_rho_zero` — 锁定中 qi_collision rho_override = 0.0
- `resonance_lock_cascade_on_sever` — 一方 SEVERED → 对方对应经脉 integrity 大幅下降
- `resonance_lock_move_speed_penalty` — 锁定中移速 -30%
- `resonance_retreat_penalty` — 先离开 2 格 → 该方受 -0.08 integrity
- `resonance_natural_expiry` — 60 tick 后自动结束，无惩罚
- `resonance_death_ends_lock` — 一方死亡 → 立即结束
- `resonance_intact_meridians_unaffected` — 锁定中 integrity ≥ 0.85 的经脉不受 drain

---

## §7 事件类型汇总

**文件**：`server/src/combat/baomai_v4/events.rs`

```rust
pub struct ScarCircuitFormedEvent {
    pub entity: Entity,
    pub circuit: ScarCircuitKind,
    pub tick: u64,
}

pub struct ScarCircuitBrokenEvent {
    pub entity: Entity,
    pub circuit: ScarCircuitKind,
    pub reason: CircuitBreakReason, // Healed / Deepened / Severed
    pub tick: u64,
}

pub struct IronCocoonStageUpEvent {
    pub entity: Entity,
    pub from: IronCocoonStage,
    pub to: IronCocoonStage,
    pub total_overloads: u32,
    pub tick: u64,
}

pub struct CrackReadingResultEvent {
    pub reader: Entity,
    pub target: Entity,
    pub result: CrackReadingResult,
    pub tick: u64,
}

pub struct VoluntarySeverEvent {
    pub entity: Entity,
    pub meridian: MeridianId,
    pub tick: u64,
}

pub struct ResonanceLockEvent {
    pub fighter_a: Entity,
    pub fighter_b: Entity,
    pub started_at: u64,
    pub ends_at: u64,
}

pub struct ResonanceLockEndEvent {
    pub fighter_a: Entity,
    pub fighter_b: Entity,
    pub reason: LockEndReason, // Expired / Retreated(Entity) / Death(Entity)
    pub tick: u64,
}
```

---

## §8 开放问题（P0 决策门前需收口）

### #1 疤纹回路上限

当前设计允许同时激活最多 7 条回路（所有 ScarCircuitKind）。是否需要设上限（如最多 4 条），避免全身微裂的"甜蜜区间维护"成为最优策略？

**权衡**：
- 不设上限：理论上限 7，但实际中同时维持 7 条经脉对在 0.50-0.85 区间极难——战斗/恢复都会打破平衡。自然约束够强。
- 设上限 4：更保守，但可能让"选哪 4 条回路"成为元游戏，偏离"自然生长"的设计理念。

### #2 活茧是否影响跨境界平衡

茧肉（Cut/Pierce 降一档）在高境界 vs 低境界对决中是否过强？低境界的利器攻击被降档后，可能进一步加大跨境界差距。

**权衡**：
- 当前设计：降档是绝对效果，不看境界差——可能需要改为"仅对同境界或高境界攻击生效"。
- 或者保持绝对效果但依赖"250 overloads 门槛极高"作为平衡——到 250 overloads 时玩家已是老手，多数对局在同境界。

### #3 死脉甲是否限定经脉类型

当前允许断任何经脉。但断奇经（Ren/Du/Chong/Dai/YinQiao/YangQiao/YinWei/YangWei）的代价/收益不明确——奇经不按肢分布，免疫哪个区域？

**权衡**：
- 仅允许断正经（12 条）：清晰，每条对应一个肢体区域。
- 允许断奇经：需要定义奇经的免疫映射。Ren→胸、Du→背比较自然，其他奇经（冲/带/阴跷/阳跷/阴维/阳维）的身体映射不直观。
- 建议：P3 v1 仅支持正经 + Ren/Du，其余奇经后续扩展。

### #4 裂读对 NPC 是否生效

裂读是否能读取 NPC（如 Heiwushi BOSS）的经脉状态？

**权衡**：
- 允许：增加 PvE 策略深度——先试探 BOSS 弱点再决定攻击方向。
- 限制：仅对玩家生效——NPC 的经脉可能是简化模型，裂读结果不可靠。
- 建议：允许，但 NPC 的 IntegrityBracket 精度降一级（只显示 Intact/Damaged/Severed 三档而非四档）。

### #5 互噬的 PvE 价值

互噬仅在双体修 PvP 中触发。如果服务器体修比例低，P5 的整个系统可能闲置。是否让体修 vs 特定 NPC（如重度过载的 Heiwushi）也能触发？

**权衡**：
- 仅 PvP：更纯粹，但可能投入产出比低。
- PvP + 特定 NPC：增加 PvE 用途，但需要给 NPC 配 ScarHistory（增加 NPC 数据复杂度）。
- 建议：P5 v1 仅 PvP。后续通过 NPC plan 扩展。

### #6 战斗中被 SEVERED 的经脉能否后续"认领"为死脉甲

当前绝脉技要求目标经脉 NOT SEVERED。但如果一条经脉已在战斗中被打断（`SeveredSource::OverloadTear` / `CombatWound`），玩家能否事后"认领"它为死脉甲（获得污染免疫）？

**权衡**：
- 不允许：保持 VoluntarySever 的"主动选择"叙事纯度——死脉甲是你深思熟虑的决定，不是"反正已经断了不如捡个好处"。
- 允许但加门槛：例如需要 100 overloads + 特殊药材 + 10s 引导（"锻化死脉"过程），区分于战中意外断裂。
- 建议：v1 不允许。如果玩家反馈强烈，v2 用"锻化死脉"引导技扩展。

全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

---

## §8.1 决议（pre-P0 收口，2026-05-18）

### #1 疤纹回路上限

**决议**：
1. 不设上限，保持 7 条回路全部可同时激活。
2. 自然约束已足够强：每条回路要求两条经脉同时在 [0.50, 0.85] 区间。战斗中单次崩拳过载 = -0.005 integrity（`overload.rs:63` severity × 0.1），安全区回复 = 0.005/tick × zone_qi（`heal.rs:23`）。实战中持续过载 + 恢复窗口不足，同时维持 7 条经脉在甜蜜区实际不可能——预期实战峰值 2-4 条，7 条仅休养状态可达。
3. 拒绝设上限 4 的理由："选哪 4 条"会成为元游戏，偏离"战斗塑造角色"的被动生长理念。如果实测发现 4+ 回路过于频繁，v2 可改为"每条回路有 120 tick 冷却期后才能重新形成"而非硬上限。

**落点**：`server/src/cultivation/heal.rs:15-27`（回复速率参照）/ `server/src/combat/baomai_v3/physics.rs:128-141`（过载 severity 参照）/ plan §2.3（形成/断裂条件不变）

### #2 活茧是否影响跨境界平衡

**决议**：
1. v1 保持绝对效果，不加境界差检查。
2. 理由：250 overloads 门槛 ≈ 150 分钟纯战斗时间（§9.2），到达时玩家已是凝脉/固元阶段老手，多数对局在同境界或相邻境界。跨境界碾压本身已由 `realm_gap.rs:7-19` 的 REALM_GAP_MATRIX 决定（高境界对低境界 15× 倍率，仅用于全力一击），DullFlesh 降一档（LACERATION→ABRASION）不改变碾压局的结果。
3. 当前 realm_gap 只应用于 `FullPowerCharge/Release`（`realm_gap.rs:2-3` 注释），普通近战无境界倍率——DullFlesh 降档影响的是同级对决的攻防博弈，不是跨级碾压场景。
4. 如果实测出现低境界 DullFlesh 体修被高境界利器攻击者反馈"砍不动"的问题，v2 方案：`if realm_gap(attacker, defender) > 2 → 跳过 DullFlesh 降档`。

**落点**：`server/src/combat/realm_gap.rs:7-19`（REALM_GAP_MATRIX 参照）/ `server/src/movement/leg_wound.rs:51-65`（WoundGrade 阈值参照）/ plan §3.3（DullFlesh 效果不变）

### #3 死脉甲是否限定经脉类型

**决议**：
1. P3 v1 仅支持 12 正经 + Ren + Du（共 14 条），其余 6 条奇经（Chong/Dai/YinQiao/YangQiao/YinWei/YangWei）排除。
2. 理由：代码中仅 Ren→Head（`dugu.rs:479` 的 `body_part_to_meridian` 中 Du→Head 的映射存在）和 plan §4.3 定义的 Ren→Chest、Du→Back 有体部映射。其余 6 条奇经在代码和 worldview 中均无身体区域对应关系（grep 确认零匹配），强行定义映射缺乏 worldview 锚点。
3. 施放条件追加：`voluntary_sever` 的 `target_meridian` 必须 ∈ `MeridianId::REGULAR ∪ {Ren, Du}`，否则 reject。
4. 后续扩展路径：如果 worldview 或 cultivation plan 补充了奇经的身体通道定义，v2 可逐条开放。

**落点**：`server/src/cultivation/dugu.rs:479-485`（现有映射参照）/ `server/src/cultivation/components.rs:66-73`（奇经枚举参照）/ plan §4.2 施放条件需追加奇经过滤

### #4 裂读对 NPC 是否生效

**决议**：
1. 允许裂读对 NPC 生效，但降级为 3 档精度：`Intact` / `Damaged` / `Severed`（合并 MicroTear + Torn 为 Damaged）。
2. 理由：NPC 已有 `MeridianSystem` 组件（`npc/lifecycle.rs:420-422` 的 `NpcRuntimeBundle` 包含 `meridian_system: MeridianSystem`）。裂读只读 `Meridian.integrity`，不依赖 `KnownTechniques`（NPC 无此组件），技术上零额外成本。
3. 深读字段处理：NPC 无 `ActiveScarCircuits` / `DeadMeridianArmor` 组件 → `has_circuit` 和 `is_dead_armor` 对 NPC 目标始终返回 `false`。深读仍可触发（连续命中同一 NPC），但额外信息为空。
4. 实现：`crack_reading.rs` 的 `build_reading_result()` 检查 target 是否有 `ActiveScarCircuits` / `DeadMeridianArmor` 组件（`Option<&ActiveScarCircuits>`），无则默认 false。IntegrityBracket 映射追加 `fn bracket_for_npc(integrity: f64) -> NpcIntegrityBracket` 使用 3 档。

**落点**：`server/src/npc/lifecycle.rs:417-432`（NpcRuntimeBundle 参照）/ plan §5.2 `MeridianReadEntry` 需追加 NPC 3 档说明 / plan §5.3 判定流程需追加 NPC 分支

### #5 互噬的 PvE 价值

**决议**：
1. P5 v1 仅 PvP。不给 NPC 添加 `ScarHistory`。
2. 理由：技术上在 `NpcRuntimeBundle` 加一行 `scar_history: ScarHistory::default()` 即可（NPCs 已有完整战斗+修炼组件栈，`npc/lifecycle.rs:455-456`），但设计上需要回答"哪些 NPC 类型有 ScarHistory"和"初始 total_overloads 如何 seed"——这些决策属于 NPC update plan 的范畴。
3. 共振锁定核心循环（`Query<..., With<ScarHistory>>`）不区分 player/NPC entity，未来开放 PvE 共振无需改核心逻辑，只需在 NPC plan 中给特定 NPC 类型（如 Heiwushi BOSS）添加 `ScarHistory` 并 seed `total_overloads`。
4. 拒绝"P5 v1 就加 NPC"的理由：共振对 NPC 的 AI 行为影响（移速 -30%、Dash 禁用、脱离惩罚）需要 big-brain scorer 调整（`npc/brain.rs` 的 `DashAction` / `MeleeRangeScorer`），不在本 plan 范围内。

**落点**：`server/src/npc/lifecycle.rs:439-460`（NpcRuntimeBundle 参照）/ `server/src/npc/brain.rs:133`（MeleeAttackAction 参照）/ plan §6.3 触发条件不变

### #6 战斗中被 SEVERED 的经脉能否后续"认领"为死脉甲

**决议**：
1. v1 不允许。战斗中被打断的经脉（`SeveredSource::OverloadTear` / `CombatWound` / `BackfireOverload` / `TribulationFail` / `DuguDistortion`）不可事后转换为死脉甲。
2. 理由（叙事）：死脉甲的核心叙事是"主动牺牲"——你深思熟虑后选择断掉一条经脉换取污染免疫。被动断裂是"事故"，不应能捡到好处。
3. 理由（技术）：虽然 `SeveredSource` 可通过 `MeridianSeveredPermanent.record_for(id).source` 查询（`severed.rs:32-44`），且实现"认领"仅需 ~50 行代码，但存在状态一致性风险：
   - ScarCircuit 在经脉 SEVERED 时已断裂（§2.3），"认领"后是否重建？答案不明确
   - SkillMeridianDependencies 的 `check_meridian_dependencies`（`severed.rs:134-146`）不区分 source，"认领"不改变技能禁用状态——用户可能误以为"认领了就能恢复技能"
4. v2 扩展路径（如果玩家反馈需要）：新增 `claim_dead_meridian` 引导技，门槛 100 overloads + 10s 引导 + 非战斗状态，将 `SeveredSource` 覆写为 `VoluntarySever` 并写入 `DeadMeridianArmor`。明确告知"技能仍然不可用，只获得污染免疫"。

**落点**：`server/src/cultivation/meridian/severed.rs:46-63`（SeveredSource 枚举参照）/ `server/src/cultivation/meridian/severed.rs:32-44`（SeveredRecord 结构参照）/ plan §4.2 施放条件"NOT SEVERED"不变

---

## §9 数值平衡备忘

### §9.1 回路效果天花板

全部 7 回路同时激活时的总增益：

| 维度 | 累计增益 | 对比基线 | 评估 |
|------|---------|---------|------|
| 污染转换 | 崩拳 8% → contam | 总伤害不变，只改类型 | 安全 |
| Reach | +0.3 blocks | FIST_REACH 1.5 → 1.8 (+20%) | 可感知，非压倒性 |
| 焚血 qi regen | ×1.15 | 仅焚血期间有效 | 受限场景，安全 |
| 排毒 | ×1.10 | 被动 QoL | 安全 |
| 撼山半径 | +1 block | 引气 3→4, 化虚 10→11 | 低境界影响大，高境界可忽略 |
| 安全区自愈 | ×1.20 | 非战斗恢复 | 安全 |
| 全力充能 | +25% (附带风险) | 有惩罚条款 | 风险平衡 |

**结论**：全开天花板下总增益约等于"一件中等品质护甲的防御提升"——显著但不决定胜负。

### §9.2 活茧的时间投资

| 阶段 | 门槛 | 假设每场 5 次过载 | 所需场次 | 所需时间（每场 ~3 min） |
|------|------|-----------------|---------|----------------------|
| 茧皮 | 50 | 10 场 | 10 | ~30 min |
| 茧骨 | 120 | 24 场 | 24 | ~72 min |
| 茧肉 | 250 | 50 场 | 50 | ~150 min |
| 茧灵 | 500 | 100 场 | 100 | ~300 min (5h) |

**结论**：茧灵需要 ~5h 纯战斗时间（不含恢复/旅行），对应 worldview 凝脉→固元的跨度。节奏合理。

### §9.3 互噬伤害预算

3s 锁定中双方各损失（假设 6 条受损经脉）：
- 每条经脉 integrity 损失: 60 tick × 0.005 = 0.30
- 一条 integrity=0.70 的经脉 → 0.40（从 MICRO_TEAR 进入 TORN 边缘）
- 一条 integrity=0.50 的经脉 → 0.20（深 TORN）
- **不会直接 SEVER**（从 MICRO_TEAR 起步需 0.85→0.0 = 170 tick，远超 60 tick 窗口），但会把所有受损经脉推向 TORN
- 加上 ρ=0 导致的异种真元零阻碍灌入，综合效果：**严重恶化但非必死**——双方战后都需要长时间休养
- 对比参照：一次崩拳过载 severity=0.05——60 tick 的共振 ≈ 6 次崩拳对每条经脉的过载当量

---

## §10 消费本 plan 的工作流约束（consume-plan agent 必读）

> 本 plan 全部是 Rust/Java 逻辑代码 + 少量 VFX 资产，**无 NBT 建筑 / worldgen layout**——不适用 docs/CLAUDE.md §6.1 建筑多轮打磨规则。纯代码 TODO 按 commands/consume-plan.md 通用的 atomic commit + 测试全绿即可。

### §10.1 PR 拆分序列（依赖顺序，前一个 merge 后开下一个）

| PR | 范围 | 关键交付物 | 预估测试数 | 依赖 |
|----|------|----------|----------|------|
| PR-1 | P0 底盘 + P1 疤纹回路 + P2 活茧 | `baomai_v4/` 模块骨架 + `ScarHistory` + `MeridianAdjacency` + `ActiveScarCircuits` + `IronCocoonStage` + qi_physics 常数扩展 + DerivedAttrs 新增字段 + 全部 P0/P1/P2 测试 | ~35 | 无 |
| PR-2 | P3 死脉甲 | `dead_armor.rs` + `VoluntarySever` skill 注册 + `DeadMeridianArmor` + contamination 拦截 + qi_release_to_zone 守恒 + 全部 P3 测试 | ~12 | PR-1 |
| PR-3 | P4 裂读 | `crack_reading.rs` + `CrackReadingResult` + `IntegrityBracket` + `bong:crack_reading` CustomPayload + Java `CrackReadingOverlayRenderer` + 全部 P4 测试 | ~9 | PR-1 |
| PR-4 | P5 互噬 | `resonance_lock.rs` + `ResonanceMeter` + `ResonanceLocked` + ρ override hook + `bong:resonance_lock` CustomPayload + `ResonanceLockVfxPlayer` + `ResonanceMeterOverlay` HUD + `bong:vfx/resonance_ribbon` 贴图 + 全部 P5 测试 | ~13 | PR-1 |

- PR-2/3/4 之间无依赖，PR-1 merge 后可按序提交
- **多 PR 仍属于同一次 `/consume-plan` 调用**——不退回让用户重跑
- **PR 依赖处理**：前序 PR merge 前不开后续 PR；前序卡住 → 走通用 step 7 / step 4.2 处理；处理不了 → 停交人工，已 land 的 PR 保留不回退

### §10.2 CodeRabbit Review 等待协议（ScheduleWakeup 驱动）

CodeRabbit 是 PR 自动 review bot，以 GitHub Actions check run 形式呈现。

#### 状态判定

`gh pr checks <PR_NUM> --json name,status,conclusion` 查 CodeRabbit check：

| 状态 | 含义 | 动作 |
|------|------|------|
| `pass` (conclusion: success) | review 通过 | 进入 step 7 评审意见处理 / step 8 merge |
| `pending` (status: in_progress / queued) | 仍在 review | **等下一回合**（ScheduleWakeup） |
| `fail` (conclusion: failure) | review 不通过 | 按 step 7 严重性桶处理修复 |

#### 等待节奏

**禁止 sleep 循环 / busy poll**。每回合用 `ScheduleWakeup`：

- 首次提 PR 后 → `ScheduleWakeup delaySeconds=1200`（20 min），reason="等 CodeRabbit review pass，PR #\<num\>"
- 醒来 → `gh pr checks <PR_NUM>` 查状态
- 若 `pending` → 再 `ScheduleWakeup delaySeconds=1200`，最多 3 回合 = 总 60 min
- 3 回合（60 min）仍 `pending` → 停交人工，输出 PR URL + "CodeRabbit 卡死 60+ min"
- `pass` / `fail` → 退出等待，进 step 7

#### 必须等 APPROVED 才 merge

对齐 memory `feedback_wait_coderabbit_approve.md`——**修完 review 意见后必须重新等 CodeRabbit re-review APPROVED**，**不自行判定**"我修好了应该过了所以直接 merge"。第二轮 review 同样按本协议（ScheduleWakeup 20 min × 最多 3 回合）。

#### 多 PR 场景

§10.1 多 PR 序列化时，**每个 PR 各自走完整 CodeRabbit 等待协议**——不能省。前一个 PR 未 APPROVED 不开下一个 PR。

### §10.3 单次 consume-plan 全自动到 merge

本 plan 的期望调用方式：**一次 `/consume-plan baomai-v4` 跑完全部 4 个 PR + 归档 plan**。

- consume-plan agent 在同一个 worktree / branch 序列中开 PR-1 → 等 review → merge → 开 PR-2 → ... → 全部 4 个 merge 完毕 → step 9 收尾清理
- 中途不要求人工干预——除非：
  - review 严重阻断（step 7 严重桶）反复修不过（≥2 轮）
  - merge 冲突 rebase 拿不准（step 4.2 ≥2 轮失败）
  - CodeRabbit 60 min 卡死（§10.2）
  - plan 设计层问题（评论指 plan 本身而非实装 patch）
- 全部 PR merge 后归档 plan：`git mv docs/plan-baomai-v4.md docs/finished_plans/` + 填写 Finish Evidence
- **预估总时长**：4 PR × (实施 1-2h + CodeRabbit 20-60 min + merge 5 min) ≈ 5-10 小时。全程 ScheduleWakeup 驱动，**不占用用户在线时间**。

### §10.4 Subagent 驱动的 4 PR 实施（context 隔离强制）

> 对齐 docs/CLAUDE.md §6.4——consume-plan 主线 agent **不亲自实施 PR**——为每个 PR 单独起一个 subagent（独立 context），主线只接收 subagent 的 `result` 段（200-500 token），避免长任务挤爆 context。

#### §10.4.1 subagent 配置（强制约定）

每次起 PR 实施 subagent 用以下参数：

```
Agent(
  description: "实施 PR-N <PR 名>",
  subagent_type: "claude",           // catch-all + 全工具集（Edit/Write/Bash/gh 全可用）
  model: "opus",                     // 强制 Opus（最强模型）
  prompt: "...任务描述...\n\nultrathink"   // 末尾 ultrathink 触发最高思维 budget
)
```

**关键约定**：

- **`subagent_type: "claude"`**：catch-all subagent，工具集 `*`；不要用 `general-purpose`（语义偏研究）也不要用 `Explore`（只读）
- **`model: "opus"`**：显式指定。**不要用 sonnet/haiku**——实施 + 多轮自我 review 需要顶级模型
- **prompt 末尾 `ultrathink`**：最高思维 budget
- **`run_in_background: false`**（默认）：主线必须等 subagent 返回（序列依赖）
- **不使用 `isolation: "worktree"`**：subagent 直接在 consume-plan 的主 worktree 内做事，避免 nested worktree

#### §10.4.2 subagent prompt 模板（PR-N 实施任务）

```
你是 plan-baomai-v4 的 PR-N 实施 subagent。任务是在主 worktree
(`$REPO_ROOT/.worktree/plan-baomai-v4`, branch: `auto/plan-baomai-v4`)
内完成以下范围：

## 范围（严格）
<本 PR 对应 plan 章节列表 + 必须实施的 TODO 清单>

## 必读
- plan: docs/plan-baomai-v4.md（特别是对应 P 阶段 + §8.1 决议 + §10）
- 现有 baomai_v3 代码: server/src/combat/baomai_v3/（理解接口模式）
- qi_physics 常数: server/src/qi_physics/constants.rs（物理常数扩展位置）
- 经脉系统: server/src/cultivation/components.rs + meridian/severed.rs
- commands: .claude/commands/consume-plan.md（通用工作流约束）

## 工作流约束
- 纯 Rust/Java 逻辑代码 → atomic commit + 对应测试全绿
- 新增 qi_physics 常数必须放 qi_physics/constants.rs
- 新增 gameplay 常数放 baomai_v4/constants.rs
- 新增 event 必须 #[derive(Event)] + 在 mod.rs Plugin 中 add_event
- 新增 skill 必须走 SkillRegistry::register + SkillMeridianDependencies::declare
- 跑测试：cd server && cargo test baomai_v4:: -- 全绿

## 禁止
- 不修改本 plan 范围外的文件（除 qi_physics/constants.rs 追加 + combat/mod.rs 注册模块 + DerivedAttrs 新增字段）
- 不动其他 plan-*.md / CLAUDE.md / worldview.md
- 不等 CodeRabbit review（主线负责）

## 完成后返回（严格 JSON 格式）
{
  "pr_url": "https://github.com/.../pull/<num>",
  "pr_number": <num>,
  "branch": "auto/plan-baomai-v4",
  "commits": [
    { "hash": "abc1234", "message": "..." }
  ],
  "tests_run": [
    "cd server && cargo test baomai_v4:: → N passed"
  ],
  "files_changed": ["server/src/combat/baomai_v4/...", ...],
  "notes": "简短说明 + 遇到的问题"
}

ultrathink
```

#### §10.4.3 主线流程（伪代码）

```
for pr_n in [PR-1..PR-4]:
    result = Agent(
        description: "实施 PR-{pr_n}",
        subagent_type: "claude",
        model: "opus",
        prompt: §10.4.2 模板填充本 PR 范围
    )
    pr_url = result.pr_url
    
    # 等 CodeRabbit review（§10.2）
    ScheduleWakeup(1200, "等 CR PR #{pr_n}")
    loop:
        status = gh pr checks {pr_url}
        if pass → break
        if pending → ScheduleWakeup(1200), max 3 rounds
        if fail → 起修复 subagent（同 §10.4.1 配置）→ 重等
    
    # merge
    gh pr merge --squash --delete-branch
    
# 归档
git mv docs/plan-baomai-v4.md docs/finished_plans/
# 填写 Finish Evidence + commit
```
