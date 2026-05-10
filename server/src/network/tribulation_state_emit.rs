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
    mut clients: Query<&mut Client, With<Client>>,
    mut announce: EventReader<TribulationAnnounce>,
    mut juebi_triggered: EventReader<JueBiTriggeredEvent>,
    mut locked: EventReader<TribulationLocked>,
    mut cleared: EventReader<TribulationWaveCleared>,
    mut settled: EventReader<TribulationSettled>,
    states: Query<(&TribulationState, Option<&Lifecycle>, Option<&Username>)>,
    mut last_client_count: Local<Option<usize>>,
) {
    let client_count = clients.iter_mut().count();
    let count_changed = last_client_count.replace(client_count) != Some(client_count);
    let mut emitted = false;

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
            |(state, lifecycle, username)| {
                snapshot_from_state(state, lifecycle, username, ev.entity)
            },
        );
        broadcast(&mut clients, data);
        emitted = true;
    }

    for ev in juebi_triggered.read() {
        let Ok((state, lifecycle, username)) = states.get(ev.entity) else {
            continue;
        };
        broadcast(
            &mut clients,
            snapshot_from_state(state, lifecycle, username, ev.entity),
        );
        emitted = true;
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
            |(state, lifecycle, username)| {
                snapshot_from_state(state, lifecycle, username, ev.entity)
            },
        );
        broadcast(&mut clients, data);
        emitted = true;
    }

    for ev in cleared.read() {
        let Ok((state, lifecycle, username)) = states.get(ev.entity) else {
            continue;
        };
        broadcast(
            &mut clients,
            snapshot_from_state(state, lifecycle, username, ev.entity),
        );
        emitted = true;
    }

    for ev in settled.read() {
        let mut data = TribulationStateV1::clear();
        data.char_id = ev.result.char_id.clone();
        data.actor_name = ev.result.char_id.clone();
        data.wave_current = ev.result.waves_survived;
        data.result = Some(outcome_label(ev.result.outcome).to_string());
        broadcast(&mut clients, data);
        emitted = true;
    }

    if count_changed && !emitted && client_count > 0 {
        for (state, lifecycle, username) in &states {
            broadcast(
                &mut clients,
                snapshot_from_state(state, lifecycle, username, Entity::PLACEHOLDER),
            );
        }
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

fn broadcast(clients: &mut Query<&mut Client, With<Client>>, data: TribulationStateV1) {
    for mut client in clients.iter_mut() {
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
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use crate::schema::tribulation::DuXuResultV1;
    use valence::prelude::{App, Events, Update, Username};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn spawn_mock_client(app: &mut App, name: &str) -> MockClientHelper {
        let (bundle, helper) = create_mock_client(name);
        app.world_mut().spawn(bundle);
        helper
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
}
