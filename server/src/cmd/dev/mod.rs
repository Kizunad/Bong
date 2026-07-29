// plan-bughunt-animal-air-spawn-gravity-v1 P2：确定性 ambient one-shot dev 命令
pub mod ambient_spawn;
// plan-tribulation-balance-v1 P0：/balance tribulation dev 命令
pub mod balance;
pub mod baolongwang;
pub mod block_picker;
pub mod clearinv;
pub mod coffin;
pub mod fog;
pub mod gallery;
pub mod give;
pub mod gm;
pub mod health;
pub mod heiwushi;
pub mod kill;
pub mod meridian;
pub mod nourish;
pub mod npc_scenario;
pub mod preview_tp;
pub mod qi;
pub mod race;
pub mod rat;
pub mod realm;
pub mod reset;
pub mod revive;
pub mod riskmap;
pub mod season;
pub mod shader_push;
pub mod shrine;
pub mod spawn;
pub mod stamina;
pub mod supply_coffin;
pub mod technique;
pub mod time;
pub mod top;
pub mod tpdim;
pub mod tppoi;
pub mod tptree;
pub mod tpzone;
pub mod tribulation_debug;
pub mod tribulation_rechallenge;
pub mod tsy_spawn;
pub mod whale;
pub mod wound;
pub mod zone_qi;
pub mod zones;

use valence::prelude::App;

pub fn dev_mode_enabled() -> bool {
    std::env::var("BONG_DEV_MODE").ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        )
    })
}

pub fn register(app: &mut App) {
    register_for_dev_mode(app, dev_mode_enabled());
}

pub(crate) fn register_for_dev_mode(app: &mut App, dev_mode_enabled: bool) {
    if dev_mode_enabled {
        ambient_spawn::register_enabled(app);
        nourish::register_enabled(app);
    }
    balance::register(app);
    baolongwang::register(app);
    block_picker::register(app);
    gallery::register(app);
    coffin::register(app);
    clearinv::register(app);
    fog::register(app);
    give::register(app);
    heiwushi::register(app);
    spawn::register(app);
    top::register(app);
    zones::register(app);
    season::register(app);
    gm::register(app);
    health::register(app);
    kill::register(app);
    meridian::register(app);
    qi::register(app);
    race::register(app);
    realm::register(app);
    reset::register(app);
    revive::register(app);
    stamina::register(app);
    supply_coffin::register(app);
    technique::register(app);
    time::register(app);
    tptree::register(app);
    tpdim::register(app);
    tppoi::register(app);
    tpzone::register(app);
    shrine::register(app);
    wound::register(app);
    tsy_spawn::register(app);
    npc_scenario::register(app);
    preview_tp::register(app);
    rat::register(app);
    riskmap::register(app);
    whale::register(app);
    zone_qi::register(app);
    shader_push::register(app);
    tribulation_debug::register(app);
    tribulation_rechallenge::register(app);
}

#[cfg(test)]
pub(crate) mod test_support {
    use valence::prelude::{App, Entity, Position};
    use valence::testing::create_mock_client;

    pub fn spawn_test_client(app: &mut App, username: &str, position: [f64; 3]) -> Entity {
        let (mut client_bundle, _helper) = create_mock_client(username);
        client_bundle.player.position = Position::new(position);
        app.world_mut().spawn(client_bundle).id()
    }

    pub fn run_update(app: &mut App) {
        app.update();
    }
}
