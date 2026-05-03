//! plan-forge-leftovers-v1 §2.3 — forge outcome 写回玩家背包。

use valence::prelude::{EventReader, Query, Res, ResMut};

use super::events::{ForgeBucket, ForgeOutcomeEvent};
use crate::inventory::{
    bump_revision, InventoryInstanceIdAllocator, ItemInstance, ItemRegistry, PlacedItemState,
    PlayerInventory, MAIN_PACK_CONTAINER_ID,
};

pub fn forge_outcome_to_inventory(
    mut events: EventReader<ForgeOutcomeEvent>,
    registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut inventories: Query<&mut PlayerInventory>,
) {
    for event in events.read() {
        if !matches!(
            event.bucket,
            ForgeBucket::Perfect | ForgeBucket::Good | ForgeBucket::Flawed
        ) {
            continue;
        }

        let Some(template_id) = event.weapon_item.as_deref() else {
            tracing::warn!(
                "[bong][forge] outcome {:?} for session {:?} has no weapon_item; inventory grant skipped",
                event.bucket,
                event.session
            );
            continue;
        };
        if !event.quality.is_finite() {
            tracing::warn!(
                "[bong][forge] outcome for session {:?} has non-finite quality {}; inventory grant skipped",
                event.session,
                event.quality
            );
            continue;
        }
        let Some(achieved_tier) = valid_achieved_tier(event.achieved_tier) else {
            tracing::warn!(
                "[bong][forge] outcome for session {:?} has invalid achieved_tier {}; inventory grant skipped",
                event.session,
                event.achieved_tier
            );
            continue;
        };

        let Some(template) = registry.get(template_id) else {
            tracing::warn!(
                "[bong][forge] outcome for session {:?} references unknown item `{}`; inventory grant skipped",
                event.session,
                template_id
            );
            continue;
        };
        if template.weapon_spec.is_none()
            && !matches!(
                template.category,
                crate::inventory::ItemCategory::Tool | crate::inventory::ItemCategory::Treasure
            )
        {
            tracing::warn!(
                "[bong][forge] outcome for session {:?} references non-craftable item `{}`; inventory grant skipped",
                event.session,
                template_id
            );
            continue;
        }

        let Ok(mut inventory) = inventories.get_mut(event.caster) else {
            tracing::warn!(
                "[bong][forge] outcome for session {:?} caster {:?} has no inventory; grant skipped",
                event.session,
                event.caster
            );
            continue;
        };
        let Some(main_pack_index) = inventory
            .containers
            .iter()
            .position(|container| container.id == MAIN_PACK_CONTAINER_ID)
        else {
            tracing::warn!(
                "[bong][forge] outcome for session {:?} caster {:?} missing main_pack; grant skipped",
                event.session,
                event.caster
            );
            continue;
        };

        let instance_id = match allocator.next_id() {
            Ok(id) => id,
            Err(err) => {
                tracing::warn!(
                    "[bong][forge] outcome for session {:?} could not allocate inventory id: {err}",
                    event.session
                );
                continue;
            }
        };

        let instance = ItemInstance {
            instance_id,
            template_id: template.id.clone(),
            display_name: template.display_name.clone(),
            grid_w: template.grid_w,
            grid_h: template.grid_h,
            weight: template.base_weight,
            rarity: template.rarity,
            description: template.description.clone(),
            stack_count: 1,
            spirit_quality: template.spirit_quality_initial,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: Some(event.quality.clamp(0.0, 1.0)),
            forge_color: event.color,
            forge_side_effects: event.side_effects.clone(),
            forge_achieved_tier: Some(achieved_tier),
        };

        inventory.containers[main_pack_index]
            .items
            .push(PlacedItemState {
                row: 0,
                col: 0,
                instance,
            });
        bump_revision(&mut inventory);
    }
}

