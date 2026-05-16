//! plan-craft-v1 — 通用手搓系统（P0 决策门收口 + P1 server 主体）。
//!
//! 模块结构：
//!   * [`recipe`] — `CraftRecipe` / `CraftCategory` / `CraftRequirements` /
//!     `RecipeId` / `UnlockSource`（数据契约）
//!   * [`registry`] — `CraftRegistry` resource（全局注册表，6 类分组）
//!   * [`session`] — `CraftSession` component + `start/cancel/finalize/tick`
//!     （守恒律走 `qi_physics::ledger::Crafting`）
//!   * [`unlock`] — `RecipeUnlockState` resource + 三渠道（残卷/师承/顿悟）
//!   * [`events`] — `CraftStartedEvent` / `CraftCompletedEvent` /
//!     `CraftFailedEvent` / `RecipeUnlockedEvent` / `InsightTrigger`
//!
//! 跨 plan 钩子（P3 接入，本 plan P0+P1 不动）：
//!   * 流派 plan（dugu-v2 / tuike-v2 / zhenfa-v2 / tools-v1）→ register 自家配方
//!   * agent narration runtime（`craft_runtime.ts`）→ 4 类叙事
//!   * client `CraftTabScreen` UI → P2
//!   * `unlock_via_scroll` ItemUse hook → inventory 物品使用层
//!   * `unlock_via_mentor` NPC dialog 选项 → social plan dialog 引擎
//!   * `unlock_via_insight` BreakthroughEvent / DefeatStrongerEvent 监听 → cultivation/combat plan
//!
//! P0 决策门收口（详见 `docs/finished_plans/plan-craft-v1.md` §5）：
//!   * #1 = A：保留 6 类（AnqiCarrier / DuguPotion / TuikeSkin / ZhenfaTrap / Tool / Misc）
//!   * #2 = A：UI 排序按类别分组 + 类别内字母（`registry::grouped_for_ui`）
//!   * #3 = B：取消任务返还材料 70%（`session::CANCEL_REFUND_RATIO`），qi 不退
//!   * #4 = A：玩家死亡 → cancel + PlayerDied reason
//!   * #5 = A：手搓 tab 不收 vanilla，凡器破例（5 个示例之一）
//!   * #6 = B：requirements 软 gate（UI 灰显 + 服务端硬校验防作弊）

pub mod events;
pub mod recipe;
pub mod registry;
pub mod session;
pub mod unlock;

use valence::prelude::App;

#[allow(unused_imports)]
pub use events::{
    CraftCancelIntent, CraftCompletedEvent, CraftFailedEvent, CraftFailureReason, CraftStartIntent,
    CraftStartedEvent, CraftUnlockIntent, InsightTrigger, RecipeUnlockedEvent, UnlockEventSource,
};
#[allow(unused_imports)]
pub use recipe::{
    CraftCategory, CraftRecipe, CraftRequirements, RecipeId, RecipeValidationError, UnlockSource,
};
#[allow(unused_imports)]
pub use registry::{CraftRegistry, RegistryError};
#[allow(unused_imports)]
pub use session::{
    cancel_craft, consume_materials_from_inventory, count_template_in_inventory, finalize_craft,
    start_craft, tick_session, CancelCraftOutcome, CraftSession, FinalizeCraftOutcome,
    MaterialDeficit, StartCraftDeps, StartCraftError, StartCraftRequest, StartCraftSuccess,
    CANCEL_REFUND_RATIO, MAX_CRAFT_QUANTITY,
};
#[allow(unused_imports)]
pub use unlock::{
    unlock_via_insight, unlock_via_mentor, unlock_via_scroll, RecipeUnlockState, UnlockOutcome,
};

use crate::cultivation::components::{ColorKind, Realm};

