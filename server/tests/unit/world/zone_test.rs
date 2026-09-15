#![allow(dead_code, unused_imports)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use bong_server::persistence::{
    ZoneOverlayRecord, ZoneRuntimeRecord, ZONE_OVERLAY_PAYLOAD_VERSION,
};
use bong_server::world::dimension::DimensionKind;
use bong_server::world::zone::{
    BotanyZoneTag, TsyDepth, Zone, ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME,
};
use valence::prelude::DVec3;

fn unique_temp_path(prefix: &str, suffix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!("{prefix}-{nanos}{suffix}"))
}

fn make_zone(name: &str, dim: bong_server::world::dimension::DimensionKind) -> Zone {
    Zone {
        name: name.to_string(),
        dimension: dim,
        bounds: (DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 10.0)),
        spirit_qi: 0.0,
        danger_level: 0,
        active_events: Vec::new(),
        patrol_anchors: Vec::new(),
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    }
}

fn make_zone_at(name: &str, x_min: f64, z_min: f64, x_max: f64, z_max: f64) -> Zone {
    Zone {
        name: name.to_string(),
        dimension: bong_server::world::dimension::DimensionKind::Overworld,
        bounds: (
            DVec3::new(x_min, 60.0, z_min),
            DVec3::new(x_max, 80.0, z_max),
        ),
        spirit_qi: 0.5,
        danger_level: 0,
        active_events: Vec::new(),
        patrol_anchors: Vec::new(),
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    }
}

#[test]
fn loads_zones_json_with_fallback() {
    let valid_path = unique_temp_path("bong-zones-valid", ".json");
    fs::write(
        &valid_path,
        r#"{
  "zones": [
{
  "name": "spawn",
  "aabb": {
    "min": [0.0, 64.0, 0.0],
    "max": [32.0, 80.0, 32.0]
  },
  "spirit_qi": 0.9,
  "danger_level": 0,
  "active_events": [],
  "patrol_anchors": [
    [14.0, 66.0, 14.0],
    [18.0, 66.0, 18.0]
  ],
  "blocked_tiles": [
    [15, 14],
    [16, 14]
  ]
},
{
  "name": "blood_valley",
  "aabb": {
    "min": [100.0, 64.0, 100.0],
    "max": [120.0, 80.0, 120.0]
  },
  "spirit_qi": -0.35,
  "danger_level": 4,
  "active_events": ["beast_tide"],
  "patrol_anchors": [
    [104.0, 66.0, 104.0]
  ],
  "blocked_tiles": [
    [106, 104]
  ]
}
  ]
}"#,
    )
    .expect("valid zones.json fixture should be writable");

    let registry = ZoneRegistry::load_from_path(&valid_path);
    let spawn = registry
        .find_zone(
            bong_server::world::dimension::DimensionKind::Overworld,
            DVec3::new(14.0, 66.0, 14.0),
        )
        .expect("valid config should load spawn zone");
    let blood_valley = registry
        .find_zone(
            bong_server::world::dimension::DimensionKind::Overworld,
            DVec3::new(110.0, 66.0, 110.0),
        )
        .expect("valid config should load blood_valley zone");

    assert_eq!(registry.zones.len(), 2);
    assert_eq!(spawn.name, DEFAULT_SPAWN_ZONE_NAME);
    assert_eq!(spawn.patrol_anchors.len(), 2);
    assert_eq!(spawn.patrol_anchors[0], DVec3::new(14.0, 66.0, 14.0));
    assert_eq!(spawn.blocked_tiles, vec![(15, 14), (16, 14)]);
    assert_eq!(blood_valley.name, "blood_valley");
    assert_eq!(blood_valley.spirit_qi, -0.35);
    assert_eq!(blood_valley.danger_level, 4);
    assert_eq!(blood_valley.active_events, vec!["beast_tide".to_string()]);
    assert_eq!(
        blood_valley.patrol_anchors,
        vec![DVec3::new(104.0, 66.0, 104.0)]
    );
    assert_eq!(blood_valley.blocked_tiles, vec![(106, 104)]);

    let fallback_path = unique_temp_path("bong-zones-missing", ".json");
    let fallback_registry = ZoneRegistry::load_from_path(&fallback_path);
    assert_eq!(fallback_registry.zones.len(), 1);
    assert_eq!(fallback_registry.zones[0].name, DEFAULT_SPAWN_ZONE_NAME);
}

#[test]
fn zones_json_without_dimension_field_defaults_to_overworld() {
    // Backwards-compat: pre-TSY zones.json snapshots have no `dimension` key.
    // `#[serde(default)]` on `ZoneConfig::dimension` must yield Overworld.
    use bong_server::world::dimension::DimensionKind;
    let path = unique_temp_path("bong-zones-default-dim", ".json");
    fs::write(
        &path,
        r#"{
  "zones": [
{
  "name": "spawn",
  "aabb": { "min": [0.0, 64.0, 0.0], "max": [32.0, 80.0, 32.0] },
  "spirit_qi": 0.9,
  "danger_level": 0
}
  ]
}"#,
    )
    .expect("fixture should be writable");
    let registry = ZoneRegistry::load_from_path(&path);
    assert_eq!(registry.zones.len(), 1);
    assert_eq!(registry.zones[0].dimension, DimensionKind::Overworld);
}

