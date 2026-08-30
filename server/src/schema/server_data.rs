use serde::{de::Error as _, ser::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use std::time::{SystemTime, UNIX_EPOCH};

use super::agent_ui::{AgentUiClosePayloadV1, AgentUiRequestPayloadV1};
use super::alchemy::{
    AlchemyContaminationDataV1, AlchemyFurnaceDataV1, AlchemyOutcomeForecastDataV1,
    AlchemyOutcomeResolvedDataV1, AlchemyRecipeBookDataV1, AlchemySessionDataV1,
};
use super::botany::BotanyPlantV2RenderProfileV1;
use super::combat_carrier::CarrierStateV1;
use super::combat_hud::{
    CastSyncV1, CombatHudStateV1, DefenseWindowV1, DerivedAttrsSyncV1, EventStreamPushV1,
    QuickSlotConfigV1, ShieldBlockHitV1, ShieldBrokenV1, SkillBarConfigV1, TechniquesSnapshotV1,
    TreasureEquippedV1, UnlocksSyncV1, WeaponBrokenV1, WeaponEquippedV1, WoundsSnapshotV1,
};
use super::common::{EventKind, MAX_PAYLOAD_BYTES};
use super::craft::{CraftOutcomeV1, CraftSessionStateV1, RecipeListV1, RecipeUnlockedV1};
use super::cultivation::{InsightOfferV1, SkillMilestoneSnapshotV1};
use super::death_cinematic::DeathCinematicS2cV1;
use super::dugu::DuguPoisonStateV1;
use super::forge::{
    ForgeBlueprintBookDataV1, ForgeOutcomeDataV1, ForgeSessionDataV1, WeaponForgeStationDataV1,
};
use super::identity::IdentityPanelStateV1;
use super::inventory::{InventoryEventV1, InventoryItemViewV1, InventorySnapshotV1};
use super::lingtian::LingtianSessionDataV1;
use super::movement::MovementStateV1;
use super::narration::Narration;
use super::poison_trait::{PoisonDoseEventV1, PoisonOverdoseEventV1, PoisonTraitStateV1};
use super::processing::FreshnessUpdateV1;
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
use super::spirit_treasure::{SpiritTreasureDialoguePayloadV1, SpiritTreasureStatePayloadV1};
use super::tuike::FalseSkinStateV1;
use super::woliu::VortexFieldStateV1;
use super::world_state::{PlayerPowerBreakdown, SeasonStateV1, ZoneStatusV1};
use super::yidao::{HealerNpcAiStateV1, YidaoHudStateV1};
use crate::cultivation::components::ColorKind;
use crate::skill::config::SkillConfigSnapshot;
pub const SERVER_DATA_VERSION: u8 = 1;
pub const WELCOME_MESSAGE: &str = "Bong server connected";
pub const HEARTBEAT_MESSAGE: &str = "mock agent tick";
pub(crate) const ANQI_HUD_ECHO_COUNT_MAX: u32 = i32::MAX as u32;
pub(crate) const ANQI_HUD_QI_PAYLOAD_MAX: f64 = 3.4028234e38;
pub(crate) const ANQI_HUD_TICK_MAX: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GatheringTargetTypeV1 {
    Herb,
    Ore,
    Wood,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GatheringQualityHintV1 {
    Normal,
    FineLikely,
    PerfectPossible,
    Fine,
    Perfect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifespanPreviewV1 {
    pub years_lived: f64,
    pub cap_by_realm: u32,
    pub remaining_years: f64,
    pub death_penalty_years: u32,
    pub tick_rate_multiplier: f64,
    pub is_wind_candle: bool,
}

/// 棺材档级（schema 端：snake_case，optional + default mundane，兼容旧 payload）
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CoffinGradeV1 {
    #[default]
    Mundane,
    Jade,
    Stone,
    Bronze,
}

impl From<crate::coffin::CoffinGrade> for CoffinGradeV1 {
    fn from(g: crate::coffin::CoffinGrade) -> Self {
        match g {
            crate::coffin::CoffinGrade::Mundane => CoffinGradeV1::Mundane,
            crate::coffin::CoffinGrade::Jade => CoffinGradeV1::Jade,
            crate::coffin::CoffinGrade::Stone => CoffinGradeV1::Stone,
            crate::coffin::CoffinGrade::Bronze => CoffinGradeV1::Bronze,
        }
    }
}

/// ⚠️ serde 兼容注意：
/// `deny_unknown_fields` 在此 struct 单独反序列化时生效（拒绝多余字段）。
/// 但 `ServerDataPayloadWireV1::CoffinState` 变体使用 `#[serde(flatten)]`（server_data.rs:914）
/// 把此 struct 的字段展平到外层 enum 中——此时内层 `deny_unknown_fields` **对展平后的字段无效**；
/// 未知字段的拒绝由外层 enum 的 `deny_unknown_fields`（server_data.rs:867）统一兜底。
/// 此处的 `deny_unknown_fields` 保留是为了在 standalone 使用场景（如测试、部分解析）
/// 仍能拒绝无效字段，不是多余的。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CoffinStateV1 {
    pub in_coffin: bool,
    pub lifespan_rate_multiplier: f64,
    /// 棺材档级（optional，缺省 = mundane；新增字段，旧 client 不需要）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coffin_grade: Option<CoffinGradeV1>,
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
    CoffinState,
    UiOpen,
    CultivationDetail,
    QiColorObserved,
    InventorySnapshot,
    InventoryEvent,
    DroppedLootSync,
    RemainsSync,
    BodyPlanLayout,
    RaceGateMeta,
    MorphState,
    BotanyHarvestProgress,
    BotanyPlantV2RenderProfiles,
    MiningProgress,
    LumberProgress,
    GatheringSession,
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
    SkillConfigSnapshot,
    UnlocksSync,
    DerivedAttrsSync,
    EventStreamPush,
    WeaponEquipped,
    WeaponBroken,
    ShieldBroken,
    /// plan-shield-block-v1 P4: 盾格挡命中通知，携带 template_id 触发材质差异化视听。
    ShieldBlockHit,
    TreasureEquipped,
    VortexState,
    DuguPoisonState,
    PoisonDoseEvent,
    PoisonOverdoseEvent,
    PoisonTraitState,
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
    BreakthroughCinematic,
    FullPowerChargingState,
    FullPowerRelease,
    FullPowerExhaustedState,
    SocialAnonymity,
    SocialExposure,
    SocialPact,
    SocialFeud,
    SocialRenownDelta,
    IdentityPanelState,
    NicheIntrusion,
    NicheGuardianFatigue,
    NicheGuardianBroken,
    SparringInvite,
    TradeOffer,
    RealmVisionParams,
    SpiritualSenseTargets,
    HealerNpcAiState,
    YidaoHudState,
    MovementState,
    SpiritTreasureState,
    SpiritTreasureDialogue,
    // ─── plan-craft-v1 P2/P3：通用手搓 IPC ────────────────────────
    CraftRecipeList,
    CraftSessionState,
    CraftOutcome,
    RecipeUnlocked,
    WorkbenchOpen,
    CombatEventFloater,
    KnockbackSync,
    TechniqueProficiencyUpdate,
    PillBuffStatus,
    // ─── plan-supply-coffin-loot-ui P1：外部容器 IPC ────────────────
    LootContainerOpen,
    LootContainerUpdate,
    LootContainerClose,
    // ─── plan-offscreen-war-v1 P9：历史战事状态 payload（保留兼容） ───────
    FactionWarState,
    // ─── plan-combat-skill-feedback-bridges-v1 P4：暗器 HUD ────────
    AnqiHud,
    // ─── plan-combat-skill-feedback-bridges-v1 P5：毒蛊 v2 HUD S2C ─
    DuguV2SkillCast,
    DuguV2SelfCure,
    DuguV2ShroudActive,
    PermanentQiMaxDecayApplied,
    // ─── plan-combat-skill-feedback-bridges-v1 P6：剑道人剑共生 HUD ─
    SwordBondHudState,
    // ─── 震脉 v2 HUD S2C（mirror dugu_v2；点亮 client ZhenmaiHudServerDataHandler） ─
    ZhenmaiHud,
    // ─── plan-exploration-probe-return-v1 P0：神识感知矿脉 S2C ───────
    MineralProbeResult,
    // ─── plan-exploration-probe-return-v1 P1：神识感知保鲜 S2C ──────
    FreshnessUpdate,
    // ─── plan-exploration-probe-return-v1 P2：修炼顿悟 S2C ──────────
    InsightOffer,
    // ─── plan-agent-ui-data-v1 P0：天道 UI-as-Data S2C ──────────────
    /// 天道 UI 面板请求（不含 realm_gate / allowed_button_ids 安全字段）。
    AgentUiRequest,
    /// 天道 UI 面板关闭信号（Replaced / 错误 / session_expired）。
    AgentUiClose,
    // ─── plan-halfstep-rechallenge-integration-v1 P0：半步化虚重渡触发 HUD ─
    /// 半步重渡触发通知（targeted→当事玩家）：灵机涌现提示 + 倒计时。
    HalfStepRechallenge,
    // ─── F9 跨层修复：出生引导棺权威坐标广播 ───────────────────────
    /// 出生引导棺权威坐标（join 时广播，取代 client 硬编码判定盒）。
    TutorialCoffinPos,
    // ─── plan-inventory-hint-panel-v1 P0：库存操作拒绝原因结构化 S2C ───
    /// 库存操作拒绝原因（targeted→触发操作的玩家，不广播）。
    InventoryMoveRejected,
    // ─── plan-scroll-reading-v1 P0：可阅读残卷阅读屏（proto tag 138，§9） ───
    /// 打开一本可阅读残卷的阅读屏（targeted→请求阅读的玩家，不广播）。
    ScrollOpen,
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
        /// 真元上限（cultivation.qi_max）。client HUD 用作真元条分母；wire 必填，
        /// 避免高境界（固元 150 ~ 化虚 2625）真元条退回 100 或 current 推导。
        spirit_qi_max: f64,
        karma: f64,
        composite_power: f64,
        breakdown: PlayerPowerBreakdown,
        zone: String,
        local_neg_pressure: Option<f32>,
        season_state: Option<SeasonStateV1>,
        social: Option<PlayerSocialSnapshotV1>,
        /// plan-wire-format-bridge-v1 P3/RC6：`zone` 对应 `Zone::spirit_qi`（`ZoneRegistry`
        /// 查得）。未知 zone 名（stale）时为 `None`。
        zone_spirit_qi: Option<f64>,
    },
    CoffinState(CoffinStateV1),
    UiOpen {
        ui: Option<String>,
        xml: String,
    },
    /// 经脉详细快照。经脉以 SoA(parallel arrays) 布局，长度随实体 `MeridianProfile`
    /// 变化（plan-race-system-v1 P1c——不再假设恰好 20 条 TCM 经脉）；`channel_ids[i]`
    /// 是第 i 条经脉的 snake_case channel id，与 `opened`/`flow_rate`/... 等数组下标
    /// 一一对应。保持 ≤ MAX_PAYLOAD_BYTES 预算。
    CultivationDetail {
        /// 境界字面量（Awaken/Induce/Condense/Solidify/Spirit/Void，与 `Realm` 判别式对齐）。
        realm: String,
        /// 每条经脉的 channel id（snake_case），与其余并行数组同序、同长。
        channel_ids: Vec<String>,
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
        /// 当前冲脉目标的 channel id（snake_case，与 `channel_ids` 同形态）。
        /// None 表示未设定目标。
        target_meridian: Option<String>,
        /// plan-race-system-v1 P2a — 实体本体（`BodyPlanPurpose::Intrinsic`）的
        /// `body_plan_id`，供 client 按 id 寻址 `BodyPlanLayout` 缓存。
        body_plan_id: String,
        /// plan-race-system-v1 P3b（决议 §8.1 身份快照 bullet）—— 身份快照五字段：
        /// client gate 判定（装备置灰等）的权威真源，不靠猜 / 不靠 `BodyPlanLayoutV1`
        /// 的 `is_humanoid` 元数据（那只供渲染）。未易形（P4 `MorphState` 落地前恒定，
        /// 见 `body_plan::resolve` 模块文档）时 `form_*` 三字段 = 对应本体字段。
        /// 本体种族 id。
        race_id: String,
        /// 当前形态种族 id（未易形时 = `race_id`）。
        form_race_id: String,
        /// 当前形态 body plan id（未易形时 = `body_plan_id`）。
        form_body_plan_id: String,
        /// 本体是否人形。
        intrinsic_is_humanoid: bool,
        /// 当前形态是否人形（未易形时 = `intrinsic_is_humanoid`）。
        form_is_humanoid: bool,
    },
    QiColorObserved(QiColorObservedV1),
    InventorySnapshot(Box<InventorySnapshotV1>),
    InventoryEvent(Box<InventoryEventV1>),
    DroppedLootSync(Vec<DroppedLootEntryV1>),
    /// plan-remains-suite P0 — 世界内遗骸容器快照（join 时 + 内容变化时广播，照
    /// `DroppedLootSync` 的内容 diff 节流套路，见 `network::remains_sync_emit`）。
    RemainsSync(Vec<RemainsEntryV1>),
    /// plan-race-system-v1 P2a — 动态部位 / 经脉面板布局元数据（见
    /// `BodyPlanLayoutV1` 文档）。
    BodyPlanLayout(BodyPlanLayoutV1),
    /// plan-race-system-v1 P3c — 种族门元数据表（item wearer_race + technique
    /// required_race），join 首帧一次性下发，client 缓存后离线判置灰（见
    /// `RaceGateMetaV1` 文档）。
    RaceGateMeta(RaceGateMetaV1),
    /// plan-race-system-v1 P4 —— 易形状态快照（见 `MorphStateV1` 文档）。
    MorphState(MorphStateV1),
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
        /// plan-wire-format-bridge-v1 P3/RC6：此前 proto 从未有此二字段（RC6 `mining_progress`）。
        mineral_id: String,
        display_name: String,
    },
    LumberProgress {
        session_id: String,
        log_pos: [i32; 3],
        progress: f64,
        interrupted: bool,
        completed: bool,
        detail: String,
    },
    GatheringSession {
        session_id: String,
        progress_ticks: u64,
        total_ticks: u64,
        target_name: String,
        target_type: GatheringTargetTypeV1,
        quality_hint: GatheringQualityHintV1,
        tool_used: Option<String>,
        interrupted: bool,
        completed: bool,
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
    SkillConfigSnapshot(SkillConfigSnapshot),
    UnlocksSync(UnlocksSyncV1),
    DerivedAttrsSync(DerivedAttrsSyncV1),
    EventStreamPush(EventStreamPushV1),
    WeaponEquipped(WeaponEquippedV1),
    WeaponBroken(WeaponBrokenV1),
    /// plan-shield-block-v1 P3: 盾牌耐久归零销毁通知。
    ShieldBroken(ShieldBrokenV1),
    /// plan-shield-block-v1 P4: 盾格挡命中通知，携带 template_id 触发材质差异化粒子+音效。
    ShieldBlockHit(ShieldBlockHitV1),
    TreasureEquipped(TreasureEquippedV1),
    VortexState(VortexFieldStateV1),
    DuguPoisonState(DuguPoisonStateV1),
    PoisonDoseEvent(PoisonDoseEventV1),
    PoisonOverdoseEvent(PoisonOverdoseEventV1),
    PoisonTraitState(PoisonTraitStateV1),
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
        cinematic: Option<DeathCinematicS2cV1>,
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
    BreakthroughCinematic(BreakthroughCinematicS2cV1),
    FullPowerChargingState(FullPowerChargingStateV1),
    FullPowerRelease(FullPowerReleaseV1),
    FullPowerExhaustedState(FullPowerExhaustedStateV1),
    SocialAnonymity(SocialAnonymityPayloadV1),
    SocialExposure(SocialExposureEventV1),
    SocialPact(SocialPactEventV1),
    SocialFeud(SocialFeudEventV1),
    SocialRenownDelta(SocialRenownDeltaV1),
    IdentityPanelState(IdentityPanelStateV1),
    NicheIntrusion(NicheIntrusionEventV1),
    NicheGuardianFatigue(NicheGuardianFatigueV1),
    NicheGuardianBroken(NicheGuardianBrokenV1),
    SparringInvite(SparringInvitePayloadV1),
    TradeOffer(TradeOfferPayloadV1),
    RealmVisionParams(RealmVisionParamsV1),
    SpiritualSenseTargets(SpiritualSenseTargetsV1),
    HealerNpcAiState(HealerNpcAiStateV1),
    YidaoHudState(YidaoHudStateV1),
    MovementState(MovementStateV1),
    SpiritTreasureState(SpiritTreasureStatePayloadV1),
    SpiritTreasureDialogue(SpiritTreasureDialoguePayloadV1),
    // ─── plan-craft-v1 P2/P3：通用手搓 IPC ────────────────────────
    /// inventory 打开时一次性推全配方表（含解锁状态）。
    CraftRecipeList(Box<RecipeListV1>),
    /// 当前 craft session 进度（每秒推 + 状态切换时推），`active=false` 表示无 session。
    CraftSessionState(CraftSessionStateV1),
    /// 出炉结果（成功 / 失败），客户端关闭进度条 + 出炉提示。
    CraftOutcome(CraftOutcomeV1),
    /// 三渠道解锁广播（残卷 / 师承 / 顿悟），客户端弹解锁通知。
    RecipeUnlocked(RecipeUnlockedV1),
    WorkbenchOpen {
        entity_id: u64,
        position: [i32; 3],
    },
    CombatEventFloater(CombatEventFloaterV1),
    KnockbackSync(KnockbackSyncV1),
    TechniqueProficiencyUpdate(TechniqueProficiencyUpdateV1),
    PillBuffStatus(PillBuffStatusV1),
    // ─── plan-supply-coffin-loot-ui P1：外部容器 IPC ────────────────
    LootContainerOpen(LootContainerOpenV1),
    LootContainerUpdate(LootContainerUpdateV1),
    LootContainerClose(LootContainerCloseV1),
    // ─── plan-offscreen-war-v1 P9：历史战事状态 payload（保留兼容） ───────
    /// 战事状态 payload（守恒红线：零真元；reframe b：零具名宗门）。
    FactionWarState(FactionWarStateV1),
    // ─── plan-combat-skill-feedback-bridges-v1 P4：暗器 HUD S2C ───
    /// 暗器分身 HUD 状态推送（守恒红线：只读事件字段，不重算真元）。
    AnqiHud(AnqiHudV1),
    // ─── plan-combat-skill-feedback-bridges-v1 P5：毒蛊 v2 HUD S2C ─
    /// 毒蛊五招招式投放事件（eclipse/penetrate/shroud/self_cure/reverse）→ client HUD 反馈。
    /// 守恒红线：只读事件字段，不重算真元，不扣 qi。
    DuguV2SkillCast(DuguV2HudSkillCastV1),
    /// 自蕴进度与暴露状态推送（守恒红线：只读 SelfCureProgressEvent 字段）。
    DuguV2SelfCure(DuguV2HudSelfCureV1),
    /// 幻影遮蔽激活状态推送（守恒红线：只读 ShroudActivatedEvent 字段）。
    DuguV2ShroudActive(DuguV2HudShroudActiveV1),
    /// 永久真元上限衰减通知（守恒红线：只读 PermanentQiMaxDecayApplied 字段，不走 Redis）。
    PermanentQiMaxDecayApplied(DuguV2HudQiDecayV1),
    // ─── plan-combat-skill-feedback-bridges-v1 P6：剑道人剑共生 HUD S2C ─
    /// 人剑共生 HUD 状态推送（守恒红线：stored_qi 只读展示，不二次扣 qi）。
    SwordBondHudState(SwordBondHudStateV1),
    // ─── 震脉 v2 HUD S2C（mirror dugu_v2 dual-emit；点亮 client ZhenmaiHudServerDataHandler） ─
    /// 震脉五招招式 HUD 反馈（parry/neutralize/multipoint/harden/sever_chain）。
    /// 守恒红线：只读事件字段，不重算真元/经脉，不扣 qi。
    ZhenmaiHud(ZhenmaiHudV1),
    // ─── plan-exploration-probe-return-v1 P0：神识感知矿脉 S2C ──────
    /// 神识感知矿脉回执（只读传输，不涉及 qi_physics 守恒）。
    MineralProbeResult(MineralProbeResultV1),
    // ─── plan-exploration-probe-return-v1 P1：神识感知保鲜 S2C ──────
    /// 神识感知保鲜回执（只读传输，复用 freshness_update 类型串）。
    FreshnessUpdate(FreshnessUpdateV1),
    // ─── plan-exploration-probe-return-v1 P2：修炼顿悟 S2C ──────────
    /// 修炼顿悟邀约（只读传输，不涉及 qi_physics 守恒）。复用 InsightOfferV1。
    InsightOffer(InsightOfferV1),
    // ─── plan-agent-ui-data-v1 P0：天道 UI-as-Data S2C ──────────────
    /// 天道 UI 面板请求（不含 realm_gate / allowed_button_ids 安全字段）。
    AgentUiRequest(AgentUiRequestPayloadV1),
    /// 天道 UI 面板关闭信号（Replaced / 错误 / session_expired）。
    AgentUiClose(AgentUiClosePayloadV1),
    // ─── plan-halfstep-rechallenge-integration-v1 P0：半步化虚重渡触发 HUD ─
    /// 半步重渡触发通知（targeted→当事玩家）。
    HalfStepRechallenge(HalfStepRechallengeV1),
    // ─── F9 跨层修复：出生引导棺权威坐标广播 ───────────────────────
    /// 出生引导棺权威坐标（server/src/world/spawn_tutorial.rs 的 `TutorialCoffin.pos`，
    /// join 时广播给 client，取代硬编码 |x|<=8 / y∈[60,90] / |z|<=8 判定盒）。
    TutorialCoffinPos {
        position: [i32; 3],
    },
    // ─── plan-inventory-hint-panel-v1 P0：库存操作拒绝原因结构化 S2C ───
    /// 库存操作拒绝原因（targeted→触发操作的玩家，不广播）。模式照抄 `MineralProbeResult`。
    InventoryMoveRejected(InventoryMoveRejectedV1),
    // ─── plan-scroll-reading-v1 P0：可阅读残卷阅读屏（proto tag 138，§9） ───
    /// 打开一本可阅读残卷的阅读屏（targeted→请求阅读的玩家，不广播）。
    /// 字段只携带正文，不 hardcode 经脉内容——任意 `readable_scroll_spec` 挂载的物品
    /// 皆可复用同一 client 阅读屏（可复用性验收）。
    ScrollOpen {
        /// 模板 id，如 `scroll_meridian_primer`。
        scroll_id: String,
        title: String,
        /// 正文分页，每元素一页；至少 1 页。
        body_pages: Vec<String>,
    },
}

/// 神识感知矿脉回执 S2C。扁平化 `MineralProbeResult` 枚举。
/// `kind = "found"` 时有 mineral_id / remaining_units / display_name_zh；
/// `kind = "denied"` 时有 denial_reason（snake_case 5 变体）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MineralProbeResultV1 {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mineral_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_units: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name_zh: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub denial_reason: Option<String>,
}