/// 注册 craft 子系统到主 App。
///
/// 当前 P0+P1：
///   * 注册 5 个示例配方到 `CraftRegistry`（流派 plan 接入前的 P1 验收基线）
///   * 注册 4 类事件
///   * 注册 `CraftRegistry` / `RecipeUnlockState` resources
///
/// P2/P3 阶段补：UI tab + agent narration + 三渠道 hook（接 inventory ItemUse / social
/// dialog / cultivation BreakthroughEvent）。
pub fn register(app: &mut App) {
    tracing::info!("[bong][craft] registering craft subsystem (plan-craft-v1 P0+P1)");

    let mut registry = CraftRegistry::new();
    register_examples(&mut registry).unwrap_or_else(|err| {
        panic!("[bong][craft] failed to register example recipes: {err}");
    });
    register_anqi_v2_recipes(&mut registry).unwrap_or_else(|err| {
        panic!("[bong][craft] failed to register anqi-v2 recipes: {err}");
    });
    register_zhenfa_v2_recipes(&mut registry).unwrap_or_else(|err| {
        panic!("[bong][craft] failed to register zhenfa-v2 recipes: {err}");
    });
    register_zhenfa_content_recipes(&mut registry).unwrap_or_else(|err| {
        panic!("[bong][craft] failed to register zhenfa-content-v1 recipes: {err}");
    });
    register_tuike_v2_recipes(&mut registry).unwrap_or_else(|err| {
        panic!("[bong][craft] failed to register tuike-v2 recipes: {err}");
    });
    crate::cultivation::poison_trait::register_craft_recipes(&mut registry).unwrap_or_else(|err| {
        panic!("[bong][craft] failed to register poison-trait recipes: {err}");
    });
    crate::armor::mundane::register_mundane_armor_recipes(&mut registry).unwrap_or_else(|err| {
        panic!("[bong][craft] failed to register armor-visual-v1 recipes: {err}");
    });
    register_gathering_tool_recipes(&mut registry).unwrap_or_else(|err| {
        panic!("[bong][craft] failed to register gathering-ux-v1 recipes: {err}");
    });
    register_basic_processing_recipes(&mut registry).unwrap_or_else(|err| {
        panic!("[bong][craft] failed to register basic-processing recipes: {err}");
    });
    crate::coffin::register_craft_recipes(&mut registry).unwrap_or_else(|err| {
        panic!("[bong][craft] failed to register coffin-v1 recipes: {err}");
    });
    tracing::info!("[bong][craft] registered {} recipe(s)", registry.len());

    app.insert_resource(registry);
    app.insert_resource(RecipeUnlockState::new());

    app.add_event::<CraftStartedEvent>();
    app.add_event::<CraftCompletedEvent>();
    app.add_event::<CraftFailedEvent>();
    app.add_event::<RecipeUnlockedEvent>();
    // P2 client → server intents（被 `network/craft_emit::apply_craft_intents` 系统消费）
    app.add_event::<CraftStartIntent>();
    app.add_event::<CraftCancelIntent>();
    // P3 三渠道解锁 intent —— 由各 source plan emit，被
    // `network/craft_emit::apply_unlock_intents` 系统消费
    app.add_event::<CraftUnlockIntent>();
}

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
    })?;

    Ok(())
}

/// plan-anqi-v2 §3：6 档暗器载体 + 3 个容器配方。
pub fn register_anqi_v2_recipes(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    fn scroll(id: &str) -> Vec<UnlockSource> {
        vec![UnlockSource::Scroll {
            item_template: format!("scroll_{id}"),
        }]
    }

    let specs = [
        (
            "anqi.carrier.bone_chip",
            CraftCategory::AnqiCarrier,
            "残骨",
            vec![("bone".to_string(), 1)],
            1.0,
            60 * 20,
            ("anqi_bone_chip".to_string(), 3),
            CraftRequirements::default(),
        ),
        (
            "anqi.carrier.mutant_bone",
            CraftCategory::AnqiCarrier,
            "异变兽骨",
            vec![
                ("mutant_beast_core".to_string(), 1),
                ("bone".to_string(), 3),
            ],
            8.0,
            8 * 60 * 20,
            ("anqi_yibian_shougu".to_string(), 1),
            CraftRequirements::default(),
        ),
        (
            "anqi.carrier.lingmu_quiver",
            CraftCategory::AnqiCarrier,
            "灵木编制箭",
            vec![("ling_mu".to_string(), 3), ("solid_qi_dye".to_string(), 1)],
            12.0,
            12 * 60 * 20,
            ("anqi_lingmu_arrow".to_string(), 5),
            CraftRequirements {
                realm_min: Some(Realm::Induce),
                qi_color_min: None,
                skill_lv_min: None,
            },
        ),
        (
            "anqi.carrier.dyed_bone",
            CraftCategory::AnqiCarrier,
            "凝实色染色骨",
            vec![
                ("anqi_yibian_shougu".to_string(), 1),
                ("solid_qi_dye".to_string(), 3),
                ("herb_mixed".to_string(), 5),
            ],
            30.0,
            30 * 60 * 20,
            ("anqi_dyed_bone".to_string(), 1),
            CraftRequirements {
                realm_min: Some(Realm::Condense),
                qi_color_min: Some((ColorKind::Solid, 0.2)),
                skill_lv_min: None,
            },
        ),
        (
            "anqi.carrier.fenglinghe_bone",
            CraftCategory::AnqiCarrier,
            "封灵匣骨",
            vec![
                ("anqi_dyed_bone".to_string(), 1),
                ("ancient_bone_shard".to_string(), 1),
            ],
            60.0,
            60 * 60 * 20,
            ("anqi_fenglinghe_bone".to_string(), 1),
            CraftRequirements {
                realm_min: Some(Realm::Solidify),
                qi_color_min: Some((ColorKind::Solid, 0.3)),
                skill_lv_min: None,
            },
        ),
        (
            "anqi.carrier.shanggu_bone",
            CraftCategory::AnqiCarrier,
            "上古残骨",
            vec![("ancient_bone_shard".to_string(), 3)],
            120.0,
            120 * 60 * 20,
            ("anqi_shanggu_bone".to_string(), 1),
            CraftRequirements {
                realm_min: Some(Realm::Void),
                qi_color_min: Some((ColorKind::Solid, 0.4)),
                skill_lv_min: None,
            },
        ),
        (
            "anqi.container.quiver",
            CraftCategory::Container,
            "暗器箭袋",
            vec![
                ("beast_leather".to_string(), 3),
                ("solid_qi_dye".to_string(), 1),
            ],
            5.0,
            5 * 60 * 20,
            ("anqi_container_quiver".to_string(), 1),
            CraftRequirements::default(),
        ),
        (
            "anqi.container.pocket_pouch",
            CraftCategory::Container,
            "暗器裤袋",
            vec![("beast_leather".to_string(), 1), ("ling_mu".to_string(), 1)],
            2.0,
            2 * 60 * 20,
            ("anqi_container_pocket_pouch".to_string(), 1),
            CraftRequirements::default(),
        ),
        (
            "anqi.container.fenglinghe",
            CraftCategory::Container,
            "封灵匣",
            vec![
                ("mutant_beast_bone".to_string(), 3),
                ("ling_mu".to_string(), 5),
            ],
            30.0,
            30 * 60 * 20,
            ("anqi_container_fenglinghe".to_string(), 1),
            CraftRequirements {
                realm_min: Some(Realm::Condense),
                qi_color_min: None,
                skill_lv_min: None,
            },
        ),
    ];

    for (id, category, display_name, materials, qi_cost, time_ticks, output, requirements) in specs
    {
        registry.register(CraftRecipe {
            id: RecipeId::new(id),
            category,
            display_name: display_name.into(),
            materials,
            qi_cost,
            time_ticks,
            output,
            requirements,
            unlock_sources: scroll(id),
        })?;
    }

    Ok(())
}

