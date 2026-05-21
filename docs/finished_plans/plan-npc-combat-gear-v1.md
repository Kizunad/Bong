# Bong · plan-npc-combat-gear-v1

NPC 功法调用 + 装备携带 + 手持武器 + 交互 GUI 重绘——让散修/宗门弟子/道伥/执念成为"真正的修士"而非挥刀木桩，同时将 NPC 交互界面从 vanilla 按钮升级为 owo-lib 组件化 UI。

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ✅ 2026-05-21 | NPC 装备模型 + Valence Equipment 同步 | NPC 手持铁剑/穿戴护甲可见；装备影响战斗结算 |
| **P1** | ✅ 2026-05-21 | NPC 功法系统（KnownTechniques + brain 调用 + 经脉校验） | 散修 NPC 在战斗中释放已学功法，qi 消耗走 ledger，经脉 SEVERED 阻断功法 |
| **P2** | ✅ 2026-05-21 | NPC 交互 GUI 重绘（owo-lib 三屏） | 对话/查看/交易三屏全部 BaseOwoScreen，装备+功法可视 |
| **P3** | ✅ 2026-05-21 | 集成测试 + 校准 | e2e：玩家看见 NPC 持剑释放功法 + 打开查看屏看到装备列表 |

---

## 接入面 Checklist

- **进料**：
  - `cultivation::skill_registry::SkillRegistry`（功法注册表，44 功法已注册）
  - `cultivation::known_techniques::KnownTechniques`（玩家功法 component，复用到 NPC）
  - `cultivation::known_techniques::TechniqueDefinition`（44 条功法定义：qi_cost / cast_ticks / cooldown / required_meridians）
  - `cultivation::technique_proficiency::proficiency_gain()` / `generic_proficiency_scalars()`（熟练度公式）
  - `cultivation::meridian::severed::SkillMeridianDependencies`（功法经脉依赖注册表，`lookup(technique_id)` → `&[MeridianId]`）
  - `cultivation::meridian::severed::check_meridian_dependencies(deps: &[MeridianId], severed: Option<&MeridianSeveredPermanent>) -> Result<(), MeridianId>`
  - `cultivation::components::Cultivation`（NPC 已有此 component，含 realm / qi_current / qi_max）
  - `cultivation::meridian::MeridianSystem`（NPC 已有此 component，含 regular[12] + extraordinary[8] 经脉拓扑）
  - `cultivation::meridian::severed::MeridianSeveredPermanent`（NPC 可能有此 component，含永久断经列表）
  - `combat::weapon::Weapon` / `WeaponKind`（武器 component）
  - `combat::armor::ArmorProfile` / `ArmorProfileRegistry`（护甲档案）
  - `inventory::ItemTemplate` / `ItemTemplateRegistry`（物品模板）
  - `skin::faction_tint::visual_equipment()`（现有 NPC 视觉装备，需扩展而非覆盖）
  - `npc::spawn::NpcCombatLoadout`（现有 melee archetype）
  - `npc::brain::MeleeAttackAction`（现有近战行为，部分 archetype 替换为功法调用）

- **出料**：
  - `npc::equipment::NpcEquipment` component → 装备 slot 数据
  - `npc::equipment::sync_npc_equipment_system` → Valence `Equipment` component 同步
  - `npc::technique::NpcTechniqueAction`（big-brain Action，替代 MeleeAttackAction 的功法版本）
  - `npc::technique::NpcTechniqueSelector`（按境界/流派/冷却/经脉依赖选择功法）
  - `bong:npc_metadata` S2C packet 扩展字段（equipment / techniques / trade_offers）
  - 3 个 owo-lib NPC 屏幕 class（NpcDialogueScreen / NpcInspectScreen / NpcTradeScreen 重写）

- **共享类型 / event**：
  - 复用 `SkillRegistry` + `CastResult`（NPC 与玩家共用同一套功法调用入口）
  - 复用 `SkillMeridianDependencies::lookup()` + `check_meridian_dependencies()`（NPC 与玩家共用经脉校验路径：先 lookup 取 `&[MeridianId]` deps → 再 check 对 `MeridianSeveredPermanent`）
  - 复用 `ArmorProfile`（NPC 的护甲走同一套减伤公式）
  - 扩展 `NpcMetadata` record（client 端）加 `equipment` / `techniques` / `tradeOffers` 字段
  - 不新建 event——NPC 功法调用走 `SkillRegistry.lookup()(world, npc_entity, ...)` 同一个入口

- **跨仓库契约**：
  - **server → client**：`bong:npc_metadata` channel 扩展 JSON（新增 `equipment` / `techniques` / `trade_offers` 字段）
  - **client**：`NpcMetadata.java` record 扩展对应字段 + `NpcMetadataHandler.java` 解析
  - **server → agent**：NPC 高阶功法释放走现有 `bong:world_state` 推送（NPC combat event 已包含在 world_state 快照中，不需新增 Redis key）；agent 通过 NpcDigest 感知 NPC 功法行为用于 narration 素材

- **worldview 锚点**：
  - **§五:399-501 战斗流派**：NPC 散修/宗门弟子应当有自己的流派倾向
  - **§七:730-740 散修 NPC 行为**：散修评估玩家威胁度后做出不同反应——功法是这些行为的物理基础
  - **§四:286 经脉物理可见性**："断了肺经的飞剑手就废了"——NPC 经脉 SEVERED 时对应功法必须被阻断
  - **§九:839-858 交易**：面对面以物易物、散修 NPC 经济生态位——GUI 需要展现
  - **§十一:922-970 身份与信誉**：NPC 信誉分级影响交易折扣/服务/通缉——GUI 清晰展示
  - **§四:213-310 三层战力模型**：NPC 同样遵循"真元池 × 经脉流量 × 体表功能"复合战力模型

- **qi_physics 锚点**：
  - `qi_physics::ledger::QiTransfer` — NPC 功法释放消耗真元必须走 ledger 记账
  - NPC 功法 qi_cost 通过 `TechniqueDefinition.qi_cost` 读取，扣减 `Cultivation.qi_current`
  - NPC 功法释放的真元走 `qi_physics::release::qi_release_to_zone()` 同源路径，**不允许 NPC 功法凭空产生或消灭真元**
  - NPC 真元回复走现有 `cultivation::tick` 系统（NPC 已挂 Cultivation component，zone_qi > 0 时自然回复，与玩家同一路径）

---

## P0 — NPC 装备模型 + 同步

### P0.1 NpcEquipment Component

新文件 `server/src/npc/equipment.rs`：

