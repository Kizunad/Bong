//! plan-tuike-v1 — 替尸 / 蜕壳流伪皮。
//!
//! 伪皮是独立于真甲的内层装备：只过滤 contam，不改 wound 结算。

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Changed, Commands, Component, Entity, Event, EventReader, Query, Res, ResMut,
};

use crate::combat::components::DerivedAttrs;
use crate::cultivation::components::{ColorKind, Cultivation, Realm};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::inventory::{
    add_item_to_player_inventory, consume_item_instance_once, InventoryInstanceIdAllocator,
    ItemRegistry, PlayerInventory, EQUIP_SLOT_FALSE_SKIN,
};
use crate::qi_physics::StyleDefense;
use crate::schema::tuike::{FalseSkinKindV1, FalseSkinStateV1, ShedEventV1};

pub const SPIDER_SILK_MATERIAL_ID: &str = "ash_spider_silk";
pub const ROTTEN_WOOD_MATERIAL_ID: &str = "tuike_rotten_wood";
pub const SPIDER_SILK_FALSE_SKIN_ITEM_ID: &str = "tuike_false_skin_silk";
pub const ROTTEN_WOOD_ARMOR_ITEM_ID: &str = "tuike_rotten_wood_armor";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FalseSkinKind {
    SpiderSilk,
    RottenWoodArmor,
}

impl FalseSkinKind {
    pub const fn layers(self) -> u8 {
        match self {
            Self::SpiderSilk => 1,
            Self::RottenWoodArmor => 3,
        }
    }

    pub const fn contam_capacity_per_layer(self) -> f64 {
        match self {
            Self::SpiderSilk => 10.0,
            Self::RottenWoodArmor => 30.0,
        }
    }

    pub const fn qi_cost(self) -> f64 {
        match self {
            Self::SpiderSilk => 5.0,
            Self::RottenWoodArmor => 30.0,
        }
    }

    pub const fn min_realm(self) -> Realm {
        match self {
            Self::SpiderSilk => Realm::Induce,
            Self::RottenWoodArmor => Realm::Condense,
        }
    }

    pub const fn output_item_id(self) -> &'static str {
        match self {
            Self::SpiderSilk => SPIDER_SILK_FALSE_SKIN_ITEM_ID,
            Self::RottenWoodArmor => ROTTEN_WOOD_ARMOR_ITEM_ID,
        }
    }

    pub const fn jiemai_window_modifier(self) -> f32 {
        match self {
            Self::SpiderSilk => 1.0,
            Self::RottenWoodArmor => 0.6,
        }
    }
}

impl StyleDefense for FalseSkinKind {
    fn defense_color(&self) -> ColorKind {
        match self {
            Self::SpiderSilk => ColorKind::Gentle,
            Self::RottenWoodArmor => ColorKind::Solid,
        }
    }

    fn resistance(&self) -> f64 {
        (self.contam_capacity_per_layer() / 30.0).clamp(0.0, 1.0)
    }

    fn drain_affinity(&self) -> f64 {
        match self {
            Self::SpiderSilk => 0.15,
            Self::RottenWoodArmor => 0.05,
        }
    }
}

impl From<FalseSkinKind> for FalseSkinKindV1 {
    fn from(value: FalseSkinKind) -> Self {
        match value {
            FalseSkinKind::SpiderSilk => Self::SpiderSilk,
            FalseSkinKind::RottenWoodArmor => Self::RottenWoodArmor,
        }
    }
}

impl From<FalseSkinKindV1> for FalseSkinKind {
    fn from(value: FalseSkinKindV1) -> Self {
        match value {
            FalseSkinKindV1::SpiderSilk => Self::SpiderSilk,
            FalseSkinKindV1::RottenWoodArmor => Self::RottenWoodArmor,
        }
    }
}

pub fn false_skin_kind_for_item(template_id: &str) -> Option<FalseSkinKind> {
    match template_id {
        SPIDER_SILK_FALSE_SKIN_ITEM_ID => Some(FalseSkinKind::SpiderSilk),
        ROTTEN_WOOD_ARMOR_ITEM_ID => Some(FalseSkinKind::RottenWoodArmor),
        _ => None,
    }
}

