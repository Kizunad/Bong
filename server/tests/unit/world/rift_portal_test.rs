use std::path::Path;

use bong_server::world::rift_portal::{load_tsy_portals_from_path, RiftKind};

#[test]
fn rift_kind_extract_table_matches_worldview() {
    assert_eq!(RiftKind::MainRift.base_extract_ticks(), 160);
    assert_eq!(RiftKind::DeepRift.base_extract_ticks(), 240);
    assert_eq!(RiftKind::CollapseTear.base_extract_ticks(), 60);
}

#[test]
fn rift_kind_entry_exit_permissions() {
    assert!(RiftKind::MainRift.allows_entry());
    assert!(!RiftKind::DeepRift.allows_entry());
    assert!(!RiftKind::CollapseTear.allows_entry());
    assert!(RiftKind::MainRift.allows_exit());
    assert!(RiftKind::DeepRift.allows_exit());
    assert!(RiftKind::CollapseTear.allows_exit());
}

#[test]
fn default_tsy_portals_fixture_loads() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tsy_portals.json");
    let registry = load_tsy_portals_from_path(path);
    let family = registry
        .by_family
        .get("tsy_lingxu_01")
        .expect("fixture should define tsy_lingxu_01");

    assert_eq!(family.shallow.len(), 1);
    assert_eq!(family.shallow[0].kind, RiftKind::MainRift);
    assert_eq!(family.deep.len(), 1);
    assert_eq!(family.deep[0].kind, RiftKind::DeepRift);
}
