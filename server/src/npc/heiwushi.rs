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
    bevy_ecs, App, Commands, Component, DVec3, Entity, EntityLayerId, Event, EventWriter,
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
/// 剑意召唤 base CD（8 秒 × 20 TPS = 160 ticks）。
pub const SWORD_INTENT_BASE_CD: u32 = 160;
pub const MELEE_RANGE: f32 = 3.0;
pub const BARRAGE_MIN_RANGE: f32 = 4.0;
pub const BARRAGE_MAX_RANGE: f32 = 8.0;
pub const VORTEX_RANGE: f32 = 6.0;
/// 追击 scorer 距离区间：16~32 格触发（拉近但不贴身）。
pub const CHASE_MIN_RANGE: f32 = 16.0;
pub const CHASE_MAX_RANGE: f32 = 32.0;
/// 剑意 scorer 距离区间：4~20 格（中距离触发）。
pub const SWORD_INTENT_MIN_RANGE: f32 = 4.0;
pub const SWORD_INTENT_MAX_RANGE: f32 = 20.0;
/// 撤退触发血量阈值（HP < 30%）。
pub const RETREAT_HP_THRESHOLD: f32 = 0.30;
/// 撤退触发玩家距离阈值（玩家≤5 格才撤）。
pub const RETREAT_PLAYER_RANGE: f32 = 5.0;
const THINKER_THRESHOLD: f32 = 0.3;

// ─── 黑武士 boss action VFX 事件（plan-sword-path-complete §B）──────────────

/// 黑武士 boss 成功发招那帧 emit 此事件；heiwushi_av_trigger 消费并转化为
/// VfxEventRequest + PlaySoundRecipeRequest 发往 client。
#[derive(Debug, Clone, Event)]
pub struct HeiwushiActionVfxEvent {
    pub boss: Entity,
    pub kind: HeiwushiActionKind,
    /// 发招时 boss 所在世界坐标（粒子/音效 origin）。
    pub origin: DVec3,
    /// 招式朝向（boss→target 方向单位向量）；None 表示无方向（全向型招式）。
    pub direction: Option<DVec3>,
    pub tick: u64,
}

/// 黑武士 boss 招式类型枚举（§B 六招对齐 audio recipe + particle event_id）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeiwushiActionKind {
    MeleeSlash,
    DarkBarrage,
    DarkVortex,
    ShadowTransform,
    Death,
    SwordIntent,
}

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
    /// 剑意召唤当前冷却（0 = 可用）。
    pub sword_intent: u32,
    pub base_melee_slash: u32,
    pub base_dark_barrage: u32,
    pub base_dark_vortex: u32,
    /// 剑意 base CD = SWORD_INTENT_BASE_CD (160)。
    pub base_sword_intent: u32,
}

