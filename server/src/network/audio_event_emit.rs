//! `bong:audio/play` / `bong:audio/stop` S2C CustomPayload emitters.

use valence::message::ChatMessageEvent;
use valence::message::SendMessage;
use valence::prelude::{
    bevy_ecs, ident, Client, DVec3, Entity, Event, EventReader, EventWriter, Position, Query, Res,
    ResMut, Resource, With,
};

use crate::audio::SoundRecipeRegistry;
use crate::schema::audio::{
    validate_recipe_id, AudioAttenuation, PlaySoundRecipeEventV1, PlaySoundRecipePayload,
    StopSoundRecipeEventV1, StopSoundRecipePayload,
};

pub const AUDIO_PLAY_CHANNEL: &str = "bong:audio/play";
pub const AUDIO_STOP_CHANNEL: &str = "bong:audio/stop";
pub const AUDIO_BROADCAST_RADIUS: f64 = 64.0;

#[derive(Debug, Default)]
pub struct AudioInstanceIdAllocator {
    next: u64,
}

impl Resource for AudioInstanceIdAllocator {}

impl AudioInstanceIdAllocator {
    pub fn allocate(&mut self) -> u64 {
        self.next = self.next.saturating_add(1).max(1);
        self.next
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioRecipient {
    Single(Entity),
    Radius { origin: DVec3, radius: f64 },
    All,
}

impl AudioRecipient {
    fn accepts(&self, entity: Entity, position: DVec3) -> bool {
        match self {
            Self::Single(target) => *target == entity,
            Self::Radius { origin, radius } => origin.distance_squared(position) <= radius * radius,
            Self::All => true,
        }
    }
}

#[derive(Debug, Clone, Event)]
pub struct PlaySoundRecipeRequest {
    pub recipe_id: String,
    /// 0 means allocate a fresh server-side id at emit time.
    pub instance_id: u64,
    pub pos: Option<[i32; 3]>,
    pub flag: Option<String>,
    pub volume_mul: f32,
    pub pitch_shift: f32,
    pub recipient: AudioRecipient,
}

#[derive(Debug, Clone, Event)]
pub struct StopSoundRecipeRequest {
    pub instance_id: u64,
    pub fade_out_ticks: u32,
    pub recipient: AudioRecipient,
}

pub fn emit_audio_play_payloads(
    mut reader: EventReader<PlaySoundRecipeRequest>,
    registry: Option<Res<SoundRecipeRegistry>>,
    mut allocator: ResMut<AudioInstanceIdAllocator>,
    mut clients: Query<(Entity, &mut Client, &Position), With<Client>>,
) {
    let Some(registry) = registry else {
        for request in reader.read() {
            tracing::warn!(
                "[bong][audio] dropping play request recipe={}: SoundRecipeRegistry missing",
                request.recipe_id
            );
        }
        return;
    };

    for request in reader.read() {
        let Some(recipe) = registry.get(&request.recipe_id) else {
            tracing::warn!(
                "[bong][audio] dropping unknown sound recipe `{}`",
                request.recipe_id
            );
            continue;
        };
        let instance_id = if request.instance_id == 0 {
            allocator.allocate()
        } else {
            request.instance_id
        };
        let event = PlaySoundRecipeEventV1::new(PlaySoundRecipePayload {
            recipe_id: request.recipe_id.clone(),
            instance_id,
            pos: request.pos,
            flag: request.flag.clone(),
            volume_mul: request.volume_mul,
            pitch_shift: request.pitch_shift,
            recipe: recipe.clone(),
        });
        let bytes = match event.to_json_bytes_checked() {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(
                    "[bong][audio] dropping play recipe={} instance={instance_id}: {error:?}",
                    request.recipe_id
                );
                continue;
            }
        };
        let mut recipients = 0usize;
        for (entity, mut client, position) in &mut clients {
            if !request.recipient.accepts(entity, position.get()) {
                continue;
            }
            let _ = AUDIO_PLAY_CHANNEL;
            client.send_custom_payload(ident!("bong:audio/play"), &bytes);
            recipients += 1;
        }
        tracing::debug!(
            "[bong][audio] dispatched play recipe={} instance={instance_id} to {recipients} client(s)",
            request.recipe_id
        );
    }
}

pub fn emit_audio_stop_payloads(
    mut reader: EventReader<StopSoundRecipeRequest>,
    mut clients: Query<(Entity, &mut Client, &Position), With<Client>>,
) {
    for request in reader.read() {
        let event = StopSoundRecipeEventV1::new(StopSoundRecipePayload {
            instance_id: request.instance_id,
            fade_out_ticks: request.fade_out_ticks,
        });
        let bytes = match event.to_json_bytes_checked() {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(
                    "[bong][audio] dropping stop instance={}: {error:?}",
                    request.instance_id
                );
                continue;
            }
        };
        for (entity, mut client, position) in &mut clients {
            if request.recipient.accepts(entity, position.get()) {
                let _ = AUDIO_STOP_CHANNEL;
                client.send_custom_payload(ident!("bong:audio/stop"), &bytes);
            }
        }
    }
}

