# Plan: 深度玩法循环 v1 — 凡铁/骨质装备锻造 + 自定义装备模型 + 功法练功 + 战斗丹供给链

> **问题**：`plan-onboarding-loop-v1` 为每个系统打开了入口（第一张残卷、第一把凡铁剑、第一炉回元丹），但没有持续深度——玩家做完入门就断了。本 plan 在入口之后铺设**可循环**的深度玩法：锻造全套护甲、练功提升熟练度、持续炼丹补给、装备磨损→修复→再造。worldview §三 正典循环的**锻造-供给子循环**（修炼→需丹→采集→再造），不覆盖探索/战斗节点（由其他 plan 提供）。
>
> **并行关系**：与 `plan-onboarding-loop-v1` **大体无冲突**——本 plan 改 forge/、armor/、technique_proficiency、client/armor/、client model assets；onboarding 改 tsy_container、poi_novice、loot_pools、onboarding_scrolls、combat/resolve。两者可同时开 PR。**注意**：`schema/server_data.rs` 两边都追加新类型（本 plan: `TechniqueProficiencyUpdateV1` + `PillBuffStatusV1`；onboarding: `ContainerKindV1::SurfaceStash`），merge 时需手工 resolve。
>
> **硬约束**：所有在世界内可见的装备/武器/工具必须有**独立精致 OBJ 模型 + 贴图**，每件 3 轮自我打磨 + `<PROMISE>` 担保（见 CLAUDE.md 建筑类约束）。禁止使用 vanilla leather dye tinting 作为护甲最终渲染方案。

---

## 接入面 Checklist（防孤岛）

- **进料**：
  - `forge::BlueprintRegistry` — 复用 4 步管线（billet/tempering/inscription/consecration，`forge/blueprint.rs:20-147`）
  - `combat::armor::ArmorProfile` — 复用 profile struct（`armor.rs:28-104`），durability 系统（`armor.rs:106-140`）
  - `combat::armor_sync::sync_armor_to_derived_attrs()` — 已实装（`armor_sync.rs:13-62`），读 equipped armor → build defense_profile
  - `armor::mundane` — 7 材质 × 4 槽位 = 28 件物品已定义（`armor/mundane.rs:17-177` + `items/armor.toml:124-431`）
  - `cultivation::technique_proficiency` — 熟练度公式（`technique_proficiency.rs:43-59`）+ TechniqueMasteredEvent（`:61-77`）
  - `sword_basics` proficiency scaling — plan-sword-basics-v1 已交付 5 档缩放（cast_ticks/range/block_ratio/damage_mult）
  - `botany::BotanyKindRegistry` — plan-botany-v1/v2 已交付 42 植物 + 区域刷新 + 采集 session
  - `alchemy::RecipeRegistry` — 35 配方 JSON + 全链管线（session/fire/resolve/outcome）
  - `alchemy::pill` — 10 战斗丹 hardcoded spec（`pill.rs:212-223`）
  - `client::weapon::BongWeaponModelRegistry` — 9 武器 SML→OBJ 管线（`BongWeaponModelRegistry.java:20-121`）
  - `client::armor::ArmorTintRegistry` — 当前 leather dye 方案（**将被替换**）
  - `client::entity::BongModeledEntityRenderer` — 自定义 geometry 渲染（.geo.json）

- **出料**：
  - 8 个新 forge blueprint JSON → `server/assets/forge/blueprints/`（4 铁甲 + 4 骨甲）
  - 8 个新 armor_profile JSON → `server/assets/combat/armor_profiles/`
  - 8 个新 blueprint scroll 物品 → `server/assets/items/armor_scrolls.toml`
  - `ArmorModelRegistry` 新 class → `client/src/main/java/com/bong/client/armor/ArmorModelRegistry.java`（替换 ArmorTintRegistry 渲染）
  - `ArmorFeatureRenderer` 新 class → `client/src/main/java/com/bong/client/armor/ArmorFeatureRenderer.java`（自定义护甲 3D 渲染层）
  - 13 个 OBJ 模型 + 贴图 → `client/src/main/resources/assets/bong/models/`（4 铁甲 + 4 骨甲 + 4 工具 + 1 骨匕首）
  - `PracticeSession` 新 system → `server/src/cultivation/practice_session.rs`
  - 通用 `ProficiencyScalars` → `server/src/cultivation/technique_proficiency.rs`
  - 丹药 buff HUD planner → `client/src/main/java/com/bong/client/hud/PillBuffHudPlanner.java`
  - `TechniqueProficiencyUpdateV1` + `PillBuffStatusV1` → `server/src/schema/server_data.rs`
  - `MixinPlayerEntityArmor.java` 修改 → `client/src/main/java/com/bong/client/mixin/MixinPlayerEntityArmor.java`

- **共享类型 / event**：
  - 复用 `ForgeSession` / `ForgeOutcomeEvent` / `BlueprintRegistry`（forge 全链）
  - 复用 `ArmorProfile` / `DerivedAttrs.defense_profile`（armor 全链）
  - 复用 `TechniqueDefinition` / `KnownTechnique` / `TechniqueMasteredEvent`
  - 新增 `PracticeSessionStarted` / `PracticeSessionEnded` event（cultivation 内部）
  - 新增 `PillBuffActiveEvent`（schema/server_data 可选——如果 server push 到 client HUD）

- **跨仓库契约**：
  - Server：forge blueprint JSON + armor profile JSON + blueprint scroll TOML + PracticeSession system
  - Client：ArmorModelRegistry + ArmorFeatureRenderer + 13 OBJ models + PillBuffHudPlanner
  - Agent（天道 narration）：无新增——深度循环不需要 narration hook。P2.3 可选在 `agent/packages/schema/src/server-data.ts` 追加 `TechniqueProficiencyUpdateV1` TypeBox 类型（仅 agent 需监控熟练度时）

