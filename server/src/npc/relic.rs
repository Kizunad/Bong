//! 仙家遗种（plan-npc-ai-v1 §2/§6/§8 Phase 8）。
//!
//! 守护遗迹 + 开启考验。考验剧本本身依赖 plan-HUD 的 Dynamic XML UI，
//! 本 plan 只提供组件 + Scorer + Guard/Trial Action 的最小状态机骨架，
//! TrialAction 对接 UI 前以"立即 Success"降级，避免 thinker 卡死。

#![allow(dead_code)]

use std::time::Instant;

use big_brain::prelude::{ActionBuilder, ActionState, Actor, BigBrainSet, Score, ScorerBuilder};
use valence::client::ClientMarker;
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, EntityKind, EventWriter, IntoSystemConfigs,
    Position, PreUpdate, Query, Res, ResMut, With, Without,
};

use crate::combat::events::{AttackIntent, AttackSource};
use crate::cultivation::components::{Cultivation, Realm};
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::movement::GameTick;
use crate::npc::navigator::Navigator;
use crate::npc::perf::NpcPerfProbe;
use crate::npc::spatial::NpcSpatialIndex;
use crate::npc::spawn::{NpcMarker, NpcMeleeProfile};

/// 守护范围默认值（格）。
pub const GUARDIAN_ALARM_RADIUS_DEFAULT: f64 = 24.0;
/// Guard 追击最大距离（超过即撤回遗迹中心）。
pub const GUARD_ENGAGE_MAX_DISTANCE: f64 = 40.0;
/// Guard 攻击冷却。
pub const GUARD_ATTACK_COOLDOWN_TICKS: u32 = 35;
/// Guard 巡逻回撤速度因子。
pub const GUARD_RETURN_SPEED_FACTOR: f64 = 0.8;
/// Guard 追击速度因子。
pub const GUARD_PURSUE_SPEED_FACTOR: f64 = 1.1;
/// TrialEvalScorer 最高分（玩家 Realm=Condense/Solidify 峰值）。
pub const TRIAL_EVAL_SCORE_PEAK: f32 = 0.8;
/// TrialAction stub 退出延时（tick）。
pub const TRIAL_STUB_COOLDOWN_TICKS: u32 = 20;

// plan-npc-ai-v1 scaffolding: GuardianRelic NPC AI is not wired into the live
// thinker set yet, but the components and helpers are kept for follow-up PRs.
/// 绑定遗迹 ID + 守护半径。`relic_id` 由 worldgen 或 agent 产生，NPC 自身不管；
/// 仅用半径 + alarm_center 判入侵。
#[derive(Clone, Debug, Component)]
pub struct GuardianDuty {
    pub relic_id: String,
    pub alarm_center: DVec3,
    pub alarm_radius: f64,
}

impl GuardianDuty {
    pub fn new(relic_id: impl Into<String>, alarm_center: DVec3) -> Self {
        Self {
            relic_id: relic_id.into(),
            alarm_center,
            alarm_radius: GUARDIAN_ALARM_RADIUS_DEFAULT,
        }
    }

    pub fn with_radius(mut self, radius: f64) -> Self {
        self.alarm_radius = radius.max(1.0);
        self
    }

    pub fn contains(&self, pos: DVec3) -> bool {
        let dx = pos.x - self.alarm_center.x;
        let dz = pos.z - self.alarm_center.z;
        dx * dx + dz * dz <= self.alarm_radius * self.alarm_radius
    }
}

/// 考验模板 ID + cooldown。实际考验流程由 plan-HUD Dynamic XML UI 承接。
#[derive(Clone, Debug, Component)]
pub struct TrialEval {
    pub trial_template_id: String,
    pub last_offered_tick: Option<u32>,
    /// 两次考验之间的最短间隔（避免同一玩家反复触发）。
    pub offer_cooldown_ticks: u32,
}

impl TrialEval {
    pub fn new(trial_template_id: impl Into<String>) -> Self {
        Self {
            trial_template_id: trial_template_id.into(),
            last_offered_tick: None,
            offer_cooldown_ticks: 600,
        }
    }
}

/// 守护评分：alarm 半径内存在玩家或非 GuardianRelic NPC → 1.0，否则 0。
#[derive(Clone, Copy, Debug, Component)]
pub struct GuardianDutyScorer;

