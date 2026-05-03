use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FalseSkinKindV1 {
    SpiderSilk,
    RottenWoodArmor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FalseSkinStateV1 {
    pub target_id: String,
    pub kind: Option<FalseSkinKindV1>,
    pub layers_remaining: u8,
    pub contam_capacity_per_layer: f64,
    pub absorbed_contam: f64,
    pub equipped_at_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShedEventV1 {
    pub target_id: String,
    pub attacker_id: Option<String>,
    pub kind: FalseSkinKindV1,
    pub layers_shed: u8,
    pub layers_remaining: u8,
    pub contam_absorbed: f64,
    pub contam_overflow: f64,
    pub tick: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn false_skin_state_roundtrip_preserves_empty_state() {
        let original = FalseSkinStateV1 {
            target_id: "offline:Azure".to_string(),
            kind: None,
            layers_remaining: 0,
            contam_capacity_per_layer: 0.0,
            absorbed_contam: 0.0,
            equipped_at_tick: 0,
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: FalseSkinStateV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, original);
    }

    #[test]
    fn shed_event_roundtrip_preserves_overflow() {
        let original = ShedEventV1 {
            target_id: "npc:1".to_string(),
            attacker_id: Some("offline:Azure".to_string()),
            kind: FalseSkinKindV1::RottenWoodArmor,
            layers_shed: 3,
            layers_remaining: 0,
            contam_absorbed: 90.0,
            contam_overflow: 12.5,
            tick: 42,
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: ShedEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, original);
    }
}
