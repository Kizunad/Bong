//! plan-combat-skill-feedback-bridges-v1 P1 — baomai_v4 Redis payload serde structs。
//!
//! 与 agent `@bong/schema` `baomai-v4.ts` TypeBox 定义 1:1 对应。
//! 双端 sample 对拍（共享 `agent/packages/schema/samples/baomai_v4_*.json`）。
//!
//! 5 个变体（crack_reading 走 S2C CustomPayload，不在此处）：
//!   - BaomaiV4ScarCircuitFormedV1   → bong:baomai_v4/scar_circuit_formed
//!   - BaomaiV4ScarCircuitBrokenV1   → bong:baomai_v4/scar_circuit_broken
//!   - BaomaiV4IronCocoonStageUpV1   → bong:baomai_v4/iron_cocoon_stage_up
//!   - BaomaiV4ResonanceLockV1       → bong:baomai_v4/resonance_lock
//!   - BaomaiV4ResonanceLockEndV1    → bong:baomai_v4/resonance_lock_end

use serde::{Deserialize, Serialize};

// ── ScarCircuitKind (wire repr) ─────────────────────────────────────────────

/// 7 种疤纹回路 wire 字符串（小写 snake_case，与 TypeBox ScarCircuitKind 对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScarCircuitKindWire {
    TigerMouth,
    TripleYang,
    HeartLung,
    LiverKidney,
    GallStomach,
    SpleenKidney,
    RenDuBridge,
}

impl ScarCircuitKindWire {
    pub fn from_domain(kind: crate::combat::baomai_v4::scar_circuit::ScarCircuitKind) -> Self {
        use crate::combat::baomai_v4::scar_circuit::ScarCircuitKind;
        match kind {
            ScarCircuitKind::TigerMouth => Self::TigerMouth,
            ScarCircuitKind::TripleYang => Self::TripleYang,
            ScarCircuitKind::HeartLung => Self::HeartLung,
            ScarCircuitKind::LiverKidney => Self::LiverKidney,
            ScarCircuitKind::GallStomach => Self::GallStomach,
            ScarCircuitKind::SpleenKidney => Self::SpleenKidney,
            ScarCircuitKind::RenDuBridge => Self::RenDuBridge,
        }
    }
}

// ── CircuitBreakReason (wire repr) ──────────────────────────────────────────

/// 疤纹回路断裂原因 wire 字符串（与 TypeBox CircuitBreakReason 对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CircuitBreakReasonWire {
    Healed,
    Deepened,
    Severed,
    Closed,
}

impl CircuitBreakReasonWire {
    pub fn from_domain(reason: crate::combat::baomai_v4::events::CircuitBreakReason) -> Self {
        use crate::combat::baomai_v4::events::CircuitBreakReason;
        match reason {
            CircuitBreakReason::Healed => Self::Healed,
            CircuitBreakReason::Deepened => Self::Deepened,
            CircuitBreakReason::Severed => Self::Severed,
            CircuitBreakReason::Closed => Self::Closed,
        }
    }
}

// ── IronCocoonStage (wire repr) ─────────────────────────────────────────────

/// 活茧四阶 + None wire 字符串（与 TypeBox IronCocoonStage 对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IronCocoonStageWire {
    None,
    ToughSkin,
    IronBone,
    DullFlesh,
    ScarForged,
}

impl IronCocoonStageWire {
    pub fn from_domain(stage: crate::combat::baomai_v4::iron_cocoon::IronCocoonStage) -> Self {
        use crate::combat::baomai_v4::iron_cocoon::IronCocoonStage;
        match stage {
            IronCocoonStage::None => Self::None,
            IronCocoonStage::ToughSkin => Self::ToughSkin,
            IronCocoonStage::IronBone => Self::IronBone,
            IronCocoonStage::DullFlesh => Self::DullFlesh,
            IronCocoonStage::ScarForged => Self::ScarForged,
        }
    }
}

// ── LockEndReason (wire repr, tagged union) ─────────────────────────────────

