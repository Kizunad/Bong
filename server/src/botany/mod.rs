pub mod components;
pub mod ecology;
pub mod events;
pub mod harvest;
pub mod integration;
pub mod lifecycle;
pub mod registry;

use valence::prelude::{App, EventReader, IntoSystemConfigs, Query, Res, Startup, Update, With};

use crate::inventory::{InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use components::{
    BotanyHarvestMode, BotanySkillChangedEvent, BotanyTrampleRoll, BotanyVariantRoll,
    HarvestSessionStore, HarvestTerminalEvent, InventorySnapshotRequestEvent, Plant,
    PlantLifecycleClock, PlantProximityTracker, PlantStaticPointStore,
};
use ecology::emit_botany_ecology_snapshot;
use events::{spawn_event_triggered_plants_on_death, BotanyEventSpawnRoll};
use harvest::{
    detect_non_session_trample, enforce_harvest_session_constraints, tick_harvest_sessions,
};
use lifecycle::{initialize_static_points_from_zones, run_botany_lifecycle_tick};
use registry::BotanyKindRegistry;

pub fn register(app: &mut App) {
    app.insert_resource(BotanyKindRegistry::default());
    app.insert_resource(PlantLifecycleClock::default());
    app.insert_resource(PlantStaticPointStore::default());
    app.insert_resource(HarvestSessionStore::default());
    app.insert_resource(BotanyTrampleRoll::default());
    app.insert_resource(BotanyEventSpawnRoll::default());
    app.insert_resource(PlantProximityTracker::default());
    app.insert_resource(BotanyVariantRoll::default());

    // plan §4 要求的跨系统异步通信：使用 bevy Event 模式，EventWriter 写、EventReader 读。
    app.add_event::<InventorySnapshotRequestEvent>();
    app.add_event::<HarvestTerminalEvent>();
    app.add_event::<BotanySkillChangedEvent>();

    app.add_systems(
        Update,
        (
            initialize_static_points_from_zones,
            spawn_event_triggered_plants_on_death,
            detect_non_session_trample,
            run_botany_lifecycle_tick,
            enforce_harvest_session_constraints,
            tick_harvest_sessions,
            emit_botany_inventory_snapshots,
            emit_botany_harvest_progress,
            emit_botany_skill,
            emit_botany_ecology_snapshot,
        )
            .chain(),
    );
    app.add_systems(Startup, validate_botany_inventory_primitives_on_startup);
}

fn validate_botany_inventory_primitives_on_startup(
    item_registry: valence::prelude::Res<ItemRegistry>,
    kind_registry: valence::prelude::Res<BotanyKindRegistry>,
    mut allocator: valence::prelude::ResMut<InventoryInstanceIdAllocator>,
) {
    if let Err(error) = ensure_botany_inventory_primitives(
        item_registry.as_ref(),
        kind_registry.as_ref(),
        &mut allocator,
    ) {
        panic!("[bong][botany] inventory primitive validation failed: {error}");
    }
}

type BotanyInventoryEmitQueryItem<'a> = (
    &'a mut valence::prelude::Client,
    &'a crate::player::state::PlayerState,
    &'a PlayerInventory,
);

fn emit_botany_inventory_snapshots(
    mut events: EventReader<InventorySnapshotRequestEvent>,
    mut clients: Query<BotanyInventoryEmitQueryItem<'_>, With<valence::prelude::Client>>,
) {
    use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
    use crate::network::inventory_snapshot_emit::build_inventory_snapshot;

    let pending: Vec<_> = events.read().copied().collect();
    if pending.is_empty() {
        return;
    }

    for target in pending {
        let Ok((mut client, player_state, inventory)) = clients.get_mut(target.client_entity)
        else {
            continue;
        };

        let snapshot = build_inventory_snapshot(inventory, player_state);
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

/// 每 tick 把 active session 进度发给 owning client；enforce / tick 阶段入队的终结帧在同 tick 同通道送出。
/// plan §1.3 channel = `bong:botany/harvest_progress`。
fn emit_botany_harvest_progress(
    gameplay_tick: Option<Res<crate::player::gameplay::GameplayTick>>,
    store: Res<HarvestSessionStore>,
    mut terminal_events: EventReader<HarvestTerminalEvent>,
    mut clients: Query<&mut valence::prelude::Client, With<valence::prelude::Client>>,
    plants: Query<&Plant, With<Plant>>,
) {
    use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};

    let now_tick = gameplay_tick.map(|t| t.current_tick()).unwrap_or(0);

    // Active sessions：持续推送进度
    for session in store.iter() {
        let Ok(mut client) = clients.get_mut(session.client_entity) else {
            continue;
        };
        let progress = session.progress_at(now_tick);
        let target_pos = session
            .target_entity
            .and_then(|entity| plants.get(entity).ok().map(|plant| plant.position));
        let payload = ServerDataV1::new(ServerDataPayloadV1::BotanyHarvestProgress {
            session_id: session.player_id.clone(),
            target_id: session
                .target_entity
                .map(|e| format!("plant-{}", e.to_bits()))
                .unwrap_or_default(),
            target_name: session.target_plant.as_str().to_string(),
            plant_kind: session.target_plant.as_str().to_string(),
            mode: harvest_mode_wire(session.mode).to_string(),
            progress: f64::from(progress),
            auto_selectable: true,
            request_pending: false,
            interrupted: false,
            completed: false,
            detail: String::new(),
            target_pos,
        });
        let payload_type = payload_type_label(payload.payload_type());
        match serialize_server_data_payload(&payload) {
            Ok(bytes) => send_server_data_payload(&mut client, bytes.as_slice()),
            Err(error) => log_payload_build_error(payload_type, &error),
        }
    }

    // Terminal 事件：一次性送出（完成或打断）
    for frame in terminal_events.read() {
        let Ok(mut client) = clients.get_mut(frame.client_entity) else {
            continue;
        };
        let payload = ServerDataV1::new(ServerDataPayloadV1::BotanyHarvestProgress {
            session_id: frame.session_id.clone(),
            target_id: frame.target_id.clone(),
            target_name: frame.target_name.clone(),
            plant_kind: frame.plant_kind.clone(),
            mode: harvest_mode_wire(frame.mode).to_string(),
            progress: if frame.completed { 1.0 } else { 0.0 },
            auto_selectable: true,
            request_pending: false,
            interrupted: frame.interrupted,
            completed: frame.completed,
            detail: frame.detail.clone(),
            target_pos: frame.target_pos,
        });
        let payload_type = payload_type_label(payload.payload_type());
        match serialize_server_data_payload(&payload) {
            Ok(bytes) => send_server_data_payload(&mut client, bytes.as_slice()),
            Err(error) => log_payload_build_error(payload_type, &error),
        }
    }
}

/// 采药技能变化后推送 BotanySkill snapshot；仅在 add_skill_xp 路径 send 事件，避免每 tick 冗发。
fn emit_botany_skill(
    mut events: EventReader<BotanySkillChangedEvent>,
    mut clients: Query<&mut valence::prelude::Client, With<valence::prelude::Client>>,
) {
    use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};

    for frame in events.read() {
        let Ok(mut client) = clients.get_mut(frame.client_entity) else {
            continue;
        };
        let state = frame.state;
        let xp_to_next_level = xp_to_next_level_for(state.level);
        let payload = ServerDataV1::new(ServerDataPayloadV1::BotanySkill {
            level: u64::from(state.level),
            xp: state.xp,
            xp_to_next_level,
            auto_unlock_level: u64::from(state.auto_unlock_level),
        });
        let payload_type = payload_type_label(payload.payload_type());
        match serialize_server_data_payload(&payload) {
            Ok(bytes) => send_server_data_payload(&mut client, bytes.as_slice()),
            Err(error) => log_payload_build_error(payload_type, &error),
        }
    }
}

