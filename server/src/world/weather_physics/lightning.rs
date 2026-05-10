use valence::entity::lightning::LightningEntityBundle;
use valence::prelude::{
    bevy_ecs, Commands, DVec3, Entity, EntityKind, EntityLayerId, Position, Res, ResMut, Resource,
};

use crate::world::dimension::{DimensionKind, DimensionLayers};
use crate::world::environment::{EnvironmentEffect, ZoneEnvironmentRegistry};

pub const WEATHER_LIGHTNING_TICKS_PER_MIN: f32 = 20.0 * 60.0;

#[derive(Debug, Resource)]
pub struct WeatherLightningRng {
    state: u64,
}

impl WeatherLightningRng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    pub fn next_f32(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        ((x & 0x00FF_FFFF) as f32) / (0x0100_0000_u32 as f32)
    }
}

impl Default for WeatherLightningRng {
    fn default() -> Self {
        Self::new(0xB011_6C7A_1EAF_2026)
    }
}

pub fn lightning_strike_at(
    commands: &mut Commands,
    layer_entity: Entity,
    position: DVec3,
) -> Entity {
    commands
        .spawn(LightningEntityBundle {
            kind: EntityKind::LIGHTNING,
            layer: EntityLayerId(layer_entity),
            position: Position::new([position.x, position.y, position.z]),
            ..Default::default()
        })
        .id()
}

pub fn lightning_strike_probability_per_tick(strike_rate_per_min: f32) -> f32 {
    if !strike_rate_per_min.is_finite() || strike_rate_per_min <= 0.0 {
        return 0.0;
    }
    (strike_rate_per_min / WEATHER_LIGHTNING_TICKS_PER_MIN).clamp(0.0, 1.0)
}

pub fn lightning_pillar_tick_system(
    mut commands: Commands,
    layers: Option<Res<DimensionLayers>>,
    registry: Res<ZoneEnvironmentRegistry>,
    mut rng: ResMut<WeatherLightningRng>,
) {
    let Some(layers) = layers else {
        return;
    };
    for (zone, effect) in registry.iter_zone_effects() {
        strike_for_effect(
            &mut commands,
            &layers,
            registry.as_ref(),
            zone,
            effect,
            &mut rng,
        );
    }
}

fn strike_for_effect(
    commands: &mut Commands,
    layers: &DimensionLayers,
    registry: &ZoneEnvironmentRegistry,
    zone: &str,
    effect: &EnvironmentEffect,
    rng: &mut WeatherLightningRng,
) {
    let EnvironmentEffect::LightningPillar {
        center,
        strike_rate_per_min,
        ..
    } = effect
    else {
        return;
    };
    let probability = lightning_strike_probability_per_tick(*strike_rate_per_min);
    if probability <= 0.0 || rng.next_f32() >= probability {
        return;
    }
    let dimension = if registry.dimension(zone) == DimensionKind::Tsy.ident_str() {
        DimensionKind::Tsy
    } else {
        DimensionKind::Overworld
    };
    lightning_strike_at(
        commands,
        layers.entity_for(dimension),
        DVec3::new(center[0], center[1], center[2]),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::entity::lightning::LightningEntity;
    use valence::prelude::{App, Update, With};

    #[test]
    fn lightning_strike_rate_converts_to_per_tick_probability() {
        let p = lightning_strike_probability_per_tick(2.0);
        assert!((p - (2.0 / WEATHER_LIGHTNING_TICKS_PER_MIN)).abs() < 1e-6);
        assert_eq!(lightning_strike_probability_per_tick(-1.0), 0.0);
    }

    #[test]
    fn lightning_pillar_tick_spawns_vanilla_lightning_entity_when_probability_hits() {
        let mut app = App::new();
        app.insert_resource(DimensionLayers {
            overworld: Entity::from_raw(1),
            tsy: Entity::from_raw(2),
        });
        app.insert_resource(WeatherLightningRng::new(1));
        let mut registry = ZoneEnvironmentRegistry::new();
        registry.replace_for_dimension(
            "spawn",
            DimensionKind::Overworld.ident_str(),
            vec![EnvironmentEffect::LightningPillar {
                center: [1.0, 66.0, 2.0],
                radius: 8.0,
                strike_rate_per_min: WEATHER_LIGHTNING_TICKS_PER_MIN,
            }],
        );
        app.insert_resource(registry);
        app.add_systems(Update, lightning_pillar_tick_system);
        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<LightningEntity>>();
        let spawned = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(spawned.len(), 1);
    }

    #[test]
    fn lightning_pillar_tick_respects_zero_probability() {
        let mut app = App::new();
        app.insert_resource(DimensionLayers {
            overworld: Entity::from_raw(1),
            tsy: Entity::from_raw(2),
        });
        app.insert_resource(WeatherLightningRng::new(1));
        let mut registry = ZoneEnvironmentRegistry::new();
        registry.replace_for_dimension(
            "spawn",
            DimensionKind::Overworld.ident_str(),
            vec![EnvironmentEffect::LightningPillar {
                center: [1.0, 66.0, 2.0],
                radius: 8.0,
                strike_rate_per_min: 0.0,
            }],
        );
        app.insert_resource(registry);
        app.add_systems(Update, lightning_pillar_tick_system);
        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<LightningEntity>>();
        let spawned = query.iter(app.world()).collect::<Vec<_>>();
        assert!(spawned.is_empty());
    }
}