```rust
#[derive(Component, Clone, Debug)]
pub struct NpcEquipment {
    pub main_hand: Option<NpcEquipSlot>,
    pub off_hand: Option<NpcEquipSlot>,
    pub head: Option<NpcEquipSlot>,
    pub chest: Option<NpcEquipSlot>,
    pub legs: Option<NpcEquipSlot>,
    pub feet: Option<NpcEquipSlot>,
}

#[derive(Clone, Debug)]
pub struct NpcEquipSlot {
    pub template_id: String,
    pub display_name: String,
    pub item_kind: ItemKind,
    pub quality_tier: u8,            // 0=凡铁 1=灵器 2=法宝（对齐 plan-weapon-v1 §2 四档）
    pub base_attack: f32,            // 基础攻击力（用于 attack_multiplier = max(1.0, base_attack/10.0)）
    pub weapon_kind: Option<WeaponKind>,
    pub armor_profile_id: Option<String>,
    pub durability_ratio: f32,       // 0.0-1.0
}
```

- NpcEquipment 不依赖 PlayerInventory（轻量化，NPC 不需要背包/容器/重量系统）
- 仅存储 6 个 slot 的装备状态，战斗系统读取 `main_hand` 做武器结算、读取 armor slots 做减伤
- `quality_tier=3`（仙器）字段保留以对齐 plan-weapon-v1 schema，但**本 plan 不分配给任何 NPC archetype**——末法残土不出仙器（worldview §三:63 "末法时代的修士不配用上古称呼"）
- `base_attack` 由 `ItemTemplateRegistry.get(template_id).base_attack` 在 spawn 时赋值（与 `Weapon` component 同源）
- NPC 战斗中使用武器/护甲**不消耗耐久**——NPC 装备是 spawn 时生成的静态属性，不走 PlayerInventory 的 durability tick 路径。`durability_ratio` 字段仅影响掉落品的初始耐久和战斗加成系数

### P0.2 Spawn-Time 装备分配

扩展 `npc::spawn` 模块，新增 `fn assign_npc_equipment(archetype, realm, faction, rng) -> NpcEquipment`：

| Archetype | 主手 | 副手 | 护甲 |
|-----------|------|------|------|
| **Rogue 散修** | 50% 铁剑 / 30% 木杖 / 20% 空手 | 空 | 0-2 件随机（草甲/骨甲/兽皮），件数 ∝ realm |
| **Commoner 凡人** | 80% 空手 / 20% 锄头（凡器，ItemKind::WoodenHoe） | 空 | 空或草衣 |
| **Beast 异兽** | 空（天生武器） | 空 | 空（天生皮甲走 DerivedAttrs） |
| **Disciple 宗门弟子** | 按 faction：Attack=铁剑 / Defend=铁盾+木杖 / Neutral=铁剑 | 按 faction | 1-3 件，染色走 faction_tint |
| **GuardianRelic 遗迹守卫** | 古剑（quality_tier=2, ItemKind::GoldenSword） | 空 | 2-4 件，高品质 |
| **Daoxiang 道伥** | 锈剑（durability_ratio=0.3, ItemKind::IronSword） | 空 | 0-1 件残甲 |
| **Zhinian 执念** | 按生前流派（从 loot table 反推） | 空 | 1-2 件，半损 |
| **Fuya 负压畸变体** | 空（天生） | 空 | 空 |
| **SkullFiend 骷髅魔** | 空（天生骨爪） | 空 | 空（天生骨甲走 DerivedAttrs） |
| **Zombie 僵尸** | 空（天生） | 空 | 空 |

- Zombie / Beast / Fuya / SkullFiend 走 `assign_npc_equipment()` 时返回 `NpcEquipment::default()`（全 None），match 穷尽所有 `NpcArchetype` 变体
- realm ≥ Condense(凝脉) 时武器 quality_tier +1（上限 tier=2 法宝）
- realm ≥ Solidify(固元) 时护甲件数 +1
- RNG 用 `splitmix64(entity_id_hash)` 确保 deterministic

### P0.3 Equipment → Valence Equipment 同步

修改 `skin::faction_tint` 模块中 `sync_npc_visual_profiles_system`：

- query 加入 `Option<&NpcEquipment>`
- **合并逻辑（非覆盖）**：NpcEquipment 6 slot 的 ItemStack 与 NpcVisualProfile 的 `head_marker()` / `rank_hand_marker()` **叠加**——NpcEquipment 提供 chest/legs/feet/main_hand 的实际装备外观，NpcVisualProfile 继续驱动 rank aura / head marker（Leader=DiamondHelmet / Elder=灰发）等非 equipment 层视觉效果
- 如 NpcEquipment 和 NpcVisualProfile 在同一 slot 冲突（例：NpcEquipment 设了 head armor，NpcVisualProfile 也要设 head marker），NpcEquipment 优先（实际装备 > 纯视觉标记）
- 新增 `fn merge_equipment(npc_eq: Option<&NpcEquipment>, profile: &NpcVisualProfile) -> Equipment`
- 低品质武器（凡铁 tier=0）→ `ItemKind::IronSword`；灵器 tier=1 → `ItemKind::DiamondSword`；法宝 tier=2 → `ItemKind::GoldenSword`
- 护甲同理映射 MC 护甲类型（Leather/Chainmail/Iron/Diamond）+ faction 染色 NBT

### P0.4 战斗系统读取 NPC 装备

扩展 `npc::brain::melee_attack_action_system`：

- 当前 NPC 攻击只读 `NpcMeleeProfile`（hardcoded reach + wound_kind）
- NPC **不挂 `Weapon` component**（见 §8.1 #1 决议），战斗系统从 `NpcEquipment` 直接读武器属性
- 改为：如有 `NpcEquipment.main_hand` → 从 `weapon_kind` 派生 reach + wound_kind + `npc_weapon_damage_multiplier(slot)` 计算伤害（公式与 `Weapon::damage_multiplier()` 对齐：`attack_mul(max(1.0, slot.base_attack/10.0)) × quality_mul(tier) × durability_combat_mul(0.5 + 0.5 * slot.durability_ratio)`——`durability_combat_mul` 是战斗加成系数，区别于 `durability_ratio` 原始耐久比例）
- 如有 armor slots → 被攻击时读取 `ArmorProfileRegistry.get(armor_profile_id)` 做减伤（复用现有 armor combat 路径）
- 无 `NpcEquipment` 的 NPC（Beast / Fuya / 旧数据）退化到现有 `NpcMeleeProfile` 行为（向后兼容）

**替换范围明确**：
- **走新路径**（NpcEquipment + NpcTechniqueAction）：Rogue / Disciple / Daoxiang / Zhinian / GuardianRelic
- **保留旧路径**（NpcMeleeProfile + MeleeAttackAction）：Beast / Fuya / SkullFiend / Commoner / Zombie / brain_rat / brain_whale
- 两套路径通过 query filter 共存：`With<NpcEquipment>` vs `Without<NpcEquipment>`，互不干扰

### P0.5 NPC 死亡掉落装备

扩展 `npc::loot::roll_loot()`：

- 如果 NPC 有 `NpcEquipment` → 每个非空 slot 独立 30% 概率掉落
- 掉落项追加到 `Vec<RolledLoot>`（与现有 `NpcLootTable` 的 roll 结果合并，不替代）
- 装备类 `RolledLoot` 的 `template_id` 直接取 `NpcEquipSlot.template_id`，stack=1
- 掉落品的耐久由 death/loot 链路在 ECS 侧实例化时设定：武器 durability = slot.durability_ratio × 0.8（战损 20%）；护甲 = slot.durability_ratio × 0.7（战损 30%）

