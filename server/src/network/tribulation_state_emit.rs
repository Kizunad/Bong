use std::collections::HashSet;

use valence::prelude::{Client, Entity, EventReader, Local, Query, Username, With};

use crate::combat::components::Lifecycle;
use crate::cultivation::tribulation::{
    JueBiTriggeredEvent, TribulationAnnounce, TribulationKind, TribulationLocked, TribulationPhase,
    TribulationSettled, TribulationState, TribulationWaveCleared,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1, TribulationStateV1};
use crate::schema::tribulation::DuXuOutcomeV1;

#[allow(clippy::too_many_arguments)]
pub fn emit_tribulation_state_payloads(
    mut clients: Query<(Entity, &mut Client), With<Client>>,
    mut announce: EventReader<TribulationAnnounce>,
    mut juebi_triggered: EventReader<JueBiTriggeredEvent>,
    mut locked: EventReader<TribulationLocked>,
    mut cleared: EventReader<TribulationWaveCleared>,
    mut settled: EventReader<TribulationSettled>,
    states: Query<(
        Entity,
        &TribulationState,
        Option<&Lifecycle>,
        Option<&Username>,
    )>,
    mut known_clients: Local<Option<HashSet<Entity>>>,
) {
    let current_clients: HashSet<Entity> = clients.iter_mut().map(|(entity, _)| entity).collect();
    let joined_clients: HashSet<Entity> = known_clients.as_ref().map_or_else(
        || current_clients.clone(),
        |previous| current_clients.difference(previous).copied().collect(),
    );
    *known_clients = Some(current_clients);
    let mut emitted_entities = HashSet::new();

    for ev in announce.read() {
        let data = states.get(ev.entity).ok().map_or_else(
            || TribulationStateV1 {
                active: true,
                char_id: ev.char_id.clone(),
                actor_name: ev.actor_name.clone(),
                kind: "du_xu".to_string(),
                phase: "omen".to_string(),
                world_x: ev.epicenter[0],
                world_z: ev.epicenter[2],
                wave_current: 0,
                wave_total: ev.waves_total,
                started_tick: 0,
                phase_started_tick: 0,
                next_wave_tick: 0,
                failed: false,
                half_step_on_success: false,
                participants: vec![ev.char_id.clone()],
                result: None,
            },
            |(_, state, lifecycle, username)| {
                snapshot_from_state(state, lifecycle, username, ev.entity)
            },
        );
        broadcast(&mut clients, data);
        emitted_entities.insert(ev.entity);
    }

    for ev in juebi_triggered.read() {
        let data = match states.get(ev.entity) {
            Ok((_, state, lifecycle, username)) => {
                snapshot_from_state(state, lifecycle, username, ev.entity)
            }
            Err(_) => snapshot_from_juebi_event(ev),
        };
        broadcast(&mut clients, data);
        emitted_entities.insert(ev.entity);
    }

    for ev in locked.read() {
        let data = states.get(ev.entity).ok().map_or_else(
            || TribulationStateV1 {
                active: true,
                char_id: ev.char_id.clone(),
                actor_name: ev.actor_name.clone(),
                kind: "du_xu".to_string(),
                phase: "lock".to_string(),
                world_x: ev.epicenter[0],
                world_z: ev.epicenter[2],
                wave_current: 0,
                wave_total: ev.waves_total,
                started_tick: 0,
                phase_started_tick: 0,
                next_wave_tick: 0,
                failed: false,
                half_step_on_success: false,
                participants: vec![ev.char_id.clone()],
                result: None,
            },
            |(_, state, lifecycle, username)| {
                snapshot_from_state(state, lifecycle, username, ev.entity)
            },
        );
        broadcast(&mut clients, data);
        emitted_entities.insert(ev.entity);
    }

    for ev in cleared.read() {
        let Ok((_, state, lifecycle, username)) = states.get(ev.entity) else {
            continue;
        };
        broadcast(
            &mut clients,
            snapshot_from_state(state, lifecycle, username, ev.entity),
        );
        emitted_entities.insert(ev.entity);
    }

    for ev in settled.read() {
        let mut data = TribulationStateV1::clear();
        data.char_id = ev.result.char_id.clone();
        data.actor_name = ev.result.char_id.clone();
        data.wave_current = ev.result.waves_survived;
        data.result = Some(outcome_label(ev.result.outcome).to_string());
        broadcast(&mut clients, data);
        emitted_entities.insert(ev.entity);
    }

    if !joined_clients.is_empty() {
        for (entity, state, lifecycle, username) in &states {
            if emitted_entities.contains(&entity) {
                continue;
            }
            broadcast_to_clients(
                &mut clients,
                &joined_clients,
                snapshot_from_state(state, lifecycle, username, entity),
            );
        }
    }
}

