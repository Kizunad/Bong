use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{FirstToScore, Thinker, ThinkerBuilder};
use valence::entity::zombie::ZombieEntityBundle;
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Despawned, Entity, EntityKind, EntityLayerId,
    Events, Position, Query, ResMut, Resource, Update, With,
};

use crate::cultivation::components::{ActorQiIdentity, ActorQiKind, Cultivation, Realm};
use crate::cultivation::life_record::LifeRecord;
use crate::npc::brain::{
    ChaseAction, ChaseTargetScorer, DashAction, DashScorer, FleeAction, MeleeAttackAction,
    MeleeRangeScorer, PlayerProximityScorer, PROXIMITY_THRESHOLD,
};
use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};
use crate::npc::movement::{MovementCapabilities, MovementController, MovementCooldowns};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{
    DuelTarget, NpcBlackboard, NpcCombatLoadout, NpcMarker, NpcMeleeArchetype,
};
use crate::qi_physics::{QiTransfer, WorldQiAccount};
use crate::world::dimension::CurrentDimension;
use crate::world::zone::ZoneRegistry;
use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

const PASSIVE_TARGET_HEALTH: f32 = 32.0;

/// Marker component for NPCs spawned by the `/npc_scenario` command.
/// Used for bulk cleanup on `/npc_scenario clear`.
#[derive(Clone, Copy, Debug, Component)]
pub struct ScenarioNpc;

/// Production contract for `/npc_scenario passive_target` entities.
///
/// A passive target may receive normal combat damage and feedback, but it never
/// owns movement. Movement, knockback, and navigation systems all consume this
/// marker so the scenario remains stationary even when hit by real combat code.
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct PassiveTarget;

/// Scenario types available via `/npc_scenario`.
#[derive(Clone, Copy, Debug)]
pub enum ScenarioType {
    /// NPC chases the nearest player.
    Chase,
    /// NPC flees from the nearest player (default brain).
    Flee,
    /// NPC chases then attacks in melee range.
    Fight,
    /// NPC maintains distance: flees when close, chases when far.
    Kite,
    /// 3 NPCs all chase + fight the player.
    Swarm,
    /// 2 NPCs fight each other for observation.
    Duel,
    /// Stationary non-retaliating NPC for deterministic protocol Bot combat evidence.
    PassiveTarget,
    /// Despawn all scenario NPCs.
    Clear,
}

impl ScenarioType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "chase" => Some(Self::Chase),
            "flee" => Some(Self::Flee),
            "fight" => Some(Self::Fight),
            "kite" => Some(Self::Kite),
            "swarm" => Some(Self::Swarm),
            "duel" => Some(Self::Duel),
            "passive_target" => Some(Self::PassiveTarget),
            "clear" => Some(Self::Clear),
            _ => None,
        }
    }
}

/// Resource that queues a scenario spawn request from the chat command.
#[derive(Default)]
pub struct PendingScenario {
    pub request: Option<(ScenarioType, DVec3)>,
}

impl Resource for PendingScenario {}

pub fn register(app: &mut App) {
    app.insert_resource(PendingScenario::default())
        .add_systems(Update, process_pending_scenarios);
}

