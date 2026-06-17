/// Redis channel names — must match @bong/schema channels.ts
pub const CH_WORLD_STATE: &str = "bong:world_state";
pub const CH_PLAYER_CHAT: &str = "bong:player_chat";
pub const CH_AGENT_COMMAND: &str = "bong:agent_command";
pub const CH_AGENT_NARRATE: &str = "bong:agent_narrate";
pub const CH_TIANDAO_HUNT_NARRATION_REQUEST: &str = "bong:tiandao_hunt_narration_request";
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
pub const CH_DEATH_CINEMATIC: &str = "bong:death_cinematic";
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

// plan-halfstep-rechallenge-integration-v1 P1：半步化虚重渡触发 → agent narration。
// server 将 `HalfStepRechallengeTriggerEvent` 序列化后 publish 到此 channel；
// agent 订阅后按 zone_halfstep_count 路由 player / zone narration。
pub const CH_HALFSTEP_RECHALLENGE: &str = "bong:tribulation/halfstep_rechallenge";

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

// plan-offscreen-war-v1 P2：离屏 dormant 派系互殴战果 telemetry。
//
// 纯**遥测**通道（server → 外部观测脚本 / 调试）：每场离屏战死发一条
// `DormantCombatOutcomeV1`（winner/loser/zone/qi_released），让真服 e2e 能把战果与
// `bong:npc/death` 对账。**真元流动不走这里**——败者残余真元守恒回灌唯一走
// `release_dormant_qi_to_zone` → `ledger.transfer(ReleaseToZone)`（真实改 balance），
// 本通道只是观测旁路（绝不学「只 emit QiTransfer 无人 apply」的吞真元红线——那会让真元
// 凭空蒸发，§10.1 #5）。agent 派系叙事 P4 复用 `bong:npc/death`，**不**订阅本通道。
pub const CH_NPC_COMBAT: &str = "bong:npc/combat";

// plan-offscreen-war-v1 P3：克制式战场遗物创建 telemetry（纯观测旁路）。一名知名战死者
// 在战场留下待物化遗物（已落盘 sqlite pending_dormant_relics）时 publish 一条
// `PendingDormantRelicV1`。**零真元**——遗物不携带真元（持久层不碰 ledger），本通道只搬观测
// 字段，让真服 e2e 在不便直接读 sqlite 时仍能 headless 断言"知名战死 → 遗物创建"（§11）。
pub const CH_NPC_RELIC: &str = "bong:npc/relic";

// plan-offscreen-war-v1 P0：守恒 telemetry。周期性 HASH（非 pub/sub），落 WorldQiAccount
// 各账户余额 + total_observed，让真服 e2e 能 `HGETALL bong:qi/ledger` 做精确守恒断言。
pub const QI_LEDGER_REDIS_KEY: &str = "bong:qi/ledger";

// plan-offscreen-war-v1 P5：散修群体消长 telemetry（纯观测旁路）。周期性 publish 每个
// 涌现群体一条 `FactionStateV1`（人口 / 消长 status / 涌现强者），让真服 e2e / 调试脚本能
// headless 观测「{zone}一带散修」群体的此消彼长。**零真元流动**——census 全只读 dormant
// store + faction store，强者陨落仍走 P2 的 release_dormant_qi_to_zone，本通道不碰 ledger。
pub const CH_FACTION_STATE: &str = "bong:faction_state";

// plan-faction-expansion-v1 P0：具名势力注册表快照（纯观测，防孤岛 #6）。
//
// 与 CH_FACTION_STATE（emergent group census，bong:faction_state）完全独立——
// 不同 key、不同 struct（NamedFactionStateV1 vs FactionStateV1），避免撞名。
// P0 publish_named_faction_state system 真发一帧到此 key，防孤岛（数据模型非死结构）。
// 下游：social-v2 WarReputation / faction-wars FactionWarEventV1 消费 NamedFactionId。
// P1: faction-wars consumes NamedFactionId via faction_id_for_war
pub const CH_NAMED_FACTION_STATE: &str = "bong:named_faction_state";