fn valid_achieved_tier(value: u8) -> Option<u8> {
    (1..=4).contains(&value).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::ColorKind;
    use crate::forge::blueprint::BlueprintId;
    use crate::forge::session::ForgeSessionId;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemCategory, ItemRarity, ItemTemplate, WeaponSpec,
    };
    use std::collections::HashMap;
    use valence::prelude::{App, Entity, Update};

    fn weapon_template(id: &str) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: id.to_string(),
            category: ItemCategory::Weapon,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 2,
            base_weight: 1.5,
            rarity: ItemRarity::Uncommon,
            spirit_quality_initial: 1.0,
            description: String::new(),
            effect: None,
            cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
            cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
            weapon_spec: Some(WeaponSpec {
                weapon_kind: crate::combat::weapon::WeaponKind::Sword,
                base_attack: 12.0,
                quality_tier: 1,
                durability_max: 400.0,
                qi_cost_mul: 1.0,
            }),
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
        }
    }

    fn misc_template(id: &str) -> ItemTemplate {
        ItemTemplate {
            weapon_spec: None,
            category: ItemCategory::Misc,
            max_stack_count: 1,
            ..weapon_template(id)
        }
    }

    fn tool_template(id: &str) -> ItemTemplate {
        ItemTemplate {
            weapon_spec: None,
            category: ItemCategory::Tool,
            max_stack_count: 1,
            grid_h: 1,
            ..weapon_template(id)
        }
    }

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(7),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "main_pack".to_string(),
                rows: 5,
                cols: 7,
                items: Vec::new(),
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    fn app_with_templates(templates: HashMap<String, ItemTemplate>) -> App {
        let mut app = App::new();
        app.insert_resource(ItemRegistry::from_map(templates));
        app.insert_resource(InventoryInstanceIdAllocator::new(100));
        app.add_event::<ForgeOutcomeEvent>();
        app.add_systems(Update, forge_outcome_to_inventory);
        app
    }

    fn outcome(
        caster: Entity,
        bucket: ForgeBucket,
        weapon_item: Option<&str>,
    ) -> ForgeOutcomeEvent {
        ForgeOutcomeEvent {
            session: ForgeSessionId(9),
            caster,
            blueprint: BlueprintId::from("ling_feng_v0"),
            bucket,
            weapon_item: weapon_item.map(str::to_string),
            quality: 0.93,
            color: None,
            side_effects: Vec::new(),
            achieved_tier: 2,
        }
    }

    #[test]
    fn outcome_perfect_gives_weapon_with_quality() {
        let mut templates = HashMap::new();
        templates.insert(
            "ling_feng_sword".to_string(),
            weapon_template("ling_feng_sword"),
        );
        let mut app = app_with_templates(templates);
        let caster = app.world_mut().spawn(empty_inventory()).id();

        app.world_mut().send_event(outcome(
            caster,
            ForgeBucket::Perfect,
            Some("ling_feng_sword"),
        ));
        app.update();

        let inventory = app.world().get::<PlayerInventory>(caster).unwrap();
        let item = &inventory.containers[0].items[0].instance;
        assert_eq!(item.instance_id, 100);
        assert_eq!(item.template_id, "ling_feng_sword");
        assert_eq!(item.forge_quality, Some(0.93));
        assert_eq!(item.forge_achieved_tier, Some(2));
        assert_eq!(inventory.revision, InventoryRevision(8));
    }

    #[test]
    fn outcome_flawed_includes_side_effects() {
        let mut templates = HashMap::new();
        templates.insert("iron_sword".to_string(), weapon_template("iron_sword"));
        let mut app = app_with_templates(templates);
        let caster = app.world_mut().spawn(empty_inventory()).id();
        let mut event = outcome(caster, ForgeBucket::Flawed, Some("iron_sword"));
        event.side_effects = vec!["brittle_edge".to_string()];

        app.world_mut().send_event(event);
        app.update();

        let inventory = app.world().get::<PlayerInventory>(caster).unwrap();
        let item = &inventory.containers[0].items[0].instance;
        assert_eq!(item.template_id, "iron_sword");
        assert_eq!(item.forge_side_effects, vec!["brittle_edge".to_string()]);
    }

    #[test]
    fn outcome_waste_gives_nothing() {
        let mut templates = HashMap::new();
        templates.insert(
            "ling_feng_sword".to_string(),
            weapon_template("ling_feng_sword"),
        );
        let mut app = app_with_templates(templates);
        let caster = app.world_mut().spawn(empty_inventory()).id();

        app.world_mut()
            .send_event(outcome(caster, ForgeBucket::Waste, Some("ling_feng_sword")));
        app.update();

        let inventory = app.world().get::<PlayerInventory>(caster).unwrap();
        assert!(inventory.containers[0].items.is_empty());
        assert_eq!(inventory.revision, InventoryRevision(7));
    }

    #[test]
    fn outcome_explode_only_wears_station() {
        let mut templates = HashMap::new();
        templates.insert(
            "ling_feng_sword".to_string(),
            weapon_template("ling_feng_sword"),
        );
        let mut app = app_with_templates(templates);
        let caster = app.world_mut().spawn(empty_inventory()).id();

        app.world_mut().send_event(outcome(
            caster,
            ForgeBucket::Explode,
            Some("ling_feng_sword"),
        ));
        app.update();

        let inventory = app.world().get::<PlayerInventory>(caster).unwrap();
        assert!(inventory.containers[0].items.is_empty());
        assert_eq!(inventory.revision, InventoryRevision(7));
    }

    #[test]
    fn outcome_consecration_writes_color() {
        let mut templates = HashMap::new();
        templates.insert(
            "ling_feng_sword".to_string(),
            weapon_template("ling_feng_sword"),
        );
        let mut app = app_with_templates(templates);
        let caster = app.world_mut().spawn(empty_inventory()).id();
        let mut event = outcome(caster, ForgeBucket::Good, Some("ling_feng_sword"));
        event.color = Some(ColorKind::Sharp);

        app.world_mut().send_event(event);
        app.update();

        let inventory = app.world().get::<PlayerInventory>(caster).unwrap();
        let item = &inventory.containers[0].items[0].instance;
        assert_eq!(item.forge_color, Some(ColorKind::Sharp));
    }

    #[test]
    fn outcome_good_gives_tool_without_weapon_stats() {
        let mut templates = HashMap::new();
        templates.insert("cai_yao_dao".to_string(), tool_template("cai_yao_dao"));
        let mut app = app_with_templates(templates);
        let caster = app.world_mut().spawn(empty_inventory()).id();

        app.world_mut()
            .send_event(outcome(caster, ForgeBucket::Good, Some("cai_yao_dao")));
        app.update();

        let inventory = app.world().get::<PlayerInventory>(caster).unwrap();
        let item = &inventory.containers[0].items[0].instance;
        assert_eq!(item.template_id, "cai_yao_dao");
        assert_eq!(item.forge_quality, Some(0.93));
        assert_eq!(item.forge_achieved_tier, Some(2));
    }

    #[test]
    fn outcome_rejects_non_weapon_non_tool_template() {
        let mut templates = HashMap::new();
        templates.insert("ling_mu_ban".to_string(), misc_template("ling_mu_ban"));
        let mut app = app_with_templates(templates);
        let caster = app.world_mut().spawn(empty_inventory()).id();

        app.world_mut()
            .send_event(outcome(caster, ForgeBucket::Good, Some("ling_mu_ban")));
        app.update();

        let inventory = app.world().get::<PlayerInventory>(caster).unwrap();
        assert!(inventory.containers[0].items.is_empty());
    }

    #[test]
    fn outcome_allows_ling_xia_treasure_container() {
        let mut templates = HashMap::new();
        templates.insert("ling_xia".to_string(), {
            let mut template = misc_template("ling_xia");
            template.category = ItemCategory::Treasure;
            template
        });
        let mut app = app_with_templates(templates);
        let caster = app.world_mut().spawn(empty_inventory()).id();

        app.world_mut()
            .send_event(outcome(caster, ForgeBucket::Good, Some("ling_xia")));
        app.update();

        let inventory = app.world().get::<PlayerInventory>(caster).unwrap();
        let item = &inventory.containers[0].items[0].instance;
        assert_eq!(item.template_id, "ling_xia");
        assert_eq!(item.forge_quality, Some(0.93));
    }
}
