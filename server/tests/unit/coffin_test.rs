use bong_server::coffin::*;
use valence::prelude::{BlockPos, Entity};

#[test]
fn registry_set_marker_entity_on_missing_coffin_returns_false() {
    let mut registry = CoffinRegistry::default();
    assert!(
        !registry.set_marker_entity(BlockPos::new(0, 0, 0), Some(Entity::from_raw(1))),
        "对不存在的棺设 marker 应返回 false（不静默造记录）"
    );
}

#[test]
fn registry_iter_unique_yields_one_record_per_coffin() {
    let mut registry = CoffinRegistry::default();
    registry.insert(BlockPos::new(0, 64, 0), 0, CoffinGrade::Mundane);
    registry.insert(BlockPos::new(10, 64, 10), 0, CoffinGrade::Jade);
    let lowers: std::collections::HashSet<BlockPos> =
        registry.iter_unique().map(|c| c.lower).collect();
    assert_eq!(
        lowers.len(),
        2,
        "两张棺占 4 条索引，iter_unique 应去重到 2 条 lower 记录，实得 {lowers:?}"
    );
}

#[test]
fn registry_tracks_occupancy_by_player() {
    let mut registry = CoffinRegistry::default();
    let lower = BlockPos::new(8, 64, 8);
    let player = Entity::from_raw(7);

    assert!(registry.insert(lower, 10, CoffinGrade::Mundane));
    assert!(registry.set_occupied(lower, player));
    assert!(!registry.set_occupied(lower, Entity::from_raw(8)));
    assert_eq!(registry.player_in_coffin.get(&player), Some(&lower));
    assert_eq!(registry.lookup(lower).unwrap().occupied_by, Some(player));

    assert_eq!(registry.clear_player(player), Some(lower));
    assert!(!registry.player_in_coffin.contains_key(&player));
    assert_eq!(registry.lookup(lower).unwrap().occupied_by, None);
}

#[test]
fn registry_reclaim_replaces_stale_occupant() {
    let mut registry = CoffinRegistry::default();
    let lower = BlockPos::new(8, 64, 8);
    let stale = Entity::from_raw(7);
    let current = Entity::from_raw(8);

    assert!(registry.insert(lower, 10, CoffinGrade::Mundane));
    assert!(registry.set_occupied(lower, stale));

    registry.reclaim_occupied(lower, current, 20, CoffinGrade::Mundane);

    assert!(!registry.player_in_coffin.contains_key(&stale));
    assert_eq!(registry.player_in_coffin.get(&current), Some(&lower));
    assert_eq!(registry.lookup(lower).unwrap().occupied_by, Some(current));
}

#[test]
fn coffin_grade_lifespan_factors_match_plan_values() {
    // 四档寿元倍率（plan-coffin-tiers-v1 P0 设计值）
    assert!(
        (CoffinGrade::Mundane.lifespan_factor() - 0.9).abs() < f64::EPSILON,
        "Mundane factor should be 0.9, got {}",
        CoffinGrade::Mundane.lifespan_factor()
    );
    assert!(
        (CoffinGrade::Jade.lifespan_factor() - 0.7).abs() < f64::EPSILON,
        "Jade factor should be 0.7, got {}",
        CoffinGrade::Jade.lifespan_factor()
    );
    assert!(
        (CoffinGrade::Stone.lifespan_factor() - 0.5).abs() < f64::EPSILON,
        "Stone factor should be 0.5, got {}",
        CoffinGrade::Stone.lifespan_factor()
    );
    assert!(
        (CoffinGrade::Bronze.lifespan_factor() - 0.3).abs() < f64::EPSILON,
        "Bronze factor should be 0.3, got {}",
        CoffinGrade::Bronze.lifespan_factor()
    );
}

#[test]
fn coffin_lifespan_multiplier_none_is_1() {
    assert!(
        (coffin_lifespan_multiplier(None) - 1.0).abs() < f64::EPSILON,
        "None grade (not in coffin) should return 1.0, got {}",
        coffin_lifespan_multiplier(None)
    );
}

#[test]
fn coffin_lifespan_multiplier_some_delegates_to_grade() {
    assert!((coffin_lifespan_multiplier(Some(CoffinGrade::Mundane)) - 0.9).abs() < f64::EPSILON);
    assert!((coffin_lifespan_multiplier(Some(CoffinGrade::Jade)) - 0.7).abs() < f64::EPSILON);
    assert!((coffin_lifespan_multiplier(Some(CoffinGrade::Stone)) - 0.5).abs() < f64::EPSILON);
    assert!((coffin_lifespan_multiplier(Some(CoffinGrade::Bronze)) - 0.3).abs() < f64::EPSILON);
}

#[test]
fn coffin_grade_item_id_roundtrip() {
    for grade in [
        CoffinGrade::Mundane,
        CoffinGrade::Jade,
        CoffinGrade::Stone,
        CoffinGrade::Bronze,
    ] {
        let id = grade.item_id();
        let back = CoffinGrade::from_item_id(id);
        assert_eq!(
            back,
            Some(grade),
            "item_id({grade:?}) = {id} should round-trip via from_item_id"
        );
    }
}

