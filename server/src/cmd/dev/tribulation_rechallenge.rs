//! plan-halfstep-buff-v1 P3：`/tribulation_rechallenge` dev 命令。
//!
//! 玩家收到 `HalfStepRechallengeTriggerEvent` 后通过此命令显式申请重新起劫；
//! 命令体内校验执行者 entity 持有 `HalfStepState` 且 `current_tick <= rechallenge_window_until`，
//! 通过则发出 `StartDuXuRequest`（接入既有渡虚劫流程，不另外开新管线）。
//!
//! dev-only：未来交互式 UI（渡劫台、HUD 按钮）应复用同一校验逻辑；本命令是占位入口。

use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, EventWriter, Query, Res, Update};

use crate::combat::CombatClock;
use crate::cultivation::tribulation::{HalfStepState, StartDuXuRequest, TribulationState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TribulationRechallengeCmd {
    Trigger,
}

impl Command for TribulationRechallengeCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("tribulation_rechallenge")
            .with_executable(|_| TribulationRechallengeCmd::Trigger);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<TribulationRechallengeCmd>()
        .add_systems(Update, handle);
}

/// 校验结果（独立 fn 便于测试 + 未来 HUD 复用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RechallengeGate {
    Ok,
    NoHalfStepState,
    WindowExpired {
        window_until: u64,
        current_tick: u64,
    },
    AlreadyTribulating,
}

pub fn check_rechallenge_gate(
    state: Option<&HalfStepState>,
    active: Option<&TribulationState>,
    current_tick: u64,
) -> RechallengeGate {
    if active.is_some() {
        return RechallengeGate::AlreadyTribulating;
    }
    let Some(state) = state else {
        return RechallengeGate::NoHalfStepState;
    };
    if current_tick > state.rechallenge_window_until {
        return RechallengeGate::WindowExpired {
            window_until: state.rechallenge_window_until,
            current_tick,
        };
    }
    RechallengeGate::Ok
}

