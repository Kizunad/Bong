//! plan-tsy-hostile-v1 §6 — server 侧 TSY hostile IPC wire serde 结构。
//!
//! 与 `agent/packages/schema/src/tsy-hostile-v1.ts` TypeBox 定义双端对齐。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TsyHostileArchetypeV1 {
    Daoxiang,
    Zhinian,
    GuardianRelicSentinel,
    Fuya,
    SkullFiend,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TsyNpcSpawnedV1 {
    pub v: u8,
    pub kind: String,
    pub family_id: String,
    pub archetype: TsyHostileArchetypeV1,
    pub count: u32,
    pub at_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TsySentinelPhaseChangedV1 {
    pub v: u8,
    pub kind: String,
    pub family_id: String,
    pub container_entity_id: u64,
    pub phase: u8,
    pub max_phase: u8,
    pub at_tick: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_tsy_npc_spawned_sample() {
        let json =
            include_str!("../../../agent/packages/schema/samples/tsy-npc-spawned.sample.json");
        let ev: TsyNpcSpawnedV1 = serde_json::from_str(json)
            .expect("tsy-npc-spawned.sample.json should deserialize into TsyNpcSpawnedV1");
        assert_eq!(ev.v, 1);
        assert_eq!(ev.kind, "tsy_npc_spawned");
        assert_eq!(ev.family_id, "tsy_zongmen_yiji_01");
        assert_eq!(ev.archetype, TsyHostileArchetypeV1::GuardianRelicSentinel);
        assert_eq!(ev.count, 3);
        assert_eq!(ev.at_tick, 12000);
    }

    #[test]
    fn deserialize_tsy_sentinel_phase_changed_sample() {
        let json = include_str!(
            "../../../agent/packages/schema/samples/tsy-sentinel-phase-changed.sample.json"
        );
        let ev: TsySentinelPhaseChangedV1 = serde_json::from_str(json).expect(
            "tsy-sentinel-phase-changed.sample.json should deserialize into TsySentinelPhaseChangedV1",
        );
        assert_eq!(ev.v, 1);
        assert_eq!(ev.kind, "tsy_sentinel_phase_changed");
        assert_eq!(ev.container_entity_id, 42);
        assert_eq!(ev.phase, 1);
        assert_eq!(ev.max_phase, 3);
        assert_eq!(ev.at_tick, 12345);
    }

    #[test]
    fn round_trip_tsy_npc_spawned_event_through_serde() {
        let ev = TsyNpcSpawnedV1 {
            v: 1,
            kind: "tsy_npc_spawned".to_string(),
            family_id: "tsy_zongmen_yiji_01".to_string(),
            archetype: TsyHostileArchetypeV1::GuardianRelicSentinel,
            count: 3,
            at_tick: 12000,
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        assert!(json.contains("guardian_relic_sentinel"));
        assert!(serde_json::to_string(&TsyHostileArchetypeV1::SkullFiend)
            .expect("serialize skull fiend")
            .contains("skull_fiend"));
        let parsed: TsyNpcSpawnedV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, ev);
    }

    #[test]
    fn round_trip_tsy_sentinel_phase_changed_event_through_serde() {
        let ev = TsySentinelPhaseChangedV1 {
            v: 1,
            kind: "tsy_sentinel_phase_changed".to_string(),
            family_id: "tsy_zongmen_yiji_01".to_string(),
            container_entity_id: 42,
            phase: 1,
            max_phase: 3,
            at_tick: 12345,
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        let parsed: TsySentinelPhaseChangedV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, ev);
    }
}
