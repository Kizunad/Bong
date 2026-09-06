use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Added, App, Client, Commands, Component, Despawned, Entity, EntityInteraction,
    EntityLayerId, Hand, InteractEntityEvent, IntoSystemConfigs, Or, Position, Query, Res, ResMut,
    Resource, Startup, Update, Username, With, Without,
};

use crate::body_plan::race_registry::HUMAN_RACE_ID;
use crate::body_plan::types::{RaceGateOwned, RaceId};
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

/// TSY 死亡掉落窗口内的临时上下文。
///
/// Collapse completed 会立刻移除 `TsyPresence`，避免玩家继续被 gate 视作 TSY 内实体；
/// 但复活掉落仍需要入场 snapshot 来执行 TSY 分流规则。
#[derive(Debug, Clone, Component)]
pub struct PendingTsyDeathDrop {
    pub presence: crate::world::tsy::TsyPresence,
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

type JoinedClientsWithoutInventoryFilter = (
    Or<(
        Added<Client>,
        Added<crate::cultivation::known_techniques::KnownTechniquesReconnectReady>,
        Added<crate::player::state::PlayerState>,
    )>,
    With<crate::player::state::PlayerState>,
    Without<PlayerInventory>,
    Without<crate::cultivation::known_techniques::KnownTechniquesReconnectBlocked>,
);

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
    /// plan-scroll-reading-v1 P0 — 可阅读残卷规格；读取不消耗物品（区别于 `technique_scroll_spec`
    /// 消耗式学招）。任意 scroll/book 类物品皆可挂此字段，供 C2S `ScrollReadRequest` 查询。
    pub readable_scroll_spec: Option<ReadableScrollSpec>,
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
    /// plan-race-system-v1 P3b（决议 §8.1 #5 装备域矩阵）— 可穿戴该物品的种族门。
    /// `#[serde(default)]` → 老配置不带该字段解析为 `RaceGateOwned::Any`（绝大多数物品
    /// 任何种族可穿）。装备门判定用 **Form 身份**（当前形态，非本体）——`Humanoid` 档判
    /// `form_is_humanoid`，`Species` 档判 `form_race_id`，与习得/施放门（判本体）刻意不同域。
    pub wearer_race: RaceGateOwned,
}

impl ItemTemplate {
    /// 测试装配用最小模板（`Misc` 类、1×1、`wearer_race = Any`），供跨模块单测
    /// （如 `network::race_gate_meta_emit`）构造 `ItemRegistry` 而无需手抄全字段。
    /// 仿 `ItemRegistry::from_map` 的 `#[doc(hidden)] pub`——非生产 API，生产走
    /// `load_item_registry` 从 toml 加载。
    #[doc(hidden)]
    pub fn minimal_for_test(id: &str) -> Self {
        Self {
            id: id.to_string(),
            display_name: id.to_string(),
            category: ItemCategory::Misc,
            placeable: None,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 1,
            base_weight: 1.0,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 0.0,
            description: String::new(),
            effect: None,
            cast_duration_ms: DEFAULT_CAST_DURATION_MS,
            cooldown_ms: DEFAULT_COOLDOWN_MS,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            readable_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shield_spec: None,
            shelflife_profile: None,
            shelflife_track: None,
            wearer_race: RaceGateOwned::default(),
        }
    }
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
    /// [快捷] 标签：此容器内物品允许被指派/拖入快捷 hotbar（F1-F9）。
    /// `body_pocket` 隐式 true（非 ContainerSpec 件，snapshot 特判）；其余容器默认 false。
    /// 未来「快捷腰包」等模板在 `[item.container]` 写 `quick_access = true` 即生效，无需改代码。
    #[serde(default)]
    pub quick_access: bool,
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

/// plan-scroll-reading-v1 P0 — 可阅读残卷的模板级静态规格。
///
/// 读取不消耗物品（区别于 `TechniqueScrollSpec` 消耗式学招）。正文按页内联存于
/// TOML（不运行时读 `docs/library/`，与其余 `*ScrollSpec` 惯例统一）。
#[derive(Debug, Clone, PartialEq)]
pub struct ReadableScrollSpec {
    /// 阅读屏标题，如 "《经脉浅述·残卷》"。
    pub title: String,
    /// 正文分页，每元素一页；`ScrollOpen` payload 原样透传。至少 1 页。
    pub body_pages: Vec<String>,
    /// 开卷时 client 播放的循环姿态动画 id（如 `bong:read_scroll`）；None = 无动画。
    pub anim_id: Option<String>,
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
    /// [快捷] 标签缓存：此容器内物品可被指派至快捷 hotbar。
    ///
    /// `body_pocket` 由 snapshot 特判恒 true（此处保持 false）；`pack_<id>` 由
    /// `rebuild_containers_from_equipment` 从 owner 背包件 `ContainerSpec.quick_access` 回填。
    /// `serde(default)` 容旧存档（旧档读为 false），rebuild 每次幂等重算，不依赖 DB 持久。
    /// 缓存于此避免 snapshot 期再次反查 ItemRegistry（snapshot 路径不持 registry）。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub quick_access: bool,
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

        self.next += 1;
        Ok(id)
    }

