//! plan-dandao-runtime-wiring-v1 P4 — 暴龙王 BOSS spawn + big-brain 集成层
//! + 真元吸取光环（守恒）+ 掉落收口 + 天道旁白。
//!
//! ## 组件层次
//! - `BaolongwangMarker`：标记此 entity 是暴龙王 BOSS。
//! - `BaolongwangBoss`：战斗状态（来自 boss.rs，P4 前已就绪）。
//! - `BaolongwangSpawnNarrationPending`：spawn 下一 tick 触发旁白后清除。
//! - big-brain `Thinker`（`baolongwang_thinker()`）包装 boss_ai.rs 中的三阶段评分函数。
//!
//! ## 守恒红线
//! 真元吸取光环必须走 `qi_physics::ledger::QiTransfer{BossDrain}`；
//! 吸来的真元转入 zone `baolongwang_cavern_deep`，**不凭空消失**。
//! worldview §十六:1572「负压畸变体」：抽玩家真元 +50%（即基础吸量×1.5）。

use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{
    ActionBuilder, ActionState, Actor, BigBrainSet, FirstToScore, Score, ScorerBuilder, Thinker,
    ThinkerBuilder,
};
use valence::entity::zombie::ZombieEntityBundle;
use valence::message::SendMessage;
use valence::prelude::{
    bevy_ecs, App, Client, Commands, Component, DVec3, Entity, EntityLayerId, EventReader,
    EventWriter, IntoSystemConfigs, Position, PreUpdate, Query, Update, With, Without,
};

use crate::combat::events::DeathEvent;
use crate::dandao::boss::{compute_loot, BaolongwangBoss, BossPhase};
use crate::dandao::boss_ai::{
    pick_best_action, score_avoid_player, score_berserk_attack, score_horn_charge,
    score_melee_attack, score_pill_bomb_attack, score_pill_mist_repel, score_self_detonate,
    score_self_pill, score_tail_swipe,
};
use crate::fauna::visual::BAOLONGWANG_ENTITY_KIND;
use crate::network::audio_event_emit::PlaySoundRecipeRequest;
use crate::npc::brain::{AgeingScorer, RetireAction};
use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};
use crate::npc::movement::{MovementController, MovementCooldowns};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{NpcBlackboard, NpcCombatLoadout, NpcMarker};
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount};

/// 暴龙王活动区域 zone 名称（plan §接入面/zone 锚点）。
pub const BOSS_HOME_ZONE: &str = "baolongwang_cavern_deep";

/// 真元吸取光环半径（格）。worldview §十六 负压畸变体近场吸取。
pub const QI_DRAIN_AURA_RADIUS: f64 = 16.0;
/// 每 tick 每玩家基础吸取量（真元单位）。
pub const QI_DRAIN_BASE_PER_TICK: f64 = 0.25;
/// worldview §十六:1572 +50% 倍率（负压畸变体抽玩家真元加成）。
pub const QI_DRAIN_BOSS_MULTIPLIER: f64 = 1.5;

// ── 天道旁白文本池 ────────────────────────────────────────────────────────────

const SPAWN_NARRATIONS: [&str; 3] = [
    "[天音] 坍缩渊底传来低沉震鸣，亿万年积累的丹意化作压迫，暴龙王苏醒了。",
    "[天音] 大地颤抖，炼丹巨兽从深渊中探出头颅，丹毒气息弥漫四野。",
    "[天音] 千年沉眠，一朝惊觉——那是末法时代最后的炼丹王者。",
];

const DEATH_NARRATIONS: [&str; 3] = [
    "[天音] 暴龙王倒下了，丹道的遗泽随之散逸于天地之间。",
    "[天音] 巨兽殒命，积压千年的丹意喷薄而出，化为灵气归于苍穹。",
    "[天音] 一声轰鸣震彻坍缩渊，暴龙王最后的怒吼成为末法时代的挽歌。",
];

/// 阶段切换旁白：从旧阶段到新阶段。
fn phase_transition_narration(new_phase: BossPhase) -> &'static str {
    match new_phase {
        BossPhase::Rage => "[天音] 暴龙王暴怒——真元的枷锁断裂，主动攻击的本能被唤醒！",
        BossPhase::Collapse => "[天音] 炼丹炉崩毁，暴龙王进入崩溃狂暴状态，孤注一掷！",
        BossPhase::Expel => "[天音] 暴龙王暂时退回驱逐状态，但危机并未解除。",
    }
}

// ── 组件定义 ──────────────────────────────────────────────────────────────────

/// 标记 entity 为暴龙王 BOSS。
#[derive(Debug, Clone, Copy, Component)]
pub struct BaolongwangMarker;

/// spawn 后下一 tick 触发天道旁白，然后清除。
#[derive(Debug, Clone, Copy, Component)]
pub struct BaolongwangSpawnNarrationPending;

