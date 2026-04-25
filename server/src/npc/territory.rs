//! 妖兽领地 + 繁衍（plan-npc-ai-v1 §3.3 / §6 / §8）。
//!
//! 领地半径决定繁衍容量：`capacity = round(radius / 10)`（§8 决议）。
//! 领地内 Beast 成体饱腹时周期性尝试繁衍：发 `NpcReproductionRequest` 请
//! `spawn` 侧创建幼崽（满员时迁出 200 格外，自动形成新领地锚点）。

use std::collections::HashMap;

use big_brain::prelude::{ActionBuilder, ActionState, Actor, BigBrainSet, Score, ScorerBuilder};
use valence::client::ClientMarker;
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Despawned, Entity, EventWriter, IntoSystemConfigs,
    Position, PreUpdate, Query, Res, ResMut, Resource, With, Without,
};

use crate::combat::components::Wounds;
use crate::combat::events::{AttackIntent, FIST_REACH};
use crate::cultivation::components::{Cultivation, Realm};
use crate::npc::hunger::Hunger;
use crate::npc::lifecycle::{
    NpcArchetype, NpcLifespan, NpcRegistry, NpcReproductionRequest, PendingRetirement,
};
use crate::npc::movement::GameTick;
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{NpcBlackboard, NpcMarker, NpcMeleeProfile};

/// 满员后幼崽迁出 200 格外开新领地（plan §8 决议）。
pub const YOUNG_EMIGRATE_DISTANCE: f64 = 200.0;
/// 幼崽认定阈值：age_ratio < 0.2（~前 20% 寿命视为幼体）。
pub const YOUNG_AGE_RATIO_THRESHOLD: f64 = 0.2;
/// 每 N tick 尝试一次繁衍判定（避免每 tick 全表扫描）。
pub const REPRODUCTION_TICK_INTERVAL: u32 = 600;
/// 成体饱腹阈值（Hunger.value）；低于此值不繁衍。
pub const REPRODUCTION_HUNGER_THRESHOLD: f64 = 0.7;
/// 领地半径→容量的换算因子：`capacity = round(radius / 10)`。
pub const TERRITORY_RADIUS_PER_CAPACITY: f64 = 10.0;
/// 幼崽濒危阈值：wounds.health_current / health_max 低于此即触发护崽。
pub const PROTECT_YOUNG_WOUND_RATIO: f32 = 0.5;
/// 护崽半径：领地内受伤幼崽距离成体超过此值时仍触发（全领地覆盖）。
pub const PROTECT_YOUNG_SEARCH_RADIUS: f64 = 32.0;
/// Hunt 目标评估：妖兽只追境界 <= Condense 的生物（高境界避战）。
pub const HUNT_MAX_TARGET_REALM: Realm = Realm::Condense;
/// Hunt 目标搜索半径（基于领地 + 该值）。
pub const HUNT_SEARCH_RADIUS: f64 = 48.0;
/// HuntAction 超时：追击 N tick 未到近战距离 → Success（让 picker 重选）。
pub const HUNT_ACTION_MAX_TICKS: u32 = 300;
/// Hunt 到达近战后攻击冷却（避免同一 tick 刷帧攻击）。
pub const HUNT_ATTACK_COOLDOWN_TICKS: u32 = 30;
/// TerritoryPatrolAction：单次巡逻 tick 上限（与 WanderAction 一致节奏）。
pub const TERRITORY_PATROL_MAX_TICKS: u32 = 240;
/// 领地内巡逻目标选择的最小 / 最大半径比例（相对 territory.radius）。
pub const PATROL_MIN_RADIUS_FRAC: f64 = 0.2;
pub const PATROL_MAX_RADIUS_FRAC: f64 = 0.9;
/// 领地巡逻速度因子。
pub const TERRITORY_PATROL_SPEED_FACTOR: f64 = 0.7;
/// Hunt 追击速度因子。
pub const HUNT_SPEED_FACTOR: f64 = 1.1;
/// ProtectYoung 护崽速度因子（快于巡逻）。
pub const PROTECT_YOUNG_SPEED_FACTOR: f64 = 1.2;

/// 妖兽领地。中心 + 半径决定繁衍空间与巡逻范围；`capacity` 由
/// `Territory::new` 按 `radius / 10` 派生，不单独存储便于避免两者漂移。
#[derive(Clone, Copy, Debug, Component)]
pub struct Territory {
    pub center: DVec3,
    pub radius: f64,
}

impl Territory {
    pub fn new(center: DVec3, radius: f64) -> Self {
        Self {
            center,
            radius: radius.max(1.0),
        }
    }

    pub fn capacity(&self) -> u32 {
        (self.radius / TERRITORY_RADIUS_PER_CAPACITY).round().max(1.0) as u32
    }

    pub fn contains(&self, pos: DVec3) -> bool {
        let dx = pos.x - self.center.x;
        let dz = pos.z - self.center.z;
        dx * dx + dz * dz <= self.radius * self.radius
    }
}

/// 幼崽标记。挂在 age_ratio < YOUNG_AGE_RATIO_THRESHOLD 的 Beast 上。
/// 由 `mark_young_beasts` 每 tick 同步（老了自动摘掉）。
#[derive(Clone, Copy, Debug, Component)]
pub struct NpcYoung;

/// 全局繁衍节拍：用一个 Resource tick 计数而非每 Beast 独立 timer，
/// 避免新生幼崽立刻又触发一轮繁衍。
#[derive(Clone, Copy, Debug, Default, Resource)]
pub struct BeastReproductionTick(pub u32);

// -------------------------------------------------------------------------
// Beast 专属 Scorer / Action（plan §2）
// -------------------------------------------------------------------------

/// 领地有入侵者评分：领地内存在玩家或非 Beast NPC → 1.0，否则 0。
#[derive(Clone, Copy, Debug, Component)]
pub struct TerritoryIntruderScorer;

/// 附近幼崽受伤评分：领地内幼崽 wounds 比 < 阈值 → 1.0。
#[derive(Clone, Copy, Debug, Component)]
pub struct ProtectYoungScorer;

/// 领地巡逻：在 territory.radius 内选随机点移动，计时到上限即 Success。
#[derive(Clone, Copy, Debug, Component)]
pub struct TerritoryPatrolAction;

/// 领地巡逻运行态（挂在 actor 上）。
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct TerritoryPatrolState {
    pub destination: Option<DVec3>,
    pub elapsed_ticks: u32,
}

