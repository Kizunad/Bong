use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const EMIT_MANIFEST: &[(&str, &str)] = &[
    ("server/src/identity/wanted_player_emit.rs", "non_s2c"),
    ("server/src/lingtian/network_emit.rs", "server_data"),
    ("server/src/network/alchemy_snapshot_emit.rs", "server_data"),
    ("server/src/network/anqi_hud_emit.rs", "server_data"),
    ("server/src/network/ascension_quota_emit.rs", "server_data"),
    ("server/src/network/audio_event_emit.rs", "side_channel"),
    ("server/src/network/burst_event_emit.rs", "server_data"),
    ("server/src/network/carrier_state_emit.rs", "server_data"),
    ("server/src/network/cast_emit.rs", "server_data"),
    ("server/src/network/combat_event_emit.rs", "server_data"),
    ("server/src/network/combat_hud_state_emit.rs", "server_data"),
    ("server/src/network/craft_emit.rs", "server_data"),
    ("server/src/network/cultivation_detail_emit.rs", "mixed"),
    (
        "server/src/network/cultivation_insight_offer_emit.rs",
        "server_data",
    ),
    (
        "server/src/network/daozhan_disguise_emit.rs",
        "side_channel",
    ),
    ("server/src/network/defense_window_emit.rs", "server_data"),
    ("server/src/network/derived_attrs_emit.rs", "server_data"),
    (
        "server/src/network/dropped_loot_sync_emit.rs",
        "server_data",
    ),
    ("server/src/network/dugu_state_emit.rs", "server_data"),
    ("server/src/network/elder_encounter_emit.rs", "side_channel"),
    ("server/src/network/era_ambiance_emit.rs", "side_channel"),
    ("server/src/network/event_stream_emit.rs", "server_data"),
    ("server/src/network/extract_emit.rs", "server_data"),
    ("server/src/network/false_skin_state_emit.rs", "server_data"),
    ("server/src/network/forge_snapshot_emit.rs", "server_data"),
    ("server/src/network/freshness_probe_emit.rs", "server_data"),
    ("server/src/network/full_power_emit.rs", "server_data"),
    (
        "server/src/network/halfstep_rechallenge_emit.rs",
        "side_channel",
    ),
    ("server/src/network/identity_panel_emit.rs", "server_data"),
    ("server/src/network/inventory_event_emit.rs", "server_data"),
    (
        "server/src/network/inventory_move_rejected_emit.rs",
        "server_data",
    ),
    (
        "server/src/network/inventory_snapshot_emit.rs",
        "server_data",
    ),
    ("server/src/network/knockback_sync_emit.rs", "server_data"),
    ("server/src/network/meridian_severed_emit.rs", "domain_only"),
    ("server/src/network/mineral_probe_emit.rs", "server_data"),
    ("server/src/network/morph_state_emit.rs", "server_data"),
    ("server/src/network/mutation_visual_emit.rs", "side_channel"),
    ("server/src/network/npc_lod_emit.rs", "side_channel"),
    ("server/src/network/poison_trait_emit.rs", "server_data"),
    ("server/src/network/qi_attrition_emit.rs", "side_channel"),
    (
        "server/src/network/qi_color_observed_emit.rs",
        "side_channel",
    ),
    ("server/src/network/quickslot_config_emit.rs", "server_data"),
    ("server/src/network/race_gate_meta_emit.rs", "server_data"),
    ("server/src/network/rat_qi_tier_emit.rs", "side_channel"),
    ("server/src/network/remains_sync_emit.rs", "server_data"),
    ("server/src/network/scroll_open_emit.rs", "server_data"),
    ("server/src/network/skill_config_emit.rs", "server_data"),
    ("server/src/network/skill_emit.rs", "server_data"),
    ("server/src/network/skill_snapshot_emit.rs", "server_data"),
    ("server/src/network/skillbar_config_emit.rs", "server_data"),
    ("server/src/network/spider_disguise_emit.rs", "side_channel"),
    ("server/src/network/spirit_treasure_emit.rs", "server_data"),
    ("server/src/network/status_snapshot_emit.rs", "server_data"),
    ("server/src/network/sword_bond_state_emit.rs", "server_data"),
    (
        "server/src/network/techniques_snapshot_emit.rs",
        "server_data",
    ),
    (
        "server/src/network/treasure_equipped_emit.rs",
        "server_data",
    ),
    (
        "server/src/network/tribulation_broadcast_emit.rs",
        "server_data",
    ),
    (
        "server/src/network/tribulation_heart_demon_offer_emit.rs",
        "server_data",
    ),
    (
        "server/src/network/tribulation_state_emit.rs",
        "server_data",
    ),
    (
        "server/src/network/tsy_container_search_emit.rs",
        "server_data",
    ),
    ("server/src/network/tuike_ash_emit.rs", "domain_only"),
    ("server/src/network/unlocks_sync_emit.rs", "server_data"),
    ("server/src/network/vfx_event_emit.rs", "side_channel"),
    (
        "server/src/network/void_erosion_visual_emit.rs",
        "side_channel",
    ),
    ("server/src/network/weapon_equipped_emit.rs", "server_data"),
    ("server/src/network/woliu_state_emit.rs", "server_data"),
    ("server/src/network/wounds_snapshot_emit.rs", "server_data"),
    ("server/src/network/yidao_state_emit.rs", "server_data"),
];

