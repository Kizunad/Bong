use std::collections::HashSet;

use valence::prelude::{
    bevy_ecs, App, BlockPos, Client, Commands, Component, DVec3, DiggingEvent, Entity, EventReader,
    Events, GameMode, IntoSystemConfigs, Position, Query, Res, ResMut, Update, With,
};

use crate::inventory::external_container::{
    ExternalContainer, ExternalContainerKind, ExternalContainerRegistry,
};
use crate::inventory::{
    spawn_template_dropped_loot, ContainerState, DroppedLootEntry, DroppedLootRegistry,
    InventoryInstanceIdAllocator, ItemRegistry, PlacedItemState, TemplateDroppedLootRequest,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest, AUDIO_AREA_RADIUS};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::gameplay::GameplayTick;
use crate::schema::server_data::{
    LootContainerCloseReasonV1, LootContainerCloseV1, ServerDataPayloadV1, ServerDataV1,
};
use crate::supply_coffin::SupplyCoffinGrade;
use crate::world::block_break::should_apply_default_break;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::entity_model::{spawn_visual_marker, BongVisualKind};

pub const CONTAINER_PLACE_AUDIO_RECIPE_ID: &str = "container_place";
pub const CONTAINER_PLACE_DEADDROP_AUDIO_RECIPE_ID: &str = "container_place_deaddrop";
pub const CONTAINER_OPEN_AUDIO_RECIPE_ID: &str = "container_open";
pub const CONTAINER_OPEN_DEADDROP_AUDIO_RECIPE_ID: &str = "container_open_deaddrop";
pub const CONTAINER_BREAK_AUDIO_RECIPE_ID: &str = "container_break";

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct ContainerBlock {
    pub kind: ContainerBlockKind,
    pub placed_by: Entity,
    pub placed_at_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerBlockKind {
    StorageCrate { is_herb: bool },
    DeadDrop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainerBlockPlacement {
    pub layer: Entity,
    pub pos: BlockPos,
    pub dimension: DimensionKind,
    pub placed_by: Entity,
    pub placed_at_tick: u64,
    pub kind: ContainerBlockKind,
}

impl ContainerBlockKind {
    pub const fn external_kind(self) -> ExternalContainerKind {
        match self {
            Self::StorageCrate { is_herb } => ExternalContainerKind::StorageCrate { is_herb },
            Self::DeadDrop => ExternalContainerKind::DeadDrop,
        }
    }

    pub const fn grid_dimensions(self) -> (u8, u8) {
        match self {
            Self::StorageCrate { .. } => (4, 4),
            Self::DeadDrop => (3, 3),
        }
    }

    pub const fn item_template_id(self) -> &'static str {
        match self {
            Self::StorageCrate { is_herb: false } => "trade_crate",
            Self::StorageCrate { is_herb: true } => "herb_crate_placed",
            Self::DeadDrop => "dead_drop_box",
        }
    }

    pub const fn visual_kind(self) -> BongVisualKind {
        match self {
            Self::StorageCrate { is_herb: false } => BongVisualKind::TradeCrate,
            Self::StorageCrate { is_herb: true } => BongVisualKind::HerbCratePlaced,
            Self::DeadDrop => BongVisualKind::DeadDropBox,
        }
    }

    const fn display_name(self) -> &'static str {
        match self {
            Self::StorageCrate { is_herb: false } => "货箱",
            Self::StorageCrate { is_herb: true } => "灵草箱",
            Self::DeadDrop => "死信箱",
        }
    }
}

pub fn register(app: &mut App) {
    app.init_resource::<ExternalContainerRegistry>();
    app.add_systems(
        Update,
        handle_container_block_break.before(crate::world::block_break::apply_default_block_break),
    );
}

pub fn handle_container_block_place(
    commands: &mut Commands,
    registry: &mut ExternalContainerRegistry,
    placement: ContainerBlockPlacement,
) -> Entity {
    let entity = spawn_visual_marker(
        commands,
        placement.layer,
        None,
        placement.kind.visual_kind(),
        DVec3::new(
            f64::from(placement.pos.x) + 0.5,
            f64::from(placement.pos.y),
            f64::from(placement.pos.z) + 0.5,
        ),
        0,
    );
    let session_id = registry.allocate_session(entity);
    commands.entity(entity).insert((
        ContainerBlock {
            kind: placement.kind,
            placed_by: placement.placed_by,
            placed_at_tick: placement.placed_at_tick,
        },
        CurrentDimension(placement.dimension),
        build_external_container(session_id, placement.kind),
    ));
    entity
}

