pub mod components;
pub mod debug;
pub mod events;
pub mod raycast;
pub mod resolve;

use valence::prelude::{
    bevy_ecs, Added, App, Client, Commands, IntoSystemConfigs, IntoSystemSetConfigs, Query,
    SystemSet, Update, Username, Without,
};

use crate::npc::brain::canonical_npc_id;
use crate::npc::spawn::NpcMarker;
use crate::player::state::canonical_player_id;

use self::components::{CombatState, DerivedAttrs, Lifecycle, Stamina, Wounds};
use self::events::{AttackIntent, CombatEvent, DeathEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum CombatSystemSet {
    IntentSet,
    PhysicsSet,
    ResolveSet,
    EmitSet,
}

#[derive(Debug, Clone, Default)]
pub struct CombatClock {
    pub tick: u64,
}

impl valence::prelude::Resource for CombatClock {}

type JoinedClientsWithoutCombatBundle<'a> = (valence::prelude::Entity, &'a Username);
type JoinedClientsWithoutCombatBundleFilter = (Added<Client>, Without<Wounds>);

fn attach_combat_bundle_to_joined_clients(
    mut commands: Commands,
    joined_clients: Query<JoinedClientsWithoutCombatBundle<'_>, JoinedClientsWithoutCombatBundleFilter>,
) {
    for (entity, username) in &joined_clients {
        commands.entity(entity).insert((
            Wounds::default(),
            Stamina::default(),
            CombatState::default(),
            DerivedAttrs::default(),
            Lifecycle {
                character_id: canonical_player_id(username.0.as_str()),
                ..Default::default()
            },
        ));
    }
}

type JoinedNpcsWithoutCombatBundle<'a> = valence::prelude::Entity;
type JoinedNpcsWithoutCombatBundleFilter = (Added<NpcMarker>, Without<Wounds>);

fn attach_combat_bundle_to_joined_npcs(
    mut commands: Commands,
    joined_npcs: Query<JoinedNpcsWithoutCombatBundle<'_>, JoinedNpcsWithoutCombatBundleFilter>,
) {
    for entity in &joined_npcs {
        commands.entity(entity).insert((
            Wounds::default(),
            Stamina::default(),
            CombatState::default(),
            DerivedAttrs::default(),
            Lifecycle {
                character_id: canonical_npc_id(entity),
                ..Default::default()
            },
        ));
    }
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][combat] registering combat skeleton systems");

    app.insert_resource(CombatClock::default());
    app.add_event::<AttackIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();

    app.configure_sets(
        Update,
        (
            CombatSystemSet::IntentSet,
            CombatSystemSet::PhysicsSet,
            CombatSystemSet::ResolveSet,
            CombatSystemSet::EmitSet,
        )
            .chain(),
    );

    app.add_systems(
        Update,
        (
            attach_combat_bundle_to_joined_clients.in_set(CombatSystemSet::IntentSet),
            attach_combat_bundle_to_joined_npcs.in_set(CombatSystemSet::IntentSet),
            debug::tick_combat_clock.in_set(CombatSystemSet::IntentSet),
            resolve::resolve_attack_intents.in_set(CombatSystemSet::ResolveSet),
            debug::drain_combat_events_for_debug
                .in_set(CombatSystemSet::EmitSet)
                .after(resolve::resolve_attack_intents),
        ),
    );
}
