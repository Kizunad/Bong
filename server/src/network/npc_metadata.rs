//! NPC engagement metadata bridge (`bong:npc_metadata`).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use valence::entity::EntityId;
use valence::prelude::{
    bevy_ecs, ident, Client, DVec3, Entity, EventWriter, Position, Query, ResMut, Resource, With,
    Without,
};

use crate::combat::components::{Lifecycle, LifecycleState};
use crate::cultivation::components::{Cultivation, Realm};
use crate::identity::PlayerIdentities;
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::npc::faction::{FactionId, FactionMembership, FactionRank};
use crate::npc::lifecycle::{NpcArchetype, NpcLifespan};
use crate::npc::spawn::NpcMarker;
use crate::schema::common::MAX_PAYLOAD_BYTES;
use crate::schema::server_data::ServerDataBuildError;

pub const NPC_METADATA_SYNC_RADIUS: f64 = 64.0;
pub const NPC_METADATA_SYNC_INTERVAL_TICKS: u64 = 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NpcMetadataS2c {
    pub v: u8,
    #[serde(rename = "type")]
    pub ty: String,
    pub entity_id: i32,
    pub archetype: String,
    pub realm: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub faction_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub faction_rank: Option<String>,
    pub reputation_to_player: i32,
    pub display_name: String,
    pub age_band: String,
    pub greeting_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qi_hint: Option<String>,
}

impl NpcMetadataS2c {
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
pub struct NpcMetadataSyncState {
    tick: u64,
    greeted_pairs: HashSet<(Entity, Entity)>,
}

type ClientMetadataItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Position,
    Option<&'a Cultivation>,
    Option<&'a PlayerIdentities>,
);
type NpcMetadataItem<'a> = (
    Entity,
    &'a EntityId,
    &'a Position,
    &'a NpcArchetype,
    Option<&'a Cultivation>,
    Option<&'a FactionMembership>,
    Option<&'a NpcLifespan>,
    Option<&'a Lifecycle>,
);

#[allow(clippy::type_complexity)]
pub fn emit_npc_metadata_payloads(
    mut state: ResMut<NpcMetadataSyncState>,
    mut clients: Query<ClientMetadataItem<'_>, With<Client>>,
    npcs: Query<NpcMetadataItem<'_>, (With<NpcMarker>, Without<Client>)>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    state.tick = state.tick.saturating_add(1);
    if !state.tick.is_multiple_of(NPC_METADATA_SYNC_INTERVAL_TICKS) {
        return;
    }

    let mut active_pairs = HashSet::new();
    let radius_sq = NPC_METADATA_SYNC_RADIUS * NPC_METADATA_SYNC_RADIUS;
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
            lifespan,
            lifecycle,
        ) in &npcs
        {
            if lifecycle.is_some_and(|lifecycle| lifecycle.state == LifecycleState::Terminated) {
                continue;
            }
            if client_position.get().distance_squared(npc_position.get()) > radius_sq {
                continue;
            }

            let metadata = build_npc_metadata(
                entity_id.get(),
                *archetype,
                npc_cultivation,
                membership,
                lifespan,
                player_cultivation,
                player_identities,
            );
            let bytes = match metadata.to_json_bytes_checked() {
                Ok(bytes) => bytes,
                Err(error) => {
                    tracing::warn!(
                        "[bong][npc_metadata] dropping npc metadata entity_id={}: {error:?}",
                        entity_id.get()
                    );
                    continue;
                }
            };
            client.send_custom_payload(ident!("bong:npc_metadata"), &bytes);

            let pair = (client_entity, npc_entity);
            if state.greeted_pairs.insert(pair) {
                if let Some(recipe_id) = greeting_recipe_for_archetype(*archetype) {
                    audio.send(PlaySoundRecipeRequest {
                        recipe_id: recipe_id.to_string(),
                        instance_id: 0,
                        pos: Some(block_pos(npc_position.get())),
                        flag: None,
                        volume_mul: 1.0,
                        pitch_shift: 0.0,
                        recipient: AudioRecipient::Single(client_entity),
                    });
                }
            }
            active_pairs.insert(pair);
        }
    }
    state
        .greeted_pairs
        .retain(|pair| active_pairs.contains(pair));
}

