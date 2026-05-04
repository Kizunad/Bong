/// Redis channel names — must match @bong/schema channels.ts
pub const CH_WORLD_STATE: &str = "bong:world_state";
pub const CH_PLAYER_CHAT: &str = "bong:player_chat";
pub const CH_AGENT_COMMAND: &str = "bong:agent_command";
pub const CH_AGENT_NARRATE: &str = "bong:agent_narrate";
pub const CH_AGENT_WORLD_MODEL: &str = "bong:agent_world_model";

// 修炼 (plan-cultivation-v1 §6.1)
pub const CH_INSIGHT_REQUEST: &str = "bong:insight_request";
pub const CH_INSIGHT_OFFER: &str = "bong:insight_offer";
pub const CH_HEART_DEMON_REQUEST: &str = "bong:heart_demon_request";
pub const CH_HEART_DEMON_OFFER: &str = "bong:heart_demon_offer";
pub const CH_BREAKTHROUGH_EVENT: &str = "bong:breakthrough_event";
pub const CH_FORGE_EVENT: &str = "bong:forge_event";
pub const CH_CULTIVATION_DEATH: &str = "bong:cultivation_death";
pub const CH_DEATH: &str = "bong:death";
pub const CH_REBIRTH: &str = "bong:rebirth";
pub const CH_DEATH_INSIGHT: &str = "bong:death_insight";
pub const CH_AGING: &str = "bong:aging";
pub const CH_LIFESPAN_EVENT: &str = "bong:lifespan_event";
pub const CH_DUO_SHE_EVENT: &str = "bong:duo_she_event";

// 天劫（plan-tribulation-v1 §6）：所有天劫事件统一进主 channel；Redis bridge
// 同时 fanout 到 phase/kind 子 channel，供前端/agent 按语义分流。
pub const CH_TRIBULATION: &str = "bong:tribulation";
pub const CH_TRIBULATION_OMEN: &str = "bong:tribulation/omen";
pub const CH_TRIBULATION_LOCK: &str = "bong:tribulation/lock";
pub const CH_TRIBULATION_WAVE: &str = "bong:tribulation/wave";
pub const CH_TRIBULATION_SETTLE: &str = "bong:tribulation/settle";
pub const CH_TRIBULATION_COLLAPSE: &str = "bong:tribulation/collapse";

// NPC / 派系观测（plan-npc-ai-v1 §6）。Agent → Server 指令仍统一走
// `bong:agent_command`，这里仅声明 server → agent 事件流水。
pub const CH_NPC_SPAWN: &str = "bong:npc/spawn";
pub const CH_NPC_DEATH: &str = "bong:npc/death";
pub const CH_FACTION_EVENT: &str = "bong:faction/event";

// 玩家社交 / 匿名 / 声名（plan-social-v1 §7）。server 为权威，agent 只消费事件流水。
pub const CH_SOCIAL_EXPOSURE: &str = "bong:social/exposure";
pub const CH_SOCIAL_PACT: &str = "bong:social/pact";
pub const CH_SOCIAL_FEUD: &str = "bong:social/feud";
pub const CH_SOCIAL_RENOWN_DELTA: &str = "bong:social/renown_delta";

// 战斗观测 (combat-no-ui-c1-c3 Task 7)
pub const CH_COMBAT_REALTIME: &str = "bong:combat_realtime";
pub const CH_COMBAT_SUMMARY: &str = "bong:combat_summary";
pub const CH_ANTICHEAT: &str = "bong:anticheat";
pub const CH_ARMOR_DURABILITY_CHANGED: &str = "bong:armor/durability_changed";
pub const CH_WOLIU_BACKFIRE: &str = "bong:woliu/backfire";
pub const CH_WOLIU_PROJECTILE_DRAINED: &str = "bong:woliu/projectile_drained";
pub const CH_WOLIU_VORTEX_STATE: &str = "bong:woliu/vortex_state";
pub const CH_ANQI_CARRIER_CHARGED: &str = "bong:combat/carrier_charged";
pub const CH_ANQI_CARRIER_IMPACT: &str = "bong:combat/carrier_impact";
pub const CH_ANQI_PROJECTILE_DESPAWNED: &str = "bong:combat/projectile_despawned";

