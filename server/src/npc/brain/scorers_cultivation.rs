use big_brain::prelude::{Actor, Score, ScorerBuilder};
use valence::client::ClientMarker;
use valence::prelude::{bevy_ecs, Commands, Component, DVec3, Entity, Query, Res, With};

use crate::cultivation::components::{Cultivation, MeridianId, MeridianSystem, Realm};
use crate::cultivation::tick::CultivationClock;
use crate::cultivation::topology::MeridianTopology;
use crate::cultivation::tribulation::TribulationState;
use crate::npc::lifecycle::PendingRetirement;
use crate::npc::lod::{lod_gated_score_by_kind, NpcLodConfig, NpcLodTick, NpcLodTier, ScorerKind};
use crate::npc::patrol::NpcPatrol;
use crate::npc::schedule::{schedule_multiplier, NpcDailySchedule, ScheduleActivity};
use crate::npc::spawn::NpcMarker;
use crate::world::era::WorldEraState;
use crate::world::zone::ZoneRegistry;

use super::{
    CultivationDriveHistory, CULTIVATE_MIN_ZONE_QI, CURIOSITY_BASELINE_SCORE,
    TRIBULATION_HOSTILE_RADIUS, TRIBULATION_READY_DRIVE_THRESHOLD, TRIBULATION_READY_SUSTAIN_TICKS,
};
use valence::prelude::Position;

// ---------------------------------------------------------------------------
// CultivationDriveScorer
// ---------------------------------------------------------------------------

/// Rogue cultivation scorer: `realm_progress x zone_spirit_qi_normalized`.
#[derive(Clone, Copy, Debug, Component)]
pub struct CultivationDriveScorer;

impl ScorerBuilder for CultivationDriveScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("CultivationDriveScorer")
    }
}

/// Zone `spirit_qi in [-1, 1]` -> `[0, 1]` (negative = dead zone, no cultivation contribution).
pub(crate) fn zone_qi_normalized(spirit_qi: f64) -> f32 {
    spirit_qi.clamp(0.0, 1.0) as f32
}

pub(crate) fn realm_progress_score(cultivation: &Cultivation, meridians: &MeridianSystem) -> f32 {
    let opened = meridians.opened_count() as f32;
    let needed = match cultivation.realm {
        Realm::Void => return 0.0,
        Realm::Spirit => Realm::Void.required_meridians(),
        other => match next_realm(other) {
            Some(next) => next.required_meridians(),
            None => return 0.0,
        },
    } as f32;
    (opened / needed.max(1.0)).clamp(0.0, 1.0)
}

pub(crate) fn cultivation_drive_score(
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
    zone_qi: f64,
) -> f32 {
    if zone_qi < CULTIVATE_MIN_ZONE_QI {
        return 0.0;
    }
    let qi = zone_qi_normalized(zone_qi);
    if qi <= 0.0 {
        return 0.0;
    }
    let progress = realm_progress_score(cultivation, meridians);
    (0.15 + 0.85 * progress) * qi
}

pub(crate) fn next_realm(current: Realm) -> Option<Realm> {
    match current {
        Realm::Awaken => Some(Realm::Induce),
        Realm::Induce => Some(Realm::Condense),
        Realm::Condense => Some(Realm::Solidify),
        Realm::Solidify => Some(Realm::Spirit),
        Realm::Spirit => None,
        Realm::Void => None,
    }
}

/// Pick next meridian to open: prefer adjacent to already opened, else any unopened.
pub(crate) fn pick_next_meridian_to_open(
    system: &MeridianSystem,
    topology: &MeridianTopology,
) -> Option<MeridianId> {
    let opened: Vec<MeridianId> = MeridianId::REGULAR
        .iter()
        .chain(MeridianId::EXTRAORDINARY.iter())
        .copied()
        .filter(|id| system.get(*id).opened)
        .collect();

    if opened.is_empty() {
        return MeridianId::REGULAR
            .iter()
            .chain(MeridianId::EXTRAORDINARY.iter())
            .copied()
            .find(|id| !system.get(*id).opened);
    }

    for opened_id in &opened {
        for cand in topology.neighbors(*opened_id) {
            if !system.get(*cand).opened {
                return Some(*cand);
            }
        }
    }
    MeridianId::REGULAR
        .iter()
        .chain(MeridianId::EXTRAORDINARY.iter())
        .copied()
        .find(|id| !system.get(*id).opened)
}

