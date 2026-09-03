use bong_server::world::dimension::DimensionKind;
use bong_server::world::environment::EnvironmentEffect;
use bong_server::world::environment_overlay::{EnvironmentOverlays, DEFAULT_FOG_BANK_TINT};
use bong_server::world::zone::Zone;
use valence::prelude::DVec3;

fn zone_at(name: &str, dimension: DimensionKind, min: [f64; 3], max: [f64; 3]) -> Zone {
    Zone {
        name: name.to_string(),
        dimension,
        bounds: (
            DVec3::new(min[0], min[1], min[2]),
            DVec3::new(max[0], max[1], max[2]),
        ),
        spirit_qi: 0.3,
        danger_level: 1,
        active_events: Vec::new(),
        patrol_anchors: Vec::new(),
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    }
}

fn overworld_zone() -> Zone {
    zone_at(
        "spawn",
        DimensionKind::Overworld,
        [0.0, 0.0, 0.0],
        [100.0, 128.0, 100.0],
    )
}

fn spawn_default(overlays: &mut EnvironmentOverlays, duration: Option<u64>) -> String {
    overlays.spawn_fog_bank(
        DimensionKind::Overworld.ident_str(),
        [10.0, 40.0, 10.0],
        [50.0, 100.0, 50.0],
        DEFAULT_FOG_BANK_TINT,
        0.95,
        duration,
    )
}

#[test]
fn spawn_assigns_unique_incrementing_ids() {
    let mut overlays = EnvironmentOverlays::default();
    let first = spawn_default(&mut overlays, None);
    let second = spawn_default(&mut overlays, None);
    assert_eq!(first, "fog_1", "首个雾堤 id 应从 fog_1 起，实际 {first}");
    assert_eq!(
        second, "fog_2",
        "第二个雾堤 id 应递增为 fog_2，实际 {second}"
    );
    assert_eq!(overlays.fog_banks().len(), 2);
}

#[test]
fn spawn_normalizes_reversed_aabb_and_clamps_density() {
    let mut overlays = EnvironmentOverlays::default();
    overlays.spawn_fog_bank(
        DimensionKind::Overworld.ident_str(),
        [50.0, 100.0, 50.0],
        [10.0, 40.0, 10.0],
        DEFAULT_FOG_BANK_TINT,
        7.5,
        None,
    );
    let bank = &overlays.fog_banks()[0];
    assert_eq!(
        bank.aabb_min,
        [10.0, 40.0, 10.0],
        "min/max 反转输入应逐轴归一化"
    );
    assert_eq!(bank.aabb_max, [50.0, 100.0, 50.0]);
    assert_eq!(bank.density, 1.0, "density 7.5 应钳到 1.0");
}

#[test]
fn spawn_zeroes_non_finite_density() {
    let mut overlays = EnvironmentOverlays::default();
    overlays.spawn_fog_bank(
        DimensionKind::Overworld.ident_str(),
        [0.0; 3],
        [1.0; 3],
        DEFAULT_FOG_BANK_TINT,
        f32::NAN,
        None,
    );
    assert_eq!(
        overlays.fog_banks()[0].density,
        0.0,
        "非有限 density 应防御性归 0 而不是把 NaN 发上 wire"
    );
}

#[test]
fn fog_effects_map_to_fog_veil_with_matching_fields() {
    let mut overlays = EnvironmentOverlays::default();
    spawn_default(&mut overlays, None);
    let effects = overlays.fog_effects_for_zone(&overworld_zone());
    assert_eq!(effects.len(), 1);
    match &effects[0] {
        EnvironmentEffect::FogVeil {
            aabb_min,
            aabb_max,
            tint_rgb,
            density,
        } => {
            assert_eq!(*aabb_min, [10.0, 40.0, 10.0]);
            assert_eq!(*aabb_max, [50.0, 100.0, 50.0]);
            assert_eq!(*tint_rgb, DEFAULT_FOG_BANK_TINT);
            assert_eq!(*density, 0.95);
        }
        other => panic!("期望 FogVeil（雾堤唯一映射目标），实际 {other:?}"),
    }
}

#[test]
fn fog_effects_exclude_other_dimension() {
    let mut overlays = EnvironmentOverlays::default();
    spawn_default(&mut overlays, None);
    let tsy_zone = zone_at(
        "tsy_rim",
        DimensionKind::Tsy,
        [0.0, 0.0, 0.0],
        [100.0, 128.0, 100.0],
    );
    assert!(
        overlays.fog_effects_for_zone(&tsy_zone).is_empty(),
        "overworld 雾堤不得附着到 tsy dimension 的 zone"
    );
}

#[test]
fn fog_effects_exclude_disjoint_zone_but_include_edge_touch() {
    let mut overlays = EnvironmentOverlays::default();
    spawn_default(&mut overlays, None); // AABB [10..50]
    let disjoint = zone_at(
        "far",
        DimensionKind::Overworld,
        [51.0, 0.0, 51.0],
        [90.0, 128.0, 90.0],
    );
    assert!(
        overlays.fog_effects_for_zone(&disjoint).is_empty(),
        "不相交 zone 不应拿到雾堤"
    );
    let edge = zone_at(
        "edge",
        DimensionKind::Overworld,
        [50.0, 40.0, 50.0],
        [90.0, 128.0, 90.0],
    );
    assert_eq!(
        overlays.fog_effects_for_zone(&edge).len(),
        1,
        "闭区间语义：恰好贴边（50.0 == max）算相交"
    );
}

#[test]
fn tick_expiry_counts_down_and_removes() {
    let mut overlays = EnvironmentOverlays::default();
    let id = spawn_default(&mut overlays, Some(2));
    assert!(overlays.tick_expiry().is_empty(), "第 1 tick 后应仍存活");
    assert_eq!(overlays.fog_banks().len(), 1);
    let expired = overlays.tick_expiry();
    assert_eq!(expired, vec![id], "第 2 tick 应到期并回报 id");
    assert!(overlays.fog_banks().is_empty());
}

#[test]
fn tick_expiry_keeps_permanent_banks() {
    let mut overlays = EnvironmentOverlays::default();
    spawn_default(&mut overlays, None);
    for _ in 0..100 {
        assert!(overlays.tick_expiry().is_empty());
    }
    assert_eq!(
        overlays.fog_banks().len(),
        1,
        "无寿命雾堤应常驻直到显式清除"
    );
}

#[test]
fn remove_and_clear_all() {
    let mut overlays = EnvironmentOverlays::default();
    let first = spawn_default(&mut overlays, None);
    spawn_default(&mut overlays, None);
    assert!(overlays.remove_fog_bank(&first));
    assert!(
        !overlays.remove_fog_bank(&first),
        "重复移除同 id 应返回 false"
    );
    assert!(!overlays.remove_fog_bank("no_such_id"));
    assert_eq!(overlays.clear_fog_banks(), 1);
    assert!(overlays.fog_banks().is_empty());
    assert_eq!(overlays.clear_fog_banks(), 0, "空表 clear 应返回 0");
}
