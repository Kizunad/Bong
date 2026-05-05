use std::collections::HashMap;

use valence::prelude::{
    bevy_ecs, Client, Entity, ParamSet, Position, Query, RemovedComponents, Res, ResMut, Resource,
    With,
};

use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::dugu::{DuguObfuscationDisrupted, DuguPractice};
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::realm_taint::{RealmTaintState, RealmTaintedKind};
use crate::cultivation::tick::CultivationClock;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::realm_vision::{SenseEntryV1, SpiritualSenseTargetsV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::spirit_eye::SpiritEyeRegistry;

use super::scanner::{
    scan_targets_inner_ring, scan_targets_mid_ring_void, SpiritualSenseTarget,
    SpiritualSenseTargetKind, StealthState,
};
use super::throttle::{should_scan, SpiritualSenseRing};

const MAX_TARGETS_PER_PAYLOAD: usize = 24;

#[derive(Default, Resource)]
pub struct SpiritualSensePushState {
    last_inner_scan_tick: HashMap<Entity, u64>,
    last_middle_scan_tick: HashMap<Entity, u64>,
    inner_entries: HashMap<Entity, Vec<SenseEntryV1>>,
    middle_entries: HashMap<Entity, Vec<SenseEntryV1>>,
    last_payload_was_empty: HashMap<Entity, bool>,
}

#[derive(Debug, Clone, Copy)]
struct PlayerSenseSnapshot {
    entity: Entity,
    position: [f64; 3],
    realm: Realm,
    stealth: Option<StealthState>,
    niche_intrusion_trace: Option<NicheIntrusionTrace>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct NicheIntrusionTrace {
    severity: f64,
}

type SpiritualSensePlayerReadItem<'a> = (
    Entity,
    &'a Position,
    &'a Cultivation,
    Option<&'a DuguPractice>,
    Option<&'a DuguObfuscationDisrupted>,
    Option<&'a RealmTaintState>,
);
type SpiritualSensePlayerReadFilter = With<Client>;
type SpiritualSenseObserverItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Position,
    Option<&'a CurrentDimension>,
    &'a Cultivation,
    &'a LifeRecord,
);
type SpiritualSenseObserverFilter = With<Client>;
type SpiritualSenseQueryParams<'w, 's> = (
    Query<'w, 's, SpiritualSensePlayerReadItem<'w>, SpiritualSensePlayerReadFilter>,
    Query<'w, 's, SpiritualSenseObserverItem<'w>, SpiritualSenseObserverFilter>,
);

pub fn send_spiritual_sense_targets(client: &mut Client, targets: SpiritualSenseTargetsV1) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::SpiritualSenseTargets(targets));
    let payload_type = payload_type_label(payload.payload_type());
    let bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(client, bytes.as_slice());
    tracing::debug!(
        "[bong][spiritual_sense] sent {} {} payload",
        SERVER_DATA_CHANNEL,
        payload_type
    );
}

pub fn push_empty_spiritual_sense_targets(mut clients: Query<&mut Client, With<Client>>) {
    for mut client in &mut clients {
        send_spiritual_sense_targets(
            &mut client,
            SpiritualSenseTargetsV1 {
                entries: Vec::new(),
                generation: 0,
            },
        );
    }
}