#[allow(clippy::type_complexity)]
pub(crate) fn cultivation_drive_scorer_system(
    mut npcs: Query<
        (
            &Cultivation,
            &MeridianSystem,
            &NpcPatrol,
            Option<&NpcDailySchedule>,
            Option<&PendingRetirement>,
            Option<&mut CultivationDriveHistory>,
            Option<&NpcLodTier>,
        ),
        With<NpcMarker>,
    >,
    zone_registry: Option<Res<ZoneRegistry>>,
    clock: Option<Res<CultivationClock>>,
    mut scorers: Query<(&Actor, &mut Score), With<CultivationDriveScorer>>,
    lod_config: Option<Res<NpcLodConfig>>,
    lod_tick: Option<Res<NpcLodTick>>,
) {
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);
    let clock_tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for (Actor(actor), mut score) in &mut scorers {
        let value = match npcs.get_mut(*actor) {
            Ok((cultivation, meridians, patrol, schedule, pending, history, tier)) => {
                // Standard（非 Cosmetic）：above_threshold_ticks 是渡劫计时的游戏状态推进，
                // 远离玩家时也应进行——与 tribulation_ready 的 hostile-distance 语义一致
                // （渡劫本就发生在玩家不在近旁时）；Cosmetic 只在 Near 计算与之矛盾致永不触发。
                match lod_gated_score_by_kind(tier, tick, &cfg, ScorerKind::Standard, || {
                    let raw = if pending.is_some() || matches!(cultivation.realm, Realm::Void) {
                        0.0
                    } else {
                        let zone_qi = zone_registry
                            .as_deref()
                            .and_then(|r| r.find_zone_by_name(&patrol.home_zone))
                            .map(|z| z.spirit_qi)
                            .unwrap_or(0.0);
                        cultivation_drive_score(cultivation, meridians, zone_qi)
                    };
                    if let Some(mut h) = history {
                        if raw >= TRIBULATION_READY_DRIVE_THRESHOLD {
                            h.above_threshold_ticks = h.above_threshold_ticks.saturating_add(1);
                        } else {
                            h.above_threshold_ticks = 0;
                        }
                    }
                    raw
                }) {
                    Some(value) => {
                        let multiplier = schedule_multiplier(
                            schedule,
                            tier,
                            clock_tick,
                            ScheduleActivity::Cultivate,
                        )
                        .unwrap_or(1.0);
                        value * multiplier
                    }
                    None => continue,
                }
            }
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

// ---------------------------------------------------------------------------
// CuriosityScorer
// ---------------------------------------------------------------------------

/// Rogue curiosity scorer (P2 placeholder: baseline only, pending POI system).
#[derive(Clone, Copy, Debug, Component)]
pub struct CuriosityScorer;

impl ScorerBuilder for CuriosityScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("CuriosityScorer")
    }
}

pub(crate) fn curiosity_scorer_system(
    npcs: Query<(Option<&PendingRetirement>, Option<&NpcLodTier>), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<CuriosityScorer>>,
    lod_config: Option<Res<NpcLodConfig>>,
    lod_tick: Option<Res<NpcLodTick>>,
) {
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut score) in &mut scorers {
        let value = match npcs.get(*actor) {
            Ok((pending, tier)) => {
                match lod_gated_score_by_kind(tier, tick, &cfg, ScorerKind::Cosmetic, || {
                    if pending.is_some() {
                        0.0
                    } else {
                        CURIOSITY_BASELINE_SCORE
                    }
                }) {
                    Some(value) => value,
                    None => continue,
                }
            }
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

// ---------------------------------------------------------------------------
// TribulationReadyScorer
// ---------------------------------------------------------------------------

/// Tribulation ready scorer (Realm=Spirit + 20 meridians + no hostiles + sustained drive).
#[derive(Clone, Copy, Debug, Component)]
pub struct TribulationReadyScorer;

impl ScorerBuilder for TribulationReadyScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("TribulationReadyScorer")
    }
}

