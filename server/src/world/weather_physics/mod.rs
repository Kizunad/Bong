//! plan-zone-weather-v1 P2 — server-side physical hooks for weather effects.

use valence::prelude::{App, IntoSystemConfigs, Update};

pub mod lightning;
#[cfg(test)]
pub mod tribulation_scorch;
pub mod vision;
pub mod wind;

pub fn register(app: &mut App) {
    app.insert_resource(lightning::WeatherLightningRng::default());
    app.add_systems(
        Update,
        (
            lightning::lightning_pillar_tick_system
                .after(crate::world::weather_to_environment::weather_environment_sync_system),
            wind::weather_dust_devil_push_system
                .after(crate::world::weather_to_environment::weather_environment_sync_system),
            vision::weather_vision_obscure_system
                .after(crate::world::weather_to_environment::weather_environment_sync_system),
        ),
    );
}
