use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{
    App, Client, EventReader, EventWriter, IntoSystemConfigs, Query, Res, Update,
};

use crate::combat::components::{Lifecycle, LifecycleState, Wounds};
use crate::cultivation::death_hooks::CultivationReviveRequested;
use crate::cultivation::tick::CultivationClock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviveCmd {
    Self_,
}

impl Command for ReviveCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("revive")
            .literal("self")
            .with_executable(|_| ReviveCmd::Self_);
    }
}

pub fn register(app: &mut App) {
    app.add_event::<CultivationReviveRequested>()
        .add_command::<ReviveCmd>()
        .add_systems(
            Update,
            handle_revive.before(crate::cultivation::death_hooks::on_cultivation_revive_requested),
        );
}

pub fn handle_revive(
    mut events: EventReader<CommandResultEvent<ReviveCmd>>,
    clock: Option<Res<CultivationClock>>,
    mut revive_requests: EventWriter<CultivationReviveRequested>,
    mut players: Query<(&mut Lifecycle, Option<&mut Wounds>, &mut Client)>,
) {
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for event in events.read() {
        let Ok((mut lifecycle, wounds, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        if lifecycle.state == LifecycleState::Alive {
            client.send_chat_message("[dev] revive self ignored: player is already alive");
            continue;
        }

        lifecycle.revive(tick);
        if let Some(mut wounds) = wounds {
            wounds.health_current = wounds.health_max.max(1.0);
        }
        revive_requests.send(CultivationReviveRequested {
            entity: event.executor,
        });
        tracing::warn!("[dev-cmd] force revive self at tick {tick}");
        client.send_chat_message("[dev] revive self queued CultivationReviveRequested");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::Events;

    fn setup_app() -> App {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 88 });
        app.add_event::<CommandResultEvent<ReviveCmd>>();
        app.add_event::<CultivationReviveRequested>();
        app.add_systems(Update, handle_revive);
        app
    }

    fn spawn_player(
        app: &mut App,
        lifecycle: Lifecycle,
        wounds: Wounds,
    ) -> valence::prelude::Entity {
        let player = spawn_test_client(app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut()
            .entity_mut(player)
            .insert((lifecycle, wounds));
        player
    }

    fn send(app: &mut App, player: valence::prelude::Entity) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<ReviveCmd>>>()
            .send(CommandResultEvent {
                result: ReviveCmd::Self_,
                executor: player,
                modifiers: Default::default(),
            });
    }

    fn setup_integration_app() -> App {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 88 });
        app.insert_resource(crate::persistence::PersistenceSettings::default());
        app.insert_resource(crate::qi_physics::WorldQiAccount::default());
        app.add_event::<CommandResultEvent<ReviveCmd>>();
        app.add_event::<CultivationReviveRequested>();
        app.add_event::<crate::cultivation::death_hooks::PlayerRevived>();
        app.add_event::<crate::cultivation::tribulation::AscensionQuotaOpened>();
        app.add_event::<crate::skill::events::SkillCapChanged>();
        app.add_event::<crate::qi_physics::QiTransfer>();
        app.add_systems(
            Update,
            (
                handle_revive
                    .before(crate::cultivation::death_hooks::on_cultivation_revive_requested),
                crate::cultivation::death_hooks::on_cultivation_revive_requested,
            ),
        );
        app
    }

    #[test]
    fn revive_self_runs_request_handler_and_emits_completed_notification_same_update() {
        let mut app = setup_integration_app();
        let mut lifecycle = Lifecycle::default();
        lifecycle.terminate(10);
        let player = spawn_player(
            &mut app,
            lifecycle,
            Wounds {
                health_current: 0.0,
                ..Default::default()
            },
        );
        app.world_mut().entity_mut(player).insert((
            crate::cultivation::components::Cultivation {
                realm: crate::cultivation::components::Realm::Induce,
                qi_current: 8.0,
                qi_max: 24.0,
                ..Default::default()
            },
            crate::cultivation::components::MeridianSystem::default(),
            crate::cultivation::components::Contamination::default(),
            crate::cultivation::life_record::LifeRecord::new(
                crate::player::state::canonical_player_id("Alice"),
            ),
        ));

        send(&mut app, player);
        run_update(&mut app);

        assert_eq!(
            app.world().get::<Lifecycle>(player).unwrap().state,
            LifecycleState::Alive
        );
        let cultivation = app
            .world()
            .get::<crate::cultivation::components::Cultivation>(player)
            .expect("dev revive should retain cultivation");
        assert_eq!(
            cultivation.realm,
            crate::cultivation::components::Realm::Awaken
        );
        assert_eq!(cultivation.qi_current, 0.0);
        assert_eq!(
            app.world()
                .resource::<Events<CultivationReviveRequested>>()
                .len(),
            1,
            "the request remains in its event buffer after the same-frame reader consumes it"
        );
        assert_eq!(
            app.world()
                .resource::<Events<crate::cultivation::death_hooks::PlayerRevived>>()
                .len(),
            1,
            "the completed notification must be emitted in the command update"
        );
        assert!(matches!(
            app.world()
                .get::<crate::cultivation::life_record::LifeRecord>(player)
                .unwrap()
                .biography
                .last(),
            Some(crate::cultivation::life_record::BiographyEntry::Rebirth { tick: 88, .. })
        ));
    }

    #[test]
    fn revive_self_emits_cultivation_request_and_restores_alive_lifecycle() {
        let mut app = setup_app();
        let mut lifecycle = Lifecycle::default();
        lifecycle.terminate(10);
        let wounds = Wounds {
            health_current: 0.0,
            ..Default::default()
        };
        let player = spawn_player(&mut app, lifecycle, wounds);

        send(&mut app, player);
        run_update(&mut app);

        assert_eq!(
            app.world().get::<Lifecycle>(player).unwrap().state,
            LifecycleState::Alive
        );
        assert_eq!(
            app.world()
                .get::<Lifecycle>(player)
                .unwrap()
                .last_revive_tick,
            Some(88)
        );
        assert_eq!(
            app.world().get::<Wounds>(player).unwrap().health_current,
            app.world().get::<Wounds>(player).unwrap().health_max
        );
        assert_eq!(
            app.world()
                .resource::<Events<CultivationReviveRequested>>()
                .len(),
            1
        );
    }

    #[test]
    fn revive_self_is_noop_for_alive_player() {
        let mut app = setup_app();
        let player = spawn_player(&mut app, Lifecycle::default(), Wounds::default());

        send(&mut app, player);
        run_update(&mut app);

        assert_eq!(
            app.world()
                .resource::<Events<CultivationReviveRequested>>()
                .len(),
            0
        );
        assert_eq!(
            app.world()
                .get::<Lifecycle>(player)
                .unwrap()
                .last_revive_tick,
            None
        );
    }
}
