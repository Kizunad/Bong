# Plan: 引气入门循环 v1 — 散修遗缴 + 手搓引导 + 基础功法/流派/炼丹/炼器入门

> **问题**：tutorial 结束（引气）后玩家面临"内容荒漠"——修炼/采集/炼丹/锻造/战斗系统**全部存在**但在游戏内**无自然获取路径**。基础招式只能 dev 命令添加（生产模式 `KnownTechniques::default()` 返回空 Vec，`server/Cargo.toml:7` `dev-techniques` feature gate 验证），工具配方残卷不在任何 loot pool 中，炼丹首方/锻造首蓝图无发现入口。本 plan 在 spawn ±1000 格范围内铺设一条 ~2h 的入门循环路径，自然引导玩家走通"采集→手搓→战斗→炼丹→炼器→流派感知"全链路。
>
> **软依赖**：`plan-cultivation-pacing-v1` P0（速率调整）。P0-P4 不依赖其数值——loot pool / 物品模板 / 容器系统独立。仅 P4 校准阶段的时间预算需等速率落地后校准；如 pacing P0 未 merge，P4 先提交代码，标记校准参数为 `TODO_PACING`，待 pacing merge 后补 calibration commit。

---

## 接入面 Checklist（防孤岛）

- **进料**：
  - `inventory::ItemRegistry` — 查模板 ID（已有物品：`bone_coin_5` 在 `fauna.toml:148`、`hui_yuan_zhi` / `ling_shui` 在 `core.toml`、6 工具残卷在 `tools.toml:148-215`）
  - `craft::CraftRegistry` — 复用 6 个 auto-unlock 基础加工配方（`craft/mod.rs:787-856`，`unlock_sources: vec![]`）+ 6 个 scroll-unlock 工具配方（`craft/mod.rs:709-781`）
  - `alchemy::RecipeRegistry` — 复用 `hui_yuan_pill_v0`（`assets/alchemy/recipes/hui_yuan_pill_v0.json`：1 stage，材料 2× hui_yuan_zhi + 1× ling_shui）
  - `forge::BlueprintRegistry` — 复用 `iron_sword_v0`（`assets/forge/blueprints/iron_sword_v0.json`：1 步 billet only，材料 fan_tie × 3，**无 za_gang，无 tempering**）
  - `world::poi_novice` — 复用 `PoiNoviceKind` enum（11 variant，`poi_novice.rs:18-30`）+ `PoiRespawnStore`（`poi_respawn_tick.rs:46`）
  - `world::loot_pool::roll_loot_pool()` — 纯模板 item factory，无 qi 参与（`loot_pool.rs:94-196`）
  - `cultivation::technique_scroll` — `read_combat_technique_scroll()` + `learn_technique_if_allowed()` + `LearnSource` enum 5 variant（`technique_scroll.rs:26-32`）
  - `cultivation::technique_observe::evaluate_observe_attempt()` — 观战学招（yellow 基础概率 0.05 即 5%，`technique_observe.rs:70`）
  - `botany::BotanyKindRegistry` — `hui_yuan_zhi` 已注册（`registry.rs:100,311,355,399`，有 ecology spawn param `registry.rs:666`）
  - `mineral` — spawn 区有 `fan_tie` 教学矿脉（`worldgen/blueprint/mineral_anchors.json:78-84`，pos [16,70,16] radius 18 max_units 100）

- **出料**：
  - 新物品模板 → `inventory::ItemRegistry`（5 基础招式残卷 + 2 黄阶残卷 + 1 丹方 fragment 残卷 = 8 件新建）+ 复用已有 `scroll_body_guangbo_ticao`（`body_scrolls.toml:4`）+ 复用已有 `blueprint_scroll_iron_sword`（`forge.toml:62`）
  - 新 `RecipeFragmentSpec` struct → `inventory/mod.rs`（TOML `[item.recipe_fragment]` 解析）
  - 新 loot pool → `server/loot_pools.json`（3 个地表缓存池）
  - 新 `PoiNoviceKind::SurfaceStash` variant → `world::poi_novice`
  - 新 `ContainerKind::SurfaceStash` variant → `world::tsy_container`
  - 新 `LearnSource::CombatInsight` variant → `cultivation::technique_scroll`（首战自学闪避）
  - 新 VFX event variant `SurfaceStashOpen` → `schema/vfx_event.rs` + `network/vfx_event_emit.rs`
  - `ContainerKindV1::SurfaceStash` variant → `schema::server_data`（IPC schema）
  - loot pool 追加 → 现有 `dry_corpse_shallow_common` / `stone_casket_mid` pool

- **共享类型 / event**：
  - 复用 `TechniqueScrollReadEvent` / `TechniqueLearnedEvent` / `ScrollReadOutcome`
  - 复用 `ContainerKind` enum（新增 `SurfaceStash` variant）
  - 复用 `PoiNoviceKind` enum（新增 `SurfaceStash` variant）
  - 新增 `LearnSource::CombatInsight` variant（**不**动 `craft::events::InsightTrigger`——那是手搓配方解锁专用 enum）
  - 新增 `TutorialHook::CraftHintShown` variant（一次性 toast 触发）
  - 新增 `ContainerKindV1::SurfaceStash` variant（IPC schema enum）

- **跨仓库契约**：
  - Server：新物品 TOML + 新 loot pool JSON + 新 container/POI variant + LearnSource variant + 首战 hook
  - Client：`ContainerKindV1::SurfaceStash` 新 variant 映射到 `DryCorpse` 搜索动画（`client_request_handler.rs` 的 container switch 加 fallback）+ `TutorialHook::CraftHintShown` toast 渲染（复用现有 `spawn_tutorial` toast 管线）
  - Agent（TypeBox schema）：`ContainerKindV1` enum 追加 `"surface_stash"` variant；`TutorialHookV1` enum 追加 `"craft_hint_shown"`
  - Agent（天道 narration）：3 条 narration 走现有 `bong:server_data` → `perception_text` 管线（`NarrationStyle::Perception` scope=player，`schema/common.rs:53-59`）