/// 捕食：领地内寻找低境界目标（玩家或 NPC），设 Navigator goal 并在近战
/// 距离发 `AttackIntent`。无目标 / 超时 → Success，让 picker 重选。
#[derive(Clone, Copy, Debug, Component)]
pub struct HuntAction;

/// Hunt 运行态：当前目标 + elapsed；last_attack_tick = None 表示首次可立刻攻击。
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct HuntState {
    pub target: Option<Entity>,
    pub elapsed_ticks: u32,
    pub last_attack_tick: Option<u32>,
}

/// 护崽：奔向领地内最近的受伤幼崽，不直接攻击（攻击交给其他 action）。
#[derive(Clone, Copy, Debug, Component)]
pub struct ProtectYoungAction;

/// 护崽运行态：当前守护目标幼崽 + elapsed。
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct ProtectYoungState {
    pub guarded: Option<Entity>,
    pub elapsed_ticks: u32,
}

impl ScorerBuilder for TerritoryIntruderScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("TerritoryIntruderScorer")
    }
}

impl ScorerBuilder for ProtectYoungScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("ProtectYoungScorer")
    }
}

impl ActionBuilder for TerritoryPatrolAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("TerritoryPatrolAction")
    }
}

impl ActionBuilder for HuntAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("HuntAction")
    }
}

impl ActionBuilder for ProtectYoungAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("ProtectYoungAction")
    }
}

pub fn register(app: &mut App) {
    // 只注册繁衍所需的系统；Beast Scorer/Action 的 register 临时撤回，
    // 测试走 add_systems 不受影响。等实际场景产出 Beast NPC 再接入。
    app.insert_resource(BeastReproductionTick::default())
        .add_systems(
            PreUpdate,
            (mark_young_beasts, beast_reproduction_tick_system)
                .before(big_brain::prelude::BigBrainSet::Scorers),
        );
}

type BeastMarkerQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static NpcArchetype, &'static NpcLifespan, Option<&'static NpcYoung>),
    (With<NpcMarker>, Without<Despawned>),
>;

fn mark_young_beasts(
    mut commands: valence::prelude::Commands<'_, '_>,
    beasts: BeastMarkerQuery<'_, '_>,
) {
    for (entity, archetype, lifespan, young_tag) in &beasts {
        let is_beast = *archetype == NpcArchetype::Beast;
        let is_young = lifespan.age_ratio() < YOUNG_AGE_RATIO_THRESHOLD;
        match (is_beast && is_young, young_tag.is_some()) {
            (true, false) => {
                commands.entity(entity).insert(NpcYoung);
            }
            (false, true) => {
                commands.entity(entity).remove::<NpcYoung>();
            }
            _ => {}
        }
    }
}

type AdultBeastQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        &'static Hunger,
        &'static NpcLifespan,
        Option<&'static Territory>,
        Option<&'static NpcPatrol>,
    ),
    (
        With<NpcMarker>,
        Without<Despawned>,
        Without<PendingRetirement>,
        Without<NpcYoung>,
    ),
>;

type YoungCountQuery<'w, 's> = Query<
    'w,
    's,
    &'static Position,
    (With<NpcMarker>, With<NpcYoung>, Without<Despawned>),
>;

/// 每 `REPRODUCTION_TICK_INTERVAL` tick 跑一次：领地内找饱腹成体 → 若领地
/// 内幼崽 < capacity 则 spawn 幼崽在中心；若满员则 spawn 在领地外
/// `YOUNG_EMIGRATE_DISTANCE` 处（下一代的领地锚点）。
#[allow(clippy::too_many_arguments)]
fn beast_reproduction_tick_system(
    mut tick: ResMut<BeastReproductionTick>,
    registry: Option<Res<NpcRegistry>>,
    adults: AdultBeastQuery<'_, '_>,
    young: YoungCountQuery<'_, '_>,
    archetypes: Query<&NpcArchetype, With<NpcMarker>>,
    mut reproduction_requests: EventWriter<NpcReproductionRequest>,
) {
    tick.0 = tick.0.wrapping_add(1);
    if tick.0 % REPRODUCTION_TICK_INTERVAL != 0 {
        return;
    }

    if registry
        .as_deref()
        .map(NpcRegistry::should_reduce_population)
        .unwrap_or(false)
    {
        return;
    }

    let mut young_positions: Vec<DVec3> = young.iter().map(|p| p.get()).collect();

    // 按领地聚合，计算每个领地内的幼崽数 + 成体数
    let mut per_territory: HashMap<i64, (Territory, String, u32, u32, Vec<Entity>)> = HashMap::new();
    for (entity, pos, hunger, _lifespan, territory, patrol) in &adults {
        let Some(t) = territory else { continue };
        // 只统计该 entity 真是 Beast 的
        let Ok(arch) = archetypes.get(entity) else {
            continue;
        };
        if *arch != NpcArchetype::Beast {
            continue;
        }
        let home = patrol
            .map(|p| p.home_zone.clone())
            .unwrap_or_else(|| "unknown".to_string());
        // 用 territory.center 作为 key（i64 网格化避免 f64 hash）
        let key = (t.center.x.round() as i64) * 1_000_003 + (t.center.z.round() as i64);
        let entry = per_territory
            .entry(key)
            .or_insert((*t, home.clone(), 0, 0, Vec::new()));
        entry.3 += 1; // adult count
        if hunger.value >= REPRODUCTION_HUNGER_THRESHOLD && t.contains(pos.get()) {
            entry.4.push(entity);
        }
    }

    // 幼崽按领地归档：任一领地 contains 幼崽位置就 count
    for (_key, (t, _home, ref mut young_count, _adults, _fed)) in per_territory.iter_mut() {
        *young_count = young_positions
            .iter()
            .filter(|p| t.contains(**p))
            .count() as u32;
    }

    for (_, (t, home, young_count, _adults, fed_adults)) in per_territory.iter() {
        if fed_adults.is_empty() {
            continue;
        }

        let capacity = t.capacity();
        let (spawn_pos, new_territory_center) = if *young_count < capacity {
            // 未满员：就地繁衍，沿用父领地。
            (t.center, t.center)
        } else {
            // 满员：幼崽迁出 YOUNG_EMIGRATE_DISTANCE 外开新领地锚点。
            // 迁出方向由第一个饱腹成体相对领地中心的方位派生，
            // 若正好在中心则退化到 index 散列四向量，避免所有满员领
            // 地都朝同一方向 overflow。
            let first = *fed_adults.first().expect("fed_adults non-empty");
            let outward = derive_emigrate_direction(first, t.center, &adults);
            let pos = DVec3::new(
                t.center.x + outward.x * YOUNG_EMIGRATE_DISTANCE,
                t.center.y,
                t.center.z + outward.z * YOUNG_EMIGRATE_DISTANCE,
            );
            (pos, pos)
        };

        reproduction_requests.send(NpcReproductionRequest {
            archetype: NpcArchetype::Beast,
            position: spawn_pos,
            home_zone: home.clone(),
            initial_age_ticks: 0.0,
            territory_center: Some(new_territory_center),
            territory_radius: Some(t.radius),
        });

        // 同一 tick 内登记一次，防止双成体一 tick 内连发同领地
        young_positions.push(spawn_pos);
    }
}

