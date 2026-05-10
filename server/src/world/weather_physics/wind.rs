use valence::entity::Velocity;
use valence::prelude::{Client, DVec3, Position, Query, Res, Vec3, With};

use crate::lingtian::ZoneWeatherProfileRegistry;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::environment::{EnvironmentEffect, ZoneEnvironmentRegistry};
use crate::world::zone::ZoneRegistry;

const SWIRL_DEGREES_PER_HEIGHT_AMP: f64 = 40.0;
const SWIRL_STRENGTH_RATIO: f64 = 0.2;

/// Weather2 spinEntityv2-inspired dust-devil push vector.
pub fn apply_dust_devil_push(
    entity_pos: DVec3,
    center: DVec3,
    radius: f64,
    strength: f32,
) -> DVec3 {
    compute_dust_devil_push_delta(entity_pos, center, radius, f64::from(strength), 1.0)
}

pub fn compute_dust_devil_push_delta(
    entity_pos: DVec3,
    center: DVec3,
    radius: f64,
    strength: f64,
    height_amp: f64,
) -> DVec3 {
    if radius <= 0.0 || strength <= 0.0 {
        return DVec3::ZERO;
    }
    let offset = entity_pos - center;
    let horizontal = DVec3::new(offset.x, 0.0, offset.z);
    let dist = horizontal.length();
    if dist > radius {
        return DVec3::ZERO;
    }
    let safe_dist = dist.max(1e-6);
    let inward = -horizontal / safe_dist;
    let pull = strength * ((radius - dist) / radius).clamp(0.0, 1.0);

    let angle = offset.z.atan2(offset.x) + (SWIRL_DEGREES_PER_HEIGHT_AMP * height_amp).to_radians();
    let swirl = DVec3::new(angle.cos(), 0.0, angle.sin()) * strength * SWIRL_STRENGTH_RATIO;
    let lift = DVec3::new(0.0, strength * height_amp.max(0.0), 0.0);

    inward * pull + swirl + lift
}

pub fn weather_dust_devil_push_system(
    zones: Option<Res<ZoneRegistry>>,
    registry: Res<ZoneEnvironmentRegistry>,
    profiles: Option<Res<ZoneWeatherProfileRegistry>>,
    mut clients: Query<(&Position, Option<&CurrentDimension>, &mut Velocity), With<Client>>,
) {
    let Some(zones) = zones else {
        return;
    };
    for (position, current_dimension, mut velocity) in &mut clients {
        let dimension = current_dimension
            .map(|dimension| dimension.0)
            .unwrap_or(DimensionKind::Overworld);
        let Some(zone) = zones.find_zone(dimension, position.get()) else {
            continue;
        };
        let strength = profiles
            .as_ref()
            .map(|profiles| {
                profiles
                    .profile_for(zone.name.as_str())
                    .dust_devil_push_strength()
            })
            .unwrap_or(crate::lingtian::weather_profile::DEFAULT_DUST_DEVIL_PUSH_STRENGTH);

        for effect in registry.current(zone.name.as_str()) {
            let EnvironmentEffect::DustDevil { center, radius, .. } = effect else {
                continue;
            };
            let delta = apply_dust_devil_push(
                position.get(),
                DVec3::new(center[0], center[1], center[2]),
                *radius,
                strength,
            );
            velocity.0 += Vec3::new(delta.x as f32, delta.y as f32, delta.z as f32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dust_devil_push_velocity_toward_center_and_up() {
        let delta = apply_dust_devil_push(DVec3::new(6.0, 64.0, 0.0), DVec3::ZERO, 8.0, 0.5);

        assert!(
            delta.x < 0.0,
            "x delta should pull toward center: {delta:?}"
        );
        assert!(delta.y > 0.0, "dust devil should lift targets: {delta:?}");
    }

    #[test]
    fn dust_devil_push_strength_uses_profile_value() {
        let weak = apply_dust_devil_push(DVec3::new(6.0, 64.0, 0.0), DVec3::ZERO, 8.0, 0.5);
        let strong = apply_dust_devil_push(DVec3::new(6.0, 64.0, 0.0), DVec3::ZERO, 8.0, 1.0);

        assert!(strong.length() > weak.length());
    }

    #[test]
    fn dust_devil_push_zero_outside_radius() {
        let delta = apply_dust_devil_push(DVec3::new(12.0, 64.0, 0.0), DVec3::ZERO, 8.0, 0.5);

        assert_eq!(delta, DVec3::ZERO);
    }
}