- **worldview 锚点**：§三（修炼体系）/ §五（战斗流派，无职业锁）/ §八（天道沉默引导 O.13）/ §九（骨币经济）/ §十（灵草/矿石/残卷获取）/ §十二（运数期 3 死安全）

- **qi_physics 锚点**：无。本 plan 不引入新真元/灵气常数，不触碰守恒律。loot rolling 不消耗 qi（`loot_pool.rs` 纯模板 factory）。

---

## 阶段总览

| 阶段 | 内容 | 验收日期 | 状态 |
|------|------|---------|------|
| P0 | 散修遗缴系统（地表容器 + 3 loot pool + POI 布点 + client schema） | 2026-05-20 | ✅ |
| P1 | 基础功法获取（10 残卷物品 + 首战自学闪避 + 工具残卷入池 + loot pool 追加） | 2026-05-20 | ✅ |
| P2 | 手搓+炼丹+炼器引导（craft hint toast + 丹方 fragment + 蓝图残卷 + ling_shui 入池） | 2026-05-20 | ✅ |
| P3 | 流派感知（黄阶残卷入池 + style 倾向 narration hook） | 2026-05-20 | ✅ |
| P4 | 校准 + 集成验收（e2e timeline + 掉率调参） | 2026-05-21 | ✅ |

---

## P0 — 散修遗缴系统 ✅ 2026-05-20

### P0.1 地表容器变体：`SurfaceStash`

**新增 `ContainerKind::SurfaceStash`**（`server/src/world/tsy_container.rs`）：

```rust
SurfaceStash,  // 散修遗缴：地表可见，无需钥匙，搜索 60 ticks
```

在 `ContainerKind` 的 match 臂全部补上 `SurfaceStash` case：
- `base_search_ticks()` → 60
- `required_key()` → None
- `is_skeleton()` → false
- `as_str()` → `"surface_stash"`
- `from_str("surface_stash")` → `Ok(Self::SurfaceStash)`

**IPC schema 扩展**（`server/src/schema/server_data.rs`）：
- `ContainerKindV1` enum 追加 `SurfaceStash` variant
- `container_kind_wire()`（`network/tsy_container_search_emit.rs:239-245`）追加 `ContainerKind::SurfaceStash => ContainerKindV1::SurfaceStash`

**Agent TypeBox schema**（`agent/packages/schema/src/container-interaction.ts`）：`ContainerKindV1` union 追加 `"surface_stash"` variant。

**Client Java 映射**：Client 侧 container kind switch（Fabric mod 的 container render handler）对未识别的 variant 做 DryCorpse fallback——如果已有 default/else 分支则无需改动，否则追加 fallback case。具体文件视 client 侧 container UI 代码位置而定（当前 Fabric mod 中 container kind 映射在 HUD render 逻辑内）。

**Respawn 机制**——**不**新增 `SurfaceStashState` 结构体，**复用**现有 `PoiRespawnStore`（`poi_respawn_tick.rs:46`）：

在 `PoiRespawnState::is_server_tick_ready()` 的 match 中追加：

```rust
PoiNoviceKind::SurfaceStash => {
    elapsed >= 3600  // 3min @ 20tps
}
```

搜索完成后调用 `store.mark_searched(poi_id)`（**新增方法**：将 `PoiRespawnState.last_server_tick` 设为当前 tick），3600 tick 后 `is_server_tick_ready()` 返回 true → `poi_respawn_tick` system 重新 spawn `LootContainer` entity（`depleted: false`）。

**Per-player 限频**：每个遗缴对每个玩家每 real-time 24h 只产出 3 次。在 `LootContainer` 搜索完成 hook 中追加：

```rust
// server/src/world/tsy_container_search.rs 扩展
pub struct SurfaceStashPlayerLimit {
    /// (poi_id, player_uuid) → search_count_today
    limits: HashMap<(String, Uuid), u8>,
    last_reset_wall_clock: u64,
}
```

`search_count >= 3` 时搜索返回空（遗缴外观不变但搜不出东西）。每 real-time 24h 重置。

**视觉**：3×2×1 方块堆，用 `minecraft:gray_concrete_powder`（底层 3×2）+ 1 个 `minecraft:brown_mushroom_block[up=false,down=false,north=false,south=false,east=true,west=false]` 角块模拟布包角。**非 NBT 结构**——在 `LootContainer` spawn 时由 `place_surface_stash_blocks()` 函数直接写 4 个方块（runtime block placement，与 spawn_tutorial coffin 同模式 `spawn_tutorial.rs:680-720`）。

交互提示：玩家 3 格内时，server 发 `bong:server_data` perception_text `"一堆灰扑扑的东西。"` 替代专用 interaction hint（当前无 `ContainerInteractHint` 管线——**不新建**，用现有 narration 管线传递即可；后续如多种容器需统一 hint 再抽象）。

搜索中视觉反馈：60 tick 期间 player 播放 `idle_crouch` 动画（已有，无需新增 PlayerAnimator JSON）。搜索完成时 server emit VFX event（`schema/vfx_event.rs` 新增 variant `SurfaceStashOpen`，`network/vfx_event_emit.rs` 新增 match arm）：
- 粒子 spec（写入 VFX event payload，client 侧按 spec 渲染）：
- 数量：3
- lifetime：15 tick
- 速度：(0, 0.1, 0) ±0.02 扰动
- 颜色：`#8B7355`（灰土色）
- spawn 模式：burst
- 贴图：`bong:textures/particle/dust_puff.png`（复用现有灰尘粒子）
- Client VfxPlayer 类名：`SurfaceStashOpenVfx`（client/src Fabric mod 内新增）
- `bong:vfx_event` ID：`"surface_stash_open"`

搜索完成音效（audio_recipe JSON）：
```json
{
  "layers": [
    { "sound": "entity.item.pickup", "pitch": 0.6, "volume": 0.4, "delay_ticks": 0 },
    { "sound": "block.gravel.break", "pitch": 0.8, "volume": 0.3, "delay_ticks": 4 }
  ]
}
```

