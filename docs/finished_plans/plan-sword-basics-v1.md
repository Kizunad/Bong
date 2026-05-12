# Bong · plan-sword-basics-v1

**基础武技·剑道三式**——劈、刺、格。前三境武人入门剑招，无境界门槛，拿起剑就能学。不是第八个流派，是**凡人武技层**的基础战斗技能，跟七大流派解耦——任何流派的修士拿起剑都能用，任何醒灵散修也能用。每招独立 proficiency，反复使用肌肉记忆递增。

**世界观锚点**：
- `worldview.md §四:332-340` 距离衰减"末法修仙是拼刺刀"——剑道三式定位近身武技
- `worldview.md §五:535-558` "流派由组合涌现（无系统门禁）"——任何修士任何时候可试任何招式；剑道三式无门禁
- `worldview.md §五:411` 凡器边界：凡铁/木石档日用工具也能打人，低基础伤害；真正决定胜负的仍是真元池、经脉流量和近身灌注
- `worldview.md §五:544` 招式熟练度（重复演示的肌肉记忆）——proficiency 正典依据
- `worldview.md §六.1:593` 手三阴偏气利远程/轻盈/御物——剑修经脉路径偏好（不锁，只适配）
- `worldview.md §六.2:611` 锋锐色：真元呈线状流动、边缘锐利、攻击穿透+、真元易漏不擅缠斗——长期练剑术后的染色副产物

**library 锚点**：无直接相关馆藏

**交叉引用**：
- `plan-weapon-v1`（finished）：`WeaponKind::Sword` + `Weapon` component + base_attack/durability 链
- `plan-combat-no_ui`（finished）：`AttackIntent` + `CombatEvent` + `StatusEffectKind`
- `plan-hotbar-modify-v1`（finished）：hotbar 1-9 `Technique` + `UseQuickSlot` 施放管线
- `plan-skill-v1`（finished）：`SkillId::Combat` XP 追踪框架（本 plan 不走 SkillId，走 `KnownTechnique.proficiency`）
- `plan-meridian-severed-v1`（active）：`SkillMeridianDependencies::declare()` 注册
- `plan-qi-physics-v1`（finished）：`qi_physics::distance::attenuation` 距离衰减（劈的外放真元微量衰减）
- `plan-style-vector-integration-v1`（finished）：`PracticeLog.add()` 染色挂钩——剑道三式**不调用**（跨流派基础武技不贡献特定流派权重）

**前置依赖**：
- `Weapon` component ✅（`server/src/combat/weapon.rs`）
- `AttackIntent` / `CombatEvent` ✅（`server/src/combat/events.rs`）
- `TechniqueDefinition` / `KnownTechniques` / `SkillRegistry` ✅（`server/src/cultivation/known_techniques.rs` + `skill_registry.rs`）
- `UseQuickSlot` hotbar 施放 ✅（`plan-hotbar-modify-v1`）
- `VfxEventRouter` + `VfxEventAnimationBridge` ✅（client VFX 管线）

**反向被依赖**：
- 未来"剑道进阶"plan（通灵+高阶剑术，依赖本 plan 作为基础层）
- 未来"刀道/枪道/拳道"基础武技 plan（同层并列，参考本 plan 结构）

---

## 接入面 Checklist

- **进料**：
  - `combat::weapon::Weapon { weapon_kind: Sword, base_attack, quality_tier, durability }` — 持剑判定 + 伤害基数
  - `combat::events::AttackIntent` — 攻击意图提交
  - `cultivation::known_techniques::KnownTechniques` — 已学招式 + proficiency 读取
  - `cultivation::skill_registry::SkillRegistry` — 招式函数注册
  - `cultivation::components::Cultivation { qi_current, qi_max }` — 真元池（可选灌注）
  - `combat::components::Stamina { current, max, recover_per_sec }` — 体力池（三式主消耗资源）
- **出料**：
  - `combat::events::CombatEvent` — 命中结算事件（劈/刺）
  - `combat::events::StatusEffectKind::Parried` — 格挡成功状态（格）
  - `network::VfxEventPayloadV1` — 动画 + 粒子 + 音效指令下发 client
  - `schema::combat_hud::TechniqueEntryV1` — HUD 招式信息同步
  - `schema::combat_hud::CastSyncV1` — 施法阶段同步（格的时机窗口）
