use serde_json::json;
use valence::prelude::{
    Client, DVec3, Entity, EventReader, EventWriter, Position, Query, Res, ResMut, Username, With,
};

use crate::combat::CombatClock;
use crate::combat::{
    components::{Stamina, StaminaState, Wound, Wounds},
    events::{AttackIntent, CombatEvent, DeathEvent},
};
use crate::cultivation::components::{ColorKind, ContamSource, Contamination, MeridianSystem};
use crate::npc::brain::canonical_npc_id;
use crate::npc::spawn::NpcMarker;
use crate::player::state::canonical_player_id;
use crate::schema::common::GameEventType;
use crate::schema::world_state::GameEvent;
use crate::world::events::ActiveEventsResource;

const DEBUG_ATTACK_DAMAGE_FACTOR: f32 = 0.5;
const DEBUG_ATTACK_STAMINA_COST: f32 = 12.0;
const DEBUG_ATTACK_QI_THROUGHPUT_GAIN: f64 = 6.0;
const DEBUG_ATTACK_CONTAMINATION_FACTOR: f64 = 0.25;
const ENTITY_TARGET_ATTACK_HEALTH_HINT: f32 = 20.0;

pub fn resolve_attack_intents(
    clock: Res<CombatClock>,
    mut intents: EventReader<AttackIntent>,
    mut active_events: Option<ResMut<ActiveEventsResource>>,
    clients: Query<
        (
            Entity,
            &Position,
            &Username,
            &crate::player::state::PlayerState,
        ),
        With<Client>,
    >,
    positions: Query<&Position>,
    npc_markers: Query<(), With<NpcMarker>>,
    npc_positions: Query<(Entity, &Position), With<NpcMarker>>,
    mut combat_targets: Query<(
        &mut Wounds,
        &mut Stamina,
        &mut Contamination,
        &mut MeridianSystem,
    )>,
    mut out_events: EventWriter<CombatEvent>,
    mut death_events: EventWriter<DeathEvent>,
) {
    for intent in intents.read() {
        let Some((
            attacker_position,
            attacker_id,
            target_entity,
            target_position,
            target_hint_qi_max,
            target_id,
            target_health_hint,
        )) = resolve_intent_entities(intent, &clients, &positions, &npc_markers, &npc_positions)
        else {
            continue;
        };

        let distance = attacker_position.distance(target_position) as f32;
        if distance > intent.reach {
            continue;
        }

        let Ok((mut wounds, mut stamina, mut contamination, mut meridians)) =
            combat_targets.get_mut(target_entity)
        else {
            continue;
        };

        let decay = ((intent.reach - distance) / intent.reach.max(0.001)).clamp(0.0, 1.0);
        let hinted_health = target_health_hint.max(0.0);
        if hinted_health <= 0.0 {
            continue;
        }
        let damage = (hinted_health * DEBUG_ATTACK_DAMAGE_FACTOR * decay).max(1.0);
        let was_alive = wounds.health_current > 0.0;

        wounds.health_current = (wounds.health_current - damage).clamp(0.0, wounds.health_max);
        wounds.entries.push(Wound {
            severity: damage,
            bleeding_per_sec: damage * 0.05,
            created_at_tick: clock.tick,
            inflicted_by: Some(attacker_id.clone()),
        });

        stamina.current =
            (stamina.current - DEBUG_ATTACK_STAMINA_COST * decay).clamp(0.0, stamina.max);
        stamina.last_drain_tick = Some(clock.tick);
        stamina.state = if stamina.current <= 0.0 {
            StaminaState::Exhausted
        } else {
            StaminaState::Combat
        };

        contamination.entries.push(ContamSource {
            amount: f64::from(damage) * DEBUG_ATTACK_CONTAMINATION_FACTOR,
            color: ColorKind::Mellow,
            attacker_id: Some(attacker_id.clone()),
            introduced_at: clock.tick,
        });

        if let Some(primary_meridian) = first_open_or_fallback_meridian(&mut meridians) {
            primary_meridian.throughput_current +=
                DEBUG_ATTACK_QI_THROUGHPUT_GAIN * f64::from(decay);
        }

        let action_label = if intent.debug_command.is_some() {
            "debug_attack_intent"
        } else {
            "attack_intent"
        };
        let description = format!(
            "{} {} -> {} dealt {:.1} damage at {:.2} reach decay",
            action_label, attacker_id, target_id, damage, decay
        );

        out_events.send(CombatEvent {
            attacker: intent.attacker,
            target: target_entity,
            resolved_at_tick: clock.tick,
            description,
        });

        if let Some(active_events) = active_events.as_deref_mut() {
            active_events.record_recent_event(GameEvent {
                event_type: GameEventType::EventTriggered,
                tick: clock.tick,
                player: Some(attacker_id.clone()),
                target: Some(target_id),
                zone: None,
                details: Some(std::collections::HashMap::from([
                    ("action".to_string(), json!(action_label)),
                    ("damage".to_string(), json!(damage)),
                    ("reach_decay".to_string(), json!(decay)),
                    ("target_health_hint".to_string(), json!(target_health_hint)),
                    ("target_qi_max_hint".to_string(), json!(target_hint_qi_max)),
                ])),
            });
        }

        if was_alive && wounds.health_current <= 0.0 {
            death_events.send(DeathEvent {
                target: target_entity,
                cause: format!("{action_label}:{attacker_id}"),
                at_tick: clock.tick,
            });
        }
    }
}

