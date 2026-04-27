use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Added, App, Client, Commands, Component, Entity, Position, Query, Resource, Update,
    Without,
};

use crate::cultivation::death_hooks::PlayerRevived;

// plan-tsy-loot-v1 §1.2 — 上古遗物模板池。
pub mod ancient_relics;
// plan-tsy-loot-v1 §4 — 干尸 component。
pub mod corpse;
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
pub const EQUIP_SLOT_TWO_HAND: &str = "two_hand";
pub const EQUIP_SLOT_TREASURE_BELT_0: &str = "treasure_belt_0";
pub const EQUIP_SLOT_TREASURE_BELT_1: &str = "treasure_belt_1";
pub const EQUIP_SLOT_TREASURE_BELT_2: &str = "treasure_belt_2";
pub const EQUIP_SLOT_TREASURE_BELT_3: &str = "treasure_belt_3";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemCategory {
    Pill,
    Herb,
    Weapon,
    Treasure,
    BoneCoin,
    Misc,
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
    BreakthroughBonus { magnitude: f64 },
    MeridianHeal { magnitude: f64, target: String },
    ContaminationCleanse { magnitude: f64 },
}

#[derive(Debug, Default)]
pub struct ItemRegistry {
    templates: HashMap<String, ItemTemplate>,
}

impl Resource for ItemRegistry {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadoutSpec {
    pub containers: Vec<ContainerState>,
    pub equipped: HashMap<String, ItemInstance>,
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
}

#[derive(Debug)]
pub struct DefaultLoadout(pub LoadoutSpec);

impl Resource for DefaultLoadout {}

#[derive(Debug)]
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

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct PlayerInventory {
    pub revision: InventoryRevision,
    pub containers: Vec<ContainerState>,
    pub equipped: HashMap<String, ItemInstance>,
    pub hotbar: [Option<ItemInstance>; 9],
    pub bone_coins: u64,
    pub max_weight: f64,
}

#[derive(Debug, Clone, Copy, Component, PartialEq)]
pub struct OverloadedMarker {
    pub current_weight: f64,
    pub max_weight: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InventoryGrantReceipt {
    pub revision: InventoryRevision,
    pub instance_id: u64,
    pub template_id: String,
    pub stack_count: u32,
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
    // plan-tsy-loot-v1 §2 — 上古遗物模板池 + 已 spawn family 集合。
    app.insert_resource(ancient_relics::AncientRelicPool::from_seed());
    app.insert_resource(tsy_loot_spawn::TsySpawnedFamilies::default());
    app.add_event::<DroppedItemEvent>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(
        Update,
        (
            apply_death_drop_on_revive,
            sync_overloaded_marker,
            // plan-tsy-loot-v1 §2.2 — 玩家踏入 family 时 spawn 1% 上古遗物（idempotent）。
            tsy_loot_spawn::tsy_loot_spawn_on_enter,
        ),
    );
}

pub(crate) fn attach_inventory_to_joined_clients(
    mut commands: Commands,
    mut allocator: valence::prelude::ResMut<InventoryInstanceIdAllocator>,
    default_loadout: valence::prelude::Res<DefaultLoadout>,
    joined_clients: Query<Entity, JoinedClientsWithoutInventoryFilter>,
) {
    for entity in &joined_clients {
        let player_inventory = instantiate_inventory_from_loadout(&default_loadout.0, &mut allocator)
            .unwrap_or_else(|error| {
                panic!(
                    "[bong][inventory] failed to instantiate default loadout for joined client {entity:?}: {error}"
                )
            });

        commands.entity(entity).insert(player_inventory);
        // plan-HUD-v1 §10.4 quickslot bindings — 加入空 default，后续 quick_slot_bind
        // 客户端 intent 会写入。挂在 inventory attach 旁边方便一起看。
        commands
            .entity(entity)
            .insert(crate::combat::components::QuickSlotBindings::default());
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

fn instantiate_inventory_from_loadout(
    loadout: &LoadoutSpec,
    allocator: &mut InventoryInstanceIdAllocator,
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
        });
    }

    let mut equipped = HashMap::with_capacity(loadout.equipped.len());
    for (slot_id, item) in &loadout.equipped {
        equipped.insert(slot_id.clone(), instantiate_item_instance(item, allocator)?);
    }

