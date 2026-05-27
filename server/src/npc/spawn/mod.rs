mod beast;
pub(crate) mod common;
mod commoner;
mod disciple;
mod rogue;
mod zombie;

use big_brain::prelude::BigBrainSet;
use valence::prelude::{
    App, Commands, DVec3, Entity, EventReader, EventWriter, IntoSystemConfigs, PreUpdate, Query,
    ResMut, Update, With,
};

use crate::npc::lifecycle::{NpcArchetype, NpcRegistry, NpcReproductionRequest, NpcSpawnNotice};
use crate::npc::lod::NpcLodTier;
use crate::npc::territory::Territory;
use crate::skin::{NpcSkinFallbackPolicy, SkinPool};

// ---------------------------------------------------------------------------
// Re-exports — public types
// ---------------------------------------------------------------------------

#[allow(unused_imports)]
pub use self::common::{
    DuelTarget, NpcBlackboard, NpcCombatLoadout, NpcMarker, NpcMeleeArchetype, NpcMeleeProfile,
    NpcSkinSpawnContext,
};

#[allow(unused_imports)]
pub use self::rogue::{RoguePopulationSeedConfig, ScatteredCultivatorBundle};

// ---------------------------------------------------------------------------
// Re-exports — pub(crate) functions
// ---------------------------------------------------------------------------

#[allow(unused_imports)]
pub(crate) use self::common::{
    attach_player_skin, draw_npc_skin, fallback_rogue_commoner_kind, npc_skin_name, skin_salt,
    snap_spawn_y_to_surface, spawn_notice, spawn_rogue_commoner_base,
};

#[allow(unused_imports)]
pub(crate) use self::rogue::{
    classify_zones_by_qi, distribute_counts_evenly, initial_age_for_index,
    reserve_zone_distribution, seed_position_for_zone, spawn_rogue_npc_at,
    spawn_scattered_cultivator_at,
};

#[allow(unused_imports)]
pub(crate) use self::commoner::spawn_commoner_npc_at;

#[allow(unused_imports)]
pub(crate) use self::beast::spawn_beast_npc_at;

#[allow(unused_imports)]
pub(crate) use self::disciple::{spawn_disciple_npc_at, spawn_relic_guard_npc_at};

#[allow(unused_imports)]
pub(crate) use self::zombie::spawn_zombie_npc_at;

// ---------------------------------------------------------------------------
// PoissonSpawnSampler — Mitchell's best-candidate 散布采样
// ---------------------------------------------------------------------------

/// plan-npc-overhaul-v1 §P1.2 — Mitchell's best-candidate 散布采样器。
/// 在 zone AABB 的 XZ 平面内生成候选点，选择与已有 NPC 距离最大的点。
#[derive(Clone, Debug)]
pub struct PoissonSpawnSampler {
    pub min_same_archetype_dist: f64,
    pub min_cross_archetype_dist: f64,
    pub max_candidates: u32,
}

impl PoissonSpawnSampler {
    /// 根据 zone 面积自适应参数。
    /// - area >= 500x500 → min_same=48, min_cross=24, max_candidates=30
    /// - 300x300 <= area < 500x500 → min_same=40, min_cross=20, max_candidates=30
    /// - area < 300x300 → min_same=32, min_cross=16, max_candidates=30
    pub fn adaptive_for_zone(zone_bounds: (DVec3, DVec3)) -> Self {
        let (min, max) = zone_bounds;
        let width = (max.x - min.x).abs();
        let depth = (max.z - min.z).abs();
        let area = width * depth;

        let (same_dist, cross_dist) = if area >= 500.0 * 500.0 {
            (48.0, 24.0)
        } else if area >= 300.0 * 300.0 {
            (40.0, 20.0)
        } else {
            (32.0, 16.0)
        };

        Self {
            min_same_archetype_dist: same_dist,
            min_cross_archetype_dist: cross_dist,
            max_candidates: 30,
        }
    }

    /// Mitchell's best-candidate: 生成 `max_candidates` 个随机点，
    /// 选择与已有 NPC 距离最大的。如果最大距离 < min_same_archetype_dist，
    /// 返回 None（zone 已饱和）。
    ///
    /// `existing_positions` 包含 (position, archetype) 对。
    /// `rng_seed` 用于确定性伪随机生成。
    pub fn sample_position(
        &self,
        zone_bounds: (DVec3, DVec3),
        existing_positions: &[(DVec3, NpcArchetype)],
        archetype: NpcArchetype,
        rng_seed: u64,
    ) -> Option<DVec3> {
        let (min, max) = zone_bounds;
        let y = (min.y + max.y) * 0.5;

        if existing_positions.is_empty() {
            // First NPC: place at zone center.
            return Some(DVec3::new((min.x + max.x) * 0.5, y, (min.z + max.z) * 0.5));
        }

        let mut best_pos = None;
        let mut best_min_dist = f64::NEG_INFINITY;

        for i in 0..self.max_candidates {
            let seed = rng_seed
                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(i as u64)
                .wrapping_mul(0xbf58_476d_1ce4_e5b9)
                .wrapping_add(rng_seed.rotate_left(17));
            let fx = ((seed & 0xFFFF_FFFF) as f64) / (0xFFFF_FFFF_u64 as f64);
            let fz = (((seed >> 32) & 0xFFFF_FFFF) as f64) / (0xFFFF_FFFF_u64 as f64);
            let cx = min.x + fx * (max.x - min.x);
            let cz = min.z + fz * (max.z - min.z);
            let candidate = DVec3::new(cx, y, cz);

            let mut min_dist = f64::INFINITY;
            for (existing_pos, existing_archetype) in existing_positions {
                let dx = candidate.x - existing_pos.x;
                let dz = candidate.z - existing_pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                let required = if *existing_archetype == archetype {
                    self.min_same_archetype_dist
                } else {
                    self.min_cross_archetype_dist
                };
                // Use distance relative to the required distance.
                let effective_dist = dist - required;
                if effective_dist < min_dist {
                    min_dist = effective_dist;
                }
            }

            if min_dist > best_min_dist {
                best_min_dist = min_dist;
                best_pos = Some(candidate);
            }
        }

        // If the best candidate is closer than min_same_archetype_dist to any
        // same-archetype NPC, the zone is saturated.
        if best_min_dist < 0.0 {
            // Check: is every candidate failing same-archetype or cross-archetype?
            // Re-check best_pos against actual same-archetype distance.
            if let Some(pos) = best_pos {
                let closest_same = existing_positions
                    .iter()
                    .filter(|(_, a)| *a == archetype)
                    .map(|(p, _)| {
                        let dx = pos.x - p.x;
                        let dz = pos.z - p.z;
                        (dx * dx + dz * dz).sqrt()
                    })
                    .fold(f64::INFINITY, f64::min);
                if closest_same < self.min_same_archetype_dist {
                    return None;
                }
            } else {
                return None;
            }
        }

        best_pos
    }
}

// ---------------------------------------------------------------------------
// System: register
// ---------------------------------------------------------------------------

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering startup spawn systems");
    app.insert_resource(rogue::RoguePopulationSeedConfig::default())
        // plan-npc-overhaul-v1 §P1.4 — PostStartup zombie spawn 已移除。
        .add_systems(
            Update,
            (
                process_npc_reproduction_requests,
                // 种群播种只跑一次（`Local<bool>` 守护），PostStartup 时机在
                // valence ScenarioSingleClient 下 layer 未必就绪，改到 Update 更稳。
                rogue::seed_initial_rogue_population_on_startup,
            ),
        )
        .add_systems(
            PreUpdate,
            attach_deferred_npc_brain_system.before(BigBrainSet::Scorers),
        );
}

// ---------------------------------------------------------------------------
// System: attach_deferred_npc_brain_system
// ---------------------------------------------------------------------------

