use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NpcSpawnedV1 {
    pub v: u8,
    pub kind: String,
    pub npc_id: String,
    pub archetype: String,
    pub source: String,
    pub zone: String,
    pub pos: [f64; 3],
    pub initial_age_ticks: f64,
    pub at_tick: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NpcDeathV1 {
    pub v: u8,
    pub kind: String,
    pub npc_id: String,
    pub archetype: String,
    pub cause: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub faction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub life_record_snapshot: Option<String>,
    pub age_ticks: f64,
    pub max_age_ticks: f64,
    pub at_tick: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FactionEventV1 {
    pub v: u8,
    pub kind: String,
    pub faction_id: String,
    pub event_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leader_id: Option<String>,
    pub loyalty_bias: f64,
    pub mission_queue_size: u32,
    pub at_tick: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npc_death_omits_absent_optional_fields() {
        let payload = NpcDeathV1 {
            v: 1,
            kind: "npc_death".to_string(),
            npc_id: "npc_1v1".to_string(),
            archetype: "commoner".to_string(),
            cause: "natural_aging".to_string(),
            faction_id: None,
            life_record_snapshot: None,
            age_ticks: 10.0,
            max_age_ticks: 20.0,
            at_tick: 3,
        };

        let value = serde_json::to_value(payload).expect("serialize");
        assert!(value.get("faction_id").is_none());
        assert!(value.get("life_record_snapshot").is_none());
    }
}
