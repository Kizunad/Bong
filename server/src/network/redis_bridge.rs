use crossbeam_channel::{Receiver, Sender, TryRecvError};
use serde_json::{Map, Value};
use std::fmt;
use std::time::{Duration, Instant};

use crate::cultivation::void::components::VoidActionKind;
use crate::fauna::rat_phase::RatPhaseChangeEvent;
use crate::npc::dormant::NPC_DORMANT_REDIS_KEY;
use crate::schema::agent_command::AgentCommandV1;
use crate::schema::agent_ui::{AgentUiRequestCommandV1, AgentUiResponsePayloadV1};
use crate::schema::agent_world_model::AgentWorldModelEnvelopeV1;
use crate::schema::alchemy::{
    AlchemyInsightV1, AlchemyInterventionResultV1, AlchemySessionEndV1, AlchemySessionStartV1,
};
use crate::schema::anticheat::AntiCheatReportV1;
use crate::schema::armor_event::ArmorDurabilityChangedV1;
use crate::schema::baomai_v3::{
    BaomaiSkillEventV1, BaomaiV3BloodBurnV1, BaomaiV3MountainShakeV1, BaomaiV3OverloadRippleV1,
    BaomaiV3TranscendenceExpiredV1,
};
use crate::schema::baomai_v4::{
    BaomaiV4IronCocoonStageUpV1, BaomaiV4ResonanceLockEndV1, BaomaiV4ResonanceLockV1,
    BaomaiV4ScarCircuitBrokenV1, BaomaiV4ScarCircuitFormedV1,
};
use crate::schema::botany::BotanyEcologySnapshotV1;
use crate::schema::channels::{
    CH_AGENT_COMMAND, CH_AGENT_NARRATE, CH_AGENT_UI_CMD, CH_AGENT_UI_RESPONSE,
    CH_AGENT_WORLD_MODEL, CH_AGING, CH_ALCHEMY_INSIGHT, CH_ALCHEMY_INTERVENTION_RESULT,
    CH_ALCHEMY_SESSION_END, CH_ALCHEMY_SESSION_START, CH_ANQI_CARRIER_ABRASION,
    CH_ANQI_CARRIER_CHARGED, CH_ANQI_CARRIER_IMPACT, CH_ANQI_CONTAINER_SWAP, CH_ANQI_ECHO_FRACTAL,
    CH_ANQI_MULTI_SHOT, CH_ANQI_PROJECTILE_DESPAWNED, CH_ANQI_QI_INJECTION, CH_ANTICHEAT,
    CH_ARMOR_DURABILITY_CHANGED, CH_BAOMAI_V3_BLOOD_BURN, CH_BAOMAI_V3_MOUNTAIN_SHAKE,
    CH_BAOMAI_V3_OVERLOAD_RIPPLE, CH_BAOMAI_V3_SKILL_EVENT, CH_BAOMAI_V3_TRANSCENDENCE_EXPIRED,
    CH_BAOMAI_V4_IRON_COCOON_STAGE_UP, CH_BAOMAI_V4_RESONANCE_LOCK,
    CH_BAOMAI_V4_RESONANCE_LOCK_END, CH_BAOMAI_V4_SCAR_CIRCUIT_BROKEN,
    CH_BAOMAI_V4_SCAR_CIRCUIT_FORMED, CH_BONE_COIN_TICK, CH_BOTANY_ECOLOGY,
    CH_BOT_DELIVERY_FENCE_ACK, CH_BOT_DELIVERY_FENCE_REQUEST, CH_BREAKTHROUGH_CINEMATIC,
    CH_BREAKTHROUGH_EVENT, CH_COMBAT_REALTIME, CH_COMBAT_SUMMARY, CH_CULTIVATION_DEATH,
    CH_DEATH_CINEMATIC, CH_DEATH_INSIGHT, CH_DUGU_ANTIDOTE_RESULT, CH_DUGU_POISON_PROGRESS,
    CH_DUGU_V2_CAST, CH_DUGU_V2_REVERSE, CH_DUGU_V2_SELF_CURE, CH_DUO_SHE_EVENT,
    CH_ELDER_ENCOUNTER, CH_FACTION_EVENT, CH_FACTION_STATE, CH_FACTION_WAR, CH_FAUNA_ECOLOGY,
    CH_FORGE_EVENT, CH_FORGE_OUTCOME, CH_FORGE_START, CH_HALFSTEP_RECHALLENGE,
    CH_HEART_DEMON_OFFER, CH_HEART_DEMON_REQUEST, CH_HIGH_RENOWN_MILESTONE, CH_INSIGHT_OFFER,
    CH_INSIGHT_REQUEST, CH_LIFESPAN_EVENT, CH_MERIDIAN_SEVERED, CH_MUTATION_EVENT,
    CH_NAMED_FACTION_STATE, CH_NPC_COMBAT, CH_NPC_DEATH, CH_NPC_RELIC, CH_NPC_SPAWN,
    CH_PLAYER_CHAT, CH_POISON_DOSE_EVENT, CH_POISON_OVERDOSE_EVENT, CH_POI_NOVICE_EVENT,
    CH_PRICE_INDEX, CH_PSEUDO_VEIN_ACTIVE, CH_PSEUDO_VEIN_DISSIPATE, CH_RAT_PHASE_EVENT,
    CH_REBIRTH, CH_SEASON_CHANGED, CH_SKILL_CAP_CHANGED, CH_SKILL_LV_UP, CH_SKILL_SCROLL_USED,
    CH_SKILL_XP_GAIN, CH_SOCIAL_EXPOSURE, CH_SOCIAL_FEUD, CH_SOCIAL_NICHE_INTRUSION,
    CH_SOCIAL_PACT, CH_SOCIAL_RENOWN_DELTA, CH_SPIRIT_EYE_DISCOVERED, CH_SPIRIT_EYE_MIGRATE,
    CH_SPIRIT_EYE_USED_FOR_BREAKTHROUGH, CH_SPIRIT_TREASURE_DIALOGUE,
    CH_SPIRIT_TREASURE_DIALOGUE_REQUEST, CH_STYLE_BALANCE_TELEMETRY,
    CH_TERRITORY_NARRATION_REQUEST, CH_TIANDAO_HUNT_NARRATION_REQUEST, CH_TRIBULATION,
    CH_TRIBULATION_COLLAPSE, CH_TRIBULATION_LOCK, CH_TRIBULATION_OMEN, CH_TRIBULATION_SETTLE,
    CH_TRIBULATION_WAVE, CH_TSY_EVENT, CH_TUIKE_ASH_DECAY, CH_TUIKE_SHED, CH_TUIKE_V2_SKILL_EVENT,
    CH_VOID_ACTION_BARRIER, CH_VOID_ACTION_EXPLODE_ZONE, CH_VOID_ACTION_SUPPRESS_TSY,
    CH_VOID_EROSION_EVENT, CH_WANTED_PLAYER, CH_WEATHER_EVENT_UPDATE, CH_WOLIU_BACKFIRE,
    CH_WOLIU_PROJECTILE_DRAINED, CH_WOLIU_V2_BACKFIRE, CH_WOLIU_V2_CAST, CH_WOLIU_V2_TURBULENCE,
    CH_WORLD_STATE, CH_YIDAO_EVENT, CH_ZHENFA_V2_EVENT, CH_ZHENMAI_SKILL_EVENT,
    CH_ZONE_ENVIRONMENT_UPDATE, CH_ZONE_PRESSURE_CROSSED, CH_ZONG_CORE_ACTIVATED,
    ELDER_ENCOUNTER_DURABLE_REDIS_KEY, QI_LEDGER_REDIS_KEY,
};
use crate::schema::chat_message::ChatMessageV1;
use crate::schema::combat_carrier::{
    CarrierAbrasionEventV1, CarrierChargedEventV1, CarrierImpactEventV1, ContainerSwapEventV1,
    EchoFractalEventV1, MultiShotEventV1, ProjectileDespawnedEventV1, QiInjectionEventV1,
};
use crate::schema::combat_event::{CombatRealtimeEventV1, CombatSummaryV1};
use crate::schema::common::{MAX_COMMANDS_PER_TICK, MAX_NARRATION_LENGTH};
use crate::schema::cultivation::{
    BreakthroughCinematicEventV1, BreakthroughEventV1, CultivationDeathV1, ForgeEventV1,
    HeartDemonPregenRequestV1, InsightOfferV1, InsightRequestV1,
};
use crate::schema::dandao::MutationEventV1;
use crate::schema::death_cinematic::DeathCinematicS2cV1;
use crate::schema::death_insight::DeathInsightRequestV1;
use crate::schema::death_lifecycle::{
    AgingEventV1, DuoSheEventV1, LifespanEventV1, RebirthEventV1,
};
use crate::schema::dugu::{AntidoteResultEventV1, DuguPoisonProgressEventV1};
use crate::schema::dugu_v2::{DuguReverseTriggeredV1, DuguSelfCureProgressV1, DuguV2SkillCastV1};
use crate::schema::economy::{BoneCoinTickV1, PriceIndexV1};
use crate::schema::elder_encounter::ElderEncounterEventV1;
use crate::schema::fauna_ecology::FaunaEcologySnapshotV1;
use crate::schema::forge_bridge::{ForgeOutcomePayloadV1, ForgeStartPayloadV1};
use crate::schema::identity::WantedPlayerEventV1;
use crate::schema::lingtian_weather::WeatherEventUpdateV1;
use crate::schema::meridian_severed::MeridianSeveredEventV1;
use crate::schema::narration::NarrationV1;
use crate::schema::npc::{
    DormantCombatOutcomeV1, FactionEventV1, FactionStateV1, FactionWarEventV1, NamedFactionStateV1,
    NpcDeathV1, NpcSpawnedV1, PendingDormantRelicV1,
};
use crate::schema::poi_novice::{PoiSpawnedEventV1, TrespassEventV1};
use crate::schema::poison_trait::{PoisonDoseEventV1, PoisonOverdoseEventV1};
use crate::schema::pseudo_vein::{PseudoVeinDissipateEventV1, PseudoVeinSnapshotV1};
use crate::schema::season::SeasonChangedV1;
use crate::schema::server_data::HeartDemonOfferV1;
use crate::schema::skill::{
    SkillCapChangedPayloadV1, SkillLvUpPayloadV1, SkillScrollUsedPayloadV1, SkillXpGainPayloadV1,
};
use crate::schema::social::{
    HighRenownMilestoneEventV1, NicheGuardianBrokenV1, NicheGuardianFatigueV1,
    NicheIntrusionEventV1, SocialExposureEventV1, SocialFeudEventV1, SocialPactEventV1,
    SocialRenownDeltaV1,
};
use crate::schema::spirit_eye::{
    SpiritEyeDiscoveredV1, SpiritEyeMigrateV1, SpiritEyeUsedForBreakthroughV1,
};
use crate::schema::spirit_treasure::{SpiritTreasureDialogueRequestV1, SpiritTreasureDialogueV1};
use crate::schema::style_balance::StyleBalanceTelemetryEventV1;
use crate::schema::territory_narration::TerritoryDominanceNarrationRequestV1;
use crate::schema::tiandao_hunt_narration::TiandaoHuntNarrationRequestV1;
use crate::schema::tribulation::{TribulationEventV1, TribulationKindV1, TribulationPhaseV1};
use crate::schema::tsy::{TsyEnterEventV1, TsyExitEventV1, TsyZoneActivatedEventV1};
use crate::schema::tsy_hostile::{TsyNpcSpawnedV1, TsySentinelPhaseChangedV1};
use crate::schema::tuike::ShedEventV1;
use crate::schema::tuike_v2::{TuikeAshDecayV1, TuikeSkillEventV1};
use crate::schema::void_actions::VoidActionBroadcastV1;
use crate::schema::woliu::{ProjectileQiDrainedEventV1, VortexBackfireEventV1};
use crate::schema::woliu_erosion::VoidErosionEventV1;
use crate::schema::woliu_v2::{TurbulenceFieldV1, WoliuBackfireV1, WoliuSkillCastV1};
use crate::schema::world_state::WorldStateV1;
use crate::schema::yidao::YidaoEventV1;
use crate::schema::zhenfa_v2::ZhenfaV2EventV1;
use crate::schema::zhenmai_v2::ZhenmaiSkillEventV1;
use crate::schema::zone_environment::ZoneEnvironmentStateV1;
use crate::schema::zone_pressure::ZonePressureCrossedV1;
use crate::schema::zong_formation::ZongCoreActivationV1;