fn snapshot_from_juebi_event(ev: &JueBiTriggeredEvent) -> TribulationStateV1 {
    TribulationStateV1 {
        active: true,
        char_id: ev.char_id.clone(),
        actor_name: ev.actor_name.clone(),
        kind: "jue_bi".to_string(),
        phase: "omen".to_string(),
        world_x: ev.epicenter[0],
        world_z: ev.epicenter[2],
        wave_current: 0,
        wave_total: ev.waves_total,
        started_tick: ev.started_tick,
        phase_started_tick: ev.started_tick,
        next_wave_tick: 0,
        failed: false,
        half_step_on_success: false,
        participants: vec![ev.char_id.clone()],
        result: None,
    }
}

fn snapshot_from_state(
    state: &TribulationState,
    lifecycle: Option<&Lifecycle>,
    username: Option<&Username>,
    entity: Entity,
) -> TribulationStateV1 {
    let char_id = lifecycle
        .map(|lifecycle| lifecycle.character_id.clone())
        .or_else(|| state.participants.first().cloned())
        .unwrap_or_else(|| format!("entity:{entity:?}"));
    let actor_name = username
        .map(|username| username.0.clone())
        .unwrap_or_else(|| char_id.clone());
    TribulationStateV1 {
        active: true,
        char_id,
        actor_name,
        kind: kind_label(state.kind).to_string(),
        phase: phase_label(state.phase).to_string(),
        world_x: state.epicenter[0],
        world_z: state.epicenter[2],
        wave_current: state.wave_current,
        wave_total: state.waves_total,
        started_tick: state.started_tick,
        phase_started_tick: state.phase_started_tick,
        next_wave_tick: state.next_wave_tick,
        failed: state.failed,
        half_step_on_success: false,
        participants: state.participants.clone(),
        result: None,
    }
}

fn broadcast(clients: &mut Query<(Entity, &mut Client), With<Client>>, data: TribulationStateV1) {
    broadcast_filtered(clients, None, data);
}

fn broadcast_to_clients(
    clients: &mut Query<(Entity, &mut Client), With<Client>>,
    target_clients: &HashSet<Entity>,
    data: TribulationStateV1,
) {
    broadcast_filtered(clients, Some(target_clients), data);
}

fn broadcast_filtered(
    clients: &mut Query<(Entity, &mut Client), With<Client>>,
    target_clients: Option<&HashSet<Entity>>,
    data: TribulationStateV1,
) {
    for (entity, mut client) in clients.iter_mut() {
        if target_clients.is_some_and(|targets| !targets.contains(&entity)) {
            continue;
        }
        let payload = ServerDataV1::new(ServerDataPayloadV1::TribulationState(data.clone()));
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

fn kind_label(kind: TribulationKind) -> &'static str {
    match kind {
        TribulationKind::DuXu => "du_xu",
        TribulationKind::ZoneCollapse => "zone_collapse",
        TribulationKind::Targeted => "targeted",
        TribulationKind::JueBi => "jue_bi",
    }
}

fn phase_label(phase: TribulationPhase) -> &'static str {
    match phase {
        TribulationPhase::Omen => "omen",
        TribulationPhase::Lock => "lock",
        TribulationPhase::Wave(_) => "wave",
        TribulationPhase::HeartDemon => "heart_demon",
        TribulationPhase::Settle => "settle",
    }
}