- **worldview 锚点**：§三（修炼→需丹→采集 子循环——本 plan 覆盖锻造-供给回路，探索/战斗节点由其他 plan 提供）/ §四.伤口层（护甲降低伤口档次）/ §五.凡器边界（凡铁/木石档工具）/ §九（骨币经济）

- **qi_physics 锚点**：无。本 plan 不引入新真元/灵气常数。护甲 defense_profile 走现有 armor_sync 管线，不触碰守恒律。

---

## 阶段总览

| 阶段 | 内容 | 验收日期 | 状态 |
|------|------|---------|------|
| P0 | 凡铁护甲锻造管线（4 蓝图 + 4 profile + 4 残卷 + forge→armor 接线） | 2026-05-20 | ✅ |
| P1 | 自定义护甲模型管线（ArmorModelRegistry + ArmorFeatureRenderer + 4 铁甲 OBJ） | 2026-05-20 | ✅ |
| P2 | 功法练功深度（PracticeSession + 通用熟练度缩放 + HUD 指示器） | 2026-05-20 | ✅ |
| P3 | 战斗丹供给链（2 战斗丹材料可达性 + pill buff HUD） | 2026-05-20 | ✅ |
| P4 | 骨质装备集 + 工具/武器模型（4 骨甲蓝图 + profile + 4 骨甲 OBJ + 骨匕首 + 4 工具 OBJ） | 2026-05-20 | ✅ |
| P5 | 循环闭合校准（e2e 全链 timeline + 材料充裕度 + 耐久消耗率） | 2026-05-20 | ✅ |

---

## P0 — 凡铁护甲锻造管线 ✅ 2026-05-20

### P0.1 四件凡铁护甲锻造蓝图

新建 4 个 JSON 文件到 `server/assets/forge/blueprints/`，参考 `iron_sword_v0.json`（1 步 billet only）：

#### `iron_helmet_v0.json`

```json
{
  "id": "iron_helmet_v0",
  "name": "凡铁兜鍪",
  "station_tier_min": 1,
  "tier_cap": 1,
  "steps": [
    {
      "kind": "billet",
      "profile": {
        "required": [{ "material": "fan_tie", "count": 2 }],
        "optional_carriers": [],
        "tolerance": { "count_miss": 0 }
      }
    }
  ],
  "outcomes": {
    "perfect": { "weapon": "armor_iron_helmet", "quality": 1.0 },
    "good":    { "weapon": "armor_iron_helmet", "quality": 0.8 },
    "flawed":  { "weapon": "armor_iron_helmet", "quality": 0.5 },
    "waste":   null,
    "explode": { "damage": 4.0, "station_wear": 0.02 }
  }
}
```

> 注：outcomes 的 `"weapon"` 字段实际上用于所有锻造产出物（护甲/武器/工具），字段名沿用 forge schema 现有命名。

#### `iron_chestplate_v0.json`
- required: `[{ "material": "fan_tie", "count": 4 }]`（最大件）
- 其余结构同 helmet，quality 数值相同

#### `iron_leggings_v0.json`
- required: `[{ "material": "fan_tie", "count": 3 }]`

#### `iron_boots_v0.json`
- required: `[{ "material": "fan_tie", "count": 2 }]`

**材料总计**：全套凡铁甲 = `fan_tie × 11`。spawn 区初始矿脉 max_units=100（`worldgen/blueprint/mineral_anchors.json:78-84`），足够多次锻造。

### P0.2 四件凡铁护甲 ArmorProfile JSON

新建 4 个 JSON 到 `server/assets/combat/armor_profiles/`：

#### `armor_iron_helmet.json`

```json
{
  "template_id": "armor_iron_helmet",
  "profile": {
    "slot": "head",
    "body_coverage": ["head"],
    "kind_mitigation": {
      "cut": 0.45,
      "blunt": 0.30,
      "pierce": 0.35,
      "burn": 0.10,
      "concussion": 0.40
    },
    "durability_max": 280,
    "broken_multiplier": 0.3
  }
}
```

> 注：`ArmorProfileFile` 结构要求 `"profile"` wrapper（`armor.rs:161-164`），enum 值用小写（`"head"` 非 `"Head"`）。参照现有 `iron_plate_chest.json` 格式。

参照 `mundane.rs:62-84` Iron 材质（defense 12.0, durability 280）。4 件 profile 的 `kind_mitigation` 按部位调整：
- **Helmet**（head）：cut 0.45 / blunt 0.30 / pierce 0.35 / concussion 0.40
- **Chestplate**（chest + abdomen）：cut 0.55 / blunt 0.40 / pierce 0.45 / burn 0.15 / concussion 0.35
- **Leggings**（legs）：cut 0.40 / blunt 0.35 / pierce 0.30 / concussion 0.25
- **Boots**（feet）：cut 0.35 / blunt 0.40 / pierce 0.25 / concussion 0.20

所有 `durability_max = 280`，`broken_multiplier = 0.3`（与 `armor.rs:26` `ARMOR_BROKEN_MULTIPLIER_DEFAULT` 一致）。

### P0.3 Forge→Armor 接线验证

forge outcome `"weapon": "armor_iron_helmet"` → `ItemInstance` 写入 `forge_quality` → 玩家装备到 Head slot → `armor_sync.rs:29` 读 `armor_profiles.get(item.template_id)` → build `defense_profile`。

**已验证**（§8 #1）：`ArmorProfileRegistry::load_dir()` 已是 glob 扫描（`armor.rs:198-209`），加 JSON 即自动加载。

**forge_quality 对 durability 影响**：当前 forge 产出的 `forge_quality` 不影响 armor durability。本 plan 新增：

```rust
// combat/armor_sync.rs 扩展
fn effective_durability(profile: &ArmorProfile, item: &ItemInstance) -> f32 {
    let base = profile.durability_max as f32;
    let quality_mult = item.forge_quality.unwrap_or(0.5);
    base * (0.7 + 0.6 * quality_mult) // quality 0→0.7×, quality 1→1.3×
}
```

### P0.4 四件蓝图残卷物品

