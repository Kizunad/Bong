use std::collections::HashMap;

use valence::prelude::{
    bevy_ecs, App, BlockPos, Commands, Component, DVec3, Entity, EventWriter, Position, Query, Res,
    ResMut, Resource, Update, With,
};

use crate::cultivation::components::Cultivation;
use crate::cultivation::tick::CultivationClock;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::player::gameplay::PendingGameplayNarrations;
use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
use crate::qi_physics::ledger::{
    pending_inflow_account, transfer_ledger_qi_to_zone, transfer_zone_qi_to_ledger, QiAccountId,
    QiTransfer, QiTransferReason, WorldQiAccount,
};
use crate::qi_physics::QiPhysicsError;
use crate::schema::common::NarrationStyle;
use crate::schema::pseudo_vein::PseudoVeinSeasonV1;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::dimension::DimensionKind;
use crate::world::season::{query_season, Season};
use crate::world::zone::{Zone, ZoneRegistry};
use crate::worldgen::pseudo_vein::{
    build_dissipate_event, storm_hotspots_from_event, PseudoVeinStormHotspot,
};

pub const PSEUDO_VEIN_RISING_TICKS: u64 = 600;
pub const PSEUDO_VEIN_DISSIPATING_TICKS: u64 = 600;
pub const PSEUDO_VEIN_BASE_DURATION_TICKS: u64 = 36_000;
/// plan-zone-qi-economy-v1 P3 §8.1 决议 #3 — 0.6 → 0.85（提升前已 grep 确认仅本文件 +
/// `network::command_executor` 引用该常量符号，无硬编码下游依赖该具体数值）。灵潮窗口目标
/// 必须显著高于 `MIN_ZONE_QI_TO_GUYUAN`（0.80，`cultivation::breakthrough`），否则灵潮窗口
/// 期内固元突破仍会被环境门槛拒绝，整个"灵潮补足固元窗口"机制形同虚设。
pub const PSEUDO_VEIN_MAX_QI: f64 = 0.85;
pub const PSEUDO_VEIN_WARNING_QI: f64 = 0.3;
#[allow(dead_code)]
pub const PSEUDO_VEIN_CRITICAL_DRAIN_RATE: f64 = 0.02;
#[allow(dead_code)]
pub const PSEUDO_VEIN_CRITICAL_PLAYER_DENSITY: u32 = 4;
pub const PSEUDO_VEIN_INFLUENCE_RADIUS_BLOCKS: f64 = 30.0;
const PSEUDO_VEIN_VISUAL_PERIOD_TICKS: u64 = 100;
const PSEUDO_VEIN_FALLBACK_EVAL_PERIOD_TICKS: u64 = 12_000;
const PSEUDO_VEIN_AFTERMATH_TICKS: u64 = 600;
const ZONE_SPIRIT_QI_MIN: f64 = -1.0;
const ZONE_SPIRIT_QI_MAX: f64 = 1.0;
pub const PSEUDO_VEIN_RISING_VFX_EVENT_ID: &str = "bong:pseudo_vein_rising";
pub const PSEUDO_VEIN_ACTIVE_VFX_EVENT_ID: &str = "bong:pseudo_vein_active";
pub const PSEUDO_VEIN_WARNING_VFX_EVENT_ID: &str = "bong:pseudo_vein_warning";
pub const PSEUDO_VEIN_DISSIPATING_VFX_EVENT_ID: &str = "bong:pseudo_vein_dissipating";
pub const PSEUDO_VEIN_AFTERMATH_VFX_EVENT_ID: &str = "bong:pseudo_vein_aftermath";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PseudoVeinPhase {
    Rising,
    Active,
    Warning,
    Dissipating,
    StormAftermath,
}

#[derive(Debug, Clone, Component, PartialEq)]
pub struct PseudoVeinRuntime {
    pub zone_id: String,
    pub center_pos: BlockPos,
    pub current_qi: f64,
    pub max_qi: f64,
    pub base_duration_ticks: u64,
    pub started_at_tick: u64,
    pub phase: PseudoVeinPhase,
    pub cultivators_in_range: u32,
    pub season_at_spawn: PseudoVeinSeasonV1,
    pub injected_qi: f64,
    phase_started_at_tick: u64,
    last_tick: u64,
    last_visual_tick: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PseudoVeinTickOutcome {
    pub phase: PseudoVeinPhase,
    pub current_qi: f64,
    pub warning_crossed: bool,
    pub settlement: Option<PseudoVeinQiSettlement>,
    pub aftermath_hotspots: Vec<PseudoVeinStormHotspot>,
}

/// plan-zone-qi-economy-v1 P3 §8.1 决议 #3 — 灵潮借还款结算。
///
/// 借款模型：`inject_zone_for_pseudo_vein` 已从独立待分配池（`pending_inflow_account`）真实
/// 借出 `injected_qi`（绝对单位，与 `QI_ZONE_UNIT_CAPACITY` 同量纲）；dissipate 时本结算把
/// **能还多少还多少**（`min(injected_qi, zone 当前绝对余额)`）转回待分配池——不是旧版本固定
/// 30% 比例，借款期间被玩家/NPC 正常吸收的部分已经通过既有 `regen_from_zone` 路径守恒记账，
/// 不需要（也无法）重复归还。
#[derive(Debug, Clone, PartialEq)]
pub struct PseudoVeinQiSettlement {
    /// 原始借款额（绝对单位），即注入时实际借出的量。
    pub injected_qi: f64,
    /// 期望归还额，等于 `injected_qi`（round3 后）——实际能否足额归还取决于
    /// zone 当前余额，由 `apply_pseudo_vein_settlement` 在应用时按 `min` 缩量。
    pub returned_to_pool: f64,
    /// 期望的归还 transfer（`from=zone`, `to=pending_inflow_account`,
    /// `reason=PseudoVeinSettle`），`amount` 为 `returned_to_pool`——应用时若 zone 余额不足会
    /// 被 `apply_pseudo_vein_settlement` 缩量后另建一份实际 transfer，本字段只是"期望值"。
    pub return_transfer: QiTransfer,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct PseudoVeinSpawnIntent {
    pub zone_id: String,
    pub max_qi: f64,
    pub duration_ticks: u64,
    pub reason: PseudoVeinSpawnReason,
}

#[derive(Debug, Default, Resource)]
pub struct PseudoVeinFallbackState {
    last_eval_tick: Option<u64>,
    last_qi_by_zone: HashMap<String, f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PseudoVeinSpawnReason {
    HighPlayerDensity,
    HighQiDrain,
    TideTurnHighDrain,
}

impl PseudoVeinRuntime {
    pub fn new(
        zone_id: impl Into<String>,
        center_pos: BlockPos,
        started_at_tick: u64,
        season_at_spawn: PseudoVeinSeasonV1,
    ) -> Self {
        Self {
            zone_id: zone_id.into(),
            center_pos,
            current_qi: 0.0,
            max_qi: PSEUDO_VEIN_MAX_QI,
            base_duration_ticks: PSEUDO_VEIN_BASE_DURATION_TICKS,
            started_at_tick,
            phase: PseudoVeinPhase::Rising,
            cultivators_in_range: 0,
            season_at_spawn,
            injected_qi: 0.0,
            phase_started_at_tick: started_at_tick,
            last_tick: started_at_tick,
            last_visual_tick: None,
        }
    }

