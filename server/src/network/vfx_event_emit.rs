//! `bong:vfx_event` S2C CustomPayload 发射器（plan-player-animation-v1 §4.2）。
//!
//! 两个职责：
//!   1. [`emit_vfx_event_payloads`]：消费 Bevy 事件 [`VfxEventRequest`]，序列化成
//!      [`VfxEventV1`] JSON，按 64 格距离过滤附近 Client，发送到 `bong:vfx_event` 通道。
//!   2. [`handle_vfx_debug_commands`]：解析 `/bong-vfx play <anim_id> [priority] [fade_in]`
//!      聊天命令，把请求翻译成 [`VfxEventRequest`] 事件，供 QA/dev 手动触发动画。
//!
//! 协议层 3-stack 对齐：
//!   * TS: `agent/packages/schema/src/vfx-event.ts`
//!   * Rust: `server/src/schema/vfx_event.rs`
//!   * Client: `client/src/main/java/com/bong/client/network/VfxEventRouter.java`
//!
//! 粒子类 VFX（`plan-particle-system-v1 §2.2`）日后以新 variant 复用同一 channel。

use valence::message::ChatMessageEvent;
use valence::message::SendMessage;
use valence::prelude::{
    bevy_ecs, ident, Client, DVec3, Entity, Event, EventReader, EventWriter, Position, Query,
    UniqueId, Uuid, With,
};

use crate::schema::vfx_event::{
    VfxEventPayloadV1, VfxEventV1, VFX_ANIM_PRIORITY_MAX, VFX_ANIM_PRIORITY_MIN,
    VFX_FADE_TICKS_MAX, VFX_PARTICLE_COUNT_MAX,
};

pub const VFX_EVENT_CHANNEL: &str = "bong:vfx_event";

/// Phase 1 默认广播半径（plan §4.2）。后续可按配置下调或按 zone 差异化。
pub const VFX_BROADCAST_RADIUS: f64 = 64.0;

/// `/bong-vfx play` 默认 priority —— 落在战斗层中段（plan §3.3: 战斗 1000~1999）。
pub const DEFAULT_DEBUG_PRIORITY: u16 = 1000;

/// `/bong-vfx play` 默认 fade_in_ticks —— 3 tick ≈ 150ms，平顺不拖沓。
pub const DEFAULT_DEBUG_FADE_IN_TICKS: u8 = 3;

/// gameplay 系统构造此事件，由 [`emit_vfx_event_payloads`] 负责序列化与广播。
///
/// `origin` 用于 §4.2 距离过滤；通常等于 `payload.target_player` 的当前 `Position`。
/// payload 里的 `target_player` 是 UUID 字符串，走客户端 `BongAnimationPlayer.play` 寻人。
#[derive(Debug, Clone, Event)]
pub struct VfxEventRequest {
    pub origin: DVec3,
    pub payload: VfxEventPayloadV1,
}

impl VfxEventRequest {
    pub fn new(origin: DVec3, payload: VfxEventPayloadV1) -> Self {
        Self { origin, payload }
    }
}

/// 距离过滤纯函数。拆成独立函数主要是单测可达性——Valence 的 `Position` / `Query`
/// 在 App 之外构造成本很高。`<=` 让正好 64 格边界的玩家也被纳入广播，避免抖动。
pub fn is_within_vfx_broadcast_radius(origin: DVec3, target: DVec3) -> bool {
    origin.distance_squared(target) <= VFX_BROADCAST_RADIUS * VFX_BROADCAST_RADIUS
}