/// 把"成体相对领地中心的方位"归一化为单位向量；距离过近则退化到
/// 按 entity index 散列的四向量（+X / +Z / -X / -Z），避免 E-W 相邻
/// 领地 overflow 全部撞到同一条线。
fn derive_emigrate_direction(
    adult: Entity,
    center: DVec3,
    adults: &AdultBeastQuery<'_, '_>,
) -> DVec3 {
    let Ok((_, pos, _, _, _, _)) = adults.get(adult) else {
        return emigrate_fallback_direction(adult);
    };
    let offset = pos.get() - center;
    let len2 = offset.x * offset.x + offset.z * offset.z;
    if len2 < 1.0 {
        emigrate_fallback_direction(adult)
    } else {
        let len = len2.sqrt();
        DVec3::new(offset.x / len, 0.0, offset.z / len)
    }
}

fn emigrate_fallback_direction(actor: Entity) -> DVec3 {
    match actor.index() % 4 {
        0 => DVec3::new(1.0, 0.0, 0.0),
        1 => DVec3::new(0.0, 0.0, 1.0),
        2 => DVec3::new(-1.0, 0.0, 0.0),
        _ => DVec3::new(0.0, 0.0, -1.0),
    }
}

// -------------------------------------------------------------------------
// Beast 行为（Scorer / Action）实现
// -------------------------------------------------------------------------

type TerritoryOwnerQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Position, &'static Territory),
    (With<NpcMarker>, Without<Despawned>),
>;

type PlayerPositionQuery<'w, 's> = Query<'w, 's, &'static Position, With<ClientMarker>>;

type NpcPosArchQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Position, &'static NpcArchetype),
    (With<NpcMarker>, Without<Despawned>),
>;