#[allow(clippy::type_complexity)]
fn process_pending_scenarios(
    mut commands: Commands,
    mut pending: ResMut<PendingScenario>,
    layers: Query<Entity, With<crate::world::dimension::OverworldLayer>>,
    mut scenario_npcs: Query<
        (
            Entity,
            Option<&mut Cultivation>,
            Option<&Position>,
            Option<&CurrentDimension>,
            Option<&LifeRecord>,
        ),
        With<ScenarioNpc>,
    >,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut qi_ledger: Option<ResMut<WorldQiAccount>>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
) {
    let Some((scenario, player_pos)) = pending.request.take() else {
        return;
    };

    let Ok(layer) = layers.get_single() else {
        tracing::warn!("[bong][npc] no layer found for scenario spawn");
        return;
    };

    // Always clear existing scenario NPCs first.
    for (entity, mut cultivation, position, dimension, life_record) in &mut scenario_npcs {
        if let Some(cultivation) = cultivation.as_deref_mut() {
            let amount = cultivation.qi_current;
            if amount > f64::EPSILON {
                let Some(life_record) = life_record else {
                    tracing::warn!(?entity, "[bong][npc] refusing to clear scenario NPC without LifeRecord for qi release");
                    continue;
                };
                let Some(ledger) = qi_ledger.as_deref_mut() else {
                    tracing::warn!(
                        ?entity,
                        "[bong][npc] refusing to clear scenario NPC without qi ledger"
                    );
                    continue;
                };
                let Ok(actor) = ActorQiIdentity::from_life_record(life_record, ActorQiKind::Npc)
                else {
                    tracing::warn!(
                        ?entity,
                        "[bong][npc] refusing to clear scenario NPC with invalid qi identity"
                    );
                    continue;
                };
                let zone_name = position.zip(dimension).and_then(|(position, dimension)| {
                    zones
                        .as_deref()
                        .and_then(|zones| zones.find_zone(dimension.0, position.get()))
                        .map(|zone| zone.name.clone())
                });
                let zone = zone_name.as_deref().and_then(|name| {
                    zones
                        .as_deref_mut()
                        .and_then(|zones| zones.find_zone_mut(name))
                });
                let Ok(outcome) = cultivation.release_to_zone(
                    zone,
                    ledger,
                    &actor,
                    amount,
                    crate::qi_physics::QiTransferReason::ReleaseToZone,
                ) else {
                    tracing::warn!(
                        ?entity,
                        "[bong][npc] refusing to clear scenario NPC after qi release failed"
                    );
                    continue;
                };
                if let Some(events) = qi_transfers.as_deref_mut() {
                    for transfer in outcome.transfers {
                        events.send(transfer);
                    }
                }
            }
        }
        commands.entity(entity).insert(Despawned);
    }

    if matches!(scenario, ScenarioType::Clear) {
        tracing::info!("[bong][npc] cleared all scenario NPCs");
        return;
    }

    let spawn_count = match scenario {
        ScenarioType::Swarm => 4,
        ScenarioType::Duel => 2,
        _ => 1,
    };

    let mut spawned_entities = Vec::new();

    for i in 0..spawn_count {
        let offset = if matches!(scenario, ScenarioType::PassiveTarget) {
            // Keep the target inside the real player melee reach so the dev command
            // can be exercised immediately by a protocol client.
            DVec3::new(1.0, 0.0, 0.0)
        } else {
            scenario_offset(i, spawn_count)
        };
        let spawn_pos = player_pos + offset;

        let loadout = scenario_combat_loadout(&scenario, i);
        let melee_archetype = loadout.melee_archetype;
        let melee_profile = loadout.melee_profile();
        let movement_capabilities = MovementCapabilities {
            can_sprint: loadout.movement_capabilities.can_sprint,
            can_dash: loadout.movement_capabilities.can_dash,
        };

        let entity = commands
            .spawn((
                ZombieEntityBundle {
                    kind: EntityKind::ZOMBIE,
                    layer: EntityLayerId(layer),
                    position: Position::new([spawn_pos.x, spawn_pos.y, spawn_pos.z]),
                    ..Default::default()
                },
                Transform::from_xyz(spawn_pos.x as f32, spawn_pos.y as f32, spawn_pos.z as f32),
                GlobalTransform::default(),
                NpcMarker,
                NpcBlackboard::default(),
                loadout,
                melee_archetype,
                melee_profile,
                ScenarioNpc,
            ))
            .id();

        if matches!(scenario, ScenarioType::PassiveTarget) {
            commands.entity(entity).insert(PassiveTarget);
        }

        let mut runtime = npc_runtime_bundle(entity, NpcArchetype::Zombie, Realm::Awaken);
        if matches!(scenario, ScenarioType::PassiveTarget) {
            runtime.wounds.health_current = PASSIVE_TARGET_HEALTH;
            runtime.wounds.health_max = PASSIVE_TARGET_HEALTH;
        }
        let mut entity_commands = commands.entity(entity);
        entity_commands
            .insert((
                NpcArchetype::Zombie,
                Navigator::new(),
                MovementController::new(),
                movement_capabilities,
                MovementCooldowns::default(),
            ))
            .insert(runtime);
        if !matches!(scenario, ScenarioType::PassiveTarget) {
            entity_commands
                .insert(NpcPatrol::new(
                    DEFAULT_SPAWN_ZONE_NAME,
                    DVec3::new(spawn_pos.x, spawn_pos.y, spawn_pos.z),
                ))
                .insert(build_thinker(&scenario));
        }

        spawned_entities.push(entity);
    }

    // Cross-link duel targets so they fight each other instead of a player.
    if matches!(scenario, ScenarioType::Duel) && spawned_entities.len() == 2 {
        commands
            .entity(spawned_entities[0])
            .insert(DuelTarget(spawned_entities[1]));
        commands
            .entity(spawned_entities[1])
            .insert(DuelTarget(spawned_entities[0]));
    }

    tracing::info!("[bong][npc] spawned {spawn_count} scenario NPC(s) ({scenario:?}) near player");
}

