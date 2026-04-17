use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BotanyHarvestModeV1 {
    Manual,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BotanyHarvestPhaseV1 {
    Pending,
    InProgress,
    Completed,
    Interrupted,
    Trampled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BotanyHarvestProgressV1 {
    pub session_id: String,
    pub target_id: String,
    pub target_name: String,
    pub plant_kind: String,
    pub mode: BotanyHarvestModeV1,
    pub progress: f64,
    pub auto_selectable: bool,
    pub request_pending: bool,
    pub interrupted: bool,
    pub completed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BotanySkillV1 {
    pub level: u64,
    pub xp: u64,
    pub xp_to_next_level: u64,
    pub auto_unlock_level: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BotanyHarvestRequestV1 {
    pub session_id: String,
    pub mode: BotanyHarvestModeV1,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn botany_harvest_progress_roundtrip() {
        let payload = BotanyHarvestProgressV1 {
            session_id: "session-botany-01".to_string(),
            target_id: "plant-1".to_string(),
            target_name: "开脉草".to_string(),
            plant_kind: "ning_mai_cao".to_string(),
            mode: BotanyHarvestModeV1::Manual,
            progress: 0.5,
            auto_selectable: true,
            request_pending: false,
            interrupted: false,
            completed: false,
            detail: "晨露未散".to_string(),
        };

        let json = serde_json::to_string(&payload).unwrap();
        let back: BotanyHarvestProgressV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, payload);
    }

    #[test]
    fn botany_skill_roundtrip() {
        let payload = BotanySkillV1 {
            level: 3,
            xp: 240,
            xp_to_next_level: 400,
            auto_unlock_level: 3,
        };

        let json = serde_json::to_string(&payload).unwrap();
        let back: BotanySkillV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, payload);
    }
}
