//! 黑武士 BOSS runtime（plan-sword-path-v2 P3）。
//!
//! 黑武士作为 Bong 自定义 Marker + 渲染实体接入：server 只负责状态机、big-brain
//! scorer/action、攻击意图与掉落链路；不使用 vanilla 有碰撞实体伪装。

use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{
    ActionBuilder, ActionState, Actor, BigBrainSet, FirstToScore, Score, ScorerBuilder, Thinker,
    ThinkerBuilder,
};
use valence::entity::marker::MarkerEntityBundle;
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, EntityLayerId, EventWriter,
    IntoSystemConfigs, Position, PreUpdate, Query, Res, Update, With,
};

use crate::combat::components::{DerivedAttrs, Lifecycle, LifecycleState, WoundKind, Wounds};
use crate::combat::events::{
    ApplyStatusEffectIntent, AttackIntent, AttackReach, AttackSource, StatusEffectKind,
};
use crate::fauna::components::{BeastKind, FaunaTag};
use crate::fauna::visual::{FaunaVisualKind, HEIWUSHI_ENTITY_KIND};
use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};
use crate::npc::movement::{GameTick, MovementCapabilities, MovementController, MovementCooldowns};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{NpcBlackboard, NpcCombatLoadout, NpcMarker, NpcMeleeArchetype};

pub const HEIWUSHI_HEALTH_MAX: f32 = 2100.0;
pub const HEIWUSHI_BASE_ATTACK: f32 = 35.0;
pub const HEIWUSHI_DEFENSE: f32 = 8.0;
pub const HEIWUSHI_MOVE_SPEED: f64 = 4.8;
pub const HEIWUSHI_PHASE3_MOVE_SPEED: f64 = 7.2;
pub const GROWTH_CYCLE_TICKS: u64 = 600;
pub const CD_DECAY_PER_CYCLE: f64 = 0.85;
pub const CD_FLOOR_RATIO: f64 = 0.40;
pub const MELEE_BASE_CD: u32 = 40;
pub const BARRAGE_BASE_CD: u32 = 60;
pub const VORTEX_BASE_CD: u32 = 80;
pub const MELEE_RANGE: f32 = 3.0;
pub const BARRAGE_MIN_RANGE: f32 = 4.0;
pub const BARRAGE_MAX_RANGE: f32 = 8.0;
pub const VORTEX_RANGE: f32 = 6.0;
const THINKER_THRESHOLD: f32 = 0.3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct HeiwushiMarker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeiwushiPhase {
    Phase1,
    Phase2,
    Phase3,
}

#[derive(Debug, Clone, PartialEq, Component)]
pub struct HeiwushiState {
    pub phase: HeiwushiPhase,
    pub health_max: f32,
    pub health: f32,
    pub base_attack: f32,
    pub defense: f32,
    pub move_speed: f64,
    pub growth_cycles: u32,
    pub last_cycle_tick: u64,
    pub skill_cooldowns: HeiwushiCooldowns,
}

impl Default for HeiwushiState {
    fn default() -> Self {
        Self {
            phase: HeiwushiPhase::Phase1,
            health_max: HEIWUSHI_HEALTH_MAX,
            health: HEIWUSHI_HEALTH_MAX,
            base_attack: HEIWUSHI_BASE_ATTACK,
            defense: HEIWUSHI_DEFENSE,
            move_speed: HEIWUSHI_MOVE_SPEED,
            growth_cycles: 0,
            last_cycle_tick: 0,
            skill_cooldowns: HeiwushiCooldowns::default(),
        }
    }
}

impl HeiwushiState {
    pub fn sync_phase_from_health(&mut self, health_current: f32, health_max: f32) {
        self.health = health_current.max(0.0);
        self.health_max = health_max.max(1.0);
        let ratio = self.health / self.health_max;
        if ratio < 0.25 {
            self.apply_phase3_transform();
        } else if ratio < 0.60 && self.phase == HeiwushiPhase::Phase1 {
            self.phase = HeiwushiPhase::Phase2;
        }
    }

    pub fn phase_damage_multiplier(&self) -> f32 {
        match self.phase {
            HeiwushiPhase::Phase1 => 1.0,
            HeiwushiPhase::Phase2 => 1.3,
            HeiwushiPhase::Phase3 => 2.0,
        }
    }

    pub fn apply_phase3_transform(&mut self) {
        if self.phase == HeiwushiPhase::Phase3 {
            return;
        }
        self.phase = HeiwushiPhase::Phase3;
        self.base_attack = HEIWUSHI_BASE_ATTACK * 2.0;
        self.defense = HEIWUSHI_DEFENSE * 0.5;
        self.move_speed = HEIWUSHI_PHASE3_MOVE_SPEED;
        self.skill_cooldowns.reset_ready();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeiwushiCooldowns {
    pub melee_slash: u32,
    pub dark_barrage: u32,
    pub dark_vortex: u32,
    pub shadow_transform: u32,
    pub base_melee_slash: u32,
    pub base_dark_barrage: u32,
    pub base_dark_vortex: u32,
}

impl Default for HeiwushiCooldowns {
    fn default() -> Self {
        Self {
            melee_slash: 0,
            dark_barrage: 0,
            dark_vortex: 0,
            shadow_transform: 0,
            base_melee_slash: MELEE_BASE_CD,
            base_dark_barrage: BARRAGE_BASE_CD,
            base_dark_vortex: VORTEX_BASE_CD,
        }
    }
}

impl HeiwushiCooldowns {
    pub fn effective_cd(&self, base: u32, cycles: u32) -> u32 {
        let factor = CD_DECAY_PER_CYCLE.powi(cycles as i32).max(CD_FLOOR_RATIO);
        ((base as f64 * factor).round() as u32).max(1)
    }

    fn tick_down(&mut self) {
        self.melee_slash = self.melee_slash.saturating_sub(1);
        self.dark_barrage = self.dark_barrage.saturating_sub(1);
        self.dark_vortex = self.dark_vortex.saturating_sub(1);
        self.shadow_transform = self.shadow_transform.saturating_sub(1);
    }

    fn reset_ready(&mut self) {
        self.melee_slash = 0;
        self.dark_barrage = 0;
        self.dark_vortex = 0;
        self.shadow_transform = 0;
    }
}

#[derive(Clone, Copy, Debug, Component)]
pub struct HeiwushiDeathScorer;
#[derive(Clone, Copy, Debug, Component)]
pub struct HeiwushiTransformScorer;
#[derive(Clone, Copy, Debug, Component)]
pub struct HeiwushiVortexScorer;
#[derive(Clone, Copy, Debug, Component)]
pub struct HeiwushiBarrageScorer;
#[derive(Clone, Copy, Debug, Component)]
pub struct HeiwushiMeleeScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct HeiwushiIdleAction;
#[derive(Clone, Copy, Debug, Component)]
pub struct HeiwushiMeleeSlashAction;
#[derive(Clone, Copy, Debug, Component)]
pub struct HeiwushiDarkBarrageAction;
#[derive(Clone, Copy, Debug, Component)]
pub struct HeiwushiDarkVortexAction;
#[derive(Clone, Copy, Debug, Component)]
pub struct HeiwushiShadowTransformAction;
#[derive(Clone, Copy, Debug, Component)]
pub struct HeiwushiDeathAction;

macro_rules! scorer_builder {
    ($ty:ty, $label:literal) => {
        impl ScorerBuilder for $ty {
            fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
                cmd.entity(scorer).insert(*self);
            }

            fn label(&self) -> Option<&str> {
                Some($label)
            }
        }
    };
}

macro_rules! action_builder {
    ($ty:ty, $label:literal) => {
        impl ActionBuilder for $ty {
            fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
                cmd.entity(action).insert(*self);
            }

            fn label(&self) -> Option<&str> {
                Some($label)
            }
        }
    };
}