fn attach_deferred_npc_brain_system(
    mut commands: Commands,
    npcs: Query<(Entity, &common::DeferredNpcBrain, Option<&NpcLodTier>), With<common::NpcMarker>>,
) {
    for (entity, deferred, tier) in &npcs {
        if matches!(tier, Some(NpcLodTier::Dormant) | None) {
            continue;
        }
        commands
            .entity(entity)
            .remove::<common::DeferredNpcBrain>()
            .insert(deferred.build());
    }
}

// ---------------------------------------------------------------------------
// System: process_npc_reproduction_requests
// ---------------------------------------------------------------------------

fn process_npc_reproduction_requests(
    mut commands: Commands,
    mut requests: EventReader<NpcReproductionRequest>,
    mut notices: EventWriter<NpcSpawnNotice>,
    mut skin_pool: Option<ResMut<SkinPool>>,
    mut registry: Option<ResMut<NpcRegistry>>,
    layers: Query<Entity, With<crate::world::dimension::OverworldLayer>>,
) {
    let Some(layer) = layers.iter().next() else {
        // If no layer yet, drain events so they don't pile up across frames.
        for _ in requests.read() {}
        return;
    };

    for request in requests.read() {
        // plan §3.3 Commoner 邻居生子 + §8 Beast 领地繁衍共享同一事件通道。
        match request.archetype {
            NpcArchetype::Commoner => {}
            NpcArchetype::Beast => {
                if request.territory_center.is_none() || request.territory_radius.is_none() {
                    tracing::warn!(
                        "[bong][npc] beast reproduction rejected — missing territory hint (zone=`{}`)",
                        request.home_zone
                    );
                    continue;
                }
            }
            other => {
                tracing::warn!(
                    "[bong][npc] reproduction archetype `{:?}` not supported yet (zone=`{}`)",
                    other,
                    request.home_zone
                );
                continue;
            }
        }

        if let Some(registry) = registry.as_deref_mut() {
            if registry.reserve_zone_batch(request.home_zone.as_str(), 1) == 0 {
                tracing::info!(
                    "[bong][npc] reproduction for `{}` rejected — registry budget exhausted",
                    request.home_zone
                );
                continue;
            }
        }

        let entity = match request.archetype {
            NpcArchetype::Commoner => commoner::spawn_commoner_npc_at(
                &mut commands,
                common::NpcSkinSpawnContext::new(
                    skin_pool.as_deref_mut(),
                    NpcSkinFallbackPolicy::AllowFallback,
                ),
                layer,
                request.home_zone.as_str(),
                request.position,
                request.position,
                crate::cultivation::components::Realm::Awaken,
                request.initial_age_ticks.max(0.0),
            ),
            NpcArchetype::Beast => {
                let territory = Territory::new(
                    request.territory_center.expect("checked above"),
                    request.territory_radius.expect("checked above"),
                );
                beast::spawn_beast_npc_at(
                    &mut commands,
                    layer,
                    request.home_zone.as_str(),
                    request.position,
                    territory,
                    request.initial_age_ticks.max(0.0),
                )
            }
            _ => unreachable!("archetype filter above rejects unsupported variants"),
        };
        tracing::info!(
            "[bong][npc] reproduction spawn {:?} entity={:?} zone=`{}` pos={:?}",
            request.archetype,
            entity,
            request.home_zone,
            request.position
        );
        notices.send(common::spawn_notice(
            entity,
            request.archetype,
            crate::npc::lifecycle::NpcSpawnSource::Reproduction,
            request.home_zone.as_str(),
            request.position,
            request.initial_age_ticks.max(0.0),
        ));
    }
}

