pub mod components;
pub mod events;
pub mod high_renown_tracker;
pub mod niche_defense;
pub mod pvp_encounter;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;
use valence::message::SendMessage;
use valence::prelude::bevy_ecs::system::ParamSet;
use valence::prelude::{
    bevy_ecs, Added, App, BlockPos, BlockState, ChunkLayer, Client, Commands, DVec3, DiggingEvent,
    DiggingState, Entity, EventReader, EventWriter, Events, IntoSystemConfigs, Or, Position, Query,
    Res, ResMut, Resource, SystemSet, Update, Username, With, Without,
};

use self::components::{
    Anonymity, ExposureEvent, ExposureLog, FactionMembership, FactionReputation, HouseGuardian,
    Relationship, Relationships, Renown, SparringState, SpiritNiche,
};
use self::events::{
    FactionMembershipDecisionEvent, FactionMembershipDecisionKind, FactionReputationDeltaEvent,
    NicheGuardianBroken, NicheGuardianFatigue, NicheIntrusionAttempt, NicheIntrusionEvent,
    PlayerChatCollected, SocialExposureEvent, SocialMentorshipEvent, SocialPactEvent,
    SocialRelationshipEvent, SocialRenownDeltaEvent, SparringInviteRequest,
    SparringInviteResponseEvent, SparringInviteResponseKind, SpiritNicheCoordinateRevealRequest,
    SpiritNichePlaceRequest, SpiritNicheRepairRequest, SpiritNicheRevealRequest,
    SpiritNicheRevealSource, TradeOfferRequest, TradeOfferResponseEvent,
};
use crate::combat::components::{Lifecycle, LifecycleState};
use crate::combat::events::{ApplyStatusEffectIntent, DeathEvent, StatusEffectKind};
use crate::combat::CombatClock;
use crate::cultivation::components::{Cultivation, Karma, QiFlowError, Realm};
use crate::cultivation::death_hooks::release_qi_amount_to_zone;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::lifespan::LifespanComponent;
use crate::identity::{reaction::npc_should_decline_trade, IdentityId, PlayerIdentities};
use crate::inventory::{
    consume_item_instance_once, exchange_inventory_items, inventory_item_by_instance, ItemInstance,
    PlayerInventory,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest, AUDIO_AREA_RADIUS};
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::network::redis_bridge::RedisOutbound;
use crate::network::{gameplay_vfx, vfx_event_emit::VfxEventRequest};
use crate::network::{send_server_data_payload, RedisBridgeResource};
use crate::npc::faction::{FactionId, FactionStatus, NamedFactionRegistry};
use crate::persistence::{identity as identity_db, PersistenceSettings};
use crate::player::gameplay::PendingGameplayNarrations;
use crate::player::state::{
    player_username_from_character_id, save_player_shrine_anchor_slice, PlayerState,
    PlayerStatePersistence,
};
use crate::qi_physics::ledger::{QiTransfer, WorldQiAccount};
use crate::schema::common::NarrationStyle;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::schema::social::{
    ExposureKindV1, GuardianKindV1, NicheGuardianBrokenV1, NicheGuardianFatigueV1,
    NicheIntrusionEventV1, RelationshipKindV1, RenownTagV1, SocialAnonymityPayloadV1,
    SocialExposureEventV1, SocialFeudEventV1, SocialPactEventV1, SocialRemoteIdentityV1,
    SocialRenownDeltaV1, SparringInvitePayloadV1, TradeItemSummaryV1, TradeOfferPayloadV1,
};
use crate::world::dimension::CurrentDimension;
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

const CHAT_EXPOSURE_RADIUS: f64 = 50.0;
const DEATH_EXPOSURE_RADIUS: f64 = 50.0;
const COMPANION_RADIUS: f64 = 50.0;
const COMPANION_SCAN_INTERVAL_TICKS: u64 = 20;
const COMPANION_REQUIRED_SECONDS: u64 = 5 * 60 * 60;
const COMPANION_EXPIRE_TICKS: u64 = 30 * 24 * 60 * 60 * 20;
const FACTION_BETRAYAL_BLOCK_TICKS: u64 = 30 * 24 * 60 * 60 * 20;
const FACTION_PERMANENT_REFUSAL_THRESHOLD: u8 = 3;
const SPARRING_INVITE_TIMEOUT_TICKS: u64 = 10 * 20;
const SPARRING_INVITE_TIMEOUT_MS: u64 = 10_000;
const SPARRING_MAX_TICKS: u64 = 5 * 60 * 20;
const SPARRING_HUMILITY_TICKS: u64 = 5 * 60 * 20;
const TRADE_OFFER_TIMEOUT_TICKS: u64 = 10 * 20;
const TRADE_OFFER_TIMEOUT_MS: u64 = 10_000;
const SPIRIT_NICHE_ITEM_TEMPLATE_ID: &str = "niche_base";
const SPIRIT_NICHE_REPAIR_ITEM_TEMPLATE_ID: &str = "niche_repair_kit";
const SPIRIT_NICHE_RADIUS: f64 = 5.0;
const SPIRIT_NICHE_NEGATIVE_QI_DAMAGE_RATIO: f64 = 0.1;
const NAMELESS_LABEL: &str = "无名修士";
const SPIRIT_NICHE_REPAIR_AUDIO_RECIPE_ID: &str = "niche_repair";
const SPIRIT_NICHE_REPAIR_NARRATIONS: [&str; 2] = [
    "龛石的裂纹被混合料填实，冷石重新泛起温润的微光。",
    "你将碎石灵铁糊进灵龛的伤口，它低低地“合”了一声，像是松了口气。",
];

type CompanionPairKey = (String, String);
type FactionMembershipSqlRow = (
    Option<String>,
    Option<String>,
    i64,
    i64,
    i64,
    Option<i64>,
    i64,
);
type SpiritNicheSqlRow = ([i32; 3], u64, bool, Option<String>, bool, String);

#[derive(Debug, Default, Resource)]
struct CompanionProgress {
    pair_seconds: HashMap<CompanionPairKey, u64>,
}

#[derive(Debug, Clone)]
struct PendingSparringInvite {
    initiator: Entity,
    target: Entity,
    created_at_tick: u64,
}

#[derive(Debug, Default, Resource)]
struct SparringInviteRegistry {
    pending: HashMap<String, PendingSparringInvite>,
}

#[derive(Debug, Clone)]
struct PendingTradeOffer {
    initiator: Entity,
    target: Entity,
    initiator_char_id: String,
    target_char_id: String,
    offered_instance_id: u64,
    offered_item: TradeItemSummaryV1,
    expires_at_tick: u64,
}

#[derive(Debug, Default, Resource)]
struct TradeOfferRegistry {
    pending: HashMap<String, PendingTradeOffer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub(crate) enum SocialSystemSet {
    TradeOfferResponse,
}

#[derive(Debug, Default, Resource)]
pub(crate) struct SpiritNicheRegistry {
    niches: HashMap<String, SpiritNiche>,
    hydrated: bool,
}

impl SpiritNicheRegistry {
    pub(crate) fn upsert(&mut self, niche: SpiritNiche) {
        self.niches.insert(niche.owner.clone(), niche);
    }

    fn reveal(&mut self, owner: &str, revealed_by: Option<String>) -> Option<SpiritNiche> {
        let niche = self.niches.get_mut(owner)?;
        if niche.revealed {
            return None;
        }
        niche.revealed = true;
        niche.revealed_by = revealed_by;
        Some(niche.clone())
    }