**Narration**（`NarrationStyle::Perception`，scope=player）：
- 首次搜索遗缴：`"灰烬下有东西。是谁死在这里都不知道了。"`
- 搜到招式残卷（`category == "scroll"` 且 `technique_scroll_spec.is_some()`）：`"死人的功法。能学多少看你造化。"`
- 搜到配方/蓝图残卷：`"丹方残页。字迹模糊——写的人大概也没活多久。"`

### P0.2 三级 loot pool

在 `server/loot_pools.json` 新增 3 个 pool。所有 `template_id` 已在 `ItemRegistry` 中验证存在：

#### `surface_stash_basic`（spawn ±500）

```json
{
  "rolls": [1, 2],
  "entries": [
    { "template_id": "crude_wood",          "weight": 30, "count": [2, 4] },
    { "template_id": "stone_chunk",         "weight": 25, "count": [1, 3] },
    { "template_id": "grass_fiber",         "weight": 25, "count": [3, 6] },
    { "template_id": "spirit_grass",        "weight": 10, "count": [1, 2] },
    { "template_id": "fengling_bone_coin",  "weight": 8,  "count": [1, 3] },
    { "template_id": "healing_herb_bundle", "weight": 2,  "count": [1, 1] }
  ]
}
```

#### `surface_stash_scroll`（spawn ±800）

```json
{
  "rolls": [1, 2],
  "entries": [
    { "template_id": "scroll_technique_sword_cleave",  "weight": 25, "count": [1, 1] },
    { "template_id": "scroll_technique_sword_thrust",  "weight": 20, "count": [1, 1] },
    { "template_id": "scroll_technique_movement_dash", "weight": 20, "count": [1, 1] },
    { "template_id": "scroll_gathering_axe_bone",      "weight": 15, "count": [1, 1] },
    { "template_id": "scroll_gathering_pickaxe_bone",  "weight": 10, "count": [1, 1] },
    { "template_id": "ci_she_hao",                     "weight": 5,  "count": [1, 2] },
    { "template_id": "fengling_bone_coin",             "weight": 5,  "count": [2, 5] }
  ]
}
```

#### `surface_stash_craft`（spawn ±1000，靠近 POI）

```json
{
  "rolls": [1, 3],
  "entries": [
    { "template_id": "scroll_technique_sword_parry",   "weight": 12, "count": [1, 1] },
    { "template_id": "scroll_body_guangbo_ticao",    "weight": 8,  "count": [1, 1] },
    { "template_id": "fragment_alchemy_hui_yuan_pill",  "weight": 12, "count": [1, 1] },
    { "template_id": "blueprint_scroll_iron_sword",    "weight": 12, "count": [1, 1] },
    { "template_id": "scroll_gathering_axe_iron",      "weight": 10, "count": [1, 1] },
    { "template_id": "scroll_gathering_pickaxe_iron",  "weight": 10, "count": [1, 1] },
    { "template_id": "hui_yuan_zhi",                   "weight": 10, "count": [1, 2] },
    { "template_id": "ling_shui",                      "weight": 8,  "count": [1, 2] },
    { "template_id": "ci_she_hao",                     "weight": 8,  "count": [1, 3] },
    { "template_id": "ning_mai_cao",                   "weight": 5,  "count": [1, 1] },
    { "template_id": "fengling_bone_coin",             "weight": 5,  "count": [3, 8] }
  ]
}
```

注意 `ling_shui` 入池——当前灵水无任何获取路径（无采集 target、无 loot drop、无 NPC 出售），本 plan 通过遗缴掉落解决（§8.1 #4）。

### P0.3 散修遗缴 POI 布点

**新增 `PoiNoviceKind::SurfaceStash`**（`server/src/world/poi_novice.rs`），追加 `as_str` / `from_str` / `first_action_label`（`"第一次搜遗缴"`）。同时更新 `novice_kinds()` 返回类型 `[PoiNoviceKind; 11]` → `[PoiNoviceKind; 12]` 并在数组体末尾追加 `PoiNoviceKind::SurfaceStash`（`poi_novice.rs:416`）。

**布点方式**——**不**修改 `worldgen/scripts/poi_novice_selector.py`（当前 Python selector 每 POI 类型只产 1 个点，架构不支持多实例）。改为 **server-side runtime scatter**：

在 `server/src/world/poi_novice.rs` 新增 `scatter_surface_stashes()` startup system：
- 读取 `TerrainProviders` spawn zone 边界
- Poisson-disk 采样 12 个点（min_dist=200 格，与已有 POI min_dist=100 格）
- 分配 pool：距 spawn 中心 ≤500 → basic（5 个）、≤800 → scroll（4 个）、≤1000 → craft（3 个）
- craft 级优先选距 `AlchemyFurnace` / `ForgeStation` POI 200-400 格的位置
- 为每个点 spawn `PoiNoviceSite` entity + `LootContainer` entity + 写 stash 方块
- 坐标 deterministic：使用 world seed × POI index 做 PRNG seed

此方式与 `spawn_tutorial.rs` 的 lingquan/rat spawn 同模式——runtime Poisson scatter，非 worldgen pipeline。

**测试**（P0 总计 18 条）：

```text
// ContainerKind::SurfaceStash enum
surface_stash_search_ticks_is_60
surface_stash_not_locked
surface_stash_not_skeleton
surface_stash_as_str_roundtrip
surface_stash_serde_pin  // serialize → deserialize identity

// ContainerKindV1 schema
container_kind_v1_surface_stash_wire  // ContainerKind → ContainerKindV1 bridge
container_kind_v1_serde_pin_with_surface_stash

// Respawn via PoiRespawnStore
surface_stash_respawn_ready_at_3600_ticks
surface_stash_respawn_not_ready_at_3599_ticks
surface_stash_respawn_resets_depleted_flag

// Per-player limit
surface_stash_player_limit_allows_3_per_day
surface_stash_player_limit_blocks_4th_search
surface_stash_player_limit_resets_after_24h

// Loot pools
surface_stash_basic_all_templates_in_registry
surface_stash_scroll_all_templates_in_registry
surface_stash_craft_all_templates_in_registry

// POI scatter
scatter_surface_stashes_produces_12_in_spawn_1000
scatter_surface_stashes_min_spacing_200
```