pub fn build_npc_metadata(
    entity_id: i32,
    archetype: NpcArchetype,
    cultivation: Option<&Cultivation>,
    membership: Option<&FactionMembership>,
    lifespan: Option<&NpcLifespan>,
    player_cultivation: Option<&Cultivation>,
    player_identities: Option<&PlayerIdentities>,
) -> NpcMetadataS2c {
    let realm = cultivation
        .map(|cultivation| cultivation.realm)
        .unwrap_or(Realm::Awaken);
    let reputation_to_player = reputation_to_player_score_for_client(membership, player_identities);
    let faction_name = membership.map(|membership| faction_name(membership.faction_id).to_string());
    let faction_rank = membership.map(|membership| faction_rank_label(membership.rank).to_string());
    let display_name = display_name(archetype, realm, membership);
    let qi_hint = player_cultivation.map(|player| qi_hint(player.realm, realm));

    NpcMetadataS2c {
        v: 1,
        ty: "npc_metadata".to_string(),
        entity_id,
        archetype: archetype.as_str().to_string(),
        realm: realm_label(realm).to_string(),
        faction_name,
        faction_rank,
        reputation_to_player,
        display_name,
        age_band: lifespan
            .map(age_band_for_lifespan)
            .unwrap_or("正值壮年")
            .to_string(),
        greeting_text: greeting_text_for_archetype(archetype).to_string(),
        qi_hint,
    }
}

pub fn display_name(
    archetype: NpcArchetype,
    realm: Realm,
    membership: Option<&FactionMembership>,
) -> String {
    if let Some(membership) = membership {
        return format!(
            "{}·{}",
            faction_name(membership.faction_id),
            faction_rank_label(membership.rank)
        );
    }
    format!("{}·{}", archetype_label(archetype), realm_label(realm))
}

pub fn archetype_label(archetype: NpcArchetype) -> &'static str {
    match archetype {
        NpcArchetype::Zombie => "游尸",
        NpcArchetype::Commoner => "凡人",
        NpcArchetype::Rogue => "散修",
        NpcArchetype::Beast => "妖兽",
        NpcArchetype::Disciple => "宗门弟子",
        NpcArchetype::GuardianRelic => "遗种守卫",
        NpcArchetype::Daoxiang => "道伥",
        NpcArchetype::Zhinian => "执念",
        NpcArchetype::Fuya => "负压畸体",
    }
}

pub fn realm_label(realm: Realm) -> &'static str {
    match realm {
        Realm::Awaken => "引气",
        Realm::Induce => "引灵",
        Realm::Condense => "凝脉",
        Realm::Solidify => "固元",
        Realm::Spirit => "化神",
        Realm::Void => "渡虚",
    }
}

pub fn realm_rank(realm: Realm) -> i32 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

pub fn faction_name(faction: FactionId) -> &'static str {
    match faction {
        FactionId::Attack => "魔修派",
        FactionId::Defend => "正道盟",
        FactionId::Neutral => "中立盟",
    }
}

pub fn faction_rank_label(rank: FactionRank) -> &'static str {
    match rank {
        FactionRank::Leader => "掌门",
        FactionRank::Disciple => "真传弟子",
        FactionRank::Ally => "客卿",
    }
}

pub fn reputation_to_player_score(membership: &FactionMembership) -> i32 {
    ((membership.reputation.loyalty() - 0.5) * 200.0).round() as i32
}

pub fn reputation_to_player_score_for_client(
    membership: Option<&FactionMembership>,
    player_identities: Option<&PlayerIdentities>,
) -> i32 {
    let faction_baseline = membership
        .map(reputation_to_player_score)
        .unwrap_or_default();
    let identity_reputation = player_identities
        .and_then(PlayerIdentities::active)
        .map(|identity| identity.reputation_score())
        .unwrap_or_default();
    faction_baseline
        .saturating_add(identity_reputation)
        .clamp(-100, 100)
}

fn age_band_for_lifespan(lifespan: &NpcLifespan) -> &'static str {
    let ratio = lifespan.age_ratio().clamp(0.0, 1.0);
    if ratio >= 0.85 {
        "风烛残年"
    } else if ratio >= 0.55 {
        "年岁渐高"
    } else if ratio >= 0.20 {
        "正值壮年"
    } else {
        "初入尘世"
    }
}

