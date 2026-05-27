# Bong · plan-npc-combat-ai-v1

NPC 战斗智能升级——技能多类型（回血/buff/控制/攻击）上下文选择 + 截脉格挡防御行为 + 战斗 LOD 分层（Near 完整模拟 / Far 抽象判定）。让 NPC 从"只会追砍"变成"能治疗/上 buff/格挡/根据局势选技能"的修士。

**与 `plan-npc-combat-gear-v1` 的关系**：gear P1 已落地（`npc/technique.rs`：NpcTechniqueScorer + NpcTechniqueAction + select_technique + NpcCooldownMap）。本 plan 在其之上扩展"什么时候用什么类型技能 + 怎么防御 + 远距离怎么简化"。

**关键架构决策**：thinker 使用 `FirstToScore { threshold: 0.05 }` picker——scorer 只做 pass/fail 门控（超过 threshold 即选中，不比大小）。技能类型的上下文评分在 `select_technique()` **内部**做加权随机，不在 scorer 层做连续曲线。需要硬优先级覆盖的行为（濒死回血）用独立 scorer 插到 thinker 更高优先级位。

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ✅ 2026-05-27 | SkillCategory + select_technique 上下文加权 + LOD 门控 + NpcHealScorer 紧急回血 | `select_technique()` 按 hp/qi/距离/状态对各 category 差异化加权随机；NpcTechniqueScorer 接 LOD 门控；NpcHealScorer 在 hp<30% 时硬覆盖 |
| **P1** | ✅ 2026-05-27 | 回血/buff/utility 技能实现 + 经脉依赖声明 | 3+ heal/buff SkillFn 注册到 SkillRegistry + `declare()` 经脉依赖；目标选择扩展（self/ally） |
| **P2** | ✅ 2026-05-27 | NPC 格挡防御行为（独立路径，不经 SkillRegistry） | `NpcDefenseScorer` + `NpcDefenseAction` 发 DefenseIntent；`select_technique()` 排除 Defense category 避免冲突 |
| **P3** | ✅ 2026-05-27 | 战斗 LOD 分层 + qi 守恒 + tier 过渡清理 | Near=完整模拟、Far=战力评分概率判定 + QiTransfer 记账、Dormant=不打；tier 变更清理 CombatState |
| **P4** | ✅ 2026-05-27 | 集成测试 + 数值校准 | e2e 全链路；Far tier NPC 互斗有胜负+qi记账；新增测试 ≥ 30 条 |

---

## 接入面 Checklist

### 进料

| 来源模块 | 消费内容 | 用途 |
|---------|---------|------|
| `cultivation::skill_registry::SkillRegistry` | 44+ SkillFn 注册表 | P0 分类标记 + P1 新增 heal/buff SkillFn |
| `cultivation::known_techniques::TechniqueDefinition` | qi_cost / cooldown / cast_ticks / range / required_meridians | P0 读取技能元数据驱动评分 |
| `cultivation::known_techniques::KnownTechniques` | NPC 已学功法列表 | P0 评分候选池 |
| `cultivation::meridian::severed::SkillMeridianDependencies` | `declare()` / `lookup()` / `check_meridian_dependencies()` | P1 新 SkillFn 声明经脉依赖（红旗条） |
| `npc::technique::NpcCooldownMap` | per-entity per-technique CD | P0 评分过滤 |
| `npc::technique::NpcTechniqueScorer` / `NpcTechniqueAction` | 现有 0/0.85 评分器 + exclusive action | P0 加 LOD 门控；P1 扩展 target 路由 |
| `npc::technique::select_technique()` | proficiency 加权随机 | P0 扩展为 category-aware 上下文加权 |
| `combat::components::Wounds` | `health_current / health_max / severity` | P0 读 HP ratio；P1 回血写入 |
| `combat::components::StatusEffects` | 当前 buff/debuff 列表 | P0 评分（已有 buff 不重复上）；P1 施加 |
| `combat::events::DefenseIntent` / `DefenseWindow` | 截脉/格挡窗口机制 | P2 NPC 发 DefenseIntent |
| `combat::events::StatusEffectKind::ParryRecovery` / `SwordParrying` / `Staggered` | 格挡相关状态 | P2 检查是否在格挡恢复中 |
| `combat::resolve` | 伤害结算系统（兼容 NPC entity：不限定 `With<Client>`） | P2 防御窗口结算；P3 战力公式输入 |
| `cultivation::components::Cultivation` | realm / qi_current / qi_max | P0 评分；P3 战力评估 |
| `npc::brain::NpcBlackboard` | target_position / player_distance / retaliation_target | P0 评分上下文 |
| `npc::lod::NpcLodTier` / `NpcLodConfig` | Near/Far/Dormant | P0 LOD 门控；P3 分层判定 |
| `combat::components::DerivedAttrs` | attack_power / defense_power / move_speed_multiplier | P3 战力评分输入 |
| `npc::spawn::common::NpcMeleeProfile` | preferred_distance / melee_damage | P3 战力评分输入 |
| `npc::faction::FactionId` / `FactionMembership` | NPC 阵营归属 | P3 Far tier 敌对判定；P1 按 faction 分配技能子集 |
| `npc::equipment::NpcEquipment` | 6 slot 装备 | P3 战力评分维度 |
| `qi_physics::ledger::QiTransfer` | 真元流动记账 | P3 Far tier 抽象战斗 qi 消耗守恒 |

