pub mod brain;
pub mod brain_rat;
pub mod faction;
pub mod farming_brain;
pub mod hunger;
pub mod intrusion_npc;
pub mod lifecycle;
pub mod lingtian_pressure;
pub mod lod;
pub mod loot;
pub mod movement;
pub mod navigator;
pub mod patrol;
pub mod poi_rogue_village;
pub mod possession;
pub mod relic;
pub mod scattered_cultivator;
pub mod scenario;
pub mod social;
pub mod spawn;
pub mod spawn_rat;
pub mod sync;
pub mod territory;
pub mod tribulation;
pub mod tsy_hostile;
pub mod zong_keeper;

use valence::prelude::App;

pub fn register(app: &mut App) {
    tracing::info!(
        "[bong][npc] registering faction/spawn/lifecycle/hunger/possession/tribulation/patrol/sync/brain/farming/movement/navigator/scenario/lingtian_pressure/territory systems"
    );
    faction::register(app);
    spawn::register(app);
    lifecycle::register(app);
    hunger::register(app);
    possession::register(app);
    tribulation::register(app);
    patrol::register(app);
    sync::register(app);
    brain::register(app);
    brain_rat::register(app);
    farming_brain::register(app);
    tsy_hostile::register(app);
    movement::register(app); // Ability layer — ticks overrides before Navigator
    navigator::register(app);
    scenario::register(app);
    lingtian_pressure::register(app);
    territory::register(app);
    scattered_cultivator::register(app);
    social::register(app);
    intrusion_npc::register(app);
    relic::register(app);
    lod::register(app);
    zong_keeper::register(app);
    poi_rogue_village::log_rogue_village_contract();
}
