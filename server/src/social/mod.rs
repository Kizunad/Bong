pub mod components;
pub mod events;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use valence::prelude::{
    bevy_ecs, Added, App, Client, Commands, Entity, EventReader, EventWriter, IntoSystemConfigs,
    Position, Query, Res, ResMut, Resource, Update, With, Without,
};

use self::components::{
    Anonymity, ExposureEvent, ExposureLog, Relationship, Relationships, Renown,
};
use self::events::{
    PlayerChatCollected, SocialExposureEvent, SocialRelationshipEvent, SocialRenownDeltaEvent,
};
use crate::combat::components::{Lifecycle, LifecycleState};
use crate::combat::events::DeathEvent;
use crate::combat::CombatClock;
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::{send_server_data_payload, RedisBridgeResource};
use crate::persistence::PersistenceSettings;
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
const NAMELESS_LABEL: &str = "无名修士";

type CompanionPairKey = (String, String);

#[derive(Debug, Default, Resource)]
struct CompanionProgress {
    pair_seconds: HashMap<CompanionPairKey, u64>,
}

pub fn register(app: &mut App) {
    app.init_resource::<CompanionProgress>();
    app.add_event::<PlayerChatCollected>();
    app.add_event::<SocialExposureEvent>();
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_event::<SocialRelationshipEvent>();
    app.add_systems(
        Update,
        (
            attach_social_bundle_to_joined_clients
                .after(crate::player::attach_player_state_to_joined_clients)
                .after(crate::combat::CombatSystemSet::Intent),
            emit_anonymity_payloads_for_joined_clients
                .after(attach_social_bundle_to_joined_clients),
            expose_chat_speakers.after(crate::network::chat_collector::collect_player_chat),
            handle_death_social_effects.after(crate::combat::CombatSystemSet::Resolve),
            update_companion_relationships.after(attach_social_bundle_to_joined_clients),
            apply_social_exposures.after(expose_chat_speakers),
            apply_social_relationships
                .after(handle_death_social_effects)
                .after(update_companion_relationships),
            apply_social_renown_deltas.after(handle_death_social_effects),
            publish_social_events
                .after(apply_social_exposures)
                .after(update_companion_relationships),
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

        commands.entity(entity).insert(social_state.into_bundle());
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
    players: Query<(Entity, &Lifecycle, &Position), With<Client>>,
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
        let Ok((victim_entity, victim_lifecycle, victim_pos)) = players.get(death.target) else {
            continue;
        };
        let victim_char = victim_lifecycle.character_id.clone();
        let mut witnesses = HashSet::new();
        if let Some(attacker_entity) = death.attacker {
            if let Ok((_, attacker_lifecycle, _)) = players.get(attacker_entity) {
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
                    renown_deltas.send(SocialRenownDeltaEvent {
                        char_id: attacker_lifecycle.character_id.clone(),
                        fame_delta: 0,
                        notoriety_delta: 10,
                        tags_added: Vec::new(),
                        tick: death.at_tick,
                        reason: "pk_death".to_string(),
                    });
                }
            }
        }
        witnesses.extend(nearby_player_char_ids(
            victim_entity,
            victim_char.as_str(),
            victim_pos,
            DEATH_EXPOSURE_RADIUS,
            &players,
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
        if let Some((_, mut anonymity, mut log)) = players
            .iter_mut()
            .find(|(lifecycle, _, _)| lifecycle.character_id == exposure.actor)
        {
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

fn apply_social_relationships(
    persistence: Option<Res<PersistenceSettings>>,
    mut events: EventReader<SocialRelationshipEvent>,
    mut players: Query<(&Lifecycle, &mut Relationships), With<Client>>,
) {
    for event in events.read() {
        for (lifecycle, mut relationships) in &mut players {
            if lifecycle.character_id == event.left {
                let relationship = Relationship {
                    kind: event.left_kind,
                    peer: event.right.clone(),
                    since_tick: event.tick,
                    metadata: event.metadata.clone(),
                };
                relationships.upsert(relationship.clone());
                if let Some(persistence) = persistence.as_deref() {
                    if let Err(error) = persist_social_relationship(
                        persistence,
                        lifecycle.character_id.as_str(),
                        &relationship,
                    ) {
                        tracing::warn!(
                            "[bong][social] failed to persist relationship for `{}`: {error}",
                            lifecycle.character_id
                        );
                    }
                }
            } else if lifecycle.character_id == event.right {
                let relationship = Relationship {
                    kind: event.right_kind,
                    peer: event.left.clone(),
                    since_tick: event.tick,
                    metadata: event.metadata.clone(),
                };
                relationships.upsert(relationship.clone());
                if let Some(persistence) = persistence.as_deref() {
                    if let Err(error) = persist_social_relationship(
                        persistence,
                        lifecycle.character_id.as_str(),
                        &relationship,
                    ) {
                        tracing::warn!(
                            "[bong][social] failed to persist relationship for `{}`: {error}",
                            lifecycle.character_id
                        );
                    }
                }
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
        if let Some((_, mut renown)) = players
            .iter_mut()
            .find(|(lifecycle, _)| lifecycle.character_id == event.char_id)
        {
            renown.apply_delta(
                event.fame_delta,
                event.notoriety_delta,
                event.tags_added.clone(),
            );
            if let Some(persistence) = persistence.as_deref() {
                if let Err(error) =
                    persist_social_renown(persistence, event.char_id.as_str(), &renown)
                {
                    tracing::warn!(
                        "[bong][social] failed to persist renown for `{}`: {error}",
                        event.char_id
                    );
                }
            }
        }
    }
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
                    }),
                });
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
struct SocialComponentsSnapshot {
    anonymity: Anonymity,
    renown: Renown,
    relationships: Relationships,
    exposure_log: ExposureLog,
}

impl SocialComponentsSnapshot {
    fn into_bundle(self) -> (Anonymity, Renown, Relationships, ExposureLog) {
        (
            self.anonymity,
            self.renown,
            self.relationships,
            self.exposure_log,
        )
    }
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
    use crate::persistence::bootstrap_sqlite;
    use crate::schema::social::RenownTagV1;
    use crate::social::events::PlayerChatCollected;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use valence::prelude::{App, Update};
    use valence::prelude::{Events, Position};
    use valence::testing::create_mock_client;

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
        assert_eq!(loaded.renown.fame, 3);
        assert_eq!(loaded.renown.notoriety, 5);
        assert_eq!(loaded.renown.tags[0].tag, "戮道者");

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
    }
}