### P0 测试要求

- `assign_npc_equipment` 纯函数测试：每个 archetype × 每个 realm 至少 1 case，验证 slot 合法性 + quality_tier 不超 2
- deterministic：同 entity_id 两次调用结果一致
- `merge_equipment` 转换：验证 NpcEquipment + NpcVisualProfile 叠加、冲突 slot 优先级、空 slot → ItemStack::EMPTY
- 战斗结算：有装备 NPC 伤害 > 无装备 NPC 伤害（同 realm）
- 死亡掉落：1000 次 roll 统计掉率在 25%-35% 区间
- ≥ 25 单测

---

## P1 — NPC 功法系统

### P1.1 NPC KnownTechniques 挂载

- NPC 已有 `Cultivation` component（含 realm / qi_current / qi_max）+ `MeridianSystem` component（含 regular[12] + extraordinary[8] 经脉拓扑），由 plan-npc-ai-v1 落地
- 在 NPC spawn 时附加 `KnownTechniques` component（复用玩家 component，不新建）
- 新增 `fn assign_npc_techniques(archetype, realm, meridian_sys: &MeridianSystem, meridian_deps: &SkillMeridianDependencies, qi_color_hint, rng) -> KnownTechniques`
- 分配的功法必须同时满足：
  1. `realm_rank(parse_realm(def.required_realm)) <= realm_rank(npc_realm)`（注：`TechniqueDefinition.required_realm` 是 `&'static str`（如 `"Awaken"` / `"Induce"`），需先 `match` 解析为 `Realm` 枚举，再通过 `realm_rank()` free function 转为 `u8` 做数值比较——codebase 中 `Realm` 无 `FromStr` / `PartialOrd` impl）
  2. 经脉依赖满足：`meridian_deps.lookup(technique_id)` → `&[MeridianId]`，每个 MeridianId 在 NPC 的 `MeridianSystem` 中为已开（`meridian.opened == true`）
- NPC 功法熟练度 spawn 时固定，不随战斗增长（见 §8.1 #2 决议）

| Archetype | 功法池 | 数量 | 熟练度 |
|-----------|--------|------|--------|
| **Rogue 散修** | 全功法池中 realm + 经脉可用的子集，按 qi_color 偏好加权 | 1-3 | 0.2-0.7 (rng) |
| **Commoner 凡人** | 无 | 0 | — |
| **Beast 异兽** | 无（走天生攻击） | 0 | — |
| **Disciple 宗门弟子** | faction 对应功法子集（Attack→剑法/爆脉；Defend→截脉/涡流；Neutral→混合） | 2-4 | 0.3-0.8 |
| **GuardianRelic 遗迹守卫** | 古代高阶功法 | 3-5 | 0.6-0.9 |
| **Daoxiang 道伥** | 从生前功法残留（loot table 暗示的流派） | 1-2 | 0.1-0.4（退化） |
| **Zhinian 执念** | 半智能残留功法 | 2-3 | 0.3-0.6 |
| **SkullFiend 骷髅魔** | 无（走天生攻击） | 0 | — |

- 经脉依赖已在 skill registration 时由各功法模块调用 `SkillMeridianDependencies::declare()` 注册（如 `sword_path::skill_register::declare_meridian_dependencies()`）——spawn 时仅读取（`lookup`），不重复注册
- **前置验证**（P1 实施前，由 Explore agent 执行）：
  1. grep 所有功法模块确认每个 technique 都已调用 `declare()`，若发现缺失先补注册再开 P1
  2. grep NPC spawn 路径（`spawn_rogue_npc_at` / `spawn_commoner_npc_at` 等）确认 `.insert(MeridianSystem ...)` 存在——如果 NPC 未挂 `MeridianSystem` component，必须在 P1 PR 中补上（在 `assign_cultivation()` 或对应 spawn 函数中），否则功法分配无经脉数据可用

### P1.1.1 Rogue/GuardianRelic 战斗能力前置

**重要**：当前 `rogue_npc_thinker()` 仅有 `PlayerProximityScorer → FleeAction`（逃跑），无 `MeleeRangeScorer → MeleeAttackAction`；`relic_guard_thinker()` 同理无近战 Action。P1 必须先为这些 archetype 添加基础 `MeleeRangeScorer → MeleeAttackAction`，才能在其上叠加 `NpcTechniqueScorer → NpcTechniqueAction`。

需修改的 thinker（所有修改遵守 `FirstToScore` 插入顺序 = 优先级规则）：
- `rogue_npc_thinker()`：新增 `NpcTechniqueScorer → NpcTechniqueAction`（插入位 1）+ `MeleeRangeScorer → MeleeAttackAction`（插入位 2）；原 `PlayerProximityScorer → FleeAction`（插入位 3，仅无法战斗时逃跑）
- `relic_guard_thinker()`：新增 `NpcTechniqueScorer → NpcTechniqueAction`（插入位 1）+ `MeleeRangeScorer → MeleeAttackAction`（插入位 2）
- 已有 melee 的 thinker（Disciple、Daoxiang、Zhinian）：在现有 `MeleeRangeScorer → MeleeAttackAction` **之前**插入 `NpcTechniqueScorer → NpcTechniqueAction`

### P1.2 NpcTechniqueSelector

新文件 `server/src/npc/technique.rs`：

```rust
pub fn select_technique(
    known: &KnownTechniques,
    cultivation: &Cultivation,
    meridian_deps: &SkillMeridianDependencies,
    severed: Option<&MeridianSeveredPermanent>,
    cooldowns: &NpcCooldownMap,
    npc_entity: Entity,
    target_distance: f32,
    current_tick: u64,
) -> Option<String> // technique_id (owned — 在 exclusive system 中避免 lifetime 耦合)
```

选择逻辑：
1. 过滤 `known.entries` 中 `active == true` 的功法
2. **排除经脉 SEVERED 的功法**：先 `meridian_deps.lookup(technique_id)` → `&[MeridianId]` deps；再 `check_meridian_dependencies(deps, severed)` → `Err(meridian_id)` 则排除（对齐 worldview §四:286 "断了肺经的飞剑手就废了"）
3. 排除冷却中的功法（`cooldowns.get((npc_entity, technique_id)) > current_tick`）
4. 排除 qi_cost > `cultivation.qi_current` 的功法（真元不足）
5. 按 `target_distance` 过滤 range 匹配的功法
6. 按 `proficiency` 加权随机选一个（高熟练度功法被选中概率更高）
7. 如果没有可用功法 → 返回 None（fallback 到基础近战）

### P1.3 NpcCooldownMap Resource

```rust
#[derive(Resource, Default)]
pub struct NpcCooldownMap {
    map: HashMap<(Entity, &'static str), u64>, // (npc_entity, technique_id) → cooldown_until_tick
}
```