scorer_builder!(HeiwushiDeathScorer, "HeiwushiDeathScorer");
scorer_builder!(HeiwushiTransformScorer, "HeiwushiTransformScorer");
scorer_builder!(HeiwushiVortexScorer, "HeiwushiVortexScorer");
scorer_builder!(HeiwushiBarrageScorer, "HeiwushiBarrageScorer");
scorer_builder!(HeiwushiMeleeScorer, "HeiwushiMeleeScorer");
action_builder!(HeiwushiIdleAction, "HeiwushiIdleAction");
action_builder!(HeiwushiMeleeSlashAction, "HeiwushiMeleeSlashAction");
action_builder!(HeiwushiDarkBarrageAction, "HeiwushiDarkBarrageAction");
action_builder!(HeiwushiDarkVortexAction, "HeiwushiDarkVortexAction");
action_builder!(
    HeiwushiShadowTransformAction,
    "HeiwushiShadowTransformAction"
);
action_builder!(HeiwushiDeathAction, "HeiwushiDeathAction");

pub fn register(app: &mut App) {
    app.add_systems(
        PreUpdate,
        (
            heiwushi_death_scorer_system,
            heiwushi_transform_scorer_system,
            heiwushi_vortex_scorer_system,
            heiwushi_barrage_scorer_system,
            heiwushi_melee_scorer_system,
        )
            .in_set(BigBrainSet::Scorers),
    )
    .add_systems(
        PreUpdate,
        (
            heiwushi_idle_action_system,
            heiwushi_melee_slash_action_system,
            heiwushi_dark_barrage_action_system,
            heiwushi_dark_vortex_action_system,
            heiwushi_shadow_transform_action_system,
            heiwushi_death_action_system,
        )
            .in_set(BigBrainSet::Actions),
    )
    .add_systems(
        Update,
        (
            heiwushi_growth_tick_system,
            heiwushi_cooldown_tick_system,
            heiwushi_phase_sync_system,
        ),
    );
}

pub fn heiwushi_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore {
            threshold: THINKER_THRESHOLD,
        })
        .when(HeiwushiDeathScorer, HeiwushiDeathAction)
        .when(HeiwushiTransformScorer, HeiwushiShadowTransformAction)
        .when(HeiwushiVortexScorer, HeiwushiDarkVortexAction)
        .when(HeiwushiBarrageScorer, HeiwushiDarkBarrageAction)
        .when(HeiwushiMeleeScorer, HeiwushiMeleeSlashAction)
        .otherwise(HeiwushiIdleAction)
}

pub fn spawn_heiwushi_at(
    commands: &mut Commands,
    layer: Entity,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_center: DVec3,
) -> Entity {
    let loadout = NpcCombatLoadout::fighter(NpcMeleeArchetype::Sword);
    let entity = commands
        .spawn(MarkerEntityBundle {
            kind: HEIWUSHI_ENTITY_KIND,
            layer: EntityLayerId(layer),
            position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
            ..Default::default()
        })
        .insert((
            Transform::from_xyz(
                spawn_position.x as f32,
                spawn_position.y as f32,
                spawn_position.z as f32,
            ),
            GlobalTransform::default(),
            NpcMarker,
            HeiwushiMarker,
            HeiwushiState::default(),
            NpcBlackboard::default(),
            loadout.clone(),
            loadout.melee_archetype,
            loadout.melee_profile(),
            NpcArchetype::Beast,
            FaunaTag::new(BeastKind::Heiwushi),
            FaunaVisualKind::Heiwushi,
        ))
        .insert((
            Navigator::new(),
            MovementController::new(),
            MovementCapabilities {
                can_sprint: true,
                can_dash: true,
            },
            MovementCooldowns::default(),
            NpcPatrol::new(home_zone, patrol_center),
            heiwushi_thinker(),
        ))
        .id();

    let mut runtime = npc_runtime_bundle(entity, NpcArchetype::Beast);
    runtime.wounds.health_current = HEIWUSHI_HEALTH_MAX;
    runtime.wounds.health_max = HEIWUSHI_HEALTH_MAX;
    // 黑武士近战走物理命中路径（qi_invest=0），伤害靠 DerivedAttrs.attack_power 传递；
    // 初始值 = Phase1 base_attack（35.0），heiwushi_phase_sync_system 随相变保持同步。
    runtime.derived_attrs.attack_power = HEIWUSHI_BASE_ATTACK;
    commands.entity(entity).insert(runtime);
    entity
}

fn heiwushi_growth_tick_system(
    tick: Option<Res<GameTick>>,
    mut bosses: Query<&mut HeiwushiState, With<HeiwushiMarker>>,
) {
    let now = tick.map(|t| u64::from(t.0)).unwrap_or(0);
    for mut state in &mut bosses {
        if now.saturating_sub(state.last_cycle_tick) >= GROWTH_CYCLE_TICKS {
            state.growth_cycles = state.growth_cycles.saturating_add(1);
            state.last_cycle_tick = now;
        }
    }
}

fn heiwushi_cooldown_tick_system(mut bosses: Query<&mut HeiwushiState, With<HeiwushiMarker>>) {
    for mut state in &mut bosses {
        state.skill_cooldowns.tick_down();
    }
}

