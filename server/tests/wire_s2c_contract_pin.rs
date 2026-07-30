use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const EMIT_MANIFEST: &[(&str, &str)] = &[
    ("server/src/identity/wanted_player_emit.rs", "redis_only"),
    ("server/src/lingtian/network_emit.rs", "helper_only"),
    ("server/src/network/alchemy_snapshot_emit.rs", "helper_only"),
    ("server/src/network/anqi_hud_emit.rs", "helper_only"),
    ("server/src/network/ascension_quota_emit.rs", "helper_only"),
    ("server/src/network/audio_event_emit.rs", "direct_only"),
    ("server/src/network/burst_event_emit.rs", "helper_only"),
    ("server/src/network/carrier_state_emit.rs", "helper_only"),
    ("server/src/network/cast_emit.rs", "helper_only"),
    ("server/src/network/combat_event_emit.rs", "helper_only"),
    ("server/src/network/combat_hud_state_emit.rs", "helper_only"),
    ("server/src/network/craft_emit.rs", "helper_only"),
    ("server/src/network/cultivation_detail_emit.rs", "both"),
    (
        "server/src/network/cultivation_insight_offer_emit.rs",
        "helper_only",
    ),
    ("server/src/network/daozhan_disguise_emit.rs", "direct_only"),
    ("server/src/network/defense_window_emit.rs", "helper_only"),
    ("server/src/network/derived_attrs_emit.rs", "helper_only"),
    (
        "server/src/network/dropped_loot_sync_emit.rs",
        "helper_only",
    ),
    ("server/src/network/dugu_state_emit.rs", "helper_only"),
    ("server/src/network/elder_encounter_emit.rs", "direct_only"),
    ("server/src/network/era_ambiance_emit.rs", "direct_only"),
    ("server/src/network/event_stream_emit.rs", "helper_only"),
    ("server/src/network/extract_emit.rs", "helper_only"),
    ("server/src/network/false_skin_state_emit.rs", "helper_only"),
    ("server/src/network/forge_snapshot_emit.rs", "helper_only"),
    ("server/src/network/freshness_probe_emit.rs", "helper_only"),
    ("server/src/network/full_power_emit.rs", "helper_only"),
    (
        "server/src/network/halfstep_rechallenge_emit.rs",
        "direct_only",
    ),
    ("server/src/network/identity_panel_emit.rs", "helper_only"),
    ("server/src/network/inventory_event_emit.rs", "helper_only"),
    (
        "server/src/network/inventory_move_rejected_emit.rs",
        "helper_only",
    ),
    (
        "server/src/network/inventory_snapshot_emit.rs",
        "helper_only",
    ),
    ("server/src/network/knockback_sync_emit.rs", "helper_only"),
    (
        "server/src/network/meridian_severed_emit.rs",
        "no_client_send",
    ),
    ("server/src/network/mineral_probe_emit.rs", "helper_only"),
    ("server/src/network/morph_state_emit.rs", "helper_only"),
    ("server/src/network/mutation_visual_emit.rs", "direct_only"),
    ("server/src/network/npc_lod_emit.rs", "direct_only"),
    ("server/src/network/poison_trait_emit.rs", "helper_only"),
    ("server/src/network/qi_attrition_emit.rs", "direct_only"),
    (
        "server/src/network/qi_color_observed_emit.rs",
        "direct_only",
    ),
    ("server/src/network/quickslot_config_emit.rs", "helper_only"),
    ("server/src/network/race_gate_meta_emit.rs", "helper_only"),
    ("server/src/network/rat_qi_tier_emit.rs", "direct_only"),
    ("server/src/network/remains_sync_emit.rs", "helper_only"),
    ("server/src/network/scroll_open_emit.rs", "helper_only"),
    ("server/src/network/skill_config_emit.rs", "helper_only"),
    ("server/src/network/skill_emit.rs", "helper_only"),
    ("server/src/network/skill_snapshot_emit.rs", "helper_only"),
    ("server/src/network/skillbar_config_emit.rs", "helper_only"),
    ("server/src/network/spider_disguise_emit.rs", "direct_only"),
    ("server/src/network/spirit_treasure_emit.rs", "helper_only"),
    ("server/src/network/status_snapshot_emit.rs", "helper_only"),
    ("server/src/network/sword_bond_state_emit.rs", "helper_only"),
    (
        "server/src/network/techniques_snapshot_emit.rs",
        "helper_only",
    ),
    (
        "server/src/network/treasure_equipped_emit.rs",
        "helper_only",
    ),
    (
        "server/src/network/tribulation_broadcast_emit.rs",
        "helper_only",
    ),
    (
        "server/src/network/tribulation_heart_demon_offer_emit.rs",
        "helper_only",
    ),
    (
        "server/src/network/tribulation_state_emit.rs",
        "helper_only",
    ),
    (
        "server/src/network/tsy_container_search_emit.rs",
        "helper_only",
    ),
    ("server/src/network/tuike_ash_emit.rs", "no_client_send"),
    ("server/src/network/unlocks_sync_emit.rs", "helper_only"),
    ("server/src/network/vfx_event_emit.rs", "direct_only"),
    (
        "server/src/network/void_erosion_visual_emit.rs",
        "direct_only",
    ),
    ("server/src/network/weapon_equipped_emit.rs", "helper_only"),
    ("server/src/network/woliu_state_emit.rs", "helper_only"),
    ("server/src/network/wounds_snapshot_emit.rs", "helper_only"),
    ("server/src/network/yidao_state_emit.rs", "helper_only"),
];

