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
    CH_BREAKTHROUGH_CINEMATIC, CH_BREAKTHROUGH_EVENT, CH_COMBAT_REALTIME, CH_COMBAT_SUMMARY,
    CH_CULTIVATION_DEATH, CH_DEATH_CINEMATIC, CH_DEATH_INSIGHT, CH_DUGU_ANTIDOTE_RESULT,
    CH_DUGU_POISON_PROGRESS, CH_DUGU_V2_CAST, CH_DUGU_V2_REVERSE, CH_DUGU_V2_SELF_CURE,
    CH_DUO_SHE_EVENT, CH_ELDER_ENCOUNTER, CH_FACTION_EVENT, CH_FACTION_STATE, CH_FACTION_WAR,
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
    CH_VOID_ACTION_BARRIER, CH_VOID_ACTION_EXPLODE_ZONE, CH_VOID_ACTION_LEGACY_ASSIGN,
    CH_VOID_ACTION_SUPPRESS_TSY, CH_VOID_EROSION_EVENT, CH_WANTED_PLAYER, CH_WEATHER_EVENT_UPDATE,
    CH_WOLIU_BACKFIRE, CH_WOLIU_PROJECTILE_DRAINED, CH_WOLIU_V2_BACKFIRE, CH_WOLIU_V2_CAST,
    CH_WOLIU_V2_TURBULENCE, CH_WORLD_STATE, CH_YIDAO_EVENT, CH_ZHENFA_V2_EVENT,
    CH_ZHENMAI_SKILL_EVENT, CH_ZONE_ENVIRONMENT_UPDATE, CH_ZONE_PRESSURE_CROSSED,
    CH_ZONG_CORE_ACTIVATED, QI_LEDGER_REDIS_KEY,
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
    NpcDormantHash(Vec<(String, String)>),
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
    /// plan-void-actions-v1 — 化虚四类世界级 action 公告。
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
    /// plan-dying-elder-v1 P3 — 垂死大能遭遇事件（bong:elder_encounter）。
    ElderEncounterEvent(ElderEncounterEventV1),
    /// plan-territory-v1 P3 — 领地霸主变动叙事请求（bong:territory_narration_request）。
    /// 三时刻：新霸主确立 / 霸主被驱逐 / 区域灵气耗尽。匿名（境界段，非 char_id）。
    TerritoryDominanceNarration(TerritoryDominanceNarrationRequestV1),
    // ─── plan-agent-ui-data-v1 P0 ───────────────────────────────────
    /// 玩家天道 UI 面板交互响应（bong:agent_ui_response）。
    AgentUiResponse(AgentUiResponsePayloadV1),
    // ─── plan-halfstep-rechallenge-integration-v1 P1 ────────────────
    /// 半步化虚重渡触发（bong:tribulation/halfstep_rechallenge），agent narration 用。
    HalfStepRechallengeTrigger(
        crate::schema::halfstep_rechallenge::HalfStepRechallengeTriggerPayloadV1,
    ),
}

