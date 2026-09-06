use super::*;

// apply_inventory_move 的私有 fixture 仍被本文件其余测试复用；外置测试保留
// 独立副本，因为集成测试不能访问本模块的私有测试 helper。
fn make_test_inventory_with_one_item() -> PlayerInventory {
    let item = ItemInstance {
        instance_id: 42,
        template_id: "rat_tail".to_string(),
        display_name: "噬元鼠尾".to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.2,
        rarity: ItemRarity::Common,
        description: String::new(),
        stack_count: 1,
        spirit_quality: 1.0,
        durability: 1.0,
        freshness: None,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    };
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(7),
        containers: vec![
            ContainerState {
                quick_access: false,
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item,
                }],
                owner_instance_id: None,
            },
            ContainerState {
                quick_access: false,
                id: SMALL_POUCH_CONTAINER_ID.to_string(),
                name: "小口袋".to_string(),
                rows: 3,
                cols: 3,
                items: Vec::new(),
                owner_instance_id: None,
            },
            ContainerState {
                quick_access: false,
                id: FRONT_SATCHEL_CONTAINER_ID.to_string(),
                name: "前挂包".to_string(),
                rows: 3,
                cols: 4,
                items: Vec::new(),
                owner_instance_id: None,
            },
        ],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 50.0,
    }
}

const BLOCK_ITEM_TEMPLATE_IDS: [&str; 14] = [
    "earth_crumb",
    "hardened_soil",
    "barren_sand",
    "weathered_stone",
    "raw_clay_lump",
    "obsidian_shard",
    "torch_item",
    "lantern_item",
    "door_bolt",
    "window_grate",
    "simple_bed",
    "meditation_mat",
    "moisture_base",
    "spirit_stone_rack",
];

fn test_registry_from_strs(entries: &[(&str, &str)]) -> Result<ItemRegistry, String> {
    let mut templates = HashMap::new();
    for (template_id, display_name) in entries {
        templates.insert(
            (*template_id).to_string(),
            ItemTemplate {
                id: (*template_id).to_string(),
                display_name: (*display_name).to_string(),
                category: ItemCategory::Misc,
                placeable: None,
                max_stack_count: 1,
                grid_w: 1,
                grid_h: 1,
                base_weight: 0.1,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 1.0,
                description: "test template".to_string(),
                effect: None,
                cast_duration_ms: DEFAULT_CAST_DURATION_MS,
                cooldown_ms: DEFAULT_COOLDOWN_MS,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                readable_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shield_spec: None,

                shelflife_profile: None,
                shelflife_track: None,
                wearer_race: crate::body_plan::types::RaceGateOwned::default(),
            },
        );
    }
    Ok(ItemRegistry { templates })
}

fn test_template(
    template_id: &str,
    category: ItemCategory,
    grid_w: u8,
    grid_h: u8,
    max_stack_count: u32,
) -> ItemTemplate {
    ItemTemplate {
        id: template_id.to_string(),
        display_name: template_id.to_string(),
        category,
        placeable: None,
        max_stack_count,
        grid_w,
        grid_h,
        base_weight: 0.1,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 1.0,
        description: "test template".to_string(),
        effect: None,
        cast_duration_ms: DEFAULT_CAST_DURATION_MS,
        cooldown_ms: DEFAULT_COOLDOWN_MS,
        weapon_spec: None,
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
        readable_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: None,
        shield_spec: None,

        shelflife_profile: None,
        shelflife_track: None,
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    }
}

fn raw_item_template_toml(id: &str, category: &str) -> ItemTemplateToml {
    ItemTemplateToml {
        id: id.to_string(),
        placeable: None,
        name: id.to_string(),
        category: category.to_string(),
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.1,
        rarity: "common".to_string(),
        spirit_quality_initial: 0.0,
        description: "test item".to_string(),
        max_stack_count: None,
        effect: None,
        cast_duration_ms: None,
        cooldown_ms: None,
        weapon: None,
        forge_station: None,
        blueprint_scroll: None,
        inscription_scroll: None,
        technique_scroll: None,
        readable_scroll: None,
        recipe_fragment: None,
        container: None,
        shield_spec: None,
        shelflife_profile: None,
        shelflife_track: None,
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    }
}

fn registry_from_templates(templates: Vec<ItemTemplate>) -> ItemRegistry {
    ItemRegistry {
        templates: templates
            .into_iter()
            .map(|template| (template.id.clone(), template))
            .collect(),
    }
}

#[test]
fn parse_item_effect_accepts_poison_pill_target() {
    let effect = parse_item_effect(
        ItemEffectToml {
            kind: "poison_pill".to_string(),
            magnitude: 0.0,
            target: Some("poison_pill_qing_lin_man_tuo".to_string()),
            duration_ticks: None,
        },
        Path::new("<inline-items.toml>"),
        "poison_pill_qing_lin_man_tuo",
    )
    .expect("poison_pill effect should parse");

    assert_eq!(
        effect,
        ItemEffect::PoisonPill {
            pill_item_id: "poison_pill_qing_lin_man_tuo".to_string()
        }
    );
}

#[test]
fn parse_item_effect_rejects_poison_pill_missing_or_empty_target() {
    for target in [None, Some("   ".to_string())] {
        let error = parse_item_effect(
            ItemEffectToml {
                kind: "poison_pill".to_string(),
                magnitude: 0.0,
                target,
                duration_ticks: None,
            },
            Path::new("<inline-items.toml>"),
            "poison_pill_missing_target",
        )
        .expect_err("poison_pill effect without target should fail");

        assert!(
            error.contains("item.effect.target"),
            "expected target validation error, got {error}"
        );
    }
}

#[test]
fn parse_item_effect_rejects_poison_pill_unknown_target() {
    let error = parse_item_effect(
        ItemEffectToml {
            kind: "poison_pill".to_string(),
            magnitude: 0.0,
            target: Some("poison_pill_typo".to_string()),
            duration_ticks: None,
        },
        Path::new("<inline-items.toml>"),
        "poison_pill_unknown_target",
    )
    .expect_err("poison_pill effect should reject unknown target ids");

    assert!(
        error.contains("unknown poison pill target `poison_pill_typo`"),
        "expected poison pill target validation error, got {error}"
    );
}

#[test]
fn parse_item_effect_accepts_wound_heal_missing_target_as_all_wounds() {
    let effect = parse_item_effect(
        ItemEffectToml {
            kind: "wound_heal".to_string(),
            magnitude: 1.0,
            target: None,
            duration_ticks: None,
        },
        Path::new("<inline-items.toml>"),
        "bandage",
    )
    .expect("wound_heal without target should parse as all wounds");

    assert_eq!(
        effect,
        ItemEffect::WoundHeal {
            magnitude: 1.0,
            target: None
        }
    );
}

#[test]
fn parse_item_effect_rejects_wound_heal_blank_target() {
    let error = parse_item_effect(
        ItemEffectToml {
            kind: "wound_heal".to_string(),
            magnitude: 1.0,
            target: Some("   ".to_string()),
            duration_ticks: None,
        },
        Path::new("<inline-items.toml>"),
        "blank_bandage",
    )
    .expect_err("blank wound_heal target should be rejected instead of healing all wounds");

    assert!(
        error.contains("empty target segment"),
        "expected empty wound_heal target validation error, got {error}"
    );
}

#[test]
fn parse_item_effect_rejects_wound_heal_unknown_target() {
    let error = parse_item_effect(
        ItemEffectToml {
            kind: "wound_heal".to_string(),
            magnitude: 1.0,
            target: Some("arm_l/tail".to_string()),
            duration_ticks: None,
        },
        Path::new("<inline-items.toml>"),
        "tail_splint",
    )
    .expect_err("unknown wound_heal body part should be rejected");

    assert!(
        error.contains("unknown target `tail`"),
        "expected unknown wound_heal target validation error, got {error}"
    );
}

#[test]
fn item_effect_new_consumable_variants_serde_roundtrip() {
    for original in [
        ItemEffect::ComposureRestore { magnitude: 0.35 },
        ItemEffect::WoundHeal {
            magnitude: 1.0,
            target: None,
        },
        ItemEffect::WoundHeal {
            magnitude: 2.0,
            target: Some("arm_l/arm_r".to_string()),
        },
    ] {
        let json = serde_json::to_string(&original).expect("new item effect should serialize");
        let parsed: ItemEffect =
            serde_json::from_str(&json).expect("new item effect should deserialize");
        assert_eq!(
            parsed, original,
            "expected serde roundtrip to preserve new consumable effect, json={json}"
        );
    }
}

#[test]
fn item_effect_new_consumable_variants_reject_invalid_json_shape() {
    for json in [
        r#"{"ComposureRestore":{"amount":0.35}}"#,
        r#"{"WoundHeal":{"magnitude":1.0,"target":5}}"#,
        r#"{"WoundHeal":{"target":"arm_l"}}"#,
    ] {
        let error = serde_json::from_str::<ItemEffect>(json)
            .expect_err("invalid new item effect JSON should fail");
        assert!(
            !error.to_string().is_empty(),
            "expected serde error for invalid new item effect JSON, json={json}"
        );
    }
}

fn empty_inventory(rows: u8, cols: u8) -> PlayerInventory {
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(0),
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows,
            cols,
            items: Vec::new(),
            owner_instance_id: None,
        }],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 99.0,
    }
}

fn clear_inventory_fixture() -> (ItemRegistry, PlayerInventory, u64) {
    let registry = load_item_registry().expect("real item registry should load");
    let loadout = load_default_loadout(&registry).expect("default loadout should load");
    let mut allocator = InventoryInstanceIdAllocator::default();
    let mut inventory = instantiate_inventory_from_loadout(&loadout, &mut allocator, &registry)
        .expect("default inventory should instantiate");
    let pack_instance_id = inventory
        .equipped
        .get(EQUIP_SLOT_CHEST)
        .and_then(|slot| {
            slot.worn
                .iter()
                .find(|item| item.template_id == "worn_grass_pouch")
        })
        .expect("starter worn_grass_pouch must exist")
        .instance_id;
    inventory
        .containers
        .iter_mut()
        .find(|container| container.id == BODY_POCKET_CONTAINER_ID)
        .expect("body_pocket must exist")
        .items
        .push(PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(90_001, "body_sentinel"),
        });
    inventory.hotbar[8] = Some(make_test_item_instance(90_002, "hotbar_sentinel"));
    inventory.containers.push(ContainerState {
        id: MAIN_PACK_CONTAINER_ID.to_string(),
        name: "legacy main pack".to_string(),
        rows: 1,
        cols: 1,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(90_003, "legacy_sentinel"),
        }],
        owner_instance_id: None,
        quick_access: false,
    });
    (registry, inventory, pack_instance_id)
}

#[test]
fn clear_player_inventory_pack_only_clears_dynamic_pack_and_legacy_main_pack() {
    let (registry, mut inventory, pack_instance_id) = clear_inventory_fixture();
    let previous_revision = inventory.revision;
    let pack_container_id = container_id_for_worn_pack(pack_instance_id);
    assert!(
        inventory
            .containers
            .iter()
            .find(|container| container.id == pack_container_id)
            .is_some_and(|container| !container.items.is_empty()),
        "fixture dynamic pack must start non-empty"
    );

    clear_player_inventory(&mut inventory, ClearScope::PackOnly, &registry);

    assert!(inventory
        .containers
        .iter()
        .filter(|container| {
            container.id == MAIN_PACK_CONTAINER_ID
                || worn_pack_instance_from_container_id(&container.id).is_some()
        })
        .all(|container| container.items.is_empty()));
    assert!(
        inventory
            .containers
            .iter()
            .find(|container| container.id == BODY_POCKET_CONTAINER_ID)
            .expect("body_pocket must remain")
            .items
            .iter()
            .any(|placed| placed.instance.instance_id == 90_001),
        "pack-only clear must preserve body pocket sentinel instance=90001"
    );
    assert_eq!(
        inventory.hotbar[8].as_ref().map(|item| item.instance_id),
        Some(90_002),
        "pack-only clear must preserve hotbar"
    );
    let dynamic_pack = inventory
        .containers
        .iter()
        .find(|container| container.id == pack_container_id)
        .expect("worn pack dynamic container must remain");
    assert_eq!(dynamic_pack.owner_instance_id, Some(pack_instance_id));
    assert_eq!(inventory.revision.0, previous_revision.0 + 1);
}

#[test]
fn clear_player_inventory_pack_and_hotbar_preserves_pack_topology_and_capacity() {
    let (registry, mut inventory, pack_instance_id) = clear_inventory_fixture();
    let previous_revision = inventory.revision;
    let pack_container_id = container_id_for_worn_pack(pack_instance_id);

    clear_player_inventory(&mut inventory, ClearScope::PackAndHotbar, &registry);

    assert!(inventory
        .containers
        .iter()
        .all(|container| container.items.is_empty()));
    assert!(inventory.hotbar.iter().all(Option::is_none));
    assert!(
        inventory
            .equipped
            .get(EQUIP_SLOT_CHEST)
            .is_some_and(|slot| slot
                .worn
                .iter()
                .any(|item| item.instance_id == pack_instance_id)),
        "pack-and-hotbar clear must preserve worn pack equipment"
    );
    let dynamic_pack = inventory
        .containers
        .iter()
        .find(|container| container.id == pack_container_id)
        .expect("worn pack dynamic container must remain");
    assert_eq!(dynamic_pack.owner_instance_id, Some(pack_instance_id));
    assert!((inventory.max_weight - 23.0).abs() < f64::EPSILON);
    assert_eq!(inventory.revision.0, previous_revision.0 + 1);
}

#[test]
fn clear_player_inventory_all_removes_pack_topology_and_restores_base_capacity() {
    let (registry, mut inventory, _) = clear_inventory_fixture();
    let previous_revision = inventory.revision;

    clear_player_inventory(&mut inventory, ClearScope::All, &registry);

    assert!(inventory
        .containers
        .iter()
        .all(|container| container.items.is_empty()));
    assert!(inventory.hotbar.iter().all(Option::is_none));
    assert!(inventory.equipped.is_empty());
    assert!(
        inventory
            .containers
            .iter()
            .all(|container| worn_pack_instance_from_container_id(&container.id).is_none()),
        "all clear must remove orphan dynamic pack containers"
    );
    assert!(
        inventory
            .containers
            .iter()
            .any(|container| container.id == BODY_POCKET_CONTAINER_ID),
        "all clear must retain the body_pocket topology"
    );
    assert!((inventory.max_weight - BASE_CARRY_CAPACITY).abs() < f64::EPSILON);
    assert_eq!(inventory.revision.0, previous_revision.0 + 1);
}

// plan-tarkov-backpack-v1 P5 — 背包平衡数值标定 sanity（固化 core.toml 解析正确）。
// 锁住起手破草包 / 升级小草包的 container_spec + 自重，任何误改数值立即撞红：
//   · 破草包(worn_grass_pouch)：3×3=9 格，容量 8.0（与 loadout BASE 15+8=23 自洽），自重 0.25。
//   · 小草包(grass_pouch)：3×3=9 格，容量 10.0（>破草包，差异化升级款），自重 0.3。
#[test]
fn grass_pouch_balance_values_parse_from_core_toml() {
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");

    let worn = registry
        .get("worn_grass_pouch")
        .expect("破草包 worn_grass_pouch 必须注册");
    assert!(
        (worn.base_weight - 0.25).abs() < f64::EPSILON,
        "破草包自重应为 0.25（最轻起手款），实际 {}",
        worn.base_weight
    );
    let worn_spec = worn
        .container_spec
        .as_ref()
        .expect("破草包必须有 container_spec");
    assert_eq!(
        (worn_spec.rows, worn_spec.cols),
        (3, 3),
        "破草包应 3×3 grid"
    );
    assert!(
        (worn_spec.weight_capacity - 8.0).abs() < f64::EPSILON,
        "破草包容量应为 8.0（与 loadout BASE 15+8=23 自洽），实际 {}",
        worn_spec.weight_capacity
    );
    assert_eq!(
        worn_spec.equip_slot, EQUIP_SLOT_CHEST,
        "破草包穿 chest 身体槽"
    );

    let pouch = registry
        .get("grass_pouch")
        .expect("小草包 grass_pouch 必须注册");
    let pouch_spec = pouch
        .container_spec
        .as_ref()
        .expect("小草包必须有 container_spec");
    assert!(
        pouch_spec.weight_capacity > worn_spec.weight_capacity,
        "小草包是升级款，容量({})必须大于破草包({})",
        pouch_spec.weight_capacity,
        worn_spec.weight_capacity
    );
    assert!(
        (pouch_spec.weight_capacity - 10.0).abs() < f64::EPSILON,
        "小草包容量应标定为 10.0，实际 {}",
        pouch_spec.weight_capacity
    );

    // 起手 loadout max_weight 必须与 BASE + 破草包容量自洽（防止数值漂移破坏起手负重）。
    let loadout = load_default_loadout(&registry).expect("default loadout 应能加载");
    assert!(
        (loadout.max_weight - (BASE_CARRY_CAPACITY + worn_spec.weight_capacity)).abs()
            < f64::EPSILON,
        "loadout max_weight({}) 应等于 BASE({}) + 破草包容量({})",
        loadout.max_weight,
        BASE_CARRY_CAPACITY,
        worn_spec.weight_capacity
    );
}

#[test]
fn loads_item_registry_from_assets() {
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");
    assert!(registry.len() >= 1);
    assert!(registry.get("starter_talisman").is_some());
    assert!(registry.get("xujie_canxie").is_some());
    assert!(matches!(
        registry.get("life_extension_pill").and_then(|item| item.effect.as_ref()),
        Some(ItemEffect::LifespanExtension {
            years: 10,
            source,
        }) if source == "life_extension_pill"
    ));
    assert!(matches!(
        registry.get("huiyuan_pill").and_then(|item| item.effect.as_ref()),
        Some(ItemEffect::QiRecovery { amount }) if (*amount - 60.0).abs() < f64::EPSILON
    ));
    assert!(matches!(
        registry
            .get("huiyuan_decoction")
            .and_then(|item| item.effect.as_ref()),
        Some(ItemEffect::QiRecovery { amount }) if (*amount - 40.0).abs() < f64::EPSILON
    ));
    assert!(matches!(
        registry
            .get("meridian_salve")
            .and_then(|item| item.effect.as_ref()),
        Some(ItemEffect::MeridianHeal { magnitude, target })
            if (*magnitude - 0.2).abs() < f64::EPSILON && target == "any_meridian"
    ));
    assert!(matches!(
        registry
            .get("meridian_rubbing")
            .and_then(|item| item.effect.as_ref()),
        Some(ItemEffect::MeridianHeal { magnitude, target })
            if (*magnitude - 0.15).abs() < f64::EPSILON && target == "any_meridian"
    ));
    assert!(matches!(
        registry
            .get("qingzhuo_powder")
            .and_then(|item| item.effect.as_ref()),
        Some(ItemEffect::ContaminationCleanse { magnitude })
            if (*magnitude - 0.4).abs() < f64::EPSILON
    ));
    assert!(matches!(
        registry
            .get("anti_gu_powder")
            .and_then(|item| item.effect.as_ref()),
        Some(ItemEffect::ContaminationCleanse { magnitude })
            if (*magnitude - 0.4).abs() < f64::EPSILON
    ));
    assert!(matches!(
        registry
            .get("qi_guide_talisman")
            .and_then(|item| item.effect.as_ref()),
        Some(ItemEffect::FoodRegen {
            bonus_factor,
            duration_ticks,
        }) if (*bonus_factor - 0.30).abs() < f32::EPSILON && *duration_ticks == 36_000
    ));
    assert!(matches!(
        registry
            .get("calming_tea")
            .and_then(|item| item.effect.as_ref()),
        Some(ItemEffect::ComposureRestore { magnitude })
            if (*magnitude - 0.35).abs() < f64::EPSILON
    ));
    assert!(matches!(
        registry.get("bandage").and_then(|item| item.effect.as_ref()),
        Some(ItemEffect::WoundHeal { magnitude, target })
            if (*magnitude - 1.0).abs() < f64::EPSILON && target.is_none()
    ));
    assert!(matches!(
        registry
            .get("arm_splint")
            .and_then(|item| item.effect.as_ref()),
        Some(ItemEffect::WoundHeal {
            magnitude,
            target: Some(target),
        }) if (*magnitude - 2.0).abs() < f64::EPSILON && target == "arm_l/arm_r"
    ));
    assert!(matches!(
        registry
            .get("leg_splint")
            .and_then(|item| item.effect.as_ref()),
        Some(ItemEffect::WoundHeal {
            magnitude,
            target: Some(target),
        }) if (*magnitude - 2.0).abs() < f64::EPSILON && target == "leg_l/leg_r"
    ));
    assert!(matches!(
        registry.get("life_core").and_then(|item| item.effect.as_ref()),
        Some(ItemEffect::LifespanExtension {
            years: 25,
            source,
        }) if source == "collapse_core"
    ));
    assert!(matches!(
        registry
            .get("anti_spirit_pressure_pill")
            .and_then(|item| item.effect.as_ref()),
        Some(ItemEffect::AntiSpiritPressure { duration_ticks }) if *duration_ticks == 36_000
    ));
    assert!(matches!(
        registry.get("spirit_treasure_jizhaojing"),
        Some(ItemTemplate {
            category: ItemCategory::Treasure,
            placeable: None,
            rarity: ItemRarity::Ancient,
            max_stack_count: 1,
            ..
        })
    ));
    assert!(matches!(
        registry
            .get("ling_iron_anvil")
            .and_then(|item| item.forge_station_spec.as_ref()),
        Some(ForgeStationSpec { tier: 2 })
    ));
    assert!(matches!(
        registry
            .get("blueprint_scroll_ling_feng")
            .and_then(|item| item.blueprint_scroll_spec.as_ref()),
        Some(BlueprintScrollSpec { blueprint_id }) if blueprint_id == "ling_feng_v0"
    ));
    assert!(matches!(
        registry
            .get("inscription_scroll_qi_amplify_v0")
            .and_then(|item| item.inscription_scroll_spec.as_ref()),
        Some(InscriptionScrollSpec { inscription_id }) if inscription_id == "qi_amplify_v0"
    ));
    for required in [
        "iron_sword_flawed",
        "qing_feng_sword",
        "qing_feng_sword_flawed",
        "ling_feng_sword",
        "ling_feng_sword_flawed",
        "ling_mu_gun",
        "ling_mu_ban",
        "ling_mu_jing",
        "ling_xia",
        "ling_mu_miao",
        "feng_he_gu",
        "yi_shou_gu",
        "xuan_iron",
        "qing_steel",
    ] {
        assert!(
            registry.get(required).is_some(),
            "forge asset `{required}` must be registered"
        );
    }
    for anqi_item in [
        "anqi_bone_chip",
        "anqi_bone_chip_charged",
        "anqi_yibian_shougu",
        "anqi_yibian_shougu_charged",
        "anqi_lingmu_arrow",
        "anqi_lingmu_arrow_charged",
        "anqi_dyed_bone",
        "anqi_dyed_bone_charged",
        "anqi_fenglinghe_bone",
        "anqi_fenglinghe_bone_charged",
        "anqi_shanggu_bone",
        "anqi_shanggu_bone_charged",
        "anqi_container_quiver",
        "anqi_container_pocket_pouch",
        "anqi_container_fenglinghe",
    ] {
        let template = registry
            .get(anqi_item)
            .unwrap_or_else(|| panic!("anqi asset `{anqi_item}` must be registered"));
        assert!(
            (0.0..=1.0).contains(&template.spirit_quality_initial),
            "anqi asset `{anqi_item}` spirit quality must remain within item registry bounds"
        );
    }
    for required_tool in [
        "cai_yao_dao",
        "bao_chu",
        "cao_lian",
        "dun_qi_jia",
        "gua_dao",
        "gu_hai_qian",
        "bing_jia_shou_tao",
        // plan-zhenfa-trap-client-equip-gate-v1 P2 — zhenfa.toml 阵法工具（此前漏收进本 pin，
        // 是导致 client TOOL_TEMPLATE_IDS 白名单漂移未被察觉的同一契约缺口）。
        "warning_trap",
        "blast_trap",
        "slow_trap",
        "array_flag",
    ] {
        let template = registry
            .get(required_tool)
            .unwrap_or_else(|| panic!("tool asset `{required_tool}` must be registered"));
        assert!(
            matches!(template.category, ItemCategory::Tool),
            "tool asset `{required_tool}` must parse as ItemCategory::Tool"
        );
        assert!(
            template.weapon_spec.is_none(),
            "tool asset `{required_tool}` must not define combat weapon stats"
        );
    }
    assert_eq!(
        registry
            .get("ci_she_hao")
            .expect("herb template should load")
            .max_stack_count,
        64
    );
    assert_eq!(
        registry
            .get("guyuan_pill")
            .expect("pill template should load")
            .max_stack_count,
        16
    );
    assert_eq!(
        registry
            .get("fengling_bone_coin")
            .expect("bone coin template should load")
            .max_stack_count,
        u32::MAX
    );
    assert_eq!(
        registry
            .get("iron_sword")
            .expect("weapon template should load")
            .max_stack_count,
        1
    );
}

// ── plan-food-v1 P2 BLOCKER 1：food.toml FoodRegen effect 解析测试 ──

/// BLOCKER 1 端到端：food.toml → ItemRegistry → ling_guo.effect = FoodRegen{0.20, 48000}
#[test]
fn food_toml_ling_guo_has_food_regen_effect() {
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");
    let ling_guo = registry
        .get("food.spirit_fruit.ling_guo")
        .expect("food.toml ling_guo must be registered");
    match &ling_guo.effect {
        Some(ItemEffect::FoodRegen {
            bonus_factor,
            duration_ticks,
        }) => {
            assert!(
                (bonus_factor - 0.20).abs() < 1e-4,
                "ling_guo bonus_factor 应=0.20（+20% 修炼速度），实际 {bonus_factor}"
            );
            assert_eq!(
                *duration_ticks, 48_000u64,
                "ling_guo duration_ticks 应=48000（2 GAME_DAY），实际 {duration_ticks}"
            );
        }
        other => panic!("ling_guo.effect 应为 FoodRegen{{0.20, 48000}}，实际 {other:?}"),
    }
}

/// BLOCKER 1 端到端：food.toml → chen_jiu.effect = FoodRegen{0.15, 36000}
#[test]
fn food_toml_chen_jiu_has_food_regen_effect() {
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");
    let chen_jiu = registry
        .get("food.spirit_wine.chen_jiu")
        .expect("food.toml chen_jiu must be registered");
    match &chen_jiu.effect {
        Some(ItemEffect::FoodRegen {
            bonus_factor,
            duration_ticks,
        }) => {
            assert!(
                (bonus_factor - 0.15).abs() < 1e-4,
                "chen_jiu bonus_factor 应=0.15（+15% 修炼速度），实际 {bonus_factor}"
            );
            assert_eq!(
                *duration_ticks, 36_000u64,
                "chen_jiu duration_ticks 应=36000（1.5 GAME_DAY），实际 {duration_ticks}"
            );
        }
        other => panic!("chen_jiu.effect 应为 FoodRegen{{0.15, 36000}}，实际 {other:?}"),
    }
}

/// 凡俗食物（cooked_meat / chen_bing）不挂修炼加速 effect
#[test]
fn food_toml_mundane_foods_have_no_cultivation_effect() {
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");
    for mundane in ["food.mundane.cooked_meat", "food.mundane.chen_bing"] {
        let item = registry
            .get(mundane)
            .unwrap_or_else(|| panic!("food.toml {mundane} must be registered"));
        assert!(
            item.effect.is_none(),
            "凡俗食物 `{mundane}` 不应有修炼加速 effect，实际 {:?}",
            item.effect
        );
    }
}

/// food_regen 解析：duration_ticks 缺失时应报错
#[test]
fn parse_item_effect_food_regen_missing_duration_ticks_returns_error() {
    let err = parse_item_effect(
        ItemEffectToml {
            kind: "food_regen".to_string(),
            magnitude: 0.20,
            target: None,
            duration_ticks: None,
        },
        std::path::Path::new("<test>"),
        "test_food_item",
    )
    .expect_err("food_regen 缺失 duration_ticks 应返回 Err");
    assert!(
        err.contains("duration_ticks"),
        "错误信息应包含 'duration_ticks'，实际: {err}"
    );
}

/// food_regen 解析：duration_ticks = 0 时应报错
#[test]
fn parse_item_effect_food_regen_zero_duration_ticks_returns_error() {
    let err = parse_item_effect(
        ItemEffectToml {
            kind: "food_regen".to_string(),
            magnitude: 0.20,
            target: None,
            duration_ticks: Some(0),
        },
        std::path::Path::new("<test>"),
        "test_food_item",
    )
    .expect_err("food_regen duration_ticks=0 应返回 Err");
    assert!(
        err.contains("duration_ticks"),
        "错误信息应包含 'duration_ticks'，实际: {err}"
    );
}

/// food_regen 解析：合法参数应成功 → FoodRegen{bonus_factor: 0.20, duration_ticks: 48000}
#[test]
fn parse_item_effect_food_regen_valid_returns_food_regen() {
    let effect = parse_item_effect(
        ItemEffectToml {
            kind: "food_regen".to_string(),
            magnitude: 0.20,
            target: None,
            duration_ticks: Some(48_000),
        },
        std::path::Path::new("<test>"),
        "test_ling_guo",
    )
    .expect("合法 food_regen 参数应成功解析");
    match effect {
        ItemEffect::FoodRegen {
            bonus_factor,
            duration_ticks,
        } => {
            assert!(
                (bonus_factor - 0.20).abs() < 1e-4,
                "bonus_factor 应=0.20，实际 {bonus_factor}"
            );
            assert_eq!(
                duration_ticks, 48_000,
                "duration_ticks 应=48000，实际 {duration_ticks}"
            );
        }
        other => panic!("期望 FoodRegen，实际 {other:?}"),
    }
}

// ── plan-cultivation-pacing-v1 P2.2：次品修炼丹药模板加载测试 ──

#[test]
fn flawed_cultivation_pill_templates_load_from_assets() {
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");

    let flawed_ling_xi = registry
        .get("ling_xi_wan_flawed")
        .expect("ling_xi_wan_flawed template should load from pills.toml");
    assert_eq!(flawed_ling_xi.display_name, "灵息丸（次品）");
    assert_eq!(flawed_ling_xi.category, ItemCategory::Pill);
    assert_eq!(flawed_ling_xi.rarity, ItemRarity::Common);

    let flawed_ju_ling = registry
        .get("ju_ling_dan_flawed")
        .expect("ju_ling_dan_flawed template should load from pills.toml");
    assert_eq!(flawed_ju_ling.display_name, "聚灵丹（次品）");
    assert_eq!(flawed_ju_ling.category, ItemCategory::Pill);
    assert_eq!(flawed_ju_ling.rarity, ItemRarity::Common);
}

#[test]
fn all_eight_cultivation_pill_templates_load_from_assets() {
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");
    let ids = [
        "ling_xi_wan",
        "ju_ling_dan",
        "tong_mai_san",
        "ning_yuan_dan",
        "xi_sui_ye",
        "po_jing_dan",
        "kai_qiao_dan",
        "du_jie_dan",
    ];
    for id in ids {
        assert!(
            registry.get(id).is_some(),
            "cultivation pill template `{id}` should be registered in assets/items/pills.toml"
        );
        let template = registry.get(id).unwrap();
        assert_eq!(
            template.category,
            ItemCategory::Pill,
            "`{id}` should have category Pill"
        );
    }
}

#[test]
fn woliu_scrolls_load_as_combat_technique_templates() {
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");
    let woliu_scrolls = registry
        .templates
        .values()
        .filter(|template| {
            template
                .technique_scroll_spec
                .as_ref()
                .is_some_and(|spec| spec.skill_id.starts_with("woliu."))
        })
        .collect::<Vec<_>>();

    assert_eq!(woliu_scrolls.len(), 11);
    assert!(woliu_scrolls.iter().all(|template| {
        matches!(template.category, ItemCategory::Scroll)
            && template
                .technique_scroll_spec
                .as_ref()
                .is_some_and(|spec| spec.kind == "combat_technique")
    }));
}

#[test]
fn woliu_scroll_skill_ids_are_known_techniques() {
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");
    let ids = registry
        .templates
        .values()
        .filter_map(|template| {
            template
                .technique_scroll_spec
                .as_ref()
                .map(|spec| spec.skill_id.as_str())
        })
        .collect::<Vec<_>>();

    assert_eq!(ids.iter().filter(|id| id.starts_with("woliu.")).count(), 11);
    let techniques = crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests();
    for id in ids {
        assert!(
            techniques.get(id).is_some(),
            "technique scroll references unknown id `{id}`"
        );
    }
}

#[test]
fn item_template_toml_allows_explicit_max_stack_override() {
    let raw: ItemTemplatesToml = toml::from_str(
        r#"
[[item]]
id = "test_powder"
name = "测试粉"
category = "misc"
grid_w = 1
grid_h = 1
base_weight = 0.1
rarity = "common"
spirit_quality_initial = 1.0
description = "测试"
max_stack_count = 7
"#,
    )
    .expect("inline item TOML should parse");

    let template = raw
        .item
        .into_iter()
        .next()
        .expect("fixture should contain one item")
        .try_into_item_template(Path::new("<inline-items.toml>"))
        .expect("explicit max_stack_count should be accepted");

    assert_eq!(template.max_stack_count, 7);
}

// ─── plan-race-system-v1 P3b —— ItemTemplate.wearer_race TOML pin 测试 ───

#[test]
fn item_template_toml_without_wearer_race_defaults_to_any() {
    // 老配置不带 [item.wearer_race] 字段——`#[serde(default)]` 必须解析为 Any，
    // 保证既有几百个 item TOML 条目零改动继续过验（绝大多数物品任何种族可穿）。
    let raw: ItemTemplatesToml = toml::from_str(
        r#"
[[item]]
id = "test_no_race_gate"
name = "无种族门测试件"
category = "misc"
grid_w = 1
grid_h = 1
base_weight = 0.1
rarity = "common"
spirit_quality_initial = 1.0
description = "测试"
"#,
    )
    .expect("inline item TOML should parse");

    let template = raw
        .item
        .into_iter()
        .next()
        .expect("fixture should contain one item")
        .try_into_item_template(Path::new("<inline-items.toml>"))
        .expect("missing wearer_race must default, not error");

    assert_eq!(
        template.wearer_race,
        RaceGateOwned::Any,
        "老配置无 wearer_race 字段必须解析为 Any（绝大多数物品任何种族可穿）"
    );
}

#[test]
fn item_template_toml_parses_explicit_humanoid_wearer_race() {
    let raw: ItemTemplatesToml = toml::from_str(
        r#"
[[item]]
id = "test_humanoid_only"
name = "人形限定测试件"
category = "misc"
grid_w = 1
grid_h = 1
base_weight = 0.1
rarity = "common"
spirit_quality_initial = 1.0
description = "测试"

[item.wearer_race]
kind = "humanoid"
"#,
    )
    .expect("inline item TOML should parse");

    let template = raw
        .item
        .into_iter()
        .next()
        .expect("fixture should contain one item")
        .try_into_item_template(Path::new("<inline-items.toml>"))
        .expect("explicit humanoid wearer_race should be accepted");

    assert_eq!(template.wearer_race, RaceGateOwned::Humanoid);
}

#[test]
fn item_template_toml_parses_explicit_species_wearer_race() {
    let raw: ItemTemplatesToml = toml::from_str(
        r#"
[[item]]
id = "test_whale_only"
name = "飞鲸限定测试件"
category = "misc"
grid_w = 1
grid_h = 1
base_weight = 0.1
rarity = "common"
spirit_quality_initial = 1.0
description = "测试"

[item.wearer_race]
kind = "species"
species = ["whale"]
"#,
    )
    .expect("inline item TOML should parse");

    let template = raw
        .item
        .into_iter()
        .next()
        .expect("fixture should contain one item")
        .try_into_item_template(Path::new("<inline-items.toml>"))
        .expect("explicit species wearer_race should be accepted");

    assert_eq!(
        template.wearer_race,
        RaceGateOwned::Species {
            species: vec![RaceId::new("whale")]
        }
    );
}

#[test]
fn item_template_toml_rejects_unknown_wearer_race_kind() {
    // RaceGateOwned 自身 `#[serde(tag = "kind", ...)]` fail-closed：未知 kind 直接
    // 解析失败（非静默兜底 Any），与 body_plan::types 的既有 pin 测试同惯例。
    let result: Result<ItemTemplatesToml, _> = toml::from_str(
        r#"
[[item]]
id = "test_bad_race_gate"
name = "坏种族门测试件"
category = "misc"
grid_w = 1
grid_h = 1
base_weight = 0.1
rarity = "common"
spirit_quality_initial = 1.0
description = "测试"

[item.wearer_race]
kind = "bogus"
"#,
    );
    assert!(
        result.is_err(),
        "未知 wearer_race.kind 必须在 TOML 层直接拒绝反序列化"
    );
}

#[test]
fn item_template_toml_rejects_zero_max_stack() {
    let raw: ItemTemplatesToml = toml::from_str(
        r#"
[[item]]
id = "bad_powder"
name = "坏粉"
category = "misc"
grid_w = 1
grid_h = 1
base_weight = 0.1
rarity = "common"
spirit_quality_initial = 1.0
description = "测试"
max_stack_count = 0
"#,
    )
    .expect("inline item TOML should parse");

    let error = raw
        .item
        .into_iter()
        .next()
        .expect("fixture should contain one item")
        .try_into_item_template(Path::new("<inline-items.toml>"))
        .expect_err("zero max_stack_count should be rejected");

    assert!(error.contains("invalid max_stack_count 0"));
}

#[test]
fn parse_item_category_accepts_tool_alias() {
    let category = parse_item_category("tool", Path::new("<inline-items.toml>"), "cai_yao_dao")
        .expect("tool category should parse");

    assert_eq!(category, ItemCategory::Tool);
}

#[test]
fn parse_item_category_accepts_armor_aliases() {
    for alias in ["armor", "armour"] {
        let category = parse_item_category(
            alias,
            Path::new("<inline-items.toml>"),
            "armor_bone_chestplate",
        )
        .expect("armor category alias should parse");

        assert_eq!(category, ItemCategory::Armor);
    }
}

#[test]
fn parse_item_category_accepts_block_alias() {
    for raw in ["block", "Block", " block "] {
        let category = parse_item_category(raw, Path::new("<inline-items.toml>"), "earth_crumb")
            .expect("block category alias should parse");

        assert_eq!(category, ItemCategory::Block);
    }
}

#[test]
fn block_category_default_stack_count_is_64() {
    assert_eq!(
        default_max_stack_count_for_category(ItemCategory::Block),
        64
    );
}

#[test]
fn block_material_templates_load_with_block_category_and_default_stack() {
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");

    for template_id in BLOCK_ITEM_TEMPLATE_IDS {
        let template = registry
            .get(template_id)
            .unwrap_or_else(|| panic!("block item `{template_id}` should load"));
        assert_eq!(
            template.category,
            ItemCategory::Block,
            "block item `{template_id}` must use ItemCategory::Block"
        );
        assert_eq!(
            template.max_stack_count, 64,
            "block item `{template_id}` should inherit Block default stack count"
        );
    }
}

#[test]
fn shelter_block_templates_keep_inventory_footprint_and_weight() {
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");
    let cases = [
        ("torch_item", 1, 1, 0.2),
        ("lantern_item", 1, 1, 0.6),
        ("door_bolt", 1, 1, 1.5),
        ("window_grate", 1, 1, 2.0),
        ("simple_bed", 2, 2, 4.0),
        ("meditation_mat", 2, 2, 1.5),
        ("moisture_base", 2, 1, 3.0),
        ("spirit_stone_rack", 1, 1, 1.0),
    ];

    for (template_id, grid_w, grid_h, base_weight) in cases {
        let template = registry
            .get(template_id)
            .unwrap_or_else(|| panic!("shelter block item `{template_id}` should load"));
        assert_eq!(
            (template.grid_w, template.grid_h),
            (grid_w, grid_h),
            "shelter block item `{template_id}` must keep its inventory footprint"
        );
        assert!(
            (template.base_weight - base_weight).abs() < f64::EPSILON,
            "shelter block item `{template_id}` must keep base_weight {base_weight}, got {}",
            template.base_weight
        );
    }
}

#[test]
fn parse_forge_station_spec_accepts_valid_tier() {
    let spec = parse_forge_station_spec(
        ForgeStationSpecToml { tier: 4 },
        Path::new("<inline-items.toml>"),
        "dao_anvil",
    )
    .expect("tier 4 forge station should parse");

    assert_eq!(spec.tier, 4);
}

#[test]
fn parse_forge_station_spec_rejects_invalid_tier() {
    let error = parse_forge_station_spec(
        ForgeStationSpecToml { tier: 0 },
        Path::new("<inline-items.toml>"),
        "bad_anvil",
    )
    .expect_err("tier 0 forge station should fail");

    assert!(error.contains("expected 1..=4"));
}

#[test]
fn parse_blueprint_scroll_spec_accepts_blueprint_id() {
    let spec = parse_blueprint_scroll_spec(
        BlueprintScrollSpecToml {
            blueprint_id: "qing_feng_v0".to_string(),
        },
        Path::new("<inline-items.toml>"),
        "blueprint_scroll_qing_feng",
    )
    .expect("blueprint scroll should parse");

    assert_eq!(spec.blueprint_id, "qing_feng_v0");
}

#[test]
fn parse_blueprint_scroll_spec_rejects_empty_blueprint_id() {
    let error = parse_blueprint_scroll_spec(
        BlueprintScrollSpecToml {
            blueprint_id: " ".to_string(),
        },
        Path::new("<inline-items.toml>"),
        "bad_blueprint_scroll",
    )
    .expect_err("empty blueprint id should fail");

    assert!(error.contains("blueprint_scroll.blueprint_id"));
}

// ── plan-scroll-reading-v1 P0：parse_readable_scroll_spec ──────────────────

#[test]
fn parse_readable_scroll_spec_accepts_three_pages_and_anim_id() {
    let spec = parse_readable_scroll_spec(
        ReadableScrollSpecToml {
            title: "《经脉浅述·残卷》".to_string(),
            body_pages: vec![
                "第一页".to_string(),
                "第二页".to_string(),
                "第三页".to_string(),
            ],
            anim_id: Some("bong:read_scroll".to_string()),
        },
        Path::new("<inline-items.toml>"),
        "scroll_meridian_primer",
    )
    .expect("readable scroll spec should parse");

    assert_eq!(spec.title, "《经脉浅述·残卷》");
    assert_eq!(spec.body_pages.len(), 3);
    assert_eq!(spec.anim_id.as_deref(), Some("bong:read_scroll"));
}

#[test]
fn parse_readable_scroll_spec_accepts_none_anim_id() {
    let spec = parse_readable_scroll_spec(
        ReadableScrollSpecToml {
            title: "无动画残卷".to_string(),
            body_pages: vec!["单页".to_string()],
            anim_id: None,
        },
        Path::new("<inline-items.toml>"),
        "scroll_no_anim",
    )
    .expect("readable scroll without anim_id should parse (anim_id is Optional)");

    assert_eq!(spec.anim_id, None);
}

#[test]
fn parse_readable_scroll_spec_rejects_empty_title() {
    let error = parse_readable_scroll_spec(
        ReadableScrollSpecToml {
            title: "   ".to_string(),
            body_pages: vec!["p1".to_string()],
            anim_id: None,
        },
        Path::new("<inline-items.toml>"),
        "bad_readable_scroll",
    )
    .expect_err("blank title should fail");

    assert!(error.contains("readable_scroll.title"));
}

#[test]
fn parse_readable_scroll_spec_rejects_zero_pages() {
    let error = parse_readable_scroll_spec(
        ReadableScrollSpecToml {
            title: "t".to_string(),
            body_pages: vec![],
            anim_id: None,
        },
        Path::new("<inline-items.toml>"),
        "bad_readable_scroll",
    )
    .expect_err("0 body_pages should fail (§9 至少 1 页)");

    assert!(error.contains("body_pages"));
}

#[test]
fn parse_readable_scroll_spec_rejects_blank_page() {
    // 边界：第 2 页（非首页）为纯空白，必须也被拒绝（逐页校验，不只查第一页）。
    let error = parse_readable_scroll_spec(
        ReadableScrollSpecToml {
            title: "t".to_string(),
            body_pages: vec!["第一页有内容".to_string(), "   ".to_string()],
            anim_id: None,
        },
        Path::new("<inline-items.toml>"),
        "bad_readable_scroll",
    )
    .expect_err("blank page (index 1) should fail");

    assert!(error.contains("body_pages[1]"));
}

#[test]
fn parse_readable_scroll_spec_rejects_blank_anim_id_when_some() {
    let error = parse_readable_scroll_spec(
        ReadableScrollSpecToml {
            title: "t".to_string(),
            body_pages: vec!["p1".to_string()],
            anim_id: Some("   ".to_string()),
        },
        Path::new("<inline-items.toml>"),
        "bad_readable_scroll",
    )
    .expect_err("blank anim_id (Some(whitespace)) should fail, not silently accept");

    assert!(error.contains("readable_scroll.anim_id"));
}

#[test]
fn parse_readable_scroll_spec_accepts_single_page_boundary() {
    // 边界：恰好 1 页（下界，§9 "至少 1 页"）。
    let spec = parse_readable_scroll_spec(
        ReadableScrollSpecToml {
            title: "t".to_string(),
            body_pages: vec!["only page".to_string()],
            anim_id: None,
        },
        Path::new("<inline-items.toml>"),
        "scroll_single_page",
    )
    .expect("exactly 1 page should be accepted (lower boundary)");

    assert_eq!(spec.body_pages, vec!["only page".to_string()]);
}

#[test]
fn onboarding_scroll_meridian_primer_parses() {
    let registry = load_item_registry().expect("item registry should load");
    let template = registry
        .get("scroll_meridian_primer")
        .expect("scroll_meridian_primer should be registered from onboarding_scrolls.toml");

    assert_eq!(template.category, ItemCategory::Scroll);
    assert_eq!(template.max_stack_count, 1);
    assert_eq!(template.grid_w, 1);
    assert_eq!(template.grid_h, 2);

    let spec = template
        .readable_scroll_spec
        .as_ref()
        .expect("scroll_meridian_primer must carry a readable_scroll_spec");
    assert_eq!(spec.title, "《经脉浅述·残卷》");
    assert_eq!(
        spec.body_pages.len(),
        3,
        "§8.1 #3 决议：3 页正文，实得 {} 页",
        spec.body_pages.len()
    );
    for (idx, page) in spec.body_pages.iter().enumerate() {
        assert!(
            !page.trim().is_empty(),
            "page[{idx}] must not be blank once loaded from the real TOML asset"
        );
    }
    assert_eq!(
        spec.anim_id.as_deref(),
        Some("bong:read_scroll"),
        "P2 阅读动画 id 应在 P0 就写入 TOML（anim_id 字段），供 P2 直接消费"
    );
}

#[test]
fn parse_inscription_scroll_spec_accepts_inscription_id() {
    let spec = parse_inscription_scroll_spec(
        InscriptionScrollSpecToml {
            inscription_id: "sharp_v0".to_string(),
        },
        Path::new("<inline-items.toml>"),
        "inscription_scroll_sharp_v0",
    )
    .expect("inscription scroll should parse");

    assert_eq!(spec.inscription_id, "sharp_v0");
}

#[test]
fn parse_inscription_scroll_spec_rejects_empty_inscription_id() {
    let error = parse_inscription_scroll_spec(
        InscriptionScrollSpecToml {
            inscription_id: " ".to_string(),
        },
        Path::new("<inline-items.toml>"),
        "bad_inscription_scroll",
    )
    .expect_err("empty inscription id should fail");

    assert!(error.contains("inscription_scroll.inscription_id"));
}

#[test]
fn loads_default_loadout_includes_textured_starter_kit() {
    // 默认 loadout 改用有 client PNG 的物品（避免 missing_texture 渲染）。
    // 至少应包含 spirit_grass / ningmai_powder（plan-HUD-v1 起手套件）。
    let registry = load_item_registry().expect("item registry should load");
    let loadout = load_default_loadout(&registry).expect("default loadout should load");

    let all_template_ids: Vec<&str> = loadout
        .containers
        .iter()
        .flat_map(|c| c.items.iter().map(|p| p.instance.template_id.as_str()))
        .chain(
            loadout
                .equipped
                .values()
                .flat_map(|s| s.iter_all())
                .map(|item| item.template_id.as_str()),
        )
        .chain(
            loadout
                .hotbar
                .iter()
                .flatten()
                .map(|item| item.template_id.as_str()),
        )
        .collect();

    for required in ["spirit_grass", "ningmai_powder", "guyuan_pill"] {
        assert!(
            all_template_ids.contains(&required),
            "default loadout missing required textured item `{required}`; have: {all_template_ids:?}"
        );
    }
    assert!(
        !all_template_ids.contains(&"niche_base"),
        "niche_base must be granted by spawn coffin, not default loadout"
    );
}

#[test]
fn rejects_unknown_template_in_loadout() {
    let registry = test_registry_from_strs(&[("starter_talisman", "启程护符")])
        .expect("registry fixture should construct");

    let loadout_toml = r#"
max_weight = 40.0

[[containers]]
id = "main_pack"
name = "主背包"
rows = 5
cols = 7

  [[containers.items]]
  row = 0
  col = 0
  template_id = "missing_template"

[[containers]]
id = "small_pouch"
name = "小口袋"
rows = 3
cols = 3

[[containers]]
id = "front_satchel"
name = "前挂包"
rows = 3
cols = 4
"#;

    let parsed: LoadoutToml =
        toml::from_str(loadout_toml).expect("fixture TOML should parse into LoadoutToml");
    let error = parsed
        .try_into_loadout(Path::new("<inline-loadout.toml>"), &registry)
        .expect_err("unknown template id in loadout should fail");

    assert!(error.contains("unknown template id `missing_template`"));
}

#[test]
fn loadout_requires_fixed_container_ids() {
    let registry = test_registry_from_strs(&[("starter_talisman", "启程护符")])
        .expect("registry fixture should construct");

    let loadout_toml = r#"
[[containers]]
id = "main_pack"
name = "主背包"
rows = 5
cols = 7

[[containers]]
id = "unknown_pack"
name = "未知"
rows = 3
cols = 3

[[containers]]
id = "front_satchel"
name = "前挂包"
rows = 3
cols = 4
"#;

    let parsed: LoadoutToml =
        toml::from_str(loadout_toml).expect("fixture TOML should parse into LoadoutToml");
    let error = parsed
        .try_into_loadout(Path::new("<inline-loadout.toml>"), &registry)
        .expect_err("unknown container id should fail");

    assert!(error.contains("unsupported container id `unknown_pack`"));
}

#[test]
fn loadout_rejects_duplicate_container_ids_during_parse() {
    let registry = test_registry_from_strs(&[("starter_talisman", "启程护符")])
        .expect("registry fixture should construct");

    let loadout_toml = r#"
[[containers]]
id = "main_pack"
name = "主背包"
rows = 5
cols = 7

[[containers]]
id = "main_pack"
name = "备用主背包"
rows = 4
cols = 6

[[containers]]
id = "small_pouch"
name = "小口袋"
rows = 3
cols = 3

[[containers]]
id = "front_satchel"
name = "前挂包"
rows = 3
cols = 4
"#;

    let parsed: LoadoutToml =
        toml::from_str(loadout_toml).expect("fixture TOML should parse into LoadoutToml");
    let error = parsed
        .try_into_loadout(Path::new("<inline-loadout.toml>"), &registry)
        .expect_err("duplicate container id should fail during parse");

    assert!(error.contains("duplicate container id `main_pack`"));
}

#[test]
fn rejects_placed_item_whose_multicell_footprint_overflows_container_bounds() {
    let mut templates = HashMap::new();
    templates.insert(
        "wide_talisman".to_string(),
        ItemTemplate {
            id: "wide_talisman".to_string(),
            display_name: "阔符".to_string(),
            category: ItemCategory::Misc,
            placeable: None,
            max_stack_count: 1,
            grid_w: 2,
            grid_h: 2,
            base_weight: 0.1,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 1.0,
            description: "test template".to_string(),
            effect: None,
            cast_duration_ms: DEFAULT_CAST_DURATION_MS,
            cooldown_ms: DEFAULT_COOLDOWN_MS,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            readable_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shield_spec: None,

            shelflife_profile: None,
            shelflife_track: None,
            wearer_race: crate::body_plan::types::RaceGateOwned::default(),
        },
    );
    let registry = ItemRegistry { templates };

    let loadout_toml = r#"
[[containers]]
id = "main_pack"
name = "主背包"
rows = 5
cols = 7

  [[containers.items]]
  row = 4
  col = 6
  template_id = "wide_talisman"

[[containers]]
id = "small_pouch"
name = "小口袋"
rows = 3
cols = 3

[[containers]]
id = "front_satchel"
name = "前挂包"
rows = 3
cols = 4
"#;

    let parsed: LoadoutToml =
        toml::from_str(loadout_toml).expect("fixture TOML should parse into LoadoutToml");
    let error = parsed
        .try_into_loadout(Path::new("<inline-loadout.toml>"), &registry)
        .expect_err("multi-cell footprint overflow should fail");

    assert!(error.contains("footprint overflows"));
}

#[test]
fn rejects_overlapping_multicell_item_footprints_within_container() {
    let mut templates = HashMap::new();
    templates.insert(
        "wide_talisman".to_string(),
        ItemTemplate {
            id: "wide_talisman".to_string(),
            display_name: "阔符".to_string(),
            category: ItemCategory::Misc,
            placeable: None,
            max_stack_count: 1,
            grid_w: 2,
            grid_h: 2,
            base_weight: 0.1,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 1.0,
            description: "test template".to_string(),
            effect: None,
            cast_duration_ms: DEFAULT_CAST_DURATION_MS,
            cooldown_ms: DEFAULT_COOLDOWN_MS,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            readable_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shield_spec: None,

            shelflife_profile: None,
            shelflife_track: None,
            wearer_race: crate::body_plan::types::RaceGateOwned::default(),
        },
    );
    let registry = ItemRegistry { templates };

    let loadout_toml = r#"
[[containers]]
id = "main_pack"
name = "主背包"
rows = 5
cols = 7

  [[containers.items]]
  row = 0
  col = 0
  template_id = "wide_talisman"

  [[containers.items]]
  row = 1
  col = 1
  template_id = "wide_talisman"

[[containers]]
id = "small_pouch"
name = "小口袋"
rows = 3
cols = 3

[[containers]]
id = "front_satchel"
name = "前挂包"
rows = 3
cols = 4
"#;

    let parsed: LoadoutToml =
        toml::from_str(loadout_toml).expect("fixture TOML should parse into LoadoutToml");
    let error = parsed
        .try_into_loadout(Path::new("<inline-loadout.toml>"), &registry)
        .expect_err("overlapping multi-cell footprints should fail during parse");

    assert!(error.contains("overlaps existing item `wide_talisman`"));
}

#[test]
fn loadout_rejects_spirit_stones_field_in_v1() {
    let loadout_toml = r#"
spirit_stones = 100

[[containers]]
id = "main_pack"
name = "主背包"
rows = 5
cols = 7

[[containers]]
id = "small_pouch"
name = "小口袋"
rows = 3
cols = 3

[[containers]]
id = "front_satchel"
name = "前挂包"
rows = 3
cols = 4
"#;

    let error = toml::from_str::<LoadoutToml>(loadout_toml)
        .expect_err("unknown spirit_stones field should be rejected by deny_unknown_fields")
        .to_string();

    assert!(error.contains("unknown field `spirit_stones`"));
}

#[test]
fn item_registry_loads_all_24_mundane_armor_templates() {
    let registry = load_item_registry().expect("item registry should load");

    for item in crate::armor::mundane::all_mundane_armor_items() {
        let template = registry
            .get(item.item_id().as_str())
            .unwrap_or_else(|| panic!("{} should load from armor.toml", item.item_id()));
        assert_eq!(template.category, ItemCategory::Armor);
        assert_eq!(template.max_stack_count, 1);
    }
}

#[test]
fn item_registry_loads_mundane_armor_unlock_scroll_templates() {
    let registry = load_item_registry().expect("item registry should load");

    for material in crate::armor::mundane::MundaneArmorMaterial::ALL {
        let id = format!("scroll_armor_{}", material.id());
        let template = registry
            .get(id.as_str())
            .unwrap_or_else(|| panic!("{id} should load from armor.toml"));
        assert_eq!(template.category, ItemCategory::Misc);
        assert_eq!(template.grid_w, 1);
        assert_eq!(template.grid_h, 2);
    }
}

#[test]
fn apply_move_allows_mundane_armor_to_matching_slot() {
    use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.template_id = "armor_bone_chestplate".to_string();
    inv.containers[0].items[0].instance.display_name = "骨甲胸甲".to_string();
    inv.containers[0].items[0].instance.grid_w = 2;
    inv.containers[0].items[0].instance.grid_h = 2;

    let outcome = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Equip {
            slot: EquipSlotV1::Chest,
            state: crate::schema::inventory::EquipStateV1::Worn,
        },
        false,
    )
    .expect("chestplate should equip to chest");

    assert_eq!(
        outcome,
        InventoryMoveOutcome::Moved {
            revision: InventoryRevision(8)
        }
    );
    assert_eq!(
        inv.equipped
            .get(EQUIP_SLOT_CHEST)
            .and_then(|s| s.worn.first())
            .map(|item| item.template_id.as_str()),
        Some("armor_bone_chestplate")
    );
}

#[test]
fn apply_move_rejects_mundane_armor_to_wrong_slot() {
    use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.template_id = "armor_bone_chestplate".to_string();
    inv.containers[0].items[0].instance.display_name = "骨甲胸甲".to_string();

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Equip {
            slot: EquipSlotV1::Head,
            state: crate::schema::inventory::EquipStateV1::Worn,
        },
        false,
    )
    .expect_err("chestplate should not equip to head");

    assert!(matches!(
        error,
        InventoryMoveRejectReason::ArmorSlotMismatch { expected_slot } if expected_slot == "chest"
    ));
}

/// 数据/注册表缺口场景：category=Armor 但 `template_id` 不遵循
/// `armor_<material>_<slot>` 命名（`equip_slot_for_item_id` 因此返回 `None`）。
/// 这与上面 `apply_move_rejects_mundane_armor_to_wrong_slot`（已知 expected_slot=="chest"，
/// 穿错槽）是不同语义分支——此处连 expected_slot 都解析不出来，必须走独立的
/// `ArmorSlotUnresolvable`，不能塞进 `ArmorSlotMismatch` 硬编一个 `"unknown"` 占位符
/// （该占位符此前会原样下发 client，拼进中文文案变成"应装于unknown"）。
#[test]
fn apply_move_rejects_armor_with_unresolvable_equip_slot() {
    use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

    let mut mystery_armor = make_container_template("mystery_plate", EQUIP_SLOT_CHEST, 1, 1, 0.0);
    mystery_armor.container_spec = None;
    mystery_armor.category = ItemCategory::Armor;
    let registry = ItemRegistry::from_map(HashMap::from([(
        "mystery_plate".to_string(),
        mystery_armor,
    )]));

    let mut inv = make_empty_inventory();
    inv.containers.push(ContainerState {
        quick_access: false,
        id: MAIN_PACK_CONTAINER_ID.to_string(),
        name: "主背包".to_string(),
        rows: 5,
        cols: 7,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_container_item(42, "mystery_plate"),
        }],
        owner_instance_id: None,
    });

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: MAIN_PACK_CONTAINER_ID.to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Equip {
            slot: EquipSlotV1::Chest,
            state: crate::schema::inventory::EquipStateV1::Worn,
        },
        false,
    )
    .expect_err("armor with unresolvable equip slot should be rejected");

    assert!(
        matches!(error, InventoryMoveRejectReason::ArmorSlotUnresolvable),
        "unresolvable armor equip slot must use the dedicated variant, not \
         ArmorSlotMismatch with a fake 'unknown' expected_slot placeholder, 实际={error:?}"
    );
    assert_eq!(
        error.to_wire_tag(),
        "armor_slot_unresolvable",
        "wire tag must be distinct from armor_slot_mismatch"
    );
    assert_eq!(
        error.slot(),
        None,
        "unresolvable slot carries no slot data — nothing to hand off to client"
    );
}

#[test]
fn apply_move_rejects_broken_armor_unequippable() {
    use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.template_id = "armor_bone_chestplate".to_string();
    inv.containers[0].items[0].instance.display_name = "骨甲胸甲".to_string();
    inv.containers[0].items[0].instance.durability = 0.0;

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Equip {
            slot: EquipSlotV1::Chest,
            state: crate::schema::inventory::EquipStateV1::Worn,
        },
        false,
    )
    .expect_err("broken armor should be rejected");

    assert!(matches!(
        error,
        InventoryMoveRejectReason::ArmorDurabilityZero
    ));
}

// plan-layered-equip-v1 P0.2（决议 #7）— 两手兵器锁对侧手：staff 在 main_hand held →
// off_hand 被锁，任何件拖入 off_hand 被拒（two_hand 槽已删，改测对侧锁）。
#[test]
fn apply_move_rejects_off_hand_when_main_hand_two_handed() {
    use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    // off_hand 能接受 dagger；这里用 bone_dagger 验证它依然被对侧锁挡住。
    inv.containers[0].items[0].instance.template_id = "bone_dagger".to_string();
    inv.containers[0].items[0].instance.display_name = "骨刀".to_string();
    inv.containers[0].items[0].instance.grid_w = 1;
    inv.containers[0].items[0].instance.grid_h = 1;
    // 在 main_hand 持双手杖（staff 派生 two-handed），锁住 off_hand。
    inv.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(ItemInstance {
            instance_id: 77,
            template_id: "wooden_staff".to_string(),
            display_name: "木杖".to_string(),
            grid_w: 1,
            grid_h: 3,
            weight: 1.2,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }),
    );

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Equip {
            slot: EquipSlotV1::OffHand,
            state: crate::schema::inventory::EquipStateV1::Held,
        },
        false,
    )
    .expect_err("off_hand should be locked by two-handed weapon in main_hand");

    assert!(
        matches!(error, InventoryMoveRejectReason::TwoHandedLocksOther),
        "期望对侧锁定拒绝，实际：{error:?}"
    );
}

#[test]
fn apply_move_rejects_weapon_to_hotbar() {
    use crate::schema::inventory::InventoryLocationV1;

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.template_id = "iron_sword".to_string();
    inv.containers[0].items[0].instance.display_name = "铁剑".to_string();
    inv.containers[0].items[0].instance.grid_h = 2;

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Hotbar { index: 0 },
        false,
    )
    .expect_err("weapon should be rejected from hotbar");

    assert!(matches!(
        error,
        InventoryMoveRejectReason::ForbiddenInHotbar {
            category: ItemCategory::Weapon
        }
    ));
}

#[test]
fn apply_move_rejects_tool_to_hotbar() {
    use crate::schema::inventory::InventoryLocationV1;

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.template_id = "cai_yao_dao".to_string();
    inv.containers[0].items[0].instance.display_name = "采药刀".to_string();

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Hotbar { index: 0 },
        false,
    )
    .expect_err("tool should be rejected from hotbar");

    assert!(matches!(
        error,
        InventoryMoveRejectReason::ForbiddenInHotbar {
            category: ItemCategory::Tool
        }
    ));
}

#[test]
fn apply_move_rejects_armor_to_hotbar() {
    use crate::schema::inventory::InventoryLocationV1;

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.template_id = "armor_bone_boots".to_string();
    inv.containers[0].items[0].instance.display_name = "骨甲靴".to_string();

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Hotbar { index: 0 },
        false,
    )
    .expect_err("armor should be rejected from hotbar");

    assert!(matches!(
        error,
        InventoryMoveRejectReason::ForbiddenInHotbar {
            category: ItemCategory::Armor
        }
    ));
}

// plan-shield-block-v1 P0 MAJOR #1 — 盾不能进 hotbar（Shield category 守卫回归）。
#[test]
fn apply_move_rejects_shield_to_hotbar() {
    use crate::schema::inventory::InventoryLocationV1;

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.template_id = "wooden_shield".to_string();
    inv.containers[0].items[0].instance.display_name = "木盾".to_string();
    inv.containers[0].items[0].instance.grid_h = 2;

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Hotbar { index: 0 },
        false,
    )
    .expect_err("shield should be rejected from hotbar");

    assert!(
        matches!(
            error,
            InventoryMoveRejectReason::ForbiddenInHotbar {
                category: ItemCategory::Shield
            }
        ),
        "期望盾牌不可进 hotbar 拒绝，实际：{error:?}"
    );
}

#[test]
fn apply_move_rejects_non_dagger_off_hand_weapon() {
    use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.template_id = "iron_sword".to_string();
    inv.containers[0].items[0].instance.display_name = "铁剑".to_string();
    inv.containers[0].items[0].instance.grid_h = 2;

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Equip {
            slot: EquipSlotV1::OffHand,
            state: crate::schema::inventory::EquipStateV1::Held,
        },
        false,
    )
    .expect_err("sword should be rejected from off_hand");

    assert!(matches!(
        error,
        InventoryMoveRejectReason::OffHandTypeMismatch
    ));
}

// plan-shield-block-v1 P0 MAJOR #2 — off_hand：无 weapon_spec 的非武器物品（armor）装 off_hand
// 被拒。plan-layered-equip-v1 统一手槽校验器后，错误消息为「expected weapon, tool, or hoe」
// （off_hand 仅额外放行 Treasure/Shield，Armor 不在其列），行为（拒绝）不变。
#[test]
fn apply_move_rejects_non_weapon_armor_to_off_hand() {
    use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    // armor_bone_boots：ItemCategory::Armor，无 weapon_spec → 走路径 a 被 ok_or_else 拒
    inv.containers[0].items[0].instance.template_id = "armor_bone_boots".to_string();
    inv.containers[0].items[0].instance.display_name = "骨甲靴".to_string();
    inv.containers[0].items[0].instance.grid_w = 1;
    inv.containers[0].items[0].instance.grid_h = 1;

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Equip {
            slot: EquipSlotV1::OffHand,
            state: crate::schema::inventory::EquipStateV1::Held,
        },
        false,
    )
    .expect_err("armor should be rejected from off_hand (no weapon_spec, not treasure/shield)");

    assert!(
        matches!(error, InventoryMoveRejectReason::EquipCategoryMismatch),
        "期望统一手槽校验器拒绝非武器/工具/锄头（Armor 不在 off_hand 额外放行的 \
         Treasure/Shield 之列），实际：{error:?}"
    );
}

// plan-layered-equip-v1 P0.2（决议 #7）— 两手兵器装入一手时，对侧手已被占用 → 拒绝
// （two_hand 槽已删，两手兵器入 main/off held 即锁对侧）。双手杖须装 main_hand
// （off_hand 仅收 dagger/fist，杖会先撞 dagger/fist 限制，无法触达双手锁分支）。
#[test]
fn apply_move_rejects_two_handed_weapon_when_opposite_hand_occupied() {
    use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    // 待装的双手杖（staff 派生 two-handed），目标 main_hand held。
    inv.containers[0].items[0].instance.template_id = "wooden_staff".to_string();
    inv.containers[0].items[0].instance.display_name = "木杖".to_string();
    inv.containers[0].items[0].instance.grid_h = 3;
    // off_hand 已持 dagger → 双手杖入 main_hand 时对侧（off_hand）被占用，应拒。
    inv.equipped.insert(
        EQUIP_SLOT_OFF_HAND.to_string(),
        SlotContents::held_single(ItemInstance {
            instance_id: 77,
            template_id: "bone_dagger".to_string(),
            display_name: "骨刀".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.5,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }),
    );

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Equip {
            slot: EquipSlotV1::MainHand,
            state: crate::schema::inventory::EquipStateV1::Held,
        },
        false,
    )
    .expect_err("two-handed weapon should conflict with occupied opposite hand");

    // 命中双手兵器对侧锁：对侧 off_hand 已被 dagger 占用。
    assert!(
        matches!(error, InventoryMoveRejectReason::TwoHandedLocksOther),
        "期望双手兵器对侧占用拒绝，实际：{error:?}"
    );
}

// ============================================================================
// plan-layered-equip-v1 PR-2 / P1 — 装备校验分层规则 state transition 饱和化
// （worn cap 满拒 / 被压层拒 / held 占拒 / 锁手拒 / worn+held 共存 / 卸顶后下层成新顶 /
//  双手占双手 / extra_hand 不锁 / 非双手不锁）。
// P0 (#736) 已落地 `validate_equip_to` 逻辑；本块锁住每条 state transition 防回归。
// ============================================================================

/// 紧凑构造一个装备/校验测试用的 `ItemInstance`（仅设关键字段）。
fn equip_test_instance(instance_id: u64, template_id: &str) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: template_id.to_string(),
        display_name: template_id.to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 1.0,
        rarity: ItemRarity::Common,
        description: String::new(),
        stack_count: 1,
        spirit_quality: 1.0,
        durability: 1.0,
        freshness: None,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    }
}

/// 直接断言 `validate_move_semantics`：把 `item`（不依赖 inventory 实存）从 `from` 移到
/// `to` 应通过 / 拒绝。让我们逐条锁 worn cap / LIFO / held 互斥分支，不必走整条
/// apply_inventory_move（后者改 inventory，破坏 multi-step 断言）。
fn validate_equip_result(
    registry: &ItemRegistry,
    inventory: &PlayerInventory,
    item: &ItemInstance,
    from: &crate::schema::inventory::InventoryLocationV1,
    to: &crate::schema::inventory::InventoryLocationV1,
) -> Result<(), InventoryMoveRejectReason> {
    validate_move_semantics(registry, inventory, item, from, to)
}

fn container_from() -> crate::schema::inventory::InventoryLocationV1 {
    crate::schema::inventory::InventoryLocationV1::Container {
        container_id: MAIN_PACK_CONTAINER_ID.to_string(),
        row: 0,
        col: 0,
    }
}

fn equip_to(
    slot: crate::schema::inventory::EquipSlotV1,
    state: crate::schema::inventory::EquipStateV1,
) -> crate::schema::inventory::InventoryLocationV1 {
    crate::schema::inventory::InventoryLocationV1::Equip { slot, state }
}

// ---- worn cap 满 → 拒绝（决议 #3 拒绝不顶替）----

#[test]
fn validate_chest_worn_cap_full_at_three_rejects_fourth_armor() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = load_item_registry().expect("registry");
    let mut inv = make_test_inventory_with_one_item();
    // chest cap = 3：填满 3 件胸甲（不同材质，均映射 Chest 槽）。
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents {
            worn: vec![
                equip_test_instance(201, "armor_straw_chestplate"),
                equip_test_instance(202, "armor_bone_chestplate"),
                equip_test_instance(203, "armor_iron_chestplate"),
            ],
            held: None,
        },
    );
    let fourth = equip_test_instance(204, "armor_bone_chestplate");
    let error = validate_equip_result(
        &registry,
        &inv,
        &fourth,
        &container_from(),
        &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
    )
    .expect_err("chest worn cap is 3; the 4th armor must be rejected");
    assert!(
        matches!(
            error,
            InventoryMoveRejectReason::WornCapFull { ref slot, cap }
                if slot.as_str() == EQUIP_SLOT_CHEST && cap == 3
        ),
        "期望 chest 满 3 层拒绝，实际：{error:?}"
    );
}

#[test]
fn validate_chest_worn_below_cap_accepts_third_armor() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = load_item_registry().expect("registry");
    let mut inv = make_test_inventory_with_one_item();
    // chest 已 2 件 → 第 3 件合法（cap=3，边界 off-by-one 正向）。
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents {
            worn: vec![
                equip_test_instance(201, "armor_straw_chestplate"),
                equip_test_instance(202, "armor_bone_chestplate"),
            ],
            held: None,
        },
    );
    let third = equip_test_instance(203, "armor_iron_chestplate");
    validate_equip_result(
        &registry,
        &inv,
        &third,
        &container_from(),
        &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
    )
    .expect("chest worn cap is 3; the 3rd armor must be accepted");
}

#[test]
fn validate_head_worn_cap_full_at_two_rejects_third_armor() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = load_item_registry().expect("registry");
    let mut inv = make_test_inventory_with_one_item();
    // head cap = 2：填满 2 件头盔。
    inv.equipped.insert(
        EQUIP_SLOT_HEAD.to_string(),
        SlotContents {
            worn: vec![
                equip_test_instance(301, "armor_straw_helmet"),
                equip_test_instance(302, "armor_bone_helmet"),
            ],
            held: None,
        },
    );
    let third = equip_test_instance(303, "armor_bone_helmet");
    let error = validate_equip_result(
        &registry,
        &inv,
        &third,
        &container_from(),
        &equip_to(EquipSlotV1::Head, EquipStateV1::Worn),
    )
    .expect_err("head worn cap is 2; the 3rd helmet must be rejected");
    assert!(
        matches!(
            error,
            InventoryMoveRejectReason::WornCapFull { ref slot, cap }
                if slot.as_str() == EQUIP_SLOT_HEAD && cap == 2
        ),
        "期望 head 满 2 层拒绝，实际：{error:?}"
    );
}

#[test]
fn validate_feet_worn_cap_full_at_two_rejects_third_armor() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = load_item_registry().expect("registry");
    let mut inv = make_test_inventory_with_one_item();
    // feet cap = 2：填满 2 件靴。
    inv.equipped.insert(
        EQUIP_SLOT_FEET.to_string(),
        SlotContents {
            worn: vec![
                equip_test_instance(401, "armor_straw_boots"),
                equip_test_instance(402, "armor_bone_boots"),
            ],
            held: None,
        },
    );
    let third = equip_test_instance(403, "armor_bone_boots");
    let error = validate_equip_result(
        &registry,
        &inv,
        &third,
        &container_from(),
        &equip_to(EquipSlotV1::Feet, EquipStateV1::Worn),
    )
    .expect_err("feet worn cap is 2; the 3rd boots must be rejected");
    assert!(
        matches!(
            error,
            InventoryMoveRejectReason::WornCapFull { ref slot, cap }
                if slot.as_str() == EQUIP_SLOT_FEET && cap == 2
        ),
        "期望 feet 满 2 层拒绝，实际：{error:?}"
    );
}

// ---- 背包件与盔甲同槽 cap 共算（决议 #17：背包占身体槽 worn 层）----

#[test]
fn validate_chest_cap_shared_between_armor_and_backpack() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = load_item_registry().expect("registry");
    let mut inv = make_test_inventory_with_one_item();
    // chest 已 2 件甲 → 拖入背包件（worn_grass_pouch，equip_slot=chest）作第 3 件合法。
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents {
            worn: vec![
                equip_test_instance(501, "armor_straw_chestplate"),
                equip_test_instance(502, "armor_bone_chestplate"),
            ],
            held: None,
        },
    );
    let pack = equip_test_instance(503, "worn_grass_pouch");
    validate_equip_result(
        &registry,
        &inv,
        &pack,
        &container_from(),
        &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
    )
    .expect("backpack as 3rd worn layer shares chest cap (cap=3) and must be accepted");

    // 再补满到 3 后，第 4 件（无论甲还是包）拒绝——cap 与盔甲/伪皮共算。
    inv.equipped
        .get_mut(EQUIP_SLOT_CHEST)
        .unwrap()
        .worn
        .push(equip_test_instance(503, "worn_grass_pouch"));
    let fourth = equip_test_instance(504, "grass_pouch");
    let error = validate_equip_result(
        &registry,
        &inv,
        &fourth,
        &container_from(),
        &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
    )
    .expect_err("chest cap=3 shared with backpack; the 4th item must be rejected");
    assert!(
        matches!(
            error,
            InventoryMoveRejectReason::WornCapFull { ref slot, cap }
                if slot.as_str() == EQUIP_SLOT_CHEST && cap == 3
        ),
        "期望 chest 共算满 3 层拒绝，实际：{error:?}"
    );
}

// ---- held 互斥（决议 #3：手槽已持械拒绝，卸下才换）----

#[test]
fn validate_held_mutex_rejects_second_weapon_to_occupied_main_hand() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = load_item_registry().expect("registry");
    let mut inv = make_test_inventory_with_one_item();
    inv.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(equip_test_instance(601, "iron_sword")),
    );
    let second = equip_test_instance(602, "iron_sword");
    let error = validate_equip_result(
        &registry,
        &inv,
        &second,
        &container_from(),
        &equip_to(EquipSlotV1::MainHand, EquipStateV1::Held),
    )
    .expect_err("main_hand already held; second weapon must be rejected (no swap)");
    assert!(
        matches!(error, InventoryMoveRejectReason::HandOccupied),
        "期望 held 互斥拒绝，实际：{error:?}"
    );
}

// ---- 双手武器锁对侧手：off_hand→main_hand 反向（补 main→off 之外的方向）----
// 注：Spear 派生双手由 `weapon_two_handed_per_kind` 单测锁（资产暂无 spear 模板，
// 实物双手锁集成测用 staff）。本例验证「双手在 off_hand 时反向锁 main_hand」。

#[test]
fn validate_two_handed_in_off_hand_locks_main_hand() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = load_item_registry().expect("registry");
    let mut inv = make_test_inventory_with_one_item();
    // off_hand 持双手杖（staff 派生双手）→ main_hand 被反向锁。
    inv.equipped.insert(
        EQUIP_SLOT_OFF_HAND.to_string(),
        SlotContents::held_single(equip_test_instance(701, "wooden_staff")),
    );
    // 往 main_hand 拖剑，应被对侧（off_hand）双手锁挡住。
    let sword = equip_test_instance(702, "iron_sword");
    let error = validate_equip_result(
        &registry,
        &inv,
        &sword,
        &container_from(),
        &equip_to(EquipSlotV1::MainHand, EquipStateV1::Held),
    )
    .expect_err("two-handed staff in off_hand must lock main_hand (reverse direction)");
    assert!(
        matches!(error, InventoryMoveRejectReason::TwoHandedLocksOther),
        "期望反向双手锁拒绝，实际：{error:?}"
    );
}

// ---- 非双手武器不锁对侧手 ----

#[test]
fn validate_one_handed_sword_does_not_lock_off_hand() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = load_item_registry().expect("registry");
    let mut inv = make_test_inventory_with_one_item();
    // main_hand 持单手剑（Sword 非双手）→ off_hand 不锁。
    inv.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(equip_test_instance(801, "iron_sword")),
    );
    let dagger = equip_test_instance(802, "bone_dagger");
    validate_equip_result(
        &registry,
        &inv,
        &dagger,
        &container_from(),
        &equip_to(EquipSlotV1::OffHand, EquipStateV1::Held),
    )
    .expect("single-handed sword must NOT lock off_hand; dagger to off_hand should pass");
}

// ---- extra_hand 独立不受双手锁（决议 #6/#7：多臂额外手）----

#[test]
fn validate_two_handed_main_hand_does_not_lock_extra_hand() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = load_item_registry().expect("registry");
    let mut inv = make_test_inventory_with_one_item();
    // main_hand 持双手杖 → off_hand 被锁，但 extra_hand_0 不受锁。
    inv.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(equip_test_instance(901, "wooden_staff")),
    );
    let tool = equip_test_instance(902, "bone_dagger");
    validate_equip_result(
        &registry,
        &inv,
        &tool,
        &container_from(),
        &equip_to(EquipSlotV1::ExtraHand0, EquipStateV1::Held),
    )
    .expect("extra_hand_0 is an independent multi-arm slot; two-handed weapon must NOT lock it");
}

// ---- worn + held 共存：身体槽 worn 满 + 手槽 held 一件并存合法 ----

#[test]
fn validate_worn_and_held_coexist_in_separate_slots() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = load_item_registry().expect("registry");
    let mut inv = make_test_inventory_with_one_item();
    // chest worn 已满 3 件 + main_hand 已 held 一把剑 —— 互不干扰。
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents {
            worn: vec![
                equip_test_instance(1001, "armor_straw_chestplate"),
                equip_test_instance(1002, "armor_bone_chestplate"),
                equip_test_instance(1003, "armor_iron_chestplate"),
            ],
            held: None,
        },
    );
    inv.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(equip_test_instance(1004, "iron_sword")),
    );
    // off_hand 仍空 → 拖入 dagger 合法（worn 满不影响其它槽 held）。
    let dagger = equip_test_instance(1005, "bone_dagger");
    validate_equip_result(
        &registry,
        &inv,
        &dagger,
        &container_from(),
        &equip_to(EquipSlotV1::OffHand, EquipStateV1::Held),
    )
    .expect("full chest worn + main_hand held must not block off_hand held");
}

// ---- 卸下后可再装：held 卸下 → 同手可装新 held ----

#[test]
fn validate_rehome_held_then_equip_new_held_succeeds() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
    let registry = load_item_registry().expect("registry");
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items.clear();
    inv.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(equip_test_instance(1101, "iron_sword")),
    );
    // 卸下 main_hand 武器（rehome 到容器）。
    move_equipped_item_to_first_container_slot(&mut inv, 1101)
        .expect("held weapon should unequip and rehome");
    assert!(
        inv.equipped
            .get(EQUIP_SLOT_MAIN_HAND)
            .map(|s| s.held.is_none())
            .unwrap_or(true),
        "卸下后 main_hand.held 应为空"
    );
    // 卸下后同手可装新武器。
    let new_sword = equip_test_instance(1102, "iron_sword");
    validate_equip_result(
        &registry,
        &inv,
        &new_sword,
        &InventoryLocationV1::Container {
            container_id: inv.containers[0].id.clone(),
            row: 0,
            col: 0,
        },
        &equip_to(EquipSlotV1::MainHand, EquipStateV1::Held),
    )
    .expect("after unequip, main_hand is free and a new weapon must be accepted");
}

// ============================================================================
// plan-race-system-v1 P3b（决议 §8.1 #5）—— 装备门 race gate 饱和测试。
// 判定域用 **Form 身份**（`validate_move_semantics_with_race` 的
// `form_race_id`/`form_is_humanoid` 参数），不是本体；`validate_move_semantics`
// （无参老签名）恒用人类/人形身份，等价于 gate 恒放行的默认路径。
// ============================================================================

/// 构造一件挂 `wearer_race` 门的护甲（沿用 `armor_straw_chestplate` 真实 item_id
/// 让 `equip_slot_for_item_id` 正确解析出 Chest 槽，只替换 `wearer_race`）。
fn make_race_gated_armor_template(wearer_race: RaceGateOwned) -> ItemTemplate {
    ItemTemplate {
        id: "armor_straw_chestplate".to_string(),
        display_name: "race-gated chestplate".to_string(),
        category: ItemCategory::Armor,
        placeable: None,
        max_stack_count: 1,
        grid_w: 1,
        grid_h: 1,
        base_weight: 1.0,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 0.0,
        description: "test".to_string(),
        effect: None,
        cast_duration_ms: DEFAULT_CAST_DURATION_MS,
        cooldown_ms: DEFAULT_COOLDOWN_MS,
        weapon_spec: None,
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
        readable_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: None,
        shield_spec: None,
        shelflife_profile: None,
        shelflife_track: None,
        wearer_race,
    }
}

/// 构造一件挂 `wearer_race` 门的武器（手槽，`is_hand_slot` 分支）。
fn make_race_gated_weapon_template(wearer_race: RaceGateOwned) -> ItemTemplate {
    ItemTemplate {
        id: "race_gated_sword".to_string(),
        display_name: "race-gated sword".to_string(),
        category: ItemCategory::Weapon,
        placeable: None,
        max_stack_count: 1,
        grid_w: 1,
        grid_h: 1,
        base_weight: 1.0,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 0.0,
        description: "test".to_string(),
        effect: None,
        cast_duration_ms: DEFAULT_CAST_DURATION_MS,
        cooldown_ms: DEFAULT_COOLDOWN_MS,
        weapon_spec: Some(WeaponSpec {
            weapon_kind: crate::combat::weapon::WeaponKind::Sword,
            base_attack: 1.0,
            quality_tier: 0,
            durability_max: 100.0,
            qi_cost_mul: 1.0,
        }),
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
        readable_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: None,
        shield_spec: None,
        shelflife_profile: None,
        shelflife_track: None,
        wearer_race,
    }
}

#[test]
fn race_gate_any_allows_any_form_identity() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = ItemRegistry::from_map(HashMap::from([(
        "armor_straw_chestplate".to_string(),
        make_race_gated_armor_template(RaceGateOwned::Any),
    )]));
    let inv = make_test_inventory_with_one_item();
    let item = equip_test_instance(1, "armor_straw_chestplate");
    let whale = RaceId::new("whale");
    validate_move_semantics_with_race(
        &registry,
        &inv,
        &item,
        &container_from(),
        &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
        &whale,
        false,
    )
    .expect("wearer_race=Any 恒放行，与 form 身份无关（非人形 whale 也应通过）");
}

#[test]
fn race_gate_humanoid_allows_humanoid_form_regardless_of_race_id() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = ItemRegistry::from_map(HashMap::from([(
        "armor_straw_chestplate".to_string(),
        make_race_gated_armor_template(RaceGateOwned::Humanoid),
    )]));
    let inv = make_test_inventory_with_one_item();
    let item = equip_test_instance(1, "armor_straw_chestplate");
    // §8.1 #6 反例：两个不同 RaceId 共享同一 humanoid BodyPlan——Humanoid 档
    // 判 is_humanoid，不认种族名单，故 "human_variant" 也应放行。
    let human_variant = RaceId::new("human_variant");
    validate_move_semantics_with_race(
        &registry,
        &inv,
        &item,
        &container_from(),
        &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
        &human_variant,
        true,
    )
    .expect("wearer_race=Humanoid 且 form_is_humanoid=true 应放行（不看 race_id 名单）");
}

#[test]
fn race_gate_humanoid_rejects_non_humanoid_form() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = ItemRegistry::from_map(HashMap::from([(
        "armor_straw_chestplate".to_string(),
        make_race_gated_armor_template(RaceGateOwned::Humanoid),
    )]));
    let inv = make_test_inventory_with_one_item();
    let item = equip_test_instance(1, "armor_straw_chestplate");
    let whale = RaceId::new("whale");
    let error = validate_move_semantics_with_race(
        &registry,
        &inv,
        &item,
        &container_from(),
        &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
        &whale,
        false,
    )
    .expect_err("wearer_race=Humanoid 且 form_is_humanoid=false 必须拒绝");
    assert!(
        matches!(error, InventoryMoveRejectReason::RaceMismatch),
        "期望 RaceMismatch，实际：{error:?}"
    );
}

#[test]
fn race_gate_species_allows_matching_race_id() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = ItemRegistry::from_map(HashMap::from([(
        "armor_straw_chestplate".to_string(),
        make_race_gated_armor_template(RaceGateOwned::Species {
            species: vec![RaceId::new("whale")],
        }),
    )]));
    let inv = make_test_inventory_with_one_item();
    let item = equip_test_instance(1, "armor_straw_chestplate");
    let whale = RaceId::new("whale");
    validate_move_semantics_with_race(
        &registry,
        &inv,
        &item,
        &container_from(),
        &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
        &whale,
        false,
    )
    .expect("wearer_race=Species([whale]) 且 form_race_id=whale 应放行");
}

#[test]
fn race_gate_species_rejects_non_matching_race_id_even_if_humanoid() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = ItemRegistry::from_map(HashMap::from([(
        "armor_straw_chestplate".to_string(),
        make_race_gated_armor_template(RaceGateOwned::Species {
            species: vec![RaceId::new("whale")],
        }),
    )]));
    let inv = make_test_inventory_with_one_item();
    let item = equip_test_instance(1, "armor_straw_chestplate");
    // Species 档精确匹配 race_id；哪怕 form_is_humanoid=true 也不放行人类。
    let human = RaceId::new(HUMAN_RACE_ID);
    let error = validate_move_semantics_with_race(
        &registry,
        &inv,
        &item,
        &container_from(),
        &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
        &human,
        true,
    )
    .expect_err("wearer_race=Species([whale]) 且 form_race_id=human 必须拒绝");
    assert!(
        matches!(error, InventoryMoveRejectReason::RaceMismatch),
        "期望 RaceMismatch，实际：{error:?}"
    );
}

#[test]
fn race_gate_applies_to_hand_slot_weapons_too() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = ItemRegistry::from_map(HashMap::from([(
        "race_gated_sword".to_string(),
        make_race_gated_weapon_template(RaceGateOwned::Humanoid),
    )]));
    let inv = make_test_inventory_with_one_item();
    let item = equip_test_instance(1, "race_gated_sword");
    let whale = RaceId::new("whale");
    let error = validate_move_semantics_with_race(
        &registry,
        &inv,
        &item,
        &container_from(),
        &equip_to(EquipSlotV1::MainHand, EquipStateV1::Held),
        &whale,
        false,
    )
    .expect_err("手槽（武器）同样受 wearer_race 约束，非人形 whale 应拒绝");
    assert!(
        matches!(error, InventoryMoveRejectReason::RaceMismatch),
        "期望 RaceMismatch，实际：{error:?}"
    );
}

#[test]
fn race_gate_default_no_race_signature_uses_humanoid_human_identity() {
    // `validate_move_semantics`（P3b 前既有老签名）套上默认人类/人形身份；
    // Humanoid 档在该默认身份下应恒放行——既有海量调用点不改行为的关键 pin。
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = ItemRegistry::from_map(HashMap::from([(
        "armor_straw_chestplate".to_string(),
        make_race_gated_armor_template(RaceGateOwned::Humanoid),
    )]));
    let inv = make_test_inventory_with_one_item();
    let item = equip_test_instance(1, "armor_straw_chestplate");
    validate_equip_result(
        &registry,
        &inv,
        &item,
        &container_from(),
        &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
    )
    .expect("老签名默认人类/人形身份，Humanoid 档应放行，行为与 P3b 前一致");
}

#[test]
fn race_gate_is_checked_after_existing_slot_validations_not_before() {
    // 顺序回归：race gate 是"槽位分支判定后、Ok(()) 前"的最后一道闸——
    // 护甲耐久 0（既有更早分支）与 race mismatch 同时触发时，必须报告
    // ArmorDurabilityZero（既有校验优先），不是 RaceMismatch。
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = ItemRegistry::from_map(HashMap::from([(
        "armor_straw_chestplate".to_string(),
        make_race_gated_armor_template(RaceGateOwned::Species {
            species: vec![RaceId::new("whale")],
        }),
    )]));
    let inv = make_test_inventory_with_one_item();
    let mut item = equip_test_instance(1, "armor_straw_chestplate");
    item.durability = 0.0;
    let human = RaceId::new(HUMAN_RACE_ID);
    let error = validate_move_semantics_with_race(
        &registry,
        &inv,
        &item,
        &container_from(),
        &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
        &human,
        true,
    )
    .expect_err("耐久 0 + race mismatch 同时命中，既有校验应优先触发");
    assert!(
        matches!(error, InventoryMoveRejectReason::ArmorDurabilityZero),
        "期望既有 ArmorDurabilityZero 优先于 race gate，实际：{error:?}"
    );
}

// ============================================================================
// plan-layered-equip-v1 PR-2 / P1 — worn 栈 LIFO（决议 #12：仅栈顶可卸下）
// ============================================================================

#[test]
fn validate_move_worn_top_layer_out_succeeds() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = load_item_registry().expect("registry");
    let mut inv = make_test_inventory_with_one_item();
    // chest worn = [底甲 1201, 顶甲 1202]；移出栈顶 1202 → 合法。
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents {
            worn: vec![
                equip_test_instance(1201, "armor_bone_chestplate"),
                equip_test_instance(1202, "armor_iron_chestplate"),
            ],
            held: None,
        },
    );
    let top = equip_test_instance(1202, "armor_iron_chestplate");
    validate_equip_result(
        &registry,
        &inv,
        &top,
        &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
        &container_from(),
    )
    .expect("moving the worn stack top (worn.last()) out must be allowed");
}

#[test]
fn validate_move_buried_worn_layer_out_rejected() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    let registry = load_item_registry().expect("registry");
    let mut inv = make_test_inventory_with_one_item();
    // chest worn = [底甲 1301, 顶甲 1302]；移出被压住的底层 1301 → 拒绝。
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents {
            worn: vec![
                equip_test_instance(1301, "armor_bone_chestplate"),
                equip_test_instance(1302, "armor_iron_chestplate"),
            ],
            held: None,
        },
    );
    let buried = equip_test_instance(1301, "armor_bone_chestplate");
    let error = validate_equip_result(
        &registry,
        &inv,
        &buried,
        &equip_to(EquipSlotV1::Chest, EquipStateV1::Worn),
        &container_from(),
    )
    .expect_err("moving a buried worn layer (not worn.last()) must be rejected");
    assert!(
        matches!(error, InventoryMoveRejectReason::WornStackNotTop),
        "期望被压层 LIFO 拒绝，实际：{error:?}"
    );
}

#[test]
fn move_equipped_top_worn_layer_succeeds_buried_rejected_then_new_top() {
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items.clear();
    // chest worn = [底甲 1401, 中甲 1402, 顶甲 1403]。
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents {
            worn: vec![
                equip_test_instance(1401, "armor_straw_chestplate"),
                equip_test_instance(1402, "armor_bone_chestplate"),
                equip_test_instance(1403, "armor_iron_chestplate"),
            ],
            held: None,
        },
    );

    // 脱被压住的底层（1401）→ 拒绝。
    let err = move_equipped_item_to_first_container_slot(&mut inv, 1401)
        .expect_err("buried bottom layer must not be removable");
    assert!(err.contains("被上层压住"), "期望底层被压拒绝，实际：{err}");

    // 脱栈顶（1403）→ 成功，剩 [1401, 1402]，1402 成新顶。
    move_equipped_item_to_first_container_slot(&mut inv, 1403)
        .expect("stack top must be removable");
    let worn = &inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn;
    assert_eq!(
        worn.iter().map(|i| i.instance_id).collect::<Vec<_>>(),
        vec![1401, 1402],
        "脱顶后 worn 应剩底+中两件，顶层移除"
    );

    // 脱新顶（1402）→ 成功，剩 [1401]，1401 成新顶。
    move_equipped_item_to_first_container_slot(&mut inv, 1402)
        .expect("the new stack top (former middle layer) must now be removable");
    let worn = &inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn;
    assert_eq!(
        worn.iter().map(|i| i.instance_id).collect::<Vec<_>>(),
        vec![1401],
        "脱新顶后 worn 应只剩底层（曾被压住，现成新顶）"
    );

    // 脱最后一层（1401，现唯一一层即栈顶）→ 成功，chest 槽清空。
    move_equipped_item_to_first_container_slot(&mut inv, 1401)
        .expect("last remaining worn layer is the top and must be removable");
    assert!(
        inv.equipped
            .get(EQUIP_SLOT_CHEST)
            .map(|s| s.worn.is_empty())
            .unwrap_or(true),
        "脱完所有层后 chest worn 应为空"
    );
}

#[test]
fn set_item_instance_durability_updates_equipped_item_and_bumps_revision() {
    let mut inv = make_test_inventory_with_one_item();
    inv.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(ItemInstance {
            instance_id: 88,
            template_id: "iron_sword".to_string(),
            display_name: "铁剑".to_string(),
            grid_w: 1,
            grid_h: 2,
            weight: 1.2,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }),
    );

    let update =
        set_item_instance_durability(&mut inv, 88, 0.25).expect("durability update should succeed");

    assert_eq!(update.revision, InventoryRevision(8));
    assert_eq!(
        inv.equipped[EQUIP_SLOT_MAIN_HAND]
            .held
            .as_ref()
            .unwrap()
            .durability,
        0.25
    );
}

#[test]
fn move_equipped_item_to_first_container_slot_unequips_and_rehomes_item() {
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items.clear();
    inv.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(ItemInstance {
            instance_id: 88,
            template_id: "iron_sword".to_string(),
            display_name: "铁剑".to_string(),
            grid_w: 1,
            grid_h: 2,
            weight: 1.2,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 0.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }),
    );

    let outcome = move_equipped_item_to_first_container_slot(&mut inv, 88)
        .expect("broken weapon should move back to container");

    assert_eq!(
        outcome,
        InventoryMoveOutcome::Moved {
            revision: InventoryRevision(8)
        }
    );
    assert!(
        inv.equipped
            .get(EQUIP_SLOT_MAIN_HAND)
            .map(|s| s.is_empty())
            .unwrap_or(true),
        "解装后 main_hand 应为空（held=None）"
    );
    assert_eq!(inv.containers[0].items.len(), 1);
    assert_eq!(inv.containers[0].items[0].instance.instance_id, 88);
}

#[test]
fn consume_item_instance_once_decrements_stack_and_bumps_revision() {
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.stack_count = 3;

    let out = consume_item_instance_once(&mut inv, 42).expect("consume should succeed");

    assert_eq!(out.remaining_stack, 2);
    assert_eq!(out.revision, InventoryRevision(8));
    assert_eq!(inv.containers[0].items[0].instance.stack_count, 2);
}

#[test]
fn consume_item_instance_once_removes_last_stack_and_bumps_revision() {
    let mut inv = make_test_inventory_with_one_item();

    let out = consume_item_instance_once(&mut inv, 42).expect("consume should succeed");

    assert_eq!(out.remaining_stack, 0);
    assert_eq!(out.revision, InventoryRevision(8));
    assert!(inv.containers[0].items.is_empty());
}

// ── plan-forge-session-entry-wiring-v1 §4.1#4 — consume_forge_materials_atomic ──

fn mineral_item(instance_id: u64, mineral_id: &str, stack_count: u32) -> ItemInstance {
    let mut item = make_test_item_instance(instance_id, &format!("mineral_{mineral_id}"));
    item.mineral_id = Some(mineral_id.to_string());
    item.stack_count = stack_count;
    item
}

fn item_material_item(instance_id: u64, template_id: &str, stack_count: u32) -> ItemInstance {
    let mut item = make_test_item_instance(instance_id, template_id);
    item.stack_count = stack_count;
    item
}

#[test]
fn consume_forge_materials_atomic_happy_path_deducts_by_mineral_id_and_bumps_revision() {
    let mut inv = empty_inventory(5, 7);
    inv.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: mineral_item(1, "fan_tie", 5),
    });

    let out = consume_forge_materials_atomic(&mut inv, &[("fan_tie".to_string(), 4)]);

    assert!(
        out.is_ok(),
        "期望持有 5 扣 4 成功，实际 Err={:?}",
        out.err()
    );
    assert_eq!(
        inv.containers[0].items[0].instance.stack_count, 1,
        "扣除 4 后 fan_tie 栈应剩 1"
    );
    assert_eq!(
        inv.revision,
        InventoryRevision(1),
        "扣料应 bump_revision 恰好一次"
    );
}

#[test]
fn consume_forge_materials_atomic_matches_by_template_id_for_non_mineral_materials() {
    // ling_mu_gun 是 blueprint::is_allowed_item_material 白名单里的非矿物锻造用料，
    // 匹配键是 template_id 而非 mineral_id（对应物品本身没有 mineral_id）。
    let mut inv = empty_inventory(5, 7);
    inv.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: item_material_item(1, "ling_mu_gun", 2),
    });

    let out = consume_forge_materials_atomic(&mut inv, &[("ling_mu_gun".to_string(), 2)]);

    assert!(out.is_ok(), "期望按 template_id 精确匹配并扣光");
    assert!(
        inv.containers[0].items.is_empty(),
        "扣光后栈应被移除，而非留 0 计数条目"
    );
}

#[test]
fn consume_forge_materials_atomic_spans_multiple_stacks_containers_then_hotbar() {
    let mut inv = empty_inventory(5, 7);
    inv.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: mineral_item(1, "fan_tie", 2),
    });
    inv.hotbar[0] = Some(mineral_item(2, "fan_tie", 3));

    let out = consume_forge_materials_atomic(&mut inv, &[("fan_tie".to_string(), 4)]);

    assert!(out.is_ok(), "跨 container+hotbar 累加应足量 2+3=5 >= 4");
    assert!(
        inv.containers[0].items.is_empty(),
        "container 栈应先被吃光（container 优先于 hotbar）"
    );
    assert_eq!(
        inv.hotbar[0].as_ref().map(|i| i.stack_count),
        Some(1),
        "hotbar 兜底吃剩余 2 个，应剩 3-2=1"
    );
}

#[test]
fn consume_forge_materials_atomic_insufficient_leaves_inventory_untouched() {
    let mut inv = empty_inventory(5, 7);
    inv.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: mineral_item(1, "fan_tie", 2),
    });
    let before_json = serde_json::to_string(&inv).unwrap();

    let err = consume_forge_materials_atomic(&mut inv, &[("fan_tie".to_string(), 4)])
        .expect_err("持有 2 < 需求 4 应该拒绝");

    assert_eq!(
        err,
        vec![ForgeMaterialDeficit {
            material: "fan_tie".to_string(),
            have: 2,
            need: 4,
        }]
    );
    assert_eq!(
        serde_json::to_string(&inv).unwrap(),
        before_json,
        "拒绝路径必须整体零改动（含 revision 不变），不能吞料"
    );
}

#[test]
fn consume_forge_materials_atomic_partial_shortage_leaves_sufficient_material_untouched_too() {
    // 原子性核心断言：即便第一个材料 fan_tie 足量，只要第二个材料 za_gang 不足，
    // 整批（含已足量的 fan_tie）都不得被扣——否则引擎下一拍拒绝时吞掉 fan_tie。
    let mut inv = empty_inventory(5, 7);
    inv.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: mineral_item(1, "fan_tie", 4),
    });
    inv.containers[0].items.push(PlacedItemState {
        row: 1,
        col: 0,
        instance: mineral_item(2, "za_gang", 0),
    });
    let before_json = serde_json::to_string(&inv).unwrap();

    let err = consume_forge_materials_atomic(
        &mut inv,
        &[("fan_tie".to_string(), 4), ("za_gang".to_string(), 1)],
    )
    .expect_err("za_gang 持有 0 < 需求 1 应该整体拒绝");

    assert_eq!(
        err,
        vec![ForgeMaterialDeficit {
            material: "za_gang".to_string(),
            have: 0,
            need: 1,
        }]
    );
    assert_eq!(
        serde_json::to_string(&inv).unwrap(),
        before_json,
        "fan_tie 已足量也不得被单独扣除——必须与 za_gang 同批要么全扣要么全不扣"
    );
}

#[test]
fn consume_forge_materials_atomic_ignores_equipped_items() {
    // equipped 槽里的东西不该被当材料吃掉：即使 held 位塞了一把 mineral_id=fan_tie
    // 的道具，也不计入持有量、更不会被扣除。
    let mut inv = empty_inventory(5, 7);
    inv.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(mineral_item(9, "fan_tie", 5)),
    );

    let err = consume_forge_materials_atomic(&mut inv, &[("fan_tie".to_string(), 1)])
        .expect_err("equipped 持有量不算数，应视为 have=0 拒绝");

    assert_eq!(
        err,
        vec![ForgeMaterialDeficit {
            material: "fan_tie".to_string(),
            have: 0,
            need: 1,
        }]
    );
    assert_eq!(
        inv.equipped[EQUIP_SLOT_MAIN_HAND]
            .held
            .as_ref()
            .unwrap()
            .stack_count,
        5,
        "equipped 物品不应被扣除"
    );
}

#[test]
fn consume_forge_materials_atomic_zero_count_is_noop() {
    let mut inv = empty_inventory(5, 7);
    inv.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: mineral_item(1, "fan_tie", 5),
    });

    let out = consume_forge_materials_atomic(&mut inv, &[("fan_tie".to_string(), 0)]);

    assert!(out.is_ok(), "count=0 不应被判定为缺料");
    assert_eq!(inv.containers[0].items[0].instance.stack_count, 5);
    assert_eq!(
        inv.revision,
        InventoryRevision(0),
        "count=0 不产生任何实际扣除，不应 bump_revision"
    );
}

#[test]
fn consume_forge_materials_atomic_empty_materials_is_noop() {
    let mut inv = empty_inventory(5, 7);
    let before_json = serde_json::to_string(&inv).unwrap();

    let out = consume_forge_materials_atomic(&mut inv, &[]);

    assert!(out.is_ok(), "空 materials 列表应视为 vacuously 满足");
    assert_eq!(serde_json::to_string(&inv).unwrap(), before_json);
}

#[test]
fn consume_forge_materials_atomic_dedupes_repeated_material_entries() {
    // materials 里同一材料出现两次（例如 client 端未合并），应按总量核对+扣除。
    let mut inv = empty_inventory(5, 7);
    inv.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: mineral_item(1, "fan_tie", 5),
    });

    let out = consume_forge_materials_atomic(
        &mut inv,
        &[("fan_tie".to_string(), 2), ("fan_tie".to_string(), 3)],
    );

    assert!(out.is_ok(), "2+3=5 应与持有量 5 恰好相抵");
    assert!(
        inv.containers[0].items.is_empty(),
        "去重累加后总需求=5，应扣光整栈"
    );
}

#[test]
fn consume_forge_materials_atomic_exact_have_equals_need_boundary() {
    let mut inv = empty_inventory(5, 7);
    inv.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: mineral_item(1, "fan_tie", 3),
    });

    let out = consume_forge_materials_atomic(&mut inv, &[("fan_tie".to_string(), 3)]);

    assert!(out.is_ok(), "have==need 边界应视为足量而非不足");
    assert!(inv.containers[0].items.is_empty());
}

#[test]
fn exchange_inventory_items_swaps_items_and_bumps_both_revisions() {
    let mut left = make_test_inventory_with_one_item();
    let mut right = make_test_inventory_with_one_item();
    right.revision = InventoryRevision(3);
    right.containers[0].items[0].row = 1;
    right.containers[0].items[0].col = 1;
    right.containers[0].items[0].instance.instance_id = 99;
    right.containers[0].items[0].instance.display_name = "右物".to_string();

    let outcome =
        exchange_inventory_items(&mut left, 42, &mut right, 99).expect("exchange should succeed");

    assert_eq!(outcome.left_revision, InventoryRevision(8));
    assert_eq!(outcome.right_revision, InventoryRevision(4));
    assert!(inventory_item_by_instance(&left, 42).is_none());
    assert!(inventory_item_by_instance(&right, 99).is_none());
    assert!(inventory_item_by_instance(&left, 99).is_some());
    assert!(inventory_item_by_instance(&right, 42).is_some());
}

#[test]
fn exchange_inventory_items_rejects_without_room_and_keeps_both_unchanged() {
    let mut left = make_test_inventory_with_one_item();
    left.containers.truncate(1);
    left.containers[0].cols = 1;
    left.containers[0].rows = 1;
    let original_left = left.clone();
    let mut right = make_test_inventory_with_one_item();
    right.containers[0].items[0].instance.instance_id = 99;
    right.containers[0].items[0].instance.grid_w = 2;
    right.containers[0].items[0].instance.grid_h = 1;
    let original_right = right.clone();

    let error = exchange_inventory_items(&mut left, 42, &mut right, 99)
        .expect_err("oversized incoming item should be rejected");

    assert!(error.contains("left inventory has no room"));
    assert_eq!(left.revision, original_left.revision);
    assert_eq!(left.containers, original_left.containers);
    assert_eq!(left.hotbar, original_left.hotbar);
    assert_eq!(right.revision, original_right.revision);
    assert_eq!(right.containers, original_right.containers);
    assert_eq!(right.hotbar, original_right.hotbar);
}

#[test]
fn select_drop_instance_ids_is_seed_stable() {
    let ids = vec![1, 2, 3, 4, 5, 6];
    let left = select_drop_instance_ids(ids.clone(), 3, 12345);
    let right = select_drop_instance_ids(ids, 3, 12345);
    assert_eq!(left, right);
    assert_eq!(left.len(), 3);
}

#[test]
fn apply_death_drop_to_inventory_removes_half_of_all_carryable_items() {
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 1,
        instance: ItemInstance {
            instance_id: 43,
            template_id: "ningmai_powder".to_string(),
            display_name: "凝脉散".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.2,
            rarity: ItemRarity::Uncommon,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        },
    });
    inv.hotbar[0] = Some(ItemInstance {
        instance_id: 99,
        template_id: "bone_spike".to_string(),
        display_name: "骨刺".to_string(),
        grid_w: 1,
        grid_h: 2,
        weight: 0.3,
        rarity: ItemRarity::Common,
        description: String::new(),
        stack_count: 1,
        spirit_quality: 1.0,
        durability: 1.0,
        freshness: None,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    });
    inv.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(ItemInstance {
            instance_id: 100,
            template_id: "rusted_blade".to_string(),
            display_name: "残破旧铁短刃".to_string(),
            grid_w: 1,
            grid_h: 2,
            weight: 0.5,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 0.5,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }),
    );

    let out = apply_death_drop_to_inventory(&mut inv, &ItemRegistry::default(), 777);

    assert_eq!(out.dropped.len(), 2);
    assert_eq!(out.revision, InventoryRevision(8));
    // 决议 #17/#12：死亡掉落按 instance 精确移除，空 SlotContents 会保留在 map 里，
    // 故统计实际剩余件须遍历 iter_all（而非 equipped.len 数槽）。
    let remaining_count = inv.containers[0].items.len()
        + inv.hotbar.iter().flatten().count()
        + inv
            .equipped
            .values()
            .map(|s| s.iter_all().count())
            .sum::<usize>();
    assert_eq!(remaining_count, 2);
}

#[test]
fn apply_death_drop_on_revive_emits_event_when_items_are_dropped() {
    use valence::prelude::{App, Events, Position, Update};

    let mut app = App::new();
    app.add_event::<PlayerRevived>();
    app.add_event::<DroppedItemEvent>();
    app.insert_resource(ItemRegistry::default());
    app.insert_resource(DroppedLootRegistry::default());
    app.add_systems(Update, apply_death_drop_on_revive);

    let entity = app
        .world_mut()
        .spawn((
            make_test_inventory_with_one_item(),
            Position::new([0.0, 64.0, 0.0]),
        ))
        .id();
    app.world_mut().send_event(PlayerRevived { entity });
    app.update();

    let events = app.world().resource::<Events<DroppedItemEvent>>();
    assert_eq!(
        events.len(),
        0,
        "single carried item should not drop when floor(n/2)=0"
    );

    {
        let mut inv = app.world_mut().get_mut::<PlayerInventory>(entity).unwrap();
        inv.containers[0].items.push(PlacedItemState {
            row: 0,
            col: 1,
            instance: ItemInstance {
                instance_id: 43,
                template_id: "ningmai_powder".to_string(),
                display_name: "凝脉散".to_string(),
                grid_w: 1,
                grid_h: 1,
                weight: 0.2,
                rarity: ItemRarity::Uncommon,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 1.0,
                durability: 1.0,
                freshness: None,
                mineral_id: None,
                charges: None,
                forge_quality: None,
                forge_color: None,
                forge_side_effects: Vec::new(),
                forge_achieved_tier: None,
                alchemy: None,
                lingering_owner_qi: None,
            },
        });
    }

    app.world_mut().send_event(PlayerRevived { entity });
    app.update();

    let inv = app.world().get::<PlayerInventory>(entity).unwrap();
    let events = app.world().resource::<Events<DroppedItemEvent>>();
    assert_eq!(events.len(), 1);
    assert_eq!(inv.revision, InventoryRevision(8));
    assert_eq!(inv.containers[0].items.len(), 1);
}

#[test]
fn apply_death_drop_on_revive_uses_pending_tsy_context_after_presence_is_cleared() {
    use crate::world::tsy::{DimensionAnchor, TsyPresence};
    use valence::prelude::{App, DVec3, Events, Position, Update};

    let mut app = App::new();
    app.add_event::<PlayerRevived>();
    app.add_event::<DroppedItemEvent>();
    app.insert_resource(ItemRegistry::default());
    app.insert_resource(DroppedLootRegistry::default());
    app.add_systems(Update, apply_death_drop_on_revive);

    let mut inventory = make_test_inventory_with_one_item();
    inventory.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 1,
        instance: ItemInstance {
            instance_id: 43,
            template_id: "ningmai_powder".to_string(),
            display_name: "凝脉散".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.2,
            rarity: ItemRarity::Uncommon,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        },
    });

    let presence = TsyPresence {
        family_id: "tsy_lingxu_01".to_string(),
        entered_at_tick: 7,
        entry_inventory_snapshot: vec![42],
        return_to: DimensionAnchor {
            dimension: DimensionKind::Overworld,
            pos: DVec3::new(1.0, 64.0, 1.0),
        },
    };
    let entity = app
        .world_mut()
        .spawn((
            inventory,
            Position::new([10.0, 64.0, 10.0]),
            DeathDropAnchor {
                pos: [10.0, 64.0, 10.0],
            },
            PendingTsyDeathDrop { presence },
        ))
        .id();

    app.world_mut().send_event(PlayerRevived { entity });
    app.update();

    assert!(
        app.world().get::<PendingTsyDeathDrop>(entity).is_none(),
        "复活掉落消费后应清除 pending TSY 上下文"
    );
    assert!(
        app.world().get::<DeathDropAnchor>(entity).is_none(),
        "复活掉落消费后应清除死亡点 anchor"
    );
    assert!(
        app.world()
            .get::<crate::world::tsy::TsyPresence>(entity)
            .is_none(),
        "pending 上下文不应重新引入 live TsyPresence"
    );

    let dropped_registry = app.world().resource::<DroppedLootRegistry>();
    let entry = dropped_registry
        .entries
        .get(&43)
        .expect("TSY acquired item should drop via pending context");
    assert_eq!(entry.dimension, DimensionKind::Tsy);
    assert!(
        entry
            .source_container_id
            .starts_with("tsy_corpse:tsy_lingxu_01/"),
        "pending TSY context 应保留 family 前缀，实际 {}",
        entry.source_container_id
    );

    let events = app.world().resource::<Events<DroppedItemEvent>>();
    assert_eq!(events.len(), 1);
}

#[test]
fn terminated_player_drops_all_items_except_on_voluntary_retire() {
    use valence::prelude::{App, EntityLayerId, InteractEntityEvent, Position, Update};

    let mut app = App::new();
    app.add_event::<PlayerTerminated>();
    app.insert_resource(DroppedLootRegistry::default());
    app.add_event::<InteractEntityEvent>();
    app.add_systems(
        Update,
        (
            apply_termination_drop_on_terminate,
            handle_remains_interactions,
        ),
    );

    let entity = app
        .world_mut()
        .spawn((
            make_test_inventory_with_one_item(),
            Position::new([10.0, 66.0, 10.0]),
            EntityLayerId(Entity::PLACEHOLDER),
            LifeRecord {
                character_id: "offline:Azure".to_string(),
                created_at: 0,
                biography: vec![BiographyEntry::Terminated {
                    cause: "tribulation_failed".to_string(),
                    tick: 1,
                }],
                insights_taken: Vec::new(),
                death_insights: Vec::new(),
                skill_milestones: Vec::new(),
                spirit_root_first: None,
                ..LifeRecord::default()
            },
        ))
        .id();

    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<DroppedLootRegistry>();
    let dropped_count = registry.entries.len();
    assert!(
        dropped_count >= 1,
        "terminated player should drop inventory"
    );

    // Voluntary retire should not create drops, but inventory should still be drained.
    let mut app = App::new();
    app.add_event::<PlayerTerminated>();
    app.insert_resource(DroppedLootRegistry::default());
    app.add_event::<InteractEntityEvent>();
    app.add_systems(
        Update,
        (
            apply_termination_drop_on_terminate,
            handle_remains_interactions,
        ),
    );

    let entity = app
        .world_mut()
        .spawn((
            make_test_inventory_with_one_item(),
            Position::new([10.0, 66.0, 10.0]),
            EntityLayerId(Entity::PLACEHOLDER),
            LifeRecord {
                character_id: "offline:Azure".to_string(),
                created_at: 0,
                biography: vec![BiographyEntry::Terminated {
                    cause: "voluntary_retire".to_string(),
                    tick: 1,
                }],
                insights_taken: Vec::new(),
                death_insights: Vec::new(),
                skill_milestones: Vec::new(),
                spirit_root_first: None,
                ..LifeRecord::default()
            },
        ))
        .id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<DroppedLootRegistry>();
    assert!(
        registry.entries.is_empty(),
        "voluntary_retire should not create drops"
    );

    let inv = app.world().get::<PlayerInventory>(entity).unwrap();
    let remaining_items = inv.containers.iter().flat_map(|c| c.items.iter()).count()
        + inv.equipped.len()
        + inv.hotbar.iter().flatten().count();
    assert_eq!(
        remaining_items, 0,
        "inventory should be drained on terminate"
    );
    assert_eq!(
        inv.bone_coins, 0,
        "bone_coins should be cleared on terminate"
    );
}

#[test]
fn natural_end_spawns_remains_and_allows_looting_via_interact() {
    use valence::prelude::{
        App, Despawned, EntityInteraction, Hand, InteractEntityEvent, Position, Update,
    };

    let mut app = App::new();
    app.add_event::<PlayerTerminated>();
    app.add_event::<InteractEntityEvent>();
    app.insert_resource(DroppedLootRegistry::default());
    app.add_systems(
        Update,
        (
            apply_termination_drop_on_terminate,
            handle_remains_interactions,
        ),
    );

    let terminated = app
        .world_mut()
        .spawn((
            make_test_inventory_with_one_item(),
            Position::new([10.0, 66.0, 10.0]),
            EntityLayerId(Entity::PLACEHOLDER),
            LifeRecord {
                character_id: "offline:OldOne".to_string(),
                created_at: 0,
                biography: vec![BiographyEntry::Terminated {
                    cause: "natural_end".to_string(),
                    tick: 1,
                }],
                insights_taken: Vec::new(),
                death_insights: Vec::new(),
                skill_milestones: Vec::new(),
                spirit_root_first: None,
                ..LifeRecord::default()
            },
        ))
        .id();
    {
        let mut inv = app
            .world_mut()
            .get_mut::<PlayerInventory>(terminated)
            .expect("terminated player should have inventory");
        inv.bone_coins = 7;
    }

    // Looter starts with an empty inventory.
    let mut looter_inv = make_test_inventory_with_one_item();
    for container in &mut looter_inv.containers {
        container.items.clear();
    }
    looter_inv.equipped.clear();
    looter_inv.hotbar = Default::default();
    looter_inv.bone_coins = 0;
    let looter = app
        .world_mut()
        .spawn((
            looter_inv,
            Position::new([10.0, 66.0, 10.0]),
            EntityLayerId(Entity::PLACEHOLDER),
        ))
        .id();

    app.world_mut()
        .send_event(PlayerTerminated { entity: terminated });
    app.update();

    // natural_end should not create world dropped loot entries.
    let registry = app.world().resource::<DroppedLootRegistry>();
    assert!(
        registry.entries.is_empty(),
        "natural_end should not create DroppedLootRegistry entries"
    );

    // Terminated player's inventory should be drained.
    let inv = app.world().get::<PlayerInventory>(terminated).unwrap();
    let remaining_items = inv.containers.iter().flat_map(|c| c.items.iter()).count()
        + inv.equipped.len()
        + inv.hotbar.iter().flatten().count();
    assert_eq!(remaining_items, 0);
    assert_eq!(inv.bone_coins, 0);

    // Remains should exist and hold the drained items/coins.
    let (
        remains_entity,
        remains_item_count,
        remains_bone_coins,
        remains_pos,
        remains_player_list_entry,
    ) = {
        let mut q = app
            .world_mut()
            .query::<(Entity, &RemainsContainer, &Position)>();
        let mut iter = q.iter(app.world());
        let (e, remains, pos) = iter.next().expect("expected exactly one remains container");
        assert!(
            iter.next().is_none(),
            "expected exactly one remains container"
        );
        let p = pos.get();
        (
            e,
            remains.items.len(),
            remains.bone_coins,
            [p.x, p.y, p.z],
            remains.player_list_entry,
        )
    };
    assert_eq!(remains_item_count, 1);
    assert_eq!(remains_bone_coins, 7);
    assert_eq!(remains_pos[0], 10.0);
    assert_eq!(remains_pos[1], 66.0);
    assert_eq!(remains_pos[2], 10.0);
    assert!(
        app.world().get_entity(remains_player_list_entry).is_some(),
        "player_list entry for remains should exist"
    );

    // Right click loots into the looter inventory.
    app.world_mut().send_event(InteractEntityEvent {
        client: looter,
        entity: remains_entity,
        sneaking: false,
        interact: EntityInteraction::Interact(Hand::Main),
    });
    app.update();

    let looter_inv = app.world().get::<PlayerInventory>(looter).unwrap();
    let has_item = looter_inv
        .containers
        .iter()
        .flat_map(|c| c.items.iter())
        .any(|placed| placed.instance.instance_id == 42);
    assert!(has_item, "looter should receive the remains item");
    assert_eq!(looter_inv.bone_coins, 7, "looter should receive bone_coins");

    assert!(
        app.world().get::<Despawned>(remains_entity).is_some(),
        "remains entity should be marked Despawned after looting"
    );
    assert!(
        app.world()
            .get::<Despawned>(remains_player_list_entry)
            .is_some(),
        "remains player_list entry should be marked Despawned after looting"
    );
}

/// plan-remains-suite P1 — 遗骸外观 pin：pose 必须是 Sleeping（Dying 对 player
/// 实体客户端不渲染躺姿），名字必须是正典中文「遗骸」（实体 CustomName 与
/// player list DisplayName 双处一致）。
#[test]
fn remains_entity_uses_sleeping_pose_and_chinese_display_name() {
    use valence::entity::entity::{CustomName, Pose as PoseComponent};
    use valence::player_list::DisplayName;
    use valence::prelude::{App, InteractEntityEvent, Position, Text, Update};

    let mut app = App::new();
    app.add_event::<PlayerTerminated>();
    app.add_event::<InteractEntityEvent>();
    app.insert_resource(DroppedLootRegistry::default());
    app.add_systems(Update, apply_termination_drop_on_terminate);

    let terminated = app
        .world_mut()
        .spawn((
            make_test_inventory_with_one_item(),
            Position::new([10.0, 66.0, 10.0]),
            EntityLayerId(Entity::PLACEHOLDER),
            LifeRecord {
                character_id: "offline:OldOne".to_string(),
                created_at: 0,
                biography: vec![BiographyEntry::Terminated {
                    cause: "natural_end".to_string(),
                    tick: 1,
                }],
                ..LifeRecord::default()
            },
        ))
        .id();
    app.world_mut()
        .send_event(PlayerTerminated { entity: terminated });
    app.update();

    let (pose, custom_name, player_list_entry) = {
        let mut q = app
            .world_mut()
            .query::<(&RemainsContainer, &PoseComponent, &CustomName)>();
        let mut iter = q.iter(app.world());
        let (remains, pose, custom_name) =
            iter.next().expect("expected exactly one remains entity");
        assert!(iter.next().is_none(), "expected exactly one remains entity");
        (pose.0, custom_name.0.clone(), remains.player_list_entry)
    };
    assert_eq!(
        pose,
        valence::entity::Pose::Sleeping,
        "遗骸 pose 必须是 Sleeping（player 实体只有 Sleeping 会整体躺平渲染；\
         Dying 只驱动 deathTime 死亡旋转，看起来是站着扭曲不像尸体）"
    );
    assert_eq!(
        custom_name,
        Some(Text::text(REMAINS_DISPLAY_NAME)),
        "遗骸实体 CustomName 必须是正典中文「遗骸」"
    );
    let display_name = app
        .world()
        .get::<DisplayName>(player_list_entry)
        .expect("remains player list entry should have DisplayName");
    assert_eq!(
        display_name.0,
        Some(Text::text(REMAINS_DISPLAY_NAME)),
        "player list DisplayName 必须与实体 CustomName 同为「遗骸」"
    );
}

/// plan-remains-suite P2 — G 键统一交互 happy path：RemainsLootIntent 把物品 +
/// 骨币全数转进拾取者背包，遗骸与 player list entry 双双 insert(Despawned)，
/// 且给玩家一条成功 narration。
#[test]
fn remains_loot_intent_happy_path_transfers_all_and_despawns() {
    use valence::prelude::{App, Despawned, Position, UniqueId, Update};

    let mut app = App::new();
    app.add_event::<PlayerTerminated>();
    app.add_event::<RemainsLootIntent>();
    app.insert_resource(DroppedLootRegistry::default());
    app.insert_resource(crate::player::gameplay::PendingGameplayNarrations::default());
    app.add_systems(
        Update,
        (
            apply_termination_drop_on_terminate,
            handle_remains_loot_intents,
        ),
    );

    let terminated = app
        .world_mut()
        .spawn((
            make_test_inventory_with_one_item(),
            Position::new([10.0, 66.0, 10.0]),
            EntityLayerId(Entity::PLACEHOLDER),
            CurrentDimension(DimensionKind::Overworld),
            LifeRecord {
                character_id: "offline:OldOne".to_string(),
                created_at: 0,
                biography: vec![BiographyEntry::Terminated {
                    cause: "natural_end".to_string(),
                    tick: 1,
                }],
                ..LifeRecord::default()
            },
        ))
        .id();
    {
        let mut inv = app
            .world_mut()
            .get_mut::<PlayerInventory>(terminated)
            .expect("terminated player should have inventory");
        inv.bone_coins = 9;
    }

    let mut looter_inv = make_test_inventory_with_one_item();
    for container in &mut looter_inv.containers {
        container.items.clear();
    }
    looter_inv.equipped.clear();
    looter_inv.hotbar = Default::default();
    looter_inv.bone_coins = 0;
    let looter = app
        .world_mut()
        .spawn((
            looter_inv,
            Position::new([10.5, 66.0, 10.0]),
            EntityLayerId(Entity::PLACEHOLDER),
            CurrentDimension(DimensionKind::Overworld),
            Username("Looter".to_string()),
        ))
        .id();

    app.world_mut()
        .send_event(PlayerTerminated { entity: terminated });
    app.update();

    let (remains_entity, remains_id, player_list_entry) = {
        let mut q = app
            .world_mut()
            .query::<(Entity, &UniqueId, &RemainsContainer)>();
        let (e, uuid, remains) = q
            .iter(app.world())
            .next()
            .expect("expected one remains entity");
        (e, uuid.0.to_string(), remains.player_list_entry)
    };

    app.world_mut().send_event(RemainsLootIntent {
        entity: looter,
        remains_id,
    });
    app.update();

    let looter_inv = app.world().get::<PlayerInventory>(looter).unwrap();
    let has_item = looter_inv
        .containers
        .iter()
        .flat_map(|c| c.items.iter())
        .any(|placed| placed.instance.instance_id == 42);
    assert!(has_item, "G 键路径应把遗骸物品转进拾取者背包");
    assert_eq!(looter_inv.bone_coins, 9, "G 键路径应把骨币转给拾取者");
    assert!(
        app.world().get::<Despawned>(remains_entity).is_some(),
        "搬空后遗骸实体必须 insert(Despawned)（不许裸 despawn——层实体裸删崩服）"
    );
    assert!(
        app.world().get::<Despawned>(player_list_entry).is_some(),
        "搬空后 player list entry 也必须 insert(Despawned)"
    );
    let narrations = app
        .world_mut()
        .resource_mut::<crate::player::gameplay::PendingGameplayNarrations>()
        .drain();
    assert_eq!(narrations.len(), 1, "成功搬运应有恰好一条成功 narration");
    assert_eq!(narrations[0].target.as_deref(), Some("Looter"));
}

/// plan-remains-suite P2 — 距离恰好等于 2.5m 上限时允许拾取，锁住
/// `distance_sq > REMAINS_PICKUP_RANGE_SQ` 的边界语义。
#[test]
fn remains_loot_intent_allows_exact_pickup_range_boundary() {
    use valence::prelude::{App, Despawned, Position, UniqueId, Update};

    let mut app = App::new();
    app.add_event::<PlayerTerminated>();
    app.add_event::<RemainsLootIntent>();
    app.insert_resource(DroppedLootRegistry::default());
    app.add_systems(
        Update,
        (
            apply_termination_drop_on_terminate,
            handle_remains_loot_intents,
        ),
    );

    let terminated = app
        .world_mut()
        .spawn((
            make_test_inventory_with_one_item(),
            Position::new([10.0, 66.0, 10.0]),
            EntityLayerId(Entity::PLACEHOLDER),
            CurrentDimension(DimensionKind::Overworld),
            LifeRecord {
                character_id: "offline:OldOne".to_string(),
                created_at: 0,
                biography: vec![BiographyEntry::Terminated {
                    cause: "natural_end".to_string(),
                    tick: 1,
                }],
                ..LifeRecord::default()
            },
        ))
        .id();

    let mut looter_inv = make_test_inventory_with_one_item();
    for container in &mut looter_inv.containers {
        container.items.clear();
    }
    looter_inv.equipped.clear();
    looter_inv.hotbar = Default::default();
    let looter = app
        .world_mut()
        .spawn((
            looter_inv,
            Position::new([12.5, 66.0, 10.0]),
            EntityLayerId(Entity::PLACEHOLDER),
            CurrentDimension(DimensionKind::Overworld),
            Username("Boundary".to_string()),
        ))
        .id();

    app.world_mut()
        .send_event(PlayerTerminated { entity: terminated });
    app.update();

    let (remains_entity, remains_id) = {
        let mut q = app
            .world_mut()
            .query::<(Entity, &UniqueId, &RemainsContainer)>();
        let (e, uuid, _) = q
            .iter(app.world())
            .next()
            .expect("expected one remains entity");
        (e, uuid.0.to_string())
    };

    app.world_mut().send_event(RemainsLootIntent {
        entity: looter,
        remains_id,
    });
    app.update();

    let looter_inv = app.world().get::<PlayerInventory>(looter).unwrap();
    let has_item = looter_inv
        .containers
        .iter()
        .flat_map(|c| c.items.iter())
        .any(|placed| placed.instance.instance_id == 42);
    assert!(
        has_item,
        "距离正好 2.5m 时应允许拾取；若这里失败，说明边界从 `>` 误改成了 `>=`"
    );
    assert!(
        app.world().get::<Despawned>(remains_entity).is_some(),
        "边界距离成功搬空后遗骸应 insert(Despawned)"
    );
}

/// plan-remains-suite P2 — 超出 2.5m 拒绝：不转移、不 despawn，且给玩家一条
/// 拒绝提示。
#[test]
fn remains_loot_intent_rejects_out_of_range() {
    use valence::prelude::{App, Despawned, Position, UniqueId, Update};

    let mut app = App::new();
    app.add_event::<PlayerTerminated>();
    app.add_event::<RemainsLootIntent>();
    app.insert_resource(DroppedLootRegistry::default());
    app.insert_resource(crate::player::gameplay::PendingGameplayNarrations::default());
    app.add_systems(
        Update,
        (
            apply_termination_drop_on_terminate,
            handle_remains_loot_intents,
        ),
    );

    let terminated = app
        .world_mut()
        .spawn((
            make_test_inventory_with_one_item(),
            Position::new([10.0, 66.0, 10.0]),
            EntityLayerId(Entity::PLACEHOLDER),
            CurrentDimension(DimensionKind::Overworld),
            LifeRecord {
                character_id: "offline:OldOne".to_string(),
                created_at: 0,
                biography: vec![BiographyEntry::Terminated {
                    cause: "natural_end".to_string(),
                    tick: 1,
                }],
                ..LifeRecord::default()
            },
        ))
        .id();
    let looter = app
        .world_mut()
        .spawn((
            make_test_inventory_with_one_item(),
            // 遗骸在 (10,66,10)，拾取者站 10m 外——2.5m 上限必须拒绝。
            Position::new([20.0, 66.0, 10.0]),
            EntityLayerId(Entity::PLACEHOLDER),
            CurrentDimension(DimensionKind::Overworld),
            Username("FarAway".to_string()),
        ))
        .id();

    app.world_mut()
        .send_event(PlayerTerminated { entity: terminated });
    app.update();

    let (remains_entity, remains_id) = {
        let mut q = app
            .world_mut()
            .query::<(Entity, &UniqueId, &RemainsContainer)>();
        let (e, uuid, _) = q
            .iter(app.world())
            .next()
            .expect("expected one remains entity");
        (e, uuid.0.to_string())
    };

    app.world_mut().send_event(RemainsLootIntent {
        entity: looter,
        remains_id,
    });
    app.update();

    let remains = app
        .world()
        .get::<RemainsContainer>(remains_entity)
        .expect("out-of-range attempt should leave remains intact");
    assert_eq!(remains.items.len(), 1, "超距请求不得转移任何物品");
    assert!(
        app.world().get::<Despawned>(remains_entity).is_none(),
        "超距请求不得 despawn 遗骸"
    );
    let narrations = app
        .world_mut()
        .resource_mut::<crate::player::gameplay::PendingGameplayNarrations>()
        .drain();
    assert_eq!(narrations.len(), 1, "超距拒绝应有一条提示 narration");
}

/// plan-remains-suite P2 — 跨 dimension 拒绝（Overworld 遗骸 vs TSY 拾取者）。
#[test]
fn remains_loot_intent_rejects_cross_dimension() {
    use valence::prelude::{App, Despawned, Position, UniqueId, Update};

    let mut app = App::new();
    app.add_event::<PlayerTerminated>();
    app.add_event::<RemainsLootIntent>();
    app.insert_resource(DroppedLootRegistry::default());
    app.add_systems(
        Update,
        (
            apply_termination_drop_on_terminate,
            handle_remains_loot_intents,
        ),
    );

    let terminated = app
        .world_mut()
        .spawn((
            make_test_inventory_with_one_item(),
            Position::new([10.0, 66.0, 10.0]),
            EntityLayerId(Entity::PLACEHOLDER),
            CurrentDimension(DimensionKind::Overworld),
            LifeRecord {
                character_id: "offline:OldOne".to_string(),
                created_at: 0,
                biography: vec![BiographyEntry::Terminated {
                    cause: "natural_end".to_string(),
                    tick: 1,
                }],
                ..LifeRecord::default()
            },
        ))
        .id();
    let looter = app
        .world_mut()
        .spawn((
            make_test_inventory_with_one_item(),
            Position::new([10.0, 66.0, 10.0]),
            EntityLayerId(Entity::PLACEHOLDER),
            // 同坐标同 layer，但人在 TSY——跨界必须拒绝。
            CurrentDimension(DimensionKind::Tsy),
            Username("TsyDiver".to_string()),
        ))
        .id();

    app.world_mut()
        .send_event(PlayerTerminated { entity: terminated });
    app.update();

    let (remains_entity, remains_id) = {
        let mut q = app
            .world_mut()
            .query::<(Entity, &UniqueId, &RemainsContainer)>();
        let (e, uuid, _) = q
            .iter(app.world())
            .next()
            .expect("expected one remains entity");
        (e, uuid.0.to_string())
    };

    app.world_mut().send_event(RemainsLootIntent {
        entity: looter,
        remains_id,
    });
    app.update();

    let remains = app
        .world()
        .get::<RemainsContainer>(remains_entity)
        .expect("cross-dimension attempt should leave remains intact");
    assert_eq!(remains.items.len(), 1, "跨 dimension 请求不得转移任何物品");
    assert!(
        app.world().get::<Despawned>(remains_entity).is_none(),
        "跨 dimension 请求不得 despawn 遗骸"
    );
}

/// plan-remains-suite P2 — unknown remains_id：无操作（良性竞态：遗骸可能刚被
/// 他人搬空 despawn，client 缓存过期）。
#[test]
fn remains_loot_intent_unknown_remains_id_is_noop() {
    use valence::prelude::{App, Position, Update};

    let mut app = App::new();
    app.add_event::<RemainsLootIntent>();
    app.add_systems(Update, handle_remains_loot_intents);

    let looter = app
        .world_mut()
        .spawn((
            make_test_inventory_with_one_item(),
            Position::new([10.0, 66.0, 10.0]),
            EntityLayerId(Entity::PLACEHOLDER),
            CurrentDimension(DimensionKind::Overworld),
            Username("Looter".to_string()),
        ))
        .id();
    let inv_before = app.world().get::<PlayerInventory>(looter).unwrap().revision;

    app.world_mut().send_event(RemainsLootIntent {
        entity: looter,
        remains_id: "00000000-0000-0000-0000-000000000000".to_string(),
    });
    app.update();

    let inv_after = app.world().get::<PlayerInventory>(looter).unwrap();
    assert_eq!(
        inv_after.revision, inv_before,
        "unknown remains_id 必须是纯 no-op，不得触碰拾取者背包 revision"
    );
}

/// plan-remains-suite P2 — 包满部分拾取：G 键路径与右键路径共用
/// transfer_remains_to_looter，行为必须一致——骨币照收、装不下的物品留在
/// 遗骸里、遗骸不 despawn。
#[test]
fn remains_loot_intent_full_inventory_partial_pickup_matches_interact_path() {
    use valence::prelude::{App, Despawned, Position, UniqueId, Update};

    let mut app = App::new();
    app.add_event::<PlayerTerminated>();
    app.add_event::<RemainsLootIntent>();
    app.insert_resource(DroppedLootRegistry::default());
    app.insert_resource(crate::player::gameplay::PendingGameplayNarrations::default());
    app.add_systems(
        Update,
        (
            apply_termination_drop_on_terminate,
            handle_remains_loot_intents,
        ),
    );

    let terminated = app
        .world_mut()
        .spawn((
            make_test_inventory_with_one_item(),
            Position::new([10.0, 66.0, 10.0]),
            EntityLayerId(Entity::PLACEHOLDER),
            CurrentDimension(DimensionKind::Overworld),
            LifeRecord {
                character_id: "offline:OldOne".to_string(),
                created_at: 0,
                biography: vec![BiographyEntry::Terminated {
                    cause: "natural_end".to_string(),
                    tick: 1,
                }],
                ..LifeRecord::default()
            },
        ))
        .id();
    {
        let mut inv = app
            .world_mut()
            .get_mut::<PlayerInventory>(terminated)
            .expect("terminated player should have inventory");
        inv.bone_coins = 5;
    }

    // 拾取者背包：完全没有容器 → 任何物品都装不下，但骨币仍能收。
    let mut looter_inv = make_test_inventory_with_one_item();
    looter_inv.containers.clear();
    looter_inv.equipped.clear();
    looter_inv.hotbar = Default::default();
    looter_inv.bone_coins = 0;
    let looter = app
        .world_mut()
        .spawn((
            looter_inv,
            Position::new([10.0, 66.0, 10.0]),
            EntityLayerId(Entity::PLACEHOLDER),
            CurrentDimension(DimensionKind::Overworld),
            Username("FullPack".to_string()),
        ))
        .id();

    app.world_mut()
        .send_event(PlayerTerminated { entity: terminated });
    app.update();

    let (remains_entity, remains_id) = {
        let mut q = app
            .world_mut()
            .query::<(Entity, &UniqueId, &RemainsContainer)>();
        let (e, uuid, _) = q
            .iter(app.world())
            .next()
            .expect("expected one remains entity");
        (e, uuid.0.to_string())
    };

    app.world_mut().send_event(RemainsLootIntent {
        entity: looter,
        remains_id,
    });
    app.update();

    let looter_inv = app.world().get::<PlayerInventory>(looter).unwrap();
    assert_eq!(
        looter_inv.bone_coins, 5,
        "包满时骨币仍应转移（与右键路径一致）"
    );
    let remains = app
        .world()
        .get::<RemainsContainer>(remains_entity)
        .expect("partially looted remains should survive");
    assert_eq!(
        remains.items.len(),
        1,
        "装不下的物品必须留在遗骸里（与右键路径一致）"
    );
    assert_eq!(remains.bone_coins, 0, "骨币应已被取走");
    assert!(
        app.world().get::<Despawned>(remains_entity).is_none(),
        "遗骸尚有剩余物品时不得 despawn（与右键路径一致）"
    );
}

#[test]
fn pickup_dropped_loot_instance_reinserts_item_and_clears_registry_entry() {
    let mut inventory = make_test_inventory_with_one_item();
    inventory.containers[0].items.clear();

    let owner = Entity::PLACEHOLDER;
    let mut registry = DroppedLootRegistry::default();
    registry.entries.insert(
        42,
        DroppedLootEntry {
            instance_id: 42,
            source_container_id: MAIN_PACK_CONTAINER_ID.to_string(),
            source_row: 0,
            source_col: 0,
            world_pos: [0.5, 64.0, 0.5],
            dimension: DimensionKind::Overworld,
            item: ItemInstance {
                instance_id: 42,
                template_id: "starter_talisman".to_string(),
                display_name: "启程护符".to_string(),
                grid_w: 1,
                grid_h: 1,
                weight: 0.2,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 1.0,
                durability: 1.0,
                freshness: None,
                mineral_id: None,
                charges: None,
                forge_quality: None,
                forge_color: None,
                forge_side_effects: Vec::new(),
                forge_achieved_tier: None,
                alchemy: None,
                lingering_owner_qi: None,
            },
        },
    );

    let revision =
        pickup_dropped_loot_instance(&mut inventory, &mut registry, [0.0, 64.0, 0.0], 42)
            .expect("pickup should succeed");

    assert_eq!(revision, InventoryRevision(8));
    assert_eq!(inventory.containers[0].items.len(), 1);
    assert!(!registry.entries.contains_key(&42));
    let _ = owner;
}

#[test]
fn discard_inventory_item_to_dropped_loot_removes_item_and_registers_drop() {
    let mut inventory = make_test_inventory_with_one_item();
    let owner = Entity::PLACEHOLDER;
    let mut registry = DroppedLootRegistry::default();

    let outcome = discard_inventory_item_to_dropped_loot(
        &mut inventory,
        &mut registry,
        [0.0, 64.0, 0.0],
        DimensionKind::Overworld,
        42,
        &crate::schema::inventory::InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
    )
    .expect("discard should succeed");

    assert_eq!(outcome.revision, InventoryRevision(8));
    assert!(inventory.containers[0].items.is_empty());
    let entry = registry
        .entries
        .get(&42)
        .expect("registry should contain dropped item");
    assert_eq!(entry.instance_id, 42);
    assert_eq!(entry.source_container_id, MAIN_PACK_CONTAINER_ID);
    let _ = owner;
}

#[test]
fn discard_inventory_item_to_dropped_loot_stays_pickable_after_registry_growth() {
    let mut inventory = make_test_inventory_with_one_item();
    let template_item = inventory.containers[0].items[0].instance.clone();
    let mut registry = DroppedLootRegistry::default();
    for index in 0..40_u64 {
        let instance_id = 1_000 + index;
        let mut item = template_item.clone();
        item.instance_id = instance_id;
        registry.entries.insert(
            instance_id,
            DroppedLootEntry {
                instance_id,
                source_container_id: MAIN_PACK_CONTAINER_ID.to_string(),
                source_row: 0,
                source_col: 0,
                world_pos: [0.5, 64.0, 0.5],
                dimension: DimensionKind::Overworld,
                item,
            },
        );
    }

    let outcome = discard_inventory_item_to_dropped_loot(
        &mut inventory,
        &mut registry,
        [0.0, 64.0, 0.0],
        DimensionKind::Overworld,
        42,
        &crate::schema::inventory::InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
    )
    .expect("discard should succeed after registry growth");

    pickup_dropped_loot_instance(&mut inventory, &mut registry, [0.0, 64.0, 0.0], 42)
        .expect("a fresh drop must remain within pickup range after registry growth");
    assert_eq!(outcome.dropped.instance_id, 42);
}

#[test]
fn death_drop_keeps_high_durability_equipped_weapon() {
    let mut registry = ItemRegistry::default();
    registry.templates.insert(
        "iron_sword".to_string(),
        ItemTemplate {
            id: "iron_sword".to_string(),
            display_name: "铁剑".to_string(),
            category: ItemCategory::Weapon,
            placeable: None,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 2,
            base_weight: 1.0,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 1.0,
            description: String::new(),
            effect: None,
            cast_duration_ms: DEFAULT_CAST_DURATION_MS,
            cooldown_ms: DEFAULT_COOLDOWN_MS,
            weapon_spec: Some(WeaponSpec {
                weapon_kind: crate::combat::weapon::WeaponKind::Sword,
                base_attack: 8.0,
                quality_tier: 0,
                durability_max: 200.0,
                qi_cost_mul: 1.0,
            }),
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            readable_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shield_spec: None,

            shelflife_profile: None,
            shelflife_track: None,
            wearer_race: crate::body_plan::types::RaceGateOwned::default(),
        },
    );
    let mut inv = make_test_inventory_with_one_item();
    inv.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(ItemInstance {
            instance_id: 9001,
            template_id: "iron_sword".to_string(),
            display_name: "铁剑".to_string(),
            grid_w: 1,
            grid_h: 2,
            weight: 1.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 0.75,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }),
    );

    let out = apply_death_drop_to_inventory(&mut inv, &registry, 42);

    assert!(out.dropped.iter().all(|d| d.instance.instance_id != 9001));
    assert_eq!(
        inv.equipped
            .get(EQUIP_SLOT_MAIN_HAND)
            .and_then(|s| s.held.as_ref())
            .map(|item| item.instance_id),
        Some(9001)
    );
}

#[test]
fn death_drop_drops_low_durability_equipped_weapon() {
    let mut registry = ItemRegistry::default();
    registry.templates.insert(
        "iron_sword".to_string(),
        ItemTemplate {
            id: "iron_sword".to_string(),
            display_name: "铁剑".to_string(),
            category: ItemCategory::Weapon,
            placeable: None,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 2,
            base_weight: 1.0,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 1.0,
            description: String::new(),
            effect: None,
            cast_duration_ms: DEFAULT_CAST_DURATION_MS,
            cooldown_ms: DEFAULT_COOLDOWN_MS,
            weapon_spec: Some(WeaponSpec {
                weapon_kind: crate::combat::weapon::WeaponKind::Sword,
                base_attack: 8.0,
                quality_tier: 0,
                durability_max: 200.0,
                qi_cost_mul: 1.0,
            }),
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            readable_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shield_spec: None,

            shelflife_profile: None,
            shelflife_track: None,
            wearer_race: crate::body_plan::types::RaceGateOwned::default(),
        },
    );
    let mut inv = make_test_inventory_with_one_item();
    inv.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(ItemInstance {
            instance_id: 9002,
            template_id: "iron_sword".to_string(),
            display_name: "铁剑".to_string(),
            grid_w: 1,
            grid_h: 2,
            weight: 1.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 0.25,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }),
    );

    let out = apply_death_drop_to_inventory(&mut inv, &registry, 42);

    assert!(out.dropped.iter().any(|d| d.instance.instance_id == 9002));
    // 死亡掉落按 instance 精确移除 held；空 SlotContents 会保留在 map 里，
    // 故断言 main_hand held 已清空（而非整槽 contains_key）。
    assert!(
        inv.equipped
            .get(EQUIP_SLOT_MAIN_HAND)
            .map(|s| s.is_empty())
            .unwrap_or(true),
        "低耐武器掉落后 main_hand held 应为空"
    );
}

#[test]
fn calculate_current_weight_includes_container_equipped_and_hotbar() {
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.weight = 1.5;
    inv.containers[0].items[0].instance.stack_count = 2;
    inv.hotbar[0] = Some(ItemInstance {
        instance_id: 99,
        template_id: "bone_spike".to_string(),
        display_name: "骨刺".to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.5,
        rarity: ItemRarity::Common,
        description: String::new(),
        stack_count: 1,
        spirit_quality: 1.0,
        durability: 1.0,
        freshness: None,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    });
    inv.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(ItemInstance {
            instance_id: 100,
            template_id: "rusted_blade".to_string(),
            display_name: "残破旧铁短刃".to_string(),
            grid_w: 1,
            grid_h: 2,
            weight: 2.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }),
    );

    let current = calculate_current_weight(&inv);

    assert!((current - 5.5).abs() < 1e-9);
}

// ========================================================================
// plan-tarkov-backpack-v1 P1 — 重量递归上卷 pin 测试（决议 #3 固化现状语义）。
//
// 决议 #3：`calculate_current_weight` 三路 flat 求和（container + equipped +
// hotbar）经核实**不重叠**：穿戴背包件自重走 equipped（worn 层），其内含物走
// container（`pack_<id>.items`），背包件本身从不出现在任何 `ContainerState.items`
// 里 → flat 求和数学等价于「外层背包自重 + 逐层递归内含物」的上卷。**不改公式**，
// 仅以下列 pin 测试锁住该等价性，任何回归（误把背包件塞进 container、或漏算内含物、
// 或双计自重）立刻撞红。
// ========================================================================

/// P1 pin：外层 worn 背包 + 其 grid 内一件物品 → current = 包自重 + 内物品自重。
/// 锁住「内含物（嵌套容器里的件）确实被计入 current_weight」（递归上卷第二层）。
#[test]
fn calculate_current_weight_counts_item_in_nested_container() {
    let mut inv = make_empty_inventory();

    // 外层：worn 背包件骑 chest 槽，自重 2.5。
    let mut pack = make_container_item(500, "large_backpack");
    pack.weight = 2.5;
    let pack_id = container_id_for_worn_pack(500);
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(pack),
    );

    // 内层：背包派生容器 pack_500 里放一件 herb，自重 3.0。
    let mut inner = make_test_item_instance(501, "herb");
    inner.weight = 3.0;
    inv.containers.push(ContainerState {
        quick_access: false,
        id: pack_id,
        name: "大背包".to_string(),
        rows: 7,
        cols: 5,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: inner,
        }],
        owner_instance_id: Some(500),
    });

    let current = calculate_current_weight(&inv);
    let expected = 2.5 + 3.0; // 包自重 + 内物品自重，递归上卷两层之和。
    assert!(
        (current - expected).abs() < 1e-9,
        "期望 current = 包自重(2.5) + 嵌套内物品自重(3.0) = {expected}（内含物必须被上卷计入），实际 {current}"
    );
}

/// P1 pin：穿戴背包件自重只计一次——背包件在 equipped(worn) 计一次，
/// 绝不在 container_weight 里被重复计（背包件本身从不进 ContainerState.items）。
/// 锁住 flat 三路求和「不重叠」前提（决议 #3 的核心）。
#[test]
fn calculate_current_weight_no_double_count_for_worn_pack() {
    let mut inv = make_empty_inventory();

    // worn 背包件自重 4.0（仅此一件，container 空）。
    let mut pack = make_container_item(600, "large_backpack");
    pack.weight = 4.0;
    let pack_id = container_id_for_worn_pack(600);
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(pack),
    );

    // 派生容器存在但为空——背包件本身不在此 items 里。
    inv.containers.push(ContainerState {
        quick_access: false,
        id: pack_id,
        name: "大背包".to_string(),
        rows: 7,
        cols: 5,
        items: Vec::new(),
        owner_instance_id: Some(600),
    });

    let current = calculate_current_weight(&inv);
    assert!(
        (current - 4.0).abs() < 1e-9,
        "期望 current = 背包件自重 4.0（仅在 equipped 计一次，不被 container_weight 重复计），实际 {current}——若 >4.0 说明背包件自重被双计"
    );
}

/// P1 pin（verifiable#4 危险边界 + 状态转换锁 P0 修复语义）：
/// 背包件被卸下（不再在任何身体槽 worn 层），但其旧 `pack_<id>` 容器尚未被
/// `rebuild_containers_from_equipment` 清除（P0 修复路径触发前的可达状态）。
/// - rebuild 前：孤儿容器内含物如实计入 current_weight（容器仍存在、items 仍在）。
/// - rebuild 后：孤儿容器被清除（内含物 spill 进 body_pocket），current_weight
///   守恒不变（内含物换了位置但仍在某容器里），且不再出现「背包件自重 + 孤儿内含物」
///   的 double-count 风险面。
#[test]
fn calculate_current_weight_after_unequip_pack_no_double_count_orphan_container() {
    let registry = ItemRegistry::from_map(HashMap::new());
    let mut inv = make_empty_inventory();

    // body_pocket 作 spill 落点（2×3=6 格，足够容纳一件 1×1 内含物）。
    inv.containers.push(ContainerState {
        quick_access: false,
        id: BODY_POCKET_CONTAINER_ID.to_string(),
        name: "暗袋".to_string(),
        rows: BODY_POCKET_ROWS,
        cols: BODY_POCKET_COLS,
        items: Vec::new(),
        owner_instance_id: None,
    });

    // 孤儿 pack_700：容器仍存在 + 含一件 herb(自重 3.0)，但 equipped 里**没有**
    // instance_id=700 的 worn 背包件（已卸下，背包件自重不再计入）。
    let pack_id = container_id_for_worn_pack(700);
    let mut inner = make_test_item_instance(701, "herb");
    inner.weight = 3.0;
    inv.containers.push(ContainerState {
        quick_access: false,
        id: pack_id.clone(),
        name: "大背包".to_string(),
        rows: 7,
        cols: 5,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: inner,
        }],
        owner_instance_id: None,
    });

    // rebuild 前：孤儿内含物如实计入（容器尚在）。背包件已卸 → 不在 equipped。
    let before = calculate_current_weight(&inv);
    assert!(
        (before - 3.0).abs() < 1e-9,
        "rebuild 前期望 current = 孤儿容器内含物自重 3.0（容器仍存在故如实计入；背包件已卸不计自重），实际 {before}"
    );

    // 触发 P0 修复路径：rebuild 清除孤儿容器，内含物 spill 进 body_pocket。
    let overflow = rebuild_containers_from_equipment(&mut inv, &registry);
    assert!(
        overflow.is_empty(),
        "body_pocket 有空位，内含物应全部 spill 进去、无 overflow，实际 overflow={overflow:?}"
    );
    assert!(
        !inv.containers.iter().any(|c| c.id == pack_id),
        "rebuild 后孤儿容器 {pack_id} 必须被清除（不再可 access），实际容器列表={:?}",
        inv.containers.iter().map(|c| &c.id).collect::<Vec<_>>()
    );

    // rebuild 后：内含物换位到 body_pocket，current 守恒不变、无 double-count。
    let after = calculate_current_weight(&inv);
    assert!(
        (after - before).abs() < 1e-9,
        "rebuild 前后 current 必须守恒（内含物只换位置不增减）：期望 {before}，实际 {after}——若变大说明孤儿内含物被 double-count，若变小说明 spill 丢物"
    );
}

/// P1 pin（状态转换 A→B）：嵌套背包内含物使总重超 max_weight → OverloadedMarker 挂上。
/// 锁住 `sync_overloaded_marker` 对「内含物（container_weight）」的感知。
#[test]
fn overloaded_marker_triggers_when_nested_pack_contents_exceed_limit() {
    use valence::prelude::{App, Update};

    let mut app = App::new();
    app.add_systems(Update, sync_overloaded_marker);

    let mut inv = make_empty_inventory();
    inv.max_weight = 10.0;

    // worn 背包件自重 1.0。
    let mut pack = make_container_item(800, "large_backpack");
    pack.weight = 1.0;
    let pack_id = container_id_for_worn_pack(800);
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(pack),
    );

    // 嵌套内含物自重 20.0 → current = 21.0 > max 10.0。
    let mut heavy = make_test_item_instance(801, "ore");
    heavy.weight = 20.0;
    inv.containers.push(ContainerState {
        quick_access: false,
        id: pack_id,
        name: "大背包".to_string(),
        rows: 7,
        cols: 5,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: heavy,
        }],
        owner_instance_id: Some(800),
    });

    let entity = app.world_mut().spawn(inv).id();
    app.update();

    let marker = app
        .world()
        .get::<OverloadedMarker>(entity)
        .expect("嵌套内含物(20.0)+包自重(1.0)=21.0 > max(10.0)，应挂 OverloadedMarker");
    assert!(
        (marker.current_weight - 21.0).abs() < 1e-9,
        "marker.current_weight 应反映含嵌套内含物的总重 21.0（包自重1.0+内含物20.0），实际 {}",
        marker.current_weight
    );
    assert!(
        marker.current_weight > marker.max_weight,
        "marker 应记录超限（current {} > max {}）",
        marker.current_weight,
        marker.max_weight
    );
}

/// P1 pin（状态转换 A→B→A）：移除嵌套内含物使总重回落 ≤ max → OverloadedMarker 清除。
#[test]
fn overloaded_marker_clears_after_removing_nested_item() {
    use valence::prelude::{App, Update};

    let mut app = App::new();
    app.add_systems(Update, sync_overloaded_marker);

    let mut inv = make_empty_inventory();
    inv.max_weight = 10.0;

    let mut pack = make_container_item(900, "large_backpack");
    pack.weight = 1.0;
    let pack_id = container_id_for_worn_pack(900);
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(pack),
    );

    let mut heavy = make_test_item_instance(901, "ore");
    heavy.weight = 20.0;
    inv.containers.push(ContainerState {
        quick_access: false,
        id: pack_id,
        name: "大背包".to_string(),
        rows: 7,
        cols: 5,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: heavy,
        }],
        owner_instance_id: Some(900),
    });

    let entity = app.world_mut().spawn(inv).id();
    app.update();
    assert!(
        app.world().get::<OverloadedMarker>(entity).is_some(),
        "前置：超限态应先挂 marker（A→B）"
    );

    // 移除嵌套内含物 → current 回落到 1.0（仅包自重）≤ max 10.0。
    {
        let mut inv = app.world_mut().get_mut::<PlayerInventory>(entity).unwrap();
        let pack_id = container_id_for_worn_pack(900);
        let container = inv
            .containers
            .iter_mut()
            .find(|c| c.id == pack_id)
            .expect("pack_900 容器应存在");
        container.items.clear();
    }
    app.update();

    assert!(
        app.world().get::<OverloadedMarker>(entity).is_none(),
        "移除嵌套内含物后 current(1.0) ≤ max(10.0)，OverloadedMarker 应被清除（A→B→A 状态转换闭环）"
    );
}

#[test]
fn sync_overloaded_marker_adds_and_removes_marker_based_on_weight() {
    use valence::prelude::{App, Update};

    let mut app = App::new();
    app.add_systems(Update, sync_overloaded_marker);

    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.weight = 60.0;
    inv.max_weight = 50.0;
    let entity = app.world_mut().spawn(inv).id();

    app.update();

    let marker = app
        .world()
        .get::<OverloadedMarker>(entity)
        .expect("marker should exist");
    assert!(marker.current_weight > marker.max_weight);

    {
        let mut inv = app.world_mut().get_mut::<PlayerInventory>(entity).unwrap();
        inv.containers[0].items[0].instance.weight = 10.0;
    }

    app.update();

    assert!(app.world().get::<OverloadedMarker>(entity).is_none());
}

// =========== inventory_item_by_instance_borrow (M4 optimization) ===========

fn make_test_item_instance(instance_id: u64, template_id: &str) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: template_id.to_string(),
        display_name: template_id.to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.1,
        rarity: ItemRarity::Common,
        description: "test".to_string(),
        stack_count: 1,
        spirit_quality: 1.0,
        durability: 1.0,
        freshness: None,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    }
}

fn make_empty_inventory() -> PlayerInventory {
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(0),
        containers: Vec::new(),
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 100.0,
    }
}

#[test]
fn borrow_helper_finds_item_in_container() {
    let mut inv = make_empty_inventory();
    inv.containers.push(ContainerState {
        quick_access: false,
        id: "main_pack".into(),
        name: "main_pack".into(),
        rows: 4,
        cols: 4,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(42, "iron_sword"),
        }],

        owner_instance_id: None,
    });
    let got = inventory_item_by_instance_borrow(&inv, 42);
    assert!(got.is_some());
    assert_eq!(got.unwrap().template_id, "iron_sword");
}

// ─── plan-layered-equip-v1 P4（决议 #8）— 法宝触发位 apply_treasure_activate ───

/// 注册一个 treasure 模板 + 一个普通（armor）模板的 registry。
fn treasure_trigger_registry() -> ItemRegistry {
    let treasure = raw_item_template_toml("test_treasure", "treasure")
        .try_into_item_template(Path::new("<inline>"))
        .expect("treasure template parses");
    let armor = raw_item_template_toml("test_armor", "armor")
        .try_into_item_template(Path::new("<inline>"))
        .expect("armor template parses");
    registry_from_templates(vec![treasure, armor])
}

/// 带一个 8x8 main_pack 容器的空 inventory（触发位 deactivate 落点）。
fn inventory_with_main_pack() -> PlayerInventory {
    let mut inv = make_empty_inventory();
    inv.containers.push(ContainerState {
        quick_access: false,
        id: MAIN_PACK_CONTAINER_ID.into(),
        name: MAIN_PACK_CONTAINER_ID.into(),
        rows: 8,
        cols: 8,
        items: Vec::new(),
        owner_instance_id: None,
    });
    inv
}

#[test]
fn treasure_activate_moves_treasure_from_container_to_trigger_slot() {
    let registry = treasure_trigger_registry();
    let mut inv = inventory_with_main_pack();
    inv.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: make_test_item_instance(500, "test_treasure"),
    });
    let before_rev = inv.revision.0;

    let outcome = apply_treasure_activate(&mut inv, &registry, 500, true)
        .expect("activating a treasure in the container should succeed");

    assert!(
        matches!(outcome, TreasureActivateOutcome::Activated { .. }),
        "expected Activated, got {outcome:?}"
    );
    assert_eq!(
        inv.triggered_treasures.len(),
        1,
        "treasure should be in the trigger slot"
    );
    assert_eq!(inv.triggered_treasures[0].instance_id, 500);
    assert!(
        inv.containers[0].items.is_empty(),
        "treasure should have left the container (no duplication)"
    );
    assert!(inv.revision.0 > before_rev, "revision should bump");
}

#[test]
fn treasure_activate_roundtrip_deactivate_returns_to_container_preserving_instance() {
    let registry = treasure_trigger_registry();
    let mut inv = inventory_with_main_pack();
    let mut original = make_test_item_instance(501, "test_treasure");
    original.durability = 0.42; // 非默认值，断言实例（含耐久）原样保留，不是重新生成
    inv.containers[0].items.push(PlacedItemState {
        row: 1,
        col: 2,
        instance: original,
    });

    apply_treasure_activate(&mut inv, &registry, 501, true).expect("activate ok");
    assert_eq!(inv.triggered_treasures.len(), 1);
    assert!(inv.containers[0].items.is_empty());

    let outcome = apply_treasure_activate(&mut inv, &registry, 501, false).expect("deactivate ok");
    assert!(
        matches!(outcome, TreasureActivateOutcome::Deactivated { .. }),
        "expected Deactivated, got {outcome:?}"
    );
    assert!(
        inv.triggered_treasures.is_empty(),
        "trigger slot should be empty after deactivate"
    );
    assert_eq!(
        inv.containers[0].items.len(),
        1,
        "treasure should be back in the container"
    );
    let returned = &inv.containers[0].items[0].instance;
    assert_eq!(returned.instance_id, 501, "same instance id preserved");
    assert!(
        (returned.durability - 0.42).abs() < f64::EPSILON,
        "durability preserved (existing instance moved, not regenerated): expected 0.42, got {}",
        returned.durability
    );
}

#[test]
fn treasure_activate_rejects_when_trigger_slot_full() {
    let registry = treasure_trigger_registry();
    let mut inv = inventory_with_main_pack();
    // 触发位预填满 CAP 件。
    for i in 0..TREASURE_TRIGGER_CAP {
        inv.triggered_treasures
            .push(make_test_item_instance(600 + i as u64, "test_treasure"));
    }
    // 背包再放一件想激活的。
    inv.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: make_test_item_instance(700, "test_treasure"),
    });

    let result = apply_treasure_activate(&mut inv, &registry, 700, true);

    assert!(
        result.is_err(),
        "activating into a full trigger slot must be rejected"
    );
    assert_eq!(
        inv.triggered_treasures.len(),
        TREASURE_TRIGGER_CAP,
        "trigger slot unchanged on reject"
    );
    assert_eq!(
        inv.containers[0].items.len(),
        1,
        "rejected treasure stays in the container (not dropped)"
    );
}

#[test]
fn treasure_activate_rejects_non_treasure_item() {
    let registry = treasure_trigger_registry();
    let mut inv = inventory_with_main_pack();
    inv.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: make_test_item_instance(800, "test_armor"),
    });

    let result = apply_treasure_activate(&mut inv, &registry, 800, true);

    assert!(
        result.is_err(),
        "non-Treasure items must not be activatable into the trigger slot"
    );
    assert!(
        inv.triggered_treasures.is_empty(),
        "trigger slot stays empty"
    );
    assert_eq!(
        inv.containers[0].items.len(),
        1,
        "armor stays in the container"
    );
}

#[test]
fn treasure_activate_rejects_unknown_instance() {
    let registry = treasure_trigger_registry();
    let mut inv = inventory_with_main_pack();
    let result = apply_treasure_activate(&mut inv, &registry, 999, true);
    assert!(
        result.is_err(),
        "activating a non-existent instance must be rejected"
    );
    assert!(inv.triggered_treasures.is_empty());
}

#[test]
fn treasure_activate_rejects_already_in_trigger_slot() {
    let registry = treasure_trigger_registry();
    let mut inv = inventory_with_main_pack();
    inv.triggered_treasures
        .push(make_test_item_instance(900, "test_treasure"));

    let result = apply_treasure_activate(&mut inv, &registry, 900, true);

    assert!(
        result.is_err(),
        "activating an instance already in the trigger slot must be rejected (idempotent)"
    );
    assert_eq!(
        inv.triggered_treasures.len(),
        1,
        "no duplicate pushed on reject"
    );
}

#[test]
fn treasure_deactivate_rejects_instance_not_in_trigger_slot() {
    let registry = treasure_trigger_registry();
    let mut inv = inventory_with_main_pack();
    inv.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: make_test_item_instance(1000, "test_treasure"),
    });

    // 该件在背包而非触发位 → 卸下应拒绝。
    let result = apply_treasure_activate(&mut inv, &registry, 1000, false);
    assert!(
        result.is_err(),
        "deactivating an instance that isn't in the trigger slot must be rejected"
    );
    assert_eq!(inv.containers[0].items.len(), 1, "container unchanged");
}

#[test]
fn treasure_deactivate_rejects_when_no_free_container_slot() {
    let registry = treasure_trigger_registry();
    let mut inv = make_empty_inventory();
    // main_pack 满（1x1 且已占用），无空位接收卸下的件。
    inv.containers.push(ContainerState {
        quick_access: false,
        id: MAIN_PACK_CONTAINER_ID.into(),
        name: MAIN_PACK_CONTAINER_ID.into(),
        rows: 1,
        cols: 1,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(1100, "test_armor"),
        }],

        owner_instance_id: None,
    });
    inv.triggered_treasures
        .push(make_test_item_instance(1200, "test_treasure"));

    let result = apply_treasure_activate(&mut inv, &registry, 1200, false);

    assert!(
        result.is_err(),
        "deactivating with no free container slot must be rejected (don't drop the item)"
    );
    assert_eq!(
        inv.triggered_treasures.len(),
        1,
        "treasure stays in the trigger slot when there's nowhere to put it"
    );
    assert_eq!(
        inv.triggered_treasures[0].instance_id, 1200,
        "the same treasure is retained, not lost"
    );
}

#[test]
fn borrow_helper_finds_item_in_equipped_and_hotbar() {
    let mut inv = make_empty_inventory();
    inv.equipped.insert(
        "main_hand".to_string(),
        SlotContents::held_single(make_test_item_instance(7, "talisman")),
    );
    inv.hotbar[0] = Some(make_test_item_instance(8, "pill"));
    assert_eq!(
        inventory_item_by_instance_borrow(&inv, 7)
            .unwrap()
            .template_id,
        "talisman"
    );
    assert_eq!(
        inventory_item_by_instance_borrow(&inv, 8)
            .unwrap()
            .template_id,
        "pill"
    );
}

#[test]
fn transfer_all_contents_moves_containers_equipped_hotbar_and_bone_coins() {
    let mut from = make_empty_inventory();
    from.revision = InventoryRevision(12);
    from.bone_coins = 9;
    from.containers.push(ContainerState {
        quick_access: false,
        id: MAIN_PACK_CONTAINER_ID.to_string(),
        name: "主背包".to_string(),
        rows: 2,
        cols: 2,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(1, "spirit_grass"),
        }],

        owner_instance_id: None,
    });
    from.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(make_test_item_instance(2, "iron_sword")),
    );
    from.hotbar[4] = Some(make_test_item_instance(3, "guyuan_pill"));

    let mut to = make_empty_inventory();
    to.revision = InventoryRevision(20);
    to.bone_coins = 5;
    to.containers.push(ContainerState {
        quick_access: false,
        id: MAIN_PACK_CONTAINER_ID.to_string(),
        name: "主背包".to_string(),
        rows: 3,
        cols: 3,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(9, "existing"),
        }],

        owner_instance_id: None,
    });

    let outcome = transfer_all_inventory_contents(&mut from, &mut to, &ItemRegistry::default());

    assert_eq!(outcome.items_moved, 3);
    assert_eq!(outcome.bone_coins_moved, 9);
    assert_eq!(outcome.from_revision, InventoryRevision(13));
    assert_eq!(outcome.to_revision, InventoryRevision(21));
    assert_eq!(from.bone_coins, 0);
    assert!(from
        .containers
        .iter()
        .all(|container| container.items.is_empty()));
    assert!(from.equipped.is_empty());
    assert!(from.hotbar.iter().all(Option::is_none));

    assert_eq!(to.bone_coins, 14);
    let moved_ids: Vec<u64> = to
        .containers
        .iter()
        .flat_map(|container| container.items.iter())
        .map(|placed| placed.instance.instance_id)
        .collect();
    for expected in [1, 2, 3, 9] {
        assert!(moved_ids.contains(&expected));
    }
}

#[test]
fn borrow_helper_returns_none_for_missing_instance() {
    let inv = make_empty_inventory();
    assert!(inventory_item_by_instance_borrow(&inv, 99).is_none());
}

// =========== plan-backpack-equip-v1 P0 — ContainerSpec + 背包槽测试 ===========

fn make_container_template(
    id: &str,
    equip_slot: &str,
    rows: u8,
    cols: u8,
    weight_capacity: f64,
) -> ItemTemplate {
    ItemTemplate {
        id: id.to_string(),
        display_name: id.to_string(),
        category: ItemCategory::Container,
        placeable: None,
        max_stack_count: 1,
        grid_w: 2,
        grid_h: 3,
        base_weight: 0.5,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 1.0,
        description: "test backpack".to_string(),
        effect: None,
        cast_duration_ms: DEFAULT_CAST_DURATION_MS,
        cooldown_ms: DEFAULT_COOLDOWN_MS,
        weapon_spec: None,
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
        readable_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: Some(ContainerSpec {
            quick_access: false,
            rows,
            cols,
            weight_capacity,
            equip_slot: equip_slot.to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: false,
            accept_filter: None,
        }),
        shield_spec: None,

        shelflife_profile: None,
        shelflife_track: None,
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    }
}

fn make_container_item(instance_id: u64, template_id: &str) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: template_id.to_string(),
        display_name: template_id.to_string(),
        grid_w: 2,
        grid_h: 3,
        weight: 0.5,
        rarity: ItemRarity::Common,
        description: String::new(),
        stack_count: 1,
        spirit_quality: 1.0,
        durability: 1.0,
        freshness: None,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    }
}

// ─── plan-tarkov-backpack-v1 套包修复（fix/tarkov-nest-persistence）— 随包保留 + 任意位置开包 ───

/// 构造带一个穿戴背包（chest 槽）+ 其 pack_<id> 容器（含 n 件内含物）的 inventory。
/// 返回 (inventory, registry, pack_instance_id, 内含物 instance_id 列表)。
fn setup_worn_pack_with_contents(
    contents: usize,
) -> (PlayerInventory, ItemRegistry, u64, Vec<u64>) {
    let pack_tpl = make_container_template("field_pack", EQUIP_SLOT_CHEST, 4, 4, 20.0);
    let mut tpls = HashMap::from([("field_pack".to_string(), pack_tpl)]);
    // 一个普通可放入的小物模板。
    let mut herb = make_container_template("spirit_herb", EQUIP_SLOT_CHEST, 1, 1, 0.0);
    herb.container_spec = None;
    herb.category = ItemCategory::Misc;
    herb.grid_w = 1;
    herb.grid_h = 1;
    tpls.insert("spirit_herb".to_string(), herb);
    let registry = ItemRegistry::from_map(tpls);

    let pack_id = 1000u64;
    let mut inv = make_empty_inventory();
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(pack_id, "field_pack")),
    );
    let mut content_ids = Vec::new();
    let mut items = Vec::new();
    for i in 0..contents {
        let cid = 2000 + i as u64;
        content_ids.push(cid);
        items.push(PlacedItemState {
            row: i as u8,
            col: 0,
            instance: make_test_item_instance(cid, "spirit_herb"),
        });
    }
    inv.containers.push(ContainerState {
        quick_access: false,
        id: container_id_for_worn_pack(pack_id),
        name: "野战背包".to_string(),
        rows: 4,
        cols: 4,
        items,
        owner_instance_id: Some(pack_id),
    });
    // 确保 body_pocket 存在（rebuild 会创建，但显式给出更清晰）。
    inv.containers.push(ContainerState {
        quick_access: false,
        id: BODY_POCKET_CONTAINER_ID.to_string(),
        name: "暗袋".to_string(),
        rows: BODY_POCKET_ROWS,
        cols: BODY_POCKET_COLS,
        items: Vec::new(),
        owner_instance_id: None,
    });
    (inv, registry, pack_id, content_ids)
}

/// 把 chest 槽的 worn 背包件移入 body_pocket（占其首格），模拟「卸包到身上」。
fn move_worn_pack_to_body_pocket(inv: &mut PlayerInventory, pack_id: u64) {
    let pack_item = inv
        .equipped
        .get_mut(EQUIP_SLOT_CHEST)
        .and_then(|s| s.worn.pop())
        .expect("pack should be worn");
    assert_eq!(pack_item.instance_id, pack_id);
    let bp = inv
        .containers
        .iter_mut()
        .find(|c| c.id == BODY_POCKET_CONTAINER_ID)
        .expect("body_pocket must exist");
    bp.items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: pack_item,
    });
}

fn pack_container_present(inv: &PlayerInventory, pack_id: u64) -> bool {
    inv.containers
        .iter()
        .any(|c| c.id == container_id_for_worn_pack(pack_id))
}

#[test]
fn rebuild_keeps_pack_container_when_pack_in_body_pocket() {
    let (mut inv, registry, pack_id, content_ids) = setup_worn_pack_with_contents(2);
    move_worn_pack_to_body_pocket(&mut inv, pack_id);

    let overflow = rebuild_containers_from_equipment(&mut inv, &registry);

    assert!(
        overflow.is_empty(),
        "背包仍在身上(body_pocket)时不应有任何 overflow，实际溢出 {} 件",
        overflow.len()
    );
    assert!(
        pack_container_present(&inv, pack_id),
        "背包在 body_pocket 时其 pack_{pack_id} 容器应保留（核心存活判据），实际容器列表={:?}",
        inv.containers.iter().map(|c| &c.id).collect::<Vec<_>>()
    );
    let pack_c = inv
        .containers
        .iter()
        .find(|c| c.id == container_id_for_worn_pack(pack_id))
        .unwrap();
    assert_eq!(
        pack_c.items.len(),
        content_ids.len(),
        "内含物应逐件守恒，不 spill"
    );
    for cid in &content_ids {
        assert!(
            pack_c.items.iter().any(|p| p.instance.instance_id == *cid),
            "内含物 instance {cid} 应原位保留在 pack 容器内"
        );
    }
}

#[test]
fn rebuild_treats_pack_in_pack_grid_as_orphan_cargo() {
    // P5「2 层封顶」交互：pack A 放进 worn pack B 的 grid（货物，层2）→ A 不属于携带面，
    // 其残留 pack_A 容器视为孤儿被清理、内含物 spill（不派生第 3 层可访问容器）。
    // 这把 retention（§2 携带面）与 2 层封顶（pack_* grid 内不展开）的交界锁死。
    let pack_tpl = make_container_template("field_pack", EQUIP_SLOT_CHEST, 4, 4, 20.0);
    let mut tpls = HashMap::from([("field_pack".to_string(), pack_tpl)]);
    let mut herb = make_container_template("spirit_herb", EQUIP_SLOT_CHEST, 1, 1, 0.0);
    herb.container_spec = None;
    herb.category = ItemCategory::Misc;
    herb.grid_w = 1;
    herb.grid_h = 1;
    tpls.insert("spirit_herb".to_string(), herb);
    let registry = ItemRegistry::from_map(tpls);
    let b_id = 100u64;
    let a_id = 200u64;
    let inner_id = 300u64;
    let mut inv = make_empty_inventory();
    // body_pocket 兜底 spill。
    inv.containers.push(ContainerState {
        quick_access: false,
        id: BODY_POCKET_CONTAINER_ID.to_string(),
        name: "暗袋".to_string(),
        rows: BODY_POCKET_ROWS,
        cols: BODY_POCKET_COLS,
        items: Vec::new(),
        owner_instance_id: None,
    });
    // pack B worn.
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(b_id, "field_pack")),
    );
    // pack B 容器，内含 pack A 件（货物）。
    inv.containers.push(ContainerState {
        quick_access: false,
        id: container_id_for_worn_pack(b_id),
        name: "B".to_string(),
        rows: 4,
        cols: 4,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_container_item(a_id, "field_pack"),
        }],
        owner_instance_id: Some(b_id),
    });
    // pack A 的残留容器（不该被 grid 内货物保有）。
    inv.containers.push(ContainerState {
        quick_access: false,
        id: container_id_for_worn_pack(a_id),
        name: "A".to_string(),
        rows: 4,
        cols: 4,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(inner_id, "spirit_herb"),
        }],
        owner_instance_id: Some(a_id),
    });

    let overflow = rebuild_containers_from_equipment(&mut inv, &registry);
    assert!(
        !pack_container_present(&inv, a_id),
        "2 层封顶：pack A 仅作为 pack B grid 内货物时不属携带面，其 pack_{a_id} 容器应被清理（不展开第 3 层）"
    );
    // 内含物不静默丢失：spill 进任一存活容器（body_pocket / pack_B）或 overflow。
    let in_inv = inventory_item_by_instance_borrow(&inv, inner_id).is_some();
    let in_overflow = overflow.iter().any(|i| i.instance_id == inner_id);
    assert!(
        in_inv || in_overflow,
        "孤儿 pack_A 内含物 {inner_id} 必须 spill 到存活容器或 overflow，禁止静默丢失"
    );
    // pack B 仍在身上，其容器保留（pack A 件仍作为货物留在 B 内）。
    assert!(pack_container_present(&inv, b_id), "worn pack B 容器应保留");
}

#[test]
fn rebuild_keeps_pack_container_when_pack_in_hotbar() {
    let (mut inv, registry, pack_id, _) = setup_worn_pack_with_contents(1);
    let pack_item = inv
        .equipped
        .get_mut(EQUIP_SLOT_CHEST)
        .and_then(|s| s.worn.pop())
        .unwrap();
    inv.hotbar[0] = Some(pack_item);

    let overflow = rebuild_containers_from_equipment(&mut inv, &registry);
    assert!(overflow.is_empty(), "背包在 hotbar 不应 spill");
    assert!(
        pack_container_present(&inv, pack_id),
        "背包在 hotbar 时 pack_{pack_id} 容器应保留"
    );
}

#[test]
fn rebuild_keeps_pack_container_when_pack_held() {
    let (mut inv, registry, pack_id, _) = setup_worn_pack_with_contents(1);
    let pack_item = inv
        .equipped
        .get_mut(EQUIP_SLOT_CHEST)
        .and_then(|s| s.worn.pop())
        .unwrap();
    // 移入手持位（另起一个手槽，held）。
    inv.equipped.insert(
        "main_hand".to_string(),
        SlotContents {
            worn: Vec::new(),
            held: Some(pack_item),
        },
    );

    let overflow = rebuild_containers_from_equipment(&mut inv, &registry);
    assert!(overflow.is_empty(), "背包在 held 不应 spill");
    assert!(
        pack_container_present(&inv, pack_id),
        "背包 held 时 pack_{pack_id} 容器应保留"
    );
}

#[test]
fn rebuild_spills_only_when_pack_left_player() {
    // 背包 detach 出 inventory（模拟丢地）后 rebuild → pack_<id> 孤儿、内含物 spill。
    let (mut inv, registry, pack_id, content_ids) = setup_worn_pack_with_contents(2);
    // 真离开玩家：从 worn 移除且不放回任何位置。
    inv.equipped.get_mut(EQUIP_SLOT_CHEST).unwrap().worn.clear();

    let overflow = rebuild_containers_from_equipment(&mut inv, &registry);

    assert!(
        !pack_container_present(&inv, pack_id),
        "背包真离开玩家后 pack_{pack_id} 容器应被移除（孤儿清理）"
    );
    // 内含物去向：spill 进 body_pocket（2×3=6 格够放 2 件）；不应 overflow。
    let bp = inv
        .containers
        .iter()
        .find(|c| c.id == BODY_POCKET_CONTAINER_ID)
        .unwrap();
    let spilled: Vec<u64> = bp.items.iter().map(|p| p.instance.instance_id).collect();
    for cid in &content_ids {
        assert!(
            spilled.contains(cid) || overflow.iter().any(|i| i.instance_id == *cid),
            "内含物 instance {cid} 必须 spill 到存活容器或进 overflow，禁止静默丢失"
        );
    }
}

#[test]
fn rebuild_spills_to_overflow_when_no_room() {
    // body_pocket 被占满 + 无其它存活容器 → 孤儿内含物进 overflow（连货掉地出口）。
    let pack_tpl = make_container_template("field_pack", EQUIP_SLOT_CHEST, 1, 1, 20.0);
    let mut tpls = HashMap::from([("field_pack".to_string(), pack_tpl)]);
    let mut blocker = make_container_template("blocker", EQUIP_SLOT_CHEST, 1, 1, 0.0);
    blocker.container_spec = None;
    blocker.category = ItemCategory::Misc;
    // body_pocket = BODY_POCKET_ROWS(2) 行 × BODY_POCKET_COLS(3) 列 → 占满需 w=3,h=2。
    blocker.grid_w = 3;
    blocker.grid_h = 2;
    tpls.insert("blocker".to_string(), blocker);
    let mut herb = make_container_template("spirit_herb", EQUIP_SLOT_CHEST, 1, 1, 0.0);
    herb.container_spec = None;
    herb.category = ItemCategory::Misc;
    herb.grid_w = 1;
    herb.grid_h = 1;
    tpls.insert("spirit_herb".to_string(), herb);
    let registry = ItemRegistry::from_map(tpls);

    let pack_id = 1000u64;
    let mut inv = make_empty_inventory();
    // pack 已不在身上（worn 为空），但其容器仍残留（孤儿）。
    let mut blocker_item = make_test_item_instance(5000, "blocker");
    blocker_item.grid_w = 3;
    blocker_item.grid_h = 2;
    inv.containers.push(ContainerState {
        quick_access: false,
        id: BODY_POCKET_CONTAINER_ID.to_string(),
        name: "暗袋".to_string(),
        rows: BODY_POCKET_ROWS,
        cols: BODY_POCKET_COLS,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: blocker_item,
        }],
        owner_instance_id: None,
    });
    inv.containers.push(ContainerState {
        quick_access: false,
        id: container_id_for_worn_pack(pack_id),
        name: "孤儿包".to_string(),
        rows: 1,
        cols: 1,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(6000, "spirit_herb"),
        }],
        owner_instance_id: Some(pack_id),
    });

    let overflow = rebuild_containers_from_equipment(&mut inv, &registry);
    assert!(
        overflow.iter().any(|i| i.instance_id == 6000),
        "body_pocket 被占满时孤儿内含物应进 overflow（非静默丢失），实际 overflow={:?}",
        overflow.iter().map(|i| i.instance_id).collect::<Vec<_>>()
    );
}

#[test]
fn find_pack_instances_anywhere_finds_all_positions() {
    let pack_tpl = make_container_template("field_pack", EQUIP_SLOT_CHEST, 4, 4, 20.0);
    let registry = ItemRegistry::from_map(HashMap::from([("field_pack".to_string(), pack_tpl)]));
    let mut inv = make_empty_inventory();
    // worn
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(1, "field_pack")),
    );
    // held
    inv.equipped.insert(
        "main_hand".to_string(),
        SlotContents {
            worn: Vec::new(),
            held: Some(make_container_item(2, "field_pack")),
        },
    );
    // hotbar
    inv.hotbar[3] = Some(make_container_item(3, "field_pack"));
    // body_pocket（携带面）
    inv.containers.push(ContainerState {
        quick_access: false,
        id: BODY_POCKET_CONTAINER_ID.to_string(),
        name: "暗袋".to_string(),
        rows: BODY_POCKET_ROWS,
        cols: BODY_POCKET_COLS,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_container_item(4, "field_pack"),
        }],
        owner_instance_id: None,
    });
    // 嵌套：worn pack(1) 的 grid 内放第 5 个 pack —— 货物，NOT 携带面（2 层封顶）。
    inv.containers.push(ContainerState {
        quick_access: false,
        id: container_id_for_worn_pack(1),
        name: "pack1".to_string(),
        rows: 4,
        cols: 4,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_container_item(5, "field_pack"),
        }],
        owner_instance_id: Some(1),
    });

    let found: Vec<u64> = find_pack_instances_anywhere(&inv, &registry)
        .map(|(i, _)| i.instance_id)
        .collect();
    assert_eq!(
        found.len(),
        4,
        "携带面=worn+held+hotbar+body_pocket 应命中 4 件背包，实际命中={found:?}"
    );
    for id in [1u64, 2, 3, 4] {
        assert!(
            found.contains(&id),
            "应命中携带面 instance {id}，实际={found:?}"
        );
    }
    assert!(
        !found.contains(&5),
        "pack_* grid 内的货物背包件(5)不属携带面（2 层封顶），不应命中，实际={found:?}"
    );
}

#[test]
fn validate_move_accepts_deposit_when_pack_in_body_pocket() {
    use crate::schema::inventory::InventoryLocationV1;
    let (mut inv, registry, pack_id, _) = setup_worn_pack_with_contents(0);
    move_worn_pack_to_body_pocket(&mut inv, pack_id);
    let deposit = make_test_item_instance(9001, "spirit_herb");
    let to = InventoryLocationV1::Container {
        container_id: container_id_for_worn_pack(pack_id),
        row: 0,
        col: 0,
    };
    let from = InventoryLocationV1::Hotbar { index: 0 };
    let res = validate_move_semantics(&registry, &inv, &deposit, &from, &to);
    assert!(
        res.is_ok(),
        "背包在 body_pocket（仍在身上）时应允许往其 pack 容器拖入内含物，实际={res:?}"
    );
}

#[test]
fn validate_move_rejects_deposit_when_pack_dropped() {
    use crate::schema::inventory::InventoryLocationV1;
    let (mut inv, registry, pack_id, _) = setup_worn_pack_with_contents(0);
    // 背包真离开玩家：worn 清空、不放回任何位置（但 pack 容器残留模拟尚未 rebuild）。
    inv.equipped.get_mut(EQUIP_SLOT_CHEST).unwrap().worn.clear();
    let deposit = make_test_item_instance(9001, "spirit_herb");
    let to = InventoryLocationV1::Container {
        container_id: container_id_for_worn_pack(pack_id),
        row: 0,
        col: 0,
    };
    let from = InventoryLocationV1::Hotbar { index: 0 };
    let res = validate_move_semantics(&registry, &inv, &deposit, &from, &to);
    assert!(res.is_err(), "背包已离开玩家时拖入内含物应被拒");
    let err = res.unwrap_err();
    assert!(
        matches!(
            err,
            InventoryMoveRejectReason::PackDetached { owner_instance_id }
                if owner_instance_id == pack_id
        ),
        "拒绝原因应为 PackDetached{{owner_instance_id: {pack_id}}} 以便定位，实际={err:?}"
    );
}

#[test]
fn validate_move_still_rejects_buried_worn_item() {
    use crate::schema::inventory::InventoryLocationV1;
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1};
    // worn 栈两层，移动被压住的底层件 → 仍 Err（回归 LIFO 门未被误放宽）。
    let armor = make_container_template("plate", EQUIP_SLOT_CHEST, 1, 1, 0.0);
    let mut armor = armor;
    armor.container_spec = None;
    armor.category = ItemCategory::Armor;
    let registry = ItemRegistry::from_map(HashMap::from([("plate".to_string(), armor)]));
    let mut inv = make_empty_inventory();
    let bottom = make_test_item_instance(10, "plate");
    let top = make_test_item_instance(11, "plate");
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents {
            worn: vec![bottom.clone(), top],
            held: None,
        },
    );
    let from = InventoryLocationV1::Equip {
        slot: EquipSlotV1::Chest,
        state: EquipStateV1::Worn,
    };
    let to = InventoryLocationV1::Hotbar { index: 0 };
    let res = validate_move_semantics(&registry, &inv, &bottom, &from, &to);
    assert!(
        res.is_err(),
        "被上层压住的 worn 件移动应仍被 LIFO 门拒绝（放宽门控不应波及此处）"
    );
}

#[test]
fn compute_max_weight_ignores_pack_in_body_pocket() {
    let (mut inv, registry, pack_id, _) = setup_worn_pack_with_contents(0);
    // worn 时含 20.0 加成。
    let worn_cap = compute_max_weight(&inv, &registry);
    assert_eq!(
        worn_cap,
        BASE_CARRY_CAPACITY + 20.0,
        "穿戴背包应提供 weight_capacity 加成"
    );
    move_worn_pack_to_body_pocket(&mut inv, pack_id);
    let pocket_cap = compute_max_weight(&inv, &registry);
    assert_eq!(
        pocket_cap, BASE_CARRY_CAPACITY,
        "背包在 body_pocket 时不提供负重加成（compute_max_weight 故意仍仅 worn）"
    );
}

#[test]
fn rebuild_keeps_empty_pack_container_in_body_pocket() {
    // 边界：空背包卸入 body_pocket，容器保留、无 overflow。
    let (mut inv, registry, pack_id, _) = setup_worn_pack_with_contents(0);
    move_worn_pack_to_body_pocket(&mut inv, pack_id);
    let overflow = rebuild_containers_from_equipment(&mut inv, &registry);
    assert!(overflow.is_empty());
    assert!(
        pack_container_present(&inv, pack_id),
        "空背包卸入 body_pocket，其 pack 容器仍应保留"
    );
}

#[test]
fn discard_worn_pack_spills_contents_and_clears_orphan() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
    // 模拟 handle_inventory_discard 的组合：discard worn 背包 → rebuild_and_drop_overflow。
    let (mut inv, registry, pack_id, content_ids) = setup_worn_pack_with_contents(2);
    let mut dropped = DroppedLootRegistry::default();
    let from = InventoryLocationV1::Equip {
        slot: EquipSlotV1::Chest,
        state: EquipStateV1::Worn,
    };
    // 1. discard 背包本体（从 worn 拿走、进 dropped registry）。
    discard_inventory_item_to_dropped_loot(
        &mut inv,
        &mut dropped,
        [0.0, 64.0, 0.0],
        DimensionKind::Overworld,
        pack_id,
        &from,
    )
    .expect("discard worn pack should succeed");
    assert!(
        dropped.entries.contains_key(&pack_id),
        "背包本体应进掉落物登记"
    );
    // 2. 补 rebuild（§5 handler 逻辑）：孤儿 pack 容器内含物 spill→掉地。
    let spilled = rebuild_and_drop_overflow(
        &mut inv,
        &registry,
        &mut dropped,
        [0.0, 64.0, 0.0],
        DimensionKind::Overworld,
    );
    assert!(
        !pack_container_present(&inv, pack_id),
        "discard + rebuild 后 pack_{pack_id} 孤儿容器不应残留 inventory（防 #736 重置 loadout）"
    );
    // 内含物去向：spill 进 body_pocket（仍在 inventory）或进 dropped（overflow）。
    for cid in &content_ids {
        let in_inv = inventory_item_by_instance_borrow(&inv, *cid).is_some();
        let in_dropped = dropped.entries.contains_key(cid);
        assert!(
            in_inv || in_dropped,
            "内含物 instance {cid} 必须在 inventory 或掉落物中，禁止静默丢失 (spilled overflow ids={spilled:?})"
        );
    }
}

#[test]
fn attrition_exempt_container_marks_inner_instance_exempt() {
    let mut sealed_bag = make_container_template("sealed_bag", EQUIP_SLOT_CHEST, 2, 2, 10.0);
    sealed_bag
        .container_spec
        .as_mut()
        .expect("sealed_bag should have container spec")
        .attrition_exempt = true;
    let registry = ItemRegistry::from_map(HashMap::from([("sealed_bag".to_string(), sealed_bag)]));

    let mut inv = make_empty_inventory();
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(1000, "sealed_bag")),
    );
    inv.containers.push(ContainerState {
        quick_access: false,
        id: container_id_for_worn_pack(1000),
        name: "封灵背包".to_string(),
        rows: 2,
        cols: 2,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(1001, "spirit_herb"),
        }],

        owner_instance_id: None,
    });

    assert!(
        inventory_instance_container_attrition_exempt(&inv, &registry, 1001),
        "封灵容器内物品应按 instance_id 识别为搬运磨损豁免"
    );
}

#[test]
fn ordinary_container_does_not_mark_inner_instance_exempt() {
    let ordinary_bag = make_container_template("ordinary_bag", EQUIP_SLOT_CHEST, 2, 2, 10.0);
    let registry =
        ItemRegistry::from_map(HashMap::from([("ordinary_bag".to_string(), ordinary_bag)]));

    let mut inv = make_empty_inventory();
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(1002, "ordinary_bag")),
    );
    inv.containers.push(ContainerState {
        quick_access: false,
        id: container_id_for_worn_pack(1002),
        name: "普通背包".to_string(),
        rows: 2,
        cols: 2,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(1003, "spirit_herb"),
        }],

        owner_instance_id: None,
    });

    assert!(
        !inventory_instance_container_attrition_exempt(&inv, &registry, 1003),
        "普通容器内物品不应误判为搬运磨损豁免"
    );
}

#[test]
fn equipped_or_hotbar_instance_is_not_container_attrition_exempt() {
    let mut sealed_bag = make_container_template("sealed_bag", EQUIP_SLOT_CHEST, 2, 2, 10.0);
    sealed_bag
        .container_spec
        .as_mut()
        .expect("sealed_bag should have container spec")
        .attrition_exempt = true;
    let registry = ItemRegistry::from_map(HashMap::from([("sealed_bag".to_string(), sealed_bag)]));

    let mut inv = make_empty_inventory();
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(1004, "sealed_bag")),
    );
    inv.hotbar[0] = Some(make_test_item_instance(1005, "spirit_herb"));

    assert!(
        !inventory_instance_container_attrition_exempt(&inv, &registry, 1004),
        "装备槽中的容器物品自身不应因自身 container_spec 被判为内含物豁免"
    );
    assert!(
        !inventory_instance_container_attrition_exempt(&inv, &registry, 1005),
        "hotbar 物品不在封灵容器内，不应获得容器级豁免"
    );
}

// P0.1 — ContainerSpec TOML 解析：正例

#[test]
fn parse_container_spec_valid_chest() {
    // 决议 #17：背包 equip_slot 指向身体槽（chest）。
    let raw = ContainerSpecToml {
        quick_access: false,
        rows: 7,
        cols: 5,
        weight_capacity: 30.0,
        equip_slot: EQUIP_SLOT_CHEST.to_string(),
        durability_cost_per_op: 0.001,
        attrition_exempt: false,
        accept: None,
    };
    let spec =
        parse_container_spec(raw, Path::new("<test>"), "chest_pack_item").expect("should parse");
    assert_eq!(spec.rows, 7, "rows mismatch");
    assert_eq!(spec.cols, 5, "cols mismatch");
    assert!(
        (spec.weight_capacity - 30.0).abs() < f64::EPSILON,
        "weight_capacity mismatch"
    );
    assert_eq!(spec.equip_slot, EQUIP_SLOT_CHEST, "equip_slot mismatch");
    assert!((spec.durability_cost_per_op - 0.001).abs() < f64::EPSILON);
    assert!(!spec.attrition_exempt, "普通背包默认不应豁免搬运磨损");
    assert_eq!(
        spec.accept_filter, None,
        "旧 TOML 未声明 accept 时应保持 accept_filter=None，避免破坏既有容器"
    );
}

#[test]
fn parse_container_spec_valid_head() {
    let raw = ContainerSpecToml {
        quick_access: false,
        rows: 3,
        cols: 3,
        weight_capacity: 10.0,
        equip_slot: EQUIP_SLOT_HEAD.to_string(),
        durability_cost_per_op: 0.0,
        attrition_exempt: false,
        accept: None,
    };
    let spec =
        parse_container_spec(raw, Path::new("<test>"), "head_pack_item").expect("should parse");
    assert_eq!(spec.equip_slot, EQUIP_SLOT_HEAD);
}

#[test]
fn parse_container_spec_valid_legs() {
    let raw = ContainerSpecToml {
        quick_access: false,
        rows: 4,
        cols: 3,
        weight_capacity: 20.0,
        equip_slot: EQUIP_SLOT_LEGS.to_string(),
        durability_cost_per_op: 0.0,
        attrition_exempt: true,
        accept: None,
    };
    let spec =
        parse_container_spec(raw, Path::new("<test>"), "legs_pack_item").expect("should parse");
    assert_eq!(spec.equip_slot, EQUIP_SLOT_LEGS);
    assert!(
        spec.attrition_exempt,
        "显式封灵容器应保留 attrition_exempt=true"
    );
}

// ── worn-tab/quickbar plan — ContainerSpec.quick_access 解析 + 透传 + rebuild 回填 ──

#[test]
fn parse_container_spec_propagates_quick_access_true() {
    let raw = ContainerSpecToml {
        quick_access: true,
        rows: 2,
        cols: 3,
        weight_capacity: 5.0,
        equip_slot: EQUIP_SLOT_CHEST.to_string(),
        durability_cost_per_op: 0.0,
        attrition_exempt: false,
        accept: None,
    };
    let spec = parse_container_spec(raw, Path::new("<test>"), "quick_pouch").expect("should parse");
    assert!(
        spec.quick_access,
        "TOML quick_access=true 应透传到 ContainerSpec.quick_access=true（未来快捷腰包靠此生效）"
    );
}

#[test]
fn container_spec_toml_quick_access_defaults_false_when_absent() {
    // 旧 TOML 不写 quick_access → serde(default) 读为 false，不退化、不报 unknown_fields。
    let toml_src = r#"
        rows = 3
        cols = 3
        weight_capacity = 10.0
        equip_slot = "chest"
    "#;
    let raw: ContainerSpecToml =
        toml::from_str(toml_src).expect("旧 TOML（无 quick_access 键）应解析成功");
    assert!(
        !raw.quick_access,
        "缺省 quick_access 应为 false（旧档兼容）"
    );
    let spec = parse_container_spec(raw, Path::new("<test>"), "legacy_pack").expect("should parse");
    assert!(
        !spec.quick_access,
        "未声明 quick_access 的容器应 quick_access=false，普通背包内物品不可入快捷栏"
    );
}

#[test]
fn container_spec_toml_quick_access_true_parses_from_toml() {
    let toml_src = r#"
        rows = 2
        cols = 3
        weight_capacity = 4.0
        equip_slot = "chest"
        quick_access = true
    "#;
    let raw: ContainerSpecToml =
        toml::from_str(toml_src).expect("含 quick_access=true 的 TOML 应解析成功");
    assert!(raw.quick_access, "TOML quick_access=true 应读为 true");
}

#[test]
fn rebuild_backfills_pack_quick_access_from_owner_template() {
    // owner 背包件模板 quick_access=true → rebuild 把派生 pack_<id> 容器 quick_access 置 true；
    // 普通背包模板（false）→ pack 容器 quick_access=false。验证「字段就位、TOML 即生效」承诺链路。
    let mut quick_tpl = make_container_template("quick_pack", EQUIP_SLOT_CHEST, 3, 3, 10.0);
    quick_tpl.container_spec.as_mut().unwrap().quick_access = true;
    let plain_tpl = make_container_template("plain_pack", EQUIP_SLOT_LEGS, 3, 3, 10.0);
    let registry = ItemRegistry::from_map(HashMap::from([
        ("quick_pack".to_string(), quick_tpl),
        ("plain_pack".to_string(), plain_tpl),
    ]));

    let quick_id = 5001u64;
    let plain_id = 5002u64;
    let mut inv = make_empty_inventory();
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(quick_id, "quick_pack")),
    );
    inv.equipped.insert(
        EQUIP_SLOT_LEGS.to_string(),
        SlotContents::worn_single(make_container_item(plain_id, "plain_pack")),
    );

    let overflow = rebuild_containers_from_equipment(&mut inv, &registry);
    assert!(overflow.is_empty(), "无内含物不应 overflow");

    let quick_container = inv
        .containers
        .iter()
        .find(|c| c.id == container_id_for_worn_pack(quick_id))
        .expect("quick_pack 应有派生 pack 容器");
    assert!(
        quick_container.quick_access,
        "owner 模板 quick_access=true → pack 容器 quick_access 应回填 true"
    );

    let plain_container = inv
        .containers
        .iter()
        .find(|c| c.id == container_id_for_worn_pack(plain_id))
        .expect("plain_pack 应有派生 pack 容器");
    assert!(
        !plain_container.quick_access,
        "owner 模板 quick_access=false → pack 容器 quick_access 应保持 false"
    );
}

#[test]
fn rebuild_keeps_body_pocket_quick_access_false_in_state() {
    // body_pocket 的快捷资格由 snapshot 特判（恒 true），ContainerState 缓存位本身保持 false。
    let registry = ItemRegistry::from_map(HashMap::new());
    let mut inv = make_empty_inventory();
    let _ = rebuild_containers_from_equipment(&mut inv, &registry);
    let bp = inv
        .containers
        .iter()
        .find(|c| c.id == BODY_POCKET_CONTAINER_ID)
        .expect("rebuild 应保证 body_pocket 存在");
    assert!(
        !bp.quick_access,
        "body_pocket 的 ContainerState.quick_access 缓存位应为 false（资格由 snapshot 特判 true）"
    );
}

// ── plan-container-filter-and-completion-v1 P0 — category + accept_filter 数据模型 ──

#[test]
fn parse_item_category_accepts_container_filter_categories_and_aliases() {
    let path = Path::new("<inline-items.toml>");
    let cases = [
        ("mineral", ItemCategory::Mineral),
        ("ore", ItemCategory::Mineral),
        ("MINERAL", ItemCategory::Mineral),
        (" anqi ", ItemCategory::Anqi),
        ("hidden_weapon", ItemCategory::Anqi),
        ("liquid", ItemCategory::Liquid),
    ];
    for (raw, expected) in cases {
        let parsed = parse_item_category(raw, path, "filter_case")
            .expect("container filter category should parse");
        assert_eq!(
            parsed, expected,
            "期望 category `{raw}` 解析为 {expected:?}，实际得到 {parsed:?}"
        );
    }
}

#[test]
fn item_category_container_filter_variants_serde_roundtrip() {
    for category in [
        ItemCategory::Mineral,
        ItemCategory::Anqi,
        ItemCategory::Liquid,
    ] {
        let json = serde_json::to_string(&category).expect("ItemCategory should serialize");
        let parsed: ItemCategory =
            serde_json::from_str(&json).expect("ItemCategory should deserialize");
        assert_eq!(
            parsed, category,
            "期望 {category:?} serde roundtrip 后保持同一变体"
        );
    }
}

#[test]
fn container_filter_categories_have_pinned_default_stack_counts() {
    assert_eq!(
        default_max_stack_count_for_category(ItemCategory::Mineral),
        64
    );
    assert_eq!(default_max_stack_count_for_category(ItemCategory::Anqi), 32);
    assert_eq!(
        default_max_stack_count_for_category(ItemCategory::Liquid),
        16
    );
}

#[test]
fn parse_container_spec_accept_empty_is_explicit_all_accepting_filter() {
    let raw = ContainerSpecToml {
        quick_access: false,
        rows: 2,
        cols: 2,
        weight_capacity: 0.0,
        equip_slot: EQUIP_SLOT_CHEST.to_string(),
        durability_cost_per_op: 0.0,
        attrition_exempt: false,
        accept: Some(Vec::new()),
    };
    let spec = parse_container_spec(raw, Path::new("<test>"), "open_pouch")
        .expect("explicit empty accept list should parse");
    assert_eq!(
        spec.accept_filter,
        Some(Vec::new()),
        "显式 accept=[] 应保留为 Some(empty)，语义仍由 item_passes_filter 判定为全收"
    );
}

#[test]
fn parse_container_spec_accept_parses_categories_and_template_prefix() {
    let raw = ContainerSpecToml {
        quick_access: false,
        rows: 3,
        cols: 3,
        weight_capacity: 0.0,
        equip_slot: EQUIP_SLOT_CHEST.to_string(),
        durability_cost_per_op: 0.0,
        attrition_exempt: false,
        accept: Some(vec![
            "mineral".to_string(),
            "prefix:anqi_".to_string(),
            "hidden_weapon".to_string(),
        ]),
    };
    let spec = parse_container_spec(raw, Path::new("<test>"), "filtered_pouch")
        .expect("category and prefix filters should parse");
    assert_eq!(
        spec.accept_filter,
        Some(vec![
            ContainerAcceptFilter::Category(ItemCategory::Mineral),
            ContainerAcceptFilter::TemplatePrefix("anqi_".to_string()),
            ContainerAcceptFilter::Category(ItemCategory::Anqi),
        ])
    );
}

#[test]
fn parse_container_spec_accept_trims_template_prefix_payload() {
    for raw_prefix in ["prefix:anqi_", "prefix: anqi_"] {
        let raw = ContainerSpecToml {
            quick_access: false,
            rows: 3,
            cols: 3,
            weight_capacity: 0.0,
            equip_slot: EQUIP_SLOT_CHEST.to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: false,
            accept: Some(vec![raw_prefix.to_string()]),
        };
        let spec = parse_container_spec(raw, Path::new("<test>"), "prefix_pouch")
            .expect("prefix accept entry should parse with optional whitespace");
        assert_eq!(
            spec.accept_filter,
            Some(vec![ContainerAcceptFilter::TemplatePrefix(
                "anqi_".to_string()
            )]),
            "prefix accept entry `{raw_prefix}` 应归一化为无空白前缀"
        );
    }
}

#[test]
fn parse_container_spec_rejects_invalid_accept_entries() {
    for (accept, expected_fragment) in [
        (vec!["unknown_category".to_string()], "unknown category"),
        (vec!["".to_string()], "empty container.accept entry"),
        (vec!["prefix:".to_string()], "empty container.accept prefix"),
    ] {
        let raw = ContainerSpecToml {
            quick_access: false,
            rows: 2,
            cols: 2,
            weight_capacity: 0.0,
            equip_slot: EQUIP_SLOT_CHEST.to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: false,
            accept: Some(accept),
        };
        let err = parse_container_spec(raw, Path::new("<test>"), "bad_accept")
            .expect_err("invalid accept entry should fail");
        assert!(
            err.contains(expected_fragment),
            "期望错误包含 `{expected_fragment}`，实际错误为 {err}"
        );
    }
}

#[test]
fn item_passes_filter_treats_none_and_empty_as_all_accepting() {
    let registry = registry_from_templates(vec![test_template(
        "ordinary_herb",
        ItemCategory::Herb,
        1,
        1,
        64,
    )]);
    let item = make_test_item_instance(42, "ordinary_herb");
    assert!(item_passes_filter(&None, &item, &registry));
    assert!(item_passes_filter(&Some(Vec::new()), &item, &registry));
}

#[test]
fn item_passes_filter_matches_category_template_prefix_and_union() {
    let registry = registry_from_templates(vec![
        test_template("ore_iron", ItemCategory::Mineral, 1, 1, 64),
        test_template("spirit_herb", ItemCategory::Herb, 1, 1, 64),
        test_template("water_skin_filled", ItemCategory::Liquid, 1, 1, 16),
        test_template("anqi_bone_chip", ItemCategory::Anqi, 1, 1, 32),
    ]);
    let mineral_filter = Some(vec![ContainerAcceptFilter::Category(ItemCategory::Mineral)]);
    assert!(item_passes_filter(
        &mineral_filter,
        &make_test_item_instance(1, "ore_iron"),
        &registry
    ));
    assert!(!item_passes_filter(
        &mineral_filter,
        &make_test_item_instance(2, "spirit_herb"),
        &registry
    ));

    let prefix_filter = Some(vec![ContainerAcceptFilter::TemplatePrefix(
        "anqi_".to_string(),
    )]);
    assert!(item_passes_filter(
        &prefix_filter,
        &make_test_item_instance(3, "anqi_bone_chip"),
        &registry
    ));
    assert!(!item_passes_filter(
        &prefix_filter,
        &make_test_item_instance(4, "ore_iron"),
        &registry
    ));

    let union_filter = Some(vec![
        ContainerAcceptFilter::Category(ItemCategory::Mineral),
        ContainerAcceptFilter::Category(ItemCategory::Liquid),
    ]);
    assert!(item_passes_filter(
        &union_filter,
        &make_test_item_instance(5, "water_skin_filled"),
        &registry
    ));
    assert!(!item_passes_filter(
        &union_filter,
        &make_test_item_instance(6, "spirit_herb"),
        &registry
    ));
}

#[test]
fn container_spec_accept_filter_serde_roundtrip() {
    let spec = ContainerSpec {
        quick_access: false,
        rows: 2,
        cols: 3,
        weight_capacity: 4.0,
        equip_slot: EQUIP_SLOT_CHEST.to_string(),
        durability_cost_per_op: 0.0,
        attrition_exempt: false,
        accept_filter: Some(vec![
            ContainerAcceptFilter::Category(ItemCategory::Mineral),
            ContainerAcceptFilter::TemplatePrefix("anqi_".to_string()),
        ]),
    };
    let json = serde_json::to_string(&spec).expect("ContainerSpec should serialize");
    let parsed: ContainerSpec =
        serde_json::from_str(&json).expect("ContainerSpec should deserialize");
    assert_eq!(parsed, spec);
}

#[test]
fn legacy_container_spec_json_without_accept_filter_defaults_to_none() {
    let json = r#"{
        "rows": 2,
        "cols": 3,
        "weight_capacity": 4.0,
        "equip_slot": "waist_pouch",
        "durability_cost_per_op": 0.0,
        "attrition_exempt": false
    }"#;
    let parsed: ContainerSpec =
        serde_json::from_str(json).expect("legacy ContainerSpec should deserialize");
    assert_eq!(
        parsed.accept_filter, None,
        "旧存档/协议缺 accept_filter 时必须默认 None"
    );
    let serialized = serde_json::to_string(&parsed).expect("legacy ContainerSpec should serialize");
    assert!(
        !serialized.contains("accept_filter"),
        "accept_filter=None 序列化时应省略字段，避免旧 JSON 形状变成 null：{serialized}"
    );
}

// P0.1 — ContainerSpec TOML 解析：反例

#[test]
fn parse_container_spec_rejects_rows_zero() {
    let raw = ContainerSpecToml {
        quick_access: false,
        rows: 0,
        cols: 4,
        weight_capacity: 10.0,
        equip_slot: EQUIP_SLOT_CHEST.to_string(),
        durability_cost_per_op: 0.0,
        attrition_exempt: false,
        accept: None,
    };
    let err = parse_container_spec(raw, Path::new("<test>"), "bad_rows")
        .expect_err("should fail with rows=0");
    assert!(err.contains("rows"), "expected rows error, got: {err}");
}

#[test]
fn parse_container_spec_rejects_rows_overflow() {
    let raw = ContainerSpecToml {
        quick_access: false,
        rows: 17,
        cols: 4,
        weight_capacity: 10.0,
        equip_slot: EQUIP_SLOT_CHEST.to_string(),
        durability_cost_per_op: 0.0,
        attrition_exempt: false,
        accept: None,
    };
    let err = parse_container_spec(raw, Path::new("<test>"), "bad_rows_overflow")
        .expect_err("rows > 16 should fail");
    assert!(err.contains("rows"), "expected rows error, got: {err}");
}

#[test]
fn parse_container_spec_rejects_cols_zero() {
    let raw = ContainerSpecToml {
        quick_access: false,
        rows: 4,
        cols: 0,
        weight_capacity: 10.0,
        equip_slot: EQUIP_SLOT_CHEST.to_string(),
        durability_cost_per_op: 0.0,
        attrition_exempt: false,
        accept: None,
    };
    let err =
        parse_container_spec(raw, Path::new("<test>"), "bad_cols").expect_err("cols=0 should fail");
    assert!(err.contains("cols"), "expected cols error, got: {err}");
}

#[test]
fn parse_container_spec_rejects_negative_weight_capacity() {
    let raw = ContainerSpecToml {
        quick_access: false,
        rows: 4,
        cols: 4,
        weight_capacity: -1.0,
        equip_slot: EQUIP_SLOT_CHEST.to_string(),
        durability_cost_per_op: 0.0,
        attrition_exempt: false,
        accept: None,
    };
    let err = parse_container_spec(raw, Path::new("<test>"), "bad_weight")
        .expect_err("negative weight_capacity should fail");
    assert!(
        err.contains("weight_capacity"),
        "expected weight_capacity error, got: {err}"
    );
}

#[test]
fn parse_container_spec_rejects_invalid_equip_slot() {
    // 决议 #17：背包 equip_slot 只接受身体槽（head/chest/legs/feet）；
    // 旧 back_pack 专属槽已删，作为 equip_slot 现属非法。
    let raw = ContainerSpecToml {
        quick_access: false,
        rows: 4,
        cols: 4,
        weight_capacity: 10.0,
        equip_slot: "back_pack".to_string(),
        durability_cost_per_op: 0.0,
        attrition_exempt: false,
        accept: None,
    };
    let err = parse_container_spec(raw, Path::new("<test>"), "bad_slot")
        .expect_err("invalid equip_slot should fail");
    assert!(
        err.contains("equip_slot"),
        "expected equip_slot error, got: {err}"
    );
}

#[test]
fn parse_container_spec_rejects_negative_durability_cost() {
    let raw = ContainerSpecToml {
        quick_access: false,
        rows: 4,
        cols: 4,
        weight_capacity: 10.0,
        equip_slot: EQUIP_SLOT_CHEST.to_string(),
        durability_cost_per_op: -0.1,
        attrition_exempt: false,
        accept: None,
    };
    let err = parse_container_spec(raw, Path::new("<test>"), "bad_dur_cost")
        .expect_err("negative durability_cost_per_op should fail");
    assert!(
        err.contains("durability_cost_per_op"),
        "expected durability_cost_per_op error, got: {err}"
    );
}

// P0.2 — 常量存在性（决议 #17 删除 back_pack/waist_pouch/chest_satchel 专属槽常量后，
// 仅保留 body_pocket / 基础负重等仍存活的常量断言）。

#[test]
fn body_pocket_and_base_carry_constants_are_correct() {
    assert_eq!(BODY_POCKET_CONTAINER_ID, "body_pocket");
    assert_eq!(BODY_POCKET_ROWS, 2);
    assert_eq!(BODY_POCKET_COLS, 3);
    assert!((BASE_CARRY_CAPACITY - 15.0).abs() < f64::EPSILON);
}

// P0.3 — rebuild_containers_from_equipment 行为

#[test]
fn rebuild_containers_creates_body_pocket_when_missing() {
    let registry = ItemRegistry::from_map(HashMap::new());
    let mut inv = make_empty_inventory();
    assert!(
        !inv.containers
            .iter()
            .any(|c| c.id == BODY_POCKET_CONTAINER_ID),
        "should not have body_pocket initially"
    );

    rebuild_containers_from_equipment(&mut inv, &registry);

    assert!(
        inv.containers
            .iter()
            .any(|c| c.id == BODY_POCKET_CONTAINER_ID),
        "body_pocket should be created"
    );
    let pocket = inv
        .containers
        .iter()
        .find(|c| c.id == BODY_POCKET_CONTAINER_ID)
        .unwrap();
    assert_eq!(
        pocket.rows, BODY_POCKET_ROWS,
        "body_pocket rows should be {BODY_POCKET_ROWS}"
    );
    assert_eq!(
        pocket.cols, BODY_POCKET_COLS,
        "body_pocket cols should be {BODY_POCKET_COLS}"
    );
}

#[test]
fn rebuild_containers_preserves_existing_body_pocket() {
    let registry = ItemRegistry::from_map(HashMap::new());
    let mut inv = make_empty_inventory();
    inv.containers.push(ContainerState {
        quick_access: false,
        id: BODY_POCKET_CONTAINER_ID.to_string(),
        name: "暗袋".to_string(),
        rows: BODY_POCKET_ROWS,
        cols: BODY_POCKET_COLS,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(77, "herb_a"),
        }],

        owner_instance_id: None,
    });

    rebuild_containers_from_equipment(&mut inv, &registry);

    let pocket = inv
        .containers
        .iter()
        .find(|c| c.id == BODY_POCKET_CONTAINER_ID)
        .unwrap();
    assert_eq!(
        pocket.items.len(),
        1,
        "existing body_pocket item should be preserved"
    );
}

#[test]
fn rebuild_containers_adds_container_for_equipped_backpack() {
    let backpack_template = make_container_template("large_backpack", EQUIP_SLOT_CHEST, 7, 5, 30.0);
    let registry = ItemRegistry::from_map(HashMap::from([(
        "large_backpack".to_string(),
        backpack_template,
    )]));

    let mut inv = make_empty_inventory();
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(200, "large_backpack")),
    );

    rebuild_containers_from_equipment(&mut inv, &registry);

    let pack_id = container_id_for_worn_pack(200);
    assert!(
        inv.containers.iter().any(|c| c.id == pack_id),
        "pack_<instance_id> container should be created when equipped to chest worn"
    );
    let bp = inv.containers.iter().find(|c| c.id == pack_id).unwrap();
    assert_eq!(bp.rows, 7, "rows should match container_spec");
    assert_eq!(bp.cols, 5, "cols should match container_spec");
}

// plan-tarkov-backpack-v1 P5（决议 #1）— 嵌套深度 2 层封顶固化回归。
// 深度上限 = 2 层：worn 背包 → 其 grid → 物品。放进 grid 的背包件**不**被
// `rebuild_containers_from_equipment` 展开为第 3 层可访问容器——rebuild 只扫身体槽
// worn 层（`worn_container_items`），grid 内的 PlacedItemState 永不被派生容器。
// 数据模型天然封顶；本测试锁住该不变量，任何「也展开 grid 内背包件」的回归立即撞红。
#[test]
fn rebuild_does_not_expand_container_item_placed_inside_grid_two_layer_cap() {
    let outer = make_container_template("outer_pack", EQUIP_SLOT_CHEST, 3, 3, 12.0);
    let inner = make_container_template("inner_pouch", EQUIP_SLOT_CHEST, 2, 2, 6.0);
    let registry = ItemRegistry::from_map(HashMap::from([
        ("outer_pack".to_string(), outer),
        ("inner_pouch".to_string(), inner),
    ]));

    let mut inv = make_empty_inventory();
    // 第 1 层：外层背包穿在 chest worn 层 → 第 2 层：其 grid 容器。
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(200, "outer_pack")),
    );
    rebuild_containers_from_equipment(&mut inv, &registry);
    let outer_id = container_id_for_worn_pack(200);
    assert!(
        inv.containers.iter().any(|c| c.id == outer_id),
        "穿戴的外层背包（worn 层）应派生可访问容器 {outer_id}"
    );

    // 把另一个背包件（inner_pouch，本身带 container_spec）放进外层背包的 grid——
    // 它是 grid 里的一件物品，不是穿在身上的 worn 件。
    let inner_instance_id = 201;
    {
        let outer_container = inv
            .containers
            .iter_mut()
            .find(|c| c.id == outer_id)
            .expect("外层容器应存在");
        outer_container.items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: make_container_item(inner_instance_id, "inner_pouch"),
        });
    }

    rebuild_containers_from_equipment(&mut inv, &registry);

    // 关键不变量：grid 内的背包件不得被展开为第 3 层容器。
    let inner_id = container_id_for_worn_pack(inner_instance_id);
    assert!(
        !inv.containers.iter().any(|c| c.id == inner_id),
        "嵌套深度封顶=2：grid 内背包件不得派生可访问容器（不应出现 {inner_id}）"
    );
    // inner_pouch 仍原样作为普通物品留在外层 grid，未被抽走。
    let outer_container = inv
        .containers
        .iter()
        .find(|c| c.id == outer_id)
        .expect("外层容器应仍存在");
    assert!(
        outer_container
            .items
            .iter()
            .any(|p| p.instance.instance_id == inner_instance_id),
        "grid 内的背包件应原样保留为 PlacedItemState，不被展开抽走"
    );
    // pack_<id> 容器恰好 1 个（仅外层 worn 件），grid 内背包件不计入。
    let pack_like = inv
        .containers
        .iter()
        .filter(|c| worn_pack_instance_from_container_id(&c.id).is_some())
        .count();
    assert_eq!(
        pack_like, 1,
        "只应有 1 个 pack_<id> 容器（外层 worn 背包），grid 内背包件不派生第 3 层"
    );
}

#[test]
fn rebuild_containers_removes_empty_container_when_unequipped() {
    let backpack_template = make_container_template("large_backpack", EQUIP_SLOT_CHEST, 7, 5, 30.0);
    let registry = ItemRegistry::from_map(HashMap::from([(
        "large_backpack".to_string(),
        backpack_template,
    )]));

    let mut inv = make_empty_inventory();
    // 预置一个 pack_<id> 容器但没有对应穿戴背包件（孤儿）。
    let pack_id = container_id_for_worn_pack(200);
    inv.containers.push(ContainerState {
        quick_access: false,
        id: pack_id.clone(),
        name: "大背包".to_string(),
        rows: 7,
        cols: 5,
        items: Vec::new(),
        owner_instance_id: None,
    });

    rebuild_containers_from_equipment(&mut inv, &registry);

    assert!(
        !inv.containers.iter().any(|c| c.id == pack_id),
        "empty pack container should be removed when unequipped"
    );
}

// Bug C（真机回归）— 孤儿非空 pack_<id> 容器（无对应穿戴背包件）必须**清理**，不得残留可
// access：先把内含物 spill 到存活容器（body_pocket 兜底），再移除容器。物品有去向不丢。
// 旧行为（`|| !c.items.is_empty()` 保留孤儿）= 丢背包后仍能从孤儿容器取物 = 数据/玩法 bug。
#[test]
fn rebuild_containers_spills_orphan_items_and_removes_container() {
    let registry = ItemRegistry::from_map(HashMap::new());
    let mut inv = make_empty_inventory();
    // body_pocket 作为 spill 兜底落点（2×3 = 6 格，足够收 1 件）。
    inv.containers.push(ContainerState {
        quick_access: false,
        id: BODY_POCKET_CONTAINER_ID.to_string(),
        name: "暗袋".to_string(),
        rows: BODY_POCKET_ROWS,
        cols: BODY_POCKET_COLS,
        items: Vec::new(),
        owner_instance_id: None,
    });
    // 孤儿 pack_200：装着 herb(instance_id=55) 但 equipped 里无 instance_id=200 的穿戴背包件。
    let pack_id = container_id_for_worn_pack(200);
    inv.containers.push(ContainerState {
        quick_access: false,
        id: pack_id.clone(),
        name: "大背包".to_string(),
        rows: 7,
        cols: 5,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(55, "herb"),
        }],

        owner_instance_id: None,
    });

    let overflow = rebuild_containers_from_equipment(&mut inv, &registry);

    // 孤儿容器消失（不可再 access）。
    assert!(
        !inv.containers.iter().any(|c| c.id == pack_id),
        "孤儿 pack_<id> 容器必须移除（丢背包后不允许残留可 access 的孤儿容器）"
    );
    // herb 应 spill 进 body_pocket（有去向、不丢、不进 overflow）。
    assert!(
        overflow.is_empty(),
        "body_pocket 有空位时不应产生 overflow；实际 overflow={:?}",
        overflow.iter().map(|i| &i.template_id).collect::<Vec<_>>()
    );
    let pocket = inv
        .containers
        .iter()
        .find(|c| c.id == BODY_POCKET_CONTAINER_ID)
        .expect("body_pocket 应存在");
    assert_eq!(
        pocket
            .items
            .iter()
            .map(|p| p.instance.instance_id)
            .collect::<Vec<_>>(),
        vec![55],
        "孤儿容器里的 herb(55) 应 spill 进 body_pocket，物品不丢"
    );
}

// Bug C（边界）— spill 落点全满时，放不下的孤儿物品上抛 overflow（由调用方掉落），仍不丢、不残留孤儿。
#[test]
fn rebuild_containers_orphan_items_overflow_when_no_room() {
    let registry = ItemRegistry::from_map(HashMap::new());
    let mut inv = make_empty_inventory();
    // 不提供任何存活容器（无 body_pocket、无 live pack）——rebuild 会建一个空 body_pocket(2×3)。
    // 孤儿 pack 里塞 7 件 1×1，body_pocket 只能收 6 件 → 第 7 件 overflow。
    let pack_id = container_id_for_worn_pack(200);
    let mut items = Vec::new();
    for i in 0..7u8 {
        items.push(PlacedItemState {
            row: i,
            col: 0,
            instance: make_test_item_instance(1000 + u64::from(i), "herb"),
        });
    }
    inv.containers.push(ContainerState {
        quick_access: false,
        id: pack_id.clone(),
        name: "大背包".to_string(),
        rows: 7,
        cols: 5,
        items,
        owner_instance_id: None,
    });

    let overflow = rebuild_containers_from_equipment(&mut inv, &registry);

    assert!(
        !inv.containers.iter().any(|c| c.id == pack_id),
        "孤儿容器必须移除"
    );
    // body_pocket(2×3=6) 收 6 件，第 7 件无处安放 → overflow（不丢，调用方掉落）。
    assert_eq!(
        overflow.len(),
        1,
        "body_pocket 6 格满后第 7 件应进 overflow；实际 overflow.len()={}",
        overflow.len()
    );
    let pocket = inv
        .containers
        .iter()
        .find(|c| c.id == BODY_POCKET_CONTAINER_ID)
        .expect("body_pocket 应被建出");
    assert_eq!(pocket.items.len(), 6, "body_pocket 应收满 6 件");
    // 总物品数守恒：6 spill + 1 overflow = 7 原始件，无丢失。
    assert_eq!(
        pocket.items.len() + overflow.len(),
        7,
        "spill + overflow 必须 = 原孤儿容器物品数（物品守恒，不丢数据）"
    );
}

// Bug C（不误删）— 仍有对应穿戴背包件的非空 pack_<id> 容器（自洽，非孤儿）必须原样保留。
#[test]
fn rebuild_containers_preserves_nonempty_container_with_live_backpack() {
    let backpack_template = make_container_template("large_backpack", EQUIP_SLOT_CHEST, 7, 5, 30.0);
    let registry = ItemRegistry::from_map(HashMap::from([(
        "large_backpack".to_string(),
        backpack_template,
    )]));
    let mut inv = make_empty_inventory();
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(200, "large_backpack")),
    );
    let pack_id = container_id_for_worn_pack(200);
    inv.containers.push(ContainerState {
        quick_access: false,
        id: pack_id.clone(),
        name: "大背包".to_string(),
        rows: 7,
        cols: 5,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(55, "herb"),
        }],

        owner_instance_id: None,
    });

    let overflow = rebuild_containers_from_equipment(&mut inv, &registry);

    assert!(overflow.is_empty(), "自洽容器不应触发 spill/overflow");
    let pack = inv
        .containers
        .iter()
        .find(|c| c.id == pack_id)
        .expect("有对应穿戴背包件的容器必须保留");
    assert_eq!(
        pack.items
            .iter()
            .map(|p| p.instance.instance_id)
            .collect::<Vec<_>>(),
        vec![55],
        "自洽容器内含物原样保留，不被 spill 走"
    );
}

// P0.4 — compute_max_weight 计算

#[test]
fn compute_max_weight_no_backpacks_returns_base() {
    let registry = ItemRegistry::from_map(HashMap::new());
    let inv = make_empty_inventory();
    let w = compute_max_weight(&inv, &registry);
    assert!(
        (w - BASE_CARRY_CAPACITY).abs() < f64::EPSILON,
        "expected BASE_CARRY_CAPACITY={BASE_CARRY_CAPACITY}, got {w}"
    );
}

#[test]
fn compute_max_weight_adds_equipped_backpack_capacity() {
    let backpack_template = make_container_template("large_backpack", EQUIP_SLOT_CHEST, 7, 5, 30.0);
    let registry = ItemRegistry::from_map(HashMap::from([(
        "large_backpack".to_string(),
        backpack_template,
    )]));

    let mut inv = make_empty_inventory();
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(300, "large_backpack")),
    );

    let w = compute_max_weight(&inv, &registry);
    assert!(
        (w - (BASE_CARRY_CAPACITY + 30.0)).abs() < f64::EPSILON,
        "expected BASE + 30.0 = {}, got {w}",
        BASE_CARRY_CAPACITY + 30.0
    );
}

/// plan-tarkov-backpack-v1 P1 pin（固化决议 #3）：穿戴背包件自重**不**额外占
/// max_weight 上限——`compute_max_weight = BASE + Σ weight_capacity`，背包件自重
/// 已在 `current_weight` 侧计一次（equipped），不在 max 侧二次扣减。
/// 此处把背包件自重设得很大（50.0）并断言 max 仍只 = BASE + capacity，与自重无关。
#[test]
fn compute_max_weight_worn_pack_self_weight_not_added_to_max() {
    // weight_capacity=30.0；下面把实际穿戴件自重设成 50.0（远大于容量）以坐实
    // 「自重不参与 max 公式」。
    let backpack_template = make_container_template("large_backpack", EQUIP_SLOT_CHEST, 7, 5, 30.0);
    let registry = ItemRegistry::from_map(HashMap::from([(
        "large_backpack".to_string(),
        backpack_template,
    )]));

    let mut inv = make_empty_inventory();
    let mut pack = make_container_item(1000, "large_backpack");
    pack.weight = 50.0; // 自重远大于 capacity，若被错误计入 max 则会撞红。
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(pack),
    );

    let w = compute_max_weight(&inv, &registry);
    let expected = BASE_CARRY_CAPACITY + 30.0; // 仅 capacity，与背包件自重(50.0)无关。
    assert!(
        (w - expected).abs() < f64::EPSILON,
        "期望 max = BASE({BASE_CARRY_CAPACITY}) + capacity(30.0) = {expected}，与背包自重(50.0)无关（决议 #3：自重已在 current 侧计、不占 max），实际 {w}——若 ≈ {} 说明自重被错误加进 max",
        expected + 50.0
    );
}

// 决议 #17：背包无专属槽，多个背包件骑在身体槽 worn 层；compute_max_weight 累加全部
// 身体槽 worn 层带 container_spec 的件的 weight_capacity（受各槽 worn_cap：chest=3/legs=3）。
#[test]
fn compute_max_weight_sums_multiple_worn_packs() {
    let bp = make_container_template("large_backpack", EQUIP_SLOT_CHEST, 7, 5, 30.0);
    let wp = make_container_template("waist_pouch", EQUIP_SLOT_CHEST, 3, 3, 10.0);
    let cs = make_container_template("chest_satchel", EQUIP_SLOT_LEGS, 3, 4, 20.0);
    let registry = ItemRegistry::from_map(HashMap::from([
        ("large_backpack".to_string(), bp),
        ("waist_pouch".to_string(), wp),
        ("chest_satchel".to_string(), cs),
    ]));

    let mut inv = make_empty_inventory();
    // chest worn 两层（cap=3 内）：large_backpack + waist_pouch。
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents {
            worn: vec![
                make_container_item(1, "large_backpack"),
                make_container_item(2, "waist_pouch"),
            ],
            held: None,
        },
    );
    // legs worn 一层：chest_satchel。
    inv.equipped.insert(
        EQUIP_SLOT_LEGS.to_string(),
        SlotContents::worn_single(make_container_item(3, "chest_satchel")),
    );

    let w = compute_max_weight(&inv, &registry);
    let expected = BASE_CARRY_CAPACITY + 30.0 + 10.0 + 20.0;
    assert!(
        (w - expected).abs() < f64::EPSILON,
        "expected {expected}, got {w}"
    );
}

#[test]
fn rebuild_containers_updates_max_weight() {
    let backpack_template = make_container_template("large_backpack", EQUIP_SLOT_CHEST, 7, 5, 30.0);
    let registry = ItemRegistry::from_map(HashMap::from([(
        "large_backpack".to_string(),
        backpack_template,
    )]));

    let mut inv = make_empty_inventory();
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(400, "large_backpack")),
    );
    inv.max_weight = 100.0; // stale value

    rebuild_containers_from_equipment(&mut inv, &registry);

    assert!(
        (inv.max_weight - (BASE_CARRY_CAPACITY + 30.0)).abs() < f64::EPSILON,
        "max_weight should be updated by rebuild, got {}",
        inv.max_weight
    );
}

// P0.5 — validate_move_semantics 背包槽校验

fn make_backpack_registry_and_inventory() -> (ItemRegistry, PlayerInventory) {
    // 决议 #17：背包 equip_slot 指向身体槽。large_backpack→chest，
    // legs_pack→legs（供「错槽」用例），chest_bag→chest。
    let bp_template = make_container_template("large_backpack", EQUIP_SLOT_CHEST, 7, 5, 30.0);
    let wp_template = make_container_template("legs_pack", EQUIP_SLOT_LEGS, 3, 3, 10.0);
    let cs_template = make_container_template("chest_bag", EQUIP_SLOT_CHEST, 3, 4, 20.0);
    let registry = ItemRegistry::from_map(HashMap::from([
        ("large_backpack".to_string(), bp_template),
        ("legs_pack".to_string(), wp_template),
        ("chest_bag".to_string(), cs_template),
    ]));
    let inv = PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(0),
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 5,
            cols: 7,
            items: Vec::new(),
            owner_instance_id: None,
        }],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 100.0,
    };
    (registry, inv)
}

// 决议 #17：背包件 equip_slot=chest，装入 chest worn 应成功。
#[test]
fn validate_move_semantics_accepts_container_equip_to_chest_worn() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
    let (registry, inv) = make_backpack_registry_and_inventory();
    let item = make_container_item(501, "large_backpack");
    let from = InventoryLocationV1::Container {
        container_id: "main_pack".to_string(),
        row: 0,
        col: 0,
    };
    let to = InventoryLocationV1::Equip {
        slot: EquipSlotV1::Chest,
        state: EquipStateV1::Worn,
    };
    assert!(
        validate_move_semantics(&registry, &inv, &item, &from, &to).is_ok(),
        "equipping large_backpack (equip_slot=chest) to chest worn should succeed"
    );
}

// 非盔甲/非伪皮/非容器的杂项物品装 chest worn → 拒绝。
#[test]
fn validate_move_semantics_rejects_non_container_item_to_chest_worn() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
    let (registry, inv) = make_backpack_registry_and_inventory();
    // Use a misc item (no container_spec, not armor, not false skin).
    let misc_template = test_template("iron_ore", ItemCategory::Misc, 1, 1, 16);
    let registry_with_misc = ItemRegistry::from_map({
        let mut m = registry.templates.clone();
        m.insert("iron_ore".to_string(), misc_template);
        m
    });
    let item = make_test_item_instance(502, "iron_ore");
    let from = InventoryLocationV1::Container {
        container_id: "main_pack".to_string(),
        row: 0,
        col: 0,
    };
    let to = InventoryLocationV1::Equip {
        slot: EquipSlotV1::Chest,
        state: EquipStateV1::Worn,
    };
    let err = validate_move_semantics(&registry_with_misc, &inv, &item, &from, &to)
        .expect_err("non-container/non-armor misc item should not equip to chest worn");
    assert!(
        matches!(err, InventoryMoveRejectReason::EquipCategoryMismatch),
        "expected body-slot type rejection, got: {err:?}"
    );
}

// 背包 equip_slot=legs，装入 chest worn → equip_slot 不匹配，拒绝。
#[test]
fn validate_move_semantics_rejects_wrong_slot_backpack() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
    let (registry, inv) = make_backpack_registry_and_inventory();
    // legs_pack has equip_slot=legs; try to equip to chest worn.
    let item = make_container_item(503, "legs_pack");
    let from = InventoryLocationV1::Container {
        container_id: "main_pack".to_string(),
        row: 0,
        col: 0,
    };
    let to = InventoryLocationV1::Equip {
        slot: EquipSlotV1::Chest,
        state: EquipStateV1::Worn,
    };
    let err = validate_move_semantics(&registry, &inv, &item, &from, &to)
        .expect_err("legs_pack should not equip to chest worn");
    assert!(
        matches!(
            err,
            InventoryMoveRejectReason::PackEquipSlotMismatch { ref expected_slot }
                if expected_slot == "legs"
        ),
        "expected equip_slot mismatch error, got: {err:?}"
    );
}

#[test]
fn validate_move_semantics_rejects_container_to_hotbar() {
    use crate::schema::inventory::InventoryLocationV1;
    let (registry, inv) = make_backpack_registry_and_inventory();
    let item = make_container_item(504, "large_backpack");
    let from = InventoryLocationV1::Container {
        container_id: "main_pack".to_string(),
        row: 0,
        col: 0,
    };
    let to = InventoryLocationV1::Hotbar { index: 0 };
    let err = validate_move_semantics(&registry, &inv, &item, &from, &to)
        .expect_err("container item should not move to hotbar");
    assert!(
        matches!(
            err,
            InventoryMoveRejectReason::ForbiddenInHotbar {
                category: ItemCategory::Container
            }
        ),
        "expected hotbar error, got: {err:?}"
    );
}

// plan-tarkov-backpack-v1 P0（交付物 #3 / 测试清单）— 非空拒卸硬门已移除：
// 穿戴背包件即使其 pack_<instance_id> 容器非空，也允许整体卸下（塔科夫式套包）。
// 内含物 spill/overflow 由 handle_inventory_move 卸包分支接管（见 e2e_*）。
#[test]
fn validate_move_semantics_allows_unequip_backpack_when_container_nonempty() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
    let (registry, mut inv) = make_backpack_registry_and_inventory();
    // Equip the backpack into chest worn.
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(505, "large_backpack")),
    );
    // 该背包件的 pack_505 容器非空。
    inv.containers.push(ContainerState {
        quick_access: false,
        id: container_id_for_worn_pack(505),
        name: "大背包".to_string(),
        rows: 7,
        cols: 5,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_test_item_instance(99, "herb"),
        }],
        owner_instance_id: Some(505),
    });

    let item = make_container_item(505, "large_backpack");
    let from = InventoryLocationV1::Equip {
        slot: EquipSlotV1::Chest,
        state: EquipStateV1::Worn,
    };
    let to = InventoryLocationV1::Container {
        container_id: "main_pack".to_string(),
        row: 0,
        col: 0,
    };
    assert!(
        validate_move_semantics(&registry, &inv, &item, &from, &to).is_ok(),
        "非空背包应允许整体卸下（非空拒卸硬门已移除）；内含物 spill/overflow 在 \
         handle_inventory_move 卸包分支处理，而非在校验层拒绝"
    );
}

#[test]
fn validate_move_semantics_allows_unequip_backpack_when_container_empty() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
    let (registry, mut inv) = make_backpack_registry_and_inventory();
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(506, "large_backpack")),
    );
    // pack_506 容器为空。
    inv.containers.push(ContainerState {
        quick_access: false,
        id: container_id_for_worn_pack(506),
        name: "大背包".to_string(),
        rows: 7,
        cols: 5,
        items: Vec::new(),
        owner_instance_id: None,
    });

    let item = make_container_item(506, "large_backpack");
    let from = InventoryLocationV1::Equip {
        slot: EquipSlotV1::Chest,
        state: EquipStateV1::Worn,
    };
    let to = InventoryLocationV1::Container {
        container_id: "main_pack".to_string(),
        row: 0,
        col: 0,
    };
    assert!(
        validate_move_semantics(&registry, &inv, &item, &from, &to).is_ok(),
        "unequipping backpack with empty container should succeed"
    );
}

// ===== plan-tarkov-backpack-v1 P0 测试清单（≥9，含 e2e） =====

/// 交付物 #2 — rebuild 创建/刷新 `pack_<id>` 容器时写 owner_instance_id = Some(instance_id)。
#[test]
fn rebuild_sets_owner_instance_id_on_pack_container() {
    let (registry, mut inv) = make_backpack_registry_and_inventory();
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(701, "large_backpack")),
    );

    let overflow = rebuild_containers_from_equipment(&mut inv, &registry);
    assert!(
        overflow.is_empty(),
        "穿背包后 rebuild 不应产生 overflow（新建空容器）；实际 {} 件",
        overflow.len()
    );

    let pack_id = container_id_for_worn_pack(701);
    let pack = inv
        .containers
        .iter()
        .find(|c| c.id == pack_id)
        .unwrap_or_else(|| panic!("rebuild 后应存在 `{pack_id}` 容器"));
    assert_eq!(
        pack.owner_instance_id,
        Some(701),
        "因为 rebuild 必须把 `{pack_id}` 容器的 owner_instance_id 写为穿戴背包件的 instance_id(701)，\
         实际 = {:?}",
        pack.owner_instance_id
    );
}

/// 交付物 #4 / 决议 #2 — 卸下非空背包：内含物 spill 进存活容器。
/// 直测生产 seam `rebuild_and_drop_overflow`（handle_inventory_move 卸包分支调用同一函数）。
#[test]
fn unequip_nonempty_backpack_spills_contents_into_other_container() {
    let (registry, mut inv) = make_backpack_registry_and_inventory();
    // 装上背包件（large_backpack, pack_801）并放两件内含物。
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(801, "large_backpack")),
    );
    inv.containers.push(ContainerState {
        quick_access: false,
        id: container_id_for_worn_pack(801),
        name: "大背包".to_string(),
        rows: 7,
        cols: 5,
        items: vec![
            PlacedItemState {
                row: 0,
                col: 0,
                instance: make_test_item_instance(10, "spirit_herb"),
            },
            PlacedItemState {
                row: 1,
                col: 0,
                instance: make_test_item_instance(11, "bone_dust"),
            },
        ],
        owner_instance_id: Some(801),
    });

    // 模拟卸下：把背包件从 chest worn 移走（apply_inventory_move 已 detach），
    // 此时 pack_801 变孤儿。handle_inventory_move 卸包分支随即调 rebuild_and_drop_overflow。
    let removed = inv
        .equipped
        .get_mut(EQUIP_SLOT_CHEST)
        .and_then(|s| (!s.worn.is_empty()).then(|| s.worn.remove(0)));
    assert!(removed.is_some(), "应能从 chest worn 移除背包件");

    let mut dropped = DroppedLootRegistry::default();
    let dropped_ids = rebuild_and_drop_overflow(
        &mut inv,
        &registry,
        &mut dropped,
        [0.0, 64.0, 0.0],
        DimensionKind::Overworld,
    );

    // main_pack（5×7=35 格）能容下 spill → 不应有 overflow 掉落。
    assert!(
        dropped_ids.is_empty(),
        "main_pack 空且足够大，spill 应全部进容器、无 overflow 掉落；实际掉落 {dropped_ids:?}"
    );
    // 孤儿 pack_801 已被移除（不可 access）。
    assert!(
        !inv.containers
            .iter()
            .any(|c| c.id == container_id_for_worn_pack(801)),
        "卸下背包后其孤儿 pack_801 容器应被 rebuild 移除"
    );
    // 两件内含物 spill 进 main_pack。
    let main = inv
        .containers
        .iter()
        .find(|c| c.id == "main_pack")
        .expect("main_pack 存在");
    let main_ids: Vec<u64> = main.items.iter().map(|p| p.instance.instance_id).collect();
    assert!(
        main_ids.contains(&10) && main_ids.contains(&11),
        "spirit_herb(10) 与 bone_dust(11) 应 spill 进 main_pack；实际 main_pack ids = {main_ids:?}"
    );
}

/// 交付物 #4 / 决议 #2 红线 — 目标容器满时，overflow 内含物**转掉落物**（DroppedLootRegistry），
/// 禁止静默丢失（断言掉落 count 守恒、非空、instance 守恒）。
#[test]
fn unequip_nonempty_backpack_overflow_drops_items_not_lost() {
    // 构造：唯一存活容器极小（1×1=1 格），背包内含 3 件 → 1 件 spill，2 件 overflow 掉落。
    let bp = make_container_template("small_pack", EQUIP_SLOT_CHEST, 3, 3, 10.0);
    let registry = ItemRegistry::from_map(HashMap::from([("small_pack".to_string(), bp)]));
    let mut inv = make_empty_inventory();
    // body_pocket（2×3=6 格）预填满——否则 rebuild 兜底创建空 body_pocket 会吸收全部 spill、
    // 不产生 overflow。填满后 spill 只能去 tiny（1 格），其余 overflow 掉落。
    inv.containers.push(ContainerState {
        quick_access: false,
        id: BODY_POCKET_CONTAINER_ID.to_string(),
        name: "暗袋".to_string(),
        rows: BODY_POCKET_ROWS,
        cols: BODY_POCKET_COLS,
        items: (0..6)
            .map(|i| PlacedItemState {
                row: (i / 3) as u8,
                col: (i % 3) as u8,
                instance: make_test_item_instance(200 + i as u64, "filler"),
            })
            .collect(),
        owner_instance_id: None,
    });
    // spill 容器：tiny 1×1。
    inv.containers.push(ContainerState {
        quick_access: false,
        id: "tiny".to_string(),
        name: "tiny".to_string(),
        rows: 1,
        cols: 1,
        items: Vec::new(),
        owner_instance_id: None,
    });
    // 穿上 small_pack（pack_900），内含 3 件 1×1。
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(900, "small_pack")),
    );
    inv.containers.push(ContainerState {
        quick_access: false,
        id: container_id_for_worn_pack(900),
        name: "small".to_string(),
        rows: 3,
        cols: 3,
        items: vec![
            PlacedItemState {
                row: 0,
                col: 0,
                instance: make_test_item_instance(20, "a"),
            },
            PlacedItemState {
                row: 0,
                col: 1,
                instance: make_test_item_instance(21, "b"),
            },
            PlacedItemState {
                row: 0,
                col: 2,
                instance: make_test_item_instance(22, "c"),
            },
        ],
        owner_instance_id: Some(900),
    });

    // 卸下：移走背包件。
    inv.equipped
        .get_mut(EQUIP_SLOT_CHEST)
        .map(|s| s.worn.remove(0));

    let mut dropped = DroppedLootRegistry::default();
    let dropped_ids = rebuild_and_drop_overflow(
        &mut inv,
        &registry,
        &mut dropped,
        [5.0, 64.0, 5.0],
        DimensionKind::Overworld,
    );

    // tiny 仅 1 格 → 1 件 spill 进 tiny，2 件 overflow 掉落（守恒：3 = 1 + 2）。
    assert_eq!(
        dropped_ids.len(),
        2,
        "tiny 容器仅 1 格，3 件内含物中 1 件 spill、2 件应转掉落物（守恒，禁止静默丢失）；实际掉落 {dropped_ids:?}"
    );
    assert_eq!(
        dropped.entries.len(),
        2,
        "DroppedLootRegistry 应含 2 条掉落条目（overflow 全部入世界，不丢失）"
    );
    // 掉落物 + spill 件 = 原 3 件（instance 守恒，无凭空消失）。
    let tiny = inv.containers.iter().find(|c| c.id == "tiny").unwrap();
    let mut all_ids: Vec<u64> = tiny.items.iter().map(|p| p.instance.instance_id).collect();
    all_ids.extend(dropped.entries.keys().copied());
    all_ids.sort_unstable();
    assert_eq!(
        all_ids,
        vec![20, 21, 22],
        "spill + 掉落必须守恒覆盖全部 3 件原内含物（20/21/22）；实际并集 = {all_ids:?}"
    );
    // 掉落条目的 item 实例非空且 dimension 正确。
    for id in &dropped_ids {
        let entry = dropped
            .entries
            .get(id)
            .unwrap_or_else(|| panic!("掉落 instance {id} 应在 registry"));
        assert_eq!(
            entry.dimension,
            DimensionKind::Overworld,
            "掉落物 dimension 应为玩家所在维度"
        );
        assert_eq!(
            entry.item.instance_id, *id,
            "掉落条目 item.instance_id 应与 key 一致（保留原 instance，不分配新 id）"
        );
    }
}

/// 交付物 #4 同步 — 穿背包路径触发 rebuild，`pack_<id>` 容器即时存在（P3 双击有容器可开）。
#[test]
fn equip_pack_creates_pack_container_via_rebuild() {
    let (registry, mut inv) = make_backpack_registry_and_inventory();
    // 穿上背包件后调 rebuild_and_drop_overflow（模拟 handle_inventory_move 穿包分支）。
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(950, "large_backpack")),
    );
    let mut dropped = DroppedLootRegistry::default();
    let dropped_ids = rebuild_and_drop_overflow(
        &mut inv,
        &registry,
        &mut dropped,
        [0.0, 64.0, 0.0],
        DimensionKind::Overworld,
    );
    assert!(
        dropped_ids.is_empty(),
        "穿包（新建空容器）不应产生 overflow 掉落；实际 {dropped_ids:?}"
    );
    let pack_id = container_id_for_worn_pack(950);
    let pack = inv
        .containers
        .iter()
        .find(|c| c.id == pack_id)
        .unwrap_or_else(|| panic!("穿包后 rebuild 应即时新建 `{pack_id}` 容器（P3 双击可开）"));
    assert_eq!(
        pack.owner_instance_id,
        Some(950),
        "穿包新建容器的 owner_instance_id 应为背包件 instance_id(950)"
    );
}

/// 交付物 #5 — 多背包 loadout：第一件复用占位、其余动态建容器，全部容器 id 正确。
#[test]
fn instantiate_remaps_all_worn_pack_placeholders() {
    // 两件 worn pack：chest + legs 各一。占位容器仅 `pack_grass_pouch` 一个 +
    // body_pocket（rebuild 兜底）。fixture 预置占位容器带一件预置物品，验证其不丢。
    let chest_pack = make_container_template("chest_pack", EQUIP_SLOT_CHEST, 3, 3, 10.0);
    let legs_pack = make_container_template("legs_pack", EQUIP_SLOT_LEGS, 3, 3, 8.0);
    let registry = ItemRegistry::from_map(HashMap::from([
        ("chest_pack".to_string(), chest_pack),
        ("legs_pack".to_string(), legs_pack),
    ]));

    let mut equipped: HashMap<String, SlotContents> = HashMap::new();
    equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(0, "chest_pack")),
    );
    equipped.insert(
        EQUIP_SLOT_LEGS.to_string(),
        SlotContents::worn_single(make_container_item(0, "legs_pack")),
    );

    // 占位容器（LOADOUT_PACK_PLACEHOLDER_CONTAINER_ID）携带一件预置物品。
    let loadout = LoadoutSpec {
        containers: vec![ContainerState {
            quick_access: false,
            id: LOADOUT_PACK_PLACEHOLDER_CONTAINER_ID.to_string(),
            name: "占位包".to_string(),
            rows: 3,
            cols: 3,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: make_test_item_instance(0, "preset_item"),
            }],
            owner_instance_id: None,
        }],
        equipped,
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 100.0,
    };

    let mut alloc = InventoryInstanceIdAllocator::new(2000);
    let inv = instantiate_inventory_from_loadout(&loadout, &mut alloc, &registry)
        .expect("instantiate 多背包 loadout");

    // 占位 id 不应残留。
    assert!(
        !inv.containers
            .iter()
            .any(|c| c.id == LOADOUT_PACK_PLACEHOLDER_CONTAINER_ID),
        "静态占位 `{LOADOUT_PACK_PLACEHOLDER_CONTAINER_ID}` 必须已重映射，不应残留"
    );

    // 收集运行时两件 worn pack 的 instance_id。
    let worn_pack_ids: Vec<u64> = inv
        .equipped
        .values()
        .flat_map(|s| s.worn.iter())
        .filter(|i| {
            registry
                .get(&i.template_id)
                .is_some_and(|t| t.container_spec.is_some())
        })
        .map(|i| i.instance_id)
        .collect();
    assert_eq!(
        worn_pack_ids.len(),
        2,
        "应有两件运行时 worn pack；实际 {worn_pack_ids:?}"
    );

    // 两件 worn pack 各自都应有对应 `pack_<id>` 容器、owner 正确。
    for inst_id in &worn_pack_ids {
        let expected = container_id_for_worn_pack(*inst_id);
        let c = inv
            .containers
            .iter()
            .find(|c| c.id == expected)
            .unwrap_or_else(|| {
                panic!(
                    "worn pack instance {inst_id} 应有容器 `{expected}`；实际 ids = {:?}",
                    inv.containers.iter().map(|c| &c.id).collect::<Vec<_>>()
                )
            });
        assert_eq!(
            c.owner_instance_id,
            Some(*inst_id),
            "容器 `{expected}` 的 owner_instance_id 应为 {inst_id}"
        );
    }

    // 占位预置物品仍在某个 pack 容器（第一件 worn pack 复用占位容器，物品不丢）。
    let preset_still_present = inv.containers.iter().any(|c| {
        c.items
            .iter()
            .any(|p| p.instance.template_id == "preset_item")
    });
    assert!(
        preset_still_present,
        "占位容器的预置物品（preset_item）在重映射后不应丢失"
    );
}

/// 单背包 loadout 不应强依赖旧占位容器：`body_pocket` 是唯一必需静态容器，
/// worn 背包件的 `pack_<instance_id>` 容器必须由实例化收尾 rebuild 派生。
#[test]
fn instantiate_single_worn_pack_without_placeholder_creates_runtime_container() {
    let chest_pack = make_container_template("chest_pack", EQUIP_SLOT_CHEST, 3, 3, 8.0);
    let registry = ItemRegistry::from_map(HashMap::from([("chest_pack".to_string(), chest_pack)]));

    let loadout = LoadoutSpec {
        containers: vec![ContainerState {
            quick_access: false,
            id: BODY_POCKET_CONTAINER_ID.to_string(),
            name: "贴身口袋".to_string(),
            rows: BODY_POCKET_ROWS,
            cols: BODY_POCKET_COLS,
            items: Vec::new(),
            owner_instance_id: None,
        }],
        equipped: HashMap::from([(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents::worn_single(make_container_item(0, "chest_pack")),
        )]),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 23.0,
    };

    let mut alloc = InventoryInstanceIdAllocator::new(3000);
    let inv = instantiate_inventory_from_loadout(&loadout, &mut alloc, &registry)
        .expect("single worn-pack loadout should instantiate");

    let pack_instance_id = inv
        .equipped
        .get(EQUIP_SLOT_CHEST)
        .and_then(|slot| slot.worn.first())
        .map(|item| item.instance_id)
        .expect("chest worn pack should exist");
    let expected_container_id = container_id_for_worn_pack(pack_instance_id);
    let pack = inv
        .containers
        .iter()
        .find(|container| container.id == expected_container_id)
        .unwrap_or_else(|| {
            panic!(
                "单背包 loadout 即使没有旧占位容器，也必须派生 `{expected_container_id}`；实际 ids = {:?}",
                inv.containers
                    .iter()
                    .map(|container| &container.id)
                    .collect::<Vec<_>>()
            )
        });

    assert_eq!(pack.owner_instance_id, Some(pack_instance_id));
    assert_eq!((pack.rows, pack.cols), (3, 3));
}

/// qi_physics 锚点 — 跨包移动 lingering_owner_qi 守恒（随 instance 走，不重算/复制/蒸发）。
#[test]
fn move_item_across_packs_preserves_lingering_owner_qi() {
    use crate::schema::inventory::InventoryLocationV1;
    // 自建 registry：两个 container 模板 + 一个 misc 物品模板（apply_inventory_move 校验需 registry 命中）。
    let chest_pack = make_container_template("chest_pack", EQUIP_SLOT_CHEST, 3, 3, 10.0);
    let legs_pack = make_container_template("legs_pack", EQUIP_SLOT_LEGS, 3, 3, 8.0);
    let mut spirit_dust = make_container_template("spirit_dust", EQUIP_SLOT_CHEST, 1, 1, 0.0);
    // spirit_dust 是普通可移动物品（非容器）：清掉 container_spec、改 Misc 类、1×1。
    spirit_dust.container_spec = None;
    spirit_dust.category = ItemCategory::Misc;
    spirit_dust.grid_w = 1;
    spirit_dust.grid_h = 1;
    let registry = ItemRegistry::from_map(HashMap::from([
        ("chest_pack".to_string(), chest_pack),
        ("legs_pack".to_string(), legs_pack),
        ("spirit_dust".to_string(), spirit_dust),
    ]));
    let mut inv = make_empty_inventory();
    // 两件 worn pack：chest（chest_pack, pack_1001）+ legs（legs_pack, pack_1002）。
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(1001, "chest_pack")),
    );
    inv.equipped.insert(
        EQUIP_SLOT_LEGS.to_string(),
        SlotContents::worn_single(make_container_item(1002, "legs_pack")),
    );
    // 两个 pack 容器都建好（rebuild 后 owner 正确）。
    let _ = rebuild_containers_from_equipment(&mut inv, &registry);

    // 在 pack_1001 放一件带 lingering_owner_qi 的物品。
    let mut item = make_test_item_instance(55, "spirit_dust");
    item.lingering_owner_qi = Some(LingeringQi {
        owner: "Kizun".to_string(),
        expire_at: 12_345,
    });
    let qi_before = item.lingering_owner_qi.clone();
    let pack1 = inv
        .containers
        .iter_mut()
        .find(|c| c.id == container_id_for_worn_pack(1001))
        .expect("pack_1001 存在");
    pack1.items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: item,
    });

    // 跨包移动：pack_1001 → pack_1002。
    let from = InventoryLocationV1::Container {
        container_id: container_id_for_worn_pack(1001),
        row: 0,
        col: 0,
    };
    let to = InventoryLocationV1::Container {
        container_id: container_id_for_worn_pack(1002),
        row: 0,
        col: 0,
    };
    apply_inventory_move(&mut inv, &registry, 55, &from, &to, false).expect("跨包移动应成功");

    // 移动后 instance 55 应在 pack_1002，且 lingering_owner_qi 不变（守恒）。
    let pack2 = inv
        .containers
        .iter()
        .find(|c| c.id == container_id_for_worn_pack(1002))
        .expect("pack_1002 存在");
    let moved = pack2
        .items
        .iter()
        .find(|p| p.instance.instance_id == 55)
        .expect("instance 55 应在 pack_1002");
    assert_eq!(
        moved.instance.lingering_owner_qi, qi_before,
        "跨包移动是同一 instance 的位置变更：lingering_owner_qi 必须守恒不变（不重算/复制/蒸发）；\
         期望 {qi_before:?}，实际 {:?}",
        moved.instance.lingering_owner_qi
    );
}

// ===== plan-tarkov-backpack-v1 P2 测试清单（≥6 + e2e；穿戴态门控 + 软门控固化） =====

/// P2 fixture — registry：两个 worn pack 模板（chest_pack/legs_pack）+ 一个 1×1 misc
/// 可移动物品模板（dust）；validate_move_semantics 校验 moving item 的 template 必须命中
/// registry，故 dust 须注册。返回 (registry, inventory)，inventory 为空（worn pack 由各
/// 用例按需装备 + rebuild）。
fn make_p2_registry() -> ItemRegistry {
    let chest_pack = make_container_template("chest_pack", EQUIP_SLOT_CHEST, 3, 3, 10.0);
    let legs_pack = make_container_template("legs_pack", EQUIP_SLOT_LEGS, 3, 3, 8.0);
    // 1×1 容量极小的 pack，用于「目标满 / 越界」边界用例。
    let tiny_pack = make_container_template("tiny_pack", EQUIP_SLOT_CHEST, 1, 1, 5.0);
    let mut dust = make_container_template("dust", EQUIP_SLOT_CHEST, 1, 1, 0.0);
    dust.container_spec = None;
    dust.category = ItemCategory::Misc;
    dust.grid_w = 1;
    dust.grid_h = 1;
    ItemRegistry::from_map(HashMap::from([
        ("chest_pack".to_string(), chest_pack),
        ("legs_pack".to_string(), legs_pack),
        ("tiny_pack".to_string(), tiny_pack),
        ("dust".to_string(), dust),
    ]))
}

/// P2 fixture — 空 inventory + 一个 5×7 main_pack 静态容器（源容器，存放待拖入的物品）。
fn make_p2_inventory() -> PlayerInventory {
    let mut inv = make_empty_inventory();
    inv.containers.push(ContainerState {
        quick_access: false,
        id: MAIN_PACK_CONTAINER_ID.to_string(),
        name: "主背包".to_string(),
        rows: 5,
        cols: 7,
        items: Vec::new(),
        owner_instance_id: None,
    });
    inv
}

/// 交付物 #1 + #2（happy）— 拖入「穿戴中」的 pack_<id> 容器：门控放行，物品落位成功。
/// 同时核实拖入持久化路径：apply_inventory_move 把物品写入 pack_<id>.items（落盘由
/// flush_changed_player_inventories 自动承载，无额外入口；e2e 锁住跨重载）。
#[test]
fn move_item_into_worn_pack_container_succeeds() {
    use crate::schema::inventory::InventoryLocationV1;
    let registry = make_p2_registry();
    let mut inv = make_p2_inventory();
    // chest 穿戴 chest_pack（pack_2001），rebuild 建容器并回填 owner。
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(2001, "chest_pack")),
    );
    let _ = rebuild_containers_from_equipment(&mut inv, &registry);
    // main_pack（默认容器）里放一件 dust，准备拖入 pack_2001。
    let main = inv
        .containers
        .iter_mut()
        .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
        .expect("main_pack 存在");
    main.items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: make_test_item_instance(70, "dust"),
    });

    let from = InventoryLocationV1::Container {
        container_id: MAIN_PACK_CONTAINER_ID.to_string(),
        row: 0,
        col: 0,
    };
    let to = InventoryLocationV1::Container {
        container_id: container_id_for_worn_pack(2001),
        row: 0,
        col: 0,
    };
    apply_inventory_move(&mut inv, &registry, 70, &from, &to, false)
        .expect("拖入穿戴中的 pack_2001 应成功（owner 在 chest worn 层）");

    let pack = inv
        .containers
        .iter()
        .find(|c| c.id == container_id_for_worn_pack(2001))
        .expect("pack_2001 存在");
    assert!(
        pack.items.iter().any(|p| p.instance.instance_id == 70),
        "因为目标 pack_2001 当前穿戴中，门控应放行且 dust(70) 落位进该容器；\
         实际 pack_2001 内含 ids = {:?}",
        pack.items
            .iter()
            .map(|p| p.instance.instance_id)
            .collect::<Vec<_>>()
    );
}

/// 交付物 #2（错误分支）— 拖入「已卸下（非穿戴）」的 pack_<id> 容器：门控拒绝，
/// 返回带修复线索的 Err。背包件已从身体槽卸到 main_pack（格子），其 pack_<id> 容器仍残留。
#[test]
fn move_item_into_unworn_pack_container_rejected() {
    use crate::schema::inventory::InventoryLocationV1;
    let registry = make_p2_registry();
    let mut inv = make_p2_inventory();
    // pack_3001 容器存在（owner_instance_id=3001），但背包件 3001 不在任何 worn 层
    // ——已卸到 main_pack 当普通物品。
    inv.containers.push(ContainerState {
        quick_access: false,
        id: container_id_for_worn_pack(3001),
        name: "已卸下的胸包".to_string(),
        rows: 3,
        cols: 3,
        items: Vec::new(),
        owner_instance_id: Some(3001),
    });
    let main = inv
        .containers
        .iter_mut()
        .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
        .expect("main_pack 存在");
    // 背包件本体卸在 main_pack（非 worn），以及一件待拖入的 dust。
    main.items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: make_container_item(3001, "chest_pack"),
    });
    main.items.push(PlacedItemState {
        row: 0,
        col: 3,
        instance: make_test_item_instance(71, "dust"),
    });

    let from = InventoryLocationV1::Container {
        container_id: MAIN_PACK_CONTAINER_ID.to_string(),
        row: 0,
        col: 3,
    };
    let to = InventoryLocationV1::Container {
        container_id: container_id_for_worn_pack(3001),
        row: 0,
        col: 0,
    };
    // §3 放宽后：携带面 = worn+held+hotbar+body_pocket。pack 3001 卸在 main_pack（grid 货物，
    // 非携带面）→ 仍被门控拒绝；文案改为「背包已不在身上」（统一新语义）。
    let err = apply_inventory_move(&mut inv, &registry, 71, &from, &to, false)
        .expect_err("拖入卸在 grid 内（非携带面）的 pack_3001 应被门控拒绝");
    assert!(
        matches!(
            err,
            InventoryMoveRejectReason::PackDetached {
                owner_instance_id: 3001
            }
        ),
        "期望带修复线索的拒绝（提示背包不在身上 + owner instance id），因为 grid 货物背包是死容器；\
         实际 err = {err:?}"
    );
    // 物品未落位（仍在 main_pack）。
    let pack = inv
        .containers
        .iter()
        .find(|c| c.id == container_id_for_worn_pack(3001))
        .expect("pack_3001 仍存在");
    assert!(
        pack.items.is_empty(),
        "拒绝后 dust(71) 不应进入 pack_3001；实际内含 {} 件",
        pack.items.len()
    );
}

/// 交付物 #2（错误分支）— 拖入「不存在」的 pack_<id> 容器：owner 不在 worn 层 → 拒绝。
/// （pack_<id> 容器本身都不存在；穿戴态门控先于落位层 unknown-container 报错命中。）
#[test]
fn move_item_into_nonexistent_pack_container_rejected() {
    use crate::schema::inventory::InventoryLocationV1;
    let registry = make_p2_registry();
    let mut inv = make_p2_inventory();
    let main = inv
        .containers
        .iter_mut()
        .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
        .expect("main_pack 存在");
    main.items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: make_test_item_instance(72, "dust"),
    });

    let from = InventoryLocationV1::Container {
        container_id: MAIN_PACK_CONTAINER_ID.to_string(),
        row: 0,
        col: 0,
    };
    // pack_9999 既无容器也无 worn owner。
    let to = InventoryLocationV1::Container {
        container_id: container_id_for_worn_pack(9999),
        row: 0,
        col: 0,
    };
    let err = apply_inventory_move(&mut inv, &registry, 72, &from, &to, false)
        .expect_err("拖入不存在/不在携带面的 pack_9999 应被拒绝");
    assert!(
        matches!(
            err,
            InventoryMoveRejectReason::PackDetached {
                owner_instance_id: 9999
            }
        ),
        "期望门控在落位前拒绝（owner 9999 不在任何携带面）；实际 err = {err:?}"
    );
}

/// 交付物 #2（状态转换）— 两个都穿戴中的 pack 之间移动：门控对源容器无要求、
/// 目标 pack owner 在 worn 层 → 放行成功。
#[test]
fn move_item_between_two_worn_packs_succeeds() {
    use crate::schema::inventory::InventoryLocationV1;
    let registry = make_p2_registry();
    let mut inv = make_p2_inventory();
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(4001, "chest_pack")),
    );
    inv.equipped.insert(
        EQUIP_SLOT_LEGS.to_string(),
        SlotContents::worn_single(make_container_item(4002, "legs_pack")),
    );
    let _ = rebuild_containers_from_equipment(&mut inv, &registry);
    // 在 pack_4001 放一件 dust。
    let pack1 = inv
        .containers
        .iter_mut()
        .find(|c| c.id == container_id_for_worn_pack(4001))
        .expect("pack_4001 存在");
    pack1.items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: make_test_item_instance(73, "dust"),
    });

    let from = InventoryLocationV1::Container {
        container_id: container_id_for_worn_pack(4001),
        row: 0,
        col: 0,
    };
    let to = InventoryLocationV1::Container {
        container_id: container_id_for_worn_pack(4002),
        row: 0,
        col: 0,
    };
    apply_inventory_move(&mut inv, &registry, 73, &from, &to, false)
        .expect("两个穿戴中的 pack 之间移动应成功");

    let pack2 = inv
        .containers
        .iter()
        .find(|c| c.id == container_id_for_worn_pack(4002))
        .expect("pack_4002 存在");
    assert!(
        pack2.items.iter().any(|p| p.instance.instance_id == 73),
        "dust(73) 应从 pack_4001 转入 pack_4002；实际 pack_4002 ids = {:?}",
        pack2
            .items
            .iter()
            .map(|p| p.instance.instance_id)
            .collect::<Vec<_>>()
    );
}

/// 交付物 #4 / 决议 #5（软门控）— 超重（current > max）时拖入穿戴中的 pack 仍成功：
/// 超限只打 OverloadedMarker，不在 move 路径硬拒绝。本测试固化「move 路径无重量门控」契约。
#[test]
fn move_into_pack_when_overloaded_still_succeeds() {
    use crate::schema::inventory::InventoryLocationV1;
    let registry = make_p2_registry();
    let mut inv = make_p2_inventory();
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(5001, "chest_pack")),
    );
    let _ = rebuild_containers_from_equipment(&mut inv, &registry);
    // 人为压低 max_weight 使其远小于实际负重，模拟超载态。
    inv.max_weight = 0.01;
    // main_pack 放一件重 dust。
    let mut heavy = make_test_item_instance(74, "dust");
    heavy.weight = 99.0;
    let main = inv
        .containers
        .iter_mut()
        .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
        .expect("main_pack 存在");
    main.items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: heavy,
    });
    // 确认确实超载（current_weight > max_weight）。
    assert!(
        calculate_current_weight(&inv) > inv.max_weight,
        "前置：构造的 inventory 应处于超载态"
    );

    let from = InventoryLocationV1::Container {
        container_id: MAIN_PACK_CONTAINER_ID.to_string(),
        row: 0,
        col: 0,
    };
    let to = InventoryLocationV1::Container {
        container_id: container_id_for_worn_pack(5001),
        row: 0,
        col: 0,
    };
    apply_inventory_move(&mut inv, &registry, 74, &from, &to, false).expect(
        "决议 #5 软门控：超载态下拖入穿戴中的 pack 仍应成功；move 路径不做重量硬拒绝（仅 OverloadedMarker debuff）",
    );
    let pack = inv
        .containers
        .iter()
        .find(|c| c.id == container_id_for_worn_pack(5001))
        .expect("pack_5001 存在");
    assert!(
        pack.items.iter().any(|p| p.instance.instance_id == 74),
        "超载态下重物 dust(74) 仍应落位进 pack_5001（软门控）"
    );
}

/// 交付物（边界：目标容器满）— 目标 pack 落位越界（无空位）→ 落位层拒绝（穿戴态门控放行后，
/// displaced_at_target 的 bounds 检查命中）。固化「门控放行 ≠ 一定落位成功」。
#[test]
fn move_into_full_pack_rejected_no_fit() {
    use crate::schema::inventory::InventoryLocationV1;
    let registry = make_p2_registry();
    let mut inv = make_p2_inventory();
    // 穿戴 1×1 的 tiny_pack（pack_6001），rebuild 建容器。
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(make_container_item(6001, "tiny_pack")),
    );
    let _ = rebuild_containers_from_equipment(&mut inv, &registry);
    // tiny_pack 唯一格 (0,0) 已被占满。
    let pack = inv
        .containers
        .iter_mut()
        .find(|c| c.id == container_id_for_worn_pack(6001))
        .expect("pack_6001 存在");
    pack.items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: make_test_item_instance(80, "dust"),
    });
    // main_pack 放一件待拖入的 dust。
    let main = inv
        .containers
        .iter_mut()
        .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
        .expect("main_pack 存在");
    main.items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: make_test_item_instance(81, "dust"),
    });

    let from = InventoryLocationV1::Container {
        container_id: MAIN_PACK_CONTAINER_ID.to_string(),
        row: 0,
        col: 0,
    };
    // 目标 (0,1)：1×1 容器越界（col 1 + 1 > cols 1）→ 落位层 no-fit 拒绝。
    let to = InventoryLocationV1::Container {
        container_id: container_id_for_worn_pack(6001),
        row: 0,
        col: 1,
    };
    let err = apply_inventory_move(&mut inv, &registry, 81, &from, &to, false).expect_err(
        "穿戴态门控放行后，落位层应因 1×1 容器越界（无空位）拒绝；门控放行 ≠ 一定能放下",
    );
    assert!(
        matches!(
            err,
            InventoryMoveRejectReason::TargetOutOfBounds
                | InventoryMoveRejectReason::TargetOccupied { .. }
        ),
        "期望落位层 no-fit 拒绝（越界/重叠），因为 tiny_pack 仅 1×1 且已满；实际 err = {err:?}"
    );
    // dust(81) 未进 pack_6001。
    let pack = inv
        .containers
        .iter()
        .find(|c| c.id == container_id_for_worn_pack(6001))
        .expect("pack_6001 仍存在");
    assert!(
        !pack.items.iter().any(|p| p.instance.instance_id == 81),
        "no-fit 拒绝后 dust(81) 不应进入 pack_6001"
    );
}

// (决议 #17/#9/#8) back_pack/waist_pouch/chest_satchel EquipSlotV1 variant 已删除，
// 原 equip_slot_v1_backpack_variants_serde_roundtrip 测试随之移除。

// ItemCategory serde pins

#[test]
fn item_category_block_serde_pin() {
    let serialized = serde_json::to_string(&ItemCategory::Block).expect("serialize Block category");
    assert_eq!(
        serialized, "\"Block\"",
        "expected ItemCategory::Block to serialize as the explicit protocol literal"
    );

    let deserialized: ItemCategory =
        serde_json::from_str("\"Block\"").expect("deserialize Block category literal");
    assert_eq!(deserialized, ItemCategory::Block);
}

#[test]
fn item_category_invalid_variant_is_rejected() {
    let result = serde_json::from_str::<ItemCategory>("\"InvalidVariant\"");
    assert!(
        result.is_err(),
        "expected invalid ItemCategory protocol literal to be rejected, got {result:?}"
    );
}

#[test]
fn item_category_container_serde_roundtrip() {
    let cat = ItemCategory::Container;
    let json = serde_json::to_string(&cat).expect("serialize Container category");
    let back: ItemCategory = serde_json::from_str(&json).expect("deserialize Container category");
    assert_eq!(back, cat);
}

// =========== plan-backpack-equip-v1 P3 — 背包耐久扣减与破损溢出测试 ===========

/// 构造一个携带草编囊的 registry + inventory（耐久 cost_per_op = 0.008，
/// durability 初始值由调用方通过 `durability` 参数控制）。
fn make_worn_grass_pouch_setup(
    durability: f64,
    with_container_items: bool,
) -> (ItemRegistry, PlayerInventory) {
    let template = ItemTemplate {
        id: "worn_grass_pouch".to_string(),
        display_name: "草编囊（磨损）".to_string(),
        category: ItemCategory::Container,
        placeable: None,
        max_stack_count: 1,
        grid_w: 1,
        grid_h: 2,
        base_weight: 0.3,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 0.5,
        description: "test".to_string(),
        effect: None,
        cast_duration_ms: DEFAULT_CAST_DURATION_MS,
        cooldown_ms: DEFAULT_COOLDOWN_MS,
        weapon_spec: None,
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
        readable_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: Some(ContainerSpec {
            quick_access: false,
            rows: 3,
            cols: 3,
            weight_capacity: 10.0,
            // 决议 #17：背包无专属槽，equip_slot 指向身体槽（chest），骑在 chest worn 层。
            equip_slot: EQUIP_SLOT_CHEST.to_string(),
            durability_cost_per_op: 0.008,
            attrition_exempt: false,
            accept_filter: None,
        }),
        shield_spec: None,

        shelflife_profile: None,
        shelflife_track: None,
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    };
    let registry =
        ItemRegistry::from_map(HashMap::from([("worn_grass_pouch".to_string(), template)]));

    // 构造一个装备了草编囊的 inventory。
    let backpack_instance = ItemInstance {
        instance_id: 1,
        template_id: "worn_grass_pouch".to_string(),
        display_name: "草编囊（磨损）".to_string(),
        grid_w: 1,
        grid_h: 2,
        weight: 0.3,
        rarity: ItemRarity::Common,
        description: "test".to_string(),
        stack_count: 1,
        spirit_quality: 0.5,
        durability,
        freshness: None,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    };

    let container_items = if with_container_items {
        vec![
            PlacedItemState {
                row: 0,
                col: 0,
                instance: make_test_item_instance(10, "spirit_herb"),
            },
            PlacedItemState {
                row: 1,
                col: 0,
                instance: make_test_item_instance(11, "bone_dust"),
            },
        ]
    } else {
        Vec::new()
    };

    let mut inv = make_empty_inventory();
    // 决议 #17：背包件骑在 chest worn 层；容器 id = pack_<instance_id> = "pack_1"。
    let pack_container_id = container_id_for_worn_pack(backpack_instance.instance_id);
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(backpack_instance),
    );
    inv.containers.push(ContainerState {
        quick_access: false,
        id: pack_container_id,
        name: "草编囊".to_string(),
        rows: 3,
        cols: 3,
        items: container_items,
        owner_instance_id: None,
    });
    inv.max_weight = BASE_CARRY_CAPACITY + 10.0;

    (registry, inv)
}

// P3.1.1 — apply_backpack_wear 正常扣减

#[test]
fn apply_backpack_wear_deducts_cost_per_op() {
    let (registry, mut inv) = make_worn_grass_pouch_setup(1.0, false);
    let event = apply_backpack_wear(&mut inv, &registry, &container_id_for_worn_pack(1));
    assert!(
        event.is_none(),
        "durability 1.0 minus 0.008 should not break yet"
    );
    let durability = inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn[0].durability;
    assert!(
        (durability - 0.992).abs() < 1e-9,
        "expected durability ≈ 0.992 after one wear, got {durability}"
    );
}

#[test]
fn apply_backpack_wear_multiple_ops_reduce_durability_cumulatively() {
    let (registry, mut inv) = make_worn_grass_pouch_setup(0.1, false);
    // 12 ops × 0.008 = 0.096 > 0.1 − 0.008×12 = 0.004; not yet broken after 12.
    for _ in 0..12 {
        let event = apply_backpack_wear(&mut inv, &registry, &container_id_for_worn_pack(1));
        assert!(
            event.is_none(),
            "should not break before 0.1/0.008 ≈ 12.5 ops"
        );
    }
    let durability = inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn[0].durability;
    let expected = 0.1 - 12.0 * 0.008;
    assert!(
        (durability - expected).abs() < 1e-9,
        "expected durability ≈ {expected} after 12 ops, got {durability}"
    );
}

// P3.1.2 — body_pocket 操作不扣减

#[test]
fn apply_backpack_wear_body_pocket_does_not_deduct() {
    let (registry, mut inv) = make_worn_grass_pouch_setup(1.0, false);
    let event = apply_backpack_wear(&mut inv, &registry, BODY_POCKET_CONTAINER_ID);
    assert!(
        event.is_none(),
        "body_pocket should never trigger wear deduction"
    );
    // 装备耐久不变。
    let durability = inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn[0].durability;
    assert!(
        (durability - 1.0).abs() < f64::EPSILON,
        "worn_grass_pouch durability should be unchanged, got {durability}"
    );
}

// P3.1.3 — 未知 container_id 不扣减

#[test]
fn apply_backpack_wear_unknown_container_id_no_deduct() {
    let (registry, mut inv) = make_worn_grass_pouch_setup(1.0, false);
    let event = apply_backpack_wear(&mut inv, &registry, "totally_unknown_container");
    assert!(event.is_none(), "unknown container id should be a no-op");
}

// P3.1.4 — 多次扣减到 ≤ε 时返回 BackpackBreakEvent

#[test]
fn apply_backpack_wear_returns_break_event_when_durability_depleted() {
    // worn_grass_pouch: durability_cost_per_op = 0.008，从 0.3 开始（P2 默认值）。
    // 0.3 / 0.008 = 37.5，所以第 38 次调用会触发破损。
    let (registry, mut inv) = make_worn_grass_pouch_setup(0.3, false);

    for i in 1..38 {
        let event = apply_backpack_wear(&mut inv, &registry, &container_id_for_worn_pack(1));
        assert!(
            event.is_none(),
            "op {i}/38 should not break yet (durability = {})",
            inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn[0].durability
        );
    }
    // 第 38 次——应触发破损。
    let event = apply_backpack_wear(&mut inv, &registry, &container_id_for_worn_pack(1));
    assert!(
        event.is_some(),
        "38th op should trigger BackpackBreakEvent (durability = {})",
        inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn[0].durability
    );
    let ev = event.unwrap();
    assert_eq!(
        ev.backpack_instance_id, 1,
        "break event backpack_instance_id mismatch（应为 worn pack 的 instance_id）"
    );
    assert_eq!(
        ev.container_id,
        container_id_for_worn_pack(1),
        "break event container_id mismatch（应为 pack_<instance_id>）"
    );
}

// P3.1.5 — cost_per_op = 0.0 时永远不扣减（无损耗背包）

#[test]
fn apply_backpack_wear_zero_cost_per_op_never_deducts() {
    let template = make_container_template("lossless_bag", EQUIP_SLOT_CHEST, 5, 5, 20.0);
    // make_container_template 默认 cost_per_op = 0.0。
    let registry = ItemRegistry::from_map(HashMap::from([("lossless_bag".to_string(), template)]));
    let mut inv = make_empty_inventory();
    let bag = ItemInstance {
        instance_id: 200,
        template_id: "lossless_bag".to_string(),
        display_name: "lossless".to_string(),
        grid_w: 2,
        grid_h: 3,
        weight: 0.5,
        rarity: ItemRarity::Common,
        description: String::new(),
        stack_count: 1,
        spirit_quality: 1.0,
        durability: 0.001, // 极低耐久但 cost=0 不应触发破损
        freshness: None,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    };
    inv.equipped
        .insert(EQUIP_SLOT_CHEST.to_string(), SlotContents::worn_single(bag));

    let event = apply_backpack_wear(&mut inv, &registry, &container_id_for_worn_pack(200));
    assert!(
        event.is_none(),
        "zero cost_per_op should never trigger wear even at low durability"
    );
    let durability = inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn[0].durability;
    assert!(
        (durability - 0.001).abs() < f64::EPSILON,
        "durability should be unchanged with cost_per_op=0.0"
    );
}

// P3.1.6 — slot 未装备时返回 None

#[test]
fn apply_backpack_wear_missing_equip_returns_none() {
    let (registry, mut inv) = make_worn_grass_pouch_setup(1.0, false);
    // 试图对一个未穿戴的 pack 容器（pack_999）扣减——无对应 worn 背包件 → None。
    let event = apply_backpack_wear(&mut inv, &registry, &container_id_for_worn_pack(999));
    assert!(
        event.is_none(),
        "empty equip slot should return None, not panic"
    );
}

// P3.2.1 — handle_backpack_break 移除容器 + 返回 spilled_items + max_weight 下降

#[test]
fn handle_backpack_break_spills_items_and_removes_container() {
    let (registry, mut inv) = make_worn_grass_pouch_setup(0.0, true);

    let initial_max_weight = inv.max_weight;
    let outcome = handle_backpack_break(&mut inv, &registry, &container_id_for_worn_pack(1))
        .expect("handle_backpack_break should return Some for valid slot");

    // 背包件已从 chest worn 层移除（空 SlotContents 可能保留，断言 worn 为空）。
    assert!(
        inv.equipped
            .get(EQUIP_SLOT_CHEST)
            .map(|s| s.worn.is_empty())
            .unwrap_or(true),
        "backpack should be removed from chest worn after break"
    );

    // 容器（pack_1）已从 containers 移除。
    assert!(
        inv.containers
            .iter()
            .all(|c| c.id != container_id_for_worn_pack(1)),
        "container should be removed from containers after break"
    );

    // 溢出物品包含原容器内的所有物品。
    assert_eq!(
        outcome.spilled_items.len(),
        2,
        "expected 2 spilled items (spirit_herb + bone_dust)"
    );
    let spilled_ids: Vec<u64> = outcome
        .spilled_items
        .iter()
        .map(|i| i.instance_id)
        .collect();
    assert!(
        spilled_ids.contains(&10),
        "spirit_herb (id=10) should be spilled"
    );
    assert!(
        spilled_ids.contains(&11),
        "bone_dust (id=11) should be spilled"
    );

    // 破损的背包物品实例正确返回。
    assert_eq!(
        outcome.backpack_item.template_id, "worn_grass_pouch",
        "backpack_item template_id mismatch"
    );

    // max_weight 下降（去掉 10.0 的 weight_capacity）。
    let expected_new_max = BASE_CARRY_CAPACITY; // 15.0
    assert!(
        (outcome.new_max_weight - expected_new_max).abs() < f64::EPSILON,
        "expected new_max_weight={expected_new_max}, got {}",
        outcome.new_max_weight
    );
    assert!(
        outcome.new_max_weight < initial_max_weight,
        "max_weight should drop after backpack break"
    );
    // inventory 本身的 max_weight 也已更新。
    assert!(
        (inv.max_weight - expected_new_max).abs() < f64::EPSILON,
        "inventory.max_weight should be refreshed to {expected_new_max}, got {}",
        inv.max_weight
    );
}

// P3.2.2 — handle_backpack_break 对空容器（无溢出物品）

#[test]
fn handle_backpack_break_empty_container_spills_nothing() {
    let (registry, mut inv) = make_worn_grass_pouch_setup(0.0, false);

    let outcome = handle_backpack_break(&mut inv, &registry, &container_id_for_worn_pack(1))
        .expect("break on empty container should still succeed");

    assert!(
        outcome.spilled_items.is_empty(),
        "no items should be spilled from an empty container"
    );
    assert_eq!(
        outcome.backpack_item.template_id, "worn_grass_pouch",
        "backpack_item should still be returned even with empty container"
    );
}

// P3.2.3 — handle_backpack_break 对 body_pocket 返回 None

#[test]
fn handle_backpack_break_body_pocket_returns_none() {
    let (registry, mut inv) = make_worn_grass_pouch_setup(0.0, false);
    let outcome = handle_backpack_break(&mut inv, &registry, BODY_POCKET_CONTAINER_ID);
    assert!(
        outcome.is_none(),
        "body_pocket should not trigger backpack break"
    );
}

// P3.2.4 — handle_backpack_break 对未装备槽返回 None

#[test]
fn handle_backpack_break_unequipped_slot_returns_none() {
    let (registry, mut inv) = make_worn_grass_pouch_setup(0.0, false);
    // 对一个未穿戴的 pack 容器（pack_999）破损——无对应 worn 背包件 → None。
    let outcome = handle_backpack_break(&mut inv, &registry, &container_id_for_worn_pack(999));
    assert!(
        outcome.is_none(),
        "unequipped slot should return None from handle_backpack_break"
    );
}

// P3.2.5 — handle_backpack_break 当容器不在 containers 列表时仍正常工作（spilled 为空）

#[test]
fn handle_backpack_break_missing_container_entry_spills_nothing() {
    let (registry, mut inv) = make_worn_grass_pouch_setup(0.0, false);
    // 手动移除容器，模拟 containers 与 equipped 不同步场景。
    inv.containers
        .retain(|c| c.id != container_id_for_worn_pack(1));

    let outcome = handle_backpack_break(&mut inv, &registry, &container_id_for_worn_pack(1))
        .expect("should succeed even without matching container");

    assert!(
        outcome.spilled_items.is_empty(),
        "no items to spill when container entry is missing"
    );
}

// P3 真实物品模板 — worn_grass_pouch（P2 草编囊）操作 38 次后破损

#[test]
fn worn_grass_pouch_breaks_after_38_ops_from_30_percent_durability() {
    // P2 default: durability=0.3, cost_per_op=0.008
    // 0.3 / 0.008 = 37.5 → floor = 37，第 38 次触发破损
    let (registry, mut inv) = make_worn_grass_pouch_setup(0.3, false);

    for i in 1..=37 {
        let ev = apply_backpack_wear(&mut inv, &registry, &container_id_for_worn_pack(1));
        assert!(
            ev.is_none(),
            "op {i}: should not break before op 38 (durability={})",
            inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn[0].durability
        );
    }
    let ev = apply_backpack_wear(&mut inv, &registry, &container_id_for_worn_pack(1));
    assert!(
        ev.is_some(),
        "38th op should return BackpackBreakEvent, durability={}",
        inv.equipped.get(EQUIP_SLOT_CHEST).unwrap().worn[0].durability
    );
    let ev = ev.unwrap();
    assert_eq!(ev.backpack_instance_id, 1);
    assert_eq!(ev.container_id, container_id_for_worn_pack(1));
}

// P3 BackpackBreakEvent PartialEq pin 测试

#[test]
fn backpack_break_event_partial_eq_and_clone() {
    let ev1 = BackpackBreakEvent {
        backpack_instance_id: 1,
        container_id: container_id_for_worn_pack(1),
    };
    let ev2 = ev1.clone();
    assert_eq!(ev1, ev2, "BackpackBreakEvent should implement PartialEq");
    let ev3 = BackpackBreakEvent {
        backpack_instance_id: 2,
        container_id: container_id_for_worn_pack(2),
    };
    assert_ne!(ev1, ev3, "different backpack instances should not be equal");
}

// plan-dandao-path-v1 — ExtraHand0/ExtraHand1 equip slot tests

fn make_weapon_template(id: &str) -> ItemTemplate {
    ItemTemplate {
        id: id.to_string(),
        display_name: id.to_string(),
        category: ItemCategory::Weapon,
        placeable: None,
        max_stack_count: 1,
        grid_w: 1,
        grid_h: 2,
        base_weight: 2.0,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 0.5,
        description: "test weapon".to_string(),
        effect: None,
        cast_duration_ms: 0,
        cooldown_ms: 0,
        weapon_spec: Some(WeaponSpec {
            weapon_kind: crate::combat::weapon::WeaponKind::Sword,
            base_attack: 5.0,
            quality_tier: 0,
            durability_max: 100.0,
            qi_cost_mul: 1.0,
        }),
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
        readable_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: None,
        shield_spec: None,

        shelflife_profile: None,
        shelflife_track: None,
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    }
}

fn make_misc_template(id: &str) -> ItemTemplate {
    ItemTemplate {
        id: id.to_string(),
        display_name: id.to_string(),
        category: ItemCategory::Misc,
        placeable: None,
        max_stack_count: 64,
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.1,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 0.0,
        description: "misc".to_string(),
        effect: None,
        cast_duration_ms: 0,
        cooldown_ms: 0,
        weapon_spec: None,
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
        readable_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: None,
        shield_spec: None,

        shelflife_profile: None,
        shelflife_track: None,
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    }
}

// 决议 #17：false_skin 专属槽已删，伪皮改穿 chest worn（身体槽接受 armor/false skin/container）。
#[test]
fn validate_move_semantics_accepts_low_cost_disguise_items_to_chest_worn() {
    use crate::combat::tuike::{CAMOUFLAGE_NET_ITEM_ID, DISGUISE_WRAP_ITEM_ID};
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};

    let registry = ItemRegistry::from_map(HashMap::from([
        (
            DISGUISE_WRAP_ITEM_ID.to_string(),
            make_misc_template(DISGUISE_WRAP_ITEM_ID),
        ),
        (
            CAMOUFLAGE_NET_ITEM_ID.to_string(),
            make_misc_template(CAMOUFLAGE_NET_ITEM_ID),
        ),
    ]));
    let inventory = make_empty_inventory();
    let from = InventoryLocationV1::Container {
        container_id: MAIN_PACK_CONTAINER_ID.to_string(),
        row: 0,
        col: 0,
    };
    let to = InventoryLocationV1::Equip {
        slot: EquipSlotV1::Chest,
        state: EquipStateV1::Worn,
    };

    for (instance_id, template_id) in [(10, DISGUISE_WRAP_ITEM_ID), (11, CAMOUFLAGE_NET_ITEM_ID)] {
        let item = make_test_item_instance(instance_id, template_id);
        assert!(
            validate_move_semantics(&registry, &inventory, &item, &from, &to).is_ok(),
            "{template_id} (false skin) should be equippable to chest worn"
        );
    }
}

// Bug2（真机回归）— fake_spirit_hide 真实数据为 category=misc（materials.toml），
// 但正典为蛛丝型伪皮。live-equip 校验（validate_move_semantics）必须放行其入胸槽 worn，
// 否则「出生自带却拖不回胸槽」自相矛盾。用真实 registry 证明放行靠 false_skin 闸而非 category。
#[test]
fn validate_move_semantics_accepts_fake_spirit_hide_to_chest_worn_with_real_registry() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};

    let registry = load_item_registry().expect("real item registry loads");
    // 前置断言：fake_spirit_hide 真实 category 不是 Armor / Container，放行只能靠 false_skin 闸。
    let template = registry
        .get("fake_spirit_hide")
        .expect("fake_spirit_hide template registered");
    assert!(
        !matches!(template.category, ItemCategory::Armor),
        "fake_spirit_hide 真实 category 应非 Armor（证明放行靠 false_skin 闸）"
    );
    assert!(
        template.container_spec.is_none(),
        "fake_spirit_hide 非容器件（证明放行靠 false_skin 闸）"
    );

    let inventory = make_empty_inventory();
    let item = make_test_item_instance(70, "fake_spirit_hide");
    let from = InventoryLocationV1::Container {
        container_id: MAIN_PACK_CONTAINER_ID.to_string(),
        row: 0,
        col: 0,
    };
    let to = InventoryLocationV1::Equip {
        slot: EquipSlotV1::Chest,
        state: EquipStateV1::Worn,
    };
    assert!(
        validate_move_semantics(&registry, &inventory, &item, &from, &to).is_ok(),
        "fake_spirit_hide（伪灵皮）必须能拖进胸槽 worn（live-equip 与 instantiate 一致）"
    );
}

// Bug2（真机回归）— instantiate（绕校验）与 live-equip（走校验）对 fake_spirit_hide 一致：
// 出生自带后必须能卸下再拖回。default.toml 把 fake_spirit_hide 放 chest worn，
// 实例化后它确实在 chest.worn，且其 validate_move_semantics 放行（上一条已证）。
#[test]
fn fake_spirit_hide_instantiate_matches_live_equip_for_chest_worn() {
    let registry = load_item_registry().expect("real item registry loads");
    let loadout = load_default_loadout(&registry).expect("default loadout loads");
    let mut alloc = InventoryInstanceIdAllocator::default();
    let inv = instantiate_inventory_from_loadout(&loadout, &mut alloc, &registry)
        .expect("instantiate default loadout");

    let chest = inv
        .equipped
        .get(EQUIP_SLOT_CHEST)
        .expect("chest slot present after instantiate");
    let chest_worn: Vec<&str> = chest.worn.iter().map(|i| i.template_id.as_str()).collect();
    assert_eq!(
        chest_worn,
        vec!["worn_grass_pouch", "fake_spirit_hide"],
        "fresh 实例化的 chest.worn 应为 [背包件, 伪皮]；实际 {chest_worn:?}"
    );

    // instantiate 放进去的 fake_spirit_hide，live-equip 校验也必须能把它放回胸槽 worn。
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
    let hide = chest
        .worn
        .iter()
        .find(|i| i.template_id == "fake_spirit_hide")
        .expect("fake_spirit_hide in chest worn");
    let from = InventoryLocationV1::Container {
        container_id: MAIN_PACK_CONTAINER_ID.to_string(),
        row: 0,
        col: 0,
    };
    let to = InventoryLocationV1::Equip {
        slot: EquipSlotV1::Chest,
        state: EquipStateV1::Worn,
    };
    let mut empty = make_empty_inventory();
    empty.max_weight = inv.max_weight;
    assert!(
        validate_move_semantics(&registry, &empty, hide, &from, &to).is_ok(),
        "instantiate 放进胸槽的 fake_spirit_hide，live-equip 必须也放行（instantiate==live）"
    );
}

// Bug3（真机回归）— fresh 实例化后，运行时容器 id 必须与 default.toml worn_grass_pouch
// 自洽：静态占位 `pack_grass_pouch` 已重映射到 pack_<背包件 instance_id>，
// 不再残留占位 id / 旧 back_pack id。
#[test]
fn fresh_instantiate_container_id_self_consistent_with_worn_pack() {
    let registry = load_item_registry().expect("real item registry loads");
    let loadout = load_default_loadout(&registry).expect("default loadout loads");
    let mut alloc = InventoryInstanceIdAllocator::default();
    let inv = instantiate_inventory_from_loadout(&loadout, &mut alloc, &registry)
        .expect("instantiate default loadout");

    // 找到 chest.worn 里的背包件（worn_grass_pouch）instance_id。
    let chest = inv.equipped.get(EQUIP_SLOT_CHEST).expect("chest present");
    let pack = chest
        .worn
        .iter()
        .find(|i| {
            registry
                .get(&i.template_id)
                .is_some_and(|t| t.container_spec.is_some())
        })
        .expect("worn pack item present");
    let expected_container_id = container_id_for_worn_pack(pack.instance_id);

    assert!(
        inv.containers.iter().any(|c| c.id == expected_container_id),
        "运行时应存在与穿戴背包件自洽的容器 `{expected_container_id}`；实际 ids = {:?}",
        inv.containers.iter().map(|c| &c.id).collect::<Vec<_>>()
    );
    assert!(
        !inv.containers
            .iter()
            .any(|c| c.id == LOADOUT_PACK_PLACEHOLDER_CONTAINER_ID),
        "静态占位容器 id `{LOADOUT_PACK_PLACEHOLDER_CONTAINER_ID}` 不应在运行时存活（必须已重映射）"
    );
    assert!(
        !inv.containers.iter().any(|c| c.id == "back_pack"),
        "运行时不应出现旧 back_pack 容器 id（命名空间已统一到 pack_<id>）"
    );
    // 背包件容器内物品应来自 default.toml 破草包（非空）。
    let pack_container = inv
        .containers
        .iter()
        .find(|c| c.id == expected_container_id)
        .expect("pack container present");
    assert!(
        !pack_container.items.is_empty(),
        "破草包容器应含 default.toml 起手物品（非空）"
    );
}

#[test]
fn validate_move_semantics_rejects_non_false_skin_misc_item_to_chest_worn() {
    use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};

    let registry = ItemRegistry::from_map(HashMap::from([(
        "rough_cloth".to_string(),
        make_misc_template("rough_cloth"),
    )]));
    let inventory = make_empty_inventory();
    let item = make_test_item_instance(12, "rough_cloth");
    let from = InventoryLocationV1::Container {
        container_id: MAIN_PACK_CONTAINER_ID.to_string(),
        row: 0,
        col: 0,
    };
    let to = InventoryLocationV1::Equip {
        slot: EquipSlotV1::Chest,
        state: EquipStateV1::Worn,
    };

    let error = validate_move_semantics(&registry, &inventory, &item, &from, &to)
        .expect_err("non false-skin / non-armor / non-container misc item should be rejected");

    assert!(
        matches!(error, InventoryMoveRejectReason::EquipCategoryMismatch),
        "expected body-slot type rejection, got: {error:?}"
    );
}

#[test]
fn equip_slot_key_extra_hand_0_returns_correct_string() {
    use crate::schema::inventory::EquipSlotV1;
    assert_eq!(
        equip_slot_key(&EquipSlotV1::ExtraHand0),
        "extra_hand_0",
        "ExtraHand0 should map to runtime key 'extra_hand_0'"
    );
}

#[test]
fn equip_slot_key_extra_hand_1_returns_correct_string() {
    use crate::schema::inventory::EquipSlotV1;
    assert_eq!(
        equip_slot_key(&EquipSlotV1::ExtraHand1),
        "extra_hand_1",
        "ExtraHand1 should map to runtime key 'extra_hand_1'"
    );
}

#[test]
fn validate_equip_slot_accepts_extra_hand_0() {
    let path = std::path::Path::new("test.toml");
    assert!(
        validate_equip_slot(EQUIP_SLOT_EXTRA_HAND_0, path).is_ok(),
        "validate_equip_slot should accept 'extra_hand_0'"
    );
}

#[test]
fn validate_equip_slot_accepts_extra_hand_1() {
    let path = std::path::Path::new("test.toml");
    assert!(
        validate_equip_slot(EQUIP_SLOT_EXTRA_HAND_1, path).is_ok(),
        "validate_equip_slot should accept 'extra_hand_1'"
    );
}

#[test]
fn validate_move_semantics_accepts_weapon_to_extra_hand_0() {
    use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};
    let registry = ItemRegistry::from_map(HashMap::from([(
        "test_sword".to_string(),
        make_weapon_template("test_sword"),
    )]));
    let inv = make_empty_inventory();
    let item = make_test_item_instance(900, "test_sword");
    let from = InventoryLocationV1::Container {
        container_id: "main_pack".to_string(),
        row: 0,
        col: 0,
    };
    let to = InventoryLocationV1::Equip {
        slot: EquipSlotV1::ExtraHand0,
        state: crate::schema::inventory::EquipStateV1::Held,
    };
    assert!(
        validate_move_semantics(&registry, &inv, &item, &from, &to).is_ok(),
        "weapon should be equippable to ExtraHand0"
    );
}

#[test]
fn validate_move_semantics_accepts_weapon_to_extra_hand_1() {
    use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};
    let registry = ItemRegistry::from_map(HashMap::from([(
        "test_sword".to_string(),
        make_weapon_template("test_sword"),
    )]));
    let inv = make_empty_inventory();
    let item = make_test_item_instance(901, "test_sword");
    let from = InventoryLocationV1::Container {
        container_id: "main_pack".to_string(),
        row: 0,
        col: 0,
    };
    let to = InventoryLocationV1::Equip {
        slot: EquipSlotV1::ExtraHand1,
        state: crate::schema::inventory::EquipStateV1::Held,
    };
    assert!(
        validate_move_semantics(&registry, &inv, &item, &from, &to).is_ok(),
        "weapon should be equippable to ExtraHand1"
    );
}

#[test]
fn validate_move_semantics_rejects_misc_item_to_extra_hand() {
    use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};
    let registry = ItemRegistry::from_map(HashMap::from([(
        "random_herb".to_string(),
        make_misc_template("random_herb"),
    )]));
    let inv = make_empty_inventory();
    let item = make_test_item_instance(902, "random_herb");
    let from = InventoryLocationV1::Container {
        container_id: "main_pack".to_string(),
        row: 0,
        col: 0,
    };
    let to = InventoryLocationV1::Equip {
        slot: EquipSlotV1::ExtraHand0,
        state: crate::schema::inventory::EquipStateV1::Held,
    };
    let err = validate_move_semantics(&registry, &inv, &item, &from, &to)
        .expect_err("misc item should not equip to ExtraHand0");
    assert!(
        matches!(err, InventoryMoveRejectReason::EquipCategoryMismatch),
        "expected weapon/tool/hoe error, got: {err:?}"
    );
}

#[test]
fn validate_move_semantics_accepts_tool_to_extra_hand() {
    use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};
    let mut tool_template = make_misc_template("test_tool");
    tool_template.category = ItemCategory::Tool;
    let registry =
        ItemRegistry::from_map(HashMap::from([("test_tool".to_string(), tool_template)]));
    let inv = make_empty_inventory();
    let item = make_test_item_instance(903, "test_tool");
    let from = InventoryLocationV1::Container {
        container_id: "main_pack".to_string(),
        row: 0,
        col: 0,
    };
    let to = InventoryLocationV1::Equip {
        slot: EquipSlotV1::ExtraHand1,
        state: crate::schema::inventory::EquipStateV1::Held,
    };
    assert!(
        validate_move_semantics(&registry, &inv, &item, &from, &to).is_ok(),
        "tool should be equippable to ExtraHand1"
    );
}

// (决议 #17) container_id_to_equip_slot 函数已删除（背包无专属槽，容器 id = pack_<id>），
// 原 container_id_to_equip_slot_maps_all_three_slots pin 测试随之移除。
// 反查改由 worn_pack_instance_from_container_id 承担，见 layered_equip_p0_pins。

// ── plan-onboarding-loop-v1 P1.1: 入门残卷 + fragment 物品解析测试 ──

#[test]
fn onboarding_scroll_sword_cleave_parses() {
    let registry = load_item_registry().expect("item registry should load");
    let template = registry
        .get("scroll_technique_sword_cleave")
        .expect("scroll_technique_sword_cleave should exist in registry");
    assert_eq!(template.category, ItemCategory::Scroll);
    assert_eq!(template.rarity, ItemRarity::Common);
    let spec = template
        .technique_scroll_spec
        .as_ref()
        .expect("should have technique_scroll_spec");
    assert_eq!(spec.skill_id, "sword.cleave");
}

#[test]
fn onboarding_scroll_sword_thrust_parses() {
    let registry = load_item_registry().expect("item registry should load");
    let template = registry
        .get("scroll_technique_sword_thrust")
        .expect("scroll_technique_sword_thrust should exist in registry");
    assert_eq!(template.category, ItemCategory::Scroll);
    let spec = template.technique_scroll_spec.as_ref().unwrap();
    assert_eq!(spec.skill_id, "sword.thrust");
}

#[test]
fn onboarding_scroll_sword_parry_parses() {
    let registry = load_item_registry().expect("item registry should load");
    let template = registry
        .get("scroll_technique_sword_parry")
        .expect("scroll_technique_sword_parry should exist in registry");
    assert_eq!(template.category, ItemCategory::Scroll);
    assert_eq!(template.rarity, ItemRarity::Uncommon);
    let spec = template.technique_scroll_spec.as_ref().unwrap();
    assert_eq!(spec.skill_id, "sword.parry");
}

#[test]
fn onboarding_scroll_sword_infuse_parses() {
    let registry = load_item_registry().expect("item registry should load");
    let template = registry
        .get("scroll_technique_sword_infuse")
        .expect("scroll_technique_sword_infuse should exist in registry");
    assert_eq!(template.category, ItemCategory::Scroll);
    assert_eq!(template.rarity, ItemRarity::Uncommon);
    let spec = template.technique_scroll_spec.as_ref().unwrap();
    assert_eq!(spec.skill_id, "sword.infuse");
}

#[test]
fn onboarding_scroll_movement_dash_parses() {
    let registry = load_item_registry().expect("item registry should load");
    let template = registry
        .get("scroll_technique_movement_dash")
        .expect("scroll_technique_movement_dash should exist in registry");
    assert_eq!(template.category, ItemCategory::Scroll);
    assert_eq!(template.rarity, ItemRarity::Common);
    let spec = template.technique_scroll_spec.as_ref().unwrap();
    assert_eq!(spec.skill_id, "movement.dash");
}

#[test]
fn existing_scroll_body_guangbo_ticao_in_registry() {
    let registry = load_item_registry().expect("item registry should load");
    let template = registry
        .get("scroll_body_guangbo_ticao")
        .expect("scroll_body_guangbo_ticao should exist in registry (body_scrolls.toml)");
    assert_eq!(template.category, ItemCategory::Scroll);
    let spec = template.technique_scroll_spec.as_ref().unwrap();
    assert_eq!(spec.skill_id, "body.guangbo_ticao");
}

#[test]
fn onboarding_scroll_burst_beng_quan_parses() {
    let registry = load_item_registry().expect("item registry should load");
    let template = registry
        .get("scroll_technique_burst_beng_quan")
        .expect("scroll_technique_burst_beng_quan should exist in registry");
    assert_eq!(template.category, ItemCategory::Scroll);
    assert_eq!(template.rarity, ItemRarity::Rare);
    let spec = template.technique_scroll_spec.as_ref().unwrap();
    assert_eq!(spec.skill_id, "burst_meridian.beng_quan");
}

#[test]
fn onboarding_scroll_zhenmai_parry_parses() {
    let registry = load_item_registry().expect("item registry should load");
    let template = registry
        .get("scroll_technique_zhenmai_parry")
        .expect("scroll_technique_zhenmai_parry should exist in registry");
    assert_eq!(template.category, ItemCategory::Scroll);
    assert_eq!(template.rarity, ItemRarity::Rare);
    let spec = template.technique_scroll_spec.as_ref().unwrap();
    assert_eq!(spec.skill_id, "zhenmai.parry");
}

#[test]
fn fragment_alchemy_hui_yuan_pill_parses() {
    let registry = load_item_registry().expect("item registry should load");
    let template = registry
        .get("fragment_alchemy_hui_yuan_pill")
        .expect("fragment_alchemy_hui_yuan_pill should exist in registry");
    assert_eq!(template.category, ItemCategory::RecipeFragment);
    assert_eq!(template.rarity, ItemRarity::Uncommon);
    let spec = template
        .recipe_fragment_spec
        .as_ref()
        .expect("should have recipe_fragment_spec");
    assert_eq!(spec.recipe_id, "hui_yuan_pill_v0");
    assert_eq!(spec.known_stages, vec![0]);
    assert_eq!(spec.max_quality_tier, 3);
}

#[test]
fn recipe_fragment_spec_toml_roundtrip() {
    let original = RecipeFragmentSpec {
        recipe_id: "hui_yuan_pill_v0".to_string(),
        known_stages: vec![0],
        max_quality_tier: 3,
    };
    let json = serde_json::to_string(&original).expect("should serialize");
    let deserialized: RecipeFragmentSpec = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(
        original, deserialized,
        "RecipeFragmentSpec roundtrip failed"
    );
}

#[test]
fn parse_recipe_fragment_spec_happy_path() {
    let raw = RecipeFragmentSpecToml {
        recipe_id: "hui_yuan_pill_v0".to_string(),
        known_stages: vec![0],
        max_quality_tier: 3,
    };
    let spec = parse_recipe_fragment_spec(raw, Path::new("test.toml"), "test_item").unwrap();
    assert_eq!(spec.recipe_id, "hui_yuan_pill_v0");
    assert_eq!(spec.known_stages, vec![0]);
    assert_eq!(spec.max_quality_tier, 3);
}

#[test]
fn parse_recipe_fragment_spec_empty_recipe_id() {
    let raw = RecipeFragmentSpecToml {
        recipe_id: String::new(),
        known_stages: vec![0],
        max_quality_tier: 1,
    };
    let err = parse_recipe_fragment_spec(raw, Path::new("test.toml"), "bad_item")
        .expect_err("empty recipe_id should fail");
    assert!(
        err.contains("test.toml"),
        "error should mention source path; got: {err}"
    );
}

#[test]
fn parse_recipe_fragment_spec_empty_known_stages() {
    let raw = RecipeFragmentSpecToml {
        recipe_id: "some_recipe".to_string(),
        known_stages: vec![],
        max_quality_tier: 1,
    };
    let err = parse_recipe_fragment_spec(raw, Path::new("test.toml"), "bad_item")
        .expect_err("empty known_stages should fail");
    assert!(
        err.contains("known_stages must not be empty"),
        "error should mention known_stages; got: {err}"
    );
}

#[test]
fn parse_recipe_fragment_spec_max_quality_tier_out_of_range() {
    for bad_tier in [0, 4] {
        let raw = RecipeFragmentSpecToml {
            recipe_id: "some_recipe".to_string(),
            known_stages: vec![0],
            max_quality_tier: bad_tier,
        };
        let err = parse_recipe_fragment_spec(raw, Path::new("test.toml"), "bad_item")
            .expect_err(&format!("tier {bad_tier} should fail"));
        assert!(
            err.contains("max_quality_tier"),
            "error should mention max_quality_tier for tier {bad_tier}; got: {err}"
        );
    }
}

#[test]
fn technique_scroll_reference_validation_accepts_checked_in_items() {
    let items = load_item_registry().expect("checked-in item registry should load");
    let techniques = crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests();

    validate_technique_scroll_references(&items, &techniques)
        .expect("all checked-in technique scroll ids must resolve at startup");
}

#[test]
fn technique_scroll_reference_validation_aggregates_unknown_ids_deterministically() {
    let mut first = make_misc_template("z_scroll");
    first.technique_scroll_spec = Some(TechniqueScrollSpec {
        kind: "combat_technique".to_string(),
        skill_id: "missing.z".to_string(),
    });
    let mut second = make_misc_template("a_scroll");
    second.technique_scroll_spec = Some(TechniqueScrollSpec {
        kind: "combat_technique".to_string(),
        skill_id: "missing.a".to_string(),
    });
    let items = ItemRegistry::from_map(HashMap::from([
        (first.id.clone(), first),
        (second.id.clone(), second),
    ]));
    let techniques = crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests();

    let error = validate_technique_scroll_references(&items, &techniques)
        .expect_err("dangling technique scroll references must reject startup");
    assert_eq!(
        error,
        "invalid technique scroll references:\n- item `a_scroll` references unknown technique_scroll.skill_id `missing.a`\n- item `z_scroll` references unknown technique_scroll.skill_id `missing.z`"
    );
}

#[test]
fn scroll_sword_cleave_skill_id_matches_definition() {
    // Verify that the skill_id in the scroll matches a valid registry definition.
    let techniques = crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests();
    let def = techniques.get("sword.cleave");
    assert!(
        def.is_some(),
        "sword.cleave should be a registered technique definition"
    );
}

// ── plan-food-v1 P0 — 食物物品模板加载测试 ──

#[test]
fn food_item_templates_load_from_assets() {
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");

    // happy path: 五个食物 ID 均可查到
    for id in [
        "food.mundane.cooked_meat",
        "food.mundane.chen_bing",
        "food.spirit_fruit.ling_guo",
        "food.spirit_wine.chen_jiu",
        "food.spirit_wine.chen_cu",
    ] {
        assert!(
            registry.get(id).is_some(),
            "food item `{id}` should load from food.toml — 确认 TOML 已添加并 category=food"
        );
    }
}

#[test]
fn food_item_templates_have_food_category() {
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");

    for id in [
        "food.mundane.cooked_meat",
        "food.mundane.chen_bing",
        "food.spirit_fruit.ling_guo",
        "food.spirit_wine.chen_jiu",
        "food.spirit_wine.chen_cu",
    ] {
        let tpl = registry
            .get(id)
            .unwrap_or_else(|| panic!("{id} must be in registry"));
        assert_eq!(
            tpl.category,
            ItemCategory::Food,
            "item `{id}` should have category=Food because plan-food-v1 P0 requires food category; \
             check parse_item_category food arm and TOML category field"
        );
    }
}

#[test]
fn food_item_default_stack_count_is_16() {
    // ItemCategory::Food stacks up to 16, same as Pill/Misc
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");

    let cooked_meat = registry
        .get("food.mundane.cooked_meat")
        .expect("food.mundane.cooked_meat must exist");
    assert_eq!(
        cooked_meat.max_stack_count, 16,
        "food items default to stack 16 because ItemCategory::Food is in same arm as Pill/Misc"
    );

    let ling_guo = registry
        .get("food.spirit_fruit.ling_guo")
        .expect("food.spirit_fruit.ling_guo must exist");
    assert_eq!(
        ling_guo.max_stack_count, 16,
        "ling_guo stack should be 16 because Food category has same default as Misc"
    );
}

#[test]
fn food_item_spirit_quality_initial_is_in_range() {
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");

    let cases: &[(&str, f64, f64)] = &[
        ("food.mundane.cooked_meat", 0.30, 0.50),
        ("food.mundane.chen_bing", 0.25, 0.50),
        ("food.spirit_fruit.ling_guo", 0.60, 0.80),
        ("food.spirit_wine.chen_jiu", 0.70, 0.90),
        ("food.spirit_wine.chen_cu", 0.55, 0.75),
    ];
    for (id, lo, hi) in cases {
        let tpl = registry
            .get(id)
            .unwrap_or_else(|| panic!("{id} must exist"));
        assert!(
            tpl.spirit_quality_initial >= *lo && tpl.spirit_quality_initial <= *hi,
            "item `{id}` spirit_quality_initial {} out of expected range [{lo},{hi}] — \
             check food.toml values",
            tpl.spirit_quality_initial
        );
    }
}

#[test]
fn parse_item_category_food_arm_roundtrip() {
    // Verify parse_item_category correctly routes "food" string
    use std::path::PathBuf;
    let path = PathBuf::from("test_path.toml");
    let result = parse_item_category("food", &path, "test_id");
    assert!(
        matches!(result, Ok(ItemCategory::Food)),
        "parse_item_category(\"food\") should return Ok(ItemCategory::Food), got {result:?}"
    );
}

#[test]
fn parse_item_category_food_arm_case_insensitive() {
    use std::path::PathBuf;
    let path = PathBuf::from("test_path.toml");
    assert!(matches!(
        parse_item_category("Food", &path, "x"),
        Ok(ItemCategory::Food)
    ));
    assert!(matches!(
        parse_item_category("FOOD", &path, "x"),
        Ok(ItemCategory::Food)
    ));
    assert!(matches!(
        parse_item_category("  food  ", &path, "x"),
        Ok(ItemCategory::Food)
    ));
}

#[test]
fn parse_item_category_unknown_still_errors() {
    use std::path::PathBuf;
    let path = PathBuf::from("test.toml");
    assert!(
        parse_item_category("totally_unknown_category", &path, "id").is_err(),
        "unknown category should still return Err — food arm must not swallow others"
    );
}

// ── plan-gathering-tool-bind-v1 P0 — herb_bundle shelflife_profile 挂载测试 ──

#[test]
fn herb_bundle_item_template_has_shelflife_profile_set() {
    // plan-gathering-tool-bind-v1 §8.1 决议 #2：workbench_materials.toml 中
    // herb_bundle 应挂 shelflife_profile=fresh_herb_v1 + shelflife_track=spoil
    // （复用既有 profile，不派生 bundled_herb_v1）。
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");
    let tpl = registry
        .get("herb_bundle")
        .expect("herb_bundle must be in registry — check workbench_materials.toml");
    assert_eq!(
        tpl.shelflife_profile.as_deref(),
        Some("fresh_herb_v1"),
        "herb_bundle should have shelflife_profile=`fresh_herb_v1` because \
         plan-gathering-tool-bind-v1 P0 挂载已存在的 profile，不新增 bundled_herb_v1"
    );
    assert_eq!(
        tpl.shelflife_track,
        Some(crate::shelflife::DecayTrack::Spoil),
        "herb_bundle should have shelflife_track=Spoil（灵草束会腐败，不是衰减/陈化）"
    );
}

// ── plan-food-v1 P1 — 食物物品 shelflife_profile 初始化测试 ──

#[test]
fn food_item_templates_have_shelflife_profile_set() {
    // plan-food-v1 P1：food.toml 中每个食物 item 应声明 shelflife_profile + shelflife_track
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");

    let cases: &[(&str, &str, crate::shelflife::DecayTrack)] = &[
        (
            "food.mundane.cooked_meat",
            "food_spoil_mundane_meat_v1",
            crate::shelflife::DecayTrack::Spoil,
        ),
        (
            "food.mundane.chen_bing",
            "food_spoil_mundane_dry_v1",
            crate::shelflife::DecayTrack::Spoil,
        ),
        (
            "food.spirit_fruit.ling_guo",
            "food_spoil_ling_guo_v1",
            crate::shelflife::DecayTrack::Spoil,
        ),
        (
            "food.spirit_wine.chen_jiu",
            "chen_jiu_v1",
            crate::shelflife::DecayTrack::Age,
        ),
        (
            "food.spirit_wine.chen_cu",
            "chen_cu_v1",
            crate::shelflife::DecayTrack::Spoil,
        ),
    ];
    for (id, expected_profile, expected_track) in cases {
        let tpl = registry
            .get(id)
            .unwrap_or_else(|| panic!("{id} must be in registry — check food.toml"));
        assert_eq!(
            tpl.shelflife_profile.as_deref(),
            Some(*expected_profile),
            "item `{id}` should have shelflife_profile=`{expected_profile}` \
             because plan-food-v1 P1 requires food items to declare their decay profile in food.toml"
        );
        assert_eq!(
            tpl.shelflife_track,
            Some(*expected_track),
            "item `{id}` should have shelflife_track={expected_track:?} \
             because plan-food-v1 P1 assigns decay track in food.toml"
        );
    }
}

#[test]
fn runtime_instance_from_template_attaches_freshness_for_food_with_shelflife_profile() {
    use crate::shelflife::{DecayProfileId, DecayTrack};
    // plan-food-v1 P1：runtime_instance_from_template が shelflife_profile を持つ
    // テンプレートで Freshness を自動挂する。
    let tpl = ItemTemplate {
        id: "food.spirit_wine.chen_jiu".to_string(),
        display_name: "陈酒".to_string(),
        category: ItemCategory::Food,
        placeable: None,
        max_stack_count: 16,
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.5,
        rarity: ItemRarity::Uncommon,
        spirit_quality_initial: 0.80,
        description: "test".to_string(),
        effect: None,
        cast_duration_ms: DEFAULT_CAST_DURATION_MS,
        cooldown_ms: DEFAULT_COOLDOWN_MS,
        weapon_spec: None,
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
        readable_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: None,
        shield_spec: None,
        shelflife_profile: Some("chen_jiu_v1".to_string()),
        shelflife_track: Some(DecayTrack::Age),
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    };

    // plan-food-v1 MAJOR2: current_tick 传入 runtime_instance_from_template，
    // created_at_tick 应等于传入的 current_tick（不再硬编码 0）。
    let spawn_tick = 12345_u64;
    let instance = runtime_instance_from_template(&tpl, 1, 1, spawn_tick);
    let freshness = instance.freshness.as_ref().expect(
        "chen_jiu item should have Freshness attached by runtime_instance_from_template \
                 because template declares shelflife_profile=chen_jiu_v1",
    );
    assert_eq!(
        freshness.track,
        DecayTrack::Age,
        "freshness.track should be Age for chen_jiu (plan-food-v1 P1 Age track)"
    );
    assert_eq!(
        freshness.profile,
        DecayProfileId::new("chen_jiu_v1"),
        "freshness.profile must be chen_jiu_v1 as declared in food.toml"
    );
    assert_eq!(
        freshness.created_at_tick, spawn_tick,
        "freshness.created_at_tick must equal current_tick passed to runtime_instance_from_template; \
         hardcoding 0 causes elapsed=now-0 to pre-age items spawned mid-session"
    );
    assert!(
        (freshness.initial_qi - 0.80_f32).abs() < 1e-4,
        "freshness.initial_qi should equal spirit_quality_initial=0.80 cast to f32; \
         got {}",
        freshness.initial_qi
    );
    assert_eq!(
        freshness.frozen_accumulated, 0,
        "new item frozen_accumulated=0"
    );
    assert!(
        freshness.frozen_since_tick.is_none(),
        "new item frozen_since_tick=None"
    );
}

#[test]
fn runtime_instance_from_template_attaches_freshness_for_herb_bundle() {
    use crate::shelflife::{DecayProfileId, DecayTrack};
    // plan-gathering-tool-bind-v1 P0：herb_bundle 挂 shelflife_profile 后，
    // runtime_instance_from_template 应像 food 物品一样自动挂 Freshness。
    let tpl = ItemTemplate {
        id: "herb_bundle".to_string(),
        display_name: "灵草束".to_string(),
        category: ItemCategory::Herb,
        placeable: None,
        max_stack_count: 16,
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.5,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 0.80,
        description: "test".to_string(),
        effect: None,
        cast_duration_ms: DEFAULT_CAST_DURATION_MS,
        cooldown_ms: DEFAULT_COOLDOWN_MS,
        weapon_spec: None,
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
        readable_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: None,
        shield_spec: None,
        shelflife_profile: Some("fresh_herb_v1".to_string()),
        shelflife_track: Some(DecayTrack::Spoil),
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    };

    let spawn_tick = 500_u64;
    let instance = runtime_instance_from_template(&tpl, 1, 1, spawn_tick);
    let freshness = instance.freshness.as_ref().expect(
        "herb_bundle should have Freshness attached by runtime_instance_from_template \
         because template declares shelflife_profile=fresh_herb_v1",
    );
    assert_eq!(
        freshness.track,
        DecayTrack::Spoil,
        "freshness.track should be Spoil for herb_bundle"
    );
    assert_eq!(
        freshness.profile,
        DecayProfileId::new("fresh_herb_v1"),
        "freshness.profile must be fresh_herb_v1 as declared in workbench_materials.toml"
    );
    assert_eq!(
        freshness.created_at_tick, spawn_tick,
        "freshness.created_at_tick must equal current_tick passed to runtime_instance_from_template"
    );
    assert!(
        (freshness.initial_qi - 0.80_f32).abs() < 1e-4,
        "freshness.initial_qi should equal spirit_quality_initial=0.80; got {}",
        freshness.initial_qi
    );
}

#[test]
fn herb_bundle_decays_identically_regardless_of_stack_count() {
    // plan-gathering-tool-bind-v1 §8.1 决议 #2 复核（PR #1293 review 修正）：上一版
    // 把"捆 vs 单株"两个样本都固定成同一份手写 `Freshness::new(0, 0.80, profile)`
    // 再喂进同一个纯函数——这是同输入自比较，恒真、什么都没有测出来。
    //
    // 本仓库里"捆"与"单株"的真实区别不是 initial_qi（herb_bundle 模板的
    // spirit_quality_initial 恒为 0.80，不管装了几株——见
    // herb_bundle_freshness_ignores_stack_count），而是 `ItemInstance.stack_count`。
    // 所以有判别力的对照必须固定 profile/initial_qi、只变 stack_count，并且要走生产
    // `runtime_instance_from_template` 链路（不是手搓 Freshness）：如果未来有人在这条
    // 链路上给批量物品加了"stack_count 越大衰减越慢"的折扣，这里必须撞红——
    // 已用假实现验证过（临时给 initial_qi 加 stack_count 相关的加成，本测试确实失败；
    // 验证后已还原，不留在正式代码里）。
    use crate::shelflife::registry::build_default_registry;
    use crate::shelflife::{compute::compute_current_qi, DecayProfileId};

    let item_registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");
    let profile_registry = build_default_registry();
    let tpl = item_registry
        .get("herb_bundle")
        .expect("herb_bundle must be in registry — check workbench_materials.toml");
    let profile = profile_registry
        .get(&DecayProfileId::new("fresh_herb_v1"))
        .expect("fresh_herb_v1 must exist in the production DecayProfileRegistry");

    // "单株"：stack_count=1；"捆"：stack_count=50——两者都经真实生产入口构造。
    let single = runtime_instance_from_template(tpl, 1, 1, 0);
    let bundle = runtime_instance_from_template(tpl, 2, 50, 0);
    let single_freshness = single
        .freshness
        .expect("herb_bundle instance must carry Freshness (shelflife_profile is set)");
    let bundle_freshness = bundle
        .freshness
        .expect("herb_bundle instance must carry Freshness (shelflife_profile is set)");

    for elapsed_ticks in [0_u64, 12_000, 36_000, 57_600, 200_000] {
        let single_current = compute_current_qi(&single_freshness, profile, elapsed_ticks, 1.0);
        let bundle_current = compute_current_qi(&bundle_freshness, profile, elapsed_ticks, 1.0);
        assert_eq!(
            single_current, bundle_current,
            "在 tick={elapsed_ticks} 时，stack_count=1（单株）与 stack_count=50（捆）经由\
             生产 runtime_instance_from_template 构造的实例应输出完全相同的 current_qi；\
             若此处不等，说明有人给批量存放加了未经决议的衰减折扣"
        );
    }
}

#[test]
fn herb_bundle_freshness_ignores_stack_count() {
    // plan-gathering-tool-bind-v1 §8.1 决议 #2 的另一面："捆"在本仓库是通过
    // `stack_count`（一个 ItemInstance 里装了几株）承载数量的，不是通过新 profile。
    // 这条测试直接锁住 runtime_instance_from_template 生成的 Freshness 与 stack_count
    // 无关——如果未来有人在 runtime_instance_from_template 或别处加了
    // "stack_count 越大衰减越慢"的批量折扣，这里会撞红。
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");
    let tpl = registry
        .get("herb_bundle")
        .expect("herb_bundle must be in registry — check workbench_materials.toml");

    let single_stack = runtime_instance_from_template(tpl, 1, 1, 500);
    let bulk_stack = runtime_instance_from_template(tpl, 2, 99, 500);

    assert_eq!(
        single_stack.freshness, bulk_stack.freshness,
        "stack_count=1 与 stack_count=99 生成的 Freshness 必须完全相等（逐字段）——\
         批量存放不应该获得任何未经决议的衰减折扣。single={:?}, bulk={:?}",
        single_stack.freshness, bulk_stack.freshness
    );
}

#[test]
fn herb_bundle_decay_curve_reaches_spoiled_state_over_three_game_days() {
    // plan-gathering-tool-bind-v1 P0："herb_bundle 实例随时间衰减曲线"——覆盖 happy
    // path（刚做出来是 Fresh）、中段（Declining）、彻底腐败（Spoiled，current 触底 0）。
    use crate::shelflife::compute::{compute_current_qi, compute_track_state};
    use crate::shelflife::registry::build_default_registry;
    use crate::shelflife::{DecayProfileId, Freshness, TrackState};

    let profile_registry = build_default_registry();
    let profile = profile_registry
        .get(&DecayProfileId::new("fresh_herb_v1"))
        .expect("fresh_herb_v1 must exist in the production DecayProfileRegistry");
    let freshness = Freshness::new(0, 0.80, profile);

    // t=0：刚扎束，应为满品质 Fresh。
    let at_zero = compute_current_qi(&freshness, profile, 0, 1.0);
    assert!(
        (at_zero - 0.80).abs() < 1e-4,
        "t=0 时 current_qi 应等于 initial_qi=0.80，实际 {at_zero}（懒计算不应在创建瞬间就衰减）"
    );
    assert_eq!(
        compute_track_state(&freshness, profile, 0, 1.0),
        TrackState::Fresh,
        "t=0 时 TrackState 应为 Fresh"
    );

    // fresh_herb_v1 是 3 游戏日线性归零（FRESH_HERB_TOTAL_TICKS = GAME_DAY_TICKS*3）。
    const GAME_DAY_TICKS: u64 = 24_000;
    let total_ticks = GAME_DAY_TICKS * 3;

    // 中点：仍有剩余但已过半，应处于 Declining（headroom 剩余 <= 50%）。
    let midpoint = total_ticks / 2;
    let at_mid = compute_current_qi(&freshness, profile, midpoint, 1.0);
    assert!(
        at_mid > 0.0 && at_mid < 0.80,
        "半程 current_qi 应严格介于 0 和 initial_qi 之间，实际 {at_mid}"
    );
    assert_eq!(
        compute_track_state(&freshness, profile, midpoint, 1.0),
        TrackState::Declining,
        "半程（headroom 剩余 50%）TrackState 应为 Declining"
    );

    // t >= total_ticks：完全腐败——current 触底 0（Spoil 路径 floor 是 0，不是 fauna 那种正 floor_qi），
    // TrackState 应为 Spoiled。
    let at_end = compute_current_qi(&freshness, profile, total_ticks, 1.0);
    assert_eq!(
        at_end, 0.0,
        "t=total_ticks 时 current_qi 应触底为 0（Spoil 公式 max(0.0, ...)），实际 {at_end}"
    );
    assert_eq!(
        compute_track_state(&freshness, profile, total_ticks, 1.0),
        TrackState::Spoiled,
        "腐败阈值以下 TrackState 应为 Spoiled"
    );

    // 过期之后继续推进时间：仍应是 0 / Spoiled，不会变负或回弹（过期行为/腐坏产物分支）。
    let long_after = compute_current_qi(&freshness, profile, total_ticks * 10, 1.0);
    assert_eq!(
        long_after, 0.0,
        "远超过期时间后 current_qi 仍应为 0，不应变负，实际 {long_after}"
    );
    assert_eq!(
        compute_track_state(&freshness, profile, total_ticks * 10, 1.0),
        TrackState::Spoiled,
        "远超过期时间后 TrackState 仍应为 Spoiled"
    );
}

#[test]
fn herb_bundle_expiry_drives_production_spoil_check_consumption_path() {
    // plan-gathering-tool-bind-v1 P0："过期行为（腐坏产物）分支"（PR #1293 review 修正）：
    // 上一版只调 compute_track_state 断言 Spoiled，没有走任何"过期之后会怎样"的生产分支。
    //
    // fresh_herb_v1 是 `DecayProfile::Spoil`。variant.rs 的生产 sweep
    // （apply_variant_switch_with_season_and_container）对 Spoil 路径的 Spoiled 状态
    // 明确"不切 item ID，走 NBT"——只有 Decay（ling_shi/bone_coin）和 Age→Spoil
    // 迁移（chen_jiu→chen_cu）会切 item ID。herb_bundle 腐败后不会变成另一个 item
    // template，所以"腐坏产物"在这个系统里的真实含义不是"变成另一件东西"，而是
    // `shelflife::consume::spoil_check` 返回的消费判定（Safe/Warn 触发 contam
    // 警告/CriticalBlock 拒绝消费）——这正是 food.rs::consume_food 的 Spoil 分支
    // 已经在用的生产入口。这里直接用 herb_bundle 的真实 registry 模板 + 生产
    // runtime_instance_from_template 构造实例，驱动 spoil_check 走过 Safe→Warn→
    // CriticalBlock 三段，锁住这条真实存在的过期消费路径。
    use crate::shelflife::consume::{spoil_check, SpoilCheckOutcome};
    use crate::shelflife::registry::build_default_registry;
    use crate::shelflife::DecayProfileId;

    let item_registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");
    let profile_registry = build_default_registry();
    let tpl = item_registry
        .get("herb_bundle")
        .expect("herb_bundle must be in registry — check workbench_materials.toml");
    let profile = profile_registry
        .get(&DecayProfileId::new("fresh_herb_v1"))
        .expect("fresh_herb_v1 must exist in the production DecayProfileRegistry");
    let instance = runtime_instance_from_template(tpl, 1, 1, 0);
    let freshness = instance
        .freshness
        .expect("herb_bundle instance must carry Freshness (shelflife_profile is set)");

    // current(t) = max(0, 0.80 - t/72000)（Linear，见 fresh_herb_profile()）。
    // spoil_threshold=0.01 在 t=56880 处被跨越；0.1×threshold=0.001 在 t=57528 处
    // 被跨越；current 在 t=57600 触底为 0。三个采样点都留了充足余量，不卡边界。

    // t=0：current=0.80 ≥ threshold=0.01 → Safe。
    match spoil_check(&freshness, profile, 0, 1.0) {
        SpoilCheckOutcome::Safe { current_qi } => {
            assert!(
                (current_qi - 0.80).abs() < 1e-4,
                "t=0 时 herb_bundle 应处于 Safe 且 current_qi=initial_qi=0.80，实际 {current_qi}"
            );
        }
        other => panic!("t=0 时 herb_bundle 的 spoil_check 应为 Safe，实际 {other:?}"),
    }

    // t=57_200：current≈0.00556，介于 [0.001, 0.01) → Warn（触发 contam 警告，非拒绝消费）。
    match spoil_check(&freshness, profile, 57_200, 1.0) {
        SpoilCheckOutcome::Warn {
            current_qi,
            spoil_threshold,
        } => {
            assert!(
                (spoil_threshold - 0.01).abs() < 1e-4,
                "spoil_threshold 应为 fresh_herb_v1 注册的 0.01，实际 {spoil_threshold}"
            );
            assert!(
                current_qi < spoil_threshold && current_qi > 0.0,
                "Warn 分支 current_qi 应严格小于 threshold 且未触底，实际 {current_qi}"
            );
        }
        other => panic!(
            "t=57_200（跨过 spoil_threshold 但未到 0.1×threshold）herb_bundle 应进入 Warn，\
             实际 {other:?}"
        ),
    }

    // t=60_000（已触底 current=0）：远低于 0.1×threshold=0.001 → CriticalBlock（拒绝自动消费）。
    match spoil_check(&freshness, profile, 60_000, 1.0) {
        SpoilCheckOutcome::CriticalBlock {
            current_qi,
            spoil_threshold,
        } => {
            assert!(
                current_qi < 0.1 * spoil_threshold,
                "CriticalBlock 分支 current_qi 应低于 0.1×threshold，实际 {current_qi}"
            );
        }
        other => panic!(
            "t=60_000（远超过期阈值）herb_bundle 应进入 CriticalBlock 拒绝消费，实际 {other:?}"
        ),
    }
}

#[test]
fn runtime_instance_from_template_no_freshness_when_no_shelflife_profile() {
    // Non-food items (or food without shelflife_profile) should have freshness=None
    let tpl = ItemTemplate {
        id: "misc_thing".to_string(),
        display_name: "misc".to_string(),
        category: ItemCategory::Misc,
        placeable: None,
        max_stack_count: 1,
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.1,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 1.0,
        description: "no shelflife".to_string(),
        effect: None,
        cast_duration_ms: DEFAULT_CAST_DURATION_MS,
        cooldown_ms: DEFAULT_COOLDOWN_MS,
        weapon_spec: None,
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
        readable_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: None,
        shield_spec: None,

        shelflife_profile: None,
        shelflife_track: None,
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    };

    let instance = runtime_instance_from_template(&tpl, 1, 1, 0);
    assert!(
        instance.freshness.is_none(),
        "item without shelflife_profile should have freshness=None — \
         only items with shelflife_profile in food.toml get auto-freshness"
    );
}

#[test]
fn chen_jiu_item_from_registry_has_age_freshness_on_spawn() {
    // End-to-end: load food.toml item, instantiate, verify freshness is Age track.
    use crate::shelflife::DecayTrack;
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");

    let tpl = registry
        .get("food.spirit_wine.chen_jiu")
        .expect("food.spirit_wine.chen_jiu must be loadable from food.toml");

    let instance = runtime_instance_from_template(tpl, 99, 1, 0);
    let freshness = instance.freshness.as_ref().expect(
        "food.spirit_wine.chen_jiu should have freshness auto-attached because \
         food.toml declares shelflife_profile=chen_jiu_v1",
    );
    assert_eq!(
        freshness.track,
        DecayTrack::Age,
        "chen_jiu template spawns with Age track because chen_jiu_v1 is an Age profile"
    );
}

#[test]
fn ling_guo_item_from_registry_has_spoil_freshness_on_spawn() {
    use crate::shelflife::DecayTrack;
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");

    let tpl = registry
        .get("food.spirit_fruit.ling_guo")
        .expect("food.spirit_fruit.ling_guo must exist");

    let instance = runtime_instance_from_template(tpl, 1, 1, 0);
    let freshness = instance.freshness.as_ref().expect(
        "ling_guo should have freshness because food.toml declares shelflife_profile=food_spoil_ling_guo_v1"
    );
    assert_eq!(
        freshness.track,
        DecayTrack::Spoil,
        "ling_guo spawns with Spoil track — it decays in 2 game days"
    );
}

#[test]
fn shelflife_track_parse_invalid_rejects_with_error() {
    // plan-food-v1 P1：无效的 shelflife_track 字符串应在 TOML 解析时报错。
    use std::path::PathBuf;
    let path = PathBuf::from("test_path.toml");

    let raw = ItemTemplateToml {
        id: "test_item".to_string(),
        name: "Test".to_string(),
        category: "food".to_string(),
        placeable: None,
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.1,
        rarity: "common".to_string(),
        spirit_quality_initial: 0.5,
        description: "test".to_string(),
        max_stack_count: None,
        effect: None,
        cast_duration_ms: None,
        cooldown_ms: None,
        weapon: None,
        forge_station: None,
        blueprint_scroll: None,
        inscription_scroll: None,
        technique_scroll: None,
        readable_scroll: None,
        recipe_fragment: None,
        container: None,
        shield_spec: None,
        shelflife_profile: Some("some_profile".to_string()),
        shelflife_track: Some("INVALID_TRACK".to_string()),
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    };

    let result = raw.try_into_item_template(&path);
    assert!(
        result.is_err(),
        "ItemTemplateToml with invalid shelflife_track should fail try_into_item_template"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("shelflife_track"),
        "error message should mention shelflife_track; got: {err}"
    );
}

#[test]
fn shelflife_track_defaults_to_spoil_when_not_specified() {
    // shelflife_profile は Some だが shelflife_track を省略 → デフォルト spoil。
    use crate::shelflife::DecayTrack;
    use std::path::PathBuf;
    let path = PathBuf::from("test_path.toml");

    let raw = ItemTemplateToml {
        id: "test_item".to_string(),
        name: "Test".to_string(),
        category: "food".to_string(),
        placeable: None,
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.1,
        rarity: "common".to_string(),
        spirit_quality_initial: 0.5,
        description: "test".to_string(),
        max_stack_count: None,
        effect: None,
        cast_duration_ms: None,
        cooldown_ms: None,
        weapon: None,
        forge_station: None,
        blueprint_scroll: None,
        inscription_scroll: None,
        technique_scroll: None,
        readable_scroll: None,
        recipe_fragment: None,
        container: None,
        shelflife_profile: Some("some_profile".to_string()),
        shield_spec: None,
        shelflife_track: None, // should default to "spoil"
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    };

    let tpl = raw
        .try_into_item_template(&path)
        .expect("valid TOML should parse OK");
    assert_eq!(
        tpl.shelflife_track,
        Some(DecayTrack::Spoil),
        "when shelflife_track is omitted but shelflife_profile is present, \
         shelflife_track defaults to Spoil"
    );
}

// ── plan-food-v1 P1 (CodeRabbit 补测) — shelflife 半配置报错 ──

/// 负向：shelflife_track=Some 但 shelflife_profile=None → try_into_item_template 必须报错，
/// 且错误信息含 "shelflife_track"（防止半配置静默绕过 freshness gate）。
#[test]
fn shelflife_track_without_profile_is_rejected() {
    use std::path::PathBuf;
    let path = PathBuf::from("test_path.toml");

    let raw = ItemTemplateToml {
        id: "bad_food_half_config".to_string(),
        name: "半配置食物".to_string(),
        category: "food".to_string(),
        placeable: None,
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.1,
        rarity: "common".to_string(),
        spirit_quality_initial: 1.0,
        description: "shelflife_track 有值但 profile 为 None".to_string(),
        max_stack_count: None,
        effect: None,
        cast_duration_ms: None,
        cooldown_ms: None,
        weapon: None,
        forge_station: None,
        blueprint_scroll: None,
        inscription_scroll: None,
        technique_scroll: None,
        readable_scroll: None,
        recipe_fragment: None,
        container: None,
        shield_spec: None,
        shelflife_profile: None,                    // ← 故意缺失
        shelflife_track: Some("spoil".to_string()), // ← 有值但 profile 为 None → 报错
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    };

    let result = raw.try_into_item_template(&path);
    assert!(
        result.is_err(),
        "shelflife_track 有值但 shelflife_profile=None 时 try_into_item_template 必须返回 Err，\
         否则 freshness gate 会被静默绕过"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("shelflife_track"),
        "错误信息应提到 shelflife_track 字段，方便定位半配置问题；实际错误：{err}"
    );
    assert!(
        err.contains("shelflife_profile"),
        "错误信息应同时提到 shelflife_profile 字段，方便定位半配置问题；实际错误：{err}"
    );
}

/// 正向对照：shelflife_profile=Some + shelflife_track=Some("spoil") → 正常解析，
/// track=Spoil，不报错。
#[test]
fn shelflife_track_and_profile_both_some_is_accepted() {
    use crate::shelflife::DecayTrack;
    use std::path::PathBuf;
    let path = PathBuf::from("test_path.toml");

    let raw = ItemTemplateToml {
        id: "good_food_full_config".to_string(),
        name: "完整配置食物".to_string(),
        category: "food".to_string(),
        placeable: None,
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.1,
        rarity: "common".to_string(),
        spirit_quality_initial: 1.0,
        description: "shelflife_track + profile 均有值".to_string(),
        max_stack_count: None,
        effect: None,
        cast_duration_ms: None,
        cooldown_ms: None,
        weapon: None,
        forge_station: None,
        blueprint_scroll: None,
        inscription_scroll: None,
        technique_scroll: None,
        readable_scroll: None,
        recipe_fragment: None,
        container: None,
        shield_spec: None,
        shelflife_profile: Some("my_spoil_profile_v1".to_string()), // ← 正确配对
        shelflife_track: Some("spoil".to_string()),
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    };

    let tpl = raw
        .try_into_item_template(&path)
        .expect("shelflife_profile + shelflife_track 均 Some 时应正常解析");
    assert_eq!(
        tpl.shelflife_profile.as_deref(),
        Some("my_spoil_profile_v1"),
        "shelflife_profile 应原样保留在解析结果中"
    );
    assert_eq!(
        tpl.shelflife_track,
        Some(DecayTrack::Spoil),
        "shelflife_track='spoil' 应解析为 DecayTrack::Spoil"
    );
}

// ── plan-shield-block-v1 P0 — ItemCategory::Shield 饱和化测试 ──────────────

/// Shield 变体 serde 正反对拍（happy path）：序列化后再反序列化须还原原值。
#[test]
fn item_category_shield_serde_roundtrip() {
    let cat = ItemCategory::Shield;
    let json = serde_json::to_string(&cat).expect(
        "期望 Shield 变体可序列化为 JSON，\
         实际 serde_json::to_string 失败",
    );
    let parsed: ItemCategory = serde_json::from_str(&json).expect(
        "期望 JSON 字符串可反序列化回 ItemCategory::Shield，\
         实际 serde_json::from_str 失败",
    );
    assert_eq!(
        parsed,
        ItemCategory::Shield,
        "期望 serde roundtrip 结果为 Shield，\
         实际得到 {parsed:?}"
    );
}

/// parse_item_category("shield") 应返回 ItemCategory::Shield。
#[test]
fn parse_item_category_shield_happy() {
    use std::path::PathBuf;
    let path = PathBuf::from("test.toml");
    let result = parse_item_category("shield", &path, "wooden_shield");
    assert!(
        matches!(result, Ok(ItemCategory::Shield)),
        "期望 parse_item_category(\"shield\") = Ok(Shield)，因为 plan-shield-block-v1 P0 加了 shield 分支，\
         实际得到 {result:?}"
    );
}

/// parse_item_category("Shield")（首字母大写）因 to_ascii_lowercase 后应也命中 shield 分支。
#[test]
fn parse_item_category_shield_case_insensitive() {
    use std::path::PathBuf;
    let path = PathBuf::from("test.toml");
    let result = parse_item_category("Shield", &path, "wooden_shield");
    assert!(
        matches!(result, Ok(ItemCategory::Shield)),
        "期望 parse_item_category(\"Shield\") 因 trim+to_ascii_lowercase 后命中 shield 分支，\
         实际得到 {result:?}"
    );
}

/// parse_item_category("") 应返回 Err（未知 category 分支）。
#[test]
fn parse_item_category_empty_string_errors() {
    use std::path::PathBuf;
    let path = PathBuf::from("test.toml");
    let result = parse_item_category("", &path, "x");
    assert!(
        result.is_err(),
        "期望 parse_item_category(\"\") 返回 Err（空字符串不是合法 category），\
         实际得到 {result:?}"
    );
}

/// ItemCategory::Shield 的 max_stack_count 应为 1（与武器/防具同级，不可叠加）。
#[test]
fn shield_category_default_stack_count_is_one() {
    assert_eq!(
        default_max_stack_count_for_category(ItemCategory::Shield),
        1,
        "期望 Shield max_stack_count = 1，因为盾牌与武器/防具同级不可叠加，\
         实际得到 {}",
        default_max_stack_count_for_category(ItemCategory::Shield)
    );
}

/// workbench_materials.toml 中 wooden_shield / bone_shield 应以 ItemCategory::Shield 加载。
#[test]
fn shield_templates_load_with_shield_category() {
    let registry =
        load_item_registry().expect("item registry should load from assets/items/*.toml");

    for id in ["wooden_shield", "bone_shield"] {
        let tpl = registry.get(id).unwrap_or_else(|| {
            panic!(
                "期望 item `{id}` 在 registry 中存在，\
                 实际未找到——检查 workbench_materials.toml 是否包含该 id"
            )
        });
        assert_eq!(
            tpl.category,
            ItemCategory::Shield,
            "期望 item `{id}` category = Shield（plan-shield-block-v1 P0 改 category），\
             实际得到 {:?}",
            tpl.category
        );
    }
}

/// 盾牌装入 off_hand 应成功（happy path）。
#[test]
fn apply_move_shield_to_off_hand_succeeds() {
    use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.template_id = "wooden_shield".to_string();
    inv.containers[0].items[0].instance.display_name = "木盾".to_string();
    inv.containers[0].items[0].instance.grid_h = 2;

    let result = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Equip {
            slot: EquipSlotV1::OffHand,
            state: crate::schema::inventory::EquipStateV1::Held,
        },
        false,
    );
    assert!(
        result.is_ok(),
        "期望 wooden_shield 装入 off_hand 成功（plan-shield-block-v1 P0 消灭孤岛根因），\
         实际得到错误：{result:?}"
    );
    // MINOR #3 — 锁住「槽位真被盾占用」：断言 equipped 里 OFF_HAND 槽存在且 template_id 正确。
    assert_eq!(
        inv.equipped
            .get(EQUIP_SLOT_OFF_HAND)
            .and_then(|s| s.held.as_ref())
            .map(|item| item.template_id.as_str()),
        Some("wooden_shield"),
        "期望 OFF_HAND 槽被 wooden_shield 占用（plan-shield-block-v1 P0 post-state 断言），\
         实际 equipped[off_hand] = {:?}",
        inv.equipped
            .get(EQUIP_SLOT_OFF_HAND)
            .and_then(|s| s.held.as_ref())
            .map(|i| &i.template_id)
    );
}

/// 主手持双手兵器（锁对侧）时拒绝装 off_hand 盾（边界，two_hand 槽已删，改对侧锁）。
#[test]
fn apply_move_shield_to_off_hand_rejected_when_main_hand_two_handed() {
    use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.template_id = "wooden_shield".to_string();
    inv.containers[0].items[0].instance.display_name = "木盾".to_string();
    inv.containers[0].items[0].instance.grid_h = 2;
    // main_hand 持双手杖（staff 派生 two-handed），锁 off_hand。
    inv.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(ItemInstance {
            instance_id: 99,
            template_id: "wooden_staff".to_string(),
            display_name: "木杖".to_string(),
            grid_w: 1,
            grid_h: 3,
            weight: 2.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }),
    );

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Equip {
            slot: EquipSlotV1::OffHand,
            state: crate::schema::inventory::EquipStateV1::Held,
        },
        false,
    )
    .expect_err(
        "期望主手双手兵器锁住 off_hand 时装盾被拒绝，\
         实际返回 Ok——对侧锁校验漏掉",
    );

    assert!(
        matches!(error, InventoryMoveRejectReason::TwoHandedLocksOther),
        "期望对侧双手锁拒绝，实际：{error:?}"
    );
}

/// 非盾非 treasure 非 dagger 物品装 off_hand 仍按原逻辑拒绝（回归保护）。
#[test]
fn apply_move_non_shield_non_treasure_non_dagger_off_hand_still_rejected() {
    use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    // iron_sword：Weapon 但非 Dagger/Fist，应被拒
    inv.containers[0].items[0].instance.template_id = "iron_sword".to_string();
    inv.containers[0].items[0].instance.display_name = "凡铁剑".to_string();
    inv.containers[0].items[0].instance.grid_h = 2;

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Equip {
            slot: EquipSlotV1::OffHand,
            state: crate::schema::inventory::EquipStateV1::Held,
        },
        false,
    )
    .expect_err(
        "期望 iron_sword 装 off_hand 被拒绝（非盾非 treasure 非 dagger），\
         实际返回 Ok——Shield 分支意外放行了其他类别",
    );

    assert!(
        matches!(error, InventoryMoveRejectReason::OffHandTypeMismatch),
        "期望 off_hand 武器类型不符拒绝，实际：{error:?}"
    );
}

/// equip_slot_for_item_id("armor_iron_chestplate") 正向返回 Some(Chest)（路由回归正向断言）。
#[test]
fn equip_slot_for_item_id_armor_still_routes_correctly() {
    use crate::armor::mundane::equip_slot_for_item_id;
    use crate::schema::inventory::EquipSlotV1;
    let slot = equip_slot_for_item_id("armor_iron_chestplate");
    // MINOR #4 — MundaneArmorSlot::Chestplate → EquipSlotV1::Chest；
    // 正向断言锁住「iron chestplate 确实路由到 Chest 槽」，防 Shield 分支意外影响 Armor routing。
    assert_eq!(
        slot,
        Some(EquipSlotV1::Chest),
        "期望 equip_slot_for_item_id(\"armor_iron_chestplate\") == Some(Chest)，\
         plan-shield-block-v1 P0 不改此函数；实际得到 {slot:?}"
    );
}

/// equip_slot_for_item_id("wooden_shield") 仍返回 None（盾不经此函数）。
#[test]
fn equip_slot_for_item_id_wooden_shield_returns_none() {
    use crate::armor::mundane::equip_slot_for_item_id;
    let slot = equip_slot_for_item_id("wooden_shield");
    assert!(
        slot.is_none(),
        "期望 equip_slot_for_item_id(\"wooden_shield\") = None，\
         因为盾走 EquipSlotV1::OffHand 3799 arm 不走此函数，\
         实际得到 {slot:?}"
    );
}

// ── plan-shield-block-v1 P2 — ShieldSpec 饱和化测试 ──────────────────────

/// ShieldSpec.validate() happy path：wooden_shield 规格通过验证。
#[test]
fn shield_spec_validate_happy_path_wooden_shield() {
    let spec = ShieldSpec {
        block_ratio: 0.5,
        durability_max: 40.0,
        stamina_drain_per_s: 3.0,
    };
    assert!(
        spec.validate("wooden_shield").is_ok(),
        "wooden_shield 规格（0.5/40/3.0）应通过 validate()；\
         实际 Err: {:?}",
        spec.validate("wooden_shield")
    );
}

/// ShieldSpec.validate() happy path：bone_shield 规格通过验证。
#[test]
fn shield_spec_validate_happy_path_bone_shield() {
    let spec = ShieldSpec {
        block_ratio: 0.65,
        durability_max: 80.0,
        stamina_drain_per_s: 3.0,
    };
    assert!(
        spec.validate("bone_shield").is_ok(),
        "bone_shield 规格（0.65/80/3.0）应通过 validate()；\
         实际 Err: {:?}",
        spec.validate("bone_shield")
    );
}

/// block_ratio = 0.7（上限）仍通过验证（边界：上界包含）。
#[test]
fn shield_spec_validate_block_ratio_max_boundary_passes() {
    let spec = ShieldSpec {
        block_ratio: 0.7,
        durability_max: 40.0,
        stamina_drain_per_s: 3.0,
    };
    assert!(
        spec.validate("test_shield").is_ok(),
        "block_ratio=0.7（worldview §五 凡人盾上限）应通过 validate()（>= 包含边界），\
         实际 Err: {:?}",
        spec.validate("test_shield")
    );
}

/// block_ratio > 0.7 被拒绝（超出凡人盾上限）。
#[test]
fn shield_spec_validate_block_ratio_above_max_rejected() {
    let spec = ShieldSpec {
        block_ratio: 0.71,
        durability_max: 40.0,
        stamina_drain_per_s: 3.0,
    };
    let err = spec
        .validate("cheat_shield")
        .expect_err("block_ratio=0.71 超出凡人盾 0.7 上限，应被拒绝");
    assert!(
        err.contains("block_ratio"),
        "错误消息应含 'block_ratio'，实际：{err}"
    );
}

/// block_ratio = 0.0 被拒绝（无效：不可为零）。
#[test]
fn shield_spec_validate_block_ratio_zero_rejected() {
    let spec = ShieldSpec {
        block_ratio: 0.0,
        durability_max: 40.0,
        stamina_drain_per_s: 3.0,
    };
    assert!(
        spec.validate("zero_shield").is_err(),
        "block_ratio=0.0 应被拒绝（无效：不能为 0）"
    );
}

/// block_ratio 负值被拒绝。
#[test]
fn shield_spec_validate_block_ratio_negative_rejected() {
    let spec = ShieldSpec {
        block_ratio: -0.1,
        durability_max: 40.0,
        stamina_drain_per_s: 3.0,
    };
    assert!(
        spec.validate("neg_shield").is_err(),
        "block_ratio 负值应被拒绝"
    );
}

/// block_ratio = NaN 被拒绝。
#[test]
fn shield_spec_validate_block_ratio_nan_rejected() {
    let spec = ShieldSpec {
        block_ratio: f64::NAN,
        durability_max: 40.0,
        stamina_drain_per_s: 3.0,
    };
    assert!(
        spec.validate("nan_shield").is_err(),
        "block_ratio=NaN 应被拒绝（is_finite 检查）"
    );
}

/// durability_max = 0.0 被拒绝。
#[test]
fn shield_spec_validate_durability_zero_rejected() {
    let spec = ShieldSpec {
        block_ratio: 0.5,
        durability_max: 0.0,
        stamina_drain_per_s: 3.0,
    };
    let err = spec
        .validate("zero_dur_shield")
        .expect_err("durability_max=0 应被拒绝");
    assert!(
        err.contains("durability_max"),
        "错误消息应含 'durability_max'，实际：{err}"
    );
}

/// stamina_drain_per_s = 0.0 被拒绝。
#[test]
fn shield_spec_validate_stamina_drain_zero_rejected() {
    let spec = ShieldSpec {
        block_ratio: 0.5,
        durability_max: 40.0,
        stamina_drain_per_s: 0.0,
    };
    let err = spec
        .validate("zero_drain_shield")
        .expect_err("stamina_drain_per_s=0 应被拒绝");
    assert!(
        err.contains("stamina_drain_per_s"),
        "错误消息应含 'stamina_drain_per_s'，实际：{err}"
    );
}

// plan-shield-block-v1 P2 §Issue5.3 — durability_max NaN/inf 独立用例
/// durability_max = NaN 被拒绝（is_finite 检查）。
#[test]
fn shield_spec_validate_durability_max_nan_rejected() {
    let spec = ShieldSpec {
        block_ratio: 0.5,
        durability_max: f64::NAN,
        stamina_drain_per_s: 3.0,
    };
    let err = spec
        .validate("nan_dur_shield")
        .expect_err("durability_max=NaN 应被拒绝（is_finite 检查）");
    assert!(
        err.contains("durability_max"),
        "错误消息应含 'durability_max'，实际：{err}"
    );
}

/// durability_max = +Inf 被拒绝（is_finite 检查）。
#[test]
fn shield_spec_validate_durability_max_inf_rejected() {
    let spec = ShieldSpec {
        block_ratio: 0.5,
        durability_max: f64::INFINITY,
        stamina_drain_per_s: 3.0,
    };
    let err = spec
        .validate("inf_dur_shield")
        .expect_err("durability_max=+Inf 应被拒绝（is_finite 检查）");
    assert!(
        err.contains("durability_max"),
        "错误消息应含 'durability_max'，实际：{err}"
    );
}

// plan-shield-block-v1 P2 §Issue5.3 — stamina_drain_per_s NaN/inf 独立用例
/// stamina_drain_per_s = NaN 被拒绝（is_finite 检查）。
#[test]
fn shield_spec_validate_stamina_drain_nan_rejected() {
    let spec = ShieldSpec {
        block_ratio: 0.5,
        durability_max: 40.0,
        stamina_drain_per_s: f32::NAN,
    };
    let err = spec
        .validate("nan_drain_shield")
        .expect_err("stamina_drain_per_s=NaN 应被拒绝（is_finite 检查）");
    assert!(
        err.contains("stamina_drain_per_s"),
        "错误消息应含 'stamina_drain_per_s'，实际：{err}"
    );
}

/// stamina_drain_per_s = +Inf 被拒绝（is_finite 检查）。
#[test]
fn shield_spec_validate_stamina_drain_inf_rejected() {
    let spec = ShieldSpec {
        block_ratio: 0.5,
        durability_max: 40.0,
        stamina_drain_per_s: f32::INFINITY,
    };
    let err = spec
        .validate("inf_drain_shield")
        .expect_err("stamina_drain_per_s=+Inf 应被拒绝（is_finite 检查）");
    assert!(
        err.contains("stamina_drain_per_s"),
        "错误消息应含 'stamina_drain_per_s'，实际：{err}"
    );
}

/// 从 TOML 加载的 wooden_shield 含正确 ShieldSpec（block_ratio=0.5, durability=40, drain=3.0）。
#[test]
fn wooden_shield_loads_correct_shield_spec_from_toml() {
    let registry = load_item_registry().expect("item registry 应从 assets/items/*.toml 加载");
    let tpl = registry
        .get("wooden_shield")
        .expect("wooden_shield 应存在于 registry");
    let spec = tpl
        .shield_spec
        .as_ref()
        .expect("wooden_shield 应有 shield_spec（P2 TOML 块必须存在）");
    assert!(
        (spec.block_ratio - 0.5).abs() < 1e-9,
        "wooden_shield.block_ratio 应为 0.5，实际 {}",
        spec.block_ratio
    );
    assert!(
        (spec.durability_max - 40.0).abs() < 1e-9,
        "wooden_shield.durability_max 应为 40.0，实际 {}",
        spec.durability_max
    );
    assert!(
        (spec.stamina_drain_per_s - 3.0).abs() < 1e-4,
        "wooden_shield.stamina_drain_per_s 应为 3.0，实际 {}",
        spec.stamina_drain_per_s
    );
}

/// 从 TOML 加载的 bone_shield 含正确 ShieldSpec（block_ratio=0.65, durability=80, drain=3.0）。
#[test]
fn bone_shield_loads_correct_shield_spec_from_toml() {
    let registry = load_item_registry().expect("item registry 应从 assets/items/*.toml 加载");
    let tpl = registry
        .get("bone_shield")
        .expect("bone_shield 应存在于 registry");
    let spec = tpl
        .shield_spec
        .as_ref()
        .expect("bone_shield 应有 shield_spec（P2 TOML 块必须存在）");
    assert!(
        (spec.block_ratio - 0.65).abs() < 1e-9,
        "bone_shield.block_ratio 应为 0.65，实际 {}",
        spec.block_ratio
    );
    assert!(
        (spec.durability_max - 80.0).abs() < 1e-9,
        "bone_shield.durability_max 应为 80.0，实际 {}",
        spec.durability_max
    );
    assert!(
        (spec.stamina_drain_per_s - 3.0).abs() < 1e-4,
        "bone_shield.stamina_drain_per_s 应为 3.0，实际 {}",
        spec.stamina_drain_per_s
    );
}

#[test]
fn placeable_container_templates_load_from_workbench_materials_toml() {
    let registry = load_item_registry().expect("item registry 应从 assets/items/*.toml 加载");
    for (id, placeable) in [
        ("trade_crate", "storage_crate"),
        ("herb_crate_placed", "storage_crate"),
        ("dead_drop_box", "dead_drop"),
    ] {
        let tpl = registry
            .get(id)
            .unwrap_or_else(|| panic!("{id} 应存在于 registry"));
        assert_eq!(tpl.category, ItemCategory::Misc, "{id} 应保持 misc 类别");
        assert_eq!(
            tpl.placeable.as_deref(),
            Some(placeable),
            "{id} 应声明正确 placeable 标记"
        );
    }
    let carried_herb = registry
        .get("herb_crate")
        .expect("随身版 herb_crate 应继续存在");
    assert_eq!(
        carried_herb.placeable, None,
        "随身版 herb_crate 不应被放置链路消费"
    );
}

#[test]
fn item_template_toml_normalizes_non_block_placeable_marker() {
    let mut raw = raw_item_template_toml("portable_trade_crate", "misc");
    raw.placeable = Some("  STORAGE_CRATE  ".to_string());

    let tpl = raw
        .try_into_item_template(std::path::Path::new("test_placeable.toml"))
        .expect("非 Block 模板应允许声明 placeable");

    assert_eq!(
        tpl.category,
        ItemCategory::Misc,
        "非 Block placeable 模板应保持自身物品分类"
    );
    assert_eq!(
        tpl.placeable.as_deref(),
        Some("storage_crate"),
        "placeable 标记应 trim 并归一化为小写"
    );
}

#[test]
fn item_template_toml_rejects_blank_placeable_marker() {
    let mut raw = raw_item_template_toml("blank_placeable_crate", "misc");
    raw.placeable = Some("   ".to_string());

    let error = raw
        .try_into_item_template(std::path::Path::new("test_placeable.toml"))
        .expect_err("空白 placeable 标记必须报错");

    assert!(
        error.contains("placeable"),
        "错误信息应指出 placeable 字段为空，实际 {error}"
    );
}

/// 非盾物品（如 iron_sword）的 shield_spec 为 None。
#[test]
fn non_shield_item_has_no_shield_spec() {
    let registry = load_item_registry().expect("item registry 应加载");
    let tpl = registry
        .get("iron_sword")
        .expect("iron_sword 应存在于 registry");
    assert!(
        tpl.shield_spec.is_none(),
        "iron_sword 不是盾，shield_spec 应为 None，实际有值"
    );
}

/// category=Shield 但缺 shield_spec 块 → try_into_item_template 报错。
#[test]
fn shield_category_without_shield_spec_block_is_rejected() {
    use std::path::PathBuf;
    let path = PathBuf::from("test_shield.toml");
    let raw = ItemTemplateToml {
        id: "bad_shield_no_spec".to_string(),
        placeable: None,
        name: "无规格盾".to_string(),
        category: "shield".to_string(),
        grid_w: 1,
        grid_h: 2,
        base_weight: 2.0,
        rarity: "common".to_string(),
        spirit_quality_initial: 0.0,
        description: "category=shield but missing shield_spec".to_string(),
        max_stack_count: None,
        effect: None,
        cast_duration_ms: None,
        cooldown_ms: None,
        weapon: None,
        forge_station: None,
        blueprint_scroll: None,
        inscription_scroll: None,
        technique_scroll: None,
        readable_scroll: None,
        recipe_fragment: None,
        container: None,
        shield_spec: None, // ← 故意缺失
        shelflife_profile: None,
        shelflife_track: None,
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    };
    let result = raw.try_into_item_template(&path);
    assert!(
        result.is_err(),
        "category=shield 但缺 shield_spec 块时应报错，防止孤岛装备"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("shield_spec") || err.contains("Shield"),
        "错误消息应提到 shield_spec 缺失，实际：{err}"
    );
}

/// 非盾 category 带 shield_spec 块 → try_into_item_template 报错。
#[test]
fn non_shield_category_with_shield_spec_block_is_rejected() {
    use std::path::PathBuf;
    let path = PathBuf::from("test_sword_with_shield_spec.toml");
    let raw = ItemTemplateToml {
        id: "bad_sword_with_shield_spec".to_string(),
        placeable: None,
        name: "剑+盾规格冲突".to_string(),
        category: "weapon".to_string(),
        grid_w: 1,
        grid_h: 2,
        base_weight: 1.0,
        rarity: "common".to_string(),
        spirit_quality_initial: 0.0,
        description: "weapon category with shield_spec should fail".to_string(),
        max_stack_count: None,
        effect: None,
        cast_duration_ms: None,
        cooldown_ms: None,
        weapon: None,
        forge_station: None,
        blueprint_scroll: None,
        inscription_scroll: None,
        technique_scroll: None,
        readable_scroll: None,
        recipe_fragment: None,
        container: None,
        shield_spec: Some(ShieldSpecToml {
            block_ratio: 0.5,
            durability_max: 40.0,
            stamina_drain_per_s: 3.0,
        }),
        shelflife_profile: None,
        shelflife_track: None,
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    };
    let result = raw.try_into_item_template(&path);
    assert!(result.is_err(), "非盾 category 带 shield_spec 块时应报错");
}

// ─── plan-worldgen-v4 P5 §8.1#5 — vanilla 模板注入专属矩阵 ───

/// happy path：注入把全部非 air vanilla BlockKind 注册为 `vanilla:<id>` 模板，
/// 数量 = BlockKind::ALL 中非 air 的个数，且空 map 注入后 air 不在结果里。
#[test]
fn inject_vanilla_block_templates_covers_all_non_air_kinds() {
    use valence::prelude::BlockKind;

    let expected = BlockKind::ALL
        .iter()
        .filter(|k| k.to_str() != "air")
        .count();

    let mut templates = HashMap::new();
    let injected = inject_vanilla_block_templates(&mut templates)
        .expect("空 registry 注入 vanilla 模板应成功");

    assert_eq!(
        injected, expected,
        "注入数量应等于非 air vanilla BlockKind 数（{expected}），实为 {injected}"
    );
    assert_eq!(
        templates.len(),
        expected,
        "templates map 大小应等于注入数（无重复），实为 {}",
        templates.len()
    );
    // air 跳过：既无 `vanilla:air`，也没把 air 当成可给予物品。
    assert!(
        !templates.contains_key("vanilla:air"),
        "air 必须被跳过，不得注册 vanilla:air 模板"
    );
    // 抽样确认常见块在内。
    assert!(
        templates.contains_key("vanilla:stone"),
        "vanilla:stone 应被注入"
    );
    assert!(
        templates.contains_key("vanilla:stone_bricks"),
        "vanilla:stone_bricks 应被注入"
    );
}

/// 字段契约：注入的 `vanilla:<id>` 模板形态固定（id/category/max_stack/placeable），
/// 与 block_place vanilla: 直通分支与 ItemCategory::Block 默认堆叠上限对齐。
#[test]
fn vanilla_block_template_field_contract() {
    let template = vanilla_block_template("stone_bricks");
    assert_eq!(
        template.id, "vanilla:stone_bricks",
        "id 必须是 vanilla:<bare>，实为 {}",
        template.id
    );
    assert_eq!(
        template.category,
        ItemCategory::Block,
        "vanilla 方块模板 category 必须为 Block"
    );
    assert_eq!(
        template.max_stack_count,
        default_max_stack_count_for_category(ItemCategory::Block),
        "max_stack_count 必须取 Block 默认堆叠上限（64）"
    );
    assert!(
        template.placeable.is_none(),
        "placeable 必须为 None——放置走 block_place 的 vanilla: 直通分支，不经 PlaceableBlockKind"
    );
}

/// 错误分支：注入若与已存在的同名 key（手写 TOML 或重复注入）撞车，必须返回 Err，
/// 保护手写映射不被静默覆盖。
#[test]
fn inject_vanilla_block_templates_errors_on_key_collision() {
    let mut templates = HashMap::new();
    // 预置一个会与 vanilla:stone 撞 key 的手写模板。
    templates.insert("vanilla:stone".to_string(), vanilla_block_template("stone"));

    let err = inject_vanilla_block_templates(&mut templates)
        .expect_err("撞 key 必须返回 Err，不得静默覆盖手写映射");
    assert!(
        err.contains("vanilla:stone") && err.contains("collides"),
        "撞 key 错误信息应指明冲突的 vanilla:stone，实为: {err}"
    );
}

/// 集成：生产 load_item_registry() 末尾确实注入了 vanilla 模板，
/// 锁住「give-block 链路依赖的 vanilla:<id> 在真 registry 里可查」。
#[test]
fn load_item_registry_includes_injected_vanilla_templates() {
    let registry = load_item_registry().expect("真 registry 加载应含 vanilla 模板");
    assert!(
        registry.get("vanilla:stone_bricks").is_some(),
        "真 ItemRegistry 必须含 vanilla:stone_bricks（give-block 链路依赖）"
    );
    assert!(
        registry.get("vanilla:air").is_none(),
        "真 ItemRegistry 不得含 vanilla:air"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// plan-race-system-v1 P4 — enforce_intrinsic_gate_on_morph_release 饱和测试。
// 决议 §6：解除易形后本体身份恢复权威，非法装备"卸包→满则掉落"绝不静默销毁。
// ─────────────────────────────────────────────────────────────────────────
mod morph_release_equip_gate {
    use super::*;
    use crate::body_plan::types::{RaceGateOwned, RaceId};
    use crate::world::dimension::DimensionKind;

    fn item_template(id: &str, wearer_race: RaceGateOwned) -> ItemTemplate {
        let mut template = test_template(id, ItemCategory::Armor, 1, 1, 1);
        template.wearer_race = wearer_race;
        template
    }

    fn registry_with(templates: Vec<ItemTemplate>) -> ItemRegistry {
        let mut map = HashMap::new();
        for t in templates {
            map.insert(t.id.clone(), t);
        }
        ItemRegistry::from_map(map)
    }

    fn worn_item_instance(instance_id: u64, template: &ItemTemplate) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template.id.clone(),
            display_name: template.display_name.clone(),
            grid_w: template.grid_w,
            grid_h: template.grid_h,
            weight: template.base_weight,
            rarity: template.rarity,
            description: template.description.clone(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn inventory_with_worn(
        item: ItemInstance,
        container_capacity: Option<(u8, u8)>,
    ) -> PlayerInventory {
        let mut equipped = HashMap::new();
        equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents {
                worn: vec![item],
                held: None,
            },
        );
        // 注：`rebuild_containers_from_equipment` 会在 body_pocket 缺席时自动补一个
        // 满容量的 body_pocket 兜底槽——若测试想验证"背包满→掉落"，必须显式提供一个
        // 与 main_pack 同容量的 body_pocket（否则摘下的装备会静默落进自动补出的宽敞
        // 暗袋，掉落分支永不触发）。这里让两个容器同容量：非满时 main_pack 优先收纳
        // （find_first_fit 先扫非 body_pocket），全满(0,0)时两者皆无位→掉落。
        let containers = match container_capacity {
            Some((rows, cols)) => vec![
                ContainerState {
                    id: MAIN_PACK_CONTAINER_ID.to_string(),
                    name: "主背包".to_string(),
                    rows,
                    cols,
                    items: Vec::new(),
                    owner_instance_id: None,
                    quick_access: false,
                },
                ContainerState {
                    id: BODY_POCKET_CONTAINER_ID.to_string(),
                    name: "暗袋".to_string(),
                    rows,
                    cols,
                    items: Vec::new(),
                    owner_instance_id: None,
                    quick_access: false,
                },
            ],
            None => Vec::new(),
        };
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers,
            equipped,
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 99.0,
        }
    }

    /// 满足档：本体身份放行该装备——原地不动，无 stash、无 drop。
    #[test]
    fn compatible_item_is_left_worn_untouched() {
        let template = item_template("chest_any", RaceGateOwned::Any);
        let registry = registry_with(vec![template.clone()]);
        let item = worn_item_instance(1, &template);
        let mut inventory = inventory_with_worn(item, Some((4, 4)));
        let mut dropped = DroppedLootRegistry::default();

        let (stashed, dropped_ids) = enforce_intrinsic_gate_on_morph_release(
            &mut inventory,
            &registry,
            &mut dropped,
            &RaceId::new("human"),
            true,
            [0.0, 64.0, 0.0],
            DimensionKind::Overworld,
        );

        assert!(stashed.is_empty(), "满足档不应被移动进背包");
        assert!(dropped_ids.is_empty(), "满足档不应被掉落");
        assert_eq!(
            inventory
                .equipped
                .get(EQUIP_SLOT_CHEST)
                .map(|c| c.worn.len()),
            Some(1),
            "满足档装备必须原样留在 worn 层"
        );
        assert!(dropped.entries.is_empty());
    }

    /// 不满足档 + 背包有空位——移出装备槽，塞进背包（不掉落）。
    #[test]
    fn incompatible_item_is_stashed_into_backpack_when_room_available() {
        let template = item_template(
            "chest_whale_only",
            RaceGateOwned::Species {
                species: vec![RaceId::new("whale")],
            },
        );
        let registry = registry_with(vec![template.clone()]);
        let item = worn_item_instance(2, &template);
        let mut inventory = inventory_with_worn(item, Some((4, 4)));
        let mut dropped = DroppedLootRegistry::default();

        let (stashed, dropped_ids) = enforce_intrinsic_gate_on_morph_release(
            &mut inventory,
            &registry,
            &mut dropped,
            &RaceId::new("human"),
            true,
            [0.0, 64.0, 0.0],
            DimensionKind::Overworld,
        );

        assert_eq!(
            stashed,
            vec![2],
            "human 本体不满足 Species([whale]) 门，必须被摘下"
        );
        assert!(dropped_ids.is_empty(), "背包有空位时不应掉落");
        assert!(
            inventory
                .equipped
                .get(EQUIP_SLOT_CHEST)
                .map(|c| c.worn.is_empty())
                .unwrap_or(true),
            "不满足档装备必须从 worn 层移除"
        );
        let found_in_container = inventory
            .containers
            .iter()
            .flat_map(|c| c.items.iter())
            .any(|placed| placed.instance.instance_id == 2);
        assert!(found_in_container, "摘下的装备必须落进背包容器");
        assert!(dropped.entries.is_empty());
    }

    /// 不满足档 + 背包已满（容量 0）——摘下后无处安放，必须转地面掉落，绝不静默销毁。
    #[test]
    fn incompatible_item_is_dropped_to_ground_when_backpack_full() {
        let template = item_template(
            "chest_whale_only_2",
            RaceGateOwned::Species {
                species: vec![RaceId::new("whale")],
            },
        );
        let registry = registry_with(vec![template.clone()]);
        let item = worn_item_instance(3, &template);
        // 容量 (0, 0) 的主背包 —— 任何格位都放不下。
        let mut inventory = inventory_with_worn(item, Some((0, 0)));
        let mut dropped = DroppedLootRegistry::default();

        let (stashed, dropped_ids) = enforce_intrinsic_gate_on_morph_release(
            &mut inventory,
            &registry,
            &mut dropped,
            &RaceId::new("human"),
            true,
            [10.0, 64.0, 10.0],
            DimensionKind::Overworld,
        );

        assert!(stashed.is_empty(), "背包满时不应算作已收纳");
        assert_eq!(dropped_ids, vec![3], "背包满时必须转地面掉落，不能凭空消失");
        assert!(
            dropped.entries.contains_key(&3),
            "DroppedLootRegistry 必须登记该 instance_id，禁止静默丢件"
        );
        assert!(inventory
            .equipped
            .get(EQUIP_SLOT_CHEST)
            .map(|c| c.worn.is_empty())
            .unwrap_or(true));
    }

    /// 无任何非法装备 —— 提前返回空结果，不触发任何容器重建副作用。
    #[test]
    fn no_incompatible_items_returns_empty_without_touching_inventory() {
        let template = item_template("chest_any_2", RaceGateOwned::Any);
        let registry = registry_with(vec![template.clone()]);
        let item = worn_item_instance(4, &template);
        let mut inventory = inventory_with_worn(item, None);
        let mut dropped = DroppedLootRegistry::default();

        let (stashed, dropped_ids) = enforce_intrinsic_gate_on_morph_release(
            &mut inventory,
            &registry,
            &mut dropped,
            &RaceId::new("human"),
            true,
            [0.0, 0.0, 0.0],
            DimensionKind::Overworld,
        );
        assert!(stashed.is_empty());
        assert!(dropped_ids.is_empty());
    }

    /// Humanoid 档：本体 is_humanoid=false（如未来非人形玩家）必须驱逐 Humanoid-only
    /// 装备——覆盖 RaceGateOwned::Humanoid 分支，不只测 Species/Any。
    #[test]
    fn humanoid_only_item_evicted_when_intrinsic_is_not_humanoid() {
        let template = item_template("chest_humanoid_only", RaceGateOwned::Humanoid);
        let registry = registry_with(vec![template.clone()]);
        let item = worn_item_instance(5, &template);
        let mut inventory = inventory_with_worn(item, Some((4, 4)));
        let mut dropped = DroppedLootRegistry::default();

        let (stashed, _dropped_ids) = enforce_intrinsic_gate_on_morph_release(
            &mut inventory,
            &registry,
            &mut dropped,
            &RaceId::new("whale"),
            false,
            [0.0, 64.0, 0.0],
            DimensionKind::Overworld,
        );
        assert_eq!(stashed, vec![5]);
    }

    /// plan-race-system-v1 P4 opus verifier MINOR —— 此前 5 条 case 只测 worn 层，
    /// held 槽（如主手武器）驱逐分支零覆盖。构造 `held` 位放一件本体不满足
    /// Species 门的武器，验证 `contents.held.take()` 分支真被驱逐+摘出。
    fn inventory_with_held(
        item: ItemInstance,
        container_capacity: Option<(u8, u8)>,
    ) -> PlayerInventory {
        let mut equipped = HashMap::new();
        equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents {
                worn: Vec::new(),
                held: Some(item),
            },
        );
        let containers = match container_capacity {
            Some((rows, cols)) => vec![
                ContainerState {
                    id: MAIN_PACK_CONTAINER_ID.to_string(),
                    name: "主背包".to_string(),
                    rows,
                    cols,
                    items: Vec::new(),
                    owner_instance_id: None,
                    quick_access: false,
                },
                ContainerState {
                    id: BODY_POCKET_CONTAINER_ID.to_string(),
                    name: "暗袋".to_string(),
                    rows,
                    cols,
                    items: Vec::new(),
                    owner_instance_id: None,
                    quick_access: false,
                },
            ],
            None => Vec::new(),
        };
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers,
            equipped,
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 99.0,
        }
    }

    #[test]
    fn incompatible_held_weapon_is_evicted_and_stashed_into_backpack() {
        let template = item_template(
            "sword_whale_only_held",
            RaceGateOwned::Species {
                species: vec![RaceId::new("whale")],
            },
        );
        let registry = registry_with(vec![template.clone()]);
        let item = worn_item_instance(6, &template);
        let mut inventory = inventory_with_held(item, Some((4, 4)));
        let mut dropped = DroppedLootRegistry::default();

        let (stashed, dropped_ids) = enforce_intrinsic_gate_on_morph_release(
            &mut inventory,
            &registry,
            &mut dropped,
            &RaceId::new("human"),
            true,
            [0.0, 64.0, 0.0],
            DimensionKind::Overworld,
        );

        assert_eq!(
            stashed,
            vec![6],
            "human 本体不满足 Species([whale]) 门，held 槽的武器必须被摘下"
        );
        assert!(dropped_ids.is_empty(), "背包有空位时不应掉落");
        assert!(
            inventory
                .equipped
                .get(EQUIP_SLOT_MAIN_HAND)
                .map(|c| c.held.is_none())
                .unwrap_or(true),
            "不满足档的武器必须从 held 位移除"
        );
        let found_in_container = inventory
            .containers
            .iter()
            .flat_map(|c| c.items.iter())
            .any(|placed| placed.instance.instance_id == 6);
        assert!(
            found_in_container,
            "摘下的武器必须落进背包容器，不能凭空消失"
        );
        assert!(dropped.entries.is_empty());
    }

    #[test]
    fn incompatible_held_weapon_drops_to_ground_when_backpack_full() {
        let template = item_template(
            "sword_whale_only_held_2",
            RaceGateOwned::Species {
                species: vec![RaceId::new("whale")],
            },
        );
        let registry = registry_with(vec![template.clone()]);
        let item = worn_item_instance(7, &template);
        // 容量 (0, 0) 的主背包 —— 任何格位都放不下。
        let mut inventory = inventory_with_held(item, Some((0, 0)));
        let mut dropped = DroppedLootRegistry::default();

        let (stashed, dropped_ids) = enforce_intrinsic_gate_on_morph_release(
            &mut inventory,
            &registry,
            &mut dropped,
            &RaceId::new("human"),
            true,
            [10.0, 64.0, 10.0],
            DimensionKind::Overworld,
        );

        assert!(stashed.is_empty(), "背包满时不应算作已收纳");
        assert_eq!(
            dropped_ids,
            vec![7],
            "背包满时 held 槽武器必须转地面掉落，不能凭空消失"
        );
        assert!(
            dropped.entries.contains_key(&7),
            "DroppedLootRegistry 必须登记该 instance_id，禁止静默丢件"
        );
        assert!(inventory
            .equipped
            .get(EQUIP_SLOT_MAIN_HAND)
            .map(|c| c.held.is_none())
            .unwrap_or(true));
    }

    /// held + worn 同槽同时各有一件不满足档——两者都必须被摘出（held 优先摘，
    /// 再摘 worn 栈，与函数文档"held 优先摘，再摘 worn 栈"一致）。
    #[test]
    fn both_held_and_worn_incompatible_items_are_evicted_together() {
        let held_template = item_template(
            "sword_whale_only_held_3",
            RaceGateOwned::Species {
                species: vec![RaceId::new("whale")],
            },
        );
        let worn_template = item_template(
            "chest_whale_only_3",
            RaceGateOwned::Species {
                species: vec![RaceId::new("whale")],
            },
        );
        let registry = registry_with(vec![held_template.clone(), worn_template.clone()]);
        let held_item = worn_item_instance(8, &held_template);
        let worn_item = worn_item_instance(9, &worn_template);

        let mut equipped = HashMap::new();
        equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents {
                worn: vec![worn_item],
                held: Some(held_item),
            },
        );
        let mut inventory = PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![
                ContainerState {
                    id: MAIN_PACK_CONTAINER_ID.to_string(),
                    name: "主背包".to_string(),
                    rows: 4,
                    cols: 4,
                    items: Vec::new(),
                    owner_instance_id: None,
                    quick_access: false,
                },
                ContainerState {
                    id: BODY_POCKET_CONTAINER_ID.to_string(),
                    name: "暗袋".to_string(),
                    rows: 4,
                    cols: 4,
                    items: Vec::new(),
                    owner_instance_id: None,
                    quick_access: false,
                },
            ],
            equipped,
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 99.0,
        };
        let mut dropped = DroppedLootRegistry::default();

        let (mut stashed, dropped_ids) = enforce_intrinsic_gate_on_morph_release(
            &mut inventory,
            &registry,
            &mut dropped,
            &RaceId::new("human"),
            true,
            [0.0, 64.0, 0.0],
            DimensionKind::Overworld,
        );
        stashed.sort_unstable();

        assert_eq!(
            stashed,
            vec![8, 9],
            "held 位与 worn 栈里各一件不满足档的物品都必须被摘出，一个不落"
        );
        assert!(dropped_ids.is_empty());
        let contents = inventory.equipped.get(EQUIP_SLOT_MAIN_HAND).unwrap();
        assert!(contents.held.is_none(), "held 应清空");
        assert!(contents.worn.is_empty(), "worn 栈应清空");
    }
}

// ─────────────────────────────────────────────────────────────────────────
// plan-layered-equip-v1 P0 — 决议锁定行为 pin 测试（SlotContents / worn_cap /
// classify_equip_state / weapon_two_handed / pack 容器 id 反查）。
// 这些把 PR-1 重构的核心契约钉死，任何回归立刻撞红。
// ─────────────────────────────────────────────────────────────────────────
mod layered_equip_p0_pins {
    use super::*;

    // ── 1. SlotContents serde roundtrip（空 / 单 worn / 多 worn / worn+held / held-only）──

    fn roundtrip(slot: &SlotContents) -> SlotContents {
        let json = serde_json::to_string(slot).expect("SlotContents should serialize");
        serde_json::from_str(&json).expect("SlotContents should deserialize")
    }

    #[test]
    fn slot_contents_serde_roundtrip_empty() {
        let empty = SlotContents::default();
        let json = serde_json::to_string(&empty).expect("empty SlotContents should serialize");
        // 空槽：worn 序列化为 []，held=None 时省略字段（skip_serializing_if）。
        assert!(
            json.contains("\"worn\":[]"),
            "空槽应把 worn 序列化为 []，实际：{json}"
        );
        assert!(
            !json.contains("held"),
            "held=None 应被省略（skip_serializing_if），实际：{json}"
        );
        assert_eq!(roundtrip(&empty), empty, "空槽 roundtrip 应保持相等");
    }

    #[test]
    fn slot_contents_serde_roundtrip_single_worn() {
        let s = SlotContents::worn_single(make_test_item_instance(1, "armor_a"));
        assert_eq!(roundtrip(&s), s, "单 worn 件 roundtrip 应保持相等");
        assert_eq!(s.worn.len(), 1);
        assert!(s.held.is_none());
    }

    #[test]
    fn slot_contents_serde_roundtrip_multi_worn() {
        let s = SlotContents {
            worn: vec![
                make_test_item_instance(1, "layer_bottom"),
                make_test_item_instance(2, "layer_mid"),
                make_test_item_instance(3, "layer_top"),
            ],
            held: None,
        };
        let back = roundtrip(&s);
        assert_eq!(back, s, "三层 worn roundtrip 应保持相等（含栈顺序）");
        // 栈顺序：worn.last() = 栈顶。
        assert_eq!(
            back.worn_top().unwrap().instance_id,
            3,
            "worn_top 应为最后压入的件（栈顶 = Vec 末尾）"
        );
    }

    #[test]
    fn slot_contents_serde_roundtrip_worn_plus_held() {
        let s = SlotContents {
            worn: vec![make_test_item_instance(1, "armor_a")],
            held: Some(make_test_item_instance(2, "sword_a")),
        };
        assert_eq!(roundtrip(&s), s, "worn+held roundtrip 应保持相等");
    }

    #[test]
    fn slot_contents_serde_roundtrip_held_only() {
        let s = SlotContents::held_single(make_test_item_instance(9, "sword_a"));
        let json = serde_json::to_string(&s).expect("held-only should serialize");
        assert!(
            json.contains("\"held\""),
            "held=Some 应序列化 held 字段：{json}"
        );
        assert_eq!(roundtrip(&s), s, "held-only roundtrip 应保持相等");
    }

    // ── 2. worn_cap 边界（决议 #6/#14/#17）──

    #[test]
    fn worn_cap_boundaries_per_slot() {
        assert_eq!(worn_cap(EQUIP_SLOT_HEAD), 2, "head worn cap=2");
        assert_eq!(worn_cap(EQUIP_SLOT_FEET), 2, "feet worn cap=2");
        assert_eq!(worn_cap(EQUIP_SLOT_CHEST), 3, "chest worn cap=3");
        assert_eq!(worn_cap(EQUIP_SLOT_LEGS), 3, "legs worn cap=3");
        assert_eq!(
            worn_cap(EQUIP_SLOT_MAIN_HAND),
            0,
            "main_hand held-only cap=0"
        );
        assert_eq!(worn_cap(EQUIP_SLOT_OFF_HAND), 0, "off_hand held-only cap=0");
        assert_eq!(
            worn_cap(EQUIP_SLOT_EXTRA_HAND_0),
            0,
            "extra_hand_0 held-only cap=0"
        );
        assert_eq!(
            worn_cap(EQUIP_SLOT_EXTRA_HAND_1),
            0,
            "extra_hand_1 held-only cap=0"
        );
    }

    // ── P5 pin — worn_cap_bonus 默认 0（扩展点未接升级源，行为不变）──

    #[test]
    fn p5_worn_cap_bonus_defaults_to_zero_so_effective_cap_equals_base() {
        // P5 hook：升级源未接时 bonus=0，有效 cap = base。
        // 当升级源接入时本测试需同步更新（预期值不再恒等 base）。
        for slot in &[
            EQUIP_SLOT_HEAD,
            EQUIP_SLOT_CHEST,
            EQUIP_SLOT_LEGS,
            EQUIP_SLOT_FEET,
            EQUIP_SLOT_MAIN_HAND,
            EQUIP_SLOT_OFF_HAND,
            EQUIP_SLOT_EXTRA_HAND_0,
            EQUIP_SLOT_EXTRA_HAND_1,
        ] {
            let base = worn_cap(slot);
            let bonus = worn_cap_bonus(slot);
            assert_eq!(
                bonus, 0,
                "worn_cap_bonus({slot}) 应为 0（P5 占位，升级源未接）；\
                 接入升级源后请删除此 assert_eq!(bonus,0) 并改为具体边界断言"
            );
            assert_eq!(
                base.saturating_add(bonus),
                base,
                "effective worn_cap({slot}) = base={base}+bonus=0，应等于 base（行为不变）"
            );
        }
    }

    #[test]
    fn p5_treasure_trigger_cap_fn_equals_constant() {
        // P5 hook：treasure_trigger_cap() 当前恒等于 TREASURE_TRIGGER_CAP 常量。
        // 接入升级源后，该函数可返回比常量更大的值；届时删除此相等断言并改边界断言。
        assert_eq!(
            treasure_trigger_cap(),
            TREASURE_TRIGGER_CAP,
            "treasure_trigger_cap() 应等于常量 TREASURE_TRIGGER_CAP={TREASURE_TRIGGER_CAP}（P5 占位，升级源未接）"
        );
    }

    // ── P5 边界 pin — worn_cap_bonus 空串 / 完全未知槽位（CR 补充）──

    #[test]
    fn p5_worn_cap_bonus_empty_slot_returns_zero() {
        // P5 占位：空字符串不是任何规范槽位，bonus 恒 0。
        // 断言信息：P5 占位——升级源未接前任意槽位 bonus 恒 0，空串亦不例外。
        assert_eq!(
            worn_cap_bonus(""),
            0,
            "P5 占位：worn_cap_bonus(\"\") 应为 0（升级源未接，任意非规范输入 bonus 恒 0）"
        );
    }

    #[test]
    fn p5_worn_cap_bonus_unknown_slot_returns_zero() {
        // P5 占位：完全陌生的槽位名不是任何规范槽位，bonus 恒 0。
        // 断言信息：P5 占位——升级源未接前任意槽位 bonus 恒 0，未知槽位亦不例外。
        assert_eq!(
            worn_cap_bonus("totally_unknown_slot"),
            0,
            "P5 占位：worn_cap_bonus(\"totally_unknown_slot\") 应为 0（升级源未接，任意非规范输入 bonus 恒 0）"
        );
    }

    // ── worn_cap 非规范输入行为 pin（CR 补充）──

    #[test]
    fn worn_cap_noncanonical_inputs_default_to_zero() {
        // worn_cap 对空串和未知槽位走 `_ => 0` 默认分支，行为是 held-only 语义（cap=0）。
        // 锁定此占位行为：任何非规范输入恒 0，防回归改变 wildcard 分支语义。
        assert_eq!(
            worn_cap(""),
            0,
            "worn_cap(\"\") 应为 0：非规范输入走 _ => 0 默认分支（held-only 语义）"
        );
        assert_eq!(
            worn_cap("unknown"),
            0,
            "worn_cap(\"unknown\") 应为 0：非规范输入走 _ => 0 默认分支（held-only 语义）"
        );
    }

    // ── 3. classify_equip_state（决议 #16）：Weapon|Tool→Held，Armor|Container→Worn ──

    fn make_tool_template(id: &str) -> ItemTemplate {
        let mut t = make_misc_template(id);
        t.category = ItemCategory::Tool;
        t
    }

    fn make_armor_template(id: &str) -> ItemTemplate {
        let mut t = make_misc_template(id);
        t.category = ItemCategory::Armor;
        t
    }

    #[test]
    fn classify_equip_state_buckets() {
        let registry = ItemRegistry::from_map(HashMap::from([
            ("weapon_a".to_string(), make_weapon_template("weapon_a")),
            ("tool_a".to_string(), make_tool_template("tool_a")),
            ("armor_a".to_string(), make_armor_template("armor_a")),
            (
                "container_a".to_string(),
                make_container_template("container_a", EQUIP_SLOT_CHEST, 2, 2, 5.0),
            ),
        ]));

        assert_eq!(
            classify_equip_state(&make_test_item_instance(1, "weapon_a"), &registry),
            EquipState::Held,
            "Weapon 应分类为 Held"
        );
        assert_eq!(
            classify_equip_state(&make_test_item_instance(2, "tool_a"), &registry),
            EquipState::Held,
            "Tool 应分类为 Held"
        );
        assert_eq!(
            classify_equip_state(&make_test_item_instance(3, "armor_a"), &registry),
            EquipState::Worn,
            "Armor 应分类为 Worn"
        );
        assert_eq!(
            classify_equip_state(&make_test_item_instance(4, "container_a"), &registry),
            EquipState::Worn,
            "Container（背包）应分类为 Worn"
        );
    }

    // ── 4. weapon_two_handed（决议 #7）：Spear/Staff→true，其余→false ──

    #[test]
    fn weapon_two_handed_per_kind() {
        use crate::combat::weapon::WeaponKind;
        assert!(weapon_two_handed(WeaponKind::Spear), "Spear 应为双手");
        assert!(weapon_two_handed(WeaponKind::Staff), "Staff 应为双手");
        assert!(!weapon_two_handed(WeaponKind::Sword), "Sword 应为单手");
        assert!(!weapon_two_handed(WeaponKind::Dagger), "Dagger 应为单手");
        assert!(!weapon_two_handed(WeaponKind::Fist), "Fist 应为单手");
    }

    // ── 6. container_id_for_worn_pack / worn_pack_instance_from_container_id 反查 roundtrip ──

    #[test]
    fn worn_pack_container_id_roundtrip() {
        let id = container_id_for_worn_pack(42);
        assert_eq!(id, "pack_42", "容器 id 应为 pack_<instance_id>");
        assert_eq!(
            worn_pack_instance_from_container_id(&id),
            Some(42),
            "pack_42 应反解回 instance_id=42"
        );
        assert_eq!(
            worn_pack_instance_from_container_id("body_pocket"),
            None,
            "body_pocket 非 pack_ 前缀，应返回 None"
        );
        assert_eq!(
            worn_pack_instance_from_container_id("main_pack"),
            None,
            "main_pack 非 pack_ 前缀，应返回 None"
        );
        assert_eq!(
            worn_pack_instance_from_container_id("pack_notanumber"),
            None,
            "pack_ 后非数字应返回 None"
        );
    }
}
