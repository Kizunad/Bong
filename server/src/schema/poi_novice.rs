//! plan-poi-novice-v1 — server 侧 POI novice IPC wire serde 结构。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PoiNoviceKindV1 {
    ForgeStation,
    AlchemyFurnace,
    RogueVillage,
    MutantNest,
    ScrollHidden,
    SpiritHerbValley,
    HerbPatch,
    QiSpring,
    TradeSpot,
    ShelterSpot,
    WaterSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PoiSpawnedEventV1 {
    pub v: u8,
    pub kind: String,
    pub poi_id: String,
    pub poi_type: PoiNoviceKindV1,
    pub zone: String,
    pub pos: [f64; 3],
    pub selection_strategy: String,
    pub qi_affinity: f32,
    pub danger_bias: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TrespassEventV1 {
    pub v: u8,
    pub kind: String,
    pub village_id: String,
    pub player_id: String,
    pub killed_npc_count: u32,
    pub refusal_until_wall_clock_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poi_spawned_sample_roundtrips() {
        let json =
            include_str!("../../../agent/packages/schema/samples/poi-spawned-event.sample.json");
        let event: PoiSpawnedEventV1 =
            serde_json::from_str(json).expect("poi spawned sample should deserialize");
        assert_eq!(event.v, 1);
        assert_eq!(event.kind, "poi_spawned");
        assert_eq!(event.poi_type, PoiNoviceKindV1::ForgeStation);
        let back = serde_json::to_string(&event).expect("serialize");
        let parsed: PoiSpawnedEventV1 = serde_json::from_str(&back).expect("deserialize");
        assert_eq!(parsed, event);
    }

    #[test]
    fn trespass_sample_roundtrips() {
        let json =
            include_str!("../../../agent/packages/schema/samples/trespass-event.sample.json");
        let event: TrespassEventV1 =
            serde_json::from_str(json).expect("trespass sample should deserialize");
        assert_eq!(event.v, 1);
        assert_eq!(event.kind, "trespass");
        assert_eq!(event.village_id, "spawn:rogue_village");
        let back = serde_json::to_string(&event).expect("serialize");
        let parsed: TrespassEventV1 = serde_json::from_str(&back).expect("deserialize");
        assert_eq!(parsed, event);
    }
}