fn qi_hint(player_realm: Realm, npc_realm: Realm) -> String {
    let delta = realm_rank(npc_realm) - realm_rank(player_realm);
    if delta >= 2 {
        "你看不清此人深浅".to_string()
    } else if delta <= -1 {
        "真元池约略可辨".to_string()
    } else {
        "真元流转平稳".to_string()
    }
}

pub fn greeting_text_for_archetype(archetype: NpcArchetype) -> &'static str {
    match archetype {
        NpcArchetype::Rogue | NpcArchetype::Disciple => "道友，可有灵草出让？",
        NpcArchetype::Commoner => "大仙，小人不敢...",
        NpcArchetype::Beast => "它盯着你，喉间低鸣。",
        NpcArchetype::GuardianRelic => "遗种守卫无声拦在前方。",
        NpcArchetype::Daoxiang | NpcArchetype::Zhinian | NpcArchetype::Fuya => {
            "此人气息浑浊，答非所问。"
        }
        NpcArchetype::Zombie => "游尸没有回应。",
    }
}

pub fn greeting_recipe_for_archetype(archetype: NpcArchetype) -> Option<&'static str> {
    match archetype {
        NpcArchetype::Rogue | NpcArchetype::Disciple => Some("npc_greeting_cultivator"),
        NpcArchetype::Commoner => Some("npc_greeting_commoner"),
        _ => None,
    }
}

fn block_pos(origin: DVec3) -> [i32; 3] {
    [
        origin.x.floor() as i32,
        origin.y.floor() as i32,
        origin.z.floor() as i32,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::npc::faction::{FactionId, FactionRank, Lineage, MissionQueue, Reputation};

    fn membership(loyalty: f64) -> FactionMembership {
        FactionMembership {
            faction_id: FactionId::Attack,
            rank: FactionRank::Disciple,
            reputation: Reputation { loyalty },
            lineage: Some(Lineage {
                master_id: Some("npc:master".to_string()),
                disciple_ids: Vec::new(),
            }),
            mission_queue: MissionQueue::default(),
        }
    }

    #[test]
    fn npc_metadata_packet_serializes() {
        let membership = membership(0.75);
        let payload = build_npc_metadata(
            42,
            NpcArchetype::Disciple,
            Some(&Cultivation {
                realm: Realm::Condense,
                ..Cultivation::default()
            }),
            Some(&membership),
            Some(&NpcLifespan::new(40.0, 100.0)),
            Some(&Cultivation {
                realm: Realm::Awaken,
                ..Cultivation::default()
            }),
            None,
        );

        let json = String::from_utf8(payload.to_json_bytes_checked().expect("serialize"))
            .expect("metadata payload should be utf8 json");
        assert!(json.contains(r#""type":"npc_metadata""#));
        assert!(json.contains(r#""entity_id":42"#));
        assert!(json.contains(r#""archetype":"disciple""#));
        assert!(json.contains(r#""realm":"凝脉""#));
        assert!(json.contains(r#""display_name":"魔修派·真传弟子""#));
        assert!(json.contains(r#""greeting_text":"道友，可有灵草出让？""#));
        assert!(json.contains(r#""reputation_to_player":50"#));
        assert!(json.contains(r#""qi_hint":"你看不清此人深浅""#));
    }

    #[test]
    fn npc_metadata_reputation_is_player_specific() {
        let membership = membership(0.5);
        let mut identities = crate::identity::PlayerIdentities::with_default("Azure", 0);
        identities.active_mut().unwrap().renown.notoriety = 80;

        let payload = build_npc_metadata(
            42,
            NpcArchetype::Rogue,
            None,
            Some(&membership),
            None,
            None,
            Some(&identities),
        );

        assert_eq!(payload.reputation_to_player, -80);
    }

    #[test]
    fn dialogue_greeting_by_archetype() {
        assert_eq!(
            greeting_text_for_archetype(NpcArchetype::Rogue),
            "道友，可有灵草出让？"
        );
        assert_eq!(
            greeting_text_for_archetype(NpcArchetype::Commoner),
            "大仙，小人不敢..."
        );
    }
}
