use bong_server::craft::recipe::{
    CraftCategory, CraftRecipe, CraftRequirements, CraftStationKind, RecipeId,
    RecipeValidationError, UnlockSource,
};
use bong_server::cultivation::components::ColorKind;

fn ok_recipe() -> CraftRecipe {
    CraftRecipe {
        id: RecipeId::new("craft.example.test"),
        category: CraftCategory::Misc,
        display_name: "测试配方".into(),
        materials: vec![("herb_a".into(), 2)],
        qi_cost: 5.0,
        time_ticks: 60,
        output: (("test_pill".into()), 1),
        requirements: CraftRequirements::default(),
        unlock_sources: vec![UnlockSource::Scroll {
            item_template: "scroll_test".into(),
        }],
        station: None,
    }
}

#[test]
fn recipe_id_roundtrip_serde() {
    let id = RecipeId::new("dugu.eclipse_needle.iron");
    let s = serde_json::to_string(&id).unwrap();
    assert_eq!(s, "\"dugu.eclipse_needle.iron\"");
    let back: RecipeId = serde_json::from_str(&s).unwrap();
    assert_eq!(back, id);
}

#[test]
fn category_str_stable_and_all_unique() {
    let strs: Vec<&str> = CraftCategory::ALL.iter().map(|c| c.as_str()).collect();
    // 类目必须各不相同 — UI 分组依赖 str id 做 key
    let mut sorted = strs.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        CraftCategory::ALL.len(),
        "expected unique category str ids"
    );
    // 顺序固定（不能因为 enum order 改变破坏 UI 分组顺序）
    assert_eq!(
        strs,
        [
            "anqi_carrier",
            "dugu_potion",
            "tuike_skin",
            "zhenfa_trap",
            "tool",
            "armor_craft",
            "container",
            "poison_powder",
            "misc"
        ]
    );
}

#[test]
fn requirements_default_is_no_gate() {
    let req = CraftRequirements::default();
    assert!(req.realm_min.is_none());
    assert!(req.qi_color_min.is_none());
    assert!(req.skill_lv_min.is_none());
}

#[test]
fn validate_accepts_well_formed_recipe() {
    assert!(ok_recipe().validate().is_ok());
}

#[test]
fn validate_rejects_empty_id() {
    let mut r = ok_recipe();
    r.id = RecipeId::new("");
    assert_eq!(r.validate(), Err(RecipeValidationError::EmptyId));
}

#[test]
fn validate_rejects_no_materials() {
    let mut r = ok_recipe();
    r.materials.clear();
    assert!(matches!(
        r.validate(),
        Err(RecipeValidationError::NoMaterials { .. })
    ));
}

#[test]
fn validate_rejects_empty_material_template() {
    let mut r = ok_recipe();
    r.materials = vec![("".into(), 1)];
    assert!(matches!(
        r.validate(),
        Err(RecipeValidationError::EmptyMaterialTemplate { .. })
    ));
}

#[test]
fn validate_rejects_zero_count_material() {
    let mut r = ok_recipe();
    r.materials = vec![("herb_a".into(), 0)];
    assert!(matches!(
        r.validate(),
        Err(RecipeValidationError::ZeroCount { .. })
    ));
}

#[test]
fn validate_rejects_duplicate_material_template() {
    let mut r = ok_recipe();
    r.materials = vec![("herb_a".into(), 1), ("herb_a".into(), 2)];
    assert!(matches!(
        r.validate(),
        Err(RecipeValidationError::DuplicateMaterialTemplate { ref template, .. })
            if template == "herb_a"
    ));
}

#[test]
fn validate_rejects_empty_output_template() {
    let mut r = ok_recipe();
    r.output = ("".into(), 1);
    assert!(matches!(
        r.validate(),
        Err(RecipeValidationError::EmptyOutputTemplate { .. })
    ));
}

#[test]
fn validate_rejects_zero_output_count() {
    let mut r = ok_recipe();
    r.output = ("test_pill".into(), 0);
    assert!(matches!(
        r.validate(),
        Err(RecipeValidationError::ZeroOutputCount { .. })
    ));
}

#[test]
fn validate_rejects_negative_qi_cost() {
    let mut r = ok_recipe();
    r.qi_cost = -1.0;
    assert!(matches!(
        r.validate(),
        Err(RecipeValidationError::InvalidQiCost { .. })
    ));
}

#[test]
fn validate_rejects_nan_qi_cost() {
    let mut r = ok_recipe();
    r.qi_cost = f64::NAN;
    assert!(matches!(
        r.validate(),
        Err(RecipeValidationError::InvalidQiCost { .. })
    ));
}

#[test]
fn validate_rejects_zero_time_ticks() {
    let mut r = ok_recipe();
    r.time_ticks = 0;
    assert!(matches!(
        r.validate(),
        Err(RecipeValidationError::ZeroTimeTicks { .. })
    ));
}