---

## P1 — 基础功法获取 ✅ 2026-05-20

### P1.1 新增 9 个残卷/fragment 物品 + 复用 1 个已有残卷

新建 `server/assets/items/onboarding_scrolls.toml`。

**5 基础招式残卷（新建）** + **1 已有残卷（复用）**（category=`"scroll"`，复用 woliu_scrolls.toml 格式——只需 `[item.technique_scroll]` + `skill_id`，`kind` 字段有 default `"combat_technique"` 不显式写）：

| 物品 ID | 显示名 | skill_id | 稀有度 | 来源 |
|---------|--------|----------|--------|------|
| `scroll_technique_sword_cleave` | 《劈法残页》 | `sword.cleave` | common | 新建 |
| `scroll_technique_sword_thrust` | 《刺法残页》 | `sword.thrust` | common | 新建 |
| `scroll_technique_sword_parry` | 《格挡残页》 | `sword.parry` | uncommon | 新建 |
| `scroll_technique_sword_infuse` | 《注剑法》 | `sword.infuse` | uncommon | 新建 |
| `scroll_technique_movement_dash` | 《闪身步残页》 | `movement.dash` | common | 新建 |
| `scroll_body_guangbo_ticao` | 残卷·广播体操 | `body.guangbo_ticao` | common | **已有** `body_scrolls.toml:4` |

TOML 模板（每个）：

```toml
[[item]]
id = "scroll_technique_sword_cleave"
name = "《劈法残页》"
category = "scroll"
grid_w = 1
grid_h = 2
base_weight = 0.04
rarity = "common"
spirit_quality_initial = 0.3
description = "残破纸页，墨迹斑驳。上面画着一个人举剑过顶劈下的分解动作。"
[item.technique_scroll]
skill_id = "sword.cleave"
```

**2 黄阶招式残卷**（完整 TOML）：

```toml
[[item]]
id = "scroll_technique_burst_beng_quan"
name = "《崩拳残页》"
category = "scroll"
grid_w = 1
grid_h = 2
base_weight = 0.04
rarity = "rare"
spirit_quality_initial = 0.5
description = "泛黄的绢帛，边角焦黑。画着一拳从丹田发力直击的姿势——旁注密密麻麻，大半已不可辨。"
[item.technique_scroll]
skill_id = "burst_meridian.beng_quan"

[[item]]
id = "scroll_technique_zhenmai_parry"
name = "《振脉格残页》"
category = "scroll"
grid_w = 1
grid_h = 2
base_weight = 0.04
rarity = "rare"
spirit_quality_initial = 0.5
description = "蜡封铜管里抽出的薄纸。描述了以经脉震荡卸力的防御法——写者笔迹颤抖，似乎在重伤时记录。"
[item.technique_scroll]
skill_id = "zhenmai.parry"
```

**1 丹方 fragment 残卷**（`AlchemyItemData::RecipeFragment`）：

```toml
[[item]]
id = "fragment_alchemy_hui_yuan_pill"
name = "《回元丹方·残》"
category = "recipe_fragment"
grid_w = 1
grid_h = 2
base_weight = 0.05
rarity = "uncommon"
spirit_quality_initial = 0.4
description = "残卷只剩半页。画着一炉简单的丹方——两根回元枝、一瓢灵水，文火慢煮。"
[item.recipe_fragment]
recipe_id = "hui_yuan_pill_v0"
known_stages = [0]
max_quality_tier = 3
```

**TOML 解析扩展**：在 `inventory/mod.rs` 的 TOML 加载逻辑中，新增 `[item.recipe_fragment]` section 解析（与 `[item.technique_scroll]` → `TechniqueScrollSpec`、`[item.blueprint_scroll]` → `BlueprintScrollSpec` 同模式）：

```rust
// inventory/mod.rs 新增
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeFragmentSpec {
    pub recipe_id: String,
    pub known_stages: Vec<u8>,
    pub max_quality_tier: u8,
}
```

`ItemTemplate` 追加 `pub recipe_fragment_spec: Option<RecipeFragmentSpec>` 字段。TOML 加载时 `[item.recipe_fragment]` → `Some(RecipeFragmentSpec)`。

**运行时 use 流程**：物品 use 时（`category == RecipeFragment`），从 template 的 `recipe_fragment_spec` 构造 `RecipeFragment { recipe_id, known_stages, max_quality_tier }`，然后 emit `LearnRecipeFragmentIntent`。现有 `handle_recipe_fragment_learning()`（`alchemy/mod.rs:165`）接收 intent 后需扩展：如果 `ItemInstance.alchemy` 为 None 但 template 有 `recipe_fragment_spec`，则从 spec 构造 `AlchemyItemData::RecipeFragment` 后走 `learn_fragment()`。

`hui_yuan_pill_v0` 只有 1 个 stage（`at_tick: 0`），故 `known_stages: [0]` = 100% 覆盖 → `completeness_for_recipe()` 返回 `UsablePartial`（1/1 ≥ 50%）→ `max_quality_tier = 3`。效果等同"学会完整配方"。

**1 蓝图残卷**（复用 `forge.toml` 格式，`BlueprintScrollSpec` 已完整实装 `inventory/mod.rs:188,144,1386,1652-1662`）：

`blueprint_scroll_iron_sword` 已在 `forge.toml:62` 定义！无需新增。只需放入 loot pool。

### P1.2 首战自学闪避

**新增 `LearnSource::CombatInsight`** variant（`cultivation/technique_scroll.rs:26-32`）：

```rust
pub enum LearnSource {
    Scroll { item_id: String },
    Observe { observed_entity: Entity },
    Mentor { npc_entity: Entity },
    DyingMaster { npc_entity: Entity },
    DevCommand,
    CombatInsight,  // 新增：战斗中本能领悟
}
```

