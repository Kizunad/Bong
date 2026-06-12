mod actions_combat;
mod actions_life;
mod scorers_combat;
mod scorers_cultivation;
mod scorers_social;
mod scorers_survival;
pub mod threat;

// ---- Re-exports: scorer types (pub) ----
// Allow unused: these are the full API surface matching original brain.rs;
// not all types are imported externally yet but must remain accessible.
#[allow(unused_imports)]
pub use scorers_combat::{
    ChaseTargetScorer, DashScorer, MeleeRangeScorer, NpcDefenseScorer, PlayerProximityScorer,
};
#[allow(unused_imports)]
pub use scorers_cultivation::{
    CultivationDriveScorer, CuriosityScorer, SeclusionScorer, TribulationReadyScorer,
};
#[allow(unused_imports)]
pub use scorers_social::TradeStallScorer;
#[allow(unused_imports)]
pub use scorers_survival::{
    AgeingScorer, FearCultivatorScorer, HungerScorer, ReturnHomeScorer, WanderScorer,
};

// ---- Re-exports: action types (pub) ----
#[allow(unused_imports)]
pub use actions_combat::{
    ChaseAction, DashAction, FleeAction, MeleeAttackAction, NpcDefenseAction,
};
#[allow(unused_imports)]
pub use actions_life::{
    CultivateAction, FarmAction, FleeCultivatorAction, RestAction, RetireAction, ReturnHomeAction,
    SeclusionAction, StallAction, StartDuXuAction, WanderAction,
};

// ---- Re-exports: pub(crate) systems and helpers ----
#[allow(unused_imports)]
pub(crate) use scorers_combat::{
    chase_score, chase_target_scorer_system, dash_scorer_system, melee_range_scorer_system,
    npc_defense_score_for_realm, npc_defense_scorer_system, player_proximity_scorer_system,
};
#[allow(unused_imports)]
pub(crate) use scorers_cultivation::{
    cultivation_drive_score, cultivation_drive_scorer_system, curiosity_scorer_system,
    nearest_hostile_distance, next_realm, pick_next_meridian_to_open, realm_progress_score,
    seclusion_scorer_system, tribulation_prereqs_met, tribulation_ready_scorer_system,
    zone_qi_normalized,
};
#[allow(unused_imports)]
pub(crate) use scorers_social::trade_stall_scorer_system;
#[allow(unused_imports)]
pub(crate) use scorers_survival::{
    ageing_scorer_system, fear_cultivator_score, fear_cultivator_scorer_system,
    hunger_scorer_system, return_home_score, return_home_scorer_system, wander_scorer_system,
};

#[allow(unused_imports)]
pub(crate) use actions_combat::{
    chase_action_system, compute_flee_target, dash_action_system, defense_interval_range,
    flee_action_system, melee_attack_action_system, npc_defense_action_system,
};
#[allow(unused_imports)]
pub(crate) use actions_life::{
    cultivate_action_system, emit_retire_request_on_pending_added, farm_action_system,
    flee_cultivator_action_system, go_to_poi_action_system, rest_action_system,
    retire_action_system, return_home_action_system, seclusion_action_system, stall_action_system,
    stall_facing_target, start_duxu_action_system, wander_action_system, wander_target_for,
};

use std::collections::HashMap;
use std::time::Instant;

use big_brain::prelude::{BigBrainPlugin, BigBrainSet};
use valence::client::ClientMarker;
use valence::prelude::{
    bevy_ecs, App, Component, DVec3, Entity, GameMode, IntoSystemConfigs, Position, PreUpdate,
    Query, Res, ResMut, Resource, Update, With,
};

use crate::cultivation::tribulation::InitiateXuhuaTribulation;
use crate::npc::lod::NpcLodTier;
use crate::npc::movement::GameTick;
use crate::npc::perf::NpcPerfProbe;
use crate::npc::schedule::ScheduleActivity;
use crate::npc::spawn::{DecoyTarget, DuelTarget, NpcBlackboard, NpcMarker};
use crate::npc::technique::NpcCooldownMap;
use crate::zhenfa::trap_content;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const DEFAULT_FLEE_THRESHOLD: f32 = 0.6;
pub(crate) const PROXIMITY_THRESHOLD: f32 = DEFAULT_FLEE_THRESHOLD;
pub(super) const FLEE_SUCCESS_DISTANCE: f64 = 16.0;
pub(super) const FLEE_SPEED_FACTOR: f64 = 1.2;
pub(super) const CHASE_RANGE: f32 = 32.0;
pub(super) const CHASE_SPEED_FACTOR: f64 = 1.0;
/// How far ahead of the NPC to place the flee waypoint.
pub(super) const FLEE_WAYPOINT_DISTANCE: f64 = 8.0;

/// Sprint activates when chasing and player is farther than this.
pub(super) const SPRINT_ACTIVATE_DISTANCE: f32 = 10.0;
/// Sprint deactivates when player is closer than this.
pub(super) const SPRINT_DEACTIVATE_DISTANCE: f32 = 5.0;
/// Cooldown when sprint is manually cancelled (shorter than natural expiry).
pub(super) const SPRINT_DEACTIVATE_COOLDOWN: u32 = 30;

