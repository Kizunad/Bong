//! 丹炉世界表现：成功事务发动作，炉次每秒发状态。旁观者无需打开工位 UI。
//!
//! `server_data.alchemy_world` v1 仅承载表现数据，不接受客户端回写。
//! 状态含材料 ID/数量，客户端以资源包图标的平均色染烟；动作与状态同包，避免串炉。

use std::collections::BTreeMap;

use valence::prelude::*;

use super::{AlchemyFurnace, AlchemySession};
use crate::network::agent_bridge::serialize_server_data_payload_proto;
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::send_server_data_payload;
use crate::schema::alchemy::AlchemyWorldDataV1;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::world::dimension::{CurrentDimension, DimensionKind};

pub const VIEW_RADIUS: f64 = 48.0;

#[derive(Debug, Clone)]
pub enum AlchemyWorldAction {
    State,
    Ignite,
    FireRaise,
    FireLower,
    InjectQi {
        source: [f64; 3],
    },
    Feed {
        item: String,
        count: u32,
        sound: String,
    },
    Incense,
    Collect {
        result: String,
        item: String,
        name: String,
    },
}

#[derive(Debug, Clone, Event)]
pub struct AlchemyWorldEffect {
    pub furnace_pos: (i32, i32, i32),
    pub heat: f64,
    pub incense: bool,
    pub materials: BTreeMap<String, u32>,
    pub action: AlchemyWorldAction,
}

impl AlchemyWorldEffect {
    pub fn new(
        furnace_pos: (i32, i32, i32),
        session: Option<&AlchemySession>,
        action: AlchemyWorldAction,
    ) -> Self {
        Self {
            furnace_pos,
            heat: session
                .filter(|s| !s.finished)
                .map_or(0.0, |s| s.temp_current.clamp(0.0, 1.0)),
            incense: session.is_some_and(|s| s.incense_active().is_some()),
            materials: session.map_or_else(BTreeMap::new, |s| {
                s.staged
                    .materials
                    .iter()
                    .map(|(id, count)| (id.clone(), *count))
                    .collect()
            }),
            action,
        }
    }

    pub fn emit(
        events: Option<&mut Events<Self>>,
        furnace_pos: (i32, i32, i32),
        session: Option<&AlchemySession>,
        action: AlchemyWorldAction,
    ) {
        if let Some(events) = events {
            events.send(Self::new(furnace_pos, session, action));
        }
    }

    fn origin(&self) -> DVec3 {
        let (x, y, z) = self.furnace_pos;
        DVec3::new(f64::from(x) + 0.5, f64::from(y) + 1.0, f64::from(z) + 0.5)
    }

    fn payload(&self) -> ServerDataV1 {
        let mut data = AlchemyWorldDataV1 {
            furnace_pos: self.furnace_pos,
            heat: self.heat,
            incense: self.incense,
            materials: self.materials.clone(),
            action: String::new(),
            item: None,
            count: None,
            result: None,
            name: None,
            source: None,
        };
        data.action = match &self.action {
            AlchemyWorldAction::State => "state",
            AlchemyWorldAction::Ignite => "ignite",
            AlchemyWorldAction::FireRaise => "fire_raise",
            AlchemyWorldAction::FireLower => "fire_lower",
            AlchemyWorldAction::InjectQi { source } => {
                data.source = Some(*source);
                "inject_qi"
            }
            AlchemyWorldAction::Feed { item, count, .. } => {
                data.item = Some(item.clone());
                data.count = Some(*count);
                "feed"
            }
            AlchemyWorldAction::Incense => "incense",
            AlchemyWorldAction::Collect { result, item, name } => {
                data.result = Some(result.clone());
                data.item = Some(item.clone());
                data.name = Some(name.clone());
                "collect"
            }
        }
        .to_string();
        ServerDataV1::new(ServerDataPayloadV1::AlchemyWorld(Box::new(data)))
    }

