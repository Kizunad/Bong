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

use crate::cultivation::meridian::severed::SkillMeridianDependencies;

pub fn register(app: &mut App) {
    if let Some(mut dependencies) = app
        .world_mut()
        .get_resource_mut::<SkillMeridianDependencies>()
    {
        skills::declare_meridian_dependencies(&mut dependencies);
    } else {
        let mut dependencies = SkillMeridianDependencies::default();
        skills::declare_meridian_dependencies(&mut dependencies);
        app.insert_resource(dependencies);
    }
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
