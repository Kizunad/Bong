//! plan-workbench-recipes-v1 的数据资产回归测试。
//!
//! 生产配方表已迁至 `assets/craft/recipes/workbench/*.toml`；本文件仅保留既有
//! 运行时契约测试，统一通过 test-only data-backed registrar 读取单一数据真源。

#[cfg(test)]
mod tests {
    use crate::craft::{
        register_workbench_recipes, CraftCategory, CraftRecipe, CraftRegistry, CraftRequirements,
        CraftStationKind, RecipeId, RegistryError, UnlockSource,
    };

    const WORKBENCH: Option<CraftStationKind> = Some(CraftStationKind::Workbench);
    const HANDCRAFT_STONE_TOOLS: [&str; 3] = [
        "workbench.tool.stone_pickaxe",
        "workbench.tool.stone_axe",
        "workbench.weapon.stone_knife",
    ];

    fn p0_baseline_recipes() -> Vec<serde_json::Value> {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "fixtures/registry_datafication_p0_baseline.json"
        ))
        .expect("P0 baseline fixture must stay valid JSON");
        fixture["recipes"]
            .as_array()
            .expect("P0 baseline fixture must contain a recipes array")
            .clone()
    }

    fn p0_workbench_asset_recipe_count() -> usize {
        p0_baseline_recipes()
            .iter()
            .filter(|recipe| {
                !recipe["id"]
                    .as_str()
                    .expect("baseline recipe id must be a string")
                    .starts_with("craft.example.")
            })
            .count()
    }

    fn p0_baseline_count_matching(predicate: impl Fn(&serde_json::Value) -> bool) -> usize {
        p0_baseline_recipes()
            .iter()
            .filter(|recipe| predicate(recipe))
            .count()
    }

    #[test]
    fn register_workbench_recipes_succeeds() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        let expected_count = p0_workbench_asset_recipe_count();
        assert_eq!(
            registry.len(),
            expected_count,
            "workbench asset recipe count must derive from the old-registrar canonical fixture; got {}",
            registry.len()
        );
    }

    // ── plan-gathering-tool-bind-v1 P0 — herb_bundle 配方唯一性回归（旧红旗 R1）──

    #[test]
    fn herb_bundle_recipe_registered_exactly_once() {
        // plan-gathering-tool-bind-v1 §8.1 决议 #1：`workbench.process.herb_bundle` 只有
        // register_processing 里的一条真实定义，另一处（spot_checks 表）只是校验用例，
        // 不是第二个注册点。pin 住"注册表里只有一条"，防止有人在别处抄一份重复定义。
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        let matches: Vec<_> = registry
            .iter()
            .filter(|r| r.id.as_str() == "workbench.process.herb_bundle")
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "workbench.process.herb_bundle 应恰好命中 1 条注册记录，实际 {}（旧红旗 R1：\
             若此处 >1，说明存在重复注册点，HashMap 语义会把重复吞成假阳性 len()==1）",
            matches.len()
        );
    }

    #[test]
    fn re_registering_herb_bundle_recipe_id_is_rejected_as_duplicate() {
        // 期望：CraftRegistry::register 对 `workbench.process.herb_bundle` 重复 id 返回
        // DuplicateId 而不是静默覆盖 —— 这是 R1 真正的防线：即使未来有人手滑在别处又写一份
        // 同 id 配方，注册期就会硬报错而不是被吃掉、也不会 panic。
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        let duplicate = CraftRecipe {
            id: RecipeId::new("workbench.process.herb_bundle"),
            category: CraftCategory::Misc,
            display_name: "重复灵草束".into(),
            materials: vec![("spirit_grass".into(), 5)],
            qi_cost: 0.0,
            time_ticks: 10 * 20,
            output: ("herb_bundle".into(), 1),
            requirements: CraftRequirements::default(),
            unlock_sources: vec![],
            station: WORKBENCH,
        };
        let err = registry
            .register(duplicate)
            .expect_err("重复 id 必须被拒绝，因为 register_workbench_recipes 已注册过一次");
        assert!(
            matches!(&err, RegistryError::DuplicateId(id) if id.as_str() == "workbench.process.herb_bundle"),
            "期望 DuplicateId(\"workbench.process.herb_bundle\")，实际 {err:?}"
        );
    }

    #[test]
    fn workbench_self_recipe_is_handcraft() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        let r = registry
            .get(&RecipeId::new("craft.tool.workbench"))
            .expect("workbench self recipe must exist");
        assert_eq!(
            r.station, None,
            "workbench self recipe must be handcraft (station=None)"
        );
        assert_eq!(r.qi_cost, 0.0);
        assert_eq!(r.time_ticks, 60 * 20);
        assert_eq!(r.output, ("workbench_item".into(), 1));
    }

    #[test]
    fn all_workbench_recipes_have_station_workbench() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        for recipe in registry.iter() {
            let rid = recipe.id.as_str();
            // self recipe + 石器工具(石镐/石斧/石刃)是手搓配方(station None)，其余制作台专属。
            if rid == "craft.tool.workbench" || HANDCRAFT_STONE_TOOLS.contains(&rid) {
                assert_eq!(
                    recipe.station, None,
                    "手搓配方 `{}` station 必须 None",
                    recipe.id
                );
                continue;
            }
            assert_eq!(
                recipe.station,
                Some(CraftStationKind::Workbench),
                "recipe `{}` must have station=Workbench",
                recipe.id
            );
        }
    }

    #[test]
    fn all_workbench_recipe_ids_start_with_workbench_or_coffin_prefix() {
        // plan-coffin-tiers-v1 P4: 三档延寿棺配方迁入 craft 数据资产，
        // 保留 `coffin.` 命名空间与 coffin 模块的 recipe_id 辅助函数对齐。
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        for recipe in registry.iter() {
            if recipe.id.as_str() == "craft.tool.workbench" {
                continue; // self recipe uses craft. prefix
            }
            assert!(
                recipe.id.as_str().starts_with("workbench.")
                    || recipe.id.as_str().starts_with("coffin."),
                "recipe `{}` must start with 'workbench.' or 'coffin.' prefix",
                recipe.id
            );
        }
    }

    #[test]
    fn all_workbench_recipes_have_positive_time() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        for recipe in registry.iter() {
            assert!(
                recipe.time_ticks > 0,
                "recipe `{}` must have time_ticks > 0, got {}",
                recipe.id,
                recipe.time_ticks
            );
        }
    }

    #[test]
    fn all_workbench_recipes_have_non_negative_qi_cost() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        for recipe in registry.iter() {
            assert!(
                recipe.qi_cost >= 0.0 && recipe.qi_cost.is_finite(),
                "recipe `{}` must have finite non-negative qi_cost, got {}",
                recipe.id,
                recipe.qi_cost
            );
        }
    }

    #[test]
    fn workbench_recipes_reject_duplicate_registration() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        let err = register_workbench_recipes(&mut registry);
        assert!(
            matches!(err, Err(RegistryError::DuplicateId(_))),
            "double registration must fail with DuplicateId"
        );
    }

    #[test]
    fn workbench_recipe_count_by_group() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        let mut workbench_count = 0;
        for recipe in registry.iter() {
            if recipe.id.as_str().starts_with("workbench.") {
                workbench_count += 1;
            }
        }
        let expected_count = p0_baseline_count_matching(|recipe| {
            recipe["id"]
                .as_str()
                .expect("baseline recipe id must be a string")
                .starts_with("workbench.")
        });
        assert_eq!(
            workbench_count, expected_count,
            "workbench.* recipe count must derive from the canonical fixture"
        );
    }

    #[test]
    fn alchemy_forge_prep_recipes_output_placeable_items_with_same_materials() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        let item_registry = crate::inventory::load_item_registry().expect("item registry loads");

        let furnace_recipe = registry
            .get(&RecipeId::new("workbench.prep.furnace_kit"))
            .expect("furnace prep recipe must exist");
        assert_eq!(
            furnace_recipe.materials,
            vec![
                ("iron_ingot".into(), 6),
                ("spirit_charcoal".into(), 2),
                ("stone_chunk".into(), 4),
            ],
            "furnace prep materials must stay unchanged while replacing the dead kit output"
        );
        assert_eq!(
            furnace_recipe.output,
            ("furnace_fantie".into(), 1),
            "furnace prep must output the item accepted by furnace placement"
        );
        assert_eq!(
            crate::alchemy::furnace::furnace_tier_from_item_id(&furnace_recipe.output.0),
            Some(1),
            "furnace prep output must map to tier-1 furnace placement"
        );

        let forge_recipe = registry
            .get(&RecipeId::new("workbench.prep.forge_station"))
            .expect("forge station prep recipe must exist");
        assert_eq!(
            forge_recipe.materials,
            vec![
                ("iron_ingot".into(), 4),
                ("spirit_wood".into(), 2),
                ("stone_chunk".into(), 6),
            ],
            "forge prep materials must stay unchanged while replacing the dead kit output"
        );
        assert_eq!(
            forge_recipe.output,
            ("fan_iron_anvil".into(), 1),
            "forge prep must output the item carrying forge_station_spec"
        );
        let forge_station_tier = item_registry
            .get(&forge_recipe.output.0)
            .and_then(|template| template.forge_station_spec.as_ref())
            .map(|spec| spec.tier);
        assert_eq!(
            forge_station_tier,
            Some(1),
            "forge prep output must be a tier-1 forge station item"
        );
    }

    #[test]
    fn old_alchemy_forge_kit_templates_are_removed_from_registry_and_recipes() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        let item_registry = crate::inventory::load_item_registry().expect("item registry loads");
        let removed_ids = ["furnace_kit_fantie", "forge_station_kit"];

        for removed_id in removed_ids {
            assert!(
                item_registry.get(removed_id).is_none(),
                "old kit template `{removed_id}` must be absent from ItemRegistry"
            );
        }
        assert!(
            item_registry.get("furnace_fantie").is_some(),
            "replacement furnace item must remain registered"
        );
        assert!(
            item_registry.get("fan_iron_anvil").is_some(),
            "replacement forge station item must remain registered"
        );

        for recipe in registry.iter() {
            let (output_id, _) = &recipe.output;
            assert!(
                !removed_ids.contains(&output_id.as_str()),
                "recipe `{}` must not output removed kit `{output_id}`",
                recipe.id
            );
            for (material_id, _) in &recipe.materials {
                assert!(
                    !removed_ids.contains(&material_id.as_str()),
                    "recipe `{}` must not consume removed kit `{material_id}`",
                    recipe.id
                );
            }
        }
    }

    #[test]
    fn economy_zombie_templates_and_recipes_are_removed() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        let item_registry = crate::inventory::load_item_registry().expect("item registry loads");
        let removed_ids = [
            "bone_coin_blank",
            "waymark_stone",
            "price_tag",
            "trade_puppet_frame",
            "rat_bait",
        ];
        let removed_recipe_ids = [
            "workbench.economy.coin_blank",
            "workbench.economy.waymark",
            "workbench.economy.price_tag",
            "workbench.economy.puppet_frame",
            "workbench.cultivation.rat_bait",
        ];

        for removed_id in removed_ids {
            assert!(
                item_registry.get(removed_id).is_none(),
                "P2 zombie template `{removed_id}` must be absent from ItemRegistry"
            );
        }

        for recipe_id in removed_recipe_ids {
            assert!(
                registry.get(&RecipeId::new(recipe_id)).is_none(),
                "P2 zombie recipe `{recipe_id}` must be absent from CraftRegistry"
            );
        }

        for recipe in registry.iter() {
            let (output_id, _) = &recipe.output;
            assert!(
                !removed_ids.contains(&output_id.as_str()),
                "recipe `{}` must not output removed economy item `{output_id}`",
                recipe.id
            );
            for (material_id, _) in &recipe.materials {
                assert!(
                    !removed_ids.contains(&material_id.as_str()),
                    "recipe `{}` must not consume removed economy item `{material_id}`",
                    recipe.id
                );
            }
        }
    }

    #[test]
    fn material_tool_zombie_templates_and_recipes_are_removed() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        let item_registry = crate::inventory::load_item_registry().expect("item registry loads");
        let removed_ids = [
            "powder_zhu_sha",
            "iron_sword_blank",
            "stone_spearhead",
            "sling_stone",
            "sling_weapon",
            "stone_hoe",
            "mortar_stone",
            "heat_gloves",
            "trade_scale",
            "trade_scale_stand",
        ];
        let removed_recipe_ids = [
            "workbench.process.zhu_sha_powder",
            "workbench.weapon.iron_sword_blank",
            "workbench.weapon.stone_spearhead",
            "workbench.weapon.sling_stone",
            "workbench.weapon.sling",
            "workbench.tool.stone_hoe",
            "workbench.tool.mortar",
            "workbench.tool.heat_gloves",
            "workbench.tool.trade_scale",
            "workbench.economy.trade_scale_stand",
        ];

        assert!(
            item_registry.get("bing_jia_shou_tao").is_some(),
            "bing_jia_shou_tao must stay registered; it is not the removed heat_gloves item"
        );
        assert!(
            registry
                .get(&RecipeId::new("workbench.tool.ice_gauntlet"))
                .is_some(),
            "bing_jia_shou_tao recipe must stay registered while heat_gloves is removed"
        );

        for removed_id in removed_ids {
            assert!(
                item_registry.get(removed_id).is_none(),
                "P3 zombie template `{removed_id}` must be absent from ItemRegistry"
            );
        }

        for recipe_id in removed_recipe_ids {
            assert!(
                registry.get(&RecipeId::new(recipe_id)).is_none(),
                "P3 zombie recipe `{recipe_id}` must be absent from CraftRegistry"
            );
        }

        for recipe in registry.iter() {
            let (output_id, _) = &recipe.output;
            assert!(
                !removed_ids.contains(&output_id.as_str()),
                "recipe `{}` must not output removed P3 item `{output_id}`",
                recipe.id
            );
            for (material_id, _) in &recipe.materials {
                assert!(
                    !removed_ids.contains(&material_id.as_str()),
                    "recipe `{}` must not consume removed P3 item `{material_id}`",
                    recipe.id
                );
            }
            assert!(
                !recipe.unlock_sources.iter().any(|unlock| matches!(
                    unlock,
                    UnlockSource::Scroll { item_template }
                        if item_template == "scroll_workbench_heat_gloves"
                )),
                "recipe `{}` must not retain removed heat_gloves unlock scroll",
                recipe.id
            );
        }
    }

    #[test]
    fn workbench_material_and_output_ids_exist_in_item_registry() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        let item_registry = crate::inventory::load_item_registry().expect("item registry loads");

        for recipe in registry.iter() {
            for (template_id, count) in &recipe.materials {
                assert!(
                    item_registry.get(template_id).is_some(),
                    "material `{template_id}` for recipe `{}` must exist in item registry",
                    recipe.id
                );
                assert!(
                    *count >= 1,
                    "material `{template_id}` for recipe `{}` must have count >= 1",
                    recipe.id
                );
            }
            let (output_id, count) = &recipe.output;
            assert!(
                item_registry.get(output_id).is_some(),
                "output `{output_id}` for recipe `{}` must exist in item registry",
                recipe.id
            );
            assert!(
                *count >= 1,
                "output count for recipe `{}` must be >= 1",
                recipe.id
            );
        }
    }

    #[test]
    fn workbench_spirit_quality_conservation() {
        // 灵质守恒律：output sq ≤ input sq × 0.95
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        let item_registry = crate::inventory::load_item_registry().expect("item registry loads");

        for recipe in registry.iter() {
            let input_sq: f64 = recipe
                .materials
                .iter()
                .filter_map(|(template_id, count)| {
                    item_registry
                        .get(template_id)
                        .map(|t| t.spirit_quality_initial * *count as f64)
                })
                .sum();

            let (output_id, output_count) = &recipe.output;
            let output_sq = item_registry
                .get(output_id)
                .map(|t| t.spirit_quality_initial * *output_count as f64)
                .unwrap_or(0.0);

            // qi_cost 可视为额外灵质输入（真元灌注）
            let total_input_sq = input_sq + recipe.qi_cost;

            // 只对有灵质投入且有灵质产出的配方校验守恒律。
            // 当 total_input_sq == 0 且 output_sq > 0 时，产出灵质来自
            // 材料的固有属性（如兽皮甲的微灵来自兽皮本身），不违反
            // worldview §八 磨损律（磨损律前提是"灵物操作"，凡物组装不适用）。
            if output_sq > 0.0 && total_input_sq > 0.0 {
                assert!(
                    output_sq <= total_input_sq * 0.95 + 1e-9,
                    "recipe `{}` violates conservation: output_sq={:.3} > (input_sq={:.3} + qi_cost={:.1}) × 0.95 = {:.3}",
                    recipe.id, output_sq, input_sq, recipe.qi_cost, total_input_sq * 0.95
                );
            }
        }
    }

    #[test]
    fn all_workbench_asset_recipes_individual_pin() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        let item_registry = crate::inventory::load_item_registry().expect("item registry loads");

        let mut workbench_count = 0u32;
        let mut handcraft_stone_count = 0u32;
        for recipe in registry.iter() {
            let rid = recipe.id.as_str();
            if rid == "craft.tool.workbench" {
                assert_eq!(
                    recipe.station, None,
                    "[{}] self recipe must be handcraft",
                    recipe.id
                );
                continue;
            }
            // 石器工具(石镐/石斧/石刃)改为手搓配方(station None)，让玩家在手搓台直接做。
            // 仍走下面的物品/守恒校验，只是归手搓、不计入 workbench_count。
            if HANDCRAFT_STONE_TOOLS.contains(&rid) {
                assert_eq!(
                    recipe.station, None,
                    "[{}] 石器工具改手搓，station 必须 None",
                    recipe.id
                );
                handcraft_stone_count += 1;
            } else {
                workbench_count += 1;
                assert_eq!(
                    recipe.station,
                    Some(CraftStationKind::Workbench),
                    "[{}] station must be Workbench",
                    recipe.id
                );
            }
            assert!(
                recipe.time_ticks > 0,
                "[{}] time_ticks must be > 0, got {}",
                recipe.id,
                recipe.time_ticks
            );
            assert!(
                recipe.qi_cost >= 0.0 && recipe.qi_cost.is_finite(),
                "[{}] qi_cost must be finite non-negative, got {}",
                recipe.id,
                recipe.qi_cost
            );
            for (mat_id, count) in &recipe.materials {
                assert!(
                    item_registry.get(mat_id).is_some(),
                    "[{}] material `{}` not found in item registry",
                    recipe.id,
                    mat_id
                );
                assert!(
                    *count >= 1,
                    "[{}] material `{}` count < 1",
                    recipe.id,
                    mat_id
                );
            }
            let (out_id, out_count) = &recipe.output;
            assert!(
                item_registry.get(out_id).is_some(),
                "[{}] output `{}` not found in item registry",
                recipe.id,
                out_id
            );
            assert!(*out_count >= 1, "[{}] output count < 1", recipe.id);

            let input_sq: f64 = recipe
                .materials
                .iter()
                .filter_map(|(tid, c)| {
                    item_registry
                        .get(tid)
                        .map(|t| t.spirit_quality_initial * *c as f64)
                })
                .sum();
            let total_input = input_sq + recipe.qi_cost;
            let output_sq = item_registry
                .get(out_id)
                .map(|t| t.spirit_quality_initial * *out_count as f64)
                .unwrap_or(0.0);
            if output_sq > 0.0 && total_input > 0.0 {
                assert!(
                    output_sq <= total_input * 0.95 + 1e-9,
                    "[{}] conservation violated: out_sq={:.3} > in_sq={:.3} × 0.95 = {:.3}",
                    recipe.id,
                    output_sq,
                    total_input,
                    total_input * 0.95
                );
            }
        }
        let expected_handcraft_count = p0_baseline_count_matching(|recipe| {
            let id = recipe["id"]
                .as_str()
                .expect("baseline recipe id must be a string");
            HANDCRAFT_STONE_TOOLS.contains(&id)
        });
        assert_eq!(
            handcraft_stone_count as usize, expected_handcraft_count,
            "手搓石器数量必须由 canonical fixture 推导"
        );
        let expected_station_count =
            p0_baseline_count_matching(|recipe| recipe["station"].as_str() == Some("workbench"));
        assert_eq!(
            workbench_count as usize, expected_station_count,
            "制作台配方数量必须由 canonical fixture 推导"
        );
    }

    #[test]
    fn trap_runtime_recipes_are_grouped_under_zhenfa_trap() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();

        let cases = [
            (
                "workbench.array.trip_wire",
                ("trip_wire", 3),
                vec![
                    ("spider_silk_cord".to_string(), 2),
                    ("iron_needle".to_string(), 1),
                ],
            ),
            (
                "workbench.array.beast_trap",
                ("beast_trap", 1),
                vec![("iron_ingot".to_string(), 3), ("grass_rope".to_string(), 2)],
            ),
            (
                "workbench.array.bait_stake",
                ("bait_stake", 1),
                vec![
                    ("wood_handle".to_string(), 1),
                    ("dried_grass".to_string(), 3),
                    ("rough_cloth".to_string(), 1),
                    ("raw_beast_hide".to_string(), 1),
                ],
            ),
        ];

        for (recipe_id, output, materials) in cases {
            let recipe = registry
                .get(&RecipeId::new(recipe_id))
                .expect("trap runtime recipe must exist");
            assert_eq!(recipe.category, CraftCategory::ZhenfaTrap);
            assert_eq!(recipe.qi_cost, 0.0);
            assert_eq!(recipe.output, (output.0.to_string(), output.1));
            assert_eq!(recipe.materials, materials);
        }
    }

    #[test]
    fn workbench_physics_derivation_spot_check() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();

        #[allow(clippy::type_complexity)]
        let spot_checks: Vec<(&str, Vec<(&str, u32)>, f64, u64, (&str, u32))> = vec![
            // #22 灵木炭: spirit_wood×2 → spirit_charcoal×3, qi=0, time=60s
            (
                "workbench.process.spirit_charcoal",
                vec![("spirit_wood", 2)],
                0.0,
                60 * 20,
                ("spirit_charcoal", 3),
            ),
            // #27 灵草束: spirit_grass×5 → herb_bundle×1, qi=0, time=10s
            (
                "workbench.process.herb_bundle",
                vec![("spirit_grass", 5)],
                0.0,
                10 * 20,
                ("herb_bundle", 1),
            ),
            // #32 密封药瓶: herb_vial×1 + spider_silk_cord×1 → sealed_vial×1, qi=1.0
            (
                "workbench.container.sealed_vial",
                vec![("herb_vial", 1), ("spider_silk_cord", 1)],
                1.0,
                25 * 20,
                ("sealed_vial", 1),
            ),
            // #16 蛛丝绳: ash_spider_silk×4 → spider_silk_cord×1, qi=0
            (
                "workbench.process.spider_cord",
                vec![("ash_spider_silk", 4)],
                0.0,
                30 * 20,
                ("spider_silk_cord", 1),
            ),
            // #17 粗布: ash_spider_silk×5 → rough_cloth×1, qi=0
            (
                "workbench.process.rough_cloth",
                vec![("ash_spider_silk", 5)],
                0.0,
                45 * 20,
                ("rough_cloth", 1),
            ),
            // #18 熟皮: raw_beast_hide×1 + hui_jin_tai×1 → tanned_hide×1, qi=0
            (
                "workbench.process.tanned_hide",
                vec![("raw_beast_hide", 1), ("hui_jin_tai", 1)],
                0.0,
                60 * 20,
                ("tanned_hide", 1),
            ),
            // #19 骨片: shu_gu×1 → bone_chip_mat×4, qi=0
            (
                "workbench.process.bone_chip",
                vec![("shu_gu", 1)],
                0.0,
                10 * 20,
                ("bone_chip_mat", 4),
            ),
            // #24 鼠尾油: rat_tail×3 → rat_tail_oil×1, qi=0
            (
                "workbench.process.rat_tail_oil",
                vec![("rat_tail", 3)],
                0.0,
                45 * 20,
                ("rat_tail_oil", 1),
            ),
            // #25 盐蓬晶: bai_yan_peng×3 → salt_crystal×1, qi=0
            (
                "workbench.process.salt_crystal",
                vec![("bai_yan_peng", 3)],
                0.0,
                35 * 20,
                ("salt_crystal", 1),
            ),
            // #21 粗铁锭: cu_tie×2 → iron_ingot×1, qi=0
            (
                "workbench.process.iron_ingot",
                vec![("cu_tie", 2)],
                0.0,
                50 * 20,
                ("iron_ingot", 1),
            ),
        ];

        for (id, expected_mats, expected_qi, expected_ticks, expected_output) in &spot_checks {
            let recipe = registry
                .get(&RecipeId::new(*id))
                .unwrap_or_else(|| panic!("[spot-check] recipe `{id}` not found"));

            let actual_mats: Vec<(&str, u32)> = recipe
                .materials
                .iter()
                .map(|(t, c)| (t.as_str(), *c))
                .collect();
            assert_eq!(actual_mats, *expected_mats, "[{id}] materials mismatch");
            assert!(
                (recipe.qi_cost - expected_qi).abs() < 1e-9,
                "[{id}] qi_cost: expected {expected_qi}, got {}",
                recipe.qi_cost
            );
            assert_eq!(
                recipe.time_ticks, *expected_ticks,
                "[{id}] time_ticks mismatch"
            );
            assert_eq!(
                recipe.output,
                (expected_output.0.into(), expected_output.1),
                "[{id}] output mismatch"
            );
        }
    }

    #[test]
    fn all_qi_cost_positive_recipes_have_finite_cost() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        let mut qi_recipe_count = 0u32;
        for recipe in registry.iter() {
            if recipe.qi_cost > 0.0 {
                qi_recipe_count += 1;
                assert!(
                    recipe.qi_cost.is_finite(),
                    "[{}] qi_cost must be finite, got {}",
                    recipe.id,
                    recipe.qi_cost
                );
                assert!(
                    recipe.qi_cost > 0.0,
                    "[{}] qi_cost flagged positive but is {}",
                    recipe.id,
                    recipe.qi_cost
                );
            }
        }
        let expected_qi_recipe_count = p0_baseline_count_matching(|recipe| {
            recipe["qi_cost"]
                .as_f64()
                .expect("baseline qi_cost must be numeric")
                > 0.0
                && !recipe["id"]
                    .as_str()
                    .expect("baseline recipe id must be a string")
                    .starts_with("craft.example.")
        });
        assert_eq!(
            qi_recipe_count as usize, expected_qi_recipe_count,
            "positive-qi recipe count must derive from the canonical fixture"
        );
    }
}