const TRANSPORT_CLASSES: &[(&str, usize)] = &[
    ("server_data", 51),
    ("side_channel", 13),
    ("mixed", 1),
    ("domain_only", 2),
    ("non_s2c", 1),
];

const JOIN_SNAPSHOT_SYMBOLS: &[(&str, &str)] = &[
    ("server/src/network/mod.rs", "send_welcome_payload_on_join"),
    ("server/src/network/mod.rs", "emit_player_state_payloads"),
    (
        "server/src/network/inventory_snapshot_emit.rs",
        "emit_join_inventory_snapshots",
    ),
    (
        "server/src/network/skill_snapshot_emit.rs",
        "emit_join_skill_snapshots",
    ),
    (
        "server/src/network/techniques_snapshot_emit.rs",
        "emit_join_techniques_snapshot_payloads",
    ),
    (
        "server/src/network/craft_emit.rs",
        "emit_recipe_list_on_join",
    ),
    (
        "server/src/network/cultivation_detail_emit.rs",
        "emit_cultivation_detail_payloads",
    ),
    (
        "server/src/network/cultivation_detail_emit.rs",
        "emit_body_plan_layout_payloads",
    ),
    (
        "server/src/network/race_gate_meta_emit.rs",
        "emit_race_gate_meta_payloads",
    ),
    (
        "server/src/network/morph_state_emit.rs",
        "emit_morph_state_payloads",
    ),
    (
        "server/src/network/dropped_loot_sync_emit.rs",
        "emit_join_dropped_loot_syncs",
    ),
    (
        "server/src/network/remains_sync_emit.rs",
        "emit_join_remains_syncs",
    ),
    (
        "server/src/network/extract_emit.rs",
        "emit_rift_portal_state_payloads_to_joined_clients",
    ),
    (
        "server/src/network/tsy_container_search_emit.rs",
        "emit_container_state_payloads_to_joined_clients",
    ),
    (
        "server/src/network/tribulation_state_emit.rs",
        "emit_tribulation_state_payloads",
    ),
    (
        "server/src/network/tribulation_broadcast_emit.rs",
        "emit_tribulation_broadcast_payloads",
    ),
    (
        "server/src/network/ascension_quota_emit.rs",
        "emit_ascension_quota_payloads",
    ),
    (
        "server/src/network/spider_disguise_emit.rs",
        "on_player_join_send_spider_disguise_list",
    ),
    (
        "server/src/network/daozhan_disguise_emit.rs",
        "on_player_join_send_daozhan_disguise_list",
    ),
    (
        "server/src/network/rat_qi_tier_emit.rs",
        "on_player_join_send_rat_qi_tiers",
    ),
    (
        "server/src/network/era_ambiance_emit.rs",
        "era_ambiance_on_join_system",
    ),
    (
        "server/src/network/zone_environment_bridge.rs",
        "mark_zone_environment_dirty_for_new_clients",
    ),
    (
        "server/src/world/spawn_tutorial.rs",
        "send_tutorial_coffin_pos_on_join",
    ),
    (
        "server/src/coffin/mod.rs",
        "emit_coffin_state_to_joined_clients",
    ),
    (
        "server/src/social/mod.rs",
        "emit_anonymity_payloads_for_joined_clients",
    ),
];