    pub fn advance_past(&mut self, used_id: u64) -> Result<(), String> {
        if used_id > JS_SAFE_INTEGER_MAX {
            return Err(format!(
                "persisted inventory instance id {used_id} exceeds JS safe integer max {JS_SAFE_INTEGER_MAX}"
            ));
        }
        self.next = self.next.max(used_id + 1);
        Ok(())
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

pub fn clear_player_inventory(
    inventory: &mut PlayerInventory,
    scope: ClearScope,
    registry: &ItemRegistry,
) {
    match scope {
        ClearScope::PackOnly => {
            for container in &mut inventory.containers {
                if container.id == MAIN_PACK_CONTAINER_ID
                    || worn_pack_instance_from_container_id(&container.id).is_some()
                {
                    container.items.clear();
                }
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

    // Clearing equipment or carried packs can change the authoritative topology and capacity.
    // The containers were emptied above, so rebuild must never need to spill an item.
    let overflow = rebuild_containers_from_equipment(inventory, registry);
    if !overflow.is_empty() {
        tracing::error!(
            ?overflow,
            "clear_player_inventory: emptied inventory unexpectedly produced rebuild overflow"
        );
    }
    debug_assert!(
        overflow.is_empty(),
        "clear_player_inventory: empty containers produced rebuild overflow: {overflow:?}"
    );
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
    // plan-remains-suite P0 — 遗骸 G 键统一交互 intent（与右键 InteractEntityEvent 并行）。
    app.add_event::<RemainsLootIntent>();
    app.add_systems(
        Startup,
        hydrate_durable_inventory_state.after(crate::persistence::PersistenceBootstrapSet),
    );
    app.add_systems(
        Update,
        (
            apply_death_drop_on_revive,
            apply_termination_drop_on_terminate,
            handle_remains_interactions,
            handle_remains_loot_intents,
            freshness::freshness_tick_system,
            sync_overloaded_marker,
            spirit_treasure::sync_spirit_treasures,
            // plan-tsy-loot-v1 §2.2 — 玩家踏入 family 时 spawn 1% 上古遗物（idempotent）。
            tsy_loot_spawn::tsy_loot_spawn_on_enter,
        ),
    );
}

fn hydrate_durable_inventory_state(
    settings: Option<Res<crate::persistence::PersistenceSettings>>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut dropped_loot: ResMut<DroppedLootRegistry>,
) {
    let Some(settings) = settings else {
        return;
    };
    let entries =
        crate::persistence::load_durable_dropped_loot(&settings).unwrap_or_else(|error| {
            panic!(
                "[bong][inventory] cannot safely hydrate durable dropped loot from {}: {error}",
                settings.db_path().display()
            )
        });
    let high_water = crate::persistence::persisted_inventory_instance_id_high_water(&settings)
        .unwrap_or_else(|error| {
            panic!(
                "[bong][inventory] cannot safely seed instance allocator from {}: {error}",
                settings.db_path().display()
            )
        });
    if let Some(high_water) = high_water {
        allocator.advance_past(high_water).unwrap_or_else(|error| {
            panic!("[bong][inventory] invalid persisted instance allocator high-water: {error}")
        });
    }
    dropped_loot.entries = entries;
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
    terrain_providers: Option<valence::prelude::Res<crate::world::terrain::TerrainProviders>>,
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

            // 死亡点若在半空（如高台战斗/浮空秘境），遗骸不应悬空——贴地才像"尸体"。
            let surface_provider = terrain_providers
                .as_deref()
                .and_then(|providers| providers.for_dimension(entity_dimension));
            let snapped = crate::npc::spawn::common::snap_spawn_y_to_surface(
                valence::prelude::DVec3::new(base[0], base[1], base[2]),
                surface_provider,
            );
            let remains_pos = [snapped.x, snapped.y, snapped.z];
            let (remains_entity, entry_entity) = spawn_player_remains_entity(
                &mut commands,
                layer_id.0,
                remains_pos,
                entity_dimension,
            );
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

/// 遗骸容器人称——worldview §十二「终结后」用的是「遗骸容器」而不是"遗蜕"，
/// 遗蜕是最初的占位英文名 "Remains" 的直译误用，这里统一成正典词。
pub const REMAINS_DISPLAY_NAME: &str = "遗骸";

fn spawn_player_remains_entity(
    commands: &mut Commands,
    layer: Entity,
    pos: [f64; 3],
    dimension: DimensionKind,
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
            // `Pose::Dying` 只驱动 vanilla LivingEntityRenderer 的死亡旋转动画（deathTime
            // 插值），不会让实体整体躺平——玩家看到的其实是"站着扭曲"，不像尸体。
            // `Pose::Sleeping` 才是唯一让 player 实体客户端整体躺平渲染的 pose。
            entity_pose: PoseComponent(valence::entity::Pose::Sleeping),
            entity_custom_name: CustomName(Some(Text::text(REMAINS_DISPLAY_NAME))),
            entity_name_visible: NameVisible(true),
            ..Default::default()
        })
        .insert(CurrentDimension(dimension))
        .id();

    // In order for the player entity to be visible to other players, there must
    // be an entry in the player list.
    let entry_entity = commands
        .spawn(PlayerListEntryBundle {
            uuid,
            username: Username(username),
            display_name: DisplayName(Some(Text::text(REMAINS_DISPLAY_NAME))),
            listed: Listed(false),
            ..Default::default()
        })
        .id();

    (remains_entity, entry_entity)
}

/// 遗骸拾取范围（右键交互与 G 键 C2S 两条路径共用同一个常量）。
pub const REMAINS_PICKUP_RANGE_SQ: f64 = 2.5 * 2.5;

/// 遗骸拾取核心：把 bone_coins + items 转移进拾取者背包，装不下的留在遗骸里；
/// 全部转移完毕后 despawn 遗骸实体（**valence 层实体必须 `insert(Despawned)`，
/// 不许裸 `despawn()`**——否则 `send_entity_update_messages` 会因为找不到已裸删的
/// 实体而 panic 崩服，见 `feedback_valence_despawn_layer_entity` 血泪教训）。
///
/// 右键交互（[`handle_remains_interactions`]）与 G 键统一交互（[`RemainsLootIntent`]）
/// 两条路径共用本函数；范围 / 同 layer / 同 dimension 校验由各自调用方负责——两条路径的
/// 校验数据来源不同（一条来自 vanilla `InteractEntityEvent`，一条来自 client 上报的
/// `remains_id` 反查），收敛在这里反而会模糊两边各自的拒绝语义。
///
/// 返回值：本次是否至少转移了一件物品 / 一点骨币（`moved_any`）。
pub fn transfer_remains_to_looter(
    commands: &mut Commands,
    remains_entity: Entity,
    remains: &mut RemainsContainer,
    inventory: &mut PlayerInventory,
) -> bool {
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

            let Some(location) = find_first_fit_container_location(inventory, &item) else {
                leftover.push(RemainsItemRecord {
                    source_container_id,
                    source_row,
                    source_col,
                    item,
                });
                continue;
            };
            if let Err(reason) = attach_at_location(inventory, item.clone(), &location) {
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
        bump_revision(inventory);
    }

    if remains.items.is_empty() && remains.bone_coins == 0 {
        commands.entity(remains_entity).insert(Despawned);
        commands.entity(remains.player_list_entry).insert(Despawned);
    }

    moved_any
}

pub fn handle_remains_interactions(
    mut interactions: bevy_ecs::event::EventReader<InteractEntityEvent>,
    mut commands: Commands,
    mut remains_q: Query<(Entity, &mut RemainsContainer, &Position, &EntityLayerId)>,
    mut inventories: Query<(&mut PlayerInventory, &Position, &EntityLayerId)>,
) {
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
        if dx * dx + dy * dy + dz * dz > REMAINS_PICKUP_RANGE_SQ {
            continue;
        }

        transfer_remains_to_looter(&mut commands, remains_entity, &mut remains, &mut inventory);
    }
}

/// plan-remains-suite P0：遗骸 G 键统一交互 intent（`ClientRequestV1::RemainsLoot` 落地后
/// 由 `network::client_request_handler` 发出，本模块的 [`handle_remains_loot_intents`] 消费）。
/// 之所以走 event 中转而不是直接塞进 `handle_client_request_payloads`：那个巨型 match 函数
/// 已经持有形状不同的 `Query<&mut PlayerInventory>` 等，本 intent 需要的
/// `(Entity, &UniqueId, &mut RemainsContainer, ...)` 组合查询与之在同一 system 内会产生
/// query 别名冲突；拆成独立 system 与 `handle_remains_interactions`（右键路径）对称。
#[derive(Debug, Clone, bevy_ecs::event::Event)]
pub struct RemainsLootIntent {
    pub entity: Entity,
    pub remains_id: String,
}

fn notify_remains_loot(
    pending_narrations: Option<&mut crate::player::gameplay::PendingGameplayNarrations>,
    usernames: &Query<&Username>,
    entity: Entity,
    text: &str,
    style: crate::schema::common::NarrationStyle,
) {
    let (Some(pending_narrations), Ok(username)) = (pending_narrations, usernames.get(entity))
    else {
        return;
    };
    pending_narrations.push_player(username.0.as_str(), text, style);
}

/// G 键统一交互路径的遗骸拾取：候选/派发在 client 侧（[`RemainsLootIntentHandler`] 的 Java
/// 对应实现）已经用 [`RemainsStore`]（client 缓存的 remains_sync 快照）挑出目标，这里只做
/// server 端权威校验（同 layer + 同 dimension + 2.5m 范围），不信任 client 的候选判断。
type RemainsLootQueryItem<'a> = (
    Entity,
    &'a valence::prelude::UniqueId,
    &'a mut RemainsContainer,
    &'a Position,
    &'a EntityLayerId,
    Option<&'a CurrentDimension>,
);
type RemainsLooterQueryItem<'a> = (
    &'a mut PlayerInventory,
    &'a Position,
    &'a EntityLayerId,
    Option<&'a CurrentDimension>,
);

#[allow(clippy::type_complexity)]
pub fn handle_remains_loot_intents(
    mut intents: bevy_ecs::event::EventReader<RemainsLootIntent>,
    mut commands: Commands,
    mut remains_q: Query<RemainsLootQueryItem<'_>>,
    mut inventories: Query<RemainsLooterQueryItem<'_>>,
    usernames: Query<&Username>,
    mut pending_narrations: Option<
        valence::prelude::ResMut<crate::player::gameplay::PendingGameplayNarrations>,
    >,
) {
    use crate::schema::common::NarrationStyle;

    for intent in intents.read() {
        let Ok((mut inventory, player_pos, player_layer, player_dimension)) =
            inventories.get_mut(intent.entity)
        else {
            continue;
        };

        let target = remains_q
            .iter_mut()
            .find(|(_, uuid, ..)| uuid.0.to_string() == intent.remains_id);
        let Some((
            remains_entity,
            _uuid,
            mut remains,
            remains_pos,
            remains_layer,
            remains_dimension,
        )) = target
        else {
            // 遗骸已被他人搬空 despawn，或 client 缓存过期——无操作即可，属于良性竞态。
            tracing::debug!(
                "[bong][inventory] remains_loot rejected: unknown remains_id `{}`",
                intent.remains_id
            );
            continue;
        };

        if remains_layer.0 != player_layer.0
            || remains_dimension.map(|d| d.0) != player_dimension.map(|d| d.0)
        {
            notify_remains_loot(
                pending_narrations.as_deref_mut(),
                &usernames,
                intent.entity,
                "那具遗骸不在此界，够不着。",
                NarrationStyle::SystemWarning,
            );
            continue;
        }

        let rp = remains_pos.get();
        let pp = player_pos.get();
        let dx = rp.x - pp.x;
        let dy = rp.y - pp.y;
        let dz = rp.z - pp.z;
        if dx * dx + dy * dy + dz * dz > REMAINS_PICKUP_RANGE_SQ {
            notify_remains_loot(
                pending_narrations.as_deref_mut(),
                &usernames,
                intent.entity,
                "离遗骸太远，够不着。",
                NarrationStyle::SystemWarning,
            );
            continue;
        }

        let moved_any =
            transfer_remains_to_looter(&mut commands, remains_entity, &mut remains, &mut inventory);
        if moved_any {
            notify_remains_loot(
                pending_narrations.as_deref_mut(),
                &usernames,
                intent.entity,
                "你搜过了那具遗骸。",
                NarrationStyle::Narration,
            );
        } else {
            notify_remains_loot(
                pending_narrations.as_deref_mut(),
                &usernames,
                intent.entity,
                "包裹已经装不下了。",
                NarrationStyle::SystemWarning,
            );
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
            // 起始 loadout 实例化：rebuild_containers_from_equipment（下方）会按 owner 模板回填 pack 的值。
            quick_access: false,
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
        readable_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: None,
        shield_spec: None,
        shelflife_profile: None,
        shelflife_track: None,
        wearer_race: RaceGateOwned::default(),
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
    let path = crate::body_plan::resolve_assets_root().join(DEFAULT_ITEMS_DIR);
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
    let path = crate::body_plan::resolve_assets_root().join(DEFAULT_LOADOUT_PATH);
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

    /// 迭代全部模板（无序）— dev 命令 Tab 补全等需要枚举全量 id 的场景用。
    pub fn iter_templates(&self) -> impl Iterator<Item = &ItemTemplate> {
        self.templates.values()
    }
}

/// 在 technique metadata registry 可用后，校验所有功法卷轴的跨表引用。
///
/// ItemRegistry 先于 cultivation 注册，因此 item TOML 的 leaf parser 只校验字段语法；
/// 完整引用闭包在启动期由 cultivation::register 一次性执行。错误按 item id 排序汇总，
/// 保证同一坏数据集产生稳定、可操作的诊断。
pub(crate) fn validate_technique_scroll_references(
    items: &ItemRegistry,
    techniques: &crate::cultivation::known_techniques::TechniqueRegistry,
) -> Result<(), String> {
    let mut missing = items
        .iter_templates()
        .filter_map(|template| {
            let scroll = template.technique_scroll_spec.as_ref()?;
            techniques.get(&scroll.skill_id).is_none().then(|| {
                format!(
                    "item `{}` references unknown technique_scroll.skill_id `{}`",
                    template.id, scroll.skill_id
                )
            })
        })
        .collect::<Vec<_>>();
    missing.sort();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "invalid technique scroll references:\n- {}",
            missing.join("\n- ")
        ))
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

/// plan-botany-harvest-full-inventory-loss-v1 §8.1 决议 #1 — 原子"入包或掉地" grant。
/// `DroppedToGround` 装箱：`DroppedLootEntry`（含内嵌 `ItemInstance`）比
/// `InventoryGrantReceipt` 大得多，不装箱会让整个 enum 按最大变体膨胀
/// （clippy::large_enum_variant）。
#[derive(Debug, Clone, PartialEq)]
pub enum GrantOrGroundOutcome {
    Granted(InventoryGrantReceipt),
    DroppedToGround(Box<DroppedLootEntry>),
}

/// plan-botany-harvest-full-inventory-loss-v1 §8.1 决议 #1 — 先原子尝试走既有
/// `add_item_to_player_inventory_inner`；仅当失败原因是背包已满（`"inventory full:"`
/// 前缀）时才 fallback 到 `DroppedLootRegistry`（地面掉落，复用
/// `fauna::drop::fauna_drop_system` 已验证的直插模式）。其它结构性错误（unknown
/// template / stack_count 0 / no containers）原样透传——这些是配置错误，不该被
/// 静默转成"地面掉落"掩盖。拒绝了 pre-check 方案：容器判定逻辑已内嵌在
/// `add_item_to_player_inventory_inner` 内部，重新实现一份等价判定必然与真正的插入
/// 逻辑产生"两处判定各自维护、迟早漂移"的技术债。
#[allow(clippy::too_many_arguments)]
pub fn add_item_to_player_inventory_or_ground(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
    dropped_loot: Option<&mut DroppedLootRegistry>,
    template_id: &str,
    stack_count: u32,
    current_tick: u64,
    ground_pos: [f64; 3],
    ground_dimension: DimensionKind,
    customize_instance: Option<&dyn Fn(&mut ItemInstance)>,
) -> Result<GrantOrGroundOutcome, String> {
    match add_item_to_player_inventory_inner(
        inventory,
        registry,
        allocator,
        template_id,
        stack_count,
        true,
        customize_instance,
        current_tick,
    ) {
        Ok(receipt) => Ok(GrantOrGroundOutcome::Granted(receipt)),
        Err(err) if err.starts_with("inventory full:") => {
            let Some(dropped_loot) = dropped_loot else {
                return Err(format!(
                    "inventory full and no DroppedLootRegistry available to fall back: {err}"
                ));
            };
            let template = registry
                .get(template_id)
                .ok_or_else(|| format!("unknown item template id `{template_id}`"))?;
            let instance_id = allocator.next_id()?;
            let mut item =
                runtime_instance_from_template(template, instance_id, stack_count, current_tick);
            if let Some(customize_instance) = customize_instance {
                customize_instance(&mut item);
            }
            let entry = DroppedLootEntry {
                instance_id,
                source_container_id: format!("overflow:{template_id}"),
                source_row: 0,
                source_col: 0,
                world_pos: ground_pos,
                dimension: ground_dimension,
                item,
            };
            if dropped_loot.entries.contains_key(&instance_id) {
                return Err(format!(
                    "dropped loot instance id collision: {instance_id} already exists"
                ));
            }
            dropped_loot.entries.insert(instance_id, entry.clone());
            Ok(GrantOrGroundOutcome::DroppedToGround(Box::new(entry)))
        }
        Err(other) => Err(other),
    }
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
    for ((row, col, new_stack_count), instance_id) in new_stacks.into_iter().zip(new_instance_ids) {
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
    /// plan-scroll-reading-v1 P0：任意 scroll/book 类物品可挂，读取不消耗。
    #[serde(default)]
    readable_scroll: Option<ReadableScrollSpecToml>,
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
    /// plan-race-system-v1 P3b — 可穿戴该物品的种族门；缺省 `Any`（老配置不带此字段
    /// 照常解析）。TOML 形状复用 `RaceGateOwned` 自身 `#[serde(tag = "kind", ...)]`：
    /// `[item.wearer_race] kind = "humanoid"` 或
    /// `[item.wearer_race]\nkind = "species"\nspecies = ["whale"]`。
    #[serde(default)]
    wearer_race: RaceGateOwned,
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

/// plan-scroll-reading-v1 P0 — TOML 层的可阅读残卷规格块（对应 `[item.readable_scroll]`）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadableScrollSpecToml {
    title: String,
    body_pages: Vec<String>,
    #[serde(default)]
    anim_id: Option<String>,
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
    /// [快捷] 标签——见 `ContainerSpec.quick_access`。缺省 false。
    #[serde(default)]
    quick_access: bool,
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
        quick_access: raw.quick_access,
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
        let readable_scroll_spec = self
            .readable_scroll
            .map(|raw| parse_readable_scroll_spec(raw, source_path, id.as_str()))
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
            readable_scroll_spec,
            recipe_fragment_spec,
            wearer_race: self.wearer_race,
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
    Ok(TechniqueScrollSpec { kind, skill_id })
}

/// plan-scroll-reading-v1 P0 — 解析 TOML `[item.readable_scroll]` 为 `ReadableScrollSpec`。
///
/// 校验：`title` 非空；`body_pages` 至少 1 页且每页非空；`anim_id`（若填）非空白字符串。
pub fn parse_readable_scroll_spec(
    raw: ReadableScrollSpecToml,
    source_path: &Path,
    item_id: &str,
) -> Result<ReadableScrollSpec, String> {
    let title = required_non_empty(
        raw.title,
        source_path,
        &format!("item `{item_id}` readable_scroll.title"),
    )?;
    if raw.body_pages.is_empty() {
        return Err(format!(
            "{} item `{item_id}` has readable_scroll.body_pages with 0 pages; expected >= 1",
            source_path.display()
        ));
    }
    let mut body_pages = Vec::with_capacity(raw.body_pages.len());
    for (idx, page) in raw.body_pages.into_iter().enumerate() {
        body_pages.push(required_non_empty(
            page,
            source_path,
            &format!("item `{item_id}` readable_scroll.body_pages[{idx}]"),
        )?);
    }
    let anim_id = match raw.anim_id {
        Some(raw_anim_id) => Some(required_non_empty(
            raw_anim_id,
            source_path,
            &format!("item `{item_id}` readable_scroll.anim_id"),
        )?),
        None => None,
    };
    Ok(ReadableScrollSpec {
        title,
        body_pages,
        anim_id,
    })
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
                // DB 加载占位：load 收尾 rebuild_containers_from_equipment 会按 owner 模板回填 pack 值。
                quick_access: false,
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

/// plan-inventory-hint-panel-v1 P0 — 结构化拒绝原因（仿 `CastRejectReason`
/// @ `cultivation/skill_registry.rs:20-34`）。
///
/// 收敛 `apply_inventory_move` / `validate_move_semantics` 及其 `?` 传播链
/// （`displaced_at_target` / `validate_attach_fits` / `attach_at_location` /
/// `validate_equip_to`）里散落的裸 `String` 拒绝原因，让每条拒绝都能：
/// 1. 经 [`InventoryMoveRejectReason::to_wire_tag`] 下发结构化 `InventoryMoveRejectedV1` payload；
/// 2. 经 [`InventoryMoveRejectReason::to_log_string`] / `Display` 继续供 `tracing::warn!` 打日志
///    （字符串内容与 P0 前的原始文案尽量保持一致，便于比对既有日志）。
///
/// `RealmTooLow` 额外并入了 `client_request_handler.rs` 里独立硬编码的伪皮胸槽境界门控——
/// 该分支此前完全绕过 `Result`，只 `tracing::warn!` + `resync_snapshot`。
#[derive(Debug, Clone, PartialEq)]
pub enum InventoryMoveRejectReason {
    /// `from` 位置不含该 instance（客户端过期乐观态 / 幽灵拖拽）。
    FromLocationMismatch,
    /// instance_id 在 inventory 里彻底找不到。
    InstanceNotFound,
    /// container_id 未知（幽灵容器 / 客户端过期状态）。
    UnknownContainerId,
    /// 落位越界（行列超出容器边界，或 row/col 转换失败）。
    TargetOutOfBounds,
    /// 目标格被另一实例占用（单一 anchor 精确匹配失败，或非 swap 候选）。
    TargetOccupied { instance_id: u64 },
    /// 目标区域与多个物品重叠，v1 不支持多重叠 swap。
    MultiOverlapNotSupported,
    /// hotbar 下标越界。
    HotbarIndexOutOfRange,
    /// hotbar 目标槽已被占用。
    HotbarOccupied,
    /// swap 候选（occupant）的 footprint 与拖拽物不一致。
    SwapFootprintMismatch,
    /// item template 未在 `ItemRegistry` 注册。
    UnknownItemTemplate,
    /// 从 worn 栈非栈顶层移出（LIFO 保护，决议 #12）。
    WornStackNotTop,
    /// 该品类不允许进 hotbar（武器/工具/护甲/盾/法宝/容器 六类，决议见 `validate_move_semantics`）。
    ForbiddenInHotbar { category: ItemCategory },
    /// 背包件已不在携带面（worn/held/hotbar/body_pocket 之外——已丢弃/销毁），
    /// 无法再放入内含物。`owner_instance_id` 是该背包件的 instance id。
    PackDetached { owner_instance_id: u64 },
    /// 手槽/身体槽装备态不匹配（手槽只能 held，身体槽只能 worn）。
    HeldWornMismatch,
    /// 目标槽物品品类不符（手槽：非武器/工具/锄头，off_hand 另收法宝/盾；
    /// 身体槽：非护甲/伪皮/容器）。
    EquipCategoryMismatch,
    /// off_hand 武器种类不符（仅 dagger/fist）。
    OffHandTypeMismatch,
    /// 该手已持械，须先卸下才能换装（决议 #3 拒绝不顶替）。
    HandOccupied,
    /// 双手兵器占用双手——本手被对侧双手兵器锁，或双手兵器入本手时对侧已被占用。
    TwoHandedLocksOther,
    /// 护甲耐久为 0，无法装备。
    ArmorDurabilityZero,
    /// 护甲槽位与其规格不符（已知 `expected_slot`——护甲穿错了槽）。
    ArmorSlotMismatch { expected_slot: String },
    /// 护甲 `template_id` 无法解析出应装槽位（`equip_slot_for_item_id` 返回 `None`，
    /// 数据/注册表缺口——不遵循 `armor_<material>_<slot>` 命名）。与 `ArmorSlotMismatch`
    /// 语义不同：那是"槽位已知但穿错了"，这是"槽位压根解析不出来"，不携带
    /// 任何可下发的槽位信息（不得塞 `"unknown"` 占位字符串糊弄 client 中文文案）。
    ArmorSlotUnresolvable,
    /// 背包件 `ContainerSpec.equip_slot` 与目标身体槽不符。`expected_slot` 是背包声明的槽位 key。
    PackEquipSlotMismatch { expected_slot: String },
    /// 身体槽 worn 层已满（决议 #3/#12/#17）。`slot` 是槽位 key，`cap` 是该槽上限。
    WornCapFull { slot: String, cap: u8 },
    /// 境界不足——并入伪皮胸槽境界门控（原 `client_request_handler.rs:9896-9925` 独立硬编码分支）。
    /// `required_realm` 存 `realm_to_string` 输出的英文 tag（如 `"Condense"`）。
    RealmTooLow { required_realm: String },
    /// plan-race-system-v1 P3b（决议 §8.1 #5）—— `ItemTemplate.wearer_race` 判定域用
    /// **当前形态（Form）身份**（未易形时 = 本体），与功法习得/施放门（判本体）不同轴。
    RaceMismatch,
}

impl InventoryMoveRejectReason {
    /// 下发 `InventoryMoveRejectedV1.reason` 用的 snake_case string tag（wire 形状安全：
    /// string tag 而非 proto enum，避免枚举前缀 noOp，见 plan-wire-format-bridge-v1 教训）。
    pub fn to_wire_tag(&self) -> &'static str {
        match self {
            Self::FromLocationMismatch => "from_location_mismatch",
            Self::InstanceNotFound => "instance_not_found",
            Self::UnknownContainerId => "unknown_container_id",
            Self::TargetOutOfBounds => "target_out_of_bounds",
            Self::TargetOccupied { .. } => "target_occupied",
            Self::MultiOverlapNotSupported => "multi_overlap_not_supported",
            Self::HotbarIndexOutOfRange => "hotbar_index_out_of_range",
            Self::HotbarOccupied => "hotbar_occupied",
            Self::SwapFootprintMismatch => "swap_footprint_mismatch",
            Self::UnknownItemTemplate => "unknown_item_template",
            Self::WornStackNotTop => "worn_stack_not_top",
            Self::ForbiddenInHotbar { .. } => "forbidden_in_hotbar",
            Self::PackDetached { .. } => "pack_detached",
            Self::HeldWornMismatch => "held_worn_mismatch",
            Self::EquipCategoryMismatch => "equip_category_mismatch",
            Self::OffHandTypeMismatch => "off_hand_type_mismatch",
            Self::HandOccupied => "hand_occupied",
            Self::TwoHandedLocksOther => "two_handed_locks_other",
            Self::ArmorDurabilityZero => "armor_durability_zero",
            Self::ArmorSlotMismatch { .. } => "armor_slot_mismatch",
            Self::ArmorSlotUnresolvable => "armor_slot_unresolvable",
            Self::PackEquipSlotMismatch { .. } => "pack_equip_slot_mismatch",
            Self::WornCapFull { .. } => "worn_cap_full",
            Self::RealmTooLow { .. } => "realm_too_low",
            Self::RaceMismatch => "race_mismatch",
        }
    }

    /// 境界不足时的 required_realm 英文 tag（其余变体 `None`）——`InventoryMoveRejectedV1.required_realm`。
    pub fn required_realm(&self) -> Option<&str> {
        match self {
            Self::RealmTooLow { required_realm } => Some(required_realm.as_str()),
            _ => None,
        }
    }

    /// worn_cap 满 / 护甲槽位不符 / 背包 equip_slot 不符时的槽位 key（其余变体 `None`）——
    /// `InventoryMoveRejectedV1.slot`。
    pub fn slot(&self) -> Option<&str> {
        match self {
            Self::WornCapFull { slot, .. } => Some(slot.as_str()),
            Self::ArmorSlotMismatch { expected_slot } => Some(expected_slot.as_str()),
            Self::PackEquipSlotMismatch { expected_slot } => Some(expected_slot.as_str()),
            _ => None,
        }
    }

    /// worn_cap 满时的槽位上限（其余变体 `None`）——`InventoryMoveRejectedV1.cap`。
    pub fn cap(&self) -> Option<u32> {
        match self {
            Self::WornCapFull { cap, .. } => Some(u32::from(*cap)),
            _ => None,
        }
    }

    /// 人类可读日志文案，供既有 `tracing::warn!("...: {reason}")` 继续打日志
    /// （内容与 P0 前散落各处的原始 `String` 文案尽量保持一致）。
    pub fn to_log_string(&self) -> String {
        match self {
            Self::FromLocationMismatch => "from-location does not hold instance".to_string(),
            Self::InstanceNotFound => "instance not found in inventory".to_string(),
            Self::UnknownContainerId => "unknown container_id".to_string(),
            Self::TargetOutOfBounds => "target rectangle exceeds container bounds".to_string(),
            Self::TargetOccupied { instance_id } => {
                format!("target overlaps instance {instance_id}")
            }
            Self::MultiOverlapNotSupported => {
                "target overlaps multiple items — multi-overlap not supported".to_string()
            }
            Self::HotbarIndexOutOfRange => "hotbar index out of range".to_string(),
            Self::HotbarOccupied => "hotbar index occupied".to_string(),
            Self::SwapFootprintMismatch => {
                "swap rejected: occupant footprint differs from dragged item".to_string()
            }
            Self::UnknownItemTemplate => "unknown item template id".to_string(),
            Self::WornStackNotTop => {
                "该件被上层压住，请先脱下上层（worn 栈 LIFO，仅栈顶可卸下）".to_string()
            }
            Self::ForbiddenInHotbar { category } => {
                format!("{category:?} cannot move to hotbar; must stay in equipped slots")
            }
            Self::PackDetached { owner_instance_id } => format!(
                "背包已不在身上，无法放入内含物：背包件 (instance {owner_instance_id}) \
                 已丢弃/销毁；请重新拾取该背包"
            ),
            Self::HeldWornMismatch => {
                "手槽/身体槽装备态不匹配（手槽只能 held，身体槽只能 worn）".to_string()
            }
            Self::EquipCategoryMismatch => "item category cannot equip to target slot".to_string(),
            Self::OffHandTypeMismatch => {
                "weapon cannot equip to off_hand; only dagger/fist are allowed".to_string()
            }
            Self::HandOccupied => "该手已持械，请先卸下".to_string(),
            Self::TwoHandedLocksOther => "双手兵器占用双手，对侧已锁定".to_string(),
            Self::ArmorDurabilityZero => "armor cannot equip; durability is 0".to_string(),
            Self::ArmorSlotMismatch { expected_slot } => {
                format!("armor cannot equip to this slot; expected {expected_slot}")
            }
            Self::ArmorSlotUnresolvable => {
                "armor template_id does not resolve to any known equip slot (registry gap)"
                    .to_string()
            }
            Self::PackEquipSlotMismatch { expected_slot } => {
                format!("container.equip_slot `{expected_slot}`; cannot equip to target slot")
            }
            Self::WornCapFull { slot, cap } => {
                format!("该部位 {slot} 已穿戴 {cap} 层，无法再叠加")
            }
            Self::RealmTooLow { required_realm } => {
                format!("realm too low; required {required_realm}")
            }
            Self::RaceMismatch => {
                "item cannot be worn by current form's race (wearer_race gate rejected)".to_string()
            }
        }
    }
}

impl std::fmt::Display for InventoryMoveRejectReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_log_string())
    }
}