// 伪灵脉（plan-terrain-pseudo-vein-v1 §6.1）
pub const CH_PSEUDO_VEIN_ACTIVE: &str = "bong:pseudo_vein:active";
pub const CH_PSEUDO_VEIN_DISSIPATE: &str = "bong:pseudo_vein:dissipate";
pub const CH_ZONG_CORE_ACTIVATED: &str = "bong:zong_core_activated";

// botany 观测通道（server-agent 侧），客户端 gameplay 仍走 bong:server_data / bong:client_request
// 注：每株 spawn / wither 不单推（agent 难处理高频事件）——聚合走 `bong:botany/ecology`，
// 从两次 snapshot 的 plant_counts 差即可算出 zone 级 spawn/wither 量。未来如需"阈值告警"，
// 可扩 ecology snapshot 加 delta 字段或新增 alert 专用 channel。
pub const CH_BOTANY_HARVEST_PROGRESS: &str = "bong:botany/harvest_progress";
pub const CH_BOTANY_ECOLOGY: &str = "bong:botany/ecology";
pub const CH_LUMBER_PROGRESS: &str = "bong:lumber_progress";

// 子技能 (plan-skill-v1 §8)：server → agent，agent 消费生成升级 narration / NPC skill 画像
pub const CH_SKILL_XP_GAIN: &str = "bong:skill/xp_gain";
pub const CH_SKILL_LV_UP: &str = "bong:skill/lv_up";
pub const CH_SKILL_CAP_CHANGED: &str = "bong:skill/cap_changed";
pub const CH_SKILL_SCROLL_USED: &str = "bong:skill/scroll_used";

// 灵眼（plan-spirit-eye-v1 §8）：server → agent 观测频道。
pub const CH_SPIRIT_EYE_MIGRATE: &str = "bong:spirit_eye/migrate";
pub const CH_SPIRIT_EYE_DISCOVERED: &str = "bong:spirit_eye/discovered";
pub const CH_SPIRIT_EYE_USED_FOR_BREAKTHROUGH: &str = "bong:spirit_eye/used_for_breakthrough";

// 活坍缩渊 (plan-tsy-zone-followup-v1 §2.4)
// 玩家踏进 / 走出 TSY 秘境时由 server publish；entry / exit 共享同一频道，consumer 按 `kind` 字段 dispatch。
pub const CH_TSY_EVENT: &str = "bong:tsy_event";

// 新手 POI（plan-poi-novice-v1 §P2）：spawned / trespass 共享频道，agent 按 kind dispatch。
pub const CH_POI_NOVICE_EVENT: &str = "bong:poi_novice/event";

// 炼器（武器）（plan-forge-v1 §4）—— gameplay 仍走 bong:client_request / bong:server_data。
// 以下为 server→agent 观测频道（锻造事件推送给天道 Agent 生成 narration）。
pub const CH_FORGE_START: &str = "bong:forge/start";
pub const CH_FORGE_OUTCOME: &str = "bong:forge/outcome";