pub fn handle_audio_debug_commands(
    mut events: EventReader<ChatMessageEvent>,
    mut registry: ResMut<SoundRecipeRegistry>,
    players: Query<(Entity, &Position), With<Client>>,
    mut clients: Query<&mut Client, With<Client>>,
    mut play_events: EventWriter<PlaySoundRecipeRequest>,
) {
    for ChatMessageEvent {
        client, message, ..
    } in events.read()
    {
        let trimmed = message.trim();
        if !trimmed.starts_with("/audio") {
            continue;
        }
        match parse_audio_debug_command(trimmed) {
            AudioDebugCommand::Usage(hint) => {
                if let Ok(mut c) = clients.get_mut(*client) {
                    c.send_chat_message(hint);
                }
            }
            AudioDebugCommand::Reload => match SoundRecipeRegistry::load_default() {
                Ok(next) => {
                    let count = next.len();
                    *registry = next;
                    if let Ok(mut c) = clients.get_mut(*client) {
                        c.send_chat_message(format!("/audio reload ok: {count} recipe(s)"));
                    }
                }
                Err(error) => {
                    if let Ok(mut c) = clients.get_mut(*client) {
                        c.send_chat_message(format!("/audio reload failed: {error}"));
                    }
                }
            },
            AudioDebugCommand::Play { recipe_id } => {
                let Ok((entity, position)) = players.get(*client) else {
                    continue;
                };
                let pos = position.get();
                let recipient = registry
                    .get(&recipe_id)
                    .map(|recipe| recipient_for_attenuation(recipe.attenuation, entity, pos))
                    .unwrap_or(AudioRecipient::Single(entity));
                play_events.send(PlaySoundRecipeRequest {
                    recipe_id: recipe_id.clone(),
                    instance_id: 0,
                    pos: Some([
                        pos.x.floor() as i32,
                        pos.y.floor() as i32,
                        pos.z.floor() as i32,
                    ]),
                    flag: None,
                    volume_mul: 1.0,
                    pitch_shift: 0.0,
                    recipient,
                });
                if let Ok(mut c) = clients.get_mut(*client) {
                    c.send_chat_message(format!("/audio play dispatched: {recipe_id}"));
                }
            }
        }
    }
}

