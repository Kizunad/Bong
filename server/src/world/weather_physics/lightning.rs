use valence::entity::lightning::LightningEntityBundle;
use valence::prelude::{
    Commands, DVec3, Entity, EntityKind, EntityLayerId, EventReader, Position, Res,
};

use crate::world::dimension::{DimensionKind, DimensionLayers};
use crate::world::environment::{
    EnvironmentEffect, ZoneEnvironmentLifecycleEvent, ZoneEnvironmentRegistry,
};

pub const WEATHER_LIGHTNING_TICKS_PER_MIN: f32 = 20.0 * 60.0;

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

pub fn lightning_pillar_lifecycle_system(
    mut commands: Commands,
    layers: Option<Res<DimensionLayers>>,
    registry: Res<ZoneEnvironmentRegistry>,
    mut lifecycle: EventReader<ZoneEnvironmentLifecycleEvent>,
) {
    let Some(layers) = layers else {
        return;
    };
    for event in lifecycle.read() {
        match event {
            ZoneEnvironmentLifecycleEvent::EffectAdded { zone, index } => {
                if let Some(effect) = registry.effect_at(zone, *index) {
                    strike_for_effect(&mut commands, &layers, registry.as_ref(), zone, effect);
                }
            }
            ZoneEnvironmentLifecycleEvent::Replaced { zone } => {
                for effect in registry.current(zone) {
                    strike_for_effect(&mut commands, &layers, registry.as_ref(), zone, effect);
                }
            }
            ZoneEnvironmentLifecycleEvent::EffectRemoved { .. } => {}
        }
    }
}

fn strike_for_effect(
    commands: &mut Commands,
    layers: &DimensionLayers,
    registry: &ZoneEnvironmentRegistry,
    zone: &str,
    effect: &EnvironmentEffect,
) {
    let EnvironmentEffect::LightningPillar {
        center,
        strike_rate_per_min,
        ..
    } = effect
    else {
        return;
    };
    if lightning_strike_probability_per_tick(*strike_rate_per_min) <= 0.0 {
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
    fn lightning_pillar_lifecycle_spawns_vanilla_lightning_entity() {
        let mut app = App::new();
        app.insert_resource(DimensionLayers {
            overworld: Entity::from_raw(1),
            tsy: Entity::from_raw(2),
        });
        let mut registry = ZoneEnvironmentRegistry::new();
        registry.replace_for_dimension(
            "spawn",
            DimensionKind::Overworld.ident_str(),
            vec![EnvironmentEffect::LightningPillar {
                center: [1.0, 66.0, 2.0],
                radius: 8.0,
                strike_rate_per_min: 1.0,
            }],
        );
        app.insert_resource(registry);
        app.add_event::<ZoneEnvironmentLifecycleEvent>();
        app.add_systems(Update, lightning_pillar_lifecycle_system);
        app.world_mut()
            .send_event(ZoneEnvironmentLifecycleEvent::Replaced {
                zone: "spawn".to_string(),
            });
        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<LightningEntity>>();
        let spawned = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(spawned.len(), 1);
    }
}
