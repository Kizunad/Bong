pub mod gm;
pub mod health;
pub mod npc_scenario;
pub mod preview_tp;
pub mod rat;
pub mod season;
pub mod shrine;
pub mod spawn;
pub mod stamina;
pub mod top;
pub mod tptree;
pub mod tpzone;
pub mod tsy_spawn;
pub mod whale;
pub mod wound;
pub mod zones;

use valence::prelude::App;

pub fn register(app: &mut App) {
    spawn::register(app);
    top::register(app);
    zones::register(app);
    season::register(app);
    gm::register(app);
    health::register(app);
    stamina::register(app);
    tptree::register(app);
    tpzone::register(app);
    shrine::register(app);
    wound::register(app);
    tsy_spawn::register(app);
    npc_scenario::register(app);
    preview_tp::register(app);
    rat::register(app);
    whale::register(app);
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
