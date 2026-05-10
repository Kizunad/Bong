use valence::prelude::{
    bevy_ecs, Client, Commands, Component, Entity, Position, Query, Res, ViewDistance, With,
};

use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::environment::{EnvironmentEffect, ZoneEnvironmentRegistry};
use crate::world::zone::ZoneRegistry;

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct WeatherVisionRestore {
    pub original_chunks: u8,
}

pub type WeatherVisionClientQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static mut ViewDistance,
        Option<&'static WeatherVisionRestore>,
    ),
    With<Client>,
>;

pub fn obscure_vision(
    view_distance: &mut ViewDistance,
    radius_blocks: f32,
) -> WeatherVisionRestore {
    let original = view_distance.get();
    let target = chunks_for_radius(radius_blocks);
    if original > target {
        view_distance.set(target);
    }
    WeatherVisionRestore {
        original_chunks: original,
    }
}

pub fn restore_vision(view_distance: &mut ViewDistance, restore: WeatherVisionRestore) {
    if view_distance.get() < restore.original_chunks {
        view_distance.set(restore.original_chunks);
    }
}

pub fn weather_vision_obscure_system(
    mut commands: Commands,
    zones: Option<Res<ZoneRegistry>>,
    registry: Res<ZoneEnvironmentRegistry>,
    mut clients: WeatherVisionClientQuery<'_, '_>,
) {
    let Some(zones) = zones else {
        return;
    };
    for (entity, position, current_dimension, mut view_distance, restore) in &mut clients {
        let dim = current_dimension
            .map(|dimension| dimension.0)
            .unwrap_or(DimensionKind::Overworld);
        let Some(zone) = zones.find_zone(dim, position.get()) else {
            if let Some(restore) = restore.copied() {
                restore_vision(&mut view_distance, restore);
                commands.entity(entity).remove::<WeatherVisionRestore>();
            }
            continue;
        };
        let should_obscure = registry
            .current(zone.name.as_str())
            .iter()
            .any(is_opaque_fog_veil);
        if should_obscure {
            if restore.is_none() {
                let restore = obscure_vision(&mut view_distance, 16.0);
                commands.entity(entity).insert(restore);
            }
        } else if let Some(restore) = restore.copied() {
            restore_vision(&mut view_distance, restore);
            commands.entity(entity).remove::<WeatherVisionRestore>();
        }
    }
}

fn is_opaque_fog_veil(effect: &EnvironmentEffect) -> bool {
    matches!(
        effect,
        EnvironmentEffect::FogVeil { density, .. } if *density >= 0.85
    )
}

fn chunks_for_radius(radius_blocks: f32) -> u8 {
    if !radius_blocks.is_finite() || radius_blocks <= 0.0 {
        return 1;
    }
    ((radius_blocks / 16.0).ceil() as u8).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::ViewDistance;

    #[test]
    fn obscure_vision_reduces_view_distance_inside_aabb() {
        let mut view_distance = ViewDistance::new(10);
        let restore = obscure_vision(&mut view_distance, 16.0);

        assert_eq!(restore.original_chunks, 10);
        assert_eq!(view_distance.get(), 2);
    }

    #[test]
    fn obscure_vision_restores_on_zone_exit() {
        let mut view_distance = ViewDistance::new(10);
        let restore = obscure_vision(&mut view_distance, 16.0);

        restore_vision(&mut view_distance, restore);

        assert_eq!(view_distance.get(), 10);
    }
}