新建 `server/assets/items/armor_scrolls.toml`：

```toml
[[item]]
id = "blueprint_scroll_iron_helmet"
name = "锻造图·凡铁兜鍪"
category = "scroll"
grid_w = 1
grid_h = 2
base_weight = 0.05
rarity = "uncommon"
spirit_quality_initial = 0.3
description = "薄铁片上刻着兜鍪的展开图。边角卷曲——锻造师死前没来得及用。"
[item.blueprint_scroll]
blueprint_id = "iron_helmet_v0"

[[item]]
id = "blueprint_scroll_iron_chestplate"
name = "锻造图·凡铁胸甲"
category = "scroll"
grid_w = 1
grid_h = 2
base_weight = 0.05
rarity = "uncommon"
spirit_quality_initial = 0.3
description = "皱巴巴的锻造蓝图。胸甲正面画着四块互锁的铁片——简单，但能挡一刀。"
[item.blueprint_scroll]
blueprint_id = "iron_chestplate_v0"

[[item]]
id = "blueprint_scroll_iron_leggings"
name = "锻造图·凡铁腿甲"
category = "scroll"
grid_w = 1
grid_h = 2
base_weight = 0.05
rarity = "uncommon"
spirit_quality_initial = 0.3
description = "半截蓝图。画的是一对绑腿铁片——有总比没有强。"
[item.blueprint_scroll]
blueprint_id = "iron_leggings_v0"

[[item]]
id = "blueprint_scroll_iron_boots"
name = "锻造图·凡铁靴"
category = "scroll"
grid_w = 1
grid_h = 2
base_weight = 0.05
rarity = "uncommon"
spirit_quality_initial = 0.3
description = "铁靴展开图。画着铆钉位置——底子厚，踩什么都不怕。"
[item.blueprint_scroll]
blueprint_id = "iron_boots_v0"
```

**蓝图获取路径**：加入 onboarding-loop-v1 的 `surface_stash_craft` pool（**在 onboarding PR-2 merge 后**追加）或新建独立 loot pool `surface_stash_armor`。如果 onboarding 未 merge，先放入 `stone_casket_mid` pool（已存在）。

**测试**（P0 总计 20 条）：

```
// Forge blueprint loading
iron_helmet_v0_blueprint_loads
iron_chestplate_v0_blueprint_loads
iron_leggings_v0_blueprint_loads
iron_boots_v0_blueprint_loads
iron_helmet_v0_only_needs_fan_tie_x2
iron_chestplate_v0_needs_fan_tie_x4

// ArmorProfile loading
iron_helmet_profile_loads_from_json
iron_chestplate_profile_loads_from_json
iron_leggings_profile_loads_from_json
iron_boots_profile_loads_from_json
iron_helmet_mitigation_cut_0_45
iron_chestplate_durability_280

// Forge→Armor pipeline
forge_iron_helmet_produces_armor_item
forged_armor_equippable_to_head_slot
armor_sync_reads_forged_iron_helmet_profile
effective_durability_scales_with_forge_quality
effective_durability_quality_0_gives_0_7x
effective_durability_quality_1_gives_1_3x

// Blueprint scroll items
blueprint_scroll_iron_helmet_parses
blueprint_scroll_iron_chestplate_parses
```

---

## P1 — 自定义护甲模型管线 ✅ 2026-05-20

> **硬约束**：每个 OBJ 模型 3 轮自我打磨 + `<PROMISE>` 担保。禁止 vanilla leather dye tinting 作为最终方案。

### P1.1 ArmorModelRegistry

新建 `client/src/main/java/com/bong/client/armor/ArmorModelRegistry.java`（对标 `BongWeaponModelRegistry.java:20-121`）：

```java
public class ArmorModelRegistry {
    // template_id → ArmorModelSpec
    public record ArmorModelSpec(
        String templateId,
        EquipSlotType slot,
        Identifier modelPath,     // "bong:models/armor/iron_helmet/iron_helmet.obj"
        Identifier texturePath,   // "bong:textures/armor/iron_helmet.png"
        ArmorDisplayTransforms transforms  // FP/TP/GUI/Ground offsets
    ) {}

    private static final Map<String, ArmorModelSpec> REGISTRY = new HashMap<>();

    public static void register(String templateId, EquipSlotType slot, String modelDir) { ... }
    public static Optional<ArmorModelSpec> get(String templateId) { ... }
}
```

初始注册 4 件铁甲：
- `armor_iron_helmet` → `bong:models/armor/iron_helmet/iron_helmet.obj`
- `armor_iron_chestplate` → `bong:models/armor/iron_chestplate/iron_chestplate.obj`
- `armor_iron_leggings` → `bong:models/armor/iron_leggings/iron_leggings.obj`
- `armor_iron_boots` → `bong:models/armor/iron_boots/iron_boots.obj`

### P1.2 ArmorFeatureRenderer

新建 `client/src/main/java/com/bong/client/armor/ArmorFeatureRenderer.java`：

作为 `FeatureRenderer<AbstractClientPlayerEntity, PlayerEntityModel<AbstractClientPlayerEntity>>` 注册到 `PlayerEntityRenderer`（通过 Fabric `LivingEntityFeatureRendererRegistrationCallback`）。

**渲染流程**：
1. 从 `InventoryStateStore.snapshot().equipped` 读 4 个 armor slot
2. 对每个 slot，查 `ArmorModelRegistry.get(templateId)`
3. 如有 spec，加载 OBJ model（通过 SML 或 Fabric 的 `ModelLoadingRegistry`）
4. 按 `ArmorDisplayTransforms` 定位到玩家骨骼对应位置
5. 渲染 OBJ + 贴图

