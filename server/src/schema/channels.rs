/// Redis channel names — must match @bong/schema channels.ts
pub const CH_WORLD_STATE: &str = "bong:world_state";
pub const CH_PLAYER_CHAT: &str = "bong:player_chat";
pub const CH_AGENT_COMMAND: &str = "bong:agent_command";
pub const CH_AGENT_NARRATE: &str = "bong:agent_narrate";

// 修炼 (plan-cultivation-v1 §6.1)
pub const CH_INSIGHT_REQUEST: &str = "bong:insight_request";
pub const CH_INSIGHT_OFFER: &str = "bong:insight_offer";
pub const CH_BREAKTHROUGH_EVENT: &str = "bong:breakthrough_event";
pub const CH_FORGE_EVENT: &str = "bong:forge_event";
pub const CH_CULTIVATION_DEATH: &str = "bong:cultivation_death";

// 战斗观测 (combat-no-ui-c1-c3 Task 7)
pub const CH_COMBAT_REALTIME: &str = "bong:combat_realtime";
pub const CH_COMBAT_SUMMARY: &str = "bong:combat_summary";

// botany 观测通道（server-agent 侧），客户端 gameplay 仍走 bong:server_data / bong:client_request
pub const CH_BOTANY_SPAWN: &str = "bong:botany/spawn";
pub const CH_BOTANY_WITHER: &str = "bong:botany/wither";
pub const CH_BOTANY_HARVEST_PROGRESS: &str = "bong:botany/harvest_progress";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redis_v1_channel_constants_remain_frozen() {
        assert_eq!(CH_WORLD_STATE, "bong:world_state");
        assert_eq!(CH_PLAYER_CHAT, "bong:player_chat");
        assert_eq!(CH_AGENT_COMMAND, "bong:agent_command");
        assert_eq!(CH_AGENT_NARRATE, "bong:agent_narrate");
        assert_eq!(CH_COMBAT_REALTIME, "bong:combat_realtime");
        assert_eq!(CH_COMBAT_SUMMARY, "bong:combat_summary");
        assert_eq!(CH_BOTANY_SPAWN, "bong:botany/spawn");
        assert_eq!(CH_BOTANY_WITHER, "bong:botany/wither");
        assert_eq!(CH_BOTANY_HARVEST_PROGRESS, "bong:botany/harvest_progress");
    }
}
