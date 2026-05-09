pub mod backfire;
pub mod events;
pub mod physics;
pub mod skills;
pub mod state;
pub mod tick;

pub use events::{
    BackfireCauseV2, BackfireLevel, EntityDisplacedByVortexPull, TurbulenceFieldDecayed,
    TurbulenceFieldSpawned, VortexBackfireEventV2, VortexCastEvent, WoliuSkillId,
};
pub use skills::register_skills;

use valence::prelude::{App, Update};

pub fn register(app: &mut App) {
    app.add_event::<VortexCastEvent>();
    app.add_event::<VortexBackfireEventV2>();
    app.add_event::<TurbulenceFieldSpawned>();
    app.add_event::<TurbulenceFieldDecayed>();
    app.add_event::<EntityDisplacedByVortexPull>();
    app.add_systems(Update, tick::turbulence_decay_tick);
}

#[cfg(test)]
mod tests;