/// plan-inventory-hint-panel-v1 P0 — 库存操作拒绝原因结构化 S2C payload。
///
/// 仅发给触发操作的玩家（不广播）。`reason` 是 [`crate::inventory::InventoryMoveRejectReason::to_wire_tag`]
/// 输出的 snake_case string tag（wire 形状安全：string tag 而非 proto enum，避免枚举前缀
/// noOp，见 plan-wire-format-bridge-v1 教训）；`required_realm` / `slot` / `cap` 仅在对应原因
/// 携带该信息时才有值（其余原因为 `None`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryMoveRejectedV1 {
    pub reason: String,
    /// 境界不足时的 required_realm 英文 tag（如 `"Condense"`），其余原因为 `None`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_realm: Option<String>,
    /// worn_cap 满 / 护甲槽位不符 / 背包 equip_slot 不符时的槽位 key，其余原因为 `None`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
    /// worn_cap 满时的槽位上限，其余原因为 `None`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatEventFloaterV1 {
    pub events: Vec<CombatEventFloaterEntryV1>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatEventFloaterEntryV1 {
    pub kind: String,
    pub amount: f32,
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// 攻守方向：true=接收方打出（己方输出），false=接收方承伤。
    #[serde(default)]
    pub outgoing: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnockbackSyncV1 {
    pub distance_blocks: f64,
    pub velocity_blocks_per_tick: f64,
    pub duration_ticks: u32,
    pub kinetic_energy: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collision_damage: Option<f32>,
    pub chain_depth: u8,
    pub block_broken: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TechniqueProficiencyUpdateV1 {
    pub technique_id: String,
    pub proficiency: f32,
    pub gain: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PillBuffStatusV1 {
    pub buff_id: String,
    pub remaining_ticks: u32,
    pub effect_multiplier: f64,
}

// ─── plan-supply-coffin-loot-ui P1：外部容器 S2C payloads ──────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum LootContainerSourceKindV1 {
    SupplyCoffin { grade: String },
    StorageCrate { is_herb: bool },
    DeadDrop,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LootContainerOpenV1 {
    pub session_id: u64,
    pub source_kind: LootContainerSourceKindV1,
    pub rows: u8,
    pub cols: u8,
    pub placed_items: Vec<super::inventory::PlacedInventoryItemV1>,
    pub timeout_wall_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LootContainerUpdateV1 {
    pub session_id: u64,
    pub placed_items: Vec<super::inventory::PlacedInventoryItemV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum LootContainerCloseReasonV1 {
    Timeout,
    Distance,
    PlayerClosed,
    CoffinDestroyed,
    ContainerDestroyed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LootContainerCloseV1 {
    pub session_id: u64,
    pub reason: LootContainerCloseReasonV1,
}

/// plan-offscreen-war-v1 P9：战事状态 payload（历史 server_data 兼容）。
///
/// 守恒红线：**不含任何真元字段**（零真元）。
/// reframe b：zone 用匿名 `region_descriptor`（无具名宗门）。
/// - `winner_group` / `loser_group`：`None` = 尚无结算结果（Emerging/Skirmish 阶段）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FactionWarStateV1 {
    pub war_id: u64,
    pub zone: String,
    pub region_descriptor: String,
    pub phase: String,
    pub groups: Vec<u16>,
    pub enlist_count: u32,
    pub mercenary_count: u32,
    pub intercept_count: u32,
    pub spectate_count: u32,
    /// 胜方 group_id，无结算结果时为 `None`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub winner_group: Option<u16>,
    /// 败方 group_id，无结算结果时为 `None`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loser_group: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnqiHudKindV1 {
    Echo,
    Aim,
    Charge,
    Abrasion,
    Multishot,
}

impl AnqiHudKindV1 {
    pub const ALL: [Self; 5] = [
        Self::Echo,
        Self::Aim,
        Self::Charge,
        Self::Abrasion,
        Self::Multishot,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Echo => "echo",
            Self::Aim => "aim",
            Self::Charge => "charge",
            Self::Abrasion => "abrasion",
            Self::Multishot => "multishot",
        }
    }

    fn from_wire_str(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}

impl Serialize for AnqiHudKindV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AnqiHudKindV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_wire_str(&value)
            .ok_or_else(|| D::Error::custom(format!("unknown anqi_hud kind `{value}`")))
    }
}

#[derive(Debug, Clone, Copy)]
struct AnqiHudBoundedIntegerVisitor {
    field: &'static str,
    maximum: u64,
}

impl AnqiHudBoundedIntegerVisitor {
    fn validate<E>(self, value: u64) -> Result<u64, E>
    where
        E: serde::de::Error,
    {
        if value <= self.maximum {
            Ok(value)
        } else {
            Err(E::custom(format!(
                "anqi_hud {} must be <= {}, got {value}",
                self.field, self.maximum
            )))
        }
    }
}

impl<'de> serde::de::Visitor<'de> for AnqiHudBoundedIntegerVisitor {
    type Value = u64;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "a non-negative integer no greater than {} for anqi_hud {}",
            self.maximum, self.field
        )
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.validate(value)
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let value = u64::try_from(value).map_err(|_| {
            E::custom(format!(
                "anqi_hud {} must be non-negative, got {value}",
                self.field
            ))
        })?;
        self.validate(value)
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > self.maximum as f64
        {
            return Err(E::custom(format!(
                "anqi_hud {} must be an integral number in 0..={}, got {value}",
                self.field, self.maximum
            )));
        }
        Ok(value as u64)
    }
}

fn deserialize_anqi_hud_bounded_integer<'de, D>(
    deserializer: D,
    field: &'static str,
    maximum: u64,
) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(AnqiHudBoundedIntegerVisitor { field, maximum })
}

fn validate_anqi_hud_echo_count(value: u32) -> Result<(), String> {
    if value <= ANQI_HUD_ECHO_COUNT_MAX {
        Ok(())
    } else {
        Err(format!(
            "anqi_hud echo_count must be <= {ANQI_HUD_ECHO_COUNT_MAX}, got {value}"
        ))
    }
}

fn serialize_anqi_hud_echo_count<S>(value: &u32, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    validate_anqi_hud_echo_count(*value).map_err(S::Error::custom)?;
    serializer.serialize_u32(*value)
}

fn deserialize_anqi_hud_echo_count<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let value = deserialize_anqi_hud_bounded_integer(
        deserializer,
        "echo_count",
        u64::from(ANQI_HUD_ECHO_COUNT_MAX),
    )?;
    u32::try_from(value).map_err(D::Error::custom)
}

fn validate_anqi_hud_unit_interval(value: f64) -> Result<(), String> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "anqi_hud progress must be finite in 0..=1, got {value}"
        ))
    }
}

fn validate_anqi_hud_container(value: &str) -> Result<(), String> {
    let is_known = value.is_empty()
        || [
            crate::qi_physics::AnqiContainerKind::HandSlot,
            crate::qi_physics::AnqiContainerKind::Quiver,
            crate::qi_physics::AnqiContainerKind::PocketPouch,
            crate::qi_physics::AnqiContainerKind::Fenglinghe,
        ]
        .into_iter()
        .any(|container| container.as_wire_str() == value);
    if is_known {
        Ok(())
    } else {
        Err(format!(
            "anqi_hud abrasion_container must be empty or a canonical container wire tag, got `{value}`"
        ))
    }
}

fn validate_anqi_hud_qi_payload(value: f64) -> Result<(), String> {
    if value.is_finite() && (0.0..=ANQI_HUD_QI_PAYLOAD_MAX).contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "anqi_hud abrasion_qi_payload must be finite in 0..={ANQI_HUD_QI_PAYLOAD_MAX}, got {value}"
        ))
    }
}

fn validate_anqi_hud_tick(value: u64) -> Result<(), String> {
    if value <= ANQI_HUD_TICK_MAX {
        Ok(())
    } else {
        Err(format!(
            "anqi_hud tick must be <= {ANQI_HUD_TICK_MAX}, got {value}"
        ))
    }
}

fn serialize_anqi_hud_unit_interval<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    validate_anqi_hud_unit_interval(*value).map_err(S::Error::custom)?;
    serializer.serialize_f64(*value)
}

fn deserialize_anqi_hud_unit_interval<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    validate_anqi_hud_unit_interval(value).map_err(D::Error::custom)?;
    Ok(value)
}

fn serialize_anqi_hud_container<S>(value: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    validate_anqi_hud_container(value).map_err(S::Error::custom)?;
    serializer.serialize_str(value)
}

fn deserialize_anqi_hud_container<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    validate_anqi_hud_container(&value).map_err(D::Error::custom)?;
    Ok(value)
}

fn serialize_anqi_hud_qi_payload<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    validate_anqi_hud_qi_payload(*value).map_err(S::Error::custom)?;
    serializer.serialize_f64(*value)
}

fn deserialize_anqi_hud_qi_payload<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    validate_anqi_hud_qi_payload(value).map_err(D::Error::custom)?;
    Ok(value)
}

fn serialize_anqi_hud_tick<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    validate_anqi_hud_tick(*value).map_err(S::Error::custom)?;
    serializer.serialize_u64(*value)
}

fn deserialize_anqi_hud_tick<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_anqi_hud_bounded_integer(deserializer, "tick", ANQI_HUD_TICK_MAX)
}

/// plan-combat-skill-feedback-bridges-v1 P4：暗器分身 HUD 状态推送（server → client）。
/// 守恒红线：全部字段只读自 ECS Event，不重算真元，不扣 qi。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnqiHudV1 {
    pub kind: AnqiHudKindV1,
    #[serde(
        serialize_with = "serialize_anqi_hud_echo_count",
        deserialize_with = "deserialize_anqi_hud_echo_count"
    )]
    pub echo_count: u32,
    #[serde(
        serialize_with = "serialize_anqi_hud_unit_interval",
        deserialize_with = "deserialize_anqi_hud_unit_interval"
    )]
    pub aim_progress: f64,
    #[serde(
        serialize_with = "serialize_anqi_hud_unit_interval",
        deserialize_with = "deserialize_anqi_hud_unit_interval"
    )]
    pub charge_progress: f64,
    #[serde(
        serialize_with = "serialize_anqi_hud_container",
        deserialize_with = "deserialize_anqi_hud_container"
    )]
    pub abrasion_container: String,
    #[serde(
        serialize_with = "serialize_anqi_hud_qi_payload",
        deserialize_with = "deserialize_anqi_hud_qi_payload"
    )]
    pub abrasion_qi_payload: f64,
    #[serde(
        serialize_with = "serialize_anqi_hud_tick",
        deserialize_with = "deserialize_anqi_hud_tick"
    )]
    pub tick: u64,
}

// ─── plan-combat-skill-feedback-bridges-v1 P5：毒蛊 v2 HUD S2C structs ──────

/// 毒蛊五招招式投放通知。kind 取值："eclipse"|"penetrate"|"shroud"|"self_cure"|"reverse"。
/// 守恒红线：全部字段只读自 ECS Event，不重算真元，不扣 qi。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DuguV2HudSkillCastV1 {
    /// 招式种类（wire 名：eclipse / penetrate / shroud / self_cure / reverse）
    pub kind: String,
    pub caster: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// 即时/临时/永久 毒蛊层级（wire 名：immediate/temporary/permanent；Shroud/SelfCure/Reverse 为 None）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taint_tier: Option<String>,
    pub reveal_probability: f32,
    pub tick: u64,
}

/// 自蕴进度与暴露状态推送（来自 SelfCureProgressEvent）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DuguV2HudSelfCureV1 {
    pub caster: String,
    /// 当前自蕴百分比（0..=100）
    pub gain_percent: f32,
    /// 形貌是否已暴露（once-set-stays-true sticky flag）
    pub self_revealed: bool,
    pub tick: u64,
}

/// 幻影遮蔽激活状态推送（来自 ShroudActivatedEvent）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DuguV2HudShroudActiveV1 {
    pub caster: String,
    /// 遮蔽强度（0..=1，强度越高遮蔽效果越好）
    pub strength: f32,
    /// 遮蔽到期 tick（用于 client 计算 shroudUntilMs）
    pub expires_at_tick: u64,
    pub tick: u64,
}

/// 永久真元上限衰减通知（来自 PermanentQiMaxDecayApplied，仅 S2C，不走 Redis）。
/// 守恒红线：loss/qi_max_after 均只读自 ECS 已扣量，不在此处重算。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DuguV2HudQiDecayV1 {
    pub target: String,
    /// 本 tick 衰减量（只读）
    pub loss: f32,
    /// 衰减后真元上限（只读）
    pub qi_max_after: f32,
    pub tick: u64,
}

/// 震脉五招 HUD S2C（server → client，mirror dugu_v2 dual-emit）。
///
/// 字段严格匹配 client `ZhenmaiHudServerDataHandler` 解析键：
/// - `skill_id`：`parry` | `neutralize` | `multipoint` | `harden` | `sever_chain`（client switch key）
/// - `meridian_id`：经脉判别名（neutralize/harden/sever_chain 携带；parry/multipoint 为空串）
/// - `contam_removed`：neutralize 去除的染毒量
/// - `remaining_points`：multipoint 剩余反震点数
/// - `damage_reduction`：harden 减伤比例 [0,1]
/// - `k_drain`：sever_chain 反噬增幅强度（仅展示）
/// - `duration_ms`：multipoint/harden/sever_chain 持续/窗口时长（ms，0=client 用缺省 DEFAULT_DURATION_MS）
///
/// 守恒红线：全部字段只读自 ECS Event，不重算真元/经脉，不扣 qi。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZhenmaiHudV1 {
    pub skill_id: String,
    /// 经脉判别名（空串表示无经脉维度，client `readString` 回退 ""）
    pub meridian_id: String,
    pub contam_removed: f32,
    pub remaining_points: u32,
    pub damage_reduction: f32,
    pub k_drain: f32,
    /// 持续/窗口时长（ms；0 → client `readDuration` 回退 DEFAULT_DURATION_MS）
    pub duration_ms: u64,
    pub tick: u64,
}

// ─── plan-combat-skill-feedback-bridges-v1 P6：剑道人剑共生 HUD S2C structs ──

/// 人剑共生 HUD 状态推送（server → client）。
///
/// 守恒红线：全部字段只读自 ECS SwordBondComponent，不重算真元，不扣 qi。
/// heavenGateReady = can_store_qi() && stored_qi >= stored_qi_cap()（满储即可开天门）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwordBondHudStateV1 {
    /// 是否处于激活状态（玩家持有 SwordBondComponent 时为 true）。
    pub active: bool,
    /// 剑品阶 tier（0=凡铁..6=化虚），与 SwordGrade::tier() 对应。
    pub grade_index: u32,
    /// 剑品阶汉字名（如 "凝脉"），与 SwordGrade::display_name() 对应。
    pub grade_name: String,
    /// 储真元比例 0..=1（stored_qi / cap，cap=0 时为 0）。
    pub stored_qi_ratio: f32,
    /// 人剑亲和度 0..=1（bond_strength 直接传递）。
    pub bond_strength: f32,
    /// 是否可开天门（stored_qi >= stored_qi_cap() && can_store_qi()）。
    pub heaven_gate_ready: bool,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AscensionQuotaV1 {
    pub occupied_slots: u32,
    pub quota_limit: u32,
    pub available_slots: u32,
    #[serde(default)]
    pub total_world_qi: f64,
    #[serde(default)]
    pub quota_k: f64,
    #[serde(default)]
    pub quota_basis: String,
}

impl AscensionQuotaV1 {
    pub fn new(occupied_slots: u32, quota_limit: u32) -> Self {
        Self::with_world_qi(occupied_slots, quota_limit, 0.0, 0.0, "")
    }

    pub fn with_world_qi(
        occupied_slots: u32,
        quota_limit: u32,
        total_world_qi: f64,
        quota_k: f64,
        quota_basis: impl Into<String>,
    ) -> Self {
        Self {
            occupied_slots,
            quota_limit,
            available_slots: quota_limit.saturating_sub(occupied_slots),
            total_world_qi: if total_world_qi.is_finite() {
                total_world_qi.max(0.0)
            } else {
                0.0
            },
            quota_k: if quota_k.is_finite() {
                quota_k.max(0.0)
            } else {
                0.0
            },
            quota_basis: quota_basis.into(),
        }
    }
}

/// plan-halfstep-rechallenge-integration-v1 P0：半步化虚重渡触发通知（targeted→玩家）。
///
/// `active=true` 表示触发/刷新；`active=false` 为显式 HIDE（成功渡劫 / 化虚 settle 时追发）。
/// 过窗（`currentTick > rechallenge_window_until`）由 client 本地判定自动淡出，不需每 tick 推送。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HalfStepRechallengeV1 {
    /// 是否激活（false = 明确清场，client 立即淡出）。
    pub active: bool,
    /// 触发者 char_id（用于 client 校验是否为本地玩家）。
    pub char_id: String,
    /// 重渡窗口截止 tick（client 用于倒计时 + 过窗自动淡出）。
    pub rechallenge_window_until: u64,
    /// 触发时服务器当前 tick（client 参考值）。
    pub at_tick: u64,
}

impl HalfStepRechallengeV1 {
    pub fn trigger(
        char_id: impl Into<String>,
        rechallenge_window_until: u64,
        at_tick: u64,
    ) -> Self {
        Self {
            active: true,
            char_id: char_id.into(),
            rechallenge_window_until,
            at_tick,
        }
    }

