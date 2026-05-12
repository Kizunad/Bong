use crate::inventory::{ItemCategory, ItemTemplate};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForeignQiResistanceOutcome {
    pub effect_multiplier: f64,
    pub qi_drain_per_second: f64,
    pub health_loss: f32,
}

impl ForeignQiResistanceOutcome {
    pub fn none() -> Self {
        Self {
            effect_multiplier: 1.0,
            qi_drain_per_second: 0.0,
            health_loss: 0.0,
        }
    }
}

pub fn foreign_qi_resistance_for_use(
    item_template: &ItemTemplate,
    has_lingering_owner_qi: bool,
) -> ForeignQiResistanceOutcome {
    if !has_lingering_owner_qi {
        return ForeignQiResistanceOutcome::none();
    }
    match item_template.category {
        ItemCategory::Pill => ForeignQiResistanceOutcome {
            effect_multiplier: 0.5,
            qi_drain_per_second: 0.0,
            health_loss: 10.0,
        },
        ItemCategory::Weapon | ItemCategory::Treasure => ForeignQiResistanceOutcome {
            effect_multiplier: 0.5,
            qi_drain_per_second: 1.0,
            health_loss: 0.0,
        },
        _ => ForeignQiResistanceOutcome::none(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ItemRarity, WeaponSpec};

    fn template(category: ItemCategory) -> ItemTemplate {
        ItemTemplate {
            id: "test_item".to_string(),
            display_name: "测试物品".to_string(),
            category,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 1,
            base_weight: 1.0,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 1.0,
            description: String::new(),
            effect: None,
            cast_duration_ms: 0,
            cooldown_ms: 0,
            weapon_spec: if category == ItemCategory::Weapon {
                Some(WeaponSpec {
                    weapon_kind: crate::combat::weapon::WeaponKind::Sword,
                    base_attack: 10.0,
                    quality_tier: 0,
                    durability_max: 100.0,
                    qi_cost_mul: 1.0,
                })
            } else {
                None
            },
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
        }
    }

    #[test]
    fn pill_lingering_qi_halves_effect_and_hurts_user() {
        let outcome = foreign_qi_resistance_for_use(&template(ItemCategory::Pill), true);
        assert_eq!(outcome.effect_multiplier, 0.5);
        assert_eq!(outcome.health_loss, 10.0);
    }

    #[test]
    fn weapon_lingering_qi_halves_effect_and_drains_qi() {
        let outcome = foreign_qi_resistance_for_use(&template(ItemCategory::Weapon), true);
        assert_eq!(outcome.effect_multiplier, 0.5);
        assert_eq!(outcome.qi_drain_per_second, 1.0);
    }

    #[test]
    fn clean_item_has_no_resistance_penalty() {
        let outcome = foreign_qi_resistance_for_use(&template(ItemCategory::Pill), false);
        assert_eq!(outcome, ForeignQiResistanceOutcome::none());
    }
}