/// plan-zhenfa-v2 §3：护龛 / 聚灵 / 欺天 / 幻阵预埋件 + 两档阵旗。
pub fn register_zhenfa_v2_recipes(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    let mentor = |npc: &str| {
        vec![UnlockSource::Mentor {
            npc_archetype: npc.to_string(),
        }]
    };
    let scroll = |id: &str| {
        vec![UnlockSource::Scroll {
            item_template: format!("scroll_{id}"),
        }]
    };

    let specs = [
        (
            "zhenfa.array.ward",
            CraftCategory::ZhenfaTrap,
            "护龛阵预埋件",
            vec![
                ("ancient_soil_shard".to_string(), 5),
                ("array_eye_block".to_string(), 1),
            ],
            15.0,
            15 * 60 * 20,
            ("zhenfa_array_ward".to_string(), 1),
            CraftRequirements::default(),
            scroll("zhenfa_array_ward"),
        ),
        (
            "zhenfa.array.lingju",
            CraftCategory::ZhenfaTrap,
            "聚灵阵预埋件",
            vec![
                ("ancient_soil_shard".to_string(), 9),
                ("array_eye_block".to_string(), 3),
                ("lingquan_water".to_string(), 3),
            ],
            30.0,
            30 * 60 * 20,
            ("zhenfa_array_lingju".to_string(), 1),
            CraftRequirements::default(),
            mentor("array_scribe"),
        ),
        (
            "zhenfa.array.deceive",
            CraftCategory::ZhenfaTrap,
            "欺天阵预埋件",
            vec![
                ("ancient_bone_shard".to_string(), 5),
                ("array_eye_block".to_string(), 1),
                ("sealed_void_qi".to_string(), 1),
            ],
            80.0,
            60 * 60 * 20,
            ("zhenfa_array_deceive".to_string(), 1),
            CraftRequirements {
                realm_min: Some(Realm::Solidify),
                qi_color_min: Some((ColorKind::Intricate, 0.2)),
                skill_lv_min: None,
            },
            vec![UnlockSource::Insight {
                trigger: InsightTrigger::Breakthrough,
            }],
        ),
        (
            "zhenfa.array.illusion",
            CraftCategory::ZhenfaTrap,
            "幻阵符",
            vec![
                ("intricate_qi_dye".to_string(), 3),
                ("ling_mu".to_string(), 2),
            ],
            8.0,
            5 * 60 * 20,
            ("zhenfa_array_illusion_talisman".to_string(), 3),
            CraftRequirements {
                realm_min: None,
                qi_color_min: Some((ColorKind::Intricate, 0.1)),
                skill_lv_min: None,
            },
            mentor("array_scribe"),
        ),
        (
            "zhenfa.flag.basic",
            CraftCategory::Tool,
            "基础阵旗",
            vec![
                ("ling_mu".to_string(), 2),
                ("beast_leather".to_string(), 1),
                ("intricate_qi_dye".to_string(), 1),
            ],
            4.0,
            8 * 60 * 20,
            ("array_flag".to_string(), 1),
            CraftRequirements::default(),
            scroll("zhenfa_flag_basic"),
        ),
        (
            "zhenfa.flag.deceive",
            CraftCategory::Tool,
            "欺天阵旗",
            vec![
                ("ancient_bone_shard".to_string(), 3),
                ("sealed_void_qi".to_string(), 1),
            ],
            50.0,
            30 * 60 * 20,
            ("array_flag_deceive".to_string(), 1),
            CraftRequirements {
                realm_min: Some(Realm::Solidify),
                qi_color_min: Some((ColorKind::Intricate, 0.2)),
                skill_lv_min: None,
            },
            vec![UnlockSource::Insight {
                trigger: InsightTrigger::Breakthrough,
            }],
        ),
    ];

    for (
        id,
        category,
        display_name,
        materials,
        qi_cost,
        time_ticks,
        output,
        requirements,
        unlock_sources,
    ) in specs
    {
        registry.register(CraftRecipe {
            id: RecipeId::new(id),
            category,
            display_name: display_name.into(),
            materials,
            qi_cost,
            time_ticks,
            output,
            requirements,
            unlock_sources,
        })?;
    }

    Ok(())
}

