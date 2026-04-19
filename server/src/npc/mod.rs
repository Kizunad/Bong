pub mod brain;
pub mod faction;
pub mod lifecycle;
pub mod lingtian_pressure;
pub mod movement;
pub mod navigator;
pub mod patrol;
pub mod scenario;
pub mod spawn;
pub mod sync;

use valence::prelude::App;

pub fn register(app: &mut App) {
    tracing::info!(
        "[bong][npc] registering faction/spawn/lifecycle/patrol/sync/brain/movement/navigator/scenario/lingtian_pressure systems"
    );
    faction::register(app);
    spawn::register(app);
    lifecycle::register(app);
    patrol::register(app);
    sync::register(app);
    brain::register(app);
    movement::register(app); // Ability layer — ticks overrides before Navigator
    navigator::register(app);
    scenario::register(app);
    lingtian_pressure::register(app);
}