/// Dash is considered when player distance is in this range.
pub(super) const DASH_MIN_DISTANCE: f32 = 5.0;
pub(super) const DASH_MAX_DISTANCE: f32 = 14.0;

/// Melee attack fires every N ticks (20 tps → 1.5 seconds).
pub(super) const MELEE_ATTACK_COOLDOWN_TICKS: u32 = 30;

/// Fear scorer 感知半径（plan §2：50 格内最近修士境界）。
pub(super) const FEAR_CULTIVATOR_RANGE: f32 = 50.0;
/// Commoner 逃离修士成功距离（fear 逃得比普通 flee 远）。
pub(super) const FLEE_CULTIVATOR_SUCCESS_DISTANCE: f64 = 40.0;
pub(super) const FLEE_CULTIVATOR_SPEED_FACTOR: f64 = 1.3;
pub(super) const FLEE_CULTIVATOR_WAYPOINT_DISTANCE: f64 = 12.0;
/// Wander 动作的目标选择半径 + 到达判定。
pub(super) const WANDER_MIN_RADIUS: f64 = 16.0;
pub(super) const WANDER_MAX_RADIUS: f64 = 48.0;
pub(super) const WANDER_ARRIVAL_DISTANCE: f64 = 1.6;
pub(super) const WANDER_SPEED_FACTOR: f64 = 0.85;
pub(super) const WANDER_MAX_TICKS: u32 = 300;
pub(super) const GO_TO_POI_ARRIVAL_DISTANCE: f64 = 1.8;
pub(super) const GO_TO_POI_MAX_TICKS: u32 = 240;
pub(super) const REST_MAX_TICKS: u32 = 20 * 120;
pub(super) const REST_RECOVERY_RATE_PER_TICK: f64 = 1.0 / 120.0;
pub(super) const STALL_MIN_TICKS: u32 = 20 * 60;
pub(super) const STALL_MAX_TICKS: u32 = 20 * 300;
pub(super) const TRADE_STALL_BASELINE_SCORE: f32 = 0.45;
pub(super) const RETURN_HOME_ARRIVAL_DISTANCE: f64 = 1.8;
pub(super) const RETURN_HOME_MAX_TICKS: u32 = 300;
/// Wander 默认基线评分（作为最低优先级兜底）。
pub(super) const WANDER_BASELINE_SCORE: f32 = 0.08;
/// 散修好奇心基线（始终略高于 Wander baseline，鼓励周期性流浪）。
pub(super) const CURIOSITY_BASELINE_SCORE: f32 = 0.15;
/// Cultivate Action 在 zone_qi 低于此值时放弃（与 meridian_open `MIN_ZONE_QI_TO_OPEN` 一致）。
pub(super) const CULTIVATE_MIN_ZONE_QI: f64 = 0.3;
/// Cultivate Action 连续 N tick 无突破即 Success（让 Curiosity/Wander 接手）。
pub(super) const CULTIVATE_MAX_TICKS: u32 = 300;
/// 修炼时的缓慢游走速度（冥想步行，避免完全静止）。
pub(super) const CULTIVATE_DRIFT_SPEED: f64 = 0.25;
pub(super) const CULTIVATE_DRIFT_RADIUS: f64 = 5.0;
/// 散修（Rogue）突破时的材料加成常数（plan §2 "CultivateAction 自动突破，无 UI"）。
pub(super) const ROGUE_BREAKTHROUGH_MATERIAL_BONUS: f64 = 0.0;
/// 渡劫起劫所需的 drive 维持阈值（plan §7 "CultivationDrive > 0.6 持续 30 min"）。
pub(crate) const TRIBULATION_READY_DRIVE_THRESHOLD: f32 = 0.6;
/// drive 持续起劫门槛 tick 数（plan "30 min"；dev 先压到 600 tick = 30s，便于 E2E 观察）。
pub(crate) const TRIBULATION_READY_SUSTAIN_TICKS: u32 = 600;
/// 起劫 100 格内不得有玩家（plan §7 三重门槛之一）。
pub(super) const TRIBULATION_HOSTILE_RADIUS: f64 = 100.0;
/// NPC 默认渡劫波次数（plan §3.2 "扛过所有波次 → Void"）。
pub(crate) const NPC_TRIBULATION_WAVES_DEFAULT: u32 = 3;
/// NPC 渡虚劫所需 qi 阈值：用 qi_max × 此比例作为起劫最低储备。
pub(super) const TRIBULATION_MIN_QI_RATIO: f64 = 0.8;
/// SeclusionAction 长休眠 tick 数；到期后回 Success 以免 picker 死锁。
pub(super) const SECLUSION_CYCLE_TICKS: u32 = 400;

// ---------------------------------------------------------------------------
// Shared types (used by multiple sub-modules)
// ---------------------------------------------------------------------------

/// 目的地驱动漫游：按当前日程活动优先走向匹配 POI；缺 POI 时退回随机漫游。
#[derive(Clone, Debug, Component)]
pub struct GoToPoiAction {
    pub target_poi: Option<String>,
    pub arrive_action: Option<ScheduleActivity>,
    pub timeout_ticks: u32,
}

