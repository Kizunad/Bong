//! plan-halfstep-rechallenge-integration-v1 P0/P1：半步化虚重渡触发推送。
//!
//! **P0**：监听 `HalfStepRechallengeTriggerEvent`（仅非 dormant 条目），targeted 下发
//! `bong:halfstep_rechallenge` 专属 JSON channel payload + `halfstep_rechallenge_trigger_player` 音效。
//!
//! ## Wire protocol（JSON 专属 channel，不走 bong:server_data/proto 路径）
//!
//! server 直接序列化 `HalfStepRechallengeV1` 为 JSON 字节，通过
//! `client.send_custom_payload(ident!("bong:halfstep_rechallenge"), &bytes)` 发送。
//! client 注册专属 channel listener（`BongNetworkHandler.registerHalfStepRechallengeChannel()`），
//! 解析 JSON 后写入 `HalfStepRechallengeStore`。
//!
//! 设计与 `bong:era_ambiance` 保持一致（era_ambiance_emit.rs L246-256）：
//! test 和 production 均走相同 JSON 路径，无 `cfg(test)/cfg(not(test))` 分叉。
//!
//! **P1**：同时 publish Redis `bong:tribulation/halfstep_rechallenge` 供 agent narration：
//! - char_id / zone_name / zone_halfstep_count（同 zone HalfStep 实体数，≥2 才产 zone echo）/ at_tick。
//! - dormant 触发方无 Position → zone_name fallback "dormant"，zone_halfstep_count = 0（agent 不产 zone echo）。
//! - 音效：zone_halfstep_count >= 2 时另发 `halfstep_rechallenge_trigger_zone_echo` 广播音效。
//!
//! HIDE payload 在两处追发：
//! 1. `/tribulation_rechallenge` 命令成功（`StartDuXuRequest` 发出后）：由 cmd 路径负责。
//! 2. 化虚结算（`TribulationSettled` outcome = Ascended 且 entity 有 `HalfStepState`）：
//!    由本模块 `emit_halfstep_rechallenge_hide_on_settle` 系统负责。

use valence::ident;
use valence::prelude::{Client, Entity, EventReader, EventWriter, Position, Query, Res, With};

use crate::cultivation::tribulation::{
    HalfStepRechallengeTriggerEvent, HalfStepState, TribulationSettled,
};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::schema::halfstep_rechallenge::HalfStepRechallengeTriggerPayloadV1;
use crate::schema::server_data::HalfStepRechallengeV1;
use crate::schema::tribulation::DuXuOutcomeV1;
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

/// S2C channel identifier for halfstep rechallenge payloads（专属 JSON channel）。
/// client 侧 `BongNetworkHandler.registerHalfStepRechallengeChannel()` 注册相同 channel。
pub const HALFSTEP_RECHALLENGE_CHANNEL: &str = "bong:halfstep_rechallenge";

pub const HALFSTEP_RECHALLENGE_AUDIO_RECIPE: &str = "halfstep_rechallenge_trigger_player";
/// P1 zone echo 音效 recipe id（plan-halfstep-buff-v1 P3 spec）。
pub const HALFSTEP_RECHALLENGE_ZONE_ECHO_AUDIO_RECIPE: &str =
    "halfstep_rechallenge_trigger_zone_echo";
/// P1 名额空出广播音效 recipe id（`AscensionQuotaOpened` 时发 broadcast，plan-halfstep-buff-v1 P3 spec）。
pub const HALFSTEP_QUOTA_RELEASE_BROADCAST_AUDIO_RECIPE: &str = "halfstep_quota_release_broadcast";

/// 监听 `HalfStepRechallengeTriggerEvent`，向非 dormant 玩家 targeted 发 HUD trigger payload
/// 并触发音效。dormant NPC 跳过（HUD 无客户端接收方）。
pub fn emit_halfstep_rechallenge_trigger(
    mut events: EventReader<HalfStepRechallengeTriggerEvent>,
    mut clients: Query<(&mut Client, &HalfStepState), With<Client>>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in events.read() {
        // dormant NPC：无 Client，跳过。
        if event.is_dormant {
            continue;
        }

        // 从 ECS 找到对应 entity 的 (Client, HalfStepState)。
        let Ok((mut client, half_step)) = clients.get_mut(event.entity) else {
            tracing::warn!(
                "[bong][halfstep-rechallenge] entity {:?} not found or missing Client/HalfStepState, skipping",
                event.entity
            );
            continue;
        };

        let data = HalfStepRechallengeV1::trigger(
            event.char_id.clone(),
            half_step.rechallenge_window_until,
            event.at_tick,
        );
        send_halfstep_rechallenge_to_client(&mut client, data);

        // 触发玩家专属音效。
        audio.send(PlaySoundRecipeRequest {
            recipe_id: HALFSTEP_RECHALLENGE_AUDIO_RECIPE.to_string(),
            instance_id: 0,
            pos: None,
            flag: None,
            volume_mul: 1.0,
            pitch_shift: 0.0,
            recipient: AudioRecipient::Single(event.entity),
        });
    }
}

