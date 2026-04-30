pub mod butcher;

use valence::prelude::App;

pub fn register(app: &mut App) {
    app.add_event::<butcher::ButcherRequest>();
    app.add_systems(valence::prelude::Update, butcher::handle_butcher_requests);
}
