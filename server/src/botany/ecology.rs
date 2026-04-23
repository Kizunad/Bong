//! plan-botany-v1 §7 生态可视化：定期聚合 zone × plant × variant 为快照。
//!
//! 两路输出：
//! - `tracing::info!` 一条结构化日志（A：运维观测）
//! - `RedisBridgeResource.tx_outbound` 发 `BotanyEcology` 到 channel `bong:botany/ecology`
//!   （B：天道 agent 消费做全局灵气重分配决策）

use std::collections::BTreeMap;

use valence::prelude::{Query, Res, With};

use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::player::gameplay::GameplayTick;
use crate::schema::botany::{
    BotanyEcologySnapshotV1, BotanyPlantCountEntryV1, BotanyVariantCountEntryV1, BotanyVariantV1,
    BotanyZoneEcologyV1,
};
use crate::world::zone::{Zone, ZoneRegistry};

use super::components::Plant;
use super::registry::PlantVariant;

type ZonePlantCounts = (BTreeMap<String, u64>, BTreeMap<u8, u64>);

/// 默认每 600 tick（~30s @ 20tps）发一次。避免每 tick 聚合 × publish。
pub const ECOLOGY_EMIT_INTERVAL_TICKS: u64 = 600;

pub fn emit_botany_ecology_snapshot(
    gameplay_tick: Option<Res<GameplayTick>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    redis: Option<Res<RedisBridgeResource>>,
    plants: Query<&Plant, With<Plant>>,
) {
    let Some(gameplay_tick) = gameplay_tick else {
        return;
    };
    let now = gameplay_tick.current_tick();
    if now == 0 || !now.is_multiple_of(ECOLOGY_EMIT_INTERVAL_TICKS) {
        return;
    }
    let Some(zone_registry) = zone_registry else {
        return;
    };

    let snapshot = aggregate(now, &zone_registry.zones, plants.iter());

    // A: 运维观测日志（每个 zone 一行，便于 grep / 结构化采集）
    for zone_eco in &snapshot.zones {
        tracing::info!(
            "[bong][botany] ecology tick={} zone={} spirit_qi={:.3} plant_counts={} variant_counts={}",
            snapshot.tick,
            zone_eco.zone,
            zone_eco.spirit_qi,
            serde_json::to_string(&zone_eco.plant_counts).unwrap_or_else(|_| "[]".to_string()),
            serde_json::to_string(&zone_eco.variant_counts).unwrap_or_else(|_| "[]".to_string())
        );
    }

    // B: 发给 agent（Redis）。tx_outbound 无 backpressure 直接 try-send。
    if let Some(redis) = redis {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::BotanyEcology(snapshot));
    }
}

/// 纯函数：按 zone 聚合传入的 Plant 集合，输出 ecology snapshot。可测试。
pub fn aggregate<'a>(
    tick: u64,
    zones: &[Zone],
    plants: impl IntoIterator<Item = &'a Plant>,
) -> BotanyEcologySnapshotV1 {
    // zone → (kind_id → count, variant → count)。BTreeMap 保证输出排序稳定。
    let mut per_zone: BTreeMap<String, ZonePlantCounts> = BTreeMap::new();

    for plant in plants {
        let entry = per_zone.entry(plant.zone_name.clone()).or_default();
        *entry.0.entry(plant.id.as_str().to_string()).or_insert(0) += 1;
        *entry.1.entry(variant_key(plant.variant)).or_insert(0) += 1;
    }

    let zones_out: Vec<BotanyZoneEcologyV1> = zones
        .iter()
        .map(|z| {
            let (kinds, variants) = per_zone.remove(&z.name).unwrap_or_default();
            let plant_counts = kinds
                .into_iter()
                .map(|(kind, count)| BotanyPlantCountEntryV1 { kind, count })
                .collect();
            let variant_counts = variants
                .into_iter()
                .map(|(k, count)| BotanyVariantCountEntryV1 {
                    variant: variant_from_key(k),
                    count,
                })
                .collect();
            BotanyZoneEcologyV1 {
                zone: z.name.clone(),
                spirit_qi: z.spirit_qi,
                plant_counts,
                variant_counts,
            }
        })
        .collect();

    BotanyEcologySnapshotV1::new(tick, zones_out)
}

fn variant_key(v: PlantVariant) -> u8 {
    match v {
        PlantVariant::None => 0,
        PlantVariant::Thunder => 1,
        PlantVariant::Tainted => 2,
    }
}