const BRIDGE_LOOP_INTERVAL: Duration = Duration::from_millis(25);
const REDIS_IO_TIMEOUT: Duration = Duration::from_millis(100);
const REDIS_WORLD_STATE_PUBLISH_TIMEOUT: Duration = Duration::from_secs(3);
const REDIS_HASH_REPLACE_TIMEOUT: Duration = Duration::from_secs(3);
const RECONNECT_BACKOFF_INITIAL: Duration = Duration::from_millis(250);
const RECONNECT_BACKOFF_MAX: Duration = Duration::from_secs(5);
const OUTBOUND_DRAIN_BUDGET: usize = 16;
const CHAT_MESSAGE_MAX_LENGTH: usize = 256;
/// Application-side cap for `bong:player_chat`: 32 agent drain windows of 128
/// messages leave room for a short agent restart while bounding Redis memory.
/// `LTRIM` keeps this queue's newest entries, so overflow drops the oldest chat.
const PLAYER_CHAT_QUEUE_MAX_LEN: i64 = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedisDeliveryReceipt {
    pub delivery_id: String,
    pub outcome: Result<(), String>,
}

#[derive(Debug, Clone)]
pub enum RedisInbound {
    AgentCommand(AgentCommandV1),
    AgentNarration(NarrationV1),
    AgentWorldModel(AgentWorldModelEnvelopeV1),
    InsightOffer(InsightOfferV1),
    HeartDemonOffer(HeartDemonOfferV1),
    SpiritTreasureDialogue(SpiritTreasureDialogueV1),
    // ─── plan-agent-ui-data-v1 P0 ───────────────────────────────────
    /// 天道 Agent 发布的 UI 面板指令（bong:agent_ui_cmd）。
    AgentUiCmd(AgentUiRequestCommandV1),
}

#[derive(Debug, Clone)]
pub enum RedisOutbound {
    WorldState(WorldStateV1),
    NpcDormantHash {
        entries: Vec<(String, String)>,
        revision: u64,
        receipt_tx: Sender<RedisDeliveryReceipt>,
    },
    /// plan-offscreen-war-v1 P0：守恒 telemetry HASH（`bong:qi/ledger`）。
    QiLedgerHash(Vec<(String, String)>),
    SeasonChanged(SeasonChangedV1),
    BoneCoinTick(BoneCoinTickV1),
    PriceIndex(PriceIndexV1),
    #[allow(dead_code)]
    PlayerChat(ChatMessageV1),
    CombatRealtime(CombatRealtimeEventV1),
    CombatSummary(CombatSummaryV1),
    AntiCheatReport(AntiCheatReportV1),
    ArmorDurabilityChanged(ArmorDurabilityChangedV1),
    #[allow(dead_code)]
    PseudoVeinSnapshot(PseudoVeinSnapshotV1),
    #[allow(dead_code)]
    PseudoVeinDissipate(PseudoVeinDissipateEventV1),
    #[allow(dead_code)]
    ZongCoreActivated(ZongCoreActivationV1),
    BreakthroughEvent(BreakthroughEventV1),
    BreakthroughCinematic(BreakthroughCinematicEventV1),
    ForgeEvent(ForgeEventV1),
    ForgeStart(ForgeStartPayloadV1),
    ForgeOutcome(ForgeOutcomePayloadV1),
    AlchemySessionStart(AlchemySessionStartV1),
    AlchemySessionEnd(AlchemySessionEndV1),
    AlchemyInterventionResult(AlchemyInterventionResultV1),
    AlchemyInsight(AlchemyInsightV1),
    CultivationDeath(CultivationDeathV1),
    InsightRequest(InsightRequestV1),
    TiandaoHuntNarrationRequest(TiandaoHuntNarrationRequestV1),
    HeartDemonRequest(HeartDemonPregenRequestV1),
    SpiritTreasureDialogueRequest(SpiritTreasureDialogueRequestV1),
    DeathInsight(DeathInsightRequestV1),
    DeathCinematic(DeathCinematicS2cV1),
    Aging(AgingEventV1),
    LifespanEvent(LifespanEventV1),
    DuoSheEvent(DuoSheEventV1),
    TribulationEvent(TribulationEventV1),
    Rebirth(RebirthEventV1),
    SkillXpGain(SkillXpGainPayloadV1),
    SkillLvUp(SkillLvUpPayloadV1),
    SkillCapChanged(SkillCapChangedPayloadV1),
    SkillScrollUsed(SkillScrollUsedPayloadV1),
    NpcSpawned(NpcSpawnedV1),
    NpcDeath(NpcDeathV1),
    /// plan-offscreen-war-v1 P2：离屏 dormant 互殴战果 telemetry（`bong:npc/combat`）。
    DormantCombatOutcome(DormantCombatOutcomeV1),
    /// plan-offscreen-war-v1 P3：克制式战场遗物创建 telemetry（`bong:npc/relic`，零真元）。
    PendingDormantRelic(PendingDormantRelicV1),
    FactionEvent(FactionEventV1),
    /// plan-offscreen-war-v1 P5：散修群体消长盘面 telemetry（`bong:faction_state`，纯观测、零真元）。
    FactionState(FactionStateV1),
    /// plan-faction-expansion-v1 P3：具名势力状态快照（`bong:named_faction_state`）。
    NamedFactionState(NamedFactionStateV1),
    /// plan-offscreen-war-v1 P6：涌现冲突生命周期 telemetry（`bong:faction/war`，纯观测、零真元）。
    FactionWar(FactionWarEventV1),
    ZonePressureCrossed(ZonePressureCrossedV1),
    RatPhaseEvent(RatPhaseChangeEvent),
    BotanyEcology(BotanyEcologySnapshotV1),
    FaunaEcology(FaunaEcologySnapshotV1),
    TsyEnter(TsyEnterEventV1),
    TsyExit(TsyExitEventV1),
    TsyNpcSpawned(TsyNpcSpawnedV1),
    TsySentinelPhaseChanged(TsySentinelPhaseChangedV1),
    /// plan-agent-ui-data-v1 server fix — 坍缩渊首次激活（bong:tsy_event kind=tsy_zone_activated）。
    /// agent 消费此事件触发秘境发现 UI 面板（drainTsyZoneActivatedEvents）。
    TsyZoneActivated(TsyZoneActivatedEventV1),
    PoiSpawned(PoiSpawnedEventV1),
    PoiTrespass(TrespassEventV1),
    SocialExposure(SocialExposureEventV1),
    SocialPact(SocialPactEventV1),
    SocialFeud(SocialFeudEventV1),
    SocialRenownDelta(SocialRenownDeltaV1),
    NicheIntrusion(NicheIntrusionEventV1),
    HighRenownMilestone(HighRenownMilestoneEventV1),
    NicheGuardianFatigue(NicheGuardianFatigueV1),
    NicheGuardianBroken(NicheGuardianBrokenV1),
    SpiritEyeMigrate(SpiritEyeMigrateV1),
    SpiritEyeDiscovered(SpiritEyeDiscoveredV1),
    SpiritEyeUsedForBreakthrough(SpiritEyeUsedForBreakthroughV1),
    DuguPoisonProgress(DuguPoisonProgressEventV1),
    AntidoteResult(AntidoteResultEventV1),
    PoisonDoseEvent(PoisonDoseEventV1),
    PoisonOverdoseEvent(PoisonOverdoseEventV1),
    DuguV2Cast(DuguV2SkillCastV1),
    DuguV2SelfCure(DuguSelfCureProgressV1),
    DuguV2Reverse(DuguReverseTriggeredV1),
    VortexBackfire(VortexBackfireEventV1),
    ProjectileQiDrained(ProjectileQiDrainedEventV1),
    WoliuV2Cast(WoliuSkillCastV1),
    WoliuV2Backfire(WoliuBackfireV1),
    WoliuV2Turbulence(TurbulenceFieldV1),
    ZhenfaV2Event(ZhenfaV2EventV1),
    ZhenmaiSkillEvent(ZhenmaiSkillEventV1),
    BaomaiV3SkillEvent(BaomaiSkillEventV1),
    /// plan-combat-skill-feedback-bridges-v1 P2 — 山震震波事件（bong:baomai_v3/mountain_shake）。
    BaomaiV3MountainShake(BaomaiV3MountainShakeV1),
    /// plan-combat-skill-feedback-bridges-v1 P2 — 血燃HP→真元事件（bong:baomai_v3/blood_burn）。
    BaomaiV3BloodBurn(BaomaiV3BloodBurnV1),
    /// plan-combat-skill-feedback-bridges-v1 P2 — 超越到期事件（bong:baomai_v3/transcendence_expired）。
    BaomaiV3TranscendenceExpired(BaomaiV3TranscendenceExpiredV1),
    /// plan-combat-skill-feedback-bridges-v1 P2 — 过载涟漪事件（bong:baomai_v3/overload_ripple）。
    BaomaiV3OverloadRipple(BaomaiV3OverloadRippleV1),
    CarrierCharged(CarrierChargedEventV1),
    CarrierImpact(CarrierImpactEventV1),
    ProjectileDespawned(ProjectileDespawnedEventV1),
    AnqiMultiShot(MultiShotEventV1),
    AnqiQiInjection(QiInjectionEventV1),
    AnqiEchoFractal(EchoFractalEventV1),
    AnqiCarrierAbrasion(CarrierAbrasionEventV1),
    AnqiContainerSwap(ContainerSwapEventV1),
    TuikeShed(ShedEventV1),
    TuikeV2SkillEvent(TuikeSkillEventV1),
    /// plan-combat-skill-feedback-bridges-v1 P6 — 蜕壳灰烬入包叙事事件（bong:tuike_v2/ash_decay）。
    TuikeAshDecay(TuikeAshDecayV1),
    YidaoEvent(YidaoEventV1),
    StyleBalanceTelemetry(StyleBalanceTelemetryEventV1),
    WantedPlayer(WantedPlayerEventV1),
    /// plan-lingtian-weather-v1 §3 / §4.4 — 天气事件起 / 落
    #[allow(dead_code)]
    WeatherEventUpdate(WeatherEventUpdateV1),
    ZoneEnvironmentUpdate(ZoneEnvironmentStateV1),
    /// plan-craft-v1 P3 — 通用手搓出炉结果（成功 / 失败），agent narration 出炉叙事 trigger
    CraftOutcome(crate::schema::craft::CraftOutcomeV1),
    /// plan-craft-v1 P3 — 三渠道解锁广播，agent narration 首学/师承/顿悟 trigger
    RecipeUnlocked(crate::schema::craft::RecipeUnlockedV1),
    /// plan-void-actions-v1 — 化虚三类世界级 action 公告。
    VoidAction(VoidActionBroadcastV1),
    /// plan-dandao-runtime-wiring-v1 P2 — 变异阶段推进叙事事件（bong:mutation_event）。
    MutationEvent(MutationEventV1),
    /// plan-combat-skill-feedback-bridges-v1 P0 — 经脉永久 SEVERED 叙事事件（bong:meridian_severed）。
    MeridianSevered(MeridianSeveredEventV1),
    /// plan-combat-skill-feedback-bridges-v1 P1 — 疤纹回路形成（bong:baomai_v4/scar_circuit_formed）。
    BaomaiV4ScarCircuitFormed(BaomaiV4ScarCircuitFormedV1),
    /// plan-combat-skill-feedback-bridges-v1 P1 — 疤纹回路断裂（bong:baomai_v4/scar_circuit_broken）。
    BaomaiV4ScarCircuitBroken(BaomaiV4ScarCircuitBrokenV1),
    /// plan-combat-skill-feedback-bridges-v1 P1 — 活茧阶段提升（bong:baomai_v4/iron_cocoon_stage_up）。
    BaomaiV4IronCocoonStageUp(BaomaiV4IronCocoonStageUpV1),
    /// plan-combat-skill-feedback-bridges-v1 P1 — 共振锁定开始（bong:baomai_v4/resonance_lock）。
    BaomaiV4ResonanceLock(BaomaiV4ResonanceLockV1),
    /// plan-combat-skill-feedback-bridges-v1 P1 — 共振锁定结束（bong:baomai_v4/resonance_lock_end）。
    BaomaiV4ResonanceLockEnd(BaomaiV4ResonanceLockEndV1),
    /// plan-combat-skill-feedback-bridges-v1 P3 — 虚蚀阶段推进叙事事件（bong:void_erosion_event）。
    VoidErosionEvent(VoidErosionEventV1),
    /// Durable terminal narration source entry. RPUSH is acknowledged only after Redis stores it.
    ElderEncounterTerminal {
        delivery_id: String,
        event: ElderEncounterEventV1,
        receipt_tx: Sender<RedisDeliveryReceipt>,
    },
    /// plan-dying-elder-v1 P3 — 垂死大能遭遇事件（bong:elder_encounter）。
    ElderEncounterEvent(ElderEncounterEventV1),
    /// plan-territory-v1 P3 — 领地霸主变动叙事请求（bong:territory_narration_request）。
    /// 三时刻：新霸主确立 / 霸主被驱逐 / 区域灵气耗尽。匿名（境界段，非 char_id）。
    TerritoryDominanceNarration(TerritoryDominanceNarrationRequestV1),
    // ─── plan-agent-ui-data-v1 P0 ───────────────────────────────────
    /// 玩家天道 UI 面板交互响应（bong:agent_ui_response）。
    AgentUiResponse(AgentUiResponsePayloadV1),
    BotDeliveryFenceAck {
        token: String,
    },
    // ─── plan-halfstep-rechallenge-integration-v1 P1 ────────────────
    /// 半步化虚重渡触发（bong:tribulation/halfstep_rechallenge），agent narration 用。
    HalfStepRechallengeTrigger(
        crate::schema::halfstep_rechallenge::HalfStepRechallengeTriggerPayloadV1,
    ),
}