type ResolvedIntent = (DVec3, String, Entity, DVec3, f64, String, f32);

fn resolve_intent_entities(
    intent: &AttackIntent,
    clients: &Query<
        (
            Entity,
            &Position,
            &Username,
            &crate::player::state::PlayerState,
        ),
        With<Client>,
    >,
    positions: &Query<&Position>,
    npc_markers: &Query<(), With<NpcMarker>>,
    npc_positions: &Query<(Entity, &Position), With<NpcMarker>>,
) -> Option<ResolvedIntent> {
    let (attacker_position, attacker_id) =
        resolve_combat_actor(intent.attacker, clients, positions, npc_markers)?;

    if let Some(action) = intent.debug_command.as_ref() {
        let (target_entity, target_position, target_hint_qi_max, target_id) = resolve_debug_target(
            intent,
            action,
            clients,
            positions,
            npc_markers,
            npc_positions,
        )?;
        return Some((
            attacker_position,
            attacker_id,
            target_entity,
            target_position,
            target_hint_qi_max,
            target_id,
            action.target_health.max(0.0) as f32,
        ));
    }

    let target_entity = intent.target?;
    if target_entity == intent.attacker {
        return None;
    }
    let (target_position, target_id) =
        resolve_combat_actor(target_entity, clients, positions, npc_markers)?;
    let target_hint_qi_max = clients
        .get(target_entity)
        .map(|(_, _, _, state)| state.spirit_qi_max)
        .unwrap_or(0.0);

    Some((
        attacker_position,
        attacker_id,
        target_entity,
        target_position,
        target_hint_qi_max,
        target_id,
        ENTITY_TARGET_ATTACK_HEALTH_HINT,
    ))
}

fn resolve_combat_actor(
    entity: Entity,
    clients: &Query<
        (
            Entity,
            &Position,
            &Username,
            &crate::player::state::PlayerState,
        ),
        With<Client>,
    >,
    positions: &Query<&Position>,
    npc_markers: &Query<(), With<NpcMarker>>,
) -> Option<(DVec3, String)> {
    if let Ok((_, position, username, _)) = clients.get(entity) {
        return Some((position.get(), canonical_player_id(username.0.as_str())));
    }
    if npc_markers.get(entity).is_ok() {
        let position = positions.get(entity).ok()?.get();
        return Some((position, canonical_npc_id(entity)));
    }
    None
}

fn resolve_debug_target<'a>(
    intent: &AttackIntent,
    action: &crate::player::gameplay::CombatAction,
    clients: &Query<
        (
            Entity,
            &Position,
            &Username,
            &crate::player::state::PlayerState,
        ),
        With<Client>,
    >,
    positions: &Query<&Position>,
    npc_markers: &Query<(), With<NpcMarker>>,
    npc_positions: &Query<(Entity, &Position), With<NpcMarker>>,
) -> Option<(Entity, DVec3, f64, String)> {
    if let Some(target) = intent.target {
        if let Ok((_, position, username, player_state)) = clients.get(target) {
            return Some((
                target,
                position.get(),
                player_state.spirit_qi_max,
                canonical_player_id(username.0.as_str()),
            ));
        }

        if npc_markers.get(target).is_ok() {
            let position = positions.get(target).ok()?.get();
            return Some((target, position, 0.0, canonical_npc_id(target)));
        }

        return None;
    }

    let target_name = action.target.trim();
    if target_name.is_empty() || action.target_health <= 0.0 {
        return None;
    }

    if let Some(player_match) =
        clients
            .iter()
            .find_map(|(entity, position, username, player_state)| {
                if entity == intent.attacker {
                    return None;
                }

                let canonical = canonical_player_id(username.0.as_str());
                (username.0.eq_ignore_ascii_case(target_name)
                    || canonical.eq_ignore_ascii_case(target_name))
                .then_some((
                    entity,
                    position.get(),
                    player_state.spirit_qi_max,
                    canonical,
                ))
            })
    {
        return Some(player_match);
    }

    npc_positions.iter().find_map(|(entity, position)| {
        if entity == intent.attacker {
            return None;
        }

        let canonical = canonical_npc_id(entity);
        canonical.eq_ignore_ascii_case(target_name).then_some((
            entity,
            position.get(),
            0.0,
            canonical,
        ))
    })
}