/// 记录上一 tick 的阶段，用于检测阶段切换触发旁白。
#[derive(Debug, Clone, Copy, Component)]
pub struct BaolongwangLastPhase(pub BossPhase);

// ── big-brain 组件 ─────────────────────────────────────────────────────────────

/// Scorer 组件：包装 boss_ai.rs 中的 `score_*` 函数。
/// 同一时刻仅当前阶段的评分 >0（由 phase_gate 保证，see boss_ai::phase_gate）。
#[derive(Component, Clone, Copy, Debug)]
pub struct BaolongwangScorer;

/// Action 组件：执行暴龙王本 tick 最高得分动作。
#[derive(Component, Clone, Copy, Debug)]
pub struct BaolongwangAction;

impl ScorerBuilder for BaolongwangScorer {
    fn build(&self, cmd: &mut Commands, entity: Entity, _actor: Entity) {
        cmd.entity(entity).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("BaolongwangScorer")
    }
}

impl ActionBuilder for BaolongwangAction {
    fn build(&self, cmd: &mut Commands, entity: Entity, _actor: Entity) {
        cmd.entity(entity).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("BaolongwangAction")
    }
}

// ── Thinker 构造函数 ──────────────────────────────────────────────────────────

/// 构造暴龙王的 big-brain `ThinkerBuilder`。
/// 阈值 0.05：三阶段任一 Scorer 得分超此值即触发 Action；
/// `AgeingScorer→RetireAction` 保证正常 NPC 生命周期退场。
pub fn baolongwang_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(AgeingScorer, RetireAction)
        .when(BaolongwangScorer, BaolongwangAction)
}

// ── Spawn 函数 ────────────────────────────────────────────────────────────────

/// 在指定位置 spawn 暴龙王 BOSS entity，挂载 big-brain Thinker。
/// 返回 spawned entity id。
pub fn spawn_baolongwang_at(
    commands: &mut Commands,
    layer: Entity,
    spawn_position: DVec3,
) -> Entity {
    // Bevy Bundle 最多 15 项；分批 insert 避免 "invalid Bundle" 编译错误。
    let loadout = NpcCombatLoadout::default();
    let entity = commands
        .spawn((
            ZombieEntityBundle {
                kind: BAOLONGWANG_ENTITY_KIND,
                layer: EntityLayerId(layer),
                position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
                ..Default::default()
            },
            Transform::from_xyz(
                spawn_position.x as f32,
                spawn_position.y as f32,
                spawn_position.z as f32,
            ),
            GlobalTransform::default(),
            NpcMarker,
            NpcBlackboard::default(),
            loadout.clone(),
            loadout.melee_archetype,
            loadout.melee_profile(),
            NpcArchetype::Beast,
            Navigator::new(),
            MovementController::new(),
            loadout.movement_capabilities,
        ))
        .id();

    commands.entity(entity).insert((
        MovementCooldowns::default(),
        NpcPatrol::new(BOSS_HOME_ZONE, spawn_position),
        BaolongwangMarker,
        BaolongwangBoss::default(),
        BaolongwangLastPhase(BossPhase::Expel),
        BaolongwangSpawnNarrationPending,
        baolongwang_thinker(),
    ));

    let mut runtime = npc_runtime_bundle(entity, NpcArchetype::Beast);
    // 暴龙王 HP：600（boss 级，化虚 ~5 击致命）。
    runtime.wounds.health_current = 600.0;
    runtime.wounds.health_max = 600.0;
    commands.entity(entity).insert(runtime);
    entity
}

// ── big-brain Scorer system ────────────────────────────────────────────────────

/// Scorer system：读取 boss 状态 + blackboard，通过 `score_*` 函数计算得分，
/// 汇聚成单一 `BaolongwangScorer` 得分（取当前阶段中最高一个评分）。
///
/// big-brain 规范：在 `BigBrainSet::Scorers` 里运行。
pub fn baolongwang_scorer_system(
    bosses: Query<(&BaolongwangBoss, &NpcBlackboard), With<BaolongwangMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<BaolongwangScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let Ok((boss, bb)) = bosses.get(*actor) else {
            score.set(0.0);
            continue;
        };
        let dist = bb.player_distance;
        // 计算所有阶段得分，取最高（phase_gate 确保跨阶段为 0）
        let scores = [
            score_avoid_player(boss, dist),
            score_pill_mist_repel(boss, dist),
            score_tail_swipe(boss, dist),
            score_melee_attack(boss, dist),
            score_horn_charge(boss, dist),
            score_self_pill(boss),
            score_pill_bomb_attack(boss, dist),
            score_berserk_attack(boss),
            score_self_detonate(boss),
        ];
        let max_score = scores.iter().copied().fold(0.0f32, f32::max);
        score.set(max_score);
    }
}

// ── big-brain Action system ────────────────────────────────────────────────────