### 出料

| 产出 | 消费方 | 形式 |
|------|--------|------|
| `SkillCategory` enum（Attack/Heal/Buff/Control/Defense） | `TechniqueDefinition` 新字段 + `select_technique()` | 技能分类标签，存量 const 条目显式标 Attack |
| `NpcSkillScoringContext` struct | `select_technique()` 内部 | 血量/距离/buff 状态等上下文快照 |
| `NpcHealScorer` + NpcTechniqueAction 复用 | rogue/散修/弟子 thinker 高优先级位 | 濒死回血硬覆盖 |
| `NpcDefenseScorer` + `NpcDefenseAction` | rogue/散修/弟子 thinker | big-brain Scorer+Action，独立于 SkillRegistry |
| `CombatPowerScore` struct | Far tier 战斗判定系统 | 战力评分（realm + attrs + technique_count + equipment quality） |
| `AbstractCombatOutcome` enum | Far tier NPC-vs-NPC 结算 | Win/Lose/Draw + HP delta + qi delta + QiTransfer 记账 |
| heal/buff SkillFn（3+） | `SkillRegistry` | 回血/防御减伤/加速，含经脉依赖声明 |

### 共享类型 / event

- **复用** `SkillRegistry` / `CastResult` / `SkillFn`（NPC 与玩家共用调用入口）
- **复用** `DefenseIntent` / `DefenseWindow` / `CombatState`（不新建防御事件）
- **复用** `ApplyStatusEffectIntent`（buff/debuff 走现有 status effect 管道）
- **复用** `NpcCooldownMap`（不新建冷却系统）
- **复用** `SkillMeridianDependencies::declare()`（新 SkillFn 必须声明经脉依赖）
- **扩展** `TechniqueDefinition` 加 `category: SkillCategory` 字段（`#[serde(default)]`）
- **扩展** `select_technique()` 签名加 `NpcSkillScoringContext` 参数
- **不新建** 独立 NPC 技能定义——复用 `TechniqueDefinition`，通过 `SkillCategory` 区分
- **不重复** DefenseIntent 路径——格挡走独立 NpcDefenseAction，`select_technique()` 排除 `category=Defense`

### 跨仓库契约

| 层 | symbol / key | 变更 |
|----|-------------|------|
| server | `npc::technique::select_technique()` | P0 扩展签名 + 上下文加权 |
| server | `npc::technique::NpcTechniqueScorer` | P0 加 LOD 门控 |
| server | `npc::brain::NpcHealScorer` | P0 新增濒死回血 scorer |
| server | `npc::brain::NpcDefenseScorer` / `NpcDefenseAction` | P2 新增 |
| server | `npc::lod::AbstractCombatSystem` | P3 新增 Far tier 判定 |
| server → client | `bong:vfx_event` | NPC 格挡/技能释放复用已有 VFX channel，不新增 |
| server → agent | `bong:world_state` NpcDigest | NPC 战斗行为纳入现有 world_state 快照 |

### worldview 锚点

- **§四 战斗系统**（L213-393）：三层战力模型（伤口×经脉×真元）、"赢了战斗输了生存"、境界差物理事实
- **§五 战斗流派**（L397-514）：截脉震爆流（L434-437）= 血肉反应装甲，弹反窗口+中和效率；流派主战斗变量表（L461-469）
- **§四 经脉物理可见性**（L286）："断了肺经的飞剑手就废了"——NPC 经脉 SEVERED 时对应功法必须被阻断
- **§七 散修 NPC 行为**（L730-740）：散修评估威胁度做出攻/守/逃/交易反应
- **§十五 设计原则 第 4 条**：死亡是学费——NPC 也应表现出"怕死"的行为（血少回血/格挡/逃跑梯度）

### qi_physics 锚点

- **Near tier NPC 技能 qi_cost 消耗**：走现有 `SkillFn` 内部的 `qi_release_to_zone()` 守恒路径，不新增
- **Far tier 抽象战斗 qi 消耗**：每次 `abstract_combat_resolve()` 后，双方按 realm 对应平均 qi_cost 扣减 `Cultivation.qi_current`，emit `QiTransfer { from: NpcEntity, to: Zone, amount, reason: AbstractCombatRelease }`。**不跳过守恒律**
- **回血技能不走真元回复**：回血 = `Wounds` 层 HP 修复（调用 `apply_wound_heal` 同时降 severity + 回 HP），不涉及 qi 守恒

---

## P0 — SkillCategory + select_technique 上下文加权 + NpcHealScorer

### P0.1 SkillCategory 分类

`cultivation::known_techniques` 新增：

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SkillCategory {
    #[default]
    Attack,
    Heal,
    Buff,
    Control,
    Defense,
}
```

`TechniqueDefinition` 加字段 `pub category: SkillCategory`。存量 44 个 const 字面量显式标记 `category: SkillCategory::Attack`，行为不变。`zhenmai.parry` 标记为 `Defense`。（注：`TechniqueDefinition` 不 derive serde，是 `pub const` 数组，无需 `#[serde(default)]`——数值见 §8.1 #5）

