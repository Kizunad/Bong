use big_brain::prelude::{
    ActionBuilder, ActionState, Actor, BigBrainPlugin, BigBrainSet, Score, ScorerBuilder,
};
use std::collections::HashMap;
use valence::client::ClientMarker;
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, EntityKind, EventWriter, IntoSystemConfigs,
    Position, PreUpdate, Query, Res, ResMut, Resource, With, Without,
};

use crate::combat::events::AttackIntent;
use crate::cultivation::breakthrough::{
    breakthrough_qi_cost, try_breakthrough, BreakthroughError, BreakthroughSuccess, XorshiftRoll,
};
use crate::cultivation::components::{Cultivation, MeridianId, MeridianSystem, Realm};
use crate::cultivation::meridian_open::MeridianTarget;
use crate::cultivation::topology::MeridianTopology;
use crate::cultivation::tribulation::{InitiateXuhuaTribulation, TribulationState};
use crate::npc::hunger::{Hunger, HungerConfig};
use crate::npc::lifecycle::{
    NpcAgingConfig, NpcArchetype, NpcLifespan, NpcRegistry, NpcRetireRequest, PendingRetirement,
};
use crate::npc::lod::{is_dormant, should_skip_scorer_tick, NpcLodConfig, NpcLodTick, NpcLodTier};
use crate::npc::movement::{
    activate_dash, activate_sprint, GameTick, MovementCapabilities, MovementController,
    MovementCooldowns, MovementMode,
};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{DuelTarget, NpcBlackboard, NpcMarker, NpcMeleeProfile};
use crate::npc::tribulation::{AscensionQuotaStore, NpcTribulationPacing};
use crate::world::zone::{Zone, ZoneRegistry};

pub const DEFAULT_FLEE_THRESHOLD: f32 = 0.6;
pub(crate) const PROXIMITY_THRESHOLD: f32 = DEFAULT_FLEE_THRESHOLD;
const FLEE_SUCCESS_DISTANCE: f64 = 16.0;
const FLEE_SPEED_FACTOR: f64 = 1.2;
const CHASE_RANGE: f32 = 32.0;
const CHASE_SPEED_FACTOR: f64 = 1.0;
/// How far ahead of the NPC to place the flee waypoint.
const FLEE_WAYPOINT_DISTANCE: f64 = 8.0;

/// Sprint activates when chasing and player is farther than this.
const SPRINT_ACTIVATE_DISTANCE: f32 = 10.0;
/// Sprint deactivates when player is closer than this.
const SPRINT_DEACTIVATE_DISTANCE: f32 = 5.0;
/// Cooldown when sprint is manually cancelled (shorter than natural expiry).
const SPRINT_DEACTIVATE_COOLDOWN: u32 = 30;

/// Dash is considered when player distance is in this range.
const DASH_MIN_DISTANCE: f32 = 5.0;
const DASH_MAX_DISTANCE: f32 = 14.0;

/// Melee attack fires every N ticks (20 tps → 1.5 seconds).
const MELEE_ATTACK_COOLDOWN_TICKS: u32 = 30;

/// Fear scorer 感知半径（plan §2：50 格内最近修士境界）。
const FEAR_CULTIVATOR_RANGE: f32 = 50.0;
/// Commoner 逃离修士成功距离（fear 逃得比普通 flee 远）。
const FLEE_CULTIVATOR_SUCCESS_DISTANCE: f64 = 40.0;
const FLEE_CULTIVATOR_SPEED_FACTOR: f64 = 1.3;
const FLEE_CULTIVATOR_WAYPOINT_DISTANCE: f64 = 12.0;
/// Wander 动作的目标选择半径 + 到达判定。
const WANDER_MIN_RADIUS: f64 = 4.0;
const WANDER_MAX_RADIUS: f64 = 10.0;
const WANDER_ARRIVAL_DISTANCE: f64 = 1.6;
const WANDER_SPEED_FACTOR: f64 = 0.6;
const WANDER_MAX_TICKS: u32 = 200;
/// Wander 默认基线评分（作为最低优先级兜底）。
const WANDER_BASELINE_SCORE: f32 = 0.08;
/// 散修好奇心基线（始终略高于 Wander baseline，鼓励周期性流浪）。
const CURIOSITY_BASELINE_SCORE: f32 = 0.15;
/// Cultivate Action 在 zone_qi 低于此值时放弃（与 meridian_open `MIN_ZONE_QI_TO_OPEN` 一致）。
const CULTIVATE_MIN_ZONE_QI: f64 = 0.3;
/// Cultivate Action 连续 N tick 无突破即 Success（让 Curiosity/Wander 接手）。
const CULTIVATE_MAX_TICKS: u32 = 600;
/// 散修（Rogue）突破时的材料加成常数（plan §2 "CultivateAction 自动突破，无 UI"）。
const ROGUE_BREAKTHROUGH_MATERIAL_BONUS: f64 = 0.0;
/// 渡劫起劫所需的 drive 维持阈值（plan §7 "CultivationDrive > 0.6 持续 30 min"）。
pub(crate) const TRIBULATION_READY_DRIVE_THRESHOLD: f32 = 0.6;
/// drive 持续起劫门槛 tick 数（plan "30 min"；dev 先压到 600 tick = 30s，便于 E2E 观察）。
pub(crate) const TRIBULATION_READY_SUSTAIN_TICKS: u32 = 600;
/// 起劫 100 格内不得有玩家（plan §7 三重门槛之一）。
const TRIBULATION_HOSTILE_RADIUS: f64 = 100.0;
/// NPC 默认渡劫波次数（plan §3.2 "扛过所有波次 → Void"）。
pub(crate) const NPC_TRIBULATION_WAVES_DEFAULT: u32 = 3;
/// NPC 渡虚劫所需 qi 阈值：用 qi_max × 此比例作为起劫最低储备。
const TRIBULATION_MIN_QI_RATIO: f64 = 0.8;
/// SeclusionAction 长休眠 tick 数；到期后回 Success 以免 picker 死锁。
const SECLUSION_CYCLE_TICKS: u32 = 400;

#[derive(Clone, Copy, Debug, Component)]
pub struct PlayerProximityScorer;

/// Scores high when a player is within [`CHASE_RANGE`] blocks.
#[derive(Clone, Copy, Debug, Component)]
pub struct ChaseTargetScorer;

/// Scores high (1.0) when a player is within [`MELEE_RANGE`] blocks.
#[derive(Clone, Copy, Debug, Component)]
pub struct MeleeRangeScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct FleeAction;

/// Moves the NPC toward the nearest player.
#[derive(Clone, Copy, Debug, Component)]
pub struct ChaseAction;

/// NPC stands still "attacking" while the player is in melee range.
#[derive(Clone, Copy, Debug, Component)]
pub struct MeleeAttackAction;

/// Scores high when the player is within dash range and dash is off cooldown.
#[derive(Clone, Copy, Debug, Component)]
pub struct DashScorer;

/// Activates a dash toward the player (Override movement).
#[derive(Clone, Copy, Debug, Component)]
pub struct DashAction;

#[derive(Clone, Copy, Debug, Component)]
pub struct AgeingScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct RetireAction;

/// 凡人看见修士（Realm > Awaken）的畏惧评分（plan §2 FearCultivatorScorer）。
#[derive(Clone, Copy, Debug, Component)]
pub struct FearCultivatorScorer;

/// Commoner/Beast 饥饿评分：`Hunger` 越低分越高。
#[derive(Clone, Copy, Debug, Component)]
pub struct HungerScorer;

/// 凡人日常漫游评分（作为兜底基线，始终 > picker threshold）。
#[derive(Clone, Copy, Debug, Component)]
pub struct WanderScorer;

/// 远离最近修士（走得比普通 FleeAction 远）。
#[derive(Clone, Copy, Debug, Component)]
pub struct FleeCultivatorAction;

/// 原地耕作：停 Navigator、按 tick 回补 Hunger，饱腹 → Success。
#[derive(Clone, Copy, Debug, Component)]
pub struct FarmAction;

/// 随机漫步，失败/到达即 Success。
#[derive(Clone, Copy, Debug, Component)]
pub struct WanderAction;

/// Wander action 运行时记忆：当前目标 + 已耗 tick。
/// 挂在 actor（不是 action entity）上，Commoner Bundle 默认插入。
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct WanderState {
    pub destination: Option<DVec3>,
    pub elapsed_ticks: u32,
}

/// 散修推进经脉 + 自动突破评分：`realm_progress × zone_spirit_qi_normalized`。
#[derive(Clone, Copy, Debug, Component)]
pub struct CultivationDriveScorer;

/// 散修好奇心评分（P2 占位：目前只给 baseline，等 POI 系统后再接）。
#[derive(Clone, Copy, Debug, Component)]
pub struct CuriosityScorer;

/// 坐关推脉：挂 MeridianTarget + 停 Navigator，达阈 inline 突破。
#[derive(Clone, Copy, Debug, Component)]
pub struct CultivateAction;

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

/// 渡虚劫 Ready 评分（Realm=Spirit + 20 脉全通 + 无敌意 + drive 持续）。
#[derive(Clone, Copy, Debug, Component)]
pub struct TribulationReadyScorer;

/// 化虚后隐世评分：Realm=Void 时始终为 1.0（压过其它行为，除 Retire 外）。
#[derive(Clone, Copy, Debug, Component)]
pub struct SeclusionScorer;

/// 主动起渡虚劫：占配额 + 发 `InitiateXuhuaTribulation`，守望至成功/失败。
#[derive(Clone, Copy, Debug, Component)]
pub struct StartDuXuAction;

/// 化虚 NPC 的隐世行为：停 Navigator、长时间静止。
#[derive(Clone, Copy, Debug, Component)]
pub struct SeclusionAction;

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

type NpcGoalQueryItem<'a> = (
    &'a Position,
    &'a NpcBlackboard,
    &'a NpcPatrol,
    &'a NpcMeleeProfile,
    &'a mut Navigator,
);
type NpcGoalQueryFilter = (With<NpcMarker>, With<EntityKind>, Without<ClientMarker>);
type NpcFleeQueryItem<'a> = (
    &'a Position,
    &'a NpcBlackboard,
    &'a NpcPatrol,
    &'a mut Navigator,
);

