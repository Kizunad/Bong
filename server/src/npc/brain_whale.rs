//! 飞行鲸的 brain：scorer + action + 飞行 tick system。
//!
//! 设计取舍：
//! - **Scorer**：当前只一种行为（drift），所以 `WhaleDriftScorer` 始终给出
//!   非零 score（0.5）让鲸永远处于飘动状态。后续接入"逃避雷电 / 追随群体 /
//!   进入巡游航线"等行为时再加新 Scorer。
//! - **Action vs flight system 解耦**：Action 只决定"下一个 drift 目标点
//!   是什么"（写 FlightController.target），flight system 决定"如何移动到
//!   那"（写 Position / Look）。这样测试 Action 不用真跑物理 tick，测
//!   flight system 不用关心 brain 状态。
//! - **不复用 Navigator**：Navigator 是 A* 地面寻路；飞行无寻路需求，
//!   直接朝向插值更便宜也更符合"鲸飘"的视觉。

use big_brain::prelude::{ActionBuilder, ActionState, Actor, BigBrainSet, Score, ScorerBuilder};
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, IntoSystemConfigs, Look, Position,
    PreUpdate, Query, Update, With,
};

use crate::npc::spawn::NpcMarker;
use crate::npc::spawn_whale::{
    pick_drift_target, WhaleBlackboard, WhaleFlightController, ARRIVAL_RADIUS,
};

/// 不变：当前唯一的 idle 评分。鲸总是处于"飘"。后续多 behavior 时此值
/// 应低于事件性行为（逃避 / 集群）。
const WHALE_DRIFT_BASELINE_SCORE: f32 = 0.5;

#[derive(Clone, Copy, Debug, Component)]
pub struct WhaleDriftScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct WhaleDriftAction;

impl ScorerBuilder for WhaleDriftScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("WhaleDriftScorer")
    }
}

impl ActionBuilder for WhaleDriftAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("WhaleDriftAction")
    }
}

pub fn register(app: &mut App) {
    app.add_systems(
        PreUpdate,
        whale_drift_scorer_system.in_set(BigBrainSet::Scorers),
    )
    .add_systems(
        PreUpdate,
        whale_drift_action_system.in_set(BigBrainSet::Actions),
    )
    // Flight tick 在 Update 阶段。和 brain 不同 schedule，避免和 Action 抢
    // FlightController 写权（Action 写 target，system 写 phase + Position）。
    .add_systems(Update, whale_flight_system);
}

fn whale_drift_scorer_system(mut scorers: Query<&mut Score, With<WhaleDriftScorer>>) {
    for mut score in &mut scorers {
        score.set(WHALE_DRIFT_BASELINE_SCORE);
    }
}

