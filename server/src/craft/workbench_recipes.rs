//! plan-workbench-recipes-v1 §P2.2 — 制作台 100 配方注册。
//!
//! 分 10 组子函数，严格对应 plan §P1.1 十大类表格。
//! 所有 workbench 配方 `station: Some(CraftStationKind::Workbench)`。
//!
//! 此外包含制作台自身的手搓配方（§P0.4）：`craft.tool.workbench`，station: None。
//!
//! **time 单位**：plan 表中 `t` 列为秒，需 × 20 转 ticks。
//! **unlock 映射**：🔓 = Default (empty vec), 📜 = Scroll, 👤 = Mentor, 💡 = Insight。

use super::recipe::{
    CraftCategory, CraftRecipe, CraftRequirements, CraftStationKind, RecipeId, UnlockSource,
};
use super::registry::{CraftRegistry, RegistryError};
use crate::cultivation::components::Realm;

const WORKBENCH: Option<CraftStationKind> = Some(CraftStationKind::Workbench);

/// 7-元组配方描述：(id, name, materials, qi, time_sec, output, unlock)
type RecipeRow<'a> = (
    &'a str,
    &'a str,
    Vec<(&'a str, u32)>,
    f64,
    u64,
    (&'a str, u32),
    Vec<UnlockSource>,
);

/// 5-元组护甲/庇护配方描述：(id, name, materials, time_sec, output_id)
type ArmorRow<'a> = (&'a str, &'a str, Vec<(&'a str, u32)>, u64, &'a str);

