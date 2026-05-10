pub mod events;
pub mod physics;
pub mod skills;
pub mod state;
pub mod tick;

#[allow(unused_imports)]
pub use events::{
    ContamTransferredEvent, DonFalseSkinEvent, FalseSkinDecayedToAshEvent, FalseSkinSheddedEvent,
    PermanentTaintAbsorbedEvent, TuikeSkillId, TuikeSkillVisual,
};
pub use skills::{declare_meridian_dependencies, register_skills};
#[allow(unused_imports)]
pub use state::{
    FalseSkinLayer, FalseSkinResidue, FalseSkinTier, PermanentQiMaxDecay, StackedFalseSkins,
    WornFalseSkin,
};

use valence::prelude::{App, IntoSystemConfigs, Update};

pub fn register(app: &mut App) {
    app.add_event::<DonFalseSkinEvent>();
    app.add_event::<FalseSkinSheddedEvent>();
    app.add_event::<ContamTransferredEvent>();
    app.add_event::<FalseSkinDecayedToAshEvent>();
    app.add_event::<PermanentTaintAbsorbedEvent>();
    app.add_systems(
        Update,
        (
            tick::sync_false_skin_stack_from_inventory,
            tick::false_skin_maintenance_tick.after(tick::sync_false_skin_stack_from_inventory),
            tick::false_skin_residue_decay_tick.after(tick::false_skin_maintenance_tick),
        ),
    );
}

#[cfg(test)]
mod tests;