#[derive(Debug, Clone)]
enum RedisIoCommand {
    ListPushWithReceipt {
        key: &'static str,
        payload: String,
        delivery_id: String,
        receipt_tx: Sender<RedisDeliveryReceipt>,
    },
    Publish {
        channel: &'static str,
        payload: String,
    },
    PublishFanout {
        channels: Vec<&'static str>,
        payload: String,
    },
    ListPush {
        key: &'static str,
        payload: String,
    },
    HashReplaceWithReceipt {
        key: &'static str,
        entries: Vec<(String, String)>,
        delivery_id: String,
        receipt_tx: Sender<RedisDeliveryReceipt>,
    },
    HashReplace {
        key: &'static str,
        entries: Vec<(String, String)>,
    },
}

#[derive(Debug, PartialEq, Eq)]
enum DrainOutcome {
    Healthy,
    Reconnect { reason: String },
    Stop,
}

#[derive(Debug, PartialEq, Eq)]
enum BridgeLoopControl {
    Reconnect { reason: String },
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubscriberTaskExit {
    StreamEnded,
    GameChannelClosed,
}

#[derive(Debug, Clone)]
struct BotDeliveryFenceRequest {
    token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReconnectSchedule {
    attempt: u32,
    delay: Duration,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ReconnectBackoff {
    attempts: u32,
}

impl ReconnectBackoff {
    fn next(&mut self) -> ReconnectSchedule {
        self.attempts = self.attempts.saturating_add(1);

        let shift = self.attempts.saturating_sub(1).min(16);
        let delay_ms = (RECONNECT_BACKOFF_INITIAL.as_millis() as u64)
            .saturating_mul(1u64 << shift)
            .min(RECONNECT_BACKOFF_MAX.as_millis() as u64);

        ReconnectSchedule {
            attempt: self.attempts,
            delay: Duration::from_millis(delay_ms),
        }
    }

    fn reset(&mut self) {
        self.attempts = 0;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidationError(String);

impl ValidationError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Narrow test seam for exercising the production Redis encoder without
/// exposing the bridge's general-purpose I/O command enum.
#[cfg(test)]
pub(super) fn encode_agent_ui_response_wire_for_test(
    response: &AgentUiResponsePayloadV1,
) -> Result<(&'static str, String), String> {
    match prepare_outbound_command(RedisOutbound::AgentUiResponse(response.clone()))
        .map_err(|error| error.to_string())?
    {
        RedisIoCommand::Publish { channel, payload } => Ok((channel, payload)),
        other => Err(format!(
            "AgentUiResponse must encode as a Redis publish command, got {other:?}"
        )),
    }
}

/// Narrow test seam for exercising the production narration decoder on the
/// exact Redis channel emitted by Tiandao's `UiResponseConsumer`.
#[cfg(test)]
pub(super) fn parse_agent_narration_wire_for_test(payload: &str) -> Result<NarrationV1, String> {
    match parse_inbound_message(CH_AGENT_NARRATE, payload).map_err(|error| error.to_string())? {
        Some(RedisInbound::AgentNarration(narration)) => Ok(narration),
        Some(other) => Err(format!(
            "{CH_AGENT_NARRATE} must decode as AgentNarration, got {other:?}"
        )),
        None => Err(format!(
            "{CH_AGENT_NARRATE} unexpectedly decoded as an ignored channel"
        )),
    }
}

pub fn spawn_redis_bridge(
    redis_url: &str,
) -> (
    std::thread::JoinHandle<()>,
    Sender<RedisOutbound>,
    Receiver<RedisInbound>,
) {
    let (tx_to_game, rx_inbound) = crossbeam_channel::unbounded::<RedisInbound>();
    let (tx_outbound, rx_from_game) = crossbeam_channel::unbounded::<RedisOutbound>();
    let (tx_fence, rx_fence) = crossbeam_channel::unbounded::<BotDeliveryFenceRequest>();

    let url = redis_url.to_string();

    let handle = std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(error) => {
                tracing::error!("[bong][redis] failed to create tokio runtime: {error}");
                return;
            }
        };

        rt.block_on(async move {
            let client = match redis::Client::open(url.as_str()) {
                Ok(client) => client,
                Err(error) => {
                    tracing::error!("[bong][redis] failed to open client: {error}");
                    return;
                }
            };
            let mut backoff = ReconnectBackoff::default();
            let mut pending_command = None;

            loop {
                match connect_bridge_session(&client, url.as_str(), &tx_to_game, &tx_fence).await {
                    Ok((pub_conn, sub_task)) => {
                        backoff.reset();

                        match run_bridge_session(
                            &rx_from_game,
                            &rx_fence,
                            &mut pending_command,
                            pub_conn,
                            sub_task,
                        )
                        .await
                        {
                            BridgeLoopControl::Reconnect { reason } => {
                                sleep_before_reconnect(url.as_str(), &mut backoff, reason.as_str())
                                    .await;
                            }
                            BridgeLoopControl::Stop => break,
                        }
                    }
                    Err(error) => {
                        tracing::warn!("[bong][redis] {error}");
                        sleep_before_reconnect(url.as_str(), &mut backoff, error.as_str()).await;
                    }
                }
            }
        });
    });

    (handle, tx_outbound, rx_inbound)
}

async fn drain_outbound_messages(
    rx_from_game: &Receiver<RedisOutbound>,
    pub_conn: &mut redis::aio::MultiplexedConnection,
    pending_command: &mut Option<RedisIoCommand>,
) -> DrainOutcome {
    let mut drained = 0;

    if let Some(command) = pending_command.take() {
        if let Err((error, command)) = dispatch_outbound_command(pub_conn, command).await {
            *pending_command = Some(command);
            return DrainOutcome::Reconnect {
                reason: format!("outbound_retry_failed: {error}"),
            };
        }
    }

    while drained < OUTBOUND_DRAIN_BUDGET {
        match rx_from_game.try_recv() {
            Ok(message) => {
                drained += 1;

                match prepare_outbound_command(message) {
                    Ok(command) => {
                        if let Err((error, command)) =
                            dispatch_outbound_command(pub_conn, command).await
                        {
                            *pending_command = Some(command);
                            return DrainOutcome::Reconnect {
                                reason: format!("outbound_failed: {error}"),
                            };
                        }
                    }
                    Err(error) => {
                        tracing::warn!("[bong][redis] dropped invalid outbound payload: {error}");
                    }
                }
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                tracing::warn!("[bong][redis] outbound channel from game closed; stopping bridge");
                return DrainOutcome::Stop;
            }
        }
    }

    if drained == OUTBOUND_DRAIN_BUDGET {
        tracing::debug!(
            "[bong][redis] outbound drain hit budget {OUTBOUND_DRAIN_BUDGET}; remaining messages will flush next cycle"
        );
    }

    DrainOutcome::Healthy
}

fn prepare_outbound_command(message: RedisOutbound) -> Result<RedisIoCommand, ValidationError> {
    match message {
        RedisOutbound::BotDeliveryFenceAck { token } => {
            let payload = serde_json::json!({"v": 1, "ok": true, "token": token});
            Ok(RedisIoCommand::Publish {
                channel: CH_BOT_DELIVERY_FENCE_ACK,
                payload: payload.to_string(),
            })
        }
        RedisOutbound::WorldState(state) => {
            validate_world_state(&state)?;

            let payload = serde_json::to_string(&state).map_err(|error| {
                ValidationError::new(format!("failed to serialize WorldStateV1: {error}"))
            })?;

            Ok(RedisIoCommand::Publish {
                channel: CH_WORLD_STATE,
                payload,
            })
        }
        RedisOutbound::NpcDormantHash {
            entries,
            revision,
            receipt_tx,
        } => Ok(RedisIoCommand::HashReplaceWithReceipt {
            key: NPC_DORMANT_REDIS_KEY,
            entries,
            delivery_id: revision.to_string(),
            receipt_tx,
        }),
        RedisOutbound::QiLedgerHash(entries) => Ok(RedisIoCommand::HashReplace {
            key: QI_LEDGER_REDIS_KEY,
            entries,
        }),
        RedisOutbound::SeasonChanged(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize SeasonChangedV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SEASON_CHANGED,
                payload,
            })
        }
        RedisOutbound::BoneCoinTick(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize BoneCoinTickV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_BONE_COIN_TICK,
                payload,
            })
        }
        RedisOutbound::PriceIndex(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize PriceIndexV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_PRICE_INDEX,
                payload,
            })
        }
        RedisOutbound::PlayerChat(chat) => {
            validate_chat_message(&chat)?;

            let payload = serde_json::to_string(&chat).map_err(|error| {
                ValidationError::new(format!("failed to serialize ChatMessageV1: {error}"))
            })?;

            Ok(RedisIoCommand::ListPush {
                key: CH_PLAYER_CHAT,
                payload,
            })
        }
        RedisOutbound::CombatRealtime(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize CombatRealtimeEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_COMBAT_REALTIME,
                payload,
            })
        }
        RedisOutbound::CombatSummary(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize CombatSummaryV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_COMBAT_SUMMARY,
                payload,
            })
        }
        RedisOutbound::StyleBalanceTelemetry(evt) => {
            evt.validate().map_err(ValidationError::new)?;
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize StyleBalanceTelemetryEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_STYLE_BALANCE_TELEMETRY,
                payload,
            })
        }
        RedisOutbound::WantedPlayer(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize WantedPlayerEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_WANTED_PLAYER,
                payload,
            })
        }
        RedisOutbound::YidaoEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize YidaoEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_YIDAO_EVENT,
                payload,
            })
        }
        RedisOutbound::AntiCheatReport(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize AntiCheatReportV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ANTICHEAT,
                payload,
            })
        }
        RedisOutbound::ArmorDurabilityChanged(evt) => {
            evt.validate().map_err(ValidationError::new)?;
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize ArmorDurabilityChangedV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ARMOR_DURABILITY_CHANGED,
                payload,
            })
        }
        RedisOutbound::PseudoVeinSnapshot(evt) => {
            evt.validate().map_err(ValidationError::new)?;
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize PseudoVeinSnapshotV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_PSEUDO_VEIN_ACTIVE,
                payload,
            })
        }
        RedisOutbound::WeatherEventUpdate(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize WeatherEventUpdateV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_WEATHER_EVENT_UPDATE,
                payload,
            })
        }
        RedisOutbound::ZoneEnvironmentUpdate(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize ZoneEnvironmentStateV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ZONE_ENVIRONMENT_UPDATE,
                payload,
            })
        }
        RedisOutbound::CraftOutcome(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize CraftOutcomeV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: crate::schema::channels::CH_CRAFT_OUTCOME,
                payload,
            })
        }
        RedisOutbound::RecipeUnlocked(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize RecipeUnlockedV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: crate::schema::channels::CH_CRAFT_RECIPE_UNLOCKED,
                payload,
            })
        }
        RedisOutbound::PseudoVeinDissipate(evt) => {
            evt.validate().map_err(ValidationError::new)?;
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize PseudoVeinDissipateEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_PSEUDO_VEIN_DISSIPATE,
                payload,
            })
        }
        RedisOutbound::ZongCoreActivated(evt) => {
            evt.validate().map_err(ValidationError::new)?;
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize ZongCoreActivationV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ZONG_CORE_ACTIVATED,
                payload,
            })
        }
        RedisOutbound::BreakthroughEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize BreakthroughEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_BREAKTHROUGH_EVENT,
                payload,
            })
        }
        RedisOutbound::BreakthroughCinematic(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize BreakthroughCinematicEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_BREAKTHROUGH_CINEMATIC,
                payload,
            })
        }
        RedisOutbound::ForgeEvent(evt) => {
            evt.validate().map_err(ValidationError::new)?;
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize ForgeEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_FORGE_EVENT,
                payload,
            })
        }
        RedisOutbound::ForgeStart(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize ForgeStartPayloadV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_FORGE_START,
                payload,
            })
        }
        RedisOutbound::ForgeOutcome(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize ForgeOutcomePayloadV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_FORGE_OUTCOME,
                payload,
            })
        }
        RedisOutbound::AlchemySessionStart(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize AlchemySessionStartV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ALCHEMY_SESSION_START,
                payload,
            })
        }
        RedisOutbound::AlchemySessionEnd(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize AlchemySessionEndV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ALCHEMY_SESSION_END,
                payload,
            })
        }
        RedisOutbound::AlchemyInterventionResult(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize AlchemyInterventionResultV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ALCHEMY_INTERVENTION_RESULT,
                payload,
            })
        }
        RedisOutbound::AlchemyInsight(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize AlchemyInsightV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ALCHEMY_INSIGHT,
                payload,
            })
        }
        RedisOutbound::CultivationDeath(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize CultivationDeathV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_CULTIVATION_DEATH,
                payload,
            })
        }
        RedisOutbound::InsightRequest(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize InsightRequestV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_INSIGHT_REQUEST,
                payload,
            })
        }
        RedisOutbound::TiandaoHuntNarrationRequest(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize TiandaoHuntNarrationRequestV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_TIANDAO_HUNT_NARRATION_REQUEST,
                payload,
            })
        }
        RedisOutbound::HeartDemonRequest(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize HeartDemonPregenRequestV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_HEART_DEMON_REQUEST,
                payload,
            })
        }
        RedisOutbound::SpiritTreasureDialogueRequest(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize SpiritTreasureDialogueRequestV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SPIRIT_TREASURE_DIALOGUE_REQUEST,
                payload,
            })
        }
        RedisOutbound::DeathInsight(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize DeathInsightRequestV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_DEATH_INSIGHT,
                payload,
            })
        }
        RedisOutbound::DeathCinematic(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize DeathCinematicS2cV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_DEATH_CINEMATIC,
                payload,
            })
        }
        RedisOutbound::Aging(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize AgingEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_AGING,
                payload,
            })
        }
        RedisOutbound::LifespanEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize LifespanEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_LIFESPAN_EVENT,
                payload,
            })
        }
        RedisOutbound::DuoSheEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize DuoSheEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_DUO_SHE_EVENT,
                payload,
            })
        }
        RedisOutbound::TribulationEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize TribulationEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::PublishFanout {
                channels: tribulation_fanout_channels(&evt),
                payload,
            })
        }
        RedisOutbound::VoidAction(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize VoidActionBroadcastV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::PublishFanout {
                channels: void_action_fanout_channels(&evt),
                payload,
            })
        }
        RedisOutbound::Rebirth(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize RebirthEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_REBIRTH,
                payload,
            })
        }
        RedisOutbound::SkillXpGain(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize SkillXpGainPayloadV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SKILL_XP_GAIN,
                payload,
            })
        }
        RedisOutbound::SkillLvUp(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize SkillLvUpPayloadV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SKILL_LV_UP,
                payload,
            })
        }
        RedisOutbound::SkillCapChanged(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize SkillCapChangedPayloadV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SKILL_CAP_CHANGED,
                payload,
            })
        }
        RedisOutbound::SkillScrollUsed(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize SkillScrollUsedPayloadV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SKILL_SCROLL_USED,
                payload,
            })
        }
        RedisOutbound::NpcSpawned(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize NpcSpawnedV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_NPC_SPAWN,
                payload,
            })
        }
        RedisOutbound::NpcDeath(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize NpcDeathV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_NPC_DEATH,
                payload,
            })
        }
        RedisOutbound::DormantCombatOutcome(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize DormantCombatOutcomeV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_NPC_COMBAT,
                payload,
            })
        }
        RedisOutbound::PendingDormantRelic(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize PendingDormantRelicV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_NPC_RELIC,
                payload,
            })
        }
        RedisOutbound::FactionEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize FactionEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_FACTION_EVENT,
                payload,
            })
        }
        RedisOutbound::FactionState(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize FactionStateV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_FACTION_STATE,
                payload,
            })
        }
        RedisOutbound::NamedFactionState(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize NamedFactionStateV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_NAMED_FACTION_STATE,
                payload,
            })
        }
        RedisOutbound::FactionWar(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize FactionWarEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_FACTION_WAR,
                payload,
            })
        }
        RedisOutbound::ZonePressureCrossed(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize ZonePressureCrossedV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ZONE_PRESSURE_CROSSED,
                payload,
            })
        }
        RedisOutbound::RatPhaseEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize RatPhaseChangeEvent: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_RAT_PHASE_EVENT,
                payload,
            })
        }
        RedisOutbound::BotanyEcology(snapshot) => {
            let payload = serde_json::to_string(&snapshot).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize BotanyEcologySnapshotV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_BOTANY_ECOLOGY,
                payload,
            })
        }
        RedisOutbound::FaunaEcology(snapshot) => {
            let payload = serde_json::to_string(&snapshot).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize FaunaEcologySnapshotV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_FAUNA_ECOLOGY,
                payload,
            })
        }
        RedisOutbound::TsyEnter(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize TsyEnterEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_TSY_EVENT,
                payload,
            })
        }
        RedisOutbound::TsyExit(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize TsyExitEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_TSY_EVENT,
                payload,
            })
        }
        RedisOutbound::TsyNpcSpawned(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize TsyNpcSpawnedV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_TSY_EVENT,
                payload,
            })
        }
        RedisOutbound::TsySentinelPhaseChanged(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize TsySentinelPhaseChangedV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_TSY_EVENT,
                payload,
            })
        }
        RedisOutbound::TsyZoneActivated(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize TsyZoneActivatedEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_TSY_EVENT,
                payload,
            })
        }
        RedisOutbound::PoiSpawned(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize PoiSpawnedEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_POI_NOVICE_EVENT,
                payload,
            })
        }
        RedisOutbound::PoiTrespass(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize TrespassEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_POI_NOVICE_EVENT,
                payload,
            })
        }
        RedisOutbound::SocialExposure(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize SocialExposureEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SOCIAL_EXPOSURE,
                payload,
            })
        }
        RedisOutbound::SocialPact(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize SocialPactEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SOCIAL_PACT,
                payload,
            })
        }
        RedisOutbound::SocialFeud(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize SocialFeudEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SOCIAL_FEUD,
                payload,
            })
        }
        RedisOutbound::SocialRenownDelta(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize SocialRenownDeltaV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SOCIAL_RENOWN_DELTA,
                payload,
            })
        }
        RedisOutbound::NicheIntrusion(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize NicheIntrusionEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SOCIAL_NICHE_INTRUSION,
                payload,
            })
        }
        RedisOutbound::HighRenownMilestone(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize HighRenownMilestoneEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_HIGH_RENOWN_MILESTONE,
                payload,
            })
        }
        RedisOutbound::NicheGuardianFatigue(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize NicheGuardianFatigueV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SOCIAL_NICHE_INTRUSION,
                payload,
            })
        }
        RedisOutbound::NicheGuardianBroken(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize NicheGuardianBrokenV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SOCIAL_NICHE_INTRUSION,
                payload,
            })
        }
        RedisOutbound::SpiritEyeMigrate(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize SpiritEyeMigrateV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SPIRIT_EYE_MIGRATE,
                payload,
            })
        }
        RedisOutbound::SpiritEyeDiscovered(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize SpiritEyeDiscoveredV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SPIRIT_EYE_DISCOVERED,
                payload,
            })
        }
        RedisOutbound::SpiritEyeUsedForBreakthrough(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize SpiritEyeUsedForBreakthroughV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SPIRIT_EYE_USED_FOR_BREAKTHROUGH,
                payload,
            })
        }
        RedisOutbound::VortexBackfire(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize VortexBackfireEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_WOLIU_BACKFIRE,
                payload,
            })
        }
        RedisOutbound::ProjectileQiDrained(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize ProjectileQiDrainedEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_WOLIU_PROJECTILE_DRAINED,
                payload,
            })
        }
        RedisOutbound::ZhenmaiSkillEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize ZhenmaiSkillEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ZHENMAI_SKILL_EVENT,
                payload,
            })
        }
        RedisOutbound::BaomaiV3SkillEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize BaomaiSkillEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_BAOMAI_V3_SKILL_EVENT,
                payload,
            })
        }
        // plan-combat-skill-feedback-bridges-v1 P2 — 爆脉 v3 残余事件桥
        RedisOutbound::BaomaiV3MountainShake(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize BaomaiV3MountainShakeV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_BAOMAI_V3_MOUNTAIN_SHAKE,
                payload,
            })
        }
        RedisOutbound::BaomaiV3BloodBurn(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize BaomaiV3BloodBurnV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_BAOMAI_V3_BLOOD_BURN,
                payload,
            })
        }
        RedisOutbound::BaomaiV3TranscendenceExpired(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize BaomaiV3TranscendenceExpiredV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_BAOMAI_V3_TRANSCENDENCE_EXPIRED,
                payload,
            })
        }
        RedisOutbound::BaomaiV3OverloadRipple(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize BaomaiV3OverloadRippleV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_BAOMAI_V3_OVERLOAD_RIPPLE,
                payload,
            })
        }
        RedisOutbound::WoliuV2Cast(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize WoliuSkillCastV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_WOLIU_V2_CAST,
                payload,
            })
        }
        RedisOutbound::WoliuV2Backfire(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize WoliuBackfireV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_WOLIU_V2_BACKFIRE,
                payload,
            })
        }
        RedisOutbound::WoliuV2Turbulence(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize TurbulenceFieldV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_WOLIU_V2_TURBULENCE,
                payload,
            })
        }
        RedisOutbound::ZhenfaV2Event(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize ZhenfaV2EventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ZHENFA_V2_EVENT,
                payload,
            })
        }
        RedisOutbound::DuguPoisonProgress(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize DuguPoisonProgressEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_DUGU_POISON_PROGRESS,
                payload,
            })
        }
        RedisOutbound::AntidoteResult(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize AntidoteResultEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_DUGU_ANTIDOTE_RESULT,
                payload,
            })
        }
        RedisOutbound::PoisonDoseEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize PoisonDoseEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_POISON_DOSE_EVENT,
                payload,
            })
        }
        RedisOutbound::PoisonOverdoseEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize PoisonOverdoseEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_POISON_OVERDOSE_EVENT,
                payload,
            })
        }
        RedisOutbound::DuguV2Cast(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize DuguV2SkillCastV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_DUGU_V2_CAST,
                payload,
            })
        }
        RedisOutbound::DuguV2SelfCure(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize DuguSelfCureProgressV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_DUGU_V2_SELF_CURE,
                payload,
            })
        }
        RedisOutbound::DuguV2Reverse(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize DuguReverseTriggeredV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_DUGU_V2_REVERSE,
                payload,
            })
        }
        RedisOutbound::CarrierCharged(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize CarrierChargedEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ANQI_CARRIER_CHARGED,
                payload,
            })
        }
        RedisOutbound::CarrierImpact(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize CarrierImpactEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ANQI_CARRIER_IMPACT,
                payload,
            })
        }
        RedisOutbound::ProjectileDespawned(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize ProjectileDespawnedEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ANQI_PROJECTILE_DESPAWNED,
                payload,
            })
        }
        RedisOutbound::AnqiMultiShot(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize MultiShotEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ANQI_MULTI_SHOT,
                payload,
            })
        }
        RedisOutbound::AnqiQiInjection(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize QiInjectionEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ANQI_QI_INJECTION,
                payload,
            })
        }
        RedisOutbound::AnqiEchoFractal(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize EchoFractalEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ANQI_ECHO_FRACTAL,
                payload,
            })
        }
        RedisOutbound::AnqiCarrierAbrasion(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize CarrierAbrasionEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ANQI_CARRIER_ABRASION,
                payload,
            })
        }
        RedisOutbound::AnqiContainerSwap(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize ContainerSwapEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ANQI_CONTAINER_SWAP,
                payload,
            })
        }
        RedisOutbound::TuikeShed(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize ShedEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_TUIKE_SHED,
                payload,
            })
        }
        RedisOutbound::TuikeV2SkillEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize TuikeSkillEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_TUIKE_V2_SKILL_EVENT,
                payload,
            })
        }
        // plan-combat-skill-feedback-bridges-v1 P6 — 蜕壳灰烬入包叙事事件
        RedisOutbound::TuikeAshDecay(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize TuikeAshDecayV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_TUIKE_ASH_DECAY,
                payload,
            })
        }
        RedisOutbound::MutationEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize MutationEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_MUTATION_EVENT,
                payload,
            })
        }
        RedisOutbound::MeridianSevered(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize MeridianSeveredEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_MERIDIAN_SEVERED,
                payload,
            })
        }
        RedisOutbound::BaomaiV4ScarCircuitFormed(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize BaomaiV4ScarCircuitFormedV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_BAOMAI_V4_SCAR_CIRCUIT_FORMED,
                payload,
            })
        }
        RedisOutbound::BaomaiV4ScarCircuitBroken(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize BaomaiV4ScarCircuitBrokenV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_BAOMAI_V4_SCAR_CIRCUIT_BROKEN,
                payload,
            })
        }
        RedisOutbound::BaomaiV4IronCocoonStageUp(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize BaomaiV4IronCocoonStageUpV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_BAOMAI_V4_IRON_COCOON_STAGE_UP,
                payload,
            })
        }
        RedisOutbound::BaomaiV4ResonanceLock(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize BaomaiV4ResonanceLockV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_BAOMAI_V4_RESONANCE_LOCK,
                payload,
            })
        }
        RedisOutbound::BaomaiV4ResonanceLockEnd(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize BaomaiV4ResonanceLockEndV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_BAOMAI_V4_RESONANCE_LOCK_END,
                payload,
            })
        }
        RedisOutbound::VoidErosionEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize VoidErosionEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_VOID_EROSION_EVENT,
                payload,
            })
        }
        RedisOutbound::ElderEncounterTerminal {
            delivery_id,
            event,
            receipt_tx,
        } => {
            let payload = serde_json::to_string(&event).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize durable ElderEncounterEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::ListPushWithReceipt {
                key: ELDER_ENCOUNTER_DURABLE_REDIS_KEY,
                payload,
                delivery_id,
                receipt_tx,
            })
        }
        RedisOutbound::ElderEncounterEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize ElderEncounterEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ELDER_ENCOUNTER,
                payload,
            })
        }
        // plan-territory-v1 P3 — 领地霸主变动叙事请求（照搬 ZonePressureCrossed arm）
        RedisOutbound::TerritoryDominanceNarration(req) => {
            let payload = serde_json::to_string(&req).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize TerritoryDominanceNarrationRequestV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_TERRITORY_NARRATION_REQUEST,
                payload,
            })
        }
        // ─── plan-agent-ui-data-v1 P0 ───────────────────────────────
        RedisOutbound::AgentUiResponse(resp) => {
            let payload = serde_json::to_string(&resp).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize AgentUiResponsePayloadV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_AGENT_UI_RESPONSE,
                payload,
            })
        }
        // ─── plan-halfstep-rechallenge-integration-v1 P1 ────────────
        RedisOutbound::HalfStepRechallengeTrigger(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize HalfStepRechallengeTriggerPayloadV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_HALFSTEP_RECHALLENGE,
                payload,
            })
        }
    }
}