> 注：部分技能有多面性（`zhenmai.parry` 既防御又有反震伤害），但在 NPC AI 决策层只需看主要用途。一个 enum 字段够用——NPC 不需要"同时当攻击和防御使用"的复合评分，选中后 SkillFn 自然执行全部效果。

### P0.2 NpcSkillScoringContext

`npc::technique` 新增结构体，在 `select_technique()` 调用前从 ECS 组装：

```rust
pub struct NpcSkillScoringContext {
    pub hp_ratio: f32,
    pub qi_ratio: f32,
    pub target_distance: f32,
    pub target_hp_ratio: f32,
    pub has_active_buff: bool,
    pub in_combat: bool,
}
```

> `ally_nearby_count` 移除——需要 O(N²) spatial query，性能不可接受。Buff 评分仅用 self 状态判断。

### P0.3 select_technique 上下文加权

**重要**：连续评分在 `select_technique()` **内部**做，不在 scorer 层。`NpcTechniqueScorer` 仍输出 pass/fail（有可用技能 → 0.85，无 → 0.0），只加 LOD 门控。

`select_technique()` 内部按 `SkillCategory` 套用权重乘数，与 proficiency 联合做加权随机：

| Category | 权重函数 `w(ctx)` | 说明 |
|----------|-----------------|------|
| `Heal` | `(1.0 - ctx.hp_ratio).powf(2.0) * 0.9` | 血越低权重越高 |
| `Buff` | `if !ctx.has_active_buff && ctx.in_combat { 0.6 } else { 0.05 }` | 战斗中无 buff 时较高 |
| `Attack` | `0.8` | 默认主力 |
| `Control` | `0.4` | 基础较低 |
| `Defense` | `0.0`（**排除**，走独立 NpcDefenseAction） | 见 P2 |

最终每个候选技能的选择权重 = `category_weight(ctx) * proficiency`。`qi_ratio < 0.15` 时排除 qi_cost 最高的 50% 候选（省着用）。

### P0.4 NpcHealScorer 紧急回血硬覆盖

新增独立 scorer，插到 thinker **高优先级位**（`NpcTechniqueScorer` 之前）：

```rust
pub struct NpcHealScorer;
```

评分逻辑：
- `hp_ratio < 0.3 && has_heal_technique && !on_heal_cooldown` → 0.9（超过 threshold，被 `FirstToScore` 优先选中）
- 否则 → 0.0

配对的 Action 复用 `NpcTechniqueAction`，但 `select_technique()` 在 NpcHealScorer 触发时强制 `category_filter = Heal`（只从 Heal 池里选）。

这解决了 `FirstToScore` 语义问题：正常情况 NpcTechniqueScorer(0.85) 触发后在 `select_technique()` 内部做加权随机（可能选到 Heal/Buff/Attack）；濒死时 NpcHealScorer(0.9) 排在前面被 `FirstToScore` 先选中，强制只选 Heal。

### P0.5 NpcTechniqueScorer LOD 门控

现有 `NpcTechniqueScorer` 加 `lod_gated_score(tier, tick, &cfg, || { ... })`，使用 `ScorerKind::Cosmetic`。Far/Dormant tier 不评估技能。

### P0.6 NpcCooldownMap 死亡清理（前置修补）

`npc::technique::NpcCooldownMap::remove_all_for(entity)` 已实现但未在 NPC 死亡/despawn 时调用。在 NPC lifecycle cleanup system 中补充调用，防止 entity 复用时继承旧 NPC 的冷却状态。

### P0 验收标准

- `select_technique()` 接受 `NpcSkillScoringContext`，按 category 差异化加权随机
- NpcHealScorer 在 hp<0.3 时得分 0.9，覆盖 NpcTechniqueScorer
- Far tier NPC 技能 scorer 得分恒为 0
- 存量 44 个 `TechniqueDefinition` 自动标记 `category: Attack`，行为不变
- NPC 死亡/despawn 时 `NpcCooldownMap` 清理
- 单测覆盖：各 category 加权边界（hp=0/0.3/0.5/1.0 对应 Heal 权重）、LOD 门控、NpcHealScorer threshold、qi_ratio 过滤、cooldown 清理

---

## P1 — 回血/buff/utility 技能实现

### P1.1 回血 SkillFn

注册 `npc_heal_basic` 到 `SkillRegistry`：
- 效果：调用 `apply_wound_heal(&mut wounds, None, heal_grades)` 同时降 severity + 回 HP（不只写 `health_current`，否则 HUD 剪影仍显示重伤）
- `heal_amount` = `5.0 + realm_rank as f64 * 3.0`
- qi_cost = `8.0`，cooldown = `200 ticks`（10 秒），cast_ticks = `20`（1 秒前摇）
- 目标 = self
- `TechniqueDefinition` 新增条目 `category: Heal`
- **经脉依赖**：`SkillMeridianDependencies::declare("npc_heal_basic", vec![MeridianId::SpLeenYin, MeridianId::KidneyYin])`（足三阴——脾/肾，偏韧/持久）

### P1.2 buff SkillFn

