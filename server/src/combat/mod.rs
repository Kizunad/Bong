pub mod anticheat;
pub mod armor;
pub mod armor_sync;
pub mod carrier;
pub mod components;
#[cfg(test)]
mod death_event_attacker_chain_test;
pub mod debug;
pub mod decay;
pub mod events;
pub mod jiemai;
pub mod lifecycle;
pub mod projectile;
pub mod raycast;
pub mod resolve;
pub mod status;
pub mod tuike;
pub mod weapon;
pub mod woliu;

use std::path::Path;
use valence::prelude::{
    bevy_ecs, Added, App, Client, Commands, IntoSystemConfigs, IntoSystemSetConfigs, Query,
    SystemSet, Update, Username, Without,
};

#[cfg(test)]
mod tests;

use crate::npc::brain::canonical_npc_id;
use crate::npc::spawn::NpcMarker;
use crate::player::state::{
    canonical_player_id, load_current_character_id, load_player_shrine_anchor_slice,
    player_character_id, PlayerStatePersistence,
};

use self::anticheat::{
    load_anticheat_config, AntiCheatConfig, AntiCheatCounter, AntiCheatViolationEvent,
    DEFAULT_ANTICHEAT_CONFIG_PATH,
};
use self::components::{CombatState, DerivedAttrs, Lifecycle, Stamina, StatusEffects, Wounds};
use self::events::{
    ApplyStatusEffectIntent, AttackIntent, CombatEvent, DeathEvent, DeathInsightRequested,
    DebugCombatCommand, DefenseIntent, RevivalActionIntent,
};

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
    player_persistence: Option<valence::prelude::Res<PlayerStatePersistence>>,
) {
    for (entity, username) in &joined_clients {
        let persistence = player_persistence.as_deref();
        let spawn_anchor = persistence.and_then(|persistence| {
            load_player_shrine_anchor_slice(persistence, username.0.as_str())
                .ok()
                .flatten()
        });
        let character_id = persistence
            .and_then(|persistence| {
                load_current_character_id(persistence, username.0.as_str())
                    .ok()
                    .flatten()
            })
            .map(|current_char_id| player_character_id(username.0.as_str(), &current_char_id))
            .unwrap_or_else(|| canonical_player_id(username.0.as_str()));
        commands.entity(entity).insert((
            Wounds::default(),
            Stamina::default(),
            CombatState::default(),
            StatusEffects::default(),
            DerivedAttrs::default(),
            AntiCheatCounter::default(),
            carrier::CarrierStore::default(),
            Lifecycle {
                character_id,
                spawn_anchor,
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
            carrier::CarrierStore::default(),
            Lifecycle {
                character_id: canonical_npc_id(entity),
                ..Default::default()
            },
        ));
    }
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][combat] registering combat skeleton systems");

    // plan-armor-v1 §1.1：启动期加载护甲 profile 蓝图（template_id -> ArmorProfile）。
    // 失败不 panic: 允许空 registry（未配置护甲数据时不会有减免）。
    let armor_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(armor::DEFAULT_ARMOR_PROFILES_DIR);
    let armor_registry = armor::ArmorProfileRegistry::load_dir(armor_dir).unwrap_or_else(|e| {
        tracing::error!("[bong][combat][armor] armor profile load failed: {e}");
        armor::ArmorProfileRegistry::new()
    });
    tracing::info!(
        "[bong][combat][armor] loaded {} armor profile(s)",
        armor_registry.len()
    );
    app.insert_resource(armor_registry);

    let anticheat_config_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_ANTICHEAT_CONFIG_PATH);
    let anticheat_config = load_anticheat_config(anticheat_config_path).unwrap_or_else(|error| {
        tracing::error!("[bong][anticheat] config load failed, using defaults: {error}");
        AntiCheatConfig::default()
    });
    app.insert_resource(anticheat_config);

    app.insert_resource(CombatClock::default());
    app.add_event::<AttackIntent>();
    app.add_event::<DefenseIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<RevivalActionIntent>();
    app.add_event::<DebugCombatCommand>();
    app.add_event::<AntiCheatViolationEvent>();
    carrier::register(app);
    app.add_event::<tuike::ShedEvent>();
    app.add_event::<tuike::FalseSkinForgeRequest>();

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
    woliu::register(app);

    app.add_systems(
        Update,
        (
            attach_combat_bundle_to_joined_clients
                .after(crate::player::attach_player_state_to_joined_clients)
                .in_set(CombatSystemSet::Intent),
            attach_combat_bundle_to_joined_npcs.in_set(CombatSystemSet::Intent),
            debug::tick_combat_clock.in_set(CombatSystemSet::Intent),
            resolve::apply_defense_intents.in_set(CombatSystemSet::Intent),
            status::status_effect_apply_tick
                .in_set(CombatSystemSet::Intent)
                .after(resolve::apply_defense_intents),
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
            lifecycle::handle_revival_action_intents
                .in_set(CombatSystemSet::Resolve)
                .after(lifecycle::near_death_tick),
            lifecycle::auto_confirm_revival_decisions
                .in_set(CombatSystemSet::Resolve)
                .after(lifecycle::handle_revival_action_intents),
            debug::drain_combat_events_for_debug
                .in_set(CombatSystemSet::Emit)
                .after(resolve::resolve_attack_intents),
            // plan §13 C1 调试命令消费 — 放 Intent 阶段，早于 WoundBleedTick，
            // 使 /health set / /wound add 当 tick 即可被后续 tick 系统感知。
            debug::apply_debug_combat_commands.in_set(CombatSystemSet::Intent),
            // plan-weapon-v1 §2.3: 装备槽 → Weapon component 同步。放 Intent 阶段,
            // 让 resolve 阶段查 Weapon 时已经是当前 tick 的最新装备状态。
            weapon::sync_weapon_component_from_equipped.in_set(CombatSystemSet::Intent),
            // plan-armor-v1 §1.3: 装备槽(四护甲槽) → DerivedAttrs.defense_profile。
            armor_sync::sync_armor_to_derived_attrs.in_set(CombatSystemSet::Intent),
        ),
    );
    // Separate add_systems call to stay below Bevy 0.14 tuple-arity limits.
    app.add_systems(
        Update,
        (
            tuike::handle_false_skin_forge_requests,
            tuike::sync_false_skin_from_inventory,
        )
            .chain()
            .in_set(CombatSystemSet::Intent),
    );
    app.add_systems(
        Update,
        anticheat::emit_anticheat_threshold_reports
            .in_set(CombatSystemSet::Resolve)
            .after(resolve::resolve_attack_intents),
    );
}