fn tribulation_fanout_channels(event: &TribulationEventV1) -> Vec<&'static str> {
    let mut channels = Vec::new();
    if event.kind == TribulationKindV1::ZoneCollapse {
        channels.push(CH_TRIBULATION_COLLAPSE);
    }

    let phase_channel = match event.phase {
        TribulationPhaseV1::Omen => CH_TRIBULATION_OMEN,
        TribulationPhaseV1::Lock => CH_TRIBULATION_LOCK,
        TribulationPhaseV1::Wave { .. } | TribulationPhaseV1::HeartDemon => CH_TRIBULATION_WAVE,
        TribulationPhaseV1::Settle => CH_TRIBULATION_SETTLE,
    };
    if !channels.contains(&phase_channel) {
        channels.push(phase_channel);
    }

    // Main channel is the primary narration consumer; publish it last so a partial
    // fanout retry is less likely to duplicate narration on the compatibility path.
    channels.push(CH_TRIBULATION);

    channels
}

fn void_action_fanout_channels(event: &VoidActionBroadcastV1) -> Vec<&'static str> {
    let channel = match event.kind {
        VoidActionKind::SuppressTsy => CH_VOID_ACTION_SUPPRESS_TSY,
        VoidActionKind::ExplodeZone => CH_VOID_ACTION_EXPLODE_ZONE,
        VoidActionKind::Barrier => CH_VOID_ACTION_BARRIER,
    };
    vec![channel]
}

