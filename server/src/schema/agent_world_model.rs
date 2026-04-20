use std::collections::{BTreeMap, HashMap};

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

use super::agent_command::Command;
use super::narration::Narration;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentWorldModelDecisionV1 {
    pub commands: Vec<Command>,
    pub narrations: Vec<Narration>,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentWorldModelSnapshotV1 {
    pub current_era: Option<CurrentEraV1>,
    #[serde(default)]
    pub zone_history: HashMap<String, Vec<ZoneHistoryEntryV1>>,
    #[serde(default)]
    pub last_decisions: BTreeMap<String, AgentWorldModelDecisionV1>,
    #[serde(default)]
    pub player_first_seen_tick: BTreeMap<String, i64>,
    pub last_tick: Option<i64>,
    pub last_state_ts: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CurrentEraV1 {
    pub name: String,
    pub since_tick: i64,
    pub global_effect: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ZoneHistoryEntryV1 {
    pub name: String,
    pub spirit_qi: f64,
    pub danger_level: i64,
    pub active_events: Vec<String>,
    pub player_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentWorldModelEnvelopeV1 {
    #[serde(deserialize_with = "deserialize_v1_version")]
    pub v: u8,
    pub id: String,
    #[serde(default, deserialize_with = "deserialize_source")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub snapshot: AgentWorldModelSnapshotV1,
}

fn deserialize_v1_version<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let version = u8::deserialize(deserializer)?;
    if version == 1 {
        Ok(version)
    } else {
        Err(D::Error::custom(format!(
            "AgentWorldModelEnvelopeV1.v must be 1, got {version}"
        )))
    }
}

fn deserialize_source<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let source = Option::<String>::deserialize(deserializer)?;
    if let Some(source_value) = source.as_deref() {
        if !matches!(source_value, "arbiter" | "calamity" | "mutation" | "era") {
            return Err(D::Error::custom(format!(
                "AgentWorldModelEnvelopeV1.source has unsupported value `{source_value}`"
            )));
        }
    }
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_agent_world_model_version() {
        let json = r#"{
            "v": 2,
            "id": "wm-1",
            "source": "arbiter",
            "snapshot": {
                "current_era": null,
                "zone_history": {},
                "last_decisions": {},
                "player_first_seen_tick": {},
                "last_tick": null,
                "last_state_ts": null
            }
        }"#;

        let error = serde_json::from_str::<AgentWorldModelEnvelopeV1>(json)
            .expect_err("unknown agent world model version should be rejected");

        assert!(
            error
                .to_string()
                .contains("AgentWorldModelEnvelopeV1.v must be 1"),
            "unexpected agent world model version error: {error}"
        );
    }
}