/// 5-元组医药配方描述：(id, name, materials, time_sec, (output_id, count))
type MedicalRow<'a> = (&'a str, &'a str, Vec<(&'a str, u32)>, u64, (&'a str, u32));

/// 8-元组配方描述（带自定义 category）：(id, name, materials, qi, time_sec, output, category, unlock)
type CategorizedRow<'a> = (
    &'a str,
    &'a str,
    Vec<(&'a str, u32)>,
    f64,
    u64,
    (&'a str, u32),
    CraftCategory,
    Vec<UnlockSource>,
);

/// 滚筒 unlock helper
fn scroll(scroll_id: &str) -> Vec<UnlockSource> {
    vec![UnlockSource::Scroll {
        item_template: scroll_id.to_string(),
    }]
}

fn mentor(npc: &str) -> Vec<UnlockSource> {
    vec![UnlockSource::Mentor {
        npc_archetype: npc.to_string(),
    }]
}

/// 注册全部制作台配方 + 制作台自身手搓配方。
/// plan-coffin-tiers-v1 P4 加入 3 档延寿棺、P2 经济僵尸清理后共 98 workbench/coffin + 1 self = 99 条。
pub fn register_workbench_recipes(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    register_workbench_self_recipe(registry)?;
    register_survival_tools(registry)?; // #1-12
    register_processing(registry)?; // #13-30
    register_containers(registry)?; // #31-42
    register_basic_armor(registry)?; // #43-52
    register_weapon_components(registry)?; // #53-62
    register_cultivation_support(registry)?; // #63-74
    register_array_basics(registry)?; // #75-82
    register_economy(registry)?; // #83-90
    register_shelter(registry)?; // #91-98
    register_alchemy_forge_prep(registry)?; // #99-100
    register_coffin_tiers(registry)?; // #101-103（plan-coffin-tiers-v1 P4）
    Ok(())
}

/// §P0.4：制作台自身手搓配方。station: None（手搓）。
fn register_workbench_self_recipe(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    registry.register(CraftRecipe {
        id: RecipeId::new("craft.tool.workbench"),
        category: CraftCategory::Tool,
        display_name: "制作台".into(),
        materials: vec![
            ("spirit_wood".into(), 4),
            ("iron_ingot".into(), 2),
            ("shu_gu".into(), 2),
        ],
        qi_cost: 0.0,
        time_ticks: 60 * 20, // 60s
        output: ("workbench_item".into(), 1),
        requirements: CraftRequirements::default(),
        unlock_sources: vec![],
        station: None, // 手搓
    })
}

/// 一、生存凡器 #1-12
fn register_survival_tools(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    let recipes: Vec<RecipeRow> = vec![
        // #1 石镐
        (
            "workbench.tool.stone_pickaxe",
            "石镐",
            vec![("stone_chunk", 3), ("wood_handle", 1)],
            0.0,
            25,
            ("stone_pickaxe", 1),
            vec![],
        ),
        // #2 石斧
        (
            "workbench.tool.stone_axe",
            "石斧",
            vec![("stone_chunk", 2), ("wood_handle", 1)],
            0.0,
            25,
            ("stone_axe", 1),
            vec![],
        ),
        // #4 铁镐
        (
            "workbench.tool.pickaxe_iron",
            "铁镐",
            vec![("iron_ingot", 3), ("wood_handle", 1)],
            0.0,
            45,
            ("pickaxe_iron", 1),
            vec![],
        ),
        // #5 铁斧
        (
            "workbench.tool.axe_iron",
            "铁斧",
            vec![("iron_ingot", 2), ("wood_handle", 1)],
            0.0,
            40,
            ("axe_iron", 1),
            vec![],
        ),
        // #6 铁锄
        (
            "workbench.tool.hoe_iron",
            "铁锄",
            vec![("iron_ingot", 1), ("wood_handle", 1)],
            0.0,
            35,
            ("hoe_iron", 1),
            vec![],
        ),
        // #8 草镰
        (
            "workbench.tool.sickle",
            "草镰",
            vec![("iron_ingot", 2), ("wood_handle", 1)],
            0.0,
            40,
            ("cao_lian", 1),
            vec![],
        ),
        // #11 冰甲手套
        (
            "workbench.tool.ice_gauntlet",
            "冰甲手套",
            vec![("tanned_hide", 2), ("iron_ingot", 1)],
            0.0,
            50,
            ("bing_jia_shou_tao", 1),
            vec![],
        ),
        // #12 刮刀
        (
            "workbench.tool.scraper",
            "刮刀",
            vec![("iron_ingot", 1), ("bone_chip_mat", 1)],
            0.0,
            30,
            ("gua_dao", 1),
            vec![],
        ),
    ];

    for (id, name, mats, qi, t_sec, output, unlock) in recipes {
        registry.register(CraftRecipe {
            id: RecipeId::new(id),
            category: CraftCategory::Tool,
            display_name: name.into(),
            materials: mats.into_iter().map(|(t, c)| (t.into(), c)).collect(),
            qi_cost: qi,
            time_ticks: t_sec * 20,
            output: (output.0.into(), output.1),
            requirements: CraftRequirements::default(),
            unlock_sources: unlock,
            station: WORKBENCH,
        })?;
    }
    Ok(())
}

/// 二、材料加工 #13-30
fn register_processing(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    let recipes: Vec<RecipeRow> = vec![
        // #13 木柄
        (
            "workbench.process.wood_handle",
            "木柄",
            vec![("spirit_wood", 1)],
            0.0,
            15,
            ("wood_handle", 4),
            vec![],
        ),
        // #14 木板
        (
            "workbench.process.wood_plank",
            "木板",
            vec![("spirit_wood", 1)],
            0.0,
            15,
            ("wood_plank", 2),
            vec![],
        ),
        // #15 草绳
        (
            "workbench.process.rope",
            "草绳",
            vec![("grass_fiber", 3)],
            0.0,
            20,
            ("grass_rope", 1),
            vec![],
        ),
        // #16 蛛丝绳
        (
            "workbench.process.spider_cord",
            "蛛丝绳",
            vec![("ash_spider_silk", 4)],
            0.0,
            30,
            ("spider_silk_cord", 1),
            vec![],
        ),
        // #17 粗布
        (
            "workbench.process.rough_cloth",
            "粗布",
            vec![("ash_spider_silk", 5)],
            0.0,
            45,
            ("rough_cloth", 1),
            vec![],
        ),
        // #18 熟皮
        (
            "workbench.process.tanned_hide",
            "熟皮",
            vec![("raw_beast_hide", 1), ("hui_jin_tai", 1)],
            0.0,
            60,
            ("tanned_hide", 1),
            vec![],
        ),
        // #19 骨片
        (
            "workbench.process.bone_chip",
            "骨片",
            vec![("shu_gu", 1)],
            0.0,
            10,
            ("bone_chip_mat", 4),
            vec![],
        ),
        // #20 骨粉
        (
            "workbench.process.bone_meal",
            "骨粉",
            vec![("shu_gu", 2)],
            0.0,
            25,
            ("bone_meal_mat", 1),
            vec![],
        ),
        // #21 粗铁锭
        (
            "workbench.process.iron_ingot",
            "粗铁锭",
            vec![("cu_tie", 2)],
            0.0,
            50,
            ("iron_ingot", 1),
            vec![],
        ),
        // #22 灵木炭
        (
            "workbench.process.spirit_charcoal",
            "灵木炭",
            vec![("spirit_wood", 2)],
            0.0,
            60,
            ("spirit_charcoal", 3),
            vec![],
        ),
        // #23 干草
        (
            "workbench.process.dried_grass",
            "干草",
            vec![("grass_fiber", 5)],
            0.0,
            20,
            ("dried_grass", 3),
            vec![],
        ),
        // #24 鼠尾油
        (
            "workbench.process.rat_tail_oil",
            "鼠尾油",
            vec![("rat_tail", 3)],
            0.0,
            45,
            ("rat_tail_oil", 1),
            vec![],
        ),
        // #25 盐蓬晶
        (
            "workbench.process.salt_crystal",
            "盐蓬晶",
            vec![("bai_yan_peng", 3)],
            0.0,
            35,
            ("salt_crystal", 1),
            vec![],
        ),
        // #26 陶罐
        (
            "workbench.process.clay_pot",
            "陶罐",
            vec![("stone_chunk", 3)],
            0.0,
            40,
            ("clay_pot", 1),
            vec![],
        ),
        // #27 灵草束
        (
            "workbench.process.herb_bundle",
            "灵草束",
            vec![("spirit_grass", 5)],
            0.0,
            10,
            ("herb_bundle", 1),
            vec![],
        ),
        // #28 丹砂粉
        (
            "workbench.process.dan_sha_powder",
            "丹砂粉",
            vec![("dan_sha", 1)],
            0.0,
            15,
            ("powder_dan_sha", 3),
            vec![],
        ),
        // #30 铁针
        (
            "workbench.process.needle_batch",
            "铁针",
            vec![("iron_ingot", 1)],
            0.0,
            20,
            ("iron_needle", 5),
            vec![],
        ),
    ];

    for (id, name, mats, qi, t_sec, output, unlock) in recipes {
        registry.register(CraftRecipe {
            id: RecipeId::new(id),
            category: CraftCategory::Misc,
            display_name: name.into(),
            materials: mats.into_iter().map(|(t, c)| (t.into(), c)).collect(),
            qi_cost: qi,
            time_ticks: t_sec * 20,
            output: (output.0.into(), output.1),
            requirements: CraftRequirements::default(),
            unlock_sources: unlock,
            station: WORKBENCH,
        })?;
    }
    Ok(())
}

/// 三、容器与存储 #31-42
fn register_containers(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    let recipes: Vec<RecipeRow> = vec![
        // #31 药瓶
        (
            "workbench.container.herb_vial",
            "药瓶",
            vec![("stone_chunk", 2), ("iron_ingot", 1)],
            0.0,
            35,
            ("herb_vial", 2),
            vec![],
        ),
        // #32 密封药瓶
        (
            "workbench.container.sealed_vial",
            "密封药瓶",
            vec![("herb_vial", 1), ("spider_silk_cord", 1)],
            1.0,
            25,
            ("sealed_vial", 1),
            scroll("scroll_workbench_sealed_vial"),
        ),
        // #33 灵草囊
        (
            "workbench.container.herb_pouch",
            "灵草囊",
            vec![("rough_cloth", 2), ("grass_rope", 1)],
            0.0,
            40,
            ("herb_pouch", 1),
            vec![],
        ),
        // #34 灵草箱
        (
            "workbench.container.herb_crate",
            "灵草箱",
            vec![("wood_plank", 4), ("rough_cloth", 1)],
            0.0,
            45,
            ("herb_crate", 1),
            vec![],
        ),
        // #35 暗器袋
        (
            "workbench.container.projectile_bag",
            "暗器袋",
            vec![("tanned_hide", 2), ("grass_rope", 1)],
            0.0,
            40,
            ("projectile_bag", 1),
            vec![],
        ),
        // #36 封灵匣
        (
            "workbench.container.seal_box",
            "封灵匣",
            vec![("wood_plank", 4), ("ling_tie", 1)],
            3.0,
            90,
            ("spirit_seal_box", 1),
            scroll("scroll_workbench_seal_box"),
        ),
        // #37 死信箱 (📜👤)
        (
            "workbench.container.dead_drop",
            "死信箱",
            vec![
                ("wood_plank", 6),
                ("iron_ingot", 2),
                ("zhenfa_blank_array", 1),
            ],
            4.0,
            120,
            ("dead_drop_box", 1),
            vec![
                UnlockSource::Scroll {
                    item_template: "scroll_workbench_dead_drop".into(),
                },
                UnlockSource::Mentor {
                    npc_archetype: "smuggler".into(),
                },
            ],
        ),
        // #38 防潮包
        (
            "workbench.container.moisture_guard",
            "防潮包",
            vec![("rough_cloth", 1), ("hui_jin_tai", 2)],
            0.0,
            25,
            ("moisture_guard", 2),
            vec![],
        ),
        // #39 矿石袋
        (
            "workbench.container.ore_sack",
            "矿石袋",
            vec![("rough_cloth", 2), ("tanned_hide", 1)],
            0.0,
            30,
            ("ore_sack", 1),
            vec![],
        ),
        // #40 水囊
        (
            "workbench.container.water_skin",
            "水囊",
            vec![("tanned_hide", 1), ("grass_rope", 1)],
            0.0,
            30,
            ("water_skin", 1),
            vec![],
        ),
        // #41 货箱
        (
            "workbench.container.trade_crate",
            "货箱",
            vec![("wood_plank", 6), ("iron_ingot", 2)],
            0.0,
            65,
            ("trade_crate", 1),
            vec![],
        ),
        // #42 密封信封
        (
            "workbench.container.sealed_envelope",
            "密封信封",
            vec![("rough_cloth", 1), ("rat_tail_oil", 1)],
            0.0,
            20,
            ("sealed_envelope", 3),
            vec![],
        ),
    ];

    for (id, name, mats, qi, t_sec, output, unlock) in recipes {
        registry.register(CraftRecipe {
            id: RecipeId::new(id),
            category: CraftCategory::Container,
            display_name: name.into(),
            materials: mats.into_iter().map(|(t, c)| (t.into(), c)).collect(),
            qi_cost: qi,
            time_ticks: t_sec * 20,
            output: (output.0.into(), output.1),
            requirements: CraftRequirements::default(),
            unlock_sources: unlock,
            station: WORKBENCH,
        })?;
    }
    Ok(())
}

/// 四、基础护甲 #43-52（含医疗夹板 #51-52 归 Misc）
fn register_basic_armor(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    // #43-50 护甲 → ArmorCraft category
    let armor_recipes: Vec<ArmorRow> = vec![
        // #43 草甲·头
        (
            "workbench.armor.armor_straw_helmet",
            "草甲·头",
            vec![("dried_grass", 5), ("grass_rope", 1)],
            30,
            "armor_straw_helmet",
        ),
        // #44 草甲·胸
        (
            "workbench.armor.straw_chest",
            "草甲·胸",
            vec![("dried_grass", 8), ("grass_rope", 2)],
            45,
            "armor_straw_chestplate",
        ),
        // #45 草甲·腿
        (
            "workbench.armor.straw_legs",
            "草甲·腿",
            vec![("dried_grass", 6), ("grass_rope", 2)],
            40,
            "armor_straw_leggings",
        ),
        // #46 草甲·脚
        (
            "workbench.armor.armor_straw_boots",
            "草甲·脚",
            vec![("dried_grass", 4), ("grass_rope", 1)],
            25,
            "armor_straw_boots",
        ),
        // #47 皮甲·头
        (
            "workbench.armor.armor_hide_helmet",
            "皮甲·头",
            vec![("tanned_hide", 2), ("grass_rope", 1)],
            45,
            "armor_hide_helmet",
        ),
        // #48 皮甲·胸
        (
            "workbench.armor.hide_chest",
            "皮甲·胸",
            vec![("tanned_hide", 4), ("grass_rope", 2)],
            65,
            "armor_hide_chestplate",
        ),
        // #49 皮甲·腿
        (
            "workbench.armor.hide_legs",
            "皮甲·腿",
            vec![("tanned_hide", 3), ("grass_rope", 2)],
            55,
            "armor_hide_leggings",
        ),
        // #50 皮甲·脚
        (
            "workbench.armor.armor_hide_boots",
            "皮甲·脚",
            vec![("tanned_hide", 2), ("grass_rope", 1)],
            40,
            "armor_hide_boots",
        ),
    ];

    for (id, name, mats, t_sec, output) in armor_recipes {
        registry.register(CraftRecipe {
            id: RecipeId::new(id),
            category: CraftCategory::ArmorCraft,
            display_name: name.into(),
            materials: mats.into_iter().map(|(t, c)| (t.into(), c)).collect(),
            qi_cost: 0.0,
            time_ticks: t_sec * 20,
            output: (output.into(), 1),
            requirements: CraftRequirements::default(),
            unlock_sources: vec![],
            station: WORKBENCH,
        })?;
    }

    // #51-52 夹板 → Misc category
    let medical_recipes: Vec<MedicalRow> = vec![
        // #51 夹板·臂
        (
            "workbench.medical.arm_splint",
            "夹板·臂",
            vec![("wood_plank", 2), ("rough_cloth", 1)],
            15,
            ("arm_splint", 2),
        ),
        // #52 夹板·腿
        (
            "workbench.medical.leg_splint",
            "夹板·腿",
            vec![("wood_plank", 3), ("rough_cloth", 2)],
            20,
            ("leg_splint", 2),
        ),
    ];

    for (id, name, mats, t_sec, output) in medical_recipes {
        registry.register(CraftRecipe {
            id: RecipeId::new(id),
            category: CraftCategory::Misc,
            display_name: name.into(),
            materials: mats.into_iter().map(|(t, c)| (t.into(), c)).collect(),
            qi_cost: 0.0,
            time_ticks: t_sec * 20,
            output: (output.0.into(), output.1),
            requirements: CraftRequirements::default(),
            unlock_sources: vec![],
            station: WORKBENCH,
        })?;
    }
    Ok(())
}

/// 五、基础武器组件 #53-62
fn register_weapon_components(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    let recipes: Vec<CategorizedRow> = vec![
        // #54 粗骨刺 (AnqiCarrier)
        (
            "workbench.weapon.bone_spike_crude",
            "粗骨刺",
            vec![("shu_gu", 2), ("stone_chunk", 1)],
            0.0,
            40,
            ("bone_spike_crude", 3),
            CraftCategory::AnqiCarrier,
            vec![],
        ),
        // #55 木棍
        (
            "workbench.weapon.wooden_club",
            "木棍",
            vec![("spirit_wood", 2)],
            0.0,
            20,
            ("wooden_club", 1),
            CraftCategory::Misc,
            vec![],
        ),
        // #59 木盾 (ArmorCraft) — plan-shield-block-v1 P0: recipe id 由 weapon.* 归位到 shield.*
        (
            "workbench.shield.wooden_shield",
            "木盾",
            vec![("wood_plank", 4), ("iron_ingot", 1)],
            0.0,
            60,
            ("wooden_shield", 1),
            CraftCategory::ArmorCraft,
            vec![],
        ),
        // #60 骨盾 (ArmorCraft) — plan-shield-block-v1 P0: recipe id 由 weapon.* 归位到 shield.*
        (
            "workbench.shield.bone_shield",
            "骨盾",
            vec![("shu_gu", 6), ("grass_rope", 2)],
            0.0,
            55,
            ("bone_shield", 1),
            CraftCategory::ArmorCraft,
            vec![],
        ),
        // #61 凡铁匕首
        (
            "workbench.weapon.iron_dagger",
            "凡铁匕首",
            vec![("iron_ingot", 1), ("wood_handle", 1)],
            0.0,
            40,
            ("iron_dagger", 1),
            CraftCategory::Misc,
            vec![],
        ),
        // #62 石刃
        (
            "workbench.weapon.stone_knife",
            "石刃",
            vec![("stone_chunk", 1), ("wood_handle", 1)],
            0.0,
            20,
            ("stone_knife", 1),
            CraftCategory::Misc,
            vec![],
        ),
    ];

    for (id, name, mats, qi, t_sec, output, cat, unlock) in recipes {
        registry.register(CraftRecipe {
            id: RecipeId::new(id),
            category: cat,
            display_name: name.into(),
            materials: mats.into_iter().map(|(t, c)| (t.into(), c)).collect(),
            qi_cost: qi,
            time_ticks: t_sec * 20,
            output: (output.0.into(), output.1),
            requirements: CraftRequirements::default(),
            unlock_sources: unlock,
            station: WORKBENCH,
        })?;
    }
    Ok(())
}

/// 六、修炼辅材 #63-74
fn register_cultivation_support(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    let recipes: Vec<CategorizedRow> = vec![
        // #63 蒲团
        (
            "workbench.cultivation.meditation_mat",
            "蒲团",
            vec![("dried_grass", 6), ("rough_cloth", 2)],
            0.0,
            35,
            ("meditation_mat", 1),
            CraftCategory::Misc,
            vec![],
        ),
        // #64 灵气引导符 (📜)
        (
            "workbench.cultivation.qi_talisman",
            "灵气引导符",
            vec![("zhenfa_blank_array", 1), ("powder_dan_sha", 1)],
            2.0,
            50,
            ("qi_guide_talisman", 1),
            CraftCategory::Misc,
            scroll("scroll_workbench_qi_talisman"),
        ),
        // #65 经脉图拓片 (📜)
        (
            "workbench.cultivation.meridian_rub",
            "经脉图拓片",
            vec![("zhenfa_blank_array", 1), ("spirit_grass", 1)],
            1.0,
            35,
            ("meridian_rubbing", 1),
            CraftCategory::Misc,
            scroll("scroll_workbench_meridian_rub"),
        ),
        // #66 凝脉散预制包 (📜)
        (
            "workbench.cultivation.ningmai_prep",
            "凝脉散预制包",
            vec![("ning_mai_cao", 3), ("powder_dan_sha", 1), ("herb_vial", 1)],
            0.0,
            45,
            ("ningmai_prep_kit", 1),
            CraftCategory::Misc,
            scroll("scroll_workbench_ningmai_prep"),
        ),
        // #67 回元芷煎汤
        (
            "workbench.cultivation.huiyuan_soup",
            "回元芷煎汤",
            vec![("hui_yuan_zhi", 2), ("clay_pot", 1), ("spirit_charcoal", 1)],
            0.0,
            60,
            ("huiyuan_decoction", 1),
            CraftCategory::Misc,
            vec![],
        ),
        // #68 养经膏 (📜)
        (
            "workbench.cultivation.meridian_salve",
            "养经膏",
            vec![("yang_jing_tai", 2), ("rat_tail_oil", 1), ("herb_vial", 1)],
            1.0,
            50,
            ("meridian_salve", 1),
            CraftCategory::Misc,
            scroll("scroll_workbench_meridian_salve"),
        ),
        // #69 解蛊散 (DuguPotion, 📜)
        (
            "workbench.cultivation.anti_gu",
            "解蛊散",
            vec![("jie_gu_rui", 3), ("xiong_huang", 1)],
            1.0,
            45,
            ("anti_gu_powder", 1),
            CraftCategory::DuguPotion,
            scroll("scroll_workbench_anti_gu"),
        ),
        // #70 清浊散 (DuguPotion, 📜)
        (
            "workbench.cultivation.qingzhuo",
            "清浊散",
            vec![("qing_zhuo_cao", 2), ("powder_dan_sha", 1)],
            1.0,
            40,
            ("qingzhuo_powder", 1),
            CraftCategory::DuguPotion,
            scroll("scroll_workbench_qingzhuo"),
        ),
        // #71 安神茶
        (
            "workbench.cultivation.calming_tea",
            "安神茶",
            vec![("an_shen_guo", 2), ("clay_pot", 1)],
            0.0,
            30,
            ("calming_tea", 1),
            CraftCategory::Misc,
            vec![],
        ),
        // #73 灵石架
        (
            "workbench.cultivation.spirit_rack",
            "灵石架",
            vec![("wood_plank", 2), ("iron_ingot", 1)],
            0.0,
            45,
            ("spirit_stone_rack", 1),
            CraftCategory::Misc,
            vec![],
        ),
        // #74 止血绷带
        (
            "workbench.cultivation.bandage",
            "止血绷带",
            vec![("rough_cloth", 2)],
            0.0,
            10,
            ("bandage", 4),
            CraftCategory::Misc,
            vec![],
        ),
    ];

    for (id, name, mats, qi, t_sec, output, cat, unlock) in recipes {
        registry.register(CraftRecipe {
            id: RecipeId::new(id),
            category: cat,
            display_name: name.into(),
            materials: mats.into_iter().map(|(t, c)| (t.into(), c)).collect(),
            qi_cost: qi,
            time_ticks: t_sec * 20,
            output: (output.0.into(), output.1),
            requirements: CraftRequirements::default(),
            unlock_sources: unlock,
            station: WORKBENCH,
        })?;
    }
    Ok(())
}

/// 七、阵法基础 #75-82
fn register_array_basics(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    let recipes: Vec<CategorizedRow> = vec![
        // #75 阵法白纸
        (
            "workbench.array.blank_paper",
            "阵法白纸",
            vec![
                ("rough_cloth", 1),
                ("powder_dan_sha", 1),
                ("rat_tail_oil", 1),
            ],
            1.0,
            35,
            ("zhenfa_blank_array", 2),
            CraftCategory::Misc,
            vec![],
        ),
        // #76 阵旗·凡 (ZhenfaTrap, 📜)
        (
            "workbench.array.flag_basic",
            "阵旗·凡",
            vec![
                ("wood_handle", 1),
                ("rough_cloth", 1),
                ("powder_dan_sha", 1),
            ],
            2.0,
            50,
            ("array_flag_basic", 1),
            CraftCategory::ZhenfaTrap,
            scroll("scroll_workbench_array_flag"),
        ),
        // #77 阵眼·凡 (ZhenfaTrap, 📜)
        (
            "workbench.array.eye_basic",
            "阵眼·凡",
            vec![("ling_jing", 1), ("iron_ingot", 2)],
            3.0,
            65,
            ("array_eye_basic", 1),
            CraftCategory::ZhenfaTrap,
            scroll("scroll_workbench_array_eye"),
        ),
        // #78 预警绊线
        (
            "workbench.array.trip_wire",
            "预警绊线",
            vec![("spider_silk_cord", 2), ("iron_needle", 1)],
            0.0,
            20,
            ("trip_wire", 3),
            CraftCategory::ZhenfaTrap,
            vec![],
        ),
        // #79 欺天阵木桩 (ZhenfaTrap, 📜👤)
        (
            "workbench.array.decoy_stake",
            "欺天阵木桩",
            vec![("spirit_wood", 2), ("rough_cloth", 1), ("shu_gu", 1)],
            2.0,
            65,
            ("decoy_stake", 1),
            CraftCategory::ZhenfaTrap,
            vec![
                UnlockSource::Scroll {
                    item_template: "scroll_workbench_decoy_stake".into(),
                },
                UnlockSource::Mentor {
                    npc_archetype: "array_scribe".into(),
                },
            ],
        ),
        // #80 散真元珠 (ZhenfaTrap, 📜)：主动投掷/埋设散逸道具；破阵被动掉落物是 scattered_qi_pearl。
        (
            "workbench.array.scatter_bead",
            "散真元珠",
            vec![("ling_jing", 1), ("bone_meal_mat", 1)],
            2.0,
            35,
            ("qi_scatter_bead", 2),
            CraftCategory::ZhenfaTrap,
            scroll("scroll_workbench_scatter_bead"),
        ),
        // #81 聚灵阵基座 (ZhenfaTrap, 📜👤)：Lingju 凡阶唯一来源；旧 zhenfa_array_lingju 为 deprecated 旧配方。
        (
            "workbench.array.gather_base",
            "聚灵阵基座",
            vec![("ling_tie", 2), ("zhenfa_blank_array", 2), ("ling_jing", 1)],
            5.0,
            130,
            ("gather_array_base", 1),
            CraftCategory::ZhenfaTrap,
            vec![
                UnlockSource::Scroll {
                    item_template: "scroll_workbench_gather_base".into(),
                },
                UnlockSource::Mentor {
                    npc_archetype: "array_scribe".into(),
                },
            ],
        ),
        // #82 困兽圈
        (
            "workbench.array.beast_trap",
            "困兽圈",
            vec![("iron_ingot", 3), ("grass_rope", 2)],
            0.0,
            50,
            ("beast_trap", 1),
            CraftCategory::ZhenfaTrap,
            vec![],
        ),
        // #104 诱饵桩
        (
            "workbench.array.bait_stake",
            "诱饵桩",
            vec![
                ("wood_handle", 1),
                ("dried_grass", 3),
                ("rough_cloth", 1),
                ("raw_beast_hide", 1),
            ],
            0.0,
            35,
            ("bait_stake", 1),
            CraftCategory::ZhenfaTrap,
            vec![],
        ),
    ];

    for (id, name, mats, qi, t_sec, output, cat, unlock) in recipes {
        registry.register(CraftRecipe {
            id: RecipeId::new(id),
            category: cat,
            display_name: name.into(),
            materials: mats.into_iter().map(|(t, c)| (t.into(), c)).collect(),
            qi_cost: qi,
            time_ticks: t_sec * 20,
            output: (output.0.into(), output.1),
            requirements: CraftRequirements::default(),
            unlock_sources: unlock,
            station: WORKBENCH,
        })?;
    }
    Ok(())
}

/// 八、经济与交易 #83-90
fn register_economy(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    let recipes: Vec<CategorizedRow> = vec![
        // #84 骨币匣 (Container)
        (
            "workbench.economy.coin_box",
            "骨币匣",
            vec![("wood_plank", 4), ("iron_ingot", 1), ("powder_dan_sha", 1)],
            0.0,
            55,
            ("coin_box", 1),
            CraftCategory::Container,
            vec![],
        ),
        // #87 伪装包裹 (TuikeSkin)
        (
            "workbench.economy.disguise_wrap",
            "伪装包裹",
            vec![("rough_cloth", 2), ("hui_jin_tai", 1)],
            0.0,
            25,
            ("disguise_wrap", 2),
            CraftCategory::TuikeSkin,
            vec![],
        ),
        // #90 灵龛修补料 (📜)
        (
            "workbench.economy.niche_repair",
            "灵龛修补料",
            vec![("stone_chunk", 3), ("ling_tie", 1)],
            2.0,
            65,
            ("niche_repair_kit", 1),
            CraftCategory::Misc,
            scroll("scroll_workbench_niche_repair"),
        ),
    ];

    for (id, name, mats, qi, t_sec, output, cat, unlock) in recipes {
        registry.register(CraftRecipe {
            id: RecipeId::new(id),
            category: cat,
            display_name: name.into(),
            materials: mats.into_iter().map(|(t, c)| (t.into(), c)).collect(),
            qi_cost: qi,
            time_ticks: t_sec * 20,
            output: (output.0.into(), output.1),
            requirements: CraftRequirements::default(),
            unlock_sources: unlock,
            station: WORKBENCH,
        })?;
    }
    Ok(())
}

/// 九、住所与防御 #91-98
fn register_shelter(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    use super::events::InsightTrigger;

    let recipes: Vec<CategorizedRow> = vec![
        // #91 火把
        (
            "workbench.shelter.torch",
            "火把",
            vec![
                ("wood_handle", 1),
                ("spirit_charcoal", 1),
                ("rat_tail_oil", 1),
            ],
            0.0,
            10,
            ("torch_item", 4),
            CraftCategory::Misc,
            vec![],
        ),
        // #92 灯笼 (📜)
        (
            "workbench.shelter.lantern",
            "灯笼",
            vec![("iron_ingot", 2), ("spirit_charcoal", 1), ("ling_jing", 1)],
            1.0,
            45,
            ("lantern_item", 1),
            CraftCategory::Misc,
            scroll("scroll_workbench_lantern"),
        ),
        // #93 门闩
        (
            "workbench.shelter.door_bolt",
            "门闩",
            vec![("iron_ingot", 3), ("wood_plank", 2)],
            0.0,
            45,
            ("door_bolt", 1),
            CraftCategory::Misc,
            vec![],
        ),
        // #94 伪装网 (TuikeSkin)
        (
            "workbench.shelter.camo_net",
            "伪装网",
            vec![("rough_cloth", 3), ("spirit_grass", 2)],
            0.0,
            55,
            ("camouflage_net", 1),
            CraftCategory::TuikeSkin,
            vec![],
        ),
        // #95 简易床铺
        (
            "workbench.shelter.simple_bed",
            "简易床铺",
            vec![("wood_plank", 4), ("dried_grass", 4), ("rough_cloth", 2)],
            0.0,
            60,
            ("simple_bed", 1),
            CraftCategory::Misc,
            vec![],
        ),
        // #96 防潮地基
        (
            "workbench.shelter.moisture_base",
            "防潮地基",
            vec![("stone_chunk", 4), ("hui_jin_tai", 2)],
            0.0,
            35,
            ("moisture_base", 4),
            CraftCategory::Misc,
            vec![],
        ),
        // #97 窗栅
        (
            "workbench.shelter.window_grate",
            "窗栅",
            vec![("iron_ingot", 4)],
            0.0,
            35,
            ("window_grate", 2),
            CraftCategory::Misc,
            vec![],
        ),
        // #98 灵龛基座 (📜👤💡)
        (
            "workbench.shelter.niche_base",
            "灵龛基座",
            vec![
                ("spirit_niche_stone", 1),
                ("ling_tie", 2),
                ("wood_plank", 4),
            ],
            5.0,
            180,
            ("niche_base", 1),
            CraftCategory::Misc,
            vec![
                UnlockSource::Scroll {
                    item_template: "scroll_workbench_niche_base".into(),
                },
                UnlockSource::Mentor {
                    npc_archetype: "hermit_builder".into(),
                },
                UnlockSource::Insight {
                    trigger: InsightTrigger::Breakthrough,
                },
            ],
        ),
    ];

    for (id, name, mats, qi, t_sec, output, cat, unlock) in recipes {
        registry.register(CraftRecipe {
            id: RecipeId::new(id),
            category: cat,
            display_name: name.into(),
            materials: mats.into_iter().map(|(t, c)| (t.into(), c)).collect(),
            qi_cost: qi,
            time_ticks: t_sec * 20,
            output: (output.0.into(), output.1),
            requirements: CraftRequirements::default(),
            unlock_sources: unlock,
            station: WORKBENCH,
        })?;
    }
    Ok(())
}

/// 十、炼丹/炼器预备 #99-100
fn register_alchemy_forge_prep(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    let recipes: Vec<ArmorRow> = vec![
        // #99 凡铁炉组件
        (
            "workbench.prep.furnace_kit",
            "凡铁炉组件",
            vec![
                ("iron_ingot", 6),
                ("spirit_charcoal", 2),
                ("stone_chunk", 4),
            ],
            120,
            "furnace_fantie",
        ),
        // #100 锻造台组件
        (
            "workbench.prep.forge_station",
            "锻造台组件",
            vec![("iron_ingot", 4), ("spirit_wood", 2), ("stone_chunk", 6)],
            100,
            "fan_iron_anvil",
        ),
    ];

    for (id, name, mats, t_sec, output) in recipes {
        registry.register(CraftRecipe {
            id: RecipeId::new(id),
            category: CraftCategory::Tool,
            display_name: name.into(),
            materials: mats.into_iter().map(|(t, c)| (t.into(), c)).collect(),
            qi_cost: 0.0,
            time_ticks: t_sec * 20,
            output: (output.into(), 1),
            requirements: CraftRequirements::default(),
            unlock_sources: vec![],
            station: WORKBENCH,
        })?;
    }
    Ok(())
}

/// plan-coffin-tiers-v1 P4 §P4 — 三档延寿棺 workbench 配方（#101-103）。
/// 凡木棺 mundane 保留在 `coffin::register_craft_recipes`（手搓 station=None）。
fn register_coffin_tiers(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    // #101 寒玉棺 ×0.7 — Scroll 解锁
    registry.register(CraftRecipe {
        id: RecipeId::new("coffin.jade_coffin"),
        category: CraftCategory::Misc,
        display_name: "寒玉棺".into(),
        materials: vec![
            ("ling_mu_ban".into(), 4),
            ("yu_sui".into(), 3),
            ("xue_po_lian".into(), 2),
        ],
        qi_cost: 2.0,
        time_ticks: 120 * 20,
        output: ("jade_coffin".into(), 1),
        requirements: CraftRequirements::default(),
        unlock_sources: vec![UnlockSource::Scroll {
            item_template: "scroll_jade_coffin".into(),
        }],
        station: WORKBENCH,
    })?;

    // #102 玄石棺 ×0.5 — Scroll + 地师流 Mentor（array_scribe）
    // P4 修正：zhen_shi_zhong 无玩家可达来源（仅被消耗，不产出），换为蜘蛛掉落 zhen_shi_chu×2（5% 稀有掉落）。
    registry.register(CraftRecipe {
        id: RecipeId::new("coffin.stone_coffin"),
        category: CraftCategory::Misc,
        display_name: "玄石棺".into(),
        materials: vec![
            ("xuan_iron".into(), 4),
            ("zhen_shi_chu".into(), 2),
            ("wu_yao".into(), 2),
        ],
        qi_cost: 4.0,
        time_ticks: 150 * 20,
        output: ("stone_coffin".into(), 1),
        requirements: CraftRequirements::default(),
        unlock_sources: vec![
            UnlockSource::Scroll {
                item_template: "scroll_stone_coffin".into(),
            },
            UnlockSource::Mentor {
                npc_archetype: "array_scribe".into(),
            },
        ],
        station: WORKBENCH,
    })?;

    // #103 青铜棺 ×0.3 — Scroll + 炼器流 Mentor（hermit_builder）+ 引气境下限
    // P4 修正：ling_mu_jing 生产链未实装（spiritwood §9）；zhen_shi_gao 无玩家可达来源（仅被消耗）。
    // 换为：ling_mu_jing×2 → ling_mu_ban×3（锻造产出，灵木主题保持）；
    //       zhen_shi_gao×1 → zhen_shi_chu×2（蜘蛛 5% 掉落，阵石主题保持）。
    registry.register(CraftRecipe {
        id: RecipeId::new("coffin.bronze_coffin"),
        category: CraftCategory::Misc,
        display_name: "青铜棺".into(),
        materials: vec![
            ("xuan_iron".into(), 3),
            ("ling_mu_ban".into(), 3),
            ("gu_tong_pian".into(), 4),
            ("zhen_shi_chu".into(), 2),
        ],
        qi_cost: 6.0,
        time_ticks: 180 * 20,
        output: ("bronze_coffin".into(), 1),
        requirements: CraftRequirements {
            realm_min: Some(Realm::Induce),
            ..CraftRequirements::default()
        },
        unlock_sources: vec![
            UnlockSource::Scroll {
                item_template: "scroll_bronze_coffin".into(),
            },
            UnlockSource::Mentor {
                npc_archetype: "hermit_builder".into(),
            },
        ],
        station: WORKBENCH,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_workbench_recipes_succeeds() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        // 89 workbench/coffin recipes (86 workbench.* + 3 coffin tiers P4) + 1 workbench self recipe = 90
        assert_eq!(
            registry.len(),
            90,
            "expected 90 recipes (89 workbench/coffin + 1 self craft), got {}",
            registry.len()
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
            if recipe.id.as_str() == "craft.tool.workbench" {
                continue; // self recipe is handcraft
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
        // plan-coffin-tiers-v1 P4: 三档延寿棺配方迁入 workbench_recipes 注册，
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
        assert_eq!(
            workbench_count, 86,
            "must have exactly 86 workbench.* recipes"
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
    fn all_88_workbench_and_coffin_recipes_individual_pin() {
        let mut registry = CraftRegistry::new();
        register_workbench_recipes(&mut registry).unwrap();
        let item_registry = crate::inventory::load_item_registry().expect("item registry loads");

        let mut workbench_count = 0u32;
        for recipe in registry.iter() {
            if recipe.id.as_str() == "craft.tool.workbench" {
                assert_eq!(
                    recipe.station, None,
                    "[{}] self recipe must be handcraft",
                    recipe.id
                );
                continue;
            }
            workbench_count += 1;

            assert_eq!(
                recipe.station,
                Some(CraftStationKind::Workbench),
                "[{}] station must be Workbench",
                recipe.id
            );
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
        assert_eq!(
            workbench_count, 89,
            "expected exactly 89 workbench/coffin recipes (86 workbench.* + 3 coffin tiers P4)"
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
        // plan-coffin-tiers-v1 P4: +3 coffin 配方（jade qi=2 / stone qi=4 / bronze qi=6）；
        // plan-economy-zombie-cleanup-v1 P2 删除 trade_puppet_frame 后 -1 条正 qi 配方。
        assert_eq!(
            qi_recipe_count, 20,
            "expected exactly 20 recipes with qi_cost > 0 (17 current + 3 coffin tiers P4)"
        );
    }
}