**触发条件**：`CombatEvent`（`combat/events.rs:160`，target 是 player entity 且造成伤害）或 `RatBiteEvent`（target 是 player entity）。在 `combat/resolve.rs` damage 结算 system 末尾追加 hook：

```rust
fn first_hit_dash_insight(
    mut combat_events: EventReader<CombatEvent>,
    mut rat_events: EventReader<RatBiteEvent>,
    mut techniques: Query<&mut KnownTechniques, With<Player>>,
    // ...
) {
    // 从 CombatEvent 中筛选 target 为 player 且有实际伤害的
    // 从 RatBiteEvent 中筛选 target 为 player 的
    // 合并两种伤害源
    for target in damaged_players {
        if !techniques.entries.iter().any(|e| e.id == "movement.dash") {
            learn_technique_if_allowed(techniques, cultivation, meridians, severed, "movement.dash", 0.0);
            emit TechniqueLearnedEvent { source: LearnSource::CombatInsight };
            emit narration;
        }
    }
}
```

注：`movement.dash` 经脉依赖为空（`required_meridians: &[]`，`known_techniques.rs:182`），引气即可学，`learn_technique_if_allowed` 不会因经脉未通而拒绝。

Narration（`NarrationStyle::Perception`，scope=player）：
`"皮肉记住了。比脑子快。"`

### P1.3 工具残卷 + 黄阶残卷入池

**追加到现有 loot pool**（单文件 `loot_pools.json` 修改）：

`dry_corpse_shallow_common` 追加：
- `scroll_gathering_axe_bone` weight=5 count=[1,1]
- `scroll_technique_burst_beng_quan` weight=1 count=[1,1]
- `scroll_technique_zhenmai_parry` weight=1 count=[1,1]

`stone_casket_mid` 追加：
- `scroll_technique_burst_beng_quan` weight=3 count=[1,1]
- `scroll_technique_zhenmai_parry` weight=3 count=[1,1]

**测试**（P1 总计 17 条）：

```text
// 8 新物品模板 parse + 1 已有物品验证
onboarding_scroll_sword_cleave_parses
onboarding_scroll_sword_thrust_parses
onboarding_scroll_sword_parry_parses
onboarding_scroll_sword_infuse_parses
onboarding_scroll_movement_dash_parses
existing_scroll_body_guangbo_ticao_in_registry  // 验证已有 body_scrolls.toml:4 可查到
onboarding_scroll_burst_beng_quan_parses
onboarding_scroll_zhenmai_parry_parses
fragment_alchemy_hui_yuan_pill_parses
recipe_fragment_spec_toml_roundtrip  // RecipeFragmentSpec serialize/deserialize identity

// technique_scroll skill_id 正确性
scroll_sword_cleave_skill_id_matches_definition  // technique_definition("sword.cleave") is Some

// LearnSource::CombatInsight
first_hit_learns_dash_on_combat_event
first_hit_learns_dash_on_rat_bite
first_hit_idempotent_if_dash_already_known
first_hit_narration_emitted

// loot pool 追加
dry_corpse_pool_contains_axe_bone_scroll
stone_casket_pool_contains_beng_quan_scroll
```

---

## P2 — 手搓 + 炼丹 + 炼器引导 ✅ 2026-05-20

### P2.1 手搓提示 toast

新增 `TutorialHook::CraftHintShown`（`spawn_tutorial.rs:52` enum）。

在 `inventory/mod.rs` 物品拾取 hook（`pickup_dropped_loot_instance()` 附近，line ~3175）中：首次拾取 `crude_wood` / `stone_chunk` / `grass_fiber` 任一 → 检查 `TutorialState.hooks_triggered` 不含 `CraftHintShown` → emit toast server_data payload（复用现有 `perception_text` 管线）+ 标记 hook。

Toast 内容（`NarrationStyle::Perception`，scope=player）：
`"碎木和草绳。死人留下的，比空手强。"`

不做 HUD 高亮——toast 足够引导，保持 worldview §八 沉默原则。

**Agent TypeBox schema**：`TutorialHookV1` enum 追加 `"craft_hint_shown"` variant。

### P2.2 丹方 fragment 使用流程

`fragment_alchemy_hui_yuan_pill` 物品 use（`category = recipe_fragment`）→ 从 template 的 `RecipeFragmentSpec` 构造 `RecipeFragment { recipe_id: "hui_yuan_pill_v0", known_stages: vec![0], max_quality_tier: 3 }` → emit `LearnRecipeFragmentIntent` → `handle_recipe_fragment_learning()` 调 `learned.learn_fragment(fragment, recipe)` → `LearnResult::Learned`。

玩家现在可以在 `LearnedRecipes.partial` 中看到 `hui_yuan_pill_v0` 的 partial knowledge（100% stages，tier 3 quality cap）→ 炼丹 UI 中出现该配方 → 可以开炉。

**材料获取路径**：
- 2× `hui_yuan_zhi`：BotanyKindRegistry 已注册（`registry.rs:100,311,666`），spawn/lingquan_marsh 区域有 ecology spawn。额外从 `surface_stash_craft` pool 掉落。
- 1× `ling_shui`：从 `surface_stash_craft` pool 掉落（P0.2）。当前灵水无其他获取路径，遗缴是唯一来源——后续 plan 应补水源采集机制。

### P2.3 锻造蓝图使用流程

`blueprint_scroll_iron_sword`（`forge.toml:62`，已存在！）物品 use → client 发 `ForgeLearnBlueprint` request（`client_request_handler.rs:2340-2418`）→ consume scroll → `learned.learn(blueprint_id)` → `LearnedBlueprints` 包含 `iron_sword_v0`。

**材料获取路径**：
- 3× `fan_tie`（凡铁矿）：spawn 区有教学矿脉（`mineral_anchors.json:78-84`，pos [16,70,16] radius 18 max_units 100），骨镐即可采
- **无 za_gang 需求**——`iron_sword_v0.json` 只有 1 步 billet，材料仅 `fan_tie × 3`

