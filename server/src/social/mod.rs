pub mod components;
pub mod events;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use valence::message::SendMessage;
use valence::prelude::bevy_ecs::system::ParamSet;
use valence::prelude::{
    bevy_ecs, Added, App, BlockPos, BlockState, ChunkLayer, Client, Commands, DVec3, DiggingEvent,
    DiggingState, Entity, EventReader, EventWriter, IntoSystemConfigs, Position, Query, Res,
    ResMut, Resource, Update, Username, With, Without,
};

use self::components::{
    Anonymity, ExposureEvent, ExposureLog, Relationship, Relationships, Renown, SpiritNiche,
};
use self::events::{
    PlayerChatCollected, SocialExposureEvent, SocialPactEvent, SocialRelationshipEvent,
    SocialRenownDeltaEvent, SpiritNicheCoordinateRevealRequest, SpiritNichePlaceRequest,
    SpiritNicheRevealRequest, SpiritNicheRevealSource,
};
use crate::combat::components::{Lifecycle, LifecycleState};
use crate::combat::events::DeathEvent;
use crate::combat::CombatClock;
use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::lifespan::LifespanComponent;
use crate::inventory::{consume_item_instance_once, inventory_item_by_instance, PlayerInventory};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::network::redis_bridge::RedisOutbound;
use crate::network::{send_server_data_payload, RedisBridgeResource};
use crate::persistence::PersistenceSettings;
use crate::player::state::{
    player_username_from_character_id, save_player_shrine_anchor_slice, PlayerState,
    PlayerStatePersistence,
};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::schema::social::{
    ExposureKindV1, RelationshipKindV1, RenownTagV1, SocialAnonymityPayloadV1,
    SocialExposureEventV1, SocialFeudEventV1, SocialPactEventV1, SocialRemoteIdentityV1,
    SocialRenownDeltaV1,
};
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

const CHAT_EXPOSURE_RADIUS: f64 = 50.0;
const DEATH_EXPOSURE_RADIUS: f64 = 50.0;
const COMPANION_RADIUS: f64 = 50.0;
const COMPANION_SCAN_INTERVAL_TICKS: u64 = 20;
const COMPANION_REQUIRED_SECONDS: u64 = 5 * 60 * 60;
const COMPANION_EXPIRE_TICKS: u64 = 30 * 24 * 60 * 60 * 20;
const SPIRIT_NICHE_ITEM_TEMPLATE_ID: &str = "spirit_niche_stone";
const SPIRIT_NICHE_RADIUS: f64 = 5.0;
const SPIRIT_NICHE_NEGATIVE_QI_DAMAGE_RATIO: f64 = 0.1;
const NAMELESS_LABEL: &str = "无名修士";

type CompanionPairKey = (String, String);
type SpiritNicheSqlRow = ([i32; 3], u64, bool, Option<String>, Option<String>);

#[derive(Debug, Default, Resource)]
struct CompanionProgress {
    pair_seconds: HashMap<CompanionPairKey, u64>,
}

#[derive(Debug, Default, Resource)]
pub(crate) struct SpiritNicheRegistry {
    niches: HashMap<String, SpiritNiche>,
    hydrated: bool,
}

impl SpiritNicheRegistry {
    fn upsert(&mut self, niche: SpiritNiche) {
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
    app.init_resource::<SpiritNicheRegistry>();
    app.add_event::<PlayerChatCollected>();
    app.add_event::<SocialExposureEvent>();
    app.add_event::<SocialPactEvent>();
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_event::<SocialRelationshipEvent>();
    app.add_event::<SpiritNichePlaceRequest>();
    app.add_event::<SpiritNicheCoordinateRevealRequest>();
    app.add_event::<SpiritNicheRevealRequest>();
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
            handle_social_pacts,
            apply_social_exposures
                .after(expose_chat_speakers)
                .after(handle_social_pacts),
            apply_social_relationships
                .after(handle_death_social_effects)
                .after(update_companion_relationships)
                .after(handle_social_pacts),
            expire_companion_relationships.after(apply_social_relationships),
            apply_social_renown_deltas
                .after(handle_death_social_effects)
                .after(handle_social_pacts),
            publish_social_events
                .after(apply_social_exposures)
                .after(apply_social_relationships)
                .after(apply_social_renown_deltas),
        ),
    );
}