const API_SHAPES: &[(&str, usize)] = &[
    ("helper_only", 51),
    ("direct_only", 13),
    ("both", 1),
    ("no_client_send", 2),
    ("redis_only", 1),
];

const WIRE_CLASSES: &[(&str, usize)] = &[
    ("server_data_only", 53),
    ("dedicated_only", 12),
    ("channel_mixed", 0),
    ("domain_only", 2),
    ("redis_only", 1),
];

macro_rules! replay_pin {
    (
        $class:ident,
        $source:literal,
        $symbol:literal,
        [$($marker:literal),+ $(,)?],
        $registration_source:literal,
        $registration_function:literal,
        $registered_callee:literal
    ) => {
        ReplayPin {
            class: ReplayClass::$class,
            source: $source,
            symbol: $symbol,
            trigger_markers: &[$($marker),+],
            registration_source: $registration_source,
            registration_function: $registration_function,
            registered_callee: $registered_callee,
        }
    };
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ReplayClass {
    ProtocolHandshake,
    StrictJoin,
    JoinDerived,
    ActiveReplay,
    DefectiveReplay,
    CacheMissImmediate,
    CacheMissAtCadence,
    PeriodicConvergence,
}

#[derive(Clone, Copy, Debug)]
struct ReplayPin {
    class: ReplayClass,
    source: &'static str,
    symbol: &'static str,
    trigger_markers: &'static [&'static str],
    registration_source: &'static str,
    registration_function: &'static str,
    registered_callee: &'static str,
}