pub fn push_spiritual_sense_targets(
    clock: Res<CultivationClock>,
    mut state: ResMut<SpiritualSensePushState>,
    spirit_eyes: Option<Res<SpiritEyeRegistry>>,
    mut player_sets: ParamSet<SpiritualSenseQueryParams<'_, '_>>,
) {
    let now_tick = clock.tick;
    let snapshots: Vec<PlayerSenseSnapshot> = {
        let players = player_sets.p0();
        players
            .iter()
            .map(
                |(entity, position, cultivation, dugu_practice, dugu_disrupted, realm_taint)| {
                    PlayerSenseSnapshot {
                        entity,
                        position: position_to_array(position),
                        realm: cultivation.realm,
                        stealth: dugu_practice
                            .is_some_and(|practice| practice.dugu_practice_level >= 1)
                            .then_some(StealthState {
                                active: true,
                                disrupted: dugu_disrupted.is_some(),
                            }),
                        niche_intrusion_trace: niche_intrusion_trace(now_tick, realm_taint),
                    }
                },
            )
            .collect()
    };
    if snapshots.is_empty() {
        return;
    }

    let mut observers = player_sets.p1();
    for (entity, mut client, position, current_dimension, cultivation, life_record) in
        &mut observers
    {
        let should_scan_inner = should_scan(
            now_tick,
            state.last_inner_scan_tick.get(&entity).copied(),
            SpiritualSenseRing::Inner,
        );
        let should_scan_middle = cultivation.realm == Realm::Void
            && should_scan(
                now_tick,
                state.last_middle_scan_tick.get(&entity).copied(),
                SpiritualSenseRing::Middle,
            );
        if !should_scan_inner && !should_scan_middle {
            continue;
        }

        let observer_pos = position_to_array(position);
        let observer_dimension = current_dimension
            .map(|current| current.0)
            .unwrap_or(DimensionKind::Overworld);
        if should_scan_inner {
            let radius = super::scanner::scan_radius_for_realm(cultivation.realm);
            let mut targets =
                build_player_sense_targets(entity, observer_pos, &snapshots, 0.0, radius);
            targets.extend(build_niche_intrusion_trace_targets(
                entity,
                observer_pos,
                &snapshots,
            ));
            let entries = scan_targets_inner_ring(observer_pos, cultivation.realm, &targets);
            state.inner_entries.insert(entity, entries);
            state.last_inner_scan_tick.insert(entity, now_tick);
        }
        if cultivation.realm == Realm::Void {
            if should_scan_middle {
                let targets =
                    build_player_sense_targets(entity, observer_pos, &snapshots, 500.0, 2000.0);
                let entries = scan_targets_mid_ring_void(observer_pos, &targets);
                state.middle_entries.insert(entity, entries);
                state.last_middle_scan_tick.insert(entity, now_tick);
            }
        } else {
            state.middle_entries.remove(&entity);
            state.last_middle_scan_tick.insert(entity, now_tick);
        }

        let mut entries = merged_cached_entries(&state, entity);
        if let Some(registry) = spirit_eyes.as_deref() {
            entries.extend(registry.private_marker_entries(
                life_record.character_id.as_str(),
                observer_dimension,
                observer_pos,
            ));
        }
        trim_entries_by_intensity(&mut entries);
        if entries.is_empty()
            && state
                .last_payload_was_empty
                .get(&entity)
                .copied()
                .unwrap_or(false)
        {
            continue;
        }
        state
            .last_payload_was_empty
            .insert(entity, entries.is_empty());
        send_spiritual_sense_targets(
            &mut client,
            SpiritualSenseTargetsV1 {
                entries,
                generation: now_tick,
            },
        );
    }
}

pub fn cleanup_spiritual_sense_push_state(
    mut removed_clients: RemovedComponents<Client>,
    mut state: ResMut<SpiritualSensePushState>,
) {
    for entity in removed_clients.read() {
        state.remove_entity(entity);
    }
}

fn build_player_sense_targets(
    observer: Entity,
    observer_pos: [f64; 3],
    players: &[PlayerSenseSnapshot],
    min_distance: f64,
    max_distance: f64,
) -> Vec<SpiritualSenseTarget> {
    players
        .iter()
        .filter(|target| target.entity != observer)
        .map(|target| SpiritualSenseTarget {
            position: target.position,
            kind: SpiritualSenseTargetKind::Cultivator(target.realm),
            intensity: distance_intensity(
                observer_pos,
                target.position,
                min_distance,
                max_distance,
            ),
            stealth: target.stealth,
        })
        .collect()
}

fn build_niche_intrusion_trace_targets(
    observer: Entity,
    observer_pos: [f64; 3],
    players: &[PlayerSenseSnapshot],
) -> Vec<SpiritualSenseTarget> {
    players
        .iter()
        .filter(|target| target.entity != observer)
        .filter_map(|target| {
            let trace = target.niche_intrusion_trace?;
            let max_distance = niche_intrusion_trace_radius(trace.severity);
            if distance_between(observer_pos, target.position) > max_distance {
                return None;
            }
            Some(SpiritualSenseTarget {
                position: target.position,
                kind: SpiritualSenseTargetKind::NicheIntrusionTrace,
                intensity: distance_intensity(observer_pos, target.position, 0.0, max_distance)
                    * trace.severity.clamp(0.2, 1.0),
                stealth: None,
            })
        })
        .collect()
}

