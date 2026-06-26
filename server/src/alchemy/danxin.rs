//! plan-alchemy-v2 P4 — 丹心识别：消耗丹药换配方碎片情报。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Entity, Event, EventReader, EventWriter, Query, ResMut};

use crate::cultivation::components::{Cultivation, Realm};
use crate::inventory::{
    add_item_to_player_inventory_with_alchemy, consume_item_instance_once,
    inventory_item_by_instance_borrow, AlchemyItemData, InventoryInstanceIdAllocator, ItemRegistry,
    PlayerInventory,
};

use super::recipe::{Recipe, RecipeId, RecipeRegistry};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecipeHint {
    pub source_pill: String,
    pub recipe_id: Option<RecipeId>,
    pub accuracy: f64,
    pub ingredients: Vec<String>,
}

#[derive(Debug, Clone, Copy, Event, PartialEq)]
pub struct DanxinIdentifyIntent {
    pub player: Entity,
    pub pill_instance_id: u64,
    /// 0.0..=1.0 roll，由 caller 注入，方便测试与回放。
    pub roll: f64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct AlchemyInsightEvent {
    pub player: Entity,
    pub player_id: String,
    pub hint: RecipeHint,
}

pub const RECIPE_HINT_TEMPLATE_ID: &str = "alchemy_recipe_hint";

pub fn realm_tier(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 1,
        Realm::Induce => 2,
        Realm::Condense => 3,
        Realm::Solidify => 4,
        Realm::Spirit => 5,
        Realm::Void => 6,
    }
}

pub fn identify_accuracy(realm_tier: u8, pill_tier: u8, roll: f64) -> f64 {
    let roll = if roll.is_finite() { roll } else { 0.5 }.clamp(0.5, 1.0);
    let pill_tier = pill_tier.max(1);
    ((f64::from(realm_tier) / f64::from(pill_tier)) * roll).clamp(0.0, 1.0)
}

