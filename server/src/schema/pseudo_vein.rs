//! plan-terrain-pseudo-vein-v1 §6.1 — 伪灵脉 IPC schema。
//!
//! 与 `agent/packages/schema/src/pseudo-vein.ts` TypeBox 定义对齐。

use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