    pub fn advance(
        &mut self,
        current_tick: u64,
        cultivators_in_range: u32,
    ) -> PseudoVeinTickOutcome {
        let previous_phase = self.phase;
        self.cultivators_in_range = cultivators_in_range;
        self.advance_rising(current_tick);

        if matches!(
            self.phase,
            PseudoVeinPhase::Active | PseudoVeinPhase::Warning
        ) {
            self.advance_active_decay(current_tick);
        }

        let mut settlement = None;
        let mut aftermath_hotspots = Vec::new();
        if matches!(self.phase, PseudoVeinPhase::Dissipating)
            && current_tick.saturating_sub(self.phase_started_at_tick)
                >= PSEUDO_VEIN_DISSIPATING_TICKS
        {
            self.phase = PseudoVeinPhase::StormAftermath;
            self.phase_started_at_tick = current_tick;
            settlement = Some(settle_pseudo_vein_qi(
                self.zone_id.as_str(),
                self.injected_qi,
            ));
            let event = build_dissipate_event(
                self.zone_id.as_str(),
                [self.center_pos.x as f64, self.center_pos.z as f64],
                current_tick,
            );
            aftermath_hotspots = storm_hotspots_from_event(&event, current_tick);
        }

        PseudoVeinTickOutcome {
            phase: self.phase,
            current_qi: round3(self.current_qi),
            warning_crossed: previous_phase != PseudoVeinPhase::Warning
                && self.phase == PseudoVeinPhase::Warning,
            settlement,
            aftermath_hotspots,
        }
    }

    fn advance_rising(&mut self, current_tick: u64) {
        if self.phase != PseudoVeinPhase::Rising {
            return;
        }

        let elapsed = current_tick.saturating_sub(self.started_at_tick);
        if elapsed < PSEUDO_VEIN_RISING_TICKS {
            self.current_qi = self.max_qi * elapsed as f64 / PSEUDO_VEIN_RISING_TICKS as f64;
            self.last_tick = current_tick;
            return;
        }

        self.current_qi = self.max_qi;
        self.phase = PseudoVeinPhase::Active;
        self.phase_started_at_tick = self.started_at_tick + PSEUDO_VEIN_RISING_TICKS;
        self.last_tick = self.phase_started_at_tick;
    }

