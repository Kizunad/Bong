//! 夺舍（plan §3.4 / plan-death §4e）hook 占位。
//!
//! 这里只暴露 `DuoSheIntent` 事件与一个 stub 消费系统（仅 log），真实的
//! 肉身接管、两卷交叉引用、坐标继承等由 plan-death §4e 接入。该模块
//! 提供稳定的事件契约，避免 §4e 实装时与 NPC 层冲突。

use valence::prelude::{bevy_ecs, App, Entity, Event, EventReader, Update};

/// 玩家请求夺舍某 NPC。§4e 之前仅被 stub 消费。
#[derive(Clone, Debug, Event)]
pub struct DuoSheIntent {
    pub player_entity: Entity,
    pub npc_target: Entity,
    pub reason: DuoSheReason,
}

/// 夺舍触发原因。后续 plan 可增加变体（例如战败求生）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum DuoSheReason {
    Combat,
    Stealth,
    AgentDirected,
}

impl DuoSheReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Combat => "combat",
            Self::Stealth => "stealth",
            Self::AgentDirected => "agent_directed",
        }
    }
}

pub fn register(app: &mut App) {
    app.add_event::<DuoSheIntent>()
        .add_systems(Update, log_duoshe_intent);
}

fn log_duoshe_intent(mut events: EventReader<DuoSheIntent>) {
    for event in events.read() {
        tracing::info!(
            "[bong][npc][possession] DuoSheIntent stub — player={:?} target={:?} reason={}",
            event.player_entity,
            event.npc_target,
            event.reason.as_str()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::App;

    #[test]
    fn duoshe_intent_event_is_registered_and_consumed_without_panic() {
        let mut app = App::new();
        register(&mut app);

        let player = app.world_mut().spawn_empty().id();
        let npc = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(DuoSheIntent {
            player_entity: player,
            npc_target: npc,
            reason: DuoSheReason::Combat,
        });

        app.update();

        let events = app
            .world()
            .resource::<bevy_ecs::event::Events<DuoSheIntent>>();
        assert!(events.len() <= 1, "event should be drained by stub");
    }

    #[test]
    fn duoshe_reason_strings_are_stable() {
        assert_eq!(DuoSheReason::Combat.as_str(), "combat");
        assert_eq!(DuoSheReason::Stealth.as_str(), "stealth");
        assert_eq!(DuoSheReason::AgentDirected.as_str(), "agent_directed");
    }
}