#[derive(Debug, Clone, PartialEq)]
enum RedisIoCommand {
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

pub fn spawn_redis_bridge(
    redis_url: &str,
) -> (
    std::thread::JoinHandle<()>,
    Sender<RedisOutbound>,
    Receiver<RedisInbound>,
) {
    let (tx_to_game, rx_inbound) = crossbeam_channel::unbounded::<RedisInbound>();
    let (tx_outbound, rx_from_game) = crossbeam_channel::unbounded::<RedisOutbound>();

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
                match connect_bridge_session(&client, url.as_str(), &tx_to_game).await {
                    Ok((pub_conn, sub_task)) => {
                        backoff.reset();

                        match run_bridge_session(
                            &rx_from_game,
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
        RedisOutbound::NpcDormantHash(entries) => Ok(RedisIoCommand::HashReplace {
            key: NPC_DORMANT_REDIS_KEY,
            entries,
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
        VoidActionKind::LegacyAssign => CH_VOID_ACTION_LEGACY_ASSIGN,
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
    // Both world_state Publish and dormant HashReplace must never block the
    // primary outbound connection: a slow / timing-out dormant write previously
    // pinned `pending_command` and starved every other channel (world_state,
    // combat, chat). Routing them onto the cloned background connection makes
    // them fire-and-forget — failures only `warn!` and never become a
    // `pending_command`. `HashReplace` is exclusively produced by
    // `RedisOutbound::NpcDormantHash` (see `prepare_outbound_command`), so the
    // sole key it ever targets is `NPC_DORMANT_REDIS_KEY`; matching the variant
    // is equivalent to "only dormant runs in the background".
    matches!(
        command,
        RedisIoCommand::Publish {
            channel: CH_WORLD_STATE,
            ..
        } | RedisIoCommand::HashReplace { .. }
    )
}

async fn execute_outbound_command(
    pub_conn: &mut redis::aio::MultiplexedConnection,
    command: &RedisIoCommand,
) -> Result<(), String> {
    match command {
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
                    tracing::debug!(
                        "[bong][redis] pushed payload onto {key}; list length {list_len}"
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
        RedisIoCommand::HashReplace { key, entries } => {
            execute_hash_replace(pub_conn, key, entries).await
        }
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
    let sub_task = tokio::spawn(async move { run_subscriber_task(pubsub, tx_to_game_clone).await });

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

    Ok(())
}

async fn run_bridge_session(
    rx_from_game: &Receiver<RedisOutbound>,
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

        if sub_task.is_finished() {
            return handle_finished_subscriber_task(sub_task).await;
        }
    }
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
mod redis_bridge_tests {
    use super::*;

    /// bug-hunt-1: war_outcome / dying_elder_* 是 agent 端生产 narration kind
    /// （war-outcome-narration.ts / elder-encounter-narration.ts）。此前 server 白名单
    /// 漏列导致 validate_narration_entry 拒收 → 整 batch 在 redis_bridge.rs:2224
    /// tracing::warn! 丢弃，玩家收不到大战结局 / 濒死老者剧情。锁定 6 个新 kind 均通过校验。
    #[test]
    fn validate_narration_entry_accepts_new_agent_narration_kinds() {
        for kind in [
            "war_outcome",
            "dying_elder_appeared",
            "dying_elder_dan_received",
            "dying_elder_betrayal",
            "dying_elder_dead_natural",
            "dying_elder_dead_player_kill",
        ] {
            let entry = serde_json::json!({
                "scope": "broadcast",
                "text": "测试播报",
                "style": "narration",
                "kind": kind,
            });
            assert!(
                validate_narration_entry(&entry, 0).is_ok(),
                "kind `{kind}` 应被 validate_narration_entry 接受（agent 端有效 narration kind，否则整 batch 被丢弃）"
            );
        }
    }

    /// 负锚点：未知 kind 仍应被拒，确保白名单非恒真。
    #[test]
    fn validate_narration_entry_rejects_unknown_kind() {
        let entry = serde_json::json!({
            "scope": "broadcast",
            "text": "x",
            "style": "narration",
            "kind": "totally_bogus_kind",
        });
        assert!(
            validate_narration_entry(&entry, 0).is_err(),
            "未知 kind 必须被拒，否则白名单形同虚设"
        );
    }

    use crate::fauna::rat_phase::RatPhase;
    use crate::schema::anticheat::{AntiCheatReportV1, ViolationKindV1};
    use crate::schema::combat_event::{
        CombatAttackSourceV1, CombatRealtimeEventV1, CombatRealtimeKindV1, CombatSummaryV1,
    };
    use crate::schema::common::CommandType;
    use crate::schema::death_insight::{
        DeathInsightCategoryV1, DeathInsightRequestV1, DeathInsightZoneKindV1,
    };
    use crate::schema::death_lifecycle::{AgingEventKindV1, LifespanEventKindV1};
    use crate::schema::economy::PriceSampleV1;
    use crate::schema::forge::ForgeOutcomeBucketV1;
    use crate::schema::social::{
        ExposureKindV1, HighRenownMilestoneEventTag, HighRenownMilestoneEventV1, RenownTagV1,
    };
    use crate::schema::spirit_eye::{
        SpiritEyeMigrateReasonV1, SpiritEyeMigrateV1, SpiritEyePositionV1,
    };
    use crate::schema::tuike_v2::{FalseSkinTierV1, TuikeSkillIdV1, TuikeSkillVisualContractV1};
    use crate::schema::zhenmai_v2::{ZhenmaiAttackKindV1, ZhenmaiSkillIdV1};
    use serde_json::json;
    use tokio::task;

    fn sample_world_state() -> WorldStateV1 {
        serde_json::from_str(include_str!(
            "../../../agent/packages/schema/samples/world-state.sample.json"
        ))
        .expect("world-state sample should deserialize")
    }

    fn sample_chat_message() -> ChatMessageV1 {
        serde_json::from_str(include_str!(
            "../../../agent/packages/schema/samples/chat-message.sample.json"
        ))
        .expect("chat-message sample should deserialize")
    }

    #[test]
    fn publishes_world_state() {
        let command = prepare_outbound_command(RedisOutbound::WorldState(sample_world_state()))
            .expect("world state should produce a publish command");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_WORLD_STATE);

                let payload: Value =
                    serde_json::from_str(&payload).expect("publish payload should be valid JSON");
                assert_eq!(payload["v"], 1);
                assert_eq!(payload["tick"], 84000);
            }
            other => panic!("expected PUBLISH command, got {other:?}"),
        }
    }

    #[test]
    fn publishes_economy_telemetry_channels() {
        let bone_tick = prepare_outbound_command(RedisOutbound::BoneCoinTick(BoneCoinTickV1 {
            v: 1,
            tick: 720_000,
            season: crate::schema::world_state::SeasonV1::SummerToWinter,
            total_spirit_qi: 27.5,
            total_face_value: 60.0,
            active_coin_count: 3,
            rotten_coin_count: 1,
            legacy_scalar_count: 7,
            rhythm_multiplier: 1.1,
            market_factor: 0.9,
        }))
        .expect("bone coin tick should publish");
        match bone_tick {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_BONE_COIN_TICK);
                let payload: Value =
                    serde_json::from_str(&payload).expect("publish payload should be valid JSON");
                assert_eq!(payload["season"], "summer_to_winter");
            }
            other => panic!("expected PUBLISH command, got {other:?}"),
        }

        let price_index = prepare_outbound_command(RedisOutbound::PriceIndex(PriceIndexV1 {
            v: 1,
            tick: 720_000,
            season: crate::schema::world_state::SeasonV1::SummerToWinter,
            supply_spirit_qi: 27.5,
            demand_spirit_qi: 50.0,
            rhythm_multiplier: 1.1,
            market_factor: 0.9,
            price_multiplier: 0.99,
            sample_prices: vec![PriceSampleV1 {
                item_id: "common_good".to_string(),
                base_price: 4,
                final_price: 4,
            }],
        }))
        .expect("price index should publish");
        match price_index {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_PRICE_INDEX);
                let payload: Value =
                    serde_json::from_str(&payload).expect("publish payload should be valid JSON");
                assert_eq!(payload["sample_prices"][0]["item_id"], "common_good");
            }
            other => panic!("expected PUBLISH command, got {other:?}"),
        }
    }

    #[test]
    fn pushes_chat_messages() {
        let command = prepare_outbound_command(RedisOutbound::PlayerChat(sample_chat_message()))
            .expect("chat payload should produce an RPUSH command");

        match command {
            RedisIoCommand::ListPush { key, payload } => {
                assert_eq!(key, CH_PLAYER_CHAT);

                let payload: Value =
                    serde_json::from_str(&payload).expect("chat payload should be valid JSON");
                assert_eq!(payload["v"], 1);
                assert_eq!(payload["player"], "offline:Steve");
            }
            other => panic!("expected RPUSH command, got {other:?}"),
        }
    }

    #[test]
    fn replaces_dormant_npc_hash() {
        let entries = vec![(
            "npc_a".to_string(),
            serde_json::json!({"char_id": "npc_a"}).to_string(),
        )];
        let command = prepare_outbound_command(RedisOutbound::NpcDormantHash(entries.clone()))
            .expect("dormant hash payload should produce a hash replace command");

        match command {
            RedisIoCommand::HashReplace { key, entries: got } => {
                assert_eq!(key, NPC_DORMANT_REDIS_KEY);
                assert_eq!(got, entries);
            }
            other => panic!("expected hash replace command, got {other:?}"),
        }
    }

    /// Build `field_count` owned `(field, value)` pairs for chunking tests.
    fn dormant_field_pairs(field_count: usize) -> Vec<(String, String)> {
        (0..field_count)
            .map(|i| (format!("field{i}"), format!("value{i}")))
            .collect()
    }

    /// Borrow owned `(String, String)` pairs as `(&str, &str)` for
    /// [`chunk_hash_fields`].
    fn borrow_field_pairs(owned: &[(String, String)]) -> Vec<(&str, &str)> {
        owned
            .iter()
            .map(|(f, v)| (f.as_str(), v.as_str()))
            .collect()
    }

    #[test]
    fn hash_replace_chunks_large_payload() {
        // Contract (named by the plan): `chunk_hash_fields` must guarantee
        //   1. batch count == ceil(N / chunk)
        //   2. concatenating the batches in order reproduces the input exactly
        //      (no dropped, reordered, or duplicated fields)
        // Saturated boundaries: N = 0, 1, chunk-1, chunk, chunk+1, 1000@256.
        let chunk = 256usize;

        // (N, expected batch count). Expected counts are hand-computed
        // ceil(N/256) and cross-checked below, so the case table cannot silently
        // agree with a buggy implementation.
        let cases: &[(usize, usize)] = &[
            (0, 0),         // empty -> 0 batches
            (1, 1),         // single element -> 1 batch
            (chunk - 1, 1), // 255 -> still 1 batch
            (chunk, 1),     // 256 -> exactly 1 full batch
            (chunk + 1, 2), // 257 -> remainder spills into a 2nd batch
            (1000, 4),      // 1000 / 256 = 3.9 -> ceil = 4 batches
        ];

        for &(n, expected_batches) in cases {
            // Independently recompute ceil to catch a typo in the case table.
            let ceil_div = n.div_ceil(chunk);
            assert_eq!(
                expected_batches, ceil_div,
                "case table self-check failed: for N={n} chunk={chunk} the \
                 expected batch count should be ceil={ceil_div}, but the table \
                 says {expected_batches}"
            );

            let owned = dormant_field_pairs(n);
            let borrowed = borrow_field_pairs(&owned);
            let batches = chunk_hash_fields(&borrowed, chunk);

            // 1. batch count == ceil(N/chunk)
            assert_eq!(
                batches.len(),
                expected_batches,
                "wrong batch count: expected ceil(N/chunk)={expected_batches} \
                 (N={n} chunk={chunk}) because that many HSETs are needed, got \
                 {}",
                batches.len(),
            );

            // Every non-final batch must be exactly `chunk`; the final batch is
            // 1..=chunk (when N>0). Pins the split granularity so an off-by-one
            // in chunking is caught.
            for (i, batch) in batches.iter().enumerate() {
                if i + 1 < batches.len() {
                    assert_eq!(
                        batch.len(),
                        chunk,
                        "batch {i} is not the last, so it should be full \
                         (chunk={chunk}) but had {} (N={n})",
                        batch.len(),
                    );
                } else {
                    assert!(
                        (1..=chunk).contains(&batch.len()),
                        "the final batch size should be in 1..={chunk}, got {} \
                         (N={n})",
                        batch.len(),
                    );
                }
            }

            // 2. concatenation == input (order-preserving, lossless).
            let flattened: Vec<(&str, &str)> =
                batches.iter().flat_map(|b| b.iter().copied()).collect();
            assert_eq!(
                flattened.len(),
                n,
                "concatenated element count should equal the input N={n} \
                 because no field may be dropped or duplicated, got {}",
                flattened.len(),
            );
            assert_eq!(
                flattened, borrowed,
                "concatenated batches should equal the input element-for-element \
                 (N={n} chunk={chunk}); a mismatch means chunking reordered or \
                 corrupted the field sequence"
            );
        }
    }

    #[test]
    fn chunk_hash_fields_zero_chunk_no_panic() {
        // Contract: chunk==0 must not hit the `slice::chunks(0)` panic; it is
        // treated as one pair per batch.
        let owned = dormant_field_pairs(3);
        let borrowed = borrow_field_pairs(&owned);
        let batches = chunk_hash_fields(&borrowed, 0);
        assert_eq!(
            batches.len(),
            3,
            "chunk==0 should degrade to one pair per batch -> N=3 yields 3 \
             batches, got {} (chunks(0) guard likely missing)",
            batches.len(),
        );
        for (i, batch) in batches.iter().enumerate() {
            assert_eq!(
                batch.len(),
                1,
                "with chunk==0, batch {i} should hold exactly 1 element, got {}",
                batch.len(),
            );
        }
        // Still must reproduce the input in order.
        let flattened: Vec<(&str, &str)> = batches.iter().flat_map(|b| b.iter().copied()).collect();
        assert_eq!(
            flattened, borrowed,
            "chunk==0 batches should still concatenate back to the input"
        );
    }

    /// P0 bug① contract: a dormant `HashReplace` (which may be arbitrarily slow
    /// or time out) must NEVER be able to starve other outbound IPC. The
    /// starvation mechanism was `dispatch_outbound_command` returning `Err` on
    /// an inline command, which `drain_outbound_messages` stored into the
    /// cross-reconnect `pending_command` and retried first forever. The fix
    /// routes `HashReplace` onto the background (fire-and-forget) connection, so
    /// it is structurally impossible for it to occupy `pending_command`. A
    /// regular Publish queued after it still routes inline and is unaffected.
    #[test]
    fn dormant_hash_replace_failure_does_not_starve_other_outbound() {
        // The would-be slow / failing dormant write.
        let dormant = prepare_outbound_command(RedisOutbound::NpcDormantHash(vec![(
            "npc_slow".to_string(),
            serde_json::json!({"char_id": "npc_slow"}).to_string(),
        )]))
        .expect("dormant payload should produce a HashReplace command");
        assert!(
            runs_on_background_redis_connection(&dormant),
            "expected dormant HashReplace to be fire-and-forget on the background connection so a slow write cannot pin pending_command; got inline routing which is exactly the starvation bug"
        );

        // A subsequently-queued non-world-state Publish still routes inline and
        // is therefore drained / dispatched independently of the dormant write.
        let other = RedisIoCommand::Publish {
            channel: CH_AGENT_COMMAND,
            payload: "{}".to_string(),
        };
        assert!(
            !runs_on_background_redis_connection(&other),
            "expected a non-world_state Publish to stay on the primary inline path (it does not depend on dormant write completion); got background routing"
        );
    }

    /// Per-variant pin of the inline-vs-background routing decision, the single
    /// switch that determines whether a command can ever occupy
    /// `pending_command`. Every `RedisIoCommand` variant is exercised so a new
    /// variant or a routing regression flips a red test instead of silently
    /// re-introducing head-of-line blocking.
    #[test]
    fn outbound_routing_background_vs_inline_per_variant() {
        // world_state Publish -> background (must not block other channels).
        assert!(
            runs_on_background_redis_connection(&RedisIoCommand::Publish {
                channel: CH_WORLD_STATE,
                payload: "{}".to_string(),
            }),
            "expected world_state Publish on background connection because a large snapshot must not stall other IPC"
        );
        // dormant HashReplace (only HashReplace producer) -> background.
        assert!(
            runs_on_background_redis_connection(&RedisIoCommand::HashReplace {
                key: NPC_DORMANT_REDIS_KEY,
                entries: vec![("a".to_string(), "b".to_string())],
            }),
            "expected dormant HashReplace on background connection so a slow / failing write cannot pin pending_command"
        );
        // Non-world_state Publish -> inline (small, latency-sensitive narration/cmd channels).
        assert!(
            !runs_on_background_redis_connection(&RedisIoCommand::Publish {
                channel: CH_AGENT_COMMAND,
                payload: "{}".to_string(),
            }),
            "expected non-world_state Publish to stay inline; only world_state is large enough to warrant the background connection"
        );
        // ListPush (e.g. player_chat) -> inline.
        assert!(
            !runs_on_background_redis_connection(&RedisIoCommand::ListPush {
                key: CH_PLAYER_CHAT,
                payload: "{}".to_string(),
            }),
            "expected ListPush to stay inline; chat pushes are small and ordered"
        );
        // PublishFanout -> inline.
        assert!(
            !runs_on_background_redis_connection(&RedisIoCommand::PublishFanout {
                channels: vec![CH_AGENT_COMMAND],
                payload: "{}".to_string(),
            }),
            "expected PublishFanout to stay inline; it is not a bulk dormant/world_state write"
        );
    }

    /// P0 bug② contract: a timed-out hash replace must not leak its temp key.
    ///
    /// The leak only matters because `tokio::timeout` *drops* the in-flight
    /// `execute_hash_replace_atomic` future (skipping its inline `DEL temp`
    /// cleanup), so the cleanup the timeout branch issues and the leading `DEL`
    /// of the next attempt can only ever clear the leak if every attempt targets
    /// the *same* deterministic temp key. The previous nanosecond-nonce scheme
    /// minted a fresh `{key}:tmp:<nonce>` per call, so no later DEL could name a
    /// dropped key and they accumulated without bound. This pins the contract
    /// that makes cleanup possible: one stable `{key}:tmp` name, identical
    /// across calls, never carrying the old `:tmp:<nonce>` suffix.
    #[test]
    fn hash_replace_timeout_cleans_temp_key() {
        let temp = dormant_temp_key(NPC_DORMANT_REDIS_KEY);
        assert_eq!(
            temp, "bong:npc/dormant:tmp",
            "expected the dormant temp key to be the deterministic `{{key}}:tmp` so a timed-out write's best-effort DEL targets the exact leaked key; got `{temp}`"
        );
        assert!(
            !temp.contains(":tmp:"),
            "expected NO `:tmp:` nonce segment because a per-call nonce makes every retry mint a brand-new key that later DELs can never name (the exact temp-key leak this fix removes); got `{temp}`"
        );
        // Idempotent across calls: two replaces of the same logical key reuse
        // the one temp key, so a retry's leading DEL overwrites rather than
        // accumulates.
        assert_eq!(
            dormant_temp_key(NPC_DORMANT_REDIS_KEY),
            dormant_temp_key(NPC_DORMANT_REDIS_KEY),
            "expected dormant_temp_key to be deterministic for a given key so retries overwrite the same temp key; got differing names which would re-introduce leakage"
        );
        // Distinct logical keys still get distinct temp keys (no cross-key clobber).
        assert_ne!(
            dormant_temp_key("bong:npc/dormant"),
            dormant_temp_key("bong:other/store"),
            "expected distinct logical keys to map to distinct temp keys so unrelated replaces never collide; got identical temp keys"
        );
    }

    #[test]
    fn replaces_qi_ledger_hash_on_dedicated_key() {
        // plan-offscreen-war-v1 P0：守恒 telemetry 必须落到 bong:qi/ledger（非 dormant key）。
        let entries = vec![
            ("total_observed".to_string(), "100".to_string()),
            ("account:zone:spawn".to_string(), "50".to_string()),
        ];
        let command = prepare_outbound_command(RedisOutbound::QiLedgerHash(entries.clone()))
            .expect("qi ledger payload should produce a hash replace command");

        match command {
            RedisIoCommand::HashReplace { key, entries: got } => {
                assert_eq!(
                    key, QI_LEDGER_REDIS_KEY,
                    "守恒 telemetry 必须发到 bong:qi/ledger，不能错写到 dormant key"
                );
                assert_eq!(got, entries);
            }
            other => panic!("expected hash replace command, got {other:?}"),
        }
    }

    #[test]
    fn replaces_qi_ledger_hash_with_empty_entries_deletes_key() {
        // 边界：空 entries（全服无任何已落位真元 / 账户）必须仍产出 HashReplace（key 不变），
        // 由下游 HASHSET-replace 语义对空 entries 退化为 DEL——即 bong:qi/ledger 被清空，
        // 不会残留上一帧的 stale 行。绝不能因为空就 swallow 命令。
        let command = prepare_outbound_command(RedisOutbound::QiLedgerHash(vec![])).expect(
            "空 entries 的 qi ledger payload 仍须产出 HashReplace（触发 DEL 语义），不能被吞",
        );

        match command {
            RedisIoCommand::HashReplace { key, entries } => {
                assert_eq!(
                    key, QI_LEDGER_REDIS_KEY,
                    "空账本 telemetry 也必须落到 bong:qi/ledger，键不能漂移"
                );
                assert!(
                    entries.is_empty(),
                    "空 entries 必须原样传给 HashReplace（触发 DEL 清键），实际 entries={entries:?}"
                );
            }
            other => panic!("空 entries 期望 HashReplace（DEL 语义），实际得到 {other:?}"),
        }
    }

    #[test]
    fn publishes_zhenmai_skill_event_on_skill_channel() {
        let mut event =
            ZhenmaiSkillEventV1::new(ZhenmaiSkillIdV1::SeverChain, "entity:7".to_string(), 42);
        event.meridian_id = Some("Heart".to_string());
        event.attack_kind = Some(ZhenmaiAttackKindV1::TaintedYuan);
        event.k_drain = Some(1.5);
        event.self_damage_multiplier = Some(0.5);

        let command = prepare_outbound_command(RedisOutbound::ZhenmaiSkillEvent(event))
            .expect("zhenmai skill payload should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_ZHENMAI_SKILL_EVENT);
                let payload: Value =
                    serde_json::from_str(&payload).expect("zhenmai payload should be valid JSON");
                assert_eq!(payload["type"], "zhenmai_skill_event");
                assert_eq!(payload["skill_id"], "sever_chain");
                assert_eq!(payload["meridian_id"], "Heart");
                assert_eq!(payload["attack_kind"], "tainted_yuan");
                assert_eq!(payload["k_drain"], 1.5);
                assert_eq!(payload["self_damage_multiplier"], 0.5);
            }
            other => panic!("expected zhenmai PUBLISH command, got {other:?}"),
        }
    }

