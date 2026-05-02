pub mod bone_coin;
pub mod butcher;
pub mod components;
pub mod drop;

use valence::prelude::App;

pub fn register(app: &mut App) {
    app.add_event::<butcher::ButcherRequest>();
    app.add_event::<bone_coin::BoneCoinCraftRequest>();
    app.add_event::<bone_coin::BoneCoinCrafted>();
    app.add_systems(
        valence::prelude::Update,
        (
            butcher::handle_butcher_requests,
            bone_coin::handle_bone_coin_craft_requests,
            drop::fauna_drop_system,
        ),
    );
}
