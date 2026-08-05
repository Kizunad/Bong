//! plan-mundane-fauna-v1 P2 — 狼/狐 Hunt 分支（凡兽内食物链）。
//!
//! **不复用 `territory.rs` 的 `TerritoryIntruderScorer`/`HuntAction`**：那套系统硬性要求
//! actor 带 `Territory` 组件、且 candidate 查询要 `&Cultivation`（凡兽自 P0 起已满足后者，
//! 但前者意味着要给狼/狐一份"领地"，会牵连 `beast_reproduction_tick_system` 等一整套
//! 领地语义；且其 `consider_hunt_candidate` 对 Beast-archetype 目标的过滤依赖调用方自带
//! `FaunaTag`，凡兽没有，无法正确表达"猎鼠但不猎妖兽"这条边）。改走一套独立的
//! `MundaneHuntScorer`/`MundaneHuntAction`，猎物筛选统一走 `fauna::mundane::preys_on`
//! 关系表（§8.1 #2：新建跨层表，不扩展 `is_prey_of`）。
//!
//! 妖兽猎凡兽（"妖兽=凡兽的口粮"一侧）**不需要本模块任何代码**——见
//! `fauna::mundane::preys_on` 文档：既有 `territory.rs` Hunt 管线对非 Beast-archetype
//! 目标本就无差别放行，凡兽自 P0 起自带 `Cultivation::default()`（满足
//! `HuntCandidateQuery` 契约），這条边是既有生产代码的自然结果。

use big_brain::prelude::{ActionBuilder, ActionState, Actor, Score, ScorerBuilder};
use valence::prelude::{
    bevy_ecs, Commands, Component, DVec3, Despawned, Entity, EventReader, EventWriter, Position,
    Query, Res, With, Without,
};
#[cfg(test)]
use valence::prelude::{ResMut, Resource};

use crate::combat::events::{AttackIntent, AttackSource, DeathEvent};
use crate::fauna::components::{BeastKind, FaunaTag};
use crate::fauna::mundane::{
    preys_on, FaunaPredatorId, FaunaPreyId, MundaneFaunaKind, MundaneFaunaSpecies,
};
use crate::npc::hunger::Hunger;
use crate::npc::movement::GameTick;
use crate::npc::navigator::Navigator;
use crate::npc::spawn::{NpcMarker, NpcMeleeProfile};

/// Hunt 目标搜索半径——凡兽尺度小于妖兽领地搜猎半径（`HUNT_SEARCH_RADIUS=48`）。
pub const MUNDANE_HUNT_SEARCH_RADIUS: f64 = 24.0;
/// Hunt 追击速度因子（同 `territory::HUNT_SPEED_FACTOR` 数值，狩猎时加速追赶）。
pub const MUNDANE_HUNT_SPEED_FACTOR: f64 = 1.1;
/// Hunt 追击超时（tick）：超时未逮到目标就放弃，回 Success 让 picker 重新评分。
pub const MUNDANE_HUNT_MAX_TICKS: u32 = 200;
/// 攻击冷却（tick），避免同 tick 刷帧攻击。
pub const MUNDANE_HUNT_ATTACK_COOLDOWN_TICKS: u32 = 30;
/// 饥饿加权评分下限——即便饱腹（`hunger_pressure()==0`）也保留一个基线分，"看到猎物基线
/// 也会咬"（T2.5"群体饥饿驱动"里"驱动"只是加权，不是唯一开关）。
pub const MUNDANE_HUNT_SCORE_FLOOR: f32 = 0.3;

// ---------------------------------------------------------------------------
// Scorer
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Component)]
pub struct MundaneHuntScorer;

impl ScorerBuilder for MundaneHuntScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("MundaneHuntScorer")
    }
}

type MundaneHuntActorScorerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static MundaneFaunaSpecies,
        &'static Hunger,
    ),
    With<NpcMarker>,
>;

/// 猎物候选：凡兽全档（含掠食者自身，筛选靠 `preys_on`，不靠 query filter 排除）。
pub type MundanePreyQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Position, &'static MundaneFaunaSpecies), Without<Despawned>>;

/// 噬元鼠候选：`FaunaTag` 全体妖兽实体，运行时按 `beast_kind == Rat` 过滤
/// （`FaunaTag` 本身不区分种类，全库通用挂载组件）。
pub type RatPreyQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Position, &'static FaunaTag), Without<Despawned>>;