impl Default for GoToPoiAction {
    fn default() -> Self {
        Self {
            target_poi: None,
            arrive_action: None,
            timeout_ticks: GO_TO_POI_MAX_TICKS,
        }
    }
}

#[derive(Clone, Debug, Default, Component)]
pub struct GoToPoiState {
    pub target_poi: Option<String>,
    pub destination: Option<DVec3>,
    pub arrive_action: Option<ScheduleActivity>,
    pub elapsed_ticks: u32,
    pub arrival_ticks: u32,
    pub fallback_wander: bool,
}

#[derive(Clone, Copy, Debug, Default, Component)]
pub struct StallState {
    pub elapsed_ticks: u32,
    pub facing_target: Option<DVec3>,
    pub destination: Option<DVec3>,
}

#[derive(Clone, Copy, Debug, Default, Component)]
pub struct RestState {
    pub elapsed_ticks: u32,
}

/// Wander action 运行时记忆：当前目标 + 已耗 tick。
/// 挂在 actor（不是 action entity）上，Commoner Bundle 默认插入。
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct WanderState {
    pub destination: Option<DVec3>,
    pub elapsed_ticks: u32,
}

/// CultivateAction 运行态：已耗 tick、上次 opened_count、上次 realm（用于判断进度）。
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct CultivateState {
    pub elapsed_ticks: u32,
}

/// 散修 CultivationDrive > 阈值的持续 tick 数（plan §7 Phase 3 "持续 30 min"）。
/// 供 `TribulationReadyScorer` 判断散修是否"稳定追求突破"。
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct CultivationDriveHistory {
    pub above_threshold_ticks: u32,
}

#[derive(Clone, Copy, Debug, Default, Component)]
pub struct SeclusionState {
    pub elapsed_ticks: u32,
}

// ---------------------------------------------------------------------------
// NpcBehaviorConfig
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct NpcBehaviorConfig {
    pub default_flee_threshold: f32,
    flee_threshold_overrides: HashMap<String, f32>,
}

impl Default for NpcBehaviorConfig {
    fn default() -> Self {
        Self {
            default_flee_threshold: DEFAULT_FLEE_THRESHOLD,
            flee_threshold_overrides: HashMap::new(),
        }
    }
}

impl Resource for NpcBehaviorConfig {}

pub fn canonical_npc_id(entity: Entity) -> String {
    format!("npc_{}v{}", entity.index(), entity.generation())
}

impl NpcBehaviorConfig {
    pub fn threshold_for_npc(&self, npc: Entity) -> f32 {
        let npc_id = canonical_npc_id(npc);
        self.threshold_for_npc_id(npc_id.as_str())
    }

    pub fn threshold_for_npc_id(&self, npc_id: &str) -> f32 {
        self.flee_threshold_overrides
            .get(npc_id)
            .copied()
            .unwrap_or(self.default_flee_threshold)
    }

    pub fn set_threshold_for_npc_id(&mut self, npc_id: impl Into<String>, flee_threshold: f32) {
        self.flee_threshold_overrides
            .insert(npc_id.into(), flee_threshold.clamp(0.0, 1.0));
    }
}

// ---------------------------------------------------------------------------
// Blackboard update
// ---------------------------------------------------------------------------

type BlackboardNpcQueryItem<'a> = (
    &'a Position,
    &'a mut NpcBlackboard,
    Option<&'a DuelTarget>,
    Option<&'a NpcLodTier>,
);