/// 考验评分：附近玩家境界 >= Condense → 接近 TRIAL_EVAL_SCORE_PEAK；
/// 更低境界线性衰减；cooldown 中返回 0。
#[derive(Clone, Copy, Debug, Component)]
pub struct TrialEvalScorer;

/// 守卫攻击入侵者：选 alarm 半径内最近 entity → 追击 + 近战 Attack；
/// 超 GUARD_ENGAGE_MAX_DISTANCE 即脱战返回中心。
#[derive(Clone, Copy, Debug, Component)]
pub struct GuardAction;

/// 考验 stub：更新 TrialEval.last_offered_tick，TRIAL_STUB_COOLDOWN_TICKS
/// 后立即 Success。plan-HUD 完成后替换为 UI 调用。
#[derive(Clone, Copy, Debug, Component)]
pub struct TrialAction;

/// GuardAction 运行态。
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct GuardState {
    pub target: Option<Entity>,
    pub last_attack_tick: Option<u32>,
}

/// TrialAction 运行态（elapsed tick）。
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct TrialState {
    pub elapsed_ticks: u32,
}

impl ScorerBuilder for GuardianDutyScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("GuardianDutyScorer")
    }
}

impl ScorerBuilder for TrialEvalScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("TrialEvalScorer")
    }
}

impl ActionBuilder for GuardAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("GuardAction")
    }
}

impl ActionBuilder for TrialAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("TrialAction")
    }
}

pub fn register(app: &mut App) {
    app.add_systems(
        PreUpdate,
        (guardian_duty_scorer_system, trial_eval_scorer_system).in_set(BigBrainSet::Scorers),
    )
    .add_systems(
        PreUpdate,
        (guard_action_system, trial_action_system).in_set(BigBrainSet::Actions),
    );
}

type GuardianRelicSelfQuery<'w, 's> =
    Query<'w, 's, (&'static Position, &'static GuardianDuty), (With<NpcMarker>, With<EntityKind>)>;

type NpcPosArchQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Position, &'static NpcArchetype), With<NpcMarker>>;

type PlayerPositionQuery<'w, 's> = Query<'w, 's, &'static Position, With<ClientMarker>>;

fn guardian_duty_scorer_system(
    self_q: Query<&GuardianDuty, With<NpcMarker>>,
    players: PlayerPositionQuery<'_, '_>,
    npcs: NpcPosArchQuery<'_, '_>,
    mut scorers: Query<(&Actor, &mut Score), With<GuardianDutyScorer>>,
    spatial_index: Option<Res<NpcSpatialIndex>>,
    game_tick: Option<Res<GameTick>>,
    mut perf_probe: Option<ResMut<NpcPerfProbe>>,
) {
    let started_at = Instant::now();
    let spatial_index = spatial_index.as_deref();
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok(duty) = self_q.get(*actor) {
            let has_player = players.iter().any(|p| duty.contains(p.get()));
            let has_intruder_npc = has_guardian_intruder_npc(*actor, duty, &npcs, spatial_index);
            if has_player || has_intruder_npc {
                1.0
            } else {
                0.0
            }
        } else {
            0.0
        };
        score.set(value);
    }

    if let Some(probe) = perf_probe.as_deref_mut() {
        probe.record_elapsed("guardian_duty_scorer", started_at);
        probe.flush_if_due(game_tick.as_deref().map(|tick| tick.0).unwrap_or(0));
    }
}

fn has_guardian_intruder_npc(
    actor: Entity,
    duty: &GuardianDuty,
    npcs: &NpcPosArchQuery<'_, '_>,
    spatial_index: Option<&NpcSpatialIndex>,
) -> bool {
    if let Some(index) = spatial_index {
        return index
            .neighbors_within(duty.alarm_center, duty.alarm_radius)
            .into_iter()
            .any(|ent| {
                ent != actor
                    && npcs.get(ent).ok().is_some_and(|(_, p, arch)| {
                        *arch != NpcArchetype::GuardianRelic && duty.contains(p.get())
                    })
            });
    }

    npcs.iter().any(|(ent, p, arch)| {
        ent != actor && *arch != NpcArchetype::GuardianRelic && duty.contains(p.get())
    })
}

