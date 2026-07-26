use big_brain::prelude::{ActionBuilder, ActionState, Actor, BigBrainSet, Score, ScorerBuilder};
use valence::client::ClientMarker;
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, EventWriter, IntoSystemConfigs, ParamSet,
    Position, PreUpdate, Query, Res, With, Without,
};

use crate::combat::rat_bite::RatBiteEvent;
use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;
use crate::fauna::rat_phase::{
    chunk_pos_from_world, is_drained_chunk, remember_drained_chunk, MeditatingState, RatGroupId,
    RatPhase,
};
use crate::npc::navigator::Navigator;
use crate::npc::spawn::NpcMarker;
use crate::npc::spawn_rat::RatBlackboard;
use crate::world::dimension::{CurrentDimension, DimensionKind};

const QI_SOURCE_SCAN_RANGE: f64 = 32.0;
/// 冲近到此距离内即视为"咬到"。**必须 ≥ navigator 的 `GOAL_REACH_XZ`（2 格）**：navigator 沿
/// A* 路点走、到路径终点（离目标最多 2 格）就停（`navigator.rs` 路径耗尽即 stay-put），且鼠与
/// 玩家有碰撞体到不了同格。取 0.8 时鼠停在 2 格外永远够不到 → "只跟着不咬"（实测）。取 2.0
/// 让它从自然停距发起啄咬；仍 < harass "远距离不咬"测试用的 3.0，不破测试。
const QI_SOURCE_ARRIVAL_DISTANCE: f64 = 2.0;
const QI_SOURCE_SPEED_FACTOR: f64 = 1.0;
const REGROUP_SUCCESS_DISTANCE: f64 = 4.0;
const REGROUP_SPEED_FACTOR: f64 = 1.05;
const GROUP_COHESION_RADIUS: f64 = 16.0;
const MEDITATING_QI_SOURCE_WEIGHT: f32 = 3.0;

// plan-ambient-threat-v1 P2 —— rat 袭扰行为常数。
/// 玩家 harass 起评的最大距离（格）。比 `QI_SOURCE_SCAN_RANGE`（32）近得多——
/// 表达"贴身骚扰"而非通用 qi 源索敌。
const PLAYER_HARASS_RANGE: f64 = 8.0;
/// 冲近到此距离内即视为"咬到"，与 `QI_SOURCE_ARRIVAL_DISTANCE` 同量级但语义独立。
/// 同样须 ≥ navigator 停距（见 `QI_SOURCE_ARRIVAL_DISTANCE` 注释），否则骚扰追而不咬。
const PLAYER_HARASS_ARRIVAL_DISTANCE: f64 = 2.0;
/// "冲近咬一口"比常规索敌更急——纳维游戏速度系数略高于 `QI_SOURCE_SPEED_FACTOR`。
const PLAYER_HARASS_SPEED_FACTOR: f64 = 1.3;
/// 咬完立即进入 20s 逃逸/游荡冷却（20 tick/s × 20s）。冷却期间
/// `PlayerHarassScorer` 恒为 0，交回 `QiSourceProximityScorer`/`WanderScorer`。
const PLAYER_HARASS_COOLDOWN_TICKS: u64 = 20 * 20;

type RegroupReadQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Position, &'static RatGroupId), With<NpcMarker>>;
type QiSourceRatQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static RatBlackboard,
    ),
    With<NpcMarker>,
>;
type QiSourceTargetQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static Cultivation,
        Option<&'static MeditatingState>,
    ),
    Without<NpcMarker>,
>;
/// plan-ambient-threat-v1 P2 —— harass 目标必须是真实在线玩家（`ClientMarker`），
/// 区别于 `QiSourceTargetQuery`（任何 `Without<NpcMarker>` 的 qi 源，含非玩家 NPC 修士）。
type HarassPlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static Cultivation,
    ),
    (With<ClientMarker>, Without<NpcMarker>),
>;
/// `RatPhase` 是 `Option`：教程/脚本刷出的鼠可能没挂相位组件，
/// 缺席按 `Solitary`（独行）处理，只影响招式选择，不影响咬击本身。
type SeekRatQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static mut RatBlackboard,
        &'static mut Navigator,
        Option<&'static RatPhase>,
    ),
    With<NpcMarker>,
>;
type RegroupNavigateQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        &'static RatGroupId,
        &'static mut Navigator,
    ),
    With<NpcMarker>,
>;

#[derive(Clone, Copy, Debug, Component)]
pub struct QiSourceProximityScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct GroupCohesionScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct DrainedChunkAvoidScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct SeekQiSourceAction;

#[derive(Clone, Copy, Debug, Component)]
pub struct RegroupAction;

/// plan-ambient-threat-v1 P2 —— "低威胁骚扰"起评器：玩家 ≤ 8 格且 rat 冷却就绪。
/// 必须排在 `rat_npc_thinker` 的 `QiSourceProximityScorer` 之前（见 `spawn_rat.rs`），
/// 与之互斥编排，避免同一玩家目标被两条分支各自 `emit RatBiteEvent` 造成双倍咬击。
#[derive(Clone, Copy, Debug, Component)]
pub struct PlayerHarassScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct HarassBiteAction;

