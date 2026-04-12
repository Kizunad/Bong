/// Redis channel names — must match @bong/schema channels.ts
pub const CH_WORLD_STATE: &str = "bong:world_state";
pub const CH_PLAYER_CHAT: &str = "bong:player_chat";
pub const CH_AGENT_COMMAND: &str = "bong:agent_command";
pub const CH_AGENT_NARRATE: &str = "bong:agent_narrate";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redis_v1_channel_constants_remain_frozen() {
        assert_eq!(CH_WORLD_STATE, "bong:world_state");
        assert_eq!(CH_PLAYER_CHAT, "bong:player_chat");
        assert_eq!(CH_AGENT_COMMAND, "bong:agent_command");
        assert_eq!(CH_AGENT_NARRATE, "bong:agent_narrate");
    }
}
