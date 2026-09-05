use bong_server::combat::is_damageable;
use valence::prelude::{bevy_ecs, App, GameMode, Query};

#[test]
fn resolve_public_game_mode_gate_respects_current_target_state() {
    let mut app = App::new();
    let (survival, creative) = {
        let world = app.world_mut();
        (
            world.spawn(GameMode::Survival).id(),
            world.spawn(GameMode::Creative).id(),
        )
    };

    let world = app.world_mut();
    let mut state = bevy_ecs::system::SystemState::<Query<&GameMode>>::new(world);
    let game_modes = state.get(world);

    assert!(
        is_damageable(survival, &game_modes),
        "Survival target must remain damageable through the public resolver gate"
    );
    assert!(
        !is_damageable(creative, &game_modes),
        "Creative target must be rejected by the public resolver gate"
    );
}