- **共享类型/event**：
  - **复用** `AttackIntent`（新增 `AttackSource::SwordCleave` / `SwordThrust` 变体）
  - **复用** `CastPhaseV1` / `CastSyncV1`（格的时机窗口）
  - **复用** `KnownTechnique.proficiency`（不新建等级系统）
  - **复用** `UseQuickSlot`（hotbar 施放）
  - **新增** 4 个 `TechniqueDefinition` 条目（`sword.cleave` / `sword.thrust` / `sword.parry` / `sword.infuse`）
  - **新增** `SwordQiStore` 临时 component（附着在 Weapon 上，持续时间内提供 qi_invest）
  - **改动** `CombatEvent` 增加 `physical_damage: f32` 字段（纯物理分支产出）
- **跨仓库契约**：
  - server: `sword_basics` 模块 / 4 个 technique fn / `AttackSource::SwordCleave|SwordThrust` / `StatusEffectKind::SwordParrying` / `SwordQiStore` component / resolve.rs 纯物理分支
  - client: 4 个 animation ID（`bong:sword_cleave` / `bong:sword_thrust` / `bong:sword_parry` / `bong:sword_infuse`）/ 4 个 audio_recipe / 3 个 VfxPlayer 类（含注剑微光）
  - agent: 无——凡人武技不需要天道叙事
- **worldview 锚点**：§四 距离衰减（近战） + §五 流派无门禁 + §五 凡器边界 + §六.2 锋锐色（长期副产物）
- **qi_physics 锚点**：劈/刺的可选真元灌注走 `QiTransfer`。**主消耗是 `Stamina`（体力），不是真元**——三式是物理武技。无新增物理量

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | resolve.rs 纯物理伤害分支 + 三式定义注册 + 战斗接入 + proficiency + 视听 | ⬜ |
| P1 | 注剑（Sword Qi Infusion）——独立真元灌注技 + 视听 | ⬜ |
| P2 | proficiency 影响曲线（5 档可见等级 + 数值缩放） | ⬜ |
| P3 | 饱和测试（物理伤害 + 三式交互 + 注剑 + proficiency + 守恒） | ⬜ |

---

## P0 — 纯物理伤害分支 + 三式定义 + 视听

### 前置：resolve.rs 纯物理伤害分支

当前 `resolve.rs:254` 的 `if intent.qi_invest <= 0.0 { continue; }` 导致无真元的攻击完全不结算。这对凡人武技不合理——worldview §五:411 明确说凡器"只给低基础伤害"，不是零伤害。

**改动**：在 `resolve_attack_intents()` 中，当 `qi_invest <= 0.0` 时走**纯物理分支**，不 `continue`：

```rust
// resolve.rs 伪代码

if intent.qi_invest <= 0.0 {
    // --- 纯物理分支（新增）---
    // 无真元消耗、无 contamination、无经脉 throughput
    // 仅走 weapon.base_attack × body_part × wound_profile → 物理 wounds
    let physical_damage = weapon_base_damage(intent, &weapon_query)
        * body_part_multipliers(hit_probe.body_part).0
        * wound_kind_profile(intent.wound_kind).damage_multiplier;
    // 直接写入 Wounds component，跳过 contamination / qi deduction
    apply_physical_wounds(&mut wounds, &mut stamina, physical_damage, hit_probe, intent.wound_kind);
    emit_combat_event(/* ... qi_damage: 0, physical_damage, contamination_delta: 0 */);
    continue;
}
// --- 原有真元分支不变 ---
```

**关键约束**：
- 纯物理分支**不扣攻击方真元**、**不产生 contamination**、**不走经脉 throughput**
- 伤害来源 = `weapon.base_attack × weapon.damage_multiplier() × body_part × wound_profile`
- 仅产生体表伤口（wounds），不影响经脉层和真元层
- `CombatEvent` 增加 `physical_damage: f32` 字段（现有 `damage` 字段 = qi 伤害，两者并列）

**影响范围**：此改动惠及所有未来纯物理攻击（拳道/枪道/凡器搏斗/NPC 凡兽攻击），不只是剑道三式。

**测试**：
- qi_invest=0 + 持剑命中 → 产生 physical_damage > 0 + contamination_delta = 0
- qi_invest=0 + 空手命中 → 产生 physical_damage = fist_base_damage
- qi_invest > 0 → 行为不变（原有真元分支）

