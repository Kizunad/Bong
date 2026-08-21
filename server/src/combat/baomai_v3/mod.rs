pub mod events;
pub mod physics;
pub mod skills;
pub mod state;
pub mod tick;

pub use events::{
    BaomaiSkillEvent, BaomaiSkillId, BloodBurnEvent, BodyTranscendenceExpiredEvent,
    DispersedQiEvent, MountainShakeEvent, OverloadMeridianRippleEvent,
};
pub use skills::register_skills;

use valence::prelude::{App, IntoSystemConfigs, Update};

pub fn register(app: &mut App) {
    // P1 technique registry builds the complete dependency table before this combat module
    // registers. Keeping a second late mutation path would either bypass startup validation or
    // trigger the duplicate-declaration guard.
    app.add_event::<BaomaiSkillEvent>();
    app.add_event::<MountainShakeEvent>();
    app.add_event::<BloodBurnEvent>();
    app.add_event::<DispersedQiEvent>();
    app.add_event::<OverloadMeridianRippleEvent>();
    app.add_event::<BodyTranscendenceExpiredEvent>();
    app.add_event::<crate::qi_physics::QiTransfer>();
    app.add_event::<crate::skill::events::SkillXpGain>();
    app.add_event::<crate::cultivation::meridian::severed::MeridianSeveredEvent>();
    app.add_event::<crate::cultivation::tribulation::JueBiTriggerEvent>();
    app.add_systems(
        Update,
        (
            tick::blood_burn_tick.in_set(crate::combat::CombatSystemSet::Physics),
            tick::body_transcendence_tick.in_set(crate::combat::CombatSystemSet::Physics),
        ),
    );
}

#[cfg(test)]
mod tests;