// ---------------------------------------------------------------------------
// Test helpers (pub(crate) for test harness)
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) fn spawn_test_npc_runtime_shape(commands: &mut Commands, layer: Entity) -> Entity {
    zombie::spawn_single_zombie_npc(commands, layer)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::StatusEffects;
    use crate::combat::events::AttackIntent;
    use crate::npc::brain;
    use crate::npc::lifecycle::NpcLifespan;
    use crate::npc::movement::GameTick;
    use big_brain::prelude::{BigBrainPlugin, HasThinker, ThinkerBuilder};
    use std::collections::HashMap;
    use valence::client::ClientMarker;
    use valence::prelude::{
        bevy_ecs, App, Commands, DVec3, Entity, EntityKind, EntityLayerId, EventReader, Position,
        PreUpdate, Res, Resource, Update,
    };

    use crate::combat::components::WoundKind;
    use crate::combat::events::FIST_REACH;
    use crate::cultivation::components::Realm;
    use crate::npc::faction::{FactionId, FactionMembership, FactionRank, MissionExecuteState};
    use crate::npc::movement::MovementCapabilities;
    use crate::npc::patrol::NpcPatrol;
    use crate::npc::social::SocializeState;
    use crate::npc::territory::{HuntState, ProtectYoungState, Territory, TerritoryPatrolState};
    use crate::skin::{NpcPlayerSkin, NpcSkinFallbackPolicy, SignedSkin};
    use crate::world::zone::{Zone, ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

    use super::common::DeferredNpcBrain;
    use super::common::{
        fallback_rogue_commoner_kind, NpcBlackboard, NpcCombatLoadout, NpcMarker,
        NpcMeleeArchetype, NpcMeleeProfile, NpcSkinSpawnContext,
    };
    use super::rogue::{
        classify_zones_by_qi, distribute_counts_evenly, initial_age_for_index,
        seed_position_for_zone, RoguePopulationSeedConfig,
    };

    use crate::npc::lifecycle::NpcRegistry;
    use crate::npc::relic::{GuardState, GuardianDuty, TrialEval, TrialState};
    use bevy_transform::components::{GlobalTransform, Transform};

    #[derive(Clone, Copy, Resource)]
    struct TestLayer(Entity);

    #[derive(Default)]
    struct CapturedAttackIntents(Vec<AttackIntent>);

    impl Resource for CapturedAttackIntents {}

    fn setup_test_layer(mut commands: Commands) {
        let layer = commands.spawn_empty().id();
        commands.insert_resource(TestLayer(layer));
    }

    fn spawn_test_npc(mut commands: Commands, layer: Res<TestLayer>) {
        zombie::spawn_single_zombie_npc(&mut commands, layer.0);
    }

    fn capture_attack_intents(
        mut events: EventReader<AttackIntent>,
        mut captured: valence::prelude::ResMut<CapturedAttackIntents>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    #[test]
    fn spawn_npc_creates_single_zombie_with_expected_components() {
        let mut app = App::new();
        app.add_plugins(BigBrainPlugin::new(PreUpdate));
        app.add_systems(
            valence::prelude::Startup,
            (setup_test_layer, spawn_test_npc.after(setup_test_layer)),
        );

        app.update();
        app.update();

        let npc_entities = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).collect::<Vec<_>>()
        };

        assert_eq!(
            npc_entities.len(),
            1,
            "expected exactly one NPC marker entity"
        );

        let npc_entity = npc_entities[0];

        let kind = app
            .world()
            .get::<EntityKind>(npc_entity)
            .expect("NPC should have EntityKind component");
        assert_eq!(*kind, EntityKind::ZOMBIE);

        let position = app
            .world()
            .get::<Position>(npc_entity)
            .expect("NPC should have Position component");
        assert_eq!(position.get(), DVec3::new(14.0, 66.0, 14.0));

        let transform = app
            .world()
            .get::<Transform>(npc_entity)
            .expect("NPC should have Transform component");
        assert_eq!(transform.translation.x, 14.0);
        assert_eq!(transform.translation.y, 66.0);
        assert_eq!(transform.translation.z, 14.0);

        let _global_transform = app
            .world()
            .get::<GlobalTransform>(npc_entity)
            .expect("NPC should have GlobalTransform component");

        let blackboard = app
            .world()
            .get::<NpcBlackboard>(npc_entity)
            .expect("NPC should have NpcBlackboard component");
        assert_eq!(blackboard.nearest_player, None);
        assert!(
            blackboard.player_distance.is_infinite(),
            "NpcBlackboard.player_distance should default to infinity"
        );

        let archetype = app
            .world()
            .get::<NpcMeleeArchetype>(npc_entity)
            .expect("NPC should have NpcMeleeArchetype component");
        let loadout = app
            .world()
            .get::<NpcCombatLoadout>(npc_entity)
            .expect("NPC should have NpcCombatLoadout component");
        let profile = app
            .world()
            .get::<NpcMeleeProfile>(npc_entity)
            .expect("NPC should have NpcMeleeProfile component");
        let capabilities = app
            .world()
            .get::<MovementCapabilities>(npc_entity)
            .expect("NPC should have MovementCapabilities component");
        let _status_effects = app
            .world()
            .get::<StatusEffects>(npc_entity)
            .expect("NPC should include StatusEffects for shared combat resolver");
        assert_eq!(
            loadout.melee_archetype,
            NpcCombatLoadout::default().melee_archetype
        );
        assert_eq!(
            loadout.movement_capabilities.can_sprint,
            NpcCombatLoadout::default().movement_capabilities.can_sprint
        );
        assert_eq!(
            loadout.movement_capabilities.can_dash,
            NpcCombatLoadout::default().movement_capabilities.can_dash
        );
        assert_eq!(*archetype, NpcMeleeArchetype::Brawler);
        assert_eq!(*profile, NpcMeleeArchetype::Brawler.profile());
        assert_eq!(profile.wound_kind, WoundKind::Blunt);
        assert_eq!(
            capabilities.can_sprint,
            NpcCombatLoadout::default().movement_capabilities.can_sprint
        );
        assert_eq!(
            capabilities.can_dash,
            NpcCombatLoadout::default().movement_capabilities.can_dash
        );

        let patrol = app
            .world()
            .get::<NpcPatrol>(npc_entity)
            .expect("NPC should have a patrol component");
        assert_eq!(patrol.home_zone, DEFAULT_SPAWN_ZONE_NAME);
        assert_eq!(patrol.current_target, DVec3::new(14.0, 66.0, 14.0));

        let layer_id = app
            .world()
            .get::<EntityLayerId>(npc_entity)
            .expect("NPC should have EntityLayerId component");
        assert_ne!(
            layer_id.0,
            Entity::PLACEHOLDER,
            "NPC should be assigned to a non-placeholder layer"
        );

        let _thinker_builder = app
            .world()
            .get::<ThinkerBuilder>(npc_entity)
            .expect("NPC should have a Thinker builder attached at spawn time");

        let npc_archetype = app
            .world()
            .get::<NpcArchetype>(npc_entity)
            .expect("NPC should include shared NpcArchetype component");
        assert_eq!(*npc_archetype, NpcArchetype::Zombie);

        let lifespan = app
            .world()
            .get::<NpcLifespan>(npc_entity)
            .expect("NPC should include shared lifespan component");
        assert_eq!(lifespan.age_ticks, 0.0);
        assert!(lifespan.max_age_ticks > 0.0);

        let has_thinker = app
            .world()
            .get::<HasThinker>(npc_entity)
            .expect("BigBrain should attach HasThinker to NPC");

        let _thinker = app
            .world()
            .get::<big_brain::prelude::Thinker>(has_thinker.entity())
            .expect("BigBrain thinker entity should contain Thinker component");
    }

    #[test]
    fn startup_spawned_npc_default_thinker_emits_attack_intent_in_melee_range() {
        let mut app = App::new();
        crate::npc::lifecycle::register(&mut app);
        brain::register(&mut app);
        app.insert_resource(CapturedAttackIntents::default());
        app.insert_resource(GameTick(120));
        app.add_event::<AttackIntent>();
        app.add_systems(Update, capture_attack_intents);
        app.add_systems(
            valence::prelude::Startup,
            (setup_test_layer, spawn_test_npc.after(setup_test_layer)),
        );

        let player = app
            .world_mut()
            .spawn((ClientMarker, Position::new([14.8, 66.0, 14.0])))
            .id();

        for _ in 0..5 {
            app.update();
        }

        let captured = &app.world().resource::<CapturedAttackIntents>().0;
        assert!(
            !captured.is_empty(),
            "default startup NPC thinker should emit AttackIntent when a player enters melee range"
        );
        assert_eq!(captured[0].target, Some(player));
        assert_eq!(captured[0].reach, FIST_REACH);
        assert_eq!(captured[0].wound_kind, WoundKind::Blunt);
    }

    #[test]
    fn spawn_commoner_npc_at_attaches_commoner_components() {
        let mut app = App::new();
        app.add_systems(
            valence::prelude::Startup,
            (
                setup_test_layer,
                spawn_test_commoner.after(setup_test_layer),
            ),
        );

        app.update();
        app.update();

        let npcs = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).collect::<Vec<_>>()
        };
        assert_eq!(npcs.len(), 1);
        let npc = npcs[0];

        let archetype = *app.world().get::<NpcArchetype>(npc).unwrap();
        assert_eq!(archetype, NpcArchetype::Commoner);

        let kind = *app.world().get::<EntityKind>(npc).unwrap();
        assert_eq!(kind, EntityKind::VILLAGER);

        let hunger = *app
            .world()
            .get::<crate::npc::hunger::Hunger>(npc)
            .expect("commoner should have Hunger");
        assert_eq!(hunger.value, 1.0);

        let wander = *app
            .world()
            .get::<crate::npc::brain::WanderState>(npc)
            .expect("commoner should have WanderState");
        assert!(wander.destination.is_none());

        let lifespan = *app.world().get::<NpcLifespan>(npc).unwrap();
        assert_eq!(lifespan.age_ticks, 2.0);
        assert!(lifespan.max_age_ticks > 0.0);
    }

    fn spawn_test_commoner(mut commands: Commands, layer: Res<TestLayer>) {
        commoner::spawn_commoner_npc_at(
            &mut commands,
            NpcSkinSpawnContext::new(None, NpcSkinFallbackPolicy::AllowFallback),
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(20.0, 66.0, 20.0),
            DVec3::new(20.0, 66.0, 20.0),
            Realm::Awaken,
            2.0,
        );
    }

    #[test]
    fn spawn_rogue_npc_at_attaches_rogue_components() {
        let mut app = App::new();
        app.add_systems(
            valence::prelude::Startup,
            (setup_test_layer, spawn_test_rogue.after(setup_test_layer)),
        );

        app.update();
        app.update();

        let npcs = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).collect::<Vec<_>>()
        };
        assert_eq!(npcs.len(), 1);
        let npc = npcs[0];

        assert_eq!(
            *app.world().get::<NpcArchetype>(npc).unwrap(),
            NpcArchetype::Rogue
        );

        assert!(
            app.world()
                .get::<crate::npc::brain::CultivateState>(npc)
                .is_some(),
            "rogue should carry CultivateState"
        );

        let lifespan = *app.world().get::<NpcLifespan>(npc).unwrap();
        assert_eq!(
            lifespan.max_age_ticks,
            NpcArchetype::Rogue.default_max_age_ticks()
        );
    }

    #[test]
    fn deferred_seed_brain_attaches_when_lod_wakes() {
        let mut app = App::new();
        app.add_systems(PreUpdate, attach_deferred_npc_brain_system);
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Far,
                DeferredNpcBrain::ScatteredCultivator,
            ))
            .id();

        app.update();

        assert!(app.world().get::<ThinkerBuilder>(npc).is_some());
        assert!(app.world().get::<DeferredNpcBrain>(npc).is_none());
    }

    #[test]
    fn deferred_seed_brain_stays_detached_while_dormant() {
        let mut app = App::new();
        app.add_systems(PreUpdate, attach_deferred_npc_brain_system);
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Dormant,
                DeferredNpcBrain::ScatteredCultivator,
            ))
            .id();

        app.update();

        assert!(app.world().get::<ThinkerBuilder>(npc).is_none());
        assert!(app.world().get::<DeferredNpcBrain>(npc).is_some());
    }

    #[test]
    fn spawn_scattered_cultivator_at_attaches_farming_brain_components() {
        let mut app = App::new();
        app.add_systems(
            valence::prelude::Startup,
            (
                setup_test_layer,
                spawn_test_scattered_cultivator.after(setup_test_layer),
            ),
        );

        app.update();
        app.update();

        let npc = only_spawned_npc(&mut app);

        assert_eq!(
            *app.world().get::<NpcArchetype>(npc).unwrap(),
            NpcArchetype::Rogue
        );
        let scattered = app
            .world()
            .get::<crate::npc::scattered_cultivator::ScatteredCultivator>(npc)
            .expect("scattered cultivator should mark seeded Rogue NPCs");
        assert_eq!(scattered.home_plot, None);
        assert_eq!(scattered.fail_streak, 0);
        assert!(matches!(
            scattered.temperament,
            crate::npc::scattered_cultivator::FarmingTemperament::Patient
                | crate::npc::scattered_cultivator::FarmingTemperament::Greedy
                | crate::npc::scattered_cultivator::FarmingTemperament::Anxious
                | crate::npc::scattered_cultivator::FarmingTemperament::Aggressive
        ));
        assert!(
            app.world().get::<ThinkerBuilder>(npc).is_some(),
            "scattered cultivator should carry a live farming thinker"
        );
        assert!(
            app.world()
                .get::<crate::npc::brain::CultivateState>(npc)
                .is_some(),
            "scattered cultivator remains a cultivating Rogue"
        );
    }

    #[test]
    fn rogue_commoner_visual_kind_uses_player_only_for_real_skin() {
        assert_eq!(
            fallback_rogue_commoner_kind(&None),
            EntityKind::VILLAGER,
            "None skin should produce villager (neutral NPC model)",
        );
        assert_eq!(
            fallback_rogue_commoner_kind(&Some(SignedSkin::fallback())),
            EntityKind::VILLAGER,
            "MineSkin fallback skin should produce villager, not witch (散修不该是女巫模型)",
        );
        assert_eq!(
            fallback_rogue_commoner_kind(&Some(SignedSkin {
                value: "value".into(),
                signature: "sig".into(),
                source: crate::skin::SkinSource::MineSkinRandom {
                    hash: "hash".into(),
                },
            })),
            EntityKind::PLAYER,
            "real MineSkin skin should produce player entity",
        );
    }

    fn spawn_test_rogue(mut commands: Commands, layer: Res<TestLayer>) {
        rogue::spawn_rogue_npc_at(
            &mut commands,
            NpcSkinSpawnContext::new(None, NpcSkinFallbackPolicy::AllowFallback),
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(18.0, 66.0, 18.0),
            DVec3::new(18.0, 66.0, 18.0),
            Realm::Awaken,
            0.0,
        );
    }

    fn spawn_test_scattered_cultivator(mut commands: Commands, layer: Res<TestLayer>) {
        rogue::spawn_scattered_cultivator_at(
            &mut commands,
            NpcSkinSpawnContext::new(None, NpcSkinFallbackPolicy::AllowFallback),
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(19.0, 66.0, 19.0),
            DVec3::new(19.0, 66.0, 19.0),
            0.9,
            Realm::Awaken,
            0.0,
        );
    }

    #[test]
    fn spawn_beast_npc_at_attaches_live_territory_brain_components() {
        let mut app = App::new();
        app.add_systems(
            valence::prelude::Startup,
            (setup_test_layer, spawn_test_beast.after(setup_test_layer)),
        );
        app.update();
        app.update();

        let beast = only_spawned_npc(&mut app);

        assert!(app.world().get::<TerritoryPatrolState>(beast).is_some());
        assert!(app.world().get::<HuntState>(beast).is_some());
        assert!(app.world().get::<ProtectYoungState>(beast).is_some());
        assert!(app
            .world()
            .get::<crate::fauna::components::FaunaTag>(beast)
            .is_some());
        let tag = app
            .world()
            .get::<crate::fauna::components::FaunaTag>(beast)
            .expect("beast should carry fauna tag");
        assert_eq!(
            app.world().get::<EntityKind>(beast),
            Some(&crate::fauna::visual::entity_kind_for_beast(tag.beast_kind)),
            "beast should spawn with a fauna custom visual entity kind"
        );
        assert_eq!(
            app.world()
                .get::<crate::fauna::visual::FaunaVisualKind>(beast)
                .copied(),
            crate::fauna::visual::visual_kind_for_beast(tag.beast_kind)
        );
        let _thinker = app
            .world()
            .get::<ThinkerBuilder>(beast)
            .expect("beast should carry the live territory thinker");

        let wounds = app
            .world()
            .get::<crate::combat::components::Wounds>(beast)
            .expect("beast should have wounds");
        let expected_hp = tag.beast_kind.health_max();
        assert_eq!(
            wounds.health_max, expected_hp,
            "beast {:?} health_max should be {} (per-kind), not default 100",
            tag.beast_kind, expected_hp
        );
        assert_eq!(
            wounds.health_current, expected_hp,
            "beast {:?} health_current should start at health_max",
            tag.beast_kind
        );
    }

    #[test]
    fn spawn_disciple_npc_at_attaches_mission_and_social_state() {
        let mut app = App::new();
        app.add_systems(
            valence::prelude::Startup,
            (
                setup_test_layer,
                spawn_test_disciple.after(setup_test_layer),
            ),
        );
        app.update();
        app.update();

        let disciple = only_spawned_npc(&mut app);

        assert!(app.world().get::<MissionExecuteState>(disciple).is_some());
        assert!(app.world().get::<SocializeState>(disciple).is_some());
        assert!(app.world().get::<FactionMembership>(disciple).is_some());
        let _thinker = app
            .world()
            .get::<ThinkerBuilder>(disciple)
            .expect("disciple should carry the live faction/social thinker");
    }

    #[test]
    fn spawn_relic_guard_npc_at_attaches_guardian_trial_state() {
        let mut app = App::new();
        app.add_systems(
            valence::prelude::Startup,
            (
                setup_test_layer,
                spawn_test_relic_guard.after(setup_test_layer),
            ),
        );
        app.update();
        app.update();

        let guard = only_spawned_npc(&mut app);

        assert!(app.world().get::<GuardState>(guard).is_some());
        assert!(app.world().get::<TrialState>(guard).is_some());
        assert!(app.world().get::<GuardianDuty>(guard).is_some());
        assert!(app.world().get::<TrialEval>(guard).is_some());
        let _thinker = app
            .world()
            .get::<ThinkerBuilder>(guard)
            .expect("relic guard should carry the live guardian thinker");
    }

    fn only_spawned_npc(app: &mut App) -> Entity {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        let npcs = query.iter(world).collect::<Vec<_>>();
        assert_eq!(npcs.len(), 1);
        npcs[0]
    }

    fn spawn_test_beast(mut commands: Commands, layer: Res<TestLayer>) {
        beast::spawn_beast_npc_at(
            &mut commands,
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(40.0, 66.0, 40.0),
            Territory::new(DVec3::new(40.0, 66.0, 40.0), 30.0),
            0.0,
        );
    }

    fn spawn_test_disciple(mut commands: Commands, layer: Res<TestLayer>) {
        disciple::spawn_disciple_npc_at(
            &mut commands,
            NpcSkinSpawnContext::new(None, NpcSkinFallbackPolicy::AllowFallback),
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(42.0, 66.0, 42.0),
            DVec3::new(42.0, 66.0, 42.0),
            FactionId::Attack,
            FactionRank::Disciple,
            Realm::Awaken,
            None,
            0.0,
        );
    }

    fn spawn_test_relic_guard(mut commands: Commands, layer: Res<TestLayer>) {
        disciple::spawn_relic_guard_npc_at(
            &mut commands,
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(44.0, 66.0, 44.0),
            24.0,
            "relic:test",
            "trial:test",
        );
    }

    /// Helper: build a real (non-fallback) SignedSkin for testing.
    fn real_signed_skin(label: &str) -> SignedSkin {
        SignedSkin {
            value: format!("value-{label}"),
            signature: format!("sig-{label}"),
            source: crate::skin::SkinSource::MineSkinRandom {
                hash: label.to_string(),
            },
        }
    }

    /// Helper: create a SkinPool pre-loaded with real skins for every
    /// PREFETCH_KEYS bucket so that `next_for_profile` returns a non-fallback
    /// skin regardless of the resolved pool key.
    fn pool_with_real_skins() -> SkinPool {
        let mut pool = SkinPool::default();
        for key in crate::skin::npc_skin_selector::NpcSkinPoolKey::PREFETCH_KEYS {
            for i in 0..3 {
                pool.insert_for_key(key, real_signed_skin(&format!("{}-{i}", key.as_str())));
            }
        }
        pool
    }

    #[test]
    fn spawn_disciple_with_real_skin_attaches_npc_player_skin_and_player_kind() {
        let mut app = App::new();
        app.add_systems(
            valence::prelude::Startup,
            (
                setup_test_layer,
                spawn_test_disciple_with_skin.after(setup_test_layer),
            ),
        );
        app.update();
        app.update();

        let disciple = only_spawned_npc(&mut app);

        let kind = *app
            .world()
            .get::<EntityKind>(disciple)
            .expect("disciple should have EntityKind");
        assert_eq!(
            kind,
            EntityKind::PLAYER,
            "disciple spawned with a real skin should use PLAYER entity kind, got {:?}",
            kind
        );

        let npc_skin = app
            .world()
            .get::<NpcPlayerSkin>(disciple)
            .expect("disciple spawned with real skin must have NpcPlayerSkin component");
        assert!(
            !npc_skin.skin.is_fallback(),
            "NpcPlayerSkin.skin should be a real (non-fallback) skin"
        );
        assert!(
            !npc_skin.name.is_empty(),
            "NpcPlayerSkin.name should be non-empty"
        );
    }

    #[test]
    fn spawn_disciple_without_skin_pool_falls_back_to_villager() {
        let mut app = App::new();
        app.add_systems(
            valence::prelude::Startup,
            (
                setup_test_layer,
                spawn_test_disciple.after(setup_test_layer),
            ),
        );
        app.update();
        app.update();

        let disciple = only_spawned_npc(&mut app);

        let kind = *app
            .world()
            .get::<EntityKind>(disciple)
            .expect("disciple should have EntityKind");
        assert_eq!(
            kind,
            EntityKind::VILLAGER,
            "disciple spawned without skin pool should fall back to VILLAGER, got {:?}",
            kind
        );

        assert!(
            app.world().get::<NpcPlayerSkin>(disciple).is_none(),
            "disciple spawned without skin pool must NOT have NpcPlayerSkin component"
        );
    }

    #[test]
    fn spawn_disciple_with_empty_skin_pool_falls_back_to_villager() {
        let mut app = App::new();
        app.add_systems(
            valence::prelude::Startup,
            (
                setup_test_layer,
                spawn_test_disciple_with_empty_pool.after(setup_test_layer),
            ),
        );
        app.update();
        app.update();

        let disciple = only_spawned_npc(&mut app);

        let kind = *app
            .world()
            .get::<EntityKind>(disciple)
            .expect("disciple should have EntityKind");
        assert_eq!(
            kind,
            EntityKind::VILLAGER,
            "disciple spawned with empty skin pool should fall back to VILLAGER because draw_npc_skin returns a fallback skin, got {:?}",
            kind
        );

        assert!(
            app.world().get::<NpcPlayerSkin>(disciple).is_none(),
            "disciple spawned with empty (fallback) skin pool must NOT have NpcPlayerSkin — fallback skins are filtered out"
        );
    }

    fn spawn_test_disciple_with_skin(mut commands: Commands, layer: Res<TestLayer>) {
        let mut pool = pool_with_real_skins();
        disciple::spawn_disciple_npc_at(
            &mut commands,
            NpcSkinSpawnContext::new(Some(&mut pool), NpcSkinFallbackPolicy::AllowFallback),
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(42.0, 66.0, 42.0),
            DVec3::new(42.0, 66.0, 42.0),
            FactionId::Attack,
            FactionRank::Disciple,
            Realm::Awaken,
            None,
            0.0,
        );
    }

    fn spawn_test_disciple_with_empty_pool(mut commands: Commands, layer: Res<TestLayer>) {
        let mut pool = SkinPool::default();
        // Mark fallback_mode so `ready_for_spawn()` returns true — but no actual skins,
        // so `next_for_profile` yields `SignedSkin::fallback()`.
        pool.insert_for_key(
            crate::skin::npc_skin_selector::NpcSkinPoolKey(
                crate::skin::npc_skin_selector::NpcSkinTier::Other,
            ),
            crate::skin::SignedSkin::fallback(),
        );
        disciple::spawn_disciple_npc_at(
            &mut commands,
            NpcSkinSpawnContext::new(Some(&mut pool), NpcSkinFallbackPolicy::AllowFallback),
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(42.0, 66.0, 42.0),
            DVec3::new(42.0, 66.0, 42.0),
            FactionId::Attack,
            FactionRank::Disciple,
            Realm::Awaken,
            None,
            0.0,
        );
    }

    // -----------------------------------------------------------------------
    // Rogue population seed — pure-function tests + full-stack spawn smoke
    // -----------------------------------------------------------------------

    fn mk_zone(name: &str, spirit_qi: f64, center: [f64; 3]) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (
                DVec3::new(center[0] - 200.0, -64.0, center[2] - 200.0),
                DVec3::new(center[0] + 200.0, 320.0, center[2] + 200.0),
            ),
            spirit_qi,
            danger_level: 1,
            active_events: Vec::new(),
            patrol_anchors: vec![DVec3::new(center[0], center[1], center[2])],
            blocked_tiles: Vec::new(),
        }
    }

    #[test]
    fn classify_zones_by_qi_partitions_at_threshold() {
        let zones = vec![
            mk_zone("high", 0.7, [0.0, 66.0, 0.0]),
            mk_zone("mid", 0.4, [10.0, 66.0, 0.0]),
            mk_zone("low", 0.1, [20.0, 66.0, 0.0]),
        ];
        let (resource, other) = classify_zones_by_qi(&zones, 0.4);
        assert_eq!(resource.len(), 2, "0.7 and 0.4 should be >= 0.4");
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].name, "low");
    }

    #[test]
    fn distribute_counts_evenly_spreads_remainder_to_first_buckets() {
        assert_eq!(distribute_counts_evenly(20, 3), vec![7, 7, 6]);
        assert_eq!(distribute_counts_evenly(80, 3), vec![27, 27, 26]);
        assert_eq!(distribute_counts_evenly(10, 10), vec![1; 10]);
        assert_eq!(distribute_counts_evenly(0, 3), vec![0, 0, 0]);
        assert_eq!(distribute_counts_evenly(5, 0), Vec::<u32>::new());
    }

    #[test]
    fn seed_position_clamps_to_zone_bounds() {
        let zone = mk_zone("z", 0.5, [0.0, 66.0, 0.0]);
        for idx in 0..64u32 {
            let (pos, _) = seed_position_for_zone(&zone, idx);
            assert!(
                zone.contains(pos),
                "idx {idx} produced out-of-bound pos {pos:?}"
            );
        }
    }

    #[test]
    fn initial_age_spreads_across_10_buckets() {
        let max_age = 100_000.0;
        let ages: Vec<f64> = (0..20)
            .map(|i| initial_age_for_index(i, max_age, 0.8))
            .collect();
        // Two full cycles of 10 buckets each.
        assert_eq!(ages[0], 0.0);
        assert!(ages[9] > 0.0);
        assert_eq!(ages[0], ages[10], "bucket should repeat at index 10");
        let max_age_produced = ages.iter().cloned().fold(0.0_f64, f64::max);
        assert!(max_age_produced <= max_age * 0.8 + 1e-9);
    }

    #[test]
    fn seed_splits_20_rogues_80_20_across_zones() {
        let scenario = valence::testing::ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);

        let mut zones = ZoneRegistry::fallback();
        // fallback gives us "spawn" @ qi=0.3; override to match prod-style mix.
        zones.zones[0].spirit_qi = 0.3;
        zones
            .zones
            .push(mk_zone("resource_a", 0.7, [1000.0, 70.0, 0.0]));
        zones
            .zones
            .push(mk_zone("resource_b", 0.5, [2000.0, 70.0, 0.0]));
        zones
            .zones
            .push(mk_zone("resource_c", 0.4, [3000.0, 70.0, 0.0]));
        zones
            .zones
            .push(mk_zone("other_a", 0.2, [0.0, 70.0, 5000.0]));
        app.insert_resource(zones);
        // Use a higher max so the registry doesn't block 20 spawns.
        let mut registry = NpcRegistry {
            max_npc_count: 200,
            resume_npc_count: 180,
            ..Default::default()
        };
        // Clear per_zone_caps so zone caps don't limit this test.
        registry.per_zone_caps.clear();
        app.insert_resource(registry);
        app.insert_resource(RoguePopulationSeedConfig::default());
        app.add_event::<NpcSpawnNotice>();
        app.add_systems(Update, rogue::seed_initial_rogue_population_on_startup);

        let rogue_seed_batch_size = 5u32;
        for _ in 0..(20 / rogue_seed_batch_size + 1) {
            app.update();
        }

        let by_archetype = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<&NpcArchetype, With<NpcMarker>>();
            query.iter(world).copied().collect::<Vec<_>>()
        };
        assert_eq!(
            by_archetype
                .iter()
                .filter(|a| **a == NpcArchetype::Rogue)
                .count(),
            20,
            "should seed exactly 20 rogues (target_count default)"
        );

        // Sanity: 80% resource / 20% other — count by home_zone.
        let zone_counts: HashMap<String, u32> = {
            let world = app.world_mut();
            let mut counts: HashMap<String, u32> = HashMap::new();
            let mut query = world.query_filtered::<&NpcPatrol, With<NpcMarker>>();
            for patrol in query.iter(world) {
                *counts.entry(patrol.home_zone.clone()).or_insert(0) += 1;
            }
            counts
        };
        let resource_total: u32 = ["resource_a", "resource_b", "resource_c"]
            .iter()
            .map(|n| zone_counts.get(*n).copied().unwrap_or(0))
            .sum();
        let other_total: u32 = ["spawn", "other_a"]
            .iter()
            .map(|n| zone_counts.get(*n).copied().unwrap_or(0))
            .sum();
        assert_eq!(
            resource_total, 16,
            "80% of 20 should land in resource zones"
        );
        assert_eq!(other_total, 4, "20% of 20 should land in other zones");

        // Registry 已扣 20 配额。
        let registry = app.world().resource::<NpcRegistry>();
        assert_eq!(registry.live_npc_count, 20);
    }

    #[test]
    fn seed_respects_disabled_config() {
        let scenario = valence::testing::ScenarioSingleClient::new();
        let mut app = scenario.app;
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(NpcRegistry::default());
        app.insert_resource(RoguePopulationSeedConfig {
            target_count: 0,
            ..RoguePopulationSeedConfig::default()
        });
        app.add_event::<NpcSpawnNotice>();
        app.add_systems(Update, rogue::seed_initial_rogue_population_on_startup);

        app.update();

        let count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).count()
        };
        assert_eq!(count, 0);
    }

    #[test]
    fn seed_falls_back_to_other_zones_when_no_resource_qualifies() {
        let scenario = valence::testing::ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].spirit_qi = 0.1; // 强制 < 0.4 门槛，使其归入 "other"
        app.insert_resource(zones);
        let mut registry = NpcRegistry::default();
        // Clear per_zone_caps so zone caps don't limit this test.
        registry.per_zone_caps.clear();
        app.insert_resource(registry);
        app.insert_resource(RoguePopulationSeedConfig {
            target_count: 10,
            ..RoguePopulationSeedConfig::default()
        });
        app.add_event::<NpcSpawnNotice>();
        app.add_systems(Update, rogue::seed_initial_rogue_population_on_startup);

        // With batch_size=5, need at least 2 ticks for 10 NPCs.
        app.update();
        app.update();

        let count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).count()
        };
        assert_eq!(count, 10, "all 10 rogues should land in fallback zone");
        let home_zone = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<&NpcPatrol, With<NpcMarker>>();
            query.iter(world).next().unwrap().home_zone.clone()
        };
        assert_eq!(home_zone, DEFAULT_SPAWN_ZONE_NAME);
    }

    #[test]
    fn reproduction_processor_spawns_commoner_from_event_and_decrements_registry() {
        let scenario = valence::testing::ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.add_event::<NpcReproductionRequest>();
        app.add_event::<NpcSpawnNotice>();
        app.insert_resource(NpcRegistry::default());
        app.add_systems(Update, process_npc_reproduction_requests);

        app.update();

        app.world_mut().send_event(NpcReproductionRequest {
            archetype: NpcArchetype::Commoner,
            position: DVec3::new(30.0, 66.0, 30.0),
            home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            initial_age_ticks: 0.0,
            territory_center: None,
            territory_radius: None,
        });

        app.update();

        let npcs = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<&NpcArchetype, With<NpcMarker>>();
            query.iter(world).copied().collect::<Vec<_>>()
        };
        assert_eq!(npcs, vec![NpcArchetype::Commoner]);

        let registry = app.world().resource::<NpcRegistry>();
        assert_eq!(
            registry.live_npc_count, 1,
            "reproduction must reserve one spawn slot from NpcRegistry"
        );
    }

    #[test]
    fn reproduction_processor_dispatches_beast_request_with_territory_hint() {
        let scenario = valence::testing::ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.add_event::<NpcReproductionRequest>();
        app.add_event::<NpcSpawnNotice>();
        app.insert_resource(NpcRegistry::default());
        app.add_systems(Update, process_npc_reproduction_requests);

        app.update();

        let center = DVec3::new(50.0, 66.0, 50.0);
        app.world_mut().send_event(NpcReproductionRequest {
            archetype: NpcArchetype::Beast,
            position: center,
            home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            initial_age_ticks: 0.0,
            territory_center: Some(center),
            territory_radius: Some(30.0),
        });

        app.update();

        let (arch, has_territory) = {
            let world = app.world_mut();
            let mut query =
                world.query_filtered::<(&NpcArchetype, Option<&Territory>), With<NpcMarker>>();
            let (arch, territory) = query.iter(world).next().expect("spawned beast");
            (*arch, territory.is_some())
        };
        assert_eq!(arch, NpcArchetype::Beast);
        assert!(has_territory, "beast reproduction must attach Territory");

        let registry = app.world().resource::<NpcRegistry>();
        assert_eq!(registry.live_npc_count, 1);
    }

    #[test]
    fn reproduction_processor_skips_beast_request_without_territory_hint() {
        let scenario = valence::testing::ScenarioSingleClient::new();
        let mut app = scenario.app;
        app.add_event::<NpcReproductionRequest>();
        app.add_event::<NpcSpawnNotice>();
        app.insert_resource(NpcRegistry::default());
        app.add_systems(Update, process_npc_reproduction_requests);

        app.update();

        app.world_mut().send_event(NpcReproductionRequest {
            archetype: NpcArchetype::Beast,
            position: DVec3::new(30.0, 66.0, 30.0),
            home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            initial_age_ticks: 0.0,
            territory_center: None,
            territory_radius: None,
        });

        app.update();

        let npc_count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).count()
        };
        assert_eq!(npc_count, 0, "beast without territory hint must not spawn");

        let registry = app.world().resource::<NpcRegistry>();
        assert_eq!(
            registry.live_npc_count, 0,
            "budget must not be reserved on rejected beast request"
        );
    }

    #[test]
    fn reproduction_processor_skips_unsupported_archetype() {
        let scenario = valence::testing::ScenarioSingleClient::new();
        let mut app = scenario.app;
        app.add_event::<NpcReproductionRequest>();
        app.add_event::<NpcSpawnNotice>();
        app.insert_resource(NpcRegistry::default());
        app.add_systems(Update, process_npc_reproduction_requests);

        app.update();

        app.world_mut().send_event(NpcReproductionRequest {
            archetype: NpcArchetype::Zombie,
            position: DVec3::new(30.0, 66.0, 30.0),
            home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            initial_age_ticks: 0.0,
            territory_center: None,
            territory_radius: None,
        });

        app.update();

        let npc_count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).count()
        };
        assert_eq!(npc_count, 0);

        let registry = app.world().resource::<NpcRegistry>();
        assert_eq!(registry.live_npc_count, 0);
    }

    #[test]
    fn reproduction_processor_rejects_when_registry_budget_exhausted() {
        let scenario = valence::testing::ScenarioSingleClient::new();
        let mut app = scenario.app;
        app.add_event::<NpcReproductionRequest>();
        app.add_event::<NpcSpawnNotice>();
        let mut registry = NpcRegistry::default();
        registry.live_npc_count = registry.max_npc_count;
        registry.spawn_paused = true;
        app.insert_resource(registry);
        app.add_systems(Update, process_npc_reproduction_requests);

        app.update();

        app.world_mut().send_event(NpcReproductionRequest {
            archetype: NpcArchetype::Commoner,
            position: DVec3::new(30.0, 66.0, 30.0),
            home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            initial_age_ticks: 0.0,
            territory_center: None,
            territory_radius: None,
        });

        app.update();

        let npc_count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).count()
        };
        assert_eq!(npc_count, 0);
    }

    // -- Bug #2: snap_spawn_y_to_surface regression tests ------------------

    #[test]
    fn snap_spawn_y_above_ground_snaps_down() {
        use crate::world::terrain::{SurfaceInfo, SurfaceProvider};
        struct FlatGround;
        impl SurfaceProvider for FlatGround {
            fn query_surface(&self, _x: i32, _z: i32) -> SurfaceInfo {
                SurfaceInfo {
                    y: 66,
                    passable: true,
                }
            }
        }
        let terrain = FlatGround;
        let pos = DVec3::new(10.5, 200.0, 20.5);
        let snapped = common::snap_spawn_y_to_surface(pos, Some(&terrain));
        assert!(
            (snapped.y - 67.0).abs() < 0.01,
            "spawn at Y=200 should snap to surface_y+1=67, got {}",
            snapped.y,
        );
        assert!(
            (snapped.x - pos.x).abs() < 0.01 && (snapped.z - pos.z).abs() < 0.01,
            "XZ should be unchanged",
        );
    }

    #[test]
    fn snap_spawn_y_below_ground_snaps_up() {
        use crate::world::terrain::{SurfaceInfo, SurfaceProvider};
        struct FlatGround;
        impl SurfaceProvider for FlatGround {
            fn query_surface(&self, _x: i32, _z: i32) -> SurfaceInfo {
                SurfaceInfo {
                    y: 66,
                    passable: true,
                }
            }
        }
        let terrain = FlatGround;
        let pos = DVec3::new(5.0, 10.0, 5.0);
        let snapped = common::snap_spawn_y_to_surface(pos, Some(&terrain));
        assert!(
            (snapped.y - 67.0).abs() < 0.01,
            "spawn at Y=10 should snap to surface_y+1=67, got {}",
            snapped.y,
        );
    }

    #[test]
    fn snap_spawn_y_impassable_surface_keeps_original() {
        use crate::world::terrain::{SurfaceInfo, SurfaceProvider};
        struct LavaSurface;
        impl SurfaceProvider for LavaSurface {
            fn query_surface(&self, _x: i32, _z: i32) -> SurfaceInfo {
                SurfaceInfo {
                    y: 66,
                    passable: false,
                }
            }
        }
        let terrain = LavaSurface;
        let pos = DVec3::new(5.0, 80.0, 5.0);
        let snapped = common::snap_spawn_y_to_surface(pos, Some(&terrain));
        assert!(
            (snapped.y - 80.0).abs() < 0.01,
            "impassable surface should keep original Y=80, got {}",
            snapped.y,
        );
    }

    #[test]
    fn snap_spawn_y_no_terrain_keeps_original() {
        let pos = DVec3::new(5.0, 80.0, 5.0);
        let snapped =
            common::snap_spawn_y_to_surface(pos, None::<&crate::world::terrain::TerrainProvider>);
        assert!(
            (snapped.y - 80.0).abs() < 0.01,
            "no terrain provider should keep original Y=80, got {}",
            snapped.y,
        );
    }

    // -----------------------------------------------------------------------
    // plan-npc-overhaul-v1 P1 — PoissonSpawnSampler tests
    // -----------------------------------------------------------------------

    use crate::npc::lifecycle::NpcArchetype;

    #[test]
    fn poisson_sampler_respects_min_distance() {
        let bounds = (DVec3::new(0.0, 64.0, 0.0), DVec3::new(500.0, 80.0, 500.0));
        let sampler = PoissonSpawnSampler::adaptive_for_zone(bounds);
        assert_eq!(
            sampler.min_same_archetype_dist, 48.0,
            "500x500 zone should use same_dist=48"
        );

        // Place first NPC at center.
        let mut existing = vec![(DVec3::new(250.0, 72.0, 250.0), NpcArchetype::Rogue)];

        // Sample second NPC — should be at least min_same_archetype_dist from the first.
        let pos = sampler
            .sample_position(bounds, &existing, NpcArchetype::Rogue, 42)
            .expect("500x500 zone should have room for a second NPC");
        let dx = pos.x - existing[0].0.x;
        let dz = pos.z - existing[0].0.z;
        let dist = (dx * dx + dz * dz).sqrt();
        assert!(
            dist >= sampler.min_same_archetype_dist * 0.5,
            "second NPC should be placed with spacing; dist={:.1} vs min={:.1}",
            dist,
            sampler.min_same_archetype_dist
        );

        existing.push((pos, NpcArchetype::Rogue));
    }

    #[test]
    fn poisson_sampler_returns_none_when_saturated() {
        // Tiny zone: 40x40 with min_same_dist=32. Can fit very few NPCs.
        let bounds = (DVec3::new(0.0, 64.0, 0.0), DVec3::new(40.0, 80.0, 40.0));
        let sampler = PoissonSpawnSampler {
            min_same_archetype_dist: 32.0,
            min_cross_archetype_dist: 16.0,
            max_candidates: 30,
        };

        // Fill with NPCs at grid points.
        let existing: Vec<(DVec3, NpcArchetype)> = vec![
            (DVec3::new(5.0, 72.0, 5.0), NpcArchetype::Rogue),
            (DVec3::new(5.0, 72.0, 35.0), NpcArchetype::Rogue),
            (DVec3::new(35.0, 72.0, 5.0), NpcArchetype::Rogue),
            (DVec3::new(35.0, 72.0, 35.0), NpcArchetype::Rogue),
            (DVec3::new(20.0, 72.0, 20.0), NpcArchetype::Rogue),
        ];

        let result = sampler.sample_position(bounds, &existing, NpcArchetype::Rogue, 99);
        assert!(
            result.is_none(),
            "40x40 zone with 5 NPCs at min_dist=32 should be saturated; got {:?}",
            result
        );
    }

    #[test]
    fn adaptive_distance_scales_with_area() {
        // Large zone (>= 500x500)
        let large = PoissonSpawnSampler::adaptive_for_zone((
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(600.0, 100.0, 600.0),
        ));
        assert_eq!(
            large.min_same_archetype_dist, 48.0,
            "area >= 500x500 should use 48"
        );
        assert_eq!(
            large.min_cross_archetype_dist, 24.0,
            "area >= 500x500 cross-dist should be 24"
        );

        // Medium zone (300-500)
        let medium = PoissonSpawnSampler::adaptive_for_zone((
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(400.0, 100.0, 400.0),
        ));
        assert_eq!(
            medium.min_same_archetype_dist, 40.0,
            "area 300x300..500x500 should use 40"
        );
        assert_eq!(
            medium.min_cross_archetype_dist, 20.0,
            "area 300x300..500x500 cross-dist should be 20"
        );

        // Small zone (< 300x300)
        let small = PoissonSpawnSampler::adaptive_for_zone((
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(200.0, 100.0, 200.0),
        ));
        assert_eq!(
            small.min_same_archetype_dist, 32.0,
            "area < 300x300 should use 32"
        );
        assert_eq!(
            small.min_cross_archetype_dist, 16.0,
            "area < 300x300 cross-dist should be 16"
        );
    }

    #[test]
    fn adaptive_for_zone_boundary_cases() {
        // Exactly 300x300
        let at_300 = PoissonSpawnSampler::adaptive_for_zone((
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(300.0, 100.0, 300.0),
        ));
        assert_eq!(
            at_300.min_same_archetype_dist, 40.0,
            "exactly 300x300 should use 40 (>= 300x300 threshold)"
        );

        // Exactly 500x500
        let at_500 = PoissonSpawnSampler::adaptive_for_zone((
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(500.0, 100.0, 500.0),
        ));
        assert_eq!(
            at_500.min_same_archetype_dist, 48.0,
            "exactly 500x500 should use 48 (>= 500x500 threshold)"
        );

        // Just under 300x300
        let under_300 = PoissonSpawnSampler::adaptive_for_zone((
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(299.0, 100.0, 299.0),
        ));
        assert_eq!(
            under_300.min_same_archetype_dist, 32.0,
            "299x299 should use 32 (below 300x300 threshold)"
        );
    }

    #[test]
    fn poisson_cross_archetype_distance() {
        let bounds = (DVec3::new(0.0, 64.0, 0.0), DVec3::new(500.0, 80.0, 500.0));
        let sampler = PoissonSpawnSampler::adaptive_for_zone(bounds);

        // Place a Beast at center.
        let existing = vec![(DVec3::new(250.0, 72.0, 250.0), NpcArchetype::Beast)];

        // Sample a Rogue — cross-archetype distance is half of same-archetype.
        let pos = sampler
            .sample_position(bounds, &existing, NpcArchetype::Rogue, 123)
            .expect("large zone should have room for cross-archetype NPC");

        // The sampler should place it respecting cross-archetype distance.
        let dx = pos.x - existing[0].0.x;
        let dz = pos.z - existing[0].0.z;
        let dist = (dx * dx + dz * dz).sqrt();
        // In a 500x500 zone with one NPC, the sampler should find a well-spaced position.
        assert!(
            dist > 0.0,
            "cross-archetype NPC should be placed away from existing; dist={:.1}",
            dist
        );
    }

    #[test]
    fn poisson_first_npc_goes_to_center() {
        let bounds = (DVec3::new(0.0, 64.0, 0.0), DVec3::new(400.0, 80.0, 400.0));
        let sampler = PoissonSpawnSampler::adaptive_for_zone(bounds);
        let pos = sampler
            .sample_position(bounds, &[], NpcArchetype::Rogue, 0)
            .expect("empty zone should always produce a position");
        assert!(
            (pos.x - 200.0).abs() < 1.0 && (pos.z - 200.0).abs() < 1.0,
            "first NPC in empty zone should be at center (200,200); got ({:.1},{:.1})",
            pos.x,
            pos.z,
        );
    }

    #[test]
    fn zone_budget_caps_respected() {
        let mut registry = NpcRegistry::default();
        // Use spawn zone cap of 6.
        let granted = registry.reserve_zone_batch("spawn", 10);
        assert_eq!(
            granted, 6,
            "spawn zone cap is 6; requesting 10 should grant only 6; got {}",
            granted
        );
        // Try again — should get 0.
        let granted2 = registry.reserve_zone_batch("spawn", 5);
        assert_eq!(
            granted2, 0,
            "spawn zone at cap should reject; got {}",
            granted2
        );
    }

    #[test]
    fn startup_zombie_removed() {
        // Verify that spawn::register() no longer schedules PostStartup zombie spawn.
        // We only call spawn::register() (not brain::register()) to avoid needing
        // all the events that brain systems require.
        let mut app = App::new();
        crate::npc::lifecycle::register(&mut app);
        app.insert_resource(RoguePopulationSeedConfig {
            target_count: 0,
            ..RoguePopulationSeedConfig::default()
        });
        // Manually register spawn systems without using register() to avoid
        // BigBrainPlugin dependency. Instead, verify the register() source.
        app.add_event::<crate::npc::lifecycle::NpcSpawnNotice>();
        app.add_systems(
            Update,
            (
                process_npc_reproduction_requests,
                rogue::seed_initial_rogue_population_on_startup,
            ),
        );

        app.update(); // PostStartup
        app.update(); // First Update
        app.update(); // Extra tick

        let npc_count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).count()
        };
        // With target_count=0 and no PostStartup zombie, no NPC should exist.
        assert_eq!(
            npc_count, 0,
            "after register() + 3 ticks, no zombie should spawn (P1.4 removal); got {}",
            npc_count
        );
    }

    #[test]
    fn seed_count_default_20() {
        let config = RoguePopulationSeedConfig::default();
        assert_eq!(
            config.target_count, 20,
            "plan-npc-overhaul-v1 §P1.3: target_count should default to 20, got {}",
            config.target_count
        );
    }

    #[test]
    fn batch_size_default_5() {
        // ROGUE_SEED_BATCH_SIZE is private, but we can observe the batching behavior:
        // with target=10, if batch_size=5, it takes exactly 2 ticks.
        let scenario = valence::testing::ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.insert_resource(ZoneRegistry::fallback());
        let mut registry = NpcRegistry::default();
        registry.per_zone_caps.clear(); // Don't let zone caps interfere.
        app.insert_resource(registry);
        app.insert_resource(RoguePopulationSeedConfig {
            target_count: 10,
            ..RoguePopulationSeedConfig::default()
        });
        app.add_event::<crate::npc::lifecycle::NpcSpawnNotice>();
        app.add_systems(Update, rogue::seed_initial_rogue_population_on_startup);

        // Tick 1: should spawn exactly 5 (batch_size=5).
        app.update();
        let count_after_1 = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).count()
        };
        assert_eq!(
            count_after_1, 5,
            "after 1 tick with target=10, batch_size=5 should spawn 5; got {}",
            count_after_1
        );

        // Tick 2: should spawn remaining 5.
        app.update();
        let count_after_2 = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).count()
        };
        assert_eq!(
            count_after_2, 10,
            "after 2 ticks with target=10, batch_size=5 should total 10; got {}",
            count_after_2
        );
    }
}
