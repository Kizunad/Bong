//! 拟态灰烬蛛 AI — plan-fauna-mimic-spider-v1 P1
//!
//! big-brain Scorer/Action 三态驱动：
//!   - [`SpiderAmbushScorer`]：Disguised 蛛检测到真元超阈值玩家 → score=1.0，触发暴起。
//!   - [`SpiderAmbushAction`]：Disguised→Ambush 状态转换 + emit SpiderAmbushTriggerEvent +
//!     emit VFX（`bong:vfx/spider_ambush`，count=16，#B8D0C8，径向 burst）+
//!     emit 音效（`entity_spider_step` recipe，pitch_shift+1.8 → 实际 pitch≈1.8，vol=0.6）+
//!     通过 NPC blackboard.target_position 驱动导航追击（复用 ChaseAction 逻辑）。
//!   - [`SpiderRetreatAction`]：Ambush 期受威胁时转入 Retreat，向低 spirit_qi 方向逃离，
//!     超过 `SPIDER_RETREAT_RADIUS` 后回 Disguised。
//!
//! P1 独立于 P2 神识识破 / P3 陷阱，所有新逻辑在本模块。

use big_brain::prelude::{ActionBuilder, ActionState, Actor, BigBrainSet, Score, ScorerBuilder};
use valence::client::ClientMarker;
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, Event, EventWriter, IntoSystemConfigs,
    Position, PreUpdate, Query, With, Without,
};

use crate::cultivation::components::Cultivation;
use crate::fauna::experience::{play_audio, spawn_particle};
use crate::fauna::mimic_spider::{
    retreat_complete, within_sense_radius, MimicSpiderBlackboard, SpiderDisguiseState,
    SPIDER_QI_SENSE_THRESHOLD, SPIDER_RETREAT_RADIUS,
};
use crate::network::audio_event_emit::PlaySoundRecipeRequest;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::navigator::Navigator;
use crate::npc::spawn::NpcMarker;

// ── 暴起 VFX 常数 ─────────────────────────────────────────────────────────────

/// `bong:vfx/spider_ambush`：灰烬蛛暴起粒子 ID（client VfxBootstrap 须注册对应 VfxPlayer）。
pub const SPIDER_AMBUSH_VFX_EVENT_ID: &str = "bong:vfx/spider_ambush";

/// 暴起粒子颜色（灰烬蛛体色 #B8D0C8）。
pub const SPIDER_AMBUSH_PARTICLE_COLOR: &str = "#B8D0C8";

/// 暴起粒子数量（径向 burst）。
pub const SPIDER_AMBUSH_PARTICLE_COUNT: u16 = 16;

/// 粒子持续时间（tick）：8 tick ≈ 400ms。
pub const SPIDER_AMBUSH_PARTICLE_DURATION_TICKS: u16 = 8;

/// 暴起粒子 strength（径向扩散速度参考，客户端据此换算 2m/s）。
pub const SPIDER_AMBUSH_PARTICLE_STRENGTH: f32 = 0.8;

// ── 暴起音效常数 ─────────────────────────────────────────────────────────────

/// 音效 recipe ID：蛛步行声（server 侧 recipe registry）。
pub const SPIDER_STEP_RECIPE_ID: &str = "entity_spider_step";

/// pitch 偏移：+1.8 会让客户端把 base pitch 拉到 ≈1.8（recipe 内 base_pitch=1.0）。
/// 文档约定：`pitch_shift` 是在 recipe base_pitch 基础上的加值。
pub const SPIDER_AMBUSH_PITCH_SHIFT: f32 = 0.8;

/// 暴起音效音量倍率。
pub const SPIDER_AMBUSH_VOLUME_MUL: f32 = 0.6;

// ── 追击速度 ─────────────────────────────────────────────────────────────────

/// Ambush 阶段追击速度因子（略快于标准 1.0 以体现掠食者优势）。
const SPIDER_CHASE_SPEED: f64 = 1.1;

/// Retreat 阶段逃跑速度因子（蛛轻盈，逃得比追得快）。
const SPIDER_RETREAT_SPEED: f64 = 1.15;

/// 暴起（进入 Ambush）时 VFX 触发半径偏移高度（蛛中心点 Y 偏移）。
const SPIDER_VFX_ORIGIN_Y_OFFSET: f64 = 0.5;

// ── 事件 ─────────────────────────────────────────────────────────────────────

/// Bevy 事件：灰烬蛛暴起触发（P2 client 伪装渲染 + 神识识破层监听此事件）。
///
/// `spider` 字段为蛛的 Entity（通过 `.index()` 跨系统传递）；
/// `trigger_pos` 为暴起世界坐标（供 P2 CustomPayload 广播）。
#[derive(Debug, Clone, Event)]
pub struct SpiderAmbushTriggerEvent {
    /// P2 CustomPayload 广播使用：蛛 Entity raw index。
    #[allow(dead_code)]
    pub spider: u32,
    /// P2 client 端渲染切换时的世界坐标原点。
    #[allow(dead_code)]
    pub trigger_pos: DVec3,
}

// ── Scorer ────────────────────────────────────────────────────────────────────