/// 将 [`VfxEventRequest`] → [`VfxEventV1`] JSON → `send_custom_payload`。
///
/// - 序列化失败（priority / fade_ticks 越界、payload oversize、json 编码失败）
///   记 warn 并跳过，单事件失败不影响同 tick 其余事件。
/// - 半径过滤走 `distance_squared`（省 sqrt），<200 玩家场景下线性扫描足够。
pub fn emit_vfx_event_payloads(
    mut reader: EventReader<VfxEventRequest>,
    mut clients: Query<(&mut Client, &Position), With<Client>>,
) {
    for request in reader.read() {
        let event = VfxEventV1::new(request.payload.clone());
        let payload_type = event.payload_type();
        let bytes = match event.to_json_bytes_checked() {
            Ok(bytes) => bytes,
            Err(err) => {
                tracing::warn!(
                    "[bong][vfx_event] dropping type={payload_type:?} origin={:?}: {err:?}",
                    request.origin
                );
                continue;
            }
        };

        let mut recipients = 0usize;
        for (mut client, position) in &mut clients {
            if !is_within_vfx_broadcast_radius(request.origin, position.get()) {
                continue;
            }
            let _ = VFX_EVENT_CHANNEL; // channel 常量，对应下面的 ident! 字面量
            client.send_custom_payload(ident!("bong:vfx_event"), &bytes);
            recipients += 1;
        }

        tracing::debug!(
            "[bong][vfx_event] dispatched type={payload_type:?} to {recipients} client(s) within {} blocks of {:?}",
            VFX_BROADCAST_RADIUS,
            request.origin
        );
    }
}

