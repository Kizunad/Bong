use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::common::CommandType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    #[serde(rename = "type")]
    pub command_type: CommandType,
    pub target: String,
    pub params: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCommandV1 {
    pub v: u8,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub commands: Vec<Command>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