fn redact_redis_url_for_log(redis_url: &str) -> String {
    let Some(scheme_index) = redis_url.find("://") else {
        return "[redacted redis endpoint]".to_string();
    };

    let authority_and_path = &redis_url[(scheme_index + 3)..];
    let authority = authority_and_path
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let endpoint = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority)
        .trim();

    if endpoint.is_empty() {
        "[redacted redis endpoint]".to_string()
    } else {
        endpoint.to_string()
    }
}

async fn dispatch_outbound_command(
    pub_conn: &mut redis::aio::MultiplexedConnection,
    command: RedisIoCommand,
) -> Result<(), (String, RedisIoCommand)> {
    if runs_on_background_redis_connection(&command) {
        let mut background_conn = pub_conn.clone();
        tokio::spawn(async move {
            if let Err(error) = execute_outbound_command(&mut background_conn, &command).await {
                tracing::warn!("[bong][redis] background outbound failed: {error}");
            }
        });
        Ok(())
    } else {
        execute_outbound_command(pub_conn, &command)
            .await
            .map_err(|error| (error, command))
    }
}

fn runs_on_background_redis_connection(command: &RedisIoCommand) -> bool {
    // World-state publish and HASH replacements must never block the primary
    // outbound connection. Dormant receipt-bearing HASH writes report their
    // outcome back to `NpcDormantStore`; ordinary telemetry HASH failures only
    // warn and never become a `pending_command`.
    matches!(
        command,
        RedisIoCommand::Publish {
            channel: CH_WORLD_STATE,
            ..
        } | RedisIoCommand::HashReplaceWithReceipt { .. }
            | RedisIoCommand::HashReplace { .. }
    )
}

async fn execute_outbound_command(
    pub_conn: &mut redis::aio::MultiplexedConnection,
    command: &RedisIoCommand,
) -> Result<(), String> {
    match command {
        RedisIoCommand::ListPushWithReceipt {
            key,
            payload,
            delivery_id,
            receipt_tx,
        } => {
            let outcome = execute_list_push(pub_conn, key, payload).await;
            if outcome.is_ok() {
                let _ = receipt_tx.send(RedisDeliveryReceipt {
                    delivery_id: delivery_id.clone(),
                    outcome: Ok(()),
                });
            }
            outcome
        }
        RedisIoCommand::Publish { channel, payload } => {
            execute_publish(pub_conn, channel, payload).await
        }
        RedisIoCommand::PublishFanout { channels, payload } => {
            for channel in channels {
                execute_publish(pub_conn, channel, payload).await?;
            }
            Ok(())
        }
        RedisIoCommand::ListPush { key, payload } => {
            if *key == CH_PLAYER_CHAT {
                execute_player_chat_list_push(pub_conn, key, payload).await
            } else {
                execute_list_push(pub_conn, key, payload).await
            }
        }
        RedisIoCommand::HashReplaceWithReceipt {
            key,
            entries,
            delivery_id,
            receipt_tx,
        } => {
            let outcome = execute_hash_replace(pub_conn, key, entries).await;
            let _ = receipt_tx.send(RedisDeliveryReceipt {
                delivery_id: delivery_id.clone(),
                outcome: outcome.clone(),
            });
            outcome
        }
        RedisIoCommand::HashReplace { key, entries } => {
            execute_hash_replace(pub_conn, key, entries).await
        }
    }
}