/// Pure function: check 4 prerequisites for tribulation.
///
/// `tribulation_threshold_mul` — era modifier from [`WorldEraState::current_modifiers`].
/// - `> 1.0` = 渡劫阈值提高（灾劫时代，天道施压）
/// - `< 1.0` = 阈值降低（演绎时代，略宽）
/// - `1.0` = 无时代修正，行为与旧版完全相同。
pub(crate) fn tribulation_prereqs_met(
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
    history: &CultivationDriveHistory,
    tribulation_threshold_mul: f64,
) -> bool {
    if !matches!(cultivation.realm, Realm::Spirit) {
        return false;
    }
    if meridians.opened_count() < Realm::Void.required_meridians() {
        return false;
    }
    // era 修正：TRIBULATION_MIN_QI_RATIO × era_modifier（不写字面值，通过常数引用）
    let effective_ratio = super::TRIBULATION_MIN_QI_RATIO * tribulation_threshold_mul;
    if cultivation.qi_current < cultivation.qi_max * effective_ratio {
        return false;
    }
    if history.above_threshold_ticks < TRIBULATION_READY_SUSTAIN_TICKS {
        return false;
    }
    true
}

pub(crate) fn nearest_hostile_distance(
    npc_pos: DVec3,
    player_positions: impl Iterator<Item = DVec3>,
) -> Option<f64> {
    player_positions
        .map(|p| npc_pos.distance(p))
        .fold(None, |acc, d| match acc {
            None => Some(d),
            Some(prev) => Some(prev.min(d)),
        })
}

#[allow(clippy::type_complexity)]
pub(crate) fn tribulation_ready_scorer_system(
    npcs: Query<
        (
            &Position,
            &Cultivation,
            &MeridianSystem,
            &CultivationDriveHistory,
            Option<&PendingRetirement>,
            Option<&TribulationState>,
            Option<&NpcLodTier>,
        ),
        With<NpcMarker>,
    >,
    players: Query<&Position, With<ClientMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<TribulationReadyScorer>>,
    lod_config: Option<Res<NpcLodConfig>>,
    lod_tick: Option<Res<NpcLodTick>>,
    world_era: Option<Res<WorldEraState>>,
) {
    let player_positions: Vec<DVec3> = players.iter().map(|p| p.get()).collect();
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);
    // P1 era 注入：从 WorldEraState 读取渡劫阈值系数；Resource 不存在时退回基准 1.0。
    let tribulation_threshold_mul = world_era
        .as_deref()
        .map(|e| e.current_modifiers().tribulation_threshold_mul)
        .unwrap_or(1.0);

    for (Actor(actor), mut score) in &mut scorers {
        let value = match npcs.get(*actor) {
            Ok((position, cultivation, meridians, history, pending, in_tribulation, tier)) => {
                // Standard（非 Cosmetic）：渡劫就绪评估需覆盖远离玩家的 NPC——本 scorer 逻辑
                // 正是「无敌对玩家在 TRIBULATION_HOSTILE_RADIUS(100) 内才给 1.0」，而 Cosmetic
                // 只在 Near（玩家近旁）计算，两者直接矛盾使渡劫永不触发。与 cultivation_drive 同改。
                match lod_gated_score_by_kind(tier, tick, &cfg, ScorerKind::Standard, || {
                    if pending.is_some()
                        || in_tribulation.is_some()
                        || !tribulation_prereqs_met(
                            cultivation,
                            meridians,
                            history,
                            tribulation_threshold_mul,
                        )
                    {
                        0.0
                    } else {
                        let nearest = nearest_hostile_distance(
                            position.get(),
                            player_positions.iter().copied(),
                        );
                        match nearest {
                            Some(dist) if dist <= TRIBULATION_HOSTILE_RADIUS => 0.0,
                            _ => 1.0,
                        }
                    }
                }) {
                    Some(value) => value,
                    None => continue,
                }
            }
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

// ---------------------------------------------------------------------------
// SeclusionScorer
// ---------------------------------------------------------------------------

/// Post-Void seclusion scorer: Realm=Void always 1.0 (overrides everything except Retire).
#[derive(Clone, Copy, Debug, Component)]
pub struct SeclusionScorer;

impl ScorerBuilder for SeclusionScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("SeclusionScorer")
    }
}