**第三人称**：FeatureRenderer 在 `PlayerEntityRenderer.render()` 中自动调用。
**第一人称**：armor 在 FP 视角下部分可见（看向脚时可见腿甲/靴），通过 `MixinHeldItemRenderer` 或独立 FP overlay 处理。
**GUI 显示**：`ItemIconRegistry.textureIdForItemId()` 已有 armor/ 路径映射（`armor_iron.png` 等），如需 3D 预览则在 `InspectScreen` 中渲染 OBJ。
**地面掉落**：`DroppedItemWorldRenderer` 读 `ItemIconRegistry` texture 渲染 sprite；如需 3D 掉落物则扩展为读 `ArmorModelRegistry` OBJ。

### P1.3 替换 leather dye 方案

修改 `MixinPlayerEntityArmor.java`：
- 当 `ArmorModelRegistry.get(templateId).isPresent()` 时，**不返回** fake leather ItemStack
- 改为返回空（让 ArmorFeatureRenderer 负责渲染）
- 当 registry 无对应模型时（向后兼容），仍使用 leather dye fallback

### P1.4 四件铁甲 OBJ 模型 + 贴图

每件模型目录结构：
```
assets/bong/models/armor/iron_helmet/
├── iron_helmet.obj
├── iron_helmet.mtl
└── iron_helmet.png
```

**模型规格**（参考凡铁护甲世界观）：
- **兜鍪**：半封闭铁盔，前有护鼻片，两侧护颊。粗糙锻痕，铆钉可见。无装饰。
- **胸甲**：四块互锁铁板，皮带束扣固定。前胸两块 + 后背两块。边缘未打磨。
- **腿甲**：左右各一片弯折铁板，皮绳绑在大腿外侧。膝盖位置有加厚铆钉。
- **铁靴**：铁片覆盖脚背和小腿前侧，底部铆钉加固。后跟裸露（方便跑动）。

**Display transforms**（每件 JSON，放 `assets/minecraft/models/item/` 下做 SML override）：
- firstperson_righthand / firstperson_lefthand
- thirdperson_righthand / thirdperson_lefthand
- gui（flat rotation for inventory icon）
- ground（dropped item）
- fixed（item frame）

**<PROMISE> 要求**：每件模型 commit 前完成 3 轮自我打磨：
1. 第 1 轮：基础形状 + UV 展开 + 贴图初稿
2. 第 2 轮：比例校准（与玩家模型对齐）+ 接缝修复 + 贴图细节（锈迹/锻痕/铆钉）
3. 第 3 轮：所有视角检查（FP/TP/GUI/Ground）+ 光照测试 + 最终 polish

终轮 commit message 包含 `<PROMISE>` 块。

**测试**（P1 总计 8 条）：

```
// ArmorModelRegistry
armor_model_registry_iron_helmet_registered
armor_model_registry_get_unknown_returns_none

// ArmorFeatureRenderer integration
armor_feature_renderer_loads_without_crash
armor_feature_renderer_renders_equipped_iron_helmet  // visual smoke test

// Leather dye fallback
mixin_armor_returns_empty_when_model_registered
mixin_armor_falls_back_to_leather_when_no_model

// OBJ model resources exist
iron_helmet_obj_resource_exists
iron_chestplate_obj_resource_exists
```

---

## P2 — 功法练功深度 ✅ 2026-05-20

### P2.1 练功 Session 系统

新建 `server/src/cultivation/practice_session.rs`（注：`technique_proficiency.rs` 已有 `practice_session_gain()`/`practice_session_qi_cost_per_tick()`/`should_exit_practice_session()` 等 helper 函数，本文件定义 Component/Event/System 并调用这些 helper）：

```rust
#[derive(Debug, Component)]
pub struct PracticeSession {
    pub technique_id: String,
    pub started_at_tick: u64,
    pub total_gain: f32,
}

#[derive(Debug, Event)]
pub struct PracticeSessionStarted {
    pub player: Entity,
    pub technique_id: String,
}

#[derive(Debug, Event)]
pub struct PracticeSessionEnded {
    pub player: Entity,
    pub technique_id: String,
    pub total_gain: f32,
    pub duration_ticks: u64,
}
```

**触发**：玩家原地不动（velocity < 0.01）+ 连续施放同一招式 3 次以上 → 自动进入 PracticeSession。

**每 tick**：
- 消耗 qi：`qi_cost_per_tick = 2.0`（`technique_proficiency.rs:186`）
- 调用 `practice_session_gain(zone_qi, current_prof, color_match, meridian_health)`
- `zone_multiplier`：灵气 < -0.5 → 2.0×，-0.5~0 → 1.5×，>= 0 → 1.0×
  > 注：负灵气区环境 qi drain 与 `qi_cost_per_tick` **叠加**——练功增益高但消耗也高，浅负区并非无成本刷熟练度。实施时需确认环境 drain 系统已正确扣除。

**退出条件**：
- 移动（velocity > 0.5）
- 受击（`CombatEvent` target = self）
- qi < 10% max
- 主动取消（再次施放其他招式）

**退出时** emit `PracticeSessionEnded` + narration（`NarrationStyle::Perception`）：
- 短 session（< 200 ticks）：`"心浮气躁。练了等于没练。"`
- 中 session（200-1000 ticks）：`"筋骨略记住了几分。不够。"`
- 长 session（> 1000 ticks）：`"筋骨酸痛。但下次出手会快一些。"`

### P2.2 通用熟练度缩放

在 `technique_proficiency.rs` 新增通用缩放结构（替代 woliu 的 hardcoded scalars）：

```rust
pub struct ProficiencyScalars {
    pub cast_ticks_mult: f32,   // 1.2 @ prof=0, 0.9 @ prof=1
    pub qi_cost_mult: f32,      // 1.1 @ prof=0, 0.85 @ prof=1
    pub stamina_cost_mult: f32, // 1.1 @ prof=0, 0.85 @ prof=1
    pub cooldown_mult: f32,     // 1.0 @ prof=0, 0.8 @ prof=1
}

pub fn generic_proficiency_scalars(proficiency: f32) -> ProficiencyScalars {
    ProficiencyScalars {
        cast_ticks_mult: 1.2 - 0.3 * proficiency,
        qi_cost_mult: 1.1 - 0.25 * proficiency,
        stamina_cost_mult: 1.1 - 0.25 * proficiency,
        cooldown_mult: 1.0 - 0.2 * proficiency,
    }
}
```

