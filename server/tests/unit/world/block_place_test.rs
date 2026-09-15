use bong_server::craft::WORKBENCH_ITEM_TEMPLATE;
use bong_server::world::block_place::{
    block_item_to_state, placeable_kind_from_str, PlaceableBlockKind,
};
use bong_server::zhenfa::trap_content::TrapTargetFace;
use valence::prelude::BlockState;

#[test]
fn block_item_to_state_maps_all_v1_block_items() {
    let cases = [
        ("earth_crumb", BlockState::DIRT),
        ("hardened_soil", BlockState::COARSE_DIRT),
        ("barren_sand", BlockState::SAND),
        ("weathered_stone", BlockState::GRAVEL),
        ("raw_clay_lump", BlockState::CLAY),
        ("obsidian_shard", BlockState::OBSIDIAN),
        ("torch_item", BlockState::TORCH),
        ("lantern_item", BlockState::LANTERN),
        ("door_bolt", BlockState::IRON_DOOR),
        ("window_grate", BlockState::IRON_BARS),
        ("simple_bed", BlockState::BONG_SIMPLE_BED),
        ("meditation_mat", BlockState::BONG_MEDITATION_MAT),
        ("moisture_base", BlockState::BONG_MOISTURE_BASE),
        ("spirit_stone_rack", BlockState::BONG_SPIRIT_STONE_RACK),
    ];

    for (template_id, expected) in cases {
        assert_eq!(
            block_item_to_state(template_id, TrapTargetFace::Top),
            Some(expected),
            "expected `{template_id}` to map to {expected:?}"
        );
    }
}

#[test]
fn block_item_to_state_rejects_non_placeable_materials() {
    for template_id in ["stone_chunk", "crude_wood", "grass_fiber", "iron_ore"] {
        assert_eq!(
            block_item_to_state(template_id, TrapTargetFace::North),
            None,
            "expected `{template_id}` to stay non-placeable in v1"
        );
    }
}

#[test]
fn block_item_to_state_keeps_workbench_out_of_vanilla_mapping() {
    assert_eq!(
        block_item_to_state(WORKBENCH_ITEM_TEMPLATE, TrapTargetFace::Top),
        None,
        "workbench_item must route through PlaceableBlockKind, not vanilla BlockState mapping"
    );
}

// ─── plan-worldgen-v4 P5 §8.1#5 — vanilla: 直通分支专属矩阵 ───

/// happy path：`vanilla:<known>` 剥前缀后用 BlockKind 解析为默认 BlockState。
/// 锁住 give-block（ItemRegistry vanilla:<id>）→ 放置链路对齐（给得到必放得下）。
#[test]
fn block_item_to_state_resolves_known_vanilla_prefix() {
    use valence::prelude::BlockKind;
    // expected 由 bare 名经 BlockKind::from_str 派生（与实现同源），不硬编码变体名。
    for bare in ["stone", "stone_bricks", "oak_log"] {
        let expected = BlockKind::from_str(bare)
            .unwrap_or_else(|| panic!("{bare} 应是合法 BlockKind"))
            .to_state();
        let template_id = format!("vanilla:{bare}");
        assert_eq!(
            block_item_to_state(&template_id, TrapTargetFace::Top),
            Some(expected),
            "`{template_id}` 应剥前缀后解析为 BlockKind 默认 state {expected:?}"
        );
    }
}

/// 边界：`vanilla:`（空 bare id）剥前缀后是空串，BlockKind::from_str("") → None。
#[test]
fn block_item_to_state_rejects_empty_vanilla_bare_id() {
    assert_eq!(
        block_item_to_state("vanilla:", TrapTargetFace::Top),
        None,
        "空 bare id（vanilla:）无对应 BlockKind，必须拒绝而非 panic 或落 air"
    );
}

/// 错误分支：`vanilla:<unknown>` 剥前缀后是未知块名，BlockKind 解析不出 → None。
#[test]
fn block_item_to_state_rejects_unknown_vanilla_block() {
    assert_eq!(
        block_item_to_state("vanilla:not_a_real_block", TrapTargetFace::North),
        None,
        "未知 vanilla 块名必须拒绝，不得静默落成其他方块"
    );
}

#[test]
fn placeable_kind_from_str_pins_declared_variants() {
    assert_eq!(
        placeable_kind_from_str("workbench"),
        Some(PlaceableBlockKind::Workbench)
    );
    assert_eq!(
        placeable_kind_from_str("storage_crate"),
        Some(PlaceableBlockKind::StorageCrate { is_herb: false })
    );
    assert_eq!(
        placeable_kind_from_str("herb_crate"),
        Some(PlaceableBlockKind::StorageCrate { is_herb: true })
    );
    assert_eq!(
        placeable_kind_from_str("dead_drop"),
        Some(PlaceableBlockKind::DeadDrop)
    );
    assert_eq!(
        placeable_kind_from_str("nonsense"),
        None,
        "unknown placeable values must reject without falling into vanilla placement"
    );
    assert_eq!(
        placeable_kind_from_str("  STORAGE_CRATE  "),
        Some(PlaceableBlockKind::StorageCrate { is_herb: false }),
        "placeable parser should trim and normalize declared TOML markers"
    );
    assert_eq!(
        placeable_kind_from_str("   "),
        None,
        "blank placeable markers must reject instead of routing to a default"
    );
}
