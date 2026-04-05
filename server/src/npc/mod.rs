pub mod brain;
pub mod spawn;
pub mod sync;

use valence::prelude::App;

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering spawn/sync/brain systems");
    spawn::register(app);
    sync::register(app);
    brain::register(app);
}