---

### 通用设计

```rust
// server/src/combat/sword_basics.rs

// 三式共通：
// - required_realm: 0（醒灵即可学）
// - required_meridians: vec![]（纯物理武技，不走经脉灌注）
//   → 但持剑手 SEVERED 时 weapon 系统已自动禁用主手槽
//     （worldview §四:254 "断了右臂别想再握剑"）
// - 必须主手持 WeaponKind::Sword
// - **主消耗 = 体力（Stamina）**，不是真元
//   → 基础体力消耗见各式定义（劈 > 格 > 刺）
//   → 体力不足时仍可施放但进入 Exhausted 后禁止
//   → 已有 ATTACK_STAMINA_COST = 3.0 是普通攻击的消耗，三式各自覆盖
// - qi_invest: 0.0（三式本身不注入真元）
//   → 真元灌注由独立技能"注剑"处理（P1）
//   → 注剑后剑上有存储真元，命中时自动附带 contamination
//   → 未注剑时纯物理伤害（需 P0 的 resolve.rs 物理分支）
// - proficiency 0-100，每次成功使用 +1（命中 +1，格挡成功 +2）
//   → diminishing：proficiency > 80 后每次 +0.5
```

### 经脉依赖声明

```rust
// 三式均为纯物理武技，不需要经脉灌注。依赖列表为空。
// 持剑手 SEVERED 由 weapon 系统的 sync_weapon_component_from_equipped() 拦截
// （整条手臂链路 SEVERED → 禁用对应主手槽 → 无 Weapon component → cast 前置检查失败）
SkillMeridianDependencies::declare("sword.cleave", vec![]);
SkillMeridianDependencies::declare("sword.thrust", vec![]);
SkillMeridianDependencies::declare("sword.parry", vec![]);
```

### PracticeLog 声明

剑道三式**不调用 `PracticeLog.add()`**。理由：三式是跨流派基础武技，不属于七大流派任何一个。长期练剑导致的锋锐色染色应由未来的"剑道进阶"plan 处理（高阶剑术需要真元灌注，那时才产生染色倾向）。

---

### 第一式——劈（Cleave）

> 最朴素的一刀。举过头顶，劈下去。没有花哨，只有份量。

#### 机制

```rust
TechniqueDefinition {
    id: "sword.cleave",
    display_name: "劈",
    grade: "common",
    description: "基础劈砍。举剑过顶，顺势劈下。",
    required_realm: 0,
    required_meridians: vec![],
    qi_cost: 0.0,
    stamina_cost: 8.0,                 // 三式中最费体力——大开大合的重劈
    cast_ticks: 16,                    // 0.8 秒（proficiency 可缩短到 10 tick）
    cooldown_ticks: 30,                // 1.5 秒
    range: 3.0,                        // 剑的物理长度，近身
    icon_texture: "bong:sword_cleave",
}
```

**伤害结算**：
- `base_damage = weapon.base_attack × weapon.damage_multiplier() × cleave_proficiency_factor`
- `cleave_proficiency_factor = 1.0 + (proficiency / 100.0) × 0.3`（prof 0 = ×1.0，prof 100 = ×1.3）
- 命中部位偏好：头/肩/胸（从上往下劈的物理路径）

**AttackSource 变体**：`AttackSource::SwordCleave`

**TechniqueDefinition 新增字段**：`stamina_cost: f32`（当前 struct 无此字段，P0 需新增——影响所有未来基于体力的技能）

#### 动画 `bong:sword_cleave`