/// 监听 `TribulationSettled`（outcome = Ascended），若 entity 有 `HalfStepState` 则追发 HIDE。
/// 玩家化虚 → 重渡触发窗口应随即消失，不等 client 超时。
pub fn emit_halfstep_rechallenge_hide_on_settle(
    mut events: EventReader<TribulationSettled>,
    mut clients: Query<(&mut Client, &HalfStepState), With<Client>>,
) {
    for event in events.read() {
        if event.result.outcome != DuXuOutcomeV1::Ascended {
            continue;
        }
        let Ok((mut client, _)) = clients.get_mut(event.entity) else {
            continue;
        };
        let char_id = event.result.char_id.clone();
        let hide = HalfStepRechallengeV1::hide(char_id);
        send_halfstep_rechallenge_to_client(&mut client, hide);
    }
}

// ─── P1：Redis publish system ────────────────────────────────────────────────

/// plan-halfstep-rechallenge-integration-v1 P1：
/// 监听 `HalfStepRechallengeTriggerEvent`，把触发信息 publish 到 Redis
/// `bong:tribulation/halfstep_rechallenge` 供 agent 生成 narration。
///
/// 同时在 zone_halfstep_count >= 2 时发 zone echo 音效广播。
///
/// dormant 触发方无 Position → zone_name = "dormant"，zone_halfstep_count = 0。
pub fn publish_halfstep_rechallenge_to_redis(
    mut events: EventReader<HalfStepRechallengeTriggerEvent>,
    redis: Res<RedisBridgeResource>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
    halfstep_positions: Query<(Entity, &Position, &HalfStepState)>,
    zone_registry: Option<Res<ZoneRegistry>>,
) {
    for event in events.read() {
        let (zone_name, zone_halfstep_count) = if event.is_dormant {
            // dormant 实体无 Position；zone 信息不可用，agent 不产 zone echo
            ("dormant".to_string(), 0u32)
        } else {
            // 非 dormant：从 Position 解析 zone_name
            let trigger_zone = halfstep_positions
                .get(event.entity)
                .ok()
                .map(|(_, pos, _)| resolve_zone_name(pos, &zone_registry))
                .unwrap_or_else(|| DEFAULT_SPAWN_ZONE_NAME.to_string());

            // 统计同 zone 内所有 HalfStep 实体数（含本次触发方）
            let count = halfstep_positions
                .iter()
                .filter(|(_, pos, _)| resolve_zone_name(pos, &zone_registry) == trigger_zone)
                .count() as u32;

            (trigger_zone, count)
        };

        let payload = HalfStepRechallengeTriggerPayloadV1 {
            char_id: event.char_id.clone(),
            zone_name: zone_name.clone(),
            zone_halfstep_count,
            at_tick: event.at_tick,
        };

        if let Err(error) = redis
            .tx_outbound
            .send(RedisOutbound::HalfStepRechallengeTrigger(payload))
        {
            tracing::warn!(
                "[bong][halfstep-rechallenge] failed to queue HalfStepRechallengeTriggerPayloadV1 for {}: {error}",
                event.char_id
            );
        }

        // zone echo 音效：同 zone ≥2 个 HalfStep 修士时广播环境音
        if zone_halfstep_count >= 2 {
            audio.send(PlaySoundRecipeRequest {
                recipe_id: HALFSTEP_RECHALLENGE_ZONE_ECHO_AUDIO_RECIPE.to_string(),
                instance_id: 0,
                pos: None,
                flag: None,
                volume_mul: 1.0,
                pitch_shift: 0.0,
                recipient: AudioRecipient::All,
            });
        }
    }
}

fn resolve_zone_name(pos: &Position, zone_registry: &Option<Res<ZoneRegistry>>) -> String {
    zone_registry
        .as_deref()
        .and_then(|reg| {
            reg.find_zone(crate::world::dimension::DimensionKind::Overworld, pos.get())
                .map(|z| z.name.clone())
        })
        .unwrap_or_else(|| DEFAULT_SPAWN_ZONE_NAME.to_string())
}

// ─────────────────────────────────────────────────────────────────────────────