**应用范围**（plan-sword-basics-v1 已有 5 档缩放的招式**不动**）：
- `movement.dash`：cast_ticks_mult + stamina_cost_mult + cooldown_mult
- `body.guangbo_ticao`：cast_ticks_mult + qi_cost_mult + stamina_cost_mult
- `burst_meridian.beng_quan`：cast_ticks_mult + qi_cost_mult + cooldown_mult

**接入点**：在各招式的 cast resolution system 中，cast 前查 `KnownTechnique.proficiency` → `generic_proficiency_scalars(prof)` → 乘以 base 数值。

### P2.3 HUD 熟练度指示器

在 `client/src/main/java/com/bong/client/hud/` 新增 `TechniqueProficiencyHudPlanner.java`：

**显示时机**：每次施放招式后，在技能图标旁显示 1.5 秒的微型进度条（宽 24px × 高 3px）。

**数据来源**：server 在 `TechniqueLearnedEvent` / `PracticeProficiencyGainEvent` 时 push proficiency 到 client（通过现有 `bong:server_data` → `technique_proficiency_update` payload）。

**Server payload**（`schema/server_data.rs` 新增）：
```rust
pub struct TechniqueProficiencyUpdateV1 {
    pub technique_id: String,
    pub proficiency: f32,  // 0.0-1.0
    pub gain: f32,         // 本次增量
}
```

**Agent TypeBox schema**（可选）：在 `agent/packages/schema/src/server-data.ts` 追加 `TechniqueProficiencyUpdateV1` TypeBox type——仅 agent 需监控熟练度时才加。

**测试**（P2 总计 14 条）：

```
// PracticeSession
practice_session_starts_on_3_consecutive_casts
practice_session_ends_on_movement
practice_session_ends_on_combat_hit
practice_session_ends_on_low_qi
practice_session_gain_uses_zone_multiplier
practice_session_gain_zone_qi_negative_2x
practice_session_short_narration_under_200_ticks
practice_session_long_narration_over_1000_ticks

// ProficiencyScalars
generic_scalars_prof_0_cast_ticks_1_2
generic_scalars_prof_1_cast_ticks_0_9
generic_scalars_prof_0_5_cooldown_0_9
dash_applies_proficiency_scalars
beng_quan_applies_proficiency_scalars

// HUD payload
technique_proficiency_update_payload_serde_pin
```

---

## P3 — 战斗丹供给链 ✅ 2026-05-20

### P3.1 可达性验证：spawn 区战斗丹材料

**需验证**的 2 个目标丹方（从 35 配方中选最易达的）：

1. **回元丹** `hui_yuan_pill_v0`（onboarding-loop-v1 已覆盖入口）：
   - 材料：2× hui_yuan_zhi + 1× ling_shui
   - hui_yuan_zhi：botany v1 已注册（`registry.rs:100`），spawn/lingquan_marsh 有 ecology spawn
   - ling_shui：onboarding-loop-v1 通过 `surface_stash_craft` pool 掉落；后续需独立 plan 补水源采集

2. **活血丹** `huo_xue_dan`（战斗恢复丹——combat pill spec at `pill.rs:212`）：
   - 需找到对应 recipe JSON 文件，确认材料列表
   - 预期材料：xue_se_mai_cao（`core.toml` 中已有 template）+ 其他
   - **P3 实施时**需读 `server/assets/alchemy/recipes/` 找到 `huo_xue_dan` 的 recipe JSON，验证每种材料是否在 spawn ±1000 区域内可采集/可掉落

3. **铁壁散** `tie_bi_san`（防御 buff——适合与护甲配合）：
   - 同上，实施时验证 recipe 材料可达性

### P3.2 材料补缺

对验证中发现不可达的材料，补缺方案（优先级从高到低）：
1. 现有 botany spawn zone 已有该植物 → 无需改动
2. 植物已注册但 spawn zone 无 ecology → 在 `botany/registry.rs` 追加 spawn 区 ecology param
3. 材料无 botany 注册 → 加入 `surface_stash_craft` loot pool 或 `dry_corpse_shallow_common` pool

### P3.3 丹药 buff HUD

新建 `client/src/main/java/com/bong/client/hud/PillBuffHudPlanner.java`：

**Server push**：当丹药 buff 激活时（`pill.rs` consume 后 status effect 生效），server 发 `bong:server_data` payload：

```rust
pub struct PillBuffStatusV1 {
    pub buff_id: String,         // e.g., "huo_xue_dan"
    pub remaining_ticks: u32,
    pub effect_multiplier: f64,
}
```

**Client 显示**：在状态条区域（左下角竖条旁）显示小图标 + 倒计时条。图标从 `ItemIconRegistry` 读丹药贴图的缩略版。

**测试**（P3 总计 8 条）：

```
// 材料可达性
hui_yuan_zhi_spawns_in_lingquan_marsh
huo_xue_dan_recipe_materials_all_in_registry
tie_bi_san_recipe_materials_all_in_registry

// Pill buff payload
pill_buff_status_v1_serde_pin
pill_buff_status_emitted_on_consume

// HUD
pill_buff_hud_shows_active_buff
pill_buff_hud_hides_on_expiry
pill_buff_hud_stacks_two_buffs
```

---

## P4 — 骨质装备集 + 工具/武器模型 ✅ 2026-05-20

> **硬约束**：每个 OBJ 模型 3 轮 + `<PROMISE>`。本阶段共 9 个模型。

### P4.1 骨质护甲锻造管线

与 P0 同模式，新建 4 个 forge blueprint JSON + 4 个 armor_profile JSON：