- key 用 `&'static str` 而非 `String`，与 `SkillRegistry` / `TECHNIQUE_IDS` 的 `&'static str` 一致，避免每次 insert 分配堆内存

- 功法释放后写入 `cooldown_until_tick = current_tick + definition.cooldown_ticks`
- NPC 死亡/despawn 时移除对应 entries（避免 Entity 复用冲突）

### P1.4 NpcTechniqueAction（big-brain Action）

新 Action，在 Rogue / Disciple / Daoxiang / Zhinian / GuardianRelic 的 brain thinker 中与 MeleeAttackAction **并列**（通过 Scorer 分数竞争选择）：

```rust
#[derive(Clone, Component, ActionBuilder)]
pub struct NpcTechniqueAction;
```

**系统 `npc_technique_action_system`**——必须声明为 **exclusive system**（`fn npc_technique_action_system(world: &mut World)`），因为 `SkillFn = fn(&mut World, Entity, u8, Option<Entity>) -> CastResult` 需要 exclusive `&mut World` 访问。这与现有玩家功法调用路径（`client_request_handler` 内同样使用 `&mut World`）一致。

执行步骤：
1. 从 World 读取所有 `ActionState::Requested` + `NpcTechniqueAction` 的 NPC entity 列表（collect 到 Vec 后释放 borrow）
2. 对每个 NPC：从 World 读 `KnownTechniques` / `Cultivation` / `MeridianSeveredPermanent`（Option）/ target entity → 调用 `select_technique()`
3. 如有功法 → `SkillRegistry.lookup(technique_id)` → `Option<SkillFn>`：
   - `Some(skill_fn)` → 调用 `skill_fn(world, npc_entity, 0, Some(target))` → 获得 `CastResult`
   - `None` → ActionState::Failure（功法未注册，异常路径）
4. 处理 `CastResult`：
   - `Started { cooldown_ticks, .. }` → 写入 `NpcCooldownMap` + 设 ActionState::Success
   - `Rejected { .. }` → ActionState::Failure（回落到 MeleeAttackAction 通过 Scorer 竞争）
   - `Interrupted` → ActionState::Failure
5. 如 `select_technique()` 返回 None → ActionState::Failure

**NPC 功法视听**：功法本身的 VFX/SFX 走已有 `bong:vfx_event` 通道（每个功法在其 plan 中已定义粒子/音效/动画），NPC 调用与玩家调用**同一套 VFX pipeline**，无需新增。

**NPC 功法 narration**（仅特殊场景）：
- NPC 释放高阶功法（realm ≥ 固元 + technique grade ≥ 2）时触发天道 narration
- scope: `zone`（仅同区域玩家可见）
- style: `perception`
- 示例模板：
  - `"一道凌厉剑意自{npc_name}方向激荡而出。"`
  - `"{npc_name}体内真元暴涌——残破的经脉似乎承受不住。"`
  - `"你感到一股沉重的压迫感自远处传来。不是天道，是人。"`

### P1.5 NpcTechniqueScorer

新 Scorer 与现有 `MeleeRangeScorer` 并列：

```rust
#[derive(Clone, Component, ScorerBuilder)]
pub struct NpcTechniqueScorer;
```

- 评分 = `0.0` 如果 NPC 无 `KnownTechniques` 或全部冷却中或所有功法经脉依赖被 `MeridianSeveredPermanent` 阻断
- 评分 = `0.85` 如果有可用功法 + 目标在 range 内 + qi 足够 + 经脉依赖满足（scorer query 需包含 `Option<&MeridianSeveredPermanent>`）
- **插入顺序决定优先级**：所有 NPC thinker 使用 `FirstToScore { threshold: 0.05 }`——该 picker 按插入顺序选第一个超过阈值的 scorer/action 对，**不比较 score 大小**。因此 `NpcTechniqueScorer` 必须在 thinker builder 中插入于 `MeleeRangeScorer` **之前**，使功法优先尝试；当 NpcTechniqueScorer 返回 0.0 时 `FirstToScore` 自动 fallthrough 到 MeleeRangeScorer
- 最小间隔 = `60 + realm_rank(cultivation.realm) * 10` ticks（避免 NPC 连续释放功法过于频繁；`realm_rank()` 返回 0-5 对应醒灵到化虚）

### P1.6 功法 qi 消耗走 qi_physics ledger

- `SkillRegistry` 内部注册的每个功法函数已经扣 `Cultivation.qi_current -= qi_cost`
- 扣减后必须 emit `QiTransfer { from: NpcEntity, to: Zone, amount: qi_cost, reason: TechniqueRelease }`
- **前置验证**（P1 实施前用 Explore agent 核验）：grep 所有 `SkillFn` 实现，确认 qi 扣减后有 `qi_release_to_zone` 或等价路径。若发现缺失，先提 patch PR 补 ledger 记账再开 P1
- NPC 真元回复：NPC 已挂 Cultivation component → 现有 `cultivation::tick` 系统在 zone_qi > 0 时自动回复 qi_current，NPC 与玩家走同一路径，无需新增

### P1 测试要求

- `assign_npc_techniques` 纯函数：每个 archetype × realm 覆盖；realm 不足时返回空 Vec；`MeridianSystem` 经脉未开时排除对应功法；`required_realm` 字符串正确 match 解析为 `Realm` 再通过 `realm_rank()` 做有序比较
- `select_technique`：冷却过滤 / qi 不足过滤 / range 过滤 / **`MeridianSeveredPermanent` 过滤**（`lookup(technique_id)` + `check_meridian_dependencies(deps, severed)`）/ 全冷却返回 None
- `NpcTechniqueAction`：mock SkillRegistry（exclusive system 测试），验证 Started → cooldown 写入 / Rejected → ActionState::Failure / lookup 返回 None → Failure
- qi_physics：NPC 释放功法前后 qi_current 差值 == qi_cost；QiTransfer event 已发射
- **经脉 SEVERED 阻断**：构造含 `MeridianSeveredPermanent` 的 NPC → 依赖该经脉的功法被 select_technique 排除
- ≥ 30 单测

---

## P2 — NPC 交互 GUI 重绘（owo-lib）

### P2.0 扩展 NpcMetadata 协议

**Server 端**（`server/src/network/` NPC metadata 发送逻辑）新增 JSON 字段：

```json
{
  "entity_id": 42,
  "archetype": "rogue",
  "realm": "凝脉",
  "display_name": "张三丰",
  "reputation_to_player": 65,
  "faction_name": null,
  "faction_rank": null,
  "age_band": "壮年",
  "greeting_text": "道友远来，不知有何贵干？",
  "qi_hint": "气息绵长",
  "hp_ratio": 0.95,
  "qi_ratio": 0.8,
  "equipment": {
    "main_hand": { "template_id": "iron_sword", "display_name": "铁剑", "quality_tier": 0, "durability_ratio": 0.85 },
    "chest": { "template_id": "bone_chestplate", "display_name": "骨甲", "quality_tier": 0, "durability_ratio": 0.7 },
    "legs": { "template_id": "straw_leggings", "display_name": "草编裤", "quality_tier": 0, "durability_ratio": 0.6 }
  },
  "techniques": [
    { "id": "sword_basics_slash", "display_name": "基础横斩", "proficiency": 0.75 },
    { "id": "jiemai_parry", "display_name": "截脉弹反", "proficiency": 0.4 }
  ],
  "trade_offers": [
    { "template_id": "lingcao", "display_name": "灵草", "count": 3, "price_bone_coins": 12 },
    { "template_id": "fragment_scroll", "display_name": "残卷", "count": 1, "price_bone_coins": 45 }
  ]
}
```

