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
    fn loads_default_loadout_with_starter_talisman() {
        let registry = load_item_registry().expect("item registry should load");
        let loadout = load_default_loadout(&registry).expect("default loadout should load");

        let contains_starter_talisman = loadout
            .containers
            .iter()
            .flat_map(|container| container.items.iter())
            .any(|item| item.instance.template_id == "starter_talisman")
            || loadout
                .equipped
                .values()
                .any(|item| item.template_id == "starter_talisman")
            || loadout
                .hotbar
                .iter()
                .flatten()
                .any(|item| item.template_id == "starter_talisman");

        assert!(contains_starter_talisman);
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
}