impl Default for HeiwushiCooldowns {
    fn default() -> Self {
        Self {
            melee_slash: 0,
            dark_barrage: 0,
            dark_vortex: 0,
            shadow_transform: 0,
            sword_intent: 0,
            base_melee_slash: MELEE_BASE_CD,
            base_dark_barrage: BARRAGE_BASE_CD,
            base_dark_vortex: VORTEX_BASE_CD,
            base_sword_intent: SWORD_INTENT_BASE_CD,
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
        self.sword_intent = self.sword_intent.saturating_sub(1);
    }

    fn reset_ready(&mut self) {
        self.melee_slash = 0;
        self.dark_barrage = 0;
        self.dark_vortex = 0;
        self.shadow_transform = 0;
        self.sword_intent = 0;
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
/// 追击：玩家 16~32 格 → 0.5。
#[derive(Clone, Copy, Debug, Component)]
pub struct HeiwushiChaseScorer;
/// 撤退：HP<30% && 玩家≤5 格 → 0.9。
#[derive(Clone, Copy, Debug, Component)]
pub struct HeiwushiRetreatScorer;
/// 剑意召唤：玩家 4~20 格 && CD==0 → 0.65。
#[derive(Clone, Copy, Debug, Component)]
pub struct HeiwushiSwordIntentScorer;
scorer_builder!(HeiwushiChaseScorer, "HeiwushiChaseScorer");
scorer_builder!(HeiwushiRetreatScorer, "HeiwushiRetreatScorer");
scorer_builder!(HeiwushiSwordIntentScorer, "HeiwushiSwordIntentScorer");
action_builder!(HeiwushiIdleAction, "HeiwushiIdleAction");
action_builder!(HeiwushiMeleeSlashAction, "HeiwushiMeleeSlashAction");
action_builder!(HeiwushiDarkBarrageAction, "HeiwushiDarkBarrageAction");
action_builder!(HeiwushiDarkVortexAction, "HeiwushiDarkVortexAction");
action_builder!(
    HeiwushiShadowTransformAction,
    "HeiwushiShadowTransformAction"
);
action_builder!(HeiwushiDeathAction, "HeiwushiDeathAction");
#[derive(Clone, Copy, Debug, Component)]
pub struct HeiwushiChaseAction;
#[derive(Clone, Copy, Debug, Component)]
pub struct HeiwushiRetreatAction;
#[derive(Clone, Copy, Debug, Component)]
pub struct HeiwushiSwordIntentAction;
action_builder!(HeiwushiChaseAction, "HeiwushiChaseAction");
action_builder!(HeiwushiRetreatAction, "HeiwushiRetreatAction");
action_builder!(HeiwushiSwordIntentAction, "HeiwushiSwordIntentAction");

pub fn register(app: &mut App) {
    app.add_event::<HeiwushiActionVfxEvent>()
        .add_systems(
            PreUpdate,
            heiwushi_cooldown_tick_system.before(BigBrainSet::Actions),
        )
        .add_systems(
            PreUpdate,
            (
                heiwushi_death_scorer_system,
                heiwushi_transform_scorer_system,
                heiwushi_vortex_scorer_system,
                heiwushi_barrage_scorer_system,
                heiwushi_sword_intent_scorer_system,
                heiwushi_melee_scorer_system,
                heiwushi_chase_scorer_system,
                heiwushi_retreat_scorer_system,
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
                heiwushi_chase_action_system,
                heiwushi_retreat_action_system,
                heiwushi_sword_intent_action_system,
            )
                .in_set(BigBrainSet::Actions),
        )
        .add_systems(
            Update,
            (heiwushi_growth_tick_system, heiwushi_phase_sync_system),
        );
}

pub fn heiwushi_thinker() -> ThinkerBuilder {
    // 优先级（高→低）：Death(1.0) → Transform(0.95) → Retreat(0.9) → Vortex(0.8) →
    // Barrage(0.7) → SwordIntent(0.65) → Melee(0.6) → Chase(0.5) → otherwise Idle
    Thinker::build()
        .picker(FirstToScore {
            threshold: THINKER_THRESHOLD,
        })
        .when(HeiwushiDeathScorer, HeiwushiDeathAction)
        .when(HeiwushiTransformScorer, HeiwushiShadowTransformAction)
        .when(HeiwushiRetreatScorer, HeiwushiRetreatAction)
        .when(HeiwushiVortexScorer, HeiwushiDarkVortexAction)
        .when(HeiwushiBarrageScorer, HeiwushiDarkBarrageAction)
        .when(HeiwushiSwordIntentScorer, HeiwushiSwordIntentAction)
        .when(HeiwushiMeleeScorer, HeiwushiMeleeSlashAction)
        .when(HeiwushiChaseScorer, HeiwushiChaseAction)
        .otherwise(HeiwushiIdleAction)
}

pub fn spawn_heiwushi_at(
    commands: &mut Commands,
    layer: Entity,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_center: DVec3,
    current_tick: u64,
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
            HeiwushiState {
                last_cycle_tick: current_tick,
                ..Default::default()
            },
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
    mut bosses: Query<
        (
            &NpcBlackboard,
            &mut HeiwushiState,
            &mut Navigator,
            &Position,
        ),
        With<HeiwushiMarker>,
    >,
    all_positions: Query<&Position>,
    mut attacks: EventWriter<AttackIntent>,
    mut vfx_events: EventWriter<HeiwushiActionVfxEvent>,
    tick: Option<Res<GameTick>>,
    mut actions: Query<(&Actor, &mut ActionState), With<HeiwushiMeleeSlashAction>>,
) {
    let now = tick.map(|t| u64::from(t.0)).unwrap_or(0);
    for (Actor(actor), mut action_state) in &mut actions {
        let Ok((bb, mut state, mut navigator, boss_pos)) = bosses.get_mut(*actor) else {
            *action_state = ActionState::Failure;
            continue;
        };
        let origin = boss_pos.get();
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
                let target_pos = all_positions.get(target).map(|p| p.get()).unwrap_or(origin);
                let raw = target_pos - origin;
                let dir = if raw.length_squared() > f64::EPSILON {
                    Some(raw.normalize())
                } else {
                    Some(DVec3::X)
                };
                vfx_events.send(HeiwushiActionVfxEvent {
                    boss: *actor,
                    kind: HeiwushiActionKind::MeleeSlash,
                    origin,
                    direction: dir,
                    tick: now,
                });
                *action_state = ActionState::Success;
            }
            ActionState::Cancelled => *action_state = ActionState::Failure,
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn heiwushi_dark_barrage_action_system(
    mut bosses: Query<(&NpcBlackboard, &mut HeiwushiState, &Position), With<HeiwushiMarker>>,
    all_positions: Query<&Position>,
    mut attacks: EventWriter<AttackIntent>,
    mut vfx_events: EventWriter<HeiwushiActionVfxEvent>,
    tick: Option<Res<GameTick>>,
    mut actions: Query<(&Actor, &mut ActionState), With<HeiwushiDarkBarrageAction>>,
) {
    let now = tick.map(|t| u64::from(t.0)).unwrap_or(0);
    for (Actor(actor), mut action_state) in &mut actions {
        let Ok((bb, mut state, boss_pos)) = bosses.get_mut(*actor) else {
            *action_state = ActionState::Failure;
            continue;
        };
        let origin = boss_pos.get();
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
                let target_pos = all_positions.get(target).map(|p| p.get()).unwrap_or(origin);
                let raw = target_pos - origin;
                let dir = if raw.length_squared() > f64::EPSILON {
                    Some(raw.normalize())
                } else {
                    Some(DVec3::X)
                };
                vfx_events.send(HeiwushiActionVfxEvent {
                    boss: *actor,
                    kind: HeiwushiActionKind::DarkBarrage,
                    origin,
                    direction: dir,
                    tick: now,
                });
                *action_state = ActionState::Success;
            }
            ActionState::Cancelled => *action_state = ActionState::Failure,
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn heiwushi_dark_vortex_action_system(
    mut bosses: Query<(&NpcBlackboard, &mut HeiwushiState, &Position), With<HeiwushiMarker>>,
    mut attacks: EventWriter<AttackIntent>,
    mut statuses: EventWriter<ApplyStatusEffectIntent>,
    mut vfx_events: EventWriter<HeiwushiActionVfxEvent>,
    tick: Option<Res<GameTick>>,
    mut actions: Query<(&Actor, &mut ActionState), With<HeiwushiDarkVortexAction>>,
) {
    let now = tick.map(|t| u64::from(t.0)).unwrap_or(0);
    for (Actor(actor), mut action_state) in &mut actions {
        let Ok((bb, mut state, boss_pos)) = bosses.get_mut(*actor) else {
            *action_state = ActionState::Failure;
            continue;
        };
        let origin = boss_pos.get();
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
                vfx_events.send(HeiwushiActionVfxEvent {
                    boss: *actor,
                    kind: HeiwushiActionKind::DarkVortex,
                    origin,
                    direction: None,
                    tick: now,
                });
                *action_state = ActionState::Success;
            }
            ActionState::Cancelled => *action_state = ActionState::Failure,
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn heiwushi_shadow_transform_action_system(
    mut bosses: Query<(&mut HeiwushiState, &Position), With<HeiwushiMarker>>,
    mut vfx_events: EventWriter<HeiwushiActionVfxEvent>,
    tick: Option<Res<GameTick>>,
    mut actions: Query<(&Actor, &mut ActionState), With<HeiwushiShadowTransformAction>>,
) {
    let now = tick.map(|t| u64::from(t.0)).unwrap_or(0);
    for (Actor(actor), mut action_state) in &mut actions {
        let Ok((mut state, boss_pos)) = bosses.get_mut(*actor) else {
            *action_state = ActionState::Failure;
            continue;
        };
        let origin = boss_pos.get();
        match *action_state {
            ActionState::Requested => *action_state = ActionState::Executing,
            ActionState::Executing => {
                state.apply_phase3_transform();
                vfx_events.send(HeiwushiActionVfxEvent {
                    boss: *actor,
                    kind: HeiwushiActionKind::ShadowTransform,
                    origin,
                    direction: None,
                    tick: now,
                });
                *action_state = ActionState::Success;
            }
            ActionState::Cancelled => *action_state = ActionState::Failure,
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn heiwushi_death_action_system(
    mut bosses: Query<(&mut Lifecycle, &mut Navigator, &Position), With<HeiwushiMarker>>,
    mut vfx_events: EventWriter<HeiwushiActionVfxEvent>,
    tick: Option<Res<GameTick>>,
    mut actions: Query<(&Actor, &mut ActionState), With<HeiwushiDeathAction>>,
) {
    let now = tick.map(|t| u64::from(t.0)).unwrap_or(0);
    for (Actor(actor), mut action_state) in &mut actions {
        let Ok((mut lifecycle, mut navigator, boss_pos)) = bosses.get_mut(*actor) else {
            *action_state = ActionState::Failure;
            continue;
        };
        let origin = boss_pos.get();
        match *action_state {
            ActionState::Requested => *action_state = ActionState::Executing,
            ActionState::Executing => {
                navigator.stop();
                // 仅在首次转入 Terminated 时 emit Death AV——否则尸体停留期间
                // Death scorer 反复选中本 action 会重复播死亡特效/音效。
                if lifecycle.state != LifecycleState::Terminated {
                    lifecycle.state = LifecycleState::Terminated;
                    vfx_events.send(HeiwushiActionVfxEvent {
                        boss: *actor,
                        kind: HeiwushiActionKind::Death,
                        origin,
                        direction: None,
                        tick: now,
                    });
                }
                *action_state = ActionState::Success;
            }
            ActionState::Cancelled => *action_state = ActionState::Failure,
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

// ─── §B 新增 Scorer/Action 系统 ─────────────────────────────────────────────

fn heiwushi_chase_scorer_system(
    bosses: Query<&NpcBlackboard, With<HeiwushiMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<HeiwushiChaseScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = bosses
            .get(*actor)
            .map(|bb| {
                if (CHASE_MIN_RANGE..=CHASE_MAX_RANGE).contains(&bb.player_distance) {
                    0.5
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        score.set(value);
    }
}

fn heiwushi_retreat_scorer_system(
    bosses: Query<(&Wounds, &NpcBlackboard), With<HeiwushiMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<HeiwushiRetreatScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = bosses
            .get(*actor)
            .map(|(wounds, bb)| {
                let hp_ratio = wounds.health_current / wounds.health_max.max(1.0);
                if hp_ratio < RETREAT_HP_THRESHOLD && bb.player_distance <= RETREAT_PLAYER_RANGE {
                    0.9
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        score.set(value);
    }
}

fn heiwushi_sword_intent_scorer_system(
    bosses: Query<(&NpcBlackboard, &HeiwushiState), With<HeiwushiMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<HeiwushiSwordIntentScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = bosses
            .get(*actor)
            .map(|(bb, state)| {
                if (SWORD_INTENT_MIN_RANGE..=SWORD_INTENT_MAX_RANGE).contains(&bb.player_distance)
                    && state.skill_cooldowns.sword_intent == 0
                {
                    0.65
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        score.set(value);
    }
}

fn heiwushi_chase_action_system(
    mut bosses: Query<(&NpcBlackboard, &mut Navigator, &Position), With<HeiwushiMarker>>,
    all_positions: Query<&Position>,
    mut actions: Query<(&Actor, &mut ActionState), With<HeiwushiChaseAction>>,
) {
    for (Actor(actor), mut action_state) in &mut actions {
        let Ok((bb, mut navigator, boss_pos)) = bosses.get_mut(*actor) else {
            *action_state = ActionState::Failure;
            continue;
        };
        match *action_state {
            ActionState::Requested => *action_state = ActionState::Executing,
            ActionState::Executing => {
                let Some(target) = bb.nearest_player else {
                    navigator.stop();
                    *action_state = ActionState::Failure;
                    continue;
                };
                let target_pos = all_positions
                    .get(target)
                    .map(|p| p.get())
                    .unwrap_or(boss_pos.get());
                navigator.set_goal(target_pos, 1.0);
                // Chase 持续追击直到 scorer 降分（Failure 放弃本帧，下帧重评）。
                *action_state = ActionState::Success;
            }
            ActionState::Cancelled => {
                navigator.stop();
                *action_state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn heiwushi_retreat_action_system(
    mut bosses: Query<(&NpcBlackboard, &mut Navigator, &Position), With<HeiwushiMarker>>,
    all_positions: Query<&Position>,
    mut actions: Query<(&Actor, &mut ActionState), With<HeiwushiRetreatAction>>,
) {
    for (Actor(actor), mut action_state) in &mut actions {
        let Ok((bb, mut navigator, boss_pos)) = bosses.get_mut(*actor) else {
            *action_state = ActionState::Failure;
            continue;
        };
        match *action_state {
            ActionState::Requested => *action_state = ActionState::Executing,
            ActionState::Executing => {
                let origin = boss_pos.get();
                let away_dir = match bb.nearest_player {
                    Some(target) => {
                        let target_pos =
                            all_positions.get(target).map(|p| p.get()).unwrap_or(origin);
                        let raw = origin - target_pos; // 背向玩家
                        if raw.length_squared() > f64::EPSILON {
                            raw.normalize()
                        } else {
                            DVec3::X
                        }
                    }
                    None => DVec3::X,
                };
                // 撤退 10 格
                let retreat_target = origin + away_dir * 10.0;
                navigator.set_goal(retreat_target, 1.0);
                *action_state = ActionState::Success;
            }
            ActionState::Cancelled => {
                navigator.stop();
                *action_state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn heiwushi_sword_intent_action_system(
    mut bosses: Query<(&NpcBlackboard, &mut HeiwushiState, &Position), With<HeiwushiMarker>>,
    all_positions: Query<&Position>,
    mut vfx_events: EventWriter<HeiwushiActionVfxEvent>,
    mut commands: Commands,
    tick: Option<Res<GameTick>>,
    mut actions: Query<(&Actor, &mut ActionState), With<HeiwushiSwordIntentAction>>,
) {
    let now = tick.map(|t| u64::from(t.0)).unwrap_or(0);
    for (Actor(actor), mut action_state) in &mut actions {
        let Ok((bb, mut state, boss_pos)) = bosses.get_mut(*actor) else {
            *action_state = ActionState::Failure;
            continue;
        };
        let origin = boss_pos.get();
        match *action_state {
            ActionState::Requested => *action_state = ActionState::Executing,
            ActionState::Executing => {
                let Some(target) = bb.nearest_player else {
                    *action_state = ActionState::Failure;
                    continue;
                };
                // 召唤剑意追踪实体（复用剑道化形追踪逻辑，伤害 = base_attack * 0.8）。
                let damage = f64::from(state.base_attack) * 0.8;
                crate::sword_path::sword_intent_entity::spawn_sword_intent(
                    &mut commands,
                    *actor,
                    origin,
                    Some(target),
                    damage,
                    crate::combat::events::AttackSource::SwordPathManifest,
                );
                state.skill_cooldowns.sword_intent = state.skill_cooldowns.base_sword_intent;
                // 方向 boss→target。
                let target_pos = all_positions.get(target).map(|p| p.get()).unwrap_or(origin);
                let raw = target_pos - origin;
                let dir = if raw.length_squared() > f64::EPSILON {
                    Some(raw.normalize())
                } else {
                    Some(DVec3::X)
                };
                vfx_events.send(HeiwushiActionVfxEvent {
                    boss: *actor,
                    kind: HeiwushiActionKind::SwordIntent,
                    origin,
                    direction: dir,
                    tick: now,
                });
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
        app.add_event::<HeiwushiActionVfxEvent>();
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
        app.add_event::<HeiwushiActionVfxEvent>();
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
        //   3. emit HeiwushiActionVfxEvent { kind: Death }
        let mut app = App::new();
        app.add_event::<HeiwushiActionVfxEvent>();
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
        app.add_event::<HeiwushiActionVfxEvent>();
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
        app.add_event::<HeiwushiActionVfxEvent>();
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
        app.add_event::<HeiwushiActionVfxEvent>();
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
            0,
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

    // ── FIX 1 regression: 新生 boss 不得因服务器 uptime 白拿 growth cycle ─────

    #[test]
    fn spawn_at_tick_1000_no_free_growth_cycle_on_first_update() {
        // 服务器已运行 1000 tick，刚 spawn 的 boss last_cycle_tick=1000；
        // 第一次 Update 时 tick 仍为 1000（或 1001 等 <1000+600），不应触发增长。
        let mut app = App::new();
        app.insert_resource(GameTick(1000));
        app.add_systems(Update, heiwushi_growth_tick_system);
        // spawn with current tick = 1000
        let boss = app
            .world_mut()
            .spawn((
                HeiwushiMarker,
                HeiwushiState {
                    last_cycle_tick: 1000,
                    ..Default::default()
                },
                NpcBlackboard::default(),
                Wounds {
                    entries: Vec::new(),
                    health_current: HEIWUSHI_HEALTH_MAX,
                    health_max: HEIWUSHI_HEALTH_MAX,
                },
                Lifecycle::default(),
                Navigator::new(),
                NpcPatrol::new("giant_sword_sea", DVec3::ZERO),
                Position::new([0.0, 64.0, 0.0]),
            ))
            .id();

        // First update at tick=1000: delta = 1000-1000 = 0 < 600, no cycle
        app.update();
        assert_eq!(
            app.world().get::<HeiwushiState>(boss).unwrap().growth_cycles,
            0,
            "tick=1000 spawn, first update at tick=1000: growth_cycles 期望 0（不应白拿 cycle），实际 {}",
            app.world().get::<HeiwushiState>(boss).unwrap().growth_cycles
        );

        // Advance to tick=1000+599: still below threshold
        app.world_mut().resource_mut::<GameTick>().0 = 1599;
        app.update();
        assert_eq!(
            app.world()
                .get::<HeiwushiState>(boss)
                .unwrap()
                .growth_cycles,
            0,
            "tick=1599 (delta=599 < 600): growth_cycles 期望仍为 0，实际 {}",
            app.world()
                .get::<HeiwushiState>(boss)
                .unwrap()
                .growth_cycles
        );

        // Advance to tick=1600 = 1000+600: first legitimate cycle
        app.world_mut().resource_mut::<GameTick>().0 = 1600;
        app.update();
        assert_eq!(
            app.world()
                .get::<HeiwushiState>(boss)
                .unwrap()
                .growth_cycles,
            1,
            "tick=1600 (delta=600 >= 600): growth_cycles 期望 1（首次合法增长），实际 {}",
            app.world()
                .get::<HeiwushiState>(boss)
                .unwrap()
                .growth_cycles
        );
    }

    // ── FIX 2 regression: action 写入冷却当帧不得被立即扣 1 ──────────────────

    #[test]
    fn cooldown_set_by_action_not_decremented_same_frame() {
        // 验证系统顺序契约：cooldown_tick 先跑（递减旧值），action 后写入新冷却值。
        // 测试使用 .chain()（cooldown_tick → melee_slash_action）模拟正确的调度顺序，
        // 确保 action 写入 effective_cd(40,1)=34 后，该值在 *当帧* 不被再次递减。
        //
        // 行为时序（3 次 update）：
        //   update 1: tick_down(0→0 saturate), action Requested→Executing (不写 CD)
        //   update 2: tick_down(0→0 saturate), action Executing→fires, 写 melee=34, →Success
        //   update 3: tick_down(34→33), action 已 Success 不再执行, melee=33
        let mut app = App::new();
        app.add_event::<AttackIntent>();
        app.add_event::<HeiwushiActionVfxEvent>();
        app.insert_resource(GameTick(42));
        // 顺序：cooldown_tick 先跑，melee_slash_action 后跑（模拟 PreUpdate 中 cooldown.before(Actions)）
        app.add_systems(
            Update,
            (
                heiwushi_cooldown_tick_system,
                heiwushi_melee_slash_action_system,
            )
                .chain(),
        );

        let boss = app
            .world_mut()
            .spawn((
                HeiwushiMarker,
                HeiwushiState {
                    growth_cycles: 1, // effective_cd(40, 1) = 34
                    ..Default::default()
                },
                NpcBlackboard {
                    nearest_player: Some(Entity::PLACEHOLDER),
                    player_distance: 2.0,
                    ..Default::default()
                },
                Wounds {
                    entries: Vec::new(),
                    health_current: HEIWUSHI_HEALTH_MAX,
                    health_max: HEIWUSHI_HEALTH_MAX,
                },
                Lifecycle::default(),
                Navigator::new(),
                NpcPatrol::new("giant_sword_sea", DVec3::ZERO),
                Position::new([0.0, 64.0, 0.0]),
            ))
            .id();

        app.world_mut().spawn((
            Actor(boss),
            ActionState::Requested,
            HeiwushiMeleeSlashAction,
        ));

        // update 1: tick_down(0→0 sat), action Requested→Executing
        app.update();
        // update 2: tick_down(0→0 sat), action Executing→fires, writes melee=34 → Success
        app.update();

        let cd_after_action = app
            .world()
            .get::<HeiwushiState>(boss)
            .unwrap()
            .skill_cooldowns
            .melee_slash;
        assert_eq!(
            cd_after_action, 34,
            "action 写入后 melee_slash 期望 34（effective_cd(40,1)），\
            若为 33 说明 tick_down 在 action 之后跑了（off-by-one bug），实际 {}",
            cd_after_action
        );

        // update 3: tick_down(34→33), action 已 Success 不再写 CD
        app.update();
        let cd_after_tick = app
            .world()
            .get::<HeiwushiState>(boss)
            .unwrap()
            .skill_cooldowns
            .melee_slash;
        assert_eq!(
            cd_after_tick, 33,
            "第三次 update（tick_down 后无 action）melee_slash 期望 33，实际 {}",
            cd_after_tick
        );
    }

    // ─── §B 新增 Scorer/Action 测试 ───────────────────────────────────────────

    /// §B — Chase scorer：玩家在 16~32 格 → 0.5；范围外 → 0.0。
    #[test]
    fn chase_scorer_fires_in_range_and_zero_outside() {
        let mut app = App::new();
        app.add_systems(Update, heiwushi_chase_scorer_system);

        // 距离 20（in range 16~32）
        let boss = spawn_boss(app.world_mut(), HEIWUSHI_HEALTH_MAX, 20.0);
        let scorer = app
            .world_mut()
            .spawn((Actor(boss), Score::default(), HeiwushiChaseScorer))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.5,
            "距离 20 在 16~32 范围内，Chase scorer 期望 0.5"
        );

        // 距离 5（out of range）
        app.world_mut()
            .get_mut::<NpcBlackboard>(boss)
            .unwrap()
            .player_distance = 5.0;
        app.update();
        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "距离 5 不在 16~32 范围内，Chase scorer 期望 0.0"
        );

        // 距离 40（beyond 32）
        app.world_mut()
            .get_mut::<NpcBlackboard>(boss)
            .unwrap()
            .player_distance = 40.0;
        app.update();
        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "距离 40 超出 32，Chase scorer 期望 0.0"
        );
    }

    /// §B — Retreat scorer：HP<30% && player≤5 → 0.9；否则 → 0.0。
    #[test]
    fn retreat_scorer_fires_when_low_hp_and_player_close() {
        let mut app = App::new();
        app.add_systems(Update, heiwushi_retreat_scorer_system);

        // HP < 30%, distance = 4 → 0.9
        let boss = spawn_boss(app.world_mut(), 500.0, 4.0); // 500/2100 ≈ 23.8% < 30%
        let scorer = app
            .world_mut()
            .spawn((Actor(boss), Score::default(), HeiwushiRetreatScorer))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.9,
            "HP<30% && player<=5 时 Retreat scorer 期望 0.9"
        );

        // HP < 30%, distance = 10 → 0.0（太远）
        app.world_mut()
            .get_mut::<NpcBlackboard>(boss)
            .unwrap()
            .player_distance = 10.0;
        app.update();
        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "HP<30% 但 distance>5 时 Retreat scorer 期望 0.0"
        );

        // HP = 70%, distance = 3 → 0.0（血量不低）
        app.world_mut()
            .get_mut::<Wounds>(boss)
            .unwrap()
            .health_current = 1500.0;
        app.world_mut()
            .get_mut::<NpcBlackboard>(boss)
            .unwrap()
            .player_distance = 3.0;
        app.update();
        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "HP≥30% 时 Retreat scorer 期望 0.0"
        );
    }

    /// §B — SwordIntent scorer：玩家 4~20 格 && CD==0 → 0.65。
    #[test]
    fn sword_intent_scorer_fires_in_range_and_cd_zero() {
        let mut app = App::new();
        app.add_systems(Update, heiwushi_sword_intent_scorer_system);

        // 距离 10, CD=0 → 0.65
        let boss = spawn_boss(app.world_mut(), HEIWUSHI_HEALTH_MAX, 10.0);
        let scorer = app
            .world_mut()
            .spawn((Actor(boss), Score::default(), HeiwushiSwordIntentScorer))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.65,
            "距离 10 在 4~20 范围 && CD=0，SwordIntent scorer 期望 0.65"
        );

        // CD > 0 → 0.0
        app.world_mut()
            .get_mut::<HeiwushiState>(boss)
            .unwrap()
            .skill_cooldowns
            .sword_intent = 50;
        app.update();
        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "CD>0 时 SwordIntent scorer 期望 0.0"
        );

        // 距离 25（beyond 20）, CD=0 → 0.0
        app.world_mut()
            .get_mut::<HeiwushiState>(boss)
            .unwrap()
            .skill_cooldowns
            .sword_intent = 0;
        app.world_mut()
            .get_mut::<NpcBlackboard>(boss)
            .unwrap()
            .player_distance = 25.0;
        app.update();
        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "距离 25 超出 20 时 SwordIntent scorer 期望 0.0"
        );
    }

    /// §B — Death action 现在 emit HeiwushiActionVfxEvent{kind: Death}。
    #[test]
    fn death_action_emits_death_vfx_event() {
        let mut app = App::new();
        app.add_event::<HeiwushiActionVfxEvent>();
        app.add_systems(Update, heiwushi_death_action_system);
        let boss = spawn_boss(app.world_mut(), 0.0, 3.0);
        app.world_mut()
            .spawn((Actor(boss), ActionState::Requested, HeiwushiDeathAction));
        app.update();
        app.update();

        let events: Vec<_> = app
            .world()
            .resource::<Events<HeiwushiActionVfxEvent>>()
            .iter_current_update_events()
            .collect();
        assert_eq!(events.len(), 1, "death action 应 emit 1 个 VFX 事件");
        assert_eq!(events[0].kind, HeiwushiActionKind::Death);
        assert_eq!(events[0].direction, None, "Death VFX dir 应为 None");
    }

    /// §B — SwordIntent cooldown 字段默认值 = SWORD_INTENT_BASE_CD (160)，tick_down 正常递减。
    #[test]
    fn sword_intent_cooldown_default_and_tick_down() {
        let mut cd = HeiwushiCooldowns::default();
        assert_eq!(
            cd.base_sword_intent, SWORD_INTENT_BASE_CD,
            "base_sword_intent 默认值应为 {SWORD_INTENT_BASE_CD}"
        );
        assert_eq!(cd.sword_intent, 0, "sword_intent 初始应为 0（即刻可用）");

        cd.sword_intent = 5;
        cd.tick_down();
        assert_eq!(cd.sword_intent, 4, "tick_down 后 sword_intent 应减 1");

        cd.sword_intent = 0;
        cd.tick_down();
        assert_eq!(
            cd.sword_intent, 0,
            "sword_intent=0 时 tick_down 饱和减法 → 仍为 0"
        );
    }

    /// §B — reset_ready 同时清零 sword_intent。
    #[test]
    fn reset_ready_clears_sword_intent() {
        let mut cd = HeiwushiCooldowns {
            sword_intent: 100,
            ..Default::default()
        };
        cd.reset_ready();
        assert_eq!(cd.sword_intent, 0, "reset_ready 应将 sword_intent 归零");
    }

    /// §B — 追击 action 成功后 ActionState 变 Success。
    #[test]
    fn chase_action_sets_navigator_goal_and_succeeds() {
        let mut app = App::new();
        app.add_systems(Update, heiwushi_chase_action_system);
        let boss = spawn_boss(app.world_mut(), HEIWUSHI_HEALTH_MAX, 20.0);
        // 真实带 Position 的玩家目标：boss 在 (0,64,0)，玩家在 (50,64,0)。
        // 若 nearest_player 是没有 Position 的 PLACEHOLDER（spawn_boss 默认），chase
        // 会 unwrap_or(boss_pos) 退化成"原地 goal"，!is_idle 仍然为真——测试就锁不住
        // "真的朝玩家走"（CodeRabbit #784 指出的盲点）。这里用真实目标关掉这条退路。
        let player = app.world_mut().spawn(Position::new([50.0, 64.0, 0.0])).id();
        app.world_mut()
            .get_mut::<NpcBlackboard>(boss)
            .unwrap()
            .nearest_player = Some(player);
        app.world_mut()
            .spawn((Actor(boss), ActionState::Requested, HeiwushiChaseAction));
        app.update();
        app.update();
        // 两次 update 后应达到 Success（Requested→Executing→Success）
        // Navigator 应有 goal（不 idle）
        let nav = app.world().get::<Navigator>(boss).unwrap();
        assert!(
            !nav.is_idle(),
            "chase action 执行后 navigator 应有 goal（非 idle）"
        );
        let goal = nav
            .current_goal_for_test()
            .expect("chase 执行后 navigator 必须有 goal");
        // goal 必须指向玩家 (50,64,0)，证明真追玩家而非退化到 boss 原地。
        assert!(
            goal.destination.distance(DVec3::new(50.0, 64.0, 0.0)) < 1e-6,
            "chase goal 应指向玩家坐标 (50,64,0)，实际 {:?}（若为 (0,64,0) 说明退化到 boss 原地 fallback）",
            goal.destination
        );
        assert!(
            goal.destination.distance(DVec3::new(0.0, 64.0, 0.0)) > 1.0,
            "chase goal 不应等于 boss 原地 (0,64,0)（那是 target 无 Position 时的退化分支），实际 {:?}",
            goal.destination
        );
    }

    /// §C — SwordIntent action happy path：spawn 剑意追踪实体 + 写 sword_intent 冷却
    /// + emit HeiwushiActionVfxEvent{kind: SwordIntent}。
    #[test]
    fn sword_intent_action_spawns_intent_writes_cooldown_and_emits_vfx() {
        use crate::sword_path::sword_intent_entity::SwordIntentEntity;
        let mut app = App::new();
        app.add_event::<HeiwushiActionVfxEvent>();
        app.insert_resource(GameTick(123));
        app.add_systems(Update, heiwushi_sword_intent_action_system);
        let boss = spawn_boss(app.world_mut(), HEIWUSHI_HEALTH_MAX, 10.0);
        // 真实带 Position 的目标（10 格外），用于锁 SwordIntentEntity.target 与方向计算。
        let player = app.world_mut().spawn(Position::new([10.0, 64.0, 0.0])).id();
        app.world_mut()
            .get_mut::<NpcBlackboard>(boss)
            .unwrap()
            .nearest_player = Some(player);
        app.world_mut().spawn((
            Actor(boss),
            ActionState::Requested,
            HeiwushiSwordIntentAction,
        ));

        app.update(); // Requested → Executing
        app.update(); // Executing → spawn + cooldown + vfx；Commands flush

        // ① spawn 恰好 1 个 SwordIntentEntity，owner = boss、target = 玩家
        let mut q = app.world_mut().query::<&SwordIntentEntity>();
        let intents: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(
            intents.len(),
            1,
            "SwordIntent action 应 spawn 恰好 1 个剑意追踪实体，实际 {}",
            intents.len()
        );
        assert_eq!(
            intents[0].owner, boss,
            "剑意实体 owner 应为发招 boss {boss:?}，实际 {:?}",
            intents[0].owner
        );
        assert_eq!(
            intents[0].target,
            Some(player),
            "剑意实体 target 应为 nearest_player {player:?}，实际 {:?}",
            intents[0].target
        );

        // ② 写了 sword_intent 冷却（置为 base_sword_intent = SWORD_INTENT_BASE_CD）
        let cd = app
            .world()
            .get::<HeiwushiState>(boss)
            .unwrap()
            .skill_cooldowns
            .sword_intent;
        assert_eq!(
            cd, SWORD_INTENT_BASE_CD,
            "发招后 sword_intent 冷却应置为 base_sword_intent={SWORD_INTENT_BASE_CD}，实际 {cd}"
        );

        // ③ emit 了 HeiwushiActionVfxEvent{kind: SwordIntent, boss, tick}
        let vfx = app.world().resource::<Events<HeiwushiActionVfxEvent>>();
        let mut reader = vfx.get_reader();
        let events: Vec<_> = reader.read(vfx).collect();
        assert_eq!(
            events.len(),
            1,
            "SwordIntent action 应 emit 恰好 1 个 HeiwushiActionVfxEvent，实际 {}",
            events.len()
        );
        assert_eq!(
            events[0].kind,
            HeiwushiActionKind::SwordIntent,
            "VFX 事件 kind 应为 SwordIntent，实际 {:?}",
            events[0].kind
        );
        assert_eq!(
            events[0].boss, boss,
            "VFX 事件 boss 应为发招 boss {boss:?}，实际 {:?}",
            events[0].boss
        );
        assert_eq!(
            events[0].tick, 123,
            "VFX 事件 tick 应为当前 GameTick=123，实际 {}",
            events[0].tick
        );
    }

    /// §B — 撤退 action：无玩家目标时也不 panic（走 DVec3::X 退化路径）。
    #[test]
    fn retreat_action_no_nearest_player_does_not_panic() {
        let mut app = App::new();
        app.add_systems(Update, heiwushi_retreat_action_system);
        // nearest_player = None
        let boss = app
            .world_mut()
            .spawn((
                HeiwushiMarker,
                HeiwushiState::default(),
                NpcBlackboard {
                    nearest_player: None,
                    player_distance: 4.0,
                    ..Default::default()
                },
                Wounds {
                    entries: Vec::new(),
                    health_current: 500.0,
                    health_max: HEIWUSHI_HEALTH_MAX,
                },
                Lifecycle::default(),
                Navigator::new(),
                NpcPatrol::new("giant_sword_sea", DVec3::ZERO),
                Position::new([0.0, 64.0, 0.0]),
            ))
            .id();
        app.world_mut()
            .spawn((Actor(boss), ActionState::Requested, HeiwushiRetreatAction));
        app.update();
        app.update(); // 不应 panic
    }
}