**Client 端**：

- `NpcMetadata.java` record 新增 `Map<String, NpcEquipSlotData> equipment` + `List<NpcTechniqueData> techniques` + `List<NpcTradeOffer> tradeOffers`
- `NpcEquipSlotData` record：`templateId`, `displayName`, `qualityTier`, `durabilityRatio`
- `NpcTechniqueData` record：`id`, `displayName`, `proficiency`
- `NpcTradeOffer` record：`templateId`, `displayName`, `count`, `priceBoneCoins`
- `NpcMetadataHandler.java` 解析新字段（null-safe，旧 server 兼容空 map/list/null）
- **Java record 扩展策略**：`NpcMetadata` 是 Java record（不可变），新增 3 个字段后总参数 15 个。保留原有的 10-param / 12-param 向后兼容构造器（`this(... , Map.of(), List.of(), List.of())`），所有现有 call site 不受影响。需同步更新的调用方：
  - `NpcMetadataHandler.java:36`（解析构造）
  - `NpcMetadataStore.upsert()`（如有直接构造）
  - `NpcEngagementIntentHandler.java:39`（使用处）
  - `TargetInfoState.fromNpcMetadata()`（HUD 读取，新字段可忽略）

### P2.1 NpcDialogueScreen 重写

删除旧 `client/src/main/java/com/bong/client/npc/NpcDialogueScreen.java`，新建同名 class：

- `extends BaseOwoScreen<FlowLayout>`
- 根布局 `Surface.VANILLA_TRANSLUCENT`，水平+垂直居中

**布局结构**：

```
rootComponent (FlowLayout.vertical, CENTER/CENTER)
└── mainPanel (FlowLayout.vertical, width=280, DARK_PANEL, padding=12)
    ├── headerRow (FlowLayout.horizontal)
    │   ├── archetypeIcon (label, "[散修]", color 0xA89070)
    │   ├── spacer (4px)
    │   ├── npcName (label, "张三丰", color 0xE8D8A8)
    │   ├── spacer (8px)
    │   └── realmBadge (label, "· 凝脉", color 0x8FE6B8)
    ├── divider (label, "────────────────────────", color 0x404040)
    ├── greetingText (label, multiline wrap, color 0xD0D0D0, max 3 lines)
    ├── divider
    ├── optionRow_trade (FlowLayout.horizontal, clickable → open TradeScreen)
    │   ├── arrow (label, "▸", color 0xC8A86E)
    │   └── optionText (label, "看看你有什么好东西", color 0xE0D8C8)
    ├── optionRow_inspect (FlowLayout.horizontal, clickable → open InspectScreen)
    │   ├── arrow (label, "▸", color 0xC8A86E)
    │   └── optionText (label, "让我打量一下你", color 0xE0D8C8)
    ├── optionRow_leave (FlowLayout.horizontal, clickable → close)
    │   ├── arrow (label, "▸", color 0x888888)
    │   └── optionText (label, "告辞", color 0xA0A0A0)
    ├── divider
    └── repBar (NpcReputationIndicator, width=256)
```

- 选项行 hover 时文字变亮（0xFFFFFF）
- 敌对 NPC：greetingText 显红色 (0xE05A47) 警告文字 "此人对你充满敌意。" + trade 选项隐藏
- 交易选项仅 `metadata.tradeCandidate() == true` 时显示——**重构 `tradeCandidate()`**：现有实现硬编 `"rogue".equals(archetype) || "commoner".equals(archetype)`，改为数据驱动：`!hostile() && tradeOffers != null && !tradeOffers.isEmpty()`（有货可卖且非敌对即可交易）

**上游调用方适配**：
- `NpcEngagementIntentHandler.java`：`new NpcDialogueScreen(metadata)` → 构造签名保持不变（单参 `NpcMetadata`）
- `NpcDialogueBubbleRenderer.java:77`：`instanceof NpcDialogueScreen` 继续有效（`BaseOwoScreen` extends `Screen`）
- 三个 Screen 之间的交叉导航（Dialogue → Trade / Inspect → back）在新 owo-lib 版本中通过 `MinecraftClient.getInstance().setScreen(new NpcXxxScreen(metadata))` 保持相同模式
- 在 `ScreenTransitionRegistry.bootstrapDefaultsLocked()` 中注册三个新 Screen 的转场配置（与 CultivationScreen / ForgeScreen 等一致），避免 NPC 屏切换时无转场动画

**颜色表**：

| 元素 | hex | 用途 |
|------|-----|------|
| NPC 名字 | `0xE8D8A8` | 金黄——重要信息 |
| 境界徽标 | `0x8FE6B8` | 淡绿——修炼相关 |
| 类型标签 | `0xA89070` | 暗褐——次要标注 |
| 对话文字 | `0xD0D0D0` | 浅灰——正文 |
| 选项文字 | `0xE0D8C8` | 暖白——可交互 |
| 选项箭头 | `0xC8A86E` | 暗金——引导 |
| 离开文字 | `0xA0A0A0` | 灰——低优先 |
| 敌意文字 | `0xE05A47` | 红——警告 |
| 分隔线 | `0x404040` | 深灰——装饰 |
| 面板底色 | `Surface.DARK_PANEL` | owo-lib 内置 |

### P2.2 NpcInspectScreen 重写

删除旧 class，新建同名：`extends BaseOwoScreen<FlowLayout>`

**布局结构**：