type TrialSelfQuery<'w, 's> =
    Query<'w, 's, (&'static Position, &'static GuardianDuty, &'static TrialEval), With<NpcMarker>>;

type PlayerCultQuery<'w, 's> =
    Query<'w, 's, (&'static Position, &'static Cultivation), With<ClientMarker>>;

fn trial_eval_scorer_system(
    self_q: TrialSelfQuery<'_, '_>,
    players: PlayerCultQuery<'_, '_>,
    mut scorers: Query<(&Actor, &mut Score), With<TrialEvalScorer>>,
    game_tick: Option<valence::prelude::Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut score) in &mut scorers {
        let value = match self_q.get(*actor) {
            Ok((_pos, duty, eval)) => {
                // cooldown 中 → 0
                let on_cooldown = eval
                    .last_offered_tick
                    .map(|last| tick.saturating_sub(last) < eval.offer_cooldown_ticks)
                    .unwrap_or(false);
                if on_cooldown {
                    0.0
                } else {
                    players
                        .iter()
                        .filter_map(|(p, cult)| {
                            if duty.contains(p.get()) {
                                Some(realm_trial_weight(cult.realm))
                            } else {
                                None
                            }
                        })
                        .fold(0.0_f32, f32::max)
                }
            }
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

/// 境界 → 考验评分权重：Awaken 小、Condense/Solidify 峰值、化虚后回落。
pub(crate) fn realm_trial_weight(realm: Realm) -> f32 {
    match realm {
        Realm::Awaken => 0.1,
        Realm::Induce => 0.3,
        Realm::Condense => TRIAL_EVAL_SCORE_PEAK,
        Realm::Solidify => TRIAL_EVAL_SCORE_PEAK,
        Realm::Spirit => 0.5,
        Realm::Void => 0.2,
    }
}

type GuardMutQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static GuardianDuty,
        &'static NpcMeleeProfile,
        &'static mut Navigator,
        &'static mut GuardState,
    ),
    With<NpcMarker>,
>;

type GuardCandidateQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Position, Option<&'static NpcArchetype>),
    Without<GuardianRelicTag>,
>;

/// 空 marker：排除自身 / 其它 GuardianRelic NPC 出 Guard 目标筛选。
/// 通过 spawn_relic_guard_npc_at 统一挂。
#[derive(Clone, Copy, Debug, Component)]
pub struct GuardianRelicTag;

fn guard_action_system(
    mut self_q: GuardMutQuery<'_, '_>,
    candidates: GuardCandidateQuery<'_, '_>,
    npc_arch: Query<&NpcArchetype, With<NpcMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<GuardAction>>,
    mut attack_intents: EventWriter<AttackIntent>,
    game_tick: Option<valence::prelude::Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut state) in &mut actions {
        let Ok((pos, duty, profile, mut navigator, mut guard)) = self_q.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };
        match *state {
            ActionState::Requested => {
                let target = pick_guard_target(pos.get(), duty, &candidates, &npc_arch, *actor);
                match target {
                    Some((ent, tpos)) => {
                        navigator.set_goal(tpos, GUARD_PURSUE_SPEED_FACTOR);
                        guard.target = Some(ent);
                        guard.last_attack_tick = None;
                        *state = ActionState::Executing;
                    }
                    None => {
                        // 无入侵者：回撤中心，立刻 Success
                        navigator.set_goal(duty.alarm_center, GUARD_RETURN_SPEED_FACTOR);
                        *state = ActionState::Success;
                    }
                }
            }
            ActionState::Executing => {
                let Some(target) = guard.target else {
                    *state = ActionState::Success;
                    continue;
                };
                let Ok((_, tpos, _)) = candidates.get(target) else {
                    guard.target = None;
                    navigator.set_goal(duty.alarm_center, GUARD_RETURN_SPEED_FACTOR);
                    *state = ActionState::Success;
                    continue;
                };
                let tposv = tpos.get();
                let dist_to_center = pos.get().distance(duty.alarm_center);
                if dist_to_center > GUARD_ENGAGE_MAX_DISTANCE {
                    // 追得太远了，撤回
                    guard.target = None;
                    navigator.set_goal(duty.alarm_center, GUARD_RETURN_SPEED_FACTOR);
                    *state = ActionState::Success;
                    continue;
                }
                navigator.set_goal(tposv, GUARD_PURSUE_SPEED_FACTOR);
                let dist_to_target = pos.get().distance(tposv) as f32;
                let cooled = guard
                    .last_attack_tick
                    .map(|last| tick.saturating_sub(last) >= GUARD_ATTACK_COOLDOWN_TICKS)
                    .unwrap_or(true);
                if dist_to_target <= profile.reach.max && cooled {
                    attack_intents.send(AttackIntent {
                        attacker: *actor,
                        target: Some(target),
                        issued_at_tick: u64::from(tick),
                        reach: profile.reach,
                        qi_invest: 12.0,
                        wound_kind: profile.wound_kind,
                        source: AttackSource::Melee,
                        debug_command: None,
                    });
                    guard.last_attack_tick = Some(tick);
                }
            }
            ActionState::Cancelled => {
                guard.target = None;
                navigator.stop();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

pub(crate) fn pick_guard_target(
    self_pos: DVec3,
    duty: &GuardianDuty,
    candidates: &GuardCandidateQuery<'_, '_>,
    npc_arch: &Query<&NpcArchetype, With<NpcMarker>>,
    self_entity: Entity,
) -> Option<(Entity, DVec3)> {
    let mut best: Option<(Entity, DVec3, f64)> = None;
    for (entity, pos, _) in candidates.iter() {
        if entity == self_entity {
            continue;
        }
        let p = pos.get();
        if !duty.contains(p) {
            continue;
        }
        // 排除同类 GuardianRelic NPC（NpcMarker 且 archetype=GuardianRelic）
        if npc_arch.get(entity).ok().copied() == Some(NpcArchetype::GuardianRelic) {
            continue;
        }
        let d = self_pos.distance(p);
        if best.as_ref().map(|(_, _, bd)| d < *bd).unwrap_or(true) {
            best = Some((entity, p, d));
        }
    }
    best.map(|(e, p, _)| (e, p))
}

fn trial_action_system(
    mut self_q: Query<(&mut TrialEval, &mut TrialState), With<NpcMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<TrialAction>>,
    game_tick: Option<valence::prelude::Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut state) in &mut actions {
        let Ok((mut eval, mut trial_state)) = self_q.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };
        match *state {
            ActionState::Requested => {
                // stub：记录最后一次 offer，进入短暂 Executing 即 Success
                eval.last_offered_tick = Some(tick);
                trial_state.elapsed_ticks = 0;
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                trial_state.elapsed_ticks = trial_state.elapsed_ticks.saturating_add(1);
                if trial_state.elapsed_ticks >= TRIAL_STUB_COOLDOWN_TICKS {
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

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, PreUpdate};

    fn spawn_guardian(app: &mut App, duty: GuardianDuty) -> Entity {
        app.world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::GuardianRelic,
                Position::new([
                    duty.alarm_center.x,
                    duty.alarm_center.y,
                    duty.alarm_center.z,
                ]),
                NpcMeleeProfile::fist(),
                Navigator::new(),
                GuardState::default(),
                GuardianRelicTag,
                duty,
            ))
            .id()
    }

    // --- GuardianDuty ---

    #[test]
    fn guardian_duty_contains_circle_xz() {
        let duty = GuardianDuty::new("relic_a", DVec3::new(0.0, 64.0, 0.0)).with_radius(10.0);
        assert!(duty.contains(DVec3::new(5.0, 64.0, 5.0)));
        assert!(duty.contains(DVec3::new(10.0, 64.0, 0.0)));
        assert!(!duty.contains(DVec3::new(11.0, 64.0, 0.0)));
        assert!(duty.contains(DVec3::new(7.0, 0.0, 7.0)), "y 不参与");
    }

    #[test]
    fn guardian_duty_radius_clamped_to_at_least_one() {
        let duty = GuardianDuty::new("r", DVec3::ZERO).with_radius(0.0);
        assert!(duty.alarm_radius >= 1.0);
    }

    // --- GuardianDutyScorer ---

    fn build_scorer_app() -> App {
        let mut app = App::new();
        app.add_systems(PreUpdate, guardian_duty_scorer_system);
        app
    }

    #[test]
    fn guardian_duty_scorer_zero_when_empty() {
        let mut app = build_scorer_app();
        let guardian = spawn_guardian(
            &mut app,
            GuardianDuty::new("r", DVec3::new(0.0, 64.0, 0.0)).with_radius(10.0),
        );
        let scorer = app
            .world_mut()
            .spawn((Actor(guardian), Score::default(), GuardianDutyScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    #[test]
    fn guardian_duty_scorer_one_when_player_inside() {
        let mut app = build_scorer_app();
        let guardian = spawn_guardian(
            &mut app,
            GuardianDuty::new("r", DVec3::new(0.0, 64.0, 0.0)).with_radius(10.0),
        );
        let _ = app
            .world_mut()
            .spawn((ClientMarker, Position::new([5.0, 64.0, 5.0])))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(guardian), Score::default(), GuardianDutyScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 1.0);
    }

    #[test]
    fn guardian_duty_scorer_zero_when_only_other_relic_guards_inside() {
        let mut app = build_scorer_app();
        let guardian = spawn_guardian(
            &mut app,
            GuardianDuty::new("r", DVec3::new(0.0, 64.0, 0.0)).with_radius(10.0),
        );
        let _ = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::GuardianRelic,
                Position::new([4.0, 64.0, 4.0]),
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(guardian), Score::default(), GuardianDutyScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    #[test]
    fn guardian_duty_scorer_one_when_non_relic_npc_inside() {
        let mut app = build_scorer_app();
        let guardian = spawn_guardian(
            &mut app,
            GuardianDuty::new("r", DVec3::new(0.0, 64.0, 0.0)).with_radius(10.0),
        );
        let _ = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Rogue,
                Position::new([4.0, 64.0, 4.0]),
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(guardian), Score::default(), GuardianDutyScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 1.0);
    }

    // --- TrialEvalScorer ---

    fn build_trial_scorer_app() -> App {
        let mut app = App::new();
        app.insert_resource(GameTick(100));
        app.add_systems(PreUpdate, trial_eval_scorer_system);
        app
    }

    #[test]
    fn trial_eval_scorer_zero_when_no_player() {
        let mut app = build_trial_scorer_app();
        let guardian = spawn_guardian(
            &mut app,
            GuardianDuty::new("r", DVec3::new(0.0, 64.0, 0.0)).with_radius(20.0),
        );
        app.world_mut()
            .entity_mut(guardian)
            .insert(TrialEval::new("trial.basic"));
        let scorer = app
            .world_mut()
            .spawn((Actor(guardian), Score::default(), TrialEvalScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    #[test]
    fn trial_eval_scorer_peaks_at_condense_or_solidify() {
        let mut app = build_trial_scorer_app();
        let guardian = spawn_guardian(
            &mut app,
            GuardianDuty::new("r", DVec3::new(0.0, 64.0, 0.0)).with_radius(20.0),
        );
        app.world_mut()
            .entity_mut(guardian)
            .insert(TrialEval::new("trial.basic"));

        let cult = Cultivation {
            realm: Realm::Condense,
            ..Default::default()
        };
        let _ = app
            .world_mut()
            .spawn((ClientMarker, Position::new([5.0, 64.0, 5.0]), cult))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(guardian), Score::default(), TrialEvalScorer))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            TRIAL_EVAL_SCORE_PEAK
        );
    }

    #[test]
    fn trial_eval_scorer_zero_during_cooldown() {
        let mut app = build_trial_scorer_app();
        let guardian = spawn_guardian(
            &mut app,
            GuardianDuty::new("r", DVec3::new(0.0, 64.0, 0.0)).with_radius(20.0),
        );
        let mut trial = TrialEval::new("trial.basic");
        trial.last_offered_tick = Some(100); // 当前 tick
        app.world_mut().entity_mut(guardian).insert(trial);
        let cult = Cultivation {
            realm: Realm::Condense,
            ..Default::default()
        };
        let _ = app
            .world_mut()
            .spawn((ClientMarker, Position::new([5.0, 64.0, 5.0]), cult))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(guardian), Score::default(), TrialEvalScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    // --- GuardAction ---

    fn build_guard_app() -> App {
        let mut app = App::new();
        app.insert_resource(GameTick(500));
        app.add_event::<AttackIntent>();
        app.add_systems(PreUpdate, guard_action_system);
        app
    }

    #[test]
    fn guard_action_success_when_no_target_inside() {
        let mut app = build_guard_app();
        let guardian = spawn_guardian(
            &mut app,
            GuardianDuty::new("r", DVec3::new(0.0, 64.0, 0.0)).with_radius(10.0),
        );
        let action = app
            .world_mut()
            .spawn((Actor(guardian), GuardAction, ActionState::Requested))
            .id();
        app.update();
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success
        );
    }

    #[test]
    fn guard_action_targets_player_and_emits_attack_at_melee_range() {
        let mut app = build_guard_app();
        let guardian = spawn_guardian(
            &mut app,
            GuardianDuty::new("r", DVec3::new(0.0, 64.0, 0.0)).with_radius(10.0),
        );
        let _player = app
            .world_mut()
            .spawn((ClientMarker, Position::new([0.7, 64.0, 0.0])))
            .id();
        let _action = app
            .world_mut()
            .spawn((Actor(guardian), GuardAction, ActionState::Requested))
            .id();
        app.update(); // Requested → Executing
        app.update(); // Executing + 攻击
        let events = app
            .world()
            .resource::<valence::prelude::Events<AttackIntent>>();
        let mut reader = events.get_reader();
        let fired: Vec<_> = reader.read(events).cloned().collect();
        assert!(!fired.is_empty(), "在 melee 距离应发 AttackIntent");
    }

    #[test]
    fn guard_action_returns_when_target_leaves_alarm_radius() {
        let mut app = build_guard_app();
        let duty = GuardianDuty::new("r", DVec3::new(0.0, 64.0, 0.0)).with_radius(5.0);
        let guardian = spawn_guardian(&mut app, duty.clone());
        let _player = app
            .world_mut()
            .spawn((ClientMarker, Position::new([3.0, 64.0, 0.0])))
            .id();
        let action = app
            .world_mut()
            .spawn((Actor(guardian), GuardAction, ActionState::Requested))
            .id();
        app.update(); // Requested → Executing
                      // 把 guardian 扯远到超出 GUARD_ENGAGE_MAX_DISTANCE
        {
            let mut pos = app.world_mut().get_mut::<Position>(guardian).unwrap();
            pos.set([100.0, 64.0, 100.0]);
        }
        app.update();
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success,
            "扯太远应脱战"
        );
    }

    // --- TrialAction stub ---

    fn build_trial_action_app() -> App {
        let mut app = App::new();
        app.insert_resource(GameTick(1000));
        app.add_systems(PreUpdate, trial_action_system);
        app
    }

    #[test]
    fn trial_action_stub_records_offered_tick_and_succeeds_after_cooldown() {
        let mut app = build_trial_action_app();
        let guardian = spawn_guardian(
            &mut app,
            GuardianDuty::new("r", DVec3::new(0.0, 64.0, 0.0)).with_radius(10.0),
        );
        app.world_mut()
            .entity_mut(guardian)
            .insert((TrialEval::new("trial.basic"), TrialState::default()));
        let action = app
            .world_mut()
            .spawn((Actor(guardian), TrialAction, ActionState::Requested))
            .id();
        app.update(); // Requested → Executing, 记 last_offered_tick
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Executing
        );
        let eval = app
            .world()
            .get::<TrialEval>(guardian)
            .unwrap()
            .last_offered_tick;
        assert_eq!(eval, Some(1000));

        // 推 TrialState.elapsed 到阈值
        {
            let mut st = app.world_mut().get_mut::<TrialState>(guardian).unwrap();
            st.elapsed_ticks = TRIAL_STUB_COOLDOWN_TICKS - 1;
        }
        app.update();
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success
        );
    }

    // --- realm_trial_weight ---

    #[test]
    fn realm_trial_weight_peaks_at_condense_and_solidify() {
        assert_eq!(realm_trial_weight(Realm::Condense), TRIAL_EVAL_SCORE_PEAK);
        assert_eq!(realm_trial_weight(Realm::Solidify), TRIAL_EVAL_SCORE_PEAK);
        assert!(realm_trial_weight(Realm::Awaken) < realm_trial_weight(Realm::Condense));
        assert!(realm_trial_weight(Realm::Void) < realm_trial_weight(Realm::Spirit));
    }
}