```json
{
  "format": "bong_player_animator",
  "id": "bong:sword_cleave",
  "endTick": 16,
  "keyframes": [
    {
      "tick": 0,
      "bones": {
        "right_upper_arm": { "pitch": 0.0, "yaw": 0.0, "roll": 0.0 },
        "right_forearm": { "pitch": 0.0 },
        "torso": { "pitch": 0.0 }
      }
    },
    {
      "tick": 5,
      "easing": "ease-out-cubic",
      "bones": {
        "right_upper_arm": { "pitch": -1.4, "yaw": 0.1, "roll": 0.0 },
        "right_forearm": { "pitch": -0.6 },
        "torso": { "pitch": -0.08 }
      },
      "body": { "z": -0.05 }
    },
    {
      "tick": 8,
      "easing": "ease-in-quad",
      "bones": {
        "right_upper_arm": { "pitch": 0.7, "yaw": -0.05, "roll": 0.0 },
        "right_forearm": { "pitch": 0.3 },
        "torso": { "pitch": 0.15 }
      },
      "body": { "z": 0.15 }
    },
    {
      "tick": 16,
      "easing": "ease-out-quad",
      "bones": {
        "right_upper_arm": { "pitch": 0.0 },
        "right_forearm": { "pitch": 0.0 },
        "torso": { "pitch": 0.0 }
      },
      "body": { "z": 0.0 }
    }
  ]
}
```

举臂（0-5 tick）→ 劈下（5-8 tick，加速）→ 收势（8-16 tick）。torso 配合前倾补偿（见 `feedback_torso_legs_hinge`：torso+legs 同向 pitch + body.z 前移）。

#### 粒子

- `bong:vfx_event` ID = `bong:sword_cleave_trail`
- `SwordCleaveTrailPlayer.java`：劈下瞬间（tick 5-8）`BongLineParticle` ×1
  - 起点：剑尖初始位 y+1.8，终点：剑尖终点位 y+0.5
  - 颜色 `#C0C0C8`（银白），width 1px，lifetime 4 tick，fade-out linear
  - spawn 模式：burst（劈下那一帧生成）
- 命中时：`BongSpriteParticle` ×3，颜色 `#E0D0C0`（骨白微黄），burst 半径 0.3 格，speed 0.05，lifetime 6 tick

#### 音效 `sword_cleave.json`

```json
{ "id": "sword_cleave", "layers": [
  { "sound": "entity.player.attack.sweep", "pitch": 0.9, "volume": 0.6, "delay_ticks": 5 },
  { "sound": "entity.player.attack.strong", "pitch": 0.8, "volume": 0.5, "delay_ticks": 8, "condition": "hit" }
]}
```

挥剑风声（tick 5 劈下瞬间）+ 命中肉声（tick 8 判定帧，仅命中时播放）。

---

### 第二式——刺（Thrust）

> 最快的一剑。收肘，蓄力半拍，捅出去。

#### 机制

```rust
TechniqueDefinition {
    id: "sword.thrust",
    display_name: "刺",
    grade: "common",
    description: "基础突刺。收肘蓄力，直线捅出。",
    required_realm: 0,
    required_meridians: vec![],
    qi_cost: 0.0,
    stamina_cost: 4.0,
    cast_ticks: 10,
    cooldown_ticks: 20,
    range: 3.5,
    icon_texture: "bong:sword_thrust",
}
```

**伤害结算**：
- `base_damage = weapon.base_attack × weapon.damage_multiplier() × 0.75 × thrust_proficiency_factor`
- 基础倍率 ×0.75（比劈低——快但轻）
- `thrust_proficiency_factor = 1.0 + (proficiency / 100.0) × 0.25`（prof 100 = ×1.25）
- 命中部位偏好：胸/腹/手臂（水平刺击的物理路径）
- **穿甲微加成**：`armor_penetration += 0.05`

**AttackSource 变体**：`AttackSource::SwordThrust`

#### 动画 `bong:sword_thrust`

```json
{
  "format": "bong_player_animator",
  "id": "bong:sword_thrust",
  "endTick": 10,
  "keyframes": [
    { "tick": 0, "bones": { "right_upper_arm": { "pitch": 0.0, "yaw": 0.0 }, "right_forearm": { "pitch": 0.0 }, "torso": { "pitch": 0.0 } } },
    { "tick": 3, "easing": "ease-out-cubic", "bones": { "right_upper_arm": { "pitch": 0.3, "yaw": 0.2 }, "right_forearm": { "pitch": 0.6 }, "torso": { "pitch": -0.05 } }, "body": { "z": -0.1 } },
    { "tick": 5, "easing": "ease-in-cubic", "bones": { "right_upper_arm": { "pitch": -0.15, "yaw": -0.05 }, "right_forearm": { "pitch": -0.4 }, "torso": { "pitch": 0.12 } }, "body": { "z": 0.3 } },
    { "tick": 10, "easing": "ease-out-quad", "bones": { "right_upper_arm": { "pitch": 0.0 }, "right_forearm": { "pitch": 0.0 }, "torso": { "pitch": 0.0 } }, "body": { "z": 0.0 } }
  ]
}
```