const REPLAY_PINS: &[ReplayPin] = &[
    replay_pin!(
        ProtocolHandshake,
        "server/src/network/mod.rs",
        "send_welcome_payload_on_join",
        ["Added", "Client"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "send_welcome_payload_on_join"
    ),
    replay_pin!(
        StrictJoin,
        "server/src/network/dropped_loot_sync_emit.rs",
        "emit_join_dropped_loot_syncs",
        ["JoinedDropSyncClientFilter"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "dropped_loot_sync_emit::emit_join_dropped_loot_syncs"
    ),
    replay_pin!(
        StrictJoin,
        "server/src/network/remains_sync_emit.rs",
        "emit_join_remains_syncs",
        ["JoinedRemainsSyncClientFilter"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "remains_sync_emit::emit_join_remains_syncs"
    ),
    replay_pin!(
        StrictJoin,
        "server/src/network/extract_emit.rs",
        "emit_rift_portal_state_payloads_to_joined_clients",
        ["Added", "Client"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "extract_emit::emit_rift_portal_state_payloads_to_joined_clients"
    ),
    replay_pin!(
        StrictJoin,
        "server/src/network/tsy_container_search_emit.rs",
        "emit_container_state_payloads_to_joined_clients",
        ["Added", "Client"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "tsy_container_search_emit::emit_container_state_payloads_to_joined_clients"
    ),
    replay_pin!(
        StrictJoin,
        "server/src/network/spider_disguise_emit.rs",
        "on_player_join_send_spider_disguise_list",
        ["Added", "Client"],
        "server/src/network/spider_disguise_emit.rs",
        "register",
        "on_player_join_send_spider_disguise_list"
    ),
    replay_pin!(
        StrictJoin,
        "server/src/network/daozhan_disguise_emit.rs",
        "on_player_join_send_daozhan_disguise_list",
        ["Added", "Client"],
        "server/src/network/daozhan_disguise_emit.rs",
        "register",
        "on_player_join_send_daozhan_disguise_list"
    ),
    replay_pin!(
        StrictJoin,
        "server/src/network/rat_qi_tier_emit.rs",
        "on_player_join_send_rat_qi_tiers",
        ["Added", "Client"],
        "server/src/network/rat_qi_tier_emit.rs",
        "register",
        "on_player_join_send_rat_qi_tiers"
    ),
    replay_pin!(
        StrictJoin,
        "server/src/network/era_ambiance_emit.rs",
        "era_ambiance_on_join_system",
        ["Added", "Client"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "era_ambiance_emit::era_ambiance_on_join_system"
    ),
    replay_pin!(
        StrictJoin,
        "server/src/coffin/mod.rs",
        "emit_coffin_state_to_joined_clients",
        ["Added", "Client"],
        "server/src/coffin/mod.rs",
        "register",
        "emit_coffin_state_to_joined_clients"
    ),
    replay_pin!(
        StrictJoin,
        "server/src/combat/lifecycle.rs",
        "reemit_death_screen_for_reconnected_awaiting_revival_clients",
        ["Added", "Client", "AwaitingRevival"],
        "server/src/combat/mod.rs",
        "register",
        "lifecycle::reemit_death_screen_for_reconnected_awaiting_revival_clients"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/mod.rs",
        "emit_player_state_payloads",
        ["PlayerStateEmitQueryFilter"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "emit_player_state_payloads"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/inventory_snapshot_emit.rs",
        "emit_join_inventory_snapshots",
        ["Added", "PlayerInventory"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "inventory_snapshot_emit::emit_join_inventory_snapshots"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/skill_snapshot_emit.rs",
        "emit_join_skill_snapshots",
        ["Added", "SkillSet"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "skill_snapshot_emit::emit_join_skill_snapshots"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/techniques_snapshot_emit.rs",
        "emit_join_techniques_snapshot_payloads",
        ["JoinTechniquesSnapshotFilter"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "techniques_snapshot_emit::emit_join_techniques_snapshot_payloads"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/cultivation/realm_vision/push.rs",
        "push_initial_realm_vision",
        ["JoinedRealmVisionClientFilter"],
        "server/src/cultivation/mod.rs",
        "register",
        "push_initial_realm_vision"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/social/mod.rs",
        "emit_anonymity_payloads_for_joined_clients",
        ["Added", "Anonymity"],
        "server/src/social/mod.rs",
        "register",
        "emit_anonymity_payloads_for_joined_clients"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/zone_environment_bridge.rs",
        "mark_zone_environment_dirty_for_new_clients",
        ["Added", "Client", "mark_all_dirty_for_snapshot"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "zone_environment_bridge::mark_zone_environment_dirty_for_new_clients"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/zone_environment_bridge.rs",
        "zone_environment_broadcast_system",
        ["drain_dirty", "send_custom_payload"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "zone_environment_bridge::zone_environment_broadcast_system"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/identity_panel_emit.rs",
        "emit_identity_panel_state_payloads",
        ["is_added", "is_changed"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "identity_panel_emit::emit_identity_panel_state_payloads"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/quickslot_config_emit.rs",
        "emit_quickslot_config_payloads",
        ["QuickSlotEmitFilter"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "quickslot_config_emit::emit_quickslot_config_payloads"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/skillbar_config_emit.rs",
        "emit_skillbar_config_payloads",
        ["SkillBarEmitFilter"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "skillbar_config_emit::emit_skillbar_config_payloads"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/unlocks_sync_emit.rs",
        "emit_unlocks_sync_payloads",
        ["UnlocksEmitFilter"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "unlocks_sync_emit::emit_unlocks_sync_payloads"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/combat_hud_state_emit.rs",
        "emit_combat_hud_state_payloads",
        ["CombatHudEmitFilter"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "combat_hud_state_emit::emit_combat_hud_state_payloads"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/wounds_snapshot_emit.rs",
        "emit_wounds_snapshot_payloads",
        ["WoundsEmitFilter"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "wounds_snapshot_emit::emit_wounds_snapshot_payloads"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/derived_attrs_emit.rs",
        "emit_derived_attrs_sync_payloads",
        ["DerivedAttrsEmitQuery", "removed_tribulations"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "derived_attrs_emit::emit_derived_attrs_sync_payloads"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/status_snapshot_emit.rs",
        "emit_status_snapshot_payloads",
        ["StatusSnapshotEmitFilter"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "status_snapshot_emit::emit_status_snapshot_payloads"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/weapon_equipped_emit.rs",
        "emit_weapon_equipped_payloads",
        ["Changed", "PlayerInventory"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "weapon_equipped_emit::emit_weapon_equipped_payloads"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/treasure_equipped_emit.rs",
        "emit_treasure_equipped_payloads",
        ["Changed", "PlayerInventory"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "treasure_equipped_emit::emit_treasure_equipped_payloads"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/spirit_treasure_emit.rs",
        "emit_spirit_treasure_state_payloads",
        ["SpiritTreasureStateClientFilter"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "spirit_treasure_emit::emit_spirit_treasure_state_payloads"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/false_skin_state_emit.rs",
        "emit_tuike_v2_false_skin_state_payloads",
        ["Changed", "StackedFalseSkins"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "false_skin_state_emit::emit_tuike_v2_false_skin_state_payloads"
    ),
    replay_pin!(
        JoinDerived,
        "server/src/network/craft_emit.rs",
        "apply_material_discovery_unlock",
        ["Changed", "PlayerInventory"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "craft_emit::apply_material_discovery_unlock"
    ),
    replay_pin!(
        ActiveReplay,
        "server/src/network/tribulation_state_emit.rs",
        "emit_tribulation_state_payloads",
        ["known_clients", "HashSet", "difference"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "tribulation_state_emit::emit_tribulation_state_payloads"
    ),
    replay_pin!(
        ActiveReplay,
        "server/src/network/tribulation_broadcast_emit.rs",
        "emit_tribulation_broadcast_payloads",
        ["known_clients", "HashSet", "difference"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "tribulation_broadcast_emit::emit_tribulation_broadcast_payloads"
    ),
    replay_pin!(
        DefectiveReplay,
        "server/src/network/ascension_quota_emit.rs",
        "emit_ascension_quota_payloads",
        ["last_client_count", "joined_count", "count_changed"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "ascension_quota_emit::emit_ascension_quota_payloads"
    ),
    replay_pin!(
        CacheMissImmediate,
        "server/src/network/craft_emit.rs",
        "emit_recipe_list_on_join",
        ["Local", "HashMap", "Entity", "String"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "craft_emit::emit_recipe_list_on_join"
    ),
    replay_pin!(
        CacheMissImmediate,
        "server/src/network/cultivation_detail_emit.rs",
        "emit_body_plan_layout_payloads",
        ["LastSentBodyPlanLayout"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "cultivation_detail_emit::emit_body_plan_layout_payloads"
    ),
    replay_pin!(
        CacheMissImmediate,
        "server/src/network/race_gate_meta_emit.rs",
        "emit_race_gate_meta_payloads",
        ["LastSentRaceGateMeta"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "race_gate_meta_emit::emit_race_gate_meta_payloads"
    ),
    replay_pin!(
        CacheMissImmediate,
        "server/src/network/morph_state_emit.rs",
        "emit_morph_state_payloads",
        ["LastSentMorphStateJoin"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "morph_state_emit::emit_morph_state_payloads"
    ),
    replay_pin!(
        CacheMissImmediate,
        "server/src/network/mod.rs",
        "emit_zone_info_on_zone_transition",
        ["last_zone_by_entity", "last_snapshot_by_entity"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "emit_zone_info_on_zone_transition"
    ),
    replay_pin!(
        CacheMissImmediate,
        "server/src/world/spawn_tutorial.rs",
        "send_tutorial_coffin_pos_on_join",
        ["UnsentTutorialCoffinPosFilter"],
        "server/src/world/spawn_tutorial.rs",
        "register",
        "send_tutorial_coffin_pos_on_join"
    ),
    replay_pin!(
        CacheMissImmediate,
        "server/src/network/skill_config_emit.rs",
        "emit_skill_config_snapshots",
        ["Local", "HashMap", "Entity", "SkillConfigSnapshot"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "skill_config_emit::emit_skill_config_snapshots"
    ),
    replay_pin!(
        CacheMissImmediate,
        "server/src/audio/ambient.rs",
        "ambient_zone_change_system",
        ["last_by_entity"],
        "server/src/audio/ambient.rs",
        "register",
        "ambient_zone_change_system"
    ),
    replay_pin!(
        CacheMissImmediate,
        "server/src/network/yidao_state_emit.rs",
        "emit_healer_npc_ai_state_payloads",
        ["Local", "HashMap", "Entity", "HealerNpcAiStateV1"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "yidao_state_emit::emit_healer_npc_ai_state_payloads"
    ),
    replay_pin!(
        CacheMissImmediate,
        "server/src/network/carrier_state_emit.rs",
        "emit_carrier_state_payloads",
        ["CarrierStateEmitCache", "is_multiple_of"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "carrier_state_emit::emit_carrier_state_payloads"
    ),
    replay_pin!(
        CacheMissAtCadence,
        "server/src/network/npc_mood.rs",
        "emit_npc_mood_payloads",
        ["NPC_MOOD_SYNC_INTERVAL_TICKS", "last_sent"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "npc_mood::emit_npc_mood_payloads"
    ),
    replay_pin!(
        CacheMissAtCadence,
        "server/src/network/tsy_polish.rs",
        "emit_tsy_boss_health_payloads",
        ["TSY_BOSS_HEALTH_SYNC_INTERVAL_TICKS", "last_sent"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "tsy_polish::emit_tsy_boss_health_payloads"
    ),
    replay_pin!(
        PeriodicConvergence,
        "server/src/network/cultivation_detail_emit.rs",
        "emit_cultivation_detail_payloads",
        ["CultivationDetailEmitState", "EMIT_INTERVAL_TICKS"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "cultivation_detail_emit::emit_cultivation_detail_payloads"
    ),
    replay_pin!(
        PeriodicConvergence,
        "server/src/network/morph_state_emit.rs",
        "emit_morph_state_payloads",
        ["MORPH_STATE_SYNC_INTERVAL_TICKS"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "morph_state_emit::emit_morph_state_payloads"
    ),
    replay_pin!(
        PeriodicConvergence,
        "server/src/network/npc_lod_emit.rs",
        "emit_npc_lod_payloads",
        ["NPC_LOD_MID_SYNC_INTERVAL_TICKS"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "npc_lod_emit::emit_npc_lod_payloads"
    ),
    replay_pin!(
        PeriodicConvergence,
        "server/src/network/spider_disguise_emit.rs",
        "periodic_spider_disguise_sync_system",
        ["SPIDER_DISGUISE_SYNC_INTERVAL_TICKS"],
        "server/src/network/spider_disguise_emit.rs",
        "register",
        "periodic_spider_disguise_sync_system"
    ),
    replay_pin!(
        PeriodicConvergence,
        "server/src/network/daozhan_disguise_emit.rs",
        "periodic_daozhan_disguise_sync_system",
        ["DAOZHAN_DISGUISE_SYNC_INTERVAL_TICKS"],
        "server/src/network/daozhan_disguise_emit.rs",
        "register",
        "periodic_daozhan_disguise_sync_system"
    ),
    replay_pin!(
        PeriodicConvergence,
        "server/src/network/rat_qi_tier_emit.rs",
        "periodic_rat_qi_tier_sync_system",
        ["RAT_QI_TIER_SYNC_INTERVAL_TICKS"],
        "server/src/network/rat_qi_tier_emit.rs",
        "register",
        "periodic_rat_qi_tier_sync_system"
    ),
    replay_pin!(
        PeriodicConvergence,
        "server/src/network/craft_emit.rs",
        "emit_craft_session_state",
        ["CraftSessionStateDirty", "sessions_with_dirty"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "craft_emit::emit_craft_session_state"
    ),
    replay_pin!(
        PeriodicConvergence,
        "server/src/network/carrier_state_emit.rs",
        "emit_carrier_state_payloads",
        ["TICKS_PER_SECOND", "is_multiple_of"],
        "server/src/network/mod.rs",
        "register_app_wiring",
        "carrier_state_emit::emit_carrier_state_payloads"
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
    let mut wire_counts = BTreeMap::new();
    for (relative, expected_class) in EMIT_MANIFEST {
        let source = fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("read {relative}: {error}"));
        let scan = scan_production_emit_source(&source);
        let actual_shape = classify_emit_api_shape(&scan);
        assert_eq!(
            *expected_class, actual_shape,
            "{relative} changed API call shape; update the R6 migration ledger deliberately"
        );
        *counts.entry(actual_shape).or_insert(0usize) += 1;
        *wire_counts
            .entry(classify_wire_transport(&scan))
            .or_insert(0usize) += 1;
    }
    assert_eq!(
        counts,
        API_SHAPES.iter().copied().collect(),
        "R6 emitter API call shapes or their P0 counts drifted"
    );
    let actual_wire_counts: BTreeMap<_, _> = WIRE_CLASSES
        .iter()
        .map(|(class, _)| (*class, *wire_counts.get(class).unwrap_or(&0)))
        .collect();
    assert_eq!(
        actual_wire_counts,
        WIRE_CLASSES.iter().copied().collect(),
        "R6 actual wire channels or their P0 counts drifted"
    );
}

#[test]
fn production_registration_roots_stay_connected() {
    let root = repository_root();
    let main = production_tokens_from_file(&root, "server/src/main.rs");
    let build_server_app = function_token_slice(&main, "build_server_app")
        .expect("production build_server_app must remain present");
    assert!(
        contains_token_path(build_server_app, "network::register"),
        "build_server_app must call network::register"
    );

    let network = production_tokens_from_file(&root, "server/src/network/mod.rs");
    let register =
        function_token_slice(&network, "register").expect("network::register must remain present");
    assert!(
        contains_token_path(register, "register_app_wiring"),
        "network::register must call register_app_wiring"
    );
}

#[test]
fn categorized_replay_producers_stay_registered_in_production_wiring() {
    let root = repository_root();
    let mut class_counts = BTreeMap::new();

    for pin in REPLAY_PINS {
        let source = production_tokens_from_file(&root, pin.source);
        let function = function_token_slice(&source, pin.symbol).unwrap_or_else(|| {
            panic!(
                "{:?} replay producer `{}` disappeared from {}; update the authoritative R6 replay contract",
                pin.class, pin.symbol, pin.source
            )
        });
        for marker in pin.trigger_markers {
            assert!(
                function.iter().any(|token| token == marker),
                "{:?} replay producer `{}` in {} lost trigger/cache marker `{marker}`",
                pin.class,
                pin.symbol,
                pin.source
            );
        }

        let registration = production_tokens_from_file(&root, pin.registration_source);
        let registration_function =
            function_token_slice(&registration, pin.registration_function).unwrap_or_else(|| {
                panic!(
                    "production registration function `{}` disappeared from {} for replay producer `{}`",
                    pin.registration_function, pin.registration_source, pin.symbol
                )
            });
        assert!(
            add_systems_registers_callee(registration_function, pin.registered_callee),
            "{:?} replay producer `{}` is not a callee inside app.add_systems(...) in {}::{} (expected `{}`)",
            pin.class,
            pin.symbol,
            pin.registration_source,
            pin.registration_function,
            pin.registered_callee
        );
        *class_counts.entry(pin.class).or_insert(0usize) += 1;
    }

    assert_eq!(
        class_counts,
        BTreeMap::from([
            (ReplayClass::ProtocolHandshake, 1),
            (ReplayClass::StrictJoin, 10),
            (ReplayClass::JoinDerived, 21),
            (ReplayClass::ActiveReplay, 2),
            (ReplayClass::DefectiveReplay, 1),
            (ReplayClass::CacheMissImmediate, 10),
            (ReplayClass::CacheMissAtCadence, 2),
            (ReplayClass::PeriodicConvergence, 8),
        ]),
        "R6 replay category counts drifted; classify additions/removals deliberately"
    );

    let alchemy = production_tokens_from_file(&root, "server/src/network/alchemy_snapshot_emit.rs");
    assert!(
        function_token_slice(&alchemy, "emit_join_alchemy_snapshots").is_some_and(|function| {
            function
                .iter()
                .any(|token| token == "alchemy_join_mocks_enabled")
        }),
        "alchemy join remains explicitly mock-gated until a production snapshot exists"
    );
    let forge_source = fs::read_to_string(root.join("server/src/network/forge_snapshot_emit.rs"))
        .expect("read forge snapshot emitter");
    assert!(
        forge_source.contains("join hydration placeholder"),
        "forge join emitter remains a documented placeholder until a real snapshot is implemented"
    );
}

fn production_tokens_from_file(root: &Path, relative: &str) -> Vec<String> {
    let source = fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"));
    rust_tokens(&source)
}

fn function_token_slice<'a>(tokens: &'a [String], symbol: &str) -> Option<&'a [String]> {
    for index in 0..tokens.len().saturating_sub(2) {
        if tokens[index] != "fn" || tokens[index + 1] != symbol {
            continue;
        }
        let body_start = tokens[index + 2..].iter().position(|token| token == "{")? + index + 2;
        let body_end = matching_delimiter(tokens, body_start, "{", "}")?;
        return Some(&tokens[index..=body_end]);
    }
    None
}

fn add_systems_registers_callee(function: &[String], callee: &str) -> bool {
    let expected = path_tokens(callee);
    let mut index = 0usize;
    while index + 3 < function.len() {
        if function[index] == "."
            && function[index + 1] == "add_systems"
            && function[index + 2] == "("
        {
            let Some(close) = matching_delimiter(function, index + 2, "(", ")") else {
                return false;
            };
            if function[index + 3..close]
                .windows(expected.len())
                .any(|window| window == expected)
            {
                return true;
            }
            index = close;
        }
        index += 1;
    }
    false
}

fn contains_token_path(tokens: &[String], path: &str) -> bool {
    let expected = path_tokens(path);
    tokens
        .windows(expected.len())
        .any(|window| window == expected)
}

fn path_tokens(path: &str) -> Vec<String> {
    path.split("::")
        .enumerate()
        .flat_map(|(index, segment)| {
            let mut tokens = Vec::new();
            if index > 0 {
                tokens.extend([":".to_string(), ":".to_string()]);
            }
            tokens.push(segment.to_string());
            tokens
        })
        .collect()
}

fn matching_delimiter(
    tokens: &[String],
    open_index: usize,
    open: &str,
    close: &str,
) -> Option<usize> {
    if tokens.get(open_index)? != open {
        return None;
    }
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open_index) {
        if token == open {
            depth += 1;
        } else if token == close {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn classify_emit_api_shape(scan: &EmitScan) -> &'static str {
    match (scan.helper_calls, scan.direct_calls) {
        (0, 0) if scan.redis_wanted_calls > 0 => "redis_only",
        (0, 0) => "no_client_send",
        (_, 0) => "helper_only",
        (0, _) => "direct_only",
        (_, _) => "both",
    }
}

fn classify_wire_transport(scan: &EmitScan) -> &'static str {
    let server_data = scan.helper_calls + scan.direct_server_data_calls;
    let dedicated = scan.direct_calls - scan.direct_server_data_calls;
    match (server_data, dedicated, scan.redis_wanted_calls) {
        (0, 0, redis) if redis > 0 => "redis_only",
        (0, 0, 0) => "domain_only",
        (_, 0, _) => "server_data_only",
        (0, _, _) => "dedicated_only",
        (_, _, _) => "channel_mixed",
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
struct EmitScan {
    helper_calls: usize,
    direct_calls: usize,
    direct_server_data_calls: usize,
    redis_wanted_calls: usize,
}

fn scan_production_emit_source(source: &str) -> EmitScan {
    let tokens = rust_tokens(source);
    let mut scan = EmitScan::default();
    for index in 0..tokens.len() {
        if tokens[index] == "send_server_data_payload"
            && tokens.get(index + 1).is_some_and(|token| token == "(")
        {
            scan.helper_calls += 1;
        }
        if tokens[index] == "send_custom_payload"
            && index
                .checked_sub(1)
                .and_then(|previous| tokens.get(previous))
                .is_some_and(|token| token == ".")
            && tokens.get(index + 1).is_some_and(|token| token == "(")
        {
            scan.direct_calls += 1;
            if first_call_argument(&tokens, index + 1).is_some_and(|argument| {
                argument.iter().any(|token| token == "SERVER_DATA_CHANNEL")
                    || argument
                        .iter()
                        .any(|token| token.contains("bong:server_data"))
            }) {
                scan.direct_server_data_calls += 1;
            }
        }
        if tokens[index] == "send"
            && index
                .checked_sub(1)
                .and_then(|previous| tokens.get(previous))
                .is_some_and(|token| token == ".")
            && tokens.get(index + 1).is_some_and(|token| token == "(")
            && first_call_argument(&tokens, index + 1).is_some_and(|argument| {
                argument
                    .windows(4)
                    .any(|pair| pair == ["RedisOutbound", ":", ":", "WantedPlayer"])
            })
        {
            scan.redis_wanted_calls += 1;
        }
    }
    scan
}

fn first_call_argument(tokens: &[String], open: usize) -> Option<&[String]> {
    if tokens.get(open)? != "(" {
        return None;
    }
    let mut depth = 0usize;
    for index in open + 1..tokens.len() {
        match tokens[index].as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" if depth == 0 => return Some(&tokens[open + 1..index]),
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "," if depth == 0 => return Some(&tokens[open + 1..index]),
            _ => {}
        }
    }
    None
}

fn rust_tokens(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
        } else if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/') {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
        } else if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
            index = skip_block_comment(bytes, index + 2);
        } else if bytes
            .get(index..)
            .is_some_and(|tail| tail.starts_with(b"#[cfg(test)]"))
        {
            index = skip_cfg_test_item(source, index + "#[cfg(test)]".len());
        } else if let Some(end) = raw_string_end(bytes, index) {
            tokens.push(String::from_utf8_lossy(&bytes[index..end]).into_owned());
            index = end;
        } else if bytes[index] == b'"' {
            let end = skip_quoted(bytes, index, b'"');
            tokens.push(String::from_utf8_lossy(&bytes[index..end]).into_owned());
            index = end;
        } else if let Some(end) = char_literal_end(bytes, index) {
            tokens.push(String::from_utf8_lossy(&bytes[index..end]).into_owned());
            index = end;
        } else if bytes[index] == b'_' || bytes[index].is_ascii_alphabetic() {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index] == b'_' || bytes[index].is_ascii_alphanumeric())
            {
                index += 1;
            }
            tokens.push(String::from_utf8_lossy(&bytes[start..index]).into_owned());
        } else {
            tokens.push(char::from(bytes[index]).to_string());
            index += 1;
        }
    }
    tokens
}

fn skip_cfg_test_item(source: &str, mut index: usize) -> usize {
    let bytes = source.as_bytes();
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    let mut cursor = index;
    while cursor < bytes.len() {
        if bytes[cursor] == b'/' && bytes.get(cursor + 1) == Some(&b'/') {
            cursor += 2;
            while cursor < bytes.len() && bytes[cursor] != b'\n' {
                cursor += 1;
            }
        } else if bytes[cursor] == b'/' && bytes.get(cursor + 1) == Some(&b'*') {
            cursor = skip_block_comment(bytes, cursor + 2);
        } else if let Some(end) = raw_string_end(bytes, cursor) {
            cursor = end;
        } else if bytes[cursor] == b'"' {
            cursor = skip_quoted(bytes, cursor, b'"');
        } else if let Some(end) = char_literal_end(bytes, cursor) {
            cursor = end;
        } else if bytes[cursor] == b'{' {
            return skip_braced_item(bytes, cursor);
        } else if bytes[cursor] == b';' {
            return cursor + 1;
        } else {
            cursor += 1;
        }
    }
    bytes.len()
}

fn skip_braced_item(bytes: &[u8], body_start: usize) -> usize {
    let mut index = body_start + 1;
    let mut depth = 1usize;
    while index < bytes.len() && depth > 0 {
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/') {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
        } else if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
            index = skip_block_comment(bytes, index + 2);
        } else if let Some(end) = raw_string_end(bytes, index) {
            index = end;
        } else if bytes[index] == b'"' {
            index = skip_quoted(bytes, index, b'"');
        } else if let Some(end) = char_literal_end(bytes, index) {
            index = end;
        } else {
            match bytes[index] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            index += 1;
        }
    }
    index
}

fn skip_block_comment(bytes: &[u8], mut index: usize) -> usize {
    let mut depth = 1usize;
    while index < bytes.len() && depth > 0 {
        if bytes.get(index..index + 2) == Some(b"/*") {
            depth += 1;
            index += 2;
        } else if bytes.get(index..index + 2) == Some(b"*/") {
            depth -= 1;
            index += 2;
        } else {
            index += 1;
        }
    }
    index
}

fn raw_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'r') {
        return None;
    }
    let mut quote = start + 1;
    while bytes.get(quote) == Some(&b'#') {
        quote += 1;
    }
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }
    let hashes = quote - start - 1;
    let mut index = quote + 1;
    while index < bytes.len() {
        if bytes[index] == b'"'
            && bytes.get(index + 1..index + 1 + hashes) == Some(&vec![b'#'; hashes][..])
        {
            return Some(index + hashes + 1);
        }
        index += 1;
    }
    Some(bytes.len())
}

fn skip_quoted(bytes: &[u8], start: usize, quote: u8) -> usize {
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = (index + 2).min(bytes.len());
        } else if bytes[index] == quote {
            return index + 1;
        } else {
            index += 1;
        }
    }
    bytes.len()
}

fn char_literal_end(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'\'') {
        return None;
    }
    let content = start + 1;
    if bytes.get(content) == Some(&b'\\') {
        let mut end = content + 2;
        if bytes.get(content + 1) == Some(&b'u') && bytes.get(content + 2) == Some(&b'{') {
            end = content + 3;
            while bytes.get(end).is_some_and(u8::is_ascii_hexdigit) {
                end += 1;
            }
            if bytes.get(end) != Some(&b'}') {
                return None;
            }
            end += 1;
        }
        return (bytes.get(end) == Some(&b'\'')).then_some(end + 1);
    }
    let character = std::str::from_utf8(bytes.get(content..)?)
        .ok()?
        .chars()
        .next()?;
    let end = content + character.len_utf8();
    (bytes.get(end) == Some(&b'\'')).then_some(end + 1)
}

#[test]
fn emitter_scanner_ignores_non_production_mentions_and_classifies_real_calls() {
    let source = r###"
        use crate::network::send_server_data_payload;
        /// send_server_data_payload(&mut client, bytes);
        // client.send_custom_payload(ident!("bong:fake"), bytes);
        const TEXT: &str = "send_custom_payload bong:server_data";
        const RAW: &str = r#"send_server_data_payload"#;
        #[cfg(test)]
        fn test_only(client: &mut Client) {
            send_server_data_payload(client, bytes);
            client.send_custom_payload(ident!("bong:test"), bytes);
        }
        fn production(client: &mut Client) {
            send_server_data_payload(client, bytes);
            client.send_custom_payload(
                ident!("bong:server_data"),
                bytes,
            );
            client.send_custom_payload(ident!("bong:dedicated"), bytes);
        }
    "###;
    let scan = scan_production_emit_source(source);
    assert_eq!(
        scan,
        EmitScan {
            helper_calls: 1,
            direct_calls: 2,
            direct_server_data_calls: 1,
            redis_wanted_calls: 0,
        }
    );
    assert_eq!(classify_emit_api_shape(&scan), "both");
    assert_eq!(classify_wire_transport(&scan), "channel_mixed");
}

#[test]
fn emitter_scanner_derives_redis_and_domain_only_without_path_exceptions() {
    let redis = scan_production_emit_source(
        "redis.tx_outbound.send(RedisOutbound::WantedPlayer(payload));",
    );
    assert_eq!(classify_emit_api_shape(&redis), "redis_only");
    assert_eq!(classify_wire_transport(&redis), "redis_only");

    let domain = scan_production_emit_source("events.send(MeridianSevered { entity });");
    assert_eq!(classify_emit_api_shape(&domain), "no_client_send");
    assert_eq!(classify_wire_transport(&domain), "domain_only");
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
