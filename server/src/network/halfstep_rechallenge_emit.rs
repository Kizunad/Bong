//! plan-halfstep-rechallenge-integration-v1 P0/P1：半步化虚重渡触发推送。
//!
//! **P0**：监听 `HalfStepRechallengeTriggerEvent`（仅非 dormant 条目），targeted 下发
//! `halfstep_rechallenge` server_data payload + `halfstep_rechallenge_trigger_player` 音效。
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

use valence::prelude::{Client, Entity, EventReader, EventWriter, Position, Query, Res, With};

use crate::cultivation::tribulation::{
    HalfStepRechallengeTriggerEvent, HalfStepState, TribulationSettled,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::{log_payload_build_error, send_server_data_payload, RedisBridgeResource};
use crate::schema::halfstep_rechallenge::HalfStepRechallengeTriggerPayloadV1;
use crate::schema::server_data::{HalfStepRechallengeV1, ServerDataPayloadV1, ServerDataV1};
use crate::schema::tribulation::DuXuOutcomeV1;
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

pub const HALFSTEP_RECHALLENGE_AUDIO_RECIPE: &str = "halfstep_rechallenge_trigger_player";
/// P1 zone echo 音效 recipe id（plan-halfstep-buff-v1 P3 spec）。
pub const HALFSTEP_RECHALLENGE_ZONE_ECHO_AUDIO_RECIPE: &str =
    "halfstep_rechallenge_trigger_zone_echo";

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

fn send_halfstep_rechallenge_to_client(client: &mut Client, data: HalfStepRechallengeV1) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::HalfStepRechallenge(data));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(client, payload_bytes.as_slice());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::tribulation::{
        HalfStepRechallengeTriggerEvent, HalfStepState, TribulationSettled,
    };
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use crate::schema::server_data::{HalfStepRechallengeV1, ServerDataPayloadV1, ServerDataV1};
    use crate::schema::tribulation::{DuXuOutcomeV1, DuXuResultV1};
    use valence::prelude::{bevy_ecs, App, Entity, Events, Update};
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

    fn flush_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    fn collect_halfstep_rechallenge_payloads(
        helper: &mut MockClientHelper,
    ) -> Vec<HalfStepRechallengeV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload: ServerDataV1 = match serde_json::from_slice(packet.data.0 .0) {
                Ok(p) => p,
                Err(_) => continue,
            };
            if let ServerDataPayloadV1::HalfStepRechallenge(data) = payload.payload {
                payloads.push(data);
            }
        }
        payloads
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
}
