/// Redis channel names — must match @bong/schema channels.ts
pub const CH_WORLD_STATE: &str = "bong:world_state";
pub const CH_PLAYER_CHAT: &str = "bong:player_chat";
pub const CH_AGENT_COMMAND: &str = "bong:agent_command";
pub const CH_AGENT_NARRATE: &str = "bong:agent_narrate";
pub const CH_AGENT_WORLD_MODEL: &str = "bong:agent_world_model";
pub const CH_CALAMITY_INTENT: &str = "bong:calamity_intent";
pub const CH_SEASON_CHANGED: &str = "bong:season_changed";
pub const CH_BONE_COIN_TICK: &str = "bong:bone_coin_tick";
pub const CH_PRICE_INDEX: &str = "bong:price_index";

// 修炼 (plan-cultivation-v1 §6.1)
pub const CH_INSIGHT_REQUEST: &str = "bong:insight_request";
pub const CH_INSIGHT_OFFER: &str = "bong:insight_offer";
pub const CH_HEART_DEMON_REQUEST: &str = "bong:heart_demon_request";
pub const CH_HEART_DEMON_OFFER: &str = "bong:heart_demon_offer";
pub const CH_BREAKTHROUGH_EVENT: &str = "bong:breakthrough_event";
pub const CH_BREAKTHROUGH_CINEMATIC: &str = "bong:breakthrough_cinematic";
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

// 化虚专属 action（plan-void-actions-v1）：四类行为各自 fanout，agent 侧
// 订阅后统一生成全服 narration。
pub const CH_VOID_ACTION_SUPPRESS_TSY: &str = "bong:void_action/suppress_tsy";
pub const CH_VOID_ACTION_EXPLODE_ZONE: &str = "bong:void_action/explode_zone";
pub const CH_VOID_ACTION_BARRIER: &str = "bong:void_action/barrier";
pub const CH_VOID_ACTION_LEGACY_ASSIGN: &str = "bong:void_action/legacy_assign";

pub fn void_action_channel(
    kind: crate::cultivation::void::components::VoidActionKind,
) -> &'static str {
    match kind {
        crate::cultivation::void::components::VoidActionKind::SuppressTsy => {
            CH_VOID_ACTION_SUPPRESS_TSY
        }
        crate::cultivation::void::components::VoidActionKind::ExplodeZone => {
            CH_VOID_ACTION_EXPLODE_ZONE
        }
        crate::cultivation::void::components::VoidActionKind::Barrier => CH_VOID_ACTION_BARRIER,
        crate::cultivation::void::components::VoidActionKind::LegacyAssign => {
            CH_VOID_ACTION_LEGACY_ASSIGN
        }
    }
}

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
pub const CH_SOCIAL_NICHE_INTRUSION: &str = "bong:social/niche_intrusion";
pub const CH_HIGH_RENOWN_MILESTONE: &str = "bong:high_renown_milestone";
pub const CH_ZONE_PRESSURE_CROSSED: &str = "bong:zone/pressure_crossed";

// 天气事件起 / 落（plan-lingtian-weather-v1 §3 / §4.4）。payload 见
// `crate::schema::lingtian_weather::WeatherEventUpdateV1`。
pub const CH_WEATHER_EVENT_UPDATE: &str = "bong:weather_event_update";

// zone-scoped 长时环境效果（plan-zone-environment-v1）。payload 见
// `crate::schema::zone_environment::ZoneEnvironmentStateV1`。
pub const CH_ZONE_ENVIRONMENT_UPDATE: &str = "bong:zone_environment_update";

// 噬元鼠相变（plan-rat-v1 P4）。server 检测 chunk 局部相变，agent 决定是否升级为跨 zone 灵蝗潮。
pub const CH_RAT_PHASE_EVENT: &str = "bong:rat_phase_event";