#[allow(clippy::type_complexity)]
fn attach_social_bundle_to_joined_clients(
    mut commands: Commands,
    persistence: Option<Res<PersistenceSettings>>,
    joined_clients: Query<
        (valence::prelude::Entity, Option<&Lifecycle>),
        (Added<Client>, Without<Anonymity>),
    >,
) {
    for (entity, lifecycle) in &joined_clients {
        let social_state = lifecycle
            .and_then(|lifecycle| {
                persistence
                    .as_deref()
                    .map(|persistence| (persistence, lifecycle))
            })
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
            relationships,
            exposure_log,
            spirit_niche,
        } = social_state;
        let mut entity_commands = commands.entity(entity);
        entity_commands.insert((anonymity, renown, relationships, exposure_log));
        if let Some(spirit_niche) = spirit_niche {
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
        let remotes = build_remote_identity_payloads(viewer_entity, viewer_lifecycle, &all_clients);
        let payload = ServerDataV1::new(ServerDataPayloadV1::SocialAnonymity(
            SocialAnonymityPayloadV1 {
                viewer: viewer_lifecycle.character_id.clone(),
                remotes,
            },
        ));
        let payload_type = payload_type_label(payload.payload_type());
        let Ok(bytes) = serialize_server_data_payload(&payload) else {
            tracing::warn!("[bong][social] failed to serialize {payload_type} payload");
            continue;
        };
        send_server_data_payload(&mut client, bytes.as_slice());
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
    players: Query<(Entity, &Lifecycle, &Position, Option<&Renown>), With<Client>>,
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
        let Ok((victim_entity, victim_lifecycle, victim_pos, victim_renown)) =
            players.get(death.target)
        else {
            continue;
        };
        let victim_char = victim_lifecycle.character_id.clone();
        let mut witnesses = HashSet::new();
        if let Some(attacker_entity) = death.attacker {
            if let Ok((_, attacker_lifecycle, _, attacker_renown)) = players.get(attacker_entity) {
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

fn apply_social_exposures(
    persistence: Option<Res<PersistenceSettings>>,
    mut exposures: EventReader<SocialExposureEvent>,
    mut players: Query<(&Lifecycle, &mut Anonymity, &mut ExposureLog), With<Client>>,
    mut clients: Query<(&Lifecycle, &mut Client), With<Client>>,
) {
    for exposure in exposures.read() {
        let mut actor_was_online = false;
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
        for (lifecycle, mut client) in &mut clients {
            if lifecycle.character_id == exposure.actor
                || exposure
                    .witnesses
                    .iter()
                    .any(|witness| witness == &lifecycle.character_id)
            {
                send_server_data_payload(&mut client, bytes.as_slice());
            }
        }
    }
}

fn handle_social_pacts(
    mut pacts: EventReader<SocialPactEvent>,
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

fn apply_social_relationships(
    persistence: Option<Res<PersistenceSettings>>,
    mut events: EventReader<SocialRelationshipEvent>,
    mut players: Query<(&Lifecycle, &mut Relationships), With<Client>>,
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

        for (lifecycle, mut relationships) in &mut players {
            if lifecycle.character_id == event.left {
                relationships.upsert(left_relationship.clone());
            } else if lifecycle.character_id == event.right {
                relationships.upsert(right_relationship.clone());
            }
        }
    }
}

fn apply_social_renown_deltas(
    persistence: Option<Res<PersistenceSettings>>,
    mut events: EventReader<SocialRenownDeltaEvent>,
    mut players: Query<(&Lifecycle, &mut Renown), With<Client>>,
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

        if let Some((_, mut renown)) = players
            .iter_mut()
            .find(|(lifecycle, _)| lifecycle.character_id == event.char_id)
        {
            if let Some(persisted_renown) = persisted_renown {
                *renown = persisted_renown;
            } else {
                renown.apply_delta(
                    event.fame_delta,
                    event.notoriety_delta,
                    event.tags_added.clone(),
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
    )>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut registry: ResMut<SpiritNicheRegistry>,
    mut layers: Query<&mut ChunkLayer, With<crate::world::dimension::OverworldLayer>>,
) {
    let zone_registry = zone_registry
        .as_deref()
        .cloned()
        .unwrap_or_else(ZoneRegistry::fallback);
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
                "[bong][social] spirit niche place rejected for `{}`: item `{}` is not a niche stone",
                lifecycle.character_id,
                instance.template_id
            );
            continue;
        }
        if let Err(error) = consume_item_instance_once(&mut inventory, item_instance_id) {
            tracing::warn!(
                "[bong][social] spirit niche place rejected for `{}`: consume failed: {error}",
                lifecycle.character_id
            );
            continue;
        }

        let zone = zone_registry.find_zone(
            crate::world::dimension::DimensionKind::Overworld,
            position.get(),
        );
        if let Some(cultivation) = cultivation.as_deref_mut() {
            apply_spirit_niche_negative_qi_cost(zone.map(|zone| zone.spirit_qi), cultivation);
        }
        if let Some(mut lifespan) = lifespan {
            apply_spirit_niche_negative_lifespan_cost(
                zone.map(|zone| zone.spirit_qi),
                &mut lifespan,
            );
        }

        let niche = SpiritNiche {
            owner: lifecycle.character_id.clone(),
            pos: event.pos,
            placed_at_tick: event.tick,
            revealed: false,
            revealed_by: None,
            defense_mode: None,
        };
        lifecycle.spawn_anchor = Some(spirit_niche_spawn_anchor(event.pos));
        let old_niche = registry.niches.get(&lifecycle.character_id).cloned();
        registry.upsert(niche.clone());
        commands.entity(event.player).insert(niche.clone());
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
                "spirit_niche_stone_consumed",
            );
        }
    }
}

fn detect_spirit_niche_break_attempts(
    mut digs: EventReader<DiggingEvent>,
    clients: Query<&Lifecycle, With<Client>>,
    registry: Option<Res<SpiritNicheRegistry>>,
    mut reveals: EventWriter<SpiritNicheRevealRequest>,
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
        let observer = clients
            .get(event.client)
            .ok()
            .is_some_and(|lifecycle| lifecycle.character_id != niche.owner)
            .then_some(event.client);
        if observer.is_none() {
            continue;
        }
        reveals.send(SpiritNicheRevealRequest {
            observer,
            owner: niche.owner.clone(),
            source: SpiritNicheRevealSource::BreakAttempt,
            tick,
        });
    }
}