/// plan-zhenfa-content-v1：三种低阶凡阵符，走通用 ZhenfaTrap 手搓类别。
pub fn register_zhenfa_content_recipes(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    let scroll = |template: &str| {
        vec![UnlockSource::Scroll {
            item_template: template.to_string(),
        }]
    };

    let specs = [
        (
            "zhenfa.content.warning_trap",
            "警示符",
            vec![
                ("copper_ore".to_string(), 1),
                ("ancient_soil_shard".to_string(), 2),
            ],
            2.0,
            30 * 20,
            ("warning_trap".to_string(), 3),
            vec![UnlockSource::Mentor {
                npc_archetype: "default_known".to_string(),
            }],
        ),
        (
            "zhenfa.content.blast_trap",
            "爆阵符",
            vec![
                ("ancient_bone_shard".to_string(), 1),
                ("zhen_shi_chu".to_string(), 1),
                ("copper_ore".to_string(), 2),
            ],
            15.0,
            60 * 20,
            ("blast_trap".to_string(), 1),
            scroll("scroll_blast_trap_talisman"),
        ),
        (
            "zhenfa.content.slow_trap",
            "缓阵符",
            vec![
                ("spirit_grass".to_string(), 1),
                ("copper_ore".to_string(), 1),
                ("ancient_soil_shard".to_string(), 1),
            ],
            8.0,
            45 * 20,
            ("slow_trap".to_string(), 2),
            scroll("scroll_slow_trap_talisman"),
        ),
    ];

    for (id, display_name, materials, qi_cost, time_ticks, output, unlock_sources) in specs {
        registry.register(CraftRecipe {
            id: RecipeId::new(id),
            category: CraftCategory::ZhenfaTrap,
            display_name: display_name.into(),
            materials,
            qi_cost,
            time_ticks,
            output,
            requirements: CraftRequirements::default(),
            unlock_sources,
        })?;
    }

    Ok(())
}

/// plan-tuike-v2：4 档伪皮制作走通用手搓 TuikeSkin 类目。
pub fn register_tuike_v2_recipes(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    use crate::combat::tuike_v2::state::{
        FALSE_SKIN_ANCIENT_ITEM_ID, FALSE_SKIN_HEAVY_ITEM_ID, FALSE_SKIN_LIGHT_ITEM_ID,
        FALSE_SKIN_MID_ITEM_ID,
    };

    fn scroll(id: &str) -> Vec<UnlockSource> {
        vec![UnlockSource::Scroll {
            item_template: format!("scroll_{id}"),
        }]
    }

    let specs = [
        (
            "tuike.false_skin.light",
            "伪灵皮（轻档）",
            vec![
                ("ash_spider_silk".to_string(), 5),
                ("spirit_wood_scrap".to_string(), 2),
            ],
            15.0,
            10 * 60 * 20,
            FALSE_SKIN_LIGHT_ITEM_ID,
            CraftRequirements::default(),
        ),
        (
            "tuike.false_skin.mid",
            "伪灵皮（中档）",
            vec![
                ("deadwood_core".to_string(), 3),
                ("ash_spider_silk".to_string(), 8),
            ],
            35.0,
            20 * 60 * 20,
            FALSE_SKIN_MID_ITEM_ID,
            CraftRequirements {
                realm_min: Some(Realm::Solidify),
                qi_color_min: None,
                skill_lv_min: None,
            },
        ),
        (
            "tuike.false_skin.heavy",
            "伪灵皮（重档）",
            vec![
                ("mutant_beast_hide".to_string(), 2),
                ("deadwood_core".to_string(), 5),
                ("solid_qi_dye".to_string(), 2),
            ],
            70.0,
            40 * 60 * 20,
            FALSE_SKIN_HEAVY_ITEM_ID,
            CraftRequirements {
                realm_min: Some(Realm::Spirit),
                qi_color_min: Some((ColorKind::Solid, 0.20)),
                skill_lv_min: None,
            },
        ),
        (
            "tuike.false_skin.ancient",
            "伪灵皮（上古级）",
            vec![
                ("ancient_relic_hide".to_string(), 1),
                ("ancient_bone_shard".to_string(), 2),
                ("solid_qi_dye".to_string(), 5),
            ],
            180.0,
            120 * 60 * 20,
            FALSE_SKIN_ANCIENT_ITEM_ID,
            CraftRequirements {
                realm_min: Some(Realm::Void),
                qi_color_min: Some((ColorKind::Solid, 0.30)),
                skill_lv_min: None,
            },
        ),
    ];

    for (id, display_name, materials, qi_cost, time_ticks, output, requirements) in specs {
        registry.register(CraftRecipe {
            id: RecipeId::new(id),
            category: CraftCategory::TuikeSkin,
            display_name: display_name.into(),
            materials,
            qi_cost,
            time_ticks,
            output: (output.to_string(), 1),
            requirements,
            unlock_sources: scroll(id),
        })?;
    }

    Ok(())
}