#[derive(Debug, Clone, Component, PartialEq, Serialize, Deserialize)]
pub struct FalseSkin {
    pub instance_id: u64,
    pub kind: FalseSkinKind,
    pub layers_remaining: u8,
    pub contam_capacity_per_layer: f64,
    pub absorbed_contam: f64,
    pub equipped_at_tick: u64,
}

impl FalseSkin {
    pub fn fresh(instance_id: u64, kind: FalseSkinKind, equipped_at_tick: u64) -> Self {
        Self {
            instance_id,
            kind,
            layers_remaining: kind.layers(),
            contam_capacity_per_layer: kind.contam_capacity_per_layer(),
            absorbed_contam: 0.0,
            equipped_at_tick,
        }
    }

    pub fn state_payload(&self, target_id: String) -> FalseSkinStateV1 {
        FalseSkinStateV1 {
            target_id,
            kind: Some(self.kind.into()),
            layers_remaining: self.layers_remaining,
            contam_capacity_per_layer: self.contam_capacity_per_layer,
            absorbed_contam: self.absorbed_contam,
            equipped_at_tick: self.equipped_at_tick,
            layers: Vec::new(),
        }
    }
}

pub fn empty_false_skin_state(target_id: String) -> FalseSkinStateV1 {
    FalseSkinStateV1 {
        target_id,
        kind: None,
        layers_remaining: 0,
        contam_capacity_per_layer: 0.0,
        absorbed_contam: 0.0,
        equipped_at_tick: 0,
        layers: Vec::new(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContamFilterResult {
    pub passes_through: f64,
    pub shed_layers: u8,
    pub contam_absorbed: f64,
    pub depleted: bool,
}

pub fn tuike_filter_contam(
    incoming_contam: f64,
    skin: Option<&mut FalseSkin>,
) -> ContamFilterResult {
    let incoming = if incoming_contam.is_finite() {
        incoming_contam.max(0.0)
    } else {
        0.0
    };
    let Some(skin) = skin else {
        return ContamFilterResult {
            passes_through: incoming,
            shed_layers: 0,
            contam_absorbed: 0.0,
            depleted: false,
        };
    };

    if incoming <= 0.0 || skin.layers_remaining == 0 {
        return ContamFilterResult {
            passes_through: incoming,
            shed_layers: 0,
            contam_absorbed: 0.0,
            depleted: skin.layers_remaining == 0,
        };
    }

    let mut remaining = incoming;
    let mut shed_layers = 0_u8;
    let mut absorbed = 0.0;

    while remaining > 0.0 && skin.layers_remaining > 0 {
        let cap = skin.contam_capacity_per_layer.max(0.0);
        if cap <= 0.0 {
            break;
        }
        let space = (cap - skin.absorbed_contam).max(0.0);
        if remaining + f64::EPSILON >= space {
            skin.absorbed_contam = 0.0;
            skin.layers_remaining = skin.layers_remaining.saturating_sub(1);
            shed_layers = shed_layers.saturating_add(1);
            absorbed += space;
            remaining = (remaining - space).max(0.0);
        } else {
            skin.absorbed_contam += remaining;
            absorbed += remaining;
            remaining = 0.0;
        }
    }

    ContamFilterResult {
        passes_through: remaining,
        shed_layers,
        contam_absorbed: absorbed,
        depleted: skin.layers_remaining == 0,
    }
}

#[derive(Debug, Clone, Event, PartialEq, Serialize, Deserialize)]
pub struct ShedEvent {
    pub target: Entity,
    pub attacker: Option<Entity>,
    pub target_id: String,
    pub attacker_id: Option<String>,
    pub kind: FalseSkinKind,
    pub layers_shed: u8,
    pub layers_remaining: u8,
    pub contam_absorbed: f64,
    pub contam_overflow: f64,
    pub tick: u64,
}

impl ShedEvent {
    pub fn payload(&self) -> ShedEventV1 {
        ShedEventV1 {
            target_id: self.target_id.clone(),
            attacker_id: self.attacker_id.clone(),
            kind: self.kind.into(),
            layers_shed: self.layers_shed,
            layers_remaining: self.layers_remaining,
            contam_absorbed: self.contam_absorbed,
            contam_overflow: self.contam_overflow,
            tick: self.tick,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FalseSkinForgeError {
    RealmTooLow,
    NotEnoughQi,
    MissingMaterial(&'static str),
    InventoryFull,
    MissingAllocator,
}

impl std::fmt::Display for FalseSkinForgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RealmTooLow => write!(f, "realm too low for false skin forging"),
            Self::NotEnoughQi => write!(f, "not enough qi for false skin forging"),
            Self::MissingMaterial(id) => write!(f, "missing false skin material `{id}`"),
            Self::InventoryFull => write!(f, "inventory full for false skin output"),
            Self::MissingAllocator => write!(f, "inventory instance allocator unavailable"),
        }
    }
}

pub struct FalseSkinRecipe {
    pub kind: FalseSkinKind,
    pub materials: &'static [(&'static str, u32)],
}

pub const RECIPE_SPIDER_SILK_FALSE_SKIN: FalseSkinRecipe = FalseSkinRecipe {
    kind: FalseSkinKind::SpiderSilk,
    materials: &[(SPIDER_SILK_MATERIAL_ID, 1)],
};

pub const RECIPE_ROTTEN_WOOD_ARMOR: FalseSkinRecipe = FalseSkinRecipe {
    kind: FalseSkinKind::RottenWoodArmor,
    materials: &[(ROTTEN_WOOD_MATERIAL_ID, 1), (SPIDER_SILK_MATERIAL_ID, 2)],
};

pub fn recipe_for_kind(kind: FalseSkinKind) -> &'static FalseSkinRecipe {
    match kind {
        FalseSkinKind::SpiderSilk => &RECIPE_SPIDER_SILK_FALSE_SKIN,
        FalseSkinKind::RottenWoodArmor => &RECIPE_ROTTEN_WOOD_ARMOR,
    }
}

pub fn forge_false_skin(
    recipe: &FalseSkinRecipe,
    cultivation: &mut Cultivation,
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    allocator: Option<&mut InventoryInstanceIdAllocator>,
) -> Result<u64, FalseSkinForgeError> {
    if !can_equip_false_skin(cultivation.realm, recipe.kind) {
        return Err(FalseSkinForgeError::RealmTooLow);
    }
    if cultivation.qi_current + f64::EPSILON < recipe.kind.qi_cost() {
        return Err(FalseSkinForgeError::NotEnoughQi);
    }
    for (template_id, count) in recipe.materials {
        if count_template(inventory, template_id) < *count {
            return Err(FalseSkinForgeError::MissingMaterial(template_id));
        }
    }

    let Some(allocator) = allocator else {
        return Err(FalseSkinForgeError::MissingAllocator);
    };

    let mut staged_inventory = inventory.clone();
    let mut staged_allocator = allocator.clone();
    for (template_id, count) in recipe.materials {
        consume_template_count(&mut staged_inventory, template_id, *count);
    }

    let receipt = add_item_to_player_inventory(
        &mut staged_inventory,
        registry,
        &mut staged_allocator,
        recipe.kind.output_item_id(),
        1,
    )
    .map_err(|_| FalseSkinForgeError::InventoryFull)?;

    *inventory = staged_inventory;
    *allocator = staged_allocator;
    cultivation.qi_current =
        (cultivation.qi_current - recipe.kind.qi_cost()).clamp(0.0, cultivation.qi_max);

    Ok(receipt.instance_id)
}

pub fn can_equip_false_skin(realm: Realm, kind: FalseSkinKind) -> bool {
    realm_rank(realm) >= realm_rank(kind.min_realm())
}

fn realm_rank(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

fn count_template(inventory: &PlayerInventory, template_id: &str) -> u32 {
    let container_count = inventory
        .containers
        .iter()
        .flat_map(|container| container.items.iter())
        .filter(|placed| placed.instance.template_id == template_id)
        .map(|placed| placed.instance.stack_count)
        .sum::<u32>();
    let hotbar_count = inventory
        .hotbar
        .iter()
        .flatten()
        .filter(|item| item.template_id == template_id)
        .map(|item| item.stack_count)
        .sum::<u32>();
    let equipped_count = inventory
        .equipped
        .values()
        .filter(|item| item.template_id == template_id)
        .map(|item| item.stack_count)
        .sum::<u32>();
    container_count
        .saturating_add(hotbar_count)
        .saturating_add(equipped_count)
}

fn consume_template_count(inventory: &mut PlayerInventory, template_id: &str, mut count: u32) {
    while count > 0 {
        let Some(instance_id) = first_instance_id_for_template(inventory, template_id) else {
            break;
        };
        let _ = consume_item_instance_once(inventory, instance_id);
        count -= 1;
    }
}

fn first_instance_id_for_template(inventory: &PlayerInventory, template_id: &str) -> Option<u64> {
    inventory
        .containers
        .iter()
        .flat_map(|container| container.items.iter())
        .find(|placed| placed.instance.template_id == template_id)
        .map(|placed| placed.instance.instance_id)
        .or_else(|| {
            inventory
                .hotbar
                .iter()
                .flatten()
                .find(|item| item.template_id == template_id)
                .map(|item| item.instance_id)
        })
        .or_else(|| {
            inventory
                .equipped
                .values()
                .find(|item| item.template_id == template_id)
                .map(|item| item.instance_id)
        })
}

#[derive(Debug, Clone, Event, PartialEq, Eq)]
pub struct FalseSkinForgeRequest {
    pub crafter: Entity,
    pub kind: FalseSkinKind,
}

pub fn handle_false_skin_forge_requests(
    mut requests: EventReader<FalseSkinForgeRequest>,
    mut inventories: Query<&mut PlayerInventory>,
    mut cultivations: Query<&mut Cultivation>,
    registry: Res<ItemRegistry>,
    mut allocator: Option<ResMut<InventoryInstanceIdAllocator>>,
) {
    for request in requests.read() {
        let Ok(mut inventory) = inventories.get_mut(request.crafter) else {
            tracing::warn!(
                "[bong][tuike] false skin forge rejected: {:?} has no PlayerInventory",
                request.crafter
            );
            continue;
        };
        let Ok(mut cultivation) = cultivations.get_mut(request.crafter) else {
            tracing::warn!(
                "[bong][tuike] false skin forge rejected: {:?} has no Cultivation",
                request.crafter
            );
            continue;
        };

        let result = forge_false_skin(
            recipe_for_kind(request.kind),
            &mut cultivation,
            &mut inventory,
            &registry,
            allocator.as_deref_mut(),
        );
        match result {
            Ok(instance_id) => tracing::info!(
                "[bong][tuike] forged {:?} for {:?} as item instance {}",
                request.kind,
                request.crafter,
                instance_id
            ),
            Err(error) => tracing::warn!(
                "[bong][tuike] false skin forge rejected for {:?}: {}",
                request.crafter,
                error
            ),
        }
    }
}

pub fn record_shed_events_in_life_record(
    mut events: EventReader<ShedEvent>,
    mut life_records: Query<&mut LifeRecord>,
) {
    for event in events.read() {
        let Ok(mut life_record) = life_records.get_mut(event.target) else {
            continue;
        };
        life_record.push(BiographyEntry::FalseSkinShed {
            kind: format!("{:?}", event.kind),
            layers_shed: event.layers_shed,
            contam_absorbed: event.contam_absorbed,
            contam_overflow: event.contam_overflow,
            attacker_id: event.attacker_id.clone(),
            tick: event.tick,
        });
    }
}

pub fn sync_false_skin_from_inventory(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &PlayerInventory,
            Option<&mut FalseSkin>,
            &mut DerivedAttrs,
        ),
        Changed<PlayerInventory>,
    >,
) {
    for (entity, inventory, skin, mut attrs) in &mut query {
        let equipped = inventory.equipped.get(EQUIP_SLOT_FALSE_SKIN);
        let next = equipped.and_then(|item| {
            false_skin_kind_for_item(item.template_id.as_str()).map(|kind| (item.instance_id, kind))
        });

        match (next, skin) {
            (Some((instance_id, _kind)), Some(skin)) if skin.instance_id == instance_id => {
                attrs.tuike_layers = skin.layers_remaining;
            }
            (Some((instance_id, kind)), _) => {
                attrs.tuike_layers = kind.layers();
                commands
                    .entity(entity)
                    .insert(FalseSkin::fresh(instance_id, kind, 0));
            }
            (None, Some(_)) => {
                attrs.tuike_layers = 0;
                commands.entity(entity).remove::<FalseSkin>();
            }
            (None, None) => {
                attrs.tuike_layers = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Cultivation;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemTemplate,
        PlayerInventory,
    };
    use std::collections::HashMap;
    use valence::prelude::{App, Update};

    fn skin(kind: FalseSkinKind) -> FalseSkin {
        FalseSkin::fresh(7, kind, 11)
    }

    #[test]
    fn filter_accumulates_without_shedding_under_capacity() {
        let mut skin = skin(FalseSkinKind::SpiderSilk);
        let result = tuike_filter_contam(4.5, Some(&mut skin));

        assert_eq!(result.passes_through, 0.0);
        assert_eq!(result.shed_layers, 0);
        assert_eq!(skin.layers_remaining, 1);
        assert!((skin.absorbed_contam - 4.5).abs() < f64::EPSILON);
    }

    #[test]
    fn filter_sheds_single_layer_and_returns_overflow() {
        let mut skin = skin(FalseSkinKind::SpiderSilk);
        skin.absorbed_contam = 8.0;

        let result = tuike_filter_contam(5.0, Some(&mut skin));

        assert_eq!(result.shed_layers, 1);
        assert!(result.depleted);
        assert_eq!(skin.layers_remaining, 0);
        assert!((result.contam_absorbed - 2.0).abs() < f64::EPSILON);
        assert!((result.passes_through - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn filter_merges_multi_layer_shed_into_one_result() {
        let mut skin = skin(FalseSkinKind::RottenWoodArmor);

        let result = tuike_filter_contam(95.0, Some(&mut skin));

        assert_eq!(result.shed_layers, 3);
        assert!(result.depleted);
        assert_eq!(skin.layers_remaining, 0);
        assert!((result.contam_absorbed - 90.0).abs() < f64::EPSILON);
        assert!((result.passes_through - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn filter_without_skin_passes_all_contam() {
        let result = tuike_filter_contam(12.0, None);

        assert_eq!(result.shed_layers, 0);
        assert_eq!(result.passes_through, 12.0);
    }

    fn template(id: &str) -> ItemTemplate {
        template_with_size(id, 1, 1)
    }

    fn template_with_size(id: &str, grid_w: u8, grid_h: u8) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: id.to_string(),
            category: ItemCategory::Misc,
            max_stack_count: 16,
            grid_w,
            grid_h,
            base_weight: 0.1,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 1.0,
            description: String::new(),
            effect: None,
            cast_duration_ms: 0,
            cooldown_ms: 0,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            container_spec: None,
        }
    }

    fn item(instance_id: u64, template_id: &str, stack_count: u32) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count,
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

    fn inventory_with_materials() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "Main".to_string(),
                rows: 4,
                cols: 9,
                items: vec![
                    crate::inventory::PlacedItemState {
                        row: 0,
                        col: 0,
                        instance: item(1, SPIDER_SILK_MATERIAL_ID, 3),
                    },
                    crate::inventory::PlacedItemState {
                        row: 0,
                        col: 1,
                        instance: item(2, ROTTEN_WOOD_MATERIAL_ID, 1),
                    },
                ],
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    #[test]
    fn forge_false_skin_consumes_materials_and_qi() {
        let mut inventory = inventory_with_materials();
        let registry = ItemRegistry::from_map(HashMap::from([
            (
                SPIDER_SILK_MATERIAL_ID.to_string(),
                template(SPIDER_SILK_MATERIAL_ID),
            ),
            (
                ROTTEN_WOOD_MATERIAL_ID.to_string(),
                template(ROTTEN_WOOD_MATERIAL_ID),
            ),
            (
                SPIDER_SILK_FALSE_SKIN_ITEM_ID.to_string(),
                template(SPIDER_SILK_FALSE_SKIN_ITEM_ID),
            ),
        ]));
        let mut cultivation = Cultivation {
            realm: Realm::Induce,
            qi_current: 20.0,
            qi_max: 20.0,
            ..Cultivation::default()
        };
        let mut allocator = InventoryInstanceIdAllocator::new(100);

        let output = forge_false_skin(
            recipe_for_kind(FalseSkinKind::SpiderSilk),
            &mut cultivation,
            &mut inventory,
            &registry,
            Some(&mut allocator),
        )
        .expect("forge should succeed");

        assert_eq!(output, 100);
        assert_eq!(cultivation.qi_current, 15.0);
        assert_eq!(count_template(&inventory, SPIDER_SILK_MATERIAL_ID), 2);
        assert_eq!(
            count_template(&inventory, SPIDER_SILK_FALSE_SKIN_ITEM_ID),
            1
        );
    }

    #[test]
    fn forge_false_skin_missing_allocator_keeps_inputs() {
        let mut inventory = inventory_with_materials();
        let registry = ItemRegistry::from_map(HashMap::from([
            (
                SPIDER_SILK_MATERIAL_ID.to_string(),
                template(SPIDER_SILK_MATERIAL_ID),
            ),
            (
                SPIDER_SILK_FALSE_SKIN_ITEM_ID.to_string(),
                template(SPIDER_SILK_FALSE_SKIN_ITEM_ID),
            ),
        ]));
        let mut cultivation = Cultivation {
            realm: Realm::Induce,
            qi_current: 20.0,
            qi_max: 20.0,
            ..Cultivation::default()
        };

        let error = forge_false_skin(
            recipe_for_kind(FalseSkinKind::SpiderSilk),
            &mut cultivation,
            &mut inventory,
            &registry,
            None,
        )
        .unwrap_err();

        assert_eq!(error, FalseSkinForgeError::MissingAllocator);
        assert_eq!(cultivation.qi_current, 20.0);
        assert_eq!(count_template(&inventory, SPIDER_SILK_MATERIAL_ID), 3);
        assert_eq!(
            count_template(&inventory, SPIDER_SILK_FALSE_SKIN_ITEM_ID),
            0
        );
    }

    #[test]
    fn forge_false_skin_inventory_full_keeps_inputs() {
        let mut inventory = inventory_with_materials();
        inventory.containers[0].rows = 1;
        inventory.containers[0].cols = 1;
        inventory.containers[0].items.truncate(1);
        let registry = ItemRegistry::from_map(HashMap::from([
            (
                SPIDER_SILK_MATERIAL_ID.to_string(),
                template(SPIDER_SILK_MATERIAL_ID),
            ),
            (
                SPIDER_SILK_FALSE_SKIN_ITEM_ID.to_string(),
                template_with_size(SPIDER_SILK_FALSE_SKIN_ITEM_ID, 2, 2),
            ),
        ]));
        let mut cultivation = Cultivation {
            realm: Realm::Induce,
            qi_current: 20.0,
            qi_max: 20.0,
            ..Cultivation::default()
        };
        let mut allocator = InventoryInstanceIdAllocator::new(100);

        let error = forge_false_skin(
            recipe_for_kind(FalseSkinKind::SpiderSilk),
            &mut cultivation,
            &mut inventory,
            &registry,
            Some(&mut allocator),
        )
        .unwrap_err();

        assert_eq!(error, FalseSkinForgeError::InventoryFull);
        assert_eq!(cultivation.qi_current, 20.0);
        assert_eq!(count_template(&inventory, SPIDER_SILK_MATERIAL_ID), 3);
        assert_eq!(
            count_template(&inventory, SPIDER_SILK_FALSE_SKIN_ITEM_ID),
            0
        );
    }

    #[test]
    fn forge_request_system_adds_output_and_spends_inputs() {
        let mut app = App::new();
        app.add_event::<FalseSkinForgeRequest>();
        app.insert_resource(ItemRegistry::from_map(HashMap::from([
            (
                SPIDER_SILK_MATERIAL_ID.to_string(),
                template(SPIDER_SILK_MATERIAL_ID),
            ),
            (
                SPIDER_SILK_FALSE_SKIN_ITEM_ID.to_string(),
                template(SPIDER_SILK_FALSE_SKIN_ITEM_ID),
            ),
        ])));
        app.insert_resource(InventoryInstanceIdAllocator::new(200));
        app.add_systems(Update, handle_false_skin_forge_requests);
        let entity = app
            .world_mut()
            .spawn((
                inventory_with_materials(),
                Cultivation {
                    realm: Realm::Induce,
                    qi_current: 20.0,
                    qi_max: 20.0,
                    ..Cultivation::default()
                },
            ))
            .id();

        app.world_mut()
            .resource_mut::<valence::prelude::Events<FalseSkinForgeRequest>>()
            .send(FalseSkinForgeRequest {
                crafter: entity,
                kind: FalseSkinKind::SpiderSilk,
            });
        app.update();

        let inventory = app.world().entity(entity).get::<PlayerInventory>().unwrap();
        assert_eq!(count_template(inventory, SPIDER_SILK_MATERIAL_ID), 2);
        assert_eq!(count_template(inventory, SPIDER_SILK_FALSE_SKIN_ITEM_ID), 1);
        let cultivation = app.world().entity(entity).get::<Cultivation>().unwrap();
        assert_eq!(cultivation.qi_current, 15.0);
    }

    #[test]
    fn shed_event_records_life_record_entry() {
        let mut app = App::new();
        app.add_event::<ShedEvent>();
        app.add_systems(Update, record_shed_events_in_life_record);
        let target = app.world_mut().spawn(LifeRecord::new("offline:Azure")).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<ShedEvent>>()
            .send(ShedEvent {
                target,
                attacker: None,
                target_id: "offline:Azure".to_string(),
                attacker_id: Some("npc:ash_spider".to_string()),
                kind: FalseSkinKind::RottenWoodArmor,
                layers_shed: 2,
                layers_remaining: 1,
                contam_absorbed: 60.0,
                contam_overflow: 5.0,
                tick: 42,
            });

        app.update();

        let life_record = app.world().entity(target).get::<LifeRecord>().unwrap();
        assert!(matches!(
            life_record.biography.as_slice(),
            [BiographyEntry::FalseSkinShed {
                kind,
                layers_shed: 2,
                contam_absorbed,
                contam_overflow,
                attacker_id: Some(attacker),
                tick: 42,
            }] if kind == "RottenWoodArmor"
                && (*contam_absorbed - 60.0).abs() < f64::EPSILON
                && (*contam_overflow - 5.0).abs() < f64::EPSILON
                && attacker == "npc:ash_spider"
        ));
    }

    #[test]
    fn sync_false_skin_sets_layers_without_using_chest_slot() {
        let mut app = App::new();
        app.add_systems(Update, sync_false_skin_from_inventory);
        let mut inv = inventory_with_materials();
        inv.equipped.insert(
            EQUIP_SLOT_FALSE_SKIN.to_string(),
            item(42, ROTTEN_WOOD_ARMOR_ITEM_ID, 1),
        );
        let entity = app.world_mut().spawn((inv, DerivedAttrs::default())).id();

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert_eq!(attrs.tuike_layers, 3);
        let skin = app.world().entity(entity).get::<FalseSkin>().unwrap();
        assert_eq!(skin.instance_id, 42);
        assert_eq!(skin.kind, FalseSkinKind::RottenWoodArmor);
    }

    #[test]
    fn false_skin_kind_implements_qi_physics_style_defense() {
        let silk = FalseSkinKind::SpiderSilk;
        let armor = FalseSkinKind::RottenWoodArmor;

        assert_eq!(silk.defense_color(), ColorKind::Gentle);
        assert_eq!(armor.defense_color(), ColorKind::Solid);
        assert!(armor.resistance() > silk.resistance());
        assert!(silk.drain_affinity() > armor.drain_affinity());
    }
}