// plan-offscreen-war-v1 P6：涌现区域冲突生命周期（纯观测旁路，零真元）。
//
// 每次 WarPhaseChanged（Emerging/Skirmish/Settling/Aftermath）或玩家参与改变 role 计数时
// publish 一条 `FactionWarEventV1`。**末法残土无宣战 / 无具名宗门**——战事是离屏 dormant
// 群体自发升级的涌现冲突；payload 仅携带裸 group_id（无专名）+ 匿名区域描述符
// `"{zone}一带散修"`。守恒红线：本通道 **不含任何真元字段**；真元流动仍唯一走 P2。
pub const CH_FACTION_WAR: &str = "bong:faction/war";

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
/// plan-combat-skill-feedback-bridges-v1 P2 — 爆脉 v3 残余事件通道
pub const CH_BAOMAI_V3_MOUNTAIN_SHAKE: &str = "bong:baomai_v3/mountain_shake";
pub const CH_BAOMAI_V3_BLOOD_BURN: &str = "bong:baomai_v3/blood_burn";
pub const CH_BAOMAI_V3_TRANSCENDENCE_EXPIRED: &str = "bong:baomai_v3/transcendence_expired";
pub const CH_BAOMAI_V3_OVERLOAD_RIPPLE: &str = "bong:baomai_v3/overload_ripple";
pub const CH_ZHENFA_V2_EVENT: &str = "bong:zhenfa/v2_event";
pub const CH_DUGU_POISON_PROGRESS: &str = "bong:dugu/poison_progress";
pub const CH_DUGU_ANTIDOTE_RESULT: &str = "bong:dugu/antidote_result";
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
/// plan-combat-skill-feedback-bridges-v1 P6 — 蜕壳灰烬入包事件（server → agent）。
pub const CH_TUIKE_ASH_DECAY: &str = "bong:tuike_v2/ash_decay";

// 垂死大能遭遇（plan-dying-elder-v1 §P0）—— server → agent 叙事频道。
// payload JSON：`ElderEncounterEventV1`（zone_name / elder_entity_id / event_kind / betray_probability）。
// event_kind: "appeared" | "dan_received" | "betrayal" | "dead_natural" | "dead_player_kill"。
// agent 订阅后 LLM 生成 zone perception / death broadcast 两类 narration（各 2 条文案）。
pub const CH_ELDER_ENCOUNTER: &str = "bong:elder_encounter";
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
pub const CH_TECHNIQUE_SCROLL_READ: &str = "bong:technique/scroll_read";
pub const CH_TECHNIQUE_LEARNED: &str = "bong:technique/learned";
pub const CH_TECHNIQUE_MASTERED: &str = "bong:technique/mastered";
pub const CH_TECHNIQUE_PROFICIENCY_UP: &str = "bong:technique/proficiency_up";

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

// 丹道变异叙事（plan-dandao-runtime-wiring-v1 P2）—— server → agent 观测频道。
/// 变异阶段推进事件，对齐 agent `CHANNELS.MUTATION_EVENT`。
pub const CH_MUTATION_EVENT: &str = "bong:mutation_event";

// 经脉永久 SEVERED（plan-combat-skill-feedback-bridges-v1 P0）—— server → agent 叙事频道。
/// 对齐 agent `CHANNELS.MERIDIAN_SEVERED`（channels.ts:323）。
pub const CH_MERIDIAN_SEVERED: &str = "bong:meridian_severed";

// 我流虚蚀整链（plan-combat-skill-feedback-bridges-v1 P3）—— server → agent 虚蚀阶段推进叙事频道。
/// 对齐 agent `CHANNELS.VOID_EROSION_EVENT`（channels.ts P3 新增）。
pub const CH_VOID_EROSION_EVENT: &str = "bong:void_erosion_event";

// 异变缝合兽（plan-fauna-stitched-beast-v1 P3）—— server → client 兽核吸收幻觉 payload。
/// S2C `bong:core_absorption_hallucination`：client 收到后触发感知幻觉 HUD（视野偏移/绿边像差/bar偏移）。
/// payload JSON: `{"duration_ticks": u32}`，client 端 200tick 后推送取消（duration_ticks=0）。
pub const CH_CORE_ABSORPTION_HALLUCINATION: &str = "bong:core_absorption_hallucination";