    let mut hotbar: [Option<ItemInstance>; 9] = Default::default();
    for (index, item) in loadout.hotbar.iter().enumerate() {
        hotbar[index] = item
            .as_ref()
            .map(|slot_item| instantiate_item_instance(slot_item, allocator))
            .transpose()?;
    }

    Ok(PlayerInventory {
        revision: InventoryRevision(1),
        containers,
        equipped,
        hotbar,
        bone_coins: loadout.bone_coins,
        max_weight: loadout.max_weight,
    })
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
    })
}

pub fn load_item_registry() -> Result<ItemRegistry, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_ITEMS_DIR);
    load_item_registry_from_dir(path)
}

fn load_item_registry_from_dir(path: impl AsRef<Path>) -> Result<ItemRegistry, String> {
    let path = path.as_ref();
    let entries = fs::read_dir(path).map_err(|error| {
        format!(
            "failed to read inventory item registry directory {}: {error}",
            path.display()
        )
    })?;

    let mut toml_paths: Vec<PathBuf> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let file_path = entry.path();
            let is_toml = file_path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"));
            is_toml.then_some(file_path)
        })
        .collect();
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

    Ok(ItemRegistry { templates })
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
    #[cfg(test)]
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
) -> Result<InventoryGrantReceipt, String> {
    if stack_count == 0 {
        return Err("add_item_to_player_inventory requires stack_count >= 1".to_string());
    }

    let template = registry
        .get(template_id)
        .ok_or_else(|| format!("unknown item template id `{template_id}`"))?;

    let instance_id = allocator.next_id()?;
    let instance = ItemInstance {
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
        freshness: None,
        mineral_id: None,
        charges: None,
    };

    let Some(main_pack) = inventory
        .containers
        .iter_mut()
        .find(|container| container.id == MAIN_PACK_CONTAINER_ID)
    else {
        return Err(format!(
            "player inventory missing required `{MAIN_PACK_CONTAINER_ID}` container"
        ));
    };

    main_pack.items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance,
    });

    inventory.revision.0 = inventory.revision.0.saturating_add(1);

    Ok(InventoryGrantReceipt {
        revision: inventory.revision,
        instance_id,
        template_id: template.id.clone(),
        stack_count,
    })
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
    grid_w: u8,
    grid_h: u8,
    base_weight: f64,
    rarity: String,
    spirit_quality_initial: f64,
    description: String,
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

fn default_qi_cost_mul() -> f32 {
    1.0
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemEffectToml {
    kind: String,
    magnitude: f64,
    target: Option<String>,
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
        let rarity = parse_item_rarity(self.rarity.as_str(), source_path, id.as_str())?;
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

        Ok(ItemTemplate {
            id,
            display_name,
            category,
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
        })
    }
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
        "weapon" => Ok(ItemCategory::Weapon),
        "treasure" => Ok(ItemCategory::Treasure),
        "bonecoin" | "bone_coin" | "bone-coins" | "bone_coins" => Ok(ItemCategory::BoneCoin),
        "misc" => Ok(ItemCategory::Misc),
        other => Err(format!(
            "{} item `{item_id}` has unknown category `{other}`",
            source_path.display()
        )),
    }
}

