//! plan-mundane-fauna-v1 P3 生态可视化：定期聚合 zone × 物种为凡兽快照。
//!
//! 两路输出（与 `botany::ecology` 同构）：
//! - `tracing::info!` 一条结构化日志（A：运维观测）
//! - `RedisBridgeResource.tx_outbound` 发 `FaunaEcology` 到 channel `bong:fauna/ecology`
//!   （B：天道 agent narration 信号——凡兽绝迹 / 兽群迁徙 = 灵气恶化的可感征兆）

use std::collections::BTreeMap;

use valence::prelude::{Query, Res};

use crate::fauna::mundane::{MundaneFaunaKind, MundaneFaunaMarker, MundaneFaunaSpecies};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::player::gameplay::GameplayTick;
use crate::schema::fauna_ecology::{
    FaunaEcologySnapshotV1, FaunaSpeciesCountEntryV1, FaunaZoneEcologyV1,
};
use crate::world::zone::{Zone, ZoneRegistry};

/// 默认每 600 tick（~30s @ 20tps）发一次。对齐 `botany::ecology::ECOLOGY_EMIT_INTERVAL_TICKS`。
pub const FAUNA_ECOLOGY_EMIT_INTERVAL_TICKS: u64 = 600;

pub fn emit_fauna_ecology_snapshot(
    gameplay_tick: Option<Res<GameplayTick>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    redis: Option<Res<RedisBridgeResource>>,
    fauna: Query<(&MundaneFaunaSpecies, &MundaneFaunaMarker)>,
) {
    let Some(gameplay_tick) = gameplay_tick else {
        return;
    };
    let now = gameplay_tick.current_tick();
    if now == 0 || !now.is_multiple_of(FAUNA_ECOLOGY_EMIT_INTERVAL_TICKS) {
        return;
    }
    let Some(zone_registry) = zone_registry else {
        return;
    };

    let snapshot = aggregate(
        now,
        &zone_registry.zones,
        fauna
            .iter()
            .map(|(species, marker)| (marker.home_zone.as_str(), species.0)),
    );

    // A: 运维观测日志（每个 zone 一行）
    for zone_eco in &snapshot.zones {
        tracing::info!(
            "[bong][fauna] ecology tick={} zone={} spirit_qi={:.3} species_counts={}",
            snapshot.tick,
            zone_eco.zone,
            zone_eco.spirit_qi,
            serde_json::to_string(&zone_eco.species_counts).unwrap_or_else(|_| "[]".to_string()),
        );
    }

    // B: 发给 agent（Redis）。tx_outbound 无 backpressure 直接 try-send。
    if let Some(redis) = redis {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::FaunaEcology(snapshot));
    }
}

/// 纯函数：按 zone 聚合传入的 (home_zone, kind) 集合，输出 ecology snapshot。可测试。
/// 只输出 `ZoneRegistry` 中登记的 zone；未登记 zone 的凡兽被丢弃（防 agent 收到未知 zone）。
pub fn aggregate<'a>(
    tick: u64,
    zones: &[Zone],
    fauna: impl IntoIterator<Item = (&'a str, MundaneFaunaKind)>,
) -> FaunaEcologySnapshotV1 {
    // zone → (kind_str → count)。BTreeMap 保证输出排序稳定。
    let mut per_zone: BTreeMap<String, BTreeMap<&'static str, u64>> = BTreeMap::new();

    for (home_zone, kind) in fauna {
        let entry = per_zone.entry(home_zone.to_string()).or_default();
        *entry.entry(kind.as_str()).or_insert(0) += 1;
    }

    let zones_out: Vec<FaunaZoneEcologyV1> = zones
        .iter()
        .map(|z| {
            let kinds = per_zone.remove(&z.name).unwrap_or_default();
            let species_counts = kinds
                .into_iter()
                .map(|(kind, count)| FaunaSpeciesCountEntryV1 {
                    kind: kind.to_string(),
                    count,
                })
                .collect();
            FaunaZoneEcologyV1 {
                zone: z.name.clone(),
                spirit_qi: z.spirit_qi,
                species_counts,
            }
        })
        .collect();

    FaunaEcologySnapshotV1::new(tick, zones_out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::Position;

    fn make_zone(name: &str, spirit_qi: f64) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (
                Position::new([0.0, 0.0, 0.0]).get(),
                Position::new([1.0, 1.0, 1.0]).get(),
            ),
            spirit_qi,
            danger_level: 1,
            active_events: vec![],
            patrol_anchors: vec![],
            blocked_tiles: vec![],
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }
    }

    #[test]
    fn aggregate_counts_species_per_zone() {
        let zones = vec![make_zone("spawn", 0.5), make_zone("blood_valley", -0.2)];
        let fauna = vec![
            ("spawn", MundaneFaunaKind::Chicken),
            ("spawn", MundaneFaunaKind::Chicken),
            ("spawn", MundaneFaunaKind::Rabbit),
            ("blood_valley", MundaneFaunaKind::Goat),
        ];

        let snap = aggregate(1200, &zones, fauna);

        assert_eq!(snap.tick, 1200);
        assert_eq!(snap.zones.len(), 2);

        let spawn_eco = &snap.zones[0];
        assert_eq!(spawn_eco.zone, "spawn");
        assert!((spawn_eco.spirit_qi - 0.5).abs() < 1e-9);
        // BTreeMap 排序：chicken < rabbit
        assert_eq!(spawn_eco.species_counts.len(), 2);
        assert_eq!(spawn_eco.species_counts[0].kind, "chicken");
        assert_eq!(spawn_eco.species_counts[0].count, 2);
        assert_eq!(spawn_eco.species_counts[1].kind, "rabbit");
        assert_eq!(spawn_eco.species_counts[1].count, 1);

        let bv_eco = &snap.zones[1];
        assert_eq!(bv_eco.zone, "blood_valley");
        assert_eq!(bv_eco.species_counts[0].kind, "goat");
        assert_eq!(bv_eco.species_counts[0].count, 1);
    }

    #[test]
    fn aggregate_produces_empty_counts_for_zone_with_no_fauna() {
        let zones = vec![make_zone("empty_zone", 0.3)];
        let snap = aggregate(100, &zones, std::iter::empty());
        assert_eq!(snap.zones.len(), 1);
        assert!(snap.zones[0].species_counts.is_empty());
    }

    #[test]
    fn aggregate_drops_fauna_in_unregistered_zones() {
        // home_zone 不在 ZoneRegistry 中 → 被忽略（防止 agent 收到未知 zone）。
        let zones = vec![make_zone("spawn", 0.5)];
        let fauna = vec![
            ("spawn", MundaneFaunaKind::Cow),
            ("ghost_zone", MundaneFaunaKind::Cow),
        ];
        let snap = aggregate(1, &zones, fauna);
        assert_eq!(snap.zones.len(), 1);
        assert_eq!(snap.zones[0].species_counts[0].kind, "cow");
        assert_eq!(snap.zones[0].species_counts[0].count, 1);
    }
}
