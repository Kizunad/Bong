//! VFX 事件（S2C CustomPayload `bong:vfx_event`）—— Rust 侧。
//!
//! 与 `agent/packages/schema/src/vfx-event.ts` 1:1 对应，当前支持：
//!   * `play_anim`：服务端广播一次性动作
//!   * `play_anim_inline`：携带 Emotecraft v3 JSON，客户端临时注册后立即播放
//!   * `stop_anim`：终止持续动画
//!   * `spawn_particle`：触发一次自定义粒子（`plan-particle-system-v1 §2.2`）
//!
//! 对齐方式：`agent/packages/schema/samples/vfx-event.*.sample.json` 由 Rust 测试
//! `include_str!` 反序列化，保证双端形态同步。

use serde::{Deserialize, Serialize};

use super::common::MAX_PAYLOAD_BYTES;

pub const VFX_EVENT_VERSION: u8 = 1;

/// 动画 priority 合法区间，对齐 `plan-player-animation-v1 §3.3` 分层约定。
pub const VFX_ANIM_PRIORITY_MIN: u16 = 100;
pub const VFX_ANIM_PRIORITY_MAX: u16 = 3999;

/// 淡入淡出 tick 上限（20 tick/s，即 2s）。
pub const VFX_FADE_TICKS_MAX: u8 = 40;

/// 粒子同 tick 合批上限（plan §2.5）。
pub const VFX_PARTICLE_COUNT_MAX: u16 = 64;

/// 粒子持续时间上限（tick）。20 tick/s → 10s 足够一次性事件。
pub const VFX_PARTICLE_DURATION_TICKS_MAX: u16 = 200;

/// inline 动画 JSON 字符串上限。最终 payload 仍受 `MAX_PAYLOAD_BYTES` 兜底。
pub const VFX_INLINE_ANIM_JSON_MAX_CHARS: usize = 4096;

#[derive(Debug)]
pub enum VfxEventBuildError {
    Json(serde_json::Error),
    Oversize {
        size: usize,
        max: usize,
    },
    PriorityOutOfRange {
        priority: u16,
    },
    FadeTicksOutOfRange {
        ticks: u8,
    },
    /// 粒子 `count` 越界（0 或 > `VFX_PARTICLE_COUNT_MAX`）。
    ParticleCountOutOfRange {
        count: u16,
    },
    /// 粒子 `duration_ticks` 越界。
    ParticleDurationOutOfRange {
        ticks: u16,
    },
    /// 粒子 `strength` 非有限或超出 `[0, 1]` 区间。
    ParticleStrengthOutOfRange {
        strength: f32,
    },
    /// 粒子 `origin`/`direction` 含 NaN / inf。
    ParticleVectorNotFinite,
    /// 粒子 `color` 不是 `#RRGGBB` 6-hex 形态。
    ParticleColorMalformed,
    /// inline 动画 JSON 为空或超过上限。
    InlineAnimJsonLengthOutOfRange {
        len: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VfxEventType {
    PlayAnim,
    PlayAnimInline,
    StopAnim,
    SpawnParticle,
}

/// VFX payload 判别式。`#[serde(tag = "type")]` + `rename_all = "snake_case"` 与
/// TypeBox `Type.Literal("play_anim" | "stop_anim")` 一致。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VfxEventPayloadV1 {
    PlayAnim {
        /// 玩家 UUID，RFC 4122 canonical `8-4-4-4-12` 格式。
        target_player: String,
        /// MC Identifier `namespace:path`。客户端查 `BongAnimationRegistry`。
        anim_id: String,
        /// 动画分层 priority（§3.3 区间）。
        priority: u16,
        #[serde(skip_serializing_if = "Option::is_none")]
        fade_in_ticks: Option<u8>,
    },
    PlayAnimInline {
        /// 玩家 UUID，RFC 4122 canonical `8-4-4-4-12` 格式。
        target_player: String,
        /// 客户端临时注册用 Identifier。重复 id 会覆盖旧 inline 动画。
        anim_id: String,
        /// 完整 Emotecraft v3 / PlayerAnimator JSON 字符串。
        anim_json: String,
        /// 动画分层 priority（§3.3 区间）。
        priority: u16,
        #[serde(skip_serializing_if = "Option::is_none")]
        fade_in_ticks: Option<u8>,
    },
    StopAnim {
        target_player: String,
        anim_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        fade_out_ticks: Option<u8>,
    },
    SpawnParticle {
        /// 粒子事件 id（`namespace:path`，如 `bong:sword_qi_slash`）。
        event_id: String,
        /// 世界坐标原点。
        origin: [f64; 3],
        /// 方向向量（可不归一；客户端按需 normalize）。
        #[serde(skip_serializing_if = "Option::is_none")]
        direction: Option<[f64; 3]>,
        /// `#RRGGBB` 十六进制颜色。
        #[serde(skip_serializing_if = "Option::is_none")]
        color: Option<String>,
        /// 归一化强度 `[0, 1]`。
        #[serde(skip_serializing_if = "Option::is_none")]
        strength: Option<f32>,
        /// 同 tick 合批数量（plan §2.5）。
        #[serde(skip_serializing_if = "Option::is_none")]
        count: Option<u16>,
        /// 粒子整体持续时间（tick，客户端可进一步约束）。
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ticks: Option<u16>,
    },
}

impl VfxEventPayloadV1 {
    pub fn payload_type(&self) -> VfxEventType {
        match self {
            Self::PlayAnim { .. } => VfxEventType::PlayAnim,
            Self::PlayAnimInline { .. } => VfxEventType::PlayAnimInline,
            Self::StopAnim { .. } => VfxEventType::StopAnim,
            Self::SpawnParticle { .. } => VfxEventType::SpawnParticle,
        }
    }