/// Spread NPCs in a circle ~12 blocks from the player.
fn scenario_offset(index: usize, total: usize) -> DVec3 {
    let angle = std::f64::consts::TAU * (index as f64) / (total as f64);
    DVec3::new(angle.cos() * 12.0, 0.0, angle.sin() * 12.0)
}

fn build_thinker(scenario: &ScenarioType) -> ThinkerBuilder {
    match scenario {
        ScenarioType::Chase => Thinker::build()
            .picker(FirstToScore { threshold: 0.05 })
            .when(ChaseTargetScorer, ChaseAction),

        ScenarioType::Flee => Thinker::build()
            .picker(FirstToScore {
                threshold: PROXIMITY_THRESHOLD,
            })
            .when(PlayerProximityScorer, FleeAction),

        ScenarioType::Fight | ScenarioType::Swarm | ScenarioType::Duel => Thinker::build()
            .picker(FirstToScore { threshold: 0.05 })
            .when(MeleeRangeScorer, MeleeAttackAction)
            .when(DashScorer, DashAction)
            .when(ChaseTargetScorer, ChaseAction),

        ScenarioType::Kite => Thinker::build()
            .picker(FirstToScore { threshold: 0.05 })
            .when(PlayerProximityScorer, FleeAction)
            .when(ChaseTargetScorer, ChaseAction),

        ScenarioType::Clear | ScenarioType::PassiveTarget => {
            // Clear is handled before we get here, but provide a default.
            Thinker::build()
                .picker(FirstToScore { threshold: 0.8 })
                .when(PlayerProximityScorer, FleeAction)
        }
    }
}

