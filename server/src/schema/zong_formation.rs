//! plan-terrain-jiuzong-ruin-v1 §7 — 九宗故地阵核激活 IPC schema。
//!
//! 与 `agent/packages/schema/src/zong-formation.ts` TypeBox 定义对齐。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZongCoreActivationV1 {
    pub v: u8,
    pub zone_id: String,
    pub core_id: String,
    pub origin_id: u8,
    pub center_xz: [f64; 2],
    pub activated_until_tick: u64,
    pub base_qi: f64,
    pub active_qi: f64,
    pub charge_required: Vec<String>,
    pub narration_radius_blocks: u32,
    pub anomaly_kind: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_zong_core_activation_sample() {
        let json =
            include_str!("../../../agent/packages/schema/samples/zong-core-activation.sample.json");
        let event: ZongCoreActivationV1 =
            serde_json::from_str(json).expect("zong core activation sample should deserialize");

        assert_eq!(event.v, 1);
        assert_eq!(event.zone_id, "jiuzong_bloodstream_ruin");
        assert_eq!(event.origin_id, 1);
        assert_eq!(event.base_qi, 0.4);
        assert_eq!(event.active_qi, 0.6);
        assert_eq!(event.narration_radius_blocks, 1000);
        assert_eq!(event.anomaly_kind, 5);
    }
}
