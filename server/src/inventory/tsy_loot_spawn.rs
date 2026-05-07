//! plan-tsy-loot-v1 §2 — 99/1 Loot 的 1% 上古遗物 spawn。
//!
//! 触发：玩家首次踏进某 TSY family（`TsyEnterEmit`）→ 检查 `TsySpawnedFamilies`，
//! 未 spawn 过则按 source class 抽取 3-5 件 `AncientRelicTemplate`，挂在 mid/deep 层
//! 的 patrol_anchors 附近 + 注入 `DroppedLootRegistry.entries`。
//!
//! 99% 凡物部分**不在本系统**：由秘境内死亡（`tsy_death_drop.rs`）的自然累积形成
//! （见 plan §2.4）。
//!
//! Source class 推导（MVP 简化）：family_id 字符串 hash → 3 路均匀分布。真正的
//! "这座 TSY 来源是大能陨落 / 宗门遗迹 / 战场沉淀" 由 `worldview §十六.一` 生命周期
//! 决定，待 P2 lifecycle / 后续 worldgen 把真实 source 写到 zone metadata。

use std::collections::{hash_map::DefaultHasher, HashSet};
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};
use valence::math::DVec3;
use valence::prelude::bevy_ecs::{
    event::EventReader,
    system::{Res, ResMut},
};
use valence::prelude::{bevy_ecs, Resource};

#[cfg(test)]
use super::ancient_relics::seed_ancient_relics;
use super::ancient_relics::{AncientRelicPool, AncientRelicSource};
use super::{DroppedLootEntry, DroppedLootRegistry, InventoryInstanceIdAllocator};
use crate::combat::CombatClock;
use crate::world::dimension::DimensionKind;
use crate::world::tsy_lifecycle::{on_first_enter, TsyZoneStateRegistry};
use crate::world::tsy_portal::TsyEnterEmit;
use crate::world::zone::{TsyDepth, ZoneRegistry};

/// 已 spawn 过遗物的 TSY family id 集合。系统重启即清空（与"干尸不持久化"一致），
/// 重启后玩家重新进入 → 重新 spawn 一批；这是 MVP 的有意为之，后续 P2 lifecycle plan
/// 会把"哪些遗物已被取走"写入持久化层来打破这个简化。
#[derive(Debug, Default, Resource, Serialize, Deserialize)]
pub struct TsySpawnedFamilies {
    pub families: HashSet<String>,
}

/// MVP 每座 family spawn 的遗物总数。Source 决定区间：
/// - DaoLord：3-5（少而精）
/// - SectRuins：5-10（最多）
/// - BattleSediment：2-4（散乱）
///
/// 最终数 = `min + (seed % (max - min + 1))`。
pub fn relic_count_for_source(source: AncientRelicSource, seed: u64) -> u32 {
    let (min, max): (u32, u32) = match source {
        AncientRelicSource::DaoLord => (3, 5),
        AncientRelicSource::SectRuins => (5, 10),
        AncientRelicSource::BattleSediment => (2, 4),
    };
    let span = max - min + 1;
    min + (seed % span as u64) as u32
}

/// 把 spawn 总数分到 (shallow, mid, deep)。MVP：全部进 mid+deep；浅层留给后续
/// 玩家自然死亡累积。返回 `(0, mid_count, deep_count)`，确保 mid+deep == count。
pub fn layer_distribution(count: u32) -> (u32, u32, u32) {
    if count == 0 {
        return (0, 0, 0);
    }
    // deep 偏多（"越深越凶险，遗物越好")：deep ≈ ⌈2count/3⌉，mid 取剩。
    let deep = count.div_ceil(3) * 2;
    let deep = deep.min(count);
    let mid = count - deep;
    (0, mid, deep)
}

/// 从 family_id 推导 source class（MVP 简化）。
pub fn source_class_from_family_id(family_id: &str) -> AncientRelicSource {
    let mut hasher = DefaultHasher::new();
    family_id.hash(&mut hasher);
    match hasher.finish() % 3 {
        0 => AncientRelicSource::DaoLord,
        1 => AncientRelicSource::SectRuins,
        _ => AncientRelicSource::BattleSediment,
    }
}

