pub mod brain;
pub mod patrol;
pub mod spawn;
pub mod sync;

use valence::prelude::App;

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering spawn/patrol/sync/brain systems");
    spawn::register(app);
    patrol::register(app);
    sync::register(app);
    brain::register(app);
}
