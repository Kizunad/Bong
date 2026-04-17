pub mod components;
pub mod debug;
pub mod events;
pub mod lifecycle;
pub mod raycast;
pub mod resolve;
pub mod status;
pub mod weapon;

use valence::prelude::{
    bevy_ecs, Added, App, Client, Commands, IntoSystemConfigs, IntoSystemSetConfigs, Query,
    SystemSet, Update, Username, Without,
};

use crate::npc::brain::canonical_npc_id;
use crate::npc::spawn::NpcMarker;
use crate::player::state::canonical_player_id;

use self::components::{CombatState, DerivedAttrs, Lifecycle, Stamina, StatusEffects, Wounds};
use self::events::{ApplyStatusEffectIntent, AttackIntent, CombatEvent, DeathEvent, DefenseIntent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum CombatSystemSet {
    Intent,
    Physics,
    Resolve,
    Emit,
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
    joined_clients: Query<
        JoinedClientsWithoutCombatBundle<'_>,
        JoinedClientsWithoutCombatBundleFilter,
    >,
) {
    for (entity, username) in &joined_clients {
        commands.entity(entity).insert((
            Wounds::default(),
            Stamina::default(),
            CombatState::default(),
            StatusEffects::default(),
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
            StatusEffects::default(),
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
    app.add_event::<DefenseIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();

    app.configure_sets(
        Update,
        (
            CombatSystemSet::Intent,
            CombatSystemSet::Physics,
            CombatSystemSet::Resolve,
            CombatSystemSet::Emit,
        )
            .chain(),
    );

    app.add_systems(
        Update,
        (
            attach_combat_bundle_to_joined_clients.in_set(CombatSystemSet::Intent),
            attach_combat_bundle_to_joined_npcs.in_set(CombatSystemSet::Intent),
            debug::tick_combat_clock.in_set(CombatSystemSet::Intent),
            resolve::apply_defense_intents.in_set(CombatSystemSet::Intent),
            status::status_effect_apply_tick.in_set(CombatSystemSet::Intent),
            lifecycle::wound_bleed_tick.in_set(CombatSystemSet::Physics),
            lifecycle::stamina_tick.in_set(CombatSystemSet::Physics),
            lifecycle::combat_state_tick.in_set(CombatSystemSet::Physics),
            status::status_effect_tick.in_set(CombatSystemSet::Physics),
            status::attribute_aggregate_tick.in_set(CombatSystemSet::Physics),
            resolve::resolve_attack_intents.in_set(CombatSystemSet::Resolve),
            lifecycle::sync_combat_state_from_events
                .in_set(CombatSystemSet::Resolve)
                .after(resolve::resolve_attack_intents),
            lifecycle::death_arbiter_tick
                .in_set(CombatSystemSet::Resolve)
                .after(resolve::resolve_attack_intents),
            lifecycle::near_death_tick
                .in_set(CombatSystemSet::Resolve)
                .after(lifecycle::death_arbiter_tick),
            debug::drain_combat_events_for_debug
                .in_set(CombatSystemSet::Emit)
                .after(resolve::resolve_attack_intents),
            // plan-weapon-v1 §2.3: 装备槽 → Weapon component 同步。放 Intent 阶段,
            // 让 resolve 阶段查 Weapon 时已经是当前 tick 的最新装备状态。
            weapon::sync_weapon_component_from_equipped.in_set(CombatSystemSet::Intent),
        ),
    );
}