/// 为某 family 在指定 depth 上挑一个落点。基于 patrol_anchors + 小偏移；
/// 若该 depth 没有 zone（世界尚未生成）→ 返回 None，跳过这次 spawn 即可。
pub fn sample_position_in_layer(
    zones: &ZoneRegistry,
    family_id: &str,
    depth: TsyDepth,
    seed: u64,
) -> Option<DVec3> {
    // 找该 family 在指定 depth 上的 zone
    let zone = zones.zones.iter().find(|z| {
        z.tsy_family_id().as_deref() == Some(family_id) && z.tsy_depth() == Some(depth)
    })?;
    if zone.patrol_anchors.is_empty() {
        // patrol_anchors 没配 → 用 zone 中心
        let (lo, hi) = zone.bounds;
        return Some(DVec3::new(
            (lo.x + hi.x) * 0.5,
            (lo.y + hi.y) * 0.5,
            (lo.z + hi.z) * 0.5,
        ));
    }
    let idx = (seed as usize) % zone.patrol_anchors.len();
    let anchor = zone.patrol_anchors[idx];
    // 小偏移避免堆同一格（±2 ticks 内）
    let off_x = ((seed.wrapping_mul(31)) % 5) as f64 - 2.0;
    let off_z = ((seed.wrapping_mul(37) >> 8) % 5) as f64 - 2.0;
    Some(DVec3::new(anchor.x + off_x, anchor.y, anchor.z + off_z))
}

/// 系统：监听 `TsyEnterEmit`，为未 spawn 过的 family 注入 1% 上古遗物。
///
/// 同时把 family 注册到 `TsyZoneStateRegistry`（如未注册）并把刚 spawn 的 instance_id
/// 列表回写到 `state.initial_skeleton` —— 让 plan-tsy-lifecycle-v1 §1.4 状态机能正确
/// 跟踪"剩余骨架"。即便 zone 未 ready 没放下任何遗物，也会先把 family 注册成 Active
/// 状态（避免 lifecycle 看不见 family）。
#[allow(clippy::too_many_arguments)]
pub fn tsy_loot_spawn_on_enter(
    mut events: EventReader<TsyEnterEmit>,
    zones: Res<ZoneRegistry>,
    relic_pool: Res<AncientRelicPool>,
    mut spawned: ResMut<TsySpawnedFamilies>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut drops: ResMut<DroppedLootRegistry>,
    mut lifecycle: ResMut<TsyZoneStateRegistry>,
    clock: Res<CombatClock>,
) {
    for ev in events.read() {
        // 任何"玩家踏进 family"都先把 family 注册到 lifecycle registry —— 即便
        // 本 tick 没真正放下遗物（zones 未 ready）也保证 lifecycle 有 anchor 可查。
        on_first_enter(&mut lifecycle, &ev.family_id, ev.return_to, clock.tick);

        // 已经 spawn 过本 family → 跳。
        if spawned.families.contains(&ev.family_id) {
            continue;
        }
        let source = source_class_from_family_id(&ev.family_id);
        let seed = family_seed(&ev.family_id);
        let count = relic_count_for_source(source, seed);
        let (_shallow, mid_count, deep_count) = layer_distribution(count);

        let mut placed_ids: Vec<u64> = Vec::new();
        spawn_for_layer(
            &ev.family_id,
            TsyDepth::Mid,
            mid_count,
            source,
            seed,
            &zones,
            &relic_pool,
            &mut allocator,
            &mut drops,
            &mut placed_ids,
        );
        let placed_mid = placed_ids.len() as u32;
        spawn_for_layer(
            &ev.family_id,
            TsyDepth::Deep,
            deep_count,
            source,
            seed.wrapping_mul(2654435761),
            &zones,
            &relic_pool,
            &mut allocator,
            &mut drops,
            &mut placed_ids,
        );
        let placed_deep = placed_ids.len() as u32 - placed_mid;

        // Codex review #2 修复：mid/deep zone 还没 ready 时（worldgen 慢于
        // 玩家入场）不要把 family 标记成 spawned，否则后续入场全 skip → family
        // 永远缺 relics。只有真正放下 ≥1 件后才记账。
        if placed_ids.is_empty() {
            tracing::debug!(
                family = %ev.family_id,
                "[bong][tsy-loot] no relics placed (zones not ready) — leaving family un-marked for retry"
            );
            continue;
        }
        spawned.families.insert(ev.family_id.clone());
        // plan-tsy-lifecycle-v1 §1.5 — spawn 完成后回写初始骨架；mark_initial_skeleton
        // 内部 idempotent（仅在 vec 为空时写入），所以重入不会污染状态。
        lifecycle.mark_initial_skeleton(&ev.family_id, placed_ids.clone());

        tracing::info!(
            family = %ev.family_id,
            source = ?source,
            requested = count,
            placed = placed_ids.len(),
            mid = placed_mid,
            deep = placed_deep,
            "[bong][tsy-loot] spawned ancient relics on first family entry"
        );
    }
}