注册 `npc_buff_speed` 到 `SkillRegistry`：
- 效果：发 `ApplyStatusEffectIntent { kind: SpeedBoost, magnitude: 0.3, remaining_ticks: 200 }`
- qi_cost = `5.0`，cooldown = `400 ticks`（20 秒），cast_ticks = `10`
- 目标 = self
- `TechniqueDefinition` 新增条目 `category: Buff`
- **经脉依赖**：`declare("npc_buff_speed", vec![MeridianId::StomachYang, MeridianId::BladderYang])`（足三阳——偏速）

注册 `npc_buff_defense` 到 `SkillRegistry`：
- 效果：发 `ApplyStatusEffectIntent { kind: DamageReduction, magnitude: 0.2, remaining_ticks: 200 }`
- qi_cost = `6.0`，cooldown = `400 ticks`，cast_ticks = `10`
- 目标 = self
- `TechniqueDefinition` 新增条目 `category: Buff`
- **经脉依赖**：`declare("npc_buff_defense", vec![MeridianId::LungYin, MeridianId::HeartYin])`（手三阴——气）

### P1.3 目标选择扩展

`select_technique()` 返回值扩展：

```rust
pub struct SelectedTechnique {
    pub technique_id: String,
    pub skill_fn: SkillFn,
    pub target: SkillTarget,
}

pub enum SkillTarget {
    NearestEnemy,
    SelfCast,
}
```

> `NearestAlly` 暂不实现（需 spatial query + ally 判定），P1 只做 self/enemy 两路。

`NpcTechniqueAction` 内部根据 `SkillTarget` 决定传入 `SkillFn` 的 target entity。

### P1.4 assign_npc_techniques 扩展

`assign_npc_techniques()` 按 realm/archetype/faction 分配 heal/buff 技能：
- 所有境界 ≥ 引气的 NPC 分配 `npc_heal_basic`
- 所有境界 ≥ 凝脉的 NPC 额外分配 `npc_buff_speed` 或 `npc_buff_defense`（按 archetype 二选一）

### P1 验收标准

- NPC 血量 < 30% 时 NpcHealScorer 触发，强制选 Heal 技能（日志可观测）
- NPC 进入战斗且无 SpeedBoost 时通过 NpcTechniqueScorer → select_technique 加权随机可能选到 buff
- `SelectedTechnique.target` 正确路由：Heal → self，Attack → nearest enemy
- 所有新 SkillFn 已调用 `SkillMeridianDependencies::declare()`
- 经脉 SEVERED 时对应技能被 `check_meridian_dependencies()` 过滤
- 单测覆盖：heal 效果（severity 降级 + HP 回复）、buff 不重复叠加、目标路由、经脉依赖过滤

---

## P2 — NPC 格挡防御行为（独立路径）

### P2.0 关键设计决策：格挡走独立 Action，不经 SkillRegistry

**问题**：`zhenmai.parry` 已注册在 SkillRegistry 中，`NpcTechniqueAction` 可能随机选到它并发 `DefenseIntent`。如果同时有独立的 `NpcDefenseAction` 也发 `DefenseIntent`，同 tick 会叠加两个 ParryRecovery。

**解法**：
1. `select_technique()` 内部排除 `category == Defense` 的候选——NpcTechnique 路径不会选到 `zhenmai.parry`
2. 格挡只走 `NpcDefenseAction` → 直接发 `DefenseIntent`，不经 SkillRegistry
3. 两个路径不会冲突

### P2.1 NpcDefenseScorer

新增 `brain/scorers_combat.rs`：

```rust
pub struct NpcDefenseScorer;
```

评分逻辑：
- 前提：`in_combat && !has_status(ParryRecovery) && !has_status(SwordParrying) && !has_status(Staggered)`
- realm 调节：`Awaken → 0.0`，`Induce → 0.5`，`Condense → 0.65`，`Solidify+ → 0.7`
- LOD 门控：`ScorerKind::Cosmetic`（仅 Near 评估）

### P2.2 NpcDefenseAction

新增 `brain/actions_combat.rs`：

```rust
pub struct NpcDefenseAction;
```

行为流程：
1. `Requested` → 检查 realm ≥ Induce + qi 够 `jiemai_qi_cost_for_realm()` + 无 ParryRecovery → `Executing`
2. `Executing` → 发 `DefenseIntent { defender: npc_entity, issued_at_tick: now }`
   - realm 控制开窗频率（越高越频繁、弹反越精准）：
     - `Induce`：每 `80~120 ticks`（手忙脚乱）
     - `Condense`：每 `60~80 ticks`
     - `Solidify`：每 `40~60 ticks`
     - `Spirit`：每 `20~40 ticks`（大师级节奏）
   - 开窗后 combat::resolve 自动写入 `ParryRecovery` 状态 → scorer 下帧返回 0 → action Success
3. `Cancelled` → `Failure`

> NPC 无 `PlayerInventory`，`jiemai_prep_window(None, ...)` 走 `None` 分支，窗口 = `QI_ZHENMAI_PREP_WINDOW_MS`（1000ms）。这是合理的——NPC 无装甲修正。

### P2.3 thinker 接入

thinker 优先级从高到低：