fn outcome_label(outcome: DuXuOutcomeV1) -> &'static str {
    match outcome {
        DuXuOutcomeV1::Ascended => "ascended",
        DuXuOutcomeV1::HalfStep => "half_step",
        DuXuOutcomeV1::Failed => "failed",
        DuXuOutcomeV1::Killed => "killed",
        DuXuOutcomeV1::Fled => "fled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::Lifecycle;
    use crate::cultivation::tribulation::JueBiTriggerSource;
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use crate::schema::tribulation::DuXuResultV1;
    use crate::world::dimension::DimensionKind;
    use valence::prelude::{App, Events, Update, Username};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn spawn_mock_client_entity(app: &mut App, name: &str) -> (Entity, MockClientHelper) {
        let (bundle, helper) = create_mock_client(name);
        let entity = app.world_mut().spawn(bundle).id();
        (entity, helper)
    }

    fn spawn_mock_client(app: &mut App, name: &str) -> MockClientHelper {
        spawn_mock_client_entity(app, name).1
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    fn collect_tribulation_states(helper: &mut MockClientHelper) -> Vec<TribulationStateV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("server data payload should decode");
            if let ServerDataPayloadV1::TribulationState(data) = payload.payload {
                payloads.push(data);
            }
        }
        payloads
    }

    #[test]
    fn wave_event_broadcasts_current_tribulation_state() {
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_systems(Update, emit_tribulation_state_payloads);
        let mut helper = spawn_mock_client(&mut app, "Azure");
        let entity = app
            .world_mut()
            .spawn((
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Lifecycle::default()
                },
                Username("Azure".to_string()),
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(2),
                    epicenter: [12.0, 66.0, -34.0],
                    wave_current: 2,
                    waves_total: 5,
                    started_tick: 100,
                    phase_started_tick: 300,
                    next_wave_tick: 600,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
            ))
            .id();
        app.world_mut()
            .resource_mut::<Events<TribulationWaveCleared>>()
            .send(TribulationWaveCleared { entity, wave: 2 });

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_tribulation_states(&mut helper);
        assert_eq!(payloads.len(), 1);
        assert!(payloads[0].active);
        assert_eq!(payloads[0].char_id, "offline:Azure");
        assert_eq!(payloads[0].actor_name, "Azure");
        assert_eq!(payloads[0].phase, "wave");
        assert_eq!(payloads[0].wave_current, 2);
        assert_eq!(payloads[0].wave_total, 5);
        assert!(!payloads[0].half_step_on_success);
    }

    #[test]
    fn settled_event_broadcasts_clear_state_with_result() {
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_systems(Update, emit_tribulation_state_payloads);
        let mut helper = spawn_mock_client(&mut app, "Azure");
        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(TribulationSettled {
                entity: Entity::PLACEHOLDER,
                kind: TribulationKind::DuXu,
                source: None,
                result: DuXuResultV1 {
                    char_id: "offline:Azure".to_string(),
                    outcome: DuXuOutcomeV1::Ascended,
                    killer: None,
                    waves_survived: 5,
                    reason: None,
                },
            });

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_tribulation_states(&mut helper);
        assert_eq!(payloads.len(), 1);
        assert!(!payloads[0].active);
        assert_eq!(payloads[0].char_id, "offline:Azure");
        assert_eq!(payloads[0].phase, "settle");
        assert_eq!(payloads[0].result.as_deref(), Some("ascended"));
    }

    #[test]
    fn juebi_trigger_emits_state_from_event_when_component_not_yet_visible() {
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_systems(Update, emit_tribulation_state_payloads);
        let mut helper = spawn_mock_client(&mut app, "Watcher");
        let entity = app.world_mut().spawn_empty().id();

        app.world_mut()
            .resource_mut::<Events<JueBiTriggeredEvent>>()
            .send(JueBiTriggeredEvent {
                entity,
                char_id: "offline:JueBiSource".to_string(),
                actor_name: "HiddenVoidActor".to_string(),
                source: JueBiTriggerSource::KarmaThreshold,
                epicenter: [301.0, 66.0, -301.0],
                dimension: DimensionKind::Overworld,
                waves_total: 4,
                started_tick: 100,
                intensity: 1.0,
            });

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_tribulation_states(&mut helper);
        assert_eq!(payloads.len(), 1);
        assert!(payloads[0].active);
        assert_eq!(payloads[0].char_id, "offline:JueBiSource");
        assert_eq!(payloads[0].actor_name, "HiddenVoidActor");
        assert_eq!(payloads[0].kind, "jue_bi");
        assert_eq!(payloads[0].phase, "omen");
        assert_eq!(payloads[0].world_x, 301.0);
        assert_eq!(payloads[0].world_z, -301.0);
        assert_eq!(payloads[0].wave_total, 4);
    }

    #[test]
    fn same_count_client_replacement_replays_active_states_to_new_entity() {
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_systems(Update, emit_tribulation_state_payloads);
        let mut stable_client = spawn_mock_client(&mut app, "Stable");
        let (departing_entity, mut departing_client) =
            spawn_mock_client_entity(&mut app, "Departing");

        app.world_mut().spawn((
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Lifecycle::default()
            },
            Username("Azure".to_string()),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(2),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 2,
                waves_total: 5,
                started_tick: 100,
                phase_started_tick: 300,
                next_wave_tick: 600,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
        ));

        app.update();
        flush_all_client_packets(&mut app);
        assert_eq!(collect_tribulation_states(&mut stable_client).len(), 1);
        assert_eq!(collect_tribulation_states(&mut departing_client).len(), 1);

        app.world_mut().entity_mut(departing_entity).despawn();
        let mut late_client = spawn_mock_client(&mut app, "Late");

        app.update();
        flush_all_client_packets(&mut app);

        assert!(
            collect_tribulation_states(&mut stable_client).is_empty(),
            "active state replay 应只补给新 client，不应刷屏既有 client"
        );
        let replayed = collect_tribulation_states(&mut late_client);
        assert_eq!(
            replayed.len(),
            1,
            "即使总 client 数不变，新 Entity client 也必须收到 active state replay"
        );
        assert!(replayed[0].active);
        assert_eq!(replayed[0].char_id, "offline:Azure");
    }

    #[test]
    fn new_client_join_replays_all_active_states_even_when_same_tick_emits_one() {
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_systems(Update, emit_tribulation_state_payloads);
        let mut first_client = spawn_mock_client(&mut app, "First");

        let first = app
            .world_mut()
            .spawn((
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Lifecycle::default()
                },
                Username("Azure".to_string()),
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(3),
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 3,
                    waves_total: 5,
                    started_tick: 100,
                    phase_started_tick: 300,
                    next_wave_tick: 600,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
            ))
            .id();
        let second = app
            .world_mut()
            .spawn((
                Lifecycle {
                    character_id: "offline:Beryl".to_string(),
                    ..Lifecycle::default()
                },
                Username("Beryl".to_string()),
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(1),
                    epicenter: [400.0, 66.0, 0.0],
                    wave_current: 1,
                    waves_total: 5,
                    started_tick: 100,
                    phase_started_tick: 300,
                    next_wave_tick: 600,
                    participants: vec!["offline:Beryl".to_string()],
                    failed: false,
                },
            ))
            .id();

        app.world_mut()
            .resource_mut::<Events<TribulationWaveCleared>>()
            .send(TribulationWaveCleared {
                entity: first,
                wave: 3,
            });
        app.update();
        flush_all_client_packets(&mut app);
        assert_eq!(collect_tribulation_states(&mut first_client).len(), 2);

        let mut late_client = spawn_mock_client(&mut app, "Late");
        app.world_mut()
            .resource_mut::<Events<TribulationWaveCleared>>()
            .send(TribulationWaveCleared {
                entity: second,
                wave: 1,
            });

        app.update();
        flush_all_client_packets(&mut app);

        let replayed = collect_tribulation_states(&mut late_client);
        assert_eq!(
            replayed.len(),
            2,
            "同 tick 增量 + join replay 后，新 client 应收敛到完整 active state 集且不重复"
        );
        assert!(
            replayed
                .iter()
                .any(|payload| payload.active && payload.char_id == "offline:Azure"),
            "同 tick 有 Beryl state emit 时，新 client 仍必须补收既有 Azure active state"
        );
        assert!(
            replayed
                .iter()
                .any(|payload| payload.active && payload.char_id == "offline:Beryl"),
            "新 client 应同时收到本 tick emit 的 Beryl state"
        );
    }
}