impl ScorerBuilder for PlayerProximityScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("PlayerProximityScorer")
    }
}

impl ActionBuilder for FleeAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("FleeAction")
    }
}

impl ScorerBuilder for ChaseTargetScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("ChaseTargetScorer")
    }
}

impl ScorerBuilder for MeleeRangeScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("MeleeRangeScorer")
    }
}

impl ActionBuilder for ChaseAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("ChaseAction")
    }
}

impl ActionBuilder for MeleeAttackAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("MeleeAttackAction")
    }
}

impl ScorerBuilder for DashScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("DashScorer")
    }
}

impl ActionBuilder for DashAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("DashAction")
    }
}

impl ScorerBuilder for AgeingScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("AgeingScorer")
    }
}

impl ActionBuilder for RetireAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("RetireAction")
    }
}

impl ScorerBuilder for FearCultivatorScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("FearCultivatorScorer")
    }
}

impl ScorerBuilder for HungerScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("HungerScorer")
    }
}

impl ScorerBuilder for WanderScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("WanderScorer")
    }
}

impl ActionBuilder for FleeCultivatorAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("FleeCultivatorAction")
    }
}

impl ActionBuilder for FarmAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("FarmAction")
    }
}

impl ActionBuilder for WanderAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("WanderAction")
    }
}

impl ScorerBuilder for CultivationDriveScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("CultivationDriveScorer")
    }
}

impl ScorerBuilder for CuriosityScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("CuriosityScorer")
    }
}

impl ActionBuilder for CultivateAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("CultivateAction")
    }
}

impl ScorerBuilder for TribulationReadyScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("TribulationReadyScorer")
    }
}

impl ScorerBuilder for SeclusionScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("SeclusionScorer")
    }
}

impl ActionBuilder for StartDuXuAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("StartDuXuAction")
    }
}

impl ActionBuilder for SeclusionAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("SeclusionAction")
    }
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering brain systems");
    // Tribulation event producer: idempotent with cultivation::register; harmless for isolated
    // tests that only register brain but not cultivation.
    app.add_event::<InitiateXuhuaTribulation>();
    app.insert_resource(NpcBehaviorConfig::default())
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
        );
}

fn ageing_scorer_system(
    npcs: Query<(&NpcLifespan, &NpcArchetype, Option<&PendingRetirement>), With<NpcMarker>>,
    registry: Option<Res<NpcRegistry>>,
    config: Option<Res<NpcAgingConfig>>,
    mut scorers: Query<(&Actor, &mut Score), With<AgeingScorer>>,
) {
    let aging_enabled = config.as_deref().map(|cfg| cfg.enabled).unwrap_or(true);
    let should_reduce_population = registry
        .as_deref()
        .map(NpcRegistry::should_reduce_population)
        .unwrap_or(false);

    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((lifespan, archetype, pending_retirement)) = npcs.get(*actor) {
            if pending_retirement.is_some()
                || !aging_enabled
                || *archetype == NpcArchetype::GuardianRelic
            {
                0.0
            } else if lifespan.is_expired() {
                1.0
            } else if should_reduce_population && lifespan.age_ratio() >= 0.8 {
                0.8
            } else {
                0.0
            }
        } else {
            0.0
        };

        score.set(value);
    }
}