async fn execute_list_push(
    pub_conn: &mut redis::aio::MultiplexedConnection,
    key: &'static str,
    payload: &str,
) -> Result<(), String> {
    match tokio::time::timeout(
        REDIS_IO_TIMEOUT,
        redis::cmd("RPUSH")
            .arg(key)
            .arg(payload)
            .query_async::<i64>(pub_conn),
    )
    .await
    {
        Ok(Ok(list_len)) => {
            tracing::debug!("[bong][redis] pushed payload onto {key}; list length {list_len}");
            Ok(())
        }
        Ok(Err(error)) => Err(format!("failed to RPUSH {key}: {error}")),
        Err(_) => Err(format!(
            "timed out RPUSH {key} after {:?}",
            REDIS_IO_TIMEOUT
        )),
    }
}

fn should_warn_player_chat_queue(list_len: i64) -> bool {
    list_len > PLAYER_CHAT_QUEUE_MAX_LEN
}

async fn execute_player_chat_list_push(
    pub_conn: &mut redis::aio::MultiplexedConnection,
    key: &'static str,
    payload: &str,
) -> Result<(), String> {
    // This is deliberately a non-transaction pipeline: Redis executes the
    // commands in order on this connection, so RPUSH reports the pre-trim
    // length while the following LTRIM retains only the newest N entries.
    let mut pipeline = redis::pipe();
    pipeline
        .cmd("RPUSH")
        .arg(key)
        .arg(payload)
        .cmd("LTRIM")
        .arg(key)
        .arg(-PLAYER_CHAT_QUEUE_MAX_LEN)
        .arg(-1_i64)
        .ignore();

    match tokio::time::timeout(REDIS_IO_TIMEOUT, pipeline.query_async::<(i64,)>(pub_conn)).await {
        Ok(Ok((list_len,))) => {
            if should_warn_player_chat_queue(list_len) {
                tracing::warn!(
                    "[bong][redis] player chat queue is being trimmed; dropping oldest entries: key={key} rpush_length={list_len} max_length={PLAYER_CHAT_QUEUE_MAX_LEN}"
                );
            }
            tracing::debug!(
                "[bong][redis] pushed and bounded player chat queue {key}; RPUSH length {list_len}, max length {PLAYER_CHAT_QUEUE_MAX_LEN}"
            );
            Ok(())
        }
        Ok(Err(error)) => Err(format!("failed to RPUSH {key}: {error}")),
        Err(_) => Err(format!(
            "timed out RPUSH {key} after {:?}",
            REDIS_IO_TIMEOUT
        )),
    }
}

async fn execute_hash_replace(
    pub_conn: &mut redis::aio::MultiplexedConnection,
    key: &'static str,
    entries: &[(String, String)],
) -> Result<(), String> {
    match tokio::time::timeout(
        REDIS_HASH_REPLACE_TIMEOUT,
        execute_hash_replace_atomic(pub_conn, key, entries),
    )
    .await
    {
        Ok(Ok(())) => {
            tracing::debug!(
                "[bong][redis] replaced hash {key}; entries={}",
                entries.len()
            );
            Ok(())
        }
        Ok(Err(error)) => Err(format!("failed to replace hash {key}: {error}")),
        Err(_) => {
            // `tokio::timeout` cancels (drops) the in-flight future instead of
            // returning Err, so `execute_hash_replace_atomic`'s inline cleanup
            // never runs and the deterministic temp key may be left behind
            // mid-write. Issue a best-effort DEL (bounded by REDIS_IO_TIMEOUT)
            // so a timed-out write does not leak the temp key. Failure here is
            // tolerated: the next attempt's leading DEL also clears it, and the
            // startup janitor sweeps any survivor.
            let temp_key = dormant_temp_key(key);
            let _ = tokio::time::timeout(
                REDIS_IO_TIMEOUT,
                redis::cmd("DEL")
                    .arg(temp_key.as_str())
                    .query_async::<i64>(pub_conn),
            )
            .await;
            Err(format!(
                "timed out replacing hash {key} after {:?}",
                REDIS_HASH_REPLACE_TIMEOUT
            ))
        }
    }
}

/// Max field count per `HSET` command when writing the dormant hash.
///
/// Root cause this guards against: `MultiplexedConnection` serialises a single
/// command's whole argument list into one frame before sending. One giant
/// `HSET` carrying ~1000+ fields degraded to ~3s of client-side framing and
/// tripped [`REDIS_HASH_REPLACE_TIMEOUT`] — even though Redis itself writes the
/// same 1000 fields in ~13ms when they arrive as several commands. So the cost
/// is the inline framing of one huge command, not the Redis server. Splitting
/// the write into sequential `HSET`s of this many fields each sidesteps it.
const DORMANT_HASH_CHUNK_SIZE: usize = 256;

/// Split `(field, value)` pairs into sub-slices of at most `chunk` each, for
/// batched `HSET`.
///
/// Pure (no Redis), so a pin test can lock "batch count == ceil(N / chunk) and
/// the batches concatenate back to the original input" without a live server.
///
/// `chunk == 0` is treated as 1 (one pair per batch) to avoid the panic from
/// `slice::chunks(0)`.
fn chunk_hash_fields<'a>(
    pairs: &'a [(&'a str, &'a str)],
    chunk: usize,
) -> Vec<&'a [(&'a str, &'a str)]> {
    // `slice::chunks(0)` panics; clamp to 1 so the "split into batches" contract
    // still holds (degenerate one-per-batch) instead of crashing.
    let chunk = chunk.max(1);
    pairs.chunks(chunk).collect()
}

async fn execute_hash_replace_atomic(
    pub_conn: &mut redis::aio::MultiplexedConnection,
    key: &'static str,
    entries: &[(String, String)],
) -> Result<(), redis::RedisError> {
    if entries.is_empty() {
        let _: i64 = redis::cmd("DEL").arg(key).query_async(pub_conn).await?;
        return Ok(());
    }

    // Deterministic temp key (single `{key}:tmp` per logical key) instead of a
    // per-call nanosecond nonce. A nonce produced a brand-new key on every
    // retry, so a `tokio::timeout` that *drops* the in-flight future (rather
    // than returning Err — the drop path skips the inline cleanup below) leaked
    // an ever-growing pile of `{key}:tmp:<nonce>` blobs and snowballed Redis
    // memory. With a fixed name, every attempt overwrites the same key: the
    // leading DEL clears any stale leftover, and the timeout branch in
    // `execute_hash_replace` issues a best-effort DEL of this exact key.
    let temp_key = dormant_temp_key(key);
    let del_start = Instant::now();
    let _: i64 = redis::cmd("DEL")
        .arg(temp_key.as_str())
        .query_async(pub_conn)
        .await?;
    let del_elapsed = del_start.elapsed();

    let field_pairs = entries
        .iter()
        .map(|(field, value)| (field.as_str(), value.as_str()))
        .collect::<Vec<_>>();

    // Sequential chunked HSET instead of one giant command: a single HSET with
    // ~1000+ fields stalls in `MultiplexedConnection`'s inline framing (~3s),
    // while several smaller commands let Redis finish in ~13ms. See
    // [`DORMANT_HASH_CHUNK_SIZE`].
    let batches = chunk_hash_fields(&field_pairs, DORMANT_HASH_CHUNK_SIZE);
    let chunk_count = batches.len();
    let hset_start = Instant::now();
    for batch in batches {
        let mut hset_cmd = redis::cmd("HSET");
        hset_cmd.arg(temp_key.as_str());
        for (field, value) in batch {
            hset_cmd.arg(*field).arg(*value);
        }
        if let Err(error) = hset_cmd.query_async::<i64>(pub_conn).await {
            // HSET failed mid-write: drop the partial temp key, then propagate.
            let _: Result<i64, _> = redis::cmd("DEL")
                .arg(temp_key.as_str())
                .query_async(pub_conn)
                .await;
            return Err(error);
        }
    }
    let hset_elapsed = hset_start.elapsed();

    let rename_start = Instant::now();
    let rename_result = redis::cmd("RENAME")
        .arg(temp_key.as_str())
        .arg(key)
        .query_async::<String>(pub_conn)
        .await;
    if let Err(error) = rename_result {
        let _: Result<i64, _> = redis::cmd("DEL")
            .arg(temp_key.as_str())
            .query_async(pub_conn)
            .await;
        return Err(error);
    }
    let rename_elapsed = rename_start.elapsed();

    tracing::debug!(
        "[bong][redis] hash_replace timing: del={:?} hset_total={:?} rename={:?} chunks={} entries={}",
        del_elapsed,
        hset_elapsed,
        rename_elapsed,
        chunk_count,
        entries.len(),
    );

    Ok(())
}

/// Deterministic temporary hash key for the atomic replace dance.
///
/// One stable name per logical key (`{key}:tmp`) so retries overwrite the
/// same key instead of leaking a fresh nonce-suffixed blob each time. A
/// `tokio::timeout` that drops the in-flight future skips the inline
/// cleanup, so the caller's timeout branch DELs exactly this key.
fn dormant_temp_key(key: &str) -> String {
    format!("{key}:tmp")
}

async fn execute_publish(
    pub_conn: &mut redis::aio::MultiplexedConnection,
    channel: &'static str,
    payload: &str,
) -> Result<(), String> {
    let timeout = publish_timeout_for_channel(channel);
    match tokio::time::timeout(
        timeout,
        redis::cmd("PUBLISH")
            .arg(channel)
            .arg(payload)
            .query_async::<i64>(pub_conn),
    )
    .await
    {
        Ok(Ok(subscribers)) => {
            tracing::debug!(
                "[bong][redis] published {channel}; observed {subscribers} subscribers"
            );
            Ok(())
        }
        Ok(Err(error)) => Err(format!("failed to publish {channel}: {error}")),
        Err(_) => Err(format!("timed out publishing {channel} after {timeout:?}")),
    }
}

fn publish_timeout_for_channel(channel: &str) -> Duration {
    if channel == CH_WORLD_STATE {
        REDIS_WORLD_STATE_PUBLISH_TIMEOUT
    } else {
        REDIS_IO_TIMEOUT
    }
}

async fn connect_bridge_session(
    client: &redis::Client,
    redis_url: &str,
    tx_to_game: &Sender<RedisInbound>,
    tx_fence: &Sender<BotDeliveryFenceRequest>,
) -> Result<
    (
        redis::aio::MultiplexedConnection,
        tokio::task::JoinHandle<SubscriberTaskExit>,
    ),
    String,
> {
    tracing::info!(
        "[bong][redis] connecting to {}",
        redact_redis_url_for_log(redis_url)
    );

    let pub_conn = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|error| format!("failed to get pub connection: {error}"))?;

    let mut pubsub = client
        .get_async_pubsub()
        .await
        .map_err(|error| format!("failed to get pubsub connection: {error}"))?;

    subscribe_inbound_channels(&mut pubsub).await?;
    tracing::info!(
        "[bong][redis] subscribed to {CH_AGENT_COMMAND}, {CH_AGENT_NARRATE}, {CH_AGENT_WORLD_MODEL}, {CH_INSIGHT_OFFER}, {CH_HEART_DEMON_OFFER}, {CH_SPIRIT_TREASURE_DIALOGUE}"
    );

    let tx_to_game_clone = tx_to_game.clone();
    let tx_fence_clone = tx_fence.clone();
    let sub_task =
        tokio::spawn(
            async move { run_subscriber_task(pubsub, tx_to_game_clone, tx_fence_clone).await },
        );

    Ok((pub_conn, sub_task))
}

