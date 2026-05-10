use valence::prelude::{ident, Added, Client, Query, Res, ResMut, With};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::schema::zone_environment::ZoneEnvironmentStateV1;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::environment::ZoneEnvironmentRegistry;

pub fn mark_zone_environment_dirty_for_new_clients(
    new_clients: Query<(), (With<Client>, Added<Client>)>,
    mut registry: ResMut<ZoneEnvironmentRegistry>,
) {
    if new_clients.iter().next().is_some() {
        registry.mark_all_dirty_for_snapshot();
    }
}

pub fn zone_environment_broadcast_system(
    mut registry: ResMut<ZoneEnvironmentRegistry>,
    redis: Res<RedisBridgeResource>,
    mut clients: Query<(&mut Client, Option<&CurrentDimension>), With<Client>>,
) {
    let dirty_zones = registry.drain_dirty();
    for zone in dirty_zones {
        let dimension = registry.dimension(zone.as_str()).to_string();
        let state = ZoneEnvironmentStateV1::new_with_dimension(
            dimension.clone(),
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
                registry.mark_dirty_for_retry(zone);
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
            registry.mark_dirty_for_retry(zone.clone());
        }

        let mut sent = 0usize;
        for (mut client, current_dimension) in &mut clients {
            let client_dimension = current_dimension
                .map(|dimension| dimension.0)
                .unwrap_or(DimensionKind::Overworld)
                .ident_str();
            if client_dimension != dimension {
                continue;
            }
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
        assert_eq!(state.dimension, "minecraft:overworld");
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

    #[test]
    fn redis_send_failure_marks_zone_dirty_for_retry() {
        let (mut app, rx) = setup_app();
        drop(rx);
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

        let mut registry = app.world_mut().resource_mut::<ZoneEnvironmentRegistry>();
        assert_eq!(registry.drain_dirty(), vec!["spawn".to_string()]);
    }
}
