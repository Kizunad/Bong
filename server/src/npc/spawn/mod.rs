pub mod ambient_scheduler;
mod beast;
pub(crate) mod common;
mod commoner;
mod disciple;
mod mundane;
mod rogue;
mod zombie;

use big_brain::prelude::{BigBrainSet, HasThinker, ThinkerBuilder};
use valence::prelude::{
    App, Commands, DVec3, Despawned, Entity, EventReader, EventWriter, IntoSystemConfigs, Last,
    PreUpdate, Query, ResMut, Update, With, Without, World,
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
    DecoyTarget, DuelTarget, NpcBlackboard, NpcCombatLoadout, NpcMarker, NpcMeleeArchetype,
    NpcMeleeProfile, NpcSkinSpawnContext,
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
    reserve_zone_distribution, spawn_rogue_npc_at, spawn_scattered_cultivator_at,
};

#[allow(unused_imports)]
pub(crate) use self::commoner::spawn_commoner_npc_at;

#[allow(unused_imports)]
pub(crate) use self::beast::{spawn_beast_npc_at, spawn_beast_npc_of_kind_at};

#[allow(unused_imports)]
pub(crate) use self::mundane::spawn_mundane_fauna_at;

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
        )
        // B0003 竞态守卫：在 big_brain attach 之前剥掉将死 NPC 的 `ThinkerBuilder`。
        // 根因与机制见 `strip_brain_from_despawning_npcs_system` 的 doc comment。
        .add_systems(
            Last,
            strip_brain_from_despawning_npcs_system.before(BigBrainSet::Cleanup),
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
        if matches!(tier, Some(NpcLodTier::Dormant)) {
            continue;
        }
        commands
            .entity(entity)
            .remove::<common::DeferredNpcBrain>()
            .insert(deferred.build());
    }
}

// ---------------------------------------------------------------------------
// System: strip_brain_from_despawning_npcs_system
// ---------------------------------------------------------------------------

/// 「将死且未 attach」NPC 的精确过滤器：持 `NpcMarker` + 已标 `Despawned` + 持 `ThinkerBuilder`
/// + 尚无 `HasThinker`。strip 系统与回归测试的 probe 共用，确保两侧锁的是同一不变量。
type DespawningBareThinkerFilter = (
    With<common::NpcMarker>,
    With<Despawned>,
    With<ThinkerBuilder>,
    Without<HasThinker>,
);

/// 剥掉「已标记 `Despawned` 且尚未拿到 `HasThinker`」的 NPC 的 `ThinkerBuilder`。
///
/// big_brain 的 `thinker_component_attach_system`（`BigBrainSet::Cleanup`）和 valence 的
/// `despawn_marked_entities` 都在 `Last` schedule。一旦某 NPC 同 tick 内既被
/// `attach_deferred_npc_brain_system` 唤醒插上 `ThinkerBuilder`、又被 `dehydrate_far_npcs_system`
/// 标记 `Despawned`，valence 在 `Last` 把它真正删除，而 big_brain 的 attach query 是
/// `Without<HasThinker>` 但**不带** `Without<Despawned>`——仍会命中这个将死实体并对它
/// `insert(HasThinker)` → Bevy `B0003` panic（“Could not insert a bundle … because it doesn't
/// exist in this World”），整个 server 崩溃。
///
/// 本系统在 `BigBrainSet::Cleanup` 之前先移除 `ThinkerBuilder`，让 attach query 不再命中——
/// 覆盖所有 despawn-mark 路径（dehydrate / scenario / war / …）。
///
/// **为何用 exclusive `&mut World` 系统**：`Commands::remove` 是延迟命令；虽然 Bevy 0.14 的
/// `auto_insert_apply_deferred`（默认开）会在「有 deferred 的系统 → 下游系统」的依赖边上自动插入
/// sync point（故 `.before(BigBrainSet::Cleanup)` 配 `Commands` 版本其实也正确），但这条 crash
/// 关键路径不依赖那一层隐式行为。exclusive 系统直接改 `World`，移除**立即生效**，进入 Cleanup 时
/// `ThinkerBuilder` 必已不在——与 `auto_insert_apply_deferred` 配置、线程数都无关。
///
/// 仅作用于 `Without<HasThinker>` 的将死实体：已 attach（持 `HasThinker`）的 NPC 走 big_brain
/// `actor_gone_cleanup` 常规清理，不在此处干预。
fn strip_brain_from_despawning_npcs_system(world: &mut World) {
    let mut query = world.query_filtered::<Entity, DespawningBareThinkerFilter>();
    let doomed: Vec<Entity> = query.iter(world).collect();
    for entity in doomed {
        world.entity_mut(entity).remove::<ThinkerBuilder>();
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

#[cfg(test)]
#[path = "../spawn_tests.rs"]
mod tests;