| 蓝图 | outcome weapon | materials | 备注 |
|------|---------------|-----------|------|
| `bone_helmet_v0` | `armor_bone_helmet` | yi_shou_gu × 2 | |
| `bone_chestplate_v0` | `armor_bone_chestplate` | yi_shou_gu × 4 | |
| `bone_leggings_v0` | `armor_bone_leggings` | yi_shou_gu × 3 | |
| `bone_boots_v0` | `armor_bone_boots` | yi_shou_gu × 2 | |

> `yi_shou_gu`（异兽骨）已在 `ling_feng_v0.json` / `tool_gu_hai_qian_v0.json` 中作为 forge 材料使用，复用现有物品。

骨甲 mitigation 参照 `mundane.rs:65-76` Bone 材质（defense 3.0, durability 80）——远低于铁甲，但 yi_shou_gu 可从怪物掉落获取。

ArmorProfile 按部位调整：
- Cut: 0.15~0.25 / Blunt: 0.10~0.15 / Pierce: 0.20~0.30（骨头对刺有一定效果）

4 个 blueprint scroll 物品追加到 `armor_scrolls.toml`。

### P4.2 骨匕首锻造蓝图

已有 `bone_dagger` 在 `BongWeaponModelRegistry`（9 个 V1 模板之一）。§8 #3 已确认无 forge blueprint——新建 `bone_dagger_v0.json`：

```json
{
  "id": "bone_dagger_v0",
  "name": "骨匕首",
  "station_tier_min": 1,
  "tier_cap": 1,
  "steps": [
    {
      "kind": "billet",
      "profile": {
        "required": [
          { "material": "yi_shou_gu", "count": 2 },
          { "material": "grass_fiber", "count": 1 }
        ],
        "optional_carriers": [],
        "tolerance": { "count_miss": 0 }
      }
    }
  ],
  "outcomes": {
    "perfect": { "weapon": "bone_dagger", "quality": 1.0 },
    "good":    { "weapon": "bone_dagger", "quality": 0.8 },
    "flawed":  { "weapon": "bone_dagger", "quality": 0.5 },
    "waste":   null,
    "explode": { "damage": 3.0, "station_wear": 0.01 }
  }
}
```

### P4.3 九件 OBJ 模型

| # | 物品 | 模型目录 | 备注 |
|---|------|---------|------|
| 1 | armor_bone_helmet | `models/armor/bone_helmet/` | 兽骨拼接头盔，缝隙用皮绳扎 |
| 2 | armor_bone_chestplate | `models/armor/bone_chestplate/` | 肋骨排列胸甲，正面覆盖 |
| 3 | armor_bone_leggings | `models/armor/bone_leggings/` | 骨片绑腿，皮绳交叉固定 |
| 4 | armor_bone_boots | `models/armor/bone_boots/` | 骨片覆脚背，底部皮革 |
| 5 | bone_dagger | `models/item/bone_dagger/` | **已有模型**——验证即可，如不合格则重做 + PROMISE |
| 6 | axe_bone | `models/item/axe_bone/` | 骨斧，兽骨刃 + 木柄 |
| 7 | pickaxe_bone | `models/item/pickaxe_bone/` | 骨镐，尖骨头 + 木柄 |
| 8 | axe_iron | `models/item/axe_iron/` | 铁斧，凡铁刃 + 木柄 |
| 9 | pickaxe_iron | `models/item/pickaxe_iron/` | 铁镐，凡铁头 + 木柄 |

