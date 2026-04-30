pub mod butcher;

use valence::prelude::App;

pub fn register(app: &mut App) {
    app.add_event::<butcher::ButcherRequest>();
}
