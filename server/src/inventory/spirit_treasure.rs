use std::collections::{HashMap, HashSet};

use valence::prelude::bevy_ecs;
use valence::prelude::{Changed, Commands, Component, DVec3, Entity, Query, ResMut, Resource};

use crate::combat::components::{ActiveStatusEffect, StatusEffects};
use crate::combat::events::StatusEffectKind;
use crate::combat::status::{remove_status_effect, upsert_status_effect};
use crate::inventory::ancient_relics::AncientRelicSource;
use crate::inventory::{
    InventoryInstanceIdAllocator, ItemInstance, ItemRarity, PlayerInventory,
    EQUIP_SLOT_TREASURE_BELT_0,
};
use crate::schema::spirit_treasure::{
    SpiritTreasureClientStateV1, SpiritTreasurePassiveV1, SpiritTreasureStatePayloadV1,
};
use crate::world::zone::TsyDepth;

pub const JIZHAOJING_TEMPLATE_ID: &str = "spirit_treasure_jizhaojing";
pub const JIZHAOJING_DISPLAY_NAME: &str = "寂照镜";
pub const JIZHAOJING_PROMPT_FILE: &str = "spirit-treasure-jizhaojing.md";

#[derive(Debug, Clone, PartialEq)]
pub struct SpiritTreasureDef {
    pub template_id: String,
    pub display_name: String,
    pub description: String,
    pub source_sect: Option<String>,
    pub passive_effects: Vec<SpiritTreasurePassive>,
    pub personality_prompt_file: String,
    pub dialogue_model: String,
    pub dialogue_cooldown_s: u32,
    pub random_dialogue_interval_s: (u32, u32),
    pub icon_texture: String,
    pub equip_slot: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpiritTreasurePassive {
    pub effect_kind: StatusEffectKind,
    pub magnitude: f32,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpiritTreasureWorldState {
    pub instance_id: u64,
    pub holder: SpiritTreasureHolder,
    pub affinity: f64,
    pub dialogue_count: u32,
    pub last_dialogue_tick: u64,
    pub sleeping: bool,
    pub spawned_at_tick: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpiritTreasureHolder {
    Player(Entity),
    Ground(DVec3),
    Lost,
}

#[derive(Debug, Resource)]
pub struct SpiritTreasureRegistry {
    pub defs: HashMap<String, SpiritTreasureDef>,
    pub active: HashMap<String, SpiritTreasureWorldState>,
    pub max_concurrent: usize,
}

impl Default for SpiritTreasureRegistry {
    fn default() -> Self {
        Self::from_defs(vec![jizhaojing_def()], 3)
    }
}

impl SpiritTreasureRegistry {
    pub fn from_defs(defs: Vec<SpiritTreasureDef>, max_concurrent: usize) -> Self {
        Self {
            defs: defs
                .into_iter()
                .map(|def| (def.template_id.clone(), def))
                .collect(),
            active: HashMap::new(),
            max_concurrent,
        }
    }

    pub fn find_by_display_name(&self, display_name: &str) -> Option<&SpiritTreasureDef> {
        self.defs
            .values()
            .find(|def| def.display_name == display_name)
    }

    pub fn ensure_player_holder(
        &mut self,
        template_id: &str,
        instance_id: u64,
        holder: Entity,
        spawned_at_tick: u64,
    ) {
        let state = self
            .active
            .entry(template_id.to_string())
            .or_insert_with(|| SpiritTreasureWorldState {
                instance_id,
                holder: SpiritTreasureHolder::Player(holder),
                affinity: 0.5,
                dialogue_count: 0,
                last_dialogue_tick: 0,
                sleeping: false,
                spawned_at_tick,
            });

        state.instance_id = instance_id;
        state.holder = SpiritTreasureHolder::Player(holder);
        state.sleeping = state.affinity <= 0.2;
    }

    pub fn mark_lost_if_instance_absent(&mut self, template_id: &str, instance_id: u64) {
        if let Some(state) = self.active.get_mut(template_id) {
            if state.instance_id == instance_id {
                state.holder = SpiritTreasureHolder::Lost;
            }
        }
    }

    pub fn affinity_scale(&self, template_id: &str) -> f32 {
        self.active
            .get(template_id)
            .map(|state| affinity_scale(state.affinity))
            .unwrap_or(1.0)
    }

    pub fn apply_affinity_delta(&mut self, template_id: &str, delta: f64) -> Option<f64> {
        let state = self.active.get_mut(template_id)?;
        state.affinity = (state.affinity + delta.clamp(-0.1, 0.1)).clamp(0.0, 1.0);
        state.dialogue_count = state.dialogue_count.saturating_add(1);
        state.sleeping = state.affinity <= 0.2;
        Some(state.affinity)
    }
}

#[derive(Debug, Clone, Component, Default, PartialEq)]
pub struct ActiveSpiritTreasures {
    pub treasures: Vec<ActiveTreasureEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveTreasureEntry {
    pub template_id: String,
    pub instance_id: u64,
    pub equipped: bool,
    pub passive_active: bool,
}

pub fn jizhaojing_def() -> SpiritTreasureDef {
    SpiritTreasureDef {
        template_id: JIZHAOJING_TEMPLATE_ID.to_string(),
        display_name: JIZHAOJING_DISPLAY_NAME.to_string(),
        description: "上古清风宗掌教本命法器，镜中残存明虚一缕神识。".to_string(),
        source_sect: Some("清风宗".to_string()),
        passive_effects: vec![
            SpiritTreasurePassive {
                effect_kind: StatusEffectKind::SpiritTreasurePerception,
                magnitude: 0.30,
                description: "感知范围 +30%".to_string(),
            },
            SpiritTreasurePassive {
                effect_kind: StatusEffectKind::MirrorConcealment,
                magnitude: 0.15,
                description: "匿探 +15%".to_string(),
            },
            SpiritTreasurePassive {
                effect_kind: StatusEffectKind::MirrorExposed,
                magnitude: 0.05,
                description: "负压 -5%".to_string(),
            },
        ],
        personality_prompt_file: JIZHAOJING_PROMPT_FILE.to_string(),
        dialogue_model: "claude-haiku-4-5-20251001".to_string(),
        dialogue_cooldown_s: 30,
        random_dialogue_interval_s: (300, 900),
        icon_texture: "bong-client:textures/gui/items/spirit_treasure_jizhaojing.png".to_string(),
        equip_slot: EQUIP_SLOT_TREASURE_BELT_0.to_string(),
    }
}

pub fn sync_spirit_treasures(
    mut commands: Commands,
    mut registry: ResMut<SpiritTreasureRegistry>,
    mut inventories: Query<
        (
            Entity,
            &PlayerInventory,
            Option<&mut ActiveSpiritTreasures>,
            Option<&mut StatusEffects>,
        ),
        Changed<PlayerInventory>,
    >,
) {
    for (entity, inventory, active_component, status_effects) in &mut inventories {
        let previous = active_component
            .as_ref()
            .map(|active| active.treasures.clone())
            .unwrap_or_default();
        let current = scan_inventory_for_spirit_treasures(&registry, inventory);

        for entry in &current {
            registry.ensure_player_holder(&entry.template_id, entry.instance_id, entity, 0);
        }
        mark_removed_treasures_lost(&mut registry, previous.as_slice(), current.as_slice());

        if let Some(mut active) = active_component {
            active.treasures = current.clone();
        } else {
            commands.entity(entity).insert(ActiveSpiritTreasures {
                treasures: current.clone(),
            });
        }

        if let Some(mut statuses) = status_effects {
            sync_passive_status_effects(&registry, current.as_slice(), &mut statuses);
        }
    }
}

pub fn scan_inventory_for_spirit_treasures(
    registry: &SpiritTreasureRegistry,
    inventory: &PlayerInventory,
) -> Vec<ActiveTreasureEntry> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();

    for item in inventory.equipped.values() {
        push_entry_for_item(registry, item, true, &mut seen, &mut entries);
    }

    for container in &inventory.containers {
        for placed in &container.items {
            push_entry_for_item(registry, &placed.instance, false, &mut seen, &mut entries);
        }
    }

    for item in inventory.hotbar.iter().flatten() {
        push_entry_for_item(registry, item, false, &mut seen, &mut entries);
    }

    entries
}

pub fn state_payload_for_active_treasures(
    registry: &SpiritTreasureRegistry,
    active: &ActiveSpiritTreasures,
) -> SpiritTreasureStatePayloadV1 {
    SpiritTreasureStatePayloadV1 {
        treasures: active
            .treasures
            .iter()
            .filter_map(|entry| {
                let def = registry.defs.get(&entry.template_id)?;
                let world_state = registry.active.get(&entry.template_id);
                let affinity = world_state.map(|state| state.affinity).unwrap_or(0.5);
                Some(SpiritTreasureClientStateV1 {
                    template_id: entry.template_id.clone(),
                    display_name: def.display_name.clone(),
                    instance_id: entry.instance_id,
                    equipped: entry.equipped,
                    passive_active: entry.passive_active,
                    affinity,
                    sleeping: world_state.map(|state| state.sleeping).unwrap_or(false),
                    source_sect: def.source_sect.clone(),
                    icon_texture: def.icon_texture.clone(),
                    passive_effects: def
                        .passive_effects
                        .iter()
                        .map(|passive| SpiritTreasurePassiveV1 {
                            kind: format!("{:?}", passive.effect_kind),
                            value: passive.magnitude as f64,
                            description: passive.description.clone(),
                        })
                        .collect(),
                })
            })
            .collect(),
    }
}

pub fn affinity_scale(affinity: f64) -> f32 {
    (0.3 + 0.7 * (affinity / 0.8).clamp(0.0, 1.0)) as f32
}

pub fn maybe_spawn_jizhaojing_from_relic_core(
    registry: &mut SpiritTreasureRegistry,
    source: AncientRelicSource,
    depth: TsyDepth,
    spawn_pos: DVec3,
    seed: u64,
    allocator: &mut InventoryInstanceIdAllocator,
) -> Option<ItemInstance> {
    if source != AncientRelicSource::SectRuins || depth != TsyDepth::Deep {
        return None;
    }
    if registry.active.len() >= registry.max_concurrent {
        return None;
    }
    if registry.active.contains_key(JIZHAOJING_TEMPLATE_ID) {
        return None;
    }
    if (seed % 100) >= 15 {
        return None;
    }

    let instance_id = allocator.next_id().ok()?;
    registry.active.insert(
        JIZHAOJING_TEMPLATE_ID.to_string(),
        SpiritTreasureWorldState {
            instance_id,
            holder: SpiritTreasureHolder::Ground(spawn_pos),
            affinity: 0.5,
            dialogue_count: 0,
            last_dialogue_tick: 0,
            sleeping: false,
            spawned_at_tick: 0,
        },
    );
    Some(jizhaojing_item_instance(instance_id))
}

pub fn jizhaojing_item_instance(instance_id: u64) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: JIZHAOJING_TEMPLATE_ID.to_string(),
        display_name: JIZHAOJING_DISPLAY_NAME.to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.6,
        rarity: ItemRarity::Ancient,
        description: "镜面不映人影，只照出周遭灵气流向。器灵明虚尚在沉睡。".to_string(),
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
    }
}

fn push_entry_for_item(
    registry: &SpiritTreasureRegistry,
    item: &ItemInstance,
    equipped: bool,
    seen: &mut HashSet<u64>,
    entries: &mut Vec<ActiveTreasureEntry>,
) {
    if !registry.defs.contains_key(&item.template_id) || !seen.insert(item.instance_id) {
        return;
    }
    entries.push(ActiveTreasureEntry {
        template_id: item.template_id.clone(),
        instance_id: item.instance_id,
        equipped,
        passive_active: equipped,
    });
}

fn mark_removed_treasures_lost(
    registry: &mut SpiritTreasureRegistry,
    previous: &[ActiveTreasureEntry],
    current: &[ActiveTreasureEntry],
) {
    for old in previous {
        let still_present = current.iter().any(|entry| {
            entry.template_id == old.template_id && entry.instance_id == old.instance_id
        });
        if !still_present {
            registry.mark_lost_if_instance_absent(&old.template_id, old.instance_id);
        }
    }
}

fn sync_passive_status_effects(
    registry: &SpiritTreasureRegistry,
    entries: &[ActiveTreasureEntry],
    statuses: &mut StatusEffects,
) {
    for def in registry.defs.values() {
        for passive in &def.passive_effects {
            remove_status_effect(statuses, passive.effect_kind.clone());
        }
    }

    for entry in entries.iter().filter(|entry| entry.passive_active) {
        let Some(def) = registry.defs.get(&entry.template_id) else {
            continue;
        };
        let scale = registry.affinity_scale(&entry.template_id);
        for passive in &def.passive_effects {
            upsert_status_effect(
                statuses,
                ActiveStatusEffect {
                    kind: passive.effect_kind.clone(),
                    magnitude: (passive.magnitude * scale).max(0.01),
                    remaining_ticks: u64::MAX / 2,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ContainerState, InventoryRevision, ItemRarity, PlacedItemState};

    fn item(instance_id: u64) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: JIZHAOJING_TEMPLATE_ID.to_string(),
            display_name: JIZHAOJING_DISPLAY_NAME.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.6,
            rarity: ItemRarity::Ancient,
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
        }
    }

    fn inventory_with_container_item(instance_id: u64) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: "main_pack".to_string(),
                name: "main_pack".to_string(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item(instance_id),
                }],
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    #[test]
    fn scans_spirit_treasure_in_backpack_without_passive() {
        let registry = SpiritTreasureRegistry::default();
        let entries =
            scan_inventory_for_spirit_treasures(&registry, &inventory_with_container_item(88));

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].template_id, JIZHAOJING_TEMPLATE_ID);
        assert!(!entries[0].equipped);
        assert!(!entries[0].passive_active);
    }

    #[test]
    fn equipped_spirit_treasure_activates_scaled_passives() {
        let mut registry = SpiritTreasureRegistry::default();
        let mut inventory = inventory_with_container_item(88);
        let equipped = inventory.containers[0].items.remove(0).instance;
        inventory
            .equipped
            .insert(EQUIP_SLOT_TREASURE_BELT_0.to_string(), equipped);
        let entries = scan_inventory_for_spirit_treasures(&registry, &inventory);
        let player = Entity::from_raw(7);
        registry.ensure_player_holder(JIZHAOJING_TEMPLATE_ID, 88, player, 0);
        registry
            .active
            .get_mut(JIZHAOJING_TEMPLATE_ID)
            .expect("state exists")
            .affinity = 0.8;
        let mut statuses = StatusEffects::default();

        sync_passive_status_effects(&registry, entries.as_slice(), &mut statuses);

        assert!(statuses.active.iter().any(|effect| effect.kind
            == StatusEffectKind::SpiritTreasurePerception
            && (effect.magnitude - 0.30).abs() < f32::EPSILON));
    }

    #[test]
    fn affinity_scale_clamps_to_design_range() {
        assert!((affinity_scale(0.0) - 0.3).abs() < f32::EPSILON);
        assert!((affinity_scale(0.8) - 1.0).abs() < f32::EPSILON);
        assert!((affinity_scale(1.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn jizhaojing_spawn_is_sect_ruins_deep_and_unique() {
        let mut registry = SpiritTreasureRegistry::default();
        let mut allocator = InventoryInstanceIdAllocator::default();

        assert!(maybe_spawn_jizhaojing_from_relic_core(
            &mut registry,
            AncientRelicSource::DaoLord,
            TsyDepth::Deep,
            DVec3::new(0.0, 0.0, 0.0),
            0,
            &mut allocator,
        )
        .is_none());
        let spawned = maybe_spawn_jizhaojing_from_relic_core(
            &mut registry,
            AncientRelicSource::SectRuins,
            TsyDepth::Deep,
            DVec3::new(0.0, 0.0, 0.0),
            0,
            &mut allocator,
        )
        .expect("0% roll should spawn jizhaojing");
        assert_eq!(spawned.template_id, JIZHAOJING_TEMPLATE_ID);
        assert!(matches!(
            registry
                .active
                .get(JIZHAOJING_TEMPLATE_ID)
                .expect("spawn should record world state")
                .holder,
            SpiritTreasureHolder::Ground(_)
        ));
        assert!(maybe_spawn_jizhaojing_from_relic_core(
            &mut registry,
            AncientRelicSource::SectRuins,
            TsyDepth::Deep,
            DVec3::new(0.0, 0.0, 0.0),
            1,
            &mut allocator,
        )
        .is_none());
    }
}
