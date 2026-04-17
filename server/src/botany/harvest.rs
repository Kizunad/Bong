use valence::prelude::{Entity, Query, Res, ResMut, With};

use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::player::state::canonical_player_id;

use super::components::{
    BotanyHarvestMode, BotanyPhase, HarvestSession, HarvestSessionStore,
    InventorySnapshotPushQueue, Plant, PlantStaticPointStore,
};
use super::registry::{BotanyKindRegistry, BotanyPlantId};

const MANUAL_DURATION_TICKS: u64 = 40;
const AUTO_DURATION_TICKS: u64 = 120;
const MANUAL_SKILL_XP: u64 = 2;
const AUTO_SKILL_XP: u64 = 6;

pub fn start_or_resume_harvest(
    store: &mut HarvestSessionStore,
    player_name: &str,
    client_entity: Entity,
    target_entity: Option<Entity>,
    target_plant: BotanyPlantId,
    mode: BotanyHarvestMode,
    now_tick: u64,
) {
    let player_id = canonical_player_id(player_name);
    if store.session_for(player_id.as_str()).is_some() {
        return;
    }

    let duration_ticks = match mode {
        BotanyHarvestMode::Manual => MANUAL_DURATION_TICKS,
        BotanyHarvestMode::Auto => AUTO_DURATION_TICKS,
    };

    store.upsert_session(HarvestSession {
        player_id,
        client_entity,
        target_entity,
        target_plant,
        mode,
        started_at_tick: now_tick,
        duration_ticks,
        phase: BotanyPhase::InProgress,
        last_progress: 0.0,
    });
}

#[allow(clippy::too_many_arguments)]
pub fn complete_harvest_for_player(
    store: &mut HarvestSessionStore,
    player_id: &str,
    plant_query: &mut Query<&mut Plant, With<Plant>>,
    inventory_query: &mut Query<&mut PlayerInventory, With<valence::prelude::Client>>,
    kind_registry: &BotanyKindRegistry,
    item_registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
    snapshot_queue: &mut InventorySnapshotPushQueue,
    static_points: &mut PlantStaticPointStore,
    now_tick: u64,
) -> Result<(), String> {
    let session = store
        .remove_session(player_id)
        .ok_or_else(|| format!("missing harvest session for `{player_id}`"))?;

    if let Some(target_entity) = session.target_entity {
        if let Ok(mut plant) = plant_query.get_mut(target_entity) {
            if let Some(source_point) = plant.source_point {
                if let Some(point) = static_points.get_mut(source_point) {
                    point.bound_entity = None;
                    point.last_spawn_tick = Some(now_tick);
                }
            }
            plant.harvested = true;
        }
    }

    let kind = kind_registry
        .get(session.target_plant)
        .ok_or_else(|| format!("missing kind for `{}`", session.target_plant.as_str()))?;

    let mut inventory = inventory_query
        .get_mut(session.client_entity)
        .map_err(|_| {
            format!(
                "player inventory missing on entity {:?}",
                session.client_entity
            )
        })?;

    add_item_to_player_inventory(&mut inventory, item_registry, allocator, kind.item_id, 1)?;

    let xp = match session.mode {
        BotanyHarvestMode::Manual => MANUAL_SKILL_XP,
        BotanyHarvestMode::Auto => AUTO_SKILL_XP,
    };
    let _ = store.add_skill_xp(player_id, xp);

    snapshot_queue.enqueue(session.client_entity);
    Ok(())
}

#[allow(dead_code)]
pub fn queue_harvest_inventory_snapshot(
    queue: &mut InventorySnapshotPushQueue,
    client_entity: Entity,
) {
    queue.enqueue(client_entity);
}

#[allow(clippy::too_many_arguments)]
pub fn tick_harvest_sessions(
    gameplay_tick: Option<Res<crate::player::gameplay::GameplayTick>>,
    mut store: ResMut<HarvestSessionStore>,
    mut plants: Query<&mut Plant, With<Plant>>,
    mut inventories: Query<&mut PlayerInventory, With<valence::prelude::Client>>,
    kind_registry: Res<BotanyKindRegistry>,
    item_registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut snapshot_queue: ResMut<InventorySnapshotPushQueue>,
    mut static_points: ResMut<PlantStaticPointStore>,
) {
    let Some(gameplay_tick) = gameplay_tick else {
        return;
    };

    let now = gameplay_tick.current_tick();
    let completed = store
        .iter()
        .filter(|session| session.progress_at(now) >= 1.0)
        .map(|session| session.player_id.clone())
        .collect::<Vec<_>>();

    for player_id in completed {
        let _ = complete_harvest_for_player(
            &mut store,
            player_id.as_str(),
            &mut plants,
            &mut inventories,
            kind_registry.as_ref(),
            item_registry.as_ref(),
            &mut allocator,
            &mut snapshot_queue,
            &mut static_points,
            now,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_progress_completes_after_duration() {
        let mut store = HarvestSessionStore::default();
        start_or_resume_harvest(
            &mut store,
            "Azure",
            Entity::from_raw(1),
            Some(Entity::from_raw(2)),
            BotanyPlantId::CiSheHao,
            BotanyHarvestMode::Manual,
            10,
        );

        let session = store.session_for("offline:Azure").unwrap();
        assert!(session.progress_at(51) >= 1.0);
    }
}