// 战斗观测 (combat-no-ui-c1-c3 Task 7)
pub const CH_COMBAT_REALTIME: &str = "bong:combat_realtime";
pub const CH_COMBAT_SUMMARY: &str = "bong:combat_summary";
pub const CH_STYLE_BALANCE_TELEMETRY: &str = "bong:style_balance_telemetry";
pub const CH_ANTICHEAT: &str = "bong:anticheat";
pub const CH_ARMOR_DURABILITY_CHANGED: &str = "bong:armor/durability_changed";
pub const CH_WOLIU_BACKFIRE: &str = "bong:woliu/backfire";
pub const CH_WOLIU_PROJECTILE_DRAINED: &str = "bong:woliu/projectile_drained";
pub const CH_WOLIU_VORTEX_STATE: &str = "bong:woliu/vortex_state";
pub const CH_WOLIU_V2_CAST: &str = "bong:woliu_v2/cast";
pub const CH_WOLIU_V2_BACKFIRE: &str = "bong:woliu_v2/backfire";
pub const CH_WOLIU_V2_TURBULENCE: &str = "bong:woliu_v2/turbulence";
pub const CH_ZHENMAI_SKILL_EVENT: &str = "bong:zhenmai/skill_event";
pub const CH_BAOMAI_V3_SKILL_EVENT: &str = "bong:baomai_v3/skill_event";
pub const CH_ZHENFA_V2_EVENT: &str = "bong:zhenfa/v2_event";
pub const CH_DUGU_POISON_PROGRESS: &str = "bong:dugu/poison_progress";
pub const CH_POISON_DOSE_EVENT: &str = "bong:poison/dose";
pub const CH_POISON_OVERDOSE_EVENT: &str = "bong:poison/overdose";
pub const CH_DUGU_V2_CAST: &str = "bong:dugu_v2/cast";
pub const CH_DUGU_V2_SELF_CURE: &str = "bong:dugu_v2/self_cure";
pub const CH_DUGU_V2_REVERSE: &str = "bong:dugu_v2/reverse";
pub const CH_ANQI_CARRIER_CHARGED: &str = "bong:combat/carrier_charged";
pub const CH_ANQI_CARRIER_IMPACT: &str = "bong:combat/carrier_impact";
pub const CH_ANQI_PROJECTILE_DESPAWNED: &str = "bong:combat/projectile_despawned";
pub const CH_ANQI_MULTI_SHOT: &str = "bong:anqi/multi_shot";
pub const CH_ANQI_QI_INJECTION: &str = "bong:anqi/qi_injection";
pub const CH_ANQI_ECHO_FRACTAL: &str = "bong:anqi/echo_fractal";
pub const CH_ANQI_CARRIER_ABRASION: &str = "bong:anqi/carrier_abrasion";
pub const CH_ANQI_CONTAINER_SWAP: &str = "bong:anqi/container_swap";
pub const CH_TUIKE_SHED: &str = "bong:tuike/shed";
pub const CH_TUIKE_FALSE_SKIN_STATE: &str = "bong:tuike/false_skin_state";
pub const CH_TUIKE_V2_SKILL_EVENT: &str = "bong:tuike_v2/skill_event";
pub const CH_YIDAO_EVENT: &str = "bong:yidao/event";

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
pub const CH_SPIRIT_TREASURE_DIALOGUE_REQUEST: &str = "bong:spirit_treasure_dialogue_request";
pub const CH_SPIRIT_TREASURE_DIALOGUE: &str = "bong:spirit_treasure_dialogue";

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

// 身份与信誉（plan-identity-v1 §7）—— Wanted 档玩家通知 agent。
pub const CH_WANTED_PLAYER: &str = "bong:wanted_player";

