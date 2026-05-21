//! NPC engagement metadata bridge (`bong:npc_metadata`).

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use valence::entity::EntityId;
use valence::prelude::{
    bevy_ecs, ident, Client, DVec3, Entity, EventWriter, Position, Query, ResMut, Resource, With,
    Without,
};

use crate::combat::components::{Lifecycle, LifecycleState, Wounds};
use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::known_techniques::KnownTechniques;
use crate::identity::PlayerIdentities;
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::npc::equipment::NpcEquipment;
use crate::npc::faction::{FactionId, FactionMembership, FactionRank};
use crate::npc::lifecycle::{NpcArchetype, NpcLifespan};
use crate::npc::spawn::NpcMarker;
use crate::npc::trade::NpcTradeInventory;
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
    pub hp_ratio: f32,
    pub qi_ratio: f32,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub equipment: HashMap<String, NpcEquipSlotS2c>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub techniques: Vec<NpcTechniqueS2c>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub trade_offers: Vec<NpcTradeOfferS2c>,
}

/// S2C equipment slot data (per-slot summary for client display).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NpcEquipSlotS2c {
    pub template_id: String,
    pub display_name: String,
    pub quality_tier: u8,
    pub durability_ratio: f32,
}

/// S2C technique data (per-technique summary for client display).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NpcTechniqueS2c {
    pub id: String,
    pub display_name: String,
    pub proficiency: f32,
}

/// S2C trade offer data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NpcTradeOfferS2c {
    pub template_id: String,
    pub display_name: String,
    pub count: u32,
    pub price_bone_coins: u32,
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
    Option<&'a Wounds>,
    Option<&'a NpcEquipment>,
    Option<&'a KnownTechniques>,
    Option<&'a NpcTradeInventory>,
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
            wounds,
            npc_equipment,
            npc_techniques,
            npc_trade_inv,
        ) in &npcs
        {
            if lifecycle.is_some_and(|lifecycle| lifecycle.state == LifecycleState::Terminated) {
                continue;
            }
            if client_position.get().distance_squared(npc_position.get()) > radius_sq {
                continue;
            }

            let metadata = build_npc_metadata(NpcMetadataBuildInput {
                entity_id: entity_id.get(),
                archetype: *archetype,
                cultivation: npc_cultivation,
                membership,
                lifespan,
                player_cultivation,
                player_identities,
                wounds,
                equipment: npc_equipment,
                techniques: npc_techniques,
                trade_inventory: npc_trade_inv,
            });
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

pub struct NpcMetadataBuildInput<'a> {
    pub entity_id: i32,
    pub archetype: NpcArchetype,
    pub cultivation: Option<&'a Cultivation>,
    pub membership: Option<&'a FactionMembership>,
    pub lifespan: Option<&'a NpcLifespan>,
    pub player_cultivation: Option<&'a Cultivation>,
    pub player_identities: Option<&'a PlayerIdentities>,
    pub wounds: Option<&'a Wounds>,
    pub equipment: Option<&'a NpcEquipment>,
    pub techniques: Option<&'a KnownTechniques>,
    pub trade_inventory: Option<&'a NpcTradeInventory>,
}

