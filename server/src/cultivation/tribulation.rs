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
    Resource, Username, With,
};

use std::collections::{HashSet, VecDeque};

use crate::combat::components::{BodyPart, Lifecycle, LifecycleState, Wound, WoundKind, Wounds};
use crate::combat::events::{CombatEvent, DeathEvent};
use crate::combat::CombatClock;
use crate::cultivation::death_hooks::CultivationDeathTrigger;
use crate::cultivation::life_record::{BiographyEntry, HeartDemonOutcome, LifeRecord};
use crate::cultivation::lifespan::{LifespanCapTable, LifespanComponent};
use crate::inventory::{transfer_all_inventory_contents, PlayerInventory};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::network::RedisBridgeResource;
use crate::qi_physics::{
    constants::DEFAULT_SPIRIT_QI_TOTAL, EnvField, QiAccountId, QiTransfer, QiTransferReason,
    WorldQiBudget,
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
use super::components::{Cultivation, MeridianId, MeridianSystem, QiColor, Realm};
use super::death_hooks::release_qi_amount_to_zone;
use super::meridian::severed::{MeridianSeveredEvent, SeveredSource};
use super::qi_zero_decay::{close_meridian, pick_closures};
use crate::persistence::{
    complete_tribulation_ascension, delete_active_tribulation, load_active_tribulation_count,
    load_ascension_quota, persist_active_tribulation, try_complete_tribulation_ascension,
    ActiveTribulationRecord, AtomicAscensionOutcome, PersistenceSettings,
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
const DUXU_AOE_DAMAGE_BASE: f32 = 18.0;
const DUXU_QI_DRAIN_BASE: f64 = 35.0;
const DUXU_CHAIN_LIGHTNING_WAVE: u32 = 2;
const DUXU_CHAIN_LIGHTNING_STRIKES: u32 = 3;
const DUXU_SOUL_DEVOUR_QI_MAX_FREEZE_RATIO: f64 = 0.20;
pub const DUXU_HEART_DEMON_WAVE: u32 = 4;
pub const DUXU_HEART_DEMON_TIMEOUT_TICKS: u64 = 30 * 20;
const DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO: f64 = 0.30;
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

pub const DEFAULT_VOID_QUOTA_K: f64 = DEFAULT_SPIRIT_QI_TOTAL / 2.0;
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

#[derive(Debug, Clone, Copy, Event)]
pub struct JueBiTriggeredEvent {
    pub entity: Entity,
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
        let _actor_name = username
            .map(|username| username.0.clone())
            .unwrap_or_else(|| lifecycle.character_id.clone());
        triggered.send(JueBiTriggeredEvent {
            entity: item.entity,
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
    )>,
    mut failed: EventWriter<TribulationFailed>,
    mut deaths: EventWriter<DeathEvent>,
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
        for (entity, pos, current_dimension, mut cultivation, mut wounds, lifecycle) in &mut targets
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
            cultivation.qi_current = (cultivation.qi_current - profile.qi_drain).max(0.0);
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
                    location: BodyPart::Chest,
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

        for (entity, position, target_dimension, mut cultivation, wounds, lifecycle) in &mut targets
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
                    commands.entity(entity).insert(JueBiPressureCollapse {
                        epicenter: epicenter_block,
                        phase_start_tick: state.phase_started_tick,
                        distance,
                    });
                    let before = cultivation.qi_current;
                    cultivation.qi_current = (cultivation.qi_current
                        * (1.0 - JUEBI_PRESSURE_DRAIN_PER_TICK * intensity_scale * factor))
                        .max(0.0);
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
                    commands.entity(entity).insert(JueBiNullified {
                        entered_tick: clock.tick,
                        accumulated_null_time: clock.tick.saturating_sub(state.phase_started_tick)
                            as f64,
                    });
                    let decay = juebi_null_decay_for_realm(cultivation.realm) * intensity_scale;
                    if decay <= 0.0 {
                        continue;
                    }
                    let before = cultivation.qi_current;
                    cultivation.qi_current = (cultivation.qi_current * (1.0 - decay)).max(0.0);
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
        location: BodyPart::Chest,
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
            if outcome.granted {
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
            } else {
                tracing::info!(
                    "[bong][cultivation] {:?} ascension denied at settle by atomic quota check \
                     (occupied_before={} limit={}); falling back to HalfStep",
                    entity,
                    outcome.occupied_before,
                    outcome.limit_used,
                );
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

/// plan-halfstep-buff-v1 P0+P1：消费 `TribulationSettled` 累计 halfstep / ascended 计数；
/// 首次 HalfStep outcome 时应用 buff (qi_max ×1.10 / lifespan +200) + qi_physics ledger 标记 +
/// 插入 `HalfStepState`。`HalfStepState.buff_applied` 守卫保证多次 HalfStep 只 apply 一次（§8 Q4）。
///
/// 设计要点：
/// - **buff_applied=false** 仅出现在 entity 缺 `Cultivation` 的测试场景或 dormant NPC 未 hydrate 时；
///   生产路径下 player / hydrated NPC 都有 Cultivation，应用即转为 `buff_applied=true`
/// - **ledger 是 audit-only**：emit `QiTransfer` event 但不调 `WorldQiAccount::transfer`，因为
///   buff 是容量扩张不是真元搬运（worldview §三:78 + §二 守恒律）
/// - **不覆盖已有 HalfStepState**：保留 `entered_at` 不漂移（§8 Q1 重渡窗口起点稳定）
pub fn track_tribulation_metrics_system(
    mut commands: Commands,
    mut events: EventReader<TribulationSettled>,
    clock: Res<CombatClock>,
    mut metrics: ResMut<TribulationMetrics>,
    mut targets: Query<(
        &mut Cultivation,
        Option<&mut LifespanComponent>,
        Option<&HalfStepState>,
    )>,
    mut qi_transfers: EventWriter<QiTransfer>,
) {
    for ev in events.read() {
        match ev.result.outcome {
            DuXuOutcomeV1::HalfStep => {
                metrics.halfstep_count = metrics.halfstep_count.saturating_add(1);

                // 先快速读 existing state（不动 borrow）确定是否需要应用 buff
                let (already_buffed, has_state) = match targets.get(ev.entity) {
                    Ok((_, _, state)) => {
                        (state.map(|s| s.buff_applied).unwrap_or(false), state.is_some())
                    }
                    Err(_) => (false, false),
                };

                let mut buff_now_applied = already_buffed;
                if !already_buffed {
                    if let Ok((mut cultivation, lifespan, _)) = targets.get_mut(ev.entity) {
                        let before = cultivation.qi_max;
                        cultivation.qi_max *= 1.0 + HALFSTEP_QI_MAX_BONUS as f64;
                        let bonus_capacity = cultivation.qi_max - before;
                        // qi_physics ledger audit-only 标记
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
                }

                if !has_state {
                    commands.entity(ev.entity).insert(HalfStepState {
                        entered_at: clock.tick,
                        rechallenge_window_until: clock
                            .tick
                            .saturating_add(RECHALLENGE_WINDOW_TICKS),
                        buff_applied: buff_now_applied,
                    });
                }
            }
            DuXuOutcomeV1::Ascended => {
                metrics.ascended_count = metrics.ascended_count.saturating_add(1);
            }
            DuXuOutcomeV1::Killed | DuXuOutcomeV1::Failed | DuXuOutcomeV1::Fled => {}
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
    )>,
) {
    for choice in choices.read() {
        let Ok((mut cultivation, state, life_record, existing_resolution)) =
            players.get_mut(choice.entity)
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
    )>,
) {
    for (entity, mut cultivation, state, life_record, existing_resolution) in &mut players {
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
        );
    }
}

fn resolve_heart_demon_choice(
    decision: HeartDemonDecision,
    commands: &mut Commands,
    cultivation: &mut Cultivation,
    state: &TribulationState,
    life_record: Option<valence::prelude::Mut<'_, LifeRecord>>,
    existing_resolution: Option<&HeartDemonResolution>,
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
            cultivation.qi_current =
                (cultivation.qi_current + effective_qi_max * 0.10).min(effective_qi_max);
        }
        HeartDemonOutcome::Obsession => {
            cultivation.qi_current *= 1.0 - DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO;
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
            if let Some(mut state) = state {
                state.failed = true;
                state.phase = TribulationPhase::Settle;
            }
            // plan-meridian-severed-v1 §4 #5：渡劫失败爆脉降境 → emit
            // MeridianSeveredEvent { TribulationFail } 让永久 SEVERED component 落档。
            // severed_events 用 Option<ResMut<Events<...>>> 以便测试 app 未注册 event 也能跑通。
            let (released_qi, severed_ids) =
                apply_tribulation_failure_penalty(&mut cultivation, meridians, wounds);
            release_qi_amount_to_zone(
                ev.entity,
                released_qi,
                position,
                current_dimension,
                life_record,
                zones.as_deref_mut(),
                qi_transfers.as_deref_mut(),
                "tribulation_failure",
            );
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
        if state.kind != TribulationKind::DuXu {
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
    qi_transfers: Option<&mut Events<QiTransfer>>,
    zones: Option<&mut ZoneRegistry>,
    position: Option<&Position>,
    current_dimension: Option<&CurrentDimension>,
) {
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
    let (released_qi, severed_ids) =
        apply_tribulation_failure_penalty(cultivation, meridians, wounds);
    release_qi_amount_to_zone(
        entity,
        released_qi,
        position,
        current_dimension,
        life_record.as_deref(),
        zones,
        qi_transfers,
        "tribulation_fled",
    );
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
        kind: TribulationKind::DuXu,
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

#[allow(clippy::type_complexity)]
pub fn tribulation_intercept_death_system(
    mut deaths: EventReader<DeathEvent>,
    mut commands: Commands,
    settings: Res<PersistenceSettings>,
    mut q: Query<(&TribulationState, &Lifecycle)>,
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
                    transfer_all_inventory_contents(&mut victim_inventory, &mut killer_inventory)
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
            .unwrap_or((None, None));
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
            let id = if is_regular {
                let m = &mut meridians.regular[idx];
                let id = m.id;
                close_meridian(m);
                id
            } else {
                let m = &mut meridians.extraordinary[idx];
                let id = m.id;
                close_meridian(m);
                id
            };
            severed_meridians.push(id);
        }
        cultivation.qi_max = 10.0 + meridians.sum_capacity();
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
mod tests {
    use super::*;

    use crate::combat::components::{CombatState, Lifecycle, LifecycleState, Stamina, Wounds};
    use crate::combat::events::{CombatEvent, DeathEvent, DeathInsightRequested};
    use crate::combat::lifecycle::death_arbiter_tick;
    use crate::combat::CombatClock;
    use crate::cultivation::components::MeridianId;
    use crate::cultivation::death_hooks::{CultivationDeathTrigger, PlayerTerminated};
    use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
    use crate::cultivation::lifespan::{
        DeathRegistry, LifespanCapTable, LifespanComponent, ZoneDeathKind,
    };
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
        PlayerInventory, MAIN_PACK_CONTAINER_ID,
    };
    use crate::network::redis_bridge::RedisOutbound;
    use crate::network::vfx_event_emit::VfxEventRequest;
    use crate::network::RedisBridgeResource;
    use crate::persistence::{bootstrap_sqlite, load_active_tribulation};
    use crate::qi_physics::{QiAccountId, QiTransfer, QiTransferReason};
    use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use valence::prelude::{App, Entity, Events, IntoSystemConfigs, Position, Update, Username};
    use valence::testing::{create_mock_client, ScenarioSingleClient};

    fn unique_temp_dir(test_name: &str) -> PathBuf {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "bong-tribulation-{test_name}-{}-{unique_suffix}",
            std::process::id()
        ))
    }

    fn persistence_settings(test_name: &str) -> (PersistenceSettings, PathBuf) {
        let root = unique_temp_dir(test_name);
        let db_path = root.join("data").join("bong.db");
        let deceased_dir = root.join("library-web").join("public").join("deceased");
        bootstrap_sqlite(&db_path, &format!("tribulation-{test_name}"))
            .expect("sqlite bootstrap should succeed");
        (
            PersistenceSettings::with_paths(
                &db_path,
                &deceased_dir,
                format!("tribulation-{test_name}"),
            ),
            root,
        )
    }

    fn unbootstrapped_persistence_settings(test_name: &str) -> (PersistenceSettings, PathBuf) {
        let root = unique_temp_dir(test_name);
        let db_path = root.join("data").join("bong.db");
        let deceased_dir = root.join("library-web").join("public").join("deceased");
        (
            PersistenceSettings::with_paths(
                &db_path,
                &deceased_dir,
                format!("tribulation-unbootstrapped-{test_name}"),
            ),
            root,
        )
    }

    fn all_meridians_open() -> MeridianSystem {
        let mut meridians = MeridianSystem::default();
        for (idx, id) in MeridianId::REGULAR
            .iter()
            .chain(MeridianId::EXTRAORDINARY.iter())
            .enumerate()
        {
            let meridian = meridians.get_mut(*id);
            meridian.opened = true;
            meridian.open_progress = 1.0;
            meridian.opened_at = idx as u64;
        }
        meridians
    }

    fn spawn_tribulation_spectator(app: &mut App, name: &str, pos: [f64; 3]) -> Entity {
        app.world_mut()
            .spawn((
                Position::new(pos),
                CurrentDimension(DimensionKind::Overworld),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 200.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                Wounds {
                    health_current: 200.0,
                    health_max: 200.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: format!("offline:{name}"),
                    ..Default::default()
                },
            ))
            .id()
    }

    fn test_item(instance_id: u64) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: format!("test_item_{instance_id}"),
            display_name: format!("test {instance_id}"),
            grid_w: 1,
            grid_h: 1,
            weight: 0.5,
            rarity: ItemRarity::Common,
            description: "test".to_string(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn test_inventory(items: Vec<ItemInstance>, bone_coins: u64) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 5,
                items: items
                    .into_iter()
                    .enumerate()
                    .map(|(idx, instance)| PlacedItemState {
                        row: (idx / 5) as u8,
                        col: (idx % 5) as u8,
                        instance,
                    })
                    .collect(),
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins,
            max_weight: 50.0,
        }
    }

    fn full_progress_life_record(spirit_tick: u64, final_meridian_tick: u64) -> LifeRecord {
        let mut record = LifeRecord::new("offline:Azure");
        record.push(BiographyEntry::BreakthroughSucceeded {
            realm: Realm::Spirit,
            tick: spirit_tick,
        });
        let meridians: Vec<_> = MeridianId::REGULAR
            .iter()
            .chain(MeridianId::EXTRAORDINARY.iter())
            .copied()
            .collect();
        let count = meridians.len().saturating_sub(1) as u64;
        for (idx, id) in meridians.into_iter().enumerate() {
            record.push(BiographyEntry::MeridianOpened {
                id,
                tick: final_meridian_tick.saturating_sub(count.saturating_sub(idx as u64)),
            });
        }
        record
    }

    #[test]
    fn omen_to_lock_emits_lock_event() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: DUXU_OMEN_TICKS,
        });
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_systems(Update, tribulation_phase_tick_system);

        let entity = app
            .world_mut()
            .spawn((
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                Username("Azure".to_string()),
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Omen,
                    epicenter: [12.0, 66.0, -8.0],
                    wave_current: 0,
                    waves_total: 3,
                    started_tick: 0,
                    phase_started_tick: 0,
                    next_wave_tick: DUXU_OMEN_TICKS + DUXU_LOCK_TICKS,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
            ))
            .id();

        app.update();

        let state = app
            .world()
            .get::<TribulationState>(entity)
            .expect("tribulation should remain active");
        assert_eq!(state.phase, TribulationPhase::Lock);

        let events = app.world().resource::<Events<TribulationLocked>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].char_id, "offline:Azure");
        assert_eq!(emitted[0].actor_name, "Azure");
        assert_eq!(emitted[0].epicenter, [12.0, 66.0, -8.0]);
        assert_eq!(emitted[0].waves_total, 3);
    }

    #[test]
    fn start_tribulation_system_dedupes_same_tick_internal_events() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("start-tribulation-dedupe");
        app.insert_resource(settings);
        app.insert_resource(WorldQiBudget::from_total(100.0));
        app.insert_resource(VoidQuotaConfig::default());
        app.add_event::<InitiateXuhuaTribulation>();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationSettled>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, start_tribulation_system);
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 210.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
                CurrentDimension(DimensionKind::Tsy),
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
            ))
            .id();
        app.world_mut()
            .entity_mut(entity)
            .insert(Position::new([12.0, 66.0, -8.0]));

        for _ in 0..2 {
            app.world_mut().send_event(InitiateXuhuaTribulation {
                entity,
                waves_total: 3,
                started_tick: 100,
            });
        }
        app.update();

        let state = app
            .world()
            .get::<TribulationState>(entity)
            .expect("tribulation should start once");
        assert_eq!(state.phase, TribulationPhase::Omen);
        assert_eq!(state.started_tick, 100);
        let origin = app
            .world()
            .get::<TribulationOriginDimension>(entity)
            .expect("tribulation should remember origin dimension");
        assert_eq!(origin.0, DimensionKind::Tsy);
        let announce = app.world().resource::<Events<TribulationAnnounce>>();
        let emitted: Vec<_> = announce.get_reader().read(announce).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].entity, entity);
        assert_eq!(emitted[0].actor_name, "Azure");
        assert_eq!(app.world().resource::<Events<VfxEventRequest>>().len(), 1);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn start_tribulation_system_reserves_void_quota_fcfs_within_tick() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("start-tribulation-quota-fcfs");
        app.insert_resource(settings);
        app.insert_resource(WorldQiBudget::from_total(50.0));
        app.insert_resource(VoidQuotaConfig { quota_k: 50.0 });
        app.add_event::<InitiateXuhuaTribulation>();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationSettled>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, start_tribulation_system);

        let first = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 210.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                Position::new([12.0, 66.0, -8.0]),
            ))
            .id();
        let second = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 210.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
                Lifecycle {
                    character_id: "offline:Beryl".to_string(),
                    ..Default::default()
                },
                Position::new([16.0, 66.0, -8.0]),
            ))
            .id();

        app.world_mut().send_event(InitiateXuhuaTribulation {
            entity: first,
            waves_total: 3,
            started_tick: 100,
        });
        app.world_mut().send_event(InitiateXuhuaTribulation {
            entity: second,
            waves_total: 3,
            started_tick: 100,
        });

        app.update();

        let first_state = app
            .world()
            .get::<TribulationState>(first)
            .expect("first tribulation should start");
        assert_eq!(first_state.kind, TribulationKind::DuXu);
        let second_state = app
            .world()
            .get::<TribulationState>(second)
            .expect("second over-quota DuXu should start before appending JueBi");
        assert_eq!(second_state.kind, TribulationKind::DuXu);
        assert!(
            app.world().get::<JueBiAfterDuXuQuota>(second).is_some(),
            "second over-quota DuXu should carry a JueBi follow-up marker"
        );
        let settled = app.world().resource::<Events<TribulationSettled>>();
        let settled_events: Vec<_> = settled.get_reader().read(settled).cloned().collect();
        assert!(
            settled_events.is_empty(),
            "over-quota DuXu should no longer settle as instant quota death"
        );
        let death_triggers = app.world().resource::<Events<CultivationDeathTrigger>>();
        let deaths: Vec<_> = death_triggers
            .get_reader()
            .read(death_triggers)
            .cloned()
            .collect();
        assert!(
            deaths.is_empty(),
            "over-quota DuXu should no longer emit instant quota death"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn start_tribulation_system_counts_in_flight_void_tribulations_across_ticks() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("start-tribulation-quota-cross-tick");
        app.insert_resource(settings);
        app.insert_resource(WorldQiBudget::from_total(50.0));
        app.insert_resource(VoidQuotaConfig { quota_k: 50.0 });
        app.add_event::<InitiateXuhuaTribulation>();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationSettled>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, start_tribulation_system);

        let first = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 210.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                Position::new([12.0, 66.0, -8.0]),
            ))
            .id();
        let second = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 210.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
                Lifecycle {
                    character_id: "offline:Beryl".to_string(),
                    ..Default::default()
                },
                Position::new([16.0, 66.0, -8.0]),
            ))
            .id();

        app.world_mut().send_event(InitiateXuhuaTribulation {
            entity: first,
            waves_total: 3,
            started_tick: 100,
        });
        app.update();
        assert!(
            app.world().get::<TribulationState>(first).is_some(),
            "first in-flight tribulation should reserve the only void slot"
        );

        app.world_mut().send_event(InitiateXuhuaTribulation {
            entity: second,
            waves_total: 3,
            started_tick: 200,
        });
        app.update();

        assert!(
            app.world().get::<TribulationState>(second).is_some(),
            "later tick over-quota starter should enter DuXu and append JueBi at settlement"
        );
        assert!(
            app.world().get::<JueBiAfterDuXuQuota>(second).is_some(),
            "later tick over-quota starter should carry a JueBi follow-up marker"
        );
        let settled = app.world().resource::<Events<TribulationSettled>>();
        let settled_events: Vec<_> = settled.get_reader().read(settled).cloned().collect();
        assert!(
            settled_events.is_empty(),
            "over-quota DuXu should defer settlement until JueBi resolves"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn start_tribulation_system_fails_closed_when_quota_store_unreadable() {
        let mut app = App::new();
        let (settings, root) =
            unbootstrapped_persistence_settings("start-tribulation-quota-read-failure");
        app.insert_resource(settings);
        app.insert_resource(WorldQiBudget::from_total(100.0));
        app.insert_resource(VoidQuotaConfig::default());
        app.add_event::<InitiateXuhuaTribulation>();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationSettled>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, start_tribulation_system);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 210.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                Position::new([12.0, 66.0, -8.0]),
            ))
            .id();

        app.world_mut().send_event(InitiateXuhuaTribulation {
            entity,
            waves_total: 3,
            started_tick: 100,
        });
        app.update();

        assert!(
            app.world().get::<TribulationState>(entity).is_none(),
            "quota store read failure must not start or reserve an in-memory tribulation"
        );
        assert_eq!(
            app.world().resource::<Events<TribulationAnnounce>>().len(),
            0
        );
        assert_eq!(
            app.world().resource::<Events<TribulationSettled>>().len(),
            0
        );
        assert_eq!(
            app.world()
                .resource::<Events<CultivationDeathTrigger>>()
                .len(),
            0
        );
        assert_eq!(app.world().resource::<Events<VfxEventRequest>>().len(), 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn start_tribulation_system_aborts_when_active_row_persist_fails() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("start-tribulation-active-row-persist-failure");
        {
            let connection =
                rusqlite::Connection::open(settings.db_path()).expect("sqlite should open");
            connection
                .execute_batch(
                    "
                    DROP TABLE tribulations_active;
                    CREATE VIEW tribulations_active AS
                    SELECT
                        'offline:Existing' AS char_id,
                        0 AS wave_current,
                        3 AS waves_total,
                        0 AS started_tick,
                        1 AS schema_version,
                        0 AS last_updated_wall
                    WHERE 0;
                    ",
                )
                .expect("active tribulation view should be installed");
        }
        app.insert_resource(settings);
        app.insert_resource(WorldQiBudget::from_total(100.0));
        app.insert_resource(VoidQuotaConfig::default());
        app.add_event::<InitiateXuhuaTribulation>();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationSettled>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, start_tribulation_system);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 210.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                Position::new([12.0, 66.0, -8.0]),
            ))
            .id();

        app.world_mut().send_event(InitiateXuhuaTribulation {
            entity,
            waves_total: 3,
            started_tick: 100,
        });
        app.update();

        assert!(
            app.world().get::<TribulationState>(entity).is_none(),
            "active-row persist failure must not start an untracked tribulation"
        );
        assert_eq!(
            app.world().resource::<Events<TribulationAnnounce>>().len(),
            0
        );
        assert_eq!(
            app.world().resource::<Events<TribulationSettled>>().len(),
            0
        );
        assert_eq!(
            app.world()
                .resource::<Events<CultivationDeathTrigger>>()
                .len(),
            0
        );
        assert_eq!(app.world().resource::<Events<VfxEventRequest>>().len(), 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tribulation_wave_system_aborts_ascension_when_quota_write_fails() {
        let mut app = App::new();
        let (settings, root) =
            unbootstrapped_persistence_settings("tribulation-ascension-quota-write-failure");
        app.insert_resource(settings);
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<TribulationSettled>();
        app.add_event::<AscensionQuotaOccupied>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_systems(Update, tribulation_wave_system);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 210.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                LifespanComponent::new(LifespanCapTable::SPIRIT),
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(3),
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 2,
                    waves_total: 3,
                    started_tick: 100,
                    phase_started_tick: 200,
                    next_wave_tick: 300,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
            ))
            .id();

        app.world_mut()
            .send_event(TribulationWaveCleared { entity, wave: 3 });
        app.update();

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.realm, Realm::Spirit);
        assert_eq!(cultivation.qi_max, 210.0);
        let lifespan = app
            .world()
            .get::<LifespanComponent>(entity)
            .expect("lifespan should remain attached");
        assert_eq!(lifespan.cap_by_realm, LifespanCapTable::SPIRIT);
        let state = app
            .world()
            .get::<TribulationState>(entity)
            .expect("failed quota write should keep tribulation state for operator recovery");
        assert_ne!(state.phase, TribulationPhase::Settle);
        assert_eq!(
            app.world()
                .resource::<Events<AscensionQuotaOccupied>>()
                .len(),
            0
        );
        assert_eq!(app.world().resource::<Events<SkillCapChanged>>().len(), 0);
        assert_eq!(
            app.world().resource::<Events<TribulationSettled>>().len(),
            0
        );

        let _ = fs::remove_dir_all(root);
    }

    fn collect_vfx_payloads(app: &mut App) -> Vec<VfxEventPayloadV1> {
        app.world_mut()
            .resource_mut::<Events<VfxEventRequest>>()
            .drain()
            .map(|event| event.payload)
            .collect()
    }

    #[test]
    fn tribulation_announce_emits_boundary_vfx() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 0 });
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_tribulation_boundary_vfx_system);

        app.world_mut().send_event(TribulationAnnounce {
            entity: Entity::PLACEHOLDER,
            char_id: "offline:Azure".to_string(),
            actor_name: "Azure".to_string(),
            epicenter: [12.0, 66.0, -8.0],
            waves_total: 3,
            started_tick: 0,
        });

        app.update();

        let payloads = collect_vfx_payloads(&mut app);
        assert_eq!(payloads.len(), 2);
        match &payloads[0] {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                origin,
                direction,
                count,
                duration_ticks,
                ..
            } => {
                assert_eq!(event_id, DUXU_OMEN_CLOUD_VFX_EVENT_ID);
                assert_eq!(*origin, [12.0, 90.0, -8.0]);
                assert_eq!(*direction, Some([24.0, 8.0, 24.0]));
                assert_eq!(*count, Some(36));
                assert_eq!(*duration_ticks, Some(200));
            }
            other => panic!("unexpected omen cloud vfx payload: {other:?}"),
        }
        match &payloads[1] {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                origin,
                strength,
                duration_ticks,
                ..
            } => {
                assert_eq!(event_id, DUXU_BOUNDARY_VFX_EVENT_ID);
                assert_eq!(*origin, [12.0, 66.0, -8.0]);
                assert_eq!(*strength, Some(1.0));
                assert_eq!(*duration_ticks, Some(200));
            }
            other => panic!("unexpected boundary vfx payload: {other:?}"),
        }
    }

    #[test]
    fn tribulation_omen_cloud_blocks_overlay_and_restore() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.world_mut()
            .get_mut::<ChunkLayer>(layer)
            .expect("test layer should carry ChunkLayer")
            .insert_chunk([0, 0], valence::prelude::UnloadedChunk::new());
        app.insert_resource(CombatClock { tick: 0 });
        app.insert_resource(TribulationOmenCloudBlocks::default());
        app.add_event::<TribulationAnnounce>();
        app.add_systems(Update, tribulation_omen_cloud_block_overlay_system);

        let entity = app
            .world_mut()
            .spawn(TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Omen,
                epicenter: [8.0, 66.0, 8.0],
                wave_current: 0,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 0,
                next_wave_tick: DUXU_OMEN_TICKS + DUXU_LOCK_TICKS,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            })
            .id();
        app.world_mut().send_event(TribulationAnnounce {
            entity,
            char_id: "offline:Azure".to_string(),
            actor_name: "Azure".to_string(),
            epicenter: [8.0, 66.0, 8.0],
            waves_total: 3,
            started_tick: 0,
        });

        app.update();

        let center = BlockPos::new(8, 90, 8);
        let edge = BlockPos::new(12, 90, 8);
        {
            let layer_ref = app
                .world()
                .get::<ChunkLayer>(layer)
                .expect("test layer should carry ChunkLayer");
            assert_eq!(
                layer_ref.block(center).map(|block| block.state),
                Some(BlockState::BLACK_WOOL)
            );
            assert_eq!(
                layer_ref.block(edge).map(|block| block.state),
                Some(BlockState::WHITE_WOOL)
            );
        }

        app.world_mut()
            .entity_mut(entity)
            .remove::<TribulationState>();
        app.world_mut().resource_mut::<CombatClock>().tick = DUXU_OMEN_TICKS;
        app.update();

        let layer_ref = app.world().get::<ChunkLayer>(layer).unwrap();
        assert_eq!(
            layer_ref.block(center).map(|block| block.state),
            Some(BlockState::AIR)
        );
        assert_eq!(
            layer_ref.block(edge).map(|block| block.state),
            Some(BlockState::AIR)
        );
    }

    #[test]
    fn omen_midpoint_emits_soft_boundary_once() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: DUXU_OMEN_TICKS / 2,
        });
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_tribulation_boundary_vfx_system);

        app.world_mut().spawn(TribulationState {
            kind: TribulationKind::DuXu,
            phase: TribulationPhase::Omen,
            epicenter: [0.0, 66.0, 0.0],
            wave_current: 0,
            waves_total: 3,
            started_tick: 0,
            phase_started_tick: 0,
            next_wave_tick: DUXU_OMEN_TICKS + DUXU_LOCK_TICKS,
            participants: vec!["offline:Azure".to_string()],
            failed: false,
        });

        app.update();
        app.update();

        let payloads = collect_vfx_payloads(&mut app);
        assert_eq!(payloads.len(), 1);
        match &payloads[0] {
            VfxEventPayloadV1::SpawnParticle { strength, .. } => {
                assert_eq!(*strength, Some(0.5));
            }
            other => panic!("unexpected boundary vfx payload: {other:?}"),
        }
    }

    #[test]
    fn lock_and_wave_events_emit_boundary_vfx() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 900 });
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_tribulation_boundary_vfx_system);

        let entity = app
            .world_mut()
            .spawn(TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(1),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 1,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 900,
                next_wave_tick: 1200,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            })
            .id();
        app.world_mut().send_event(TribulationLocked {
            entity,
            char_id: "offline:Azure".to_string(),
            actor_name: "Azure".to_string(),
            epicenter: [0.0, 66.0, 0.0],
            waves_total: 3,
        });
        app.world_mut()
            .send_event(TribulationWaveCleared { entity, wave: 1 });

        app.update();

        let strengths: Vec<_> = collect_vfx_payloads(&mut app)
            .into_iter()
            .map(|payload| match payload {
                VfxEventPayloadV1::SpawnParticle { strength, .. } => strength,
                other => panic!("unexpected boundary vfx payload: {other:?}"),
            })
            .collect();
        assert_eq!(strengths, vec![Some(0.2), Some(0.1)]);
    }

    #[test]
    fn long_full_progress_du_xu_request_adds_heart_demon_and_kaitian_waves() {
        let mut app = App::new();
        app.add_event::<StartDuXuRequest>();
        app.add_event::<InitiateXuhuaTribulation>();
        app.add_systems(Update, start_du_xu_request_system);
        let requested_at_tick = DUXU_FULL_PROGRESS_MIN_TICKS + 500;
        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 210.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
                full_progress_life_record(100, 500),
            ))
            .id();

        app.world_mut().send_event(StartDuXuRequest {
            entity,
            requested_at_tick,
        });
        app.update();

        let events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].entity, entity);
        assert_eq!(emitted[0].waves_total, 5);
        assert_eq!(emitted[0].started_tick, requested_at_tick);
    }

    #[test]
    fn recent_full_progress_du_xu_request_keeps_default_three_waves() {
        let mut app = App::new();
        app.add_event::<StartDuXuRequest>();
        app.add_event::<InitiateXuhuaTribulation>();
        app.add_systems(Update, start_du_xu_request_system);
        let requested_at_tick = DUXU_FULL_PROGRESS_MIN_TICKS + 500;
        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 210.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
                full_progress_life_record(100, requested_at_tick - 1),
            ))
            .id();

        app.world_mut().send_event(StartDuXuRequest {
            entity,
            requested_at_tick,
        });
        app.update();

        let events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].waves_total, 3);
    }

    #[test]
    fn start_du_xu_request_rejects_non_spirit_or_incomplete_meridians() {
        let mut app = App::new();
        app.add_event::<StartDuXuRequest>();
        app.add_event::<InitiateXuhuaTribulation>();
        app.add_systems(Update, start_du_xu_request_system);

        let non_spirit = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Condense,
                    qi_current: 210.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
            ))
            .id();
        let incomplete = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 210.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                MeridianSystem::default(),
            ))
            .id();

        app.world_mut().send_event(StartDuXuRequest {
            entity: non_spirit,
            requested_at_tick: 100,
        });
        app.world_mut().send_event(StartDuXuRequest {
            entity: incomplete,
            requested_at_tick: 100,
        });
        app.update();

        let events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
        assert!(events.get_reader().read(events).next().is_none());
    }

    #[test]
    fn start_du_xu_request_rejects_already_active_tribulation() {
        let mut app = App::new();
        app.add_event::<StartDuXuRequest>();
        app.add_event::<InitiateXuhuaTribulation>();
        app.add_systems(Update, start_du_xu_request_system);
        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 210.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Omen,
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 0,
                    waves_total: 3,
                    started_tick: 0,
                    phase_started_tick: 0,
                    next_wave_tick: 0,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
            ))
            .id();

        app.world_mut().send_event(StartDuXuRequest {
            entity,
            requested_at_tick: 100,
        });
        app.update();

        let events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
        assert!(events.get_reader().read(events).next().is_none());
    }

    #[test]
    fn start_du_xu_request_dedupes_same_tick_duplicate_requests() {
        let mut app = App::new();
        app.add_event::<StartDuXuRequest>();
        app.add_event::<InitiateXuhuaTribulation>();
        app.add_systems(Update, start_du_xu_request_system);
        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 210.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
            ))
            .id();

        for _ in 0..2 {
            app.world_mut().send_event(StartDuXuRequest {
                entity,
                requested_at_tick: 100,
            });
        }
        app.update();

        let events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].entity, entity);
        assert_eq!(emitted[0].started_tick, 100);
    }

    #[test]
    fn fourth_wave_enters_heart_demon_without_aoe() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 2100 });
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_systems(
            Update,
            (
                tribulation_phase_tick_system,
                tribulation_aoe_system.after(tribulation_phase_tick_system),
            ),
        );

        let tribulator = app
            .world_mut()
            .spawn((
                Position::new([0.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 200.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                Wounds {
                    health_current: 100.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(3),
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 3,
                    waves_total: 5,
                    started_tick: 0,
                    phase_started_tick: 1800,
                    next_wave_tick: 2100,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
            ))
            .id();

        app.update();

        let state = app
            .world()
            .get::<TribulationState>(tribulator)
            .expect("tribulation should remain active");
        assert_eq!(state.phase, TribulationPhase::HeartDemon);
        assert_eq!(state.wave_current, 3);
        let wounds = app
            .world()
            .get::<Wounds>(tribulator)
            .expect("wounds should remain attached");
        assert_eq!(wounds.health_current, 100.0);
        assert!(wounds.entries.is_empty());
        let events = app.world().resource::<Events<TribulationWaveCleared>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].wave, 4);
    }

    #[test]
    fn pregen_offer_inserts_heart_demon_after_chain_lightning_without_consuming_wave() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1500 });
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_systems(Update, tribulation_phase_tick_system);

        let entity = app
            .world_mut()
            .spawn((
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(2),
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 2,
                    waves_total: 5,
                    started_tick: 100,
                    phase_started_tick: 1200,
                    next_wave_tick: 1500,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
                PendingHeartDemonOffer {
                    trigger_id: String::new(),
                    payload: HeartDemonOfferV1 {
                        offer_id: "heart-demon-pregen".to_string(),
                        trigger_id: String::new(),
                        trigger_label: "心魔照见".to_string(),
                        realm_label: "渡虚劫 · 心魔".to_string(),
                        composure: 0.7,
                        quota_remaining: 1,
                        quota_total: 1,
                        expires_at_ms: 1,
                        choices: Vec::new(),
                    },
                },
            ))
            .id();
        let trigger_id = format!("heart_demon:{}:100", entity.index());
        {
            let mut offer = app
                .world_mut()
                .get_mut::<PendingHeartDemonOffer>(entity)
                .expect("pregen offer should attach");
            offer.trigger_id = trigger_id.clone();
            offer.payload.trigger_id = trigger_id;
        }

        app.update();

        let state = app
            .world()
            .get::<TribulationState>(entity)
            .expect("tribulation should remain active");
        assert_eq!(state.phase, TribulationPhase::HeartDemon);
        assert_eq!(state.wave_current, 2);
        let events = app.world().resource::<Events<TribulationWaveCleared>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].wave, 2);
    }

    #[test]
    fn heart_demon_still_falls_back_to_fourth_slot_when_pregen_is_absent() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 2100 });
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_systems(Update, tribulation_phase_tick_system);
        let entity = app
            .world_mut()
            .spawn(TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(3),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 3,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 1800,
                next_wave_tick: 2100,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            })
            .id();

        app.update();

        let state = app
            .world()
            .get::<TribulationState>(entity)
            .expect("tribulation should remain active");
        assert_eq!(state.phase, TribulationPhase::HeartDemon);
        assert_eq!(state.wave_current, 3);
        let events = app.world().resource::<Events<TribulationWaveCleared>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].wave, 4);
    }

    #[test]
    fn resolved_early_heart_demon_continues_next_combat_wave() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1810 });
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_systems(Update, tribulation_phase_tick_system);
        let entity = app
            .world_mut()
            .spawn((
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::HeartDemon,
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 2,
                    waves_total: 5,
                    started_tick: 100,
                    phase_started_tick: 1500,
                    next_wave_tick: 1800,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
                HeartDemonResolution {
                    outcome: HeartDemonOutcome::Steadfast,
                    choice_idx: Some(0),
                    tick: 1510,
                    next_wave_multiplier: 1.0,
                },
            ))
            .id();

        app.update();

        let state = app
            .world()
            .get::<TribulationState>(entity)
            .expect("tribulation should remain active");
        assert_eq!(state.phase, TribulationPhase::Wave(3));
        assert_eq!(state.wave_current, 2);
        let events = app.world().resource::<Events<TribulationWaveCleared>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].wave, 3);
    }

    #[test]
    fn resolved_heart_demon_after_soul_devouring_skips_original_heart_demon_slot() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 2110 });
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_systems(Update, tribulation_phase_tick_system);
        let entity = app
            .world_mut()
            .spawn((
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::HeartDemon,
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 3,
                    waves_total: 5,
                    started_tick: 100,
                    phase_started_tick: 1800,
                    next_wave_tick: 2100,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
                HeartDemonResolution {
                    outcome: HeartDemonOutcome::Steadfast,
                    choice_idx: Some(0),
                    tick: 1810,
                    next_wave_multiplier: 1.0,
                },
            ))
            .id();

        app.update();

        let state = app
            .world()
            .get::<TribulationState>(entity)
            .expect("tribulation should remain active");
        assert_eq!(state.phase, TribulationPhase::Wave(DUXU_KAITIAN_WAVE));
        assert_eq!(state.wave_current, 3);
        let events = app.world().resource::<Events<TribulationWaveCleared>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].wave, DUXU_KAITIAN_WAVE);
    }

    #[test]
    fn unresolved_heart_demon_waits_without_advancing_to_kaitian_wave() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 2400 });
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_systems(Update, tribulation_phase_tick_system);
        let entity = app
            .world_mut()
            .spawn(TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::HeartDemon,
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 4,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 2100,
                next_wave_tick: 2400,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            })
            .id();

        app.update();

        let state = app
            .world()
            .get::<TribulationState>(entity)
            .expect("tribulation should remain active");
        assert_eq!(state.phase, TribulationPhase::HeartDemon);
        assert_eq!(state.phase_started_tick, 2100);
        let events = app.world().resource::<Events<TribulationWaveCleared>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert!(emitted.is_empty());
    }

    #[test]
    fn restored_fourth_wave_remains_heart_demon() {
        let state = TribulationState::restored(4, 5, 120);

        assert_eq!(state.phase, TribulationPhase::HeartDemon);
        assert_eq!(state.wave_current, 4);
        assert_eq!(state.waves_total, 5);
    }

    #[test]
    fn heart_demon_steadfast_choice_records_and_restores_qi() {
        let mut app = App::new();
        app.add_event::<HeartDemonChoiceSubmitted>();
        app.add_systems(Update, heart_demon_choice_system);
        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 120.0,
                    qi_max: 210.0,
                    qi_max_frozen: Some(10.0),
                    ..Default::default()
                },
                LifeRecord::new("offline:Azure"),
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::HeartDemon,
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 4,
                    waves_total: 5,
                    started_tick: 0,
                    phase_started_tick: 2100,
                    next_wave_tick: 2400,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
            ))
            .id();

        app.world_mut().send_event(HeartDemonChoiceSubmitted {
            entity,
            choice_idx: Some(0),
            submitted_at_tick: 2110,
        });
        app.update();

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.qi_current, 140.0);
        let resolution = app
            .world()
            .get::<HeartDemonResolution>(entity)
            .expect("resolution should be recorded");
        assert_eq!(resolution.outcome, HeartDemonOutcome::Steadfast);
        assert_eq!(resolution.choice_idx, Some(0));
        assert_eq!(resolution.tick, 2110);
        assert_eq!(resolution.next_wave_multiplier, 1.0);
        let life = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("life record should remain attached");
        assert!(matches!(
            life.biography.last(),
            Some(BiographyEntry::HeartDemonRecord {
                outcome: HeartDemonOutcome::Steadfast,
                choice_idx: Some(0),
                tick: 2110
            })
        ));
    }

    #[test]
    fn heart_demon_obsession_timeout_penalizes_qi_and_boosts_kaitian_damage() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: 2100 + DUXU_HEART_DEMON_TIMEOUT_TICKS,
        });
        app.add_event::<TribulationWaveCleared>();
        app.add_systems(Update, heart_demon_timeout_system);
        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 100.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                LifeRecord::new("offline:Azure"),
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::HeartDemon,
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 4,
                    waves_total: 5,
                    started_tick: 0,
                    phase_started_tick: 2100,
                    next_wave_tick: 2400,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
            ))
            .id();

        app.update();

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.qi_current, 70.0);
        let resolution = app
            .world()
            .get::<HeartDemonResolution>(entity)
            .expect("resolution should be recorded");
        assert_eq!(resolution.outcome, HeartDemonOutcome::Obsession);
        assert_eq!(resolution.choice_idx, None);
        assert_eq!(
            resolution.next_wave_multiplier,
            DUXU_HEART_DEMON_OBSESSION_NEXT_WAVE_MULTIPLIER
        );
    }

    #[test]
    fn heart_demon_no_solution_choice_records_without_penalty_or_boost() {
        let mut app = App::new();
        app.add_event::<HeartDemonChoiceSubmitted>();
        app.add_systems(Update, heart_demon_choice_system);
        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 100.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                LifeRecord::new("offline:Azure"),
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::HeartDemon,
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 4,
                    waves_total: 5,
                    started_tick: 0,
                    phase_started_tick: 2100,
                    next_wave_tick: 2400,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
            ))
            .id();

        app.world_mut().send_event(HeartDemonChoiceSubmitted {
            entity,
            choice_idx: Some(2),
            submitted_at_tick: 2115,
        });
        app.update();

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.qi_current, 100.0);
        let resolution = app
            .world()
            .get::<HeartDemonResolution>(entity)
            .expect("resolution should be recorded");
        assert_eq!(resolution.outcome, HeartDemonOutcome::NoSolution);
        assert_eq!(resolution.choice_idx, Some(2));
        assert_eq!(resolution.tick, 2115);
        assert_eq!(resolution.next_wave_multiplier, 1.0);
        let life = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("life record should remain attached");
        assert!(matches!(
            life.biography.last(),
            Some(BiographyEntry::HeartDemonRecord {
                outcome: HeartDemonOutcome::NoSolution,
                choice_idx: Some(2),
                tick: 2115
            })
        ));
    }

    #[test]
    fn heart_demon_resolution_advances_to_kaitian_without_republishing_fourth_wave() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 2140 });
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_systems(Update, tribulation_phase_tick_system);
        let entity = app
            .world_mut()
            .spawn((
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::HeartDemon,
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 4,
                    waves_total: 5,
                    started_tick: 0,
                    phase_started_tick: 2100,
                    next_wave_tick: 2400,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
                HeartDemonResolution {
                    outcome: HeartDemonOutcome::Obsession,
                    choice_idx: None,
                    tick: 2130,
                    next_wave_multiplier: DUXU_HEART_DEMON_OBSESSION_NEXT_WAVE_MULTIPLIER,
                },
            ))
            .id();

        app.update();

        let state = app
            .world()
            .get::<TribulationState>(entity)
            .expect("tribulation should remain active");
        assert_eq!(state.phase, TribulationPhase::Wave(5));
        assert_eq!(state.phase_started_tick, 2140);
        let events = app.world().resource::<Events<TribulationWaveCleared>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].wave, 5);
    }

    #[test]
    fn obsession_resolution_increases_kaitian_damage() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 2400 });
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, tribulation_aoe_system);
        let entity = app
            .world_mut()
            .spawn((
                Position::new([0.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 300.0,
                    qi_max: 300.0,
                    ..Default::default()
                },
                Wounds {
                    health_current: 200.0,
                    health_max: 200.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(5),
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 5,
                    waves_total: 5,
                    started_tick: 0,
                    phase_started_tick: 2400,
                    next_wave_tick: 2700,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
                HeartDemonResolution {
                    outcome: HeartDemonOutcome::Obsession,
                    choice_idx: None,
                    tick: 2130,
                    next_wave_multiplier: DUXU_HEART_DEMON_OBSESSION_NEXT_WAVE_MULTIPLIER,
                },
            ))
            .id();

        app.update();

        assert_eq!(app.world().resource::<Events<TribulationFailed>>().len(), 0);
        let wounds = app
            .world()
            .get::<Wounds>(entity)
            .expect("wounds should remain attached");
        assert_eq!(
            wounds.health_current,
            200.0 - DUXU_AOE_DAMAGE_BASE * 5.0 * DUXU_HEART_DEMON_OBSESSION_NEXT_WAVE_MULTIPLIER
        );
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(
            wounds.entries[0].severity,
            DUXU_AOE_DAMAGE_BASE * 5.0 * DUXU_HEART_DEMON_OBSESSION_NEXT_WAVE_MULTIPLIER
        );
    }

    #[test]
    fn publish_lock_event_to_tribulation_channel() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_systems(Update, publish_tribulation_events);

        app.world_mut()
            .resource_mut::<Events<TribulationLocked>>()
            .send(TribulationLocked {
                entity: Entity::PLACEHOLDER,
                char_id: "offline:Azure".to_string(),
                actor_name: "Azure".to_string(),
                epicenter: [12.0, 66.0, -8.0],
                waves_total: 3,
            });

        app.update();

        let outbound = rx_outbound
            .try_recv()
            .expect("lock event should publish to redis bridge");
        match outbound {
            RedisOutbound::TribulationEvent(payload) => {
                assert_eq!(payload.phase, TribulationPhaseV1::Lock);
                assert_eq!(payload.char_id.as_deref(), Some("offline:Azure"));
                assert_eq!(payload.actor_name.as_deref(), Some("Azure"));
                assert_eq!(payload.epicenter, Some([12.0, 66.0, -8.0]));
                assert_eq!(payload.wave_total, Some(3));
            }
            other => panic!("unexpected outbound payload: {other:?}"),
        }
    }

    #[test]
    fn publish_wave_event_keeps_tribulator_identity() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_systems(Update, publish_tribulation_events);

        let entity = app
            .world_mut()
            .spawn((
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                Username("Azure".to_string()),
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(2),
                    epicenter: [12.0, 66.0, -8.0],
                    wave_current: 1,
                    waves_total: 5,
                    started_tick: 0,
                    phase_started_tick: 1200,
                    next_wave_tick: 1500,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
            ))
            .id();
        app.world_mut()
            .resource_mut::<Events<TribulationWaveCleared>>()
            .send(TribulationWaveCleared { entity, wave: 2 });

        app.update();

        let outbound = rx_outbound
            .try_recv()
            .expect("wave event should publish to redis bridge");
        match outbound {
            RedisOutbound::TribulationEvent(payload) => {
                assert_eq!(payload.phase, TribulationPhaseV1::Wave { wave: 2 });
                assert_eq!(payload.char_id.as_deref(), Some("offline:Azure"));
                assert_eq!(payload.actor_name.as_deref(), Some("Azure"));
                assert_eq!(payload.epicenter, Some([12.0, 66.0, -8.0]));
                assert_eq!(payload.wave_current, Some(2));
                assert_eq!(payload.wave_total, Some(5));
            }
            other => panic!("unexpected outbound payload: {other:?}"),
        }
    }

    #[test]
    fn publish_settle_event_uses_actor_name() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_systems(Update, publish_tribulation_events);

        let entity = app
            .world_mut()
            .spawn((
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                Username("Azure".to_string()),
            ))
            .id();
        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(TribulationSettled {
                entity,
                kind: TribulationKind::DuXu,
                source: None,
                result: DuXuResultV1 {
                    char_id: "offline:Azure".to_string(),
                    outcome: DuXuOutcomeV1::Ascended,
                    killer: None,
                    waves_survived: 5,
                    reason: None,
                },
            });

        app.update();

        let outbound = rx_outbound
            .try_recv()
            .expect("settle event should publish to redis bridge");
        match outbound {
            RedisOutbound::TribulationEvent(payload) => {
                assert_eq!(payload.phase, TribulationPhaseV1::Settle);
                assert_eq!(payload.char_id.as_deref(), Some("offline:Azure"));
                assert_eq!(payload.actor_name.as_deref(), Some("Azure"));
                assert_eq!(
                    payload.result.expect("settle should carry result").outcome,
                    DuXuOutcomeV1::Ascended
                );
            }
            other => panic!("unexpected outbound payload: {other:?}"),
        }
    }

    #[test]
    fn publish_ascension_quota_open_event_to_tribulation_channel() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_systems(Update, publish_tribulation_events);

        app.world_mut()
            .resource_mut::<Events<AscensionQuotaOpened>>()
            .send(AscensionQuotaOpened { occupied_slots: 1 });

        app.update();

        let outbound = rx_outbound
            .try_recv()
            .expect("quota open event should publish to redis bridge");
        match outbound {
            RedisOutbound::TribulationEvent(payload) => {
                assert_eq!(
                    payload.kind,
                    crate::schema::tribulation::TribulationKindV1::AscensionQuotaOpen
                );
                assert_eq!(payload.phase, TribulationPhaseV1::Settle);
                assert_eq!(payload.occupied_slots, Some(1));
            }
            other => panic!("unexpected outbound payload: {other:?}"),
        }
    }

    #[test]
    fn void_quota_limit_uses_world_qi_budget_floor() {
        let k = DEFAULT_VOID_QUOTA_K;
        assert_eq!(compute_void_quota_limit(0.0, k), 0);
        assert_eq!(compute_void_quota_limit(49.999, k), 0);
        assert_eq!(compute_void_quota_limit(50.0, k), 1);
        assert_eq!(compute_void_quota_limit(99.999, k), 1);
        assert_eq!(compute_void_quota_limit(100.0, k), 2);
        assert_eq!(compute_void_quota_limit(-1.0, k), 0);
        assert_eq!(compute_void_quota_limit(f64::NAN, k), 0);
        assert_eq!(compute_void_quota_limit(100.0, 0.0), 0);
    }

    #[test]
    fn check_void_quota_allows_zero_and_reports_availability() {
        let config = VoidQuotaConfig { quota_k: 50.0 };
        let low_budget = WorldQiBudget::from_total(100.0);
        let mut depleted = low_budget;
        depleted.current_total = 49.0;

        let depleted_check = check_void_quota(0, &depleted, &config);
        assert_eq!(depleted_check.quota_limit, 0);
        assert_eq!(depleted_check.available_slots, 0);
        assert!(depleted_check.exceeded);

        let check = check_void_quota(1, &low_budget, &config);
        assert_eq!(check.quota_limit, 2);
        assert_eq!(check.available_slots, 1);
        assert!(!check.exceeded);

        let full = check_void_quota(2, &low_budget, &config);
        assert_eq!(full.available_slots, 0);
        assert!(full.exceeded);
    }

    #[test]
    fn phase7_balance_three_wave_curve_fits_spirit_pool() {
        let spirit_pool = 210.0;
        let profiles = (1..=3).map(du_xu_wave_profile).collect::<Vec<_>>();
        assert_eq!(profiles[0].damage, 18.0);
        assert_eq!(profiles[1].damage, 36.0);
        assert_eq!(profiles[2].damage, 54.0);
        assert_eq!(profiles[0].qi_drain, 35.0);
        assert_eq!(profiles[1].qi_drain, 70.0);
        assert_eq!(profiles[2].qi_drain, 105.0);
        assert_eq!(
            profiles.iter().map(|profile| profile.damage).sum::<f32>(),
            108.0
        );
        assert_eq!(
            profiles.iter().map(|profile| profile.qi_drain).sum::<f64>(),
            210.0
        );
        assert!(profiles.iter().map(|profile| profile.qi_drain).sum::<f64>() <= spirit_pool);
    }

    #[test]
    fn phase7_balance_interception_window_matches_lock_and_heart_demon_timing() {
        let windows = [DUXU_WAVE_COOLDOWN_TICKS, DUXU_HEART_DEMON_TIMEOUT_TICKS];
        let radii = [
            DUXU_LOCK_RADIUS_FINAL,
            DUXU_LOCK_RADIUS_HARD,
            TRIBULATION_DANGER_RADIUS,
        ];

        assert_eq!(windows, [15 * 20, 30 * 20]);
        assert_eq!(radii, [10.0, 20.0, 100.0]);
        assert!(radii.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn lock_expiry_starts_first_wave_and_schedules_cooldown() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 900 });
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_systems(Update, tribulation_phase_tick_system);

        let entity = app
            .world_mut()
            .spawn(TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Lock,
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 0,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 300,
                next_wave_tick: 0,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            })
            .id();

        app.update();

        let state = app
            .world()
            .get::<TribulationState>(entity)
            .expect("tribulation should remain active");
        assert_eq!(state.phase, TribulationPhase::Wave(1));
        assert_eq!(state.phase_started_tick, 900);
        assert_eq!(state.next_wave_tick, 900 + DUXU_WAVE_COOLDOWN_TICKS);
        let events = app.world().resource::<Events<TribulationWaveCleared>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].entity, entity);
        assert_eq!(emitted[0].wave, 1);
    }

    #[test]
    fn wave_cooldown_starts_next_wave_without_reusing_first_wave_phase() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1200 });
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_systems(Update, tribulation_phase_tick_system);

        let entity = app
            .world_mut()
            .spawn(TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(1),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 1,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 900,
                next_wave_tick: 1200,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            })
            .id();

        app.update();

        let state = app
            .world()
            .get::<TribulationState>(entity)
            .expect("tribulation should remain active");
        assert_eq!(state.phase, TribulationPhase::Wave(2));
        assert_eq!(state.phase_started_tick, 1200);
        assert_eq!(state.next_wave_tick, 1200 + DUXU_WAVE_COOLDOWN_TICKS);
        let events = app.world().resource::<Events<TribulationWaveCleared>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].wave, 2);
    }

    #[test]
    fn aoe_uses_current_wave_strength_only_on_wave_start_tick() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1200 });
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, tribulation_aoe_system);

        app.world_mut().spawn(TribulationState {
            kind: TribulationKind::DuXu,
            phase: TribulationPhase::Wave(2),
            epicenter: [0.0, 66.0, 0.0],
            wave_current: 1,
            waves_total: 3,
            started_tick: 0,
            phase_started_tick: 1200,
            next_wave_tick: 1500,
            participants: vec!["offline:Azure".to_string()],
            failed: false,
        });
        let target = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 100.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                Wounds {
                    health_current: 100.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: "offline:Spectator".to_string(),
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        let wounds = app
            .world()
            .get::<Wounds>(target)
            .expect("wounds should remain attached");
        assert_eq!(wounds.health_current, 100.0 - DUXU_AOE_DAMAGE_BASE * 2.0);
        assert_eq!(wounds.entries.len(), DUXU_CHAIN_LIGHTNING_STRIKES as usize);
        for wound in &wounds.entries {
            assert_eq!(wound.kind, WoundKind::Burn);
            assert_eq!(wound.severity, DUXU_AOE_DAMAGE_BASE * 2.0 / 3.0);
            assert_eq!(wound.created_at_tick, 1200);
            assert_eq!(wound.inflicted_by.as_deref(), Some("du_xu_tribulation"));
        }
        let cultivation = app
            .world()
            .get::<Cultivation>(target)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.qi_current, 100.0 - DUXU_QI_DRAIN_BASE * 2.0);
        assert_eq!(cultivation.qi_max_frozen, None);

        app.world_mut().resource_mut::<CombatClock>().tick = 1201;
        app.update();

        let wounds = app
            .world()
            .get::<Wounds>(target)
            .expect("wounds should remain attached");
        assert_eq!(wounds.health_current, 100.0 - DUXU_AOE_DAMAGE_BASE * 2.0);
        assert_eq!(wounds.entries.len(), DUXU_CHAIN_LIGHTNING_STRIKES as usize);
    }

    #[test]
    fn spectator_aoe_is_not_reduced_by_distance_within_danger_radius() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1200 });
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, tribulation_aoe_system);

        app.world_mut().spawn(TribulationState {
            kind: TribulationKind::DuXu,
            phase: TribulationPhase::Wave(2),
            epicenter: [0.0, 66.0, 0.0],
            wave_current: 2,
            waves_total: 3,
            started_tick: 0,
            phase_started_tick: 1200,
            next_wave_tick: 1500,
            participants: vec!["offline:Azure".to_string()],
            failed: false,
        });
        let near = spawn_tribulation_spectator(&mut app, "Near", [3.0, 66.0, 0.0]);
        let far_inside = spawn_tribulation_spectator(
            &mut app,
            "FarInside",
            [TRIBULATION_DANGER_RADIUS, 66.0, 0.0],
        );
        let outside = spawn_tribulation_spectator(
            &mut app,
            "Outside",
            [TRIBULATION_DANGER_RADIUS + 0.1, 66.0, 0.0],
        );

        app.update();

        let expected_health = 200.0 - DUXU_AOE_DAMAGE_BASE * 2.0;
        let expected_qi = 200.0 - DUXU_QI_DRAIN_BASE * 2.0;
        for entity in [near, far_inside] {
            let wounds = app
                .world()
                .get::<Wounds>(entity)
                .expect("spectator wounds should remain attached");
            assert_eq!(wounds.health_current, expected_health);
            assert_eq!(wounds.entries.len(), DUXU_CHAIN_LIGHTNING_STRIKES as usize);
            let cultivation = app
                .world()
                .get::<Cultivation>(entity)
                .expect("spectator cultivation should remain attached");
            assert_eq!(cultivation.qi_current, expected_qi);
        }
        let outside_wounds = app
            .world()
            .get::<Wounds>(outside)
            .expect("outside spectator wounds should remain attached");
        assert_eq!(outside_wounds.health_current, 200.0);
        assert!(outside_wounds.entries.is_empty());
        let outside_cultivation = app
            .world()
            .get::<Cultivation>(outside)
            .expect("outside spectator cultivation should remain attached");
        assert_eq!(outside_cultivation.qi_current, 200.0);
    }

    #[test]
    fn tribulation_aoe_ignores_targets_in_other_dimension() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1200 });
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, tribulation_aoe_system);

        app.world_mut().spawn((
            CurrentDimension(DimensionKind::Overworld),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(2),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 2,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 1200,
                next_wave_tick: 1500,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
        ));
        let target = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 0.0]),
                CurrentDimension(DimensionKind::Tsy),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 100.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                Wounds {
                    health_current: 100.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: "offline:Spectator".to_string(),
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        let wounds = app
            .world()
            .get::<Wounds>(target)
            .expect("wounds should remain attached");
        assert_eq!(wounds.health_current, 100.0);
        assert!(wounds.entries.is_empty());
        let cultivation = app
            .world()
            .get::<Cultivation>(target)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.qi_current, 100.0);
    }

    #[test]
    fn third_wave_freezes_qi_max_as_soul_devouring_lightning() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1500 });
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, tribulation_aoe_system);

        app.world_mut().spawn(TribulationState {
            kind: TribulationKind::DuXu,
            phase: TribulationPhase::Wave(3),
            epicenter: [0.0, 66.0, 0.0],
            wave_current: 3,
            waves_total: 3,
            started_tick: 0,
            phase_started_tick: 1500,
            next_wave_tick: 1800,
            participants: vec!["offline:Azure".to_string()],
            failed: false,
        });
        let target = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 200.0,
                    qi_max: 210.0,
                    qi_max_frozen: Some(10.0),
                    ..Default::default()
                },
                Wounds {
                    health_current: 200.0,
                    health_max: 200.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: "offline:Spectator".to_string(),
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        let wounds = app
            .world()
            .get::<Wounds>(target)
            .expect("wounds should remain attached");
        assert_eq!(wounds.health_current, 200.0 - DUXU_AOE_DAMAGE_BASE * 3.0);
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(wounds.entries[0].severity, DUXU_AOE_DAMAGE_BASE * 3.0);
        let cultivation = app
            .world()
            .get::<Cultivation>(target)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.qi_current, 200.0 - DUXU_QI_DRAIN_BASE * 3.0);
        let expected_frozen = 10.0 + 210.0 * DUXU_SOUL_DEVOUR_QI_MAX_FREEZE_RATIO;
        assert!(
            (cultivation.qi_max_frozen.expect("qi max should freeze") - expected_frozen).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn kaitian_lightning_fails_tribulator_without_full_health() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 2100 });
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, tribulation_aoe_system);

        let entity = app
            .world_mut()
            .spawn((
                Position::new([0.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 210.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                Wounds {
                    health_current: 99.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(5),
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 5,
                    waves_total: 5,
                    started_tick: 0,
                    phase_started_tick: 2100,
                    next_wave_tick: 2400,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
            ))
            .id();

        app.update();

        let failures = app.world().resource::<Events<TribulationFailed>>();
        let emitted: Vec<_> = failures.get_reader().read(failures).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].entity, entity);
        assert_eq!(emitted[0].wave, 5);
        let wounds = app
            .world()
            .get::<Wounds>(entity)
            .expect("wounds should remain attached");
        assert_eq!(wounds.health_current, 99.0);
        assert!(wounds.entries.is_empty());
    }

    #[test]
    fn kaitian_lightning_fails_tribulator_without_full_available_qi() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 2100 });
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, tribulation_aoe_system);

        let entity = app
            .world_mut()
            .spawn((
                Position::new([0.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 189.0,
                    qi_max: 210.0,
                    qi_max_frozen: Some(20.0),
                    ..Default::default()
                },
                Wounds {
                    health_current: 100.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(5),
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 5,
                    waves_total: 5,
                    started_tick: 0,
                    phase_started_tick: 2100,
                    next_wave_tick: 2400,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
            ))
            .id();

        app.update();

        let failures = app.world().resource::<Events<TribulationFailed>>();
        let emitted: Vec<_> = failures.get_reader().read(failures).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].entity, entity);
        assert_eq!(emitted[0].wave, 5);
        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.qi_current, 189.0);
    }

    #[test]
    fn kaitian_lightning_hits_normally_when_tribulator_has_full_resources() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 2100 });
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, tribulation_aoe_system);

        let entity = app
            .world_mut()
            .spawn((
                Position::new([0.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 190.0,
                    qi_max: 210.0,
                    qi_max_frozen: Some(20.0),
                    ..Default::default()
                },
                Wounds {
                    health_current: 200.0,
                    health_max: 200.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(5),
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 5,
                    waves_total: 5,
                    started_tick: 0,
                    phase_started_tick: 2100,
                    next_wave_tick: 2400,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                },
            ))
            .id();

        app.update();

        assert_eq!(app.world().resource::<Events<TribulationFailed>>().len(), 0);
        let wounds = app
            .world()
            .get::<Wounds>(entity)
            .expect("wounds should remain attached");
        assert_eq!(wounds.health_current, 200.0 - DUXU_AOE_DAMAGE_BASE * 5.0);
        assert_eq!(wounds.entries.len(), 1);
        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.qi_current, 190.0 - DUXU_QI_DRAIN_BASE * 5.0);
    }

    #[test]
    fn void_quota_exceeded_start_marks_du_xu_for_juebi_instead_of_terminal_death() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("void-quota-exceeded-start");
        let char_id = "offline:Azure";
        let mut depleted_budget = WorldQiBudget::from_total(100.0);
        depleted_budget.current_total = 0.0;

        app.insert_resource(settings.clone());
        app.insert_resource(depleted_budget);
        app.insert_resource(VoidQuotaConfig { quota_k: 50.0 });
        app.add_event::<InitiateXuhuaTribulation>();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationSettled>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, start_tribulation_system);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 210.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
                Wounds::default(),
                Lifecycle {
                    character_id: char_id.to_string(),
                    ..Default::default()
                },
                LifeRecord::new(char_id),
                DeathRegistry {
                    char_id: char_id.to_string(),
                    death_count: 0,
                    last_death_tick: None,
                    prev_death_tick: None,
                    last_death_zone: None,
                },
                LifespanComponent {
                    born_at_tick: 0,
                    years_lived: 80.0,
                    cap_by_realm: LifespanCapTable::SPIRIT,
                    offline_pause_tick: None,
                },
                Position::new([0.0, 66.0, 0.0]),
            ))
            .id();
        app.world_mut().send_event(InitiateXuhuaTribulation {
            entity,
            waves_total: 5,
            started_tick: 120,
        });

        app.update();

        let entity_ref = app.world().entity(entity);
        let state = entity_ref
            .get::<TribulationState>()
            .expect("over-quota DuXu should still start and append JueBi at settlement");
        assert_eq!(state.kind, TribulationKind::DuXu);
        assert!(entity_ref.get::<JueBiAfterDuXuQuota>().is_some());

        let death_triggers = app.world().resource::<Events<CultivationDeathTrigger>>();
        let deaths: Vec<_> = death_triggers
            .get_reader()
            .read(death_triggers)
            .cloned()
            .collect();
        assert!(deaths.is_empty());

        let settled = app.world().resource::<Events<TribulationSettled>>();
        let emitted: Vec<_> = settled.get_reader().read(settled).cloned().collect();
        assert!(emitted.is_empty());
        assert!(
            load_active_tribulation(&settings, char_id)
                .expect("active tribulation query should succeed")
                .is_some(),
            "over-quota DuXu should persist until JueBi settlement"
        );
        let quota = load_ascension_quota(&settings).expect("quota load should succeed");
        assert_eq!(quota.occupied_slots, 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn juebi_trigger_event_starts_juebi_state_after_delay() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("juebi-trigger-start");
        let char_id = "offline:Azure";
        app.insert_resource(settings.clone());
        app.insert_resource(CombatClock { tick: 40 });
        app.insert_resource(PendingJueBiTriggers::default());
        app.add_event::<JueBiTriggerEvent>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_systems(
            Update,
            (
                schedule_juebi_triggers_system,
                start_due_juebi_triggers_system.after(schedule_juebi_triggers_system),
            ),
        );
        let entity = app
            .world_mut()
            .spawn((
                Lifecycle {
                    character_id: char_id.to_string(),
                    ..Default::default()
                },
                Username("Azure".to_string()),
                Position::new([12.0, 66.0, -3.0]),
                Cultivation {
                    realm: Realm::Void,
                    qi_current: 300.0,
                    qi_max: 300.0,
                    ..Default::default()
                },
            ))
            .id();
        app.world_mut().send_event(JueBiTriggerEvent {
            entity,
            source: JueBiTriggerSource::VoidActionExplodeZone,
            delay_ticks: 0,
            triggered_at_tick: 40,
            epicenter: None,
        });

        app.update();

        let state = app
            .world()
            .get::<TribulationState>(entity)
            .expect("JueBi trigger should insert active tribulation");
        assert_eq!(state.kind, TribulationKind::JueBi);
        assert_eq!(state.phase, TribulationPhase::Omen);
        assert_eq!(state.epicenter, [12.0, 66.0, -3.0]);
        let events = app.world().resource::<Events<JueBiTriggeredEvent>>();
        assert_eq!(events.len(), 1);
        let active = load_active_tribulation(&settings, char_id)
            .expect("active tribulation query should succeed")
            .expect("JueBi trigger should persist active row");
        assert_eq!(active.kind, "jue_bi");
        assert_eq!(active.epicenter, [12.0, 66.0, -3.0]);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn juebi_pressure_collapse_drains_qi_and_marks_targets() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 100 });
        app.insert_resource(JueBiNullFields::default());
        app.add_event::<DeathEvent>();
        app.add_systems(Update, juebi_phase_effect_system);
        let tribulator = app
            .world_mut()
            .spawn(TribulationState {
                kind: TribulationKind::JueBi,
                phase: TribulationPhase::Wave(1),
                epicenter: [0.0, 64.0, 0.0],
                wave_current: 0,
                waves_total: JUEBI_WAVES_TOTAL,
                started_tick: 0,
                phase_started_tick: 100,
                next_wave_tick: 100 + JUEBI_PHASE_TICKS,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            })
            .id();
        let target = app
            .world_mut()
            .spawn((
                Position::new([10.0, 64.0, 0.0]),
                Cultivation {
                    realm: Realm::Void,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                Wounds::default(),
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
            ))
            .id();
        let _ = tribulator;

        app.update();

        let cultivation = app.world().get::<Cultivation>(target).unwrap();
        assert!(cultivation.qi_current < 100.0);
        assert!(app.world().get::<JueBiPressureCollapse>(target).is_some());
    }

    #[test]
    fn juebi_phase_effect_clears_stale_phase_markers() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 200 });
        app.insert_resource(JueBiNullFields::default());
        app.add_event::<DeathEvent>();
        app.add_systems(Update, juebi_phase_effect_system);
        app.world_mut().spawn(TribulationState {
            kind: TribulationKind::JueBi,
            phase: TribulationPhase::Wave(2),
            epicenter: [0.0, 64.0, 0.0],
            wave_current: 1,
            waves_total: JUEBI_WAVES_TOTAL,
            started_tick: 0,
            phase_started_tick: 200,
            next_wave_tick: 200 + JUEBI_PHASE_TICKS,
            participants: vec!["offline:Azure".to_string()],
            failed: false,
        });
        let target = app
            .world_mut()
            .spawn((
                Position::new([10.0, 64.0, 0.0]),
                Cultivation {
                    realm: Realm::Void,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                Wounds::default(),
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                JueBiPressureCollapse {
                    epicenter: BlockPos::new(0, 64, 0),
                    phase_start_tick: 100,
                    distance: 10.0,
                },
            ))
            .id();

        app.update();

        assert!(app.world().get::<JueBiPressureCollapse>(target).is_none());
        assert!(app.world().get::<JueBiLawDisruption>(target).is_some());
    }

    #[test]
    fn juebi_settlement_treats_zero_health_as_killed_even_with_qi() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("juebi-zero-health-settle");
        let char_id = "offline:Azure";
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 600 });
        app.insert_resource(WorldQiBudget::from_total(100.0));
        app.insert_resource(VoidQuotaConfig::default());
        app.add_event::<SkillCapChanged>();
        app.add_event::<TribulationSettled>();
        app.add_event::<AscensionQuotaOccupied>();
        app.add_systems(Update, juebi_settlement_system);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Void,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                Wounds {
                    health_current: 0.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: char_id.to_string(),
                    ..Default::default()
                },
                LifeRecord::new(char_id),
                TribulationState {
                    kind: TribulationKind::JueBi,
                    phase: TribulationPhase::Settle,
                    epicenter: [0.0, 64.0, 0.0],
                    wave_current: JUEBI_WAVES_TOTAL,
                    waves_total: JUEBI_WAVES_TOTAL,
                    started_tick: 0,
                    phase_started_tick: 600,
                    next_wave_tick: 600,
                    participants: vec![char_id.to_string()],
                    failed: false,
                },
                JueBiRuntimeContext {
                    source: JueBiTriggerSource::VoidActionExplodeZone,
                    intensity: 1.6,
                },
            ))
            .id();

        app.update();

        let settled = app.world().resource::<Events<TribulationSettled>>();
        let emitted: Vec<_> = settled.get_reader().read(settled).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].kind, TribulationKind::JueBi);
        assert_eq!(emitted[0].result.outcome, DuXuOutcomeV1::Killed);
        let life_record = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("life record should remain attached");
        assert!(life_record
            .biography
            .iter()
            .any(|entry| matches!(entry, BiographyEntry::JueBiKilled { .. })));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn juebi_settlement_clears_independent_active_row() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("juebi-settle-clears-active-row");
        let char_id = "offline:Azure";
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: char_id.to_string(),
                kind: "jue_bi".to_string(),
                source: JueBiTriggerSource::VoidActionExplodeZone
                    .wire_name()
                    .to_string(),
                origin_dimension: Some("minecraft:overworld".to_string()),
                wave_current: JUEBI_WAVES_TOTAL,
                waves_total: JUEBI_WAVES_TOTAL,
                started_tick: 120,
                epicenter: [0.0, 64.0, 0.0],
                intensity: 1.6,
            },
        )
        .expect("active JueBi should persist before settlement");
        app.insert_resource(settings.clone());
        app.insert_resource(CombatClock { tick: 600 });
        app.insert_resource(WorldQiBudget::from_total(100.0));
        app.insert_resource(VoidQuotaConfig::default());
        app.add_event::<SkillCapChanged>();
        app.add_event::<TribulationSettled>();
        app.add_event::<AscensionQuotaOccupied>();
        app.add_systems(Update, juebi_settlement_system);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Void,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                Wounds {
                    health_current: 100.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: char_id.to_string(),
                    ..Default::default()
                },
                LifeRecord::new(char_id),
                TribulationState {
                    kind: TribulationKind::JueBi,
                    phase: TribulationPhase::Settle,
                    epicenter: [0.0, 64.0, 0.0],
                    wave_current: JUEBI_WAVES_TOTAL,
                    waves_total: JUEBI_WAVES_TOTAL,
                    started_tick: 120,
                    phase_started_tick: 600,
                    next_wave_tick: 600,
                    participants: vec![char_id.to_string()],
                    failed: false,
                },
                JueBiRuntimeContext {
                    source: JueBiTriggerSource::VoidActionExplodeZone,
                    intensity: 1.6,
                },
            ))
            .id();

        app.update();

        assert!(app.world().get::<TribulationState>(entity).is_none());
        assert!(
            load_active_tribulation(&settings, char_id)
                .expect("active tribulation query should succeed")
                .is_none(),
            "settled independent JueBi should clear active row"
        );
        let settled = app.world().resource::<Events<TribulationSettled>>();
        let emitted: Vec<_> = settled.get_reader().read(settled).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].kind, TribulationKind::JueBi);
        assert_eq!(emitted[0].result.outcome, DuXuOutcomeV1::HalfStep);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn juebi_terrain_generation_keeps_animation_order() {
        let mut pending = VecDeque::new();
        enqueue_juebi_terrain_ops(&mut pending, [0.0, 64.0, 0.0], 42, 6_000);
        assert!(pending.len() > 10_000);
        let mut last = 0;
        for op in pending {
            assert!(op.anim_order >= last);
            assert_eq!(op.restore_at_tick, 6_000);
            last = op.anim_order;
        }
    }

    #[test]
    fn juebi_terrain_tick_skips_unloaded_chunks_without_air_restore() {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.insert_resource(CombatClock { tick: 10 });
        let mut overlay = JueBiTerrainOverlay::default();
        overlay.pending.push_back(TerrainModOp {
            pos: BlockPos::new(32, 64, 32),
            new_state: BlockState::MAGMA_BLOCK,
            anim_order: 0,
            restore_at_tick: 6_000,
        });
        app.insert_resource(overlay);
        app.add_systems(Update, juebi_terrain_tick_system);

        app.update();

        let overlay = app.world().resource::<JueBiTerrainOverlay>();
        assert!(
            overlay.placed.is_empty(),
            "unloaded chunks must not record AIR as original block for later restore"
        );
        assert!(overlay.pending.is_empty());
    }

    #[test]
    fn juebi_terrain_tick_records_original_once_for_overlapping_ops() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.world_mut()
            .get_mut::<ChunkLayer>(layer)
            .expect("test layer should carry ChunkLayer")
            .insert_chunk([0, 0], valence::prelude::UnloadedChunk::new());
        let pos = BlockPos::new(1, 64, 1);
        app.world_mut()
            .get_mut::<ChunkLayer>(layer)
            .expect("test layer should carry ChunkLayer")
            .set_block(pos, BlockState::STONE);
        let mut overlay = JueBiTerrainOverlay::default();
        overlay.pending.push_back(TerrainModOp {
            pos,
            new_state: BlockState::MAGMA_BLOCK,
            anim_order: 0,
            restore_at_tick: 20,
        });
        overlay.pending.push_back(TerrainModOp {
            pos,
            new_state: BlockState::DEEPSLATE,
            anim_order: 1,
            restore_at_tick: 20,
        });
        app.insert_resource(CombatClock { tick: 10 });
        app.insert_resource(overlay);
        app.add_systems(Update, juebi_terrain_tick_system);

        app.update();

        {
            let overlay = app.world().resource::<JueBiTerrainOverlay>();
            assert_eq!(overlay.placed.len(), 1);
            assert_eq!(overlay.placed[0].original, BlockState::STONE);
        }
        {
            let layer_ref = app.world().get::<ChunkLayer>(layer).unwrap();
            assert_eq!(
                layer_ref.block(pos).map(|block| block.state),
                Some(BlockState::DEEPSLATE)
            );
        }

        app.world_mut().resource_mut::<CombatClock>().tick = 20;
        app.update();

        let layer_ref = app.world().get::<ChunkLayer>(layer).unwrap();
        assert_eq!(
            layer_ref.block(pos).map(|block| block.state),
            Some(BlockState::STONE)
        );
    }

    #[test]
    fn tribulation_failure_regresses_without_death_lifecycle_side_effects() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("failure-not-death");
        let char_id = "offline:Azure";
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: char_id.to_string(),
                kind: "du_xu".to_string(),
                source: String::new(),
                origin_dimension: Some("minecraft:overworld".to_string()),
                wave_current: 2,
                waves_total: 5,
                started_tick: 120,
                epicenter: [0.0, 64.0, 0.0],
                intensity: 0.0,
            },
        )
        .expect("active tribulation should persist before failure");

        app.insert_resource(settings.clone());
        app.insert_resource(CombatClock { tick: 300 });
        let mut zones = ZoneRegistry::fallback();
        zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .expect("fallback zone should exist")
            .spirit_qi = 0.25;
        app.insert_resource(zones);
        app.add_event::<TribulationFailed>();
        app.add_event::<TribulationFled>();
        app.add_event::<TribulationSettled>();
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<QiTransfer>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(
            Update,
            (
                tribulation_failure_system,
                death_arbiter_tick.after(tribulation_failure_system),
            ),
        );

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 880.0,
                    qi_max: 210.0,
                    last_qi_zero_at: Some(77),
                    pending_material_bonus: 0.3,
                    ..Default::default()
                },
                all_meridians_open(),
                Wounds {
                    health_current: 0.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Stamina::default(),
                CombatState::default(),
                Lifecycle {
                    character_id: char_id.to_string(),
                    death_count: 2,
                    last_death_tick: Some(55),
                    state: LifecycleState::Alive,
                    ..Default::default()
                },
                DeathRegistry {
                    char_id: char_id.to_string(),
                    death_count: 2,
                    last_death_tick: Some(55),
                    prev_death_tick: Some(12),
                    last_death_zone: Some(ZoneDeathKind::Ordinary),
                },
                LifespanComponent {
                    born_at_tick: 0,
                    years_lived: 90.0,
                    cap_by_realm: LifespanCapTable::SPIRIT,
                    offline_pause_tick: None,
                },
                LifeRecord::new(char_id),
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension(DimensionKind::Overworld),
                TribulationState::restored(2, 5, 120),
            ))
            .id();
        let zone_before = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("fallback zone should exist")
            .spirit_qi;

        app.world_mut()
            .resource_mut::<Events<TribulationFailed>>()
            .send(TribulationFailed { entity, wave: 3 });
        app.update();

        let entity_ref = app.world().entity(entity);
        let cultivation = entity_ref
            .get::<Cultivation>()
            .expect("cultivation should remain attached");
        let meridians = entity_ref
            .get::<MeridianSystem>()
            .expect("meridians should remain attached");
        let wounds = entity_ref
            .get::<Wounds>()
            .expect("wounds should remain attached");
        let lifecycle = entity_ref
            .get::<Lifecycle>()
            .expect("lifecycle should remain attached");
        let registry = entity_ref
            .get::<DeathRegistry>()
            .expect("death registry should remain attached");
        let lifespan = entity_ref
            .get::<LifespanComponent>()
            .expect("lifespan should remain attached");

        assert_eq!(cultivation.realm, Realm::Spirit);
        assert_eq!(cultivation.qi_current, 0.0);
        assert_eq!(cultivation.last_qi_zero_at, None);
        assert_eq!(cultivation.pending_material_bonus, 0.0);
        assert_eq!(meridians.opened_count(), Realm::Spirit.required_meridians());
        assert_eq!(cultivation.qi_max, 10.0 + meridians.sum_capacity());
        assert!(wounds.health_current > 0.0);
        assert_eq!(lifecycle.state, LifecycleState::Alive);
        assert_eq!(lifecycle.death_count, 2);
        assert_eq!(lifecycle.last_death_tick, Some(55));
        assert_eq!(registry.death_count, 2);
        assert_eq!(registry.last_death_tick, Some(55));
        assert_eq!(lifespan.years_lived, 90.0);
        assert!(entity_ref.get::<TribulationState>().is_none());
        let zone_after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("fallback zone should exist")
            .spirit_qi;
        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();

        assert_eq!(
            app.world()
                .resource::<Events<CultivationDeathTrigger>>()
                .len(),
            0
        );
        assert_eq!(
            app.world()
                .resource::<Events<DeathInsightRequested>>()
                .len(),
            0
        );
        assert_eq!(app.world().resource::<Events<PlayerTerminated>>().len(), 0);
        assert!(
            zone_after > zone_before,
            "tribulation failure should release cleared qi back to the current zone"
        );
        assert_eq!(transfers.len(), 2);
        assert_eq!(transfers[0].reason, QiTransferReason::ReleaseToZone);
        assert_eq!(transfers[0].to, QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME));
        assert_eq!(
            transfers[1].to,
            QiAccountId::overflow(format!("tribulation_failure:{entity:?}"))
        );
        assert_eq!(transfers[1].reason, QiTransferReason::ReleaseToZone);
        assert!(
            load_active_tribulation(&settings, char_id)
                .expect("active tribulation query should succeed")
                .is_none(),
            "failed tribulation should clear active row"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn intercepted_tribulation_transfers_all_inventory_to_killer() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("intercept-loot-transfer");
        app.insert_resource(settings.clone());
        app.add_event::<DeathEvent>();
        app.add_event::<TribulationSettled>();
        app.add_systems(Update, tribulation_intercept_death_system);

        let victim = app
            .world_mut()
            .spawn((
                Lifecycle {
                    character_id: "offline:Victim".to_string(),
                    ..Default::default()
                },
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(2),
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 2,
                    waves_total: 3,
                    started_tick: 0,
                    phase_started_tick: 0,
                    next_wave_tick: 0,
                    participants: vec!["offline:Victim".to_string(), "offline:Killer".to_string()],
                    failed: false,
                },
                test_inventory(vec![test_item(101), test_item(102)], 7),
            ))
            .id();
        let killer = app
            .world_mut()
            .spawn((
                test_inventory(vec![test_item(201)], 3),
                LifeRecord::new("offline:Killer"),
            ))
            .id();

        app.world_mut().send_event(DeathEvent {
            target: victim,
            cause: "pvp:offline:Killer".to_string(),
            attacker: Some(killer),
            attacker_player_id: Some("offline:Killer".to_string()),
            at_tick: 120,
        });

        app.update();

        let victim_inventory = app
            .world()
            .get::<PlayerInventory>(victim)
            .expect("victim inventory should remain attached");
        assert_eq!(victim_inventory.bone_coins, 0);
        assert!(victim_inventory
            .containers
            .iter()
            .all(|container| container.items.is_empty()));
        assert!(victim_inventory.equipped.is_empty());
        assert!(victim_inventory.hotbar.iter().all(Option::is_none));

        let killer_inventory = app
            .world()
            .get::<PlayerInventory>(killer)
            .expect("killer inventory should remain attached");
        assert_eq!(killer_inventory.bone_coins, 10);
        let killer_item_ids = killer_inventory
            .containers
            .iter()
            .flat_map(|container| container.items.iter())
            .map(|placed| placed.instance.instance_id)
            .collect::<Vec<_>>();
        assert!(killer_item_ids.contains(&101));
        assert!(killer_item_ids.contains(&102));
        assert!(killer_item_ids.contains(&201));

        let killer_life_record = app
            .world()
            .get::<LifeRecord>(killer)
            .expect("killer life record should remain attached");
        assert!(matches!(
            killer_life_record.biography.last(),
            Some(BiographyEntry::TribulationIntercepted { victim_id, tag, tick })
                if victim_id == "offline:Victim" && tag == "戮道者 · 截劫" && *tick == 120
        ));

        assert!(app.world().get::<TribulationState>(victim).is_none());
        let settled = app.world().resource::<Events<TribulationSettled>>();
        let emitted: Vec<_> = settled.get_reader().read(settled).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].result.outcome, DuXuOutcomeV1::Killed);
        assert_eq!(emitted[0].result.killer.as_deref(), Some("offline:Killer"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unregistered_player_kill_does_not_claim_interception_settlement() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("intercept-killer-must-be-participant");
        app.insert_resource(settings);
        app.add_event::<DeathEvent>();
        app.add_event::<TribulationSettled>();
        app.add_systems(Update, tribulation_intercept_death_system);

        let victim = app
            .world_mut()
            .spawn((
                Lifecycle {
                    character_id: "offline:Victim".to_string(),
                    ..Default::default()
                },
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(2),
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 2,
                    waves_total: 3,
                    started_tick: 0,
                    phase_started_tick: 0,
                    next_wave_tick: 0,
                    participants: vec!["offline:Victim".to_string()],
                    failed: false,
                },
                test_inventory(vec![test_item(101)], 7),
            ))
            .id();
        let killer = app
            .world_mut()
            .spawn((
                test_inventory(vec![test_item(201)], 3),
                LifeRecord::new("offline:Killer"),
            ))
            .id();

        app.world_mut().send_event(DeathEvent {
            target: victim,
            cause: "pvp:offline:Killer".to_string(),
            attacker: Some(killer),
            attacker_player_id: Some("offline:Killer".to_string()),
            at_tick: 120,
        });

        app.update();

        assert!(app.world().get::<TribulationState>(victim).is_some());
        assert_eq!(
            app.world().resource::<Events<TribulationSettled>>().len(),
            0
        );
        let victim_inventory = app.world().get::<PlayerInventory>(victim).unwrap();
        assert_eq!(victim_inventory.bone_coins, 7);
        let killer_inventory = app.world().get::<PlayerInventory>(killer).unwrap();
        assert_eq!(killer_inventory.bone_coins, 3);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn attacking_locked_tribulator_records_interceptor_participant() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_systems(Update, record_tribulation_interceptor_system);

        let victim = app
            .world_mut()
            .spawn((
                Position::new([0.0, 66.0, 0.0]),
                Lifecycle {
                    character_id: "offline:Victim".to_string(),
                    ..Default::default()
                },
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Lock,
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 0,
                    waves_total: 3,
                    started_tick: 0,
                    phase_started_tick: 0,
                    next_wave_tick: 0,
                    participants: vec!["offline:Victim".to_string()],
                    failed: false,
                },
            ))
            .id();
        let interceptor = app
            .world_mut()
            .spawn((
                Position::new([12.0, 66.0, 0.0]),
                Lifecycle {
                    character_id: "offline:Killer".to_string(),
                    ..Default::default()
                },
            ))
            .id();

        for _ in 0..2 {
            app.world_mut().send_event(CombatEvent {
                attacker: interceptor,
                target: victim,
                resolved_at_tick: 120,
                body_part: BodyPart::Chest,
                wound_kind: WoundKind::Cut,
                source: crate::combat::events::AttackSource::Melee,
                debug_command: false,
                physical_damage: 0.0,
                damage: 12.0,
                contam_delta: 0.0,
                description: "test interception hit".to_string(),
                defense_kind: None,
                defense_effectiveness: None,
                defense_contam_reduced: None,
                defense_wound_severity: None,
            });
        }
        app.update();

        let state = app
            .world()
            .get::<TribulationState>(victim)
            .expect("tribulation should remain active");
        assert_eq!(
            state.participants,
            vec!["offline:Victim".to_string(), "offline:Killer".to_string()]
        );
    }

    #[test]
    fn attacking_during_heart_demon_records_interceptor_participant() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_systems(Update, record_tribulation_interceptor_system);

        let victim = app
            .world_mut()
            .spawn((
                Position::new([0.0, 66.0, 0.0]),
                Lifecycle {
                    character_id: "offline:Victim".to_string(),
                    ..Default::default()
                },
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::HeartDemon,
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 4,
                    waves_total: 5,
                    started_tick: 0,
                    phase_started_tick: 2100,
                    next_wave_tick: 2400,
                    participants: vec!["offline:Victim".to_string()],
                    failed: false,
                },
            ))
            .id();
        let interceptor = app
            .world_mut()
            .spawn((
                Position::new([12.0, 66.0, 0.0]),
                Lifecycle {
                    character_id: "offline:Killer".to_string(),
                    ..Default::default()
                },
            ))
            .id();

        app.world_mut().send_event(CombatEvent {
            attacker: interceptor,
            target: victim,
            resolved_at_tick: 2130,
            body_part: BodyPart::Chest,
            wound_kind: WoundKind::Cut,
            source: crate::combat::events::AttackSource::Melee,
            debug_command: false,
            physical_damage: 0.0,
            damage: 12.0,
            contam_delta: 0.0,
            description: "test heart demon interception hit".to_string(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        });
        app.update();

        let state = app
            .world()
            .get::<TribulationState>(victim)
            .expect("tribulation should remain active");
        assert_eq!(
            state.participants,
            vec!["offline:Victim".to_string(), "offline:Killer".to_string()]
        );
    }

    #[test]
    fn attacking_tribulator_from_other_dimension_does_not_record_interceptor() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_systems(Update, record_tribulation_interceptor_system);

        let victim = app
            .world_mut()
            .spawn((
                Position::new([0.0, 66.0, 0.0]),
                CurrentDimension(DimensionKind::Overworld),
                Lifecycle {
                    character_id: "offline:Victim".to_string(),
                    ..Default::default()
                },
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Lock,
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 0,
                    waves_total: 3,
                    started_tick: 0,
                    phase_started_tick: 120,
                    next_wave_tick: 300,
                    participants: vec!["offline:Victim".to_string()],
                    failed: false,
                },
            ))
            .id();
        let interceptor = app
            .world_mut()
            .spawn((
                Position::new([12.0, 66.0, 0.0]),
                CurrentDimension(DimensionKind::Tsy),
                Lifecycle {
                    character_id: "offline:Killer".to_string(),
                    ..Default::default()
                },
            ))
            .id();

        app.world_mut().send_event(CombatEvent {
            attacker: interceptor,
            target: victim,
            resolved_at_tick: 120,
            body_part: BodyPart::Chest,
            wound_kind: WoundKind::Cut,
            source: crate::combat::events::AttackSource::Melee,
            debug_command: false,
            physical_damage: 0.0,
            damage: 12.0,
            contam_delta: 0.0,
            description: "test cross-dimension interception hit".to_string(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        });
        app.update();

        let state = app
            .world()
            .get::<TribulationState>(victim)
            .expect("tribulation should remain active");
        assert_eq!(state.participants, vec!["offline:Victim".to_string()]);
    }

    #[test]
    fn attacking_restored_tribulator_preserves_primary_participant() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_systems(Update, record_tribulation_interceptor_system);

        let victim = app
            .world_mut()
            .spawn((
                Position::new([0.0, 66.0, 0.0]),
                Lifecycle {
                    character_id: "offline:Victim".to_string(),
                    ..Default::default()
                },
                TribulationState::restored(1, 3, 0),
            ))
            .id();
        let interceptor = app
            .world_mut()
            .spawn((
                Position::new([12.0, 66.0, 0.0]),
                Lifecycle {
                    character_id: "offline:Killer".to_string(),
                    ..Default::default()
                },
            ))
            .id();

        app.world_mut().send_event(CombatEvent {
            attacker: interceptor,
            target: victim,
            resolved_at_tick: 120,
            body_part: BodyPart::Chest,
            wound_kind: WoundKind::Cut,
            source: crate::combat::events::AttackSource::Melee,
            debug_command: false,
            physical_damage: 0.0,
            damage: 12.0,
            contam_delta: 0.0,
            description: "test restored interception hit".to_string(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        });
        app.update();

        let state = app
            .world()
            .get::<TribulationState>(victim)
            .expect("tribulation should remain active");
        assert_eq!(
            state.participants,
            vec!["offline:Victim".to_string(), "offline:Killer".to_string()]
        );
    }

    #[test]
    fn registered_interceptor_dies_to_aoe_without_failing_tribulation() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 300 });
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, tribulation_aoe_system);

        app.world_mut().spawn((
            Position::new([0.0, 66.0, 0.0]),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 120.0,
                qi_max: 210.0,
                ..Default::default()
            },
            Wounds {
                health_current: 100.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Lifecycle {
                character_id: "offline:Victim".to_string(),
                ..Default::default()
            },
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(1),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 1,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 300,
                next_wave_tick: 300,
                participants: vec!["offline:Victim".to_string(), "offline:Killer".to_string()],
                failed: false,
            },
        ));
        let interceptor = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 50.0,
                    qi_max: 80.0,
                    ..Default::default()
                },
                Wounds {
                    health_current: 1.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: "offline:Killer".to_string(),
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        assert_eq!(app.world().resource::<Events<TribulationFailed>>().len(), 0);
        let deaths = app.world().resource::<Events<DeathEvent>>();
        let emitted: Vec<_> = deaths.get_reader().read(deaths).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].target, interceptor);
        assert_eq!(emitted[0].cause, "观劫而亡");
        assert_eq!(emitted[0].attacker_player_id, None);
    }

    #[test]
    fn spectator_death_by_tribulation_aoe_is_written_to_life_record() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("spectator-death-biography");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 300 });
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(
            Update,
            (
                tribulation_aoe_system,
                death_arbiter_tick.after(tribulation_aoe_system),
            ),
        );

        app.world_mut().spawn((
            Position::new([0.0, 66.0, 0.0]),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 120.0,
                qi_max: 210.0,
                ..Default::default()
            },
            Wounds {
                health_current: 100.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Lifecycle {
                character_id: "offline:Victim".to_string(),
                state: LifecycleState::Alive,
                ..Default::default()
            },
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(1),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 1,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 300,
                next_wave_tick: 300,
                participants: vec!["offline:Victim".to_string()],
                failed: false,
            },
        ));
        let spectator = app
            .world_mut()
            .spawn((
                Position::new([20.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Awaken,
                    qi_current: 10.0,
                    qi_max: 40.0,
                    ..Default::default()
                },
                Wounds {
                    health_current: 1.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Stamina::default(),
                CombatState::default(),
                Lifecycle {
                    character_id: "offline:Spectator".to_string(),
                    state: LifecycleState::Alive,
                    fortune_remaining: 1,
                    ..Default::default()
                },
                DeathRegistry::new("offline:Spectator".to_string()),
                LifespanComponent::new(LifespanCapTable::AWAKEN),
                LifeRecord::new("offline:Spectator"),
            ))
            .id();

        app.update();

        let lifecycle = app
            .world()
            .get::<Lifecycle>(spectator)
            .expect("spectator lifecycle should remain attached");
        assert_eq!(lifecycle.state, LifecycleState::NearDeath);
        let life = app
            .world()
            .get::<LifeRecord>(spectator)
            .expect("spectator life record should remain attached");
        assert!(matches!(
            life.biography.last(),
            Some(BiographyEntry::NearDeath { cause, tick }) if cause == "观劫而亡" && *tick == 300
        ));
        assert_eq!(app.world().resource::<Events<TribulationFailed>>().len(), 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn disconnecting_during_tribulation_flees_and_regresses_without_death() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("disconnect-fled");
        let char_id = "offline:Azure";
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: char_id.to_string(),
                kind: "du_xu".to_string(),
                source: String::new(),
                origin_dimension: Some("minecraft:overworld".to_string()),
                wave_current: 1,
                waves_total: 3,
                started_tick: 80,
                epicenter: [0.0, 64.0, 0.0],
                intensity: 0.0,
            },
        )
        .expect("active tribulation should persist before disconnect");

        app.insert_resource(settings.clone());
        app.insert_resource(CombatClock { tick: 320 });
        app.add_event::<TribulationSettled>();
        app.add_event::<TribulationFled>();
        app.add_systems(Update, abort_du_xu_on_client_removed);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 120.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
                Wounds {
                    health_current: 0.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: char_id.to_string(),
                    state: LifecycleState::Alive,
                    ..Default::default()
                },
                LifeRecord::new(char_id),
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(2),
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 1,
                    waves_total: 3,
                    started_tick: 80,
                    phase_started_tick: 280,
                    next_wave_tick: 320,
                    participants: vec![char_id.to_string()],
                    failed: false,
                },
            ))
            .id();

        app.world_mut().entity_mut(entity).remove::<Client>();
        app.update();

        assert!(app.world().get::<TribulationState>(entity).is_none());
        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.realm, Realm::Spirit);
        assert_eq!(cultivation.qi_current, 0.0);
        let life = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("life record should remain attached");
        assert!(matches!(
            life.biography.last(),
            Some(BiographyEntry::TribulationFled { wave: 2, tick: 320 })
        ));

        let settled: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .drain()
            .collect();
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0].result.outcome, DuXuOutcomeV1::Fled);
        assert_eq!(settled[0].result.waves_survived, 1);
        let fled: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<TribulationFled>>()
            .drain()
            .collect();
        assert_eq!(fled.len(), 1);
        assert_eq!(fled[0].entity, entity);
        assert_eq!(fled[0].tick, 320);
        assert!(
            load_active_tribulation(&settings, char_id)
                .expect("active tribulation query should succeed")
                .is_none(),
            "fled tribulation should clear active row"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn leaving_lock_radius_flees_and_regresses_without_death() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("boundary-fled");
        let char_id = "offline:Azure";
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: char_id.to_string(),
                kind: "du_xu".to_string(),
                source: String::new(),
                origin_dimension: Some("minecraft:overworld".to_string()),
                wave_current: 1,
                waves_total: 3,
                started_tick: 80,
                epicenter: [0.0, 64.0, 0.0],
                intensity: 0.0,
            },
        )
        .expect("active tribulation should persist before flee");

        app.insert_resource(settings.clone());
        app.insert_resource(CombatClock { tick: 340 });
        app.add_event::<TribulationSettled>();
        app.add_event::<TribulationFled>();
        app.add_systems(Update, tribulation_escape_boundary_system);

        let entity = app
            .world_mut()
            .spawn((
                Position::new([30.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 160.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
                Wounds {
                    health_current: 0.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: char_id.to_string(),
                    state: LifecycleState::Alive,
                    ..Default::default()
                },
                LifeRecord::new(char_id),
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Lock,
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 1,
                    waves_total: 3,
                    started_tick: 80,
                    phase_started_tick: 300,
                    next_wave_tick: 360,
                    participants: vec![char_id.to_string()],
                    failed: false,
                },
            ))
            .id();

        app.update();

        assert!(app.world().get::<TribulationState>(entity).is_none());
        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.realm, Realm::Spirit);
        assert_eq!(cultivation.qi_current, 0.0);
        let life = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("life record should remain attached");
        assert!(matches!(
            life.biography.last(),
            Some(BiographyEntry::TribulationFled { wave: 2, tick: 340 })
        ));

        let settled: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .drain()
            .collect();
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0].result.outcome, DuXuOutcomeV1::Fled);
        assert_eq!(settled[0].result.waves_survived, 1);
        assert!(
            load_active_tribulation(&settings, char_id)
                .expect("active tribulation query should succeed")
                .is_none(),
            "fled tribulation should clear active row"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn changing_dimension_during_lock_flees_even_inside_radius() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("dimension-fled");
        let char_id = "offline:Azure";
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: char_id.to_string(),
                kind: "du_xu".to_string(),
                source: String::new(),
                origin_dimension: Some("minecraft:overworld".to_string()),
                wave_current: 1,
                waves_total: 3,
                started_tick: 80,
                epicenter: [0.0, 64.0, 0.0],
                intensity: 0.0,
            },
        )
        .expect("active tribulation should persist before flee");

        app.insert_resource(settings.clone());
        app.insert_resource(CombatClock { tick: 345 });
        app.add_event::<TribulationSettled>();
        app.add_event::<TribulationFled>();
        app.add_systems(Update, tribulation_escape_boundary_system);

        let entity = app
            .world_mut()
            .spawn((
                Position::new([0.0, 66.0, 0.0]),
                CurrentDimension(DimensionKind::Tsy),
                TribulationOriginDimension(DimensionKind::Overworld),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 160.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
                Wounds {
                    health_current: 40.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: char_id.to_string(),
                    state: LifecycleState::Alive,
                    ..Default::default()
                },
                LifeRecord::new(char_id),
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Lock,
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 1,
                    waves_total: 3,
                    started_tick: 80,
                    phase_started_tick: 300,
                    next_wave_tick: 360,
                    participants: vec![char_id.to_string()],
                    failed: false,
                },
            ))
            .id();

        app.update();

        assert!(app.world().get::<TribulationState>(entity).is_none());
        assert!(app
            .world()
            .get::<TribulationOriginDimension>(entity)
            .is_none());
        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.realm, Realm::Spirit);
        assert_eq!(cultivation.qi_current, 0.0);
        let settled: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .drain()
            .collect();
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0].result.outcome, DuXuOutcomeV1::Fled);
        let fled: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<TribulationFled>>()
            .drain()
            .collect();
        assert_eq!(fled.len(), 1);
        assert_eq!(fled[0].entity, entity);
        assert_eq!(fled[0].tick, 345);
        assert!(
            load_active_tribulation(&settings, char_id)
                .expect("active tribulation query should succeed")
                .is_none(),
            "dimension flee should clear active row"
        );

        let _ = fs::remove_dir_all(root);
    }

    // ─────────────────────────────────────────────────────────────
    // plan-halfstep-buff-v1 P0：遥测 + quota 满时长追踪测试
    // ─────────────────────────────────────────────────────────────

    fn make_settled_event(entity: Entity, outcome: DuXuOutcomeV1) -> TribulationSettled {
        TribulationSettled {
            entity,
            kind: TribulationKind::JueBi,
            source: Some(JueBiTriggerSource::VoidQuotaExceeded),
            result: DuXuResultV1 {
                char_id: "halfstep_test_char".to_string(),
                outcome,
                killer: None,
                waves_survived: 3,
                reason: Some("halfstep_test".to_string()),
            },
        }
    }

    fn p0_metrics_test_app() -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.init_resource::<TribulationMetrics>();
        app.init_resource::<QuotaFullTracker>();
        app.insert_resource(WorldQiBudget::from_total(100.0));
        app.insert_resource(VoidQuotaConfig::default());
        app.add_event::<TribulationSettled>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<AscensionQuotaOccupied>();
        app.add_event::<QiTransfer>();
        app.add_systems(
            Update,
            (
                track_tribulation_metrics_system,
                track_quota_full_duration_system,
            ),
        );
        app
    }

    /// 在测试 app 里 spawn 一个有 Cultivation + LifespanComponent 的 entity，
    /// 用于 P1 buff 应用相关测试。
    fn spawn_halfstep_candidate(
        app: &mut App,
        initial_qi_max: f64,
        initial_lifespan_cap: u32,
    ) -> Entity {
        app.world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 0.0,
                    qi_max: initial_qi_max,
                    ..Default::default()
                },
                LifespanComponent::new(initial_lifespan_cap),
            ))
            .id()
    }

    #[test]
    fn track_metrics_increments_halfstep_counter_per_event() {
        let mut app = p0_metrics_test_app();
        let dummy = app.world_mut().spawn(()).id();
        for _ in 0..10 {
            app.world_mut()
                .resource_mut::<Events<TribulationSettled>>()
                .send(make_settled_event(dummy, DuXuOutcomeV1::HalfStep));
        }
        app.update();
        let metrics = app.world().resource::<TribulationMetrics>();
        assert_eq!(
            metrics.halfstep_count, 10,
            "10 HalfStep settlements should increment counter to 10 (got {}); P0 验收 \
             — mock 10 halfstep 后 counter == 10",
            metrics.halfstep_count
        );
        assert_eq!(
            metrics.ascended_count, 0,
            "ascended_count must stay 0 when only HalfStep events fire; bleed-over bug"
        );
    }

    #[test]
    fn track_metrics_increments_ascended_counter_per_event() {
        let mut app = p0_metrics_test_app();
        let dummy = app.world_mut().spawn(()).id();
        for _ in 0..5 {
            app.world_mut()
                .resource_mut::<Events<TribulationSettled>>()
                .send(make_settled_event(dummy, DuXuOutcomeV1::Ascended));
        }
        app.update();
        let metrics = app.world().resource::<TribulationMetrics>();
        assert_eq!(metrics.ascended_count, 5);
        assert_eq!(metrics.halfstep_count, 0);
    }

    #[test]
    fn track_metrics_ignores_failed_killed_fled_outcomes() {
        let mut app = p0_metrics_test_app();
        let dummy = app.world_mut().spawn(()).id();
        for outcome in [
            DuXuOutcomeV1::Killed,
            DuXuOutcomeV1::Failed,
            DuXuOutcomeV1::Fled,
        ] {
            app.world_mut()
                .resource_mut::<Events<TribulationSettled>>()
                .send(make_settled_event(dummy, outcome));
        }
        app.update();
        let metrics = app.world().resource::<TribulationMetrics>();
        assert_eq!(
            metrics.halfstep_count, 0,
            "Killed/Failed/Fled outcomes must not touch halfstep counter"
        );
        assert_eq!(metrics.ascended_count, 0);
    }

    #[test]
    fn track_metrics_inserts_halfstep_state_with_correct_entered_at() {
        let mut app = p0_metrics_test_app();
        app.world_mut().resource_mut::<CombatClock>().tick = 12345;
        let target = app.world_mut().spawn(()).id();
        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(make_settled_event(target, DuXuOutcomeV1::HalfStep));
        app.update();
        let state = app
            .world()
            .get::<HalfStepState>(target)
            .copied()
            .expect("HalfStepState component must be inserted on HalfStep settlement");
        assert_eq!(state.entered_at, 12345, "entered_at must equal clock.tick");
        assert_eq!(
            state.rechallenge_window_until,
            12345 + RECHALLENGE_WINDOW_TICKS,
            "rechallenge_window_until must be entered_at + 7d (§8 Q1)"
        );
        assert!(!state.buff_applied, "buff_applied starts false (§8 Q4 守卫)");
    }

    #[test]
    fn track_metrics_does_not_insert_halfstep_state_for_non_halfstep_outcomes() {
        let mut app = p0_metrics_test_app();
        let target = app.world_mut().spawn(()).id();
        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(make_settled_event(target, DuXuOutcomeV1::Ascended));
        app.update();
        assert!(
            app.world().get::<HalfStepState>(target).is_none(),
            "Ascended outcome must NOT insert HalfStepState; bleed-over would put 化虚 修士 in 半步 queue"
        );
    }

    #[test]
    fn quota_full_tracker_starts_full_period_on_occupied_event() {
        let mut app = p0_metrics_test_app();
        // limit = floor(100 / DEFAULT_VOID_QUOTA_K=50) = 2，先令 occupied 达到 limit
        app.world_mut().resource_mut::<CombatClock>().tick = 100;
        app.world_mut()
            .resource_mut::<Events<AscensionQuotaOccupied>>()
            .send(AscensionQuotaOccupied { occupied_slots: 2 });
        app.update();
        let tracker = app.world().resource::<QuotaFullTracker>();
        assert_eq!(tracker.current_occupied, 2);
        assert_eq!(
            tracker.current_limit, 2,
            "limit should be derived from check_void_quota(WorldQiBudget=100, k=50)"
        );
        assert_eq!(
            tracker.full_since_tick,
            Some(100),
            "进入 full 状态应记录 full_since_tick=current clock"
        );
    }

    #[test]
    fn quota_full_tracker_ends_period_and_accumulates_on_opened_event() {
        let mut app = p0_metrics_test_app();
        app.world_mut().resource_mut::<CombatClock>().tick = 100;
        app.world_mut()
            .resource_mut::<Events<AscensionQuotaOccupied>>()
            .send(AscensionQuotaOccupied { occupied_slots: 2 });
        app.update();
        // 时间推进 500 ticks，quota 名额空出
        app.world_mut().resource_mut::<CombatClock>().tick = 600;
        app.world_mut()
            .resource_mut::<Events<AscensionQuotaOpened>>()
            .send(AscensionQuotaOpened { occupied_slots: 1 });
        app.update();
        let metrics = app.world().resource::<TribulationMetrics>();
        let tracker = app.world().resource::<QuotaFullTracker>();
        assert_eq!(
            metrics.quota_full_duration_ticks, 500,
            "离开 full 状态后应把 (600-100)=500 写入累计计数；off-by-one 或漏算"
        );
        assert!(
            tracker.full_since_tick.is_none(),
            "离开 full 状态后 full_since_tick 必须清空"
        );
    }

    #[test]
    fn current_quota_full_duration_includes_pending_window() {
        let mut app = p0_metrics_test_app();
        app.world_mut().resource_mut::<CombatClock>().tick = 100;
        app.world_mut()
            .resource_mut::<Events<AscensionQuotaOccupied>>()
            .send(AscensionQuotaOccupied { occupied_slots: 2 });
        app.update();
        let metrics = *app.world().resource::<TribulationMetrics>();
        let tracker = *app.world().resource::<QuotaFullTracker>();
        // 当前仍在 full 状态：current_tick=200，pending = 200 - 100 = 100
        let observed = current_quota_full_duration_ticks(&metrics, &tracker, 200);
        assert_eq!(
            observed, 100,
            "current 函数应把 pending (current_tick - full_since_tick) 加到 base 上"
        );
    }

    #[test]
    fn halfstep_state_is_within_window_boundary() {
        let state = HalfStepState::new(1000);
        let window_end = 1000 + RECHALLENGE_WINDOW_TICKS;
        assert!(state.is_within_window(1000), "进入 tick 应在窗口内");
        assert!(state.is_within_window(window_end), "等于窗口末端应在窗口内（闭区间）");
        assert!(
            !state.is_within_window(window_end + 1),
            "超过窗口末端 1 tick 必须不在窗口内（边界一致性）"
        );
    }

    // ─────────────────────────────────────────────────────────────
    // plan-halfstep-buff-v1 P1：buff 实装 + qi_physics ledger + 不叠加守卫
    // ─────────────────────────────────────────────────────────────

    fn collect_qi_transfers(app: &mut App) -> Vec<QiTransfer> {
        app.world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect()
    }

    #[test]
    fn halfstep_buff_applies_qi_max_and_lifespan_on_first_settlement() {
        let mut app = p0_metrics_test_app();
        let entity = spawn_halfstep_candidate(&mut app, 1000.0, LifespanCapTable::SPIRIT);
        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
        app.update();

        let cultivation = app.world().get::<Cultivation>(entity).expect("cultivation");
        let lifespan = app.world().get::<LifespanComponent>(entity).expect("lifespan");
        let state = app.world().get::<HalfStepState>(entity).expect("halfstep state");

        // qi_max × (1.0 + 0.10) = 1100.0
        let expected_qi_max = 1000.0 * (1.0 + HALFSTEP_QI_MAX_BONUS as f64);
        assert!(
            (cultivation.qi_max - expected_qi_max).abs() < 1e-6,
            "qi_max expected {expected_qi_max} but got {} (HALFSTEP_QI_MAX_BONUS={HALFSTEP_QI_MAX_BONUS})",
            cultivation.qi_max
        );
        assert_eq!(
            lifespan.cap_by_realm,
            LifespanCapTable::SPIRIT + HALFSTEP_LIFESPAN_BONUS_YEARS,
            "lifespan cap should increment by HALFSTEP_LIFESPAN_BONUS_YEARS={HALFSTEP_LIFESPAN_BONUS_YEARS}"
        );
        assert!(
            state.buff_applied,
            "buff_applied must be true after first HalfStep settlement on entity with Cultivation"
        );
    }

    #[test]
    fn halfstep_buff_emits_audit_qi_transfer_event() {
        let mut app = p0_metrics_test_app();
        let entity = spawn_halfstep_candidate(&mut app, 1000.0, LifespanCapTable::SPIRIT);
        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
        app.update();
        let transfers = collect_qi_transfers(&mut app);

        let halfstep_transfers: Vec<&QiTransfer> = transfers
            .iter()
            .filter(|t| matches!(t.reason, QiTransferReason::HalfStepBuff))
            .collect();
        assert_eq!(
            halfstep_transfers.len(),
            1,
            "expected exactly 1 HalfStepBuff QiTransfer event (got {}); transfers={transfers:?}",
            halfstep_transfers.len()
        );
        let transfer = halfstep_transfers[0];
        // bonus = 1000 × 0.10 = 100
        let expected_bonus = 1000.0 * HALFSTEP_QI_MAX_BONUS as f64;
        assert!(
            (transfer.amount - expected_bonus).abs() < 1e-6,
            "amount expected {expected_bonus} got {}",
            transfer.amount
        );
        assert_eq!(transfer.from, QiAccountId::tiandao());
        assert_eq!(transfer.to, QiAccountId::player("halfstep_test_char"));
    }

    #[test]
    fn halfstep_buff_not_reapplied_when_state_already_marks_buff_applied() {
        let mut app = p0_metrics_test_app();
        let entity = spawn_halfstep_candidate(&mut app, 1000.0, LifespanCapTable::SPIRIT);
        // Pre-insert HalfStepState 标记 buff_applied=true（模拟二次进入 HalfStep 的状态）
        app.world_mut().entity_mut(entity).insert(HalfStepState {
            entered_at: 100,
            rechallenge_window_until: 100 + RECHALLENGE_WINDOW_TICKS,
            buff_applied: true,
        });
        // 触发新一次 HalfStep settlement
        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
        app.update();

        let cultivation = app.world().get::<Cultivation>(entity).unwrap();
        let lifespan = app.world().get::<LifespanComponent>(entity).unwrap();
        let state = app.world().get::<HalfStepState>(entity).unwrap();
        assert_eq!(
            cultivation.qi_max, 1000.0,
            "qi_max must stay 1000.0 (not re-buffed); reapply would yield 1100.0 — §8 Q4 守卫失效"
        );
        assert_eq!(
            lifespan.cap_by_realm,
            LifespanCapTable::SPIRIT,
            "lifespan cap must stay at SPIRIT base; reapply守卫失效"
        );
        assert_eq!(
            state.entered_at, 100,
            "entered_at must preserve original value (rechallenge window 起点稳定)"
        );
        let transfers = collect_qi_transfers(&mut app);
        let halfstep_transfers: Vec<&QiTransfer> = transfers
            .iter()
            .filter(|t| matches!(t.reason, QiTransferReason::HalfStepBuff))
            .collect();
        assert!(
            halfstep_transfers.is_empty(),
            "no ledger emit on reapply skip; got {halfstep_transfers:?}"
        );
        // metric 仍累计（halfstep_count 计的是 settlement 次数，不是 buff 次数）
        assert_eq!(
            app.world().resource::<TribulationMetrics>().halfstep_count,
            1,
            "halfstep_count tracks settlement events, not unique entities; should still increment"
        );
    }

    #[test]
    fn halfstep_buff_applies_to_qi_max_only_when_lifespan_component_absent() {
        let mut app = p0_metrics_test_app();
        let entity = app
            .world_mut()
            .spawn(Cultivation {
                realm: Realm::Spirit,
                qi_current: 0.0,
                qi_max: 500.0,
                ..Default::default()
            })
            .id();
        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
        app.update();
        let cultivation = app.world().get::<Cultivation>(entity).unwrap();
        let expected = 500.0 * (1.0 + HALFSTEP_QI_MAX_BONUS as f64);
        assert!(
            (cultivation.qi_max - expected).abs() < 1e-6,
            "qi_max should still buff even without lifespan; got {}",
            cultivation.qi_max
        );
        let state = app.world().get::<HalfStepState>(entity).unwrap();
        assert!(state.buff_applied);
    }

    #[test]
    fn halfstep_buff_skipped_when_entity_lacks_cultivation_and_state_left_unbuffed() {
        let mut app = p0_metrics_test_app();
        // 仅 bare entity（无 Cultivation），模拟 dormant NPC / 测试 stub
        let entity = app.world_mut().spawn(()).id();
        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
        app.update();

        let state = app
            .world()
            .get::<HalfStepState>(entity)
            .expect("HalfStepState 仍应插入（P3 重渡队列需要）");
        assert!(
            !state.buff_applied,
            "无 Cultivation entity 应留 buff_applied=false，等待 hydrate 后由后续 settlement 应用；\
             直接置 true 会丢失「未来需补 buff」信号"
        );
        // ledger 不应 emit
        let transfers = collect_qi_transfers(&mut app);
        assert!(
            transfers
                .iter()
                .all(|t| !matches!(t.reason, QiTransferReason::HalfStepBuff)),
            "无 Cultivation 路径不应触发 HalfStepBuff 转账事件"
        );
    }
}