fn first_open_or_fallback_meridian(
    meridians: &mut MeridianSystem,
) -> Option<&mut crate::cultivation::components::Meridian> {
    if let Some(index) = meridians
        .regular
        .iter()
        .position(|meridian| meridian.opened)
    {
        return meridians.regular.get_mut(index);
    }

    meridians.regular.get_mut(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::combat::components::{CombatState, DerivedAttrs, Lifecycle, Wounds};
    use crate::combat::events::AttackIntent;
    use crate::cultivation::components::{Contamination, Cultivation, MeridianId, MeridianSystem};
    use crate::npc::brain::canonical_npc_id;
    use crate::npc::spawn::{spawn_test_npc_runtime_shape, NpcMarker};
    use crate::player::state::PlayerState;
    use valence::prelude::{
        bevy_ecs, App, Entity, Events, IntoSystemConfigs, Position, Resource, Update,
    };
    use valence::testing::create_mock_client;

    #[derive(Clone, Copy, Resource)]
    struct TestLayer(Entity);

    fn setup_test_layer(mut commands: valence::prelude::Commands) {
        let layer = commands.spawn_empty().id();
        commands.insert_resource(TestLayer(layer));
    }

    fn spawn_runtime_npc(
        mut commands: valence::prelude::Commands,
        layer: valence::prelude::Res<TestLayer>,
    ) {
        spawn_test_npc_runtime_shape(&mut commands, layer.0);
    }

    fn spawn_player(
        app: &mut App,
        username: &str,
        position: [f64; 3],
        wounds: Wounds,
        stamina: Stamina,
    ) -> Entity {
        let (mut client_bundle, _helper) = create_mock_client(username);
        client_bundle.player.position = Position::new(position);
        app.world_mut()
            .spawn((
                client_bundle,
                PlayerState {
                    realm: "qi_refining_1".to_string(),
                    spirit_qi: 60.0,
                    spirit_qi_max: 100.0,
                    karma: 0.0,
                    experience: 0,
                    inventory_score: 0.0,
                },
                Cultivation::default(),
                MeridianSystem::default(),
                Contamination::default(),
                wounds,
                stamina,
                CombatState::default(),
                DerivedAttrs::default(),
                Lifecycle {
                    character_id: canonical_player_id(username),
                    ..Default::default()
                },
            ))
            .id()
    }

    fn spawn_npc(app: &mut App, position: [f64; 3], wounds: Wounds, stamina: Stamina) -> Entity {
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new(position),
                Cultivation::default(),
                MeridianSystem::default(),
                Contamination::default(),
                wounds,
                stamina,
                CombatState::default(),
                DerivedAttrs::default(),
            ))
            .id();
        app.world_mut().entity_mut(entity).insert(Lifecycle {
            character_id: canonical_npc_id(entity),
            ..Default::default()
        });
        entity
    }

    #[test]
    fn resolve_debug_attack_applies_damage_contamination_throughput_and_death() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 12 });
        app.add_event::<AttackIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let mut target_meridians = MeridianSystem::default();
        target_meridians.get_mut(MeridianId::Lung).opened = true;
        let target = spawn_player(
            &mut app,
            "Crimson",
            [2.0, 64.0, 0.0],
            Wounds {
                health_current: 8.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Stamina::default(),
        );
        app.world_mut().entity_mut(target).insert(target_meridians);

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: None,
            issued_at_tick: 11,
            reach: 3.5,
            debug_command: Some(crate::player::gameplay::CombatAction {
                target: "Crimson".to_string(),
                target_health: 40.0,
            }),
        });

        app.update();

        let target_ref = app.world().entity(target);
        let wounds = target_ref
            .get::<Wounds>()
            .expect("target should keep wounds");
        let stamina = target_ref
            .get::<Stamina>()
            .expect("target should keep stamina");
        let contamination = target_ref
            .get::<Contamination>()
            .expect("target should keep contamination");
        let meridians = target_ref
            .get::<MeridianSystem>()
            .expect("target should keep meridians");

        assert!(
            wounds.health_current <= 0.0,
            "damage should reduce health to zero"
        );
        assert_eq!(wounds.entries.len(), 1, "damage should record one wound");
        assert!(
            stamina.current < stamina.max,
            "damage should consume stamina"
        );
        assert_eq!(stamina.state, StaminaState::Combat);
        assert_eq!(
            contamination.entries.len(),
            1,
            "valid attack should write contamination"
        );
        assert_eq!(
            contamination.entries[0].attacker_id.as_deref(),
            Some("offline:Azure")
        );
        assert!(
            meridians.get(MeridianId::Lung).throughput_current > 0.0,
            "valid attack should add meridian throughput"
        );

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let death_events = app.world().resource::<Events<DeathEvent>>();
        assert!(
            !combat_events.is_empty(),
            "resolver should emit CombatEvent"
        );
        assert!(
            !death_events.is_empty(),
            "lethal attack should emit DeathEvent"
        );
    }

    #[test]
    fn invalid_debug_attacks_have_no_side_effects() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 3 });
        app.add_event::<AttackIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [20.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        for action in [
            crate::player::gameplay::CombatAction {
                target: "".to_string(),
                target_health: 20.0,
            },
            crate::player::gameplay::CombatAction {
                target: "Crimson".to_string(),
                target_health: 0.0,
            },
            crate::player::gameplay::CombatAction {
                target: "Crimson".to_string(),
                target_health: 20.0,
            },
        ] {
            app.world_mut().send_event(AttackIntent {
                attacker,
                target: None,
                issued_at_tick: 2,
                reach: 3.5,
                debug_command: Some(action),
            });
            app.update();
        }

        let target_ref = app.world().entity(target);
        let wounds = target_ref
            .get::<Wounds>()
            .expect("target should keep wounds");
        let stamina = target_ref
            .get::<Stamina>()
            .expect("target should keep stamina");
        let contamination = target_ref
            .get::<Contamination>()
            .expect("target should keep contamination");
        let meridians = target_ref
            .get::<MeridianSystem>()
            .expect("target should keep meridians");

        assert_eq!(wounds.health_current, wounds.health_max);
        assert!(
            wounds.entries.is_empty(),
            "invalid attacks must not create wounds"
        );
        assert_eq!(stamina.current, stamina.max);
        assert!(
            contamination.entries.is_empty(),
            "invalid attacks must not contaminate"
        );
        assert_eq!(meridians.get(MeridianId::Lung).throughput_current, 0.0);

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let death_events = app.world().resource::<Events<DeathEvent>>();
        assert!(
            combat_events.is_empty(),
            "invalid attacks must not emit CombatEvent"
        );
        assert!(
            death_events.is_empty(),
            "invalid attacks must not emit DeathEvent"
        );
    }

    #[test]
    fn npc_entity_target_attack_intent_flows_through_shared_resolver() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 44 });
        app.add_event::<AttackIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let npc_attacker = spawn_npc(
            &mut app,
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds {
                health_current: 5.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker: npc_attacker,
            target: Some(target),
            issued_at_tick: 43,
            reach: 3.5,
            debug_command: None,
        });

        app.update();

        let target_ref = app.world().entity(target);
        let wounds = target_ref
            .get::<Wounds>()
            .expect("target should keep wounds");
        let contamination = target_ref
            .get::<Contamination>()
            .expect("target should keep contamination");

        assert!(
            wounds.health_current <= 0.0,
            "npc entity-target intent should apply lethal damage"
        );
        assert_eq!(
            wounds.entries.len(),
            1,
            "resolver should append exactly one wound"
        );
        assert_eq!(
            contamination.entries[0].attacker_id.as_deref(),
            Some(canonical_npc_id(npc_attacker).as_str()),
            "npc attacker identity should use canonical_npc_id"
        );

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let death_events = app.world().resource::<Events<DeathEvent>>();
        assert!(
            !combat_events.is_empty(),
            "npc entity-target intent should still emit CombatEvent via shared resolver"
        );
        assert!(
            !death_events.is_empty(),
            "npc entity-target intent should emit DeathEvent when lethal"
        );
    }

    #[test]
    fn player_to_npc_and_npc_to_player_share_same_resolver_path() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 91 });
        app.add_event::<AttackIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let player = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let npc = spawn_npc(
            &mut app,
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker: player,
            target: Some(npc),
            issued_at_tick: 90,
            reach: 3.5,
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: npc,
            target: Some(player),
            issued_at_tick: 90,
            reach: 3.5,
            debug_command: None,
        });

        app.update();

        let player_ref = app.world().entity(player);
        let npc_ref = app.world().entity(npc);
        let player_wounds = player_ref
            .get::<Wounds>()
            .expect("player target should keep wounds");
        let npc_wounds = npc_ref
            .get::<Wounds>()
            .expect("npc target should keep wounds");
        let player_contamination = player_ref
            .get::<Contamination>()
            .expect("player target should keep contamination");
        let npc_contamination = npc_ref
            .get::<Contamination>()
            .expect("npc target should keep contamination");

        assert_eq!(
            player_wounds.entries.len(),
            1,
            "npc->player should resolve exactly one wound"
        );
        assert_eq!(
            npc_wounds.entries.len(),
            1,
            "player->npc should resolve exactly one wound"
        );
        assert_eq!(
            player_contamination.entries[0].attacker_id.as_deref(),
            Some(canonical_npc_id(npc).as_str())
        );
        assert_eq!(
            npc_contamination.entries[0].attacker_id.as_deref(),
            Some("offline:Azure")
        );

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        assert!(
            !combat_events.is_empty(),
            "both directions should emit CombatEvent through the same resolver event family"
        );
    }

    #[test]
    fn player_to_runtime_spawned_zombie_npc_target_resolves_without_dropping_intent() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 128 });
        app.add_event::<AttackIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_systems(
            valence::prelude::Startup,
            (setup_test_layer, spawn_runtime_npc.after(setup_test_layer)),
        );
        app.add_systems(Update, resolve_attack_intents);

        app.update();
        app.update();

        let npc = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query
                .iter(world)
                .next()
                .expect("runtime zombie NPC should be spawned for resolver coverage test")
        };

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [13.0, 66.0, 14.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(npc),
            issued_at_tick: 127,
            reach: 3.5,
            debug_command: None,
        });

        app.update();

        let npc_ref = app.world().entity(npc);
        let npc_wounds = npc_ref
            .get::<Wounds>()
            .expect("runtime zombie NPC should carry Wounds for shared resolver");
        let npc_contamination = npc_ref
            .get::<Contamination>()
            .expect("runtime zombie NPC should carry Contamination for shared resolver");

        assert_eq!(
            npc_wounds.entries.len(),
            1,
            "player->runtime-zombie intent should apply one wound"
        );
        assert_eq!(
            npc_contamination.entries[0].attacker_id.as_deref(),
            Some("offline:Azure"),
            "shared resolver should attribute player attacker on runtime zombie target"
        );

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        assert!(
            !combat_events.is_empty(),
            "player->runtime-zombie intent should emit CombatEvent instead of dropping"
        );
    }

    #[test]
    fn repeated_hits_on_dead_target_emit_single_death_event() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 300 });
        app.add_event::<AttackIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds {
                health_current: 1.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 299,
            reach: 3.5,
            debug_command: None,
        });
        app.update();

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 300,
            reach: 3.5,
            debug_command: None,
        });
        app.update();

        let death_events = app.world().resource::<Events<DeathEvent>>();
        assert_eq!(
            death_events.len(),
            1,
            "DeathEvent should only emit on alive->dead transition, not repeated corpse hits"
        );
    }

    #[test]
    fn debug_attack_resolves_canonical_npc_target_without_client_query_match() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 512 });
        app.add_event::<AttackIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let npc_target = spawn_npc(
            &mut app,
            [2.0, 64.0, 0.0],
            Wounds {
                health_current: 8.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Stamina::default(),
        );
        let npc_id = canonical_npc_id(npc_target);

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: None,
            issued_at_tick: 511,
            reach: 3.5,
            debug_command: Some(crate::player::gameplay::CombatAction {
                target: npc_id.clone(),
                target_health: 40.0,
            }),
        });

        app.update();

        let npc_ref = app.world().entity(npc_target);
        let wounds = npc_ref
            .get::<Wounds>()
            .expect("npc debug target should keep wounds");
        let contamination = npc_ref
            .get::<Contamination>()
            .expect("npc debug target should keep contamination");

        assert!(
            wounds.health_current <= 0.0,
            "debug npc target should receive resolver damage"
        );
        assert_eq!(
            contamination.entries[0].attacker_id.as_deref(),
            Some("offline:Azure"),
            "debug npc target should preserve canonical player attacker identity"
        );

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let death_events = app.world().resource::<Events<DeathEvent>>();
        assert!(
            !combat_events.is_empty(),
            "debug npc target should emit CombatEvent through shared resolver"
        );
        assert!(
            !death_events.is_empty(),
            "lethal debug npc target should emit DeathEvent"
        );
    }
}