/// plan-gathering-ux-v1 P4：斧 / 镐凡器配方。锄头沿用 lingtian 既有三档。
pub fn register_gathering_tool_recipes(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    const BONE_COIN_TEMPLATE: &str = "bone_coin_5";
    let scroll = |id: &str| {
        vec![UnlockSource::Scroll {
            item_template: format!("scroll_gathering_{id}"),
        }]
    };
    let specs = [
        (
            "gathering.tool.axe_bone",
            "骨斧",
            vec![
                (BONE_COIN_TEMPLATE.to_string(), 3),
                ("spirit_wood".to_string(), 2),
            ],
            "axe_bone",
        ),
        (
            "gathering.tool.axe_iron",
            "铁斧",
            vec![("iron_ore".to_string(), 3), ("spirit_wood".to_string(), 1)],
            "axe_iron",
        ),
        (
            "gathering.tool.axe_copper",
            "铜斧",
            vec![
                ("copper_ore".to_string(), 3),
                ("spirit_wood".to_string(), 1),
            ],
            "axe_copper",
        ),
        (
            "gathering.tool.pickaxe_bone",
            "骨镐",
            vec![
                (BONE_COIN_TEMPLATE.to_string(), 3),
                ("spirit_wood".to_string(), 2),
            ],
            "pickaxe_bone",
        ),
        (
            "gathering.tool.pickaxe_iron",
            "铁镐",
            vec![("iron_ore".to_string(), 4), ("spirit_wood".to_string(), 1)],
            "pickaxe_iron",
        ),
        (
            "gathering.tool.pickaxe_copper",
            "铜镐",
            vec![
                ("copper_ore".to_string(), 4),
                ("spirit_wood".to_string(), 1),
            ],
            "pickaxe_copper",
        ),
    ];

    for (id, display_name, materials, output) in specs {
        registry.register(CraftRecipe {
            id: RecipeId::new(id),
            category: CraftCategory::Tool,
            display_name: display_name.into(),
            materials,
            qi_cost: 0.0,
            time_ticks: 40 * 20,
            output: (output.to_string(), 1),
            requirements: CraftRequirements::default(),
            unlock_sources: scroll(output),
        })?;
    }
    Ok(())
}

