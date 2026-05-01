//! 余烬死域 tick：零回复之外的自然真元挥发。

use valence::prelude::{bevy_ecs, Position, Query, Res, Resource, Without};

use crate::npc::spawn::NpcMarker;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::{Zone, ZoneRegistry};

use super::components::Cultivation;

pub const DEAD_ZONE_QI_THRESHOLD: f64 = 0.01;
pub const TICKS_PER_MINUTE: f64 = 20.0 * 60.0;

#[derive(Debug, Clone, Copy, Resource)]
pub struct DeadZoneTickHandler {
    pub qi_drain_per_minute: f64,
    pub shelflife_zone_multiplier: f32,
}

impl Default for DeadZoneTickHandler {
    fn default() -> Self {
        Self {
            qi_drain_per_minute: 1.0,
            shelflife_zone_multiplier: 3.0,
        }
    }
}

impl DeadZoneTickHandler {
    pub fn qi_drain_per_tick(self) -> f64 {
        (self.qi_drain_per_minute / TICKS_PER_MINUTE).max(0.0)
    }
}

pub fn is_dead_zone(zone: &Zone) -> bool {
    zone.spirit_qi >= 0.0 && zone.spirit_qi < DEAD_ZONE_QI_THRESHOLD
}

pub fn drain_for_ticks(handler: DeadZoneTickHandler, ticks: u64) -> f64 {
    handler.qi_drain_per_tick() * ticks as f64
}

pub fn apply_dead_zone_drain(cultivation: &mut Cultivation, handler: DeadZoneTickHandler) -> f64 {
    let drain = handler
        .qi_drain_per_tick()
        .min(cultivation.qi_current.max(0.0));
    cultivation.qi_current = (cultivation.qi_current - drain).max(0.0);
    drain
}

pub fn dead_zone_silent_qi_loss_tick(
    handler: Option<Res<DeadZoneTickHandler>>,
    zones: Option<Res<ZoneRegistry>>,
    mut players: Query<
        (&Position, Option<&CurrentDimension>, &mut Cultivation),
        Without<NpcMarker>,
    >,
) {
    let Some(zones) = zones else {
        return;
    };
    let handler = handler.as_deref().copied().unwrap_or_default();
    if handler.qi_drain_per_tick() <= 0.0 {
        return;
    }

    for (position, current_dim, mut cultivation) in &mut players {
        let dim = current_dim.map(|c| c.0).unwrap_or(DimensionKind::Overworld);
        let Some(zone) = zones.find_zone(dim, position.0) else {
            continue;
        };
        if is_dead_zone(zone) {
            apply_dead_zone_drain(&mut cultivation, handler);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use valence::prelude::{App, DVec3, Update};

    fn zone(name: &str, spirit_qi: f64) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 100.0, 100.0)),
            spirit_qi,
            danger_level: 5,
            active_events: vec!["no_cadence".to_string()],
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        }
    }

    #[test]
    fn dead_zone_predicate_excludes_negative_fields() {
        assert!(is_dead_zone(&zone("ash", 0.0)));
        assert!(is_dead_zone(&zone("ash_edge", 0.009)));
        assert!(!is_dead_zone(&zone("waste", 0.05)));
        assert!(!is_dead_zone(&zone("negative", -0.1)));
    }

    #[test]
    fn sixty_seconds_drains_one_qi_for_all_realms() {
        let handler = DeadZoneTickHandler::default();
        for realm in [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ] {
            let mut cultivation = Cultivation {
                realm,
                qi_current: 10.0,
                qi_max: 100.0,
                ..Cultivation::default()
            };
            let drain = drain_for_ticks(handler, TICKS_PER_MINUTE as u64);
            cultivation.qi_current = (cultivation.qi_current - drain).max(0.0);
            assert!(
                (cultivation.qi_current - 9.0).abs() < 1e-9,
                "realm={realm:?}"
            );
        }
    }

    #[test]
    fn system_drains_players_inside_dead_zone_only() {
        let mut app = App::new();
        app.insert_resource(DeadZoneTickHandler::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![
                zone("ash", 0.0),
                Zone {
                    bounds: (DVec3::new(200.0, 0.0, 0.0), DVec3::new(300.0, 100.0, 100.0)),
                    ..zone("normal", 0.4)
                },
            ],
        });
        app.add_systems(Update, dead_zone_silent_qi_loss_tick);

        let inside = app
            .world_mut()
            .spawn((
                Position::new([10.0, 66.0, 10.0]),
                Cultivation {
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Cultivation::default()
                },
            ))
            .id();
        let outside = app
            .world_mut()
            .spawn((
                Position::new([210.0, 66.0, 10.0]),
                Cultivation {
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Cultivation::default()
                },
            ))
            .id();

        app.update();

        let inside_qi = app.world().get::<Cultivation>(inside).unwrap().qi_current;
        let outside_qi = app.world().get::<Cultivation>(outside).unwrap().qi_current;
        assert!((inside_qi - (10.0 - 1.0 / TICKS_PER_MINUTE)).abs() < 1e-9);
        assert_eq!(outside_qi, 10.0);
    }
}