    pub fn hide(char_id: impl Into<String>) -> Self {
        Self {
            active: false,
            char_id: char_id.into(),
            rechallenge_window_until: 0,
            at_tick: 0,
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
pub struct BreakthroughCinematicS2cV1 {
    pub actor_id: String,
    pub phase: String,
    pub phase_tick: u32,
    pub phase_duration_ticks: u32,
    pub realm_from: String,
    pub realm_to: String,
    pub result: String,
    pub interrupted: bool,
    pub world_pos: [f64; 3],
    pub visible_radius_blocks: f64,
    pub global: bool,
    pub distant_billboard: bool,
    pub particle_density: f32,
    pub intensity: f32,
    pub season_overlay: String,
    pub style: String,
    pub at_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FullPowerChargingStateV1 {
    pub caster_uuid: String,
    pub active: bool,
    pub qi_committed: f64,
    pub target_qi: f64,
    pub started_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FullPowerReleaseV1 {
    pub caster_uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_uuid: Option<String>,
    pub qi_released: f64,
    pub tick: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hit_position: Option<[f64; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FullPowerExhaustedStateV1 {
    pub caster_uuid: String,
    pub active: bool,
    pub started_tick: u64,
    pub recovery_at_tick: u64,
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
        /// 真元上限（cultivation.qi_max），client HUD 真元条分母。
        spirit_qi_max: f64,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        zone_spirit_qi: Option<f64>,
    },
    CoffinState {
        #[serde(flatten)]
        state: CoffinStateV1,
    },
    UiOpen {
        #[serde(skip_serializing_if = "Option::is_none")]
        ui: Option<String>,
        xml: String,
    },
    CultivationDetail {
        realm: String,
        #[serde(default)]
        channel_ids: Vec<String>,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_meridian: Option<String>,
        #[serde(default)]
        body_plan_id: String,
        // plan-race-system-v1 P3b — 身份快照五字段（见 `ServerDataPayloadV1::CultivationDetail`
        // 同名字段文档）；`#[serde(default)]` 保证老 sample/客户端零改动继续过验。
        #[serde(default)]
        race_id: String,
        #[serde(default)]
        form_race_id: String,
        #[serde(default)]
        form_body_plan_id: String,
        #[serde(default)]
        intrinsic_is_humanoid: bool,
        #[serde(default)]
        form_is_humanoid: bool,
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
    RemainsSync {
        remains: Vec<RemainsEntryV1>,
    },
    BodyPlanLayout {
        #[serde(flatten)]
        layout: BodyPlanLayoutV1,
    },
    RaceGateMeta {
        #[serde(flatten)]
        meta: RaceGateMetaV1,
    },
    MorphState {
        #[serde(flatten)]
        state: MorphStateV1,
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
        #[serde(default)]
        mineral_id: String,
        #[serde(default)]
        display_name: String,
    },
    LumberProgress {
        session_id: String,
        log_pos: [i32; 3],
        progress: f64,
        interrupted: bool,
        completed: bool,
        detail: String,
    },
    GatheringSession {
        session_id: String,
        progress_ticks: u64,
        total_ticks: u64,
        target_name: String,
        target_type: GatheringTargetTypeV1,
        quality_hint: GatheringQualityHintV1,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_used: Option<String>,
        interrupted: bool,
        completed: bool,
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
    SkillConfigSnapshot {
        #[serde(flatten)]
        snapshot: SkillConfigSnapshot,
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
    ShieldBroken {
        #[serde(flatten)]
        shield_broken: ShieldBrokenV1,
    },
    ShieldBlockHit {
        #[serde(flatten)]
        shield_block_hit: ShieldBlockHitV1,
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
    PoisonDoseEvent {
        #[serde(flatten)]
        event: PoisonDoseEventV1,
    },
    PoisonOverdoseEvent {
        #[serde(flatten)]
        event: PoisonOverdoseEventV1,
    },
    PoisonTraitState {
        #[serde(flatten)]
        state: PoisonTraitStateV1,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cinematic: Option<DeathCinematicS2cV1>,
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
    BreakthroughCinematic {
        #[serde(flatten)]
        event: BreakthroughCinematicS2cV1,
    },
    FullPowerChargingState {
        #[serde(flatten)]
        state: FullPowerChargingStateV1,
    },
    FullPowerRelease {
        #[serde(flatten)]
        event: FullPowerReleaseV1,
    },
    FullPowerExhaustedState {
        #[serde(flatten)]
        state: FullPowerExhaustedStateV1,
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
    IdentityPanelState {
        #[serde(flatten)]
        state: IdentityPanelStateV1,
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
    HealerNpcAiState {
        #[serde(flatten)]
        state: HealerNpcAiStateV1,
    },
    YidaoHudState {
        #[serde(flatten)]
        state: YidaoHudStateV1,
    },
    MovementState {
        #[serde(flatten)]
        state: MovementStateV1,
    },
    SpiritTreasureState {
        #[serde(flatten)]
        state: SpiritTreasureStatePayloadV1,
    },
    SpiritTreasureDialogue {
        #[serde(flatten)]
        dialogue: SpiritTreasureDialoguePayloadV1,
    },
    // ─── plan-craft-v1 P2/P3：通用手搓 IPC ────────────────────────
    CraftRecipeList {
        #[serde(flatten)]
        list: Box<RecipeListV1>,
    },
    CraftSessionState {
        #[serde(flatten)]
        state: CraftSessionStateV1,
    },
    CraftOutcome {
        #[serde(flatten)]
        outcome: CraftOutcomeV1,
    },
    RecipeUnlocked {
        #[serde(flatten)]
        event: RecipeUnlockedV1,
    },
    WorkbenchOpen {
        entity_id: u64,
        position: [i32; 3],
    },
    CombatEvent {
        events: Vec<CombatEventFloaterEntryV1>,
    },
    KnockbackSync {
        #[serde(flatten)]
        sync: KnockbackSyncV1,
    },
    TechniqueProficiencyUpdate {
        #[serde(flatten)]
        update: TechniqueProficiencyUpdateV1,
    },
    PillBuffStatus {
        #[serde(flatten)]
        status: PillBuffStatusV1,
    },
    // ─── plan-supply-coffin-loot-ui P1：外部容器 ────────────────
    LootContainerOpen {
        #[serde(flatten)]
        data: LootContainerOpenV1,
    },
    LootContainerUpdate {
        #[serde(flatten)]
        data: LootContainerUpdateV1,
    },
    LootContainerClose {
        #[serde(flatten)]
        data: LootContainerCloseV1,
    },
    // ─── plan-offscreen-war-v1 P9：历史战事状态 payload（保留兼容） ───────
    FactionWarState {
        #[serde(flatten)]
        data: FactionWarStateV1,
    },
    // ─── plan-combat-skill-feedback-bridges-v1 P4：暗器 HUD ────────
    AnqiHud {
        #[serde(flatten)]
        data: AnqiHudV1,
    },
    // ─── plan-combat-skill-feedback-bridges-v1 P5：毒蛊 v2 HUD S2C ─
    DuguV2SkillCast {
        #[serde(flatten)]
        data: DuguV2HudSkillCastV1,
    },
    DuguV2SelfCure {
        #[serde(flatten)]
        data: DuguV2HudSelfCureV1,
    },
    DuguV2ShroudActive {
        #[serde(flatten)]
        data: DuguV2HudShroudActiveV1,
    },
    PermanentQiMaxDecayApplied {
        #[serde(flatten)]
        data: DuguV2HudQiDecayV1,
    },
    // ─── plan-combat-skill-feedback-bridges-v1 P6：剑道人剑共生 HUD ─
    SwordBondHudState {
        #[serde(flatten)]
        data: SwordBondHudStateV1,
    },
    // ─── 震脉 v2 HUD S2C（mirror dugu_v2） ─
    ZhenmaiHud {
        #[serde(flatten)]
        data: ZhenmaiHudV1,
    },
    // ─── plan-exploration-probe-return-v1 P0：神识感知矿脉 S2C ───────
    MineralProbeResult {
        #[serde(flatten)]
        data: MineralProbeResultV1,
    },
    // ─── plan-exploration-probe-return-v1 P1：神识感知保鲜 S2C ──────
    FreshnessUpdate {
        #[serde(flatten)]
        data: FreshnessUpdateV1,
    },
    // ─── plan-exploration-probe-return-v1 P2：修炼顿悟 S2C ──────────
    InsightOffer {
        #[serde(flatten)]
        data: InsightOfferV1,
    },
    // ─── plan-agent-ui-data-v1 P0：天道 UI-as-Data S2C ──────────────
    /// 天道 UI 面板请求（不含 realm_gate / allowed_button_ids 安全字段）。
    AgentUiRequest {
        #[serde(flatten)]
        data: AgentUiRequestPayloadV1,
    },
    /// 天道 UI 面板关闭信号。
    AgentUiClose {
        #[serde(flatten)]
        data: AgentUiClosePayloadV1,
    },
    // ─── plan-halfstep-rechallenge-integration-v1 P0 ────────────────
    /// 半步重渡触发通知（targeted→玩家）。
    HalfStepRechallenge {
        #[serde(flatten)]
        data: HalfStepRechallengeV1,
    },
    // ─── F9 跨层修复：出生引导棺权威坐标广播 ───────────────────────
    TutorialCoffinPos {
        position: [i32; 3],
    },
    // ─── plan-inventory-hint-panel-v1 P0：库存操作拒绝原因结构化 S2C ───
    InventoryMoveRejected {
        #[serde(flatten)]
        data: InventoryMoveRejectedV1,
    },
    // ─── plan-scroll-reading-v1 P0：可阅读残卷阅读屏（proto tag 138，§9） ───
    ScrollOpen {
        scroll_id: String,
        title: String,
        body_pages: Vec<String>,
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

/// plan-remains-suite P0 — 世界内遗骸容器的轻量摘要快照（照 [`DroppedLootEntryV1`] 的
/// 形状；不像 dropped_loot 那样带完整 `item` 列表，只给"有没有东西/东西有多少"的摘要，
/// 详细内容由拾取动作在 server 权威结算，不需要 client 提前知道）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RemainsEntryV1 {
    /// 遗骸实体的稳定 id（复用 valence `UniqueId`，标准 UUID 字符串形式）。
    pub remains_id: String,
    pub world_pos: [f64; 3],
    /// `DimensionKind::ident_str()`（如 `minecraft:overworld` / `bong:tsy`）。
    pub dimension: String,
    pub display_name: String,
    pub item_count: u64,
    pub bone_coins: u64,
}

/// plan-race-system-v1 P3a — `RaceGate` 的 wire 形状（与 proto `bong.RaceGate` /
/// TS `RaceGateV1` 精确对应）：扁平结构，`kind` 恒为必填字符串标签，`species` 恒为
/// 必填数组（`kind != "species"` 时恒为空，而非省略字段）。
///
/// 与 `body_plan::types::RaceGateOwned`（内部标签枚举，`Any`/`Humanoid` 变体序列化
/// 时**不**携带 `species` 字段）刻意区分为两份形状——`RaceGateOwned` 服务
/// `ItemTemplate` TOML 等 Rust 内部消费场景的人体工学；本类型服务需要与
/// proto flat message 字段级 1:1 对应的 wire 场景（prost message 恒有全部字段，
/// 无法表达"某变体缺某字段"）。两者互转见
/// `proto_convert::{race_gate_owned_to_proto, race_gate_owned_from_proto}`
/// （直接对接 prost `bong::RaceGate`，本类型只用于 JSON sample pin 测试 +
/// 未来挂载 payload 字段时的手写镜像）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RaceGateWireV1 {
    pub kind: String,
    pub species: Vec<String>,
}

/// 未知 `kind` 解码错误——fail-closed，调用方必须拒绝而非兜底 `Any`（决议 §8.1 #5/#6）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaceGateWireUnknownKind(pub String);

impl std::fmt::Display for RaceGateWireUnknownKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unknown RaceGate wire kind {:?} — refusing to decode",
            self.0
        )
    }
}

impl std::error::Error for RaceGateWireUnknownKind {}

impl RaceGateWireV1 {
    pub fn from_owned(gate: &crate::body_plan::RaceGateOwned) -> Self {
        use crate::body_plan::RaceGateOwned;
        match gate {
            RaceGateOwned::Any => RaceGateWireV1 {
                kind: "any".to_string(),
                species: Vec::new(),
            },
            RaceGateOwned::Humanoid => RaceGateWireV1 {
                kind: "humanoid".to_string(),
                species: Vec::new(),
            },
            RaceGateOwned::Species { species } => RaceGateWireV1 {
                kind: "species".to_string(),
                species: species.iter().map(|id| id.as_str().to_string()).collect(),
            },
        }
    }

    pub fn try_into_owned(
        &self,
    ) -> Result<crate::body_plan::RaceGateOwned, RaceGateWireUnknownKind> {
        use crate::body_plan::{RaceGateOwned, RaceId};
        match self.kind.as_str() {
            "any" => Ok(RaceGateOwned::Any),
            "humanoid" => Ok(RaceGateOwned::Humanoid),
            "species" => Ok(RaceGateOwned::Species {
                species: self
                    .species
                    .iter()
                    .map(|s| RaceId::new(s.clone()))
                    .collect(),
            }),
            other => Err(RaceGateWireUnknownKind(other.to_string())),
        }
    }
}

/// plan-race-system-v1 P3c — 种族门元数据表的单条目：`id`（item template_id 或
/// technique skill_id）→ `gate`（该条目的种族门）。恒只装非 `Any` 条目
/// （`Any` 是默认，client 表里查不到即恒放行）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RaceGateMetaEntryV1 {
    pub id: String,
    pub gate: RaceGateWireV1,
}

/// plan-race-system-v1 P3c — 静态种族门元数据表（`ServerDataPayloadV1::RaceGateMeta`）。
///
/// 两张表都只装 **非 `Any`** 条目（`Any` 是默认，client 缺省即 `Any`，省流量）：
/// - `item_wearer_race`：item template_id → `wearer_race`，**装备门**判定域用
///   **当前形态身份**（`form_race_id` / `form_is_humanoid`）。
/// - `technique_required_race`：technique skill_id → `required_race`，**功法门**
///   （习得 / 施放）判定域用**本体身份**（`race_id` / `intrinsic_is_humanoid`）。
///
/// 两域不同轴（决议 §8.1 #5/#6）：装备看形态、功法看本体。join 首帧一次性下发
/// （`network::cultivation_detail_emit::emit_race_gate_meta_payloads`，`LastSentRaceGateMeta`
/// 防重发），内容静态（与玩家身份无关），client 换身份时不需重发——client 用
/// `PlayerRaceIdentityStore` 的最新身份对同一张表重判即可。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RaceGateMetaV1 {
    #[serde(default)]
    pub item_wearer_race: Vec<RaceGateMetaEntryV1>,
    #[serde(default)]
    pub technique_required_race: Vec<RaceGateMetaEntryV1>,
}

/// plan-race-system-v1 P4 —— 单个实体的易形状态快照（proto field 142 `morph_state`）。
///
/// `active = false` 专用于 `mode = "delta"` 广播——实体解除易形时下发一条
/// `active=false` 的 entry，client 收到即从本地易形态缓存里删除该 entity_id（不携带
/// 完整字段语义，`model_kind`/`form_race_id`/`form_body_plan_id` 在 `active=false`
/// 时恒为空/0，仅 `entity_id`/`active` 有意义）。`mode = "full"`（join / 周期 sync）时
/// 只包含当前处于 `MorphState` 的实体，`active` 恒为 `true`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MorphStateEntryV1 {
    /// Valence entity id（client 通过 MC entity id 定位实体，与 `daozhan_disguise` 同惯例）。
    pub entity_id: i32,
    pub model_kind: u32,
    pub form_race_id: String,
    pub form_body_plan_id: String,
    pub active: bool,
}

/// plan-race-system-v1 P4 —— `ServerDataPayloadV1::MorphState` 载荷。
///
/// `mode`："full"（join 首帧全量替换 + 周期 sync）| "delta"（易形解除瞬间半径广播，
/// 只携带发生变化的 entity，`active=false` 表示删除）。本 PR 只保证 payload 能被
/// `proto_min` bot 解码（PR-5b 负责 client 渲染消费）。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MorphStateV1 {
    // 注：本结构体刻意不携带独立 `v` 字段——`v` 由外层 `ServerDataV1.v`（信封版本号）
    // 提供，`#[serde(flatten)]` 进 `ServerDataPayloadWireV1::MorphState` 时若本结构体
    // 也声明 `v` 会与外层字段名撞车（`RaceGateMetaV1` 同一惯例，同理无 `v` 字段）。
    // proto `bong::MorphState.v` 字段是 proto message 自身的 schema 版本号，由
    // `proto_convert::server_data_to_proto_payload` 直接常量填 `1`，不经由本结构体。
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub entries: Vec<MorphStateEntryV1>,
}

/// plan-race-system-v1 P2a — `BodyPlanLayoutV1` 的坐标点，归一化到 `[0,1]`（原点 =
/// 布局画布左上角）。同一类型既用作磁盘 `layouts/*.json` 的数据源，也直接是
/// wire payload 的字段（无独立域模型/wire 模型两份拷贝，仿 `RemainsEntryV1` 先例）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BodyPlanPoint2V1 {
    pub x: f64,
    pub y: f64,
}

/// 单个部位的剪影多边形（顶点归一化坐标，按声明顺序首尾相连）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BodyPlanSilhouettePartV1 {
    pub part_id: String,
    pub polygon: Vec<BodyPlanPoint2V1>,
}

/// 部位锚点（伤口红点位 / 状态图标定位点）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BodyPlanPartAnchorV1 {
    pub part_id: String,
    pub point: BodyPlanPoint2V1,
}

/// 单条经脉的多段折线路径（替代 client `BodyInspectComponent.MERIDIAN_PATHS` 硬编码）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BodyPlanMeridianPathV1 {
    pub channel_id: String,
    pub points: Vec<BodyPlanPoint2V1>,
}

/// server 部位 id → client 展示段 id 映射（替代
/// `network::wounds_snapshot_emit::body_part_wire` 的硬编码 match）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BodyPlanPartDisplayMappingV1 {
    pub server_part_id: String,
    pub display_segment_id: String,
}

/// plan-race-system-v1 P2a — 动态部位 / 经脉面板布局元数据。以 `body_plan_id` 为
/// 主键，随 `cultivation_detail` 首帧下发；实体 body_plan 变化（真实换 race）时重发，
/// 易形不触发（P4 语义）。
///
/// `hud_anchors`（P2 major 修复）—— **可选的第二套锚点组**，专供 mini HUD
/// （`MiniBodyHudPlanner`，30×75 粗网格，宽高比 0.40）使用，与 `anchors`
/// （`BodyInspectComponent`，168×236 精细画布，宽高比 0.71）分离：两个消费者画布
/// 比例不同，均匀缩放同一套 `anchors` 会在 mini HUD 上产生 4-6px 像素漂移，违反 plan
/// 「首版渲染与现状像素级一致」红线。humanoid.json 把 `hud_anchors` 原样抽取自
/// `MiniBodyHudPlanner` 改造前的硬编码表（逐值相等，见 `layout.rs` 底部 pin 测试）；
/// 未来非人 plan 可不配（留空 `Vec::new()`），此时 client 回退到用 `anchors` 缩放推导
/// （`locatePart` 换轨逻辑，非人形没有另一份权威像素表可抽取，缩放推导是唯一选择）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BodyPlanLayoutV1 {
    pub body_plan_id: String,
    pub silhouette: Vec<BodyPlanSilhouettePartV1>,
    pub anchors: Vec<BodyPlanPartAnchorV1>,
    pub meridian_paths: Vec<BodyPlanMeridianPathV1>,
    pub part_display_map: Vec<BodyPlanPartDisplayMappingV1>,
    #[serde(default)]
    pub hud_anchors: Vec<BodyPlanPartAnchorV1>,
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
    PortalOccupied,
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
    SurfaceStash,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual_entity_id: Option<i32>,
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
                spirit_qi_max,
                karma,
                composite_power,
                breakdown,
                zone,
                local_neg_pressure,
                season_state,
                social,
                zone_spirit_qi,
            } => {
                if !spirit_qi_max.is_finite() || spirit_qi_max <= 0.0 {
                    return Err("player_state.spirit_qi_max must be positive".to_string());
                }
                Ok(Self::PlayerState {
                    player,
                    realm,
                    spirit_qi,
                    spirit_qi_max,
                    karma,
                    composite_power,
                    breakdown,
                    zone,
                    local_neg_pressure,
                    season_state,
                    social,
                    zone_spirit_qi,
                })
            }
            ServerDataPayloadWireV1::CoffinState { state } => Ok(Self::CoffinState(state)),
            ServerDataPayloadWireV1::UiOpen { ui, xml } => Ok(Self::UiOpen { ui, xml }),
            ServerDataPayloadWireV1::CultivationDetail {
                realm,
                channel_ids,
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
                target_meridian,
                body_plan_id,
                race_id,
                form_race_id,
                form_body_plan_id,
                intrinsic_is_humanoid,
                form_is_humanoid,
            } => Ok(Self::CultivationDetail {
                realm,
                channel_ids,
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
                target_meridian,
                body_plan_id,
                race_id,
                form_race_id,
                form_body_plan_id,
                intrinsic_is_humanoid,
                form_is_humanoid,
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
            ServerDataPayloadWireV1::RemainsSync { remains } => Ok(Self::RemainsSync(remains)),
            ServerDataPayloadWireV1::BodyPlanLayout { layout } => Ok(Self::BodyPlanLayout(layout)),
            ServerDataPayloadWireV1::RaceGateMeta { meta } => Ok(Self::RaceGateMeta(meta)),
            ServerDataPayloadWireV1::MorphState { state } => Ok(Self::MorphState(state)),
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
                mineral_id,
                display_name,
            } => Ok(Self::MiningProgress {
                session_id,
                ore_pos,
                progress,
                interrupted,
                completed,
                mineral_id,
                display_name,
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
            ServerDataPayloadWireV1::GatheringSession {
                session_id,
                progress_ticks,
                total_ticks,
                target_name,
                target_type,
                quality_hint,
                tool_used,
                interrupted,
                completed,
            } => Ok(Self::GatheringSession {
                session_id,
                progress_ticks,
                total_ticks,
                target_name,
                target_type,
                quality_hint,
                tool_used,
                interrupted,
                completed,
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
            ServerDataPayloadWireV1::SkillConfigSnapshot { snapshot } => {
                Ok(Self::SkillConfigSnapshot(snapshot))
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
            ServerDataPayloadWireV1::ShieldBroken { shield_broken } => {
                Ok(Self::ShieldBroken(shield_broken))
            }
            ServerDataPayloadWireV1::ShieldBlockHit { shield_block_hit } => {
                Ok(Self::ShieldBlockHit(shield_block_hit))
            }
            ServerDataPayloadWireV1::TreasureEquipped { treasure_equipped } => {
                Ok(Self::TreasureEquipped(treasure_equipped))
            }
            ServerDataPayloadWireV1::VortexState { state } => Ok(Self::VortexState(state)),
            ServerDataPayloadWireV1::DuguPoisonState { state } => Ok(Self::DuguPoisonState(state)),
            ServerDataPayloadWireV1::PoisonDoseEvent { event } => Ok(Self::PoisonDoseEvent(event)),
            ServerDataPayloadWireV1::PoisonOverdoseEvent { event } => {
                Ok(Self::PoisonOverdoseEvent(event))
            }
            ServerDataPayloadWireV1::PoisonTraitState { state } => {
                Ok(Self::PoisonTraitState(state))
            }
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
                cinematic,
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
                cinematic,
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
            ServerDataPayloadWireV1::BreakthroughCinematic { event } => {
                validate_breakthrough_cinematic(&event)?;
                Ok(Self::BreakthroughCinematic(event))
            }
            ServerDataPayloadWireV1::FullPowerChargingState { state } => {
                validate_full_power_charging_state(&state)?;
                Ok(Self::FullPowerChargingState(state))
            }
            ServerDataPayloadWireV1::FullPowerRelease { event } => {
                validate_full_power_release(&event)?;
                Ok(Self::FullPowerRelease(event))
            }
            ServerDataPayloadWireV1::FullPowerExhaustedState { state } => {
                validate_full_power_exhausted_state(&state)?;
                Ok(Self::FullPowerExhaustedState(state))
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
            ServerDataPayloadWireV1::IdentityPanelState { state } => {
                Ok(Self::IdentityPanelState(state))
            }
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
            ServerDataPayloadWireV1::HealerNpcAiState { state } => {
                Ok(Self::HealerNpcAiState(state))
            }
            ServerDataPayloadWireV1::YidaoHudState { state } => Ok(Self::YidaoHudState(state)),
            ServerDataPayloadWireV1::MovementState { state } => Ok(Self::MovementState(state)),
            ServerDataPayloadWireV1::SpiritTreasureState { state } => {
                Ok(Self::SpiritTreasureState(state))
            }
            ServerDataPayloadWireV1::SpiritTreasureDialogue { dialogue } => {
                Ok(Self::SpiritTreasureDialogue(dialogue))
            }
            ServerDataPayloadWireV1::CraftRecipeList { list } => Ok(Self::CraftRecipeList(list)),
            ServerDataPayloadWireV1::CraftSessionState { state } => {
                Ok(Self::CraftSessionState(state))
            }
            ServerDataPayloadWireV1::CraftOutcome { outcome } => Ok(Self::CraftOutcome(outcome)),
            ServerDataPayloadWireV1::RecipeUnlocked { event } => Ok(Self::RecipeUnlocked(event)),
            ServerDataPayloadWireV1::WorkbenchOpen {
                entity_id,
                position,
            } => Ok(Self::WorkbenchOpen {
                entity_id,
                position,
            }),
            ServerDataPayloadWireV1::CombatEvent { events } => {
                Ok(Self::CombatEventFloater(CombatEventFloaterV1 { events }))
            }
            ServerDataPayloadWireV1::KnockbackSync { sync } => Ok(Self::KnockbackSync(sync)),
            ServerDataPayloadWireV1::TechniqueProficiencyUpdate { update } => {
                Ok(Self::TechniqueProficiencyUpdate(update))
            }
            ServerDataPayloadWireV1::PillBuffStatus { status } => Ok(Self::PillBuffStatus(status)),
            ServerDataPayloadWireV1::LootContainerOpen { data } => {
                Ok(Self::LootContainerOpen(data))
            }
            ServerDataPayloadWireV1::LootContainerUpdate { data } => {
                Ok(Self::LootContainerUpdate(data))
            }
            ServerDataPayloadWireV1::LootContainerClose { data } => {
                Ok(Self::LootContainerClose(data))
            }
            ServerDataPayloadWireV1::FactionWarState { data } => Ok(Self::FactionWarState(data)),
            ServerDataPayloadWireV1::AnqiHud { data } => Ok(Self::AnqiHud(data)),
            // ─── plan-combat-skill-feedback-bridges-v1 P5 ──────────
            ServerDataPayloadWireV1::DuguV2SkillCast { data } => Ok(Self::DuguV2SkillCast(data)),
            ServerDataPayloadWireV1::DuguV2SelfCure { data } => Ok(Self::DuguV2SelfCure(data)),
            ServerDataPayloadWireV1::DuguV2ShroudActive { data } => {
                Ok(Self::DuguV2ShroudActive(data))
            }
            ServerDataPayloadWireV1::PermanentQiMaxDecayApplied { data } => {
                Ok(Self::PermanentQiMaxDecayApplied(data))
            }
            // ─── plan-combat-skill-feedback-bridges-v1 P6 ──────────
            ServerDataPayloadWireV1::SwordBondHudState { data } => {
                Ok(Self::SwordBondHudState(data))
            }
            // ─── 震脉 v2 HUD S2C ──────────
            ServerDataPayloadWireV1::ZhenmaiHud { data } => Ok(Self::ZhenmaiHud(data)),
            ServerDataPayloadWireV1::MineralProbeResult { data } => {
                Ok(Self::MineralProbeResult(data))
            }
            ServerDataPayloadWireV1::FreshnessUpdate { data } => Ok(Self::FreshnessUpdate(data)),
            ServerDataPayloadWireV1::InsightOffer { data } => Ok(Self::InsightOffer(data)),
            ServerDataPayloadWireV1::AgentUiRequest { data } => Ok(Self::AgentUiRequest(data)),
            ServerDataPayloadWireV1::AgentUiClose { data } => Ok(Self::AgentUiClose(data)),
            // ─── plan-halfstep-rechallenge-integration-v1 P0 ────────────────
            ServerDataPayloadWireV1::HalfStepRechallenge { data } => {
                Ok(Self::HalfStepRechallenge(data))
            }
            // ─── F9 跨层修复：出生引导棺权威坐标广播 ────────────────────
            ServerDataPayloadWireV1::TutorialCoffinPos { position } => {
                Ok(Self::TutorialCoffinPos { position })
            }
            // ─── plan-inventory-hint-panel-v1 P0：库存操作拒绝原因结构化 S2C ───
            ServerDataPayloadWireV1::InventoryMoveRejected { data } => {
                Ok(Self::InventoryMoveRejected(data))
            }
            // ─── plan-scroll-reading-v1 P0：可阅读残卷阅读屏 ───
            ServerDataPayloadWireV1::ScrollOpen {
                scroll_id,
                title,
                body_pages,
            } => Ok(Self::ScrollOpen {
                scroll_id,
                title,
                body_pages,
            }),
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
                spirit_qi_max,
                karma,
                composite_power,
                breakdown,
                zone,
                local_neg_pressure,
                season_state,
                social,
                zone_spirit_qi,
            } => Self::PlayerState {
                player: player.clone(),
                realm: realm.clone(),
                spirit_qi: *spirit_qi,
                spirit_qi_max: *spirit_qi_max,
                karma: *karma,
                composite_power: *composite_power,
                breakdown: breakdown.clone(),
                zone: zone.clone(),
                local_neg_pressure: *local_neg_pressure,
                season_state: *season_state,
                social: social.clone(),
                zone_spirit_qi: *zone_spirit_qi,
            },
            ServerDataPayloadV1::CoffinState(state) => Self::CoffinState { state: *state },
            ServerDataPayloadV1::UiOpen { ui, xml } => Self::UiOpen {
                ui: ui.clone(),
                xml: xml.clone(),
            },
            ServerDataPayloadV1::CultivationDetail {
                realm,
                channel_ids,
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
                target_meridian,
                body_plan_id,
                race_id,
                form_race_id,
                form_body_plan_id,
                intrinsic_is_humanoid,
                form_is_humanoid,
            } => Self::CultivationDetail {
                realm: realm.clone(),
                channel_ids: channel_ids.clone(),
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
                target_meridian: target_meridian.clone(),
                body_plan_id: body_plan_id.clone(),
                race_id: race_id.clone(),
                form_race_id: form_race_id.clone(),
                form_body_plan_id: form_body_plan_id.clone(),
                intrinsic_is_humanoid: *intrinsic_is_humanoid,
                form_is_humanoid: *form_is_humanoid,
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
            ServerDataPayloadV1::RemainsSync(remains) => Self::RemainsSync {
                remains: remains.clone(),
            },
            ServerDataPayloadV1::BodyPlanLayout(layout) => Self::BodyPlanLayout {
                layout: layout.clone(),
            },
            ServerDataPayloadV1::RaceGateMeta(meta) => Self::RaceGateMeta { meta: meta.clone() },
            ServerDataPayloadV1::MorphState(state) => Self::MorphState {
                state: state.clone(),
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
                mineral_id,
                display_name,
            } => Self::MiningProgress {
                session_id: session_id.clone(),
                ore_pos: *ore_pos,
                progress: *progress,
                interrupted: *interrupted,
                completed: *completed,
                mineral_id: mineral_id.clone(),
                display_name: display_name.clone(),
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
            ServerDataPayloadV1::GatheringSession {
                session_id,
                progress_ticks,
                total_ticks,
                target_name,
                target_type,
                quality_hint,
                tool_used,
                interrupted,
                completed,
            } => Self::GatheringSession {
                session_id: session_id.clone(),
                progress_ticks: *progress_ticks,
                total_ticks: *total_ticks,
                target_name: target_name.clone(),
                target_type: *target_type,
                quality_hint: *quality_hint,
                tool_used: tool_used.clone(),
                interrupted: *interrupted,
                completed: *completed,
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
            ServerDataPayloadV1::SkillConfigSnapshot(snapshot) => Self::SkillConfigSnapshot {
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
            ServerDataPayloadV1::ShieldBroken(b) => Self::ShieldBroken {
                shield_broken: b.clone(),
            },
            ServerDataPayloadV1::ShieldBlockHit(h) => Self::ShieldBlockHit {
                shield_block_hit: h.clone(),
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
            ServerDataPayloadV1::PoisonDoseEvent(event) => Self::PoisonDoseEvent {
                event: event.clone(),
            },
            ServerDataPayloadV1::PoisonOverdoseEvent(event) => Self::PoisonOverdoseEvent {
                event: event.clone(),
            },
            ServerDataPayloadV1::PoisonTraitState(state) => Self::PoisonTraitState {
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
                cinematic,
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
                cinematic: cinematic.clone(),
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
            ServerDataPayloadV1::AscensionQuota(data) => {
                Self::AscensionQuota { data: data.clone() }
            }
            ServerDataPayloadV1::HeartDemonOffer(data) => {
                Self::HeartDemonOffer { data: data.clone() }
            }
            ServerDataPayloadV1::BurstMeridianEvent(event) => Self::BurstMeridianEvent {
                event: event.clone(),
            },
            ServerDataPayloadV1::BreakthroughCinematic(event) => Self::BreakthroughCinematic {
                event: event.clone(),
            },
            ServerDataPayloadV1::FullPowerChargingState(state) => Self::FullPowerChargingState {
                state: state.clone(),
            },
            ServerDataPayloadV1::FullPowerRelease(event) => Self::FullPowerRelease {
                event: event.clone(),
            },
            ServerDataPayloadV1::FullPowerExhaustedState(state) => Self::FullPowerExhaustedState {
                state: state.clone(),
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
            ServerDataPayloadV1::IdentityPanelState(state) => Self::IdentityPanelState {
                state: state.clone(),
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
            ServerDataPayloadV1::HealerNpcAiState(state) => Self::HealerNpcAiState {
                state: state.clone(),
            },
            ServerDataPayloadV1::YidaoHudState(state) => Self::YidaoHudState {
                state: state.clone(),
            },
            ServerDataPayloadV1::MovementState(state) => Self::MovementState {
                state: state.clone(),
            },
            ServerDataPayloadV1::SpiritTreasureState(state) => Self::SpiritTreasureState {
                state: state.clone(),
            },
            ServerDataPayloadV1::SpiritTreasureDialogue(dialogue) => Self::SpiritTreasureDialogue {
                dialogue: dialogue.clone(),
            },
            ServerDataPayloadV1::CraftRecipeList(list) => {
                Self::CraftRecipeList { list: list.clone() }
            }
            ServerDataPayloadV1::CraftSessionState(state) => Self::CraftSessionState {
                state: state.clone(),
            },
            ServerDataPayloadV1::CraftOutcome(outcome) => Self::CraftOutcome {
                outcome: outcome.clone(),
            },
            ServerDataPayloadV1::RecipeUnlocked(event) => Self::RecipeUnlocked {
                event: event.clone(),
            },
            ServerDataPayloadV1::WorkbenchOpen {
                entity_id,
                position,
            } => Self::WorkbenchOpen {
                entity_id: *entity_id,
                position: *position,
            },
            ServerDataPayloadV1::CombatEventFloater(floater) => Self::CombatEvent {
                events: floater.events.clone(),
            },
            ServerDataPayloadV1::KnockbackSync(sync) => Self::KnockbackSync { sync: sync.clone() },
            ServerDataPayloadV1::TechniqueProficiencyUpdate(update) => {
                Self::TechniqueProficiencyUpdate {
                    update: update.clone(),
                }
            }
            ServerDataPayloadV1::PillBuffStatus(status) => Self::PillBuffStatus {
                status: status.clone(),
            },
            ServerDataPayloadV1::LootContainerOpen(data) => {
                Self::LootContainerOpen { data: data.clone() }
            }
            ServerDataPayloadV1::LootContainerUpdate(data) => {
                Self::LootContainerUpdate { data: data.clone() }
            }
            ServerDataPayloadV1::LootContainerClose(data) => {
                Self::LootContainerClose { data: data.clone() }
            }
            ServerDataPayloadV1::FactionWarState(data) => {
                Self::FactionWarState { data: data.clone() }
            }
            ServerDataPayloadV1::AnqiHud(data) => Self::AnqiHud { data: data.clone() },
            // ─── plan-combat-skill-feedback-bridges-v1 P5 ──────────
            ServerDataPayloadV1::DuguV2SkillCast(data) => {
                Self::DuguV2SkillCast { data: data.clone() }
            }
            ServerDataPayloadV1::DuguV2SelfCure(data) => {
                Self::DuguV2SelfCure { data: data.clone() }
            }
            ServerDataPayloadV1::DuguV2ShroudActive(data) => {
                Self::DuguV2ShroudActive { data: data.clone() }
            }
            ServerDataPayloadV1::PermanentQiMaxDecayApplied(data) => {
                Self::PermanentQiMaxDecayApplied { data: data.clone() }
            }
            // ─── plan-combat-skill-feedback-bridges-v1 P6 ──────────
            ServerDataPayloadV1::SwordBondHudState(data) => {
                Self::SwordBondHudState { data: data.clone() }
            }
            // ─── 震脉 v2 HUD S2C ──────────
            ServerDataPayloadV1::ZhenmaiHud(data) => Self::ZhenmaiHud { data: data.clone() },
            ServerDataPayloadV1::MineralProbeResult(data) => {
                Self::MineralProbeResult { data: data.clone() }
            }
            ServerDataPayloadV1::FreshnessUpdate(data) => {
                Self::FreshnessUpdate { data: data.clone() }
            }
            ServerDataPayloadV1::InsightOffer(data) => Self::InsightOffer { data: data.clone() },
            ServerDataPayloadV1::AgentUiRequest(data) => {
                Self::AgentUiRequest { data: data.clone() }
            }
            ServerDataPayloadV1::AgentUiClose(data) => Self::AgentUiClose { data: data.clone() },
            // ─── plan-halfstep-rechallenge-integration-v1 P0 ────────────────
            ServerDataPayloadV1::HalfStepRechallenge(data) => {
                Self::HalfStepRechallenge { data: data.clone() }
            }
            // ─── F9 跨层修复：出生引导棺权威坐标广播 ────────────────────
            ServerDataPayloadV1::TutorialCoffinPos { position } => Self::TutorialCoffinPos {
                position: *position,
            },
            // ─── plan-inventory-hint-panel-v1 P0：库存操作拒绝原因结构化 S2C ───
            ServerDataPayloadV1::InventoryMoveRejected(data) => {
                Self::InventoryMoveRejected { data: data.clone() }
            }
            // ─── plan-scroll-reading-v1 P0：可阅读残卷阅读屏 ───
            ServerDataPayloadV1::ScrollOpen {
                scroll_id,
                title,
                body_pages,
            } => Self::ScrollOpen {
                scroll_id: scroll_id.clone(),
                title: title.clone(),
                body_pages: body_pages.clone(),
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

fn validate_breakthrough_cinematic(event: &BreakthroughCinematicS2cV1) -> Result<(), String> {
    if event.actor_id.trim().is_empty() {
        return Err("BreakthroughCinematicS2cV1.actor_id must not be empty".to_string());
    }
    if !matches!(
        event.phase.as_str(),
        "prelude" | "charge" | "catalyze" | "apex" | "aftermath"
    ) {
        return Err("BreakthroughCinematicS2cV1.phase is not recognized".to_string());
    }
    if !matches!(
        event.result.as_str(),
        "pending" | "success" | "failure" | "interrupted"
    ) {
        return Err("BreakthroughCinematicS2cV1.result is not recognized".to_string());
    }
    if event.phase_duration_ticks == 0 {
        return Err("BreakthroughCinematicS2cV1.phase_duration_ticks must be > 0".to_string());
    }
    if event.realm_from.trim().is_empty() || event.realm_to.trim().is_empty() {
        return Err("BreakthroughCinematicS2cV1 realm fields must not be empty".to_string());
    }
    if event.world_pos.iter().any(|value| !value.is_finite()) {
        return Err("BreakthroughCinematicS2cV1.world_pos must be finite".to_string());
    }
    if !event.visible_radius_blocks.is_finite() || event.visible_radius_blocks <= 0.0 {
        return Err(
            "BreakthroughCinematicS2cV1.visible_radius_blocks must be finite and > 0".to_string(),
        );
    }
    if !(0.0..=8.0).contains(&event.particle_density) || !event.particle_density.is_finite() {
        return Err(
            "BreakthroughCinematicS2cV1.particle_density must be finite in 0..=8".to_string(),
        );
    }
    if !(0.0..=1.0).contains(&event.intensity) || !event.intensity.is_finite() {
        return Err("BreakthroughCinematicS2cV1.intensity must be finite in 0..=1".to_string());
    }
    Ok(())
}

fn validate_full_power_charging_state(state: &FullPowerChargingStateV1) -> Result<(), String> {
    if state.caster_uuid.is_empty() {
        return Err("FullPowerChargingStateV1.caster_uuid must not be empty".to_string());
    }
    if !state.qi_committed.is_finite() || state.qi_committed < 0.0 {
        return Err("FullPowerChargingStateV1.qi_committed must be finite and >= 0".to_string());
    }
    if !state.target_qi.is_finite() || state.target_qi < 0.0 {
        return Err("FullPowerChargingStateV1.target_qi must be finite and >= 0".to_string());
    }
    Ok(())
}

fn validate_full_power_release(event: &FullPowerReleaseV1) -> Result<(), String> {
    if event.caster_uuid.is_empty() {
        return Err("FullPowerReleaseV1.caster_uuid must not be empty".to_string());
    }
    if event.target_uuid.as_deref().is_some_and(str::is_empty) {
        return Err("FullPowerReleaseV1.target_uuid must not be empty when present".to_string());
    }
    if !event.qi_released.is_finite() || event.qi_released < 0.0 {
        return Err("FullPowerReleaseV1.qi_released must be finite and >= 0".to_string());
    }
    if event
        .hit_position
        .is_some_and(|pos| pos.iter().any(|v| !v.is_finite()))
    {
        return Err("FullPowerReleaseV1.hit_position must be finite when present".to_string());
    }
    Ok(())
}

fn validate_full_power_exhausted_state(state: &FullPowerExhaustedStateV1) -> Result<(), String> {
    if state.caster_uuid.is_empty() {
        return Err("FullPowerExhaustedStateV1.caster_uuid must not be empty".to_string());
    }
    if state.active && state.recovery_at_tick < state.started_tick {
        return Err(
            "FullPowerExhaustedStateV1.recovery_at_tick must be >= started_tick".to_string(),
        );
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

    /// Encode the payload as a protobuf `ServerDataEnvelope`.
    pub fn to_proto_bytes(&self) -> Vec<u8> {
        use super::proto_convert::server_data_to_proto_payload;
        use prost::Message;

        let envelope = super::proto_gen::bong::ServerDataEnvelope {
            payload: Some(server_data_to_proto_payload(&self.payload)),
        };
        envelope.encode_to_vec()
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
            Self::CoffinState(..) => ServerDataType::CoffinState,
            Self::UiOpen { .. } => ServerDataType::UiOpen,
            Self::CultivationDetail { .. } => ServerDataType::CultivationDetail,
            Self::QiColorObserved(..) => ServerDataType::QiColorObserved,
            Self::InventorySnapshot(..) => ServerDataType::InventorySnapshot,
            Self::InventoryEvent(..) => ServerDataType::InventoryEvent,
            Self::DroppedLootSync(..) => ServerDataType::DroppedLootSync,
            Self::RemainsSync(..) => ServerDataType::RemainsSync,
            Self::BodyPlanLayout(..) => ServerDataType::BodyPlanLayout,
            Self::RaceGateMeta(..) => ServerDataType::RaceGateMeta,
            Self::MorphState(..) => ServerDataType::MorphState,
            Self::BotanyHarvestProgress { .. } => ServerDataType::BotanyHarvestProgress,
            Self::BotanyPlantV2RenderProfiles(..) => ServerDataType::BotanyPlantV2RenderProfiles,
            Self::MiningProgress { .. } => ServerDataType::MiningProgress,
            Self::LumberProgress { .. } => ServerDataType::LumberProgress,
            Self::GatheringSession { .. } => ServerDataType::GatheringSession,
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
            Self::SkillConfigSnapshot(..) => ServerDataType::SkillConfigSnapshot,
            Self::UnlocksSync(..) => ServerDataType::UnlocksSync,
            Self::DerivedAttrsSync(..) => ServerDataType::DerivedAttrsSync,
            Self::EventStreamPush(..) => ServerDataType::EventStreamPush,
            Self::WeaponEquipped(..) => ServerDataType::WeaponEquipped,
            Self::WeaponBroken(..) => ServerDataType::WeaponBroken,
            Self::ShieldBroken(..) => ServerDataType::ShieldBroken,
            Self::ShieldBlockHit(..) => ServerDataType::ShieldBlockHit,
            Self::TreasureEquipped(..) => ServerDataType::TreasureEquipped,
            Self::VortexState(..) => ServerDataType::VortexState,
            Self::DuguPoisonState(..) => ServerDataType::DuguPoisonState,
            Self::PoisonDoseEvent(..) => ServerDataType::PoisonDoseEvent,
            Self::PoisonOverdoseEvent(..) => ServerDataType::PoisonOverdoseEvent,
            Self::PoisonTraitState(..) => ServerDataType::PoisonTraitState,
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
            Self::BreakthroughCinematic(..) => ServerDataType::BreakthroughCinematic,
            Self::FullPowerChargingState(..) => ServerDataType::FullPowerChargingState,
            Self::FullPowerRelease(..) => ServerDataType::FullPowerRelease,
            Self::FullPowerExhaustedState(..) => ServerDataType::FullPowerExhaustedState,
            Self::SocialAnonymity(..) => ServerDataType::SocialAnonymity,
            Self::SocialExposure(..) => ServerDataType::SocialExposure,
            Self::SocialPact(..) => ServerDataType::SocialPact,
            Self::SocialFeud(..) => ServerDataType::SocialFeud,
            Self::SocialRenownDelta(..) => ServerDataType::SocialRenownDelta,
            Self::IdentityPanelState(..) => ServerDataType::IdentityPanelState,
            Self::NicheIntrusion(..) => ServerDataType::NicheIntrusion,
            Self::NicheGuardianFatigue(..) => ServerDataType::NicheGuardianFatigue,
            Self::NicheGuardianBroken(..) => ServerDataType::NicheGuardianBroken,
            Self::SparringInvite(..) => ServerDataType::SparringInvite,
            Self::TradeOffer(..) => ServerDataType::TradeOffer,
            Self::RealmVisionParams(..) => ServerDataType::RealmVisionParams,
            Self::SpiritualSenseTargets(..) => ServerDataType::SpiritualSenseTargets,
            Self::HealerNpcAiState(..) => ServerDataType::HealerNpcAiState,
            Self::YidaoHudState(..) => ServerDataType::YidaoHudState,
            Self::MovementState(..) => ServerDataType::MovementState,
            Self::SpiritTreasureState(..) => ServerDataType::SpiritTreasureState,
            Self::SpiritTreasureDialogue(..) => ServerDataType::SpiritTreasureDialogue,
            Self::CraftRecipeList(..) => ServerDataType::CraftRecipeList,
            Self::CraftSessionState(..) => ServerDataType::CraftSessionState,
            Self::CraftOutcome(..) => ServerDataType::CraftOutcome,
            Self::RecipeUnlocked(..) => ServerDataType::RecipeUnlocked,
            Self::WorkbenchOpen { .. } => ServerDataType::WorkbenchOpen,
            Self::CombatEventFloater(..) => ServerDataType::CombatEventFloater,
            Self::KnockbackSync(..) => ServerDataType::KnockbackSync,
            Self::TechniqueProficiencyUpdate(..) => ServerDataType::TechniqueProficiencyUpdate,
            Self::PillBuffStatus(..) => ServerDataType::PillBuffStatus,
            Self::LootContainerOpen(..) => ServerDataType::LootContainerOpen,
            Self::LootContainerUpdate(..) => ServerDataType::LootContainerUpdate,
            Self::LootContainerClose(..) => ServerDataType::LootContainerClose,
            Self::FactionWarState(..) => ServerDataType::FactionWarState,
            Self::AnqiHud(..) => ServerDataType::AnqiHud,
            // ─── plan-combat-skill-feedback-bridges-v1 P5 ──────────
            Self::DuguV2SkillCast(..) => ServerDataType::DuguV2SkillCast,
            Self::DuguV2SelfCure(..) => ServerDataType::DuguV2SelfCure,
            Self::DuguV2ShroudActive(..) => ServerDataType::DuguV2ShroudActive,
            Self::PermanentQiMaxDecayApplied(..) => ServerDataType::PermanentQiMaxDecayApplied,
            // ─── plan-combat-skill-feedback-bridges-v1 P6 ──────────
            Self::SwordBondHudState(..) => ServerDataType::SwordBondHudState,
            // ─── 震脉 v2 HUD S2C ──────────
            Self::ZhenmaiHud(..) => ServerDataType::ZhenmaiHud,
            // ─── plan-exploration-probe-return-v1 P0 ────────────────
            Self::MineralProbeResult(..) => ServerDataType::MineralProbeResult,
            Self::FreshnessUpdate(..) => ServerDataType::FreshnessUpdate,
            // ─── plan-exploration-probe-return-v1 P2 ────────────────
            Self::InsightOffer(..) => ServerDataType::InsightOffer,
            // ─── plan-agent-ui-data-v1 P0 ───────────────────────────
            Self::AgentUiRequest(..) => ServerDataType::AgentUiRequest,
            Self::AgentUiClose(..) => ServerDataType::AgentUiClose,
            // ─── plan-halfstep-rechallenge-integration-v1 P0 ────────────────
            Self::HalfStepRechallenge(..) => ServerDataType::HalfStepRechallenge,
            // ─── F9 跨层修复：出生引导棺权威坐标广播 ────────────────────
            Self::TutorialCoffinPos { .. } => ServerDataType::TutorialCoffinPos,
            // ─── plan-inventory-hint-panel-v1 P0：库存操作拒绝原因结构化 S2C ───
            Self::InventoryMoveRejected(..) => ServerDataType::InventoryMoveRejected,
            // ─── plan-scroll-reading-v1 P0：可阅读残卷阅读屏 ───
            Self::ScrollOpen { .. } => ServerDataType::ScrollOpen,
        }
    }

    /// Returns `true` for variants that are sent via JSON CustomPayload (not proto encoding).
    ///
    /// **CRITICAL — NO catch-all `_` ARM**: this exhaustive match is the compile-time guard
    /// that forces every new `ServerDataPayloadV1` variant to be explicitly classified.
    /// If you add a variant and do not add it here, rustc E0004 (non-exhaustive) fires.
    ///
    /// Current JSON-bypass variants (3):
    /// - `AgentUiRequest` / `AgentUiClose` — 天道 UI-as-Data, no proto definition.
    /// - `HalfStepRechallenge` — 半步重渡触发通知, sent via bong:server_data JSON channel.
    ///
    /// Every new variant MUST be added here as either `true` (JSON-bypass) or `false` (proto).
    /// Proto-path variants return `false`; guard tests will call `to_proto_bytes()` on them.
    /// Bypass variants return `true`; guard tests verify they panic on the proto path.
    ///
    /// MUTATION GUARDS (see `s2c_all_proto_variants_encode_without_panic`):
    /// - Delete any proto arm in `From<&ServerDataPayloadV1>` → that variant hits `unreachable!()` → test panic → RED.
    /// - Flip a bypass variant here to `false` → guard calls `to_proto_bytes()` → `unreachable!()` → RED.
    /// - Add a new variant without updating this fn → rustc E0004 → compile failure.
    pub const fn is_json_bypass(&self) -> bool {
        match self {
            Self::Welcome { .. } => false,
            Self::Heartbeat { .. } => false,
            Self::Narration { .. } => false,
            Self::ZoneInfo { .. } => false,
            Self::EventAlert { .. } => false,
            Self::PlayerState { .. } => false,
            Self::CoffinState(..) => false,
            Self::UiOpen { .. } => false,
            Self::CultivationDetail { .. } => false,
            Self::QiColorObserved(..) => false,
            Self::InventorySnapshot(..) => false,
            Self::InventoryEvent(..) => false,
            Self::DroppedLootSync(..) => false,
            Self::RemainsSync(..) => false,
            Self::BodyPlanLayout(..) => false,
            Self::RaceGateMeta(..) => false,
            Self::MorphState(..) => false,
            Self::BotanyHarvestProgress { .. } => false,
            Self::BotanyPlantV2RenderProfiles(..) => false,
            Self::MiningProgress { .. } => false,
            Self::LumberProgress { .. } => false,
            Self::GatheringSession { .. } => false,
            Self::BotanySkill { .. } => false,
            Self::AlchemyFurnace(..) => false,
            Self::AlchemySession(..) => false,
            Self::AlchemyOutcomeForecast(..) => false,
            Self::AlchemyOutcomeResolved(..) => false,
            Self::AlchemyRecipeBook(..) => false,
            Self::AlchemyContamination(..) => false,
            Self::CombatHudState(..) => false,
            Self::WoundsSnapshot(..) => false,
            Self::DefenseWindow(..) => false,
            Self::CastSync(..) => false,
            Self::QuickSlotConfig(..) => false,
            Self::SkillBarConfig(..) => false,
            Self::TechniquesSnapshot(..) => false,
            Self::SkillConfigSnapshot(..) => false,
            Self::UnlocksSync(..) => false,
            Self::DerivedAttrsSync(..) => false,
            Self::EventStreamPush(..) => false,
            Self::WeaponEquipped(..) => false,
            Self::WeaponBroken(..) => false,
            Self::ShieldBroken(..) => false,
            Self::ShieldBlockHit(..) => false,
            Self::TreasureEquipped(..) => false,
            Self::VortexState(..) => false,
            Self::DuguPoisonState(..) => false,
            Self::PoisonDoseEvent(..) => false,
            Self::PoisonOverdoseEvent(..) => false,
            Self::PoisonTraitState(..) => false,
            Self::CarrierState(..) => false,
            Self::FalseSkinState(..) => false,
            Self::LingtianSession(..) => false,
            Self::DeathScreen { .. } => false,
            Self::TerminateScreen { .. } => false,
            Self::RiftPortalState(..) => false,
            Self::RiftPortalRemoved(..) => false,
            Self::ExtractStarted(..) => false,
            Self::ExtractProgress(..) => false,
            Self::ExtractCompleted(..) => false,
            Self::ExtractAborted(..) => false,
            Self::ExtractFailed(..) => false,
            Self::TsyCollapseStartedIpc(..) => false,
            Self::ContainerState(..) => false,
            Self::SearchStarted(..) => false,
            Self::SearchProgress(..) => false,
            Self::SearchCompleted(..) => false,
            Self::SearchAborted(..) => false,
            Self::SkillXpGain(..) => false,
            Self::SkillLvUp(..) => false,
            Self::SkillCapChanged(..) => false,
            Self::SkillScrollUsed(..) => false,
            Self::SkillSnapshot(..) => false,
            Self::ForgeStation(..) => false,
            Self::ForgeSession(..) => false,
            Self::ForgeOutcome(..) => false,
            Self::ForgeBlueprintBook(..) => false,
            Self::TribulationState(..) => false,
            Self::TribulationBroadcast(..) => false,
            Self::AscensionQuota(..) => false,
            Self::HeartDemonOffer(..) => false,
            Self::BurstMeridianEvent(..) => false,
            Self::BreakthroughCinematic(..) => false,
            Self::FullPowerChargingState(..) => false,
            Self::FullPowerRelease(..) => false,
            Self::FullPowerExhaustedState(..) => false,
            Self::SocialAnonymity(..) => false,
            Self::SocialExposure(..) => false,
            Self::SocialPact(..) => false,
            Self::SocialFeud(..) => false,
            Self::SocialRenownDelta(..) => false,
            Self::IdentityPanelState(..) => false,
            Self::NicheIntrusion(..) => false,
            Self::NicheGuardianFatigue(..) => false,
            Self::NicheGuardianBroken(..) => false,
            Self::SparringInvite(..) => false,
            Self::TradeOffer(..) => false,
            Self::RealmVisionParams(..) => false,
            Self::SpiritualSenseTargets(..) => false,
            Self::HealerNpcAiState(..) => false,
            Self::YidaoHudState(..) => false,
            Self::MovementState(..) => false,
            Self::SpiritTreasureState(..) => false,
            Self::SpiritTreasureDialogue(..) => false,
            Self::CraftRecipeList(..) => false,
            Self::CraftSessionState(..) => false,
            Self::CraftOutcome(..) => false,
            Self::RecipeUnlocked(..) => false,
            Self::WorkbenchOpen { .. } => false,
            Self::CombatEventFloater(..) => false,
            Self::KnockbackSync(..) => false,
            Self::TechniqueProficiencyUpdate(..) => false,
            Self::PillBuffStatus(..) => false,
            Self::LootContainerOpen(..) => false,
            Self::LootContainerUpdate(..) => false,
            Self::LootContainerClose(..) => false,
            Self::FactionWarState(..) => false,
            Self::AnqiHud(..) => false,
            Self::DuguV2SkillCast(..) => false,
            Self::DuguV2SelfCure(..) => false,
            Self::DuguV2ShroudActive(..) => false,
            Self::PermanentQiMaxDecayApplied(..) => false,
            Self::SwordBondHudState(..) => false,
            Self::ZhenmaiHud(..) => false,
            Self::MineralProbeResult(..) => false,
            Self::FreshnessUpdate(..) => false,
            Self::InsightOffer(..) => false,
            // ─── JSON-bypass variants (no proto definition, sent via JSON CustomPayload) ───
            Self::AgentUiRequest(..) => true,
            Self::AgentUiClose(..) => true,
            Self::HalfStepRechallenge(..) => true,
            // ─── F9 跨层修复：出生引导棺权威坐标广播（proto 路径） ──────
            Self::TutorialCoffinPos { .. } => false,
            // ─── plan-inventory-hint-panel-v1 P0：库存操作拒绝原因结构化 S2C（proto 路径） ───
            Self::InventoryMoveRejected(..) => false,
            // ─── plan-scroll-reading-v1 P0：可阅读残卷阅读屏（proto 路径） ───
            Self::ScrollOpen { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::agent_bridge::payload_type_label;
    use crate::schema::movement::{MovementActionRequestV1, MovementActionV1, MovementZoneKindV1};
    use crate::schema::poison_trait::{PoisonOverdoseSeverityV1, PoisonSideEffectTagV1};

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
                combat_active: false,
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
                ack_request_id: None,
                bind_accepted: None,
            }),
            ServerDataPayloadV1::SkillBarConfig(SkillBarConfigV1 {
                slots: vec![None; 9],
                cooldown_until_ms: vec![0; 9],
            }),
            ServerDataPayloadV1::TechniquesSnapshot(TechniquesSnapshotV1 { entries: vec![] }),
            ServerDataPayloadV1::SkillConfigSnapshot(SkillConfigSnapshot {
                configs: Default::default(),
            }),
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
                active_skill_id: "woliu.hold".to_string(),
                charge_progress: 1.0,
                cooldown_until_ms: 0,
                backfire_level: String::new(),
                turbulence_radius: 1.0,
                turbulence_intensity: 0.5,
                turbulence_until_ms: 0,
            }),
            ServerDataPayloadV1::FalseSkinState(FalseSkinStateV1 {
                target_id: "offline:Azure".to_string(),
                kind: Some(crate::schema::tuike::FalseSkinKindV1::SpiderSilk),
                layers_remaining: 1,
                contam_capacity_per_layer: 10.0,
                absorbed_contam: 3.0,
                equipped_at_tick: 7,
                layers: Vec::new(),
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
                reason: ExtractAbortedReasonV1::PortalOccupied,
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
                visual_entity_id: Some(2048),
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
            ServerDataPayloadV1::BreakthroughCinematic(BreakthroughCinematicS2cV1 {
                actor_id: "offline:Kiz".to_string(),
                phase: "apex".to_string(),
                phase_tick: 0,
                phase_duration_ticks: 80,
                realm_from: "Condense".to_string(),
                realm_to: "Solidify".to_string(),
                result: "success".to_string(),
                interrupted: false,
                world_pos: [12.0, 64.0, -8.0],
                visible_radius_blocks: 1024.0,
                global: false,
                distant_billboard: true,
                particle_density: 2.2,
                intensity: 0.78,
                season_overlay: "adaptive".to_string(),
                style: "golden_core".to_string(),
                at_tick: 2400,
            }),
            ServerDataPayloadV1::FullPowerChargingState(FullPowerChargingStateV1 {
                caster_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                active: true,
                qi_committed: 150.0,
                target_qi: 600.0,
                started_tick: 12,
            }),
            ServerDataPayloadV1::FullPowerRelease(FullPowerReleaseV1 {
                caster_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                target_uuid: Some("00000000-0000-0000-0000-000000000002".to_string()),
                qi_released: 600.0,
                tick: 24,
                hit_position: Some([8.0, 66.0, 8.0]),
            }),
            ServerDataPayloadV1::FullPowerExhaustedState(FullPowerExhaustedStateV1 {
                caster_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                active: true,
                started_tick: 24,
                recovery_at_tick: 1224,
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
            ServerDataPayloadV1::PoisonDoseEvent(PoisonDoseEventV1 {
                v: 1,
                player_entity_id: 7,
                dose_amount: 5.0,
                side_effect_tag: PoisonSideEffectTagV1::QiFocusDrift2h,
                poison_level_after: 17.0,
                digestion_after: 50.0,
                at_tick: 100,
            }),
            ServerDataPayloadV1::PoisonOverdoseEvent(PoisonOverdoseEventV1 {
                v: 1,
                player_entity_id: 7,
                severity: PoisonOverdoseSeverityV1::Moderate,
                overflow: 30.0,
                lifespan_penalty_years: 1.0,
                micro_tear_probability: 0.1,
                at_tick: 120,
            }),
            ServerDataPayloadV1::PoisonTraitState(PoisonTraitStateV1 {
                v: 1,
                player_entity_id: 7,
                poison_toxicity: 17.0,
                digestion_current: 50.0,
                digestion_capacity: 100.0,
                toxicity_tier_unlocked: false,
            }),
            ServerDataPayloadV1::BotanyPlantV2RenderProfiles(vec![BotanyPlantV2RenderProfileV1 {
                plant_id: "ying_yuan_gu".to_string(),
                base_mesh_ref: "red_mushroom".to_string(),
                tint_rgb: 0xFFA040,
                tint_rgb_secondary: None,
                model_overlay: super::super::botany::BotanyModelOverlayV1::Emissive,
            }]),
            ServerDataPayloadV1::GatheringSession {
                session_id: "gathering:herb:offline-kiz".to_string(),
                progress_ticks: 20,
                total_ticks: 40,
                target_name: "凝脉草".to_string(),
                target_type: GatheringTargetTypeV1::Herb,
                quality_hint: GatheringQualityHintV1::FineLikely,
                tool_used: Some("hoe_iron".to_string()),
                interrupted: false,
                completed: false,
            },
            ServerDataPayloadV1::GatheringSession {
                session_id: "mining:10:64:10:FanTie".to_string(),
                progress_ticks: 60,
                total_ticks: 60,
                target_name: "凡铁矿".to_string(),
                target_type: GatheringTargetTypeV1::Ore,
                quality_hint: GatheringQualityHintV1::Perfect,
                tool_used: Some("pickaxe_iron".to_string()),
                interrupted: false,
                completed: true,
            },
            ServerDataPayloadV1::GatheringSession {
                session_id: "lumber:offline-kiz:1".to_string(),
                progress_ticks: 0,
                total_ticks: 50,
                target_name: "灵木".to_string(),
                target_type: GatheringTargetTypeV1::Wood,
                quality_hint: GatheringQualityHintV1::Normal,
                tool_used: None,
                interrupted: true,
                completed: false,
            },
            ServerDataPayloadV1::GatheringSession {
                session_id: "gathering:herb:fine".to_string(),
                progress_ticks: 40,
                total_ticks: 40,
                target_name: "优良凝脉草".to_string(),
                target_type: GatheringTargetTypeV1::Herb,
                quality_hint: GatheringQualityHintV1::Fine,
                tool_used: Some("hoe_copper".to_string()),
                interrupted: false,
                completed: true,
            },
            ServerDataPayloadV1::GatheringSession {
                session_id: "lumber:perfect-possible".to_string(),
                progress_ticks: 45,
                total_ticks: 50,
                target_name: "灵木".to_string(),
                target_type: GatheringTargetTypeV1::Wood,
                quality_hint: GatheringQualityHintV1::PerfectPossible,
                tool_used: Some("axe_copper".to_string()),
                interrupted: false,
                completed: false,
            },
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
            ServerDataPayloadV1::HealerNpcAiState(HealerNpcAiStateV1 {
                healer_id: "npc:doctor".to_string(),
                active_action: "triage".to_string(),
                queue_len: 2,
                reputation: 12,
                retreating: false,
            }),
            ServerDataPayloadV1::YidaoHudState(YidaoHudStateV1 {
                healer_id: "npc:doctor".to_string(),
                reputation: 12,
                peace_mastery: 48.0,
                karma: 3.5,
                active_skill: Some(crate::schema::yidao::YidaoSkillIdV1::MeridianRepair),
                patient_ids: vec!["offline:Kiz".to_string()],
                patient_hp_percent: Some(0.75),
                patient_contam_total: Some(1.25),
                severed_meridian_count: 1,
                contract_count: 2,
                mass_preview_count: 0,
            }),
            ServerDataPayloadV1::MovementState(MovementStateV1 {
                current_speed_multiplier: 0.75,
                stamina_cost_active: true,
                movement_action: MovementActionV1::Dashing,
                zone_kind: MovementZoneKindV1::Normal,
                dash_cooldown_remaining_ticks: 40,
                hitbox_height_blocks: 1.8,
                stamina_current: 85.0,
                stamina_max: 100.0,
                low_stamina: false,
                last_action_tick: Some(120),
                rejected_action: Some(MovementActionRequestV1::Dash),
            }),
            ServerDataPayloadV1::CoffinState(CoffinStateV1 {
                in_coffin: true,
                lifespan_rate_multiplier: 0.9,
                coffin_grade: Some(CoffinGradeV1::Mundane),
            }),
            // ─── plan-craft-v1 P2 wire ↔ label drift guard ──────
            ServerDataPayloadV1::CraftRecipeList(Box::new(RecipeListV1 {
                v: 1,
                player_id: "offline:Kiz".to_string(),
                recipes: vec![],
                ts: 1234567,
            })),
            ServerDataPayloadV1::CraftSessionState(CraftSessionStateV1 {
                v: 1,
                player_id: "offline:Kiz".to_string(),
                active: false,
                recipe_id: None,
                elapsed_ticks: 0,
                total_ticks: 0,
                completed_count: 0,
                total_count: 0,
                ts: 1234567,
            }),
            ServerDataPayloadV1::CraftOutcome(CraftOutcomeV1::Completed {
                v: 1,
                player_id: "offline:Kiz".to_string(),
                recipe_id: "craft.example.eclipse_needle.iron".to_string(),
                output_template: "eclipse_needle_iron".to_string(),
                output_count: 3,
                completed_at_tick: 5000,
                ts: 1234567,
            }),
            ServerDataPayloadV1::RecipeUnlocked(RecipeUnlockedV1 {
                v: 1,
                player_id: "offline:Kiz".to_string(),
                recipe_id: "craft.example.fake_skin.light".to_string(),
                source: crate::schema::craft::UnlockEventSourceV1::Insight {
                    trigger: crate::schema::craft::InsightTriggerV1::NearDeath,
                },
                unlocked_at_tick: 8000,
                ts: 1234567,
            }),
            ServerDataPayloadV1::WorkbenchOpen {
                entity_id: 42,
                position: [1, 64, -2],
            },
            // F9 跨层修复：出生引导棺权威坐标广播 wire tag pin。
            ServerDataPayloadV1::TutorialCoffinPos {
                position: [0, 69, 0],
            },
            ServerDataPayloadV1::CombatEventFloater(CombatEventFloaterV1 {
                events: vec![CombatEventFloaterEntryV1 {
                    kind: "hit".to_string(),
                    amount: 5.0,
                    text: "5".to_string(),
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    outgoing: false,
                }],
            }),
            ServerDataPayloadV1::KnockbackSync(KnockbackSyncV1 {
                distance_blocks: 4.0,
                velocity_blocks_per_tick: 0.8,
                duration_ticks: 5,
                kinetic_energy: 22.4,
                collision_damage: Some(3.0),
                chain_depth: 2,
                block_broken: true,
            }),
            ServerDataPayloadV1::TechniqueProficiencyUpdate(TechniqueProficiencyUpdateV1 {
                technique_id: "sword.cleave".to_string(),
                proficiency: 0.42,
                gain: 0.008,
            }),
            ServerDataPayloadV1::PillBuffStatus(PillBuffStatusV1 {
                buff_id: "huo_xue_dan".to_string(),
                remaining_ticks: 3000,
                effect_multiplier: 1.0,
            }),
            // ─── plan-exploration-probe-return-v1 P0 ────────────────
            ServerDataPayloadV1::MineralProbeResult(MineralProbeResultV1 {
                kind: "found".to_string(),
                mineral_id: Some("chi_tong_ore".to_string()),
                remaining_units: Some(23),
                display_name_zh: Some("赤铜矿脉".to_string()),
                denial_reason: None,
            }),
            // ─── plan-exploration-probe-return-v1 P1: FreshnessUpdate wire/label guard ──
            ServerDataPayloadV1::FreshnessUpdate(FreshnessUpdateV1 {
                item_uuid: "42".to_string(),
                freshness: 0.75,
                profile_name: "test_decay".to_string(),
            }),
            // ─── plan-exploration-probe-return-v1 P2: InsightOffer wire/label guard ─────
            ServerDataPayloadV1::InsightOffer(InsightOfferV1 {
                offer_id: "insight:1:100".to_string(),
                trigger_id: "insight:1:100".to_string(),
                character_id: "offline:Kiz".to_string(),
                choices: vec![crate::schema::cultivation::InsightChoiceV1 {
                    category: "Qi".to_string(),
                    effect_kind: "qi_max".to_string(),
                    magnitude: 0.05,
                    flavor_text: "气海微扩张。".to_string(),
                    narrator_voice: None,
                    alignment: None,
                    cost_kind: None,
                    cost_magnitude: None,
                    cost_flavor: None,
                }],
            }),
            // ─── plan-agent-ui-data-v1 P0: Agent UI wire/label guard ─────────
            ServerDataPayloadV1::AgentUiRequest(AgentUiRequestPayloadV1 {
                request_id: "agent-ui-req".to_string(),
                target_player: "offline:Kiz".to_string(),
                xml: "<owo-ui><components><label>test</label></components></owo-ui>".to_string(),
                timeout_ticks: 600,
            }),
            ServerDataPayloadV1::AgentUiClose(AgentUiClosePayloadV1 {
                request_id: "agent-ui-req".to_string(),
                reason: Some("invalid_button_id".to_string()),
            }),
            // ─── plan-halfstep-rechallenge-integration-v1 P0 wire/label guard ─────
            ServerDataPayloadV1::HalfStepRechallenge(HalfStepRechallengeV1 {
                active: true,
                char_id: "offline:Kiz".to_string(),
                rechallenge_window_until: 50_000,
                at_tick: 1_000,
            }),
            // ─── plan-inventory-hint-panel-v1 P0 wire/label guard ─────
            ServerDataPayloadV1::InventoryMoveRejected(InventoryMoveRejectedV1 {
                reason: "worn_cap_full".to_string(),
                required_realm: None,
                slot: Some("chest".to_string()),
                cap: Some(3),
            }),
            // ─── plan-scroll-reading-v1 P0 wire/label guard ─────
            ServerDataPayloadV1::ScrollOpen {
                scroll_id: "scroll_meridian_primer".to_string(),
                title: "《经脉浅述·残卷》".to_string(),
                body_pages: vec!["第一页".to_string(), "第二页".to_string()],
            },
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

    // ─── plan-scroll-reading-v1 P0：ScrollOpen serde pin + TS↔Rust sample 对拍 ───

    /// TS 端 sample（TypeBox source of truth）必须反序列化为 ScrollOpen 且字段全等。
    #[test]
    fn scroll_open_ts_sample_deserializes_in_rust() {
        let json = include_str!(
            "../../../agent/packages/schema/samples/server-data.scroll-open.sample.json"
        );
        let envelope: ServerDataV1 = serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("scroll-open sample should deserialize: {e}"));
        match envelope.payload {
            ServerDataPayloadV1::ScrollOpen {
                scroll_id,
                title,
                body_pages,
            } => {
                assert_eq!(scroll_id, "scroll_meridian_primer");
                assert_eq!(title, "《经脉浅述·残卷》");
                assert_eq!(
                    body_pages.len(),
                    3,
                    "sample 应有 3 页正文，得到 {}",
                    body_pages.len()
                );
            }
            other => panic!("expected ScrollOpen, got {other:?}"),
        }
    }

    #[test]
    fn scroll_open_roundtrip() {
        let payload = ServerDataPayloadV1::ScrollOpen {
            scroll_id: "scroll_meridian_primer".to_string(),
            title: "《经脉浅述·残卷》".to_string(),
            body_pages: vec!["第一页".to_string(), "第二页".to_string()],
        };
        let envelope = ServerDataV1::new(payload);
        let bytes = serde_json::to_vec(&envelope).expect("ScrollOpen serializes");
        let decoded: ServerDataV1 =
            serde_json::from_slice(&bytes).expect("ScrollOpen round-trip deserializes");
        match decoded.payload {
            ServerDataPayloadV1::ScrollOpen {
                scroll_id,
                title,
                body_pages,
            } => {
                assert_eq!(scroll_id, "scroll_meridian_primer");
                assert_eq!(title, "《经脉浅述·残卷》");
                assert_eq!(body_pages, vec!["第一页".to_string(), "第二页".to_string()]);
            }
            other => panic!("expected ScrollOpen after round-trip, got {other:?}"),
        }
    }

    /// 边界：body_pages 为空数组——wire 层不校验（校验在 TOML 解析层 `parse_readable_scroll_spec`），
    /// 但 serde 本身必须允许空数组反序列化（不是 wire 契约拒绝的形状）。
    #[test]
    fn scroll_open_wire_accepts_empty_body_pages() {
        let json = r#"{"v":1,"type":"scroll_open","scroll_id":"x","title":"t","body_pages":[]}"#;
        let envelope: ServerDataV1 =
            serde_json::from_str(json).expect("empty body_pages array should deserialize");
        match envelope.payload {
            ServerDataPayloadV1::ScrollOpen { body_pages, .. } => {
                assert!(body_pages.is_empty());
            }
            other => panic!("expected ScrollOpen, got {other:?}"),
        }
    }

    /// 缺失 title 字段应反序列化失败。
    #[test]
    fn scroll_open_rejects_missing_title() {
        let json = r#"{"v":1,"type":"scroll_open","scroll_id":"x","body_pages":["p1"]}"#;
        let result: Result<ServerDataV1, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "scroll_open without title should fail deserialization"
        );
    }

    /// 额外字段被拒绝（deny_unknown_fields）。
    #[test]
    fn scroll_open_rejects_extra_fields() {
        let json = r#"{"v":1,"type":"scroll_open","scroll_id":"x","title":"t","body_pages":["p1"],"extra":true}"#;
        let result: Result<ServerDataV1, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "scroll_open with extra field should fail deserialization (deny_unknown_fields)"
        );
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
        let channel_ids: Vec<String> = crate::cultivation::components::MeridianId::ALL
            .iter()
            .map(|m| m.channel_id().to_string())
            .collect();
        let payload = ServerDataV1::new(ServerDataPayloadV1::CultivationDetail {
            realm: "Induce".to_string(),
            channel_ids: channel_ids.clone(),
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
            target_meridian: Some(channel_ids[4].clone()),
            body_plan_id: "humanoid".to_string(),
            race_id: String::new(),
            form_race_id: String::new(),
            form_body_plan_id: String::new(),
            intrinsic_is_humanoid: false,
            form_is_humanoid: false,
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
                channel_ids: back_channel_ids,
                opened,
                flow_rate,
                lifespan,
                recent_skill_milestones_summary,
                skill_milestones,
                qi_color_main,
                qi_color_secondary,
                practice_weights,
                target_meridian,
                ..
            } => {
                assert_eq!(back_channel_ids, channel_ids);
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
                assert_eq!(target_meridian, Some(channel_ids[4].clone()));
            }
            other => panic!("expected CultivationDetail, got {other:?}"),
        }
    }

    /// plan-race-system-v1 P1c — `channel_ids`/其余并行数组不再假设恰好 20 条；一个
    /// 合成的 6 脉非 humanoid 构型（如 P5 飞鲸草案）必须同样 round-trip 成功。
    #[test]
    fn cultivation_detail_non_humanoid_channel_count_roundtrips() {
        let channel_ids = vec![
            "skull_channel".to_string(),
            "spine_channel".to_string(),
            "dorsal_fin_channel".to_string(),
            "pect_fin_l_channel".to_string(),
            "pect_fin_r_channel".to_string(),
            "tail_fin_channel".to_string(),
        ];
        let n = channel_ids.len();
        let payload = ServerDataV1::new(ServerDataPayloadV1::CultivationDetail {
            realm: "Awaken".to_string(),
            channel_ids: channel_ids.clone(),
            opened: vec![false; n],
            flow_rate: vec![1.0; n],
            flow_capacity: vec![10.0; n],
            integrity: vec![1.0; n],
            open_progress: vec![0.0; n],
            cracks_count: vec![0; n],
            contamination_total: 0.0,
            lifespan: None,
            recent_skill_milestones_summary: String::new(),
            skill_milestones: Vec::new(),
            qi_color_main: ColorKind::Mellow,
            qi_color_secondary: None,
            qi_color_chaotic: false,
            qi_color_hunyuan: false,
            practice_weights: Vec::new(),
            target_meridian: Some("tail_fin_channel".to_string()),
            body_plan_id: "whale".to_string(),
            race_id: String::new(),
            form_race_id: String::new(),
            form_body_plan_id: String::new(),
            intrinsic_is_humanoid: false,
            form_is_humanoid: false,
        });
        let bytes = payload
            .to_json_bytes_checked()
            .expect("6-channel cultivation_detail must fit MAX_PAYLOAD_BYTES");
        let back: ServerDataV1 = serde_json::from_slice(&bytes).expect("roundtrip");
        match back.payload {
            ServerDataPayloadV1::CultivationDetail {
                channel_ids: back_channel_ids,
                opened,
                target_meridian,
                ..
            } => {
                assert_eq!(
                    back_channel_ids.len(),
                    6,
                    "non-humanoid channel array length must not be forced to 20"
                );
                assert_eq!(back_channel_ids, channel_ids);
                assert_eq!(opened.len(), 6);
                assert_eq!(target_meridian, Some("tail_fin_channel".to_string()));
            }
            other => panic!("expected CultivationDetail, got {other:?}"),
        }
    }

    /// plan-race-system-v1 P1c — wire 直改新形状不留兼容层：`target_meridian` 旧形态
    /// 是数组下标（`u8`），新形态必须是 channel id 字符串；旧数字形状必须被拒绝，
    /// 不允许静默兼容解析成某个 channel。
    #[test]
    fn cultivation_detail_rejects_legacy_numeric_target_meridian() {
        let legacy_json = r#"{
            "v": 1,
            "type": "cultivation_detail",
            "realm": "Induce",
            "opened": [true],
            "flow_rate": [1.5],
            "flow_capacity": [10.25],
            "integrity": [0.87],
            "contamination_total": 0.0,
            "target_meridian": 4
        }"#;
        let result: Result<ServerDataV1, _> = serde_json::from_str(legacy_json);
        assert!(
            result.is_err(),
            "legacy numeric target_meridian (index-based) must be rejected after wire \
             open-up to channel id string, got {result:?}"
        );
    }

    /// plan-remains-suite P0 — remains_sync 双端 sample 对拍：字段值必须与
    /// agent/packages/schema/samples/server-data.remains-sync.sample.json 完全一致，
    /// 改 schema 必须连同 sample 一起改。
    #[test]
    fn remains_sync_sample_pins_wire_shape() {
        let json = include_str!(
            "../../../agent/packages/schema/samples/server-data.remains-sync.sample.json"
        );
        let payload: ServerDataV1 =
            serde_json::from_str(json).expect("remains-sync sample should deserialize");
        match payload.payload {
            ServerDataPayloadV1::RemainsSync(remains) => {
                assert_eq!(remains.len(), 1, "sample 固定 1 条 entry");
                let entry = &remains[0];
                assert_eq!(entry.remains_id, "3fa85f64-5717-4562-b3fc-2c963f66afa6");
                assert_eq!(entry.world_pos, [8.5, 66.0, 8.5]);
                assert_eq!(entry.dimension, "minecraft:overworld");
                assert_eq!(entry.display_name, "遗骸");
                assert_eq!(entry.item_count, 3);
                assert_eq!(entry.bone_coins, 12);
            }
            other => panic!("expected RemainsSync, got {other:?}"),
        }
    }

    #[test]
    fn remains_sync_rejects_entry_unknown_field() {
        let json = serde_json::json!({
            "v": SERVER_DATA_VERSION,
            "type": "remains_sync",
            "remains": [{
                "remains_id": "x",
                "world_pos": [0.0, 64.0, 0.0],
                "dimension": "minecraft:overworld",
                "display_name": "遗骸",
                "item_count": 1,
                "bone_coins": 0,
                "unexpected": true
            }]
        });

        assert!(
            serde_json::from_value::<ServerDataV1>(json).is_err(),
            "RemainsEntryV1 额外字段应被 deny_unknown_fields 拒绝"
        );
    }

    #[test]
    fn remains_sync_rejects_entry_missing_remains_id() {
        let json = serde_json::json!({
            "v": SERVER_DATA_VERSION,
            "type": "remains_sync",
            "remains": [{
                "world_pos": [0.0, 64.0, 0.0],
                "dimension": "minecraft:overworld",
                "display_name": "遗骸",
                "item_count": 1,
                "bone_coins": 0
            }]
        });

        assert!(
            serde_json::from_value::<ServerDataV1>(json).is_err(),
            "RemainsEntryV1 缺 remains_id 应反序列化失败"
        );
    }

    /// plan-race-system-v1 P2a — body_plan_layout 双端 sample 对拍：字段值必须与
    /// agent/packages/schema/samples/server-data.body-plan-layout.sample.json 完全一致，
    /// 改 schema 必须连同 sample 一起改。
    #[test]
    fn body_plan_layout_sample_pins_wire_shape() {
        let json = include_str!(
            "../../../agent/packages/schema/samples/server-data.body-plan-layout.sample.json"
        );
        let payload: ServerDataV1 =
            serde_json::from_str(json).expect("body-plan-layout sample should deserialize");
        match payload.payload {
            ServerDataPayloadV1::BodyPlanLayout(layout) => {
                assert_eq!(layout.body_plan_id, "humanoid");
                assert_eq!(
                    layout.silhouette.len(),
                    2,
                    "sample 固定 head+chest 两段剪影"
                );
                assert_eq!(layout.silhouette[0].part_id, "head");
                assert_eq!(layout.silhouette[0].polygon.len(), 4);
                assert_eq!(
                    layout.silhouette[0].polygon[0],
                    BodyPlanPoint2V1 {
                        x: 0.434524,
                        y: 0.025424
                    }
                );
                assert_eq!(layout.anchors.len(), 2);
                assert_eq!(layout.anchors[0].part_id, "head");
                assert_eq!(
                    layout.anchors[0].point,
                    BodyPlanPoint2V1 {
                        x: 0.5,
                        y: 0.042373
                    }
                );
                assert_eq!(layout.meridian_paths.len(), 1);
                assert_eq!(layout.meridian_paths[0].channel_id, "ren");
                assert_eq!(layout.meridian_paths[0].points.len(), 2);
                assert_eq!(layout.part_display_map.len(), 2);
                assert_eq!(layout.part_display_map[0].server_part_id, "head");
                assert_eq!(layout.part_display_map[0].display_segment_id, "head");
            }
            other => panic!("expected BodyPlanLayout, got {other:?}"),
        }
    }

    // ─────────────────────────────────────────────────────────────────
    // plan-race-system-v1 P3a —— RaceGateWireV1 双端 sample 对拍 + fail-closed 解码。
    // 三变体样本文件与 agent/packages/schema/samples/race-gate.*.sample.json 完全一致，
    // 改 schema 必须连同 sample 一起改。
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn race_gate_any_sample_pins_wire_shape_and_round_trips_to_owned() {
        let json = include_str!("../../../agent/packages/schema/samples/race-gate.any.sample.json");
        let wire: RaceGateWireV1 =
            serde_json::from_str(json).expect("any sample should deserialize");
        assert_eq!(wire.kind, "any");
        assert!(wire.species.is_empty());
        assert_eq!(
            wire.try_into_owned().expect("any must decode"),
            crate::body_plan::RaceGateOwned::Any
        );
    }

    #[test]
    fn race_gate_humanoid_sample_pins_wire_shape_and_round_trips_to_owned() {
        let json =
            include_str!("../../../agent/packages/schema/samples/race-gate.humanoid.sample.json");
        let wire: RaceGateWireV1 =
            serde_json::from_str(json).expect("humanoid sample should deserialize");
        assert_eq!(wire.kind, "humanoid");
        assert!(wire.species.is_empty());
        assert_eq!(
            wire.try_into_owned().expect("humanoid must decode"),
            crate::body_plan::RaceGateOwned::Humanoid
        );
    }

    #[test]
    fn race_gate_species_sample_pins_wire_shape_and_round_trips_to_owned() {
        let json =
            include_str!("../../../agent/packages/schema/samples/race-gate.species.sample.json");
        let wire: RaceGateWireV1 =
            serde_json::from_str(json).expect("species sample should deserialize");
        assert_eq!(wire.kind, "species");
        assert_eq!(wire.species, vec!["whale".to_string()]);
        assert_eq!(
            wire.try_into_owned().expect("species must decode"),
            crate::body_plan::RaceGateOwned::Species {
                species: vec![crate::body_plan::RaceId::new("whale")]
            }
        );
    }

    #[test]
    fn race_gate_wire_from_owned_round_trips_every_variant() {
        use crate::body_plan::{RaceGateOwned, RaceId};

        let cases = [
            (RaceGateOwned::Any, "any", Vec::<String>::new()),
            (RaceGateOwned::Humanoid, "humanoid", Vec::new()),
            (
                RaceGateOwned::Species {
                    species: vec![RaceId::new("whale")],
                },
                "species",
                vec!["whale".to_string()],
            ),
        ];
        for (owned, expected_kind, expected_species) in cases {
            let wire = RaceGateWireV1::from_owned(&owned);
            assert_eq!(wire.kind, expected_kind);
            assert_eq!(wire.species, expected_species);
            assert_eq!(
                wire.try_into_owned().expect("round trip must decode"),
                owned
            );
        }
    }

    #[test]
    fn race_gate_wire_species_empty_and_duplicate_preserved() {
        use crate::body_plan::RaceId;

        let empty = RaceGateWireV1 {
            kind: "species".to_string(),
            species: Vec::new(),
        };
        assert_eq!(
            empty.try_into_owned().expect("empty species list is valid"),
            crate::body_plan::RaceGateOwned::Species { species: vec![] }
        );

        let duplicate = RaceGateWireV1 {
            kind: "species".to_string(),
            species: vec!["whale".to_string(), "whale".to_string()],
        };
        match duplicate
            .try_into_owned()
            .expect("duplicate species entries are structurally valid")
        {
            crate::body_plan::RaceGateOwned::Species { species } => {
                assert_eq!(
                    species,
                    vec![RaceId::new("whale"), RaceId::new("whale")],
                    "重复条目原样保留，不做去重"
                );
            }
            other => panic!("expected Species, got {other:?}"),
        }
    }

    #[test]
    fn race_gate_wire_unknown_kind_decode_fails_closed() {
        let wire = RaceGateWireV1 {
            kind: "bogus".to_string(),
            species: Vec::new(),
        };
        let err = wire
            .try_into_owned()
            .expect_err("unknown kind must fail closed, not silently default to Any");
        assert_eq!(err.0, "bogus");
    }

    #[test]
    fn race_gate_wire_unknown_kind_json_deserialize_succeeds_but_conversion_fails_closed() {
        // RaceGateWireV1 本身是扁平结构（deny_unknown_fields 只管字段名，不管 kind 取值语义），
        // 未知 kind 字符串本身能反序列化成 RaceGateWireV1；fail-closed 拒绝发生在
        // try_into_owned() 转换语义层——两阶段分别验证，防止把"反序列化失败"和
        // "语义拒绝"混为一谈。
        let wire: RaceGateWireV1 =
            serde_json::from_str(r#"{"kind":"bogus","species":[]}"#).expect("deserialize");
        assert!(wire.try_into_owned().is_err());
    }

    /// wire 往返：BodyPlanLayout 序列化 → 反序列化必须无损（含空 anchors /
    /// meridian_paths 边界）。
    #[test]
    fn body_plan_layout_roundtrips_including_empty_optional_sections() {
        let payload = ServerDataV1::new(ServerDataPayloadV1::BodyPlanLayout(BodyPlanLayoutV1 {
            body_plan_id: "whale".to_string(),
            silhouette: vec![BodyPlanSilhouettePartV1 {
                part_id: "tail_fin".to_string(),
                polygon: vec![
                    BodyPlanPoint2V1 { x: 0.1, y: 0.9 },
                    BodyPlanPoint2V1 { x: 0.5, y: 0.8 },
                    BodyPlanPoint2V1 { x: 0.9, y: 0.9 },
                ],
            }],
            anchors: Vec::new(),
            meridian_paths: Vec::new(),
            part_display_map: Vec::new(),
            hud_anchors: Vec::new(),
        }));
        let bytes = payload
            .to_json_bytes_checked()
            .expect("body_plan_layout must serialize");
        let back: ServerDataV1 =
            serde_json::from_slice(&bytes).expect("body_plan_layout must deserialize back");
        match back.payload {
            ServerDataPayloadV1::BodyPlanLayout(layout) => {
                assert_eq!(layout.body_plan_id, "whale");
                assert_eq!(layout.silhouette.len(), 1);
                assert_eq!(layout.silhouette[0].part_id, "tail_fin");
                assert!(layout.anchors.is_empty());
                assert!(layout.meridian_paths.is_empty());
                assert!(layout.part_display_map.is_empty());
                assert!(
                    layout.hud_anchors.is_empty(),
                    "hud_anchors 是可选第二锚点组，非人形/未配置构型必须留空往返"
                );
            }
            other => panic!("expected BodyPlanLayout after roundtrip, got {other:?}"),
        }
    }

    /// plan-race-system-v1 P2 major 修复 —— `hud_anchors` 非空往返必须逐值保留，
    /// 且缺省 JSON（旧数据 / 未配置该字段的 plan）反序列化必须默认落空 `Vec`（不是
    /// 反序列化失败），两条转换分支各有专属 pin。
    #[test]
    fn body_plan_layout_hud_anchors_roundtrip_and_missing_field_defaults_to_empty() {
        let payload = ServerDataV1::new(ServerDataPayloadV1::BodyPlanLayout(BodyPlanLayoutV1 {
            body_plan_id: "humanoid".to_string(),
            silhouette: vec![BodyPlanSilhouettePartV1 {
                part_id: "head".to_string(),
                polygon: vec![
                    BodyPlanPoint2V1 { x: 0.4, y: 0.0 },
                    BodyPlanPoint2V1 { x: 0.6, y: 0.0 },
                    BodyPlanPoint2V1 { x: 0.5, y: 0.1 },
                ],
            }],
            anchors: Vec::new(),
            meridian_paths: Vec::new(),
            part_display_map: Vec::new(),
            hud_anchors: vec![BodyPlanPartAnchorV1 {
                part_id: "head".to_string(),
                point: BodyPlanPoint2V1 { x: 0.5, y: 0.04 },
            }],
        }));
        let bytes = payload
            .to_json_bytes_checked()
            .expect("body_plan_layout with hud_anchors must serialize");
        let back: ServerDataV1 = serde_json::from_slice(&bytes)
            .expect("body_plan_layout with hud_anchors must deserialize back");
        match back.payload {
            ServerDataPayloadV1::BodyPlanLayout(layout) => {
                assert_eq!(layout.hud_anchors.len(), 1);
                assert_eq!(layout.hud_anchors[0].part_id, "head");
                assert_eq!(layout.hud_anchors[0].point.x, 0.5);
                assert_eq!(layout.hud_anchors[0].point.y, 0.04);
            }
            other => panic!("expected BodyPlanLayout after roundtrip, got {other:?}"),
        }

        // 缺省字段（旧数据 / 未来非人 plan 不配置）反序列化必须默认空 Vec，不是 error。
        let legacy_json = serde_json::json!({
            "v": SERVER_DATA_VERSION,
            "type": "body_plan_layout",
            "body_plan_id": "whale",
            "silhouette": [{
                "part_id": "tail_fin",
                "polygon": [{"x": 0.1, "y": 0.9}, {"x": 0.5, "y": 0.8}, {"x": 0.9, "y": 0.9}],
            }],
            "anchors": [],
            "meridian_paths": [],
            "part_display_map": [],
        });
        let decoded: ServerDataV1 = serde_json::from_value(legacy_json)
            .expect("body_plan_layout missing hud_anchors field must default to empty, not fail");
        match decoded.payload {
            ServerDataPayloadV1::BodyPlanLayout(layout) => {
                assert!(
                    layout.hud_anchors.is_empty(),
                    "missing hud_anchors field must default to an empty Vec"
                );
            }
            other => panic!("expected BodyPlanLayout, got {other:?}"),
        }
    }

    #[test]
    fn body_plan_layout_rejects_unknown_field_in_point() {
        let json = serde_json::json!({
            "v": SERVER_DATA_VERSION,
            "type": "body_plan_layout",
            "body_plan_id": "humanoid",
            "silhouette": [{
                "part_id": "chest",
                "polygon": [
                    {"x": 0.3, "y": 0.1, "z": 0.0},
                    {"x": 0.7, "y": 0.1},
                    {"x": 0.7, "y": 0.3}
                ]
            }],
            "anchors": [],
            "meridian_paths": [],
            "part_display_map": []
        });
        assert!(
            serde_json::from_value::<ServerDataV1>(json).is_err(),
            "BodyPlanPoint2V1 额外字段（z）应被 deny_unknown_fields 拒绝——布局是 2D 归一化坐标"
        );
    }

    #[test]
    fn body_plan_layout_rejects_missing_body_plan_id() {
        let json = serde_json::json!({
            "v": SERVER_DATA_VERSION,
            "type": "body_plan_layout",
            "silhouette": [],
            "anchors": [],
            "meridian_paths": [],
            "part_display_map": []
        });
        assert!(
            serde_json::from_value::<ServerDataV1>(json).is_err(),
            "BodyPlanLayoutV1 缺 body_plan_id 应反序列化失败——它是 client 寻址缓存的主键"
        );
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
                "../../../agent/packages/schema/samples/server-data.remains-sync.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.body-plan-layout.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.botany-harvest-progress.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.gathering-session.active.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.gathering-session.completed.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.gathering-session.interrupted.sample.json"
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
                "../../../agent/packages/schema/samples/server-data.skill-config-snapshot.sample.json"
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
            include_str!(
                "../../../agent/packages/schema/samples/server-data.movement-state.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.spirit-treasure-state.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.spirit-treasure-dialogue.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.agent-ui-request.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.agent-ui-close.sample.json"
            ),
            // plan-coffin-tiers-v1 P0 charge #7：四档 + no-grade serde pin samples
            include_str!(
                "../../../agent/packages/schema/samples/server-data.coffin-state-mundane.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.coffin-state-jade.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.coffin-state-stone.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.coffin-state-bronze.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.coffin-state-no-grade.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.scroll-open.sample.json"
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
    fn player_state_requires_spirit_qi_max() {
        let json = serde_json::json!({
            "v": SERVER_DATA_VERSION,
            "type": "player_state",
            "realm": "Solidify",
            "spirit_qi": 78.0,
            "karma": 0.2,
            "composite_power": 0.35,
            "breakdown": {
                "combat": 0.2,
                "wealth": 0.4,
                "social": 0.65,
                "karma": 0.2,
                "territory": 0.1
            },
            "zone": "blood_valley"
        });

        assert!(
            serde_json::from_value::<ServerDataV1>(json).is_err(),
            "player_state 缺 spirit_qi_max 必须反序列化失败；否则 HUD 真元条会退回 100 分母"
        );
    }

    #[test]
    fn player_state_rejects_zero_spirit_qi_max() {
        let json = serde_json::json!({
            "v": SERVER_DATA_VERSION,
            "type": "player_state",
            "realm": "Solidify",
            "spirit_qi": 78.0,
            "spirit_qi_max": 0.0,
            "karma": 0.2,
            "composite_power": 0.35,
            "breakdown": {
                "combat": 0.2,
                "wealth": 0.4,
                "social": 0.65,
                "karma": 0.2,
                "territory": 0.1
            },
            "zone": "blood_valley"
        });

        assert!(
            serde_json::from_value::<ServerDataV1>(json).is_err(),
            "player_state spirit_qi_max=0 必须拒绝；proto3 缺 scalar tag 会退成 0，不能进 HUD fallback"
        );
    }

    // ─── plan-coffin-tiers-v1 P0 charge #7：CoffinGradeV1/CoffinStateV1 serde pin ────

    #[test]
    fn coffin_grade_v1_all_variants_serde_roundtrip() {
        // 每个 enum 变体至少一条专属正例
        let cases: &[(&str, CoffinGradeV1)] = &[
            ("\"mundane\"", CoffinGradeV1::Mundane),
            ("\"jade\"", CoffinGradeV1::Jade),
            ("\"stone\"", CoffinGradeV1::Stone),
            ("\"bronze\"", CoffinGradeV1::Bronze),
        ];
        for (json_str, expected) in cases {
            let parsed: CoffinGradeV1 = serde_json::from_str(json_str)
                .unwrap_or_else(|e| panic!("{json_str} should parse as CoffinGradeV1: {e}"));
            assert_eq!(
                parsed, *expected,
                "CoffinGradeV1 from {json_str} should equal {expected:?}"
            );
            let reserialized =
                serde_json::to_string(&parsed).expect("CoffinGradeV1 should serialize back");
            assert_eq!(
                reserialized, *json_str,
                "CoffinGradeV1::{expected:?} roundtrip should produce {json_str}"
            );
        }
    }

    #[test]
    fn coffin_grade_v1_rejects_unknown_variant() {
        // 反例：未知 variant 必须失败
        let result = serde_json::from_str::<CoffinGradeV1>("\"diamond\"");
        assert!(
            result.is_err(),
            "unknown grade 'diamond' should fail to deserialize"
        );
    }

    #[test]
    fn coffin_state_v1_all_grades_serde_pin() {
        // 四档 + None（出棺）serde 正例
        let cases: &[(&str, Option<CoffinGradeV1>, bool, f64)] = &[
            ("mundane", Some(CoffinGradeV1::Mundane), true, 0.9),
            ("jade", Some(CoffinGradeV1::Jade), true, 0.7),
            ("stone", Some(CoffinGradeV1::Stone), true, 0.5),
            ("bronze", Some(CoffinGradeV1::Bronze), true, 0.3),
        ];
        for (grade_str, expected_grade, in_coffin, multiplier) in cases {
            let json = serde_json::json!({
                "in_coffin": in_coffin,
                "lifespan_rate_multiplier": multiplier,
                "coffin_grade": grade_str
            });
            let state: CoffinStateV1 = serde_json::from_value(json.clone())
                .unwrap_or_else(|e| panic!("grade={grade_str} json={json} should parse: {e}"));
            assert_eq!(
                state.coffin_grade, *expected_grade,
                "grade={grade_str}: parsed coffin_grade should equal {expected_grade:?}"
            );
            assert_eq!(state.in_coffin, *in_coffin);
            assert!((state.lifespan_rate_multiplier - multiplier).abs() < 1e-9);
        }
    }

    #[test]
    fn coffin_state_v1_none_grade_serde_pin() {
        // None（出棺）：coffin_grade 字段缺失 → None（向后兼容）
        let json = serde_json::json!({
            "in_coffin": false,
            "lifespan_rate_multiplier": 1.0
        });
        let state: CoffinStateV1 =
            serde_json::from_value(json).expect("no-grade CoffinStateV1 should parse");
        assert_eq!(
            state.coffin_grade, None,
            "missing coffin_grade should parse as None (向后兼容旧 payload)"
        );
        // 序列化时 skip_serializing_if = None → 字段不出现在 JSON
        let reserialized =
            serde_json::to_value(state).expect("CoffinStateV1 should serialize to JSON value");
        assert!(
            reserialized.get("coffin_grade").is_none(),
            "coffin_grade=None should be omitted during serialization, got {reserialized}"
        );
    }

    #[test]
    fn coffin_state_v1_deny_unknown_fields_standalone() {
        // standalone 反序列化：deny_unknown_fields 拒绝多余字段
        let json = serde_json::json!({
            "in_coffin": true,
            "lifespan_rate_multiplier": 0.9,
            "unknown_field": "oops"
        });
        let result = serde_json::from_value::<CoffinStateV1>(json);
        assert!(
            result.is_err(),
            "CoffinStateV1 standalone deny_unknown_fields should reject extra fields"
        );
    }

    #[test]
    fn gathering_session_rejects_invalid_enum_values() {
        let invalid_quality =
            include_str!("../../../agent/packages/schema/samples/server-data.gathering-session.invalid-quality.sample.json");
        assert!(
            serde_json::from_str::<ServerDataV1>(invalid_quality).is_err(),
            "invalid gathering_session quality_hint sample should fail to deserialize"
        );

        let invalid_target = serde_json::json!({
            "v": SERVER_DATA_VERSION,
            "type": "gathering_session",
            "session_id": "gathering:bad-target",
            "progress_ticks": 10,
            "total_ticks": 40,
            "target_name": "测试采集物",
            "target_type": "invalid_type",
            "quality_hint": "normal",
            "interrupted": false,
            "completed": false
        });
        assert!(
            serde_json::from_value::<ServerDataV1>(invalid_target).is_err(),
            "invalid gathering_session target_type should fail to deserialize"
        );
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
    fn ascension_quota_defaults_new_world_qi_fields_for_legacy_payloads() {
        let payload: AscensionQuotaV1 =
            serde_json::from_str(r#"{"occupied_slots":1,"quota_limit":3,"available_slots":2}"#)
                .expect("legacy ascension quota payload should deserialize");

        assert_eq!(payload, AscensionQuotaV1::new(1, 3));
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

    #[test]
    fn container_kind_v1_surface_stash_wire() {
        use crate::network::tsy_container_search_emit::container_kind_wire;
        use crate::world::tsy_container::ContainerKind;

        assert_eq!(
            container_kind_wire(ContainerKind::SurfaceStash),
            ContainerKindV1::SurfaceStash,
            "ContainerKind::SurfaceStash should map to ContainerKindV1::SurfaceStash"
        );
    }

    #[test]
    fn container_kind_v1_serde_pin_with_surface_stash() {
        let json = serde_json::to_string(&ContainerKindV1::SurfaceStash)
            .expect("ContainerKindV1::SurfaceStash should serialize");
        assert_eq!(
            json, "\"surface_stash\"",
            "ContainerKindV1::SurfaceStash serde should produce \"surface_stash\", got {json}"
        );
        let round: ContainerKindV1 = serde_json::from_str(&json).expect("should deserialize back");
        assert_eq!(round, ContainerKindV1::SurfaceStash);
    }

    #[test]
    fn technique_proficiency_update_rejects_missing_gain() {
        let missing_gain = serde_json::json!({
            "v": SERVER_DATA_VERSION,
            "type": "technique_proficiency_update",
            "update": {
                "technique_id": "sword.cleave",
                "proficiency": 0.42
            }
        });
        assert!(
            serde_json::from_value::<ServerDataV1>(missing_gain).is_err(),
            "technique_proficiency_update missing 'gain' should fail deserialization"
        );
    }

    #[test]
    fn technique_proficiency_update_rejects_unknown_field() {
        let unknown_field = serde_json::json!({
            "v": SERVER_DATA_VERSION,
            "type": "technique_proficiency_update",
            "update": {
                "technique_id": "sword.cleave",
                "proficiency": 0.42,
                "gain": 0.008,
                "unexpected": true
            }
        });
        assert!(
            serde_json::from_value::<ServerDataV1>(unknown_field).is_err(),
            "technique_proficiency_update with unknown field should fail due to deny_unknown_fields"
        );
    }

    #[test]
    fn pill_buff_status_v1_serde_pin() {
        let original = PillBuffStatusV1 {
            buff_id: "huo_xue_dan".to_string(),
            remaining_ticks: 3000,
            effect_multiplier: 1.0,
        };
        let json = serde_json::to_string(&original).expect("PillBuffStatusV1 should serialize");
        let back: PillBuffStatusV1 =
            serde_json::from_str(&json).expect("PillBuffStatusV1 should deserialize");
        assert_eq!(
            original, back,
            "PillBuffStatusV1 roundtrip must be lossless"
        );

        let envelope = ServerDataV1::new(ServerDataPayloadV1::PillBuffStatus(original.clone()));
        let bytes = serde_json::to_vec(&envelope).expect("envelope should serialize");
        let round: ServerDataV1 =
            serde_json::from_slice(&bytes).expect("envelope should roundtrip");
        match round.payload {
            ServerDataPayloadV1::PillBuffStatus(status) => {
                assert_eq!(
                    status, original,
                    "envelope roundtrip must preserve PillBuffStatusV1"
                );
            }
            other => panic!("expected PillBuffStatus, got {other:?}"),
        }
    }

    #[test]
    fn pill_buff_status_v1_rejects_unknown_field() {
        let unknown_field = serde_json::json!({
            "v": SERVER_DATA_VERSION,
            "type": "pill_buff_status",
            "buff_id": "tie_bi_san",
            "remaining_ticks": 600,
            "effect_multiplier": 1.2,
            "unexpected": true
        });
        assert!(
            serde_json::from_value::<ServerDataV1>(unknown_field).is_err(),
            "PillBuffStatusV1 with unknown field should fail due to deny_unknown_fields"
        );
    }

    #[test]
    fn pill_buff_status_v1_rejects_missing_buff_id() {
        let missing = serde_json::json!({
            "v": SERVER_DATA_VERSION,
            "type": "pill_buff_status",
            "remaining_ticks": 600,
            "effect_multiplier": 1.2
        });
        assert!(
            serde_json::from_value::<ServerDataV1>(missing).is_err(),
            "PillBuffStatusV1 missing 'buff_id' should fail deserialization"
        );
    }

    #[test]
    fn pill_buff_status_v1_zero_ticks_roundtrips() {
        let zero = PillBuffStatusV1 {
            buff_id: "expired_buff".to_string(),
            remaining_ticks: 0,
            effect_multiplier: 0.0,
        };
        let json =
            serde_json::to_string(&zero).expect("zero-tick PillBuffStatusV1 should serialize");
        let back: PillBuffStatusV1 =
            serde_json::from_str(&json).expect("zero-tick should deserialize");
        assert_eq!(zero, back);
    }

    // ─── plan-supply-coffin-loot-ui P1：外部容器 S2C tests ──────────

    fn sample_placed_item() -> super::super::inventory::PlacedInventoryItemV1 {
        super::super::inventory::PlacedInventoryItemV1 {
            container_id: "ext_42".to_string(),
            row: 0,
            col: 1,
            item: super::super::inventory::InventoryItemViewV1 {
                instance_id: 100,
                item_id: "iron_sword".to_string(),
                display_name: "铁剑".to_string(),
                grid_width: 1,
                grid_height: 2,
                weight: 2.5,
                rarity: super::super::inventory::ItemRarityV1::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 0.0,
                durability: 1.0,
                freshness: None,
                freshness_current: None,
                mineral_id: None,
                scroll_kind: None,
                scroll_skill_id: None,
                scroll_xp_grant: None,
                charges: None,
                forge_quality: None,
                forge_color: None,
                forge_side_effects: vec![],
                forge_achieved_tier: None,
                alchemy: None,
                lingering_owner_qi: None,
            },
        }
    }

    #[test]
    fn loot_container_open_serde_roundtrip() {
        let original = LootContainerOpenV1 {
            session_id: 42,
            source_kind: LootContainerSourceKindV1::SupplyCoffin {
                grade: "common".to_string(),
            },
            rows: 3,
            cols: 4,
            placed_items: vec![sample_placed_item()],
            timeout_wall_secs: 1716872400,
        };
        let json = serde_json::to_string(&original).expect("LootContainerOpenV1 should serialize");
        let back: LootContainerOpenV1 =
            serde_json::from_str(&json).expect("LootContainerOpenV1 should deserialize");
        assert_eq!(
            original, back,
            "LootContainerOpenV1 roundtrip must be lossless"
        );
    }

    #[test]
    fn loot_container_open_envelope_roundtrip() {
        let payload = ServerDataPayloadV1::LootContainerOpen(LootContainerOpenV1 {
            session_id: 7,
            source_kind: LootContainerSourceKindV1::SupplyCoffin {
                grade: "rare".to_string(),
            },
            rows: 4,
            cols: 5,
            placed_items: vec![],
            timeout_wall_secs: 1716872500,
        });
        let envelope = ServerDataV1::new(payload.clone());
        let bytes = serde_json::to_vec(&envelope).expect("envelope should serialize");
        let round: ServerDataV1 =
            serde_json::from_slice(&bytes).expect("envelope should roundtrip");
        assert_eq!(
            round.payload.payload_type(),
            ServerDataType::LootContainerOpen,
            "deserialized type must be LootContainerOpen"
        );
    }

    #[test]
    fn loot_container_open_empty_items_roundtrips() {
        let open = LootContainerOpenV1 {
            session_id: 0,
            source_kind: LootContainerSourceKindV1::SupplyCoffin {
                grade: "precious".to_string(),
            },
            rows: 5,
            cols: 6,
            placed_items: vec![],
            timeout_wall_secs: 0,
        };
        let json = serde_json::to_string(&open)
            .expect("LootContainerOpenV1 with empty items should serialize");
        let back: LootContainerOpenV1 = serde_json::from_str(&json)
            .expect("LootContainerOpenV1 with empty items should deserialize");
        assert!(
            back.placed_items.is_empty(),
            "empty placed_items must survive roundtrip"
        );
    }

    #[test]
    fn loot_container_update_serde_roundtrip() {
        let original = LootContainerUpdateV1 {
            session_id: 42,
            placed_items: vec![sample_placed_item()],
        };
        let json =
            serde_json::to_string(&original).expect("LootContainerUpdateV1 should serialize");
        let back: LootContainerUpdateV1 =
            serde_json::from_str(&json).expect("LootContainerUpdateV1 should deserialize");
        assert_eq!(
            original, back,
            "LootContainerUpdateV1 roundtrip must be lossless"
        );
    }

    #[test]
    fn loot_container_close_all_reasons_roundtrip() {
        let reasons = [
            LootContainerCloseReasonV1::Timeout,
            LootContainerCloseReasonV1::Distance,
            LootContainerCloseReasonV1::PlayerClosed,
            LootContainerCloseReasonV1::CoffinDestroyed,
            LootContainerCloseReasonV1::ContainerDestroyed,
        ];
        for reason in reasons {
            let close = LootContainerCloseV1 {
                session_id: 99,
                reason: reason.clone(),
            };
            let json =
                serde_json::to_string(&close).expect("LootContainerCloseV1 should serialize");
            let back: LootContainerCloseV1 =
                serde_json::from_str(&json).expect("LootContainerCloseV1 should deserialize");
            assert_eq!(
                close, back,
                "LootContainerCloseV1 roundtrip must be lossless for reason {reason:?}"
            );
        }
    }

    #[test]
    fn loot_container_close_envelope_roundtrip() {
        let payload = ServerDataPayloadV1::LootContainerClose(LootContainerCloseV1 {
            session_id: 5,
            reason: LootContainerCloseReasonV1::Timeout,
        });
        let envelope = ServerDataV1::new(payload);
        let bytes = serde_json::to_vec(&envelope).expect("envelope should serialize");
        let round: ServerDataV1 =
            serde_json::from_slice(&bytes).expect("envelope should roundtrip");
        assert_eq!(
            round.payload.payload_type(),
            ServerDataType::LootContainerClose,
            "deserialized type must be LootContainerClose"
        );
    }

    #[test]
    fn loot_container_source_kind_supply_coffin_wire_format() {
        let kind = LootContainerSourceKindV1::SupplyCoffin {
            grade: "common".to_string(),
        };
        let json = serde_json::to_string(&kind)
            .expect("LootContainerSourceKindV1::SupplyCoffin should serialize");
        assert!(
            json.contains("\"supply_coffin\""),
            "source_kind wire should use snake_case tag, got: {json}"
        );
        assert!(
            json.contains("\"grade\":\"common\""),
            "source_kind wire should contain grade field, got: {json}"
        );
    }

    #[test]
    fn loot_container_source_kind_storage_crate_wire_format() {
        let kind = LootContainerSourceKindV1::StorageCrate { is_herb: true };
        let json = serde_json::to_string(&kind)
            .expect("LootContainerSourceKindV1::StorageCrate should serialize");
        assert!(
            json.contains("\"storage_crate\""),
            "source_kind wire should use snake_case tag, got: {json}"
        );
        assert!(
            json.contains("\"is_herb\":true"),
            "source_kind wire should contain is_herb field, got: {json}"
        );
        let back: LootContainerSourceKindV1 =
            serde_json::from_str(&json).expect("StorageCrate source_kind should deserialize");
        assert_eq!(
            kind, back,
            "StorageCrate source_kind must roundtrip without losing is_herb"
        );
    }

    #[test]
    fn loot_container_source_kind_dead_drop_wire_format() {
        let kind = LootContainerSourceKindV1::DeadDrop;
        let json = serde_json::to_string(&kind)
            .expect("LootContainerSourceKindV1::DeadDrop should serialize");
        assert_eq!(
            json, "\"dead_drop\"",
            "unit source_kind wire should be the snake_case tag"
        );
        let back: LootContainerSourceKindV1 =
            serde_json::from_str(&json).expect("DeadDrop source_kind should deserialize");
        assert_eq!(kind, back, "DeadDrop source_kind must roundtrip");
    }

    #[test]
    fn loot_container_close_reason_wire_values() {
        let cases = [
            (LootContainerCloseReasonV1::Timeout, "\"timeout\""),
            (LootContainerCloseReasonV1::Distance, "\"distance\""),
            (
                LootContainerCloseReasonV1::PlayerClosed,
                "\"player_closed\"",
            ),
            (
                LootContainerCloseReasonV1::CoffinDestroyed,
                "\"coffin_destroyed\"",
            ),
            (
                LootContainerCloseReasonV1::ContainerDestroyed,
                "\"container_destroyed\"",
            ),
        ];
        for (reason, expected) in cases {
            let json = serde_json::to_string(&reason)
                .expect("LootContainerCloseReasonV1 variant should serialize");
            assert_eq!(
                json, expected,
                "LootContainerCloseReasonV1::{reason:?} wire value mismatch"
            );
        }
    }

    #[test]
    fn payload_type_label_matches_for_loot_container_types() {
        assert_eq!(
            payload_type_label(ServerDataType::LootContainerOpen),
            "loot_container_open"
        );
        assert_eq!(
            payload_type_label(ServerDataType::LootContainerUpdate),
            "loot_container_update"
        );
        assert_eq!(
            payload_type_label(ServerDataType::LootContainerClose),
            "loot_container_close"
        );
    }

    #[test]
    fn loot_container_open_rejects_missing_session_id() {
        let json = r#"{"source_kind":{"kind":"supply_coffin","grade":"common"},"rows":3,"cols":4,"placed_items":[],"timeout_wall_secs":0}"#;
        assert!(
            serde_json::from_str::<LootContainerOpenV1>(json).is_err(),
            "LootContainerOpenV1 missing session_id should fail deserialization"
        );
    }

    #[test]
    fn loot_container_close_rejects_missing_reason() {
        let json = r#"{"session_id":1}"#;
        assert!(
            serde_json::from_str::<LootContainerCloseV1>(json).is_err(),
            "LootContainerCloseV1 missing reason should fail deserialization"
        );
    }

    #[test]
    fn loot_container_close_rejects_unknown_reason() {
        let json = r#"{"session_id":1,"reason":"alien_abduction"}"#;
        assert!(
            serde_json::from_str::<LootContainerCloseV1>(json).is_err(),
            "LootContainerCloseV1 unknown reason should fail deserialization"
        );
    }

    #[test]
    fn loot_container_open_rejects_missing_placed_items() {
        let json = r#"{"session_id":1,"source_kind":{"kind":"supply_coffin","grade":"rare"},"rows":5,"cols":4,"timeout_wall_secs":100}"#;
        assert!(
            serde_json::from_str::<LootContainerOpenV1>(json).is_err(),
            "LootContainerOpenV1 missing placed_items should fail deserialization"
        );
    }

    #[test]
    fn loot_container_update_rejects_missing_session_id() {
        let json = r#"{"placed_items":[]}"#;
        assert!(
            serde_json::from_str::<LootContainerUpdateV1>(json).is_err(),
            "LootContainerUpdateV1 missing session_id should fail deserialization"
        );
    }

    // ─── plan-offscreen-war-v1 P9：FactionWarState payload 测试 ─────────────

    #[test]
    fn faction_war_state_v1_roundtrips_with_outcome() {
        // 有 winner/loser 的 Settling 阶段 payload 完整无损 roundtrip。
        let payload = FactionWarStateV1 {
            war_id: 42,
            zone: "残灰谷".to_string(),
            region_descriptor: "残灰谷一带散修".to_string(),
            phase: "settling".to_string(),
            groups: vec![0, 1],
            enlist_count: 3,
            mercenary_count: 1,
            intercept_count: 0,
            spectate_count: 2,
            winner_group: Some(0),
            loser_group: Some(1),
        };
        let json = serde_json::to_string(&payload).expect("FactionWarStateV1 should serialize");
        let back: FactionWarStateV1 =
            serde_json::from_str(&json).expect("FactionWarStateV1 should deserialize");
        assert_eq!(
            payload, back,
            "FactionWarStateV1 roundtrip must be lossless"
        );
        // winner_group/loser_group Some 时 JSON 应包含这两个字段
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            v.get("winner_group").is_some(),
            "winner_group should be in JSON when Some"
        );
        assert!(
            v.get("loser_group").is_some(),
            "loser_group should be in JSON when Some"
        );
    }

    #[test]
    fn faction_war_state_v1_roundtrips_without_outcome() {
        // 无 winner/loser 的 Skirmish 阶段：winner_group/loser_group 字段应被 skip_serializing。
        let payload = FactionWarStateV1 {
            war_id: 7,
            zone: "残灰谷".to_string(),
            region_descriptor: "残灰谷一带散修".to_string(),
            phase: "skirmish".to_string(),
            groups: vec![0, 1],
            enlist_count: 1,
            mercenary_count: 0,
            intercept_count: 0,
            spectate_count: 0,
            winner_group: None,
            loser_group: None,
        };
        let json = serde_json::to_string(&payload).expect("serialize");
        let back: FactionWarStateV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            payload, back,
            "FactionWarStateV1 no-outcome roundtrip must be lossless"
        );
        // None 时 JSON 不含 winner_group/loser_group（skip_serializing_if）
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            v.get("winner_group").is_none(),
            "winner_group should be absent when None"
        );
        assert!(
            v.get("loser_group").is_none(),
            "loser_group should be absent when None"
        );
    }

    #[test]
    fn faction_war_state_wire_type_label_is_faction_war_state() {
        // payload_type_label → "faction_war_state"（历史 wire label 兼容）
        let label = payload_type_label(ServerDataType::FactionWarState);
        assert_eq!(
            label, "faction_war_state",
            "期望 FactionWarState 的 label 为 'faction_war_state'（历史 wire label），实际 {label}"
        );
    }

    #[test]
    fn faction_war_state_serializes_type_field_as_faction_war_state() {
        // wire type tag "faction_war_state" 保持历史兼容。
        // ServerDataV1 用 #[serde(flatten)]，所以 type + fields 全在顶层（无 "payload" 嵌套）。
        let inner = FactionWarStateV1 {
            war_id: 1,
            zone: "血谷".to_string(),
            region_descriptor: "血谷一带散修".to_string(),
            phase: "emerging".to_string(),
            groups: vec![2, 3],
            enlist_count: 0,
            mercenary_count: 0,
            intercept_count: 0,
            spectate_count: 0,
            winner_group: None,
            loser_group: None,
        };
        let wrapper = ServerDataV1::new(ServerDataPayloadV1::FactionWarState(inner));
        let json = serde_json::to_string(&wrapper).expect("serialize wrapper");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        // payload 字段 flatten 到顶层，type 字段在顶层
        assert_eq!(
            v["type"],
            serde_json::json!("faction_war_state"),
            "期望 wire type = 'faction_war_state'（守恒：payload 零真元，reframe b 零宗门），实际 {}",
            v["type"]
        );
        // 守恒红线：不含任何真元字段名（qi 在字段名中不应出现）
        assert!(
            !json.contains("\"qi\"") && !json.contains("_qi\"") && !json.contains("\"qi_"),
            "期望 faction_war_state JSON 不含 qi 字段（零真元），实际 JSON: {json}"
        );
        // reframe b：region_descriptor 含「散修」
        assert!(
            v["region_descriptor"]
                .as_str()
                .unwrap_or("")
                .contains("散修"),
            "期望 region_descriptor 含「散修」（匿名散修描述符），实际 {}",
            v["region_descriptor"]
        );
    }

    // ─── plan-combat-skill-feedback-bridges-v1 P4：AnqiHud schema pin ─

    #[derive(Debug, serde::Deserialize)]
    struct AnqiHudWireCorpus {
        base: serde_json::Value,
        cases: Vec<AnqiHudWireCorpusCase>,
    }

    #[derive(Debug, serde::Deserialize)]
    struct AnqiHudWireCorpusCase {
        name: String,
        accepted: bool,
        set: Option<serde_json::Map<String, serde_json::Value>>,
        remove: Option<String>,
    }

    fn materialize_anqi_hud_wire_case(
        base: &serde_json::Value,
        test_case: &AnqiHudWireCorpusCase,
    ) -> serde_json::Value {
        let mut payload = base
            .as_object()
            .expect("anqi_hud wire corpus base must be an object")
            .clone();
        let mutation_count = test_case.set.as_ref().map_or(0, serde_json::Map::len)
            + usize::from(test_case.remove.is_some());
        assert!(
            mutation_count <= 1,
            "corpus case '{}' must isolate at most one field constraint",
            test_case.name
        );

        if let Some(fields) = &test_case.set {
            for (field, value) in fields {
                payload.insert(field.clone(), value.clone());
            }
        }
        if let Some(field) = &test_case.remove {
            payload.remove(field);
        }
        serde_json::Value::Object(payload)
    }

    #[test]
    fn anqi_hud_shared_wire_corpus_matches_rust_serde() {
        let corpus: AnqiHudWireCorpus = serde_json::from_str(include_str!(
            "../../../agent/packages/schema/samples/server-data.anqi-hud.wire-corpus.json"
        ))
        .expect("shared anqi_hud wire corpus must be valid JSON");
        let mut names = std::collections::HashSet::new();

        for test_case in &corpus.cases {
            assert!(
                names.insert(test_case.name.as_str()),
                "duplicate corpus case '{}'",
                test_case.name
            );
            let payload = materialize_anqi_hud_wire_case(&corpus.base, test_case);
            let result = serde_json::from_value::<ServerDataV1>(payload.clone());
            assert_eq!(
                result.is_ok(),
                test_case.accepted,
                "Rust serde verdict drifted for case '{}'; payload={payload}; result={result:?}",
                test_case.name
            );

            if let Ok(wrapper) = result {
                assert_eq!(
                    wrapper.payload_type(),
                    ServerDataType::AnqiHud,
                    "accepted corpus case '{}' must retain the anqi_hud payload type",
                    test_case.name
                );
                assert!(
                    matches!(&wrapper.payload, ServerDataPayloadV1::AnqiHud(_)),
                    "accepted corpus case '{}' must deserialize to the AnqiHud variant; actual={:?}",
                    test_case.name,
                    wrapper.payload
                );
            }
        }
    }

    #[test]
    fn anqi_hud_shared_samples_match_rust_serde_verdicts() {
        let valid_samples = [
            include_str!(
                "../../../agent/packages/schema/samples/server-data.anqi-hud.echo.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.anqi-hud.aim.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.anqi-hud.charge.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.anqi-hud.abrasion.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.anqi-hud.multishot.sample.json"
            ),
        ];
        for sample in valid_samples {
            let wrapper: ServerDataV1 = serde_json::from_str(sample)
                .expect("valid shared anqi_hud sample must deserialize");
            assert_eq!(
                wrapper.payload_type(),
                ServerDataType::AnqiHud,
                "valid shared sample must retain the anqi_hud payload type; sample={sample}"
            );
            assert!(
                matches!(&wrapper.payload, ServerDataPayloadV1::AnqiHud(_)),
                "valid shared sample must deserialize to the AnqiHud variant; actual={:?}; sample={sample}",
                wrapper.payload
            );
        }

        let invalid_samples = [
            include_str!(
                "../../../agent/packages/schema/samples/server-data.anqi-hud.invalid-missing-field.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.anqi-hud.invalid-extra-field.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.anqi-hud.invalid-kind.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.anqi-hud.invalid-tick-overflow.sample.json"
            ),
        ];
        for sample in invalid_samples {
            assert!(
                serde_json::from_str::<ServerDataV1>(sample).is_err(),
                "invalid shared anqi_hud sample must be rejected: {sample}"
            );
        }
    }

    #[test]
    fn anqi_hud_v1_roundtrip() {
        let original = crate::schema::server_data::AnqiHudV1 {
            kind: AnqiHudKindV1::Abrasion,
            echo_count: 3,
            aim_progress: 0.5,
            charge_progress: 0.25,
            abrasion_container: "quiver".to_string(),
            abrasion_qi_payload: 12.5,
            tick: 999,
        };
        let json = serde_json::to_string(&original).expect("AnqiHudV1 应能序列化");
        let back: crate::schema::server_data::AnqiHudV1 =
            serde_json::from_str(&json).expect("AnqiHudV1 应能反序列化");
        assert_eq!(
            original, back,
            "AnqiHudV1 JSON roundtrip 必须无损；JSON={json}"
        );
    }

    #[test]
    fn anqi_hud_payload_type_label_is_anqi_hud() {
        let label = payload_type_label(ServerDataType::AnqiHud);
        assert_eq!(
            label, "anqi_hud",
            "期望 AnqiHud 的 label 为 'anqi_hud'（client 路由键），实际 {label}"
        );
    }

    #[test]
    fn anqi_hud_variant_type_and_complete_wire_shape_serialize_together() {
        let inner = crate::schema::server_data::AnqiHudV1 {
            kind: AnqiHudKindV1::Echo,
            echo_count: 5,
            aim_progress: 0.0,
            charge_progress: 0.0,
            abrasion_container: String::new(),
            abrasion_qi_payload: 0.0,
            tick: 42,
        };
        let wrapper = ServerDataV1::new(ServerDataPayloadV1::AnqiHud(inner));
        assert_eq!(
            wrapper.payload_type(),
            ServerDataType::AnqiHud,
            "AnqiHud wrapper must report the payload type used by client routing"
        );
        let value = serde_json::to_value(&wrapper).expect("serialize AnqiHud wrapper");
        assert_eq!(
            value,
            serde_json::json!({
                "v": SERVER_DATA_VERSION,
                "type": "anqi_hud",
                "kind": "echo",
                "echo_count": 5,
                "aim_progress": 0.0,
                "charge_progress": 0.0,
                "abrasion_container": "",
                "abrasion_qi_payload": 0.0,
                "tick": 42
            }),
            "AnqiHud wrapper must serialize every canonical v1 wire field together"
        );
        assert_eq!(
            payload_type_label(wrapper.payload_type()),
            value["type"].as_str().expect("wire type must be a string"),
            "payload_type_label must match the serialized wire type used by client routing"
        );
        let decoded: ServerDataV1 =
            serde_json::from_value(value).expect("serialized wrapper must deserialize");
        let ServerDataPayloadV1::AnqiHud(decoded_hud) = decoded.payload else {
            panic!("wire type anqi_hud must deserialize to the AnqiHud payload variant");
        };
        assert_eq!(
            decoded_hud.kind,
            AnqiHudKindV1::Echo,
            "complete wire shape must preserve the echo kind after deserialization"
        );
        assert_eq!(
            decoded_hud.echo_count, 5,
            "complete wire shape must preserve echo_count=5 after deserialization"
        );
        assert_eq!(
            decoded_hud.tick, 42,
            "complete wire shape must preserve tick=42 after deserialization"
        );
    }

    #[test]
    fn anqi_hud_invalid_outbound_values_fail_serialization() {
        let invalid_payloads = [
            AnqiHudV1 {
                kind: AnqiHudKindV1::Aim,
                echo_count: 0,
                aim_progress: f64::NAN,
                charge_progress: 0.0,
                abrasion_container: String::new(),
                abrasion_qi_payload: 0.0,
                tick: 0,
            },
            AnqiHudV1 {
                kind: AnqiHudKindV1::Charge,
                echo_count: 0,
                aim_progress: 0.0,
                charge_progress: f64::INFINITY,
                abrasion_container: String::new(),
                abrasion_qi_payload: 0.0,
                tick: 0,
            },
            AnqiHudV1 {
                kind: AnqiHudKindV1::Abrasion,
                echo_count: 0,
                aim_progress: 0.0,
                charge_progress: 0.0,
                abrasion_container: String::new(),
                abrasion_qi_payload: -1.0,
                tick: 0,
            },
            AnqiHudV1 {
                kind: AnqiHudKindV1::Multishot,
                echo_count: ANQI_HUD_ECHO_COUNT_MAX + 1,
                aim_progress: 0.0,
                charge_progress: 0.0,
                abrasion_container: String::new(),
                abrasion_qi_payload: 0.0,
                tick: 0,
            },
            AnqiHudV1 {
                kind: AnqiHudKindV1::Abrasion,
                echo_count: 0,
                aim_progress: 0.0,
                charge_progress: 0.0,
                abrasion_container: "unknown".to_string(),
                abrasion_qi_payload: 0.0,
                tick: 0,
            },
            AnqiHudV1 {
                kind: AnqiHudKindV1::Abrasion,
                echo_count: 0,
                aim_progress: 0.0,
                charge_progress: 0.0,
                abrasion_container: "quiver".to_string(),
                abrasion_qi_payload: ANQI_HUD_QI_PAYLOAD_MAX * 2.0,
                tick: 0,
            },
            AnqiHudV1 {
                kind: AnqiHudKindV1::Echo,
                echo_count: 0,
                aim_progress: 0.0,
                charge_progress: 0.0,
                abrasion_container: String::new(),
                abrasion_qi_payload: 0.0,
                tick: ANQI_HUD_TICK_MAX + 1,
            },
        ];

        for payload in invalid_payloads {
            let wrapper = ServerDataV1::new(ServerDataPayloadV1::AnqiHud(payload));
            assert!(
                serde_json::to_value(wrapper).is_err(),
                "invalid outbound anqi_hud payload must fail serialization"
            );
        }
    }

    // ─── 震脉 v2 HUD S2C：schema pin（字段须与 client ZhenmaiHudServerDataHandler 逐一对齐） ─

    #[test]
    fn zhenmai_hud_v1_roundtrip() {
        let original = crate::schema::server_data::ZhenmaiHudV1 {
            skill_id: "sever_chain".to_string(),
            meridian_id: "Heart".to_string(),
            contam_removed: 0.0,
            remaining_points: 0,
            damage_reduction: 0.0,
            k_drain: 1.5,
            duration_ms: 60_000,
            tick: 999,
        };
        let json = serde_json::to_string(&original).expect("ZhenmaiHudV1 应能序列化");
        let back: crate::schema::server_data::ZhenmaiHudV1 =
            serde_json::from_str(&json).expect("ZhenmaiHudV1 应能反序列化");
        assert_eq!(
            original, back,
            "ZhenmaiHudV1 JSON roundtrip 必须无损；JSON={json}"
        );
    }

    #[test]
    fn zhenmai_hud_payload_type_label_is_zhenmai_hud() {
        let label = payload_type_label(ServerDataType::ZhenmaiHud);
        assert_eq!(
            label, "zhenmai_hud",
            "期望 ZhenmaiHud 的 label 为 'zhenmai_hud'（client ServerDataRouter 路由键），实际 {label}"
        );
    }

    #[test]
    fn zhenmai_hud_wire_emits_client_contract_fields() {
        // 字段名/类型须与 client ZhenmaiHudServerDataHandler.readString/readDouble/readDuration 对齐。
        let inner = crate::schema::server_data::ZhenmaiHudV1 {
            skill_id: "neutralize".to_string(),
            meridian_id: "Lung".to_string(),
            contam_removed: 2.5,
            remaining_points: 0,
            damage_reduction: 0.0,
            k_drain: 0.0,
            duration_ms: 0,
            tick: 64,
        };
        let wrapper = ServerDataV1::new(ServerDataPayloadV1::ZhenmaiHud(inner));
        let json = serde_json::to_string(&wrapper).expect("serialize ZhenmaiHud wrapper");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            v["type"],
            serde_json::json!("zhenmai_hud"),
            "wire type 须为 client 路由键 'zhenmai_hud'，实际 {}",
            v["type"]
        );
        // client switch(skill_id) 的判别键
        assert_eq!(
            v["skill_id"],
            serde_json::json!("neutralize"),
            "skill_id 须为 client switch 键 'neutralize'，实际 {}",
            v["skill_id"]
        );
        // client readString("meridian_id")
        assert_eq!(v["meridian_id"], serde_json::json!("Lung"));
        // client readDouble("contam_removed", 0.0)
        assert_eq!(v["contam_removed"], serde_json::json!(2.5));
        // 契约字段全部在场（即使为零值，flatten 不 skip → client readX 各有所依）
        for field in [
            "skill_id",
            "meridian_id",
            "contam_removed",
            "remaining_points",
            "damage_reduction",
            "k_drain",
            "duration_ms",
            "tick",
        ] {
            assert!(
                v.get(field).is_some(),
                "ZhenmaiHud wire 须含 client 契约字段 '{field}'；实际 JSON={json}"
            );
        }
    }

    #[test]
    fn zhenmai_hud_harden_wire_damage_reduction_is_reduction_not_passthrough() {
        // 契约语义 pin：client ZhenmaiHudPlanner.appendHarden 把 damage_reduction 当作
        // 「减伤比例」渲染（value1*100 → 「减伤X%」、条形填充 = value1，1.0=全免）。
        // 因此 wire 的 damage_reduction 必须是减伤比例（reduction），而不是 server 内部
        // HardenProfile.damage_multiplier 的「伤害通过率」（passthrough）。
        // bridge 负责转换 reduction = 1 - passthrough（见 zhenmai_v2_event_bridge.rs harden 分支）；
        // 本 pin 锁住 wire 形态：harden 场景下 damage_reduction 是 [0,1] 的减伤比例。
        // 例：Spirit 境 passthrough=0.35 → wire damage_reduction=0.65（实际减伤 65%）。
        let inner = crate::schema::server_data::ZhenmaiHudV1 {
            skill_id: "harden".to_string(),
            meridian_id: "Heart".to_string(),
            contam_removed: 0.0,
            remaining_points: 0,
            damage_reduction: 0.65,
            k_drain: 0.0,
            duration_ms: 1_000,
            tick: 70,
        };
        let wrapper = ServerDataV1::new(ServerDataPayloadV1::ZhenmaiHud(inner));
        let json = serde_json::to_string(&wrapper).expect("serialize harden ZhenmaiHud wrapper");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["skill_id"], serde_json::json!("harden"));
        let reduction = v["damage_reduction"]
            .as_f64()
            .expect("damage_reduction 须为数值");
        assert!(
            (reduction - 0.65).abs() < 1e-4,
            "harden wire damage_reduction 须为减伤比例 0.65（client 渲染「减伤65%」），\
             不是 passthrough multiplier 0.35；实际 {reduction}（JSON={json}）"
        );
        assert!(
            (0.0..=1.0).contains(&reduction),
            "damage_reduction 须落在减伤比例区间 [0,1]，实际 {reduction}"
        );
    }
}