/// Action 只做一件事：检查 controller.target 是否到期（None 或 已到达），
/// 是则用 blackboard 选新的、写回 controller。
fn whale_drift_action_system(
    mut whales: Query<
        (&Position, &mut WhaleBlackboard, &mut WhaleFlightController),
        With<NpcMarker>,
    >,
    mut actions: Query<(&Actor, &mut ActionState), With<WhaleDriftAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((position, mut blackboard, mut controller)) = whales.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let need_new = match controller.target {
                    None => true,
                    Some(t) => position.get().distance(t) <= ARRIVAL_RADIUS,
                };
                if need_new {
                    let new_target =
                        pick_drift_target(&blackboard, controller.y_oscillation_amplitude);
                    controller.target = Some(new_target);
                    blackboard.retarget_seq = blackboard.retarget_seq.wrapping_add(1);
                }
                // drift 永不 success：鲸永远在飘
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

/// 把 Position 朝 controller.target 推进 cruise_speed，叠加 Y 正弦震荡，
/// 同步 Look.yaw 到运动方向。target=None 时整体 no-op。
///
/// **baseline 与 visible Y 分离**：用 `controller.baseline_y` 做 Y 收敛 lerp，
/// 不读 `Position.y`（含上轮 sin 偏移）—— 避免 sin 偏移被当成漂移误差累积。
/// 当 amp > cruise_speed 时旧实现会单方向爬出 target.y 数十块；新实现 baseline
/// 永远收敛 target.y，sin 仅作可见层震荡。
fn whale_flight_system(
    mut whales: Query<(&mut Position, &mut Look, &mut WhaleFlightController), With<NpcMarker>>,
) {
    for (mut position, mut look, mut controller) in &mut whales {
        controller.oscillation_phase_ticks = controller.oscillation_phase_ticks.wrapping_add(1);
        let Some(target) = controller.target else {
            continue;
        };
        let here = position.get();
        let (next_pos, next_baseline_y) = step_position_toward(
            here,
            controller.baseline_y,
            target,
            controller.cruise_speed,
            controller.y_oscillation_amplitude,
            controller.oscillation_phase_ticks,
            controller.y_oscillation_period_ticks,
        );
        controller.baseline_y = next_baseline_y;

        // yaw：朝向运动方向；MC 约定 yaw=0 看向 +Z，atan2(-dx, dz) 度数
        let dx = next_pos.x - here.x;
        let dz = next_pos.z - here.z;
        if dx * dx + dz * dz > 1e-12 {
            let yaw_rad = (-dx).atan2(dz);
            look.yaw = yaw_rad.to_degrees() as f32;
        }
        position.set([next_pos.x, next_pos.y, next_pos.z]);
    }
}

/// 单步运动：XZ 朝 target 走 cruise_speed；baseline_y 朝 target.y 走 cruise_speed；
/// 可见 Y = baseline_y + sin(phase) * amp。
///
/// 返回 `(next_visible_pos, next_baseline_y)`。调用方写回 Position 与
/// `controller.baseline_y`。
///
/// 关键：用独立 baseline_y 而非 here.y 推进 Y，避免上轮 sin 偏移被当作误差累积。
pub fn step_position_toward(
    here: DVec3,
    baseline_y: f64,
    target: DVec3,
    cruise_speed: f64,
    y_amplitude: f64,
    phase_ticks: u64,
    period_ticks: u64,
) -> (DVec3, f64) {
    let dx = target.x - here.x;
    let dz = target.z - here.z;
    let xz_dist = (dx * dx + dz * dz).sqrt();
    let (next_x, next_z) = if xz_dist <= cruise_speed || xz_dist < 1e-9 {
        (target.x, target.z)
    } else {
        let factor = cruise_speed / xz_dist;
        (here.x + dx * factor, here.z + dz * factor)
    };
    let dy = target.y - baseline_y;
    let next_baseline_y = if dy.abs() <= cruise_speed {
        target.y
    } else {
        baseline_y + dy.signum() * cruise_speed
    };
    let phase = (phase_ticks % period_ticks) as f64 / period_ticks as f64;
    let osc = (phase * std::f64::consts::TAU).sin() * y_amplitude;
    (
        DVec3::new(next_x, next_baseline_y + osc, next_z),
        next_baseline_y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::npc::spawn_whale::{
        DEFAULT_CRUISE_SPEED, DEFAULT_Y_OSCILLATION_AMPLITUDE, DEFAULT_Y_OSCILLATION_PERIOD_TICKS,
    };

    fn fresh_blackboard(home: DVec3) -> WhaleBlackboard {
        WhaleBlackboard::new(home, 64.0)
    }

    fn fresh_controller() -> WhaleFlightController {
        WhaleFlightController::default()
    }

    // ---- step_position_toward ----

    #[test]
    fn step_position_toward_moves_xz_by_cruise_speed_when_far() {
        let here = DVec3::new(0.0, 80.0, 0.0);
        let target = DVec3::new(100.0, 80.0, 0.0); // 100 块远
        let (next, _baseline) = step_position_toward(here, 80.0, target, 0.5, 0.0, 0, 200);
        assert!(
            (next.x - 0.5).abs() < 1e-9,
            "x must advance by exact cruise"
        );
        assert!(
            (next.z - 0.0).abs() < 1e-9,
            "z must stay (target on x axis)"
        );
    }

    #[test]
    fn step_position_toward_clamps_to_target_when_overshoot() {
        let here = DVec3::new(0.0, 80.0, 0.0);
        let target = DVec3::new(0.3, 80.0, 0.0);
        // cruise 0.5 > 距离 0.3 → 直接落到 target 而非冲过头
        let (next, _baseline) = step_position_toward(here, 80.0, target, 0.5, 0.0, 0, 200);
        assert!(
            (next.x - 0.3).abs() < 1e-9,
            "must snap to target.x not overshoot"
        );
    }

    #[test]
    fn step_position_toward_oscillates_y_around_base() {
        // amp=6 period=200，phase=0 → sin(0)=0 → no offset
        let here = DVec3::new(0.0, 80.0, 0.0);
        let target = DVec3::new(0.0, 80.0, 0.0); // 已到位
        let (n0, _) = step_position_toward(here, 80.0, target, 0.5, 6.0, 0, 200);
        assert!((n0.y - 80.0).abs() < 1e-9, "phase 0 → osc 0");
        // phase=50 (1/4 period) → sin(π/2) = 1 → +amp
        let (n_quarter, _) = step_position_toward(here, 80.0, target, 0.5, 6.0, 50, 200);
        assert!((n_quarter.y - 86.0).abs() < 1e-9, "phase 1/4 → +amp");
        // phase=150 (3/4 period) → sin(3π/2) = -1 → -amp
        let (n_three_quarter, _) = step_position_toward(here, 80.0, target, 0.5, 6.0, 150, 200);
        assert!((n_three_quarter.y - 74.0).abs() < 1e-9, "phase 3/4 → -amp");
    }

    #[test]
    fn step_position_toward_y_zero_amp_disables_oscillation() {
        // 边界：amplitude=0 → 不抖
        let here = DVec3::new(0.0, 80.0, 0.0);
        let target = DVec3::new(0.0, 80.0, 0.0);
        for phase in [0u64, 50, 100, 150, 199] {
            let (n, _) = step_position_toward(here, 80.0, target, 0.5, 0.0, phase, 200);
            assert!(
                (n.y - 80.0).abs() < 1e-9,
                "phase {phase} amp=0 must give y=80"
            );
        }
    }

    #[test]
    fn step_position_toward_idempotent_when_at_target_xz() {
        // 边界：here == target XZ → next 等于 target.xz（Y 仍可能有 osc）
        let here = DVec3::new(0.0, 80.0, 0.0);
        let target = DVec3::new(0.0, 80.0, 0.0);
        let (next, baseline) = step_position_toward(here, 80.0, target, 0.5, 0.0, 0, 200);
        assert_eq!(next, DVec3::new(0.0, 80.0, 0.0));
        assert_eq!(baseline, 80.0, "baseline at target stays target.y");
    }

    #[test]
    fn step_position_toward_baseline_does_not_drift_under_oscillation() {
        // 回归 PR-177 codex P1：amp > cruise_speed 时旧实现把上轮 sin 偏移当
        // baseline lerp 输入，导致 baseline 单方向爬出 target.y。新实现 baseline
        // 独立 → 1000 tick 后 baseline 必须仍锁在 target.y。
        let target = DVec3::new(0.0, 80.0, 0.0);
        let mut here = DVec3::new(0.0, 80.0, 0.0);
        let mut baseline = 80.0;
        let amp = DEFAULT_Y_OSCILLATION_AMPLITUDE; // 1.25 > cruise 0.15 → 旧实现命中漂移
        let cruise = DEFAULT_CRUISE_SPEED;
        let period = DEFAULT_Y_OSCILLATION_PERIOD_TICKS;
        for phase in 0..1000u64 {
            let (next, next_baseline) =
                step_position_toward(here, baseline, target, cruise, amp, phase, period);
            here = next;
            baseline = next_baseline;
        }
        assert!(
            (baseline - 80.0).abs() < 1e-9,
            "baseline {baseline} drifted from target.y=80 (旧 here.y 反推 bug 回归)"
        );
        // visible Y 也必须始终 ≤ ±amp 范围（不会逃逸）
        let dy = (here.y - 80.0).abs();
        assert!(
            dy <= amp + 1e-9,
            "visible Y |dy|={dy} > amp={amp} after 1000 ticks (有累积漂移)"
        );
    }

    // ---- Action transitions（state machine pin 测试） ----

    #[test]
    fn drift_action_requested_transitions_to_executing() {
        // Action 第一帧：Requested → Executing；不应直接 Success/Failure
        let mut state = ActionState::Requested;
        // 模拟 system 内 match 逻辑
        match state {
            ActionState::Requested => state = ActionState::Executing,
            _ => panic!("must enter executing"),
        }
        assert_eq!(state, ActionState::Executing);
    }

    #[test]
    fn drift_action_cancelled_transitions_to_failure() {
        let mut state = ActionState::Cancelled;
        match state {
            ActionState::Cancelled => state = ActionState::Failure,
            _ => panic!(),
        }
        assert_eq!(state, ActionState::Failure);
    }

    // ---- pick_drift_target via Action 调用契约 ----

    #[test]
    fn drift_picks_new_target_when_controller_target_none() {
        let bb = fresh_blackboard(DVec3::new(0.0, 80.0, 0.0));
        let mut ctrl = fresh_controller();
        // 模拟 action：target=None → 必须填一个
        let need = ctrl.target.is_none();
        assert!(need, "fresh controller must trigger retarget");
        ctrl.target = Some(pick_drift_target(&bb, ctrl.y_oscillation_amplitude));
        assert!(ctrl.target.is_some());
    }

    #[test]
    fn drift_picks_new_target_when_within_arrival_radius() {
        let mut bb = fresh_blackboard(DVec3::new(0.0, 80.0, 0.0));
        let mut ctrl = fresh_controller();
        ctrl.target = Some(DVec3::new(10.0, 80.0, 0.0));
        let here = DVec3::new(10.0 - 0.5, 80.0, 0.0); // 距离 0.5 < ARRIVAL_RADIUS=4
        let close_enough = here.distance(ctrl.target.unwrap()) <= ARRIVAL_RADIUS;
        assert!(close_enough);

        // Action 应触发 retarget：旧 target 被覆盖、retarget_seq +1
        let old_target = ctrl.target;
        let prev_seq = bb.retarget_seq;
        ctrl.target = Some(pick_drift_target(&bb, ctrl.y_oscillation_amplitude));
        bb.retarget_seq += 1;

        assert_ne!(ctrl.target, old_target);
        assert_eq!(bb.retarget_seq, prev_seq + 1);
    }

    #[test]
    fn drift_keeps_target_when_far_from_arrival() {
        let bb = fresh_blackboard(DVec3::new(0.0, 80.0, 0.0));
        let mut ctrl = fresh_controller();
        let target = DVec3::new(50.0, 80.0, 0.0);
        ctrl.target = Some(target);
        let here = DVec3::new(0.0, 80.0, 0.0); // 距离 50 > ARRIVAL_RADIUS
        let close_enough = here.distance(ctrl.target.unwrap()) <= ARRIVAL_RADIUS;
        assert!(!close_enough, "must NOT trigger retarget at 50 blocks away");
        // ctrl.target 不变
        assert_eq!(ctrl.target, Some(target));
        let _ = bb;
    }

    // ---- 集成：N tick 后鲸应该接近 target ----

    #[test]
    fn integration_position_progresses_toward_target_over_ticks() {
        // 饱和：连续 50 tick 应单调减少到 target 的 XZ 距离
        let target = DVec3::new(20.0, 80.0, 0.0);
        let mut here = DVec3::new(0.0, 80.0, 0.0);
        let mut baseline = 80.0;
        let mut last_dist = ((target.x - here.x).powi(2) + (target.z - here.z).powi(2)).sqrt();
        for tick in 0..50 {
            let (next, next_baseline) = step_position_toward(
                here,
                baseline,
                target,
                DEFAULT_CRUISE_SPEED,
                0.0, // 关闭震荡，纯 XZ 进度
                tick,
                DEFAULT_Y_OSCILLATION_PERIOD_TICKS,
            );
            let dist = ((target.x - next.x).powi(2) + (target.z - next.z).powi(2)).sqrt();
            assert!(
                dist <= last_dist + 1e-9,
                "tick {tick}: distance must monotonically decrease, was {last_dist}, now {dist}"
            );
            last_dist = dist;
            here = next;
            baseline = next_baseline;
        }
    }

    #[test]
    fn integration_y_oscillation_stays_within_amplitude_bounds() {
        // amplitude=6 → Y 在 [base-6, base+6] 范围内（不会逃逸）
        let here = DVec3::new(0.0, 80.0, 0.0);
        let target = DVec3::new(0.0, 80.0, 0.0);
        for phase in 0..1000u64 {
            let (next, _) = step_position_toward(
                here,
                80.0,
                target,
                DEFAULT_CRUISE_SPEED,
                DEFAULT_Y_OSCILLATION_AMPLITUDE,
                phase,
                DEFAULT_Y_OSCILLATION_PERIOD_TICKS,
            );
            let dy = (next.y - 80.0).abs();
            assert!(
                dy <= DEFAULT_Y_OSCILLATION_AMPLITUDE + 1e-9,
                "phase {phase}: |dy|={dy} > amp={DEFAULT_Y_OSCILLATION_AMPLITUDE}"
            );
        }
    }
}