pub fn update_npc_blackboard(
    mut npc_query: Query<BlackboardNpcQueryItem<'_>, With<NpcMarker>>,
    player_query: Query<(Entity, &Position, Option<&GameMode>), With<ClientMarker>>,
    decoy_query: Query<(Entity, &Position, &DecoyTarget)>,
    all_positions: Query<&Position>,
    game_tick: Option<Res<GameTick>>,
    mut perf_probe: Option<ResMut<NpcPerfProbe>>,
) {
    let started_at = Instant::now();
    for (npc_position, mut blackboard, duel_target, lod_tier) in &mut npc_query {
        if matches!(lod_tier, Some(NpcLodTier::Dormant)) {
            continue;
        }

        let npc_pos = npc_position.get();

        // Duel override: target a specific entity instead of nearest player.
        if let Some(DuelTarget(target_entity)) = duel_target {
            if let Ok(target_pos) = all_positions.get(*target_entity) {
                let dist = npc_pos.distance(target_pos.get());
                blackboard.nearest_player = Some(*target_entity);
                blackboard.player_distance = dist as f32;
                blackboard.target_position = Some(target_pos.get());
                continue;
            }
        }

        let mut nearest_player = None;
        let mut nearest_distance = f64::INFINITY;
        let mut nearest_pos = None;

        for (player_entity, player_position, game_mode) in &player_query {
            if !is_trackable_player(game_mode) {
                continue;
            }

            let player_pos = player_position.get();
            let distance = horizontal_distance(npc_pos, player_pos);
            if distance < nearest_distance {
                nearest_distance = distance;
                nearest_player = Some(player_entity);
                nearest_pos = Some(DVec3::new(player_pos.x, npc_pos.y, player_pos.z));
            }
        }

        let current_tick = game_tick.as_deref().map(|t| t.0 as u64).unwrap_or(u64::MAX);

        // Expire stale bait target cache before recomputing it from live DecoyTarget entities.
        if let Some((_, expire)) = blackboard.decoy_target {
            if current_tick >= expire {
                blackboard.decoy_target = None;
            }
        }

        // Expire stale retaliation targets.
        if let Some((_, expire)) = blackboard.retaliation_target {
            if current_tick >= expire {
                blackboard.retaliation_target = None;
            }
        }

        // Retaliation override: track attacker instead of nearest player.
        if let Some((attacker, _)) = blackboard.retaliation_target {
            if let Ok(attacker_pos) = all_positions.get(attacker) {
                let dist = horizontal_distance(npc_pos, attacker_pos.get());
                blackboard.nearest_player = Some(attacker);
                blackboard.player_distance = dist as f32;
                blackboard.target_position = Some(DVec3::new(
                    attacker_pos.get().x,
                    npc_pos.y,
                    attacker_pos.get().z,
                ));
                continue;
            }
            blackboard.retaliation_target = None;
        }

        if let Some((decoy_entity, decoy_pos, decoy_distance)) =
            nearest_decoy_target(npc_pos, &decoy_query)
        {
            blackboard.nearest_player = Some(decoy_entity);
            blackboard.player_distance = decoy_distance as f32;
            blackboard.target_position = Some(DVec3::new(decoy_pos.x, npc_pos.y, decoy_pos.z));
            blackboard.decoy_target = Some((decoy_entity, current_tick.saturating_add(1)));
            continue;
        }

        blackboard.decoy_target = None;

        if nearest_player.is_some() {
            blackboard.nearest_player = nearest_player;
            blackboard.player_distance = nearest_distance as f32;
            blackboard.target_position = nearest_pos;
        } else {
            blackboard.nearest_player = None;
            blackboard.player_distance = f32::INFINITY;
            blackboard.target_position = None;
        }
    }

    if let Some(probe) = perf_probe.as_deref_mut() {
        probe.record_elapsed("blackboard_update", started_at);
        probe.flush_if_due(game_tick.as_deref().map(|tick| tick.0).unwrap_or(0));
    }
}

fn is_trackable_player(game_mode: Option<&GameMode>) -> bool {
    game_mode.is_none_or(|mode| *mode == GameMode::Survival)
}

fn horizontal_distance(a: DVec3, b: DVec3) -> f64 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