/// 共振锁定结束原因 wire 格式（tagged union，与 TypeBox LockEndReason 对齐）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LockEndReasonWire {
    Expired,
    Retreated { entity_id: String },
    Death { entity_id: String },
}

// ── BaomaiV4ScarCircuitFormedV1 ─────────────────────────────────────────────

/// `bong:baomai_v4/scar_circuit_formed` 通道载荷。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaomaiV4ScarCircuitFormedV1 {
    pub v: u8,
    pub entity_id: String,
    pub circuit: ScarCircuitKindWire,
    pub tick: u64,
}

impl BaomaiV4ScarCircuitFormedV1 {
    pub fn new(
        entity_id: impl Into<String>,
        circuit: crate::combat::baomai_v4::scar_circuit::ScarCircuitKind,
        tick: u64,
    ) -> Self {
        Self {
            v: 1,
            entity_id: entity_id.into(),
            circuit: ScarCircuitKindWire::from_domain(circuit),
            tick,
        }
    }
}

// ── BaomaiV4ScarCircuitBrokenV1 ─────────────────────────────────────────────

/// `bong:baomai_v4/scar_circuit_broken` 通道载荷。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaomaiV4ScarCircuitBrokenV1 {
    pub v: u8,
    pub entity_id: String,
    pub circuit: ScarCircuitKindWire,
    pub reason: CircuitBreakReasonWire,
    pub tick: u64,
}

impl BaomaiV4ScarCircuitBrokenV1 {
    pub fn new(
        entity_id: impl Into<String>,
        circuit: crate::combat::baomai_v4::scar_circuit::ScarCircuitKind,
        reason: crate::combat::baomai_v4::events::CircuitBreakReason,
        tick: u64,
    ) -> Self {
        Self {
            v: 1,
            entity_id: entity_id.into(),
            circuit: ScarCircuitKindWire::from_domain(circuit),
            reason: CircuitBreakReasonWire::from_domain(reason),
            tick,
        }
    }
}

// ── BaomaiV4IronCocoonStageUpV1 ─────────────────────────────────────────────

/// `bong:baomai_v4/iron_cocoon_stage_up` 通道载荷。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaomaiV4IronCocoonStageUpV1 {
    pub v: u8,
    pub entity_id: String,
    pub from: IronCocoonStageWire,
    pub to: IronCocoonStageWire,
    pub total_overloads: u32,
    pub tick: u64,
}

impl BaomaiV4IronCocoonStageUpV1 {
    pub fn new(
        entity_id: impl Into<String>,
        from: crate::combat::baomai_v4::iron_cocoon::IronCocoonStage,
        to: crate::combat::baomai_v4::iron_cocoon::IronCocoonStage,
        total_overloads: u32,
        tick: u64,
    ) -> Self {
        Self {
            v: 1,
            entity_id: entity_id.into(),
            from: IronCocoonStageWire::from_domain(from),
            to: IronCocoonStageWire::from_domain(to),
            total_overloads,
            tick,
        }
    }
}

// ── BaomaiV4ResonanceLockV1 ─────────────────────────────────────────────────

/// `bong:baomai_v4/resonance_lock` 通道载荷。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaomaiV4ResonanceLockV1 {
    pub v: u8,
    pub fighter_a_id: String,
    pub fighter_b_id: String,
    pub started_at: u64,
    pub ends_at: u64,
}

impl BaomaiV4ResonanceLockV1 {
    pub fn new(
        fighter_a_id: impl Into<String>,
        fighter_b_id: impl Into<String>,
        started_at: u64,
        ends_at: u64,
    ) -> Self {
        Self {
            v: 1,
            fighter_a_id: fighter_a_id.into(),
            fighter_b_id: fighter_b_id.into(),
            started_at,
            ends_at,
        }
    }
}

// ── BaomaiV4ResonanceLockEndV1 ──────────────────────────────────────────────