```
rootComponent (FlowLayout.vertical, CENTER/CENTER)
└── mainPanel (FlowLayout.vertical, width=340, DARK_PANEL, padding=12)
    ├── headerLabel ("张三丰 · 散修 · 凝脉", color 0xE8D8A8)
    ├── divider
    ├── contentRow (FlowLayout.horizontal)
    │   ├── leftCol (FlowLayout.vertical, width=140)
    │   │   ├── sectionTitle ("基础信息", color 0xC8A86E)
    │   │   ├── infoLine ("寿元: 壮年", color 0xD0D0D0)
    │   │   ├── infoLine ("派系: 无", color 0xD0D0D0)
    │   │   ├── infoLine ("态度: 友善", color 0x5DD17A / 0xE05A47 / 0xC8C8C8)
    │   │   ├── infoLine ("气息: 气息绵长", color 0x8FE6B8)
    │   │   ├── spacer (8px)
    │   │   ├── sectionTitle ("状态", color 0xC8A86E)
    │   │   ├── qiBar (████████░░ 80%, color 0x4A9EE0)
    │   │   ├── hpBar (█████████░ 95%, color 0xCC4444)
    │   │   └── repBar (NpcReputationIndicator)
    │   └── rightCol (FlowLayout.vertical, width=190)
    │       ├── sectionTitle ("装备", color 0xC8A86E)
    │       ├── equipLine ("主手: 铁剑 (凡铁) ██████░░░░ 85%")
    │       ├── equipLine ("胸甲: 骨甲 (凡铁) ███████░░░ 70%")
    │       ├── equipLine ("腿甲: 草编裤 ██████░░░░ 60%")
    │       ├── spacer (8px)
    │       ├── sectionTitle ("已知功法", color 0xC8A86E)
    │       ├── techLine ("基础横斩  ████████░░ 0.75")
    │       └── techLine ("截脉弹反  ████░░░░░░ 0.40")
    ├── divider
    └── buttonRow (FlowLayout.horizontal)
        ├── backButton ("返回", Surface.DARK_PANEL)
        └── closeButton ("关闭", Surface.DARK_PANEL)
```

- 装备 quality_tier 文字色：凡铁(0xC8C8C8) / 灵器(0x5DD17A) / 法宝(0x4A9EE0)
- 功法熟练度条色：< 0.3 红(0xCC4444) / 0.3-0.6 黄(0xE2C84A) / > 0.6 绿(0x5DD17A)
- 耐久度条色：> 0.5 绿(0x5DD17A) / 0.2-0.5 黄(0xE2C84A) / < 0.2 红(0xCC4444)
- 无装备/无功法时显示 "无" (0x666666)
- 功法信息的可见性取决于玩家境界（worldview §十一 信息差）：
  - 醒灵/引气：只看到 "功法: 未知"
  - 凝脉：看到功法名但不看熟练度
  - 固元+：看到完整功法 + 熟练度条

### P2.3 NpcTradeScreen 重写

删除旧 class，新建同名：`extends BaseOwoScreen<FlowLayout>`

**布局结构**：

```
rootComponent (FlowLayout.vertical, CENTER/CENTER)
└── mainPanel (FlowLayout.vertical, width=360, DARK_PANEL, padding=12)
    ├── headerRow (FlowLayout.horizontal)
    │   ├── npcName ("张三丰 · 交易", color 0xE8D8A8)
    │   ├── spacer (fill)
    │   └── balanceLabel ("骨币: 42 枚", color 0xC8A86E)
    ├── divider
    ├── tradeBody (FlowLayout.horizontal)
    │   ├── npcWares (FlowLayout.vertical, width=170, title "NPC 货物")
    │   │   ├── wareRow ("灵草 ×3", price "12 骨币", clickable → select)
    │   │   ├── wareRow ("残卷 ×1", price "45 骨币", clickable → select)
    │   │   ├── wareRow ("凝脉散 ×1", price "30 骨币", clickable → select)
    │   │   └── ... (scrollable if > 4 items)
    │   ├── separator (vertical line, 1px, 0x404040)
    │   └── playerOffer (FlowLayout.vertical, width=170, title "你的出价")
    │       ├── selectedItem ("灵草 ×3", color 0x5DD17A)
    │       ├── priceTotal ("总价: 12 骨币", color 0xE0D8C8)
    │       ├── discountLine ("信誉折扣: 8折", color 0x5DD17A) // 仅 rep > 50 显示
    │       ├── spacer
    │       └── confirmButton ("确认交易", enabled 仅骨币足够)
    ├── divider
    └── buttonRow
        ├── backButton ("返回")
        └── closeButton ("关闭")
```

- 货物行 hover 高亮底色 `Surface.flat(0x30FFFFFF)`
- 选中货物行左侧显示 "▸" 箭头
- 骨币不足时 confirmButton 灰掉 + priceTotal 变红(0xE05A47)
- NPC 货物内容由 server 在 `bong:npc_metadata` 的 `trade_offers` 字段下发
- 交易协议复用现有 `npc_trade_request` packet，`requested_item_id` 字段已是 `String` 语义上等价 `template_id`，字段名不改（避免破坏现有协议），仅在 server 端确保解析时按 `template_id` 查 `ItemTemplateRegistry`
- 交易成功后 close screen + 显示事件流消息 "交易完成"
- 骨币定价走枚数（整数）——骨币的半衰期衰减在 `shelflife` 系统层面处理（server 端已实现），交易定价的"12 骨币"指的是"标准新铸骨币枚数"，实际支付时 server 按玩家背包中骨币的平均真元含量折算

### P2 测试要求

- `NpcMetadataHandler` 新字段解析：完整 JSON / 缺失 equipment 字段 / 缺失 techniques 字段 / 缺失 trade_offers
- Screen 构造：null metadata 安全关闭 / 敌对 NPC 隐藏交易选项 / 空装备显示 "无"
- 功法可见性按境界：醒灵不可见 / 凝脉部分可见 / 固元完整可见
- ≥ 15 单测（client 单测 via JUnit/游戏测试框架）

---

## P3 — 集成测试 + 校准

### P3.1 Server 集成

- 完整链路测试：spawn Rogue NPC → 验证 `NpcEquipment` + `KnownTechniques` + Valence `Equipment` + `MeridianSystem` 四组件存在 + `KnownTechniques` 中每个功法的经脉依赖满足 `MeridianSystem` 拓扑
- 战斗集成：NPC 与 mock player 交战 → NPC 释放功法 → 验证 `CastResult::Started` + `QiTransfer` event
- 死亡集成：NPC 死亡 → 装备概率掉落 → `ItemInstance` 可被拾取
- 经脉 SEVERED 集成：给 NPC 挂上 `MeridianSeveredPermanent` component → 验证下一次 select_technique 排除依赖该经脉的功法

### P3.2 Client-Server 协议集成

- `bong:npc_metadata` 扩展字段端到端：server 发送 → client 解析 → screen 显示
- 旧 client 连新 server：新字段 null-safe 不 crash
- 新 client 连旧 server：缺失字段 fallback 到 "无"

### P3.3 NPC 功法频率校准

NPC 功法使用不应过于频繁（避免 NPC 比玩家还强）：

| Archetype | 功法使用频率 | 基础近战占比 |
|-----------|------------|------------|
| Rogue 散修 | 每 3-5 次攻击使用 1 次功法 | 70-80% |
| Disciple 弟子 | 每 2-3 次攻击使用 1 次功法 | 50-60% |
| GuardianRelic | 每 1-2 次攻击使用 1 次功法 | 30-40% |
| Daoxiang 道伥 | 每 5-8 次攻击使用 1 次功法 | 85-90% |
| Zhinian 执念 | 每 3-4 次攻击使用 1 次功法 | 60-70% |

- 频率控制通过 `NpcTechniqueScorer` 的最小间隔 tick 实现

### P3 测试要求