```rust
// ... (生存/特殊行为 scorers)
.when(NpcHealScorer, NpcTechniqueAction)       // 濒死回血（P0）
.when(NpcTechniqueScorer, NpcTechniqueAction)  // 功法释放（已有）
.when(MeleeRangeScorer, MeleeAttackAction)     // 近战普攻
.when(NpcDefenseScorer, NpcDefenseAction)       // 格挡（P2）
.when(ChaseTargetScorer, ChaseAction)           // 追击
.when(PlayerProximityScorer, FleeAction)        // 逃跑
// ... (日常行为 scorers)
```

**关键顺序**：Defense 排在 MeleeAttack **之后**——近身时优先攻击，攻击 action Success 后下一帧如果还在战斗则 Defense 有机会触发（因为 MeleeRangeScorer 需要 melee cooldown 过后才重新得分）。这产生自然的"攻→防→攻"节奏。

### P2 验收标准

- 近身战斗中 NPC 周期性发 `DefenseIntent`（日志可观测）
- `Awaken` NPC 不格挡；`Spirit` NPC 格挡频率明显高于 `Induce`
- `NpcTechniqueAction` 不会选到 `zhenmai.parry`（category=Defense 被排除）
- 格挡成功时触发 `ParryRecovery` 状态效果
- 单测覆盖：各 realm 开窗间隔范围、ParryRecovery 期间 scorer 得分 0、LOD 门控、Defense category 排除

---

## P3 — 战斗 LOD 分层

### P3.1 CombatPowerScore

战力评分公式，使用加权平均 + 下限（避免残血低 qi 时直接归零）：

```rust
pub struct CombatPowerScore(pub f32);

pub fn compute_combat_power(
    realm: Realm,
    cultivation: &Cultivation,
    wounds: &Wounds,
    derived: &DerivedAttrs,
    techniques: &KnownTechniques,
    equipment: Option<&NpcEquipment>,
) -> CombatPowerScore {
    let realm_weight = realm.ordinal() as f32 * 20.0;
    let hp_ratio = wounds.health_current / wounds.health_max;
    let qi_ratio = (cultivation.qi_current / cultivation.qi_max) as f32;
    let condition_factor = (hp_ratio * 0.6 + qi_ratio * 0.4).max(0.15);
    let combat_attrs = derived.attack_power + derived.defense_power;
    let tech_count = techniques.entries.len() as f32 * 2.0;
    let equip_quality = equipment
        .map(|e| e.total_quality_score())
        .unwrap_or(0.0);

    CombatPowerScore(
        (realm_weight + combat_attrs + tech_count + equip_quality) * condition_factor
    )
}
```

> `condition_factor` 用加权平均 `hp*0.6 + qi*0.4`，下限 0.15。残血半 qi 时 factor=0.5（而非乘法的 0.25），保留翻盘空间（worldview §五"后招原则"）。

### P3.2 Far tier 抽象判定 + qi 守恒

新增 system `abstract_combat_system`，仅 Far tier NPC-vs-NPC 战斗使用：

```rust
fn abstract_combat_resolve(
    attacker_power: CombatPowerScore,
    defender_power: CombatPowerScore,
    attacker_realm: Realm,
    defender_realm: Realm,
    rng: &mut impl Rng,
) -> AbstractCombatOutcome {
    let ratio = attacker_power.0 / (attacker_power.0 + defender_power.0 + f32::EPSILON);
    let roll: f32 = rng.gen();
    if roll < ratio {
        AbstractCombatOutcome::AttackerWins {
            damage_ratio: ratio * 0.3,
            attacker_qi_cost: avg_qi_cost_for_realm(attacker_realm),
            defender_qi_cost: avg_qi_cost_for_realm(defender_realm) * 0.5,
        }
    } else {
        AbstractCombatOutcome::DefenderWins {
            damage_ratio: (1.0 - ratio) * 0.3,
            attacker_qi_cost: avg_qi_cost_for_realm(attacker_realm),
            defender_qi_cost: avg_qi_cost_for_realm(defender_realm),
        }
    }
}
```

每次判定后：
1. 败方 `wounds.health_current -= wounds.health_max * damage_ratio`
2. 双方 `cultivation.qi_current -= qi_cost`
3. emit `QiTransfer { from: NpcEntity, to: Zone(npc_zone), amount: qi_cost }` 归还环境（守恒律）

每 200 ticks（10 秒）判定一次。

### P3.3 LOD 战斗路由

| LOD tier | 战斗方式 | 系统 |
|----------|---------|------|
| **Near** | 完整模拟：AttackIntent / DefenseWindow / StatusEffect / 动画 | 现有 `combat::resolve` |
| **Far** | 抽象判定：`CombatPowerScore` 对拍 → `AbstractCombatOutcome` → 写 Wounds + qi + QiTransfer | `abstract_combat_system` |
| **Dormant** | 不战斗 | — |

### P3.4 tier 过渡清理

新增 system 或 hook：当 `NpcLodTier` 从 Near 变为 Far/Dormant 时：
1. 清理 `CombatState.incoming_window = None`（防止 DefenseWindow 永远挂着——Far 不跑 resolve 无法自然消费）
2. 当前 big-brain action 的 ActionState 不需要手动 cancel——scorer 降频后自然不再得分，action idle 后 thinker 重新选择
3. `StatusEffects` 中的 `ParryRecovery` / `SwordParrying` 等短时效果由 `status_effect_tick` 自然过期（该 system 不受 LOD 影响）