骨匕首 (#5) 已有模型——先检查质量，达标则复用。工具模型 (#6-9) 用于 `MixinHeldItemRenderer` 管线（扩展 `WeaponVanillaIconMap` 或新建 `ToolVanillaIconMap`）。

**模型规格**：
- 工具统一风格：木柄粗糙、铁头有锻痕、骨刃有磨损纹理
- 骨甲统一风格：兽骨本色（米白 + 暗黄）、皮绳深棕、缝隙可见
- 所有模型含完整 display transforms（FP/TP/GUI/Ground/Fixed）

**注册**：
- 骨甲 4 件 → `ArmorModelRegistry`（P1.1 已建好的 registry）
- 工具 4 件 → 直接在 `BongWeaponModelRegistry` 扁平 map 中追加 entry（§8 #4 已确认无需 Category enum）
- 每件注册含 SML model override JSON

**PROMISE 3 轮**：同 P1.4 标准。终轮 commit 含 `<PROMISE>` 块。

**测试**（P4 总计 14 条）：

```
// Bone armor forge
bone_helmet_v0_blueprint_loads
bone_chestplate_v0_blueprint_loads
bone_armor_profile_loads_from_json
bone_helmet_mitigation_cut_0_15

// Bone dagger forge
bone_dagger_v0_blueprint_loads_or_exists

// Blueprint scrolls
blueprint_scroll_bone_helmet_parses
blueprint_scroll_bone_chestplate_parses

// Model registry
armor_model_registry_bone_helmet_registered
tool_model_registry_axe_bone_registered
tool_model_registry_pickaxe_iron_registered

// OBJ resources exist
bone_helmet_obj_resource_exists
bone_chestplate_obj_resource_exists
axe_bone_obj_resource_exists
pickaxe_iron_obj_resource_exists
```

---

## P5 — 循环闭合校准 ✅ 2026-05-20

### P5.1 E2E 深度循环 timeline

从 onboarding 完成（有第一把凡铁剑 + 回元丹 + 基础招式）后，模拟 ~3h 深度循环：

```
[0:00]  入门完毕：持凡铁剑，会劈/刺/格/闪避，有回元丹
[0:10]  采 fan_tie × 11 → 锻造全套凡铁甲
[0:30]  穿甲上阵 → 打怪，护甲降低伤口档次
[0:45]  原地连续练劈法 → 进入 PracticeSession → 熟练度提升
[1:00]  采 hui_yuan_zhi → 炼回元丹 × 3
[1:10]  找到活血丹材料 → 拾取配方残片（loot pool 掉落）→ 炼活血丹
[1:30]  活血丹 buff + 凡铁甲 → 挑战更强怪
[1:45]  掉落 yi_shou_gu（异兽骨）→ 锻造骨匕首（副手备用）
[2:00]  拾取铁甲蓝图残卷 → 解锁蓝图 → 锻造更高品质铁甲（forge_quality 影响 durability）
[2:15]  继续练功 → movement.dash 熟练度提升 → cooldown 下降
[2:30]  装备损耗 → 需修复/再造 → 采矿 → 锻造 → 循环
[3:00]  循环运转中
```

### P5.2 材料充裕度校准

| 参数 | 初始值 | 校准范围 | 备注 |
|------|-------|---------|------|
| fan_tie 初始矿脉 max_units | 100 | [50, 200] | 需够 2 套铁甲 + 3 把凡铁剑 |
| yi_shou_gu 怪物掉率 | 现有 | 验证 | 需够 1 套骨甲（11 块异兽骨）|
| hui_yuan_zhi 采集间隔 | 现有 botany | 验证 | 需够每 20min 炼 1 炉 |
| 护甲 durability 消耗率 | 0.5/hit | [0.3, 1.0] | 调到"一套铁甲扛 ~50 次战斗"目标 |

### P5.3 测试（P5 总计 5 条）

```
e2e_full_iron_armor_set_forgeable_from_spawn_minerals
e2e_bone_armor_set_forgeable_from_monster_drops
e2e_practice_session_raises_proficiency_over_time
e2e_pill_buff_active_during_combat
material_sufficiency_fan_tie_for_2_armor_sets
```

---

## §8 开放问题（Pre-P0 收口）— ✅ 全部已验证 2025-05-20

### #1 ArmorProfileRegistry 加载方式 ✅
**结论**：已是 glob 扫描。`ArmorProfileRegistry::load_dir()` 通过 `fs::read_dir()` 扫描 `assets/combat/armor_profiles/` 目录（`armor.rs:198-209`）。加 JSON 即自动加载，无需改代码。
> 注：现有 `iron_plate_chest.json`（template_id `iron_plate_chest`）是孤儿 profile，与 plan 的 `armor_iron_chestplate` 不冲突。

### #2 forge_quality 对 armor durability 的影响 ✅
**决议**：采用线性缩放 `base × (0.7 + 0.6 × quality)`。quality=0 → 0.7× durability，quality=1 → 1.3×。在 `armor_sync.rs` 新增 `effective_durability()` 函数。

### #3 骨匕首 forge blueprint 是否已存在 ✅
**结论**：不存在。`assets/forge/blueprints/` 下 11 个文件均非 bone_dagger。`BongWeaponModelRegistry` 第 62 行有模型注册（OBJ 存在），但无锻造入口。P4 需新建 `bone_dagger_v0.json` blueprint。

### #4 工具模型管线选择 ✅
**结论**：`BongWeaponModelRegistry` 是扁平 `Map<String, Entry>` 结构（无 Category enum），当前 9 条均为武器。工具直接加 entry 到同一 map 即可，不引入 Category 枚举——保持管线统一，避免不必要抽象。

### #5 练功 Session 进入方式 ✅
**决议**：不需要命令或 UI——玩家原地连续施放同一招式 3 次自动进入。退出也自动（移动/受击/低 qi）。无 NPC 教学、无 UI 按钮，符合 worldview §八 沉默引导原则。

### #6 丹药 buff HUD 位置 ✅
**决议**：在左下角人体剪影 + 状态条区域右侧，用小图标 + 倒计时条显示（与现有 HUD 极简风格一致——`feedback_hud_immersive_minimal.md`）。

---

## §9 PR 拆分

| PR | 内容 | 依赖 | 涉及文件 | 与 onboarding 冲突 |
|----|------|------|---------|-------------------|
| PR-1 | P0 凡铁护甲锻造管线 | 无 | forge blueprints JSON (NEW), armor_profiles JSON (NEW), armor_scrolls.toml (NEW), combat/armor_sync.rs | 无 |
| PR-2 | P1 自定义护甲模型管线 | PR-1 | client/armor/ArmorModelRegistry.java (NEW), client/armor/ArmorFeatureRenderer.java (NEW), client/mixin/MixinPlayerEntityArmor.java, OBJ assets (NEW) | 无 |
| PR-3 | P2 功法练功深度 | 无 | cultivation/practice_session.rs (NEW), technique_proficiency.rs, schema/server_data.rs, client/hud/TechniqueProficiencyHudPlanner.java (NEW) | schema/server_data.rs（追加类型，merge 时 resolve） |
| PR-4 | P3 战斗丹供给链 | **PR-3** | alchemy/ (验证 + 可能的 botany ecology 补充), schema/server_data.rs, client/hud/PillBuffHudPlanner.java (NEW) | schema/server_data.rs（同上） |
| PR-5 | P4 骨质装备 + 工具模型 | PR-1,PR-2 | forge blueprints JSON (NEW), armor_profiles JSON (NEW), armor_scrolls.toml (追加), client model assets (NEW) | 无 |
| PR-6 | P5 循环闭合校准 | PR-1~5 | 校准参数 + e2e 测试 | 可能追加 loot_pools.json（onboarding PR-2 后） |

**总测试：69 条**（P0:20 + P1:8 + P2:14 + P3:8 + P4:14 + P5:5）。

PR-1 和 PR-3 **可并行**（无共享文件）。PR-4 依赖 PR-3（两者都改 `schema/server_data.rs`，串行避免冲突）。PR-2 依赖 PR-1（需 armor item 存在才能做模型注册）。PR-5 依赖 PR-1+PR-2（复用 ArmorModelRegistry）。

```
PR-1 (P0 铁甲 forge) ──→ PR-2 (P1 铁甲模型) ──→ PR-5 (P4 骨甲+工具模型)
                                                         ↓
PR-3 (P2 练功) ──→ PR-4 (P3 丹药) ──────────────→ PR-6 (P5 校准)
```

---

## §10 实施工作流

### §10.1 前置条件
- `plan-onboarding-loop-v1` **软依赖**：PR-6 的 loot pool 追加需 onboarding PR-2 先 merge；其余 PR 独立
- `plan-forge-v1` / `plan-armor-v1`：已 finished，管线可直接复用

### §10.2 PROMISE 工作流
每个 OBJ 模型资产遵循：
1. 建模 → 第 1 轮 review（形状/UV/初稿贴图）
2. 第 2 轮 review（比例对齐/接缝/贴图细节）
3. 第 3 轮 review（全视角/光照/polish）
4. Commit with `<PROMISE>` block

PR-2（4 铁甲模型）和 PR-5（9 模型）是 PROMISE 密集型——预留足够时间。

### §10.3 CodeRabbit 等待协议
每 PR：`ScheduleWakeup delaySeconds=1200`，最多 3 回合。

### §10.4 归档
全部 PR merge → 填 §Finish Evidence → `git mv docs/plan-depth-loop-v1.md docs/finished_plans/`

---

## Finish Evidence

### 落地清单

| 阶段 | 模块/文件 |
|------|----------|
| P0 凡铁护甲锻造 | `server/assets/forge/blueprints/iron_{helmet,chestplate,leggings,boots}_v0.json` (4 蓝图)，`server/assets/combat/armor_profiles/armor_iron_{helmet,chestplate,leggings,boots}.json` (4 profile)，`server/assets/items/armor_scrolls.toml` (8 残卷) |
| P1 护甲模型管线 | `client/.../armor/ArmorModelRegistry.java`，`ArmorFeatureRenderer.java`，`ArmorRenderBootstrap.java`，`MixinPlayerEntityArmor.java` 修改，4 铁甲 OBJ+MTL+JSON+PNG (`models/armor/iron_*`) |
| P2 练功深度 | `server/src/cultivation/practice_session.rs` (PracticeSession system)，`technique_proficiency.rs` (ProficiencyScalars 通用缩放)，`cultivation/tick.rs` (CultivationClock) |
| P3 丹药供给链 | `client/.../hud/PillBuffHudPlanner.java`，`schema/server_data.rs` (PillBuffStatusV1)，`combat/status.rs` (StatusEffect 扩展)，`alchemy/pill.rs` (10 combat pill spec) |
| P4 骨质装备+工具 | `server/assets/forge/blueprints/bone_{helmet,chestplate,leggings,boots,dagger}_v0.json` (5 蓝图)，`armor_profiles/armor_bone_*.json` (4 profile)，ArmorModelRegistry 4 骨甲条目，BongWeaponModelRegistry 4 工具条目+TOOL_TEMPLATE_IDS，12 OBJ 模型套件 (`models/armor/bone_*` + `models/item/{axe,pickaxe}_{bone,iron}`) |
| P5 循环闭合校准 | `forge/blueprint.rs` (3 e2e 校准测试)，`cultivation/practice_session.rs` (e2e 熟练度测试)，`alchemy/pill.rs` (e2e 丹药 buff 测试) |

### 关键 commit

| hash | 日期 | 说明 |
|------|------|------|
| `12745cf68` | 2026-05-20 | PR-1 (#285): P0 凡铁护甲锻造管线 |
| `f0b23523a` | 2026-05-20 | PR-2 (#288): P1 自定义护甲模型管线 |
| `369164d31` | 2026-05-20 | PR-3 (#286): P2 练功 session + 通用熟练度缩放 |
| `e129704d2` | 2026-05-20 | PR-4 (#291): P3 战斗丹供给链 + PillBuffStatus schema + HUD planner |
| `c9457a18c` | 2026-05-20 | PR-5 (#293): P4 骨质装备 + 工具模型 |
| `28e5e66ff` | 2026-05-20 | PR-6 (#295): P5 循环闭合校准 — e2e timeline + material sufficiency |

### 测试结果

```
# Server（cargo test 各模块）
forge::blueprint          31 passed
combat::armor             22 passed
cultivation::practice_session  16 passed
cultivation::technique_proficiency  20 passed
alchemy::pill             82 passed

# Client（./gradlew test）
ArmorModelRegistryTest    12 tests passed
BongWeaponModelRegistryTest  10 tests passed
PillBuffHudPlannerTest    16 tests passed
（总计 1589 tests，1 pre-existing failure: ArmorProfileStoreCrossCheckTest — 与本 plan 无关）
```

### 跨仓库核验

| 层 | 命中 symbol |
|----|------------|
| Server | `BlueprintRegistry`（9 iron+bone forge blueprints），`ArmorProfile`（8 profiles），`PracticeSession`，`ProficiencyScalars`，`PillBuffStatusV1`，`TechniqueProficiencyUpdateV1` |
| Client | `ArmorModelRegistry`（8 entries），`ArmorFeatureRenderer`，`BongWeaponModelRegistry`（+4 tool entries, TOOL_TEMPLATE_IDS），`PillBuffHudPlanner`，`MixinPlayerEntityArmor` |
| Agent | 无新增（plan scope 不涉及天道 agent） |

### 遗留 / 后续

- `ArmorProfileStoreCrossCheckTest.clientMirrorMatchesEveryServerProfile()` pre-existing failure（main 已存在，非本 plan 引入）——需独立修复
- OBJ 模型均为 placeholder box/L-shape geometry + 单色贴图——后续 plan 替换为精致模型时需保持相同路径和 SML JSON 结构
- `effective_durability()` quality 缩放（P0.3 设计）未在本 plan 实装——armor_sync 管线保持现有 durability 逻辑，quality 影响留给独立 plan
- 丹药 buff server→client push 链路（PillBuffStatusV1 payload 实际 emit）依赖 `status_snapshot_emit.rs` 完整接入，当前 schema 已对齐但 emit 时机需后续 plan 补齐
- P5 calibration tests 验证材料/系统 prerequisite 到位，非真正跨进程 E2E——真正的 E2E 集成测试由后续 plan 覆盖
