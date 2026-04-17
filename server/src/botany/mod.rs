pub mod components;
pub mod harvest;
pub mod integration;
pub mod lifecycle;
pub mod registry;

use valence::prelude::{App, IntoSystemConfigs, Query, Startup, Update, With};

use crate::inventory::{InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use components::{
    HarvestSessionStore, InventorySnapshotPushQueue, PlantLifecycleClock, PlantStaticPointStore,
};
use harvest::tick_harvest_sessions;
use lifecycle::{initialize_static_points_from_zones, run_botany_lifecycle_tick};
use registry::BotanyKindRegistry;

pub fn register(app: &mut App) {
    app.insert_resource(BotanyKindRegistry::default());
    app.insert_resource(PlantLifecycleClock::default());
    app.insert_resource(PlantStaticPointStore::default());
    app.insert_resource(HarvestSessionStore::default());
    app.insert_resource(InventorySnapshotPushQueue::default());

    app.add_systems(
        Update,
        (
            initialize_static_points_from_zones,
            run_botany_lifecycle_tick,
            tick_harvest_sessions,
            emit_botany_inventory_snapshots,
        )
            .chain(),
    );
    app.add_systems(Startup, validate_botany_inventory_primitives_on_startup);
}

fn validate_botany_inventory_primitives_on_startup(
    item_registry: valence::prelude::Res<ItemRegistry>,
    mut allocator: valence::prelude::ResMut<InventoryInstanceIdAllocator>,
) {
    if let Err(error) = ensure_botany_inventory_primitives(item_registry.as_ref(), &mut allocator) {
        panic!("[bong][botany] inventory primitive validation failed: {error}");
    }
}

type BotanyInventoryEmitQueryItem<'a> = (
    &'a mut valence::prelude::Client,
    &'a crate::player::state::PlayerState,
    &'a PlayerInventory,
);

fn emit_botany_inventory_snapshots(
    mut queue: valence::prelude::ResMut<InventorySnapshotPushQueue>,
    mut clients: Query<BotanyInventoryEmitQueryItem<'_>, With<valence::prelude::Client>>,
) {
    use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
    use crate::network::inventory_snapshot_emit::build_inventory_snapshot_for_network;

    let pending = queue.drain();
    if pending.is_empty() {
        return;
    }

    for target in pending {
        let Ok((mut client, player_state, inventory)) = clients.get_mut(target.client_entity)
        else {
            continue;
        };

        let snapshot = build_inventory_snapshot_for_network(inventory, player_state);
        let payload = ServerDataV1::new(ServerDataPayloadV1::InventorySnapshot(Box::new(snapshot)));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(payload) => payload,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };

        send_server_data_payload(&mut client, payload_bytes.as_slice());
    }
}

pub fn ensure_botany_inventory_primitives(
    item_registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
) -> Result<(), String> {
    // 通过一次最小路径调用确保 core primitives 可用，避免后续 runtime 才暴雷。
    let _ = item_registry
        .get(registry::CI_SHE_HAO)
        .ok_or_else(|| format!("missing canonical herb template `{}`", registry::CI_SHE_HAO))?;
    let _ = allocator.next_id()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_registers_botany_resources() {
        let mut app = App::new();
        register(&mut app);

        assert!(app.world().contains_resource::<BotanyKindRegistry>());
        assert!(app.world().contains_resource::<PlantLifecycleClock>());
        assert!(app.world().contains_resource::<PlantStaticPointStore>());
        assert!(app.world().contains_resource::<HarvestSessionStore>());
        assert!(app
            .world()
            .contains_resource::<InventorySnapshotPushQueue>());
    }
}
