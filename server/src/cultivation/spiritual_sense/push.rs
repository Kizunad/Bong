use std::collections::HashMap;

use valence::prelude::{
    bevy_ecs, Client, Entity, ParamSet, Position, Query, Res, ResMut, Resource, With,
};

use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::tick::CultivationClock;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::realm_vision::{SenseEntryV1, SpiritualSenseTargetsV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

use super::scanner::{
    scan_targets_inner_ring, scan_targets_mid_ring_void, SpiritualSenseTarget,
    SpiritualSenseTargetKind,
};
use super::throttle::{should_scan, SpiritualSenseRing};

const MAX_TARGETS_PER_PAYLOAD: usize = 24;

#[derive(Default, Resource)]
pub struct SpiritualSensePushState {
    last_inner_scan_tick: HashMap<Entity, u64>,
    last_middle_scan_tick: HashMap<Entity, u64>,
}

#[derive(Debug, Clone, Copy)]
struct PlayerSenseSnapshot {
    entity: Entity,
    position: [f64; 3],
    realm: Realm,
}

type SpiritualSensePlayerReadItem<'a> = (Entity, &'a Position, &'a Cultivation);
type SpiritualSensePlayerReadFilter = With<Client>;
type SpiritualSenseObserverItem<'a> = (Entity, &'a mut Client, &'a Position, &'a Cultivation);
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
    mut player_sets: ParamSet<SpiritualSenseQueryParams<'_, '_>>,
) {
    let now_tick = clock.tick;
    let snapshots: Vec<PlayerSenseSnapshot> = {
        let players = player_sets.p0();
        players
            .iter()
            .map(|(entity, position, cultivation)| PlayerSenseSnapshot {
                entity,
                position: position_to_array(position),
                realm: cultivation.realm,
            })
            .collect()
    };
    if snapshots.is_empty() {
        return;
    }

    let mut observers = player_sets.p1();
    for (entity, mut client, position, cultivation) in &mut observers {
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
        let targets = build_player_sense_targets(entity, observer_pos, &snapshots);
        let mut entries = Vec::new();
        if should_scan_inner {
            entries.extend(scan_targets_inner_ring(
                observer_pos,
                cultivation.realm,
                &targets,
            ));
            state.last_inner_scan_tick.insert(entity, now_tick);
        }
        if should_scan_middle {
            entries.extend(scan_targets_mid_ring_void(observer_pos, &targets));
            state.last_middle_scan_tick.insert(entity, now_tick);
        }
        trim_entries_by_intensity(&mut entries);
        send_spiritual_sense_targets(
            &mut client,
            SpiritualSenseTargetsV1 {
                entries,
                generation: now_tick,
            },
        );
    }
}

fn build_player_sense_targets(
    observer: Entity,
    observer_pos: [f64; 3],
    players: &[PlayerSenseSnapshot],
) -> Vec<SpiritualSenseTarget> {
    players
        .iter()
        .filter(|target| target.entity != observer)
        .map(|target| SpiritualSenseTarget {
            position: target.position,
            kind: SpiritualSenseTargetKind::Cultivator(target.realm),
            intensity: distance_intensity(observer_pos, target.position, 500.0),
        })
        .collect()
}

fn trim_entries_by_intensity(entries: &mut Vec<SenseEntryV1>) {
    entries.sort_by(|a, b| b.intensity.total_cmp(&a.intensity));
    entries.truncate(MAX_TARGETS_PER_PAYLOAD);
}

fn distance_intensity(a: [f64; 3], b: [f64; 3], max_distance: f64) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();
    if max_distance <= 0.0 {
        return 1.0;
    }
    (1.0 - distance / max_distance).clamp(0.1, 1.0)
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
        let near = distance_intensity([0.0, 64.0, 0.0], [50.0, 64.0, 0.0], 500.0);
        let far = distance_intensity([0.0, 64.0, 0.0], [450.0, 64.0, 0.0], 500.0);
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
            },
            PlayerSenseSnapshot {
                entity: other,
                position: [10.0, 64.0, 0.0],
                realm: Realm::Condense,
            },
        ];
        let targets = build_player_sense_targets(observer, [0.0, 64.0, 0.0], &players);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].position, [10.0, 64.0, 0.0]);
        assert_eq!(
            targets[0].kind,
            SpiritualSenseTargetKind::Cultivator(Realm::Condense)
        );

        let position = Position(DVec3::new(1.0, 2.0, 3.0));
        assert_eq!(position_to_array(&position), [1.0, 2.0, 3.0]);
    }
}