收肘蓄力（0-3 tick）→ 突刺（3-5 tick，body.z 前冲 0.3）→ 收回（5-10 tick）。

#### 粒子

- 无拖尾粒子——刺太快
- 命中时：`BongSpriteParticle` ×2，颜色 `#C03030`（暗红），从命中点向后 burst，speed 0.03，lifetime 4 tick

#### 音效 `sword_thrust.json`

```json
{ "id": "sword_thrust", "layers": [
  { "sound": "entity.player.attack.knockback", "pitch": 1.2, "volume": 0.5, "delay_ticks": 3 },
  { "sound": "entity.player.attack.strong", "pitch": 1.1, "volume": 0.4, "delay_ticks": 5, "condition": "hit" }
]}
```

---

### 第三式——格（Parry）

> 用剑身挡住来路。时机对了，对方的力气反噬回去。

#### 机制

```rust
TechniqueDefinition {
    id: "sword.parry",
    display_name: "格",
    grade: "common",
    description: "基础格挡。以剑身格开来袭，时机精准可反震对手。",
    required_realm: 0,
    required_meridians: vec![],
    qi_cost: 0.0,
    stamina_cost: 6.0,
    cast_ticks: 4,
    cooldown_ticks: 40,
    range: 0.0,
    icon_texture: "bong:sword_parry",
}
```

**格挡窗口**：
- 按下后进入 `SwordParrying` 状态，持续 `parry_window_ticks`
- `parry_window_ticks = 4 + floor(proficiency / 25)`（prof 0 = 4 tick，prof 100 = 8 tick）

**格挡成功效果**：
- 格挡方：受到伤害 ×`(1.0 - block_ratio)`
- `block_ratio = 0.3 + (proficiency / 100.0) × 0.3`（prof 0 格 30%，prof 100 格 60%）
- 攻击方：受到 `reflected_damage = blocked_damage × 0.15`
- 攻击方附加 `StatusEffectKind::Staggered`（duration 10 tick，移速 -30%）——已有 `ParryRecovery` 变体可评估复用

**StatusEffectKind 新增变体**：`SwordParrying`（格挡窗口中）、`Staggered`（被格后短暂硬直，评估复用已有 `ParryRecovery`）

#### 动画 `bong:sword_parry`

```json
{
  "format": "bong_player_animator",
  "id": "bong:sword_parry",
  "endTick": 12,
  "keyframes": [
    { "tick": 0, "bones": { "right_upper_arm": { "pitch": 0.0, "yaw": 0.0 }, "right_forearm": { "pitch": 0.0 }, "left_upper_arm": { "pitch": 0.0 }, "torso": { "pitch": 0.0 } } },
    { "tick": 4, "easing": "ease-out-cubic", "bones": { "right_upper_arm": { "pitch": -0.3, "yaw": 0.5 }, "right_forearm": { "pitch": -0.5 }, "left_upper_arm": { "pitch": -0.2, "yaw": -0.3 }, "torso": { "pitch": 0.0, "yaw": 0.15 } }, "body": { "z": -0.05 } },
    { "tick": 12, "easing": "ease-out-quad", "bones": { "right_upper_arm": { "pitch": 0.0, "yaw": 0.0 }, "right_forearm": { "pitch": 0.0 }, "left_upper_arm": { "pitch": 0.0 }, "torso": { "pitch": 0.0, "yaw": 0.0 } }, "body": { "z": 0.0 } }
  ]
}
```

#### 粒子

- 格挡成功时 `bong:vfx_event` ID = `bong:sword_parry_spark`
- `SwordParrySparkPlayer.java`：`BongSpriteParticle` ×4 `#FFD080`（橙白火星）radial burst speed 0.08 lifetime 3 tick + ×1 `#FFFFFF`（白闪）size 0.5 格 lifetime 2 tick

#### 音效 `sword_parry.json`

