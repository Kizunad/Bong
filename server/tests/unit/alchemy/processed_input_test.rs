use bong_server::alchemy::processed_input::{
    processed_alchemy_bonus, route_withered_item_to_alchemy_recycle_hook,
};

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
