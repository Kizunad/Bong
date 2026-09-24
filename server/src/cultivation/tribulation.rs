//! 化虚渡劫（plan §3.2）。
//!
//! Spirit → Void 的唯一通路，流程：
//!   1. 玩家 `InitiateXuhuaTribulation` → 进入 TribulationState
//!   2. 全服广播（由 network 层消费 `TribulationAnnounce`）
//!   3. calamity agent 生成天劫脚本（多波次），本 plan 接收 `TribulationWave`
//!      事件并让战斗 plan 施加伤害（此处不实现）
//!   4. 扛过所有波次 → realm = Void；任一波次失败 → 退回通灵初期，不进入死亡流程
//!
//! P1/P5：本文件只定义状态机 + 事件；真实天劫伤害由战斗 plan 实施。

use valence::prelude::{
    bevy_ecs, BlockPos, BlockState, ChunkLayer, ChunkPos, Client, Commands, Component, Entity,
    Event, EventReader, EventWriter, Events, Or, Position, Query, RemovedComponents, Res, ResMut,
    Resource, Username, With, Without,
};

use std::collections::{HashSet, VecDeque};

use crate::combat::components::{BodyPart, Lifecycle, LifecycleState, Wound, WoundKind, Wounds};
use crate::combat::events::{CombatEvent, DeathEvent};
use crate::combat::CombatClock;
use crate::cultivation::death_hooks::CultivationDeathTrigger;
use crate::cultivation::life_record::{BiographyEntry, HeartDemonOutcome, LifeRecord};
use crate::cultivation::lifespan::{LifespanCapTable, LifespanComponent};
use crate::inventory::{transfer_all_inventory_contents, ItemRegistry, PlayerInventory};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::halfstep_rechallenge_emit::HALFSTEP_QUOTA_RELEASE_BROADCAST_AUDIO_RECIPE;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::network::RedisBridgeResource;
use crate::qi_physics::{
    constants::{DEFAULT_SPIRIT_QI_TOTAL, QI_EPSILON},
    EnvField, QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount, WorldQiBudget,
};
use crate::schema::cultivation::{
    color_kind_to_string, realm_to_string, HeartDemonPregenRequestV1, QiColorStateV1,
};
use crate::schema::server_data::HeartDemonOfferV1;
use crate::schema::tribulation::{
    DuXuOutcomeV1, DuXuResultV1, TribulationEventV1, TribulationPhaseV1,
};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::skill::components::SkillId;
use crate::skill::events::SkillCapChanged;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::karma::KarmaWeightStore;
use crate::world::zone::ZoneRegistry;

use super::breakthrough::skill_cap_for_realm;
use super::components::{
    ActorQiIdentity, ActorQiKind, Cultivation, MeridianId, MeridianSystem, QiColor, Realm,
};
use super::death_hooks::release_qi_amount_to_zone;
use super::meridian::severed::{MeridianSeveredEvent, SeveredSource};
use super::qi_zero_decay::{close_meridian, pick_closures};
use crate::persistence::{
    complete_tribulation_ascension, delete_active_tribulation, load_active_tribulation_count,
    load_ascension_quota, persist_active_tribulation, try_complete_tribulation_ascension,
    ActiveTribulationRecord, AscensionGrant, AtomicAscensionOutcome, PersistenceSettings,
};

pub const DUXU_OMEN_TICKS: u64 = 60 * 20;
pub const DUXU_LOCK_TICKS: u64 = 30 * 20;
pub const DUXU_WAVE_COOLDOWN_TICKS: u64 = 15 * 20;
pub const DUXU_MAX_WAVES: u32 = 5;
const DUXU_FULL_PROGRESS_MIN_TICKS: u64 = 30 * 60 * 20;
pub const TRIBULATION_DANGER_RADIUS: f64 = 100.0;
pub const DUXU_LOCK_RADIUS_SOFT: f64 = 50.0;
pub const DUXU_LOCK_RADIUS_HARD: f64 = 20.0;
pub const DUXU_LOCK_RADIUS_FINAL: f64 = 10.0;
pub const DUXU_BOUNDARY_VFX_EVENT_ID: &str = "bong:tribulation_boundary";
pub const DUXU_OMEN_CLOUD_VFX_EVENT_ID: &str = "bong:tribulation_omen_cloud";
pub const JUEBI_BOUNDARY_VFX_EVENT_ID: &str = "bong:juebi_boundary";
pub const JUEBI_FISSURE_VFX_EVENT_ID: &str = "bong:juebi_fissure";
pub const JUEBI_ERUPTION_VFX_EVENT_ID: &str = "bong:juebi_eruption";

const DUXU_DEFAULT_WAVES: u32 = 3;
// plan-tribulation-balance-v1 P0：pub 化供 TribulationBalanceConfig 镜像（pin 测试防漂移）
pub const DUXU_AOE_DAMAGE_BASE: f32 = 18.0;
pub const DUXU_QI_DRAIN_BASE: f64 = 35.0;
const DUXU_CHAIN_LIGHTNING_WAVE: u32 = 2;
const DUXU_CHAIN_LIGHTNING_STRIKES: u32 = 3;
const DUXU_SOUL_DEVOUR_QI_MAX_FREEZE_RATIO: f64 = 0.20;
pub const DUXU_HEART_DEMON_WAVE: u32 = 4;
pub const DUXU_HEART_DEMON_TIMEOUT_TICKS: u64 = 30 * 20;
// plan-tribulation-balance-v1 P0：pub 化供 TribulationBalanceConfig 镜像（pin 测试防漂移）
pub const DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO: f64 = 0.30;
const DUXU_HEART_DEMON_OBSESSION_NEXT_WAVE_MULTIPLIER: f32 = 1.20;
const DUXU_KAITIAN_WAVE: u32 = 5;
const DUXU_FULL_HEALTH_EPSILON: f32 = 0.001;
const DUXU_FULL_QI_EPSILON: f64 = 0.001;
const DUXU_OMEN_CLOUD_BLOCK_Y_OFFSET: i32 = 24;
const DUXU_OMEN_CLOUD_BLOCK_OFFSETS: [i32; 5] = [-8, -4, 0, 4, 8];
const VOID_QUOTA_K_ENV: &str = "BONG_VOID_QUOTA_K";

pub const JUEBI_OMEN_TICKS: u64 = 10 * 20;
pub const JUEBI_PHASE_TICKS: u64 = 15 * 20;
pub const JUEBI_AFTERSHOCK_TICKS: u64 = 24 * 60 * 60 * 20;
pub const JUEBI_NULL_FIELD_MAX_RADIUS: f64 = 150.0;
pub const JUEBI_ZONE_RADIUS: f64 = 300.0;
pub const JUEBI_CORE_RADIUS: f64 = 50.0;
pub const JUEBI_HEAVY_RADIUS: f64 = 150.0;
pub const JUEBI_PRESSURE_DRAIN_PER_TICK: f64 = 0.02;
pub const JUEBI_NULL_VOID_DECAY_PER_TICK: f64 = 0.03;
pub const JUEBI_NULL_SPIRIT_DECAY_PER_TICK: f64 = 0.01;
pub const JUEBI_INTENSITY_BASE: f32 = 1.5;
pub const JUEBI_WAVES_TOTAL: u32 = 3;
const JUEBI_TERRAIN_BUDGET_PER_TICK: usize = 200;
const JUEBI_FISSURE_COUNT: usize = 8;
const JUEBI_FISSURE_RADIUS: i32 = 80;
const JUEBI_CONE_COUNT: usize = 6;
const JUEBI_CONE_MAX_RADIUS: i32 = 50;
const JUEBI_UPHEAVAL_INNER_RADIUS: i32 = 50;
const JUEBI_UPHEAVAL_OUTER_RADIUS: i32 = 120;
const JUEBI_UPHEAVAL_DENSITY_PER_MILLE: u32 = 350;

/// plan-tribulation-balance-v1 P1：默认满灵气预算下保留 2 个化虚名额。
///
/// 真实名额公式不是 plan 草稿里的 player_count/hard_cap，而是
/// `floor(WorldQiBudget.current_total / quota_k)`。当前运营校准目标是：
/// `DEFAULT_SPIRIT_QI_TOTAL` 满额时 quota_limit=2；若 1 人占用，满载率为 50%，
/// 落在 P1 目标区间 30%-70%。
pub const DEFAULT_VOID_QUOTA_TARGET_SLOTS_AT_FULL_QI: u32 = 2;
pub const DEFAULT_VOID_QUOTA_K: f64 =
    DEFAULT_SPIRIT_QI_TOTAL / DEFAULT_VOID_QUOTA_TARGET_SLOTS_AT_FULL_QI as f64;
pub const VOID_QUOTA_BASIS: &str = "world_qi_budget.current_total";
pub const VOID_QUOTA_EXCEEDED_REASON: &str = "void_quota_exceeded";

// plan-halfstep-buff-v1 P1：HalfStep buff 实装常数（首期值；后续运营数据驱动校准）
pub const HALFSTEP_QI_MAX_BONUS: f32 = 0.10;
pub const HALFSTEP_LIFESPAN_BONUS_YEARS: u32 = 200;
// plan-halfstep-buff-v1 §8 Q1：重渡窗口 7 days in-game = 7 × 24 × 3600 sec × 20 ticks/sec
pub const RECHALLENGE_WINDOW_TICKS: u64 = 7 * 24 * 3600 * 20;

#[derive(Debug, Clone, Copy, PartialEq, Resource)]
pub struct VoidQuotaConfig {
    pub quota_k: f64,
}

impl Default for VoidQuotaConfig {
    fn default() -> Self {
        Self {
            quota_k: DEFAULT_VOID_QUOTA_K,
        }
    }
}