fn handle_spirit_niche_coordinate_reveals(
    mut events: EventReader<SpiritNicheCoordinateRevealRequest>,
    observers: Query<&Lifecycle, With<Client>>,
    registry: Option<Res<SpiritNicheRegistry>>,
    mut reveals: EventWriter<SpiritNicheRevealRequest>,
) {
    let Some(registry) = registry.as_deref() else {
        return;
    };
    for event in events.read() {
        let Ok(observer) = observers.get(event.observer) else {
            continue;
        };
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
            if let Some(mut client) = client {
                client.send_chat_message("灵龛再无庇佑");
            }
        }
    }
}

fn niche_place_target_is_close(position: &Position, target: [i32; 3]) -> bool {
    distance_squared_to_niche(position.get(), target) <= SPIRIT_NICHE_RADIUS * SPIRIT_NICHE_RADIUS
}

fn apply_spirit_niche_negative_qi_cost(zone_qi: Option<f64>, cultivation: &mut Cultivation) {
    if !zone_qi.is_some_and(|qi| qi < 0.0) {
        return;
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
    cultivation.qi_current = (cultivation.qi_current - damage).max(0.0);
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
    relationships: Relationships,
    exposure_log: ExposureLog,
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
        relationships: load_social_relationships(&connection, char_id)?,
        exposure_log: load_social_exposure_log(&connection, char_id)?,
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

fn load_social_renown_from_persistence(
    persistence: &PersistenceSettings,
    char_id: &str,
) -> io::Result<Renown> {
    let connection = open_social_connection(persistence)?;
    load_social_renown(&connection, char_id)
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

fn load_social_spirit_niche(
    connection: &Connection,
    char_id: &str,
) -> io::Result<Option<SpiritNiche>> {
    let row: Option<SpiritNicheSqlRow> = connection
        .query_row(
            "
            SELECT pos_x, pos_y, pos_z, placed_at_tick, revealed, revealed_by, defense_mode
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
                    row.get(6)?,
                ))
            },
        )
        .optional()
        .map_err(io::Error::other)?;
    Ok(row.map(
        |(pos, placed_at_tick, revealed, revealed_by, defense_mode)| SpiritNiche {
            owner: char_id.to_string(),
            pos,
            placed_at_tick,
            revealed,
            revealed_by,
            defense_mode,
        },
    ))
}