#[test]
fn zones_json_with_explicit_tsy_dimension_loads_correctly() {
    use bong_server::world::dimension::DimensionKind;
    let path = unique_temp_path("bong-zones-explicit-tsy", ".json");
    fs::write(
        &path,
        r#"{
  "zones": [
{
  "name": "spawn",
  "dimension": "overworld",
  "aabb": { "min": [0.0, 64.0, 0.0], "max": [32.0, 80.0, 32.0] },
  "spirit_qi": 0.9,
  "danger_level": 0
},
{
  "name": "tsy_test",
  "dimension": "tsy",
  "aabb": { "min": [-100.0, 0.0, -100.0], "max": [100.0, 128.0, 100.0] },
  "spirit_qi": -0.5,
  "danger_level": 5
}
  ]
}"#,
    )
    .expect("fixture should be writable");
    let registry = ZoneRegistry::load_from_path(&path);
    assert_eq!(registry.zones.len(), 2);
    let tsy_zone = registry
        .zones
        .iter()
        .find(|z| z.name == "tsy_test")
        .expect("tsy_test zone should be present");
    assert_eq!(tsy_zone.dimension, DimensionKind::Tsy);
}

#[test]
fn find_zone_filters_by_dimension() {
    use bong_server::world::dimension::DimensionKind;
    let path = unique_temp_path("bong-zones-find-by-dim", ".json");
    fs::write(
        &path,
        r#"{
  "zones": [
{
  "name": "spawn",
  "aabb": { "min": [0.0, 64.0, 0.0], "max": [32.0, 80.0, 32.0] },
  "spirit_qi": 0.9,
  "danger_level": 0
},
{
  "name": "tsy_overlap",
  "dimension": "tsy",
  "aabb": { "min": [0.0, 64.0, 0.0], "max": [32.0, 80.0, 32.0] },
  "spirit_qi": -0.5,
  "danger_level": 5
}
  ]
}"#,
    )
    .expect("fixture should be writable");
    let registry = ZoneRegistry::load_from_path(&path);
    // Same XYZ, but `find_zone` must return only the matching dimension.
    let pos = DVec3::new(8.0, 66.0, 8.0);
    let overworld_match = registry
        .find_zone(DimensionKind::Overworld, pos)
        .expect("overworld query should find spawn");
    let tsy_match = registry
        .find_zone(DimensionKind::Tsy, pos)
        .expect("tsy query should find tsy_overlap");
    assert_eq!(overworld_match.name, "spawn");
    assert_eq!(tsy_match.name, "tsy_overlap");
}

#[test]
fn accepts_zone_spirit_qi_at_full_negative_bound() {
    let valid_path = unique_temp_path("bong-zones-negative-bound", ".json");
    fs::write(
        &valid_path,
        r#"{
  "zones": [
{
  "name": "spawn",
  "aabb": {
    "min": [0.0, 64.0, 0.0],
    "max": [32.0, 80.0, 32.0]
  },
  "spirit_qi": -1.0,
  "danger_level": 0,
  "active_events": [],
  "patrol_anchors": [],
  "blocked_tiles": []
}
  ]
}"#,
    )
    .expect("negative bound zones.json fixture should be writable");

    let registry = ZoneRegistry::load_from_path(&valid_path);
    assert_eq!(registry.zones.len(), 1);
    assert_eq!(registry.zones[0].spirit_qi, -1.0);
}

#[test]
fn accepts_zone_spirit_qi_at_positive_bound() {
    let valid_path = unique_temp_path("bong-zones-positive-bound", ".json");
    fs::write(
        &valid_path,
        r#"{
  "zones": [
{
  "name": "spawn",
  "aabb": {
    "min": [0.0, 64.0, 0.0],
    "max": [32.0, 80.0, 32.0]
  },
  "spirit_qi": 1.0,
  "danger_level": 0,
  "active_events": [],
  "patrol_anchors": [],
  "blocked_tiles": []
}
  ]
}"#,
    )
    .expect("positive bound zones.json fixture should be writable");

    let registry = ZoneRegistry::load_from_path(&valid_path);
    assert_eq!(registry.zones.len(), 1);
    assert_eq!(registry.zones[0].spirit_qi, 1.0);
}

#[test]
fn qi_equilibrium_and_inflow_default_to_zero_when_omitted_from_zones_json() {
    // 向后兼容红线：pre-P1 快照（没有这两个字段）必须解析成 0.0，不回流，不报错。
    let path = unique_temp_path("bong-zones-qi-equilibrium-omitted", ".json");
    fs::write(
        &path,
        r#"{
  "zones": [
{
  "name": "spawn",
  "aabb": { "min": [0.0, 64.0, 0.0], "max": [32.0, 80.0, 32.0] },
  "spirit_qi": 0.9,
  "danger_level": 0
}
  ]
}"#,
    )
    .expect("fixture should be writable");
    let registry = ZoneRegistry::load_from_path(&path);
    assert_eq!(registry.zones.len(), 1);
    assert_eq!(
        registry.zones[0].qi_equilibrium, 0.0,
        "omitted qi_equilibrium must default to 0.0 (opt-out), not fail to parse"
    );
    assert_eq!(
        registry.zones[0].qi_inflow_per_min, 0.0,
        "omitted qi_inflow_per_min must default to 0.0 (opt-out), not fail to parse"
    );
}