impl VoidQuotaConfig {
    pub fn from_env() -> Self {
        std::env::var(VOID_QUOTA_K_ENV)
            .ok()
            .and_then(|raw| raw.parse::<f64>().ok())
            .filter(|quota_k| quota_k.is_finite() && *quota_k > 0.0)
            .map(|quota_k| Self { quota_k })
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoidQuotaCheck {
    pub occupied_slots: u32,
    pub quota_limit: u32,
    pub available_slots: u32,
    pub total_world_qi: f64,
    pub quota_k: f64,
    pub exceeded: bool,
}

pub fn compute_void_quota_limit(total_world_qi: f64, quota_k: f64) -> u32 {
    if !total_world_qi.is_finite() || !quota_k.is_finite() || quota_k <= 0.0 {
        return 0;
    }
    let slots = (total_world_qi.max(0.0) / quota_k).floor();
    if slots >= u32::MAX as f64 {
        u32::MAX
    } else {
        slots as u32
    }
}

pub fn check_void_quota(
    occupied_slots: u32,
    budget: &WorldQiBudget,
    config: &VoidQuotaConfig,
) -> VoidQuotaCheck {
    let quota_limit = compute_void_quota_limit(budget.current_total, config.quota_k);
    VoidQuotaCheck {
        occupied_slots,
        quota_limit,
        available_slots: quota_limit.saturating_sub(occupied_slots),
        total_world_qi: budget.current_total.max(0.0),
        quota_k: config.quota_k,
        exceeded: occupied_slots >= quota_limit,
    }
}

#[derive(Debug, Clone, Copy)]
struct DuXuWaveProfile {
    strikes: u32,
    damage: f32,
    qi_drain: f64,
    qi_max_freeze_ratio: f64,
    requires_full_resources: bool,
}

#[derive(Debug, Clone, Component)]
pub struct TribulationState {
    pub kind: TribulationKind,
    pub phase: TribulationPhase,
    pub epicenter: [f64; 3],
    pub wave_current: u32,
    pub waves_total: u32,
    pub started_tick: u64,
    pub phase_started_tick: u64,
    pub next_wave_tick: u64,
    pub participants: Vec<String>,
    pub failed: bool,
}

#[derive(Debug, Clone, Copy)]
struct TribulationOmenCloudBlock {
    entity: Entity,
    pos: BlockPos,
    original: BlockState,
    expires_at_tick: u64,
}

#[derive(Debug, Default, Resource)]
pub struct TribulationOmenCloudBlocks {
    blocks: Vec<TribulationOmenCloudBlock>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JueBiTriggerSource {
    VoidQuotaExceeded,
    VoidActionExplodeZone,
    DuguReverse,
    BaomaiDisperse,
    WoliuVortexHeart,
    ZhenfaDeceptionExposed,
    KarmaThreshold,
}

impl JueBiTriggerSource {
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::VoidQuotaExceeded => "void_quota_exceeded",
            Self::VoidActionExplodeZone => "void_action_explode_zone",
            Self::DuguReverse => "dugu_reverse",
            Self::BaomaiDisperse => "baomai_disperse",
            Self::WoliuVortexHeart => "woliu_vortex_heart",
            Self::ZhenfaDeceptionExposed => "zhenfa_deception_exposed",
            Self::KarmaThreshold => "karma_threshold",
        }
    }

    pub fn from_wire_name(value: &str) -> Option<Self> {
        match value {
            "void_quota_exceeded" => Some(Self::VoidQuotaExceeded),
            "void_action_explode_zone" => Some(Self::VoidActionExplodeZone),
            "dugu_reverse" => Some(Self::DuguReverse),
            "baomai_disperse" => Some(Self::BaomaiDisperse),
            "woliu_vortex_heart" => Some(Self::WoliuVortexHeart),
            "zhenfa_deception_exposed" => Some(Self::ZhenfaDeceptionExposed),
            "karma_threshold" => Some(Self::KarmaThreshold),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Event)]
pub struct JueBiTriggerEvent {
    pub entity: Entity,
    pub source: JueBiTriggerSource,
    pub delay_ticks: u64,
    pub triggered_at_tick: u64,
    pub epicenter: Option<[f64; 3]>,
}

#[derive(Debug, Clone)]
struct PendingJueBiTrigger {
    entity: Entity,
    source: JueBiTriggerSource,
    trigger_at_tick: u64,
    epicenter: Option<[f64; 3]>,
}

#[derive(Debug, Default, Resource)]
pub struct PendingJueBiTriggers {
    pending: Vec<PendingJueBiTrigger>,
}

#[derive(Debug, Clone, Event)]
pub struct JueBiTriggeredEvent {
    pub entity: Entity,
    pub char_id: String,
    pub actor_name: String,
    pub source: JueBiTriggerSource,
    pub epicenter: [f64; 3],
    pub dimension: DimensionKind,
    pub waves_total: u32,
    pub started_tick: u64,
    pub intensity: f32,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct JueBiAfterDuXuQuota {
    pub occupied_slots: u32,
    pub quota_limit: u32,
    pub total_world_qi: f64,
    pub quota_k: f64,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct JueBiRuntimeContext {
    pub source: JueBiTriggerSource,
    pub intensity: f32,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct JueBiPressureCollapse {
    pub epicenter: BlockPos,
    pub phase_start_tick: u64,
    pub distance: f64,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct JueBiLawDisruption {
    pub epicenter: BlockPos,
    pub distance: f64,
    pub seed: u64,
}

impl JueBiLawDisruption {
    pub fn intensity(self) -> f64 {
        juebi_near_factor(self.distance)
    }

    pub fn apply_to_env(self, env: EnvField) -> EnvField {
        env.with_law_disruption(self.intensity())
    }

    pub fn env_field(self) -> EnvField {
        self.apply_to_env(EnvField::default())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct JueBiNullField {
    pub epicenter: BlockPos,
    pub dimension: DimensionKind,
    pub current_radius: f64,
    pub expansion_rate: f64,
    pub max_radius: f64,
    pub started_tick: u64,
}

#[derive(Debug, Default, Resource)]
pub struct JueBiNullFields {
    fields: Vec<JueBiNullField>,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct JueBiNullified {
    pub entered_tick: u64,
    pub accumulated_null_time: f64,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct JueBiAftershockDebuff {
    pub until_tick: u64,
    pub rhythm_multiplier: f64,
}

#[derive(Debug, Clone, Copy)]
struct TerrainModOp {
    pos: BlockPos,
    new_state: BlockState,
    anim_order: u32,
    restore_at_tick: u64,
}

#[derive(Debug, Clone, Copy)]
struct JueBiTerrainBlock {
    pos: BlockPos,
    original: BlockState,
    restore_at_tick: u64,
    scar_permanent: bool,
}

#[derive(Debug, Resource)]
pub struct JueBiTerrainOverlay {
    pending: VecDeque<TerrainModOp>,
    placed: Vec<JueBiTerrainBlock>,
    budget_per_tick: usize,
}

impl Default for JueBiTerrainOverlay {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            placed: Vec::new(),
            budget_per_tick: JUEBI_TERRAIN_BUDGET_PER_TICK,
        }
    }
}

#[derive(Debug, Clone)]
struct JueBiZoneAftershock {
    name: String,
    dimension: DimensionKind,
    original_qi: f64,
    started_tick: u64,
    restore_until_tick: u64,
}

#[derive(Debug, Default, Resource)]
pub struct JueBiZoneAftershocks {
    zones: Vec<JueBiZoneAftershock>,
}

fn tribulation_dimension_for_participant(
    current_dimension: Option<&CurrentDimension>,
) -> DimensionKind {
    current_dimension
        .map(|dimension| dimension.0)
        .unwrap_or(DimensionKind::Overworld)
}

#[derive(Debug, Clone, Copy, Component)]
pub struct TribulationOriginDimension(pub DimensionKind);

fn active_tribulation_dimension(
    origin_dimension: Option<&TribulationOriginDimension>,
    current_dimension: Option<&CurrentDimension>,
) -> DimensionKind {
    origin_dimension
        .map(|dimension| dimension.0)
        .unwrap_or_else(|| tribulation_dimension_for_participant(current_dimension))
}

#[derive(Debug, Clone, Component)]
pub struct PendingHeartDemonOffer {
    pub trigger_id: String,
    pub payload: HeartDemonOfferV1,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct HeartDemonResolution {
    pub outcome: HeartDemonOutcome,
    pub choice_idx: Option<u32>,
    pub tick: u64,
    pub next_wave_multiplier: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TribulationKind {
    DuXu,
    ZoneCollapse,
    Targeted,
    JueBi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TribulationPhase {
    Omen,
    Lock,
    Wave(u32),
    HeartDemon,
    Settle,
}

impl TribulationState {
    pub fn restored(wave_current: u32, waves_total: u32, started_tick: u64) -> Self {
        Self::restored_for_kind(
            "du_xu",
            wave_current,
            waves_total,
            started_tick,
            [0.0, 64.0, 0.0],
        )
    }

    pub fn restored_for_kind(
        kind: &str,
        wave_current: u32,
        waves_total: u32,
        started_tick: u64,
        epicenter: [f64; 3],
    ) -> Self {
        let kind = match kind {
            "jue_bi" => TribulationKind::JueBi,
            _ => TribulationKind::DuXu,
        };
        Self {
            kind,
            phase: if kind == TribulationKind::DuXu && wave_current == DUXU_HEART_DEMON_WAVE {
                TribulationPhase::HeartDemon
            } else if kind == TribulationKind::JueBi && wave_current == 0 {
                TribulationPhase::Omen
            } else {
                TribulationPhase::Wave(wave_current.max(1))
            },
            epicenter,
            wave_current,
            waves_total,
            started_tick,
            phase_started_tick: started_tick,
            next_wave_tick: started_tick,
            participants: Vec::new(),
            failed: false,
        }
    }

    pub fn lock_radius(&self, now_tick: u64) -> f64 {
        if self.kind == TribulationKind::JueBi {
            return match self.phase {
                TribulationPhase::Omen => JUEBI_ZONE_RADIUS,
                TribulationPhase::Wave(1) => JUEBI_HEAVY_RADIUS,
                TribulationPhase::Wave(2) | TribulationPhase::Wave(3) => {
                    JUEBI_NULL_FIELD_MAX_RADIUS
                }
                TribulationPhase::Settle => 0.0,
                TribulationPhase::Lock | TribulationPhase::HeartDemon => JUEBI_HEAVY_RADIUS,
                TribulationPhase::Wave(_) => JUEBI_HEAVY_RADIUS,
            };
        }
        match self.phase {
            TribulationPhase::Omen => {
                if now_tick.saturating_sub(self.started_tick) >= DUXU_OMEN_TICKS / 2 {
                    DUXU_LOCK_RADIUS_SOFT
                } else {
                    TRIBULATION_DANGER_RADIUS
                }
            }
            TribulationPhase::Lock => DUXU_LOCK_RADIUS_HARD,
            TribulationPhase::Wave(_) | TribulationPhase::HeartDemon => DUXU_LOCK_RADIUS_FINAL,
            TribulationPhase::Settle => 0.0,
        }
    }

    fn is_primary_tribulator(&self, character_id: &str) -> bool {
        self.participants
            .first()
            .is_some_and(|participant| participant == character_id)
    }

    fn record_interceptor(&mut self, character_id: &str) -> bool {
        if self
            .participants
            .iter()
            .any(|participant| participant == character_id)
        {
            return false;
        }
        self.participants.push(character_id.to_string());
        true
    }

    fn ensure_primary_tribulator(&mut self, character_id: &str) {
        if self.participants.is_empty() {
            self.participants.push(character_id.to_string());
        }
    }
}

fn active_record_for_state(
    char_id: &str,
    state: &TribulationState,
    runtime: Option<&JueBiRuntimeContext>,
    origin_dimension: Option<DimensionKind>,
) -> ActiveTribulationRecord {
    ActiveTribulationRecord {
        char_id: char_id.to_string(),
        kind: tribulation_kind_record_label(state.kind).to_string(),
        source: runtime
            .map(|runtime| runtime.source.wire_name().to_string())
            .unwrap_or_default(),
        origin_dimension: origin_dimension.map(|dimension| dimension.ident_str().to_string()),
        wave_current: state.wave_current,
        waves_total: state.waves_total,
        started_tick: state.started_tick,
        epicenter: state.epicenter,
        intensity: runtime.map(|runtime| runtime.intensity).unwrap_or(0.0),
    }
}

fn tribulation_kind_record_label(kind: TribulationKind) -> &'static str {
    match kind {
        TribulationKind::JueBi => "jue_bi",
        _ => "du_xu",
    }
}

fn persist_active_state(
    settings: &PersistenceSettings,
    lifecycle: &Lifecycle,
    state: &TribulationState,
    runtime: Option<&JueBiRuntimeContext>,
    origin_dimension: Option<DimensionKind>,
) -> std::io::Result<()> {
    persist_active_tribulation(
        settings,
        &active_record_for_state(
            lifecycle.character_id.as_str(),
            state,
            runtime,
            origin_dimension,
        ),
    )
}

fn juebi_intensity_scale(intensity: f32) -> f32 {
    (intensity / JUEBI_INTENSITY_BASE).clamp(0.5, 2.0)
}

#[derive(Debug, Clone, Event)]
pub struct InitiateXuhuaTribulation {
    pub entity: Entity,
    pub waves_total: u32,
    pub started_tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct StartDuXuRequest {
    pub entity: Entity,
    pub requested_at_tick: u64,
}

/// plan-halfstep-buff-v1 P0/P1/P3：HalfStep 修士状态
///
/// `entered_at` 由 P0 metrics 系统在 settlement 时填入；
/// `rechallenge_window_until` = `entered_at + RECHALLENGE_WINDOW_TICKS`（§8 Q1）；
/// `buff_applied` 由 P1 守卫 — 防止多次 HalfStep 叠加 buff（§8 Q4）。
#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct HalfStepState {
    pub entered_at: u64,
    pub rechallenge_window_until: u64,
    pub buff_applied: bool,
}

impl HalfStepState {
    pub fn new(entered_at: u64) -> Self {
        Self {
            entered_at,
            rechallenge_window_until: entered_at.saturating_add(RECHALLENGE_WINDOW_TICKS),
            buff_applied: false,
        }
    }

    pub fn is_within_window(&self, current_tick: u64) -> bool {
        current_tick <= self.rechallenge_window_until
    }
}

/// plan-halfstep-buff-v1 P0：渡虚劫遥测计数（结算次数 + quota 满时长）
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TribulationMetrics {
    pub halfstep_count: u64,
    pub ascended_count: u64,
    pub quota_full_duration_ticks: u64,
}

/// plan-halfstep-buff-v1 P0：quota 满时长事件驱动追踪器
///
/// 由 `AscensionQuotaOpened` / `AscensionQuotaOccupied` 事件驱动，状态变化时计算当前
/// occupied / limit；当 `current_occupied >= current_limit > 0` 时标记 `full_since_tick`，
/// 离开 full 状态时把累计 ticks 写入 `TribulationMetrics.quota_full_duration_ticks`。
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct QuotaFullTracker {
    pub current_occupied: u32,
    pub current_limit: u32,
    pub full_since_tick: Option<u64>,
}

/// plan-halfstep-buff-v1 P3：单个 HalfStep 修士在重渡队列中的条目。
///
/// `entity` 是 ECS entity 句柄（hydrated 玩家 / hydrated NPC）；`is_dormant` 标记表示
/// dormant NPC 占位（hydrate-on-trigger 由 dispatch system 处理，参 plan-npc-virtualize-v1）。
/// `char_id` 为持久化键（dormant NPC 没有 entity，靠 char_id 唯一标识）。
/// `buff_applied` 镜像 `HalfStepState.buff_applied`（P5 review #1 fix）—— dormant→hydrate
/// 换 entity 时用 `find_by_char_id` 复用 entered_at/window，**也必须**复用 buff 标志，否则
/// 已 buffed 的同角色再次结算会重复加 buff（破 §8 Q4）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HalfStepRechallengeEntry {
    pub char_id: String,
    pub entity: Entity,
    pub entered_at: u64,
    pub rechallenge_window_until: u64,
    pub is_dormant: bool,
    pub buff_applied: bool,
}

/// plan-halfstep-buff-v1 P3：FIFO 重渡队列（§8 Q2 先到先得 + Q5 NPC 同池）。
///
/// 按 `entered_at` 升序保有所有当前 HalfStep 修士（玩家 + dormant NPC 同池）；
/// `AscensionQuotaOpened` 事件触发时 dispatch system 从头取一名通知重渡。
/// 队列头部过窗（§8 Q1）的条目会被 dispatch system 直接出队，继续看下一个。
#[derive(Resource, Debug, Default, Clone)]
pub struct HalfStepRechallengeQueue {
    pub queue: VecDeque<HalfStepRechallengeEntry>,
}

impl HalfStepRechallengeQueue {
    /// 按 `entered_at` 升序插入。FIFO 顺序：早进 HalfStep 的先得通知。
    pub fn enqueue(&mut self, entry: HalfStepRechallengeEntry) {
        // 找到第一个 entered_at > 我的位置插入；若都 ≤ 我，则 push_back
        let insert_at = self
            .queue
            .iter()
            .position(|e| e.entered_at > entry.entered_at)
            .unwrap_or(self.queue.len());
        self.queue.insert(insert_at, entry);
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// 清理一个 entity 的所有 entry（用于 Ascended / 老死 / 降境时手动出队）。
    pub fn remove_entity(&mut self, target: Entity) {
        self.queue.retain(|e| e.entity != target);
    }

    /// 清理一个 char_id 的所有 entry（dormant NPC 用，没有 entity 句柄）。
    pub fn remove_char_id(&mut self, target: &str) {
        self.queue.retain(|e| e.char_id != target);
    }

    /// plan-halfstep-buff-v1 P4 review #2：按 `char_id` 找队列项（dormant→hydrate
    /// 换 entity 时复用旧 `entered_at` / `rechallenge_window_until`，防 §8 Q1 7d 窗口
    /// 被刷新 + §8 Q2 FCFS 顺序乱）。返回第一个匹配项的拷贝，调用方负责后续 dedup。
    pub fn find_by_char_id(&self, target: &str) -> Option<HalfStepRechallengeEntry> {
        self.queue.iter().find(|e| e.char_id == target).cloned()
    }
}

/// plan-halfstep-buff-v1 P3：重渡触发事件。dispatch system 取队列头有效 entry 时 emit；
/// HUD（玩家）/ dormant hydrate（NPC）订阅响应。
#[derive(Debug, Clone, Event, PartialEq, Eq)]
pub struct HalfStepRechallengeTriggerEvent {
    pub char_id: String,
    pub entity: Entity,
    pub is_dormant: bool,
    pub at_tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct TribulationAnnounce {
    pub entity: Entity,
    pub char_id: String,
    pub actor_name: String,
    pub epicenter: [f64; 3],
    pub waves_total: u32,
    pub started_tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct TribulationLocked {
    pub entity: Entity,
    pub char_id: String,
    pub actor_name: String,
    pub epicenter: [f64; 3],
    pub waves_total: u32,
}

#[derive(Debug, Clone, Event)]
pub struct TribulationSettled {
    pub entity: Entity,
    pub kind: TribulationKind,
    pub source: Option<JueBiTriggerSource>,
    pub result: DuXuResultV1,
}

#[derive(Debug, Clone, Event)]
pub struct AscensionQuotaOpened {
    pub occupied_slots: u32,
}

#[derive(Debug, Clone, Event)]
pub struct AscensionQuotaOccupied {
    pub occupied_slots: u32,
}

/// 单波次通过（由战斗 plan 发送）。
#[derive(Debug, Clone, Event)]
pub struct TribulationWaveCleared {
    pub entity: Entity,
    pub wave: u32,
}

/// 渡劫失败（战斗 plan 在天劫波次失败时发送；不进入死亡生命周期）。
#[derive(Debug, Clone, Event)]
pub struct TribulationFailed {
    pub entity: Entity,
    pub wave: u32,
}

#[derive(Debug, Clone, Event)]
pub struct TribulationFled {
    pub entity: Entity,
    pub tick: u64,
}

#[derive(Debug, Clone, Copy, Event)]
pub struct HeartDemonChoiceSubmitted {
    pub entity: Entity,
    pub choice_idx: Option<u32>,
    pub submitted_at_tick: u64,
}

#[derive(Debug, Clone, Copy)]
struct HeartDemonDecision {
    entity: Entity,
    choice_idx: Option<u32>,
    tick: u64,
}

#[allow(clippy::type_complexity)]
pub fn start_du_xu_request_system(
    mut requests: EventReader<StartDuXuRequest>,
    mut initiate: EventWriter<InitiateXuhuaTribulation>,
    players: Query<(
        &Cultivation,
        &MeridianSystem,
        Option<&TribulationState>,
        Option<&LifeRecord>,
    )>,
) {
    let mut accepted_this_tick = HashSet::new();
    for request in requests.read() {
        let Ok((cultivation, meridians, active, life_record)) = players.get(request.entity) else {
            continue;
        };
        if active.is_some()
            || accepted_this_tick.contains(&request.entity)
            || !du_xu_prereqs_met(cultivation, meridians)
        {
            tracing::warn!(
                "[bong][cultivation] start_du_xu rejected entity={:?} realm={:?} opened_meridians={}",
                request.entity,
                cultivation.realm,
                meridians.opened_count(),
            );
            continue;
        }
        initiate.send(InitiateXuhuaTribulation {
            entity: request.entity,
            waves_total: du_xu_waves_total(request.requested_at_tick, life_record),
            started_tick: request.requested_at_tick,
        });
        accepted_this_tick.insert(request.entity);
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn start_tribulation_system(
    settings: Res<PersistenceSettings>,
    budget: Res<WorldQiBudget>,
    void_quota: Res<VoidQuotaConfig>,
    mut events: EventReader<InitiateXuhuaTribulation>,
    mut announce: EventWriter<TribulationAnnounce>,
    mut _settled: EventWriter<TribulationSettled>,
    mut _death_triggers: EventWriter<CultivationDeathTrigger>,
    mut players: Query<(
        &Cultivation,
        &MeridianSystem,
        &Lifecycle,
        Option<&Username>,
        Option<&TribulationState>,
        Option<&CurrentDimension>,
    )>,
    mut commands: Commands,
    positions: Query<&Position>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    let mut accepted_this_tick = HashSet::new();
    let active_quota_slots = match load_active_tribulation_count(&settings) {
        Ok(count) => count,
        Err(error) => {
            tracing::error!(
                "[bong][cultivation] failed to load active tribulation count before start: {error}"
            );
            return;
        }
    };
    let mut reserved_occupied_slots = None;
    for ev in events.read() {
        if let Ok((c, meridians, lifecycle, username, active, current_dimension)) =
            players.get_mut(ev.entity)
        {
            if active.is_some() || accepted_this_tick.contains(&ev.entity) {
                tracing::warn!(
                    "[bong][cultivation] duplicate active tribulation start for {:?}, rejected",
                    ev.entity,
                );
                continue;
            }
            if c.realm != Realm::Spirit {
                tracing::warn!(
                    "[bong][cultivation] {:?} tried to tribulate from {:?}, rejected",
                    ev.entity,
                    c.realm
                );
                continue;
            }
            if !du_xu_prereqs_met(c, meridians) {
                tracing::warn!(
                    "[bong][cultivation] {:?} tried to tribulate without all meridians open",
                    ev.entity,
                );
                continue;
            }
            let p = positions
                .get(ev.entity)
                .map(|pos| pos.get())
                .unwrap_or(valence::math::DVec3::new(0.0, 64.0, 0.0));
            let occupied_slots = match reserved_occupied_slots {
                Some(slots) => slots,
                None => {
                    let persisted_occupied = match load_ascension_quota(&settings) {
                        Ok(quota) => quota.occupied_slots,
                        Err(error) => {
                            tracing::error!(
                                "[bong][cultivation] failed to load ascension quota before tribulation start for {:?}: {error}",
                                ev.entity,
                            );
                            continue;
                        }
                    };
                    let slots = persisted_occupied.saturating_add(active_quota_slots);
                    reserved_occupied_slots = Some(slots);
                    slots
                }
            };
            let quota_check = check_void_quota(occupied_slots, &budget, &void_quota);
            let juebi_after_quota = if quota_check.exceeded {
                tracing::info!(
                    "[bong][cultivation] {:?} void-quota exceeded; DuXu may continue but settlement will trigger JueBi (quota {}/{}, total_world_qi={}, quota_k={})",
                    ev.entity,
                    quota_check.occupied_slots,
                    quota_check.quota_limit,
                    quota_check.total_world_qi,
                    quota_check.quota_k,
                );
                Some(JueBiAfterDuXuQuota {
                    occupied_slots: quota_check.occupied_slots,
                    quota_limit: quota_check.quota_limit,
                    total_world_qi: quota_check.total_world_qi,
                    quota_k: quota_check.quota_k,
                })
            } else {
                None
            };
            let origin_dimension = tribulation_dimension_for_participant(current_dimension);
            let state = TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Omen,
                epicenter: [p.x, p.y, p.z],
                wave_current: 0,
                waves_total: ev.waves_total.clamp(1, DUXU_MAX_WAVES),
                started_tick: ev.started_tick,
                phase_started_tick: ev.started_tick,
                next_wave_tick: ev
                    .started_tick
                    .saturating_add(DUXU_OMEN_TICKS + DUXU_LOCK_TICKS),
                participants: vec![lifecycle.character_id.clone()],
                failed: false,
            };
            if let Err(error) =
                persist_active_state(&settings, lifecycle, &state, None, Some(origin_dimension))
            {
                tracing::warn!(
                    "[bong][cultivation] failed to persist active tribulation for {:?}: {error}",
                    ev.entity,
                );
                continue;
            }
            reserved_occupied_slots = Some(occupied_slots.saturating_add(1));
            let mut entity_commands = commands.entity(ev.entity);
            entity_commands.insert((state, TribulationOriginDimension(origin_dimension)));
            if let Some(marker) = juebi_after_quota {
                entity_commands.insert(marker);
            }
            announce.send(TribulationAnnounce {
                entity: ev.entity,
                char_id: lifecycle.character_id.clone(),
                actor_name: username
                    .map(|name| name.0.clone())
                    .unwrap_or_else(|| lifecycle.character_id.clone()),
                epicenter: [p.x, p.y, p.z],
                waves_total: ev.waves_total.clamp(1, DUXU_MAX_WAVES),
                started_tick: ev.started_tick,
            });
            tracing::info!(
                "[bong][cultivation] {:?} initiated tribulation ({} waves, quota {}/{}, total_world_qi={}, quota_k={})",
                ev.entity,
                ev.waves_total,
                quota_check.occupied_slots,
                quota_check.quota_limit,
                quota_check.total_world_qi,
                quota_check.quota_k,
            );
            // plan-particle-system-v1 §4.4：渡劫开场一道预警雷。
            vfx_events.send(VfxEventRequest::new(
                p,
                VfxEventPayloadV1::SpawnParticle {
                    event_id: "bong:tribulation_lightning".to_string(),
                    origin: [p.x, p.y, p.z],
                    direction: None,
                    color: Some("#D0C8FF".to_string()),
                    strength: Some(1.0),
                    count: Some(3),
                    duration_ticks: Some(14),
                },
            ));
            accepted_this_tick.insert(ev.entity);
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn tribulation_phase_tick_system(
    clock: Res<CombatClock>,
    mut query: Query<(
        Entity,
        &mut TribulationState,
        Option<&HeartDemonResolution>,
        Option<&PendingHeartDemonOffer>,
        Option<&Lifecycle>,
        Option<&Username>,
    )>,
    mut locked: EventWriter<TribulationLocked>,
    mut cleared: EventWriter<TribulationWaveCleared>,
) {
    for (entity, mut state, heart_demon, pregen, lifecycle, username) in &mut query {
        if state.kind == TribulationKind::JueBi {
            tick_juebi_phase(&mut state, entity, clock.tick, &mut cleared);
            continue;
        }
        match state.phase {
            TribulationPhase::Omen
                if clock.tick.saturating_sub(state.phase_started_tick) >= DUXU_OMEN_TICKS =>
            {
                let char_id = lifecycle
                    .map(|lifecycle| lifecycle.character_id.clone())
                    .or_else(|| state.participants.first().cloned())
                    .unwrap_or_else(|| format!("entity:{entity:?}"));
                let actor_name = username
                    .map(|name| name.0.clone())
                    .unwrap_or_else(|| char_id.clone());
                state.phase = TribulationPhase::Lock;
                state.phase_started_tick = clock.tick;
                locked.send(TribulationLocked {
                    entity,
                    char_id,
                    actor_name,
                    epicenter: state.epicenter,
                    waves_total: state.waves_total,
                });
            }
            TribulationPhase::Lock
                if clock.tick.saturating_sub(state.phase_started_tick) >= DUXU_LOCK_TICKS =>
            {
                let next_wave = state.wave_current.saturating_add(1);
                begin_tribulation_wave(&mut state, entity, next_wave, clock.tick, &mut cleared);
            }
            TribulationPhase::Wave(_) if clock.tick >= state.next_wave_tick && !state.failed => {
                let next_wave = next_tribulation_wave(&state, heart_demon.is_some());
                if should_enter_heart_demon_phase(entity, &state, heart_demon, pregen, next_wave) {
                    let event_wave = if next_wave == DUXU_HEART_DEMON_WAVE {
                        DUXU_HEART_DEMON_WAVE
                    } else {
                        state.wave_current
                    };
                    begin_heart_demon_phase(
                        &mut state,
                        entity,
                        event_wave,
                        clock.tick,
                        &mut cleared,
                    );
                } else {
                    begin_tribulation_wave(&mut state, entity, next_wave, clock.tick, &mut cleared);
                }
            }
            TribulationPhase::HeartDemon if heart_demon.is_some() => {
                let next_wave = next_tribulation_wave(&state, true);
                begin_tribulation_wave(&mut state, entity, next_wave, clock.tick, &mut cleared);
            }
            _ => {}
        }
    }
}

fn tick_juebi_phase(
    state: &mut TribulationState,
    entity: Entity,
    tick: u64,
    cleared: &mut EventWriter<TribulationWaveCleared>,
) {
    match state.phase {
        TribulationPhase::Omen
            if tick.saturating_sub(state.phase_started_tick) >= JUEBI_OMEN_TICKS =>
        {
            begin_juebi_phase(state, entity, 1, tick, cleared);
        }
        TribulationPhase::Wave(wave)
            if wave < state.waves_total
                && tick.saturating_sub(state.phase_started_tick) >= JUEBI_PHASE_TICKS =>
        {
            begin_juebi_phase(state, entity, wave.saturating_add(1), tick, cleared);
        }
        TribulationPhase::Wave(wave)
            if wave >= state.waves_total
                && tick.saturating_sub(state.phase_started_tick) >= JUEBI_PHASE_TICKS =>
        {
            state.phase = TribulationPhase::Settle;
            state.phase_started_tick = tick;
            state.next_wave_tick = tick;
        }
        _ => {}
    }
}

fn begin_juebi_phase(
    state: &mut TribulationState,
    entity: Entity,
    wave: u32,
    tick: u64,
    cleared: &mut EventWriter<TribulationWaveCleared>,
) {
    state.phase = TribulationPhase::Wave(wave);
    state.wave_current = state.wave_current.max(wave.saturating_sub(1));
    state.phase_started_tick = tick;
    state.next_wave_tick = tick.saturating_add(JUEBI_PHASE_TICKS);
    cleared.send(TribulationWaveCleared { entity, wave });
}

pub fn schedule_juebi_triggers_system(
    mut events: EventReader<JueBiTriggerEvent>,
    mut pending: ResMut<PendingJueBiTriggers>,
) {
    for event in events.read() {
        pending.pending.push(PendingJueBiTrigger {
            entity: event.entity,
            source: event.source,
            trigger_at_tick: event.triggered_at_tick.saturating_add(event.delay_ticks),
            epicenter: event.epicenter,
        });
    }
}

#[allow(clippy::type_complexity)]
pub fn start_due_juebi_triggers_system(
    settings: Option<Res<PersistenceSettings>>,
    clock: Res<CombatClock>,
    mut pending: ResMut<PendingJueBiTriggers>,
    karma: Option<Res<KarmaWeightStore>>,
    mut commands: Commands,
    actors: Query<(
        &Lifecycle,
        Option<&Username>,
        Option<&Position>,
        Option<&CurrentDimension>,
        Option<&TribulationState>,
    )>,
    mut triggered: EventWriter<JueBiTriggeredEvent>,
) {
    let mut waiting = Vec::with_capacity(pending.pending.len());
    for item in pending.pending.drain(..) {
        if item.trigger_at_tick > clock.tick {
            waiting.push(item);
            continue;
        }
        let Ok((lifecycle, username, position, current_dimension, active)) =
            actors.get(item.entity)
        else {
            continue;
        };
        if active.is_some() {
            tracing::warn!(
                "[bong][cultivation] JueBi trigger ignored for {:?}; active tribulation already exists",
                item.entity,
            );
            continue;
        }
        let p = item.epicenter.unwrap_or_else(|| {
            position
                .map(|position| {
                    let p = position.get();
                    [p.x, p.y, p.z]
                })
                .unwrap_or([0.0, 64.0, 0.0])
        });
        let dimension = tribulation_dimension_for_participant(current_dimension);
        let intensity = juebi_intensity_for_source(item.source, lifecycle, karma.as_deref());
        let state = juebi_state(p, clock.tick, lifecycle.character_id.clone());
        let runtime = JueBiRuntimeContext {
            source: item.source,
            intensity,
        };
        if let Some(settings) = settings.as_deref() {
            if let Err(error) =
                persist_active_state(settings, lifecycle, &state, Some(&runtime), Some(dimension))
            {
                tracing::warn!(
                    "[bong][cultivation] failed to persist JueBi trigger for {:?}: {error}",
                    item.entity,
                );
                continue;
            }
        }
        commands.entity(item.entity).insert((
            state,
            TribulationOriginDimension(dimension),
            runtime,
        ));
        let actor_name = username
            .map(|username| username.0.clone())
            .unwrap_or_else(|| lifecycle.character_id.clone());
        triggered.send(JueBiTriggeredEvent {
            entity: item.entity,
            char_id: lifecycle.character_id.clone(),
            actor_name,
            source: item.source,
            epicenter: p,
            dimension,
            waves_total: JUEBI_WAVES_TOTAL,
            started_tick: clock.tick,
            intensity,
        });
        tracing::info!(
            "[bong][cultivation] {:?} started JueBi from {} intensity={}",
            item.entity,
            item.source.wire_name(),
            intensity,
        );
    }
    pending.pending = waiting;
}

fn juebi_state(
    epicenter: [f64; 3],
    started_tick: u64,
    primary_participant: String,
) -> TribulationState {
    TribulationState {
        kind: TribulationKind::JueBi,
        phase: TribulationPhase::Omen,
        epicenter,
        wave_current: 0,
        waves_total: JUEBI_WAVES_TOTAL,
        started_tick,
        phase_started_tick: started_tick,
        next_wave_tick: started_tick.saturating_add(JUEBI_OMEN_TICKS),
        participants: vec![primary_participant],
        failed: false,
    }
}

fn juebi_intensity_for_source(
    source: JueBiTriggerSource,
    lifecycle: &Lifecycle,
    karma: Option<&KarmaWeightStore>,
) -> f32 {
    let source_bonus = match source {
        JueBiTriggerSource::VoidQuotaExceeded => 0.15,
        JueBiTriggerSource::VoidActionExplodeZone => 0.10,
        JueBiTriggerSource::WoliuVortexHeart => 0.10,
        JueBiTriggerSource::ZhenfaDeceptionExposed => 0.25,
        JueBiTriggerSource::DuguReverse
        | JueBiTriggerSource::BaomaiDisperse
        | JueBiTriggerSource::KarmaThreshold => 0.0,
    };
    let karma_bonus = karma
        .map(|karma| karma.weight_for_player(&lifecycle.character_id) * 0.35)
        .unwrap_or(0.0);
    (JUEBI_INTENSITY_BASE + source_bonus + karma_bonus).clamp(JUEBI_INTENSITY_BASE, 2.0)
}

fn juebi_intensity_for_quota_marker(marker: &JueBiAfterDuXuQuota) -> f32 {
    let pressure = if marker.quota_limit == 0 {
        0.35
    } else {
        marker.occupied_slots.saturating_sub(marker.quota_limit) as f32 * 0.10
    };
    (JUEBI_INTENSITY_BASE + 0.15 + pressure).clamp(JUEBI_INTENSITY_BASE, 2.0)
}

fn next_tribulation_wave(state: &TribulationState, heart_demon_resolved: bool) -> u32 {
    let next_wave = state.wave_current.saturating_add(1);
    if heart_demon_resolved
        && state.waves_total >= DUXU_KAITIAN_WAVE
        && next_wave == DUXU_HEART_DEMON_WAVE
    {
        DUXU_KAITIAN_WAVE
    } else {
        next_wave
    }
}

fn should_enter_heart_demon_phase(
    entity: Entity,
    state: &TribulationState,
    heart_demon: Option<&HeartDemonResolution>,
    pregen: Option<&PendingHeartDemonOffer>,
    next_wave: u32,
) -> bool {
    if heart_demon.is_some() || state.waves_total < DUXU_HEART_DEMON_WAVE {
        return false;
    }
    if next_wave == DUXU_HEART_DEMON_WAVE {
        return true;
    }
    state.wave_current >= DUXU_CHAIN_LIGHTNING_WAVE
        && pending_heart_demon_offer_matches(entity, state, pregen)
}

fn pending_heart_demon_offer_matches(
    entity: Entity,
    state: &TribulationState,
    pregen: Option<&PendingHeartDemonOffer>,
) -> bool {
    pregen.is_some_and(|offer| {
        offer.trigger_id == heart_demon_trigger_id(entity.index(), state.started_tick)
    })
}

fn begin_tribulation_wave(
    state: &mut TribulationState,
    entity: Entity,
    wave: u32,
    tick: u64,
    cleared: &mut EventWriter<TribulationWaveCleared>,
) {
    if wave == 0 || wave > state.waves_total {
        return;
    }
    state.phase = TribulationPhase::Wave(wave);
    state.phase_started_tick = tick;
    state.next_wave_tick = tick.saturating_add(DUXU_WAVE_COOLDOWN_TICKS);
    cleared.send(TribulationWaveCleared { entity, wave });
}

fn begin_heart_demon_phase(
    state: &mut TribulationState,
    entity: Entity,
    event_wave: u32,
    tick: u64,
    cleared: &mut EventWriter<TribulationWaveCleared>,
) {
    state.phase = TribulationPhase::HeartDemon;
    state.phase_started_tick = tick;
    state.next_wave_tick = tick.saturating_add(DUXU_WAVE_COOLDOWN_TICKS);
    cleared.send(TribulationWaveCleared {
        entity,
        wave: event_wave,
    });
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn tribulation_aoe_system(
    clock: Res<CombatClock>,
    tribulations: Query<(
        Entity,
        &TribulationState,
        Option<&HeartDemonResolution>,
        Option<&CurrentDimension>,
        Option<&TribulationOriginDimension>,
    )>,
    mut targets: Query<(
        Entity,
        &Position,
        Option<&CurrentDimension>,
        &mut Cultivation,
        &mut Wounds,
        Option<&Lifecycle>,
        Option<&LifeRecord>,
    )>,
    mut failed: EventWriter<TribulationFailed>,
    mut deaths: EventWriter<DeathEvent>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut ledger: ResMut<WorldQiAccount>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
) {
    for (tribulator_entity, state, heart_demon, tribulator_dimension, origin_dimension) in
        &tribulations
    {
        let TribulationPhase::Wave(wave) = state.phase else {
            continue;
        };
        if clock.tick != state.phase_started_tick {
            continue;
        }
        let tribulation_dimension =
            active_tribulation_dimension(origin_dimension, tribulator_dimension);
        if tribulation_dimension_for_participant(tribulator_dimension) != tribulation_dimension {
            continue;
        }
        let center =
            valence::math::DVec3::new(state.epicenter[0], state.epicenter[1], state.epicenter[2]);
        let profile = du_xu_wave_profile(wave);
        let damage_multiplier = heart_demon
            .filter(|_| wave == DUXU_KAITIAN_WAVE)
            .map(|heart_demon| heart_demon.next_wave_multiplier)
            .unwrap_or(1.0);
        let strike_damage = profile.damage / profile.strikes.max(1) as f32;
        for (entity, pos, current_dimension, mut cultivation, mut wounds, lifecycle, life_record) in
            &mut targets
        {
            if tribulation_dimension_for_participant(current_dimension) != tribulation_dimension {
                continue;
            }
            if pos.get().distance(center) > TRIBULATION_DANGER_RADIUS {
                continue;
            }
            let is_tribulator = entity == tribulator_entity
                || lifecycle
                    .map(|lifecycle| state.is_primary_tribulator(&lifecycle.character_id))
                    .unwrap_or(false);
            if profile.requires_full_resources
                && is_tribulator
                && !has_full_tribulation_resources(&cultivation, &wounds)
            {
                failed.send(TribulationFailed { entity, wave });
                continue;
            }
            let actual_drain = profile.qi_drain.min(cultivation.qi_current.max(0.0));
            let release = release_qi_amount_to_zone(
                &mut cultivation,
                actual_drain,
                Some(pos),
                current_dimension,
                life_record,
                zones.as_deref_mut(),
                &mut ledger,
                qi_transfers.as_deref_mut(),
                "tribulation_wave_aoe",
            );
            if let Err(error) = release {
                tracing::warn!(
                    ?error,
                    "[bong][cultivation] tribulation wave qi drain failed closed"
                );
                continue;
            }
            if profile.qi_max_freeze_ratio > 0.0 {
                let frozen = cultivation.qi_max_frozen.unwrap_or(0.0);
                cultivation.qi_max_frozen = Some(
                    (frozen + cultivation.qi_max * profile.qi_max_freeze_ratio)
                        .min(cultivation.qi_max),
                );
            }
            let was_alive = wounds.health_current > 0.0;
            let damage = profile.damage * damage_multiplier;
            wounds.health_current = (wounds.health_current - damage).clamp(0.0, wounds.health_max);
            for _ in 0..profile.strikes {
                wounds.entries.push(Wound {
                    // humanoid-only boundary（P0 决议，本轮不迁移）：渡劫雷击是无差别范围
                    // 伤害，固定命中 Chest 代表部位；玩家恒为人形。
                    location: crate::body_plan::legacy_body_part_to_id(BodyPart::Chest),
                    kind: WoundKind::Burn,
                    severity: strike_damage * damage_multiplier,
                    bleeding_per_sec: 0.0,
                    created_at_tick: clock.tick,
                    inflicted_by: Some("du_xu_tribulation".to_string()),
                });
            }
            if !was_alive || wounds.health_current > 0.0 {
                continue;
            }
            if is_tribulator {
                failed.send(TribulationFailed { entity, wave });
            } else {
                deaths.send(DeathEvent {
                    target: entity,
                    cause: "观劫而亡".to_string(),
                    attacker: None,
                    attacker_player_id: None,
                    at_tick: clock.tick,
                });
            }
        }
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn juebi_phase_effect_system(
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut null_fields: ResMut<JueBiNullFields>,
    tribulations: Query<(
        &TribulationState,
        Option<&JueBiRuntimeContext>,
        Option<&CurrentDimension>,
        Option<&TribulationOriginDimension>,
    )>,
    mut targets: Query<(
        Entity,
        &Position,
        Option<&CurrentDimension>,
        &mut Cultivation,
        Option<&mut Wounds>,
        Option<&Lifecycle>,
        Option<&LifeRecord>,
    )>,
    marked_targets: Query<
        Entity,
        Or<(
            With<JueBiPressureCollapse>,
            With<JueBiLawDisruption>,
            With<JueBiNullified>,
        )>,
    >,
    mut deaths: EventWriter<DeathEvent>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut ledger: ResMut<WorldQiAccount>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
) {
    null_fields.fields.clear();
    for entity in &marked_targets {
        commands
            .entity(entity)
            .remove::<(JueBiPressureCollapse, JueBiLawDisruption, JueBiNullified)>();
    }

    for (state, runtime, current_dimension, origin_dimension) in &tribulations {
        if state.kind != TribulationKind::JueBi {
            continue;
        }
        let TribulationPhase::Wave(wave) = state.phase else {
            continue;
        };
        let intensity = runtime
            .map(|runtime| runtime.intensity)
            .unwrap_or(JUEBI_INTENSITY_BASE);
        let intensity_scale = f64::from(juebi_intensity_scale(intensity));
        let dimension = active_tribulation_dimension(origin_dimension, current_dimension);
        let epicenter_vec =
            valence::math::DVec3::new(state.epicenter[0], state.epicenter[1], state.epicenter[2]);
        let epicenter_block = block_pos_from_epicenter(state.epicenter);
        if wave == 3 {
            let elapsed = clock.tick.saturating_sub(state.phase_started_tick);
            null_fields.fields.push(JueBiNullField {
                epicenter: epicenter_block,
                dimension,
                current_radius: juebi_null_radius(elapsed),
                expansion_rate: JUEBI_NULL_FIELD_MAX_RADIUS / JUEBI_PHASE_TICKS as f64,
                max_radius: JUEBI_NULL_FIELD_MAX_RADIUS,
                started_tick: state.phase_started_tick,
            });
        }

        for (entity, position, target_dimension, mut cultivation, wounds, lifecycle, life_record) in
            &mut targets
        {
            if tribulation_dimension_for_participant(target_dimension) != dimension {
                continue;
            }
            let distance = position.get().distance(epicenter_vec);
            if distance > JUEBI_ZONE_RADIUS {
                continue;
            }
            if clock.tick == state.phase_started_tick {
                apply_juebi_phase_damage(
                    entity,
                    wave,
                    distance,
                    &cultivation,
                    wounds,
                    lifecycle,
                    state,
                    intensity,
                    clock.tick,
                    &mut deaths,
                );
            }
            match wave {
                1 => {
                    let factor = juebi_near_factor(distance);
                    if factor <= 0.0 {
                        continue;
                    }
                    let before = cultivation.qi_current;
                    let actual_drain =
                        before * JUEBI_PRESSURE_DRAIN_PER_TICK * intensity_scale * factor;
                    let release = release_qi_amount_to_zone(
                        &mut cultivation,
                        actual_drain.min(before),
                        Some(position),
                        target_dimension,
                        life_record,
                        zones.as_deref_mut(),
                        &mut ledger,
                        qi_transfers.as_deref_mut(),
                        "juebi_pressure_collapse",
                    );
                    if let Err(error) = release {
                        tracing::warn!(
                            ?error,
                            "[bong][cultivation] juebi pressure qi drain failed closed"
                        );
                        continue;
                    }
                    commands.entity(entity).insert(JueBiPressureCollapse {
                        epicenter: epicenter_block,
                        phase_start_tick: state.phase_started_tick,
                        distance,
                    });
                    if before > 0.0 && cultivation.qi_current <= f64::EPSILON {
                        deaths.send(DeathEvent {
                            target: entity,
                            cause: "绝壁劫·灵压坍缩".to_string(),
                            attacker: None,
                            attacker_player_id: None,
                            at_tick: clock.tick,
                        });
                    }
                }
                2 => {
                    let factor = juebi_near_factor(distance);
                    if factor <= 0.0 {
                        continue;
                    }
                    commands.entity(entity).insert(JueBiLawDisruption {
                        epicenter: epicenter_block,
                        distance,
                        seed: juebi_hash3(
                            state.started_tick,
                            entity.index() as u64,
                            distance.round().max(0.0) as u64,
                        ),
                    });
                }
                3 => {
                    let radius =
                        juebi_null_radius(clock.tick.saturating_sub(state.phase_started_tick));
                    if distance > radius {
                        continue;
                    }
                    let decay = juebi_null_decay_for_realm(cultivation.realm) * intensity_scale;
                    if decay > 0.0 {
                        let before = cultivation.qi_current;
                        let actual_drain = (before * decay).min(before);
                        let release = release_qi_amount_to_zone(
                            &mut cultivation,
                            actual_drain,
                            Some(position),
                            target_dimension,
                            life_record,
                            zones.as_deref_mut(),
                            &mut ledger,
                            qi_transfers.as_deref_mut(),
                            "juebi_null_field",
                        );
                        if let Err(error) = release {
                            tracing::warn!(
                                ?error,
                                "[bong][cultivation] juebi null qi drain failed closed"
                            );
                            continue;
                        }
                        if cultivation.realm == Realm::Void
                            && before > 0.0
                            && cultivation.qi_current <= f64::EPSILON
                        {
                            deaths.send(DeathEvent {
                                target: entity,
                                cause: "绝壁劫·凡躯崩解".to_string(),
                                attacker: None,
                                attacker_player_id: None,
                                at_tick: clock.tick,
                            });
                        }
                    }
                    commands.entity(entity).insert(JueBiNullified {
                        entered_tick: clock.tick,
                        accumulated_null_time: clock.tick.saturating_sub(state.phase_started_tick)
                            as f64,
                    });
                }
                _ => {}
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_juebi_phase_damage(
    entity: Entity,
    wave: u32,
    distance: f64,
    cultivation: &Cultivation,
    wounds: Option<valence::prelude::Mut<'_, Wounds>>,
    lifecycle: Option<&Lifecycle>,
    state: &TribulationState,
    intensity: f32,
    tick: u64,
    deaths: &mut EventWriter<DeathEvent>,
) {
    let Some(mut wounds) = wounds else {
        return;
    };
    let damage = juebi_phase_damage(wave, distance, cultivation.realm, intensity);
    if damage <= 0.0 {
        return;
    }
    let was_alive = wounds.health_current > 0.0;
    wounds.health_current = (wounds.health_current - damage).clamp(0.0, wounds.health_max);
    wounds.entries.push(Wound {
        // humanoid-only boundary（P0 决议，本轮不迁移）：绝壁天劫波是无差别范围伤害，
        // 固定命中 Chest 代表部位；玩家恒为人形。
        location: crate::body_plan::legacy_body_part_to_id(BodyPart::Chest),
        kind: WoundKind::Concussion,
        severity: damage,
        bleeding_per_sec: 0.0,
        created_at_tick: tick,
        inflicted_by: Some("jue_bi_tribulation".to_string()),
    });
    if !was_alive || wounds.health_current > 0.0 {
        return;
    }
    let is_primary = lifecycle
        .map(|lifecycle| state.is_primary_tribulator(&lifecycle.character_id))
        .unwrap_or(false);
    deaths.send(DeathEvent {
        target: entity,
        cause: if is_primary {
            "绝壁劫·殁".to_string()
        } else {
            "绝壁劫波及而亡".to_string()
        },
        attacker: None,
        attacker_player_id: None,
        at_tick: tick,
    });
}

fn juebi_phase_damage(wave: u32, distance: f64, realm: Realm, intensity: f32) -> f32 {
    let realm_factor = match realm {
        Realm::Void => 1.0,
        Realm::Spirit => 0.65,
        Realm::Solidify | Realm::Condense => 0.18,
        Realm::Induce | Realm::Awaken => 0.08,
    };
    let distance_factor = if distance <= JUEBI_CORE_RADIUS {
        1.5
    } else if distance <= JUEBI_HEAVY_RADIUS {
        1.0
    } else if distance <= JUEBI_ZONE_RADIUS {
        0.5
    } else {
        0.0
    };
    (DUXU_AOE_DAMAGE_BASE
        * wave as f32
        * intensity.clamp(JUEBI_INTENSITY_BASE, 2.0)
        * realm_factor
        * distance_factor)
        .max(0.0)
}

fn juebi_near_factor(distance: f64) -> f64 {
    if !distance.is_finite() || distance < 0.0 {
        return 0.0;
    }
    if distance <= JUEBI_CORE_RADIUS {
        1.0
    } else if distance <= JUEBI_HEAVY_RADIUS {
        1.0 - (distance - JUEBI_CORE_RADIUS) / (JUEBI_HEAVY_RADIUS - JUEBI_CORE_RADIUS)
    } else {
        0.0
    }
}

fn juebi_null_radius(elapsed_ticks: u64) -> f64 {
    let progress = (elapsed_ticks as f64 / JUEBI_PHASE_TICKS as f64).clamp(0.0, 1.0);
    JUEBI_NULL_FIELD_MAX_RADIUS * progress
}

fn juebi_null_decay_for_realm(realm: Realm) -> f64 {
    match realm {
        Realm::Void => JUEBI_NULL_VOID_DECAY_PER_TICK,
        Realm::Spirit => JUEBI_NULL_SPIRIT_DECAY_PER_TICK,
        Realm::Awaken | Realm::Induce | Realm::Condense | Realm::Solidify => 0.0,
    }
}

pub fn juebi_zone_aftershock_system(
    clock: Res<CombatClock>,
    mut triggered: EventReader<JueBiTriggeredEvent>,
    mut aftershocks: ResMut<JueBiZoneAftershocks>,
    mut zones: Option<ResMut<ZoneRegistry>>,
) {
    let Some(zones) = zones.as_deref_mut() else {
        triggered.clear();
        return;
    };

    for event in triggered.read() {
        let p =
            valence::math::DVec3::new(event.epicenter[0], event.epicenter[1], event.epicenter[2]);
        for zone in &mut zones.zones {
            if zone.dimension != event.dimension || !zone.contains(p) {
                continue;
            }
            if !aftershocks.zones.iter().any(|aftershock| {
                aftershock.name == zone.name && aftershock.dimension == zone.dimension
            }) {
                aftershocks.zones.push(JueBiZoneAftershock {
                    name: zone.name.clone(),
                    dimension: zone.dimension,
                    original_qi: zone.spirit_qi,
                    started_tick: clock.tick,
                    restore_until_tick: clock.tick.saturating_add(5 * 60 * 20),
                });
            }
            zone.spirit_qi = 0.0;
            if !zone
                .active_events
                .iter()
                .any(|event| event == "jue_bi_scar")
            {
                zone.active_events.push("jue_bi_scar".to_string());
            }
        }
    }

    aftershocks.zones.retain(|aftershock| {
        let Some(zone) = zones
            .zones
            .iter_mut()
            .find(|zone| zone.name == aftershock.name && zone.dimension == aftershock.dimension)
        else {
            return false;
        };
        if clock.tick >= aftershock.restore_until_tick {
            zone.spirit_qi = (aftershock.original_qi * 0.5).clamp(-1.0, 1.0);
            return false;
        }
        let span = aftershock
            .restore_until_tick
            .saturating_sub(aftershock.started_tick)
            .max(1);
        let elapsed = clock.tick.saturating_sub(aftershock.started_tick);
        let ratio = (elapsed as f64 / span as f64).clamp(0.0, 1.0);
        zone.spirit_qi = (aftershock.original_qi * 0.5 * ratio).clamp(-1.0, 1.0);
        true
    });
}

pub fn juebi_terrain_seed_system(
    mut triggered: EventReader<JueBiTriggeredEvent>,
    mut overlay: ResMut<JueBiTerrainOverlay>,
) {
    for event in triggered.read() {
        if event.dimension != DimensionKind::Overworld {
            continue;
        }
        enqueue_juebi_terrain_ops(
            &mut overlay.pending,
            event.epicenter,
            event.started_tick,
            event.started_tick.saturating_add(5 * 60 * 20),
        );
    }
}

pub fn juebi_terrain_tick_system(
    clock: Res<CombatClock>,
    mut overlay: ResMut<JueBiTerrainOverlay>,
    mut layers: Query<&mut ChunkLayer, With<crate::world::dimension::OverworldLayer>>,
) {
    let Ok(mut layer) = layers.get_single_mut() else {
        return;
    };

    let mut remaining = Vec::with_capacity(overlay.placed.len());
    for block in overlay.placed.drain(..) {
        if block.scar_permanent || clock.tick < block.restore_at_tick {
            remaining.push(block);
        } else {
            layer.set_block(block.pos, block.original);
        }
    }
    overlay.placed = remaining;

    let mut recorded_originals: HashSet<BlockPos> =
        overlay.placed.iter().map(|block| block.pos).collect();
    for _ in 0..overlay.budget_per_tick {
        let Some(op) = overlay.pending.pop_front() else {
            break;
        };
        if layer.chunk(chunk_pos_for_block(op.pos)).is_none() {
            continue;
        }
        if recorded_originals.insert(op.pos) {
            let Some(original) = layer.block(op.pos).map(|block| block.state) else {
                continue;
            };
            overlay.placed.push(JueBiTerrainBlock {
                pos: op.pos,
                original,
                restore_at_tick: op.restore_at_tick,
                scar_permanent: false,
            });
        }
        layer.set_block(op.pos, op.new_state);
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn juebi_settlement_system(
    settings: Res<PersistenceSettings>,
    clock: Res<CombatClock>,
    budget: Res<WorldQiBudget>,
    void_quota: Res<VoidQuotaConfig>,
    mut commands: Commands,
    mut skill_cap_events: EventWriter<SkillCapChanged>,
    mut settled: EventWriter<TribulationSettled>,
    mut quota_occupied: EventWriter<AscensionQuotaOccupied>,
    mut players: Query<(
        Entity,
        &mut Cultivation,
        &Lifecycle,
        Option<&Wounds>,
        Option<&mut LifespanComponent>,
        Option<&mut LifeRecord>,
        &TribulationState,
        Option<&JueBiRuntimeContext>,
        Option<&JueBiAfterDuXuQuota>,
    )>,
) {
    for (
        entity,
        mut cultivation,
        lifecycle,
        wounds,
        lifespan,
        life_record,
        state,
        runtime,
        quota_marker,
    ) in &mut players
    {
        if state.kind != TribulationKind::JueBi || !matches!(state.phase, TribulationPhase::Settle)
        {
            continue;
        }
        let survived = cultivation.qi_current > f64::EPSILON
            && lifecycle.state == LifecycleState::Alive
            && wounds.is_none_or(|wounds| wounds.health_current > 0.0);
        let source = runtime
            .map(|runtime| runtime.source)
            .unwrap_or(JueBiTriggerSource::VoidQuotaExceeded);
        if let Some(mut life_record) = life_record {
            life_record.push(if survived {
                BiographyEntry::JueBiSurvived {
                    source: source.wire_name().to_string(),
                    tick: clock.tick,
                }
            } else {
                BiographyEntry::JueBiKilled {
                    source: source.wire_name().to_string(),
                    tick: clock.tick,
                }
            });
        }

        // plan-halfstep-buff-v1 P2：atomic quota grant 替换原来的 unconditional increment。
        // 仅当 survived + 有 quota_marker（DuXu 起劫时已占额）+ realm=Spirit 才走 ascension 路径；
        // 否则 outcome 由其他分支决定（HalfStep/Killed）。`ascension_granted` 是最终授予标志，
        // 用于 outcome enum + 是否真正翻 Realm 到 Void。
        let mut ascension_granted = false;
        let mut try_complete_invoked = false;
        if survived && quota_marker.is_some() && cultivation.realm == Realm::Spirit {
            let quota_limit = compute_void_quota_limit(budget.current_total, void_quota.quota_k);
            let outcome: AtomicAscensionOutcome = match try_complete_tribulation_ascension(
                &settings,
                lifecycle.character_id.as_str(),
                quota_limit,
            ) {
                Ok(outcome) => outcome,
                Err(error) => {
                    tracing::error!(
                        "[bong][cultivation] failed to finalize void-quota JueBi ascension for {:?}: {error}",
                        entity,
                    );
                    continue;
                }
            };
            try_complete_invoked = true;
            match outcome.grant {
                AscensionGrant::Granted => {
                    ascension_granted = true;
                    quota_occupied.send(AscensionQuotaOccupied {
                        occupied_slots: outcome.quota.occupied_slots,
                    });
                    cultivation.realm = Realm::Void;
                    cultivation.qi_max *= super::breakthrough::qi_max_multiplier(Realm::Void);
                    if let Some(mut lifespan) = lifespan {
                        lifespan.apply_cap(LifespanCapTable::VOID);
                    }
                    let new_cap = skill_cap_for_realm(Realm::Void);
                    for skill in SkillId::ALL {
                        skill_cap_events.send(SkillCapChanged {
                            char_entity: entity,
                            skill,
                            new_cap,
                        });
                    }
                }
                AscensionGrant::Denied => {
                    tracing::info!(
                        "[bong][cultivation] {:?} ascension denied at settle by atomic quota check \
                         (occupied_before={} limit={}); falling back to HalfStep",
                        entity,
                        outcome.occupied_before,
                        outcome.limit_used,
                    );
                }
                AscensionGrant::SettledOnly => {
                    // 非占额路径（独立 JueBi 等）幸存；不升 Realm，仅作 settlement 完毕标志。
                    // 实际 juebi_settlement_system 这条 caller 路径只在 quota_marker.is_some()
                    // 才调 try_complete，理论不会进入；保留分支防御未来 caller 扩展
                    tracing::info!(
                        "[bong][cultivation] {:?} settle-only path at atomic quota check \
                         (non-quota-occupying tribulation source); no Realm elevation",
                        entity,
                    );
                }
                AscensionGrant::MissingActive => {
                    // 状态不一致：start 时插入了 marker 但 settle 时 active row 已消失。
                    // 可能是另一进程 / 重复结算 / row 被外部清理。回退 HalfStep（保守），不升 Realm
                    tracing::warn!(
                        "[bong][cultivation] {:?} ascension at settle saw missing tribulations_active row \
                         (possible double-settle or external cleanup); falling back to HalfStep",
                        entity,
                    );
                }
            }
        }
        // 仅当未调用 try_complete（active row 未被事务删除）时才需要单独清理 active 行
        if !try_complete_invoked {
            if let Err(error) =
                delete_active_tribulation(&settings, lifecycle.character_id.as_str())
            {
                tracing::warn!(
                    "[bong][cultivation] failed to delete settled JueBi active row for {:?}: {error}",
                    entity,
                );
            }
        }

        settled.send(TribulationSettled {
            entity,
            kind: TribulationKind::JueBi,
            source: Some(source),
            result: DuXuResultV1 {
                char_id: lifecycle.character_id.clone(),
                outcome: if survived && ascension_granted {
                    DuXuOutcomeV1::Ascended
                } else if survived {
                    DuXuOutcomeV1::HalfStep
                } else {
                    DuXuOutcomeV1::Killed
                },
                killer: None,
                waves_survived: state.waves_total,
                reason: Some(
                    quota_marker
                        .map(|_| VOID_QUOTA_EXCEEDED_REASON.to_string())
                        .unwrap_or_else(|| format!("jue_bi:{}", source.wire_name())),
                ),
            },
        });
        if survived {
            commands.entity(entity).insert(JueBiAftershockDebuff {
                until_tick: clock.tick.saturating_add(JUEBI_AFTERSHOCK_TICKS),
                rhythm_multiplier: 0.5,
            });
        }
        commands.entity(entity).remove::<(
            TribulationState,
            TribulationOriginDimension,
            HeartDemonResolution,
            PendingHeartDemonOffer,
            JueBiAfterDuXuQuota,
            JueBiRuntimeContext,
            JueBiPressureCollapse,
            JueBiLawDisruption,
            JueBiNullified,
        )>();
    }
}

/// plan-halfstep-buff-v1 P0+P1+P3：消费 `TribulationSettled` 累计 halfstep / ascended 计数；
/// HalfStep outcome 时应用 buff (qi_max ×1.10 / lifespan +200) + qi_physics ledger 标记 +
/// 插入/更新 `HalfStepState` + 确保 FIFO 队列里有 entry。`HalfStepState.buff_applied` 守卫
/// 保证 buff 只 apply 一次（§8 Q4）；保留原 `entered_at` 让 §8 Q1 重渡窗口起点稳定。
///
/// 设计要点：
/// - **buff_applied=false → true 是状态机的单向跃迁**：dormant NPC 第一次 HalfStep 时缺
///   `Cultivation`（buff_applied 留 false），后续 hydrate 再次结算 HalfStep 时**必须**回写
///   buff_applied=true 并应用 buff，否则 dormant 玩家永远拿不到 buff
/// - **二次入队是 §8 Q1 + Q2 的可恢复性保证**：dispatch system pop_front 后玩家若没起劫
///   就结束（被新一波 HalfStep settlement 再次结算，例如重渡又失败回 HalfStep 的边界情况），
///   必须**重新入队**才能在下次 quota 空出时被通知；否则永久失去 FCFS 资格
/// - **ledger 是 audit-only**：emit `QiTransfer` event 但不调 `WorldQiAccount::transfer`（已
///   在 `WorldQiAccount::transfer` 入口硬拒，参 `qi_physics::ledger` 中 `AuditOnlyReason` 守卫）
/// - **终态清理**：Ascended/Killed/Failed/Fled 不光出队，**还要 remove HalfStepState** —— 否则
///   下一轮 HalfStep（如复活 + 重新渡虚劫失败）会用旧 `entered_at`、`buff_applied=true` 守卫
///   错乱，污染整个状态机
#[allow(clippy::too_many_arguments)]
pub fn track_tribulation_metrics_system(
    mut commands: Commands,
    mut events: EventReader<TribulationSettled>,
    clock: Res<CombatClock>,
    mut metrics: ResMut<TribulationMetrics>,
    mut queue: ResMut<HalfStepRechallengeQueue>,
    mut targets: Query<(&mut Cultivation, Option<&mut LifespanComponent>)>,
    existing_states: Query<&HalfStepState>,
    mut qi_transfers: EventWriter<QiTransfer>,
) {
    // plan-halfstep-buff-v1 fix（CodeRabbit P4 review #1）：同帧多条 HalfStep event 给同一 entity
    // 时，`commands.entity().insert()` 是 deferred 到 system 结束才生效，所以第二条 event 仍会
    // 从 `existing_states` 里读到旧值（buff_applied=false） → 重复应用 buff、破 §8 Q4。
    // 用 local HashMap 在 system 内部 staged 已处理的状态作为 commands 队列的镜像。
    let mut staged_states: std::collections::HashMap<Entity, HalfStepState> =
        std::collections::HashMap::new();

    for ev in events.read() {
        match ev.result.outcome {
            DuXuOutcomeV1::HalfStep => {
                // plan-halfstep-buff-v1 fix（CodeRabbit P3 review #1 outside-diff）：
                // 只有 `VoidQuotaExceeded` 路径才是真半步化虚（升格冲刺被 quota 阻挡）；
                // 其他 JueBi 来源（当前 `VoidActionExplodeZone`，未来可能扩展 ZoneCollapse /
                // Hypocrite 等）虽然 settlement outcome 也是 HalfStep（schema 兼容），但语义上
                // 是**"额外天劫幸存"**——化虚老怪可能扛过 zone collapse 触发的 JueBi 而幸存，
                // 但他/她**没在升格**，绝不应授予半步 buff、不入重渡队列、不计 halfstep_count
                if !matches!(ev.source, Some(JueBiTriggerSource::VoidQuotaExceeded)) {
                    continue;
                }
                metrics.halfstep_count = metrics.halfstep_count.saturating_add(1);

                // 状态优先级（P4 review #2 fix）：
                //   1. staged（本帧已处理） —— 解决 commands 同帧延迟问题
                //   2. existing_states（world 现状） —— 跨帧 buff 透传
                //   3. queue 中按 char_id 找的旧 entry —— dormant→hydrate 换 ECS entity 时
                //      复用原 entered_at/window，防 §8 Q1 7d 窗口被刷新 + §8 Q2 FCFS 乱序
                //   4. 全新（用 clock.tick）
                let prior_state_from_components = staged_states
                    .get(&ev.entity)
                    .copied()
                    .or_else(|| existing_states.get(ev.entity).ok().copied());
                let (existing_entered_at, existing_window, already_buffed) =
                    match prior_state_from_components {
                        Some(state) => (
                            Some(state.entered_at),
                            Some(state.rechallenge_window_until),
                            state.buff_applied,
                        ),
                        None => {
                            // component 缺失 → 按 char_id 查队列里的旧 entry，复用 entered_at /
                            // window_until + **buff_applied**（P5 review #1 fix）。
                            //
                            // 如果旧 entry 已 buff_applied=true，新 entity（同 char_id）必继承，
                            // 否则 dormant→hydrate 同角色再 settle 会重复加 buff（破 §8 Q4）。
                            // 这是 P5 修复的真 bug：上一版回退 false 是错的"保守"——保守应是
                            // 假设已 buff（避免重复加），而非假设没 buff（导致重复加）
                            match queue.find_by_char_id(&ev.result.char_id) {
                                Some(entry) => (
                                    Some(entry.entered_at),
                                    Some(entry.rechallenge_window_until),
                                    entry.buff_applied,
                                ),
                                None => (None, None, false),
                            }
                        }
                    };

                let mut buff_now_applied = already_buffed;
                // P4 review #1 fix：用 has_cultivation 兼做 is_dormant heuristic。
                // 缺 Cultivation 的 entity 是 dormant 占位（参 plan-npc-virtualize-v1）；
                // hydrated 玩家/NPC 都带 Cultivation。这条让 dispatch 系统派发的 trigger
                // 能区分在线/休眠目标，hydrate-on-trigger 路径才能正确识别
                let mut has_cultivation = false;
                if !already_buffed {
                    if let Ok((mut cultivation, lifespan)) = targets.get_mut(ev.entity) {
                        has_cultivation = true;
                        let before = cultivation.qi_max;
                        cultivation.qi_max *= 1.0 + HALFSTEP_QI_MAX_BONUS as f64;
                        let bonus_capacity = cultivation.qi_max - before;
                        // qi_physics ledger audit-only 标记（emit event；WorldQiAccount::transfer
                        // 拒绝该 reason，防止误调动 balance）
                        if bonus_capacity > 0.0 {
                            if let Ok(transfer) = QiTransfer::new(
                                QiAccountId::tiandao(),
                                QiAccountId::player(ev.result.char_id.clone()),
                                bonus_capacity,
                                QiTransferReason::HalfStepBuff,
                            ) {
                                qi_transfers.send(transfer);
                            }
                        }
                        if let Some(mut lifespan) = lifespan {
                            let new_cap = lifespan
                                .cap_by_realm
                                .saturating_add(HALFSTEP_LIFESPAN_BONUS_YEARS);
                            lifespan.apply_cap(new_cap);
                        }
                        buff_now_applied = true;
                    }
                } else {
                    // 已 buffed 不 reapply，但仍需要 has_cultivation 判 is_dormant
                    has_cultivation = targets.get(ev.entity).is_ok();
                }

                let entered_at = existing_entered_at.unwrap_or(clock.tick);
                let window_until = existing_window
                    .unwrap_or_else(|| entered_at.saturating_add(RECHALLENGE_WINDOW_TICKS));

                // upsert HalfStepState：新建或回写 buff_applied（dormant→hydrate 路径 + Cultivation
                // 缺失 → 已 hydrate 路径都靠这条统一）
                let new_state = HalfStepState {
                    entered_at,
                    rechallenge_window_until: window_until,
                    buff_applied: buff_now_applied,
                };
                commands.entity(ev.entity).insert(new_state);
                staged_states.insert(ev.entity, new_state);

                // 确保队列里有 entry（dedup-then-enqueue，防止二次 HalfStep 留下重复或
                // dispatch pop_front 过后永久失去重渡资格）。
                // **双键 dedup**：char_id 是持久化稳定键，entity 是 ECS 句柄；
                // dormant→hydrate 之间 ECS entity 会换号，仅按 entity 清不掉同一角色的
                // 旧 entry，会造成同 char_id 双入队 + 收到重复 trigger。
                // 注意：必须先 find_by_char_id 取旧时间戳（上面 prior_state fallback 用），
                // 再 remove_char_id 删旧条目
                queue.remove_entity(ev.entity);
                queue.remove_char_id(&ev.result.char_id);
                queue.enqueue(HalfStepRechallengeEntry {
                    char_id: ev.result.char_id.clone(),
                    entity: ev.entity,
                    entered_at,
                    rechallenge_window_until: window_until,
                    is_dormant: !has_cultivation,
                    // P5 review #1 fix：镜像 HalfStepState.buff_applied 让 char_id-fallback
                    // 复用（dormant→hydrate 同角色再 settle 时不重复加 buff）
                    buff_applied: buff_now_applied,
                });
            }
            DuXuOutcomeV1::Ascended => {
                metrics.ascended_count = metrics.ascended_count.saturating_add(1);
                // 化虚成功 → 不再是 HalfStep；清队 + 清 component 防下一轮污染
                queue.remove_entity(ev.entity);
                queue.remove_char_id(&ev.result.char_id);
                commands.entity(ev.entity).remove::<HalfStepState>();
                // 也清 staged：本帧若 HalfStep→Ascended 接续发生（罕见但可能 in test），
                // 后续 HalfStep 在同帧应按"全新"处理而非看到旧 staged buff_applied=true
                staged_states.remove(&ev.entity);
            }
            DuXuOutcomeV1::Killed | DuXuOutcomeV1::Failed | DuXuOutcomeV1::Fled => {
                // 死亡 / 降境 / 逃跑 → 同 Ascended：清队 + 清 component；下一轮重新建状态
                queue.remove_entity(ev.entity);
                queue.remove_char_id(&ev.result.char_id);
                commands.entity(ev.entity).remove::<HalfStepState>();
                staged_states.remove(&ev.entity);
            }
        }
    }
}

/// plan-halfstep-buff-v1 P3：`AscensionQuotaOpened` 事件触发派发——每个事件取队列头
/// 一个有效（未过窗）entry 发出 `HalfStepRechallengeTriggerEvent`；过窗 entry 出队丢弃。
///
/// 玩家头部 entry → emit 事件，client HUD 监听后弹"灵机涌现，可重渡虚劫"提示。
/// dormant NPC 头部 entry（`is_dormant=true`）→ 同样 emit；plan-npc-virtualize-v1
/// 的 hydrate-on-trigger 路径监听 dormant 标记，强制 hydrate 后玩家路径生效。
///
/// 多个 quota slot 同时空出（多个 events）→ 队列出多个头，按 FIFO 顺序通知。
pub fn dispatch_rechallenge_on_quota_opened_system(
    mut events: EventReader<AscensionQuotaOpened>,
    mut queue: ResMut<HalfStepRechallengeQueue>,
    mut triggers: EventWriter<HalfStepRechallengeTriggerEvent>,
    clock: Res<CombatClock>,
) {
    let event_count = events.read().count();
    for _ in 0..event_count {
        loop {
            let Some(entry) = queue.queue.pop_front() else {
                return; // 队列空，停
            };
            if clock.tick > entry.rechallenge_window_until {
                tracing::info!(
                    "[bong][cultivation] HalfStep rechallenge entry {} expired (window_until={}, current_tick={}), dropping",
                    entry.char_id,
                    entry.rechallenge_window_until,
                    clock.tick,
                );
                continue; // 过窗，继续取下一个
            }
            triggers.send(HalfStepRechallengeTriggerEvent {
                char_id: entry.char_id,
                entity: entry.entity,
                is_dormant: entry.is_dormant,
                at_tick: clock.tick,
            });
            break; // 本 event 派发完毕
        }
    }
}

/// plan-halfstep-buff-v1 P0：事件驱动追踪 quota 满时长。
///
/// 由 `AscensionQuotaOpened` / `AscensionQuotaOccupied` 事件触发；状态变化时根据
/// `check_void_quota` 重新计算 limit；进入 full 状态记 `full_since_tick`，离开
/// full 状态把累计 ticks 写入 `TribulationMetrics.quota_full_duration_ticks`。
///
/// 当前 pending（仍在 full 状态的累计）可由 `current_quota_full_duration_ticks` 取得。
pub fn track_quota_full_duration_system(
    mut tracker: ResMut<QuotaFullTracker>,
    mut metrics: ResMut<TribulationMetrics>,
    mut occupied_events: EventReader<AscensionQuotaOccupied>,
    mut opened_events: EventReader<AscensionQuotaOpened>,
    clock: Res<CombatClock>,
    budget: Res<WorldQiBudget>,
    void_quota: Res<VoidQuotaConfig>,
) {
    let mut latest_occupied: Option<u32> = None;
    for ev in occupied_events.read() {
        latest_occupied = Some(ev.occupied_slots);
    }
    for ev in opened_events.read() {
        latest_occupied = Some(ev.occupied_slots);
    }

    let Some(new_occupied) = latest_occupied else {
        return;
    };

    let quota = check_void_quota(new_occupied, &budget, &void_quota);
    tracker.current_occupied = new_occupied;
    tracker.current_limit = quota.quota_limit;

    let is_full = tracker.current_limit > 0 && tracker.current_occupied >= tracker.current_limit;
    let was_full = tracker.full_since_tick.is_some();

    match (was_full, is_full) {
        (false, true) => {
            tracker.full_since_tick = Some(clock.tick);
        }
        (true, false) => {
            if let Some(since) = tracker.full_since_tick.take() {
                let delta = clock.tick.saturating_sub(since);
                metrics.quota_full_duration_ticks =
                    metrics.quota_full_duration_ticks.saturating_add(delta);
            }
        }
        _ => {}
    }
}

/// plan-halfstep-buff-v1 P0：取当前累计 quota_full_duration_ticks，含仍在 full 状态的 pending。
///
/// 调用方（dev cmd / 测试）可获得"截至当前 tick 为止的真实满时长"，不需要等 quota 状态变化才结算。
pub fn current_quota_full_duration_ticks(
    metrics: &TribulationMetrics,
    tracker: &QuotaFullTracker,
    current_tick: u64,
) -> u64 {
    let base = metrics.quota_full_duration_ticks;
    if let Some(since) = tracker.full_since_tick {
        base.saturating_add(current_tick.saturating_sub(since))
    } else {
        base
    }
}

fn enqueue_juebi_terrain_ops(
    pending: &mut VecDeque<TerrainModOp>,
    epicenter: [f64; 3],
    seed: u64,
    restore_at_tick: u64,
) {
    let origin = block_pos_from_epicenter(epicenter);
    let mut ops = Vec::new();
    generate_radial_fissures(&mut ops, origin, seed, restore_at_tick);
    generate_eruption_cones(&mut ops, origin, seed.rotate_left(17), restore_at_tick);
    generate_surface_upheaval(&mut ops, origin, seed.rotate_left(31), restore_at_tick);
    ops.sort_by_key(|op| op.anim_order);
    pending.extend(ops);
}

fn generate_radial_fissures(
    ops: &mut Vec<TerrainModOp>,
    origin: BlockPos,
    seed: u64,
    restore_at_tick: u64,
) {
    for crack in 0..JUEBI_FISSURE_COUNT {
        let base_angle = std::f64::consts::TAU * crack as f64 / JUEBI_FISSURE_COUNT as f64;
        let jitter = (hash_unit(seed, crack as u64) - 0.5) * 0.6;
        let mut angle = base_angle + jitter;
        let mut x = origin.x as f64;
        let mut z = origin.z as f64;
        let length = (JUEBI_FISSURE_RADIUS as f64
            * (0.6 + hash_unit(seed ^ 0xA11CE, crack as u64) * 0.4)) as i32;
        for step in 0..length {
            x += angle.cos();
            z += angle.sin();
            angle += (hash_unit(seed ^ step as u64, crack as u64) - 0.5) * 0.52;
            let ratio = step as f64 / length.max(1) as f64;
            let depth = crack_depth(ratio, seed, crack as u64, step as u64);
            let width = crack_width(ratio);
            let perp_sin = angle.sin();
            let perp_cos = angle.cos();
            for dw in -(width / 2)..=(width / 2) {
                let wx = (x + dw as f64 * perp_sin).round() as i32;
                let wz = (z - dw as f64 * perp_cos).round() as i32;
                let surface_y = origin.y;
                for dy in 0..depth {
                    let pos = BlockPos::new(wx, (surface_y - dy).clamp(-64, 319), wz);
                    let new_state = if dy >= depth - 1 {
                        BlockState::MAGMA_BLOCK
                    } else if dy >= depth.saturating_sub(3) {
                        BlockState::DEEPSLATE
                    } else {
                        BlockState::AIR
                    };
                    ops.push(TerrainModOp {
                        pos,
                        new_state,
                        anim_order: step as u32,
                        restore_at_tick,
                    });
                }
            }
        }
    }
}

fn generate_eruption_cones(
    ops: &mut Vec<TerrainModOp>,
    origin: BlockPos,
    seed: u64,
    restore_at_tick: u64,
) {
    for cone in 0..JUEBI_CONE_COUNT {
        let theta = hash_unit(seed, cone as u64) * std::f64::consts::TAU;
        let radius = 8.0
            + (1.0 - hash_unit(seed ^ 0xC0E, cone as u64).sqrt())
                * (JUEBI_CONE_MAX_RADIUS as f64 - 8.0);
        let cx = origin.x + (radius * theta.cos()).round() as i32;
        let cz = origin.z + (radius * theta.sin()).round() as i32;
        let dist_ratio = (radius / JUEBI_CONE_MAX_RADIUS as f64).clamp(0.0, 1.0);
        let height = (100.0 - 50.0 * dist_ratio
            + (hash_unit(seed ^ 0x51A7, cone as u64) - 0.5) * 16.0)
            .round()
            .clamp(50.0, 100.0) as i32;
        let base_radius = (18.0 - 8.0 * dist_ratio
            + (hash_unit(seed ^ 0xB45E, cone as u64) - 0.5) * 4.0)
            .round()
            .clamp(10.0, 18.0) as i32;
        for dy in 0..height {
            let layer_ratio = dy as f64 / height.max(1) as f64;
            let layer_radius = base_radius as f64 * (1.0 - layer_ratio * 0.93);
            let r_ceil = layer_radius.ceil() as i32 + 1;
            for dx in -r_ceil..=r_ceil {
                for dz in -r_ceil..=r_ceil {
                    let dist = ((dx * dx + dz * dz) as f64).sqrt();
                    let noise = (hash_unit(seed ^ dy as u64, (dx as i64 as u64) ^ dz as u64) - 0.5)
                        * layer_radius
                        * 0.35;
                    if dist > layer_radius + noise {
                        continue;
                    }
                    ops.push(TerrainModOp {
                        pos: BlockPos::new(cx + dx, (origin.y + dy).clamp(-64, 319), cz + dz),
                        new_state: cone_block(dy, height, dist, layer_radius, seed),
                        anim_order: 220 + dy as u32,
                        restore_at_tick,
                    });
                }
            }
        }
    }
}

fn generate_surface_upheaval(
    ops: &mut Vec<TerrainModOp>,
    origin: BlockPos,
    seed: u64,
    restore_at_tick: u64,
) {
    for x in (origin.x - JUEBI_UPHEAVAL_OUTER_RADIUS)..=(origin.x + JUEBI_UPHEAVAL_OUTER_RADIUS) {
        for z in (origin.z - JUEBI_UPHEAVAL_OUTER_RADIUS)..=(origin.z + JUEBI_UPHEAVAL_OUTER_RADIUS)
        {
            let dx = (x - origin.x) as f64;
            let dz = (z - origin.z) as f64;
            let dist = (dx * dx + dz * dz).sqrt();
            if dist < JUEBI_UPHEAVAL_INNER_RADIUS as f64
                || dist > JUEBI_UPHEAVAL_OUTER_RADIUS as f64
            {
                continue;
            }
            if (juebi_hash3(seed, x as i64 as u64, z as i64 as u64) % 1000)
                > JUEBI_UPHEAVAL_DENSITY_PER_MILLE as u64
            {
                continue;
            }
            let ratio = (dist - JUEBI_UPHEAVAL_INNER_RADIUS as f64)
                / (JUEBI_UPHEAVAL_OUTER_RADIUS - JUEBI_UPHEAVAL_INNER_RADIUS) as f64;
            let max_shift = 5.0 - ratio * 4.0;
            let signed = hash_unit(seed ^ 0xD15C, (x as i64 as u64) ^ z as u64) * 2.0 - 1.0;
            let shift = (signed * max_shift).round() as i32;
            if shift == 0 {
                continue;
            }
            if shift > 0 {
                for dy in 1..=shift {
                    ops.push(TerrainModOp {
                        pos: BlockPos::new(x, (origin.y + dy).clamp(-64, 319), z),
                        new_state: BlockState::DEEPSLATE,
                        anim_order: 560 + dist.round() as u32,
                        restore_at_tick,
                    });
                }
            } else {
                for dy in 0..shift.unsigned_abs() as i32 {
                    ops.push(TerrainModOp {
                        pos: BlockPos::new(x, (origin.y - dy).clamp(-64, 319), z),
                        new_state: BlockState::AIR,
                        anim_order: 560 + dist.round() as u32,
                        restore_at_tick,
                    });
                }
            }
        }
    }
}

fn crack_depth(ratio: f64, seed: u64, crack: u64, step: u64) -> i32 {
    let jitter = (hash_unit(seed ^ crack, step) * 5.0).round() as i32;
    match ratio {
        r if r < 0.2 => 40 + jitter.clamp(0, 10),
        r if r < 0.5 => 20 + jitter.clamp(0, 20),
        r if r < 0.8 => 8 + jitter.clamp(0, 12),
        _ => 2 + jitter.clamp(0, 6),
    }
}

fn crack_width(ratio: f64) -> i32 {
    match ratio {
        r if r < 0.2 => 5,
        r if r < 0.5 => 3,
        r if r < 0.8 => 2,
        _ => 1,
    }
}

fn cone_block(
    dy: i32,
    total_height: i32,
    dist_from_axis: f64,
    layer_radius: f64,
    seed: u64,
) -> BlockState {
    let height_ratio = dy as f64 / total_height.max(1) as f64;
    let edge_ratio = dist_from_axis / layer_radius.max(1.0);
    let shell = edge_ratio > 0.75;
    if height_ratio < 0.25 {
        if shell {
            BlockState::COBBLED_DEEPSLATE
        } else {
            BlockState::DEEPSLATE
        }
    } else if height_ratio < 0.65 {
        if hash_unit(seed ^ dy as u64, dist_from_axis.round() as u64) < 0.12 {
            BlockState::CRYING_OBSIDIAN
        } else if shell {
            BlockState::POLISHED_BASALT
        } else {
            BlockState::BASALT
        }
    } else if height_ratio < 0.90 {
        if hash_unit(seed ^ 0xB1A, dy as u64) < 0.20 {
            BlockState::CRYING_OBSIDIAN
        } else {
            BlockState::BLACKSTONE
        }
    } else {
        BlockState::OBSIDIAN
    }
}

fn block_pos_from_epicenter(epicenter: [f64; 3]) -> BlockPos {
    BlockPos::new(
        epicenter[0].round() as i32,
        (epicenter[1].round() as i32).clamp(-64, 319),
        epicenter[2].round() as i32,
    )
}

fn chunk_pos_for_block(pos: BlockPos) -> ChunkPos {
    ChunkPos::new(pos.x.div_euclid(16), pos.z.div_euclid(16))
}

fn hash_unit(seed: u64, salt: u64) -> f64 {
    (juebi_hash3(seed, salt, 0) as f64) / (u64::MAX as f64)
}

fn juebi_hash3(a: u64, b: u64, c: u64) -> u64 {
    let mut x = a ^ 0x9E37_79B9_7F4A_7C15;
    x ^= b.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = x.rotate_left(27);
    x ^= c.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[allow(clippy::too_many_arguments)]
pub fn emit_tribulation_boundary_vfx_system(
    clock: Res<CombatClock>,
    mut announce: EventReader<TribulationAnnounce>,
    mut juebi_triggered: EventReader<JueBiTriggeredEvent>,
    mut locked: EventReader<TribulationLocked>,
    mut cleared: EventReader<TribulationWaveCleared>,
    mut omen_soft_emitted: valence::prelude::Local<HashSet<Entity>>,
    states: Query<(Entity, &TribulationState)>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    omen_soft_emitted.retain(|entity| {
        states
            .get(*entity)
            .is_ok_and(|(_, state)| matches!(state.phase, TribulationPhase::Omen))
    });
    for (entity, state) in &states {
        if matches!(state.phase, TribulationPhase::Omen)
            && clock.tick.saturating_sub(state.started_tick) >= DUXU_OMEN_TICKS / 2
            && omen_soft_emitted.insert(entity)
        {
            emit_tribulation_boundary_vfx(
                &mut vfx_events,
                state.epicenter,
                DUXU_LOCK_RADIUS_SOFT,
                200,
            );
        }
    }
    for ev in announce.read() {
        emit_tribulation_omen_cloud_vfx(&mut vfx_events, ev.epicenter);
        emit_tribulation_boundary_vfx(
            &mut vfx_events,
            ev.epicenter,
            TRIBULATION_DANGER_RADIUS,
            200,
        );
    }
    for ev in juebi_triggered.read() {
        emit_juebi_vfx(
            &mut vfx_events,
            ev.epicenter,
            JUEBI_BOUNDARY_VFX_EVENT_ID,
            JUEBI_ZONE_RADIUS,
            220,
            ev.intensity,
        );
        emit_juebi_vfx(
            &mut vfx_events,
            ev.epicenter,
            JUEBI_FISSURE_VFX_EVENT_ID,
            JUEBI_HEAVY_RADIUS,
            240,
            ev.intensity,
        );
    }
    for ev in locked.read() {
        emit_tribulation_boundary_vfx(&mut vfx_events, ev.epicenter, DUXU_LOCK_RADIUS_HARD, 160);
    }
    for ev in cleared.read() {
        let Ok((_, state)) = states.get(ev.entity) else {
            continue;
        };
        if state.kind == TribulationKind::JueBi {
            let event_id = match ev.wave {
                1 => JUEBI_FISSURE_VFX_EVENT_ID,
                2 => JUEBI_BOUNDARY_VFX_EVENT_ID,
                _ => JUEBI_ERUPTION_VFX_EVENT_ID,
            };
            emit_juebi_vfx(
                &mut vfx_events,
                state.epicenter,
                event_id,
                state.lock_radius(clock.tick),
                180,
                JUEBI_INTENSITY_BASE,
            );
        } else {
            emit_tribulation_boundary_vfx(
                &mut vfx_events,
                state.epicenter,
                DUXU_LOCK_RADIUS_FINAL,
                100,
            );
        }
    }
}

fn emit_tribulation_boundary_vfx(
    vfx_events: &mut EventWriter<VfxEventRequest>,
    epicenter: [f64; 3],
    radius: f64,
    duration_ticks: u16,
) {
    let origin = valence::math::DVec3::new(epicenter[0], epicenter[1], epicenter[2]);
    vfx_events.send(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id: DUXU_BOUNDARY_VFX_EVENT_ID.to_string(),
            origin: epicenter,
            direction: None,
            color: Some("#D0C8FF".to_string()),
            strength: Some((radius / TRIBULATION_DANGER_RADIUS).clamp(0.0, 1.0) as f32),
            count: Some(1),
            duration_ticks: Some(duration_ticks),
        },
    ));
}

fn emit_tribulation_omen_cloud_vfx(
    vfx_events: &mut EventWriter<VfxEventRequest>,
    epicenter: [f64; 3],
) {
    let origin = [epicenter[0], epicenter[1] + 24.0, epicenter[2]];
    vfx_events.send(VfxEventRequest::new(
        valence::math::DVec3::new(origin[0], origin[1], origin[2]),
        VfxEventPayloadV1::SpawnParticle {
            event_id: DUXU_OMEN_CLOUD_VFX_EVENT_ID.to_string(),
            origin,
            direction: Some([24.0, 8.0, 24.0]),
            color: Some("#3B3448".to_string()),
            strength: Some(0.85),
            count: Some(36),
            duration_ticks: Some(200),
        },
    ));
}

fn emit_juebi_vfx(
    vfx_events: &mut EventWriter<VfxEventRequest>,
    epicenter: [f64; 3],
    event_id: &str,
    radius: f64,
    duration_ticks: u16,
    intensity: f32,
) {
    let origin = valence::math::DVec3::new(epicenter[0], epicenter[1], epicenter[2]);
    vfx_events.send(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id: event_id.to_string(),
            origin: epicenter,
            direction: Some([radius, 0.0, radius]),
            color: Some("#140D18".to_string()),
            strength: Some((intensity / JUEBI_INTENSITY_BASE).clamp(0.5, 1.6)),
            count: Some(48),
            duration_ticks: Some(duration_ticks),
        },
    ));
}

pub fn tribulation_omen_cloud_block_overlay_system(
    clock: Res<CombatClock>,
    mut announced: EventReader<TribulationAnnounce>,
    active: Query<&TribulationState>,
    mut clouds: ResMut<TribulationOmenCloudBlocks>,
    mut layers: Query<&mut ChunkLayer, With<crate::world::dimension::OverworldLayer>>,
) {
    let Ok(mut layer) = layers.get_single_mut() else {
        announced.clear();
        return;
    };

    let mut next_blocks = Vec::with_capacity(clouds.blocks.len());
    for block in clouds.blocks.drain(..) {
        let still_omen = active.get(block.entity).is_ok_and(|state| {
            matches!(state.phase, TribulationPhase::Omen) && clock.tick < block.expires_at_tick
        });
        if still_omen {
            next_blocks.push(block);
        } else {
            layer.set_block(block.pos, block.original);
        }
    }
    clouds.blocks = next_blocks;

    for event in announced.read() {
        if active
            .get(event.entity)
            .is_ok_and(|state| !matches!(state.phase, TribulationPhase::Omen))
        {
            continue;
        }
        let y =
            (event.epicenter[1].round() as i32 + DUXU_OMEN_CLOUD_BLOCK_Y_OFFSET).clamp(-64, 319);
        let expires_at_tick = event.started_tick.saturating_add(DUXU_OMEN_TICKS);
        for dx in DUXU_OMEN_CLOUD_BLOCK_OFFSETS {
            for dz in DUXU_OMEN_CLOUD_BLOCK_OFFSETS {
                if dx.abs() + dz.abs() > 12 {
                    continue;
                }
                let pos = BlockPos::new(
                    event.epicenter[0].round() as i32 + dx,
                    y,
                    event.epicenter[2].round() as i32 + dz,
                );
                if clouds
                    .blocks
                    .iter()
                    .any(|block| block.entity == event.entity && block.pos == pos)
                {
                    continue;
                }
                let original = layer
                    .block(pos)
                    .map(|block| block.state)
                    .unwrap_or(BlockState::AIR);
                layer.set_block(pos, omen_cloud_block_for_offset(dx, dz));
                clouds.blocks.push(TribulationOmenCloudBlock {
                    entity: event.entity,
                    pos,
                    original,
                    expires_at_tick,
                });
            }
        }
    }
}

fn omen_cloud_block_for_offset(dx: i32, dz: i32) -> BlockState {
    if dx == 0 && dz == 0 {
        BlockState::BLACK_WOOL
    } else {
        BlockState::WHITE_WOOL
    }
}

#[allow(clippy::type_complexity)]
pub fn heart_demon_choice_system(
    mut choices: EventReader<HeartDemonChoiceSubmitted>,
    mut commands: Commands,
    mut players: Query<(
        &mut Cultivation,
        &mut TribulationState,
        Option<&mut LifeRecord>,
        Option<&HeartDemonResolution>,
        Option<&Position>,
        Option<&CurrentDimension>,
    )>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut ledger: ResMut<WorldQiAccount>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
) {
    // review finding major-4：一个 Update 内每个实体只消费**首个**决策，后续（含同 tick
    // 双发包）一律拒绝。此前 HeartDemonResolution 经 deferred Commands 插入，同一 system
    // 调用内对后续事件不可见，两条 Obsession 包会把 30% 真元惩罚叠两次、混合包会留下
    // 最后一条的结局与倍率——自相矛盾的 biography/状态。
    let mut processed: HashSet<Entity> = HashSet::new();
    for choice in choices.read() {
        if !processed.insert(choice.entity) {
            continue;
        }
        let Ok((
            mut cultivation,
            state,
            life_record,
            existing_resolution,
            position,
            current_dimension,
        )) = players.get_mut(choice.entity)
        else {
            continue;
        };
        if !matches!(state.phase, TribulationPhase::HeartDemon) {
            continue;
        }
        resolve_heart_demon_choice(
            HeartDemonDecision {
                entity: choice.entity,
                choice_idx: choice.choice_idx,
                tick: choice.submitted_at_tick,
            },
            &mut commands,
            &mut cultivation,
            &state,
            life_record,
            existing_resolution,
            position,
            current_dimension,
            zones.as_deref_mut(),
            &mut ledger,
            qi_transfers.as_deref_mut(),
        );
    }
}

#[allow(clippy::type_complexity)]
pub fn heart_demon_timeout_system(
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut players: Query<(
        Entity,
        &mut Cultivation,
        &mut TribulationState,
        Option<&mut LifeRecord>,
        Option<&HeartDemonResolution>,
        Option<&Position>,
        Option<&CurrentDimension>,
    )>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut ledger: ResMut<WorldQiAccount>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
) {
    for (
        entity,
        mut cultivation,
        state,
        life_record,
        existing_resolution,
        position,
        current_dimension,
    ) in &mut players
    {
        if !matches!(state.phase, TribulationPhase::HeartDemon) {
            continue;
        }
        if existing_resolution.is_some()
            || clock.tick.saturating_sub(state.phase_started_tick) < DUXU_HEART_DEMON_TIMEOUT_TICKS
        {
            continue;
        }
        resolve_heart_demon_choice(
            HeartDemonDecision {
                entity,
                choice_idx: None,
                tick: clock.tick,
            },
            &mut commands,
            &mut cultivation,
            &state,
            life_record,
            existing_resolution,
            position,
            current_dimension,
            zones.as_deref_mut(),
            &mut ledger,
            qi_transfers.as_deref_mut(),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_heart_demon_choice(
    decision: HeartDemonDecision,
    commands: &mut Commands,
    cultivation: &mut Cultivation,
    state: &TribulationState,
    life_record: Option<valence::prelude::Mut<'_, LifeRecord>>,
    existing_resolution: Option<&HeartDemonResolution>,
    position: Option<&Position>,
    current_dimension: Option<&CurrentDimension>,
    zones: Option<&mut ZoneRegistry>,
    ledger: &mut WorldQiAccount,
    qi_transfers: Option<&mut Events<QiTransfer>>,
) {
    if existing_resolution.is_some() {
        return;
    }
    if !matches!(state.phase, TribulationPhase::HeartDemon) {
        return;
    }
    let outcome = heart_demon_outcome_for_choice(decision.choice_idx);
    let mut next_wave_multiplier = 1.0;
    match outcome {
        HeartDemonOutcome::Steadfast => {
            let effective_qi_max =
                (cultivation.qi_max - cultivation.qi_max_frozen.unwrap_or(0.0)).max(0.0);
            let desired_grant =
                (effective_qi_max * 0.10).min((effective_qi_max - cultivation.qi_current).max(0.0));
            if desired_grant > QI_EPSILON {
                let Some(life_record_ref) = life_record.as_deref() else {
                    tracing::warn!(
                        "[bong][cultivation] HeartDemon Steadfast missing canonical LifeRecord; qi grant failed closed"
                    );
                    return;
                };
                let actor = match ActorQiIdentity::from_life_record(
                    life_record_ref,
                    ActorQiKind::Player,
                ) {
                    Ok(actor) => actor,
                    Err(error) => {
                        tracing::warn!(
                            ?error,
                            "[bong][cultivation] HeartDemon Steadfast invalid actor identity; qi grant failed closed"
                        );
                        return;
                    }
                };
                let Some(position) = position else {
                    tracing::warn!(
                        "[bong][cultivation] HeartDemon Steadfast missing Position; qi grant failed closed"
                    );
                    return;
                };
                let Some(current_dimension) = current_dimension else {
                    tracing::warn!(
                        "[bong][cultivation] HeartDemon Steadfast missing CurrentDimension; qi grant failed closed"
                    );
                    return;
                };
                let Some(zones) = zones else {
                    tracing::warn!(
                        "[bong][cultivation] HeartDemon Steadfast missing ZoneRegistry; qi grant failed closed"
                    );
                    return;
                };
                let Some(zone) = zones.find_zone_mut_by_pos(current_dimension.0, position.0) else {
                    tracing::warn!(
                        "[bong][cultivation] HeartDemon Steadfast outside known zone; qi grant failed closed"
                    );
                    return;
                };
                let gain = cultivation.gain_from_zone(
                    zone,
                    ledger,
                    &actor,
                    desired_grant,
                    QiTransferReason::CultivationRegen,
                );
                let outcome = match gain {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        tracing::warn!(
                            ?error,
                            "[bong][cultivation] HeartDemon Steadfast qi gain failed closed"
                        );
                        return;
                    }
                };
                if let Some(qi_transfers) = qi_transfers {
                    for transfer in outcome.transfers {
                        qi_transfers.send(transfer);
                    }
                }
            }
        }
        HeartDemonOutcome::Obsession => {
            let actual_drain = cultivation.qi_current * DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO;
            let release = release_qi_amount_to_zone(
                cultivation,
                actual_drain,
                position,
                current_dimension,
                life_record.as_deref(),
                zones,
                ledger,
                qi_transfers,
                "heart_demon_obsession",
            );
            if let Err(error) = release {
                tracing::warn!(
                    ?error,
                    "[bong][cultivation] heart demon qi drain failed closed"
                );
                return;
            }
            next_wave_multiplier = DUXU_HEART_DEMON_OBSESSION_NEXT_WAVE_MULTIPLIER;
        }
        HeartDemonOutcome::NoSolution => {}
    }
    if let Some(mut life_record) = life_record {
        life_record.push(BiographyEntry::HeartDemonRecord {
            outcome,
            choice_idx: decision.choice_idx,
            tick: decision.tick,
        });
    }
    commands
        .entity(decision.entity)
        .insert(HeartDemonResolution {
            outcome,
            choice_idx: decision.choice_idx,
            tick: decision.tick,
            next_wave_multiplier,
        });
}

fn heart_demon_outcome_for_choice(choice_idx: Option<u32>) -> HeartDemonOutcome {
    match choice_idx {
        Some(0) => HeartDemonOutcome::Steadfast,
        Some(2) => HeartDemonOutcome::NoSolution,
        _ => HeartDemonOutcome::Obsession,
    }
}

fn du_xu_wave_profile(wave: u32) -> DuXuWaveProfile {
    DuXuWaveProfile {
        strikes: if wave == DUXU_CHAIN_LIGHTNING_WAVE {
            DUXU_CHAIN_LIGHTNING_STRIKES
        } else {
            1
        },
        damage: DUXU_AOE_DAMAGE_BASE * wave as f32,
        qi_drain: DUXU_QI_DRAIN_BASE * f64::from(wave),
        qi_max_freeze_ratio: if wave == 3 {
            DUXU_SOUL_DEVOUR_QI_MAX_FREEZE_RATIO
        } else {
            0.0
        },
        requires_full_resources: wave == DUXU_KAITIAN_WAVE,
    }
}

fn has_full_tribulation_resources(cultivation: &Cultivation, wounds: &Wounds) -> bool {
    let effective_qi_max = (cultivation.qi_max - cultivation.qi_max_frozen.unwrap_or(0.0)).max(0.0);
    wounds.health_current + DUXU_FULL_HEALTH_EPSILON >= wounds.health_max
        && cultivation.qi_current + DUXU_FULL_QI_EPSILON >= effective_qi_max
}

#[allow(clippy::type_complexity)]
pub fn record_tribulation_interceptor_system(
    mut combat_events: EventReader<CombatEvent>,
    mut tribulators: Query<(
        &mut TribulationState,
        &Lifecycle,
        Option<&CurrentDimension>,
        Option<&TribulationOriginDimension>,
    )>,
    actors: Query<(&Lifecycle, &Position, Option<&CurrentDimension>)>,
) {
    for event in combat_events.read() {
        let Ok((mut state, target_lifecycle, target_dimension, origin_dimension)) =
            tribulators.get_mut(event.target)
        else {
            continue;
        };
        if state.kind != TribulationKind::DuXu
            || !matches!(
                state.phase,
                TribulationPhase::Lock | TribulationPhase::Wave(_) | TribulationPhase::HeartDemon
            )
        {
            continue;
        }
        if state
            .participants
            .first()
            .is_some_and(|participant| participant != &target_lifecycle.character_id)
        {
            continue;
        }
        let Ok((attacker_lifecycle, attacker_position, attacker_dimension)) =
            actors.get(event.attacker)
        else {
            continue;
        };
        if attacker_lifecycle.character_id == target_lifecycle.character_id
            || !attacker_lifecycle.character_id.starts_with("offline:")
        {
            continue;
        }
        let tribulation_dimension =
            active_tribulation_dimension(origin_dimension, target_dimension);
        if tribulation_dimension_for_participant(target_dimension) != tribulation_dimension
            || tribulation_dimension_for_participant(attacker_dimension) != tribulation_dimension
        {
            continue;
        }
        let center =
            valence::math::DVec3::new(state.epicenter[0], state.epicenter[1], state.epicenter[2]);
        if attacker_position.get().distance(center) > DUXU_LOCK_RADIUS_HARD {
            continue;
        }
        state.ensure_primary_tribulator(&target_lifecycle.character_id);
        if state.record_interceptor(&attacker_lifecycle.character_id) {
            tracing::info!(
                "[bong][cultivation] {} entered DuXu interception against {}",
                attacker_lifecycle.character_id,
                target_lifecycle.character_id,
            );
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn tribulation_wave_system(
    settings: Res<PersistenceSettings>,
    mut cleared: EventReader<TribulationWaveCleared>,
    mut players: Query<(
        &mut Cultivation,
        &mut TribulationState,
        &MeridianSystem,
        &Lifecycle,
        Option<&TribulationOriginDimension>,
        Option<&JueBiRuntimeContext>,
        Option<&mut LifespanComponent>,
        Option<&JueBiAfterDuXuQuota>,
    )>,
    mut commands: Commands,
    mut skill_cap_events: EventWriter<SkillCapChanged>,
    mut settled: EventWriter<TribulationSettled>,
    mut quota_occupied: EventWriter<AscensionQuotaOccupied>,
    mut juebi_triggered: EventWriter<JueBiTriggeredEvent>,
) {
    for ev in cleared.read() {
        if let Ok((
            mut c,
            mut state,
            _,
            lifecycle,
            origin_dimension,
            runtime,
            lifespan,
            juebi_after_quota,
        )) = players.get_mut(ev.entity)
        {
            if state.failed {
                continue;
            }
            state.wave_current = state.wave_current.max(ev.wave);
            if state.kind == TribulationKind::JueBi {
                if let Err(error) = persist_active_state(
                    &settings,
                    lifecycle,
                    &state,
                    runtime,
                    origin_dimension.map(|origin| origin.0),
                ) {
                    tracing::warn!(
                        "[bong][cultivation] failed to update active JueBi for {:?}: {error}",
                        ev.entity,
                    );
                }
                continue;
            }
            if state.wave_current >= state.waves_total {
                if let Some(quota_marker) = juebi_after_quota {
                    let epicenter = state.epicenter;
                    let started_tick = state.phase_started_tick;
                    let dimension = active_tribulation_dimension(origin_dimension, None);
                    let intensity = juebi_intensity_for_quota_marker(quota_marker);
                    let next_state =
                        juebi_state(epicenter, started_tick, lifecycle.character_id.clone());
                    let next_runtime = JueBiRuntimeContext {
                        source: JueBiTriggerSource::VoidQuotaExceeded,
                        intensity,
                    };
                    if let Err(error) = persist_active_state(
                        &settings,
                        lifecycle,
                        &next_state,
                        Some(&next_runtime),
                        Some(dimension),
                    ) {
                        tracing::warn!(
                            "[bong][cultivation] failed to persist over-quota JueBi for {:?}: {error}",
                            ev.entity,
                        );
                        continue;
                    }
                    *state = next_state;
                    commands.entity(ev.entity).insert(next_runtime);
                    juebi_triggered.send(JueBiTriggeredEvent {
                        entity: ev.entity,
                        char_id: lifecycle.character_id.clone(),
                        actor_name: lifecycle.character_id.clone(),
                        source: JueBiTriggerSource::VoidQuotaExceeded,
                        epicenter,
                        dimension,
                        waves_total: JUEBI_WAVES_TOTAL,
                        started_tick,
                        intensity,
                    });
                    tracing::info!(
                        "[bong][cultivation] {:?} cleared over-quota DuXu; JueBi sequence appended",
                        ev.entity,
                    );
                    continue;
                }
                // 渡劫成功。先落库占用名额，再修改 ECS；否则 SQLite 失败会制造未持久化的化虚者。
                let quota = match complete_tribulation_ascension(
                    &settings,
                    lifecycle.character_id.as_str(),
                ) {
                    Ok(quota) => quota,
                    Err(error) => {
                        tracing::error!(
                                "[bong][cultivation] failed to finalize tribulation ascension for {:?}: {error}",
                                ev.entity,
                            );
                        continue;
                    }
                };
                quota_occupied.send(AscensionQuotaOccupied {
                    occupied_slots: quota.occupied_slots,
                });
                c.realm = Realm::Void;
                c.qi_max *= super::breakthrough::qi_max_multiplier(Realm::Void);
                if let Some(mut lifespan) = lifespan {
                    lifespan.apply_cap(LifespanCapTable::VOID);
                }
                // plan-skill-v1 §4：化虚 cap=10，全部 skill 解锁满级上限。
                let new_cap = skill_cap_for_realm(Realm::Void);
                for skill in SkillId::ALL {
                    skill_cap_events.send(SkillCapChanged {
                        char_entity: ev.entity,
                        skill,
                        new_cap,
                    });
                }
                settled.send(TribulationSettled {
                    entity: ev.entity,
                    kind: TribulationKind::DuXu,
                    source: None,
                    result: DuXuResultV1 {
                        char_id: lifecycle.character_id.clone(),
                        outcome: DuXuOutcomeV1::Ascended,
                        killer: None,
                        waves_survived: state.waves_total,
                        reason: None,
                    },
                });
                state.phase = TribulationPhase::Settle;
                commands.entity(ev.entity).remove::<(
                    TribulationState,
                    TribulationOriginDimension,
                    HeartDemonResolution,
                    PendingHeartDemonOffer,
                )>();
                tracing::info!(
                    "[bong][cultivation] {:?} settled DuXu as {:?} after {} waves",
                    ev.entity,
                    DuXuOutcomeV1::Ascended,
                    state.waves_total
                );
            } else if let Err(error) = persist_active_state(
                &settings,
                lifecycle,
                &state,
                None,
                origin_dimension.map(|origin| origin.0),
            ) {
                tracing::warn!(
                    "[bong][cultivation] failed to update active tribulation for {:?}: {error}",
                    ev.entity,
                );
            }
        }
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn tribulation_failure_system(
    settings: Res<PersistenceSettings>,
    clock: Option<Res<CombatClock>>,
    mut failed: EventReader<TribulationFailed>,
    mut players: Query<(
        &mut Cultivation,
        Option<&mut MeridianSystem>,
        &Lifecycle,
        Option<&mut Wounds>,
        Option<&mut TribulationState>,
        Option<&Position>,
        Option<&CurrentDimension>,
        Option<&LifeRecord>,
    )>,
    mut commands: Commands,
    mut settled: EventWriter<TribulationSettled>,
    mut severed_events: Option<ResMut<Events<MeridianSeveredEvent>>>,
    mut ledger: ResMut<WorldQiAccount>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
    mut zones: Option<ResMut<ZoneRegistry>>,
) {
    for ev in failed.read() {
        if let Ok((
            mut cultivation,
            meridians,
            lifecycle,
            wounds,
            state,
            position,
            current_dimension,
            life_record,
        )) = players.get_mut(ev.entity)
        {
            let released_qi = cultivation.qi_current.max(0.0);
            let release = release_qi_amount_to_zone(
                &mut cultivation,
                released_qi,
                position,
                current_dimension,
                life_record,
                zones.as_deref_mut(),
                &mut ledger,
                qi_transfers.as_deref_mut(),
                "tribulation_failure",
            );
            if let Err(error) = release {
                tracing::warn!(
                    ?error,
                    "[bong][cultivation] tribulation failure qi release failed closed"
                );
                continue;
            }
            if let Some(mut state) = state {
                state.failed = true;
                state.phase = TribulationPhase::Settle;
            }
            // plan-meridian-severed-v1 §4 #5：渡劫失败爆脉降境 → emit
            // MeridianSeveredEvent { TribulationFail } 让永久 SEVERED component 落档。
            // severed_events 用 Option<ResMut<Events<...>>> 以便测试 app 未注册 event 也能跑通。
            let (_, severed_ids) =
                apply_tribulation_failure_penalty(&mut cultivation, meridians, wounds);
            if let Some(ref mut sink) = severed_events {
                let now_tick = clock.as_deref().map(|c| c.tick).unwrap_or_default();
                for id in severed_ids {
                    sink.send(MeridianSeveredEvent {
                        entity: ev.entity,
                        meridian_id: id,
                        source: SeveredSource::TribulationFail,
                        at_tick: now_tick,
                    });
                }
            }
            if let Err(error) =
                delete_active_tribulation(&settings, lifecycle.character_id.as_str())
            {
                tracing::warn!(
                    "[bong][cultivation] failed to delete failed active tribulation for {:?}: {error}",
                    ev.entity,
                );
            }
            tracing::info!(
                "[bong][cultivation] {:?} failed tribulation at wave {}; regressed to Spirit without death lifecycle",
                ev.entity,
                ev.wave,
            );
            settled.send(TribulationSettled {
                entity: ev.entity,
                kind: TribulationKind::DuXu,
                source: None,
                result: DuXuResultV1 {
                    char_id: lifecycle.character_id.clone(),
                    outcome: DuXuOutcomeV1::Failed,
                    killer: None,
                    waves_survived: ev.wave.saturating_sub(1),
                    reason: None,
                },
            });
        }
        commands.entity(ev.entity).remove::<(
            TribulationState,
            TribulationOriginDimension,
            HeartDemonResolution,
            PendingHeartDemonOffer,
        )>();
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn abort_du_xu_on_client_removed(
    clock: Res<CombatClock>,
    settings: Res<PersistenceSettings>,
    mut removed_clients: RemovedComponents<Client>,
    mut players: Query<(
        &mut Cultivation,
        Option<&mut MeridianSystem>,
        &Lifecycle,
        Option<&mut Wounds>,
        &mut TribulationState,
        Option<&mut LifeRecord>,
        Option<&Position>,
        Option<&CurrentDimension>,
    )>,
    mut commands: Commands,
    mut settled: EventWriter<TribulationSettled>,
    mut fled: EventWriter<TribulationFled>,
    mut severed_events: Option<ResMut<Events<MeridianSeveredEvent>>>,
    mut ledger: ResMut<WorldQiAccount>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
    mut zones: Option<ResMut<ZoneRegistry>>,
) {
    for entity in removed_clients.read() {
        let Ok((
            mut cultivation,
            meridians,
            lifecycle,
            wounds,
            mut state,
            life_record,
            position,
            current_dimension,
        )) = players.get_mut(entity)
        else {
            continue;
        };
        // abort_as_fled 语义：断线 = 逃劫（DuXu 已有此路径；JueBi 同样适用，
        // 防止玩家在绝壁劫压力下主动断线 exploit + 保持两种劫型一致性）。
        // ZoneCollapse / Targeted 不绑定玩家身份，不处理。
        if !matches!(state.kind, TribulationKind::DuXu | TribulationKind::JueBi) {
            continue;
        }
        settle_fled_tribulation(
            entity,
            clock.tick,
            &settings,
            &mut commands,
            &mut cultivation,
            meridians,
            lifecycle,
            wounds,
            &mut state,
            life_record,
            &mut settled,
            &mut fled,
            severed_events.as_deref_mut(),
            &mut ledger,
            qi_transfers.as_deref_mut(),
            zones.as_deref_mut(),
            position,
            current_dimension,
        );
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn tribulation_escape_boundary_system(
    clock: Res<CombatClock>,
    settings: Res<PersistenceSettings>,
    mut players: Query<(
        Entity,
        &Position,
        &mut Cultivation,
        Option<&mut MeridianSystem>,
        &Lifecycle,
        Option<&mut Wounds>,
        &mut TribulationState,
        Option<&CurrentDimension>,
        Option<&TribulationOriginDimension>,
        Option<&mut LifeRecord>,
    )>,
    mut commands: Commands,
    mut settled: EventWriter<TribulationSettled>,
    mut fled: EventWriter<TribulationFled>,
    mut severed_events: Option<ResMut<Events<MeridianSeveredEvent>>>,
    mut ledger: ResMut<WorldQiAccount>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
    mut zones: Option<ResMut<ZoneRegistry>>,
) {
    for (
        entity,
        position,
        mut cultivation,
        meridians,
        lifecycle,
        wounds,
        mut state,
        current_dimension,
        origin_dimension,
        life_record,
    ) in &mut players
    {
        if state.kind != TribulationKind::DuXu || matches!(state.phase, TribulationPhase::Omen) {
            continue;
        }
        let tribulation_dimension =
            active_tribulation_dimension(origin_dimension, current_dimension);
        if tribulation_dimension_for_participant(current_dimension) != tribulation_dimension {
            settle_fled_tribulation(
                entity,
                clock.tick,
                &settings,
                &mut commands,
                &mut cultivation,
                meridians,
                lifecycle,
                wounds,
                &mut state,
                life_record,
                &mut settled,
                &mut fled,
                severed_events.as_deref_mut(),
                &mut ledger,
                qi_transfers.as_deref_mut(),
                zones.as_deref_mut(),
                Some(position),
                current_dimension,
            );
            continue;
        }
        let center =
            valence::math::DVec3::new(state.epicenter[0], state.epicenter[1], state.epicenter[2]);
        if position.get().distance(center) <= state.lock_radius(clock.tick) {
            continue;
        }
        settle_fled_tribulation(
            entity,
            clock.tick,
            &settings,
            &mut commands,
            &mut cultivation,
            meridians,
            lifecycle,
            wounds,
            &mut state,
            life_record,
            &mut settled,
            &mut fled,
            severed_events.as_deref_mut(),
            &mut ledger,
            qi_transfers.as_deref_mut(),
            zones.as_deref_mut(),
            Some(position),
            current_dimension,
        );
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
fn settle_fled_tribulation(
    entity: Entity,
    fled_tick: u64,
    settings: &PersistenceSettings,
    commands: &mut Commands,
    cultivation: &mut Cultivation,
    meridians: Option<valence::prelude::Mut<'_, MeridianSystem>>,
    lifecycle: &Lifecycle,
    wounds: Option<valence::prelude::Mut<'_, Wounds>>,
    state: &mut TribulationState,
    mut life_record: Option<valence::prelude::Mut<'_, LifeRecord>>,
    settled: &mut EventWriter<TribulationSettled>,
    fled: &mut EventWriter<TribulationFled>,
    severed_events: Option<&mut Events<MeridianSeveredEvent>>,
    ledger: &mut WorldQiAccount,
    qi_transfers: Option<&mut Events<QiTransfer>>,
    zones: Option<&mut ZoneRegistry>,
    position: Option<&Position>,
    current_dimension: Option<&CurrentDimension>,
) {
    // 先结算真元；失败时不提交降境、断脉、传记、事件或删除 active record。
    let released_qi = cultivation.qi_current.max(0.0);
    let release = release_qi_amount_to_zone(
        cultivation,
        released_qi,
        position,
        current_dimension,
        life_record.as_deref(),
        zones,
        ledger,
        qi_transfers,
        "tribulation_fled",
    );
    if let Err(error) = release {
        tracing::warn!(
            ?error,
            "[bong][cultivation] fled tribulation qi release failed closed"
        );
        return;
    }

    state.failed = true;
    state.phase = TribulationPhase::Settle;
    let waves_survived = state.wave_current;
    if let Some(life_record) = life_record.as_deref_mut() {
        life_record.push(BiographyEntry::TribulationFled {
            wave: waves_survived.saturating_add(1),
            tick: fled_tick,
        });
    }
    // plan-meridian-severed-v1 §4 #5：渡劫逃跑也算失败，关闭的经脉同样写永久 SEVERED
    let (_, severed_ids) = apply_tribulation_failure_penalty(cultivation, meridians, wounds);
    if let Some(sink) = severed_events {
        for id in severed_ids {
            sink.send(MeridianSeveredEvent {
                entity,
                meridian_id: id,
                source: SeveredSource::TribulationFail,
                at_tick: fled_tick,
            });
        }
    }
    if let Err(error) = delete_active_tribulation(settings, lifecycle.character_id.as_str()) {
        tracing::warn!(
            "[bong][cultivation] failed to delete fled active tribulation for {:?}: {error}",
            entity,
        );
    }
    settled.send(TribulationSettled {
        entity,
        // 使用 state.kind 而非硬编码 DuXu，避免 JueBi 断线时下游（halfstep metrics 等）
        // 收到错误 kind 的 TribulationSettled 事件。
        kind: state.kind,
        source: None,
        result: DuXuResultV1 {
            char_id: lifecycle.character_id.clone(),
            outcome: DuXuOutcomeV1::Fled,
            killer: None,
            waves_survived,
            reason: None,
        },
    });
    fled.send(TribulationFled {
        entity,
        tick: fled_tick,
    });
    commands.entity(entity).remove::<(
        TribulationState,
        TribulationOriginDimension,
        HeartDemonResolution,
        PendingHeartDemonOffer,
    )>();
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn tribulation_intercept_death_system(
    mut deaths: EventReader<DeathEvent>,
    mut commands: Commands,
    settings: Res<PersistenceSettings>,
    item_registry: Res<ItemRegistry>,
    mut q: Query<(&TribulationState, &Lifecycle), Without<crate::npc::spawn::NpcMarker>>,
    mut inventories: Query<&mut PlayerInventory>,
    mut life_records: Query<&mut LifeRecord>,
    mut settled: EventWriter<TribulationSettled>,
) {
    for death in deaths.read() {
        let Ok((state, lifecycle)) = q.get_mut(death.target) else {
            continue;
        };
        let Some(killer_id) = death.attacker_player_id.as_deref() else {
            continue;
        };
        if !state
            .participants
            .iter()
            .any(|participant| participant == killer_id)
        {
            continue;
        }
        if let Err(error) = delete_active_tribulation(&settings, lifecycle.character_id.as_str()) {
            tracing::warn!(
                "[bong][cultivation] failed to clear intercepted tribulation for {:?}: {error}",
                death.target,
            );
        }
        if let Some(killer_entity) = death.attacker.filter(|attacker| *attacker != death.target) {
            let loot_outcome = inventories
                .get_many_mut([death.target, killer_entity])
                .ok()
                .map(|[mut victim_inventory, mut killer_inventory]| {
                    transfer_all_inventory_contents(
                        &mut victim_inventory,
                        &mut killer_inventory,
                        &item_registry,
                    )
                });
            if let Some(outcome) = loot_outcome {
                tracing::info!(
                    "[bong][cultivation] {:?} intercepted DuXu target {:?}; transferred {} item(s), {} bone coin(s)",
                    killer_entity,
                    death.target,
                    outcome.items_moved,
                    outcome.bone_coins_moved,
                );
            }
            if let Ok(mut life_record) = life_records.get_mut(killer_entity) {
                life_record.push(BiographyEntry::TribulationIntercepted {
                    victim_id: lifecycle.character_id.clone(),
                    tag: "戮道者 · 截劫".to_string(),
                    tick: death.at_tick,
                });
            }
        }
        settled.send(TribulationSettled {
            entity: death.target,
            kind: TribulationKind::DuXu,
            source: None,
            result: DuXuResultV1 {
                char_id: lifecycle.character_id.clone(),
                outcome: DuXuOutcomeV1::Killed,
                killer: Some(killer_id.to_string()),
                waves_survived: state.wave_current,
                reason: None,
            },
        });
        commands.entity(death.target).remove::<(
            TribulationState,
            TribulationOriginDimension,
            HeartDemonResolution,
            PendingHeartDemonOffer,
        )>();
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn publish_tribulation_events(
    redis: Res<RedisBridgeResource>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
    mut announce: EventReader<TribulationAnnounce>,
    mut juebi_triggered: EventReader<JueBiTriggeredEvent>,
    mut locked: EventReader<TribulationLocked>,
    mut cleared: EventReader<TribulationWaveCleared>,
    mut settled: EventReader<TribulationSettled>,
    mut quota_opened: EventReader<AscensionQuotaOpened>,
    states: Query<(
        &TribulationState,
        Option<&Lifecycle>,
        Option<&Username>,
        Option<&JueBiRuntimeContext>,
    )>,
    actors: Query<(Option<&Lifecycle>, Option<&Username>)>,
) {
    for ev in announce.read() {
        let payload = TribulationEventV1::du_xu(
            TribulationPhaseV1::Omen,
            Some(ev.char_id.clone()),
            Some(ev.actor_name.clone()),
            Some(ev.epicenter),
            Some(0),
            Some(ev.waves_total),
            None,
        );
        let _ = redis
            .tx_outbound
            .send(crate::network::redis_bridge::RedisOutbound::TribulationEvent(payload));
    }
    for ev in juebi_triggered.read() {
        let (char_id, actor_name) = actors
            .get(ev.entity)
            .ok()
            .map(|(lifecycle, username)| {
                let char_id = lifecycle.map(|lifecycle| lifecycle.character_id.clone());
                let actor_name = username
                    .map(|name| name.0.clone())
                    .or_else(|| char_id.clone());
                (char_id, actor_name)
            })
            .unwrap_or((Some(ev.char_id.clone()), Some(ev.actor_name.clone())));
        let payload = TribulationEventV1::jue_bi(
            TribulationPhaseV1::Omen,
            char_id,
            actor_name,
            Some(ev.source.wire_name().to_string()),
            Some(ev.epicenter),
            Some(0),
            Some(ev.waves_total),
            None,
        );
        let _ = redis
            .tx_outbound
            .send(crate::network::redis_bridge::RedisOutbound::TribulationEvent(payload));
    }
    for ev in locked.read() {
        let payload = TribulationEventV1::du_xu(
            TribulationPhaseV1::Lock,
            Some(ev.char_id.clone()),
            Some(ev.actor_name.clone()),
            Some(ev.epicenter),
            Some(0),
            Some(ev.waves_total),
            None,
        );
        let _ = redis
            .tx_outbound
            .send(crate::network::redis_bridge::RedisOutbound::TribulationEvent(payload));
    }
    for ev in cleared.read() {
        let Ok((state, lifecycle, username, runtime)) = states.get(ev.entity) else {
            continue;
        };
        let char_id = lifecycle
            .map(|lifecycle| lifecycle.character_id.clone())
            .or_else(|| state.participants.first().cloned());
        let actor_name = username
            .map(|name| name.0.clone())
            .or_else(|| char_id.clone());
        let phase = if matches!(state.phase, TribulationPhase::HeartDemon) {
            TribulationPhaseV1::HeartDemon
        } else {
            TribulationPhaseV1::Wave { wave: ev.wave }
        };
        let payload = match state.kind {
            TribulationKind::JueBi => TribulationEventV1::jue_bi(
                phase,
                char_id,
                actor_name,
                runtime.map(|runtime| runtime.source.wire_name().to_string()),
                Some(state.epicenter),
                Some(ev.wave),
                Some(state.waves_total),
                None,
            ),
            _ => TribulationEventV1::du_xu(
                phase,
                char_id,
                actor_name,
                Some(state.epicenter),
                Some(ev.wave),
                Some(state.waves_total),
                None,
            ),
        };
        let _ = redis
            .tx_outbound
            .send(crate::network::redis_bridge::RedisOutbound::TribulationEvent(payload));
    }
    for ev in settled.read() {
        let actor_name = actors
            .get(ev.entity)
            .ok()
            .and_then(|(lifecycle, username)| {
                username
                    .map(|name| name.0.clone())
                    .or_else(|| lifecycle.map(|lifecycle| lifecycle.character_id.clone()))
            });
        let payload = if ev.kind == TribulationKind::JueBi {
            TribulationEventV1::jue_bi(
                TribulationPhaseV1::Settle,
                Some(ev.result.char_id.clone()),
                actor_name,
                ev.source.map(|source| source.wire_name().to_string()),
                None,
                Some(ev.result.waves_survived),
                None,
                Some(ev.result.clone()),
            )
        } else {
            TribulationEventV1::du_xu(
                TribulationPhaseV1::Settle,
                Some(ev.result.char_id.clone()),
                actor_name,
                None,
                Some(ev.result.waves_survived),
                None,
                Some(ev.result.clone()),
            )
        };
        let _ = redis
            .tx_outbound
            .send(crate::network::redis_bridge::RedisOutbound::TribulationEvent(payload));
    }
    for ev in quota_opened.read() {
        let payload = TribulationEventV1::ascension_quota_open(Some(ev.occupied_slots));
        let _ = redis
            .tx_outbound
            .send(crate::network::redis_bridge::RedisOutbound::TribulationEvent(payload));
        // plan-halfstep-rechallenge-integration-v1 P1：名额空出时广播音效。
        // 消除 halfstep_quota_release_broadcast recipe 的死资产状态。
        audio.send(PlaySoundRecipeRequest {
            recipe_id: HALFSTEP_QUOTA_RELEASE_BROADCAST_AUDIO_RECIPE.to_string(),
            instance_id: 0,
            pos: None,
            flag: None,
            volume_mul: 1.0,
            pitch_shift: 0.0,
            recipient: AudioRecipient::All,
        });
    }
}

const HEART_DEMON_RECENT_BIO_N: usize = 12;

#[allow(clippy::type_complexity)]
pub fn publish_heart_demon_pregen_requests(
    redis: Res<RedisBridgeResource>,
    mut announce: EventReader<TribulationAnnounce>,
    players: Query<(Option<&Cultivation>, Option<&QiColor>, Option<&LifeRecord>)>,
) {
    for ev in announce.read() {
        if ev.waves_total < DUXU_HEART_DEMON_WAVE {
            continue;
        }
        let (cultivation, qi_color, life_record) =
            players.get(ev.entity).unwrap_or((None, None, None));
        let payload = HeartDemonPregenRequestV1 {
            trigger_id: heart_demon_trigger_id(ev.entity.index(), ev.started_tick),
            character_id: ev.char_id.clone(),
            actor_name: ev.actor_name.clone(),
            realm: cultivation
                .map(|cultivation| realm_to_string(cultivation.realm).to_string())
                .unwrap_or_else(|| realm_to_string(Realm::Spirit).to_string()),
            qi_color_state: qi_color_state_for_request(qi_color),
            recent_biography: life_record
                .map(|record| {
                    record
                        .recent_summary(HEART_DEMON_RECENT_BIO_N)
                        .iter()
                        .map(|entry| format!("{entry:?}"))
                        .collect()
                })
                .unwrap_or_default(),
            composure: cultivation
                .map(|cultivation| cultivation.composure)
                .unwrap_or(0.5),
            started_tick: ev.started_tick,
            waves_total: ev.waves_total,
        };
        let _ = redis
            .tx_outbound
            .send(crate::network::redis_bridge::RedisOutbound::HeartDemonRequest(payload));
    }
}

fn heart_demon_trigger_id(entity_index: u32, started_tick: u64) -> String {
    format!("heart_demon:{entity_index}:{started_tick}")
}

fn qi_color_state_for_request(qi_color: Option<&QiColor>) -> QiColorStateV1 {
    let default_qi_color = QiColor::default();
    let qi_color = qi_color.unwrap_or(&default_qi_color);
    QiColorStateV1 {
        main: color_kind_to_string(qi_color.main).to_string(),
        secondary: qi_color
            .secondary
            .map(|color| color_kind_to_string(color).to_string()),
        is_chaotic: qi_color.is_chaotic,
        is_hunyuan: qi_color.is_hunyuan,
    }
}

pub fn du_xu_prereqs_met(cultivation: &Cultivation, meridians: &MeridianSystem) -> bool {
    cultivation.realm == Realm::Spirit
        && meridians.iter().all(|meridian| meridian.opened)
        && meridians.opened_count() >= Realm::Void.required_meridians()
}

fn du_xu_waves_total(requested_at_tick: u64, life_record: Option<&LifeRecord>) -> u32 {
    if life_record.is_some_and(|record| {
        du_xu_full_progress_ticks(record, requested_at_tick) >= DUXU_FULL_PROGRESS_MIN_TICKS
    }) {
        DUXU_MAX_WAVES
    } else {
        DUXU_DEFAULT_WAVES
    }
}

fn du_xu_full_progress_ticks(record: &LifeRecord, requested_at_tick: u64) -> u64 {
    let Some(spirit_tick) = latest_spirit_breakthrough_tick(record) else {
        return 0;
    };
    let Some(full_meridians_tick) = full_meridians_opened_tick(record) else {
        return 0;
    };
    requested_at_tick.saturating_sub(spirit_tick.max(full_meridians_tick))
}

fn latest_spirit_breakthrough_tick(record: &LifeRecord) -> Option<u64> {
    record.biography.iter().rev().find_map(|entry| match entry {
        BiographyEntry::BreakthroughSucceeded { realm, tick } if *realm == Realm::Spirit => {
            Some(*tick)
        }
        _ => None,
    })
}

fn full_meridians_opened_tick(record: &LifeRecord) -> Option<u64> {
    let mut opened: Vec<(MeridianId, u64)> = Vec::new();
    let mut full_tick = None;
    for entry in &record.biography {
        match entry {
            BiographyEntry::MeridianOpened { id, tick } => {
                if let Some((_, opened_tick)) =
                    opened.iter_mut().find(|(opened_id, _)| opened_id == id)
                {
                    *opened_tick = *tick;
                } else {
                    opened.push((*id, *tick));
                }
            }
            BiographyEntry::MeridianClosed { id, .. } => {
                opened.retain(|(opened_id, _)| opened_id != id);
                full_tick = None;
            }
            _ => {}
        }
        if opened.len() >= Realm::Void.required_meridians() {
            full_tick = opened.iter().map(|(_, tick)| *tick).max();
        }
    }
    if opened.len() >= Realm::Void.required_meridians() {
        full_tick
    } else {
        None
    }
}

fn apply_tribulation_failure_penalty(
    cultivation: &mut Cultivation,
    meridians: Option<valence::prelude::Mut<'_, MeridianSystem>>,
    wounds: Option<valence::prelude::Mut<'_, Wounds>>,
) -> (f64, Vec<MeridianId>) {
    let released_qi = cultivation.qi_current.max(0.0);
    cultivation.realm = Realm::Spirit;
    cultivation.qi_current = 0.0;
    cultivation.last_qi_zero_at = None;
    cultivation.pending_material_bonus = 0.0;

    let mut severed_meridians: Vec<MeridianId> = Vec::new();
    if let Some(mut meridians) = meridians {
        let keep = Realm::Spirit.required_meridians();
        let closures = pick_closures(&meridians, keep);
        for (is_regular, idx) in closures {
            // plan-race-system-v1 P1a：`Meridian.id` 已换轨为 `MeridianChannelId`，
            // 本函数返回值仍是 legacy `Vec<MeridianId>`（wire 开放化留待后续 P1 子阶段）
            // ——humanoid 20 条经脉均可逆映射回 `MeridianId`。
            let channel_id = if is_regular {
                let m = &mut meridians.regular[idx];
                let channel_id = m.id.clone();
                close_meridian(m);
                channel_id
            } else {
                let m = &mut meridians.extraordinary[idx];
                let channel_id = m.id.clone();
                close_meridian(m);
                channel_id
            };
            let id = channel_id.to_meridian_id().unwrap_or_else(|| {
                panic!(
                    "[bong][cultivation][tribulation] channel id {channel_id} has no legacy \
                     MeridianId mapping — apply_tribulation_failure_penalty cannot represent \
                     non-humanoid channels yet"
                )
            });
            severed_meridians.push(id);
        }
        cultivation.qi_max = 10.0 + meridians.sum_capacity();
        // 收敛 qi_max_frozen 到新 qi_max*0.5，避免 effective_max 变负锁死真元回复
        // （与 qi_zero_decay / breakthrough 同一不变量）。
        if let Some(frozen) = cultivation.qi_max_frozen {
            cultivation.qi_max_frozen =
                Some(frozen.min(
                    cultivation.qi_max * super::breakthrough::BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO,
                ));
        }
    }

    if let Some(mut wounds) = wounds {
        let floor = (wounds.health_max.max(1.0) * 0.05).max(1.0);
        wounds.health_current = wounds
            .health_current
            .max(floor)
            .min(wounds.health_max.max(1.0));
    }
    (released_qi, severed_meridians)
}

#[cfg(test)]
#[path = "tribulation_tests.rs"]
mod tests;