// 炼丹（plan-alchemy-client-v1 §6 / P4）—— server → agent 观测频道。
pub const CH_ALCHEMY_SESSION_START: &str = "bong:alchemy/session_start";
pub const CH_ALCHEMY_SESSION_END: &str = "bong:alchemy/session_end";
pub const CH_ALCHEMY_INTERVENTION_RESULT: &str = "bong:alchemy/intervention_result";
pub const CH_ALCHEMY_INSIGHT: &str = "bong:alchemy_insight";

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
        assert_eq!(CH_INSIGHT_REQUEST, "bong:insight_request");
        assert_eq!(CH_INSIGHT_OFFER, "bong:insight_offer");
        assert_eq!(CH_HEART_DEMON_REQUEST, "bong:heart_demon_request");
        assert_eq!(CH_HEART_DEMON_OFFER, "bong:heart_demon_offer");
        assert_eq!(CH_BREAKTHROUGH_EVENT, "bong:breakthrough_event");
        assert_eq!(CH_FORGE_EVENT, "bong:forge_event");
        assert_eq!(CH_CULTIVATION_DEATH, "bong:cultivation_death");
        assert_eq!(CH_DEATH, "bong:death");
        assert_eq!(CH_REBIRTH, "bong:rebirth");
        assert_eq!(CH_DEATH_INSIGHT, "bong:death_insight");
        assert_eq!(CH_AGING, "bong:aging");
        assert_eq!(CH_LIFESPAN_EVENT, "bong:lifespan_event");
        assert_eq!(CH_DUO_SHE_EVENT, "bong:duo_she_event");
        assert_eq!(CH_TRIBULATION, "bong:tribulation");
        assert_eq!(CH_TRIBULATION_OMEN, "bong:tribulation/omen");
        assert_eq!(CH_TRIBULATION_LOCK, "bong:tribulation/lock");
        assert_eq!(CH_TRIBULATION_WAVE, "bong:tribulation/wave");
        assert_eq!(CH_TRIBULATION_SETTLE, "bong:tribulation/settle");
        assert_eq!(CH_TRIBULATION_COLLAPSE, "bong:tribulation/collapse");
        assert_eq!(CH_NPC_SPAWN, "bong:npc/spawn");
        assert_eq!(CH_NPC_DEATH, "bong:npc/death");
        assert_eq!(CH_FACTION_EVENT, "bong:faction/event");
        assert_eq!(CH_SOCIAL_EXPOSURE, "bong:social/exposure");
        assert_eq!(CH_SOCIAL_PACT, "bong:social/pact");
        assert_eq!(CH_SOCIAL_FEUD, "bong:social/feud");
        assert_eq!(CH_SOCIAL_RENOWN_DELTA, "bong:social/renown_delta");
        assert_eq!(CH_COMBAT_REALTIME, "bong:combat_realtime");
        assert_eq!(CH_COMBAT_SUMMARY, "bong:combat_summary");
        assert_eq!(CH_ANTICHEAT, "bong:anticheat");
        assert_eq!(CH_ARMOR_DURABILITY_CHANGED, "bong:armor/durability_changed");
        assert_eq!(CH_WOLIU_BACKFIRE, "bong:woliu/backfire");
        assert_eq!(CH_WOLIU_PROJECTILE_DRAINED, "bong:woliu/projectile_drained");
        assert_eq!(CH_WOLIU_VORTEX_STATE, "bong:woliu/vortex_state");
        assert_eq!(CH_ANQI_CARRIER_CHARGED, "bong:combat/carrier_charged");
        assert_eq!(CH_ANQI_CARRIER_IMPACT, "bong:combat/carrier_impact");
        assert_eq!(
            CH_ANQI_PROJECTILE_DESPAWNED,
            "bong:combat/projectile_despawned"
        );
        assert_eq!(CH_PSEUDO_VEIN_ACTIVE, "bong:pseudo_vein:active");
        assert_eq!(CH_PSEUDO_VEIN_DISSIPATE, "bong:pseudo_vein:dissipate");
        assert_eq!(CH_ZONG_CORE_ACTIVATED, "bong:zong_core_activated");
        assert_eq!(CH_BOTANY_HARVEST_PROGRESS, "bong:botany/harvest_progress");
        assert_eq!(CH_BOTANY_ECOLOGY, "bong:botany/ecology");
        assert_eq!(CH_LUMBER_PROGRESS, "bong:lumber_progress");
        assert_eq!(CH_SKILL_XP_GAIN, "bong:skill/xp_gain");
        assert_eq!(CH_SKILL_LV_UP, "bong:skill/lv_up");
        assert_eq!(CH_SKILL_CAP_CHANGED, "bong:skill/cap_changed");
        assert_eq!(CH_SKILL_SCROLL_USED, "bong:skill/scroll_used");
        assert_eq!(CH_SPIRIT_EYE_MIGRATE, "bong:spirit_eye/migrate");
        assert_eq!(CH_SPIRIT_EYE_DISCOVERED, "bong:spirit_eye/discovered");
        assert_eq!(
            CH_SPIRIT_EYE_USED_FOR_BREAKTHROUGH,
            "bong:spirit_eye/used_for_breakthrough"
        );
        assert_eq!(CH_TSY_EVENT, "bong:tsy_event");
        assert_eq!(CH_POI_NOVICE_EVENT, "bong:poi_novice/event");
        assert_eq!(CH_FORGE_START, "bong:forge/start");
        assert_eq!(CH_FORGE_OUTCOME, "bong:forge/outcome");
        assert_eq!(CH_ALCHEMY_SESSION_START, "bong:alchemy/session_start");
        assert_eq!(CH_ALCHEMY_SESSION_END, "bong:alchemy/session_end");
        assert_eq!(
            CH_ALCHEMY_INTERVENTION_RESULT,
            "bong:alchemy/intervention_result"
        );
        assert_eq!(CH_ALCHEMY_INSIGHT, "bong:alchemy_insight");
    }
}