pub(crate) fn seclusion_scorer_system(
    npcs: Query<(&Cultivation, Option<&PendingRetirement>), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<SeclusionScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = match npcs.get(*actor) {
            Ok((cultivation, pending)) => {
                if pending.is_some() || !matches!(cultivation.realm, Realm::Void) {
                    0.0
                } else {
                    1.0
                }
            }
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use big_brain::prelude::BigBrainSet;
    use valence::prelude::{App, IntoSystemConfigs, Position, PreUpdate};

    fn spirit_cultivation_at_qi(qi_ratio: f64) -> Cultivation {
        Cultivation {
            realm: Realm::Spirit,
            qi_max: 100.0,
            qi_current: 100.0 * qi_ratio,
            ..Cultivation::default()
        }
    }

    fn all_meridians_open() -> MeridianSystem {
        let mut m = MeridianSystem::default();
        for meridian in m.regular.iter_mut() {
            meridian.opened = true;
        }
        for meridian in m.extraordinary.iter_mut() {
            meridian.opened = true;
        }
        m
    }

    #[test]
    fn zone_qi_normalized_clamps_below_zero_to_zero() {
        assert_eq!(zone_qi_normalized(-1.0), 0.0);
        assert_eq!(zone_qi_normalized(0.0), 0.0);
        assert!((zone_qi_normalized(0.5) - 0.5).abs() < 1e-6);
        assert_eq!(zone_qi_normalized(1.5), 1.0);
    }

    #[test]
    fn realm_progress_score_is_zero_at_void() {
        let c = Cultivation {
            realm: Realm::Void,
            ..Cultivation::default()
        };
        let m = MeridianSystem::default();
        assert_eq!(realm_progress_score(&c, &m), 0.0);
    }

    #[test]
    fn cultivation_drive_score_zero_in_negative_zone() {
        let c = Cultivation::default();
        let mut m = MeridianSystem::default();
        m.regular[0].opened = true;
        assert_eq!(cultivation_drive_score(&c, &m, -0.4), 0.0);
    }

    #[test]
    fn cultivation_drive_score_zero_below_cultivate_min_zone_qi() {
        let c = Cultivation::default();
        let mut m = MeridianSystem::default();
        m.regular[0].opened = true;
        assert_eq!(cultivation_drive_score(&c, &m, 0.0), 0.0);
        assert_eq!(cultivation_drive_score(&c, &m, 0.1), 0.0);
        assert_eq!(cultivation_drive_score(&c, &m, 0.29), 0.0);
        assert!(
            cultivation_drive_score(&c, &m, 0.3) > 0.0,
            "at exactly CULTIVATE_MIN_ZONE_QI scorer must unblock cultivate path"
        );
    }

    #[test]
    fn cultivation_drive_score_grows_with_zone_qi_and_progress() {
        let c = Cultivation {
            realm: Realm::Condense,
            ..Cultivation::default()
        };
        let mut m_low = MeridianSystem::default();
        m_low.regular[0].opened = true;
        let mut m_mid = MeridianSystem::default();
        for m in m_mid.regular.iter_mut().take(4) {
            m.opened = true;
        }
        assert!(
            cultivation_drive_score(&c, &m_mid, 0.8) > cultivation_drive_score(&c, &m_low, 0.8),
            "more opened meridians -> higher drive at same zone_qi"
        );
        assert!(
            cultivation_drive_score(&c, &m_low, 0.9) > cultivation_drive_score(&c, &m_low, 0.3),
            "higher zone_qi -> higher drive at same progress"
        );
    }

    #[test]
    fn curiosity_scorer_emits_baseline_for_active_npc() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            curiosity_scorer_system.in_set(BigBrainSet::Scorers),
        );
        let npc = app.world_mut().spawn(NpcMarker).id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), CuriosityScorer))
            .id();
        app.update();
        assert!(
            (app.world().get::<Score>(scorer).unwrap().get() - CURIOSITY_BASELINE_SCORE).abs()
                < 1e-5
        );
    }

    #[test]
    fn curiosity_scorer_is_zero_when_pending_retirement() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            curiosity_scorer_system.in_set(BigBrainSet::Scorers),
        );
        let npc = app.world_mut().spawn((NpcMarker, PendingRetirement)).id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), CuriosityScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    #[test]
    fn curiosity_scorer_is_zero_when_dormant() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            curiosity_scorer_system.in_set(BigBrainSet::Scorers),
        );
        let npc = app.world_mut().spawn((NpcMarker, NpcLodTier::Dormant)).id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), CuriosityScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    #[test]
    fn tribulation_prereqs_reject_non_spirit_realm() {
        let c = Cultivation::default();
        let m = all_meridians_open();
        let h = CultivationDriveHistory {
            above_threshold_ticks: TRIBULATION_READY_SUSTAIN_TICKS,
        };
        assert!(!tribulation_prereqs_met(&c, &m, &h, 1.0));
    }

    #[test]
    fn tribulation_prereqs_reject_not_enough_meridians() {
        let c = spirit_cultivation_at_qi(0.9);
        let m = MeridianSystem::default();
        let h = CultivationDriveHistory {
            above_threshold_ticks: TRIBULATION_READY_SUSTAIN_TICKS,
        };
        assert!(!tribulation_prereqs_met(&c, &m, &h, 1.0));
    }

    #[test]
    fn tribulation_prereqs_reject_low_qi() {
        let c = spirit_cultivation_at_qi(0.3);
        let m = all_meridians_open();
        let h = CultivationDriveHistory {
            above_threshold_ticks: TRIBULATION_READY_SUSTAIN_TICKS,
        };
        assert!(!tribulation_prereqs_met(&c, &m, &h, 1.0));
    }

    #[test]
    fn tribulation_prereqs_reject_not_sustained() {
        let c = spirit_cultivation_at_qi(0.9);
        let m = all_meridians_open();
        let h = CultivationDriveHistory {
            above_threshold_ticks: 0,
        };
        assert!(!tribulation_prereqs_met(&c, &m, &h, 1.0));
    }

    #[test]
    fn tribulation_prereqs_pass_when_all_conditions_met() {
        let c = spirit_cultivation_at_qi(0.9);
        let m = all_meridians_open();
        let h = CultivationDriveHistory {
            above_threshold_ticks: TRIBULATION_READY_SUSTAIN_TICKS,
        };
        assert!(tribulation_prereqs_met(&c, &m, &h, 1.0));
    }

    // ── P1 Era 渡劫阈值注入测试 ──────────────────────────────────────────────

    #[test]
    fn tribulation_prereqs_calamity_era_higher_threshold_blocks_borderline_qi() {
        use crate::world::era::{current_modifiers, EraType, CALAMITY_TRIBULATION_MUL};
        // qi = 0.85 在 Unknown 时代（ratio=0.8）下通过，但灾劫时代（0.8*1.1=0.88）下应被拒绝
        let c = spirit_cultivation_at_qi(0.85);
        let m = all_meridians_open();
        let h = CultivationDriveHistory {
            above_threshold_ticks: TRIBULATION_READY_SUSTAIN_TICKS,
        };
        let calamity_mul = current_modifiers(EraType::Calamity).tribulation_threshold_mul;
        assert_eq!(
            calamity_mul, CALAMITY_TRIBULATION_MUL,
            "灾劫时代系数应等于 CALAMITY_TRIBULATION_MUL 常数（不写字面值）"
        );
        assert!(
            !tribulation_prereqs_met(&c, &m, &h, calamity_mul),
            "灾劫时代 qi=0.85 < 0.8*1.1=0.88 应被拒绝（天道施压）"
        );
        // 但同一 qi 在 Unknown 时代应通过
        assert!(
            tribulation_prereqs_met(&c, &m, &h, 1.0),
            "Unknown 时代 qi=0.85 >= 0.8 应通过（基准不变）"
        );
    }

    #[test]
    fn tribulation_prereqs_deduction_era_lower_threshold_allows_borderline_qi() {
        use crate::world::era::{current_modifiers, EraType, DEDUCTION_TRIBULATION_MUL};
        // qi = 0.77 在 Unknown 时代（ratio=0.8）下被拒，但演绎时代（0.8*0.95=0.76）下应通过
        let c = spirit_cultivation_at_qi(0.77);
        let m = all_meridians_open();
        let h = CultivationDriveHistory {
            above_threshold_ticks: TRIBULATION_READY_SUSTAIN_TICKS,
        };
        let deduction_mul = current_modifiers(EraType::Deduction).tribulation_threshold_mul;
        assert_eq!(
            deduction_mul, DEDUCTION_TRIBULATION_MUL,
            "演绎时代系数应等于 DEDUCTION_TRIBULATION_MUL 常数（不写字面值）"
        );
        assert!(
            tribulation_prereqs_met(&c, &m, &h, deduction_mul),
            "演绎时代 qi=0.77 >= 0.8*0.95=0.76 应通过（天道略宽）"
        );
        // 同一 qi 在 Unknown 时代应被拒绝
        assert!(
            !tribulation_prereqs_met(&c, &m, &h, 1.0),
            "Unknown 时代 qi=0.77 < 0.8 应被拒绝"
        );
    }

    #[test]
    fn tribulation_prereqs_tribulation_min_qi_ratio_constant_not_hardcoded() {
        // 确保阈值检查通过常数 TRIBULATION_MIN_QI_RATIO，而非写死字面值
        // 测试方式：qi = TRIBULATION_MIN_QI_RATIO * 1.05 (略高于阈值) 应通过；
        //          qi = TRIBULATION_MIN_QI_RATIO * 0.95 (略低于阈值) 应被拒绝
        use crate::npc::brain::TRIBULATION_MIN_QI_RATIO;
        let above = spirit_cultivation_at_qi(TRIBULATION_MIN_QI_RATIO * 1.05);
        let below = spirit_cultivation_at_qi(TRIBULATION_MIN_QI_RATIO * 0.95);
        let m = all_meridians_open();
        let h = CultivationDriveHistory {
            above_threshold_ticks: TRIBULATION_READY_SUSTAIN_TICKS,
        };
        assert!(
            tribulation_prereqs_met(&above, &m, &h, 1.0),
            "qi 略高于 TRIBULATION_MIN_QI_RATIO 应通过（常数参照正确）"
        );
        assert!(
            !tribulation_prereqs_met(&below, &m, &h, 1.0),
            "qi 略低于 TRIBULATION_MIN_QI_RATIO 应被拒绝（常数参照正确）"
        );
    }

    #[test]
    fn nearest_hostile_distance_empty_returns_none() {
        let pos = DVec3::new(0.0, 66.0, 0.0);
        assert_eq!(nearest_hostile_distance(pos, std::iter::empty()), None);
    }

    #[test]
    fn nearest_hostile_distance_picks_min() {
        let pos = DVec3::new(0.0, 66.0, 0.0);
        let players = vec![
            DVec3::new(5.0, 66.0, 0.0),
            DVec3::new(150.0, 66.0, 0.0),
            DVec3::new(2.0, 66.0, 0.0),
        ];
        let nearest = nearest_hostile_distance(pos, players.into_iter()).unwrap();
        assert!((nearest - 2.0).abs() < 1e-9);
    }

    #[test]
    fn tribulation_ready_scorer_zero_when_hostile_within_radius() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            tribulation_ready_scorer_system.in_set(BigBrainSet::Scorers),
        );
        app.world_mut()
            .spawn((ClientMarker, Position::new([50.0, 66.0, 0.0])));
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 66.0, 0.0]),
                spirit_cultivation_at_qi(0.9),
                all_meridians_open(),
                CultivationDriveHistory {
                    above_threshold_ticks: TRIBULATION_READY_SUSTAIN_TICKS,
                },
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), TribulationReadyScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    #[test]
    fn tribulation_ready_scorer_one_when_all_conditions_met_and_no_hostile() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            tribulation_ready_scorer_system.in_set(BigBrainSet::Scorers),
        );
        app.world_mut()
            .spawn((ClientMarker, Position::new([500.0, 66.0, 0.0])));
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 66.0, 0.0]),
                spirit_cultivation_at_qi(0.9),
                all_meridians_open(),
                CultivationDriveHistory {
                    above_threshold_ticks: TRIBULATION_READY_SUSTAIN_TICKS,
                },
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), TribulationReadyScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 1.0);
    }

    #[test]
    fn seclusion_scorer_reads_void_realm() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            seclusion_scorer_system.in_set(BigBrainSet::Scorers),
        );
        let void_npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Cultivation {
                    realm: Realm::Void,
                    ..Cultivation::default()
                },
            ))
            .id();
        let spirit_npc = app
            .world_mut()
            .spawn((NpcMarker, spirit_cultivation_at_qi(0.5)))
            .id();
        let void_scorer = app
            .world_mut()
            .spawn((Actor(void_npc), Score::default(), SeclusionScorer))
            .id();
        let spirit_scorer = app
            .world_mut()
            .spawn((Actor(spirit_npc), Score::default(), SeclusionScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(void_scorer).unwrap().get(), 1.0);
        assert_eq!(app.world().get::<Score>(spirit_scorer).unwrap().get(), 0.0);
    }

    #[test]
    fn cultivation_drive_scorer_reads_home_zone_qi() {
        use crate::world::zone::ZoneRegistry;

        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].name = crate::world::zone::DEFAULT_SPAWN_ZONE_NAME.to_string();
        zones.zones[0].spirit_qi = 0.7;
        app.insert_resource(zones);
        app.add_systems(
            PreUpdate,
            cultivation_drive_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Cultivation::default(),
                MeridianSystem::default(),
                NpcPatrol::new(
                    crate::world::zone::DEFAULT_SPAWN_ZONE_NAME,
                    DVec3::new(0.0, 66.0, 0.0),
                ),
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), CultivationDriveScorer))
            .id();
        app.update();
        let val = app.world().get::<Score>(scorer).unwrap().get();
        assert!(val > 0.0 && val < 1.0, "expected partial score, got {val}");
    }

    #[test]
    fn tribulation_ready_scorer_evaluates_for_far_tier_npc_when_no_hostile() {
        // 回归锁：渡劫就绪本就发生在玩家不在近旁时（>TRIBULATION_HOSTILE_RADIUS=100）。修前
        // Cosmetic 只在 Near 计算 → Far NPC 永不评估 → 自主渡劫死。改 Standard 后 Far 在 tick0
        // 计算（0 % interval == 0 不跳过），prereqs 满足且无敌对玩家在 100 内 → 1.0。
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            tribulation_ready_scorer_system.in_set(BigBrainSet::Scorers),
        );
        app.world_mut()
            .spawn((ClientMarker, Position::new([500.0, 66.0, 0.0])));
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 66.0, 0.0]),
                spirit_cultivation_at_qi(0.9),
                all_meridians_open(),
                CultivationDriveHistory {
                    above_threshold_ticks: TRIBULATION_READY_SUSTAIN_TICKS,
                },
                NpcLodTier::Far,
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), TribulationReadyScorer))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            1.0,
            "Far 档 NPC 无敌对玩家在 100 内、prereqs 满足时应就绪 1.0（修前 Cosmetic 跳过 Far 恒为 0）"
        );
    }

    #[test]
    fn cultivation_drive_scorer_advances_counter_for_far_tier_npc() {
        use crate::world::zone::ZoneRegistry;

        // 回归锁（本 PR 核心）：above_threshold_ticks 是渡劫计时状态，远离玩家也须推进。修前
        // Cosmetic 使 Far NPC 的 compute 闭包永不执行 → 计数器恒 0 → 渡劫永不就绪。改 Standard 后
        // Far 在 tick0 执行闭包，Spirit+全脉通+高灵气 drive≈1.0 ≥ 0.6 阈值 → 计数器 +1。
        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].name = crate::world::zone::DEFAULT_SPAWN_ZONE_NAME.to_string();
        zones.zones[0].spirit_qi = 0.9;
        app.insert_resource(zones);
        app.add_systems(
            PreUpdate,
            cultivation_drive_scorer_system.in_set(BigBrainSet::Scorers),
        );
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                spirit_cultivation_at_qi(0.9),
                all_meridians_open(),
                NpcPatrol::new(
                    crate::world::zone::DEFAULT_SPAWN_ZONE_NAME,
                    DVec3::new(0.0, 66.0, 0.0),
                ),
                NpcLodTier::Far,
                CultivationDriveHistory {
                    above_threshold_ticks: 0,
                },
            ))
            .id();
        let _scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), CultivationDriveScorer))
            .id();
        app.update();
        let history = app.world().get::<CultivationDriveHistory>(npc).unwrap();
        assert_eq!(
            history.above_threshold_ticks, 1,
            "Far 档 NPC 的渡劫计时计数器应递增（修前 Cosmetic 跳过 Far → 恒为 0 → 自主渡劫死）"
        );
    }

    #[test]
    fn drive_before_ready_same_frame_write_then_read_reaches_ready() {
        use crate::world::zone::ZoneRegistry;

        // 锁 .before() 调度：同一 PreUpdate 帧先跑 cultivation_drive（写 above_threshold_ticks）
        // 再跑 tribulation_ready（读它）。history 起于 SUSTAIN-1，本帧 drive 达阈值 → 计数器 +1
        // 到 SUSTAIN → 同帧 ready 立即 1.0。若无 .before（ready 先跑）则读 stale SUSTAIN-1 → 0.0。
        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].name = crate::world::zone::DEFAULT_SPAWN_ZONE_NAME.to_string();
        zones.zones[0].spirit_qi = 0.9;
        app.insert_resource(zones);
        app.add_systems(
            PreUpdate,
            (
                cultivation_drive_scorer_system.before(tribulation_ready_scorer_system),
                tribulation_ready_scorer_system,
            )
                .in_set(BigBrainSet::Scorers),
        );
        // 玩家远在 500（>hostile 半径 100），不抑制渡劫。
        app.world_mut()
            .spawn((ClientMarker, Position::new([500.0, 66.0, 0.0])));
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 66.0, 0.0]),
                spirit_cultivation_at_qi(0.9),
                all_meridians_open(),
                NpcPatrol::new(
                    crate::world::zone::DEFAULT_SPAWN_ZONE_NAME,
                    DVec3::new(0.0, 66.0, 0.0),
                ),
                NpcLodTier::Far,
                CultivationDriveHistory {
                    above_threshold_ticks: TRIBULATION_READY_SUSTAIN_TICKS - 1,
                },
            ))
            .id();
        // 两个 scorer actor 都需在场：cultivation_drive 经 CultivationDriveScorer 写计数器，
        // tribulation_ready 经 TribulationReadyScorer 读。
        app.world_mut()
            .spawn((Actor(npc), Score::default(), CultivationDriveScorer));
        let ready = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), TribulationReadyScorer))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Score>(ready).unwrap().get(),
            1.0,
            "同帧 cultivation_drive(写) 先于 tribulation_ready(读)：计数器跨过 SUSTAIN 后 ready 当帧即 \
             1.0；若乱序读到 stale SUSTAIN-1 则为 0.0"
        );
    }

    #[test]
    fn tribulation_ready_scorer_suppressed_at_exact_hostile_radius_far_tier() {
        // 边界锁：dist == TRIBULATION_HOSTILE_RADIUS(100) 应判 0.0（抑制），证 `<=` 非 `<`，锁
        // off-by-one。Far 档（Standard，tick0 计算），玩家恰在 100 距处。
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            tribulation_ready_scorer_system.in_set(BigBrainSet::Scorers),
        );
        app.world_mut()
            .spawn((ClientMarker, Position::new([100.0, 66.0, 0.0])));
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 66.0, 0.0]),
                spirit_cultivation_at_qi(0.9),
                all_meridians_open(),
                CultivationDriveHistory {
                    above_threshold_ticks: TRIBULATION_READY_SUSTAIN_TICKS,
                },
                NpcLodTier::Far,
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), TribulationReadyScorer))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "敌对玩家恰在 TRIBULATION_HOSTILE_RADIUS(100) 处应抑制渡劫（dist <= 100 → 0.0）"
        );
    }
}
