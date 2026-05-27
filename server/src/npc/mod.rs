pub mod abstract_combat;
#[allow(dead_code)]
pub mod abstract_combat_system;
pub mod brain;
pub mod brain_rat;
pub mod brain_whale;
#[cfg(test)]
mod combat_gear_integration_test;
pub mod combat_power;
pub mod dormant;
pub mod dying_master;
#[allow(dead_code)]
pub mod equipment;
pub mod faction;
pub mod farming_brain;
pub mod hunger;
pub mod hydrate;
#[cfg(test)]
mod integration_tests;
pub mod intel;
pub mod interaction_memory;
pub mod intrusion_npc;
pub mod lifecycle;
pub mod lingtian_pressure;
pub mod lod;
pub mod loot;
pub mod movement;
pub mod navigator;
pub mod npc_skill;
pub mod patrol;
pub mod perf;
pub mod poi_rogue_village;
pub mod possession;
pub mod relic;
pub mod scattered_cultivator;
pub mod scenario;
pub mod schedule;
pub mod seasonal_behavior;
pub mod skull_fiend;
pub mod social;
pub mod spatial;
pub mod spawn;
#[allow(dead_code)]
pub mod spawn_pillar;
pub mod spawn_rat;
pub mod spawn_whale;
pub mod sync;
#[allow(dead_code)]
pub mod technique;
pub mod territory;
#[allow(dead_code)]
pub mod trade;
pub mod tribulation;
pub mod tsy_hostile;
pub mod whale_narration;
pub mod zong_keeper;

use valence::prelude::{App, Update};

pub fn register(app: &mut App) {
    tracing::info!(
        "[bong][npc] registering perf/spatial/faction/spawn/lifecycle/hunger/possession/tribulation/seasonal/patrol/sync/brain/farming/movement/navigator/scenario/schedule/lingtian_pressure/territory/dormant systems"
    );
    perf::register(app);
    spatial::register(app);
    faction::register(app);
    spawn::register(app);
    lifecycle::register(app);
    hunger::register(app);
    possession::register(app);
    tribulation::register(app);
    seasonal_behavior::register(app);
    patrol::register(app);
    sync::register(app);
    brain::register(app);
    brain_rat::register(app);
    brain_whale::register(app);
    whale_narration::register(app);
    dormant::register(app);
    farming_brain::register(app);
    skull_fiend::register(app);
    tsy_hostile::register(app);
    movement::register(app); // Ability layer — ticks overrides before Navigator
    hydrate::register(app);
    navigator::register(app);
    scenario::register(app);
    schedule::register(app);
    lingtian_pressure::register(app);
    territory::register(app);
    scattered_cultivator::register(app);
    social::register(app);
    intrusion_npc::register(app);
    interaction_memory::register(app);
    trade::register(app);
    relic::register(app);
    lod::register(app);
    zong_keeper::register(app);
    app.add_event::<dying_master::DyingMasterEncounterEvent>();
    app.add_systems(Update, dying_master::dying_master_despawn_tick);
    dying_master::log_dying_master_contract();
    poi_rogue_village::log_rogue_village_contract();
}