// 爆脉 v4（plan-combat-skill-feedback-bridges-v1 P1）—— baomai_v4 反馈整桥。
/// 疤纹回路形成事件，对齐 agent `CHANNELS.BAOMAI_V4_SCAR_CIRCUIT_FORMED`。
pub const CH_BAOMAI_V4_SCAR_CIRCUIT_FORMED: &str = "bong:baomai_v4/scar_circuit_formed";
/// 疤纹回路断裂事件，对齐 agent `CHANNELS.BAOMAI_V4_SCAR_CIRCUIT_BROKEN`。
pub const CH_BAOMAI_V4_SCAR_CIRCUIT_BROKEN: &str = "bong:baomai_v4/scar_circuit_broken";
/// 活茧阶段提升事件，对齐 agent `CHANNELS.BAOMAI_V4_IRON_COCOON_STAGE_UP`。
pub const CH_BAOMAI_V4_IRON_COCOON_STAGE_UP: &str = "bong:baomai_v4/iron_cocoon_stage_up";
/// 共振锁定开始事件，对齐 agent `CHANNELS.BAOMAI_V4_RESONANCE_LOCK`。
pub const CH_BAOMAI_V4_RESONANCE_LOCK: &str = "bong:baomai_v4/resonance_lock";
/// 共振锁定结束事件，对齐 agent `CHANNELS.BAOMAI_V4_RESONANCE_LOCK_END`。
pub const CH_BAOMAI_V4_RESONANCE_LOCK_END: &str = "bong:baomai_v4/resonance_lock_end";

// 领地信息暴露（plan-territory-v1 P3）—— server → agent 领地叙事请求频道。
/// 领地霸主变动叙事请求（新确立 / 被驱逐 / 灵气耗尽）。
/// 对齐 agent `CHANNELS.TERRITORY_NARRATION_REQUEST`。
/// 注意：P3 agent 侧暂无订阅 runtime；push_zone 兜底保证不依赖 agent 即 in-game 可见。
pub const CH_TERRITORY_NARRATION_REQUEST: &str = "bong:territory_narration_request";

// ─── 天道 UI-as-Data（plan-agent-ui-data-v1 P0/P1） ─────────────────────────────

/// Agent → Server: 天道 UI 面板指令（含 realm_gate / allowed_button_ids，Pub/Sub）。
/// 对齐 agent `CHANNELS.AGENT_UI_CMD`。
pub const CH_AGENT_UI_CMD: &str = "bong:agent_ui_cmd";

/// Server → Agent: 天道 UI 面板响应（Pub/Sub）。
/// 对齐 agent `CHANNELS.AGENT_UI_RESPONSE`。
pub const CH_AGENT_UI_RESPONSE: &str = "bong:agent_ui_response";

/// Server → Client: 天道 UI 面板请求（专属 JSON channel，裸 AgentUiRequestPayloadV1，无 envelope）。
///
/// 绕开 `bong:server_data` proto 路径（`proto_convert.rs` 对 AgentUiRequest 是
/// `unreachable!()`，生产会 panic）。client 侧 `BongNetworkHandler.registerAgentUiChannels()`
/// 注册同名 channel listener。
/// 对齐 `network::agent_ui::AGENT_UI_REQUEST_CHANNEL`（两处必须保持一致）。
pub const CH_AGENT_UI_REQUEST: &str = "bong:agent_ui_request";