```json
{ "id": "sword_parry", "layers": [
  { "sound": "block.anvil.hit", "pitch": 1.3, "volume": 0.5, "delay_ticks": 0 },
  { "sound": "entity.player.attack.crit", "pitch": 0.7, "volume": 0.4, "delay_ticks": 1, "condition": "parry_success" },
  { "sound": "entity.iron_golem.hurt", "pitch": 1.5, "volume": 0.3, "delay_ticks": 0, "condition": "parry_fail" }
]}
```

---

### 学习获取

1. **拾取残卷**：荒野遗迹/NPC 商人处获得「基础剑术残卷」→ 三式同时解锁，proficiency 均为 0
2. **NPC 教授**：出生点附近"老剑客"NPC → 对话后解锁
3. **观摩学习**（v2 候选）：目睹其他修士使用剑道三式 N 次后自动解锁

---

## P1 — 注剑（Sword Qi Infusion）

> 提前把真元灌进剑身。剑在鞘中也行，手中也行。灌好了，下几刀就带真元注入。

### 机制

```rust
TechniqueDefinition {
    id: "sword.infuse",
    display_name: "注剑",
    grade: "common",
    description: "将真元注入剑身。持续期间命中附带真元污染。",
    required_realm: 1,                 // 引气起
    required_meridians: vec![],
    qi_cost: 0.0,                      // 实际消耗在 infuse_amount 中动态扣
    stamina_cost: 3.0,
    cast_ticks: 40,                    // 2 秒引导（可被打断）
    cooldown_ticks: 100,               // 5 秒 CD
    range: 0.0,
    icon_texture: "bong:sword_infuse",
}
```

### 注剑流程

1. 施放条件：主手或副手有 `WeaponKind::Sword`
2. 引导 2 秒：`CastPhaseV1::Casting`，被命中会打断
3. 引导完成：从 `qi_current` 扣除 `infuse_amount`，走 `QiTransfer { from: player, to: weapon_qi_store }`
4. 剑获得 `SwordQiStore` 状态

```rust
pub struct SwordQiStore {
    pub stored_qi: f64,
    pub qi_per_hit: f64,         // = infuse_amount / 5（管 5 刀）
    pub remaining_ticks: u64,    // 60 秒（1200 tick）
    pub infuser_color: QiColor,
}
```

### 数值设计

```
infuse_amount：最小 5 点，最大 qi_current × 0.5
qi_per_hit = infuse_amount / 5
remaining_ticks = 1200（60 秒）

剑身真元逸散（存储损耗）：
  注入后 SwordQiStore.stored_qi 每 tick 按 qi_physics::excretion 逸散衰减。
  衰减速率取决于剑的材质，carrier grade 由 quality_tier 映射：

  | quality_tier | qi_physics CarrierGrade | 逸散特征 |
  |---|---|---|
  | 0（凡铁） | BareQi | 基线逸散——60 秒内大量流失 |
  | 1（灵器） | SpiritWeapon | loss -0.006/格——逸散慢 |
  | 2（法宝） | AncientRelic | loss -0.012/格——法宝级封存 |
  | 3（仙器） | AncientRelic | 同法宝级 |

  凡铁剑注剑后必须尽快用完。逸散走 QiTransfer 归还 zone（守恒）。
```

### 命中时的 qi_invest 注入

三式命中时，如果持剑有 `SwordQiStore` 且 `stored_qi > 0`：
- `AttackIntent.qi_invest = min(qi_per_hit, stored_qi)`
- `SwordQiStore.stored_qi -= qi_invest`
- 走原有 resolve.rs 真元分支 → contamination

`SwordQiStore` 为空或过期 → `qi_invest = 0` → 走 P0 物理分支。

### 视听

**引导动画** `bong:sword_infuse`（endTick: 40，双手扶剑身 10 tick → 维持 30 tick → 收回）

**引导粒子**：剑身 `BongRibbonParticle` ×1，沿剑身流动，颜色取 `infuser_color` hex，width 2px，lifetime = cast_ticks，continuous

**注剑后微光**：`BongSpriteParticle` ×1/秒，贴附剑身，opacity 0.3，lifetime 20 tick——**所有人可见**

**音效** `sword_infuse.json`：
```json
{ "id": "sword_infuse", "layers": [
  { "sound": "block.amethyst_block.chime", "pitch": 0.6, "volume": 0.4, "delay_ticks": 10 },
  { "sound": "block.amethyst_cluster.step", "pitch": 0.8, "volume": 0.3, "delay_ticks": 20, "loop_until": 40 }
]}
```