// 通用手搓（plan-craft-v1 P3）—— server → agent 观测频道。
pub const CH_CRAFT_OUTCOME: &str = "bong:craft/outcome";
pub const CH_CRAFT_RECIPE_UNLOCKED: &str = "bong:craft/recipe_unlocked";

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
        assert_eq!(CH_CALAMITY_INTENT, "bong:calamity_intent");
        assert_eq!(CH_SEASON_CHANGED, "bong:season_changed");
        assert_eq!(CH_BONE_COIN_TICK, "bong:bone_coin_tick");
        assert_eq!(CH_PRICE_INDEX, "bong:price_index");
        assert_eq!(CH_INSIGHT_REQUEST, "bong:insight_request");
        assert_eq!(CH_INSIGHT_OFFER, "bong:insight_offer");
        assert_eq!(CH_HEART_DEMON_REQUEST, "bong:heart_demon_request");
        assert_eq!(CH_HEART_DEMON_OFFER, "bong:heart_demon_offer");
        assert_eq!(CH_BREAKTHROUGH_EVENT, "bong:breakthrough_event");
        assert_eq!(CH_BREAKTHROUGH_CINEMATIC, "bong:breakthrough_cinematic");
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
        assert_eq!(CH_VOID_ACTION_SUPPRESS_TSY, "bong:void_action/suppress_tsy");
        assert_eq!(CH_VOID_ACTION_EXPLODE_ZONE, "bong:void_action/explode_zone");
        assert_eq!(CH_VOID_ACTION_BARRIER, "bong:void_action/barrier");
        assert_eq!(
            CH_VOID_ACTION_LEGACY_ASSIGN,
            "bong:void_action/legacy_assign"
        );
        assert_eq!(CH_NPC_SPAWN, "bong:npc/spawn");
        assert_eq!(CH_NPC_DEATH, "bong:npc/death");
        assert_eq!(CH_FACTION_EVENT, "bong:faction/event");
        assert_eq!(CH_SOCIAL_EXPOSURE, "bong:social/exposure");
        assert_eq!(CH_SOCIAL_PACT, "bong:social/pact");
        assert_eq!(CH_SOCIAL_FEUD, "bong:social/feud");
        assert_eq!(CH_SOCIAL_RENOWN_DELTA, "bong:social/renown_delta");
        assert_eq!(CH_SOCIAL_NICHE_INTRUSION, "bong:social/niche_intrusion");
        assert_eq!(CH_HIGH_RENOWN_MILESTONE, "bong:high_renown_milestone");
        assert_eq!(CH_ZONE_PRESSURE_CROSSED, "bong:zone/pressure_crossed");
        assert_eq!(CH_WEATHER_EVENT_UPDATE, "bong:weather_event_update");
        assert_eq!(CH_ZONE_ENVIRONMENT_UPDATE, "bong:zone_environment_update");
        assert_eq!(CH_RAT_PHASE_EVENT, "bong:rat_phase_event");
        assert_eq!(CH_COMBAT_REALTIME, "bong:combat_realtime");
        assert_eq!(CH_COMBAT_SUMMARY, "bong:combat_summary");
        assert_eq!(CH_STYLE_BALANCE_TELEMETRY, "bong:style_balance_telemetry");
        assert_eq!(CH_ANTICHEAT, "bong:anticheat");
        assert_eq!(CH_ARMOR_DURABILITY_CHANGED, "bong:armor/durability_changed");
        assert_eq!(CH_WOLIU_BACKFIRE, "bong:woliu/backfire");
        assert_eq!(CH_WOLIU_PROJECTILE_DRAINED, "bong:woliu/projectile_drained");
        assert_eq!(CH_WOLIU_VORTEX_STATE, "bong:woliu/vortex_state");
        assert_eq!(CH_WOLIU_V2_CAST, "bong:woliu_v2/cast");
        assert_eq!(CH_WOLIU_V2_BACKFIRE, "bong:woliu_v2/backfire");
        assert_eq!(CH_WOLIU_V2_TURBULENCE, "bong:woliu_v2/turbulence");
        assert_eq!(CH_ZHENMAI_SKILL_EVENT, "bong:zhenmai/skill_event");
        assert_eq!(CH_BAOMAI_V3_SKILL_EVENT, "bong:baomai_v3/skill_event");
        assert_eq!(CH_ZHENFA_V2_EVENT, "bong:zhenfa/v2_event");
        assert_eq!(CH_DUGU_POISON_PROGRESS, "bong:dugu/poison_progress");
        assert_eq!(CH_POISON_DOSE_EVENT, "bong:poison/dose");
        assert_eq!(CH_POISON_OVERDOSE_EVENT, "bong:poison/overdose");
        assert_eq!(CH_DUGU_V2_CAST, "bong:dugu_v2/cast");
        assert_eq!(CH_DUGU_V2_SELF_CURE, "bong:dugu_v2/self_cure");
        assert_eq!(CH_DUGU_V2_REVERSE, "bong:dugu_v2/reverse");
        assert_eq!(CH_ANQI_CARRIER_CHARGED, "bong:combat/carrier_charged");
        assert_eq!(CH_ANQI_CARRIER_IMPACT, "bong:combat/carrier_impact");
        assert_eq!(
            CH_ANQI_PROJECTILE_DESPAWNED,
            "bong:combat/projectile_despawned"
        );
        assert_eq!(CH_TUIKE_SHED, "bong:tuike/shed");
        assert_eq!(CH_TUIKE_FALSE_SKIN_STATE, "bong:tuike/false_skin_state");
        assert_eq!(CH_TUIKE_V2_SKILL_EVENT, "bong:tuike_v2/skill_event");
        assert_eq!(CH_YIDAO_EVENT, "bong:yidao/event");
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
        assert_eq!(
            CH_SPIRIT_TREASURE_DIALOGUE_REQUEST,
            "bong:spirit_treasure_dialogue_request"
        );
        assert_eq!(CH_SPIRIT_TREASURE_DIALOGUE, "bong:spirit_treasure_dialogue");
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
        assert_eq!(CH_WANTED_PLAYER, "bong:wanted_player");
        // plan-craft-v1 P3 — 通用手搓 server → agent 频道
        assert_eq!(CH_CRAFT_OUTCOME, "bong:craft/outcome");
        assert_eq!(CH_CRAFT_RECIPE_UNLOCKED, "bong:craft/recipe_unlocked");
    }
}
