/// Redis channel names — must match @bong/schema channels.ts
pub const CH_WORLD_STATE: &str = "bong:world_state";
pub const CH_PLAYER_CHAT: &str = "bong:player_chat";
pub const CH_AGENT_COMMAND: &str = "bong:agent_command";
pub const CH_AGENT_NARRATE: &str = "bong:agent_narrate";
pub const CH_AGENT_WORLD_MODEL: &str = "bong:agent_world_model";

// 修炼 (plan-cultivation-v1 §6.1)
pub const CH_INSIGHT_REQUEST: &str = "bong:insight_request";
pub const CH_INSIGHT_OFFER: &str = "bong:insight_offer";
pub const CH_BREAKTHROUGH_EVENT: &str = "bong:breakthrough_event";
pub const CH_FORGE_EVENT: &str = "bong:forge_event";
pub const CH_CULTIVATION_DEATH: &str = "bong:cultivation_death";
pub const CH_DEATH: &str = "bong:death";
pub const CH_REBIRTH: &str = "bong:rebirth";
pub const CH_DEATH_INSIGHT: &str = "bong:death_insight";
pub const CH_AGING: &str = "bong:aging";
pub const CH_LIFESPAN_EVENT: &str = "bong:lifespan_event";
pub const CH_DUO_SHE_EVENT: &str = "bong:duo_she_event";

// NPC / 派系观测（plan-npc-ai-v1 §6）。Agent → Server 指令仍统一走
// `bong:agent_command`，这里仅声明 server → agent 事件流水。
pub const CH_NPC_SPAWN: &str = "bong:npc/spawn";
pub const CH_NPC_DEATH: &str = "bong:npc/death";
pub const CH_FACTION_EVENT: &str = "bong:faction/event";

// 战斗观测 (combat-no-ui-c1-c3 Task 7)
pub const CH_COMBAT_REALTIME: &str = "bong:combat_realtime";
pub const CH_COMBAT_SUMMARY: &str = "bong:combat_summary";
pub const CH_ARMOR_DURABILITY_CHANGED: &str = "bong:armor/durability_changed";

// botany 观测通道（server-agent 侧），客户端 gameplay 仍走 bong:server_data / bong:client_request
// 注：每株 spawn / wither 不单推（agent 难处理高频事件）——聚合走 `bong:botany/ecology`，
// 从两次 snapshot 的 plant_counts 差即可算出 zone 级 spawn/wither 量。未来如需"阈值告警"，
// 可扩 ecology snapshot 加 delta 字段或新增 alert 专用 channel。
pub const CH_BOTANY_HARVEST_PROGRESS: &str = "bong:botany/harvest_progress";
pub const CH_BOTANY_ECOLOGY: &str = "bong:botany/ecology";

// 子技能 (plan-skill-v1 §8)：server → agent，agent 消费生成升级 narration / NPC skill 画像
pub const CH_SKILL_XP_GAIN: &str = "bong:skill/xp_gain";
pub const CH_SKILL_LV_UP: &str = "bong:skill/lv_up";
pub const CH_SKILL_CAP_CHANGED: &str = "bong:skill/cap_changed";
pub const CH_SKILL_SCROLL_USED: &str = "bong:skill/scroll_used";

// 活坍缩渊 (plan-tsy-zone-followup-v1 §2.4)
// 玩家踏进 / 走出 TSY 秘境时由 server publish；entry / exit 共享同一频道，consumer 按 `kind` 字段 dispatch。
pub const CH_TSY_EVENT: &str = "bong:tsy_event";

// 炼器（武器）（plan-forge-v1 §4）—— gameplay 仍走 bong:client_request / bong:server_data。
// 以下为 server→agent 观测频道（锻造事件推送给天道 Agent 生成 narration）。
pub const CH_FORGE_START: &str = "bong:forge/start";
pub const CH_FORGE_OUTCOME: &str = "bong:forge/outcome";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redis_v1_channel_constants_remain_frozen() {
        assert_eq!(CH_WORLD_STATE, "bong:world_state");
        assert_eq!(CH_PLAYER_CHAT, "bong:player_chat");
        assert_eq!(CH_AGENT_COMMAND, "bong:agent_command");
        assert_eq!(CH_AGENT_NARRATE, "bong:agent_narrate");
        assert_eq!(CH_AGENT_WORLD_MODEL, "bong:agent_world_model");
        assert_eq!(CH_DEATH, "bong:death");
        assert_eq!(CH_REBIRTH, "bong:rebirth");
        assert_eq!(CH_DEATH_INSIGHT, "bong:death_insight");
        assert_eq!(CH_AGING, "bong:aging");
        assert_eq!(CH_LIFESPAN_EVENT, "bong:lifespan_event");
        assert_eq!(CH_DUO_SHE_EVENT, "bong:duo_she_event");
        assert_eq!(CH_NPC_SPAWN, "bong:npc/spawn");
        assert_eq!(CH_NPC_DEATH, "bong:npc/death");
        assert_eq!(CH_FACTION_EVENT, "bong:faction/event");
        assert_eq!(CH_COMBAT_REALTIME, "bong:combat_realtime");
        assert_eq!(CH_COMBAT_SUMMARY, "bong:combat_summary");
        assert_eq!(CH_ARMOR_DURABILITY_CHANGED, "bong:armor/durability_changed");
        assert_eq!(CH_BOTANY_HARVEST_PROGRESS, "bong:botany/harvest_progress");
        assert_eq!(CH_BOTANY_ECOLOGY, "bong:botany/ecology");
        assert_eq!(CH_SKILL_XP_GAIN, "bong:skill/xp_gain");
        assert_eq!(CH_SKILL_LV_UP, "bong:skill/lv_up");
        assert_eq!(CH_SKILL_CAP_CHANGED, "bong:skill/cap_changed");
        assert_eq!(CH_SKILL_SCROLL_USED, "bong:skill/scroll_used");
        assert_eq!(CH_TSY_EVENT, "bong:tsy_event");
        assert_eq!(CH_FORGE_START, "bong:forge/start");
        assert_eq!(CH_FORGE_OUTCOME, "bong:forge/outcome");
    }
}