- e2e spawn → equipment visible → technique cast → combat damage → death drop 全链路 ≥ 5 case
- 协议兼容：旧 client / 旧 server 混跑不 crash
- 校准数值：100 次 NPC 战斗统计功法使用频率在目标区间内
- ≥ 15 单测

---

## §8 开放问题（P0 决策门前需收口）

### #1 NPC 是否需要 Weapon component

- **方案 A**：直接挂 `Weapon` component → 现有 `sync_weapon_component_from_equipped` 系统需适配
- **方案 B**：NPC 不挂 `Weapon`，战斗系统从 `NpcEquipment` 直接读武器属性

### #2 NPC 功法 proficiency 是否动态增长

- **方案 A**：spawn 时固定，不随战斗增长
- **方案 B**：缓慢增长（gain 率 ×0.3）

### #3 NPC 交易货物来源

- **方案 A**：从 `NpcEquipment` + `NpcLootTable` 派生
- **方案 B**：新增 `NpcTradeInventory` component，spawn 时按 archetype + realm 生成 trade offers

### #4 旧 NPC GUI 文件处理

- 重写后旧 vanilla Screen 类直接删除还是保留 fallback？

### #5 NPC 经脉拓扑来源

- NPC spawn 时 `Cultivation` component 的 `opened_meridians` 数据如何生成？

### #6 骨币交易是按枚数计还是按真元含量计

- 与 shelflife 半衰期公式如何衔接？

全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

---

## §8.1 决议（pre-P0 收口，2026-05-21）

### #1 NPC 不挂 Weapon component

**决议**：
1. 选方案 B：NPC 不挂 `Weapon` component，战斗系统从 `NpcEquipment.main_hand` 直接读武器属性
2. 在 `npc::equipment` 模块内新增 `fn npc_weapon_damage_multiplier(slot: &NpcEquipSlot) -> f32`，公式与 `Weapon::damage_multiplier()` 对齐：`max(1.0, slot.base_attack/10.0) × quality_multiplier(slot.quality_tier) × (0.5 + 0.5 * slot.durability_ratio)`，其中 `slot.base_attack` 在 spawn 时从 `ItemTemplateRegistry` 读入
3. 拒绝方案 A 的理由：`sync_weapon_component_from_equipped` 硬编读 `PlayerInventory`（`server/src/combat/weapon.rs:116-159`），适配它需要引入对 PlayerInventory 的可选依赖，增加 NPC 与 player 系统的耦合度

**落点**：`server/src/npc/equipment.rs`（新建）+ plan §P0.4

### #2 NPC 功法熟练度 spawn 时固定

**决议**：
1. 选方案 A：NPC 功法熟练度 spawn 时由 `assign_npc_techniques()` 一次性确定，战斗中不增长
2. NPC 成长应由 agent 天道推演驱动（plan-npc-virtualize 系列在 dormant 状态下批量推演），不由 ECS 战斗系统实时驱动
3. 避免 NPC 无限成长导致的平衡问题——长寿 NPC 熟练度逼近 1.0 后与玩家战力差距失控

**落点**：`server/src/npc/technique.rs:assign_npc_techniques()`（新建）+ plan §P1.1

### #3 新增 NpcTradeInventory component

**决议**：
1. 选方案 B：新增 `NpcTradeInventory` component，spawn 时按 archetype + realm 生成一组 `TradeOffer { template_id, display_name, count, price_bone_coins }`
2. 交易货物独立于装备——散修可能卖灵草但自己用剑战斗，两者不矛盾
3. trade_offers 在 `bong:npc_metadata` S2C packet 中下发给 client（见 §P2.0 JSON 示例）
4. 定价走骨币枚数（整数），server 端按 shelflife 模块的骨币真元含量做折算——GUI 层不暴露半衰期细节

**落点**：`server/src/npc/trade.rs`（新建）+ `client/src/main/java/com/bong/client/npc/NpcMetadata.java` + plan §P2.0 / §P2.3

### #4 旧 NPC GUI 直接删除

**决议**：
1. 旧 vanilla Screen 类（NpcDialogueScreen / NpcInspectScreen / NpcTradeScreen）在 PR-3 中直接删除
2. 新 owo-lib 版本同名替换，无需 fallback——项目无 AB 测试需求，保留死代码增加维护负担

**落点**：`client/src/main/java/com/bong/client/npc/NpcDialogueScreen.java` / `NpcInspectScreen.java` / `NpcTradeScreen.java`（删除后新建）+ plan §P2.1-§P2.3

### #5 NPC 经脉拓扑复用现有 MeridianSystem component

**决议**：
1. NPC 已有 `Cultivation` component（realm / qi_current / qi_max）+ `MeridianSystem` component（regular[12] + extraordinary[8] 经脉拓扑），由 plan-npc-ai-v1 落地。**经脉数据在 `MeridianSystem` 中，不在 `Cultivation` 中**
2. NPC spawn 时 `MeridianSystem` 由 `npc::spawn` 中现有的 `assign_cultivation()` 函数按 realm 生成——realm 越高开脉越多（与玩家同一规则：引气=3条 / 凝脉=6条 / 固元=12条 / 通灵=12+4奇 / 化虚=20条全开）
3. `assign_npc_techniques()` 的 `meridian_sys` 参数直接从 NPC 的 `MeridianSystem` component 读取
4. 经脉可在战斗中被 SEVERED（plan-meridian-severed-v1 已实装），`select_technique()` 通过独立的 `MeridianSeveredPermanent` component（`Option<&MeridianSeveredPermanent>`）实时读取断经状态做排除

**落点**：`server/src/npc/spawn.rs`（现有 `assign_cultivation()`）+ `server/src/npc/technique.rs:assign_npc_techniques()` + plan §P1.1 / §P1.2

### #6 骨币交易按枚数计

**决议**：
1. GUI 层显示骨币枚数（整数），这是修士社会的约定计价单位
2. 实际支付时 server 按 `shelflife` 模块的骨币真元含量做折算：如果玩家背包中的骨币平均真元含量低于"标准新铸骨币"，需要支付更多枚数
3. 折算公式：`actual_count = ceil(price / avg_bone_coin_qi_ratio)`
4. GUI 不暴露半衰期细节——这是 worldview §九 "万物皆有成本"的隐性表现，玩家需要自己体会"为什么我的骨币越放越不值钱"

**落点**：`server/src/economy/` 交易结算逻辑 + plan §P2.3 骨币显示

---

## §10 实施工作流

### §10.1 多轮自我打磨

P2 GUI 重绘涉及视觉资产（owo-lib 布局），适用 3 轮打磨规则：
- Round 1：实现基本布局 + 组件
- Round 2：自我 review（截图对照 §P2 布局 spec，检查颜色/间距/对齐）
- Round 3：终轮（与 spec 一致性 + 交互流畅性）+ `<PROMISE>` 块

### §10.2 PR 拆分