#[test]
fn coffin_grade_from_item_id_rejects_unknown() {
    assert_eq!(
        CoffinGrade::from_item_id("unknown_coffin"),
        None,
        "unknown item id should return None"
    );
    assert_eq!(
        CoffinGrade::from_item_id(""),
        None,
        "empty string should return None"
    );
}

#[test]
fn coffin_grade_default_is_mundane() {
    assert_eq!(CoffinGrade::default(), CoffinGrade::Mundane);
}

/// legacy constant 保持不变，下游断言不破坏

#[test]
fn coffin_lifespan_factor_legacy_constant_unchanged() {
    assert_eq!(
        COFFIN_LIFESPAN_FACTOR, 0.9,
        "COFFIN_LIFESPAN_FACTOR legacy constant must remain 0.9 for backward compat"
    );
}

#[test]
fn coffin_grade_registry_preserves_grade_on_insert() {
    let mut registry = CoffinRegistry::default();
    let lower = BlockPos::new(0, 64, 0);
    registry.insert(lower, 0, CoffinGrade::Jade);
    let stored = registry.lookup(lower).expect("coffin should be registered");
    assert_eq!(
        stored.grade,
        CoffinGrade::Jade,
        "registry should preserve grade=Jade after insert"
    );
}

#[test]
fn coffin_grade_db_str_roundtrip() {
    for grade in [
        CoffinGrade::Mundane,
        CoffinGrade::Jade,
        CoffinGrade::Stone,
        CoffinGrade::Bronze,
    ] {
        let s = grade.as_db_str();
        let back = CoffinGrade::from_db_str(s);
        assert_eq!(
            back, grade,
            "db_str({grade:?}) = {s} should round-trip via from_db_str"
        );
    }
}

#[test]
fn coffin_grade_from_db_str_unknown_defaults_to_mundane() {
    assert_eq!(CoffinGrade::from_db_str(""), CoffinGrade::Mundane);
    assert_eq!(CoffinGrade::from_db_str("invalid"), CoffinGrade::Mundane);
}

#[test]
fn p4_spirit_quality_conservation_trivial() {
    // coffin item spirit_quality_initial = 0.0 → output_sq = 0.0 → 守恒律不触发断言。
    // 本 test 作为"已确认不触发"的 pin：若未来 coffin 改为有灵质产出，要重新核算。
    let item_registry =
        bong_server::inventory::load_item_registry().expect("item registry should load");
    for grade in [CoffinGrade::Jade, CoffinGrade::Stone, CoffinGrade::Bronze] {
        let output_id = grade.item_id();
        let output_sq = item_registry
            .get(output_id)
            .map(|t| t.spirit_quality_initial)
            .unwrap_or_else(|| panic!("item `{output_id}` must be in registry"));
        assert_eq!(
            output_sq, 0.0,
            "coffin item `{output_id}` spirit_quality_initial 预期为 0.0（棺材器物无灵质产出），\
             若改为有灵质须重算守恒"
        );
    }
}

/// P4 §材料模板：yu_sui / wu_yao / gu_tong_pian 都能从 ItemRegistry 查到，
/// spirit_quality_initial 按 plan §8.1 #2。

#[test]
fn p4_new_material_templates_in_registry() {
    let item_registry =
        bong_server::inventory::load_item_registry().expect("item registry should load");
    let checks = [("yu_sui", 0.8f64), ("wu_yao", 0.85), ("gu_tong_pian", 0.6)];
    for (id, expected_sq) in checks {
        let tmpl = item_registry
            .get(id)
            .unwrap_or_else(|| panic!("item `{id}` not found in registry（P4 新材料）"));
        assert!(
            (tmpl.spirit_quality_initial - expected_sq).abs() < 1e-9,
            "item `{id}` spirit_quality_initial: expect {expected_sq}, got {}",
            tmpl.spirit_quality_initial
        );
    }
}

/// P4 §卷轴模板：3 张配方卷轴在 ItemRegistry 中存在。

#[test]
fn p4_scroll_templates_in_registry() {
    let item_registry =
        bong_server::inventory::load_item_registry().expect("item registry should load");
    for scroll_id in [
        "scroll_jade_coffin",
        "scroll_stone_coffin",
        "scroll_bronze_coffin",
    ] {
        assert!(
            item_registry.get(scroll_id).is_some(),
            "scroll item `{scroll_id}` not found in registry（P4 配方卷轴）"
        );
    }
}

/// P4 §set_grade：CoffinRegistry.set_grade 正确更新档级（双索引）。

#[test]
fn p4_registry_set_grade_on_missing_returns_false() {
    let mut registry = CoffinRegistry::default();
    assert!(
        !registry.set_grade(BlockPos::new(99, 64, 99), CoffinGrade::Jade),
        "对不存在的棺 set_grade 应返回 false"
    );
}