#[test]
fn qi_equilibrium_and_inflow_round_trip_when_present() {
    let path = unique_temp_path("bong-zones-qi-equilibrium-present", ".json");
    fs::write(
        &path,
        r#"{
  "zones": [
{
  "name": "spawn",
  "aabb": { "min": [0.0, 64.0, 0.0], "max": [32.0, 80.0, 32.0] },
  "spirit_qi": 0.2,
  "danger_level": 0,
  "qi_equilibrium": 0.35,
  "qi_inflow_per_min": 0.4
}
  ]
}"#,
    )
    .expect("fixture should be writable");
    let registry = ZoneRegistry::load_from_path(&path);
    assert_eq!(registry.zones.len(), 1);
    assert_eq!(registry.zones[0].qi_equilibrium, 0.35);
    assert_eq!(registry.zones[0].qi_inflow_per_min, 0.4);
}

#[test]
fn accepts_qi_equilibrium_at_bounds() {
    for bound in [0.0_f64, 1.0_f64] {
        let path = unique_temp_path("bong-zones-qi-equilibrium-bound", ".json");
        fs::write(
            &path,
            format!(
                r#"{{
  "zones": [
{{
  "name": "spawn",
  "aabb": {{ "min": [0.0, 64.0, 0.0], "max": [32.0, 80.0, 32.0] }},
  "spirit_qi": 0.2,
  "danger_level": 0,
  "qi_equilibrium": {bound},
  "qi_inflow_per_min": 0.0
}}
  ]
}}"#
            ),
        )
        .expect("fixture should be writable");
        let registry = ZoneRegistry::load_from_path(&path);
        assert_eq!(
            registry.zones[0].qi_equilibrium, bound,
            "qi_equilibrium boundary {bound} must be accepted, not rejected into fallback"
        );
    }
}

#[test]
fn rejects_qi_equilibrium_below_zero() {
    let path = unique_temp_path("bong-zones-qi-equilibrium-negative", ".json");
    fs::write(
        &path,
        r#"{
  "zones": [
{
  "name": "spawn",
  "aabb": { "min": [0.0, 64.0, 0.0], "max": [32.0, 80.0, 32.0] },
  "spirit_qi": 0.2,
  "danger_level": 0,
  "qi_equilibrium": -0.01,
  "qi_inflow_per_min": 0.4
}
  ]
}"#,
    )
    .expect("fixture should be writable");
    let registry = ZoneRegistry::load_from_path(&path);
    // 校验失败 -> 整份 zones.json 被拒绝 -> 落回硬编码 fallback（既有 spirit_qi 校验失败
    // 测试同款断言范式）。
    assert_eq!(registry.zones.len(), 1);
    assert_eq!(registry.zones[0].name, DEFAULT_SPAWN_ZONE_NAME);
    assert_eq!(registry.zones[0].qi_equilibrium, 0.0);
}

#[test]
fn rejects_qi_equilibrium_above_one() {
    let path = unique_temp_path("bong-zones-qi-equilibrium-above-one", ".json");
    fs::write(
        &path,
        r#"{
  "zones": [
{
  "name": "spawn",
  "aabb": { "min": [0.0, 64.0, 0.0], "max": [32.0, 80.0, 32.0] },
  "spirit_qi": 0.2,
  "danger_level": 0,
  "qi_equilibrium": 1.01,
  "qi_inflow_per_min": 0.4
}
  ]
}"#,
    )
    .expect("fixture should be writable");
    let registry = ZoneRegistry::load_from_path(&path);
    assert_eq!(registry.zones.len(), 1);
    assert_eq!(registry.zones[0].name, DEFAULT_SPAWN_ZONE_NAME);
}

#[test]
fn rejects_qi_equilibrium_non_finite() {
    for bad in ["NaN", "Infinity", "-Infinity"] {
        let path = unique_temp_path("bong-zones-qi-equilibrium-non-finite", ".json");
        fs::write(
            &path,
            format!(
                r#"{{
  "zones": [
{{
  "name": "spawn",
  "aabb": {{ "min": [0.0, 64.0, 0.0], "max": [32.0, 80.0, 32.0] }},
  "spirit_qi": 0.2,
  "danger_level": 0,
  "qi_equilibrium": {bad},
  "qi_inflow_per_min": 0.4
}}
  ]
}}"#
            ),
        )
        .expect("fixture should be writable");
        // serde_json rejects non-finite float literals like `NaN`/`Infinity` as invalid
        // JSON syntax before validate_zone even runs — confirm the whole file still falls
        // back safely rather than panicking on a malformed zones.json.
        let registry = ZoneRegistry::load_from_path(&path);
        assert_eq!(registry.zones.len(), 1);
        assert_eq!(registry.zones[0].name, DEFAULT_SPAWN_ZONE_NAME);
    }
}

