pub mod components;
pub mod events;

use std::collections::HashSet;

use valence::prelude::{
    Added, App, Client, Commands, Entity, EventReader, EventWriter, IntoSystemConfigs, Position,
    Query, Res, Update, With, Without,
};

use self::components::{
    Anonymity, ExposureEvent, ExposureLog, Relationship, Relationships, Renown,
};
use self::events::{
    PlayerChatCollected, SocialExposureEvent, SocialRelationshipEvent, SocialRenownDeltaEvent,
};
use crate::combat::components::{Lifecycle, LifecycleState};
use crate::combat::events::DeathEvent;
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::{send_server_data_payload, RedisBridgeResource};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::schema::social::{
    ExposureKindV1, RelationshipKindV1, SocialAnonymityPayloadV1, SocialExposureEventV1,
    SocialFeudEventV1, SocialPactEventV1, SocialRemoteIdentityV1, SocialRenownDeltaV1,
};
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

const CHAT_EXPOSURE_RADIUS: f64 = 50.0;
const DEATH_EXPOSURE_RADIUS: f64 = 50.0;
const NAMELESS_LABEL: &str = "无名修士";

pub fn register(app: &mut App) {
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
            apply_social_exposures.after(expose_chat_speakers),
            apply_social_relationships.after(handle_death_social_effects),
            apply_social_renown_deltas.after(handle_death_social_effects),
            publish_social_events.after(apply_social_exposures),
        ),
    );
}

fn attach_social_bundle_to_joined_clients(
    mut commands: Commands,
    joined_clients: Query<valence::prelude::Entity, (Added<Client>, Without<Anonymity>)>,
) {
    for entity in &joined_clients {
        commands.entity(entity).insert((
            Anonymity::default(),
            Renown::default(),
            Relationships::default(),
            ExposureLog::default(),
        ));
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
    mut events: EventReader<SocialRelationshipEvent>,
    mut players: Query<(&Lifecycle, &mut Relationships), With<Client>>,
) {
    for event in events.read() {
        for (lifecycle, mut relationships) in &mut players {
            if lifecycle.character_id == event.left {
                relationships.upsert(Relationship {
                    kind: event.left_kind,
                    peer: event.right.clone(),
                    since_tick: event.tick,
                    metadata: event.metadata.clone(),
                });
            } else if lifecycle.character_id == event.right {
                relationships.upsert(Relationship {
                    kind: event.right_kind,
                    peer: event.left.clone(),
                    since_tick: event.tick,
                    metadata: event.metadata.clone(),
                });
            }
        }
    }
}

fn apply_social_renown_deltas(
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
        }
    }
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
    use crate::social::events::PlayerChatCollected;
    use valence::prelude::{App, Update};
    use valence::prelude::{Events, Position};
    use valence::testing::create_mock_client;

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
}