### P2.4 首次炼丹/锻造 toast

当 `realm ≥ Induce` 且对应 `LearnedRecipes`/`LearnedBlueprints` 非空 且从未完成过对应操作时，首次交互 emit toast：

丹炉：`"这炉子还能用。把材料放进去，小心火候。"`
锻造台：`"锤和砧还在。先试试凡铁——别上来就打灵器，会炸的。"`

均为 `NarrationStyle::Perception` scope=player，一次性（标记在 `TutorialState` 或独立 `OnboardingHints` component）。

**测试**（P2 总计 14 条）：

```text
// CraftHintShown
pickup_crude_wood_triggers_craft_hint_toast
craft_hint_only_once
craft_hint_any_base_material  // crude_wood / stone_chunk / grass_fiber all trigger
craft_hint_not_triggered_for_non_base_item  // e.g., spirit_grass

// 丹方 fragment 学习
fragment_hui_yuan_pill_learn_fragment_result_learned
fragment_hui_yuan_pill_partial_knowledge_covers_all_stages
fragment_hui_yuan_pill_max_quality_tier_3
fragment_hui_yuan_pill_idempotent_if_already_known

// 蓝图学习（复用 blueprint_scroll_iron_sword 已有物品）
blueprint_scroll_iron_sword_already_exists_in_forge_toml
iron_sword_v0_only_needs_fan_tie_x3  // verify no za_gang
iron_sword_v0_station_tier_min_is_1

// 首次 toast
alchemy_hint_on_first_furnace_interact
alchemy_hint_only_once
forge_hint_on_first_station_interact
```

---

## P3 — 流派感知 ✅ 2026-05-20

> **设计哲学**：worldview §五"无职业锁，流派 = 行为涌现"。本 plan **不**新增"选择流派"UI，不新增 NPC 脚本对战系统（big-brain 无 Script action，基建不存在）。流派感知通过两条现有管线实现：(1) 黄阶残卷掉落让玩家体验不同招式；(2) style_telemetry 攻击统计触发 narration 反馈。

### P3.1 黄阶残卷已入池

P1.3 已将 `scroll_technique_burst_beng_quan` 和 `scroll_technique_zhenmai_parry` 放入 `dry_corpse_shallow_common`（weight 1）和 `stone_casket_mid`（weight 3）。观战学招（`technique_observe.rs`）走现有管线——玩家在野外遇到 NPC 战斗时自然有 5% 概率（yellow 基础 `observe_learn_chance` = 0.05，`technique_observe.rs:70`）学到黄阶招式。

### P3.2 流派倾向 narration

在 `server/src/combat/style_telemetry.rs` 追加 hook：

当某一 `attacker_style` 的累计使用次数达到 10 次时（查 `StyleUsageCounter` resource），emit 一次性 narration：

`"你似乎偏好[爆脉]的打法。经脉记得——你的选择会刻进身体。"`（`NarrationStyle::Perception`，scope=player）

`StyleUsageCounter`：`HashMap<(Entity, String), u32>`，每次 `StyleBalanceTelemetryEventV1` emit 时 +1。每个 (player, style) pair 只触发一次 narration（追加 `HashSet<(Entity, String)>` 已通知集）。

**测试**（P3 总计 6 条）：

```text
style_tendency_counter_increments_on_attack
style_tendency_narration_at_10_uses
style_tendency_narration_once_per_style
style_tendency_narration_different_styles_independent
style_tendency_counter_ignores_npc_vs_npc
observe_yellow_technique_base_chance_0_05  // pin test
```

---

## P4 — 校准 + 集成验收 ✅ 2026-05-21

### P4.1 全链路 timeline

从 tutorial 完成（引气）开始，模拟 ~2h 新手体验：

```text
[0:00]  引气态，空手出生点
[0:05]  找到第 1 个 basic 遗缴 → crude_wood + grass_fiber + stone_chunk
[0:08]  手搓 wood_handle + grass_rope + grass_pouch（背包！）
[0:12]  找到第 1 个 scroll 遗缴 → sword.cleave 残卷 + axe_bone 残卷
[0:15]  学劈法 + 学骨斧配方
[0:18]  被鼠群攻击 → 自学 movement.dash → 第一次战斗
[0:30]  手搓骨斧 → 砍灵木 → 采集灵草
[0:45]  找到 craft 遗缴 → fragment_alchemy_hui_yuan_pill + blueprint_scroll_iron_sword + ling_shui
[0:50]  找到 alchemy_furnace POI → 学 fragment → 采 2× hui_yuan_zhi
[1:05]  第一次炼丹（回元丹）
[1:15]  找到 forge_station POI → 学蓝图 → 采 3× fan_tie
[1:30]  第一次锻造（凡铁剑）
[1:45]  继续修炼/探索 → 衔接 cultivation-pacing-v1 丹药加速循环
```

### P4.2 掉率校准参数

| 参数 | 初始值 | 校准范围 |
|------|-------|---------|
| basic stash 数量 | 5 | [3, 8] |
| scroll stash 数量 | 4 | [3, 6] |
| craft stash 数量 | 3 | [2, 4] |
| SurfaceStash respawn_ticks | 3600 | [2400, 7200] |
| per-player 24h search limit | 3 | [2, 5] |

### P4.3 测试（P4 总计 5 条）

```text
e2e_12_stashes_within_spawn_1000  // P0.3 已实现为 scatter_surface_stashes_produces_12_in_spawn_1000
basic_pool_base_material_weight_over_50_pct  // 期望产出 ≥ 首件工具所需
scroll_pool_technique_scroll_weight_over_40_pct  // 期望 ≥ 1 张招式残卷
craft_stashes_within_spawn_radius
iron_sword_v0_fan_tie_available_in_spawn_mineral_anchor
```

---

## §8 开放问题（Pre-P0 收口）

> 以下 9 个问题**已全部收口**。§8.1 决议如下。原表保留以备追溯，**实施时以 §8.1 决议为准**。

