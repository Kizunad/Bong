use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeathInsightCategoryV1 {
    Combat,
    Cultivation,
    Natural,
    Tribulation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeathInsightZoneKindV1 {
    Ordinary,
    Death,
    Negative,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DeathInsightPositionV1 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeathInsightRequestV1 {
    pub v: u8,
    pub request_id: String,
    pub character_id: String,
    pub at_tick: u64,
    pub cause: String,
    pub category: DeathInsightCategoryV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player_realm: Option<String>,
    pub zone_kind: DeathInsightZoneKindV1,
    pub death_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rebirth_chance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifespan_remaining_years: Option<f64>,
    pub recent_biography: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<DeathInsightPositionV1>,
    pub context: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn death_insight_request_serializes_snake_case_contract() {
        let payload = DeathInsightRequestV1 {
            v: 1,
            request_id: "death_insight:offline:Azure:84000:3".to_string(),
            character_id: "offline:Azure".to_string(),
            at_tick: 84_000,
            cause: "cultivation:NaturalAging".to_string(),
            category: DeathInsightCategoryV1::Natural,
            realm: Some("Condense".to_string()),
            player_realm: Some("qi_refining_6".to_string()),
            zone_kind: DeathInsightZoneKindV1::Ordinary,
            death_count: 3,
            rebirth_chance: None,
            lifespan_remaining_years: Some(0.0),
            recent_biography: vec!["t83980:near_death:cultivation:NaturalAging".to_string()],
            position: Some(DeathInsightPositionV1 {
                x: 8.0,
                y: 150.0,
                z: 8.0,
            }),
            context: serde_json::json!({"will_terminate": true}),
        };

        let json = serde_json::to_value(&payload).expect("payload should serialize");
        assert_eq!(json["category"], "natural");
        assert_eq!(json["zone_kind"], "ordinary");
        assert_eq!(json["lifespan_remaining_years"], 0.0);
        assert!(json.get("rebirth_chance").is_none());

        let roundtrip: DeathInsightRequestV1 =
            serde_json::from_value(json).expect("payload should deserialize");
        assert_eq!(roundtrip.death_count, 3);
        assert_eq!(roundtrip.category, DeathInsightCategoryV1::Natural);
    }
}