    #[test]
    fn publishes_baomai_skill_event_on_skill_channel() {
        let mut event = crate::schema::baomai_v3::BaomaiSkillEventV1::new(
            crate::schema::baomai_v3::BaomaiSkillIdV1::Disperse,
            "entity:7".to_string(),
            42,
        );
        event.flow_rate_multiplier = 10.0;
        event.qi_invested = 5350.0;
        event.meridian_ids = vec!["Ren".to_string(), "Du".to_string()];

        let command = prepare_outbound_command(RedisOutbound::BaomaiV3SkillEvent(event))
            .expect("baomai skill payload should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_BAOMAI_V3_SKILL_EVENT);
                let payload: Value =
                    serde_json::from_str(&payload).expect("baomai payload should be valid JSON");
                assert_eq!(payload["type"], "baomai_skill_event");
                assert_eq!(payload["skill_id"], "disperse");
                assert_eq!(payload["flow_rate_multiplier"], 10.0);
                assert_eq!(payload["meridian_ids"][0], "Ren");
            }
            other => panic!("expected baomai PUBLISH command, got {other:?}"),
        }
    }

    // plan-combat-skill-feedback-bridges-v1 P2 — 爆脉 v3 残余事件桥 channel pin tests

    #[test]
    fn baomai_v3_mountain_shake_publishes_on_correct_channel() {
        let mut evt = crate::schema::baomai_v3::BaomaiV3MountainShakeV1::new(
            "offline:Player".to_string(),
            3,
            200,
        );
        evt.qi_spent = 1200.0;
        evt.radius_blocks = 5.0;
        evt.shock_damage = 420.0;

        let command = prepare_outbound_command(RedisOutbound::BaomaiV3MountainShake(evt))
            .expect("mountain shake payload should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(
                    channel, CH_BAOMAI_V3_MOUNTAIN_SHAKE,
                    "mountain_shake must publish to bong:baomai_v3/mountain_shake"
                );
                let json: Value =
                    serde_json::from_str(&payload).expect("mountain shake should be valid JSON");
                assert_eq!(json["v"], 1);
                assert_eq!(json["affected_count"], 3);
                assert_eq!(json["qi_spent"], 1200.0);
                assert_eq!(json["radius_blocks"], 5.0);
                assert_eq!(json["shock_damage"], 420.0);
            }
            other => panic!("expected mountain_shake PUBLISH, got {other:?}"),
        }
    }

    #[test]
    fn baomai_v3_blood_burn_publishes_near_death_on_correct_channel() {
        let mut evt =
            crate::schema::baomai_v3::BaomaiV3BloodBurnV1::new("offline:Player".to_string(), 300);
        evt.hp_burned = 300.0;
        evt.qi_multiplier = 5.0;
        evt.active_until_tick = 300;
        evt.ended_in_near_death = true;

        let command = prepare_outbound_command(RedisOutbound::BaomaiV3BloodBurn(evt))
            .expect("blood burn payload should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(
                    channel, CH_BAOMAI_V3_BLOOD_BURN,
                    "blood_burn must publish to bong:baomai_v3/blood_burn"
                );
                let json: Value =
                    serde_json::from_str(&payload).expect("blood burn should be valid JSON");
                assert_eq!(json["ended_in_near_death"], true);
                assert_eq!(json["qi_multiplier"], 5.0);
            }
            other => panic!("expected blood_burn PUBLISH, got {other:?}"),
        }
    }

    #[test]
    fn baomai_v3_transcendence_expired_publishes_on_correct_channel() {
        let evt = crate::schema::baomai_v3::BaomaiV3TranscendenceExpiredV1::new(
            "offline:Player".to_string(),
            700,
        );

        let command = prepare_outbound_command(RedisOutbound::BaomaiV3TranscendenceExpired(evt))
            .expect("transcendence expired payload should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(
                    channel, CH_BAOMAI_V3_TRANSCENDENCE_EXPIRED,
                    "transcendence_expired must publish to bong:baomai_v3/transcendence_expired"
                );
                let json: Value = serde_json::from_str(&payload)
                    .expect("transcendence expired should be valid JSON");
                assert_eq!(json["v"], 1);
                assert_eq!(json["tick"], 700);
            }
            other => panic!("expected transcendence_expired PUBLISH, got {other:?}"),
        }
    }

    #[test]
    fn baomai_v3_overload_ripple_publishes_on_correct_channel() {
        let mut evt = crate::schema::baomai_v3::BaomaiV3OverloadRippleV1::new(
            "offline:Player".to_string(),
            150,
            crate::schema::baomai_v3::BaomaiSkillIdV1::BengQuan,
        );
        evt.severity_delta = 0.05;
        evt.total_severity = 0.35;
        evt.meridian_ids = vec!["LargeIntestine".to_string()];

        let command = prepare_outbound_command(RedisOutbound::BaomaiV3OverloadRipple(evt))
            .expect("overload ripple payload should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(
                    channel, CH_BAOMAI_V3_OVERLOAD_RIPPLE,
                    "overload_ripple must publish to bong:baomai_v3/overload_ripple"
                );
                let json: Value =
                    serde_json::from_str(&payload).expect("overload ripple should be valid JSON");
                assert_eq!(json["skill_id"], "beng_quan");
                assert_eq!(json["severity_delta"], 0.05);
                assert_eq!(json["meridian_ids"][0], "LargeIntestine");
            }
            other => panic!("expected overload_ripple PUBLISH, got {other:?}"),
        }
    }

    #[test]
    fn publishes_zhenfa_v2_event_on_dedicated_channel() {
        let event = crate::schema::zhenfa_v2::ZhenfaV2EventV1::deploy(
            7,
            crate::schema::zhenfa_v2::ZhenfaArrayKindV2::DeceiveHeaven,
            "offline:Azure",
            [1, 64, -2],
            20,
        );

        let command = prepare_outbound_command(RedisOutbound::ZhenfaV2Event(event))
            .expect("zhenfa v2 payload should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_ZHENFA_V2_EVENT);
                let payload: Value =
                    serde_json::from_str(&payload).expect("zhenfa v2 payload should be valid JSON");
                assert_eq!(payload["v"], 1);
                assert_eq!(payload["event"], "deploy");
                assert_eq!(payload["kind"], "deceive_heaven");
            }
            other => panic!("expected zhenfa v2 PUBLISH command, got {other:?}"),
        }
    }

    #[test]
    fn publishes_cultivation_events_on_correct_channels() {
        let bt = prepare_outbound_command(RedisOutbound::BreakthroughEvent(BreakthroughEventV1 {
            kind: "Succeeded".into(),
            from_realm: "Awaken".into(),
            to_realm: Some("Induce".into()),
            success_rate: Some(0.9),
            severity: None,
        }))
        .expect("breakthrough payload should serialize");
        match bt {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_BREAKTHROUGH_EVENT);
                let v: Value = serde_json::from_str(&payload).unwrap();
                assert_eq!(v["kind"], "Succeeded");
                assert_eq!(v["to_realm"], "Induce");
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let forge = prepare_outbound_command(RedisOutbound::ForgeEvent(ForgeEventV1 {
            meridian: "Lung".into(),
            axis: "Rate".into(),
            from_tier: 2,
            to_tier: 3,
            success: true,
        }))
        .expect("forge payload should serialize");
        match forge {
            RedisIoCommand::Publish { channel, .. } => assert_eq!(channel, CH_FORGE_EVENT),
            other => panic!("expected publish, got {other:?}"),
        }

        let death = prepare_outbound_command(RedisOutbound::CultivationDeath(CultivationDeathV1 {
            cause: "BreakthroughBackfire".into(),
            context: serde_json::json!({"from":"Spirit"}),
        }))
        .expect("death payload should serialize");
        match death {
            RedisIoCommand::Publish { channel, .. } => assert_eq!(channel, CH_CULTIVATION_DEATH),
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_heart_demon_requests_on_dedicated_channel() {
        let command = prepare_outbound_command(RedisOutbound::HeartDemonRequest(
            HeartDemonPregenRequestV1 {
                trigger_id: "heart_demon:1:1000".into(),
                character_id: "offline:Azure".into(),
                actor_name: "Azure".into(),
                realm: "Spirit".into(),
                qi_color_state: crate::schema::cultivation::QiColorStateV1 {
                    main: "Mellow".into(),
                    secondary: None,
                    is_chaotic: false,
                    is_hunyuan: false,
                },
                recent_biography: vec!["t240:reach:Spirit".into()],
                composure: 0.7,
                started_tick: 1000,
                waves_total: 5,
            },
        ))
        .expect("heart demon request should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_HEART_DEMON_REQUEST);
                let v: Value = serde_json::from_str(&payload).unwrap();
                assert_eq!(v["trigger_id"], "heart_demon:1:1000");
                assert_eq!(v["recent_biography"][0], "t240:reach:Spirit");
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_tiandao_hunt_narration_request_on_dedicated_channel() {
        let command = prepare_outbound_command(RedisOutbound::TiandaoHuntNarrationRequest(
            TiandaoHuntNarrationRequestV1 {
                v: 1,
                character_id: "offline:Alice".into(),
                realm: "Spirit".into(),
                attention_level: 72.5,
                response_level:
                    crate::schema::tiandao_hunt_narration::TiandaoHuntResponseLevelV1::Tribulation,
                zone: "血谷".into(),
                recent_actions: vec!["activity:meditating".into()],
                narration_count: 2,
            },
        ))
        .expect("tiandao hunt narration request should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_TIANDAO_HUNT_NARRATION_REQUEST);
                let v: Value = serde_json::from_str(&payload).unwrap();
                assert_eq!(v["character_id"], "offline:Alice");
                assert_eq!(v["response_level"], "tribulation");
                assert_eq!(v["narration_count"], 2);
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_spirit_treasure_dialogue_requests_on_dedicated_channel() {
        let request: SpiritTreasureDialogueRequestV1 = serde_json::from_str(include_str!(
            "../../../agent/packages/schema/samples/spirit-treasure-dialogue-request.sample.json"
        ))
        .expect("spirit treasure dialogue request sample should deserialize");

        let command =
            prepare_outbound_command(RedisOutbound::SpiritTreasureDialogueRequest(request))
                .expect("spirit treasure dialogue request should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_SPIRIT_TREASURE_DIALOGUE_REQUEST);
                let v: Value = serde_json::from_str(&payload).unwrap();
                assert_eq!(v["treasure_id"], "spirit_treasure_jizhaojing");
                assert_eq!(v["trigger"], "player");
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_npc_and_faction_events_on_dedicated_channels() {
        let spawned = prepare_outbound_command(RedisOutbound::NpcSpawned(NpcSpawnedV1 {
            v: 1,
            kind: "npc_spawned".to_string(),
            npc_id: "npc_1v1".to_string(),
            archetype: "rogue".to_string(),
            source: "agent_command".to_string(),
            zone: "spawn".to_string(),
            pos: [1.0, 66.0, 2.0],
            initial_age_ticks: 0.0,
            at_tick: 0,
        }))
        .expect("NPC spawn payload should serialize");
        match spawned {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_NPC_SPAWN);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["kind"], "npc_spawned");
                assert_eq!(v["archetype"], "rogue");
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let death = prepare_outbound_command(RedisOutbound::NpcDeath(NpcDeathV1 {
            v: 1,
            kind: "npc_death".to_string(),
            npc_id: "npc_1v1".to_string(),
            archetype: "commoner".to_string(),
            cause: "natural_aging".to_string(),
            faction_id: None,
            life_record_snapshot: Some("生平摘要".to_string()),
            age_ticks: 10.0,
            max_age_ticks: 10.0,
            at_tick: 0,
            from_dormant_combat: false,
            pos: None,
        }))
        .expect("NPC death payload should serialize");
        match death {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_NPC_DEATH);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["kind"], "npc_death");
                assert_eq!(v["cause"], "natural_aging");
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let faction = prepare_outbound_command(RedisOutbound::FactionEvent(FactionEventV1 {
            v: 1,
            kind: "faction_event".to_string(),
            faction_id: "attack".to_string(),
            event_kind: "adjust_loyalty_bias".to_string(),
            leader_id: None,
            loyalty_bias: 0.6,
            mission_queue_size: 1,
            at_tick: 0,
        }))
        .expect("faction payload should serialize");
        match faction {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_FACTION_EVENT);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["kind"], "faction_event");
                assert_eq!(v["event_kind"], "adjust_loyalty_bias");
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let pressure =
            prepare_outbound_command(RedisOutbound::ZonePressureCrossed(ZonePressureCrossedV1 {
                v: 1,
                kind: "zone_pressure_crossed".to_string(),
                zone: "spawn".to_string(),
                level: "high".to_string(),
                raw_pressure: 1.25,
                at_tick: 42,
            }))
            .expect("zone pressure payload should serialize");
        match pressure {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_ZONE_PRESSURE_CROSSED);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["kind"], "zone_pressure_crossed");
                assert_eq!(v["level"], "high");
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_dormant_combat_outcome_on_npc_combat_channel() {
        // plan-offscreen-war-v1 P2：离屏战果 telemetry 必须落在专属 CH_NPC_COMBAT
        // （bong:npc/combat）频道，且 payload 无损 roundtrip——e2e 验收门靠它把
        // outcome.loser 与 bong:npc/death.npc_id 对账，路由错频道或字段丢失都会让验收假过。
        let outcome = prepare_outbound_command(RedisOutbound::DormantCombatOutcome(
            DormantCombatOutcomeV1 {
                v: 1,
                kind: "dormant_combat_outcome".to_string(),
                winner: "dormant:combat:atk".to_string(),
                loser: "dormant:combat:def".to_string(),
                zone: "spawn".to_string(),
                qi_released: 4.5,
                at_tick: 4321,
            },
        ))
        .expect("DormantCombatOutcomeV1 payload should serialize");
        match outcome {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(
                    channel, CH_NPC_COMBAT,
                    "战果 telemetry 必须路由到 bong:npc/combat（CH_NPC_COMBAT），\
                     否则 e2e 的 ⑤ outcome 对账订阅不到、验收假过"
                );
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                // payload 必须无损 roundtrip：e2e 读 winner/loser/zone/qi_released 做守恒对账。
                assert_eq!(
                    v["kind"], "dormant_combat_outcome",
                    "kind 标签丢失，下游解析器无法区分 payload 类型"
                );
                assert_eq!(
                    v["winner"], "dormant:combat:atk",
                    "winner 字段须原样上线，否则对账读到错值"
                );
                assert_eq!(
                    v["loser"], "dormant:combat:def",
                    "loser 字段须 == 对应 NpcDeathV1.npc_id，e2e ⑤ 据此对账"
                );
                assert_eq!(
                    v["zone"], "spawn",
                    "zone 字段须原样上线，e2e ③ 据此读 ledger 账户"
                );
                assert_eq!(
                    v["qi_released"], 4.5,
                    "qi_released 须原样上线，e2e ③ 用它对齐 zone 回灌增量"
                );
            }
            other => panic!(
                "DormantCombatOutcome 应产出 Publish 命令，实际得到 {other:?}——\
                 路由分支若改成 HashReplace/Fanout 会让战果观测断链"
            ),
        }
    }

    #[test]
    fn publishes_pending_dormant_relic_on_npc_relic_channel() {
        // plan-offscreen-war-v1 P3（CodeRabbit）：克制式战场遗物 telemetry 必须落在专属
        // CH_NPC_RELIC（bong:npc/relic）频道，且 payload 无损 roundtrip——e2e 验收门靠它把
        // relic.char_id 与 bong:npc/death.npc_id 对账。频道名或 match 分支回退都会让 P3 观测静默
        // 失守。loot_seed 取含 high-bit 的 u64 边界值，验证 u64 序列化不被截断/丢精度。
        const HIGH_BIT_SEED: u64 = 0xFFFF_FFFF_0000_0001;
        let relic =
            prepare_outbound_command(RedisOutbound::PendingDormantRelic(PendingDormantRelicV1 {
                v: 1,
                kind: "pending_dormant_relic".to_string(),
                char_id: "dormant:fallen:disciple".to_string(),
                zone: "rift_valley".to_string(),
                pos: [12.0, 64.0, -8.0],
                archetype: "disciple".to_string(),
                loot_seed: HIGH_BIT_SEED,
                created_tick: 42,
                at_tick: 4321,
            }))
            .expect("PendingDormantRelicV1 payload should serialize");
        match relic {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(
                    channel, CH_NPC_RELIC,
                    "battlefield relic telemetry must route to bong:npc/relic (CH_NPC_RELIC); a wrong channel would make the e2e P3① relic subscription silently miss every event"
                );
                let v: Value = serde_json::from_str(payload.as_str())
                    .expect("relic publish payload must be valid JSON");
                assert_eq!(
                    v["kind"], "pending_dormant_relic",
                    "kind tag must survive so the downstream parser can discriminate payload type; got {}",
                    v["kind"]
                );
                assert_eq!(
                    v["char_id"], "dormant:fallen:disciple",
                    "char_id must round-trip unchanged (e2e ③ matches it against NpcDeathV1.npc_id); got {}",
                    v["char_id"]
                );
                assert_eq!(
                    v["zone"], "rift_valley",
                    "zone must round-trip (hydrate materialization reads it); got {}",
                    v["zone"]
                );
                assert_eq!(
                    v["pos"],
                    serde_json::json!([12.0, 64.0, -8.0]),
                    "pos must round-trip as a [f64;3] (hydrate spawns the decal/loot there); got {}",
                    v["pos"]
                );
                assert_eq!(
                    v["archetype"], "disciple",
                    "archetype must round-trip (e2e ② asserts disciple = relic-eligible); got {}",
                    v["archetype"]
                );
                // loot_seed 是 u64：serde_json 以数值输出，high-bit 值不得被截断成 i64 或丢精度。
                assert_eq!(
                    v["loot_seed"].as_u64(),
                    Some(HIGH_BIT_SEED),
                    "loot_seed must round-trip losslessly as a u64 including the high bit (deterministic loot depends on it); got {}",
                    v["loot_seed"]
                );
                assert_eq!(
                    v["created_tick"].as_u64(),
                    Some(42),
                    "created_tick must round-trip as u64; got {}",
                    v["created_tick"]
                );
                assert_eq!(
                    v["at_tick"].as_u64(),
                    Some(4321),
                    "at_tick must round-trip as u64; got {}",
                    v["at_tick"]
                );
            }
            other => panic!(
                "PendingDormantRelic must produce a Publish command; got {other:?} — switching the route to HashReplace/Fanout would break P3 relic observation"
            ),
        }
    }

    #[test]
    fn publishes_faction_state_on_faction_state_channel() {
        // plan-offscreen-war-v1 P5：散修群体消长盘面 telemetry 必须落在专属 CH_FACTION_STATE
        // （bong:faction_state）频道，且 payload 无损 roundtrip——观测脚本据此读 group_id /
        // population / status / strongest_*。路由错频道或 status 序列化丢失都会让群体消长观测断链。
        let state = prepare_outbound_command(RedisOutbound::FactionState(FactionStateV1 {
            v: 1,
            kind: "faction_state".to_string(),
            group_id: 2,
            region_descriptor: "rift_valley一带散修".to_string(),
            population: 7,
            status: crate::npc::faction::GroupStatus::Waning,
            dominant_zone: "rift_valley".to_string(),
            strongest_realm: "Solidify".to_string(),
            strongest_char_id: "dormant:rogue:3".to_string(),
            at_tick: 4321,
        }))
        .expect("FactionStateV1 payload should serialize");
        match state {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(
                    channel, CH_FACTION_STATE,
                    "群体消长盘面 telemetry 必须路由到 bong:faction_state（CH_FACTION_STATE），\
                     否则观测脚本订阅不到散修群体消长"
                );
                let v: Value = serde_json::from_str(payload.as_str())
                    .expect("faction_state publish payload must be valid JSON");
                assert_eq!(
                    v["kind"], "faction_state",
                    "kind tag must survive so downstream can discriminate payload type; got {}",
                    v["kind"]
                );
                assert_eq!(
                    v["group_id"].as_u64(),
                    Some(2),
                    "group_id must round-trip as a bare u16 number (anonymous group id, no named sect); got {}",
                    v["group_id"]
                );
                assert_eq!(
                    v["status"], "waning",
                    "status must serialize as the snake_case GroupStatus string (waning here); got {}",
                    v["status"]
                );
                assert_eq!(
                    v["region_descriptor"], "rift_valley一带散修",
                    "region_descriptor must round-trip as the anonymous \"{{zone}}一带散修\" descriptor; got {}",
                    v["region_descriptor"]
                );
                assert_eq!(
                    v["strongest_realm"], "Solidify",
                    "strongest_realm must round-trip as the emergent strongest's realm label; got {}",
                    v["strongest_realm"]
                );
                assert_eq!(
                    v["population"].as_u64(),
                    Some(7),
                    "population must round-trip as u32; got {}",
                    v["population"]
                );
            }
            other => panic!(
                "FactionState must produce a Publish command; got {other:?} — switching the route to HashReplace/Fanout would break P5 群体消长 observation"
            ),
        }
    }

    #[test]
    fn publishes_rat_phase_event_on_correct_channel() {
        let command = prepare_outbound_command(RedisOutbound::RatPhaseEvent(RatPhaseChangeEvent {
            chunk: [8, 8],
            zone: "spawn".to_string(),
            group_id: 7,
            from: RatPhase::Solitary,
            to: RatPhase::Transitioning { progress: 0 },
            rat_count: 12,
            local_qi: 0.42,
            qi_gradient: 0.31,
            tick: 12345,
        }))
        .expect("rat phase event should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_RAT_PHASE_EVENT);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["zone"], "spawn");
                assert_eq!(v["from"], "solitary");
                assert_eq!(v["to"], json!({"transitioning":{"progress":0}}));
                assert_eq!(v["rat_count"], 12);
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_tribulation_events_to_main_and_phase_channels() {
        let cases = [
            (
                TribulationEventV1::du_xu(
                    TribulationPhaseV1::Omen,
                    Some("offline:Azure".to_string()),
                    Some("Azure".to_string()),
                    Some([8.0, 66.0, 8.0]),
                    Some(0),
                    Some(5),
                    None,
                ),
                vec![CH_TRIBULATION_OMEN, CH_TRIBULATION],
            ),
            (
                TribulationEventV1::du_xu(
                    TribulationPhaseV1::Lock,
                    Some("offline:Azure".to_string()),
                    Some("Azure".to_string()),
                    Some([8.0, 66.0, 8.0]),
                    Some(0),
                    Some(5),
                    None,
                ),
                vec![CH_TRIBULATION_LOCK, CH_TRIBULATION],
            ),
            (
                TribulationEventV1::du_xu(
                    TribulationPhaseV1::HeartDemon,
                    Some("offline:Azure".to_string()),
                    Some("Azure".to_string()),
                    Some([8.0, 66.0, 8.0]),
                    Some(4),
                    Some(5),
                    None,
                ),
                vec![CH_TRIBULATION_WAVE, CH_TRIBULATION],
            ),
            (
                TribulationEventV1::du_xu(
                    TribulationPhaseV1::Settle,
                    Some("offline:Azure".to_string()),
                    None,
                    None,
                    Some(5),
                    Some(5),
                    Some(crate::schema::tribulation::DuXuResultV1 {
                        char_id: "offline:Azure".to_string(),
                        outcome: crate::schema::tribulation::DuXuOutcomeV1::Ascended,
                        killer: None,
                        waves_survived: 5,
                        reason: None,
                    }),
                ),
                vec![CH_TRIBULATION_SETTLE, CH_TRIBULATION],
            ),
        ];

        for (event, expected_channels) in cases {
            let command = prepare_outbound_command(RedisOutbound::TribulationEvent(event))
                .expect("tribulation event should serialize");
            match command {
                RedisIoCommand::PublishFanout { channels, payload } => {
                    assert_eq!(channels, expected_channels);
                    let value: Value = serde_json::from_str(payload.as_str()).unwrap();
                    assert_eq!(value["v"], 1);
                    assert_eq!(value["kind"], "du_xu");
                }
                other => panic!("expected fanout publish, got {other:?}"),
            }
        }
    }

    #[test]
    fn publishes_zone_collapse_to_main_collapse_and_phase_channels() {
        let event = TribulationEventV1::zone_collapse(
            TribulationPhaseV1::Settle,
            Some("spawn".to_string()),
            Some([8.0, 66.0, 8.0]),
        );
        let command = prepare_outbound_command(RedisOutbound::TribulationEvent(event))
            .expect("zone collapse event should serialize");

        match command {
            RedisIoCommand::PublishFanout { channels, payload } => {
                assert_eq!(
                    channels,
                    vec![
                        CH_TRIBULATION_COLLAPSE,
                        CH_TRIBULATION_SETTLE,
                        CH_TRIBULATION
                    ]
                );
                let value: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(value["kind"], "zone_collapse");
                assert_eq!(value["phase"]["kind"], "settle");
                assert_eq!(value["zone"], "spawn");
            }
            other => panic!("expected fanout publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_targeted_calamity_only_to_phase_and_main_channels() {
        let event = TribulationEventV1::targeted(
            TribulationPhaseV1::Omen,
            Some("spawn".to_string()),
            Some([8.0, 66.0, 8.0]),
        );
        let command = prepare_outbound_command(RedisOutbound::TribulationEvent(event))
            .expect("targeted calamity event should serialize");

        match command {
            RedisIoCommand::PublishFanout { channels, payload } => {
                assert_eq!(channels, vec![CH_TRIBULATION_OMEN, CH_TRIBULATION]);
                let value: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(value["kind"], "targeted");
                assert_eq!(value["phase"]["kind"], "omen");
                assert_eq!(value["zone"], "spawn");
            }
            other => panic!("expected fanout publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_wanted_player_on_correct_channel() {
        use crate::schema::identity::{
            RevealedTagKindV1, WantedPlayerEventTag, WantedPlayerEventV1,
        };
        let payload = WantedPlayerEventV1 {
            event: WantedPlayerEventTag::WantedPlayer,
            player_uuid: "11111111-1111-1111-1111-111111111111".to_string(),
            char_id: "offline:kiz".to_string(),
            identity_display_name: "毒蛊师小李".to_string(),
            identity_id: 0,
            reputation_score: -100,
            primary_tag: RevealedTagKindV1::DuguRevealed,
            tick: 24_000,
        };

        let command = prepare_outbound_command(RedisOutbound::WantedPlayer(payload))
            .expect("wanted player payload should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_WANTED_PLAYER);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["event"], "wanted_player");
                assert_eq!(v["primary_tag"], "dugu_revealed");
                assert_eq!(v["identity_id"], 0);
                assert_eq!(v["reputation_score"], -100);
                assert_eq!(v["tick"], 24_000);
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_forge_start_on_correct_channel() {
        let payload = ForgeStartPayloadV1 {
            v: 1,
            session_id: 7,
            blueprint_id: "qing_feng_v0".to_string(),
            station_id: "forge_station_42".to_string(),
            caster_id: "offline:Azure".to_string(),
            materials: vec![crate::schema::forge_bridge::ForgeMaterialStackV1 {
                material: "fan_tie".to_string(),
                count: 3,
            }],
            ts: 84_000,
        };

        let command = prepare_outbound_command(RedisOutbound::ForgeStart(payload))
            .expect("forge start payload should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_FORGE_START);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["session_id"], 7);
                assert_eq!(v["blueprint_id"], "qing_feng_v0");
                assert_eq!(v["materials"][0]["material"], "fan_tie");
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_tuike_v2_skill_event_on_correct_channel() {
        let event = TuikeSkillEventV1::new(
            "offline:Azure".to_string(),
            TuikeSkillIdV1::TransferTaint,
            FalseSkinTierV1::Ancient,
            2,
            84_000,
            TuikeSkillVisualContractV1::new(
                "bong:tuike_taint_transfer",
                "bong:ancient_skin_glow",
                "contam_transfer_hum",
                "bong-client:textures/gui/items/skill_scroll_tuike_transfer_taint.png",
            ),
        );

        let command = prepare_outbound_command(RedisOutbound::TuikeV2SkillEvent(event.clone()))
            .expect("tuike v2 skill event should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_TUIKE_V2_SKILL_EVENT);
                let parsed: TuikeSkillEventV1 =
                    serde_json::from_str(&payload).expect("tuike event payload should be valid");
                assert_eq!(parsed.caster_id, event.caster_id);
                assert_eq!(parsed.skill_id, event.skill_id);
                assert_eq!(parsed.tier, event.tier);
                assert_eq!(parsed.animation_id, event.animation_id);
                assert_eq!(parsed.particle_id, event.particle_id);
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_forge_outcome_on_correct_channel() {
        let cases = [
            ForgeOutcomePayloadV1 {
                v: 1,
                session_id: 7,
                blueprint_id: "qing_feng_v0".to_string(),
                bucket: ForgeOutcomeBucketV1::Perfect,
                weapon_item: Some("qing_feng_sword".to_string()),
                quality: 0.98,
                color: Some(crate::cultivation::components::ColorKind::Sharp),
                side_effects: vec![],
                achieved_tier: 2,
                caster_id: "offline:Azure".to_string(),
                ts: 84_020,
            },
            ForgeOutcomePayloadV1 {
                v: 1,
                session_id: 8,
                blueprint_id: "qing_feng_v0".to_string(),
                bucket: ForgeOutcomeBucketV1::Flawed,
                weapon_item: Some("iron_sword".to_string()),
                quality: 0.42,
                color: None,
                side_effects: vec!["brittle_edge".to_string()],
                achieved_tier: 1,
                caster_id: "offline:Azure".to_string(),
                ts: 84_040,
            },
        ];

        for case in cases {
            let command = prepare_outbound_command(RedisOutbound::ForgeOutcome(case))
                .expect("forge outcome payload should serialize");
            match command {
                RedisIoCommand::Publish { channel, payload } => {
                    assert_eq!(channel, CH_FORGE_OUTCOME);
                    let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                    assert_eq!(v["v"], 1);
                    assert!(matches!(
                        v["bucket"].as_str(),
                        Some("perfect") | Some("flawed")
                    ));
                }
                other => panic!("expected publish, got {other:?}"),
            }
        }
    }

    #[test]
    fn publishes_alchemy_bridge_payloads_on_correct_channels() {
        let start =
            prepare_outbound_command(RedisOutbound::AlchemySessionStart(AlchemySessionStartV1 {
                v: 1,
                session_id: "alchemy:-12:64:38:kai_mai_pill_v0".to_string(),
                recipe_id: "kai_mai_pill_v0".to_string(),
                furnace_pos: (-12, 64, 38),
                furnace_tier: 1,
                caster_id: "offline:Azure".to_string(),
                ts: 84_000,
            }))
            .expect("alchemy session start payload should serialize");
        match start {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_ALCHEMY_SESSION_START);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["recipe_id"], "kai_mai_pill_v0");
                assert_eq!(v["furnace_pos"], serde_json::json!([-12, 64, 38]));
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let end = prepare_outbound_command(RedisOutbound::AlchemySessionEnd(AlchemySessionEndV1 {
            v: 1,
            session_id: "alchemy:-12:64:38:kai_mai_pill_v0".to_string(),
            recipe_id: Some("kai_mai_pill_v0".to_string()),
            furnace_pos: (-12, 64, 38),
            furnace_tier: 1,
            caster_id: "offline:Azure".to_string(),
            bucket: crate::schema::alchemy::AlchemyOutcomeBucketV1::Explode,
            pill: None,
            quality: None,
            damage: Some(12.0),
            meridian_crack: Some(0.2),
            elapsed_ticks: 120,
            ts: 84_120,
        }))
        .expect("alchemy session end payload should serialize");
        match end {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_ALCHEMY_SESSION_END);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["bucket"], "explode");
                assert_eq!(v["damage"], 12.0);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let intervention = prepare_outbound_command(RedisOutbound::AlchemyInterventionResult(
            AlchemyInterventionResultV1 {
                v: 1,
                session_id: "alchemy:-12:64:38:kai_mai_pill_v0".to_string(),
                recipe_id: "kai_mai_pill_v0".to_string(),
                furnace_pos: (-12, 64, 38),
                caster_id: "offline:Azure".to_string(),
                intervention: crate::schema::alchemy::AlchemyInterventionV1::InjectQi { qi: 3.0 },
                temp_current: 0.6,
                qi_injected: 3.0,
                accepted: true,
                message: None,
                ts: 84_020,
            },
        ))
        .expect("alchemy intervention payload should serialize");
        match intervention {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_ALCHEMY_INTERVENTION_RESULT);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["intervention"]["kind"], "inject_qi");
                assert_eq!(v["accepted"], true);
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_death_insight_on_correct_channel() {
        let command =
            prepare_outbound_command(RedisOutbound::DeathInsight(DeathInsightRequestV1 {
                v: 1,
                request_id: "death_insight:offline:Azure:84000:3".to_string(),
                character_id: "offline:Azure".to_string(),
                at_tick: 84_000,
                cause: "cultivation:NaturalAging".to_string(),
                category: DeathInsightCategoryV1::Natural,
                realm: Some("Condense".to_string()),
                player_realm: Some("qi_refining_6".to_string()),
                zone_kind: DeathInsightZoneKindV1::Ordinary,
                death_count: 3,
                rebirth_chance: None,
                lifespan_remaining_years: Some(0.0),
                recent_biography: vec!["t83980:near_death:cultivation:NaturalAging".to_string()],
                position: None,
                known_spirit_eyes: Vec::new(),
                context: serde_json::json!({"will_terminate": true}),
            }))
            .expect("death insight payload should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_DEATH_INSIGHT);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["character_id"], "offline:Azure");
                assert_eq!(v["category"], "natural");
                assert_eq!(v["zone_kind"], "ordinary");
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_death_cinematic_on_correct_channel() {
        use crate::schema::death_cinematic::{
            DeathCinematicPhaseV1, DeathCinematicRollV1, DeathCinematicS2cV1,
            DeathCinematicZoneKindV1, DeathRollResultV1,
        };

        let command =
            prepare_outbound_command(RedisOutbound::DeathCinematic(DeathCinematicS2cV1 {
                v: 1,
                character_id: "offline:Azure".to_string(),
                phase: DeathCinematicPhaseV1::Roll,
                phase_tick: 0,
                phase_duration_ticks: 80,
                total_elapsed_ticks: 80,
                total_duration_ticks: 380,
                roll: DeathCinematicRollV1 {
                    probability: 0.65,
                    threshold: 0.65,
                    luck_value: 0.65,
                    result: DeathRollResultV1::Pending,
                },
                insight_text: vec!["尘归尘，劫未尽。".to_string()],
                is_final: false,
                death_number: 1,
                zone_kind: DeathCinematicZoneKindV1::Ordinary,
                tsy_death: false,
                rebirth_weakened_ticks: 3600,
                skip_predeath: false,
            }))
            .expect("death cinematic payload should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_DEATH_CINEMATIC);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["phase"], "roll");
                assert_eq!(v["roll"]["result"], "pending");
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_spirit_eye_migrate_on_correct_channel() {
        let command =
            prepare_outbound_command(RedisOutbound::SpiritEyeMigrate(SpiritEyeMigrateV1 {
                v: 1,
                eye_id: "spirit_eye:spawn:0".to_string(),
                from: SpiritEyePositionV1 {
                    x: 0.0,
                    y: 66.0,
                    z: 0.0,
                },
                to: SpiritEyePositionV1 {
                    x: 640.0,
                    y: 66.0,
                    z: 0.0,
                },
                reason: SpiritEyeMigrateReasonV1::UsagePressure,
                usage_pressure: 0.0,
                tick: 120,
            }))
            .expect("spirit eye migrate should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_SPIRIT_EYE_MIGRATE);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["eye_id"], "spirit_eye:spawn:0");
                assert_eq!(v["reason"], "usage_pressure");
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_lifespan_and_aging_events_on_correct_channels() {
        let lifespan = prepare_outbound_command(RedisOutbound::LifespanEvent(LifespanEventV1 {
            v: 1,
            character_id: "offline:Azure".to_string(),
            at_tick: 84_000,
            kind: LifespanEventKindV1::DeathPenalty,
            delta_years: -4,
            source: "bleed_out".to_string(),
        }))
        .expect("lifespan payload should serialize");

        match lifespan {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_LIFESPAN_EVENT);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["character_id"], "offline:Azure");
                assert_eq!(v["kind"], "death_penalty");
                assert_eq!(v["delta_years"], -4);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let aging = prepare_outbound_command(RedisOutbound::Aging(AgingEventV1 {
            v: 1,
            character_id: "offline:Azure".to_string(),
            at_tick: 84_000,
            kind: AgingEventKindV1::NaturalDeath,
            years_lived: 80.0,
            cap_by_realm: 80,
            remaining_years: 0.0,
            tick_rate_multiplier: 1.0,
            source: "online".to_string(),
        }))
        .expect("aging payload should serialize");

        match aging {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_AGING);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["character_id"], "offline:Azure");
                assert_eq!(v["kind"], "natural_death");
                assert_eq!(v["remaining_years"], 0.0);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let rebirth = prepare_outbound_command(RedisOutbound::Rebirth(RebirthEventV1 {
            v: 1,
            character_id: "offline:Azure".to_string(),
            at_tick: 84_100,
            prior_realm: "Induce".to_string(),
            new_realm: "Awaken".to_string(),
        }))
        .expect("rebirth payload should serialize");

        match rebirth {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_REBIRTH);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["character_id"], "offline:Azure");
                assert_eq!(v["prior_realm"], "Induce");
                assert_eq!(v["new_realm"], "Awaken");
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_skill_events_on_skill_channels() {
        let xp = prepare_outbound_command(RedisOutbound::SkillXpGain(SkillXpGainPayloadV1::new(
            1001,
            crate::schema::skill::SkillIdV1::Combat,
            4,
            crate::schema::skill::XpGainSourceV1::Action {
                plan_id: "combat".to_string(),
                action: "kill_npc".to_string(),
            },
        )))
        .expect("skill xp payload should serialize");
        match xp {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_SKILL_XP_GAIN);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["skill"], "combat");
                assert_eq!(v["amount"], 4);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let lv = prepare_outbound_command(RedisOutbound::SkillLvUp(SkillLvUpPayloadV1::new(
            1001,
            crate::schema::skill::SkillIdV1::Mineral,
            2,
        )))
        .expect("skill level payload should serialize");
        match lv {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_SKILL_LV_UP);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["skill"], "mineral");
                assert_eq!(v["new_lv"], 2);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let cap = prepare_outbound_command(RedisOutbound::SkillCapChanged(
            SkillCapChangedPayloadV1::new(1001, crate::schema::skill::SkillIdV1::Cultivation, 7),
        ))
        .expect("skill cap payload should serialize");
        match cap {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_SKILL_CAP_CHANGED);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["skill"], "cultivation");
                assert_eq!(v["new_cap"], 7);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let scroll = prepare_outbound_command(RedisOutbound::SkillScrollUsed(
            SkillScrollUsedPayloadV1::new(
                1001,
                "scroll:mine_cave_scrap",
                crate::schema::skill::SkillIdV1::Mineral,
                100,
                false,
            ),
        ))
        .expect("skill scroll payload should serialize");
        match scroll {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_SKILL_SCROLL_USED);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["skill"], "mineral");
                assert_eq!(v["scroll_id"], "scroll:mine_cave_scrap");
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_combat_realtime_and_summary_on_correct_channels() {
        let realtime =
            prepare_outbound_command(RedisOutbound::CombatRealtime(CombatRealtimeEventV1 {
                v: 1,
                kind: CombatRealtimeKindV1::CombatEvent,
                tick: 44,
                target_id: "offline:Crimson".to_string(),
                attacker_id: Some("offline:Azure".to_string()),
                body_part: Some(crate::schema::combat_event::CombatBodyPartV1::Chest),
                wound_kind: Some(crate::schema::combat_event::CombatWoundKindV1::Blunt),
                source: Some(CombatAttackSourceV1::Melee),
                damage: Some(20.0),
                physical_damage: None,
                contam_delta: None,
                description: Some(
                    "attack_intent offline:Azure -> offline:Crimson hit Chest with Blunt for 20.0 damage at 0.90 reach decay"
                        .to_string(),
                ),
                cause: None,
                defense_kind: None,
                defense_effectiveness: None,
                defense_contam_reduced: None,
                defense_wound_severity: None,
            }))
            .expect("combat realtime payload should serialize");
        match realtime {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_COMBAT_REALTIME);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["kind"], "combat_event");
                assert_eq!(v["tick"], 44);
                assert_eq!(v["target_id"], "offline:Crimson");
                assert_eq!(v["attacker_id"], "offline:Azure");
                assert_eq!(v["body_part"], "chest");
                assert_eq!(v["wound_kind"], "blunt");
                assert_eq!(v["damage"], 20.0);
                assert_eq!(
                    v["description"],
                    "attack_intent offline:Azure -> offline:Crimson hit Chest with Blunt for 20.0 damage at 0.90 reach decay"
                );
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let summary = prepare_outbound_command(RedisOutbound::CombatSummary(CombatSummaryV1 {
            v: 1,
            window_start_tick: 201,
            window_end_tick: 400,
            combat_event_count: 9,
            death_event_count: 2,
            damage_total: 88.0,
            contam_delta_total: 16.0,
        }))
        .expect("combat summary payload should serialize");
        match summary {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_COMBAT_SUMMARY);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["window_start_tick"], 201);
                assert_eq!(v["window_end_tick"], 400);
                assert_eq!(v["combat_event_count"], 9);
                assert_eq!(v["death_event_count"], 2);
                assert_eq!(v["damage_total"], 88.0);
                assert_eq!(v["contam_delta_total"], 16.0);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let style = prepare_outbound_command(RedisOutbound::StyleBalanceTelemetry(
            crate::schema::style_balance::StyleBalanceTelemetryEventV1 {
                v: 1,
                attacker_player_id: "offline:Azure".to_string(),
                defender_player_id: "offline:Crimson".to_string(),
                attacker_color: Some(
                    crate::schema::style_balance::StyleTelemetryColorSnapshotV1 {
                        main: crate::cultivation::components::ColorKind::Heavy,
                        secondary: Some(crate::cultivation::components::ColorKind::Solid),
                        is_chaotic: false,
                        is_hunyuan: true,
                    },
                ),
                defender_color: None,
                attacker_style: Some("baomai".to_string()),
                defender_style: Some("jiemai".to_string()),
                attacker_rejection_rate: Some(0.65),
                defender_resistance: Some(0.95),
                defender_drain_affinity: Some(0.2),
                attacker_qi: Some(20.0),
                distance_blocks: Some(3.0),
                effective_hit: Some(11.8),
                defender_lost: Some(0.59),
                defender_absorbed: Some(0.12),
                cause: "attack_intent:offline:Azure".to_string(),
                resolved_at_tick: 404,
            },
        ))
        .expect("style balance telemetry payload should serialize");
        match style {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_STYLE_BALANCE_TELEMETRY);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["attacker_player_id"], "offline:Azure");
                assert_eq!(v["defender_player_id"], "offline:Crimson");
                assert_eq!(v["attacker_color"]["main"], "Heavy");
                assert_eq!(v["attacker_color"]["is_hunyuan"], true);
                assert_eq!(v["attacker_style"], "baomai");
                assert_eq!(v["defender_style"], "jiemai");
                assert_eq!(v["attacker_rejection_rate"], 0.65);
                assert_eq!(v["defender_resistance"], 0.95);
                assert_eq!(v["defender_lost"], 0.59);
                assert!(v.get("defender_color").is_none());
                assert_eq!(v["resolved_at_tick"], 404);
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_anticheat_report_on_correct_channel() {
        let command =
            prepare_outbound_command(RedisOutbound::AntiCheatReport(AntiCheatReportV1::new(
                "offline:Azure",
                42,
                1200,
                ViolationKindV1::ReachExceeded,
                10,
                "reach: target_distance=6.200 server_max=4.000",
            )))
            .expect("anticheat payload should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_ANTICHEAT);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["type"], "anticheat_report");
                assert_eq!(v["char_id"], "offline:Azure");
                assert_eq!(v["entity_id"], 42);
                assert_eq!(v["at_tick"], 1200);
                assert_eq!(v["kind"], "reach_exceeded");
                assert_eq!(v["count"], 10);
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_armor_durability_changed_on_correct_channel() {
        let command = prepare_outbound_command(RedisOutbound::ArmorDurabilityChanged(
            ArmorDurabilityChangedV1 {
                v: 1,
                entity_id: "offline:Crimson".to_string(),
                slot: crate::schema::inventory::EquipSlotV1::Chest,
                instance_id: 88,
                template_id: "fake_spirit_hide".to_string(),
                cur: 0.0,
                max: 100.0,
                durability_ratio: 0.0,
                broken: true,
            },
        ))
        .expect("armor durability payload should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_ARMOR_DURABILITY_CHANGED);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["entity_id"], "offline:Crimson");
                assert_eq!(v["slot"], "chest");
                assert_eq!(v["instance_id"], 88);
                assert_eq!(v["template_id"], "fake_spirit_hide");
                assert_eq!(v["broken"], true);
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_pseudo_vein_events_on_dedicated_channels() {
        let snapshot =
            prepare_outbound_command(RedisOutbound::PseudoVeinSnapshot(PseudoVeinSnapshotV1 {
                v: 1,
                id: "pseudo_vein_42".to_string(),
                center_xz: [1280.0, -640.0],
                spirit_qi_current: 0.6,
                occupants: vec!["offline:Azure".to_string()],
                spawned_at_tick: 24000,
                estimated_decay_at_tick: 60000,
                season_at_spawn: crate::schema::pseudo_vein::PseudoVeinSeasonV1::SummerToWinter,
            }))
            .expect("pseudo vein snapshot payload should serialize");
        match snapshot {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_PSEUDO_VEIN_ACTIVE);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["id"], "pseudo_vein_42");
                assert_eq!(v["season_at_spawn"], "summer_to_winter");
                assert_eq!(v["spirit_qi_current"], 0.6);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let dissipate = prepare_outbound_command(RedisOutbound::PseudoVeinDissipate(
            PseudoVeinDissipateEventV1 {
                v: 1,
                id: "pseudo_vein_42".to_string(),
                center_xz: [1280.0, -640.0],
                storm_anchors: vec![[1380.0, -650.0], [1160.0, -720.0]],
                storm_duration_ticks: 9000,
                qi_redistribution: crate::schema::pseudo_vein::PseudoVeinQiRedistributionV1 {
                    refill_to_hungry_ring: 0.7,
                    collected_by_tiandao: 0.3,
                },
            },
        ))
        .expect("pseudo vein dissipate payload should serialize");
        match dissipate {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_PSEUDO_VEIN_DISSIPATE);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["id"], "pseudo_vein_42");
                assert_eq!(v["storm_anchors"].as_array().unwrap().len(), 2);
                assert_eq!(v["qi_redistribution"]["collected_by_tiandao"], 0.3);
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn rejects_pseudo_vein_dissipate_with_too_many_storm_anchors() {
        let result = prepare_outbound_command(RedisOutbound::PseudoVeinDissipate(
            PseudoVeinDissipateEventV1 {
                v: 1,
                id: "pseudo_vein_42".to_string(),
                center_xz: [1280.0, -640.0],
                storm_anchors: vec![
                    [1380.0, -650.0],
                    [1160.0, -720.0],
                    [1270.0, -600.0],
                    [1220.0, -690.0],
                ],
                storm_duration_ticks: 9000,
                qi_redistribution: crate::schema::pseudo_vein::PseudoVeinQiRedistributionV1 {
                    refill_to_hungry_ring: 0.7,
                    collected_by_tiandao: 0.3,
                },
            },
        ));

        assert!(
            result.is_err(),
            "TypeScript PseudoVeinDissipateEventV1 storm_anchors maxItems=3，4 个锚点应被 Rust outbound 校验拒绝"
        );
    }

    #[test]
    fn rejects_pseudo_vein_dissipate_with_no_storm_anchors() {
        let result = prepare_outbound_command(RedisOutbound::PseudoVeinDissipate(
            PseudoVeinDissipateEventV1 {
                v: 1,
                id: "pseudo_vein_42".to_string(),
                center_xz: [1280.0, -640.0],
                storm_anchors: vec![],
                storm_duration_ticks: 9000,
                qi_redistribution: crate::schema::pseudo_vein::PseudoVeinQiRedistributionV1 {
                    refill_to_hungry_ring: 0.7,
                    collected_by_tiandao: 0.3,
                },
            },
        ));

        assert!(
            result.is_err(),
            "TypeScript PseudoVeinDissipateEventV1 storm_anchors minItems=1，0 个锚点应被 Rust outbound 校验拒绝"
        );
    }

    #[test]
    fn publishes_pseudo_vein_dissipate_with_max_storm_anchors() {
        let dissipate = prepare_outbound_command(RedisOutbound::PseudoVeinDissipate(
            PseudoVeinDissipateEventV1 {
                v: 1,
                id: "pseudo_vein_42".to_string(),
                center_xz: [1280.0, -640.0],
                storm_anchors: vec![[1380.0, -650.0], [1160.0, -720.0], [1270.0, -600.0]],
                storm_duration_ticks: 9000,
                qi_redistribution: crate::schema::pseudo_vein::PseudoVeinQiRedistributionV1 {
                    refill_to_hungry_ring: 0.7,
                    collected_by_tiandao: 0.3,
                },
            },
        ))
        .expect("TypeScript PseudoVeinDissipateEventV1 storm_anchors maxItems=3，应接受 3 个锚点");

        match dissipate {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_PSEUDO_VEIN_DISSIPATE);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["storm_anchors"].as_array().unwrap().len(), 3);
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn rejects_pseudo_vein_dissipate_with_too_short_storm_duration() {
        let result = prepare_outbound_command(RedisOutbound::PseudoVeinDissipate(
            PseudoVeinDissipateEventV1 {
                v: 1,
                id: "pseudo_vein_42".to_string(),
                center_xz: [1280.0, -640.0],
                storm_anchors: vec![[1380.0, -650.0]],
                storm_duration_ticks: 5999,
                qi_redistribution: crate::schema::pseudo_vein::PseudoVeinQiRedistributionV1 {
                    refill_to_hungry_ring: 0.7,
                    collected_by_tiandao: 0.3,
                },
            },
        ));

        assert!(
            result.is_err(),
            "TypeScript PseudoVeinDissipateEventV1 storm_duration_ticks minimum=6000，5999 ticks 应被 Rust outbound 校验拒绝"
        );
    }

    #[test]
    fn rejects_pseudo_vein_dissipate_with_too_long_storm_duration() {
        let result = prepare_outbound_command(RedisOutbound::PseudoVeinDissipate(
            PseudoVeinDissipateEventV1 {
                v: 1,
                id: "pseudo_vein_42".to_string(),
                center_xz: [1280.0, -640.0],
                storm_anchors: vec![[1380.0, -650.0]],
                storm_duration_ticks: 12001,
                qi_redistribution: crate::schema::pseudo_vein::PseudoVeinQiRedistributionV1 {
                    refill_to_hungry_ring: 0.7,
                    collected_by_tiandao: 0.3,
                },
            },
        ));

        assert!(
            result.is_err(),
            "TypeScript PseudoVeinDissipateEventV1 storm_duration_ticks maximum=12000，12001 ticks 应被 Rust outbound 校验拒绝"
        );
    }

    #[test]
    fn publishes_pseudo_vein_dissipate_with_storm_duration_bounds() {
        for storm_duration_ticks in [6000, 12000] {
            let dissipate = prepare_outbound_command(RedisOutbound::PseudoVeinDissipate(
                PseudoVeinDissipateEventV1 {
                    v: 1,
                    id: "pseudo_vein_42".to_string(),
                    center_xz: [1280.0, -640.0],
                    storm_anchors: vec![[1380.0, -650.0]],
                    storm_duration_ticks,
                    qi_redistribution: crate::schema::pseudo_vein::PseudoVeinQiRedistributionV1 {
                        refill_to_hungry_ring: 0.7,
                        collected_by_tiandao: 0.3,
                    },
                },
            ))
            .expect("TypeScript PseudoVeinDissipateEventV1 storm_duration_ticks 边界值应被接受");

            match dissipate {
                RedisIoCommand::Publish { channel, payload } => {
                    assert_eq!(channel, CH_PSEUDO_VEIN_DISSIPATE);
                    let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                    assert_eq!(v["storm_duration_ticks"], storm_duration_ticks);
                }
                other => panic!("expected publish, got {other:?}"),
            }
        }
    }

    #[test]
    fn publishes_tsy_hostile_events_on_tsy_channel() {
        let spawned = prepare_outbound_command(RedisOutbound::TsyNpcSpawned(TsyNpcSpawnedV1 {
            v: 1,
            kind: "tsy_npc_spawned".to_string(),
            family_id: "tsy_zongmen_yiji_01".to_string(),
            archetype: crate::schema::tsy_hostile::TsyHostileArchetypeV1::GuardianRelicSentinel,
            count: 3,
            at_tick: 12000,
        }))
        .expect("TSY NPC spawned payload should serialize");
        match spawned {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_TSY_EVENT);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["kind"], "tsy_npc_spawned");
                assert_eq!(v["archetype"], "guardian_relic_sentinel");
                assert_eq!(v["count"], 3);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let phase = prepare_outbound_command(RedisOutbound::TsySentinelPhaseChanged(
            TsySentinelPhaseChangedV1 {
                v: 1,
                kind: "tsy_sentinel_phase_changed".to_string(),
                family_id: "tsy_zongmen_yiji_01".to_string(),
                container_entity_id: 42,
                phase: 1,
                max_phase: 3,
                at_tick: 12345,
            },
        ))
        .expect("TSY sentinel phase payload should serialize");
        match phase {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_TSY_EVENT);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["kind"], "tsy_sentinel_phase_changed");
                assert_eq!(v["container_entity_id"], 42);
                assert_eq!(v["phase"], 1);
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_social_events_on_social_channels() {
        let exposure =
            prepare_outbound_command(RedisOutbound::SocialExposure(SocialExposureEventV1 {
                v: 1,
                actor: "char:alice".to_string(),
                kind: ExposureKindV1::Chat,
                witnesses: vec!["char:bob".to_string()],
                tick: 120,
                zone: Some("spawn".to_string()),
            }))
            .expect("social exposure should serialize");
        match exposure {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_SOCIAL_EXPOSURE);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["kind"], "chat");
                assert_eq!(v["witnesses"][0], "char:bob");
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let pact = prepare_outbound_command(RedisOutbound::SocialPact(SocialPactEventV1 {
            v: 1,
            left: "char:alice".to_string(),
            right: "char:bob".to_string(),
            terms: "shared shelter".to_string(),
            tick: 121,
            broken: false,
        }))
        .expect("social pact should serialize");
        match pact {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_SOCIAL_PACT);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["left"], "char:alice");
                assert_eq!(v["broken"], false);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let renown =
            prepare_outbound_command(RedisOutbound::SocialRenownDelta(SocialRenownDeltaV1 {
                v: 1,
                char_id: "char:alice".to_string(),
                fame_delta: 0,
                notoriety_delta: 10,
                tags_added: vec![RenownTagV1 {
                    tag: "戮道者".to_string(),
                    weight: 10.0,
                    last_seen_tick: 120,
                    permanent: false,
                }],
                tick: 120,
                reason: "pk".to_string(),
            }))
            .expect("social renown should serialize");
        match renown {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_SOCIAL_RENOWN_DELTA);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["notoriety_delta"], 10);
                assert_eq!(v["tags_added"][0]["tag"], "戮道者");
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let milestone = prepare_outbound_command(RedisOutbound::HighRenownMilestone(
            HighRenownMilestoneEventV1 {
                v: 1,
                event: HighRenownMilestoneEventTag::HighRenownMilestone,
                player_uuid: "11111111-1111-1111-1111-111111111111".to_string(),
                char_id: "offline:kiz".to_string(),
                identity_id: 0,
                identity_display_name: "玄锋".to_string(),
                fame: 1000,
                milestone: 1000,
                identity_exposed: true,
                tick: 24_000,
                zone: Some("spawn".to_string()),
            },
        ))
        .expect("high renown milestone should serialize");
        match milestone {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_HIGH_RENOWN_MILESTONE);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["event"], "high_renown_milestone");
                assert_eq!(v["identity_display_name"], "玄锋");
                assert_eq!(v["milestone"], 1000);
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_inbound_payloads() {
        let invalid_agent_command = r#"{
            "v": 1,
            "id": "cmd_bad",
            "commands": [],
            "unexpected": true
        }"#;
        let invalid_narration = format!(
            r#"{{
                "v": 1,
                "narrations": [{{
                    "scope": "broadcast",
                    "text": "{}",
                    "style": "narration"
                }}]
            }}"#,
            "x".repeat(MAX_NARRATION_LENGTH + 1)
        );

        assert!(parse_inbound_message(CH_AGENT_COMMAND, invalid_agent_command).is_err());
        assert!(parse_inbound_message(CH_AGENT_NARRATE, &invalid_narration).is_err());

        let valid_agent_command =
            include_str!("../../../agent/packages/schema/samples/agent-command.sample.json");
        assert!(matches!(
            parse_inbound_message(CH_AGENT_COMMAND, valid_agent_command)
                .expect("valid command payload should pass"),
            Some(RedisInbound::AgentCommand(_))
        ));

        let arbiter_agent_command = r#"{
            "v": 1,
            "id": "cmd_arbiter",
            "source": "arbiter",
            "commands": []
        }"#;
        assert!(matches!(
            parse_inbound_message(CH_AGENT_COMMAND, arbiter_agent_command)
                .expect("arbiter command payload should pass"),
            Some(RedisInbound::AgentCommand(_))
        ));

        let heartbeat_override_command = r#"{
            "v": 1,
            "id": "cmd_heartbeat_override",
            "source": "arbiter",
            "commands": [{
                "type": "heartbeat_override",
                "target": "spawn",
                "params": {
                    "action": "suppress",
                    "event_type": "beast_tide",
                    "duration_ticks": 6000
                }
            }]
        }"#;
        match parse_inbound_message(CH_AGENT_COMMAND, heartbeat_override_command)
            .expect("heartbeat_override command payload should pass")
        {
            Some(RedisInbound::AgentCommand(batch)) => {
                assert_eq!(batch.commands.len(), 1);
                assert_eq!(
                    batch.commands[0].command_type,
                    CommandType::HeartbeatOverride
                );
            }
            other => panic!("expected heartbeat_override AgentCommand, got {other:?}"),
        }

        let political_narration = r#"{
            "v": 1,
            "narrations": [{
                "scope": "zone",
                "target": "spawn",
                "text": "江湖有传，血谷旧怨又添一笔，闻者只把灯挑暗。",
                "style": "political_jianghu",
                "kind": "political_jianghu"
            }]
        }"#;
        assert!(matches!(
            parse_inbound_message(CH_AGENT_NARRATE, political_narration)
                .expect("political jianghu narration payload should pass"),
            Some(RedisInbound::AgentNarration(_))
        ));

        let heart_demon_offer = r#"{
            "offer_id": "heart_demon:1:1000",
            "trigger_id": "heart_demon:1:1000",
            "trigger_label": "心魔照见",
            "realm_label": "渡虚劫 · 心魔",
            "composure": 0.7,
            "quota_remaining": 1,
            "quota_total": 1,
            "expires_at_ms": 123,
            "choices": [{
                "choice_id": "heart_demon_choice_0",
                "category": "Composure",
                "title": "守本心",
                "effect_summary": "稳住心神，回复少量当前真元",
                "flavor": "旧事浮起，仍可守心。",
                "style_hint": "稳妥"
            }]
        }"#;
        assert!(matches!(
            parse_inbound_message(CH_HEART_DEMON_OFFER, heart_demon_offer)
                .expect("heart demon offer payload should pass"),
            Some(RedisInbound::HeartDemonOffer(_))
        ));

        let spirit_treasure_dialogue = include_str!(
            "../../../agent/packages/schema/samples/spirit-treasure-dialogue.sample.json"
        );
        assert!(matches!(
            parse_inbound_message(CH_SPIRIT_TREASURE_DIALOGUE, spirit_treasure_dialogue)
                .expect("spirit treasure dialogue payload should pass"),
            Some(RedisInbound::SpiritTreasureDialogue(_))
        ));

        let invalid_spawn_npc = r#"{
            "v": 1,
            "id": "cmd_spawn_bad",
            "source": "arbiter",
            "commands": [{
                "type": "spawn_npc",
                "target": "spawn",
                "params": {}
            }]
        }"#;
        assert!(parse_inbound_message(CH_AGENT_COMMAND, invalid_spawn_npc).is_err());
    }

    #[test]
    fn reconnect_backoff_grows_and_caps() {
        let mut backoff = ReconnectBackoff::default();

        let first = backoff.next();
        let second = backoff.next();
        let third = backoff.next();

        assert_eq!(first.attempt, 1);
        assert_eq!(first.delay, RECONNECT_BACKOFF_INITIAL);
        assert_eq!(second.attempt, 2);
        assert_eq!(second.delay, Duration::from_millis(500));
        assert_eq!(third.attempt, 3);
        assert_eq!(third.delay, Duration::from_secs(1));

        let mut capped = third;
        for _ in 0..8 {
            capped = backoff.next();
        }

        assert_eq!(capped.delay, RECONNECT_BACKOFF_MAX);

        backoff.reset();
        let reset = backoff.next();
        assert_eq!(reset.attempt, 1);
        assert_eq!(reset.delay, RECONNECT_BACKOFF_INITIAL);
    }

    #[test]
    fn redact_redis_url_for_log_strips_credentials_in_bridge_logs() {
        assert_eq!(
            redact_redis_url_for_log("redis://:password@cache.internal:6380/4"),
            "cache.internal:6380"
        );
        assert_eq!(
            redact_redis_url_for_log("rediss://user:password@[::1]:6390/0?tls=true"),
            "[::1]:6390"
        );
        assert_eq!(
            redact_redis_url_for_log("not-a-redis-url"),
            "[redacted redis endpoint]"
        );
    }

    #[test]
    fn world_state_publish_uses_extended_timeout_without_slowing_other_channels() {
        assert_eq!(
            publish_timeout_for_channel(CH_WORLD_STATE),
            REDIS_WORLD_STATE_PUBLISH_TIMEOUT
        );
        assert_eq!(
            publish_timeout_for_channel(CH_AGENT_COMMAND),
            REDIS_IO_TIMEOUT
        );
        assert!(runs_on_background_redis_connection(
            &prepare_outbound_command(RedisOutbound::WorldState(sample_world_state())).unwrap()
        ));
    }

    #[test]
    fn hash_replace_uses_batch_timeout_budget() {
        assert!(
            REDIS_HASH_REPLACE_TIMEOUT > REDIS_IO_TIMEOUT,
            "dormant HASH replace writes batches and should not share the tiny per-command timeout"
        );
        // P0: dormant HashReplace now runs on the background connection
        // (fire-and-forget) so a slow / failing dormant write can never pin
        // `pending_command` and starve world_state. Expected: background == true
        // because NpcDormantHash -> HashReplace must NOT block the primary
        // outbound connection (the old behaviour, asserting !background, was the
        // exact bug this plan fixes).
        assert!(
            runs_on_background_redis_connection(
                &prepare_outbound_command(RedisOutbound::NpcDormantHash(Vec::new())).unwrap()
            ),
            "expected dormant HashReplace to run on the background connection so it cannot starve other outbound IPC; got inline routing"
        );
    }

    #[tokio::test]
    async fn finished_subscriber_stream_triggers_reconnect() {
        let sub_task = task::spawn(async { SubscriberTaskExit::StreamEnded });

        assert_eq!(
            handle_finished_subscriber_task(sub_task).await,
            BridgeLoopControl::Reconnect {
                reason: "subscriber_ended:stream_ended".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn finished_subscriber_game_channel_close_triggers_reconnect() {
        let sub_task = task::spawn(async { SubscriberTaskExit::GameChannelClosed });

        assert_eq!(
            handle_finished_subscriber_task(sub_task).await,
            BridgeLoopControl::Reconnect {
                reason: "subscriber_ended:game_channel_closed".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn cancelled_subscriber_maps_to_reconnect() {
        let sub_task = task::spawn(async {
            tokio::time::sleep(Duration::from_secs(30)).await;
            SubscriberTaskExit::StreamEnded
        });
        sub_task.abort();

        assert_eq!(
            map_subscriber_join_result(sub_task.await),
            BridgeLoopControl::Reconnect {
                reason: "subscriber_cancelled".to_string(),
            }
        );
    }

    /// plan-combat-skill-feedback-bridges-v1 P0 — MeridianSevered arm channel pin。
    ///
    /// `prepare_outbound_command(RedisOutbound::MeridianSevered(..))` 必须发到
    /// `CH_MERIDIAN_SEVERED`（"bong:meridian_severed"）而非其他频道。
    #[test]
    fn publishes_meridian_severed_on_correct_channel() {
        use crate::cultivation::meridian::severed::SeveredSource;

        let evt = MeridianSeveredEventV1::new(
            "offline:TestPlayer",
            "Lung",
            SeveredSource::CombatWound,
            9_999,
        );

        let command = prepare_outbound_command(RedisOutbound::MeridianSevered(evt))
            .expect("meridian severed payload should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(
                    channel, CH_MERIDIAN_SEVERED,
                    "MeridianSevered must publish to CH_MERIDIAN_SEVERED; got {channel:?} — \
                     changing the channel would silently break agent narration subscription"
                );
                let v: serde_json::Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["type"], "meridian_severed");
                assert_eq!(v["entity_id"], "offline:TestPlayer");
                assert_eq!(v["meridian_id"], "Lung");
                assert_eq!(v["source"], "CombatWound");
                assert_eq!(v["at_tick"], 9_999);
            }
            other => panic!(
                "expected Publish command for MeridianSevered, got {other:?} — \
                 MeridianSevered must not fan-out; single-channel publish only"
            ),
        }
    }

    // ── plan-combat-skill-feedback-bridges-v1 P1 — baomai_v4 arm channel pins ──

    /// BaomaiV4ScarCircuitFormed arm 发到正确 channel。
    #[test]
    fn publishes_baomai_v4_scar_circuit_formed_on_correct_channel() {
        use crate::combat::baomai_v4::scar_circuit::ScarCircuitKind;

        let evt = crate::schema::baomai_v4::BaomaiV4ScarCircuitFormedV1::new(
            "offline:TestPlayer",
            ScarCircuitKind::TigerMouth,
            100,
        );
        let command = prepare_outbound_command(RedisOutbound::BaomaiV4ScarCircuitFormed(evt))
            .expect("scar_circuit_formed should serialize");
        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(
                    channel, CH_BAOMAI_V4_SCAR_CIRCUIT_FORMED,
                    "BaomaiV4ScarCircuitFormed must publish to {CH_BAOMAI_V4_SCAR_CIRCUIT_FORMED}"
                );
                let v: serde_json::Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["circuit"], "tiger_mouth");
                assert_eq!(v["tick"], 100);
            }
            other => panic!("expected Publish for ScarCircuitFormed, got {other:?}"),
        }
    }

    /// BaomaiV4ScarCircuitBroken arm 发到正确 channel。
    #[test]
    fn publishes_baomai_v4_scar_circuit_broken_on_correct_channel() {
        use crate::combat::baomai_v4::events::CircuitBreakReason;
        use crate::combat::baomai_v4::scar_circuit::ScarCircuitKind;

        let evt = crate::schema::baomai_v4::BaomaiV4ScarCircuitBrokenV1::new(
            "char:999",
            ScarCircuitKind::HeartLung,
            CircuitBreakReason::Healed,
            200,
        );
        let command = prepare_outbound_command(RedisOutbound::BaomaiV4ScarCircuitBroken(evt))
            .expect("scar_circuit_broken should serialize");
        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(
                    channel, CH_BAOMAI_V4_SCAR_CIRCUIT_BROKEN,
                    "BaomaiV4ScarCircuitBroken must publish to {CH_BAOMAI_V4_SCAR_CIRCUIT_BROKEN}"
                );
                let v: serde_json::Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["reason"], "healed");
                assert_eq!(v["circuit"], "heart_lung");
            }
            other => panic!("expected Publish for ScarCircuitBroken, got {other:?}"),
        }
    }

    /// BaomaiV4IronCocoonStageUp arm 发到正确 channel。
    #[test]
    fn publishes_baomai_v4_iron_cocoon_stage_up_on_correct_channel() {
        use crate::combat::baomai_v4::iron_cocoon::IronCocoonStage;

        let evt = crate::schema::baomai_v4::BaomaiV4IronCocoonStageUpV1::new(
            "offline:TestPlayer",
            IronCocoonStage::None,
            IronCocoonStage::ToughSkin,
            50,
            300,
        );
        let command = prepare_outbound_command(RedisOutbound::BaomaiV4IronCocoonStageUp(evt))
            .expect("iron_cocoon_stage_up should serialize");
        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(
                    channel, CH_BAOMAI_V4_IRON_COCOON_STAGE_UP,
                    "BaomaiV4IronCocoonStageUp must publish to {CH_BAOMAI_V4_IRON_COCOON_STAGE_UP}"
                );
                let v: serde_json::Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["from"], "none");
                assert_eq!(v["to"], "tough_skin");
                assert_eq!(v["total_overloads"], 50);
            }
            other => panic!("expected Publish for IronCocoonStageUp, got {other:?}"),
        }
    }

    /// BaomaiV4ResonanceLock arm 发到正确 channel。
    #[test]
    fn publishes_baomai_v4_resonance_lock_on_correct_channel() {
        let evt = crate::schema::baomai_v4::BaomaiV4ResonanceLockV1::new(
            "offline:PlayerA",
            "offline:PlayerB",
            1000,
            1060,
        );
        let command = prepare_outbound_command(RedisOutbound::BaomaiV4ResonanceLock(evt))
            .expect("resonance_lock should serialize");
        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(
                    channel, CH_BAOMAI_V4_RESONANCE_LOCK,
                    "BaomaiV4ResonanceLock must publish to {CH_BAOMAI_V4_RESONANCE_LOCK}"
                );
                let v: serde_json::Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["started_at"], 1000);
                assert_eq!(v["ends_at"], 1060);
            }
            other => panic!("expected Publish for ResonanceLock, got {other:?}"),
        }
    }

    /// BaomaiV4ResonanceLockEnd arm 发到正确 channel，expired 原因 wire 形态。
    #[test]
    fn publishes_baomai_v4_resonance_lock_end_expired_on_correct_channel() {
        use crate::schema::baomai_v4::LockEndReasonWire;

        let evt = crate::schema::baomai_v4::BaomaiV4ResonanceLockEndV1::new(
            "offline:PlayerA",
            "offline:PlayerB",
            LockEndReasonWire::Expired,
            1060,
        );
        let command = prepare_outbound_command(RedisOutbound::BaomaiV4ResonanceLockEnd(evt))
            .expect("resonance_lock_end should serialize");
        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(
                    channel, CH_BAOMAI_V4_RESONANCE_LOCK_END,
                    "BaomaiV4ResonanceLockEnd must publish to {CH_BAOMAI_V4_RESONANCE_LOCK_END}"
                );
                let v: serde_json::Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["reason"]["type"], "expired");
                assert_eq!(v["tick"], 1060);
            }
            other => panic!("expected Publish for ResonanceLockEnd, got {other:?}"),
        }
    }

    // ── plan-combat-skill-feedback-bridges-v1 P3 — VoidErosionEvent arm channel pin ──

    /// `prepare_outbound_command(RedisOutbound::VoidErosionEvent(..))` 必须发布到
    /// `CH_VOID_EROSION_EVENT`（`"bong:void_erosion_event"`）。
    ///
    /// agent 天道层订阅 `bong:void_erosion_event` 以触发虚蚀叙事；若 arm 被误改为其他
    /// channel 或 arm 匹配顺序变化，agent 端将静默收不到事件。本测试锁住该契约。
    #[test]
    fn publishes_void_erosion_event_on_correct_channel() {
        use crate::schema::woliu_erosion::{VoidErosionEventV1, VoidErosionStageV1};

        let evt = VoidErosionEventV1 {
            entity: "offline:Azure".to_string(),
            from_stage: VoidErosionStageV1::LowPressure,
            to_stage: VoidErosionStageV1::VoidShadow,
            cumulative_erosion: 55.0,
            server_tick: 1234,
        };

        let command = prepare_outbound_command(RedisOutbound::VoidErosionEvent(evt))
            .expect("VoidErosionEvent payload should serialize without error");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(
                    channel, CH_VOID_EROSION_EVENT,
                    "VoidErosionEvent must publish to CH_VOID_EROSION_EVENT \
                     (\"bong:void_erosion_event\"); got {channel:?} — \
                     changing the channel silently breaks agent narration subscription"
                );
                assert_eq!(
                    channel, "bong:void_erosion_event",
                    "channel literal must stay 'bong:void_erosion_event' for agent IPC contract"
                );
                let v: serde_json::Value =
                    serde_json::from_str(payload.as_str()).expect("payload must be valid JSON");
                assert_eq!(
                    v["entity"], "offline:Azure",
                    "entity_id round-trips through payload serialization"
                );
                assert_eq!(
                    v["cumulative_erosion"], 55.0,
                    "cumulative_erosion round-trips"
                );
                assert_eq!(v["server_tick"], 1234, "server_tick round-trips");
            }
            other => panic!(
                "expected RedisIoCommand::Publish for VoidErosionEvent, got {other:?} — \
                 VoidErosionEvent must not fan-out; single-channel publish only"
            ),
        }
    }

    /// `prepare_outbound_command(RedisOutbound::HalfStepRechallengeTrigger(..))` 必须发布到
    /// `CH_HALFSTEP_RECHALLENGE`（`"bong:tribulation/halfstep_rechallenge"`）。
    ///
    /// agent 天道层订阅此 channel 以路由 player/zone scope narration；若 arm 被误改或
    /// channel 常量漂移，agent 静默收不到触发事件。本测试锁住该契约。
    #[test]
    fn publishes_halfstep_rechallenge_trigger_on_correct_channel() {
        use crate::schema::halfstep_rechallenge::HalfStepRechallengeTriggerPayloadV1;

        let payload = HalfStepRechallengeTriggerPayloadV1 {
            char_id: "offline:Azure".to_string(),
            zone_name: "qingyun_peaks".to_string(),
            zone_halfstep_count: 2,
            at_tick: 99_000,
        };

        let command = prepare_outbound_command(RedisOutbound::HalfStepRechallengeTrigger(payload))
            .expect("HalfStepRechallengeTrigger payload should serialize without error");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(
                    channel, CH_HALFSTEP_RECHALLENGE,
                    "HalfStepRechallengeTrigger must publish to CH_HALFSTEP_RECHALLENGE \
                     (\"bong:tribulation/halfstep_rechallenge\"); got {channel:?} — \
                     changing the channel silently breaks agent narration subscription"
                );
                assert_eq!(
                    channel, "bong:tribulation/halfstep_rechallenge",
                    "channel literal must stay 'bong:tribulation/halfstep_rechallenge' \
                     for agent IPC contract (plan-halfstep-rechallenge-integration-v1 P1)"
                );
                let v: serde_json::Value =
                    serde_json::from_str(payload.as_str()).expect("payload must be valid JSON");
                assert_eq!(
                    v["char_id"], "offline:Azure",
                    "char_id round-trips through payload serialization"
                );
                assert_eq!(v["zone_name"], "qingyun_peaks", "zone_name round-trips");
                assert_eq!(
                    v["zone_halfstep_count"], 2,
                    "zone_halfstep_count round-trips; agent uses this for zone echo threshold"
                );
                assert_eq!(v["at_tick"], 99_000, "at_tick round-trips");
            }
            other => panic!(
                "expected RedisIoCommand::Publish for HalfStepRechallengeTrigger, got {other:?} — \
                 HalfStepRechallengeTrigger must not fan-out; single-channel publish only"
            ),
        }
    }
}