### §8.1 决议（pre-P0 收口，2026-05-20）

#### #1 bone_coin_5 物品模板

**决议**：`bone_coin_5` **存在**。`fauna.toml:148` 定义 id="bone_coin_5" name="封灵骨币 五" category=bone_coin rarity=common。`craft/mod.rs:710` 的 `BONE_COIN_TEMPLATE` 引用正确。`economy/mod.rs:61` 估值 5.0。无需修改。

**落点**：`server/assets/items/fauna.toml:148` / `server/src/craft/mod.rs:710`

#### #2 炼丹首方发现路径

**决议**：使用 `RecipeFragment` 路径（方案 A 变体）。新增 `fragment_alchemy_hui_yuan_pill` 物品，use 时构造 `RecipeFragment { recipe_id: "hui_yuan_pill_v0", known_stages: vec![0], max_quality_tier: 3 }`。`hui_yuan_pill_v0` 只有 1 stage → fragment 100% 覆盖 → `UsablePartial` + tier 3 quality。走 `alchemy::learned::learn_fragment()` 现有管线（`learned.rs:37`）。不用 `AlchemyLearnRecipe` client request（那是无 item-gating 的直接注入，不符合"知识需获取"调性）。

**落点**：`server/src/alchemy/learned.rs:37` + `server/src/alchemy/recipe_fragment.rs:8-12` + plan P1.1 / P2.2

#### #3 hui_yuan_zhi 是否可采集

**决议**：**已注册**。`botany/registry.rs:100` const + `registry.rs:311` enum BotanyPlantId::HuiYuanZhi + `registry.rs:666` ecology spawn spec。`lifecycle.rs:978,1006,1063` 有 lingquan_marsh 环境测试。spawn 区灵气 0.3 ≥ 采集阈值。无需新增注册。

**落点**：`server/src/botany/registry.rs:100,311,355,399,666`

#### #4 灵水获取路径

**决议**：当前 `ling_shui` **无任何获取路径**（`gathering/tools.rs:23-27` 只有 Herb/Ore/Wood 三种 target，无 Water）。本 plan 通过 `surface_stash_craft` pool 掉落灵水（P0.2），作为入门阶段唯一来源。后续应由独立 plan 补水源采集机制（例如 WaterSource POI + GatheringTargetKind::Water），但不在本 plan 范围。

**落点**：`server/src/gathering/tools.rs:23-27`（验证无 Water target）/ plan P0.2 `surface_stash_craft` pool

#### #5 spawn 区矿石分布

**决议**：`fan_tie` **在 spawn 区**（`worldgen/blueprint/mineral_anchors.json:78-84`，pos [16,70,16] radius 18 max_units 100，注释"出生点旁的小型凡铁矿——教学用脉"）。`iron_sword_v0.json` **只需 fan_tie × 3**（1 步 billet only，无 za_gang，无 tempering）。无需新增矿脉。

**落点**：`worldgen/blueprint/mineral_anchors.json:78-84` / `server/assets/forge/blueprints/iron_sword_v0.json`

#### #6 SurfaceStash respawn 无限材料

**决议**：采用 per-player 限频（选项 A）。每个遗缴对每个玩家每 real-time 24h 只产出 3 次。实现 `SurfaceStashPlayerLimit` resource（`HashMap<(String, Uuid), u8>` + 24h wall-clock reset）。3 次足够入门，不破坏资源稀缺性。

**落点**：`server/src/world/tsy_container_search.rs`（新增 resource）/ plan P0.1

#### #7 NPC 对战脚本

**决议**：**砍掉 P5 NPC 脚本对战**。big-brain 无 Script action 类型，通用对话 UI 不存在，实现需要全新 AI 范式——不在入门循环 scope 内。流派感知改为：(1) 黄阶残卷入现有 loot pool，(2) style_telemetry 攻击统计触发 narration。玩家在野外自然遇到 NPC 战斗时，现有 `evaluate_observe_attempt()` 管线已能提供 5% 概率观战学招。

**落点**：plan 重构——原 P5 拆分为 P1.3（残卷入池）+ P3（流派 narration hook）

#### #8 dev_default() 生产模式

**决议**：`dev-techniques` feature 正确 gated（`Cargo.toml:7`）。`#[cfg(feature = "dev-techniques")]` 在 `known_techniques.rs:17` 控制 `impl Default`。非 dev 模式 `KnownTechniques` derive `Default` = 空 Vec。生产模式新玩家从零招式开始。本 plan 的招式残卷是生产模式下唯一获取路径。

**落点**：`server/Cargo.toml:7` / `server/src/cultivation/known_techniques.rs:5,17-22`

#### #9 首战自学闪避触发条件

**决议**：`CombatEvent`（`combat/events.rs:160`，target=player，有实际伤害）**或** `RatBiteEvent`（target=player）。用 `LearnSource::CombatInsight` variant（**不**用 `craft::events::InsightTrigger`——那是手搓配方解锁专用 enum `craft/events.rs:28-32`，与技能学习无关）。`movement.dash` 经脉依赖为空（`required_meridians: &[]`，`known_techniques.rs:182`），引气即可学。

**落点**：`server/src/cultivation/technique_scroll.rs:26-32`（LearnSource enum）/ `server/src/combat/resolve.rs`（hook 插入点）/ `server/src/combat/rat_bite.rs`（RatBiteEvent 源）

---

## §9 PR 拆分