pub fn build_external_container(session_id: u64, kind: ContainerBlockKind) -> ExternalContainer {
    let (rows, cols) = kind.grid_dimensions();
    ExternalContainer {
        session_id,
        container: ContainerState {
            id: ExternalContainer::container_id(session_id),
            name: kind.display_name().to_string(),
            rows,
            cols,
            items: vec![],
        },
        opened_by: None,
        timeout_wall_secs: 0,
        source_kind: kind.external_kind(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_container_block_break(
    mut commands: Commands,
    mut digs: EventReader<DiggingEvent>,
    item_registry: Res<ItemRegistry>,
    current_tick: Option<Res<GameplayTick>>,
    mut instance_allocator: ResMut<InventoryInstanceIdAllocator>,
    mut dropped_registry: ResMut<DroppedLootRegistry>,
    mut ext_registry: ResMut<ExternalContainerRegistry>,
    breakers: Query<(&GameMode, Option<&CurrentDimension>), With<Client>>,
    containers: Query<(
        Entity,
        &Position,
        Option<&CurrentDimension>,
        &ContainerBlock,
        &ExternalContainer,
    )>,
    mut clients: Query<&mut Client, With<Client>>,
    mut audio_events: Option<ResMut<Events<PlaySoundRecipeRequest>>>,
) {
    let mut handled_entities = HashSet::new();
    for event in digs.read() {
        let Ok((game_mode, current_dimension)) = breakers.get(event.client) else {
            continue;
        };
        if !should_apply_default_break(event.state, *game_mode) {
            continue;
        }
        let dimension = current_dimension
            .map(|component| component.0)
            .unwrap_or(DimensionKind::Overworld);
        let Some((entity, position, _container_dimension, block, ext)) =
            containers
                .iter()
                .find(|(_, position, container_dimension, _, _)| {
                    container_block_pos(position)
                        == [event.position.x, event.position.y, event.position.z]
                        && container_dimension
                            .map(|component| component.0)
                            .unwrap_or(DimensionKind::Overworld)
                            == dimension
                })
        else {
            continue;
        };
        if !handled_entities.insert(entity) {
            continue;
        }

        let now = current_tick
            .as_ref()
            .map(|tick| tick.current_tick())
            .unwrap_or(0);
        let world_pos = [position.0.x, position.0.y, position.0.z];

        if let Err(error) = spawn_template_dropped_loot(
            &mut dropped_registry,
            &item_registry,
            &mut instance_allocator,
            TemplateDroppedLootRequest {
                template_id: block.kind.item_template_id(),
                stack_count: 1,
                world_pos,
                dimension,
                current_tick: now,
            },
        ) {
            tracing::error!(
                "[bong][container_block] rejected break: failed to drop `{}`: {error}",
                block.kind.item_template_id()
            );
        }

        if let Some(player) = ext.opened_by {
            send_container_destroyed_close(ext.session_id, &mut clients, player);
        }
        drain_container_items_to_drops(&mut dropped_registry, ext, world_pos, dimension);
        ext_registry.remove_session(ext.session_id);
        commands.entity(entity).insert(valence::prelude::Despawned);
        send_container_audio(
            audio_events.as_deref_mut(),
            CONTAINER_BREAK_AUDIO_RECIPE_ID,
            [event.position.x, event.position.y, event.position.z],
            0.0,
        );
    }
}

pub const fn container_place_audio_recipe_id(kind: ContainerBlockKind) -> &'static str {
    match kind {
        ContainerBlockKind::StorageCrate { .. } => CONTAINER_PLACE_AUDIO_RECIPE_ID,
        ContainerBlockKind::DeadDrop => CONTAINER_PLACE_DEADDROP_AUDIO_RECIPE_ID,
    }
}

pub const fn container_open_audio_cue(kind: &ExternalContainerKind) -> (&'static str, f32) {
    match kind {
        ExternalContainerKind::StorageCrate { is_herb: false } => {
            (CONTAINER_OPEN_AUDIO_RECIPE_ID, 0.0)
        }
        ExternalContainerKind::StorageCrate { is_herb: true } => {
            (CONTAINER_OPEN_AUDIO_RECIPE_ID, 0.1)
        }
        ExternalContainerKind::DeadDrop => (CONTAINER_OPEN_DEADDROP_AUDIO_RECIPE_ID, 0.0),
        ExternalContainerKind::SupplyCoffin { grade } => match grade {
            SupplyCoffinGrade::Common => ("supply_coffin_open_common", 0.0),
            SupplyCoffinGrade::Rare => ("supply_coffin_open_rare", 0.0),
            SupplyCoffinGrade::Precious => ("supply_coffin_open_precious", 0.0),
        },
    }
}

pub fn send_container_audio(
    audio_events: Option<&mut Events<PlaySoundRecipeRequest>>,
    recipe_id: &str,
    block_pos: [i32; 3],
    pitch_shift: f32,
) {
    let Some(audio_events) = audio_events else {
        return;
    };
    let origin = DVec3::new(
        f64::from(block_pos[0]) + 0.5,
        f64::from(block_pos[1]) + 0.5,
        f64::from(block_pos[2]) + 0.5,
    );
    audio_events.send(PlaySoundRecipeRequest {
        recipe_id: recipe_id.to_string(),
        instance_id: 0,
        pos: Some(block_pos),
        flag: None,
        volume_mul: 1.0,
        pitch_shift,
        recipient: AudioRecipient::Radius {
            origin,
            radius: AUDIO_AREA_RADIUS,
        },
    });
}

pub fn drain_container_items_to_drops(
    registry: &mut DroppedLootRegistry,
    ext: &ExternalContainer,
    world_pos: [f64; 3],
    dimension: DimensionKind,
) -> usize {
    let mut count = 0;
    for placed in &ext.container.items {
        let entry = dropped_entry_from_placed(placed, &ext.container.id, world_pos, dimension);
        registry.entries.insert(entry.instance_id, entry);
        count += 1;
    }
    count
}

fn dropped_entry_from_placed(
    placed: &PlacedItemState,
    source_container_id: &str,
    world_pos: [f64; 3],
    dimension: DimensionKind,
) -> DroppedLootEntry {
    DroppedLootEntry {
        instance_id: placed.instance.instance_id,
        source_container_id: source_container_id.to_string(),
        source_row: placed.row,
        source_col: placed.col,
        world_pos,
        dimension,
        item: placed.instance.clone(),
    }
}

fn send_container_destroyed_close(
    session_id: u64,
    clients: &mut Query<&mut Client, With<Client>>,
    player_entity: Entity,
) {
    let Ok(mut client) = clients.get_mut(player_entity) else {
        return;
    };
    let close_payload = ServerDataV1::new(ServerDataPayloadV1::LootContainerClose(
        LootContainerCloseV1 {
            session_id,
            reason: LootContainerCloseReasonV1::ContainerDestroyed,
        },
    ));
    let payload_type = payload_type_label(close_payload.payload_type());
    let bytes = match serialize_server_data_payload(&close_payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(&mut client, bytes.as_slice());
}

pub fn container_block_pos(position: &Position) -> [i32; 3] {
    [
        position.0.x.floor() as i32,
        position.0.y.floor() as i32,
        position.0.z.floor() as i32,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{DroppedLootRegistry, ItemInstance, ItemRarity, PlacedItemState};

    #[test]
    fn storage_crate_kind_uses_trade_template_and_4x4_grid() {
        let kind = ContainerBlockKind::StorageCrate { is_herb: false };

        assert_eq!(
            kind.grid_dimensions(),
            (4, 4),
            "expected trade crate to expose a 4x4 grid because P0 storage crates use crate capacity, actual {:?}",
            kind.grid_dimensions()
        );
        assert_eq!(
            kind.item_template_id(),
            "trade_crate",
            "expected non-herb storage crate to drop trade_crate because it maps to the trade template, actual {}",
            kind.item_template_id()
        );
        assert_eq!(
            kind.external_kind(),
            ExternalContainerKind::StorageCrate { is_herb: false },
            "expected non-herb storage crate to preserve is_herb=false in ExternalContainerKind, actual {:?}",
            kind.external_kind()
        );
        assert_eq!(
            kind.visual_kind().entity_kind(),
            crate::world::entity_model::TRADE_CRATE_ENTITY_KIND,
            "expected trade crate visual kind to use raw_id 166"
        );
    }

    #[test]
    fn herb_crate_kind_uses_dedicated_template_and_4x4_grid() {
        let kind = ContainerBlockKind::StorageCrate { is_herb: true };

        assert_eq!(
            kind.grid_dimensions(),
            (4, 4),
            "expected placed herb crate to expose a 4x4 grid because it shares storage crate capacity, actual {:?}",
            kind.grid_dimensions()
        );
        assert_eq!(
            kind.item_template_id(),
            "herb_crate_placed",
            "expected herb storage crate to drop the dedicated placed template, actual {}",
            kind.item_template_id()
        );
        assert_eq!(
            kind.external_kind(),
            ExternalContainerKind::StorageCrate { is_herb: true },
            "expected herb storage crate to preserve is_herb=true in ExternalContainerKind, actual {:?}",
            kind.external_kind()
        );
        assert_eq!(
            kind.visual_kind().entity_kind(),
            crate::world::entity_model::HERB_CRATE_PLACED_ENTITY_KIND,
            "expected herb crate visual kind to use raw_id 167"
        );
    }

    #[test]
    fn dead_drop_kind_uses_dead_drop_template_and_3x3_grid() {
        let kind = ContainerBlockKind::DeadDrop;

        assert_eq!(
            kind.grid_dimensions(),
            (3, 3),
            "expected dead drop to expose a 3x3 grid because P0 reserves smaller dead-drop storage, actual {:?}",
            kind.grid_dimensions()
        );
        assert_eq!(
            kind.item_template_id(),
            "dead_drop_box",
            "expected dead drop break to drop dead_drop_box template, actual {}",
            kind.item_template_id()
        );
        assert_eq!(
            kind.external_kind(),
            ExternalContainerKind::DeadDrop,
            "expected dead drop to map to ExternalContainerKind::DeadDrop, actual {:?}",
            kind.external_kind()
        );
        assert_eq!(
            kind.visual_kind().entity_kind(),
            crate::world::entity_model::DEAD_DROP_BOX_ENTITY_KIND,
            "expected dead drop visual kind to use raw_id 168"
        );
    }

    #[test]
    fn container_audio_cues_match_placeable_container_spec() {
        assert_eq!(
            container_place_audio_recipe_id(ContainerBlockKind::StorageCrate { is_herb: false }),
            CONTAINER_PLACE_AUDIO_RECIPE_ID,
            "trade crate placement should use the plain wood placement recipe"
        );
        assert_eq!(
            container_place_audio_recipe_id(ContainerBlockKind::StorageCrate { is_herb: true }),
            CONTAINER_PLACE_AUDIO_RECIPE_ID,
            "herb crate placement should use the plain wood placement recipe"
        );
        assert_eq!(
            container_place_audio_recipe_id(ContainerBlockKind::DeadDrop),
            CONTAINER_PLACE_DEADDROP_AUDIO_RECIPE_ID,
            "dead drop placement should use the deaddrop recipe with the buried gravel layer"
        );
        assert_eq!(
            container_open_audio_cue(&ExternalContainerKind::StorageCrate { is_herb: false }),
            (CONTAINER_OPEN_AUDIO_RECIPE_ID, 0.0),
            "trade crate open should use barrel open at base pitch"
        );
        assert_eq!(
            container_open_audio_cue(&ExternalContainerKind::StorageCrate { is_herb: true }),
            (CONTAINER_OPEN_AUDIO_RECIPE_ID, 0.1),
            "herb crate open should lift barrel pitch by +0.1"
        );
        assert_eq!(
            container_open_audio_cue(&ExternalContainerKind::DeadDrop),
            (CONTAINER_OPEN_DEADDROP_AUDIO_RECIPE_ID, 0.0),
            "dead drop open should use its chest + chime recipe"
        );
        assert_eq!(
            container_open_audio_cue(&ExternalContainerKind::SupplyCoffin {
                grade: SupplyCoffinGrade::Common,
            }),
            ("supply_coffin_open_common", 0.0),
            "generic open fallback must preserve existing common supply coffin audio"
        );
        assert_eq!(
            container_open_audio_cue(&ExternalContainerKind::SupplyCoffin {
                grade: SupplyCoffinGrade::Rare,
            }),
            ("supply_coffin_open_rare", 0.0),
            "generic open fallback must preserve existing rare supply coffin audio"
        );
        assert_eq!(
            container_open_audio_cue(&ExternalContainerKind::SupplyCoffin {
                grade: SupplyCoffinGrade::Precious,
            }),
            ("supply_coffin_open_precious", 0.0),
            "generic open fallback must preserve existing precious supply coffin audio"
        );
    }

    #[test]
    fn handle_container_block_place_spawns_dimensioned_external_container() {
        let mut app = App::new();
        let layer = app.world_mut().spawn_empty().id();
        let placed_by = app.world_mut().spawn_empty().id();
        let mut registry = ExternalContainerRegistry::default();
        let pos = BlockPos::new(-2, 63, 4);
        let kind = ContainerBlockKind::DeadDrop;

        let entity = handle_container_block_place(
            &mut app.world_mut().commands(),
            &mut registry,
            ContainerBlockPlacement {
                layer,
                pos,
                dimension: DimensionKind::Tsy,
                placed_by,
                placed_at_tick: 77,
                kind,
            },
        );
        app.world_mut().flush();

        let block = app
            .world()
            .get::<ContainerBlock>(entity)
            .expect("placed container entity should carry ContainerBlock");
        assert_eq!(
            block.kind, kind,
            "expected placed entity to store requested ContainerBlockKind because break routing uses it, actual {:?}",
            block.kind
        );
        let visual = app
            .world()
            .get::<crate::world::entity_model::BongVisualEntity>(entity)
            .expect("placed container entity should be a Bong visual marker");
        assert_eq!(
            visual.kind,
            BongVisualKind::DeadDropBox,
            "expected dead_drop_box placement to expose DEAD_DROP_BOX marker raw id, actual {:?}",
            visual.kind
        );
        assert_eq!(
            block.placed_by, placed_by,
            "expected placed_by to preserve the placing player for later ownership rules, actual {:?}",
            block.placed_by
        );
        assert_eq!(
            block.placed_at_tick, 77,
            "expected placed_at_tick to preserve placement tick for lifecycle logic, actual {}",
            block.placed_at_tick
        );

        let position = app
            .world()
            .get::<Position>(entity)
            .expect("placed container entity should carry Position");
        assert_eq!(
            position.0,
            DVec3::new(-1.5, 63.0, 4.5),
            "expected container marker to sit at block center because break matching floors marker position, actual {:?}",
            position.0
        );
        let dimension = app
            .world()
            .get::<CurrentDimension>(entity)
            .expect("placed container entity should carry CurrentDimension");
        assert_eq!(
            dimension.0,
            DimensionKind::Tsy,
            "expected placed container to store requested dimension because break matching is dimension-scoped, actual {:?}",
            dimension.0
        );

        let ext = app
            .world()
            .get::<ExternalContainer>(entity)
            .expect("placed container entity should carry ExternalContainer");
        assert_eq!(
            ext.session_id, 0,
            "expected first allocated external container session to be 0, actual {}",
            ext.session_id
        );
        assert_eq!(
            registry.sessions.get(&ext.session_id),
            Some(&entity),
            "expected ExternalContainerRegistry to map session to placed entity, actual {:?}",
            registry.sessions.get(&ext.session_id)
        );
    }

    #[test]
    fn build_external_container_sets_session_and_source_kind() {
        let ext = build_external_container(42, ContainerBlockKind::StorageCrate { is_herb: true });

        assert_eq!(
            ext.session_id, 42,
            "expected ExternalContainer to preserve allocated session id, actual {}",
            ext.session_id
        );
        assert_eq!(
            ext.container.id, "ext_42",
            "expected container id to derive from session id, actual {}",
            ext.container.id
        );
        assert_eq!(
            ext.container.name, "灵草箱",
            "expected herb crate display name because UI uses container.name, actual {}",
            ext.container.name
        );
        assert_eq!(
            ext.container.rows, 4,
            "expected herb crate rows to match 4x4 grid, actual {}",
            ext.container.rows
        );
        assert_eq!(
            ext.container.cols, 4,
            "expected herb crate cols to match 4x4 grid, actual {}",
            ext.container.cols
        );
        assert_eq!(
            ext.source_kind,
            ExternalContainerKind::StorageCrate { is_herb: true },
            "expected source_kind to preserve herb storage semantics for later open payload mapping, actual {:?}",
            ext.source_kind
        );
        assert_eq!(
            ext.opened_by, None,
            "expected freshly placed container to start closed, actual {:?}",
            ext.opened_by
        );
        assert_eq!(
            ext.timeout_wall_secs, 0,
            "expected placed container P0 to have no timeout, actual {}",
            ext.timeout_wall_secs
        );
    }

    #[test]
    fn drain_container_items_to_drops_preserves_origin_and_instance() {
        let mut ext = build_external_container(7, ContainerBlockKind::DeadDrop);
        ext.container.items.push(PlacedItemState {
            row: 1,
            col: 2,
            instance: test_item(9001, "bone_coin_stack", 3),
        });
        let mut registry = DroppedLootRegistry::default();

        let count = drain_container_items_to_drops(
            &mut registry,
            &ext,
            [10.5, 64.0, -2.5],
            DimensionKind::Tsy,
        );

        assert_eq!(
            count, 1,
            "expected exactly one placed item to drain into dropped loot, actual {count}"
        );
        let entry = registry
            .entries
            .get(&9001)
            .expect("placed item instance should become dropped loot");
        assert_eq!(
            entry.source_container_id, "ext_7",
            "expected dropped loot to preserve source container id for auditability, actual {}",
            entry.source_container_id
        );
        assert_eq!(
            entry.source_row, 1,
            "expected dropped loot to preserve source row, actual {}",
            entry.source_row
        );
        assert_eq!(
            entry.source_col, 2,
            "expected dropped loot to preserve source col, actual {}",
            entry.source_col
        );
        assert_eq!(
            entry.world_pos,
            [10.5, 64.0, -2.5],
            "expected dropped loot world_pos to match destroyed container marker, actual {:?}",
            entry.world_pos
        );
        assert_eq!(
            entry.dimension,
            DimensionKind::Tsy,
            "expected dropped loot dimension to match breaker/container dimension, actual {:?}",
            entry.dimension
        );
        assert_eq!(
            entry.item.template_id, "bone_coin_stack",
            "expected dropped loot to preserve item template id, actual {}",
            entry.item.template_id
        );
        assert_eq!(
            entry.item.stack_count, 3,
            "expected dropped loot to preserve stack count, actual {}",
            entry.item.stack_count
        );
    }

    #[test]
    fn drain_empty_container_items_to_drops_is_noop() {
        let ext = build_external_container(8, ContainerBlockKind::StorageCrate { is_herb: false });
        let mut registry = DroppedLootRegistry::default();

        let count = drain_container_items_to_drops(
            &mut registry,
            &ext,
            [0.5, 64.0, 0.5],
            DimensionKind::Overworld,
        );

        assert_eq!(
            count, 0,
            "expected empty container drain to report zero dropped entries, actual {count}"
        );
        assert!(
            registry.entries.is_empty(),
            "expected empty container drain to leave DroppedLootRegistry empty, actual {:?}",
            registry.entries
        );
    }

    #[test]
    fn container_block_pos_floors_marker_position_to_block_pos() {
        let position = Position(DVec3::new(3.5, 64.0, -7.5));

        assert_eq!(
            container_block_pos(&position),
            [3, 64, -8],
            "expected marker position to floor to containing block coordinates, actual {:?}",
            container_block_pos(&position)
        );
    }

    fn test_item(instance_id: u64, template_id: &str, stack_count: u32) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: "test item".to_string(),
            stack_count,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: vec![],
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }
}
