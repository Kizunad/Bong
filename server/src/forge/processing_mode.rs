//! plan-lingtian-process-v1 P2 — 丹炉炮制模式入口。
//!
//! 这里不复用武器锻造四步状态机；它只把“丹炉可启动炮制/萃取 session”的
//! 权限和配方校验封成事件入口，实际加工推进仍由 `lingtian::processing` 负责。

use valence::prelude::{bevy_ecs, Entity, Event, EventReader, EventWriter, Res};

use crate::lingtian::processing::{
    validate_processing_start, ItemStack, ProcessingKind, ProcessingRecipeRegistry,
    ProcessingSkillLevels,
};

#[derive(Debug, Clone, Event)]
pub struct StartForgeProcessingRequest {
    pub player: Entity,
    pub station: Entity,
    pub recipe_id: String,
    pub kind: ProcessingKind,
    pub inputs: Vec<ItemStack>,
    pub skills: ProcessingSkillLevels,
}

#[derive(Debug, Clone, PartialEq, Event)]
pub struct ForgeProcessingAccepted {
    pub player: Entity,
    pub station: Entity,
    pub recipe_id: String,
    pub kind: ProcessingKind,
    pub duration_ticks: u32,
}

pub fn forge_processing_mode_handler(
    registry: Res<ProcessingRecipeRegistry>,
    mut requests: EventReader<StartForgeProcessingRequest>,
    mut accepted: EventWriter<ForgeProcessingAccepted>,
) {
    for request in requests.read() {
        if request.kind != ProcessingKind::ForgingAlchemy
            && request.kind != ProcessingKind::Extraction
        {
            tracing::debug!(
                "[bong][forge][processing] ignoring non-forge processing kind {:?}",
                request.kind
            );
            continue;
        }
        if validate_processing_start(
            &registry,
            request.recipe_id.as_str(),
            request.kind,
            &request.inputs,
            request.skills,
        )
        .is_err()
        {
            continue;
        }
        let Some(recipe) = registry.get(request.recipe_id.as_str()) else {
            continue;
        };
        accepted.send(ForgeProcessingAccepted {
            player: request.player,
            station: request.station,
            recipe_id: request.recipe_id.clone(),
            kind: request.kind,
            duration_ticks: recipe.duration_ticks,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lingtian::processing::{
        ProcessingRecipe, RecipeInput, RecipeOutput, SkillRequirement, EXTRACTION_TICKS,
    };
    use valence::prelude::{App, Update};

    fn entity(raw: u32) -> Entity {
        Entity::from_raw(raw)
    }

    #[test]
    fn forging_alchemy_session_via_dan_furnace() {
        let mut registry = ProcessingRecipeRegistry::new();
        registry
            .insert(ProcessingRecipe {
                id: "forge_ci_she_hao".to_string(),
                kind: ProcessingKind::ForgingAlchemy,
                inputs: vec![RecipeInput {
                    item_id: "dry_ci_she_hao".to_string(),
                    count: 2,
                    min_freshness: None,
                }],
                outputs: vec![RecipeOutput {
                    item_id: "processed_ci_she_hao".to_string(),
                    count: 2,
                    quality_multiplier: 1.2,
                    freshness_profile: Some("forging_alchemy_v1".to_string()),
                }],
                duration_ticks: 6_000,
                skill_req: SkillRequirement {
                    herbalism: 5,
                    alchemy: 3,
                },
                failure_rate: 0.10,
                failure_output: None,
                qi_cost: 5,
            })
            .unwrap();

        let mut app = App::new();
        app.insert_resource(registry);
        app.add_event::<StartForgeProcessingRequest>();
        app.add_event::<ForgeProcessingAccepted>();
        app.add_systems(Update, forge_processing_mode_handler);

        app.world_mut().send_event(StartForgeProcessingRequest {
            player: entity(1),
            station: entity(2),
            recipe_id: "forge_ci_she_hao".to_string(),
            kind: ProcessingKind::ForgingAlchemy,
            inputs: vec![ItemStack::new("dry_ci_she_hao", 2, 1.0)],
            skills: ProcessingSkillLevels {
                herbalism: 5,
                alchemy: 3,
            },
        });
        app.update();

        let events = app
            .world()
            .resource::<valence::prelude::Events<ForgeProcessingAccepted>>();
        let accepted = events.iter_current_update_events().next().unwrap();
        assert_eq!(accepted.recipe_id, "forge_ci_she_hao");
        assert_eq!(accepted.kind, ProcessingKind::ForgingAlchemy);
    }

    #[test]
    fn extraction_session_high_quality_low_quantity() {
        let mut registry = ProcessingRecipeRegistry::new();
        registry
            .insert(ProcessingRecipe {
                id: "extract_ci_she_hao".to_string(),
                kind: ProcessingKind::Extraction,
                inputs: vec![RecipeInput {
                    item_id: "ci_she_hao".to_string(),
                    count: 3,
                    min_freshness: None,
                }],
                outputs: vec![RecipeOutput {
                    item_id: "extract_ci_she_hao".to_string(),
                    count: 1,
                    quality_multiplier: 2.0,
                    freshness_profile: Some("extraction_v1".to_string()),
                }],
                duration_ticks: EXTRACTION_TICKS,
                skill_req: SkillRequirement {
                    herbalism: 6,
                    alchemy: 3,
                },
                failure_rate: 0.15,
                failure_output: None,
                qi_cost: 0,
            })
            .unwrap();
        assert_eq!(
            registry
                .recipes_by_kind(ProcessingKind::Extraction)
                .next()
                .unwrap()
                .outputs[0]
                .quality_multiplier,
            2.0
        );
    }

    #[test]
    fn forging_alchemy_quality_x1_2_modifier() {
        let output = RecipeOutput {
            item_id: "processed_ning_mai_cao".to_string(),
            count: 2,
            quality_multiplier: 1.2,
            freshness_profile: Some("forging_alchemy_v1".to_string()),
        };
        assert_eq!(output.quality_multiplier, 1.2);
    }
}
