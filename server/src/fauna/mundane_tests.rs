use super::*;
use crate::world::dimension::DimensionKind;

fn zone_with_name(name: &str) -> Zone {
    zone_with_qi(name, 0.5)
}

fn zone_with_qi(name: &str, spirit_qi: f64) -> Zone {
    Zone {
        name: name.to_string(),
        dimension: DimensionKind::Overworld,
        bounds: (
            DVec3::new(-500.0, 0.0, -500.0),
            DVec3::new(500.0, 200.0, 500.0),
        ),
        spirit_qi,
        danger_level: 1,
        active_events: Vec::new(),
        patrol_anchors: Vec::new(),
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    }
}

#[test]
fn unknown_zone_name_falls_back_to_plains_pool() {
    assert_eq!(
        mundane_biome_pool("some_unmapped_zone_name"),
        PLAINS_POOL,
        "未映射的 zone 名应兜底走平原池，不能返回空池"
    );
}

#[test]
fn biome_pool_name_match_is_case_insensitive() {
    assert_eq!(mundane_biome_pool("QINGYUN_PEAKS"), PEAKS_POOL);
}

#[test]
fn select_mundane_species_deterministic_for_same_seed() {
    let pool = PLAINS_POOL;
    assert_eq!(
        select_mundane_species(pool, 777),
        select_mundane_species(pool, 777),
        "同一 seed 必须产出同一物种（可复现）"
    );
}

#[test]
fn select_mundane_species_exhaustive_over_plains_pool() {
    // PLAINS_POOL 5 条目，seed 0..5 应恰好遍历全部各一次（roll = seed % 5）。
    let pool = PLAINS_POOL;
    let mut hit: Vec<MundaneFaunaKind> = (0u64..5)
        .map(|seed| select_mundane_species(pool, seed).expect("非空池必命中"))
        .collect();
    hit.sort_by_key(|k| k.as_str());
    let mut expected: Vec<MundaneFaunaKind> = pool.to_vec();
    expected.sort_by_key(|k| k.as_str());
    assert_eq!(hit, expected);
}

#[test]
fn species_for_position_respects_biome_pool() {
    // north_wastes 池只含 兔/狐/狼，任意坐标结果都必须落在该集合内。
    for x in [0.0, 17.0, -42.5, 999.0] {
        let kind = mundane_species_for_position("north_wastes", DVec3::new(x, 64.0, 0.0));
        assert!(
            WASTES_POOL.contains(&kind),
            "north_wastes 池派生出的物种 {kind:?} 必须属于 {WASTES_POOL:?}"
        );
    }
}

#[test]
fn mundane_pool_fn_spawns_entity_with_species_matching_biome_pool() {
    let mut app = valence::prelude::App::new();
    let layer = app.world_mut().spawn_empty().id();
    let zone = zone_with_name("qingyun_peaks");
    let spawned = {
        let mut commands = app.world_mut().commands();
        mundane_pool_fn(
            &mut commands,
            layer,
            &zone,
            DVec3::new(10.0, 64.0, 10.0),
            DVec3::new(10.0, 64.0, 10.0),
            Season::Summer,
        )
    };
    app.world_mut().flush();
    let entity = spawned.expect("非死域/非塌缩 zone 应恒返回 Some");
    let species = app
        .world()
        .get::<crate::fauna::mundane::MundaneFaunaSpecies>(entity)
        .expect("mundane_pool_fn 产出的实体必须带 MundaneFaunaSpecies");
    assert!(
        PEAKS_POOL.contains(&species.0),
        "qingyun_peaks zone 产出的物种 {:?} 必须属于峰区池 {PEAKS_POOL:?}",
        species.0
    );
}

#[test]
fn select_mundane_species_weighted_is_deterministic() {
    let pool = MARSH_POOL;
    assert_eq!(
        select_mundane_species_weighted(pool, Season::Summer, 777),
        select_mundane_species_weighted(pool, Season::Summer, 777),
        "同 seed 必须复现同一物种"
    );
}

#[test]
fn select_mundane_species_weighted_biases_toward_frog_in_summer() {
    // MARSH_POOL = [Frog, Rabbit]，夏季 Frog 权重 3 : Rabbit 1 → total=4，
    // seed % 4 ∈ {0,1,2} 应命中 Frog（cumulative=3），只有 seed%4==3 命中 Rabbit。
    let pool = MARSH_POOL;
    let hits: Vec<MundaneFaunaKind> = (0u64..8)
        .map(|seed| select_mundane_species_weighted(pool, Season::Summer, seed).unwrap())
        .collect();
    let frog_count = hits
        .iter()
        .filter(|k| **k == MundaneFaunaKind::Frog)
        .count();
    let rabbit_count = hits
        .iter()
        .filter(|k| **k == MundaneFaunaKind::Rabbit)
        .count();
    assert_eq!(
        frog_count, 6,
        "8 个 seed 里应有 6 个命中 Frog（3:1 权重 × 2 轮）"
    );
    assert_eq!(rabbit_count, 2);
}

#[test]
fn species_for_position_seasonal_respects_biome_pool() {
    for season in [
        Season::Summer,
        Season::Winter,
        Season::SummerToWinter,
        Season::WinterToSummer,
    ] {
        let kind = mundane_species_for_position_seasonal(
            "north_wastes",
            DVec3::new(3.0, 64.0, 3.0),
            season,
        );
        assert!(
            WASTES_POOL.contains(&kind),
            "north_wastes 池派生出的物种 {kind:?}（season={season:?}）必须属于 {WASTES_POOL:?}"
        );
    }
}
