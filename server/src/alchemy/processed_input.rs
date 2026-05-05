//! plan-lingtian-process-v1 P4 — 加工产物投料加成。
//!
//! 炼丹配方仍按原 material 匹配；本 helper 只根据 inventory item id + quality
//! 计算“加工后投料”的品质 / 成功率修饰，避免改动 alchemy recipe JSON 结构。

use crate::lingtian::processing::processed_input_quality_bonus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessedIngredientKind {
    Fresh,
    Dried,
    Powder,
    ForgingAlchemy,
    Extract,
    Withered,
}

impl ProcessedIngredientKind {
    pub fn from_item_id(item_id: &str) -> Self {
        if item_id.starts_with("extract_") {
            Self::Extract
        } else if item_id.starts_with("processed_") {
            Self::ForgingAlchemy
        } else if item_id.starts_with("powder_") {
            Self::Powder
        } else if item_id.starts_with("dry_") {
            Self::Dried
        } else if item_id.starts_with("withered_") {
            Self::Withered
        } else {
            Self::Fresh
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProcessedAlchemyBonus {
    pub quality_bonus: f32,
    pub success_rate_bonus: f32,
    pub recycle_hook: bool,
}

pub fn processed_alchemy_bonus(item_id: &str, item_quality: f32) -> ProcessedAlchemyBonus {
    let kind = ProcessedIngredientKind::from_item_id(item_id);
    let quality_bonus = match kind {
        ProcessedIngredientKind::Fresh => 0.0,
        ProcessedIngredientKind::Dried => processed_input_quality_bonus(item_id, item_quality),
        ProcessedIngredientKind::Powder => {
            processed_input_quality_bonus(item_id, item_quality) + 0.02
        }
        ProcessedIngredientKind::ForgingAlchemy => {
            processed_input_quality_bonus(item_id, item_quality) + 0.08
        }
        ProcessedIngredientKind::Extract => {
            processed_input_quality_bonus(item_id, item_quality) + 0.16
        }
        ProcessedIngredientKind::Withered => -0.20,
    };
    let success_rate_bonus = match kind {
        ProcessedIngredientKind::ForgingAlchemy => 0.05,
        ProcessedIngredientKind::Extract => 0.10,
        ProcessedIngredientKind::Withered => -0.15,
        _ => 0.0,
    };
    ProcessedAlchemyBonus {
        quality_bonus,
        success_rate_bonus,
        recycle_hook: kind == ProcessedIngredientKind::Withered,
    }
}

pub fn route_withered_item_to_alchemy_recycle_hook(item_id: &str) -> Option<&'static str> {
    if ProcessedIngredientKind::from_item_id(item_id) == ProcessedIngredientKind::Withered {
        Some("alchemy_recycle_v1")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alchemy_pill_recipe_with_processed_input_quality_bonus() {
        let bonus = processed_alchemy_bonus("processed_ci_she_hao", 1.2);
        assert!(bonus.quality_bonus > 0.10);
        assert_eq!(bonus.success_rate_bonus, 0.05);
    }

    #[test]
    fn alchemy_pill_recipe_with_extracted_input_success_rate_bonus() {
        let bonus = processed_alchemy_bonus("extract_ci_she_hao", 1.6);
        assert!(bonus.quality_bonus > 0.30);
        assert_eq!(bonus.success_rate_bonus, 0.10);
    }

    #[test]
    fn withered_item_routes_to_alchemy_recycle_hook() {
        assert_eq!(
            route_withered_item_to_alchemy_recycle_hook("withered_dry_ci_she_hao"),
            Some("alchemy_recycle_v1")
        );
        assert_eq!(
            route_withered_item_to_alchemy_recycle_hook("dry_ci_she_hao"),
            None
        );
    }
}
