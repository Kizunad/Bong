//! TSY polish UI bridges (`bong:tsy_boss_health`, `bong:tsy_death_vfx`).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, ident, Client, Entity, EntityLayerId, EventReader, Position, Query, ResMut, Resource,
    With, Without,
};

use crate::combat::components::{Lifecycle, LifecycleState, Wounds};
use crate::combat::events::DeathEvent;
use crate::cultivation::components::{Cultivation, Realm};
use crate::npc::tsy_hostile::TsySentinelMarker;
use crate::schema::common::MAX_PAYLOAD_BYTES;
use crate::schema::server_data::ServerDataBuildError;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::tsy::TsyPresence;

pub const TSY_BOSS_HEALTH_SYNC_RADIUS: f64 = 64.0;
pub const TSY_BOSS_HEALTH_SYNC_INTERVAL_TICKS: u64 = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TsyBossHealthS2c {
    pub v: u8,
    #[serde(rename = "type")]
    pub ty: String,
    pub active: bool,
    pub boss_name: String,
    pub realm: String,
    pub health_ratio: f32,
    pub phase: u8,
    pub max_phase: u8,
}

impl TsyBossHealthS2c {
    pub fn to_json_bytes_checked(&self) -> Result<Vec<u8>, ServerDataBuildError> {
        let bytes = serde_json::to_vec(self).map_err(ServerDataBuildError::Json)?;
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(ServerDataBuildError::Oversize {
                size: bytes.len(),
                max: MAX_PAYLOAD_BYTES,
            });
        }
        Ok(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TsyDeathVfxS2c {
    pub v: u8,
    #[serde(rename = "type")]
    pub ty: String,
}

impl TsyDeathVfxS2c {
    pub fn to_json_bytes_checked(&self) -> Result<Vec<u8>, ServerDataBuildError> {
        let bytes = serde_json::to_vec(self).map_err(ServerDataBuildError::Json)?;
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(ServerDataBuildError::Oversize {
                size: bytes.len(),
                max: MAX_PAYLOAD_BYTES,
            });
        }
        Ok(bytes)
    }
}

#[derive(Debug, Default, Resource)]
pub struct TsyBossHealthSyncState {
    tick: u64,
    last_sent: HashMap<Entity, TsyBossHealthSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TsyBossHealthSnapshot {
    active: bool,
    boss_name: String,
    realm: String,
    health_bucket: u8,
    phase: u8,
    max_phase: u8,
}

type ClientBossHealthItem<'a> = (Entity, &'a mut Client, &'a Position, &'a EntityLayerId);
type SentinelBossHealthItem<'a> = (
    &'a Position,
    &'a EntityLayerId,
    &'a Wounds,
    &'a TsySentinelMarker,
    Option<&'a Cultivation>,
    Option<&'a Lifecycle>,
);

pub fn emit_tsy_boss_health_payloads(
    mut state: ResMut<TsyBossHealthSyncState>,
    mut clients: Query<ClientBossHealthItem<'_>, With<Client>>,
    sentinels: Query<SentinelBossHealthItem<'_>, Without<Client>>,
) {
    state.tick = state.tick.saturating_add(1);
    if !state
        .tick
        .is_multiple_of(TSY_BOSS_HEALTH_SYNC_INTERVAL_TICKS)
    {
        return;
    }

    let radius_sq = TSY_BOSS_HEALTH_SYNC_RADIUS * TSY_BOSS_HEALTH_SYNC_RADIUS;
    let mut active_clients = HashMap::new();
    for (client_entity, mut client, client_position, client_layer) in &mut clients {
        let client_origin = client_position.get();
        let payload = sentinels
            .iter()
            .filter(|(_, sentinel_layer, _, _, _, lifecycle)| {
                sentinel_layer.0 == client_layer.0
                    && !lifecycle
                        .is_some_and(|lifecycle| lifecycle.state == LifecycleState::Terminated)
            })
            .filter_map(|(sentinel_position, _, wounds, marker, cultivation, _)| {
                let distance_sq = client_origin.distance_squared(sentinel_position.get());
                if distance_sq > radius_sq {
                    return None;
                }
                Some((
                    distance_sq,
                    build_tsy_boss_health_payload(true, wounds, marker, cultivation),
                ))
            })
            .min_by(|left, right| {
                left.0
                    .partial_cmp(&right.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(_, payload)| payload)
            .unwrap_or_else(build_inactive_tsy_boss_health_payload);

        let snapshot = TsyBossHealthSnapshot::from_payload(&payload);
        if state.last_sent.get(&client_entity) == Some(&snapshot) {
            active_clients.insert(client_entity, snapshot);
            continue;
        }
        let bytes = match payload.to_json_bytes_checked() {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!("[bong][tsy_polish] dropping boss health payload: {error:?}");
                continue;
            }
        };
        client.send_custom_payload(ident!("bong:tsy_boss_health"), &bytes);
        active_clients.insert(client_entity, snapshot);
    }
    state.last_sent = active_clients;
}

type DeathVfxClientItem<'a> = (
    &'a mut Client,
    Option<&'a CurrentDimension>,
    Option<&'a TsyPresence>,
);