#[test]
fn rejects_qi_inflow_per_min_negative() {
    let path = unique_temp_path("bong-zones-qi-inflow-negative", ".json");
    fs::write(
        &path,
        r#"{
  "zones": [
{
  "name": "spawn",
  "aabb": { "min": [0.0, 64.0, 0.0], "max": [32.0, 80.0, 32.0] },
  "spirit_qi": 0.2,
  "danger_level": 0,
  "qi_equilibrium": 0.35,
  "qi_inflow_per_min": -0.1
}
  ]
}"#,
    )
    .expect("fixture should be writable");
    let registry = ZoneRegistry::load_from_path(&path);
    assert_eq!(registry.zones.len(), 1);
    assert_eq!(registry.zones[0].name, DEFAULT_SPAWN_ZONE_NAME);
}

#[test]
fn accepts_qi_inflow_per_min_zero_and_large_positive() {
    for value in [0.0_f64, 1000.0_f64] {
        let path = unique_temp_path("bong-zones-qi-inflow-positive", ".json");
        fs::write(
            &path,
            format!(
                r#"{{
  "zones": [
{{
  "name": "spawn",
  "aabb": {{ "min": [0.0, 64.0, 0.0], "max": [32.0, 80.0, 32.0] }},
  "spirit_qi": 0.2,
  "danger_level": 0,
  "qi_equilibrium": 0.35,
  "qi_inflow_per_min": {value}
}}
  ]
}}"#
            ),
        )
        .expect("fixture should be writable");
        let registry = ZoneRegistry::load_from_path(&path);
        assert_eq!(
            registry.zones[0].qi_inflow_per_min, value,
            "qi_inflow_per_min has no configured upper bound — {value} must round-trip"
        );
    }
}

#[test]
fn spawn_zones_json_fixture_configures_equilibrium_above_meridian_open_threshold() {
    // plan-zone-qi-economy-v1 §8.1 #2 — spawn 的 qi_equilibrium 必须 > MIN_ZONE_QI_TO_OPEN
    // (0.3)，否则开脉门槛永远打不开。用真实的 server/zones.json（非临时 fixture）核验。
    let registry = ZoneRegistry::load();
    let spawn = registry
        .zones
        .iter()
        .find(|zone| zone.name == DEFAULT_SPAWN_ZONE_NAME)
        .expect("real zones.json must contain a spawn zone");
    assert!(
        spawn.qi_equilibrium > bong_server::cultivation::meridian_open::MIN_ZONE_QI_TO_OPEN,
        "spawn qi_equilibrium ({}) must clear MIN_ZONE_QI_TO_OPEN ({}) or meridian-opening \
         is permanently unreachable at spawn even after P1 inflow settles",
        spawn.qi_equilibrium,
        bong_server::cultivation::meridian_open::MIN_ZONE_QI_TO_OPEN,
    );
    assert!(
        spawn.qi_inflow_per_min > 0.0,
        "spawn must actually configure a positive inflow rate — otherwise qi_equilibrium \
         is a dead config value"
    );
}

#[test]
fn default_load_merges_tsy_blueprint_zones() {
    let registry = ZoneRegistry::load();

    let spawn = registry
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("default load must keep the overworld spawn zone");
    assert_eq!(
        spawn.dimension,
        bong_server::world::dimension::DimensionKind::Overworld,
        "spawn zone must remain in overworld after TSY supplement merge"
    );

    let daneng = registry
        .find_zone_by_name("tsy_daneng_01_shallow")
        .expect("default load must merge TSY blueprint zones from zones.tsy.json");
    assert_eq!(
        daneng.dimension,
        bong_server::world::dimension::DimensionKind::Tsy,
        "TSY blueprint zone must retain dimension=tsy"
    );
    assert!(
        daneng.is_tsy(),
        "tsy_daneng_01_shallow must be recognised by the TSY prefix gate"
    );
    assert!(
        daneng.spirit_qi < -0.4,
        "tsy_daneng_01_shallow must stay below the dying elder spawn threshold; got {}",
        daneng.spirit_qi
    );

    assert!(
        registry
            .zones
            .iter()
            .any(|zone| zone.name == "tsy_lingxu_01_deep" && zone.spirit_qi < -1.0),
        "TSY blueprint merge must preserve deep-layer collapse pressure values used by tsy_drain"
    );
}

#[test]
fn apply_runtime_records_overrides_only_known_zones() {
    let mut registry = ZoneRegistry::fallback();
    registry.apply_runtime_records(&[
        ZoneRuntimeRecord {
            zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            spirit_qi: -0.2,
            danger_level: 3,
        },
        ZoneRuntimeRecord {
            zone_id: "missing".to_string(),
            spirit_qi: 0.8,
            danger_level: 5,
        },
    ]);

    assert_eq!(registry.zones.len(), 1);
    assert_eq!(registry.zones[0].name, DEFAULT_SPAWN_ZONE_NAME);
    assert_eq!(registry.zones[0].spirit_qi, -0.2);
    assert_eq!(registry.zones[0].danger_level, 3);
}