| PR | 内容 | 依赖 | 涉及文件 |
|----|------|------|---------|
| PR-1 | P0 散修遗缴系统 | 无 | tsy_container.rs, poi_novice.rs, poi_respawn_tick.rs, tsy_container_search.rs, loot_pools.json, schema/server_data.rs, network/tsy_container_search_emit.rs, schema/vfx_event.rs, network/vfx_event_emit.rs, **agent/packages/schema/src/container-interaction.ts** + 测试 18 条 |
| PR-2 | P1 基础功法 + loot pool 追加 | PR-1 | onboarding_scrolls.toml, technique_scroll.rs, combat/resolve.rs, combat/events.rs, loot_pools.json, inventory/mod.rs（RecipeFragmentSpec 新增） + 测试 16 条 |
| PR-3 | P2 手搓+炼丹+炼器引导 | PR-1,PR-2 | spawn_tutorial.rs, inventory/mod.rs, alchemy/mod.rs, **agent/packages/schema/src/spawn-tutorial.ts** + 测试 14 条 |
| PR-4 | P3 流派感知 | PR-2 | combat/style_telemetry.rs + 测试 6 条 |
| PR-5 | P4 校准 | PR-1~4 | 掉率参数调整 + e2e 测试 5 条 |

**总测试：60 条**。

PR-3 和 PR-4 **可并行**（PR-3 改 spawn_tutorial.rs / inventory/mod.rs / alchemy/mod.rs / agent schema；PR-4 改 style_telemetry.rs——**无共享文件**）。PR-2 独占 `loot_pools.json` 修改（黄阶残卷入池 + 工具残卷入池合并到同一 PR），避免 PR-3/4 冲突。

---

## §10 实施工作流

### §10.1 前置条件

- §8 开放问题全部收口 ✅（§8.1 决议已写）
- `plan-cultivation-pacing-v1` P0：**软依赖**——PR-1~4 可先行；PR-5 校准时间参数如 pacing 未 merge 则标记 `TODO_PACING`

### §10.2 PR 序列

```text
PR-1 (P0) ──→ PR-2 (P1) ──→ PR-3 (P2) [与 PR-4 并行]
                          └──→ PR-4 (P3) [与 PR-3 并行]
                                    ↓
                              PR-5 (P4 校准)
```

### §10.3 subagent 配置

```rust
Agent(
  subagent_type: "claude",
  model: "opus",
  prompt: "...任务...\n\nultrathink"
)
```

### §10.4 CodeRabbit 等待协议

每 PR：`ScheduleWakeup delaySeconds=1200`，最多 3 回合。修完 review 重新等 CR re-review。

### §10.5 归档

全部 PR merge → 填 §Finish Evidence → `git mv docs/plan-onboarding-loop-v1.md docs/finished_plans/`

---

## Finish Evidence

### 落地清单

| 阶段 | 模块/文件 |
|------|----------|
| P0 | `server/src/world/tsy_container.rs` (ContainerKind::SurfaceStash) · `server/src/world/poi_novice.rs` (PoiNoviceKind::SurfaceStash + scatter) · `server/src/world/tsy_container_search.rs` (SurfaceStashPlayerLimit) · `server/loot_pools.json` (3 新 pool) · `server/src/schema/server_data.rs` (ContainerKindV1::SurfaceStash) · `server/src/schema/vfx_event.rs` (SurfaceStashOpen) · `agent/packages/schema/src/container-interaction.ts` (surface_stash variant) |
| P1 | `server/assets/items/onboarding_scrolls.toml` (8 新物品) · `server/src/inventory/mod.rs` (RecipeFragmentSpec + TOML 解析) · `server/src/cultivation/technique_scroll.rs` (LearnSource::CombatInsight) · `server/src/cultivation/first_hit_dash.rs` (首战自学闪避系统) · `server/loot_pools.json` (残卷入池追加) |
| P2 | `server/src/world/spawn_tutorial.rs` (TutorialHook::CraftHintShown/FirstAlchemyHint/FirstForgeHint + 3 ECS hint 系统) · `server/src/schema/client_request.rs` (AlchemyLearnRecipeFragment) · `server/src/network/client_request_handler.rs` (fragment 学习管线) · `agent/packages/schema/src/spawn-tutorial.ts` (3 新 variant) |
| P3 | `server/src/combat/style_telemetry.rs` (StyleUsageCounter + track_style_tendency) · `server/src/combat/mod.rs` (系统注册) · `server/src/cultivation/technique_observe.rs` (pin 测试) |
| P4 | `server/src/world/loot_pool.rs` (basic_pool_base_material_weight_over_50_pct · scroll_pool_technique_scroll_weight_over_40_pct · iron_sword_v0_fan_tie_available_in_spawn_mineral_anchor) · `server/src/world/poi_novice.rs` (craft_stashes_within_spawn_radius) · P0.3 已实现 scatter_surface_stashes_produces_12_in_spawn_1000 |

### 关键 commit

| Hash | 日期 | 说明 |
|------|------|------|
| `5e2b528b0` | 2026-05-20 | PR-1: P0 散修遗缴系统 (#284) |
| `40afe0cc8` | 2026-05-20 | PR-2: P1 基础功法获取 (#287) |
| `7a5f84f61` | 2026-05-20 | PR-3: P2 手搓+炼丹+炼器引导 (#289) |
| `ec98e2180` | 2026-05-20 | PR-4: P3 流派感知 (#292) |

### 测试结果

```text
cargo test — 5827 passed, 0 failed
npm test (agent/packages/schema) — 410 passed
```

### 跨仓库核验

- **Server**：ContainerKind::SurfaceStash · PoiNoviceKind::SurfaceStash · LearnSource::CombatInsight · RecipeFragmentSpec · TutorialHook::{CraftHintShown,FirstAlchemyHint,FirstForgeHint} · StyleUsageCounter · first_hit_dash_insight
- **Agent**：ContainerKindV1 `"surface_stash"` · TutorialHookV1 `"craft_hint_shown"` / `"first_alchemy_hint"` / `"first_forge_hint"`
- **Client**：ContainerKind fallback to DryCorpse（已有 default 分支）

### 遗留 / 后续

- 灵水（ling_shui）仅从 surface_stash_craft 掉落——后续需独立 plan 补水源采集机制（WaterSource POI + GatheringTargetKind::Water）
- style_telemetry 流派倾向计数基于 DeathEvent（击杀）而非攻击——后续可在攻击解算链路新增 AttackStyleUsedEvent 改为按攻击计数
- P4 校准参数（stash 数量/respawn ticks/search limit）使用初始值，需 playtest 后微调