fn family_seed(family_id: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    family_id.hash(&mut hasher);
    hasher.finish()
}

/// 在指定 depth 上 spawn `count` 件遗物。把 placed 的 `instance_id` 追加到
/// `placed_ids`（caller 负责跨层共享同一 vec，方便回写到 lifecycle.initial_skeleton）。
/// zone/池缺失时可能放下 < count，由 caller 判断是否标记 family 为已 spawn。
#[allow(clippy::too_many_arguments)]
fn spawn_for_layer(
    family_id: &str,
    depth: TsyDepth,
    count: u32,
    source: AncientRelicSource,
    base_seed: u64,
    zones: &ZoneRegistry,
    relic_pool: &AncientRelicPool,
    allocator: &mut InventoryInstanceIdAllocator,
    drops: &mut DroppedLootRegistry,
    placed_ids: &mut Vec<u64>,
) {
    for i in 0..count {
        let seed = base_seed.wrapping_add(i as u64).wrapping_mul(0x9E37_79B9);
        let Some(template) = relic_pool.sample(source, seed) else {
            tracing::warn!(
                "[bong][tsy-loot] empty relic pool — skipping spawn for family={family_id} depth={depth:?}"
            );
            return;
        };
        let Some(pos) = sample_position_in_layer(zones, family_id, depth, seed) else {
            tracing::debug!(
                family = family_id,
                ?depth,
                "[bong][tsy-loot] no zone at this depth yet — skipping {} drops",
                count - i
            );
            return;
        };
        let instance = match template.to_item_instance(allocator) {
            Ok(item) => item,
            Err(err) => {
                tracing::warn!(
                    "[bong][tsy-loot] allocator overflow / failure: {err}; aborting layer"
                );
                return;
            }
        };
        let entry = DroppedLootEntry {
            instance_id: instance.instance_id,
            source_container_id: format!("tsy_spawn:{family_id}"),
            source_row: 0,
            source_col: 0,
            world_pos: [pos.x, pos.y, pos.z],
            dimension: DimensionKind::Tsy,
            item: instance,
        };
        placed_ids.push(entry.instance_id);
        drops.entries.insert(entry.instance_id, entry);
    }
}