#[test]
fn emit_file_inventory_and_transport_classification_stay_frozen() {
    let root = repository_root();
    let mut actual_paths = Vec::new();
    collect_emit_files(&root.join("server/src"), &root, &mut actual_paths);
    actual_paths.sort();

    let expected_paths: Vec<String> = EMIT_MANIFEST
        .iter()
        .map(|(path, _)| (*path).to_string())
        .collect();
    assert_eq!(
        expected_paths, actual_paths,
        "R6 emit census drifted; classify every added/removed *_emit.rs in the P0 manifest"
    );
    assert_eq!(
        actual_paths.len(),
        68,
        "P0 baseline is exactly 68 emit files"
    );

    let mut counts = BTreeMap::new();
    for (relative, expected_class) in EMIT_MANIFEST {
        let source = fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("read {relative}: {error}"));
        let actual_class = classify_emit_transport(relative, &source);
        assert_eq!(
            *expected_class, actual_class,
            "{relative} changed transport class; update the R6 migration ledger deliberately"
        );
        *counts.entry(actual_class).or_insert(0usize) += 1;
    }
    assert_eq!(
        counts,
        TRANSPORT_CLASSES.iter().copied().collect(),
        "R6 transport classes or their P0 counts drifted"
    );
}

#[test]
fn join_snapshot_producer_symbols_stay_present() {
    let root = repository_root();
    for (relative, symbol) in JOIN_SNAPSHOT_SYMBOLS {
        let source = fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("read {relative}: {error}"));
        assert!(
            source.contains(symbol),
            "join snapshot producer `{symbol}` disappeared from {relative}; update the authoritative R6 replay contract"
        );
    }

    let alchemy = fs::read_to_string(root.join("server/src/network/alchemy_snapshot_emit.rs"))
        .expect("read alchemy snapshot emitter");
    assert!(
        alchemy.contains("alchemy_join_mocks_enabled"),
        "alchemy join remains explicitly mock-gated until a production snapshot exists"
    );
    let forge = fs::read_to_string(root.join("server/src/network/forge_snapshot_emit.rs"))
        .expect("read forge snapshot emitter");
    assert!(
        forge.contains("join hydration placeholder"),
        "forge join emitter remains a documented placeholder until a real snapshot is implemented"
    );
}

fn classify_emit_transport(relative: &str, source: &str) -> &'static str {
    if relative == "server/src/identity/wanted_player_emit.rs" {
        return "non_s2c";
    }
    let server_data = source.contains("send_server_data_payload");
    let direct = source.contains("send_custom_payload");
    match (server_data, direct) {
        (true, true) => "mixed",
        (true, false) => "server_data",
        (false, true) => "side_channel",
        (false, false) => "domain_only",
    }
}

fn collect_emit_files(directory: &Path, root: &Path, output: &mut Vec<String>) {
    for entry in fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read directory {}: {error}", directory.display()))
    {
        let path = entry.expect("read directory entry").path();
        if path.is_dir() {
            collect_emit_files(&path, root, output);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with("_emit.rs"))
        {
            output.push(
                path.strip_prefix(root)
                    .expect("emit file is below repository root")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

fn repository_root() -> PathBuf {
    let server = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    server
        .parent()
        .expect("server crate is below repository root")
        .to_path_buf()
}

#[test]
fn manifest_has_no_duplicate_paths() {
    let paths: BTreeSet<_> = EMIT_MANIFEST.iter().map(|(path, _)| *path).collect();
    assert_eq!(paths.len(), EMIT_MANIFEST.len());
}