### P3.5 NPC-vs-NPC 战斗触发

Far tier NPC 之间的战斗触发由已实装的 `ThreatAssessment` / `SelfInterestDecision::Attack` 驱动（`plan-npc-overhaul-v1` P2 ✅ 2026-05-24）。`assign_hostile_encounters`（faction.rs:534）已有 Near 范围（16 格）的 `DuelTarget` 分配；Far tier `abstract_combat_system` 在更大范围查询 `FactionStore::is_hostile_pair()`（faction.rs:268）判定敌对。（数值见 §8.1 #3）

### P3.6 NPC despawn 安全处理

Far tier NPC 被 despawn（LOD 切 Dormant 或被回收）时：
- `abstract_combat_system` 下一轮 query 时 entity 不存在 → 对手的 target 无效
- 对手 graceful fallback：target entity miss 时重置 `retaliation_target = None`，回到 idle

### P3 验收标准

- Far tier 两个 NPC 相遇时通过 `abstract_combat_resolve()` 产出胜负（日志可观测）
- 败方 `Wounds.health_current` 按 `damage_ratio` 扣减
- 双方 `qi_current` 扣减 + `QiTransfer` event 发出（守恒律）
- Near→Far 过渡时 `CombatState.incoming_window` 被清理
- `CombatPowerScore`：高境界 > 低境界、满血 > 残血、`condition_factor` 下限 0.15
- NPC despawn 后对手不 panic
- 单测覆盖：战力公式边界、抽象判定概率分布（1000 次 roll）、LOD 路由分叉、tier 过渡清理、despawn fallback

---

## P4 — 集成测试 + 数值校准

### P4.1 全链路 e2e 测试

构造测试场景（无需玩家连接）：
1. 两个 Near tier NPC 互殴：攻击 → 格挡 → 技能释放 → 血少回血 → buff → 继续战斗
2. 两个 Far tier NPC 互殴：`abstract_combat_resolve()` → 败方扣血 + 双方扣 qi + QiTransfer → 低于阈值死亡
3. NPC 从 Far → Near 过渡：CombatState.incoming_window 被清理；战斗方式切到完整模拟，HP/qi 状态保持一致
4. NPC 死亡后 `NpcCooldownMap` 清理 + `StatusEffects` 不残留到复用 entity

### P4.2 数值校准

- 回血量 vs 攻击伤害比：回血不应让 NPC 不死，每次回血约恢复 1~2 次攻击伤害量
- 格挡频率 vs 攻击频率：NPC 不应 100% 格挡，`Induce` 命中率约 20%，`Spirit` 约 60%
- Far tier 战斗速率：每 200 ticks（10 秒）判定一次，避免瞬间决出胜负
- buff 持续时间 vs 战斗长度：一场典型战斗 30~60 秒，buff 200 ticks（10 秒）= 约 1/3 战斗长度
- `CombatPowerScore` 各维度权重校准：realm_weight 占比 ~50%，attrs ~25%，tech ~15%，equipment ~10%

### P4 验收标准

- e2e 测试全绿
- 数值校准参数写入 `npc::technique` 模块常量（可配置）
- `cargo test` 全绿，新增测试 ≥ 30 条

---

## §8 开放问题（P0 决策门前需收口）

### #1 NPC 回血技能的 HUD 标记

回血效果走 `apply_wound_heal()`（直接修 severity + HP）。是否额外发 `ApplyStatusEffectIntent { kind: WoundHeal }` 作为 HUD 可视标记？`WoundHeal` 是纯标记型 status effect（`pill.rs` 中一次性调用，无 tick 自动回血逻辑）。如果发，client 能看到"NPC 正在自愈"；如果不发，省一条 event。

### #2 NPC buff 是否需要对玩家可见？

如果可见，需要扩展 `bong:npc_metadata` S2C packet 传 active status effects 给 client。如果不可见，省带宽但玩家无法通过观察 NPC 判断其状态（inspect 除外）。

### #3 Far tier 抽象战斗的触发条件

当前设计依赖 `plan-npc-overhaul-v1` P2 的 `ThreatAssessment`。如果 overhaul P2 未实装，需要定义 `FactionId` 敌对关系的简化触发规则（§P3.5 已给出降级方案）。

### #4 NPC 复活后技能状态重置

`plan-death-lifecycle-v1` 重生链路会重置 `Cultivation` / `Wounds`。但 Bevy entity 复用时 `StatusEffects` 中的 buff 残留（如 SpeedBoost）是否会污染新 NPC？需确认 lifecycle cleanup 是否 remove `StatusEffects` component 或 clear entries。

### #5 TechniqueDefinition 实例化方式与 serde 兼容

`TechniqueDefinition` 目前是 `const` 数组（`TECHNIQUE_DEFINITIONS`）还是走外部 TOML/JSON？新增 `category` 字段用 `#[serde(default)]` 兼容，但如果是 `const` 则无 serde 问题。需确认后决定是否需要 migration。

### #6 NPC 自我治疗/buff/格挡的视听反馈

