//! 通用动画触发组件 -> `bong:vfx_event` 适配层。
//!
//! 业务系统若已经有明确事件，优先用 `vfx_animation_trigger` 里的专门 adapter；本组件给后续
//! plan 一个低耦合入口：在玩家实体上插入 `AnimationTrigger`，本系统会在同 tick 转成
//! `play_anim` / `stop_anim` payload 并清掉组件。

use valence::prelude::{bevy_ecs, Commands, Component, DVec3, Entity, Position, Query, UniqueId};

use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::vfx_event::VfxEventPayloadV1;

#[derive(Debug, Clone, PartialEq, Eq, Component)]
pub struct AnimationTrigger {
    pub anim_id: String,
    pub action: AnimationTriggerAction,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimationTriggerAction {
    Play {
        priority: u16,
        fade_in_ticks: Option<u8>,
    },
    Stop {
        fade_out_ticks: Option<u8>,
    },
}

#[allow(dead_code)]
impl AnimationTrigger {
    pub fn play_with_fade(anim_id: impl Into<String>, priority: u16, fade_in_ticks: u8) -> Self {
        Self {
            anim_id: anim_id.into(),
            action: AnimationTriggerAction::Play {
                priority,
                fade_in_ticks: Some(fade_in_ticks),
            },
        }
    }

    pub fn stop(anim_id: impl Into<String>) -> Self {
        Self {
            anim_id: anim_id.into(),
            action: AnimationTriggerAction::Stop {
                fade_out_ticks: None,
            },
        }
    }
}

pub fn emit_animation_trigger_components(
    mut commands: Commands,
    triggers: Query<(
        Entity,
        &AnimationTrigger,
        Option<&Position>,
        Option<&UniqueId>,
    )>,
    mut vfx_events: valence::prelude::EventWriter<VfxEventRequest>,
) {
    for (entity, trigger, position, unique_id) in &triggers {
        if let Some(unique_id) = unique_id {
            let origin = position
                .map(|pos| pos.get())
                .unwrap_or(DVec3::new(0.0, 0.0, 0.0));
            let target_player = unique_id.0.to_string();
            let payload = match &trigger.action {
                AnimationTriggerAction::Play {
                    priority,
                    fade_in_ticks,
                } => VfxEventPayloadV1::PlayAnim {
                    target_player,
                    anim_id: trigger.anim_id.clone(),
                    priority: *priority,
                    fade_in_ticks: *fade_in_ticks,
                },
                AnimationTriggerAction::Stop { fade_out_ticks } => VfxEventPayloadV1::StopAnim {
                    target_player,
                    anim_id: trigger.anim_id.clone(),
                    fade_out_ticks: *fade_out_ticks,
                },
            };
            vfx_events.send(VfxEventRequest::new(origin, payload));
        }
        commands.entity(entity).remove::<AnimationTrigger>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Events, Update};
    use valence::testing::create_mock_client;

    fn spawn_player(app: &mut App, name: &str) -> Entity {
        let (mut bundle, _helper) = create_mock_client(name);
        bundle.player.position = Position::new([1.0, 64.0, 2.0]);
        app.world_mut().spawn(bundle).id()
    }

    fn drain_vfx(app: &mut App) -> Vec<VfxEventRequest> {
        app.world_mut()
            .resource_mut::<Events<VfxEventRequest>>()
            .drain()
            .collect()
    }

    #[test]
    fn play_component_emits_play_anim_payload_and_clears_component() {
        let mut app = App::new();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_animation_trigger_components);
        let player = spawn_player(&mut app, "Alice");
        app.world_mut()
            .entity_mut(player)
            .insert(AnimationTrigger::play_with_fade(
                "bong:palm_strike",
                1000,
                3,
            ));

        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 1);
        match &emitted[0].payload {
            VfxEventPayloadV1::PlayAnim {
                anim_id,
                priority,
                fade_in_ticks,
                ..
            } => {
                assert_eq!(anim_id, "bong:palm_strike");
                assert_eq!(*priority, 1000);
                assert_eq!(*fade_in_ticks, Some(3));
            }
            other => panic!("expected PlayAnim, got {other:?}"),
        }
        assert!(app.world().get::<AnimationTrigger>(player).is_none());
    }

    #[test]
    fn stop_component_emits_stop_anim_payload() {
        let mut app = App::new();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_animation_trigger_components);
        let player = spawn_player(&mut app, "Alice");
        app.world_mut()
            .entity_mut(player)
            .insert(AnimationTrigger::stop("bong:meditate_sit"));

        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 1);
        match &emitted[0].payload {
            VfxEventPayloadV1::StopAnim { anim_id, .. } => {
                assert_eq!(anim_id, "bong:meditate_sit");
            }
            other => panic!("expected StopAnim, got {other:?}"),
        }
    }
}