async fn subscribe_inbound_channels(pubsub: &mut redis::aio::PubSub) -> Result<(), String> {
    pubsub
        .subscribe(CH_AGENT_COMMAND)
        .await
        .map_err(|error| format!("failed to subscribe to {CH_AGENT_COMMAND}: {error}"))?;

    pubsub
        .subscribe(CH_AGENT_NARRATE)
        .await
        .map_err(|error| format!("failed to subscribe to {CH_AGENT_NARRATE}: {error}"))?;

    pubsub
        .subscribe(CH_AGENT_WORLD_MODEL)
        .await
        .map_err(|error| format!("failed to subscribe to {CH_AGENT_WORLD_MODEL}: {error}"))?;

    pubsub
        .subscribe(CH_INSIGHT_OFFER)
        .await
        .map_err(|error| format!("failed to subscribe to {CH_INSIGHT_OFFER}: {error}"))?;

    pubsub
        .subscribe(CH_HEART_DEMON_OFFER)
        .await
        .map_err(|error| format!("failed to subscribe to {CH_HEART_DEMON_OFFER}: {error}"))?;

    pubsub
        .subscribe(CH_SPIRIT_TREASURE_DIALOGUE)
        .await
        .map_err(|error| {
            format!("failed to subscribe to {CH_SPIRIT_TREASURE_DIALOGUE}: {error}")
        })?;

    // ─── plan-agent-ui-data-v1 P0 ───────────────────────────────────
    pubsub
        .subscribe(CH_AGENT_UI_CMD)
        .await
        .map_err(|error| format!("failed to subscribe to {CH_AGENT_UI_CMD}: {error}"))?;

    pubsub
        .subscribe(CH_BOT_DELIVERY_FENCE_REQUEST)
        .await
        .map_err(|error| {
            format!("failed to subscribe to {CH_BOT_DELIVERY_FENCE_REQUEST}: {error}")
        })?;

    Ok(())
}

async fn run_bridge_session(
    rx_from_game: &Receiver<RedisOutbound>,
    rx_fence: &Receiver<BotDeliveryFenceRequest>,
    pending_command: &mut Option<RedisIoCommand>,
    mut pub_conn: redis::aio::MultiplexedConnection,
    sub_task: tokio::task::JoinHandle<SubscriberTaskExit>,
) -> BridgeLoopControl {
    let mut bridge_tick = tokio::time::interval(BRIDGE_LOOP_INTERVAL);
    let sub_task = sub_task;

    loop {
        bridge_tick.tick().await;

        match drain_outbound_messages(rx_from_game, &mut pub_conn, pending_command).await {
            DrainOutcome::Healthy => {}
            DrainOutcome::Reconnect { reason } => {
                abort_subscriber_task(sub_task).await;
                return BridgeLoopControl::Reconnect { reason };
            }
            DrainOutcome::Stop => {
                abort_subscriber_task(sub_task).await;
                return BridgeLoopControl::Stop;
            }
        }

        while let Ok(request) = rx_fence.try_recv() {
            if let Err(control) =
                flush_and_ack_delivery_fence(rx_from_game, pending_command, &mut pub_conn, request)
                    .await
            {
                abort_subscriber_task(sub_task).await;
                return control;
            }
        }

        if sub_task.is_finished() {
            return handle_finished_subscriber_task(sub_task).await;
        }
    }
}

async fn flush_and_ack_delivery_fence(
    rx_from_game: &Receiver<RedisOutbound>,
    pending_command: &mut Option<RedisIoCommand>,
    pub_conn: &mut redis::aio::MultiplexedConnection,
    request: BotDeliveryFenceRequest,
) -> Result<(), BridgeLoopControl> {
    loop {
        match drain_outbound_messages(rx_from_game, pub_conn, pending_command).await {
            DrainOutcome::Healthy if pending_command.is_none() && rx_from_game.is_empty() => break,
            DrainOutcome::Healthy => continue,
            DrainOutcome::Reconnect { reason } => {
                return Err(BridgeLoopControl::Reconnect { reason });
            }
            DrainOutcome::Stop => return Err(BridgeLoopControl::Stop),
        }
    }
    let payload = serde_json::json!({"v": 1, "ok": true, "token": request.token});
    execute_publish(
        pub_conn,
        CH_BOT_DELIVERY_FENCE_ACK,
        payload.to_string().as_str(),
    )
    .await
    .map_err(|error| BridgeLoopControl::Reconnect {
        reason: format!("delivery_fence_ack_failed: {error}"),
    })
}

async fn abort_subscriber_task(sub_task: tokio::task::JoinHandle<SubscriberTaskExit>) {
    if !sub_task.is_finished() {
        sub_task.abort();
    }

    let _ = sub_task.await;
}

async fn handle_finished_subscriber_task(
    sub_task: tokio::task::JoinHandle<SubscriberTaskExit>,
) -> BridgeLoopControl {
    map_subscriber_join_result(sub_task.await)
}

fn map_subscriber_join_result(
    result: Result<SubscriberTaskExit, tokio::task::JoinError>,
) -> BridgeLoopControl {
    match result {
        Ok(SubscriberTaskExit::StreamEnded) => {
            tracing::warn!("[bong][redis] subscriber_ended reason=stream_ended");
            BridgeLoopControl::Reconnect {
                reason: "subscriber_ended:stream_ended".to_string(),
            }
        }
        Ok(SubscriberTaskExit::GameChannelClosed) => {
            tracing::warn!("[bong][redis] subscriber_ended reason=game_channel_closed");
            BridgeLoopControl::Reconnect {
                reason: "subscriber_ended:game_channel_closed".to_string(),
            }
        }
        Err(error) if error.is_cancelled() => BridgeLoopControl::Reconnect {
            reason: "subscriber_cancelled".to_string(),
        },
        Err(error) => {
            tracing::warn!("[bong][redis] subscriber_ended reason=join_error error={error}");
            BridgeLoopControl::Reconnect {
                reason: format!("subscriber_join_error: {error}"),
            }
        }
    }
}

async fn run_subscriber_task(
    mut pubsub: redis::aio::PubSub,
    tx_to_game: Sender<RedisInbound>,
    tx_fence: Sender<BotDeliveryFenceRequest>,
) -> SubscriberTaskExit {
    use futures_util::StreamExt;

    let mut stream = pubsub.on_message();
    while let Some(message) = stream.next().await {
        let channel: String = match message.get_channel() {
            Ok(channel) => channel,
            Err(error) => {
                tracing::warn!("[bong][redis] failed to read inbound channel name: {error}");
                continue;
            }
        };

        let payload: String = match message.get_payload() {
            Ok(payload) => payload,
            Err(error) => {
                tracing::warn!("[bong][redis] failed to read payload from {channel}: {error}");
                continue;
            }
        };

        if channel == CH_BOT_DELIVERY_FENCE_REQUEST {
            let request = match serde_json::from_str::<serde_json::Value>(payload.as_str()) {
                Ok(value) => value,
                Err(error) => {
                    tracing::warn!("[bong][redis] dropped invalid delivery fence request: {error}");
                    continue;
                }
            };
            let Some(token) = request.get("token").and_then(Value::as_str) else {
                tracing::warn!("[bong][redis] dropped delivery fence request without token");
                continue;
            };
            if token.is_empty() || token.len() > 128 {
                tracing::warn!(
                    "[bong][redis] dropped delivery fence request with invalid token length"
                );
                continue;
            }
            if tx_fence
                .send(BotDeliveryFenceRequest {
                    token: token.to_string(),
                })
                .is_err()
            {
                tracing::warn!(
                    "[bong][redis] delivery fence channel closed; stopping subscriber task"
                );
                return SubscriberTaskExit::GameChannelClosed;
            }
            continue;
        }

        match parse_inbound_message(channel.as_str(), payload.as_str()) {
            Ok(Some(inbound)) => {
                match &inbound {
                    RedisInbound::AgentCommand(command) => tracing::info!(
                        "[bong][redis] received agent command: {} ({} cmds)",
                        command.id,
                        command.commands.len()
                    ),
                    RedisInbound::AgentNarration(narration) => tracing::info!(
                        "[bong][redis] received narration ({} entries)",
                        narration.narrations.len()
                    ),
                    RedisInbound::AgentWorldModel(envelope) => tracing::info!(
                        "[bong][redis] received world model envelope: {} (last_tick={:?})",
                        envelope.id,
                        envelope.snapshot.last_tick
                    ),
                    RedisInbound::InsightOffer(offer) => tracing::info!(
                        "[bong][redis] received insight offer: trigger={} ({} choices)",
                        offer.trigger_id,
                        offer.choices.len()
                    ),
                    RedisInbound::HeartDemonOffer(offer) => tracing::info!(
                        "[bong][redis] received heart demon offer: trigger={} ({} choices)",
                        offer.trigger_id,
                        offer.choices.len()
                    ),
                    RedisInbound::SpiritTreasureDialogue(dialogue) => tracing::info!(
                        "[bong][redis] received spirit treasure dialogue: treasure={} request={} tone={:?}",
                        dialogue.treasure_id,
                        dialogue.request_id,
                        dialogue.tone
                    ),
                    // ─── plan-agent-ui-data-v1 P0 ──────────────────────────────────────
                    RedisInbound::AgentUiCmd(cmd) => tracing::info!(
                        "[bong][redis] received agent_ui_cmd: request_id={} target={}",
                        cmd.request_id,
                        cmd.target_player,
                    ),
                }

                if tx_to_game.send(inbound).is_err() {
                    tracing::warn!(
                        "[bong][redis] inbound channel to game closed; stopping subscriber task"
                    );
                    return SubscriberTaskExit::GameChannelClosed;
                }
            }
            Ok(None) => {
                tracing::debug!("[bong][redis] ignoring message on unexpected channel {channel}");
            }
            Err(error) => tracing::warn!(
                "[bong][redis] dropped invalid inbound payload on {channel}: {error}"
            ),
        }
    }

    SubscriberTaskExit::StreamEnded
}

async fn sleep_before_reconnect(redis_url: &str, backoff: &mut ReconnectBackoff, reason: &str) {
    let schedule = backoff.next();
    tracing::info!(
        "[bong][redis] reconnect attempt={} endpoint={} reason={reason}",
        schedule.attempt,
        redact_redis_url_for_log(redis_url),
    );
    tracing::info!(
        "[bong][redis] backoff {:?} before reconnect attempt={}",
        schedule.delay,
        schedule.attempt,
    );
    tokio::time::sleep(schedule.delay).await;
}