/// 基础加工配方：把采集掉落的粗原料加工成中间材料。
///
/// 这些配方无境界 / 真元色门槛、0 qi 消耗、空 unlock_sources（默认解锁，
/// 不需要残卷 / 师承 / 顿悟）。产出物接入现有高级配方的材料链。
pub fn register_basic_processing_recipes(
    registry: &mut CraftRegistry,
) -> Result<(), RegistryError> {
    #[allow(clippy::type_complexity)]
    let specs: &[(&str, &str, &[(&str, u32)], u64, (&str, u32))] = &[
        // 粗木 → 木柄：采药刀 / 真元诡雷等的前置
        (
            "basic.wood_handle",
            "削木柄",
            &[("crude_wood", 2)],
            20 * 20,
            ("wood_handle", 2),
        ),
        // 铁矿 → 粗铁锭：铁器的前置
        (
            "basic.iron_ingot",
            "粗炼铁锭",
            &[("iron_ore", 3)],
            40 * 20,
            ("iron_ingot", 1),
        ),
        // 粗铁锭 + 木柄 → 铁针：蚀针等暗器前置
        (
            "basic.iron_needle",
            "锻铁针",
            &[("iron_ingot", 1), ("wood_handle", 1)],
            30 * 20,
            ("iron_needle", 5),
        ),
        // 石块 + 草根 → 土罐：毒源煎汤前置
        (
            "basic.clay_pot",
            "捏土罐",
            &[("stone_chunk", 2), ("grass_fiber", 1)],
            30 * 20,
            ("clay_pot", 1),
        ),
        // 草根 → 草绳：通用绑扎材料
        (
            "basic.grass_rope",
            "搓草绳",
            &[("grass_fiber", 4)],
            15 * 20,
            ("grass_rope", 1),
        ),
    ];

    for (id, display_name, materials, time_ticks, output) in specs {
        registry.register(CraftRecipe {
            id: RecipeId::new(*id),
            category: CraftCategory::Tool,
            display_name: (*display_name).into(),
            materials: materials.iter().map(|(t, c)| (t.to_string(), *c)).collect(),
            qi_cost: 0.0,
            time_ticks: *time_ticks,
            output: (output.0.to_string(), output.1),
            requirements: CraftRequirements::default(),
            unlock_sources: vec![],
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_examples_succeeds() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        assert_eq!(registry.len(), 5);
    }

    #[test]
    fn register_examples_covers_baseline_categories_except_misc_and_containers() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        let categories: std::collections::HashSet<CraftCategory> =
            registry.iter().map(|r| r.category).collect();
        for cat in [
            CraftCategory::AnqiCarrier,
            CraftCategory::DuguPotion,
            CraftCategory::TuikeSkin,
            CraftCategory::ZhenfaTrap,
            CraftCategory::Tool,
        ] {
            assert!(
                categories.contains(&cat),
                "missing example for category {:?}",
                cat
            );
        }
        // 5 个示例不该覆盖 Misc — Misc 是兜底
        assert!(!categories.contains(&CraftCategory::Misc));
        // Container 是 anqi-v2 专项类目，不属于 craft-v1 示例基线。
        assert!(!categories.contains(&CraftCategory::Container));
        // ArmorCraft 由 armor-visual-v1 注册，不属于 craft-v1 示例基线。
        assert!(!categories.contains(&CraftCategory::ArmorCraft));
    }

    #[test]
    fn register_examples_non_early_entries_keep_unlock_sources() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        for recipe in registry.iter() {
            if matches!(
                recipe.id.as_str(),
                "craft.example.eclipse_needle.iron" | "craft.example.poison_decoction.fan"
            ) {
                continue;
            }
            assert!(
                !recipe.unlock_sources.is_empty(),
                "example `{}` must have at least one unlock_source",
                recipe.id
            );
        }
    }

    #[test]
    fn register_examples_qi_cost_uses_ledger_safe_finite_values() {
        // 守恒律前置 — 所有示例 qi_cost finite >= 0
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        for recipe in registry.iter() {
            assert!(recipe.qi_cost.is_finite());
            assert!(recipe.qi_cost >= 0.0);
        }
    }

    #[test]
    fn register_examples_includes_tool_with_zero_qi_cost() {
        // §5 决策门 #5 凡器破例 — Tool 无真元投入
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        let tool = registry
            .by_category(CraftCategory::Tool)
            .next()
            .expect("must have at least one Tool example");
        assert_eq!(tool.qi_cost, 0.0);
    }

    #[test]
    fn register_examples_eclipse_needle_uses_insidious_qi_color_gate() {
        // 蚀针匹配 worldview §六：阴诡色需求
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        let recipe = registry
            .get(&RecipeId::new("craft.example.eclipse_needle.iron"))
            .expect("eclipse_needle example must register");
        let (kind, share) = recipe
            .requirements
            .qi_color_min
            .expect("eclipse_needle must have qi_color gate");
        assert_eq!(kind, ColorKind::Insidious);
        assert!(share > 0.0);
    }

    #[test]
    fn register_examples_early_dugu_recipes_are_default_unlocked() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        for id in [
            "craft.example.eclipse_needle.iron",
            "craft.example.poison_decoction.fan",
        ] {
            let recipe = registry
                .get(&RecipeId::new(id))
                .expect("early Dugu recipe must register");
            assert!(
                recipe.unlock_sources.is_empty(),
                "`{id}` should be visible in hand-craft UI by default"
            );
        }
    }

    #[test]
    fn register_examples_early_dugu_item_templates_exist() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        let item_registry = crate::inventory::load_item_registry().expect("item registry loads");
        for id in [
            "craft.example.eclipse_needle.iron",
            "craft.example.poison_decoction.fan",
        ] {
            let recipe = registry
                .get(&RecipeId::new(id))
                .expect("early Dugu recipe must register");
            for (template_id, count) in &recipe.materials {
                assert!(
                    item_registry.get(template_id).is_some(),
                    "material `{template_id}` for recipe `{}` must exist in item registry",
                    recipe.id
                );
                assert!(*count >= 1, "`{}` material count must be >= 1", recipe.id);
            }
            let (output_id, count) = &recipe.output;
            assert!(
                item_registry.get(output_id).is_some(),
                "output `{output_id}` for recipe `{}` must exist in item registry",
                recipe.id
            );
            assert!(*count >= 1, "`{}` output count must be >= 1", recipe.id);
        }
    }

    #[test]
    fn register_anqi_v2_recipes_adds_six_carriers_and_three_containers() {
        let mut registry = CraftRegistry::new();
        register_anqi_v2_recipes(&mut registry).unwrap();
        assert_eq!(registry.by_category(CraftCategory::AnqiCarrier).count(), 6);
        assert_eq!(registry.by_category(CraftCategory::Container).count(), 3);
        assert!(registry
            .get(&RecipeId::new("anqi.carrier.shanggu_bone"))
            .is_some_and(|recipe| recipe.requirements.realm_min == Some(Realm::Void)));
    }

    #[test]
    fn register_zhenfa_v2_recipes_adds_four_arrays_and_two_flags() {
        let mut registry = CraftRegistry::new();
        register_zhenfa_v2_recipes(&mut registry).unwrap();
        assert_eq!(registry.by_category(CraftCategory::ZhenfaTrap).count(), 4);
        assert_eq!(registry.by_category(CraftCategory::Tool).count(), 2);
        assert!(registry
            .get(&RecipeId::new("zhenfa.array.deceive"))
            .is_some_and(|recipe| recipe.requirements.realm_min == Some(Realm::Solidify)));
    }

    #[test]
    fn register_zhenfa_v2_recipes_keep_ids_in_zhenfa_namespace() {
        let mut registry = CraftRegistry::new();
        register_zhenfa_v2_recipes(&mut registry).unwrap();
        for recipe in registry.iter() {
            assert!(
                recipe.id.as_str().starts_with("zhenfa."),
                "zhenfa recipe id `{}` should stay in zhenfa namespace",
                recipe.id
            );
        }
    }

    #[test]
    fn register_zhenfa_content_recipes_adds_three_ordinary_traps() {
        let mut registry = CraftRegistry::new();
        register_zhenfa_content_recipes(&mut registry).unwrap();

        let warning = registry
            .get(&RecipeId::new("zhenfa.content.warning_trap"))
            .expect("warning trap recipe should be registered");
        assert_eq!(
            warning.category,
            CraftCategory::ZhenfaTrap,
            "expected warning trap recipe to use ZhenfaTrap category because content-v1 routes through generic trap crafting; actual={:?}",
            warning.category
        );
        assert_eq!(
            warning.output,
            ("warning_trap".to_string(), 3),
            "expected warning trap recipe to batch craft three talismans; actual={:?}",
            warning.output
        );

        let blast = registry
            .get(&RecipeId::new("zhenfa.content.blast_trap"))
            .expect("blast trap recipe should be registered");
        assert_eq!(
            blast.time_ticks,
            60 * 20,
            "expected blast trap craft time to be 60 seconds at 20 tps; actual={}",
            blast.time_ticks
        );
        assert_eq!(
            blast.output,
            ("blast_trap".to_string(), 1),
            "expected blast trap recipe to output one talisman because it is the expensive trap; actual={:?}",
            blast.output
        );

        let slow = registry
            .get(&RecipeId::new("zhenfa.content.slow_trap"))
            .expect("slow trap recipe should be registered");
        assert_eq!(
            slow.qi_cost, 8.0,
            "expected slow trap recipe qi cost to match plan pin; actual={}",
            slow.qi_cost
        );
        assert_eq!(
            slow.output,
            ("slow_trap".to_string(), 2),
            "expected slow trap recipe to output two talismans; actual={:?}",
            slow.output
        );
    }

    #[test]
    fn register_zhenfa_content_recipes_rejects_duplicate_ids() {
        let mut registry = CraftRegistry::new();
        register_zhenfa_content_recipes(&mut registry).unwrap();

        let duplicate = register_zhenfa_content_recipes(&mut registry);

        assert!(
            matches!(duplicate, Err(RegistryError::DuplicateId(_))),
            "expected duplicate recipe ids to be rejected when registering zhenfa content twice; actual={duplicate:?}"
        );
    }

    #[test]
    fn register_tuike_v2_recipes_adds_four_false_skin_tiers() {
        let mut registry = CraftRegistry::new();
        register_tuike_v2_recipes(&mut registry).unwrap();
        assert_eq!(registry.by_category(CraftCategory::TuikeSkin).count(), 4);
        assert!(registry
            .get(&RecipeId::new("tuike.false_skin.ancient"))
            .is_some_and(|recipe| recipe.requirements.realm_min == Some(Realm::Void)
                && recipe.output.0 == crate::combat::tuike_v2::state::FALSE_SKIN_ANCIENT_ITEM_ID));
    }

    #[test]
    fn register_mundane_armor_recipes_adds_28_armor_craft_entries() {
        let mut registry = CraftRegistry::new();
        crate::armor::mundane::register_mundane_armor_recipes(&mut registry).unwrap();

        assert_eq!(
            registry.by_category(CraftCategory::ArmorCraft).count(),
            28,
            "expected 28 mundane armor recipes (7 materials × 4 slots), got {}",
            registry.by_category(CraftCategory::ArmorCraft).count()
        );
        assert!(registry
            .get(&RecipeId::new("armor.mundane.iron.chestplate"))
            .is_some_and(|recipe| recipe.output == ("armor_iron_chestplate".to_string(), 1)));
    }

    #[test]
    fn register_gathering_tool_recipes_adds_six_tool_entries() {
        let mut registry = CraftRegistry::new();
        register_gathering_tool_recipes(&mut registry).unwrap();

        let tools: Vec<_> = registry.by_category(CraftCategory::Tool).collect();
        assert_eq!(
            tools.len(),
            6,
            "gathering-ux-v1 must register exactly six axe/pickaxe tool recipes"
        );
        for recipe in &tools {
            assert_eq!(
                recipe.qi_cost, 0.0,
                "{} should remain a mundane gathering tool recipe with zero qi_cost",
                recipe.id
            );
            assert_eq!(
                recipe.time_ticks, 800,
                "{} should keep the 40s mundane tool craft time",
                recipe.id
            );
        }
        let outputs = tools
            .iter()
            .map(|recipe| recipe.output.0.as_str())
            .collect::<Vec<_>>();
        assert!(
            outputs.contains(&"axe_copper"),
            "expected recipe output `axe_copper` because copper axe is one of the six gathering tools; actual outputs: {:?}",
            outputs
        );
        assert!(
            outputs.contains(&"pickaxe_iron"),
            "expected recipe output `pickaxe_iron` because iron pickaxe is one of the six gathering tools; actual outputs: {:?}",
            outputs
        );
        let bone_axe = tools
            .iter()
            .find(|recipe| recipe.output.0 == "axe_bone")
            .expect("gathering.tool.axe_bone recipe should exist");
        assert!(
            bone_axe
                .materials
                .iter()
                .any(|(template, count)| template == "bone_coin_5" && *count == 3),
            "axe_bone should consume three bone_coin_5 entries as the low-tier mundane currency"
        );
        assert!(
            bone_axe
                .materials
                .iter()
                .any(|(template, count)| template == "spirit_wood" && *count == 2),
            "axe_bone should consume two spirit_wood entries as the handle material"
        );
        assert!(
            bone_axe.unlock_sources.iter().any(|source| matches!(
                source,
                UnlockSource::Scroll { item_template } if item_template == "scroll_gathering_axe_bone"
            )),
            "axe_bone should unlock from scroll_gathering_axe_bone"
        );
    }

    #[test]
    fn register_gathering_tool_recipes_rejects_duplicate_registration() {
        let mut registry = CraftRegistry::new();
        register_gathering_tool_recipes(&mut registry).unwrap();

        let duplicate = register_gathering_tool_recipes(&mut registry);
        assert!(
            matches!(duplicate, Err(RegistryError::DuplicateId(_))),
            "registering gathering tool recipes twice should fail with DuplicateId; actual result: {:?}",
            duplicate
        );
    }

    #[test]
    fn gathering_tool_recipe_unlock_scrolls_exist_in_item_registry() {
        let mut registry = CraftRegistry::new();
        register_gathering_tool_recipes(&mut registry).unwrap();
        let item_registry = crate::inventory::load_item_registry().expect("item registry loads");

        for recipe in registry.by_category(CraftCategory::Tool) {
            let Some(UnlockSource::Scroll { item_template }) = recipe.unlock_sources.first() else {
                panic!("{} should unlock from a recipe scroll", recipe.id);
            };
            assert!(
                item_registry.get(item_template).is_some(),
                "unlock scroll `{item_template}` for recipe `{}` must exist",
                recipe.id
            );
        }
    }

    #[test]
    fn register_examples_each_id_starts_with_craft_example_namespace() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        for recipe in registry.iter() {
            assert!(
                recipe.id.as_str().starts_with("craft.example."),
                "example recipe id `{}` should be in `craft.example.*` namespace",
                recipe.id
            );
        }
    }

    #[test]
    fn register_examples_rejects_double_register() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        // 第二次 register 必须 reject（duplicate id）
        let err = register_examples(&mut registry).unwrap_err();
        assert!(matches!(err, RegistryError::DuplicateId(_)));
    }

    #[test]
    fn register_basic_processing_succeeds_with_5_recipes() {
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();
        assert_eq!(registry.len(), 5);
    }

    #[test]
    fn basic_processing_ids_start_with_basic_namespace() {
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();
        for recipe in registry.iter() {
            assert!(
                recipe.id.as_str().starts_with("basic."),
                "basic processing recipe id `{}` should be in `basic.*` namespace",
                recipe.id
            );
        }
    }

    #[test]
    fn basic_processing_all_zero_qi_and_no_realm() {
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();
        for recipe in registry.iter() {
            assert_eq!(recipe.qi_cost, 0.0, "`{}` should have 0 qi cost", recipe.id);
            assert_eq!(
                recipe.requirements,
                CraftRequirements::default(),
                "`{}` should have no requirements",
                recipe.id
            );
        }
    }

    #[test]
    fn basic_processing_unlock_sources_empty_means_default_unlocked() {
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();
        for recipe in registry.iter() {
            assert!(
                recipe.unlock_sources.is_empty(),
                "`{}` should have empty unlock_sources (default unlocked)",
                recipe.id
            );
        }
    }

    #[test]
    fn basic_processing_output_templates_exist_in_item_registry() {
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();
        let item_registry = crate::inventory::load_item_registry().expect("item registry loads");
        for recipe in registry.iter() {
            let (output_id, count) = &recipe.output;
            assert!(
                item_registry.get(output_id).is_some(),
                "output `{output_id}` for recipe `{}` must exist in item registry",
                recipe.id
            );
            assert!(*count >= 1, "`{}` output count must be >= 1", recipe.id);
        }
    }

    #[test]
    fn basic_processing_material_templates_exist_in_item_registry() {
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();
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
                    "`{}` material `{template_id}` count must be >= 1",
                    recipe.id
                );
            }
        }
    }

    #[test]
    fn basic_processing_rejects_duplicate() {
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();
        let err = register_basic_processing_recipes(&mut registry).unwrap_err();
        assert!(matches!(err, RegistryError::DuplicateId(_)));
    }

    #[test]
    fn basic_wood_handle_recipe_chain() {
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();
        let recipe = registry.get(&RecipeId::new("basic.wood_handle")).unwrap();
        assert_eq!(recipe.materials, vec![("crude_wood".into(), 2)]);
        assert_eq!(recipe.output, ("wood_handle".into(), 2));
    }

    #[test]
    fn basic_iron_ingot_recipe_chain() {
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();
        let recipe = registry.get(&RecipeId::new("basic.iron_ingot")).unwrap();
        assert_eq!(recipe.materials, vec![("iron_ore".into(), 3)]);
        assert_eq!(recipe.output, ("iron_ingot".into(), 1));
    }
}
