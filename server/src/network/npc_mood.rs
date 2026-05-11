//! NPC mood/threat bridge (`bong:npc_mood`).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::entity::EntityId;
use valence::prelude::{
    bevy_ecs, ident, Client, Entity, Position, Query, ResMut, Resource, With, Without,
};

use crate::combat::components::{Lifecycle, LifecycleState};
use crate::cultivation::components::{Cultivation, Realm};
use crate::identity::PlayerIdentities;
use crate::network::npc_metadata::{realm_rank, reputation_to_player_score_for_client};
use crate::npc::faction::FactionMembership;
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::spawn::NpcMarker;
use crate::schema::common::MAX_PAYLOAD_BYTES;
use crate::schema::server_data::ServerDataBuildError;

pub const NPC_MOOD_SYNC_RADIUS: f64 = 32.0;
pub const NPC_MOOD_SYNC_INTERVAL_TICKS: u64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpcMood {
    Neutral,
    Alert,
    Hostile,
    Fearful,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NpcMoodS2c {
    pub v: u8,
    #[serde(rename = "type")]
    pub ty: String,
    pub entity_id: i32,
    pub mood: NpcMood,
    pub threat_level: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qi_level_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inner_monologue: Option<String>,
}

impl NpcMoodS2c {
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
pub struct NpcMoodSyncState {
    tick: u64,
    last_sent: HashMap<(Entity, Entity), MoodSyncSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MoodSyncSnapshot {
    mood: NpcMood,
    threat_bucket: u8,
    qi_level_hint: Option<String>,
    inner_monologue: Option<String>,
}

type ClientMoodItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Position,
    Option<&'a Cultivation>,
    Option<&'a PlayerIdentities>,
);
type NpcMoodItem<'a> = (
    Entity,
    &'a EntityId,
    &'a Position,
    &'a NpcArchetype,
    Option<&'a Cultivation>,
    Option<&'a FactionMembership>,
    Option<&'a Lifecycle>,
);

pub fn emit_npc_mood_payloads(
    mut state: ResMut<NpcMoodSyncState>,
    mut clients: Query<ClientMoodItem<'_>, With<Client>>,
    npcs: Query<NpcMoodItem<'_>, (With<NpcMarker>, Without<Client>)>,
) {
    state.tick = state.tick.saturating_add(1);
    if !state.tick.is_multiple_of(NPC_MOOD_SYNC_INTERVAL_TICKS) {
        return;
    }

    let mut active_pairs = HashMap::new();
    let radius_sq = NPC_MOOD_SYNC_RADIUS * NPC_MOOD_SYNC_RADIUS;
    for (client_entity, mut client, client_position, player_cultivation, player_identities) in
        &mut clients
    {
        for (
            npc_entity,
            entity_id,
            npc_position,
            archetype,
            npc_cultivation,
            membership,
            lifecycle,
        ) in &npcs
        {
            if lifecycle.is_some_and(|lifecycle| lifecycle.state == LifecycleState::Terminated) {
                continue;
            }
            if client_position.get().distance_squared(npc_position.get()) > radius_sq {
                continue;
            }

            let payload = build_npc_mood(
                entity_id.get(),
                *archetype,
                npc_cultivation,
                membership,
                player_cultivation,
                player_identities,
            );
            let snapshot = MoodSyncSnapshot::from_payload(&payload);
            let pair = (client_entity, npc_entity);
            if state.last_sent.get(&pair) == Some(&snapshot) {
                active_pairs.insert(pair, snapshot);
                continue;
            }

            let bytes = match payload.to_json_bytes_checked() {
                Ok(bytes) => bytes,
                Err(error) => {
                    tracing::warn!(
                        "[bong][npc_mood] dropping npc mood entity_id={}: {error:?}",
                        entity_id.get()
                    );
                    continue;
                }
            };
            client.send_custom_payload(ident!("bong:npc_mood"), &bytes);
            active_pairs.insert(pair, snapshot);
        }
    }

    state.last_sent = active_pairs;
}

impl MoodSyncSnapshot {
    fn from_payload(payload: &NpcMoodS2c) -> Self {
        Self {
            mood: payload.mood,
            threat_bucket: threat_bucket(payload.threat_level),
            qi_level_hint: payload.qi_level_hint.clone(),
            inner_monologue: payload.inner_monologue.clone(),
        }
    }
}

pub fn build_npc_mood(
    entity_id: i32,
    archetype: NpcArchetype,
    npc_cultivation: Option<&Cultivation>,
    membership: Option<&FactionMembership>,
    player_cultivation: Option<&Cultivation>,
    player_identities: Option<&PlayerIdentities>,
) -> NpcMoodS2c {
    let npc_realm = npc_cultivation
        .map(|cultivation| cultivation.realm)
        .unwrap_or(Realm::Awaken);
    let player_realm = player_cultivation
        .map(|cultivation| cultivation.realm)
        .unwrap_or(Realm::Awaken);
    let reputation = reputation_to_player_score_for_client(membership, player_identities);
    let threat_level = threat_level_for(archetype, npc_realm, player_realm, reputation);
    let mood = mood_for_context(threat_level, npc_realm, player_realm, reputation);
    NpcMoodS2c {
        v: 1,
        ty: "npc_mood".to_string(),
        entity_id,
        mood,
        threat_level,
        qi_level_hint: qi_level_hint_for(player_realm, npc_realm),
        inner_monologue: inner_monologue_for(archetype, mood, player_realm, threat_level),
    }
}

pub fn threat_level_for(
    archetype: NpcArchetype,
    npc_realm: Realm,
    player_realm: Realm,
    reputation_to_player: i32,
) -> f32 {
    let reputation_pressure = match reputation_to_player {
        i32::MIN..=-71 => 0.82,
        -70..=-31 => 0.62,
        -30..=-1 => 0.42,
        0..=50 => 0.24,
        _ => 0.12,
    };
    let realm_delta = realm_rank(npc_realm) - realm_rank(player_realm);
    let realm_pressure = (realm_delta as f32 * 0.12).clamp(-0.24, 0.30);
    let archetype_floor = match archetype {
        NpcArchetype::Daoxiang => 0.86,
        NpcArchetype::Zhinian => 0.74,
        NpcArchetype::Fuya => 0.80,
        NpcArchetype::GuardianRelic => 0.58,
        NpcArchetype::Beast | NpcArchetype::Zombie => 0.48,
        _ => 0.0,
    };
    let archetype_floor = if archetype == NpcArchetype::Daoxiang {
        crate::npc::tsy_hostile::dao_chang_threat_level_for_realm(player_realm)
    } else {
        archetype_floor
    };
    (reputation_pressure + realm_pressure)
        .max(archetype_floor)
        .clamp(0.0, 1.0)
}

pub fn mood_for_context(
    threat_level: f32,
    npc_realm: Realm,
    player_realm: Realm,
    reputation_to_player: i32,
) -> NpcMood {
    if threat_level >= 0.72 || reputation_to_player <= -70 {
        return NpcMood::Hostile;
    }
    let player_advantage = realm_rank(player_realm) - realm_rank(npc_realm);
    if player_advantage >= 2 && reputation_to_player >= -30 {
        return NpcMood::Fearful;
    }
    if threat_level >= 0.34 {
        return NpcMood::Alert;
    }
    NpcMood::Neutral
}

fn threat_bucket(threat_level: f32) -> u8 {
    (threat_level.clamp(0.0, 1.0) * 100.0).round() as u8
}

fn qi_level_hint_for(player_realm: Realm, npc_realm: Realm) -> Option<String> {
    if realm_rank(player_realm) < realm_rank(Realm::Solidify) {
        return None;
    }
    let delta = realm_rank(npc_realm) - realm_rank(player_realm);
    Some(if delta <= -1 {
        "低".to_string()
    } else if delta >= 1 {
        "高".to_string()
    } else {
        "中".to_string()
    })
}

fn inner_monologue_for(
    archetype: NpcArchetype,
    mood: NpcMood,
    player_realm: Realm,
    threat_level: f32,
) -> Option<String> {
    if realm_rank(player_realm) < realm_rank(Realm::Spirit) {
        return None;
    }
    let text = match (archetype, mood) {
        (NpcArchetype::Daoxiang, _) => {
            let flip_ticks = crate::npc::tsy_hostile::dao_chang_lure_flip_delay(8.0).unwrap_or(6);
            return Some(format!("先示弱，等他近到八步，{flip_ticks} 刻后动手。"));
        }
        (NpcArchetype::Zhinian, _) => {
            let seconds =
                crate::npc::tsy_hostile::obsession_lure_release_window_seconds(3).unwrap_or(5);
            return Some(format!("灵物是真的，{seconds} 息足够。"));
        }
        (_, NpcMood::Hostile) if threat_level >= 0.9 => "此人真元快空了，动手！",
        (_, NpcMood::Fearful) => "打不过，先退。",
        (_, NpcMood::Alert) => "先看他出价，再决定。",
        _ => return None,
    };
    Some(text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::npc::faction::{FactionId, FactionRank, Lineage, MissionQueue, Reputation};

    fn membership(loyalty: f64) -> FactionMembership {
        FactionMembership {
            faction_id: FactionId::Neutral,
            rank: FactionRank::Ally,
            reputation: Reputation { loyalty },
            lineage: Some(Lineage {
                master_id: None,
                disciple_ids: Vec::new(),
            }),
            mission_queue: MissionQueue::default(),
        }
    }

    #[test]
    fn mood_change_emits_packet() {
        let previous = NpcMoodS2c {
            v: 1,
            ty: "npc_mood".to_string(),
            entity_id: 7,
            mood: NpcMood::Neutral,
            threat_level: 0.24,
            qi_level_hint: None,
            inner_monologue: None,
        };
        let next = NpcMoodS2c {
            mood: NpcMood::Alert,
            threat_level: 0.42,
            ..previous.clone()
        };

        assert_eq!(
            MoodSyncSnapshot::from_payload(&previous),
            MoodSyncSnapshot::from_payload(&previous)
        );
        assert_ne!(
            MoodSyncSnapshot::from_payload(&previous),
            MoodSyncSnapshot::from_payload(&next)
        );
    }

    #[test]
    fn threat_level_pins_daochang_fake_friendly_as_dangerous() {
        let threat = threat_level_for(NpcArchetype::Daoxiang, Realm::Awaken, Realm::Spirit, 80);
        assert!(threat > 0.80);
        assert_eq!(
            mood_for_context(threat, Realm::Awaken, Realm::Spirit, 80),
            NpcMood::Hostile
        );
    }

    #[test]
    fn archetype_mood_reputation_matrix() {
        let archetypes = [
            NpcArchetype::Zombie,
            NpcArchetype::Commoner,
            NpcArchetype::Rogue,
            NpcArchetype::Beast,
            NpcArchetype::Disciple,
            NpcArchetype::GuardianRelic,
            NpcArchetype::Daoxiang,
            NpcArchetype::Zhinian,
            NpcArchetype::Fuya,
        ];
        let reputations = [-80, 0, 60];
        for archetype in archetypes {
            for reputation in reputations {
                let player_realm = if matches!(archetype, NpcArchetype::Daoxiang) {
                    Realm::Spirit
                } else {
                    Realm::Condense
                };
                let threat = threat_level_for(archetype, Realm::Condense, player_realm, reputation);
                assert!((0.0..=1.0).contains(&threat));
                let mood = mood_for_context(threat, Realm::Condense, player_realm, reputation);
                if reputation <= -70
                    || matches!(
                        archetype,
                        NpcArchetype::Daoxiang | NpcArchetype::Zhinian | NpcArchetype::Fuya
                    )
                {
                    assert!(
                        matches!(mood, NpcMood::Hostile | NpcMood::Alert),
                        "high-risk combination should not be neutral: {archetype:?} reputation={reputation}"
                    );
                }
            }
        }
    }

    #[test]
    fn solidify_and_spirit_viewers_get_extra_sense_fields() {
        let neutral = membership(0.5);
        let solidify = Cultivation {
            realm: Realm::Solidify,
            ..Cultivation::default()
        };
        let spirit = Cultivation {
            realm: Realm::Spirit,
            ..Cultivation::default()
        };
        let npc = Cultivation {
            realm: Realm::Condense,
            ..Cultivation::default()
        };

        let solidify_payload = build_npc_mood(
            3,
            NpcArchetype::Rogue,
            Some(&npc),
            Some(&neutral),
            Some(&solidify),
            None,
        );
        assert_eq!(solidify_payload.qi_level_hint.as_deref(), Some("低"));
        assert!(solidify_payload.inner_monologue.is_none());

        let spirit_payload = build_npc_mood(
            3,
            NpcArchetype::Daoxiang,
            Some(&npc),
            Some(&neutral),
            Some(&spirit),
            None,
        );
        assert!(spirit_payload.inner_monologue.is_some());
    }
}