/// 挑猎物：遍历凡兽 + 鼠候选，按 `preys_on` 过滤 + 半径裁剪，取最近者。
pub fn pick_mundane_hunt_target(
    pos: DVec3,
    predator_kind: MundaneFaunaKind,
    self_entity: Entity,
    prey: &MundanePreyQuery<'_, '_>,
    rats: &RatPreyQuery<'_, '_>,
) -> Option<(Entity, DVec3)> {
    let mut best: Option<(Entity, DVec3, f64)> = None;
    for (entity, tpos, species) in prey.iter() {
        if entity == self_entity {
            continue;
        }
        if !preys_on(
            FaunaPredatorId::Mundane(predator_kind),
            FaunaPreyId::Mundane(species.0),
        ) {
            continue;
        }
        consider(pos, &mut best, entity, tpos.get());
    }
    for (entity, tpos, tag) in rats.iter() {
        if tag.beast_kind != BeastKind::Rat {
            continue;
        }
        if !preys_on(FaunaPredatorId::Mundane(predator_kind), FaunaPreyId::Rat) {
            continue;
        }
        consider(pos, &mut best, entity, tpos.get());
    }
    best.map(|(entity, tpos, _)| (entity, tpos))
}

fn consider(
    pos: DVec3,
    best: &mut Option<(Entity, DVec3, f64)>,
    entity: Entity,
    target_pos: DVec3,
) {
    let d = pos.distance(target_pos);
    if d > MUNDANE_HUNT_SEARCH_RADIUS {
        return;
    }
    if best.as_ref().is_none_or(|(_, _, best_d)| d < *best_d) {
        *best = Some((entity, target_pos, d));
    }
}

pub(crate) fn mundane_hunt_scorer_system(
    actors: MundaneHuntActorScorerQuery<'_, '_>,
    prey: MundanePreyQuery<'_, '_>,
    rats: RatPreyQuery<'_, '_>,
    mut scorers: Query<(&Actor, &mut Score), With<MundaneHuntScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((pos, species, hunger)) = actors.get(*actor) {
            if pick_mundane_hunt_target(pos.get(), species.0, *actor, &prey, &rats).is_some() {
                (MUNDANE_HUNT_SCORE_FLOOR
                    + (1.0 - MUNDANE_HUNT_SCORE_FLOOR) * hunger.hunger_pressure())
                .clamp(MUNDANE_HUNT_SCORE_FLOOR, 1.0)
            } else {
                0.0
            }
        } else {
            0.0
        };
        score.set(value);
    }
}

// ---------------------------------------------------------------------------
// Action
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Component)]
pub struct MundaneHuntAction;

impl ActionBuilder for MundaneHuntAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("MundaneHuntAction")
    }
}

/// Hunt 运行态：当前目标 + elapsed + 上次攻击 tick。挂在 actor（预掠者）上。
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct MundaneHuntState {
    pub target: Option<Entity>,
    pub elapsed_ticks: u32,
    pub last_attack_tick: Option<u32>,
}

type MundaneHuntActorQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static MundaneFaunaSpecies,
        &'static NpcMeleeProfile,
        &'static mut Navigator,
        &'static mut MundaneHuntState,
    ),
    With<NpcMarker>,
>;