fn niche_intrusion_trace(
    now_tick: u64,
    taint: Option<&RealmTaintState>,
) -> Option<NicheIntrusionTrace> {
    let taint = taint?;
    if taint.kind != RealmTaintedKind::NicheIntrusion
        || taint.qi_taint_severity <= 0.0
        || now_tick >= taint.wash_available_at
    {
        return None;
    }
    Some(NicheIntrusionTrace {
        severity: f64::from(taint.qi_taint_severity.clamp(0.0, 1.0)),
    })
}

fn niche_intrusion_trace_radius(severity: f64) -> f64 {
    let severity = severity.clamp(0.0, 1.0);
    50.0 + ((severity - 0.2).max(0.0) / 0.8) * 150.0
}

fn merged_cached_entries(state: &SpiritualSensePushState, entity: Entity) -> Vec<SenseEntryV1> {
    let mut entries = state
        .inner_entries
        .get(&entity)
        .cloned()
        .unwrap_or_default();
    if let Some(middle) = state.middle_entries.get(&entity) {
        entries.extend(middle.iter().cloned());
    }
    entries
}

impl SpiritualSensePushState {
    fn remove_entity(&mut self, entity: Entity) {
        self.last_inner_scan_tick.remove(&entity);
        self.last_middle_scan_tick.remove(&entity);
        self.inner_entries.remove(&entity);
        self.middle_entries.remove(&entity);
        self.last_payload_was_empty.remove(&entity);
    }
}

fn trim_entries_by_intensity(entries: &mut Vec<SenseEntryV1>) {
    entries.sort_by(|a, b| b.intensity.total_cmp(&a.intensity));
    entries.truncate(MAX_TARGETS_PER_PAYLOAD);
}

fn distance_intensity(a: [f64; 3], b: [f64; 3], min_distance: f64, max_distance: f64) -> f64 {
    let distance = distance_between(a, b);
    let range = max_distance - min_distance;
    if range <= 0.0 {
        return 1.0;
    }
    (1.0 - (distance - min_distance).max(0.0) / range).clamp(0.1, 1.0)
}