/// 感知真元评分器：Disguised 状态下扫描感知半径内的玩家真元，超过阈值则 score=1.0。
///
/// 条件同时满足时才触发（AND 关系）：
///   1. 蛛处于 `SpiderDisguiseState::Disguised`
///   2. 最近玩家在 `SPIDER_SENSE_RADIUS` 内
///   3. 玩家 `qi_current > qi_max * SPIDER_QI_SENSE_THRESHOLD`
#[derive(Clone, Copy, Debug, Component)]
pub struct SpiderAmbushScorer;

impl ScorerBuilder for SpiderAmbushScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("SpiderAmbushScorer")
    }
}

type SpiderScorerActorQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static SpiderDisguiseState,
        &'static MimicSpiderBlackboard,
    ),
    (With<NpcMarker>, Without<ClientMarker>),
>;

type PlayerCultivationQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Position, &'static Cultivation),
    (With<ClientMarker>, Without<NpcMarker>),
>;

pub(crate) fn spider_ambush_scorer_system(
    spiders: SpiderScorerActorQuery<'_, '_>,
    players: PlayerCultivationQuery<'_, '_>,
    mut scorers: Query<(&Actor, &mut Score), With<SpiderAmbushScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let Ok((spider_pos, state, blackboard)) = spiders.get(*actor) else {
            score.set(0.0);
            continue;
        };

        // 只在 Disguised 状态触发感知
        if *state != SpiderDisguiseState::Disguised {
            score.set(0.0);
            continue;
        }

        // P3：陷阱蛛不攻击 trap_owner（只攻击第三方玩家）
        let trap_owner = blackboard.trapped_by;

        // 扫描感知半径内的玩家（排除陷阱归属玩家）
        let detected = players
            .iter()
            .filter(|(player_entity, _, _)| {
                // 野生蛛（trap_owner=None）攻击所有玩家；
                // 陷阱蛛（trap_owner=Some(owner)）跳过 owner，攻击其他玩家
                trap_owner
                    .map(|owner| *player_entity != owner)
                    .unwrap_or(true)
            })
            .any(|(_, player_pos, cultivation)| {
                within_sense_radius(spider_pos.get(), player_pos.get())
                    && cultivation.qi_current > cultivation.qi_max * SPIDER_QI_SENSE_THRESHOLD
            });

        score.set(if detected { 1.0 } else { 0.0 });
    }
}

// ── 暴起 Action ───────────────────────────────────────────────────────────────

/// 暴起 Action：Disguised → Ambush 状态切换 + VFX + 音效 + 追击导航。
///
/// 执行期间持续追击最近真元超阈值玩家。若蛛受重创（hp < 20% 或玩家脱离感知范围）
/// 则自然停止（Success），由 big-brain picker 转入 RetreatAction。
#[derive(Clone, Copy, Debug, Component)]
pub struct SpiderAmbushAction;

impl ActionBuilder for SpiderAmbushAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("SpiderAmbushAction")
    }
}

type SpiderAmbushActorQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static mut SpiderDisguiseState,
        &'static mut MimicSpiderBlackboard,
        &'static mut Navigator,
    ),
    (With<NpcMarker>, Without<ClientMarker>),
>;

#[allow(clippy::type_complexity)]
pub(crate) fn spider_ambush_action_system(
    mut spiders: SpiderAmbushActorQuery<'_, '_>,
    players: PlayerCultivationQuery<'_, '_>,
    mut actions: Query<(&Actor, &mut ActionState), With<SpiderAmbushAction>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
    mut ambush_events: EventWriter<SpiderAmbushTriggerEvent>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((position, mut disguise_state, mut blackboard, mut navigator)) =
            spiders.get_mut(*actor)
        else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                // Disguised → Ambush 切换 + emit VFX + 音效 + 事件
                let pos = position.get();
                *disguise_state = SpiderDisguiseState::Ambush;

                // VFX：径向粒子 burst（count=16，#B8D0C8，lifetime=8tick）
                vfx_events.send(spawn_particle(
                    SPIDER_AMBUSH_VFX_EVENT_ID,
                    pos + DVec3::new(0.0, SPIDER_VFX_ORIGIN_Y_OFFSET, 0.0),
                    SPIDER_AMBUSH_PARTICLE_COLOR,
                    SPIDER_AMBUSH_PARTICLE_STRENGTH,
                    SPIDER_AMBUSH_PARTICLE_COUNT,
                    SPIDER_AMBUSH_PARTICLE_DURATION_TICKS,
                ));

                // 音效：蛛暴起声（pitch 偏高，vol=0.6）
                audio_events.send(play_audio(
                    SPIDER_STEP_RECIPE_ID,
                    pos,
                    SPIDER_AMBUSH_VOLUME_MUL,
                    SPIDER_AMBUSH_PITCH_SHIFT,
                ));

                // Bevy 事件：供 P2 CustomPayload 发射层监听
                ambush_events.send(SpiderAmbushTriggerEvent {
                    spider: actor.index(),
                    trigger_pos: pos,
                });

                *state = ActionState::Executing;
            }

            ActionState::Executing => {
                // 确保处于 Ambush 状态（外部可能被强制修改）
                if *disguise_state != SpiderDisguiseState::Ambush {
                    navigator.stop();
                    *state = ActionState::Success;
                    continue;
                }

                let pos = position.get();

                // P3：陷阱蛛追击时排除 trap_owner
                let trap_owner = blackboard.trapped_by;

                // 找最近真元超阈值玩家（排除 trap_owner）
                let nearest = players
                    .iter()
                    .filter(|(player_entity, _, cult)| {
                        let not_owner = trap_owner
                            .map(|owner| *player_entity != owner)
                            .unwrap_or(true);
                        not_owner && cult.qi_current > cult.qi_max * SPIDER_QI_SENSE_THRESHOLD
                    })
                    .min_by(|(_, pa, _), (_, pb, _)| {
                        pos.distance(pa.get())
                            .partial_cmp(&pos.distance(pb.get()))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });

                let Some((_, target_pos, _)) = nearest else {
                    // 无目标 → Ambush 完成（转回 Disguised 由外部 scorer 决定）
                    navigator.stop();
                    *disguise_state = SpiderDisguiseState::Disguised;
                    blackboard.drained_qi = blackboard
                        .drained_qi
                        .min(0.0_f64.max(blackboard.drained_qi - 1.0));
                    *state = ActionState::Success;
                    continue;
                };

                let target = target_pos.get();
                // 已近身（≤2.0 格）：停止移动，视为 Success（由 MeleeAttackAction 接手）
                if pos.distance(target) <= 2.0 {
                    navigator.stop();
                    *state = ActionState::Success;
                    continue;
                }

                navigator.set_goal(target, SPIDER_CHASE_SPEED);
            }

            ActionState::Cancelled => {
                navigator.stop();
                *state = ActionState::Failure;
            }

            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