fn parse_inbound_message(
    channel: &str,
    payload: &str,
) -> Result<Option<RedisInbound>, ValidationError> {
    let value: Value = serde_json::from_str(payload)
        .map_err(|error| ValidationError::new(format!("invalid JSON payload: {error}")))?;

    match channel {
        CH_AGENT_COMMAND => {
            validate_agent_command_value(&value)?;
            let command = serde_json::from_value::<AgentCommandV1>(value).map_err(|error| {
                ValidationError::new(format!("failed to deserialize AgentCommandV1: {error}"))
            })?;
            Ok(Some(RedisInbound::AgentCommand(command)))
        }
        CH_AGENT_NARRATE => {
            validate_narration_value(&value)?;
            let narration = serde_json::from_value::<NarrationV1>(value).map_err(|error| {
                ValidationError::new(format!("failed to deserialize NarrationV1: {error}"))
            })?;
            Ok(Some(RedisInbound::AgentNarration(narration)))
        }
        CH_AGENT_WORLD_MODEL => {
            validate_agent_world_model_value(&value)?;
            let envelope =
                serde_json::from_value::<AgentWorldModelEnvelopeV1>(value).map_err(|error| {
                    ValidationError::new(format!(
                        "failed to deserialize AgentWorldModelEnvelopeV1: {error}"
                    ))
                })?;
            Ok(Some(RedisInbound::AgentWorldModel(envelope)))
        }
        CH_INSIGHT_OFFER => {
            let offer = serde_json::from_value::<InsightOfferV1>(value).map_err(|error| {
                ValidationError::new(format!("failed to deserialize InsightOfferV1: {error}"))
            })?;
            Ok(Some(RedisInbound::InsightOffer(offer)))
        }
        CH_HEART_DEMON_OFFER => {
            let offer = serde_json::from_value::<HeartDemonOfferV1>(value).map_err(|error| {
                ValidationError::new(format!("failed to deserialize HeartDemonOfferV1: {error}"))
            })?;
            Ok(Some(RedisInbound::HeartDemonOffer(offer)))
        }
        CH_SPIRIT_TREASURE_DIALOGUE => {
            let dialogue =
                serde_json::from_value::<SpiritTreasureDialogueV1>(value).map_err(|error| {
                    ValidationError::new(format!(
                        "failed to deserialize SpiritTreasureDialogueV1: {error}"
                    ))
                })?;
            Ok(Some(RedisInbound::SpiritTreasureDialogue(dialogue)))
        }
        // ─── plan-agent-ui-data-v1 P0 ───────────────────────────────
        CH_AGENT_UI_CMD => {
            let cmd =
                serde_json::from_value::<AgentUiRequestCommandV1>(value).map_err(|error| {
                    ValidationError::new(format!(
                        "failed to deserialize AgentUiRequestCommandV1: {error}"
                    ))
                })?;
            Ok(Some(RedisInbound::AgentUiCmd(cmd)))
        }
        _ => Ok(None),
    }
}

fn validate_world_state(state: &WorldStateV1) -> Result<(), ValidationError> {
    if state.v != 1 {
        return Err(ValidationError::new(format!(
            "WorldStateV1 must use version 1, got {}",
            state.v
        )));
    }

    Ok(())
}

fn validate_agent_world_model_value(value: &Value) -> Result<(), ValidationError> {
    let object = expect_object(value, "AgentWorldModelEnvelopeV1")?;
    validate_known_keys(
        object,
        &["v", "id", "source", "snapshot"],
        "AgentWorldModelEnvelopeV1",
    )?;
    validate_schema_version(object, "AgentWorldModelEnvelopeV1")?;
    expect_string_field(object, "id", "AgentWorldModelEnvelopeV1")?;

    if let Some(source) = object.get("source") {
        let source = source.as_str().ok_or_else(|| {
            ValidationError::new("AgentWorldModelEnvelopeV1.source must be a string when present")
        })?;

        if !matches!(source, "calamity" | "mutation" | "era" | "arbiter") {
            return Err(ValidationError::new(format!(
                "AgentWorldModelEnvelopeV1.source has unsupported value `{source}`"
            )));
        }
    }

    let snapshot = expect_field(object, "snapshot", "AgentWorldModelEnvelopeV1")?;
    if !snapshot.is_object() {
        return Err(ValidationError::new(
            "AgentWorldModelEnvelopeV1.snapshot must be an object",
        ));
    }

    Ok(())
}

fn validate_chat_message(chat: &ChatMessageV1) -> Result<(), ValidationError> {
    if chat.v != 1 {
        return Err(ValidationError::new(format!(
            "ChatMessageV1 must use version 1, got {}",
            chat.v
        )));
    }

    if chat.raw.chars().count() > CHAT_MESSAGE_MAX_LENGTH {
        return Err(ValidationError::new(format!(
            "ChatMessageV1.raw exceeds {CHAT_MESSAGE_MAX_LENGTH} characters"
        )));
    }

    Ok(())
}

fn validate_agent_command_value(value: &Value) -> Result<(), ValidationError> {
    let object = expect_object(value, "AgentCommandV1")?;
    validate_known_keys(object, &["v", "id", "source", "commands"], "AgentCommandV1")?;
    validate_schema_version(object, "AgentCommandV1")?;
    expect_string_field(object, "id", "AgentCommandV1")?;

    if let Some(source) = object.get("source") {
        let source = source.as_str().ok_or_else(|| {
            ValidationError::new("AgentCommandV1.source must be a string when present")
        })?;

        if !matches!(source, "calamity" | "mutation" | "era" | "arbiter") {
            return Err(ValidationError::new(format!(
                "AgentCommandV1.source has unsupported value `{source}`"
            )));
        }
    }

    let commands = expect_array_field(object, "commands", "AgentCommandV1")?;
    if commands.len() > MAX_COMMANDS_PER_TICK {
        return Err(ValidationError::new(format!(
            "AgentCommandV1.commands exceeds maxItems {MAX_COMMANDS_PER_TICK}"
        )));
    }

    for (index, command) in commands.iter().enumerate() {
        validate_command_value(command, index)?;
    }

    Ok(())
}

fn validate_command_value(value: &Value, index: usize) -> Result<(), ValidationError> {
    let context = format!("AgentCommandV1.commands[{index}]");
    let object = expect_object(value, context.as_str())?;
    validate_known_keys(object, &["type", "target", "params"], context.as_str())?;

    let command_type = expect_string_field(object, "type", context.as_str())?;
    if !matches!(
        command_type,
        "spawn_event"
            | "spawn_npc"
            | "despawn_npc"
            | "faction_event"
            | "modify_zone"
            | "npc_behavior"
            | "heartbeat_override"
    ) {
        return Err(ValidationError::new(format!(
            "{context}.type has unsupported value `{command_type}`"
        )));
    }

    expect_string_field(object, "target", context.as_str())?;

    let params = expect_field(object, "params", context.as_str())?;
    if !params.is_object() {
        return Err(ValidationError::new(format!(
            "{context}.params must be an object"
        )));
    }

    if command_type == "spawn_npc" {
        let params = params
            .as_object()
            .ok_or_else(|| ValidationError::new(format!("{context}.params must be an object")))?;
        let archetype = params.get("archetype").ok_or_else(|| {
            ValidationError::new(format!(
                "{context}.params is missing required field `archetype`"
            ))
        })?;
        if !archetype.is_string() {
            return Err(ValidationError::new(format!(
                "{context}.params.archetype must be a string"
            )));
        }
    }

    if command_type == "faction_event" {
        let params = params
            .as_object()
            .ok_or_else(|| ValidationError::new(format!("{context}.params must be an object")))?;
        let kind = params.get("kind").ok_or_else(|| {
            ValidationError::new(format!("{context}.params is missing required field `kind`"))
        })?;
        if !kind.is_string() {
            return Err(ValidationError::new(format!(
                "{context}.params.kind must be a string"
            )));
        }

        let faction_id = params.get("faction_id").ok_or_else(|| {
            ValidationError::new(format!(
                "{context}.params is missing required field `faction_id`"
            ))
        })?;
        if !faction_id.is_string() {
            return Err(ValidationError::new(format!(
                "{context}.params.faction_id must be a string"
            )));
        }
    }

    Ok(())
}

fn validate_narration_value(value: &Value) -> Result<(), ValidationError> {
    let object = expect_object(value, "NarrationV1")?;
    validate_known_keys(object, &["v", "narrations"], "NarrationV1")?;
    validate_schema_version(object, "NarrationV1")?;

    let narrations = expect_array_field(object, "narrations", "NarrationV1")?;
    for (index, narration) in narrations.iter().enumerate() {
        validate_narration_entry(narration, index)?;
    }

    Ok(())
}

fn validate_narration_entry(value: &Value, index: usize) -> Result<(), ValidationError> {
    let context = format!("NarrationV1.narrations[{index}]");
    let object = expect_object(value, context.as_str())?;
    validate_known_keys(
        object,
        &["scope", "target", "text", "style", "kind"],
        context.as_str(),
    )?;

    let scope = expect_string_field(object, "scope", context.as_str())?;
    if !matches!(scope, "broadcast" | "zone" | "player") {
        return Err(ValidationError::new(format!(
            "{context}.scope has unsupported value `{scope}`"
        )));
    }

    let has_target = object.get("target").is_some();
    if let Some(target) = object.get("target") {
        if !target.is_string() {
            return Err(ValidationError::new(format!(
                "{context}.target must be a string when present"
            )));
        }
    }

    if scope != "broadcast" && !has_target {
        return Err(ValidationError::new(format!(
            "{context}.target is required when scope is `{scope}`"
        )));
    }

    let text = expect_string_field(object, "text", context.as_str())?;
    if text.chars().count() > MAX_NARRATION_LENGTH {
        return Err(ValidationError::new(format!(
            "{context}.text exceeds {MAX_NARRATION_LENGTH} characters"
        )));
    }

    let style = expect_string_field(object, "style", context.as_str())?;
    if !matches!(
        style,
        "system_warning" | "perception" | "narration" | "era_decree" | "political_jianghu"
    ) {
        return Err(ValidationError::new(format!(
            "{context}.style has unsupported value `{style}`"
        )));
    }

    if let Some(kind) = object.get("kind") {
        let Some(kind) = kind.as_str() else {
            return Err(ValidationError::new(format!(
                "{context}.kind must be a string when present"
            )));
        };
        if !matches!(
            kind,
            "death_insight"
                | "niche_intrusion"
                | "niche_intrusion_by_npc"
                | "npc_farm_pressure"
                | "scattered_cultivator"
                | "political_jianghu"
                // bug-hunt-1: 与 NarrationKind enum（common.rs）+ agent TypeBox
                // （common.ts:81-87）对齐，补 war_outcome / dying_elder_* 这 6 个，
                // 否则天道发的大战结局与濒死老者剧情 narration 被此处校验拒收丢弃。
                | "war_outcome"
                | "dying_elder_appeared"
                | "dying_elder_dan_received"
                | "dying_elder_betrayal"
                | "dying_elder_dead_natural"
                | "dying_elder_dead_player_kill"
        ) {
            return Err(ValidationError::new(format!(
                "{context}.kind has unsupported value `{kind}`"
            )));
        }
    }

    Ok(())
}

fn validate_schema_version(
    object: &Map<String, Value>,
    context: &str,
) -> Result<(), ValidationError> {
    let version = expect_field(object, "v", context)?
        .as_u64()
        .ok_or_else(|| ValidationError::new(format!("{context}.v must be an integer")))?;

    if version != 1 {
        return Err(ValidationError::new(format!(
            "{context}.v must be 1, got {version}"
        )));
    }

    Ok(())
}

fn validate_known_keys(
    object: &Map<String, Value>,
    allowed_keys: &[&str],
    context: &str,
) -> Result<(), ValidationError> {
    if let Some(unexpected) = object
        .keys()
        .find(|key| !allowed_keys.contains(&key.as_str()))
    {
        return Err(ValidationError::new(format!(
            "{context} contains unsupported field `{unexpected}`"
        )));
    }

    Ok(())
}

fn expect_object<'a>(
    value: &'a Value,
    context: &str,
) -> Result<&'a Map<String, Value>, ValidationError> {
    value
        .as_object()
        .ok_or_else(|| ValidationError::new(format!("{context} must be a JSON object")))
}

fn expect_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a Value, ValidationError> {
    object.get(field).ok_or_else(|| {
        ValidationError::new(format!("{context} is missing required field `{field}`"))
    })
}

fn expect_string_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a str, ValidationError> {
    expect_field(object, field, context)?
        .as_str()
        .ok_or_else(|| ValidationError::new(format!("{context}.{field} must be a string")))
}

fn expect_array_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a Vec<Value>, ValidationError> {
    expect_field(object, field, context)?
        .as_array()
        .ok_or_else(|| ValidationError::new(format!("{context}.{field} must be an array")))
}

#[cfg(test)]
#[path = "redis_bridge_tests.rs"]
mod tests;
