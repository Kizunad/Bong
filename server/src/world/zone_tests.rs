#![allow(dead_code, unused_imports)]

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
use valence::prelude::DVec3;

fn unique_temp_path(prefix: &str, suffix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!("{prefix}-{nanos}{suffix}"))
}

fn make_zone(name: &str, dim: crate::world::dimension::DimensionKind) -> super::Zone {
    super::Zone {
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

#[test]
fn rejects_zone_spirit_qi_below_negative_bound() {
    let invalid_path = unique_temp_path("bong-zones-below-negative-bound", ".json");
    fs::write(
        &invalid_path,
        r#"{
  "zones": [
{
  "name": "spawn",
  "aabb": {
    "min": [0.0, 64.0, 0.0],
    "max": [32.0, 80.0, 32.0]
  },
  "spirit_qi": -1.01,
  "danger_level": 0,
  "active_events": [],
  "patrol_anchors": [],
  "blocked_tiles": []
}
  ]
}"#,
    )
    .expect("invalid negative bound zones.json fixture should be writable");

    let registry = ZoneRegistry::load_from_path(&invalid_path);
    assert_eq!(registry.zones.len(), 1);
    assert_eq!(registry.zones[0].name, DEFAULT_SPAWN_ZONE_NAME);
    assert_eq!(registry.zones[0].spirit_qi, super::DEFAULT_SPAWN_SPIRIT_QI);
}

#[test]
fn tsy_blueprint_merge_rejects_conflicts_without_partial_state() {
    let path = unique_temp_path("bong-zones-tsy-conflict", ".json");
    fs::write(
        &path,
        r#"{
  "zones": [
{
  "name": "tsy_partial_01_shallow",
  "dimension": "tsy",
  "aabb": { "min": [0.0, 20.0, 0.0], "max": [16.0, 40.0, 16.0] },
  "spirit_qi": -0.5,
  "danger_level": 2
},
{
  "name": "spawn",
  "dimension": "tsy",
  "aabb": { "min": [32.0, 20.0, 32.0], "max": [48.0, 40.0, 48.0] },
  "spirit_qi": -0.5,
  "danger_level": 2
}
  ]
}"#,
    )
    .expect("fixture should be writable");

    let mut registry = ZoneRegistry::fallback();
    let result = registry.merge_tsy_blueprint_from_path(&path);

    assert!(
        result.is_err(),
        "TSY supplemental merge must reject names already present in the base registry"
    );
    assert_eq!(
        registry.zones.len(),
        1,
        "conflicting supplemental merge must be atomic and keep the base registry unchanged"
    );
    assert!(
        registry
            .find_zone_by_name("tsy_partial_01_shallow")
            .is_none(),
        "no earlier supplemental zone should be left behind after a later name conflict"
    );
}

#[test]
fn tsy_blueprint_merge_rejects_non_tsy_zones_without_partial_state() {
    let path = unique_temp_path("bong-zones-tsy-non-tsy", ".json");
    fs::write(
        &path,
        r#"{
  "zones": [
{
  "name": "tsy_valid_01_shallow",
  "dimension": "tsy",
  "aabb": { "min": [0.0, 20.0, 0.0], "max": [16.0, 40.0, 16.0] },
  "spirit_qi": -1.2,
  "danger_level": 2
},
{
  "name": "overworld_leak",
  "dimension": "overworld",
  "aabb": { "min": [32.0, 20.0, 32.0], "max": [48.0, 40.0, 48.0] },
  "spirit_qi": -0.5,
  "danger_level": 2
}
  ]
}"#,
    )
    .expect("fixture should be writable");

    let mut registry = ZoneRegistry::fallback();
    let result = registry.merge_tsy_blueprint_from_path(&path);

    assert!(
        result.is_err(),
        "TSY supplemental merge must reject non-TSY zones instead of widening the runtime registry"
    );
    assert_eq!(
        registry.zones.len(),
        1,
        "invalid supplemental merge must be atomic and keep the base registry unchanged"
    );
    assert!(
        registry.find_zone_by_name("tsy_valid_01_shallow").is_none(),
        "no earlier valid supplemental zone should be left behind after a later non-TSY zone"
    );
}