/// 兼容既有调用点：`attach_at_location` / `validate_attach_fits` 等 helper 的
/// `Result<_, InventoryMoveRejectReason>` 通过 `?` 传播进仍返回 `Result<_, String>`
/// 的兄弟函数（`exchange_inventory_items` / `pickup_dropped_loot_instance` /
/// `apply_treasure_activate` 等，均在本 plan 范围外，不改其签名）。
impl From<InventoryMoveRejectReason> for String {
    fn from(reason: InventoryMoveRejectReason) -> Self {
        reason.to_log_string()
    }
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroppedLootEntry {
    pub instance_id: u64,
    pub source_container_id: String,
    pub source_row: u8,
    pub source_col: u8,
    pub world_pos: [f64; 3],
    pub dimension: DimensionKind,
    pub item: ItemInstance,
}

#[derive(Default, Resource, Debug, Clone)]
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
/// plan-rotate-v1 — `rotated=true` 表示落位前先把该 instance 的 `grid_w`/`grid_h`
/// 互换（拖拽中按 R 旋转，2x1 ↔ 1x2）。旋转只对容器网格目标生效：
/// - 目标是 Equip / Hotbar（非网格落位）时忽略旋转标志，保持原朝向——这些槽位
///   不感知 footprint 方向，静默旋转只会在物品回到网格时造成意外形状。
/// - 正方形物品（含 1x1，`grid_w == grid_h`）互换是恒等操作，直接 no-op。
/// - 所有校验（越界 / 碰撞 / swap footprint）均以旋转后的尺寸进行；任何拒绝路径
///   返回前 inventory 均未被写入（校验用的是 clone），不会留下已互换的脏状态。
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
    rotated: bool,
) -> Result<InventoryMoveOutcome, InventoryMoveRejectReason> {
    // plan-race-system-v1 P3b —— 老签名默认人类/人形身份（见 `validate_move_semantics`
    // 同款说明）；既有调用点（大量既有单测 + 未接身份解析的路径）继续用它。真正需要
    // Form 身份种族门的生产路径走 `apply_inventory_move_with_race`。
    let default_race = RaceId::new(HUMAN_RACE_ID);
    apply_inventory_move_with_race(
        inventory,
        registry,
        instance_id,
        from,
        to,
        rotated,
        &default_race,
        true,
    )
}

