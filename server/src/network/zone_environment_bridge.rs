use valence::prelude::{ident, Client, Query, Res, ResMut, With};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::schema::zone_environment::ZoneEnvironmentStateV1;
use crate::world::environment::ZoneEnvironmentRegistry;

pub const ZONE_ENVIRONMENT_CLIENT_CHANNEL: &str = "bong:zone_environment";

pub fn zone_environment_broadcast_system(
    mut registry: ResMut<ZoneEnvironmentRegistry>,
    redis: Res<RedisBridgeResource>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    let dirty_zones = registry.drain_dirty();
    for zone in dirty_zones {
        let state = ZoneEnvironmentStateV1::new(
            zone.clone(),
            registry.current(zone.as_str()).to_vec(),
            registry.generation(zone.as_str()),
        );

        let bytes = match state.to_json_bytes_checked() {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(
                    "[bong][zone_environment] dropping invalid state for zone={zone}: {error:?}"
                );
                continue;
            }
        };

        if let Err(error) = redis
            .tx_outbound
            .send(RedisOutbound::ZoneEnvironmentUpdate(state.clone()))
        {
            tracing::warn!(
                "[bong][zone_environment] dropped Redis ZoneEnvironmentStateV1: {error}"
            );
        }

        let mut sent = 0usize;
        for mut client in &mut clients {
            let _ = ZONE_ENVIRONMENT_CLIENT_CHANNEL;
            client.send_custom_payload(ident!("bong:zone_environment"), bytes.as_slice());
            sent += 1;
        }
        tracing::debug!(
            "[bong][zone_environment] broadcast zone={zone} generation={} to {sent} client(s)",
            state.generation
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::{unbounded, Receiver};
    use valence::prelude::{App, Update};

    fn setup_app() -> (App, Receiver<RedisOutbound>) {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.insert_resource(ZoneEnvironmentRegistry::new());
        app.add_systems(Update, zone_environment_broadcast_system);
        (app, rx_outbound)
    }

    #[test]
    fn dirty_registry_state_publishes_to_redis() {
        let (mut app, rx) = setup_app();
        {
            let mut registry = app.world_mut().resource_mut::<ZoneEnvironmentRegistry>();
            registry.add(
                "spawn",
                crate::world::environment::EnvironmentEffect::FogVeil {
                    aabb_min: [0.0, 60.0, 0.0],
                    aabb_max: [16.0, 90.0, 16.0],
                    tint_rgb: [80, 90, 100],
                    density: 0.25,
                },
            );
        }
        app.update();

        let outbound = rx.try_recv().expect("expected zone environment outbound");
        let RedisOutbound::ZoneEnvironmentUpdate(state) = outbound else {
            panic!("expected ZoneEnvironmentUpdate outbound");
        };
        assert_eq!(state.zone_id, "spawn");
        assert_eq!(state.generation, 1);
        assert_eq!(state.effects.len(), 1);
        assert_eq!(state.effects[0].kind(), "fog_veil");
    }

    #[test]
    fn clean_registry_does_not_publish() {
        let (mut app, rx) = setup_app();
        app.update();
        assert!(rx.try_recv().is_err());
    }
}