// ── 撤退 Action ───────────────────────────────────────────────────────────────

/// 撤退 Action：Ambush → Retreat → Disguised 转换。
///
/// 触发时机：`SpiderRetreatScorer` score > 0 且高于 Ambush scorer（由 thinker picker 决定）。
/// 运动方式：向蛛的 `home_pos`（孵化地）移动，到达后回 Disguised。
/// 若距离威胁超过 `SPIDER_RETREAT_RADIUS` 则也视为撤退完成。
#[derive(Clone, Copy, Debug, Component)]
pub struct SpiderRetreatAction;

impl ActionBuilder for SpiderRetreatAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("SpiderRetreatAction")
    }
}

/// 撤退评分器：处于 Ambush 状态且玩家在 Ambush 范围内（感知半径外的威胁无需撤退）时触发。
/// 实际使用场景：蛛暴起后被玩家还击（hp 低于阈值），由外部 hp-check scorer 触发。
/// P1 简化实现：Ambush 状态下随机撤退（score = 0.4，低于 Ambush scorer 的 1.0）。
#[derive(Clone, Copy, Debug, Component)]
pub struct SpiderRetreatScorer;

impl ScorerBuilder for SpiderRetreatScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("SpiderRetreatScorer")
    }
}

type SpiderRetreatActorQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static mut SpiderDisguiseState,
        &'static MimicSpiderBlackboard,
        &'static mut Navigator,
    ),
    (With<NpcMarker>, Without<ClientMarker>),
>;

type SpiderRetreatScorerActorQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Position, &'static SpiderDisguiseState),
    (With<NpcMarker>, Without<ClientMarker>),
>;

pub(crate) fn spider_retreat_scorer_system(
    spiders: SpiderRetreatScorerActorQuery<'_, '_>,
    players: PlayerCultivationQuery<'_, '_>,
    mut scorers: Query<(&Actor, &mut Score), With<SpiderRetreatScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let Ok((spider_pos, state)) = spiders.get(*actor) else {
            score.set(0.0);
            continue;
        };

        // 只在 Ambush 期才考虑撤退
        if *state != SpiderDisguiseState::Ambush {
            score.set(0.0);
            continue;
        }

        // 有玩家在退路上（感知半径内）→ 撤退分值 0.4（低于 Ambush=1.0，Ambush 优先）
        // 无玩家 → 0.0（无需撤退）
        // P1 策略：Ambush 状态且附近仍有玩家时给出基线分，让外部 hp-scorer 可以在必要时覆盖
        let has_nearby_player = players
            .iter()
            .any(|(_, pp, _)| within_sense_radius(spider_pos.get(), pp.get()));

        score.set(if has_nearby_player { 0.4 } else { 0.0 });
    }
}

