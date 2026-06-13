use std::collections::HashSet;

use valence::prelude::{
    bevy_ecs, App, BlockPos, Client, Commands, Component, DVec3, DiggingEvent, Entity, EventReader,
    GameMode, IntoSystemConfigs, Position, Query, Res, ResMut, Update, With,
};

use crate::inventory::external_container::{
    ExternalContainer, ExternalContainerKind, ExternalContainerRegistry,
};
use crate::inventory::{
    spawn_template_dropped_loot, ContainerState, DroppedLootEntry, DroppedLootRegistry,
    InventoryInstanceIdAllocator, ItemRegistry, PlacedItemState, TemplateDroppedLootRequest,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::gameplay::GameplayTick;
use crate::schema::server_data::{
    LootContainerCloseReasonV1, LootContainerCloseV1, ServerDataPayloadV1, ServerDataV1,
};
use crate::world::block_break::should_apply_default_break;
use crate::world::dimension::{CurrentDimension, DimensionKind};

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
    pos: BlockPos,
    dimension: DimensionKind,
    placed_by: Entity,
    placed_at_tick: u64,
    kind: ContainerBlockKind,
) -> Entity {
    let entity = commands.spawn_empty().id();
    let session_id = registry.allocate_session(entity);
    commands.entity(entity).insert((
        ContainerBlock {
            kind,
            placed_by,
            placed_at_tick,
        },
        Position(DVec3::new(
            f64::from(pos.x) + 0.5,
            f64::from(pos.y),
            f64::from(pos.z) + 0.5,
        )),
        CurrentDimension(dimension),
        build_external_container(session_id, kind),
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
        commands.entity(entity).despawn();
    }
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

        assert_eq!(kind.grid_dimensions(), (4, 4));
        assert_eq!(kind.item_template_id(), "trade_crate");
        assert_eq!(
            kind.external_kind(),
            ExternalContainerKind::StorageCrate { is_herb: false }
        );
    }

    #[test]
    fn herb_crate_kind_uses_dedicated_template_and_4x4_grid() {
        let kind = ContainerBlockKind::StorageCrate { is_herb: true };

        assert_eq!(kind.grid_dimensions(), (4, 4));
        assert_eq!(kind.item_template_id(), "herb_crate_placed");
        assert_eq!(
            kind.external_kind(),
            ExternalContainerKind::StorageCrate { is_herb: true }
        );
    }

    #[test]
    fn dead_drop_kind_uses_dead_drop_template_and_3x3_grid() {
        let kind = ContainerBlockKind::DeadDrop;

        assert_eq!(kind.grid_dimensions(), (3, 3));
        assert_eq!(kind.item_template_id(), "dead_drop_box");
        assert_eq!(kind.external_kind(), ExternalContainerKind::DeadDrop);
    }

    #[test]
    fn build_external_container_sets_session_and_source_kind() {
        let ext = build_external_container(42, ContainerBlockKind::StorageCrate { is_herb: true });

        assert_eq!(ext.session_id, 42);
        assert_eq!(ext.container.id, "ext_42");
        assert_eq!(ext.container.name, "灵草箱");
        assert_eq!(ext.container.rows, 4);
        assert_eq!(ext.container.cols, 4);
        assert_eq!(
            ext.source_kind,
            ExternalContainerKind::StorageCrate { is_herb: true }
        );
        assert_eq!(ext.opened_by, None);
        assert_eq!(ext.timeout_wall_secs, 0);
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

        assert_eq!(count, 1);
        let entry = registry
            .entries
            .get(&9001)
            .expect("placed item instance should become dropped loot");
        assert_eq!(entry.source_container_id, "ext_7");
        assert_eq!(entry.source_row, 1);
        assert_eq!(entry.source_col, 2);
        assert_eq!(entry.world_pos, [10.5, 64.0, -2.5]);
        assert_eq!(entry.dimension, DimensionKind::Tsy);
        assert_eq!(entry.item.template_id, "bone_coin_stack");
        assert_eq!(entry.item.stack_count, 3);
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

        assert_eq!(count, 0);
        assert!(registry.entries.is_empty());
    }

    #[test]
    fn container_block_pos_floors_marker_position_to_block_pos() {
        let position = Position(DVec3::new(3.5, 64.0, -7.5));

        assert_eq!(container_block_pos(&position), [3, 64, -8]);
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