    fn active_niches(&self) -> impl Iterator<Item = &SpiritNiche> {
        self.niches.values().filter(|niche| !niche.revealed)
    }
}

pub fn register(app: &mut App) {
    app.init_resource::<CompanionProgress>();
    app.init_resource::<SparringInviteRegistry>();
    app.init_resource::<TradeOfferRegistry>();
    app.init_resource::<SpiritNicheRegistry>();
    high_renown_tracker::register(app);
    pvp_encounter::register(app);
    app.add_event::<PlayerChatCollected>();
    app.add_event::<SocialExposureEvent>();
    app.add_event::<SocialMentorshipEvent>();
    app.add_event::<SocialPactEvent>();
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_event::<FactionReputationDeltaEvent>();
    app.add_event::<SocialRelationshipEvent>();
    app.add_event::<SparringInviteRequest>();
    app.add_event::<SparringInviteResponseEvent>();
    app.add_event::<TradeOfferRequest>();
    app.add_event::<TradeOfferResponseEvent>();
    app.add_event::<FactionMembershipDecisionEvent>();
    app.add_event::<SpiritNichePlaceRequest>();
    app.add_event::<SpiritNicheRepairRequest>();
    app.add_event::<SpiritNicheCoordinateRevealRequest>();
    app.add_event::<SpiritNicheRevealRequest>();
    niche_defense::register(app);
    app.add_systems(
        Update,
        handle_spirit_niche_repair_requests
            .after(crate::network::client_request_handler::handle_client_request_payloads),
    );
    app.add_systems(
        Update,
        (
            attach_social_bundle_to_joined_clients
                .after(crate::player::attach_player_state_to_joined_clients)
                .after(crate::combat::CombatSystemSet::Intent),
            emit_anonymity_payloads_for_joined_clients
                .after(attach_social_bundle_to_joined_clients),
            hydrate_spirit_niche_registry,
            expose_chat_speakers.after(crate::network::chat_collector::collect_player_chat),
            handle_death_social_effects.after(crate::combat::CombatSystemSet::Resolve),
            handle_spirit_niche_place_requests
                .after(crate::network::client_request_handler::handle_client_request_payloads),
            handle_spirit_niche_coordinate_reveals
                .after(crate::network::client_request_handler::handle_client_request_payloads),
            detect_spirit_niche_break_attempts,
            apply_spirit_niche_reveals
                .after(detect_spirit_niche_break_attempts)
                .after(handle_spirit_niche_coordinate_reveals),
            update_companion_relationships.after(attach_social_bundle_to_joined_clients),
            handle_social_mentorships,
            handle_social_pacts,
            dispatch_sparring_invites,
            apply_faction_membership_decisions,
            apply_social_exposures
                .after(expose_chat_speakers)
                .after(handle_social_pacts),
            apply_social_relationships
                .after(handle_death_social_effects)
                .after(pvp_encounter::handle_pvp_encounter_events)
                .after(handle_social_mentorships)
                .after(update_companion_relationships)
                .after(handle_social_pacts),
            expire_companion_relationships.after(apply_social_relationships),
            apply_social_renown_deltas
                .after(handle_death_social_effects)
                .after(pvp_encounter::handle_pvp_encounter_events)
                .after(handle_social_pacts)
                .after(apply_faction_membership_decisions)
                .after(crate::cultivation::full_power_strike::full_power_kill_detection_system)
                .after(crate::npc::war::settle::award_war_winner_renown)
                .before(crate::identity::command::handle_identity_command),
            emit_niche_defense_server_data,
            publish_social_events
                .after(apply_social_exposures)
                .after(apply_social_relationships)
                .after(apply_social_renown_deltas)
                .after(apply_faction_reputation_deltas)
                .after(emit_niche_defense_server_data),
        ),
    );
    app.add_systems(
        Update,
        apply_faction_reputation_deltas.after(apply_social_renown_deltas),
    );
    app.add_systems(
        Update,
        (
            handle_sparring_invite_responses.after(dispatch_sparring_invites),
            dispatch_trade_offers,
            handle_trade_offer_responses
                .after(dispatch_trade_offers)
                .in_set(SocialSystemSet::TradeOfferResponse),
            expire_sparring_sessions.after(handle_sparring_invite_responses),
            expire_trade_offers.after(handle_trade_offer_responses),
        ),
    );
}

#[allow(clippy::type_complexity)]
fn attach_social_bundle_to_joined_clients(
    mut commands: Commands,
    persistence: Option<Res<PersistenceSettings>>,
    mut joined_clients: Query<
        (valence::prelude::Entity, Option<&mut Lifecycle>),
        (
            Or<(
                Added<Client>,
                Added<crate::cultivation::known_techniques::KnownTechniquesReconnectReady>,
            )>,
            Without<Anonymity>,
            Without<crate::cultivation::known_techniques::KnownTechniquesReconnectBlocked>,
        ),
    >,
) {
    for (entity, mut lifecycle) in &mut joined_clients {
        let social_state = persistence
            .as_deref()
            .zip(lifecycle.as_deref())
            .and_then(|(persistence, lifecycle)| {
                match load_social_components(persistence, lifecycle.character_id.as_str()) {
                    Ok(components) => Some(components),
                    Err(error) => {
                        tracing::warn!(
                            "[bong][social] failed to load social state for `{}`: {error}",
                            lifecycle.character_id
                        );
                        None
                    }
                }
            })
            .unwrap_or_default();

        let SocialComponentsSnapshot {
            anonymity,
            renown,
            faction_reputation,
            relationships,
            exposure_log,
            faction_membership,
            spirit_niche,
        } = social_state;
        let mut entity_commands = commands.entity(entity);
        entity_commands.insert((
            anonymity,
            renown,
            relationships,
            exposure_log,
            faction_reputation,
        ));
        if let Some(faction_membership) = faction_membership {
            entity_commands.insert(faction_membership);
        }
        if let Some(spirit_niche) = spirit_niche {
            if let Some(lifecycle) = lifecycle.as_deref_mut() {
                lifecycle.spawn_anchor_damaged = spirit_niche.is_damaged;
            }
            entity_commands.insert(spirit_niche);
        }
    }
}

#[allow(clippy::type_complexity)]
fn emit_anonymity_payloads_for_joined_clients(
    mut joined_clients: Query<(Entity, &mut Client, &Lifecycle), Added<Anonymity>>,
    all_clients: Query<(Entity, &Lifecycle, Option<&Anonymity>, Option<&Renown>), With<Client>>,
) {
    for (viewer_entity, mut client, viewer_lifecycle) in &mut joined_clients {
        if let Some(bytes) = serialize_social_anonymity_payload_for_viewer(
            viewer_entity,
            viewer_lifecycle,
            &all_clients,
        ) {
            send_server_data_payload(&mut client, bytes.as_slice());
        }
    }
}

#[allow(clippy::type_complexity)]
fn serialize_social_anonymity_payload_for_viewer(
    viewer_entity: Entity,
    viewer_lifecycle: &Lifecycle,
    all_clients: &Query<(Entity, &Lifecycle, Option<&Anonymity>, Option<&Renown>), With<Client>>,
) -> Option<Vec<u8>> {
    let remotes = build_remote_identity_payloads(viewer_entity, viewer_lifecycle, all_clients);
    let payload = ServerDataV1::new(ServerDataPayloadV1::SocialAnonymity(
        SocialAnonymityPayloadV1 {
            viewer: viewer_lifecycle.character_id.clone(),
            remotes,
        },
    ));
    let payload_type = payload_type_label(payload.payload_type());
    match serialize_server_data_payload(&payload) {
        Ok(bytes) => Some(bytes),
        Err(_) => {
            tracing::warn!("[bong][social] failed to serialize {payload_type} payload");
            None
        }
    }
}

#[allow(clippy::type_complexity)]
fn build_remote_identity_payloads(
    viewer_entity: Entity,
    viewer_lifecycle: &Lifecycle,
    all_clients: &Query<(Entity, &Lifecycle, Option<&Anonymity>, Option<&Renown>), With<Client>>,
) -> Vec<SocialRemoteIdentityV1> {
    let mut remotes = all_clients
        .iter()
        .filter_map(|(remote_entity, lifecycle, anonymity, renown)| {
            if remote_entity == viewer_entity {
                return None;
            }
            let is_exposed = anonymity
                .map(|anonymity| anonymity.is_exposed_to(&viewer_lifecycle.character_id))
                .unwrap_or(false);
            Some(SocialRemoteIdentityV1 {
                player_uuid: lifecycle.character_id.clone(),
                anonymous: !is_exposed,
                display_name: is_exposed.then(|| lifecycle.character_id.clone()),
                realm_band: None,
                breath_hint: Some(NAMELESS_LABEL.to_string()),
                renown_tags: renown
                    .map(|renown| {
                        renown
                            .top_tags(0, 5)
                            .into_iter()
                            .map(|tag| tag.tag)
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();
    remotes.sort_by(|left, right| left.player_uuid.cmp(&right.player_uuid));
    remotes
}

fn hydrate_spirit_niche_registry(
    persistence: Option<Res<PersistenceSettings>>,
    mut registry: ResMut<SpiritNicheRegistry>,
) {
    if registry.hydrated {
        return;
    }
    registry.hydrated = true;
    let Some(persistence) = persistence.as_deref() else {
        return;
    };
    match load_all_social_spirit_niches(persistence) {
        Ok(niches) => {
            for niche in niches {
                registry.upsert(niche);
            }
        }
        Err(error) => {
            tracing::warn!("[bong][social] failed to hydrate spirit niche registry: {error}");
        }
    }
}

#[allow(clippy::type_complexity)]
fn expose_chat_speakers(
    mut chats: EventReader<PlayerChatCollected>,
    players: Query<(Entity, &Lifecycle, &Position), With<Client>>,
    mut exposures: EventWriter<SocialExposureEvent>,
) {
    for chat in chats.read() {
        let Ok((speaker_entity, speaker_lifecycle, speaker_pos)) = players.get(chat.entity) else {
            continue;
        };
        let witnesses = nearby_player_char_ids(
            speaker_entity,
            speaker_lifecycle.character_id.as_str(),
            speaker_pos,
            CHAT_EXPOSURE_RADIUS,
            &players,
        );
        if witnesses.is_empty() {
            continue;
        }
        exposures.send(SocialExposureEvent {
            actor: chat.char_id.clone(),
            kind: ExposureKindV1::Chat,
            witnesses,
            tick: chat.timestamp,
            zone: Some(chat.zone.clone()),
        });
    }
}

#[allow(clippy::type_complexity)]
fn handle_death_social_effects(
    mut deaths: EventReader<DeathEvent>,
    players: Query<
        (
            Entity,
            &Lifecycle,
            &Position,
            Option<&Renown>,
            Option<&PlayerIdentities>,
        ),
        With<Client>,
    >,
    witness_players: Query<(Entity, &Lifecycle, &Position), With<Client>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut exposures: EventWriter<SocialExposureEvent>,
    mut relationships: EventWriter<SocialRelationshipEvent>,
    mut renown_deltas: EventWriter<SocialRenownDeltaEvent>,
) {
    let zone_registry = zone_registry
        .as_deref()
        .cloned()
        .unwrap_or_else(ZoneRegistry::fallback);
    for death in deaths.read() {
        let Ok((victim_entity, victim_lifecycle, victim_pos, victim_renown, _)) =
            players.get(death.target)
        else {
            continue;
        };
        let victim_char = victim_lifecycle.character_id.clone();
        let mut witnesses = HashSet::new();
        if let Some(attacker_entity) = death.attacker {
            if let Ok((_, attacker_lifecycle, _, attacker_renown, attacker_identities)) =
                players.get(attacker_entity)
            {
                witnesses.insert(attacker_lifecycle.character_id.clone());
                if attacker_lifecycle.character_id != victim_char {
                    relationships.send(SocialRelationshipEvent {
                        left: attacker_lifecycle.character_id.clone(),
                        right: victim_char.clone(),
                        left_kind: RelationshipKindV1::Feud,
                        right_kind: RelationshipKindV1::Feud,
                        tick: death.at_tick,
                        metadata: serde_json::json!({
                            "cause": death.cause,
                            "place": zone_name_for_position(&zone_registry, victim_pos),
                        }),
                    });
                    if victim_renown.map(|renown| renown.fame).unwrap_or_default()
                        > attacker_renown
                            .map(|renown| renown.fame)
                            .unwrap_or_default()
                    {
                        renown_deltas.send(SocialRenownDeltaEvent {
                            char_id: attacker_lifecycle.character_id.clone(),
                            identity_id: attacker_identities
                                .and_then(|identities| identities.active().map(|active| active.id)),
                            fame_delta: 0,
                            notoriety_delta: 10,
                            tags_added: Vec::new(),
                            tick: death.at_tick,
                            reason: "pk_death_higher_fame_victim".to_string(),
                        });
                    }
                }
            }
        }
        witnesses.extend(nearby_player_char_ids(
            victim_entity,
            victim_char.as_str(),
            victim_pos,
            DEATH_EXPOSURE_RADIUS,
            &witness_players,
        ));
        let witnesses = sorted_witnesses(witnesses);
        if !witnesses.is_empty() {
            exposures.send(SocialExposureEvent {
                actor: victim_char,
                kind: ExposureKindV1::Death,
                witnesses,
                tick: death.at_tick,
                zone: Some(zone_name_for_position(&zone_registry, victim_pos)),
            });
        }
    }
}

#[allow(clippy::type_complexity)]
fn apply_social_exposures(
    persistence: Option<Res<PersistenceSettings>>,
    mut exposures: EventReader<SocialExposureEvent>,
    mut social_sets: ParamSet<(
        Query<(&Lifecycle, &mut Anonymity, &mut ExposureLog), With<Client>>,
        Query<(Entity, &Lifecycle, Option<&Anonymity>, Option<&Renown>), With<Client>>,
        Query<(Entity, &Lifecycle, &mut Client), With<Client>>,
    )>,
) {
    for exposure in exposures.read() {
        let recipient_char_ids = social_exposure_recipient_char_ids(exposure);
        let mut actor_was_online = false;
        {
            let mut players = social_sets.p0();
            if let Some((_, mut anonymity, mut log)) = players
                .iter_mut()
                .find(|(lifecycle, _, _)| lifecycle.character_id == exposure.actor)
            {
                actor_was_online = true;
                anonymity.expose_to(exposure.witnesses.clone());
                log.0.push(ExposureEvent {
                    tick: exposure.tick,
                    kind: exposure.kind,
                    witnesses: exposure.witnesses.clone(),
                });
                if let Some(persistence) = persistence.as_deref() {
                    if let Err(error) =
                        persist_social_anonymity(persistence, exposure.actor.as_str(), &anonymity)
                    {
                        tracing::warn!(
                            "[bong][social] failed to persist anonymity for `{}`: {error}",
                            exposure.actor
                        );
                    }
                    if let Err(error) = persist_social_exposure(persistence, exposure) {
                        tracing::warn!(
                            "[bong][social] failed to persist exposure for `{}`: {error}",
                            exposure.actor
                        );
                    }
                }
            }
        }

        if !actor_was_online {
            if let Some(persistence) = persistence.as_deref() {
                match load_social_anonymity_from_persistence(persistence, exposure.actor.as_str()) {
                    Ok(mut anonymity) => {
                        anonymity.expose_to(exposure.witnesses.clone());
                        if let Err(error) = persist_social_anonymity(
                            persistence,
                            exposure.actor.as_str(),
                            &anonymity,
                        ) {
                            tracing::warn!(
                                "[bong][social] failed to persist offline anonymity for `{}`: {error}",
                                exposure.actor
                            );
                        }
                    }
                    Err(error) => {
                        tracing::warn!(
                            "[bong][social] failed to load anonymity for `{}` before exposure: {error}",
                            exposure.actor
                        );
                    }
                }
                if let Err(error) = persist_social_exposure(persistence, exposure) {
                    tracing::warn!(
                        "[bong][social] failed to persist offline exposure for `{}`: {error}",
                        exposure.actor
                    );
                }
            }
        }

        let payload =
            ServerDataV1::new(ServerDataPayloadV1::SocialExposure(SocialExposureEventV1 {
                v: 1,
                actor: exposure.actor.clone(),
                kind: exposure.kind,
                witnesses: exposure.witnesses.clone(),
                tick: exposure.tick,
                zone: exposure.zone.clone(),
            }));
        let Ok(bytes) = serialize_server_data_payload(&payload) else {
            tracing::warn!("[bong][social] failed to serialize social_exposure payload");
            continue;
        };

        let refresh_targets = {
            let clients = social_sets.p1();
            clients
                .iter()
                .filter_map(|(entity, lifecycle, _, _)| {
                    recipient_char_ids
                        .contains(&lifecycle.character_id)
                        .then_some(entity)
                })
                .collect::<Vec<_>>()
        };
        let anonymity_refreshes = {
            let clients = social_sets.p1();
            refresh_targets
                .into_iter()
                .filter_map(|viewer_entity| {
                    let Ok((viewer_entity, viewer_lifecycle, _, _)) = clients.get(viewer_entity)
                    else {
                        return None;
                    };
                    serialize_social_anonymity_payload_for_viewer(
                        viewer_entity,
                        viewer_lifecycle,
                        &clients,
                    )
                    .map(|bytes| (viewer_entity, bytes))
                })
                .collect::<Vec<_>>()
        };
        for (entity, lifecycle, mut client) in &mut social_sets.p2() {
            if recipient_char_ids.contains(&lifecycle.character_id) {
                send_server_data_payload(&mut client, bytes.as_slice());
            }
            if let Some((_, refresh_bytes)) = anonymity_refreshes
                .iter()
                .find(|(target_entity, _)| *target_entity == entity)
            {
                send_server_data_payload(&mut client, refresh_bytes.as_slice());
            }
        }
    }
}

fn social_exposure_recipient_char_ids(exposure: &SocialExposureEvent) -> HashSet<String> {
    let mut recipients = HashSet::with_capacity(exposure.witnesses.len() + 1);
    recipients.insert(exposure.actor.clone());
    recipients.extend(exposure.witnesses.iter().cloned());
    recipients
}

fn emit_niche_defense_server_data(
    mut intrusions: EventReader<NicheIntrusionEvent>,
    mut fatigues: EventReader<NicheGuardianFatigue>,
    mut broken_events: EventReader<NicheGuardianBroken>,
    mut clients: Query<(&Lifecycle, &mut Client), With<Client>>,
) {
    for intrusion in intrusions.read() {
        let payload =
            ServerDataV1::new(ServerDataPayloadV1::NicheIntrusion(NicheIntrusionEventV1 {
                v: 1,
                niche_pos: intrusion.niche_pos,
                intruder_id: intrusion.intruder_char_id.clone(),
                items_taken: intrusion.items_taken.clone(),
                taint_delta: intrusion.taint_delta,
            }));
        send_niche_payload_to_participants(
            &payload,
            intrusion.niche_owner.as_str(),
            Some(intrusion.intruder_char_id.as_str()),
            &mut clients,
        );
    }

    for fatigue in fatigues.read() {
        let payload = ServerDataV1::new(ServerDataPayloadV1::NicheGuardianFatigue(
            NicheGuardianFatigueV1 {
                v: 1,
                guardian_kind: guardian_kind_to_schema(fatigue.guardian_kind),
                charges_remaining: fatigue.charges_remaining,
            },
        ));
        send_niche_payload_to_participants(
            &payload,
            fatigue.niche_owner.as_str(),
            None,
            &mut clients,
        );
    }

    for broken in broken_events.read() {
        let payload = ServerDataV1::new(ServerDataPayloadV1::NicheGuardianBroken(
            NicheGuardianBrokenV1 {
                v: 1,
                guardian_kind: guardian_kind_to_schema(broken.guardian_kind),
                intruder_id: broken.intruder_char_id.clone(),
            },
        ));
        send_niche_payload_to_participants(
            &payload,
            broken.niche_owner.as_str(),
            Some(broken.intruder_char_id.as_str()),
            &mut clients,
        );
    }
}

fn send_niche_payload_to_participants(
    payload: &ServerDataV1,
    owner: &str,
    intruder: Option<&str>,
    clients: &mut Query<(&Lifecycle, &mut Client), With<Client>>,
) {
    let Ok(bytes) = serialize_server_data_payload(payload) else {
        tracing::warn!(
            "[bong][social][niche-defense] failed to serialize {} payload",
            payload_type_label(payload.payload.payload_type())
        );
        return;
    };
    for (lifecycle, mut client) in clients.iter_mut() {
        if lifecycle.character_id == owner
            || intruder.is_some_and(|id| id == lifecycle.character_id)
        {
            send_server_data_payload(&mut client, bytes.as_slice());
        }
    }
}

fn guardian_kind_to_schema(kind: self::components::GuardianKind) -> GuardianKindV1 {
    match kind {
        self::components::GuardianKind::Puppet => GuardianKindV1::Puppet,
        self::components::GuardianKind::ZhenfaTrap => GuardianKindV1::ZhenfaTrap,
        self::components::GuardianKind::BondedDaoxiang => GuardianKindV1::BondedDaoxiang,
    }
}

fn handle_social_pacts(
    mut pacts: EventReader<SocialPactEvent>,
    players: Query<(&Lifecycle, &PlayerIdentities), With<Client>>,
    mut exposures: EventWriter<SocialExposureEvent>,
    mut relationships: EventWriter<SocialRelationshipEvent>,
    mut renown_deltas: EventWriter<SocialRenownDeltaEvent>,
) {
    for pact in pacts.read() {
        if pact.left == pact.right {
            continue;
        }
        relationships.send(SocialRelationshipEvent {
            left: pact.left.clone(),
            right: pact.right.clone(),
            left_kind: RelationshipKindV1::Pact,
            right_kind: RelationshipKindV1::Pact,
            tick: pact.tick,
            metadata: serde_json::json!({
                "source": "pact",
                "terms": pact.terms.clone(),
                "broken": pact.broken,
                "breaker": pact.breaker.clone(),
            }),
        });

        let left_witnesses = pact_exposure_witnesses(
            pact.left.as_str(),
            pact.right.as_str(),
            pact.witnesses.as_slice(),
        );
        if !left_witnesses.is_empty() {
            exposures.send(SocialExposureEvent {
                actor: pact.left.clone(),
                kind: ExposureKindV1::Trade,
                witnesses: left_witnesses,
                tick: pact.tick,
                zone: None,
            });
        }
        let right_witnesses = pact_exposure_witnesses(
            pact.right.as_str(),
            pact.left.as_str(),
            pact.witnesses.as_slice(),
        );
        if !right_witnesses.is_empty() {
            exposures.send(SocialExposureEvent {
                actor: pact.right.clone(),
                kind: ExposureKindV1::Trade,
                witnesses: right_witnesses,
                tick: pact.tick,
                zone: None,
            });
        }

        if !pact.broken {
            continue;
        }
        let Some(breaker) = pact.breaker.as_ref() else {
            continue;
        };
        if breaker != &pact.left && breaker != &pact.right {
            continue;
        }
        renown_deltas.send(SocialRenownDeltaEvent {
            char_id: breaker.clone(),
            identity_id: active_identity_id_for_char(breaker, &players),
            fame_delta: 0,
            notoriety_delta: 50,
            tags_added: vec![RenownTagV1 {
                tag: "背盟者".to_string(),
                weight: 50.0,
                last_seen_tick: pact.tick,
                permanent: true,
            }],
            tick: pact.tick,
            reason: "pact_broken".to_string(),
        });
    }
}

fn active_identity_id_for_char(
    char_id: &str,
    players: &Query<(&Lifecycle, &PlayerIdentities), With<Client>>,
) -> Option<IdentityId> {
    players
        .iter()
        .find(|(lifecycle, _)| lifecycle.character_id == char_id)
        .and_then(|(_, identities)| identities.active().map(|active| active.id))
}

fn handle_social_mentorships(
    mut mentorships: EventReader<SocialMentorshipEvent>,
    mut relationships: EventWriter<SocialRelationshipEvent>,
) {
    for mentorship in mentorships.read() {
        if mentorship.master == mentorship.disciple {
            continue;
        }
        relationships.send(SocialRelationshipEvent {
            left: mentorship.master.clone(),
            right: mentorship.disciple.clone(),
            left_kind: RelationshipKindV1::Master,
            right_kind: RelationshipKindV1::Disciple,
            tick: mentorship.tick,
            metadata: serde_json::json!({
                "source": mentorship.source.clone(),
                "technique_hint": mentorship.technique_hint.clone(),
            }),
        });
    }
}

fn dispatch_sparring_invites(
    mut invites: EventReader<SparringInviteRequest>,
    mut registry: ResMut<SparringInviteRegistry>,
    mut players: Query<(Entity, &Lifecycle, Option<&Cultivation>, &mut Client), With<Client>>,
) {
    for invite in invites.read() {
        if invite.initiator == invite.target {
            continue;
        }
        let mut initiator_row = None;
        let mut target_row = None;
        for (entity, lifecycle, cultivation, _) in &mut players {
            if lifecycle.state == LifecycleState::Terminated {
                continue;
            }
            if entity == invite.initiator {
                initiator_row =
                    Some((lifecycle.character_id.clone(), cultivation.map(|c| c.realm)));
            } else if entity == invite.target {
                target_row = Some(lifecycle.character_id.clone());
            }
        }
        let (Some((initiator, initiator_realm)), Some(target)) = (initiator_row, target_row) else {
            continue;
        };
        let invite_id = format!("sparring:{}", Uuid::now_v7());
        let payload = ServerDataV1::new(ServerDataPayloadV1::SparringInvite(
            SparringInvitePayloadV1 {
                invite_id: invite_id.clone(),
                initiator,
                target,
                realm_band: initiator_realm
                    .map(realm_band_label)
                    .unwrap_or_else(|| "unknown".to_string()),
                breath_hint: "气息相试".to_string(),
                terms: invite.terms.clone(),
                expires_at_ms: current_unix_millis().saturating_add(SPARRING_INVITE_TIMEOUT_MS),
            },
        ));
        let Ok(bytes) = serialize_server_data_payload(&payload) else {
            tracing::warn!("[bong][social] failed to serialize sparring_invite payload");
            continue;
        };
        if let Ok((_, _, _, mut client)) = players.get_mut(invite.target) {
            send_server_data_payload(&mut client, bytes.as_slice());
            registry.pending.insert(
                invite_id,
                PendingSparringInvite {
                    initiator: invite.initiator,
                    target: invite.target,
                    created_at_tick: invite.tick,
                },
            );
        }
    }
}

fn handle_sparring_invite_responses(
    mut responses: EventReader<SparringInviteResponseEvent>,
    mut registry: ResMut<SparringInviteRegistry>,
    mut commands: Commands,
    players: Query<(Entity, &Lifecycle, Option<&SparringState>), With<Client>>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    for response in responses.read() {
        let Some(pending_target) = registry
            .pending
            .get(response.invite_id.as_str())
            .map(|pending| pending.target)
        else {
            if response.kind == SparringInviteResponseKind::Accept {
                tracing::warn!(
                    "[bong][social] rejected unknown sparring invite id `{}`",
                    response.invite_id
                );
            } else if let Ok(mut client) = clients.get_mut(response.player) {
                client.send_chat_message(match response.kind {
                    SparringInviteResponseKind::Decline => "切磋已拒绝",
                    SparringInviteResponseKind::Timeout => "切磋邀请已逾时",
                    SparringInviteResponseKind::Accept => "",
                });
            }
            continue;
        };
        if response.player != pending_target {
            tracing::warn!(
                "[bong][social] rejected sparring invite response from non-target {:?} for `{}`",
                response.player,
                response.invite_id
            );
            continue;
        }
        let Ok((target_entity, target_lifecycle, target_sparring)) = players.get(response.player)
        else {
            continue;
        };
        if target_lifecycle.state == LifecycleState::Terminated {
            continue;
        }
        if target_sparring.is_some() {
            continue;
        }
        if response.kind != SparringInviteResponseKind::Accept {
            registry.pending.remove(response.invite_id.as_str());
            if let Ok(mut client) = clients.get_mut(response.player) {
                client.send_chat_message(match response.kind {
                    SparringInviteResponseKind::Decline => "切磋已拒绝",
                    SparringInviteResponseKind::Timeout => "切磋邀请已逾时",
                    SparringInviteResponseKind::Accept => "",
                });
            }
            continue;
        }

        let pending = registry
            .pending
            .remove(response.invite_id.as_str())
            .expect("authenticated sparring invite must remain pending until consumption");
        if response.tick.saturating_sub(pending.created_at_tick) > SPARRING_INVITE_TIMEOUT_TICKS {
            if let Ok(mut client) = clients.get_mut(response.player) {
                client.send_chat_message("切磋邀请已过期");
            }
            continue;
        }

        let Ok((_, initiator_lifecycle, initiator_sparring)) = players.get(pending.initiator)
        else {
            if let Ok(mut client) = clients.get_mut(response.player) {
                client.send_chat_message("切磋发起者已离开");
            }
            continue;
        };
        if initiator_lifecycle.state == LifecycleState::Terminated || initiator_sparring.is_some() {
            if let Ok(mut client) = clients.get_mut(response.player) {
                client.send_chat_message("切磋发起者已不可应战");
            }
            continue;
        }
        let expires_at_tick = response.tick.saturating_add(SPARRING_MAX_TICKS);
        let state_for_target = SparringState {
            partner: pending.initiator,
            invite_id: response.invite_id.clone(),
            started_at_tick: response.tick,
            expires_at_tick,
        };
        let state_for_initiator = SparringState {
            partner: target_entity,
            invite_id: response.invite_id.clone(),
            started_at_tick: response.tick,
            expires_at_tick,
        };
        commands.entity(target_entity).insert(state_for_target);
        commands
            .entity(pending.initiator)
            .insert(state_for_initiator);
        if let Ok(mut target_client) = clients.get_mut(response.player) {
            target_client.send_chat_message("切磋开始：不掉装、不扣寿、不记死仇");
        }
        if let Ok(mut initiator_client) = clients.get_mut(pending.initiator) {
            initiator_client.send_chat_message("切磋开始：不掉装、不扣寿、不记死仇");
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn dispatch_trade_offers(
    mut requests: EventReader<TradeOfferRequest>,
    mut registry: ResMut<TradeOfferRegistry>,
    players: Query<
        (
            Entity,
            &Lifecycle,
            &Position,
            &PlayerInventory,
            Option<&PlayerIdentities>,
        ),
        With<Client>,
    >,
    mut clients: Query<&mut Client, With<Client>>,
) {
    for request in requests.read() {
        if request.initiator == request.target {
            continue;
        }
        let Ok((_, initiator_lifecycle, initiator_pos, initiator_inventory, initiator_identities)) =
            players.get(request.initiator)
        else {
            continue;
        };
        let Ok((_, target_lifecycle, target_pos, target_inventory, _)) =
            players.get(request.target)
        else {
            continue;
        };
        if initiator_lifecycle.state == LifecycleState::Terminated
            || target_lifecycle.state == LifecycleState::Terminated
            || initiator_pos.get().distance(target_pos.get()) > CHAT_EXPOSURE_RADIUS
        {
            continue;
        }
        let Some(offered_item) =
            trade_item_by_instance(initiator_inventory, request.offered_instance_id)
        else {
            continue;
        };
        let requested_items = trade_item_summaries(target_inventory);
        if requested_items.is_empty() {
            continue;
        }
        if initiator_identities
            .and_then(PlayerIdentities::active)
            .is_some_and(npc_should_decline_trade)
        {
            if let Ok(mut initiator_client) = clients.get_mut(request.initiator) {
                initiator_client.send_chat_message("对方听过这张面孔的事，不愿交易");
            }
            continue;
        }
        registry.pending.retain(|_, pending| {
            pending.initiator != request.initiator && pending.target != request.target
        });
        let offer_id = format!("trade:{}", Uuid::now_v7());
        let pending = PendingTradeOffer {
            initiator: request.initiator,
            target: request.target,
            initiator_char_id: initiator_lifecycle.character_id.clone(),
            target_char_id: target_lifecycle.character_id.clone(),
            offered_instance_id: request.offered_instance_id,
            offered_item: trade_item_summary(&offered_item),
            expires_at_tick: request.tick.saturating_add(TRADE_OFFER_TIMEOUT_TICKS),
        };
        let payload = ServerDataV1::new(ServerDataPayloadV1::TradeOffer(TradeOfferPayloadV1 {
            offer_id: offer_id.clone(),
            initiator: pending.initiator_char_id.clone(),
            target: pending.target_char_id.clone(),
            offered_item: pending.offered_item.clone(),
            requested_items,
            expires_at_ms: current_unix_millis().saturating_add(TRADE_OFFER_TIMEOUT_MS),
        }));
        let Ok(bytes) = serialize_server_data_payload(&payload) else {
            tracing::warn!("[bong][social] failed to serialize trade_offer payload");
            continue;
        };
        if let Ok(mut target_client) = clients.get_mut(request.target) {
            send_server_data_payload(&mut target_client, bytes.as_slice());
            registry.pending.insert(offer_id, pending);
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn handle_trade_offer_responses(
    mut responses: EventReader<TradeOfferResponseEvent>,
    mut registry: ResMut<TradeOfferRegistry>,
    mut players: Query<
        (
            Entity,
            &Lifecycle,
            &Position,
            &mut PlayerInventory,
            Option<&mut LifeRecord>,
        ),
        With<Client>,
    >,
    mut clients: Query<(&Username, &mut Client), With<Client>>,
    mut exposures: EventWriter<SocialExposureEvent>,
) {
    for response in responses.read() {
        let Some(pending) = registry.pending.get(response.offer_id.as_str()).cloned() else {
            continue;
        };
        if response.player != pending.target {
            continue;
        }
        let Some(pending) = registry.pending.remove(response.offer_id.as_str()) else {
            continue;
        };
        if !response.accepted {
            continue;
        }
        let Some(requested_instance_id) = response.requested_instance_id else {
            continue;
        };
        if response.tick > pending.expires_at_tick {
            if let Ok((_, mut client)) = clients.get_mut(response.player) {
                client.send_chat_message("交易邀请已过期");
            }
            continue;
        }

        let mut exchanged = false;
        if let Ok(
            [(_, initiator_lifecycle, initiator_pos, mut initiator_inventory, initiator_life_record), (_, target_lifecycle, target_pos, mut target_inventory, target_life_record)],
        ) = players.get_many_mut([pending.initiator, pending.target])
        {
            if initiator_lifecycle.state == LifecycleState::Terminated
                || target_lifecycle.state == LifecycleState::Terminated
                || initiator_lifecycle.character_id != pending.initiator_char_id
                || target_lifecycle.character_id != pending.target_char_id
                || initiator_pos.get().distance(target_pos.get()) > CHAT_EXPOSURE_RADIUS
            {
                continue;
            }
            let Some(offered_item) =
                trade_item_by_instance(&initiator_inventory, pending.offered_instance_id)
            else {
                continue;
            };
            let Some(requested_item) =
                trade_item_by_instance(&target_inventory, requested_instance_id)
            else {
                continue;
            };
            let mut next_initiator_inventory = initiator_inventory.clone();
            let mut next_target_inventory = target_inventory.clone();
            if let Err(error) = exchange_inventory_items(
                &mut next_initiator_inventory,
                pending.offered_instance_id,
                &mut next_target_inventory,
                requested_instance_id,
            ) {
                tracing::warn!(
                    "[bong][social] rejected trade offer {}: {error}",
                    response.offer_id
                );
                continue;
            }
            *initiator_inventory = next_initiator_inventory;
            *target_inventory = next_target_inventory;
            if let Some(mut life_record) = initiator_life_record {
                life_record.push(BiographyEntry::TradeCompleted {
                    counterparty_id: pending.target_char_id.clone(),
                    offered_item: offered_item.display_name.clone(),
                    received_item: requested_item.display_name.clone(),
                    tick: response.tick,
                });
            }
            if let Some(mut life_record) = target_life_record {
                life_record.push(BiographyEntry::TradeCompleted {
                    counterparty_id: pending.initiator_char_id.clone(),
                    offered_item: requested_item.display_name.clone(),
                    received_item: offered_item.display_name.clone(),
                    tick: response.tick,
                });
            }
            exchanged = true;
        }
        if !exchanged {
            continue;
        }

        exposures.send(SocialExposureEvent {
            actor: pending.initiator_char_id.clone(),
            kind: ExposureKindV1::Trade,
            witnesses: vec![pending.target_char_id.clone()],
            tick: response.tick,
            zone: None,
        });
        exposures.send(SocialExposureEvent {
            actor: pending.target_char_id.clone(),
            kind: ExposureKindV1::Trade,
            witnesses: vec![pending.initiator_char_id.clone()],
            tick: response.tick,
            zone: None,
        });
    }
}

fn trade_item_summary(item: &ItemInstance) -> TradeItemSummaryV1 {
    TradeItemSummaryV1 {
        instance_id: item.instance_id,
        item_id: item.template_id.clone(),
        display_name: item.display_name.clone(),
        stack_count: item.stack_count,
    }
}

fn trade_item_by_instance(inventory: &PlayerInventory, instance_id: u64) -> Option<ItemInstance> {
    for container in &inventory.containers {
        if let Some(placed) = container
            .items
            .iter()
            .find(|placed| placed.instance.instance_id == instance_id)
        {
            return Some(placed.instance.clone());
        }
    }
    inventory
        .hotbar
        .iter()
        .flatten()
        .find(|item| item.instance_id == instance_id)
        .cloned()
}

fn trade_item_summaries(inventory: &PlayerInventory) -> Vec<TradeItemSummaryV1> {
    let mut items = Vec::new();
    for container in &inventory.containers {
        for placed in &container.items {
            items.push(trade_item_summary(&placed.instance));
        }
    }
    for item in inventory.hotbar.iter().flatten() {
        items.push(trade_item_summary(item));
    }
    items.sort_by(|left, right| {
        left.display_name
            .cmp(&right.display_name)
            .then(left.instance_id.cmp(&right.instance_id))
    });
    items
}

fn expire_sparring_sessions(
    clock: Res<CombatClock>,
    mut registry: ResMut<SparringInviteRegistry>,
    mut commands: Commands,
    sessions: Query<(Entity, &SparringState)>,
) {
    registry.pending.retain(|_, pending| {
        clock.tick.saturating_sub(pending.created_at_tick) <= SPARRING_INVITE_TIMEOUT_TICKS
    });
    for (entity, session) in &sessions {
        if clock.tick >= session.expires_at_tick {
            commands.entity(entity).remove::<SparringState>();
        }
    }
}

fn expire_trade_offers(clock: Res<CombatClock>, mut registry: ResMut<TradeOfferRegistry>) {
    registry
        .pending
        .retain(|_, pending| clock.tick <= pending.expires_at_tick);
}

pub fn active_sparring_between(
    sessions: &Query<&SparringState>,
    left: Entity,
    right: Entity,
) -> Option<SparringState> {
    let left_state = sessions.get(left).ok()?;
    let right_state = sessions.get(right).ok()?;
    if left_state.partner == right
        && right_state.partner == left
        && left_state.invite_id == right_state.invite_id
    {
        return Some(left_state.clone());
    }
    None
}

pub fn conclude_sparring_defeat(
    commands: &mut Commands,
    status_effect_intents: &mut EventWriter<ApplyStatusEffectIntent>,
    loser: Entity,
    winner: Entity,
    tick: u64,
) {
    commands.entity(loser).remove::<SparringState>();
    commands.entity(winner).remove::<SparringState>();
    status_effect_intents.send(ApplyStatusEffectIntent {
        target: loser,
        kind: StatusEffectKind::Humility,
        magnitude: 0.3,
        duration_ticks: SPARRING_HUMILITY_TICKS,
        issued_at_tick: tick,
    });
}

fn apply_social_relationships(
    persistence: Option<Res<PersistenceSettings>>,
    mut events: EventReader<SocialRelationshipEvent>,
    mut players: Query<(&Lifecycle, Option<&Position>, &mut Relationships), With<Client>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
) {
    for event in events.read() {
        let left_relationship = Relationship {
            kind: event.left_kind,
            peer: event.right.clone(),
            since_tick: event.tick,
            metadata: event.metadata.clone(),
        };
        let right_relationship = Relationship {
            kind: event.right_kind,
            peer: event.left.clone(),
            since_tick: event.tick,
            metadata: event.metadata.clone(),
        };
        if let Some(persistence) = persistence.as_deref() {
            if let Err(error) =
                persist_social_relationship(persistence, event.left.as_str(), &left_relationship)
            {
                tracing::warn!(
                    "[bong][social] failed to persist relationship for `{}`: {error}",
                    event.left
                );
            }
            if let Err(error) =
                persist_social_relationship(persistence, event.right.as_str(), &right_relationship)
            {
                tracing::warn!(
                    "[bong][social] failed to persist relationship for `{}`: {error}",
                    event.right
                );
            }
        }

        let mut left_position = None;
        let mut right_position = None;
        for (lifecycle, position, mut relationships) in &mut players {
            if lifecycle.character_id == event.left {
                left_position = position.map(|position| position.get());
                relationships.upsert(left_relationship.clone());
            } else if lifecycle.character_id == event.right {
                right_position = position.map(|position| position.get());
                relationships.upsert(right_relationship.clone());
            }
        }
        emit_social_relationship_vfx(
            vfx_events.as_deref_mut(),
            event,
            left_position,
            right_position,
        );
    }
}

fn emit_social_relationship_vfx(
    events: Option<&mut Events<VfxEventRequest>>,
    event: &SocialRelationshipEvent,
    left_position: Option<DVec3>,
    right_position: Option<DVec3>,
) {
    let Some(events) = events else {
        return;
    };
    match event.left_kind {
        RelationshipKindV1::Pact => {
            if event
                .metadata
                .get("broken")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                return;
            }
            let (Some(left), Some(right)) = (left_position, right_position) else {
                return;
            };
            let midpoint = (left + right) * 0.5 + DVec3::new(0.0, 1.0, 0.0);
            gameplay_vfx::send_spawn(
                events,
                gameplay_vfx::spawn_request(
                    gameplay_vfx::SOCIAL_PACT_LINK,
                    midpoint,
                    Some([right.x - left.x, right.y - left.y, right.z - left.z]),
                    "#C4E0FF",
                    0.8,
                    8,
                    60,
                ),
            );
        }
        RelationshipKindV1::Feud => {
            for position in [left_position, right_position].into_iter().flatten() {
                gameplay_vfx::send_spawn(
                    events,
                    gameplay_vfx::spawn_request(
                        gameplay_vfx::SOCIAL_FEUD_MARK,
                        position + DVec3::new(0.0, 2.0, 0.0),
                        Some([0.0, 0.6, 0.0]),
                        "#FF3344",
                        0.85,
                        6,
                        20,
                    ),
                );
            }
        }
        _ => {}
    }
}

pub(crate) fn apply_social_renown_deltas(
    persistence: Option<Res<PersistenceSettings>>,
    mut events: EventReader<SocialRenownDeltaEvent>,
    mut players: Query<(&Lifecycle, &mut Renown, Option<&mut PlayerIdentities>), With<Client>>,
) {
    for event in events.read() {
        let mut persisted_renown = None;
        if let Some(persistence) = persistence.as_deref() {
            match load_social_renown_from_persistence(persistence, event.char_id.as_str()) {
                Ok(mut renown) => {
                    renown.apply_delta(
                        event.fame_delta,
                        event.notoriety_delta,
                        event.tags_added.clone(),
                    );
                    if let Err(error) =
                        persist_social_renown(persistence, event.char_id.as_str(), &renown)
                    {
                        tracing::warn!(
                            "[bong][social] failed to persist renown for `{}`: {error}",
                            event.char_id
                        );
                    }
                    persisted_renown = Some(renown);
                }
                Err(error) => {
                    tracing::warn!(
                        "[bong][social] failed to load renown for `{}` before delta: {error}",
                        event.char_id
                    );
                }
            }
        }

        let mut identity_synced = false;
        if let Some((_, mut renown, identities)) = players
            .iter_mut()
            .find(|(lifecycle, _, _)| lifecycle.character_id == event.char_id)
        {
            if let Some(persisted_renown) = persisted_renown.as_ref() {
                *renown = persisted_renown.clone();
            } else {
                renown.apply_delta(
                    event.fame_delta,
                    event.notoriety_delta,
                    event.tags_added.clone(),
                );
            }

            if let Some(mut identities) = identities {
                identity_synced =
                    apply_social_renown_delta_to_identity(event, &mut identities, "online");
                if identity_synced {
                    if let Some(persistence) = persistence.as_deref() {
                        if let Err(error) = identity_db::save_player_identities(
                            persistence,
                            event.char_id.as_str(),
                            &identities,
                        ) {
                            tracing::warn!(
                                "[bong][social] failed to persist identity renown for `{}`: {error}",
                                event.char_id
                            );
                        }
                    }
                }
            }
        }

        if let (false, Some(persistence)) = (identity_synced, persistence.as_deref()) {
            apply_social_renown_delta_to_persisted_identity(persistence, event);
        }
    }
}

fn apply_social_renown_delta_to_identity(
    event: &SocialRenownDeltaEvent,
    identities: &mut PlayerIdentities,
    source: &str,
) -> bool {
    let target = match event.identity_id {
        Some(identity_id) => {
            let Some(target) = identities.get_mut(identity_id) else {
                tracing::warn!(
                    "[bong][social] cannot bridge renown delta `{}` for `{}` from {source}: identity id {} missing",
                    event.reason,
                    event.char_id,
                    identity_id.0
                );
                return false;
            };
            target
        }
        None => {
            let Some(active) = identities.active_mut() else {
                tracing::warn!(
                    "[bong][social] cannot bridge renown delta `{}` for `{}` from {source}: active identity missing",
                    event.reason,
                    event.char_id
                );
                return false;
            };
            active
        }
    };
    target.renown.apply_delta(
        event.fame_delta,
        event.notoriety_delta,
        event.tags_added.clone(),
    );
    true
}

fn apply_social_renown_delta_to_persisted_identity(
    persistence: &PersistenceSettings,
    event: &SocialRenownDeltaEvent,
) {
    match identity_db::load_player_identities(persistence, event.char_id.as_str()) {
        Ok(Some(mut identities)) => {
            if apply_social_renown_delta_to_identity(event, &mut identities, "persistence") {
                if let Err(error) = identity_db::save_player_identities(
                    persistence,
                    event.char_id.as_str(),
                    &identities,
                ) {
                    tracing::warn!(
                        "[bong][social] failed to persist offline identity renown for `{}`: {error}",
                        event.char_id
                    );
                }
            }
        }
        Ok(None) => {
            tracing::warn!(
                "[bong][social] social renown delta `{}` for `{}` has no player_identities row; persisted only in social_renown",
                event.reason,
                event.char_id
            );
        }
        Err(error) => {
            tracing::warn!(
                "[bong][social] failed to load player identities for `{}` before renown bridge: {error}",
                event.char_id
            );
        }
    }
}

fn apply_faction_reputation_deltas(
    registry: Option<Res<NamedFactionRegistry>>,
    persistence: Option<Res<PersistenceSettings>>,
    mut events: EventReader<FactionReputationDeltaEvent>,
    mut players: Query<(&Lifecycle, &mut FactionReputation), With<Client>>,
) {
    for event in events.read() {
        if registry
            .as_deref()
            .and_then(|registry| registry.get(event.faction))
            .is_some_and(|faction| faction.status == FactionStatus::Decayed)
        {
            continue;
        }

        let mut persisted_reputation = None;
        if let Some(persistence) = persistence.as_deref() {
            match load_social_faction_reputation_from_persistence(
                persistence,
                event.char_id.as_str(),
            ) {
                Ok(mut reputation) => {
                    reputation.apply_delta(event.faction, event.delta);
                    if let Err(error) = persist_social_faction_reputation(
                        persistence,
                        event.char_id.as_str(),
                        &reputation,
                    ) {
                        tracing::warn!(
                            "[bong][social] failed to persist faction reputation for `{}`: {error}",
                            event.char_id
                        );
                    }
                    persisted_reputation = Some(reputation);
                }
                Err(error) => {
                    tracing::warn!(
                        "[bong][social] failed to load faction reputation for `{}` before delta: {error}",
                        event.char_id
                    );
                }
            }
        }

        let Some((_, mut reputation)) = players
            .iter_mut()
            .find(|(lifecycle, _)| lifecycle.character_id == event.char_id)
        else {
            continue;
        };
        if let Some(persisted_reputation) = persisted_reputation {
            *reputation = persisted_reputation;
        } else {
            reputation.apply_delta(event.faction, event.delta);
        }
    }
}

type FactionMembershipDecisionPlayer<'a> = (
    Entity,
    &'a Lifecycle,
    Option<&'a mut FactionMembership>,
    Option<&'a mut Karma>,
    Option<&'a PlayerIdentities>,
);

fn apply_faction_membership_decisions(
    persistence: Option<Res<PersistenceSettings>>,
    mut events: EventReader<FactionMembershipDecisionEvent>,
    mut commands: Commands,
    mut players: Query<FactionMembershipDecisionPlayer<'_>>,
    mut renown_deltas: EventWriter<SocialRenownDeltaEvent>,
) {
    for event in events.read() {
        let Ok((entity, lifecycle, membership, karma, identities)) = players.get_mut(event.player)
        else {
            continue;
        };
        if entity != event.player || lifecycle.state == LifecycleState::Terminated {
            continue;
        }
        let mut next_membership = membership
            .as_deref()
            .cloned()
            .or_else(|| {
                persistence
                    .as_deref()
                    .and_then(|persistence| {
                        load_social_faction_membership_from_persistence(
                            persistence,
                            lifecycle.character_id.as_str(),
                        )
                        .ok()
                    })
                    .flatten()
            })
            .unwrap_or(FactionMembership {
                faction: event.faction,
                named_faction: None,
                rank: 0,
                loyalty: 0,
                betrayal_count: 0,
                invite_block_until_tick: None,
                permanently_refused: false,
            });

        match event.kind {
            FactionMembershipDecisionKind::AcceptInvite => {
                if next_membership.permanently_refused
                    || next_membership
                        .invite_block_until_tick
                        .is_some_and(|until| until > event.tick)
                {
                    continue;
                }
                next_membership.faction = event.faction;
                next_membership.rank = 0;
                next_membership.loyalty = next_membership.loyalty.max(10);
                commands
                    .entity(event.player)
                    .insert(next_membership.clone());
            }
            FactionMembershipDecisionKind::Resign => {
                // Set faction to Neutral so that persisted record does not re-admit the player
                // on reconnect. The betrayal_count/invite_block/permanently_refused fields are
                // preserved so they can still gate future invite acceptance.
                next_membership.faction = FactionId::Neutral;
                next_membership.loyalty = next_membership.loyalty.saturating_sub(20);
                commands.entity(event.player).remove::<FactionMembership>();
            }
            FactionMembershipDecisionKind::Expel | FactionMembershipDecisionKind::Betray => {
                // Same: clear faction affiliation to Neutral rather than keeping the old faction,
                // so the persisted row cannot restore membership on reconnect.
                next_membership.faction = FactionId::Neutral;
                next_membership.loyalty = 0;
                next_membership.betrayal_count = next_membership.betrayal_count.saturating_add(1);
                next_membership.invite_block_until_tick =
                    Some(event.tick.saturating_add(FACTION_BETRAYAL_BLOCK_TICKS));
                if next_membership.betrayal_count >= FACTION_PERMANENT_REFUSAL_THRESHOLD {
                    next_membership.permanently_refused = true;
                }
                commands.entity(event.player).remove::<FactionMembership>();
                if let Some(mut karma) = karma {
                    karma.weight = (karma.weight + faction_betrayal_karma_delta(&next_membership))
                        .clamp(-1.0, 1.0);
                }
                renown_deltas.send(SocialRenownDeltaEvent {
                    char_id: lifecycle.character_id.clone(),
                    identity_id: identities
                        .and_then(|identities| identities.active().map(|active| active.id)),
                    fame_delta: 0,
                    notoriety_delta: 50,
                    tags_added: faction_betrayal_tags(&next_membership, event.tick),
                    tick: event.tick,
                    reason: "faction_betrayal".to_string(),
                });
            }
        }

        if let Some(persistence) = persistence.as_deref() {
            if let Err(error) = persist_social_faction_membership(
                persistence,
                lifecycle.character_id.as_str(),
                &next_membership,
            ) {
                tracing::warn!(
                    "[bong][social] failed to persist faction membership for `{}`: {error}",
                    lifecycle.character_id
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn handle_spirit_niche_place_requests(
    persistence: Option<Res<PersistenceSettings>>,
    player_persistence: Option<Res<PlayerStatePersistence>>,
    mut events: EventReader<SpiritNichePlaceRequest>,
    mut commands: Commands,
    mut players: Query<(
        Entity,
        &mut Lifecycle,
        &Position,
        Option<&mut PlayerInventory>,
        Option<&mut Cultivation>,
        Option<&mut LifespanComponent>,
        Option<&Username>,
        Option<&mut Client>,
        Option<&PlayerState>,
        Option<&CurrentDimension>,
        Option<&LifeRecord>,
    )>,
    mut zone_registry: Option<ResMut<ZoneRegistry>>,
    mut registry: ResMut<SpiritNicheRegistry>,
    mut layers: Query<&mut ChunkLayer, With<crate::world::dimension::OverworldLayer>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
    mut ledger: ResMut<WorldQiAccount>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
) {
    for event in events.read() {
        let Ok((
            entity,
            mut lifecycle,
            position,
            inventory,
            mut cultivation,
            lifespan,
            username,
            client,
            player_state,
            current_dimension,
            life_record,
        )) = players.get_mut(event.player)
        else {
            continue;
        };

        if entity != event.player || lifecycle.state == LifecycleState::Terminated {
            continue;
        }
        if !niche_place_target_is_close(position, event.pos) {
            tracing::warn!(
                "[bong][social] spirit niche place rejected for `{}`: target {:?} too far from player",
                lifecycle.character_id,
                event.pos
            );
            continue;
        }

        let Some(mut inventory) = inventory else {
            continue;
        };
        let Some(item_instance_id) = event.item_instance_id else {
            tracing::warn!(
                "[bong][social] spirit niche place rejected for `{}`: missing item instance",
                lifecycle.character_id
            );
            continue;
        };
        let Some(instance) = inventory_item_by_instance(&inventory, item_instance_id) else {
            tracing::warn!(
                "[bong][social] spirit niche place rejected for `{}`: missing instance {item_instance_id}",
                lifecycle.character_id
            );
            continue;
        };
        if instance.template_id != SPIRIT_NICHE_ITEM_TEMPLATE_ID {
            tracing::warn!(
                "[bong][social] spirit niche place rejected for `{}`: item `{}` is not a niche base",
                lifecycle.character_id,
                instance.template_id
            );
            continue;
        }
        if registry
            .active_niches()
            .any(|niche| niche.owner != lifecycle.character_id && niche.pos == event.pos)
        {
            tracing::warn!(
                "[bong][social] spirit niche place rejected for `{}`: target {:?} already occupied",
                lifecycle.character_id,
                event.pos
            );
            continue;
        }
        let mut staged_inventory = inventory.clone();
        if let Err(error) = consume_item_instance_once(&mut staged_inventory, item_instance_id) {
            tracing::warn!(
                "[bong][social] spirit niche place rejected for `{}`: consume failed: {error}",
                lifecycle.character_id
            );
            continue;
        }

        // Extract zone_qi as a scalar first (immutable borrow), then drop the borrow before
        // the mutable release call below.  Fallback registry has spirit_qi=0.9, so the cost
        // guard (zone_qi < 0) will be false and release_qi_amount_to_zone is not reached.
        let fallback_registry;
        let zone_registry_ref: &ZoneRegistry = match zone_registry.as_deref() {
            Some(r) => r,
            None => {
                fallback_registry = ZoneRegistry::fallback();
                &fallback_registry
            }
        };
        // 用玩家当前维度查 zone_qi（CodeRabbit #699 Major）：硬编码 Overworld 会让非主世界玩家
        // 按错维度判正/负灵域，与后续 release 用的 current_dimension 不一致 → 该扣不扣 / 回灌错维度。
        let lookup_dim = current_dimension
            .map(|d| d.0)
            .unwrap_or(crate::world::dimension::DimensionKind::Overworld);
        let zone_qi: Option<f64> = zone_registry_ref
            .find_zone(lookup_dim, position.get())
            .map(|z| z.spirit_qi);
        if let Some(cultivation) = cultivation.as_deref_mut() {
            if let Err(error) = apply_spirit_niche_negative_qi_cost(
                zone_qi,
                position,
                current_dimension,
                life_record,
                zone_registry.as_deref_mut(),
                &mut ledger,
                qi_transfers.as_deref_mut(),
                cultivation,
            ) {
                tracing::warn!(
                    ?error,
                    "[bong][social] spirit niche placement qi cost failed closed"
                );
                continue;
            }
        }
        *inventory = staged_inventory;
        if let Some(mut lifespan) = lifespan {
            apply_spirit_niche_negative_lifespan_cost(zone_qi, &mut lifespan);
        }

        let niche = SpiritNiche {
            owner: lifecycle.character_id.clone(),
            pos: event.pos,
            placed_at_tick: event.tick,
            revealed: false,
            revealed_by: None,
            is_damaged: false,
            guardians: Vec::new(),
        };
        lifecycle.spawn_anchor = Some(spirit_niche_spawn_anchor(event.pos));
        lifecycle.spawn_anchor_damaged = false;
        let old_niche = registry.niches.get(&lifecycle.character_id).cloned();
        registry.upsert(niche.clone());
        commands.entity(event.player).insert(niche.clone());
        if let Some(events) = vfx_events.as_deref_mut() {
            gameplay_vfx::send_spawn(
                events,
                gameplay_vfx::spawn_request(
                    gameplay_vfx::SOCIAL_NICHE_ESTABLISH,
                    gameplay_vfx::block_center(event.pos),
                    Some([0.0, 0.8, 0.0]),
                    "#C4E0FF",
                    0.8,
                    12,
                    60,
                ),
            );
        }
        if let Ok(mut layer) = layers.get_single_mut() {
            if let Some(old_niche) = old_niche {
                if !old_niche.revealed && old_niche.pos != event.pos {
                    layer.set_block(block_pos_from_array(old_niche.pos), BlockState::AIR);
                }
            }
            layer.set_block(block_pos_from_array(event.pos), BlockState::LODESTONE);
        }
        if let Some(persistence) = persistence.as_deref() {
            if let Err(error) = persist_social_spirit_niche(persistence, &niche) {
                tracing::warn!(
                    "[bong][social] failed to persist spirit niche for `{}`: {error}",
                    lifecycle.character_id
                );
            }
        }
        if let (Some(player_persistence), Some(username)) =
            (player_persistence.as_deref(), username)
        {
            if let Err(error) = save_player_shrine_anchor_slice(
                player_persistence,
                username.0.as_str(),
                Some(spirit_niche_spawn_anchor(event.pos)),
            ) {
                tracing::warn!(
                    "[bong][social] failed to persist shrine anchor for `{}`: {error}",
                    username.0
                );
            }
        }
        if let (Some(mut client), Some(username), Some(player_state), Some(cultivation)) =
            (client, username, player_state, cultivation.as_deref())
        {
            send_inventory_snapshot_to_client(
                event.player,
                &mut client,
                username.0.as_str(),
                &inventory,
                player_state,
                cultivation,
                "niche_base_consumed",
            );
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn handle_spirit_niche_repair_requests(
    persistence: Option<Res<PersistenceSettings>>,
    mut events: EventReader<SpiritNicheRepairRequest>,
    mut players: Query<(
        Entity,
        &mut Lifecycle,
        &Position,
        Option<&mut PlayerInventory>,
        Option<&mut SpiritNiche>,
    )>,
    mut registry: ResMut<SpiritNicheRegistry>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
    mut audio_events: Option<ResMut<Events<PlaySoundRecipeRequest>>>,
    mut narrations: Option<ResMut<PendingGameplayNarrations>>,
) {
    for event in events.read() {
        let Ok((entity, mut lifecycle, position, inventory, niche)) = players.get_mut(event.player)
        else {
            continue;
        };
        if entity != event.player || lifecycle.state == LifecycleState::Terminated {
            continue;
        }
        if !niche_place_target_is_close(position, event.pos) {
            tracing::warn!(
                "[bong][social] spirit niche repair rejected for `{}`: target {:?} too far from player",
                lifecycle.character_id,
                event.pos
            );
            continue;
        }

        let Some(mut niche) = niche else {
            tracing::warn!(
                "[bong][social] spirit niche repair rejected for `{}`: player has no niche component",
                lifecycle.character_id
            );
            continue;
        };
        if niche.owner != lifecycle.character_id || niche.pos != event.pos || niche.revealed {
            tracing::warn!(
                "[bong][social] spirit niche repair rejected for `{}`: target {:?} is not own active niche",
                lifecycle.character_id,
                event.pos
            );
            continue;
        }
        if !niche.is_damaged {
            tracing::warn!(
                "[bong][social] spirit niche repair rejected for `{}`: niche at {:?} is not damaged",
                lifecycle.character_id,
                event.pos
            );
            continue;
        }

        let Some(mut inventory) = inventory else {
            continue;
        };
        let Some(item_instance_id) = event.item_instance_id else {
            tracing::warn!(
                "[bong][social] spirit niche repair rejected for `{}`: missing item instance",
                lifecycle.character_id
            );
            continue;
        };
        let Some(instance) = inventory_item_by_instance(&inventory, item_instance_id) else {
            tracing::warn!(
                "[bong][social] spirit niche repair rejected for `{}`: missing instance {item_instance_id}",
                lifecycle.character_id
            );
            continue;
        };
        if instance.template_id != SPIRIT_NICHE_REPAIR_ITEM_TEMPLATE_ID {
            tracing::warn!(
                "[bong][social] spirit niche repair rejected for `{}`: item `{}` is not a repair kit",
                lifecycle.character_id,
                instance.template_id
            );
            continue;
        }
        if let Err(error) = consume_item_instance_once(&mut inventory, item_instance_id) {
            tracing::warn!(
                "[bong][social] spirit niche repair rejected for `{}`: consume failed: {error}",
                lifecycle.character_id
            );
            continue;
        }

        niche.is_damaged = false;
        lifecycle.spawn_anchor_damaged = false;
        let repaired_niche = niche.clone();
        registry.upsert(repaired_niche.clone());
        if let Some(persistence) = persistence.as_deref() {
            if let Err(error) = persist_social_spirit_niche(persistence, &repaired_niche) {
                tracing::warn!(
                    "[bong][social] failed to persist repaired spirit niche for `{}`: {error}",
                    lifecycle.character_id
                );
            }
        }

        let origin = gameplay_vfx::block_center(event.pos);
        if let Some(events) = vfx_events.as_deref_mut() {
            gameplay_vfx::send_spawn(
                events,
                gameplay_vfx::spawn_request(
                    gameplay_vfx::SOCIAL_NICHE_REPAIR,
                    origin,
                    Some([0.0, 0.4, 0.0]),
                    "#B8B0A0",
                    0.7,
                    6,
                    14,
                ),
            );
        }
        if let Some(audio_events) = audio_events.as_deref_mut() {
            audio_events.send(PlaySoundRecipeRequest {
                recipe_id: SPIRIT_NICHE_REPAIR_AUDIO_RECIPE_ID.to_string(),
                instance_id: 0,
                pos: Some(event.pos),
                flag: None,
                volume_mul: 1.0,
                pitch_shift: 0.0,
                recipient: AudioRecipient::Radius {
                    origin,
                    radius: AUDIO_AREA_RADIUS,
                },
            });
        }
        if let Some(narrations) = narrations.as_deref_mut() {
            let text = SPIRIT_NICHE_REPAIR_NARRATIONS
                [event.tick as usize % SPIRIT_NICHE_REPAIR_NARRATIONS.len()];
            narrations.push_player(&lifecycle.character_id, text, NarrationStyle::Perception);
        }
    }
}

fn detect_spirit_niche_break_attempts(
    mut digs: EventReader<DiggingEvent>,
    clients: Query<(&Lifecycle, Option<&Cultivation>), With<Client>>,
    registry: Option<Res<SpiritNicheRegistry>>,
    mut reveals: EventWriter<SpiritNicheRevealRequest>,
    mut attempts: EventWriter<NicheIntrusionAttempt>,
    clock: Option<Res<CombatClock>>,
) {
    let Some(registry) = registry.as_deref() else {
        return;
    };
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for event in digs.read() {
        if !matches!(event.state, DiggingState::Start | DiggingState::Stop) {
            continue;
        }
        let attempted = array_from_block_pos(event.position);
        let Some(niche) = registry
            .active_niches()
            .find(|niche| !niche.revealed && niche.pos == attempted)
        else {
            continue;
        };
        let Ok((observer_lifecycle, observer_cultivation)) = clients.get(event.client) else {
            continue;
        };
        if observer_lifecycle.character_id == niche.owner {
            continue;
        }
        reveals.send(SpiritNicheRevealRequest {
            observer: Some(event.client),
            owner: niche.owner.clone(),
            source: SpiritNicheRevealSource::BreakAttempt,
            tick,
        });
        attempts.send(NicheIntrusionAttempt {
            intruder: event.client,
            intruder_char_id: observer_lifecycle.character_id.clone(),
            niche_owner: niche.owner.clone(),
            niche_pos: niche.pos,
            items_taken: Vec::new(),
            intruder_qi_fraction: cultivation_qi_fraction(observer_cultivation),
            intruder_back_turned: false,
            tick,
        });
    }
}

fn cultivation_qi_fraction(cultivation: Option<&Cultivation>) -> f32 {
    let Some(cultivation) = cultivation else {
        return 1.0;
    };
    let effective_qi_max = (cultivation.qi_max - cultivation.qi_max_frozen.unwrap_or(0.0)).max(0.0);
    if !effective_qi_max.is_finite() || effective_qi_max <= f64::EPSILON {
        return 0.0;
    }
    (cultivation.qi_current / effective_qi_max).clamp(0.0, 1.0) as f32
}

fn handle_spirit_niche_coordinate_reveals(
    mut events: EventReader<SpiritNicheCoordinateRevealRequest>,
    observers: Query<(&Lifecycle, &Position), With<Client>>,
    registry: Option<Res<SpiritNicheRegistry>>,
    mut reveals: EventWriter<SpiritNicheRevealRequest>,
) {
    let Some(registry) = registry.as_deref() else {
        return;
    };
    for event in events.read() {
        let Ok((observer, observer_pos)) = observers.get(event.observer) else {
            continue;
        };
        if observer.state == LifecycleState::Terminated
            || !niche_place_target_is_close(observer_pos, event.pos)
        {
            continue;
        }
        let Some(niche) = registry
            .active_niches()
            .find(|niche| niche.pos == event.pos && niche.owner != observer.character_id)
        else {
            continue;
        };
        reveals.send(SpiritNicheRevealRequest {
            observer: Some(event.observer),
            owner: niche.owner.clone(),
            source: event.source,
            tick: event.tick,
        });
    }
}

#[allow(clippy::type_complexity)]
fn apply_spirit_niche_reveals(
    persistence: Option<Res<PersistenceSettings>>,
    player_persistence: Option<Res<PlayerStatePersistence>>,
    mut events: EventReader<SpiritNicheRevealRequest>,
    mut registry: ResMut<SpiritNicheRegistry>,
    mut players: ParamSet<(
        Query<&Lifecycle, With<Client>>,
        Query<(&mut Lifecycle, &mut SpiritNiche, Option<&mut Client>), With<Client>>,
    )>,
    mut layers: Query<&mut ChunkLayer, With<crate::world::dimension::OverworldLayer>>,
) {
    for event in events.read() {
        let observer_char = {
            let observers = players.p0();
            event
                .observer
                .and_then(|observer| observers.get(observer).ok())
                .map(|lifecycle| lifecycle.character_id.clone())
        };
        let Some(revealed_niche) = registry.reveal(&event.owner, observer_char.clone()) else {
            continue;
        };
        if let Some(persistence) = persistence.as_deref() {
            if let Err(error) = persist_social_spirit_niche(persistence, &revealed_niche) {
                tracing::warn!(
                    "[bong][social] failed to persist revealed spirit niche for `{}`: {error}",
                    event.owner
                );
            }
        }
        if let Some(player_persistence) = player_persistence.as_deref() {
            if let Some(username) = player_username_from_character_id(&event.owner) {
                if let Err(error) =
                    save_player_shrine_anchor_slice(player_persistence, username, None)
                {
                    tracing::warn!(
                        "[bong][social] failed to clear revealed shrine anchor for `{}`: {error}",
                        event.owner
                    );
                }
            }
        }
        if let Ok(mut layer) = layers.get_single_mut() {
            layer.set_block(block_pos_from_array(revealed_niche.pos), BlockState::AIR);
        }
        for (mut lifecycle, mut niche, client) in &mut players.p1() {
            if lifecycle.character_id != event.owner {
                continue;
            }
            *niche = revealed_niche.clone();
            lifecycle.spawn_anchor = None;
            lifecycle.spawn_anchor_damaged = false;
            if let Some(mut client) = client {
                client.send_chat_message("灵龛再无庇佑");
            }
        }
    }
}

fn niche_place_target_is_close(position: &Position, target: [i32; 3]) -> bool {
    distance_squared_to_niche(position.get(), target) <= SPIRIT_NICHE_RADIUS * SPIRIT_NICHE_RADIUS
}

#[allow(clippy::too_many_arguments)]
fn apply_spirit_niche_negative_qi_cost(
    zone_qi: Option<f64>,
    position: &Position,
    current_dimension: Option<&CurrentDimension>,
    life_record: Option<&LifeRecord>,
    zones: Option<&mut ZoneRegistry>,
    ledger: &mut WorldQiAccount,
    qi_transfers: Option<&mut Events<QiTransfer>>,
    cultivation: &mut Cultivation,
) -> Result<(), QiFlowError> {
    if !zone_qi.is_some_and(|qi| qi < 0.0) {
        return Ok(());
    }
    let realm_factor = match cultivation.realm {
        Realm::Awaken => 1.0,
        Realm::Induce => 1.25,
        Realm::Condense => 1.5,
        Realm::Solidify => 1.75,
        Realm::Spirit => 2.0,
        Realm::Void => 2.5,
    };
    let damage = cultivation.qi_max * SPIRIT_NICHE_NEGATIVE_QI_DAMAGE_RATIO * realm_factor;
    // 守恒（CodeRabbit #699 Critical）：qi_current clamp 到 0，实际扣减 = min(damage, qi_current)；
    // 必须只回灌实际扣减量，否则 qi_current<damage 时向 zone 多记、凭空创生真元。
    let actual_damage = damage.min(cultivation.qi_current.max(0.0));
    release_qi_amount_to_zone(
        cultivation,
        actual_damage,
        Some(position),
        current_dimension,
        life_record,
        zones,
        ledger,
        qi_transfers,
        "spirit_niche_penalty",
    )?;
    Ok(())
}

fn apply_spirit_niche_negative_lifespan_cost(
    zone_qi: Option<f64>,
    lifespan: &mut LifespanComponent,
) {
    let Some(zone_qi) = zone_qi else {
        return;
    };
    if zone_qi >= 0.0 {
        return;
    }
    lifespan.years_lived =
        (lifespan.years_lived + zone_qi.abs() * 0.1).min(lifespan.cap_by_realm as f64);
}

fn spirit_niche_spawn_anchor(pos: [i32; 3]) -> [f64; 3] {
    [
        pos[0] as f64 + 0.5,
        pos[1] as f64 + 1.0,
        pos[2] as f64 + 0.5,
    ]
}

fn block_pos_from_array(pos: [i32; 3]) -> BlockPos {
    BlockPos::new(pos[0], pos[1], pos[2])
}

fn array_from_block_pos(pos: BlockPos) -> [i32; 3] {
    [pos.x, pos.y, pos.z]
}

fn distance_squared_to_niche(pos: DVec3, niche_pos: [i32; 3]) -> f64 {
    let center = DVec3::new(
        niche_pos[0] as f64 + 0.5,
        niche_pos[1] as f64 + 0.5,
        niche_pos[2] as f64 + 0.5,
    );
    (pos - center).length_squared()
}

fn block_distance_squared(left: [i32; 3], right: [i32; 3]) -> f64 {
    let dx = f64::from(left[0] - right[0]);
    let dy = f64::from(left[1] - right[1]);
    let dz = f64::from(left[2] - right[2]);
    dx * dx + dy * dy + dz * dz
}

pub(crate) fn position_is_within_registered_active_spirit_niche(
    pos: DVec3,
    registry: &SpiritNicheRegistry,
) -> bool {
    registry.active_niches().any(|niche| {
        distance_squared_to_niche(pos, niche.pos) <= SPIRIT_NICHE_RADIUS * SPIRIT_NICHE_RADIUS
    })
}

pub(crate) fn block_break_is_protected_by_registered_spirit_niche(
    actor_char_id: Option<&str>,
    block_pos: [i32; 3],
    registry: &SpiritNicheRegistry,
) -> bool {
    registry.active_niches().any(|niche| {
        Some(niche.owner.as_str()) != actor_char_id
            && block_distance_squared(block_pos, niche.pos)
                <= SPIRIT_NICHE_RADIUS * SPIRIT_NICHE_RADIUS
    })
}

/// 玩家是否在自己灵龛的 5 格安全半径内（plan-identity-v1 P1 `WithinOwnNiche`）。
///
/// 仅匹配 `niche.owner == actor_char_id` 且 `!niche.revealed`（已被识破的灵龛
/// 失去安全语义，worldview §十一 "灵龛 = 安全空间" 仅指未暴露的灵龛）。
pub(crate) fn position_is_within_own_active_spirit_niche(
    actor_char_id: &str,
    pos: DVec3,
    registry: &SpiritNicheRegistry,
) -> bool {
    registry.active_niches().any(|niche| {
        niche.owner == actor_char_id
            && distance_squared_to_niche(pos, niche.pos)
                <= SPIRIT_NICHE_RADIUS * SPIRIT_NICHE_RADIUS
    })
}

#[allow(clippy::type_complexity)]
fn update_companion_relationships(
    clock: Res<CombatClock>,
    mut progress: ResMut<CompanionProgress>,
    players: Query<(Entity, &Lifecycle, &Position, &Relationships), With<Client>>,
    mut relationships: EventWriter<SocialRelationshipEvent>,
) {
    if clock.tick == 0 || !clock.tick.is_multiple_of(COMPANION_SCAN_INTERVAL_TICKS) {
        return;
    }

    let rows = players
        .iter()
        .filter_map(|(entity, lifecycle, position, relationships)| {
            if lifecycle.state == LifecycleState::Terminated {
                return None;
            }
            let companions = relationships
                .edges
                .iter()
                .filter(|relationship| relationship.kind == RelationshipKindV1::Companion)
                .map(|relationship| relationship.peer.clone())
                .collect::<HashSet<_>>();
            Some((
                entity,
                lifecycle.character_id.clone(),
                position.get(),
                companions,
            ))
        })
        .collect::<Vec<_>>();

    for (left_index, (_, left_char, left_pos, left_companions)) in rows.iter().enumerate() {
        for (_, right_char, right_pos, right_companions) in rows.iter().skip(left_index + 1) {
            if left_companions.contains(right_char) || right_companions.contains(left_char) {
                progress
                    .pair_seconds
                    .remove(&companion_pair_key(left_char, right_char));
                continue;
            }
            let delta = *left_pos - *right_pos;
            if delta.length_squared() > COMPANION_RADIUS * COMPANION_RADIUS {
                continue;
            }

            let pair_key = companion_pair_key(left_char, right_char);
            let seconds = progress
                .pair_seconds
                .entry(pair_key.clone())
                .or_default()
                .saturating_add(COMPANION_SCAN_INTERVAL_TICKS / 20);
            progress.pair_seconds.insert(pair_key.clone(), seconds);

            if seconds >= COMPANION_REQUIRED_SECONDS {
                progress.pair_seconds.remove(&pair_key);
                relationships.send(SocialRelationshipEvent {
                    left: left_char.clone(),
                    right: right_char.clone(),
                    left_kind: RelationshipKindV1::Companion,
                    right_kind: RelationshipKindV1::Companion,
                    tick: clock.tick,
                    metadata: serde_json::json!({
                        "source": "co_presence",
                        "accumulated_seconds": seconds,
                        "last_interaction_tick": clock.tick,
                    }),
                });
            }
        }
    }
}

fn expire_companion_relationships(
    clock: Res<CombatClock>,
    persistence: Option<Res<PersistenceSettings>>,
    mut players: Query<(&Lifecycle, &mut Relationships), With<Client>>,
) {
    if clock.tick == 0 {
        return;
    }

    let mut expired_pairs = HashSet::new();
    for (lifecycle, mut relationships) in &mut players {
        if lifecycle.state == LifecycleState::Terminated {
            continue;
        }
        let char_id = lifecycle.character_id.clone();
        relationships.edges.retain(|relationship| {
            if relationship.kind != RelationshipKindV1::Companion {
                return true;
            }
            let last_interaction_tick = companion_last_interaction_tick(relationship);
            let expired =
                clock.tick.saturating_sub(last_interaction_tick) >= COMPANION_EXPIRE_TICKS;
            if expired {
                expired_pairs.insert(companion_pair_key(&char_id, &relationship.peer));
            }
            !expired
        });
    }

    let Some(persistence) = persistence.as_deref() else {
        return;
    };
    for (left, right) in expired_pairs {
        if let Err(error) = delete_social_relationship(
            persistence,
            left.as_str(),
            right.as_str(),
            RelationshipKindV1::Companion,
        ) {
            tracing::warn!(
                "[bong][social] failed to delete expired companion `{}` -> `{}`: {error}",
                left,
                right
            );
        }
        if let Err(error) = delete_social_relationship(
            persistence,
            right.as_str(),
            left.as_str(),
            RelationshipKindV1::Companion,
        ) {
            tracing::warn!(
                "[bong][social] failed to delete expired companion `{}` -> `{}`: {error}",
                right,
                left
            );
        }
    }
}

#[derive(Debug, Clone, Default)]
struct SocialComponentsSnapshot {
    anonymity: Anonymity,
    renown: Renown,
    faction_reputation: FactionReputation,
    relationships: Relationships,
    exposure_log: ExposureLog,
    faction_membership: Option<FactionMembership>,
    spirit_niche: Option<SpiritNiche>,
}

fn load_social_components(
    persistence: &PersistenceSettings,
    char_id: &str,
) -> io::Result<SocialComponentsSnapshot> {
    let connection = open_social_connection(persistence)?;
    Ok(SocialComponentsSnapshot {
        anonymity: load_social_anonymity(&connection, char_id)?,
        renown: load_social_renown(&connection, char_id)?,
        faction_reputation: load_social_faction_reputation(&connection, char_id)?,
        relationships: load_social_relationships(&connection, char_id)?,
        exposure_log: load_social_exposure_log(&connection, char_id)?,
        faction_membership: load_social_faction_membership(&connection, char_id)?,
        spirit_niche: load_social_spirit_niche(&connection, char_id)?,
    })
}

fn open_social_connection(persistence: &PersistenceSettings) -> io::Result<Connection> {
    if let Some(parent) = persistence.db_path().parent() {
        fs::create_dir_all(parent)?;
    }
    let connection = Connection::open(persistence.db_path()).map_err(io::Error::other)?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")
        .map_err(io::Error::other)?;
    Ok(connection)
}

fn load_social_anonymity(connection: &Connection, char_id: &str) -> io::Result<Anonymity> {
    let row: Option<(Option<String>, String)> = connection
        .query_row(
            "SELECT displayed_name, exposed_to_json FROM social_anonymity WHERE char_id = ?1",
            params![char_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(io::Error::other)?;
    let Some((displayed_name, exposed_to_json)) = row else {
        return Ok(Anonymity::default());
    };
    let exposed_to = serde_json::from_str::<Vec<String>>(&exposed_to_json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        .into_iter()
        .collect();
    Ok(Anonymity {
        displayed_name,
        exposed_to,
    })
}

fn load_social_renown(connection: &Connection, char_id: &str) -> io::Result<Renown> {
    let row: Option<(i32, i32, String)> = connection
        .query_row(
            "SELECT fame, notoriety, tags_json FROM social_renown WHERE char_id = ?1",
            params![char_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(io::Error::other)?;
    let Some((fame, notoriety, tags_json)) = row else {
        return Ok(Renown::default());
    };
    let tags = serde_json::from_str::<Vec<RenownTagV1>>(&tags_json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(Renown {
        fame,
        notoriety,
        tags,
    })
}

fn load_social_anonymity_from_persistence(
    persistence: &PersistenceSettings,
    char_id: &str,
) -> io::Result<Anonymity> {
    let connection = open_social_connection(persistence)?;
    load_social_anonymity(&connection, char_id)
}

fn load_social_faction_membership_from_persistence(
    persistence: &PersistenceSettings,
    char_id: &str,
) -> io::Result<Option<FactionMembership>> {
    let connection = open_social_connection(persistence)?;
    load_social_faction_membership(&connection, char_id)
}

fn load_social_renown_from_persistence(
    persistence: &PersistenceSettings,
    char_id: &str,
) -> io::Result<Renown> {
    let connection = open_social_connection(persistence)?;
    load_social_renown(&connection, char_id)
}

fn load_social_faction_reputation_from_persistence(
    persistence: &PersistenceSettings,
    char_id: &str,
) -> io::Result<FactionReputation> {
    let connection = open_social_connection(persistence)?;
    load_social_faction_reputation(&connection, char_id)
}

fn load_social_faction_reputation(
    connection: &Connection,
    char_id: &str,
) -> io::Result<FactionReputation> {
    let mut statement = connection
        .prepare(
            "
            SELECT named_faction, score
            FROM social_faction_reputations
            WHERE char_id = ?1
            ORDER BY named_faction ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map(params![char_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        })
        .map_err(io::Error::other)?;
    let mut reputation = FactionReputation::default();
    for row in rows {
        let (faction_label, score) = row.map_err(io::Error::other)?;
        let faction = crate::npc::faction::NamedFactionId::from_str_name(faction_label.as_str())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "unknown named faction `{faction_label}` in social_faction_reputations"
                    ),
                )
            })?;
        reputation.per_faction.insert(
            faction,
            score.clamp(FactionReputation::MIN_SCORE, FactionReputation::MAX_SCORE),
        );
    }
    Ok(reputation)
}

fn load_social_relationships(connection: &Connection, char_id: &str) -> io::Result<Relationships> {
    let mut statement = connection
        .prepare(
            "
            SELECT peer_char_id, relationship_type, since_tick, metadata_json
            FROM social_relationships
            WHERE char_id = ?1
            ORDER BY peer_char_id ASC, relationship_type ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map(params![char_id], |row| {
            let kind_label: String = row.get(1)?;
            let metadata_json: String = row.get(3)?;
            let kind = parse_label::<RelationshipKindV1>(&kind_label).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            let metadata = serde_json::from_str(&metadata_json).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            Ok(Relationship {
                peer: row.get(0)?,
                kind,
                since_tick: sql_to_tick(row.get::<_, i64>(2)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
                metadata,
            })
        })
        .map_err(io::Error::other)?;
    let mut relationships = Relationships::default();
    for row in rows {
        relationships.edges.push(row.map_err(io::Error::other)?);
    }
    Ok(relationships)
}

fn load_social_exposure_log(connection: &Connection, char_id: &str) -> io::Result<ExposureLog> {
    let mut statement = connection
        .prepare(
            "
            SELECT kind, witnesses_json, at_tick
            FROM social_exposures
            WHERE char_id = ?1
            ORDER BY at_tick ASC, event_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map(params![char_id], |row| {
            let kind_label: String = row.get(0)?;
            let witnesses_json: String = row.get(1)?;
            let kind = parse_label::<ExposureKindV1>(&kind_label).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            let witnesses = serde_json::from_str(&witnesses_json).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            Ok(ExposureEvent {
                kind,
                witnesses,
                tick: sql_to_tick(row.get::<_, i64>(2)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
            })
        })
        .map_err(io::Error::other)?;
    let mut exposure_log = ExposureLog::default();
    for row in rows {
        exposure_log.0.push(row.map_err(io::Error::other)?);
    }
    Ok(exposure_log)
}

fn load_social_faction_membership(
    connection: &Connection,
    char_id: &str,
) -> io::Result<Option<FactionMembership>> {
    let row: Option<FactionMembershipSqlRow> = connection
        .query_row(
            "
            SELECT faction, named_faction, rank, loyalty, betrayal_count, invite_block_until_tick, permanently_refused
            FROM social_faction_memberships
            WHERE char_id = ?1
            ",
            params![char_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .optional()
        .map_err(io::Error::other)?;
    let Some((
        faction_label,
        named_faction_label,
        rank,
        loyalty,
        betrayal_count,
        invite_block_until_tick,
        permanently_refused,
    )) = row
    else {
        return Ok(None);
    };
    let faction = faction_label
        .as_deref()
        .and_then(FactionId::from_str_name)
        .unwrap_or(FactionId::Neutral);
    let named_faction = named_faction_label
        .as_deref()
        .and_then(crate::npc::faction::NamedFactionId::from_str_name);
    Ok(Some(FactionMembership {
        faction,
        named_faction,
        rank: u8::try_from(rank).unwrap_or_default(),
        loyalty: i32::try_from(loyalty).unwrap_or_default(),
        betrayal_count: u8::try_from(betrayal_count).unwrap_or_default(),
        invite_block_until_tick: invite_block_until_tick.and_then(|tick| u64::try_from(tick).ok()),
        permanently_refused: permanently_refused != 0,
    }))
}

fn load_social_spirit_niche(
    connection: &Connection,
    char_id: &str,
) -> io::Result<Option<SpiritNiche>> {
    let row: Option<SpiritNicheSqlRow> = connection
        .query_row(
            "
            SELECT pos_x, pos_y, pos_z, placed_at_tick, revealed, revealed_by, is_damaged, guardians_json
            FROM social_spirit_niches
            WHERE owner = ?1
            ",
            params![char_id],
            |row| {
                Ok((
                    [row.get(0)?, row.get(1)?, row.get(2)?],
                    sql_to_tick(row.get::<_, i64>(3)?).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Integer,
                            Box::new(error),
                        )
                    })?,
                    row.get::<_, i64>(4)? != 0,
                    row.get(5)?,
                    row.get::<_, i64>(6)? != 0,
                    row.get(7)?,
                ))
            },
        )
        .optional()
        .map_err(io::Error::other)?;
    row.map(
        |(
            pos,
            placed_at_tick,
            revealed,
            revealed_by,
            is_damaged,
            guardians_json,
        )| -> io::Result<SpiritNiche> {
            let guardians = decode_guardians_json(guardians_json.as_str()).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("invalid guardians_json for `{char_id}`: {error}"),
                )
            })?;
            Ok(SpiritNiche {
                owner: char_id.to_string(),
                pos,
                placed_at_tick,
                revealed,
                revealed_by,
                is_damaged,
                guardians,
            })
        },
    )
    .transpose()
}

fn load_all_social_spirit_niches(
    persistence: &PersistenceSettings,
) -> io::Result<Vec<SpiritNiche>> {
    let connection = open_social_connection(persistence)?;
    let mut statement = connection
        .prepare(
            "
            SELECT owner, pos_x, pos_y, pos_z, placed_at_tick, revealed, revealed_by, is_damaged, guardians_json
            FROM social_spirit_niches
            ORDER BY owner ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok(SpiritNiche {
                owner: row.get(0)?,
                pos: [row.get(1)?, row.get(2)?, row.get(3)?],
                placed_at_tick: sql_to_tick(row.get::<_, i64>(4)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
                revealed: row.get::<_, i64>(5)? != 0,
                revealed_by: row.get(6)?,
                is_damaged: row.get::<_, i64>(7)? != 0,
                guardians: decode_guardians_json(row.get::<_, String>(8)?.as_str()).map_err(
                    |error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            8,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    },
                )?,
            })
        })
        .map_err(io::Error::other)?;
    let mut niches = Vec::new();
    for row in rows {
        niches.push(row.map_err(io::Error::other)?);
    }
    Ok(niches)
}

fn decode_guardians_json(value: &str) -> io::Result<Vec<HouseGuardian>> {
    if value.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(value).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn encode_guardians_json(guardians: &[HouseGuardian]) -> io::Result<String> {
    serde_json::to_string(guardians)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn persist_social_anonymity(
    persistence: &PersistenceSettings,
    char_id: &str,
    anonymity: &Anonymity,
) -> io::Result<()> {
    let connection = open_social_connection(persistence)?;
    let mut exposed_to = anonymity.exposed_to.iter().cloned().collect::<Vec<_>>();
    exposed_to.sort();
    let exposed_to_json = serde_json::to_string(&exposed_to)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let wall_clock = current_unix_seconds();
    connection
        .execute(
            "
            INSERT INTO social_anonymity (
                char_id, displayed_name, exposed_to_json, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, 1, ?4)
            ON CONFLICT(char_id) DO UPDATE SET
                displayed_name = excluded.displayed_name,
                exposed_to_json = excluded.exposed_to_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                char_id,
                anonymity.displayed_name.as_deref(),
                exposed_to_json,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn persist_social_relationship(
    persistence: &PersistenceSettings,
    char_id: &str,
    relationship: &Relationship,
) -> io::Result<()> {
    let connection = open_social_connection(persistence)?;
    let kind = enum_label(relationship.kind)?;
    let metadata_json = serde_json::to_string(&relationship.metadata)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let wall_clock = current_unix_seconds();
    connection
        .execute(
            "
            INSERT INTO social_relationships (
                char_id, peer_char_id, relationship_type, since_tick, metadata_json,
                schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)
            ON CONFLICT(char_id, peer_char_id, relationship_type) DO UPDATE SET
                since_tick = excluded.since_tick,
                metadata_json = excluded.metadata_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                char_id,
                relationship.peer,
                kind,
                tick_to_sql(relationship.since_tick)?,
                metadata_json,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn delete_social_relationship(
    persistence: &PersistenceSettings,
    char_id: &str,
    peer_char_id: &str,
    kind: RelationshipKindV1,
) -> io::Result<()> {
    let connection = open_social_connection(persistence)?;
    let kind = enum_label(kind)?;
    connection
        .execute(
            "
            DELETE FROM social_relationships
            WHERE char_id = ?1 AND peer_char_id = ?2 AND relationship_type = ?3
            ",
            params![char_id, peer_char_id, kind],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn persist_social_exposure(
    persistence: &PersistenceSettings,
    exposure: &SocialExposureEvent,
) -> io::Result<()> {
    let connection = open_social_connection(persistence)?;
    let kind = enum_label(exposure.kind)?;
    let witnesses_json = serde_json::to_string(&exposure.witnesses)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let event_id = social_exposure_event_id(exposure, witnesses_json.as_str());
    let wall_clock = current_unix_seconds();
    connection
        .execute(
            "
            INSERT OR IGNORE INTO social_exposures (
                event_id, char_id, kind, witnesses_json, at_tick, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)
            ",
            params![
                event_id,
                exposure.actor,
                kind,
                witnesses_json,
                tick_to_sql(exposure.tick)?,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn persist_social_renown(
    persistence: &PersistenceSettings,
    char_id: &str,
    renown: &Renown,
) -> io::Result<()> {
    let connection = open_social_connection(persistence)?;
    let tags_json = serde_json::to_string(&renown.tags)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let wall_clock = current_unix_seconds();
    connection
        .execute(
            "
            INSERT INTO social_renown (
                char_id, fame, notoriety, tags_json, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, 1, ?5)
            ON CONFLICT(char_id) DO UPDATE SET
                fame = excluded.fame,
                notoriety = excluded.notoriety,
                tags_json = excluded.tags_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                char_id,
                renown.fame,
                renown.notoriety,
                tags_json,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn persist_social_faction_reputation(
    persistence: &PersistenceSettings,
    char_id: &str,
    reputation: &FactionReputation,
) -> io::Result<()> {
    let mut connection = open_social_connection(persistence)?;
    let wall_clock = current_unix_seconds();
    let transaction = connection.transaction().map_err(io::Error::other)?;
    transaction
        .execute(
            "DELETE FROM social_faction_reputations WHERE char_id = ?1",
            params![char_id],
        )
        .map_err(io::Error::other)?;
    for (faction, score) in &reputation.per_faction {
        if *score == 0 {
            continue;
        }
        transaction
            .execute(
                "
                INSERT INTO social_faction_reputations (
                    char_id, named_faction, score, schema_version, last_updated_wall
                ) VALUES (?1, ?2, ?3, 1, ?4)
                ",
                params![char_id, faction.as_str(), score, wall_clock],
            )
            .map_err(io::Error::other)?;
    }
    transaction.commit().map_err(io::Error::other)?;
    Ok(())
}

fn persist_social_faction_membership(
    persistence: &PersistenceSettings,
    char_id: &str,
    membership: &FactionMembership,
) -> io::Result<()> {
    let connection = open_social_connection(persistence)?;
    let wall_clock = current_unix_seconds();
    connection
        .execute(
            "
            INSERT INTO social_faction_memberships (
                char_id, faction, named_faction, rank, loyalty, betrayal_count, invite_block_until_tick,
                permanently_refused, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9)
            ON CONFLICT(char_id) DO UPDATE SET
                faction = excluded.faction,
                named_faction = excluded.named_faction,
                rank = excluded.rank,
                loyalty = excluded.loyalty,
                betrayal_count = excluded.betrayal_count,
                invite_block_until_tick = excluded.invite_block_until_tick,
                permanently_refused = excluded.permanently_refused,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                char_id,
                membership.faction.as_str(),
                membership.named_faction.map(|faction| faction.as_str()),
                i64::from(membership.rank),
                i64::from(membership.loyalty),
                i64::from(membership.betrayal_count),
                membership
                    .invite_block_until_tick
                    .map(tick_to_sql)
                    .transpose()?,
                if membership.permanently_refused {
                    1_i64
                } else {
                    0_i64
                },
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn persist_social_spirit_niche(
    persistence: &PersistenceSettings,
    niche: &SpiritNiche,
) -> io::Result<()> {
    let connection = open_social_connection(persistence)?;
    let wall_clock = current_unix_seconds();
    connection
        .execute(
            "
            INSERT INTO social_spirit_niches (
                owner, pos_x, pos_y, pos_z, placed_at_tick, revealed, revealed_by,
                is_damaged, guardians_json, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10)
            ON CONFLICT(owner) DO UPDATE SET
                pos_x = excluded.pos_x,
                pos_y = excluded.pos_y,
                pos_z = excluded.pos_z,
                placed_at_tick = excluded.placed_at_tick,
                revealed = excluded.revealed,
                revealed_by = excluded.revealed_by,
                is_damaged = excluded.is_damaged,
                guardians_json = excluded.guardians_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                niche.owner,
                niche.pos[0],
                niche.pos[1],
                niche.pos[2],
                tick_to_sql(niche.placed_at_tick)?,
                if niche.revealed { 1_i64 } else { 0_i64 },
                niche.revealed_by.as_deref(),
                if niche.is_damaged { 1_i64 } else { 0_i64 },
                encode_guardians_json(&niche.guardians)?,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn social_exposure_event_id(exposure: &SocialExposureEvent, witnesses_json: &str) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        exposure.actor,
        exposure.tick,
        enum_label(exposure.kind).unwrap_or_else(|_| "unknown".to_string()),
        exposure.zone.as_deref().unwrap_or_default(),
        witnesses_json
    )
}

fn enum_label<T>(value: T) -> io::Result<String>
where
    T: Serialize,
{
    serde_json::to_value(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "enum label must be a string"))
}

fn parse_label<T>(label: &str) -> io::Result<T>
where
    T: DeserializeOwned,
{
    serde_json::from_value(serde_json::Value::String(label.to_string()))
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn tick_to_sql(tick: u64) -> io::Result<i64> {
    i64::try_from(tick).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn sql_to_tick(value: i64) -> io::Result<u64> {
    u64::try_from(value).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn current_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn publish_social_events(
    redis: Option<Res<RedisBridgeResource>>,
    mut exposures: EventReader<SocialExposureEvent>,
    mut relationships: EventReader<SocialRelationshipEvent>,
    mut renown_deltas: EventReader<SocialRenownDeltaEvent>,
    mut intrusions: EventReader<NicheIntrusionEvent>,
    mut fatigues: EventReader<NicheGuardianFatigue>,
    mut broken_events: EventReader<NicheGuardianBroken>,
) {
    let Some(redis) = redis else {
        return;
    };
    for exposure in exposures.read() {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::SocialExposure(SocialExposureEventV1 {
                v: 1,
                actor: exposure.actor.clone(),
                kind: exposure.kind,
                witnesses: exposure.witnesses.clone(),
                tick: exposure.tick,
                zone: exposure.zone.clone(),
            }));
    }
    for relationship in relationships.read() {
        if relationship.left_kind == RelationshipKindV1::Feud {
            let _ = redis
                .tx_outbound
                .send(RedisOutbound::SocialFeud(SocialFeudEventV1 {
                    v: 1,
                    left: relationship.left.clone(),
                    right: relationship.right.clone(),
                    tick: relationship.tick,
                    place: relationship
                        .metadata
                        .get("place")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                }));
        } else if relationship.left_kind == RelationshipKindV1::Pact {
            let terms = relationship
                .metadata
                .get("terms")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string();
            let broken = relationship
                .metadata
                .get("broken")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let _ = redis
                .tx_outbound
                .send(RedisOutbound::SocialPact(SocialPactEventV1 {
                    v: 1,
                    left: relationship.left.clone(),
                    right: relationship.right.clone(),
                    terms,
                    tick: relationship.tick,
                    broken,
                }));
        }
    }
    for event in renown_deltas.read() {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::SocialRenownDelta(SocialRenownDeltaV1 {
                v: 1,
                char_id: event.char_id.clone(),
                fame_delta: event.fame_delta,
                notoriety_delta: event.notoriety_delta,
                tags_added: event.tags_added.clone(),
                tick: event.tick,
                reason: event.reason.clone(),
            }));
    }
    for intrusion in intrusions.read() {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::NicheIntrusion(NicheIntrusionEventV1 {
                v: 1,
                niche_pos: intrusion.niche_pos,
                intruder_id: intrusion.intruder_char_id.clone(),
                items_taken: intrusion.items_taken.clone(),
                taint_delta: intrusion.taint_delta,
            }));
    }
    for fatigue in fatigues.read() {
        let _ = redis.tx_outbound.send(RedisOutbound::NicheGuardianFatigue(
            NicheGuardianFatigueV1 {
                v: 1,
                guardian_kind: guardian_kind_to_schema(fatigue.guardian_kind),
                charges_remaining: fatigue.charges_remaining,
            },
        ));
    }
    for broken in broken_events.read() {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::NicheGuardianBroken(NicheGuardianBrokenV1 {
                v: 1,
                guardian_kind: guardian_kind_to_schema(broken.guardian_kind),
                intruder_id: broken.intruder_char_id.clone(),
            }));
    }
}

fn nearby_player_char_ids(
    origin_entity: Entity,
    origin_char_id: &str,
    origin_pos: &Position,
    radius: f64,
    players: &Query<(Entity, &Lifecycle, &Position), With<Client>>,
) -> Vec<String> {
    let origin = origin_pos.get();
    let radius_sq = radius * radius;
    let mut witnesses = players
        .iter()
        .filter_map(|(entity, lifecycle, position)| {
            if entity == origin_entity
                || lifecycle.character_id == origin_char_id
                || lifecycle.state == LifecycleState::Terminated
            {
                return None;
            }
            let delta = position.get() - origin;
            (delta.length_squared() <= radius_sq).then(|| lifecycle.character_id.clone())
        })
        .collect::<Vec<_>>();
    witnesses.sort();
    witnesses.dedup();
    witnesses
}

fn sorted_witnesses(witnesses: HashSet<String>) -> Vec<String> {
    let mut witnesses = witnesses.into_iter().collect::<Vec<_>>();
    witnesses.sort();
    witnesses
}

fn pact_exposure_witnesses(actor: &str, peer: &str, witnesses: &[String]) -> Vec<String> {
    let mut all = HashSet::from([peer.to_string()]);
    for witness in witnesses {
        if witness != actor {
            all.insert(witness.clone());
        }
    }
    sorted_witnesses(all)
}

fn companion_last_interaction_tick(relationship: &Relationship) -> u64 {
    relationship
        .metadata
        .get("last_interaction_tick")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(relationship.since_tick)
}

fn realm_band_label(realm: Realm) -> String {
    match realm {
        Realm::Awaken | Realm::Induce => "awaken_induce",
        Realm::Condense | Realm::Solidify => "condense_solidify",
        Realm::Spirit | Realm::Void => "spirit_void",
    }
    .to_string()
}

fn faction_betrayal_karma_delta(membership: &FactionMembership) -> f64 {
    if membership.betrayal_count >= FACTION_PERMANENT_REFUSAL_THRESHOLD {
        1.0
    } else {
        0.5
    }
}

fn faction_betrayal_tags(membership: &FactionMembership, tick: u64) -> Vec<RenownTagV1> {
    let mut tags = vec![RenownTagV1 {
        tag: "叛门者".to_string(),
        weight: 50.0,
        last_seen_tick: tick,
        permanent: true,
    }];
    if membership.betrayal_count >= FACTION_PERMANENT_REFUSAL_THRESHOLD {
        tags.push(RenownTagV1 {
            tag: "三叛之人".to_string(),
            weight: 100.0,
            last_seen_tick: tick,
            permanent: true,
        });
    }
    tags
}

fn current_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn companion_pair_key(left: &str, right: &str) -> CompanionPairKey {
    if left <= right {
        (left.to_string(), right.to_string())
    } else {
        (right.to_string(), left.to_string())
    }
}

fn zone_name_for_position(zone_registry: &ZoneRegistry, position: &Position) -> String {
    zone_registry
        .find_zone(
            crate::world::dimension::DimensionKind::Overworld,
            position.get(),
        )
        .map(|zone| zone.name.clone())
        .unwrap_or_else(|| DEFAULT_SPAWN_ZONE_NAME.to_string())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
