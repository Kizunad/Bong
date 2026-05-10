pub mod events;
pub mod physics;
pub mod skills;
pub mod state;
pub mod tick;

pub use events::{
    DuguSelfRevealedEvent, EclipseNeedleEvent, PenetrateChainEvent, PermanentQiMaxDecayApplied,
    ReverseTriggeredEvent, SelfCureProgressEvent, ShroudActivatedEvent,
};
pub use skills::{declare_meridian_dependencies, register_skills};

use valence::prelude::{App, IntoSystemConfigs, Update};

pub fn register(app: &mut App) {
    app.add_event::<EclipseNeedleEvent>();
    app.add_event::<SelfCureProgressEvent>();
    app.add_event::<PenetrateChainEvent>();
    app.add_event::<ShroudActivatedEvent>();
    app.add_event::<ReverseTriggeredEvent>();
    app.add_event::<PermanentQiMaxDecayApplied>();
    app.add_event::<DuguSelfRevealedEvent>();
    app.add_systems(
        Update,
        (
            tick::taint_decay_tick,
            tick::permanent_qi_max_decay_tick.after(tick::taint_decay_tick),
            tick::shroud_maintain_tick.after(tick::permanent_qi_max_decay_tick),
            tick::reverse_aftermath_decay_tick.after(tick::shroud_maintain_tick),
        ),
    );
}

#[cfg(test)]
mod matrix_tests;
#[cfg(test)]
mod tests;