pub fn recipient_for_attenuation(
    attenuation: AudioAttenuation,
    entity: Entity,
    origin: DVec3,
) -> AudioRecipient {
    match attenuation {
        AudioAttenuation::PlayerLocal => AudioRecipient::Single(entity),
        AudioAttenuation::World3d | AudioAttenuation::ZoneBroadcast => AudioRecipient::Radius {
            origin,
            radius: AUDIO_BROADCAST_RADIUS,
        },
        AudioAttenuation::GlobalHint => AudioRecipient::All,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AudioDebugCommand {
    Play { recipe_id: String },
    Reload,
    Usage(&'static str),
}

const AUDIO_USAGE_HINT: &str = "Usage: /audio play <recipe_id> | /audio reload";
const AUDIO_RECIPE_ID_HINT: &str = "recipe_id must match [a-z0-9_]+ (e.g. pill_consume)";

fn parse_audio_debug_command(message: &str) -> AudioDebugCommand {
    let mut tokens = message.split_whitespace();
    let _command = tokens.next();
    match tokens.next() {
        Some("reload") => AudioDebugCommand::Reload,
        Some("play") => {
            let Some(recipe_id) = tokens.next() else {
                return AudioDebugCommand::Usage(AUDIO_USAGE_HINT);
            };
            if validate_recipe_id(recipe_id).is_err() {
                return AudioDebugCommand::Usage(AUDIO_RECIPE_ID_HINT);
            }
            AudioDebugCommand::Play {
                recipe_id: recipe_id.to_string(),
            }
        }
        _ => AudioDebugCommand::Usage(AUDIO_USAGE_HINT),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn setup_audio_emit_app() -> App {
        let mut app = App::new();
        app.insert_resource(
            SoundRecipeRegistry::load_default().expect("default recipes should load"),
        );
        app.init_resource::<AudioInstanceIdAllocator>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_event::<StopSoundRecipeRequest>();
        app.add_systems(Update, (emit_audio_play_payloads, emit_audio_stop_payloads));
        app
    }

    fn spawn_mock_client_at(
        app: &mut App,
        name: &str,
        pos: [f64; 3],
    ) -> (Entity, MockClientHelper) {
        let (mut bundle, helper) = create_mock_client(name);
        bundle.player.position = Position::new(pos);
        let entity = app.world_mut().spawn(bundle).id();
        (entity, helper)
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    fn collect_play_payloads(helper: &mut MockClientHelper) -> Vec<PlaySoundRecipeEventV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != AUDIO_PLAY_CHANNEL {
                continue;
            }
            payloads.push(
                serde_json::from_slice(packet.data.0 .0).expect("audio play payload should decode"),
            );
        }
        payloads
    }

    #[test]
    fn parse_audio_debug_play() {
        assert_eq!(
            parse_audio_debug_command("/audio play pill_consume"),
            AudioDebugCommand::Play {
                recipe_id: "pill_consume".to_string()
            }
        );
    }

    #[test]
    fn parse_audio_debug_rejects_bad_recipe_id() {
        match parse_audio_debug_command("/audio play minecraft:bell") {
            AudioDebugCommand::Usage(hint) => assert!(hint.contains("recipe_id")),
            other => panic!("expected Usage, got {other:?}"),
        }
    }

    #[test]
    fn play_request_sends_inline_recipe_to_single_target() {
        let mut app = setup_audio_emit_app();
        let (near_entity, mut near_helper) =
            spawn_mock_client_at(&mut app, "near", [0.0, 64.0, 0.0]);
        let (_far_entity, mut far_helper) = spawn_mock_client_at(&mut app, "far", [4.0, 64.0, 0.0]);
        app.world_mut().send_event(PlaySoundRecipeRequest {
            recipe_id: "pill_consume".to_string(),
            instance_id: 42,
            pos: Some([0, 64, 0]),
            flag: None,
            volume_mul: 1.0,
            pitch_shift: 0.0,
            recipient: AudioRecipient::Single(near_entity),
        });

        app.update();
        flush_all_client_packets(&mut app);

        let near_payloads = collect_play_payloads(&mut near_helper);
        let far_payloads = collect_play_payloads(&mut far_helper);
        assert_eq!(near_payloads.len(), 1);
        assert!(
            far_payloads.is_empty(),
            "single-target audio should not broadcast"
        );
        assert_eq!(near_payloads[0].payload.recipe_id, "pill_consume");
        assert_eq!(near_payloads[0].payload.recipe.layers.len(), 2);
    }

    #[test]
    fn radius_recipient_filters_by_distance() {
        let mut app = setup_audio_emit_app();
        let (_near_entity, mut near_helper) =
            spawn_mock_client_at(&mut app, "near", [0.0, 64.0, 0.0]);
        let (_far_entity, mut far_helper) =
            spawn_mock_client_at(&mut app, "far", [100.0, 64.0, 0.0]);
        app.world_mut().send_event(PlaySoundRecipeRequest {
            recipe_id: "parry_clang".to_string(),
            instance_id: 7,
            pos: Some([0, 64, 0]),
            flag: None,
            volume_mul: 1.0,
            pitch_shift: 0.0,
            recipient: AudioRecipient::Radius {
                origin: DVec3::new(0.0, 64.0, 0.0),
                radius: AUDIO_BROADCAST_RADIUS,
            },
        });

        app.update();
        flush_all_client_packets(&mut app);

        assert_eq!(collect_play_payloads(&mut near_helper).len(), 1);
        assert!(collect_play_payloads(&mut far_helper).is_empty());
    }
}