/// 向指定 client 发送 `bong:halfstep_rechallenge` JSON payload。
///
/// 直接序列化 `HalfStepRechallengeV1` 为 JSON，经专属 channel 发送——
/// 不走 `bong:server_data` / proto 路径（该路径对 HalfStepRechallenge 无 proto 定义）。
/// 仿照 `era_ambiance_emit.rs` L246-256 的 `to_json_bytes_checked` → `send_custom_payload` 模式。
fn send_halfstep_rechallenge_to_client(client: &mut Client, data: HalfStepRechallengeV1) {
    match serde_json::to_vec(&data) {
        Ok(bytes) => {
            client.send_custom_payload(ident!("bong:halfstep_rechallenge"), &bytes);
        }
        Err(error) => {
            tracing::error!(
                "[bong][halfstep-rechallenge] failed to serialize HalfStepRechallengeV1 for \
                 channel {}: {error}",
                HALFSTEP_RECHALLENGE_CHANNEL
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::tribulation::{
        HalfStepRechallengeTriggerEvent, HalfStepState, TribulationSettled,
    };
    use crate::network::redis_bridge::RedisOutbound;
    use crate::schema::halfstep_rechallenge::HalfStepRechallengeTriggerPayloadV1;
    use crate::schema::server_data::HalfStepRechallengeV1;
    use crate::schema::tribulation::{DuXuOutcomeV1, DuXuResultV1};
    use crossbeam_channel::Receiver;
    use valence::prelude::{bevy_ecs, App, Entity, Events, Position, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn spawn_halfstep_client(
        app: &mut App,
        name: &str,
        at_tick: u64,
    ) -> (MockClientHelper, Entity) {
        let (bundle, helper) = create_mock_client(name);
        let entity = app
            .world_mut()
            .spawn((bundle, HalfStepState::new(at_tick)))
            .id();
        (helper, entity)
    }

    fn spawn_halfstep_entity(app: &mut App, at_tick: u64) -> Entity {
        app.world_mut()
            .spawn((
                Position::new([0.0_f64, 64.0, 0.0]),
                HalfStepState::new(at_tick),
            ))
            .id()
    }

    fn flush_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    /// 从 MockClientHelper 中收集所有发往 `bong:halfstep_rechallenge` 专属 channel 的 payload。
    ///
    /// 不检查 `bong:server_data` channel——该 channel 不再用于 HalfStepRechallenge。
    /// payload 直接是 `HalfStepRechallengeV1` JSON（无外层 ServerDataV1 envelope）。
    fn collect_halfstep_rechallenge_payloads(
        helper: &mut MockClientHelper,
    ) -> Vec<HalfStepRechallengeV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            // 必须是专属 channel（非 bong:server_data）
            if packet.channel.as_str() != HALFSTEP_RECHALLENGE_CHANNEL {
                continue;
            }
            // 直接反序列化 HalfStepRechallengeV1（无 ServerDataV1 外层）
            match serde_json::from_slice::<HalfStepRechallengeV1>(packet.data.0 .0) {
                Ok(data) => payloads.push(data),
                Err(err) => {
                    panic!(
                        "bong:halfstep_rechallenge payload 无法反序列化为 HalfStepRechallengeV1: \
                         {err}\n bytes={:?}",
                        packet.data.0 .0
                    );
                }
            }
        }
        payloads
    }

    /// 断言：bong:server_data channel 上不存在 HalfStepRechallenge payload。
    /// 防止未来代码意外地把 halfstep_rechallenge 通过旧路径发送（进而在 production 引起 proto panic）。
    fn assert_no_halfstep_on_server_data_channel(helper: &mut MockClientHelper) {
        use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
        use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            if let Ok(envelope) = serde_json::from_slice::<ServerDataV1>(packet.data.0 .0) {
                if matches!(
                    envelope.payload,
                    ServerDataPayloadV1::HalfStepRechallenge(_)
                ) {
                    panic!(
                        "HalfStepRechallenge 不应经由 bong:server_data channel 发送（production 会触发 proto unreachable! panic）；\
                         应使用专属 bong:halfstep_rechallenge channel"
                    );
                }
            }
        }
    }

    fn make_app() -> App {
        let mut app = App::new();
        app.add_event::<HalfStepRechallengeTriggerEvent>();
        app.add_event::<TribulationSettled>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(
            Update,
            (
                emit_halfstep_rechallenge_trigger,
                emit_halfstep_rechallenge_hide_on_settle,
            ),
        );
        app
    }

    /// 返回 (App, rx_outbound)，仅含 publish_halfstep_rechallenge_to_redis 系统。
    fn make_redis_app() -> (App, Receiver<RedisOutbound>) {
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded::<RedisOutbound>();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        let mut app = App::new();
        app.add_event::<HalfStepRechallengeTriggerEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_systems(Update, publish_halfstep_rechallenge_to_redis);
        (app, rx_outbound)
    }

    fn drain_redis_outbound(rx: &Receiver<RedisOutbound>) -> Vec<RedisOutbound> {
        let mut msgs = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            msgs.push(msg);
        }
        msgs
    }

    fn drain_halfstep_trigger_payloads(
        rx: &Receiver<RedisOutbound>,
    ) -> Vec<HalfStepRechallengeTriggerPayloadV1> {
        drain_redis_outbound(rx)
            .into_iter()
            .filter_map(|msg| {
                if let RedisOutbound::HalfStepRechallengeTrigger(p) = msg {
                    Some(p)
                } else {
                    None
                }
            })
            .collect()
    }

    /// 防回归：trigger payload 不经由 bong:server_data channel 发送。
    /// 若未来有人误将 HalfStepRechallenge 改回 server_data 路径，此测试立即撞红，
    /// 防止 production proto unreachable! panic 再次出现。
    #[test]
    fn trigger_payload_not_sent_on_server_data_channel() {
        let mut app = make_app();
        let at_tick: u64 = 9_001;
        let (mut helper, entity) = spawn_halfstep_client(&mut app, "offline:Azure", at_tick);

        app.world_mut()
            .resource_mut::<Events<HalfStepRechallengeTriggerEvent>>()
            .send(HalfStepRechallengeTriggerEvent {
                char_id: "offline:Azure".to_string(),
                entity,
                is_dormant: false,
                at_tick,
            });

        app.update();
        flush_client_packets(&mut app);
        // assert_no_halfstep_on_server_data_channel 会扫描剩余帧；
        // collect_received() 是消费性的，所以在此之前不要再调用 collect_halfstep_rechallenge_payloads。
        assert_no_halfstep_on_server_data_channel(&mut helper);
    }

    /// 防回归：HIDE payload（化虚成功）不经由 bong:server_data channel 发送。
    #[test]
    fn hide_payload_not_sent_on_server_data_channel() {
        use crate::cultivation::tribulation::TribulationKind;
        let mut app = make_app();
        let (mut helper, entity) = spawn_halfstep_client(&mut app, "offline:Azure", 500);

        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(TribulationSettled {
                entity,
                kind: TribulationKind::DuXu,
                source: None,
                result: DuXuResultV1 {
                    char_id: "offline:Azure".to_string(),
                    outcome: DuXuOutcomeV1::Ascended,
                    killer: None,
                    waves_survived: 3,
                    reason: None,
                },
            });

        app.update();
        flush_client_packets(&mut app);
        assert_no_halfstep_on_server_data_channel(&mut helper);
    }

    /// happy path：非 dormant 玩家收到 trigger payload，active=true，window 字段正确。
    #[test]
    fn non_dormant_player_receives_trigger_payload() {
        let mut app = make_app();
        let at_tick: u64 = 1000;
        let (mut helper, entity) = spawn_halfstep_client(&mut app, "offline:Azure", at_tick);

        app.world_mut()
            .resource_mut::<Events<HalfStepRechallengeTriggerEvent>>()
            .send(HalfStepRechallengeTriggerEvent {
                char_id: "offline:Azure".to_string(),
                entity,
                is_dormant: false,
                at_tick,
            });

        app.update();
        flush_client_packets(&mut app);

        let payloads = collect_halfstep_rechallenge_payloads(&mut helper);
        assert_eq!(
            payloads.len(),
            1,
            "期望玩家收到 1 条 halfstep_rechallenge trigger payload，实际 {}",
            payloads.len()
        );
        assert!(payloads[0].active, "trigger payload active 必须为 true");
        assert_eq!(
            payloads[0].char_id, "offline:Azure",
            "char_id 必须与事件一致"
        );
        assert_eq!(payloads[0].at_tick, at_tick, "at_tick 必须与事件一致");
        // 窗口截止 = entered_at + RECHALLENGE_WINDOW_TICKS
        let expected_window = HalfStepState::new(at_tick).rechallenge_window_until;
        assert_eq!(
            payloads[0].rechallenge_window_until, expected_window,
            "rechallenge_window_until 必须等于 HalfStepState::new(at_tick).rechallenge_window_until"
        );
    }

    /// dormant NPC 触发事件时，不向任何 client 发包。
    #[test]
    fn dormant_npc_trigger_sends_no_payload() {
        let mut app = make_app();
        let at_tick: u64 = 2000;
        let (mut helper, entity) = spawn_halfstep_client(&mut app, "offline:NPC", at_tick);

        app.world_mut()
            .resource_mut::<Events<HalfStepRechallengeTriggerEvent>>()
            .send(HalfStepRechallengeTriggerEvent {
                char_id: "offline:NPC".to_string(),
                entity,
                is_dormant: true,
                at_tick,
            });

        app.update();
        flush_client_packets(&mut app);

        let payloads = collect_halfstep_rechallenge_payloads(&mut helper);
        assert!(
            payloads.is_empty(),
            "dormant NPC 不应向 client 发任何 halfstep_rechallenge payload，实际 {:?}",
            payloads
        );
    }

    /// 化虚成功（Ascended）触发 HIDE payload。
    #[test]
    fn ascended_settle_sends_hide_payload() {
        use crate::cultivation::tribulation::TribulationKind;
        let mut app = make_app();
        let (mut helper, entity) = spawn_halfstep_client(&mut app, "offline:Azure", 500);

        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(TribulationSettled {
                entity,
                kind: TribulationKind::DuXu,
                source: None,
                result: DuXuResultV1 {
                    char_id: "offline:Azure".to_string(),
                    outcome: DuXuOutcomeV1::Ascended,
                    killer: None,
                    waves_survived: 3,
                    reason: None,
                },
            });

        app.update();
        flush_client_packets(&mut app);

        let payloads = collect_halfstep_rechallenge_payloads(&mut helper);
        assert_eq!(
            payloads.len(),
            1,
            "化虚成功后应发 1 条 HIDE payload，实际 {}",
            payloads.len()
        );
        assert!(!payloads[0].active, "HIDE payload 的 active 必须为 false");
        assert_eq!(payloads[0].char_id, "offline:Azure");
    }

    /// 渡劫结局非 Ascended（Failed/Killed/HalfStep）时，不发 HIDE。
    #[test]
    fn non_ascended_settle_sends_no_hide_payload() {
        use crate::cultivation::tribulation::TribulationKind;
        let mut app = make_app();
        let (mut helper, entity) = spawn_halfstep_client(&mut app, "offline:Beryl", 600);

        for outcome in [DuXuOutcomeV1::Failed, DuXuOutcomeV1::Killed] {
            app.world_mut()
                .resource_mut::<Events<TribulationSettled>>()
                .send(TribulationSettled {
                    entity,
                    kind: TribulationKind::DuXu,
                    source: None,
                    result: DuXuResultV1 {
                        char_id: "offline:Beryl".to_string(),
                        outcome,
                        killer: None,
                        waves_survived: 1,
                        reason: None,
                    },
                });
        }

        app.update();
        flush_client_packets(&mut app);

        let payloads = collect_halfstep_rechallenge_payloads(&mut helper);
        assert!(
            payloads.is_empty(),
            "非 Ascended 结局不应发 HIDE payload，实际 {:?}",
            payloads
        );
    }

    /// HIDE payload 包含正确的 active=false 和 char_id，window 字段为 0。
    #[test]
    fn hide_payload_structure_is_correct() {
        let hide = HalfStepRechallengeV1::hide("offline:Test");
        assert!(!hide.active, "hide payload active 应为 false");
        assert_eq!(hide.char_id, "offline:Test");
        assert_eq!(hide.rechallenge_window_until, 0);
        assert_eq!(hide.at_tick, 0);
    }

    /// wire JSON 结构 pin：发往 bong:halfstep_rechallenge 的字节必须是裸 HalfStepRechallengeV1 JSON，
    /// 无 ServerDataV1 外层 envelope（无 "v"/"type" 包装字段）。
    /// client 侧 BongNetworkHandler.registerHalfStepRechallengeChannel() 直接解析这个结构。
    #[test]
    fn trigger_payload_wire_json_is_bare_halfstep_rechallenge_v1() {
        let mut app = make_app();
        let at_tick: u64 = 42_000;
        let (mut helper, entity) = spawn_halfstep_client(&mut app, "offline:Azure", at_tick);

        app.world_mut()
            .resource_mut::<Events<HalfStepRechallengeTriggerEvent>>()
            .send(HalfStepRechallengeTriggerEvent {
                char_id: "offline:Azure".to_string(),
                entity,
                is_dormant: false,
                at_tick,
            });

        app.update();
        flush_client_packets(&mut app);

        // 收集原始 CustomPayloadS2c 帧，检查 JSON 结构
        let mut raw_bytes: Option<Vec<u8>> = None;
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() == HALFSTEP_RECHALLENGE_CHANNEL {
                raw_bytes = Some(packet.data.0 .0.to_vec());
                break;
            }
        }

        let bytes =
            raw_bytes.expect("应在 bong:halfstep_rechallenge channel 收到至少 1 条 payload");
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).expect("payload 应为有效 JSON");

        // 裸 HalfStepRechallengeV1：有 active/char_id/rechallenge_window_until/at_tick
        assert!(
            value.get("active").is_some(),
            "裸 HalfStepRechallengeV1 必须包含 'active' 字段；当前 JSON={value}"
        );
        assert!(
            value.get("char_id").is_some(),
            "裸 HalfStepRechallengeV1 必须包含 'char_id' 字段；当前 JSON={value}"
        );
        assert!(
            value.get("rechallenge_window_until").is_some(),
            "裸 HalfStepRechallengeV1 必须包含 'rechallenge_window_until' 字段；当前 JSON={value}"
        );
        assert!(
            value.get("at_tick").is_some(),
            "裸 HalfStepRechallengeV1 必须包含 'at_tick' 字段；当前 JSON={value}"
        );
        // 绝对不应有 ServerDataV1 外层 envelope 字段
        assert!(
            value.get("v").is_none(),
            "payload 不应包含 ServerDataV1 envelope 的 'v' 版本字段（应为裸 HalfStepRechallengeV1）；\
             当前 JSON={value}"
        );
        assert!(
            value.get("type").is_none(),
            "payload 不应包含 ServerDataV1 envelope 的 'type' 字段（应为裸 HalfStepRechallengeV1）；\
             当前 JSON={value}"
        );
        // 验证 active=true
        assert_eq!(
            value.get("active").and_then(|v| v.as_bool()),
            Some(true),
            "trigger payload active 字段应为 true；当前 JSON={value}"
        );
        assert_eq!(
            value.get("char_id").and_then(|v| v.as_str()),
            Some("offline:Azure"),
            "trigger payload char_id 字段应为 'offline:Azure'；当前 JSON={value}"
        );
    }

    /// trigger payload 中 window_until 随 HalfStepState 入队 tick 正确计算。
    #[test]
    fn trigger_payload_window_field_matches_halfstep_state() {
        let state = HalfStepState::new(12345);
        let payload =
            HalfStepRechallengeV1::trigger("offline:Azure", state.rechallenge_window_until, 12345);
        assert!(payload.active);
        assert_eq!(
            payload.rechallenge_window_until,
            state.rechallenge_window_until
        );
    }

    /// 非 HalfStep entity（无 HalfStepState component）收到事件时不崩溃且不发包。
    #[test]
    fn event_for_non_halfstep_entity_does_not_panic() {
        let mut app = make_app();
        let (bundle, mut helper) = create_mock_client("offline:NoHalfStep");
        let entity = app.world_mut().spawn(bundle).id();

        app.world_mut()
            .resource_mut::<Events<HalfStepRechallengeTriggerEvent>>()
            .send(HalfStepRechallengeTriggerEvent {
                char_id: "offline:NoHalfStep".to_string(),
                entity,
                is_dormant: false,
                at_tick: 100,
            });

        app.update();
        flush_client_packets(&mut app);

        let payloads = collect_halfstep_rechallenge_payloads(&mut helper);
        assert!(
            payloads.is_empty(),
            "无 HalfStepState 的 entity 不应收到 payload，实际 {:?}",
            payloads
        );
    }

    /// 音效请求在触发时正确发出（recipe_id 和 recipient 均正确）。
    #[test]
    fn trigger_emits_audio_request() {
        let mut app = make_app();
        let at_tick: u64 = 3000;
        let (_helper, entity) = spawn_halfstep_client(&mut app, "offline:Azure", at_tick);

        app.world_mut()
            .resource_mut::<Events<HalfStepRechallengeTriggerEvent>>()
            .send(HalfStepRechallengeTriggerEvent {
                char_id: "offline:Azure".to_string(),
                entity,
                is_dormant: false,
                at_tick,
            });

        app.update();

        let audio_events = app.world().resource::<Events<PlaySoundRecipeRequest>>();
        let mut reader = bevy_ecs::event::ManualEventReader::default();
        let requests: Vec<_> = reader.read(audio_events).collect();
        assert_eq!(
            requests.len(),
            1,
            "应发出 1 条音效请求，实际 {}",
            requests.len()
        );
        assert_eq!(
            requests[0].recipe_id, HALFSTEP_RECHALLENGE_AUDIO_RECIPE,
            "音效 recipe_id 应为 {}，实际 {}",
            HALFSTEP_RECHALLENGE_AUDIO_RECIPE, requests[0].recipe_id
        );
        assert_eq!(
            requests[0].recipient,
            AudioRecipient::Single(entity),
            "音效应 targeted 到当事玩家 entity"
        );
    }

    // ─── publish_halfstep_rechallenge_to_redis tests ─────────────────────────

    /// happy path：非 dormant 单实体触发 → HalfStepRechallengeTrigger 入队，字段正确，
    /// zone_halfstep_count == 1（单 zone，无 zone echo 音效）。
    #[test]
    fn redis_publish_enqueues_trigger_payload_for_non_dormant() {
        let (mut app, rx) = make_redis_app();
        let at_tick: u64 = 5_000;
        let entity = spawn_halfstep_entity(&mut app, at_tick);

        app.world_mut()
            .resource_mut::<Events<HalfStepRechallengeTriggerEvent>>()
            .send(HalfStepRechallengeTriggerEvent {
                char_id: "offline:Azure".to_string(),
                entity,
                is_dormant: false,
                at_tick,
            });

        app.update();

        let payloads = drain_halfstep_trigger_payloads(&rx);
        assert_eq!(
            payloads.len(),
            1,
            "非 dormant 触发应入队 1 条 HalfStepRechallengeTrigger payload，实际 {}",
            payloads.len()
        );
        assert_eq!(
            payloads[0].char_id, "offline:Azure",
            "char_id 必须与事件一致，实际 {:?}",
            payloads[0].char_id
        );
        assert_eq!(
            payloads[0].at_tick, at_tick,
            "at_tick 必须与事件一致，实际 {}",
            payloads[0].at_tick
        );
        assert_eq!(
            payloads[0].zone_halfstep_count, 1,
            "单实体触发时 zone_halfstep_count 应为 1（仅含触发方自身），实际 {}；\
             off-by-one 错误会导致 zone echo 音效在单人时误触发",
            payloads[0].zone_halfstep_count
        );
    }

    /// zone_halfstep_count off-by-one 边界：两个 HalfStep 实体时 count == 2 → zone echo 音效触发。
    #[test]
    fn redis_publish_zone_count_ge2_triggers_zone_echo_audio() {
        let (mut app, rx) = make_redis_app();
        let at_tick: u64 = 6_000;

        // 两个 HalfStep 实体在同一 zone（无 ZoneRegistry → 都归 spawn zone）
        let entity_a = spawn_halfstep_entity(&mut app, at_tick);
        let _entity_b = spawn_halfstep_entity(&mut app, at_tick);

        app.world_mut()
            .resource_mut::<Events<HalfStepRechallengeTriggerEvent>>()
            .send(HalfStepRechallengeTriggerEvent {
                char_id: "offline:Azure".to_string(),
                entity: entity_a,
                is_dormant: false,
                at_tick,
            });

        app.update();

        let payloads = drain_halfstep_trigger_payloads(&rx);
        assert_eq!(payloads.len(), 1, "应入队 1 条 trigger payload");
        assert_eq!(
            payloads[0].zone_halfstep_count, 2,
            "两个 HalfStep 实体时 zone_halfstep_count 应为 2，实际 {}；\
             必须 ≥2 才触发 zone echo narration 和音效",
            payloads[0].zone_halfstep_count
        );

        // zone echo 音效应发出
        let audio_events = app.world().resource::<Events<PlaySoundRecipeRequest>>();
        let mut reader = bevy_ecs::event::ManualEventReader::default();
        let audio_reqs: Vec<_> = reader.read(audio_events).collect();
        let zone_echo_count = audio_reqs
            .iter()
            .filter(|r| r.recipe_id == HALFSTEP_RECHALLENGE_ZONE_ECHO_AUDIO_RECIPE)
            .count();
        assert_eq!(
            zone_echo_count, 1,
            "zone_halfstep_count==2 时必须发出 1 条 zone echo 音效（{}），实际 {}",
            HALFSTEP_RECHALLENGE_ZONE_ECHO_AUDIO_RECIPE, zone_echo_count
        );
    }

    /// off-by-one 边界：单个 HalfStep 实体（count == 1）时不发 zone echo 音效。
    #[test]
    fn redis_publish_zone_count_1_no_zone_echo_audio() {
        let (mut app, _rx) = make_redis_app();
        let at_tick: u64 = 7_000;
        let entity = spawn_halfstep_entity(&mut app, at_tick);

        app.world_mut()
            .resource_mut::<Events<HalfStepRechallengeTriggerEvent>>()
            .send(HalfStepRechallengeTriggerEvent {
                char_id: "offline:Beryl".to_string(),
                entity,
                is_dormant: false,
                at_tick,
            });

        app.update();

        let audio_events = app.world().resource::<Events<PlaySoundRecipeRequest>>();
        let mut reader = bevy_ecs::event::ManualEventReader::default();
        let audio_reqs: Vec<_> = reader.read(audio_events).collect();
        let zone_echo_count = audio_reqs
            .iter()
            .filter(|r| r.recipe_id == HALFSTEP_RECHALLENGE_ZONE_ECHO_AUDIO_RECIPE)
            .count();
        assert_eq!(
            zone_echo_count, 0,
            "zone_halfstep_count==1 時絕不能發 zone echo 音效，實際發了 {}，\
             off-by-one 邊界（計數閾值 ≥2 不含 == 1）",
            zone_echo_count
        );
    }

    // ─── cross-zone count + resolve_zone_name(Some) coverage ────────────────

    /// 跨 zone 排除：注入含两个命名 zone 的 ZoneRegistry，trigger 方在 zone_a，
    /// 另一个 HalfStep 实体在 zone_b（不同 Position / zone）。
    /// 断言 zone_halfstep_count == 1（zone_b 实体被 .filter 排除，不计入 zone_a 计数）。
    /// 如果 .filter 从未真执行（如 ZoneRegistry=None 时全部归同一 fallback），
    /// 计数会错误地变成 2，本测试会撞红。
    #[test]
    fn redis_publish_cross_zone_entity_excluded_from_count() {
        use crate::world::dimension::DimensionKind;
        use crate::world::zone::Zone;
        use valence::prelude::DVec3;

        let (mut app, rx) = make_redis_app();
        let at_tick: u64 = 9_000;

        // 构造两个不重叠的命名 zone：
        //   zone_a: x=[-50,50], z=[-50,50] —— trigger 方所在 zone
        //   zone_b: x=[1000,1100], z=[1000,1100] —— 另一 HalfStep 实体所在 zone（远离 zone_a）
        // 注意：ZoneRegistry 必须含 spawn zone，否则 try_from 校验失败；
        // 但这里我们直接构造 ZoneRegistry { zones } 跳过文件验证。
        let zone_a = Zone {
            name: "zone_a".to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (
                DVec3::new(-50.0, 50.0, -50.0),
                DVec3::new(50.0, 100.0, 50.0),
            ),
            spirit_qi: 0.5,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        };
        let zone_b = Zone {
            name: "zone_b".to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (
                DVec3::new(1000.0, 50.0, 1000.0),
                DVec3::new(1100.0, 100.0, 1100.0),
            ),
            spirit_qi: 0.3,
            danger_level: 1,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        };
        let registry = ZoneRegistry {
            zones: vec![zone_a, zone_b],
        };
        app.insert_resource(registry);

        // trigger 方：在 zone_a 内
        let trigger_entity = app
            .world_mut()
            .spawn((
                Position::new([0.0_f64, 64.0, 0.0]),
                HalfStepState::new(at_tick),
            ))
            .id();

        // 另一 HalfStep 实体：在 zone_b 内（位置 [1050, 64, 1050]）
        app.world_mut().spawn((
            Position::new([1050.0_f64, 64.0, 1050.0]),
            HalfStepState::new(at_tick),
        ));

        app.world_mut()
            .resource_mut::<Events<HalfStepRechallengeTriggerEvent>>()
            .send(HalfStepRechallengeTriggerEvent {
                char_id: "offline:Azure".to_string(),
                entity: trigger_entity,
                is_dormant: false,
                at_tick,
            });

        app.update();

        let payloads = drain_halfstep_trigger_payloads(&rx);
        assert_eq!(payloads.len(), 1, "应入队 1 条 trigger payload");
        assert_eq!(
            payloads[0].zone_halfstep_count, 1,
            "zone_b 内的 HalfStep 实体不应计入 zone_a 的 count，\
             期望 zone_halfstep_count == 1（仅触发方自身），实际 {}；\
             .filter zone 判别未正确执行（跨 zone 排除失效）",
            payloads[0].zone_halfstep_count
        );
        assert_eq!(
            payloads[0].zone_name, "zone_a",
            "trigger 方在 zone_a 内，payload.zone_name 应为 \"zone_a\"（非 fallback spawn），\
             实际 {:?}；resolve_zone_name(Some(registry)) 主路径未正确解析",
            payloads[0].zone_name
        );
    }

    /// resolve_zone_name Some 主路径：注入 ZoneRegistry 后，trigger 方位置落在命名 zone
    /// 的 AABB 内，payload.zone_name 应等于该 zone 的名称（非 DEFAULT_SPAWN_ZONE_NAME fallback）。
    #[test]
    fn redis_publish_named_zone_resolves_zone_name_from_registry() {
        use crate::world::dimension::DimensionKind;
        use crate::world::zone::Zone;
        use valence::prelude::DVec3;

        let (mut app, rx) = make_redis_app();
        let at_tick: u64 = 10_000;

        // 构造一个 "qingyun_peaks" 命名 zone：trigger 方位置 [500, 70, 300] 落在此 AABB 内。
        let qingyun = Zone {
            name: "qingyun_peaks".to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (
                DVec3::new(400.0, 60.0, 200.0),
                DVec3::new(600.0, 120.0, 400.0),
            ),
            spirit_qi: 0.7,
            danger_level: 2,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        };
        let registry = ZoneRegistry {
            zones: vec![qingyun],
        };
        app.insert_resource(registry);

        let trigger_entity = app
            .world_mut()
            .spawn((
                Position::new([500.0_f64, 70.0, 300.0]),
                HalfStepState::new(at_tick),
            ))
            .id();

        app.world_mut()
            .resource_mut::<Events<HalfStepRechallengeTriggerEvent>>()
            .send(HalfStepRechallengeTriggerEvent {
                char_id: "offline:Beryl".to_string(),
                entity: trigger_entity,
                is_dormant: false,
                at_tick,
            });

        app.update();

        let payloads = drain_halfstep_trigger_payloads(&rx);
        assert_eq!(payloads.len(), 1, "应入队 1 条 trigger payload");
        assert_eq!(
            payloads[0].zone_name, "qingyun_peaks",
            "trigger 方在 qingyun_peaks AABB 内，zone_name 应解析为 \"qingyun_peaks\"，\
             实际 {:?}；期望 resolve_zone_name(Some(registry)) 走 ZoneRegistry.find_zone 主路径，\
             而非 fallback DEFAULT_SPAWN_ZONE_NAME（\"{}\"）",
            payloads[0].zone_name, DEFAULT_SPAWN_ZONE_NAME
        );
        assert_eq!(
            payloads[0].zone_halfstep_count, 1,
            "单实体在命名 zone 内，zone_halfstep_count 应为 1，实际 {}",
            payloads[0].zone_halfstep_count
        );
    }

    /// dormant 触发：不入队 Redis payload（dormant 实体无位置，agent 不产 zone echo），
    /// 但 dormant fallback zone_name == "dormant" / zone_halfstep_count == 0 的逻辑
    /// 仍会走，确认入队一条 payload（dormant 路径需要 agent 记录触发但不 echo）。
    #[test]
    fn redis_publish_dormant_enqueues_payload_with_dormant_zone() {
        let (mut app, rx) = make_redis_app();
        let at_tick: u64 = 8_000;
        // dormant 实体通常没有 Position component；这里用虚构 entity id 模拟
        let dummy_entity = app.world_mut().spawn_empty().id();

        app.world_mut()
            .resource_mut::<Events<HalfStepRechallengeTriggerEvent>>()
            .send(HalfStepRechallengeTriggerEvent {
                char_id: "npc:dormant_azure".to_string(),
                entity: dummy_entity,
                is_dormant: true,
                at_tick,
            });

        app.update();

        let payloads = drain_halfstep_trigger_payloads(&rx);
        assert_eq!(
            payloads.len(),
            1,
            "dormant 触发仍应入队 1 条 Redis payload（agent 记录用），实际 {}",
            payloads.len()
        );
        assert_eq!(
            payloads[0].char_id, "npc:dormant_azure",
            "dormant payload char_id 应保留原始值"
        );
        assert_eq!(
            payloads[0].zone_name, "dormant",
            "dormant 触发方无 Position，zone_name 应 fallback 为 \"dormant\"，实际 {:?};\
             agent 读取 zone_name==\"dormant\" 时跳过 zone echo narration",
            payloads[0].zone_name
        );
        assert_eq!(
            payloads[0].zone_halfstep_count, 0,
            "dormant 触发时 zone_halfstep_count 应为 0（无 Position 无法统计），实际 {}",
            payloads[0].zone_halfstep_count
        );

        // dormant 触发不发 zone echo 音效
        let audio_events = app.world().resource::<Events<PlaySoundRecipeRequest>>();
        let mut reader = bevy_ecs::event::ManualEventReader::default();
        let audio_reqs: Vec<_> = reader.read(audio_events).collect();
        let zone_echo_count = audio_reqs
            .iter()
            .filter(|r| r.recipe_id == HALFSTEP_RECHALLENGE_ZONE_ECHO_AUDIO_RECIPE)
            .count();
        assert_eq!(
            zone_echo_count, 0,
            "dormant 触发不应发 zone echo 音效（zone_halfstep_count == 0 < 2），实际 {}",
            zone_echo_count
        );
    }
}