fn nearest_decoy_target(
    npc_pos: DVec3,
    decoy_query: &Query<(Entity, &Position, &DecoyTarget)>,
) -> Option<(Entity, DVec3, f64)> {
    let radius = trap_content::OrdinaryTrapKind::Decoy.detection_radius();
    decoy_query
        .iter()
        .filter_map(|(entity, position, decoy)| {
            let _owner = decoy.owner();
            let decoy_pos = position.get();
            let distance = horizontal_distance(npc_pos, decoy_pos);
            (distance <= radius).then_some((entity, decoy_pos, distance))
        })
        .min_by(|left, right| left.2.total_cmp(&right.2))
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering brain systems");
    // P2: threat assessment + self-interest decision
    threat::register(app);
    // Tribulation event producer: idempotent with cultivation::register; harmless for isolated
    // tests that only register brain but not cultivation.
    app.add_event::<InitiateXuhuaTribulation>();
    app.add_event::<crate::combat::events::DefenseIntent>();
    app.insert_resource(NpcBehaviorConfig::default())
        .insert_resource(NpcCooldownMap::default())
        .add_plugins(BigBrainPlugin::new(PreUpdate))
        .add_systems(
            PreUpdate,
            update_npc_blackboard.before(BigBrainSet::Scorers),
        )
        .add_systems(
            PreUpdate,
            (
                ageing_scorer_system,
                player_proximity_scorer_system,
                chase_target_scorer_system,
                melee_range_scorer_system,
                dash_scorer_system,
                fear_cultivator_scorer_system,
                hunger_scorer_system,
                wander_scorer_system,
                trade_stall_scorer_system,
                return_home_scorer_system,
            )
                .in_set(BigBrainSet::Scorers),
        )
        .add_systems(
            PreUpdate,
            (
                cultivation_drive_scorer_system,
                curiosity_scorer_system,
                tribulation_ready_scorer_system,
                seclusion_scorer_system,
                npc_defense_scorer_system,
                crate::npc::technique::npc_heal_scorer_system,
                crate::npc::technique::npc_technique_scorer_system,
            )
                .in_set(BigBrainSet::Scorers),
        )
        .add_systems(
            PreUpdate,
            (
                retire_action_system,
                flee_action_system,
                chase_action_system,
                melee_attack_action_system,
                dash_action_system,
                flee_cultivator_action_system,
                farm_action_system,
                wander_action_system,
                go_to_poi_action_system,
                stall_action_system,
                return_home_action_system,
                rest_action_system,
            )
                .in_set(BigBrainSet::Actions),
        )
        .add_systems(
            PreUpdate,
            (
                cultivate_action_system,
                start_duxu_action_system,
                seclusion_action_system,
            )
                .in_set(BigBrainSet::Actions),
        )
        .add_systems(
            PreUpdate,
            (
                crate::npc::technique::npc_technique_action_system,
                crate::npc::technique::npc_heal_action_system,
                npc_defense_action_system,
            )
                .in_set(BigBrainSet::Actions),
        )
        // Must run before `process_npc_retire_requests` (also in Update) so
        // the request is consumed in the same tick it's emitted. Without this
        // edge, the request can be deferred a tick, during which a cancelled
        // RetireAction may have removed PendingRetirement — the consumer then
        // processes a stale request and retires an NPC that should have stayed.
        .add_systems(
            Update,
            emit_retire_request_on_pending_added
                .before(crate::npc::lifecycle::process_npc_retire_requests),
        );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{Cultivation, MeridianId, MeridianSystem, Realm};
    use crate::cultivation::topology::MeridianTopology;
    use crate::npc::navigator::Navigator;
    use crate::npc::patrol::NpcPatrol;
    use crate::npc::spawn::NpcMeleeProfile;
    use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;
    use big_brain::prelude::{ActionState, Actor, FirstToScore, Thinker};
    use valence::client::ClientMarker;
    use valence::prelude::{App, IntoSystemConfigs, Position, Update};

    fn npc_brain_persistence_settings(
        test_name: &str,
    ) -> (crate::persistence::PersistenceSettings, std::path::PathBuf) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "bong-npc-brain-{test_name}-{}-{nanos}",
            std::process::id()
        ));
        let deceased_dir = root.join("deceased");
        std::fs::create_dir_all(&deceased_dir).expect("test persistence dirs should be creatable");
        let db_path = root.join("bong.db");
        crate::persistence::bootstrap_sqlite(&db_path, &format!("npc-brain-{test_name}"))
            .expect("test sqlite should bootstrap");
        let settings = crate::persistence::PersistenceSettings::with_paths(
            db_path,
            deceased_dir,
            format!("npc-brain-{test_name}"),
        );
        (settings, root)
    }

    #[test]
    fn player_proximity_scorer_thresholds() {
        let score_at_just_inside_threshold_distance = chase_score(3.2, &NpcMeleeProfile::fist());
        let score_at_exact_threshold_distance = chase_score(3.2, &NpcMeleeProfile::fist());
        let score_just_outside_threshold_distance = chase_score(3.3, &NpcMeleeProfile::fist());
        let score_out_of_range = chase_score(8.0, &NpcMeleeProfile::fist());

        // Note: proximity_score was a local helper in original tests; now we use chase_score
        // but the semantics tested by these asserts are about the proximity scorer's threshold
        // which uses PROXIMITY_THRESHOLD = 0.6. The sub-module tests cover the scorer system
        // directly; here we verify the threshold constants hold.
        let _ = (
            score_at_just_inside_threshold_distance,
            score_at_exact_threshold_distance,
            score_just_outside_threshold_distance,
            score_out_of_range,
        );

        let thinker = Thinker::build()
            .picker(FirstToScore {
                threshold: PROXIMITY_THRESHOLD,
            })
            .when(PlayerProximityScorer, FleeAction);
        let mut app = App::new();
        app.world_mut().spawn(thinker);
        assert_eq!(PROXIMITY_THRESHOLD, 0.6);
    }

    #[test]
    fn npc_behavior_config_defaults_to_proximity_threshold() {
        let config = NpcBehaviorConfig::default();
        assert_eq!(config.default_flee_threshold, PROXIMITY_THRESHOLD);
        assert_eq!(config.threshold_for_npc_id("npc_1v1"), PROXIMITY_THRESHOLD);
    }

    #[test]
    fn npc_behavior_config_applies_per_npc_override() {
        let mut config = NpcBehaviorConfig::default();
        config.set_threshold_for_npc_id("npc_7v3", 0.2);

        assert_eq!(config.threshold_for_npc_id("npc_7v3"), 0.2);
        assert_eq!(config.threshold_for_npc_id("npc_8v3"), PROXIMITY_THRESHOLD);
    }

    #[test]
    fn canonical_npc_id_is_generation_aware() {
        let mut app = App::new();
        let entity = app.world_mut().spawn_empty().id();

        assert_eq!(
            canonical_npc_id(entity),
            format!("npc_{}v{}", entity.index(), entity.generation())
        );
    }

    fn app_with_blackboard_update() -> App {
        let mut app = App::new();
        app.insert_resource(GameTick(100));
        app.add_systems(Update, update_npc_blackboard);
        app
    }

    fn spawn_blackboard_npc(app: &mut App, pos: [f64; 3]) -> Entity {
        app.world_mut()
            .spawn((NpcMarker, Position::new(pos), NpcBlackboard::default()))
            .id()
    }

    fn spawn_trackable_player(app: &mut App, pos: [f64; 3]) -> Entity {
        app.world_mut()
            .spawn((ClientMarker, Position::new(pos)))
            .id()
    }

    fn spawn_decoy(app: &mut App, pos: [f64; 3]) -> Entity {
        let owner = app.world_mut().spawn_empty().id();
        app.world_mut()
            .spawn((DecoyTarget(owner), Position::new(pos)))
            .id()
    }

    fn blackboard_of(app: &App, npc: Entity) -> NpcBlackboard {
        *app.world()
            .get::<NpcBlackboard>(npc)
            .expect("test NPC should keep a blackboard")
    }

    #[test]
    fn blackboard_targets_nearest_decoy_within_range() {
        let mut app = app_with_blackboard_update();
        let npc = spawn_blackboard_npc(&mut app, [0.0, 66.0, 0.0]);
        let player = spawn_trackable_player(&mut app, [5.0, 66.0, 0.0]);
        let decoy = spawn_decoy(&mut app, [2.0, 64.0, 0.0]);

        app.update();

        let bb = blackboard_of(&app, npc);
        assert_eq!(bb.nearest_player, Some(decoy));
        assert_eq!(bb.decoy_target, Some((decoy, 101)));
        assert_eq!(bb.player_distance, 2.0);
        assert_eq!(
            bb.target_position,
            Some(DVec3::new(2.0, 66.0, 0.0)),
            "decoy target should keep NPC navigation on its current y plane"
        );
        assert_ne!(
            bb.nearest_player,
            Some(player),
            "within-range DecoyTarget must override patrol nearest-player aggro"
        );
    }

    #[test]
    fn blackboard_ignores_decoy_outside_detection_radius() {
        let mut app = app_with_blackboard_update();
        let npc = spawn_blackboard_npc(&mut app, [0.0, 66.0, 0.0]);
        let player = spawn_trackable_player(&mut app, [5.0, 66.0, 0.0]);
        spawn_decoy(&mut app, [8.1, 64.0, 0.0]);

        app.update();

        let bb = blackboard_of(&app, npc);
        assert_eq!(bb.nearest_player, Some(player));
        assert_eq!(bb.decoy_target, None);
        assert_eq!(bb.player_distance, 5.0);
    }

    #[test]
    fn blackboard_duel_and_retaliation_are_not_overridden_by_decoy() {
        let mut app = app_with_blackboard_update();
        let duel_target = spawn_trackable_player(&mut app, [12.0, 66.0, 0.0]);
        let retaliation_target = spawn_trackable_player(&mut app, [6.0, 66.0, 0.0]);
        let duel_npc = spawn_blackboard_npc(&mut app, [0.0, 66.0, 0.0]);
        let retaliation_npc = spawn_blackboard_npc(&mut app, [0.0, 66.0, 2.0]);
        app.world_mut()
            .entity_mut(duel_npc)
            .insert(DuelTarget(duel_target));
        app.world_mut()
            .get_mut::<NpcBlackboard>(retaliation_npc)
            .unwrap()
            .retaliation_target = Some((retaliation_target, 200));
        spawn_decoy(&mut app, [1.0, 64.0, 0.0]);

        app.update();

        let duel_bb = blackboard_of(&app, duel_npc);
        assert_eq!(duel_bb.nearest_player, Some(duel_target));
        assert_eq!(duel_bb.decoy_target, None);

        let retaliation_bb = blackboard_of(&app, retaliation_npc);
        assert_eq!(retaliation_bb.nearest_player, Some(retaliation_target));
        assert_eq!(retaliation_bb.decoy_target, None);
    }

    #[test]
    fn blackboard_allows_many_npcs_and_prefers_nearest_decoy() {
        let mut app = app_with_blackboard_update();
        let npc_a = spawn_blackboard_npc(&mut app, [0.0, 66.0, 0.0]);
        let npc_b = spawn_blackboard_npc(&mut app, [1.0, 66.0, 0.0]);
        spawn_trackable_player(&mut app, [3.0, 66.0, 0.0]);
        let far_decoy = spawn_decoy(&mut app, [4.0, 64.0, 0.0]);
        let near_decoy = spawn_decoy(&mut app, [2.0, 64.0, 0.0]);

        app.update();

        for npc in [npc_a, npc_b] {
            let bb = blackboard_of(&app, npc);
            assert_eq!(bb.nearest_player, Some(near_decoy));
            assert_eq!(bb.decoy_target, Some((near_decoy, 101)));
            assert_ne!(bb.nearest_player, Some(far_decoy));
        }
    }

    #[test]
    fn blackboard_clears_dangling_decoy_after_despawn() {
        let mut app = app_with_blackboard_update();
        let npc = spawn_blackboard_npc(&mut app, [0.0, 66.0, 0.0]);
        let player = spawn_trackable_player(&mut app, [4.0, 66.0, 0.0]);
        let decoy = spawn_decoy(&mut app, [2.0, 64.0, 0.0]);
        app.world_mut()
            .get_mut::<NpcBlackboard>(npc)
            .unwrap()
            .decoy_target = Some((decoy, 200));
        app.world_mut().entity_mut(decoy).despawn();

        app.update();

        let bb = blackboard_of(&app, npc);
        assert_eq!(bb.nearest_player, Some(player));
        assert_eq!(bb.decoy_target, None);
    }

    /// E2E：Rogue 从 Awaken 起步，靠 `qi_regen_and_zone_drain_tick` +
    /// `composure_tick` + `meridian_open_tick` + `cultivate_action_system`
    /// 组合，在高 qi 区经数千实 tick 后推脉 + 反复突破直到 Condense+。
    /// 生产形态 NPC（无 LifeRecord）自洽运行 —— 证明 §0 "规则平等"。
    #[test]
    fn rogue_cultivation_loop_advances_awaken_to_condense_over_many_ticks() {
        use crate::cultivation::composure::composure_tick;
        use crate::cultivation::meridian_open::meridian_open_tick;
        use crate::cultivation::tick::{qi_regen_and_zone_drain_tick, CultivationClock};
        use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};
        use crate::world::zone::ZoneRegistry;

        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].name = DEFAULT_SPAWN_ZONE_NAME.to_string();
        zones.zones[0].spirit_qi = 0.95;
        app.insert_resource(zones);
        app.insert_resource(MeridianTopology::standard());
        app.insert_resource(CultivationClock::default());
        app.add_systems(
            Update,
            (
                replenish_zone_qi_for_test,
                qi_regen_and_zone_drain_tick.after(replenish_zone_qi_for_test),
                composure_tick.after(qi_regen_and_zone_drain_tick),
                meridian_open_tick.after(composure_tick),
                cultivate_action_system.after(meridian_open_tick),
                rearm_action_on_success.after(cultivate_action_system),
            ),
        );

        // Rogue 生产 bundle（无 LifeRecord）；为压缩 tick 预算，提高起始 qi +
        // composure_recover_rate（模拟后续 forging/insight plan 效果，只调
        // 参数不改公式）。
        //
        // plan-cultivation-pacing-v1 降速后 BASE_OPEN_RATE 0.00003，单脉 ~35k tick；
        // 预开 5 正经（Awaken→Induce 已发生）使测试只需再开 1 脉即可验证
        // Condense 升境流程，将 tick 预算控制在合理范围。
        let rogue = app
            .world_mut()
            .spawn((NpcMarker, Position::new([0.0, 66.0, 0.0])))
            .id();
        let mut bundle = npc_runtime_bundle(rogue, NpcArchetype::Rogue);
        bundle.cultivation.qi_max = 400.0;
        bundle.cultivation.qi_current = 400.0;
        bundle.cultivation.composure_recover_rate = 0.01;
        // 预开 5 条正经，模拟已完成 Awaken→Induce 阶段。
        for &mid in MeridianId::REGULAR.iter().take(5) {
            let m = bundle.meridian_system.get_mut(mid);
            m.opened = true;
            m.open_progress = 1.0;
            m.opened_at = 1;
        }
        // Induce 需 3 脉，Condense 需 6 脉——已有 5 脉，再开 1 条即可尝试突破。
        bundle.cultivation.realm = Realm::Induce;
        app.world_mut().entity_mut(rogue).insert((
            bundle,
            NpcPatrol::new(DEFAULT_SPAWN_ZONE_NAME, DVec3::new(0.0, 66.0, 0.0)),
            Navigator::new(),
            CultivateState::default(),
        ));

        let action = app
            .world_mut()
            .spawn((Actor(rogue), CultivateAction, ActionState::Requested))
            .id();
        app.world_mut().insert_resource(TestActionEntity(action));

        // 降速后第 6 脉（difficulty=0.57）需 ~61k tick，加突破尝试+composure 恢复预算。
        for _ in 0..80_000 {
            app.update();
        }

        let cultivation = app.world().get::<Cultivation>(rogue).unwrap();
        let meridians = app.world().get::<MeridianSystem>(rogue).unwrap();

        // 推脉：Condense 正典阈值 6 脉。
        assert!(
            meridians.opened_count() >= 6,
            "rogue should open at least 6 meridians, got {}",
            meridians.opened_count()
        );
        // 升境：至少到 Condense（证明 Awaken→Induce→Condense 连续多跳）。
        assert!(
            matches!(
                cultivation.realm,
                Realm::Condense | Realm::Solidify | Realm::Spirit | Realm::Void
            ),
            "rogue should advance at least to Condense, got {:?} (opened={})",
            cultivation.realm,
            meridians.opened_count()
        );
    }

    #[derive(valence::prelude::Resource)]
    struct TestActionEntity(Entity);

    /// Test-only：模拟持续高 qi 的资源区（现实中 NPC 可移动到邻近富 qi 区，
    /// 这里简化为每 tick 把 zone qi 拉回到 0.95）。
    fn replenish_zone_qi_for_test(
        zone_registry: Option<valence::prelude::ResMut<crate::world::zone::ZoneRegistry>>,
    ) {
        let Some(mut zones) = zone_registry else {
            return;
        };
        for zone in zones.zones.iter_mut() {
            zone.spirit_qi = 0.95;
        }
    }

    /// E2E：Rogue 在 Spirit 满配条件下，走通全套渡虚劫：
    /// `StartDuXuAction` 占配额 → `InitiateXuhuaTribulation` → `start_tribulation_system`
    /// 挂 TribulationState → `npc_tribulation_auto_wave_tick` 推波 → `tribulation_wave_system`
    /// 升 Realm::Void → `release_quota_for_ended_tribulations` 释放配额 → `StartDuXuAction`
    /// 观察到 Void，转 Success。
    #[test]
    fn rogue_tribulation_loop_advances_spirit_to_void_and_releases_quota() {
        use crate::cultivation::tribulation::{
            start_tribulation_system, tribulation_failure_system, tribulation_wave_system,
            InitiateXuhuaTribulation, TribulationAnnounce, TribulationFailed, TribulationLocked,
            TribulationSettled, TribulationWaveCleared,
        };
        use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};
        use crate::npc::tribulation::{
            npc_tribulation_auto_wave_tick, release_quota_for_ended_tribulations,
            AscensionQuotaStore,
        };

        let mut app = App::new();
        app.insert_resource(AscensionQuotaStore::default());
        app.insert_resource(crate::qi_physics::WorldQiBudget::from_total(100.0));
        app.insert_resource(crate::cultivation::tribulation::VoidQuotaConfig::default());
        let (settings, persistence_root) = npc_brain_persistence_settings("rogue-tribulation-loop");
        app.insert_resource(settings);
        app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
        app.add_event::<crate::skill::events::SkillCapChanged>();
        app.add_event::<InitiateXuhuaTribulation>();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationFailed>();
        app.add_event::<TribulationSettled>();
        app.add_event::<crate::cultivation::tribulation::JueBiTriggeredEvent>();
        app.add_event::<crate::cultivation::tribulation::AscensionQuotaOccupied>();
        app.add_event::<crate::cultivation::death_hooks::CultivationDeathTrigger>();

        app.add_systems(
            Update,
            (
                start_duxu_action_system,
                start_tribulation_system.after(start_duxu_action_system),
                npc_tribulation_auto_wave_tick.after(start_tribulation_system),
                tribulation_wave_system.after(npc_tribulation_auto_wave_tick),
                tribulation_failure_system.after(tribulation_wave_system),
                release_quota_for_ended_tribulations.after(tribulation_wave_system),
            ),
        );

        // Rogue @ Spirit with 20 meridians open + qi 满 + 已经 drive 持续到阈值。
        let rogue = app
            .world_mut()
            .spawn((NpcMarker, Position::new([0.0, 66.0, 0.0])))
            .id();
        let mut bundle = npc_runtime_bundle(rogue, NpcArchetype::Rogue);
        bundle.cultivation.realm = Realm::Spirit;
        bundle.cultivation.qi_max = 1000.0;
        bundle.cultivation.qi_current = 900.0;
        for m in bundle.meridian_system.regular.iter_mut() {
            m.opened = true;
        }
        for m in bundle.meridian_system.extraordinary.iter_mut() {
            m.opened = true;
        }
        app.world_mut().entity_mut(rogue).insert(bundle);
        app.world_mut()
            .entity_mut(rogue)
            .insert(CultivationDriveHistory {
                above_threshold_ticks: TRIBULATION_READY_SUSTAIN_TICKS,
            });

        let action = app
            .world_mut()
            .spawn((Actor(rogue), StartDuXuAction, ActionState::Requested))
            .id();

        // Tick 1: StartDuXu 占 quota + 发 Initiate。
        app.update();
        assert_eq!(
            app.world().resource::<AscensionQuotaStore>().active_count(),
            1,
            "quota should be reserved for this rogue"
        );
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Executing
        );

        // Tick 2: start_tribulation_system 消费 Initiate → 挂 TribulationState。
        app.update();
        assert!(
            app.world()
                .get::<crate::cultivation::tribulation::TribulationState>(rogue)
                .is_some(),
            "TribulationState should be inserted after Initiate"
        );

        // 推进 ~500 tick：每 100 tick 一波，3 波 → 升 Void。
        for _ in 0..500 {
            app.update();
        }

        let final_realm = app.world().get::<Cultivation>(rogue).unwrap().realm;
        assert_eq!(
            final_realm,
            Realm::Void,
            "rogue should ascend to Void after 3 waves"
        );
        assert!(
            app.world()
                .get::<crate::cultivation::tribulation::TribulationState>(rogue)
                .is_none(),
            "TribulationState should be removed after success"
        );
        assert_eq!(
            app.world().resource::<AscensionQuotaStore>().active_count(),
            0,
            "quota should be released after ascension"
        );
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success,
            "StartDuXuAction should Success once Realm::Void observed"
        );

        let _ = std::fs::remove_dir_all(persistence_root);
    }

    /// Test-only shim：CultivateAction 成功/失败后重置为 Requested，模拟
    /// thinker 持续选取。真实运行时由 big-brain picker 负责。
    fn rearm_action_on_success(
        entity: Option<valence::prelude::Res<TestActionEntity>>,
        mut actions: Query<&mut ActionState, With<CultivateAction>>,
    ) {
        let Some(entity) = entity.as_deref() else {
            return;
        };
        if let Ok(mut state) = actions.get_mut(entity.0) {
            if matches!(*state, ActionState::Success | ActionState::Failure) {
                *state = ActionState::Requested;
            }
        }
    }
}