#[test]
fn apply_overlay_records_merges_supported_overlay_payloads() {
    let mut registry = ZoneRegistry::fallback();
    registry.zones[0].spirit_qi = 0.8;
    registry
        .apply_overlay_records(&[
            ZoneOverlayRecord {
                zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                overlay_kind: "collapsed".to_string(),
                payload_json: serde_json::json!({
                    "danger_level": 4,
                    "zone_status": "collapsed",
                    "blocked_tiles": [[1, 2], [3, 4]],
                })
                .to_string(),
                payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
                since_wall: 100,
            },
            ZoneOverlayRecord {
                zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                overlay_kind: "qi_eye_formed".to_string(),
                payload_json: serde_json::json!({
                    "active_events": ["qi_eye_formed"],
                })
                .to_string(),
                payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
                since_wall: 101,
            },
            ZoneOverlayRecord {
                zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                overlay_kind: "ruins_discovered".to_string(),
                payload_json: serde_json::json!({
                    "active_events": ["ruins_discovered"],
                    "blocked_tiles": [[5, 6]],
                })
                .to_string(),
                payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
                since_wall: 102,
            },
        ])
        .expect("overlay application should succeed");

    assert_eq!(registry.zones[0].spirit_qi, 0.0);
    assert_eq!(registry.zones[0].danger_level, 4);
    assert_eq!(
        registry.zones[0].active_events,
        vec![
            "realm_collapse".to_string(),
            "qi_eye_formed".to_string(),
            "ruins_discovered".to_string(),
        ]
    );
    assert_eq!(
        registry.zones[0].blocked_tiles,
        vec![(1, 2), (3, 4), (5, 6)]
    );
}

#[test]
fn botany_tags_are_derived_from_zone_name_without_biome_field() {
    let registry =
        ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));

    let spawn = registry
        .find_zone_by_name("spawn")
        .expect("spawn zone should exist");
    assert!(spawn.supports_botany_tag(BotanyZoneTag::Plains));

    let marsh = registry
        .find_zone_by_name("lingquan_marsh")
        .expect("lingquan_marsh should exist");
    assert!(marsh.supports_botany_tag(BotanyZoneTag::Marsh));

    let blood = registry
        .find_zone_by_name("blood_valley")
        .expect("blood_valley should exist");
    assert!(blood.supports_botany_tag(BotanyZoneTag::BloodValley));
}

#[test]
fn wangyintai_zone_loads_from_zones_json() {
    let registry =
        ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));
    let zone = registry
        .find_zone_by_name("wangyintai")
        .expect("wangyintai zone should exist in zones.json");

    assert_eq!(zone.name, "wangyintai", "zone name must be wangyintai");
    assert_eq!(
        zone.dimension,
        bong_server::world::dimension::DimensionKind::Overworld,
        "wangyintai is an overworld zone"
    );
}

#[test]
fn wangyintai_spirit_qi_is_negative() {
    let registry =
        ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));
    let zone = registry
        .find_zone_by_name("wangyintai")
        .expect("wangyintai zone should exist");

    assert!(
        zone.spirit_qi < 0.0,
        "wangyintai spirit_qi must stay negative, got {}",
        zone.spirit_qi
    );
    assert!(
        (zone.spirit_qi - (-0.15)).abs() <= 0.01,
        "wangyintai spirit_qi should stay near -0.15 negative qi edge, got {}",
        zone.spirit_qi
    );
}

#[test]
fn wangyintai_danger_level_is_3() {
    let registry =
        ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));
    let zone = registry
        .find_zone_by_name("wangyintai")
        .expect("wangyintai zone should exist");

    assert_eq!(
        zone.danger_level, 3,
        "wangyintai danger_level should be 3 (medium risk), got {}",
        zone.danger_level
    );
}

#[test]
fn wangyintai_bounds_match_plan_spec() {
    let registry =
        ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));
    let zone = registry
        .find_zone_by_name("wangyintai")
        .expect("wangyintai zone should exist");

    // plan-zone-registry-coverage-v1 §P2 — moved off the blood_valley AABB overlap.
    let (min, max) = zone.bounds;
    assert!(
        (min.x - 3500.0).abs() < f64::EPSILON
            && (min.y - 40.0).abs() < f64::EPSILON
            && (min.z - (-2150.0)).abs() < f64::EPSILON,
        "wangyintai min bounds should be (3500, 40, -2150), got ({}, {}, {})",
        min.x,
        min.y,
        min.z
    );
    assert!(
        (max.x - 4500.0).abs() < f64::EPSILON
            && (max.y - 200.0).abs() < f64::EPSILON
            && (max.z - (-1150.0)).abs() < f64::EPSILON,
        "wangyintai max bounds should be (4500, 200, -1150), got ({}, {}, {})",
        max.x,
        max.y,
        max.z
    );
}