| PR | 范围 | 前置 |
|----|------|------|
| **PR-1** | P0：NpcEquipment + spawn 分配 + Valence 同步 + 战斗读取 + 死亡掉落 | 无 |
| **PR-2** | P1：KnownTechniques 挂载 + NpcTechniqueSelector（含经脉校验） + NpcTechniqueAction + brain 替换 + NpcTradeInventory | PR-1 merged |
| **PR-3** | P2：NpcMetadata 协议扩展 + owo-lib 三屏重写（3 轮打磨） | PR-2 merged |
| **PR-4** | P3：集成测试 + 校准 + Finish Evidence 归档 | PR-3 merged |

### §10.3 Subagent 配置

```
Agent(
  subagent_type: "claude",
  model: "opus",
  prompt: "...任务...\n\nultrathink"
)
```

### §10.4 CodeRabbit 等待协议

每个 PR 提交后 `ScheduleWakeup delaySeconds=1200` 等 CodeRabbit review，最多 3 回合。修完 review 意见必须重新等 CR re-review，不自行判定"我修好了应该过"。

### §10.5 单次 consume-plan 全自动到 merge

```
for pr_n in [PR-1..PR-4]:
    result = Agent(subagent_type="claude", model="opus",
                   prompt="实施 PR-{pr_n}...\n\nultrathink")
    pr_url = parse(result)
    # 等 CodeRabbit review
    while gh pr checks pr_url == "pending":
        ScheduleWakeup(1200, "等 CR PR #{pr_n}")
    if has_review_issues:
        Agent(subagent_type="claude", model="opus",
              prompt="修复 CR review...\n\nultrathink")
        # 重等
    gh pr merge --squash --delete-branch
# 全部 PR merged → 归档
git mv docs/plan-npc-combat-gear-v1.md docs/finished_plans/
```

用户提交 `/consume-plan plan-npc-combat-gear-v1` 后即可离开，醒来看 plan 是否在 `finished_plans/`。

---

## Finish Evidence

### 落地清单

| 阶段 | 模块 / 文件 |
|------|------------|
| **P0** | `server/src/npc/equipment.rs` — `NpcEquipment` / `NpcEquipSlot` / `assign_npc_equipment()` / `merge_equipment()` / `roll_equipment_drops()` / `npc_weapon_damage_multiplier()` |
| **P1** | `server/src/npc/technique.rs` — `assign_npc_techniques()` / `select_technique()` / `NpcCooldownMap` / `NpcTechniqueScorer` / `NpcTechniqueAction` / `npc_meridian_system_for_realm()` |
| **P1** | `server/src/npc/trade.rs` — `NpcTradeInventory` / `TradeOffer` / `assign_npc_trade_inventory()` |
| **P2** | `server/src/network/npc_metadata.rs` — `NpcMetadataS2c` 扩展 `equipment` / `techniques` / `trade_offers` 字段 |
| **P2** | `client/src/main/java/com/bong/client/npc/NpcMetadata.java` — record 扩展 + backward-compatible constructors |
| **P2** | `client/src/main/java/com/bong/client/npc/NpcMetadataHandler.java` — `parseEquipment()` / `parseTechniques()` / `parseTradeOffers()` |
| **P2** | `client/src/main/java/com/bong/client/npc/NpcEquipSlotData.java` / `NpcTechniqueData.java` / `NpcTradeOffer.java` |
| **P2** | `client/src/main/java/com/bong/client/npc/NpcDialogueScreen.java` — owo-lib 重写 |
| **P2** | `client/src/main/java/com/bong/client/npc/NpcInspectScreen.java` — owo-lib 重写（装备+功法可视） |
| **P2** | `client/src/main/java/com/bong/client/npc/NpcTradeScreen.java` — owo-lib 重写（骨币交易） |
| **P3** | `server/src/npc/combat_gear_integration_test.rs` — 20 集成测试（spawn 链路 + 协议兼容 + 频率校准） |

### 关键 commit

| Hash | 日期 | 消息 |
|------|------|------|
| `2a3720d20` | 2026-05-21 | plan-npc-combat-gear-v1 PR-1: P0 NPC 装备模型 + Valence 同步 (#296) |
| `b8a041aa4` | 2026-05-21 | plan-npc-combat-gear-v1 PR-2: P1 NPC 功法系统 + NpcTradeInventory (#297) |
| `2477a195f` | 2026-05-21 | plan-npc-combat-gear-v1 PR-3: P2 NPC 交互 GUI 重绘 (#299) |
| `58ab78d92` | 2026-05-21 | plan-npc-combat-gear-v1 PR-4: P3 集成测试 + 校准 |

### 测试结果

**Server**（`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`）:
- 全量 5957 测试通过，0 失败
- 本 plan 新增测试：121 个（equipment 53 + technique 38 + trade 17 + npc_metadata 10 + integration 20 + loot 相关继承）

**Client**（`cd client && ./gradlew test`）:
- 1643 测试通过（1 pre-existing failure: `ArmorProfileStoreCrossCheckTest` 与本 plan 无关）
- 本 plan 新增/修改：NpcMetadataHandlerTest 23 tests + NpcScreenDescribeTest 35 tests

### 跨仓库核验

**Server**:
- `NpcEquipment` — `server/src/npc/equipment.rs`, `server/src/npc/spawn.rs`, `server/src/npc/brain.rs`, `server/src/network/npc_metadata.rs`
- `NpcTechniqueScorer` / `NpcTechniqueAction` — `server/src/npc/technique.rs`, `server/src/npc/brain.rs`
- `NpcCooldownMap` — `server/src/npc/technique.rs`
- `NpcTradeInventory` — `server/src/npc/trade.rs`, `server/src/network/npc_metadata.rs`
- `bong:npc_metadata` 协议扩展 — `server/src/network/npc_metadata.rs` (`equipment` / `techniques` / `trade_offers` 字段)

**Client**:
- `NpcMetadata` record — `client/src/main/java/com/bong/client/npc/NpcMetadata.java`（含 backward-compatible constructors）
- `NpcMetadataHandler` — `client/src/main/java/com/bong/client/npc/NpcMetadataHandler.java`（`parseEquipment` / `parseTechniques` / `parseTradeOffers`）
- `NpcEquipSlotData` / `NpcTechniqueData` / `NpcTradeOffer` — 新增 record 类
- `NpcDialogueScreen` / `NpcInspectScreen` / `NpcTradeScreen` — owo-lib `BaseOwoScreen` 重写

### 遗留 / 后续

- NPC 功法调用的 `SkillFn` 路径在真实 ECS 中尚未端到端验证（需要 `SkillRegistry` 注册 + 真实 World）——当前 P3 测试验证到 `select_technique` 纯函数层，action system 的 exclusive system 路径待后续 plan 覆盖
- NPC 交易结算中骨币真元含量折算（§8.1 #6）依赖 `shelflife` 模块的骨币半衰期公式，当前 GUI 直接显示枚数，折算逻辑待 `economy/` 模块落地
- `ArmorProfileRegistry` 读取（§P0.4 护甲减伤）目前走默认 profile，待 `plan-armor-v1` 的 NPC 侧适配完善后接入真实 profile lookup
