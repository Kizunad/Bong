pub mod backfire;
#[allow(dead_code)]
pub mod erosion;
pub mod events;
pub mod physics;
pub mod skills;
pub mod state;
pub mod tick;

#[allow(unused_imports)]
pub use erosion::{VoidErosion, VoidErosionAdvanceEvent, VoidErosionStage};
pub use events::{
    BackfireCauseV2, BackfireLevel, EntityDisplacedByVortexPull, TurbulenceFieldDecayed,
    TurbulenceFieldSpawned, VortexBackfireEventV2, VortexCastEvent, WoliuSkillId,
};
pub use skills::register_skills;
#[allow(unused_imports)]
pub use state::{ScheduledEcho, VoidCoreState};

use valence::prelude::{App, IntoSystemConfigs, Update};

pub fn register(app: &mut App) {
    app.add_event::<VortexCastEvent>();
    app.add_event::<VortexBackfireEventV2>();
    app.add_event::<TurbulenceFieldSpawned>();
    app.add_event::<TurbulenceFieldDecayed>();
    app.add_event::<EntityDisplacedByVortexPull>();
    app.add_event::<VoidErosionAdvanceEvent>();
    // P1 technique registry builds the complete dependency table before combat registers.
    // Do not append the same declarations in Startup: duplicate declarations are fatal so the
    // table cannot silently drift after its startup wiring audit.
    app.add_systems(
        Update,
        (
            tick::turbulence_decay_tick,
            tick::update_turbulence_exposure_tick.after(tick::turbulence_decay_tick),
            tick::heart_active_backfire_tick.after(tick::update_turbulence_exposure_tick),
            tick::vortex_v2_state_lifecycle_tick.after(tick::heart_active_backfire_tick),
        ),
    );
    // plan-combat-skill-feedback-bridges-v1 P3 — 每 600 tick 检测虚蚀阶段推进。
    app.add_systems(Update, erosion::void_erosion_check_system);
}

#[cfg(test)]
mod tests;