#[test]
fn spatial_revision_mutates_only_on_successful_membership_change() {
    // fix-spec-1901-v2 §7.1 — spatial_revision 契约：成功注册 +1、非空 TSY 并入 +1；
    // 空并入与一切被拒绝的变化都必须保持 revision 不变（否则 lingtian 的 pending
    // plot retry 门会误判，见 auto_set_plot_zone 的 last_seen_spatial_revision）。
    let mut registry = ZoneRegistry::fallback();
    assert_eq!(
        registry.spatial_revision, 0,
        "fallback registry starts at revision 0"
    );

    // 成功 register_runtime_zone → +1。
    registry
        .register_runtime_zone(make_zone(
            "tsy_lingxu_01_shallow",
            crate::world::dimension::DimensionKind::Tsy,
        ))
        .expect("first add ok");
    assert_eq!(
        registry.spatial_revision, 1,
        "successful runtime add must bump spatial_revision"
    );

    // 重名 register 被拒绝 → 不变。
    registry
        .register_runtime_zone(make_zone(
            "tsy_lingxu_01_shallow",
            crate::world::dimension::DimensionKind::Tsy,
        ))
        .expect_err("duplicate name should be rejected");
    assert_eq!(
        registry.spatial_revision, 1,
        "rejected duplicate add must leave spatial_revision unchanged"
    );

    // 非空 TSY blueprint 并入 → +1。
    let path = unique_temp_path("bong-zones-tsy-revision", ".json");
    fs::write(
        &path,
        r#"{
  "zones": [
{
  "name": "tsy_revision_01_shallow",
  "dimension": "tsy",
  "aabb": { "min": [0.0, 20.0, 0.0], "max": [16.0, 40.0, 16.0] },
  "spirit_qi": -0.5,
  "danger_level": 2
}
  ]
}"#,
    )
    .expect("fixture should be writable");
    let loaded = registry
        .merge_tsy_blueprint_from_path(&path)
        .expect("non-conflicting TSY merge ok");
    assert_eq!(loaded, 1, "one supplemental zone should be merged");
    assert_eq!(
        registry.spatial_revision, 2,
        "non-empty supplemental merge must bump spatial_revision"
    );

    // 空并入（文件缺失 → Ok(0)）→ 不变。
    let missing = unique_temp_path("bong-zones-tsy-revision-missing", ".json");
    let loaded_missing = registry
        .merge_tsy_blueprint_from_path(&missing)
        .expect("missing TSY blueprint path is Ok(0)");
    assert_eq!(loaded_missing, 0);
    assert_eq!(
        registry.spatial_revision, 2,
        "empty merge must leave spatial_revision unchanged"
    );

    // 被拒绝的并入（与刚并入的合法 TSY zone 重名）→ 冲突 Err 且不变。
    // 使用非 TSY 的 `spawn` 会先撞 supplemental identity gate，覆盖不到重名分支。
    let conflict = unique_temp_path("bong-zones-tsy-revision-conflict", ".json");
    fs::write(
        &conflict,
        r#"{
  "zones": [
{
  "name": "tsy_revision_01_shallow",
  "dimension": "tsy",
  "aabb": { "min": [32.0, 20.0, 32.0], "max": [48.0, 40.0, 48.0] },
  "spirit_qi": -0.5,
  "danger_level": 2
}
  ]
}"#,
    )
    .expect("fixture should be writable");
    let conflict_error = registry
        .merge_tsy_blueprint_from_path(&conflict)
        .expect_err("registered TSY name conflict must be rejected");
    assert_eq!(
        conflict_error,
        "zone `tsy_revision_01_shallow` already registered; TSY blueprint merge rejected",
        "fixture must reach the existing-name conflict branch"
    );
    assert_eq!(
        registry.spatial_revision, 2,
        "rejected existing-name merge must leave spatial_revision unchanged"
    );
}