---

## P2 — Proficiency 影响曲线

### 五档可见等级

| 档 | proficiency | 标签 |
|----|------------|------|
| 1 | 0-19 | 生疏 |
| 2 | 20-49 | 入门 |
| 3 | 50-79 | 熟练 |
| 4 | 80-94 | 精通 |
| 5 | 95-100 | 化境 |

### proficiency 获取

```
命中 / 格挡成功：prof<50 → +1.0 / 50-79 → +0.5 / 80-94 → +0.3 / ≥95 → +0.1
格挡成功额外 +0.5
未命中 / 格挡失败：+0.2
越级命中额外 +0.5
```

### 各式 proficiency 缩放

**劈**：

| 属性 | prof 0 | prof 50 | prof 100 |
|------|--------|---------|----------|
| stamina_cost | 8.0 | 6.5 | 5.0 |
| cast_ticks | 16 | 13 | 10 |
| cooldown_ticks | 30 | 26 | 22 |
| damage_multiplier | ×1.0 | ×1.15 | ×1.3 |

**刺**：

| 属性 | prof 0 | prof 50 | prof 100 |
|------|--------|---------|----------|
| stamina_cost | 4.0 | 3.0 | 2.0 |
| cast_ticks | 10 | 8 | 7 |
| cooldown_ticks | 20 | 17 | 14 |
| damage_multiplier | ×0.75 | ×0.84 | ×0.94 |
| range | 3.5 | 3.75 | 4.0 |

**格**：

| 属性 | prof 0 | prof 50 | prof 100 |
|------|--------|---------|----------|
| stamina_cost | 6.0 | 5.0 | 4.0 |
| parry_window_ticks | 4 | 6 | 8 |
| block_ratio | 30% | 45% | 60% |
| reflected_damage | 15% | 15% | 15% |
| cooldown_ticks | 40 | 35 | 30 |

**体力节奏**：Stamina max=100，recover 5/s。化境劈(5)+刺(2)=7 体力 1.4s 回满；生疏劈(8)+刺(4)=12 体力 2.4s 回满。Exhausted 禁止施放。

### Proficiency 不重置

- 死亡重生 proficiency **不归零**（肌肉记忆是身体物理变化）
- **提案，需确认**：与 plan-skill-v1 "死透 skill 归零" 有张力（不同系统）

---

## P3 — 饱和测试

### 基础功能
1. 持剑 sword.cleave → AttackIntent → CombatEvent 结算
2. 非剑武器 → 拒绝
3. 空手 → 拒绝
4. 断臂 → 拒绝

### 三式交互
5. 劈→刺连招（独立 CD）
6. 格后反击窗口（Staggered 10 tick ≈ 刺 cast 10 tick）
7. 格挡窗口边界（第 1 tick / 最后 1 tick / 窗口后 1 tick）
8. 双方同时格

### Proficiency 递增
9-14. 命中 +1.0 / 未命中 +0.2 / 格挡成功 +1.5 / 越级 +0.5 / 衰减区间 / 化境缓慢

### Proficiency 缩放
15-18. 劈 cast_ticks / 刺 range / 格 parry_window / 格 block_ratio

### 体力消耗
19. 劈 stamina 100→92
20. 刺 stamina 100→96
21. 格 stamina 100→94
22. Exhausted 拒绝
23. 体力不足但未 Exhausted → 允许 → Exhausted
24. proficiency 减耗
25. 连招压力
26. 体力恢复

### 注剑
27. 注剑引导 → SwordQiStore
28. 注剑扣真元
29. 凡铁持续逸散
30. 灵器逸散慢
31. 命中消耗存储
32. 5 刀用完 → 物理分支
33. 超时逸散
34. 引导被打断
35. 无剑拒绝
36. 醒灵拒绝
37. 注剑后发光可见

### 守恒断言
38. 注剑 qi 走 QiTransfer
39. 凡铁持续逸散走 excretion
40. 命中 contamination 走 QiTransfer
41. 超时逸散走 excretion
42. 格挡不凭空消灭伤害
43. reflected_damage 是物理

### 视听验证
44. 劈动画
45. 劈命中粒子
46. 刺音效
47. 格火星
48. 三式音效区分
49. 三式动画区分