/// plan-devour-rat-model P3 —— 噬元鼠出招种类，一一对应
/// `devour_rat.animation.json` 里的一次性招式动画。
///
/// 语境映射（见各 action system 的 emit 点）：
/// - [`RatAttackKind::Peck`]：`SeekQiSourceAction` 贴脸吸元——低头啄咬，最轻的一招。
/// - [`RatAttackKind::Claw`]：独行/过渡期鼠的 `HarassBiteAction`——前爪抓挠骚扰。
/// - [`RatAttackKind::Pounce`]：群居（`RatPhase::Gregarious`）鼠的 `HarassBiteAction`
///   ——成群才敢整只扑上来，视觉上把"鼠患成潮"和"独鼠试探"区分开。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RatAttackKind {
    Peck,
    Claw,
    Pounce,
}

/// plan-devour-rat-model P3 —— 鼠成功咬中那帧 emit；
/// `network::rat_av_trigger` 消费并转成 `VfxEventPayloadV1::PlayEntityAnim`
/// （按 MC 协议 entity_id 定位客户端 `FaunaEntity` 播一次性招式动画）。
///
/// 与 `RatBiteEvent` 分开的原因：`RatBiteEvent` 是**伤害/守恒**事件（4 个 emit 点，含
/// 教程兽潮与蝗群脚本），只带 `qi_steal`，读不出"用的是哪一招"；招式表现需要 action
/// 语境（吸元 / 独鼠骚扰 / 群鼠扑击），故由 action system 单独 emit。
#[derive(Clone, Copy, Debug, PartialEq, valence::prelude::Event)]
pub struct RatAttackAvEvent {
    /// 出招的鼠（ECS Entity）；消费方据此查 `EntityId` 取 MC 协议 id。
    pub rat: Entity,
    pub kind: RatAttackKind,
    /// 出招位置（鼠自身坐标）——`VfxEventRequest` 按此做 64 格广播过滤。
    pub origin: DVec3,
}

impl ScorerBuilder for QiSourceProximityScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("QiSourceProximityScorer")
    }
}

impl ScorerBuilder for GroupCohesionScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("GroupCohesionScorer")
    }
}

impl ScorerBuilder for DrainedChunkAvoidScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("DrainedChunkAvoidScorer")
    }
}

impl ActionBuilder for SeekQiSourceAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("SeekQiSourceAction")
    }
}

impl ActionBuilder for RegroupAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("RegroupAction")
    }
}

impl ScorerBuilder for PlayerHarassScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("PlayerHarassScorer")
    }
}

impl ActionBuilder for HarassBiteAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("HarassBiteAction")
    }
}

pub fn register(app: &mut App) {
    // plan-devour-rat-model P3 —— 招式动画事件；消费方见 `network::rat_av_trigger`。
    app.add_event::<RatAttackAvEvent>();
    app.add_systems(
        PreUpdate,
        (
            qi_source_proximity_scorer_system,
            group_cohesion_scorer_system,
            drained_chunk_avoid_scorer_system,
            player_harass_scorer_system,
        )
            .in_set(BigBrainSet::Scorers),
    )
    .add_systems(
        PreUpdate,
        (
            seek_qi_source_action_system,
            regroup_action_system,
            harass_bite_action_system,
        )
            .in_set(BigBrainSet::Actions),
    );
}

fn qi_source_proximity_scorer_system(
    rats: QiSourceRatQuery<'_, '_>,
    targets: QiSourceTargetQuery<'_, '_>,
    mut scorers: Query<(&Actor, &mut Score), With<QiSourceProximityScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let Ok((position, dimension, blackboard)) = rats.get(*actor) else {
            score.set(0.0);
            continue;
        };
        let current_chunk = chunk_pos_from_world(position.get());
        if is_drained_chunk(blackboard, current_chunk) {
            score.set(0.0);
            continue;
        }
        let value = nearest_qi_source_entity(position.get(), dimension_kind(dimension), &targets)
            .map(|source| qi_source_score(position.get(), source.position, source.weight))
            .unwrap_or(0.0);
        score.set(value);
    }
}

fn group_cohesion_scorer_system(
    rats: Query<(Entity, &Position, &RatGroupId), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<GroupCohesionScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let Ok((_, position, group_id)) = rats.get(*actor) else {
            score.set(0.0);
            continue;
        };
        let Some(centroid) = group_centroid(group_id.0, actor, &rats) else {
            score.set(0.0);
            continue;
        };
        let distance = xz_distance(position.get(), centroid);
        score.set((distance / GROUP_COHESION_RADIUS).clamp(0.0, 1.0) as f32);
    }
}