fn distance_between(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn position_to_array(position: &Position) -> [f64; 3] {
    let p = position.get();
    [p.x, p.y, p.z]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::realm_vision::{SenseEntryV1, SenseKindV1};
    use valence::prelude::DVec3;

    #[test]
    fn server_data_v1_targets_variant() {
        let payload = ServerDataV1::new(ServerDataPayloadV1::SpiritualSenseTargets(
            SpiritualSenseTargetsV1 {
                generation: 2,
                entries: vec![SenseEntryV1 {
                    kind: SenseKindV1::LivingQi,
                    x: 1.0,
                    y: 64.0,
                    z: 2.0,
                    intensity: 0.5,
                }],
            },
        ));
        let value: serde_json::Value = serde_json::to_value(payload).expect("serialize");
        assert_eq!(
            value.get("type").and_then(|v| v.as_str()),
            Some("spiritual_sense_targets")
        );
        assert_eq!(value.get("generation").and_then(|v| v.as_u64()), Some(2));
    }

    #[test]
    fn distance_intensity_falls_with_distance() {
        let near = distance_intensity([0.0, 64.0, 0.0], [50.0, 64.0, 0.0], 0.0, 500.0);
        let far = distance_intensity([0.0, 64.0, 0.0], [450.0, 64.0, 0.0], 0.0, 500.0);
        assert!(near > far);
        assert_eq!(far, 0.1);
    }

    #[test]
    fn build_player_targets_excludes_observer() {
        let observer = Entity::from_raw(1);
        let other = Entity::from_raw(2);
        let players = vec![
            PlayerSenseSnapshot {
                entity: observer,
                position: [0.0, 64.0, 0.0],
                realm: Realm::Induce,
                stealth: None,
                niche_intrusion_trace: None,
            },
            PlayerSenseSnapshot {
                entity: other,
                position: [10.0, 64.0, 0.0],
                realm: Realm::Condense,
                stealth: Some(StealthState {
                    active: true,
                    disrupted: false,
                }),
                niche_intrusion_trace: None,
            },
        ];
        let targets = build_player_sense_targets(observer, [0.0, 64.0, 0.0], &players, 0.0, 500.0);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].position, [10.0, 64.0, 0.0]);
        assert_eq!(
            targets[0].kind,
            SpiritualSenseTargetKind::Cultivator(Realm::Condense)
        );
        assert_eq!(
            targets[0].stealth,
            Some(StealthState {
                active: true,
                disrupted: false,
            })
        );

        let position = Position(DVec3::new(1.0, 2.0, 3.0));
        assert_eq!(position_to_array(&position), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn niche_intrusion_trace_targets_scale_with_severity_and_window() {
        let observer = Entity::from_raw(1);
        let traced = Entity::from_raw(2);
        let expired = Entity::from_raw(3);
        let mut active_taint = RealmTaintState::default();
        active_taint.add_niche_intrusion(0.2, 10);
        let mut expired_taint = RealmTaintState::default();
        expired_taint.add_niche_intrusion(1.0, 10);

        assert_eq!(niche_intrusion_trace_radius(0.2), 50.0);
        assert_eq!(niche_intrusion_trace_radius(1.0), 200.0);
        assert!(niche_intrusion_trace(11, Some(&active_taint)).is_some());
        assert!(
            niche_intrusion_trace(active_taint.wash_available_at, Some(&active_taint)).is_none()
        );

        let players = vec![
            PlayerSenseSnapshot {
                entity: observer,
                position: [0.0, 64.0, 0.0],
                realm: Realm::Solidify,
                stealth: None,
                niche_intrusion_trace: None,
            },
            PlayerSenseSnapshot {
                entity: traced,
                position: [45.0, 64.0, 0.0],
                realm: Realm::Induce,
                stealth: None,
                niche_intrusion_trace: niche_intrusion_trace(11, Some(&active_taint)),
            },
            PlayerSenseSnapshot {
                entity: expired,
                position: [40.0, 64.0, 0.0],
                realm: Realm::Induce,
                stealth: None,
                niche_intrusion_trace: niche_intrusion_trace(
                    expired_taint.wash_available_at,
                    Some(&expired_taint),
                ),
            },
        ];

        let targets = build_niche_intrusion_trace_targets(observer, [0.0, 64.0, 0.0], &players);
        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].kind,
            SpiritualSenseTargetKind::NicheIntrusionTrace
        );
        assert_eq!(targets[0].position, [45.0, 64.0, 0.0]);
    }

    #[test]
    fn cached_ring_entries_merge_and_cleanup() {
        let entity = Entity::from_raw(7);
        let mut state = SpiritualSensePushState::default();
        state.inner_entries.insert(
            entity,
            vec![SenseEntryV1 {
                kind: SenseKindV1::LivingQi,
                x: 1.0,
                y: 64.0,
                z: 1.0,
                intensity: 0.5,
            }],
        );
        state.middle_entries.insert(
            entity,
            vec![SenseEntryV1 {
                kind: SenseKindV1::CultivatorRealm,
                x: 900.0,
                y: 64.0,
                z: 0.0,
                intensity: 0.8,
            }],
        );
        assert_eq!(merged_cached_entries(&state, entity).len(), 2);

        state.last_inner_scan_tick.insert(entity, 10);
        state.last_middle_scan_tick.insert(entity, 20);
        state.last_payload_was_empty.insert(entity, false);
        state.remove_entity(entity);

        assert!(merged_cached_entries(&state, entity).is_empty());
        assert!(!state.last_inner_scan_tick.contains_key(&entity));
        assert!(!state.last_middle_scan_tick.contains_key(&entity));
        assert!(!state.last_payload_was_empty.contains_key(&entity));
    }

    #[test]
    fn middle_ring_intensity_uses_middle_ring_span() {
        let near_middle = distance_intensity([0.0, 64.0, 0.0], [600.0, 64.0, 0.0], 500.0, 2000.0);
        let far_middle = distance_intensity([0.0, 64.0, 0.0], [1900.0, 64.0, 0.0], 500.0, 2000.0);
        assert!(near_middle > far_middle);
        assert!(near_middle > 0.8);
        assert_eq!(far_middle, 0.1);
    }
}
