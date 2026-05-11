use std::collections::HashMap;

use valence::prelude::{
    bevy_ecs, App, BlockPos, Commands, Component, DVec3, EventWriter, Position, Query, Res, ResMut,
    Resource, Update, With,
};

use crate::cultivation::components::Cultivation;
use crate::cultivation::tick::CultivationClock;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason};
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
pub const PSEUDO_VEIN_MAX_QI: f64 = 0.6;
pub const PSEUDO_VEIN_WARNING_QI: f64 = 0.3;
#[allow(dead_code)]
pub const PSEUDO_VEIN_CRITICAL_DRAIN_RATE: f64 = 0.02;
#[allow(dead_code)]
pub const PSEUDO_VEIN_CRITICAL_PLAYER_DENSITY: u32 = 4;
pub const PSEUDO_VEIN_INFLUENCE_RADIUS_BLOCKS: f64 = 30.0;
const PSEUDO_VEIN_VISUAL_PERIOD_TICKS: u64 = 100;
const PSEUDO_VEIN_FALLBACK_EVAL_PERIOD_TICKS: u64 = 12_000;
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

#[derive(Debug, Clone, PartialEq)]
pub struct PseudoVeinQiSettlement {
    pub initial_injected: f64,
    pub released_to_zones: f64,
    pub collected_by_tiandao: f64,
    pub injection_transfer: QiTransfer,
    pub collection_transfer: QiTransfer,
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
            settlement = Some(settle_pseudo_vein_qi(self.zone_id.as_str(), self.max_qi));
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

pub fn pseudo_vein_runtime_tick_system(
    clock: Option<Res<CultivationClock>>,
    mut runtimes: Query<&mut PseudoVeinRuntime>,
    cultivators: Query<&Position, With<Cultivation>>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut qi_transfers: EventWriter<QiTransfer>,
) {
    let now = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for mut runtime in &mut runtimes {
        let previous_phase = runtime.phase;
        let cultivator_count =
            count_cultivators_near(runtime.center_pos, cultivators.iter().map(|pos| pos.get()));
        let outcome = runtime.advance(now, cultivator_count);
        if let Some(settlement) = outcome.settlement.as_ref() {
            apply_pseudo_vein_settlement(zones.as_deref_mut(), settlement, &mut qi_transfers);
        }
        if should_emit_visual(&mut runtime, previous_phase, now, outcome.warning_crossed) {
            vfx_events.send(pseudo_vein_vfx_request(&runtime, outcome.phase));
        }
    }
}

pub fn pseudo_vein_fallback_spawn_system(
    clock: Option<Res<CultivationClock>>,
    mut state: ResMut<PseudoVeinFallbackState>,
    mut commands: Commands,
    mut zones: Option<ResMut<ZoneRegistry>>,
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

fn spawn_fallback_pseudo_vein(
    commands: &mut Commands,
    zones: &mut ZoneRegistry,
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
    if let Some(transfer) = inject_zone_for_pseudo_vein(zone) {
        qi_transfers.send(transfer);
    }
    let center = zone.center();
    commands.spawn(PseudoVeinRuntime::new(
        zone.name.clone(),
        BlockPos::new(
            center.x.round() as i32,
            center.y.round() as i32,
            center.z.round() as i32,
        ),
        tick,
        season,
    ));
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

pub fn inject_zone_for_pseudo_vein(zone: &mut Zone) -> Option<QiTransfer> {
    let before = zone.spirit_qi;
    zone.spirit_qi = zone
        .spirit_qi
        .max(PSEUDO_VEIN_MAX_QI)
        .clamp(ZONE_SPIRIT_QI_MIN, ZONE_SPIRIT_QI_MAX);
    let injected = round3((zone.spirit_qi - before).max(0.0));
    if injected <= f64::EPSILON {
        return None;
    }
    QiTransfer::new(
        QiAccountId::tiandao(),
        QiAccountId::zone(zone.name.as_str()),
        injected,
        QiTransferReason::ReleaseToZone,
    )
    .ok()
}

fn apply_pseudo_vein_settlement(
    zones: Option<&mut ZoneRegistry>,
    settlement: &PseudoVeinQiSettlement,
    qi_transfers: &mut EventWriter<QiTransfer>,
) {
    if let Some(zones) = zones {
        if let Some(zone) = zones.find_zone_mut(settlement.collection_transfer.from.id.as_str()) {
            zone.spirit_qi = (zone.spirit_qi - settlement.collected_by_tiandao)
                .clamp(ZONE_SPIRIT_QI_MIN, ZONE_SPIRIT_QI_MAX);
        }
    }
    qi_transfers.send(settlement.collection_transfer.clone());
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
    let initial_injected = round3(injected_qi.max(0.0));
    let collected_by_tiandao = round3(initial_injected * 0.3);
    let released_to_zones = round3(initial_injected - collected_by_tiandao);
    PseudoVeinQiSettlement {
        initial_injected,
        released_to_zones,
        collected_by_tiandao,
        injection_transfer: QiTransfer::new(
            QiAccountId::tiandao(),
            QiAccountId::zone(zone_id),
            initial_injected,
            QiTransferReason::ReleaseToZone,
        )
        .expect("pseudo vein injected qi is finite and non-negative"),
        collection_transfer: QiTransfer::new(
            QiAccountId::zone(zone_id),
            QiAccountId::tiandao(),
            collected_by_tiandao,
            QiTransferReason::EraDecay,
        )
        .expect("pseudo vein tiandao collection is finite and non-negative"),
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
    use crate::world::dimension::DimensionKind;
    use crate::world::zone::Zone;
    use valence::prelude::{DVec3, Events};

    #[test]
    fn rising_reaches_0_6_in_600_ticks() {
        let mut runtime = runtime(PseudoVeinSeasonV1::Summer);

        let midway = runtime.advance(300, 0);
        assert_eq!(midway.current_qi, 0.3);
        assert_eq!(midway.phase, PseudoVeinPhase::Rising);

        let risen = runtime.advance(600, 0);
        assert_eq!(risen.current_qi, 0.6);
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
        let settlement = settle_pseudo_vein_qi("lingquan_marsh", PSEUDO_VEIN_MAX_QI);

        assert_eq!(
            settlement.initial_injected,
            round3(settlement.released_to_zones + settlement.collected_by_tiandao)
        );
        assert_eq!(settlement.injection_transfer.from, QiAccountId::tiandao());
        assert_eq!(
            settlement.injection_transfer.to,
            QiAccountId::zone("lingquan_marsh")
        );
        assert_eq!(
            settlement.collection_transfer.from,
            QiAccountId::zone("lingquan_marsh")
        );
        assert_eq!(settlement.collection_transfer.to, QiAccountId::tiandao());
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
    fn inject_zone_for_pseudo_vein_records_actual_zone_delta() {
        let mut zone = zone("fast", 0.1, 0.0);

        let transfer = super::inject_zone_for_pseudo_vein(&mut zone)
            .expect("low-qi zone should receive tiandao injection");

        assert_eq!(zone.spirit_qi, PSEUDO_VEIN_MAX_QI);
        assert_eq!(transfer.from, QiAccountId::tiandao());
        assert_eq!(transfer.to, QiAccountId::zone("fast"));
        assert_eq!(transfer.amount, 0.5);
        assert_eq!(transfer.reason, QiTransferReason::ReleaseToZone);
    }

    #[test]
    fn runtime_tick_emits_collection_transfer_on_settlement() {
        let mut app = App::new();
        app.insert_resource(CultivationClock {
            tick: PSEUDO_VEIN_DISSIPATING_TICKS,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("lingquan_marsh", PSEUDO_VEIN_MAX_QI, 0.0)],
        });
        app.add_event::<VfxEventRequest>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, pseudo_vein_runtime_tick_system);

        let mut runtime = runtime(PseudoVeinSeasonV1::Summer);
        runtime.phase = PseudoVeinPhase::Dissipating;
        runtime.phase_started_at_tick = 0;
        runtime.current_qi = 0.0;
        runtime.last_tick = 0;
        app.world_mut().spawn(runtime);

        app.update();

        let zone_qi = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("lingquan_marsh")
            .expect("test zone should exist")
            .spirit_qi;
        assert_eq!(zone_qi, 0.42);
        let transfers = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .collect::<Vec<_>>();
        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].from, QiAccountId::zone("lingquan_marsh"));
        assert_eq!(transfers[0].to, QiAccountId::tiandao());
        assert_eq!(transfers[0].amount, 0.18);
        assert_eq!(transfers[0].reason, QiTransferReason::EraDecay);
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
        assert_eq!(
            app.world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name("fast")
                .expect("test zone should exist")
                .spirit_qi,
            PSEUDO_VEIN_MAX_QI
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
        }
    }
}