pub(crate) fn spider_retreat_action_system(
    mut spiders: SpiderRetreatActorQuery<'_, '_>,
    players: PlayerCultivationQuery<'_, '_>,
    mut actions: Query<(&Actor, &mut ActionState), With<SpiderRetreatAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((position, mut disguise_state, blackboard, mut navigator)) = spiders.get_mut(*actor)
        else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                *disguise_state = SpiderDisguiseState::Retreat;
                *state = ActionState::Executing;
            }

            ActionState::Executing => {
                let pos = position.get();

                // 找最近玩家（作为威胁参考方向）
                let threat_pos = players
                    .iter()
                    .min_by(|(_, pa, _), (_, pb, _)| {
                        pos.distance(pa.get())
                            .partial_cmp(&pos.distance(pb.get()))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(_, pp, _)| pp.get());

                // 检查是否满足撤退完成条件
                let retreat_done = threat_pos
                    .map(|threat| retreat_complete(pos, threat))
                    .unwrap_or(true); // 无威胁 → 直接完成

                if retreat_done || pos.distance(blackboard.home_pos) <= 2.0 {
                    navigator.stop();
                    *disguise_state = SpiderDisguiseState::Disguised;
                    *state = ActionState::Success;
                    continue;
                }

                // 向 home_pos 方向移动（低灵气区往往在出生地附近）
                navigator.set_goal(
                    backtrack_target(pos, blackboard.home_pos),
                    SPIDER_RETREAT_SPEED,
                );
            }

            ActionState::Cancelled => {
                navigator.stop();
                *state = ActionState::Failure;
            }

            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

/// 计算撤退目标点：朝 home_pos 方向前进一步（避免直接跳到 home_pos 跳过寻路）。
fn backtrack_target(pos: DVec3, home: DVec3) -> DVec3 {
    let dir = (home - pos).with_y(0.0);
    if dir.length_squared() < 1e-6 {
        return home; // 已到 home 附近
    }
    // 朝 home 移动一步（距离 ≤ RETREAT_RADIUS，让寻路系统分段处理）
    let step = dir.normalize() * (SPIDER_RETREAT_RADIUS / 4.0);
    pos + step
}

// ── ThinkerBuilder ────────────────────────────────────────────────────────────

/// 拟态灰烬蛛 big-brain thinker（供 spawn_spider.rs 使用）。
///
/// 优先级：AmbushScorer(1.0) > RetreatScorer(0.4) > WanderScorer(0.08 baseline)
#[allow(dead_code)]
pub fn spider_thinker() -> big_brain::prelude::ThinkerBuilder {
    use crate::npc::brain::{ChaseAction, ChaseTargetScorer, MeleeAttackAction, MeleeRangeScorer};
    use big_brain::prelude::{FirstToScore, Thinker};

    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(SpiderAmbushScorer, SpiderAmbushAction)
        .when(SpiderRetreatScorer, SpiderRetreatAction)
        .when(MeleeRangeScorer, MeleeAttackAction)
        .when(ChaseTargetScorer, ChaseAction)
}

// ── 注册 ──────────────────────────────────────────────────────────────────────

pub fn register(app: &mut App) {
    app.add_event::<SpiderAmbushTriggerEvent>();
    app.add_systems(
        PreUpdate,
        (spider_ambush_scorer_system, spider_retreat_scorer_system).in_set(BigBrainSet::Scorers),
    );
    app.add_systems(
        PreUpdate,
        (spider_ambush_action_system, spider_retreat_action_system).in_set(BigBrainSet::Actions),
    );
}

// ── 测试 ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Events, Update};

    use crate::cultivation::components::{Cultivation, Realm};
    use crate::fauna::mimic_spider::{
        MimicSpiderBlackboard, SpiderDisguiseState, SPIDER_SENSE_RADIUS,
    };
    use crate::npc::navigator::Navigator;
    use crate::npc::spawn::NpcMarker;

    // ── 测试工具 ────────────────────────────────────────────────────────────

    fn cultivation_with_qi(qi_current: f64, qi_max: f64) -> Cultivation {
        Cultivation {
            realm: Realm::Induce,
            qi_current,
            qi_max,
            ..Default::default()
        }
    }

    fn make_blackboard(zone: &str, home_pos: DVec3) -> MimicSpiderBlackboard {
        MimicSpiderBlackboard::new(zone, home_pos)
    }

    /// 构造一个最小化带 big-brain 的 App（没有完整 BigBrainPlugin，只注册系统）。
    fn spider_test_app() -> App {
        let mut app = App::new();
        app.add_event::<VfxEventRequest>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_event::<SpiderAmbushTriggerEvent>();
        app.add_systems(
            Update,
            (
                spider_ambush_scorer_system,
                spider_retreat_scorer_system,
                spider_ambush_action_system,
                spider_retreat_action_system,
            ),
        );
        app
    }

    // ── SpiderAmbushScorer 测试 ──────────────────────────────────────────────

    #[test]
    fn ambush_scorer_scores_zero_when_disguised_no_player() {
        // Disguised 蛛，无玩家 → score 应为 0
        let mut app = spider_test_app();
        let spider_pos = DVec3::new(0.0, 64.0, 0.0);
        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Disguised,
                make_blackboard("spawn", spider_pos),
            ))
            .id();

        use big_brain::prelude::Score;
        let scorer_entity = app
            .world_mut()
            .spawn((Actor(spider), Score::default(), SpiderAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer_entity).unwrap().get();
        assert_eq!(
            score, 0.0,
            "无玩家时 SpiderAmbushScorer 应为 0（期望 0.0，实际 {score}）"
        );
    }

    #[test]
    fn ambush_scorer_scores_one_when_player_in_range_with_qi() {
        // Disguised 蛛，玩家在感知半径内且真元超阈值 → score=1.0
        let mut app = spider_test_app();
        let spider_pos = DVec3::new(0.0, 64.0, 0.0);
        let player_pos = DVec3::new(4.0, 64.0, 0.0); // 距离 4 < SPIDER_SENSE_RADIUS=8

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Disguised,
                make_blackboard("spawn", spider_pos),
            ))
            .id();

        // 玩家真元超阈值（qi_current > qi_max * 0.1）
        app.world_mut().spawn((
            ClientMarker,
            Position::new([player_pos.x, player_pos.y, player_pos.z]),
            cultivation_with_qi(5.0, 10.0), // 5.0 > 1.0
        ));

        use big_brain::prelude::Score;
        let scorer_entity = app
            .world_mut()
            .spawn((Actor(spider), Score::default(), SpiderAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer_entity).unwrap().get();
        assert_eq!(
            score, 1.0,
            "玩家在感知半径内且真元超阈值时 SpiderAmbushScorer 应为 1.0（实际 {score}）"
        );
    }

    #[test]
    fn ambush_scorer_scores_zero_for_player_below_qi_threshold() {
        // 玩家真元等于阈值边界（不超过），不触发
        let mut app = spider_test_app();
        let spider_pos = DVec3::new(0.0, 64.0, 0.0);
        let player_pos = DVec3::new(3.0, 64.0, 0.0);

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Disguised,
                make_blackboard("spawn", spider_pos),
            ))
            .id();

        // 恰好等于阈值（== 0.1 × qi_max，严格大于才触发）
        app.world_mut().spawn((
            ClientMarker,
            Position::new([player_pos.x, player_pos.y, player_pos.z]),
            cultivation_with_qi(1.0, 10.0), // 1.0 == 10.0 * 0.1，不满足 >
        ));

        use big_brain::prelude::Score;
        let scorer_entity = app
            .world_mut()
            .spawn((Actor(spider), Score::default(), SpiderAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer_entity).unwrap().get();
        assert_eq!(
            score, 0.0,
            "玩家真元 == 阈值时不应触发感知（严格大于）（实际 {score}）"
        );
    }

    #[test]
    fn ambush_scorer_scores_zero_when_player_out_of_sense_radius() {
        // 玩家超出感知半径 → score=0，即使真元充足
        let mut app = spider_test_app();
        let spider_pos = DVec3::new(0.0, 64.0, 0.0);
        let player_pos = DVec3::new(SPIDER_SENSE_RADIUS + 0.1, 64.0, 0.0); // 超出半径

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Disguised,
                make_blackboard("spawn", spider_pos),
            ))
            .id();

        app.world_mut().spawn((
            ClientMarker,
            Position::new([player_pos.x, player_pos.y, player_pos.z]),
            cultivation_with_qi(100.0, 200.0),
        ));

        use big_brain::prelude::Score;
        let scorer_entity = app
            .world_mut()
            .spawn((Actor(spider), Score::default(), SpiderAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer_entity).unwrap().get();
        assert_eq!(
            score, 0.0,
            "玩家在感知半径外时 score 应为 0（实际 {score}）"
        );
    }

    #[test]
    fn ambush_scorer_scores_zero_when_not_disguised() {
        // 非 Disguised 状态（Ambush 中）不触发感知
        let mut app = spider_test_app();
        let spider_pos = DVec3::new(0.0, 64.0, 0.0);
        let player_pos = DVec3::new(2.0, 64.0, 0.0);

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Ambush, // 已处于 Ambush，感知不再重触发
                make_blackboard("spawn", spider_pos),
            ))
            .id();

        app.world_mut().spawn((
            ClientMarker,
            Position::new([player_pos.x, player_pos.y, player_pos.z]),
            cultivation_with_qi(50.0, 100.0),
        ));

        use big_brain::prelude::Score;
        let scorer_entity = app
            .world_mut()
            .spawn((Actor(spider), Score::default(), SpiderAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer_entity).unwrap().get();
        assert_eq!(
            score, 0.0,
            "非 Disguised 状态时感知评分应为 0（实际 {score}）"
        );
    }

    // ── SpiderAmbushAction 测试 ──────────────────────────────────────────────

    #[test]
    fn ambush_action_transitions_disguised_to_ambush_on_requested() {
        // Requested 时蛛从 Disguised 切换到 Ambush
        let mut app = spider_test_app();
        let spider_pos = DVec3::new(0.0, 64.0, 0.0);

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Disguised,
                make_blackboard("spawn", spider_pos),
                Navigator::new(),
            ))
            .id();

        // 添加一个玩家在附近（让 Executing 不立即 Success）
        let player_pos = DVec3::new(5.0, 64.0, 0.0);
        app.world_mut().spawn((
            ClientMarker,
            Position::new([player_pos.x, player_pos.y, player_pos.z]),
            cultivation_with_qi(50.0, 100.0),
        ));

        use big_brain::prelude::ActionState;
        app.world_mut()
            .spawn((Actor(spider), ActionState::Requested, SpiderAmbushAction));

        app.update();

        let new_state = app.world().get::<SpiderDisguiseState>(spider).unwrap();
        assert_eq!(
            *new_state,
            SpiderDisguiseState::Ambush,
            "Requested → 应切换到 Ambush 状态（实际 {new_state:?}）"
        );
    }

    #[test]
    fn ambush_action_emits_vfx_and_audio_on_requested() {
        // Requested 时应同时 emit VFX 事件和音效事件
        let mut app = spider_test_app();
        let spider_pos = DVec3::new(10.0, 64.0, 5.0);

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Disguised,
                make_blackboard("spawn", spider_pos),
                Navigator::new(),
            ))
            .id();

        // 玩家在附近
        let player_pos = DVec3::new(13.0, 64.0, 5.0);
        app.world_mut().spawn((
            ClientMarker,
            Position::new([player_pos.x, player_pos.y, player_pos.z]),
            cultivation_with_qi(50.0, 100.0),
        ));

        use big_brain::prelude::ActionState;
        app.world_mut()
            .spawn((Actor(spider), ActionState::Requested, SpiderAmbushAction));

        app.update();

        // 验证 VFX 事件已 emit
        let vfx_count = app
            .world()
            .resource::<Events<VfxEventRequest>>()
            .iter_current_update_events()
            .count();
        assert!(
            vfx_count > 0,
            "暴起时必须 emit VFX 事件（实际 count={vfx_count}）"
        );

        // 验证音效事件已 emit
        let audio_count = app
            .world()
            .resource::<Events<PlaySoundRecipeRequest>>()
            .iter_current_update_events()
            .count();
        assert!(
            audio_count > 0,
            "暴起时必须 emit 音效事件（实际 count={audio_count}）"
        );
    }

    #[test]
    fn ambush_action_emits_ambush_trigger_event_on_requested() {
        // 暴起时应 emit SpiderAmbushTriggerEvent
        let mut app = spider_test_app();
        let spider_pos = DVec3::new(5.0, 64.0, 5.0);

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Disguised,
                make_blackboard("spawn", spider_pos),
                Navigator::new(),
            ))
            .id();

        let player_pos = DVec3::new(7.0, 64.0, 5.0);
        app.world_mut().spawn((
            ClientMarker,
            Position::new([player_pos.x, player_pos.y, player_pos.z]),
            cultivation_with_qi(50.0, 100.0),
        ));

        use big_brain::prelude::ActionState;
        app.world_mut()
            .spawn((Actor(spider), ActionState::Requested, SpiderAmbushAction));

        app.update();

        let trigger_events: Vec<_> = app
            .world()
            .resource::<Events<SpiderAmbushTriggerEvent>>()
            .iter_current_update_events()
            .collect();
        assert_eq!(
            trigger_events.len(),
            1,
            "应恰好 emit 一个 SpiderAmbushTriggerEvent（实际 {}）",
            trigger_events.len()
        );
        assert_eq!(
            trigger_events[0].spider,
            spider.index(),
            "SpiderAmbushTriggerEvent.spider 应与蛛 Entity index 一致"
        );
    }

    #[test]
    fn ambush_action_reverts_to_disguised_when_no_player() {
        // Executing 时无玩家 → 回 Disguised
        let mut app = spider_test_app();
        let spider_pos = DVec3::new(0.0, 64.0, 0.0);

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Ambush,
                make_blackboard("spawn", spider_pos),
                Navigator::new(),
            ))
            .id();

        use big_brain::prelude::ActionState;
        let action = app
            .world_mut()
            .spawn((Actor(spider), ActionState::Executing, SpiderAmbushAction))
            .id();

        app.update();

        let new_state = app.world().get::<SpiderDisguiseState>(spider).unwrap();
        assert_eq!(
            *new_state,
            SpiderDisguiseState::Disguised,
            "无玩家时 Executing 应回 Disguised（实际 {new_state:?}）"
        );
        let action_state = app.world().get::<ActionState>(action).unwrap();
        assert_eq!(
            *action_state,
            ActionState::Success,
            "无玩家时 action 应 Success（实际 {action_state:?}）"
        );
    }

    #[test]
    fn ambush_action_cancelled_sets_failure() {
        // Cancelled 时 action 应变 Failure
        let mut app = spider_test_app();
        let spider_pos = DVec3::new(0.0, 64.0, 0.0);

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Ambush,
                make_blackboard("spawn", spider_pos),
                Navigator::new(),
            ))
            .id();

        use big_brain::prelude::ActionState;
        let action = app
            .world_mut()
            .spawn((Actor(spider), ActionState::Cancelled, SpiderAmbushAction))
            .id();

        app.update();

        let state = app.world().get::<ActionState>(action).unwrap();
        assert_eq!(
            *state,
            ActionState::Failure,
            "Cancelled 时 action 应变 Failure（实际 {state:?}）"
        );
    }

    // ── SpiderRetreatAction 测试 ─────────────────────────────────────────────

    #[test]
    fn retreat_action_transitions_ambush_to_retreat_on_requested() {
        // Requested 时从 Ambush → Retreat
        let mut app = spider_test_app();
        let spider_pos = DVec3::new(0.0, 64.0, 0.0);
        let home_pos = DVec3::new(5.0, 64.0, 5.0);

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Ambush,
                make_blackboard("spawn", home_pos),
                Navigator::new(),
            ))
            .id();

        use big_brain::prelude::ActionState;
        app.world_mut()
            .spawn((Actor(spider), ActionState::Requested, SpiderRetreatAction));

        app.update();

        let new_state = app.world().get::<SpiderDisguiseState>(spider).unwrap();
        assert_eq!(
            *new_state,
            SpiderDisguiseState::Retreat,
            "Requested → 应切换到 Retreat 状态（实际 {new_state:?}）"
        );
    }

    #[test]
    fn retreat_action_completes_when_no_threat_present() {
        // 无威胁玩家时撤退立即完成，回 Disguised
        let mut app = spider_test_app();
        let spider_pos = DVec3::new(0.0, 64.0, 0.0);

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Retreat,
                make_blackboard("spawn", spider_pos),
                Navigator::new(),
            ))
            .id();

        use big_brain::prelude::ActionState;
        let action = app
            .world_mut()
            .spawn((Actor(spider), ActionState::Executing, SpiderRetreatAction))
            .id();

        app.update();

        let new_state = app.world().get::<SpiderDisguiseState>(spider).unwrap();
        assert_eq!(
            *new_state,
            SpiderDisguiseState::Disguised,
            "无威胁时撤退完成应回 Disguised（实际 {new_state:?}）"
        );
        let action_state = app.world().get::<ActionState>(action).unwrap();
        assert_eq!(
            *action_state,
            ActionState::Success,
            "撤退完成时 action 应 Success（实际 {action_state:?}）"
        );
    }

    #[test]
    fn retreat_action_completes_when_beyond_retreat_radius() {
        // 蛛已超过撤退半径，即使有玩家也视为撤退完成
        let mut app = spider_test_app();
        let threat_pos = DVec3::new(0.0, 64.0, 0.0);
        // 蛛位置距威胁 > SPIDER_RETREAT_RADIUS
        let spider_pos = DVec3::new(
            crate::fauna::mimic_spider::SPIDER_RETREAT_RADIUS + 1.0,
            64.0,
            0.0,
        );

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Retreat,
                make_blackboard("spawn", spider_pos),
                Navigator::new(),
            ))
            .id();

        // 玩家在原点（威胁方向）
        app.world_mut().spawn((
            ClientMarker,
            Position::new([threat_pos.x, threat_pos.y, threat_pos.z]),
            cultivation_with_qi(50.0, 100.0),
        ));

        use big_brain::prelude::ActionState;
        let action = app
            .world_mut()
            .spawn((Actor(spider), ActionState::Executing, SpiderRetreatAction))
            .id();

        app.update();

        let new_state = app.world().get::<SpiderDisguiseState>(spider).unwrap();
        assert_eq!(
            *new_state,
            SpiderDisguiseState::Disguised,
            "超过撤退半径时应回 Disguised（实际 {new_state:?}）"
        );
        let action_state = app.world().get::<ActionState>(action).unwrap();
        assert_eq!(
            *action_state,
            ActionState::Success,
            "超过撤退半径时 action 应 Success（实际 {action_state:?}）"
        );
    }

    #[test]
    fn retreat_action_navigates_toward_home_when_threat_nearby() {
        // 有威胁玩家且距离 < 撤退半径时，navigator 应被设置目标（继续移动）
        let mut app = spider_test_app();
        let spider_pos = DVec3::new(0.0, 64.0, 0.0);
        let home_pos = DVec3::new(15.0, 64.0, 0.0);
        let threat_pos = DVec3::new(-2.0, 64.0, 0.0); // 威胁在蛛正后方

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Retreat,
                make_blackboard("spawn", home_pos),
                Navigator::new(),
            ))
            .id();

        // 威胁在感知半径内但蛛未超过撤退半径
        app.world_mut().spawn((
            ClientMarker,
            Position::new([threat_pos.x, threat_pos.y, threat_pos.z]),
            cultivation_with_qi(50.0, 100.0),
        ));

        use big_brain::prelude::ActionState;
        app.world_mut()
            .spawn((Actor(spider), ActionState::Executing, SpiderRetreatAction));

        app.update();

        // 蛛应保持 Retreat 状态（未完成撤退，距威胁 2.0 < 32.0）
        let new_state = app.world().get::<SpiderDisguiseState>(spider).unwrap();
        assert_eq!(
            *new_state,
            SpiderDisguiseState::Retreat,
            "未超过撤退半径时应继续 Retreat（实际 {new_state:?}）"
        );
    }

    // ── VFX / 音效常数 pin 测试 ──────────────────────────────────────────────

    #[test]
    fn ambush_vfx_event_id_pin() {
        assert_eq!(
            SPIDER_AMBUSH_VFX_EVENT_ID, "bong:vfx/spider_ambush",
            "VFX event ID 必须稳定，改变前先更新 client VfxBootstrap 注册"
        );
    }

    #[test]
    fn ambush_particle_color_pin() {
        assert_eq!(
            SPIDER_AMBUSH_PARTICLE_COLOR, "#B8D0C8",
            "粒子颜色应与 FaunaVisualKind::AshSpider event_color 一致"
        );
    }

    #[test]
    fn ambush_particle_count_pin() {
        assert_eq!(
            SPIDER_AMBUSH_PARTICLE_COUNT, 16,
            "暴起粒子数量应为 16（plan §P1 规格）"
        );
    }

    #[test]
    fn ambush_particle_duration_ticks_pin() {
        assert_eq!(
            SPIDER_AMBUSH_PARTICLE_DURATION_TICKS, 8,
            "粒子 lifetime 应为 8 tick（plan §P1 规格）"
        );
    }

    #[test]
    fn spider_step_recipe_id_pin() {
        assert_eq!(
            SPIDER_STEP_RECIPE_ID, "entity_spider_step",
            "音效 recipe ID 稳定性约束，改动须同步更新 client audio registry"
        );
    }

    #[test]
    fn ambush_volume_mul_pin() {
        assert!(
            (SPIDER_AMBUSH_VOLUME_MUL - 0.6).abs() < 1e-6,
            "暴起音效音量应为 0.6（实际 {SPIDER_AMBUSH_VOLUME_MUL}）"
        );
    }

    // ── backtrack_target 单元测试 ────────────────────────────────────────────

    #[test]
    fn backtrack_target_moves_toward_home() {
        let pos = DVec3::new(0.0, 64.0, 0.0);
        let home = DVec3::new(20.0, 64.0, 0.0);
        let target = backtrack_target(pos, home);
        assert!(
            target.x > pos.x,
            "撤退目标应朝 home 方向移动（期望 x > 0，实际 {target:?}）"
        );
        // 不应直接跳到 home（分步寻路）
        assert!(
            target.x < home.x,
            "撤退目标不应越过 home（期望 x < 20，实际 {target:?}）"
        );
    }

    #[test]
    fn backtrack_target_returns_home_when_at_same_position() {
        // 蛛已在 home 附近 → 直接返回 home
        let pos = DVec3::new(5.0, 64.0, 5.0);
        let home = DVec3::new(5.0, 64.0, 5.0); // 同一位置
        let target = backtrack_target(pos, home);
        assert_eq!(
            target, home,
            "蛛在 home 位置时 backtrack_target 应返回 home（实际 {target:?}）"
        );
    }

    // ── P3 陷阱归属感知测试 ──────────────────────────────────────────────────

    #[test]
    fn ambush_scorer_skips_trap_owner_player() {
        // 陷阱蛛（trapped_by = owner）不应被 owner 触发感知——
        // 只有第三方玩家才能触发暴起。
        let mut app = spider_test_app();
        let spider_pos = DVec3::new(0.0, 64.0, 0.0);

        // trap_owner：部署陷阱的玩家（蛛不感知它）
        let owner = app
            .world_mut()
            .spawn((
                ClientMarker,
                Position::new([2.0_f64, 64.0, 0.0]), // 距离 2 < SENSE_RADIUS=8
                cultivation_with_qi(50.0, 100.0),
            ))
            .id();

        let mut bb = make_blackboard("spawn", spider_pos);
        bb.trapped_by = Some(owner);

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Disguised,
                bb,
            ))
            .id();

        use big_brain::prelude::Score;
        let scorer_entity = app
            .world_mut()
            .spawn((Actor(spider), Score::default(), SpiderAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer_entity).unwrap().get();
        assert_eq!(
            score, 0.0,
            "陷阱蛛不应被 trap_owner 触发感知（owner 在范围内时 score 期望 0.0，实际 {score}）"
        );
    }

    #[test]
    fn ambush_scorer_triggers_on_third_party_player_not_trap_owner() {
        // 陷阱蛛（trapped_by = owner）：owner 在感知范围内不触发，但第三方玩家在范围内触发。
        let mut app = spider_test_app();
        let spider_pos = DVec3::new(0.0, 64.0, 0.0);

        // trap_owner：范围内，不触发
        let owner = app
            .world_mut()
            .spawn((
                ClientMarker,
                Position::new([2.0_f64, 64.0, 0.0]),
                cultivation_with_qi(50.0, 100.0),
            ))
            .id();

        // 第三方：也在范围内，应触发
        app.world_mut().spawn((
            ClientMarker,
            Position::new([3.0_f64, 64.0, 0.0]),
            cultivation_with_qi(50.0, 100.0),
        ));

        let mut bb = make_blackboard("spawn", spider_pos);
        bb.trapped_by = Some(owner);

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Disguised,
                bb,
            ))
            .id();

        use big_brain::prelude::Score;
        let scorer_entity = app
            .world_mut()
            .spawn((Actor(spider), Score::default(), SpiderAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer_entity).unwrap().get();
        assert_eq!(
            score, 1.0,
            "第三方玩家在感知范围内时陷阱蛛应触发感知（期望 1.0，实际 {score}）"
        );
    }

    #[test]
    fn wild_spider_triggers_on_all_players() {
        // 野生蛛（trapped_by=None）应对所有玩家触发感知（无排除逻辑）
        let mut app = spider_test_app();
        let spider_pos = DVec3::new(0.0, 64.0, 0.0);

        // 只有一个玩家（无 trap_owner 概念）
        app.world_mut().spawn((
            ClientMarker,
            Position::new([4.0_f64, 64.0, 0.0]),
            cultivation_with_qi(50.0, 100.0),
        ));

        // 野生蛛：trapped_by=None
        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Disguised,
                make_blackboard("spawn", spider_pos), // trapped_by=None
            ))
            .id();

        use big_brain::prelude::Score;
        let scorer_entity = app
            .world_mut()
            .spawn((Actor(spider), Score::default(), SpiderAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer_entity).unwrap().get();
        assert_eq!(
            score, 1.0,
            "野生蛛对所有玩家触发感知（期望 1.0，实际 {score}）"
        );
    }

    #[test]
    fn ambush_scorer_zero_when_only_owner_present_for_trap_spider() {
        // 陷阱蛛感知范围内只有 owner，无第三方 → score=0
        let mut app = spider_test_app();
        let spider_pos = DVec3::new(0.0, 64.0, 0.0);

        let owner = app
            .world_mut()
            .spawn((
                ClientMarker,
                Position::new([1.0_f64, 64.0, 0.0]),
                cultivation_with_qi(200.0, 500.0), // 即使高境界也不触发
            ))
            .id();

        let mut bb = make_blackboard("spawn", spider_pos);
        bb.trapped_by = Some(owner);

        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([spider_pos.x, spider_pos.y, spider_pos.z]),
                SpiderDisguiseState::Disguised,
                bb,
            ))
            .id();

        use big_brain::prelude::Score;
        let scorer_entity = app
            .world_mut()
            .spawn((Actor(spider), Score::default(), SpiderAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer_entity).unwrap().get();
        assert_eq!(
            score, 0.0,
            "感知范围内只有 owner 时陷阱蛛 score 必须为 0（即使 owner 境界极高）（实际 {score}）"
        );
    }
}