#[test]
fn wangyintai_patrol_anchors_within_bounds() {
    let registry =
        ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));
    let zone = registry
        .find_zone_by_name("wangyintai")
        .expect("wangyintai zone should exist");

    assert_eq!(
        zone.patrol_anchors.len(),
        2,
        "wangyintai should have 2 patrol anchors per plan spec, got {}",
        zone.patrol_anchors.len()
    );
    for (i, anchor) in zone.patrol_anchors.iter().enumerate() {
        assert!(
            zone.contains(*anchor),
            "wangyintai patrol_anchor[{}] at ({}, {}, {}) must be within zone bounds",
            i,
            anchor.x,
            anchor.y,
            anchor.z
        );
    }
}

#[test]
fn wangyintai_center_resolves_to_zone() {
    let registry =
        ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));

    // Center of the zone AABB (moved off blood_valley overlap, plan-zone-registry-coverage-v1).
    let center = DVec3::new(4000.0, 120.0, -1650.0);
    let found = registry
        .find_zone(
            bong_server::world::dimension::DimensionKind::Overworld,
            center,
        )
        .expect("center point should resolve to wangyintai zone");
    assert_eq!(found.name, "wangyintai");
}

#[test]
fn wangyintai_is_not_tsy() {
    let registry =
        ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));
    let zone = registry
        .find_zone_by_name("wangyintai")
        .expect("wangyintai zone should exist");
    assert!(!zone.is_tsy(), "wangyintai is an overworld zone, not TSY");
}

#[test]
fn zones_json_covers_all_overworld_blueprint_zones() {
    let registry =
        ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));
    let expected = [
        "spawn",
        "qingyun_peaks",
        "lingquan_marsh",
        "blood_valley",
        "youan_depths",
        "north_wastes",
        "giant_sword_sea",
        "dan_zong_yi_yuan",
        "wangyintai",
        "baolongwang_cavern_deep",
        "celestial_isles",
        "south_ash_dead_zone",
        "zhanhun_plain",
        "wuxing_abyss",
        "blood_valley_east_scorch",
        "north_waste_east_scorch",
        "drift_scorch_001",
        "rift_mouth_north_001",
        "rift_mouth_north_002",
        "rift_mouth_blood_001",
        "rift_mouth_west_001",
        "jiuzong_bloodstream_ruin",
        "jiuzong_beiling_ruin",
        "jiuzong_nanyuan_ruin",
        "jiuzong_chixia_ruin",
        "jiuzong_xuanshui_ruin",
        "jiuzong_taichu_ruin",
        "jiuzong_youan_ruin",
    ];
    for name in expected {
        assert!(
            registry.find_zone_by_name(name).is_some(),
            "zones.json must contain overworld zone `{name}` (regen from blueprint if missing)"
        );
    }
    assert_eq!(
        registry.zones.len(),
        expected.len(),
        "zones.json overworld zone count should match the pinned blueprint set"
    );
}

#[test]
fn spawn_zone_baked_qi_clears_breakthrough_threshold_with_headroom() {
    use bong_server::cultivation::breakthrough::MIN_ZONE_QI_TO_BREAKTHROUGH;

    let registry =
        ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));
    let spawn = registry
        .find_zone_by_name("spawn")
        .expect("zones.json must contain the spawn zone");
    let headroom = 0.03;
    assert!(
        spawn.spirit_qi >= MIN_ZONE_QI_TO_BREAKTHROUGH + headroom,
        "spawn 烘焙 spirit_qi ({}) 必须 >= 突破门槛 ({MIN_ZONE_QI_TO_BREAKTHROUGH}) + \
         运行时余量 ({headroom})，否则 qi 经济把 live 值拉下门槛后新手在出生区永远无法\
         突破；蓝图 spawn spirit_qi=0.35 重跑 zones_export 应得 ~0.363",
        spawn.spirit_qi
    );
}

#[test]
fn find_zone_returns_smallest_containing_zone() {
    use bong_server::world::dimension::DimensionKind;
    use bong_server::world::zone::Zone;
    let big = Zone {
        name: "big".to_string(),
        dimension: DimensionKind::Overworld,
        bounds: (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 100.0, 100.0)),
        spirit_qi: 0.0,
        danger_level: 1,
        active_events: Vec::new(),
        patrol_anchors: Vec::new(),
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    };
    let small = Zone {
        name: "small".to_string(),
        dimension: DimensionKind::Overworld,
        bounds: (DVec3::new(40.0, 40.0, 40.0), DVec3::new(60.0, 60.0, 60.0)),
        spirit_qi: 0.0,
        danger_level: 1,
        active_events: Vec::new(),
        patrol_anchors: Vec::new(),
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    };
    // `big` is registered first; without smallest-AABB selection, find_zone
    // would return it for points inside the nested `small` zone.
    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![big, small],
    };
    assert_eq!(
        registry
            .find_zone(DimensionKind::Overworld, DVec3::new(50.0, 50.0, 50.0))
            .map(|z| z.name.as_str()),
        Some("small"),
        "nested overlap must resolve to the smaller (more specific) zone"
    );
    assert_eq!(
        registry
            .find_zone(DimensionKind::Overworld, DVec3::new(10.0, 10.0, 10.0))
            .map(|z| z.name.as_str()),
        Some("big"),
        "a point only inside the larger zone must resolve to it"
    );
}

