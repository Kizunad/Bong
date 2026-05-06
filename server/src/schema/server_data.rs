use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use super::alchemy::{
    AlchemyContaminationDataV1, AlchemyFurnaceDataV1, AlchemyOutcomeForecastDataV1,
    AlchemyOutcomeResolvedDataV1, AlchemyRecipeBookDataV1, AlchemySessionDataV1,
};
use super::botany::BotanyPlantV2RenderProfileV1;
use super::combat_carrier::CarrierStateV1;
use super::combat_hud::{
    CastSyncV1, CombatHudStateV1, DefenseWindowV1, DerivedAttrsSyncV1, EventStreamPushV1,
    QuickSlotConfigV1, SkillBarConfigV1, TechniquesSnapshotV1, TreasureEquippedV1, UnlocksSyncV1,
    WeaponBrokenV1, WeaponEquippedV1, WoundsSnapshotV1,
};
use super::common::{EventKind, MAX_PAYLOAD_BYTES};
use super::cultivation::SkillMilestoneSnapshotV1;
use super::dugu::DuguPoisonStateV1;
use super::forge::{
    ForgeBlueprintBookDataV1, ForgeOutcomeDataV1, ForgeSessionDataV1, WeaponForgeStationDataV1,
};
use super::inventory::{InventoryEventV1, InventoryItemViewV1, InventorySnapshotV1};
use super::lingtian::LingtianSessionDataV1;
use super::narration::Narration;
use super::realm_vision::{RealmVisionParamsV1, SpiritualSenseTargetsV1};
use super::skill::{
    SkillCapChangedPayloadV1, SkillEntrySnapshotV1, SkillIdV1, SkillLvUpPayloadV1,
    SkillScrollUsedPayloadV1, SkillSnapshotPayloadV1, SkillXpGainPayloadV1, XpGainSourceV1,
};
use super::social::{
    NicheGuardianBrokenV1, NicheGuardianFatigueV1, NicheIntrusionEventV1, PlayerSocialSnapshotV1,
    SocialAnonymityPayloadV1, SocialExposureEventV1, SocialFeudEventV1, SocialPactEventV1,
    SocialRenownDeltaV1, SparringInvitePayloadV1, TradeOfferPayloadV1,
};
use super::tuike::FalseSkinStateV1;
use super::woliu::VortexFieldStateV1;
use super::world_state::{PlayerPowerBreakdown, SeasonStateV1, ZoneStatusV1};
use crate::cultivation::components::ColorKind;
pub const SERVER_DATA_VERSION: u8 = 1;
pub const WELCOME_MESSAGE: &str = "Bong server connected";
pub const HEARTBEAT_MESSAGE: &str = "mock agent tick";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifespanPreviewV1 {
    pub years_lived: f64,
    pub cap_by_realm: u32,
    pub remaining_years: f64,
    pub death_penalty_years: u32,
    pub tick_rate_multiplier: f64,
    pub is_wind_candle: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeathScreenStageV1 {
    Fortune,
    Tribulation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeathScreenZoneKindV1 {
    Ordinary,
    Death,
    Negative,
}

#[derive(Debug)]
pub enum ServerDataBuildError {
    Json(serde_json::Error),
    Oversize { size: usize, max: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ServerDataType {
    Welcome,
    Heartbeat,
    Narration,
    ZoneInfo,
    EventAlert,
    PlayerState,
    UiOpen,
    CultivationDetail,
    QiColorObserved,
    InventorySnapshot,
    InventoryEvent,
    DroppedLootSync,
    BotanyHarvestProgress,
    BotanyPlantV2RenderProfiles,
    MiningProgress,
    LumberProgress,
    BotanySkill,
    AlchemyFurnace,
    AlchemySession,
    AlchemyOutcomeForecast,
    AlchemyOutcomeResolved,
    AlchemyRecipeBook,
    AlchemyContamination,
    CombatHudState,
    WoundsSnapshot,
    DefenseWindow,
    CastSync,
    QuickSlotConfig,
    SkillBarConfig,
    TechniquesSnapshot,
    UnlocksSync,
    DerivedAttrsSync,
    EventStreamPush,
    WeaponEquipped,
    WeaponBroken,
    TreasureEquipped,
    VortexState,
    DuguPoisonState,
    CarrierState,
    FalseSkinState,
    LingtianSession,
    DeathScreen,
    TerminateScreen,
    RiftPortalState,
    RiftPortalRemoved,
    ExtractStarted,
    ExtractProgress,
    ExtractCompleted,
    ExtractAborted,
    ExtractFailed,
    TsyCollapseStartedIpc,
    ContainerState,
    SearchStarted,
    SearchProgress,
    SearchCompleted,
    SearchAborted,
    SkillXpGain,
    SkillLvUp,
    SkillCapChanged,
    SkillScrollUsed,
    SkillSnapshot,
    ForgeStation,
    ForgeSession,
    ForgeOutcome,
    ForgeBlueprintBook,
    TribulationState,
    TribulationBroadcast,
    AscensionQuota,
    HeartDemonOffer,
    BurstMeridianEvent,
    SocialAnonymity,
    SocialExposure,
    SocialPact,
    SocialFeud,
    SocialRenownDelta,
    NicheIntrusion,
    NicheGuardianFatigue,
    NicheGuardianBroken,
    SparringInvite,
    TradeOffer,
    RealmVisionParams,
    SpiritualSenseTargets,
}

#[derive(Debug, Clone)]
pub enum ServerDataPayloadV1 {
    Welcome {
        message: String,
    },
    Heartbeat {
        message: String,
    },
    Narration {
        narrations: Vec<Narration>,
    },
    ZoneInfo {
        zone: String,
        spirit_qi: f64,
        danger_level: u8,
        status: ZoneStatusV1,
        active_events: Option<Vec<String>>,
        perception_text: Option<String>,
    },
    EventAlert {
        event: EventKind,
        message: String,
        zone: Option<String>,
        duration_ticks: Option<u64>,
    },
    PlayerState {
        player: Option<String>,
        realm: String,
        spirit_qi: f64,
        karma: f64,
        composite_power: f64,
        breakdown: PlayerPowerBreakdown,
        zone: String,
        local_neg_pressure: Option<f32>,
        season_state: Option<SeasonStateV1>,
        social: Option<PlayerSocialSnapshotV1>,
    },
    UiOpen {
        ui: Option<String>,
        xml: String,
    },
    /// 经脉详细快照。20 条经脉以 SoA(parallel arrays) 布局，顺序与 `MeridianId` 判别式一致
    /// (Lung=0..Liver=11, Ren=12..YangWei=19)。保持 ≤ MAX_PAYLOAD_BYTES 预算。
    CultivationDetail {
        /// 境界字面量（Awaken/Induce/Condense/Solidify/Spirit/Void，与 `Realm` 判别式对齐）。
        realm: String,
        opened: Vec<bool>,
        flow_rate: Vec<f64>,
        flow_capacity: Vec<f64>,
        integrity: Vec<f64>,
        /// 每条经脉未打通时的累积进度 0..=1（已打通恒为 1.0）。
        open_progress: Vec<f64>,
        /// 每条经脉当前裂痕条目数（0..=255，饱和）。UI 用于渲染裂痕图标密度。
        cracks_count: Vec<u8>,
        /// 整个实体的污染总量（所有 `Contamination.entries.amount` 求和）。
        contamination_total: f64,
        lifespan: Option<LifespanPreviewV1>,
        /// 最近里程碑摘要，供客户端轻量展示；空串表示暂无。
        recent_skill_milestones_summary: String,
        /// 结构化 skill milestone 列表，通常只传最近若干条。
        skill_milestones: Vec<SkillMilestoneSnapshotV1>,
        qi_color_main: ColorKind,
        qi_color_secondary: Option<ColorKind>,
        qi_color_chaotic: bool,
        qi_color_hunyuan: bool,
        practice_weights: Vec<PracticeWeightV1>,
    },
    QiColorObserved(QiColorObservedV1),
    InventorySnapshot(Box<InventorySnapshotV1>),
    InventoryEvent(Box<InventoryEventV1>),
    DroppedLootSync(Vec<DroppedLootEntryV1>),
    BotanyHarvestProgress {
        session_id: String,
        target_id: String,
        target_name: String,
        plant_kind: String,
        mode: String,
        progress: f64,
        auto_selectable: bool,
        request_pending: bool,
        interrupted: bool,
        completed: bool,
        detail: String,
        hazard_hints: Vec<String>,
        target_pos: Option<[f64; 3]>,
    },
    BotanyPlantV2RenderProfiles(Vec<BotanyPlantV2RenderProfileV1>),
    MiningProgress {
        session_id: String,
        ore_pos: [i32; 3],
        progress: f64,
        interrupted: bool,
        completed: bool,
    },
    LumberProgress {
        session_id: String,
        log_pos: [i32; 3],
        progress: f64,
        interrupted: bool,
        completed: bool,
        detail: String,
    },
    BotanySkill {
        level: u64,
        xp: u64,
        xp_to_next_level: u64,
        auto_unlock_level: u64,
    },
    AlchemyFurnace(Box<AlchemyFurnaceDataV1>),
    AlchemySession(Box<AlchemySessionDataV1>),
    AlchemyOutcomeForecast(Box<AlchemyOutcomeForecastDataV1>),
    AlchemyOutcomeResolved(Box<AlchemyOutcomeResolvedDataV1>),
    AlchemyRecipeBook(Box<AlchemyRecipeBookDataV1>),
    AlchemyContamination(Box<AlchemyContaminationDataV1>),
    CombatHudState(CombatHudStateV1),
    WoundsSnapshot(WoundsSnapshotV1),
    DefenseWindow(DefenseWindowV1),
    CastSync(CastSyncV1),
    QuickSlotConfig(QuickSlotConfigV1),
    SkillBarConfig(SkillBarConfigV1),
    TechniquesSnapshot(TechniquesSnapshotV1),
    UnlocksSync(UnlocksSyncV1),
    DerivedAttrsSync(DerivedAttrsSyncV1),
    EventStreamPush(EventStreamPushV1),
    WeaponEquipped(WeaponEquippedV1),
    WeaponBroken(WeaponBrokenV1),
    TreasureEquipped(TreasureEquippedV1),
    VortexState(VortexFieldStateV1),
    DuguPoisonState(DuguPoisonStateV1),
    CarrierState(CarrierStateV1),
    FalseSkinState(FalseSkinStateV1),
    LingtianSession(Box<LingtianSessionDataV1>),
    DeathScreen {
        visible: bool,
        cause: String,
        luck_remaining: f64,
        final_words: Vec<String>,
        countdown_until_ms: u64,
        can_reincarnate: bool,
        can_terminate: bool,
        stage: Option<DeathScreenStageV1>,
        death_number: Option<u32>,
        zone_kind: Option<DeathScreenZoneKindV1>,
        lifespan: Option<LifespanPreviewV1>,
    },
    TerminateScreen {
        visible: bool,
        final_words: String,
        epilogue: String,
        archetype_suggestion: String,
    },
    RiftPortalState(RiftPortalStateV1),
    RiftPortalRemoved(RiftPortalRemovedV1),
    ExtractStarted(ExtractStartedV1),
    ExtractProgress(ExtractProgressV1),
    ExtractCompleted(ExtractCompletedV1),
    ExtractAborted(ExtractAbortedV1),
    ExtractFailed(ExtractFailedV1),
    TsyCollapseStartedIpc(TsyCollapseStartedIpcV1),
    ContainerState(ContainerStateV1),
    SearchStarted(SearchStartedV1),
    SearchProgress(SearchProgressV1),
    SearchCompleted(SearchCompletedV1),
    SearchAborted(SearchAbortedV1),
    SkillXpGain(Box<SkillXpGainPayloadV1>),
    SkillLvUp(SkillLvUpPayloadV1),
    SkillCapChanged(SkillCapChangedPayloadV1),
    SkillScrollUsed(Box<SkillScrollUsedPayloadV1>),
    SkillSnapshot(Box<SkillSnapshotPayloadV1>),
    ForgeStation(Box<WeaponForgeStationDataV1>),
    ForgeSession(Box<ForgeSessionDataV1>),
    ForgeOutcome(Box<ForgeOutcomeDataV1>),
    ForgeBlueprintBook(Box<ForgeBlueprintBookDataV1>),
    TribulationState(TribulationStateV1),
    TribulationBroadcast(TribulationBroadcastV1),
    AscensionQuota(AscensionQuotaV1),
    HeartDemonOffer(HeartDemonOfferV1),
    BurstMeridianEvent(BurstMeridianEventV1),
    SocialAnonymity(SocialAnonymityPayloadV1),
    SocialExposure(SocialExposureEventV1),
    SocialPact(SocialPactEventV1),
    SocialFeud(SocialFeudEventV1),
    SocialRenownDelta(SocialRenownDeltaV1),
    NicheIntrusion(NicheIntrusionEventV1),
    NicheGuardianFatigue(NicheGuardianFatigueV1),
    NicheGuardianBroken(NicheGuardianBrokenV1),
    SparringInvite(SparringInvitePayloadV1),
    TradeOffer(TradeOfferPayloadV1),
    RealmVisionParams(RealmVisionParamsV1),
    SpiritualSenseTargets(SpiritualSenseTargetsV1),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeartDemonOfferChoiceV1 {
    pub choice_id: String,
    pub category: String,
    pub title: String,
    pub effect_summary: String,
    pub flavor: String,
    pub style_hint: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeartDemonOfferV1 {
    pub offer_id: String,
    pub trigger_id: String,
    pub trigger_label: String,
    pub realm_label: String,
    pub composure: f64,
    pub quota_remaining: u32,
    pub quota_total: u32,
    pub expires_at_ms: u64,
    pub choices: Vec<HeartDemonOfferChoiceV1>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TribulationBroadcastV1 {
    pub active: bool,
    pub actor_name: String,
    pub stage: String,
    pub world_x: f64,
    pub world_z: f64,
    pub expires_at_ms: u64,
    pub spectate_invite: bool,
    pub spectate_distance: f64,
}

impl TribulationBroadcastV1 {
    pub fn active(
        actor_name: impl Into<String>,
        stage: impl Into<String>,
        world_x: f64,
        world_z: f64,
        ttl_ms: u64,
    ) -> Self {
        Self {
            active: true,
            actor_name: actor_name.into(),
            stage: stage.into(),
            world_x,
            world_z,
            expires_at_ms: tribulation_broadcast_expires_at_ms(ttl_ms),
            spectate_invite: false,
            spectate_distance: 0.0,
        }
    }

    pub fn clear() -> Self {
        Self {
            active: false,
            actor_name: String::new(),
            stage: "done".to_string(),
            world_x: 0.0,
            world_z: 0.0,
            expires_at_ms: 0,
            spectate_invite: false,
            spectate_distance: 0.0,
        }
    }

    pub fn refresh(&mut self, ttl_ms: u64) {
        self.expires_at_ms = tribulation_broadcast_expires_at_ms(ttl_ms);
    }
}

fn tribulation_broadcast_expires_at_ms(ttl_ms: u64) -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
        .saturating_add(ttl_ms)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TribulationStateV1 {
    pub active: bool,
    pub char_id: String,
    pub actor_name: String,
    pub kind: String,
    pub phase: String,
    pub world_x: f64,
    pub world_z: f64,
    pub wave_current: u32,
    pub wave_total: u32,
    pub started_tick: u64,
    pub phase_started_tick: u64,
    pub next_wave_tick: u64,
    pub failed: bool,
    pub half_step_on_success: bool,
    pub participants: Vec<String>,
    pub result: Option<String>,
}

impl TribulationStateV1 {
    pub fn clear() -> Self {
        Self {
            active: false,
            char_id: String::new(),
            actor_name: String::new(),
            kind: "du_xu".to_string(),
            phase: "settle".to_string(),
            world_x: 0.0,
            world_z: 0.0,
            wave_current: 0,
            wave_total: 0,
            started_tick: 0,
            phase_started_tick: 0,
            next_wave_tick: 0,
            failed: false,
            half_step_on_success: false,
            participants: Vec::new(),
            result: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AscensionQuotaV1 {
    pub occupied_slots: u32,
    pub quota_limit: u32,
    pub available_slots: u32,
}

impl AscensionQuotaV1 {
    pub fn new(occupied_slots: u32, quota_limit: u32) -> Self {
        Self {
            occupied_slots,
            quota_limit,
            available_slots: quota_limit.saturating_sub(occupied_slots),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BurstMeridianEventV1 {
    pub skill: String,
    pub caster: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub tick: u64,
    pub overload_ratio: f64,
    pub integrity_snapshot: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QiColorObservedV1 {
    pub observer: String,
    pub observed: String,
    pub main: ColorKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<ColorKind>,
    pub is_chaotic: bool,
    pub is_hunyuan: bool,
    pub realm_diff: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PracticeWeightV1 {
    pub color: ColorKind,
    pub weight: f64,
    pub ratio: f64,
}

fn default_qi_color_main() -> ColorKind {
    ColorKind::Mellow
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "type", rename_all = "snake_case")]
enum ServerDataPayloadWireV1 {
    Welcome {
        message: String,
    },
    Heartbeat {
        message: String,
    },
    Narration {
        narrations: Vec<Narration>,
    },
    ZoneInfo {
        zone: String,
        spirit_qi: f64,
        danger_level: u8,
        #[serde(default)]
        status: ZoneStatusV1,
        #[serde(skip_serializing_if = "Option::is_none")]
        active_events: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        perception_text: Option<String>,
    },
    EventAlert {
        event: EventKind,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        zone: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ticks: Option<u64>,
    },
    PlayerState {
        #[serde(skip_serializing_if = "Option::is_none")]
        player: Option<String>,
        realm: String,
        spirit_qi: f64,
        karma: f64,
        composite_power: f64,
        breakdown: PlayerPowerBreakdown,
        zone: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        local_neg_pressure: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        season_state: Option<SeasonStateV1>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        social: Option<PlayerSocialSnapshotV1>,
    },
    UiOpen {
        #[serde(skip_serializing_if = "Option::is_none")]
        ui: Option<String>,
        xml: String,
    },
    CultivationDetail {
        realm: String,
        opened: Vec<bool>,
        flow_rate: Vec<f64>,
        flow_capacity: Vec<f64>,
        integrity: Vec<f64>,
        open_progress: Vec<f64>,
        cracks_count: Vec<u8>,
        contamination_total: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        lifespan: Option<LifespanPreviewV1>,
        #[serde(default)]
        recent_skill_milestones_summary: String,
        #[serde(default)]
        skill_milestones: Vec<SkillMilestoneSnapshotV1>,
        #[serde(default = "default_qi_color_main")]
        qi_color_main: ColorKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        qi_color_secondary: Option<ColorKind>,
        #[serde(default)]
        qi_color_chaotic: bool,
        #[serde(default)]
        qi_color_hunyuan: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        practice_weights: Vec<PracticeWeightV1>,
    },
    QiColorObserved {
        #[serde(flatten)]
        observed: QiColorObservedV1,
    },
    InventorySnapshot {
        #[serde(flatten)]
        snapshot: Box<InventorySnapshotV1>,
    },
    InventoryEvent {
        #[serde(flatten)]
        event: ServerDataInventoryEventWireV1,
    },
    DroppedLootSync {
        drops: Vec<DroppedLootEntryV1>,
    },
    BotanyHarvestProgress {
        session_id: String,
        target_id: String,
        target_name: String,
        plant_kind: String,
        mode: String,
        progress: f64,
        auto_selectable: bool,
        request_pending: bool,
        interrupted: bool,
        completed: bool,
        detail: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        hazard_hints: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_pos: Option<[f64; 3]>,
    },
    BotanyPlantV2RenderProfiles {
        profiles: Vec<BotanyPlantV2RenderProfileV1>,
    },
    MiningProgress {
        session_id: String,
        ore_pos: [i32; 3],
        progress: f64,
        interrupted: bool,
        completed: bool,
    },
    LumberProgress {
        session_id: String,
        log_pos: [i32; 3],
        progress: f64,
        interrupted: bool,
        completed: bool,
        detail: String,
    },
    BotanySkill {
        level: u64,
        xp: u64,
        xp_to_next_level: u64,
        auto_unlock_level: u64,
    },
    AlchemyFurnace {
        #[serde(flatten)]
        data: Box<AlchemyFurnaceDataV1>,
    },
    AlchemySession {
        #[serde(flatten)]
        data: Box<AlchemySessionDataV1>,
    },
    AlchemyOutcomeForecast {
        #[serde(flatten)]
        data: Box<AlchemyOutcomeForecastDataV1>,
    },
    AlchemyOutcomeResolved {
        #[serde(flatten)]
        data: Box<AlchemyOutcomeResolvedDataV1>,
    },
    AlchemyRecipeBook {
        #[serde(flatten)]
        data: Box<AlchemyRecipeBookDataV1>,
    },
    AlchemyContamination {
        #[serde(flatten)]
        data: Box<AlchemyContaminationDataV1>,
    },
    CombatHudState {
        #[serde(flatten)]
        state: CombatHudStateV1,
    },
    WoundsSnapshot {
        #[serde(flatten)]
        snapshot: WoundsSnapshotV1,
    },
    DefenseWindow {
        #[serde(flatten)]
        window: DefenseWindowV1,
    },
    CastSync {
        #[serde(flatten)]
        state: CastSyncV1,
    },
    // 显式 rename 因为默认 snake_case 会得到 "quick_slot_config"，
    // 但 plan §11.4 / client handler 注册的是无下划线 "quickslot_config"。
    #[serde(rename = "quickslot_config")]
    QuickSlotConfig {
        #[serde(flatten)]
        config: QuickSlotConfigV1,
    },
    #[serde(rename = "skillbar_config")]
    SkillBarConfig {
        #[serde(flatten)]
        config: SkillBarConfigV1,
    },
    TechniquesSnapshot {
        #[serde(flatten)]
        snapshot: TechniquesSnapshotV1,
    },
    UnlocksSync {
        #[serde(flatten)]
        unlocks: UnlocksSyncV1,
    },
    DerivedAttrsSync {
        #[serde(flatten)]
        attrs: DerivedAttrsSyncV1,
    },
    EventStreamPush {
        #[serde(flatten)]
        event: EventStreamPushV1,
    },
    WeaponEquipped {
        #[serde(flatten)]
        weapon_equipped: WeaponEquippedV1,
    },
    WeaponBroken {
        #[serde(flatten)]
        weapon_broken: WeaponBrokenV1,
    },
    TreasureEquipped {
        #[serde(flatten)]
        treasure_equipped: TreasureEquippedV1,
    },
    VortexState {
        #[serde(flatten)]
        state: VortexFieldStateV1,
    },
    DuguPoisonState {
        #[serde(flatten)]
        state: DuguPoisonStateV1,
    },
    CarrierState {
        #[serde(flatten)]
        state: CarrierStateV1,
    },
    FalseSkinState {
        #[serde(flatten)]
        state: FalseSkinStateV1,
    },
    LingtianSession {
        #[serde(flatten)]
        lingtian_session: LingtianSessionDataV1,
    },
    DeathScreen {
        visible: bool,
        cause: String,
        luck_remaining: f64,
        final_words: Vec<String>,
        countdown_until_ms: u64,
        can_reincarnate: bool,
        can_terminate: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stage: Option<DeathScreenStageV1>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        death_number: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        zone_kind: Option<DeathScreenZoneKindV1>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lifespan: Option<LifespanPreviewV1>,
    },
    TerminateScreen {
        visible: bool,
        final_words: String,
        epilogue: String,
        archetype_suggestion: String,
    },
    RiftPortalState {
        #[serde(flatten)]
        state: RiftPortalStateV1,
    },
    RiftPortalRemoved {
        #[serde(flatten)]
        removed: RiftPortalRemovedV1,
    },
    ExtractStarted {
        #[serde(flatten)]
        data: ExtractStartedV1,
    },
    ExtractProgress {
        #[serde(flatten)]
        data: ExtractProgressV1,
    },
    ExtractCompleted {
        #[serde(flatten)]
        data: ExtractCompletedV1,
    },
    ExtractAborted {
        #[serde(flatten)]
        data: ExtractAbortedV1,
    },
    ExtractFailed {
        #[serde(flatten)]
        data: ExtractFailedV1,
    },
    TsyCollapseStartedIpc {
        #[serde(flatten)]
        data: TsyCollapseStartedIpcV1,
    },
    ContainerState {
        #[serde(flatten)]
        data: ContainerStateV1,
    },
    SearchStarted {
        #[serde(flatten)]
        data: SearchStartedV1,
    },
    SearchProgress {
        #[serde(flatten)]
        data: SearchProgressV1,
    },
    SearchCompleted {
        #[serde(flatten)]
        data: SearchCompletedV1,
    },
    SearchAborted {
        #[serde(flatten)]
        data: SearchAbortedV1,
    },
    SkillXpGain {
        char_id: u64,
        skill: SkillIdV1,
        amount: u32,
        source: XpGainSourceV1,
    },
    SkillLvUp {
        char_id: u64,
        skill: SkillIdV1,
        new_lv: u8,
    },
    SkillCapChanged {
        char_id: u64,
        skill: SkillIdV1,
        new_cap: u8,
    },
    SkillScrollUsed {
        char_id: u64,
        scroll_id: String,
        skill: SkillIdV1,
        xp_granted: u32,
        was_duplicate: bool,
    },
    SkillSnapshot {
        char_id: u64,
        skills: std::collections::BTreeMap<String, SkillEntrySnapshotV1>,
        consumed_scrolls: Vec<String>,
    },
    ForgeStation {
        #[serde(flatten)]
        data: Box<WeaponForgeStationDataV1>,
    },
    ForgeSession {
        #[serde(flatten)]
        data: Box<ForgeSessionDataV1>,
    },
    ForgeOutcome {
        #[serde(flatten)]
        data: Box<ForgeOutcomeDataV1>,
    },
    ForgeBlueprintBook {
        #[serde(flatten)]
        data: Box<ForgeBlueprintBookDataV1>,
    },
    TribulationState {
        #[serde(flatten)]
        data: TribulationStateV1,
    },
    TribulationBroadcast {
        #[serde(flatten)]
        data: TribulationBroadcastV1,
    },
    AscensionQuota {
        #[serde(flatten)]
        data: AscensionQuotaV1,
    },
    HeartDemonOffer {
        #[serde(flatten)]
        data: HeartDemonOfferV1,
    },
    BurstMeridianEvent {
        #[serde(flatten)]
        event: BurstMeridianEventV1,
    },
    SocialAnonymity {
        #[serde(flatten)]
        payload: SocialAnonymityPayloadV1,
    },
    SocialExposure {
        actor: String,
        kind: super::social::ExposureKindV1,
        witnesses: Vec<String>,
        tick: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        zone: Option<String>,
    },
    SocialPact {
        left: String,
        right: String,
        terms: String,
        tick: u64,
        broken: bool,
    },
    SocialFeud {
        left: String,
        right: String,
        tick: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        place: Option<String>,
    },
    SocialRenownDelta {
        char_id: String,
        fame_delta: i32,
        notoriety_delta: i32,
        #[serde(default)]
        tags_added: Vec<super::social::RenownTagV1>,
        tick: u64,
        reason: String,
    },
    NicheIntrusion {
        niche_pos: [i32; 3],
        intruder_id: String,
        #[serde(default)]
        items_taken: Vec<u64>,
        taint_delta: f32,
    },
    NicheGuardianFatigue {
        guardian_kind: super::social::GuardianKindV1,
        charges_remaining: u8,
    },
    NicheGuardianBroken {
        guardian_kind: super::social::GuardianKindV1,
        intruder_id: String,
    },
    SparringInvite {
        #[serde(flatten)]
        invite: SparringInvitePayloadV1,
    },
    TradeOffer {
        #[serde(flatten)]
        offer: TradeOfferPayloadV1,
    },
    RealmVisionParams {
        #[serde(flatten)]
        params: RealmVisionParamsV1,
    },
    SpiritualSenseTargets {
        #[serde(flatten)]
        targets: SpiritualSenseTargetsV1,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum InventoryEventKindWireV1 {
    Moved,
    Dropped,
    StackChanged,
    DurabilityChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ServerDataInventoryEventWireV1 {
    kind: InventoryEventKindWireV1,
    revision: u64,
    instance_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<super::inventory::InventoryLocationV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<super::inventory::InventoryLocationV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    world_pos: Option<[f64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    item: Option<Box<super::inventory::InventoryItemViewV1>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stack_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    durability: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DroppedLootEntryV1 {
    pub instance_id: u64,
    pub source_container_id: String,
    pub source_row: u64,
    pub source_col: u64,
    pub world_pos: [f64; 3],
    pub item: InventoryItemViewV1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiftPortalKindV1 {
    MainRift,
    DeepRift,
    CollapseTear,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiftPortalDirectionV1 {
    Entry,
    Exit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RiftPortalStateV1 {
    pub entity_id: u64,
    pub kind: RiftPortalKindV1,
    pub direction: RiftPortalDirectionV1,
    pub family_id: String,
    pub world_pos: [f64; 3],
    pub trigger_radius: f64,
    pub current_extract_ticks: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation_window_end: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RiftPortalRemovedV1 {
    pub entity_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExtractStartedV1 {
    pub player_id: String,
    pub portal_entity_id: u64,
    pub portal_kind: RiftPortalKindV1,
    pub required_ticks: u32,
    pub at_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExtractProgressV1 {
    pub player_id: String,
    pub portal_entity_id: u64,
    pub elapsed_ticks: u32,
    pub required_ticks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExtractCompletedV1 {
    pub player_id: String,
    pub portal_kind: RiftPortalKindV1,
    pub family_id: String,
    pub exit_world_pos: [f64; 3],
    pub at_tick: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtractAbortedReasonV1 {
    Moved,
    Combat,
    Damaged,
    Cancelled,
    PortalExpired,
    OutOfRange,
    NotInTsy,
    AlreadyBusy,
    CannotExit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExtractAbortedV1 {
    pub player_id: String,
    pub reason: ExtractAbortedReasonV1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtractFailedReasonV1 {
    SpiritQiDrained,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExtractFailedV1 {
    pub player_id: String,
    pub reason: ExtractFailedReasonV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TsyCollapseStartedIpcV1 {
    pub family_id: String,
    pub at_tick: u64,
    pub remaining_ticks: u64,
    pub collapse_tear_entity_ids: Vec<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContainerKindV1 {
    DryCorpse,
    Skeleton,
    StoragePouch,
    StoneCasket,
    RelicCore,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KeyKindV1 {
    StoneCasketKey,
    JadeCoffinSeal,
    ArrayCoreSigil,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchAbortReasonV1 {
    Moved,
    Combat,
    Damaged,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ContainerStateV1 {
    pub entity_id: u64,
    pub kind: ContainerKindV1,
    pub family_id: String,
    pub world_pos: [f64; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked: Option<KeyKindV1>,
    pub depleted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub searched_by_player_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SearchStartedV1 {
    pub player_id: String,
    pub container_entity_id: u64,
    pub required_ticks: u32,
    pub at_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SearchProgressV1 {
    pub player_id: String,
    pub container_entity_id: u64,
    pub elapsed_ticks: u32,
    pub required_ticks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LootPreviewItemV1 {
    pub template_id: String,
    pub display_name: String,
    pub stack_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SearchCompletedV1 {
    pub player_id: String,
    pub container_entity_id: u64,
    pub family_id: String,
    pub loot_preview: Vec<LootPreviewItemV1>,
    pub at_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SearchAbortedV1 {
    pub player_id: String,
    pub container_entity_id: u64,
    pub reason: SearchAbortReasonV1,
    pub at_tick: u64,
}

impl TryFrom<ServerDataInventoryEventWireV1> for InventoryEventV1 {
    type Error = String;

    fn try_from(value: ServerDataInventoryEventWireV1) -> Result<Self, Self::Error> {
        let raw = serde_json::to_value(value).map_err(|err| err.to_string())?;
        serde_json::from_value(raw).map_err(|err| err.to_string())
    }
}

impl From<&InventoryEventV1> for ServerDataInventoryEventWireV1 {
    fn from(value: &InventoryEventV1) -> Self {
        match value {
            InventoryEventV1::Moved {
                revision,
                instance_id,
                from,
                to,
            } => Self {
                kind: InventoryEventKindWireV1::Moved,
                revision: *revision,
                instance_id: *instance_id,
                from: Some(from.clone()),
                to: Some(to.clone()),
                world_pos: None,
                item: None,
                stack_count: None,
                durability: None,
            },
            InventoryEventV1::Dropped {
                revision,
                instance_id,
                from,
                world_pos,
                item,
            } => Self {
                kind: InventoryEventKindWireV1::Dropped,
                revision: *revision,
                instance_id: *instance_id,
                from: Some(from.clone()),
                to: None,
                world_pos: Some(*world_pos),
                item: Some(Box::new(item.clone())),
                stack_count: None,
                durability: None,
            },
            InventoryEventV1::StackChanged {
                revision,
                instance_id,
                stack_count,
            } => Self {
                kind: InventoryEventKindWireV1::StackChanged,
                revision: *revision,
                instance_id: *instance_id,
                from: None,
                to: None,
                world_pos: None,
                item: None,
                stack_count: Some(*stack_count),
                durability: None,
            },
            InventoryEventV1::DurabilityChanged {
                revision,
                instance_id,
                durability,
            } => Self {
                kind: InventoryEventKindWireV1::DurabilityChanged,
                revision: *revision,
                instance_id: *instance_id,
                from: None,
                to: None,
                world_pos: None,
                item: None,
                stack_count: None,
                durability: Some(*durability),
            },
        }
    }
}

impl TryFrom<ServerDataPayloadWireV1> for ServerDataPayloadV1 {
    type Error = String;

    fn try_from(value: ServerDataPayloadWireV1) -> Result<Self, Self::Error> {
        match value {
            ServerDataPayloadWireV1::Welcome { message } => Ok(Self::Welcome { message }),
            ServerDataPayloadWireV1::Heartbeat { message } => Ok(Self::Heartbeat { message }),
            ServerDataPayloadWireV1::Narration { narrations } => Ok(Self::Narration { narrations }),
            ServerDataPayloadWireV1::ZoneInfo {
                zone,
                spirit_qi,
                danger_level,
                status,
                active_events,
                perception_text,
            } => Ok(Self::ZoneInfo {
                zone,
                spirit_qi,
                danger_level,
                status,
                active_events,
                perception_text,
            }),
            ServerDataPayloadWireV1::EventAlert {
                event,
                message,
                zone,
                duration_ticks,
            } => Ok(Self::EventAlert {
                event,
                message,
                zone,
                duration_ticks,
            }),
            ServerDataPayloadWireV1::PlayerState {
                player,
                realm,
                spirit_qi,
                karma,
                composite_power,
                breakdown,
                zone,
                local_neg_pressure,
                season_state,
                social,
            } => Ok(Self::PlayerState {
                player,
                realm,
                spirit_qi,
                karma,
                composite_power,
                breakdown,
                zone,
                local_neg_pressure,
                season_state,
                social,
            }),
            ServerDataPayloadWireV1::UiOpen { ui, xml } => Ok(Self::UiOpen { ui, xml }),
            ServerDataPayloadWireV1::CultivationDetail {
                realm,
                opened,
                flow_rate,
                flow_capacity,
                integrity,
                open_progress,
                cracks_count,
                contamination_total,
                lifespan,
                recent_skill_milestones_summary,
                skill_milestones,
                qi_color_main,
                qi_color_secondary,
                qi_color_chaotic,
                qi_color_hunyuan,
                practice_weights,
            } => Ok(Self::CultivationDetail {
                realm,
                opened,
                flow_rate,
                flow_capacity,
                integrity,
                open_progress,
                cracks_count,
                contamination_total,
                lifespan,
                recent_skill_milestones_summary,
                skill_milestones,
                qi_color_main,
                qi_color_secondary,
                qi_color_chaotic,
                qi_color_hunyuan,
                practice_weights,
            }),
            ServerDataPayloadWireV1::QiColorObserved { observed } => {
                Ok(Self::QiColorObserved(observed))
            }
            ServerDataPayloadWireV1::InventorySnapshot { snapshot } => {
                Ok(Self::InventorySnapshot(snapshot))
            }
            ServerDataPayloadWireV1::InventoryEvent { event } => {
                Ok(Self::InventoryEvent(Box::new(event.try_into()?)))
            }
            ServerDataPayloadWireV1::DroppedLootSync { drops } => Ok(Self::DroppedLootSync(drops)),
            ServerDataPayloadWireV1::BotanyHarvestProgress {
                session_id,
                target_id,
                target_name,
                plant_kind,
                mode,
                progress,
                auto_selectable,
                request_pending,
                interrupted,
                completed,
                detail,
                hazard_hints,
                target_pos,
            } => Ok(Self::BotanyHarvestProgress {
                session_id,
                target_id,
                target_name,
                plant_kind,
                mode,
                progress,
                auto_selectable,
                request_pending,
                interrupted,
                completed,
                detail,
                hazard_hints,
                target_pos,
            }),
            ServerDataPayloadWireV1::BotanyPlantV2RenderProfiles { profiles } => {
                Ok(Self::BotanyPlantV2RenderProfiles(profiles))
            }
            ServerDataPayloadWireV1::MiningProgress {
                session_id,
                ore_pos,
                progress,
                interrupted,
                completed,
            } => Ok(Self::MiningProgress {
                session_id,
                ore_pos,
                progress,
                interrupted,
                completed,
            }),
            ServerDataPayloadWireV1::LumberProgress {
                session_id,
                log_pos,
                progress,
                interrupted,
                completed,
                detail,
            } => Ok(Self::LumberProgress {
                session_id,
                log_pos,
                progress,
                interrupted,
                completed,
                detail,
            }),
            ServerDataPayloadWireV1::BotanySkill {
                level,
                xp,
                xp_to_next_level,
                auto_unlock_level,
            } => Ok(Self::BotanySkill {
                level,
                xp,
                xp_to_next_level,
                auto_unlock_level,
            }),
            ServerDataPayloadWireV1::AlchemyFurnace { data } => Ok(Self::AlchemyFurnace(data)),
            ServerDataPayloadWireV1::AlchemySession { data } => Ok(Self::AlchemySession(data)),
            ServerDataPayloadWireV1::AlchemyOutcomeForecast { data } => {
                Ok(Self::AlchemyOutcomeForecast(data))
            }
            ServerDataPayloadWireV1::AlchemyOutcomeResolved { data } => {
                Ok(Self::AlchemyOutcomeResolved(data))
            }
            ServerDataPayloadWireV1::AlchemyRecipeBook { data } => {
                Ok(Self::AlchemyRecipeBook(data))
            }
            ServerDataPayloadWireV1::AlchemyContamination { data } => {
                Ok(Self::AlchemyContamination(data))
            }
            ServerDataPayloadWireV1::CombatHudState { state } => Ok(Self::CombatHudState(state)),
            ServerDataPayloadWireV1::WoundsSnapshot { snapshot } => {
                Ok(Self::WoundsSnapshot(snapshot))
            }
            ServerDataPayloadWireV1::DefenseWindow { window } => Ok(Self::DefenseWindow(window)),
            ServerDataPayloadWireV1::CastSync { state } => Ok(Self::CastSync(state)),
            ServerDataPayloadWireV1::QuickSlotConfig { config } => {
                Ok(Self::QuickSlotConfig(config))
            }
            ServerDataPayloadWireV1::SkillBarConfig { config } => Ok(Self::SkillBarConfig(config)),
            ServerDataPayloadWireV1::TechniquesSnapshot { snapshot } => {
                Ok(Self::TechniquesSnapshot(snapshot))
            }
            ServerDataPayloadWireV1::UnlocksSync { unlocks } => Ok(Self::UnlocksSync(unlocks)),
            ServerDataPayloadWireV1::DerivedAttrsSync { attrs } => {
                Ok(Self::DerivedAttrsSync(attrs))
            }
            ServerDataPayloadWireV1::EventStreamPush { event } => Ok(Self::EventStreamPush(event)),
            ServerDataPayloadWireV1::WeaponEquipped { weapon_equipped } => {
                Ok(Self::WeaponEquipped(weapon_equipped))
            }
            ServerDataPayloadWireV1::WeaponBroken { weapon_broken } => {
                Ok(Self::WeaponBroken(weapon_broken))
            }
            ServerDataPayloadWireV1::TreasureEquipped { treasure_equipped } => {
                Ok(Self::TreasureEquipped(treasure_equipped))
            }
            ServerDataPayloadWireV1::VortexState { state } => Ok(Self::VortexState(state)),
            ServerDataPayloadWireV1::DuguPoisonState { state } => Ok(Self::DuguPoisonState(state)),
            ServerDataPayloadWireV1::CarrierState { state } => Ok(Self::CarrierState(state)),
            ServerDataPayloadWireV1::FalseSkinState { state } => Ok(Self::FalseSkinState(state)),
            ServerDataPayloadWireV1::LingtianSession { lingtian_session } => {
                Ok(Self::LingtianSession(Box::new(lingtian_session)))
            }
            ServerDataPayloadWireV1::DeathScreen {
                visible,
                cause,
                luck_remaining,
                final_words,
                countdown_until_ms,
                can_reincarnate,
                can_terminate,
                stage,
                death_number,
                zone_kind,
                lifespan,
            } => Ok(Self::DeathScreen {
                visible,
                cause,
                luck_remaining,
                final_words,
                countdown_until_ms,
                can_reincarnate,
                can_terminate,
                stage,
                death_number,
                zone_kind,
                lifespan,
            }),
            ServerDataPayloadWireV1::TerminateScreen {
                visible,
                final_words,
                epilogue,
                archetype_suggestion,
            } => Ok(Self::TerminateScreen {
                visible,
                final_words,
                epilogue,
                archetype_suggestion,
            }),
            ServerDataPayloadWireV1::RiftPortalState { state } => Ok(Self::RiftPortalState(state)),
            ServerDataPayloadWireV1::RiftPortalRemoved { removed } => {
                Ok(Self::RiftPortalRemoved(removed))
            }
            ServerDataPayloadWireV1::ExtractStarted { data } => Ok(Self::ExtractStarted(data)),
            ServerDataPayloadWireV1::ExtractProgress { data } => Ok(Self::ExtractProgress(data)),
            ServerDataPayloadWireV1::ExtractCompleted { data } => Ok(Self::ExtractCompleted(data)),
            ServerDataPayloadWireV1::ExtractAborted { data } => Ok(Self::ExtractAborted(data)),
            ServerDataPayloadWireV1::ExtractFailed { data } => Ok(Self::ExtractFailed(data)),
            ServerDataPayloadWireV1::TsyCollapseStartedIpc { data } => {
                Ok(Self::TsyCollapseStartedIpc(data))
            }
            ServerDataPayloadWireV1::ContainerState { data } => Ok(Self::ContainerState(data)),
            ServerDataPayloadWireV1::SearchStarted { data } => Ok(Self::SearchStarted(data)),
            ServerDataPayloadWireV1::SearchProgress { data } => Ok(Self::SearchProgress(data)),
            ServerDataPayloadWireV1::SearchCompleted { data } => Ok(Self::SearchCompleted(data)),
            ServerDataPayloadWireV1::SearchAborted { data } => Ok(Self::SearchAborted(data)),
            ServerDataPayloadWireV1::SkillXpGain {
                char_id,
                skill,
                amount,
                source,
            } => Ok(Self::SkillXpGain(Box::new(SkillXpGainPayloadV1::new(
                char_id, skill, amount, source,
            )))),
            ServerDataPayloadWireV1::SkillLvUp {
                char_id,
                skill,
                new_lv,
            } => Ok(Self::SkillLvUp(SkillLvUpPayloadV1::new(
                char_id, skill, new_lv,
            ))),
            ServerDataPayloadWireV1::SkillCapChanged {
                char_id,
                skill,
                new_cap,
            } => Ok(Self::SkillCapChanged(SkillCapChangedPayloadV1::new(
                char_id, skill, new_cap,
            ))),
            ServerDataPayloadWireV1::SkillScrollUsed {
                char_id,
                scroll_id,
                skill,
                xp_granted,
                was_duplicate,
            } => Ok(Self::SkillScrollUsed(Box::new(
                SkillScrollUsedPayloadV1::new(char_id, scroll_id, skill, xp_granted, was_duplicate),
            ))),
            ServerDataPayloadWireV1::SkillSnapshot {
                char_id,
                skills,
                consumed_scrolls,
            } => Ok(Self::SkillSnapshot(Box::new(SkillSnapshotPayloadV1::new(
                char_id,
                skills,
                consumed_scrolls,
            )))),
            ServerDataPayloadWireV1::ForgeStation { data } => Ok(Self::ForgeStation(data)),
            ServerDataPayloadWireV1::ForgeSession { data } => Ok(Self::ForgeSession(data)),
            ServerDataPayloadWireV1::ForgeOutcome { data } => Ok(Self::ForgeOutcome(data)),
            ServerDataPayloadWireV1::ForgeBlueprintBook { data } => {
                Ok(Self::ForgeBlueprintBook(data))
            }
            ServerDataPayloadWireV1::TribulationState { data } => Ok(Self::TribulationState(data)),
            ServerDataPayloadWireV1::TribulationBroadcast { data } => {
                Ok(Self::TribulationBroadcast(data))
            }
            ServerDataPayloadWireV1::AscensionQuota { data } => Ok(Self::AscensionQuota(data)),
            ServerDataPayloadWireV1::HeartDemonOffer { data } => Ok(Self::HeartDemonOffer(data)),
            ServerDataPayloadWireV1::BurstMeridianEvent { event } => {
                validate_burst_meridian_event(&event)?;
                Ok(Self::BurstMeridianEvent(event))
            }
            ServerDataPayloadWireV1::SocialAnonymity { payload } => {
                Ok(Self::SocialAnonymity(payload))
            }
            ServerDataPayloadWireV1::SocialExposure {
                actor,
                kind,
                witnesses,
                tick,
                zone,
            } => Ok(Self::SocialExposure(SocialExposureEventV1 {
                v: 1,
                actor,
                kind,
                witnesses,
                tick,
                zone,
            })),
            ServerDataPayloadWireV1::SocialPact {
                left,
                right,
                terms,
                tick,
                broken,
            } => Ok(Self::SocialPact(SocialPactEventV1 {
                v: 1,
                left,
                right,
                terms,
                tick,
                broken,
            })),
            ServerDataPayloadWireV1::SocialFeud {
                left,
                right,
                tick,
                place,
            } => Ok(Self::SocialFeud(SocialFeudEventV1 {
                v: 1,
                left,
                right,
                tick,
                place,
            })),
            ServerDataPayloadWireV1::SocialRenownDelta {
                char_id,
                fame_delta,
                notoriety_delta,
                tags_added,
                tick,
                reason,
            } => Ok(Self::SocialRenownDelta(SocialRenownDeltaV1 {
                v: 1,
                char_id,
                fame_delta,
                notoriety_delta,
                tags_added,
                tick,
                reason,
            })),
            ServerDataPayloadWireV1::NicheIntrusion {
                niche_pos,
                intruder_id,
                items_taken,
                taint_delta,
            } => Ok(Self::NicheIntrusion(NicheIntrusionEventV1 {
                v: 1,
                niche_pos,
                intruder_id,
                items_taken,
                taint_delta,
            })),
            ServerDataPayloadWireV1::NicheGuardianFatigue {
                guardian_kind,
                charges_remaining,
            } => Ok(Self::NicheGuardianFatigue(NicheGuardianFatigueV1 {
                v: 1,
                guardian_kind,
                charges_remaining,
            })),
            ServerDataPayloadWireV1::NicheGuardianBroken {
                guardian_kind,
                intruder_id,
            } => Ok(Self::NicheGuardianBroken(NicheGuardianBrokenV1 {
                v: 1,
                guardian_kind,
                intruder_id,
            })),
            ServerDataPayloadWireV1::SparringInvite { invite } => Ok(Self::SparringInvite(invite)),
            ServerDataPayloadWireV1::TradeOffer { offer } => Ok(Self::TradeOffer(offer)),
            ServerDataPayloadWireV1::RealmVisionParams { params } => {
                Ok(Self::RealmVisionParams(params))
            }
            ServerDataPayloadWireV1::SpiritualSenseTargets { targets } => {
                Ok(Self::SpiritualSenseTargets(targets))
            }
        }
    }
}

impl From<&ServerDataPayloadV1> for ServerDataPayloadWireV1 {
    fn from(value: &ServerDataPayloadV1) -> Self {
        match value {
            ServerDataPayloadV1::Welcome { message } => Self::Welcome {
                message: message.clone(),
            },
            ServerDataPayloadV1::Heartbeat { message } => Self::Heartbeat {
                message: message.clone(),
            },
            ServerDataPayloadV1::Narration { narrations } => Self::Narration {
                narrations: narrations.clone(),
            },
            ServerDataPayloadV1::ZoneInfo {
                zone,
                spirit_qi,
                danger_level,
                status,
                active_events,
                perception_text,
            } => Self::ZoneInfo {
                zone: zone.clone(),
                spirit_qi: *spirit_qi,
                danger_level: *danger_level,
                status: *status,
                active_events: active_events.clone(),
                perception_text: perception_text.clone(),
            },
            ServerDataPayloadV1::EventAlert {
                event,
                message,
                zone,
                duration_ticks,
            } => Self::EventAlert {
                event: event.clone(),
                message: message.clone(),
                zone: zone.clone(),
                duration_ticks: *duration_ticks,
            },
            ServerDataPayloadV1::PlayerState {
                player,
                realm,
                spirit_qi,
                karma,
                composite_power,
                breakdown,
                zone,
                local_neg_pressure,
                season_state,
                social,
            } => Self::PlayerState {
                player: player.clone(),
                realm: realm.clone(),
                spirit_qi: *spirit_qi,
                karma: *karma,
                composite_power: *composite_power,
                breakdown: breakdown.clone(),
                zone: zone.clone(),
                local_neg_pressure: *local_neg_pressure,
                season_state: *season_state,
                social: social.clone(),
            },
            ServerDataPayloadV1::UiOpen { ui, xml } => Self::UiOpen {
                ui: ui.clone(),
                xml: xml.clone(),
            },
            ServerDataPayloadV1::CultivationDetail {
                realm,
                opened,
                flow_rate,
                flow_capacity,
                integrity,
                open_progress,
                cracks_count,
                contamination_total,
                lifespan,
                recent_skill_milestones_summary,
                skill_milestones,
                qi_color_main,
                qi_color_secondary,
                qi_color_chaotic,
                qi_color_hunyuan,
                practice_weights,
            } => Self::CultivationDetail {
                realm: realm.clone(),
                opened: opened.clone(),
                flow_rate: flow_rate.clone(),
                flow_capacity: flow_capacity.clone(),
                integrity: integrity.clone(),
                open_progress: open_progress.clone(),
                cracks_count: cracks_count.clone(),
                contamination_total: *contamination_total,
                lifespan: lifespan.clone(),
                recent_skill_milestones_summary: recent_skill_milestones_summary.clone(),
                skill_milestones: skill_milestones.clone(),
                qi_color_main: *qi_color_main,
                qi_color_secondary: *qi_color_secondary,
                qi_color_chaotic: *qi_color_chaotic,
                qi_color_hunyuan: *qi_color_hunyuan,
                practice_weights: practice_weights.clone(),
            },
            ServerDataPayloadV1::QiColorObserved(observed) => Self::QiColorObserved {
                observed: observed.clone(),
            },
            ServerDataPayloadV1::InventorySnapshot(snapshot) => Self::InventorySnapshot {
                snapshot: snapshot.clone(),
            },
            ServerDataPayloadV1::InventoryEvent(event) => Self::InventoryEvent {
                event: event.as_ref().into(),
            },
            ServerDataPayloadV1::DroppedLootSync(drops) => Self::DroppedLootSync {
                drops: drops.clone(),
            },
            ServerDataPayloadV1::BotanyHarvestProgress {
                session_id,
                target_id,
                target_name,
                plant_kind,
                mode,
                progress,
                auto_selectable,
                request_pending,
                interrupted,
                completed,
                detail,
                hazard_hints,
                target_pos,
            } => Self::BotanyHarvestProgress {
                session_id: session_id.clone(),
                target_id: target_id.clone(),
                target_name: target_name.clone(),
                plant_kind: plant_kind.clone(),
                mode: mode.clone(),
                progress: *progress,
                auto_selectable: *auto_selectable,
                request_pending: *request_pending,
                interrupted: *interrupted,
                completed: *completed,
                detail: detail.clone(),
                hazard_hints: hazard_hints.clone(),
                target_pos: *target_pos,
            },
            ServerDataPayloadV1::BotanyPlantV2RenderProfiles(profiles) => {
                Self::BotanyPlantV2RenderProfiles {
                    profiles: profiles.clone(),
                }
            }
            ServerDataPayloadV1::MiningProgress {
                session_id,
                ore_pos,
                progress,
                interrupted,
                completed,
            } => Self::MiningProgress {
                session_id: session_id.clone(),
                ore_pos: *ore_pos,
                progress: *progress,
                interrupted: *interrupted,
                completed: *completed,
            },
            ServerDataPayloadV1::LumberProgress {
                session_id,
                log_pos,
                progress,
                interrupted,
                completed,
                detail,
            } => Self::LumberProgress {
                session_id: session_id.clone(),
                log_pos: *log_pos,
                progress: *progress,
                interrupted: *interrupted,
                completed: *completed,
                detail: detail.clone(),
            },
            ServerDataPayloadV1::BotanySkill {
                level,
                xp,
                xp_to_next_level,
                auto_unlock_level,
            } => Self::BotanySkill {
                level: *level,
                xp: *xp,
                xp_to_next_level: *xp_to_next_level,
                auto_unlock_level: *auto_unlock_level,
            },
            ServerDataPayloadV1::AlchemyFurnace(data) => {
                Self::AlchemyFurnace { data: data.clone() }
            }
            ServerDataPayloadV1::AlchemySession(data) => {
                Self::AlchemySession { data: data.clone() }
            }
            ServerDataPayloadV1::AlchemyOutcomeForecast(data) => {
                Self::AlchemyOutcomeForecast { data: data.clone() }
            }
            ServerDataPayloadV1::AlchemyOutcomeResolved(data) => {
                Self::AlchemyOutcomeResolved { data: data.clone() }
            }
            ServerDataPayloadV1::AlchemyRecipeBook(data) => {
                Self::AlchemyRecipeBook { data: data.clone() }
            }
            ServerDataPayloadV1::AlchemyContamination(data) => {
                Self::AlchemyContamination { data: data.clone() }
            }
            ServerDataPayloadV1::CombatHudState(state) => Self::CombatHudState { state: *state },
            ServerDataPayloadV1::WoundsSnapshot(snapshot) => Self::WoundsSnapshot {
                snapshot: snapshot.clone(),
            },
            ServerDataPayloadV1::DefenseWindow(window) => Self::DefenseWindow { window: *window },
            ServerDataPayloadV1::CastSync(state) => Self::CastSync { state: *state },
            ServerDataPayloadV1::QuickSlotConfig(config) => Self::QuickSlotConfig {
                config: config.clone(),
            },
            ServerDataPayloadV1::SkillBarConfig(config) => Self::SkillBarConfig {
                config: config.clone(),
            },
            ServerDataPayloadV1::TechniquesSnapshot(snapshot) => Self::TechniquesSnapshot {
                snapshot: snapshot.clone(),
            },
            ServerDataPayloadV1::UnlocksSync(unlocks) => Self::UnlocksSync { unlocks: *unlocks },
            ServerDataPayloadV1::DerivedAttrsSync(attrs) => Self::DerivedAttrsSync {
                attrs: attrs.clone(),
            },
            ServerDataPayloadV1::EventStreamPush(event) => Self::EventStreamPush {
                event: event.clone(),
            },
            ServerDataPayloadV1::WeaponEquipped(w) => Self::WeaponEquipped {
                weapon_equipped: w.clone(),
            },
            ServerDataPayloadV1::WeaponBroken(b) => Self::WeaponBroken {
                weapon_broken: b.clone(),
            },
            ServerDataPayloadV1::TreasureEquipped(t) => Self::TreasureEquipped {
                treasure_equipped: t.clone(),
            },
            ServerDataPayloadV1::VortexState(state) => Self::VortexState {
                state: state.clone(),
            },
            ServerDataPayloadV1::DuguPoisonState(state) => Self::DuguPoisonState {
                state: state.clone(),
            },
            ServerDataPayloadV1::CarrierState(state) => Self::CarrierState {
                state: state.clone(),
            },
            ServerDataPayloadV1::FalseSkinState(state) => Self::FalseSkinState {
                state: state.clone(),
            },
            ServerDataPayloadV1::LingtianSession(s) => Self::LingtianSession {
                lingtian_session: (**s).clone(),
            },
            ServerDataPayloadV1::DeathScreen {
                visible,
                cause,
                luck_remaining,
                final_words,
                countdown_until_ms,
                can_reincarnate,
                can_terminate,
                stage,
                death_number,
                zone_kind,
                lifespan,
            } => Self::DeathScreen {
                visible: *visible,
                cause: cause.clone(),
                luck_remaining: *luck_remaining,
                final_words: final_words.clone(),
                countdown_until_ms: *countdown_until_ms,
                can_reincarnate: *can_reincarnate,
                can_terminate: *can_terminate,
                stage: stage.clone(),
                death_number: *death_number,
                zone_kind: zone_kind.clone(),
                lifespan: lifespan.clone(),
            },
            ServerDataPayloadV1::TerminateScreen {
                visible,
                final_words,
                epilogue,
                archetype_suggestion,
            } => Self::TerminateScreen {
                visible: *visible,
                final_words: final_words.clone(),
                epilogue: epilogue.clone(),
                archetype_suggestion: archetype_suggestion.clone(),
            },
            ServerDataPayloadV1::RiftPortalState(state) => Self::RiftPortalState {
                state: state.clone(),
            },
            ServerDataPayloadV1::RiftPortalRemoved(removed) => Self::RiftPortalRemoved {
                removed: removed.clone(),
            },
            ServerDataPayloadV1::ExtractStarted(data) => {
                Self::ExtractStarted { data: data.clone() }
            }
            ServerDataPayloadV1::ExtractProgress(data) => {
                Self::ExtractProgress { data: data.clone() }
            }
            ServerDataPayloadV1::ExtractCompleted(data) => {
                Self::ExtractCompleted { data: data.clone() }
            }
            ServerDataPayloadV1::ExtractAborted(data) => {
                Self::ExtractAborted { data: data.clone() }
            }
            ServerDataPayloadV1::ExtractFailed(data) => Self::ExtractFailed { data: data.clone() },
            ServerDataPayloadV1::TsyCollapseStartedIpc(data) => {
                Self::TsyCollapseStartedIpc { data: data.clone() }
            }
            ServerDataPayloadV1::ContainerState(data) => {
                Self::ContainerState { data: data.clone() }
            }
            ServerDataPayloadV1::SearchStarted(data) => Self::SearchStarted { data: data.clone() },
            ServerDataPayloadV1::SearchProgress(data) => {
                Self::SearchProgress { data: data.clone() }
            }
            ServerDataPayloadV1::SearchCompleted(data) => {
                Self::SearchCompleted { data: data.clone() }
            }
            ServerDataPayloadV1::SearchAborted(data) => Self::SearchAborted { data: data.clone() },
            ServerDataPayloadV1::SkillXpGain(data) => Self::SkillXpGain {
                char_id: data.char_id,
                skill: data.skill,
                amount: data.amount,
                source: data.source.clone(),
            },
            ServerDataPayloadV1::SkillLvUp(data) => Self::SkillLvUp {
                char_id: data.char_id,
                skill: data.skill,
                new_lv: data.new_lv,
            },
            ServerDataPayloadV1::SkillCapChanged(data) => Self::SkillCapChanged {
                char_id: data.char_id,
                skill: data.skill,
                new_cap: data.new_cap,
            },
            ServerDataPayloadV1::SkillScrollUsed(data) => Self::SkillScrollUsed {
                char_id: data.char_id,
                scroll_id: data.scroll_id.clone(),
                skill: data.skill,
                xp_granted: data.xp_granted,
                was_duplicate: data.was_duplicate,
            },
            ServerDataPayloadV1::SkillSnapshot(data) => Self::SkillSnapshot {
                char_id: data.char_id,
                skills: data.skills.clone(),
                consumed_scrolls: data.consumed_scrolls.clone(),
            },
            ServerDataPayloadV1::ForgeStation(data) => Self::ForgeStation { data: data.clone() },
            ServerDataPayloadV1::ForgeSession(data) => Self::ForgeSession { data: data.clone() },
            ServerDataPayloadV1::ForgeOutcome(data) => Self::ForgeOutcome { data: data.clone() },
            ServerDataPayloadV1::ForgeBlueprintBook(data) => {
                Self::ForgeBlueprintBook { data: data.clone() }
            }
            ServerDataPayloadV1::TribulationState(data) => {
                Self::TribulationState { data: data.clone() }
            }
            ServerDataPayloadV1::TribulationBroadcast(data) => {
                Self::TribulationBroadcast { data: data.clone() }
            }
            ServerDataPayloadV1::AscensionQuota(data) => Self::AscensionQuota { data: *data },
            ServerDataPayloadV1::HeartDemonOffer(data) => {
                Self::HeartDemonOffer { data: data.clone() }
            }
            ServerDataPayloadV1::BurstMeridianEvent(event) => Self::BurstMeridianEvent {
                event: event.clone(),
            },
            ServerDataPayloadV1::SocialAnonymity(payload) => Self::SocialAnonymity {
                payload: payload.clone(),
            },
            ServerDataPayloadV1::SocialExposure(event) => Self::SocialExposure {
                actor: event.actor.clone(),
                kind: event.kind,
                witnesses: event.witnesses.clone(),
                tick: event.tick,
                zone: event.zone.clone(),
            },
            ServerDataPayloadV1::SocialPact(event) => Self::SocialPact {
                left: event.left.clone(),
                right: event.right.clone(),
                terms: event.terms.clone(),
                tick: event.tick,
                broken: event.broken,
            },
            ServerDataPayloadV1::SocialFeud(event) => Self::SocialFeud {
                left: event.left.clone(),
                right: event.right.clone(),
                tick: event.tick,
                place: event.place.clone(),
            },
            ServerDataPayloadV1::SocialRenownDelta(event) => Self::SocialRenownDelta {
                char_id: event.char_id.clone(),
                fame_delta: event.fame_delta,
                notoriety_delta: event.notoriety_delta,
                tags_added: event.tags_added.clone(),
                tick: event.tick,
                reason: event.reason.clone(),
            },
            ServerDataPayloadV1::NicheIntrusion(event) => Self::NicheIntrusion {
                niche_pos: event.niche_pos,
                intruder_id: event.intruder_id.clone(),
                items_taken: event.items_taken.clone(),
                taint_delta: event.taint_delta,
            },
            ServerDataPayloadV1::NicheGuardianFatigue(event) => Self::NicheGuardianFatigue {
                guardian_kind: event.guardian_kind,
                charges_remaining: event.charges_remaining,
            },
            ServerDataPayloadV1::NicheGuardianBroken(event) => Self::NicheGuardianBroken {
                guardian_kind: event.guardian_kind,
                intruder_id: event.intruder_id.clone(),
            },
            ServerDataPayloadV1::SparringInvite(invite) => Self::SparringInvite {
                invite: invite.clone(),
            },
            ServerDataPayloadV1::TradeOffer(offer) => Self::TradeOffer {
                offer: offer.clone(),
            },
            ServerDataPayloadV1::RealmVisionParams(params) => Self::RealmVisionParams {
                params: params.clone(),
            },
            ServerDataPayloadV1::SpiritualSenseTargets(targets) => Self::SpiritualSenseTargets {
                targets: targets.clone(),
            },
        }
    }
}

fn validate_burst_meridian_event(event: &BurstMeridianEventV1) -> Result<(), String> {
    if event.skill.trim().is_empty() {
        return Err("BurstMeridianEventV1.skill must not be empty".to_string());
    }
    if event.caster.trim().is_empty() {
        return Err("BurstMeridianEventV1.caster must not be empty".to_string());
    }
    if event.target.as_deref().is_some_and(str::is_empty) {
        return Err("BurstMeridianEventV1.target must not be empty when present".to_string());
    }
    if !event.overload_ratio.is_finite() || event.overload_ratio < 0.0 {
        return Err("BurstMeridianEventV1.overload_ratio must be finite and >= 0".to_string());
    }
    if !event.integrity_snapshot.is_finite() || !(0.0..=1.0).contains(&event.integrity_snapshot) {
        return Err("BurstMeridianEventV1.integrity_snapshot must be finite in 0..=1".to_string());
    }
    Ok(())
}

impl Serialize for ServerDataPayloadV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ServerDataPayloadWireV1::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ServerDataPayloadV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ServerDataPayloadWireV1::deserialize(deserializer)?;
        wire.try_into().map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerDataV1 {
    #[serde(deserialize_with = "deserialize_server_data_version")]
    pub v: u8,
    #[serde(flatten)]
    pub payload: ServerDataPayloadV1,
}

fn deserialize_server_data_version<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let version = u8::deserialize(deserializer)?;
    if version == SERVER_DATA_VERSION {
        Ok(version)
    } else {
        Err(D::Error::custom(format!(
            "ServerDataV1.v must be {SERVER_DATA_VERSION}, got {version}"
        )))
    }
}

impl ServerDataV1 {
    pub fn new(payload: ServerDataPayloadV1) -> Self {
        Self {
            v: SERVER_DATA_VERSION,
            payload,
        }
    }

    pub fn welcome(message: impl Into<String>) -> Self {
        Self::new(ServerDataPayloadV1::Welcome {
            message: message.into(),
        })
    }

    pub fn heartbeat(message: impl Into<String>) -> Self {
        Self::new(ServerDataPayloadV1::Heartbeat {
            message: message.into(),
        })
    }

    pub fn payload_type(&self) -> ServerDataType {
        self.payload.payload_type()
    }

    pub fn to_json_bytes_checked(&self) -> Result<Vec<u8>, ServerDataBuildError> {
        let bytes = serde_json::to_vec(self).map_err(ServerDataBuildError::Json)?;
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(ServerDataBuildError::Oversize {
                size: bytes.len(),
                max: MAX_PAYLOAD_BYTES,
            });
        }

        Ok(bytes)
    }
}

impl ServerDataPayloadV1 {
    pub fn payload_type(&self) -> ServerDataType {
        match self {
            Self::Welcome { .. } => ServerDataType::Welcome,
            Self::Heartbeat { .. } => ServerDataType::Heartbeat,
            Self::Narration { .. } => ServerDataType::Narration,
            Self::ZoneInfo { .. } => ServerDataType::ZoneInfo,
            Self::EventAlert { .. } => ServerDataType::EventAlert,
            Self::PlayerState { .. } => ServerDataType::PlayerState,
            Self::UiOpen { .. } => ServerDataType::UiOpen,
            Self::CultivationDetail { .. } => ServerDataType::CultivationDetail,
            Self::QiColorObserved(..) => ServerDataType::QiColorObserved,
            Self::InventorySnapshot(..) => ServerDataType::InventorySnapshot,
            Self::InventoryEvent(..) => ServerDataType::InventoryEvent,
            Self::DroppedLootSync(..) => ServerDataType::DroppedLootSync,
            Self::BotanyHarvestProgress { .. } => ServerDataType::BotanyHarvestProgress,
            Self::BotanyPlantV2RenderProfiles(..) => ServerDataType::BotanyPlantV2RenderProfiles,
            Self::MiningProgress { .. } => ServerDataType::MiningProgress,
            Self::LumberProgress { .. } => ServerDataType::LumberProgress,
            Self::BotanySkill { .. } => ServerDataType::BotanySkill,
            Self::AlchemyFurnace(..) => ServerDataType::AlchemyFurnace,
            Self::AlchemySession(..) => ServerDataType::AlchemySession,
            Self::AlchemyOutcomeForecast(..) => ServerDataType::AlchemyOutcomeForecast,
            Self::AlchemyOutcomeResolved(..) => ServerDataType::AlchemyOutcomeResolved,
            Self::AlchemyRecipeBook(..) => ServerDataType::AlchemyRecipeBook,
            Self::AlchemyContamination(..) => ServerDataType::AlchemyContamination,
            Self::CombatHudState(..) => ServerDataType::CombatHudState,
            Self::WoundsSnapshot(..) => ServerDataType::WoundsSnapshot,
            Self::DefenseWindow(..) => ServerDataType::DefenseWindow,
            Self::CastSync(..) => ServerDataType::CastSync,
            Self::QuickSlotConfig(..) => ServerDataType::QuickSlotConfig,
            Self::SkillBarConfig(..) => ServerDataType::SkillBarConfig,
            Self::TechniquesSnapshot(..) => ServerDataType::TechniquesSnapshot,
            Self::UnlocksSync(..) => ServerDataType::UnlocksSync,
            Self::DerivedAttrsSync(..) => ServerDataType::DerivedAttrsSync,
            Self::EventStreamPush(..) => ServerDataType::EventStreamPush,
            Self::WeaponEquipped(..) => ServerDataType::WeaponEquipped,
            Self::WeaponBroken(..) => ServerDataType::WeaponBroken,
            Self::TreasureEquipped(..) => ServerDataType::TreasureEquipped,
            Self::VortexState(..) => ServerDataType::VortexState,
            Self::DuguPoisonState(..) => ServerDataType::DuguPoisonState,
            Self::CarrierState(..) => ServerDataType::CarrierState,
            Self::FalseSkinState(..) => ServerDataType::FalseSkinState,
            Self::LingtianSession(..) => ServerDataType::LingtianSession,
            Self::DeathScreen { .. } => ServerDataType::DeathScreen,
            Self::TerminateScreen { .. } => ServerDataType::TerminateScreen,
            Self::RiftPortalState(..) => ServerDataType::RiftPortalState,
            Self::RiftPortalRemoved(..) => ServerDataType::RiftPortalRemoved,
            Self::ExtractStarted(..) => ServerDataType::ExtractStarted,
            Self::ExtractProgress(..) => ServerDataType::ExtractProgress,
            Self::ExtractCompleted(..) => ServerDataType::ExtractCompleted,
            Self::ExtractAborted(..) => ServerDataType::ExtractAborted,
            Self::ExtractFailed(..) => ServerDataType::ExtractFailed,
            Self::TsyCollapseStartedIpc(..) => ServerDataType::TsyCollapseStartedIpc,
            Self::ContainerState(..) => ServerDataType::ContainerState,
            Self::SearchStarted(..) => ServerDataType::SearchStarted,
            Self::SearchProgress(..) => ServerDataType::SearchProgress,
            Self::SearchCompleted(..) => ServerDataType::SearchCompleted,
            Self::SearchAborted(..) => ServerDataType::SearchAborted,
            Self::SkillXpGain(..) => ServerDataType::SkillXpGain,
            Self::SkillLvUp(..) => ServerDataType::SkillLvUp,
            Self::SkillCapChanged(..) => ServerDataType::SkillCapChanged,
            Self::SkillScrollUsed(..) => ServerDataType::SkillScrollUsed,
            Self::SkillSnapshot(..) => ServerDataType::SkillSnapshot,
            Self::ForgeStation(..) => ServerDataType::ForgeStation,
            Self::ForgeSession(..) => ServerDataType::ForgeSession,
            Self::ForgeOutcome(..) => ServerDataType::ForgeOutcome,
            Self::ForgeBlueprintBook(..) => ServerDataType::ForgeBlueprintBook,
            Self::TribulationState(..) => ServerDataType::TribulationState,
            Self::TribulationBroadcast(..) => ServerDataType::TribulationBroadcast,
            Self::AscensionQuota(..) => ServerDataType::AscensionQuota,
            Self::HeartDemonOffer(..) => ServerDataType::HeartDemonOffer,
            Self::BurstMeridianEvent(..) => ServerDataType::BurstMeridianEvent,
            Self::SocialAnonymity(..) => ServerDataType::SocialAnonymity,
            Self::SocialExposure(..) => ServerDataType::SocialExposure,
            Self::SocialPact(..) => ServerDataType::SocialPact,
            Self::SocialFeud(..) => ServerDataType::SocialFeud,
            Self::SocialRenownDelta(..) => ServerDataType::SocialRenownDelta,
            Self::NicheIntrusion(..) => ServerDataType::NicheIntrusion,
            Self::NicheGuardianFatigue(..) => ServerDataType::NicheGuardianFatigue,
            Self::NicheGuardianBroken(..) => ServerDataType::NicheGuardianBroken,
            Self::SparringInvite(..) => ServerDataType::SparringInvite,
            Self::TradeOffer(..) => ServerDataType::TradeOffer,
            Self::RealmVisionParams(..) => ServerDataType::RealmVisionParams,
            Self::SpiritualSenseTargets(..) => ServerDataType::SpiritualSenseTargets,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::agent_bridge::payload_type_label;

    /// Catches wire-vs-label drift like the QuickSlotConfig "snake_case" bug
    /// (would have routed `quick_slot_config` while client expected `quickslot_config`).
    #[test]
    fn hud_payload_wire_type_matches_label() {
        use crate::schema::combat_hud::*;
        let cases: Vec<ServerDataPayloadV1> = vec![
            ServerDataPayloadV1::CombatHudState(CombatHudStateV1 {
                hp_percent: 1.0,
                qi_percent: 1.0,
                stamina_percent: 1.0,
                derived: DerivedAttrFlagsV1::default(),
            }),
            ServerDataPayloadV1::WoundsSnapshot(WoundsSnapshotV1 { wounds: vec![] }),
            ServerDataPayloadV1::DefenseWindow(DefenseWindowV1 {
                duration_ms: 200,
                started_at_ms: 0,
                expires_at_ms: 200,
            }),
            ServerDataPayloadV1::CastSync(CastSyncV1 {
                phase: CastPhaseV1::Idle,
                slot: 0,
                duration_ms: 0,
                started_at_ms: 0,
                outcome: CastOutcomeV1::None,
            }),
            ServerDataPayloadV1::QuickSlotConfig(QuickSlotConfigV1 {
                slots: vec![None; 9],
                cooldown_until_ms: vec![0; 9],
            }),
            ServerDataPayloadV1::SkillBarConfig(SkillBarConfigV1 {
                slots: vec![None; 9],
                cooldown_until_ms: vec![0; 9],
            }),
            ServerDataPayloadV1::TechniquesSnapshot(TechniquesSnapshotV1 { entries: vec![] }),
            ServerDataPayloadV1::UnlocksSync(UnlocksSyncV1::default()),
            ServerDataPayloadV1::DerivedAttrsSync(DerivedAttrsSyncV1 {
                flying: false,
                flying_qi_remaining: 0.0,
                flying_force_descent_at_ms: 0,
                phasing: false,
                phasing_until_ms: 0,
                tribulation_locked: false,
                tribulation_stage: String::new(),
                throughput_peak_norm: 0.0,
                tuike_layers: 0,
                vortex_active: false,
            }),
            ServerDataPayloadV1::EventStreamPush(EventStreamPushV1 {
                channel: EventChannelV1::Combat,
                priority: EventPriorityV1::P1Important,
                source_tag: String::new(),
                text: "x".to_string(),
                color: 0,
                created_at_ms: 0,
            }),
            ServerDataPayloadV1::VortexState(VortexFieldStateV1 {
                caster: "entity:1".to_string(),
                active: true,
                center: [0.0, 64.0, 0.0],
                radius: 1.5,
                delta: 0.25,
                env_qi_at_cast: 0.9,
                maintain_remaining_ticks: 80,
                intercepted_count: 1,
            }),
            ServerDataPayloadV1::FalseSkinState(FalseSkinStateV1 {
                target_id: "offline:Azure".to_string(),
                kind: Some(crate::schema::tuike::FalseSkinKindV1::SpiderSilk),
                layers_remaining: 1,
                contam_capacity_per_layer: 10.0,
                absorbed_contam: 3.0,
                equipped_at_tick: 7,
            }),
            ServerDataPayloadV1::RiftPortalState(RiftPortalStateV1 {
                entity_id: 1,
                kind: RiftPortalKindV1::MainRift,
                direction: RiftPortalDirectionV1::Exit,
                family_id: "tsy_lingxu_01".to_string(),
                world_pos: [0.0, 64.0, 0.0],
                trigger_radius: 2.0,
                current_extract_ticks: 160,
                activation_window_end: None,
            }),
            ServerDataPayloadV1::RiftPortalRemoved(RiftPortalRemovedV1 { entity_id: 1 }),
            ServerDataPayloadV1::ExtractStarted(ExtractStartedV1 {
                player_id: "offline:Kiz".to_string(),
                portal_entity_id: 1,
                portal_kind: RiftPortalKindV1::MainRift,
                required_ticks: 160,
                at_tick: 10,
            }),
            ServerDataPayloadV1::ExtractProgress(ExtractProgressV1 {
                player_id: "offline:Kiz".to_string(),
                portal_entity_id: 1,
                elapsed_ticks: 5,
                required_ticks: 160,
            }),
            ServerDataPayloadV1::ExtractCompleted(ExtractCompletedV1 {
                player_id: "offline:Kiz".to_string(),
                portal_kind: RiftPortalKindV1::MainRift,
                family_id: "tsy_lingxu_01".to_string(),
                exit_world_pos: [0.0, 64.0, 0.0],
                at_tick: 170,
            }),
            ServerDataPayloadV1::ExtractAborted(ExtractAbortedV1 {
                player_id: "offline:Kiz".to_string(),
                reason: ExtractAbortedReasonV1::Damaged,
            }),
            ServerDataPayloadV1::ExtractFailed(ExtractFailedV1 {
                player_id: "offline:Kiz".to_string(),
                reason: ExtractFailedReasonV1::SpiritQiDrained,
            }),
            ServerDataPayloadV1::TsyCollapseStartedIpc(TsyCollapseStartedIpcV1 {
                family_id: "tsy_lingxu_01".to_string(),
                at_tick: 100,
                remaining_ticks: 600,
                collapse_tear_entity_ids: vec![2, 3, 4],
            }),
            ServerDataPayloadV1::ContainerState(ContainerStateV1 {
                entity_id: 42,
                kind: ContainerKindV1::StoragePouch,
                family_id: "tsy_lingxu_01".to_string(),
                world_pos: [8.0, 64.0, -4.0],
                locked: None,
                depleted: false,
                searched_by_player_id: None,
            }),
            ServerDataPayloadV1::SearchStarted(SearchStartedV1 {
                player_id: "offline:Kiz".to_string(),
                container_entity_id: 42,
                required_ticks: 200,
                at_tick: 100,
            }),
            ServerDataPayloadV1::SearchProgress(SearchProgressV1 {
                player_id: "offline:Kiz".to_string(),
                container_entity_id: 42,
                elapsed_ticks: 20,
                required_ticks: 200,
            }),
            ServerDataPayloadV1::SearchCompleted(SearchCompletedV1 {
                player_id: "offline:Kiz".to_string(),
                container_entity_id: 42,
                family_id: "tsy_lingxu_01".to_string(),
                loot_preview: vec![LootPreviewItemV1 {
                    template_id: "bone_coin".to_string(),
                    display_name: "骨币".to_string(),
                    stack_count: 3,
                }],
                at_tick: 300,
            }),
            ServerDataPayloadV1::SearchAborted(SearchAbortedV1 {
                player_id: "offline:Kiz".to_string(),
                container_entity_id: 42,
                reason: SearchAbortReasonV1::Cancelled,
                at_tick: 150,
            }),
            ServerDataPayloadV1::TribulationBroadcast(TribulationBroadcastV1::active(
                "Kiz", "warn", 12.0, -34.0, 60_000,
            )),
            ServerDataPayloadV1::TribulationState(TribulationStateV1 {
                active: true,
                char_id: "offline:Kiz".to_string(),
                actor_name: "Kiz".to_string(),
                kind: "du_xu".to_string(),
                phase: "wave".to_string(),
                world_x: 12.0,
                world_z: -34.0,
                wave_current: 2,
                wave_total: 5,
                started_tick: 120,
                phase_started_tick: 2_400,
                next_wave_tick: 2_700,
                failed: false,
                half_step_on_success: false,
                participants: vec!["offline:Kiz".to_string()],
                result: None,
            }),
            ServerDataPayloadV1::AscensionQuota(AscensionQuotaV1::new(1, 3)),
            ServerDataPayloadV1::HeartDemonOffer(HeartDemonOfferV1 {
                offer_id: "heart_demon:1:100".to_string(),
                trigger_id: "heart_demon:1:100".to_string(),
                trigger_label: "心魔劫临身".to_string(),
                realm_label: "渡虚劫 · 心魔".to_string(),
                composure: 0.5,
                quota_remaining: 1,
                quota_total: 1,
                expires_at_ms: 1_700_000_000_000,
                choices: vec![HeartDemonOfferChoiceV1 {
                    choice_id: "heart_demon_choice_0".to_string(),
                    category: "Composure".to_string(),
                    title: "守本心".to_string(),
                    effect_summary: "回复少量当前真元".to_string(),
                    flavor: "你把呼吸压回丹田。".to_string(),
                    style_hint: "稳妥".to_string(),
                }],
            }),
            ServerDataPayloadV1::BurstMeridianEvent(BurstMeridianEventV1 {
                skill: "beng_quan".to_string(),
                caster: "offline:Kiz".to_string(),
                target: Some("entity:42".to_string()),
                tick: 12,
                overload_ratio: 1.5,
                integrity_snapshot: 0.9,
            }),
            ServerDataPayloadV1::QiColorObserved(QiColorObservedV1 {
                observer: "offline:Kiz".to_string(),
                observed: "offline:Azure".to_string(),
                main: ColorKind::Intricate,
                secondary: Some(ColorKind::Heavy),
                is_chaotic: false,
                is_hunyuan: false,
                realm_diff: 2,
            }),
            ServerDataPayloadV1::BotanyPlantV2RenderProfiles(vec![BotanyPlantV2RenderProfileV1 {
                plant_id: "ying_yuan_gu".to_string(),
                base_mesh_ref: "red_mushroom".to_string(),
                tint_rgb: 0xFFA040,
                tint_rgb_secondary: None,
                model_overlay: super::super::botany::BotanyModelOverlayV1::Emissive,
            }]),
            ServerDataPayloadV1::RealmVisionParams(RealmVisionParamsV1 {
                fog_start: 30.0,
                fog_end: 60.0,
                fog_color_rgb: 0xB8B0A8,
                fog_shape: super::super::realm_vision::FogShapeV1::Cylinder,
                vignette_alpha: 0.55,
                tint_color_argb: 0x0FF0EDE8,
                particle_density: 0.0,
                transition_ticks: 100,
                server_view_distance_chunks: 4,
                post_fx_sharpen: 0.0,
            }),
            ServerDataPayloadV1::SpiritualSenseTargets(SpiritualSenseTargetsV1 {
                generation: 1,
                entries: vec![super::super::realm_vision::SenseEntryV1 {
                    kind: super::super::realm_vision::SenseKindV1::LivingQi,
                    x: 8.0,
                    y: 64.0,
                    z: -4.0,
                    intensity: 0.75,
                }],
            }),
        ];

        for payload in cases {
            let label = payload_type_label(payload.payload_type());
            let envelope = ServerDataV1::new(payload);
            let bytes = serde_json::to_vec(&envelope).expect("serialize");
            let value: serde_json::Value = serde_json::from_slice(&bytes).expect("decode");
            let wire_type = value
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            assert_eq!(
                wire_type, label,
                "wire type {wire_type} does not match payload_type_label {label}"
            );
        }
    }

    #[test]
    fn social_server_data_wire_uses_single_envelope_version() {
        let envelope =
            ServerDataV1::new(ServerDataPayloadV1::SocialExposure(SocialExposureEventV1 {
                v: 1,
                actor: "char:alice".to_string(),
                kind: super::super::social::ExposureKindV1::Chat,
                witnesses: vec!["char:bob".to_string()],
                tick: 42,
                zone: Some("spawn".to_string()),
            }));
        let value = serde_json::to_value(&envelope).expect("serialize social exposure");
        assert_eq!(value["v"], 1);
        assert_eq!(value["type"], "social_exposure");
        assert_eq!(value["kind"], "chat");
        assert!(
            value.get("event_v").is_none(),
            "server_data payload must not duplicate nested event version"
        );
    }

    #[test]
    fn social_server_data_deserializes_without_nested_event_version() {
        let json = include_str!(
            "../../../agent/packages/schema/samples/server-data.social-renown-delta.sample.json"
        );
        let payload: ServerDataV1 = serde_json::from_str(json).expect("social renown sample");

        match payload.payload {
            ServerDataPayloadV1::SocialRenownDelta(event) => {
                assert_eq!(event.v, 1);
                assert_eq!(event.char_id, "char:steve");
                assert_eq!(event.tags_added[0].tag, "kept_pact");
            }
            other => panic!("expected SocialRenownDelta, got {other:?}"),
        }
    }

    #[test]
    fn cultivation_detail_roundtrip_and_size_budget() {
        let payload = ServerDataV1::new(ServerDataPayloadV1::CultivationDetail {
            realm: "Induce".to_string(),
            opened: vec![true; 20],
            flow_rate: vec![1.5; 20],
            flow_capacity: vec![10.25; 20],
            integrity: vec![0.87; 20],
            open_progress: vec![1.0; 20],
            cracks_count: vec![0; 20],
            contamination_total: 0.0,
            lifespan: Some(LifespanPreviewV1 {
                years_lived: 42.0,
                cap_by_realm: 200,
                remaining_years: 158.0,
                death_penalty_years: 10,
                tick_rate_multiplier: 1.0,
                is_wind_candle: false,
            }),
            recent_skill_milestones_summary: "t82000:skill:herbalism:lv3".to_string(),
            skill_milestones: vec![SkillMilestoneSnapshotV1 {
                skill: "herbalism".to_string(),
                new_lv: 3,
                achieved_at: 82_000,
                narration: "你摘得百草渐熟，今已识八分。".to_string(),
                total_xp_at: 550,
            }],
            qi_color_main: ColorKind::Intricate,
            qi_color_secondary: Some(ColorKind::Heavy),
            qi_color_chaotic: false,
            qi_color_hunyuan: false,
            practice_weights: vec![PracticeWeightV1 {
                color: ColorKind::Intricate,
                weight: 42.0,
                ratio: 0.7,
            }],
        });
        let bytes = payload
            .to_json_bytes_checked()
            .expect("cultivation_detail must fit MAX_PAYLOAD_BYTES");
        assert!(
            bytes.len() <= super::super::common::MAX_PAYLOAD_BYTES,
            "over budget: {} bytes",
            bytes.len()
        );
        let back: ServerDataV1 = serde_json::from_slice(&bytes).expect("roundtrip");
        match back.payload {
            ServerDataPayloadV1::CultivationDetail {
                opened,
                flow_rate,
                lifespan,
                recent_skill_milestones_summary,
                skill_milestones,
                qi_color_main,
                qi_color_secondary,
                practice_weights,
                ..
            } => {
                assert_eq!(opened.len(), 20);
                assert_eq!(flow_rate.len(), 20);
                assert_eq!(flow_rate[0], 1.5);
                assert_eq!(lifespan.unwrap().death_penalty_years, 10);
                assert_eq!(
                    recent_skill_milestones_summary,
                    "t82000:skill:herbalism:lv3"
                );
                assert_eq!(skill_milestones.len(), 1);
                assert_eq!(skill_milestones[0].skill, "herbalism");
                assert_eq!(qi_color_main, ColorKind::Intricate);
                assert_eq!(qi_color_secondary, Some(ColorKind::Heavy));
                assert_eq!(practice_weights[0].color, ColorKind::Intricate);
                assert_eq!(practice_weights[0].weight, 42.0);
            }
            other => panic!("expected CultivationDetail, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_server_data_samples() {
        let samples = [
            include_str!("../../../agent/packages/schema/samples/server-data.welcome.sample.json"),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.heartbeat.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.narration.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.zone-info.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.event-alert.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.player-state.sample.json"
            ),
            include_str!("../../../agent/packages/schema/samples/server-data.ui-open.sample.json"),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.inventory-snapshot.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.inventory-event.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.dropped-loot-sync.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.botany-harvest-progress.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.botany-skill.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.alchemy-furnace.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.alchemy-session.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.alchemy-outcome-forecast.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.alchemy-outcome-resolved.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.alchemy-recipe-book.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.alchemy-contamination.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.death-screen.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.skill-xp-gain.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.skill-lv-up.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.skill-cap-changed.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.skill-scroll-used.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.skill-snapshot.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.skillbar-config.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.techniques-snapshot.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.rift-portal-state.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.rift-portal-removed.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.extract-started.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.extract-progress.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.extract-completed.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.extract-aborted.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.extract-failed.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.tsy-collapse-started-ipc.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.forge-station.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.forge-session.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.forge-outcome-perfect.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.forge-outcome-flawed.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.forge-blueprint-book.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.tribulation-broadcast.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.tribulation-state.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.ascension-quota.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.heart-demon-offer.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.burst-meridian-event.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.social-anonymity.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.social-exposure.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.social-pact.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.social-feud.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.social-renown-delta.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.sparring-invite.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.trade-offer.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.realm-vision-params.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.spiritual-sense-targets.sample.json"
            ),
        ];

        for json in samples {
            let payload: ServerDataV1 =
                serde_json::from_str(json).expect("sample should deserialize into ServerDataV1");

            let reserialized = serde_json::to_string(&payload)
                .expect("deserialized ServerDataV1 should serialize back to JSON");
            let roundtrip: ServerDataV1 = serde_json::from_str(&reserialized)
                .expect("serialized ServerDataV1 should deserialize again");

            let payload_value =
                serde_json::to_value(&payload).expect("payload should convert to JSON value");
            let roundtrip_value =
                serde_json::to_value(&roundtrip).expect("roundtrip should convert to JSON value");

            assert_eq!(
                payload_value, roundtrip_value,
                "roundtrip must preserve typed payload content"
            );
        }
    }

    #[test]
    fn deserialize_zone_info_defaults_missing_status() {
        let value = serde_json::json!({
            "v": SERVER_DATA_VERSION,
            "type": "zone_info",
            "zone": "blood_valley",
            "spirit_qi": -0.42,
            "danger_level": 3,
            "active_events": ["beast_tide"]
        });

        let payload: ServerDataV1 = serde_json::from_value(value).expect("deserialize zone_info");
        match payload.payload {
            ServerDataPayloadV1::ZoneInfo { status, .. } => {
                assert_eq!(status, ZoneStatusV1::Normal);
            }
            other => panic!("expected ZoneInfo, got {other:?}"),
        }
    }

    #[test]
    fn serialize_zone_info_includes_status() {
        let payload = ServerDataV1::new(ServerDataPayloadV1::ZoneInfo {
            zone: "blood_valley".to_string(),
            spirit_qi: -0.42,
            danger_level: 3,
            status: ZoneStatusV1::Collapsed,
            active_events: Some(vec!["realm_collapse".to_string()]),
            perception_text: Some("灵气几近断绝，此地有不祥预感".to_string()),
        });

        let value: serde_json::Value = serde_json::from_slice(
            &payload
                .to_json_bytes_checked()
                .expect("zone_info should serialize"),
        )
        .expect("zone_info JSON should decode");

        assert_eq!(value["status"], "collapsed");
        assert_eq!(value["perception_text"], "灵气几近断绝，此地有不祥预感");
    }

    #[test]
    fn rejects_unknown_server_data_version() {
        let json = r#"{"v":99,"type":"welcome","message":"hello"}"#;
        let error = serde_json::from_str::<ServerDataV1>(json)
            .expect_err("unknown server_data version should be rejected");

        assert!(
            error.to_string().contains("ServerDataV1.v must be"),
            "unexpected server_data version error: {error}"
        );
    }
}