#[test]
fn validate_accepts_empty_unlock_sources_for_material_discovery() {
    // 空 unlock_sources 合法 = 材料发现路径（持有任一原料才解锁），
    // 而非旧语义的"默认全解锁"。
    let mut r = ok_recipe();
    r.unlock_sources.clear();
    assert!(r.validate().is_ok());
}

#[test]
fn validate_accepts_zero_qi_cost() {
    // qi_cost = 0 是合法的（凡器手搓，无真元投入）
    let mut r = ok_recipe();
    r.qi_cost = 0.0;
    assert!(r.validate().is_ok());
}

#[test]
fn validate_rejects_skill_requirement_above_runtime_maximum() {
    let mut recipe = ok_recipe();
    recipe.requirements.skill_lv_min = Some(bong_server::skill::curve::SKILL_MAX_LEVEL + 1);
    assert!(matches!(
        recipe.validate(),
        Err(RecipeValidationError::SkillLevelTooHigh { skill_lv_min, .. })
            if skill_lv_min == bong_server::skill::curve::SKILL_MAX_LEVEL + 1
    ));
}

#[test]
fn validate_accepts_skill_requirement_at_runtime_maximum() {
    let mut recipe = ok_recipe();
    recipe.requirements.skill_lv_min = Some(bong_server::skill::curve::SKILL_MAX_LEVEL);
    assert!(recipe.validate().is_ok());
}

#[test]
fn validate_rejects_qi_color_min_share_above_one() {
    let mut r = ok_recipe();
    r.requirements.qi_color_min = Some((ColorKind::Insidious, 1.5));
    assert!(matches!(
        r.validate(),
        Err(RecipeValidationError::InvalidQiColorMinShare { share, .. }) if (share - 1.5).abs() < 1e-6
    ));
}

#[test]
fn validate_rejects_qi_color_min_share_negative() {
    let mut r = ok_recipe();
    r.requirements.qi_color_min = Some((ColorKind::Insidious, -0.1));
    assert!(matches!(
        r.validate(),
        Err(RecipeValidationError::InvalidQiColorMinShare { share, .. }) if share < 0.0
    ));
}

#[test]
fn validate_rejects_qi_color_min_share_nan() {
    let mut r = ok_recipe();
    r.requirements.qi_color_min = Some((ColorKind::Insidious, f32::NAN));
    assert!(matches!(
        r.validate(),
        Err(RecipeValidationError::InvalidQiColorMinShare { share, .. }) if share.is_nan()
    ));
}

#[test]
fn validate_accepts_qi_color_min_share_at_bounds() {
    let mut r = ok_recipe();
    // 0.0 边界
    r.requirements.qi_color_min = Some((ColorKind::Insidious, 0.0));
    assert!(r.validate().is_ok());
    // 1.0 边界
    r.requirements.qi_color_min = Some((ColorKind::Insidious, 1.0));
    assert!(r.validate().is_ok());
}

#[test]
fn validate_rejects_empty_scroll_unlock_payload() {
    let mut r = ok_recipe();
    r.unlock_sources = vec![UnlockSource::Scroll {
        item_template: String::new(),
    }];
    assert!(matches!(
        r.validate(),
        Err(RecipeValidationError::EmptyUnlockSourceTemplate { kind: "scroll", .. })
    ));
}

#[test]
fn validate_rejects_empty_mentor_unlock_payload() {
    let mut r = ok_recipe();
    r.unlock_sources = vec![UnlockSource::Mentor {
        npc_archetype: String::new(),
    }];
    assert!(matches!(
        r.validate(),
        Err(RecipeValidationError::EmptyUnlockSourceTemplate { kind: "mentor", .. })
    ));
}

// ============= CraftStationKind pin tests =============

#[test]
fn craft_station_kind_workbench_str_stable() {
    assert_eq!(
        CraftStationKind::Workbench.as_str(),
        "workbench",
        "CraftStationKind::Workbench str repr must be 'workbench' for IPC/agent contract"
    );
}

#[test]
fn craft_station_kind_serde_roundtrip() {
    let kind = CraftStationKind::Workbench;
    let json = serde_json::to_string(&kind).unwrap();
    assert_eq!(json, "\"workbench\"");
    let back: CraftStationKind = serde_json::from_str(&json).unwrap();
    assert_eq!(back, kind);
}

#[test]
fn craft_recipe_station_none_means_handcraft() {
    let r = ok_recipe();
    assert_eq!(
        r.station, None,
        "default ok_recipe must have station=None (handcraft)"
    );
}

#[test]
fn craft_recipe_station_workbench_accepted() {
    let mut r = ok_recipe();
    r.station = Some(CraftStationKind::Workbench);
    assert!(
        r.validate().is_ok(),
        "station=Workbench must pass validation"
    );
}