fn load_all_social_spirit_niches(
    persistence: &PersistenceSettings,
) -> io::Result<Vec<SpiritNiche>> {
    let connection = open_social_connection(persistence)?;
    let mut statement = connection
        .prepare(
            "
            SELECT owner, pos_x, pos_y, pos_z, placed_at_tick, revealed, revealed_by, defense_mode
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
                defense_mode: row.get(7)?,
            })
        })
        .map_err(io::Error::other)?;
    let mut niches = Vec::new();
    for row in rows {
        niches.push(row.map_err(io::Error::other)?);
    }
    Ok(niches)
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
                defense_mode, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9)
            ON CONFLICT(owner) DO UPDATE SET
                pos_x = excluded.pos_x,
                pos_y = excluded.pos_y,
                pos_z = excluded.pos_z,
                placed_at_tick = excluded.placed_at_tick,
                revealed = excluded.revealed,
                revealed_by = excluded.revealed_by,
                defense_mode = excluded.defense_mode,
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
                niche.defense_mode.as_deref(),
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
mod tests {
    use super::*;
    use crate::combat::components::Lifecycle;
    use crate::combat::CombatClock;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
    };
    use crate::persistence::bootstrap_sqlite;
    use crate::schema::social::RenownTagV1;
    use crate::social::events::PlayerChatCollected;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use valence::prelude::{App, Events, Position, Update};
    use valence::testing::create_mock_client;

    fn spirit_niche_test_item(instance_id: u64) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: SPIRIT_NICHE_ITEM_TEMPLATE_ID.to_string(),
            display_name: "龛石".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.4,
            rarity: ItemRarity::Rare,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
        }
    }

    fn inventory_with_item(item: ItemInstance) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: "main_pack".to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item,
                }],
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    fn unique_temp_dir(test_name: &str) -> PathBuf {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "bong-social-{test_name}-{}-{unique_suffix}",
            std::process::id()
        ))
    }

    fn social_persistence(test_name: &str) -> (PersistenceSettings, PathBuf) {
        let data_dir = unique_temp_dir(test_name);
        let db_path = data_dir.join("bong.db");
        bootstrap_sqlite(&db_path, &format!("social-{test_name}"))
            .expect("sqlite bootstrap should succeed");
        (
            PersistenceSettings::with_paths(
                db_path,
                data_dir.join("deceased"),
                format!("social-{test_name}"),
            ),
            data_dir,
        )
    }

    #[test]
    fn joined_client_gets_default_social_bundle() {
        let mut app = App::new();
        app.add_systems(Update, attach_social_bundle_to_joined_clients);
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();

        app.update();

        let entity_ref = app.world().entity(entity);
        assert!(entity_ref.contains::<Anonymity>());
        assert!(entity_ref.contains::<Renown>());
        assert!(entity_ref.contains::<Relationships>());
        assert!(entity_ref.contains::<ExposureLog>());
    }

    #[test]
    fn chat_exposure_records_nearby_witness_only_after_collected_chat() {
        let mut app = App::new();
        app.add_event::<PlayerChatCollected>();
        app.add_event::<SocialExposureEvent>();
        app.add_systems(Update, expose_chat_speakers);
        let (mut alice_bundle, _alice_helper) = create_mock_client("Alice");
        alice_bundle.player.position = Position::new([0.0, 64.0, 0.0]);
        let alice = app.world_mut().spawn(alice_bundle).id();
        let (mut bob_bundle, _bob_helper) = create_mock_client("Bob");
        bob_bundle.player.position = Position::new([30.0, 64.0, 0.0]);
        let bob = app.world_mut().spawn(bob_bundle).id();
        let (mut far_bundle, _far_helper) = create_mock_client("Far");
        far_bundle.player.position = Position::new([80.0, 64.0, 0.0]);
        let far = app.world_mut().spawn(far_bundle).id();
        app.world_mut().entity_mut(alice).insert(Lifecycle {
            character_id: "char:alice".to_string(),
            ..Default::default()
        });
        app.world_mut().entity_mut(bob).insert(Lifecycle {
            character_id: "char:bob".to_string(),
            ..Default::default()
        });
        app.world_mut().entity_mut(far).insert(Lifecycle {
            character_id: "char:far".to_string(),
            ..Default::default()
        });
        app.world_mut().send_event(PlayerChatCollected {
            entity: alice,
            username: "Alice".to_string(),
            char_id: "char:alice".to_string(),
            zone: "spawn".to_string(),
            raw: "我在此处".to_string(),
            timestamp: 99,
        });

        app.update();

        let events = app.world().resource::<Events<SocialExposureEvent>>();
        let mut reader = events.get_reader();
        let collected = reader.read(events).cloned().collect::<Vec<_>>();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].actor, "char:alice");
        assert_eq!(collected[0].witnesses, vec!["char:bob"]);
    }

    #[test]
    fn social_events_persist_and_reload_by_character_id() {
        let (persistence, data_dir) = social_persistence("event-roundtrip");
        let mut app = App::new();
        app.insert_resource(persistence.clone());
        app.add_event::<SocialExposureEvent>();
        app.add_event::<SocialRelationshipEvent>();
        app.add_event::<SocialRenownDeltaEvent>();
        app.add_systems(
            Update,
            (
                apply_social_exposures,
                apply_social_relationships,
                apply_social_renown_deltas,
            ),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let azure = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(azure).insert((
            Lifecycle {
                character_id: "char:azure".to_string(),
                ..Default::default()
            },
            Anonymity::default(),
            Renown::default(),
            Relationships::default(),
            ExposureLog::default(),
        ));

        app.world_mut().send_event(SocialExposureEvent {
            actor: "char:azure".to_string(),
            kind: ExposureKindV1::Chat,
            witnesses: vec!["char:bob".to_string()],
            tick: 42,
            zone: Some("spawn".to_string()),
        });
        app.world_mut().send_event(SocialRelationshipEvent {
            left: "char:azure".to_string(),
            right: "char:rival".to_string(),
            left_kind: RelationshipKindV1::Feud,
            right_kind: RelationshipKindV1::Feud,
            tick: 43,
            metadata: serde_json::json!({ "place": "spawn" }),
        });
        app.world_mut().send_event(SocialRenownDeltaEvent {
            char_id: "char:azure".to_string(),
            fame_delta: 3,
            notoriety_delta: 5,
            tags_added: vec![RenownTagV1 {
                tag: "戮道者".to_string(),
                weight: 9.0,
                last_seen_tick: 44,
                permanent: false,
            }],
            tick: 44,
            reason: "test".to_string(),
        });

        app.update();

        let loaded = load_social_components(&persistence, "char:azure")
            .expect("persisted social components should reload");

        assert!(loaded.anonymity.is_exposed_to("char:bob"));
        assert_eq!(loaded.exposure_log.0.len(), 1);
        assert_eq!(loaded.exposure_log.0[0].kind, ExposureKindV1::Chat);
        assert_eq!(loaded.relationships.edges.len(), 1);
        assert_eq!(loaded.relationships.edges[0].kind, RelationshipKindV1::Feud);
        assert_eq!(loaded.relationships.edges[0].peer, "char:rival");
        assert_eq!(loaded.relationships.edges[0].metadata["place"], "spawn");
        let loaded_peer = load_social_components(&persistence, "char:rival")
            .expect("reverse relationship should persist for offline peer");
        assert_eq!(loaded_peer.relationships.edges.len(), 1);
        assert_eq!(
            loaded_peer.relationships.edges[0].kind,
            RelationshipKindV1::Feud
        );
        assert_eq!(loaded_peer.relationships.edges[0].peer, "char:azure");
        assert_eq!(loaded.renown.fame, 3);
        assert_eq!(loaded.renown.notoriety, 5);
        assert_eq!(loaded.renown.tags[0].tag, "戮道者");

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn pact_events_create_relationship_exposure_and_betrayer_renown() {
        let (persistence, data_dir) = social_persistence("pact-event");
        let mut app = App::new();
        app.insert_resource(persistence.clone());
        app.add_event::<SocialPactEvent>();
        app.add_event::<SocialExposureEvent>();
        app.add_event::<SocialRelationshipEvent>();
        app.add_event::<SocialRenownDeltaEvent>();
        app.add_systems(
            Update,
            (
                handle_social_pacts,
                apply_social_exposures.after(handle_social_pacts),
                apply_social_relationships.after(handle_social_pacts),
                apply_social_renown_deltas.after(handle_social_pacts),
            ),
        );

        app.world_mut().send_event(SocialPactEvent {
            left: "char:alice".to_string(),
            right: "char:bob".to_string(),
            terms: "同渡此劫".to_string(),
            tick: 81,
            broken: false,
            breaker: None,
            witnesses: vec!["char:witness".to_string()],
        });

        app.update();

        let alice = load_social_components(&persistence, "char:alice")
            .expect("alice pact state should persist");
        assert!(alice.anonymity.is_exposed_to("char:bob"));
        assert!(alice.anonymity.is_exposed_to("char:witness"));
        assert_eq!(alice.relationships.edges.len(), 1);
        assert_eq!(alice.relationships.edges[0].kind, RelationshipKindV1::Pact);
        assert_eq!(alice.relationships.edges[0].peer, "char:bob");
        assert_eq!(alice.relationships.edges[0].metadata["terms"], "同渡此劫");
        assert_eq!(alice.relationships.edges[0].metadata["broken"], false);
        let bob = load_social_components(&persistence, "char:bob")
            .expect("bob pact state should persist");
        assert!(bob.anonymity.is_exposed_to("char:alice"));
        assert!(bob.anonymity.is_exposed_to("char:witness"));
        assert_eq!(bob.relationships.edges[0].peer, "char:alice");

        app.world_mut().send_event(SocialPactEvent {
            left: "char:alice".to_string(),
            right: "char:bob".to_string(),
            terms: "同渡此劫".to_string(),
            tick: 99,
            broken: true,
            breaker: Some("char:bob".to_string()),
            witnesses: Vec::new(),
        });

        app.update();

        let alice = load_social_components(&persistence, "char:alice")
            .expect("alice broken pact state should persist");
        assert_eq!(alice.relationships.edges.len(), 1);
        assert_eq!(alice.relationships.edges[0].since_tick, 99);
        assert_eq!(alice.relationships.edges[0].metadata["broken"], true);
        assert_eq!(alice.relationships.edges[0].metadata["breaker"], "char:bob");
        let bob = load_social_components(&persistence, "char:bob")
            .expect("bob broken pact state should persist");
        assert_eq!(bob.renown.notoriety, 50);
        assert_eq!(bob.renown.tags.len(), 1);
        assert_eq!(bob.renown.tags[0].tag, "背盟者");
        assert!(bob.renown.tags[0].permanent);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn renown_delta_persists_for_offline_character() {
        let (persistence, data_dir) = social_persistence("offline-renown-delta");
        let mut app = App::new();
        app.insert_resource(persistence.clone());
        app.add_event::<SocialRenownDeltaEvent>();
        app.add_systems(Update, apply_social_renown_deltas);

        app.world_mut().send_event(SocialRenownDeltaEvent {
            char_id: "char:offline".to_string(),
            fame_delta: 2,
            notoriety_delta: 7,
            tags_added: vec![RenownTagV1 {
                tag: "背盟者".to_string(),
                weight: 10.0,
                last_seen_tick: 55,
                permanent: true,
            }],
            tick: 55,
            reason: "test_offline".to_string(),
        });

        app.update();

        let loaded = load_social_components(&persistence, "char:offline")
            .expect("offline renown should persist");
        assert_eq!(loaded.renown.fame, 2);
        assert_eq!(loaded.renown.notoriety, 7);
        assert_eq!(loaded.renown.tags.len(), 1);
        assert_eq!(loaded.renown.tags[0].tag, "背盟者");
        assert!(loaded.renown.tags[0].permanent);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn companion_relationship_emits_after_five_hours_nearby() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: COMPANION_SCAN_INTERVAL_TICKS,
        });
        let mut progress = CompanionProgress::default();
        progress.pair_seconds.insert(
            companion_pair_key("char:alice", "char:bob"),
            COMPANION_REQUIRED_SECONDS - 1,
        );
        app.insert_resource(progress);
        app.add_event::<SocialRelationshipEvent>();
        app.add_systems(Update, update_companion_relationships);

        let (mut alice_bundle, _alice_helper) = create_mock_client("Alice");
        alice_bundle.player.position = Position::new([0.0, 64.0, 0.0]);
        let alice = app.world_mut().spawn(alice_bundle).id();
        let (mut bob_bundle, _bob_helper) = create_mock_client("Bob");
        bob_bundle.player.position = Position::new([30.0, 64.0, 0.0]);
        let bob = app.world_mut().spawn(bob_bundle).id();
        app.world_mut().entity_mut(alice).insert((
            Lifecycle {
                character_id: "char:alice".to_string(),
                ..Default::default()
            },
            Relationships::default(),
        ));
        app.world_mut().entity_mut(bob).insert((
            Lifecycle {
                character_id: "char:bob".to_string(),
                ..Default::default()
            },
            Relationships::default(),
        ));

        app.update();

        let events = app.world().resource::<Events<SocialRelationshipEvent>>();
        let mut reader = events.get_reader();
        let collected = reader.read(events).cloned().collect::<Vec<_>>();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].left, "char:alice");
        assert_eq!(collected[0].right, "char:bob");
        assert_eq!(collected[0].left_kind, RelationshipKindV1::Companion);
        assert_eq!(collected[0].right_kind, RelationshipKindV1::Companion);
        assert_eq!(collected[0].metadata["source"], "co_presence");
        assert_eq!(collected[0].metadata["last_interaction_tick"], 20);
    }

    #[test]
    fn stale_companion_relationships_expire_and_delete_persisted_edges() {
        let (persistence, data_dir) = social_persistence("companion-expire");
        persist_social_relationship(
            &persistence,
            "char:alice",
            &Relationship {
                kind: RelationshipKindV1::Companion,
                peer: "char:bob".to_string(),
                since_tick: 10,
                metadata: serde_json::json!({ "last_interaction_tick": 10 }),
            },
        )
        .expect("left companion edge should persist");
        persist_social_relationship(
            &persistence,
            "char:bob",
            &Relationship {
                kind: RelationshipKindV1::Companion,
                peer: "char:alice".to_string(),
                since_tick: 10,
                metadata: serde_json::json!({ "last_interaction_tick": 10 }),
            },
        )
        .expect("right companion edge should persist");

        let mut app = App::new();
        app.insert_resource(persistence.clone());
        app.insert_resource(CombatClock {
            tick: 10 + COMPANION_EXPIRE_TICKS,
        });
        app.add_systems(Update, expire_companion_relationships);
        let (alice_bundle, _alice_helper) = create_mock_client("Alice");
        let alice = app.world_mut().spawn(alice_bundle).id();
        app.world_mut().entity_mut(alice).insert((
            Lifecycle {
                character_id: "char:alice".to_string(),
                ..Default::default()
            },
            Relationships {
                edges: vec![Relationship {
                    kind: RelationshipKindV1::Companion,
                    peer: "char:bob".to_string(),
                    since_tick: 10,
                    metadata: serde_json::json!({ "last_interaction_tick": 10 }),
                }],
            },
        ));

        app.update();

        let relationships = app.world().get::<Relationships>(alice).unwrap();
        assert!(relationships.edges.is_empty());
        let alice = load_social_components(&persistence, "char:alice")
            .expect("alice relationship state should reload");
        assert!(alice.relationships.edges.is_empty());
        let bob = load_social_components(&persistence, "char:bob")
            .expect("bob relationship state should reload");
        assert!(bob.relationships.edges.is_empty());

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn pk_notoriety_delta_requires_higher_fame_victim() {
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.add_event::<SocialExposureEvent>();
        app.add_event::<SocialRelationshipEvent>();
        app.add_event::<SocialRenownDeltaEvent>();
        app.add_systems(Update, handle_death_social_effects);

        let (mut killer_bundle, _killer_helper) = create_mock_client("Killer");
        killer_bundle.player.position = Position::new([0.0, 64.0, 0.0]);
        let killer = app.world_mut().spawn(killer_bundle).id();
        let (mut low_fame_bundle, _low_helper) = create_mock_client("LowFame");
        low_fame_bundle.player.position = Position::new([1.0, 64.0, 0.0]);
        let low_fame_victim = app.world_mut().spawn(low_fame_bundle).id();
        let (mut high_fame_bundle, _high_helper) = create_mock_client("HighFame");
        high_fame_bundle.player.position = Position::new([2.0, 64.0, 0.0]);
        let high_fame_victim = app.world_mut().spawn(high_fame_bundle).id();

        app.world_mut().entity_mut(killer).insert((
            Lifecycle {
                character_id: "char:killer".to_string(),
                ..Default::default()
            },
            Renown {
                fame: 5,
                ..Default::default()
            },
        ));
        app.world_mut().entity_mut(low_fame_victim).insert((
            Lifecycle {
                character_id: "char:low".to_string(),
                ..Default::default()
            },
            Renown {
                fame: 4,
                ..Default::default()
            },
        ));
        app.world_mut().entity_mut(high_fame_victim).insert((
            Lifecycle {
                character_id: "char:high".to_string(),
                ..Default::default()
            },
            Renown {
                fame: 9,
                ..Default::default()
            },
        ));
        app.world_mut().send_event(DeathEvent {
            target: low_fame_victim,
            cause: "pvp".to_string(),
            attacker: Some(killer),
            attacker_player_id: Some("char:killer".to_string()),
            at_tick: 10,
        });
        app.world_mut().send_event(DeathEvent {
            target: high_fame_victim,
            cause: "pvp".to_string(),
            attacker: Some(killer),
            attacker_player_id: Some("char:killer".to_string()),
            at_tick: 11,
        });

        app.update();

        let events = app.world().resource::<Events<SocialRenownDeltaEvent>>();
        let mut reader = events.get_reader();
        let collected = reader.read(events).cloned().collect::<Vec<_>>();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].char_id, "char:killer");
        assert_eq!(collected[0].notoriety_delta, 10);
        assert_eq!(collected[0].tick, 11);
        assert_eq!(collected[0].reason, "pk_death_higher_fame_victim");
    }

    #[test]
    fn spirit_niche_place_consumes_stone_sets_anchor_and_persists() {
        let (persistence, data_dir) = social_persistence("spirit-niche-place");
        let mut app = App::new();
        app.insert_resource(persistence.clone());
        app.insert_resource(SpiritNicheRegistry::default());
        app.add_event::<SpiritNichePlaceRequest>();
        app.add_systems(Update, handle_spirit_niche_place_requests);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([10.0, 64.0, 10.0]);
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert((
            Lifecycle {
                character_id: "char:azure".to_string(),
                ..Default::default()
            },
            inventory_with_item(spirit_niche_test_item(4242)),
            Cultivation {
                qi_current: 10.0,
                ..Default::default()
            },
        ));
        app.world_mut().send_event(SpiritNichePlaceRequest {
            player: entity,
            pos: [11, 64, 10],
            item_instance_id: Some(4242),
            tick: 77,
        });

        app.update();

        let lifecycle = app.world().get::<Lifecycle>(entity).unwrap();
        assert_eq!(lifecycle.spawn_anchor, Some([11.5, 65.0, 10.5]));
        assert!(app.world().get::<SpiritNiche>(entity).is_some());
        let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
        assert!(inventory_item_by_instance(inventory, 4242).is_none());
        let loaded = load_social_components(&persistence, "char:azure")
            .expect("spirit niche should persist")
            .spirit_niche
            .expect("persisted niche should load");
        assert_eq!(loaded.pos, [11, 64, 10]);
        assert!(!loaded.revealed);
        let registry = app.world().resource::<SpiritNicheRegistry>();
        assert!(block_break_is_protected_by_registered_spirit_niche(
            Some("char:other"),
            [11, 64, 10],
            registry
        ));

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn spirit_niche_break_attempt_reveals_and_disables_anchor() {
        let (persistence, data_dir) = social_persistence("spirit-niche-reveal");
        let mut app = App::new();
        app.insert_resource(persistence.clone());
        let mut registry = SpiritNicheRegistry::default();
        registry.upsert(SpiritNiche {
            owner: "char:owner".to_string(),
            pos: [20, 64, 20],
            placed_at_tick: 1,
            revealed: false,
            revealed_by: None,
            defense_mode: None,
        });
        app.insert_resource(registry);
        app.add_event::<DiggingEvent>();
        app.add_event::<SpiritNicheRevealRequest>();
        app.add_systems(
            Update,
            (
                detect_spirit_niche_break_attempts,
                apply_spirit_niche_reveals.after(detect_spirit_niche_break_attempts),
            ),
        );

        let (owner_bundle, _owner_helper) = create_mock_client("Owner");
        let owner = app.world_mut().spawn(owner_bundle).id();
        app.world_mut().entity_mut(owner).insert((
            Lifecycle {
                character_id: "char:owner".to_string(),
                spawn_anchor: Some([20.5, 65.0, 20.5]),
                ..Default::default()
            },
            SpiritNiche {
                owner: "char:owner".to_string(),
                pos: [20, 64, 20],
                placed_at_tick: 1,
                revealed: false,
                revealed_by: None,
                defense_mode: None,
            },
        ));
        let (observer_bundle, _observer_helper) = create_mock_client("Observer");
        let observer = app.world_mut().spawn(observer_bundle).id();
        app.world_mut().entity_mut(observer).insert(Lifecycle {
            character_id: "char:observer".to_string(),
            ..Default::default()
        });
        app.world_mut().send_event(DiggingEvent {
            client: observer,
            position: BlockPos::new(20, 64, 20),
            direction: valence::protocol::Direction::Up,
            state: DiggingState::Start,
        });

        app.update();

        let lifecycle = app.world().get::<Lifecycle>(owner).unwrap();
        assert_eq!(lifecycle.spawn_anchor, None);
        let niche = app.world().get::<SpiritNiche>(owner).unwrap();
        assert!(niche.revealed);
        assert_eq!(niche.revealed_by.as_deref(), Some("char:observer"));
        assert!(!block_break_is_protected_by_registered_spirit_niche(
            Some("char:other"),
            [20, 64, 20],
            app.world().resource::<SpiritNicheRegistry>()
        ));
        let loaded = load_social_components(&persistence, "char:owner")
            .expect("revealed spirit niche should persist")
            .spirit_niche
            .expect("persisted niche should load");
        assert!(loaded.revealed);
        assert_eq!(loaded.revealed_by.as_deref(), Some("char:observer"));

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn spirit_niche_coordinate_reveal_emits_owner_reveal_only_for_exact_active_hit() {
        let mut app = App::new();
        let mut registry = SpiritNicheRegistry::default();
        registry.upsert(SpiritNiche {
            owner: "char:owner".to_string(),
            pos: [20, 64, 20],
            placed_at_tick: 1,
            revealed: false,
            revealed_by: None,
            defense_mode: None,
        });
        app.insert_resource(registry);
        app.add_event::<SpiritNicheCoordinateRevealRequest>();
        app.add_event::<SpiritNicheRevealRequest>();
        app.add_systems(Update, handle_spirit_niche_coordinate_reveals);

        let (observer_bundle, _observer_helper) = create_mock_client("Observer");
        let observer = app.world_mut().spawn(observer_bundle).id();
        app.world_mut().entity_mut(observer).insert(Lifecycle {
            character_id: "char:observer".to_string(),
            ..Default::default()
        });
        app.world_mut()
            .send_event(SpiritNicheCoordinateRevealRequest {
                observer,
                pos: [20, 64, 20],
                source: SpiritNicheRevealSource::Gaze,
                tick: 99,
            });
        app.world_mut()
            .send_event(SpiritNicheCoordinateRevealRequest {
                observer,
                pos: [21, 64, 20],
                source: SpiritNicheRevealSource::MarkCoordinate,
                tick: 100,
            });

        app.update();

        let mut events = app
            .world_mut()
            .resource_mut::<Events<SpiritNicheRevealRequest>>();
        let collected = events.drain().collect::<Vec<_>>();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].observer, Some(observer));
        assert_eq!(collected[0].owner, "char:owner");
        assert_eq!(collected[0].source, SpiritNicheRevealSource::Gaze);
        assert_eq!(collected[0].tick, 99);
    }
}