    fn advance_active_decay(&mut self, current_tick: u64) {
        let elapsed = current_tick.saturating_sub(self.last_tick);
        if elapsed == 0 {
            return;
        }

        let duration_ticks =
            effective_duration_ticks(self.base_duration_ticks, self.season_at_spawn);
        let decay = elapsed as f64
            * (self.max_qi / duration_ticks as f64)
            * pseudo_vein_decay_multiplier(self.cultivators_in_range);
        self.current_qi = (self.current_qi - decay).max(0.0);
        self.last_tick = current_tick;

        if self.current_qi <= 0.0 {
            self.phase = PseudoVeinPhase::Dissipating;
            self.phase_started_at_tick = current_tick;
        } else if self.current_qi <= PSEUDO_VEIN_WARNING_QI {
            self.phase = PseudoVeinPhase::Warning;
        }
    }
}

pub fn register(app: &mut App) {
    app.init_resource::<PseudoVeinFallbackState>().add_systems(
        Update,
        (
            pseudo_vein_fallback_spawn_system,
            pseudo_vein_runtime_tick_system,
        ),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn pseudo_vein_runtime_tick_system(
    clock: Option<Res<CultivationClock>>,
    mut commands: Commands,
    mut runtimes: Query<(Entity, &mut PseudoVeinRuntime)>,
    cultivators: Query<&Position, With<Cultivation>>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut ledger: Option<ResMut<WorldQiAccount>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut qi_transfers: EventWriter<QiTransfer>,
    mut pending_narrations: Option<ResMut<PendingGameplayNarrations>>,
) {
    let now = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for (entity, mut runtime) in &mut runtimes {
        let previous_phase = runtime.phase;
        let cultivator_count =
            count_cultivators_near(runtime.center_pos, cultivators.iter().map(|pos| pos.get()));
        let outcome = runtime.advance(now, cultivator_count);
        if let Some(settlement) = outcome.settlement.as_ref() {
            apply_pseudo_vein_settlement(
                zones.as_deref_mut(),
                ledger.as_deref_mut(),
                settlement,
                &mut qi_transfers,
            );
        }
        if matches!(runtime.phase, PseudoVeinPhase::StormAftermath)
            && now.saturating_sub(runtime.phase_started_at_tick) >= PSEUDO_VEIN_AFTERMATH_TICKS
        {
            commands.entity(entity).despawn();
            continue;
        }
        if should_emit_visual(&mut runtime, previous_phase, now, outcome.warning_crossed) {
            vfx_events.send(pseudo_vein_vfx_request(&runtime, outcome.phase));
        }
        if previous_phase != runtime.phase {
            if let Some(text) = pseudo_vein_phase_narration(runtime.phase) {
                if let Some(narrations) = pending_narrations.as_deref_mut() {
                    narrations.push_zone(
                        runtime.zone_id.as_str(),
                        text,
                        NarrationStyle::Perception,
                    );
                }
            }
        }
    }
}

/// plan-zone-qi-economy-v1 P3 §8.1 决议 #3 — 灵潮阶段切换的天道 zone-scope narration。
///
/// 仅在"窗口开启"（Active，固元环境门槛可用）与"窗口关闭"（Dissipating，灵潮开始消散）两个
/// 对玩家决策有意义的阶段边界发声；Rising/Warning/StormAftermath 不产生文案（避免刷屏，
/// 且这些阶段没有新的可玩信息——已有 VFX 承担视觉提示）。
fn pseudo_vein_phase_narration(phase: PseudoVeinPhase) -> Option<&'static str> {
    match phase {
        PseudoVeinPhase::Active => Some("灵潮涌动，此地灵气一时丰沛，正是冲击固元的良机。"),
        PseudoVeinPhase::Dissipating => Some("灵潮渐渐消散，天地灵气归于平淡。"),
        PseudoVeinPhase::Rising | PseudoVeinPhase::Warning | PseudoVeinPhase::StormAftermath => {
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn pseudo_vein_fallback_spawn_system(
    clock: Option<Res<CultivationClock>>,
    mut state: ResMut<PseudoVeinFallbackState>,
    mut commands: Commands,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut ledger: Option<ResMut<WorldQiAccount>>,
    cultivators: Query<&Position, With<Cultivation>>,
    runtimes: Query<&PseudoVeinRuntime>,
    mut qi_transfers: EventWriter<QiTransfer>,
) {
    let now = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    let Some(zones) = zones.as_deref_mut() else {
        return;
    };

    let Some(previous_tick) = state.last_eval_tick else {
        state.record_baseline(now, zones);
        return;
    };

    let elapsed_ticks = now.saturating_sub(previous_tick);
    if elapsed_ticks < PSEUDO_VEIN_FALLBACK_EVAL_PERIOD_TICKS {
        return;
    }

    let drain_by_zone = state.drain_rate_by_zone(zones, elapsed_ticks);
    let density_by_zone =
        player_density_by_zone(zones, cultivators.iter().map(|position| position.get()));
    let season = pseudo_vein_season_from_world(query_season("", now).season);
    let intent = fallback_auto_spawn_on_high_drain(zones, &drain_by_zone, &density_by_zone, season);

    if let Some(intent) = intent {
        spawn_fallback_pseudo_vein(
            &mut commands,
            zones,
            ledger.as_deref_mut(),
            &runtimes,
            &mut qi_transfers,
            intent,
            now,
            season,
        );
    }

    state.record_baseline(now, zones);
}

impl PseudoVeinFallbackState {
    fn record_baseline(&mut self, tick: u64, zones: &ZoneRegistry) {
        self.last_eval_tick = Some(tick);
        self.last_qi_by_zone = zones
            .zones
            .iter()
            .map(|zone| (zone.name.clone(), zone.spirit_qi))
            .collect();
    }

    fn drain_rate_by_zone(&self, zones: &ZoneRegistry, elapsed_ticks: u64) -> HashMap<String, f64> {
        if elapsed_ticks == 0 {
            return HashMap::new();
        }
        zones
            .zones
            .iter()
            .map(|zone| {
                let previous_qi = self
                    .last_qi_by_zone
                    .get(zone.name.as_str())
                    .copied()
                    .unwrap_or(zone.spirit_qi);
                let drained = (previous_qi - zone.spirit_qi).max(0.0);
                (zone.name.clone(), drained / elapsed_ticks as f64)
            })
            .collect()
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_fallback_pseudo_vein(
    commands: &mut Commands,
    zones: &mut ZoneRegistry,
    ledger: Option<&mut WorldQiAccount>,
    runtimes: &Query<&PseudoVeinRuntime>,
    qi_transfers: &mut EventWriter<QiTransfer>,
    intent: PseudoVeinSpawnIntent,
    tick: u64,
    season: PseudoVeinSeasonV1,
) {
    if runtimes
        .iter()
        .any(|runtime| runtime.zone_id == intent.zone_id)
    {
        return;
    }

    let Some(zone) = zones.find_zone_mut(intent.zone_id.as_str()) else {
        return;
    };
    let injected_qi = match ledger {
        Some(ledger) => {
            if let Some(transfer) = inject_zone_for_pseudo_vein(zone, ledger) {
                let amount = transfer.amount;
                qi_transfers.send(transfer);
                amount
            } else {
                0.0
            }
        }
        None => 0.0,
    };
    let center = zone.center();
    let mut runtime = PseudoVeinRuntime::new(
        zone.name.clone(),
        BlockPos::new(
            center.x.round() as i32,
            center.y.round() as i32,
            center.z.round() as i32,
        ),
        tick,
        season,
    );
    runtime.injected_qi = injected_qi;
    commands.spawn(runtime);
}

fn player_density_by_zone(
    zones: &ZoneRegistry,
    positions: impl IntoIterator<Item = DVec3>,
) -> HashMap<String, u32> {
    let mut density_by_zone = HashMap::new();
    for position in positions {
        let Some(zone) = zones.find_zone(DimensionKind::Overworld, position) else {
            continue;
        };
        let count = density_by_zone.entry(zone.name.clone()).or_insert(0u32);
        *count = count.saturating_add(1);
    }
    density_by_zone
}

/// plan-zone-qi-economy-v1 P3 §8.1 决议 #3 — 灵潮注入从独立待分配池**真实借出**，绝不凭空
/// 创生。借出额被待分配池当前余额钳制（`min(desired, available)`）——池子余额不足时灵潮只能
/// 把 zone 顶到"能负担"的高度，绝不透支（§8.1 #1 红线）。
///
/// 记账通过 `transfer_ledger_qi_to_zone` 直接更新外部 Zone owner；账本只保留稳定池与审计，
/// 不创建长期 `zone:*` 镜像。由 pending pool 余额钳制实际注入量。
pub fn inject_zone_for_pseudo_vein(
    zone: &mut Zone,
    ledger: &mut WorldQiAccount,
) -> Option<QiTransfer> {
    inject_zone_for_pseudo_vein_target(zone, ledger, PSEUDO_VEIN_MAX_QI)
}

/// heartbeat 动态伪灵脉复用的定额借出入口。
///
/// 与 [`inject_zone_for_pseudo_vein`] 使用同一 pending-pool → zone 账本路径，只把目标
/// 浓度改为 omen 已裁定的强度；这样动态 zone 不再用 `spirit_qi = intensity` 凭空创生。
pub(crate) fn inject_zone_for_pseudo_vein_target(
    zone: &mut Zone,
    ledger: &mut WorldQiAccount,
    target_spirit_qi: f64,
) -> Option<QiTransfer> {
    let before = zone.spirit_qi;
    let target = before
        .max(target_spirit_qi)
        .clamp(ZONE_SPIRIT_QI_MIN, ZONE_SPIRIT_QI_MAX);
    let desired_fraction = (target - before).max(0.0);
    if desired_fraction <= f64::EPSILON {
        return None;
    }
    let desired_absolute = round3(desired_fraction * QI_ZONE_UNIT_CAPACITY);
    if desired_absolute <= f64::EPSILON {
        return None;
    }

    let pool = pending_inflow_account();
    let available = ledger.balance(&pool).max(0.0);
    let actual_absolute = round3(desired_absolute.min(available));
    if actual_absolute <= f64::EPSILON {
        return None;
    }

    transfer_ledger_qi_to_zone(
        ledger,
        pool,
        zone.name.as_str(),
        &mut zone.spirit_qi,
        actual_absolute,
        ZONE_SPIRIT_QI_MAX,
        QiTransferReason::ReleaseToZone,
    )
    .ok()
    .flatten()
}

/// heartbeat 动态伪灵脉 zone 即将删除前的最终结算。
///
/// 普通 [`PseudoVeinRuntime`] 依附既有 zone，只需归还最初借款；heartbeat 版本会删除
/// 整个动态 zone，因此必须把该 zone 此刻的**全部剩余真元**转回 pending pool，包含借款
/// 余量以及期间可能由其他守恒路径释放进来的真元，避免随 zone 删除而吞掉余额。
pub(crate) fn settle_ephemeral_pseudo_vein_zone(
    zone: &mut Zone,
    ledger: &mut WorldQiAccount,
) -> Result<Option<QiTransfer>, QiPhysicsError> {
    settle_ephemeral_pseudo_vein_zone_to_target(zone, ledger, 0.0)
}

/// 将 heartbeat 动态伪灵脉 zone 的真实余额守恒收敛到生命周期目标值。
///
/// `PseudoVeinRuntimeState::advance` 只负责计算目标浓度；这里把减少量从 zone 账户真实
/// 转回 pending pool，再用账本结果回写 `zone.spirit_qi`。因此运行期衰减与最终删除共用
/// 同一条可审计路径，不会出现 state 已衰减、zone/ledger 仍停在旧值的三份状态分叉。
pub(crate) fn settle_ephemeral_pseudo_vein_zone_to_target(
    zone: &mut Zone,
    ledger: &mut WorldQiAccount,
    target_spirit_qi: f64,
) -> Result<Option<QiTransfer>, QiPhysicsError> {
    let current_absolute = zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY;
    let target_absolute = (target_spirit_qi.clamp(0.0, ZONE_SPIRIT_QI_MAX) * QI_ZONE_UNIT_CAPACITY)
        .min(current_absolute);
    let returned_absolute = (current_absolute - target_absolute).max(0.0);
    if returned_absolute == 0.0 {
        return Ok(None);
    }

    transfer_zone_qi_to_ledger(
        ledger,
        zone.name.as_str(),
        &mut zone.spirit_qi,
        pending_inflow_account(),
        returned_absolute,
        QiTransferReason::PseudoVeinSettle,
    )
}

/// plan-zone-qi-economy-v1 P3 §8.1 决议 #3 — 灵潮借款归还：能还多少还多少
/// （`min(settlement.returned_to_pool, zone 当前绝对余额)`），修复旧版本"仅收回 30%、70%
/// 永久留 zone 凭空创生"缺陷。`ledger` 缺失（如 headless 测试未插入该资源）时静默跳过，
/// zone 保留全部借款、不产生 transfer——这与"绝不透支"同一保守方向：宁可不结算，不可让
/// zone 凭空变化。
fn apply_pseudo_vein_settlement(
    zones: Option<&mut ZoneRegistry>,
    ledger: Option<&mut WorldQiAccount>,
    settlement: &PseudoVeinQiSettlement,
    qi_transfers: &mut EventWriter<QiTransfer>,
) {
    let (Some(zones), Some(ledger)) = (zones, ledger) else {
        return;
    };
    let Some(zone) = zones.find_zone_mut(settlement.return_transfer.from.id.as_str()) else {
        return;
    };
    let current_absolute = zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY;
    let actual_absolute = round3(settlement.returned_to_pool.min(current_absolute).max(0.0));
    if actual_absolute <= f64::EPSILON {
        return;
    }
    let Ok(Some(transfer)) = transfer_zone_qi_to_ledger(
        ledger,
        zone.name.as_str(),
        &mut zone.spirit_qi,
        pending_inflow_account(),
        actual_absolute,
        QiTransferReason::PseudoVeinSettle,
    ) else {
        return;
    };
    qi_transfers.send(transfer);
}

fn pseudo_vein_season_from_world(season: Season) -> PseudoVeinSeasonV1 {
    match season {
        Season::Summer => PseudoVeinSeasonV1::Summer,
        Season::SummerToWinter => PseudoVeinSeasonV1::SummerToWinter,
        Season::Winter => PseudoVeinSeasonV1::Winter,
        Season::WinterToSummer => PseudoVeinSeasonV1::WinterToSummer,
    }
}

fn count_cultivators_near(center: BlockPos, positions: impl IntoIterator<Item = DVec3>) -> u32 {
    let center = block_pos_center(center);
    let radius_sq = PSEUDO_VEIN_INFLUENCE_RADIUS_BLOCKS * PSEUDO_VEIN_INFLUENCE_RADIUS_BLOCKS;
    positions
        .into_iter()
        .filter(|position| position.distance_squared(center) <= radius_sq)
        .count()
        .try_into()
        .unwrap_or(u32::MAX)
}

fn should_emit_visual(
    runtime: &mut PseudoVeinRuntime,
    previous_phase: PseudoVeinPhase,
    current_tick: u64,
    warning_crossed: bool,
) -> bool {
    let phase_changed = previous_phase != runtime.phase;
    let due_periodic = runtime
        .last_visual_tick
        .map(|last_tick| current_tick.saturating_sub(last_tick) >= PSEUDO_VEIN_VISUAL_PERIOD_TICKS)
        .unwrap_or(true);
    if phase_changed || warning_crossed || due_periodic {
        runtime.last_visual_tick = Some(current_tick);
        return true;
    }
    false
}

fn pseudo_vein_vfx_request(runtime: &PseudoVeinRuntime, phase: PseudoVeinPhase) -> VfxEventRequest {
    let origin = block_pos_center(runtime.center_pos);
    let event_id = pseudo_vein_vfx_event_id(phase).to_string();
    VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id,
            origin: [origin.x, origin.y, origin.z],
            direction: Some([0.0, 1.0, 0.0]),
            color: Some(pseudo_vein_vfx_color(phase).to_string()),
            strength: Some(pseudo_vein_vfx_strength(runtime, phase)),
            count: Some(pseudo_vein_vfx_count(phase)),
            duration_ticks: Some(pseudo_vein_vfx_duration(phase)),
        },
    )
}

fn pseudo_vein_vfx_event_id(phase: PseudoVeinPhase) -> &'static str {
    match phase {
        PseudoVeinPhase::Rising => PSEUDO_VEIN_RISING_VFX_EVENT_ID,
        PseudoVeinPhase::Active => PSEUDO_VEIN_ACTIVE_VFX_EVENT_ID,
        PseudoVeinPhase::Warning => PSEUDO_VEIN_WARNING_VFX_EVENT_ID,
        PseudoVeinPhase::Dissipating => PSEUDO_VEIN_DISSIPATING_VFX_EVENT_ID,
        PseudoVeinPhase::StormAftermath => PSEUDO_VEIN_AFTERMATH_VFX_EVENT_ID,
    }
}

fn pseudo_vein_vfx_color(phase: PseudoVeinPhase) -> &'static str {
    match phase {
        PseudoVeinPhase::Rising | PseudoVeinPhase::Active => "#FFD36A",
        PseudoVeinPhase::Warning => "#CFA84A",
        PseudoVeinPhase::Dissipating => "#8C8C82",
        PseudoVeinPhase::StormAftermath => "#4D4A55",
    }
}

fn pseudo_vein_vfx_strength(runtime: &PseudoVeinRuntime, phase: PseudoVeinPhase) -> f32 {
    let qi_ratio = if runtime.max_qi <= f64::EPSILON {
        0.0
    } else {
        (runtime.current_qi / runtime.max_qi).clamp(0.0, 1.0)
    };
    let strength = match phase {
        PseudoVeinPhase::Rising | PseudoVeinPhase::Active => qi_ratio.max(0.35),
        PseudoVeinPhase::Warning => 0.75,
        PseudoVeinPhase::Dissipating => 0.45,
        PseudoVeinPhase::StormAftermath => 0.65,
    };
    strength as f32
}

fn pseudo_vein_vfx_count(phase: PseudoVeinPhase) -> u16 {
    match phase {
        PseudoVeinPhase::Rising => 24,
        PseudoVeinPhase::Active => 18,
        PseudoVeinPhase::Warning => 28,
        PseudoVeinPhase::Dissipating => 22,
        PseudoVeinPhase::StormAftermath => 30,
    }
}

fn pseudo_vein_vfx_duration(phase: PseudoVeinPhase) -> u16 {
    match phase {
        PseudoVeinPhase::Rising => 120,
        PseudoVeinPhase::Active => 100,
        PseudoVeinPhase::Warning => 80,
        PseudoVeinPhase::Dissipating => 100,
        PseudoVeinPhase::StormAftermath => 140,
    }
}

fn block_pos_center(pos: BlockPos) -> DVec3 {
    DVec3::new(pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5)
}

pub fn pseudo_vein_decay_multiplier(cultivators_in_range: u32) -> f64 {
    match cultivators_in_range {
        0..=1 => 1.0,
        2 => 1.4,
        3 => 1.8,
        4 => 2.5,
        _ => 3.5,
    }
}

pub fn effective_duration_ticks(base_duration_ticks: u64, season: PseudoVeinSeasonV1) -> u64 {
    let multiplier = match season {
        PseudoVeinSeasonV1::SummerToWinter | PseudoVeinSeasonV1::WinterToSummer => 2,
        PseudoVeinSeasonV1::Summer | PseudoVeinSeasonV1::Winter => 1,
    };
    base_duration_ticks.saturating_mul(multiplier)
}

pub fn settle_pseudo_vein_qi(zone_id: &str, injected_qi: f64) -> PseudoVeinQiSettlement {
    let injected_qi = round3(injected_qi.max(0.0));
    PseudoVeinQiSettlement {
        injected_qi,
        returned_to_pool: injected_qi,
        return_transfer: QiTransfer::new(
            QiAccountId::zone(zone_id),
            pending_inflow_account(),
            injected_qi,
            QiTransferReason::PseudoVeinSettle,
        )
        .expect("pseudo vein settlement return amount is finite and non-negative"),
    }
}

#[allow(dead_code)]
pub fn fallback_auto_spawn_on_high_drain(
    zones: &ZoneRegistry,
    qi_drain_rate_by_zone: &HashMap<String, f64>,
    player_density_by_zone: &HashMap<String, u32>,
    season: PseudoVeinSeasonV1,
) -> Option<PseudoVeinSpawnIntent> {
    zones
        .zones
        .iter()
        .filter_map(|zone| {
            let drain = qi_drain_rate_by_zone
                .get(zone.name.as_str())
                .copied()
                .unwrap_or_default();
            let density = player_density_by_zone
                .get(zone.name.as_str())
                .copied()
                .unwrap_or_default();
            let reason = if is_tide_turn(season) && drain > PSEUDO_VEIN_CRITICAL_DRAIN_RATE {
                PseudoVeinSpawnReason::TideTurnHighDrain
            } else if drain > PSEUDO_VEIN_CRITICAL_DRAIN_RATE {
                PseudoVeinSpawnReason::HighQiDrain
            } else if density >= PSEUDO_VEIN_CRITICAL_PLAYER_DENSITY {
                PseudoVeinSpawnReason::HighPlayerDensity
            } else {
                return None;
            };
            Some(PseudoVeinSpawnIntent {
                zone_id: zone.name.clone(),
                max_qi: PSEUDO_VEIN_MAX_QI,
                duration_ticks: effective_duration_ticks(PSEUDO_VEIN_BASE_DURATION_TICKS, season),
                reason,
            })
        })
        .max_by(|left, right| {
            let left_drain = qi_drain_rate_by_zone
                .get(left.zone_id.as_str())
                .copied()
                .unwrap_or_default();
            let right_drain = qi_drain_rate_by_zone
                .get(right.zone_id.as_str())
                .copied()
                .unwrap_or_default();
            left_drain.total_cmp(&right_drain)
        })
}

#[allow(dead_code)]
fn is_tide_turn(season: PseudoVeinSeasonV1) -> bool {
    matches!(
        season,
        PseudoVeinSeasonV1::SummerToWinter | PseudoVeinSeasonV1::WinterToSummer
    )
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::common::NarrationScope;
    use crate::world::dimension::DimensionKind;
    use crate::world::zone::Zone;
    use valence::prelude::{DVec3, Events};

    #[test]
    fn rising_reaches_max_qi_in_600_ticks() {
        let mut runtime = runtime(PseudoVeinSeasonV1::Summer);

        let midway = runtime.advance(300, 0);
        assert_eq!(midway.current_qi, PSEUDO_VEIN_MAX_QI / 2.0);
        assert_eq!(midway.phase, PseudoVeinPhase::Rising);

        let risen = runtime.advance(600, 0);
        assert_eq!(risen.current_qi, PSEUDO_VEIN_MAX_QI);
        assert_eq!(risen.phase, PseudoVeinPhase::Active);
    }

    #[test]
    fn crowded_dissipates_faster() {
        let mut quiet = runtime(PseudoVeinSeasonV1::Summer);
        let mut crowded = runtime(PseudoVeinSeasonV1::Summer);
        quiet.advance(600, 0);
        crowded.advance(600, 0);

        let quiet_after = quiet.advance(600 + 6_000, 1);
        let crowded_after = crowded.advance(600 + 6_000, 5);

        assert!(
            crowded_after.current_qi < quiet_after.current_qi,
            "5 人聚集应比 1 人更快消耗伪灵脉"
        );
        assert_eq!(pseudo_vein_decay_multiplier(5), 3.5);
    }

    #[test]
    fn qi_conservation() {
        // plan-zone-qi-economy-v1 P3 §8.1 决议 #3 — 借多少还多少（100% 归还），不是旧版本
        // 固定 30% 比例（那条路 70% 会永久留在 zone，凭空创生）。
        let settlement = settle_pseudo_vein_qi("lingquan_marsh", 37.5);

        assert_eq!(
            settlement.returned_to_pool, settlement.injected_qi,
            "settlement must plan to return exactly what was borrowed (100%), not a partial \
             ratio — got injected_qi={} returned_to_pool={}",
            settlement.injected_qi, settlement.returned_to_pool
        );
        assert_eq!(
            settlement.return_transfer.from,
            QiAccountId::zone("lingquan_marsh")
        );
        assert_eq!(settlement.return_transfer.to, pending_inflow_account());
        assert_eq!(settlement.return_transfer.amount, settlement.injected_qi);
        assert_eq!(
            settlement.return_transfer.reason,
            QiTransferReason::PseudoVeinSettle
        );
    }

    #[test]
    fn settle_pseudo_vein_qi_rejects_negative_injected_as_zero() {
        let settlement = settle_pseudo_vein_qi("lingquan_marsh", -5.0);
        assert_eq!(
            settlement.injected_qi, 0.0,
            "a negative injected_qi (should never happen, but defensive) must clamp to a \
             zero-amount settlement, not underflow or panic"
        );
        assert_eq!(settlement.returned_to_pool, 0.0);
    }

    #[test]
    fn aftermath_spawns_negative_hotspots() {
        let mut runtime = runtime(PseudoVeinSeasonV1::Summer);
        runtime.advance(600, 0);
        runtime.advance(600 + PSEUDO_VEIN_BASE_DURATION_TICKS, 1);

        let outcome = runtime.advance(
            600 + PSEUDO_VEIN_BASE_DURATION_TICKS + PSEUDO_VEIN_DISSIPATING_TICKS,
            1,
        );

        assert_eq!(outcome.phase, PseudoVeinPhase::StormAftermath);
        assert!(outcome.settlement.is_some());
        assert!((1..=3).contains(&outcome.aftermath_hotspots.len()));
    }

    #[test]
    fn tide_turn_doubles_duration() {
        assert_eq!(
            effective_duration_ticks(PSEUDO_VEIN_BASE_DURATION_TICKS, PseudoVeinSeasonV1::Summer),
            PSEUDO_VEIN_BASE_DURATION_TICKS
        );
        assert_eq!(
            effective_duration_ticks(
                PSEUDO_VEIN_BASE_DURATION_TICKS,
                PseudoVeinSeasonV1::WinterToSummer,
            ),
            PSEUDO_VEIN_BASE_DURATION_TICKS * 2
        );
    }

    #[test]
    fn fallback_auto_spawn_on_high_drain() {
        let registry = ZoneRegistry {
            zones: vec![zone("slow", 0.4, 0.0), zone("fast", 0.2, 64.0)],
        };
        let drain = HashMap::from([("slow".to_string(), 0.01), ("fast".to_string(), 0.03)]);
        let density = HashMap::new();

        let intent = super::fallback_auto_spawn_on_high_drain(
            &registry,
            &drain,
            &density,
            PseudoVeinSeasonV1::SummerToWinter,
        )
        .expect("高消耗汐转期应触发 fallback 伪灵脉");

        assert_eq!(intent.zone_id, "fast");
        assert_eq!(intent.reason, PseudoVeinSpawnReason::TideTurnHighDrain);
        assert_eq!(intent.duration_ticks, PSEUDO_VEIN_BASE_DURATION_TICKS * 2);
    }

    #[test]
    fn inject_zone_for_pseudo_vein_borrows_from_pending_pool_and_debits_it() {
        let mut zone = zone("fast", 0.1, 0.0);
        let mut ledger = WorldQiAccount::default();
        ledger
            .set_balance(pending_inflow_account(), 1000.0)
            .expect("seeding pool balance must succeed");

        let transfer = super::inject_zone_for_pseudo_vein(&mut zone, &mut ledger)
            .expect("a well-funded pending pool should fund the injection");

        let expected_absolute = round3((PSEUDO_VEIN_MAX_QI - 0.1) * QI_ZONE_UNIT_CAPACITY);
        assert_eq!(
            zone.spirit_qi, PSEUDO_VEIN_MAX_QI,
            "a well-funded pool must let the zone reach the full pseudo-vein target"
        );
        assert_eq!(transfer.from, pending_inflow_account());
        assert_eq!(transfer.to, QiAccountId::zone("fast"));
        assert_eq!(transfer.amount, expected_absolute);
        assert_eq!(transfer.reason, QiTransferReason::ReleaseToZone);
        assert_eq!(
            ledger.balance(&pending_inflow_account()),
            round3(1000.0 - expected_absolute),
            "pending pool must be debited by exactly the amount credited to the zone \
             (conservation: no qi created out of thin air)"
        );
    }

    #[test]
    fn inject_zone_for_pseudo_vein_scales_down_when_pool_is_underfunded() {
        let mut zone = zone("fast", 0.1, 0.0);
        let mut ledger = WorldQiAccount::default();
        // Only 5.0 available — far less than the ~37.5 a full injection to PSEUDO_VEIN_MAX_QI
        // would need.
        ledger
            .set_balance(pending_inflow_account(), 5.0)
            .expect("seeding pool balance must succeed");

        let transfer = super::inject_zone_for_pseudo_vein(&mut zone, &mut ledger)
            .expect("a partially-funded pool should still fund a partial injection");

        assert_eq!(transfer.amount, 5.0);
        assert!(
            zone.spirit_qi < PSEUDO_VEIN_MAX_QI,
            "an underfunded pool must not let the zone reach the full pseudo-vein target \
             (would be an overdraw), got {}",
            zone.spirit_qi
        );
        assert_eq!(
            ledger.balance(&pending_inflow_account()),
            0.0,
            "the pool must be drained to exactly zero (scaled down), never left negative or \
             partially untouched"
        );
    }

    #[test]
    fn inject_zone_for_pseudo_vein_is_a_noop_when_pool_is_empty() {
        let mut zone = zone("fast", 0.1, 0.0);
        let mut ledger = WorldQiAccount::default();

        let outcome = super::inject_zone_for_pseudo_vein(&mut zone, &mut ledger);

        assert!(
            outcome.is_none(),
            "an empty pending pool must yield no injection at all, not a zero-amount transfer"
        );
        assert_eq!(
            zone.spirit_qi, 0.1,
            "the zone must be left completely untouched when the pool cannot afford anything"
        );
    }

    #[test]
    fn inject_zone_for_pseudo_vein_is_a_noop_when_zone_already_above_target() {
        let mut zone = zone("fast", PSEUDO_VEIN_MAX_QI + 0.05, 0.0);
        let mut ledger = WorldQiAccount::default();
        ledger
            .set_balance(pending_inflow_account(), 1000.0)
            .expect("seeding pool balance must succeed");

        let outcome = super::inject_zone_for_pseudo_vein(&mut zone, &mut ledger);

        assert!(
            outcome.is_none(),
            "a zone already above the pseudo-vein target must not receive (or drain) any qi"
        );
        assert_eq!(
            ledger.balance(&pending_inflow_account()),
            1000.0,
            "the pool must be left untouched when the zone doesn't need topping up"
        );
    }

    #[test]
    fn runtime_tick_settlement_fully_repays_pending_pool_when_zone_still_holds_it() {
        let mut app = App::new();
        app.insert_resource(CultivationClock {
            tick: PSEUDO_VEIN_DISSIPATING_TICKS,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("lingquan_marsh", PSEUDO_VEIN_MAX_QI, 0.0)],
        });
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<VfxEventRequest>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, pseudo_vein_runtime_tick_system);

        let injected_absolute = round3(PSEUDO_VEIN_MAX_QI * QI_ZONE_UNIT_CAPACITY);
        let mut runtime = runtime(PseudoVeinSeasonV1::Summer);
        runtime.phase = PseudoVeinPhase::Dissipating;
        runtime.phase_started_at_tick = 0;
        runtime.current_qi = 0.0;
        runtime.injected_qi = injected_absolute;
        runtime.last_tick = 0;
        app.world_mut().spawn(runtime);

        app.update();

        let zone_qi = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("lingquan_marsh")
            .expect("test zone should exist")
            .spirit_qi;
        assert!(
            zone_qi.abs() < 1e-9,
            "when nobody consumed the pseudo-vein boost, settlement must return 100% of the \
             borrowed amount (not the old fixed 30%), draining the zone back to ~0, got {zone_qi}"
        );
        let transfers = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .collect::<Vec<_>>();
        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].from, QiAccountId::zone("lingquan_marsh"));
        assert_eq!(transfers[0].to, pending_inflow_account());
        assert_eq!(transfers[0].amount, injected_absolute);
        assert_eq!(transfers[0].reason, QiTransferReason::PseudoVeinSettle);
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&pending_inflow_account()),
            injected_absolute,
            "the pending pool must receive back exactly what was originally borrowed"
        );
    }

    #[test]
    fn runtime_tick_settlement_caps_repay_at_whatever_the_zone_still_holds() {
        // 借款期间部分被玩家/NPC 正常吸收（已通过既有 regen_from_zone 路径守恒记账）——
        // 结算时只能"能还多少还多少"，不能把 zone 打成负数去凑足全额归还。
        let mut app = App::new();
        app.insert_resource(CultivationClock {
            tick: PSEUDO_VEIN_DISSIPATING_TICKS,
        });
        // zone only holds 0.2 fraction (10.0 absolute) worth of qi at settlement time, far less
        // than the 42.5 absolute that was originally borrowed.
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("lingquan_marsh", 0.2, 0.0)],
        });
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<VfxEventRequest>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, pseudo_vein_runtime_tick_system);

        let injected_absolute = round3(PSEUDO_VEIN_MAX_QI * QI_ZONE_UNIT_CAPACITY);
        let mut runtime = runtime(PseudoVeinSeasonV1::Summer);
        runtime.phase = PseudoVeinPhase::Dissipating;
        runtime.phase_started_at_tick = 0;
        runtime.current_qi = 0.0;
        runtime.injected_qi = injected_absolute;
        runtime.last_tick = 0;
        app.world_mut().spawn(runtime);

        app.update();

        let zone_qi = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("lingquan_marsh")
            .expect("test zone should exist")
            .spirit_qi;
        assert!(
            zone_qi.abs() < 1e-9,
            "the zone must be drained to exactly zero (everything it still held), not left \
             negative, got {zone_qi}"
        );
        let transfers = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .collect::<Vec<_>>();
        assert_eq!(transfers.len(), 1);
        assert_eq!(
            transfers[0].amount, 10.0,
            "repay must be capped at what the zone actually held (10.0 absolute), not the \
             originally-borrowed 42.5 — the difference was already legitimately consumed \
             elsewhere and cannot be conjured back"
        );
    }

    #[test]
    fn aftermath_runtime_expires_after_ttl() {
        let mut app = App::new();
        app.insert_resource(CultivationClock {
            tick: PSEUDO_VEIN_AFTERMATH_TICKS,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("lingquan_marsh", 0.42, 0.0)],
        });
        app.add_event::<VfxEventRequest>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, pseudo_vein_runtime_tick_system);

        let mut runtime = runtime(PseudoVeinSeasonV1::Summer);
        runtime.phase = PseudoVeinPhase::StormAftermath;
        runtime.phase_started_at_tick = 0;
        app.world_mut().spawn(runtime);

        app.update();

        let mut query = app.world_mut().query::<&PseudoVeinRuntime>();
        assert_eq!(query.iter(app.world()).count(), 0);
    }

    // ── plan-zone-qi-economy-v1 P3 §8.1 决议 #3 — 灵潮 narration pin ──

    #[test]
    fn phase_narration_covers_all_variants() {
        assert!(
            pseudo_vein_phase_narration(PseudoVeinPhase::Active).is_some(),
            "entering Active (窗口开启，固元门槛可用) must produce a narration cue"
        );
        assert!(
            pseudo_vein_phase_narration(PseudoVeinPhase::Dissipating).is_some(),
            "entering Dissipating (窗口关闭) must produce a narration cue"
        );
        assert_ne!(
            pseudo_vein_phase_narration(PseudoVeinPhase::Active),
            pseudo_vein_phase_narration(PseudoVeinPhase::Dissipating),
            "the two narration cues must be textually distinct, not a copy-paste placeholder"
        );
        assert!(pseudo_vein_phase_narration(PseudoVeinPhase::Rising).is_none());
        assert!(pseudo_vein_phase_narration(PseudoVeinPhase::Warning).is_none());
        assert!(pseudo_vein_phase_narration(PseudoVeinPhase::StormAftermath).is_none());
    }

    #[test]
    fn runtime_tick_pushes_zone_scope_narration_on_entering_active() {
        let mut app = App::new();
        app.insert_resource(CultivationClock {
            tick: PSEUDO_VEIN_RISING_TICKS,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("lingquan_marsh", 0.1, 0.0)],
        });
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_event::<VfxEventRequest>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, pseudo_vein_runtime_tick_system);

        let runtime = runtime(PseudoVeinSeasonV1::Summer);
        app.world_mut().spawn(runtime);

        app.update();

        let mut narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(
            narrations.len(),
            1,
            "crossing Rising -> Active must push exactly one zone-scope narration"
        );
        let narration = narrations.remove(0);
        assert_eq!(narration.scope, NarrationScope::Zone);
        assert_eq!(narration.target.as_deref(), Some("lingquan_marsh"));
        assert_eq!(narration.style, NarrationStyle::Perception);
        assert_eq!(
            narration.text,
            pseudo_vein_phase_narration(PseudoVeinPhase::Active).unwrap()
        );
    }

    #[test]
    fn fallback_system_spawns_runtime_on_high_density() {
        let mut app = App::new();
        app.insert_resource(CultivationClock {
            tick: PSEUDO_VEIN_FALLBACK_EVAL_PERIOD_TICKS,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("fast", 0.1, 0.0)],
        });
        app.insert_resource(PseudoVeinFallbackState {
            last_eval_tick: Some(0),
            last_qi_by_zone: HashMap::from([("fast".to_string(), 0.1)]),
        });
        let mut ledger = WorldQiAccount::default();
        ledger
            .set_balance(pending_inflow_account(), 1000.0)
            .expect("seeding pool balance must succeed");
        app.insert_resource(ledger);
        app.add_event::<QiTransfer>();
        app.add_systems(Update, pseudo_vein_fallback_spawn_system);
        for _ in 0..PSEUDO_VEIN_CRITICAL_PLAYER_DENSITY {
            app.world_mut()
                .spawn((Cultivation::default(), Position::new([8.0, 66.0, 8.0])));
        }

        app.update();

        let mut query = app.world_mut().query::<&PseudoVeinRuntime>();
        let runtimes = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(runtimes.len(), 1);
        assert_eq!(runtimes[0].zone_id, "fast");
        let expected_absolute = round3((PSEUDO_VEIN_MAX_QI - 0.1) * QI_ZONE_UNIT_CAPACITY);
        assert_eq!(runtimes[0].injected_qi, expected_absolute);
        assert_eq!(
            app.world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name("fast")
                .expect("test zone should exist")
                .spirit_qi,
            PSEUDO_VEIN_MAX_QI
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&pending_inflow_account()),
            round3(1000.0 - expected_absolute),
            "the fallback-spawn path must debit the pending pool by exactly the amount \
             credited to the zone, same conservation invariant as the agent-command path"
        );
    }

    #[test]
    fn fallback_system_spawns_runtime_with_zero_injection_when_pool_is_missing() {
        // ledger resource 缺失（例如某些 headless 测试场景没插入 WorldQiAccount）时，
        // spawn_fallback_pseudo_vein 必须优雅降级为零注入，而不是 panic 或凭空创生。
        let mut app = App::new();
        app.insert_resource(CultivationClock {
            tick: PSEUDO_VEIN_FALLBACK_EVAL_PERIOD_TICKS,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("fast", 0.1, 0.0)],
        });
        app.insert_resource(PseudoVeinFallbackState {
            last_eval_tick: Some(0),
            last_qi_by_zone: HashMap::from([("fast".to_string(), 0.1)]),
        });
        app.add_event::<QiTransfer>();
        app.add_systems(Update, pseudo_vein_fallback_spawn_system);
        for _ in 0..PSEUDO_VEIN_CRITICAL_PLAYER_DENSITY {
            app.world_mut()
                .spawn((Cultivation::default(), Position::new([8.0, 66.0, 8.0])));
        }

        app.update();

        let mut query = app.world_mut().query::<&PseudoVeinRuntime>();
        let runtimes = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(runtimes.len(), 1, "the runtime should still spawn");
        assert_eq!(
            runtimes[0].injected_qi, 0.0,
            "with no ledger resource available, injection must be a no-op zero, not a crash"
        );
        assert_eq!(
            app.world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name("fast")
                .expect("test zone should exist")
                .spirit_qi,
            0.1,
            "the zone must be left completely untouched when the ledger is unavailable"
        );
    }

    #[test]
    fn visual_cue_matches_runtime_phase() {
        let mut runtime = runtime(PseudoVeinSeasonV1::Summer);
        runtime.advance(600, 0);

        let request = pseudo_vein_vfx_request(&runtime, PseudoVeinPhase::Active);

        match request.payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                color,
                strength,
                ..
            } => {
                assert_eq!(event_id, PSEUDO_VEIN_ACTIVE_VFX_EVENT_ID);
                assert_eq!(color.as_deref(), Some("#FFD36A"));
                assert_eq!(strength, Some(1.0));
            }
            other => panic!("expected pseudo vein SpawnParticle VFX, got {other:?}"),
        }
    }

    #[test]
    fn visual_throttle_emits_on_period_or_phase_change() {
        let mut runtime = runtime(PseudoVeinSeasonV1::Summer);

        assert!(should_emit_visual(
            &mut runtime,
            PseudoVeinPhase::Rising,
            10,
            false
        ));
        assert!(!should_emit_visual(
            &mut runtime,
            PseudoVeinPhase::Rising,
            50,
            false
        ));
        runtime.phase = PseudoVeinPhase::Active;
        assert!(should_emit_visual(
            &mut runtime,
            PseudoVeinPhase::Rising,
            51,
            false
        ));
    }

    fn runtime(season: PseudoVeinSeasonV1) -> PseudoVeinRuntime {
        PseudoVeinRuntime::new("lingquan_marsh", BlockPos::new(8, 66, 8), 0, season)
    }

    fn zone(name: &str, spirit_qi: f64, x: f64) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::new(x, 64.0, 0.0), DVec3::new(x + 16.0, 80.0, 16.0)),
            spirit_qi,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }
    }
}
