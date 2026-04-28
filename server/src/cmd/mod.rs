pub mod ping;

use valence::prelude::App;

pub fn register(app: &mut App) {
    ping::register(app);
}