纯 server 逻辑阶段（P0-P3）先不做视听，P4 或后续 plan 补。规格需参照 docs/CLAUDE.md §二视听要求（粒子基类/音效 recipe/HUD layer 等全精度规格）。

全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

---

## §8.1 决议（pre-P0 收口，2026-05-27）

### #1 NPC 回血技能的 HUD 标记

**决议**：
1. P1 NPC heal SkillFn 不发 `ApplyStatusEffectIntent { kind: WoundHeal }`
2. `WoundHeal` 是纯 HUD 标记（`status_effect_tick` 只倒计时，无 tick 回血逻辑）。Client 目前无 NPC status effect 渲染管线（`StatusEffectHudPlanner` 只渲染玩家自身），发了也没有可视效果
3. 省一条 event 交通。后续若实装 NPC buff 可见性（#2），再补发

**落点**：`server/src/alchemy/pill.rs:406`（apply_wound_heal 签名）/ `server/src/combat/events.rs:121`（WoundHeal variant）/ `client/src/.../hud/StatusEffectHudPlanner.java`（仅渲染玩家自身）/ plan §P1.1

### #2 NPC buff 是否需要对玩家可见？

**决议**：
1. 本 plan scope 内不做 NPC buff 可见性
2. 需要扩展 `NpcMetadataS2c`（npc_metadata.rs:28-54）加 status_effects 字段 + client 全新渲染管线（NPC 头顶 buff 图标）—— 工作量超出本 plan 纯 server 逻辑定位
3. 玩家仍可通过 inspect 交互查看 NPC 状态（已有 `bong:npc_metadata` 传 hp_ratio / qi_ratio / techniques）

**落点**：`server/src/network/npc_metadata.rs:28-54`（NpcMetadataS2c 当前字段）/ `client/src/.../npc/NpcMetadataStore.java`（client 侧 NPC 数据存储）/ plan 无需修改

### #3 Far tier 抽象战斗的触发条件

**决议**：
1. `plan-npc-overhaul-v1` P2 (ThreatAssessment) 已 ✅ 2026-05-24 — 降级方案不需要
2. Far tier 战斗直接由 `ThreatAssessment` + `SelfInterestDecision::Attack` 驱动
3. `assign_hostile_encounters`（faction.rs:534）已有 Near 范围（16 格）的 `DuelTarget` 分配。Far tier 新增 `abstract_combat_system` 在更大范围查询 `FactionStore::is_hostile_pair()`（faction.rs:268）

**落点**：`server/src/npc/faction.rs:268`（is_hostile_pair）/ `server/src/npc/faction.rs:534`（assign_hostile_encounters）/ `docs/finished_plans/plan-npc-overhaul-v1.md:11`（P2 ✅）/ plan §P3.5 已修改

### #4 NPC 复活后技能状态重置

**决议**：
1. `StatusEffects` 在 entity despawn 时由 Bevy 自动 drop（component 级清理），不需要手动 clear
2. `NpcCooldownMap` 是全局 `Resource`（非 component），entity index 复用时旧 entries 会残留 — P0.6 必须接入 `remove_all_for`
3. 接入点：`handle_npc_terminated`（lifecycle.rs:693）中 `Despawned` marker 插入后，同系统内调用 `cooldown_map.remove_all_for(entity)`
4. 新 NPC spawn 时通过 `npc_runtime_bundle`（lifecycle.rs:578）拿到 `StatusEffects::default()`，干净初始状态

**落点**：`server/src/npc/technique.rs:92-94`（remove_all_for 定义）/ `server/src/npc/lifecycle.rs:693-727`（handle_npc_terminated）/ `server/src/npc/lifecycle.rs:578`（npc_runtime_bundle）/ plan §P0.6

### #5 TechniqueDefinition 实例化方式与 serde 兼容

**决议**：
1. `TechniqueDefinition` 是 `pub const [TechniqueDefinition; 44]` 硬编码数组，derive `Debug, Clone, Copy, PartialEq`，**不 derive Serialize/Deserialize**
2. 新增 `category: SkillCategory` 直接加到 struct 定义 + 44 个 const 字面量里，Rust 编译器强制所有字段显式赋值
3. `#[serde(default)]` **不需要** — plan §P0.1 和出料表已修正
4. `SkillCategory` 必须 derive `Copy`（因为 `TechniqueDefinition` 是 Copy）

**落点**：`server/src/cultivation/known_techniques.rs:86-100`（struct 定义）/ `server/src/cultivation/known_techniques.rs:119`（TECHNIQUE_DEFINITIONS const 数组，44 条）/ plan §P0.1 已修改

### #6 NPC 自我治疗/buff/格挡的视听反馈

**决议**：
1. 确认本 plan 为纯 server 逻辑 plan（P0-P3 纯 Rust ECS 系统），per docs/CLAUDE.md 视听规格要求"纯 server 逻辑 plan 无此要求"
2. NPC 技能释放/格挡的视听反馈推迟到后续 plan

**落点**：`docs/CLAUDE.md` 视听规格排除条款 / plan 无需修改

---

## Finish Evidence

### 落地清单