fn heiwushi_phase_sync_system(
    mut bosses: Query<(&Wounds, &mut HeiwushiState, &mut DerivedAttrs), With<HeiwushiMarker>>,
) {
    for (wounds, mut state, mut derived) in &mut bosses {
        state.sync_phase_from_health(wounds.health_current, wounds.health_max);
        // 近战走物理路径（qi_invest=0），DerivedAttrs.attack_power 是伤害唯一来源；
        // 随相变保持 = base_attack（已含 Phase3 翻倍）× phase_damage_multiplier()。
        derived.attack_power = state.base_attack * state.phase_damage_multiplier();
    }
}

fn heiwushi_death_scorer_system(
    bosses: Query<&Wounds, With<HeiwushiMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<HeiwushiDeathScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = bosses
            .get(*actor)
            .map(|w| if w.health_current <= 0.0 { 1.0 } else { 0.0 })
            .unwrap_or(0.0);
        score.set(value);
    }
}

fn heiwushi_transform_scorer_system(
    bosses: Query<(&Wounds, &HeiwushiState), With<HeiwushiMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<HeiwushiTransformScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = bosses
            .get(*actor)
            .map(|(wounds, state)| {
                let ratio = wounds.health_current / wounds.health_max.max(1.0);
                if ratio < 0.25 && state.phase != HeiwushiPhase::Phase3 {
                    0.95
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        score.set(value);
    }
}

fn heiwushi_vortex_scorer_system(
    bosses: Query<(&NpcBlackboard, &HeiwushiState), With<HeiwushiMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<HeiwushiVortexScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = bosses
            .get(*actor)
            .map(|(bb, state)| {
                if matches!(state.phase, HeiwushiPhase::Phase2 | HeiwushiPhase::Phase3)
                    && bb.player_distance <= VORTEX_RANGE
                    && state.skill_cooldowns.dark_vortex == 0
                {
                    0.8
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        score.set(value);
    }
}

fn heiwushi_barrage_scorer_system(
    bosses: Query<(&NpcBlackboard, &HeiwushiState), With<HeiwushiMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<HeiwushiBarrageScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = bosses
            .get(*actor)
            .map(|(bb, state)| {
                if matches!(state.phase, HeiwushiPhase::Phase1 | HeiwushiPhase::Phase2)
                    && (BARRAGE_MIN_RANGE..=BARRAGE_MAX_RANGE).contains(&bb.player_distance)
                    && state.skill_cooldowns.dark_barrage == 0
                {
                    0.7
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        score.set(value);
    }
}

fn heiwushi_melee_scorer_system(
    bosses: Query<(&NpcBlackboard, &HeiwushiState), With<HeiwushiMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<HeiwushiMeleeScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = bosses
            .get(*actor)
            .map(|(bb, state)| {
                if bb.player_distance <= MELEE_RANGE && state.skill_cooldowns.melee_slash == 0 {
                    0.6
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        score.set(value);
    }
}

fn heiwushi_idle_action_system(
    mut bosses: Query<(&Position, &NpcPatrol, &mut Navigator), With<HeiwushiMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<HeiwushiIdleAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((position, patrol, mut navigator)) = bosses.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };
        match *state {
            ActionState::Requested => *state = ActionState::Executing,
            ActionState::Executing => {
                if navigator.is_idle() {
                    let center = patrol.current_target;
                    let phase =
                        ((position.get().x.to_bits() ^ position.get().z.to_bits()) % 3) as f64;
                    let target = DVec3::new(center.x + phase * 6.0 - 6.0, center.y, center.z + 4.0);
                    navigator.set_goal(target, 0.6);
                }
            }
            ActionState::Cancelled => {
                navigator.stop();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn heiwushi_melee_slash_action_system(
    mut bosses: Query<(&NpcBlackboard, &mut HeiwushiState, &mut Navigator), With<HeiwushiMarker>>,
    mut attacks: EventWriter<AttackIntent>,
    tick: Option<Res<GameTick>>,
    mut actions: Query<(&Actor, &mut ActionState), With<HeiwushiMeleeSlashAction>>,
) {
    let now = tick.map(|t| u64::from(t.0)).unwrap_or(0);
    for (Actor(actor), mut action_state) in &mut actions {
        let Ok((bb, mut state, mut navigator)) = bosses.get_mut(*actor) else {
            *action_state = ActionState::Failure;
            continue;
        };
        match *action_state {
            ActionState::Requested => {
                navigator.stop();
                *action_state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Some(target) = bb.nearest_player else {
                    *action_state = ActionState::Failure;
                    continue;
                };
                // qi_invest=0 → 物理命中路径（is_physical_hit=true），绕开 qi 闸门。
                // 相位缩放伤害已写入 DerivedAttrs.attack_power（由 heiwushi_phase_sync_system 维护）。
                attacks.send(AttackIntent {
                    attacker: *actor,
                    target: Some(target),
                    issued_at_tick: now,
                    reach: AttackReach::new(3.0, 0.5),
                    qi_invest: 0.0,
                    wound_kind: WoundKind::Cut,
                    source: AttackSource::Melee,
                    debug_command: None,
                });
                state.skill_cooldowns.melee_slash = state
                    .skill_cooldowns
                    .effective_cd(state.skill_cooldowns.base_melee_slash, state.growth_cycles);
                *action_state = ActionState::Success;
            }
            ActionState::Cancelled => *action_state = ActionState::Failure,
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn heiwushi_dark_barrage_action_system(
    mut bosses: Query<(&NpcBlackboard, &mut HeiwushiState), With<HeiwushiMarker>>,
    mut attacks: EventWriter<AttackIntent>,
    tick: Option<Res<GameTick>>,
    mut actions: Query<(&Actor, &mut ActionState), With<HeiwushiDarkBarrageAction>>,
) {
    let now = tick.map(|t| u64::from(t.0)).unwrap_or(0);
    for (Actor(actor), mut action_state) in &mut actions {
        let Ok((bb, mut state)) = bosses.get_mut(*actor) else {
            *action_state = ActionState::Failure;
            continue;
        };
        match *action_state {
            ActionState::Requested => *action_state = ActionState::Executing,
            ActionState::Executing => {
                let Some(target) = bb.nearest_player else {
                    *action_state = ActionState::Failure;
                    continue;
                };
                attacks.send(AttackIntent {
                    attacker: *actor,
                    target: Some(target),
                    issued_at_tick: now,
                    reach: AttackReach::new(8.0, 0.0),
                    qi_invest: state.base_attack * 1.2,
                    wound_kind: WoundKind::Pierce,
                    source: AttackSource::SwordPathQiSlash,
                    debug_command: None,
                });
                state.skill_cooldowns.dark_barrage = state
                    .skill_cooldowns
                    .effective_cd(state.skill_cooldowns.base_dark_barrage, state.growth_cycles);
                *action_state = ActionState::Success;
            }
            ActionState::Cancelled => *action_state = ActionState::Failure,
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn heiwushi_dark_vortex_action_system(
    mut bosses: Query<(&NpcBlackboard, &mut HeiwushiState), With<HeiwushiMarker>>,
    mut attacks: EventWriter<AttackIntent>,
    mut statuses: EventWriter<ApplyStatusEffectIntent>,
    tick: Option<Res<GameTick>>,
    mut actions: Query<(&Actor, &mut ActionState), With<HeiwushiDarkVortexAction>>,
) {
    let now = tick.map(|t| u64::from(t.0)).unwrap_or(0);
    for (Actor(actor), mut action_state) in &mut actions {
        let Ok((bb, mut state)) = bosses.get_mut(*actor) else {
            *action_state = ActionState::Failure;
            continue;
        };
        match *action_state {
            ActionState::Requested => *action_state = ActionState::Executing,
            ActionState::Executing => {
                let Some(target) = bb.nearest_player else {
                    *action_state = ActionState::Failure;
                    continue;
                };
                attacks.send(AttackIntent {
                    attacker: *actor,
                    target: Some(target),
                    issued_at_tick: now,
                    reach: AttackReach::new(6.0, 0.0),
                    qi_invest: state.base_attack * 1.5,
                    wound_kind: WoundKind::Concussion,
                    source: AttackSource::SwordPathResonance,
                    debug_command: None,
                });
                statuses.send(ApplyStatusEffectIntent {
                    target,
                    kind: StatusEffectKind::Slowed,
                    magnitude: 0.4,
                    duration_ticks: 80,
                    issued_at_tick: now,
                });
                state.skill_cooldowns.dark_vortex = state
                    .skill_cooldowns
                    .effective_cd(state.skill_cooldowns.base_dark_vortex, state.growth_cycles);
                *action_state = ActionState::Success;
            }
            ActionState::Cancelled => *action_state = ActionState::Failure,
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn heiwushi_shadow_transform_action_system(
    mut bosses: Query<&mut HeiwushiState, With<HeiwushiMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<HeiwushiShadowTransformAction>>,
) {
    for (Actor(actor), mut action_state) in &mut actions {
        let Ok(mut state) = bosses.get_mut(*actor) else {
            *action_state = ActionState::Failure;
            continue;
        };
        match *action_state {
            ActionState::Requested => *action_state = ActionState::Executing,
            ActionState::Executing => {
                state.apply_phase3_transform();
                *action_state = ActionState::Success;
            }
            ActionState::Cancelled => *action_state = ActionState::Failure,
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn heiwushi_death_action_system(
    mut bosses: Query<(&mut Lifecycle, &mut Navigator), With<HeiwushiMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<HeiwushiDeathAction>>,
) {
    for (Actor(actor), mut action_state) in &mut actions {
        let Ok((mut lifecycle, mut navigator)) = bosses.get_mut(*actor) else {
            *action_state = ActionState::Failure;
            continue;
        };
        match *action_state {
            ActionState::Requested => *action_state = ActionState::Executing,
            ActionState::Executing => {
                navigator.stop();
                lifecycle.state = LifecycleState::Terminated;
                *action_state = ActionState::Success;
            }
            ActionState::Cancelled => *action_state = ActionState::Failure,
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use big_brain::prelude::Actor;
    use valence::prelude::{Events, World};

    fn spawn_boss(world: &mut World, health_current: f32, distance: f32) -> Entity {
        world
            .spawn((
                HeiwushiMarker,
                HeiwushiState::default(),
                NpcBlackboard {
                    nearest_player: Some(Entity::PLACEHOLDER),
                    player_distance: distance,
                    ..Default::default()
                },
                Wounds {
                    entries: Vec::new(),
                    health_current,
                    health_max: HEIWUSHI_HEALTH_MAX,
                },
                Lifecycle::default(),
                Navigator::new(),
                NpcPatrol::new("giant_sword_sea", DVec3::ZERO),
                Position::new([0.0, 64.0, 0.0]),
            ))
            .id()
    }

    #[test]
    fn cooldown_effective_cd_decays_to_floor() {
        let cd = HeiwushiCooldowns::default();
        assert_eq!(cd.effective_cd(40, 0), 40);
        assert_eq!(cd.effective_cd(40, 1), 34);
        assert_eq!(cd.effective_cd(40, 2), 29);
        assert_eq!(cd.effective_cd(40, 3), 25);
        assert_eq!(cd.effective_cd(40, 20), 16);
        assert_eq!(cd.effective_cd(60, 20), 24);
        assert_eq!(cd.effective_cd(80, 20), 32);
    }

    #[test]
    fn phase_sync_enters_phase2_and_phase3_thresholds() {
        let mut state = HeiwushiState::default();
        state.sync_phase_from_health(1300.0, HEIWUSHI_HEALTH_MAX);
        assert_eq!(state.phase, HeiwushiPhase::Phase1);
        state.sync_phase_from_health(1200.0, HEIWUSHI_HEALTH_MAX);
        assert_eq!(state.phase, HeiwushiPhase::Phase2);
        state.sync_phase_from_health(500.0, HEIWUSHI_HEALTH_MAX);
        assert_eq!(state.phase, HeiwushiPhase::Phase3);
        assert_eq!(state.base_attack, HEIWUSHI_BASE_ATTACK * 2.0);
        assert_eq!(state.defense, HEIWUSHI_DEFENSE * 0.5);
        assert_eq!(state.move_speed, HEIWUSHI_PHASE3_MOVE_SPEED);
    }

    #[test]
    fn growth_tick_advances_one_cycle_per_window() {
        let mut app = App::new();
        app.insert_resource(GameTick(600));
        app.add_systems(Update, heiwushi_growth_tick_system);
        let boss = spawn_boss(app.world_mut(), HEIWUSHI_HEALTH_MAX, 3.0);

        app.update();
        assert_eq!(
            app.world()
                .get::<HeiwushiState>(boss)
                .unwrap()
                .growth_cycles,
            1
        );

        app.world_mut().resource_mut::<GameTick>().0 = 900;
        app.update();
        assert_eq!(
            app.world()
                .get::<HeiwushiState>(boss)
                .unwrap()
                .growth_cycles,
            1
        );

        app.world_mut().resource_mut::<GameTick>().0 = 1200;
        app.update();
        assert_eq!(
            app.world()
                .get::<HeiwushiState>(boss)
                .unwrap()
                .growth_cycles,
            2
        );
    }

    #[test]
    fn scorers_follow_phase_range_and_cooldown_contracts() {
        let mut app = App::new();
        app.add_systems(
            Update,
            (
                heiwushi_melee_scorer_system,
                heiwushi_barrage_scorer_system,
                heiwushi_vortex_scorer_system,
                heiwushi_transform_scorer_system,
                heiwushi_death_scorer_system,
            ),
        );
        let boss = spawn_boss(app.world_mut(), HEIWUSHI_HEALTH_MAX, 5.0);
        let melee = app
            .world_mut()
            .spawn((Actor(boss), Score::default(), HeiwushiMeleeScorer))
            .id();
        let barrage = app
            .world_mut()
            .spawn((Actor(boss), Score::default(), HeiwushiBarrageScorer))
            .id();
        let vortex = app
            .world_mut()
            .spawn((Actor(boss), Score::default(), HeiwushiVortexScorer))
            .id();
        let transform = app
            .world_mut()
            .spawn((Actor(boss), Score::default(), HeiwushiTransformScorer))
            .id();
        let death = app
            .world_mut()
            .spawn((Actor(boss), Score::default(), HeiwushiDeathScorer))
            .id();

        app.update();
        assert_eq!(app.world().get::<Score>(melee).unwrap().get(), 0.0);
        assert_eq!(app.world().get::<Score>(barrage).unwrap().get(), 0.7);
        assert_eq!(app.world().get::<Score>(vortex).unwrap().get(), 0.0);
        assert_eq!(app.world().get::<Score>(transform).unwrap().get(), 0.0);
        assert_eq!(app.world().get::<Score>(death).unwrap().get(), 0.0);

        app.world_mut()
            .get_mut::<HeiwushiState>(boss)
            .unwrap()
            .phase = HeiwushiPhase::Phase2;
        app.update();
        assert_eq!(app.world().get::<Score>(vortex).unwrap().get(), 0.8);

        app.world_mut()
            .get_mut::<Wounds>(boss)
            .unwrap()
            .health_current = 0.0;
        app.update();
        assert_eq!(app.world().get::<Score>(death).unwrap().get(), 1.0);
    }

    #[test]
    fn melee_action_emits_attack_and_sets_scaled_cooldown() {
        let mut app = App::new();
        app.add_event::<AttackIntent>();
        app.insert_resource(GameTick(42));
        app.add_systems(Update, heiwushi_melee_slash_action_system);
        let boss = spawn_boss(app.world_mut(), HEIWUSHI_HEALTH_MAX, 2.0);
        app.world_mut()
            .get_mut::<HeiwushiState>(boss)
            .unwrap()
            .growth_cycles = 1;
        app.world_mut().spawn((
            Actor(boss),
            ActionState::Requested,
            HeiwushiMeleeSlashAction,
        ));

        app.update();
        app.update();

        let attacks = app.world().resource::<Events<AttackIntent>>();
        let mut attack_reader = attacks.get_reader();
        let events: Vec<_> = attack_reader.read(attacks).collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].attacker, boss);
        assert_eq!(events[0].issued_at_tick, 42);
        assert_eq!(events[0].source, AttackSource::Melee);
        // qi_invest=0.0 → 物理命中路径，绕过 qi 闸门（修复 plan-sword-path-v2 P3 melee zero-damage bug）。
        // 若此值被改回非零（如 base_attack * phase_mult），resolver 会因 qi_current=0 拒绝 intent。
        assert_eq!(
            events[0].qi_invest, 0.0,
            "黑武士近战 qi_invest 必须为 0.0（物理路径），否则 resolver qi 闸门会拒绝攻击，\
            实际 qi_invest={:.3}",
            events[0].qi_invest
        );
        assert_eq!(
            app.world()
                .get::<HeiwushiState>(boss)
                .unwrap()
                .skill_cooldowns
                .melee_slash,
            34
        );
    }

    #[test]
    fn vortex_action_emits_attack_and_slow_status() {
        let mut app = App::new();
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.insert_resource(GameTick(77));
        app.add_systems(Update, heiwushi_dark_vortex_action_system);
        let boss = spawn_boss(app.world_mut(), HEIWUSHI_HEALTH_MAX, 4.0);
        app.world_mut()
            .get_mut::<HeiwushiState>(boss)
            .unwrap()
            .phase = HeiwushiPhase::Phase2;
        app.world_mut().spawn((
            Actor(boss),
            ActionState::Requested,
            HeiwushiDarkVortexAction,
        ));

        app.update();
        app.update();

        let attacks = app.world().resource::<Events<AttackIntent>>();
        let mut attack_reader = attacks.get_reader();
        let attack_events: Vec<_> = attack_reader.read(attacks).collect();
        assert_eq!(attack_events.len(), 1);
        assert_eq!(attack_events[0].source, AttackSource::SwordPathResonance);

        let statuses = app.world().resource::<Events<ApplyStatusEffectIntent>>();
        let mut status_reader = statuses.get_reader();
        let status_events: Vec<_> = status_reader.read(statuses).collect();
        assert_eq!(status_events.len(), 1);
        assert_eq!(status_events[0].kind, StatusEffectKind::Slowed);
    }

    // ── 饱和测试：P3 阶段边界、幂等、scorer gating ────────────────────────

    #[test]
    fn phase_sync_boundary_exact_sixty_percent_stays_phase1() {
        // HP/max == 0.60 (ratio == 0.60, NOT < 0.60) → 期望停在 Phase1
        // 边界下一个 tick (1259.9) 才应进入 Phase2
        let mut state = HeiwushiState::default();
        state.sync_phase_from_health(1260.0, HEIWUSHI_HEALTH_MAX); // 1260/2100 = 0.600…
        assert_eq!(
            state.phase,
            HeiwushiPhase::Phase1,
            "HP==60% (ratio==0.60) 不满足 ratio<0.60，期望 Phase1，实际 {:?}",
            state.phase
        );

        let mut state2 = HeiwushiState::default();
        state2.sync_phase_from_health(1259.9, HEIWUSHI_HEALTH_MAX); // 1259.9/2100 ≈ 0.5999 < 0.60
        assert_eq!(
            state2.phase,
            HeiwushiPhase::Phase2,
            "HP<60% (ratio<0.60) 期望进入 Phase2，实际 {:?}",
            state2.phase
        );
    }

    #[test]
    fn phase_sync_boundary_exact_twenty_five_percent_stays_phase2() {
        // HP/max == 0.25 (ratio == 0.25, NOT < 0.25) → 期望停在 Phase2
        // 边界下一个 tick (524.9) 才应触发 Phase3 transform
        let mut state = HeiwushiState::default();
        // 先推入 Phase2
        state.sync_phase_from_health(1000.0, HEIWUSHI_HEALTH_MAX);
        assert_eq!(state.phase, HeiwushiPhase::Phase2, "前置：应已进入 Phase2");
        state.sync_phase_from_health(525.0, HEIWUSHI_HEALTH_MAX); // 525/2100 = 0.25
        assert_eq!(
            state.phase,
            HeiwushiPhase::Phase2,
            "HP==25% (ratio==0.25) 不满足 ratio<0.25，期望留在 Phase2，实际 {:?}",
            state.phase
        );

        let mut state2 = HeiwushiState::default();
        state2.sync_phase_from_health(1000.0, HEIWUSHI_HEALTH_MAX); // Phase2
        state2.sync_phase_from_health(524.9, HEIWUSHI_HEALTH_MAX); // 524.9/2100 ≈ 0.2499 < 0.25
        assert_eq!(
            state2.phase,
            HeiwushiPhase::Phase3,
            "HP<25% (ratio<0.25) 期望触发 Phase3 transform，实际 {:?}",
            state2.phase
        );
        // Phase3 transform 副作用也应落地
        assert_eq!(
            state2.base_attack,
            HEIWUSHI_BASE_ATTACK * 2.0,
            "Phase3 transform: base_attack 期望 {}，实际 {}",
            HEIWUSHI_BASE_ATTACK * 2.0,
            state2.base_attack
        );
    }

    #[test]
    fn apply_phase3_transform_is_idempotent() {
        // apply_phase3_transform 调用两次 → base_attack 只翻一次（*2，不是*4）
        // 守卫：self.phase == Phase3 时提前 return
        let mut state = HeiwushiState::default();
        state.apply_phase3_transform();
        let attack_after_first = state.base_attack;
        let defense_after_first = state.defense;
        let speed_after_first = state.move_speed;

        state.apply_phase3_transform(); // 第二次调用：guard 应拦截
        assert_eq!(
            state.base_attack, attack_after_first,
            "第二次 apply_phase3_transform 不应再翻倍 base_attack：期望 {}，实际 {}",
            attack_after_first, state.base_attack
        );
        assert_eq!(
            state.base_attack,
            HEIWUSHI_BASE_ATTACK * 2.0,
            "Phase3 base_attack 应为原始值*2：期望 {}，实际 {}",
            HEIWUSHI_BASE_ATTACK * 2.0,
            state.base_attack
        );
        assert_eq!(
            state.defense, defense_after_first,
            "第二次调用不应再减半 defense：期望 {}，实际 {}",
            defense_after_first, state.defense
        );
        assert_eq!(
            state.defense,
            HEIWUSHI_DEFENSE * 0.5,
            "Phase3 defense 应为原始值*0.5：期望 {}，实际 {}",
            HEIWUSHI_DEFENSE * 0.5,
            state.defense
        );
        assert_eq!(
            state.move_speed, speed_after_first,
            "第二次调用不应改变 move_speed：期望 {}，实际 {}",
            speed_after_first, state.move_speed
        );
    }

    #[test]
    fn phase_damage_multiplier_all_three_variants() {
        // 每个 Phase variant 的 damage multiplier 直接断言，防止 transposition/off-by-one
        // Phase1 是 Default，直接用 default()；Phase2/Phase3 用 struct update 语法避免 clippy::field_reassign_with_default
        let phase1 = HeiwushiState::default();
        assert_eq!(
            phase1.phase_damage_multiplier(),
            1.0_f32,
            "Phase1 damage multiplier 期望 1.0，实际 {}",
            phase1.phase_damage_multiplier()
        );

        let phase2 = HeiwushiState {
            phase: HeiwushiPhase::Phase2,
            ..Default::default()
        };
        assert_eq!(
            phase2.phase_damage_multiplier(),
            1.3_f32,
            "Phase2 damage multiplier 期望 1.3，实际 {}",
            phase2.phase_damage_multiplier()
        );

        let phase3 = HeiwushiState {
            phase: HeiwushiPhase::Phase3,
            ..Default::default()
        };
        assert_eq!(
            phase3.phase_damage_multiplier(),
            2.0_f32,
            "Phase3 damage multiplier 期望 2.0，实际 {}",
            phase3.phase_damage_multiplier()
        );
    }

    #[test]
    fn scorers_return_zero_when_cooldown_active() {
        // 三个 skill scorer 在 CD > 0 时必须返回 0.0（CD gate）
        let mut app = App::new();
        app.add_systems(
            Update,
            (
                heiwushi_melee_scorer_system,
                heiwushi_barrage_scorer_system,
                heiwushi_vortex_scorer_system,
            ),
        );
        // Phase2，距离满足所有 scorer 的 range 条件，但 CD 均 > 0
        let boss = spawn_boss(app.world_mut(), HEIWUSHI_HEALTH_MAX, 2.5);
        {
            let mut state = app.world_mut().get_mut::<HeiwushiState>(boss).unwrap();
            state.phase = HeiwushiPhase::Phase2;
            state.skill_cooldowns.melee_slash = 10;
            state.skill_cooldowns.dark_barrage = 10;
            state.skill_cooldowns.dark_vortex = 10;
        }
        let melee = app
            .world_mut()
            .spawn((Actor(boss), Score::default(), HeiwushiMeleeScorer))
            .id();
        let barrage = app
            .world_mut()
            .spawn((Actor(boss), Score::default(), HeiwushiBarrageScorer))
            .id();
        let vortex = app
            .world_mut()
            .spawn((Actor(boss), Score::default(), HeiwushiVortexScorer))
            .id();

        // 距离 2.5 不在 barrage 4-8 范围内，但我们要确认 CD gate 优先
        // 再改距离为 5.0（barrage 范围内），验证 barrage CD gate
        app.world_mut()
            .get_mut::<NpcBlackboard>(boss)
            .unwrap()
            .player_distance = 5.0;

        app.update();
        assert_eq!(
            app.world().get::<Score>(melee).unwrap().get(),
            0.0,
            "melee_slash CD>0 时 scorer 期望 0.0，实际 {}",
            app.world().get::<Score>(melee).unwrap().get()
        );
        assert_eq!(
            app.world().get::<Score>(barrage).unwrap().get(),
            0.0,
            "dark_barrage CD>0 时 scorer 期望 0.0，实际 {}",
            app.world().get::<Score>(barrage).unwrap().get()
        );
        assert_eq!(
            app.world().get::<Score>(vortex).unwrap().get(),
            0.0,
            "dark_vortex CD>0 时 scorer 期望 0.0，实际 {}",
            app.world().get::<Score>(vortex).unwrap().get()
        );
    }

    #[test]
    fn transform_scorer_returns_zero_when_already_phase3() {
        // HeiwushiTransformScorer 在 phase==Phase3 时必须返回 0.0（one-time guard）
        let mut app = App::new();
        app.add_systems(Update, heiwushi_transform_scorer_system);
        // HP < 25%，phase 已经是 Phase3
        let boss = spawn_boss(app.world_mut(), 400.0, 3.0);
        app.world_mut()
            .get_mut::<HeiwushiState>(boss)
            .unwrap()
            .phase = HeiwushiPhase::Phase3;
        let transform_scorer = app
            .world_mut()
            .spawn((Actor(boss), Score::default(), HeiwushiTransformScorer))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<Score>(transform_scorer).unwrap().get(),
            0.0,
            "phase==Phase3 时 TransformScorer 期望 0.0（已变身，不应再触发），实际 {}",
            app.world().get::<Score>(transform_scorer).unwrap().get()
        );
    }

    #[test]
    fn barrage_scorer_excluded_in_phase3() {
        // HeiwushiBarrageScorer 在 Phase3 时必须返回 0.0（phase gate: Phase1|Phase2 only）
        let mut app = App::new();
        app.add_systems(Update, heiwushi_barrage_scorer_system);
        // 距离 5.0 在 barrage 范围 4-8 内，CD = 0，但 phase = Phase3
        let boss = spawn_boss(app.world_mut(), HEIWUSHI_HEALTH_MAX, 5.0);
        app.world_mut()
            .get_mut::<HeiwushiState>(boss)
            .unwrap()
            .phase = HeiwushiPhase::Phase3;
        let barrage = app
            .world_mut()
            .spawn((Actor(boss), Score::default(), HeiwushiBarrageScorer))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<Score>(barrage).unwrap().get(),
            0.0,
            "Phase3 时 BarrageScorer 期望 0.0（Phase3 不可用 barrage），实际 {}",
            app.world().get::<Score>(barrage).unwrap().get()
        );
    }

    #[test]
    fn vortex_scorer_active_in_phase3() {
        // HeiwushiVortexScorer 在 Phase3 也应激活（phase gate: Phase2|Phase3）
        let mut app = App::new();
        app.add_systems(Update, heiwushi_vortex_scorer_system);
        // 距离 4.0 <= VORTEX_RANGE=6, CD=0, phase=Phase3
        let boss = spawn_boss(app.world_mut(), HEIWUSHI_HEALTH_MAX, 4.0);
        app.world_mut()
            .get_mut::<HeiwushiState>(boss)
            .unwrap()
            .phase = HeiwushiPhase::Phase3;
        let vortex = app
            .world_mut()
            .spawn((Actor(boss), Score::default(), HeiwushiVortexScorer))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<Score>(vortex).unwrap().get(),
            0.8,
            "Phase3 + in_range + CD=0 时 VortexScorer 期望 0.8，实际 {}",
            app.world().get::<Score>(vortex).unwrap().get()
        );
    }

    #[test]
    fn effective_cd_floor_is_stable_after_floor_cycle() {
        // floor 生效后（factor 触达 CD_FLOOR_RATIO=0.40），后续 cycle 值不再下降（稳定）
        // melee base=40: floor = round(40*0.40)=16; 第一个 floor cycle 是 cycle=6（0.85^6≈0.377<0.40）
        // barrage base=60: floor = round(60*0.40)=24
        // vortex base=80: floor = round(80*0.40)=32
        // cycle=5 对 melee：factor = 0.85^5 ≈ 0.4437 > 0.40 → 仍在衰减，值 = round(40*0.4437)=18 (>16)
        let cd = HeiwushiCooldowns::default();

        // melee (base=40): cycle 5 应高于 floor（衰减未结束）
        let cycle5_melee = cd.effective_cd(40, 5);
        let cycle6_melee = cd.effective_cd(40, 6);
        let cycle7_melee = cd.effective_cd(40, 7);
        let cycle100_melee = cd.effective_cd(40, 100);
        assert!(
            cycle5_melee > cycle6_melee,
            "melee cycle 5 (值={}) 应高于 cycle 6 (值={})，floor 在 cycle 6 生效",
            cycle5_melee,
            cycle6_melee
        );
        assert_eq!(
            cycle6_melee, 16,
            "melee cycle 6 应首次到达 floor=16（round(40*0.40)=16），实际 {}",
            cycle6_melee
        );
        assert_eq!(
            cycle7_melee, cycle6_melee,
            "melee floor 稳定：cycle 7 (值={}) 应等于 cycle 6 (值={})，floor 不再下降",
            cycle7_melee, cycle6_melee
        );
        assert_eq!(
            cycle100_melee, 16,
            "melee 极大 cycle 数 (100) 应稳定在 floor=16，实际 {}",
            cycle100_melee
        );

        // barrage (base=60): floor = round(60*0.40) = 24
        let barrage_floor = cd.effective_cd(60, 100);
        assert_eq!(
            barrage_floor, 24,
            "barrage floor 期望 24（round(60*0.40)=24），实际 {}",
            barrage_floor
        );

        // vortex (base=80): floor = round(80*0.40) = 32
        let vortex_floor = cd.effective_cd(80, 100);
        assert_eq!(
            vortex_floor, 32,
            "vortex floor 期望 32（round(80*0.40)=32），实际 {}",
            vortex_floor
        );
    }

    // ── Fix 3：death / transform action system 覆盖（review 补测）────────────────────────────

    #[test]
    fn death_action_sets_lifecycle_terminated_and_stops_navigator() {
        // heiwushi_death_action_system 副作用：
        //   1. Lifecycle.state → LifecycleState::Terminated
        //   2. Navigator.stop()（is_idle() = true）
        let mut app = App::new();
        app.add_systems(Update, heiwushi_death_action_system);
        let boss = spawn_boss(app.world_mut(), 0.0, 3.0); // health=0 → death scorer 会打高分

        // 确认初始状态
        assert_eq!(
            app.world().get::<Lifecycle>(boss).unwrap().state,
            LifecycleState::Alive,
            "前置：boss 初始应为 Alive"
        );

        // 注入 Requested action state
        app.world_mut()
            .spawn((Actor(boss), ActionState::Requested, HeiwushiDeathAction));

        // Requested → Executing → Success 需要两次 update
        app.update();
        app.update();

        let lifecycle_state = app.world().get::<Lifecycle>(boss).unwrap().state;
        assert_eq!(
            lifecycle_state,
            LifecycleState::Terminated,
            "heiwushi_death_action_system 执行后 Lifecycle.state 应为 Terminated，实际 {:?}",
            lifecycle_state
        );

        let navigator_is_idle = app.world().get::<Navigator>(boss).unwrap().is_idle();
        assert!(
            navigator_is_idle,
            "heiwushi_death_action_system 执行后 Navigator 应被 stop()（is_idle=true）"
        );
    }

    #[test]
    fn death_action_is_idempotent_on_already_terminated_boss() {
        // 死亡 action 在 boss 已经是 Terminated 时不应 panic / 改其他状态
        let mut app = App::new();
        app.add_systems(Update, heiwushi_death_action_system);
        let boss = spawn_boss(app.world_mut(), 0.0, 3.0);

        // 先手动 Terminate
        app.world_mut().get_mut::<Lifecycle>(boss).unwrap().state = LifecycleState::Terminated;

        app.world_mut()
            .spawn((Actor(boss), ActionState::Requested, HeiwushiDeathAction));

        app.update();
        app.update();

        // 状态应仍是 Terminated，不应 panic 也不应回退
        assert_eq!(
            app.world().get::<Lifecycle>(boss).unwrap().state,
            LifecycleState::Terminated,
            "已 Terminated 的 boss 再次触发 death action 后状态应保持 Terminated"
        );
    }

    #[test]
    fn shadow_transform_action_transitions_to_phase3_once() {
        // heiwushi_shadow_transform_action_system：
        //   - 执行时调用 apply_phase3_transform()
        //   - ActionState 变为 Success
        //   - 第二次调用（幂等）不应再翻倍 base_attack
        let mut app = App::new();
        app.add_systems(Update, heiwushi_shadow_transform_action_system);

        // Phase1/2 boss，HP 已降至 <25%
        let boss = spawn_boss(app.world_mut(), 400.0, 3.0);
        app.world_mut()
            .get_mut::<HeiwushiState>(boss)
            .unwrap()
            .phase = HeiwushiPhase::Phase1; // 确保还没变身

        let action_entity = app
            .world_mut()
            .spawn((
                Actor(boss),
                ActionState::Requested,
                HeiwushiShadowTransformAction,
            ))
            .id();

        // Requested → Executing → Success
        app.update();
        app.update();

        let state = app.world().get::<HeiwushiState>(boss).unwrap();
        assert_eq!(
            state.phase,
            HeiwushiPhase::Phase3,
            "shadow_transform_action 执行后 phase 应为 Phase3，实际 {:?}",
            state.phase
        );
        assert_eq!(
            state.base_attack,
            HEIWUSHI_BASE_ATTACK * 2.0,
            "Phase3 base_attack 应为原始值 *2 = {}，实际 {}",
            HEIWUSHI_BASE_ATTACK * 2.0,
            state.base_attack
        );
        assert_eq!(
            state.defense,
            HEIWUSHI_DEFENSE * 0.5,
            "Phase3 defense 应为原始值 *0.5 = {}，实际 {}",
            HEIWUSHI_DEFENSE * 0.5,
            state.defense
        );

        let action_state = app.world().get::<ActionState>(action_entity).unwrap();
        assert_eq!(
            *action_state,
            ActionState::Success,
            "ShadowTransformAction 执行后 ActionState 应为 Success，实际 {:?}",
            action_state
        );
    }

    #[test]
    fn shadow_transform_action_is_one_shot_already_phase3_stays_success() {
        // apply_phase3_transform 有幂等守卫；Phase3 boss 再次收到 Requested → 应顺利 Success
        // 但 base_attack 不应再翻倍（守卫已 return）
        let mut app = App::new();
        app.add_systems(Update, heiwushi_shadow_transform_action_system);

        let boss = spawn_boss(app.world_mut(), 400.0, 3.0);
        // 先推入 Phase3
        app.world_mut()
            .get_mut::<HeiwushiState>(boss)
            .unwrap()
            .apply_phase3_transform();
        let attack_after_first_transform =
            app.world().get::<HeiwushiState>(boss).unwrap().base_attack;

        // 再触发一次 transform action
        app.world_mut().spawn((
            Actor(boss),
            ActionState::Requested,
            HeiwushiShadowTransformAction,
        ));
        app.update();
        app.update();

        let state = app.world().get::<HeiwushiState>(boss).unwrap();
        assert_eq!(
            state.phase,
            HeiwushiPhase::Phase3,
            "已是 Phase3：再次 transform 后 phase 应仍为 Phase3，实际 {:?}",
            state.phase
        );
        assert_eq!(
            state.base_attack, attack_after_first_transform,
            "已是 Phase3：base_attack 不应再翻倍，期望 {}，实际 {}",
            attack_after_first_transform, state.base_attack
        );
    }

    // ── (已有测试保持不变) ─────────────────────────────────────────────────

    #[test]
    fn spawn_helper_uses_marker_kind_and_fauna_drop_tag() {
        let mut app = App::new();
        let layer = app.world_mut().spawn_empty().id();
        let mut commands = app.world_mut().commands();
        let boss = spawn_heiwushi_at(
            &mut commands,
            layer,
            "giant_sword_sea",
            DVec3::new(1.0, 64.0, 2.0),
            DVec3::ZERO,
        );
        app.world_mut().flush();

        assert!(app.world().get::<HeiwushiMarker>(boss).is_some());
        assert_eq!(
            app.world()
                .get::<valence::prelude::EntityKind>(boss)
                .unwrap()
                .get(),
            145
        );
        assert_eq!(
            app.world().get::<FaunaTag>(boss).unwrap().beast_kind,
            BeastKind::Heiwushi
        );
        assert_eq!(
            app.world().get::<Wounds>(boss).unwrap().health_current,
            HEIWUSHI_HEALTH_MAX
        );
    }
}