/// 测试辅助：返回 seed 起始时的 ancient relics 总池大小。
#[cfg(test)]
fn seed_pool_count() -> usize {
    seed_ancient_relics().len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::dimension::DimensionKind;
    use crate::world::zone::Zone;

    fn make_tsy_zone(name: &str, anchors: Vec<DVec3>) -> Zone {
        Zone {
            name: name.into(),
            dimension: DimensionKind::Tsy,
            bounds: (DVec3::new(-32.0, 0.0, -32.0), DVec3::new(32.0, 64.0, 32.0)),
            spirit_qi: 0.0,
            danger_level: 5,
            active_events: vec!["tsy_entry".into()],
            patrol_anchors: anchors,
            blocked_tiles: vec![],
        }
    }

    #[test]
    fn relic_count_for_source_within_expected_range() {
        for seed in 0..32u64 {
            let n = relic_count_for_source(AncientRelicSource::DaoLord, seed);
            assert!((3..=5).contains(&n), "DaoLord: {n}");
            let n = relic_count_for_source(AncientRelicSource::SectRuins, seed);
            assert!((5..=10).contains(&n), "SectRuins: {n}");
            let n = relic_count_for_source(AncientRelicSource::BattleSediment, seed);
            assert!((2..=4).contains(&n), "BattleSediment: {n}");
        }
    }

    #[test]
    fn layer_distribution_sums_to_count_and_skips_shallow() {
        for n in [0u32, 1, 3, 5, 7, 10] {
            let (s, m, d) = layer_distribution(n);
            assert_eq!(s + m + d, n);
            assert_eq!(s, 0, "MVP 浅层不 spawn 遗物");
            if n > 0 {
                assert!(d >= m, "deep 应不少于 mid（越深越凶险越值钱）");
            }
        }
    }

    #[test]
    fn source_class_from_family_id_is_deterministic() {
        let a = source_class_from_family_id("tsy_lingxu_01");
        let b = source_class_from_family_id("tsy_lingxu_01");
        assert_eq!(a, b);
    }

    #[test]
    fn source_class_distribution_uses_all_three_over_many_families() {
        let mut seen = HashSet::new();
        for i in 0..100 {
            seen.insert(source_class_from_family_id(&format!("tsy_test_{i:03}")));
        }
        assert!(
            seen.len() == 3,
            "100 个 family 应覆盖全部 3 个 source class，实际 {}",
            seen.len()
        );
    }

    #[test]
    fn sample_position_uses_patrol_anchor_when_available() {
        let zone = make_tsy_zone(
            "tsy_lingxu_01_mid",
            vec![DVec3::new(10.0, 64.0, 20.0), DVec3::new(-5.0, 64.0, 30.0)],
        );
        let zones = ZoneRegistry { zones: vec![zone] };
        let pos = sample_position_in_layer(&zones, "tsy_lingxu_01", TsyDepth::Mid, 0)
            .expect("Mid zone exists");
        // 偏移 ±2 之内
        assert!((pos.x - 10.0).abs() <= 2.0 || (pos.x + 5.0).abs() <= 2.0);
        assert_eq!(pos.y, 64.0);
    }

    #[test]
    fn sample_position_returns_none_when_layer_missing() {
        let zone = make_tsy_zone("tsy_other_01_mid", vec![DVec3::new(0.0, 64.0, 0.0)]);
        let zones = ZoneRegistry { zones: vec![zone] };
        let pos = sample_position_in_layer(&zones, "tsy_lingxu_01", TsyDepth::Mid, 0);
        assert!(pos.is_none(), "family 不匹配应返回 None");
    }

    #[test]
    fn sample_position_falls_back_to_center_when_no_anchors() {
        let zone = make_tsy_zone("tsy_lingxu_01_deep", vec![]);
        let zones = ZoneRegistry { zones: vec![zone] };
        let pos = sample_position_in_layer(&zones, "tsy_lingxu_01", TsyDepth::Deep, 0)
            .expect("Deep zone exists");
        // bounds 中心：x=0, y=32, z=0
        assert_eq!(pos, DVec3::new(0.0, 32.0, 0.0));
    }

    #[test]
    fn relic_pool_seeded_for_tests() {
        // 保 seed_pool_count 在 §1 调整种子表时不被遗忘。
        assert!(seed_pool_count() >= 8);
    }

    #[test]
    fn family_seed_deterministic() {
        assert_eq!(family_seed("tsy_a"), family_seed("tsy_a"));
        assert_ne!(family_seed("tsy_a"), family_seed("tsy_b"));
    }
}
