use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DuguPoisonStateV1 {
    pub target: String,
    pub active: bool,
    pub meridian_id: String,
    pub attacker: String,
    pub attached_at_tick: u64,
    pub poisoner_realm_tier: u8,
    pub loss_per_tick: f64,
    pub flow_capacity_after: f64,
    pub qi_max_after: f64,
    pub server_tick: u64,
}

impl DuguPoisonStateV1 {
    pub fn clear(target: String, server_tick: u64) -> Self {
        Self {
            target,
            active: false,
            meridian_id: String::new(),
            attacker: String::new(),
            attached_at_tick: 0,
            poisoner_realm_tier: 0,
            loss_per_tick: 0.0,
            flow_capacity_after: 0.0,
            qi_max_after: 0.0,
            server_tick,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DuguPoisonProgressEventV1 {
    pub target: String,
    pub attacker: String,
    pub meridian_id: String,
    pub flow_capacity_after: f64,
    pub qi_max_after: f64,
    pub actual_loss_this_tick: f64,
    pub tick: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DuguObfuscationStateV1 {
    pub entity: String,
    pub active: bool,
    pub disrupted_until_tick: Option<u64>,
    pub server_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AntidoteResultV1 {
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AntidoteResultEventV1 {
    pub healer: String,
    pub target: String,
    pub result: AntidoteResultV1,
    pub meridian_id: String,
    pub qi_max_after: f64,
    pub tick: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DuguRevealedEventV1 {
    pub revealed_player: String,
    pub witness: String,
    pub witness_realm: String,
    pub at_position: [f64; 3],
    pub at_tick: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_poison_state_keeps_target_and_tick() {
        let state = DuguPoisonStateV1::clear("player:alice".to_string(), 88);
        assert_eq!(state.target, "player:alice");
        assert_eq!(state.server_tick, 88);
        assert!(!state.active);
        assert!(state.meridian_id.is_empty());
    }

    #[test]
    fn progress_roundtrip_uses_snake_case_antidote_result() {
        let event = AntidoteResultEventV1 {
            healer: "player:a".to_string(),
            target: "player:a".to_string(),
            result: AntidoteResultV1::Failed,
            meridian_id: "Heart".to_string(),
            qi_max_after: 10.0,
            tick: 7,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"failed\""));
        let back: AntidoteResultEventV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, event);
    }
}
