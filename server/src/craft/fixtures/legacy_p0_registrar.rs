//! P0 迁移前真实 Rust registrar 的 test-only 固化副本。
//!
//! 来源固定为 commit `6a6a262cecf9126c102378bb4b12bdcd3ba450eb`：
//! - `server/src/craft/mod.rs::register_examples`
//! - `server/src/craft/workbench_recipes.rs::register_workbench_recipes`
//!
//! 生产构建不编译本文件。它只让回归测试从旧 registrar 程序化重建 95 条
//! canonical 配方，避免 TOML 与 JSON fixture 同步误改后仍然假绿。

use crate::craft::events::InsightTrigger;
use crate::craft::recipe::{
    CraftCategory, CraftRecipe, CraftRequirements, CraftStationKind, RecipeId, UnlockSource,
};
use crate::craft::registry::{CraftRegistry, RegistryError};
use crate::cultivation::components::{ColorKind, Realm};

/// P1 验收基线：注册 5 个示例配方覆盖全 6 类（除 Misc 外）。
///
/// 命名约定：`craft.example.<物品>.<档位>` —— `craft.example.*` 命名空间
/// 标识"plan-craft-v1 自带的示例"，流派 plan vN+1 接入时用各自命名空间
/// （`dugu.*` / `tuike.*` / `zhenfa.*` / `tools.*`）。
///
/// 5 个示例分布（plan §2 UI Mockup / plan §1 P1 验收清单）：
///   1. AnqiCarrier — 蚀针（凡铁）
///   2. DuguPotion  — 毒源煎汤（凡毒）
///   3. TuikeSkin   — 伪灵皮（轻档）
///   4. ZhenfaTrap  — 真元诡雷（凡铁）
///   5. Tool        — 采药刀（凡铁）— §5 决策门 #5 凡器破例收录
pub fn register_examples(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    // 1. 蚀针（凡铁）— AnqiCarrier
    registry.register(CraftRecipe {
        id: RecipeId::new("craft.example.eclipse_needle.iron"),
        category: CraftCategory::AnqiCarrier,
        display_name: "蚀针（凡铁档）".into(),
        materials: vec![
            ("iron_needle".into(), 3),
            ("chi_sui_cao".into(), 1), // 赤髓草（plan-botany / 现有 herbalism 词条）
        ],
        qi_cost: 8.0,
        time_ticks: 3 * 60 * 20, // 3 min in-game
        output: ("eclipse_needle_iron".into(), 3),
        requirements: CraftRequirements {
            realm_min: None, // 不强制 — worldview §五:537 流派由组合涌现
            qi_color_min: Some((ColorKind::Insidious, 0.05)),
            skill_lv_min: None,
        },
        unlock_sources: vec![],
        station: None,
    })?;

    // 2. 毒源煎汤（凡毒）— DuguPotion
    registry.register(CraftRecipe {
        id: RecipeId::new("craft.example.poison_decoction.fan"),
        category: CraftCategory::DuguPotion,
        display_name: "毒源煎汤（凡毒）".into(),
        materials: vec![
            ("shao_hou_man".into(), 2), // 烧候蔓
            ("clay_pot".into(), 1),
        ],
        qi_cost: 4.0,
        time_ticks: 90 * 20, // 1.5 min in-game
        output: ("poison_decoction_fan".into(), 1),
        requirements: CraftRequirements::default(),
        unlock_sources: vec![],
        station: None,
    })?;

    // 3. 伪灵皮（轻档）— TuikeSkin
    registry.register(CraftRecipe {
        id: RecipeId::new("craft.example.fake_skin.light"),
        category: CraftCategory::TuikeSkin,
        display_name: "伪灵皮（轻档）".into(),
        materials: vec![
            ("rabbit_pelt".into(), 4),
            ("yu_yi_zhi".into(), 1), // 鱼衣脂
        ],
        qi_cost: 2.0,
        time_ticks: 2 * 60 * 20, // 2 min in-game
        output: ("fake_skin_light".into(), 1),
        requirements: CraftRequirements {
            realm_min: Some(Realm::Induce), // 引气起步 — 替尸需要灵气过渡
            qi_color_min: None,
            skill_lv_min: None,
        },
        unlock_sources: vec![
            UnlockSource::Scroll {
                item_template: "scroll_fake_skin_light".into(),
            },
            UnlockSource::Insight {
                trigger: InsightTrigger::NearDeath,
            },
        ],
        station: None,
    })?;

    // 4. 真元诡雷（凡铁）— ZhenfaTrap
    registry.register(CraftRecipe {
        id: RecipeId::new("craft.example.zhenfa_trap.iron"),
        category: CraftCategory::ZhenfaTrap,
        display_name: "真元诡雷（凡铁芯）".into(),
        materials: vec![
            ("iron_ingot".into(), 2),
            ("zhenfa_blank_array".into(), 1), // 阵法白纸
        ],
        qi_cost: 6.0,
        time_ticks: 4 * 60 * 20, // 4 min in-game
        output: ("zhenfa_trap_iron".into(), 1),
        requirements: CraftRequirements {
            realm_min: Some(Realm::Induce),
            qi_color_min: None,
            skill_lv_min: None,
        },
        unlock_sources: vec![
            UnlockSource::Scroll {
                item_template: "scroll_zhenfa_trap_iron".into(),
            },
            UnlockSource::Mentor {
                npc_archetype: "array_scribe".into(),
            },
        ],
        station: None,
    })?;

    // 5. 采药刀（凡铁）— Tool（§5 决策门 #5 凡器破例收录手搓 tab）
    registry.register(CraftRecipe {
        id: RecipeId::new("craft.example.herb_knife.iron"),
        category: CraftCategory::Tool,
        display_name: "采药刀（凡铁）".into(),
        materials: vec![("iron_ingot".into(), 1), ("wood_handle".into(), 1)],
        qi_cost: 0.0,        // 凡器不投入真元
        time_ticks: 30 * 20, // 30 sec in-game
        output: ("herb_knife_iron".into(), 1),
        requirements: CraftRequirements::default(),
        unlock_sources: vec![UnlockSource::Scroll {
            item_template: "scroll_herb_knife_iron".into(),
        }],
        station: None,
    })?;

    Ok(())
}

const WORKBENCH: Option<CraftStationKind> = Some(CraftStationKind::Workbench);

/// 石器是最原始工具（敲石成器），改为**手搓配方**（station None），让玩家在手搓台直接做出来，
/// 不必先有制作台。其余 workbench 配方仍需制作台。用 recipe id 精确匹配这 3 个石器。
const HANDCRAFT_STONE_TOOLS: [&str; 3] = [
    "workbench.tool.stone_pickaxe",
    "workbench.tool.stone_axe",
    "workbench.weapon.stone_knife",
];

fn station_for(recipe_id: &str) -> Option<CraftStationKind> {
    if HANDCRAFT_STONE_TOOLS.contains(&recipe_id) {
        None
    } else {
        WORKBENCH
    }
}

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
/// 数据资产共包含 90 条 workbench/coffin 配方和 1 条制作台自身配方。
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
            station: station_for(id),
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
            station: station_for(id),
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
            station: station_for(id),
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
            station: station_for(id),
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
            station: station_for(id),
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
            station: station_for(id),
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
            station: station_for(id),
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
            station: station_for(id),
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
            station: station_for(id),
        })?;
    }
    Ok(())
}

/// 九、住所与防御 #91-98
fn register_shelter(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
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
            station: station_for(id),
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
            station: station_for(id),
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