fn parse_item_rarity(raw: &str, source_path: &Path, item_id: &str) -> Result<ItemRarity, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "common" => Ok(ItemRarity::Common),
        "uncommon" => Ok(ItemRarity::Uncommon),
        "rare" => Ok(ItemRarity::Rare),
        "epic" => Ok(ItemRarity::Epic),
        "legendary" => Ok(ItemRarity::Legendary),
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
        other => Err(format!(
            "{} item `{item_id}` has unsupported effect kind `{other}`",
            source_path.display()
        )),
    }
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
            });
        }

        ensure_required_containers_present(&containers, source_path)?;

        let mut equipped = HashMap::new();
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

            if equipped.insert(slot_id.clone(), instance).is_some() {
                return Err(format!(
                    "{} has duplicate equip slot `{slot_id}` in loadout",
                    source_path.display()
                ));
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
pub struct InventoryDurabilityUpdate {
    pub revision: InventoryRevision,
    pub instance_id: u64,
    pub durability: f64,
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
pub struct DroppedLootEntry {
    pub instance_id: u64,
    pub source_container_id: String,
    pub source_row: u8,
    pub source_col: u8,
    pub world_pos: [f64; 3],
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
    let (slot_key, _slot_wire) = inventory
        .equipped
        .iter()
        .find_map(|(slot, item)| {
            (item.instance_id == instance_id)
                .then_some((slot.clone(), equip_slot_wire_from_runtime(slot.as_str())))
        })
        .ok_or_else(|| format!("equipped instance {instance_id} not found"))?;
    let item = inventory
        .equipped
        .get(&slot_key)
        .cloned()
        .ok_or_else(|| format!("equipped slot `{slot_key}` missing instance {instance_id}"))?;
    let to = find_first_fit_container_location(inventory, &item)
        .ok_or_else(|| format!("no free container slot for instance {instance_id}"))?;

    detach_instance(inventory, instance_id);
    attach_at_location(inventory, item, &to)?;
    bump_revision(inventory);
    Ok(InventoryMoveOutcome::Moved {
        revision: inventory.revision,
    })
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
    for item in inventory.equipped.values() {
        if item.instance_id == instance_id {
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

    if let Some(slot_key) = inventory
        .equipped
        .iter()
        .find_map(|(key, item)| (item.instance_id == instance_id).then(|| key.clone()))
    {
        let remove = inventory
            .equipped
            .get(&slot_key)
            .map(|item| item.stack_count <= 1)
            .unwrap_or(false);
        let remaining_stack = if remove {
            inventory.equipped.remove(&slot_key);
            0
        } else {
            let item = inventory
                .equipped
                .get_mut(&slot_key)
                .expect("equipped slot key should still exist");
            item.stack_count -= 1;
            item.stack_count
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
    presences: Query<&crate::world::tsy::TsyPresence>,
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
                let entry = DroppedLootEntry {
                    instance_id: record.instance.instance_id,
                    source_container_id: record.container_id.clone(),
                    source_row: record.row,
                    source_col: record.col,
                    world_pos: [base.x + 0.35 + idx as f64 * 0.1, base.y, base.z + 0.35],
                    item: record.instance.clone(),
                };
                dropped_registry.entries.insert(entry.instance_id, entry);
                combined.push(record.clone());
            }

            // §4.3：干尸实体落 corpse_pos。MVP 仅 Position + CorpseEmbalmed component；
            // visual marker mob 由后续 P3 plan-tsy-polish 接 Valence entity sync。
            let drop_ids: Vec<u64> = combined.iter().map(|r| r.instance.instance_id).collect();
            commands.spawn((
                Position(tsy_outcome.corpse_pos),
                corpse::CorpseEmbalmed {
                    family_id: presence.family_id.clone(),
                    died_at_tick: presence.entered_at_tick, // MVP：用 entered_tick 占位；P2 lifecycle 用真 death tick
                    death_cause: "tsy_death".to_string(),
                    drops: drop_ids,
                    activated_to_daoxiang: false,
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

        for (idx, dropped) in outcome.dropped.iter().enumerate() {
            let entry = DroppedLootEntry {
                instance_id: dropped.instance.instance_id,
                source_container_id: dropped.container_id.clone(),
                source_row: dropped.row,
                source_col: dropped.col,
                world_pos: [base.x + 0.35 + idx as f64 * 0.1, base.y, base.z + 0.35],
                item: dropped.instance.clone(),
            };
            dropped_registry.entries.insert(entry.instance_id, entry);
        }

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
    let protected_weapon_ids = inventory
        .equipped
        .iter()
        .filter(|(slot, item)| {
            matches!(
                slot.as_str(),
                EQUIP_SLOT_MAIN_HAND | EQUIP_SLOT_OFF_HAND | EQUIP_SLOT_TWO_HAND
            ) && item.durability >= 0.5
        })
        .filter_map(|(_, item)| {
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
    for (slot, item) in &inventory.equipped {
        let is_weapon_slot = matches!(
            slot.as_str(),
            EQUIP_SLOT_MAIN_HAND | EQUIP_SLOT_OFF_HAND | EQUIP_SLOT_TWO_HAND
        );
        if is_weapon_slot && protected_weapon_ids.contains(&item.instance_id) {
            continue;
        }
        candidate_ids.push(item.instance_id);
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

    let equipped_to_drop = inventory
        .equipped
        .iter()
        .filter(|(_, item)| selected.contains(&item.instance_id))
        .map(|(slot, item)| (slot.clone(), item.clone()))
        .collect::<Vec<_>>();
    for (slot, item) in equipped_to_drop {
        inventory.equipped.remove(&slot);
        dropped.push(DroppedItemRecord {
            container_id: slot,
            row: 0,
            col: 0,
            instance: item,
        });
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

pub fn calculate_current_weight(inventory: &PlayerInventory) -> f64 {
    let container_weight = inventory
        .containers
        .iter()
        .flat_map(|container| container.items.iter())
        .map(|entry| entry.instance.weight * entry.instance.stack_count as f64)
        .sum::<f64>();
    let equipped_weight = inventory
        .equipped
        .values()
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

pub fn dropped_loot_snapshot(registry: &DroppedLootRegistry) -> Vec<DroppedLootEntry> {
    let mut drops = registry.entries.values().cloned().collect::<Vec<_>>();
    // Deterministic ordering avoids client-side insertionOrder churn.
    drops.sort_by_key(|entry| entry.instance_id);
    drops
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
        crate::schema::inventory::InventoryLocationV1::Equip { slot } => {
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
        InventoryLocationV1::Equip { slot } => {
            let key = equip_slot_key(slot);
            match inventory.equipped.get(key) {
                None => Ok(None),
                Some(occupant) if occupant.instance_id == moving_instance_id => Ok(None),
                Some(occupant) => Ok(Some(occupant.clone())),
            }
        }
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
        InventoryLocationV1::Equip { slot } => {
            let key = equip_slot_key(slot);
            if inventory.equipped.contains_key(key) {
                return Err(format!("equip slot '{key}' occupied"));
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
    use crate::combat::weapon::WeaponKind;
    use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

    let template = registry
        .get(&item.template_id)
        .ok_or_else(|| format!("unknown item template id `{}`", item.template_id))?;
    let from_two_hand = matches!(
        from,
        InventoryLocationV1::Equip {
            slot: EquipSlotV1::TwoHand
        }
    );

    match to {
        InventoryLocationV1::Hotbar { .. } if template.weapon_spec.is_some() => Err(format!(
            "weapon `{}` cannot move to hotbar; weapons must stay in equipped slots",
            item.template_id
        )),
        InventoryLocationV1::Hotbar { .. }
            if matches!(template.category, ItemCategory::Treasure) =>
        {
            Err(format!(
                "treasure `{}` cannot move to hotbar; treasures must stay in equipped slots",
                item.template_id
            ))
        }
        InventoryLocationV1::Equip { slot } => match slot {
            EquipSlotV1::MainHand => {
                if template.weapon_spec.is_none()
                    && crate::lingtian::hoe::HoeKind::from_item_id(&item.template_id).is_none()
                {
                    return Err(format!(
                        "item `{}` cannot equip to main_hand; expected weapon or hoe",
                        item.template_id
                    ));
                }
                if template.weapon_spec.is_some()
                    && inventory.equipped.contains_key(EQUIP_SLOT_TWO_HAND)
                    && !from_two_hand
                {
                    return Err(
                        "cannot equip main_hand while two_hand slot is occupied".to_string()
                    );
                }
                Ok(())
            }
            EquipSlotV1::OffHand => {
                if matches!(template.category, ItemCategory::Treasure) {
                    if inventory.equipped.contains_key(EQUIP_SLOT_TWO_HAND) && !from_two_hand {
                        return Err(
                            "cannot equip off_hand while two_hand slot is occupied".to_string()
                        );
                    }
                    return Ok(());
                }

                let spec = template.weapon_spec.as_ref().ok_or_else(|| {
                    format!(
                        "item `{}` cannot equip to off_hand; expected dagger/fist weapon or treasure",
                        item.template_id
                    )
                })?;
                if !matches!(spec.weapon_kind, WeaponKind::Dagger | WeaponKind::Fist) {
                    return Err(format!(
                        "weapon `{}` cannot equip to off_hand; only dagger/fist are allowed",
                        item.template_id
                    ));
                }
                if inventory.equipped.contains_key(EQUIP_SLOT_TWO_HAND) && !from_two_hand {
                    return Err("cannot equip off_hand while two_hand slot is occupied".to_string());
                }
                Ok(())
            }
            EquipSlotV1::TwoHand => {
                let spec = template.weapon_spec.as_ref().ok_or_else(|| {
                    format!(
                        "item `{}` cannot equip to two_hand; expected spear/staff weapon",
                        item.template_id
                    )
                })?;
                if !matches!(spec.weapon_kind, WeaponKind::Spear | WeaponKind::Staff) {
                    return Err(format!(
                        "weapon `{}` cannot equip to two_hand; only spear/staff are allowed",
                        item.template_id
                    ));
                }
                if inventory.equipped.contains_key(EQUIP_SLOT_MAIN_HAND) && !from_two_hand {
                    return Err(
                        "cannot equip two_hand while main_hand slot is occupied".to_string()
                    );
                }
                if inventory.equipped.contains_key(EQUIP_SLOT_OFF_HAND) && !from_two_hand {
                    return Err("cannot equip two_hand while off_hand slot is occupied".to_string());
                }
                Ok(())
            }
            EquipSlotV1::TreasureBelt0
            | EquipSlotV1::TreasureBelt1
            | EquipSlotV1::TreasureBelt2
            | EquipSlotV1::TreasureBelt3 => {
                if !matches!(template.category, ItemCategory::Treasure) {
                    return Err(format!(
                        "item `{}` cannot equip to {}; expected treasure",
                        item.template_id,
                        equip_slot_key(slot)
                    ));
                }
                Ok(())
            }
            _ => Ok(()),
        },
        _ => Ok(()),
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
        InventoryLocationV1::Equip { slot } => {
            let key = equip_slot_key(slot);
            inventory
                .equipped
                .get(key)
                .map(|item| item.instance_id == instance_id)
                .unwrap_or(false)
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
    for item in inventory.equipped.values() {
        if item.instance_id == instance_id {
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

fn inventory_item_by_instance_mut(
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
    for item in inventory.equipped.values_mut() {
        if item.instance_id == instance_id {
            return Some(item);
        }
    }
    inventory
        .hotbar
        .iter_mut()
        .flatten()
        .find(|item| item.instance_id == instance_id)
}

fn detach_instance(inventory: &mut PlayerInventory, instance_id: u64) {
    for c in &mut inventory.containers {
        c.items.retain(|p| p.instance.instance_id != instance_id);
    }
    inventory
        .equipped
        .retain(|_, item| item.instance_id != instance_id);
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
        InventoryLocationV1::Equip { slot } => {
            let key = equip_slot_key(slot).to_string();
            inventory.equipped.insert(key, item);
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

fn find_first_fit_container_location(
    inventory: &PlayerInventory,
    item: &ItemInstance,
) -> Option<crate::schema::inventory::InventoryLocationV1> {
    use crate::schema::inventory::{ContainerIdV1, InventoryLocationV1};

    let ordered = [
        (MAIN_PACK_CONTAINER_ID, ContainerIdV1::MainPack),
        (SMALL_POUCH_CONTAINER_ID, ContainerIdV1::SmallPouch),
        (FRONT_SATCHEL_CONTAINER_ID, ContainerIdV1::FrontSatchel),
    ];

    for (runtime_id, wire_id) in ordered {
        let Some(container) = inventory.containers.iter().find(|c| c.id == runtime_id) else {
            continue;
        };
        for row in 0..container.rows {
            for col in 0..container.cols {
                let location = InventoryLocationV1::Container {
                    container_id: wire_id.clone(),
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
    use crate::schema::inventory::ContainerIdV1;
    match cid {
        ContainerIdV1::MainPack => MAIN_PACK_CONTAINER_ID,
        ContainerIdV1::SmallPouch => SMALL_POUCH_CONTAINER_ID,
        ContainerIdV1::FrontSatchel => FRONT_SATCHEL_CONTAINER_ID,
    }
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
        EquipSlotV1::TwoHand => EQUIP_SLOT_TWO_HAND,
        EquipSlotV1::TreasureBelt0 => EQUIP_SLOT_TREASURE_BELT_0,
        EquipSlotV1::TreasureBelt1 => EQUIP_SLOT_TREASURE_BELT_1,
        EquipSlotV1::TreasureBelt2 => EQUIP_SLOT_TREASURE_BELT_2,
        EquipSlotV1::TreasureBelt3 => EQUIP_SLOT_TREASURE_BELT_3,
    }
}

fn equip_slot_wire_from_runtime(slot: &str) -> crate::schema::inventory::EquipSlotV1 {
    use crate::schema::inventory::EquipSlotV1;

    match slot {
        EQUIP_SLOT_HEAD => EquipSlotV1::Head,
        EQUIP_SLOT_CHEST => EquipSlotV1::Chest,
        EQUIP_SLOT_LEGS => EquipSlotV1::Legs,
        EQUIP_SLOT_FEET => EquipSlotV1::Feet,
        EQUIP_SLOT_MAIN_HAND => EquipSlotV1::MainHand,
        EQUIP_SLOT_OFF_HAND => EquipSlotV1::OffHand,
        EQUIP_SLOT_TWO_HAND => EquipSlotV1::TwoHand,
        _ => EquipSlotV1::MainHand,
    }
}

fn placed_item_footprints_overlap(left: &PlacedItemState, right: &PlacedItemState) -> bool {
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
    })
}

fn ensure_required_containers_present(
    containers: &[ContainerState],
    source_path: &Path,
) -> Result<(), String> {
    for required in [
        MAIN_PACK_CONTAINER_ID,
        SMALL_POUCH_CONTAINER_ID,
        FRONT_SATCHEL_CONTAINER_ID,
    ] {
        let exists = containers.iter().any(|container| container.id == required);
        if !exists {
            return Err(format!(
                "{} loadout missing required container id `{required}`",
                source_path.display()
            ));
        }
    }
    Ok(())
}

fn validate_container_id(id: &str, source_path: &Path) -> Result<(), String> {
    let is_allowed = [
        MAIN_PACK_CONTAINER_ID,
        SMALL_POUCH_CONTAINER_ID,
        FRONT_SATCHEL_CONTAINER_ID,
    ]
    .contains(&id);

    if is_allowed {
        Ok(())
    } else {
        Err(format!(
            "{} has unsupported container id `{id}`; expected one of [{}, {}, {}]",
            source_path.display(),
            MAIN_PACK_CONTAINER_ID,
            SMALL_POUCH_CONTAINER_ID,
            FRONT_SATCHEL_CONTAINER_ID
        ))
    }
}

fn validate_equip_slot(slot: &str, source_path: &Path) -> Result<(), String> {
    let is_allowed = [
        EQUIP_SLOT_HEAD,
        EQUIP_SLOT_CHEST,
        EQUIP_SLOT_LEGS,
        EQUIP_SLOT_FEET,
        EQUIP_SLOT_MAIN_HAND,
        EQUIP_SLOT_OFF_HAND,
        EQUIP_SLOT_TWO_HAND,
        EQUIP_SLOT_TREASURE_BELT_0,
        EQUIP_SLOT_TREASURE_BELT_1,
        EQUIP_SLOT_TREASURE_BELT_2,
        EQUIP_SLOT_TREASURE_BELT_3,
    ]
    .contains(&slot);

    if is_allowed {
        Ok(())
    } else {
        Err(format!(
            "{} has unsupported equip slot `{slot}`; expected one of [{}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}]",
            source_path.display(),
            EQUIP_SLOT_HEAD,
            EQUIP_SLOT_CHEST,
            EQUIP_SLOT_LEGS,
            EQUIP_SLOT_FEET,
            EQUIP_SLOT_MAIN_HAND,
            EQUIP_SLOT_OFF_HAND,
            EQUIP_SLOT_TWO_HAND,
            EQUIP_SLOT_TREASURE_BELT_0,
            EQUIP_SLOT_TREASURE_BELT_1,
            EQUIP_SLOT_TREASURE_BELT_2,
            EQUIP_SLOT_TREASURE_BELT_3
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

    fn test_registry_from_strs(entries: &[(&str, &str)]) -> Result<ItemRegistry, String> {
        let mut templates = HashMap::new();
        for (template_id, display_name) in entries {
            templates.insert(
                (*template_id).to_string(),
                ItemTemplate {
                    id: (*template_id).to_string(),
                    display_name: (*display_name).to_string(),
                    category: ItemCategory::Misc,
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
                },
            );
        }
        Ok(ItemRegistry { templates })
    }

    #[test]
    fn loads_item_registry_from_assets() {
        let registry =
            load_item_registry().expect("item registry should load from assets/items/*.toml");
        assert!(registry.len() >= 1);
        assert!(registry.get("starter_talisman").is_some());
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

        let player_inventory = instantiate_inventory_from_loadout(&loadout, &mut allocator)
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
            .chain(player_inventory.equipped.values())
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
    fn runtime_grant_increments_revision_and_creates_instance() {
        let registry = load_item_registry().expect("item registry should load");
        let loadout = load_default_loadout(&registry).expect("default loadout should load");
        let mut allocator = InventoryInstanceIdAllocator::new(1);
        let mut inventory = instantiate_inventory_from_loadout(&loadout, &mut allocator)
            .expect("inventory should instantiate from loadout");

        let baseline_revision = inventory.revision;
        let receipt = add_item_to_player_inventory(
            &mut inventory,
            &registry,
            &mut allocator,
            "ci_she_hao",
            2,
        )
        .expect("runtime inventory grant should succeed for canonical herb");

        assert_eq!(receipt.template_id, "ci_she_hao");
        assert_eq!(receipt.stack_count, 2);
        assert!(receipt.instance_id >= 1);
        assert_eq!(inventory.revision.0, baseline_revision.0.saturating_add(1));

        let main_pack = inventory
            .containers
            .iter()
            .find(|container| container.id == MAIN_PACK_CONTAINER_ID)
            .expect("main pack should exist");
        assert!(
            main_pack
                .items
                .iter()
                .any(|entry| entry.instance.template_id == "ci_she_hao"),
            "runtime grant should materialize in main pack"
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
        };
        PlayerInventory {
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
                },
                ContainerState {
                    id: SMALL_POUCH_CONTAINER_ID.to_string(),
                    name: "小口袋".to_string(),
                    rows: 3,
                    cols: 3,
                    items: Vec::new(),
                },
                ContainerState {
                    id: FRONT_SATCHEL_CONTAINER_ID.to_string(),
                    name: "前挂包".to_string(),
                    rows: 3,
                    cols: 4,
                    items: Vec::new(),
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
        use crate::schema::inventory::{ContainerIdV1, InventoryLocationV1};
        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        let outcome = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: ContainerIdV1::MainPack,
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
        use crate::schema::inventory::{ContainerIdV1, InventoryLocationV1};
        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        let result = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            // Wrong from cell.
            &InventoryLocationV1::Container {
                container_id: ContainerIdV1::MainPack,
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
        });

        let outcome = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: crate::schema::inventory::ContainerIdV1::MainPack,
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
        use crate::schema::inventory::{ContainerIdV1, InventoryLocationV1};
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
            },
        });

        // Try to drop 1×1 (#42) onto the 2×2 anchor — overlap, mismatched footprint → reject.
        let result = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: ContainerIdV1::MainPack,
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Container {
                container_id: ContainerIdV1::MainPack,
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
        use crate::schema::inventory::{ContainerIdV1, InventoryLocationV1};
        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        let _ = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: ContainerIdV1::MainPack,
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Container {
                container_id: ContainerIdV1::MainPack,
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
        use crate::schema::inventory::{ContainerIdV1, EquipSlotV1, InventoryLocationV1};

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
                container_id: ContainerIdV1::MainPack,
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Equip {
                slot: EquipSlotV1::MainHand,
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
                .map(|item| item.template_id.as_str()),
            Some("iron_sword")
        );
    }

    #[test]
    fn apply_move_rejects_weapon_to_hotbar() {
        use crate::schema::inventory::{ContainerIdV1, InventoryLocationV1};

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
                container_id: ContainerIdV1::MainPack,
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Hotbar { index: 0 },
        )
        .expect_err("weapon should be rejected from hotbar");

        assert!(error.contains("cannot move to hotbar"));
    }

    #[test]
    fn apply_move_rejects_non_dagger_off_hand_weapon() {
        use crate::schema::inventory::{ContainerIdV1, EquipSlotV1, InventoryLocationV1};

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
                container_id: ContainerIdV1::MainPack,
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Equip {
                slot: EquipSlotV1::OffHand,
            },
        )
        .expect_err("sword should be rejected from off_hand");

        assert!(error.contains("only dagger/fist are allowed"));
    }

    #[test]
    fn apply_move_rejects_two_hand_when_main_hand_occupied() {
        use crate::schema::inventory::{ContainerIdV1, EquipSlotV1, InventoryLocationV1};

        let registry = load_item_registry().expect("item registry should load");
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items[0].instance.template_id = "wooden_staff".to_string();
        inv.containers[0].items[0].instance.display_name = "木杖".to_string();
        inv.containers[0].items[0].instance.grid_h = 3;
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            ItemInstance {
                instance_id: 77,
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
            },
        );

        let error = apply_inventory_move(
            &mut inv,
            &registry,
            42,
            &InventoryLocationV1::Container {
                container_id: ContainerIdV1::MainPack,
                row: 0,
                col: 0,
            },
            &InventoryLocationV1::Equip {
                slot: EquipSlotV1::TwoHand,
            },
        )
        .expect_err("two_hand should conflict with occupied main_hand");

        assert!(error.contains("main_hand slot is occupied"));
    }

    #[test]
    fn set_item_instance_durability_updates_equipped_item_and_bumps_revision() {
        let mut inv = make_test_inventory_with_one_item();
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            ItemInstance {
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
            },
        );

        let update = set_item_instance_durability(&mut inv, 88, 0.25)
            .expect("durability update should succeed");

        assert_eq!(update.revision, InventoryRevision(8));
        assert_eq!(inv.equipped[EQUIP_SLOT_MAIN_HAND].durability, 0.25);
    }

    #[test]
    fn move_equipped_item_to_first_container_slot_unequips_and_rehomes_item() {
        let mut inv = make_test_inventory_with_one_item();
        inv.containers[0].items.clear();
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            ItemInstance {
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
            },
        );

        let outcome = move_equipped_item_to_first_container_slot(&mut inv, 88)
            .expect("broken weapon should move back to container");

        assert_eq!(
            outcome,
            InventoryMoveOutcome::Moved {
                revision: InventoryRevision(8)
            }
        );
        assert!(!inv.equipped.contains_key(EQUIP_SLOT_MAIN_HAND));
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
        });
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            ItemInstance {
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
            },
        );

        let out = apply_death_drop_to_inventory(&mut inv, &ItemRegistry::default(), 777);

        assert_eq!(out.dropped.len(), 2);
        assert_eq!(out.revision, InventoryRevision(8));
        let remaining_count = inv.containers[0].items.len()
            + inv.hotbar.iter().flatten().count()
            + inv.equipped.len();
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
            42,
            &crate::schema::inventory::InventoryLocationV1::Container {
                container_id: crate::schema::inventory::ContainerIdV1::MainPack,
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
            },
        );
        let mut inv = make_test_inventory_with_one_item();
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            ItemInstance {
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
            },
        );

        let out = apply_death_drop_to_inventory(&mut inv, &registry, 42);

        assert!(out.dropped.iter().all(|d| d.instance.instance_id != 9001));
        assert_eq!(
            inv.equipped
                .get(EQUIP_SLOT_MAIN_HAND)
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
            },
        );
        let mut inv = make_test_inventory_with_one_item();
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            ItemInstance {
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
            },
        );

        let out = apply_death_drop_to_inventory(&mut inv, &registry, 42);

        assert!(out.dropped.iter().any(|d| d.instance.instance_id == 9002));
        assert!(!inv.equipped.contains_key(EQUIP_SLOT_MAIN_HAND));
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
        });
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            ItemInstance {
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
            },
        );

        let current = calculate_current_weight(&inv);

        assert!((current - 5.5).abs() < 1e-9);
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
        }
    }

    fn make_empty_inventory() -> PlayerInventory {
        PlayerInventory {
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
        });
        let got = inventory_item_by_instance_borrow(&inv, 42);
        assert!(got.is_some());
        assert_eq!(got.unwrap().template_id, "iron_sword");
    }

    #[test]
    fn borrow_helper_finds_item_in_equipped_and_hotbar() {
        let mut inv = make_empty_inventory();
        inv.equipped.insert(
            "main_hand".to_string(),
            make_test_item_instance(7, "talisman"),
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
    fn borrow_helper_returns_none_for_missing_instance() {
        let inv = make_empty_inventory();
        assert!(inventory_item_by_instance_borrow(&inv, 99).is_none());
    }
}