#[test]
fn north_rift_and_scorch_are_adjacent_but_mutually_exclusive() {
    let registry = ZoneRegistry::load();
    assert!(
        registry
            .find_zone_by_name("tsy_zongmen_01_shallow")
            .is_some(),
        "production load path must merge TSY blueprint zones before this pin runs"
    );
    assert_eq!(
        registry
            .zones
            .iter()
            .filter(|zone| zone.name == "rift_mouth_north_002")
            .count(),
        1,
        "north rift must remain uniquely registered after production merge"
    );
    assert_eq!(
        registry
            .zones
            .iter()
            .filter(|zone| zone.name == "north_waste_east_scorch")
            .count(),
        1,
        "north scorch must remain uniquely registered after production merge"
    );
    let rift = registry
        .find_zone_by_name("rift_mouth_north_002")
        .expect("north rift zone must remain registered");
    let scorch = registry
        .find_zone_by_name("north_waste_east_scorch")
        .expect("north scorch zone must remain registered");
    assert_eq!(rift.dimension, DimensionKind::Overworld);
    assert_eq!(scorch.dimension, DimensionKind::Overworld);
    assert!(
        scorch
            .active_events
            .iter()
            .any(|event| event == "tribulation_scorch")
            && scorch
                .active_events
                .iter()
                .any(|event| event == "tianjie_ascension_pit"),
        "scorch weather/tribulation semantics must remain attached to the production zone"
    );
    let (rift_min, rift_max) = rift.bounds;
    let (scorch_min, scorch_max) = scorch.bounds;

    assert!(
        rift_max.x < scorch_min.x
            || scorch_max.x < rift_min.x
            || rift_max.y < scorch_min.y
            || scorch_max.y < rift_min.y
            || rift_max.z < scorch_min.z
            || scorch_max.z < rift_min.z,
        "north rift and scorch AABBs must remain strictly separated on at least one axis"
    );
    assert!(
        registry.zones_are_adjacent(&rift.name, &scorch.name, 100.0),
        "north rift must remain a neighbour of the scorch zone after removing overlap"
    );
    assert_eq!(
        registry
            .find_zone(DimensionKind::Overworld, DVec3::new(2000.0, 74.0, -7300.0))
            .map(|zone| zone.name.as_str()),
        Some("rift_mouth_north_002"),
        "relocated portal anchor must resolve to the rift zone"
    );
    assert_eq!(
        registry
            .find_zone(DimensionKind::Overworld, DVec3::new(2000.0, 74.0, -7800.0))
            .map(|zone| zone.name.as_str()),
        Some("north_waste_east_scorch"),
        "the former rift anchor must now resolve exclusively to scorch semantics"
    );
    assert_eq!(
        registry
            .find_zone(DimensionKind::Overworld, DVec3::new(2100.0, 80.0, -8000.0))
            .map(|zone| zone.name.as_str()),
        Some("north_waste_east_scorch"),
        "ascension pit must retain scorch weather and tribulation semantics"
    );
    assert!(
        !rift.contains(DVec3::new(2150.0, 74.0, -7500.0))
            && scorch.contains(DVec3::new(2150.0, 74.0, -7500.0)),
        "scorch north boundary must not be shadowed by the relocated rift"
    );
    assert!(
        rift.contains(DVec3::new(1850.0, 50.0, -7450.0))
            && rift.contains(DVec3::new(2150.0, 100.0, -7150.0)),
        "rift AABB inclusive min/max boundaries must remain reachable"
    );
}

#[test]
fn is_tsy_recognises_prefix() {
    assert!(make_zone(
        "tsy_lingxu_01_shallow",
        bong_server::world::dimension::DimensionKind::Tsy
    )
    .is_tsy());
    assert!(!make_zone(
        "blood_valley",
        bong_server::world::dimension::DimensionKind::Overworld
    )
    .is_tsy());
    assert!(!make_zone("", bong_server::world::dimension::DimensionKind::Overworld).is_tsy());
}

#[test]
fn tsy_depth_parses_layer_suffix() {
    use bong_server::world::zone::TsyDepth;
    let dim = bong_server::world::dimension::DimensionKind::Tsy;
    assert_eq!(
        make_zone("tsy_lingxu_01_shallow", dim).tsy_depth(),
        Some(TsyDepth::Shallow)
    );
    assert_eq!(
        make_zone("tsy_lingxu_01_mid", dim).tsy_depth(),
        Some(TsyDepth::Mid)
    );
    assert_eq!(
        make_zone("tsy_lingxu_01_deep", dim).tsy_depth(),
        Some(TsyDepth::Deep)
    );
    // Non-tsy zone returns None even if suffix matches.
    assert_eq!(
        make_zone(
            "foo_shallow",
            bong_server::world::dimension::DimensionKind::Overworld
        )
        .tsy_depth(),
        None
    );
    // Malformed depth suffix returns None.
    assert_eq!(make_zone("tsy_lingxu_01_abyss", dim).tsy_depth(), None);
}

