//! VFX 事件（S2C CustomPayload `bong:vfx_event`）—— Rust 侧。
//!
//! 与 `agent/packages/schema/src/vfx-event.ts` 1:1 对应。Phase 1 只承载动画触发：
//!   * `play_anim`：服务端广播一次性动作
//!   * `stop_anim`：终止持续动画
//!
//! 粒子类 VFX（`plan-particle-system-v1 §2.2`）后续以新 variant 扩入同一 enum。
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

#[derive(Debug)]
pub enum VfxEventBuildError {
    Json(serde_json::Error),
    Oversize { size: usize, max: usize },
    PriorityOutOfRange { priority: u16 },
    FadeTicksOutOfRange { ticks: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VfxEventType {
    PlayAnim,
    StopAnim,
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
    StopAnim {
        target_player: String,
        anim_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        fade_out_ticks: Option<u8>,
    },
}

impl VfxEventPayloadV1 {
    pub fn payload_type(&self) -> VfxEventType {
        match self {
            Self::PlayAnim { .. } => VfxEventType::PlayAnim,
            Self::StopAnim { .. } => VfxEventType::StopAnim,
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
                if *priority < VFX_ANIM_PRIORITY_MIN || *priority > VFX_ANIM_PRIORITY_MAX {
                    return Err(VfxEventBuildError::PriorityOutOfRange {
                        priority: *priority,
                    });
                }
                if let Some(ticks) = fade_in_ticks {
                    if *ticks > VFX_FADE_TICKS_MAX {
                        return Err(VfxEventBuildError::FadeTicksOutOfRange { ticks: *ticks });
                    }
                }
            }
            Self::StopAnim { fade_out_ticks, .. } => {
                if let Some(ticks) = fade_out_ticks {
                    if *ticks > VFX_FADE_TICKS_MAX {
                        return Err(VfxEventBuildError::FadeTicksOutOfRange { ticks: *ticks });
                    }
                }
            }
        }
        Ok(())
    }
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

    pub fn payload_type(&self) -> VfxEventType {
        self.payload.payload_type()
    }

    /// 先做区间校验，再序列化 + 检查 MAX_PAYLOAD_BYTES（1024 字节，和 server_data 一致）。
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

    const TEST_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";

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
    fn deserialize_vfx_event_samples() {
        let samples = [
            include_str!("../../../agent/packages/schema/samples/vfx-event.play-anim.sample.json"),
            include_str!("../../../agent/packages/schema/samples/vfx-event.stop-anim.sample.json"),
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
    }
}
