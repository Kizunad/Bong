//! plan-zone-weather-v1 P2 — server-side physical hooks for weather effects.

use valence::prelude::{App, IntoSystemConfigs, Update};

pub mod lightning;
pub mod vision;
pub mod wind;

pub fn register(app: &mut App) {
    app.add_systems(
        Update,
        (
            lightning::lightning_pillar_lifecycle_system
                .after(crate::world::environment::publish_zone_environment_lifecycle_events),
            wind::weather_dust_devil_push_system
                .after(crate::world::weather_to_environment::weather_environment_sync_system),
            vision::weather_vision_obscure_system
                .after(crate::world::weather_to_environment::weather_environment_sync_system),
        ),
    );
}
