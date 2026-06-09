//! plan-tiandao-hunt-v1 P3 — 天道狩猎专属叙事请求 IPC schema。
//!
//! 与 `agent/packages/schema/src/tiandao-hunt-narration.ts` 1:1 对齐。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TiandaoHuntResponseLevelV1 {
    Watch,
    Pressure,
    Tribulation,
    Annihilate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TiandaoHuntNarrationRequestV1 {
    pub v: u8,
    pub character_id: String,
    pub realm: String,
    pub attention_level: f64,
    pub response_level: TiandaoHuntResponseLevelV1,
    pub zone: String,
    pub recent_actions: Vec<String>,
    pub narration_count: u32,
}

impl TiandaoHuntNarrationRequestV1 {
    pub fn new(
        character_id: impl Into<String>,
        realm: impl Into<String>,
        attention_level: f64,
        response_level: TiandaoHuntResponseLevelV1,
        zone: impl Into<String>,
        recent_actions: Vec<String>,
        narration_count: u32,
    ) -> Self {
        Self {
            v: 1,
            character_id: character_id.into(),
            realm: realm.into(),
            attention_level,
            response_level,
            zone: zone.into(),
            recent_actions,
            narration_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_wire_shape() {
        let payload = TiandaoHuntNarrationRequestV1::new(
            "offline:Alice",
            "Spirit",
            72.5,
            TiandaoHuntResponseLevelV1::Tribulation,
            "血谷",
            vec!["activity:meditating".to_string()],
            2,
        );

        let wire = serde_json::to_value(payload).expect("payload should serialize");

        assert_eq!(
            wire,
            json!({
                "v": 1,
                "character_id": "offline:Alice",
                "realm": "Spirit",
                "attention_level": 72.5,
                "response_level": "tribulation",
                "zone": "血谷",
                "recent_actions": ["activity:meditating"],
                "narration_count": 2
            })
        );
    }

    #[test]
    fn rejects_unknown_response_level() {
        let error = serde_json::from_value::<TiandaoHuntNarrationRequestV1>(json!({
            "v": 1,
            "character_id": "offline:Alice",
            "realm": "Spirit",
            "attention_level": 72.5,
            "response_level": "none",
            "zone": "血谷",
            "recent_actions": [],
            "narration_count": 0
        }))
        .expect_err("none must not deserialize as narration response");

        assert!(
            error.to_string().contains("unknown variant"),
            "expected unknown variant error, got {error}"
        );
    }
}
