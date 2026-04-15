//! 客户端 → 服务端请求 schema（plan-cultivation-v1 §P1 剩余）。
//!
//! 与 TypeScript `agent/packages/schema/src/client-request.ts` 1:1。
//! 由 Fabric 客户端通过 Minecraft CustomPayload 发送，服务端反序列化为对应
//! Bevy Event（MeridianTarget Component 更新 / BreakthroughRequest / ForgeRequest）。

use serde::{Deserialize, Serialize};

use crate::cultivation::components::MeridianId;
use crate::cultivation::forging::ForgeAxis;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientRequestV1 {
    SetMeridianTarget {
        v: u8,
        meridian: MeridianId,
    },
    BreakthroughRequest {
        v: u8,
    },
    ForgeRequest {
        v: u8,
        meridian: MeridianId,
        axis: ForgeAxis,
    },
    /// 顿悟邀约回执：玩家选择 / 拒绝 / 超时。
    /// `choice_idx = Some(n)` → 选中第 n 个候选；`None` → 拒绝或超时（服务端等价处理）。
    InsightDecision {
        v: u8,
        trigger_id: String,
        choice_idx: Option<u32>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_meridian_target_roundtrip() {
        let json = r#"{"type":"set_meridian_target","v":1,"meridian":"Lung"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::SetMeridianTarget { v, meridian } => {
                assert_eq!(v, 1);
                assert_eq!(meridian, MeridianId::Lung);
            }
            other => panic!("expected SetMeridianTarget, got {other:?}"),
        }
    }

    #[test]
    fn breakthrough_request_roundtrip() {
        let json = r#"{"type":"breakthrough_request","v":1}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        assert!(matches!(req, ClientRequestV1::BreakthroughRequest { v: 1 }));
    }

    #[test]
    fn forge_request_roundtrip() {
        let json = r#"{"type":"forge_request","v":1,"meridian":"Ren","axis":"Rate"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::ForgeRequest { meridian, axis, .. } => {
                assert_eq!(meridian, MeridianId::Ren);
                assert!(matches!(axis, ForgeAxis::Rate));
            }
            other => panic!("expected ForgeRequest, got {other:?}"),
        }
    }

    #[test]
    fn forge_request_capacity_axis_roundtrip() {
        let v = ClientRequestV1::ForgeRequest {
            v: 1,
            meridian: MeridianId::Du,
            axis: ForgeAxis::Capacity,
        };
        let s = serde_json::to_string(&v).unwrap();
        assert!(s.contains("\"axis\":\"Capacity\""));
        let back: ClientRequestV1 = serde_json::from_str(&s).unwrap();
        assert!(matches!(
            back,
            ClientRequestV1::ForgeRequest {
                axis: ForgeAxis::Capacity,
                ..
            }
        ));
    }

    #[test]
    fn insight_decision_chosen_roundtrip() {
        let json =
            r#"{"type":"insight_decision","v":1,"trigger_id":"awaken_first","choice_idx":2}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::InsightDecision {
                v,
                trigger_id,
                choice_idx,
            } => {
                assert_eq!(v, 1);
                assert_eq!(trigger_id, "awaken_first");
                assert_eq!(choice_idx, Some(2));
            }
            other => panic!("expected InsightDecision, got {other:?}"),
        }
    }

    #[test]
    fn insight_decision_declined_roundtrip() {
        let json =
            r#"{"type":"insight_decision","v":1,"trigger_id":"awaken_first","choice_idx":null}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::InsightDecision {
                choice_idx: None,
                ..
            }
        ));
    }

    #[test]
    fn rejects_unknown_type() {
        let json = r#"{"type":"nuke_world","v":1}"#;
        assert!(serde_json::from_str::<ClientRequestV1>(json).is_err());
    }
}