/// 在 prey/rats 两张查询里定位目标当前坐标（用于判断"目标是否还在场"）。目标消失
/// （被杀死已 despawn，或超距/其它原因被移除）统一视为"本次狩猎结束"——回 Success 让
/// picker 重新评分，不区分"杀死"与"消失"（同 `territory::hunt_action_system` 既有对
/// `candidates.get(target)` 失败的处理方式一致）。真正的猎杀→回补 Hunger 闭环走独立的
/// `DeathEvent` 监听（`mundane_predator_kill_hunger_reward_system`，`fauna/mundane.rs`
/// 注册），不依赖本函数的"消失"信号做二次判定，从而避免"超距回收 vs 真实猎杀"的歧义。
fn locate_target(
    target: Entity,
    prey: &MundanePreyQuery<'_, '_>,
    rats: &RatPreyQuery<'_, '_>,
) -> Option<DVec3> {
    if let Ok((_, pos, _)) = prey.get(target) {
        return Some(pos.get());
    }
    if let Ok((_, pos, _)) = rats.get(target) {
        return Some(pos.get());
    }
    None
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn mundane_hunt_action_system(
    mut actors: MundaneHuntActorQuery<'_, '_>,
    prey: MundanePreyQuery<'_, '_>,
    rats: RatPreyQuery<'_, '_>,
    mut actions: Query<(&Actor, &mut ActionState), With<MundaneHuntAction>>,
    mut attack_intents: EventWriter<AttackIntent>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut state) in &mut actions {
        let Ok((pos, species, profile, mut navigator, mut hunt)) = actors.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };
        match *state {
            ActionState::Requested => {
                match pick_mundane_hunt_target(pos.get(), species.0, *actor, &prey, &rats) {
                    Some((entity, tpos)) => {
                        navigator.set_goal(tpos, MUNDANE_HUNT_SPEED_FACTOR);
                        hunt.target = Some(entity);
                        hunt.elapsed_ticks = 0;
                        hunt.last_attack_tick = None;
                        *state = ActionState::Executing;
                    }
                    None => {
                        navigator.stop();
                        *state = ActionState::Success;
                    }
                }
            }
            ActionState::Executing => {
                hunt.elapsed_ticks = hunt.elapsed_ticks.saturating_add(1);
                if hunt.elapsed_ticks >= MUNDANE_HUNT_MAX_TICKS {
                    navigator.stop();
                    hunt.target = None;
                    *state = ActionState::Success;
                    continue;
                }
                let Some(target) = hunt.target else {
                    *state = ActionState::Success;
                    continue;
                };
                let Some(tpos) = locate_target(target, &prey, &rats) else {
                    hunt.target = None;
                    *state = ActionState::Success;
                    continue;
                };
                navigator.set_goal(tpos, MUNDANE_HUNT_SPEED_FACTOR);
                let distance = pos.get().distance(tpos) as f32;
                let cooled = hunt
                    .last_attack_tick
                    .map(|last| tick.saturating_sub(last) >= MUNDANE_HUNT_ATTACK_COOLDOWN_TICKS)
                    .unwrap_or(true);
                if distance <= profile.reach.max && cooled {
                    attack_intents.send(AttackIntent {
                        attacker: *actor,
                        target: Some(target),
                        issued_at_tick: u64::from(tick),
                        reach: profile.reach,
                        qi_invest: 0.0,
                        wound_kind: profile.wound_kind,
                        source: AttackSource::Melee,
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

// ---------------------------------------------------------------------------
// 捕食闭环 — 猎杀成功回补 Hunger（"复用 FarmAction 回补路径换触发源"）
// ---------------------------------------------------------------------------

/// 猎杀成功回补 Hunger 的量——一顿饱餐，远高于 `HungerConfig::farm_restore_per_tick`
/// 逐 tick 回补速率（一次性事件 vs 持续进食，量级刻意不同）。
pub const MUNDANE_HUNT_KILL_HUNGER_REWARD: f64 = 0.6;

/// 监听 `DeathEvent`（唯一权威死亡信号，`emit_death_event_if_lethal` 校验过
/// `was_alive && health_current<=0` 才发出，不会误判"超距回收"之类的非战斗消失）：
/// 若攻击者是凡兽狼/狐，回补其 Hunger。**不关心受害者是谁**（凡兽同类 or 噬元鼠皆可）——
/// 猎杀就是猎杀，回补统一。同一只狼同 tick 内不会重复计入（`DeathEvent` 每次死亡只发一次）。
pub(crate) fn mundane_predator_kill_hunger_reward_system(
    mut deaths: EventReader<DeathEvent>,
    mut predators: Query<(&MundaneFaunaSpecies, &mut Hunger)>,
) {
    for death in deaths.read() {
        let Some(attacker) = death.attacker else {
            continue;
        };
        let Ok((species, mut hunger)) = predators.get_mut(attacker) else {
            continue;
        };
        if !species.0.is_predator() {
            continue;
        }
        hunger.replenish(MUNDANE_HUNT_KILL_HUNGER_REWARD);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fauna::components::FaunaTag;
    use crate::fauna::mundane::MundaneFaunaKind;

    // -----------------------------------------------------------------
    // preys_on — 跨层关系表全边 pin
    // -----------------------------------------------------------------

    #[test]
    fn wolf_preys_on_t0_and_t1_mundane_but_not_fox_or_wolf() {
        for prey in [
            MundaneFaunaKind::Cow,
            MundaneFaunaKind::Pig,
            MundaneFaunaKind::Sheep,
            MundaneFaunaKind::Chicken,
            MundaneFaunaKind::Rabbit,
            MundaneFaunaKind::Goat,
            MundaneFaunaKind::Frog,
        ] {
            assert!(
                preys_on(
                    FaunaPredatorId::Mundane(MundaneFaunaKind::Wolf),
                    FaunaPreyId::Mundane(prey)
                ),
                "狼应猎 T0∪T1 凡兽，{prey:?} 未命中"
            );
        }
        for non_prey in [MundaneFaunaKind::Fox, MundaneFaunaKind::Wolf] {
            assert!(
                !preys_on(
                    FaunaPredatorId::Mundane(MundaneFaunaKind::Wolf),
                    FaunaPreyId::Mundane(non_prey)
                ),
                "狼不应猎 {non_prey:?}（同为掠食者/自身）"
            );
        }
    }

    #[test]
    fn wolf_preys_on_rat() {
        assert!(preys_on(
            FaunaPredatorId::Mundane(MundaneFaunaKind::Wolf),
            FaunaPreyId::Rat
        ));
    }

    #[test]
    fn fox_preys_on_t0_only_not_t1() {
        for prey in [
            MundaneFaunaKind::Chicken,
            MundaneFaunaKind::Rabbit,
            MundaneFaunaKind::Frog,
        ] {
            assert!(
                preys_on(
                    FaunaPredatorId::Mundane(MundaneFaunaKind::Fox),
                    FaunaPreyId::Mundane(prey)
                ),
                "狐应猎 T0，{prey:?} 未命中"
            );
        }
        for non_prey in [
            MundaneFaunaKind::Cow,
            MundaneFaunaKind::Pig,
            MundaneFaunaKind::Sheep,
            MundaneFaunaKind::Goat,
            MundaneFaunaKind::Fox,
            MundaneFaunaKind::Wolf,
        ] {
            assert!(
                !preys_on(
                    FaunaPredatorId::Mundane(MundaneFaunaKind::Fox),
                    FaunaPreyId::Mundane(non_prey)
                ),
                "狐不应猎 {non_prey:?}（T1 或掠食者）"
            );
        }
    }

    #[test]
    fn fox_preys_on_rat() {
        assert!(preys_on(
            FaunaPredatorId::Mundane(MundaneFaunaKind::Fox),
            FaunaPreyId::Rat
        ));
    }

    #[test]
    fn t0_species_never_predator() {
        // T0（鸡/兔/蛙）从不作为 predator——避一切，不猎任何东西。
        for t0 in [
            MundaneFaunaKind::Chicken,
            MundaneFaunaKind::Rabbit,
            MundaneFaunaKind::Frog,
        ] {
            for prey_kind in MundaneFaunaKind::ALL {
                assert!(
                    !preys_on(
                        FaunaPredatorId::Mundane(t0),
                        FaunaPreyId::Mundane(prey_kind)
                    ),
                    "{t0:?} 不应捕食 {prey_kind:?}（T0 永不作为 predator）"
                );
            }
            assert!(
                !preys_on(FaunaPredatorId::Mundane(t0), FaunaPreyId::Rat),
                "{t0:?} 不应捕食鼠（T0 永不作为 predator）"
            );
        }
    }

    #[test]
    fn t1_non_predator_species_never_predator() {
        // T1（牛/猪/羊/山羊）本身也不是 predator（只有狼/狐是）。
        for t1 in [
            MundaneFaunaKind::Cow,
            MundaneFaunaKind::Pig,
            MundaneFaunaKind::Sheep,
            MundaneFaunaKind::Goat,
        ] {
            for prey_kind in MundaneFaunaKind::ALL {
                assert!(
                    !preys_on(
                        FaunaPredatorId::Mundane(t1),
                        FaunaPreyId::Mundane(prey_kind)
                    ),
                    "{t1:?} 不应捕食任何凡兽"
                );
            }
        }
    }

    #[test]
    fn beast_preys_on_mundane_full_tier_but_not_rat_beast() {
        // "妖兽猎凡兽=全档"（plan 设计原则表明文），且陆生非鼠妖兽才算数。
        for beast in BeastKind::ALL_TERRESTRIAL {
            if *beast == BeastKind::Rat {
                continue;
            }
            for prey_kind in MundaneFaunaKind::ALL {
                assert!(
                    preys_on(
                        FaunaPredatorId::Beast(*beast),
                        FaunaPreyId::Mundane(prey_kind)
                    ),
                    "陆生妖兽 {beast:?} 应能猎凡兽全档，{prey_kind:?} 未命中"
                );
            }
        }
        // 鼠本身不是"妖兽猎凡兽"意义上的 predator（鼠患天敌来自凡兽层，不反过来）。
        for prey_kind in MundaneFaunaKind::ALL {
            assert!(
                !preys_on(
                    FaunaPredatorId::Beast(BeastKind::Rat),
                    FaunaPreyId::Mundane(prey_kind)
                ),
                "鼠不应捕食凡兽 {prey_kind:?}"
            );
        }
    }

    #[test]
    fn non_terrestrial_beast_does_not_prey_on_mundane() {
        // 化虚级非陆生（如 Whale）不应被误判为"陆生妖兽猎凡兽"边命中。
        assert!(
            !BeastKind::Whale.is_terrestrial(),
            "Whale 应为非陆生（本用例前置假设）"
        );
        for prey_kind in MundaneFaunaKind::ALL {
            assert!(
                !preys_on(
                    FaunaPredatorId::Beast(BeastKind::Whale),
                    FaunaPreyId::Mundane(prey_kind)
                ),
                "非陆生妖兽 Whale 不应捕食凡兽 {prey_kind:?}"
            );
        }
    }

    #[test]
    fn reverse_edge_closed_rat_never_preys_on_mundane_or_scavenges() {
        // §8.1 #8 定案：鼠不腐食/不捕食凡兽尸体或活体——鼠患的"吃"（噬元）与凡兽的
        // "死"（无灵）不构成边。与"狼狐猎鼠"是不同的边、不冲突（本用例只锁"反方向"）。
        for prey_kind in MundaneFaunaKind::ALL {
            assert!(
                !preys_on(
                    FaunaPredatorId::Beast(BeastKind::Rat),
                    FaunaPreyId::Mundane(prey_kind)
                ),
                "反向断言：preys_on 表内不应存在 鼠→{prey_kind:?} 的边"
            );
        }
    }

    #[test]
    fn wolf_and_fox_hunt_edges_coexist_with_rat_predation() {
        // 回归测试同时验证两者共存：狼/狐猎鼠（合法边）与鼠不猎/不腐食凡兽（关闭边）
        // 互不冲突——狼/狐是"捕食者"角色，鼠是"猎物"角色，二者在关系表里是不同方向。
        assert!(preys_on(
            FaunaPredatorId::Mundane(MundaneFaunaKind::Wolf),
            FaunaPreyId::Rat
        ));
        assert!(preys_on(
            FaunaPredatorId::Mundane(MundaneFaunaKind::Fox),
            FaunaPreyId::Rat
        ));
        assert!(!preys_on(
            FaunaPredatorId::Beast(BeastKind::Rat),
            FaunaPreyId::Mundane(MundaneFaunaKind::Chicken)
        ));
    }

    // -----------------------------------------------------------------
    // pick_mundane_hunt_target — 纯函数筛选逻辑（走真实 Bevy Query，非手搓断言）
    // -----------------------------------------------------------------

    /// 建一个 App，spawn 若干 prey/rat 候选后跑一次 `pick_mundane_hunt_target`，
    /// 用一次性系统把结果带出来（`Query` 类型无法脱离 `World` 存活，只能在系统内跑）。
    fn run_pick(
        wolf_pos: DVec3,
        predator_kind: MundaneFaunaKind,
        self_entity: Entity,
        setup: impl FnOnce(&mut valence::prelude::World),
    ) -> Option<(Entity, DVec3)> {
        let mut app = valence::prelude::App::new();
        setup(app.world_mut());

        #[derive(Resource, Default)]
        struct PickResult(Option<(Entity, DVec3)>);
        app.insert_resource(PickResult::default());

        let system = move |prey: MundanePreyQuery<'_, '_>,
                           rats: RatPreyQuery<'_, '_>,
                           mut out: ResMut<PickResult>| {
            out.0 = pick_mundane_hunt_target(wolf_pos, predator_kind, self_entity, &prey, &rats);
        };
        app.add_systems(valence::prelude::Update, system);
        app.update();
        app.world().resource::<PickResult>().0
    }

    #[test]
    fn pick_target_excludes_self() {
        let mut app = valence::prelude::App::new();
        let wolf = app
            .world_mut()
            .spawn((
                Position::new([0.0, 64.0, 0.0]),
                MundaneFaunaSpecies(MundaneFaunaKind::Wolf),
            ))
            .id();
        let picked = run_pick(
            DVec3::new(0.0, 64.0, 0.0),
            MundaneFaunaKind::Wolf,
            wolf,
            |_world| {},
        );
        // wolf 自身也满足 `MundaneFaunaKind::Wolf` prey 条件为 false（preys_on 已排除
        // Wolf→Wolf），但即便 preys_on 意外放行，self_entity 排除逻辑也必须兜底。
        assert!(
            picked.is_none(),
            "无其它候选时不应选中任何目标（尤其不能选中自身）"
        );
    }

    #[test]
    fn pick_target_excludes_out_of_range_prey() {
        let wolf = Entity::from_raw(1);
        let picked = run_pick(
            DVec3::new(0.0, 64.0, 0.0),
            MundaneFaunaKind::Wolf,
            wolf,
            |world| {
                world.spawn((
                    Position::new([999.0, 64.0, 999.0]),
                    MundaneFaunaSpecies(MundaneFaunaKind::Rabbit),
                ));
            },
        );
        assert!(
            picked.is_none(),
            "超出 MUNDANE_HUNT_SEARCH_RADIUS 的猎物不应被选中"
        );
    }

    #[test]
    fn pick_target_selects_nearest_eligible_prey() {
        let wolf = Entity::from_raw(1);
        let mut near_entity = None;
        let picked = run_pick(
            DVec3::new(0.0, 64.0, 0.0),
            MundaneFaunaKind::Wolf,
            wolf,
            |world| {
                near_entity = Some(
                    world
                        .spawn((
                            Position::new([5.0, 64.0, 0.0]),
                            MundaneFaunaSpecies(MundaneFaunaKind::Rabbit),
                        ))
                        .id(),
                );
                world.spawn((
                    Position::new([15.0, 64.0, 0.0]),
                    MundaneFaunaSpecies(MundaneFaunaKind::Sheep),
                ));
            },
        );
        let (picked_entity, picked_pos) = picked.expect("范围内应有合法猎物");
        assert_eq!(
            picked_entity,
            near_entity.expect("测试前置：near_entity 必须已 spawn"),
            "应选中距离更近的兔子而非更远的羊"
        );
        assert!((picked_pos - DVec3::new(5.0, 64.0, 0.0)).length() < 1e-9);
    }

    #[test]
    fn pick_target_skips_ineligible_species_for_fox() {
        // 狐不猎 T1（牛/猪/羊/山羊），即便距离更近也不应命中。
        let fox = Entity::from_raw(1);
        let picked = run_pick(
            DVec3::new(0.0, 64.0, 0.0),
            MundaneFaunaKind::Fox,
            fox,
            |world| {
                world.spawn((
                    Position::new([2.0, 64.0, 0.0]),
                    MundaneFaunaSpecies(MundaneFaunaKind::Sheep),
                ));
            },
        );
        assert!(picked.is_none(), "狐不应把羊（T1）当猎物");
    }

    #[test]
    fn pick_target_includes_rat_candidates() {
        let wolf = Entity::from_raw(1);
        let mut rat_entity = None;
        let picked = run_pick(
            DVec3::new(0.0, 64.0, 0.0),
            MundaneFaunaKind::Wolf,
            wolf,
            |world| {
                rat_entity = Some(
                    world
                        .spawn((
                            Position::new([3.0, 64.0, 0.0]),
                            FaunaTag::new(BeastKind::Rat),
                        ))
                        .id(),
                );
            },
        );
        assert_eq!(
            picked.map(|(e, _)| e),
            rat_entity,
            "狼应能把噬元鼠列为合法猎物"
        );
    }

    #[test]
    fn pick_target_ignores_non_rat_beast_candidates() {
        // FaunaTag 挂载的非鼠妖兽不应被 pick_mundane_hunt_target 当成猎物
        // （狼/狐不猎妖兽，只猎凡兽 + 鼠）。
        let wolf = Entity::from_raw(1);
        let picked = run_pick(
            DVec3::new(0.0, 64.0, 0.0),
            MundaneFaunaKind::Wolf,
            wolf,
            |world| {
                world.spawn((
                    Position::new([2.0, 64.0, 0.0]),
                    FaunaTag::new(BeastKind::Spider),
                ));
            },
        );
        assert!(picked.is_none(), "狼不应把蜘蛛（非鼠妖兽）当猎物");
    }

    // -----------------------------------------------------------------
    // mundane_predator_kill_hunger_reward_system — 捕食闭环
    // -----------------------------------------------------------------

    fn hunger_reward_test_app() -> valence::prelude::App {
        let mut app = valence::prelude::App::new();
        app.add_event::<DeathEvent>();
        app.add_systems(
            valence::prelude::Update,
            mundane_predator_kill_hunger_reward_system,
        );
        app
    }

    #[test]
    fn kill_hunger_reward_replenishes_predator_hunger_on_death_event() {
        let mut app = hunger_reward_test_app();
        let wolf = app
            .world_mut()
            .spawn((
                MundaneFaunaSpecies(MundaneFaunaKind::Wolf),
                Hunger::new(0.2),
            ))
            .id();
        let victim = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(DeathEvent {
            target: victim,
            cause: "hunted".to_string(),
            attacker: Some(wolf),
            attacker_player_id: None,
            at_tick: 0,
        });
        app.update();
        let hunger = app
            .world()
            .get::<Hunger>(wolf)
            .expect("wolf should still have Hunger");
        assert!(
            (hunger.value - (0.2 + MUNDANE_HUNT_KILL_HUNGER_REWARD)).abs() < 1e-9,
            "猎杀成功应回补 Hunger {MUNDANE_HUNT_KILL_HUNGER_REWARD}，期望 {}，实际 {}",
            0.2 + MUNDANE_HUNT_KILL_HUNGER_REWARD,
            hunger.value
        );
    }

    #[test]
    fn kill_hunger_reward_ignores_non_predator_attacker() {
        // 攻击者是非 predator 凡兽（如受击反击的兔子）不应触发回补——`is_predator()` 门控。
        let mut app = hunger_reward_test_app();
        let rabbit = app
            .world_mut()
            .spawn((
                MundaneFaunaSpecies(MundaneFaunaKind::Rabbit),
                Hunger::new(0.2),
            ))
            .id();
        let victim = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(DeathEvent {
            target: victim,
            cause: "test".to_string(),
            attacker: Some(rabbit),
            attacker_player_id: None,
            at_tick: 0,
        });
        app.update();
        let hunger = app.world().get::<Hunger>(rabbit).unwrap();
        assert!(
            (hunger.value - 0.2).abs() < 1e-9,
            "非 predator（兔子）不应因 DeathEvent 回补 Hunger"
        );
    }

    #[test]
    fn kill_hunger_reward_ignores_death_event_with_no_attacker() {
        // attacker=None（环境死亡/自然死亡等）不应 panic，也不应误触发任何回补。
        let mut app = hunger_reward_test_app();
        let victim = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(DeathEvent {
            target: victim,
            cause: "environment".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 0,
        });
        // 不应 panic。
        app.update();
    }

    #[test]
    fn kill_hunger_reward_clamps_at_full_hunger() {
        // Hunger::replenish 内部走 `set` 的 clamp(0,1)——即便回补量让总和 > 1.0 也不 panic
        // 或溢出，钳在满值。
        let mut app = hunger_reward_test_app();
        let wolf = app
            .world_mut()
            .spawn((
                MundaneFaunaSpecies(MundaneFaunaKind::Wolf),
                Hunger::new(0.9),
            ))
            .id();
        let victim = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(DeathEvent {
            target: victim,
            cause: "hunted".to_string(),
            attacker: Some(wolf),
            attacker_player_id: None,
            at_tick: 0,
        });
        app.update();
        let hunger = app.world().get::<Hunger>(wolf).unwrap();
        assert!(
            (hunger.value - 1.0).abs() < 1e-9,
            "回补超过满值应钳在 1.0，实际 {}",
            hunger.value
        );
    }
}