fn retire_action_system(
    mut commands: Commands,
    npcs: Query<(Option<&PendingRetirement>, &NpcLifespan), With<NpcMarker>>,
    mut retire_requests: EventWriter<NpcRetireRequest>,
    mut actions: Query<(&Actor, &mut ActionState), With<RetireAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((pending_retirement, lifespan)) = npcs.get(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                if pending_retirement.is_none() {
                    commands.entity(*actor).insert(PendingRetirement);
                    retire_requests.send(NpcRetireRequest { entity: *actor });
                }
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if pending_retirement.is_some() || lifespan.is_expired() {
                    continue;
                }
                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                commands.entity(*actor).remove::<PendingRetirement>();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

pub fn update_npc_blackboard(
    mut npc_query: Query<(&Position, &mut NpcBlackboard, Option<&DuelTarget>), With<NpcMarker>>,
    player_query: Query<(Entity, &Position), With<ClientMarker>>,
    all_positions: Query<&Position>,
) {
    for (npc_position, mut blackboard, duel_target) in &mut npc_query {
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

        for (player_entity, player_position) in &player_query {
            let distance = npc_pos.distance(player_position.get());
            if distance < nearest_distance {
                nearest_distance = distance;
                nearest_player = Some(player_entity);
                nearest_pos = Some(player_position.get());
            }
        }

        blackboard.nearest_player = nearest_player;
        blackboard.player_distance = nearest_distance as f32;
        blackboard.target_position = nearest_pos;
    }
}

fn player_proximity_scorer_system(
    npcs: Query<(&NpcBlackboard, Option<&NpcLodTier>), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<PlayerProximityScorer>>,
    npc_behavior: Option<Res<NpcBehaviorConfig>>,
    lod_config: Option<Res<NpcLodConfig>>,
    lod_tick: Option<Res<NpcLodTick>>,
) {
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut score) in &mut scorers {
        let flee_threshold = npc_behavior
            .as_deref()
            .map(|behavior| behavior.threshold_for_npc(*actor))
            .unwrap_or(DEFAULT_FLEE_THRESHOLD)
            .clamp(0.0, 1.0);

        let value = if let Ok((blackboard, tier)) = npcs.get(*actor) {
            if is_dormant(tier) {
                0.0
            } else if tier
                .copied()
                .map(|t| should_skip_scorer_tick(t, tick, &cfg))
                .unwrap_or(false)
            {
                continue;
            } else {
                score_for_flee_threshold(
                    proximity_score(blackboard.player_distance),
                    flee_threshold,
                )
            }
        } else {
            0.0
        };

        score.set(value);
    }
}

fn score_for_flee_threshold(score: f32, flee_threshold: f32) -> f32 {
    if score >= flee_threshold {
        1.0
    } else {
        0.0
    }
}

// ---------------------------------------------------------------------------
// Flee action — sets Navigator goal away from player
// ---------------------------------------------------------------------------

fn flee_action_system(
    mut npcs: Query<NpcFleeQueryItem<'_>, NpcGoalQueryFilter>,
    mut actions: Query<(&Actor, &mut ActionState), With<FleeAction>>,
    zone_registry: Option<Res<ZoneRegistry>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((npc_position, blackboard, patrol, mut navigator)) = npcs.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if blackboard.player_distance > FLEE_SUCCESS_DISTANCE as f32 {
                    navigator.stop();
                    *state = ActionState::Success;
                    continue;
                }

                let Some(target_pos) = blackboard.target_position else {
                    continue;
                };

                let flee_target = compute_flee_target(
                    npc_position.get(),
                    target_pos,
                    zone_registry.as_deref(),
                    &patrol.home_zone,
                );
                navigator.set_goal(flee_target, FLEE_SPEED_FACTOR);
            }
            ActionState::Cancelled => {
                navigator.stop();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

/// Compute a point FLEE_WAYPOINT_DISTANCE blocks away from the player,
/// clamped to zone bounds.
fn compute_flee_target(
    npc_pos: DVec3,
    player_pos: DVec3,
    zone_registry: Option<&ZoneRegistry>,
    home_zone: &str,
) -> DVec3 {
    let mut flee_dir = npc_pos - player_pos;
    flee_dir.y = 0.0;

    let dir = if flee_dir.length_squared() <= f64::EPSILON {
        DVec3::new(1.0, 0.0, 0.0)
    } else {
        flee_dir.normalize()
    };

    let target = npc_pos + dir * FLEE_WAYPOINT_DISTANCE;

    // Clamp to zone bounds.
    if let Some(zone) = zone_registry.and_then(|r| r.find_zone_by_name(home_zone)) {
        let (min, max) = zone.bounds;
        DVec3::new(
            target.x.clamp(min.x, max.x),
            target.y,
            target.z.clamp(min.z, max.z),
        )
    } else {
        target
    }
}

fn proximity_score(distance: f32) -> f32 {
    if !distance.is_finite() {
        return 0.0;
    }

    ((8.0 - distance) / 8.0).clamp(0.0, 1.0)
}

#[cfg(test)]
fn should_flee_from_score(score: f32) -> bool {
    score >= PROXIMITY_THRESHOLD
}

// ---------------------------------------------------------------------------
// Chase action — sets Navigator goal toward the player
// ---------------------------------------------------------------------------

fn chase_target_scorer_system(
    npcs: Query<(&NpcBlackboard, &NpcMeleeProfile), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<ChaseTargetScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((bb, profile)) = npcs.get(*actor) {
            chase_score(bb.player_distance, profile)
        } else {
            0.0
        };
        score.set(value);
    }
}

fn chase_score(distance: f32, profile: &NpcMeleeProfile) -> f32 {
    if !distance.is_finite() || distance > CHASE_RANGE {
        return 0.0;
    }
    if distance <= profile.preferred_distance {
        return 0.0;
    }
    ((CHASE_RANGE - distance) / CHASE_RANGE).clamp(0.0, 1.0)
}

fn chase_action_system(
    mut npcs: Query<NpcGoalQueryItem<'_>, NpcGoalQueryFilter>,
    mut movement: Query<
        (
            &mut MovementController,
            &MovementCapabilities,
            &mut MovementCooldowns,
        ),
        With<NpcMarker>,
    >,
    mut actions: Query<(&Actor, &mut ActionState), With<ChaseAction>>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.map(|t| t.0).unwrap_or(0);

    for (Actor(actor), mut state) in &mut actions {
        let Ok((_npc_position, blackboard, _patrol, melee_profile, mut navigator)) =
            npcs.get_mut(*actor)
        else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if blackboard.player_distance <= melee_profile.preferred_distance {
                    navigator.stop();
                    *state = ActionState::Success;
                    continue;
                }

                let Some(target_pos) = blackboard.target_position else {
                    continue;
                };

                navigator.set_goal(target_pos, CHASE_SPEED_FACTOR);

                // Sprint enhancement: activate sprint when chasing at medium range.
                if let Ok((mut ctrl, caps, mut cooldowns)) = movement.get_mut(*actor) {
                    if blackboard.player_distance > SPRINT_ACTIVATE_DISTANCE {
                        activate_sprint(&mut ctrl, caps, &mut cooldowns, tick);
                    } else if blackboard.player_distance < SPRINT_DEACTIVATE_DISTANCE
                        && matches!(ctrl.mode, MovementMode::Sprinting(_))
                    {
                        cooldowns.sprint_ready_at = tick + SPRINT_DEACTIVATE_COOLDOWN;
                        ctrl.reset_to_ground();
                    }
                }
            }
            ActionState::Cancelled => {
                navigator.stop();
                if let Ok((mut ctrl, _, mut cooldowns)) = movement.get_mut(*actor) {
                    if matches!(ctrl.mode, MovementMode::Sprinting(_)) {
                        cooldowns.sprint_ready_at = tick + SPRINT_DEACTIVATE_COOLDOWN;
                    }
                    ctrl.reset_to_ground();
                }
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Melee attack — NPC stands still
// ---------------------------------------------------------------------------

fn melee_range_scorer_system(
    npcs: Query<(&NpcBlackboard, &NpcMeleeProfile), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<MeleeRangeScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((bb, profile)) = npcs.get(*actor) {
            if bb.player_distance <= profile.reach.max {
                1.0
            } else {
                0.0
            }
        } else {
            0.0
        };
        score.set(value);
    }
}

fn melee_attack_action_system(
    mut actions: Query<(&Actor, &mut ActionState), With<MeleeAttackAction>>,
    mut npcs: Query<
        (
            &Position,
            &mut NpcBlackboard,
            &NpcMeleeProfile,
            &mut Navigator,
        ),
        With<NpcMarker>,
    >,
    mut attack_intents: EventWriter<AttackIntent>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.map(|t| t.0).unwrap_or(0);

    for (Actor(actor), mut state) in &mut actions {
        match *state {
            ActionState::Requested => {
                if let Ok((_, _, _, mut nav)) = npcs.get_mut(*actor) {
                    nav.stop();
                }
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok((_npc_pos, mut bb, profile, _)) = npcs.get_mut(*actor) else {
                    continue;
                };

                if bb.player_distance > profile.disengage_distance {
                    *state = ActionState::Success;
                    continue;
                }

                if bb.player_distance > profile.reach.max {
                    continue;
                }

                // Attack on cooldown — emit AttackIntent into shared combat resolver.
                if tick.wrapping_sub(bb.last_melee_tick) >= MELEE_ATTACK_COOLDOWN_TICKS {
                    bb.last_melee_tick = tick;

                    if let Some(target_entity) = bb.nearest_player {
                        if target_entity != *actor {
                            attack_intents.send(AttackIntent {
                                attacker: *actor,
                                target: Some(target_entity),
                                issued_at_tick: u64::from(tick),
                                reach: profile.reach,
                                qi_invest: 10.0,
                                wound_kind: profile.wound_kind,
                                debug_command: None,
                            });
                        }
                    }
                }
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Dash — short-range burst toward the player (Override movement)
// ---------------------------------------------------------------------------

fn dash_scorer_system(
    npcs: Query<
        (
            &NpcBlackboard,
            &MovementCapabilities,
            &MovementCooldowns,
            &MovementController,
        ),
        With<NpcMarker>,
    >,
    mut scorers: Query<(&Actor, &mut Score), With<DashScorer>>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.map(|t| t.0).unwrap_or(0);

    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((bb, caps, cooldowns, ctrl)) = npcs.get(*actor) {
            dash_score(bb, caps, cooldowns, ctrl, tick)
        } else {
            0.0
        };
        score.set(value);
    }
}

fn dash_score(
    bb: &NpcBlackboard,
    caps: &MovementCapabilities,
    cooldowns: &MovementCooldowns,
    ctrl: &MovementController,
    current_tick: u32,
) -> f32 {
    if !caps.can_dash {
        return 0.0;
    }
    if current_tick < cooldowns.dash_ready_at {
        return 0.0;
    }
    if ctrl.navigator_should_yield() {
        return 0.0; // already in an override
    }
    if !bb.player_distance.is_finite() {
        return 0.0;
    }
    if bb.player_distance < DASH_MIN_DISTANCE || bb.player_distance > DASH_MAX_DISTANCE {
        return 0.0;
    }

    // Score high — dash should take priority over regular chase when available.
    0.9
}

#[allow(clippy::type_complexity)]
fn dash_action_system(
    mut npcs: Query<
        (
            &Position,
            &NpcBlackboard,
            &mut Navigator,
            &mut MovementController,
            &MovementCapabilities,
            &mut MovementCooldowns,
        ),
        With<NpcMarker>,
    >,
    mut actions: Query<(&Actor, &mut ActionState), With<DashAction>>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.map(|t| t.0).unwrap_or(0);

    for (Actor(actor), mut state) in &mut actions {
        let Ok((npc_pos, blackboard, mut navigator, mut ctrl, caps, mut cooldowns)) =
            npcs.get_mut(*actor)
        else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                let Some(target) = blackboard.target_position else {
                    *state = ActionState::Failure;
                    continue;
                };

                let npc = npc_pos.get();
                let dir = DVec3::new(target.x - npc.x, 0.0, target.z - npc.z);

                // Stop the navigator — dash takes over.
                navigator.stop();

                let activated = activate_dash(
                    &mut ctrl,
                    caps,
                    &mut cooldowns,
                    tick,
                    dir,
                    npc.y, // ground Y at current position
                );

                if activated {
                    *state = ActionState::Executing;
                } else {
                    *state = ActionState::Failure;
                }
            }
            ActionState::Executing => {
                // Dash is done when MovementController returns to GroundNav.
                if !ctrl.navigator_should_yield() {
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                ctrl.reset_to_ground();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Commoner brains — FearCultivator / Hunger / Wander scorers + actions
// ---------------------------------------------------------------------------

/// 境界 → 畏惧权重（Awaken = 0）。
fn realm_fear_weight(realm: Realm) -> f32 {
    match realm {
        Realm::Awaken => 0.0,
        Realm::Induce => 0.25,
        Realm::Condense => 0.5,
        Realm::Solidify => 0.75,
        Realm::Spirit => 0.9,
        Realm::Void => 1.0,
    }
}

/// 距离衰减：0 → 1；`max_range` 外 → 0。
fn fear_distance_falloff(distance: f32, max_range: f32) -> f32 {
    if !distance.is_finite() || distance >= max_range {
        0.0
    } else {
        (1.0 - distance / max_range).clamp(0.0, 1.0)
    }
}

pub(crate) fn fear_cultivator_score(distance: f32, realm: Realm) -> f32 {
    realm_fear_weight(realm) * fear_distance_falloff(distance, FEAR_CULTIVATOR_RANGE)
}

fn fear_cultivator_scorer_system(
    npcs: Query<&NpcBlackboard, With<NpcMarker>>,
    players: Query<&Cultivation, With<ClientMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<FearCultivatorScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = match npcs.get(*actor) {
            Ok(bb) => match bb.nearest_player.and_then(|e| players.get(e).ok()) {
                Some(cult) => fear_cultivator_score(bb.player_distance, cult.realm),
                None => 0.0,
            },
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

fn hunger_scorer_system(
    npcs: Query<(&Hunger, Option<&NpcLodTier>), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<HungerScorer>>,
    lod_config: Option<Res<NpcLodConfig>>,
    lod_tick: Option<Res<NpcLodTick>>,
) {
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut score) in &mut scorers {
        let value = match npcs.get(*actor) {
            Ok((h, tier)) => {
                if is_dormant(tier) {
                    0.0
                } else if tier
                    .copied()
                    .map(|t| should_skip_scorer_tick(t, tick, &cfg))
                    .unwrap_or(false)
                {
                    continue;
                } else {
                    h.hunger_pressure()
                }
            }
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

fn wander_scorer_system(
    npcs: Query<(Option<&PendingRetirement>, Option<&NpcLodTier>), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<WanderScorer>>,
    lod_config: Option<Res<NpcLodConfig>>,
    lod_tick: Option<Res<NpcLodTick>>,
) {
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut score) in &mut scorers {
        let value = match npcs.get(*actor) {
            Ok((pending, tier)) => {
                if is_dormant(tier) {
                    0.0
                } else if tier
                    .copied()
                    .map(|t| should_skip_scorer_tick(t, tick, &cfg))
                    .unwrap_or(false)
                {
                    continue;
                } else if pending.is_some() {
                    0.0
                } else {
                    WANDER_BASELINE_SCORE
                }
            }
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

fn flee_cultivator_action_system(
    mut npcs: Query<NpcFleeQueryItem<'_>, NpcGoalQueryFilter>,
    mut actions: Query<(&Actor, &mut ActionState), With<FleeCultivatorAction>>,
    zone_registry: Option<Res<ZoneRegistry>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((npc_position, blackboard, patrol, mut navigator)) = npcs.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if blackboard.player_distance > FLEE_CULTIVATOR_SUCCESS_DISTANCE as f32 {
                    navigator.stop();
                    *state = ActionState::Success;
                    continue;
                }

                let Some(target_pos) = blackboard.target_position else {
                    continue;
                };

                let flee_target = compute_cultivator_flee_target(
                    npc_position.get(),
                    target_pos,
                    zone_registry.as_deref(),
                    &patrol.home_zone,
                );
                navigator.set_goal(flee_target, FLEE_CULTIVATOR_SPEED_FACTOR);
            }
            ActionState::Cancelled => {
                navigator.stop();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn compute_cultivator_flee_target(
    npc_pos: DVec3,
    player_pos: DVec3,
    zone_registry: Option<&ZoneRegistry>,
    home_zone: &str,
) -> DVec3 {
    let mut flee_dir = npc_pos - player_pos;
    flee_dir.y = 0.0;

    let dir = if flee_dir.length_squared() <= f64::EPSILON {
        DVec3::new(1.0, 0.0, 0.0)
    } else {
        flee_dir.normalize()
    };

    let target = npc_pos + dir * FLEE_CULTIVATOR_WAYPOINT_DISTANCE;

    if let Some(zone) = zone_registry.and_then(|r| r.find_zone_by_name(home_zone)) {
        let (min, max) = zone.bounds;
        DVec3::new(
            target.x.clamp(min.x, max.x),
            target.y,
            target.z.clamp(min.z, max.z),
        )
    } else {
        target
    }
}

fn farm_action_system(
    mut npcs: Query<(&mut Navigator, &mut Hunger), With<NpcMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<FarmAction>>,
    hunger_config: Option<Res<HungerConfig>>,
) {
    let restore = hunger_config
        .as_deref()
        .map(|c| c.farm_restore_per_tick)
        .unwrap_or(0.0);

    for (Actor(actor), mut state) in &mut actions {
        let Ok((mut navigator, mut hunger)) = npcs.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                navigator.stop();
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                hunger.replenish(restore);
                if hunger.value >= 0.99 {
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn wander_action_system(
    mut npcs: Query<(&Position, &NpcPatrol, &mut Navigator, &mut WanderState), With<NpcMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<WanderAction>>,
    game_tick: Option<Res<GameTick>>,
    zone_registry: Option<Res<ZoneRegistry>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0).unwrap_or(0);

    for (Actor(actor), mut state) in &mut actions {
        let Ok((position, patrol, mut navigator, mut wander)) = npcs.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                let home = zone_registry
                    .as_deref()
                    .and_then(|r| r.find_zone_by_name(&patrol.home_zone));
                let target = wander_target_for(position.get(), actor.index(), tick, home);
                navigator.set_goal(target, WANDER_SPEED_FACTOR);
                wander.destination = Some(target);
                wander.elapsed_ticks = 0;
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                wander.elapsed_ticks = wander.elapsed_ticks.saturating_add(1);
                let arrived = wander
                    .destination
                    .map(|dest| position.get().distance(dest) <= WANDER_ARRIVAL_DISTANCE)
                    .unwrap_or(true);
                if arrived || wander.elapsed_ticks >= WANDER_MAX_TICKS {
                    navigator.stop();
                    wander.destination = None;
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                navigator.stop();
                wander.destination = None;
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

/// 基于（entity.index, game_tick）的确定性伪随机方向选取。
pub(crate) fn wander_target_for(
    npc_pos: DVec3,
    actor_index: u32,
    game_tick: u32,
    home_zone: Option<&Zone>,
) -> DVec3 {
    let seed = (actor_index as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((game_tick as u64).wrapping_mul(0x94D0_49BB_1331_11EB));
    let angle = ((seed >> 16) % 3600) as f64 / 3600.0 * std::f64::consts::TAU;
    let radius_range = WANDER_MAX_RADIUS - WANDER_MIN_RADIUS;
    let radius = WANDER_MIN_RADIUS + ((seed >> 32) % 1000) as f64 / 1000.0 * radius_range;

    let target = DVec3::new(
        npc_pos.x + angle.cos() * radius,
        npc_pos.y,
        npc_pos.z + angle.sin() * radius,
    );

    if let Some(zone) = home_zone {
        let (min, max) = zone.bounds;
        DVec3::new(
            target.x.clamp(min.x, max.x),
            target.y,
            target.z.clamp(min.z, max.z),
        )
    } else {
        target
    }
}

// ---------------------------------------------------------------------------
// Rogue brains — CultivationDrive / Curiosity scorers + CultivateAction
// ---------------------------------------------------------------------------

/// Zone `spirit_qi ∈ [-1, 1]` → `[0, 1]`（负值代表死域，对修炼无贡献）。
pub(crate) fn zone_qi_normalized(spirit_qi: f64) -> f32 {
    spirit_qi.clamp(0.0, 1.0) as f32
}

pub(crate) fn realm_progress_score(cultivation: &Cultivation, meridians: &MeridianSystem) -> f32 {
    let opened = meridians.opened_count() as f32;
    let needed = match cultivation.realm {
        // 已至 Void：修为顶点，无"下一境进度"可言
        Realm::Void => return 0.0,
        // Spirit → Void 通过 tribulation，但"追求突破"仍以 20 脉为目标
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
    // 门槛与 `cultivate_action_system` 的硬失败条件（CULTIVATE_MIN_ZONE_QI）对齐。
    // 低于门槛时 scorer 返回 0，避免散修在低灵气带反复选 CultivateAction 却每次
    // 立即 Failure —— picker 陷在这一路径上，Curiosity / Wander 永远不接棒。
    if zone_qi < CULTIVATE_MIN_ZONE_QI {
        return 0.0;
    }
    let qi = zone_qi_normalized(zone_qi);
    if qi <= 0.0 {
        return 0.0;
    }
    let progress = realm_progress_score(cultivation, meridians);
    // baseline 0.15：刚起步（progress=0）也有微弱的推进欲望，避免无脉可开时永不坐关。
    (0.15 + 0.85 * progress) * qi
}

fn next_realm(current: Realm) -> Option<Realm> {
    match current {
        Realm::Awaken => Some(Realm::Induce),
        Realm::Induce => Some(Realm::Condense),
        Realm::Condense => Some(Realm::Solidify),
        Realm::Solidify => Some(Realm::Spirit),
        Realm::Spirit => None, // Spirit→Void 走 tribulation，非 breakthrough
        Realm::Void => None,
    }
}

#[allow(clippy::type_complexity)]
fn cultivation_drive_scorer_system(
    mut npcs: Query<
        (
            &Cultivation,
            &MeridianSystem,
            &NpcPatrol,
            Option<&PendingRetirement>,
            Option<&mut CultivationDriveHistory>,
        ),
        With<NpcMarker>,
    >,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut scorers: Query<(&Actor, &mut Score), With<CultivationDriveScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = match npcs.get_mut(*actor) {
            Ok((cultivation, meridians, patrol, pending, history)) => {
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
            }
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

fn curiosity_scorer_system(
    npcs: Query<Option<&PendingRetirement>, With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<CuriosityScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let pending = npcs.get(*actor).ok().flatten().is_some();
        score.set(if pending {
            0.0
        } else {
            CURIOSITY_BASELINE_SCORE
        });
    }
}

/// 挑下一条待开经脉：优先与已开脉相邻，否则任选一条未开（Awaken 首脉特许）。
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

type CultivateNpcQueryItem<'a> = (
    &'a mut Navigator,
    &'a mut Cultivation,
    &'a mut MeridianSystem,
    &'a NpcPatrol,
    &'a mut CultivateState,
    Option<&'a MeridianTarget>,
);

/// 持久化 CultivateAction 的骰子状态：每次 try_breakthrough 会 advance
/// 此 RNG。若每 tick 重置种子，同一 seed 产生的第一个 roll 固定，连续
/// 失败会在几 tick 内把 composure 打到 0，修真路径死锁。
#[derive(Default)]
struct CultivateRngState(Option<u64>);

#[allow(clippy::type_complexity)]
fn cultivate_action_system(
    mut commands: Commands,
    mut npcs: Query<CultivateNpcQueryItem<'_>, With<NpcMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<CultivateAction>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    topology: Option<Res<MeridianTopology>>,
    mut rng_state: valence::prelude::Local<CultivateRngState>,
) {
    let zone_qi_for = |zone_name: &str| -> f64 {
        zone_registry
            .as_deref()
            .and_then(|r| r.find_zone_by_name(zone_name))
            .map(|z| z.spirit_qi)
            .unwrap_or(0.0)
    };

    let mut roll = XorshiftRoll(rng_state.0.unwrap_or(0x51_2f_c8_4d_1a_49_08_0b));

    for (Actor(actor), mut state) in &mut actions {
        let Ok((
            mut navigator,
            mut cultivation,
            mut meridians,
            patrol,
            mut cultivate,
            existing_target,
        )) = npcs.get_mut(*actor)
        else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                navigator.stop();
                cultivate.elapsed_ticks = 0;

                if matches!(cultivation.realm, Realm::Void) {
                    // 已至 Void：由 SeclusionAction 接管，此 action 立即结束。
                    *state = ActionState::Success;
                    continue;
                }

                if zone_qi_for(patrol.home_zone.as_str()) < CULTIVATE_MIN_ZONE_QI {
                    *state = ActionState::Failure;
                    continue;
                }

                if existing_target.is_none() {
                    if let Some(topology) = topology.as_deref() {
                        if let Some(next_m) = pick_next_meridian_to_open(&meridians, topology) {
                            commands.entity(*actor).insert(MeridianTarget(next_m));
                        }
                    }
                }
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                cultivate.elapsed_ticks = cultivate.elapsed_ticks.saturating_add(1);

                // 若当前 target 已打通，挑下一条；若没有目标，也尝试补一条。
                let need_retarget = existing_target
                    .map(|t| meridians.get(t.0).opened)
                    .unwrap_or(true);
                if need_retarget {
                    if let Some(topology) = topology.as_deref() {
                        match pick_next_meridian_to_open(&meridians, topology) {
                            Some(next_m) => {
                                commands.entity(*actor).insert(MeridianTarget(next_m));
                            }
                            None => {
                                // 所有脉已开：尝试突破（若 qi 够）。
                            }
                        }
                    }
                }

                // 看是否能 inline 突破：下一境界所需经脉数已达、qi 够。
                if let Some(next) = next_realm(cultivation.realm) {
                    let have = meridians.opened_count();
                    let need = next.required_meridians();
                    let qi_need = breakthrough_qi_cost(next);
                    if have >= need && cultivation.qi_current >= qi_need {
                        match try_breakthrough(
                            &mut cultivation,
                            &mut meridians,
                            ROGUE_BREAKTHROUGH_MATERIAL_BONUS,
                            &mut roll,
                        ) {
                            Ok(BreakthroughSuccess { to, .. }) => {
                                tracing::info!(
                                    "[bong][npc] rogue breakthrough actor={:?} to={:?}",
                                    actor,
                                    to
                                );
                                commands.entity(*actor).remove::<MeridianTarget>();
                                *state = ActionState::Success;
                                continue;
                            }
                            Err(BreakthroughError::RolledFailure { .. }) => {
                                // 失败：本次 action 结束，外层 scorer 会再次驱动（如 qi 已被扣）。
                                *state = ActionState::Failure;
                                continue;
                            }
                            Err(_) => {
                                // 其它 error（qi 不够 / 经脉不够）— 继续等待。
                            }
                        }
                    }
                }

                if cultivate.elapsed_ticks >= CULTIVATE_MAX_TICKS {
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                navigator.stop();
                commands.entity(*actor).remove::<MeridianTarget>();
                cultivate.elapsed_ticks = 0;
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }

    rng_state.0 = Some(roll.0);
}

// ---------------------------------------------------------------------------
// Tribulation (Spirit→Void) — TribulationReadyScorer + StartDuXuAction
// Seclusion (post-Void) — SeclusionScorer + SeclusionAction
// ---------------------------------------------------------------------------

/// 纯函数：根据 4 项前置条件判断是否可起劫。
///
/// - `realm == Spirit`
/// - `opened_count >= 20`
/// - `qi_current >= qi_max × TRIBULATION_MIN_QI_RATIO`
/// - `drive_history >= TRIBULATION_READY_SUSTAIN_TICKS`
///
/// 敌意判定由 scorer system 单独做。
pub(crate) fn tribulation_prereqs_met(
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
    history: &CultivationDriveHistory,
) -> bool {
    if !matches!(cultivation.realm, Realm::Spirit) {
        return false;
    }
    if meridians.opened_count() < Realm::Void.required_meridians() {
        return false;
    }
    if cultivation.qi_current < cultivation.qi_max * TRIBULATION_MIN_QI_RATIO {
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
fn tribulation_ready_scorer_system(
    npcs: Query<
        (
            &Position,
            &Cultivation,
            &MeridianSystem,
            &CultivationDriveHistory,
            Option<&PendingRetirement>,
            Option<&TribulationState>,
        ),
        With<NpcMarker>,
    >,
    players: Query<&Position, With<ClientMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<TribulationReadyScorer>>,
) {
    let player_positions: Vec<DVec3> = players.iter().map(|p| p.get()).collect();

    for (Actor(actor), mut score) in &mut scorers {
        let value = match npcs.get(*actor) {
            Ok((position, cultivation, meridians, history, pending, in_tribulation)) => {
                if pending.is_some()
                    || in_tribulation.is_some()
                    || !tribulation_prereqs_met(cultivation, meridians, history)
                {
                    0.0
                } else {
                    let nearest =
                        nearest_hostile_distance(position.get(), player_positions.iter().copied());
                    match nearest {
                        Some(dist) if dist <= TRIBULATION_HOSTILE_RADIUS => 0.0,
                        _ => 1.0,
                    }
                }
            }
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

fn seclusion_scorer_system(
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

#[allow(clippy::type_complexity)]
fn start_duxu_action_system(
    mut commands: Commands,
    mut quota: Option<ResMut<AscensionQuotaStore>>,
    mut initiate: EventWriter<InitiateXuhuaTribulation>,
    clock: Option<Res<crate::cultivation::tick::CultivationClock>>,
    npcs: Query<(&Cultivation, Option<&TribulationState>), With<NpcMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<StartDuXuAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((cultivation, in_tribulation)) = npcs.get(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                if in_tribulation.is_some() {
                    // 已经在渡劫中（可能来自 agent 命令），直接进入观察态。
                    *state = ActionState::Executing;
                    continue;
                }
                if !matches!(cultivation.realm, Realm::Spirit) {
                    *state = ActionState::Failure;
                    continue;
                }
                let reserved = quota
                    .as_deref_mut()
                    .map(|q| q.try_reserve(*actor))
                    .unwrap_or(false);
                if !reserved {
                    tracing::info!(
                        "[bong][npc] tribulation slot exhausted or store missing, actor={:?}",
                        actor
                    );
                    *state = ActionState::Failure;
                    continue;
                }

                let started_tick = clock.as_deref().map(|c| c.tick).unwrap_or(0);
                initiate.send(InitiateXuhuaTribulation {
                    entity: *actor,
                    waves_total: NPC_TRIBULATION_WAVES_DEFAULT,
                    started_tick,
                });
                commands
                    .entity(*actor)
                    .insert(NpcTribulationPacing::default());
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if matches!(cultivation.realm, Realm::Void) {
                    // 成功：realm 已升顶，quota 会由 tribulation::release_quota_for_ended_tribulations 释放。
                    commands.entity(*actor).remove::<NpcTribulationPacing>();
                    *state = ActionState::Success;
                } else if in_tribulation.is_none() {
                    // 失败：TribulationState 被 tribulation_failure_system 移除但 realm 未升 → 渡劫失败。
                    if let Some(q) = quota.as_deref_mut() {
                        q.release(*actor);
                    }
                    commands.entity(*actor).remove::<NpcTribulationPacing>();
                    *state = ActionState::Failure;
                }
            }
            ActionState::Cancelled => {
                if let Some(q) = quota.as_deref_mut() {
                    q.release(*actor);
                }
                commands.entity(*actor).remove::<NpcTribulationPacing>();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Component)]
pub struct SeclusionState {
    pub elapsed_ticks: u32,
}

fn seclusion_action_system(
    mut commands: Commands,
    mut npcs: Query<(&mut Navigator, Option<&mut SeclusionState>), With<NpcMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<SeclusionAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((mut navigator, existing)) = npcs.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                navigator.stop();
                if existing.is_none() {
                    commands.entity(*actor).insert(SeclusionState::default());
                }
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let elapsed = match existing {
                    Some(mut s) => {
                        s.elapsed_ticks = s.elapsed_ticks.saturating_add(1);
                        s.elapsed_ticks
                    }
                    None => 0,
                };
                if elapsed >= SECLUSION_CYCLE_TICKS {
                    commands.entity(*actor).remove::<SeclusionState>();
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                commands.entity(*actor).remove::<SeclusionState>();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::events::AttackIntent;
    use crate::npc::hunger::HungerConfig;
    use crate::npc::movement::{MovementCapabilities, MovementController, MovementCooldowns};
    use crate::npc::navigator::Navigator;
    use crate::npc::patrol::NpcPatrol;
    use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;
    use bevy_transform::components::Transform;
    use big_brain::prelude::{FirstToScore, Thinker};
    use valence::prelude::{App, EventReader, IntoSystemConfigs, Position, Update};

    #[derive(Default)]
    struct CapturedAttackIntents(Vec<AttackIntent>);

    impl valence::prelude::Resource for CapturedAttackIntents {}

    fn capture_attack_intents(
        mut events: EventReader<AttackIntent>,
        mut captured: valence::prelude::ResMut<CapturedAttackIntents>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    #[test]
    fn player_proximity_scorer_thresholds() {
        let score_at_just_inside_threshold_distance = proximity_score(3.2);
        let score_at_exact_threshold_distance = proximity_score(3.2);
        let score_just_outside_threshold_distance = proximity_score(3.3);
        let score_out_of_range = proximity_score(8.0);

        assert!(
            should_flee_from_score(score_at_just_inside_threshold_distance),
            "3.2 blocks should meet threshold"
        );
        assert!(
            should_flee_from_score(score_at_exact_threshold_distance),
            "exact threshold score should trigger flee"
        );
        assert!(
            !should_flee_from_score(score_just_outside_threshold_distance),
            "3.3 blocks should fall under threshold"
        );
        assert_eq!(score_out_of_range, 0.0, "8+ blocks should score 0");

        let thinker = Thinker::build()
            .picker(FirstToScore {
                threshold: PROXIMITY_THRESHOLD,
            })
            .when(PlayerProximityScorer, FleeAction);
        let mut app = App::new();
        app.world_mut().spawn(thinker);
        assert_eq!(PROXIMITY_THRESHOLD, 0.6);
        assert!((proximity_score(3.2) - 0.6).abs() < 1e-6);
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

    #[test]
    fn flee_target_computation_moves_away_from_player() {
        let npc = DVec3::new(10.0, 67.0, 10.0);
        let player = DVec3::new(15.0, 67.0, 10.0);
        let target = compute_flee_target(npc, player, None, DEFAULT_SPAWN_ZONE_NAME);

        // Should flee in -X direction.
        assert!(target.x < npc.x, "flee target should be away from player");
    }

    #[test]
    fn flee_target_same_position_uses_fallback() {
        let pos = DVec3::new(10.0, 67.0, 10.0);
        let target = compute_flee_target(pos, pos, None, DEFAULT_SPAWN_ZONE_NAME);

        // Fallback direction is +X.
        assert!(target.x > pos.x);
    }

    #[test]
    fn chase_score_within_range() {
        let profile = NpcMeleeProfile::fist();
        assert!(chase_score(10.0, &profile) > 0.0);
        assert!(chase_score(32.0, &profile) > -f32::EPSILON);
        assert_eq!(chase_score(33.0, &profile), 0.0);
        assert_eq!(chase_score(f32::INFINITY, &profile), 0.0);
        assert_eq!(chase_score(0.8, &profile), 0.0);
    }

    #[test]
    fn flee_action_completes_above_sixteen_blocks() {
        let mut app = App::new();
        app.add_systems(PreUpdate, flee_action_system.in_set(BigBrainSet::Actions));

        let player = app
            .world_mut()
            .spawn((ClientMarker, Position::new([0.0, 66.0, 0.0])))
            .id();

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                EntityKind::ZOMBIE,
                Position::new([30.0, 66.0, 0.0]),
                Transform::from_xyz(30.0, 66.0, 0.0),
                NpcBlackboard {
                    nearest_player: Some(player),
                    player_distance: 30.0,
                    target_position: Some(DVec3::new(0.0, 66.0, 0.0)),
                    ..Default::default()
                },
                NpcMeleeProfile::default(),
                NpcPatrol::new(DEFAULT_SPAWN_ZONE_NAME, DVec3::new(30.0, 66.0, 0.0)),
                Navigator::new(),
            ))
            .id();

        let action_entity = app
            .world_mut()
            .spawn((Actor(npc), FleeAction, ActionState::Requested))
            .id();

        app.update();
        app.update();

        let action_state = app
            .world()
            .get::<ActionState>(action_entity)
            .expect("flee action entity should still exist");
        assert_eq!(*action_state, ActionState::Success);
    }

    #[test]
    fn bridge_less_no_player_behavior() {
        let mut app = App::new();
        app.add_systems(PreUpdate, update_npc_blackboard);

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([14.0, 66.0, 14.0]),
                NpcBlackboard::default(),
            ))
            .id();

        app.update();

        let blackboard = app
            .world()
            .get::<NpcBlackboard>(npc)
            .expect("NPC blackboard should exist");

        assert!(
            blackboard.nearest_player.is_none(),
            "without players, nearest_player must remain None"
        );
        assert!(
            blackboard.player_distance.is_infinite(),
            "without players, distance must remain infinity"
        );
    }

    #[test]
    fn melee_attack_action_bridges_to_attack_intent_without_knockback_side_path() {
        let mut app = App::new();
        app.insert_resource(GameTick(120));
        app.insert_resource(CapturedAttackIntents::default());
        app.add_event::<AttackIntent>();
        app.add_systems(
            PreUpdate,
            (
                melee_attack_action_system,
                capture_attack_intents.after(melee_attack_action_system),
            ),
        );

        let target = app
            .world_mut()
            .spawn((ClientMarker, Position::new([12.0, 66.0, 10.0])))
            .id();
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([10.0, 66.0, 10.0]),
                NpcBlackboard {
                    nearest_player: Some(target),
                    player_distance: 2.0,
                    target_position: Some(DVec3::new(12.0, 66.0, 10.0)),
                    ..Default::default()
                },
                NpcMeleeProfile::spear(),
                Navigator::new(),
            ))
            .id();
        let action_entity = app
            .world_mut()
            .spawn((Actor(npc), MeleeAttackAction, ActionState::Requested))
            .id();

        app.update();
        app.update();

        let action_state = app
            .world()
            .get::<ActionState>(action_entity)
            .expect("melee action entity should still exist");
        assert_eq!(*action_state, ActionState::Executing);

        let captured = &app.world().resource::<CapturedAttackIntents>().0;
        assert_eq!(
            captured.len(),
            1,
            "melee cooldown should emit one AttackIntent"
        );
        assert_eq!(captured[0].attacker, npc);
        assert_eq!(captured[0].target, Some(target));
        assert_eq!(captured[0].reach, NpcMeleeProfile::spear().reach);
        assert_eq!(captured[0].qi_invest, 10.0);
        assert_eq!(captured[0].wound_kind, NpcMeleeProfile::spear().wound_kind);
        assert_eq!(captured[0].debug_command, None);

        assert!(
            app.world()
                .get::<crate::npc::movement::PendingKnockback>(target)
                .is_none(),
            "melee bridge should not rely on PendingKnockback as primary damage path"
        );
    }

    #[test]
    fn melee_attack_action_same_tick_does_not_emit_duplicate_attack_intents() {
        let mut app = App::new();
        app.insert_resource(GameTick(240));
        app.insert_resource(CapturedAttackIntents::default());
        app.add_event::<AttackIntent>();
        app.add_systems(
            PreUpdate,
            (
                melee_attack_action_system,
                capture_attack_intents.after(melee_attack_action_system),
            ),
        );

        let target = app
            .world_mut()
            .spawn((ClientMarker, Position::new([12.0, 66.0, 10.0])))
            .id();
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([10.0, 66.0, 10.0]),
                NpcBlackboard {
                    nearest_player: Some(target),
                    player_distance: 2.0,
                    target_position: Some(DVec3::new(12.0, 66.0, 10.0)),
                    ..Default::default()
                },
                NpcMeleeProfile::spear(),
                Navigator::new(),
            ))
            .id();
        app.world_mut()
            .spawn((Actor(npc), MeleeAttackAction, ActionState::Requested));

        app.update();
        app.update();
        app.update();

        let captured = &app.world().resource::<CapturedAttackIntents>().0;
        assert_eq!(
            captured.len(),
            1,
            "same GameTick should not produce duplicate melee AttackIntent"
        );
    }

    #[test]
    fn melee_range_scorer_respects_profile_reach_max() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            melee_range_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let short_npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    player_distance: 1.6,
                    ..Default::default()
                },
                NpcMeleeProfile::fist(),
            ))
            .id();
        let long_npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    player_distance: 1.6,
                    ..Default::default()
                },
                NpcMeleeProfile::spear(),
            ))
            .id();

        let short_scorer = app
            .world_mut()
            .spawn((Actor(short_npc), Score::default(), MeleeRangeScorer))
            .id();
        let long_scorer = app
            .world_mut()
            .spawn((Actor(long_npc), Score::default(), MeleeRangeScorer))
            .id();

        app.update();

        assert_eq!(app.world().get::<Score>(short_scorer).unwrap().get(), 0.0);
        assert_eq!(app.world().get::<Score>(long_scorer).unwrap().get(), 1.0);
    }

    #[test]
    fn melee_action_waits_inside_disengage_band_without_swinging() {
        let mut app = App::new();
        app.insert_resource(GameTick(240));
        app.insert_resource(CapturedAttackIntents::default());
        app.add_event::<AttackIntent>();
        app.add_systems(
            PreUpdate,
            (
                melee_attack_action_system,
                capture_attack_intents.after(melee_attack_action_system),
            ),
        );

        let target = app
            .world_mut()
            .spawn((ClientMarker, Position::new([13.2, 66.0, 10.0])))
            .id();
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([10.0, 66.0, 10.0]),
                NpcBlackboard {
                    nearest_player: Some(target),
                    player_distance: 3.2,
                    target_position: Some(DVec3::new(13.2, 66.0, 10.0)),
                    ..Default::default()
                },
                NpcMeleeProfile::spear(),
                Navigator::new(),
            ))
            .id();
        let action_entity = app
            .world_mut()
            .spawn((Actor(npc), MeleeAttackAction, ActionState::Requested))
            .id();

        app.update();
        app.update();

        let action_state = app.world().get::<ActionState>(action_entity).unwrap();
        let captured = &app.world().resource::<CapturedAttackIntents>().0;

        assert_eq!(*action_state, ActionState::Executing);
        assert!(
            captured.is_empty(),
            "npc should hold range instead of swinging outside reach"
        );
    }

    #[test]
    fn dash_score_zero_without_capability() {
        let bb = NpcBlackboard {
            player_distance: 8.0,
            ..Default::default()
        };
        let caps = MovementCapabilities {
            can_sprint: true,
            can_dash: false,
        };
        let cd = MovementCooldowns::default();
        let ctrl = MovementController::new();

        assert_eq!(dash_score(&bb, &caps, &cd, &ctrl, 0), 0.0);
    }

    #[test]
    fn dash_score_positive_in_range_with_capability() {
        let bb = NpcBlackboard {
            player_distance: 8.0, // within DASH_MIN..DASH_MAX
            ..Default::default()
        };
        let caps = MovementCapabilities {
            can_sprint: true,
            can_dash: true,
        };
        let cd = MovementCooldowns::default();
        let ctrl = MovementController::new();

        assert!(dash_score(&bb, &caps, &cd, &ctrl, 0) > 0.0);
    }

    #[test]
    fn dash_score_zero_on_cooldown() {
        let bb = NpcBlackboard {
            player_distance: 8.0,
            ..Default::default()
        };
        let caps = MovementCapabilities {
            can_sprint: true,
            can_dash: true,
        };
        let cd = MovementCooldowns {
            sprint_ready_at: 0,
            dash_ready_at: 100, // cooldown active
        };
        let ctrl = MovementController::new();

        assert_eq!(dash_score(&bb, &caps, &cd, &ctrl, 50), 0.0);
    }

    #[test]
    fn dash_score_zero_outside_range() {
        let bb_too_close = NpcBlackboard {
            player_distance: 3.0, // < DASH_MIN_DISTANCE
            ..Default::default()
        };
        let bb_too_far = NpcBlackboard {
            player_distance: 20.0, // > DASH_MAX_DISTANCE
            ..Default::default()
        };
        let caps = MovementCapabilities {
            can_sprint: true,
            can_dash: true,
        };
        let cd = MovementCooldowns::default();
        let ctrl = MovementController::new();

        assert_eq!(dash_score(&bb_too_close, &caps, &cd, &ctrl, 0), 0.0);
        assert_eq!(dash_score(&bb_too_far, &caps, &cd, &ctrl, 0), 0.0);
    }

    // -----------------------------------------------------------------------
    // Commoner brain tests
    // -----------------------------------------------------------------------

    #[test]
    fn fear_score_is_zero_for_awaken_realm_regardless_of_distance() {
        assert_eq!(fear_cultivator_score(0.0, Realm::Awaken), 0.0);
        assert_eq!(fear_cultivator_score(5.0, Realm::Awaken), 0.0);
        assert_eq!(fear_cultivator_score(49.0, Realm::Awaken), 0.0);
    }

    #[test]
    fn fear_score_scales_with_realm_and_proximity() {
        let near_void = fear_cultivator_score(1.0, Realm::Void);
        let far_void = fear_cultivator_score(45.0, Realm::Void);
        let near_induce = fear_cultivator_score(1.0, Realm::Induce);

        assert!(
            near_void > far_void,
            "higher proximity → higher fear, got near={near_void} far={far_void}"
        );
        assert!(
            near_void > near_induce,
            "higher realm at same distance → higher fear"
        );
        assert_eq!(
            fear_cultivator_score(FEAR_CULTIVATOR_RANGE, Realm::Void),
            0.0,
            "fear at or beyond range must drop to 0"
        );
    }

    #[test]
    fn wander_target_stays_inside_home_zone_bounds() {
        let zone = Zone {
            name: "test".to_string(),
            bounds: (DVec3::new(0.0, 60.0, 0.0), DVec3::new(20.0, 80.0, 20.0)),
            spirit_qi: 0.0,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        };
        for tick in 0..32u32 {
            for idx in 0..8u32 {
                let t = wander_target_for(DVec3::new(10.0, 66.0, 10.0), idx, tick, Some(&zone));
                assert!(t.x >= 0.0 && t.x <= 20.0);
                assert!(t.z >= 0.0 && t.z <= 20.0);
            }
        }
    }

    #[test]
    fn wander_target_differs_across_entities_or_ticks() {
        let a = wander_target_for(DVec3::new(10.0, 66.0, 10.0), 1, 0, None);
        let b = wander_target_for(DVec3::new(10.0, 66.0, 10.0), 2, 0, None);
        let c = wander_target_for(DVec3::new(10.0, 66.0, 10.0), 1, 200, None);
        assert_ne!(a, b, "different entities should pick different wander dirs");
        assert_ne!(a, c, "same entity across ticks should re-roll");
    }

    #[test]
    fn fear_cultivator_scorer_reads_realm_from_nearest_player() {
        use crate::cultivation::components::{Cultivation, Realm};

        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            fear_cultivator_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let player = app
            .world_mut()
            .spawn((
                ClientMarker,
                Position::new([0.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Solidify,
                    ..Cultivation::default()
                },
            ))
            .id();

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    nearest_player: Some(player),
                    player_distance: 10.0,
                    target_position: Some(DVec3::new(0.0, 66.0, 0.0)),
                    ..Default::default()
                },
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), FearCultivatorScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            score > 0.4,
            "solidify-realm player at 10 blocks should score above 0.4, got {score}"
        );
    }

    #[test]
    fn hunger_scorer_is_inverse_of_hunger_value() {
        let mut app = App::new();
        app.add_systems(PreUpdate, hunger_scorer_system.in_set(BigBrainSet::Scorers));

        let npc = app.world_mut().spawn((NpcMarker, Hunger::new(0.25))).id();

        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), HungerScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert!((score - 0.75).abs() < 1e-5);
    }

    #[test]
    fn wander_scorer_is_zero_when_pending_retirement() {
        let mut app = App::new();
        app.add_systems(PreUpdate, wander_scorer_system.in_set(BigBrainSet::Scorers));

        let npc = app.world_mut().spawn((NpcMarker, PendingRetirement)).id();

        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), WanderScorer))
            .id();

        app.update();

        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    #[test]
    fn farm_action_replenishes_hunger_until_success() {
        let mut app = App::new();
        app.insert_resource(HungerConfig {
            decay_per_tick: 0.0,
            farm_restore_per_tick: 0.4,
        });
        app.add_systems(PreUpdate, farm_action_system.in_set(BigBrainSet::Actions));

        let npc = app
            .world_mut()
            .spawn((NpcMarker, Hunger::new(0.1), Navigator::new()))
            .id();

        let action = app
            .world_mut()
            .spawn((Actor(npc), FarmAction, ActionState::Requested))
            .id();

        for _ in 0..6 {
            app.update();
        }

        let hunger = *app.world().get::<Hunger>(npc).unwrap();
        assert!(
            hunger.value >= 0.99,
            "hunger should replenish, got {}",
            hunger.value
        );
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success
        );
    }

    #[test]
    fn flee_cultivator_action_completes_above_forty_blocks() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            flee_cultivator_action_system.in_set(BigBrainSet::Actions),
        );

        let player = app
            .world_mut()
            .spawn((ClientMarker, Position::new([0.0, 66.0, 0.0])))
            .id();

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                EntityKind::VILLAGER,
                Position::new([60.0, 66.0, 0.0]),
                NpcBlackboard {
                    nearest_player: Some(player),
                    player_distance: 60.0,
                    target_position: Some(DVec3::new(0.0, 66.0, 0.0)),
                    ..Default::default()
                },
                NpcPatrol::new(DEFAULT_SPAWN_ZONE_NAME, DVec3::new(60.0, 66.0, 0.0)),
                Navigator::new(),
            ))
            .id();

        let action_entity = app
            .world_mut()
            .spawn((Actor(npc), FleeCultivatorAction, ActionState::Requested))
            .id();

        app.update();
        app.update();

        assert_eq!(
            *app.world().get::<ActionState>(action_entity).unwrap(),
            ActionState::Success
        );
    }

    #[test]
    fn wander_action_requested_sets_navigator_goal_then_succeeds_on_max_ticks() {
        let mut app = App::new();
        app.insert_resource(GameTick(1234));
        app.add_systems(PreUpdate, wander_action_system.in_set(BigBrainSet::Actions));

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([10.0, 66.0, 10.0]),
                NpcPatrol::new(DEFAULT_SPAWN_ZONE_NAME, DVec3::new(10.0, 66.0, 10.0)),
                Navigator::new(),
                WanderState::default(),
            ))
            .id();

        let action = app
            .world_mut()
            .spawn((Actor(npc), WanderAction, ActionState::Requested))
            .id();

        app.update();

        let wander_state = *app.world().get::<WanderState>(npc).unwrap();
        assert!(
            wander_state.destination.is_some(),
            "first tick should set a destination"
        );
        assert!(!app.world().get::<Navigator>(npc).unwrap().is_idle());

        // Force timeout: bump elapsed_ticks close to cap, then tick once.
        {
            let mut state = app.world_mut().get_mut::<WanderState>(npc).unwrap();
            state.elapsed_ticks = WANDER_MAX_TICKS - 1;
        }
        app.update();

        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success
        );
        assert!(app.world().get::<Navigator>(npc).unwrap().is_idle());
    }

    // -----------------------------------------------------------------------
    // Rogue brain tests
    // -----------------------------------------------------------------------

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
        // P1: Codex review — scorer 门槛必须和 cultivate_action_system 的
        // CULTIVATE_MIN_ZONE_QI 硬失败条件对齐，否则散修在 0.1–0.29 区间
        // 会反复选 CultivateAction 立刻失败，picker 卡死。
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
        // 取 Condense realm（下一境 Solidify 需 8 经脉）观察非饱和进度差异。
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
            "more opened meridians → higher drive at same zone_qi"
        );
        assert!(
            cultivation_drive_score(&c, &m_low, 0.9) > cultivation_drive_score(&c, &m_low, 0.3),
            "higher zone_qi → higher drive at same progress"
        );
    }

    #[test]
    fn pick_next_meridian_prefers_adjacent_to_opened() {
        let topology = MeridianTopology::standard();
        let mut system = MeridianSystem::default();
        // open one meridian
        system.regular[0].opened = true;
        let first_opened = MeridianId::REGULAR[0];
        let picked = pick_next_meridian_to_open(&system, &topology)
            .expect("there should be an adjacent candidate");
        assert!(
            topology.neighbors(first_opened).contains(&picked),
            "picked {:?} should be adjacent to {:?}",
            picked,
            first_opened
        );
    }

    #[test]
    fn pick_next_meridian_returns_none_when_all_open() {
        let topology = MeridianTopology::standard();
        let mut system = MeridianSystem::default();
        for m in system.regular.iter_mut() {
            m.opened = true;
        }
        for m in system.extraordinary.iter_mut() {
            m.opened = true;
        }
        assert_eq!(pick_next_meridian_to_open(&system, &topology), None);
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
    fn cultivate_action_fails_when_zone_qi_too_low() {
        use crate::world::zone::ZoneRegistry;

        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].name = DEFAULT_SPAWN_ZONE_NAME.to_string();
        zones.zones[0].spirit_qi = 0.05;
        app.insert_resource(zones);
        app.insert_resource(MeridianTopology::standard());
        app.add_systems(
            PreUpdate,
            cultivate_action_system.in_set(BigBrainSet::Actions),
        );

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Cultivation::default(),
                MeridianSystem::default(),
                NpcPatrol::new(DEFAULT_SPAWN_ZONE_NAME, DVec3::new(0.0, 66.0, 0.0)),
                Navigator::new(),
                CultivateState::default(),
            ))
            .id();
        let action = app
            .world_mut()
            .spawn((Actor(npc), CultivateAction, ActionState::Requested))
            .id();

        app.update();

        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Failure
        );
    }

    #[test]
    fn cultivate_action_inserts_meridian_target_and_triggers_breakthrough() {
        use crate::world::zone::ZoneRegistry;

        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].name = DEFAULT_SPAWN_ZONE_NAME.to_string();
        zones.zones[0].spirit_qi = 0.9;
        app.insert_resource(zones);
        app.insert_resource(MeridianTopology::standard());
        app.add_systems(
            PreUpdate,
            cultivate_action_system.in_set(BigBrainSet::Actions),
        );

        // 预开 1 条经脉（>= required_meridians(Induce)=1）并注满 qi。
        let mut meridians = MeridianSystem::default();
        meridians.regular[0].opened = true;
        let cultivation = Cultivation {
            realm: Realm::Awaken,
            qi_current: 50.0,
            qi_max: 50.0,
            ..Cultivation::default()
        };

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                cultivation,
                meridians,
                NpcPatrol::new(DEFAULT_SPAWN_ZONE_NAME, DVec3::new(0.0, 66.0, 0.0)),
                Navigator::new(),
                CultivateState::default(),
            ))
            .id();

        let action = app
            .world_mut()
            .spawn((Actor(npc), CultivateAction, ActionState::Requested))
            .id();

        // Tick 1: Requested → Executing + MeridianTarget 插入。
        app.update();
        assert!(app.world().get::<MeridianTarget>(npc).is_some());
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Executing
        );

        // Tick 2: Executing → try_breakthrough 成功到 Induce（确定性 XorshiftRoll seed 已命中）。
        app.update();

        let realm = app.world().get::<Cultivation>(npc).unwrap().realm;
        let state_now = app.world().get::<ActionState>(action).unwrap().clone();
        // 允许：Success 且升到 Induce；或者骰子翻车导致 Failure（罕见，和种子有关）。
        assert!(
            matches!(state_now, ActionState::Success | ActionState::Failure),
            "should terminate after breakthrough attempt, got {state_now:?}"
        );
        if matches!(state_now, ActionState::Success) {
            assert_eq!(realm, Realm::Induce);
            assert!(app.world().get::<MeridianTarget>(npc).is_none());
        }
    }

    /// NPC 自驱修炼 E2E：高 qi 区 + qi 注满 + MeridianTarget → meridian_open_tick
    /// 应推进 open_progress 直到首脉 `opened = true`。
    #[test]
    fn meridian_open_tick_advances_npc_without_life_record() {
        use crate::cultivation::meridian_open::{meridian_open_tick, MeridianTarget};
        use crate::cultivation::tick::CultivationClock;
        use crate::world::zone::ZoneRegistry;

        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].name = DEFAULT_SPAWN_ZONE_NAME.to_string();
        zones.zones[0].spirit_qi = 0.9;
        app.insert_resource(zones);
        app.insert_resource(MeridianTopology::standard());
        app.insert_resource(CultivationClock::default());
        app.add_systems(valence::prelude::Update, meridian_open_tick);

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 66.0, 0.0]),
                Cultivation {
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Cultivation::default()
                },
                MeridianSystem::default(),
                MeridianTarget(MeridianId::Lung),
            ))
            .id();

        assert!(!app.world().get::<MeridianSystem>(npc).unwrap().regular[0].opened);

        // Open rate ~0.01 per tick in high-qi zone — 300 ticks should be plenty.
        for _ in 0..300 {
            app.update();
        }

        let ms = app.world().get::<MeridianSystem>(npc).unwrap();
        assert!(
            ms.opened_count() >= 1,
            "NPC Lung meridian should open after sustained cultivation (opened_count={})",
            ms.opened_count()
        );
    }

    #[test]
    fn cultivate_action_success_at_void_realm() {
        use crate::world::zone::ZoneRegistry;

        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].name = DEFAULT_SPAWN_ZONE_NAME.to_string();
        zones.zones[0].spirit_qi = 0.9;
        app.insert_resource(zones);
        app.insert_resource(MeridianTopology::standard());
        app.add_systems(
            PreUpdate,
            cultivate_action_system.in_set(BigBrainSet::Actions),
        );

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Cultivation {
                    realm: Realm::Void,
                    ..Cultivation::default()
                },
                MeridianSystem::default(),
                NpcPatrol::new(DEFAULT_SPAWN_ZONE_NAME, DVec3::new(0.0, 66.0, 0.0)),
                Navigator::new(),
                CultivateState::default(),
            ))
            .id();

        let action = app
            .world_mut()
            .spawn((Actor(npc), CultivateAction, ActionState::Requested))
            .id();

        app.update();

        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success
        );
    }

    #[test]
    fn cultivation_drive_scorer_reads_home_zone_qi() {
        use crate::world::zone::ZoneRegistry;

        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].name = DEFAULT_SPAWN_ZONE_NAME.to_string();
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
                NpcPatrol::new(DEFAULT_SPAWN_ZONE_NAME, DVec3::new(0.0, 66.0, 0.0)),
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

    // -----------------------------------------------------------------------
    // Phase 3 — Tribulation + Seclusion tests
    // -----------------------------------------------------------------------

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
    fn tribulation_prereqs_reject_non_spirit_realm() {
        let c = Cultivation::default(); // Awaken
        let m = all_meridians_open();
        let h = CultivationDriveHistory {
            above_threshold_ticks: TRIBULATION_READY_SUSTAIN_TICKS,
        };
        assert!(!tribulation_prereqs_met(&c, &m, &h));
    }

    #[test]
    fn tribulation_prereqs_reject_not_enough_meridians() {
        let c = spirit_cultivation_at_qi(0.9);
        let m = MeridianSystem::default(); // none open
        let h = CultivationDriveHistory {
            above_threshold_ticks: TRIBULATION_READY_SUSTAIN_TICKS,
        };
        assert!(!tribulation_prereqs_met(&c, &m, &h));
    }

    #[test]
    fn tribulation_prereqs_reject_low_qi() {
        let c = spirit_cultivation_at_qi(0.3);
        let m = all_meridians_open();
        let h = CultivationDriveHistory {
            above_threshold_ticks: TRIBULATION_READY_SUSTAIN_TICKS,
        };
        assert!(!tribulation_prereqs_met(&c, &m, &h));
    }

    #[test]
    fn tribulation_prereqs_reject_not_sustained() {
        let c = spirit_cultivation_at_qi(0.9);
        let m = all_meridians_open();
        let h = CultivationDriveHistory {
            above_threshold_ticks: 0,
        };
        assert!(!tribulation_prereqs_met(&c, &m, &h));
    }

    #[test]
    fn tribulation_prereqs_pass_when_all_conditions_met() {
        let c = spirit_cultivation_at_qi(0.9);
        let m = all_meridians_open();
        let h = CultivationDriveHistory {
            above_threshold_ticks: TRIBULATION_READY_SUSTAIN_TICKS,
        };
        assert!(tribulation_prereqs_met(&c, &m, &h));
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

        // Player far away, outside 100-block radius.
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
    fn start_duxu_action_reserves_quota_and_fires_initiate() {
        let mut app = App::new();
        app.insert_resource(AscensionQuotaStore::default());
        app.add_event::<InitiateXuhuaTribulation>();
        app.add_systems(
            PreUpdate,
            start_duxu_action_system.in_set(BigBrainSet::Actions),
        );

        let npc = app
            .world_mut()
            .spawn((NpcMarker, spirit_cultivation_at_qi(0.9)))
            .id();

        let action = app
            .world_mut()
            .spawn((Actor(npc), StartDuXuAction, ActionState::Requested))
            .id();

        app.update();

        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Executing
        );
        assert_eq!(
            app.world().resource::<AscensionQuotaStore>().active_count(),
            1
        );

        let events = app
            .world()
            .resource::<bevy_ecs::event::Events<InitiateXuhuaTribulation>>();
        let all: Vec<_> = events.iter_current_update_events().cloned().collect();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].entity, npc);
        assert_eq!(all[0].waves_total, NPC_TRIBULATION_WAVES_DEFAULT);
    }

    #[test]
    fn start_duxu_action_fails_when_quota_exhausted() {
        let mut app = App::new();
        app.insert_resource(AscensionQuotaStore {
            max_concurrent: 0,
            ..AscensionQuotaStore::default()
        });
        app.add_event::<InitiateXuhuaTribulation>();
        app.add_systems(
            PreUpdate,
            start_duxu_action_system.in_set(BigBrainSet::Actions),
        );

        let npc = app
            .world_mut()
            .spawn((NpcMarker, spirit_cultivation_at_qi(0.9)))
            .id();

        let action = app
            .world_mut()
            .spawn((Actor(npc), StartDuXuAction, ActionState::Requested))
            .id();

        app.update();

        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Failure
        );
    }

    #[test]
    fn start_duxu_action_success_when_realm_advances_to_void() {
        let mut app = App::new();
        app.insert_resource(AscensionQuotaStore::default());
        app.add_event::<InitiateXuhuaTribulation>();
        app.add_systems(
            PreUpdate,
            start_duxu_action_system.in_set(BigBrainSet::Actions),
        );

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Cultivation {
                    realm: Realm::Void, // 已升顶（模拟 wave 都清完）
                    ..Cultivation::default()
                },
            ))
            .id();

        let action = app
            .world_mut()
            .spawn((Actor(npc), StartDuXuAction, ActionState::Executing))
            .id();

        app.update();

        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success
        );
    }

    #[test]
    fn seclusion_action_stops_navigator_and_succeeds_on_timer() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            seclusion_action_system.in_set(BigBrainSet::Actions),
        );

        let mut nav = Navigator::new();
        nav.set_goal(DVec3::new(10.0, 66.0, 10.0), 1.0);
        let npc = app.world_mut().spawn((NpcMarker, nav)).id();
        let action = app
            .world_mut()
            .spawn((Actor(npc), SeclusionAction, ActionState::Requested))
            .id();

        // Tick 1: Requested → Executing，Navigator 被 stop + 插入 SeclusionState。
        app.update();
        assert!(app.world().get::<Navigator>(npc).unwrap().is_idle());
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Executing
        );

        // 强行快进 timer 到阈值。
        {
            let mut s = app
                .world_mut()
                .get_mut::<SeclusionState>(npc)
                .expect("SeclusionState should be inserted");
            s.elapsed_ticks = SECLUSION_CYCLE_TICKS - 1;
        }
        app.update();

        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success
        );
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
        let rogue = app
            .world_mut()
            .spawn((NpcMarker, Position::new([0.0, 66.0, 0.0])))
            .id();
        let mut bundle = npc_runtime_bundle(rogue, NpcArchetype::Rogue);
        bundle.cultivation.qi_max = 400.0;
        bundle.cultivation.qi_current = 400.0;
        bundle.cultivation.composure_recover_rate = 0.01;
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

        // 30000 tick：多次突破 + 失败恢复 + 开多脉的完整循环预算。
        for _ in 0..30000 {
            app.update();
        }

        let cultivation = app.world().get::<Cultivation>(rogue).unwrap();
        let meridians = app.world().get::<MeridianSystem>(rogue).unwrap();

        // 推脉：Condense 阈 4 脉。
        assert!(
            meridians.opened_count() >= 4,
            "rogue should open at least 4 meridians, got {}",
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
            InitiateXuhuaTribulation, TribulationAnnounce, TribulationFailed,
            TribulationWaveCleared,
        };
        use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};
        use crate::npc::tribulation::{
            npc_tribulation_auto_wave_tick, release_quota_for_ended_tribulations,
            AscensionQuotaStore,
        };

        let mut app = App::new();
        app.insert_resource(AscensionQuotaStore::default());
        app.insert_resource(crate::persistence::PersistenceSettings::default());
        app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
        app.add_event::<crate::skill::events::SkillCapChanged>();
        app.add_event::<InitiateXuhuaTribulation>();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationFailed>();
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
            app.world().get::<TribulationState>(rogue).is_some(),
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
            app.world().get::<TribulationState>(rogue).is_none(),
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
