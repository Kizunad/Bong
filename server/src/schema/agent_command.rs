use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

use super::common::{CommandType, MAX_COMMANDS_PER_TICK};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    #[serde(rename = "type")]
    pub command_type: CommandType,
    pub target: String,
    pub params: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCommandV1 {
    #[serde(deserialize_with = "deserialize_v1_version")]
    pub v: u8,
    pub id: String,
    #[serde(default, deserialize_with = "deserialize_command_source")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(deserialize_with = "deserialize_commands")]
    pub commands: Vec<Command>,
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
            "AgentCommandV1.v must be 1, got {version}"
        )))
    }
}

fn deserialize_command_source<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let source = Option::<String>::deserialize(deserializer)?;
    if let Some(source_value) = source.as_deref() {
        if !matches!(source_value, "arbiter" | "calamity" | "mutation" | "era") {
            return Err(D::Error::custom(format!(
                "AgentCommandV1.source has unsupported value `{source_value}`"
            )));
        }
    }

    Ok(source)
}

fn deserialize_commands<'de, D>(deserializer: D) -> Result<Vec<Command>, D::Error>
where
    D: Deserializer<'de>,
{
    let commands = Vec::<Command>::deserialize(deserializer)?;
    if commands.len() > MAX_COMMANDS_PER_TICK {
        return Err(D::Error::custom(format!(
            "AgentCommandV1.commands exceeds maxItems {MAX_COMMANDS_PER_TICK}"
        )));
    }

    Ok(commands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::channels::CH_AGENT_COMMAND;
    use serde_json::{json, Value};

    fn sample_agent_command_value() -> Value {
        serde_json::from_str(include_str!(
            "../../../agent/packages/schema/samples/agent-command.sample.json"
        ))
        .expect("agent-command sample should parse into JSON value")
    }

    #[test]
    fn deserialize_agent_command_sample() {
        let json = include_str!("../../../agent/packages/schema/samples/agent-command.sample.json");
        let cmd: AgentCommandV1 = serde_json::from_str(json)
            .expect("agent-command.sample.json should deserialize into AgentCommandV1");

        assert_eq!(cmd.v, 1);
        assert_eq!(cmd.id, "cmd_1712345678_001");
        assert_eq!(cmd.source.as_deref(), Some("calamity"));
        assert_eq!(cmd.commands.len(), 2);
        assert_eq!(cmd.commands[0].command_type, CommandType::SpawnEvent);
        assert_eq!(cmd.commands[0].target, "blood_valley");
        assert_eq!(cmd.commands[1].command_type, CommandType::ModifyZone);
        assert_eq!(CH_AGENT_COMMAND, "bong:agent_command");
    }

    #[test]
    fn roundtrip_agent_command() {
        let json = include_str!("../../../agent/packages/schema/samples/agent-command.sample.json");
        let cmd: AgentCommandV1 = serde_json::from_str(json).unwrap();
        let re_json = serde_json::to_string(&cmd).unwrap();
        let cmd2: AgentCommandV1 = serde_json::from_str(&re_json).unwrap();
        assert_eq!(cmd.id, cmd2.id);
        assert_eq!(cmd.commands.len(), cmd2.commands.len());
    }

    #[test]
    fn deserialize_agent_command_sample_rejects_wrong_version() {
        let mut value = sample_agent_command_value();
        value["v"] = json!(2);

        assert!(serde_json::from_value::<AgentCommandV1>(value).is_err());
    }

    #[test]
    fn deserialize_agent_command_sample_rejects_unknown_top_level_field() {
        let mut value = sample_agent_command_value();
        value["retry_after_ms"] = json!(500);

        assert!(serde_json::from_value::<AgentCommandV1>(value).is_err());
    }

    #[test]
    fn deserialize_agent_command_sample_rejects_unknown_nested_field() {
        let mut value = sample_agent_command_value();
        value["commands"][0]["priority"] = json!("urgent");

        assert!(serde_json::from_value::<AgentCommandV1>(value).is_err());
    }

    #[test]
    fn deserialize_agent_command_sample_rejects_too_many_commands() {
        let mut value = sample_agent_command_value();
        let commands = value["commands"]
            .as_array()
            .cloned()
            .expect("commands should be an array");

        value["commands"] = Value::Array([commands.clone(), commands.clone(), commands].concat());

        assert!(serde_json::from_value::<AgentCommandV1>(value).is_err());
    }

    #[test]
    fn deserialize_agent_command_sample_rejects_invalid_type() {
        let mut value = sample_agent_command_value();
        value["commands"][0]["type"] = json!("delete_world");

        assert!(serde_json::from_value::<AgentCommandV1>(value).is_err());
    }

    #[test]
    fn deserialize_agent_command_sample_rejects_non_object_params() {
        let mut value = sample_agent_command_value();
        value["commands"][0]["params"] = json!(["invalid"]);

        assert!(serde_json::from_value::<AgentCommandV1>(value).is_err());
    }

    #[test]
    fn deserialize_agent_command_sample_accepts_arbiter_source() {
        let mut value = sample_agent_command_value();
        value["source"] = json!("arbiter");

        let cmd: AgentCommandV1 =
            serde_json::from_value(value).expect("arbiter source should remain valid");
        assert_eq!(cmd.source.as_deref(), Some("arbiter"));
    }
}