    fn sound(&self) -> Option<&str> {
        Some(match &self.action {
            AlchemyWorldAction::State => return None,
            AlchemyWorldAction::Ignite => "alchemy_ignite",
            AlchemyWorldAction::FireRaise => "alchemy_fire_raise",
            AlchemyWorldAction::FireLower => "alchemy_fire_lower",
            AlchemyWorldAction::InjectQi { .. } => "alchemy_qi_inject",
            AlchemyWorldAction::Feed { sound, .. } => sound,
            AlchemyWorldAction::Incense => "alchemy_incense_light",
            AlchemyWorldAction::Collect { result, .. } => match result.as_str() {
                "perfect" => "alchemy_collect_perfect",
                "good" => "alchemy_collect_good",
                "flawed" => "alchemy_collect_flawed",
                "early_take" => "alchemy_collect_early",
                "explode" => "alchemy_explode",
                _ => "alchemy_collect_waste",
            },
        })
    }
}

/// 即使炉主离线或没有打开 UI，也继续同步；进入视野的玩家最多一秒后取得火候。
/// 有限寿命心跳避免退服、卸载区块或炉体移除后留下永久粒子/循环声音。
pub fn emit_world_states(
    mut ticks: Local<u64>,
    furnaces: Query<&AlchemyFurnace>,
    mut events: EventWriter<AlchemyWorldEffect>,
) {
    *ticks += 1;
    if !(*ticks).is_multiple_of(20) {
        return;
    }
    for furnace in &furnaces {
        if let Some(pos) = furnace.pos {
            events.send(AlchemyWorldEffect::new(
                pos,
                furnace.session.as_ref(),
                AlchemyWorldAction::State,
            ));
        }
    }
}