/// `bong:baomai_v4/resonance_lock_end` 通道载荷。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaomaiV4ResonanceLockEndV1 {
    pub v: u8,
    pub fighter_a_id: String,
    pub fighter_b_id: String,
    pub reason: LockEndReasonWire,
    pub tick: u64,
}

impl BaomaiV4ResonanceLockEndV1 {
    pub fn new(
        fighter_a_id: impl Into<String>,
        fighter_b_id: impl Into<String>,
        reason: LockEndReasonWire,
        tick: u64,
    ) -> Self {
        Self {
            v: 1,
            fighter_a_id: fighter_a_id.into(),
            fighter_b_id: fighter_b_id.into(),
            reason,
            tick,
        }
    }
}

// ── helper ──────────────────────────────────────────────────────────────────

/// 把 LockEndReason domain event 类型转为 wire 格式。
pub fn lock_end_reason_wire(
    reason: crate::combat::baomai_v4::events::LockEndReason,
    entity_wire_resolver: impl Fn(valence::prelude::Entity) -> String,
) -> LockEndReasonWire {
    use crate::combat::baomai_v4::events::LockEndReason;
    match reason {
        LockEndReason::Expired => LockEndReasonWire::Expired,
        LockEndReason::Retreated(entity) => LockEndReasonWire::Retreated {
            entity_id: entity_wire_resolver(entity),
        },
        LockEndReason::Death(entity) => LockEndReasonWire::Death {
            entity_id: entity_wire_resolver(entity),
        },
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── sample files ─────────────────────────────────────────────────────────

    const SCAR_CIRCUIT_FORMED_SAMPLES: &str =
        include_str!("../../../agent/packages/schema/samples/baomai_v4_scar_circuit_formed.json");
    const SCAR_CIRCUIT_BROKEN_SAMPLES: &str =
        include_str!("../../../agent/packages/schema/samples/baomai_v4_scar_circuit_broken.json");
    const IRON_COCOON_STAGE_UP_SAMPLES: &str =
        include_str!("../../../agent/packages/schema/samples/baomai_v4_iron_cocoon_stage_up.json");
    const RESONANCE_LOCK_SAMPLES: &str =
        include_str!("../../../agent/packages/schema/samples/baomai_v4_resonance_lock.json");
    const RESONANCE_LOCK_END_SAMPLES: &str =
        include_str!("../../../agent/packages/schema/samples/baomai_v4_resonance_lock_end.json");

    // ── ScarCircuitFormed ─────────────────────────────────────────────────────

    #[test]
    fn scar_circuit_formed_all_samples_deserialize() {
        let samples: Vec<serde_json::Value> = serde_json::from_str(SCAR_CIRCUIT_FORMED_SAMPLES)
            .expect("SCAR_CIRCUIT_FORMED_SAMPLES should parse as JSON array");
        assert!(
            !samples.is_empty(),
            "sample file must contain at least 1 entry"
        );
        for (i, sample) in samples.iter().enumerate() {
            let result: Result<BaomaiV4ScarCircuitFormedV1, _> =
                serde_json::from_value(sample.clone());
            assert!(
                result.is_ok(),
                "scar_circuit_formed sample[{i}] should deserialize; error: {:?}",
                result.err()
            );
        }
    }

    #[test]
    fn scar_circuit_formed_tiger_mouth_wire() {
        let evt = BaomaiV4ScarCircuitFormedV1 {
            v: 1,
            entity_id: "offline:TestPlayer".to_string(),
            circuit: ScarCircuitKindWire::TigerMouth,
            tick: 100,
        };
        let json = serde_json::to_value(&evt).expect("serialize");
        assert_eq!(
            json["circuit"], "tiger_mouth",
            "TigerMouth must serialize to snake_case 'tiger_mouth'"
        );
        assert_eq!(json["v"], 1);
    }

    #[test]
    fn scar_circuit_formed_ren_du_bridge_wire() {
        let evt = BaomaiV4ScarCircuitFormedV1 {
            v: 1,
            entity_id: "char:123".to_string(),
            circuit: ScarCircuitKindWire::RenDuBridge,
            tick: 9999,
        };
        let json = serde_json::to_value(&evt).expect("serialize");
        assert_eq!(json["circuit"], "ren_du_bridge");
    }

    #[test]
    fn scar_circuit_formed_deny_unknown_fields() {
        let bad = serde_json::json!({
            "v": 1,
            "entity_id": "offline:TestPlayer",
            "circuit": "tiger_mouth",
            "tick": 100,
            "extra": "forbidden"
        });
        let result: Result<BaomaiV4ScarCircuitFormedV1, _> = serde_json::from_value(bad);
        assert!(
            result.is_err(),
            "deny_unknown_fields should reject extra fields"
        );
    }

    // ── ScarCircuitBroken ─────────────────────────────────────────────────────

    #[test]
    fn scar_circuit_broken_all_samples_deserialize() {
        let samples: Vec<serde_json::Value> =
            serde_json::from_str(SCAR_CIRCUIT_BROKEN_SAMPLES).expect("parse");
        assert!(!samples.is_empty());
        for (i, sample) in samples.iter().enumerate() {
            let result: Result<BaomaiV4ScarCircuitBrokenV1, _> =
                serde_json::from_value(sample.clone());
            assert!(
                result.is_ok(),
                "scar_circuit_broken sample[{i}] should deserialize; error: {:?}",
                result.err()
            );
        }
    }

    #[test]
    fn scar_circuit_broken_all_reason_variants() {
        let reasons = [
            ("healed", CircuitBreakReasonWire::Healed),
            ("deepened", CircuitBreakReasonWire::Deepened),
            ("severed", CircuitBreakReasonWire::Severed),
            ("closed", CircuitBreakReasonWire::Closed),
        ];
        for (wire_str, variant) in &reasons {
            let json = serde_json::json!({
                "v": 1,
                "entity_id": "offline:TestPlayer",
                "circuit": "heart_lung",
                "reason": wire_str,
                "tick": 100
            });
            let result: Result<BaomaiV4ScarCircuitBrokenV1, _> =
                serde_json::from_value(json.clone());
            assert!(
                result.is_ok(),
                "reason '{wire_str}' should deserialize; err: {:?}",
                result.err()
            );
            assert_eq!(
                result.unwrap().reason,
                *variant,
                "reason should map to {variant:?}"
            );
        }
    }

    // ── IronCocoonStageUp ─────────────────────────────────────────────────────

    #[test]
    fn iron_cocoon_stage_up_all_samples_deserialize() {
        let samples: Vec<serde_json::Value> =
            serde_json::from_str(IRON_COCOON_STAGE_UP_SAMPLES).expect("parse");
        assert!(!samples.is_empty());
        for (i, sample) in samples.iter().enumerate() {
            let result: Result<BaomaiV4IronCocoonStageUpV1, _> =
                serde_json::from_value(sample.clone());
            assert!(
                result.is_ok(),
                "iron_cocoon_stage_up sample[{i}] should deserialize; error: {:?}",
                result.err()
            );
        }
    }

    #[test]
    fn iron_cocoon_stage_up_all_stage_variants_wire() {
        let stages = [
            ("none", IronCocoonStageWire::None),
            ("tough_skin", IronCocoonStageWire::ToughSkin),
            ("iron_bone", IronCocoonStageWire::IronBone),
            ("dull_flesh", IronCocoonStageWire::DullFlesh),
            ("scar_forged", IronCocoonStageWire::ScarForged),
        ];
        for (wire_str, variant) in &stages {
            let json = serde_json::json!({
                "v": 1,
                "entity_id": "offline:TestPlayer",
                "from": "none",
                "to": wire_str,
                "total_overloads": 50,
                "tick": 100
            });
            let result: Result<BaomaiV4IronCocoonStageUpV1, _> =
                serde_json::from_value(json.clone());
            assert!(
                result.is_ok(),
                "to='{wire_str}' should deserialize; err: {:?}",
                result.err()
            );
            assert_eq!(result.unwrap().to, *variant);
        }
    }

    // ── ResonanceLock ─────────────────────────────────────────────────────────

    #[test]
    fn resonance_lock_all_samples_deserialize() {
        let samples: Vec<serde_json::Value> =
            serde_json::from_str(RESONANCE_LOCK_SAMPLES).expect("parse");
        assert!(!samples.is_empty());
        for (i, sample) in samples.iter().enumerate() {
            let result: Result<BaomaiV4ResonanceLockV1, _> = serde_json::from_value(sample.clone());
            assert!(
                result.is_ok(),
                "resonance_lock sample[{i}] should deserialize; error: {:?}",
                result.err()
            );
        }
    }

    #[test]
    fn resonance_lock_roundtrip() {
        let evt = BaomaiV4ResonanceLockV1::new("offline:PlayerA", "offline:PlayerB", 1000, 1060);
        let json = serde_json::to_string(&evt).expect("serialize");
        let back: BaomaiV4ResonanceLockV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            back, evt,
            "resonance_lock roundtrip must preserve all fields"
        );
    }

    // ── ResonanceLockEnd ──────────────────────────────────────────────────────

    #[test]
    fn resonance_lock_end_all_samples_deserialize() {
        let samples: Vec<serde_json::Value> =
            serde_json::from_str(RESONANCE_LOCK_END_SAMPLES).expect("parse");
        assert!(!samples.is_empty());
        for (i, sample) in samples.iter().enumerate() {
            let result: Result<BaomaiV4ResonanceLockEndV1, _> =
                serde_json::from_value(sample.clone());
            assert!(
                result.is_ok(),
                "resonance_lock_end sample[{i}] should deserialize; error: {:?}",
                result.err()
            );
        }
    }

    #[test]
    fn resonance_lock_end_expired_wire() {
        let json = serde_json::json!({
            "v": 1,
            "fighter_a_id": "offline:PlayerA",
            "fighter_b_id": "offline:PlayerB",
            "reason": { "type": "expired" },
            "tick": 1060
        });
        let evt: BaomaiV4ResonanceLockEndV1 =
            serde_json::from_value(json).expect("expired sample should deserialize");
        assert_eq!(
            evt.reason,
            LockEndReasonWire::Expired,
            "expired reason must map to LockEndReasonWire::Expired"
        );
    }

    #[test]
    fn resonance_lock_end_retreated_wire() {
        let json = serde_json::json!({
            "v": 1,
            "fighter_a_id": "offline:PlayerA",
            "fighter_b_id": "offline:PlayerB",
            "reason": { "type": "retreated", "entity_id": "offline:PlayerA" },
            "tick": 1030
        });
        let evt: BaomaiV4ResonanceLockEndV1 = serde_json::from_value(json).expect("parse");
        assert_eq!(
            evt.reason,
            LockEndReasonWire::Retreated {
                entity_id: "offline:PlayerA".to_string()
            }
        );
    }

    #[test]
    fn resonance_lock_end_death_wire() {
        let json = serde_json::json!({
            "v": 1,
            "fighter_a_id": "offline:PlayerA",
            "fighter_b_id": "offline:PlayerB",
            "reason": { "type": "death", "entity_id": "offline:PlayerB" },
            "tick": 1010
        });
        let evt: BaomaiV4ResonanceLockEndV1 = serde_json::from_value(json).expect("parse");
        assert_eq!(
            evt.reason,
            LockEndReasonWire::Death {
                entity_id: "offline:PlayerB".to_string()
            }
        );
    }

    #[test]
    fn resonance_lock_end_deny_unknown_fields() {
        let bad = serde_json::json!({
            "v": 1,
            "fighter_a_id": "offline:PlayerA",
            "fighter_b_id": "offline:PlayerB",
            "reason": { "type": "expired" },
            "tick": 1060,
            "extra": "forbidden"
        });
        let result: Result<BaomaiV4ResonanceLockEndV1, _> = serde_json::from_value(bad);
        assert!(
            result.is_err(),
            "deny_unknown_fields must reject extra fields"
        );
    }
}