#[test]
fn tsy_family_id_strips_depth_suffix() {
    let dim = bong_server::world::dimension::DimensionKind::Tsy;
    assert_eq!(
        make_zone("tsy_lingxu_01_shallow", dim).tsy_family_id(),
        Some("tsy_lingxu_01".to_string())
    );
    assert_eq!(
        make_zone("tsy_a_b_c_deep", dim).tsy_family_id(),
        Some("tsy_a_b_c".to_string())
    );
    // Malformed suffix → None (we refuse to chop arbitrary trailing tokens).
    assert_eq!(make_zone("tsy_lingxu_01_abyss", dim).tsy_family_id(), None);
}

#[test]
fn is_tsy_entry_checks_active_events() {
    let mut z = make_zone(
        "tsy_lingxu_01_shallow",
        bong_server::world::dimension::DimensionKind::Tsy,
    );
    assert!(!z.is_tsy_entry());
    z.active_events.push("tsy_entry".to_string());
    assert!(z.is_tsy_entry());
}

#[test]
fn register_runtime_zone_appends_unique_zone() {
    let mut registry = ZoneRegistry::fallback();
    let initial_len = registry.zones.len();
    let zone = make_zone(
        "tsy_lingxu_01_shallow",
        bong_server::world::dimension::DimensionKind::Tsy,
    );
    registry.register_runtime_zone(zone).expect("first add ok");
    assert_eq!(registry.zones.len(), initial_len + 1);
    assert!(registry
        .find_zone_by_name("tsy_lingxu_01_shallow")
        .is_some());
}

#[test]
fn zones_are_adjacent_touching_within_margin() {
    // zone A: x=[0,100], zone B: x=[200,300], margin=150 → gap=100 < margin → 相邻
    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            make_zone_at("zone_a", 0.0, 0.0, 100.0, 100.0),
            make_zone_at("zone_b", 200.0, 0.0, 300.0, 100.0),
        ],
    };
    assert!(
        registry.zones_are_adjacent("zone_a", "zone_b", 150.0),
        "gap=100 < margin=150 应相邻"
    );
}

#[test]
fn zones_are_adjacent_far_apart_not_adjacent() {
    // zone A: x=[0,100], zone B: x=[400,500], margin=100 → gap=300 > margin → 不相邻
    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            make_zone_at("zone_a", 0.0, 0.0, 100.0, 100.0),
            make_zone_at("zone_c", 400.0, 0.0, 500.0, 100.0),
        ],
    };
    assert!(
        !registry.zones_are_adjacent("zone_a", "zone_c", 100.0),
        "gap=300 > margin=100 不应相邻"
    );
}

#[test]
fn zones_are_adjacent_overlapping() {
    // 两个 zone AABB 直接重叠 → 相邻
    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            make_zone_at("zone_a", 0.0, 0.0, 200.0, 200.0),
            make_zone_at("zone_b", 100.0, 0.0, 300.0, 200.0),
        ],
    };
    assert!(
        registry.zones_are_adjacent("zone_a", "zone_b", 0.0),
        "AABB 重叠（margin=0）应相邻"
    );
}

#[test]
fn zones_are_adjacent_same_name_returns_false() {
    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![make_zone_at("spawn", 0.0, 0.0, 100.0, 100.0)],
    };
    assert!(
        !registry.zones_are_adjacent("spawn", "spawn", 999.0),
        "相同 zone 名称不应视为相邻（自相邻无意义）"
    );
}

#[test]
fn zones_are_adjacent_missing_zone_returns_false() {
    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![make_zone_at("zone_a", 0.0, 0.0, 100.0, 100.0)],
    };
    assert!(
        !registry.zones_are_adjacent("zone_a", "nonexistent", 9999.0),
        "找不到 zone 应返回 false"
    );
}

#[test]
fn adjacent_zone_names_returns_neighbors() {
    // A 与 B 相邻（gap=50 < margin=100），A 与 C 不相邻（gap=500 > margin=100）
    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            make_zone_at("zone_a", 0.0, 0.0, 100.0, 100.0),
            make_zone_at("zone_b", 150.0, 0.0, 250.0, 100.0), // gap=50
            make_zone_at("zone_c", 700.0, 0.0, 800.0, 100.0), // gap=600
        ],
    };
    let neighbors = registry.adjacent_zone_names("zone_a", 100.0);
    assert!(
        neighbors.contains(&"zone_b".to_string()),
        "zone_b 应在 zone_a 的相邻列表中"
    );
    assert!(
        !neighbors.contains(&"zone_c".to_string()),
        "zone_c 不应在 zone_a 的相邻列表中"
    );
}

#[test]
fn register_runtime_zone_rejects_duplicate_name() {
    let mut registry = ZoneRegistry::fallback();
    let zone = make_zone(
        "tsy_lingxu_01_shallow",
        bong_server::world::dimension::DimensionKind::Tsy,
    );
    registry
        .register_runtime_zone(zone.clone())
        .expect("first add ok");
    let err = registry
        .register_runtime_zone(zone)
        .expect_err("duplicate name should be rejected");
    assert!(err.contains("already registered"), "got: {err}");
}