pub fn emit_world_effects(
    mut events: EventReader<AlchemyWorldEffect>,
    mut clients: Query<(Entity, &mut Client, &Position, &CurrentDimension)>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in events.read() {
        let Ok(bytes) = serialize_server_data_payload_proto(&event.payload()) else {
            continue;
        };
        for (entity, mut client, position, dimension) in &mut clients {
            if dimension.0 != DimensionKind::Overworld
                || position.0.distance_squared(event.origin()) > VIEW_RADIUS * VIEW_RADIUS
            {
                continue;
            }
            send_server_data_payload(&mut client, &bytes);
            if let Some(recipe) = event.sound() {
                let (x, y, z) = event.furnace_pos;
                audio.send(PlaySoundRecipeRequest {
                    recipe_id: recipe.to_string(),
                    instance_id: 0,
                    pos: Some([x, y, z]),
                    flag: None,
                    volume_mul: 1.0,
                    pitch_shift: 0.0,
                    // 半径、维度在上方统一裁剪，避免同坐标异维度玩家听到操作声。
                    recipient: AudioRecipient::Single(entity),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::SoundRecipeRegistry;
    use crate::network::audio_event_emit::{emit_audio_play_payloads, AudioInstanceIdAllocator};
    use crate::schema::proto_gen::bong;
    use prost::Message;
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::create_mock_client;

    #[test]
    fn nearby_observer_gets_world_animation_and_positional_audio_without_opening_ui() {
        let mut app = App::new();
        app.add_event::<AlchemyWorldEffect>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.init_resource::<AudioInstanceIdAllocator>();
        app.insert_resource(SoundRecipeRegistry::load_default().unwrap());
        app.add_systems(
            Update,
            (emit_world_effects, emit_audio_play_payloads).chain(),
        );
        let mut clients = Vec::new();
        for (name, x, dimension, receives) in [
            ("owner", 2.5, DimensionKind::Overworld, true),
            ("observer", 5.0, DimensionKind::Overworld, true),
            ("far", 100.0, DimensionKind::Overworld, false),
            ("other_dimension", 2.5, DimensionKind::Tsy, false),
        ] {
            let (bundle, helper) = create_mock_client(name);
            let entity = app
                .world_mut()
                .spawn(bundle)
                .insert((
                    Position::new(DVec3::new(x, 64.0, 3.5)),
                    CurrentDimension(dimension),
                ))
                .id();
            clients.push((entity, helper, receives));
        }
        app.world_mut().send_event(AlchemyWorldEffect::new(
            (2, 64, 3),
            None,
            AlchemyWorldAction::FireRaise,
        ));
        app.update();
        for (entity, mut helper, receives) in clients {
            app.world_mut()
                .get_mut::<Client>(entity)
                .unwrap()
                .flush_packets()
                .unwrap();
            let packets = helper.collect_received();
            let mut channels = Vec::new();
            for frame in packets.0 {
                if let Ok(packet) = frame.decode::<CustomPayloadS2c>() {
                    if packet.channel.as_str() == "bong:server_data" {
                        let envelope = bong::ServerDataEnvelope::decode(packet.data.0 .0).unwrap();
                        let Some(bong::server_data_envelope::Payload::AlchemyWorld(body)) =
                            envelope.payload
                        else {
                            panic!("世界表现必须走统一 server_data 的 alchemy_world 分支");
                        };
                        assert_eq!(body.action, "fire_raise");
                        assert_eq!(body.furnace_pos, vec![2, 64, 3]);
                    }
                    if packet.channel.as_str() == "bong:audio/play" {
                        let body: serde_json::Value =
                            serde_json::from_slice(packet.data.0 .0).unwrap();
                        assert_eq!(body["pos"], serde_json::json!([2, 64, 3]));
                        assert_eq!(body["recipe_id"], "alchemy_fire_raise");
                    }
                    channels.push(packet.channel.to_string());
                }
            }
            assert_eq!(channels.contains(&"bong:server_data".into()), receives);
            assert_eq!(channels.contains(&"bong:audio/play".into()), receives);
        }
    }

    #[test]
    fn rust_world_payload_matches_client_fixture() {
        let mut session = AlchemySession::new("test".into(), "owner".into());
        session.temp_current = 0.6;
        session.staged.materials.insert("ci_she_hao".into(), 2);
        let actual = [
            AlchemyWorldEffect::new(
                (2, 64, 3),
                Some(&session),
                AlchemyWorldAction::InjectQi {
                    source: [1.0, 65.1, 2.0],
                },
            ),
            AlchemyWorldEffect::new(
                (2, 64, 3),
                None,
                AlchemyWorldAction::Collect {
                    result: "early_take".into(),
                    item: "alchemy_residue_processing_dregs".into(),
                    name: "炮制药渣".into(),
                },
            ),
        ];
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../../proto/fixtures/alchemy_world_v1.json"
        ))
        .unwrap();
        let payloads = actual.map(|event| event.payload());
        assert_eq!(serde_json::to_value(&payloads).unwrap(), expected);
        // Java 消费 Rust 生产编码的真实字节；普通测试逐字节核验，避免双端各自自证。
        for (index, payload) in payloads.iter().enumerate() {
            let bytes = serialize_server_data_payload_proto(payload).unwrap();
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../proto/fixtures/alchemy_world_{index}_v1.pb"));
            if std::env::var_os("BONG_UPDATE_ALCHEMY_WORLD_FIXTURES").is_some() {
                std::fs::write(&path, &bytes).unwrap();
            }
            assert_eq!(std::fs::read(path).unwrap(), bytes);
        }
    }

    #[test]
    fn heartbeat_replays_current_heat_without_replaying_actions_and_stops_when_finished() {
        let mut app = App::new();
        app.add_event::<AlchemyWorldEffect>();
        app.add_systems(Update, emit_world_states);
        let mut furnace = AlchemyFurnace::placed(BlockPos::new(2, 64, 3), 1);
        let mut session = AlchemySession::new("test".into(), "offline_owner".into());
        session.temp_current = 0.75;
        furnace.session = Some(session);
        let furnace = app.world_mut().spawn(furnace).id();
        for _ in 0..20 {
            app.update();
        }
        let states: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<AlchemyWorldEffect>>()
            .drain()
            .collect();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].heat, 0.75, "没有炉主 Client 也必须同步炉火");
        assert!(
            states[0].sound().is_none(),
            "心跳不能重播起炉、注元或收取声音"
        );
        app.world_mut()
            .get_mut::<AlchemyFurnace>(furnace)
            .unwrap()
            .session
            .as_mut()
            .unwrap()
            .finished = true;
        for _ in 0..20 {
            app.update();
        }
        let stopped: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<AlchemyWorldEffect>>()
            .drain()
            .collect();
        assert_eq!(stopped[0].heat, 0.0, "到时停炉必须停止世界火焰及燃烧声");
    }
}