/// Server → Client: 天道 UI 面板关闭信号（专属 JSON channel，裸 AgentUiClosePayloadV1，无 envelope）。
///
/// 绕开 `bong:server_data` proto 路径（`proto_convert.rs` 对 AgentUiClose 是
/// `unreachable!()`，生产会 panic）。client 侧 `BongNetworkHandler.registerAgentUiChannels()`
/// 注册同名 channel listener。
/// 对齐 `network::agent_ui::AGENT_UI_CLOSE_CHANNEL`（两处必须保持一致）。
pub const CH_AGENT_UI_CLOSE: &str = "bong:agent_ui_close";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halfstep_rechallenge_channel_pin() {
        // plan-halfstep-rechallenge-integration-v1 P1：双端 pin，防 channels.ts 漂移。
        assert_eq!(
            CH_HALFSTEP_RECHALLENGE,
            "bong:tribulation/halfstep_rechallenge",
            "CH_HALFSTEP_RECHALLENGE 必须是 \"bong:tribulation/halfstep_rechallenge\"（plan-halfstep-rechallenge-integration-v1 P1）"
        );
        // 不允许与主 tribulation channel 撞名（防 agent 收双份 event）
        assert_ne!(
            CH_HALFSTEP_RECHALLENGE, CH_TRIBULATION,
            "CH_HALFSTEP_RECHALLENGE 绝不能等于 CH_TRIBULATION（防 agent fanout 混淆）"
        );
    }

    #[test]
    fn redis_v1_channel_constants_remain_frozen() {
        assert_eq!(CH_WORLD_STATE, "bong:world_state");
        assert_eq!(CH_PLAYER_CHAT, "bong:player_chat");
        assert_eq!(CH_AGENT_COMMAND, "bong:agent_command");
        assert_eq!(CH_AGENT_NARRATE, "bong:agent_narrate");
        assert_eq!(
            CH_TIANDAO_HUNT_NARRATION_REQUEST,
            "bong:tiandao_hunt_narration_request"
        );
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
        assert_eq!(CH_DEATH_CINEMATIC, "bong:death_cinematic");
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
        // plan-offscreen-war-v1 P2 — 离屏战果 telemetry channel
        assert_eq!(CH_NPC_COMBAT, "bong:npc/combat");
        // plan-offscreen-war-v1 P3 — 战场遗物创建 telemetry channel
        assert_eq!(CH_NPC_RELIC, "bong:npc/relic");
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
        assert_eq!(CH_DUGU_ANTIDOTE_RESULT, "bong:dugu/antidote_result");
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
        assert_eq!(CH_TUIKE_ASH_DECAY, "bong:tuike_v2/ash_decay");
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
        assert_eq!(CH_TECHNIQUE_SCROLL_READ, "bong:technique/scroll_read");
        assert_eq!(CH_TECHNIQUE_LEARNED, "bong:technique/learned");
        assert_eq!(CH_TECHNIQUE_MASTERED, "bong:technique/mastered");
        assert_eq!(CH_TECHNIQUE_PROFICIENCY_UP, "bong:technique/proficiency_up");
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
        // plan-offscreen-war-v1 P0 — 守恒 telemetry HASH key
        assert_eq!(QI_LEDGER_REDIS_KEY, "bong:qi/ledger");
        // plan-offscreen-war-v1 P5 — 散修群体消长 telemetry channel
        assert_eq!(CH_FACTION_STATE, "bong:faction_state");
        // plan-offscreen-war-v1 P6 — 涌现区域冲突生命周期 telemetry channel（纯观测、零真元）
        assert_eq!(CH_FACTION_WAR, "bong:faction/war");
        // plan-combat-skill-feedback-bridges-v1 P3 — 我流虚蚀阶段推进 agent 叙事频道
        assert_eq!(CH_VOID_EROSION_EVENT, "bong:void_erosion_event");
        // plan-combat-skill-feedback-bridges-v1 P0 — 经脉永久 SEVERED 叙事频道
        assert_eq!(CH_MERIDIAN_SEVERED, "bong:meridian_severed");
        // plan-combat-skill-feedback-bridges-v1 P1 — baomai_v4 反馈整桥频道
        assert_eq!(
            CH_BAOMAI_V4_SCAR_CIRCUIT_FORMED,
            "bong:baomai_v4/scar_circuit_formed"
        );
        assert_eq!(
            CH_BAOMAI_V4_SCAR_CIRCUIT_BROKEN,
            "bong:baomai_v4/scar_circuit_broken"
        );
        assert_eq!(
            CH_BAOMAI_V4_IRON_COCOON_STAGE_UP,
            "bong:baomai_v4/iron_cocoon_stage_up"
        );
        assert_eq!(CH_BAOMAI_V4_RESONANCE_LOCK, "bong:baomai_v4/resonance_lock");
        assert_eq!(
            CH_BAOMAI_V4_RESONANCE_LOCK_END,
            "bong:baomai_v4/resonance_lock_end"
        );
        // plan-combat-skill-feedback-bridges-v1 P2 — 爆脉 v3 残余事件通道 pin
        assert_eq!(CH_BAOMAI_V3_MOUNTAIN_SHAKE, "bong:baomai_v3/mountain_shake");
        assert_eq!(CH_BAOMAI_V3_BLOOD_BURN, "bong:baomai_v3/blood_burn");
        assert_eq!(
            CH_BAOMAI_V3_TRANSCENDENCE_EXPIRED,
            "bong:baomai_v3/transcendence_expired"
        );
        assert_eq!(
            CH_BAOMAI_V3_OVERLOAD_RIPPLE,
            "bong:baomai_v3/overload_ripple"
        );
        // plan-dying-elder-v1 P0 — 垂死大能遭遇 server → agent 叙事频道
        assert_eq!(CH_ELDER_ENCOUNTER, "bong:elder_encounter");
        // plan-territory-v1 P3 — 领地叙事请求频道
        assert_eq!(
            CH_TERRITORY_NARRATION_REQUEST,
            "bong:territory_narration_request"
        );
    }

    #[test]
    fn test_named_faction_state_channel() {
        // plan-faction-expansion-v1 P0：具名势力注册表快照 key 防撞（双端 pin）。
        // CH_NAMED_FACTION_STATE 必须是 "bong:named_faction_state"（非 "bong:faction_state"）。
        assert_eq!(
            CH_NAMED_FACTION_STATE,
            "bong:named_faction_state",
            "CH_NAMED_FACTION_STATE 必须是 \"bong:named_faction_state\"（plan-faction-expansion-v1 P0）"
        );
        assert_ne!(
            CH_NAMED_FACTION_STATE,
            CH_FACTION_STATE,
            "CH_NAMED_FACTION_STATE 绝不能等于 CH_FACTION_STATE（防撞：emergent group census vs 具名势力快照）"
        );
    }

    #[test]
    fn agent_ui_channels_match_typescript_source() {
        // plan-agent-ui-data-v1 P0：双端 pin，防 channels.ts 与 Rust 常量漂移。
        assert_eq!(
            CH_AGENT_UI_CMD, "bong:agent_ui_cmd",
            "CH_AGENT_UI_CMD 必须是 \"bong:agent_ui_cmd\"（plan-agent-ui-data-v1 P0）"
        );
        assert_eq!(
            CH_AGENT_UI_RESPONSE, "bong:agent_ui_response",
            "CH_AGENT_UI_RESPONSE 必须是 \"bong:agent_ui_response\"（plan-agent-ui-data-v1 P0）"
        );
        // 两个 channel 不能相同（防乒乓路由）
        assert_ne!(
            CH_AGENT_UI_CMD, CH_AGENT_UI_RESPONSE,
            "CH_AGENT_UI_CMD 与 CH_AGENT_UI_RESPONSE 必须不同"
        );
    }

    #[test]
    fn agent_ui_s2c_channel_pin() {
        // fix-s2c-proto-panic / plan-agent-ui-data-v1 P1：
        // S2C 专属 JSON channel 常量 pin，防漂移。
        // client BongNetworkHandler.registerAgentUiChannels() 必须注册同名 channel。
        assert_eq!(
            CH_AGENT_UI_REQUEST, "bong:agent_ui_request",
            "CH_AGENT_UI_REQUEST 必须是 \"bong:agent_ui_request\"（fix-s2c-proto-panic）"
        );
        assert_eq!(
            CH_AGENT_UI_CLOSE, "bong:agent_ui_close",
            "CH_AGENT_UI_CLOSE 必须是 \"bong:agent_ui_close\"（fix-s2c-proto-panic）"
        );
        // 两个 S2C channel 不能相同（防 client 端 listener 串联）
        assert_ne!(
            CH_AGENT_UI_REQUEST, CH_AGENT_UI_CLOSE,
            "CH_AGENT_UI_REQUEST 与 CH_AGENT_UI_CLOSE 必须不同"
        );
        // S2C channel 与 cmd/response channel 不能相混（防路由串联）
        assert_ne!(
            CH_AGENT_UI_REQUEST, CH_AGENT_UI_CMD,
            "CH_AGENT_UI_REQUEST 与 CH_AGENT_UI_CMD 必须不同"
        );
        assert_ne!(
            CH_AGENT_UI_CLOSE, CH_AGENT_UI_RESPONSE,
            "CH_AGENT_UI_CLOSE 与 CH_AGENT_UI_RESPONSE 必须不同"
        );
        // 对齐 network::agent_ui 局部常量（两处定义必须一致，此 pin 兜底）
        use crate::network::agent_ui as nai;
        assert_eq!(
            CH_AGENT_UI_REQUEST, nai::AGENT_UI_REQUEST_CHANNEL,
            "schema::channels::CH_AGENT_UI_REQUEST 必须与 network::agent_ui::AGENT_UI_REQUEST_CHANNEL 一致"
        );
        assert_eq!(
            CH_AGENT_UI_CLOSE, nai::AGENT_UI_CLOSE_CHANNEL,
            "schema::channels::CH_AGENT_UI_CLOSE 必须与 network::agent_ui::AGENT_UI_CLOSE_CHANNEL 一致"
        );
    }
}
