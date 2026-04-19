use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use valence::prelude::{
    bevy_ecs, Added, App, Client, Commands, Component, Entity, Query, Resource, Without,
};

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

type JoinedClientsWithoutInventoryFilter = (Added<Client>, Without<PlayerInventory>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemCategory {
    Pill,
    Herb,
    Weapon,
    BoneCoin,
    Misc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
pub struct LoadoutSpec {
    pub containers: Vec<ContainerState>,
    pub equipped: HashMap<String, ItemInstance>,
    pub hotbar: [Option<ItemInstance>; 9],
    pub bone_coins: u64,
    pub max_weight: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContainerState {
    pub id: String,
    pub name: String,
    pub rows: u8,
    pub cols: u8,
    pub items: Vec<PlacedItemState>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlacedItemState {
    pub row: u8,
    pub col: u8,
    pub instance: ItemInstance,
}

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, Component)]
pub struct PlayerInventory {
    pub revision: InventoryRevision,
    pub containers: Vec<ContainerState>,
    pub equipped: HashMap<String, ItemInstance>,
    pub hotbar: [Option<ItemInstance>; 9],
    pub bone_coins: u64,
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
        // plan-HUD-v1 §3.4 默认 stance=None，伪皮 0，涡流未激活。switch 后才出现指示器。
        commands
            .entity(entity)
            .insert(crate::combat::components::DefenseStance::default());
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
        })
    }
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

fn bump_revision(inventory: &mut PlayerInventory) {
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
    ]
    .contains(&slot);

    if is_allowed {
        Ok(())
    } else {
        Err(format!(
            "{} has unsupported equip slot `{slot}`; expected one of [{}, {}, {}, {}, {}, {}, {}]",
            source_path.display(),
            EQUIP_SLOT_HEAD,
            EQUIP_SLOT_CHEST,
            EQUIP_SLOT_LEGS,
            EQUIP_SLOT_FEET,
            EQUIP_SLOT_MAIN_HAND,
            EQUIP_SLOT_OFF_HAND,
            EQUIP_SLOT_TWO_HAND
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
        let mut inv = make_test_inventory_with_one_item();
        let outcome = apply_inventory_move(
            &mut inv,
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
        let mut inv = make_test_inventory_with_one_item();
        let result = apply_inventory_move(
            &mut inv,
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
        });

        let outcome = apply_inventory_move(
            &mut inv,
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
            },
        });

        // Try to drop 1×1 (#42) onto the 2×2 anchor — overlap, mismatched footprint → reject.
        let result = apply_inventory_move(
            &mut inv,
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
        let mut inv = make_test_inventory_with_one_item();
        let _ = apply_inventory_move(
            &mut inv,
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
}