pub fn build_recipe_hint(source_pill: &str, recipe: Option<&Recipe>, accuracy: f64) -> RecipeHint {
    let ingredients = recipe
        .map(|recipe| {
            let mut materials = recipe
                .stages
                .iter()
                .flat_map(|stage| {
                    stage
                        .required
                        .iter()
                        .map(|ingredient| ingredient.material.clone())
                })
                .collect::<Vec<_>>();
            materials.sort();
            materials.dedup();
            let reveal_count = if accuracy >= 0.80 {
                3
            } else if accuracy >= 0.50 {
                2
            } else {
                1
            };
            materials.truncate(reveal_count.min(materials.len()));
            materials
        })
        .unwrap_or_default();

    RecipeHint {
        source_pill: source_pill.to_string(),
        recipe_id: recipe.map(|recipe| recipe.id.clone()),
        accuracy,
        ingredients,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_danxin_identify_intents(
    mut intents: EventReader<DanxinIdentifyIntent>,
    recipes: valence::prelude::Res<RecipeRegistry>,
    item_registry: valence::prelude::Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut inventories: Query<&mut PlayerInventory>,
    cultivations: Query<&Cultivation>,
    usernames: Query<&valence::prelude::Username>,
    mut insight_events: EventWriter<AlchemyInsightEvent>,
) {
    for intent in intents.read() {
        let pill = inventories.get(intent.player).ok().and_then(|inventory| {
            inventory_item_by_instance_borrow(inventory, intent.pill_instance_id).cloned()
        });
        let Some(pill) = pill else {
            continue;
        };
        let Ok(mut inventory) = inventories.get_mut(intent.player) else {
            continue;
        };
        let Some(AlchemyItemData::Pill {
            recipe_id,
            quality_tier,
            ..
        }) = pill.alchemy.as_ref()
        else {
            continue;
        };

        let realm_tier = cultivations
            .get(intent.player)
            .map(|cultivation| realm_tier(cultivation.realm))
            .unwrap_or(1);
        let accuracy = identify_accuracy(realm_tier, *quality_tier, intent.roll);
        let recipe = recipes.get(recipe_id);
        let hint = build_recipe_hint(pill.template_id.as_str(), recipe, accuracy);

        let mut staged_inventory = inventory.clone();
        let mut staged_allocator = allocator.clone();
        if consume_item_instance_once(&mut staged_inventory, intent.pill_instance_id).is_err() {
            continue;
        }
        if add_item_to_player_inventory_with_alchemy(
            &mut staged_inventory,
            &item_registry,
            &mut staged_allocator,
            RECIPE_HINT_TEMPLATE_ID,
            1,
            Some(AlchemyItemData::RecipeHint { hint: hint.clone() }),
            0,
        )
        .is_err()
        {
            continue;
        }
        *inventory = staged_inventory;
        *allocator = staged_allocator;

        if accuracy >= 0.80 {
            let player_id = usernames
                .get(intent.player)
                .map(|username| crate::player::state::canonical_player_id(username.0.as_str()))
                .unwrap_or_else(|_| format!("entity:{:?}", intent.player));
            insight_events.send(AlchemyInsightEvent {
                player: intent.player,
                player_id,
                hint,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::alchemy::recipe::{
        FireProfile, IngredientSpec, Outcomes, RecipeStage, ToleranceSpec,
    };
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemTemplate,
        PlacedItemState,
    };
    use valence::prelude::{App, Update, Username};

    fn sample_recipe() -> Recipe {
        Recipe {
            id: "hui_yuan_pill_v0".to_string(),
            name: "回元丹".to_string(),
            furnace_tier_min: 1,
            stages: vec![RecipeStage {
                at_tick: 0,
                required: vec![
                    IngredientSpec {
                        material: "hui_yuan_zhi".to_string(),
                        count: 2,
                        mineral_id: None,
                    },
                    IngredientSpec {
                        material: "ling_shui".to_string(),
                        count: 1,
                        mineral_id: None,
                    },
                ],
                window: 0,
            }],
            fire_profile: FireProfile {
                target_temp: 0.5,
                target_duration_ticks: 20,
                qi_cost: 1.0,
                tolerance: ToleranceSpec {
                    temp_band: 0.1,
                    duration_band: 5,
                },
            },
            outcomes: Outcomes {
                perfect: None,
                good: None,
                flawed: None,
                waste: None,
                explode: None,
            },
            flawed_fallback: None,
        }
    }

    #[test]
    fn accuracy_uses_realm_over_pill_tier_and_clamps_roll() {
        assert!((identify_accuracy(2, 4, 0.75) - 0.375).abs() < 1e-9);
        assert_eq!(identify_accuracy(6, 1, 1.0), 1.0);
        assert!((identify_accuracy(1, 5, 0.1) - 0.1).abs() < 1e-9);
    }

    #[test]
    fn high_accuracy_reveals_up_to_three_unique_ingredients() {
        let recipe = sample_recipe();
        let hint = build_recipe_hint("huiyuan_pill", Some(&recipe), 0.9);

        assert_eq!(hint.recipe_id.as_deref(), Some("hui_yuan_pill_v0"));
        assert_eq!(hint.ingredients, vec!["hui_yuan_zhi", "ling_shui"]);
    }

    #[test]
    fn low_accuracy_never_restores_full_recipe() {
        let recipe = sample_recipe();
        let hint = build_recipe_hint("huiyuan_pill", Some(&recipe), 0.4);

        assert_eq!(hint.ingredients.len(), 1);
    }

    fn template(id: &str, category: ItemCategory, grid_w: u8, grid_h: u8) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: id.to_string(),
            category,
            placeable: None,
            max_stack_count: 1,
            grid_w,
            grid_h,
            base_weight: 0.1,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 1.0,
            description: id.to_string(),
            effect: None,
            cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
            cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shield_spec: None,
            shelflife_track: None,
        }
    }

    fn pill_instance(instance_id: u64) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: "huiyuan_pill".to_string(),
            display_name: "huiyuan_pill".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: "huiyuan_pill".to_string(),
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
            alchemy: Some(AlchemyItemData::Pill {
                recipe_id: "hui_yuan_pill_v0".to_string(),
                quality_tier: 1,
                effect_multiplier: 1.0,
                consecrated: false,
                side_effect: None,
            }),
            lingering_owner_qi: None,
        }
    }

    #[test]
    fn identify_does_not_consume_pill_or_emit_insight_when_hint_cannot_fit() {
        let mut app = App::new();
        let mut recipes = RecipeRegistry::new();
        recipes.insert(sample_recipe()).unwrap();
        app.insert_resource(recipes);
        app.insert_resource(ItemRegistry::from_map(HashMap::from([
            (
                "huiyuan_pill".to_string(),
                template("huiyuan_pill", ItemCategory::Pill, 1, 1),
            ),
            (
                RECIPE_HINT_TEMPLATE_ID.to_string(),
                template(RECIPE_HINT_TEMPLATE_ID, ItemCategory::RecipeHint, 2, 1),
            ),
        ])));
        app.insert_resource(InventoryInstanceIdAllocator::new(100));
        app.add_event::<DanxinIdentifyIntent>();
        app.add_event::<AlchemyInsightEvent>();
        app.add_systems(Update, handle_danxin_identify_intents);

        let player = app
            .world_mut()
            .spawn((
                PlayerInventory {
                    revision: InventoryRevision(0),
                    containers: vec![ContainerState {
                        id: "pack_1".to_string(),
                        name: "pack_1".to_string(),
                        rows: 1,
                        cols: 1,
                        items: vec![PlacedItemState {
                            row: 0,
                            col: 0,
                            instance: pill_instance(42),
                        }],
                        owner_instance_id: None,
                    }],
                    equipped: HashMap::new(),
                    hotbar: Default::default(),
                    bone_coins: 0,
                    max_weight: 50.0,
                    triggered_treasures: Vec::new(),
                },
                Cultivation {
                    realm: Realm::Void,
                    ..Default::default()
                },
                Username("Azure".to_string()),
            ))
            .id();
        app.world_mut().send_event(DanxinIdentifyIntent {
            player,
            pill_instance_id: 42,
            roll: 1.0,
        });

        app.update();

        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(
            inventory.containers[0].items[0].instance.instance_id, 42,
            "提示纸模板 2x1 放不进 1x1 容器时，丹药不能先被消耗"
        );
        assert_eq!(
            inventory.revision,
            InventoryRevision(0),
            "失败路径不应修改 inventory revision"
        );
        let events = app
            .world()
            .resource::<valence::prelude::Events<AlchemyInsightEvent>>();
        assert_eq!(
            events.len(),
            0,
            "提示纸没有实际入包时不能发 insight，避免前端/叙事误报成功"
        );
    }
}
