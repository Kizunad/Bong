pub mod bone_coin;
pub mod butcher;
pub mod components;
pub mod drop;
pub mod experience;
pub mod rat_phase;
pub mod visual;

use valence::prelude::{App, IntoSystemConfigs};

pub fn register(app: &mut App) {
    app.add_event::<butcher::ButcherRequest>();
    app.add_event::<bone_coin::BoneCoinCraftRequest>();
    app.add_event::<bone_coin::BoneCoinCrafted>();
    app.add_event::<rat_phase::RatPhaseChangeEvent>();
    app.add_systems(
        valence::prelude::Update,
        (
            rat_phase::pressure_sensor_tick_system,
            rat_phase::emit_rat_swarm_aura_vfx_system.after(rat_phase::pressure_sensor_tick_system),
            rat_phase::apply_rat_phase_change_system,
            rat_phase::advance_transitioning_phase_system,
            rat_phase::apply_rat_phase_visual_system
                .after(rat_phase::apply_rat_phase_change_system)
                .after(rat_phase::advance_transitioning_phase_system),
            rat_phase::release_drained_qi_on_death_system.before(drop::fauna_drop_system),
            experience::emit_fauna_spawn_vfx_system,
            experience::emit_fauna_spawn_ambient_audio_system,
            experience::emit_fauna_attack_audio_system,
            experience::emit_fauna_death_vfx_audio_system.before(drop::fauna_drop_system),
            experience::emit_rat_bite_audio_system,
            butcher::handle_butcher_requests,
            bone_coin::handle_bone_coin_craft_requests,
            drop::fauna_drop_system,
        ),
    );
}