/// plan-race-system-v1 P3b（决议 §8.1 #5）—— 携带 Form 身份（当前形态 race_id +
/// is_humanoid，未易形时 = 本体）的装备移动入口。生产路径：
/// `client_request_handler::handle_inventory_move`（`InventoryMoveIntent` /
/// `EquipFalseSkin` 两条 C2S 分支）。
#[allow(clippy::too_many_arguments)]
pub fn apply_inventory_move_with_race(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    instance_id: u64,
    from: &crate::schema::inventory::InventoryLocationV1,
    to: &crate::schema::inventory::InventoryLocationV1,
    rotated: bool,
    form_race_id: &RaceId,
    form_is_humanoid: bool,
) -> Result<InventoryMoveOutcome, InventoryMoveRejectReason> {
    if !location_holds_instance(inventory, instance_id, from) {
        return Err(InventoryMoveRejectReason::FromLocationMismatch);
    }

    let original_item =
        clone_item_at(inventory, instance_id).ok_or(InventoryMoveRejectReason::InstanceNotFound)?;

    // plan-rotate-v1 — 只在网格目标 + 非正方形 footprint 时真正互换；克隆件上互换，
    // 校验全部通过、真正 attach 时才写回 inventory，拒绝路径不产生脏状态。
    let apply_rotation = rotated
        && matches!(
            to,
            crate::schema::inventory::InventoryLocationV1::Container { .. }
        )
        && original_item.grid_w != original_item.grid_h;
    let mut item = original_item.clone();
    if apply_rotation {
        std::mem::swap(&mut item.grid_w, &mut item.grid_h);
    }

    validate_move_semantics_with_race(
        registry,
        inventory,
        &item,
        from,
        to,
        form_race_id,
        form_is_humanoid,
    )?;

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
                return Err(InventoryMoveRejectReason::SwapFootprintMismatch);
            }
            // Build a temp inventory after detaching both, then check occupant
            // fits at `from` against remaining items.
            let occupant_id = occupant.instance_id;
            detach_instance(inventory, instance_id);
            detach_instance(inventory, occupant_id);
            // Validate occupant fits at `from` (excluding both — both detached).
            if let Err(reason) = validate_attach_fits(inventory, &occupant, from) {
                // Restore originals to keep server state coherent on rare rejection.
                // plan-rotate-v1 — 回滚必须放回「原朝向」的件（original_item）：
                // 旋转后的 footprint 在原锚点可能与邻居重叠，原朝向则必然合法。
                attach_at_location(inventory, original_item, from)
                    .expect("restoring original from is always valid (just detached)");
                attach_at_location(inventory, occupant, to)
                    .expect("restoring original to is always valid (just detached)");
                return Err(reason);
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

/// plan-forge-session-entry-wiring-v1 §4.1#4 — 起炉原子扣料的缺料清单条目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeMaterialDeficit {
    /// 材料名——与调用方传入 `consume_forge_materials_atomic` 的 material 字符串一致
    /// （矿物 canonical name，如 `"fan_tie"`，或非矿物锻造用料 template_id，如 `"ling_mu_gun"`）。
    pub material: String,
    pub have: u32,
    pub need: u32,
}

