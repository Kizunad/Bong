//! plan-lingtian-weather-v1 §3 / §4.4 — 把 ECS `WeatherLifecycleEvent` 转译成
//! `RedisOutbound::WeatherEventUpdate`，单 zone MVP 用 `DEFAULT_ZONE` 标识。
//!
//! 同 `zone_pressure_bridge.rs` 的"读 Bevy event → 写 RedisOutbound"模式。

use valence::prelude::{EventReader, Res};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::lingtian::qi_account::DEFAULT_ZONE;
use crate::lingtian::weather::WeatherLifecycleEvent;
use crate::schema::lingtian_weather::{WeatherEventDataV1, WeatherEventUpdateV1};

pub fn publish_weather_lifecycle_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<WeatherLifecycleEvent>,
) {
    for ev in events.read() {
        let envelope = match *ev {
            WeatherLifecycleEvent::Started {
                event,
                started_at_lingtian_tick,
                expires_at_lingtian_tick,
            } => {
                let data = WeatherEventDataV1::new(
                    DEFAULT_ZONE,
                    event,
                    started_at_lingtian_tick,
                    expires_at_lingtian_tick,
                    started_at_lingtian_tick,
                );
                WeatherEventUpdateV1::started(data)
            }
            WeatherLifecycleEvent::Expired {
                event,
                expired_at_lingtian_tick,
            } => {
                // expired 时 remaining_ticks=0；started_at 用 expired_at（信息丢失，保留语义可读性）
                let data = WeatherEventDataV1::new(
                    DEFAULT_ZONE,
                    event,
                    expired_at_lingtian_tick,
                    expired_at_lingtian_tick,
                    expired_at_lingtian_tick,
                );
                WeatherEventUpdateV1::expired(data)
            }
        };
        if let Err(error) = redis
            .tx_outbound
            .send(RedisOutbound::WeatherEventUpdate(envelope))
        {
            tracing::warn!("[bong][weather_bridge] dropped WeatherEventUpdate: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lingtian::weather::WeatherEvent;
    use crate::network::redis_bridge::RedisOutbound;
    use crossbeam_channel::Receiver;
    use valence::prelude::{App, Update};

    fn build_app() -> (App, Receiver<RedisOutbound>) {
        let (tx, rx) = crossbeam_channel::unbounded::<RedisOutbound>();
        // rx_inbound 不在本测试范围内，给一个空 channel 占位即可。
        let (_dummy_inbound_tx, dummy_inbound_rx) =
            crossbeam_channel::unbounded::<crate::network::redis_bridge::RedisInbound>();
        let mut app = App::new();
        app.insert_resource(RedisBridgeResource {
            tx_outbound: tx,
            rx_inbound: dummy_inbound_rx,
        });
        app.add_event::<WeatherLifecycleEvent>();
        app.add_systems(Update, publish_weather_lifecycle_events);
        (app, rx)
    }

    #[test]
    fn started_event_publishes_redis_outbound_with_started_kind() {
        let (mut app, rx) = build_app();
        app.world_mut().send_event(WeatherLifecycleEvent::Started {
            event: WeatherEvent::Thunderstorm,
            started_at_lingtian_tick: 1440,
            expires_at_lingtian_tick: 1620,
        });
        app.update();

        let outbound = rx.try_recv().expect("应当 publish 一条 outbound");
        match outbound {
            RedisOutbound::WeatherEventUpdate(env) => {
                assert_eq!(
                    env.kind,
                    crate::schema::lingtian_weather::WeatherEventUpdateKindV1::Started
                );
                assert_eq!(
                    env.data.kind,
                    crate::schema::lingtian_weather::WeatherEventKindV1::Thunderstorm
                );
                assert_eq!(env.data.zone_id, DEFAULT_ZONE);
                assert_eq!(env.data.started_at_lingtian_tick, 1440);
                assert_eq!(env.data.expires_at_lingtian_tick, 1620);
                assert_eq!(env.data.remaining_ticks, 180);
            }
            other => panic!("expected WeatherEventUpdate, got {other:?}"),
        }
    }

    #[test]
    fn expired_event_publishes_redis_outbound_with_expired_kind() {
        let (mut app, rx) = build_app();
        app.world_mut().send_event(WeatherLifecycleEvent::Expired {
            event: WeatherEvent::Blizzard,
            expired_at_lingtian_tick: 2000,
        });
        app.update();

        let outbound = rx.try_recv().expect("应当 publish 一条 outbound");
        match outbound {
            RedisOutbound::WeatherEventUpdate(env) => {
                assert_eq!(
                    env.kind,
                    crate::schema::lingtian_weather::WeatherEventUpdateKindV1::Expired
                );
                assert_eq!(
                    env.data.kind,
                    crate::schema::lingtian_weather::WeatherEventKindV1::Blizzard
                );
                assert_eq!(env.data.remaining_ticks, 0);
            }
            other => panic!("expected WeatherEventUpdate, got {other:?}"),
        }
    }

    #[test]
    fn no_lifecycle_event_means_no_publish() {
        let (mut app, rx) = build_app();
        app.update();
        assert!(rx.try_recv().is_err(), "无事件时不应 publish");
    }

    #[test]
    fn multiple_events_publish_in_order() {
        let (mut app, rx) = build_app();
        app.world_mut().send_event(WeatherLifecycleEvent::Started {
            event: WeatherEvent::LingMist,
            started_at_lingtian_tick: 100,
            expires_at_lingtian_tick: 200,
        });
        app.world_mut().send_event(WeatherLifecycleEvent::Expired {
            event: WeatherEvent::Thunderstorm,
            expired_at_lingtian_tick: 150,
        });
        app.update();

        let mut received = Vec::new();
        while let Ok(o) = rx.try_recv() {
            if let RedisOutbound::WeatherEventUpdate(env) = o {
                received.push(env.kind);
            }
        }
        // started 后 expired，按 send 顺序
        assert_eq!(
            received,
            vec![
                crate::schema::lingtian_weather::WeatherEventUpdateKindV1::Started,
                crate::schema::lingtian_weather::WeatherEventUpdateKindV1::Expired,
            ]
        );
    }
}