fn scenario_combat_loadout(scenario: &ScenarioType, index: usize) -> NpcCombatLoadout {
    match scenario {
        ScenarioType::Fight => NpcCombatLoadout::fighter(NpcMeleeArchetype::Sword),
        ScenarioType::Swarm => {
            if index.is_multiple_of(2) {
                NpcCombatLoadout::fighter(NpcMeleeArchetype::Brawler)
            } else {
                NpcCombatLoadout::fighter(NpcMeleeArchetype::Sword)
            }
        }
        ScenarioType::Duel => {
            if index == 0 {
                NpcCombatLoadout::fighter(NpcMeleeArchetype::Spear)
            } else {
                NpcCombatLoadout::fighter(NpcMeleeArchetype::Sword)
            }
        }
        ScenarioType::PassiveTarget => NpcCombatLoadout::new(
            NpcMeleeArchetype::Brawler,
            MovementCapabilities {
                can_sprint: false,
                can_dash: false,
            },
        ),
        _ => NpcCombatLoadout::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::cmd::dev::npc_scenario::{handle_npc_scenario, NpcScenarioAction, NpcScenarioCmd};
    use crate::combat::components::{Lifecycle, Stamina, StatusEffects, Wounds};
    use crate::combat::events::{AttackIntent, CombatEvent, DeathEvent};
    use crate::combat::lifecycle::death_arbiter_tick;
    use crate::combat::player_attack::{handle_player_attack, PlayerAttackCooldown};
    use crate::combat::resolve::resolve_attack_intents;
    use crate::combat::CombatClock;
    use crate::cultivation::components::{Contamination, Cultivation, MeridianSystem};
    use crate::cultivation::death_hooks::PlayerTerminated;
    use crate::inventory::InventoryDurabilityChangedEvent;
    use crate::network::combat_event_emit::emit_combat_event_to_client;
    use crate::npc::brain::canonical_npc_id;
    use crate::npc::lifecycle::NpcLifespan;
    use crate::npc::lifecycle::{NpcTerminalSettlementSucceeded, NpcTerminalSystemSet};
    use crate::npc::movement::PendingKnockback;
    use crate::npc::spawn::{NpcCombatLoadout, NpcMeleeProfile};
    use crate::persistence::{bootstrap_sqlite, PersistenceSettings};
    use crate::player::state::{canonical_player_id, PlayerState};
    use crate::qi_physics::WorldQiAccount;
    use big_brain::prelude::ThinkerBuilder;
    use std::time::{SystemTime, UNIX_EPOCH};
    use valence::command::handler::CommandResultEvent;
    use valence::prelude::{
        Client, Entity, EntityInteraction, Events, FixedUpdate, GameMode, InteractEntityEvent,
        IntoSystemConfigs, Position, Update, With,
    };
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::ScenarioSingleClient;

    #[test]
    fn scenario_spawned_npcs_include_shared_combat_target_components() {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.insert_resource(PendingScenario {
            request: Some((ScenarioType::Duel, DVec3::new(8.0, 66.0, 8.0))),
        });
        app.add_systems(Update, process_pending_scenarios);

        app.update();

        let scenario_npcs = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<ScenarioNpc>>();
            query.iter(world).collect::<Vec<_>>()
        };

        assert_eq!(
            scenario_npcs.len(),
            2,
            "duel scenario should spawn two NPCs for coverage"
        );

        for npc in scenario_npcs {
            let entity_ref = app.world().entity(npc);
            assert!(
                entity_ref.get::<Cultivation>().is_some(),
                "scenario NPC should include Cultivation for shared resolver"
            );
            assert!(
                entity_ref.get::<MeridianSystem>().is_some(),
                "scenario NPC should include MeridianSystem for shared resolver"
            );
            assert!(
                entity_ref.get::<Contamination>().is_some(),
                "scenario NPC should include Contamination for shared resolver"
            );
            assert!(
                entity_ref.get::<Wounds>().is_some(),
                "scenario NPC should include Wounds for shared resolver"
            );
            assert!(
                entity_ref.get::<Stamina>().is_some(),
                "scenario NPC should include Stamina for shared resolver"
            );
            assert!(
                entity_ref.get::<StatusEffects>().is_some(),
                "scenario NPC should include StatusEffects for shared resolver"
            );
            let lifecycle = entity_ref
                .get::<Lifecycle>()
                .expect("scenario NPC should include Lifecycle identity component");
            assert_eq!(
                lifecycle.character_id,
                canonical_npc_id(npc),
                "scenario NPC Lifecycle should use canonical npc identity"
            );
            assert!(
                entity_ref.get::<NpcLifespan>().is_some(),
                "scenario NPC should include shared lifespan component"
            );
        }
    }

    #[test]
    fn clear_releases_scenario_qi_before_marking_entity_despawned() {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.insert_resource(PendingScenario {
            request: Some((ScenarioType::Clear, DVec3::ZERO)),
        });
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<QiTransfer>();
        app.add_systems(Update, process_pending_scenarios);

        let entity = app
            .world_mut()
            .spawn((
                ScenarioNpc,
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension::default(),
                LifeRecord::new("scenario_qi_release"),
                Cultivation {
                    qi_current: 12.0,
                    qi_max: 12.0,
                    ..Cultivation::default()
                },
            ))
            .id();

        app.world_mut().run_schedule(Update);

        assert_eq!(
            app.world().get::<Cultivation>(entity).unwrap().qi_current,
            0.0
        );
        assert!(app.world().get::<Despawned>(entity).is_some());
        let ledger = app.world().resource::<WorldQiAccount>();
        assert_eq!(ledger.transfers().len(), 1);
        assert_eq!(ledger.transfers()[0].amount, 12.0);
        assert_eq!(
            ledger.transfers()[0].reason,
            crate::qi_physics::QiTransferReason::ReleaseToZone
        );
        assert_eq!(
            app.world()
                .resource::<Events<QiTransfer>>()
                .iter_current_update_events()
                .count(),
            1,
            "successful clear settlement must expose the same committed transfer as an event"
        );
    }

    #[test]
    fn clear_fails_closed_when_scenario_qi_cannot_be_settled() {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.insert_resource(PendingScenario {
            request: Some((ScenarioType::Clear, DVec3::ZERO)),
        });
        app.add_systems(Update, process_pending_scenarios);

        let entity = app
            .world_mut()
            .spawn((
                ScenarioNpc,
                LifeRecord::new("scenario_qi_retry"),
                Cultivation {
                    qi_current: 7.0,
                    qi_max: 7.0,
                    ..Cultivation::default()
                },
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Cultivation>(entity).unwrap().qi_current,
            7.0
        );
        assert!(
            app.world().get::<Despawned>(entity).is_none(),
            "missing ledger must preserve the live scenario NPC for a later settlement retry"
        );
    }

    #[test]
    fn passive_target_is_stationary_non_retaliating_real_combat_npc() {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.insert_resource(PendingScenario {
            request: Some((ScenarioType::PassiveTarget, DVec3::new(8.0, 66.0, 8.0))),
        });
        app.add_event::<crate::combat::knockback::KnockbackEvent>();
        app.add_systems(Update, process_pending_scenarios);
        crate::npc::patrol::register(&mut app);
        crate::npc::movement::register(&mut app);
        crate::npc::navigator::register(&mut app);

        app.update();

        let npc = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<ScenarioNpc>>();
            query
                .iter(world)
                .next()
                .expect("passive_target should spawn one scenario NPC")
        };
        let initial_position = {
            let entity_ref = app.world().entity(npc);
            let wounds = entity_ref
                .get::<Wounds>()
                .expect("passive target must use the production combat Wounds component");
            assert_eq!(wounds.health_current, PASSIVE_TARGET_HEALTH);
            assert_eq!(wounds.health_max, PASSIVE_TARGET_HEALTH);
            let movement = entity_ref
                .get::<MovementCapabilities>()
                .expect("passive target must expose explicit movement capabilities");
            assert!(!movement.can_sprint);
            assert!(!movement.can_dash);
            assert!(
                entity_ref.get::<ThinkerBuilder>().is_none(),
                "passive target must not receive a brain that could move or retaliate"
            );
            assert!(
                entity_ref.get::<NpcPatrol>().is_none(),
                "passive target must not receive patrol state that can assign navigation goals"
            );
            assert!(
                entity_ref
                    .get::<Navigator>()
                    .expect("passive target must retain production navigation state")
                    .is_idle(),
                "passive target navigator must start idle"
            );
            assert!(entity_ref.get::<NpcMarker>().is_some());
            assert!(entity_ref.get::<Lifecycle>().is_some());
            assert!(entity_ref.get::<Cultivation>().is_some());
            assert!(entity_ref.get::<PassiveTarget>().is_some());
            entity_ref
                .get::<Position>()
                .expect("passive target must expose authoritative Position")
                .get()
        };

        app.world_mut()
            .entity_mut(npc)
            .insert(PendingKnockback::from_distance(
                DVec3::new(1.0, 0.0, 0.0),
                6.0,
                70.0,
                3,
            ));

        for _ in 0..8 {
            app.world_mut().run_schedule(FixedUpdate);
            app.update();
        }

        let entity_ref = app.world().entity(npc);
        let final_position = entity_ref
            .get::<Position>()
            .expect("passive target must remain alive after schedule ticks")
            .get();
        assert_eq!(final_position.x, initial_position.x);
        assert_eq!(final_position.z, initial_position.z);
        assert!(
            entity_ref
                .get::<Navigator>()
                .expect("passive target must retain its Navigator")
                .is_idle(),
            "real patrol and navigator schedules must not move a passive target"
        );
    }

    #[test]
    fn passive_target_command_attack_feedback_damage_and_terminal_lifecycle_are_production_path() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "bong-passive-target-production-path-{}-{unique}",
            std::process::id()
        ));
        let db_path = root.join("data").join("bong.db");
        let deceased_dir = root.join("deceased");
        bootstrap_sqlite(&db_path, "passive-target-production-path")
            .expect("terminal lifecycle test database should bootstrap");

        let scenario = ScenarioSingleClient::new();
        let valence::testing::ScenarioSingleClient {
            mut app,
            client: attacker,
            mut helper,
            ..
        } = scenario;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.init_schedule(FixedUpdate);
        app.insert_resource(PendingScenario::default());
        app.insert_resource(CombatClock { tick: 100 });
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            &deceased_dir,
            "passive-target-production-path",
        ));
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(crate::world::zone::ZoneRegistry::default());

        app.add_event::<CommandResultEvent<NpcScenarioCmd>>();
        app.add_event::<InteractEntityEvent>();
        app.add_event::<AttackIntent>();
        app.add_event::<crate::combat::knockback::KnockbackEvent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::events::ApplyStatusEffectIntent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<crate::combat::weapon::ShieldBlockHit>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_event::<PlayerTerminated>();

        crate::npc::lifecycle::register(&mut app);
        crate::npc::movement::register(&mut app);
        crate::npc::navigator::register(&mut app);
        app.add_systems(Update, handle_npc_scenario);
        app.add_systems(Update, process_pending_scenarios.after(handle_npc_scenario));
        app.add_systems(Update, handle_player_attack);
        app.add_systems(Update, resolve_attack_intents.after(handle_player_attack));
        app.add_systems(
            Update,
            emit_combat_event_to_client.after(resolve_attack_intents),
        );
        app.add_systems(
            Update,
            death_arbiter_tick
                .in_set(NpcTerminalSystemSet::Stage)
                .after(resolve_attack_intents),
        );

        // Use the production command handler to queue the exact scenario spawn.
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<NpcScenarioCmd>>>()
            .send(CommandResultEvent {
                result: NpcScenarioCmd::Run {
                    scenario: NpcScenarioAction::PassiveTarget,
                },
                executor: attacker,
                modifiers: Default::default(),
            });

        // The command queues the request; the next Update runs the production spawn path.
        app.update();
        let target = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<ScenarioNpc>>();
            query
                .iter(world)
                .next()
                .expect("production command should spawn a passive scenario target")
        };
        assert!(app.world().get::<PassiveTarget>(target).is_some());
        assert_eq!(
            app.world().get::<Wounds>(target).unwrap().health_current,
            PASSIVE_TARGET_HEALTH,
            "command-spawned passive target must start with the production scenario health"
        );

        // Complete the player-side production combat prerequisites. The target remains a real
        // runtime NPC bundle; the first ordinary fist attack proves nonlethal damage and feedback.
        app.world_mut().entity_mut(attacker).insert((
            Cultivation::default(),
            MeridianSystem::default(),
            PlayerState {
                karma: 0.0,
                inventory_score: 0.0,
            },
            Lifecycle {
                character_id: canonical_player_id("test"),
                ..Default::default()
            },
            Stamina::default(),
            PlayerAttackCooldown::default(),
            GameMode::Survival,
        ));
        app.world_mut()
            .get_mut::<Wounds>(target)
            .expect("scenario target must carry production Wounds")
            .health_current = PASSIVE_TARGET_HEALTH;

        // First hit proves the target is attackable, deals damage, emits a typed outgoing
        // combat_event to the client, and does not retaliate.
        app.world_mut().send_event(InteractEntityEvent {
            client: attacker,
            entity: target,
            sneaking: false,
            interact: EntityInteraction::Attack,
        });
        app.update();

        let first_hit_health = app.world().get::<Wounds>(target).unwrap().health_current;
        assert!(
            first_hit_health < PASSIVE_TARGET_HEALTH,
            "InteractEntityEvent::Attack must damage a passive target; before={PASSIVE_TARGET_HEALTH}, after={first_hit_health}"
        );
        assert!(
            app.world().get::<PendingKnockback>(target).is_none(),
            "passive target attack resolution must not queue PendingKnockback"
        );
        let attack_events = app.world().resource::<Events<AttackIntent>>();
        let attack_events = attack_events
            .iter_current_update_events()
            .collect::<Vec<_>>();
        assert_eq!(attack_events.len(), 1, "one player attack should resolve");
        assert_eq!(attack_events[0].attacker, attacker);
        assert_eq!(attack_events[0].target, Some(target));
        assert!(
            !attack_events.iter().any(|event| event.attacker == target),
            "passive target must not retaliate with an NPC attack intent"
        );

        {
            let world = app.world_mut();
            let mut clients = world.query::<&mut Client>();
            for mut client_component in clients.iter_mut(world) {
                client_component
                    .flush_packets()
                    .expect("mock client should flush combat feedback packets");
            }
        }
        let combat_payloads = helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                if packet.channel.as_str() != crate::network::agent_bridge::SERVER_DATA_CHANNEL {
                    return None;
                }
                let payload = serde_json::from_slice::<crate::schema::server_data::ServerDataV1>(
                    packet.data.0 .0,
                )
                .ok()?;
                match payload.payload {
                    crate::schema::server_data::ServerDataPayloadV1::CombatEventFloater(
                        floater,
                    ) => Some(floater),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        assert!(
            combat_payloads.iter().any(|floater| floater
                .events
                .iter()
                .any(|event| { event.outgoing && event.amount > 0.0 })),
            "a real CombatEvent must produce a typed outgoing combat_event payload"
        );

        // Exercise the real movement systems with a forced movement request. The marker contract
        // must preserve the exact authoritative position even if another production producer
        // leaves a PendingKnockback on the entity.
        let position_before_knockback = app.world().get::<Position>(target).unwrap().get();
        app.world_mut()
            .entity_mut(target)
            .insert(PendingKnockback::from_distance(
                DVec3::new(1.0, 0.0, 0.0),
                6.0,
                70.0,
                3,
            ));
        for _ in 0..4 {
            app.world_mut().run_schedule(FixedUpdate);
            app.update();
        }
        assert_eq!(
            app.world().get::<Position>(target).unwrap().get(),
            position_before_knockback,
            "real movement and navigator systems must not displace a passive target"
        );
        assert!(app.world().get::<PendingKnockback>(target).is_none());

        // Make the second ordinary attack lethal and drive the complete NPC terminal path.
        app.world_mut().resource_mut::<CombatClock>().tick = 111;
        app.world_mut()
            .get_mut::<Wounds>(target)
            .expect("target remains alive before lethal attack")
            .health_current = 1.0;
        app.world_mut().send_event(InteractEntityEvent {
            client: attacker,
            entity: target,
            sneaking: false,
            interact: EntityInteraction::Attack,
        });
        app.update();

        let settlements = app
            .world()
            .resource::<Events<NpcTerminalSettlementSucceeded>>();
        let settlement = settlements
            .iter_current_update_events()
            .find(|event| event.entity == target)
            .expect("lethal real attack must complete the NPC terminal settlement");
        assert_eq!(settlement.attacker, Some(attacker));
        assert_eq!(
            settlement.reason,
            crate::npc::lifecycle::NpcDeathReason::Combat
        );
        assert!(
            app.world().get_entity(target).is_none(),
            "Valence Last schedule must physically despawn the terminal passive target"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn passive_target_parser_is_pinned() {
        assert!(matches!(
            ScenarioType::from_str("passive_target"),
            Some(ScenarioType::PassiveTarget)
        ));
        assert!(ScenarioType::from_str("passive-target").is_none());
    }

    #[test]
    fn duel_scenario_assigns_distinct_melee_profiles() {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.insert_resource(PendingScenario {
            request: Some((ScenarioType::Duel, DVec3::new(8.0, 66.0, 8.0))),
        });
        app.add_systems(Update, process_pending_scenarios);

        app.update();

        let entries = {
            let world = app.world_mut();
            let mut query =
                world.query_filtered::<(&NpcCombatLoadout, &NpcMeleeArchetype, &NpcMeleeProfile), With<ScenarioNpc>>();
            query
                .iter(world)
                .map(|(l, a, p)| {
                    (
                        l.melee_archetype,
                        l.movement_capabilities.can_sprint,
                        l.movement_capabilities.can_dash,
                        *a,
                        *p,
                    )
                })
                .collect::<Vec<_>>()
        };

        assert_eq!(entries.len(), 2);
        assert!(entries.contains(&(
            NpcMeleeArchetype::Spear,
            true,
            true,
            NpcMeleeArchetype::Spear,
            NpcMeleeArchetype::Spear.profile(),
        )));
        assert!(entries.contains(&(
            NpcMeleeArchetype::Sword,
            true,
            true,
            NpcMeleeArchetype::Sword,
            NpcMeleeArchetype::Sword.profile(),
        )));
    }
}