/// QA 辅助命令：`/bong-vfx play <anim_id> [priority] [fade_in_ticks]`
/// → 构造 [`VfxEventRequest::PlayAnim`] 并派发。调用方玩家本身即 `target_player`，
/// 用于独自测试某个动画能否正确触发与回显。
///
/// 约束：
///  * 只识别 `/bong-vfx` 前缀；`/bong combat …`、`!spawn` 等既有命令不受影响。
///  * `anim_id` 必须是 `namespace:path`，priority 缺省 `DEFAULT_DEBUG_PRIORITY`，
///    fade_in_ticks 缺省 `DEFAULT_DEBUG_FADE_IN_TICKS`。
///  * priority/fade 超出 schema 合法区间时自动 clamp 到边界——dev 体验优先。
pub fn handle_vfx_debug_commands(
    mut events: EventReader<ChatMessageEvent>,
    players: Query<(Entity, &UniqueId, &Position), With<Client>>,
    mut clients: Query<&mut Client, With<Client>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for ChatMessageEvent {
        client, message, ..
    } in events.read()
    {
        let trimmed = message.trim();
        if !trimmed.starts_with("/bong-vfx") {
            continue;
        }

        let Ok((_, unique_id, position)) = players.get(*client) else {
            continue;
        };

        let origin = position.get();
        let outcome =
            parse_vfx_debug_command_with_origin(trimmed, unique_id.0, [origin.x, origin.y, origin.z]);
        match outcome {
            VfxDebugCommand::Usage(hint) => {
                if let Ok(mut c) = clients.get_mut(*client) {
                    c.send_chat_message(hint);
                }
            }
            VfxDebugCommand::Play { payload } => {
                let id = anim_id_from_payload(&payload).to_string();
                let kind = match &payload {
                    VfxEventPayloadV1::PlayAnim { .. } => "play",
                    VfxEventPayloadV1::StopAnim { .. } => "stop",
                    VfxEventPayloadV1::SpawnParticle { .. } => "particle",
                };
                vfx_events.send(VfxEventRequest::new(origin, payload));
                if let Ok(mut c) = clients.get_mut(*client) {
                    c.send_chat_message(format!("/bong-vfx {kind} dispatched: {id}"));
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum VfxDebugCommand {
    Play { payload: VfxEventPayloadV1 },
    Usage(&'static str),
}

const USAGE_HINT: &str = "Usage: /bong-vfx play <anim_id> [priority] [fade_in_ticks] | /bong-vfx particle <event_id> [#RRGGBB] [strength] [count]";
const ANIM_ID_HINT: &str = "anim_id must be namespace:path (e.g. bong:sword_swing_horiz)";
const EVENT_ID_HINT: &str = "event_id must be namespace:path (e.g. bong:sword_qi_slash)";
const COLOR_HINT: &str = "color must be #RRGGBB (6 hex digits)";
const STRENGTH_HINT: &str = "strength must be a number in [0.0, 1.0]";

/// 默认粒子强度（plan-particle-system-v1 §4.4 颜色强度参考）。
const DEFAULT_PARTICLE_STRENGTH: f32 = 0.8;
/// 默认粒子 count（单发）。
const DEFAULT_PARTICLE_COUNT: u16 = 1;
/// 默认粒子持续 tick 数（= 20 tick / 1s）。
const DEFAULT_PARTICLE_DURATION_TICKS: u16 = 20;

/// 2-arg 便捷版：老测试用。生产路径走 [`parse_vfx_debug_command_with_origin`]。
#[cfg(test)]
fn parse_vfx_debug_command(message: &str, target_uuid: Uuid) -> VfxDebugCommand {
    parse_vfx_debug_command_with_origin(message, target_uuid, [0.0, 0.0, 0.0])
}

/// 独立出这一层的原因：`particle` 子命令需要 origin（调用方的 `Position`），
/// 而原 `play` 流程只用 `target_uuid`。测试时传 `[0.0, 0.0, 0.0]` 即可。
fn parse_vfx_debug_command_with_origin(
    message: &str,
    target_uuid: Uuid,
    origin: [f64; 3],
) -> VfxDebugCommand {
    let mut tokens = message.split_whitespace();
    let _command = tokens.next(); // "/bong-vfx"
    let Some(sub) = tokens.next() else {
        return VfxDebugCommand::Usage(USAGE_HINT);
    };

    match sub {
        "particle" => {
            let Some(event_id) = tokens.next() else {
                return VfxDebugCommand::Usage(USAGE_HINT);
            };
            if !is_valid_anim_id_shape(event_id) {
                return VfxDebugCommand::Usage(EVENT_ID_HINT);
            }

            // 颜色可选。提供了但不合法 → 报错（dev 想写颜色却拼错时要知情）。
            let color = match tokens.next() {
                Some(token) => {
                    if is_valid_color_hex_shape(token) {
                        Some(token.to_string())
                    } else {
                        return VfxDebugCommand::Usage(COLOR_HINT);
                    }
                }
                None => None,
            };

            let strength = match tokens.next() {
                Some(token) => match token.parse::<f32>() {
                    Ok(v) if v.is_finite() && (0.0..=1.0).contains(&v) => Some(v),
                    _ => return VfxDebugCommand::Usage(STRENGTH_HINT),
                },
                None => Some(DEFAULT_PARTICLE_STRENGTH),
            };

            let count = tokens
                .next()
                .and_then(|s| s.parse::<u16>().ok())
                .map(|c| c.clamp(1, VFX_PARTICLE_COUNT_MAX))
                .unwrap_or(DEFAULT_PARTICLE_COUNT);

            // direction 由"玩家朝向"决定太复杂；debug 命令用固定 +X 朝向，配合 origin 足够
            // 验证端到端链路。正式 gameplay 系统会从 player look 向量取。
            VfxDebugCommand::Play {
                payload: VfxEventPayloadV1::SpawnParticle {
                    event_id: event_id.to_string(),
                    origin,
                    direction: Some([1.0, 0.0, 0.0]),
                    color,
                    strength,
                    count: Some(count),
                    duration_ticks: Some(DEFAULT_PARTICLE_DURATION_TICKS),
                },
            }
        }
        "play" => {
            // play 子命令不用 origin——它只广播动画；保留签名统一是为了测试对称。
            let _ = origin;
            let Some(anim_id) = tokens.next() else {
                return VfxDebugCommand::Usage(USAGE_HINT);
            };
            if !is_valid_anim_id_shape(anim_id) {
                return VfxDebugCommand::Usage(ANIM_ID_HINT);
            }

            let priority = tokens
                .next()
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(DEFAULT_DEBUG_PRIORITY)
                .clamp(VFX_ANIM_PRIORITY_MIN, VFX_ANIM_PRIORITY_MAX);

            let fade_in_ticks = tokens
                .next()
                .and_then(|s| s.parse::<u8>().ok())
                .map(|t| t.min(VFX_FADE_TICKS_MAX))
                .unwrap_or(DEFAULT_DEBUG_FADE_IN_TICKS);

            VfxDebugCommand::Play {
                payload: VfxEventPayloadV1::PlayAnim {
                    target_player: target_uuid.to_string(),
                    anim_id: anim_id.to_string(),
                    priority,
                    fade_in_ticks: Some(fade_in_ticks),
                },
            }
        }
        _ => VfxDebugCommand::Usage(USAGE_HINT),
    }
}

fn is_valid_anim_id_shape(anim_id: &str) -> bool {
    let Some((namespace, path)) = anim_id.split_once(':') else {
        return false;
    };
    !namespace.is_empty() && !path.is_empty()
}

/// `#RRGGBB` 形态（6 位 hex）。与 `VfxEventPayloadV1::validate_ranges` 的 `is_valid_color_hex`
/// 一致，但那个在 schema 模块里是 private，为保持模块边界干净这里独立实现。
fn is_valid_color_hex_shape(hex: &str) -> bool {
    hex.len() == 7
        && hex.starts_with('#')
        && hex[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn anim_id_from_payload(payload: &VfxEventPayloadV1) -> &str {
    match payload {
        VfxEventPayloadV1::PlayAnim { anim_id, .. } => anim_id,
        VfxEventPayloadV1::StopAnim { anim_id, .. } => anim_id,
        // 粒子 variant 没有 anim_id 概念；debug 命令目前只生产 Play/Stop，
        // 保底返回 event_id 以便日志不致 panic。
        VfxEventPayloadV1::SpawnParticle { event_id, .. } => event_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    const TEST_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";

    fn test_uuid() -> Uuid {
        Uuid::parse_str(TEST_UUID).unwrap()
    }

    // ========== is_within_vfx_broadcast_radius ==========

    #[test]
    fn within_radius_at_zero_distance() {
        let origin = DVec3::new(0.0, 64.0, 0.0);
        assert!(is_within_vfx_broadcast_radius(origin, origin));
    }

    #[test]
    fn within_radius_just_under_limit() {
        let origin = DVec3::new(0.0, 64.0, 0.0);
        let target = DVec3::new(63.9, 64.0, 0.0);
        assert!(is_within_vfx_broadcast_radius(origin, target));
    }

    #[test]
    fn within_radius_at_exact_boundary() {
        // distance_squared == 4096 == 64*64；<= 判定把正好 64 的人也纳入，避免边界抖动
        let origin = DVec3::new(0.0, 64.0, 0.0);
        let target = DVec3::new(64.0, 64.0, 0.0);
        assert!(is_within_vfx_broadcast_radius(origin, target));
    }

    #[test]
    fn out_of_radius_beyond_limit() {
        let origin = DVec3::new(0.0, 64.0, 0.0);
        let target = DVec3::new(64.5, 64.0, 0.0);
        assert!(!is_within_vfx_broadcast_radius(origin, target));
    }

    #[test]
    fn out_of_radius_via_vertical_only() {
        let origin = DVec3::new(0.0, 0.0, 0.0);
        let target = DVec3::new(0.0, 100.0, 0.0);
        assert!(!is_within_vfx_broadcast_radius(origin, target));
    }

    #[test]
    fn within_radius_via_diagonal() {
        let origin = DVec3::new(0.0, 0.0, 0.0);
        // sqrt(30^2 + 30^2 + 30^2) ≈ 51.96，仍 ≤ 64
        let target = DVec3::new(30.0, 30.0, 30.0);
        assert!(is_within_vfx_broadcast_radius(origin, target));
    }

    #[test]
    fn out_of_radius_via_diagonal() {
        let origin = DVec3::new(0.0, 0.0, 0.0);
        // sqrt(40^2 + 40^2 + 40^2) ≈ 69.28 > 64
        let target = DVec3::new(40.0, 40.0, 40.0);
        assert!(!is_within_vfx_broadcast_radius(origin, target));
    }

    // ========== parse_vfx_debug_command ==========

    #[test]
    fn parse_play_with_defaults() {
        match parse_vfx_debug_command("/bong-vfx play bong:sword_swing_horiz", test_uuid()) {
            VfxDebugCommand::Play {
                payload:
                    VfxEventPayloadV1::PlayAnim {
                        target_player,
                        anim_id,
                        priority,
                        fade_in_ticks,
                    },
            } => {
                assert_eq!(target_player, TEST_UUID);
                assert_eq!(anim_id, "bong:sword_swing_horiz");
                assert_eq!(priority, DEFAULT_DEBUG_PRIORITY);
                assert_eq!(fade_in_ticks, Some(DEFAULT_DEBUG_FADE_IN_TICKS));
            }
            other => panic!("expected Play, got {other:?}"),
        }
    }

    #[test]
    fn parse_play_with_explicit_priority_and_fade() {
        match parse_vfx_debug_command("/bong-vfx play bong:meditate_sit 500 10", test_uuid()) {
            VfxDebugCommand::Play {
                payload:
                    VfxEventPayloadV1::PlayAnim {
                        priority,
                        fade_in_ticks,
                        ..
                    },
            } => {
                assert_eq!(priority, 500);
                assert_eq!(fade_in_ticks, Some(10));
            }
            other => panic!("expected Play, got {other:?}"),
        }
    }

    #[test]
    fn parse_play_clamps_priority_above_max() {
        match parse_vfx_debug_command("/bong-vfx play bong:foo 9999", test_uuid()) {
            VfxDebugCommand::Play {
                payload: VfxEventPayloadV1::PlayAnim { priority, .. },
            } => assert_eq!(priority, VFX_ANIM_PRIORITY_MAX),
            other => panic!("expected Play, got {other:?}"),
        }
    }

    #[test]
    fn parse_play_clamps_priority_below_min() {
        match parse_vfx_debug_command("/bong-vfx play bong:foo 10", test_uuid()) {
            VfxDebugCommand::Play {
                payload: VfxEventPayloadV1::PlayAnim { priority, .. },
            } => assert_eq!(priority, VFX_ANIM_PRIORITY_MIN),
            other => panic!("expected Play, got {other:?}"),
        }
    }

    #[test]
    fn parse_play_clamps_fade_ticks_above_max() {
        match parse_vfx_debug_command("/bong-vfx play bong:foo 1000 99", test_uuid()) {
            VfxDebugCommand::Play {
                payload: VfxEventPayloadV1::PlayAnim { fade_in_ticks, .. },
            } => assert_eq!(fade_in_ticks, Some(VFX_FADE_TICKS_MAX)),
            other => panic!("expected Play, got {other:?}"),
        }
    }

    #[test]
    fn parse_play_rejects_anim_id_without_colon() {
        match parse_vfx_debug_command("/bong-vfx play sword_swing", test_uuid()) {
            VfxDebugCommand::Usage(hint) => assert!(hint.contains("namespace:path")),
            other => panic!("expected Usage, got {other:?}"),
        }
    }

    #[test]
    fn parse_play_rejects_anim_id_empty_parts() {
        match parse_vfx_debug_command("/bong-vfx play :path", test_uuid()) {
            VfxDebugCommand::Usage(_) => {}
            other => panic!("expected Usage, got {other:?}"),
        }
        match parse_vfx_debug_command("/bong-vfx play bong:", test_uuid()) {
            VfxDebugCommand::Usage(_) => {}
            other => panic!("expected Usage, got {other:?}"),
        }
    }

    #[test]
    fn parse_missing_subcommand_returns_usage() {
        match parse_vfx_debug_command("/bong-vfx", test_uuid()) {
            VfxDebugCommand::Usage(_) => {}
            other => panic!("expected Usage, got {other:?}"),
        }
    }

    #[test]
    fn parse_unknown_subcommand_returns_usage() {
        match parse_vfx_debug_command("/bong-vfx foobar", test_uuid()) {
            VfxDebugCommand::Usage(_) => {}
            other => panic!("expected Usage, got {other:?}"),
        }
    }

    #[test]
    fn parse_play_missing_anim_id_returns_usage() {
        match parse_vfx_debug_command("/bong-vfx play", test_uuid()) {
            VfxDebugCommand::Usage(_) => {}
            other => panic!("expected Usage, got {other:?}"),
        }
    }

    // ========== /bong-vfx particle ==========

    fn test_origin() -> [f64; 3] {
        [42.0, 64.0, -7.0]
    }

    #[test]
    fn parse_particle_with_defaults() {
        match parse_vfx_debug_command_with_origin(
            "/bong-vfx particle bong:sword_qi_slash",
            test_uuid(),
            test_origin(),
        ) {
            VfxDebugCommand::Play {
                payload: VfxEventPayloadV1::SpawnParticle {
                    event_id,
                    origin,
                    color,
                    strength,
                    count,
                    duration_ticks,
                    direction,
                },
            } => {
                assert_eq!(event_id, "bong:sword_qi_slash");
                assert_eq!(origin, test_origin());
                assert!(color.is_none(), "color not provided -> None");
                assert_eq!(strength, Some(DEFAULT_PARTICLE_STRENGTH));
                assert_eq!(count, Some(DEFAULT_PARTICLE_COUNT));
                assert_eq!(duration_ticks, Some(DEFAULT_PARTICLE_DURATION_TICKS));
                assert_eq!(direction, Some([1.0, 0.0, 0.0]));
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    #[test]
    fn parse_particle_with_color_strength_count() {
        match parse_vfx_debug_command_with_origin(
            "/bong-vfx particle bong:sword_qi_slash #ffaa00 0.5 3",
            test_uuid(),
            test_origin(),
        ) {
            VfxDebugCommand::Play {
                payload: VfxEventPayloadV1::SpawnParticle {
                    color,
                    strength,
                    count,
                    ..
                },
            } => {
                assert_eq!(color.as_deref(), Some("#ffaa00"));
                assert_eq!(strength, Some(0.5));
                assert_eq!(count, Some(3));
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    #[test]
    fn parse_particle_rejects_bad_color() {
        match parse_vfx_debug_command_with_origin(
            "/bong-vfx particle bong:x nothex",
            test_uuid(),
            test_origin(),
        ) {
            VfxDebugCommand::Usage(hint) => assert!(hint.contains("#RRGGBB")),
            other => panic!("expected Usage, got {other:?}"),
        }
    }

    #[test]
    fn parse_particle_rejects_strength_out_of_range() {
        match parse_vfx_debug_command_with_origin(
            "/bong-vfx particle bong:x #ffffff 1.5",
            test_uuid(),
            test_origin(),
        ) {
            VfxDebugCommand::Usage(hint) => assert!(hint.contains("strength")),
            other => panic!("expected Usage, got {other:?}"),
        }
    }

    #[test]
    fn parse_particle_rejects_bad_event_id() {
        match parse_vfx_debug_command_with_origin(
            "/bong-vfx particle sword_qi",
            test_uuid(),
            test_origin(),
        ) {
            VfxDebugCommand::Usage(hint) => assert!(hint.contains("namespace:path")),
            other => panic!("expected Usage, got {other:?}"),
        }
    }

    #[test]
    fn parse_particle_clamps_count_above_max() {
        match parse_vfx_debug_command_with_origin(
            "/bong-vfx particle bong:x #ffffff 0.5 9999",
            test_uuid(),
            test_origin(),
        ) {
            VfxDebugCommand::Play {
                payload: VfxEventPayloadV1::SpawnParticle { count, .. },
            } => assert_eq!(count, Some(VFX_PARTICLE_COUNT_MAX)),
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    #[test]
    fn particle_command_builds_serializable_payload() {
        let cmd = parse_vfx_debug_command_with_origin(
            "/bong-vfx particle bong:sword_qi_slash #88ccff 0.8 2",
            test_uuid(),
            test_origin(),
        );
        let VfxDebugCommand::Play { payload } = cmd else {
            panic!("expected Play, got {cmd:?}");
        };
        let event = VfxEventV1::new(payload);
        let bytes = event
            .to_json_bytes_checked()
            .expect("particle debug payload should serialize");
        let back: VfxEventV1 = serde_json::from_slice(&bytes).unwrap();
        match back.payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                origin,
                color,
                strength,
                count,
                ..
            } => {
                assert_eq!(event_id, "bong:sword_qi_slash");
                assert_eq!(origin, test_origin());
                assert_eq!(color.as_deref(), Some("#88ccff"));
                assert_eq!(strength, Some(0.8));
                assert_eq!(count, Some(2));
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    // ========== payload 构造端到端（不入 ECS）==========

    #[test]
    fn play_command_builds_serializable_payload() {
        let cmd = parse_vfx_debug_command("/bong-vfx play bong:meditate_sit 800 5", test_uuid());
        let VfxDebugCommand::Play { payload } = cmd else {
            panic!("expected Play, got {cmd:?}");
        };
        let event = VfxEventV1::new(payload);
        let bytes = event
            .to_json_bytes_checked()
            .expect("debug-built payload should serialize");
        // 反序列化回来应当 roundtrip（同一 UUID、anim_id、priority、fade_in_ticks）
        let back: VfxEventV1 = serde_json::from_slice(&bytes).expect("json should be valid");
        match back.payload {
            VfxEventPayloadV1::PlayAnim {
                target_player,
                anim_id,
                priority,
                fade_in_ticks,
            } => {
                assert_eq!(target_player, TEST_UUID);
                assert_eq!(anim_id, "bong:meditate_sit");
                assert_eq!(priority, 800);
                assert_eq!(fade_in_ticks, Some(5));
            }
            other => panic!("expected PlayAnim, got {other:?}"),
        }
    }

    // ========== emit_vfx_event_payloads ECS 集成 ==========
    //
    // 两个 mock client 分别放在半径内外；系统应当只把 CustomPayloadS2c 发给近的那个。

    fn setup_vfx_emit_app() -> App {
        let mut app = App::new();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_vfx_event_payloads);
        app
    }

    fn spawn_mock_client_at(app: &mut App, name: &str, pos: [f64; 3]) -> MockClientHelper {
        let (mut bundle, helper) = create_mock_client(name);
        bundle.player.position = Position::new(pos);
        app.world_mut().spawn(bundle);
        helper
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

    fn count_vfx_channel_packets(helper: &mut MockClientHelper) -> Vec<VfxEventV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != VFX_EVENT_CHANNEL {
                continue;
            }
            let payload: VfxEventV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("vfx custom payload should decode as VfxEventV1 JSON");
            payloads.push(payload);
        }
        payloads
    }

    #[test]
    fn emit_only_delivers_within_64_blocks() {
        let mut app = setup_vfx_emit_app();
        let mut near_helper = spawn_mock_client_at(&mut app, "Near", [10.0, 64.0, 10.0]);
        let mut far_helper = spawn_mock_client_at(&mut app, "Far", [1000.0, 64.0, 1000.0]);

        app.world_mut().send_event(VfxEventRequest::new(
            DVec3::new(10.0, 64.0, 10.0),
            VfxEventPayloadV1::PlayAnim {
                target_player: TEST_UUID.to_string(),
                anim_id: "bong:sword_swing_horiz".to_string(),
                priority: 1000,
                fade_in_ticks: Some(3),
            },
        ));

        app.update();
        flush_all_client_packets(&mut app);

        let near_payloads = count_vfx_channel_packets(&mut near_helper);
        let far_payloads = count_vfx_channel_packets(&mut far_helper);

        assert_eq!(
            near_payloads.len(),
            1,
            "near client should receive exactly one vfx payload"
        );
        assert!(
            far_payloads.is_empty(),
            "far client should not receive vfx payload (filtered at 64-block radius)"
        );

        match &near_payloads[0].payload {
            VfxEventPayloadV1::PlayAnim {
                anim_id, priority, ..
            } => {
                assert_eq!(anim_id, "bong:sword_swing_horiz");
                assert_eq!(*priority, 1000);
            }
            other => panic!("expected PlayAnim, got {other:?}"),
        }
    }

    #[test]
    fn emit_drops_oversize_payload_without_crashing() {
        // 单独伪造一个超过 MAX_PAYLOAD_BYTES 的 anim_id，触发 to_json_bytes_checked 里的 Oversize 分支。
        let mut app = setup_vfx_emit_app();
        let mut helper = spawn_mock_client_at(&mut app, "Near", [10.0, 64.0, 10.0]);

        app.world_mut().send_event(VfxEventRequest::new(
            DVec3::new(10.0, 64.0, 10.0),
            VfxEventPayloadV1::PlayAnim {
                target_player: TEST_UUID.to_string(),
                anim_id: format!(
                    "bong:{}",
                    "a".repeat(crate::schema::common::MAX_PAYLOAD_BYTES * 2)
                ),
                priority: 1000,
                fade_in_ticks: Some(3),
            },
        ));

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = count_vfx_channel_packets(&mut helper);
        assert!(
            payloads.is_empty(),
            "oversize payload must be dropped rather than sent"
        );
    }

    #[test]
    fn emit_drops_out_of_range_priority_without_crashing() {
        let mut app = setup_vfx_emit_app();
        let mut helper = spawn_mock_client_at(&mut app, "Near", [10.0, 64.0, 10.0]);

        app.world_mut().send_event(VfxEventRequest::new(
            DVec3::new(10.0, 64.0, 10.0),
            VfxEventPayloadV1::PlayAnim {
                target_player: TEST_UUID.to_string(),
                anim_id: "bong:foo".to_string(),
                priority: 9999, // > VFX_ANIM_PRIORITY_MAX, validate_ranges 应拦截
                fade_in_ticks: Some(3),
            },
        ));

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = count_vfx_channel_packets(&mut helper);
        assert!(
            payloads.is_empty(),
            "priority out of range should fail validation before send"
        );
    }
}