| 阶段 | 模块 / 文件路径 |
|------|----------------|
| P0 | `server/src/cultivation/known_techniques.rs` — SkillCategory enum + 47 TechniqueDefinition 标注 |
| P0 | `server/src/npc/technique.rs` — NpcSkillScoringContext + category_weight + select_technique 上下文加权 + NpcHealScorer/NpcHealAction + LOD 门控 |
| P0 | `server/src/npc/lifecycle.rs` — NpcCooldownMap 死亡清理 |
| P1 | `server/src/npc/npc_skill.rs` — npc_heal_basic / npc_buff_speed / npc_buff_defense SkillFn |
| P1 | `server/src/cultivation/skill_registry.rs` — SkillRegistry 注册 |
| P1 | `server/src/cultivation/mod.rs` — SkillMeridianDependencies 声明 |
| P1 | `server/src/npc/technique.rs` — SelectedTechnique / SkillTarget / inject_npc_utility_skills |
| P2 | `server/src/npc/brain/scorers_combat.rs` — NpcDefenseScorer |
| P2 | `server/src/npc/brain/actions_combat.rs` — NpcDefenseAction |
| P2 | `server/src/npc/spawn/rogue.rs` — thinker 接入 |
| P3 | `server/src/npc/combat_power.rs` — CombatPowerScore + compute_combat_power |
| P3 | `server/src/npc/abstract_combat.rs` — abstract_combat_resolve + apply_outcome + apply_qi_cost |
| P3 | `server/src/npc/abstract_combat_system.rs` — LOD 战斗路由 + 过渡清理 + NPC-vs-NPC 触发 + despawn 安全 |
| P3 | `server/src/qi_physics/ledger.rs` — QiTransferReason::AbstractCombat |
| P4 | `server/src/npc/combat_ai_integration_test.rs` — 25 条 e2e 集成测试 |
| P4 | `server/src/npc/npc_skill.rs` — 10 个校准常量提取 |

### 关键 commit

| Hash | 日期 | 描述 |
|------|------|------|
| `00b656eb8` | 2026-05-27 | docs: plan + §8.1 pre-P0 决议收口 |
| `a4dd9500b` | 2026-05-27 | P0.1 SkillCategory 分类 — 44 技能逐条标注 |
| `d6cdf8bb8` | 2026-05-27 | P0.2+P0.3 NpcSkillScoringContext + select_technique 上下文加权 |
| `923780bde` | 2026-05-27 | P0.4+P0.5+P0.6 NpcHealScorer + LOD 门控 + 死亡冷却清理 |
| `22b5d6efd` | 2026-05-27 | P1.1-P1.2 注册 3 个 NPC 功法 TechniqueDefinition |
| `51a88f47a` | 2026-05-27 | P1.1-P1.2 NPC 回血/buff SkillFn 实现 + SkillRegistry/MeridianDeps 注册 |
| `bf0727515` | 2026-05-27 | P1.3 SelectedTechnique + SkillTarget 目标路由 |
| `858d8610b` | 2026-05-27 | P1.4 inject_npc_utility_skills 按境界注入技能 |
| `4c72cd46a` | 2026-05-27 | P2 NPC 格挡防御行为 — NpcDefenseScorer + NpcDefenseAction |
| `927d6579f` | 2026-05-27 | P3.1 CombatPowerScore 战力评分 |
| `9eff9564e` | 2026-05-27 | P3.2 抽象战斗判定 + QiTransferReason::AbstractCombat |
| `9f40f7350` | 2026-05-27 | P3.3-P3.6 LOD 战斗路由 + 过渡清理 + 触发 + despawn 安全 |
| `86c19f33f` | 2026-05-27 | P4.1 集成测试 25 条 |
| `aea233795` | 2026-05-27 | P4.2 数值校准常量提取 |
| `2042835c0` | 2026-05-27 | fix: NPC SkillFn qi 守恒 + LodTransitionCleaned marker（review 修复） |

### 测试结果

```
cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
→ 6587 passed, 0 failed, 1 ignored
NPC 战斗 AI 相关测试：166 条（含 5 条 qi 守恒 pin 测试）
```

### 跨仓库核验

| 层 | symbol | 状态 |
|----|--------|------|
| server | `npc::technique::select_technique()` | ✅ 扩展签名 + 上下文加权 |
| server | `npc::technique::NpcHealScorer` / `NpcHealAction` | ✅ 新增 |
| server | `npc::brain::NpcDefenseScorer` / `NpcDefenseAction` | ✅ 新增 |
| server | `npc::abstract_combat_system` | ✅ 新增 Far tier 判定 |
| server | `qi_physics::ledger::QiTransferReason::AbstractCombat` | ✅ 新增守恒记账 |
| server → client | `bong:vfx_event` | 不涉及（纯 server 逻辑 plan） |
| server → agent | `bong:world_state` | 不涉及（NpcDigest 无结构变更） |

### 遗留 / 后续

- NPC 技能释放 / 格挡的视听反馈推迟到后续 plan（§8.1 #6 决议）
- NPC buff 对玩家可见性推迟到后续 plan（§8.1 #2 决议，需扩展 bong:npc_metadata + client 渲染管线）
- NPC 回血时的 WoundHeal HUD 标记推迟到 buff 可见性一并实装（§8.1 #1 决议）
- NPC ally 目标选择（NearestAlly）暂未实装，P1.3 只做了 SelfCast / NearestEnemy 两路