/// 一件 item 是否算作某 forge 材料——按 `mineral_id == Some(material)`（矿物）或
/// `template_id == material`（非矿物锻造用料，如 spirit wood 系）匹配，二者任一命中即可。
fn item_matches_forge_material(item: &ItemInstance, material: &str) -> bool {
    item.mineral_id.as_deref() == Some(material) || item.template_id == material
}

/// 服务端按 forge 材料名统计玩家 inventory 内某材料的总数（containers + hotbar；
/// 不含 equipped —— 装备中的东西不该被当材料吃掉，与 `count_template_in_inventory` 同规则）。
fn count_forge_material_in_inventory(inventory: &PlayerInventory, material: &str) -> u32 {
    let from_containers: u32 = inventory
        .containers
        .iter()
        .flat_map(|c: &ContainerState| c.items.iter())
        .filter(|p| item_matches_forge_material(&p.instance, material))
        .map(|p| p.instance.stack_count)
        .sum();
    let from_hotbar: u32 = inventory
        .hotbar
        .iter()
        .filter_map(|s| s.as_ref())
        .filter(|i: &&ItemInstance| item_matches_forge_material(i, material))
        .map(|i| i.stack_count)
        .sum();
    from_containers + from_hotbar
}

/// plan-forge-session-entry-wiring-v1 §4.1#4（CRUX）—— 起炉原子扣料。
///
/// 与最终受理判定同层调用（`handle_start_forge_requests` 建会话前）：先对全部
/// `(material, count)` 做只读盘点，任一材料背包持有量不足 → **整体零改动**，返回
/// `Err(缺料清单)`（调用方据此拒绝起炉、不建会话、发 reject 回执）；全部足量才真正
/// 扣除（containers 优先，hotbar 兜底，与 `consume_materials_from_inventory` 同扣除
/// 顺序），扣除后 `bump_revision` 一次。
///
/// 这同时封堵了「引擎只记账 `committed_materials`、从不核验玩家是否真持有 `req.materials`
/// 声明」的 anti-cheat 漏洞——任何拒绝路径（含 Waste billet，调用方在那之前就已
/// `continue`，根本不会调用本函数）都不吞料。
pub fn consume_forge_materials_atomic(
    inventory: &mut PlayerInventory,
    materials: &[(String, u32)],
) -> Result<(), Vec<ForgeMaterialDeficit>> {
    // 同一材料在 materials 中可能出现多次（调用方一般已 dedupe，但这里独立防御）。
    let mut needed: Vec<(String, u32)> = Vec::new();
    for (material, count) in materials {
        if let Some(entry) = needed.iter_mut().find(|(m, _)| m == material) {
            entry.1 += count;
        } else {
            needed.push((material.clone(), *count));
        }
    }

    // 阶段一：只读盘点，任一不足则整体不改动。
    let mut deficits = Vec::new();
    for (material, need) in &needed {
        let have = count_forge_material_in_inventory(inventory, material);
        if have < *need {
            deficits.push(ForgeMaterialDeficit {
                material: material.clone(),
                have,
                need: *need,
            });
        }
    }
    if !deficits.is_empty() {
        return Err(deficits);
    }

    // 阶段二：全部足量，真正扣除。
    let mut consumed_any = false;
    for (material, need) in &needed {
        let mut remaining = *need;
        if remaining == 0 {
            continue;
        }
        'containers: for container in inventory.containers.iter_mut() {
            let mut i = 0;
            while i < container.items.len() {
                if item_matches_forge_material(&container.items[i].instance, material) {
                    let take = remaining.min(container.items[i].instance.stack_count);
                    container.items[i].instance.stack_count -= take;
                    remaining -= take;
                    consumed_any |= take > 0;
                    if container.items[i].instance.stack_count == 0 {
                        container.items.remove(i);
                        continue;
                    }
                }
                i += 1;
                if remaining == 0 {
                    break 'containers;
                }
            }
        }
        if remaining > 0 {
            for slot in inventory.hotbar.iter_mut() {
                if remaining == 0 {
                    break;
                }
                let drop_slot = if let Some(item) = slot.as_mut() {
                    if item_matches_forge_material(item, material) {
                        let take = remaining.min(item.stack_count);
                        item.stack_count -= take;
                        remaining -= take;
                        consumed_any |= take > 0;
                        item.stack_count == 0
                    } else {
                        false
                    }
                } else {
                    false
                };
                if drop_slot {
                    *slot = None;
                }
            }
        }
        debug_assert_eq!(
            remaining, 0,
            "consume_forge_materials_atomic: 缺料盘点已通过但 `{material}` 实扣仍缺 {remaining}\
             （count_forge_material_in_inventory 与实扣逻辑的匹配规则失步）"
        );
    }

    if consumed_any {
        bump_revision(inventory);
    }
    Ok(())
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
    pending_tsy_deaths: Query<&PendingTsyDeathDrop>,
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
        let pending_tsy_presence = pending_tsy_deaths
            .get(ev.entity)
            .ok()
            .map(|ctx| &ctx.presence);
        let tsy_presence = pending_tsy_presence.or_else(|| presences.get(ev.entity).ok());
        if let Some(presence) = tsy_presence {
            let tsy_outcome = tsy_death_drop::apply_tsy_death_drop(
                &mut inventory,
                &registry,
                presence,
                base,
                seed,
            );
            clear_death_drop_window_components(&mut commands, ev.entity);
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

fn clear_death_drop_window_components(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<(
        DeathDropAnchor,
        PendingTsyDeathDrop,
        crate::world::tsy::TsyPresence,
    )>();
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
        .values()
        .filter_map(|contents| contents.held.as_ref())
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
    registry: &ItemRegistry,
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

    // plan-bughunt-inventory-transfer-orphan-pack-v1 P0：全量 drain 后 `from.equipped` 已清空，
    // 任何 `pack_<id>` 容器此刻必定失去 owner、成为孤儿——但上面只 drain 了 items，容器壳本身
    // 仍留在 `from.containers` 里（此时已空，drain 后没有可 spill 的内含物）。不显式 rebuild 的话，
    // loader（`inventory_has_orphan_pack_container`）后续会把这份 inventory 判成污染档整体丢弃，
    // 回退默认新手 loadout（详见 plan 文档复现链路）。此处用 rebuild 统一收口：清空孤儿容器壳、
    // 补齐 body_pocket、重算 max_weight，恢复「源码态合法 == 持久化态合法」的不变量。
    let leftover = rebuild_containers_from_equipment(from, registry);
    debug_assert!(
        leftover.is_empty(),
        "transfer_all_inventory_contents: rebuild 后不应有 spill 溢出物（drain 已清空容器内含物）"
    );

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
                quick_access: false, // 静态 main_pack 兜底容器，非快捷来源。
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

/// plan-tarkov-backpack-v1 套包修复（fix/tarkov-nest-persistence）— 找出「玩家身上直接携带」
/// 的带 `container_spec` 的背包件，产出 `(instance, ContainerSpec)`。
///
/// **携带面 = worn / held / hotbar / `body_pocket`（贴身暗袋）**。用于
/// `rebuild_containers_from_equipment` 的 live_ids：背包件只要在这些携带位之一，其 `pack_<id>`
/// 容器都应保留、内含物不 spill；只有背包真离开玩家（丢地/销毁 → 不在携带面）才 spill。
///
/// **为何不扫 `pack_*` 容器内含物（grid 内的背包件）**：P5 决议 #1「嵌套深度 2 层封顶」固化——
/// worn 背包(层1) → 其 grid(层2) → 物品，放进 grid 的背包件是「货物」，**不**派生第 3 层可访问
/// 容器（`rebuild_does_not_expand_container_item_placed_inside_grid_two_layer_cap` 锁死）。
/// 同理 `main_pack` 等其它 grid 容器内的背包件也视为货物。仅 `body_pocket` 是与 worn 同级的
/// 「贴身携带位」，故纳入携带面——这正是「卸包到暗袋」bug 的修复点。
///
/// 与 `worn_container_items` 的区别 = 携带面（worn+held+hotbar+body_pocket vs 仅 worn）。
/// `compute_max_weight` 故意仍走 `worn_container_items`：只有穿戴态背包提供负重加成。
fn find_pack_instances_anywhere<'a>(
    inventory: &'a PlayerInventory,
    registry: &'a ItemRegistry,
) -> impl Iterator<Item = (&'a ItemInstance, &'a ContainerSpec)> {
    let worn = inventory.equipped.values().flat_map(|s| s.worn.iter());
    let held = inventory.equipped.values().filter_map(|s| s.held.as_ref());
    let in_body_pocket = inventory
        .containers
        .iter()
        .filter(|c| c.id == BODY_POCKET_CONTAINER_ID)
        .flat_map(|c| c.items.iter().map(|p| &p.instance));
    let in_hotbar = inventory.hotbar.iter().filter_map(|o| o.as_ref());
    worn.chain(held)
        .chain(in_body_pocket)
        .chain(in_hotbar)
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
            // body_pocket 的快捷资格由 snapshot 特判恒 true；此缓存位保持 false。
            quick_access: false,
        });
    }

    // 2. 扫所有身体槽 worn 层里带 container_spec 的背包件，确保各自容器存在。
    //    plan-tarkov-backpack-v1 P0（交付物 #2）：创建/刷新 `pack_<id>` 容器时写
    //    `owner_instance_id = Some(instance_id)`，建立背包件 ↔ 容器的语义归属。
    //    plan-tarkov-backpack-v1 套包修复：live 判据从「仅 worn」扩到「身上任意位置」
    //    （find_pack_instances_anywhere），背包移入 body_pocket / 另一 pack / hotbar / held
    //    后其 pack_<id> 容器仍 live、内含物不 spill；只有真离开玩家（丢地/销毁）才孤儿化。
    // [快捷] 元组末位携带 owner 背包件 ContainerSpec.quick_access，回填进 ContainerState.quick_access，
    // 使 snapshot 无需反查 registry。未来「快捷腰包」模板设 quick_access=true 即随此链路下发生效。
    let live_specs: Vec<(String, u8, u8, String, u64, bool)> =
        find_pack_instances_anywhere(inventory, registry)
            .map(|(item, spec)| {
                (
                    container_id_for_worn_pack(item.instance_id),
                    spec.rows,
                    spec.cols,
                    item.display_name.clone(),
                    item.instance_id,
                    spec.quick_access,
                )
            })
            .collect();
    let live_ids: std::collections::HashSet<String> = live_specs
        .iter()
        .map(|(id, _, _, _, _, _)| id.clone())
        .collect();

    for (container_id, rows, cols, name, instance_id, quick_access) in live_specs {
        if let Some(existing) = inventory
            .containers
            .iter_mut()
            .find(|c| c.id == container_id)
        {
            existing.rows = rows;
            existing.cols = cols;
            existing.owner_instance_id = Some(instance_id);
            existing.quick_access = quick_access;
        } else {
            inventory.containers.push(ContainerState {
                id: container_id,
                name,
                rows,
                cols,
                items: Vec::new(),
                owner_instance_id: Some(instance_id),
                quick_access,
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
                // plan-tarkov-backpack-v1 套包修复 §8：+0.5 Y margin，防脚下 +0.35/+0.35
                // 对角处为台阶/坡顶时物品渲染陷入地下。
                player_pos[1] + 0.5,
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

/// plan-race-system-v1 P4（决议 §6）—— 易形解除（手动再 cast / 死亡 / 下线三种触发）
/// 后的装备门重校验：本体（intrinsic）race/is_humanoid 恢复权威真源后，遍历全部身体槽
/// worn 层 + 手槽 held 装备，凡 `ItemTemplate.wearer_race` 不再放行本体身份的物件——
/// 移出装备槽，优先塞进玩家现有容器（复用 `find_first_fit_container_location` +
/// `attach_at_location`，与 `rebuild_and_drop_overflow` 同一套 spill 机制），容器也放
/// 不下时转 `DroppedLootRegistry` 地面掉落（禁止静默销毁——塔科夫式直觉）。
///
/// 返回 (背包收容的 instance_id 列表, 地面掉落的 instance_id 列表)，供调用方日志 /
/// 测试断言"解除后无非法装备残留"。
pub fn enforce_intrinsic_gate_on_morph_release(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    dropped_registry: &mut DroppedLootRegistry,
    intrinsic_race: &RaceId,
    intrinsic_is_humanoid: bool,
    player_pos: [f64; 3],
    player_dimension: DimensionKind,
) -> (Vec<u64>, Vec<u64>) {
    let template_allows = |template_id: &str| -> bool {
        registry
            .get(template_id)
            .map(|template| {
                template
                    .wearer_race
                    .allows(intrinsic_race, intrinsic_is_humanoid)
            })
            // 未知 template（数据损坏）保守放行——不该因为查不到模板就把玩家装备扒光。
            .unwrap_or(true)
    };

    // 1. 从装备槽摘出全部本体身份不再允许的 instance（held 优先摘，再摘 worn 栈，
    //    自顶向下——与既有卸下语义"先卸最外层"一致）。
    let mut displaced: Vec<ItemInstance> = Vec::new();
    for contents in inventory.equipped.values_mut() {
        if let Some(held) = &contents.held {
            if !template_allows(&held.template_id) {
                if let Some(item) = contents.held.take() {
                    displaced.push(item);
                }
            }
        }
        let mut kept_worn = Vec::with_capacity(contents.worn.len());
        for item in std::mem::take(&mut contents.worn) {
            if template_allows(&item.template_id) {
                kept_worn.push(item);
            } else {
                displaced.push(item);
            }
        }
        contents.worn = kept_worn;
    }

    if displaced.is_empty() {
        return (Vec::new(), Vec::new());
    }

    // 2. 装备槽结构变化（worn 背包件可能被摘下）—— 刷新容器列表，让摘下的背包件
    //    自己的 pack_<id> 容器先归位/孤儿化，再尝试安放被摘下的物件。
    let overflow_from_rebuild = rebuild_containers_from_equipment(inventory, registry);
    inventory.max_weight = compute_max_weight(inventory, registry);

    let mut stashed_ids = Vec::new();
    let mut dropped_ids = Vec::new();
    for item in displaced.into_iter().chain(overflow_from_rebuild) {
        match find_first_fit_container_location(inventory, &item) {
            Some(location) => {
                let instance_id = item.instance_id;
                match attach_at_location(inventory, item, &location) {
                    Ok(()) => stashed_ids.push(instance_id),
                    Err(_) => {
                        // 罕见竞态（location 校验后容器状态变化）——不丢件，转掉落。
                        dropped_ids.push(instance_id);
                    }
                }
            }
            None => {
                let instance_id = item.instance_id;
                let next_idx = dropped_registry.entries.len();
                let dropped = DroppedLootEntry {
                    instance_id,
                    source_container_id: "morph_release_gate_overflow".to_string(),
                    source_row: 0,
                    source_col: 0,
                    world_pos: [
                        player_pos[0] + 0.35 + next_idx as f64 * 0.1,
                        player_pos[1] + 0.5,
                        player_pos[2] + 0.35,
                    ],
                    dimension: player_dimension,
                    item,
                };
                dropped_registry.entries.insert(instance_id, dropped);
                dropped_ids.push(instance_id);
            }
        }
    }
    (stashed_ids, dropped_ids)
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

    // Keep the visual spread local to the player. Using the global registry length here
    // eventually puts a fresh drop outside the 2.5-block pickup radius during long e2e runs.
    let next_idx = registry.entries.len().min(8);
    let dropped = DroppedLootEntry {
        instance_id,
        source_container_id,
        source_row,
        source_col,
        world_pos: [
            player_pos[0] + 0.35 + next_idx as f64 * 0.1,
            // plan-tarkov-backpack-v1 套包修复 §8：+0.5 Y margin，防脚下对角处为台阶/坡顶时陷地。
            player_pos[1] + 0.5,
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
) -> Result<Option<ItemInstance>, InventoryMoveRejectReason> {
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
                .ok_or(InventoryMoveRejectReason::UnknownContainerId)?;

            let row_u8 =
                u8::try_from(*row).map_err(|_| InventoryMoveRejectReason::TargetOutOfBounds)?;
            let col_u8 =
                u8::try_from(*col).map_err(|_| InventoryMoveRejectReason::TargetOutOfBounds)?;
            if u16::from(row_u8) + u16::from(item.grid_h) > u16::from(container.rows)
                || u16::from(col_u8) + u16::from(item.grid_w) > u16::from(container.cols)
            {
                return Err(InventoryMoveRejectReason::TargetOutOfBounds);
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
                        return Err(InventoryMoveRejectReason::TargetOccupied {
                            instance_id: occ.instance.instance_id,
                        });
                    }
                    Ok(Some(occ.instance.clone()))
                }
                _ => Err(InventoryMoveRejectReason::MultiOverlapNotSupported),
            }
        }
        // plan-layered-equip-v1 P0.2（决议 #3 拒绝不顶替）— equip 落位不做 swap 顶替；
        // 满 / 占用由 validate_move_semantics / validate_attach_fits 拒绝。恒无 displaced。
        InventoryLocationV1::Equip { .. } => Ok(None),
        InventoryLocationV1::Hotbar { index } => {
            let idx = *index as usize;
            if idx >= inventory.hotbar.len() {
                return Err(InventoryMoveRejectReason::HotbarIndexOutOfRange);
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
) -> Result<(), InventoryMoveRejectReason> {
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
                .ok_or(InventoryMoveRejectReason::UnknownContainerId)?;
            let row_u8 =
                u8::try_from(*row).map_err(|_| InventoryMoveRejectReason::TargetOutOfBounds)?;
            let col_u8 =
                u8::try_from(*col).map_err(|_| InventoryMoveRejectReason::TargetOutOfBounds)?;
            if u16::from(row_u8) + u16::from(item.grid_h) > u16::from(container.rows)
                || u16::from(col_u8) + u16::from(item.grid_w) > u16::from(container.cols)
            {
                return Err(InventoryMoveRejectReason::TargetOutOfBounds);
            }
            let candidate = PlacedItemState {
                row: row_u8,
                col: col_u8,
                instance: item.clone(),
            };
            for existing in &container.items {
                if placed_item_footprints_overlap(existing, &candidate) {
                    return Err(InventoryMoveRejectReason::TargetOccupied {
                        instance_id: existing.instance.instance_id,
                    });
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
                    let cap = worn_cap(key);
                    if cur as u8 >= cap {
                        return Err(InventoryMoveRejectReason::WornCapFull {
                            slot: key.to_string(),
                            cap,
                        });
                    }
                }
                EquipStateV1::Held => {
                    if contents.is_some_and(|c| c.held.is_some()) {
                        return Err(InventoryMoveRejectReason::HandOccupied);
                    }
                }
            }
            Ok(())
        }
        InventoryLocationV1::Hotbar { index } => {
            let idx = *index as usize;
            if idx >= inventory.hotbar.len() {
                return Err(InventoryMoveRejectReason::HotbarIndexOutOfRange);
            }
            if inventory.hotbar[idx].is_some() {
                return Err(InventoryMoveRejectReason::HotbarOccupied);
            }
            Ok(())
        }
    }
}

// plan-race-system-v1 P3b —— 生产路径已改走 `validate_move_semantics_with_race`（携带
// Form 身份），本无种族参数的老签名只剩既有单测直接调用；非 test 构建里私有函数无
// 外部调用点会被 dead_code 误报，同 `persistence::mod` 既有惯例标注。
#[cfg_attr(not(test), allow(dead_code))]
fn validate_move_semantics(
    registry: &ItemRegistry,
    inventory: &PlayerInventory,
    item: &ItemInstance,
    from: &crate::schema::inventory::InventoryLocationV1,
    to: &crate::schema::inventory::InventoryLocationV1,
) -> Result<(), InventoryMoveRejectReason> {
    // plan-race-system-v1 P3b — 既有调用点（大量既有单测 + 未接 body_plan 的其他生产
    // 路径）默认用人类/人形身份走这条无种族门控的老签名：`RaceGateOwned::Any`/`Humanoid`
    // 两档对 (human, true) 恒放行，只有真正需要种族门断言的新测试才需要用
    // `validate_move_semantics_with_race` 传入非默认身份。
    let default_race = RaceId::new(HUMAN_RACE_ID);
    validate_move_semantics_with_race(registry, inventory, item, from, to, &default_race, true)
}

/// plan-race-system-v1 P3b（决议 §8.1 #5）—— 携带 **Form 身份**（当前形态 race_id +
/// is_humanoid，未易形时 = 本体）的装备门校验入口。生产路径见
/// `client_request_handler::handle_inventory_move`；`validate_move_semantics`
/// （无种族参数的老签名）是本函数套上默认人类人形身份的薄包装，供不关心种族门的既有
/// 调用点/单测继续使用而不必改签名。
fn validate_move_semantics_with_race(
    registry: &ItemRegistry,
    inventory: &PlayerInventory,
    item: &ItemInstance,
    from: &crate::schema::inventory::InventoryLocationV1,
    to: &crate::schema::inventory::InventoryLocationV1,
    form_race_id: &RaceId,
    form_is_humanoid: bool,
) -> Result<(), InventoryMoveRejectReason> {
    use crate::schema::inventory::InventoryLocationV1;

    let template = registry
        .get(&item.template_id)
        .ok_or(InventoryMoveRejectReason::UnknownItemTemplate)?;

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
                return Err(InventoryMoveRejectReason::WornStackNotTop);
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
        InventoryLocationV1::Hotbar { .. } if template.weapon_spec.is_some() => {
            Err(InventoryMoveRejectReason::ForbiddenInHotbar {
                category: ItemCategory::Weapon,
            })
        }
        InventoryLocationV1::Hotbar { .. } if matches!(template.category, ItemCategory::Tool) => {
            Err(InventoryMoveRejectReason::ForbiddenInHotbar {
                category: ItemCategory::Tool,
            })
        }
        InventoryLocationV1::Hotbar { .. } if matches!(template.category, ItemCategory::Armor) => {
            Err(InventoryMoveRejectReason::ForbiddenInHotbar {
                category: ItemCategory::Armor,
            })
        }
        // plan-shield-block-v1 P0 — 盾牌（Shield 类）同样不能进 hotbar，必须留在 off_hand 槽。
        InventoryLocationV1::Hotbar { .. } if matches!(template.category, ItemCategory::Shield) => {
            Err(InventoryMoveRejectReason::ForbiddenInHotbar {
                category: ItemCategory::Shield,
            })
        }
        InventoryLocationV1::Hotbar { .. }
            if matches!(template.category, ItemCategory::Treasure) =>
        {
            Err(InventoryMoveRejectReason::ForbiddenInHotbar {
                category: ItemCategory::Treasure,
            })
        }
        InventoryLocationV1::Hotbar { .. }
            if matches!(template.category, ItemCategory::Container) =>
        {
            Err(InventoryMoveRejectReason::ForbiddenInHotbar {
                category: ItemCategory::Container,
            })
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
                form_race_id,
                form_is_humanoid,
            )
        }
        // plan-tarkov-backpack-v1 P2（交付物 #2，决议 #2/#5）— 穿戴态门控（server 侧）。
        // 拖入 `pack_<instance_id>` 容器时，校验该背包件当前确实穿戴在某身体槽 worn 层；
        // 背包件已被卸到手持/格子（非穿戴态）后其 `pack_<id>` 容器仍残留于 snapshot，
        // 但不可再被塞入新内含物——塔科夫式语义：卸下的包是「死容器」，重新穿上才能装东西。
        // 非 `pack_<id>` 容器（如 body_pocket / main_pack）放行（保持现状无门控）。
        InventoryLocationV1::Container { container_id, .. } => {
            if let Some(owner_instance_id) = worn_pack_instance_from_container_id(container_id) {
                // plan-tarkov-backpack-v1 套包修复（决议 #2 重定义）：放宽门控——背包件在携带面
                // （worn/held/hotbar/body_pocket）即视为「活背包」，可放入内含物；只有背包不在
                // 携带面（丢地/销毁，或仅作为 pack_* grid 货物——2 层封顶下不可开）才拒。与存活
                // 判据 find_pack_instances_anywhere 一致，避免「容器还在却拒绝拖入」的割裂。
                let owner_carried = find_pack_instances_anywhere(inventory, registry)
                    .any(|(item, _)| item.instance_id == owner_instance_id);
                if !owner_carried {
                    return Err(InventoryMoveRejectReason::PackDetached { owner_instance_id });
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
    // plan-race-system-v1 P3b（决议 §8.1 #5）—— 装备域判定用 **Form 身份**（当前形态，
    // 未易形时 = 本体）：`RaceGateOwned::Species` 判 `form_race_id`，`Humanoid` 判
    // `form_is_humanoid`，`Any` 恒通过。与功法习得/施放门（判本体 intrinsic 身份）不同轴，
    // 详见 plan §8.1 #5/#6 身份快照矩阵。
    form_race_id: &RaceId,
    form_is_humanoid: bool,
) -> Result<(), InventoryMoveRejectReason> {
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
            return Err(InventoryMoveRejectReason::HeldWornMismatch);
        }
        (false, EquipStateV1::Held) => {
            return Err(InventoryMoveRejectReason::HeldWornMismatch);
        }
        _ => {}
    }

    let slot_result: Result<(), InventoryMoveRejectReason> = match slot {
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
                return Err(InventoryMoveRejectReason::EquipCategoryMismatch);
            }
            // off_hand 武器仅 dagger/fist。
            if matches!(slot, EquipSlotV1::OffHand) && is_weapon {
                if let Some(spec) = template.weapon_spec.as_ref() {
                    if !matches!(spec.weapon_kind, WeaponKind::Dagger | WeaponKind::Fist) {
                        return Err(InventoryMoveRejectReason::OffHandTypeMismatch);
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
                return Err(InventoryMoveRejectReason::HandOccupied);
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
                            return Err(InventoryMoveRejectReason::TwoHandedLocksOther);
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
                    return Err(InventoryMoveRejectReason::TwoHandedLocksOther);
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
                return Err(InventoryMoveRejectReason::EquipCategoryMismatch);
            }

            if is_armor {
                if item.durability <= 0.0 {
                    return Err(InventoryMoveRejectReason::ArmorDurabilityZero);
                }
                let expected_slot =
                    crate::armor::mundane::equip_slot_for_item_id(&item.template_id)
                        .ok_or(InventoryMoveRejectReason::ArmorSlotUnresolvable)?;
                if expected_slot != *slot {
                    return Err(InventoryMoveRejectReason::ArmorSlotMismatch {
                        expected_slot: equip_slot_key(&expected_slot).to_string(),
                    });
                }
            }

            // 背包件：ContainerSpec.equip_slot 必须指向当前身体槽（决议 #17）。
            if is_container && !is_armor && !is_false_skin {
                if let Some(spec) = template.container_spec.as_ref() {
                    if spec.equip_slot != slot_key {
                        return Err(InventoryMoveRejectReason::PackEquipSlotMismatch {
                            expected_slot: spec.equip_slot.clone(),
                        });
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
                    return Err(InventoryMoveRejectReason::WornCapFull {
                        slot: slot_key.to_string(),
                        cap,
                    });
                }
            }
            Ok(())
        }
    };
    slot_result?;

    // plan-race-system-v1 P3b（决议 §8.1 #5，"校验统一进 validate_equip_to，槽位分支
    // 判定后、Ok(()) 前"）—— 种族门是槽位/类型/耐久等既有校验全部通过后的最后一道闸，
    // 对手槽与身体槽两个分支统一生效（法宝/兵刃/防具/背包件皆受 wearer_race 约束，
    // 绝大多数物品 `wearer_race = Any` 时恒放行，不影响既有行为）。
    if !template.wearer_race.allows(form_race_id, form_is_humanoid) {
        return Err(InventoryMoveRejectReason::RaceMismatch);
    }
    Ok(())
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
) -> Result<(), InventoryMoveRejectReason> {
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
                .ok_or(InventoryMoveRejectReason::UnknownContainerId)?;
            let row_u8 =
                u8::try_from(*row).map_err(|_| InventoryMoveRejectReason::TargetOutOfBounds)?;
            let col_u8 =
                u8::try_from(*col).map_err(|_| InventoryMoveRejectReason::TargetOutOfBounds)?;
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
                return Err(InventoryMoveRejectReason::HotbarIndexOutOfRange);
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
mod tests;