fn harvest_mode_wire(mode: BotanyHarvestMode) -> &'static str {
    match mode {
        BotanyHarvestMode::Manual => "manual",
        BotanyHarvestMode::Auto => "auto",
    }
}

// add_skill_xp 当前实现：level = (xp/100)+1，饱和到 u8::MAX。单级固定 100 XP。
fn xp_to_next_level_for(level: u8) -> u64 {
    u64::from(level).saturating_mul(100)
}

/// 启动期全量校验：每个 BotanyPlantKind 的 item_id 必须在 ItemRegistry 里有模板，
/// 否则将来 harvest 那一刻才 panic。这里统一 fail-fast，便于排查。
pub fn ensure_botany_inventory_primitives(
    item_registry: &ItemRegistry,
    kind_registry: &BotanyKindRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
) -> Result<(), String> {
    let mut missing: Vec<&'static str> = Vec::new();
    for kind in kind_registry.iter() {
        if item_registry.get(kind.item_id).is_none() {
            missing.push(kind.item_id);
        }
    }
    if !missing.is_empty() {
        missing.sort_unstable();
        return Err(format!(
            "missing canonical herb template(s) in ItemRegistry: [{}]",
            missing.join(", ")
        ));
    }
    let _ = allocator.next_id()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_primitives_accepts_all_registered_kinds() {
        use crate::inventory::{load_item_registry, InventoryInstanceIdAllocator};

        let item_registry =
            load_item_registry().expect("shipped assets must produce valid ItemRegistry");
        let kind_registry = BotanyKindRegistry::default();
        let mut allocator = InventoryInstanceIdAllocator::default();

        let result =
            ensure_botany_inventory_primitives(&item_registry, &kind_registry, &mut allocator);
        assert!(
            result.is_ok(),
            "startup validation should pass for shipped assets, got {result:?}"
        );
    }

    #[test]
    fn ensure_primitives_fails_when_kind_item_missing() {
        use crate::inventory::{InventoryInstanceIdAllocator, ItemRegistry};

        let item_registry = ItemRegistry::default();
        let kind_registry = BotanyKindRegistry::default();
        let mut allocator = InventoryInstanceIdAllocator::default();

        let err =
            ensure_botany_inventory_primitives(&item_registry, &kind_registry, &mut allocator)
                .expect_err("empty ItemRegistry must fail");
        assert!(
            err.contains("ci_she_hao"),
            "error should list missing kinds, got `{err}`"
        );
        assert!(
            err.contains("kong_shou_hen"),
            "error should list kong_shou_hen, got `{err}`"
        );
    }

    #[test]
    fn module_registers_botany_resources() {
        let mut app = App::new();
        register(&mut app);

        use valence::prelude::Events;

        assert!(app.world().contains_resource::<BotanyKindRegistry>());
        assert!(app.world().contains_resource::<PlantLifecycleClock>());
        assert!(app.world().contains_resource::<PlantStaticPointStore>());
        assert!(app.world().contains_resource::<HarvestSessionStore>());
        assert!(app.world().contains_resource::<BotanyTrampleRoll>());
        assert!(app.world().contains_resource::<BotanyEventSpawnRoll>());
        assert!(app.world().contains_resource::<PlantProximityTracker>());
        assert!(app.world().contains_resource::<BotanyVariantRoll>());
        // bevy Events 注册后会生成 `Events<T>` resource
        assert!(app
            .world()
            .contains_resource::<Events<InventorySnapshotRequestEvent>>());
        assert!(app
            .world()
            .contains_resource::<Events<HarvestTerminalEvent>>());
        assert!(app
            .world()
            .contains_resource::<Events<BotanySkillChangedEvent>>());
    }
}
