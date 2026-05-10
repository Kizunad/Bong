use crate::craft::{
    CraftCategory, CraftRecipe, CraftRegistry, CraftRequirements, RecipeId, RegistryError,
    UnlockSource,
};

use super::components::{PoisonPillKind, PoisonPowderKind};

pub fn register_craft_recipes(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    for powder in PoisonPowderKind::ALL {
        registry.register(craft_recipe_for_powder(powder))?;
    }
    Ok(())
}

pub fn craft_recipe_for_powder(powder: PoisonPowderKind) -> CraftRecipe {
    let pill = powder.source_pill();
    CraftRecipe {
        id: RecipeId::new(format!("poison_trait.grind.{}", suffix_for_pill(pill))),
        category: CraftCategory::PoisonPowder,
        display_name: format!("研磨{}", powder.display_name()),
        materials: vec![(pill.item_id().to_string(), 1)],
        qi_cost: 2.0,
        time_ticks: 30 * 20,
        output: (powder.item_id().to_string(), 3),
        requirements: CraftRequirements::default(),
        unlock_sources: vec![
            UnlockSource::Scroll {
                item_template: "scroll_poison_grind".into(),
            },
            UnlockSource::Mentor {
                npc_archetype: "alchemist_quirk".into(),
            },
        ],
    }
}

pub fn poison_alchemy_recipe_ids() -> [&'static str; 5] {
    [
        "poison_trait_wu_sui_san_xin_v1",
        "poison_trait_chi_tuo_zhi_sui_v1",
        "poison_trait_qing_lin_man_tuo_v1",
        "poison_trait_tie_fu_she_dan_v1",
        "poison_trait_fu_xin_xuan_gui_v1",
    ]
}

fn suffix_for_pill(pill: PoisonPillKind) -> &'static str {
    match pill {
        PoisonPillKind::WuSuiSanXin => "wu_sui_san_xin",
        PoisonPillKind::ChiTuoZhiSui => "chi_tuo_zhi_sui",
        PoisonPillKind::QingLinManTuo => "qing_lin_man_tuo",
        PoisonPillKind::TieFuSheDan => "tie_fu_she_dan",
        PoisonPillKind::FuXinXuanGui => "fu_xin_xuan_gui",
    }
}