    /// 校验 priority / fade ticks 是否落在 schema 约束区间；越界时返回具体错误。
    ///
    /// 服务端构造 payload 前调用，避免发出 client 会拒绝的 JSON。
    pub fn validate_ranges(&self) -> Result<(), VfxEventBuildError> {
        match self {
            Self::PlayAnim {
                priority,
                fade_in_ticks,
                ..
            } => {
                validate_anim_timing(*priority, *fade_in_ticks)?;
            }
            Self::PlayAnimInline {
                priority,
                fade_in_ticks,
                anim_json,
                ..
            } => {
                validate_anim_timing(*priority, *fade_in_ticks)?;
                let len = anim_json.chars().count();
                if len == 0 || len > VFX_INLINE_ANIM_JSON_MAX_CHARS {
                    return Err(VfxEventBuildError::InlineAnimJsonLengthOutOfRange { len });
                }
            }
            Self::StopAnim { fade_out_ticks, .. } => {
                if let Some(ticks) = fade_out_ticks {
                    if *ticks > VFX_FADE_TICKS_MAX {
                        return Err(VfxEventBuildError::FadeTicksOutOfRange { ticks: *ticks });
                    }
                }
            }
            Self::SpawnParticle {
                origin,
                direction,
                color,
                strength,
                count,
                duration_ticks,
                ..
            } => {
                if !origin.iter().all(|v| v.is_finite()) {
                    return Err(VfxEventBuildError::ParticleVectorNotFinite);
                }
                if let Some(dir) = direction {
                    if !dir.iter().all(|v| v.is_finite()) {
                        return Err(VfxEventBuildError::ParticleVectorNotFinite);
                    }
                }
                if let Some(s) = strength {
                    if !s.is_finite() || *s < 0.0 || *s > 1.0 {
                        return Err(VfxEventBuildError::ParticleStrengthOutOfRange {
                            strength: *s,
                        });
                    }
                }
                if let Some(c) = count {
                    if *c == 0 || *c > VFX_PARTICLE_COUNT_MAX {
                        return Err(VfxEventBuildError::ParticleCountOutOfRange { count: *c });
                    }
                }
                if let Some(d) = duration_ticks {
                    if *d == 0 || *d > VFX_PARTICLE_DURATION_TICKS_MAX {
                        return Err(VfxEventBuildError::ParticleDurationOutOfRange { ticks: *d });
                    }
                }
                if let Some(hex) = color {
                    if !is_valid_color_hex(hex) {
                        return Err(VfxEventBuildError::ParticleColorMalformed);
                    }
                }
            }
        }
        Ok(())
    }
}

fn validate_anim_timing(
    priority: u16,
    fade_in_ticks: Option<u8>,
) -> Result<(), VfxEventBuildError> {
    if !(VFX_ANIM_PRIORITY_MIN..=VFX_ANIM_PRIORITY_MAX).contains(&priority) {
        return Err(VfxEventBuildError::PriorityOutOfRange { priority });
    }
    if let Some(ticks) = fade_in_ticks {
        if ticks > VFX_FADE_TICKS_MAX {
            return Err(VfxEventBuildError::FadeTicksOutOfRange { ticks });
        }
    }
    Ok(())
}

/// `#RRGGBB` 形态校验：7 字符，`#` 前缀 + 6 位 hex。
fn is_valid_color_hex(hex: &str) -> bool {
    if hex.len() != 7 {
        return false;
    }
    let mut it = hex.chars();
    if it.next() != Some('#') {
        return false;
    }
    it.all(|c| c.is_ascii_hexdigit())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VfxEventV1 {
    pub v: u8,
    #[serde(flatten)]
    pub payload: VfxEventPayloadV1,
}

impl VfxEventV1 {
    pub fn new(payload: VfxEventPayloadV1) -> Self {
        Self {
            v: VFX_EVENT_VERSION,
            payload,
        }
    }

    pub fn play_anim(
        target_player: impl Into<String>,
        anim_id: impl Into<String>,
        priority: u16,
        fade_in_ticks: Option<u8>,
    ) -> Self {
        Self::new(VfxEventPayloadV1::PlayAnim {
            target_player: target_player.into(),
            anim_id: anim_id.into(),
            priority,
            fade_in_ticks,
        })
    }

    pub fn play_anim_inline(
        target_player: impl Into<String>,
        anim_id: impl Into<String>,
        anim_json: impl Into<String>,
        priority: u16,
        fade_in_ticks: Option<u8>,
    ) -> Self {
        Self::new(VfxEventPayloadV1::PlayAnimInline {
            target_player: target_player.into(),
            anim_id: anim_id.into(),
            anim_json: anim_json.into(),
            priority,
            fade_in_ticks,
        })
    }

    pub fn stop_anim(
        target_player: impl Into<String>,
        anim_id: impl Into<String>,
        fade_out_ticks: Option<u8>,
    ) -> Self {
        Self::new(VfxEventPayloadV1::StopAnim {
            target_player: target_player.into(),
            anim_id: anim_id.into(),
            fade_out_ticks,
        })
    }

    /// 构造一个粒子触发事件。共享参数全都 Optional，主要为了让 gameplay 系统
    /// 在不知道某字段时不要被迫填填充值（避免"默认值"被 hard-code 进 server）。
    pub fn spawn_particle(
        event_id: impl Into<String>,
        origin: [f64; 3],
        direction: Option<[f64; 3]>,
        color: Option<String>,
        strength: Option<f32>,
        count: Option<u16>,
        duration_ticks: Option<u16>,
    ) -> Self {
        Self::new(VfxEventPayloadV1::SpawnParticle {
            event_id: event_id.into(),
            origin,
            direction,
            color,
            strength,
            count,
            duration_ticks,
        })
    }

    pub fn payload_type(&self) -> VfxEventType {
        self.payload.payload_type()
    }

    /// 先做区间校验，再序列化 + 检查 MAX_PAYLOAD_BYTES（当前与 server_data 共用上限）。
    pub fn to_json_bytes_checked(&self) -> Result<Vec<u8>, VfxEventBuildError> {
        self.payload.validate_ranges()?;
        let bytes = serde_json::to_vec(self).map_err(VfxEventBuildError::Json)?;
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(VfxEventBuildError::Oversize {
                size: bytes.len(),
                max: MAX_PAYLOAD_BYTES,
            });
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_DIAGONAL_COMPONENT: f64 = 7071.0 / 10_000.0;

    const TEST_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";

    fn inline_anim_json() -> &'static str {
        r#"{"version":3,"name":"inline_test_pose","emote":{"beginTick":0,"endTick":4,"isLoop":false,"moves":[{"tick":0,"rightArm":{"pitch":-0.6},"easing":"LINEAR"},{"tick":4,"rightArm":{"pitch":0.4},"easing":"INOUTSINE"}]}}"#
    }

    #[test]
    fn play_anim_roundtrip() {
        let payload = VfxEventV1::play_anim(TEST_UUID, "bong:sword_swing_horiz", 1000, Some(3));
        let bytes = payload.to_json_bytes_checked().expect("serialize");
        let back: VfxEventV1 = serde_json::from_slice(&bytes).expect("deserialize");
        assert_eq!(payload, back);
        match back.payload {
            VfxEventPayloadV1::PlayAnim {
                anim_id,
                priority,
                fade_in_ticks,
                ..
            } => {
                assert_eq!(anim_id, "bong:sword_swing_horiz");
                assert_eq!(priority, 1000);
                assert_eq!(fade_in_ticks, Some(3));
            }
            other => panic!("expected PlayAnim, got {other:?}"),
        }
    }

    #[test]
    fn play_anim_inline_roundtrip() {
        let payload = VfxEventV1::play_anim_inline(
            TEST_UUID,
            "bong:inline_test_pose",
            inline_anim_json(),
            3000,
            Some(3),
        );
        let bytes = payload.to_json_bytes_checked().expect("serialize");
        let back: VfxEventV1 = serde_json::from_slice(&bytes).expect("deserialize");
        assert_eq!(payload, back);
        match back.payload {
            VfxEventPayloadV1::PlayAnimInline {
                target_player,
                anim_id,
                anim_json,
                priority,
                fade_in_ticks,
            } => {
                assert_eq!(target_player, TEST_UUID);
                assert_eq!(anim_id, "bong:inline_test_pose");
                assert!(anim_json.contains("inline_test_pose"));
                assert_eq!(priority, 3000);
                assert_eq!(fade_in_ticks, Some(3));
            }
            other => panic!("expected PlayAnimInline, got {other:?}"),
        }
    }

    #[test]
    fn stop_anim_roundtrip_without_fade() {
        let payload = VfxEventV1::stop_anim(TEST_UUID, "bong:meditate_sit", None);
        let bytes = payload.to_json_bytes_checked().expect("serialize");
        // fade_out_ticks = None 时不应出现在 JSON 中（skip_serializing_if）
        let json_text = std::str::from_utf8(&bytes).unwrap();
        assert!(!json_text.contains("fade_out_ticks"));
        let back: VfxEventV1 = serde_json::from_slice(&bytes).expect("deserialize");
        assert_eq!(payload, back);
    }

    #[test]
    fn rejects_priority_out_of_range() {
        let payload = VfxEventV1::play_anim(TEST_UUID, "bong:foo", 99, None);
        match payload.to_json_bytes_checked() {
            Err(VfxEventBuildError::PriorityOutOfRange { priority }) => assert_eq!(priority, 99),
            other => panic!("expected PriorityOutOfRange, got {other:?}"),
        }

        let payload = VfxEventV1::play_anim(TEST_UUID, "bong:foo", 4000, None);
        match payload.to_json_bytes_checked() {
            Err(VfxEventBuildError::PriorityOutOfRange { priority }) => assert_eq!(priority, 4000),
            other => panic!("expected PriorityOutOfRange, got {other:?}"),
        }
    }

    #[test]
    fn rejects_fade_ticks_out_of_range() {
        let payload = VfxEventV1::play_anim(TEST_UUID, "bong:foo", 1000, Some(41));
        match payload.to_json_bytes_checked() {
            Err(VfxEventBuildError::FadeTicksOutOfRange { ticks }) => assert_eq!(ticks, 41),
            other => panic!("expected FadeTicksOutOfRange, got {other:?}"),
        }
    }

    #[test]
    fn rejects_inline_anim_json_length_out_of_range() {
        let payload = VfxEventV1::play_anim_inline(TEST_UUID, "bong:inline", "", 1000, None);
        match payload.to_json_bytes_checked() {
            Err(VfxEventBuildError::InlineAnimJsonLengthOutOfRange { len }) => assert_eq!(len, 0),
            other => panic!("expected InlineAnimJsonLengthOutOfRange, got {other:?}"),
        }

        let payload = VfxEventV1::play_anim_inline(
            TEST_UUID,
            "bong:inline",
            "x".repeat(VFX_INLINE_ANIM_JSON_MAX_CHARS + 1),
            1000,
            None,
        );
        match payload.to_json_bytes_checked() {
            Err(VfxEventBuildError::InlineAnimJsonLengthOutOfRange { len }) => {
                assert_eq!(len, VFX_INLINE_ANIM_JSON_MAX_CHARS + 1)
            }
            other => panic!("expected InlineAnimJsonLengthOutOfRange, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_vfx_event_samples() {
        let samples = [
            include_str!("../../../agent/packages/schema/samples/vfx-event.play-anim.sample.json"),
            include_str!(
                "../../../agent/packages/schema/samples/vfx-event.play-anim-inline.sample.json"
            ),
            include_str!("../../../agent/packages/schema/samples/vfx-event.stop-anim.sample.json"),
            include_str!(
                "../../../agent/packages/schema/samples/vfx-event.spawn-particle.sample.json"
            ),
        ];

        for json in samples {
            let payload: VfxEventV1 =
                serde_json::from_str(json).expect("sample should deserialize into VfxEventV1");

            let reserialized = serde_json::to_string(&payload).expect("serialize");
            let roundtrip: VfxEventV1 =
                serde_json::from_str(&reserialized).expect("deserialize again");

            let payload_value = serde_json::to_value(&payload).expect("to_value");
            let roundtrip_value = serde_json::to_value(&roundtrip).expect("to_value");
            assert_eq!(payload_value, roundtrip_value, "roundtrip must preserve");
        }
    }

    #[test]
    fn sample_play_anim_tag_alignment() {
        let json =
            include_str!("../../../agent/packages/schema/samples/vfx-event.play-anim.sample.json");
        let payload: VfxEventV1 = serde_json::from_str(json).expect("deserialize");
        assert_eq!(payload.v, VFX_EVENT_VERSION);
        match payload.payload {
            VfxEventPayloadV1::PlayAnim {
                target_player,
                anim_id,
                priority,
                fade_in_ticks,
            } => {
                assert_eq!(target_player, TEST_UUID);
                assert_eq!(anim_id, "bong:sword_swing_horiz");
                assert_eq!(priority, 1000);
                assert_eq!(fade_in_ticks, Some(3));
            }
            other => panic!("expected PlayAnim, got {other:?}"),
        }
    }

    #[test]
    fn sample_play_anim_inline_tag_alignment() {
        let json = include_str!(
            "../../../agent/packages/schema/samples/vfx-event.play-anim-inline.sample.json"
        );
        let payload: VfxEventV1 = serde_json::from_str(json).expect("deserialize");
        assert_eq!(payload.v, VFX_EVENT_VERSION);
        match payload.payload {
            VfxEventPayloadV1::PlayAnimInline {
                target_player,
                anim_id,
                anim_json,
                priority,
                fade_in_ticks,
            } => {
                assert_eq!(target_player, TEST_UUID);
                assert_eq!(anim_id, "bong:inline_test_pose");
                assert!(anim_json.contains("inline_test_pose"));
                assert_eq!(priority, 3000);
                assert_eq!(fade_in_ticks, Some(3));
            }
            other => panic!("expected PlayAnimInline, got {other:?}"),
        }
    }

    #[test]
    fn sample_stop_anim_tag_alignment() {
        let json =
            include_str!("../../../agent/packages/schema/samples/vfx-event.stop-anim.sample.json");
        let payload: VfxEventV1 = serde_json::from_str(json).expect("deserialize");
        assert_eq!(payload.v, VFX_EVENT_VERSION);
        match payload.payload {
            VfxEventPayloadV1::StopAnim {
                target_player,
                anim_id,
                fade_out_ticks,
            } => {
                assert_eq!(target_player, TEST_UUID);
                assert_eq!(anim_id, "bong:meditate_sit");
                assert_eq!(fade_out_ticks, Some(5));
            }
            other => panic!("expected StopAnim, got {other:?}"),
        }
    }

    #[test]
    fn payload_type_lookup() {
        let play = VfxEventV1::play_anim(TEST_UUID, "bong:foo", 1000, None);
        assert_eq!(play.payload_type(), VfxEventType::PlayAnim);
        let stop = VfxEventV1::stop_anim(TEST_UUID, "bong:foo", None);
        assert_eq!(stop.payload_type(), VfxEventType::StopAnim);
        let particle = VfxEventV1::spawn_particle(
            "bong:sword_qi_slash",
            [0.0, 64.0, 0.0],
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(particle.payload_type(), VfxEventType::SpawnParticle);
    }

    // ========== SpawnParticle validation ==========

    #[test]
    fn spawn_particle_roundtrip_minimal() {
        let event = VfxEventV1::spawn_particle(
            "bong:sword_qi_slash",
            [10.0, 64.0, -5.0],
            None,
            None,
            None,
            None,
            None,
        );
        let bytes = event.to_json_bytes_checked().expect("serialize");
        let text = std::str::from_utf8(&bytes).unwrap();
        // 可选字段为 None 时不应出现在 JSON
        assert!(!text.contains("direction"));
        assert!(!text.contains("color"));
        assert!(!text.contains("strength"));
        assert!(!text.contains("count"));
        assert!(!text.contains("duration_ticks"));
        let back: VfxEventV1 = serde_json::from_slice(&bytes).expect("deserialize");
        assert_eq!(event, back);
    }

    #[test]
    fn spawn_particle_roundtrip_full() {
        let event = VfxEventV1::spawn_particle(
            "bong:sword_qi_slash",
            [10.0, 64.0, -5.0],
            Some([SAMPLE_DIAGONAL_COMPONENT, 0.0, SAMPLE_DIAGONAL_COMPONENT]),
            Some("#88ccff".to_string()),
            Some(0.75),
            Some(4),
            Some(20),
        );
        let bytes = event.to_json_bytes_checked().expect("serialize");
        let back: VfxEventV1 = serde_json::from_slice(&bytes).expect("deserialize");
        assert_eq!(event, back);
    }

    #[test]
    fn spawn_particle_rejects_non_finite_origin() {
        let event = VfxEventV1::spawn_particle(
            "bong:sword_qi_slash",
            [0.0, f64::NAN, 0.0],
            None,
            None,
            None,
            None,
            None,
        );
        assert!(matches!(
            event.to_json_bytes_checked(),
            Err(VfxEventBuildError::ParticleVectorNotFinite)
        ));
    }

    #[test]
    fn spawn_particle_rejects_strength_out_of_range() {
        let event = VfxEventV1::spawn_particle(
            "bong:x",
            [0.0, 0.0, 0.0],
            None,
            None,
            Some(1.5),
            None,
            None,
        );
        assert!(matches!(
            event.to_json_bytes_checked(),
            Err(VfxEventBuildError::ParticleStrengthOutOfRange { .. })
        ));
    }

    #[test]
    fn spawn_particle_rejects_zero_count() {
        let event =
            VfxEventV1::spawn_particle("bong:x", [0.0, 0.0, 0.0], None, None, None, Some(0), None);
        assert!(matches!(
            event.to_json_bytes_checked(),
            Err(VfxEventBuildError::ParticleCountOutOfRange { count: 0 })
        ));
    }

    #[test]
    fn spawn_particle_rejects_count_above_max() {
        let event = VfxEventV1::spawn_particle(
            "bong:x",
            [0.0, 0.0, 0.0],
            None,
            None,
            None,
            Some(VFX_PARTICLE_COUNT_MAX + 1),
            None,
        );
        assert!(matches!(
            event.to_json_bytes_checked(),
            Err(VfxEventBuildError::ParticleCountOutOfRange { .. })
        ));
    }

    #[test]
    fn spawn_particle_rejects_duration_above_max() {
        let event = VfxEventV1::spawn_particle(
            "bong:x",
            [0.0, 0.0, 0.0],
            None,
            None,
            None,
            None,
            Some(VFX_PARTICLE_DURATION_TICKS_MAX + 1),
        );
        assert!(matches!(
            event.to_json_bytes_checked(),
            Err(VfxEventBuildError::ParticleDurationOutOfRange { .. })
        ));
    }

    #[test]
    fn spawn_particle_rejects_malformed_color() {
        for bad in ["#abc", "88ccff", "#88ccfg", "#88ccffaa"] {
            let event = VfxEventV1::spawn_particle(
                "bong:x",
                [0.0, 0.0, 0.0],
                None,
                Some(bad.to_string()),
                None,
                None,
                None,
            );
            assert!(
                matches!(
                    event.to_json_bytes_checked(),
                    Err(VfxEventBuildError::ParticleColorMalformed)
                ),
                "expected malformed reject for {bad}"
            );
        }
    }

    #[test]
    fn spawn_particle_accepts_boundary_values() {
        let event = VfxEventV1::spawn_particle(
            "bong:x",
            [0.0, 0.0, 0.0],
            Some([1.0, 0.0, 0.0]),
            Some("#000000".to_string()),
            Some(0.0),
            Some(1),
            Some(1),
        );
        assert!(event.to_json_bytes_checked().is_ok());
        let event = VfxEventV1::spawn_particle(
            "bong:x",
            [0.0, 0.0, 0.0],
            None,
            Some("#FFFFFF".to_string()),
            Some(1.0),
            Some(VFX_PARTICLE_COUNT_MAX),
            Some(VFX_PARTICLE_DURATION_TICKS_MAX),
        );
        assert!(event.to_json_bytes_checked().is_ok());
    }

    #[test]
    fn sample_spawn_particle_tag_alignment() {
        let json = include_str!(
            "../../../agent/packages/schema/samples/vfx-event.spawn-particle.sample.json"
        );
        let payload: VfxEventV1 = serde_json::from_str(json).expect("deserialize");
        assert_eq!(payload.v, VFX_EVENT_VERSION);
        match payload.payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                origin,
                direction,
                color,
                strength,
                count,
                duration_ticks,
            } => {
                assert_eq!(event_id, "bong:sword_qi_slash");
                assert_eq!(origin, [128.5, 64.0, -32.25]);
                assert_eq!(
                    direction,
                    Some([SAMPLE_DIAGONAL_COMPONENT, 0.0, SAMPLE_DIAGONAL_COMPONENT])
                );
                assert_eq!(color.as_deref(), Some("#88ccff"));
                assert_eq!(strength, Some(0.8));
                assert_eq!(count, Some(1));
                assert_eq!(duration_ticks, Some(20));
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }
}
