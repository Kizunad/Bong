//! plan-craft-v1 P3 — server craft event → Redis 桥。
//!
//! 把 craft 子系统产生的 Bevy event 转成 schema payload 推到 Redis：
//!   * `CraftCompletedEvent` / `CraftFailedEvent` → `bong:craft/outcome`
//!   * `RecipeUnlockedEvent`                     → `bong:craft/recipe_unlocked`
//!
//! `CraftStartedEvent` 当前不出 Redis（agent 没消费 trigger）。后续如果
//! agent 想做 "起手" narration，再增 channel + 桥即可。

use valence::prelude::{EventReader, Query, Res, Username};

use super::cast_emit::current_unix_millis;
use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::craft::{
    CraftCompletedEvent, CraftFailedEvent, CraftFailureReason, RecipeUnlockedEvent,
    UnlockEventSource,
};
use crate::player::state::canonical_player_id;
use crate::schema::craft::{
    CraftFailureReasonV1, CraftOutcomeV1, InsightTriggerV1, RecipeUnlockedV1, UnlockEventSourceV1,
};

fn caster_player_id(caster: valence::prelude::Entity, names: &Query<&Username>) -> String {
    names
        .get(caster)
        .map(|u| canonical_player_id(u.0.as_str()))
        .unwrap_or_else(|_| format!("entity:{}", caster.to_bits()))
}

fn map_failure_reason(reason: CraftFailureReason) -> CraftFailureReasonV1 {
    reason.into()
}

fn map_unlock_source(source: &UnlockEventSource) -> UnlockEventSourceV1 {
    match source {
        UnlockEventSource::Scroll { item_template } => UnlockEventSourceV1::Scroll {
            item_template: item_template.clone(),
        },
        UnlockEventSource::Mentor { npc_archetype } => UnlockEventSourceV1::Mentor {
            npc_archetype: npc_archetype.clone(),
        },
        UnlockEventSource::Insight { trigger } => UnlockEventSourceV1::Insight {
            trigger: InsightTriggerV1::from(*trigger),
        },
    }
}

pub fn publish_craft_completed_to_redis(
    redis: Option<Res<RedisBridgeResource>>,
    mut completed: EventReader<CraftCompletedEvent>,
    names: Query<&Username>,
) {
    let Some(redis) = redis else {
        completed.clear();
        return;
    };
    for event in completed.read() {
        let player_id = caster_player_id(event.caster, &names);
        let payload = CraftOutcomeV1::Completed {
            v: 1,
            player_id,
            recipe_id: event.recipe_id.as_str().to_string(),
            output_template: event.output_template.clone(),
            output_count: event.output_count,
            completed_at_tick: event.completed_at_tick,
            ts: current_unix_millis(),
        };
        let _ = redis.tx_outbound.send(RedisOutbound::CraftOutcome(payload));
    }
}

pub fn publish_craft_failed_to_redis(
    redis: Option<Res<RedisBridgeResource>>,
    mut failed: EventReader<CraftFailedEvent>,
    names: Query<&Username>,
) {
    let Some(redis) = redis else {
        failed.clear();
        return;
    };
    for event in failed.read() {
        let player_id = caster_player_id(event.caster, &names);
        let payload = CraftOutcomeV1::Failed {
            v: 1,
            player_id,
            recipe_id: event.recipe_id.as_str().to_string(),
            reason: map_failure_reason(event.reason),
            material_returned: event.material_returned,
            qi_refunded: event.qi_refunded,
            ts: current_unix_millis(),
        };
        let _ = redis.tx_outbound.send(RedisOutbound::CraftOutcome(payload));
    }
}