pub fn build_npc_metadata(input: NpcMetadataBuildInput<'_>) -> NpcMetadataS2c {
    let NpcMetadataBuildInput {
        entity_id,
        archetype,
        cultivation,
        membership,
        lifespan,
        player_cultivation,
        player_identities,
        wounds,
        equipment,
        techniques,
        trade_inventory,
    } = input;
    let realm = cultivation
        .map(|cultivation| cultivation.realm)
        .unwrap_or(Realm::Awaken);
    let reputation_to_player = reputation_to_player_score_for_client(membership, player_identities);
    let faction_name = membership.map(|membership| faction_name(membership.faction_id).to_string());
    let faction_rank = membership.map(|membership| faction_rank_label(membership.rank).to_string());
    let display_name = display_name(archetype, realm, membership);
    let qi_hint = player_cultivation.map(|player| qi_hint(player.realm, realm));
    let hp_ratio = wounds
        .map(|wounds| {
            if wounds.health_max > 0.0 {
                (wounds.health_current / wounds.health_max).clamp(0.0, 1.0)
            } else {
                0.0
            }
        })
        .unwrap_or(1.0);
    let qi_ratio = cultivation
        .map(|cultivation| {
            if cultivation.qi_max > 0.0 {
                (cultivation.qi_current / cultivation.qi_max).clamp(0.0, 1.0) as f32
            } else {
                0.0
            }
        })
        .unwrap_or(0.0);

    // ── equipment → HashMap<slot_name, NpcEquipSlotS2c> ──
    let equip_map: HashMap<String, NpcEquipSlotS2c> = equipment
        .map(|eq| {
            eq.iter_slots()
                .map(|(slot_name, slot)| {
                    (
                        slot_name.to_string(),
                        NpcEquipSlotS2c {
                            template_id: slot.template_id.clone(),
                            display_name: slot.display_name.clone(),
                            quality_tier: slot.quality_tier,
                            durability_ratio: slot.durability_ratio,
                        },
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    // ── techniques → Vec<NpcTechniqueS2c> ──
    let tech_vec: Vec<NpcTechniqueS2c> = techniques
        .map(|kt| {
            kt.entries
                .iter()
                .filter(|entry| entry.active)
                .map(|entry| {
                    let def_name =
                        crate::cultivation::known_techniques::technique_definition(&entry.id)
                            .map(|def| def.display_name)
                            .unwrap_or("未知功法");
                    NpcTechniqueS2c {
                        id: entry.id.clone(),
                        display_name: def_name.to_string(),
                        proficiency: entry.proficiency,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    // ── trade_offers → Vec<NpcTradeOfferS2c> ──
    let trade_vec: Vec<NpcTradeOfferS2c> = trade_inventory
        .map(|inv| {
            inv.offers
                .iter()
                .map(|offer| NpcTradeOfferS2c {
                    template_id: offer.template_id.clone(),
                    display_name: offer.display_name.clone(),
                    count: offer.count,
                    price_bone_coins: offer.price_bone_coins,
                })
                .collect()
        })
        .unwrap_or_default();

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
        hp_ratio,
        qi_ratio,
        equipment: equip_map,
        techniques: tech_vec,
        trade_offers: trade_vec,
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
        NpcArchetype::SkullFiend => "骨煞",
    }
}

pub fn realm_label(realm: Realm) -> &'static str {
    match realm {
        Realm::Awaken => "醒灵",
        Realm::Induce => "引气",
        Realm::Condense => "凝脉",
        Realm::Solidify => "固元",
        Realm::Spirit => "通灵",
        Realm::Void => "化虚",
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
        NpcArchetype::Daoxiang
        | NpcArchetype::Zhinian
        | NpcArchetype::Fuya
        | NpcArchetype::SkullFiend => "此人气息浑浊，答非所问。",
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
    use valence::prelude::ItemKind;

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
        let payload = build_npc_metadata(NpcMetadataBuildInput {
            entity_id: 42,
            archetype: NpcArchetype::Disciple,
            cultivation: Some(&Cultivation {
                realm: Realm::Condense,
                ..Cultivation::default()
            }),
            membership: Some(&membership),
            lifespan: Some(&NpcLifespan::new(40.0, 100.0)),
            player_cultivation: Some(&Cultivation {
                realm: Realm::Awaken,
                ..Cultivation::default()
            }),
            player_identities: None,
            wounds: Some(&Wounds {
                health_current: 40.0,
                health_max: 100.0,
                entries: Vec::new(),
            }),
            equipment: None,
            techniques: None,
            trade_inventory: None,
        });

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
        assert!(json.contains(r#""hp_ratio":0.4"#));
        assert!(json.contains(r#""qi_ratio":0.0"#));
    }

    fn ratio_payload(wounds: Option<&Wounds>, cultivation: Option<&Cultivation>) -> NpcMetadataS2c {
        build_npc_metadata(NpcMetadataBuildInput {
            entity_id: 42,
            archetype: NpcArchetype::Beast,
            cultivation,
            membership: None,
            lifespan: None,
            player_cultivation: None,
            player_identities: None,
            wounds,
            equipment: None,
            techniques: None,
            trade_inventory: None,
        })
    }

    #[test]
    fn npc_metadata_hp_ratio_clamps_boundaries() {
        let zero_max = Wounds {
            health_current: 40.0,
            health_max: 0.0,
            entries: Vec::new(),
        };
        let over_max = Wounds {
            health_current: 140.0,
            health_max: 100.0,
            entries: Vec::new(),
        };
        let below_zero = Wounds {
            health_current: -5.0,
            health_max: 100.0,
            entries: Vec::new(),
        };

        assert_eq!(
            ratio_payload(Some(&zero_max), None).hp_ratio,
            0.0,
            "expected hp_ratio 0.0 because health_max <= 0 is invalid"
        );
        assert_eq!(
            ratio_payload(Some(&over_max), None).hp_ratio,
            1.0,
            "expected hp_ratio 1.0 because health_current > health_max is clamped"
        );
        assert_eq!(
            ratio_payload(Some(&below_zero), None).hp_ratio,
            0.0,
            "expected hp_ratio 0.0 because health_current < 0 is clamped"
        );
        assert_eq!(
            ratio_payload(None, None).hp_ratio,
            1.0,
            "expected hp_ratio 1.0 because missing Wounds defaults to full health"
        );
    }

    #[test]
    fn npc_metadata_qi_ratio_zero_when_qi_max_is_nonpositive() {
        let cultivation = Cultivation {
            qi_current: 20.0,
            qi_max: 0.0,
            ..Cultivation::default()
        };

        assert_eq!(
            ratio_payload(None, Some(&cultivation)).qi_ratio,
            0.0,
            "expected qi_ratio 0.0 because qi_max <= 0 is invalid"
        );
    }

    #[test]
    fn realm_label_matches_worldview_canon() {
        assert_eq!(realm_label(Realm::Awaken), "醒灵");
        assert_eq!(realm_label(Realm::Induce), "引气");
        assert_eq!(realm_label(Realm::Condense), "凝脉");
        assert_eq!(realm_label(Realm::Solidify), "固元");
        assert_eq!(realm_label(Realm::Spirit), "通灵");
        assert_eq!(realm_label(Realm::Void), "化虚");
    }

    #[test]
    fn npc_metadata_reputation_is_player_specific() {
        let membership = membership(0.5);
        let mut identities = crate::identity::PlayerIdentities::with_default("Azure", 0);
        identities.active_mut().unwrap().renown.notoriety = 80;

        let payload = build_npc_metadata(NpcMetadataBuildInput {
            entity_id: 42,
            archetype: NpcArchetype::Rogue,
            cultivation: None,
            membership: Some(&membership),
            lifespan: None,
            player_cultivation: None,
            player_identities: Some(&identities),
            wounds: None,
            equipment: None,
            techniques: None,
            trade_inventory: None,
        });

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

    #[test]
    fn npc_metadata_equipment_serializes() {
        use crate::combat::weapon::WeaponKind;
        use crate::npc::equipment::{NpcEquipSlot, NpcEquipment};

        let equipment = NpcEquipment {
            main_hand: Some(NpcEquipSlot {
                template_id: "iron_sword".to_string(),
                display_name: "铁剑".to_string(),
                item_kind: ItemKind::IronSword,
                quality_tier: 0,
                base_attack: 6.0,
                weapon_kind: Some(WeaponKind::Sword),
                armor_profile_id: None,
                durability_ratio: 0.85,
            }),
            chest: Some(NpcEquipSlot {
                template_id: "bone_chestplate".to_string(),
                display_name: "骨甲".to_string(),
                item_kind: ItemKind::LeatherChestplate,
                quality_tier: 0,
                base_attack: 0.0,
                weapon_kind: None,
                armor_profile_id: Some("bone_chest".to_string()),
                durability_ratio: 0.7,
            }),
            ..Default::default()
        };
        let payload = build_npc_metadata(NpcMetadataBuildInput {
            entity_id: 99,
            archetype: NpcArchetype::Rogue,
            cultivation: None,
            membership: None,
            lifespan: None,
            player_cultivation: None,
            player_identities: None,
            wounds: None,
            equipment: Some(&equipment),
            techniques: None,
            trade_inventory: None,
        });

        assert_eq!(payload.equipment.len(), 2, "expected 2 equipment slots");
        let main = payload
            .equipment
            .get("main_hand")
            .expect("main_hand missing");
        assert_eq!(main.template_id, "iron_sword");
        assert_eq!(main.quality_tier, 0);
        let chest = payload.equipment.get("chest").expect("chest missing");
        assert_eq!(chest.display_name, "骨甲");
    }

    #[test]
    fn npc_metadata_techniques_serializes() {
        use crate::cultivation::known_techniques::{KnownTechnique, KnownTechniques};

        let techniques = KnownTechniques {
            entries: vec![
                KnownTechnique {
                    id: "sword.cleave".to_string(),
                    proficiency: 0.75,
                    active: true,
                },
                KnownTechnique {
                    id: "sword.thrust".to_string(),
                    proficiency: 0.4,
                    active: false,
                },
            ],
        };
        let payload = build_npc_metadata(NpcMetadataBuildInput {
            entity_id: 99,
            archetype: NpcArchetype::Rogue,
            cultivation: None,
            membership: None,
            lifespan: None,
            player_cultivation: None,
            player_identities: None,
            wounds: None,
            equipment: None,
            techniques: Some(&techniques),
            trade_inventory: None,
        });

        assert_eq!(
            payload.techniques.len(),
            1,
            "expected only active techniques"
        );
        assert_eq!(payload.techniques[0].id, "sword.cleave");
        assert_eq!(payload.techniques[0].display_name, "劈");
        assert!((payload.techniques[0].proficiency - 0.75).abs() < 0.01);
    }

    #[test]
    fn npc_metadata_trade_offers_serializes() {
        use crate::npc::trade::{NpcTradeInventory, TradeOffer};

        let trade = NpcTradeInventory {
            offers: vec![TradeOffer {
                template_id: "lingcao".to_string(),
                display_name: "灵草".to_string(),
                count: 3,
                price_bone_coins: 12,
            }],
        };
        let payload = build_npc_metadata(NpcMetadataBuildInput {
            entity_id: 99,
            archetype: NpcArchetype::Rogue,
            cultivation: None,
            membership: None,
            lifespan: None,
            player_cultivation: None,
            player_identities: None,
            wounds: None,
            equipment: None,
            techniques: None,
            trade_inventory: Some(&trade),
        });

        assert_eq!(payload.trade_offers.len(), 1);
        assert_eq!(payload.trade_offers[0].template_id, "lingcao");
        assert_eq!(payload.trade_offers[0].price_bone_coins, 12);
    }

    #[test]
    fn npc_metadata_empty_equipment_not_in_json() {
        let payload = build_npc_metadata(NpcMetadataBuildInput {
            entity_id: 99,
            archetype: NpcArchetype::Beast,
            cultivation: None,
            membership: None,
            lifespan: None,
            player_cultivation: None,
            player_identities: None,
            wounds: None,
            equipment: None,
            techniques: None,
            trade_inventory: None,
        });

        let json = String::from_utf8(payload.to_json_bytes_checked().expect("serialize"))
            .expect("metadata payload should be utf8 json");
        assert!(
            !json.contains("\"equipment\""),
            "expected empty equipment map to be skipped in JSON"
        );
        assert!(
            !json.contains("\"techniques\""),
            "expected empty techniques vec to be skipped in JSON"
        );
        assert!(
            !json.contains("\"trade_offers\""),
            "expected empty trade_offers vec to be skipped in JSON"
        );
    }
}
