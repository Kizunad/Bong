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

const ZONG_CORE_ACTIVATION_VERSION: u8 = 1;
const ZONG_CORE_ORIGIN_ID_MIN: u8 = 1;
const ZONG_CORE_ORIGIN_ID_MAX: u8 = 7;
const ZONG_CORE_QI_MIN: f64 = 0.0;
const ZONG_CORE_QI_MAX: f64 = 1.0;
const ZONG_CORE_ANOMALY_KIND: u8 = 5;

impl ZongCoreActivationV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.v != ZONG_CORE_ACTIVATION_VERSION {
            return Err(format!(
                "ZongCoreActivationV1.v must be {ZONG_CORE_ACTIVATION_VERSION}, got {}",
                self.v
            ));
        }
        if self.zone_id.is_empty() {
            return Err("ZongCoreActivationV1.zone_id must be non-empty".into());
        }
        if self.core_id.is_empty() {
            return Err("ZongCoreActivationV1.core_id must be non-empty".into());
        }
        if !(ZONG_CORE_ORIGIN_ID_MIN..=ZONG_CORE_ORIGIN_ID_MAX).contains(&self.origin_id) {
            return Err(format!(
                "ZongCoreActivationV1.origin_id must be {ZONG_CORE_ORIGIN_ID_MIN}..={ZONG_CORE_ORIGIN_ID_MAX}, got {}",
                self.origin_id
            ));
        }
        if !self.center_xz.iter().all(|value| value.is_finite()) {
            return Err("ZongCoreActivationV1.center_xz values must be finite".into());
        }
        validate_unit_qi("base_qi", self.base_qi)?;
        validate_unit_qi("active_qi", self.active_qi)?;
        if self.charge_required.is_empty() {
            return Err("ZongCoreActivationV1.charge_required must be non-empty".into());
        }
        if self.charge_required.iter().any(|item| item.is_empty()) {
            return Err("ZongCoreActivationV1.charge_required items must be non-empty".into());
        }
        if self.narration_radius_blocks == 0 {
            return Err("ZongCoreActivationV1.narration_radius_blocks must be >= 1".into());
        }
        if self.anomaly_kind != ZONG_CORE_ANOMALY_KIND {
            return Err(format!(
                "ZongCoreActivationV1.anomaly_kind must be {ZONG_CORE_ANOMALY_KIND}, got {}",
                self.anomaly_kind
            ));
        }

        Ok(())
    }
}

fn validate_unit_qi(field: &str, value: f64) -> Result<(), String> {
    if !(value.is_finite() && (ZONG_CORE_QI_MIN..=ZONG_CORE_QI_MAX).contains(&value)) {
        return Err(format!(
            "ZongCoreActivationV1.{field} must be finite and {ZONG_CORE_QI_MIN}..={ZONG_CORE_QI_MAX}, got {value}"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_zong_core_activation() -> ZongCoreActivationV1 {
        ZongCoreActivationV1 {
            v: 1,
            zone_id: "jiuzong_bloodstream_ruin".into(),
            core_id: "core:1".into(),
            origin_id: 1,
            center_xz: [0.0, 0.0],
            activated_until_tick: 100,
            base_qi: 0.4,
            active_qi: 0.6,
            charge_required: vec!["bone_coin".into()],
            narration_radius_blocks: 1000,
            anomaly_kind: 5,
        }
    }

    fn assert_zong_core_activation_invalid(
        label: &str,
        mutate: impl FnOnce(&mut ZongCoreActivationV1),
    ) {
        let mut event = valid_zong_core_activation();
        mutate(&mut event);

        assert!(
            event.validate().is_err(),
            "{label} should violate TypeScript ZongCoreActivationV1 contract"
        );
    }

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
        assert!(event.validate().is_ok(), "sample should pass validate()");
    }

    #[test]
    fn accepts_zong_core_activation_origin_boundaries() {
        for origin_id in [ZONG_CORE_ORIGIN_ID_MIN, ZONG_CORE_ORIGIN_ID_MAX] {
            let mut event = valid_zong_core_activation();
            event.origin_id = origin_id;

            event
                .validate()
                .expect("TypeScript ZongmenOriginIdV1 boundary values should pass");
        }
    }

    #[test]
    fn rejects_zong_core_activation_contract_violations() {
        assert_zong_core_activation_invalid("version", |event| event.v = 2);
        assert_zong_core_activation_invalid("zone_id", |event| event.zone_id.clear());
        assert_zong_core_activation_invalid("core_id", |event| event.core_id.clear());
        assert_zong_core_activation_invalid("origin_id below", |event| event.origin_id = 0);
        assert_zong_core_activation_invalid("origin_id above", |event| event.origin_id = 8);
        assert_zong_core_activation_invalid("center_xz", |event| event.center_xz[0] = f64::NAN);
        assert_zong_core_activation_invalid("base_qi below", |event| event.base_qi = -0.1);
        assert_zong_core_activation_invalid("base_qi above", |event| event.base_qi = 1.1);
        assert_zong_core_activation_invalid("active_qi below", |event| event.active_qi = -0.1);
        assert_zong_core_activation_invalid("active_qi above", |event| event.active_qi = 1.1);
        assert_zong_core_activation_invalid("charge_required empty", |event| {
            event.charge_required.clear();
        });
        assert_zong_core_activation_invalid("charge_required item", |event| {
            event.charge_required[0].clear();
        });
        assert_zong_core_activation_invalid("narration_radius_blocks", |event| {
            event.narration_radius_blocks = 0;
        });
        assert_zong_core_activation_invalid("anomaly_kind", |event| event.anomaly_kind = 4);
    }
}