fn variant_from_key(k: u8) -> BotanyVariantV1 {
    match k {
        1 => BotanyVariantV1::Thunder,
        2 => BotanyVariantV1::Tainted,
        _ => BotanyVariantV1::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::botany::registry::BotanyPlantId;
    use valence::prelude::Position;

    fn make_zone(name: &str, spirit_qi: f64) -> Zone {
        Zone {
            name: name.to_string(),
            bounds: (
                Position::new([0.0, 0.0, 0.0]).get(),
                Position::new([1.0, 1.0, 1.0]).get(),
            ),
            spirit_qi,
            danger_level: 1,
            active_events: vec![],
            patrol_anchors: vec![],
            blocked_tiles: vec![],
        }
    }

    fn make_plant(id: BotanyPlantId, zone: &str, variant: PlantVariant) -> Plant {
        Plant {
            id,
            zone_name: zone.to_string(),
            position: [0.0, 0.0, 0.0],
            planted_at_tick: 0,
            wither_progress: 0,
            source_point: None,
            harvested: false,
            trampled: false,
            variant,
        }
    }

    #[test]
    fn aggregate_counts_kinds_and_variants_per_zone() {
        let zones = vec![make_zone("spawn", 0.5), make_zone("blood_valley", -0.2)];
        let plants = vec![
            make_plant(BotanyPlantId::CiSheHao, "spawn", PlantVariant::None),
            make_plant(BotanyPlantId::CiSheHao, "spawn", PlantVariant::None),
            make_plant(BotanyPlantId::CiSheHao, "spawn", PlantVariant::Thunder),
            make_plant(BotanyPlantId::NingMaiCao, "spawn", PlantVariant::None),
            make_plant(
                BotanyPlantId::ChiSuiCao,
                "blood_valley",
                PlantVariant::Tainted,
            ),
        ];

        let snap = aggregate(1200, &zones, plants.iter());

        assert_eq!(snap.tick, 1200);
        assert_eq!(snap.zones.len(), 2);

        let spawn_eco = &snap.zones[0];
        assert_eq!(spawn_eco.zone, "spawn");
        assert!((spawn_eco.spirit_qi - 0.5).abs() < 1e-9);
        assert_eq!(spawn_eco.plant_counts.len(), 2);
        // BTreeMap 排序：ci_she_hao < ning_mai_cao
        assert_eq!(spawn_eco.plant_counts[0].kind, "ci_she_hao");
        assert_eq!(spawn_eco.plant_counts[0].count, 3);
        assert_eq!(spawn_eco.plant_counts[1].kind, "ning_mai_cao");
        assert_eq!(spawn_eco.plant_counts[1].count, 1);
        // variant 排序 key：None(0) < Thunder(1)
        assert_eq!(spawn_eco.variant_counts[0].variant, BotanyVariantV1::None);
        assert_eq!(spawn_eco.variant_counts[0].count, 3);
        assert_eq!(
            spawn_eco.variant_counts[1].variant,
            BotanyVariantV1::Thunder
        );
        assert_eq!(spawn_eco.variant_counts[1].count, 1);

        let bv_eco = &snap.zones[1];
        assert_eq!(bv_eco.zone, "blood_valley");
        assert_eq!(bv_eco.variant_counts[0].variant, BotanyVariantV1::Tainted);
        assert_eq!(bv_eco.variant_counts[0].count, 1);
    }

    #[test]
    fn aggregate_produces_empty_counts_for_zone_with_no_plants() {
        let zones = vec![make_zone("empty_zone", 0.3)];
        let snap = aggregate(100, &zones, std::iter::empty());
        assert_eq!(snap.zones.len(), 1);
        assert!(snap.zones[0].plant_counts.is_empty());
        assert!(snap.zones[0].variant_counts.is_empty());
    }

    #[test]
    fn aggregate_drops_plants_in_unregistered_zones() {
        // 植物 zone_name 不在 ZoneRegistry 中 → 被忽略（防止 agent 收到未知 zone）
        let zones = vec![make_zone("spawn", 0.5)];
        let plants = [
            make_plant(BotanyPlantId::CiSheHao, "spawn", PlantVariant::None),
            make_plant(BotanyPlantId::CiSheHao, "ghost_zone", PlantVariant::None),
        ];
        let snap = aggregate(1, &zones, plants.iter());
        assert_eq!(snap.zones.len(), 1);
        assert_eq!(snap.zones[0].plant_counts[0].count, 1);
    }
}