fn drained_chunk_avoid_scorer_system(
    rats: Query<(&Position, &RatBlackboard), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<DrainedChunkAvoidScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = rats
            .get(*actor)
            .map(|(position, blackboard)| {
                if is_drained_chunk(blackboard, chunk_pos_from_world(position.get())) {
                    1.0
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        score.set(value);
    }
}

/// plan-ambient-threat-v1 P2 —— 玩家 harass 起评：距离门 + 冷却门都必须满足才起评，
/// 否则恒为 0（让 `QiSourceProximityScorer`/`WanderScorer` 接手）。
fn player_harass_scorer_system(
    clock: Option<Res<CombatClock>>,
    rats: QiSourceRatQuery<'_, '_>,
    players: HarassPlayerQuery<'_, '_>,
    mut scorers: Query<(&Actor, &mut Score), With<PlayerHarassScorer>>,
) {
    let tick = clock.map(|clock| clock.tick).unwrap_or(0);
    for (Actor(actor), mut score) in &mut scorers {
        let Ok((position, dimension, blackboard)) = rats.get(*actor) else {
            score.set(0.0);
            continue;
        };
        if blackboard.harass_cooldown_until_tick > tick {
            score.set(0.0);
            continue;
        }
        let has_target =
            nearest_harass_player(position.get(), dimension_kind(dimension), &players).is_some();
        score.set(if has_target { 1.0 } else { 0.0 });
    }
}

fn seek_qi_source_action_system(
    mut rats: SeekRatQuery<'_, '_>,
    targets: QiSourceTargetQuery<'_, '_>,
    mut bites: EventWriter<RatBiteEvent>,
    mut attack_av: EventWriter<RatAttackAvEvent>,
    mut actions: Query<(&Actor, &mut ActionState), With<SeekQiSourceAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((position, dimension, mut blackboard, mut navigator, _phase)) = rats.get_mut(*actor)
        else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => *state = ActionState::Executing,
            ActionState::Executing => {
                let Some(source) =
                    nearest_qi_source_entity(position.get(), dimension_kind(dimension), &targets)
                else {
                    navigator.stop();
                    *state = ActionState::Success;
                    continue;
                };
                blackboard.last_pressure_target = Some(source.position);
                if position.get().distance(source.position) <= QI_SOURCE_ARRIVAL_DISTANCE {
                    bites.send(RatBiteEvent {
                        rat: *actor,
                        target: source.entity,
                        qi_steal: 1,
                    });
                    // P3：贴脸吸元 = 低头啄咬（peck）。
                    attack_av.send(RatAttackAvEvent {
                        rat: *actor,
                        kind: RatAttackKind::Peck,
                        origin: position.get(),
                    });
                    remember_drained_chunk(&mut blackboard, chunk_pos_from_world(position.get()));
                    navigator.stop();
                    *state = ActionState::Success;
                } else {
                    navigator.set_goal(source.position, QI_SOURCE_SPEED_FACTOR);
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

/// plan-ambient-threat-v1 P2 —— "冲近咬一口"：够近直接咬 + 记冷却 + 记 drained chunk
/// （复用 `SeekQiSourceAction` 的 drained-chunk 避让基建：咬完后 `DrainedChunkAvoidScorer`
/// 会在原地暂时压过其余分支，天然充当"逃逸游荡"的替身，不必新建独立 flee 状态机）。
fn harass_bite_action_system(
    clock: Option<Res<CombatClock>>,
    mut rats: SeekRatQuery<'_, '_>,
    players: HarassPlayerQuery<'_, '_>,
    mut bites: EventWriter<RatBiteEvent>,
    mut attack_av: EventWriter<RatAttackAvEvent>,
    mut actions: Query<(&Actor, &mut ActionState), With<HarassBiteAction>>,
) {
    let tick = clock.map(|clock| clock.tick).unwrap_or(0);
    for (Actor(actor), mut state) in &mut actions {
        let Ok((position, dimension, mut blackboard, mut navigator, phase)) = rats.get_mut(*actor)
        else {
            *state = ActionState::Failure;
            continue;
        };
        match *state {
            ActionState::Requested => *state = ActionState::Executing,
            ActionState::Executing => {
                let Some((target, target_position)) =
                    nearest_harass_player(position.get(), dimension_kind(dimension), &players)
                else {
                    navigator.stop();
                    *state = ActionState::Success;
                    continue;
                };
                if position.get().distance(target_position) <= PLAYER_HARASS_ARRIVAL_DISTANCE {
                    bites.send(RatBiteEvent {
                        rat: *actor,
                        target,
                        qi_steal: 1,
                    });
                    // 群居鼠敢整只扑上来，独行/过渡期鼠只敢前爪抓一下。相位组件缺席按独行处理。
                    // 就地算：`Init/Success/Failure` 分支用不到，没必要每 actor 每 tick 都算。
                    attack_av.send(RatAttackAvEvent {
                        rat: *actor,
                        kind: if matches!(phase, Some(RatPhase::Gregarious)) {
                            RatAttackKind::Pounce
                        } else {
                            RatAttackKind::Claw
                        },
                        origin: position.get(),
                    });
                    blackboard.harass_cooldown_until_tick = tick + PLAYER_HARASS_COOLDOWN_TICKS;
                    remember_drained_chunk(&mut blackboard, chunk_pos_from_world(position.get()));
                    navigator.stop();
                    *state = ActionState::Success;
                } else {
                    navigator.set_goal(target_position, PLAYER_HARASS_SPEED_FACTOR);
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

fn regroup_action_system(
    mut rats: ParamSet<(RegroupReadQuery<'_, '_>, RegroupNavigateQuery<'_, '_>)>,
    mut actions: Query<(&Actor, &mut ActionState), With<RegroupAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let group_id = {
            let group_query = rats.p0();
            let Ok((_, _, group_id)) = group_query.get(*actor) else {
                *state = ActionState::Failure;
                continue;
            };
            group_id.0
        };
        let centroid = {
            let group_query = rats.p0();
            let Some(centroid) = group_centroid(group_id, actor, &group_query) else {
                *state = ActionState::Success;
                continue;
            };
            centroid
        };
        let mut actor_query = rats.p1();
        let Ok((_, position, _, mut navigator)) = actor_query.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => *state = ActionState::Executing,
            ActionState::Executing => {
                if xz_distance(position.get(), centroid) <= REGROUP_SUCCESS_DISTANCE {
                    navigator.stop();
                    *state = ActionState::Success;
                } else {
                    navigator.set_goal(centroid, REGROUP_SPEED_FACTOR);
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct QiSource {
    entity: Entity,
    position: DVec3,
    weight: f32,
}

fn nearest_qi_source_entity(
    origin: DVec3,
    origin_dimension: DimensionKind,
    targets: &QiSourceTargetQuery<'_, '_>,
) -> Option<QiSource> {
    targets
        .iter()
        .filter(|(_, _, dimension, cultivation, _)| {
            cultivation.qi_current > 0.0 && dimension_kind(*dimension) == origin_dimension
        })
        .map(|(entity, position, _, _, meditating)| QiSource {
            entity,
            position: position.get(),
            weight: if meditating.is_some() {
                MEDITATING_QI_SOURCE_WEIGHT
            } else {
                1.0
            },
        })
        .filter(|source| origin.distance(source.position) <= QI_SOURCE_SCAN_RANGE)
        .max_by(|left, right| {
            qi_source_score(origin, left.position, left.weight).total_cmp(&qi_source_score(
                origin,
                right.position,
                right.weight,
            ))
        })
}

/// plan-ambient-threat-v1 P2 —— 最近的可骚扰玩家（`PLAYER_HARASS_RANGE` 内、同维度）。
/// 与 `nearest_qi_source_entity` 独立：只看真实玩家（`ClientMarker`），不含 meditating
/// 权重加成，用 `min_by` 取最近（harass 是"贴身"而非"最优 qi 源"语义）。
fn nearest_harass_player(
    origin: DVec3,
    origin_dimension: DimensionKind,
    players: &HarassPlayerQuery<'_, '_>,
) -> Option<(Entity, DVec3)> {
    players
        .iter()
        .filter(|(_, _, dimension, cultivation)| {
            cultivation.qi_current > 0.0 && dimension_kind(*dimension) == origin_dimension
        })
        .map(|(entity, position, _, _)| (entity, position.get()))
        .filter(|(_, position)| origin.distance(*position) <= PLAYER_HARASS_RANGE)
        .min_by(|(_, left), (_, right)| origin.distance(*left).total_cmp(&origin.distance(*right)))
}

fn dimension_kind(dimension: Option<&CurrentDimension>) -> DimensionKind {
    dimension.map(|dim| dim.0).unwrap_or_default()
}

fn qi_source_score(origin: DVec3, source: DVec3, weight: f32) -> f32 {
    let distance_score = 1.0 - (origin.distance(source) / QI_SOURCE_SCAN_RANGE).clamp(0.0, 1.0);
    (distance_score as f32 * weight).clamp(0.0, 1.0)
}

fn group_centroid(
    group_id: u64,
    exclude: &Entity,
    rats: &Query<(Entity, &Position, &RatGroupId), With<NpcMarker>>,
) -> Option<DVec3> {
    group_centroid_from_iter(
        group_id,
        *exclude,
        rats.iter()
            .map(|(entity, position, group)| (entity, position.get(), *group)),
    )
}

fn group_centroid_from_iter<I>(group_id: u64, exclude: Entity, rats: I) -> Option<DVec3>
where
    I: IntoIterator<Item = (Entity, DVec3, RatGroupId)>,
{
    let mut sum = DVec3::ZERO;
    let mut count = 0.0;
    for (entity, position, group) in rats {
        if entity == exclude || group.0 != group_id {
            continue;
        }
        sum += position;
        count += 1.0;
    }
    (count > 0.0).then_some(sum / count)
}

fn xz_distance(a: DVec3, b: DVec3) -> f64 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, EventReader, Events, ResMut, Resource, Update};

    use crate::cultivation::components::{Cultivation, Realm};

    fn cultivation(qi_current: f64) -> Cultivation {
        Cultivation {
            realm: Realm::Induce,
            qi_current,
            qi_max: 10.0,
            ..Default::default()
        }
    }

    fn rat_blackboard_with_cooldown(harass_cooldown_until_tick: u64) -> RatBlackboard {
        RatBlackboard {
            home_chunk: crate::fauna::rat_phase::chunk_pos_from_world(DVec3::ZERO),
            home_zone: "spawn".to_string(),
            group_id: RatGroupId(7),
            last_pressure_target: None,
            recently_drained: Vec::new(),
            drained_qi: 0.0,
            harass_cooldown_until_tick,
        }
    }

    #[test]
    fn qi_source_proximity_scorer_ranks_nearest_meditator_first() {
        let origin = DVec3::ZERO;
        let near = DVec3::new(8.0, 0.0, 0.0);
        let far = DVec3::new(20.0, 0.0, 0.0);

        assert!(
            qi_source_score(origin, near, 1.0) > qi_source_score(origin, far, 1.0),
            "closer qi sources should score higher at equal weight"
        );
        assert!(
            qi_source_score(origin, far, MEDITATING_QI_SOURCE_WEIGHT)
                > qi_source_score(origin, near, 1.0),
            "meditating qi sources should carry the plan's 修炼苍蝇 weight"
        );
    }

    #[test]
    fn group_cohesion_pulls_lone_rat_back_to_centroid() {
        let group = RatGroupId(7);
        let lone = Entity::from_raw(1);
        let rats = [
            (lone, DVec3::new(32.0, 64.0, 0.0), group),
            (Entity::from_raw(2), DVec3::new(0.0, 64.0, 0.0), group),
            (Entity::from_raw(3), DVec3::new(0.0, 64.0, 16.0), group),
        ];

        let centroid = group_centroid_from_iter(group.0, lone, rats)
            .expect("other rats in group should define a centroid");

        assert_eq!(centroid, DVec3::new(0.0, 64.0, 8.0));
    }

    #[test]
    fn seek_qi_source_action_triggers_rat_bite_at_close_range() {
        let mut app = App::new();
        app.add_event::<RatBiteEvent>();
        app.add_event::<RatAttackAvEvent>();
        app.add_systems(Update, seek_qi_source_action_system);
        let rat = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                RatBlackboard {
                    home_chunk: crate::fauna::rat_phase::chunk_pos_from_world(DVec3::ZERO),
                    home_zone: "spawn".to_string(),
                    group_id: RatGroupId(7),
                    last_pressure_target: None,
                    recently_drained: Vec::new(),
                    drained_qi: 0.0,
                    harass_cooldown_until_tick: 0,
                },
                Navigator::new(),
            ))
            .id();
        let target = app
            .world_mut()
            .spawn((
                Position::new([0.2, 64.0, 0.0]),
                cultivation(5.0),
                MeditatingState { since_tick: 1 },
            ))
            .id();
        app.world_mut()
            .spawn((Actor(rat), ActionState::Executing, SeekQiSourceAction));

        app.update();

        let bites = app.world().resource::<Events<RatBiteEvent>>();
        let event = bites
            .iter_current_update_events()
            .next()
            .expect("close qi source should emit RatBiteEvent");
        assert_eq!(event.rat, rat);
        assert_eq!(event.target, target);
        assert_eq!(event.qi_steal, 1);
    }

    #[test]
    fn seek_qi_source_action_filters_targets_by_dimension() {
        let mut app = App::new();
        app.add_event::<RatBiteEvent>();
        app.add_event::<RatAttackAvEvent>();
        app.add_systems(Update, seek_qi_source_action_system);
        let rat = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                CurrentDimension(DimensionKind::Overworld),
                RatBlackboard {
                    home_chunk: crate::fauna::rat_phase::chunk_pos_from_world(DVec3::ZERO),
                    home_zone: "spawn".to_string(),
                    group_id: RatGroupId(7),
                    last_pressure_target: None,
                    recently_drained: Vec::new(),
                    drained_qi: 0.0,
                    harass_cooldown_until_tick: 0,
                },
                Navigator::new(),
            ))
            .id();
        let cross_dimension_target = app
            .world_mut()
            .spawn((
                Position::new([0.1, 64.0, 0.0]),
                CurrentDimension(DimensionKind::Tsy),
                cultivation(5.0),
            ))
            .id();
        let same_dimension_target = app
            .world_mut()
            .spawn((
                Position::new([0.3, 64.0, 0.0]),
                CurrentDimension(DimensionKind::Overworld),
                cultivation(5.0),
            ))
            .id();
        app.world_mut()
            .spawn((Actor(rat), ActionState::Executing, SeekQiSourceAction));

        app.update();

        let bites = app.world().resource::<Events<RatBiteEvent>>();
        let event = bites
            .iter_current_update_events()
            .next()
            .expect("same-dimension qi source in bite range should be selected");
        assert_eq!(event.rat, rat);
        assert_eq!(event.target, same_dimension_target);
        assert_ne!(
            event.target, cross_dimension_target,
            "rats must not bite qi targets from another dimension"
        );
    }

    // --- plan-ambient-threat-v1 P2: PlayerHarassScorer ------------------------------

    fn harass_scorer_app() -> App {
        let mut app = App::new();
        app.add_systems(Update, player_harass_scorer_system);
        app
    }

    fn spawn_harass_rat(app: &mut App, cooldown_until_tick: u64) -> Entity {
        app.world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                rat_blackboard_with_cooldown(cooldown_until_tick),
            ))
            .id()
    }

    fn spawn_harass_player(app: &mut App, distance: f64) -> Entity {
        app.world_mut()
            .spawn((
                ClientMarker,
                Position::new([distance, 64.0, 0.0]),
                cultivation(5.0),
            ))
            .id()
    }

    fn score_of(app: &mut App, scorer_entity: Entity) -> f32 {
        app.update();
        app.world()
            .get::<Score>(scorer_entity)
            .expect("scorer entity must carry a Score component")
            .get()
    }

    #[test]
    fn player_harass_scorer_scores_zero_when_player_beyond_range() {
        let mut app = harass_scorer_app();
        let rat = spawn_harass_rat(&mut app, 0);
        spawn_harass_player(&mut app, PLAYER_HARASS_RANGE + 0.01);
        let scorer = app
            .world_mut()
            .spawn((Actor(rat), Score::default(), PlayerHarassScorer))
            .id();

        assert_eq!(
            score_of(&mut app, scorer),
            0.0,
            "player just outside PLAYER_HARASS_RANGE (8.0) must not trigger harass"
        );
    }

    #[test]
    fn player_harass_scorer_scores_positive_at_range_boundary() {
        let mut app = harass_scorer_app();
        let rat = spawn_harass_rat(&mut app, 0);
        spawn_harass_player(&mut app, PLAYER_HARASS_RANGE);
        let scorer = app
            .world_mut()
            .spawn((Actor(rat), Score::default(), PlayerHarassScorer))
            .id();

        assert!(
            score_of(&mut app, scorer) > 0.0,
            "player exactly at PLAYER_HARASS_RANGE (8.0, inclusive <=) must still trigger harass"
        );
    }

    #[test]
    fn player_harass_scorer_scores_zero_during_active_cooldown() {
        let mut app = harass_scorer_app();
        app.insert_resource(CombatClock { tick: 50 });
        // cooldown_until_tick(100) > current tick(50) → 仍在冷却，恒 0。
        let rat = spawn_harass_rat(&mut app, 100);
        spawn_harass_player(&mut app, 1.0);
        let scorer = app
            .world_mut()
            .spawn((Actor(rat), Score::default(), PlayerHarassScorer))
            .id();

        assert_eq!(
            score_of(&mut app, scorer),
            0.0,
            "rat still on harass cooldown must not re-trigger even with a player right next to it"
        );
    }

    #[test]
    fn player_harass_scorer_scores_positive_once_cooldown_elapses() {
        let mut app = harass_scorer_app();
        app.insert_resource(CombatClock { tick: 100 });
        // cooldown_until_tick(100) 不 > 当前 tick(100) → 冷却已到期（边界含等）。
        let rat = spawn_harass_rat(&mut app, 100);
        spawn_harass_player(&mut app, 1.0);
        let scorer = app
            .world_mut()
            .spawn((Actor(rat), Score::default(), PlayerHarassScorer))
            .id();

        assert!(
            score_of(&mut app, scorer) > 0.0,
            "cooldown boundary tick == harass_cooldown_until_tick must count as elapsed, not still-cooling"
        );
    }

    #[test]
    fn player_harass_scorer_ignores_non_player_qi_sources() {
        // 无 ClientMarker 的普通修士型 qi 源（例如 scattered cultivator NPC）不应触发
        // harass——区别于 QiSourceProximityScorer 的"任意 qi 源"语义。
        let mut app = harass_scorer_app();
        let rat = spawn_harass_rat(&mut app, 0);
        app.world_mut()
            .spawn((Position::new([1.0, 64.0, 0.0]), cultivation(5.0)));
        let scorer = app
            .world_mut()
            .spawn((Actor(rat), Score::default(), PlayerHarassScorer))
            .id();

        assert_eq!(
            score_of(&mut app, scorer),
            0.0,
            "non-ClientMarker qi sources must not count as harass targets"
        );
    }

    // --- plan-ambient-threat-v1 P2: HarassBiteAction --------------------------------

    #[test]
    fn harass_bite_action_triggers_rat_bite_at_close_range_and_sets_cooldown() {
        let mut app = App::new();
        app.add_event::<RatBiteEvent>();
        app.add_event::<RatAttackAvEvent>();
        app.insert_resource(CombatClock { tick: 200 });
        app.add_systems(Update, harass_bite_action_system);
        let rat = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                rat_blackboard_with_cooldown(0),
                Navigator::new(),
            ))
            .id();
        let target = app
            .world_mut()
            .spawn((
                ClientMarker,
                Position::new([0.2, 64.0, 0.0]),
                cultivation(5.0),
            ))
            .id();
        app.world_mut()
            .spawn((Actor(rat), ActionState::Executing, HarassBiteAction));

        app.update();

        let bites = app.world().resource::<Events<RatBiteEvent>>();
        let event = bites
            .iter_current_update_events()
            .next()
            .expect("close-range player should be bitten by HarassBiteAction");
        assert_eq!(event.rat, rat);
        assert_eq!(event.target, target);
        assert_eq!(event.qi_steal, 1);

        let blackboard = app
            .world()
            .get::<RatBlackboard>(rat)
            .expect("rat must retain its RatBlackboard");
        assert_eq!(
            blackboard.harass_cooldown_until_tick,
            200 + PLAYER_HARASS_COOLDOWN_TICKS,
            "biting must arm a fresh 20s (400 tick) escape cooldown from the current tick"
        );
    }

    #[test]
    fn harass_bite_action_navigates_without_biting_when_out_of_arrival_range() {
        let mut app = App::new();
        app.add_event::<RatBiteEvent>();
        app.add_event::<RatAttackAvEvent>();
        app.insert_resource(CombatClock { tick: 10 });
        app.add_systems(Update, harass_bite_action_system);
        let rat = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                rat_blackboard_with_cooldown(0),
                Navigator::new(),
            ))
            .id();
        // 在 harass 起评范围内(<=8)但超出咬击到达距离(>0.8)——应冲近而非直接咬。
        app.world_mut().spawn((
            ClientMarker,
            Position::new([3.0, 64.0, 0.0]),
            cultivation(5.0),
        ));
        app.world_mut()
            .spawn((Actor(rat), ActionState::Executing, HarassBiteAction));

        app.update();

        assert!(
            app.world().resource::<Events<RatBiteEvent>>().is_empty(),
            "rat still rushing in (distance 3.0 > arrival 0.8) must not have bitten yet"
        );
        let navigator = app
            .world()
            .get::<Navigator>(rat)
            .expect("rat must retain its Navigator");
        assert!(
            !navigator.is_idle(),
            "HarassBiteAction must set a navigation goal while rushing toward the player"
        );
        let blackboard = app.world().get::<RatBlackboard>(rat).unwrap();
        assert_eq!(
            blackboard.harass_cooldown_until_tick, 0,
            "cooldown must only be armed on an actual bite, not while still rushing in"
        );
    }

    // --- plan-devour-rat-model P3: 咬击 → 招式动画事件 -------------------------------

    /// 抽出本 tick emit 的 (rat, kind)。
    fn drain_attack_av(app: &App) -> Vec<(Entity, RatAttackKind)> {
        app.world()
            .resource::<Events<RatAttackAvEvent>>()
            .iter_current_update_events()
            .map(|event| (event.rat, event.kind))
            .collect()
    }

    /// 会导航的 harass 鼠（比 `spawn_harass_rat` 多挂 `Navigator`，并可选相位组件）。
    fn spawn_harass_rat_with_phase(app: &mut App, phase: Option<RatPhase>) -> Entity {
        let mut entity = app.world_mut().spawn((
            NpcMarker,
            Position::new([0.0, 64.0, 0.0]),
            rat_blackboard_with_cooldown(0),
            Navigator::new(),
        ));
        if let Some(phase) = phase {
            entity.insert(phase);
        }
        entity.id()
    }

    fn harass_bite_app() -> App {
        let mut app = App::new();
        app.add_event::<RatBiteEvent>();
        app.add_event::<RatAttackAvEvent>();
        app.insert_resource(CombatClock { tick: 0 });
        app.add_systems(Update, harass_bite_action_system);
        app
    }

    /// 贴脸吸元 = peck。
    #[test]
    fn seek_qi_source_bite_emits_peck_attack_av() {
        let mut app = App::new();
        app.add_event::<RatBiteEvent>();
        app.add_event::<RatAttackAvEvent>();
        app.add_systems(Update, seek_qi_source_action_system);
        let rat = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                rat_blackboard_with_cooldown(0),
                Navigator::new(),
            ))
            .id();
        app.world_mut()
            .spawn((Position::new([0.2, 64.0, 0.0]), cultivation(5.0)));
        app.world_mut()
            .spawn((Actor(rat), ActionState::Executing, SeekQiSourceAction));

        app.update();

        assert_eq!(
            drain_attack_av(&app),
            vec![(rat, RatAttackKind::Peck)],
            "贴脸吸元应 emit Peck（低头啄咬）——没有招式事件 = 客户端只播 idle，\
             玩家看不出鼠在咬人"
        );
    }

    /// 独行鼠骚扰 = claw。
    #[test]
    fn solitary_harass_bite_emits_claw_attack_av() {
        let mut app = harass_bite_app();
        let rat = spawn_harass_rat_with_phase(&mut app, Some(RatPhase::Solitary));
        app.world_mut().spawn((
            ClientMarker,
            Position::new([0.2, 64.0, 0.0]),
            cultivation(5.0),
        ));
        app.world_mut()
            .spawn((Actor(rat), ActionState::Executing, HarassBiteAction));

        app.update();

        assert_eq!(
            drain_attack_av(&app),
            vec![(rat, RatAttackKind::Claw)],
            "独行鼠只敢前爪抓挠（Claw），不该用群鼠才敢的扑击"
        );
    }

    /// 群居鼠骚扰 = pounce（与独行鼠视觉可辨）。
    #[test]
    fn gregarious_harass_bite_emits_pounce_attack_av() {
        let mut app = harass_bite_app();
        let rat = spawn_harass_rat_with_phase(&mut app, Some(RatPhase::Gregarious));
        app.world_mut().spawn((
            ClientMarker,
            Position::new([0.2, 64.0, 0.0]),
            cultivation(5.0),
        ));
        app.world_mut()
            .spawn((Actor(rat), ActionState::Executing, HarassBiteAction));

        app.update();

        assert_eq!(
            drain_attack_av(&app),
            vec![(rat, RatAttackKind::Pounce)],
            "群居（Gregarious）鼠应扑击（Pounce）——鼠患成潮与独鼠试探必须视觉可辨"
        );
    }

    /// 过渡相位仍算"没成群"，走 Claw。
    #[test]
    fn transitioning_harass_bite_still_emits_claw() {
        let mut app = harass_bite_app();
        let rat =
            spawn_harass_rat_with_phase(&mut app, Some(RatPhase::Transitioning { progress: 300 }));
        app.world_mut().spawn((
            ClientMarker,
            Position::new([0.2, 64.0, 0.0]),
            cultivation(5.0),
        ));
        app.world_mut()
            .spawn((Actor(rat), ActionState::Executing, HarassBiteAction));

        app.update();

        assert_eq!(
            drain_attack_av(&app),
            vec![(rat, RatAttackKind::Claw)],
            "Transitioning 还没成群，应仍是 Claw；只有 Gregarious 才升级为 Pounce"
        );
    }

    /// 无 `RatPhase` 组件（教程/脚本刷出的鼠）按独行处理，不 panic、不漏 emit。
    #[test]
    fn harass_bite_without_rat_phase_component_falls_back_to_claw() {
        let mut app = harass_bite_app();
        let rat = spawn_harass_rat_with_phase(&mut app, None);
        app.world_mut().spawn((
            ClientMarker,
            Position::new([0.2, 64.0, 0.0]),
            cultivation(5.0),
        ));
        app.world_mut()
            .spawn((Actor(rat), ActionState::Executing, HarassBiteAction));

        app.update();

        assert_eq!(
            drain_attack_av(&app),
            vec![(rat, RatAttackKind::Claw)],
            "缺 RatPhase 组件应按独行（Claw）兜底，而不是漏 emit 让鼠无动画"
        );
    }

    /// 还在冲刺（未到咬击距离）不该 emit 招式动画——否则鼠会在半路上凭空挥爪。
    #[test]
    fn harass_bite_out_of_range_emits_no_attack_av() {
        let mut app = harass_bite_app();
        let rat = spawn_harass_rat_with_phase(&mut app, Some(RatPhase::Gregarious));
        app.world_mut().spawn((
            ClientMarker,
            Position::new([3.0, 64.0, 0.0]),
            cultivation(5.0),
        ));
        app.world_mut()
            .spawn((Actor(rat), ActionState::Executing, HarassBiteAction));

        app.update();

        assert!(
            drain_attack_av(&app).is_empty(),
            "距离 3.0 > 到达距离 0.8，鼠还在冲刺，不该播招式动画，实际 {:?}",
            drain_attack_av(&app)
        );
    }

    /// 没有目标时（qi 源全空）既不咬也不出招。
    #[test]
    fn seek_qi_source_without_target_emits_no_attack_av() {
        let mut app = App::new();
        app.add_event::<RatBiteEvent>();
        app.add_event::<RatAttackAvEvent>();
        app.add_systems(Update, seek_qi_source_action_system);
        let rat = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                rat_blackboard_with_cooldown(0),
                Navigator::new(),
            ))
            .id();
        app.world_mut()
            .spawn((Actor(rat), ActionState::Executing, SeekQiSourceAction));

        app.update();

        assert!(
            drain_attack_av(&app).is_empty(),
            "无 qi 源目标时不该 emit 招式动画"
        );
    }

    /// 招式事件必须与咬击事件**同帧成对**出现：漏了任一边都是接线断裂。
    #[test]
    fn attack_av_and_bite_are_emitted_together() {
        let mut app = harass_bite_app();
        let rat = spawn_harass_rat_with_phase(&mut app, Some(RatPhase::Gregarious));
        app.world_mut().spawn((
            ClientMarker,
            Position::new([0.2, 64.0, 0.0]),
            cultivation(5.0),
        ));
        app.world_mut()
            .spawn((Actor(rat), ActionState::Executing, HarassBiteAction));

        app.update();

        let bites: Vec<Entity> = app
            .world()
            .resource::<Events<RatBiteEvent>>()
            .iter_current_update_events()
            .map(|bite| bite.rat)
            .collect();
        let attacks: Vec<Entity> = drain_attack_av(&app)
            .into_iter()
            .map(|(rat, _)| rat)
            .collect();
        assert_eq!(
            bites, attacks,
            "同一次咬击必须同帧同时产出 RatBiteEvent（伤害/守恒）与 RatAttackAvEvent（表现）；\
             只剩一边 = 要么无伤害的空动画，要么无动画的隐形咬击"
        );
    }

    // --- plan-ambient-threat-v1 P2: mutual exclusion with QiSourceProximityScorer ---

    #[derive(Default, Resource)]
    struct RecordedBites(Vec<RatBiteEvent>);

    fn record_bites(mut events: EventReader<RatBiteEvent>, mut recorded: ResMut<RecordedBites>) {
        recorded.0.extend(events.read().copied());
    }

    #[test]
    fn harass_and_seek_qi_source_never_double_bite_the_same_player_via_thinker_ordering() {
        use big_brain::prelude::BigBrainPlugin;

        let mut app = App::new();
        app.add_plugins(BigBrainPlugin::new(PreUpdate));
        app.add_event::<RatBiteEvent>();
        app.add_event::<RatAttackAvEvent>();
        app.insert_resource(CombatClock { tick: 0 });
        app.insert_resource(RecordedBites::default());
        register(&mut app);
        app.add_systems(Update, record_bites);

        let rat = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                rat_blackboard_with_cooldown(0),
                Navigator::new(),
                crate::npc::spawn_rat::rat_npc_thinker(),
            ))
            .id();
        // 玩家同时满足两条分支的起评条件：≤8 格（harass）且 meditating（qi 源权重加成）——
        // 这正是 plan 里"双 Scorer 冲突"的场景。若互斥编排失效，同一 tick 或紧邻几 tick
        // 内会各自 emit 一次 RatBiteEvent，产生双倍咬击。
        let player = app
            .world_mut()
            .spawn((
                ClientMarker,
                Position::new([0.2, 64.0, 0.0]),
                cultivation(5.0),
                MeditatingState { since_tick: 0 },
            ))
            .id();

        for _ in 0..10 {
            app.update();
        }

        let recorded = &app.world().resource::<RecordedBites>().0;
        assert_eq!(
            recorded.len(),
            1,
            "harass ordering + post-bite cooldown/drained-chunk must prevent a double bite \
             within a single close-range encounter window; got {} RatBiteEvents: {:?}",
            recorded.len(),
            recorded
        );
        assert_eq!(recorded[0].rat, rat);
        assert_eq!(recorded[0].target, player);
    }
}
