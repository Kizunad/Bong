//! plan-terrain-pseudo-vein-v1 §6.1 — 伪灵脉 IPC schema。
//!
//! 与 `agent/packages/schema/src/pseudo-vein.ts` TypeBox 定义对齐。

use serde::{Deserialize, Serialize};

const PSEUDO_VEIN_VERSION: u8 = 1;
const PSEUDO_VEIN_UNIT_MIN: f64 = 0.0;
const PSEUDO_VEIN_UNIT_MAX: f64 = 1.0;
pub(crate) const PSEUDO_VEIN_STORM_ANCHORS_MIN: usize = 1;
pub(crate) const PSEUDO_VEIN_STORM_ANCHORS_MAX: usize = 3;
pub(crate) const PSEUDO_VEIN_STORM_DURATION_TICKS_MIN: u64 = 6000;
pub(crate) const PSEUDO_VEIN_STORM_DURATION_TICKS_MAX: u64 = 12000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PseudoVeinSeasonV1 {
    Summer,
    SummerToWinter,
    Winter,
    WinterToSummer,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PseudoVeinSnapshotV1 {
    pub v: u8,
    pub id: String,
    pub center_xz: [f64; 2],
    pub spirit_qi_current: f64,
    pub occupants: Vec<String>,
    pub spawned_at_tick: u64,
    pub estimated_decay_at_tick: u64,
    pub season_at_spawn: PseudoVeinSeasonV1,
}

impl PseudoVeinSnapshotV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_version("PseudoVeinSnapshotV1", self.v)?;
        validate_non_empty("PseudoVeinSnapshotV1", "id", &self.id)?;
        validate_xz_pair("PseudoVeinSnapshotV1", "center_xz", self.center_xz)?;
        validate_unit(
            "PseudoVeinSnapshotV1",
            "spirit_qi_current",
            self.spirit_qi_current,
        )?;

        for (index, occupant) in self.occupants.iter().enumerate() {
            if occupant.is_empty() {
                return Err(format!(
                    "PseudoVeinSnapshotV1.occupants[{index}] must be non-empty"
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PseudoVeinQiRedistributionV1 {
    pub refill_to_hungry_ring: f64,
    pub collected_by_tiandao: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PseudoVeinDissipateEventV1 {
    pub v: u8,
    pub id: String,
    pub center_xz: [f64; 2],
    pub storm_anchors: Vec<[f64; 2]>,
    pub storm_duration_ticks: u64,
    pub qi_redistribution: PseudoVeinQiRedistributionV1,
}

impl PseudoVeinDissipateEventV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_version("PseudoVeinDissipateEventV1", self.v)?;
        validate_non_empty("PseudoVeinDissipateEventV1", "id", &self.id)?;
        validate_xz_pair("PseudoVeinDissipateEventV1", "center_xz", self.center_xz)?;

        let anchor_count = self.storm_anchors.len();
        if !(PSEUDO_VEIN_STORM_ANCHORS_MIN..=PSEUDO_VEIN_STORM_ANCHORS_MAX).contains(&anchor_count)
        {
            return Err(format!(
                "PseudoVeinDissipateEventV1.storm_anchors must contain {PSEUDO_VEIN_STORM_ANCHORS_MIN}..={PSEUDO_VEIN_STORM_ANCHORS_MAX} anchors, got {anchor_count}"
            ));
        }
        for (index, anchor) in self.storm_anchors.iter().copied().enumerate() {
            validate_xz_pair(
                "PseudoVeinDissipateEventV1",
                &format!("storm_anchors[{index}]"),
                anchor,
            )?;
        }

        if !(PSEUDO_VEIN_STORM_DURATION_TICKS_MIN..=PSEUDO_VEIN_STORM_DURATION_TICKS_MAX)
            .contains(&self.storm_duration_ticks)
        {
            return Err(format!(
                "PseudoVeinDissipateEventV1.storm_duration_ticks must be {PSEUDO_VEIN_STORM_DURATION_TICKS_MIN}..={PSEUDO_VEIN_STORM_DURATION_TICKS_MAX}, got {}",
                self.storm_duration_ticks
            ));
        }
        validate_unit(
            "PseudoVeinDissipateEventV1",
            "qi_redistribution.refill_to_hungry_ring",
            self.qi_redistribution.refill_to_hungry_ring,
        )?;
        validate_unit(
            "PseudoVeinDissipateEventV1",
            "qi_redistribution.collected_by_tiandao",
            self.qi_redistribution.collected_by_tiandao,
        )?;

        Ok(())
    }
}

fn validate_version(context: &str, value: u8) -> Result<(), String> {
    if value != PSEUDO_VEIN_VERSION {
        return Err(format!(
            "{context}.v must be {PSEUDO_VEIN_VERSION}, got {value}"
        ));
    }

    Ok(())
}

fn validate_non_empty(context: &str, field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{context}.{field} must be non-empty"));
    }

    Ok(())
}

fn validate_xz_pair(context: &str, field: &str, value: [f64; 2]) -> Result<(), String> {
    if !value.iter().all(|coordinate| coordinate.is_finite()) {
        return Err(format!("{context}.{field} values must be finite"));
    }

    Ok(())
}

fn validate_unit(context: &str, field: &str, value: f64) -> Result<(), String> {
    if !(value.is_finite() && (PSEUDO_VEIN_UNIT_MIN..=PSEUDO_VEIN_UNIT_MAX).contains(&value)) {
        return Err(format!(
            "{context}.{field} must be finite and {PSEUDO_VEIN_UNIT_MIN}..={PSEUDO_VEIN_UNIT_MAX}, got {value}"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_pseudo_vein_snapshot() -> PseudoVeinSnapshotV1 {
        PseudoVeinSnapshotV1 {
            v: 1,
            id: "pseudo_vein_42".to_string(),
            center_xz: [1280.0, -640.0],
            spirit_qi_current: 0.6,
            occupants: vec!["offline:Azure".to_string(), "offline:Crimson".to_string()],
            spawned_at_tick: 24000,
            estimated_decay_at_tick: 60000,
            season_at_spawn: PseudoVeinSeasonV1::SummerToWinter,
        }
    }

    fn valid_pseudo_vein_dissipate_event() -> PseudoVeinDissipateEventV1 {
        PseudoVeinDissipateEventV1 {
            v: 1,
            id: "pseudo_vein_42".to_string(),
            center_xz: [1280.0, -640.0],
            storm_anchors: vec![[1380.0, -650.0], [1160.0, -720.0]],
            storm_duration_ticks: 9000,
            qi_redistribution: PseudoVeinQiRedistributionV1 {
                refill_to_hungry_ring: 0.7,
                collected_by_tiandao: 0.3,
            },
        }
    }

    fn assert_snapshot_invalid(label: &str, mutate: impl FnOnce(&mut PseudoVeinSnapshotV1)) {
        let mut snapshot = valid_pseudo_vein_snapshot();
        mutate(&mut snapshot);

        assert!(
            snapshot.validate().is_err(),
            "{label} should violate TypeScript PseudoVeinSnapshotV1 contract"
        );
    }

    fn assert_dissipate_invalid(label: &str, mutate: impl FnOnce(&mut PseudoVeinDissipateEventV1)) {
        let mut event = valid_pseudo_vein_dissipate_event();
        mutate(&mut event);

        assert!(
            event.validate().is_err(),
            "{label} should violate TypeScript PseudoVeinDissipateEventV1 contract"
        );
    }

    #[test]
    fn deserialize_pseudo_vein_snapshot_sample() {
        let json =
            include_str!("../../../agent/packages/schema/samples/pseudo-vein-snapshot.sample.json");
        let snapshot: PseudoVeinSnapshotV1 =
            serde_json::from_str(json).expect("pseudo vein snapshot sample should deserialize");

        assert_eq!(snapshot.v, 1);
        assert_eq!(snapshot.id, "pseudo_vein_42");
        assert_eq!(snapshot.center_xz, [1280.0, -640.0]);
        assert_eq!(snapshot.spirit_qi_current, 0.6);
        assert_eq!(snapshot.occupants.len(), 2);
        assert_eq!(snapshot.season_at_spawn, PseudoVeinSeasonV1::SummerToWinter);
        assert!(snapshot.validate().is_ok(), "sample should pass validate()");
    }

    #[test]
    fn deserialize_pseudo_vein_dissipate_event_sample() {
        let json = include_str!(
            "../../../agent/packages/schema/samples/pseudo-vein-dissipate-event.sample.json"
        );
        let event: PseudoVeinDissipateEventV1 =
            serde_json::from_str(json).expect("pseudo vein dissipate sample should deserialize");

        assert_eq!(event.v, 1);
        assert_eq!(event.id, "pseudo_vein_42");
        assert_eq!(event.storm_anchors.len(), 2);
        assert_eq!(event.storm_duration_ticks, 9000);
        assert_eq!(event.qi_redistribution.refill_to_hungry_ring, 0.7);
        assert_eq!(event.qi_redistribution.collected_by_tiandao, 0.3);
        assert!(event.validate().is_ok(), "sample should pass validate()");
    }

    #[test]
    fn accepts_pseudo_vein_snapshot_spirit_qi_boundaries() {
        for spirit_qi_current in [PSEUDO_VEIN_UNIT_MIN, PSEUDO_VEIN_UNIT_MAX] {
            let mut snapshot = valid_pseudo_vein_snapshot();
            snapshot.spirit_qi_current = spirit_qi_current;

            snapshot
                .validate()
                .expect("TypeScript PseudoVeinSnapshotV1 spirit_qi_current boundaries should pass");
        }
    }

    #[test]
    fn rejects_pseudo_vein_snapshot_contract_violations() {
        assert_snapshot_invalid("version", |snapshot| snapshot.v = 2);
        assert_snapshot_invalid("id", |snapshot| snapshot.id.clear());
        assert_snapshot_invalid("center_xz", |snapshot| snapshot.center_xz[0] = f64::NAN);
        assert_snapshot_invalid("spirit_qi_current below", |snapshot| {
            snapshot.spirit_qi_current = -0.1;
        });
        assert_snapshot_invalid("spirit_qi_current above", |snapshot| {
            snapshot.spirit_qi_current = 1.1;
        });
        assert_snapshot_invalid("spirit_qi_current inf", |snapshot| {
            snapshot.spirit_qi_current = f64::INFINITY;
        });
        assert_snapshot_invalid("occupant", |snapshot| snapshot.occupants[0].clear());
    }

    #[test]
    fn rejects_pseudo_vein_dissipate_contract_violations() {
        assert_dissipate_invalid("version", |event| event.v = 2);
        assert_dissipate_invalid("id", |event| event.id.clear());
        assert_dissipate_invalid("center_xz", |event| event.center_xz[0] = f64::NAN);
        assert_dissipate_invalid("storm_anchor", |event| event.storm_anchors[0][0] = f64::NAN);
        assert_dissipate_invalid("refill_to_hungry_ring below", |event| {
            event.qi_redistribution.refill_to_hungry_ring = -0.1;
        });
        assert_dissipate_invalid("refill_to_hungry_ring above", |event| {
            event.qi_redistribution.refill_to_hungry_ring = 1.1;
        });
        assert_dissipate_invalid("collected_by_tiandao below", |event| {
            event.qi_redistribution.collected_by_tiandao = -0.1;
        });
        assert_dissipate_invalid("collected_by_tiandao above", |event| {
            event.qi_redistribution.collected_by_tiandao = 1.1;
        });
    }
}
