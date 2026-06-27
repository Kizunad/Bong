use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Added, App, Client, Commands, Component, Despawned, Entity, EntityInteraction,
    EntityLayerId, Hand, InteractEntityEvent, Position, Query, Resource, Update, Username, Without,
};

use crate::cultivation::death_hooks::{PlayerRevived, PlayerTerminated};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::poison_trait::PoisonPillKind;
use crate::world::dimension::{CurrentDimension, DimensionKind};

/// Worldview §十二：死亡掉落应落在「死亡点」而不是「重生点」。
///
/// Combat 生命周期在判定死亡时把死亡瞬间坐标暂存到玩家实体上，
/// `apply_death_drop_on_revive` 在玩家重生结算时读取该坐标用于掉落落点。
///
/// 该组件只用于“死亡 → 重生”窗口内的临时锚点，不做持久化。
#[derive(Debug, Clone, Copy, Component, PartialEq)]
pub struct DeathDropAnchor {
    pub pos: [f64; 3],
}

/// plan-death-lifecycle-v1 §4b：寿元耗尽（老死）后，不应把遗物散落为地面掉落点。
/// 遗物应以“遗骸容器”的形式留在世界中供他人搜刮。
///
/// MVP：用假玩家实体承载遗骸，并在右键交互时把内容转移到拾取者背包。
#[derive(Debug, Component)]
pub struct RemainsContainer {
    pub items: Vec<RemainsItemRecord>,
    pub bone_coins: u64,
    pub player_list_entry: Entity,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemainsItemRecord {
    pub source_container_id: String,
    pub source_row: u8,
    pub source_col: u8,
    pub item: ItemInstance,
}

// plan-tsy-loot-v1 §1.2 — 上古遗物模板池。
pub mod ancient_relics;
pub mod external_container;
// plan-tsy-loot-v1 §4 — 干尸 component。
pub mod corpse;
// plan-food-v1 P2 — 灵食消费路径（consume_food + FoodRegen 临时修炼加速）。
pub mod food;
// plan-lingtian-process-v1 P1 — 在线 tick freshness cache + season/anqi multiplier.
pub mod freshness;
// plan-poi-novice-v1 §P1 — 新手 POI loot 表。
pub mod poi_loot;
pub mod spirit_treasure;
// plan-tsy-loot-v1 §3 — 秘境内死亡分流。
pub mod tsy_death_drop;
// plan-tsy-loot-v1 §2 — 99/1 上古遗物 spawn。
pub mod tsy_loot_spawn;
// plan-tsy-loot-v1 §8.2 — 端到端集成测试。
#[cfg(test)]
mod tsy_loot_integration_test;

pub const JS_SAFE_INTEGER_MAX: u64 = 9_007_199_254_740_991;
const DEFAULT_ITEMS_DIR: &str = "assets/items";
const DEFAULT_LOADOUT_PATH: &str = "assets/inventory/loadouts/default.toml";
const DEFAULT_PLAYER_MAX_WEIGHT: f64 = 45.0;

pub const MAIN_PACK_CONTAINER_ID: &str = "main_pack";
pub const SMALL_POUCH_CONTAINER_ID: &str = "small_pouch";
pub const FRONT_SATCHEL_CONTAINER_ID: &str = "front_satchel";

pub const EQUIP_SLOT_HEAD: &str = "head";
pub const EQUIP_SLOT_CHEST: &str = "chest";
pub const EQUIP_SLOT_LEGS: &str = "legs";
pub const EQUIP_SLOT_FEET: &str = "feet";
pub const EQUIP_SLOT_MAIN_HAND: &str = "main_hand";
pub const EQUIP_SLOT_OFF_HAND: &str = "off_hand";
// plan-layered-equip-v1 P0.1（决议 #17 / #9 / #8）：删除 false_skin / two_hand /
// treasure_belt_0..3 / back_pack / waist_pouch / chest_satchel 七个废弃装备槽。
// - 伪皮归 CHEST worn 层（蜕壳流读 CHEST 扫 false_skin_kind_for_item）
// - 双手武器放一手 held + 锁对侧手（weapon_two_handed 派生）
// - 法宝激活态改由灵宝 UI 触发位承载（不再有 belt 装备槽）
// - 背包按 ContainerSpec.equip_slot 指向身体槽（head/chest/legs/feet），作该槽 worn 层
/// plan-dandao-path-v1 §8.1 #2 — 变异多臂额外手槽 0。
#[allow(dead_code)]
pub const EQUIP_SLOT_EXTRA_HAND_0: &str = "extra_hand_0";
/// plan-dandao-path-v1 §8.1 #2 — 变异多臂额外手槽 1。
#[allow(dead_code)]
pub const EQUIP_SLOT_EXTRA_HAND_1: &str = "extra_hand_1";
/// 身体自带暗袋容器 id（不占装备槽，始终存在）。
#[allow(dead_code)]
pub const BODY_POCKET_CONTAINER_ID: &str = "body_pocket";
/// plan-layered-equip-v1 P0.6（决议 #17）— default.toml 静态背包容器占位 id。
/// `instantiate_inventory_from_loadout` 会把它重映射到运行时穿戴背包件的 `pack_<instance_id>`。
pub const LOADOUT_PACK_PLACEHOLDER_CONTAINER_ID: &str = "pack_grass_pouch";
/// 暗袋行数（2 行）。
#[allow(dead_code)]
pub const BODY_POCKET_ROWS: u8 = 2;
/// 暗袋列数（3 列）。
#[allow(dead_code)]
pub const BODY_POCKET_COLS: u8 = 3;
/// 玩家裸体基础负重（不含任何背包）。
#[allow(dead_code)]
pub const BASE_CARRY_CAPACITY: f64 = 15.0;

type JoinedClientsWithoutInventoryFilter = (Added<Client>, Without<PlayerInventory>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryRevision(pub u64);

/// plan-HUD-v1 §10.4 cast 默认时长（无 template 字段时使用）。
pub const DEFAULT_CAST_DURATION_MS: u32 = 1500;
/// plan-HUD-v1 §4.4 cooldown 默认（完成后冷却 ms）。
pub const DEFAULT_COOLDOWN_MS: u32 = 1500;

#[derive(Debug, Clone, PartialEq)]
pub struct ItemTemplate {
    pub id: String,
    pub display_name: String,
    pub category: ItemCategory,
    pub placeable: Option<String>,
    pub max_stack_count: u32,
    pub grid_w: u8,
    pub grid_h: u8,
    pub base_weight: f64,
    pub rarity: ItemRarity,
    pub spirit_quality_initial: f64,
    pub description: String,
    pub effect: Option<ItemEffect>,
    /// plan-HUD-v1 §10.4 / §4.1 cast 持续时间（ms）。
    pub cast_duration_ms: u32,
    /// plan-HUD-v1 §4.4 完成后冷却（ms）。中断短冷却另算固定值。
    pub cooldown_ms: u32,
    /// plan-weapon-v1 §1.1：武器特有属性。非武器恒为 None。
    pub weapon_spec: Option<WeaponSpec>,
    pub forge_station_spec: Option<ForgeStationSpec>,
    pub blueprint_scroll_spec: Option<BlueprintScrollSpec>,
    pub inscription_scroll_spec: Option<InscriptionScrollSpec>,
    pub technique_scroll_spec: Option<TechniqueScrollSpec>,
    /// plan-onboarding-loop-v1 P1.1 — 丹方 fragment 物品规格；category=RecipeFragment 时可填。
    pub recipe_fragment_spec: Option<RecipeFragmentSpec>,
    /// plan-backpack-equip-v1 P0 — 可装备容器规格；category=Container 时必填。
    pub container_spec: Option<ContainerSpec>,
    /// plan-shield-block-v1 P2 — 盾牌物理防御规格；category=Shield 时必填。
    /// 不继承 ArmorProfile（其 validate 硬拒非四体护甲槽）。
    pub shield_spec: Option<ShieldSpec>,
    /// plan-food-v1 P1 — 默认 shelflife profile ID；Some(id) 时 `runtime_instance_from_template`
    /// 在 tick=0 自动挂 `Freshness`，无需消费侧手动初始化。
    /// 食物类物品（category=Food）在 food.toml 内填此字段。
    pub shelflife_profile: Option<String>,
    /// plan-food-v1 P1 — shelflife 初始路径（`DecayTrack`）；配合 `shelflife_profile` 使用。
    /// None = 无 shelflife（shelflife_profile 也为 None 时）。
    pub shelflife_track: Option<crate::shelflife::DecayTrack>,
}

/// plan-shield-block-v1 P2 — 盾牌物理防御模板级别静态规格（不随 instance 变动）。
/// 凡人级物理盾，不触真元（qi_physics），与 ArmorProfile 独立。
/// - `block_ratio`：正面命中时削减伤害的比例（0.0..=0.7；worldview §五 凡人盾上限 0.7）。
/// - `durability_max`：盾的最大耐久点数（P3 按点数扣减）。
/// - `stamina_drain_per_s`：持续举盾每秒消耗体力。**P2 仅 validate/存储，运行时由常量
///   `SHIELD_DRAIN_PER_SEC = 3.0` 覆盖（两盾当前同值）；P4 将经 `shield_block_profile`
///   按熟练度接入此 per-shield 字段。配置非 3.0 的值在 P2 过验但暂不生效，勿误判为已接线。
#[derive(Debug, Clone, PartialEq)]
pub struct ShieldSpec {
    /// 正面命中削减比例（0.0..=0.7）。
    pub block_ratio: f64,
    /// 最大耐久点数（P3 用）。
    pub durability_max: f64,
    /// 每秒体力 drain（P2 validate/存储但运行时不消费——drain 用常量 SHIELD_DRAIN_PER_SEC=3.0；P4 接入按熟练度调整）。
    pub stamina_drain_per_s: f32,
}

impl ShieldSpec {
    /// 校验 ShieldSpec 字段合法性：
    /// - block_ratio 须在 (0, 0.7] 区间（worldview §五 凡人盾上限 0.7，不能为 0）。
    /// - durability_max 须 > 0。
    /// - stamina_drain_per_s 须 > 0 且有限。
    pub fn validate(&self, item_id: &str) -> Result<(), String> {
        if !self.block_ratio.is_finite() || self.block_ratio <= 0.0 || self.block_ratio > 0.7 {
            return Err(format!(
                "item `{item_id}` shield_spec.block_ratio {} must be in (0, 0.7] \
                 (worldview §五: 凡人盾不得压过修士防御)",
                self.block_ratio
            ));
        }
        if !self.durability_max.is_finite() || self.durability_max <= 0.0 {
            return Err(format!(
                "item `{item_id}` shield_spec.durability_max {} must be > 0",
                self.durability_max
            ));
        }
        if !self.stamina_drain_per_s.is_finite() || self.stamina_drain_per_s <= 0.0 {
            return Err(format!(
                "item `{item_id}` shield_spec.stamina_drain_per_s {} must be > 0 and finite",
                self.stamina_drain_per_s
            ));
        }
        Ok(())
    }
}

/// plan-backpack-equip-v1 P0 — 可装备容器（背包/囊/挎包）的模板级静态规格。
///
/// 物品模板 `category = Container` 时必填；其余 category 须缺省（None）。
/// 装备此物品后，`rebuild_containers_from_equipment` 会自动在 `PlayerInventory.containers`
/// 中维护对应的 `ContainerState`，容量和行列数由此规格决定。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContainerSpec {
    /// 容器行数（1..=16）。
    pub rows: u8,
    /// 容器列数（1..=16）。
    pub cols: u8,
    /// 此容器提供的额外负重上限（叠加到 BASE_CARRY_CAPACITY）。
    pub weight_capacity: f64,
    /// 必须装在哪个 equip_slot 上才有效（"back_pack" / "waist_pouch" / "chest_satchel"）。
    pub equip_slot: String,
    /// 每次操作扣除的耐久度比例（0.0 = 无损耗）。
    pub durability_cost_per_op: f64,
    /// plan-qi-handling-attrition-v1 P3 — 此容器内物品跳过搬运磨损。
    pub attrition_exempt: bool,
    /// plan-container-filter-and-completion-v1 P0 — 可接受物品筛选；None/empty = 全收。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accept_filter: Option<Vec<ContainerAcceptFilter>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerAcceptFilter {
    Category(ItemCategory),
    TemplatePrefix(String),
}

/// plan-weapon-v1 §1.1：武器模板级别的静态属性（不随 instance 变动）。
#[derive(Debug, Clone, PartialEq)]
pub struct WeaponSpec {
    pub weapon_kind: crate::combat::weapon::WeaponKind,
    pub base_attack: f32,
    /// 0=凡铁 · 1=灵器 · 2=法宝 · 3=仙器。
    pub quality_tier: u8,
    pub durability_max: f32,
    /// qi 技能消耗倍率（v1 默认 1.0）。
    pub qi_cost_mul: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForgeStationSpec {
    pub tier: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlueprintScrollSpec {
    pub blueprint_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InscriptionScrollSpec {
    pub inscription_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TechniqueScrollSpec {
    pub kind: String,
    pub skill_id: String,
}

/// plan-onboarding-loop-v1 P1.1 — 丹方 fragment 物品的模板级静态规格。
/// TOML `[item.recipe_fragment]` 解析后存入 `ItemTemplate.recipe_fragment_spec`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecipeFragmentSpec {
    pub recipe_id: String,
    pub known_stages: Vec<u8>,
    pub max_quality_tier: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemCategory {
    Pill,
    Herb,
    RecipeFragment,
    RecipeHint,
    Weapon,
    Armor,
    Treasure,
    BoneCoin,
    Tool,
    Scroll,
    Misc,
    Block,
    /// plan-container-filter-and-completion-v1 P0 — 矿石/矿物类散料。
    Mineral,
    /// plan-container-filter-and-completion-v1 P0 — 暗器类物品。
    Anqi,
    /// plan-container-filter-and-completion-v1 P0 — 液体类容器内容物。
    Liquid,
    /// plan-backpack-equip-v1 P0 — 可装备容器（背包/囊/挎包），携带 ContainerSpec。
    #[allow(dead_code)]
    Container,
    /// plan-food-v1 P0 — 灵食（熟肉 / 陈饼 / 灵果 / 陈酒 / 陈醋等），消费时触发 FoodRegen。
    Food,
    /// plan-shield-block-v1 P0 — 凡人级物理防御盾牌（wooden_shield / bone_shield），装备于 off_hand 槽。
    /// 全程不涉真元（纯体力消耗），与 ItemCategory::Armor 语义不同（ArmorProfile 硬拒非四槽）。
    Shield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    /// plan-tsy-loot-v1 §1.1 — 上古遗物，仅由 TSY 自然 spawn 产生，
    /// 灵质恒为 0（"无灵"），耐久作为"剩余使用次数"语义。
    Ancient,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ItemEffect {
    BreakthroughBonus {
        magnitude: f64,
    },
    QiRecovery {
        amount: f64,
    },
    MeridianHeal {
        magnitude: f64,
        target: String,
    },
    ContaminationCleanse {
        magnitude: f64,
    },
    ComposureRestore {
        magnitude: f64,
    },
    WoundHeal {
        magnitude: f64,
        target: Option<String>,
    },
    LifespanExtension {
        years: u32,
        source: String,
    },
    AntiSpiritPressure {
        duration_ticks: u64,
    },
    PoisonPill {
        pill_item_id: String,
    },
    CombatPill {
        pill_item_id: String,
    },
    /// plan-food-v1 P2 — 灵食消费：临时修炼加速。
    ///
    /// `bonus_factor`：加速比例（0.20 = +20% 修炼速度），挂 `CultivationAcceleration`。
    /// `duration_ticks`：效果持续 tick 数（通常 `GAME_DAY_TICKS * 2`）。
    FoodRegen {
        bonus_factor: f32,
        duration_ticks: u64,
    },
    /// plan-fauna-stitched-beast-v1 P3 — 异变兽核吸收：使用 `bian_yi_hexin` 时触发幻觉。
    ///
    /// 向玩家施加突破加成（同 `BreakthroughBonus`）并额外 emit `CoreAbsorptionHallucinationEvent`
    /// 触发 client 端感知幻觉 HUD（视野偏移/绿边像差/bar ±20% 随机偏移，绝不改实际值）。
    ///
    /// - `breakthrough_magnitude`：突破加成幅度（同 `BreakthroughBonus.magnitude` 语义）
    /// - `hallucination_duration_ticks`：幻觉持续 tick 数（P3 固定 200，约 10s @ 20TPS）
    BeastCoreAbsorption {
        breakthrough_magnitude: f64,
        hallucination_duration_ticks: u32,
    },
}

#[derive(Debug, Default)]
pub struct ItemRegistry {
    templates: HashMap<String, ItemTemplate>,
}

impl Resource for ItemRegistry {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadoutSpec {
    pub containers: Vec<ContainerState>,
    pub equipped: HashMap<String, SlotContents>,
    pub hotbar: [Option<ItemInstance>; 9],
    pub bone_coins: u64,
    pub max_weight: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContainerState {
    pub id: String,
    pub name: String,
    pub rows: u8,
    pub cols: u8,
    pub items: Vec<PlacedItemState>,
    /// plan-tarkov-backpack-v1 P0（决议 #1，方案 A）— 该容器归属的穿戴背包件 instance_id。
    ///
    /// 仅 `pack_<id>` 派生容器有值（`body_pocket` / 其它静态容器为 `None`）。
    /// `serde(default)` 容旧存档（旧档无此字段读为 `None`），load 时按 `pack_` 前缀回填，
    /// 不写回 DB、内存层每次加载幂等重算（见 `load_player_inventory_from_sqlite` 回填）。
    /// **P0 仅服务端内部使用，不下发 client**（client 可从 `pack_` 前缀反解；下发推迟到 P3）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_instance_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlacedItemState {
    pub row: u8,
    pub col: u8,
    pub instance: ItemInstance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemInstance {
    pub instance_id: u64,
    pub template_id: String,
    pub display_name: String,
    pub grid_w: u8,
    pub grid_h: u8,
    pub weight: f64,
    pub rarity: ItemRarity,
    pub description: String,
    pub stack_count: u32,
    pub spirit_quality: f64,
    pub durability: f64,
    /// plan-shelflife-v1 §0.4 / §2.1 — 物品保质期 NBT。
    /// `None` = 无时间敏感（凡俗工具 / 瑶器 等），`Some` = 接 shelflife 路径计算。
    pub freshness: Option<crate::shelflife::Freshness>,
    /// plan-mineral-v1 §2.2 — 矿物来源 item 的正典 mineral_id（如 `"fan_tie"`）。
    /// `None` = 非矿物物品 / 凡俗 item（打怪掉落 / creative 给的 vanilla 方块）；
    /// `Some` = `MineralDropEvent` 产出，`MineralRegistry::is_valid_mineral_id(..)` 保证正典性。
    /// 序列化省略 None 以兼容旧 snapshot（见 freshness）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mineral_id: Option<String>,
    /// plan-tsy-loot-v1 §1.3 — "剩余使用次数"。Ancient rarity 物品用此存 tier
    /// 1/3/5 的初始剩余次数，每次使用 -= 1，归零销毁。非 ancient 物品恒为 None；
    /// `durability` 字段保持 0..=1 normalized 语义不变（与 schema 边界对齐）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charges: Option<u32>,
    /// plan-forge-leftovers-v1 §2.2 — 炼器产物运行时品质；None = 非 forge 产物。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forge_quality: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forge_color: Option<crate::cultivation::components::ColorKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forge_side_effects: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forge_achieved_tier: Option<u8>,
    /// plan-alchemy-v2：炼丹产物 / 残卷 / 丹心线索的动态 NBT。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alchemy: Option<AlchemyItemData>,
    /// plan-niche-defense-v1 P3：抄家物品携带龛主异体真元残留。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lingering_owner_qi: Option<LingeringQi>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LingeringQi {
    pub owner: String,
    pub expire_at: u64,
}

fn attach_lingering_owner_qi(item: &mut ItemInstance, owner: String, expire_at: u64) {
    item.lingering_owner_qi = Some(LingeringQi { owner, expire_at });
}

pub fn attach_lingering_owner_qi_by_instance(
    inventory: &mut PlayerInventory,
    instance_id: u64,
    owner: String,
    expire_at: u64,
) -> bool {
    let Some(item) = inventory_item_by_instance_mut(inventory, instance_id) else {
        return false;
    };
    attach_lingering_owner_qi(item, owner, expire_at);
    bump_revision(inventory);
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AlchemyItemData {
    Pill {
        recipe_id: String,
        quality_tier: u8,
        effect_multiplier: f64,
        consecrated: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        side_effect: Option<crate::alchemy::recipe::SideEffect>,
    },
    RecipeFragment {
        fragment: crate::alchemy::recipe_fragment::RecipeFragment,
    },
    RecipeHint {
        hint: crate::alchemy::danxin::RecipeHint,
    },
    PillResidue {
        residue_kind: crate::alchemy::residue::PillResidueKind,
        produced_at_tick: u64,
        expires_at_tick: u64,
    },
}

#[derive(Debug)]
pub struct DefaultLoadout(pub LoadoutSpec);

impl Resource for DefaultLoadout {}

#[derive(Debug, Clone)]
pub struct InventoryInstanceIdAllocator {
    next: u64,
}

impl Resource for InventoryInstanceIdAllocator {}

impl Default for InventoryInstanceIdAllocator {
    fn default() -> Self {
        Self::new(1)
    }
}

impl InventoryInstanceIdAllocator {
    pub fn new(start: u64) -> Self {
        assert!(
            start <= JS_SAFE_INTEGER_MAX,
            "inventory instance id allocator start {start} exceeds JS safe integer max {JS_SAFE_INTEGER_MAX}"
        );
        Self { next: start }
    }

    pub fn next_id(&mut self) -> Result<u64, String> {
        let id = self.next;
        if id > JS_SAFE_INTEGER_MAX {
            return Err(format!(
                "inventory instance id allocation overflow: next id {id} exceeds JS safe integer max {JS_SAFE_INTEGER_MAX}"
            ));
        }

        self.next = self.next.saturating_add(1);
        Ok(id)
    }
}

/// plan-layered-equip-v1 P0.1 — 装备态分类（决议 #16）。
/// `Worn` = 穿戴层（计 worn cap、走 LIFO 栈语义）；`Held` = 手持（held-only，不计 cap）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipState {
    Worn,
    Held,
}

/// plan-layered-equip-v1 P0.1 — 单装备槽内容（决议 #1 方案B / #12 LIFO 栈语义）。
///
/// - `worn`：穿戴层 Vec，**语义 = 栈（LIFO，约定栈顶 = Vec 末尾）**。装备 = `worn.push`（push 到尾），
///   卸下 = `worn.pop()`（pop 从尾），**只有栈顶（`worn.last()`）能被拖下/卸下**，下层被压住需先脱上层。
/// - `held`：手持单件（Option，不分层、不计 worn cap）。
///
/// 空槽序列化为 `{worn:[],held:null}`。手槽（main_hand/off_hand/extra_hand_0/extra_hand_1）
/// worn 恒空（worn_cap=0，见 `worn_cap`）；身体槽（head/chest/legs/feet）held 恒空。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SlotContents {
    #[serde(default)]
    pub worn: Vec<ItemInstance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held: Option<ItemInstance>,
}

impl SlotContents {
    /// 仅含一件 worn 的槽（迁移 / loadout 装配用）。
    pub fn worn_single(item: ItemInstance) -> Self {
        SlotContents {
            worn: vec![item],
            held: None,
        }
    }

    /// 仅含 held 的槽（武器 / 工具）。
    pub fn held_single(item: ItemInstance) -> Self {
        SlotContents {
            worn: Vec::new(),
            held: Some(item),
        }
    }

    /// 栈顶（最上层 / `worn.last()`）只读访问入口（决议 #12）。
    pub fn worn_top(&self) -> Option<&ItemInstance> {
        self.worn.last()
    }

    /// 栈顶可变访问入口（决议 #12）。
    pub fn worn_top_mut(&mut self) -> Option<&mut ItemInstance> {
        self.worn.last_mut()
    }

    /// 槽是否完全为空（无 worn 层且无 held）。
    pub fn is_empty(&self) -> bool {
        self.worn.is_empty() && self.held.is_none()
    }

    /// 迭代槽内全部件（worn 全层 + held），用于桶④「迭代全件」。
    pub fn iter_all(&self) -> impl Iterator<Item = &ItemInstance> {
        self.worn.iter().chain(self.held.iter())
    }

    /// 可变迭代槽内全部件。
    pub fn iter_all_mut(&mut self) -> impl Iterator<Item = &mut ItemInstance> {
        self.worn.iter_mut().chain(self.held.iter_mut())
    }
}

/// plan-layered-equip-v1 P0.1 — 各槽 worn 层容量上限（决议 #6 / #14 / #17）。
///
/// - head / feet = 2；chest / legs = 3。
/// - main_hand / off_hand / extra_hand_0 / extra_hand_1 = 0（held-only，不计 worn cap）。
/// - 背包**无专属槽**（决议 #17）：背包按 `ContainerSpec.equip_slot` 占其指定身体槽
///   （head/chest/legs/feet）的一个 worn 层，受该身体槽 cap，无独立退化槽。
///
/// worn 栈 LIFO（决议 #12）只作用于 head/chest/legs/feet worn 槽（手槽 worn 恒空）。
pub fn worn_cap(slot: &str) -> u8 {
    match slot {
        EQUIP_SLOT_HEAD | EQUIP_SLOT_FEET => 2,
        EQUIP_SLOT_CHEST | EQUIP_SLOT_LEGS => 3,
        // 手槽 held-only，worn 恒空。
        _ => 0,
    }
}

/// **P5 hook — worn_cap 升级奖励（占位扩展点）**。
///
/// 当前恒返回 0，有效 cap = `worn_cap(slot) + worn_cap_bonus(slot, ...)` = 基础值不变。
///
/// 未来由境界 / 功法 / 法宝派生加成，需要以下前置工作才能接入：
/// - 境界加成：`worldview.md` §四 装备容量锚点（决议 #24，尚无 worldview 节号）
/// - 功法加成：修炼功法 modifier 系统设计
/// - 法宝加成：triggered_treasures passive 效果框架
///
/// 调用方可选择性传入上下文；调用方拿不到 cultivation/techniques 时直接调 `worn_cap(slot)`
/// 即可（等价于 bonus=0，行为与重构前完全一致）。
///
/// plan-layered-equip-v1 P5（决议 #24）— 升级源需 worldview 锚点，本 PR 不接任何升级源。
#[allow(dead_code)]
pub fn worn_cap_bonus(_slot: &str) -> u8 {
    // P5 占位：升级源未接，返回 0。接入升级源时删除 #[allow(dead_code)] 并实现派生逻辑。
    0
}

/// plan-layered-equip-v1 P0.1 — 物品装备态分类（决议 #16 / #17）。
///
/// - `Weapon | Tool` → Held（不计 worn cap）。
/// - `Armor | Treasure | Shield` + 伪皮物品 → Worn（计 worn cap）。
/// - `Container`（背包）→ Worn（占其 `ContainerSpec.equip_slot` 指定身体槽的一个 worn 层）。
/// - 其余（Hoe 等工具走 Tool）默认 Worn 兜底。
pub fn classify_equip_state(item: &ItemInstance, registry: &ItemRegistry) -> EquipState {
    match registry.get(&item.template_id).map(|t| t.category) {
        Some(ItemCategory::Weapon) | Some(ItemCategory::Tool) => EquipState::Held,
        _ => EquipState::Worn,
    }
}

/// plan-layered-equip-v1 P0.1（决议 #7）— 双手武器判定（spear/staff 派生）。
/// 双手兵器占一手 held + 锁对侧手（extra_hand 独立不锁）。
pub fn weapon_two_handed(kind: crate::combat::weapon::WeaponKind) -> bool {
    use crate::combat::weapon::WeaponKind;
    matches!(kind, WeaponKind::Spear | WeaponKind::Staff)
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct PlayerInventory {
    pub revision: InventoryRevision,
    pub containers: Vec<ContainerState>,
    pub equipped: HashMap<String, SlotContents>,
    pub hotbar: [Option<ItemInstance>; 9],
    pub bone_coins: u64,
    pub max_weight: f64,
    /// plan-layered-equip-v1 P4 — 法宝激活「触发位」（决议 #8）。
    ///
    /// 法宝激活态承载从已删除的 `treasure_belt` 装备槽迁到灵宝 UI 内的「触发位」。
    /// 容量上限 `TREASURE_TRIGGER_CAP`（默认 4 = 旧 belt 槽数，P5 可挂升级）。
    /// 与装备槽正交（决议 #16）：装备槽 worn 里的 treasure 仍是 worn 装备件
    /// （equipped=true / passive_active=false），唯有进入触发位才 passive_active=true。
    #[serde(default)]
    pub triggered_treasures: Vec<ItemInstance>,
}

/// plan-layered-equip-v1 P4 — 法宝触发位容量上限（默认 4 = 旧 treasure_belt 槽数，决议 #8）。
pub const TREASURE_TRIGGER_CAP: usize = 4;

/// **P5 hook — 法宝触发位有效容量（占位扩展点）**。
///
/// 当前恒返回 `TREASURE_TRIGGER_CAP`（= 4），行为与重构前完全一致。
///
/// 未来接入升级源（境界 / 功法 / 法宝 passive）时，把扩展逻辑写到此函数，调用方无需改动。
/// 与 `worn_cap_bonus` 同步，升级源需 worldview 锚点（决议 #24）。
///
/// plan-layered-equip-v1 P5 — 升级源未接，本 PR 为占位扩展点。
#[allow(dead_code)]
pub fn treasure_trigger_cap() -> usize {
    // P5 占位：升级源未接，返回基础常量。接入升级源时删除 #[allow(dead_code)] 并实现派生逻辑。
    TREASURE_TRIGGER_CAP
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearScope {
    PackOnly,
    PackAndHotbar,
    All,
}

pub fn clear_player_inventory(inventory: &mut PlayerInventory, scope: ClearScope) {
    match scope {
        ClearScope::PackOnly => {
            if let Some(container) = inventory
                .containers
                .iter_mut()
                .find(|container| container.id == MAIN_PACK_CONTAINER_ID)
            {
                container.items.clear();
            }
        }
        ClearScope::PackAndHotbar => {
            // `clearinv all`: clear every carried container plus hotbar, but keep equipment.
            for container in &mut inventory.containers {
                container.items.clear();
            }
            inventory.hotbar = Default::default();
        }
        ClearScope::All => {
            for container in &mut inventory.containers {
                container.items.clear();
            }
            inventory.hotbar = Default::default();
            inventory.equipped.clear();
        }
    }
    bump_revision(inventory);
}

#[derive(Debug, Clone, Copy, Component, PartialEq)]
pub struct OverloadedMarker {
    pub current_weight: f64,
    pub max_weight: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InventoryGrantReceipt {
    pub revision: InventoryRevision,
    /// 兼容旧调用方：指向本次 grant 创建的第一个新实例；纯 merge 时为 0。
    pub instance_id: u64,
    pub template_id: String,
    pub stack_count: u32,
    pub created_instance_ids: Vec<u64>,
    pub merged_instance_ids: Vec<u64>,
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][inventory] registering inventory resources and join attach system");

    let item_registry = load_item_registry().unwrap_or_else(|error| {
        panic!("[bong][inventory] failed to load item registry: {error}");
    });

    let default_loadout = load_default_loadout(&item_registry).unwrap_or_else(|error| {
        panic!("[bong][inventory] failed to load default inventory loadout: {error}");
    });

    app.insert_resource(item_registry);
    app.insert_resource(DefaultLoadout(default_loadout));
    app.insert_resource(InventoryInstanceIdAllocator::default());
    app.insert_resource(DroppedLootRegistry::default());
    app.insert_resource(freshness::FreshnessEnvironment::default());
    app.insert_resource(spirit_treasure::SpiritTreasureRegistry::default());
    // plan-tsy-loot-v1 §2 — 上古遗物模板池 + 已 spawn family 集合。
    app.insert_resource(ancient_relics::AncientRelicPool::from_seed());
    app.insert_resource(tsy_loot_spawn::TsySpawnedFamilies::default());
    poi_loot::log_novice_poi_loot_tables();
    app.add_event::<DroppedItemEvent>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(
        Update,
        (
            apply_death_drop_on_revive,
            apply_termination_drop_on_terminate,
            handle_remains_interactions,
            freshness::freshness_tick_system,
            sync_overloaded_marker,
            spirit_treasure::sync_spirit_treasures,
            // plan-tsy-loot-v1 §2.2 — 玩家踏入 family 时 spawn 1% 上古遗物（idempotent）。
            tsy_loot_spawn::tsy_loot_spawn_on_enter,
        ),
    );
}

fn last_termination_cause(life_record: Option<&LifeRecord>) -> Option<&str> {
    match life_record.and_then(|record| record.biography.last()) {
        Some(BiographyEntry::Terminated { cause, .. }) => Some(cause.as_str()),
        _ => None,
    }
}

/// Worldview §十二：角色终结后，身上物品应全部留世，掉在死亡点供他人拾取。
///
/// 例外：plan-death-lifecycle-v1 §3「自主归隐」走善终路径，不掉物品。
#[allow(clippy::too_many_arguments)]
pub fn apply_termination_drop_on_terminate(
    mut terminated: bevy_ecs::event::EventReader<PlayerTerminated>,
    mut commands: Commands,
    life_records: Query<&LifeRecord>,
    mut inventories: Query<&mut PlayerInventory>,
    positions: Query<&Position>,
    anchors: Query<&DeathDropAnchor>,
    layer_ids: Query<&EntityLayerId>,
    dimensions: Query<&CurrentDimension>,
    mut dropped_registry: bevy_ecs::system::ResMut<DroppedLootRegistry>,
) {
    for ev in terminated.read() {
        let Ok(mut inventory) = inventories.get_mut(ev.entity) else {
            continue;
        };

        let cause = last_termination_cause(life_records.get(ev.entity).ok());
        let should_spawn_remains = cause == Some("natural_end");
        let should_drop_to_world = !should_spawn_remains && cause != Some("voluntary_retire");

        let base = anchors
            .get(ev.entity)
            .map(|anchor| anchor.pos)
            .or_else(|_| {
                positions.get(ev.entity).map(|pos| {
                    let p = pos.0;
                    [p.x, p.y, p.z]
                })
            })
            .unwrap_or([0.0, 64.0, 0.0]);
        let entity_dimension = dimensions
            .get(ev.entity)
            .map(|dimension| dimension.0)
            .unwrap_or_default();

        let mut drained = Vec::new();
        for container in &mut inventory.containers {
            let container_id = container.id.clone();
            for placed in container.items.drain(..) {
                drained.push((
                    container_id.clone(),
                    placed.row,
                    placed.col,
                    placed.instance,
                ));
            }
        }
        // plan-layered-equip-v1 P0.2（桶④）— drain 全件（worn 全层 + held），按槽 key 标记。
        for (slot, contents) in inventory.equipped.drain() {
            for item in contents.worn.into_iter().chain(contents.held) {
                drained.push((slot.clone(), 0, 0, item));
            }
        }
        for idx in 0..inventory.hotbar.len() {
            if let Some(item) = inventory.hotbar[idx].take() {
                drained.push(("hotbar".to_string(), 0, idx as u8, item));
            }
        }

        let drained_bone_coins = inventory.bone_coins;
        inventory.bone_coins = 0;

        if should_spawn_remains && (!drained.is_empty() || drained_bone_coins > 0) {
            let Ok(layer_id) = layer_ids.get(ev.entity) else {
                tracing::warn!(
                    "[bong][inventory] natural_end terminate entity={:?} missing EntityLayerId; falling back to world drops",
                    ev.entity
                );
                // Fall back to world drops if we can't place a remains entity.
                let start_idx = dropped_registry.entries.len();
                for (idx, (source_container_id, source_row, source_col, item)) in
                    drained.into_iter().enumerate()
                {
                    let entry = DroppedLootEntry {
                        instance_id: item.instance_id,
                        source_container_id,
                        source_row,
                        source_col,
                        world_pos: [
                            base[0] + 0.35 + (start_idx + idx) as f64 * 0.1,
                            base[1],
                            base[2] + 0.35,
                        ],
                        dimension: entity_dimension,
                        item,
                    };
                    dropped_registry.entries.insert(entry.instance_id, entry);
                }
                commands.entity(ev.entity).remove::<DeathDropAnchor>();
                bump_revision(&mut inventory);
                continue;
            };

            let (remains_entity, entry_entity) =
                spawn_player_remains_entity(&mut commands, layer_id.0, base);
            let items = drained
                .into_iter()
                .map(
                    |(source_container_id, source_row, source_col, item)| RemainsItemRecord {
                        source_container_id,
                        source_row,
                        source_col,
                        item,
                    },
                )
                .collect::<Vec<_>>();
            commands.entity(remains_entity).insert(RemainsContainer {
                items,
                bone_coins: drained_bone_coins,
                player_list_entry: entry_entity,
            });
        } else if should_drop_to_world && !drained.is_empty() {
            let start_idx = dropped_registry.entries.len();
            for (idx, (source_container_id, source_row, source_col, item)) in
                drained.into_iter().enumerate()
            {
                let entry = DroppedLootEntry {
                    instance_id: item.instance_id,
                    source_container_id,
                    source_row,
                    source_col,
                    world_pos: [
                        base[0] + 0.35 + (start_idx + idx) as f64 * 0.1,
                        base[1],
                        base[2] + 0.35,
                    ],
                    dimension: entity_dimension,
                    item,
                };
                dropped_registry.entries.insert(entry.instance_id, entry);
            }
        }

        commands.entity(ev.entity).remove::<DeathDropAnchor>();
        bump_revision(&mut inventory);
    }
}

fn spawn_player_remains_entity(
    commands: &mut Commands,
    layer: Entity,
    pos: [f64; 3],
) -> (Entity, Entity) {
    use valence::entity::entity::{CustomName, NameVisible, NoGravity, Pose as PoseComponent};
    use valence::entity::player::PlayerEntityBundle;
    use valence::player_list::{DisplayName, Listed, PlayerListEntryBundle};
    use valence::prelude::Text;

    let uuid = valence::prelude::UniqueId::default();
    let raw_hex = format!("{:032x}", uuid.0.as_u128());
    let suffix = &raw_hex[raw_hex.len().saturating_sub(8)..];
    let username = format!("Remains_{suffix}");

    let remains_entity = commands
        .spawn(PlayerEntityBundle {
            layer: EntityLayerId(layer),
            uuid,
            position: Position::new(pos),
            // Keep it in-place and visibly "dead".
            entity_no_gravity: NoGravity(true),
            entity_pose: PoseComponent(valence::entity::Pose::Dying),
            entity_custom_name: CustomName(Some(Text::text("Remains"))),
            entity_name_visible: NameVisible(true),
            ..Default::default()
        })
        .id();

    // In order for the player entity to be visible to other players, there must
    // be an entry in the player list.
    let entry_entity = commands
        .spawn(PlayerListEntryBundle {
            uuid,
            username: Username(username),
            display_name: DisplayName(Some(Text::text("Remains"))),
            listed: Listed(false),
            ..Default::default()
        })
        .id();

    (remains_entity, entry_entity)
}

pub fn handle_remains_interactions(
    mut interactions: bevy_ecs::event::EventReader<InteractEntityEvent>,
    mut commands: Commands,
    mut remains_q: Query<(Entity, &mut RemainsContainer, &Position, &EntityLayerId)>,
    mut inventories: Query<(&mut PlayerInventory, &Position, &EntityLayerId)>,
) {
    const PICKUP_RANGE_SQ: f64 = 2.5 * 2.5;

    for ev in interactions.read() {
        match ev.interact {
            EntityInteraction::Interact(Hand::Main)
            | EntityInteraction::InteractAt {
                hand: Hand::Main, ..
            } => {}
            _ => continue,
        }

        let Ok((remains_entity, mut remains, remains_pos, remains_layer)) =
            remains_q.get_mut(ev.entity)
        else {
            continue;
        };
        let Ok((mut inventory, player_pos, player_layer)) = inventories.get_mut(ev.client) else {
            continue;
        };
        if remains_layer.0 != player_layer.0 {
            continue;
        }

        let rp = remains_pos.get();
        let pp = player_pos.get();
        let dx = rp.x - pp.x;
        let dy = rp.y - pp.y;
        let dz = rp.z - pp.z;
        if dx * dx + dy * dy + dz * dz > PICKUP_RANGE_SQ {
            continue;
        }

        let mut moved_any = false;

        // Transfer wallet bone coins first (no slot requirements).
        if remains.bone_coins > 0 && inventory.bone_coins < JS_SAFE_INTEGER_MAX {
            let available = JS_SAFE_INTEGER_MAX.saturating_sub(inventory.bone_coins);
            let transfer = remains.bone_coins.min(available);
            if transfer > 0 {
                inventory.bone_coins = inventory.bone_coins.saturating_add(transfer);
                remains.bone_coins = remains.bone_coins.saturating_sub(transfer);
                moved_any = true;
            }
        }

        // Transfer item instances into the looter's containers.
        if !remains.items.is_empty() {
            let mut leftover = Vec::with_capacity(remains.items.len());
            for record in remains.items.drain(..) {
                let RemainsItemRecord {
                    source_container_id,
                    source_row,
                    source_col,
                    item,
                } = record;

                let Some(location) = find_first_fit_container_location(&inventory, &item) else {
                    leftover.push(RemainsItemRecord {
                        source_container_id,
                        source_row,
                        source_col,
                        item,
                    });
                    continue;
                };
                if let Err(reason) = attach_at_location(&mut inventory, item.clone(), &location) {
                    tracing::warn!("[bong][inventory] remains loot attach rejected: {reason}");
                    leftover.push(RemainsItemRecord {
                        source_container_id,
                        source_row,
                        source_col,
                        item,
                    });
                    continue;
                }
                moved_any = true;
            }
            remains.items = leftover;
        }

        if moved_any {
            bump_revision(&mut inventory);
        }

        if remains.items.is_empty() && remains.bone_coins == 0 {
            commands.entity(remains_entity).insert(Despawned);
            commands.entity(remains.player_list_entry).insert(Despawned);
        }
    }
}

pub(crate) fn attach_inventory_to_joined_clients(
    mut commands: Commands,
    mut allocator: valence::prelude::ResMut<InventoryInstanceIdAllocator>,
    default_loadout: valence::prelude::Res<DefaultLoadout>,
    item_registry: valence::prelude::Res<ItemRegistry>,
    joined_clients: Query<Entity, JoinedClientsWithoutInventoryFilter>,
) {
    for entity in &joined_clients {
        let player_inventory =
            instantiate_inventory_from_loadout(&default_loadout.0, &mut allocator, &item_registry)
                .unwrap_or_else(|error| {
                panic!(
                    "[bong][inventory] failed to instantiate default loadout for joined client {entity:?}: {error}"
                )
            });

        commands.entity(entity).insert(player_inventory);
        // plan-HUD-v1 §1.3 默认全解锁（v1 演示）。后续接入修炼系统按真实条件 mutate。
        commands
            .entity(entity)
            .insert(crate::combat::components::UnlockedStyles::default());
        // plan-skill-v1 §8 SkillSet 挂玩家 entity；consumed_scrolls 一生累积（死透重生由
        // plan-death-lifecycle §4/§5 新建 default 实例，不迁移）。
        commands
            .entity(entity)
            .insert(crate::skill::components::SkillSet::default());
        tracing::info!("[bong][inventory] attached PlayerInventory to joined client {entity:?}");
    }
}

pub fn instantiate_inventory_from_loadout(
    loadout: &LoadoutSpec,
    allocator: &mut InventoryInstanceIdAllocator,
    registry: &ItemRegistry,
) -> Result<PlayerInventory, String> {
    let mut containers = Vec::with_capacity(loadout.containers.len());
    for container in &loadout.containers {
        let mut placed_items = Vec::with_capacity(container.items.len());
        for item in &container.items {
            placed_items.push(PlacedItemState {
                row: item.row,
                col: item.col,
                instance: instantiate_item_instance(&item.instance, allocator)?,
            });
        }

        containers.push(ContainerState {
            id: container.id.clone(),
            name: container.name.clone(),
            rows: container.rows,
            cols: container.cols,
            items: placed_items,
            owner_instance_id: None,
        });
    }

    // plan-layered-equip-v1 P0.6 — 每槽 SlotContents 重建（worn 全件 + held），各自分配新 instance_id。
    let mut equipped = HashMap::with_capacity(loadout.equipped.len());
    // plan-tarkov-backpack-v1 P0（交付物 #5，衔接决议 #2）— 收集**所有**穿戴背包件 instance_id。
    // 旧版只取第一个；现遍历所有 worn container_spec 件，第一个复用静态占位容器 id 重映射，
    // 其余 worn pack 由收尾 `rebuild_containers_from_equipment` 动态新建 `pack_<id>` 容器
    // （不依赖 toml 预配多占位）。
    let mut worn_pack_instances: Vec<u64> = Vec::new();
    for (slot_id, contents) in &loadout.equipped {
        let mut worn = Vec::with_capacity(contents.worn.len());
        for item in &contents.worn {
            let instance = instantiate_item_instance(item, allocator)?;
            if registry
                .get(&instance.template_id)
                .is_some_and(|t| t.container_spec.is_some())
            {
                worn_pack_instances.push(instance.instance_id);
            }
            worn.push(instance);
        }
        let held = contents
            .held
            .as_ref()
            .map(|item| instantiate_item_instance(item, allocator))
            .transpose()?;
        equipped.insert(slot_id.clone(), SlotContents { worn, held });
    }
    // 确定性：按 instance_id 排序，使「第一个 worn pack」（复用占位容器、携带占位预置物品）
    // 在 HashMap 迭代顺序不定的情况下仍稳定（取最小 instance_id）。
    worn_pack_instances.sort_unstable();

    // plan-layered-equip-v1 P0.6（决议 #17 / #13.5）/ plan-tarkov-backpack-v1 P0（交付物 #5）—
    // 把静态占位背包容器 id 重映射到**第一个**运行时穿戴背包件的 `pack_<instance_id>`，
    // 并写 owner_instance_id；占位仅服务第一个 worn pack，其余 worn pack 走 rebuild 动态建容器。
    if let Some(&first_instance_id) = worn_pack_instances.first() {
        let runtime_id = container_id_for_worn_pack(first_instance_id);
        for container in containers.iter_mut() {
            if container.id == LOADOUT_PACK_PLACEHOLDER_CONTAINER_ID {
                container.id = runtime_id.clone();
                container.owner_instance_id = Some(first_instance_id);
            }
        }
    }

    let mut hotbar: [Option<ItemInstance>; 9] = Default::default();
    for (index, item) in loadout.hotbar.iter().enumerate() {
        hotbar[index] = item
            .as_ref()
            .map(|slot_item| instantiate_item_instance(slot_item, allocator))
            .transpose()?;
    }

    let mut inventory = PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers,
        equipped,
        hotbar,
        bone_coins: loadout.bone_coins,
        max_weight: loadout.max_weight,
    };

    // plan-tarkov-backpack-v1 P0（交付物 #5）— 任何 worn pack loadout 都收尾 rebuild：
    // - 只有占位容器时，第一个 worn pack 复用占位（保留预置物品）并回填 owner_instance_id；
    // - 没有占位容器时，单背包也必须动态新建 `pack_<id>` 容器；
    // - 多背包时为第 2+ 个 worn pack 动态建容器。
    //
    // 单背包既有 loadout 可能依赖显式 max_weight；仅多背包沿用 rebuild 的重算行为。
    if !worn_pack_instances.is_empty() {
        let configured_max_weight = inventory.max_weight;
        let _overflow = rebuild_containers_from_equipment(&mut inventory, registry);
        if worn_pack_instances.len() == 1 {
            inventory.max_weight = configured_max_weight;
        }
    }

    Ok(inventory)
}

fn instantiate_item_instance(
    template_instance: &ItemInstance,
    allocator: &mut InventoryInstanceIdAllocator,
) -> Result<ItemInstance, String> {
    Ok(ItemInstance {
        instance_id: allocator.next_id()?,
        template_id: template_instance.template_id.clone(),
        display_name: template_instance.display_name.clone(),
        grid_w: template_instance.grid_w,
        grid_h: template_instance.grid_h,
        weight: template_instance.weight,
        rarity: template_instance.rarity,
        description: template_instance.description.clone(),
        stack_count: template_instance.stack_count,
        spirit_quality: template_instance.spirit_quality,
        durability: template_instance.durability,
        freshness: None,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    })
}

/// plan-worldgen-v4 P5 §8.1#5 — vanilla 方块物品模板 ID 前缀。
///
/// 启动期为**每个** vanilla `BlockKind` 自动生成一个 `template_id = "vanilla:<block_id>"`
/// 的 `category = Block` 模板，让画廊 dev-only give-block 链路（BlockPickerGive）
/// 不必为每个 vanilla 方块手写 TOML。`block_id` 为 valence `BlockKind::to_str()` 短名
/// （不含 `minecraft:` namespace），与 `block_place::block_item_to_state` 的 `vanilla:`
/// 直通分支严格对齐。
pub const VANILLA_TEMPLATE_PREFIX: &str = "vanilla:";

/// 为单个 vanilla 方块短名构造最小化 `Block` 模板（dev-only give-block 用）。
fn vanilla_block_template(block_id: &str) -> ItemTemplate {
    ItemTemplate {
        id: format!("{VANILLA_TEMPLATE_PREFIX}{block_id}"),
        display_name: block_id.to_string(),
        category: ItemCategory::Block,
        // placeable=None：放置走 block_place::block_item_to_state 的 vanilla: 直通分支，
        // 不经 PlaceableBlockKind（那是 Bong custom 方块的语义）。
        placeable: None,
        max_stack_count: default_max_stack_count_for_category(ItemCategory::Block),
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.1,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 0.0,
        description: format!("vanilla {block_id}（dev-only 画廊方块）"),
        effect: None,
        cast_duration_ms: DEFAULT_CAST_DURATION_MS,
        cooldown_ms: DEFAULT_COOLDOWN_MS,
        weapon_spec: None,
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: None,
        shield_spec: None,
        shelflife_profile: None,
        shelflife_track: None,
    }
}

/// 为全部 vanilla `BlockKind` 注入 `vanilla:<block_id>` 模板。
///
/// `air` 跳过（无可给予物品）。若某个 `vanilla:<id>` 与手工 TOML 撞 key，返回 Err —
/// 保护手写映射不被静默覆盖（与 TOML 重复检测同语义）。
fn inject_vanilla_block_templates(
    templates: &mut HashMap<String, ItemTemplate>,
) -> Result<usize, String> {
    use valence::prelude::BlockKind;
    let mut injected = 0usize;
    for kind in BlockKind::ALL {
        let block_id = kind.to_str();
        if block_id == "air" {
            continue;
        }
        let template = vanilla_block_template(block_id);
        let template_id = template.id.clone();
        if templates.insert(template_id.clone(), template).is_some() {
            return Err(format!(
                "vanilla block template `{template_id}` collides with a hand-authored item template; \
                 rename the TOML item or remove the conflicting vanilla mapping"
            ));
        }
        injected += 1;
    }
    Ok(injected)
}

pub fn load_item_registry() -> Result<ItemRegistry, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_ITEMS_DIR);
    load_item_registry_from_dir(path)
}

fn load_item_registry_from_dir(path: impl AsRef<Path>) -> Result<ItemRegistry, String> {
    let path = path.as_ref();
    let mut toml_paths = Vec::new();
    collect_item_toml_paths(path, &mut toml_paths)?;
    toml_paths.sort();

    if toml_paths.is_empty() {
        return Err(format!(
            "inventory item registry directory {} contains no *.toml files",
            path.display()
        ));
    }

    let mut templates = HashMap::new();

    for toml_path in toml_paths {
        let content = fs::read_to_string(&toml_path)
            .map_err(|error| format!("failed to read {}: {error}", toml_path.display()))?;
        let parsed: ItemTemplatesToml = toml::from_str(&content).map_err(|error| {
            format!(
                "failed to parse {} as item template TOML: {error}",
                toml_path.display()
            )
        })?;

        for raw in parsed.item {
            let template = raw.try_into_item_template(&toml_path)?;
            let template_id = template.id.clone();

            if templates.insert(template_id.clone(), template).is_some() {
                return Err(format!(
                    "duplicate item template id `{template_id}` found while loading {}",
                    toml_path.display()
                ));
            }
        }
    }

    // plan-worldgen-v4 P5 §8.1#5 — 在手工 TOML 全部加载后注入 vanilla:<block_id> 模板，
    // 供画廊 dev-only give-block（BlockPickerGive）链路使用。撞 key 即 Err 保护手写映射。
    //
    // 为何无条件注入而非 feature-gate / dev-flag（CodeRabbit 关注点的裁决）：
    // 1. 注入是**纯数据**——只往 ItemRegistry 加 ~760 个 `vanilla:<id>` / category=Block 的
    //    最小模板，不触发任何 gameplay 副作用、不进 qi_physics ledger、不改自然修炼规则。
    // 2. 唯一取用面是 dev give-block handler（cmd::dev::block_picker），它在**运行时**用
    //    GameMode::Creative 门控——生产玩家默认 Survival，每次请求都被拒，比"启动标志"
    //    更强（标志可配错，gamemode 门控逐请求生效）。block_place 的 `vanilla:` 直通也只
    //    对已持有 vanilla:<id> instance 的玩家生效，而该 instance 仅能经 Creative give-block 取得。
    // 3. server 是单一 offline-mode binary，没有干净的 dev/creative 启动开关；引入 feature
    //    flag 会把 load_item_registry 劈成生产/dev 两条路径，破坏现有统一加载链与全部既有测试。
    // 故选「纯数据 + Creative-gated 唯一消费者」这条安全且不破坏现有路径的方案。
    inject_vanilla_block_templates(&mut templates)?;

    Ok(ItemRegistry { templates })
}

fn collect_item_toml_paths(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(path).map_err(|error| {
        format!(
            "failed to read inventory item registry directory {}: {error}",
            path.display()
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read inventory item registry entry under {}: {error}",
                path.display()
            )
        })?;
        let file_path = entry.path();
        if file_path.is_dir() {
            collect_item_toml_paths(&file_path, out)?;
            continue;
        }
        let is_toml = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"));
        if is_toml {
            out.push(file_path);
        }
    }

    Ok(())
}

pub fn load_default_loadout(registry: &ItemRegistry) -> Result<LoadoutSpec, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_LOADOUT_PATH);
    load_default_loadout_from_path(path, registry)
}

fn load_default_loadout_from_path(
    path: impl AsRef<Path>,
    registry: &ItemRegistry,
) -> Result<LoadoutSpec, String> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read inventory loadout {}: {error}",
            path.display()
        )
    })?;
    let raw: LoadoutToml = toml::from_str(&content).map_err(|error| {
        format!(
            "failed to parse inventory loadout TOML {}: {error}",
            path.display()
        )
    })?;

    raw.try_into_loadout(path, registry)
}

impl ItemRegistry {
    pub fn get(&self, template_id: &str) -> Option<&ItemTemplate> {
        self.templates.get(template_id)
    }

    /// 测试用:从手动构造的 templates map 建 registry。
    ///
    /// plan-tarkov-backpack-v1 P0: 去掉 `#[cfg(test)]` 门控，使 `server/tests/` 集成 e2e
    /// （外部 crate，看不到 cfg(test) 项）也能构造 registry。`#[doc(hidden)]` 标明仅供测试装配，
    /// 非生产 API（生产经 `load_item_registry` 从 toml 加载）。
    #[doc(hidden)]
    pub fn from_map(templates: HashMap<String, ItemTemplate>) -> Self {
        Self { templates }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.templates.len()
    }
}

pub fn add_item_to_player_inventory(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
    template_id: &str,
    stack_count: u32,
    current_tick: u64,
) -> Result<InventoryGrantReceipt, String> {
    add_item_to_player_inventory_inner(
        inventory,
        registry,
        allocator,
        template_id,
        stack_count,
        true,
        None,
        current_tick,
    )
}

pub fn add_customized_item_to_player_inventory(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
    template_id: &str,
    stack_count: u32,
    current_tick: u64,
    customize_instance: impl Fn(&mut ItemInstance),
) -> Result<InventoryGrantReceipt, String> {
    add_item_to_player_inventory_inner(
        inventory,
        registry,
        allocator,
        template_id,
        stack_count,
        true,
        Some(&customize_instance),
        current_tick,
    )
}

pub fn add_item_to_player_inventory_with_alchemy(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
    template_id: &str,
    stack_count: u32,
    alchemy: Option<AlchemyItemData>,
    current_tick: u64,
) -> Result<InventoryGrantReceipt, String> {
    add_item_to_player_inventory_inner(
        inventory,
        registry,
        allocator,
        template_id,
        stack_count,
        true,
        Some(&|instance| {
            instance.alchemy = alchemy.clone();
        }),
        current_tick,
    )
}

#[allow(clippy::too_many_arguments)]
fn add_item_to_player_inventory_inner(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
    template_id: &str,
    stack_count: u32,
    merge_existing_stacks: bool,
    customize_instance: Option<&dyn Fn(&mut ItemInstance)>,
    current_tick: u64,
) -> Result<InventoryGrantReceipt, String> {
    if stack_count == 0 {
        return Err("add_item_to_player_inventory requires stack_count >= 1".to_string());
    }

    let template = registry
        .get(template_id)
        .ok_or_else(|| format!("unknown item template id `{template_id}`"))?;

    let candidate_indices = carried_container_candidate_indices(inventory);
    if candidate_indices.is_empty() {
        return Err("player inventory has no containers".to_string());
    }

    let max_stack_count = template.max_stack_count.max(1);
    let mut merge_probe = runtime_instance_from_template(template, 0, 1, current_tick);
    if let Some(customize_instance) = customize_instance {
        customize_instance(&mut merge_probe);
    }
    let mut selected_index = None;
    let mut new_stacks = Vec::new();
    'candidate: for candidate_index in candidate_indices {
        if !container_accepts_runtime_grant(
            inventory,
            registry,
            &inventory.containers[candidate_index],
            &merge_probe,
        ) {
            continue;
        }
        let mut remaining = stack_count;
        let mut staged = inventory.containers[candidate_index].clone();

        if merge_existing_stacks && max_stack_count > 1 {
            for placed in staged.items.iter_mut().filter(|placed| {
                placed.instance.template_id == template.id
                    && stack_identity_matches(&placed.instance, &merge_probe)
            }) {
                let available = max_stack_count.saturating_sub(placed.instance.stack_count);
                let merged = remaining.min(available);
                placed.instance.stack_count = placed.instance.stack_count.saturating_add(merged);
                remaining -= merged;
                if remaining == 0 {
                    break;
                }
            }
        }

        let mut staged_new_stacks = Vec::new();
        while remaining > 0 {
            let new_stack_count = remaining.min(max_stack_count);
            let Some((row, col)) = find_free_slot(&staged, template.grid_w, template.grid_h) else {
                continue 'candidate;
            };
            let mut staged_instance =
                runtime_instance_from_template(template, 0, new_stack_count, current_tick);
            if let Some(customize_instance) = customize_instance {
                customize_instance(&mut staged_instance);
            }
            staged.items.push(PlacedItemState {
                row,
                col,
                instance: staged_instance,
            });
            staged_new_stacks.push((row, col, new_stack_count));
            remaining -= new_stack_count;
        }
        selected_index = Some(candidate_index);
        new_stacks = staged_new_stacks;
        break;
    }
    let Some(chosen_index) = selected_index else {
        return Err(format!("inventory full: {template_id}"));
    };

    let mut new_instance_ids = Vec::with_capacity(new_stacks.len());
    for _ in 0..new_stacks.len() {
        new_instance_ids.push(allocator.next_id()?);
    }

    let target_container = &mut inventory.containers[chosen_index];
    let mut merged_instance_ids = Vec::new();
    let mut remaining = stack_count;
    if merge_existing_stacks && max_stack_count > 1 {
        for placed in target_container.items.iter_mut().filter(|placed| {
            placed.instance.template_id == template.id
                && stack_identity_matches(&placed.instance, &merge_probe)
        }) {
            let available = max_stack_count.saturating_sub(placed.instance.stack_count);
            let merged = remaining.min(available);
            if merged > 0 {
                if !merged_instance_ids.contains(&placed.instance.instance_id) {
                    merged_instance_ids.push(placed.instance.instance_id);
                }
                placed.instance.stack_count = placed.instance.stack_count.saturating_add(merged);
                remaining -= merged;
            }
            if remaining == 0 {
                break;
            }
        }
    }

    let mut created_instance_ids = Vec::new();
    for ((row, col, new_stack_count), instance_id) in
        new_stacks.into_iter().zip(new_instance_ids.into_iter())
    {
        created_instance_ids.push(instance_id);
        let mut instance =
            runtime_instance_from_template(template, instance_id, new_stack_count, current_tick);
        if let Some(customize_instance) = customize_instance {
            customize_instance(&mut instance);
        }
        target_container
            .items
            .push(PlacedItemState { row, col, instance });
    }

    inventory.revision.0 = inventory.revision.0.saturating_add(1);

    Ok(InventoryGrantReceipt {
        revision: inventory.revision,
        instance_id: created_instance_ids.first().copied().unwrap_or(0),
        template_id: template.id.clone(),
        stack_count,
        created_instance_ids,
        merged_instance_ids,
    })
}

/// 放置已经实例化的 loot，保留调用方生成的 instance_id / 特殊字段。
pub fn add_existing_item_to_player_inventory(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    item: ItemInstance,
) -> Result<InventoryRevision, String> {
    let candidate_indices = carried_container_candidate_indices(inventory);
    if candidate_indices.is_empty() {
        return Err("player inventory has no containers".to_string());
    }

    let template_id = item.template_id.clone();
    for candidate_index in candidate_indices {
        if !container_accepts_runtime_grant(
            inventory,
            registry,
            &inventory.containers[candidate_index],
            &item,
        ) {
            continue;
        }
        let Some((row, col)) = find_free_slot(
            &inventory.containers[candidate_index],
            item.grid_w,
            item.grid_h,
        ) else {
            continue;
        };
        inventory.containers[candidate_index]
            .items
            .push(PlacedItemState {
                row,
                col,
                instance: item,
            });
        inventory.revision.0 = inventory.revision.0.saturating_add(1);
        return Ok(inventory.revision);
    }

    Err(format!("inventory full: {template_id}"))
}

fn carried_container_candidate_indices(inventory: &PlayerInventory) -> Vec<usize> {
    // 优先尝试非 body_pocket 容器（pack_<id> / main_pack 等），都放不下再兜底 body_pocket。
    let mut candidate_indices: Vec<usize> = inventory
        .containers
        .iter()
        .enumerate()
        .filter_map(|(index, container)| {
            (container.id != BODY_POCKET_CONTAINER_ID).then_some(index)
        })
        .collect();
    candidate_indices.extend(inventory.containers.iter().enumerate().filter_map(
        |(index, container)| (container.id == BODY_POCKET_CONTAINER_ID).then_some(index),
    ));
    candidate_indices
}

fn container_accepts_runtime_grant(
    inventory: &PlayerInventory,
    registry: &ItemRegistry,
    container: &ContainerState,
    item: &ItemInstance,
) -> bool {
    let Some(owner_instance_id) = container.owner_instance_id else {
        return true;
    };

    worn_container_items(inventory, registry)
        .find(|(owner, _)| owner.instance_id == owner_instance_id)
        .is_some_and(|(_, spec)| item_passes_filter(&spec.accept_filter, item, registry))
}

pub fn find_free_slot(container: &ContainerState, grid_w: u8, grid_h: u8) -> Option<(u8, u8)> {
    if grid_w == 0 || grid_h == 0 || grid_w > container.cols || grid_h > container.rows {
        return None;
    }

    for row in 0..=container.rows - grid_h {
        for col in 0..=container.cols - grid_w {
            let candidate = footprint_probe(row, col, grid_w, grid_h);
            if !container
                .items
                .iter()
                .any(|existing| placed_item_footprints_overlap(&candidate, existing))
            {
                return Some((row, col));
            }
        }
    }

    None
}

pub(crate) fn item_fits_in_container_bounds(
    container: &ContainerState,
    row: u8,
    col: u8,
    grid_w: u8,
    grid_h: u8,
) -> bool {
    u16::from(row) + u16::from(grid_h) <= u16::from(container.rows)
        && u16::from(col) + u16::from(grid_w) <= u16::from(container.cols)
}

pub fn find_mergeable_stack<'a>(
    container: &'a mut ContainerState,
    template_id: &str,
    max_stack_count: u32,
) -> Option<&'a mut PlacedItemState> {
    if max_stack_count <= 1 {
        return None;
    }

    container.items.iter_mut().find(|placed| {
        placed.instance.template_id == template_id && placed.instance.stack_count < max_stack_count
    })
}

fn runtime_instance_from_template(
    template: &ItemTemplate,
    instance_id: u64,
    stack_count: u32,
    current_tick: u64,
) -> ItemInstance {
    // plan-food-v1 P1/MAJOR2：当 template 声明了 shelflife_profile + track 时，
    // 自动挂 Freshness；created_at_tick 取 current_tick，避免服务器运行一段时间后
    // 发出的食物 elapsed=T 立刻被当已陈化（raw_dt=now_tick-0 >> 实际存放时间）。
    let freshness = match (&template.shelflife_profile, &template.shelflife_track) {
        (Some(profile_id), Some(track)) => Some(crate::shelflife::Freshness {
            created_at_tick: current_tick,
            initial_qi: template.spirit_quality_initial as f32,
            track: *track,
            profile: crate::shelflife::DecayProfileId::new(profile_id),
            frozen_accumulated: 0,
            frozen_since_tick: None,
        }),
        _ => None,
    };

    ItemInstance {
        instance_id,
        template_id: template.id.clone(),
        display_name: template.display_name.clone(),
        grid_w: template.grid_w,
        grid_h: template.grid_h,
        weight: template.base_weight,
        rarity: template.rarity,
        description: template.description.clone(),
        stack_count,
        spirit_quality: template.spirit_quality_initial,
        durability: 1.0,
        freshness,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    }
}

fn stack_identity_matches(left: &ItemInstance, right: &ItemInstance) -> bool {
    left.template_id == right.template_id
        && left.display_name == right.display_name
        && left.grid_w == right.grid_w
        && left.grid_h == right.grid_h
        && f64_values_match(left.weight, right.weight)
        && left.rarity == right.rarity
        && left.description == right.description
        && f64_values_match(left.spirit_quality, right.spirit_quality)
        && f64_values_match(left.durability, right.durability)
        && left.freshness == right.freshness
        && left.mineral_id == right.mineral_id
        && left.charges == right.charges
        && left.forge_quality == right.forge_quality
        && left.forge_color == right.forge_color
        && left.forge_side_effects == right.forge_side_effects
        && left.forge_achieved_tier == right.forge_achieved_tier
        && left.alchemy == right.alchemy
        && left.lingering_owner_qi == right.lingering_owner_qi
}

fn f64_values_match(left: f64, right: f64) -> bool {
    (left - right).abs() <= f64::EPSILON
}

pub(crate) fn footprint_probe(row: u8, col: u8, grid_w: u8, grid_h: u8) -> PlacedItemState {
    PlacedItemState {
        row,
        col,
        instance: ItemInstance {
            instance_id: 0,
            template_id: String::new(),
            display_name: String::new(),
            grid_w,
            grid_h,
            weight: 0.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        },
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemTemplatesToml {
    item: Vec<ItemTemplateToml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemTemplateToml {
    id: String,
    name: String,
    category: String,
    #[serde(default)]
    placeable: Option<String>,
    grid_w: u8,
    grid_h: u8,
    base_weight: f64,
    rarity: String,
    spirit_quality_initial: f64,
    description: String,
    #[serde(default)]
    max_stack_count: Option<u32>,
    effect: Option<ItemEffectToml>,
    /// 缺省 → DEFAULT_CAST_DURATION_MS。
    #[serde(default)]
    cast_duration_ms: Option<u32>,
    /// 缺省 → DEFAULT_COOLDOWN_MS。
    #[serde(default)]
    cooldown_ms: Option<u32>,
    /// plan-weapon-v1 §1.1：category == "Weapon" 时必填，否则须缺省。
    #[serde(default)]
    weapon: Option<WeaponSpecToml>,
    #[serde(default)]
    forge_station: Option<ForgeStationSpecToml>,
    #[serde(default)]
    blueprint_scroll: Option<BlueprintScrollSpecToml>,
    #[serde(default)]
    inscription_scroll: Option<InscriptionScrollSpecToml>,
    #[serde(default)]
    technique_scroll: Option<TechniqueScrollSpecToml>,
    /// plan-onboarding-loop-v1 P1.1：category == "recipe_fragment" 时可填。
    #[serde(default)]
    recipe_fragment: Option<RecipeFragmentSpecToml>,
    /// plan-backpack-equip-v1 P0：category == "Container" 时必填，否则须缺省。
    #[serde(default)]
    container: Option<ContainerSpecToml>,
    /// plan-shield-block-v1 P2：category == "Shield" 时必填，否则须缺省。
    #[serde(default)]
    shield_spec: Option<ShieldSpecToml>,
    /// plan-food-v1 P1：食物类物品的默认 shelflife profile ID。
    /// Some(id) → 物品生成时自动挂 `Freshness`；None → 无 shelflife。
    #[serde(default)]
    shelflife_profile: Option<String>,
    /// plan-food-v1 P1：shelflife 路径 — "decay" / "spoil" / "age"；缺省 "spoil"。
    /// 仅当 shelflife_profile 非 None 时有效。
    #[serde(default)]
    shelflife_track: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WeaponSpecToml {
    /// `sword` / `saber` / `staff` / `fist` / `spear` / `dagger` / `bow`。
    kind: String,
    base_attack: f32,
    quality_tier: u8,
    durability_max: f32,
    #[serde(default = "default_qi_cost_mul")]
    qi_cost_mul: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeStationSpecToml {
    tier: u8,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintScrollSpecToml {
    blueprint_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InscriptionScrollSpecToml {
    inscription_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TechniqueScrollSpecToml {
    #[serde(default = "default_combat_technique_scroll_kind")]
    kind: String,
    skill_id: String,
}

/// plan-onboarding-loop-v1 P1.1 — TOML 层的丹方 fragment 规格块（对应 `[item.recipe_fragment]`）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeFragmentSpecToml {
    recipe_id: String,
    known_stages: Vec<u8>,
    max_quality_tier: u8,
}

/// plan-backpack-equip-v1 P0 — TOML 层的容器规格块（对应 `[item.container]`）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerSpecToml {
    rows: u8,
    cols: u8,
    weight_capacity: f64,
    equip_slot: String,
    #[serde(default)]
    durability_cost_per_op: f64,
    #[serde(default)]
    attrition_exempt: bool,
    #[serde(default)]
    accept: Option<Vec<String>>,
}

/// plan-shield-block-v1 P2 — TOML 层的盾牌规格块（对应 `[item.shield_spec]`）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShieldSpecToml {
    block_ratio: f64,
    durability_max: f64,
    stamina_drain_per_s: f32,
}

fn default_qi_cost_mul() -> f32 {
    1.0
}

fn default_combat_technique_scroll_kind() -> String {
    "combat_technique".to_string()
}

/// plan-backpack-equip-v1 P0 — 解析 TOML 容器规格块为 `ContainerSpec`。
pub fn parse_container_spec(
    raw: ContainerSpecToml,
    source_path: &Path,
    item_id: &str,
) -> Result<ContainerSpec, String> {
    if !(1..=16).contains(&raw.rows) {
        return Err(format!(
            "{} item `{item_id}` has invalid container.rows {}; expected 1..=16",
            source_path.display(),
            raw.rows
        ));
    }
    if !(1..=16).contains(&raw.cols) {
        return Err(format!(
            "{} item `{item_id}` has invalid container.cols {}; expected 1..=16",
            source_path.display(),
            raw.cols
        ));
    }
    if !raw.weight_capacity.is_finite() || raw.weight_capacity < 0.0 {
        return Err(format!(
            "{} item `{item_id}` has invalid container.weight_capacity {}; expected finite >= 0",
            source_path.display(),
            raw.weight_capacity
        ));
    }
    // plan-layered-equip-v1 P0.1（决议 #17）— 背包 ContainerSpec.equip_slot 指向身体槽。
    let valid_slots = [
        EQUIP_SLOT_HEAD,
        EQUIP_SLOT_CHEST,
        EQUIP_SLOT_LEGS,
        EQUIP_SLOT_FEET,
    ];
    if !valid_slots.contains(&raw.equip_slot.as_str()) {
        return Err(format!(
            "{} item `{item_id}` has invalid container.equip_slot `{}`; expected one of [{}, {}, {}, {}]",
            source_path.display(),
            raw.equip_slot,
            EQUIP_SLOT_HEAD,
            EQUIP_SLOT_CHEST,
            EQUIP_SLOT_LEGS,
            EQUIP_SLOT_FEET
        ));
    }
    if !raw.durability_cost_per_op.is_finite() || raw.durability_cost_per_op < 0.0 {
        return Err(format!(
            "{} item `{item_id}` has invalid container.durability_cost_per_op {}; expected finite >= 0",
            source_path.display(),
            raw.durability_cost_per_op
        ));
    }
    let accept_filter = raw
        .accept
        .map(|accept| {
            accept
                .into_iter()
                .map(|raw_filter| parse_container_accept_filter(&raw_filter, source_path, item_id))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    Ok(ContainerSpec {
        rows: raw.rows,
        cols: raw.cols,
        weight_capacity: raw.weight_capacity,
        equip_slot: raw.equip_slot,
        durability_cost_per_op: raw.durability_cost_per_op,
        attrition_exempt: raw.attrition_exempt,
        accept_filter,
    })
}

fn parse_container_accept_filter(
    raw: &str,
    source_path: &Path,
    item_id: &str,
) -> Result<ContainerAcceptFilter, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!(
            "{} item `{item_id}` has empty container.accept entry",
            source_path.display()
        ));
    }
    if let Some(prefix) = trimmed.strip_prefix("prefix:") {
        let prefix = prefix.trim();
        if prefix.is_empty() {
            return Err(format!(
                "{} item `{item_id}` has empty container.accept prefix",
                source_path.display()
            ));
        }
        return Ok(ContainerAcceptFilter::TemplatePrefix(prefix.to_string()));
    }
    parse_item_category(trimmed, source_path, item_id).map(ContainerAcceptFilter::Category)
}

/// plan-shield-block-v1 P2 — 解析 TOML 盾牌规格块为 `ShieldSpec`。
pub fn parse_shield_spec(
    raw: ShieldSpecToml,
    source_path: &Path,
    item_id: &str,
) -> Result<ShieldSpec, String> {
    let spec = ShieldSpec {
        block_ratio: raw.block_ratio,
        durability_max: raw.durability_max,
        stamina_drain_per_s: raw.stamina_drain_per_s,
    };
    spec.validate(item_id)
        .map_err(|e| format!("{} {}", source_path.display(), e))?;
    Ok(spec)
}

fn default_max_stack_count_for_category(category: ItemCategory) -> u32 {
    match category {
        ItemCategory::Herb | ItemCategory::Block | ItemCategory::Mineral => 64,
        ItemCategory::BoneCoin => u32::MAX,
        ItemCategory::Anqi => 32,
        ItemCategory::Pill | ItemCategory::Misc | ItemCategory::Food | ItemCategory::Liquid => 16,
        ItemCategory::Armor
        | ItemCategory::Weapon
        | ItemCategory::Tool
        | ItemCategory::Treasure
        | ItemCategory::RecipeFragment
        | ItemCategory::RecipeHint
        | ItemCategory::Scroll
        | ItemCategory::Container
        // plan-shield-block-v1 P0 — 盾牌不可叠加，与武器/防具同为 1。
        | ItemCategory::Shield => 1,
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemEffectToml {
    kind: String,
    magnitude: f64,
    target: Option<String>,
    /// plan-food-v1 P2 — food_regen 专用：效果持续 tick 数（其他 effect 忽略）。
    #[serde(default)]
    duration_ticks: Option<u64>,
}

impl ItemTemplateToml {
    fn try_into_item_template(self, source_path: &Path) -> Result<ItemTemplate, String> {
        let id = required_non_empty(self.id, source_path, "item.id")?;
        let display_name = required_non_empty(self.name, source_path, "item.name")?;
        let description = required_non_empty(self.description, source_path, "item.description")?;

        if !(1..=4).contains(&self.grid_w) {
            return Err(format!(
                "{} item `{id}` has invalid grid_w {}; expected 1..=4",
                source_path.display(),
                self.grid_w
            ));
        }
        if !(1..=4).contains(&self.grid_h) {
            return Err(format!(
                "{} item `{id}` has invalid grid_h {}; expected 1..=4",
                source_path.display(),
                self.grid_h
            ));
        }
        if !self.base_weight.is_finite() || self.base_weight < 0.0 {
            return Err(format!(
                "{} item `{id}` has invalid base_weight {}; expected finite >= 0",
                source_path.display(),
                self.base_weight
            ));
        }
        if !(0.0..=1.0).contains(&self.spirit_quality_initial) {
            return Err(format!(
                "{} item `{id}` has invalid spirit_quality_initial {}; expected 0..=1",
                source_path.display(),
                self.spirit_quality_initial
            ));
        }

        let category = parse_item_category(self.category.as_str(), source_path, id.as_str())?;
        let placeable = self
            .placeable
            .map(|raw| {
                required_non_empty(raw, source_path, &format!("item `{id}` placeable"))
                    .map(|value| value.trim().to_ascii_lowercase())
            })
            .transpose()?;
        let rarity = parse_item_rarity(self.rarity.as_str(), source_path, id.as_str())?;
        let max_stack_count = self
            .max_stack_count
            .unwrap_or_else(|| default_max_stack_count_for_category(category));
        if max_stack_count == 0 {
            return Err(format!(
                "{} item `{id}` has invalid max_stack_count 0; expected >= 1",
                source_path.display()
            ));
        }
        let effect = self
            .effect
            .map(|raw| parse_item_effect(raw, source_path, id.as_str()))
            .transpose()?;

        // plan-weapon-v1 §1.1：weapon 块与 category=Weapon 必须一致。
        let weapon_spec = match (&category, self.weapon) {
            (ItemCategory::Weapon, Some(raw)) => {
                Some(parse_weapon_spec(raw, source_path, id.as_str())?)
            }
            (ItemCategory::Weapon, None) => {
                return Err(format!(
                    "{} item `{id}` has category=Weapon but missing [item.weapon] block",
                    source_path.display()
                ));
            }
            (_, Some(_)) => {
                return Err(format!(
                    "{} item `{id}` has [item.weapon] block but category != Weapon",
                    source_path.display()
                ));
            }
            (_, None) => None,
        };
        let forge_station_spec = self
            .forge_station
            .map(|raw| parse_forge_station_spec(raw, source_path, id.as_str()))
            .transpose()?;
        let blueprint_scroll_spec = self
            .blueprint_scroll
            .map(|raw| parse_blueprint_scroll_spec(raw, source_path, id.as_str()))
            .transpose()?;
        let inscription_scroll_spec = self
            .inscription_scroll
            .map(|raw| parse_inscription_scroll_spec(raw, source_path, id.as_str()))
            .transpose()?;
        let technique_scroll_spec = self
            .technique_scroll
            .map(|raw| parse_technique_scroll_spec(raw, source_path, id.as_str()))
            .transpose()?;
        let recipe_fragment_spec = match (&category, self.recipe_fragment) {
            (ItemCategory::RecipeFragment, Some(raw)) => {
                Some(parse_recipe_fragment_spec(raw, source_path, id.as_str())?)
            }
            (ItemCategory::RecipeFragment, None) => None,
            (_, Some(_)) => {
                return Err(format!(
                    "{} item `{id}` has [item.recipe_fragment] block but category != RecipeFragment",
                    source_path.display()
                ));
            }
            (_, None) => None,
        };

        // plan-backpack-equip-v1 P0：container 块与 category=Container 必须一致。
        let container_spec = match (&category, self.container) {
            (ItemCategory::Container, Some(raw)) => {
                Some(parse_container_spec(raw, source_path, id.as_str())?)
            }
            (ItemCategory::Container, None) => {
                return Err(format!(
                    "{} item `{id}` has category=Container but missing [item.container] block",
                    source_path.display()
                ));
            }
            (_, Some(_)) => {
                return Err(format!(
                    "{} item `{id}` has [item.container] block but category != Container",
                    source_path.display()
                ));
            }
            (_, None) => None,
        };

        // plan-shield-block-v1 P2：shield_spec 块与 category=Shield 必须一致。
        let shield_spec = match (&category, self.shield_spec) {
            (ItemCategory::Shield, Some(raw)) => {
                Some(parse_shield_spec(raw, source_path, id.as_str())?)
            }
            (ItemCategory::Shield, None) => {
                return Err(format!(
                    "{} item `{id}` has category=Shield but missing [item.shield_spec] block",
                    source_path.display()
                ));
            }
            (_, Some(_)) => {
                return Err(format!(
                    "{} item `{id}` has [item.shield_spec] block but category != Shield",
                    source_path.display()
                ));
            }
            (_, None) => None,
        };

        // plan-food-v1 P1：解析 shelflife_track 字符串 → DecayTrack。
        // CodeRabbit fix：shelflife_track 只写而 shelflife_profile=None 时报错，
        // 防止半配置静默绕过 freshness gate。
        if self.shelflife_profile.is_none() && self.shelflife_track.is_some() {
            return Err(format!(
                "{} item `{id}` has shelflife_track set but missing shelflife_profile; both must be specified together",
                source_path.display()
            ));
        }
        let shelflife_track = match self.shelflife_profile.as_deref() {
            Some(_) => {
                // shelflife_profile 存在时必须有合法 track（默认 spoil）。
                let raw_track = self.shelflife_track.as_deref().unwrap_or("spoil");
                match raw_track.trim().to_lowercase().as_str() {
                    "decay" => Some(crate::shelflife::DecayTrack::Decay),
                    "spoil" => Some(crate::shelflife::DecayTrack::Spoil),
                    "age" => Some(crate::shelflife::DecayTrack::Age),
                    other => {
                        return Err(format!(
                            "{} item `{id}` has invalid shelflife_track `{other}`; expected decay/spoil/age",
                            source_path.display()
                        ));
                    }
                }
            }
            None => None,
        };

        Ok(ItemTemplate {
            id,
            display_name,
            category,
            placeable,
            max_stack_count,
            grid_w: self.grid_w,
            grid_h: self.grid_h,
            base_weight: self.base_weight,
            rarity,
            spirit_quality_initial: self.spirit_quality_initial,
            description,
            effect,
            cast_duration_ms: self.cast_duration_ms.unwrap_or(DEFAULT_CAST_DURATION_MS),
            cooldown_ms: self.cooldown_ms.unwrap_or(DEFAULT_COOLDOWN_MS),
            weapon_spec,
            forge_station_spec,
            blueprint_scroll_spec,
            inscription_scroll_spec,
            technique_scroll_spec,
            recipe_fragment_spec,
            container_spec,
            shield_spec,
            shelflife_profile: self.shelflife_profile,
            shelflife_track,
        })
    }
}

pub fn parse_forge_station_spec(
    raw: ForgeStationSpecToml,
    source_path: &Path,
    item_id: &str,
) -> Result<ForgeStationSpec, String> {
    if !(1..=4).contains(&raw.tier) {
        return Err(format!(
            "{} item `{item_id}` has invalid forge_station.tier {}; expected 1..=4",
            source_path.display(),
            raw.tier
        ));
    }
    Ok(ForgeStationSpec { tier: raw.tier })
}

pub fn parse_blueprint_scroll_spec(
    raw: BlueprintScrollSpecToml,
    source_path: &Path,
    item_id: &str,
) -> Result<BlueprintScrollSpec, String> {
    let blueprint_id = required_non_empty(
        raw.blueprint_id,
        source_path,
        &format!("item `{item_id}` blueprint_scroll.blueprint_id"),
    )?;
    Ok(BlueprintScrollSpec { blueprint_id })
}

pub fn parse_inscription_scroll_spec(
    raw: InscriptionScrollSpecToml,
    source_path: &Path,
    item_id: &str,
) -> Result<InscriptionScrollSpec, String> {
    let inscription_id = required_non_empty(
        raw.inscription_id,
        source_path,
        &format!("item `{item_id}` inscription_scroll.inscription_id"),
    )?;
    Ok(InscriptionScrollSpec { inscription_id })
}

pub fn parse_technique_scroll_spec(
    raw: TechniqueScrollSpecToml,
    source_path: &Path,
    item_id: &str,
) -> Result<TechniqueScrollSpec, String> {
    let kind = required_non_empty(
        raw.kind,
        source_path,
        &format!("item `{item_id}` technique_scroll.kind"),
    )?;
    if kind != "combat_technique" {
        return Err(format!(
            "{} item `{item_id}` has unsupported technique_scroll.kind `{kind}`",
            source_path.display()
        ));
    }
    let skill_id = required_non_empty(
        raw.skill_id,
        source_path,
        &format!("item `{item_id}` technique_scroll.skill_id"),
    )?;
    if crate::cultivation::known_techniques::technique_definition(skill_id.as_str()).is_none() {
        return Err(format!(
            "{} item `{item_id}` references unknown technique_scroll.skill_id `{skill_id}`",
            source_path.display()
        ));
    }
    Ok(TechniqueScrollSpec { kind, skill_id })
}

/// plan-onboarding-loop-v1 P1.1 — 解析 TOML `[item.recipe_fragment]` 为 `RecipeFragmentSpec`。
pub fn parse_recipe_fragment_spec(
    raw: RecipeFragmentSpecToml,
    source_path: &Path,
    item_id: &str,
) -> Result<RecipeFragmentSpec, String> {
    let recipe_id = required_non_empty(
        raw.recipe_id,
        source_path,
        &format!("item `{item_id}` recipe_fragment.recipe_id"),
    )?;
    if raw.known_stages.is_empty() {
        return Err(format!(
            "{} item `{item_id}` recipe_fragment.known_stages must not be empty",
            source_path.display()
        ));
    }
    if !(1..=3).contains(&raw.max_quality_tier) {
        return Err(format!(
            "{} item `{item_id}` recipe_fragment.max_quality_tier {} must be 1..=3",
            source_path.display(),
            raw.max_quality_tier
        ));
    }
    Ok(RecipeFragmentSpec {
        recipe_id,
        known_stages: raw.known_stages,
        max_quality_tier: raw.max_quality_tier,
    })
}

fn parse_weapon_spec(
    raw: WeaponSpecToml,
    source_path: &Path,
    item_id: &str,
) -> Result<WeaponSpec, String> {
    use crate::combat::weapon::WeaponKind;
    let weapon_kind = match raw.kind.as_str() {
        "sword" => WeaponKind::Sword,
        "saber" => WeaponKind::Saber,
        "staff" => WeaponKind::Staff,
        "fist" => WeaponKind::Fist,
        "spear" => WeaponKind::Spear,
        "dagger" => WeaponKind::Dagger,
        "bow" => WeaponKind::Bow,
        other => {
            return Err(format!(
                "{} item `{item_id}` has invalid weapon.kind `{other}`; expected sword/saber/staff/fist/spear/dagger/bow",
                source_path.display()
            ));
        }
    };
    if !raw.base_attack.is_finite() || raw.base_attack < 0.0 {
        return Err(format!(
            "{} item `{item_id}` has invalid weapon.base_attack {}; expected finite >= 0",
            source_path.display(),
            raw.base_attack
        ));
    }
    if raw.quality_tier > 3 {
        return Err(format!(
            "{} item `{item_id}` has invalid weapon.quality_tier {}; expected 0..=3",
            source_path.display(),
            raw.quality_tier
        ));
    }
    if !raw.durability_max.is_finite() || raw.durability_max <= 0.0 {
        return Err(format!(
            "{} item `{item_id}` has invalid weapon.durability_max {}; expected finite > 0",
            source_path.display(),
            raw.durability_max
        ));
    }
    if !raw.qi_cost_mul.is_finite() || raw.qi_cost_mul <= 0.0 {
        return Err(format!(
            "{} item `{item_id}` has invalid weapon.qi_cost_mul {}; expected finite > 0",
            source_path.display(),
            raw.qi_cost_mul
        ));
    }
    Ok(WeaponSpec {
        weapon_kind,
        base_attack: raw.base_attack,
        quality_tier: raw.quality_tier,
        durability_max: raw.durability_max,
        qi_cost_mul: raw.qi_cost_mul,
    })
}

fn parse_item_category(
    raw: &str,
    source_path: &Path,
    item_id: &str,
) -> Result<ItemCategory, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "pill" => Ok(ItemCategory::Pill),
        "herb" => Ok(ItemCategory::Herb),
        "recipe_fragment" | "recipe-fragment" => Ok(ItemCategory::RecipeFragment),
        "recipe_hint" | "recipe-hint" => Ok(ItemCategory::RecipeHint),
        "weapon" => Ok(ItemCategory::Weapon),
        "armor" | "armour" => Ok(ItemCategory::Armor),
        "treasure" => Ok(ItemCategory::Treasure),
        "bonecoin" | "bone_coin" | "bone-coins" | "bone_coins" => Ok(ItemCategory::BoneCoin),
        "tool" => Ok(ItemCategory::Tool),
        "scroll" => Ok(ItemCategory::Scroll),
        "misc" => Ok(ItemCategory::Misc),
        "block" => Ok(ItemCategory::Block),
        "mineral" | "ore" => Ok(ItemCategory::Mineral),
        "anqi" | "hidden_weapon" => Ok(ItemCategory::Anqi),
        "liquid" => Ok(ItemCategory::Liquid),
        "container" => Ok(ItemCategory::Container),
        // plan-food-v1 P0 — 灵食分类
        "food" => Ok(ItemCategory::Food),
        // plan-shield-block-v1 P0 — 凡人级物理防御盾牌
        "shield" => Ok(ItemCategory::Shield),
        other => Err(format!(
            "{} item `{item_id}` has unknown category `{other}`",
            source_path.display()
        )),
    }
}

#[allow(dead_code)]
pub fn item_passes_filter(
    filter: &Option<Vec<ContainerAcceptFilter>>,
    item: &ItemInstance,
    registry: &ItemRegistry,
) -> bool {
    match filter.as_deref() {
        None | Some([]) => true,
        Some(filters) => filters.iter().any(|entry| match entry {
            ContainerAcceptFilter::Category(category) => registry
                .get(&item.template_id)
                .is_some_and(|template| template.category == *category),
            ContainerAcceptFilter::TemplatePrefix(prefix) => item.template_id.starts_with(prefix),
        }),
    }
}

fn parse_item_rarity(raw: &str, source_path: &Path, item_id: &str) -> Result<ItemRarity, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "common" => Ok(ItemRarity::Common),
        "uncommon" => Ok(ItemRarity::Uncommon),
        "rare" => Ok(ItemRarity::Rare),
        "epic" => Ok(ItemRarity::Epic),
        "legendary" => Ok(ItemRarity::Legendary),
        "ancient" => Ok(ItemRarity::Ancient),
        other => Err(format!(
            "{} item `{item_id}` has unknown rarity `{other}`",
            source_path.display()
        )),
    }
}

fn parse_item_effect(
    effect: ItemEffectToml,
    source_path: &Path,
    item_id: &str,
) -> Result<ItemEffect, String> {
    if !effect.magnitude.is_finite() || effect.magnitude < 0.0 {
        return Err(format!(
            "{} item `{item_id}` effect `{}` has invalid magnitude {}; expected finite >= 0",
            source_path.display(),
            effect.kind,
            effect.magnitude
        ));
    }

    match effect.kind.trim().to_ascii_lowercase().as_str() {
        "breakthrough_bonus" => Ok(ItemEffect::BreakthroughBonus {
            magnitude: effect.magnitude,
        }),
        "qi_recovery" => Ok(ItemEffect::QiRecovery {
            amount: effect.magnitude,
        }),
        "meridian_heal" => {
            let target =
                required_non_empty_option(effect.target, source_path, "item.effect.target")?;
            Ok(ItemEffect::MeridianHeal {
                magnitude: effect.magnitude,
                target,
            })
        }
        "contamination_cleanse" => Ok(ItemEffect::ContaminationCleanse {
            magnitude: effect.magnitude,
        }),
        "composure_restore" => Ok(ItemEffect::ComposureRestore {
            magnitude: effect.magnitude,
        }),
        "wound_heal" => Ok(ItemEffect::WoundHeal {
            magnitude: effect.magnitude,
            target: parse_wound_heal_effect_target(effect.target, source_path, item_id)?,
        }),
        "lifespan_extension" => {
            let source = effect
                .target
                .filter(|target| !target.trim().is_empty())
                .unwrap_or_else(|| "life_extension_pill".to_string());
            Ok(ItemEffect::LifespanExtension {
                years: effect.magnitude.floor() as u32,
                source,
            })
        }
        "anti_spirit_pressure" => Ok(ItemEffect::AntiSpiritPressure {
            duration_ticks: effect.magnitude.floor() as u64,
        }),
        "poison_pill" => {
            let pill_item_id =
                required_non_empty_option(effect.target, source_path, "item.effect.target")?;
            if PoisonPillKind::from_item_id(&pill_item_id).is_none() {
                return Err(format!(
                    "{} item `{item_id}` effect `poison_pill` has unknown poison pill target `{pill_item_id}`",
                    source_path.display()
                ));
            }
            Ok(ItemEffect::PoisonPill { pill_item_id })
        }
        "combat_pill" => {
            let pill_item_id = effect
                .target
                .filter(|target| !target.trim().is_empty())
                .unwrap_or_else(|| item_id.to_string());
            if crate::alchemy::pill::combat_pill_spec(&pill_item_id).is_none() {
                return Err(format!(
                    "{} item `{item_id}` effect `combat_pill` has unknown combat pill target `{pill_item_id}`",
                    source_path.display()
                ));
            }
            Ok(ItemEffect::CombatPill { pill_item_id })
        }
        "food_regen" => {
            // plan-food-v1 P2 — bonus_factor 来自 magnitude，duration_ticks 来自专属字段。
            let duration_ticks = effect.duration_ticks.ok_or_else(|| {
                format!(
                    "{} item `{item_id}` effect `food_regen` missing required field `duration_ticks`",
                    source_path.display()
                )
            })?;
            if duration_ticks == 0 {
                return Err(format!(
                    "{} item `{item_id}` effect `food_regen` has invalid duration_ticks 0; expected >= 1",
                    source_path.display()
                ));
            }
            Ok(ItemEffect::FoodRegen {
                bonus_factor: effect.magnitude as f32,
                duration_ticks,
            })
        }
        "beast_core_absorption" => {
            // plan-fauna-stitched-beast-v1 P3 — 异变兽核吸收。
            // magnitude = 突破加成幅度；duration_ticks = 幻觉持续 tick 数（默认 200）。
            let hallucination_duration_ticks =
                effect.duration_ticks.map(|t| t as u32).unwrap_or(200);
            if hallucination_duration_ticks == 0 {
                return Err(format!(
                    "{} item `{item_id}` effect `beast_core_absorption` has invalid hallucination_duration_ticks 0; expected >= 1",
                    source_path.display()
                ));
            }
            Ok(ItemEffect::BeastCoreAbsorption {
                breakthrough_magnitude: effect.magnitude,
                hallucination_duration_ticks,
            })
        }
        other => Err(format!(
            "{} item `{item_id}` has unsupported effect kind `{other}`",
            source_path.display()
        )),
    }
}

fn parse_wound_heal_effect_target(
    target: Option<String>,
    source_path: &Path,
    item_id: &str,
) -> Result<Option<String>, String> {
    let Some(raw) = target else {
        return Ok(None);
    };
    let parts: Vec<String> = raw
        .split('/')
        .map(|part| part.trim().to_ascii_lowercase())
        .collect();
    if parts.iter().any(String::is_empty) {
        return Err(format!(
            "{} item `{item_id}` effect `wound_heal` has empty target segment; omit target for all wounds or use body parts separated by `/`",
            source_path.display()
        ));
    }
    for part in &parts {
        if !is_wound_heal_body_part(part) {
            return Err(format!(
                "{} item `{item_id}` effect `wound_heal` has unknown target `{part}`; expected one of head/chest/back/abdomen/arm_l/arm_r/leg_l/leg_r",
                source_path.display()
            ));
        }
    }
    Ok(Some(parts.join("/")))
}

fn is_wound_heal_body_part(part: &str) -> bool {
    matches!(
        part,
        "head" | "chest" | "back" | "abdomen" | "arm_l" | "arm_r" | "leg_l" | "leg_r"
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LoadoutToml {
    #[serde(default)]
    max_weight: Option<f64>,
    #[serde(default)]
    bone_coins: Option<u64>,
    #[serde(default)]
    containers: Vec<LoadoutContainerToml>,
    #[serde(default)]
    equip: Vec<LoadoutEquipToml>,
    #[serde(default)]
    hotbar: Vec<LoadoutHotbarToml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LoadoutContainerToml {
    id: String,
    name: String,
    rows: u8,
    cols: u8,
    #[serde(default)]
    items: Vec<LoadoutPlacedItemToml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LoadoutPlacedItemToml {
    row: u8,
    col: u8,
    template_id: String,
    #[serde(default)]
    stack_count: Option<u32>,
    #[serde(default)]
    spirit_quality: Option<f64>,
    #[serde(default)]
    durability: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LoadoutEquipToml {
    slot: String,
    template_id: String,
    #[serde(default)]
    stack_count: Option<u32>,
    #[serde(default)]
    spirit_quality: Option<f64>,
    #[serde(default)]
    durability: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LoadoutHotbarToml {
    index: u8,
    template_id: String,
    #[serde(default)]
    stack_count: Option<u32>,
    #[serde(default)]
    spirit_quality: Option<f64>,
    #[serde(default)]
    durability: Option<f64>,
}

impl LoadoutToml {
    fn try_into_loadout(
        self,
        source_path: &Path,
        registry: &ItemRegistry,
    ) -> Result<LoadoutSpec, String> {
        let mut containers = Vec::new();
        let mut seen_container_ids = HashSet::new();
        for raw_container in self.containers {
            let container_id = required_non_empty(raw_container.id, source_path, "containers.id")?;
            validate_container_id(container_id.as_str(), source_path)?;
            if !seen_container_ids.insert(container_id.clone()) {
                return Err(format!(
                    "{} has duplicate container id `{container_id}` in loadout",
                    source_path.display()
                ));
            }
            let container_name =
                required_non_empty(raw_container.name, source_path, "containers.name")?;

            if !(1..=16).contains(&raw_container.rows) {
                return Err(format!(
                    "{} container `{container_id}` has invalid rows {}; expected 1..=16",
                    source_path.display(),
                    raw_container.rows
                ));
            }
            if !(1..=16).contains(&raw_container.cols) {
                return Err(format!(
                    "{} container `{container_id}` has invalid cols {}; expected 1..=16",
                    source_path.display(),
                    raw_container.cols
                ));
            }

            let mut items = Vec::new();
            for raw_item in raw_container.items {
                let row = raw_item.row;
                let col = raw_item.col;

                if row >= raw_container.rows {
                    return Err(format!(
                        "{} container `{container_id}` item row {} out of bounds for rows {}",
                        source_path.display(),
                        row,
                        raw_container.rows
                    ));
                }
                if col >= raw_container.cols {
                    return Err(format!(
                        "{} container `{container_id}` item col {} out of bounds for cols {}",
                        source_path.display(),
                        col,
                        raw_container.cols
                    ));
                }

                let instance = loadout_item_to_instance(raw_item, source_path, registry)?;
                let row_footprint_end = u16::from(row) + u16::from(instance.grid_h);
                let col_footprint_end = u16::from(col) + u16::from(instance.grid_w);

                if row_footprint_end > u16::from(raw_container.rows) {
                    return Err(format!(
                        "{} container `{container_id}` item `{}` footprint overflows rows: row {} + grid_h {} > {}",
                        source_path.display(),
                        instance.template_id,
                        row,
                        instance.grid_h,
                        raw_container.rows
                    ));
                }
                if col_footprint_end > u16::from(raw_container.cols) {
                    return Err(format!(
                        "{} container `{container_id}` item `{}` footprint overflows cols: col {} + grid_w {} > {}",
                        source_path.display(),
                        instance.template_id,
                        col,
                        instance.grid_w,
                        raw_container.cols
                    ));
                }

                let placed_item = PlacedItemState { row, col, instance };
                if let Some(existing_item) = items.iter().find(|existing_item| {
                    placed_item_footprints_overlap(existing_item, &placed_item)
                }) {
                    return Err(format!(
                        "{} container `{container_id}` item `{}` at row {}, col {} overlaps existing item `{}` at row {}, col {}",
                        source_path.display(),
                        placed_item.instance.template_id,
                        placed_item.row,
                        placed_item.col,
                        existing_item.instance.template_id,
                        existing_item.row,
                        existing_item.col
                    ));
                }

                items.push(placed_item);
            }

            containers.push(ContainerState {
                id: container_id,
                name: container_name,
                rows: raw_container.rows,
                cols: raw_container.cols,
                items,
                owner_instance_id: None,
            });
        }

        ensure_required_containers_present(&containers, source_path)?;

        // plan-layered-equip-v1 P0.6（决议 #17）— 每条 [[equip]] 按 classify 落 SlotContents：
        // 武器/工具 → held（手槽，每槽 ≤1）；盔甲/伪皮/背包件 → worn 栈尾（身体槽，可叠多件）。
        let mut equipped: HashMap<String, SlotContents> = HashMap::new();
        for raw_equip in self.equip {
            let slot_id = required_non_empty(raw_equip.slot, source_path, "equip.slot")?;
            validate_equip_slot(slot_id.as_str(), source_path)?;

            let instance = build_item_instance_from_template(
                raw_equip.template_id,
                raw_equip.stack_count,
                raw_equip.spirit_quality,
                raw_equip.durability,
                source_path,
                registry,
            )?;

            let state = classify_equip_state(&instance, registry);
            let contents = equipped.entry(slot_id.clone()).or_default();
            match state {
                EquipState::Held => {
                    if contents.held.is_some() {
                        return Err(format!(
                            "{} has duplicate held item in equip slot `{slot_id}`",
                            source_path.display()
                        ));
                    }
                    contents.held = Some(instance);
                }
                EquipState::Worn => contents.worn.push(instance),
            }
        }

        let mut hotbar: [Option<ItemInstance>; 9] = Default::default();
        for raw_slot in self.hotbar {
            if raw_slot.index >= 9 {
                return Err(format!(
                    "{} hotbar index {} out of bounds; expected 0..=8",
                    source_path.display(),
                    raw_slot.index
                ));
            }
            if hotbar[raw_slot.index as usize].is_some() {
                return Err(format!(
                    "{} has duplicate hotbar index {} in loadout",
                    source_path.display(),
                    raw_slot.index
                ));
            }

            let instance = build_item_instance_from_template(
                raw_slot.template_id,
                raw_slot.stack_count,
                raw_slot.spirit_quality,
                raw_slot.durability,
                source_path,
                registry,
            )?;
            hotbar[raw_slot.index as usize] = Some(instance);
        }

        let bone_coins = self.bone_coins.unwrap_or(0);
        if bone_coins > JS_SAFE_INTEGER_MAX {
            return Err(format!(
                "{} loadout bone_coins {} exceeds JS safe integer max {JS_SAFE_INTEGER_MAX}",
                source_path.display(),
                bone_coins
            ));
        }

        let max_weight = self.max_weight.unwrap_or(DEFAULT_PLAYER_MAX_WEIGHT);
        if !max_weight.is_finite() || max_weight <= 0.0 {
            return Err(format!(
                "{} loadout max_weight {} must be finite and > 0",
                source_path.display(),
                max_weight
            ));
        }

        Ok(LoadoutSpec {
            containers,
            equipped,
            hotbar,
            bone_coins,
            max_weight,
        })
    }
}

// ─── Inventory move (client → server intent application) ────────────────────

/// Outcome of a successful `apply_inventory_move`.
///
/// `Swapped` means the target slot was occupied by a same-footprint item; the
/// occupant has been bounced back to the source location. Caller should
/// resync the client (full snapshot) since two moves can't be expressed as
/// one ordered `inventory_event::moved` without ordering hazards.
#[derive(Debug, Clone, PartialEq)]
pub enum InventoryMoveOutcome {
    Moved {
        revision: InventoryRevision,
    },
    Swapped {
        revision: InventoryRevision,
        displaced_instance_id: u64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct InventoryExchangeOutcome {
    pub left_revision: InventoryRevision,
    pub right_revision: InventoryRevision,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InventoryDurabilityUpdate {
    pub revision: InventoryRevision,
    pub instance_id: u64,
    pub durability: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InventorySpiritualWearUpdate {
    pub revision: InventoryRevision,
    pub instance_id: u64,
    pub durability: f64,
    pub spirit_quality: f64,
    pub wear_fraction: f64,
}

/// Inventory item durability changed for a specific client entity.
///
/// This event exists to allow low-frequency incremental updates (e.g. armor hit
/// durability ticks) without requiring a full `inventory_snapshot` UI refresh.
#[derive(Debug, Clone, bevy_ecs::event::Event, PartialEq)]
pub struct InventoryDurabilityChangedEvent {
    pub entity: Entity,
    pub revision: InventoryRevision,
    pub instance_id: u64,
    pub durability: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InventoryConsumeOutcome {
    pub revision: InventoryRevision,
    pub remaining_stack: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DroppedItemRecord {
    pub container_id: String,
    pub row: u8,
    pub col: u8,
    pub instance: ItemInstance,
}

#[derive(Debug, Clone, bevy_ecs::event::Event, PartialEq)]
pub struct DroppedItemEvent {
    pub entity: Entity,
    pub revision: InventoryRevision,
    pub dropped: Vec<DroppedItemRecord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeathDropOutcome {
    pub revision: InventoryRevision,
    pub dropped: Vec<DroppedItemRecord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FullInventoryTransferOutcome {
    pub items_moved: usize,
    pub bone_coins_moved: u64,
    pub from_revision: InventoryRevision,
    pub to_revision: InventoryRevision,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DroppedLootEntry {
    pub instance_id: u64,
    pub source_container_id: String,
    pub source_row: u8,
    pub source_col: u8,
    pub world_pos: [f64; 3],
    pub dimension: DimensionKind,
    pub item: ItemInstance,
}

#[derive(Default, Resource, Debug)]
pub struct DroppedLootRegistry {
    /// World-visible drops keyed by `instance_id`.
    ///
    /// The pickup request only carries `instance_id`, so the registry must be
    /// addressable without an implicit owner. `instance_id` values are globally
    /// unique within a running server.
    pub entries: HashMap<u64, DroppedLootEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InventoryDiscardOutcome {
    pub revision: InventoryRevision,
    pub dropped: DroppedLootEntry,
}

/// Apply an `inventory_move_intent` to a player's inventory.
///
/// On success returns a [`InventoryMoveOutcome`] describing whether it was a
/// plain move or a same-footprint swap. On rejection returns the failure
/// reason; the caller is responsible for resyncing the client (e.g. via a
/// fresh `inventory_snapshot`) since the client UI optimistically updated.
///
/// Rejection paths:
/// - source location does not actually hold the named instance
/// - target out of bounds / unknown container
/// - target collides with a multi-cell item or the occupant footprint differs
pub fn apply_inventory_move(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    instance_id: u64,
    from: &crate::schema::inventory::InventoryLocationV1,
    to: &crate::schema::inventory::InventoryLocationV1,
) -> Result<InventoryMoveOutcome, String> {
    if !location_holds_instance(inventory, instance_id, from) {
        return Err(format!(
            "from-location {from:?} does not hold instance {instance_id}"
        ));
    }

    let item = clone_item_at(inventory, instance_id)
        .ok_or_else(|| format!("instance {instance_id} not found in inventory"))?;

    validate_move_semantics(registry, inventory, &item, from, to)?;

    let displaced = displaced_at_target(inventory, &item, instance_id, to)?;

    match displaced {
        None => {
            // Plain move.
            detach_instance(inventory, instance_id);
            attach_at_location(inventory, item, to)?;
            bump_revision(inventory);
            Ok(InventoryMoveOutcome::Moved {
                revision: inventory.revision,
            })
        }
        Some(occupant) => {
            // Footprint-matched swap. Validate occupant fits at `from`.
            if occupant.grid_w != item.grid_w || occupant.grid_h != item.grid_h {
                return Err(format!(
                    "swap rejected: occupant {} footprint {}x{} differs from dragged {}x{}",
                    occupant.instance_id,
                    occupant.grid_w,
                    occupant.grid_h,
                    item.grid_w,
                    item.grid_h
                ));
            }
            // Build a temp inventory after detaching both, then check occupant
            // fits at `from` against remaining items.
            let occupant_id = occupant.instance_id;
            detach_instance(inventory, instance_id);
            detach_instance(inventory, occupant_id);
            // Validate occupant fits at `from` (excluding both — both detached).
            if let Err(reason) = validate_attach_fits(inventory, &occupant, from) {
                // Restore originals to keep server state coherent on rare rejection.
                attach_at_location(inventory, item, from)
                    .expect("restoring original from is always valid (just detached)");
                attach_at_location(inventory, occupant, to)
                    .expect("restoring original to is always valid (just detached)");
                return Err(format!("swap rejected: {reason}"));
            }
            attach_at_location(inventory, item, to)?;
            attach_at_location(inventory, occupant, from)?;
            bump_revision(inventory);
            Ok(InventoryMoveOutcome::Swapped {
                revision: inventory.revision,
                displaced_instance_id: occupant_id,
            })
        }
    }
}

pub fn exchange_inventory_items(
    left_inventory: &mut PlayerInventory,
    left_instance_id: u64,
    right_inventory: &mut PlayerInventory,
    right_instance_id: u64,
) -> Result<InventoryExchangeOutcome, String> {
    if left_instance_id == right_instance_id {
        return Err(format!(
            "cannot exchange identical instance {left_instance_id}"
        ));
    }
    let left_item = clone_item_at(left_inventory, left_instance_id)
        .ok_or_else(|| format!("left instance {left_instance_id} not found"))?;
    let right_item = clone_item_at(right_inventory, right_instance_id)
        .ok_or_else(|| format!("right instance {right_instance_id} not found"))?;

    let mut next_left = left_inventory.clone();
    let mut next_right = right_inventory.clone();
    detach_instance(&mut next_left, left_instance_id);
    detach_instance(&mut next_right, right_instance_id);

    let left_receive_location = find_first_fit_container_location(&next_left, &right_item)
        .ok_or_else(|| format!("left inventory has no room for instance {right_instance_id}"))?;
    let right_receive_location = find_first_fit_container_location(&next_right, &left_item)
        .ok_or_else(|| format!("right inventory has no room for instance {left_instance_id}"))?;

    attach_at_location(&mut next_left, right_item, &left_receive_location)?;
    attach_at_location(&mut next_right, left_item, &right_receive_location)?;
    bump_revision(&mut next_left);
    bump_revision(&mut next_right);

    *left_inventory = next_left;
    *right_inventory = next_right;
    Ok(InventoryExchangeOutcome {
        left_revision: left_inventory.revision,
        right_revision: right_inventory.revision,
    })
}

pub fn set_item_instance_durability(
    inventory: &mut PlayerInventory,
    instance_id: u64,
    durability: f64,
) -> Result<InventoryDurabilityUpdate, String> {
    if !durability.is_finite() || !(0.0..=1.0).contains(&durability) {
        return Err(format!(
            "invalid durability {durability}; expected finite value in [0, 1]"
        ));
    }

    let item = inventory_item_by_instance_mut(inventory, instance_id)
        .ok_or_else(|| format!("instance {instance_id} not found in inventory"))?;
    item.durability = durability;
    bump_revision(inventory);
    Ok(InventoryDurabilityUpdate {
        revision: inventory.revision,
        instance_id,
        durability,
    })
}

pub fn apply_item_spiritual_wear(
    inventory: &mut PlayerInventory,
    instance_id: u64,
    wear_fraction: f64,
) -> Result<InventorySpiritualWearUpdate, String> {
    if !wear_fraction.is_finite() || !(0.0..=1.0).contains(&wear_fraction) {
        return Err(format!(
            "invalid spiritual wear {wear_fraction}; expected finite value in [0, 1]"
        ));
    }

    let item = inventory_item_by_instance_mut(inventory, instance_id)
        .ok_or_else(|| format!("instance {instance_id} not found in inventory"))?;
    item.durability = (item.durability - wear_fraction).clamp(0.0, 1.0);
    item.spirit_quality = (item.spirit_quality - wear_fraction).clamp(0.0, 1.0);
    let durability = item.durability;
    let spirit_quality = item.spirit_quality;
    bump_revision(inventory);
    Ok(InventorySpiritualWearUpdate {
        revision: inventory.revision,
        instance_id,
        durability,
        spirit_quality,
        wear_fraction,
    })
}

pub fn fully_repair_weapon_instance(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    instance_id: u64,
) -> Result<InventoryDurabilityUpdate, String> {
    let item = inventory_item_by_instance_borrow(inventory, instance_id)
        .ok_or_else(|| format!("instance {instance_id} not found in inventory"))?;
    let template = registry.get(&item.template_id).ok_or_else(|| {
        format!(
            "unknown template `{}` for instance {instance_id}",
            item.template_id
        )
    })?;
    if template.weapon_spec.is_none() {
        return Err(format!(
            "instance {instance_id} template `{}` is not a weapon",
            item.template_id
        ));
    }
    set_item_instance_durability(inventory, instance_id, 1.0)
}

pub fn move_equipped_item_to_first_container_slot(
    inventory: &mut PlayerInventory,
    instance_id: u64,
) -> Result<InventoryMoveOutcome, String> {
    // plan-layered-equip-v1 P0.2 / §11.1 #12 — 在 equipped 里定位该 instance 的槽 + 装备态。
    // worn 件仅栈顶（worn.last()）可移出，被压住的下层拒绝；held 件直接精确移除。
    let loc = find_equipped_instance(inventory, instance_id)
        .ok_or_else(|| format!("equipped instance {instance_id} not found"))?;
    if let EquippedInstanceLoc::Worn { ref slot, index } = loc {
        let worn_len = inventory
            .equipped
            .get(slot)
            .map(|s| s.worn.len())
            .unwrap_or(0);
        if index + 1 != worn_len {
            return Err(format!(
                "instance {instance_id} 被上层压住，请先脱下上层（worn 栈 LIFO，仅栈顶可卸下）"
            ));
        }
    }
    let item = clone_item_at(inventory, instance_id)
        .ok_or_else(|| format!("equipped instance {instance_id} missing"))?;
    let to = find_first_fit_container_location(inventory, &item)
        .ok_or_else(|| format!("no free container slot for instance {instance_id}"))?;

    detach_instance(inventory, instance_id);
    attach_at_location(inventory, item, &to)?;
    bump_revision(inventory);
    Ok(InventoryMoveOutcome::Moved {
        revision: inventory.revision,
    })
}

/// plan-layered-equip-v1 P0.2 — equipped 内某 instance 的精确位置。
#[derive(Debug, Clone)]
pub enum EquippedInstanceLoc {
    /// worn 层第 `index` 件（栈底=0，栈顶=worn.len()-1）。
    Worn { slot: String, index: usize },
    /// held 位。
    Held { slot: String },
}

/// plan-layered-equip-v1 P0.2 — 在 equipped 里按 instance_id 定位件（worn 层索引 / held 位）。
pub fn find_equipped_instance(
    inventory: &PlayerInventory,
    instance_id: u64,
) -> Option<EquippedInstanceLoc> {
    for (slot, contents) in &inventory.equipped {
        if let Some(index) = contents
            .worn
            .iter()
            .position(|item| item.instance_id == instance_id)
        {
            return Some(EquippedInstanceLoc::Worn {
                slot: slot.clone(),
                index,
            });
        }
        if contents
            .held
            .as_ref()
            .is_some_and(|item| item.instance_id == instance_id)
        {
            return Some(EquippedInstanceLoc::Held { slot: slot.clone() });
        }
    }
    None
}

pub fn inventory_item_by_instance(
    inventory: &PlayerInventory,
    instance_id: u64,
) -> Option<ItemInstance> {
    clone_item_at(inventory, instance_id)
}

/// Borrow-only 版本 — 返回 `&ItemInstance` 引用，避免 clone_item_at 的 ~5-6 次
/// String heap alloc。用于只读消费者（如 shelflife probe resolver），不需要把
/// item 搬出 inventory 的场景。
pub fn inventory_item_by_instance_borrow(
    inventory: &PlayerInventory,
    instance_id: u64,
) -> Option<&ItemInstance> {
    for c in &inventory.containers {
        if let Some(p) = c
            .items
            .iter()
            .find(|p| p.instance.instance_id == instance_id)
        {
            return Some(&p.instance);
        }
    }
    for slot in inventory.equipped.values() {
        if let Some(item) = slot.iter_all().find(|item| item.instance_id == instance_id) {
            return Some(item);
        }
    }
    inventory
        .hotbar
        .iter()
        .flatten()
        .find(|item| item.instance_id == instance_id)
}

pub fn consume_item_instance_once(
    inventory: &mut PlayerInventory,
    instance_id: u64,
) -> Result<InventoryConsumeOutcome, String> {
    for idx in 0..inventory.containers.len() {
        let maybe_remaining = {
            let container = &mut inventory.containers[idx];
            container
                .items
                .iter()
                .position(|p| p.instance.instance_id == instance_id)
                .map(|pos| {
                    if container.items[pos].instance.stack_count > 1 {
                        container.items[pos].instance.stack_count -= 1;
                        container.items[pos].instance.stack_count
                    } else {
                        container.items.remove(pos);
                        0
                    }
                })
        };
        if let Some(remaining_stack) = maybe_remaining {
            bump_revision(inventory);
            return Ok(InventoryConsumeOutcome {
                revision: inventory.revision,
                remaining_stack,
            });
        }
    }

    // plan-layered-equip-v1 P0.2 — 在 equipped worn 层 / held 位按 instance 精确消耗一件。
    if let Some(loc) = find_equipped_instance(inventory, instance_id) {
        let remaining_stack = match loc {
            EquippedInstanceLoc::Worn { slot, index } => {
                let contents = inventory
                    .equipped
                    .get_mut(&slot)
                    .expect("equipped slot key should still exist");
                if contents.worn[index].stack_count > 1 {
                    contents.worn[index].stack_count -= 1;
                    contents.worn[index].stack_count
                } else {
                    contents.worn.remove(index);
                    0
                }
            }
            EquippedInstanceLoc::Held { slot } => {
                let contents = inventory
                    .equipped
                    .get_mut(&slot)
                    .expect("equipped slot key should still exist");
                if let Some(held) = contents.held.as_mut() {
                    if held.stack_count > 1 {
                        held.stack_count -= 1;
                        held.stack_count
                    } else {
                        contents.held = None;
                        0
                    }
                } else {
                    0
                }
            }
        };
        bump_revision(inventory);
        return Ok(InventoryConsumeOutcome {
            revision: inventory.revision,
            remaining_stack,
        });
    }

    for idx in 0..inventory.hotbar.len() {
        let maybe_remaining = match &mut inventory.hotbar[idx] {
            Some(item) if item.instance_id == instance_id => {
                if item.stack_count > 1 {
                    item.stack_count -= 1;
                    Some(item.stack_count)
                } else {
                    inventory.hotbar[idx] = None;
                    Some(0)
                }
            }
            _ => None,
        };
        if let Some(remaining_stack) = maybe_remaining {
            bump_revision(inventory);
            return Ok(InventoryConsumeOutcome {
                revision: inventory.revision,
                remaining_stack,
            });
        }
    }

    Err(format!("instance {instance_id} not found in inventory"))
}

#[allow(clippy::too_many_arguments)]
pub fn apply_death_drop_on_revive(
    mut revived: bevy_ecs::event::EventReader<PlayerRevived>,
    mut commands: Commands,
    mut inventories: Query<&mut PlayerInventory>,
    registry: bevy_ecs::system::Res<ItemRegistry>,
    positions: Query<&Position>,
    anchors: Query<&DeathDropAnchor>,
    dimensions: Query<&CurrentDimension>,
    presences: Query<&crate::world::tsy::TsyPresence>,
    cultivations: Query<&crate::cultivation::components::Cultivation>,
    mut dropped_registry: bevy_ecs::system::ResMut<DroppedLootRegistry>,
    mut dropped_events: bevy_ecs::event::EventWriter<DroppedItemEvent>,
) {
    for ev in revived.read() {
        let Ok(mut inventory) = inventories.get_mut(ev.entity) else {
            continue;
        };
        let seed = death_drop_seed(ev.entity, inventory.revision.0);
        let base = positions
            .get(ev.entity)
            .map(|pos| pos.0)
            .unwrap_or(valence::math::DVec3::new(0.0, 64.0, 0.0));
        let entity_dimension = dimensions
            .get(ev.entity)
            .map(|dimension| dimension.0)
            .unwrap_or_default();

        // plan-tsy-loot-v1 §3.1：玩家在 TSY 内死亡 → 走分流（秘境所得 100% / 原带 50%）
        // + spawn 干尸 entity；否则走 §十二 主世界 50% 规则。
        if let Ok(presence) = presences.get(ev.entity) {
            let tsy_outcome = tsy_death_drop::apply_tsy_death_drop(
                &mut inventory,
                &registry,
                presence,
                base,
                seed,
            );
            if tsy_outcome.total_dropped() == 0 {
                continue;
            }
            let mut combined: Vec<DroppedItemRecord> = Vec::new();
            for (idx, record) in tsy_outcome
                .entry_carry_dropped
                .iter()
                .chain(tsy_outcome.tsy_acquired_dropped.iter())
                .enumerate()
            {
                // plan-tsy-lifecycle-v1 §3.3 — 把 family 写进 source_container_id 前缀，
                // 让 lifecycle cleanup 能精确识别"属于本 family 的塌缩残留"，避免
                // 主世界同 XYZ 的 entries 被误删（Codex review P1）。
                let entry = DroppedLootEntry {
                    instance_id: record.instance.instance_id,
                    source_container_id: format!(
                        "tsy_corpse:{}/{}",
                        presence.family_id, record.container_id
                    ),
                    source_row: record.row,
                    source_col: record.col,
                    world_pos: [base.x + 0.35 + idx as f64 * 0.1, base.y, base.z + 0.35],
                    dimension: DimensionKind::Tsy,
                    item: record.instance.clone(),
                };
                dropped_registry.entries.insert(entry.instance_id, entry);
                combined.push(record.clone());
            }

            // §4.3：干尸实体落 corpse_pos。MVP 仅 Position + CorpseEmbalmed component；
            // visual marker mob 由后续 P3 plan-tsy-polish 接 Valence entity sync。
            let drop_ids: Vec<u64> = combined.iter().map(|r| r.instance.instance_id).collect();
            // plan-daozhan-v1 B2 fix：从死亡玩家的 Cultivation 读取真实境界，
            // 让 CorpseEmbalmed.origin_realm 携带真境界触发道伥 spawn 概率门控。
            let origin_realm = cultivations.get(ev.entity).ok().map(|c| c.realm);
            commands.spawn((
                Position(tsy_outcome.corpse_pos),
                corpse::CorpseEmbalmed {
                    family_id: presence.family_id.clone(),
                    died_at_tick: presence.entered_at_tick, // MVP：用 entered_tick 占位；P2 lifecycle 用真 death tick
                    death_cause: "tsy_death".to_string(),
                    drops: drop_ids,
                    activated_to_daoxiang: false,
                    origin_realm,
                },
            ));

            dropped_events.send(DroppedItemEvent {
                entity: ev.entity,
                revision: inventory.revision,
                dropped: combined,
            });
            continue;
        }

        // ----- 主世界路径（保持原 §十二 50% 行为） -----
        let outcome = apply_death_drop_to_inventory(&mut inventory, &registry, seed);

        if outcome.dropped.is_empty() {
            continue;
        }

        let base = anchors
            .get(ev.entity)
            .map(|anchor| anchor.pos)
            .or_else(|_| {
                positions.get(ev.entity).map(|pos| {
                    let p = pos.0;
                    [p.x, p.y, p.z]
                })
            })
            .unwrap_or([0.0, 64.0, 0.0]);
        let start_idx = dropped_registry.entries.len();
        for (idx, dropped) in outcome.dropped.iter().enumerate() {
            let entry = DroppedLootEntry {
                instance_id: dropped.instance.instance_id,
                source_container_id: dropped.container_id.clone(),
                source_row: dropped.row,
                source_col: dropped.col,
                world_pos: [
                    base[0] + 0.35 + (start_idx + idx) as f64 * 0.1,
                    base[1],
                    base[2] + 0.35,
                ],
                dimension: entity_dimension,
                item: dropped.instance.clone(),
            };
            dropped_registry.entries.insert(entry.instance_id, entry);
        }

        // Anchor is only needed until the revive-drop is materialized.
        commands.entity(ev.entity).remove::<DeathDropAnchor>();

        dropped_events.send(DroppedItemEvent {
            entity: ev.entity,
            revision: outcome.revision,
            dropped: outcome.dropped,
        });
    }
}

pub fn apply_death_drop_to_inventory(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    seed: u64,
) -> DeathDropOutcome {
    // plan-layered-equip-v1 P0.2 死亡掉落子任务（gap#2 blocker）— 高耐真武器（durability≥0.5）
    // 免 50% 掉落 Roll；武器从手槽 held 派生（双手兵器即 main_hand.held，决议 #7，不再有 two_hand 槽）。
    let protected_weapon_ids = inventory
        .equipped
        .iter()
        .filter_map(|(_, contents)| contents.held.as_ref())
        .filter(|item| item.durability >= 0.5)
        .filter_map(|item| {
            registry
                .get(&item.template_id)
                .and_then(|template| template.weapon_spec.as_ref().map(|_| item.instance_id))
        })
        .collect::<HashSet<_>>();

    let mut candidate_ids = Vec::new();
    for container in &inventory.containers {
        for placed in &container.items {
            candidate_ids.push(placed.instance.instance_id);
        }
    }
    // plan-layered-equip-v1 P0.2（桶④）— 遍历全件：worn 全层 + held。
    for contents in inventory.equipped.values() {
        for item in contents.iter_all() {
            // held 武器若受高耐保护则跳过；worn 件无保护。
            if protected_weapon_ids.contains(&item.instance_id) {
                continue;
            }
            candidate_ids.push(item.instance_id);
        }
    }
    for item in inventory.hotbar.iter().flatten() {
        candidate_ids.push(item.instance_id);
    }

    let drop_count = candidate_ids.len() / 2;
    if drop_count == 0 {
        return DeathDropOutcome {
            revision: inventory.revision,
            dropped: Vec::new(),
        };
    }

    let selected_ids = select_drop_instance_ids(candidate_ids, drop_count, seed);
    let selected: HashSet<u64> = selected_ids.into_iter().collect();

    let mut dropped = Vec::new();
    for container in &mut inventory.containers {
        let container_id = container.id.clone();
        let mut kept = Vec::with_capacity(container.items.len());
        for placed in container.items.drain(..) {
            if selected.contains(&placed.instance.instance_id) {
                dropped.push(DroppedItemRecord {
                    container_id: container_id.clone(),
                    row: placed.row,
                    col: placed.col,
                    instance: placed.instance,
                });
            } else {
                kept.push(placed);
            }
        }
        container.items = kept;
    }

    // plan-layered-equip-v1 P0.2 死亡掉落子任务（gap#2 blocker）— 按 instance 精确移除：
    // worn 件按 instance_id 在该槽 worn Vec 定位后移除该一件（保留同槽其余 worn 层）；
    // held 命中则清 held=None。**禁止整槽 remove（会连带删未掉落的 worn 下层 / held）**。
    for (slot, contents) in inventory.equipped.iter_mut() {
        let mut idx = 0;
        while idx < contents.worn.len() {
            if selected.contains(&contents.worn[idx].instance_id) {
                let instance = contents.worn.remove(idx);
                dropped.push(DroppedItemRecord {
                    container_id: slot.clone(),
                    row: 0,
                    col: 0,
                    instance,
                });
            } else {
                idx += 1;
            }
        }
        if contents
            .held
            .as_ref()
            .is_some_and(|item| selected.contains(&item.instance_id))
        {
            if let Some(instance) = contents.held.take() {
                dropped.push(DroppedItemRecord {
                    container_id: slot.clone(),
                    row: 0,
                    col: 0,
                    instance,
                });
            }
        }
    }

    for slot_idx in 0..inventory.hotbar.len() {
        let should_drop = inventory.hotbar[slot_idx]
            .as_ref()
            .map(|item| selected.contains(&item.instance_id))
            .unwrap_or(false);
        if !should_drop {
            continue;
        }
        if let Some(item) = inventory.hotbar[slot_idx].take() {
            dropped.push(DroppedItemRecord {
                container_id: "hotbar".to_string(),
                row: 0,
                col: slot_idx as u8,
                instance: item,
            });
        }
    }

    if !dropped.is_empty() {
        bump_revision(inventory);
    }

    DeathDropOutcome {
        revision: inventory.revision,
        dropped,
    }
}

pub fn transfer_all_inventory_contents(
    from: &mut PlayerInventory,
    to: &mut PlayerInventory,
) -> FullInventoryTransferOutcome {
    let mut items = Vec::new();
    for container in &mut from.containers {
        items.extend(container.items.drain(..).map(|placed| placed.instance));
    }
    // plan-layered-equip-v1 P0.2（桶④）— transfer 全件（worn 全层 + held）。
    items.extend(
        from.equipped
            .drain()
            .flat_map(|(_, contents)| contents.worn.into_iter().chain(contents.held)),
    );
    for slot in &mut from.hotbar {
        if let Some(item) = slot.take() {
            items.push(item);
        }
    }

    let moved_items = items.len();
    for item in items {
        force_attach_item_to_inventory(to, item);
    }

    let bone_coin_room = JS_SAFE_INTEGER_MAX.saturating_sub(to.bone_coins);
    let moved_bone_coins = from.bone_coins.min(bone_coin_room);
    if moved_bone_coins > 0 {
        from.bone_coins = from.bone_coins.saturating_sub(moved_bone_coins);
        to.bone_coins = to.bone_coins.saturating_add(moved_bone_coins);
    }

    if moved_items > 0 || moved_bone_coins > 0 {
        bump_revision(from);
        bump_revision(to);
    }

    FullInventoryTransferOutcome {
        items_moved: moved_items,
        bone_coins_moved: moved_bone_coins,
        from_revision: from.revision,
        to_revision: to.revision,
    }
}

pub(crate) fn force_attach_item_to_inventory(inventory: &mut PlayerInventory, item: ItemInstance) {
    if let Some(location) = find_first_fit_container_location(inventory, &item) {
        if attach_at_location(inventory, item.clone(), &location).is_ok() {
            return;
        }
    }

    let target_idx = inventory
        .containers
        .iter()
        .position(|container| container.id == MAIN_PACK_CONTAINER_ID)
        .or_else(|| (!inventory.containers.is_empty()).then_some(0))
        .unwrap_or_else(|| {
            inventory.containers.push(ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 16,
                cols: 16,
                items: Vec::new(),
                owner_instance_id: None,
            });
            inventory.containers.len() - 1
        });
    inventory.containers[target_idx]
        .items
        .push(PlacedItemState {
            row: 0,
            col: 0,
            instance: item,
        });
}

pub fn calculate_current_weight(inventory: &PlayerInventory) -> f64 {
    let container_weight = inventory
        .containers
        .iter()
        .flat_map(|container| container.items.iter())
        .map(|entry| entry.instance.weight * entry.instance.stack_count as f64)
        .sum::<f64>();
    // plan-layered-equip-v1 P3 公式7：遍历每槽 worn 全件 + held（含手持武器、含背包件自重）。
    let equipped_weight = inventory
        .equipped
        .values()
        .flat_map(|slot| slot.iter_all())
        .map(|item| item.weight * item.stack_count as f64)
        .sum::<f64>();
    let hotbar_weight = inventory
        .hotbar
        .iter()
        .flatten()
        .map(|item| item.weight * item.stack_count as f64)
        .sum::<f64>();

    container_weight + equipped_weight + hotbar_weight
}

/// plan-layered-equip-v1 P0.2（决议 #17）— 容器 id 命名规则：穿戴背包件 → 容器 id。
///
/// 背包专属槽取消后，容器 id 由穿戴背包件 instance 派生（`pack_<instance_id>`），
/// 与 rebuild_containers / attrition 反查 / 静态 `[[containers]]` 共用一套命名空间。
pub fn container_id_for_worn_pack(instance_id: u64) -> String {
    format!("pack_{instance_id}")
}

/// plan-layered-equip-v1 P0.2（决议 #17）— 反解容器 id → 穿戴背包件 instance_id。
/// 仅识别 `pack_<id>` 前缀；body_pocket / 其余容器返回 None。
pub fn worn_pack_instance_from_container_id(container_id: &str) -> Option<u64> {
    container_id.strip_prefix("pack_")?.parse::<u64>().ok()
}

/// plan-layered-equip-v1 P0.2（决议 #17）— 迭代所有身体槽 worn 层里带 `container_spec` 的背包件。
/// 产出 `(instance, ContainerSpec)`，供 compute_max_weight / rebuild_containers / attrition 共用。
fn worn_container_items<'a>(
    inventory: &'a PlayerInventory,
    registry: &'a ItemRegistry,
) -> impl Iterator<Item = (&'a ItemInstance, &'a ContainerSpec)> {
    inventory
        .equipped
        .values()
        .flat_map(|slot| slot.worn.iter())
        .filter_map(move |item| {
            registry
                .get(&item.template_id)
                .and_then(|t| t.container_spec.as_ref())
                .map(|spec| (item, spec))
        })
}

/// plan-layered-equip-v1 P0.2 / §11.1 #17 — 根据已装备背包重算 `max_weight`。
///
/// 公式：`BASE_CARRY_CAPACITY + Σ(所有身体槽 worn 层里带 container_spec 的件的 weight_capacity)`。
/// 暗袋（body_pocket）不提供额外负重，始终使用 BASE_CARRY_CAPACITY 作为基础。
#[allow(dead_code)]
pub fn compute_max_weight(inventory: &PlayerInventory, registry: &ItemRegistry) -> f64 {
    let backpack_bonus: f64 = worn_container_items(inventory, registry)
        .map(|(_, spec)| spec.weight_capacity)
        .sum();

    BASE_CARRY_CAPACITY + backpack_bonus
}

/// plan-layered-equip-v1 P0.2 / §11.1 #13.5 #17 — 根据身体槽 worn 层背包件重建动态容器列表。
///
/// 规则（决议 #17，背包专属槽取消）：
/// 1. `body_pocket`（2×3）始终存在；不存在时创建空容器。
/// 2. 扫所有身体槽 worn 层里带 `container_spec` 的背包件：容器 id = `pack_<instance_id>`；
///    存在则更新 rows/cols（升级换品），否则 push 新空容器。
/// 3. 移除已不再对应任何穿戴背包件的孤儿 `pack_*` 容器。**孤儿容器若非空，先把其物品
///    溢出（spill）到其它存活容器（背包/暗袋），实在放不下的随返回值上抛由调用方掉落，
///    再移除容器**——绝不允许残留「可 access 的孤儿容器」（Bug C：丢背包后还能从孤儿容器
///    取物）。
/// 4. 刷新 `max_weight = compute_max_weight(...)`。
///
/// 返回：溢出后仍无处安放、需由调用方转为掉落物的物品（无则空 Vec）。
#[allow(dead_code)]
pub fn rebuild_containers_from_equipment(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
) -> Vec<ItemInstance> {
    // 1. 确保 body_pocket 始终存在。
    if !inventory
        .containers
        .iter()
        .any(|c| c.id == BODY_POCKET_CONTAINER_ID)
    {
        inventory.containers.push(ContainerState {
            id: BODY_POCKET_CONTAINER_ID.to_string(),
            name: "暗袋".to_string(),
            rows: BODY_POCKET_ROWS,
            cols: BODY_POCKET_COLS,
            items: Vec::new(),
            owner_instance_id: None,
        });
    }

    // 2. 扫所有身体槽 worn 层里带 container_spec 的背包件，确保各自容器存在。
    //    plan-tarkov-backpack-v1 P0（交付物 #2）：创建/刷新 `pack_<id>` 容器时写
    //    `owner_instance_id = Some(instance_id)`，建立背包件 ↔ 容器的语义归属。
    let live_specs: Vec<(String, u8, u8, String, u64)> = worn_container_items(inventory, registry)
        .map(|(item, spec)| {
            (
                container_id_for_worn_pack(item.instance_id),
                spec.rows,
                spec.cols,
                item.display_name.clone(),
                item.instance_id,
            )
        })
        .collect();
    let live_ids: std::collections::HashSet<String> = live_specs
        .iter()
        .map(|(id, _, _, _, _)| id.clone())
        .collect();

    for (container_id, rows, cols, name, instance_id) in live_specs {
        if let Some(existing) = inventory
            .containers
            .iter_mut()
            .find(|c| c.id == container_id)
        {
            existing.rows = rows;
            existing.cols = cols;
            existing.owner_instance_id = Some(instance_id);
        } else {
            inventory.containers.push(ContainerState {
                id: container_id,
                name,
                rows,
                cols,
                items: Vec::new(),
                owner_instance_id: Some(instance_id),
            });
        }
    }

    // 3. 处理孤儿 pack_* 容器（无对应穿戴背包件）。
    //    先收集孤儿容器 id（不可在迭代 containers 时同时 mutate），再逐个：
    //    取出内含物 → 移除容器 → 把内含物 spill 进存活容器（背包优先、暗袋兜底） →
    //    放不下的归到 overflow 由调用方掉落。决不保留可 access 的孤儿容器。
    let orphan_ids: Vec<String> = inventory
        .containers
        .iter()
        .filter(|c| {
            worn_pack_instance_from_container_id(&c.id).is_some() && !live_ids.contains(&c.id)
        })
        .map(|c| c.id.clone())
        .collect();

    let mut overflow: Vec<ItemInstance> = Vec::new();
    for orphan_id in orphan_ids {
        let Some(pos) = inventory.containers.iter().position(|c| c.id == orphan_id) else {
            continue;
        };
        // 取出孤儿容器（连同其物品），从 containers 中移除——孤儿不再可 access。
        let orphan = inventory.containers.remove(pos);
        for placed in orphan.items {
            // spill 到其它存活容器；放不下则上抛 overflow（调用方掉落，不丢数据）。
            match find_first_fit_container_location(inventory, &placed.instance) {
                Some(location) => {
                    // attach 不应失败（location 来自 find_first_fit 的实时校验）；
                    // 万一失败也不丢件——回落 overflow。
                    if let Err(_reason) =
                        attach_at_location(inventory, placed.instance.clone(), &location)
                    {
                        overflow.push(placed.instance);
                    }
                }
                None => overflow.push(placed.instance),
            }
        }
    }

    // 4. 刷新 max_weight。
    inventory.max_weight = compute_max_weight(inventory, registry);

    overflow
}

/// plan-tarkov-backpack-v1 P0（交付物 #4 红线 — rebuild + overflow→掉落接进 move 路径）。
///
/// **这是「卸下 worn 背包件后内含物 spill / overflow→掉落」生产接线的唯一封装**——`handle_inventory_move`
/// 的 worn-pack 卸下分支显式调用本函数（不再依赖任何不存在的 Bevy auto-system），server e2e
/// 经本 seam 锁住接线（不绕 handler 直测 `apply_inventory_move` 内部）。
///
/// 流程：
/// 1. 调 `rebuild_containers_from_equipment` 刷新容器列表——卸下的背包件已不在 worn 层，
///    其 `pack_<id>` 容器变孤儿，rebuild 把内含物 spill 进其它存活容器（背包/暗袋），
///    放不下的归 overflow。穿背包时（背包件进 worn 层）rebuild 即时新建对应 `pack_<id>` 容器。
/// 2. overflow 逐件转 `DroppedLootEntry` 写入 `DroppedLootRegistry`（用既有 instance_id，
///    不分配新 id），world_pos 在玩家脚下错位铺开（与 `discard_inventory_item_to_dropped_loot`
///    同款）。**禁止静默丢失**——放不下的内含物连货掉地（塔科夫式直觉）。
///
/// 返回掉落的 instance_id 列表（供调用方日志 / 测试守恒断言；空 = 无 overflow）。
pub fn rebuild_and_drop_overflow(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    dropped_registry: &mut DroppedLootRegistry,
    player_pos: [f64; 3],
    player_dimension: DimensionKind,
) -> Vec<u64> {
    let overflow = rebuild_containers_from_equipment(inventory, registry);
    let mut dropped_ids = Vec::with_capacity(overflow.len());
    for item in overflow {
        let instance_id = item.instance_id;
        // 错位铺开避免叠在同一格；index 取自当前 registry 大小（与 discard 一致）。
        let next_idx = dropped_registry.entries.len();
        let dropped = DroppedLootEntry {
            instance_id,
            source_container_id: "backpack_unequip_overflow".to_string(),
            source_row: 0,
            source_col: 0,
            world_pos: [
                player_pos[0] + 0.35 + next_idx as f64 * 0.1,
                player_pos[1],
                player_pos[2] + 0.35,
            ],
            dimension: player_dimension,
            item,
        };
        dropped_registry.entries.insert(instance_id, dropped);
        dropped_ids.push(instance_id);
    }
    dropped_ids
}

// ─── plan-layered-equip-v1 P0.2 — 背包耐久扣减与破损溢出（决议 #17 重定向到 worn 背包件 instance） ───

/// 背包破损事件，当背包耐久降至 ≤ε 时由 `apply_backpack_wear` 返回。
#[derive(Debug, Clone, PartialEq)]
pub struct BackpackBreakEvent {
    /// 破损背包件的 instance_id。
    pub backpack_instance_id: u64,
    /// 触发耗损操作的容器 id（`pack_<instance_id>`）。
    pub container_id: String,
}

/// 背包破损溢出结果。
#[derive(Debug, Clone, PartialEq)]
pub struct BackpackBreakOutcome {
    /// 破损背包件的 instance_id。
    pub backpack_instance_id: u64,
    /// 容器 id（`pack_<instance_id>`）。
    pub container_id: String,
    /// 背包内容物（调用方负责转为 DroppedItemEvent）。
    pub spilled_items: Vec<ItemInstance>,
    /// 破损的背包物品实例（已从 equipped worn 中移除）。
    pub backpack_item: ItemInstance,
    /// 破损后重算的新 max_weight。
    pub new_max_weight: f64,
}

/// plan-layered-equip-v1 P0.2（决议 #17 / #20）— 容器 id → 穿戴背包件可变借用定位。
///
/// 在所有身体槽 worn 层里按 `pack_<instance_id>` 反解出的 instance 定位背包件。
/// `body_pocket` 及未知容器返回 `None`（不扣耐久 / 不豁免）。
fn worn_pack_item_mut<'a>(
    inventory: &'a mut PlayerInventory,
    container_id: &str,
) -> Option<&'a mut ItemInstance> {
    let instance_id = worn_pack_instance_from_container_id(container_id)?;
    inventory
        .equipped
        .values_mut()
        .flat_map(|slot| slot.worn.iter_mut())
        .find(|item| item.instance_id == instance_id)
}

/// plan-layered-equip-v1 P0.2 — 对指定背包容器的穿戴背包件扣减一次耐久损耗。
///
/// 规则：
/// - `container_id` 非 `pack_*`（body_pocket 或未知）→ 返回 None，不扣减。
/// - 对应背包件未穿戴 → 返回 None。
/// - 背包模板无 `container_spec` → 返回 None。
/// - `durability_cost_per_op == 0.0` → 不扣减（无损耗），返回 None。
/// - 扣减后 `durability ≤ ε` → 返回 `Some(BackpackBreakEvent)`，否则返回 None。
#[allow(dead_code)]
pub fn apply_backpack_wear(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    container_id: &str,
) -> Option<BackpackBreakEvent> {
    let (instance_id, durability) = {
        let backpack = worn_pack_item_mut(inventory, container_id)?;
        let template = registry.get(&backpack.template_id)?;
        let cost = template.container_spec.as_ref()?.durability_cost_per_op;
        if cost <= 0.0 {
            return None;
        }
        backpack.durability = (backpack.durability - cost).max(0.0);
        (backpack.instance_id, backpack.durability)
    };
    bump_revision(inventory);
    if durability <= f64::EPSILON {
        Some(BackpackBreakEvent {
            backpack_instance_id: instance_id,
            container_id: container_id.to_string(),
        })
    } else {
        None
    }
}

/// plan-layered-equip-v1 P0.2 — 处理背包破损溢出。
///
/// 逻辑：
/// 1. 从身体槽 worn 层精确移除该背包件（按 instance_id）。
/// 2. 从 `containers` 中找到对应 `pack_*` 容器，提取所有 items。
/// 3. 移除该容器。
/// 4. 调用 `rebuild_containers_from_equipment` 刷新 `max_weight`。
/// 5. 返回 `BackpackBreakOutcome`（spilled_items 由调用方转为 DroppedItemEvent）。
///
/// 若容器 id 非 `pack_*`，或对应背包件未穿戴，返回 `None`（无操作）。
#[allow(dead_code)]
pub fn handle_backpack_break(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    container_id: &str,
) -> Option<BackpackBreakOutcome> {
    let instance_id = worn_pack_instance_from_container_id(container_id)?;

    // 1. 在身体槽 worn 层按 instance_id 精确移除背包件。
    let mut backpack_item: Option<ItemInstance> = None;
    for slot in inventory.equipped.values_mut() {
        if let Some(pos) = slot
            .worn
            .iter()
            .position(|item| item.instance_id == instance_id)
        {
            backpack_item = Some(slot.worn.remove(pos));
            break;
        }
    }
    let backpack_item = backpack_item?;

    // 2. 提取容器内所有物品。
    let container_pos = inventory
        .containers
        .iter()
        .position(|c| c.id == container_id);
    let mut spilled_items: Vec<ItemInstance> = if let Some(pos) = container_pos {
        // 3. 移除容器，并取出内容物。
        let container = inventory.containers.remove(pos);
        container
            .items
            .into_iter()
            .map(|placed| placed.instance)
            .collect()
    } else {
        Vec::new()
    };

    // 4. 刷新 max_weight（equipped 已更新，rebuild 会重算）。
    //    rebuild 顺带清理任何其它孤儿 pack_* 容器，其 spill 不下的 overflow 一并掉落（不丢数据）。
    let rebuild_overflow = rebuild_containers_from_equipment(inventory, registry);
    spilled_items.extend(rebuild_overflow);
    let new_max_weight = inventory.max_weight;
    bump_revision(inventory);

    Some(BackpackBreakOutcome {
        backpack_instance_id: instance_id,
        container_id: container_id.to_string(),
        spilled_items,
        backpack_item,
        new_max_weight,
    })
}

pub fn dropped_loot_snapshot(registry: &DroppedLootRegistry) -> Vec<DroppedLootEntry> {
    let mut drops = registry.entries.values().cloned().collect::<Vec<_>>();
    // Deterministic ordering avoids client-side insertionOrder churn.
    drops.sort_by_key(|entry| entry.instance_id);
    drops
}

pub struct TemplateDroppedLootRequest<'a> {
    pub template_id: &'a str,
    pub stack_count: u32,
    pub world_pos: [f64; 3],
    pub dimension: DimensionKind,
    pub current_tick: u64,
}

pub fn spawn_template_dropped_loot(
    registry: &mut DroppedLootRegistry,
    item_registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
    request: TemplateDroppedLootRequest<'_>,
) -> Result<DroppedLootEntry, String> {
    if request.stack_count == 0 {
        return Err("spawn_template_dropped_loot requires stack_count >= 1".to_string());
    }
    let template = item_registry
        .get(request.template_id)
        .ok_or_else(|| format!("unknown item template id `{}`", request.template_id))?;
    let instance_id = allocator.next_id()?;
    let dropped = DroppedLootEntry {
        instance_id,
        source_container_id: "placeable_break".to_string(),
        source_row: 0,
        source_col: 0,
        world_pos: request.world_pos,
        dimension: request.dimension,
        item: runtime_instance_from_template(
            template,
            instance_id,
            request.stack_count,
            request.current_tick,
        ),
    };
    registry.entries.insert(instance_id, dropped.clone());
    Ok(dropped)
}

pub fn pickup_dropped_loot_instance(
    inventory: &mut PlayerInventory,
    registry: &mut DroppedLootRegistry,
    player_pos: [f64; 3],
    instance_id: u64,
) -> Result<InventoryRevision, String> {
    let entry = registry
        .entries
        .get(&instance_id)
        .cloned()
        .ok_or_else(|| format!("dropped instance {instance_id} not found"))?;
    let dx = entry.world_pos[0] - player_pos[0];
    let dy = entry.world_pos[1] - player_pos[1];
    let dz = entry.world_pos[2] - player_pos[2];
    if dx * dx + dy * dy + dz * dz > 2.5f64 * 2.5f64 {
        return Err(format!(
            "dropped instance {instance_id} out of pickup range"
        ));
    }

    let location = find_first_fit_container_location(inventory, &entry.item)
        .ok_or_else(|| format!("no free container slot for dropped instance {instance_id}"))?;
    attach_at_location(inventory, entry.item, &location)?;
    bump_revision(inventory);

    registry.entries.remove(&instance_id);

    Ok(inventory.revision)
}

pub fn discard_inventory_item_to_dropped_loot(
    inventory: &mut PlayerInventory,
    registry: &mut DroppedLootRegistry,
    player_pos: [f64; 3],
    player_dimension: DimensionKind,
    instance_id: u64,
    from: &crate::schema::inventory::InventoryLocationV1,
) -> Result<InventoryDiscardOutcome, String> {
    if !location_holds_instance(inventory, instance_id, from) {
        return Err(format!(
            "from-location {from:?} does not hold instance {instance_id}"
        ));
    }

    let item = clone_item_at(inventory, instance_id)
        .ok_or_else(|| format!("instance {instance_id} not found in inventory"))?;

    detach_instance(inventory, instance_id);
    bump_revision(inventory);

    let (source_container_id, source_row, source_col) = match from {
        crate::schema::inventory::InventoryLocationV1::Container {
            container_id,
            row,
            col,
        } => (
            container_id_str(container_id).to_string(),
            *row as u8,
            *col as u8,
        ),
        crate::schema::inventory::InventoryLocationV1::Equip { slot, .. } => {
            (equip_slot_key(slot).to_string(), 0, 0)
        }
        crate::schema::inventory::InventoryLocationV1::Hotbar { index } => {
            ("hotbar".to_string(), 0, u64::from(*index) as u8)
        }
    };

    let next_idx = registry.entries.len();
    let dropped = DroppedLootEntry {
        instance_id,
        source_container_id,
        source_row,
        source_col,
        world_pos: [
            player_pos[0] + 0.35 + next_idx as f64 * 0.1,
            player_pos[1],
            player_pos[2] + 0.35,
        ],
        dimension: player_dimension,
        item,
    };
    registry.entries.insert(instance_id, dropped.clone());

    Ok(InventoryDiscardOutcome {
        revision: inventory.revision,
        dropped,
    })
}

pub fn sync_overloaded_marker(
    mut commands: Commands,
    players: Query<(Entity, &PlayerInventory, Option<&OverloadedMarker>)>,
) {
    for (entity, inventory, existing_marker) in &players {
        let current_weight = calculate_current_weight(inventory);
        let should_mark = current_weight > inventory.max_weight;

        match (should_mark, existing_marker) {
            (true, Some(marker))
                if (marker.current_weight - current_weight).abs() < f64::EPSILON
                    && (marker.max_weight - inventory.max_weight).abs() < f64::EPSILON => {}
            (true, _) => {
                commands.entity(entity).insert(OverloadedMarker {
                    current_weight,
                    max_weight: inventory.max_weight,
                });
            }
            (false, Some(_)) => {
                commands.entity(entity).remove::<OverloadedMarker>();
            }
            (false, None) => {}
        }
    }
}

fn death_drop_seed(entity: Entity, revision: u64) -> u64 {
    entity
        .to_bits()
        .rotate_left(17)
        .wrapping_add(revision.wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

pub(crate) fn select_drop_instance_ids(
    mut instance_ids: Vec<u64>,
    drop_count: usize,
    mut seed: u64,
) -> Vec<u64> {
    for idx in (1..instance_ids.len()).rev() {
        seed = xorshift64(seed);
        let swap_idx = (seed as usize) % (idx + 1);
        instance_ids.swap(idx, swap_idx);
    }
    instance_ids.truncate(drop_count);
    instance_ids
}

fn xorshift64(mut x: u64) -> u64 {
    if x == 0 {
        x = 0x9E37_79B9_7F4A_7C15;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

pub(crate) fn bump_revision(inventory: &mut PlayerInventory) {
    inventory.revision = InventoryRevision(inventory.revision.0.saturating_add(1));
}

/// Returns Some(occupant) if `to` is occupied by another item, None if free.
/// Returns Err if the target is structurally invalid (unknown container, out
/// of bounds, multi-cell overlap that isn't a clean swap candidate).
fn displaced_at_target(
    inventory: &PlayerInventory,
    item: &ItemInstance,
    moving_instance_id: u64,
    location: &crate::schema::inventory::InventoryLocationV1,
) -> Result<Option<ItemInstance>, String> {
    use crate::schema::inventory::InventoryLocationV1;
    match location {
        InventoryLocationV1::Container {
            container_id,
            row,
            col,
        } => {
            let cid = container_id_str(container_id);
            let container = inventory
                .containers
                .iter()
                .find(|c| c.id == cid)
                .ok_or_else(|| format!("unknown container_id '{cid}'"))?;

            let row_u8 = u8::try_from(*row).map_err(|_| format!("row {row} out of u8 range"))?;
            let col_u8 = u8::try_from(*col).map_err(|_| format!("col {col} out of u8 range"))?;
            if u16::from(row_u8) + u16::from(item.grid_h) > u16::from(container.rows)
                || u16::from(col_u8) + u16::from(item.grid_w) > u16::from(container.cols)
            {
                return Err("target rectangle exceeds container bounds".to_string());
            }

            let candidate = PlacedItemState {
                row: row_u8,
                col: col_u8,
                instance: item.clone(),
            };
            // Find ALL items whose footprints overlap the target rectangle,
            // excluding the moving instance itself. If exactly one and its
            // anchor sits at (row,col) with same footprint → swap candidate.
            // Anything else → reject (multi-overlap not supported in v1).
            let mut overlapping = container
                .items
                .iter()
                .filter(|p| {
                    p.instance.instance_id != moving_instance_id
                        && placed_item_footprints_overlap(p, &candidate)
                })
                .collect::<Vec<_>>();
            match overlapping.len() {
                0 => Ok(None),
                1 => {
                    let occ = overlapping.pop().unwrap();
                    if occ.row != row_u8 || occ.col != col_u8 {
                        return Err(format!(
                            "target overlaps instance {} at ({},{}) but anchors mismatch — multi-cell swap not supported",
                            occ.instance.instance_id, occ.row, occ.col
                        ));
                    }
                    Ok(Some(occ.instance.clone()))
                }
                n => Err(format!(
                    "target overlaps {n} items — multi-overlap not supported"
                )),
            }
        }
        // plan-layered-equip-v1 P0.2（决议 #3 拒绝不顶替）— equip 落位不做 swap 顶替；
        // 满 / 占用由 validate_move_semantics / validate_attach_fits 拒绝。恒无 displaced。
        InventoryLocationV1::Equip { .. } => Ok(None),
        InventoryLocationV1::Hotbar { index } => {
            let idx = *index as usize;
            if idx >= inventory.hotbar.len() {
                return Err(format!("hotbar index {idx} out of range"));
            }
            match &inventory.hotbar[idx] {
                None => Ok(None),
                Some(occupant) if occupant.instance_id == moving_instance_id => Ok(None),
                Some(occupant) => Ok(Some(occupant.clone())),
            }
        }
    }
}

/// Validate that {item} would fit at {location} given the current state of the
/// inventory (assumes both swap participants have been detached).
fn validate_attach_fits(
    inventory: &PlayerInventory,
    item: &ItemInstance,
    location: &crate::schema::inventory::InventoryLocationV1,
) -> Result<(), String> {
    use crate::schema::inventory::InventoryLocationV1;
    match location {
        InventoryLocationV1::Container {
            container_id,
            row,
            col,
        } => {
            let cid = container_id_str(container_id);
            let container = inventory
                .containers
                .iter()
                .find(|c| c.id == cid)
                .ok_or_else(|| format!("unknown container_id '{cid}'"))?;
            let row_u8 = u8::try_from(*row).map_err(|_| format!("row {row} out of u8 range"))?;
            let col_u8 = u8::try_from(*col).map_err(|_| format!("col {col} out of u8 range"))?;
            if u16::from(row_u8) + u16::from(item.grid_h) > u16::from(container.rows)
                || u16::from(col_u8) + u16::from(item.grid_w) > u16::from(container.cols)
            {
                return Err("target rectangle exceeds container bounds".to_string());
            }
            let candidate = PlacedItemState {
                row: row_u8,
                col: col_u8,
                instance: item.clone(),
            };
            for existing in &container.items {
                if placed_item_footprints_overlap(existing, &candidate) {
                    return Err(format!(
                        "target overlaps instance {}",
                        existing.instance.instance_id
                    ));
                }
            }
            Ok(())
        }
        InventoryLocationV1::Equip { slot, state } => {
            // plan-layered-equip-v1 P0.2（决议 #3 / #12）— worn 满则拒、held 占则拒。
            use crate::schema::inventory::EquipStateV1;
            let key = equip_slot_key(slot);
            let contents = inventory.equipped.get(key);
            match state {
                EquipStateV1::Worn => {
                    let cur = contents.map(|c| c.worn.len()).unwrap_or(0);
                    if cur as u8 >= worn_cap(key) {
                        return Err(format!("equip slot '{key}' worn 层已满"));
                    }
                }
                EquipStateV1::Held => {
                    if contents.is_some_and(|c| c.held.is_some()) {
                        return Err(format!("equip slot '{key}' held 已占用"));
                    }
                }
            }
            Ok(())
        }
        InventoryLocationV1::Hotbar { index } => {
            let idx = *index as usize;
            if idx >= inventory.hotbar.len() {
                return Err(format!("hotbar index {idx} out of range"));
            }
            if inventory.hotbar[idx].is_some() {
                return Err(format!("hotbar index {idx} occupied"));
            }
            Ok(())
        }
    }
}

fn validate_move_semantics(
    registry: &ItemRegistry,
    inventory: &PlayerInventory,
    item: &ItemInstance,
    from: &crate::schema::inventory::InventoryLocationV1,
    to: &crate::schema::inventory::InventoryLocationV1,
) -> Result<(), String> {
    use crate::schema::inventory::InventoryLocationV1;

    let template = registry
        .get(&item.template_id)
        .ok_or_else(|| format!("unknown item template id `{}`", item.template_id))?;

    // plan-layered-equip-v1 P0.2 / §11.1 #12 — 从 worn 槽移出时只允许栈顶件；
    // 移动被压住的下层 = 拒绝（决议 #12）。从 held 移出无此限制。
    if let InventoryLocationV1::Equip {
        state: crate::schema::inventory::EquipStateV1::Worn,
        ..
    } = from
    {
        if let Some(EquippedInstanceLoc::Worn { slot, index }) =
            find_equipped_instance(inventory, item.instance_id)
        {
            let worn_len = inventory
                .equipped
                .get(&slot)
                .map(|s| s.worn.len())
                .unwrap_or(0);
            if worn_len > 0 && index + 1 != worn_len {
                return Err(
                    "该件被上层压住，请先脱下上层（worn 栈 LIFO，仅栈顶可卸下）".to_string()
                );
            }
        }
    }

    // plan-tarkov-backpack-v1 P0（交付物 #3，决议 #2）— 移除「非空背包拒卸」硬门。
    // 塔科夫式套包：非空背包可连货整体卸下；卸下后内含物由 `handle_inventory_move`
    // 的 worn-pack 卸下分支调 `rebuild_containers_from_equipment` spill 进存活容器，
    // 放不下的 overflow 转掉落物（见 `handle_inventory_move` 红线接线）。
    // 此处不再因 `pack_<instance_id>` 容器非空返回 Err（原 plan-layered-equip-v1 P0.2
    // 决议 #17 的非空拒卸分支已删除）。

    match to {
        InventoryLocationV1::Hotbar { .. } if template.weapon_spec.is_some() => Err(format!(
            "weapon `{}` cannot move to hotbar; weapons must stay in equipped slots",
            item.template_id
        )),
        InventoryLocationV1::Hotbar { .. } if matches!(template.category, ItemCategory::Tool) => {
            Err(format!(
                "tool `{}` cannot move to hotbar; tools must stay in equipped slots",
                item.template_id
            ))
        }
        InventoryLocationV1::Hotbar { .. } if matches!(template.category, ItemCategory::Armor) => {
            Err(format!(
                "armor `{}` cannot move to hotbar; armor must stay in equipped slots",
                item.template_id
            ))
        }
        // plan-shield-block-v1 P0 — 盾牌（Shield 类）同样不能进 hotbar，必须留在 off_hand 槽。
        InventoryLocationV1::Hotbar { .. } if matches!(template.category, ItemCategory::Shield) => {
            Err(format!(
                "shield `{}` cannot move to hotbar; shield must stay in equipped slots",
                item.template_id
            ))
        }
        InventoryLocationV1::Hotbar { .. }
            if matches!(template.category, ItemCategory::Treasure) =>
        {
            Err(format!(
                "treasure `{}` cannot move to hotbar; treasures must stay in equipped slots",
                item.template_id
            ))
        }
        InventoryLocationV1::Hotbar { .. }
            if matches!(template.category, ItemCategory::Container) =>
        {
            Err(format!(
                "container `{}` cannot move to hotbar; containers must stay in equipped slots",
                item.template_id
            ))
        }
        InventoryLocationV1::Equip { slot, state } => {
            // plan-layered-equip-v1 P1 — 是否「从同槽原位移回」（不触发占用拒绝）。
            let from_same_slot = matches!(
                from,
                InventoryLocationV1::Equip { slot: from_slot, .. } if from_slot == slot
            );
            validate_equip_to(
                registry,
                inventory,
                item,
                template,
                slot,
                state,
                from_same_slot,
            )
        }
        // plan-tarkov-backpack-v1 P2（交付物 #2，决议 #2/#5）— 穿戴态门控（server 侧）。
        // 拖入 `pack_<instance_id>` 容器时，校验该背包件当前确实穿戴在某身体槽 worn 层；
        // 背包件已被卸到手持/格子（非穿戴态）后其 `pack_<id>` 容器仍残留于 snapshot，
        // 但不可再被塞入新内含物——塔科夫式语义：卸下的包是「死容器」，重新穿上才能装东西。
        // 非 `pack_<id>` 容器（如 body_pocket / main_pack）放行（保持现状无门控）。
        InventoryLocationV1::Container { container_id, .. } => {
            if let Some(owner_instance_id) = worn_pack_instance_from_container_id(container_id) {
                let owner_is_worn = matches!(
                    find_equipped_instance(inventory, owner_instance_id),
                    Some(EquippedInstanceLoc::Worn { .. })
                );
                if !owner_is_worn {
                    return Err(format!(
                        "背包未穿戴，无法放入内含物：容器 `{container_id}` 的背包件 (instance {owner_instance_id}) \
                         当前不在任何身体槽 worn 层（已卸到手持/格子）；请先穿上该背包再放入物品"
                    ));
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// plan-layered-equip-v1 P1 — 装备到指定槽 + 装备态的校验（决议 #3 拒绝不顶替 / #6 手槽 / #7 双手锁 / #12 worn 栈 / #17 背包身体槽）。
#[allow(clippy::too_many_arguments)]
fn validate_equip_to(
    registry: &ItemRegistry,
    inventory: &PlayerInventory,
    item: &ItemInstance,
    template: &ItemTemplate,
    slot: &crate::schema::inventory::EquipSlotV1,
    state: &crate::schema::inventory::EquipStateV1,
    from_same_slot: bool,
) -> Result<(), String> {
    use crate::combat::weapon::WeaponKind;
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};

    let slot_key = equip_slot_key(slot);
    let is_hand_slot = matches!(
        slot,
        EquipSlotV1::MainHand
            | EquipSlotV1::OffHand
            | EquipSlotV1::ExtraHand0
            | EquipSlotV1::ExtraHand1
    );

    // 手槽 = held-only；身体槽 = worn-only（决议 #6 / #12）。
    match (is_hand_slot, state) {
        (true, EquipStateV1::Worn) => {
            return Err(format!(
                "手槽 {slot_key} 只能持械（held），不能穿戴（worn）"
            ));
        }
        (false, EquipStateV1::Held) => {
            return Err(format!(
                "身体槽 {slot_key} 只能穿戴（worn），不能持械（held）"
            ));
        }
        _ => {}
    }

    match slot {
        EquipSlotV1::MainHand
        | EquipSlotV1::OffHand
        | EquipSlotV1::ExtraHand0
        | EquipSlotV1::ExtraHand1 => {
            // 类型校验：武器 / 工具 / 锄头。off_hand 另接受 Treasure / Shield。
            let is_weapon = template.weapon_spec.is_some();
            let is_tool = matches!(template.category, ItemCategory::Tool)
                || crate::lingtian::hoe::HoeKind::from_item_id(&item.template_id).is_some();
            let off_hand_extra = matches!(slot, EquipSlotV1::OffHand)
                && matches!(
                    template.category,
                    ItemCategory::Treasure | ItemCategory::Shield
                );
            if !is_weapon && !is_tool && !off_hand_extra {
                return Err(format!(
                    "item `{}` cannot equip to {slot_key}; expected weapon, tool, or hoe",
                    item.template_id
                ));
            }
            // off_hand 武器仅 dagger/fist。
            if matches!(slot, EquipSlotV1::OffHand) && is_weapon {
                if let Some(spec) = template.weapon_spec.as_ref() {
                    if !matches!(spec.weapon_kind, WeaponKind::Dagger | WeaponKind::Fist) {
                        return Err(format!(
                            "weapon `{}` cannot equip to off_hand; only dagger/fist are allowed",
                            item.template_id
                        ));
                    }
                }
            }

            // held 互斥（决议 #3）：手槽已持械 → 拒绝（从同槽原位移回除外）。
            if !from_same_slot
                && inventory
                    .equipped
                    .get(slot_key)
                    .is_some_and(|c| c.held.is_some())
            {
                return Err(format!("该手 {slot_key} 已持械，请先卸下"));
            }

            // 双手武器锁对侧手（决议 #7）：main↔off 互锁，extra_hand 独立不锁。
            if let Some(spec) = template.weapon_spec.as_ref() {
                if weapon_two_handed(spec.weapon_kind) {
                    let opposite = match slot {
                        EquipSlotV1::MainHand => Some(EQUIP_SLOT_OFF_HAND),
                        EquipSlotV1::OffHand => Some(EQUIP_SLOT_MAIN_HAND),
                        _ => None,
                    };
                    if let Some(opp) = opposite {
                        if inventory
                            .equipped
                            .get(opp)
                            .is_some_and(|c| c.held.is_some())
                        {
                            return Err("双手兵器占用双手，对侧已被占用，请先卸下".to_string());
                        }
                    }
                }
            }
            // 对侧手已是双手武器 → 本手被锁，拒绝拖入。
            let opposite_holder = match slot {
                EquipSlotV1::MainHand => Some(EQUIP_SLOT_OFF_HAND),
                EquipSlotV1::OffHand => Some(EQUIP_SLOT_MAIN_HAND),
                _ => None,
            };
            if let Some(opp) = opposite_holder {
                let opp_two_handed = inventory
                    .equipped
                    .get(opp)
                    .and_then(|c| c.held.as_ref())
                    .and_then(|held| registry.get(&held.template_id))
                    .and_then(|t| t.weapon_spec.as_ref())
                    .is_some_and(|spec| weapon_two_handed(spec.weapon_kind));
                if opp_two_handed {
                    return Err("双手兵器占用双手，对侧已锁定".to_string());
                }
            }
            Ok(())
        }
        EquipSlotV1::Head | EquipSlotV1::Chest | EquipSlotV1::Legs | EquipSlotV1::Feet => {
            // 身体槽 worn 层：盔甲 / 伪皮 / 背包件（决议 #16 / #17）。
            let is_armor = matches!(template.category, ItemCategory::Armor);
            let is_false_skin =
                crate::combat::tuike::false_skin_kind_for_item(&item.template_id).is_some();
            let is_container = template.container_spec.is_some();
            if !is_armor && !is_false_skin && !is_container {
                return Err(format!(
                    "item `{}` cannot equip to {slot_key}; expected armor / false skin / container",
                    item.template_id
                ));
            }

            if is_armor {
                if item.durability <= 0.0 {
                    return Err(format!(
                        "armor `{}` cannot equip to {slot_key}; durability is 0",
                        item.template_id
                    ));
                }
                let expected_slot = crate::armor::mundane::equip_slot_for_item_id(
                    &item.template_id,
                )
                .ok_or_else(|| {
                    format!(
                        "armor `{}` cannot equip to {slot_key}; unknown armor slot",
                        item.template_id
                    )
                })?;
                if expected_slot != *slot {
                    return Err(format!(
                        "armor `{}` cannot equip to {slot_key}; expected {}",
                        item.template_id,
                        equip_slot_key(&expected_slot)
                    ));
                }
            }

            // 背包件：ContainerSpec.equip_slot 必须指向当前身体槽（决议 #17）。
            if is_container && !is_armor && !is_false_skin {
                if let Some(spec) = template.container_spec.as_ref() {
                    if spec.equip_slot != slot_key {
                        return Err(format!(
                            "item `{}` has container.equip_slot `{}`; cannot equip to {slot_key}",
                            item.template_id, spec.equip_slot,
                        ));
                    }
                }
            }

            // worn cap（决议 #3 / #12 / #17）：满则拒绝（从同槽原位移回除外）。
            if !from_same_slot {
                let cap = worn_cap(slot_key);
                let cur = inventory
                    .equipped
                    .get(slot_key)
                    .map(|c| c.worn.len())
                    .unwrap_or(0);
                if cur as u8 >= cap {
                    return Err(format!("该部位 {slot_key} 已穿戴 {cap} 层，无法再叠加"));
                }
            }
            Ok(())
        }
    }
}

fn location_holds_instance(
    inventory: &PlayerInventory,
    instance_id: u64,
    location: &crate::schema::inventory::InventoryLocationV1,
) -> bool {
    use crate::schema::inventory::InventoryLocationV1;
    match location {
        InventoryLocationV1::Container {
            container_id,
            row,
            col,
        } => {
            let container = match inventory
                .containers
                .iter()
                .find(|c| c.id == container_id_str(container_id))
            {
                Some(c) => c,
                None => return false,
            };
            container.items.iter().any(|p| {
                p.instance.instance_id == instance_id
                    && u64::from(p.row) == *row
                    && u64::from(p.col) == *col
            })
        }
        InventoryLocationV1::Equip { slot, state } => {
            use crate::schema::inventory::EquipStateV1;
            let key = equip_slot_key(slot);
            let Some(contents) = inventory.equipped.get(key) else {
                return false;
            };
            match state {
                EquipStateV1::Worn => contents
                    .worn
                    .iter()
                    .any(|item| item.instance_id == instance_id),
                EquipStateV1::Held => contents
                    .held
                    .as_ref()
                    .is_some_and(|item| item.instance_id == instance_id),
            }
        }
        InventoryLocationV1::Hotbar { index } => {
            let idx = *index as usize;
            if idx >= inventory.hotbar.len() {
                return false;
            }
            inventory.hotbar[idx]
                .as_ref()
                .map(|item| item.instance_id == instance_id)
                .unwrap_or(false)
        }
    }
}

fn clone_item_at(inventory: &PlayerInventory, instance_id: u64) -> Option<ItemInstance> {
    for c in &inventory.containers {
        if let Some(p) = c
            .items
            .iter()
            .find(|p| p.instance.instance_id == instance_id)
        {
            return Some(p.instance.clone());
        }
    }
    for slot in inventory.equipped.values() {
        if let Some(item) = slot.iter_all().find(|item| item.instance_id == instance_id) {
            return Some(item.clone());
        }
    }
    for item in inventory.hotbar.iter().flatten() {
        if item.instance_id == instance_id {
            return Some(item.clone());
        }
    }
    None
}

pub(crate) fn inventory_item_by_instance_mut(
    inventory: &mut PlayerInventory,
    instance_id: u64,
) -> Option<&mut ItemInstance> {
    for container in &mut inventory.containers {
        if let Some(placed) = container
            .items
            .iter_mut()
            .find(|placed| placed.instance.instance_id == instance_id)
        {
            return Some(&mut placed.instance);
        }
    }
    for slot in inventory.equipped.values_mut() {
        if let Some(item) = slot
            .iter_all_mut()
            .find(|item| item.instance_id == instance_id)
        {
            return Some(item);
        }
    }
    inventory
        .hotbar
        .iter_mut()
        .flatten()
        .find(|item| item.instance_id == instance_id)
}

pub(crate) fn inventory_location_by_instance(
    inventory: &PlayerInventory,
    instance_id: u64,
) -> Option<crate::schema::inventory::InventoryLocationV1> {
    use crate::schema::inventory::InventoryLocationV1;

    for container in &inventory.containers {
        if let Some(placed) = container
            .items
            .iter()
            .find(|placed| placed.instance.instance_id == instance_id)
        {
            return Some(InventoryLocationV1::Container {
                container_id: container.id.clone(),
                row: u64::from(placed.row),
                col: u64::from(placed.col),
            });
        }
    }

    // plan-layered-equip-v1 P0.3（gap#5 / #26）— 在 worn/held 定位 instance 推导 state。
    if let Some(loc) = find_equipped_instance(inventory, instance_id) {
        use crate::schema::inventory::EquipStateV1;
        let (slot_key, state) = match loc {
            EquippedInstanceLoc::Worn { slot, .. } => (slot, EquipStateV1::Worn),
            EquippedInstanceLoc::Held { slot } => (slot, EquipStateV1::Held),
        };
        return equip_slot_v1_for_runtime_key(&slot_key)
            .map(|slot| InventoryLocationV1::Equip { slot, state });
    }

    inventory
        .hotbar
        .iter()
        .enumerate()
        .find_map(|(index, item)| {
            item.as_ref()
                .filter(|item| item.instance_id == instance_id)
                .map(|_| InventoryLocationV1::Hotbar { index: index as u8 })
        })
}

pub(crate) fn inventory_location_attrition_exempt(
    inventory: &PlayerInventory,
    registry: &ItemRegistry,
    location: &crate::schema::inventory::InventoryLocationV1,
) -> bool {
    use crate::schema::inventory::InventoryLocationV1;

    let InventoryLocationV1::Container { container_id, .. } = location else {
        return false;
    };
    container_attrition_exempt(inventory, registry, container_id_str(container_id))
}

pub(crate) fn inventory_instance_container_attrition_exempt(
    inventory: &PlayerInventory,
    registry: &ItemRegistry,
    instance_id: u64,
) -> bool {
    inventory_location_by_instance(inventory, instance_id)
        .as_ref()
        .is_some_and(|location| inventory_location_attrition_exempt(inventory, registry, location))
}

/// plan-layered-equip-v1 P0.2（决议 #20）— 容器 id → 穿戴背包件 instance → attrition_exempt。
///
/// 封灵背包搬运跳过 qi 磨损：容器 id（`pack_<instance_id>`）反解出背包件，读其
/// `container_spec.attrition_exempt`。body_pocket / 未知容器恒 false。
fn container_attrition_exempt(
    inventory: &PlayerInventory,
    registry: &ItemRegistry,
    container_id: &str,
) -> bool {
    let Some(instance_id) = worn_pack_instance_from_container_id(container_id) else {
        return false;
    };
    let Some(container_item) = inventory
        .equipped
        .values()
        .flat_map(|slot| slot.worn.iter())
        .find(|item| item.instance_id == instance_id)
    else {
        return false;
    };
    registry
        .get(&container_item.template_id)
        .and_then(|template| template.container_spec.as_ref())
        .is_some_and(|spec| spec.attrition_exempt)
}

fn equip_slot_v1_for_runtime_key(slot: &str) -> Option<crate::schema::inventory::EquipSlotV1> {
    use crate::schema::inventory::EquipSlotV1;

    match slot {
        EQUIP_SLOT_HEAD => Some(EquipSlotV1::Head),
        EQUIP_SLOT_CHEST => Some(EquipSlotV1::Chest),
        EQUIP_SLOT_LEGS => Some(EquipSlotV1::Legs),
        EQUIP_SLOT_FEET => Some(EquipSlotV1::Feet),
        EQUIP_SLOT_MAIN_HAND => Some(EquipSlotV1::MainHand),
        EQUIP_SLOT_OFF_HAND => Some(EquipSlotV1::OffHand),
        EQUIP_SLOT_EXTRA_HAND_0 => Some(EquipSlotV1::ExtraHand0),
        EQUIP_SLOT_EXTRA_HAND_1 => Some(EquipSlotV1::ExtraHand1),
        _ => None,
    }
}

/// plan-layered-equip-v1 P0.2 / §11.1 #12 — 从 equipped 精确移除单件（worn 栈顶 / held），
/// 不整槽删（保留同槽其余 worn 下层 / held）。worn 件即使非栈顶也按 instance 移除——
/// 调用方（move/unequip 路径）已用 `find_equipped_instance` 做 LIFO 栈顶校验。
fn detach_instance(inventory: &mut PlayerInventory, instance_id: u64) {
    for c in &mut inventory.containers {
        c.items.retain(|p| p.instance.instance_id != instance_id);
    }
    for contents in inventory.equipped.values_mut() {
        contents.worn.retain(|item| item.instance_id != instance_id);
        if contents
            .held
            .as_ref()
            .is_some_and(|item| item.instance_id == instance_id)
        {
            contents.held = None;
        }
    }
    // plan-layered-equip-v1 P0.2 — 移出后槽全空则移除空 SlotContents（unequip/move 路径，
    // 使 contains_key 反映槽空）；死亡掉落走独立 iter_mut 路径不经此，保留空槽语义。
    inventory
        .equipped
        .retain(|_, contents| !contents.is_empty());
    for slot in inventory.hotbar.iter_mut() {
        if let Some(item) = slot {
            if item.instance_id == instance_id {
                *slot = None;
            }
        }
    }
}

fn attach_at_location(
    inventory: &mut PlayerInventory,
    item: ItemInstance,
    location: &crate::schema::inventory::InventoryLocationV1,
) -> Result<(), String> {
    use crate::schema::inventory::{EquipStateV1, InventoryLocationV1};
    match location {
        InventoryLocationV1::Container {
            container_id,
            row,
            col,
        } => {
            let cid = container_id_str(container_id);
            let container = inventory
                .containers
                .iter_mut()
                .find(|c| c.id == cid)
                .ok_or_else(|| format!("unknown container_id '{cid}'"))?;
            let row_u8 = u8::try_from(*row).map_err(|_| "row out of range".to_string())?;
            let col_u8 = u8::try_from(*col).map_err(|_| "col out of range".to_string())?;
            container.items.push(PlacedItemState {
                row: row_u8,
                col: col_u8,
                instance: item,
            });
            Ok(())
        }
        InventoryLocationV1::Equip { slot, state } => {
            // plan-layered-equip-v1 P0.2（决议 #2 / #12）— 按 state 写 worn 栈尾（push）或 held。
            let key = equip_slot_key(slot).to_string();
            let contents = inventory.equipped.entry(key).or_default();
            match state {
                EquipStateV1::Worn => contents.worn.push(item),
                EquipStateV1::Held => contents.held = Some(item),
            }
            Ok(())
        }
        InventoryLocationV1::Hotbar { index } => {
            let idx = *index as usize;
            if idx >= inventory.hotbar.len() {
                return Err(format!("hotbar index {idx} out of range"));
            }
            inventory.hotbar[idx] = Some(item);
            Ok(())
        }
    }
}

/// plan-layered-equip-v1 P4（决议 #8）— 法宝激活/卸下到触发位的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreasureActivateOutcome {
    /// 法宝从背包/装备结构移入触发位（激活）。
    Activated { revision: InventoryRevision },
    /// 法宝从触发位卸下，落回背包（失活）。
    Deactivated { revision: InventoryRevision },
}

/// plan-layered-equip-v1 P4（决议 #8）— 在 inventory 与触发位之间移动指定法宝实例。
///
/// - `activate = true`：把 `instance_id` 指向的件移入触发位。
///   - 拒绝：实例不存在 / 模板未注册 / 非 `Treasure` 类 / 触发位已满（`TREASURE_TRIGGER_CAP`）/
///     该件已在触发位。
/// - `activate = false`：把触发位中的该件卸下、落回背包首个能放下的容器格。
///   - 拒绝：该件不在触发位 / 背包无空位（不丢件，原样保留在触发位）。
///
/// 成功时 `bump_revision`。本路径是纯 inventory 结构变更，不走 qi 磨损（移入 UI 承載而非
/// 装备槽搬运）。
pub fn apply_treasure_activate(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    instance_id: u64,
    activate: bool,
) -> Result<TreasureActivateOutcome, String> {
    if activate {
        // 已在触发位 → 幂等拒绝（避免重复移入）。
        if inventory
            .triggered_treasures
            .iter()
            .any(|item| item.instance_id == instance_id)
        {
            return Err(format!(
                "treasure instance {instance_id} already in trigger slot"
            ));
        }
        // 容量满拒绝。
        if inventory.triggered_treasures.len() >= TREASURE_TRIGGER_CAP {
            return Err(format!(
                "trigger slot full ({}/{}), cannot activate instance {instance_id}",
                inventory.triggered_treasures.len(),
                TREASURE_TRIGGER_CAP
            ));
        }
        // 实例必须在 inventory（背包/装备/快捷栏）里。
        let item = inventory_item_by_instance_borrow(inventory, instance_id)
            .cloned()
            .ok_or_else(|| format!("treasure instance {instance_id} not found in inventory"))?;
        // 必须是 Treasure 类（不允许激活普通物品）。
        let is_treasure = registry
            .get(&item.template_id)
            .is_some_and(|tpl| matches!(tpl.category, ItemCategory::Treasure));
        if !is_treasure {
            return Err(format!(
                "instance {instance_id} (template '{}') is not a Treasure, cannot activate",
                item.template_id
            ));
        }
        detach_instance(inventory, instance_id);
        inventory.triggered_treasures.push(item);
        bump_revision(inventory);
        Ok(TreasureActivateOutcome::Activated {
            revision: inventory.revision,
        })
    } else {
        // 卸下：必须在触发位。
        let index = inventory
            .triggered_treasures
            .iter()
            .position(|item| item.instance_id == instance_id)
            .ok_or_else(|| {
                format!("treasure instance {instance_id} not in trigger slot, cannot deactivate")
            })?;
        // 先看背包是否有空位（不真改），无空位则拒绝、不丢件。
        let item = inventory.triggered_treasures[index].clone();
        let Some(target) = find_first_fit_container_location(inventory, &item) else {
            return Err(format!(
                "no free inventory slot to receive deactivated treasure instance {instance_id}"
            ));
        };
        // 有空位 → 从触发位移除并落回背包。
        inventory.triggered_treasures.remove(index);
        attach_at_location(inventory, item, &target)?;
        bump_revision(inventory);
        Ok(TreasureActivateOutcome::Deactivated {
            revision: inventory.revision,
        })
    }
}

fn find_first_fit_container_location(
    inventory: &PlayerInventory,
    item: &ItemInstance,
) -> Option<crate::schema::inventory::InventoryLocationV1> {
    use crate::schema::inventory::InventoryLocationV1;

    // plan-backpack-equip-v1 P1 — ContainerIdV1 is now an open String alias.
    // Scan non-body_pocket containers first (backpacks, pouches, satchels),
    // then fall back to body_pocket so it acts as a last-resort slot.
    for container in inventory
        .containers
        .iter()
        .filter(|c| c.id != BODY_POCKET_CONTAINER_ID)
    {
        let container_id = container.id.clone();
        for row in 0..container.rows {
            for col in 0..container.cols {
                let location = InventoryLocationV1::Container {
                    container_id: container_id.clone(),
                    row: u64::from(row),
                    col: u64::from(col),
                };
                if validate_attach_fits(inventory, item, &location).is_ok() {
                    return Some(location);
                }
            }
        }
    }

    // body_pocket 兜底：只在所有背包/腰囊/挎包都满时才放入贴身口袋。
    for container in inventory
        .containers
        .iter()
        .filter(|c| c.id == BODY_POCKET_CONTAINER_ID)
    {
        let container_id = container.id.clone();
        for row in 0..container.rows {
            for col in 0..container.cols {
                let location = InventoryLocationV1::Container {
                    container_id: container_id.clone(),
                    row: u64::from(row),
                    col: u64::from(col),
                };
                if validate_attach_fits(inventory, item, &location).is_ok() {
                    return Some(location);
                }
            }
        }
    }

    None
}

fn container_id_str(cid: &crate::schema::inventory::ContainerIdV1) -> &str {
    // plan-backpack-equip-v1 P1 — ContainerIdV1 is now String; wire id equals runtime id.
    cid.as_str()
}

fn equip_slot_key(slot: &crate::schema::inventory::EquipSlotV1) -> &'static str {
    use crate::schema::inventory::EquipSlotV1;
    match slot {
        EquipSlotV1::Head => EQUIP_SLOT_HEAD,
        EquipSlotV1::Chest => EQUIP_SLOT_CHEST,
        EquipSlotV1::Legs => EQUIP_SLOT_LEGS,
        EquipSlotV1::Feet => EQUIP_SLOT_FEET,
        EquipSlotV1::MainHand => EQUIP_SLOT_MAIN_HAND,
        EquipSlotV1::OffHand => EQUIP_SLOT_OFF_HAND,
        EquipSlotV1::ExtraHand0 => EQUIP_SLOT_EXTRA_HAND_0,
        EquipSlotV1::ExtraHand1 => EQUIP_SLOT_EXTRA_HAND_1,
    }
}

#[allow(dead_code)]
fn equip_slot_wire_from_runtime(slot: &str) -> crate::schema::inventory::EquipSlotV1 {
    use crate::schema::inventory::EquipSlotV1;

    match slot {
        EQUIP_SLOT_HEAD => EquipSlotV1::Head,
        EQUIP_SLOT_CHEST => EquipSlotV1::Chest,
        EQUIP_SLOT_LEGS => EquipSlotV1::Legs,
        EQUIP_SLOT_FEET => EquipSlotV1::Feet,
        EQUIP_SLOT_MAIN_HAND => EquipSlotV1::MainHand,
        EQUIP_SLOT_OFF_HAND => EquipSlotV1::OffHand,
        EQUIP_SLOT_EXTRA_HAND_0 => EquipSlotV1::ExtraHand0,
        EQUIP_SLOT_EXTRA_HAND_1 => EquipSlotV1::ExtraHand1,
        _ => EquipSlotV1::MainHand,
    }
}

pub(crate) fn placed_item_footprints_overlap(
    left: &PlacedItemState,
    right: &PlacedItemState,
) -> bool {
    let left_row_start = u16::from(left.row);
    let left_row_end = left_row_start + u16::from(left.instance.grid_h);
    let left_col_start = u16::from(left.col);
    let left_col_end = left_col_start + u16::from(left.instance.grid_w);

    let right_row_start = u16::from(right.row);
    let right_row_end = right_row_start + u16::from(right.instance.grid_h);
    let right_col_start = u16::from(right.col);
    let right_col_end = right_col_start + u16::from(right.instance.grid_w);

    left_row_start < right_row_end
        && right_row_start < left_row_end
        && left_col_start < right_col_end
        && right_col_start < left_col_end
}

fn loadout_item_to_instance(
    raw_item: LoadoutPlacedItemToml,
    source_path: &Path,
    registry: &ItemRegistry,
) -> Result<ItemInstance, String> {
    build_item_instance_from_template(
        raw_item.template_id,
        raw_item.stack_count,
        raw_item.spirit_quality,
        raw_item.durability,
        source_path,
        registry,
    )
}

fn build_item_instance_from_template(
    template_id: String,
    stack_count: Option<u32>,
    spirit_quality: Option<f64>,
    durability: Option<f64>,
    source_path: &Path,
    registry: &ItemRegistry,
) -> Result<ItemInstance, String> {
    let template_id = required_non_empty(template_id, source_path, "template_id")?;
    let template = registry.get(template_id.as_str()).ok_or_else(|| {
        format!(
            "{} loadout references unknown template id `{template_id}`",
            source_path.display()
        )
    })?;

    let stack_count = stack_count.unwrap_or(1);
    if stack_count == 0 {
        return Err(format!(
            "{} loadout template `{template_id}` has stack_count=0, expected >= 1",
            source_path.display()
        ));
    }

    let spirit_quality = spirit_quality.unwrap_or(template.spirit_quality_initial);
    if !spirit_quality.is_finite() || !(0.0..=1.0).contains(&spirit_quality) {
        return Err(format!(
            "{} loadout template `{template_id}` has invalid spirit_quality {}; expected 0..=1",
            source_path.display(),
            spirit_quality
        ));
    }

    let durability = durability.unwrap_or(1.0);
    if !durability.is_finite() || !(0.0..=1.0).contains(&durability) {
        return Err(format!(
            "{} loadout template `{template_id}` has invalid durability {}; expected 0..=1",
            source_path.display(),
            durability
        ));
    }

    Ok(ItemInstance {
        instance_id: 0,
        template_id,
        display_name: template.display_name.clone(),
        grid_w: template.grid_w,
        grid_h: template.grid_h,
        weight: template.base_weight,
        rarity: template.rarity,
        description: template.description.clone(),
        stack_count,
        spirit_quality,
        durability,
        freshness: None,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    })
}

fn ensure_required_containers_present(
    containers: &[ContainerState],
    source_path: &Path,
) -> Result<(), String> {
    // plan-backpack-equip-v1 P2 — body_pocket 是唯一始终必须存在的容器；
    // back_pack / waist_pouch / chest_satchel 由装备动态产生，不强制要求。
    let exists = containers
        .iter()
        .any(|container| container.id == BODY_POCKET_CONTAINER_ID);
    if !exists {
        return Err(format!(
            "{} loadout missing required container id `{BODY_POCKET_CONTAINER_ID}`",
            source_path.display()
        ));
    }
    Ok(())
}

fn validate_container_id(id: &str, source_path: &Path) -> Result<(), String> {
    // plan-layered-equip-v1 P0.6（决议 #17）— 容器 id：body_pocket（固定）、`pack_*`（穿戴背包件派生）、
    // 旧 id（历史兼容）。背包专属槽 id（back_pack/waist_pouch/chest_satchel）取消。
    let is_allowed = id == BODY_POCKET_CONTAINER_ID
        || id == LOADOUT_PACK_PLACEHOLDER_CONTAINER_ID
        || worn_pack_instance_from_container_id(id).is_some()
        || [
            MAIN_PACK_CONTAINER_ID,
            SMALL_POUCH_CONTAINER_ID,
            FRONT_SATCHEL_CONTAINER_ID,
        ]
        .contains(&id);

    if is_allowed {
        Ok(())
    } else {
        Err(format!(
            "{} has unsupported container id `{id}`; expected one of \
            [{}, pack_<instance_id>, {}, {}, {}]",
            source_path.display(),
            BODY_POCKET_CONTAINER_ID,
            MAIN_PACK_CONTAINER_ID,
            SMALL_POUCH_CONTAINER_ID,
            FRONT_SATCHEL_CONTAINER_ID,
        ))
    }
}

fn validate_equip_slot(slot: &str, source_path: &Path) -> Result<(), String> {
    // plan-layered-equip-v1 P0.1（决议 #17）— 仅余身体槽 + 手槽。
    let is_allowed = [
        EQUIP_SLOT_HEAD,
        EQUIP_SLOT_CHEST,
        EQUIP_SLOT_LEGS,
        EQUIP_SLOT_FEET,
        EQUIP_SLOT_MAIN_HAND,
        EQUIP_SLOT_OFF_HAND,
        EQUIP_SLOT_EXTRA_HAND_0,
        EQUIP_SLOT_EXTRA_HAND_1,
    ]
    .contains(&slot);

    if is_allowed {
        Ok(())
    } else {
        Err(format!(
            "{} has unsupported equip slot `{slot}`; expected one of [{}, {}, {}, {}, {}, {}, {}, {}]",
            source_path.display(),
            EQUIP_SLOT_HEAD,
            EQUIP_SLOT_CHEST,
            EQUIP_SLOT_LEGS,
            EQUIP_SLOT_FEET,
            EQUIP_SLOT_MAIN_HAND,
            EQUIP_SLOT_OFF_HAND,
            EQUIP_SLOT_EXTRA_HAND_0,
            EQUIP_SLOT_EXTRA_HAND_1
        ))
    }
}

fn required_non_empty(value: String, source_path: &Path, field: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(format!(
            "{} has empty required field `{field}`",
            source_path.display()
        ))
    } else {
        Ok(trimmed.to_string())
    }
}

fn required_non_empty_option(
    value: Option<String>,
    source_path: &Path,
    field: &str,
) -> Result<String, String> {
    match value {
        Some(v) => required_non_empty(v, source_path, field),
        None => Err(format!(
            "{} missing required field `{field}`",
            source_path.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK_ITEM_TEMPLATE_IDS: [&str; 14] = [
        "earth_crumb",
        "hardened_soil",
        "barren_sand",
        "weathered_stone",
        "raw_clay_lump",
        "obsidian_shard",
        "torch_item",
        "lantern_item",
        "door_bolt",
        "window_grate",
        "simple_bed",
        "meditation_mat",
        "moisture_base",
        "spirit_stone_rack",
    ];

    fn test_registry_from_strs(entries: &[(&str, &str)]) -> Result<ItemRegistry, String> {
        let mut templates = HashMap::new();
        for (template_id, display_name) in entries {
            templates.insert(
                (*template_id).to_string(),
                ItemTemplate {
                    id: (*template_id).to_string(),
                    display_name: (*display_name).to_string(),
                    category: ItemCategory::Misc,
                    placeable: None,
                    max_stack_count: 1,
                    grid_w: 1,
                    grid_h: 1,
                    base_weight: 0.1,
                    rarity: ItemRarity::Common,
                    spirit_quality_initial: 1.0,
                    description: "test template".to_string(),
                    effect: None,
                    cast_duration_ms: DEFAULT_CAST_DURATION_MS,
                    cooldown_ms: DEFAULT_COOLDOWN_MS,
                    weapon_spec: None,
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shield_spec: None,

                    shelflife_profile: None,
                    shelflife_track: None,
                },
            );
        }
        Ok(ItemRegistry { templates })
    }

    fn test_template(
        template_id: &str,
        category: ItemCategory,
        grid_w: u8,
        grid_h: u8,
        max_stack_count: u32,
    ) -> ItemTemplate {
        ItemTemplate {
            id: template_id.to_string(),
            display_name: template_id.to_string(),
            category,
            placeable: None,
            max_stack_count,
            grid_w,
            grid_h,
            base_weight: 0.1,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 1.0,
            description: "test template".to_string(),
            effect: None,
            cast_duration_ms: DEFAULT_CAST_DURATION_MS,
            cooldown_ms: DEFAULT_COOLDOWN_MS,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shield_spec: None,

            shelflife_profile: None,
            shelflife_track: None,
        }
    }

    fn raw_item_template_toml(id: &str, category: &str) -> ItemTemplateToml {
        ItemTemplateToml {
            id: id.to_string(),
            placeable: None,
            name: id.to_string(),
            category: category.to_string(),
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.1,
            rarity: "common".to_string(),
            spirit_quality_initial: 0.0,
            description: "test item".to_string(),
            max_stack_count: None,
            effect: None,
            cast_duration_ms: None,
            cooldown_ms: None,
            weapon: None,
            forge_station: None,
            blueprint_scroll: None,
            inscription_scroll: None,
            technique_scroll: None,
            recipe_fragment: None,
            container: None,
            shield_spec: None,
            shelflife_profile: None,
            shelflife_track: None,
        }
    }

    fn registry_from_templates(templates: Vec<ItemTemplate>) -> ItemRegistry {
        ItemRegistry {
            templates: templates
                .into_iter()
                .map(|template| (template.id.clone(), template))
                .collect(),
        }
    }

    #[test]
    fn parse_item_effect_accepts_poison_pill_target() {
        let effect = parse_item_effect(
            ItemEffectToml {
                kind: "poison_pill".to_string(),
                magnitude: 0.0,
                target: Some("poison_pill_qing_lin_man_tuo".to_string()),
                duration_ticks: None,
            },
            Path::new("<inline-items.toml>"),
            "poison_pill_qing_lin_man_tuo",
        )
        .expect("poison_pill effect should parse");

        assert_eq!(
            effect,
            ItemEffect::PoisonPill {
                pill_item_id: "poison_pill_qing_lin_man_tuo".to_string()
            }
        );
    }

    #[test]
    fn parse_item_effect_rejects_poison_pill_missing_or_empty_target() {
        for target in [None, Some("   ".to_string())] {
            let error = parse_item_effect(
                ItemEffectToml {
                    kind: "poison_pill".to_string(),
                    magnitude: 0.0,
                    target,
                    duration_ticks: None,
                },
                Path::new("<inline-items.toml>"),
                "poison_pill_missing_target",
            )
            .expect_err("poison_pill effect without target should fail");

            assert!(
                error.contains("item.effect.target"),
                "expected target validation error, got {error}"
            );
        }
    }

    #[test]
    fn parse_item_effect_rejects_poison_pill_unknown_target() {
        let error = parse_item_effect(
            ItemEffectToml {
                kind: "poison_pill".to_string(),
                magnitude: 0.0,
                target: Some("poison_pill_typo".to_string()),
                duration_ticks: None,
            },
            Path::new("<inline-items.toml>"),
            "poison_pill_unknown_target",
        )
        .expect_err("poison_pill effect should reject unknown target ids");

        assert!(
            error.contains("unknown poison pill target `poison_pill_typo`"),
            "expected poison pill target validation error, got {error}"
        );
    }

    #[test]
    fn parse_item_effect_accepts_wound_heal_missing_target_as_all_wounds() {
        let effect = parse_item_effect(
            ItemEffectToml {
                kind: "wound_heal".to_string(),
                magnitude: 1.0,
                target: None,
                duration_ticks: None,
            },
            Path::new("<inline-items.toml>"),
            "bandage",
        )
        .expect("wound_heal without target should parse as all wounds");

        assert_eq!(
            effect,
            ItemEffect::WoundHeal {
                magnitude: 1.0,
                target: None
            }
        );
    }

    #[test]
    fn parse_item_effect_rejects_wound_heal_blank_target() {
        let error = parse_item_effect(
            ItemEffectToml {
                kind: "wound_heal".to_string(),
                magnitude: 1.0,
                target: Some("   ".to_string()),
                duration_ticks: None,
            },
            Path::new("<inline-items.toml>"),
            "blank_bandage",
        )
        .expect_err("blank wound_heal target should be rejected instead of healing all wounds");

        assert!(
            error.contains("empty target segment"),
            "expected empty wound_heal target validation error, got {error}"
        );
    }

    #[test]
    fn parse_item_effect_rejects_wound_heal_unknown_target() {
        let error = parse_item_effect(
            ItemEffectToml {
                kind: "wound_heal".to_string(),
                magnitude: 1.0,
                target: Some("arm_l/tail".to_string()),
                duration_ticks: None,
            },
            Path::new("<inline-items.toml>"),
            "tail_splint",
        )
        .expect_err("unknown wound_heal body part should be rejected");

        assert!(
            error.contains("unknown target `tail`"),
            "expected unknown wound_heal target validation error, got {error}"
        );
    }

    #[test]
    fn item_effect_new_consumable_variants_serde_roundtrip() {
        for original in [
            ItemEffect::ComposureRestore { magnitude: 0.35 },
            ItemEffect::WoundHeal {
                magnitude: 1.0,
                target: None,
            },
            ItemEffect::WoundHeal {
                magnitude: 2.0,
                target: Some("arm_l/arm_r".to_string()),
            },
        ] {
            let json = serde_json::to_string(&original).expect("new item effect should serialize");
            let parsed: ItemEffect =
                serde_json::from_str(&json).expect("new item effect should deserialize");
            assert_eq!(
                parsed, original,
                "expected serde roundtrip to preserve new consumable effect, json={json}"
            );
        }
    }

    #[test]
    fn item_effect_new_consumable_variants_reject_invalid_json_shape() {
        for json in [
            r#"{"ComposureRestore":{"amount":0.35}}"#,
            r#"{"WoundHeal":{"magnitude":1.0,"target":5}}"#,
            r#"{"WoundHeal":{"target":"arm_l"}}"#,
        ] {
            let error = serde_json::from_str::<ItemEffect>(json)
                .expect_err("invalid new item effect JSON should fail");
            assert!(
                !error.to_string().is_empty(),
                "expected serde error for invalid new item effect JSON, json={json}"
            );
        }
    }

    fn empty_inventory(rows: u8, cols: u8) -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows,
                cols,
                items: Vec::new(),
                owner_instance_id: None,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 99.0,
        }
    }

    fn populated_clear_inventory() -> PlayerInventory {
        let mut inv = empty_inventory(2, 2);
        inv.containers[0].items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(1, "main_item"),
        });
        inv.containers.push(ContainerState {
            id: "side_pack".to_string(),
            name: "侧袋".to_string(),
            rows: 1,
            cols: 1,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: make_test_item_instance(2, "side_item"),
            }],

            owner_instance_id: None,
        });
        inv.hotbar[0] = Some(make_test_item_instance(3, "hotbar_item"));
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(make_test_item_instance(4, "sword")),
        );
        inv
    }

    fn assert_container_has_no_overlaps(container: &ContainerState) {
        for (left_index, left) in container.items.iter().enumerate() {
            for right in container.items.iter().skip(left_index + 1) {
                assert!(
                    !placed_item_footprints_overlap(left, right),
                    "items `{}` and `{}` should not overlap",
                    left.instance.template_id,
                    right.instance.template_id
                );
            }
        }
    }

    #[test]
    fn clear_player_inventory_pack_only_preserves_other_storage() {
        let mut inv = populated_clear_inventory();

        clear_player_inventory(&mut inv, ClearScope::PackOnly);

        assert!(inv.containers[0].items.is_empty());
        assert_eq!(inv.containers[1].items.len(), 1);
        assert!(inv.hotbar[0].is_some());
        assert_eq!(inv.equipped.len(), 1);
        assert_eq!(inv.revision, InventoryRevision(1));
    }

    #[test]
    fn clear_player_inventory_pack_and_hotbar_preserves_equipment() {
        let mut inv = populated_clear_inventory();

        clear_player_inventory(&mut inv, ClearScope::PackAndHotbar);

        assert!(inv
            .containers
            .iter()
            .all(|container| container.items.is_empty()));
        assert!(inv.hotbar.iter().all(Option::is_none));
        assert_eq!(inv.equipped.len(), 1);
        assert_eq!(inv.revision, InventoryRevision(1));
    }

    #[test]
    fn clear_player_inventory_all_removes_equipment() {
        let mut inv = populated_clear_inventory();

        clear_player_inventory(&mut inv, ClearScope::All);

        assert!(inv
            .containers
            .iter()
            .all(|container| container.items.is_empty()));
        assert!(inv.hotbar.iter().all(Option::is_none));
        assert!(inv.equipped.is_empty());
        assert_eq!(inv.revision, InventoryRevision(1));
    }

    // plan-tarkov-backpack-v1 P5 — 背包平衡数值标定 sanity（固化 core.toml 解析正确）。
    // 锁住起手破草包 / 升级小草包的 container_spec + 自重，任何误改数值立即撞红：
    //   · 破草包(worn_grass_pouch)：3×3=9 格，容量 8.0（与 loadout BASE 15+8=23 自洽），自重 0.25。
    //   · 小草包(grass_pouch)：3×3=9 格，容量 10.0（>破草包，差异化升级款），自重 0.3。
    #[test]
    fn grass_pouch_balance_values_parse_from_core_toml() {
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");

        let worn = registry
            .get("worn_grass_pouch")
            .expect("破草包 worn_grass_pouch 必须注册");
        assert!(
            (worn.base_weight - 0.25).abs() < f64::EPSILON,
            "破草包自重应为 0.25（最轻起手款），实际 {}",
            worn.base_weight
        );
        let worn_spec = worn
            .container_spec
            .as_ref()
            .expect("破草包必须有 container_spec");
        assert_eq!(
            (worn_spec.rows, worn_spec.cols),
            (3, 3),
            "破草包应 3×3 grid"
        );
        assert!(
            (worn_spec.weight_capacity - 8.0).abs() < f64::EPSILON,
            "破草包容量应为 8.0（与 loadout BASE 15+8=23 自洽），实际 {}",
            worn_spec.weight_capacity
        );
        assert_eq!(
            worn_spec.equip_slot, EQUIP_SLOT_CHEST,
            "破草包穿 chest 身体槽"
        );

        let pouch = registry
            .get("grass_pouch")
            .expect("小草包 grass_pouch 必须注册");
        let pouch_spec = pouch
            .container_spec
            .as_ref()
            .expect("小草包必须有 container_spec");
        assert!(
            pouch_spec.weight_capacity > worn_spec.weight_capacity,
            "小草包是升级款，容量({})必须大于破草包({})",
            pouch_spec.weight_capacity,
            worn_spec.weight_capacity
        );
        assert!(
            (pouch_spec.weight_capacity - 10.0).abs() < f64::EPSILON,
            "小草包容量应标定为 10.0，实际 {}",
            pouch_spec.weight_capacity
        );

        // 起手 loadout max_weight 必须与 BASE + 破草包容量自洽（防止数值漂移破坏起手负重）。
        let loadout = load_default_loadout(&registry).expect("default loadout 应能加载");
        assert!(
            (loadout.max_weight - (BASE_CARRY_CAPACITY + worn_spec.weight_capacity)).abs()
                < f64::EPSILON,
            "loadout max_weight({}) 应等于 BASE({}) + 破草包容量({})",
            loadout.max_weight,
            BASE_CARRY_CAPACITY,
            worn_spec.weight_capacity
        );
    }

    #[test]
    fn loads_item_registry_from_assets() {
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");
        assert!(registry.len() >= 1);
        assert!(registry.get("starter_talisman").is_some());
        assert!(registry.get("xujie_canxie").is_some());
        assert!(matches!(
            registry.get("life_extension_pill").and_then(|item| item.effect.as_ref()),
            Some(ItemEffect::LifespanExtension {
                years: 10,
                source,
            }) if source == "life_extension_pill"
        ));
        assert!(matches!(
            registry.get("huiyuan_pill").and_then(|item| item.effect.as_ref()),
            Some(ItemEffect::QiRecovery { amount }) if (*amount - 60.0).abs() < f64::EPSILON
        ));
        assert!(matches!(
            registry
                .get("huiyuan_decoction")
                .and_then(|item| item.effect.as_ref()),
            Some(ItemEffect::QiRecovery { amount }) if (*amount - 40.0).abs() < f64::EPSILON
        ));
        assert!(matches!(
            registry
                .get("meridian_salve")
                .and_then(|item| item.effect.as_ref()),
            Some(ItemEffect::MeridianHeal { magnitude, target })
                if (*magnitude - 0.2).abs() < f64::EPSILON && target == "any_meridian"
        ));
        assert!(matches!(
            registry
                .get("meridian_rubbing")
                .and_then(|item| item.effect.as_ref()),
            Some(ItemEffect::MeridianHeal { magnitude, target })
                if (*magnitude - 0.15).abs() < f64::EPSILON && target == "any_meridian"
        ));
        assert!(matches!(
            registry
                .get("qingzhuo_powder")
                .and_then(|item| item.effect.as_ref()),
            Some(ItemEffect::ContaminationCleanse { magnitude })
                if (*magnitude - 0.4).abs() < f64::EPSILON
        ));
        assert!(matches!(
            registry
                .get("anti_gu_powder")
                .and_then(|item| item.effect.as_ref()),
            Some(ItemEffect::ContaminationCleanse { magnitude })
                if (*magnitude - 0.4).abs() < f64::EPSILON
        ));
        assert!(matches!(
            registry
                .get("qi_guide_talisman")
                .and_then(|item| item.effect.as_ref()),
            Some(ItemEffect::FoodRegen {
                bonus_factor,
                duration_ticks,
            }) if (*bonus_factor - 0.30).abs() < f32::EPSILON && *duration_ticks == 36_000
        ));
        assert!(matches!(
            registry
                .get("calming_tea")
                .and_then(|item| item.effect.as_ref()),
            Some(ItemEffect::ComposureRestore { magnitude })
                if (*magnitude - 0.35).abs() < f64::EPSILON
        ));
        assert!(matches!(
            registry.get("bandage").and_then(|item| item.effect.as_ref()),
            Some(ItemEffect::WoundHeal { magnitude, target })
                if (*magnitude - 1.0).abs() < f64::EPSILON && target.is_none()
        ));
        assert!(matches!(
            registry
                .get("arm_splint")
                .and_then(|item| item.effect.as_ref()),
            Some(ItemEffect::WoundHeal {
                magnitude,
                target: Some(target),
            }) if (*magnitude - 2.0).abs() < f64::EPSILON && target == "arm_l/arm_r"
        ));
        assert!(matches!(
            registry
                .get("leg_splint")
                .and_then(|item| item.effect.as_ref()),
            Some(ItemEffect::WoundHeal {
                magnitude,
                target: Some(target),
            }) if (*magnitude - 2.0).abs() < f64::EPSILON && target == "leg_l/leg_r"
        ));
        assert!(matches!(
            registry.get("life_core").and_then(|item| item.effect.as_ref()),
            Some(ItemEffect::LifespanExtension {
                years: 25,
                source,
            }) if source == "collapse_core"
        ));
        assert!(matches!(
            registry
                .get("anti_spirit_pressure_pill")
                .and_then(|item| item.effect.as_ref()),
            Some(ItemEffect::AntiSpiritPressure { duration_ticks }) if *duration_ticks == 36_000
        ));
        assert!(matches!(
            registry.get("spirit_treasure_jizhaojing"),
            Some(ItemTemplate {
                category: ItemCategory::Treasure,
                placeable: None,
                rarity: ItemRarity::Ancient,
                max_stack_count: 1,
                ..
            })
        ));
        assert!(matches!(
            registry
                .get("ling_iron_anvil")
                .and_then(|item| item.forge_station_spec.as_ref()),
            Some(ForgeStationSpec { tier: 2 })
        ));
        assert!(matches!(
            registry
                .get("blueprint_scroll_ling_feng")
                .and_then(|item| item.blueprint_scroll_spec.as_ref()),
            Some(BlueprintScrollSpec { blueprint_id }) if blueprint_id == "ling_feng_v0"
        ));
        assert!(matches!(
            registry
                .get("inscription_scroll_qi_amplify_v0")
                .and_then(|item| item.inscription_scroll_spec.as_ref()),
            Some(InscriptionScrollSpec { inscription_id }) if inscription_id == "qi_amplify_v0"
        ));
        for required in [
            "iron_sword_flawed",
            "qing_feng_sword",
            "qing_feng_sword_flawed",
            "ling_feng_sword",
            "ling_feng_sword_flawed",
            "ling_mu_gun",
            "ling_mu_ban",
            "ling_mu_jing",
            "ling_xia",
            "ling_mu_miao",
            "feng_he_gu",
            "yi_shou_gu",
            "xuan_iron",
            "qing_steel",
        ] {
            assert!(
                registry.get(required).is_some(),
                "forge asset `{required}` must be registered"
            );
        }
        for anqi_item in [
            "anqi_bone_chip",
            "anqi_bone_chip_charged",
            "anqi_yibian_shougu",
            "anqi_yibian_shougu_charged",
            "anqi_lingmu_arrow",
            "anqi_lingmu_arrow_charged",
            "anqi_dyed_bone",
            "anqi_dyed_bone_charged",
            "anqi_fenglinghe_bone",
            "anqi_fenglinghe_bone_charged",
            "anqi_shanggu_bone",
            "anqi_shanggu_bone_charged",
            "anqi_container_quiver",
            "anqi_container_pocket_pouch",
            "anqi_container_fenglinghe",
        ] {
            let template = registry
                .get(anqi_item)
                .unwrap_or_else(|| panic!("anqi asset `{anqi_item}` must be registered"));
            assert!(
                (0.0..=1.0).contains(&template.spirit_quality_initial),
                "anqi asset `{anqi_item}` spirit quality must remain within item registry bounds"
            );
        }
        for required_tool in [
            "cai_yao_dao",
            "bao_chu",
            "cao_lian",
            "dun_qi_jia",
            "gua_dao",
            "gu_hai_qian",
            "bing_jia_shou_tao",
        ] {
            let template = registry
                .get(required_tool)
                .unwrap_or_else(|| panic!("tool asset `{required_tool}` must be registered"));
            assert!(
                matches!(template.category, ItemCategory::Tool),
                "tool asset `{required_tool}` must parse as ItemCategory::Tool"
            );
            assert!(
                template.weapon_spec.is_none(),
                "tool asset `{required_tool}` must not define combat weapon stats"
            );
        }
        assert_eq!(
            registry
                .get("ci_she_hao")
                .expect("herb template should load")
                .max_stack_count,
            64
        );
        assert_eq!(
            registry
                .get("guyuan_pill")
                .expect("pill template should load")
                .max_stack_count,
            16
        );
        assert_eq!(
            registry
                .get("fengling_bone_coin")
                .expect("bone coin template should load")
                .max_stack_count,
            u32::MAX
        );
        assert_eq!(
            registry
                .get("iron_sword")
                .expect("weapon template should load")
                .max_stack_count,
            1
        );
    }

    // ── plan-food-v1 P2 BLOCKER 1：food.toml FoodRegen effect 解析测试 ──

    /// BLOCKER 1 端到端：food.toml → ItemRegistry → ling_guo.effect = FoodRegen{0.20, 48000}
    #[test]
    fn food_toml_ling_guo_has_food_regen_effect() {
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");
        let ling_guo = registry
            .get("food.spirit_fruit.ling_guo")
            .expect("food.toml ling_guo must be registered");
        match &ling_guo.effect {
            Some(ItemEffect::FoodRegen {
                bonus_factor,
                duration_ticks,
            }) => {
                assert!(
                    (bonus_factor - 0.20).abs() < 1e-4,
                    "ling_guo bonus_factor 应=0.20（+20% 修炼速度），实际 {bonus_factor}"
                );
                assert_eq!(
                    *duration_ticks, 48_000u64,
                    "ling_guo duration_ticks 应=48000（2 GAME_DAY），实际 {duration_ticks}"
                );
            }
            other => panic!("ling_guo.effect 应为 FoodRegen{{0.20, 48000}}，实际 {other:?}"),
        }
    }

    /// BLOCKER 1 端到端：food.toml → chen_jiu.effect = FoodRegen{0.15, 36000}
    #[test]
    fn food_toml_chen_jiu_has_food_regen_effect() {
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");
        let chen_jiu = registry
            .get("food.spirit_wine.chen_jiu")
            .expect("food.toml chen_jiu must be registered");
        match &chen_jiu.effect {
            Some(ItemEffect::FoodRegen {
                bonus_factor,
                duration_ticks,
            }) => {
                assert!(
                    (bonus_factor - 0.15).abs() < 1e-4,
                    "chen_jiu bonus_factor 应=0.15（+15% 修炼速度），实际 {bonus_factor}"
                );
                assert_eq!(
                    *duration_ticks, 36_000u64,
                    "chen_jiu duration_ticks 应=36000（1.5 GAME_DAY），实际 {duration_ticks}"
                );
            }
            other => panic!("chen_jiu.effect 应为 FoodRegen{{0.15, 36000}}，实际 {other:?}"),
        }
    }

    /// 凡俗食物（cooked_meat / chen_bing）不挂修炼加速 effect
    #[test]
    fn food_toml_mundane_foods_have_no_cultivation_effect() {
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");
        for mundane in ["food.mundane.cooked_meat", "food.mundane.chen_bing"] {
            let item = registry
                .get(mundane)
                .unwrap_or_else(|| panic!("food.toml {mundane} must be registered"));
            assert!(
                item.effect.is_none(),
                "凡俗食物 `{mundane}` 不应有修炼加速 effect，实际 {:?}",
                item.effect
            );
        }
    }

    /// food_regen 解析：duration_ticks 缺失时应报错
    #[test]
    fn parse_item_effect_food_regen_missing_duration_ticks_returns_error() {
        let err = parse_item_effect(
            ItemEffectToml {
                kind: "food_regen".to_string(),
                magnitude: 0.20,
                target: None,
                duration_ticks: None,
            },
            std::path::Path::new("<test>"),
            "test_food_item",
        )
        .expect_err("food_regen 缺失 duration_ticks 应返回 Err");
        assert!(
            err.contains("duration_ticks"),
            "错误信息应包含 'duration_ticks'，实际: {err}"
        );
    }

    /// food_regen 解析：duration_ticks = 0 时应报错
    #[test]
    fn parse_item_effect_food_regen_zero_duration_ticks_returns_error() {
        let err = parse_item_effect(
            ItemEffectToml {
                kind: "food_regen".to_string(),
                magnitude: 0.20,
                target: None,
                duration_ticks: Some(0),
            },
            std::path::Path::new("<test>"),
            "test_food_item",
        )
        .expect_err("food_regen duration_ticks=0 应返回 Err");
        assert!(
            err.contains("duration_ticks"),
            "错误信息应包含 'duration_ticks'，实际: {err}"
        );
    }

    /// food_regen 解析：合法参数应成功 → FoodRegen{bonus_factor: 0.20, duration_ticks: 48000}
    #[test]
    fn parse_item_effect_food_regen_valid_returns_food_regen() {
        let effect = parse_item_effect(
            ItemEffectToml {
                kind: "food_regen".to_string(),
                magnitude: 0.20,
                target: None,
                duration_ticks: Some(48_000),
            },
            std::path::Path::new("<test>"),
            "test_ling_guo",
        )
        .expect("合法 food_regen 参数应成功解析");
        match effect {
            ItemEffect::FoodRegen {
                bonus_factor,
                duration_ticks,
            } => {
                assert!(
                    (bonus_factor - 0.20).abs() < 1e-4,
                    "bonus_factor 应=0.20，实际 {bonus_factor}"
                );
                assert_eq!(
                    duration_ticks, 48_000,
                    "duration_ticks 应=48000，实际 {duration_ticks}"
                );
            }
            other => panic!("期望 FoodRegen，实际 {other:?}"),
        }
    }

    // ── plan-cultivation-pacing-v1 P2.2：次品修炼丹药模板加载测试 ──

    #[test]
    fn flawed_cultivation_pill_templates_load_from_assets() {
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");

        let flawed_ling_xi = registry
            .get("ling_xi_wan_flawed")
            .expect("ling_xi_wan_flawed template should load from pills.toml");
        assert_eq!(flawed_ling_xi.display_name, "灵息丸（次品）");
        assert_eq!(flawed_ling_xi.category, ItemCategory::Pill);
        assert_eq!(flawed_ling_xi.rarity, ItemRarity::Common);

        let flawed_ju_ling = registry
            .get("ju_ling_dan_flawed")
            .expect("ju_ling_dan_flawed template should load from pills.toml");
        assert_eq!(flawed_ju_ling.display_name, "聚灵丹（次品）");
        assert_eq!(flawed_ju_ling.category, ItemCategory::Pill);
        assert_eq!(flawed_ju_ling.rarity, ItemRarity::Common);
    }

    #[test]
    fn all_eight_cultivation_pill_templates_load_from_assets() {
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");
        let ids = [
            "ling_xi_wan",
            "ju_ling_dan",
            "tong_mai_san",
            "ning_yuan_dan",
            "xi_sui_ye",
            "po_jing_dan",
            "kai_qiao_dan",
            "du_jie_dan",
        ];
        for id in ids {
            assert!(
                registry.get(id).is_some(),
                "cultivation pill template `{id}` should be registered in assets/items/pills.toml"
            );
            let template = registry.get(id).unwrap();
            assert_eq!(
                template.category,
                ItemCategory::Pill,
                "`{id}` should have category Pill"
            );
        }
    }

    #[test]
    fn woliu_scrolls_load_as_combat_technique_templates() {
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");
        let woliu_scrolls = registry
            .templates
            .values()
            .filter(|template| {
                template
                    .technique_scroll_spec
                    .as_ref()
                    .is_some_and(|spec| spec.skill_id.starts_with("woliu."))
            })
            .collect::<Vec<_>>();

        assert_eq!(woliu_scrolls.len(), 11);
        assert!(woliu_scrolls.iter().all(|template| {
            matches!(template.category, ItemCategory::Scroll)
                && template
                    .technique_scroll_spec
                    .as_ref()
                    .is_some_and(|spec| spec.kind == "combat_technique")
        }));
    }

    #[test]
    fn woliu_scroll_skill_ids_are_known_techniques() {
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");
        let ids = registry
            .templates
            .values()
            .filter_map(|template| {
                template
                    .technique_scroll_spec
                    .as_ref()
                    .map(|spec| spec.skill_id.as_str())
            })
            .collect::<Vec<_>>();

        assert_eq!(ids.iter().filter(|id| id.starts_with("woliu.")).count(), 11);
        for id in ids {
            assert!(
                crate::cultivation::known_techniques::technique_definition(id).is_some(),
                "technique scroll references unknown id `{id}`"
            );
        }
    }

    #[test]
    fn item_template_toml_allows_explicit_max_stack_override() {
        let raw: ItemTemplatesToml = toml::from_str(
            r#"
[[item]]
id = "test_powder"
name = "测试粉"
category = "misc"
grid_w = 1
grid_h = 1
base_weight = 0.1
rarity = "common"
spirit_quality_initial = 1.0
description = "测试"
max_stack_count = 7
"#,
        )
        .expect("inline item TOML should parse");

        let template = raw
            .item
            .into_iter()
            .next()
            .expect("fixture should contain one item")
            .try_into_item_template(Path::new("<inline-items.toml>"))
            .expect("explicit max_stack_count should be accepted");

        assert_eq!(template.max_stack_count, 7);
    }

    #[test]
    fn item_template_toml_rejects_zero_max_stack() {
        let raw: ItemTemplatesToml = toml::from_str(
            r#"
[[item]]
id = "bad_powder"
name = "坏粉"
category = "misc"
grid_w = 1
grid_h = 1
base_weight = 0.1
rarity = "common"
spirit_quality_initial = 1.0
description = "测试"
max_stack_count = 0
"#,
        )
        .expect("inline item TOML should parse");

        let error = raw
            .item
            .into_iter()
            .next()
            .expect("fixture should contain one item")
            .try_into_item_template(Path::new("<inline-items.toml>"))
            .expect_err("zero max_stack_count should be rejected");

        assert!(error.contains("invalid max_stack_count 0"));
    }

    #[test]
    fn parse_item_category_accepts_tool_alias() {
        let category = parse_item_category("tool", Path::new("<inline-items.toml>"), "cai_yao_dao")
            .expect("tool category should parse");

        assert_eq!(category, ItemCategory::Tool);
    }

    #[test]
    fn parse_item_category_accepts_armor_aliases() {
        for alias in ["armor", "armour"] {
            let category = parse_item_category(
                alias,
                Path::new("<inline-items.toml>"),
                "armor_bone_chestplate",
            )
            .expect("armor category alias should parse");

            assert_eq!(category, ItemCategory::Armor);
        }
    }

    #[test]
    fn parse_item_category_accepts_block_alias() {
        for raw in ["block", "Block", " block "] {
            let category =
                parse_item_category(raw, Path::new("<inline-items.toml>"), "earth_crumb")
                    .expect("block category alias should parse");

            assert_eq!(category, ItemCategory::Block);
        }
    }

    #[test]
    fn block_category_default_stack_count_is_64() {
        assert_eq!(
            default_max_stack_count_for_category(ItemCategory::Block),
            64
        );
    }

    #[test]
    fn block_material_templates_load_with_block_category_and_default_stack() {
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");

        for template_id in BLOCK_ITEM_TEMPLATE_IDS {
            let template = registry
                .get(template_id)
                .unwrap_or_else(|| panic!("block item `{template_id}` should load"));
            assert_eq!(
                template.category,
                ItemCategory::Block,
                "block item `{template_id}` must use ItemCategory::Block"
            );
            assert_eq!(
                template.max_stack_count, 64,
                "block item `{template_id}` should inherit Block default stack count"
            );
        }
    }

    #[test]
    fn shelter_block_templates_keep_inventory_footprint_and_weight() {
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");
        let cases = [
            ("torch_item", 1, 1, 0.2),
            ("lantern_item", 1, 1, 0.6),
            ("door_bolt", 1, 1, 1.5),
            ("window_grate", 1, 1, 2.0),
            ("simple_bed", 2, 2, 4.0),
            ("meditation_mat", 2, 2, 1.5),
            ("moisture_base", 2, 1, 3.0),
            ("spirit_stone_rack", 1, 1, 1.0),
        ];

        for (template_id, grid_w, grid_h, base_weight) in cases {
            let template = registry
                .get(template_id)
                .unwrap_or_else(|| panic!("shelter block item `{template_id}` should load"));
            assert_eq!(
                (template.grid_w, template.grid_h),
                (grid_w, grid_h),
                "shelter block item `{template_id}` must keep its inventory footprint"
            );
            assert!(
                (template.base_weight - base_weight).abs() < f64::EPSILON,
                "shelter block item `{template_id}` must keep base_weight {base_weight}, got {}",
                template.base_weight
            );
        }
    }

    #[test]
    fn parse_forge_station_spec_accepts_valid_tier() {
        let spec = parse_forge_station_spec(
            ForgeStationSpecToml { tier: 4 },
            Path::new("<inline-items.toml>"),
            "dao_anvil",
        )
        .expect("tier 4 forge station should parse");

        assert_eq!(spec.tier, 4);
    }

    #[test]
    fn parse_forge_station_spec_rejects_invalid_tier() {
        let error = parse_forge_station_spec(
            ForgeStationSpecToml { tier: 0 },
            Path::new("<inline-items.toml>"),
            "bad_anvil",
        )
        .expect_err("tier 0 forge station should fail");

        assert!(error.contains("expected 1..=4"));
    }

    #[test]
    fn parse_blueprint_scroll_spec_accepts_blueprint_id() {
        let spec = parse_blueprint_scroll_spec(
            BlueprintScrollSpecToml {
                blueprint_id: "qing_feng_v0".to_string(),
            },
            Path::new("<inline-items.toml>"),
            "blueprint_scroll_qing_feng",
        )
        .expect("blueprint scroll should parse");

        assert_eq!(spec.blueprint_id, "qing_feng_v0");
    }

    #[test]
    fn parse_blueprint_scroll_spec_rejects_empty_blueprint_id() {
        let error = parse_blueprint_scroll_spec(
            BlueprintScrollSpecToml {
                blueprint_id: " ".to_string(),
            },
            Path::new("<inline-items.toml>"),
            "bad_blueprint_scroll",
        )
        .expect_err("empty blueprint id should fail");

        assert!(error.contains("blueprint_scroll.blueprint_id"));
    }

    #[test]
    fn parse_inscription_scroll_spec_accepts_inscription_id() {
        let spec = parse_inscription_scroll_spec(
            InscriptionScrollSpecToml {
                inscription_id: "sharp_v0".to_string(),
            },
            Path::new("<inline-items.toml>"),
            "inscription_scroll_sharp_v0",
        )
        .expect("inscription scroll should parse");

        assert_eq!(spec.inscription_id, "sharp_v0");
    }

    #[test]
    fn parse_inscription_scroll_spec_rejects_empty_inscription_id() {
        let error = parse_inscription_scroll_spec(
            InscriptionScrollSpecToml {
                inscription_id: " ".to_string(),
            },
            Path::new("<inline-items.toml>"),
            "bad_inscription_scroll",
        )
        .expect_err("empty inscription id should fail");

        assert!(error.contains("inscription_scroll.inscription_id"));
    }

    #[test]
    fn loads_default_loadout_includes_textured_starter_kit() {
        // 默认 loadout 改用有 client PNG 的物品（避免 missing_texture 渲染）。
        // 至少应包含 spirit_grass / ningmai_powder（plan-HUD-v1 起手套件）。
        let registry = load_item_registry().expect("item registry should load");
        let loadout = load_default_loadout(&registry).expect("default loadout should load");

        let all_template_ids: Vec<&str> = loadout
            .containers
            .iter()
            .flat_map(|c| c.items.iter().map(|p| p.instance.template_id.as_str()))
            .chain(
                loadout
                    .equipped
                    .values()
                    .flat_map(|s| s.iter_all())
                    .map(|item| item.template_id.as_str()),
            )
            .chain(
                loadout
                    .hotbar
                    .iter()
                    .flatten()
                    .map(|item| item.template_id.as_str()),
            )
            .collect();

        for required in ["spirit_grass", "ningmai_powder", "guyuan_pill"] {
            assert!(
                all_template_ids.contains(&required),
                "default loadout missing required textured item `{required}`; have: {all_template_ids:?}"
            );
        }
        assert!(
            !all_template_ids.contains(&"niche_base"),
            "niche_base must be granted by spawn coffin, not default loadout"
        );
    }

    #[test]
    fn rejects_unknown_template_in_loadout() {
        let registry = test_registry_from_strs(&[("starter_talisman", "启程护符")])
            .expect("registry fixture should construct");

        let loadout_toml = r#"
max_weight = 40.0

[[containers]]
id = "main_pack"
name = "主背包"
rows = 5
cols = 7

  [[containers.items]]
  row = 0
  col = 0
  template_id = "missing_template"

[[containers]]
id = "small_pouch"
name = "小口袋"
rows = 3
cols = 3

[[containers]]
id = "front_satchel"
name = "前挂包"
rows = 3
cols = 4
"#;

        let parsed: LoadoutToml =
            toml::from_str(loadout_toml).expect("fixture TOML should parse into LoadoutToml");
        let error = parsed
            .try_into_loadout(Path::new("<inline-loadout.toml>"), &registry)
            .expect_err("unknown template id in loadout should fail");

        assert!(error.contains("unknown template id `missing_template`"));
    }

    #[test]
    fn allocator_rejects_values_above_js_safe_integer_max() {
        let mut allocator = InventoryInstanceIdAllocator::new(JS_SAFE_INTEGER_MAX);
        assert_eq!(
            allocator.next_id().expect("max id should be allocatable"),
            JS_SAFE_INTEGER_MAX
        );

        let error = allocator
            .next_id()
            .expect_err("allocator should fail after JS safe integer max");
        assert!(error.contains("exceeds JS safe integer max"));
    }

    #[test]
    fn instantiated_inventory_uses_allocator_ids_within_js_safe_bound() {
        let registry = load_item_registry().expect("item registry should load");
        let loadout = load_default_loadout(&registry).expect("default loadout should load");
        let mut allocator = InventoryInstanceIdAllocator::new(1);

        let player_inventory =
            instantiate_inventory_from_loadout(&loadout, &mut allocator, &registry)
                .expect("inventory should instantiate from loadout");

        assert_eq!(player_inventory.revision, InventoryRevision(1));
        assert_eq!(player_inventory.bone_coins, loadout.bone_coins);
        assert!(
            (player_inventory.max_weight - loadout.max_weight).abs() < f64::EPSILON,
            "expected instantiated max_weight {} to match loadout {}",
            player_inventory.max_weight,
            loadout.max_weight
        );

        for item in player_inventory
            .containers
            .iter()
            .flat_map(|container| container.items.iter().map(|entry| &entry.instance))
            .chain(
                player_inventory
                    .equipped
                    .values()
                    .flat_map(|s| s.iter_all()),
            )
            .chain(player_inventory.hotbar.iter().flatten())
        {
            assert!(item.instance_id <= JS_SAFE_INTEGER_MAX);
            assert!(!item.display_name.trim().is_empty());
        }
    }

    #[test]
    fn loadout_requires_fixed_container_ids() {
        let registry = test_registry_from_strs(&[("starter_talisman", "启程护符")])
            .expect("registry fixture should construct");

        let loadout_toml = r#"
[[containers]]
id = "main_pack"
name = "主背包"
rows = 5
cols = 7

[[containers]]
id = "unknown_pack"
name = "未知"
rows = 3
cols = 3

[[containers]]
id = "front_satchel"
name = "前挂包"
rows = 3
cols = 4
"#;

        let parsed: LoadoutToml =
            toml::from_str(loadout_toml).expect("fixture TOML should parse into LoadoutToml");
        let error = parsed
            .try_into_loadout(Path::new("<inline-loadout.toml>"), &registry)
            .expect_err("unknown container id should fail");

        assert!(error.contains("unsupported container id `unknown_pack`"));
    }

    #[test]
    fn loadout_rejects_duplicate_container_ids_during_parse() {
        let registry = test_registry_from_strs(&[("starter_talisman", "启程护符")])
            .expect("registry fixture should construct");

        let loadout_toml = r#"
[[containers]]
id = "main_pack"
name = "主背包"
rows = 5
cols = 7

[[containers]]
id = "main_pack"
name = "备用主背包"
rows = 4
cols = 6

[[containers]]
id = "small_pouch"
name = "小口袋"
rows = 3
cols = 3

[[containers]]
id = "front_satchel"
name = "前挂包"
rows = 3
cols = 4
"#;

        let parsed: LoadoutToml =
            toml::from_str(loadout_toml).expect("fixture TOML should parse into LoadoutToml");
        let error = parsed
            .try_into_loadout(Path::new("<inline-loadout.toml>"), &registry)
            .expect_err("duplicate container id should fail during parse");

        assert!(error.contains("duplicate container id `main_pack`"));
    }

    #[test]
    fn rejects_placed_item_whose_multicell_footprint_overflows_container_bounds() {
        let mut templates = HashMap::new();
        templates.insert(
            "wide_talisman".to_string(),
            ItemTemplate {
                id: "wide_talisman".to_string(),
                display_name: "阔符".to_string(),
                category: ItemCategory::Misc,
                placeable: None,
                max_stack_count: 1,
                grid_w: 2,
                grid_h: 2,
                base_weight: 0.1,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 1.0,
                description: "test template".to_string(),
                effect: None,
                cast_duration_ms: DEFAULT_CAST_DURATION_MS,
                cooldown_ms: DEFAULT_COOLDOWN_MS,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shield_spec: None,

                shelflife_profile: None,
                shelflife_track: None,
            },
        );
        let registry = ItemRegistry { templates };

        let loadout_toml = r#"
[[containers]]
id = "main_pack"
name = "主背包"
rows = 5
cols = 7

  [[containers.items]]
  row = 4
  col = 6
  template_id = "wide_talisman"

[[containers]]
id = "small_pouch"
name = "小口袋"
rows = 3
cols = 3

[[containers]]
id = "front_satchel"
name = "前挂包"
rows = 3
cols = 4
"#;

        let parsed: LoadoutToml =
            toml::from_str(loadout_toml).expect("fixture TOML should parse into LoadoutToml");
        let error = parsed
            .try_into_loadout(Path::new("<inline-loadout.toml>"), &registry)
            .expect_err("multi-cell footprint overflow should fail");

        assert!(error.contains("footprint overflows"));
    }

    #[test]
    fn rejects_overlapping_multicell_item_footprints_within_container() {
        let mut templates = HashMap::new();
        templates.insert(
            "wide_talisman".to_string(),
            ItemTemplate {
                id: "wide_talisman".to_string(),
                display_name: "阔符".to_string(),
                category: ItemCategory::Misc,
                placeable: None,
                max_stack_count: 1,
                grid_w: 2,
                grid_h: 2,
                base_weight: 0.1,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 1.0,
                description: "test template".to_string(),
                effect: None,
                cast_duration_ms: DEFAULT_CAST_DURATION_MS,
                cooldown_ms: DEFAULT_COOLDOWN_MS,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shield_spec: None,

                shelflife_profile: None,
                shelflife_track: None,
            },
        );
        let registry = ItemRegistry { templates };

        let loadout_toml = r#"
[[containers]]
id = "main_pack"
name = "主背包"
rows = 5
cols = 7

  [[containers.items]]
  row = 0
  col = 0
  template_id = "wide_talisman"

  [[containers.items]]
  row = 1
  col = 1
  template_id = "wide_talisman"

[[containers]]
id = "small_pouch"
name = "小口袋"
rows = 3
cols = 3

[[containers]]
id = "front_satchel"
name = "前挂包"
rows = 3
cols = 4
"#;

        let parsed: LoadoutToml =
            toml::from_str(loadout_toml).expect("fixture TOML should parse into LoadoutToml");
        let error = parsed
            .try_into_loadout(Path::new("<inline-loadout.toml>"), &registry)
            .expect_err("overlapping multi-cell footprints should fail during parse");

        assert!(error.contains("overlaps existing item `wide_talisman`"));
    }

    #[test]
    fn loadout_rejects_spirit_stones_field_in_v1() {
        let loadout_toml = r#"
spirit_stones = 100

[[containers]]
id = "main_pack"
name = "主背包"
rows = 5
cols = 7

[[containers]]
id = "small_pouch"
name = "小口袋"
rows = 3
cols = 3

[[containers]]
id = "front_satchel"
name = "前挂包"
rows = 3
cols = 4
"#;

        let error = toml::from_str::<LoadoutToml>(loadout_toml)
            .expect_err("unknown spirit_stones field should be rejected by deny_unknown_fields")
            .to_string();

        assert!(error.contains("unknown field `spirit_stones`"));
    }

    #[test]
    fn find_free_slot_returns_top_left_for_empty_container() {
        let inventory = empty_inventory(5, 7);
        let main_pack = &inventory.containers[0];

        assert_eq!(find_free_slot(main_pack, 1, 1), Some((0, 0)));
        assert_eq!(find_free_slot(main_pack, 2, 2), Some((0, 0)));
    }

    #[test]
    fn find_free_slot_scans_row_major_and_respects_multicell_bounds() {
        let registry =
            registry_from_templates(vec![test_template("wide", ItemCategory::Misc, 2, 2, 1)]);
        let mut inventory = empty_inventory(3, 3);
        let mut allocator = InventoryInstanceIdAllocator::new(1);

        add_item_to_player_inventory(&mut inventory, &registry, &mut allocator, "wide", 1, 0)
            .expect("first wide item should fit at top-left");

        let main_pack = &inventory.containers[0];
        assert_eq!(
            find_free_slot(main_pack, 1, 1),
            Some((0, 2)),
            "row-major scan should skip the occupied 2x2 footprint"
        );
        assert_eq!(
            find_free_slot(main_pack, 2, 2),
            None,
            "remaining space cannot hold a second 2x2 footprint"
        );
    }

    #[test]
    fn find_free_slot_finds_fragmented_hole_and_returns_none_when_full() {
        let registry =
            registry_from_templates(vec![test_template("one", ItemCategory::Misc, 1, 1, 1)]);
        let mut inventory = empty_inventory(2, 3);
        let mut allocator = InventoryInstanceIdAllocator::new(1);

        for _ in 0..5 {
            add_item_to_player_inventory(&mut inventory, &registry, &mut allocator, "one", 1, 0)
                .expect("first five one-cell items should fit");
        }

        let main_pack = &inventory.containers[0];
        assert_eq!(find_free_slot(main_pack, 1, 1), Some((1, 2)));
        assert_eq!(find_free_slot(main_pack, 2, 2), None);

        add_item_to_player_inventory(&mut inventory, &registry, &mut allocator, "one", 1, 0)
            .expect("last one-cell slot should fit");
        assert_eq!(find_free_slot(&inventory.containers[0], 1, 1), None);
    }

    #[test]
    fn runtime_grant_increments_revision_and_creates_instance() {
        let registry = load_item_registry().expect("item registry should load");
        let loadout = load_default_loadout(&registry).expect("default loadout should load");
        let mut allocator = InventoryInstanceIdAllocator::new(1);
        let mut inventory = instantiate_inventory_from_loadout(&loadout, &mut allocator, &registry)
            .expect("inventory should instantiate from loadout");

        let baseline_revision = inventory.revision;
        let receipt = add_item_to_player_inventory(
            &mut inventory,
            &registry,
            &mut allocator,
            "ci_she_hao",
            2,
            0,
        )
        .expect("runtime inventory grant should succeed for canonical herb");

        assert_eq!(receipt.template_id, "ci_she_hao");
        assert_eq!(receipt.stack_count, 2);
        assert!(receipt.instance_id >= 1);
        assert_eq!(receipt.created_instance_ids, vec![receipt.instance_id]);
        assert!(receipt.merged_instance_ids.is_empty());
        assert_eq!(inventory.revision.0, baseline_revision.0.saturating_add(1));

        // plan-backpack-equip-v1 P2 — 新 loadout 无 main_pack，检查 back_pack（首个非 body_pocket 容器）。
        let primary_pack = inventory
            .containers
            .iter()
            .find(|container| container.id != BODY_POCKET_CONTAINER_ID)
            .expect("primary pack should exist");
        assert!(
            primary_pack
                .items
                .iter()
                .any(|entry| entry.instance.template_id == "ci_she_hao"),
            "runtime grant should materialize in primary pack; got: {:?}",
            primary_pack
                .items
                .iter()
                .map(|p| &p.instance.template_id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn runtime_grant_falls_back_to_body_pocket_when_primary_pack_is_full() {
        let registry =
            registry_from_templates(vec![test_template("one", ItemCategory::Misc, 1, 1, 1)]);
        let mut inventory = empty_inventory(1, 1);
        inventory.containers[0].items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(77, "filler"),
        });
        inventory.containers.push(ContainerState {
            id: BODY_POCKET_CONTAINER_ID.to_string(),
            name: "贴身口袋".to_string(),
            rows: 2,
            cols: 3,
            items: Vec::new(),
            owner_instance_id: None,
        });
        let mut allocator = InventoryInstanceIdAllocator::new(100);

        let receipt =
            add_item_to_player_inventory(&mut inventory, &registry, &mut allocator, "one", 1, 0)
                .expect("body_pocket should receive runtime grant when primary pack is full");

        assert_eq!(
            receipt.created_instance_ids,
            vec![100],
            "expected runtime grant to create instance 100 because allocator starts at 100 and no stack merge is possible, actual {:?}",
            receipt.created_instance_ids
        );
        assert_eq!(
            inventory.containers[0].items.len(),
            1,
            "expected primary pack to keep only the original filler because it was full, actual items {:?}",
            inventory.containers[0]
                .items
                .iter()
                .map(|placed| &placed.instance.template_id)
                .collect::<Vec<_>>()
        );
        let body_pocket = inventory
            .containers
            .iter()
            .find(|container| container.id == BODY_POCKET_CONTAINER_ID)
            .unwrap_or_else(|| {
                panic!(
                    "expected `{BODY_POCKET_CONTAINER_ID}` to exist because fallback grants need a final carried container, actual container ids {:?}",
                    inventory
                        .containers
                        .iter()
                        .map(|container| &container.id)
                        .collect::<Vec<_>>()
                )
            });
        assert_eq!(
            body_pocket.items.len(),
            1,
            "expected body_pocket to receive the grant because primary pack was full, actual items {:?}",
            body_pocket
                .items
                .iter()
                .map(|placed| &placed.instance.template_id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            body_pocket.items[0].instance.template_id, "one",
            "expected body_pocket item template to be `one` because that was the granted template, actual `{}`",
            body_pocket.items[0].instance.template_id
        );
    }

    fn filtered_pack_inventory_fixture() -> (ItemRegistry, PlayerInventory, String, String) {
        let mut rejecting_pack_template =
            test_template("mineral_pack", ItemCategory::Container, 1, 1, 1);
        rejecting_pack_template.container_spec = Some(ContainerSpec {
            rows: 2,
            cols: 2,
            weight_capacity: 10.0,
            equip_slot: EQUIP_SLOT_CHEST.to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: false,
            accept_filter: Some(vec![ContainerAcceptFilter::Category(ItemCategory::Mineral)]),
        });
        let mut accepting_pack_template =
            test_template("general_pack", ItemCategory::Container, 1, 1, 1);
        accepting_pack_template.container_spec = Some(ContainerSpec {
            rows: 2,
            cols: 2,
            weight_capacity: 10.0,
            equip_slot: EQUIP_SLOT_CHEST.to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: false,
            accept_filter: None,
        });
        let registry = registry_from_templates(vec![
            test_template("one", ItemCategory::Misc, 1, 1, 1),
            rejecting_pack_template,
            accepting_pack_template,
        ]);
        let rejecting_pack_item = make_test_item_instance(10, "mineral_pack");
        let accepting_pack_item = make_test_item_instance(20, "general_pack");
        let rejecting_pack_id = container_id_for_worn_pack(rejecting_pack_item.instance_id);
        let accepting_pack_id = container_id_for_worn_pack(accepting_pack_item.instance_id);
        let inventory = PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![
                ContainerState {
                    id: rejecting_pack_id.clone(),
                    name: "矿物袋".to_string(),
                    rows: 2,
                    cols: 2,
                    items: Vec::new(),
                    owner_instance_id: Some(rejecting_pack_item.instance_id),
                },
                ContainerState {
                    id: accepting_pack_id.clone(),
                    name: "通用包".to_string(),
                    rows: 2,
                    cols: 2,
                    items: Vec::new(),
                    owner_instance_id: Some(accepting_pack_item.instance_id),
                },
            ],
            equipped: HashMap::from([(
                EQUIP_SLOT_CHEST.to_string(),
                SlotContents {
                    worn: vec![rejecting_pack_item, accepting_pack_item],
                    held: None,
                },
            )]),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 99.0,
        };

        (registry, inventory, rejecting_pack_id, accepting_pack_id)
    }

    #[test]
    fn runtime_grant_skips_non_body_pack_when_accept_filter_rejects_item() {
        let (registry, mut inventory, rejecting_pack_id, accepting_pack_id) =
            filtered_pack_inventory_fixture();
        let mut allocator = InventoryInstanceIdAllocator::new(200);

        let receipt =
            add_item_to_player_inventory(&mut inventory, &registry, &mut allocator, "one", 1, 0)
                .expect("general pack should receive runtime grant after filtered pack rejects it");

        assert_eq!(
            receipt.created_instance_ids,
            vec![200],
            "expected grant to create instance 200 in accepting pack because rejecting pack filter only accepts minerals, actual {:?}",
            receipt.created_instance_ids
        );
        let rejecting_pack = inventory
            .containers
            .iter()
            .find(|container| container.id == rejecting_pack_id)
            .expect("rejecting pack should still exist");
        assert!(
            rejecting_pack.items.is_empty(),
            "expected rejecting pack to stay empty because its accept_filter rejects `one`, actual items {:?}",
            rejecting_pack
                .items
                .iter()
                .map(|placed| &placed.instance.template_id)
                .collect::<Vec<_>>()
        );
        let accepting_pack = inventory
            .containers
            .iter()
            .find(|container| container.id == accepting_pack_id)
            .expect("accepting pack should still exist");
        assert_eq!(
            accepting_pack.items.len(),
            1,
            "expected accepting pack to receive one granted item because it has no accept_filter, actual items {:?}",
            accepting_pack
                .items
                .iter()
                .map(|placed| &placed.instance.template_id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            accepting_pack.items[0].instance.template_id, "one",
            "expected accepting pack item template to be `one` because that was the granted template, actual `{}`",
            accepting_pack.items[0].instance.template_id
        );
    }

    #[test]
    fn existing_item_grant_skips_non_body_pack_when_accept_filter_rejects_item() {
        let (registry, mut inventory, rejecting_pack_id, accepting_pack_id) =
            filtered_pack_inventory_fixture();

        let mut item = make_test_item_instance(300, "one");
        item.spirit_quality = 0.75;
        add_existing_item_to_player_inventory(&mut inventory, &registry, item)
            .expect("existing item should land in accepting pack after filtered pack rejects it");

        let rejecting_pack = inventory
            .containers
            .iter()
            .find(|container| container.id == rejecting_pack_id)
            .expect("rejecting pack should still exist");
        assert!(
            rejecting_pack.items.is_empty(),
            "expected rejecting pack to stay empty because its accept_filter rejects `one`, actual items {:?}",
            rejecting_pack
                .items
                .iter()
                .map(|placed| &placed.instance.template_id)
                .collect::<Vec<_>>()
        );
        let accepting_pack = inventory
            .containers
            .iter()
            .find(|container| container.id == accepting_pack_id)
            .expect("accepting pack should still exist");
        assert_eq!(
            accepting_pack.items.len(),
            1,
            "expected accepting pack to receive one existing loot item because it has no accept_filter, actual items {:?}",
            accepting_pack
                .items
                .iter()
                .map(|placed| &placed.instance.template_id)
                .collect::<Vec<_>>()
        );
        let placed = &accepting_pack.items[0].instance;
        assert_eq!(
            placed.instance_id, 300,
            "expected existing item grant to preserve caller-allocated instance_id 300, actual {}",
            placed.instance_id
        );
        assert!(
            (placed.spirit_quality - 0.75).abs() < f64::EPSILON,
            "expected existing item grant to preserve spirit_quality 0.75, actual {}",
            placed.spirit_quality
        );
    }

    #[test]
    fn runtime_grant_places_multiple_non_stack_items_without_overlap() {
        let registry =
            registry_from_templates(vec![test_template("stone", ItemCategory::Misc, 1, 1, 1)]);
        let mut inventory = empty_inventory(2, 2);
        let mut allocator = InventoryInstanceIdAllocator::new(1);

        let receipt =
            add_item_to_player_inventory(&mut inventory, &registry, &mut allocator, "stone", 4, 0)
                .expect("four non-stack one-cell items should exactly fill a 2x2 pack");

        assert_eq!(receipt.stack_count, 4);
        let main_pack = &inventory.containers[0];
        let positions: Vec<_> = main_pack
            .items
            .iter()
            .map(|placed| (placed.row, placed.col, placed.instance.stack_count))
            .collect();
        assert_eq!(positions, vec![(0, 0, 1), (0, 1, 1), (1, 0, 1), (1, 1, 1)]);
        assert_container_has_no_overlaps(main_pack);

        let error =
            add_item_to_player_inventory(&mut inventory, &registry, &mut allocator, "stone", 1, 0)
                .expect_err("full pack should reject another non-stack item");
        assert!(error.contains("inventory full: stone"));
    }

    #[test]
    fn runtime_grant_merges_existing_stack_before_allocating_new_slot() {
        let registry = registry_from_templates(vec![test_template(
            "ci_she_hao",
            ItemCategory::Herb,
            1,
            1,
            64,
        )]);
        let mut inventory = empty_inventory(2, 2);
        let mut allocator = InventoryInstanceIdAllocator::new(10);

        add_item_to_player_inventory(
            &mut inventory,
            &registry,
            &mut allocator,
            "ci_she_hao",
            10,
            0,
        )
        .expect("initial herb stack should fit");
        let first_instance_id = inventory.containers[0].items[0].instance.instance_id;

        let receipt = add_item_to_player_inventory(
            &mut inventory,
            &registry,
            &mut allocator,
            "ci_she_hao",
            5,
            0,
        )
        .expect("second herb grant should merge into existing stack");

        assert_eq!(receipt.instance_id, 0);
        assert!(receipt.created_instance_ids.is_empty());
        assert_eq!(receipt.merged_instance_ids, vec![first_instance_id]);
        assert_eq!(inventory.containers[0].items.len(), 1);
        assert_eq!(inventory.containers[0].items[0].instance.stack_count, 15);
    }

    #[test]
    fn runtime_grant_merges_same_block_template_stack() {
        let registry = registry_from_templates(vec![test_template(
            "earth_crumb",
            ItemCategory::Block,
            1,
            1,
            64,
        )]);
        let mut inventory = empty_inventory(2, 2);
        let mut allocator = InventoryInstanceIdAllocator::new(100);

        add_item_to_player_inventory(
            &mut inventory,
            &registry,
            &mut allocator,
            "earth_crumb",
            10,
            0,
        )
        .expect("initial block stack should fit");
        let first_instance_id = inventory.containers[0].items[0].instance.instance_id;

        let receipt = add_item_to_player_inventory(
            &mut inventory,
            &registry,
            &mut allocator,
            "earth_crumb",
            5,
            0,
        )
        .expect("same block template should merge into existing stack");

        assert_eq!(receipt.instance_id, 0);
        assert!(receipt.created_instance_ids.is_empty());
        assert_eq!(receipt.merged_instance_ids, vec![first_instance_id]);
        assert_eq!(inventory.containers[0].items.len(), 1);
        assert_eq!(inventory.containers[0].items[0].instance.stack_count, 15);
    }

    #[test]
    fn runtime_grant_keeps_different_block_templates_in_separate_stacks() {
        let registry = registry_from_templates(vec![
            test_template("earth_crumb", ItemCategory::Block, 1, 1, 64),
            test_template("barren_sand", ItemCategory::Block, 1, 1, 64),
        ]);
        let mut inventory = empty_inventory(2, 2);
        let mut allocator = InventoryInstanceIdAllocator::new(110);

        add_item_to_player_inventory(
            &mut inventory,
            &registry,
            &mut allocator,
            "earth_crumb",
            1,
            0,
        )
        .expect("earth_crumb block stack should fit");
        add_item_to_player_inventory(
            &mut inventory,
            &registry,
            &mut allocator,
            "barren_sand",
            1,
            0,
        )
        .expect("barren_sand block stack should fit");

        let main_pack = &inventory.containers[0];
        assert_eq!(main_pack.items.len(), 2);
        assert_eq!(main_pack.items[0].instance.template_id, "earth_crumb");
        assert_eq!(main_pack.items[0].instance.stack_count, 1);
        assert_eq!(main_pack.items[1].instance.template_id, "barren_sand");
        assert_eq!(main_pack.items[1].instance.stack_count, 1);
    }

    #[test]
    fn runtime_grant_repeated_herb_harvests_merge_into_one_stack() {
        let registry = registry_from_templates(vec![test_template(
            "ci_she_hao",
            ItemCategory::Herb,
            1,
            1,
            64,
        )]);
        let mut inventory = empty_inventory(5, 7);
        let mut allocator = InventoryInstanceIdAllocator::new(30);

        for _ in 0..5 {
            let receipt = add_item_to_player_inventory(
                &mut inventory,
                &registry,
                &mut allocator,
                "ci_she_hao",
                1,
                0,
            )
            .expect("batch herb harvest grant should merge into existing stack");
            if receipt.merged_instance_ids.is_empty() {
                assert_eq!(receipt.created_instance_ids.len(), 1);
            } else {
                assert_eq!(receipt.instance_id, 0);
                assert!(receipt.created_instance_ids.is_empty());
            }
        }

        let main_pack = &inventory.containers[0];
        assert_eq!(main_pack.items.len(), 1);
        assert_eq!(main_pack.items[0].row, 0);
        assert_eq!(main_pack.items[0].col, 0);
        assert_eq!(main_pack.items[0].instance.stack_count, 5);
        assert_eq!(inventory.revision.0, 5);
    }

    #[test]
    fn runtime_grant_caps_stack_and_places_remainder_in_new_slot() {
        let registry = registry_from_templates(vec![test_template(
            "ci_she_hao",
            ItemCategory::Herb,
            1,
            1,
            64,
        )]);
        let mut inventory = empty_inventory(2, 2);
        let mut allocator = InventoryInstanceIdAllocator::new(20);

        add_item_to_player_inventory(
            &mut inventory,
            &registry,
            &mut allocator,
            "ci_she_hao",
            63,
            0,
        )
        .expect("initial herb stack should fit");
        let receipt = add_item_to_player_inventory(
            &mut inventory,
            &registry,
            &mut allocator,
            "ci_she_hao",
            3,
            0,
        )
        .expect("overflow should create a second stack");

        let main_pack = &inventory.containers[0];
        assert_eq!(main_pack.items.len(), 2);
        assert_eq!(main_pack.items[0].instance.stack_count, 64);
        assert_eq!(main_pack.items[1].row, 0);
        assert_eq!(main_pack.items[1].col, 1);
        assert_eq!(main_pack.items[1].instance.stack_count, 2);
        assert_eq!(receipt.instance_id, main_pack.items[1].instance.instance_id);
        assert_eq!(receipt.created_instance_ids, vec![receipt.instance_id]);
        assert_eq!(
            receipt.merged_instance_ids,
            vec![main_pack.items[0].instance.instance_id]
        );
        assert_container_has_no_overlaps(main_pack);
    }

    #[test]
    fn find_mergeable_stack_respects_capacity_boundaries() {
        let registry = registry_from_templates(vec![test_template(
            "ci_she_hao",
            ItemCategory::Herb,
            1,
            1,
            64,
        )]);
        let mut inventory = empty_inventory(2, 2);
        let mut allocator = InventoryInstanceIdAllocator::new(40);

        add_item_to_player_inventory(
            &mut inventory,
            &registry,
            &mut allocator,
            "ci_she_hao",
            1,
            0,
        )
        .expect("initial herb stack should fit");

        assert!(
            find_mergeable_stack(&mut inventory.containers[0], "ci_she_hao", 1).is_none(),
            "max_stack_count=1 must disable stack merging"
        );

        inventory.containers[0].items[0].instance.stack_count = 64;
        assert!(
            find_mergeable_stack(&mut inventory.containers[0], "ci_she_hao", 64).is_none(),
            "full stack must not be mergeable"
        );
    }

    #[test]
    fn runtime_grant_does_not_merge_customized_stack_with_default_grant() {
        let registry = registry_from_templates(vec![test_template(
            "ci_she_hao",
            ItemCategory::Herb,
            1,
            1,
            64,
        )]);
        let mut inventory = empty_inventory(2, 2);
        let mut allocator = InventoryInstanceIdAllocator::new(50);

        add_customized_item_to_player_inventory(
            &mut inventory,
            &registry,
            &mut allocator,
            "ci_she_hao",
            1,
            0,
            |instance| {
                instance.display_name = format!("雷 · {}", instance.display_name);
                instance.spirit_quality = (instance.spirit_quality + 0.1).clamp(0.0, 1.0);
            },
        )
        .expect("customized herb stack should fit");
        let receipt = add_item_to_player_inventory(
            &mut inventory,
            &registry,
            &mut allocator,
            "ci_she_hao",
            1,
            0,
        )
        .expect("default herb should fit beside customized stack");

        let main_pack = &inventory.containers[0];
        assert_eq!(main_pack.items.len(), 2);
        assert!(receipt.merged_instance_ids.is_empty());
        assert_eq!(receipt.created_instance_ids, vec![receipt.instance_id]);
        assert_eq!(main_pack.items[0].instance.stack_count, 1);
        assert_eq!(main_pack.items[1].instance.stack_count, 1);
        assert_ne!(
            main_pack.items[0].instance.display_name,
            main_pack.items[1].instance.display_name
        );
    }

    // ─── apply_inventory_move ───────────────────────────────────────────────

    fn make_test_inventory_with_one_item() -> PlayerInventory {
        let item = ItemInstance {
            instance_id: 42,
            template_id: "rat_tail".to_string(),
            display_name: "噬元鼠尾".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.2,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        };
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(7),
            containers: vec![
                ContainerState {
                    id: MAIN_PACK_CONTAINER_ID.to_string(),
                    name: "主背包".to_string(),
                    rows: 5,
                    cols: 7,
                    items: vec![PlacedItemState {
                        row: 0,
                        col: 0,
                        instance: item,
                    }],

                    owner_instance_id: None,
                },
                ContainerState {
                    id: SMALL_POUCH_CONTAINER_ID.to_string(),
                    name: "小口袋".to_string(),
                    rows: 3,
                    cols: 3,
                    items: Vec::new(),
                    owner_instance_id: None,
                },
                ContainerState {
                    id: FRONT_SATCHEL_CONTAINER_ID.to_string(),
                    name: "前挂包".to_string(),
                    rows: 3,
                    cols: 4,
                    items: Vec::new(),
                    owner_instance_id: None,
                },
            ],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    #[test]
    fn apply_move_grid_to_hotbar_succeeds_and_bumps_revision() {
        use crate::schema::inventory::InventoryLocationV1;
        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        let outcome = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Hotbar { index: 3 },
        )
        .expect("move should succeed");

        assert_eq!(
            outcome,
            InventoryMoveOutcome::Moved {
                revision: InventoryRevision(8)
            }
        );
        assert!(inv.containers[0].items.is_empty());
        assert_eq!(inv.hotbar[3].as_ref().unwrap().instance_id, 42);
    }

    #[test]
    fn apply_move_rejects_when_from_does_not_match() {
        use crate::schema::inventory::InventoryLocationV1;
        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        let result = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            // Wrong from cell.
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 1,
                col: 1,
            },
            &InventoryLocationV1::Hotbar { index: 3 },
        );

        assert!(result.is_err());
        // Inventory unchanged.
        assert_eq!(inv.revision, InventoryRevision(7));
        assert_eq!(inv.containers[0].items.len(), 1);
        assert!(inv.hotbar[3].is_none());
    }

    #[test]
    fn apply_move_swaps_when_target_occupied_with_same_footprint() {
        use crate::schema::inventory::InventoryLocationV1;
        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        // Pre-populate hotbar slot 3 with a 1×1 item.
        inv.hotbar[3] = Some(ItemInstance {
            instance_id: 99,
            template_id: "blocker".to_string(),
            display_name: "占位物".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        });

        let outcome = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Hotbar { index: 3 },
        )
        .expect("swap should succeed");

        assert_eq!(
            outcome,
            InventoryMoveOutcome::Swapped {
                revision: InventoryRevision(8),
                displaced_instance_id: 99,
            }
        );
        // Dragged is now at hotbar(3); displaced is at container(0,0).
        assert_eq!(inv.hotbar[3].as_ref().unwrap().instance_id, 42);
        assert_eq!(inv.containers[0].items.len(), 1);
        assert_eq!(inv.containers[0].items[0].instance.instance_id, 99);
        assert_eq!(inv.containers[0].items[0].row, 0);
        assert_eq!(inv.containers[0].items[0].col, 0);
    }

    #[test]
    fn apply_move_rejects_swap_when_footprints_differ() {
        use crate::schema::inventory::InventoryLocationV1;
        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        // Add a 2×2 occupant at container (2,2).
        inv.containers[0].items.push(PlacedItemState {
            row: 2,
            col: 2,
            instance: ItemInstance {
                instance_id: 200,
                template_id: "big".to_string(),
                display_name: "大物".to_string(),
                grid_w: 2,
                grid_h: 2,
                weight: 0.5,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 1.0,
                durability: 1.0,
                freshness: None,
                mineral_id: None,
                charges: None,
                forge_quality: None,
                forge_color: None,
                forge_side_effects: Vec::new(),
                forge_achieved_tier: None,
                alchemy: None,
                lingering_owner_qi: None,
            },
        });

        // Try to drop 1×1 (#42) onto the 2×2 anchor — overlap, mismatched footprint → reject.
        let result = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 2,
                col: 2,
            },
        );

        assert!(result.is_err());
        assert_eq!(inv.revision, InventoryRevision(7));
        // Both items remain in their original positions.
        assert_eq!(inv.containers[0].items.len(), 2);
    }

    #[test]
    fn apply_move_within_grid_succeeds() {
        use crate::schema::inventory::InventoryLocationV1;
        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        let _ = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 2,
                col: 3,
            },
        )
        .expect("intra-grid move should succeed");

        assert_eq!(inv.containers[0].items.len(), 1);
        let placed = &inv.containers[0].items[0];
        assert_eq!(placed.instance.instance_id, 42);
        assert_eq!(placed.row, 2);
        assert_eq!(placed.col, 3);
    }

    #[test]
    fn apply_move_allows_weapon_to_main_hand() {
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.template_id = "iron_sword".to_string();
        inv.containers[0].items[0].instance.display_name = "铁剑".to_string();
        inv.containers[0].items[0].instance.grid_h = 2;

        let outcome = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Equip {
                slot: EquipSlotV1::MainHand,
                state: crate::schema::inventory::EquipStateV1::Held,
            },
        )
        .expect("weapon should equip to main_hand");

        assert_eq!(
            outcome,
            InventoryMoveOutcome::Moved {
                revision: InventoryRevision(8)
            }
        );
        assert_eq!(
            inv.equipped
                .get(EQUIP_SLOT_MAIN_HAND)
                .and_then(|s| s.held.as_ref())
                .map(|item| item.template_id.as_str()),
            Some("iron_sword")
        );
    }

    #[test]
    fn apply_move_allows_tool_to_main_hand() {
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.template_id = "dun_qi_jia".to_string();
        inv.containers[0].items[0].instance.display_name = "钝气夹".to_string();

        let outcome = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Equip {
                slot: EquipSlotV1::MainHand,
                state: crate::schema::inventory::EquipStateV1::Held,
            },
        )
        .expect("tool should equip to main_hand");

        assert_eq!(
            outcome,
            InventoryMoveOutcome::Moved {
                revision: InventoryRevision(8)
            }
        );
        assert_eq!(
            inv.equipped
                .get(EQUIP_SLOT_MAIN_HAND)
                .and_then(|s| s.held.as_ref())
                .map(|item| item.template_id.as_str()),
            Some("dun_qi_jia")
        );
    }

    #[test]
    fn apply_move_allows_tool_to_off_hand() {
        // 用户反馈：工具双手都要能装。off_hand 现也放行 Tool/Hoe（与 client InventoryEquipRules
        // OFF_HAND 同步）。此前 off_hand 只收 dagger/fist/treasure/shield，工具被拒。
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.template_id = "stone_pickaxe".to_string();
        inv.containers[0].items[0].instance.display_name = "石镐".to_string();

        let outcome = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Equip {
                slot: EquipSlotV1::OffHand,
                state: crate::schema::inventory::EquipStateV1::Held,
            },
        )
        .expect("tool should equip to off_hand");

        assert_eq!(
            outcome,
            InventoryMoveOutcome::Moved {
                revision: InventoryRevision(8)
            }
        );
        assert_eq!(
            inv.equipped
                .get(EQUIP_SLOT_OFF_HAND)
                .and_then(|s| s.held.as_ref())
                .map(|item| item.template_id.as_str()),
            Some("stone_pickaxe")
        );
    }

    #[test]
    fn apply_move_rejects_block_to_main_hand() {
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.template_id = "earth_crumb".to_string();
        inv.containers[0].items[0].instance.display_name = "土屑".to_string();

        let error = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Equip {
                slot: EquipSlotV1::MainHand,
                state: crate::schema::inventory::EquipStateV1::Held,
            },
        )
        .expect_err("block items must not equip to main_hand");

        assert!(
            error.contains("expected weapon, tool, or hoe"),
            "expected main_hand category rejection, got: {error}"
        );
        assert!(!inv.equipped.contains_key(EQUIP_SLOT_MAIN_HAND));
    }

    #[test]
    fn item_registry_loads_all_24_mundane_armor_templates() {
        let registry = load_item_registry().expect("item registry should load");

        for item in crate::armor::mundane::all_mundane_armor_items() {
            let template = registry
                .get(item.item_id().as_str())
                .unwrap_or_else(|| panic!("{} should load from armor.toml", item.item_id()));
            assert_eq!(template.category, ItemCategory::Armor);
            assert_eq!(template.max_stack_count, 1);
        }
    }

    #[test]
    fn item_registry_loads_mundane_armor_unlock_scroll_templates() {
        let registry = load_item_registry().expect("item registry should load");

        for material in crate::armor::mundane::MundaneArmorMaterial::ALL {
            let id = format!("scroll_armor_{}", material.id());
            let template = registry
                .get(id.as_str())
                .unwrap_or_else(|| panic!("{id} should load from armor.toml"));
            assert_eq!(template.category, ItemCategory::Misc);
            assert_eq!(template.grid_w, 1);
            assert_eq!(template.grid_h, 2);
        }
    }

    #[test]
    fn apply_move_allows_mundane_armor_to_matching_slot() {
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.template_id = "armor_bone_chestplate".to_string();
        inv.containers[0].items[0].instance.display_name = "骨甲胸甲".to_string();
        inv.containers[0].items[0].instance.grid_w = 2;
        inv.containers[0].items[0].instance.grid_h = 2;

        let outcome = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Equip {
                slot: EquipSlotV1::Chest,
                state: crate::schema::inventory::EquipStateV1::Worn,
            },
        )
        .expect("chestplate should equip to chest");

        assert_eq!(
            outcome,
            InventoryMoveOutcome::Moved {
                revision: InventoryRevision(8)
            }
        );
        assert_eq!(
            inv.equipped
                .get(EQUIP_SLOT_CHEST)
                .and_then(|s| s.worn.first())
                .map(|item| item.template_id.as_str()),
            Some("armor_bone_chestplate")
        );
    }

    #[test]
    fn apply_move_rejects_mundane_armor_to_wrong_slot() {
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.template_id = "armor_bone_chestplate".to_string();
        inv.containers[0].items[0].instance.display_name = "骨甲胸甲".to_string();

        let error = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Equip {
                slot: EquipSlotV1::Head,
                state: crate::schema::inventory::EquipStateV1::Worn,
            },
        )
        .expect_err("chestplate should not equip to head");

        assert!(error.contains("expected chest"));
    }

    #[test]
    fn apply_move_rejects_broken_armor_unequippable() {
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.template_id = "armor_bone_chestplate".to_string();
        inv.containers[0].items[0].instance.display_name = "骨甲胸甲".to_string();
        inv.containers[0].items[0].instance.durability = 0.0;

        let error = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Equip {
                slot: EquipSlotV1::Chest,
                state: crate::schema::inventory::EquipStateV1::Worn,
            },
        )
        .expect_err("broken armor should be rejected");

        assert!(error.contains("durability is 0"));
    }

    // plan-layered-equip-v1 P0.2（决议 #7）— 两手兵器锁对侧手：staff 在 main_hand held →
    // off_hand 被锁，任何件拖入 off_hand 被拒（two_hand 槽已删，改测对侧锁）。
    #[test]
    fn apply_move_rejects_off_hand_when_main_hand_two_handed() {
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        // off_hand 能接受 dagger；这里用 bone_dagger 验证它依然被对侧锁挡住。
        inv.containers[0].items[0].instance.template_id = "bone_dagger".to_string();
        inv.containers[0].items[0].instance.display_name = "骨刀".to_string();
        inv.containers[0].items[0].instance.grid_w = 1;
        inv.containers[0].items[0].instance.grid_h = 1;
        // 在 main_hand 持双手杖（staff 派生 two-handed），锁住 off_hand。
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(ItemInstance {
                instance_id: 77,
                template_id: "wooden_staff".to_string(),
                display_name: "木杖".to_string(),
                grid_w: 1,
                grid_h: 3,
                weight: 1.2,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 1.0,
                durability: 1.0,
                freshness: None,
                mineral_id: None,
                charges: None,
                forge_quality: None,
                forge_color: None,
                forge_side_effects: Vec::new(),
                forge_achieved_tier: None,
                alchemy: None,
                lingering_owner_qi: None,
            }),
        );

        let error = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Equip {
                slot: EquipSlotV1::OffHand,
                state: crate::schema::inventory::EquipStateV1::Held,
            },
        )
        .expect_err("off_hand should be locked by two-handed weapon in main_hand");

        assert!(
            error.contains("双手兵器占用双手，对侧已锁定"),
            "期望对侧锁定拒绝，实际：{error}"
        );
    }

    #[test]
    fn apply_move_rejects_weapon_to_hotbar() {
        use crate::schema::inventory::InventoryLocationV1;

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.template_id = "iron_sword".to_string();
        inv.containers[0].items[0].instance.display_name = "铁剑".to_string();
        inv.containers[0].items[0].instance.grid_h = 2;

        let error = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Hotbar { index: 0 },
        )
        .expect_err("weapon should be rejected from hotbar");

        assert!(error.contains("cannot move to hotbar"));
    }

    #[test]
    fn apply_move_rejects_tool_to_hotbar() {
        use crate::schema::inventory::InventoryLocationV1;

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.template_id = "cai_yao_dao".to_string();
        inv.containers[0].items[0].instance.display_name = "采药刀".to_string();

        let error = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Hotbar { index: 0 },
        )
        .expect_err("tool should be rejected from hotbar");

        assert!(error.contains("tool `cai_yao_dao` cannot move to hotbar"));
    }

    #[test]
    fn apply_move_rejects_armor_to_hotbar() {
        use crate::schema::inventory::InventoryLocationV1;

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.template_id = "armor_bone_boots".to_string();
        inv.containers[0].items[0].instance.display_name = "骨甲靴".to_string();

        let error = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Hotbar { index: 0 },
        )
        .expect_err("armor should be rejected from hotbar");

        assert!(error.contains("armor `armor_bone_boots` cannot move to hotbar"));
    }

    // plan-shield-block-v1 P0 MAJOR #1 — 盾不能进 hotbar（Shield category 守卫回归）。
    #[test]
    fn apply_move_rejects_shield_to_hotbar() {
        use crate::schema::inventory::InventoryLocationV1;

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.template_id = "wooden_shield".to_string();
        inv.containers[0].items[0].instance.display_name = "木盾".to_string();
        inv.containers[0].items[0].instance.grid_h = 2;

        let error = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Hotbar { index: 0 },
        )
        .expect_err("shield should be rejected from hotbar");

        assert!(
            error.contains("shield `wooden_shield` cannot move to hotbar"),
            "期望错误消息含 'shield `wooden_shield` cannot move to hotbar'，\
             实际消息：{error}"
        );
        assert!(
            error.contains("shield must stay in equipped slots"),
            "期望错误消息含 'shield must stay in equipped slots'，\
             实际消息：{error}"
        );
    }

    #[test]
    fn apply_move_rejects_non_dagger_off_hand_weapon() {
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.template_id = "iron_sword".to_string();
        inv.containers[0].items[0].instance.display_name = "铁剑".to_string();
        inv.containers[0].items[0].instance.grid_h = 2;

        let error = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Equip {
                slot: EquipSlotV1::OffHand,
                state: crate::schema::inventory::EquipStateV1::Held,
            },
        )
        .expect_err("sword should be rejected from off_hand");

        assert!(error.contains("only dagger/fist are allowed"));
    }

    // plan-shield-block-v1 P0 MAJOR #2 — off_hand：无 weapon_spec 的非武器物品（armor）装 off_hand
    // 被拒。plan-layered-equip-v1 统一手槽校验器后，错误消息为「expected weapon, tool, or hoe」
    // （off_hand 仅额外放行 Treasure/Shield，Armor 不在其列），行为（拒绝）不变。
    #[test]
    fn apply_move_rejects_non_weapon_armor_to_off_hand() {
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        // armor_bone_boots：ItemCategory::Armor，无 weapon_spec → 走路径 a 被 ok_or_else 拒
        inv.containers[0].items[0].instance.template_id = "armor_bone_boots".to_string();
        inv.containers[0].items[0].instance.display_name = "骨甲靴".to_string();
        inv.containers[0].items[0].instance.grid_w = 1;
        inv.containers[0].items[0].instance.grid_h = 1;

        let error = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Equip {
                slot: EquipSlotV1::OffHand,
                state: crate::schema::inventory::EquipStateV1::Held,
            },
        )
        .expect_err("armor should be rejected from off_hand (no weapon_spec, not treasure/shield)");

        assert!(
            error.contains("expected weapon, tool, or hoe"),
            "期望错误消息含 'expected weapon, tool, or hoe'（统一手槽校验器拒绝非武器/工具/锄头，\
             Armor 不在 off_hand 额外放行的 Treasure/Shield 之列），实际消息：{error}"
        );
    }

    // plan-layered-equip-v1 P0.2（决议 #7）— 两手兵器装入一手时，对侧手已被占用 → 拒绝
    // （two_hand 槽已删，两手兵器入 main/off held 即锁对侧）。双手杖须装 main_hand
    // （off_hand 仅收 dagger/fist，杖会先撞 dagger/fist 限制，无法触达双手锁分支）。
    #[test]
    fn apply_move_rejects_two_handed_weapon_when_opposite_hand_occupied() {
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        // 待装的双手杖（staff 派生 two-handed），目标 main_hand held。
        inv.containers[0].items[0].instance.template_id = "wooden_staff".to_string();
        inv.containers[0].items[0].instance.display_name = "木杖".to_string();
        inv.containers[0].items[0].instance.grid_h = 3;
        // off_hand 已持 dagger → 双手杖入 main_hand 时对侧（off_hand）被占用，应拒。
        inv.equipped.insert(
            EQUIP_SLOT_OFF_HAND.to_string(),
            SlotContents::held_single(ItemInstance {
                instance_id: 77,
                template_id: "bone_dagger".to_string(),
                display_name: "骨刀".to_string(),
                grid_w: 1,
                grid_h: 1,
                weight: 0.5,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 1.0,
                durability: 1.0,
                freshness: None,
                mineral_id: None,
                charges: None,
                forge_quality: None,
                forge_color: None,
                forge_side_effects: Vec::new(),
                forge_achieved_tier: None,
                alchemy: None,
                lingering_owner_qi: None,
            }),
        );

        let error = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Equip {
                slot: EquipSlotV1::MainHand,
                state: crate::schema::inventory::EquipStateV1::Held,
            },
        )
        .expect_err("two-handed weapon should conflict with occupied opposite hand");

        // 命中双手兵器对侧锁：对侧 off_hand 已被 dagger 占用。
        assert!(
            error.contains("双手兵器占用双手，对侧已被占用"),
            "期望双手兵器对侧占用拒绝，实际：{error}"
        );
    }

    // ============================================================================
    // plan-layered-equip-v1 PR-2 / P1 — 装备校验分层规则 state transition 饱和化
    // （worn cap 满拒 / 被压层拒 / held 占拒 / 锁手拒 / worn+held 共存 / 卸顶后下层成新顶 /
    //  双手占双手 / extra_hand 不锁 / 非双手不锁）。
    // P0 (#736) 已落地 `validate_equip_to` 逻辑；本块锁住每条 state transition 防回归。
    // ============================================================================

    /// 紧凑构造一个装备/校验测试用的 `ItemInstance`（仅设关键字段）。
    fn equip_test_instance(instance_id: u64, template_id: &str) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 1.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    /// 直接断言 `validate_move_semantics`：把 `item`（不依赖 inventory 实存）从 `from` 移到
    /// `to` 应通过 / 拒绝。让我们逐条锁 worn cap / LIFO / held 互斥分支，不必走整条
    /// apply_inventory_move（后者改 inventory，破坏 multi-step 断言）。
    fn validate_equip_result(
        registry: &ItemRegistry,
        inventory: &PlayerInventory,
        item: &ItemInstance,
        from: &crate::schema::inventory::InventoryLocationV1,
        to: &crate::schema::inventory::InventoryLocationV1,
    ) -> Result<(), String> {
        validate_move_semantics(registry, inventory, item, from, to)
    }

    fn container_from() -> crate::schema::inventory::InventoryLocationV1 {
        crate::schema::inventory::InventoryLocationV1::Container {
            container_id: MAIN_PACK_CONTAINER_ID.to_string(),
            row: 0,
            col: 0,
        }
    }

    fn equip_to(
        slot: crate::schema::inventory::EquipSlotV1,
        state: crate::schema::inventory::EquipStateV1,
    ) -> crate::schema::inventory::InventoryLocationV1 {
        crate::schema::inventory::InventoryLocationV1::Equip { slot, state }
    }

    // ---- worn cap 满 → 拒绝（决议 #3 拒绝不顶替）----

    #[test]
    fn validate_chest_worn_cap_full_at_three_rejects_fourth_armor() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
        let registry = load_item_registry().expect("registry");
        let mut inv = make_test_inventory_with_one_item();
        // chest cap = 3：填满 3 件胸甲（不同材质，均映射 Chest 槽）。
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents {
                worn: vec![
                    equip_test_instance(201, "armor_straw_chestplate"),
                    equip_test_instance(202, "armor_bone_chestplate"),
                    equip_test_instance(203, "armor_iron_chestplate"),
                ],
                held: None,
            },
        );
        let fourth = equip_test_instance(204, "armor_bone_chestplate");
        let error = validate_equip_result(
            &registry,
            &inv,
            &fourth,
            &container_from(),
            &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
        )
        .expect_err("chest worn cap is 3; the 4th armor must be rejected");
        assert!(
            error.contains("已穿戴 3 层") && error.contains("无法再叠加"),
            "期望 chest 满 3 层拒绝文案，实际：{error}"
        );
    }

    #[test]
    fn validate_chest_worn_below_cap_accepts_third_armor() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
        let registry = load_item_registry().expect("registry");
        let mut inv = make_test_inventory_with_one_item();
        // chest 已 2 件 → 第 3 件合法（cap=3，边界 off-by-one 正向）。
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents {
                worn: vec![
                    equip_test_instance(201, "armor_straw_chestplate"),
                    equip_test_instance(202, "armor_bone_chestplate"),
                ],
                held: None,
            },
        );
        let third = equip_test_instance(203, "armor_iron_chestplate");
        validate_equip_result(
            &registry,
            &inv,
            &third,
            &container_from(),
            &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
        )
        .expect("chest worn cap is 3; the 3rd armor must be accepted");
    }

    #[test]
    fn validate_head_worn_cap_full_at_two_rejects_third_armor() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
        let registry = load_item_registry().expect("registry");
        let mut inv = make_test_inventory_with_one_item();
        // head cap = 2：填满 2 件头盔。
        inv.equipped.insert(
            EQUIP_SLOT_HEAD.to_string(),
            SlotContents {
                worn: vec![
                    equip_test_instance(301, "armor_straw_helmet"),
                    equip_test_instance(302, "armor_bone_helmet"),
                ],
                held: None,
            },
        );
        let third = equip_test_instance(303, "armor_bone_helmet");
        let error = validate_equip_result(
            &registry,
            &inv,
            &third,
            &container_from(),
            &equip_to(EquipSlotV1::Head, EquipStateV1::Worn),
        )
        .expect_err("head worn cap is 2; the 3rd helmet must be rejected");
        assert!(
            error.contains("已穿戴 2 层"),
            "期望 head 满 2 层拒绝文案，实际：{error}"
        );
    }

    #[test]
    fn validate_feet_worn_cap_full_at_two_rejects_third_armor() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
        let registry = load_item_registry().expect("registry");
        let mut inv = make_test_inventory_with_one_item();
        // feet cap = 2：填满 2 件靴。
        inv.equipped.insert(
            EQUIP_SLOT_FEET.to_string(),
            SlotContents {
                worn: vec![
                    equip_test_instance(401, "armor_straw_boots"),
                    equip_test_instance(402, "armor_bone_boots"),
                ],
                held: None,
            },
        );
        let third = equip_test_instance(403, "armor_bone_boots");
        let error = validate_equip_result(
            &registry,
            &inv,
            &third,
            &container_from(),
            &equip_to(EquipSlotV1::Feet, EquipStateV1::Worn),
        )
        .expect_err("feet worn cap is 2; the 3rd boots must be rejected");
        assert!(
            error.contains("已穿戴 2 层"),
            "期望 feet 满 2 层拒绝文案，实际：{error}"
        );
    }

    // ---- 背包件与盔甲同槽 cap 共算（决议 #17：背包占身体槽 worn 层）----

    #[test]
    fn validate_chest_cap_shared_between_armor_and_backpack() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
        let registry = load_item_registry().expect("registry");
        let mut inv = make_test_inventory_with_one_item();
        // chest 已 2 件甲 → 拖入背包件（worn_grass_pouch，equip_slot=chest）作第 3 件合法。
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents {
                worn: vec![
                    equip_test_instance(501, "armor_straw_chestplate"),
                    equip_test_instance(502, "armor_bone_chestplate"),
                ],
                held: None,
            },
        );
        let pack = equip_test_instance(503, "worn_grass_pouch");
        validate_equip_result(
            &registry,
            &inv,
            &pack,
            &container_from(),
            &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
        )
        .expect("backpack as 3rd worn layer shares chest cap (cap=3) and must be accepted");

        // 再补满到 3 后，第 4 件（无论甲还是包）拒绝——cap 与盔甲/伪皮共算。
        inv.equipped
            .get_mut(EQUIP_SLOT_CHEST)
            .unwrap()
            .worn
            .push(equip_test_instance(503, "worn_grass_pouch"));
        let fourth = equip_test_instance(504, "grass_pouch");
        let error = validate_equip_result(
            &registry,
            &inv,
            &fourth,
            &container_from(),
            &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
        )
        .expect_err("chest cap=3 shared with backpack; the 4th item must be rejected");
        assert!(
            error.contains("已穿戴 3 层"),
            "期望 chest 共算满 3 层拒绝，实际：{error}"
        );
    }

    // ---- held 互斥（决议 #3：手槽已持械拒绝，卸下才换）----

    #[test]
    fn validate_held_mutex_rejects_second_weapon_to_occupied_main_hand() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
        let registry = load_item_registry().expect("registry");
        let mut inv = make_test_inventory_with_one_item();
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(equip_test_instance(601, "iron_sword")),
        );
        let second = equip_test_instance(602, "iron_sword");
        let error = validate_equip_result(
            &registry,
            &inv,
            &second,
            &container_from(),
            &equip_to(EquipSlotV1::MainHand, EquipStateV1::Held),
        )
        .expect_err("main_hand already held; second weapon must be rejected (no swap)");
        assert!(
            error.contains("已持械") && error.contains("请先卸下"),
            "期望 held 互斥拒绝文案，实际：{error}"
        );
    }

    // ---- 双手武器锁对侧手：off_hand→main_hand 反向（补 main→off 之外的方向）----
    // 注：Spear 派生双手由 `weapon_two_handed_per_kind` 单测锁（资产暂无 spear 模板，
    // 实物双手锁集成测用 staff）。本例验证「双手在 off_hand 时反向锁 main_hand」。

    #[test]
    fn validate_two_handed_in_off_hand_locks_main_hand() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
        let registry = load_item_registry().expect("registry");
        let mut inv = make_test_inventory_with_one_item();
        // off_hand 持双手杖（staff 派生双手）→ main_hand 被反向锁。
        inv.equipped.insert(
            EQUIP_SLOT_OFF_HAND.to_string(),
            SlotContents::held_single(equip_test_instance(701, "wooden_staff")),
        );
        // 往 main_hand 拖剑，应被对侧（off_hand）双手锁挡住。
        let sword = equip_test_instance(702, "iron_sword");
        let error = validate_equip_result(
            &registry,
            &inv,
            &sword,
            &container_from(),
            &equip_to(EquipSlotV1::MainHand, EquipStateV1::Held),
        )
        .expect_err("two-handed staff in off_hand must lock main_hand (reverse direction)");
        assert!(
            error.contains("双手兵器占用双手，对侧已锁定"),
            "期望反向双手锁拒绝文案，实际：{error}"
        );
    }

    // ---- 非双手武器不锁对侧手 ----

    #[test]
    fn validate_one_handed_sword_does_not_lock_off_hand() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
        let registry = load_item_registry().expect("registry");
        let mut inv = make_test_inventory_with_one_item();
        // main_hand 持单手剑（Sword 非双手）→ off_hand 不锁。
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(equip_test_instance(801, "iron_sword")),
        );
        let dagger = equip_test_instance(802, "bone_dagger");
        validate_equip_result(
            &registry,
            &inv,
            &dagger,
            &container_from(),
            &equip_to(EquipSlotV1::OffHand, EquipStateV1::Held),
        )
        .expect("single-handed sword must NOT lock off_hand; dagger to off_hand should pass");
    }

    // ---- extra_hand 独立不受双手锁（决议 #6/#7：多臂额外手）----

    #[test]
    fn validate_two_handed_main_hand_does_not_lock_extra_hand() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
        let registry = load_item_registry().expect("registry");
        let mut inv = make_test_inventory_with_one_item();
        // main_hand 持双手杖 → off_hand 被锁，但 extra_hand_0 不受锁。
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(equip_test_instance(901, "wooden_staff")),
        );
        let tool = equip_test_instance(902, "bone_dagger");
        validate_equip_result(
            &registry,
            &inv,
            &tool,
            &container_from(),
            &equip_to(EquipSlotV1::ExtraHand0, EquipStateV1::Held),
        )
        .expect(
            "extra_hand_0 is an independent multi-arm slot; two-handed weapon must NOT lock it",
        );
    }

    // ---- worn + held 共存：身体槽 worn 满 + 手槽 held 一件并存合法 ----

    #[test]
    fn validate_worn_and_held_coexist_in_separate_slots() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
        let registry = load_item_registry().expect("registry");
        let mut inv = make_test_inventory_with_one_item();
        // chest worn 已满 3 件 + main_hand 已 held 一把剑 —— 互不干扰。
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents {
                worn: vec![
                    equip_test_instance(1001, "armor_straw_chestplate"),
                    equip_test_instance(1002, "armor_bone_chestplate"),
                    equip_test_instance(1003, "armor_iron_chestplate"),
                ],
                held: None,
            },
        );
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(equip_test_instance(1004, "iron_sword")),
        );
        // off_hand 仍空 → 拖入 dagger 合法（worn 满不影响其它槽 held）。
        let dagger = equip_test_instance(1005, "bone_dagger");
        validate_equip_result(
            &registry,
            &inv,
            &dagger,
            &container_from(),
            &equip_to(EquipSlotV1::OffHand, EquipStateV1::Held),
        )
        .expect("full chest worn + main_hand held must not block off_hand held");
    }

    // ---- 卸下后可再装：held 卸下 → 同手可装新 held ----

    #[test]
    fn validate_rehome_held_then_equip_new_held_succeeds() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
        let registry = load_item_registry().expect("registry");
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items.clear();
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(equip_test_instance(1101, "iron_sword")),
        );
        // 卸下 main_hand 武器（rehome 到容器）。
        move_equipped_item_to_first_container_slot(&mut inv, 1101)
            .expect("held weapon should unequip and rehome");
        assert!(
            inv.equipped
                .get(EQUIP_SLOT_MAIN_HAND)
                .map(|s| s.held.is_none())
                .unwrap_or(true),
            "卸下后 main_hand.held 应为空"
        );
        // 卸下后同手可装新武器。
        let new_sword = equip_test_instance(1102, "iron_sword");
        validate_equip_result(
            &registry,
            &inv,
            &new_sword,
            &InventoryLocationV1::Container {
                container_id: inv.containers[0].id.clone(),
                row: 0,
                col: 0,
            },
            &equip_to(EquipSlotV1::MainHand, EquipStateV1::Held),
        )
        .expect("after unequip, main_hand is free and a new weapon must be accepted");
    }

    // ============================================================================
    // plan-layered-equip-v1 PR-2 / P1 — worn 栈 LIFO（决议 #12：仅栈顶可卸下）
    // ============================================================================

    #[test]
    fn validate_move_worn_top_layer_out_succeeds() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
        let registry = load_item_registry().expect("registry");
        let mut inv = make_test_inventory_with_one_item();
        // chest worn = [底甲 1201, 顶甲 1202]；移出栈顶 1202 → 合法。
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents {
                worn: vec![
                    equip_test_instance(1201, "armor_bone_chestplate"),
                    equip_test_instance(1202, "armor_iron_chestplate"),
                ],
                held: None,
            },
        );
        let top = equip_test_instance(1202, "armor_iron_chestplate");
        validate_equip_result(
            &registry,
            &inv,
            &top,
            &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
            &container_from(),
        )
        .expect("moving the worn stack top (worn.last()) out must be allowed");
    }

    #[test]
    fn validate_move_buried_worn_layer_out_rejected() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
        let registry = load_item_registry().expect("registry");
        let mut inv = make_test_inventory_with_one_item();
        // chest worn = [底甲 1301, 顶甲 1302]；移出被压住的底层 1301 → 拒绝。
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents {
                worn: vec![
                    equip_test_instance(1301, "armor_bone_chestplate"),
                    equip_test_instance(1302, "armor_iron_chestplate"),
                ],
                held: None,
            },
        );
        let buried = equip_test_instance(1301, "armor_bone_chestplate");
        let error = validate_equip_result(
            &registry,
            &inv,
            &buried,
            &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
            &container_from(),
        )
        .expect_err("moving a buried worn layer (not worn.last()) must be rejected");
        assert!(
            error.contains("被上层压住") && error.contains("worn 栈 LIFO"),
            "期望被压层 LIFO 拒绝文案，实际：{error}"
        );
    }

    #[test]
    fn move_equipped_top_worn_layer_succeeds_buried_rejected_then_new_top() {
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items.clear();
        // chest worn = [底甲 1401, 中甲 1402, 顶甲 1403]。
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents {
                worn: vec![
                    equip_test_instance(1401, "armor_straw_chestplate"),
                    equip_test_instance(1402, "armor_bone_chestplate"),
                    equip_test_instance(1403, "armor_iron_chestplate"),
                ],
                held: None,
            },
        );

        // 脱被压住的底层（1401）→ 拒绝。
        let err = move_equipped_item_to_first_container_slot(&mut inv, 1401)
            .expect_err("buried bottom layer must not be removable");
        assert!(err.contains("被上层压住"), "期望底层被压拒绝，实际：{err}");

        // 脱栈顶（1403）→ 成功，剩 [1401, 1402]，1402 成新顶。
        move_equipped_item_to_first_container_slot(&mut inv, 1403)
            .expect("stack top must be removable");
        let worn = &inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn;
        assert_eq!(
            worn.iter().map(|i| i.instance_id).collect::<Vec<_>>(),
            vec![1401, 1402],
            "脱顶后 worn 应剩底+中两件，顶层移除"
        );

        // 脱新顶（1402）→ 成功，剩 [1401]，1401 成新顶。
        move_equipped_item_to_first_container_slot(&mut inv, 1402)
            .expect("the new stack top (former middle layer) must now be removable");
        let worn = &inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn;
        assert_eq!(
            worn.iter().map(|i| i.instance_id).collect::<Vec<_>>(),
            vec![1401],
            "脱新顶后 worn 应只剩底层（曾被压住，现成新顶）"
        );

        // 脱最后一层（1401，现唯一一层即栈顶）→ 成功，chest 槽清空。
        move_equipped_item_to_first_container_slot(&mut inv, 1401)
            .expect("last remaining worn layer is the top and must be removable");
        assert!(
            inv.equipped
                .get(EQUIP_SLOT_CHEST)
                .map(|s| s.worn.is_empty())
                .unwrap_or(true),
            "脱完所有层后 chest worn 应为空"
        );
    }

    #[test]
    fn set_item_instance_durability_updates_equipped_item_and_bumps_revision() {
        let mut inv = make_test_inventory_with_one_item();
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(ItemInstance {
                instance_id: 88,
                template_id: "iron_sword".to_string(),
                display_name: "铁剑".to_string(),
                grid_w: 1,
                grid_h: 2,
                weight: 1.2,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 1.0,
                durability: 1.0,
                freshness: None,
                mineral_id: None,
                charges: None,
                forge_quality: None,
                forge_color: None,
                forge_side_effects: Vec::new(),
                forge_achieved_tier: None,
                alchemy: None,
                lingering_owner_qi: None,
            }),
        );

        let update = set_item_instance_durability(&mut inv, 88, 0.25)
            .expect("durability update should succeed");

        assert_eq!(update.revision, InventoryRevision(8));
        assert_eq!(
            inv.equipped[EQUIP_SLOT_MAIN_HAND]
                .held
                .as_ref()
                .unwrap()
                .durability,
            0.25
        );
    }

    #[test]
    fn move_equipped_item_to_first_container_slot_unequips_and_rehomes_item() {
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items.clear();
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(ItemInstance {
                instance_id: 88,
                template_id: "iron_sword".to_string(),
                display_name: "铁剑".to_string(),
                grid_w: 1,
                grid_h: 2,
                weight: 1.2,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 1.0,
                durability: 0.0,
                freshness: None,
                mineral_id: None,
                charges: None,
                forge_quality: None,
                forge_color: None,
                forge_side_effects: Vec::new(),
                forge_achieved_tier: None,
                alchemy: None,
                lingering_owner_qi: None,
            }),
        );

        let outcome = move_equipped_item_to_first_container_slot(&mut inv, 88)
            .expect("broken weapon should move back to container");

        assert_eq!(
            outcome,
            InventoryMoveOutcome::Moved {
                revision: InventoryRevision(8)
            }
        );
        assert!(
            inv.equipped
                .get(EQUIP_SLOT_MAIN_HAND)
                .map(|s| s.is_empty())
                .unwrap_or(true),
            "解装后 main_hand 应为空（held=None）"
        );
        assert_eq!(inv.containers[0].items.len(), 1);
        assert_eq!(inv.containers[0].items[0].instance.instance_id, 88);
    }

    #[test]
    fn consume_item_instance_once_decrements_stack_and_bumps_revision() {
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.stack_count = 3;

        let out = consume_item_instance_once(&mut inv, 42).expect("consume should succeed");

        assert_eq!(out.remaining_stack, 2);
        assert_eq!(out.revision, InventoryRevision(8));
        assert_eq!(inv.containers[0].items[0].instance.stack_count, 2);
    }

    #[test]
    fn consume_item_instance_once_removes_last_stack_and_bumps_revision() {
        let mut inv = make_test_inventory_with_one_item();

        let out = consume_item_instance_once(&mut inv, 42).expect("consume should succeed");

        assert_eq!(out.remaining_stack, 0);
        assert_eq!(out.revision, InventoryRevision(8));
        assert!(inv.containers[0].items.is_empty());
    }

    #[test]
    fn exchange_inventory_items_swaps_items_and_bumps_both_revisions() {
        let mut left = make_test_inventory_with_one_item();
        let mut right = make_test_inventory_with_one_item();
        right.revision = InventoryRevision(3);
        right.containers[0].items[0].row = 1;
        right.containers[0].items[0].col = 1;
        right.containers[0].items[0].instance.instance_id = 99;
        right.containers[0].items[0].instance.display_name = "右物".to_string();

        let outcome = exchange_inventory_items(&mut left, 42, &mut right, 99)
            .expect("exchange should succeed");

        assert_eq!(outcome.left_revision, InventoryRevision(8));
        assert_eq!(outcome.right_revision, InventoryRevision(4));
        assert!(inventory_item_by_instance(&left, 42).is_none());
        assert!(inventory_item_by_instance(&right, 99).is_none());
        assert!(inventory_item_by_instance(&left, 99).is_some());
        assert!(inventory_item_by_instance(&right, 42).is_some());
    }

    #[test]
    fn exchange_inventory_items_rejects_without_room_and_keeps_both_unchanged() {
        let mut left = make_test_inventory_with_one_item();
        left.containers.truncate(1);
        left.containers[0].cols = 1;
        left.containers[0].rows = 1;
        let original_left = left.clone();
        let mut right = make_test_inventory_with_one_item();
        right.containers[0].items[0].instance.instance_id = 99;
        right.containers[0].items[0].instance.grid_w = 2;
        right.containers[0].items[0].instance.grid_h = 1;
        let original_right = right.clone();

        let error = exchange_inventory_items(&mut left, 42, &mut right, 99)
            .expect_err("oversized incoming item should be rejected");

        assert!(error.contains("left inventory has no room"));
        assert_eq!(left.revision, original_left.revision);
        assert_eq!(left.containers, original_left.containers);
        assert_eq!(left.hotbar, original_left.hotbar);
        assert_eq!(right.revision, original_right.revision);
        assert_eq!(right.containers, original_right.containers);
        assert_eq!(right.hotbar, original_right.hotbar);
    }

    #[test]
    fn select_drop_instance_ids_is_seed_stable() {
        let ids = vec![1, 2, 3, 4, 5, 6];
        let left = select_drop_instance_ids(ids.clone(), 3, 12345);
        let right = select_drop_instance_ids(ids, 3, 12345);
        assert_eq!(left, right);
        assert_eq!(left.len(), 3);
    }

    #[test]
    fn apply_death_drop_to_inventory_removes_half_of_all_carryable_items() {
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items.push(PlacedItemState {
            row: 0,
            col: 1,
            instance: ItemInstance {
                instance_id: 43,
                template_id: "ningmai_powder".to_string(),
                display_name: "凝脉散".to_string(),
                grid_w: 1,
                grid_h: 1,
                weight: 0.2,
                rarity: ItemRarity::Uncommon,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 1.0,
                durability: 1.0,
                freshness: None,
                mineral_id: None,
                charges: None,
                forge_quality: None,
                forge_color: None,
                forge_side_effects: Vec::new(),
                forge_achieved_tier: None,
                alchemy: None,
                lingering_owner_qi: None,
            },
        });
        inv.hotbar[0] = Some(ItemInstance {
            instance_id: 99,
            template_id: "bone_spike".to_string(),
            display_name: "骨刺".to_string(),
            grid_w: 1,
            grid_h: 2,
            weight: 0.3,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        });
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(ItemInstance {
                instance_id: 100,
                template_id: "rusted_blade".to_string(),
                display_name: "残破旧铁短刃".to_string(),
                grid_w: 1,
                grid_h: 2,
                weight: 0.5,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 1.0,
                durability: 0.5,
                freshness: None,
                mineral_id: None,
                charges: None,
                forge_quality: None,
                forge_color: None,
                forge_side_effects: Vec::new(),
                forge_achieved_tier: None,
                alchemy: None,
                lingering_owner_qi: None,
            }),
        );

        let out = apply_death_drop_to_inventory(&mut inv, &ItemRegistry::default(), 777);

        assert_eq!(out.dropped.len(), 2);
        assert_eq!(out.revision, InventoryRevision(8));
        // 决议 #17/#12：死亡掉落按 instance 精确移除，空 SlotContents 会保留在 map 里，
        // 故统计实际剩余件须遍历 iter_all（而非 equipped.len 数槽）。
        let remaining_count = inv.containers[0].items.len()
            + inv.hotbar.iter().flatten().count()
            + inv
                .equipped
                .values()
                .map(|s| s.iter_all().count())
                .sum::<usize>();
        assert_eq!(remaining_count, 2);
    }

    #[test]
    fn apply_death_drop_on_revive_emits_event_when_items_are_dropped() {
        use valence::prelude::{App, Events, Position, Update};

        let mut app = App::new();
        app.add_event::<PlayerRevived>();
        app.add_event::<DroppedItemEvent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(Update, apply_death_drop_on_revive);

        let entity = app
            .world_mut()
            .spawn((
                make_test_inventory_with_one_item(),
                Position::new([0.0, 64.0, 0.0]),
            ))
            .id();
        app.world_mut().send_event(PlayerRevived { entity });
        app.update();

        let events = app.world().resource::<Events<DroppedItemEvent>>();
        assert_eq!(
            events.len(),
            0,
            "single carried item should not drop when floor(n/2)=0"
        );

        {
            let mut inv = app.world_mut().get_mut::<PlayerInventory>(entity).unwrap();
            inv.containers[0].items.push(PlacedItemState {
                row: 0,
                col: 1,
                instance: ItemInstance {
                    instance_id: 43,
                    template_id: "ningmai_powder".to_string(),
                    display_name: "凝脉散".to_string(),
                    grid_w: 1,
                    grid_h: 1,
                    weight: 0.2,
                    rarity: ItemRarity::Uncommon,
                    description: String::new(),
                    stack_count: 1,
                    spirit_quality: 1.0,
                    durability: 1.0,
                    freshness: None,
                    mineral_id: None,
                    charges: None,
                    forge_quality: None,
                    forge_color: None,
                    forge_side_effects: Vec::new(),
                    forge_achieved_tier: None,
                    alchemy: None,
                    lingering_owner_qi: None,
                },
            });
        }

        app.world_mut().send_event(PlayerRevived { entity });
        app.update();

        let inv = app.world().get::<PlayerInventory>(entity).unwrap();
        let events = app.world().resource::<Events<DroppedItemEvent>>();
        assert_eq!(events.len(), 1);
        assert_eq!(inv.revision, InventoryRevision(8));
        assert_eq!(inv.containers[0].items.len(), 1);
    }

    #[test]
    fn terminated_player_drops_all_items_except_on_voluntary_retire() {
        use valence::prelude::{App, EntityLayerId, InteractEntityEvent, Position, Update};

        let mut app = App::new();
        app.add_event::<PlayerTerminated>();
        app.insert_resource(DroppedLootRegistry::default());
        app.add_event::<InteractEntityEvent>();
        app.add_systems(
            Update,
            (
                apply_termination_drop_on_terminate,
                handle_remains_interactions,
            ),
        );

        let entity = app
            .world_mut()
            .spawn((
                make_test_inventory_with_one_item(),
                Position::new([10.0, 66.0, 10.0]),
                EntityLayerId(Entity::PLACEHOLDER),
                LifeRecord {
                    character_id: "offline:Azure".to_string(),
                    created_at: 0,
                    biography: vec![BiographyEntry::Terminated {
                        cause: "tribulation_failed".to_string(),
                        tick: 1,
                    }],
                    insights_taken: Vec::new(),
                    death_insights: Vec::new(),
                    skill_milestones: Vec::new(),
                    spirit_root_first: None,
                    ..LifeRecord::default()
                },
            ))
            .id();

        app.world_mut().send_event(PlayerTerminated { entity });
        app.update();

        let registry = app.world().resource::<DroppedLootRegistry>();
        let dropped_count = registry.entries.len();
        assert!(
            dropped_count >= 1,
            "terminated player should drop inventory"
        );

        // Voluntary retire should not create drops, but inventory should still be drained.
        let mut app = App::new();
        app.add_event::<PlayerTerminated>();
        app.insert_resource(DroppedLootRegistry::default());
        app.add_event::<InteractEntityEvent>();
        app.add_systems(
            Update,
            (
                apply_termination_drop_on_terminate,
                handle_remains_interactions,
            ),
        );

        let entity = app
            .world_mut()
            .spawn((
                make_test_inventory_with_one_item(),
                Position::new([10.0, 66.0, 10.0]),
                EntityLayerId(Entity::PLACEHOLDER),
                LifeRecord {
                    character_id: "offline:Azure".to_string(),
                    created_at: 0,
                    biography: vec![BiographyEntry::Terminated {
                        cause: "voluntary_retire".to_string(),
                        tick: 1,
                    }],
                    insights_taken: Vec::new(),
                    death_insights: Vec::new(),
                    skill_milestones: Vec::new(),
                    spirit_root_first: None,
                    ..LifeRecord::default()
                },
            ))
            .id();
        app.world_mut().send_event(PlayerTerminated { entity });
        app.update();

        let registry = app.world().resource::<DroppedLootRegistry>();
        assert!(
            registry.entries.is_empty(),
            "voluntary_retire should not create drops"
        );

        let inv = app.world().get::<PlayerInventory>(entity).unwrap();
        let remaining_items = inv.containers.iter().flat_map(|c| c.items.iter()).count()
            + inv.equipped.len()
            + inv.hotbar.iter().flatten().count();
        assert_eq!(
            remaining_items, 0,
            "inventory should be drained on terminate"
        );
        assert_eq!(
            inv.bone_coins, 0,
            "bone_coins should be cleared on terminate"
        );
    }

    #[test]
    fn natural_end_spawns_remains_and_allows_looting_via_interact() {
        use valence::prelude::{
            App, Despawned, EntityInteraction, Hand, InteractEntityEvent, Position, Update,
        };

        let mut app = App::new();
        app.add_event::<PlayerTerminated>();
        app.add_event::<InteractEntityEvent>();
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(
            Update,
            (
                apply_termination_drop_on_terminate,
                handle_remains_interactions,
            ),
        );

        let terminated = app
            .world_mut()
            .spawn((
                make_test_inventory_with_one_item(),
                Position::new([10.0, 66.0, 10.0]),
                EntityLayerId(Entity::PLACEHOLDER),
                LifeRecord {
                    character_id: "offline:OldOne".to_string(),
                    created_at: 0,
                    biography: vec![BiographyEntry::Terminated {
                        cause: "natural_end".to_string(),
                        tick: 1,
                    }],
                    insights_taken: Vec::new(),
                    death_insights: Vec::new(),
                    skill_milestones: Vec::new(),
                    spirit_root_first: None,
                    ..LifeRecord::default()
                },
            ))
            .id();
        {
            let mut inv = app
                .world_mut()
                .get_mut::<PlayerInventory>(terminated)
                .expect("terminated player should have inventory");
            inv.bone_coins = 7;
        }

        // Looter starts with an empty inventory.
        let mut looter_inv = make_test_inventory_with_one_item();
        for container in &mut looter_inv.containers {
            container.items.clear();
        }
        looter_inv.equipped.clear();
        looter_inv.hotbar = Default::default();
        looter_inv.bone_coins = 0;
        let looter = app
            .world_mut()
            .spawn((
                looter_inv,
                Position::new([10.0, 66.0, 10.0]),
                EntityLayerId(Entity::PLACEHOLDER),
            ))
            .id();

        app.world_mut()
            .send_event(PlayerTerminated { entity: terminated });
        app.update();

        // natural_end should not create world dropped loot entries.
        let registry = app.world().resource::<DroppedLootRegistry>();
        assert!(
            registry.entries.is_empty(),
            "natural_end should not create DroppedLootRegistry entries"
        );

        // Terminated player's inventory should be drained.
        let inv = app.world().get::<PlayerInventory>(terminated).unwrap();
        let remaining_items = inv.containers.iter().flat_map(|c| c.items.iter()).count()
            + inv.equipped.len()
            + inv.hotbar.iter().flatten().count();
        assert_eq!(remaining_items, 0);
        assert_eq!(inv.bone_coins, 0);

        // Remains should exist and hold the drained items/coins.
        let (
            remains_entity,
            remains_item_count,
            remains_bone_coins,
            remains_pos,
            remains_player_list_entry,
        ) = {
            let mut q = app
                .world_mut()
                .query::<(Entity, &RemainsContainer, &Position)>();
            let mut iter = q.iter(app.world());
            let (e, remains, pos) = iter.next().expect("expected exactly one remains container");
            assert!(
                iter.next().is_none(),
                "expected exactly one remains container"
            );
            let p = pos.get();
            (
                e,
                remains.items.len(),
                remains.bone_coins,
                [p.x, p.y, p.z],
                remains.player_list_entry,
            )
        };
        assert_eq!(remains_item_count, 1);
        assert_eq!(remains_bone_coins, 7);
        assert_eq!(remains_pos[0], 10.0);
        assert_eq!(remains_pos[1], 66.0);
        assert_eq!(remains_pos[2], 10.0);
        assert!(
            app.world().get_entity(remains_player_list_entry).is_some(),
            "player_list entry for remains should exist"
        );

        // Right click loots into the looter inventory.
        app.world_mut().send_event(InteractEntityEvent {
            client: looter,
            entity: remains_entity,
            sneaking: false,
            interact: EntityInteraction::Interact(Hand::Main),
        });
        app.update();

        let looter_inv = app.world().get::<PlayerInventory>(looter).unwrap();
        let has_item = looter_inv
            .containers
            .iter()
            .flat_map(|c| c.items.iter())
            .any(|placed| placed.instance.instance_id == 42);
        assert!(has_item, "looter should receive the remains item");
        assert_eq!(looter_inv.bone_coins, 7, "looter should receive bone_coins");

        assert!(
            app.world().get::<Despawned>(remains_entity).is_some(),
            "remains entity should be marked Despawned after looting"
        );
        assert!(
            app.world()
                .get::<Despawned>(remains_player_list_entry)
                .is_some(),
            "remains player_list entry should be marked Despawned after looting"
        );
    }

    #[test]
    fn pickup_dropped_loot_instance_reinserts_item_and_clears_registry_entry() {
        let mut inventory = make_test_inventory_with_one_item();
        inventory.containers[0].items.clear();

        let owner = Entity::PLACEHOLDER;
        let mut registry = DroppedLootRegistry::default();
        registry.entries.insert(
            42,
            DroppedLootEntry {
                instance_id: 42,
                source_container_id: MAIN_PACK_CONTAINER_ID.to_string(),
                source_row: 0,
                source_col: 0,
                world_pos: [0.5, 64.0, 0.5],
                dimension: DimensionKind::Overworld,
                item: ItemInstance {
                    instance_id: 42,
                    template_id: "starter_talisman".to_string(),
                    display_name: "启程护符".to_string(),
                    grid_w: 1,
                    grid_h: 1,
                    weight: 0.2,
                    rarity: ItemRarity::Common,
                    description: String::new(),
                    stack_count: 1,
                    spirit_quality: 1.0,
                    durability: 1.0,
                    freshness: None,
                    mineral_id: None,
                    charges: None,
                    forge_quality: None,
                    forge_color: None,
                    forge_side_effects: Vec::new(),
                    forge_achieved_tier: None,
                    alchemy: None,
                    lingering_owner_qi: None,
                },
            },
        );

        let revision =
            pickup_dropped_loot_instance(&mut inventory, &mut registry, [0.0, 64.0, 0.0], 42)
                .expect("pickup should succeed");

        assert_eq!(revision, InventoryRevision(8));
        assert_eq!(inventory.containers[0].items.len(), 1);
        assert!(!registry.entries.contains_key(&42));
        let _ = owner;
    }

    #[test]
    fn discard_inventory_item_to_dropped_loot_removes_item_and_registers_drop() {
        let mut inventory = make_test_inventory_with_one_item();
        let owner = Entity::PLACEHOLDER;
        let mut registry = DroppedLootRegistry::default();

        let outcome = discard_inventory_item_to_dropped_loot(
            &mut inventory,
            &mut registry,
            [0.0, 64.0, 0.0],
            DimensionKind::Overworld,
            42,
            &crate::schema::inventory::InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
        )
        .expect("discard should succeed");

        assert_eq!(outcome.revision, InventoryRevision(8));
        assert!(inventory.containers[0].items.is_empty());
        let entry = registry
            .entries
            .get(&42)
            .expect("registry should contain dropped item");
        assert_eq!(entry.instance_id, 42);
        assert_eq!(entry.source_container_id, MAIN_PACK_CONTAINER_ID);
        let _ = owner;
    }

    #[test]
    fn death_drop_keeps_high_durability_equipped_weapon() {
        let mut registry = ItemRegistry::default();
        registry.templates.insert(
            "iron_sword".to_string(),
            ItemTemplate {
                id: "iron_sword".to_string(),
                display_name: "铁剑".to_string(),
                category: ItemCategory::Weapon,
                placeable: None,
                max_stack_count: 1,
                grid_w: 1,
                grid_h: 2,
                base_weight: 1.0,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 1.0,
                description: String::new(),
                effect: None,
                cast_duration_ms: DEFAULT_CAST_DURATION_MS,
                cooldown_ms: DEFAULT_COOLDOWN_MS,
                weapon_spec: Some(WeaponSpec {
                    weapon_kind: crate::combat::weapon::WeaponKind::Sword,
                    base_attack: 8.0,
                    quality_tier: 0,
                    durability_max: 200.0,
                    qi_cost_mul: 1.0,
                }),
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shield_spec: None,

                shelflife_profile: None,
                shelflife_track: None,
            },
        );
        let mut inv = make_test_inventory_with_one_item();
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(ItemInstance {
                instance_id: 9001,
                template_id: "iron_sword".to_string(),
                display_name: "铁剑".to_string(),
                grid_w: 1,
                grid_h: 2,
                weight: 1.0,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 1.0,
                durability: 0.75,
                freshness: None,
                mineral_id: None,
                charges: None,
                forge_quality: None,
                forge_color: None,
                forge_side_effects: Vec::new(),
                forge_achieved_tier: None,
                alchemy: None,
                lingering_owner_qi: None,
            }),
        );

        let out = apply_death_drop_to_inventory(&mut inv, &registry, 42);

        assert!(out.dropped.iter().all(|d| d.instance.instance_id != 9001));
        assert_eq!(
            inv.equipped
                .get(EQUIP_SLOT_MAIN_HAND)
                .and_then(|s| s.held.as_ref())
                .map(|item| item.instance_id),
            Some(9001)
        );
    }

    #[test]
    fn death_drop_drops_low_durability_equipped_weapon() {
        let mut registry = ItemRegistry::default();
        registry.templates.insert(
            "iron_sword".to_string(),
            ItemTemplate {
                id: "iron_sword".to_string(),
                display_name: "铁剑".to_string(),
                category: ItemCategory::Weapon,
                placeable: None,
                max_stack_count: 1,
                grid_w: 1,
                grid_h: 2,
                base_weight: 1.0,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 1.0,
                description: String::new(),
                effect: None,
                cast_duration_ms: DEFAULT_CAST_DURATION_MS,
                cooldown_ms: DEFAULT_COOLDOWN_MS,
                weapon_spec: Some(WeaponSpec {
                    weapon_kind: crate::combat::weapon::WeaponKind::Sword,
                    base_attack: 8.0,
                    quality_tier: 0,
                    durability_max: 200.0,
                    qi_cost_mul: 1.0,
                }),
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shield_spec: None,

                shelflife_profile: None,
                shelflife_track: None,
            },
        );
        let mut inv = make_test_inventory_with_one_item();
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(ItemInstance {
                instance_id: 9002,
                template_id: "iron_sword".to_string(),
                display_name: "铁剑".to_string(),
                grid_w: 1,
                grid_h: 2,
                weight: 1.0,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 1.0,
                durability: 0.25,
                freshness: None,
                mineral_id: None,
                charges: None,
                forge_quality: None,
                forge_color: None,
                forge_side_effects: Vec::new(),
                forge_achieved_tier: None,
                alchemy: None,
                lingering_owner_qi: None,
            }),
        );

        let out = apply_death_drop_to_inventory(&mut inv, &registry, 42);

        assert!(out.dropped.iter().any(|d| d.instance.instance_id == 9002));
        // 死亡掉落按 instance 精确移除 held；空 SlotContents 会保留在 map 里，
        // 故断言 main_hand held 已清空（而非整槽 contains_key）。
        assert!(
            inv.equipped
                .get(EQUIP_SLOT_MAIN_HAND)
                .map(|s| s.is_empty())
                .unwrap_or(true),
            "低耐武器掉落后 main_hand held 应为空"
        );
    }

    #[test]
    fn calculate_current_weight_includes_container_equipped_and_hotbar() {
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.weight = 1.5;
        inv.containers[0].items[0].instance.stack_count = 2;
        inv.hotbar[0] = Some(ItemInstance {
            instance_id: 99,
            template_id: "bone_spike".to_string(),
            display_name: "骨刺".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.5,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        });
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(ItemInstance {
                instance_id: 100,
                template_id: "rusted_blade".to_string(),
                display_name: "残破旧铁短刃".to_string(),
                grid_w: 1,
                grid_h: 2,
                weight: 2.0,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 1.0,
                durability: 1.0,
                freshness: None,
                mineral_id: None,
                charges: None,
                forge_quality: None,
                forge_color: None,
                forge_side_effects: Vec::new(),
                forge_achieved_tier: None,
                alchemy: None,
                lingering_owner_qi: None,
            }),
        );

        let current = calculate_current_weight(&inv);

        assert!((current - 5.5).abs() < 1e-9);
    }

    // ========================================================================
    // plan-tarkov-backpack-v1 P1 — 重量递归上卷 pin 测试（决议 #3 固化现状语义）。
    //
    // 决议 #3：`calculate_current_weight` 三路 flat 求和（container + equipped +
    // hotbar）经核实**不重叠**：穿戴背包件自重走 equipped（worn 层），其内含物走
    // container（`pack_<id>.items`），背包件本身从不出现在任何 `ContainerState.items`
    // 里 → flat 求和数学等价于「外层背包自重 + 逐层递归内含物」的上卷。**不改公式**，
    // 仅以下列 pin 测试锁住该等价性，任何回归（误把背包件塞进 container、或漏算内含物、
    // 或双计自重）立刻撞红。
    // ========================================================================

    /// P1 pin：外层 worn 背包 + 其 grid 内一件物品 → current = 包自重 + 内物品自重。
    /// 锁住「内含物（嵌套容器里的件）确实被计入 current_weight」（递归上卷第二层）。
    #[test]
    fn calculate_current_weight_counts_item_in_nested_container() {
        let mut inv = make_empty_inventory();

        // 外层：worn 背包件骑 chest 槽，自重 2.5。
        let mut pack = make_container_item(500, "large_backpack");
        pack.weight = 2.5;
        let pack_id = container_id_for_worn_pack(500);
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(pack),
        );

        // 内层：背包派生容器 pack_500 里放一件 herb，自重 3.0。
        let mut inner = make_test_item_instance(501, "herb");
        inner.weight = 3.0;
        inv.containers.push(ContainerState {
            id: pack_id,
            name: "大背包".to_string(),
            rows: 7,
            cols: 5,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: inner,
            }],
            owner_instance_id: Some(500),
        });

        let current = calculate_current_weight(&inv);
        let expected = 2.5 + 3.0; // 包自重 + 内物品自重，递归上卷两层之和。
        assert!(
            (current - expected).abs() < 1e-9,
            "期望 current = 包自重(2.5) + 嵌套内物品自重(3.0) = {expected}（内含物必须被上卷计入），实际 {current}"
        );
    }

    /// P1 pin：穿戴背包件自重只计一次——背包件在 equipped(worn) 计一次，
    /// 绝不在 container_weight 里被重复计（背包件本身从不进 ContainerState.items）。
    /// 锁住 flat 三路求和「不重叠」前提（决议 #3 的核心）。
    #[test]
    fn calculate_current_weight_no_double_count_for_worn_pack() {
        let mut inv = make_empty_inventory();

        // worn 背包件自重 4.0（仅此一件，container 空）。
        let mut pack = make_container_item(600, "large_backpack");
        pack.weight = 4.0;
        let pack_id = container_id_for_worn_pack(600);
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(pack),
        );

        // 派生容器存在但为空——背包件本身不在此 items 里。
        inv.containers.push(ContainerState {
            id: pack_id,
            name: "大背包".to_string(),
            rows: 7,
            cols: 5,
            items: Vec::new(),
            owner_instance_id: Some(600),
        });

        let current = calculate_current_weight(&inv);
        assert!(
            (current - 4.0).abs() < 1e-9,
            "期望 current = 背包件自重 4.0（仅在 equipped 计一次，不被 container_weight 重复计），实际 {current}——若 >4.0 说明背包件自重被双计"
        );
    }

    /// P1 pin（verifiable#4 危险边界 + 状态转换锁 P0 修复语义）：
    /// 背包件被卸下（不再在任何身体槽 worn 层），但其旧 `pack_<id>` 容器尚未被
    /// `rebuild_containers_from_equipment` 清除（P0 修复路径触发前的可达状态）。
    /// - rebuild 前：孤儿容器内含物如实计入 current_weight（容器仍存在、items 仍在）。
    /// - rebuild 后：孤儿容器被清除（内含物 spill 进 body_pocket），current_weight
    ///   守恒不变（内含物换了位置但仍在某容器里），且不再出现「背包件自重 + 孤儿内含物」
    ///   的 double-count 风险面。
    #[test]
    fn calculate_current_weight_after_unequip_pack_no_double_count_orphan_container() {
        let registry = ItemRegistry::from_map(HashMap::new());
        let mut inv = make_empty_inventory();

        // body_pocket 作 spill 落点（2×3=6 格，足够容纳一件 1×1 内含物）。
        inv.containers.push(ContainerState {
            id: BODY_POCKET_CONTAINER_ID.to_string(),
            name: "暗袋".to_string(),
            rows: BODY_POCKET_ROWS,
            cols: BODY_POCKET_COLS,
            items: Vec::new(),
            owner_instance_id: None,
        });

        // 孤儿 pack_700：容器仍存在 + 含一件 herb(自重 3.0)，但 equipped 里**没有**
        // instance_id=700 的 worn 背包件（已卸下，背包件自重不再计入）。
        let pack_id = container_id_for_worn_pack(700);
        let mut inner = make_test_item_instance(701, "herb");
        inner.weight = 3.0;
        inv.containers.push(ContainerState {
            id: pack_id.clone(),
            name: "大背包".to_string(),
            rows: 7,
            cols: 5,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: inner,
            }],
            owner_instance_id: None,
        });

        // rebuild 前：孤儿内含物如实计入（容器尚在）。背包件已卸 → 不在 equipped。
        let before = calculate_current_weight(&inv);
        assert!(
            (before - 3.0).abs() < 1e-9,
            "rebuild 前期望 current = 孤儿容器内含物自重 3.0（容器仍存在故如实计入；背包件已卸不计自重），实际 {before}"
        );

        // 触发 P0 修复路径：rebuild 清除孤儿容器，内含物 spill 进 body_pocket。
        let overflow = rebuild_containers_from_equipment(&mut inv, &registry);
        assert!(
            overflow.is_empty(),
            "body_pocket 有空位，内含物应全部 spill 进去、无 overflow，实际 overflow={overflow:?}"
        );
        assert!(
            !inv.containers.iter().any(|c| c.id == pack_id),
            "rebuild 后孤儿容器 {pack_id} 必须被清除（不再可 access），实际容器列表={:?}",
            inv.containers.iter().map(|c| &c.id).collect::<Vec<_>>()
        );

        // rebuild 后：内含物换位到 body_pocket，current 守恒不变、无 double-count。
        let after = calculate_current_weight(&inv);
        assert!(
            (after - before).abs() < 1e-9,
            "rebuild 前后 current 必须守恒（内含物只换位置不增减）：期望 {before}，实际 {after}——若变大说明孤儿内含物被 double-count，若变小说明 spill 丢物"
        );
    }

    /// P1 pin（状态转换 A→B）：嵌套背包内含物使总重超 max_weight → OverloadedMarker 挂上。
    /// 锁住 `sync_overloaded_marker` 对「内含物（container_weight）」的感知。
    #[test]
    fn overloaded_marker_triggers_when_nested_pack_contents_exceed_limit() {
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.add_systems(Update, sync_overloaded_marker);

        let mut inv = make_empty_inventory();
        inv.max_weight = 10.0;

        // worn 背包件自重 1.0。
        let mut pack = make_container_item(800, "large_backpack");
        pack.weight = 1.0;
        let pack_id = container_id_for_worn_pack(800);
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(pack),
        );

        // 嵌套内含物自重 20.0 → current = 21.0 > max 10.0。
        let mut heavy = make_test_item_instance(801, "ore");
        heavy.weight = 20.0;
        inv.containers.push(ContainerState {
            id: pack_id,
            name: "大背包".to_string(),
            rows: 7,
            cols: 5,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: heavy,
            }],
            owner_instance_id: Some(800),
        });

        let entity = app.world_mut().spawn(inv).id();
        app.update();

        let marker = app
            .world()
            .get::<OverloadedMarker>(entity)
            .expect("嵌套内含物(20.0)+包自重(1.0)=21.0 > max(10.0)，应挂 OverloadedMarker");
        assert!(
            (marker.current_weight - 21.0).abs() < 1e-9,
            "marker.current_weight 应反映含嵌套内含物的总重 21.0（包自重1.0+内含物20.0），实际 {}",
            marker.current_weight
        );
        assert!(
            marker.current_weight > marker.max_weight,
            "marker 应记录超限（current {} > max {}）",
            marker.current_weight,
            marker.max_weight
        );
    }

    /// P1 pin（状态转换 A→B→A）：移除嵌套内含物使总重回落 ≤ max → OverloadedMarker 清除。
    #[test]
    fn overloaded_marker_clears_after_removing_nested_item() {
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.add_systems(Update, sync_overloaded_marker);

        let mut inv = make_empty_inventory();
        inv.max_weight = 10.0;

        let mut pack = make_container_item(900, "large_backpack");
        pack.weight = 1.0;
        let pack_id = container_id_for_worn_pack(900);
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(pack),
        );

        let mut heavy = make_test_item_instance(901, "ore");
        heavy.weight = 20.0;
        inv.containers.push(ContainerState {
            id: pack_id,
            name: "大背包".to_string(),
            rows: 7,
            cols: 5,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: heavy,
            }],
            owner_instance_id: Some(900),
        });

        let entity = app.world_mut().spawn(inv).id();
        app.update();
        assert!(
            app.world().get::<OverloadedMarker>(entity).is_some(),
            "前置：超限态应先挂 marker（A→B）"
        );

        // 移除嵌套内含物 → current 回落到 1.0（仅包自重）≤ max 10.0。
        {
            let mut inv = app.world_mut().get_mut::<PlayerInventory>(entity).unwrap();
            let pack_id = container_id_for_worn_pack(900);
            let container = inv
                .containers
                .iter_mut()
                .find(|c| c.id == pack_id)
                .expect("pack_900 容器应存在");
            container.items.clear();
        }
        app.update();

        assert!(
            app.world().get::<OverloadedMarker>(entity).is_none(),
            "移除嵌套内含物后 current(1.0) ≤ max(10.0)，OverloadedMarker 应被清除（A→B→A 状态转换闭环）"
        );
    }

    #[test]
    fn sync_overloaded_marker_adds_and_removes_marker_based_on_weight() {
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.add_systems(Update, sync_overloaded_marker);

        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.weight = 60.0;
        inv.max_weight = 50.0;
        let entity = app.world_mut().spawn(inv).id();

        app.update();

        let marker = app
            .world()
            .get::<OverloadedMarker>(entity)
            .expect("marker should exist");
        assert!(marker.current_weight > marker.max_weight);

        {
            let mut inv = app.world_mut().get_mut::<PlayerInventory>(entity).unwrap();
            inv.containers[0].items[0].instance.weight = 10.0;
        }

        app.update();

        assert!(app.world().get::<OverloadedMarker>(entity).is_none());
    }

    // =========== inventory_item_by_instance_borrow (M4 optimization) ===========

    fn make_test_item_instance(instance_id: u64, template_id: &str) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: "test".to_string(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn make_empty_inventory() -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: Vec::new(),
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        }
    }

    #[test]
    fn borrow_helper_finds_item_in_container() {
        let mut inv = make_empty_inventory();
        inv.containers.push(ContainerState {
            id: "main_pack".into(),
            name: "main_pack".into(),
            rows: 4,
            cols: 4,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: make_test_item_instance(42, "iron_sword"),
            }],

            owner_instance_id: None,
        });
        let got = inventory_item_by_instance_borrow(&inv, 42);
        assert!(got.is_some());
        assert_eq!(got.unwrap().template_id, "iron_sword");
    }

    // ─── plan-layered-equip-v1 P4（决议 #8）— 法宝触发位 apply_treasure_activate ───

    /// 注册一个 treasure 模板 + 一个普通（armor）模板的 registry。
    fn treasure_trigger_registry() -> ItemRegistry {
        let treasure = raw_item_template_toml("test_treasure", "treasure")
            .try_into_item_template(Path::new("<inline>"))
            .expect("treasure template parses");
        let armor = raw_item_template_toml("test_armor", "armor")
            .try_into_item_template(Path::new("<inline>"))
            .expect("armor template parses");
        registry_from_templates(vec![treasure, armor])
    }

    /// 带一个 8x8 main_pack 容器的空 inventory（触发位 deactivate 落点）。
    fn inventory_with_main_pack() -> PlayerInventory {
        let mut inv = make_empty_inventory();
        inv.containers.push(ContainerState {
            id: MAIN_PACK_CONTAINER_ID.into(),
            name: MAIN_PACK_CONTAINER_ID.into(),
            rows: 8,
            cols: 8,
            items: Vec::new(),
            owner_instance_id: None,
        });
        inv
    }

    #[test]
    fn treasure_activate_moves_treasure_from_container_to_trigger_slot() {
        let registry = treasure_trigger_registry();
        let mut inv = inventory_with_main_pack();
        inv.containers[0].items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(500, "test_treasure"),
        });
        let before_rev = inv.revision.0;

        let outcome = apply_treasure_activate(&mut inv, &registry, 500, true)
            .expect("activating a treasure in the container should succeed");

        assert!(
            matches!(outcome, TreasureActivateOutcome::Activated { .. }),
            "expected Activated, got {outcome:?}"
        );
        assert_eq!(
            inv.triggered_treasures.len(),
            1,
            "treasure should be in the trigger slot"
        );
        assert_eq!(inv.triggered_treasures[0].instance_id, 500);
        assert!(
            inv.containers[0].items.is_empty(),
            "treasure should have left the container (no duplication)"
        );
        assert!(inv.revision.0 > before_rev, "revision should bump");
    }

    #[test]
    fn treasure_activate_roundtrip_deactivate_returns_to_container_preserving_instance() {
        let registry = treasure_trigger_registry();
        let mut inv = inventory_with_main_pack();
        let mut original = make_test_item_instance(501, "test_treasure");
        original.durability = 0.42; // 非默认值，断言实例（含耐久）原样保留，不是重新生成
        inv.containers[0].items.push(PlacedItemState {
            row: 1,
            col: 2,
            instance: original,
        });

        apply_treasure_activate(&mut inv, &registry, 501, true).expect("activate ok");
        assert_eq!(inv.triggered_treasures.len(), 1);
        assert!(inv.containers[0].items.is_empty());

        let outcome =
            apply_treasure_activate(&mut inv, &registry, 501, false).expect("deactivate ok");
        assert!(
            matches!(outcome, TreasureActivateOutcome::Deactivated { .. }),
            "expected Deactivated, got {outcome:?}"
        );
        assert!(
            inv.triggered_treasures.is_empty(),
            "trigger slot should be empty after deactivate"
        );
        assert_eq!(
            inv.containers[0].items.len(),
            1,
            "treasure should be back in the container"
        );
        let returned = &inv.containers[0].items[0].instance;
        assert_eq!(returned.instance_id, 501, "same instance id preserved");
        assert!(
            (returned.durability - 0.42).abs() < f64::EPSILON,
            "durability preserved (existing instance moved, not regenerated): expected 0.42, got {}",
            returned.durability
        );
    }

    #[test]
    fn treasure_activate_rejects_when_trigger_slot_full() {
        let registry = treasure_trigger_registry();
        let mut inv = inventory_with_main_pack();
        // 触发位预填满 CAP 件。
        for i in 0..TREASURE_TRIGGER_CAP {
            inv.triggered_treasures
                .push(make_test_item_instance(600 + i as u64, "test_treasure"));
        }
        // 背包再放一件想激活的。
        inv.containers[0].items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(700, "test_treasure"),
        });

        let result = apply_treasure_activate(&mut inv, &registry, 700, true);

        assert!(
            result.is_err(),
            "activating into a full trigger slot must be rejected"
        );
        assert_eq!(
            inv.triggered_treasures.len(),
            TREASURE_TRIGGER_CAP,
            "trigger slot unchanged on reject"
        );
        assert_eq!(
            inv.containers[0].items.len(),
            1,
            "rejected treasure stays in the container (not dropped)"
        );
    }

    #[test]
    fn treasure_activate_rejects_non_treasure_item() {
        let registry = treasure_trigger_registry();
        let mut inv = inventory_with_main_pack();
        inv.containers[0].items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(800, "test_armor"),
        });

        let result = apply_treasure_activate(&mut inv, &registry, 800, true);

        assert!(
            result.is_err(),
            "non-Treasure items must not be activatable into the trigger slot"
        );
        assert!(
            inv.triggered_treasures.is_empty(),
            "trigger slot stays empty"
        );
        assert_eq!(
            inv.containers[0].items.len(),
            1,
            "armor stays in the container"
        );
    }

    #[test]
    fn treasure_activate_rejects_unknown_instance() {
        let registry = treasure_trigger_registry();
        let mut inv = inventory_with_main_pack();
        let result = apply_treasure_activate(&mut inv, &registry, 999, true);
        assert!(
            result.is_err(),
            "activating a non-existent instance must be rejected"
        );
        assert!(inv.triggered_treasures.is_empty());
    }

    #[test]
    fn treasure_activate_rejects_already_in_trigger_slot() {
        let registry = treasure_trigger_registry();
        let mut inv = inventory_with_main_pack();
        inv.triggered_treasures
            .push(make_test_item_instance(900, "test_treasure"));

        let result = apply_treasure_activate(&mut inv, &registry, 900, true);

        assert!(
            result.is_err(),
            "activating an instance already in the trigger slot must be rejected (idempotent)"
        );
        assert_eq!(
            inv.triggered_treasures.len(),
            1,
            "no duplicate pushed on reject"
        );
    }

    #[test]
    fn treasure_deactivate_rejects_instance_not_in_trigger_slot() {
        let registry = treasure_trigger_registry();
        let mut inv = inventory_with_main_pack();
        inv.containers[0].items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(1000, "test_treasure"),
        });

        // 该件在背包而非触发位 → 卸下应拒绝。
        let result = apply_treasure_activate(&mut inv, &registry, 1000, false);
        assert!(
            result.is_err(),
            "deactivating an instance that isn't in the trigger slot must be rejected"
        );
        assert_eq!(inv.containers[0].items.len(), 1, "container unchanged");
    }

    #[test]
    fn treasure_deactivate_rejects_when_no_free_container_slot() {
        let registry = treasure_trigger_registry();
        let mut inv = make_empty_inventory();
        // main_pack 满（1x1 且已占用），无空位接收卸下的件。
        inv.containers.push(ContainerState {
            id: MAIN_PACK_CONTAINER_ID.into(),
            name: MAIN_PACK_CONTAINER_ID.into(),
            rows: 1,
            cols: 1,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: make_test_item_instance(1100, "test_armor"),
            }],

            owner_instance_id: None,
        });
        inv.triggered_treasures
            .push(make_test_item_instance(1200, "test_treasure"));

        let result = apply_treasure_activate(&mut inv, &registry, 1200, false);

        assert!(
            result.is_err(),
            "deactivating with no free container slot must be rejected (don't drop the item)"
        );
        assert_eq!(
            inv.triggered_treasures.len(),
            1,
            "treasure stays in the trigger slot when there's nowhere to put it"
        );
        assert_eq!(
            inv.triggered_treasures[0].instance_id, 1200,
            "the same treasure is retained, not lost"
        );
    }

    #[test]
    fn borrow_helper_finds_item_in_equipped_and_hotbar() {
        let mut inv = make_empty_inventory();
        inv.equipped.insert(
            "main_hand".to_string(),
            SlotContents::held_single(make_test_item_instance(7, "talisman")),
        );
        inv.hotbar[0] = Some(make_test_item_instance(8, "pill"));
        assert_eq!(
            inventory_item_by_instance_borrow(&inv, 7)
                .unwrap()
                .template_id,
            "talisman"
        );
        assert_eq!(
            inventory_item_by_instance_borrow(&inv, 8)
                .unwrap()
                .template_id,
            "pill"
        );
    }

    #[test]
    fn transfer_all_contents_moves_containers_equipped_hotbar_and_bone_coins() {
        let mut from = make_empty_inventory();
        from.revision = InventoryRevision(12);
        from.bone_coins = 9;
        from.containers.push(ContainerState {
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 2,
            cols: 2,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: make_test_item_instance(1, "spirit_grass"),
            }],

            owner_instance_id: None,
        });
        from.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(make_test_item_instance(2, "iron_sword")),
        );
        from.hotbar[4] = Some(make_test_item_instance(3, "guyuan_pill"));

        let mut to = make_empty_inventory();
        to.revision = InventoryRevision(20);
        to.bone_coins = 5;
        to.containers.push(ContainerState {
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 3,
            cols: 3,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: make_test_item_instance(9, "existing"),
            }],

            owner_instance_id: None,
        });

        let outcome = transfer_all_inventory_contents(&mut from, &mut to);

        assert_eq!(outcome.items_moved, 3);
        assert_eq!(outcome.bone_coins_moved, 9);
        assert_eq!(outcome.from_revision, InventoryRevision(13));
        assert_eq!(outcome.to_revision, InventoryRevision(21));
        assert_eq!(from.bone_coins, 0);
        assert!(from
            .containers
            .iter()
            .all(|container| container.items.is_empty()));
        assert!(from.equipped.is_empty());
        assert!(from.hotbar.iter().all(Option::is_none));

        assert_eq!(to.bone_coins, 14);
        let moved_ids: Vec<u64> = to
            .containers
            .iter()
            .flat_map(|container| container.items.iter())
            .map(|placed| placed.instance.instance_id)
            .collect();
        for expected in [1, 2, 3, 9] {
            assert!(moved_ids.contains(&expected));
        }
    }

    #[test]
    fn borrow_helper_returns_none_for_missing_instance() {
        let inv = make_empty_inventory();
        assert!(inventory_item_by_instance_borrow(&inv, 99).is_none());
    }

    // =========== plan-backpack-equip-v1 P0 — ContainerSpec + 背包槽测试 ===========

    fn make_container_template(
        id: &str,
        equip_slot: &str,
        rows: u8,
        cols: u8,
        weight_capacity: f64,
    ) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: id.to_string(),
            category: ItemCategory::Container,
            placeable: None,
            max_stack_count: 1,
            grid_w: 2,
            grid_h: 3,
            base_weight: 0.5,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 1.0,
            description: "test backpack".to_string(),
            effect: None,
            cast_duration_ms: DEFAULT_CAST_DURATION_MS,
            cooldown_ms: DEFAULT_COOLDOWN_MS,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: Some(ContainerSpec {
                rows,
                cols,
                weight_capacity,
                equip_slot: equip_slot.to_string(),
                durability_cost_per_op: 0.0,
                attrition_exempt: false,
                accept_filter: None,
            }),
            shield_spec: None,

            shelflife_profile: None,
            shelflife_track: None,
        }
    }

    fn make_container_item(instance_id: u64, template_id: &str) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 2,
            grid_h: 3,
            weight: 0.5,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    #[test]
    fn attrition_exempt_container_marks_inner_instance_exempt() {
        let mut sealed_bag = make_container_template("sealed_bag", EQUIP_SLOT_CHEST, 2, 2, 10.0);
        sealed_bag
            .container_spec
            .as_mut()
            .expect("sealed_bag should have container spec")
            .attrition_exempt = true;
        let registry =
            ItemRegistry::from_map(HashMap::from([("sealed_bag".to_string(), sealed_bag)]));

        let mut inv = make_empty_inventory();
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(1000, "sealed_bag")),
        );
        inv.containers.push(ContainerState {
            id: container_id_for_worn_pack(1000),
            name: "封灵背包".to_string(),
            rows: 2,
            cols: 2,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: make_test_item_instance(1001, "spirit_herb"),
            }],

            owner_instance_id: None,
        });

        assert!(
            inventory_instance_container_attrition_exempt(&inv, &registry, 1001),
            "封灵容器内物品应按 instance_id 识别为搬运磨损豁免"
        );
    }

    #[test]
    fn ordinary_container_does_not_mark_inner_instance_exempt() {
        let ordinary_bag = make_container_template("ordinary_bag", EQUIP_SLOT_CHEST, 2, 2, 10.0);
        let registry =
            ItemRegistry::from_map(HashMap::from([("ordinary_bag".to_string(), ordinary_bag)]));

        let mut inv = make_empty_inventory();
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(1002, "ordinary_bag")),
        );
        inv.containers.push(ContainerState {
            id: container_id_for_worn_pack(1002),
            name: "普通背包".to_string(),
            rows: 2,
            cols: 2,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: make_test_item_instance(1003, "spirit_herb"),
            }],

            owner_instance_id: None,
        });

        assert!(
            !inventory_instance_container_attrition_exempt(&inv, &registry, 1003),
            "普通容器内物品不应误判为搬运磨损豁免"
        );
    }

    #[test]
    fn equipped_or_hotbar_instance_is_not_container_attrition_exempt() {
        let mut sealed_bag = make_container_template("sealed_bag", EQUIP_SLOT_CHEST, 2, 2, 10.0);
        sealed_bag
            .container_spec
            .as_mut()
            .expect("sealed_bag should have container spec")
            .attrition_exempt = true;
        let registry =
            ItemRegistry::from_map(HashMap::from([("sealed_bag".to_string(), sealed_bag)]));

        let mut inv = make_empty_inventory();
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(1004, "sealed_bag")),
        );
        inv.hotbar[0] = Some(make_test_item_instance(1005, "spirit_herb"));

        assert!(
            !inventory_instance_container_attrition_exempt(&inv, &registry, 1004),
            "装备槽中的容器物品自身不应因自身 container_spec 被判为内含物豁免"
        );
        assert!(
            !inventory_instance_container_attrition_exempt(&inv, &registry, 1005),
            "hotbar 物品不在封灵容器内，不应获得容器级豁免"
        );
    }

    // P0.1 — ContainerSpec TOML 解析：正例

    #[test]
    fn parse_container_spec_valid_chest() {
        // 决议 #17：背包 equip_slot 指向身体槽（chest）。
        let raw = ContainerSpecToml {
            rows: 7,
            cols: 5,
            weight_capacity: 30.0,
            equip_slot: EQUIP_SLOT_CHEST.to_string(),
            durability_cost_per_op: 0.001,
            attrition_exempt: false,
            accept: None,
        };
        let spec = parse_container_spec(raw, Path::new("<test>"), "chest_pack_item")
            .expect("should parse");
        assert_eq!(spec.rows, 7, "rows mismatch");
        assert_eq!(spec.cols, 5, "cols mismatch");
        assert!(
            (spec.weight_capacity - 30.0).abs() < f64::EPSILON,
            "weight_capacity mismatch"
        );
        assert_eq!(spec.equip_slot, EQUIP_SLOT_CHEST, "equip_slot mismatch");
        assert!((spec.durability_cost_per_op - 0.001).abs() < f64::EPSILON);
        assert!(!spec.attrition_exempt, "普通背包默认不应豁免搬运磨损");
        assert_eq!(
            spec.accept_filter, None,
            "旧 TOML 未声明 accept 时应保持 accept_filter=None，避免破坏既有容器"
        );
    }

    #[test]
    fn parse_container_spec_valid_head() {
        let raw = ContainerSpecToml {
            rows: 3,
            cols: 3,
            weight_capacity: 10.0,
            equip_slot: EQUIP_SLOT_HEAD.to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: false,
            accept: None,
        };
        let spec =
            parse_container_spec(raw, Path::new("<test>"), "head_pack_item").expect("should parse");
        assert_eq!(spec.equip_slot, EQUIP_SLOT_HEAD);
    }

    #[test]
    fn parse_container_spec_valid_legs() {
        let raw = ContainerSpecToml {
            rows: 4,
            cols: 3,
            weight_capacity: 20.0,
            equip_slot: EQUIP_SLOT_LEGS.to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: true,
            accept: None,
        };
        let spec =
            parse_container_spec(raw, Path::new("<test>"), "legs_pack_item").expect("should parse");
        assert_eq!(spec.equip_slot, EQUIP_SLOT_LEGS);
        assert!(
            spec.attrition_exempt,
            "显式封灵容器应保留 attrition_exempt=true"
        );
    }

    // ── plan-container-filter-and-completion-v1 P0 — category + accept_filter 数据模型 ──

    #[test]
    fn parse_item_category_accepts_container_filter_categories_and_aliases() {
        let path = Path::new("<inline-items.toml>");
        let cases = [
            ("mineral", ItemCategory::Mineral),
            ("ore", ItemCategory::Mineral),
            ("MINERAL", ItemCategory::Mineral),
            (" anqi ", ItemCategory::Anqi),
            ("hidden_weapon", ItemCategory::Anqi),
            ("liquid", ItemCategory::Liquid),
        ];
        for (raw, expected) in cases {
            let parsed = parse_item_category(raw, path, "filter_case")
                .expect("container filter category should parse");
            assert_eq!(
                parsed, expected,
                "期望 category `{raw}` 解析为 {expected:?}，实际得到 {parsed:?}"
            );
        }
    }

    #[test]
    fn item_category_container_filter_variants_serde_roundtrip() {
        for category in [
            ItemCategory::Mineral,
            ItemCategory::Anqi,
            ItemCategory::Liquid,
        ] {
            let json = serde_json::to_string(&category).expect("ItemCategory should serialize");
            let parsed: ItemCategory =
                serde_json::from_str(&json).expect("ItemCategory should deserialize");
            assert_eq!(
                parsed, category,
                "期望 {category:?} serde roundtrip 后保持同一变体"
            );
        }
    }

    #[test]
    fn container_filter_categories_have_pinned_default_stack_counts() {
        assert_eq!(
            default_max_stack_count_for_category(ItemCategory::Mineral),
            64
        );
        assert_eq!(default_max_stack_count_for_category(ItemCategory::Anqi), 32);
        assert_eq!(
            default_max_stack_count_for_category(ItemCategory::Liquid),
            16
        );
    }

    #[test]
    fn parse_container_spec_accept_empty_is_explicit_all_accepting_filter() {
        let raw = ContainerSpecToml {
            rows: 2,
            cols: 2,
            weight_capacity: 0.0,
            equip_slot: EQUIP_SLOT_CHEST.to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: false,
            accept: Some(Vec::new()),
        };
        let spec = parse_container_spec(raw, Path::new("<test>"), "open_pouch")
            .expect("explicit empty accept list should parse");
        assert_eq!(
            spec.accept_filter,
            Some(Vec::new()),
            "显式 accept=[] 应保留为 Some(empty)，语义仍由 item_passes_filter 判定为全收"
        );
    }

    #[test]
    fn parse_container_spec_accept_parses_categories_and_template_prefix() {
        let raw = ContainerSpecToml {
            rows: 3,
            cols: 3,
            weight_capacity: 0.0,
            equip_slot: EQUIP_SLOT_CHEST.to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: false,
            accept: Some(vec![
                "mineral".to_string(),
                "prefix:anqi_".to_string(),
                "hidden_weapon".to_string(),
            ]),
        };
        let spec = parse_container_spec(raw, Path::new("<test>"), "filtered_pouch")
            .expect("category and prefix filters should parse");
        assert_eq!(
            spec.accept_filter,
            Some(vec![
                ContainerAcceptFilter::Category(ItemCategory::Mineral),
                ContainerAcceptFilter::TemplatePrefix("anqi_".to_string()),
                ContainerAcceptFilter::Category(ItemCategory::Anqi),
            ])
        );
    }

    #[test]
    fn parse_container_spec_accept_trims_template_prefix_payload() {
        for raw_prefix in ["prefix:anqi_", "prefix: anqi_"] {
            let raw = ContainerSpecToml {
                rows: 3,
                cols: 3,
                weight_capacity: 0.0,
                equip_slot: EQUIP_SLOT_CHEST.to_string(),
                durability_cost_per_op: 0.0,
                attrition_exempt: false,
                accept: Some(vec![raw_prefix.to_string()]),
            };
            let spec = parse_container_spec(raw, Path::new("<test>"), "prefix_pouch")
                .expect("prefix accept entry should parse with optional whitespace");
            assert_eq!(
                spec.accept_filter,
                Some(vec![ContainerAcceptFilter::TemplatePrefix(
                    "anqi_".to_string()
                )]),
                "prefix accept entry `{raw_prefix}` 应归一化为无空白前缀"
            );
        }
    }

    #[test]
    fn parse_container_spec_rejects_invalid_accept_entries() {
        for (accept, expected_fragment) in [
            (vec!["unknown_category".to_string()], "unknown category"),
            (vec!["".to_string()], "empty container.accept entry"),
            (vec!["prefix:".to_string()], "empty container.accept prefix"),
        ] {
            let raw = ContainerSpecToml {
                rows: 2,
                cols: 2,
                weight_capacity: 0.0,
                equip_slot: EQUIP_SLOT_CHEST.to_string(),
                durability_cost_per_op: 0.0,
                attrition_exempt: false,
                accept: Some(accept),
            };
            let err = parse_container_spec(raw, Path::new("<test>"), "bad_accept")
                .expect_err("invalid accept entry should fail");
            assert!(
                err.contains(expected_fragment),
                "期望错误包含 `{expected_fragment}`，实际错误为 {err}"
            );
        }
    }

    #[test]
    fn item_passes_filter_treats_none_and_empty_as_all_accepting() {
        let registry = registry_from_templates(vec![test_template(
            "ordinary_herb",
            ItemCategory::Herb,
            1,
            1,
            64,
        )]);
        let item = make_test_item_instance(42, "ordinary_herb");
        assert!(item_passes_filter(&None, &item, &registry));
        assert!(item_passes_filter(&Some(Vec::new()), &item, &registry));
    }

    #[test]
    fn item_passes_filter_matches_category_template_prefix_and_union() {
        let registry = registry_from_templates(vec![
            test_template("ore_iron", ItemCategory::Mineral, 1, 1, 64),
            test_template("spirit_herb", ItemCategory::Herb, 1, 1, 64),
            test_template("water_skin_filled", ItemCategory::Liquid, 1, 1, 16),
            test_template("anqi_bone_chip", ItemCategory::Anqi, 1, 1, 32),
        ]);
        let mineral_filter = Some(vec![ContainerAcceptFilter::Category(ItemCategory::Mineral)]);
        assert!(item_passes_filter(
            &mineral_filter,
            &make_test_item_instance(1, "ore_iron"),
            &registry
        ));
        assert!(!item_passes_filter(
            &mineral_filter,
            &make_test_item_instance(2, "spirit_herb"),
            &registry
        ));

        let prefix_filter = Some(vec![ContainerAcceptFilter::TemplatePrefix(
            "anqi_".to_string(),
        )]);
        assert!(item_passes_filter(
            &prefix_filter,
            &make_test_item_instance(3, "anqi_bone_chip"),
            &registry
        ));
        assert!(!item_passes_filter(
            &prefix_filter,
            &make_test_item_instance(4, "ore_iron"),
            &registry
        ));

        let union_filter = Some(vec![
            ContainerAcceptFilter::Category(ItemCategory::Mineral),
            ContainerAcceptFilter::Category(ItemCategory::Liquid),
        ]);
        assert!(item_passes_filter(
            &union_filter,
            &make_test_item_instance(5, "water_skin_filled"),
            &registry
        ));
        assert!(!item_passes_filter(
            &union_filter,
            &make_test_item_instance(6, "spirit_herb"),
            &registry
        ));
    }

    #[test]
    fn container_spec_accept_filter_serde_roundtrip() {
        let spec = ContainerSpec {
            rows: 2,
            cols: 3,
            weight_capacity: 4.0,
            equip_slot: EQUIP_SLOT_CHEST.to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: false,
            accept_filter: Some(vec![
                ContainerAcceptFilter::Category(ItemCategory::Mineral),
                ContainerAcceptFilter::TemplatePrefix("anqi_".to_string()),
            ]),
        };
        let json = serde_json::to_string(&spec).expect("ContainerSpec should serialize");
        let parsed: ContainerSpec =
            serde_json::from_str(&json).expect("ContainerSpec should deserialize");
        assert_eq!(parsed, spec);
    }

    #[test]
    fn legacy_container_spec_json_without_accept_filter_defaults_to_none() {
        let json = r#"{
            "rows": 2,
            "cols": 3,
            "weight_capacity": 4.0,
            "equip_slot": "waist_pouch",
            "durability_cost_per_op": 0.0,
            "attrition_exempt": false
        }"#;
        let parsed: ContainerSpec =
            serde_json::from_str(json).expect("legacy ContainerSpec should deserialize");
        assert_eq!(
            parsed.accept_filter, None,
            "旧存档/协议缺 accept_filter 时必须默认 None"
        );
        let serialized =
            serde_json::to_string(&parsed).expect("legacy ContainerSpec should serialize");
        assert!(
            !serialized.contains("accept_filter"),
            "accept_filter=None 序列化时应省略字段，避免旧 JSON 形状变成 null：{serialized}"
        );
    }

    // P0.1 — ContainerSpec TOML 解析：反例

    #[test]
    fn parse_container_spec_rejects_rows_zero() {
        let raw = ContainerSpecToml {
            rows: 0,
            cols: 4,
            weight_capacity: 10.0,
            equip_slot: EQUIP_SLOT_CHEST.to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: false,
            accept: None,
        };
        let err = parse_container_spec(raw, Path::new("<test>"), "bad_rows")
            .expect_err("should fail with rows=0");
        assert!(err.contains("rows"), "expected rows error, got: {err}");
    }

    #[test]
    fn parse_container_spec_rejects_rows_overflow() {
        let raw = ContainerSpecToml {
            rows: 17,
            cols: 4,
            weight_capacity: 10.0,
            equip_slot: EQUIP_SLOT_CHEST.to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: false,
            accept: None,
        };
        let err = parse_container_spec(raw, Path::new("<test>"), "bad_rows_overflow")
            .expect_err("rows > 16 should fail");
        assert!(err.contains("rows"), "expected rows error, got: {err}");
    }

    #[test]
    fn parse_container_spec_rejects_cols_zero() {
        let raw = ContainerSpecToml {
            rows: 4,
            cols: 0,
            weight_capacity: 10.0,
            equip_slot: EQUIP_SLOT_CHEST.to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: false,
            accept: None,
        };
        let err = parse_container_spec(raw, Path::new("<test>"), "bad_cols")
            .expect_err("cols=0 should fail");
        assert!(err.contains("cols"), "expected cols error, got: {err}");
    }

    #[test]
    fn parse_container_spec_rejects_negative_weight_capacity() {
        let raw = ContainerSpecToml {
            rows: 4,
            cols: 4,
            weight_capacity: -1.0,
            equip_slot: EQUIP_SLOT_CHEST.to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: false,
            accept: None,
        };
        let err = parse_container_spec(raw, Path::new("<test>"), "bad_weight")
            .expect_err("negative weight_capacity should fail");
        assert!(
            err.contains("weight_capacity"),
            "expected weight_capacity error, got: {err}"
        );
    }

    #[test]
    fn parse_container_spec_rejects_invalid_equip_slot() {
        // 决议 #17：背包 equip_slot 只接受身体槽（head/chest/legs/feet）；
        // 旧 back_pack 专属槽已删，作为 equip_slot 现属非法。
        let raw = ContainerSpecToml {
            rows: 4,
            cols: 4,
            weight_capacity: 10.0,
            equip_slot: "back_pack".to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: false,
            accept: None,
        };
        let err = parse_container_spec(raw, Path::new("<test>"), "bad_slot")
            .expect_err("invalid equip_slot should fail");
        assert!(
            err.contains("equip_slot"),
            "expected equip_slot error, got: {err}"
        );
    }

    #[test]
    fn parse_container_spec_rejects_negative_durability_cost() {
        let raw = ContainerSpecToml {
            rows: 4,
            cols: 4,
            weight_capacity: 10.0,
            equip_slot: EQUIP_SLOT_CHEST.to_string(),
            durability_cost_per_op: -0.1,
            attrition_exempt: false,
            accept: None,
        };
        let err = parse_container_spec(raw, Path::new("<test>"), "bad_dur_cost")
            .expect_err("negative durability_cost_per_op should fail");
        assert!(
            err.contains("durability_cost_per_op"),
            "expected durability_cost_per_op error, got: {err}"
        );
    }

    // P0.2 — 常量存在性（决议 #17 删除 back_pack/waist_pouch/chest_satchel 专属槽常量后，
    // 仅保留 body_pocket / 基础负重等仍存活的常量断言）。

    #[test]
    fn body_pocket_and_base_carry_constants_are_correct() {
        assert_eq!(BODY_POCKET_CONTAINER_ID, "body_pocket");
        assert_eq!(BODY_POCKET_ROWS, 2);
        assert_eq!(BODY_POCKET_COLS, 3);
        assert!((BASE_CARRY_CAPACITY - 15.0).abs() < f64::EPSILON);
    }

    // P0.3 — rebuild_containers_from_equipment 行为

    #[test]
    fn rebuild_containers_creates_body_pocket_when_missing() {
        let registry = ItemRegistry::from_map(HashMap::new());
        let mut inv = make_empty_inventory();
        assert!(
            !inv.containers
                .iter()
                .any(|c| c.id == BODY_POCKET_CONTAINER_ID),
            "should not have body_pocket initially"
        );

        rebuild_containers_from_equipment(&mut inv, &registry);

        assert!(
            inv.containers
                .iter()
                .any(|c| c.id == BODY_POCKET_CONTAINER_ID),
            "body_pocket should be created"
        );
        let pocket = inv
            .containers
            .iter()
            .find(|c| c.id == BODY_POCKET_CONTAINER_ID)
            .unwrap();
        assert_eq!(
            pocket.rows, BODY_POCKET_ROWS,
            "body_pocket rows should be {BODY_POCKET_ROWS}"
        );
        assert_eq!(
            pocket.cols, BODY_POCKET_COLS,
            "body_pocket cols should be {BODY_POCKET_COLS}"
        );
    }

    #[test]
    fn rebuild_containers_preserves_existing_body_pocket() {
        let registry = ItemRegistry::from_map(HashMap::new());
        let mut inv = make_empty_inventory();
        inv.containers.push(ContainerState {
            id: BODY_POCKET_CONTAINER_ID.to_string(),
            name: "暗袋".to_string(),
            rows: BODY_POCKET_ROWS,
            cols: BODY_POCKET_COLS,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: make_test_item_instance(77, "herb_a"),
            }],

            owner_instance_id: None,
        });

        rebuild_containers_from_equipment(&mut inv, &registry);

        let pocket = inv
            .containers
            .iter()
            .find(|c| c.id == BODY_POCKET_CONTAINER_ID)
            .unwrap();
        assert_eq!(
            pocket.items.len(),
            1,
            "existing body_pocket item should be preserved"
        );
    }

    #[test]
    fn rebuild_containers_adds_container_for_equipped_backpack() {
        let backpack_template =
            make_container_template("large_backpack", EQUIP_SLOT_CHEST, 7, 5, 30.0);
        let registry = ItemRegistry::from_map(HashMap::from([(
            "large_backpack".to_string(),
            backpack_template,
        )]));

        let mut inv = make_empty_inventory();
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(200, "large_backpack")),
        );

        rebuild_containers_from_equipment(&mut inv, &registry);

        let pack_id = container_id_for_worn_pack(200);
        assert!(
            inv.containers.iter().any(|c| c.id == pack_id),
            "pack_<instance_id> container should be created when equipped to chest worn"
        );
        let bp = inv.containers.iter().find(|c| c.id == pack_id).unwrap();
        assert_eq!(bp.rows, 7, "rows should match container_spec");
        assert_eq!(bp.cols, 5, "cols should match container_spec");
    }

    // plan-tarkov-backpack-v1 P5（决议 #1）— 嵌套深度 2 层封顶固化回归。
    // 深度上限 = 2 层：worn 背包 → 其 grid → 物品。放进 grid 的背包件**不**被
    // `rebuild_containers_from_equipment` 展开为第 3 层可访问容器——rebuild 只扫身体槽
    // worn 层（`worn_container_items`），grid 内的 PlacedItemState 永不被派生容器。
    // 数据模型天然封顶；本测试锁住该不变量，任何「也展开 grid 内背包件」的回归立即撞红。
    #[test]
    fn rebuild_does_not_expand_container_item_placed_inside_grid_two_layer_cap() {
        let outer = make_container_template("outer_pack", EQUIP_SLOT_CHEST, 3, 3, 12.0);
        let inner = make_container_template("inner_pouch", EQUIP_SLOT_CHEST, 2, 2, 6.0);
        let registry = ItemRegistry::from_map(HashMap::from([
            ("outer_pack".to_string(), outer),
            ("inner_pouch".to_string(), inner),
        ]));

        let mut inv = make_empty_inventory();
        // 第 1 层：外层背包穿在 chest worn 层 → 第 2 层：其 grid 容器。
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(200, "outer_pack")),
        );
        rebuild_containers_from_equipment(&mut inv, &registry);
        let outer_id = container_id_for_worn_pack(200);
        assert!(
            inv.containers.iter().any(|c| c.id == outer_id),
            "穿戴的外层背包（worn 层）应派生可访问容器 {outer_id}"
        );

        // 把另一个背包件（inner_pouch，本身带 container_spec）放进外层背包的 grid——
        // 它是 grid 里的一件物品，不是穿在身上的 worn 件。
        let inner_instance_id = 201;
        {
            let outer_container = inv
                .containers
                .iter_mut()
                .find(|c| c.id == outer_id)
                .expect("外层容器应存在");
            outer_container.items.push(PlacedItemState {
                row: 0,
                col: 0,
                instance: make_container_item(inner_instance_id, "inner_pouch"),
            });
        }

        rebuild_containers_from_equipment(&mut inv, &registry);

        // 关键不变量：grid 内的背包件不得被展开为第 3 层容器。
        let inner_id = container_id_for_worn_pack(inner_instance_id);
        assert!(
            !inv.containers.iter().any(|c| c.id == inner_id),
            "嵌套深度封顶=2：grid 内背包件不得派生可访问容器（不应出现 {inner_id}）"
        );
        // inner_pouch 仍原样作为普通物品留在外层 grid，未被抽走。
        let outer_container = inv
            .containers
            .iter()
            .find(|c| c.id == outer_id)
            .expect("外层容器应仍存在");
        assert!(
            outer_container
                .items
                .iter()
                .any(|p| p.instance.instance_id == inner_instance_id),
            "grid 内的背包件应原样保留为 PlacedItemState，不被展开抽走"
        );
        // pack_<id> 容器恰好 1 个（仅外层 worn 件），grid 内背包件不计入。
        let pack_like = inv
            .containers
            .iter()
            .filter(|c| worn_pack_instance_from_container_id(&c.id).is_some())
            .count();
        assert_eq!(
            pack_like, 1,
            "只应有 1 个 pack_<id> 容器（外层 worn 背包），grid 内背包件不派生第 3 层"
        );
    }

    #[test]
    fn rebuild_containers_removes_empty_container_when_unequipped() {
        let backpack_template =
            make_container_template("large_backpack", EQUIP_SLOT_CHEST, 7, 5, 30.0);
        let registry = ItemRegistry::from_map(HashMap::from([(
            "large_backpack".to_string(),
            backpack_template,
        )]));

        let mut inv = make_empty_inventory();
        // 预置一个 pack_<id> 容器但没有对应穿戴背包件（孤儿）。
        let pack_id = container_id_for_worn_pack(200);
        inv.containers.push(ContainerState {
            id: pack_id.clone(),
            name: "大背包".to_string(),
            rows: 7,
            cols: 5,
            items: Vec::new(),
            owner_instance_id: None,
        });

        rebuild_containers_from_equipment(&mut inv, &registry);

        assert!(
            !inv.containers.iter().any(|c| c.id == pack_id),
            "empty pack container should be removed when unequipped"
        );
    }

    // Bug C（真机回归）— 孤儿非空 pack_<id> 容器（无对应穿戴背包件）必须**清理**，不得残留可
    // access：先把内含物 spill 到存活容器（body_pocket 兜底），再移除容器。物品有去向不丢。
    // 旧行为（`|| !c.items.is_empty()` 保留孤儿）= 丢背包后仍能从孤儿容器取物 = 数据/玩法 bug。
    #[test]
    fn rebuild_containers_spills_orphan_items_and_removes_container() {
        let registry = ItemRegistry::from_map(HashMap::new());
        let mut inv = make_empty_inventory();
        // body_pocket 作为 spill 兜底落点（2×3 = 6 格，足够收 1 件）。
        inv.containers.push(ContainerState {
            id: BODY_POCKET_CONTAINER_ID.to_string(),
            name: "暗袋".to_string(),
            rows: BODY_POCKET_ROWS,
            cols: BODY_POCKET_COLS,
            items: Vec::new(),
            owner_instance_id: None,
        });
        // 孤儿 pack_200：装着 herb(instance_id=55) 但 equipped 里无 instance_id=200 的穿戴背包件。
        let pack_id = container_id_for_worn_pack(200);
        inv.containers.push(ContainerState {
            id: pack_id.clone(),
            name: "大背包".to_string(),
            rows: 7,
            cols: 5,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: make_test_item_instance(55, "herb"),
            }],

            owner_instance_id: None,
        });

        let overflow = rebuild_containers_from_equipment(&mut inv, &registry);

        // 孤儿容器消失（不可再 access）。
        assert!(
            !inv.containers.iter().any(|c| c.id == pack_id),
            "孤儿 pack_<id> 容器必须移除（丢背包后不允许残留可 access 的孤儿容器）"
        );
        // herb 应 spill 进 body_pocket（有去向、不丢、不进 overflow）。
        assert!(
            overflow.is_empty(),
            "body_pocket 有空位时不应产生 overflow；实际 overflow={:?}",
            overflow.iter().map(|i| &i.template_id).collect::<Vec<_>>()
        );
        let pocket = inv
            .containers
            .iter()
            .find(|c| c.id == BODY_POCKET_CONTAINER_ID)
            .expect("body_pocket 应存在");
        assert_eq!(
            pocket
                .items
                .iter()
                .map(|p| p.instance.instance_id)
                .collect::<Vec<_>>(),
            vec![55],
            "孤儿容器里的 herb(55) 应 spill 进 body_pocket，物品不丢"
        );
    }

    // Bug C（边界）— spill 落点全满时，放不下的孤儿物品上抛 overflow（由调用方掉落），仍不丢、不残留孤儿。
    #[test]
    fn rebuild_containers_orphan_items_overflow_when_no_room() {
        let registry = ItemRegistry::from_map(HashMap::new());
        let mut inv = make_empty_inventory();
        // 不提供任何存活容器（无 body_pocket、无 live pack）——rebuild 会建一个空 body_pocket(2×3)。
        // 孤儿 pack 里塞 7 件 1×1，body_pocket 只能收 6 件 → 第 7 件 overflow。
        let pack_id = container_id_for_worn_pack(200);
        let mut items = Vec::new();
        for i in 0..7u8 {
            items.push(PlacedItemState {
                row: i,
                col: 0,
                instance: make_test_item_instance(1000 + u64::from(i), "herb"),
            });
        }
        inv.containers.push(ContainerState {
            id: pack_id.clone(),
            name: "大背包".to_string(),
            rows: 7,
            cols: 5,
            items,
            owner_instance_id: None,
        });

        let overflow = rebuild_containers_from_equipment(&mut inv, &registry);

        assert!(
            !inv.containers.iter().any(|c| c.id == pack_id),
            "孤儿容器必须移除"
        );
        // body_pocket(2×3=6) 收 6 件，第 7 件无处安放 → overflow（不丢，调用方掉落）。
        assert_eq!(
            overflow.len(),
            1,
            "body_pocket 6 格满后第 7 件应进 overflow；实际 overflow.len()={}",
            overflow.len()
        );
        let pocket = inv
            .containers
            .iter()
            .find(|c| c.id == BODY_POCKET_CONTAINER_ID)
            .expect("body_pocket 应被建出");
        assert_eq!(pocket.items.len(), 6, "body_pocket 应收满 6 件");
        // 总物品数守恒：6 spill + 1 overflow = 7 原始件，无丢失。
        assert_eq!(
            pocket.items.len() + overflow.len(),
            7,
            "spill + overflow 必须 = 原孤儿容器物品数（物品守恒，不丢数据）"
        );
    }

    // Bug C（不误删）— 仍有对应穿戴背包件的非空 pack_<id> 容器（自洽，非孤儿）必须原样保留。
    #[test]
    fn rebuild_containers_preserves_nonempty_container_with_live_backpack() {
        let backpack_template =
            make_container_template("large_backpack", EQUIP_SLOT_CHEST, 7, 5, 30.0);
        let registry = ItemRegistry::from_map(HashMap::from([(
            "large_backpack".to_string(),
            backpack_template,
        )]));
        let mut inv = make_empty_inventory();
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(200, "large_backpack")),
        );
        let pack_id = container_id_for_worn_pack(200);
        inv.containers.push(ContainerState {
            id: pack_id.clone(),
            name: "大背包".to_string(),
            rows: 7,
            cols: 5,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: make_test_item_instance(55, "herb"),
            }],

            owner_instance_id: None,
        });

        let overflow = rebuild_containers_from_equipment(&mut inv, &registry);

        assert!(overflow.is_empty(), "自洽容器不应触发 spill/overflow");
        let pack = inv
            .containers
            .iter()
            .find(|c| c.id == pack_id)
            .expect("有对应穿戴背包件的容器必须保留");
        assert_eq!(
            pack.items
                .iter()
                .map(|p| p.instance.instance_id)
                .collect::<Vec<_>>(),
            vec![55],
            "自洽容器内含物原样保留，不被 spill 走"
        );
    }

    // P0.4 — compute_max_weight 计算

    #[test]
    fn compute_max_weight_no_backpacks_returns_base() {
        let registry = ItemRegistry::from_map(HashMap::new());
        let inv = make_empty_inventory();
        let w = compute_max_weight(&inv, &registry);
        assert!(
            (w - BASE_CARRY_CAPACITY).abs() < f64::EPSILON,
            "expected BASE_CARRY_CAPACITY={BASE_CARRY_CAPACITY}, got {w}"
        );
    }

    #[test]
    fn compute_max_weight_adds_equipped_backpack_capacity() {
        let backpack_template =
            make_container_template("large_backpack", EQUIP_SLOT_CHEST, 7, 5, 30.0);
        let registry = ItemRegistry::from_map(HashMap::from([(
            "large_backpack".to_string(),
            backpack_template,
        )]));

        let mut inv = make_empty_inventory();
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(300, "large_backpack")),
        );

        let w = compute_max_weight(&inv, &registry);
        assert!(
            (w - (BASE_CARRY_CAPACITY + 30.0)).abs() < f64::EPSILON,
            "expected BASE + 30.0 = {}, got {w}",
            BASE_CARRY_CAPACITY + 30.0
        );
    }

    /// plan-tarkov-backpack-v1 P1 pin（固化决议 #3）：穿戴背包件自重**不**额外占
    /// max_weight 上限——`compute_max_weight = BASE + Σ weight_capacity`，背包件自重
    /// 已在 `current_weight` 侧计一次（equipped），不在 max 侧二次扣减。
    /// 此处把背包件自重设得很大（50.0）并断言 max 仍只 = BASE + capacity，与自重无关。
    #[test]
    fn compute_max_weight_worn_pack_self_weight_not_added_to_max() {
        // weight_capacity=30.0；下面把实际穿戴件自重设成 50.0（远大于容量）以坐实
        // 「自重不参与 max 公式」。
        let backpack_template =
            make_container_template("large_backpack", EQUIP_SLOT_CHEST, 7, 5, 30.0);
        let registry = ItemRegistry::from_map(HashMap::from([(
            "large_backpack".to_string(),
            backpack_template,
        )]));

        let mut inv = make_empty_inventory();
        let mut pack = make_container_item(1000, "large_backpack");
        pack.weight = 50.0; // 自重远大于 capacity，若被错误计入 max 则会撞红。
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(pack),
        );

        let w = compute_max_weight(&inv, &registry);
        let expected = BASE_CARRY_CAPACITY + 30.0; // 仅 capacity，与背包件自重(50.0)无关。
        assert!(
            (w - expected).abs() < f64::EPSILON,
            "期望 max = BASE({BASE_CARRY_CAPACITY}) + capacity(30.0) = {expected}，与背包自重(50.0)无关（决议 #3：自重已在 current 侧计、不占 max），实际 {w}——若 ≈ {} 说明自重被错误加进 max",
            expected + 50.0
        );
    }

    // 决议 #17：背包无专属槽，多个背包件骑在身体槽 worn 层；compute_max_weight 累加全部
    // 身体槽 worn 层带 container_spec 的件的 weight_capacity（受各槽 worn_cap：chest=3/legs=3）。
    #[test]
    fn compute_max_weight_sums_multiple_worn_packs() {
        let bp = make_container_template("large_backpack", EQUIP_SLOT_CHEST, 7, 5, 30.0);
        let wp = make_container_template("waist_pouch", EQUIP_SLOT_CHEST, 3, 3, 10.0);
        let cs = make_container_template("chest_satchel", EQUIP_SLOT_LEGS, 3, 4, 20.0);
        let registry = ItemRegistry::from_map(HashMap::from([
            ("large_backpack".to_string(), bp),
            ("waist_pouch".to_string(), wp),
            ("chest_satchel".to_string(), cs),
        ]));

        let mut inv = make_empty_inventory();
        // chest worn 两层（cap=3 内）：large_backpack + waist_pouch。
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents {
                worn: vec![
                    make_container_item(1, "large_backpack"),
                    make_container_item(2, "waist_pouch"),
                ],
                held: None,
            },
        );
        // legs worn 一层：chest_satchel。
        inv.equipped.insert(
            EQUIP_SLOT_LEGS.to_string(),
            SlotContents::worn_single(make_container_item(3, "chest_satchel")),
        );

        let w = compute_max_weight(&inv, &registry);
        let expected = BASE_CARRY_CAPACITY + 30.0 + 10.0 + 20.0;
        assert!(
            (w - expected).abs() < f64::EPSILON,
            "expected {expected}, got {w}"
        );
    }

    #[test]
    fn rebuild_containers_updates_max_weight() {
        let backpack_template =
            make_container_template("large_backpack", EQUIP_SLOT_CHEST, 7, 5, 30.0);
        let registry = ItemRegistry::from_map(HashMap::from([(
            "large_backpack".to_string(),
            backpack_template,
        )]));

        let mut inv = make_empty_inventory();
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(400, "large_backpack")),
        );
        inv.max_weight = 100.0; // stale value

        rebuild_containers_from_equipment(&mut inv, &registry);

        assert!(
            (inv.max_weight - (BASE_CARRY_CAPACITY + 30.0)).abs() < f64::EPSILON,
            "max_weight should be updated by rebuild, got {}",
            inv.max_weight
        );
    }

    // P0.5 — validate_move_semantics 背包槽校验

    fn make_backpack_registry_and_inventory() -> (ItemRegistry, PlayerInventory) {
        // 决议 #17：背包 equip_slot 指向身体槽。large_backpack→chest，
        // legs_pack→legs（供「错槽」用例），chest_bag→chest。
        let bp_template = make_container_template("large_backpack", EQUIP_SLOT_CHEST, 7, 5, 30.0);
        let wp_template = make_container_template("legs_pack", EQUIP_SLOT_LEGS, 3, 3, 10.0);
        let cs_template = make_container_template("chest_bag", EQUIP_SLOT_CHEST, 3, 4, 20.0);
        let registry = ItemRegistry::from_map(HashMap::from([
            ("large_backpack".to_string(), bp_template),
            ("legs_pack".to_string(), wp_template),
            ("chest_bag".to_string(), cs_template),
        ]));
        let inv = PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: Vec::new(),
                owner_instance_id: None,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        };
        (registry, inv)
    }

    // 决议 #17：背包件 equip_slot=chest，装入 chest worn 应成功。
    #[test]
    fn validate_move_semantics_accepts_container_equip_to_chest_worn() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
        let (registry, inv) = make_backpack_registry_and_inventory();
        let item = make_container_item(501, "large_backpack");
        let from = InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        };
        let to = InventoryLocationV1::Equip {
            slot: EquipSlotV1::Chest,
            state: EquipStateV1::Worn,
        };
        assert!(
            validate_move_semantics(&registry, &inv, &item, &from, &to).is_ok(),
            "equipping large_backpack (equip_slot=chest) to chest worn should succeed"
        );
    }

    // 非盔甲/非伪皮/非容器的杂项物品装 chest worn → 拒绝。
    #[test]
    fn validate_move_semantics_rejects_non_container_item_to_chest_worn() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
        let (registry, inv) = make_backpack_registry_and_inventory();
        // Use a misc item (no container_spec, not armor, not false skin).
        let misc_template = test_template("iron_ore", ItemCategory::Misc, 1, 1, 16);
        let registry_with_misc = ItemRegistry::from_map({
            let mut m = registry.templates.clone();
            m.insert("iron_ore".to_string(), misc_template);
            m
        });
        let item = make_test_item_instance(502, "iron_ore");
        let from = InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        };
        let to = InventoryLocationV1::Equip {
            slot: EquipSlotV1::Chest,
            state: EquipStateV1::Worn,
        };
        let err = validate_move_semantics(&registry_with_misc, &inv, &item, &from, &to)
            .expect_err("non-container/non-armor misc item should not equip to chest worn");
        assert!(
            err.contains("armor / false skin / container"),
            "expected body-slot type rejection, got: {err}"
        );
    }

    // 背包 equip_slot=legs，装入 chest worn → equip_slot 不匹配，拒绝。
    #[test]
    fn validate_move_semantics_rejects_wrong_slot_backpack() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
        let (registry, inv) = make_backpack_registry_and_inventory();
        // legs_pack has equip_slot=legs; try to equip to chest worn.
        let item = make_container_item(503, "legs_pack");
        let from = InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        };
        let to = InventoryLocationV1::Equip {
            slot: EquipSlotV1::Chest,
            state: EquipStateV1::Worn,
        };
        let err = validate_move_semantics(&registry, &inv, &item, &from, &to)
            .expect_err("legs_pack should not equip to chest worn");
        assert!(
            err.contains("legs"),
            "expected equip_slot mismatch error, got: {err}"
        );
    }

    #[test]
    fn validate_move_semantics_rejects_container_to_hotbar() {
        use crate::schema::inventory::InventoryLocationV1;
        let (registry, inv) = make_backpack_registry_and_inventory();
        let item = make_container_item(504, "large_backpack");
        let from = InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        };
        let to = InventoryLocationV1::Hotbar { index: 0 };
        let err = validate_move_semantics(&registry, &inv, &item, &from, &to)
            .expect_err("container item should not move to hotbar");
        assert!(err.contains("hotbar"), "expected hotbar error, got: {err}");
    }

    // plan-tarkov-backpack-v1 P0（交付物 #3 / 测试清单）— 非空拒卸硬门已移除：
    // 穿戴背包件即使其 pack_<instance_id> 容器非空，也允许整体卸下（塔科夫式套包）。
    // 内含物 spill/overflow 由 handle_inventory_move 卸包分支接管（见 e2e_*）。
    #[test]
    fn validate_move_semantics_allows_unequip_backpack_when_container_nonempty() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
        let (registry, mut inv) = make_backpack_registry_and_inventory();
        // Equip the backpack into chest worn.
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(505, "large_backpack")),
        );
        // 该背包件的 pack_505 容器非空。
        inv.containers.push(ContainerState {
            id: container_id_for_worn_pack(505),
            name: "大背包".to_string(),
            rows: 7,
            cols: 5,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: make_test_item_instance(99, "herb"),
            }],
            owner_instance_id: Some(505),
        });

        let item = make_container_item(505, "large_backpack");
        let from = InventoryLocationV1::Equip {
            slot: EquipSlotV1::Chest,
            state: EquipStateV1::Worn,
        };
        let to = InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        };
        assert!(
            validate_move_semantics(&registry, &inv, &item, &from, &to).is_ok(),
            "非空背包应允许整体卸下（非空拒卸硬门已移除）；内含物 spill/overflow 在 \
             handle_inventory_move 卸包分支处理，而非在校验层拒绝"
        );
    }

    #[test]
    fn validate_move_semantics_allows_unequip_backpack_when_container_empty() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
        let (registry, mut inv) = make_backpack_registry_and_inventory();
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(506, "large_backpack")),
        );
        // pack_506 容器为空。
        inv.containers.push(ContainerState {
            id: container_id_for_worn_pack(506),
            name: "大背包".to_string(),
            rows: 7,
            cols: 5,
            items: Vec::new(),
            owner_instance_id: None,
        });

        let item = make_container_item(506, "large_backpack");
        let from = InventoryLocationV1::Equip {
            slot: EquipSlotV1::Chest,
            state: EquipStateV1::Worn,
        };
        let to = InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        };
        assert!(
            validate_move_semantics(&registry, &inv, &item, &from, &to).is_ok(),
            "unequipping backpack with empty container should succeed"
        );
    }

    // ===== plan-tarkov-backpack-v1 P0 测试清单（≥9，含 e2e） =====

    /// 交付物 #2 — rebuild 创建/刷新 `pack_<id>` 容器时写 owner_instance_id = Some(instance_id)。
    #[test]
    fn rebuild_sets_owner_instance_id_on_pack_container() {
        let (registry, mut inv) = make_backpack_registry_and_inventory();
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(701, "large_backpack")),
        );

        let overflow = rebuild_containers_from_equipment(&mut inv, &registry);
        assert!(
            overflow.is_empty(),
            "穿背包后 rebuild 不应产生 overflow（新建空容器）；实际 {} 件",
            overflow.len()
        );

        let pack_id = container_id_for_worn_pack(701);
        let pack = inv
            .containers
            .iter()
            .find(|c| c.id == pack_id)
            .unwrap_or_else(|| panic!("rebuild 后应存在 `{pack_id}` 容器"));
        assert_eq!(
            pack.owner_instance_id,
            Some(701),
            "因为 rebuild 必须把 `{pack_id}` 容器的 owner_instance_id 写为穿戴背包件的 instance_id(701)，\
             实际 = {:?}",
            pack.owner_instance_id
        );
    }

    /// 交付物 #4 / 决议 #2 — 卸下非空背包：内含物 spill 进存活容器。
    /// 直测生产 seam `rebuild_and_drop_overflow`（handle_inventory_move 卸包分支调用同一函数）。
    #[test]
    fn unequip_nonempty_backpack_spills_contents_into_other_container() {
        let (registry, mut inv) = make_backpack_registry_and_inventory();
        // 装上背包件（large_backpack, pack_801）并放两件内含物。
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(801, "large_backpack")),
        );
        inv.containers.push(ContainerState {
            id: container_id_for_worn_pack(801),
            name: "大背包".to_string(),
            rows: 7,
            cols: 5,
            items: vec![
                PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: make_test_item_instance(10, "spirit_herb"),
                },
                PlacedItemState {
                    row: 1,
                    col: 0,
                    instance: make_test_item_instance(11, "bone_dust"),
                },
            ],
            owner_instance_id: Some(801),
        });

        // 模拟卸下：把背包件从 chest worn 移走（apply_inventory_move 已 detach），
        // 此时 pack_801 变孤儿。handle_inventory_move 卸包分支随即调 rebuild_and_drop_overflow。
        let removed = inv
            .equipped
            .get_mut(EQUIP_SLOT_CHEST)
            .and_then(|s| (!s.worn.is_empty()).then(|| s.worn.remove(0)));
        assert!(removed.is_some(), "应能从 chest worn 移除背包件");

        let mut dropped = DroppedLootRegistry::default();
        let dropped_ids = rebuild_and_drop_overflow(
            &mut inv,
            &registry,
            &mut dropped,
            [0.0, 64.0, 0.0],
            DimensionKind::Overworld,
        );

        // main_pack（5×7=35 格）能容下 spill → 不应有 overflow 掉落。
        assert!(
            dropped_ids.is_empty(),
            "main_pack 空且足够大，spill 应全部进容器、无 overflow 掉落；实际掉落 {dropped_ids:?}"
        );
        // 孤儿 pack_801 已被移除（不可 access）。
        assert!(
            !inv.containers
                .iter()
                .any(|c| c.id == container_id_for_worn_pack(801)),
            "卸下背包后其孤儿 pack_801 容器应被 rebuild 移除"
        );
        // 两件内含物 spill 进 main_pack。
        let main = inv
            .containers
            .iter()
            .find(|c| c.id == "main_pack")
            .expect("main_pack 存在");
        let main_ids: Vec<u64> = main.items.iter().map(|p| p.instance.instance_id).collect();
        assert!(
            main_ids.contains(&10) && main_ids.contains(&11),
            "spirit_herb(10) 与 bone_dust(11) 应 spill 进 main_pack；实际 main_pack ids = {main_ids:?}"
        );
    }

    /// 交付物 #4 / 决议 #2 红线 — 目标容器满时，overflow 内含物**转掉落物**（DroppedLootRegistry），
    /// 禁止静默丢失（断言掉落 count 守恒、非空、instance 守恒）。
    #[test]
    fn unequip_nonempty_backpack_overflow_drops_items_not_lost() {
        // 构造：唯一存活容器极小（1×1=1 格），背包内含 3 件 → 1 件 spill，2 件 overflow 掉落。
        let bp = make_container_template("small_pack", EQUIP_SLOT_CHEST, 3, 3, 10.0);
        let registry = ItemRegistry::from_map(HashMap::from([("small_pack".to_string(), bp)]));
        let mut inv = make_empty_inventory();
        // body_pocket（2×3=6 格）预填满——否则 rebuild 兜底创建空 body_pocket 会吸收全部 spill、
        // 不产生 overflow。填满后 spill 只能去 tiny（1 格），其余 overflow 掉落。
        inv.containers.push(ContainerState {
            id: BODY_POCKET_CONTAINER_ID.to_string(),
            name: "暗袋".to_string(),
            rows: BODY_POCKET_ROWS,
            cols: BODY_POCKET_COLS,
            items: (0..6)
                .map(|i| PlacedItemState {
                    row: (i / 3) as u8,
                    col: (i % 3) as u8,
                    instance: make_test_item_instance(200 + i as u64, "filler"),
                })
                .collect(),
            owner_instance_id: None,
        });
        // spill 容器：tiny 1×1。
        inv.containers.push(ContainerState {
            id: "tiny".to_string(),
            name: "tiny".to_string(),
            rows: 1,
            cols: 1,
            items: Vec::new(),
            owner_instance_id: None,
        });
        // 穿上 small_pack（pack_900），内含 3 件 1×1。
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(900, "small_pack")),
        );
        inv.containers.push(ContainerState {
            id: container_id_for_worn_pack(900),
            name: "small".to_string(),
            rows: 3,
            cols: 3,
            items: vec![
                PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: make_test_item_instance(20, "a"),
                },
                PlacedItemState {
                    row: 0,
                    col: 1,
                    instance: make_test_item_instance(21, "b"),
                },
                PlacedItemState {
                    row: 0,
                    col: 2,
                    instance: make_test_item_instance(22, "c"),
                },
            ],
            owner_instance_id: Some(900),
        });

        // 卸下：移走背包件。
        inv.equipped
            .get_mut(EQUIP_SLOT_CHEST)
            .map(|s| s.worn.remove(0));

        let mut dropped = DroppedLootRegistry::default();
        let dropped_ids = rebuild_and_drop_overflow(
            &mut inv,
            &registry,
            &mut dropped,
            [5.0, 64.0, 5.0],
            DimensionKind::Overworld,
        );

        // tiny 仅 1 格 → 1 件 spill 进 tiny，2 件 overflow 掉落（守恒：3 = 1 + 2）。
        assert_eq!(
            dropped_ids.len(),
            2,
            "tiny 容器仅 1 格，3 件内含物中 1 件 spill、2 件应转掉落物（守恒，禁止静默丢失）；实际掉落 {dropped_ids:?}"
        );
        assert_eq!(
            dropped.entries.len(),
            2,
            "DroppedLootRegistry 应含 2 条掉落条目（overflow 全部入世界，不丢失）"
        );
        // 掉落物 + spill 件 = 原 3 件（instance 守恒，无凭空消失）。
        let tiny = inv.containers.iter().find(|c| c.id == "tiny").unwrap();
        let mut all_ids: Vec<u64> = tiny.items.iter().map(|p| p.instance.instance_id).collect();
        all_ids.extend(dropped.entries.keys().copied());
        all_ids.sort_unstable();
        assert_eq!(
            all_ids,
            vec![20, 21, 22],
            "spill + 掉落必须守恒覆盖全部 3 件原内含物（20/21/22）；实际并集 = {all_ids:?}"
        );
        // 掉落条目的 item 实例非空且 dimension 正确。
        for id in &dropped_ids {
            let entry = dropped
                .entries
                .get(id)
                .unwrap_or_else(|| panic!("掉落 instance {id} 应在 registry"));
            assert_eq!(
                entry.dimension,
                DimensionKind::Overworld,
                "掉落物 dimension 应为玩家所在维度"
            );
            assert_eq!(
                entry.item.instance_id, *id,
                "掉落条目 item.instance_id 应与 key 一致（保留原 instance，不分配新 id）"
            );
        }
    }

    /// 交付物 #4 同步 — 穿背包路径触发 rebuild，`pack_<id>` 容器即时存在（P3 双击有容器可开）。
    #[test]
    fn equip_pack_creates_pack_container_via_rebuild() {
        let (registry, mut inv) = make_backpack_registry_and_inventory();
        // 穿上背包件后调 rebuild_and_drop_overflow（模拟 handle_inventory_move 穿包分支）。
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(950, "large_backpack")),
        );
        let mut dropped = DroppedLootRegistry::default();
        let dropped_ids = rebuild_and_drop_overflow(
            &mut inv,
            &registry,
            &mut dropped,
            [0.0, 64.0, 0.0],
            DimensionKind::Overworld,
        );
        assert!(
            dropped_ids.is_empty(),
            "穿包（新建空容器）不应产生 overflow 掉落；实际 {dropped_ids:?}"
        );
        let pack_id = container_id_for_worn_pack(950);
        let pack = inv
            .containers
            .iter()
            .find(|c| c.id == pack_id)
            .unwrap_or_else(|| panic!("穿包后 rebuild 应即时新建 `{pack_id}` 容器（P3 双击可开）"));
        assert_eq!(
            pack.owner_instance_id,
            Some(950),
            "穿包新建容器的 owner_instance_id 应为背包件 instance_id(950)"
        );
    }

    /// 交付物 #5 — 多背包 loadout：第一件复用占位、其余动态建容器，全部容器 id 正确。
    #[test]
    fn instantiate_remaps_all_worn_pack_placeholders() {
        // 两件 worn pack：chest + legs 各一。占位容器仅 `pack_grass_pouch` 一个 +
        // body_pocket（rebuild 兜底）。fixture 预置占位容器带一件预置物品，验证其不丢。
        let chest_pack = make_container_template("chest_pack", EQUIP_SLOT_CHEST, 3, 3, 10.0);
        let legs_pack = make_container_template("legs_pack", EQUIP_SLOT_LEGS, 3, 3, 8.0);
        let registry = ItemRegistry::from_map(HashMap::from([
            ("chest_pack".to_string(), chest_pack),
            ("legs_pack".to_string(), legs_pack),
        ]));

        let mut equipped: HashMap<String, SlotContents> = HashMap::new();
        equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(0, "chest_pack")),
        );
        equipped.insert(
            EQUIP_SLOT_LEGS.to_string(),
            SlotContents::worn_single(make_container_item(0, "legs_pack")),
        );

        // 占位容器（LOADOUT_PACK_PLACEHOLDER_CONTAINER_ID）携带一件预置物品。
        let loadout = LoadoutSpec {
            containers: vec![ContainerState {
                id: LOADOUT_PACK_PLACEHOLDER_CONTAINER_ID.to_string(),
                name: "占位包".to_string(),
                rows: 3,
                cols: 3,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: make_test_item_instance(0, "preset_item"),
                }],
                owner_instance_id: None,
            }],
            equipped,
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        };

        let mut alloc = InventoryInstanceIdAllocator::new(2000);
        let inv = instantiate_inventory_from_loadout(&loadout, &mut alloc, &registry)
            .expect("instantiate 多背包 loadout");

        // 占位 id 不应残留。
        assert!(
            !inv.containers
                .iter()
                .any(|c| c.id == LOADOUT_PACK_PLACEHOLDER_CONTAINER_ID),
            "静态占位 `{LOADOUT_PACK_PLACEHOLDER_CONTAINER_ID}` 必须已重映射，不应残留"
        );

        // 收集运行时两件 worn pack 的 instance_id。
        let worn_pack_ids: Vec<u64> = inv
            .equipped
            .values()
            .flat_map(|s| s.worn.iter())
            .filter(|i| {
                registry
                    .get(&i.template_id)
                    .is_some_and(|t| t.container_spec.is_some())
            })
            .map(|i| i.instance_id)
            .collect();
        assert_eq!(
            worn_pack_ids.len(),
            2,
            "应有两件运行时 worn pack；实际 {worn_pack_ids:?}"
        );

        // 两件 worn pack 各自都应有对应 `pack_<id>` 容器、owner 正确。
        for inst_id in &worn_pack_ids {
            let expected = container_id_for_worn_pack(*inst_id);
            let c = inv
                .containers
                .iter()
                .find(|c| c.id == expected)
                .unwrap_or_else(|| {
                    panic!(
                        "worn pack instance {inst_id} 应有容器 `{expected}`；实际 ids = {:?}",
                        inv.containers.iter().map(|c| &c.id).collect::<Vec<_>>()
                    )
                });
            assert_eq!(
                c.owner_instance_id,
                Some(*inst_id),
                "容器 `{expected}` 的 owner_instance_id 应为 {inst_id}"
            );
        }

        // 占位预置物品仍在某个 pack 容器（第一件 worn pack 复用占位容器，物品不丢）。
        let preset_still_present = inv.containers.iter().any(|c| {
            c.items
                .iter()
                .any(|p| p.instance.template_id == "preset_item")
        });
        assert!(
            preset_still_present,
            "占位容器的预置物品（preset_item）在重映射后不应丢失"
        );
    }

    /// 单背包 loadout 不应强依赖旧占位容器：`body_pocket` 是唯一必需静态容器，
    /// worn 背包件的 `pack_<instance_id>` 容器必须由实例化收尾 rebuild 派生。
    #[test]
    fn instantiate_single_worn_pack_without_placeholder_creates_runtime_container() {
        let chest_pack = make_container_template("chest_pack", EQUIP_SLOT_CHEST, 3, 3, 8.0);
        let registry =
            ItemRegistry::from_map(HashMap::from([("chest_pack".to_string(), chest_pack)]));

        let loadout = LoadoutSpec {
            containers: vec![ContainerState {
                id: BODY_POCKET_CONTAINER_ID.to_string(),
                name: "贴身口袋".to_string(),
                rows: BODY_POCKET_ROWS,
                cols: BODY_POCKET_COLS,
                items: Vec::new(),
                owner_instance_id: None,
            }],
            equipped: HashMap::from([(
                EQUIP_SLOT_CHEST.to_string(),
                SlotContents::worn_single(make_container_item(0, "chest_pack")),
            )]),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 23.0,
        };

        let mut alloc = InventoryInstanceIdAllocator::new(3000);
        let inv = instantiate_inventory_from_loadout(&loadout, &mut alloc, &registry)
            .expect("single worn-pack loadout should instantiate");

        let pack_instance_id = inv
            .equipped
            .get(EQUIP_SLOT_CHEST)
            .and_then(|slot| slot.worn.first())
            .map(|item| item.instance_id)
            .expect("chest worn pack should exist");
        let expected_container_id = container_id_for_worn_pack(pack_instance_id);
        let pack = inv
            .containers
            .iter()
            .find(|container| container.id == expected_container_id)
            .unwrap_or_else(|| {
                panic!(
                    "单背包 loadout 即使没有旧占位容器，也必须派生 `{expected_container_id}`；实际 ids = {:?}",
                    inv.containers
                        .iter()
                        .map(|container| &container.id)
                        .collect::<Vec<_>>()
                )
            });

        assert_eq!(pack.owner_instance_id, Some(pack_instance_id));
        assert_eq!((pack.rows, pack.cols), (3, 3));
    }

    /// qi_physics 锚点 — 跨包移动 lingering_owner_qi 守恒（随 instance 走，不重算/复制/蒸发）。
    #[test]
    fn move_item_across_packs_preserves_lingering_owner_qi() {
        use crate::schema::inventory::InventoryLocationV1;
        // 自建 registry：两个 container 模板 + 一个 misc 物品模板（apply_inventory_move 校验需 registry 命中）。
        let chest_pack = make_container_template("chest_pack", EQUIP_SLOT_CHEST, 3, 3, 10.0);
        let legs_pack = make_container_template("legs_pack", EQUIP_SLOT_LEGS, 3, 3, 8.0);
        let mut spirit_dust = make_container_template("spirit_dust", EQUIP_SLOT_CHEST, 1, 1, 0.0);
        // spirit_dust 是普通可移动物品（非容器）：清掉 container_spec、改 Misc 类、1×1。
        spirit_dust.container_spec = None;
        spirit_dust.category = ItemCategory::Misc;
        spirit_dust.grid_w = 1;
        spirit_dust.grid_h = 1;
        let registry = ItemRegistry::from_map(HashMap::from([
            ("chest_pack".to_string(), chest_pack),
            ("legs_pack".to_string(), legs_pack),
            ("spirit_dust".to_string(), spirit_dust),
        ]));
        let mut inv = make_empty_inventory();
        // 两件 worn pack：chest（chest_pack, pack_1001）+ legs（legs_pack, pack_1002）。
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(1001, "chest_pack")),
        );
        inv.equipped.insert(
            EQUIP_SLOT_LEGS.to_string(),
            SlotContents::worn_single(make_container_item(1002, "legs_pack")),
        );
        // 两个 pack 容器都建好（rebuild 后 owner 正确）。
        let _ = rebuild_containers_from_equipment(&mut inv, &registry);

        // 在 pack_1001 放一件带 lingering_owner_qi 的物品。
        let mut item = make_test_item_instance(55, "spirit_dust");
        item.lingering_owner_qi = Some(LingeringQi {
            owner: "Kizun".to_string(),
            expire_at: 12_345,
        });
        let qi_before = item.lingering_owner_qi.clone();
        let pack1 = inv
            .containers
            .iter_mut()
            .find(|c| c.id == container_id_for_worn_pack(1001))
            .expect("pack_1001 存在");
        pack1.items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: item,
        });

        // 跨包移动：pack_1001 → pack_1002。
        let from = InventoryLocationV1::Container {
            container_id: container_id_for_worn_pack(1001),
            row: 0,
            col: 0,
        };
        let to = InventoryLocationV1::Container {
            container_id: container_id_for_worn_pack(1002),
            row: 0,
            col: 0,
        };
        apply_inventory_move(&mut inv, &registry, 55, &from, &to).expect("跨包移动应成功");

        // 移动后 instance 55 应在 pack_1002，且 lingering_owner_qi 不变（守恒）。
        let pack2 = inv
            .containers
            .iter()
            .find(|c| c.id == container_id_for_worn_pack(1002))
            .expect("pack_1002 存在");
        let moved = pack2
            .items
            .iter()
            .find(|p| p.instance.instance_id == 55)
            .expect("instance 55 应在 pack_1002");
        assert_eq!(
            moved.instance.lingering_owner_qi, qi_before,
            "跨包移动是同一 instance 的位置变更：lingering_owner_qi 必须守恒不变（不重算/复制/蒸发）；\
             期望 {qi_before:?}，实际 {:?}",
            moved.instance.lingering_owner_qi
        );
    }

    // ===== plan-tarkov-backpack-v1 P2 测试清单（≥6 + e2e；穿戴态门控 + 软门控固化） =====

    /// P2 fixture — registry：两个 worn pack 模板（chest_pack/legs_pack）+ 一个 1×1 misc
    /// 可移动物品模板（dust）；validate_move_semantics 校验 moving item 的 template 必须命中
    /// registry，故 dust 须注册。返回 (registry, inventory)，inventory 为空（worn pack 由各
    /// 用例按需装备 + rebuild）。
    fn make_p2_registry() -> ItemRegistry {
        let chest_pack = make_container_template("chest_pack", EQUIP_SLOT_CHEST, 3, 3, 10.0);
        let legs_pack = make_container_template("legs_pack", EQUIP_SLOT_LEGS, 3, 3, 8.0);
        // 1×1 容量极小的 pack，用于「目标满 / 越界」边界用例。
        let tiny_pack = make_container_template("tiny_pack", EQUIP_SLOT_CHEST, 1, 1, 5.0);
        let mut dust = make_container_template("dust", EQUIP_SLOT_CHEST, 1, 1, 0.0);
        dust.container_spec = None;
        dust.category = ItemCategory::Misc;
        dust.grid_w = 1;
        dust.grid_h = 1;
        ItemRegistry::from_map(HashMap::from([
            ("chest_pack".to_string(), chest_pack),
            ("legs_pack".to_string(), legs_pack),
            ("tiny_pack".to_string(), tiny_pack),
            ("dust".to_string(), dust),
        ]))
    }

    /// P2 fixture — 空 inventory + 一个 5×7 main_pack 静态容器（源容器，存放待拖入的物品）。
    fn make_p2_inventory() -> PlayerInventory {
        let mut inv = make_empty_inventory();
        inv.containers.push(ContainerState {
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 5,
            cols: 7,
            items: Vec::new(),
            owner_instance_id: None,
        });
        inv
    }

    /// 交付物 #1 + #2（happy）— 拖入「穿戴中」的 pack_<id> 容器：门控放行，物品落位成功。
    /// 同时核实拖入持久化路径：apply_inventory_move 把物品写入 pack_<id>.items（落盘由
    /// flush_changed_player_inventories 自动承载，无额外入口；e2e 锁住跨重载）。
    #[test]
    fn move_item_into_worn_pack_container_succeeds() {
        use crate::schema::inventory::InventoryLocationV1;
        let registry = make_p2_registry();
        let mut inv = make_p2_inventory();
        // chest 穿戴 chest_pack（pack_2001），rebuild 建容器并回填 owner。
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(2001, "chest_pack")),
        );
        let _ = rebuild_containers_from_equipment(&mut inv, &registry);
        // main_pack（默认容器）里放一件 dust，准备拖入 pack_2001。
        let main = inv
            .containers
            .iter_mut()
            .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
            .expect("main_pack 存在");
        main.items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(70, "dust"),
        });

        let from = InventoryLocationV1::Container {
            container_id: MAIN_PACK_CONTAINER_ID.to_string(),
            row: 0,
            col: 0,
        };
        let to = InventoryLocationV1::Container {
            container_id: container_id_for_worn_pack(2001),
            row: 0,
            col: 0,
        };
        apply_inventory_move(&mut inv, &registry, 70, &from, &to)
            .expect("拖入穿戴中的 pack_2001 应成功（owner 在 chest worn 层）");

        let pack = inv
            .containers
            .iter()
            .find(|c| c.id == container_id_for_worn_pack(2001))
            .expect("pack_2001 存在");
        assert!(
            pack.items.iter().any(|p| p.instance.instance_id == 70),
            "因为目标 pack_2001 当前穿戴中，门控应放行且 dust(70) 落位进该容器；\
             实际 pack_2001 内含 ids = {:?}",
            pack.items
                .iter()
                .map(|p| p.instance.instance_id)
                .collect::<Vec<_>>()
        );
    }

    /// 交付物 #2（错误分支）— 拖入「已卸下（非穿戴）」的 pack_<id> 容器：门控拒绝，
    /// 返回带修复线索的 Err。背包件已从身体槽卸到 main_pack（格子），其 pack_<id> 容器仍残留。
    #[test]
    fn move_item_into_unworn_pack_container_rejected() {
        use crate::schema::inventory::InventoryLocationV1;
        let registry = make_p2_registry();
        let mut inv = make_p2_inventory();
        // pack_3001 容器存在（owner_instance_id=3001），但背包件 3001 不在任何 worn 层
        // ——已卸到 main_pack 当普通物品。
        inv.containers.push(ContainerState {
            id: container_id_for_worn_pack(3001),
            name: "已卸下的胸包".to_string(),
            rows: 3,
            cols: 3,
            items: Vec::new(),
            owner_instance_id: Some(3001),
        });
        let main = inv
            .containers
            .iter_mut()
            .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
            .expect("main_pack 存在");
        // 背包件本体卸在 main_pack（非 worn），以及一件待拖入的 dust。
        main.items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: make_container_item(3001, "chest_pack"),
        });
        main.items.push(PlacedItemState {
            row: 0,
            col: 3,
            instance: make_test_item_instance(71, "dust"),
        });

        let from = InventoryLocationV1::Container {
            container_id: MAIN_PACK_CONTAINER_ID.to_string(),
            row: 0,
            col: 3,
        };
        let to = InventoryLocationV1::Container {
            container_id: container_id_for_worn_pack(3001),
            row: 0,
            col: 0,
        };
        let err = apply_inventory_move(&mut inv, &registry, 71, &from, &to)
            .expect_err("拖入未穿戴的 pack_3001 应被穿戴态门控拒绝（owner 3001 不在任何 worn 层）");
        assert!(
            err.contains("背包未穿戴") && err.contains("3001"),
            "期望带修复线索的拒绝（提示背包未穿戴 + owner instance id），因为塔科夫式语义下卸下的包是死容器；\
             实际 err = {err}"
        );
        // 物品未落位（仍在 main_pack）。
        let pack = inv
            .containers
            .iter()
            .find(|c| c.id == container_id_for_worn_pack(3001))
            .expect("pack_3001 仍存在");
        assert!(
            pack.items.is_empty(),
            "拒绝后 dust(71) 不应进入 pack_3001；实际内含 {} 件",
            pack.items.len()
        );
    }

    /// 交付物 #2（错误分支）— 拖入「不存在」的 pack_<id> 容器：owner 不在 worn 层 → 拒绝。
    /// （pack_<id> 容器本身都不存在；穿戴态门控先于落位层 unknown-container 报错命中。）
    #[test]
    fn move_item_into_nonexistent_pack_container_rejected() {
        use crate::schema::inventory::InventoryLocationV1;
        let registry = make_p2_registry();
        let mut inv = make_p2_inventory();
        let main = inv
            .containers
            .iter_mut()
            .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
            .expect("main_pack 存在");
        main.items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(72, "dust"),
        });

        let from = InventoryLocationV1::Container {
            container_id: MAIN_PACK_CONTAINER_ID.to_string(),
            row: 0,
            col: 0,
        };
        // pack_9999 既无容器也无 worn owner。
        let to = InventoryLocationV1::Container {
            container_id: container_id_for_worn_pack(9999),
            row: 0,
            col: 0,
        };
        let err = apply_inventory_move(&mut inv, &registry, 72, &from, &to)
            .expect_err("拖入不存在/未穿戴的 pack_9999 应被拒绝");
        assert!(
            err.contains("背包未穿戴") && err.contains("9999"),
            "期望穿戴态门控在落位前拒绝（owner 9999 不在 worn 层）；实际 err = {err}"
        );
    }

    /// 交付物 #2（状态转换）— 两个都穿戴中的 pack 之间移动：门控对源容器无要求、
    /// 目标 pack owner 在 worn 层 → 放行成功。
    #[test]
    fn move_item_between_two_worn_packs_succeeds() {
        use crate::schema::inventory::InventoryLocationV1;
        let registry = make_p2_registry();
        let mut inv = make_p2_inventory();
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(4001, "chest_pack")),
        );
        inv.equipped.insert(
            EQUIP_SLOT_LEGS.to_string(),
            SlotContents::worn_single(make_container_item(4002, "legs_pack")),
        );
        let _ = rebuild_containers_from_equipment(&mut inv, &registry);
        // 在 pack_4001 放一件 dust。
        let pack1 = inv
            .containers
            .iter_mut()
            .find(|c| c.id == container_id_for_worn_pack(4001))
            .expect("pack_4001 存在");
        pack1.items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(73, "dust"),
        });

        let from = InventoryLocationV1::Container {
            container_id: container_id_for_worn_pack(4001),
            row: 0,
            col: 0,
        };
        let to = InventoryLocationV1::Container {
            container_id: container_id_for_worn_pack(4002),
            row: 0,
            col: 0,
        };
        apply_inventory_move(&mut inv, &registry, 73, &from, &to)
            .expect("两个穿戴中的 pack 之间移动应成功");

        let pack2 = inv
            .containers
            .iter()
            .find(|c| c.id == container_id_for_worn_pack(4002))
            .expect("pack_4002 存在");
        assert!(
            pack2.items.iter().any(|p| p.instance.instance_id == 73),
            "dust(73) 应从 pack_4001 转入 pack_4002；实际 pack_4002 ids = {:?}",
            pack2
                .items
                .iter()
                .map(|p| p.instance.instance_id)
                .collect::<Vec<_>>()
        );
    }

    /// 交付物 #4 / 决议 #5（软门控）— 超重（current > max）时拖入穿戴中的 pack 仍成功：
    /// 超限只打 OverloadedMarker，不在 move 路径硬拒绝。本测试固化「move 路径无重量门控」契约。
    #[test]
    fn move_into_pack_when_overloaded_still_succeeds() {
        use crate::schema::inventory::InventoryLocationV1;
        let registry = make_p2_registry();
        let mut inv = make_p2_inventory();
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(5001, "chest_pack")),
        );
        let _ = rebuild_containers_from_equipment(&mut inv, &registry);
        // 人为压低 max_weight 使其远小于实际负重，模拟超载态。
        inv.max_weight = 0.01;
        // main_pack 放一件重 dust。
        let mut heavy = make_test_item_instance(74, "dust");
        heavy.weight = 99.0;
        let main = inv
            .containers
            .iter_mut()
            .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
            .expect("main_pack 存在");
        main.items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: heavy,
        });
        // 确认确实超载（current_weight > max_weight）。
        assert!(
            calculate_current_weight(&inv) > inv.max_weight,
            "前置：构造的 inventory 应处于超载态"
        );

        let from = InventoryLocationV1::Container {
            container_id: MAIN_PACK_CONTAINER_ID.to_string(),
            row: 0,
            col: 0,
        };
        let to = InventoryLocationV1::Container {
            container_id: container_id_for_worn_pack(5001),
            row: 0,
            col: 0,
        };
        apply_inventory_move(&mut inv, &registry, 74, &from, &to).expect(
            "决议 #5 软门控：超载态下拖入穿戴中的 pack 仍应成功；move 路径不做重量硬拒绝（仅 OverloadedMarker debuff）",
        );
        let pack = inv
            .containers
            .iter()
            .find(|c| c.id == container_id_for_worn_pack(5001))
            .expect("pack_5001 存在");
        assert!(
            pack.items.iter().any(|p| p.instance.instance_id == 74),
            "超载态下重物 dust(74) 仍应落位进 pack_5001（软门控）"
        );
    }

    /// 交付物（边界：目标容器满）— 目标 pack 落位越界（无空位）→ 落位层拒绝（穿戴态门控放行后，
    /// displaced_at_target 的 bounds 检查命中）。固化「门控放行 ≠ 一定落位成功」。
    #[test]
    fn move_into_full_pack_rejected_no_fit() {
        use crate::schema::inventory::InventoryLocationV1;
        let registry = make_p2_registry();
        let mut inv = make_p2_inventory();
        // 穿戴 1×1 的 tiny_pack（pack_6001），rebuild 建容器。
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(6001, "tiny_pack")),
        );
        let _ = rebuild_containers_from_equipment(&mut inv, &registry);
        // tiny_pack 唯一格 (0,0) 已被占满。
        let pack = inv
            .containers
            .iter_mut()
            .find(|c| c.id == container_id_for_worn_pack(6001))
            .expect("pack_6001 存在");
        pack.items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(80, "dust"),
        });
        // main_pack 放一件待拖入的 dust。
        let main = inv
            .containers
            .iter_mut()
            .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
            .expect("main_pack 存在");
        main.items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(81, "dust"),
        });

        let from = InventoryLocationV1::Container {
            container_id: MAIN_PACK_CONTAINER_ID.to_string(),
            row: 0,
            col: 0,
        };
        // 目标 (0,1)：1×1 容器越界（col 1 + 1 > cols 1）→ 落位层 no-fit 拒绝。
        let to = InventoryLocationV1::Container {
            container_id: container_id_for_worn_pack(6001),
            row: 0,
            col: 1,
        };
        let err = apply_inventory_move(&mut inv, &registry, 81, &from, &to).expect_err(
            "穿戴态门控放行后，落位层应因 1×1 容器越界（无空位）拒绝；门控放行 ≠ 一定能放下",
        );
        assert!(
            err.contains("bounds") || err.contains("overlaps"),
            "期望落位层 no-fit 拒绝（越界/重叠），因为 tiny_pack 仅 1×1 且已满；实际 err = {err}"
        );
        // dust(81) 未进 pack_6001。
        let pack = inv
            .containers
            .iter()
            .find(|c| c.id == container_id_for_worn_pack(6001))
            .expect("pack_6001 仍存在");
        assert!(
            !pack.items.iter().any(|p| p.instance.instance_id == 81),
            "no-fit 拒绝后 dust(81) 不应进入 pack_6001"
        );
    }

    // (决议 #17/#9/#8) back_pack/waist_pouch/chest_satchel EquipSlotV1 variant 已删除，
    // 原 equip_slot_v1_backpack_variants_serde_roundtrip 测试随之移除。

    // ItemCategory serde pins

    #[test]
    fn item_category_block_serde_pin() {
        let serialized =
            serde_json::to_string(&ItemCategory::Block).expect("serialize Block category");
        assert_eq!(
            serialized, "\"Block\"",
            "expected ItemCategory::Block to serialize as the explicit protocol literal"
        );

        let deserialized: ItemCategory =
            serde_json::from_str("\"Block\"").expect("deserialize Block category literal");
        assert_eq!(deserialized, ItemCategory::Block);
    }

    #[test]
    fn item_category_invalid_variant_is_rejected() {
        let result = serde_json::from_str::<ItemCategory>("\"InvalidVariant\"");
        assert!(
            result.is_err(),
            "expected invalid ItemCategory protocol literal to be rejected, got {result:?}"
        );
    }

    #[test]
    fn item_category_container_serde_roundtrip() {
        let cat = ItemCategory::Container;
        let json = serde_json::to_string(&cat).expect("serialize Container category");
        let back: ItemCategory =
            serde_json::from_str(&json).expect("deserialize Container category");
        assert_eq!(back, cat);
    }

    // =========== plan-backpack-equip-v1 P3 — 背包耐久扣减与破损溢出测试 ===========

    /// 构造一个携带草编囊的 registry + inventory（耐久 cost_per_op = 0.008，
    /// durability 初始值由调用方通过 `durability` 参数控制）。
    fn make_worn_grass_pouch_setup(
        durability: f64,
        with_container_items: bool,
    ) -> (ItemRegistry, PlayerInventory) {
        let template = ItemTemplate {
            id: "worn_grass_pouch".to_string(),
            display_name: "草编囊（磨损）".to_string(),
            category: ItemCategory::Container,
            placeable: None,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 2,
            base_weight: 0.3,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 0.5,
            description: "test".to_string(),
            effect: None,
            cast_duration_ms: DEFAULT_CAST_DURATION_MS,
            cooldown_ms: DEFAULT_COOLDOWN_MS,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: Some(ContainerSpec {
                rows: 3,
                cols: 3,
                weight_capacity: 10.0,
                // 决议 #17：背包无专属槽，equip_slot 指向身体槽（chest），骑在 chest worn 层。
                equip_slot: EQUIP_SLOT_CHEST.to_string(),
                durability_cost_per_op: 0.008,
                attrition_exempt: false,
                accept_filter: None,
            }),
            shield_spec: None,

            shelflife_profile: None,
            shelflife_track: None,
        };
        let registry =
            ItemRegistry::from_map(HashMap::from([("worn_grass_pouch".to_string(), template)]));

        // 构造一个装备了草编囊的 inventory。
        let backpack_instance = ItemInstance {
            instance_id: 1,
            template_id: "worn_grass_pouch".to_string(),
            display_name: "草编囊（磨损）".to_string(),
            grid_w: 1,
            grid_h: 2,
            weight: 0.3,
            rarity: ItemRarity::Common,
            description: "test".to_string(),
            stack_count: 1,
            spirit_quality: 0.5,
            durability,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        };

        let container_items = if with_container_items {
            vec![
                PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: make_test_item_instance(10, "spirit_herb"),
                },
                PlacedItemState {
                    row: 1,
                    col: 0,
                    instance: make_test_item_instance(11, "bone_dust"),
                },
            ]
        } else {
            Vec::new()
        };

        let mut inv = make_empty_inventory();
        // 决议 #17：背包件骑在 chest worn 层；容器 id = pack_<instance_id> = "pack_1"。
        let pack_container_id = container_id_for_worn_pack(backpack_instance.instance_id);
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(backpack_instance),
        );
        inv.containers.push(ContainerState {
            id: pack_container_id,
            name: "草编囊".to_string(),
            rows: 3,
            cols: 3,
            items: container_items,
            owner_instance_id: None,
        });
        inv.max_weight = BASE_CARRY_CAPACITY + 10.0;

        (registry, inv)
    }

    // P3.1.1 — apply_backpack_wear 正常扣减

    #[test]
    fn apply_backpack_wear_deducts_cost_per_op() {
        let (registry, mut inv) = make_worn_grass_pouch_setup(1.0, false);
        let event = apply_backpack_wear(&mut inv, &registry, &container_id_for_worn_pack(1));
        assert!(
            event.is_none(),
            "durability 1.0 minus 0.008 should not break yet"
        );
        let durability = inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn[0].durability;
        assert!(
            (durability - 0.992).abs() < 1e-9,
            "expected durability ≈ 0.992 after one wear, got {durability}"
        );
    }

    #[test]
    fn apply_backpack_wear_multiple_ops_reduce_durability_cumulatively() {
        let (registry, mut inv) = make_worn_grass_pouch_setup(0.1, false);
        // 12 ops × 0.008 = 0.096 > 0.1 − 0.008×12 = 0.004; not yet broken after 12.
        for _ in 0..12 {
            let event = apply_backpack_wear(&mut inv, &registry, &container_id_for_worn_pack(1));
            assert!(
                event.is_none(),
                "should not break before 0.1/0.008 ≈ 12.5 ops"
            );
        }
        let durability = inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn[0].durability;
        let expected = 0.1 - 12.0 * 0.008;
        assert!(
            (durability - expected).abs() < 1e-9,
            "expected durability ≈ {expected} after 12 ops, got {durability}"
        );
    }

    // P3.1.2 — body_pocket 操作不扣减

    #[test]
    fn apply_backpack_wear_body_pocket_does_not_deduct() {
        let (registry, mut inv) = make_worn_grass_pouch_setup(1.0, false);
        let event = apply_backpack_wear(&mut inv, &registry, BODY_POCKET_CONTAINER_ID);
        assert!(
            event.is_none(),
            "body_pocket should never trigger wear deduction"
        );
        // 装备耐久不变。
        let durability = inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn[0].durability;
        assert!(
            (durability - 1.0).abs() < f64::EPSILON,
            "worn_grass_pouch durability should be unchanged, got {durability}"
        );
    }

    // P3.1.3 — 未知 container_id 不扣减

    #[test]
    fn apply_backpack_wear_unknown_container_id_no_deduct() {
        let (registry, mut inv) = make_worn_grass_pouch_setup(1.0, false);
        let event = apply_backpack_wear(&mut inv, &registry, "totally_unknown_container");
        assert!(event.is_none(), "unknown container id should be a no-op");
    }

    // P3.1.4 — 多次扣减到 ≤ε 时返回 BackpackBreakEvent

    #[test]
    fn apply_backpack_wear_returns_break_event_when_durability_depleted() {
        // worn_grass_pouch: durability_cost_per_op = 0.008，从 0.3 开始（P2 默认值）。
        // 0.3 / 0.008 = 37.5，所以第 38 次调用会触发破损。
        let (registry, mut inv) = make_worn_grass_pouch_setup(0.3, false);

        for i in 1..38 {
            let event = apply_backpack_wear(&mut inv, &registry, &container_id_for_worn_pack(1));
            assert!(
                event.is_none(),
                "op {i}/38 should not break yet (durability = {})",
                inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn[0].durability
            );
        }
        // 第 38 次——应触发破损。
        let event = apply_backpack_wear(&mut inv, &registry, &container_id_for_worn_pack(1));
        assert!(
            event.is_some(),
            "38th op should trigger BackpackBreakEvent (durability = {})",
            inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn[0].durability
        );
        let ev = event.unwrap();
        assert_eq!(
            ev.backpack_instance_id, 1,
            "break event backpack_instance_id mismatch（应为 worn pack 的 instance_id）"
        );
        assert_eq!(
            ev.container_id,
            container_id_for_worn_pack(1),
            "break event container_id mismatch（应为 pack_<instance_id>）"
        );
    }

    // P3.1.5 — cost_per_op = 0.0 时永远不扣减（无损耗背包）

    #[test]
    fn apply_backpack_wear_zero_cost_per_op_never_deducts() {
        let template = make_container_template("lossless_bag", EQUIP_SLOT_CHEST, 5, 5, 20.0);
        // make_container_template 默认 cost_per_op = 0.0。
        let registry =
            ItemRegistry::from_map(HashMap::from([("lossless_bag".to_string(), template)]));
        let mut inv = make_empty_inventory();
        let bag = ItemInstance {
            instance_id: 200,
            template_id: "lossless_bag".to_string(),
            display_name: "lossless".to_string(),
            grid_w: 2,
            grid_h: 3,
            weight: 0.5,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 0.001, // 极低耐久但 cost=0 不应触发破损
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        };
        inv.equipped
            .insert(EQUIP_SLOT_CHEST.to_string(), SlotContents::worn_single(bag));

        let event = apply_backpack_wear(&mut inv, &registry, &container_id_for_worn_pack(200));
        assert!(
            event.is_none(),
            "zero cost_per_op should never trigger wear even at low durability"
        );
        let durability = inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn[0].durability;
        assert!(
            (durability - 0.001).abs() < f64::EPSILON,
            "durability should be unchanged with cost_per_op=0.0"
        );
    }

    // P3.1.6 — slot 未装备时返回 None

    #[test]
    fn apply_backpack_wear_missing_equip_returns_none() {
        let (registry, mut inv) = make_worn_grass_pouch_setup(1.0, false);
        // 试图对一个未穿戴的 pack 容器（pack_999）扣减——无对应 worn 背包件 → None。
        let event = apply_backpack_wear(&mut inv, &registry, &container_id_for_worn_pack(999));
        assert!(
            event.is_none(),
            "empty equip slot should return None, not panic"
        );
    }

    // P3.2.1 — handle_backpack_break 移除容器 + 返回 spilled_items + max_weight 下降

    #[test]
    fn handle_backpack_break_spills_items_and_removes_container() {
        let (registry, mut inv) = make_worn_grass_pouch_setup(0.0, true);

        let initial_max_weight = inv.max_weight;
        let outcome = handle_backpack_break(&mut inv, &registry, &container_id_for_worn_pack(1))
            .expect("handle_backpack_break should return Some for valid slot");

        // 背包件已从 chest worn 层移除（空 SlotContents 可能保留，断言 worn 为空）。
        assert!(
            inv.equipped
                .get(EQUIP_SLOT_CHEST)
                .map(|s| s.worn.is_empty())
                .unwrap_or(true),
            "backpack should be removed from chest worn after break"
        );

        // 容器（pack_1）已从 containers 移除。
        assert!(
            inv.containers
                .iter()
                .all(|c| c.id != container_id_for_worn_pack(1)),
            "container should be removed from containers after break"
        );

        // 溢出物品包含原容器内的所有物品。
        assert_eq!(
            outcome.spilled_items.len(),
            2,
            "expected 2 spilled items (spirit_herb + bone_dust)"
        );
        let spilled_ids: Vec<u64> = outcome
            .spilled_items
            .iter()
            .map(|i| i.instance_id)
            .collect();
        assert!(
            spilled_ids.contains(&10),
            "spirit_herb (id=10) should be spilled"
        );
        assert!(
            spilled_ids.contains(&11),
            "bone_dust (id=11) should be spilled"
        );

        // 破损的背包物品实例正确返回。
        assert_eq!(
            outcome.backpack_item.template_id, "worn_grass_pouch",
            "backpack_item template_id mismatch"
        );

        // max_weight 下降（去掉 10.0 的 weight_capacity）。
        let expected_new_max = BASE_CARRY_CAPACITY; // 15.0
        assert!(
            (outcome.new_max_weight - expected_new_max).abs() < f64::EPSILON,
            "expected new_max_weight={expected_new_max}, got {}",
            outcome.new_max_weight
        );
        assert!(
            outcome.new_max_weight < initial_max_weight,
            "max_weight should drop after backpack break"
        );
        // inventory 本身的 max_weight 也已更新。
        assert!(
            (inv.max_weight - expected_new_max).abs() < f64::EPSILON,
            "inventory.max_weight should be refreshed to {expected_new_max}, got {}",
            inv.max_weight
        );
    }

    // P3.2.2 — handle_backpack_break 对空容器（无溢出物品）

    #[test]
    fn handle_backpack_break_empty_container_spills_nothing() {
        let (registry, mut inv) = make_worn_grass_pouch_setup(0.0, false);

        let outcome = handle_backpack_break(&mut inv, &registry, &container_id_for_worn_pack(1))
            .expect("break on empty container should still succeed");

        assert!(
            outcome.spilled_items.is_empty(),
            "no items should be spilled from an empty container"
        );
        assert_eq!(
            outcome.backpack_item.template_id, "worn_grass_pouch",
            "backpack_item should still be returned even with empty container"
        );
    }

    // P3.2.3 — handle_backpack_break 对 body_pocket 返回 None

    #[test]
    fn handle_backpack_break_body_pocket_returns_none() {
        let (registry, mut inv) = make_worn_grass_pouch_setup(0.0, false);
        let outcome = handle_backpack_break(&mut inv, &registry, BODY_POCKET_CONTAINER_ID);
        assert!(
            outcome.is_none(),
            "body_pocket should not trigger backpack break"
        );
    }

    // P3.2.4 — handle_backpack_break 对未装备槽返回 None

    #[test]
    fn handle_backpack_break_unequipped_slot_returns_none() {
        let (registry, mut inv) = make_worn_grass_pouch_setup(0.0, false);
        // 对一个未穿戴的 pack 容器（pack_999）破损——无对应 worn 背包件 → None。
        let outcome = handle_backpack_break(&mut inv, &registry, &container_id_for_worn_pack(999));
        assert!(
            outcome.is_none(),
            "unequipped slot should return None from handle_backpack_break"
        );
    }

    // P3.2.5 — handle_backpack_break 当容器不在 containers 列表时仍正常工作（spilled 为空）

    #[test]
    fn handle_backpack_break_missing_container_entry_spills_nothing() {
        let (registry, mut inv) = make_worn_grass_pouch_setup(0.0, false);
        // 手动移除容器，模拟 containers 与 equipped 不同步场景。
        inv.containers
            .retain(|c| c.id != container_id_for_worn_pack(1));

        let outcome = handle_backpack_break(&mut inv, &registry, &container_id_for_worn_pack(1))
            .expect("should succeed even without matching container");

        assert!(
            outcome.spilled_items.is_empty(),
            "no items to spill when container entry is missing"
        );
    }

    // P3 真实物品模板 — worn_grass_pouch（P2 草编囊）操作 38 次后破损

    #[test]
    fn worn_grass_pouch_breaks_after_38_ops_from_30_percent_durability() {
        // P2 default: durability=0.3, cost_per_op=0.008
        // 0.3 / 0.008 = 37.5 → floor = 37，第 38 次触发破损
        let (registry, mut inv) = make_worn_grass_pouch_setup(0.3, false);

        for i in 1..=37 {
            let ev = apply_backpack_wear(&mut inv, &registry, &container_id_for_worn_pack(1));
            assert!(
                ev.is_none(),
                "op {i}: should not break before op 38 (durability={})",
                inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn[0].durability
            );
        }
        let ev = apply_backpack_wear(&mut inv, &registry, &container_id_for_worn_pack(1));
        assert!(
            ev.is_some(),
            "38th op should return BackpackBreakEvent, durability={}",
            inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn[0].durability
        );
        let ev = ev.unwrap();
        assert_eq!(ev.backpack_instance_id, 1);
        assert_eq!(ev.container_id, container_id_for_worn_pack(1));
    }

    // P3 BackpackBreakEvent PartialEq pin 测试

    #[test]
    fn backpack_break_event_partial_eq_and_clone() {
        let ev1 = BackpackBreakEvent {
            backpack_instance_id: 1,
            container_id: container_id_for_worn_pack(1),
        };
        let ev2 = ev1.clone();
        assert_eq!(ev1, ev2, "BackpackBreakEvent should implement PartialEq");
        let ev3 = BackpackBreakEvent {
            backpack_instance_id: 2,
            container_id: container_id_for_worn_pack(2),
        };
        assert_ne!(ev1, ev3, "different backpack instances should not be equal");
    }

    // plan-dandao-path-v1 — ExtraHand0/ExtraHand1 equip slot tests

    fn make_weapon_template(id: &str) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: id.to_string(),
            category: ItemCategory::Weapon,
            placeable: None,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 2,
            base_weight: 2.0,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 0.5,
            description: "test weapon".to_string(),
            effect: None,
            cast_duration_ms: 0,
            cooldown_ms: 0,
            weapon_spec: Some(WeaponSpec {
                weapon_kind: crate::combat::weapon::WeaponKind::Sword,
                base_attack: 5.0,
                quality_tier: 0,
                durability_max: 100.0,
                qi_cost_mul: 1.0,
            }),
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shield_spec: None,

            shelflife_profile: None,
            shelflife_track: None,
        }
    }

    fn make_misc_template(id: &str) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: id.to_string(),
            category: ItemCategory::Misc,
            placeable: None,
            max_stack_count: 64,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.1,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 0.0,
            description: "misc".to_string(),
            effect: None,
            cast_duration_ms: 0,
            cooldown_ms: 0,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shield_spec: None,

            shelflife_profile: None,
            shelflife_track: None,
        }
    }

    // 决议 #17：false_skin 专属槽已删，伪皮改穿 chest worn（身体槽接受 armor/false skin/container）。
    #[test]
    fn validate_move_semantics_accepts_low_cost_disguise_items_to_chest_worn() {
        use crate::combat::tuike::{CAMOUFLAGE_NET_ITEM_ID, DISGUISE_WRAP_ITEM_ID};
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};

        let registry = ItemRegistry::from_map(HashMap::from([
            (
                DISGUISE_WRAP_ITEM_ID.to_string(),
                make_misc_template(DISGUISE_WRAP_ITEM_ID),
            ),
            (
                CAMOUFLAGE_NET_ITEM_ID.to_string(),
                make_misc_template(CAMOUFLAGE_NET_ITEM_ID),
            ),
        ]));
        let inventory = make_empty_inventory();
        let from = InventoryLocationV1::Container {
            container_id: MAIN_PACK_CONTAINER_ID.to_string(),
            row: 0,
            col: 0,
        };
        let to = InventoryLocationV1::Equip {
            slot: EquipSlotV1::Chest,
            state: EquipStateV1::Worn,
        };

        for (instance_id, template_id) in
            [(10, DISGUISE_WRAP_ITEM_ID), (11, CAMOUFLAGE_NET_ITEM_ID)]
        {
            let item = make_test_item_instance(instance_id, template_id);
            assert!(
                validate_move_semantics(&registry, &inventory, &item, &from, &to).is_ok(),
                "{template_id} (false skin) should be equippable to chest worn"
            );
        }
    }

    // Bug2（真机回归）— fake_spirit_hide 真实数据为 category=misc（materials.toml），
    // 但正典为蛛丝型伪皮。live-equip 校验（validate_move_semantics）必须放行其入胸槽 worn，
    // 否则「出生自带却拖不回胸槽」自相矛盾。用真实 registry 证明放行靠 false_skin 闸而非 category。
    #[test]
    fn validate_move_semantics_accepts_fake_spirit_hide_to_chest_worn_with_real_registry() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};

        let registry = load_item_registry().expect("real item registry loads");
        // 前置断言：fake_spirit_hide 真实 category 不是 Armor / Container，放行只能靠 false_skin 闸。
        let template = registry
            .get("fake_spirit_hide")
            .expect("fake_spirit_hide template registered");
        assert!(
            !matches!(template.category, ItemCategory::Armor),
            "fake_spirit_hide 真实 category 应非 Armor（证明放行靠 false_skin 闸）"
        );
        assert!(
            template.container_spec.is_none(),
            "fake_spirit_hide 非容器件（证明放行靠 false_skin 闸）"
        );

        let inventory = make_empty_inventory();
        let item = make_test_item_instance(70, "fake_spirit_hide");
        let from = InventoryLocationV1::Container {
            container_id: MAIN_PACK_CONTAINER_ID.to_string(),
            row: 0,
            col: 0,
        };
        let to = InventoryLocationV1::Equip {
            slot: EquipSlotV1::Chest,
            state: EquipStateV1::Worn,
        };
        assert!(
            validate_move_semantics(&registry, &inventory, &item, &from, &to).is_ok(),
            "fake_spirit_hide（伪灵皮）必须能拖进胸槽 worn（live-equip 与 instantiate 一致）"
        );
    }

    // Bug2（真机回归）— instantiate（绕校验）与 live-equip（走校验）对 fake_spirit_hide 一致：
    // 出生自带后必须能卸下再拖回。default.toml 把 fake_spirit_hide 放 chest worn，
    // 实例化后它确实在 chest.worn，且其 validate_move_semantics 放行（上一条已证）。
    #[test]
    fn fake_spirit_hide_instantiate_matches_live_equip_for_chest_worn() {
        let registry = load_item_registry().expect("real item registry loads");
        let loadout = load_default_loadout(&registry).expect("default loadout loads");
        let mut alloc = InventoryInstanceIdAllocator::default();
        let inv = instantiate_inventory_from_loadout(&loadout, &mut alloc, &registry)
            .expect("instantiate default loadout");

        let chest = inv
            .equipped
            .get(EQUIP_SLOT_CHEST)
            .expect("chest slot present after instantiate");
        let chest_worn: Vec<&str> = chest.worn.iter().map(|i| i.template_id.as_str()).collect();
        assert_eq!(
            chest_worn,
            vec!["worn_grass_pouch", "fake_spirit_hide"],
            "fresh 实例化的 chest.worn 应为 [背包件, 伪皮]；实际 {chest_worn:?}"
        );

        // instantiate 放进去的 fake_spirit_hide，live-equip 校验也必须能把它放回胸槽 worn。
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
        let hide = chest
            .worn
            .iter()
            .find(|i| i.template_id == "fake_spirit_hide")
            .expect("fake_spirit_hide in chest worn");
        let from = InventoryLocationV1::Container {
            container_id: MAIN_PACK_CONTAINER_ID.to_string(),
            row: 0,
            col: 0,
        };
        let to = InventoryLocationV1::Equip {
            slot: EquipSlotV1::Chest,
            state: EquipStateV1::Worn,
        };
        let mut empty = make_empty_inventory();
        empty.max_weight = inv.max_weight;
        assert!(
            validate_move_semantics(&registry, &empty, hide, &from, &to).is_ok(),
            "instantiate 放进胸槽的 fake_spirit_hide，live-equip 必须也放行（instantiate==live）"
        );
    }

    // Bug3（真机回归）— fresh 实例化后，运行时容器 id 必须与 default.toml worn_grass_pouch
    // 自洽：静态占位 `pack_grass_pouch` 已重映射到 pack_<背包件 instance_id>，
    // 不再残留占位 id / 旧 back_pack id。
    #[test]
    fn fresh_instantiate_container_id_self_consistent_with_worn_pack() {
        let registry = load_item_registry().expect("real item registry loads");
        let loadout = load_default_loadout(&registry).expect("default loadout loads");
        let mut alloc = InventoryInstanceIdAllocator::default();
        let inv = instantiate_inventory_from_loadout(&loadout, &mut alloc, &registry)
            .expect("instantiate default loadout");

        // 找到 chest.worn 里的背包件（worn_grass_pouch）instance_id。
        let chest = inv.equipped.get(EQUIP_SLOT_CHEST).expect("chest present");
        let pack = chest
            .worn
            .iter()
            .find(|i| {
                registry
                    .get(&i.template_id)
                    .is_some_and(|t| t.container_spec.is_some())
            })
            .expect("worn pack item present");
        let expected_container_id = container_id_for_worn_pack(pack.instance_id);

        assert!(
            inv.containers.iter().any(|c| c.id == expected_container_id),
            "运行时应存在与穿戴背包件自洽的容器 `{expected_container_id}`；实际 ids = {:?}",
            inv.containers.iter().map(|c| &c.id).collect::<Vec<_>>()
        );
        assert!(
            !inv.containers
                .iter()
                .any(|c| c.id == LOADOUT_PACK_PLACEHOLDER_CONTAINER_ID),
            "静态占位容器 id `{LOADOUT_PACK_PLACEHOLDER_CONTAINER_ID}` 不应在运行时存活（必须已重映射）"
        );
        assert!(
            !inv.containers.iter().any(|c| c.id == "back_pack"),
            "运行时不应出现旧 back_pack 容器 id（命名空间已统一到 pack_<id>）"
        );
        // 背包件容器内物品应来自 default.toml 破草包（非空）。
        let pack_container = inv
            .containers
            .iter()
            .find(|c| c.id == expected_container_id)
            .expect("pack container present");
        assert!(
            !pack_container.items.is_empty(),
            "破草包容器应含 default.toml 起手物品（非空）"
        );
    }

    #[test]
    fn validate_move_semantics_rejects_non_false_skin_misc_item_to_chest_worn() {
        use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};

        let registry = ItemRegistry::from_map(HashMap::from([(
            "rough_cloth".to_string(),
            make_misc_template("rough_cloth"),
        )]));
        let inventory = make_empty_inventory();
        let item = make_test_item_instance(12, "rough_cloth");
        let from = InventoryLocationV1::Container {
            container_id: MAIN_PACK_CONTAINER_ID.to_string(),
            row: 0,
            col: 0,
        };
        let to = InventoryLocationV1::Equip {
            slot: EquipSlotV1::Chest,
            state: EquipStateV1::Worn,
        };

        let error = validate_move_semantics(&registry, &inventory, &item, &from, &to)
            .expect_err("non false-skin / non-armor / non-container misc item should be rejected");

        assert!(
            error.contains("armor / false skin / container"),
            "expected body-slot type rejection, got: {error}"
        );
    }

    #[test]
    fn equip_slot_key_extra_hand_0_returns_correct_string() {
        use crate::schema::inventory::EquipSlotV1;
        assert_eq!(
            equip_slot_key(&EquipSlotV1::ExtraHand0),
            "extra_hand_0",
            "ExtraHand0 should map to runtime key 'extra_hand_0'"
        );
    }

    #[test]
    fn equip_slot_key_extra_hand_1_returns_correct_string() {
        use crate::schema::inventory::EquipSlotV1;
        assert_eq!(
            equip_slot_key(&EquipSlotV1::ExtraHand1),
            "extra_hand_1",
            "ExtraHand1 should map to runtime key 'extra_hand_1'"
        );
    }

    #[test]
    fn validate_equip_slot_accepts_extra_hand_0() {
        let path = std::path::Path::new("test.toml");
        assert!(
            validate_equip_slot(EQUIP_SLOT_EXTRA_HAND_0, path).is_ok(),
            "validate_equip_slot should accept 'extra_hand_0'"
        );
    }

    #[test]
    fn validate_equip_slot_accepts_extra_hand_1() {
        let path = std::path::Path::new("test.toml");
        assert!(
            validate_equip_slot(EQUIP_SLOT_EXTRA_HAND_1, path).is_ok(),
            "validate_equip_slot should accept 'extra_hand_1'"
        );
    }

    #[test]
    fn validate_move_semantics_accepts_weapon_to_extra_hand_0() {
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};
        let registry = ItemRegistry::from_map(HashMap::from([(
            "test_sword".to_string(),
            make_weapon_template("test_sword"),
        )]));
        let inv = make_empty_inventory();
        let item = make_test_item_instance(900, "test_sword");
        let from = InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        };
        let to = InventoryLocationV1::Equip {
            slot: EquipSlotV1::ExtraHand0,
            state: crate::schema::inventory::EquipStateV1::Held,
        };
        assert!(
            validate_move_semantics(&registry, &inv, &item, &from, &to).is_ok(),
            "weapon should be equippable to ExtraHand0"
        );
    }

    #[test]
    fn validate_move_semantics_accepts_weapon_to_extra_hand_1() {
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};
        let registry = ItemRegistry::from_map(HashMap::from([(
            "test_sword".to_string(),
            make_weapon_template("test_sword"),
        )]));
        let inv = make_empty_inventory();
        let item = make_test_item_instance(901, "test_sword");
        let from = InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        };
        let to = InventoryLocationV1::Equip {
            slot: EquipSlotV1::ExtraHand1,
            state: crate::schema::inventory::EquipStateV1::Held,
        };
        assert!(
            validate_move_semantics(&registry, &inv, &item, &from, &to).is_ok(),
            "weapon should be equippable to ExtraHand1"
        );
    }

    #[test]
    fn validate_move_semantics_rejects_misc_item_to_extra_hand() {
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};
        let registry = ItemRegistry::from_map(HashMap::from([(
            "random_herb".to_string(),
            make_misc_template("random_herb"),
        )]));
        let inv = make_empty_inventory();
        let item = make_test_item_instance(902, "random_herb");
        let from = InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        };
        let to = InventoryLocationV1::Equip {
            slot: EquipSlotV1::ExtraHand0,
            state: crate::schema::inventory::EquipStateV1::Held,
        };
        let err = validate_move_semantics(&registry, &inv, &item, &from, &to)
            .expect_err("misc item should not equip to ExtraHand0");
        assert!(
            err.contains("weapon, tool, or hoe"),
            "expected weapon/tool/hoe error, got: {err}"
        );
    }

    #[test]
    fn validate_move_semantics_accepts_tool_to_extra_hand() {
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};
        let mut tool_template = make_misc_template("test_tool");
        tool_template.category = ItemCategory::Tool;
        let registry =
            ItemRegistry::from_map(HashMap::from([("test_tool".to_string(), tool_template)]));
        let inv = make_empty_inventory();
        let item = make_test_item_instance(903, "test_tool");
        let from = InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        };
        let to = InventoryLocationV1::Equip {
            slot: EquipSlotV1::ExtraHand1,
            state: crate::schema::inventory::EquipStateV1::Held,
        };
        assert!(
            validate_move_semantics(&registry, &inv, &item, &from, &to).is_ok(),
            "tool should be equippable to ExtraHand1"
        );
    }

    // (决议 #17) container_id_to_equip_slot 函数已删除（背包无专属槽，容器 id = pack_<id>），
    // 原 container_id_to_equip_slot_maps_all_three_slots pin 测试随之移除。
    // 反查改由 worn_pack_instance_from_container_id 承担，见 layered_equip_p0_pins。

    // ── plan-onboarding-loop-v1 P1.1: 入门残卷 + fragment 物品解析测试 ──

    #[test]
    fn onboarding_scroll_sword_cleave_parses() {
        let registry = load_item_registry().expect("item registry should load");
        let template = registry
            .get("scroll_technique_sword_cleave")
            .expect("scroll_technique_sword_cleave should exist in registry");
        assert_eq!(template.category, ItemCategory::Scroll);
        assert_eq!(template.rarity, ItemRarity::Common);
        let spec = template
            .technique_scroll_spec
            .as_ref()
            .expect("should have technique_scroll_spec");
        assert_eq!(spec.skill_id, "sword.cleave");
    }

    #[test]
    fn onboarding_scroll_sword_thrust_parses() {
        let registry = load_item_registry().expect("item registry should load");
        let template = registry
            .get("scroll_technique_sword_thrust")
            .expect("scroll_technique_sword_thrust should exist in registry");
        assert_eq!(template.category, ItemCategory::Scroll);
        let spec = template.technique_scroll_spec.as_ref().unwrap();
        assert_eq!(spec.skill_id, "sword.thrust");
    }

    #[test]
    fn onboarding_scroll_sword_parry_parses() {
        let registry = load_item_registry().expect("item registry should load");
        let template = registry
            .get("scroll_technique_sword_parry")
            .expect("scroll_technique_sword_parry should exist in registry");
        assert_eq!(template.category, ItemCategory::Scroll);
        assert_eq!(template.rarity, ItemRarity::Uncommon);
        let spec = template.technique_scroll_spec.as_ref().unwrap();
        assert_eq!(spec.skill_id, "sword.parry");
    }

    #[test]
    fn onboarding_scroll_sword_infuse_parses() {
        let registry = load_item_registry().expect("item registry should load");
        let template = registry
            .get("scroll_technique_sword_infuse")
            .expect("scroll_technique_sword_infuse should exist in registry");
        assert_eq!(template.category, ItemCategory::Scroll);
        assert_eq!(template.rarity, ItemRarity::Uncommon);
        let spec = template.technique_scroll_spec.as_ref().unwrap();
        assert_eq!(spec.skill_id, "sword.infuse");
    }

    #[test]
    fn onboarding_scroll_movement_dash_parses() {
        let registry = load_item_registry().expect("item registry should load");
        let template = registry
            .get("scroll_technique_movement_dash")
            .expect("scroll_technique_movement_dash should exist in registry");
        assert_eq!(template.category, ItemCategory::Scroll);
        assert_eq!(template.rarity, ItemRarity::Common);
        let spec = template.technique_scroll_spec.as_ref().unwrap();
        assert_eq!(spec.skill_id, "movement.dash");
    }

    #[test]
    fn existing_scroll_body_guangbo_ticao_in_registry() {
        let registry = load_item_registry().expect("item registry should load");
        let template = registry
            .get("scroll_body_guangbo_ticao")
            .expect("scroll_body_guangbo_ticao should exist in registry (body_scrolls.toml)");
        assert_eq!(template.category, ItemCategory::Scroll);
        let spec = template.technique_scroll_spec.as_ref().unwrap();
        assert_eq!(spec.skill_id, "body.guangbo_ticao");
    }

    #[test]
    fn onboarding_scroll_burst_beng_quan_parses() {
        let registry = load_item_registry().expect("item registry should load");
        let template = registry
            .get("scroll_technique_burst_beng_quan")
            .expect("scroll_technique_burst_beng_quan should exist in registry");
        assert_eq!(template.category, ItemCategory::Scroll);
        assert_eq!(template.rarity, ItemRarity::Rare);
        let spec = template.technique_scroll_spec.as_ref().unwrap();
        assert_eq!(spec.skill_id, "burst_meridian.beng_quan");
    }

    #[test]
    fn onboarding_scroll_zhenmai_parry_parses() {
        let registry = load_item_registry().expect("item registry should load");
        let template = registry
            .get("scroll_technique_zhenmai_parry")
            .expect("scroll_technique_zhenmai_parry should exist in registry");
        assert_eq!(template.category, ItemCategory::Scroll);
        assert_eq!(template.rarity, ItemRarity::Rare);
        let spec = template.technique_scroll_spec.as_ref().unwrap();
        assert_eq!(spec.skill_id, "zhenmai.parry");
    }

    #[test]
    fn fragment_alchemy_hui_yuan_pill_parses() {
        let registry = load_item_registry().expect("item registry should load");
        let template = registry
            .get("fragment_alchemy_hui_yuan_pill")
            .expect("fragment_alchemy_hui_yuan_pill should exist in registry");
        assert_eq!(template.category, ItemCategory::RecipeFragment);
        assert_eq!(template.rarity, ItemRarity::Uncommon);
        let spec = template
            .recipe_fragment_spec
            .as_ref()
            .expect("should have recipe_fragment_spec");
        assert_eq!(spec.recipe_id, "hui_yuan_pill_v0");
        assert_eq!(spec.known_stages, vec![0]);
        assert_eq!(spec.max_quality_tier, 3);
    }

    #[test]
    fn recipe_fragment_spec_toml_roundtrip() {
        let original = RecipeFragmentSpec {
            recipe_id: "hui_yuan_pill_v0".to_string(),
            known_stages: vec![0],
            max_quality_tier: 3,
        };
        let json = serde_json::to_string(&original).expect("should serialize");
        let deserialized: RecipeFragmentSpec =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(
            original, deserialized,
            "RecipeFragmentSpec roundtrip failed"
        );
    }

    #[test]
    fn parse_recipe_fragment_spec_happy_path() {
        let raw = RecipeFragmentSpecToml {
            recipe_id: "hui_yuan_pill_v0".to_string(),
            known_stages: vec![0],
            max_quality_tier: 3,
        };
        let spec = parse_recipe_fragment_spec(raw, Path::new("test.toml"), "test_item").unwrap();
        assert_eq!(spec.recipe_id, "hui_yuan_pill_v0");
        assert_eq!(spec.known_stages, vec![0]);
        assert_eq!(spec.max_quality_tier, 3);
    }

    #[test]
    fn parse_recipe_fragment_spec_empty_recipe_id() {
        let raw = RecipeFragmentSpecToml {
            recipe_id: String::new(),
            known_stages: vec![0],
            max_quality_tier: 1,
        };
        let err = parse_recipe_fragment_spec(raw, Path::new("test.toml"), "bad_item")
            .expect_err("empty recipe_id should fail");
        assert!(
            err.contains("test.toml"),
            "error should mention source path; got: {err}"
        );
    }

    #[test]
    fn parse_recipe_fragment_spec_empty_known_stages() {
        let raw = RecipeFragmentSpecToml {
            recipe_id: "some_recipe".to_string(),
            known_stages: vec![],
            max_quality_tier: 1,
        };
        let err = parse_recipe_fragment_spec(raw, Path::new("test.toml"), "bad_item")
            .expect_err("empty known_stages should fail");
        assert!(
            err.contains("known_stages must not be empty"),
            "error should mention known_stages; got: {err}"
        );
    }

    #[test]
    fn parse_recipe_fragment_spec_max_quality_tier_out_of_range() {
        for bad_tier in [0, 4] {
            let raw = RecipeFragmentSpecToml {
                recipe_id: "some_recipe".to_string(),
                known_stages: vec![0],
                max_quality_tier: bad_tier,
            };
            let err = parse_recipe_fragment_spec(raw, Path::new("test.toml"), "bad_item")
                .expect_err(&format!("tier {bad_tier} should fail"));
            assert!(
                err.contains("max_quality_tier"),
                "error should mention max_quality_tier for tier {bad_tier}; got: {err}"
            );
        }
    }

    #[test]
    fn scroll_sword_cleave_skill_id_matches_definition() {
        // Verify that the skill_id in the scroll matches a valid technique_definition
        use crate::cultivation::known_techniques::technique_definition;
        let def = technique_definition("sword.cleave");
        assert!(
            def.is_some(),
            "sword.cleave should be a registered technique definition"
        );
    }

    // ── plan-food-v1 P0 — 食物物品模板加载测试 ──

    #[test]
    fn food_item_templates_load_from_assets() {
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");

        // happy path: 五个食物 ID 均可查到
        for id in [
            "food.mundane.cooked_meat",
            "food.mundane.chen_bing",
            "food.spirit_fruit.ling_guo",
            "food.spirit_wine.chen_jiu",
            "food.spirit_wine.chen_cu",
        ] {
            assert!(
                registry.get(id).is_some(),
                "food item `{id}` should load from food.toml — 确认 TOML 已添加并 category=food"
            );
        }
    }

    #[test]
    fn food_item_templates_have_food_category() {
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");

        for id in [
            "food.mundane.cooked_meat",
            "food.mundane.chen_bing",
            "food.spirit_fruit.ling_guo",
            "food.spirit_wine.chen_jiu",
            "food.spirit_wine.chen_cu",
        ] {
            let tpl = registry
                .get(id)
                .unwrap_or_else(|| panic!("{id} must be in registry"));
            assert_eq!(
                tpl.category,
                ItemCategory::Food,
                "item `{id}` should have category=Food because plan-food-v1 P0 requires food category; \
                 check parse_item_category food arm and TOML category field"
            );
        }
    }

    #[test]
    fn food_item_default_stack_count_is_16() {
        // ItemCategory::Food stacks up to 16, same as Pill/Misc
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");

        let cooked_meat = registry
            .get("food.mundane.cooked_meat")
            .expect("food.mundane.cooked_meat must exist");
        assert_eq!(
            cooked_meat.max_stack_count, 16,
            "food items default to stack 16 because ItemCategory::Food is in same arm as Pill/Misc"
        );

        let ling_guo = registry
            .get("food.spirit_fruit.ling_guo")
            .expect("food.spirit_fruit.ling_guo must exist");
        assert_eq!(
            ling_guo.max_stack_count, 16,
            "ling_guo stack should be 16 because Food category has same default as Misc"
        );
    }

    #[test]
    fn food_item_spirit_quality_initial_is_in_range() {
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");

        let cases: &[(&str, f64, f64)] = &[
            ("food.mundane.cooked_meat", 0.30, 0.50),
            ("food.mundane.chen_bing", 0.25, 0.50),
            ("food.spirit_fruit.ling_guo", 0.60, 0.80),
            ("food.spirit_wine.chen_jiu", 0.70, 0.90),
            ("food.spirit_wine.chen_cu", 0.55, 0.75),
        ];
        for (id, lo, hi) in cases {
            let tpl = registry
                .get(id)
                .unwrap_or_else(|| panic!("{id} must exist"));
            assert!(
                tpl.spirit_quality_initial >= *lo && tpl.spirit_quality_initial <= *hi,
                "item `{id}` spirit_quality_initial {} out of expected range [{lo},{hi}] — \
                 check food.toml values",
                tpl.spirit_quality_initial
            );
        }
    }

    #[test]
    fn parse_item_category_food_arm_roundtrip() {
        // Verify parse_item_category correctly routes "food" string
        use std::path::PathBuf;
        let path = PathBuf::from("test_path.toml");
        let result = parse_item_category("food", &path, "test_id");
        assert!(
            matches!(result, Ok(ItemCategory::Food)),
            "parse_item_category(\"food\") should return Ok(ItemCategory::Food), got {result:?}"
        );
    }

    #[test]
    fn parse_item_category_food_arm_case_insensitive() {
        use std::path::PathBuf;
        let path = PathBuf::from("test_path.toml");
        assert!(matches!(
            parse_item_category("Food", &path, "x"),
            Ok(ItemCategory::Food)
        ));
        assert!(matches!(
            parse_item_category("FOOD", &path, "x"),
            Ok(ItemCategory::Food)
        ));
        assert!(matches!(
            parse_item_category("  food  ", &path, "x"),
            Ok(ItemCategory::Food)
        ));
    }

    #[test]
    fn parse_item_category_unknown_still_errors() {
        use std::path::PathBuf;
        let path = PathBuf::from("test.toml");
        assert!(
            parse_item_category("totally_unknown_category", &path, "id").is_err(),
            "unknown category should still return Err — food arm must not swallow others"
        );
    }

    // ── plan-food-v1 P1 — 食物物品 shelflife_profile 初始化测试 ──

    #[test]
    fn food_item_templates_have_shelflife_profile_set() {
        // plan-food-v1 P1：food.toml 中每个食物 item 应声明 shelflife_profile + shelflife_track
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");

        let cases: &[(&str, &str, crate::shelflife::DecayTrack)] = &[
            (
                "food.mundane.cooked_meat",
                "food_spoil_mundane_meat_v1",
                crate::shelflife::DecayTrack::Spoil,
            ),
            (
                "food.mundane.chen_bing",
                "food_spoil_mundane_dry_v1",
                crate::shelflife::DecayTrack::Spoil,
            ),
            (
                "food.spirit_fruit.ling_guo",
                "food_spoil_ling_guo_v1",
                crate::shelflife::DecayTrack::Spoil,
            ),
            (
                "food.spirit_wine.chen_jiu",
                "chen_jiu_v1",
                crate::shelflife::DecayTrack::Age,
            ),
            (
                "food.spirit_wine.chen_cu",
                "chen_cu_v1",
                crate::shelflife::DecayTrack::Spoil,
            ),
        ];
        for (id, expected_profile, expected_track) in cases {
            let tpl = registry
                .get(id)
                .unwrap_or_else(|| panic!("{id} must be in registry — check food.toml"));
            assert_eq!(
                tpl.shelflife_profile.as_deref(),
                Some(*expected_profile),
                "item `{id}` should have shelflife_profile=`{expected_profile}` \
                 because plan-food-v1 P1 requires food items to declare their decay profile in food.toml"
            );
            assert_eq!(
                tpl.shelflife_track,
                Some(*expected_track),
                "item `{id}` should have shelflife_track={expected_track:?} \
                 because plan-food-v1 P1 assigns decay track in food.toml"
            );
        }
    }

    #[test]
    fn runtime_instance_from_template_attaches_freshness_for_food_with_shelflife_profile() {
        use crate::shelflife::{DecayProfileId, DecayTrack};
        // plan-food-v1 P1：runtime_instance_from_template が shelflife_profile を持つ
        // テンプレートで Freshness を自動挂する。
        let tpl = ItemTemplate {
            id: "food.spirit_wine.chen_jiu".to_string(),
            display_name: "陈酒".to_string(),
            category: ItemCategory::Food,
            placeable: None,
            max_stack_count: 16,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.5,
            rarity: ItemRarity::Uncommon,
            spirit_quality_initial: 0.80,
            description: "test".to_string(),
            effect: None,
            cast_duration_ms: DEFAULT_CAST_DURATION_MS,
            cooldown_ms: DEFAULT_COOLDOWN_MS,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shield_spec: None,
            shelflife_profile: Some("chen_jiu_v1".to_string()),
            shelflife_track: Some(DecayTrack::Age),
        };

        // plan-food-v1 MAJOR2: current_tick 传入 runtime_instance_from_template，
        // created_at_tick 应等于传入的 current_tick（不再硬编码 0）。
        let spawn_tick = 12345_u64;
        let instance = runtime_instance_from_template(&tpl, 1, 1, spawn_tick);
        let freshness = instance.freshness.as_ref().expect(
            "chen_jiu item should have Freshness attached by runtime_instance_from_template \
                     because template declares shelflife_profile=chen_jiu_v1",
        );
        assert_eq!(
            freshness.track,
            DecayTrack::Age,
            "freshness.track should be Age for chen_jiu (plan-food-v1 P1 Age track)"
        );
        assert_eq!(
            freshness.profile,
            DecayProfileId::new("chen_jiu_v1"),
            "freshness.profile must be chen_jiu_v1 as declared in food.toml"
        );
        assert_eq!(
            freshness.created_at_tick, spawn_tick,
            "freshness.created_at_tick must equal current_tick passed to runtime_instance_from_template; \
             hardcoding 0 causes elapsed=now-0 to pre-age items spawned mid-session"
        );
        assert!(
            (freshness.initial_qi - 0.80_f32).abs() < 1e-4,
            "freshness.initial_qi should equal spirit_quality_initial=0.80 cast to f32; \
             got {}",
            freshness.initial_qi
        );
        assert_eq!(
            freshness.frozen_accumulated, 0,
            "new item frozen_accumulated=0"
        );
        assert!(
            freshness.frozen_since_tick.is_none(),
            "new item frozen_since_tick=None"
        );
    }

    #[test]
    fn runtime_instance_from_template_no_freshness_when_no_shelflife_profile() {
        // Non-food items (or food without shelflife_profile) should have freshness=None
        let tpl = ItemTemplate {
            id: "misc_thing".to_string(),
            display_name: "misc".to_string(),
            category: ItemCategory::Misc,
            placeable: None,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.1,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 1.0,
            description: "no shelflife".to_string(),
            effect: None,
            cast_duration_ms: DEFAULT_CAST_DURATION_MS,
            cooldown_ms: DEFAULT_COOLDOWN_MS,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shield_spec: None,

            shelflife_profile: None,
            shelflife_track: None,
        };

        let instance = runtime_instance_from_template(&tpl, 1, 1, 0);
        assert!(
            instance.freshness.is_none(),
            "item without shelflife_profile should have freshness=None — \
             only items with shelflife_profile in food.toml get auto-freshness"
        );
    }

    #[test]
    fn chen_jiu_item_from_registry_has_age_freshness_on_spawn() {
        // End-to-end: load food.toml item, instantiate, verify freshness is Age track.
        use crate::shelflife::DecayTrack;
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");

        let tpl = registry
            .get("food.spirit_wine.chen_jiu")
            .expect("food.spirit_wine.chen_jiu must be loadable from food.toml");

        let instance = runtime_instance_from_template(tpl, 99, 1, 0);
        let freshness = instance.freshness.as_ref().expect(
            "food.spirit_wine.chen_jiu should have freshness auto-attached because \
             food.toml declares shelflife_profile=chen_jiu_v1",
        );
        assert_eq!(
            freshness.track,
            DecayTrack::Age,
            "chen_jiu template spawns with Age track because chen_jiu_v1 is an Age profile"
        );
    }

    #[test]
    fn ling_guo_item_from_registry_has_spoil_freshness_on_spawn() {
        use crate::shelflife::DecayTrack;
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");

        let tpl = registry
            .get("food.spirit_fruit.ling_guo")
            .expect("food.spirit_fruit.ling_guo must exist");

        let instance = runtime_instance_from_template(tpl, 1, 1, 0);
        let freshness = instance.freshness.as_ref().expect(
            "ling_guo should have freshness because food.toml declares shelflife_profile=food_spoil_ling_guo_v1"
        );
        assert_eq!(
            freshness.track,
            DecayTrack::Spoil,
            "ling_guo spawns with Spoil track — it decays in 2 game days"
        );
    }

    #[test]
    fn shelflife_track_parse_invalid_rejects_with_error() {
        // plan-food-v1 P1：无效的 shelflife_track 字符串应在 TOML 解析时报错。
        use std::path::PathBuf;
        let path = PathBuf::from("test_path.toml");

        let raw = ItemTemplateToml {
            id: "test_item".to_string(),
            name: "Test".to_string(),
            category: "food".to_string(),
            placeable: None,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.1,
            rarity: "common".to_string(),
            spirit_quality_initial: 0.5,
            description: "test".to_string(),
            max_stack_count: None,
            effect: None,
            cast_duration_ms: None,
            cooldown_ms: None,
            weapon: None,
            forge_station: None,
            blueprint_scroll: None,
            inscription_scroll: None,
            technique_scroll: None,
            recipe_fragment: None,
            container: None,
            shield_spec: None,
            shelflife_profile: Some("some_profile".to_string()),
            shelflife_track: Some("INVALID_TRACK".to_string()),
        };

        let result = raw.try_into_item_template(&path);
        assert!(
            result.is_err(),
            "ItemTemplateToml with invalid shelflife_track should fail try_into_item_template"
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("shelflife_track"),
            "error message should mention shelflife_track; got: {err}"
        );
    }

    #[test]
    fn shelflife_track_defaults_to_spoil_when_not_specified() {
        // shelflife_profile は Some だが shelflife_track を省略 → デフォルト spoil。
        use crate::shelflife::DecayTrack;
        use std::path::PathBuf;
        let path = PathBuf::from("test_path.toml");

        let raw = ItemTemplateToml {
            id: "test_item".to_string(),
            name: "Test".to_string(),
            category: "food".to_string(),
            placeable: None,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.1,
            rarity: "common".to_string(),
            spirit_quality_initial: 0.5,
            description: "test".to_string(),
            max_stack_count: None,
            effect: None,
            cast_duration_ms: None,
            cooldown_ms: None,
            weapon: None,
            forge_station: None,
            blueprint_scroll: None,
            inscription_scroll: None,
            technique_scroll: None,
            recipe_fragment: None,
            container: None,
            shelflife_profile: Some("some_profile".to_string()),
            shield_spec: None,
            shelflife_track: None, // should default to "spoil"
        };

        let tpl = raw
            .try_into_item_template(&path)
            .expect("valid TOML should parse OK");
        assert_eq!(
            tpl.shelflife_track,
            Some(DecayTrack::Spoil),
            "when shelflife_track is omitted but shelflife_profile is present, \
             shelflife_track defaults to Spoil"
        );
    }

    // ── plan-food-v1 P1 (CodeRabbit 补测) — shelflife 半配置报错 ──

    /// 负向：shelflife_track=Some 但 shelflife_profile=None → try_into_item_template 必须报错，
    /// 且错误信息含 "shelflife_track"（防止半配置静默绕过 freshness gate）。
    #[test]
    fn shelflife_track_without_profile_is_rejected() {
        use std::path::PathBuf;
        let path = PathBuf::from("test_path.toml");

        let raw = ItemTemplateToml {
            id: "bad_food_half_config".to_string(),
            name: "半配置食物".to_string(),
            category: "food".to_string(),
            placeable: None,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.1,
            rarity: "common".to_string(),
            spirit_quality_initial: 1.0,
            description: "shelflife_track 有值但 profile 为 None".to_string(),
            max_stack_count: None,
            effect: None,
            cast_duration_ms: None,
            cooldown_ms: None,
            weapon: None,
            forge_station: None,
            blueprint_scroll: None,
            inscription_scroll: None,
            technique_scroll: None,
            recipe_fragment: None,
            container: None,
            shield_spec: None,
            shelflife_profile: None,                    // ← 故意缺失
            shelflife_track: Some("spoil".to_string()), // ← 有值但 profile 为 None → 报错
        };

        let result = raw.try_into_item_template(&path);
        assert!(
            result.is_err(),
            "shelflife_track 有值但 shelflife_profile=None 时 try_into_item_template 必须返回 Err，\
             否则 freshness gate 会被静默绕过"
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("shelflife_track"),
            "错误信息应提到 shelflife_track 字段，方便定位半配置问题；实际错误：{err}"
        );
        assert!(
            err.contains("shelflife_profile"),
            "错误信息应同时提到 shelflife_profile 字段，方便定位半配置问题；实际错误：{err}"
        );
    }

    /// 正向对照：shelflife_profile=Some + shelflife_track=Some("spoil") → 正常解析，
    /// track=Spoil，不报错。
    #[test]
    fn shelflife_track_and_profile_both_some_is_accepted() {
        use crate::shelflife::DecayTrack;
        use std::path::PathBuf;
        let path = PathBuf::from("test_path.toml");

        let raw = ItemTemplateToml {
            id: "good_food_full_config".to_string(),
            name: "完整配置食物".to_string(),
            category: "food".to_string(),
            placeable: None,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.1,
            rarity: "common".to_string(),
            spirit_quality_initial: 1.0,
            description: "shelflife_track + profile 均有值".to_string(),
            max_stack_count: None,
            effect: None,
            cast_duration_ms: None,
            cooldown_ms: None,
            weapon: None,
            forge_station: None,
            blueprint_scroll: None,
            inscription_scroll: None,
            technique_scroll: None,
            recipe_fragment: None,
            container: None,
            shield_spec: None,
            shelflife_profile: Some("my_spoil_profile_v1".to_string()), // ← 正确配对
            shelflife_track: Some("spoil".to_string()),
        };

        let tpl = raw
            .try_into_item_template(&path)
            .expect("shelflife_profile + shelflife_track 均 Some 时应正常解析");
        assert_eq!(
            tpl.shelflife_profile.as_deref(),
            Some("my_spoil_profile_v1"),
            "shelflife_profile 应原样保留在解析结果中"
        );
        assert_eq!(
            tpl.shelflife_track,
            Some(DecayTrack::Spoil),
            "shelflife_track='spoil' 应解析为 DecayTrack::Spoil"
        );
    }

    // ── plan-shield-block-v1 P0 — ItemCategory::Shield 饱和化测试 ──────────────

    /// Shield 变体 serde 正反对拍（happy path）：序列化后再反序列化须还原原值。
    #[test]
    fn item_category_shield_serde_roundtrip() {
        let cat = ItemCategory::Shield;
        let json = serde_json::to_string(&cat).expect(
            "期望 Shield 变体可序列化为 JSON，\
             实际 serde_json::to_string 失败",
        );
        let parsed: ItemCategory = serde_json::from_str(&json).expect(
            "期望 JSON 字符串可反序列化回 ItemCategory::Shield，\
             实际 serde_json::from_str 失败",
        );
        assert_eq!(
            parsed,
            ItemCategory::Shield,
            "期望 serde roundtrip 结果为 Shield，\
             实际得到 {parsed:?}"
        );
    }

    /// parse_item_category("shield") 应返回 ItemCategory::Shield。
    #[test]
    fn parse_item_category_shield_happy() {
        use std::path::PathBuf;
        let path = PathBuf::from("test.toml");
        let result = parse_item_category("shield", &path, "wooden_shield");
        assert!(
            matches!(result, Ok(ItemCategory::Shield)),
            "期望 parse_item_category(\"shield\") = Ok(Shield)，因为 plan-shield-block-v1 P0 加了 shield 分支，\
             实际得到 {result:?}"
        );
    }

    /// parse_item_category("Shield")（首字母大写）因 to_ascii_lowercase 后应也命中 shield 分支。
    #[test]
    fn parse_item_category_shield_case_insensitive() {
        use std::path::PathBuf;
        let path = PathBuf::from("test.toml");
        let result = parse_item_category("Shield", &path, "wooden_shield");
        assert!(
            matches!(result, Ok(ItemCategory::Shield)),
            "期望 parse_item_category(\"Shield\") 因 trim+to_ascii_lowercase 后命中 shield 分支，\
             实际得到 {result:?}"
        );
    }

    /// parse_item_category("") 应返回 Err（未知 category 分支）。
    #[test]
    fn parse_item_category_empty_string_errors() {
        use std::path::PathBuf;
        let path = PathBuf::from("test.toml");
        let result = parse_item_category("", &path, "x");
        assert!(
            result.is_err(),
            "期望 parse_item_category(\"\") 返回 Err（空字符串不是合法 category），\
             实际得到 {result:?}"
        );
    }

    /// ItemCategory::Shield 的 max_stack_count 应为 1（与武器/防具同级，不可叠加）。
    #[test]
    fn shield_category_default_stack_count_is_one() {
        assert_eq!(
            default_max_stack_count_for_category(ItemCategory::Shield),
            1,
            "期望 Shield max_stack_count = 1，因为盾牌与武器/防具同级不可叠加，\
             实际得到 {}",
            default_max_stack_count_for_category(ItemCategory::Shield)
        );
    }

    /// workbench_materials.toml 中 wooden_shield / bone_shield 应以 ItemCategory::Shield 加载。
    #[test]
    fn shield_templates_load_with_shield_category() {
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");

        for id in ["wooden_shield", "bone_shield"] {
            let tpl = registry.get(id).unwrap_or_else(|| {
                panic!(
                    "期望 item `{id}` 在 registry 中存在，\
                     实际未找到——检查 workbench_materials.toml 是否包含该 id"
                )
            });
            assert_eq!(
                tpl.category,
                ItemCategory::Shield,
                "期望 item `{id}` category = Shield（plan-shield-block-v1 P0 改 category），\
                 实际得到 {:?}",
                tpl.category
            );
        }
    }

    /// 盾牌装入 off_hand 应成功（happy path）。
    #[test]
    fn apply_move_shield_to_off_hand_succeeds() {
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.template_id = "wooden_shield".to_string();
        inv.containers[0].items[0].instance.display_name = "木盾".to_string();
        inv.containers[0].items[0].instance.grid_h = 2;

        let result = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Equip {
                slot: EquipSlotV1::OffHand,
                state: crate::schema::inventory::EquipStateV1::Held,
            },
        );
        assert!(
            result.is_ok(),
            "期望 wooden_shield 装入 off_hand 成功（plan-shield-block-v1 P0 消灭孤岛根因），\
             实际得到错误：{result:?}"
        );
        // MINOR #3 — 锁住「槽位真被盾占用」：断言 equipped 里 OFF_HAND 槽存在且 template_id 正确。
        assert_eq!(
            inv.equipped
                .get(EQUIP_SLOT_OFF_HAND)
                .and_then(|s| s.held.as_ref())
                .map(|item| item.template_id.as_str()),
            Some("wooden_shield"),
            "期望 OFF_HAND 槽被 wooden_shield 占用（plan-shield-block-v1 P0 post-state 断言），\
             实际 equipped[off_hand] = {:?}",
            inv.equipped
                .get(EQUIP_SLOT_OFF_HAND)
                .and_then(|s| s.held.as_ref())
                .map(|i| &i.template_id)
        );
    }

    /// 主手持双手兵器（锁对侧）时拒绝装 off_hand 盾（边界，two_hand 槽已删，改对侧锁）。
    #[test]
    fn apply_move_shield_to_off_hand_rejected_when_main_hand_two_handed() {
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.template_id = "wooden_shield".to_string();
        inv.containers[0].items[0].instance.display_name = "木盾".to_string();
        inv.containers[0].items[0].instance.grid_h = 2;
        // main_hand 持双手杖（staff 派生 two-handed），锁 off_hand。
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(ItemInstance {
                instance_id: 99,
                template_id: "wooden_staff".to_string(),
                display_name: "木杖".to_string(),
                grid_w: 1,
                grid_h: 3,
                weight: 2.0,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 1.0,
                durability: 1.0,
                freshness: None,
                mineral_id: None,
                charges: None,
                forge_quality: None,
                forge_color: None,
                forge_side_effects: Vec::new(),
                forge_achieved_tier: None,
                alchemy: None,
                lingering_owner_qi: None,
            }),
        );

        let error = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Equip {
                slot: EquipSlotV1::OffHand,
                state: crate::schema::inventory::EquipStateV1::Held,
            },
        )
        .expect_err(
            "期望主手双手兵器锁住 off_hand 时装盾被拒绝，\
             实际返回 Ok——对侧锁校验漏掉",
        );

        assert!(
            error.contains("双手兵器占用双手，对侧已锁定"),
            "期望错误消息含 '双手兵器占用双手，对侧已锁定'，\
             实际消息：{error}"
        );
    }

    /// 非盾非 treasure 非 dagger 物品装 off_hand 仍按原逻辑拒绝（回归保护）。
    #[test]
    fn apply_move_non_shield_non_treasure_non_dagger_off_hand_still_rejected() {
        use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        // iron_sword：Weapon 但非 Dagger/Fist，应被拒
        inv.containers[0].items[0].instance.template_id = "iron_sword".to_string();
        inv.containers[0].items[0].instance.display_name = "凡铁剑".to_string();
        inv.containers[0].items[0].instance.grid_h = 2;

        let error = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Equip {
                slot: EquipSlotV1::OffHand,
                state: crate::schema::inventory::EquipStateV1::Held,
            },
        )
        .expect_err(
            "期望 iron_sword 装 off_hand 被拒绝（非盾非 treasure 非 dagger），\
             实际返回 Ok——Shield 分支意外放行了其他类别",
        );

        assert!(
            error.contains("only dagger/fist are allowed"),
            "期望错误消息含 'only dagger/fist are allowed'，\
             实际消息：{error}"
        );
    }

    /// equip_slot_for_item_id("armor_iron_chestplate") 正向返回 Some(Chest)（路由回归正向断言）。
    #[test]
    fn equip_slot_for_item_id_armor_still_routes_correctly() {
        use crate::armor::mundane::equip_slot_for_item_id;
        use crate::schema::inventory::EquipSlotV1;
        let slot = equip_slot_for_item_id("armor_iron_chestplate");
        // MINOR #4 — MundaneArmorSlot::Chestplate → EquipSlotV1::Chest；
        // 正向断言锁住「iron chestplate 确实路由到 Chest 槽」，防 Shield 分支意外影响 Armor routing。
        assert_eq!(
            slot,
            Some(EquipSlotV1::Chest),
            "期望 equip_slot_for_item_id(\"armor_iron_chestplate\") == Some(Chest)，\
             plan-shield-block-v1 P0 不改此函数；实际得到 {slot:?}"
        );
    }

    /// equip_slot_for_item_id("wooden_shield") 仍返回 None（盾不经此函数）。
    #[test]
    fn equip_slot_for_item_id_wooden_shield_returns_none() {
        use crate::armor::mundane::equip_slot_for_item_id;
        let slot = equip_slot_for_item_id("wooden_shield");
        assert!(
            slot.is_none(),
            "期望 equip_slot_for_item_id(\"wooden_shield\") = None，\
             因为盾走 EquipSlotV1::OffHand 3799 arm 不走此函数，\
             实际得到 {slot:?}"
        );
    }

    // ── plan-shield-block-v1 P2 — ShieldSpec 饱和化测试 ──────────────────────

    /// ShieldSpec.validate() happy path：wooden_shield 规格通过验证。
    #[test]
    fn shield_spec_validate_happy_path_wooden_shield() {
        let spec = ShieldSpec {
            block_ratio: 0.5,
            durability_max: 40.0,
            stamina_drain_per_s: 3.0,
        };
        assert!(
            spec.validate("wooden_shield").is_ok(),
            "wooden_shield 规格（0.5/40/3.0）应通过 validate()；\
             实际 Err: {:?}",
            spec.validate("wooden_shield")
        );
    }

    /// ShieldSpec.validate() happy path：bone_shield 规格通过验证。
    #[test]
    fn shield_spec_validate_happy_path_bone_shield() {
        let spec = ShieldSpec {
            block_ratio: 0.65,
            durability_max: 80.0,
            stamina_drain_per_s: 3.0,
        };
        assert!(
            spec.validate("bone_shield").is_ok(),
            "bone_shield 规格（0.65/80/3.0）应通过 validate()；\
             实际 Err: {:?}",
            spec.validate("bone_shield")
        );
    }

    /// block_ratio = 0.7（上限）仍通过验证（边界：上界包含）。
    #[test]
    fn shield_spec_validate_block_ratio_max_boundary_passes() {
        let spec = ShieldSpec {
            block_ratio: 0.7,
            durability_max: 40.0,
            stamina_drain_per_s: 3.0,
        };
        assert!(
            spec.validate("test_shield").is_ok(),
            "block_ratio=0.7（worldview §五 凡人盾上限）应通过 validate()（>= 包含边界），\
             实际 Err: {:?}",
            spec.validate("test_shield")
        );
    }

    /// block_ratio > 0.7 被拒绝（超出凡人盾上限）。
    #[test]
    fn shield_spec_validate_block_ratio_above_max_rejected() {
        let spec = ShieldSpec {
            block_ratio: 0.71,
            durability_max: 40.0,
            stamina_drain_per_s: 3.0,
        };
        let err = spec
            .validate("cheat_shield")
            .expect_err("block_ratio=0.71 超出凡人盾 0.7 上限，应被拒绝");
        assert!(
            err.contains("block_ratio"),
            "错误消息应含 'block_ratio'，实际：{err}"
        );
    }

    /// block_ratio = 0.0 被拒绝（无效：不可为零）。
    #[test]
    fn shield_spec_validate_block_ratio_zero_rejected() {
        let spec = ShieldSpec {
            block_ratio: 0.0,
            durability_max: 40.0,
            stamina_drain_per_s: 3.0,
        };
        assert!(
            spec.validate("zero_shield").is_err(),
            "block_ratio=0.0 应被拒绝（无效：不能为 0）"
        );
    }

    /// block_ratio 负值被拒绝。
    #[test]
    fn shield_spec_validate_block_ratio_negative_rejected() {
        let spec = ShieldSpec {
            block_ratio: -0.1,
            durability_max: 40.0,
            stamina_drain_per_s: 3.0,
        };
        assert!(
            spec.validate("neg_shield").is_err(),
            "block_ratio 负值应被拒绝"
        );
    }

    /// block_ratio = NaN 被拒绝。
    #[test]
    fn shield_spec_validate_block_ratio_nan_rejected() {
        let spec = ShieldSpec {
            block_ratio: f64::NAN,
            durability_max: 40.0,
            stamina_drain_per_s: 3.0,
        };
        assert!(
            spec.validate("nan_shield").is_err(),
            "block_ratio=NaN 应被拒绝（is_finite 检查）"
        );
    }

    /// durability_max = 0.0 被拒绝。
    #[test]
    fn shield_spec_validate_durability_zero_rejected() {
        let spec = ShieldSpec {
            block_ratio: 0.5,
            durability_max: 0.0,
            stamina_drain_per_s: 3.0,
        };
        let err = spec
            .validate("zero_dur_shield")
            .expect_err("durability_max=0 应被拒绝");
        assert!(
            err.contains("durability_max"),
            "错误消息应含 'durability_max'，实际：{err}"
        );
    }

    /// stamina_drain_per_s = 0.0 被拒绝。
    #[test]
    fn shield_spec_validate_stamina_drain_zero_rejected() {
        let spec = ShieldSpec {
            block_ratio: 0.5,
            durability_max: 40.0,
            stamina_drain_per_s: 0.0,
        };
        let err = spec
            .validate("zero_drain_shield")
            .expect_err("stamina_drain_per_s=0 应被拒绝");
        assert!(
            err.contains("stamina_drain_per_s"),
            "错误消息应含 'stamina_drain_per_s'，实际：{err}"
        );
    }

    // plan-shield-block-v1 P2 §Issue5.3 — durability_max NaN/inf 独立用例
    /// durability_max = NaN 被拒绝（is_finite 检查）。
    #[test]
    fn shield_spec_validate_durability_max_nan_rejected() {
        let spec = ShieldSpec {
            block_ratio: 0.5,
            durability_max: f64::NAN,
            stamina_drain_per_s: 3.0,
        };
        let err = spec
            .validate("nan_dur_shield")
            .expect_err("durability_max=NaN 应被拒绝（is_finite 检查）");
        assert!(
            err.contains("durability_max"),
            "错误消息应含 'durability_max'，实际：{err}"
        );
    }

    /// durability_max = +Inf 被拒绝（is_finite 检查）。
    #[test]
    fn shield_spec_validate_durability_max_inf_rejected() {
        let spec = ShieldSpec {
            block_ratio: 0.5,
            durability_max: f64::INFINITY,
            stamina_drain_per_s: 3.0,
        };
        let err = spec
            .validate("inf_dur_shield")
            .expect_err("durability_max=+Inf 应被拒绝（is_finite 检查）");
        assert!(
            err.contains("durability_max"),
            "错误消息应含 'durability_max'，实际：{err}"
        );
    }

    // plan-shield-block-v1 P2 §Issue5.3 — stamina_drain_per_s NaN/inf 独立用例
    /// stamina_drain_per_s = NaN 被拒绝（is_finite 检查）。
    #[test]
    fn shield_spec_validate_stamina_drain_nan_rejected() {
        let spec = ShieldSpec {
            block_ratio: 0.5,
            durability_max: 40.0,
            stamina_drain_per_s: f32::NAN,
        };
        let err = spec
            .validate("nan_drain_shield")
            .expect_err("stamina_drain_per_s=NaN 应被拒绝（is_finite 检查）");
        assert!(
            err.contains("stamina_drain_per_s"),
            "错误消息应含 'stamina_drain_per_s'，实际：{err}"
        );
    }

    /// stamina_drain_per_s = +Inf 被拒绝（is_finite 检查）。
    #[test]
    fn shield_spec_validate_stamina_drain_inf_rejected() {
        let spec = ShieldSpec {
            block_ratio: 0.5,
            durability_max: 40.0,
            stamina_drain_per_s: f32::INFINITY,
        };
        let err = spec
            .validate("inf_drain_shield")
            .expect_err("stamina_drain_per_s=+Inf 应被拒绝（is_finite 检查）");
        assert!(
            err.contains("stamina_drain_per_s"),
            "错误消息应含 'stamina_drain_per_s'，实际：{err}"
        );
    }

    /// 从 TOML 加载的 wooden_shield 含正确 ShieldSpec（block_ratio=0.5, durability=40, drain=3.0）。
    #[test]
    fn wooden_shield_loads_correct_shield_spec_from_toml() {
        let registry = load_item_registry().expect("item registry 应从 assets/items/*.toml 加载");
        let tpl = registry
            .get("wooden_shield")
            .expect("wooden_shield 应存在于 registry");
        let spec = tpl
            .shield_spec
            .as_ref()
            .expect("wooden_shield 应有 shield_spec（P2 TOML 块必须存在）");
        assert!(
            (spec.block_ratio - 0.5).abs() < 1e-9,
            "wooden_shield.block_ratio 应为 0.5，实际 {}",
            spec.block_ratio
        );
        assert!(
            (spec.durability_max - 40.0).abs() < 1e-9,
            "wooden_shield.durability_max 应为 40.0，实际 {}",
            spec.durability_max
        );
        assert!(
            (spec.stamina_drain_per_s - 3.0).abs() < 1e-4,
            "wooden_shield.stamina_drain_per_s 应为 3.0，实际 {}",
            spec.stamina_drain_per_s
        );
    }

    /// 从 TOML 加载的 bone_shield 含正确 ShieldSpec（block_ratio=0.65, durability=80, drain=3.0）。
    #[test]
    fn bone_shield_loads_correct_shield_spec_from_toml() {
        let registry = load_item_registry().expect("item registry 应从 assets/items/*.toml 加载");
        let tpl = registry
            .get("bone_shield")
            .expect("bone_shield 应存在于 registry");
        let spec = tpl
            .shield_spec
            .as_ref()
            .expect("bone_shield 应有 shield_spec（P2 TOML 块必须存在）");
        assert!(
            (spec.block_ratio - 0.65).abs() < 1e-9,
            "bone_shield.block_ratio 应为 0.65，实际 {}",
            spec.block_ratio
        );
        assert!(
            (spec.durability_max - 80.0).abs() < 1e-9,
            "bone_shield.durability_max 应为 80.0，实际 {}",
            spec.durability_max
        );
        assert!(
            (spec.stamina_drain_per_s - 3.0).abs() < 1e-4,
            "bone_shield.stamina_drain_per_s 应为 3.0，实际 {}",
            spec.stamina_drain_per_s
        );
    }

    #[test]
    fn placeable_container_templates_load_from_workbench_materials_toml() {
        let registry = load_item_registry().expect("item registry 应从 assets/items/*.toml 加载");
        for (id, placeable) in [
            ("trade_crate", "storage_crate"),
            ("herb_crate_placed", "storage_crate"),
            ("dead_drop_box", "dead_drop"),
        ] {
            let tpl = registry
                .get(id)
                .unwrap_or_else(|| panic!("{id} 应存在于 registry"));
            assert_eq!(tpl.category, ItemCategory::Misc, "{id} 应保持 misc 类别");
            assert_eq!(
                tpl.placeable.as_deref(),
                Some(placeable),
                "{id} 应声明正确 placeable 标记"
            );
        }
        let carried_herb = registry
            .get("herb_crate")
            .expect("随身版 herb_crate 应继续存在");
        assert_eq!(
            carried_herb.placeable, None,
            "随身版 herb_crate 不应被放置链路消费"
        );
    }

    #[test]
    fn item_template_toml_normalizes_non_block_placeable_marker() {
        let mut raw = raw_item_template_toml("portable_trade_crate", "misc");
        raw.placeable = Some("  STORAGE_CRATE  ".to_string());

        let tpl = raw
            .try_into_item_template(std::path::Path::new("test_placeable.toml"))
            .expect("非 Block 模板应允许声明 placeable");

        assert_eq!(
            tpl.category,
            ItemCategory::Misc,
            "非 Block placeable 模板应保持自身物品分类"
        );
        assert_eq!(
            tpl.placeable.as_deref(),
            Some("storage_crate"),
            "placeable 标记应 trim 并归一化为小写"
        );
    }

    #[test]
    fn item_template_toml_rejects_blank_placeable_marker() {
        let mut raw = raw_item_template_toml("blank_placeable_crate", "misc");
        raw.placeable = Some("   ".to_string());

        let error = raw
            .try_into_item_template(std::path::Path::new("test_placeable.toml"))
            .expect_err("空白 placeable 标记必须报错");

        assert!(
            error.contains("placeable"),
            "错误信息应指出 placeable 字段为空，实际 {error}"
        );
    }

    /// 非盾物品（如 iron_sword）的 shield_spec 为 None。
    #[test]
    fn non_shield_item_has_no_shield_spec() {
        let registry = load_item_registry().expect("item registry 应加载");
        let tpl = registry
            .get("iron_sword")
            .expect("iron_sword 应存在于 registry");
        assert!(
            tpl.shield_spec.is_none(),
            "iron_sword 不是盾，shield_spec 应为 None，实际有值"
        );
    }

    /// category=Shield 但缺 shield_spec 块 → try_into_item_template 报错。
    #[test]
    fn shield_category_without_shield_spec_block_is_rejected() {
        use std::path::PathBuf;
        let path = PathBuf::from("test_shield.toml");
        let raw = ItemTemplateToml {
            id: "bad_shield_no_spec".to_string(),
            placeable: None,
            name: "无规格盾".to_string(),
            category: "shield".to_string(),
            grid_w: 1,
            grid_h: 2,
            base_weight: 2.0,
            rarity: "common".to_string(),
            spirit_quality_initial: 0.0,
            description: "category=shield but missing shield_spec".to_string(),
            max_stack_count: None,
            effect: None,
            cast_duration_ms: None,
            cooldown_ms: None,
            weapon: None,
            forge_station: None,
            blueprint_scroll: None,
            inscription_scroll: None,
            technique_scroll: None,
            recipe_fragment: None,
            container: None,
            shield_spec: None, // ← 故意缺失
            shelflife_profile: None,
            shelflife_track: None,
        };
        let result = raw.try_into_item_template(&path);
        assert!(
            result.is_err(),
            "category=shield 但缺 shield_spec 块时应报错，防止孤岛装备"
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("shield_spec") || err.contains("Shield"),
            "错误消息应提到 shield_spec 缺失，实际：{err}"
        );
    }

    /// 非盾 category 带 shield_spec 块 → try_into_item_template 报错。
    #[test]
    fn non_shield_category_with_shield_spec_block_is_rejected() {
        use std::path::PathBuf;
        let path = PathBuf::from("test_sword_with_shield_spec.toml");
        let raw = ItemTemplateToml {
            id: "bad_sword_with_shield_spec".to_string(),
            placeable: None,
            name: "剑+盾规格冲突".to_string(),
            category: "weapon".to_string(),
            grid_w: 1,
            grid_h: 2,
            base_weight: 1.0,
            rarity: "common".to_string(),
            spirit_quality_initial: 0.0,
            description: "weapon category with shield_spec should fail".to_string(),
            max_stack_count: None,
            effect: None,
            cast_duration_ms: None,
            cooldown_ms: None,
            weapon: None,
            forge_station: None,
            blueprint_scroll: None,
            inscription_scroll: None,
            technique_scroll: None,
            recipe_fragment: None,
            container: None,
            shield_spec: Some(ShieldSpecToml {
                block_ratio: 0.5,
                durability_max: 40.0,
                stamina_drain_per_s: 3.0,
            }),
            shelflife_profile: None,
            shelflife_track: None,
        };
        let result = raw.try_into_item_template(&path);
        assert!(result.is_err(), "非盾 category 带 shield_spec 块时应报错");
    }

    // ─── plan-worldgen-v4 P5 §8.1#5 — vanilla 模板注入专属矩阵 ───

    /// happy path：注入把全部非 air vanilla BlockKind 注册为 `vanilla:<id>` 模板，
    /// 数量 = BlockKind::ALL 中非 air 的个数，且空 map 注入后 air 不在结果里。
    #[test]
    fn inject_vanilla_block_templates_covers_all_non_air_kinds() {
        use valence::prelude::BlockKind;

        let expected = BlockKind::ALL
            .iter()
            .filter(|k| k.to_str() != "air")
            .count();

        let mut templates = HashMap::new();
        let injected = inject_vanilla_block_templates(&mut templates)
            .expect("空 registry 注入 vanilla 模板应成功");

        assert_eq!(
            injected, expected,
            "注入数量应等于非 air vanilla BlockKind 数（{expected}），实为 {injected}"
        );
        assert_eq!(
            templates.len(),
            expected,
            "templates map 大小应等于注入数（无重复），实为 {}",
            templates.len()
        );
        // air 跳过：既无 `vanilla:air`，也没把 air 当成可给予物品。
        assert!(
            !templates.contains_key("vanilla:air"),
            "air 必须被跳过，不得注册 vanilla:air 模板"
        );
        // 抽样确认常见块在内。
        assert!(
            templates.contains_key("vanilla:stone"),
            "vanilla:stone 应被注入"
        );
        assert!(
            templates.contains_key("vanilla:stone_bricks"),
            "vanilla:stone_bricks 应被注入"
        );
    }

    /// 字段契约：注入的 `vanilla:<id>` 模板形态固定（id/category/max_stack/placeable），
    /// 与 block_place vanilla: 直通分支与 ItemCategory::Block 默认堆叠上限对齐。
    #[test]
    fn vanilla_block_template_field_contract() {
        let template = vanilla_block_template("stone_bricks");
        assert_eq!(
            template.id, "vanilla:stone_bricks",
            "id 必须是 vanilla:<bare>，实为 {}",
            template.id
        );
        assert_eq!(
            template.category,
            ItemCategory::Block,
            "vanilla 方块模板 category 必须为 Block"
        );
        assert_eq!(
            template.max_stack_count,
            default_max_stack_count_for_category(ItemCategory::Block),
            "max_stack_count 必须取 Block 默认堆叠上限（64）"
        );
        assert!(
            template.placeable.is_none(),
            "placeable 必须为 None——放置走 block_place 的 vanilla: 直通分支，不经 PlaceableBlockKind"
        );
    }

    /// 错误分支：注入若与已存在的同名 key（手写 TOML 或重复注入）撞车，必须返回 Err，
    /// 保护手写映射不被静默覆盖。
    #[test]
    fn inject_vanilla_block_templates_errors_on_key_collision() {
        let mut templates = HashMap::new();
        // 预置一个会与 vanilla:stone 撞 key 的手写模板。
        templates.insert("vanilla:stone".to_string(), vanilla_block_template("stone"));

        let err = inject_vanilla_block_templates(&mut templates)
            .expect_err("撞 key 必须返回 Err，不得静默覆盖手写映射");
        assert!(
            err.contains("vanilla:stone") && err.contains("collides"),
            "撞 key 错误信息应指明冲突的 vanilla:stone，实为: {err}"
        );
    }

    /// 集成：生产 load_item_registry() 末尾确实注入了 vanilla 模板，
    /// 锁住「give-block 链路依赖的 vanilla:<id> 在真 registry 里可查」。
    #[test]
    fn load_item_registry_includes_injected_vanilla_templates() {
        let registry = load_item_registry().expect("真 registry 加载应含 vanilla 模板");
        assert!(
            registry.get("vanilla:stone_bricks").is_some(),
            "真 ItemRegistry 必须含 vanilla:stone_bricks（give-block 链路依赖）"
        );
        assert!(
            registry.get("vanilla:air").is_none(),
            "真 ItemRegistry 不得含 vanilla:air"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // plan-layered-equip-v1 P0 — 决议锁定行为 pin 测试（SlotContents / worn_cap /
    // classify_equip_state / weapon_two_handed / pack 容器 id 反查）。
    // 这些把 PR-1 重构的核心契约钉死，任何回归立刻撞红。
    // ─────────────────────────────────────────────────────────────────────────
    mod layered_equip_p0_pins {
        use super::*;

        // ── 1. SlotContents serde roundtrip（空 / 单 worn / 多 worn / worn+held / held-only）──

        fn roundtrip(slot: &SlotContents) -> SlotContents {
            let json = serde_json::to_string(slot).expect("SlotContents should serialize");
            serde_json::from_str(&json).expect("SlotContents should deserialize")
        }

        #[test]
        fn slot_contents_serde_roundtrip_empty() {
            let empty = SlotContents::default();
            let json = serde_json::to_string(&empty).expect("empty SlotContents should serialize");
            // 空槽：worn 序列化为 []，held=None 时省略字段（skip_serializing_if）。
            assert!(
                json.contains("\"worn\":[]"),
                "空槽应把 worn 序列化为 []，实际：{json}"
            );
            assert!(
                !json.contains("held"),
                "held=None 应被省略（skip_serializing_if），实际：{json}"
            );
            assert_eq!(roundtrip(&empty), empty, "空槽 roundtrip 应保持相等");
        }

        #[test]
        fn slot_contents_serde_roundtrip_single_worn() {
            let s = SlotContents::worn_single(make_test_item_instance(1, "armor_a"));
            assert_eq!(roundtrip(&s), s, "单 worn 件 roundtrip 应保持相等");
            assert_eq!(s.worn.len(), 1);
            assert!(s.held.is_none());
        }

        #[test]
        fn slot_contents_serde_roundtrip_multi_worn() {
            let s = SlotContents {
                worn: vec![
                    make_test_item_instance(1, "layer_bottom"),
                    make_test_item_instance(2, "layer_mid"),
                    make_test_item_instance(3, "layer_top"),
                ],
                held: None,
            };
            let back = roundtrip(&s);
            assert_eq!(back, s, "三层 worn roundtrip 应保持相等（含栈顺序）");
            // 栈顺序：worn.last() = 栈顶。
            assert_eq!(
                back.worn_top().unwrap().instance_id,
                3,
                "worn_top 应为最后压入的件（栈顶 = Vec 末尾）"
            );
        }

        #[test]
        fn slot_contents_serde_roundtrip_worn_plus_held() {
            let s = SlotContents {
                worn: vec![make_test_item_instance(1, "armor_a")],
                held: Some(make_test_item_instance(2, "sword_a")),
            };
            assert_eq!(roundtrip(&s), s, "worn+held roundtrip 应保持相等");
        }

        #[test]
        fn slot_contents_serde_roundtrip_held_only() {
            let s = SlotContents::held_single(make_test_item_instance(9, "sword_a"));
            let json = serde_json::to_string(&s).expect("held-only should serialize");
            assert!(
                json.contains("\"held\""),
                "held=Some 应序列化 held 字段：{json}"
            );
            assert_eq!(roundtrip(&s), s, "held-only roundtrip 应保持相等");
        }

        // ── 2. worn_cap 边界（决议 #6/#14/#17）──

        #[test]
        fn worn_cap_boundaries_per_slot() {
            assert_eq!(worn_cap(EQUIP_SLOT_HEAD), 2, "head worn cap=2");
            assert_eq!(worn_cap(EQUIP_SLOT_FEET), 2, "feet worn cap=2");
            assert_eq!(worn_cap(EQUIP_SLOT_CHEST), 3, "chest worn cap=3");
            assert_eq!(worn_cap(EQUIP_SLOT_LEGS), 3, "legs worn cap=3");
            assert_eq!(
                worn_cap(EQUIP_SLOT_MAIN_HAND),
                0,
                "main_hand held-only cap=0"
            );
            assert_eq!(worn_cap(EQUIP_SLOT_OFF_HAND), 0, "off_hand held-only cap=0");
            assert_eq!(
                worn_cap(EQUIP_SLOT_EXTRA_HAND_0),
                0,
                "extra_hand_0 held-only cap=0"
            );
            assert_eq!(
                worn_cap(EQUIP_SLOT_EXTRA_HAND_1),
                0,
                "extra_hand_1 held-only cap=0"
            );
        }

        // ── P5 pin — worn_cap_bonus 默认 0（扩展点未接升级源，行为不变）──

        #[test]
        fn p5_worn_cap_bonus_defaults_to_zero_so_effective_cap_equals_base() {
            // P5 hook：升级源未接时 bonus=0，有效 cap = base。
            // 当升级源接入时本测试需同步更新（预期值不再恒等 base）。
            for slot in &[
                EQUIP_SLOT_HEAD,
                EQUIP_SLOT_CHEST,
                EQUIP_SLOT_LEGS,
                EQUIP_SLOT_FEET,
                EQUIP_SLOT_MAIN_HAND,
                EQUIP_SLOT_OFF_HAND,
                EQUIP_SLOT_EXTRA_HAND_0,
                EQUIP_SLOT_EXTRA_HAND_1,
            ] {
                let base = worn_cap(slot);
                let bonus = worn_cap_bonus(slot);
                assert_eq!(
                    bonus, 0,
                    "worn_cap_bonus({slot}) 应为 0（P5 占位，升级源未接）；\
                     接入升级源后请删除此 assert_eq!(bonus,0) 并改为具体边界断言"
                );
                assert_eq!(
                    base.saturating_add(bonus),
                    base,
                    "effective worn_cap({slot}) = base={base}+bonus=0，应等于 base（行为不变）"
                );
            }
        }

        #[test]
        fn p5_treasure_trigger_cap_fn_equals_constant() {
            // P5 hook：treasure_trigger_cap() 当前恒等于 TREASURE_TRIGGER_CAP 常量。
            // 接入升级源后，该函数可返回比常量更大的值；届时删除此相等断言并改边界断言。
            assert_eq!(
                treasure_trigger_cap(),
                TREASURE_TRIGGER_CAP,
                "treasure_trigger_cap() 应等于常量 TREASURE_TRIGGER_CAP={TREASURE_TRIGGER_CAP}（P5 占位，升级源未接）"
            );
        }

        // ── P5 边界 pin — worn_cap_bonus 空串 / 完全未知槽位（CR 补充）──

        #[test]
        fn p5_worn_cap_bonus_empty_slot_returns_zero() {
            // P5 占位：空字符串不是任何规范槽位，bonus 恒 0。
            // 断言信息：P5 占位——升级源未接前任意槽位 bonus 恒 0，空串亦不例外。
            assert_eq!(
                worn_cap_bonus(""),
                0,
                "P5 占位：worn_cap_bonus(\"\") 应为 0（升级源未接，任意非规范输入 bonus 恒 0）"
            );
        }

        #[test]
        fn p5_worn_cap_bonus_unknown_slot_returns_zero() {
            // P5 占位：完全陌生的槽位名不是任何规范槽位，bonus 恒 0。
            // 断言信息：P5 占位——升级源未接前任意槽位 bonus 恒 0，未知槽位亦不例外。
            assert_eq!(
                worn_cap_bonus("totally_unknown_slot"),
                0,
                "P5 占位：worn_cap_bonus(\"totally_unknown_slot\") 应为 0（升级源未接，任意非规范输入 bonus 恒 0）"
            );
        }

        // ── worn_cap 非规范输入行为 pin（CR 补充）──

        #[test]
        fn worn_cap_noncanonical_inputs_default_to_zero() {
            // worn_cap 对空串和未知槽位走 `_ => 0` 默认分支，行为是 held-only 语义（cap=0）。
            // 锁定此占位行为：任何非规范输入恒 0，防回归改变 wildcard 分支语义。
            assert_eq!(
                worn_cap(""),
                0,
                "worn_cap(\"\") 应为 0：非规范输入走 _ => 0 默认分支（held-only 语义）"
            );
            assert_eq!(
                worn_cap("unknown"),
                0,
                "worn_cap(\"unknown\") 应为 0：非规范输入走 _ => 0 默认分支（held-only 语义）"
            );
        }

        // ── 3. classify_equip_state（决议 #16）：Weapon|Tool→Held，Armor|Container→Worn ──

        fn make_tool_template(id: &str) -> ItemTemplate {
            let mut t = make_misc_template(id);
            t.category = ItemCategory::Tool;
            t
        }

        fn make_armor_template(id: &str) -> ItemTemplate {
            let mut t = make_misc_template(id);
            t.category = ItemCategory::Armor;
            t
        }

        #[test]
        fn classify_equip_state_buckets() {
            let registry = ItemRegistry::from_map(HashMap::from([
                ("weapon_a".to_string(), make_weapon_template("weapon_a")),
                ("tool_a".to_string(), make_tool_template("tool_a")),
                ("armor_a".to_string(), make_armor_template("armor_a")),
                (
                    "container_a".to_string(),
                    make_container_template("container_a", EQUIP_SLOT_CHEST, 2, 2, 5.0),
                ),
            ]));

            assert_eq!(
                classify_equip_state(&make_test_item_instance(1, "weapon_a"), &registry),
                EquipState::Held,
                "Weapon 应分类为 Held"
            );
            assert_eq!(
                classify_equip_state(&make_test_item_instance(2, "tool_a"), &registry),
                EquipState::Held,
                "Tool 应分类为 Held"
            );
            assert_eq!(
                classify_equip_state(&make_test_item_instance(3, "armor_a"), &registry),
                EquipState::Worn,
                "Armor 应分类为 Worn"
            );
            assert_eq!(
                classify_equip_state(&make_test_item_instance(4, "container_a"), &registry),
                EquipState::Worn,
                "Container（背包）应分类为 Worn"
            );
        }

        // ── 4. weapon_two_handed（决议 #7）：Spear/Staff→true，其余→false ──

        #[test]
        fn weapon_two_handed_per_kind() {
            use crate::combat::weapon::WeaponKind;
            assert!(weapon_two_handed(WeaponKind::Spear), "Spear 应为双手");
            assert!(weapon_two_handed(WeaponKind::Staff), "Staff 应为双手");
            assert!(!weapon_two_handed(WeaponKind::Sword), "Sword 应为单手");
            assert!(!weapon_two_handed(WeaponKind::Dagger), "Dagger 应为单手");
            assert!(!weapon_two_handed(WeaponKind::Fist), "Fist 应为单手");
        }

        // ── 6. container_id_for_worn_pack / worn_pack_instance_from_container_id 反查 roundtrip ──

        #[test]
        fn worn_pack_container_id_roundtrip() {
            let id = container_id_for_worn_pack(42);
            assert_eq!(id, "pack_42", "容器 id 应为 pack_<instance_id>");
            assert_eq!(
                worn_pack_instance_from_container_id(&id),
                Some(42),
                "pack_42 应反解回 instance_id=42"
            );
            assert_eq!(
                worn_pack_instance_from_container_id("body_pocket"),
                None,
                "body_pocket 非 pack_ 前缀，应返回 None"
            );
            assert_eq!(
                worn_pack_instance_from_container_id("main_pack"),
                None,
                "main_pack 非 pack_ 前缀，应返回 None"
            );
            assert_eq!(
                worn_pack_instance_from_container_id("pack_notanumber"),
                None,
                "pack_ 后非数字应返回 None"
            );
        }
    }
}
