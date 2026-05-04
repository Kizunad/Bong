pub mod intrusion_log;

use valence::prelude::App;

pub fn register(app: &mut App) {
    intrusion_log::register(app);
}