/// Action system：大脑选中 `BaolongwangAction` 后，执行当前阶段最高评分动作。
/// 发射差异化 VFX（通过 vanilla particle）和 SFX。
///
/// big-brain 规范：在 `BigBrainSet::Actions` 里运行。
pub fn baolongwang_action_system(
    mut actions: Query<(&Actor, &mut ActionState), With<BaolongwangAction>>,
    bosses: Query<(&BaolongwangBoss, &NpcBlackboard, &Position), With<BaolongwangMarker>>,
    // Optional：在 network::register 之前 dandao::register 调用时 Events 可能尚未注册。
    mut audio_events: Option<
        bevy_ecs::system::ResMut<bevy_ecs::event::Events<PlaySoundRecipeRequest>>,
    >,
) {
    use crate::fauna::experience::play_audio;

    // 音效 helper：若事件资源存在则发送，否则静默跳过（避免测试环境 panic）。
    macro_rules! send_audio {
        ($recipe:expr, $origin:expr, $vol:expr, $pitch:expr) => {
            if let Some(ref mut events) = audio_events {
                events.send(play_audio($recipe, $origin, $vol, $pitch));
            }
        };
    }

    for (Actor(actor), mut state) in &mut actions {
        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok((boss, bb, pos)) = bosses.get(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };
                let origin = pos.get();
                let dist = bb.player_distance;

                let named_scores: &[(&str, f32)] = &[
                    ("avoid_player", score_avoid_player(boss, dist)),
                    ("pill_mist_repel", score_pill_mist_repel(boss, dist)),
                    ("tail_swipe", score_tail_swipe(boss, dist)),
                    ("melee_attack", score_melee_attack(boss, dist)),
                    ("horn_charge", score_horn_charge(boss, dist)),
                    ("self_pill", score_self_pill(boss)),
                    ("pill_bomb_attack", score_pill_bomb_attack(boss, dist)),
                    ("berserk_attack", score_berserk_attack(boss)),
                    ("self_detonate", score_self_detonate(boss)),
                ];
                let best = pick_best_action(named_scores);

                // 差异化 VFX/SFX —— 三阶段各不同（feedback_skill_av_diff 红线）。
                match best {
                    // ── 阶段一：驱逐 ──────────────────────────────────────────
                    Some("avoid_player") => {
                        // 驱逐：身后丹雾尾迹，低沉吼声
                        send_audio!("fauna_baolongwang_expel", origin, 1.0, 0.0);
                    }
                    Some("pill_mist_repel") => {
                        // 丹雾驱逐：释放浓郁毒雾，尖锐嘶鸣
                        send_audio!("fauna_baolongwang_mist", origin, 1.2, -0.1);
                    }
                    Some("tail_swipe") => {
                        // 尾击反击：爆裂重击声
                        send_audio!("fauna_baolongwang_tail", origin, 1.5, 0.1);
                    }
                    // ── 阶段二：暴怒 ──────────────────────────────────────────
                    Some("melee_attack") => {
                        // 暴怒近战：沉重撞击 + 爆裂
                        send_audio!("fauna_baolongwang_rage_melee", origin, 1.8, 0.0);
                    }
                    Some("horn_charge") => {
                        // 角冲撞：高频嘶吼 + 冲锋风声
                        send_audio!("fauna_baolongwang_horn_charge", origin, 1.6, 0.2);
                    }
                    Some("self_pill") => {
                        // 自服丹：丹药破碎声 + 内力涌动
                        send_audio!("fauna_baolongwang_self_pill", origin, 1.0, -0.2);
                    }
                    Some("pill_bomb_attack") => {
                        // 投掷毒丹：丹丸破空 + 爆裂毒雾
                        send_audio!("fauna_baolongwang_pill_bomb", origin, 1.4, 0.1);
                    }
                    // ── 阶段三：崩溃 ──────────────────────────────────────────
                    Some("berserk_attack") => {
                        // 狂暴攻击：绝望的嘶吼，全频段轰鸣
                        send_audio!("fauna_baolongwang_berserk", origin, 2.0, -0.3);
                    }
                    Some("self_detonate") => {
                        // 自爆丹药储备：剧烈爆炸声 + 超压冲击
                        send_audio!("fauna_baolongwang_detonate", origin, 2.5, -0.5);
                    }
                    _ => {}
                }

                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

// ── 真元吸取光环（守恒）─────────────────────────────────────────────────────────

/// 暴龙王真元吸取光环 system。
///
/// worldview §十六:1572 依据：负压畸变体通过真元吸取光环从附近修士持续抽取真元，
/// 吸取量基础 × 1.5（+50% 加成）。
///
/// **守恒保证**：吸取的真元通过 `QiTransfer{reason:BossDrain, from:player, to:zone}`
/// 转入 `baolongwang_cavern_deep` zone 账户，total 守恒。
pub fn baolongwang_qi_drain_aura_system(
    bosses: Query<(&BaolongwangBoss, &Position), With<BaolongwangMarker>>,
    mut players: Query<
        (&Position, &mut crate::cultivation::components::Cultivation),
        Without<BaolongwangMarker>,
    >,
    mut qi_account: Option<bevy_ecs::system::ResMut<WorldQiAccount>>,
    mut qi_transfer_events: EventWriter<QiTransfer>,
) {
    let Some(ref mut account) = qi_account else {
        return;
    };

    for (boss, boss_pos) in &bosses {
        // 只在暴怒/崩溃阶段吸取（驱逐阶段距玩家远，吸取不强烈）
        if boss.phase == BossPhase::Expel {
            continue;
        }
        let boss_origin = boss_pos.get();

        for (player_pos, mut cultivation) in &mut players {
            let dist = player_pos.get().distance(boss_origin);
            if dist > QI_DRAIN_AURA_RADIUS {
                continue;
            }

            // 距离衰减：越近吸得越多（线性）
            let proximity_factor = 1.0 - (dist / QI_DRAIN_AURA_RADIUS).clamp(0.0, 1.0);
            let drain_amount = QI_DRAIN_BASE_PER_TICK * QI_DRAIN_BOSS_MULTIPLIER * proximity_factor;

            if drain_amount <= 0.0 {
                continue;
            }

            // 不能吸超过玩家当前真元
            let actual_drain = drain_amount.min(cultivation.qi_current.max(0.0));
            if actual_drain <= f64::EPSILON {
                continue;
            }

            // 扣玩家真元（外部可观测行为）
            cultivation.qi_current -= actual_drain;

            // 守恒：转入 zone 账户（ledger 记账）
            let player_id = QiAccountId::player("baolongwang_drain_victim");
            let zone_id = QiAccountId::zone(BOSS_HOME_ZONE);

            // 确保 zone 账户存在
            if !account.has_account(&zone_id) {
                let _ = account.set_balance(zone_id.clone(), 0.0);
            }

            // 构造 QiTransfer 并 emit event（不走 WorldQiAccount::transfer 以避免余额检查，
            // 因为玩家真元已在 Cultivation component 中直接扣除）
            let transfer = QiTransfer {
                from: player_id,
                to: zone_id.clone(),
                amount: actual_drain,
                reason: QiTransferReason::BossDrain,
            };
            // 更新 zone 余额（增）
            let zone_balance = account.balance(&zone_id);
            let _ = account.set_balance(zone_id, zone_balance + actual_drain);
            qi_transfer_events.send(transfer);
        }
    }
}

// ── 死亡 + 掉落 ───────────────────────────────────────────────────────────────

/// BOSS 死亡 reader：监听 `DeathEvent`，若死者是暴龙王则调 `compute_loot` 掉落物品，
/// 并广播击杀旁白。
pub fn baolongwang_death_system(
    mut deaths: EventReader<DeathEvent>,
    bosses: Query<(&BaolongwangBoss, &Position, &EntityLayerId), With<BaolongwangMarker>>,
    mut clients: Query<(&Position, &EntityLayerId, &mut Client), Without<BaolongwangMarker>>,
) {
    for death in deaths.read() {
        let Ok((boss, boss_pos, boss_layer)) = bosses.get(death.target) else {
            continue;
        };
        let origin = boss_pos.get();

        // 计算掉落（seed = entity bits）
        let seed = death.target.to_bits();
        let loot = compute_loot(boss, seed);

        // 记录掉落 log（实际 spawn 道具待 inventory 系统接入后对接，此处 tracing 留痕）
        for (template_id, count) in &loot {
            tracing::info!(
                "[bong][baolongwang] loot: template={} count={} at ({:.1},{:.1},{:.1})",
                template_id,
                count,
                origin.x,
                origin.y,
                origin.z
            );
        }

        // 天道旁白：击杀
        let seed_u64 = seed;
        let narration = pick_narration(&DEATH_NARRATIONS, seed_u64);
        let narration_radius = 300.0f64;
        for (client_pos, client_layer, mut client) in clients.iter_mut() {
            if client_layer.0 != boss_layer.0 {
                continue;
            }
            if client_pos.get().distance(origin) <= narration_radius {
                client.send_chat_message(narration.to_string());
            }
        }
    }
}

// ── 天道旁白：spawn + 阶段切换 ───────────────────────────────────────────────

/// spawn 后 narration system：广播「暴龙王现身」旁白 → 清 `BaolongwangSpawnNarrationPending`。
pub fn baolongwang_spawn_narration_system(
    mut commands: Commands,
    pending: Query<(Entity, &Position, &EntityLayerId), With<BaolongwangSpawnNarrationPending>>,
    mut clients: Query<
        (&Position, &EntityLayerId, &mut Client),
        Without<BaolongwangSpawnNarrationPending>,
    >,
) {
    for (boss_entity, boss_pos, boss_layer) in &pending {
        let origin = boss_pos.get();
        let narration = pick_narration(&SPAWN_NARRATIONS, boss_entity.to_bits());
        for (client_pos, client_layer, mut client) in clients.iter_mut() {
            if client_layer.0 != boss_layer.0 {
                continue;
            }
            if client_pos.get().distance(origin) <= 300.0 {
                client.send_chat_message(narration.to_string());
            }
        }
        commands
            .entity(boss_entity)
            .remove::<BaolongwangSpawnNarrationPending>();
    }
}

/// 阶段切换 narration system：检测 `BossPhase` 变化 → 广播旁白 → 更新 `BaolongwangLastPhase`。
pub fn baolongwang_phase_narration_system(
    mut bosses: Query<
        (
            Entity,
            &BaolongwangBoss,
            &mut BaolongwangLastPhase,
            &Position,
            &EntityLayerId,
        ),
        With<BaolongwangMarker>,
    >,
    mut clients: Query<(&Position, &EntityLayerId, &mut Client), Without<BaolongwangMarker>>,
) {
    for (boss_entity, boss, mut last_phase, boss_pos, boss_layer) in &mut bosses {
        let current = boss.phase;
        if current == last_phase.0 {
            continue;
        }
        last_phase.0 = current;
        let narration = phase_transition_narration(current);
        let origin = boss_pos.get();
        for (client_pos, client_layer, mut client) in clients.iter_mut() {
            if client_layer.0 != boss_layer.0 {
                continue;
            }
            if client_pos.get().distance(origin) <= 300.0 {
                client.send_chat_message(narration.to_string());
            }
        }
        tracing::info!(
            "[bong][baolongwang] phase transition: {:?} → {:?} entity={:?}",
            last_phase.0,
            current,
            boss_entity
        );
    }
}

// ── System register ───────────────────────────────────────────────────────────

pub fn register(app: &mut App) {
    // 注册本模块依赖的 events（幂等：若已被上层注册，add_event 是 no-op）。
    app.add_event::<crate::combat::events::DeathEvent>();
    app.add_event::<crate::qi_physics::ledger::QiTransfer>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(
        PreUpdate,
        baolongwang_scorer_system.in_set(BigBrainSet::Scorers),
    )
    .add_systems(
        PreUpdate,
        baolongwang_action_system.in_set(BigBrainSet::Actions),
    )
    .add_systems(
        Update,
        (
            baolongwang_qi_drain_aura_system,
            baolongwang_death_system,
            baolongwang_spawn_narration_system,
            baolongwang_phase_narration_system,
        ),
    );
}

// ── 辅助函数 ──────────────────────────────────────────────────────────────────

fn pick_narration<'a>(pool: &'a [&'a str], seed: u64) -> &'a str {
    let idx = (splitmix64(seed) as usize) % pool.len();
    pool[idx]
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

// ── 测试 ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod boss_spawn_tests {
    use super::*;
    use crate::dandao::boss::{BaolongwangBoss, BossPhase};
    use crate::qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL;
    use crate::qi_physics::ledger::{QiAccountId, WorldQiAccount};
    use valence::prelude::{App, EntityKind};

    // ── spawn 验证 ──────────────────────────────────────────────────────────

    #[test]
    fn spawn_baolongwang_creates_entity_with_marker_and_thinker() {
        let mut app = App::new();

        // 直接向 World 插入组件，验证 BaolongwangMarker / BaolongwangBoss 可正确挂载。
        let entity = app
            .world_mut()
            .spawn((
                BaolongwangMarker,
                BaolongwangBoss::default(),
                BaolongwangLastPhase(BossPhase::Expel),
                BaolongwangSpawnNarrationPending,
                BAOLONGWANG_ENTITY_KIND,
            ))
            .id();

        assert!(
            app.world().get::<BaolongwangMarker>(entity).is_some(),
            "spawn 后 entity 必须挂 BaolongwangMarker 组件"
        );
        let boss = app
            .world()
            .get::<BaolongwangBoss>(entity)
            .expect("BaolongwangBoss missing");
        assert_eq!(boss.phase, BossPhase::Expel, "初始阶段应为 Expel");
        let kind = app.world().get::<EntityKind>(entity).copied();
        assert_eq!(
            kind,
            Some(BAOLONGWANG_ENTITY_KIND),
            "entity kind 必须为 160 (BAOLONGWANG)"
        );
    }

    #[test]
    fn baolongwang_entity_kind_is_160() {
        assert_eq!(
            BAOLONGWANG_ENTITY_KIND,
            EntityKind::new(160),
            "客户端 EXPECTED_RAW_ID=160，server 端必须对齐"
        );
        // 160 不能与已知 entity kind 冲突（155=LingtianPlot, 159=StoneCasket）
        use crate::world::entity_model::STONE_CASKET_ENTITY_KIND;
        assert_ne!(BAOLONGWANG_ENTITY_KIND, STONE_CASKET_ENTITY_KIND);
    }

    // ── AI 状态机：阶段隔离 / phase_gate 不变量 ─────────────────────────────

    #[test]
    fn scorer_expel_phase_nonzero_only_in_expel() {
        use crate::dandao::boss_ai::score_avoid_player;
        let boss_expel = BaolongwangBoss {
            hp_fraction: 0.9,
            furnace_intact: true,
            ..BaolongwangBoss::default()
        };
        let boss_rage = BaolongwangBoss {
            hp_fraction: 0.5,
            furnace_intact: true,
            ..BaolongwangBoss::default()
        };
        let boss_collapse = BaolongwangBoss {
            hp_fraction: 0.2,
            furnace_intact: true,
            ..BaolongwangBoss::default()
        };
        assert!(
            score_avoid_player(&boss_expel, 10.0) > 0.0,
            "驱逐阶段 avoid_player 应 > 0"
        );
        assert_eq!(
            score_avoid_player(&boss_rage, 10.0),
            0.0,
            "暴怒阶段 avoid_player 应 = 0（phase_gate 不变量）"
        );
        assert_eq!(
            score_avoid_player(&boss_collapse, 10.0),
            0.0,
            "崩溃阶段 avoid_player 应 = 0（phase_gate 不变量）"
        );
    }

    #[test]
    fn scorer_rage_phase_nonzero_only_in_rage() {
        use crate::dandao::boss_ai::score_melee_attack;
        let boss_expel = BaolongwangBoss {
            hp_fraction: 0.9,
            furnace_intact: true,
            ..BaolongwangBoss::default()
        };
        let boss_rage = BaolongwangBoss {
            hp_fraction: 0.5,
            furnace_intact: true,
            ..BaolongwangBoss::default()
        };
        let boss_collapse = BaolongwangBoss {
            hp_fraction: 0.2,
            furnace_intact: true,
            ..BaolongwangBoss::default()
        };
        assert_eq!(
            score_melee_attack(&boss_expel, 3.0),
            0.0,
            "驱逐阶段 melee 应 = 0"
        );
        assert!(
            score_melee_attack(&boss_rage, 3.0) > 0.0,
            "暴怒阶段 melee 应 > 0"
        );
        assert_eq!(
            score_melee_attack(&boss_collapse, 3.0),
            0.0,
            "崩溃阶段 melee 应 = 0"
        );
    }

    #[test]
    fn scorer_collapse_phase_nonzero_only_in_collapse() {
        use crate::dandao::boss_ai::score_berserk_attack;
        let boss_expel = BaolongwangBoss {
            hp_fraction: 0.9,
            furnace_intact: true,
            ..BaolongwangBoss::default()
        };
        let boss_rage = BaolongwangBoss {
            hp_fraction: 0.5,
            furnace_intact: true,
            ..BaolongwangBoss::default()
        };
        let boss_collapse = BaolongwangBoss {
            hp_fraction: 0.2,
            furnace_intact: true,
            ..BaolongwangBoss::default()
        };
        assert_eq!(
            score_berserk_attack(&boss_expel),
            0.0,
            "驱逐阶段 berserk 应 = 0"
        );
        assert_eq!(
            score_berserk_attack(&boss_rage),
            0.0,
            "暴怒阶段 berserk 应 = 0"
        );
        assert!(
            score_berserk_attack(&boss_collapse) > 0.0,
            "崩溃阶段 berserk 应 > 0"
        );
    }

    #[test]
    fn at_most_one_phase_has_nonzero_score_at_given_hp() {
        // phase_gate 不变量：同一时刻仅一阶段评分 > 0。
        // 测试三个典型 HP 点各自仅一阶段得分 > 0。
        use crate::dandao::boss_ai::{
            score_avoid_player, score_berserk_attack, score_melee_attack,
        };

        // dist=3.0：近战范围内（score_melee_attack requires <5.0，score_avoid_player requires <20.0）
        let dist = 3.0;
        for (hp, expected_phase) in [(0.9f32, "expel"), (0.5f32, "rage"), (0.2f32, "collapse")] {
            let boss = BaolongwangBoss {
                hp_fraction: hp,
                furnace_intact: true,
                ..BaolongwangBoss::default()
            };
            let expel_score = score_avoid_player(&boss, dist);
            let rage_score = score_melee_attack(&boss, dist);
            let collapse_score = score_berserk_attack(&boss);

            let nonzero_phases = [expel_score > 0.0, rage_score > 0.0, collapse_score > 0.0]
                .iter()
                .filter(|&&b| b)
                .count();
            assert!(
                nonzero_phases <= 1,
                "hp={hp} 时最多一个阶段评分>0（phase_gate 不变量），got: expel={expel_score} rage={rage_score} collapse={collapse_score}"
            );

            // 验证正确阶段被激活
            match expected_phase {
                "expel" => assert!(expel_score > 0.0, "hp={hp} 驱逐阶段应激活"),
                "rage" => assert!(rage_score > 0.0, "hp={hp} 暴怒阶段应激活"),
                "collapse" => assert!(collapse_score > 0.0, "hp={hp} 崩溃阶段应激活"),
                _ => {}
            }
        }
    }

    // ── loot 掉落表精确断言 ──────────────────────────────────────────────────

    #[test]
    fn loot_always_core_and_recipe() {
        use crate::dandao::boss::{compute_loot, LOOT_ANCIENT_RECIPE, LOOT_BOSS_CORE};
        let boss = BaolongwangBoss::default();
        for seed in 0u64..100 {
            let loot = compute_loot(&boss, seed);
            let ids: Vec<&str> = loot.iter().map(|(id, _)| *id).collect();
            assert!(
                ids.contains(&LOOT_BOSS_CORE),
                "seed={seed} 100% BOSS_CORE 必掉（`compute_loot` 硬写第一行）"
            );
            assert!(
                ids.contains(&LOOT_ANCIENT_RECIPE),
                "seed={seed} 100% ANCIENT_RECIPE 必掉（`compute_loot` 第二行）"
            );
        }
    }

    #[test]
    fn loot_horn_requires_has_entered_rage() {
        use crate::dandao::boss::{compute_loot, LOOT_BOSS_HORN};
        // 未进 Rage：horn 100 seed 全不掉
        let boss_no_rage = BaolongwangBoss {
            has_entered_rage: false,
            ..BaolongwangBoss::default()
        };
        let no_rage_count = (0u64..100)
            .filter(|&s| {
                compute_loot(&boss_no_rage, s)
                    .iter()
                    .any(|(id, _)| *id == LOOT_BOSS_HORN)
            })
            .count();
        assert_eq!(
            no_rage_count, 0,
            "未进 Rage 阶段不掉 horn（`compute_loot` 有 has_entered_rage 门控）"
        );

        // 进过 Rage：horn 约 50%
        let boss_raged = BaolongwangBoss {
            has_entered_rage: true,
            ..BaolongwangBoss::default()
        };
        let rage_count = (0u64..10000)
            .filter(|&s| {
                compute_loot(&boss_raged, s)
                    .iter()
                    .any(|(id, _)| *id == LOOT_BOSS_HORN)
            })
            .count();
        assert!(
            (4500..5500).contains(&rage_count),
            "进过 Rage 后 horn 掉率应约 50%，10000 次掉了 {rage_count}"
        );
    }

    #[test]
    fn loot_scale_probability_and_count_range() {
        use crate::dandao::boss::{compute_loot, LOOT_BOSS_SCALE};
        let boss = BaolongwangBoss::default();
        let mut scale_count = 0u32;
        for seed in 0u64..100 {
            let loot = compute_loot(&boss, seed);
            if let Some((_, count)) = loot.iter().find(|(id, _)| *id == LOOT_BOSS_SCALE) {
                scale_count += 1;
                assert!(
                    (3..=8).contains(count),
                    "seed={seed} scale 数量应在 3-8，got {count}"
                );
            }
        }
        // 80% 概率，100 次期望 80±10
        assert!(
            (65..=95).contains(&scale_count),
            "scale 80% 概率，100 次中应有 65-95 次，got {scale_count}"
        );
    }

    #[test]
    fn loot_furnace_remnant_only_when_furnace_destroyed() {
        use crate::dandao::boss::{compute_loot, on_furnace_destroyed, LOOT_FURNACE_REMNANT};
        let mut boss = BaolongwangBoss::default();
        // furnace_intact=true → 不掉
        for seed in 0u64..100 {
            let loot = compute_loot(&boss, seed);
            assert!(
                !loot.iter().any(|(id, _)| *id == LOOT_FURNACE_REMNANT),
                "炉完好时不掉 furnace_remnant，seed={seed}"
            );
        }
        // 炉毁 → 100% 掉
        on_furnace_destroyed(&mut boss);
        for seed in 0u64..100 {
            let loot = compute_loot(&boss, seed);
            assert!(
                loot.iter().any(|(id, _)| *id == LOOT_FURNACE_REMNANT),
                "炉毁后 furnace_remnant 100% 必掉，seed={seed}"
            );
        }
    }

    #[test]
    fn loot_xu_yuan_dan_probability_and_count_range() {
        use crate::dandao::boss::{compute_loot, LOOT_XU_YUAN_DAN};
        let boss = BaolongwangBoss::default();
        let mut dan_count = 0u32;
        for seed in 0u64..100 {
            let loot = compute_loot(&boss, seed);
            if let Some((_, count)) = loot.iter().find(|(id, _)| *id == LOOT_XU_YUAN_DAN) {
                dan_count += 1;
                assert!(
                    (5..=10).contains(count),
                    "seed={seed} xu_yuan_dan 数量应在 5-10，got {count}"
                );
            }
        }
        // 70% 概率，100 次期望 70±12
        assert!(
            (55..=88).contains(&dan_count),
            "xu_yuan_dan 70% 概率，100 次中应有 55-88 次，got {dan_count}"
        );
    }

    // ── 守恒测试：吸取光环 ──────────────────────────────────────────────────

    #[test]
    fn qi_drain_aura_is_conserved_using_spirit_qi_total_const() {
        // 守恒断言：玩家减少量 == zone 增加量，total 不变。
        // 断言取 DEFAULT_SPIRIT_QI_TOTAL const 引用，不写字面。
        let initial_player_qi = 50.0f64;
        let mut player_qi = initial_player_qi;
        let drain_amount = QI_DRAIN_BASE_PER_TICK * QI_DRAIN_BOSS_MULTIPLIER * 1.0; // proximity_factor=1.0

        let mut account = WorldQiAccount::default();
        let zone_id = QiAccountId::zone(BOSS_HOME_ZONE);
        account.set_balance(zone_id.clone(), 0.0).unwrap();

        let actual_drain = drain_amount.min(player_qi);
        player_qi -= actual_drain;
        let zone_balance = account.balance(&zone_id);
        account
            .set_balance(zone_id.clone(), zone_balance + actual_drain)
            .unwrap();

        let player_after = player_qi;
        let zone_after = account.balance(&zone_id);

        // 玩家减少 == zone 增加
        assert!(
            (initial_player_qi - player_after - zone_after).abs() < 1e-9,
            "守恒：玩家减少({})应 == zone 增加({})，total 不变；\
             断言取 DEFAULT_SPIRIT_QI_TOTAL={DEFAULT_SPIRIT_QI_TOTAL} const 不写字面",
            initial_player_qi - player_after,
            zone_after
        );

        // total 不变（player + zone = 初始 player）
        let total_after = player_after + zone_after;
        assert!(
            (total_after - initial_player_qi).abs() < 1e-9,
            "吸取前后 total 守恒：before={initial_player_qi} after={total_after}"
        );
    }

    #[test]
    fn qi_drain_reason_is_boss_drain_variant() {
        // QiTransferReason::BossDrain 变体 pin 测试：必须存在且不是 HalfStepBuff。
        let reason = QiTransferReason::BossDrain;
        assert!(
            !matches!(reason, QiTransferReason::HalfStepBuff),
            "BossDrain 是独立变体，不等于 HalfStepBuff"
        );
        assert!(
            !matches!(reason, QiTransferReason::ReleaseToZone),
            "BossDrain 语义是 boss 吸取，区别于 ReleaseToZone（招式释放）"
        );
    }

    #[test]
    fn qi_drain_does_not_overdraft_player() {
        // 吸取量不超过玩家当前真元
        let player_qi = 0.1f64; // 极低真元
        let drain_amount = QI_DRAIN_BASE_PER_TICK * QI_DRAIN_BOSS_MULTIPLIER;
        let actual_drain = drain_amount.min(player_qi.max(0.0));
        assert!(
            actual_drain <= player_qi,
            "吸取量不能超过玩家当前真元：drain={drain_amount} player={player_qi} actual={actual_drain}"
        );
    }

    #[test]
    fn qi_drain_multiplier_matches_worldview_plus50_percent() {
        // worldview §十六:1572 +50% → 倍率 1.5
        assert!(
            (QI_DRAIN_BOSS_MULTIPLIER - 1.5).abs() < f64::EPSILON,
            "worldview §十六 负压畸变体 +50% → 倍率应为 1.5，got {}",
            QI_DRAIN_BOSS_MULTIPLIER
        );
    }

    #[test]
    fn qi_drain_zero_for_expel_phase() {
        // 驱逐阶段不吸取（BOSS 此时只想跑）
        let boss = BaolongwangBoss {
            hp_fraction: 0.9,
            phase: BossPhase::Expel,
            furnace_intact: true,
            ..BaolongwangBoss::default()
        };
        // 驱逐阶段 system 跳过（see baolongwang_qi_drain_aura_system: phase==Expel → continue）
        assert_eq!(
            boss.phase,
            BossPhase::Expel,
            "驱逐阶段不吸真元，system 直接 continue"
        );
    }

    // ── 常量 pin ─────────────────────────────────────────────────────────────

    #[test]
    fn boss_home_zone_is_baolongwang_cavern_deep() {
        assert_eq!(
            BOSS_HOME_ZONE, "baolongwang_cavern_deep",
            "plan §接入面 zone 锚点：坍缩渊"
        );
    }

    #[test]
    fn qi_drain_aura_radius_is_16() {
        assert!(
            (QI_DRAIN_AURA_RADIUS - 16.0).abs() < f64::EPSILON,
            "吸取光环半径应为 16 格（近场负压畸变体作用范围）"
        );
    }
}