pub fn handle(
    mut events: EventReader<CommandResultEvent<TribulationRechallengeCmd>>,
    clock: Option<Res<CombatClock>>,
    halfstep_q: Query<&HalfStepState>,
    active_q: Query<&TribulationState>,
    mut clients: Query<&mut Client>,
    mut start_requests: EventWriter<StartDuXuRequest>,
) {
    for event in events.read() {
        let Some(clock) = clock.as_deref() else {
            // CombatClock 缺失 → 整体 no-op；不能凭空猜 tick
            continue;
        };
        let state = halfstep_q.get(event.executor).ok();
        let active = active_q.get(event.executor).ok();
        let gate = check_rechallenge_gate(state, active, clock.tick);

        let feedback = match gate {
            RechallengeGate::Ok => {
                // side effect 在 Client 反馈之前发出：测试 / 自动化 / dormant scripted path
                // 没有 Client 时也要把 StartDuXuRequest 发出去
                start_requests.send(StartDuXuRequest {
                    entity: event.executor,
                    requested_at_tick: clock.tick,
                });
                "tribulation_rechallenge: StartDuXuRequest 已发出".to_string()
            }
            RechallengeGate::NoHalfStepState => {
                "tribulation_rechallenge: 未处于半步化虚状态".to_string()
            }
            RechallengeGate::WindowExpired {
                window_until,
                current_tick,
            } => format!(
                "tribulation_rechallenge: 重渡窗口已过 (window_until={window_until}, current_tick={current_tick})"
            ),
            RechallengeGate::AlreadyTribulating => {
                "tribulation_rechallenge: 已在渡劫中，无法重渡".to_string()
            }
        };

        if let Ok(mut client) = clients.get_mut(event.executor) {
            client.send_chat_message(feedback);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::tribulation::{
        TribulationKind, TribulationPhase, RECHALLENGE_WINDOW_TICKS,
    };
    use valence::prelude::Events;

    fn make_halfstep_state(entered_at: u64, buff_applied: bool) -> HalfStepState {
        HalfStepState {
            entered_at,
            rechallenge_window_until: entered_at + RECHALLENGE_WINDOW_TICKS,
            buff_applied,
        }
    }

    fn make_tribulation_state() -> TribulationState {
        TribulationState {
            kind: TribulationKind::DuXu,
            phase: TribulationPhase::Omen,
            epicenter: [0.0, 64.0, 0.0],
            wave_current: 0,
            waves_total: 3,
            started_tick: 0,
            phase_started_tick: 0,
            next_wave_tick: 0,
            participants: vec!["test".to_string()],
            failed: false,
        }
    }

    #[test]
    fn gate_passes_when_within_window_and_not_already_tribulating() {
        let state = make_halfstep_state(100, true);
        assert_eq!(
            check_rechallenge_gate(Some(&state), None, 200),
            RechallengeGate::Ok
        );
    }

    #[test]
    fn gate_rejects_without_halfstep_state() {
        assert_eq!(
            check_rechallenge_gate(None, None, 100),
            RechallengeGate::NoHalfStepState
        );
    }

    #[test]
    fn gate_rejects_when_window_expired() {
        let state = make_halfstep_state(100, true);
        let outcome =
            check_rechallenge_gate(Some(&state), None, 100 + RECHALLENGE_WINDOW_TICKS + 1);
        assert!(
            matches!(
                outcome,
                RechallengeGate::WindowExpired {
                    window_until: _,
                    current_tick: _
                }
            ),
            "过窗 1 tick 必须 WindowExpired; got {outcome:?}"
        );
    }

    #[test]
    fn gate_boundary_window_end_is_still_ok() {
        let state = make_halfstep_state(100, true);
        let window_end = 100 + RECHALLENGE_WINDOW_TICKS;
        assert_eq!(
            check_rechallenge_gate(Some(&state), None, window_end),
            RechallengeGate::Ok,
            "current_tick == window_until 应仍在窗口内（闭区间，与 HalfStepState.is_within_window 一致）"
        );
    }

    #[test]
    fn gate_rejects_when_already_tribulating() {
        let state = make_halfstep_state(100, true);
        let active = make_tribulation_state();
        assert_eq!(
            check_rechallenge_gate(Some(&state), Some(&active), 200),
            RechallengeGate::AlreadyTribulating,
            "活跃 TribulationState 应阻止重渡；防止双重渡劫"
        );
    }

    #[test]
    fn handle_emits_start_duxu_request_on_pass() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 500 });
        app.add_event::<CommandResultEvent<TribulationRechallengeCmd>>();
        app.add_event::<StartDuXuRequest>();
        app.add_systems(Update, handle);
        let entity = app.world_mut().spawn(make_halfstep_state(100, true)).id();
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<TribulationRechallengeCmd>>>()
            .send(CommandResultEvent {
                result: TribulationRechallengeCmd::Trigger,
                executor: entity,
                modifiers: Default::default(),
            });
        app.update();
        let requests: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<StartDuXuRequest>>()
            .drain()
            .collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].entity, entity);
        assert_eq!(requests[0].requested_at_tick, 500);
    }

    #[test]
    fn handle_does_not_emit_when_gate_fails() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 500 });
        app.add_event::<CommandResultEvent<TribulationRechallengeCmd>>();
        app.add_event::<StartDuXuRequest>();
        app.add_systems(Update, handle);
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<TribulationRechallengeCmd>>>()
            .send(CommandResultEvent {
                result: TribulationRechallengeCmd::Trigger,
                executor: entity,
                modifiers: Default::default(),
            });
        app.update();
        let requests: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<StartDuXuRequest>>()
            .drain()
            .collect();
        assert!(
            requests.is_empty(),
            "无 HalfStepState 应 gate 拦截，绝不能 emit StartDuXuRequest"
        );
    }
}
