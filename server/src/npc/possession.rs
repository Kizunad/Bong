//! 夺舍（plan §3.4 / plan-death §4e）NPC 入口。
//!
//! 本模块把 NPC 层选中的实体目标转成 cultivation 侧权威请求；资格校验、
//! 肉身接管、两卷交叉引用与坐标继承由 `cultivation::possession` 统一结算。

use valence::prelude::{
    bevy_ecs, App, Entity, Event, EventReader, EventWriter, IntoSystemConfigs, Update,
};

use crate::cultivation::possession::DuoSheRequestEvent;
use crate::npc::brain::canonical_npc_id;

/// 玩家请求夺舍某 NPC。
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
        .add_systems(Update, forward_duoshe_intent.in_set(DuoSheIntentForwardSet));
}

#[derive(bevy_ecs::schedule::SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DuoSheIntentForwardSet;

fn forward_duoshe_intent(
    mut events: EventReader<DuoSheIntent>,
    mut requests: EventWriter<DuoSheRequestEvent>,
) {
    for event in events.read() {
        tracing::info!(
            "[bong][npc] forwarding duo_she intent reason={} target={}",
            event.reason.as_str(),
            canonical_npc_id(event.npc_target)
        );
        requests.send(DuoSheRequestEvent {
            host: event.player_entity,
            target_id: canonical_npc_id(event.npc_target),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::App;

    #[test]
    fn duoshe_intent_event_forwards_runtime_request() {
        let mut app = App::new();
        app.add_event::<DuoSheRequestEvent>();
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
        assert!(
            events.len() <= 1,
            "event should be drained by forwarding system"
        );

        let requests = app
            .world()
            .resource::<bevy_ecs::event::Events<DuoSheRequestEvent>>();
        assert_eq!(requests.len(), 1);
    }

    #[test]
    fn duoshe_reason_strings_are_stable() {
        assert_eq!(DuoSheReason::Combat.as_str(), "combat");
        assert_eq!(DuoSheReason::Stealth.as_str(), "stealth");
        assert_eq!(DuoSheReason::AgentDirected.as_str(), "agent_directed");
    }
}