pub fn emit_tsy_death_vfx_payloads(
    mut deaths: EventReader<DeathEvent>,
    mut clients: Query<DeathVfxClientItem<'_>, With<Client>>,
) {
    for event in deaths.read() {
        let Ok((mut client, dimension, presence)) = clients.get_mut(event.target) else {
            continue;
        };
        if !is_tsy_death_target(dimension, presence, event.cause.as_str()) {
            continue;
        }
        let payload = build_tsy_death_vfx_payload();
        let bytes = match payload.to_json_bytes_checked() {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!("[bong][tsy_polish] dropping death vfx payload: {error:?}");
                continue;
            }
        };
        client.send_custom_payload(ident!("bong:tsy_death_vfx"), &bytes);
    }
}

pub fn build_tsy_boss_health_payload(
    active: bool,
    wounds: &Wounds,
    marker: &TsySentinelMarker,
    cultivation: Option<&Cultivation>,
) -> TsyBossHealthS2c {
    let health_ratio = if active && wounds.health_max > f32::EPSILON {
        (wounds.health_current / wounds.health_max).clamp(0.0, 1.0)
    } else {
        0.0
    };
    TsyBossHealthS2c {
        v: 1,
        ty: "tsy_boss_health".to_string(),
        active,
        boss_name: "秘境守灵".to_string(),
        realm: cultivation
            .map(|cultivation| realm_label(cultivation.realm))
            .unwrap_or("未知")
            .to_string(),
        health_ratio,
        phase: marker
            .phase
            .saturating_add(1)
            .clamp(1, marker.max_phase.max(1)),
        max_phase: marker.max_phase.clamp(1, 5),
    }
}

pub fn build_inactive_tsy_boss_health_payload() -> TsyBossHealthS2c {
    TsyBossHealthS2c {
        v: 1,
        ty: "tsy_boss_health".to_string(),
        active: false,
        boss_name: "秘境守灵".to_string(),
        realm: "未知".to_string(),
        health_ratio: 0.0,
        phase: 1,
        max_phase: 1,
    }
}

pub fn build_tsy_death_vfx_payload() -> TsyDeathVfxS2c {
    TsyDeathVfxS2c {
        v: 1,
        ty: "tsy_death_vfx".to_string(),
    }
}

fn is_tsy_death_target(
    dimension: Option<&CurrentDimension>,
    presence: Option<&TsyPresence>,
    cause: &str,
) -> bool {
    presence.is_some()
        || dimension.is_some_and(|dimension| dimension.0 == DimensionKind::Tsy)
        || matches!(cause, "tsy_drain" | "tsy_collapsed")
}

fn realm_label(realm: Realm) -> &'static str {
    match realm {
        Realm::Awaken => "醒灵",
        Realm::Induce => "引气",
        Realm::Condense => "凝脉",
        Realm::Solidify => "固元",
        Realm::Spirit => "通灵",
        Realm::Void => "洞虚",
    }
}

impl TsyBossHealthSnapshot {
    fn from_payload(payload: &TsyBossHealthS2c) -> Self {
        Self {
            active: payload.active,
            boss_name: payload.boss_name.clone(),
            realm: payload.realm.clone(),
            health_bucket: (payload.health_ratio.clamp(0.0, 1.0) * 100.0).round() as u8,
            phase: payload.phase,
            max_phase: payload.max_phase,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sentinel(phase: u8, max_phase: u8) -> TsySentinelMarker {
        TsySentinelMarker {
            family_id: "tsy_test".to_string(),
            guarding_container: None,
            phase,
            max_phase,
        }
    }

    #[test]
    fn boss_health_payload_serializes_contract_fields() {
        let wounds = Wounds {
            health_current: 40.0,
            health_max: 100.0,
            ..Wounds::default()
        };
        let cultivation = Cultivation {
            realm: Realm::Spirit,
            ..Cultivation::default()
        };
        let payload =
            build_tsy_boss_health_payload(true, &wounds, &sentinel(1, 3), Some(&cultivation));
        let json = String::from_utf8(payload.to_json_bytes_checked().expect("serialize"))
            .expect("boss health payload should be utf8 json");

        assert!(json.contains(r#""type":"tsy_boss_health""#));
        assert!(json.contains(r#""active":true"#));
        assert!(json.contains(r#""health_ratio":0.4"#));
        assert!(json.contains(r#""phase":2"#));
        assert!(json.contains(r#""max_phase":3"#));
        assert!(json.contains(r#""realm":"通灵""#));
    }

    #[test]
    fn boss_health_snapshot_buckets_health_ratio() {
        let mut payload = build_inactive_tsy_boss_health_payload();
        payload.active = true;
        payload.health_ratio = 0.664;
        let snapshot = TsyBossHealthSnapshot::from_payload(&payload);

        assert_eq!(snapshot.health_bucket, 66);
    }

    #[test]
    fn death_vfx_gate_accepts_tsy_causes_and_dimension() {
        assert!(is_tsy_death_target(None, None, "tsy_drain"));
        assert!(is_tsy_death_target(
            Some(&CurrentDimension(DimensionKind::Tsy)),
            None,
            "bleed_out"
        ));
        assert!(!is_tsy_death_target(
            Some(&CurrentDimension(DimensionKind::Overworld)),
            None,
            "bleed_out"
        ));
    }

    #[test]
    fn death_vfx_payload_serializes_contract_fields() {
        let payload = build_tsy_death_vfx_payload();
        let json = String::from_utf8(payload.to_json_bytes_checked().expect("serialize"))
            .expect("death vfx payload should be utf8 json");

        assert!(json.contains(r#""type":"tsy_death_vfx""#));
        assert!(json.contains(r#""v":1"#));
    }
}