pub fn publish_recipe_unlocked_to_redis(
    redis: Option<Res<RedisBridgeResource>>,
    mut events: EventReader<RecipeUnlockedEvent>,
    names: Query<&Username>,
) {
    let Some(redis) = redis else {
        events.clear();
        return;
    };
    for event in events.read() {
        let player_id = caster_player_id(event.caster, &names);
        let payload = RecipeUnlockedV1 {
            v: 1,
            player_id,
            recipe_id: event.recipe_id.as_str().to_string(),
            source: map_unlock_source(&event.source),
            unlocked_at_tick: event.unlocked_at_tick,
            ts: current_unix_millis(),
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::RecipeUnlocked(payload));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::craft::{CraftCompletedEvent, CraftFailureReason, InsightTrigger, RecipeId};
    use crossbeam_channel::unbounded;
    use valence::prelude::{App, Update};

    #[test]
    fn publish_craft_completed_queues_completed_payload() {
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();
        let mut app = App::new();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<CraftCompletedEvent>();
        app.add_systems(Update, publish_craft_completed_to_redis);

        let caster = app.world_mut().spawn(Username("Azure".to_string())).id();
        app.world_mut().send_event(CraftCompletedEvent {
            caster,
            recipe_id: RecipeId::new("craft.example.eclipse_needle.iron"),
            completed_at_tick: 5000,
            output_template: "eclipse_needle_iron".into(),
            output_count: 3,
        });
        app.update();
        let outbound = rx_outbound.try_recv().expect("must enqueue payload");
        match outbound {
            RedisOutbound::CraftOutcome(CraftOutcomeV1::Completed {
                player_id,
                recipe_id,
                output_template,
                output_count,
                ..
            }) => {
                assert_eq!(player_id, "offline:Azure");
                assert_eq!(recipe_id, "craft.example.eclipse_needle.iron");
                assert_eq!(output_template, "eclipse_needle_iron");
                assert_eq!(output_count, 3);
            }
            other => panic!("expected CraftOutcome::Completed, got {other:?}"),
        }
    }

    #[test]
    fn publish_craft_failed_queues_failed_payload_with_reason() {
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();
        let mut app = App::new();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<CraftFailedEvent>();
        app.add_systems(Update, publish_craft_failed_to_redis);

        let caster = app.world_mut().spawn(Username("Bob".to_string())).id();
        app.world_mut().send_event(CraftFailedEvent {
            caster,
            recipe_id: RecipeId::new("craft.example.fake_skin.light"),
            reason: CraftFailureReason::PlayerCancelled,
            material_returned: 2,
            qi_refunded: 0.0,
        });
        app.update();
        let outbound = rx_outbound.try_recv().expect("must enqueue payload");
        match outbound {
            RedisOutbound::CraftOutcome(CraftOutcomeV1::Failed {
                player_id,
                recipe_id,
                reason,
                material_returned,
                ..
            }) => {
                assert_eq!(player_id, "offline:Bob");
                assert_eq!(recipe_id, "craft.example.fake_skin.light");
                assert_eq!(reason, CraftFailureReasonV1::PlayerCancelled);
                assert_eq!(material_returned, 2);
            }
            other => panic!("expected CraftOutcome::Failed, got {other:?}"),
        }
    }

    #[test]
    fn publish_recipe_unlocked_serializes_each_source_variant() {
        for source in [
            UnlockEventSource::Scroll {
                item_template: "scroll_eclipse_needle_iron".into(),
            },
            UnlockEventSource::Mentor {
                npc_archetype: "poison_master".into(),
            },
            UnlockEventSource::Insight {
                trigger: InsightTrigger::NearDeath,
            },
        ] {
            let (tx_outbound, rx_outbound) = unbounded();
            let (_tx_inbound, rx_inbound) = unbounded();
            let mut app = App::new();
            app.insert_resource(RedisBridgeResource {
                tx_outbound,
                rx_inbound,
            });
            app.add_event::<RecipeUnlockedEvent>();
            app.add_systems(Update, publish_recipe_unlocked_to_redis);

            let caster = app.world_mut().spawn(Username("Charlie".to_string())).id();
            app.world_mut().send_event(RecipeUnlockedEvent {
                caster,
                recipe_id: RecipeId::new("craft.example.fake_skin.light"),
                source: source.clone(),
                unlocked_at_tick: 8000,
            });
            app.update();
            let outbound = rx_outbound.try_recv().expect("must enqueue payload");
            match outbound {
                RedisOutbound::RecipeUnlocked(payload) => {
                    assert_eq!(payload.player_id, "offline:Charlie");
                    assert_eq!(payload.recipe_id, "craft.example.fake_skin.light");
                    assert_eq!(payload.unlocked_at_tick, 8000);
                    // 验证 source variant 1:1 镜像
                    match (&source, &payload.source) {
                        (
                            UnlockEventSource::Scroll { item_template: a },
                            UnlockEventSourceV1::Scroll { item_template: b },
                        ) => assert_eq!(a, b),
                        (
                            UnlockEventSource::Mentor { npc_archetype: a },
                            UnlockEventSourceV1::Mentor { npc_archetype: b },
                        ) => assert_eq!(a, b),
                        (
                            UnlockEventSource::Insight { trigger: a },
                            UnlockEventSourceV1::Insight { trigger: b },
                        ) => assert_eq!(InsightTriggerV1::from(*a), *b),
                        (a, b) => panic!("source mismatch: {a:?} → {b:?}"),
                    }
                }
                other => panic!("expected RecipeUnlocked, got {other:?}"),
            }
        }
    }

    #[test]
    fn no_redis_resource_means_silent_drop_no_panic() {
        let mut app = App::new();
        app.add_event::<CraftCompletedEvent>();
        app.add_systems(Update, publish_craft_completed_to_redis);

        let caster = app.world_mut().spawn(Username("X".to_string())).id();
        app.world_mut().send_event(CraftCompletedEvent {
            caster,
            recipe_id: RecipeId::new("x"),
            completed_at_tick: 1,
            output_template: "y".into(),
            output_count: 1,
        });
        app.update(); // 必须不 panic
    }
}