fn territory_intruder_scorer_system(
    beasts: TerritoryOwnerQuery<'_, '_>,
    players: PlayerPositionQuery<'_, '_>,
    npcs: NpcPosArchQuery<'_, '_>,
    mut scorers: Query<(&Actor, &mut Score), With<TerritoryIntruderScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((_, territory)) = beasts.get(*actor) {
            let has_player = players.iter().any(|p| territory.contains(p.get()));
            let has_hostile_npc = npcs.iter().any(|(ent, p, arch)| {
                ent != *actor
                    && *arch != NpcArchetype::Beast
                    && territory.contains(p.get())
            });
            if has_player || has_hostile_npc {
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

type InjuredYoungQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Position, &'static Wounds),
    (With<NpcMarker>, With<NpcYoung>, Without<Despawned>),
>;

fn protect_young_scorer_system(
    beasts: TerritoryOwnerQuery<'_, '_>,
    young: InjuredYoungQuery<'_, '_>,
    mut scorers: Query<(&Actor, &mut Score), With<ProtectYoungScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((_, territory)) = beasts.get(*actor) {
            let injured = young.iter().any(|(_, pos, wounds)| {
                let ratio = wounds.health_current / wounds.health_max.max(1.0);
                ratio < PROTECT_YOUNG_WOUND_RATIO && territory.contains(pos.get())
            });
            if injured {
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

/// Territory 内基于 actor.index + tick 的确定性巡逻目标。
pub(crate) fn territory_patrol_target(
    npc_pos: DVec3,
    territory: &Territory,
    actor_index: u32,
    game_tick: u32,
) -> DVec3 {
    let seed = (actor_index as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((game_tick as u64).wrapping_mul(0x94D0_49BB_1331_11EB));
    let angle = ((seed >> 16) % 3600) as f64 / 3600.0 * std::f64::consts::TAU;
    let r_frac = PATROL_MIN_RADIUS_FRAC
        + ((seed >> 32) % 1000) as f64 / 1000.0
            * (PATROL_MAX_RADIUS_FRAC - PATROL_MIN_RADIUS_FRAC);
    let radius = territory.radius * r_frac;
    DVec3::new(
        territory.center.x + angle.cos() * radius,
        npc_pos.y,
        territory.center.z + angle.sin() * radius,
    )
}

type PatrolMutQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static Territory,
        &'static mut Navigator,
        &'static mut TerritoryPatrolState,
    ),
    With<NpcMarker>,
>;

fn territory_patrol_action_system(
    mut beasts: PatrolMutQuery<'_, '_>,
    mut actions: Query<(&Actor, &mut ActionState), With<TerritoryPatrolAction>>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut state) in &mut actions {
        let Ok((pos, territory, mut navigator, mut patrol_state)) = beasts.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };
        match *state {
            ActionState::Requested => {
                let target = territory_patrol_target(pos.get(), territory, actor.index(), tick);
                navigator.set_goal(target, TERRITORY_PATROL_SPEED_FACTOR);
                patrol_state.destination = Some(target);
                patrol_state.elapsed_ticks = 0;
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                patrol_state.elapsed_ticks = patrol_state.elapsed_ticks.saturating_add(1);
                let arrived = patrol_state
                    .destination
                    .map(|dest| pos.get().distance(dest) <= 1.6)
                    .unwrap_or(true);
                if arrived || patrol_state.elapsed_ticks >= TERRITORY_PATROL_MAX_TICKS {
                    navigator.stop();
                    patrol_state.destination = None;
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                navigator.stop();
                patrol_state.destination = None;
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

type HuntCandidateQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Position, &'static Cultivation),
    (Without<Despawned>, Without<NpcYoung>),
>;

fn hunt_action_system(
    mut beasts: Query<
        (
            &Position,
            &Territory,
            &NpcMeleeProfile,
            &mut Navigator,
            &mut HuntState,
        ),
        With<NpcMarker>,
    >,
    candidates: HuntCandidateQuery<'_, '_>,
    npc_arch: Query<&NpcArchetype, With<NpcMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<HuntAction>>,
    mut attack_intents: EventWriter<AttackIntent>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut state) in &mut actions {
        let Ok((pos, territory, profile, mut navigator, mut hunt)) = beasts.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };
        match *state {
            ActionState::Requested => {
                let picked = pick_hunt_target(pos.get(), territory, &candidates, &npc_arch, *actor);
                match picked {
                    Some((ent, tpos)) => {
                        navigator.set_goal(tpos, HUNT_SPEED_FACTOR);
                        hunt.target = Some(ent);
                        hunt.elapsed_ticks = 0;
                        hunt.last_attack_tick = None; // 首次攻击无 cooldown
                        *state = ActionState::Executing;
                    }
                    None => {
                        navigator.stop();
                        hunt.target = None;
                        *state = ActionState::Success;
                    }
                }
            }
            ActionState::Executing => {
                hunt.elapsed_ticks = hunt.elapsed_ticks.saturating_add(1);
                if hunt.elapsed_ticks >= HUNT_ACTION_MAX_TICKS {
                    navigator.stop();
                    hunt.target = None;
                    *state = ActionState::Success;
                    continue;
                }
                let Some(target) = hunt.target else {
                    *state = ActionState::Success;
                    continue;
                };
                let Ok((_, tpos, _)) = candidates.get(target) else {
                    hunt.target = None;
                    *state = ActionState::Success;
                    continue;
                };
                let tposv = tpos.get();
                navigator.set_goal(tposv, HUNT_SPEED_FACTOR);
                let distance = pos.get().distance(tposv) as f32;
                let cooled = hunt
                    .last_attack_tick
                    .map(|last| tick.saturating_sub(last) >= HUNT_ATTACK_COOLDOWN_TICKS)
                    .unwrap_or(true);
                if distance <= profile.reach.max && cooled {
                    attack_intents.send(AttackIntent {
                        attacker: *actor,
                        target: Some(target),
                        issued_at_tick: u64::from(tick),
                        reach: profile.reach,
                        qi_invest: 8.0,
                        wound_kind: profile.wound_kind,
                        debug_command: None,
                    });
                    hunt.last_attack_tick = Some(tick);
                }
            }
            ActionState::Cancelled => {
                navigator.stop();
                hunt.target = None;
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

/// Beast 选择捕食目标：领地内 + 搜索半径内 + 境界 <= HUNT_MAX_TARGET_REALM +
/// 非自身 + 非 Beast archetype。取距离最近者。
pub(crate) fn pick_hunt_target(
    pos: DVec3,
    territory: &Territory,
    candidates: &HuntCandidateQuery<'_, '_>,
    npc_arch: &Query<&NpcArchetype, With<NpcMarker>>,
    self_entity: Entity,
) -> Option<(Entity, DVec3)> {
    let mut best: Option<(Entity, DVec3, f64)> = None;
    for (entity, tpos, cult) in candidates.iter() {
        if entity == self_entity {
            continue;
        }
        let tposv = tpos.get();
        if !territory.contains(tposv) {
            continue;
        }
        if (cult.realm as u32) > (HUNT_MAX_TARGET_REALM as u32) {
            continue;
        }
        // NPC 侧过滤：Beast 不猎 Beast。Client（玩家）无 NpcMarker，arch 查询会返回 Err 视为"非 Beast"。
        if npc_arch.get(entity).ok().copied() == Some(NpcArchetype::Beast) {
            continue;
        }
        let d = pos.distance(tposv);
        if d > HUNT_SEARCH_RADIUS {
            continue;
        }
        if best.as_ref().map(|(_, _, bd)| d < *bd).unwrap_or(true) {
            best = Some((entity, tposv, d));
        }
    }
    best.map(|(e, p, _)| (e, p))
}

type ProtectYoungMutQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static Territory,
        &'static mut Navigator,
        &'static mut ProtectYoungState,
    ),
    With<NpcMarker>,
>;

fn protect_young_action_system(
    mut beasts: ProtectYoungMutQuery<'_, '_>,
    young: InjuredYoungQuery<'_, '_>,
    mut actions: Query<(&Actor, &mut ActionState), With<ProtectYoungAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((pos, territory, mut navigator, mut py_state)) = beasts.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        // 重新选一次最近低血幼崽
        let nearest_injured = young
            .iter()
            .filter_map(|(ent, ypos, wounds)| {
                let ratio = wounds.health_current / wounds.health_max.max(1.0);
                if ratio < PROTECT_YOUNG_WOUND_RATIO && territory.contains(ypos.get()) {
                    let d = pos.get().distance(ypos.get());
                    if d <= PROTECT_YOUNG_SEARCH_RADIUS {
                        return Some((ent, ypos.get(), d));
                    }
                }
                None
            })
            .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

        match *state {
            ActionState::Requested => {
                let Some((ent, tpos, _)) = nearest_injured else {
                    navigator.stop();
                    py_state.guarded = None;
                    *state = ActionState::Success;
                    continue;
                };
                navigator.set_goal(tpos, PROTECT_YOUNG_SPEED_FACTOR);
                py_state.guarded = Some(ent);
                py_state.elapsed_ticks = 0;
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                py_state.elapsed_ticks = py_state.elapsed_ticks.saturating_add(1);
                let Some((ent, tpos, d)) = nearest_injured else {
                    navigator.stop();
                    py_state.guarded = None;
                    *state = ActionState::Success;
                    continue;
                };
                // 贴近即 Success（让后续 Hunt 接手反击入侵者）
                if d < 4.0 {
                    navigator.stop();
                    py_state.guarded = None;
                    *state = ActionState::Success;
                    continue;
                }
                py_state.guarded = Some(ent);
                navigator.set_goal(tpos, PROTECT_YOUNG_SPEED_FACTOR);
            }
            ActionState::Cancelled => {
                navigator.stop();
                py_state.guarded = None;
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::npc::patrol::NpcPatrol;
    use valence::prelude::{App, Events, PreUpdate};

    fn spawn_adult_beast(
        app: &mut App,
        pos: DVec3,
        age_ratio: f64,
        hunger_value: f64,
        territory: Territory,
    ) -> Entity {
        let max_age = NpcArchetype::Beast.default_max_age_ticks();
        app.world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Beast,
                NpcLifespan::new(max_age * age_ratio, max_age),
                Hunger::new(hunger_value),
                Position::new([pos.x, pos.y, pos.z]),
                NpcPatrol::new("zone_x", pos),
                territory,
            ))
            .id()
    }

    fn spawn_young_beast(app: &mut App, pos: DVec3, territory: Territory) -> Entity {
        let max_age = NpcArchetype::Beast.default_max_age_ticks();
        app.world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Beast,
                NpcLifespan::new(max_age * 0.05, max_age),
                Hunger::new(1.0),
                Position::new([pos.x, pos.y, pos.z]),
                NpcPatrol::new("zone_x", pos),
                territory,
                NpcYoung,
            ))
            .id()
    }

    #[test]
    fn territory_capacity_rounds_half_away_from_zero() {
        assert_eq!(Territory::new(DVec3::ZERO, 10.0).capacity(), 1);
        assert_eq!(Territory::new(DVec3::ZERO, 19.0).capacity(), 2);
        assert_eq!(Territory::new(DVec3::ZERO, 20.0).capacity(), 2);
        assert_eq!(Territory::new(DVec3::ZERO, 25.0).capacity(), 3);
        assert_eq!(Territory::new(DVec3::ZERO, 100.0).capacity(), 10);
    }

    #[test]
    fn territory_capacity_clamps_tiny_radius_to_one() {
        assert_eq!(Territory::new(DVec3::ZERO, 0.5).capacity(), 1);
        assert_eq!(Territory::new(DVec3::ZERO, 3.0).capacity(), 1);
    }

    #[test]
    fn territory_contains_uses_xz_distance_only() {
        let t = Territory::new(DVec3::new(0.0, 64.0, 0.0), 10.0);
        assert!(t.contains(DVec3::new(5.0, 64.0, 5.0)));
        assert!(t.contains(DVec3::new(10.0, 10.0, 0.0)), "y 不参与判定");
        assert!(!t.contains(DVec3::new(8.0, 64.0, 8.0)), "边界外 xz 距离 11.3");
    }

    #[test]
    fn mark_young_beasts_tags_young_and_untags_grown() {
        let mut app = App::new();
        app.add_systems(PreUpdate, mark_young_beasts);

        let max_age = NpcArchetype::Beast.default_max_age_ticks();
        let young = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Beast,
                NpcLifespan::new(max_age * 0.05, max_age), // 年幼
            ))
            .id();
        let adult = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Beast,
                NpcLifespan::new(max_age * 0.5, max_age), // 成年
            ))
            .id();

        app.update();
        assert!(app.world().get::<NpcYoung>(young).is_some());
        assert!(app.world().get::<NpcYoung>(adult).is_none());

        // 推进：young → 长大（age_ratio 超过阈值）
        {
            let mut ls = app.world_mut().get_mut::<NpcLifespan>(young).unwrap();
            ls.age_ticks = max_age * 0.5;
        }
        app.update();
        assert!(app.world().get::<NpcYoung>(young).is_none());
    }

    #[test]
    fn mark_young_ignores_non_beast_archetype() {
        let mut app = App::new();
        app.add_systems(PreUpdate, mark_young_beasts);

        let max_age = NpcArchetype::Commoner.default_max_age_ticks();
        let young_commoner = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Commoner,
                NpcLifespan::new(max_age * 0.05, max_age),
            ))
            .id();

        app.update();
        assert!(
            app.world().get::<NpcYoung>(young_commoner).is_none(),
            "NpcYoung 只标 Beast"
        );
    }

    fn build_app() -> App {
        let mut app = App::new();
        app.insert_resource(BeastReproductionTick::default());
        app.insert_resource(NpcRegistry::default());
        app.add_event::<NpcReproductionRequest>();
        app.add_systems(PreUpdate, beast_reproduction_tick_system);
        app
    }

    fn tick_to_interval(app: &mut App) {
        for _ in 0..REPRODUCTION_TICK_INTERVAL {
            app.update();
        }
    }

    #[test]
    fn reproduction_fires_when_adult_fed_and_territory_empty() {
        let mut app = build_app();
        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 30.0);
        let _adult = spawn_adult_beast(
            &mut app,
            DVec3::new(0.0, 64.0, 0.0),
            0.5,
            0.9,
            territory,
        );

        tick_to_interval(&mut app);

        let events = app.world().resource::<Events<NpcReproductionRequest>>();
        let mut reader = events.get_reader();
        let batch: Vec<_> = reader.read(events).cloned().collect();
        assert_eq!(batch.len(), 1, "应发 1 条幼崽生成请求");
        let req = &batch[0];
        assert_eq!(req.archetype, NpcArchetype::Beast);
        assert!(territory.contains(req.position), "未满员时在领地内生");
        assert_eq!(req.initial_age_ticks, 0.0);
    }

    #[test]
    fn reproduction_skipped_when_adult_hungry() {
        let mut app = build_app();
        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 30.0);
        let _adult = spawn_adult_beast(
            &mut app,
            DVec3::new(0.0, 64.0, 0.0),
            0.5,
            0.4, // 低于阈值
            territory,
        );

        tick_to_interval(&mut app);
        let events = app.world().resource::<Events<NpcReproductionRequest>>();
        assert_eq!(events.len(), 0, "饿肚子不繁衍");
    }

    #[test]
    fn reproduction_emigrates_when_territory_full() {
        let mut app = build_app();
        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 10.0); // capacity=1
        let _adult = spawn_adult_beast(
            &mut app,
            DVec3::new(0.0, 64.0, 0.0),
            0.5,
            0.95,
            territory,
        );
        let _existing_young =
            spawn_young_beast(&mut app, DVec3::new(0.0, 64.0, 0.0), territory);

        tick_to_interval(&mut app);

        let events = app.world().resource::<Events<NpcReproductionRequest>>();
        let mut reader = events.get_reader();
        let batch: Vec<_> = reader.read(events).cloned().collect();
        assert_eq!(batch.len(), 1);
        let req = &batch[0];
        let dx = req.position.x - territory.center.x;
        let dz = req.position.z - territory.center.z;
        let d = (dx * dx + dz * dz).sqrt();
        assert!(
            (d - YOUNG_EMIGRATE_DISTANCE).abs() < 1e-6,
            "满员时幼崽外迁应在 {YOUNG_EMIGRATE_DISTANCE:.0} 格处，实际 {d:.1}"
        );
    }

    #[test]
    fn reproduction_emigrate_direction_follows_adult_offset_from_center() {
        // P2-4（Claude review）: 满员迁出方向必须派生自"成体相对领地中心的偏移"，
        // 而不是硬编码 +X —— 否则多个相邻领地的 overflow 会全部撞到同一条线。
        let mut app = build_app();
        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 10.0); // capacity=1
        // 成体位于中心正 +Z 方向；领地满员（1 幼崽占位）。
        let _adult = spawn_adult_beast(
            &mut app,
            DVec3::new(0.0, 64.0, 5.0),
            0.5,
            0.95,
            territory,
        );
        let _existing_young =
            spawn_young_beast(&mut app, DVec3::new(0.0, 64.0, 0.0), territory);

        tick_to_interval(&mut app);

        let events = app.world().resource::<Events<NpcReproductionRequest>>();
        let mut reader = events.get_reader();
        let batch: Vec<_> = reader.read(events).cloned().collect();
        assert_eq!(batch.len(), 1);
        let req = &batch[0];
        let dx = req.position.x - territory.center.x;
        let dz = req.position.z - territory.center.z;
        assert!(dz > 0.0, "成体在 +Z，迁出方向也应偏 +Z（实际 dz={dz:.2}）");
        assert!(dx.abs() < 1e-6, "成体只偏 Z 轴，dx 应 ≈ 0（实际 dx={dx:.2}）");
    }

    #[test]
    fn reproduction_skipped_when_registry_paused() {
        let mut app = build_app();
        // 填满 registry
        app.world_mut().resource_mut::<NpcRegistry>().live_npc_count = 9999;
        app.world_mut().resource_mut::<NpcRegistry>().max_npc_count = 100;

        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 30.0);
        let _adult = spawn_adult_beast(
            &mut app,
            DVec3::new(0.0, 64.0, 0.0),
            0.5,
            0.95,
            territory,
        );

        tick_to_interval(&mut app);
        let events = app.world().resource::<Events<NpcReproductionRequest>>();
        assert_eq!(events.len(), 0, "超员时不繁衍");
    }

    #[test]
    fn reproduction_only_fires_on_interval_boundary() {
        let mut app = build_app();
        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 30.0);
        let _adult = spawn_adult_beast(
            &mut app,
            DVec3::new(0.0, 64.0, 0.0),
            0.5,
            0.95,
            territory,
        );

        // 前 N-1 tick 不应发事件
        for _ in 0..REPRODUCTION_TICK_INTERVAL - 1 {
            app.update();
            let events = app.world().resource::<Events<NpcReproductionRequest>>();
            assert_eq!(events.len(), 0, "非整数倍 tick 不触发");
        }
        // 第 N tick 触发
        app.update();
        let events = app.world().resource::<Events<NpcReproductionRequest>>();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn reproduction_skipped_when_territory_already_full_and_outside_adult_no_territory() {
        // 成体站在领地内，但不是 adult（young 不算繁衍来源）。
        let mut app = build_app();
        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 30.0);
        let _only_young =
            spawn_young_beast(&mut app, DVec3::new(0.0, 64.0, 0.0), territory);

        tick_to_interval(&mut app);
        let events = app.world().resource::<Events<NpcReproductionRequest>>();
        assert_eq!(events.len(), 0, "没有成体则没繁衍源");
    }

    #[test]
    fn reproduction_groups_multiple_adults_single_request_per_territory() {
        // 同一领地 3 个成体，同一 tick 只发一条请求。
        let mut app = build_app();
        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 40.0); // capacity=4
        spawn_adult_beast(&mut app, DVec3::new(0.0, 64.0, 0.0), 0.5, 0.9, territory);
        spawn_adult_beast(&mut app, DVec3::new(5.0, 64.0, 5.0), 0.5, 0.9, territory);
        spawn_adult_beast(&mut app, DVec3::new(-5.0, 64.0, 5.0), 0.5, 0.9, territory);

        tick_to_interval(&mut app);
        let events = app.world().resource::<Events<NpcReproductionRequest>>();
        assert_eq!(events.len(), 1, "同领地多成体一 tick 一条请求");
    }

    // ---------------------------------------------------------------------
    // Beast 行为（Scorer / Action）饱和测试
    // ---------------------------------------------------------------------

    use crate::cultivation::components::{Cultivation, Realm};

    const TEST_HEALTH_MAX: f32 = 100.0;

    fn make_wounds(current: f32) -> Wounds {
        Wounds {
            health_current: current,
            health_max: TEST_HEALTH_MAX,
            entries: Vec::new(),
        }
    }

    fn spawn_beast_with_territory(app: &mut App, pos: DVec3, territory: Territory) -> Entity {
        let max_age = NpcArchetype::Beast.default_max_age_ticks();
        app.world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Beast,
                NpcLifespan::new(max_age * 0.5, max_age),
                Position::new([pos.x, pos.y, pos.z]),
                territory,
                NpcBlackboard::default(),
                NpcMeleeProfile::fist(),
                Navigator::new(),
            ))
            .id()
    }

    // --- TerritoryIntruderScorer ---

    fn build_intruder_app() -> App {
        let mut app = App::new();
        app.add_systems(PreUpdate, territory_intruder_scorer_system);
        app
    }

    #[test]
    fn territory_intruder_scorer_zero_when_no_one_in_range() {
        let mut app = build_intruder_app();
        let beast = spawn_beast_with_territory(
            &mut app,
            DVec3::new(0.0, 64.0, 0.0),
            Territory::new(DVec3::new(0.0, 64.0, 0.0), 20.0),
        );
        let scorer = app
            .world_mut()
            .spawn((Actor(beast), Score::default(), TerritoryIntruderScorer))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "空领地评分应为 0"
        );
    }

    #[test]
    fn territory_intruder_scorer_one_when_player_inside() {
        let mut app = build_intruder_app();
        let beast = spawn_beast_with_territory(
            &mut app,
            DVec3::new(0.0, 64.0, 0.0),
            Territory::new(DVec3::new(0.0, 64.0, 0.0), 20.0),
        );
        let _player = app
            .world_mut()
            .spawn((
                ClientMarker,
                Position::new([5.0, 64.0, 5.0]),
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(beast), Score::default(), TerritoryIntruderScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 1.0);
    }

    #[test]
    fn territory_intruder_scorer_one_when_non_beast_npc_inside() {
        let mut app = build_intruder_app();
        let beast = spawn_beast_with_territory(
            &mut app,
            DVec3::new(0.0, 64.0, 0.0),
            Territory::new(DVec3::new(0.0, 64.0, 0.0), 20.0),
        );
        let _rogue = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Rogue,
                Position::new([6.0, 64.0, 6.0]),
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(beast), Score::default(), TerritoryIntruderScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 1.0);
    }

    #[test]
    fn territory_intruder_scorer_zero_when_only_other_beasts_inside() {
        let mut app = build_intruder_app();
        let beast = spawn_beast_with_territory(
            &mut app,
            DVec3::new(0.0, 64.0, 0.0),
            Territory::new(DVec3::new(0.0, 64.0, 0.0), 20.0),
        );
        let _other_beast = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Beast,
                Position::new([6.0, 64.0, 6.0]),
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(beast), Score::default(), TerritoryIntruderScorer))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "同类不算入侵者"
        );
    }

    #[test]
    fn territory_intruder_scorer_zero_when_player_outside_radius() {
        let mut app = build_intruder_app();
        let beast = spawn_beast_with_territory(
            &mut app,
            DVec3::new(0.0, 64.0, 0.0),
            Territory::new(DVec3::new(0.0, 64.0, 0.0), 10.0),
        );
        let _player = app
            .world_mut()
            .spawn((ClientMarker, Position::new([50.0, 64.0, 50.0])))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(beast), Score::default(), TerritoryIntruderScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    // --- ProtectYoungScorer ---

    fn build_protect_young_app() -> App {
        let mut app = App::new();
        app.add_systems(PreUpdate, protect_young_scorer_system);
        app
    }

    #[test]
    fn protect_young_scorer_zero_when_no_young() {
        let mut app = build_protect_young_app();
        let beast = spawn_beast_with_territory(
            &mut app,
            DVec3::new(0.0, 64.0, 0.0),
            Territory::new(DVec3::new(0.0, 64.0, 0.0), 20.0),
        );
        let scorer = app
            .world_mut()
            .spawn((Actor(beast), Score::default(), ProtectYoungScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    #[test]
    fn protect_young_scorer_zero_when_young_full_hp() {
        let mut app = build_protect_young_app();
        let beast = spawn_beast_with_territory(
            &mut app,
            DVec3::new(0.0, 64.0, 0.0),
            Territory::new(DVec3::new(0.0, 64.0, 0.0), 20.0),
        );
        app.world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Beast,
                NpcYoung,
                Position::new([5.0, 64.0, 5.0]),
                make_wounds(TEST_HEALTH_MAX),
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(beast), Score::default(), ProtectYoungScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    #[test]
    fn protect_young_scorer_one_when_young_injured_inside() {
        let mut app = build_protect_young_app();
        let beast = spawn_beast_with_territory(
            &mut app,
            DVec3::new(0.0, 64.0, 0.0),
            Territory::new(DVec3::new(0.0, 64.0, 0.0), 20.0),
        );
        app.world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Beast,
                NpcYoung,
                Position::new([5.0, 64.0, 5.0]),
                make_wounds(TEST_HEALTH_MAX * 0.3), // 30% < PROTECT_YOUNG_WOUND_RATIO=0.5
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(beast), Score::default(), ProtectYoungScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 1.0);
    }

    #[test]
    fn protect_young_scorer_zero_when_injured_young_outside_territory() {
        let mut app = build_protect_young_app();
        let beast = spawn_beast_with_territory(
            &mut app,
            DVec3::new(0.0, 64.0, 0.0),
            Territory::new(DVec3::new(0.0, 64.0, 0.0), 10.0),
        );
        app.world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Beast,
                NpcYoung,
                Position::new([30.0, 64.0, 30.0]), // 领地外
                make_wounds(TEST_HEALTH_MAX * 0.2),
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(beast), Score::default(), ProtectYoungScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    // --- territory_patrol_target ---

    #[test]
    fn territory_patrol_target_stays_within_radius() {
        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 30.0);
        for actor_idx in 0..50 {
            for tick in 0..50 {
                let target = territory_patrol_target(DVec3::ZERO, &territory, actor_idx, tick);
                let dx = target.x - territory.center.x;
                let dz = target.z - territory.center.z;
                let d = (dx * dx + dz * dz).sqrt();
                assert!(
                    d <= territory.radius * PATROL_MAX_RADIUS_FRAC + 1e-6,
                    "巡逻目标应在 {}*0.9 内，得到 {d:.2}",
                    territory.radius
                );
                assert!(
                    d >= territory.radius * PATROL_MIN_RADIUS_FRAC - 1e-6,
                    "巡逻目标应在 {}*0.2 外，得到 {d:.2}",
                    territory.radius
                );
            }
        }
    }

    // --- TerritoryPatrolAction ---

    #[test]
    fn territory_patrol_action_requested_sets_navigator_then_succeeds_on_timeout() {
        let mut app = App::new();
        app.insert_resource(GameTick(42));
        app.add_systems(
            PreUpdate,
            territory_patrol_action_system.in_set(big_brain::prelude::BigBrainSet::Actions),
        );

        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 20.0);
        let beast = spawn_beast_with_territory(&mut app, DVec3::new(0.0, 64.0, 0.0), territory);
        app.world_mut()
            .entity_mut(beast)
            .insert(TerritoryPatrolState::default());

        let action = app
            .world_mut()
            .spawn((Actor(beast), TerritoryPatrolAction, ActionState::Requested))
            .id();

        app.update();
        let patrol = *app.world().get::<TerritoryPatrolState>(beast).unwrap();
        assert!(patrol.destination.is_some(), "Requested 应设目标");
        assert!(
            !app.world().get::<Navigator>(beast).unwrap().is_idle(),
            "Requested 应驱动 Navigator"
        );

        {
            let mut state = app
                .world_mut()
                .get_mut::<TerritoryPatrolState>(beast)
                .unwrap();
            state.elapsed_ticks = TERRITORY_PATROL_MAX_TICKS - 1;
        }
        app.update();
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success
        );
        assert!(app.world().get::<Navigator>(beast).unwrap().is_idle());
    }

    // --- HuntAction ---

    fn spawn_hunt_target(app: &mut App, pos: DVec3, realm: Realm) -> Entity {
        let mut cult = Cultivation::default();
        cult.realm = realm;
        app.world_mut()
            .spawn((Position::new([pos.x, pos.y, pos.z]), cult))
            .id()
    }

    fn spawn_beast_hunter(app: &mut App, pos: DVec3, territory: Territory) -> Entity {
        let beast = spawn_beast_with_territory(app, pos, territory);
        app.world_mut()
            .entity_mut(beast)
            .insert(HuntState::default());
        beast
    }

    fn build_hunt_app() -> App {
        let mut app = App::new();
        app.insert_resource(GameTick(10));
        app.add_event::<AttackIntent>();
        app.add_systems(
            PreUpdate,
            hunt_action_system.in_set(big_brain::prelude::BigBrainSet::Actions),
        );
        app
    }

    #[test]
    fn hunt_action_success_when_no_targets_available() {
        let mut app = build_hunt_app();
        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 20.0);
        let beast = spawn_beast_hunter(&mut app, DVec3::new(0.0, 64.0, 0.0), territory);
        let action = app
            .world_mut()
            .spawn((Actor(beast), HuntAction, ActionState::Requested))
            .id();

        app.update();
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success,
            "无目标应立即 Success"
        );
    }

    #[test]
    fn hunt_action_pursues_low_realm_target_and_sets_navigator() {
        let mut app = build_hunt_app();
        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 50.0);
        let beast = spawn_beast_hunter(&mut app, DVec3::new(0.0, 64.0, 0.0), territory);
        let target = spawn_hunt_target(&mut app, DVec3::new(10.0, 64.0, 10.0), Realm::Awaken);

        let _action = app
            .world_mut()
            .spawn((Actor(beast), HuntAction, ActionState::Requested))
            .id();
        app.update();

        let hunt = *app.world().get::<HuntState>(beast).unwrap();
        assert_eq!(hunt.target, Some(target));
        assert!(!app.world().get::<Navigator>(beast).unwrap().is_idle());
    }

    #[test]
    fn hunt_action_skips_target_above_max_realm() {
        let mut app = build_hunt_app();
        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 50.0);
        let beast = spawn_beast_hunter(&mut app, DVec3::new(0.0, 64.0, 0.0), territory);
        let _untouchable = spawn_hunt_target(&mut app, DVec3::new(10.0, 64.0, 10.0), Realm::Void);

        let action = app
            .world_mut()
            .spawn((Actor(beast), HuntAction, ActionState::Requested))
            .id();
        app.update();
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success,
            "高境界目标不追"
        );
    }

    #[test]
    fn hunt_action_emits_attack_intent_at_melee_range() {
        let mut app = build_hunt_app();
        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 20.0);
        let beast = spawn_beast_hunter(&mut app, DVec3::new(0.0, 64.0, 0.0), territory);
        // target 0.8 格外，在 FIST_REACH.max = 1.3 内
        let target = spawn_hunt_target(&mut app, DVec3::new(0.8, 64.0, 0.0), Realm::Awaken);

        let _action = app
            .world_mut()
            .spawn((Actor(beast), HuntAction, ActionState::Requested))
            .id();
        app.update(); // Requested → Executing
        app.update(); // Executing：距离判定 + 发 AttackIntent

        let events = app.world().resource::<Events<AttackIntent>>();
        let mut reader = events.get_reader();
        let fired: Vec<_> = reader.read(events).cloned().collect();
        assert!(
            !fired.is_empty(),
            "HuntAction 在近战距离应至少发一次 AttackIntent"
        );
        let first = &fired[0];
        assert_eq!(first.attacker, beast);
        assert_eq!(first.target, Some(target));
    }

    #[test]
    fn hunt_action_skips_other_beast_targets() {
        let mut app = build_hunt_app();
        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 50.0);
        let beast = spawn_beast_hunter(&mut app, DVec3::new(0.0, 64.0, 0.0), territory);
        // 另一只 Beast（应被排除）
        app.world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Beast,
                Position::new([10.0, 64.0, 10.0]),
                Cultivation::default(),
            ))
            .id();

        let action = app
            .world_mut()
            .spawn((Actor(beast), HuntAction, ActionState::Requested))
            .id();
        app.update();
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success,
            "Beast 不猎 Beast"
        );
    }

    // --- ProtectYoungAction ---

    fn build_protect_young_action_app() -> App {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            protect_young_action_system.in_set(big_brain::prelude::BigBrainSet::Actions),
        );
        app
    }

    fn spawn_injured_young(app: &mut App, pos: DVec3, hp_ratio: f32) -> Entity {
        app.world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Beast,
                NpcYoung,
                Position::new([pos.x, pos.y, pos.z]),
                make_wounds(TEST_HEALTH_MAX * hp_ratio),
            ))
            .id()
    }

    #[test]
    fn protect_young_action_success_when_no_injured_young() {
        let mut app = build_protect_young_action_app();
        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 20.0);
        let beast = spawn_beast_with_territory(&mut app, DVec3::new(0.0, 64.0, 0.0), territory);
        app.world_mut()
            .entity_mut(beast)
            .insert(ProtectYoungState::default());

        let action = app
            .world_mut()
            .spawn((Actor(beast), ProtectYoungAction, ActionState::Requested))
            .id();
        app.update();
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success
        );
    }

    #[test]
    fn protect_young_action_moves_to_injured_young() {
        let mut app = build_protect_young_action_app();
        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 30.0);
        let beast = spawn_beast_with_territory(&mut app, DVec3::new(0.0, 64.0, 0.0), territory);
        app.world_mut()
            .entity_mut(beast)
            .insert(ProtectYoungState::default());
        let young = spawn_injured_young(&mut app, DVec3::new(10.0, 64.0, 10.0), 0.2);

        let _action = app
            .world_mut()
            .spawn((Actor(beast), ProtectYoungAction, ActionState::Requested))
            .id();
        app.update();
        let py = *app.world().get::<ProtectYoungState>(beast).unwrap();
        assert_eq!(py.guarded, Some(young));
        assert!(!app.world().get::<Navigator>(beast).unwrap().is_idle());
    }

    #[test]
    fn protect_young_action_succeeds_when_close_enough() {
        let mut app = build_protect_young_action_app();
        let territory = Territory::new(DVec3::new(0.0, 64.0, 0.0), 20.0);
        let beast = spawn_beast_with_territory(&mut app, DVec3::new(0.0, 64.0, 0.0), territory);
        app.world_mut()
            .entity_mut(beast)
            .insert(ProtectYoungState::default());
        let _young = spawn_injured_young(&mut app, DVec3::new(1.0, 64.0, 1.0), 0.2); // 距离 1.4，< 4.0

        let action = app
            .world_mut()
            .spawn((Actor(beast), ProtectYoungAction, ActionState::Requested))
            .id();
        // 第一 tick → Executing；第二 tick 发现距离 < 4 → Success
        app.update();
        app.update();
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success
        );
    }
}
