//! plan-forge-leftovers-v1 §2.3 — forge outcome 写回玩家背包。

use valence::prelude::{EventReader, Query, Res, ResMut};

use super::artifact_meridian::{artifact_state_for_outcome, write_artifact_state_to_item};
use super::events::{ForgeBucket, ForgeOutcomeEvent};
use crate::inventory::{
    bump_revision, find_free_slot, ContainerState, InventoryInstanceIdAllocator, ItemInstance,
    ItemRegistry, PlacedItemState, PlayerInventory, BODY_POCKET_CONTAINER_ID,
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
        let Some((container_index, row, col)) =
            find_forge_output_slot(&inventory, template.grid_w, template.grid_h)
        else {
            tracing::warn!(
                "[bong][forge] outcome for session {:?} caster {:?} has no free carried slot for `{}`; grant skipped",
                event.session,
                event.caster,
                template_id
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

        let mut instance = ItemInstance {
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
            alchemy: None,
            lingering_owner_qi: None,
        };

        if template.weapon_spec.is_some()
            || crate::combat::carrier::CarrierKind::from_template_id(template_id).is_some()
        {
            let state = artifact_state_for_outcome(
                template_id,
                achieved_tier,
                event.quality,
                event.color,
                event.consecration_qi_amount,
                0,
            );
            write_artifact_state_to_item(&mut instance, &state);
        }

        inventory.containers[container_index]
            .items
            .push(PlacedItemState { row, col, instance });
        bump_revision(&mut inventory);
    }
}

fn valid_achieved_tier(value: u8) -> Option<u8> {
    (1..=4).contains(&value).then_some(value)
}

fn forge_container_order(containers: &[ContainerState]) -> Vec<usize> {
    containers
        .iter()
        .enumerate()
        .filter_map(|(index, container)| {
            (container.id != BODY_POCKET_CONTAINER_ID).then_some(index)
        })
        .chain(
            containers
                .iter()
                .enumerate()
                .filter_map(|(index, container)| {
                    (container.id == BODY_POCKET_CONTAINER_ID).then_some(index)
                }),
        )
        .collect()
}

fn find_forge_output_slot(
    inventory: &PlayerInventory,
    grid_w: u8,
    grid_h: u8,
) -> Option<(usize, u8, u8)> {
    for container_index in forge_container_order(&inventory.containers) {
        if let Some((row, col)) =
            find_free_slot(&inventory.containers[container_index], grid_w, grid_h)
        {
            return Some((container_index, row, col));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::ColorKind;
    use crate::forge::blueprint::BlueprintId;
    use crate::forge::session::ForgeSessionId;
    use crate::inventory::{
        instantiate_inventory_from_loadout, load_default_loadout, load_item_registry,
        ContainerState, InventoryRevision, ItemCategory, ItemRarity, ItemTemplate, WeaponSpec,
        BODY_POCKET_CONTAINER_ID, MAIN_PACK_CONTAINER_ID,
    };
    use std::collections::HashMap;
    use valence::prelude::{App, Entity, Update};

    fn weapon_template(id: &str) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: id.to_string(),
            category: ItemCategory::Weapon,
            placeable: None,
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
            technique_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shield_spec: None,
            shelflife_track: None,
        }
    }

    fn misc_template(id: &str) -> ItemTemplate {
        ItemTemplate {
            weapon_spec: None,
            category: ItemCategory::Misc,
            placeable: None,
            max_stack_count: 1,
            ..weapon_template(id)
        }
    }

    fn tool_template(id: &str) -> ItemTemplate {
        ItemTemplate {
            weapon_spec: None,
            category: ItemCategory::Tool,
            placeable: None,
            max_stack_count: 1,
            grid_h: 1,
            ..weapon_template(id)
        }
    }

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(7),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "main_pack".to_string(),
                rows: 5,
                cols: 7,
                items: Vec::new(),
                owner_instance_id: None,
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    fn filler_instance(instance_id: u64, template_id: &str) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn full_carried_inventory() -> PlayerInventory {
        let mut inventory = empty_inventory();
        inventory.containers[0].rows = 1;
        inventory.containers[0].cols = 1;
        inventory.containers[0].items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: filler_instance(9_001, "filler"),
        });
        inventory.containers.push(ContainerState {
            id: BODY_POCKET_CONTAINER_ID.to_string(),
            name: BODY_POCKET_CONTAINER_ID.to_string(),
            rows: 1,
            cols: 1,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: filler_instance(9_002, "pocket_filler"),
            }],
            owner_instance_id: None,
        });
        inventory
    }

    fn app_with_templates(templates: HashMap<String, ItemTemplate>) -> App {
        app_with_registry(ItemRegistry::from_map(templates))
    }

    fn app_with_registry(registry: ItemRegistry) -> App {
        let mut app = App::new();
        app.insert_resource(registry);
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
            consecration_qi_amount: 100.0,
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
    fn outcome_with_default_runtime_pack_without_main_pack_is_not_lost() {
        let item_registry = load_item_registry().expect("item registry should load");
        let loadout = load_default_loadout(&item_registry).expect("default loadout should load");
        let mut loadout_allocator = InventoryInstanceIdAllocator::new(3_000);
        let inventory =
            instantiate_inventory_from_loadout(&loadout, &mut loadout_allocator, &item_registry)
                .expect("default loadout should instantiate");
        let runtime_pack_id = inventory
            .containers
            .iter()
            .find_map(|container| {
                container
                    .id
                    .strip_prefix("pack_")
                    .map(|_| container.id.clone())
            })
            .expect("default loadout should derive a runtime pack_<instance_id> container");
        assert!(
            inventory
                .containers
                .iter()
                .all(|container| container.id != MAIN_PACK_CONTAINER_ID),
            "default loadout no longer creates `{MAIN_PACK_CONTAINER_ID}`; ids={:?}",
            inventory
                .containers
                .iter()
                .map(|container| container.id.as_str())
                .collect::<Vec<_>>()
        );

        let mut app = app_with_registry(item_registry);
        let caster = app.world_mut().spawn(inventory).id();

        app.world_mut()
            .send_event(outcome(caster, ForgeBucket::Perfect, Some("bone_sword")));
        app.update();

        let inventory = app.world().get::<PlayerInventory>(caster).unwrap();
        let target_container = inventory
            .containers
            .iter()
            .find(|container| {
                container
                    .items
                    .iter()
                    .any(|placed| placed.instance.template_id == "bone_sword")
            })
            .expect("forged weapon should be granted into some carried container");
        assert_eq!(
            target_container.id, BODY_POCKET_CONTAINER_ID,
            "默认 `{runtime_pack_id}` 已碎片化到放不下 1x2 forged weapon 时，应落入 body_pocket 兜底而不是丢失"
        );
        assert!(
            target_container
                .items
                .iter()
                .any(|placed| placed.instance.template_id == "bone_sword"),
            "锻造产物必须进入随身容器，不能因缺 `{MAIN_PACK_CONTAINER_ID}` 或 `{runtime_pack_id}` 放不下而静默丢失；\
             当前目标容器 items={:?}",
            target_container
                .items
                .iter()
                .map(|placed| placed.instance.template_id.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            inventory.revision,
            InventoryRevision(2),
            "锻造产物成功入包后应递增 revision；若缺 `{MAIN_PACK_CONTAINER_ID}` 直接 continue 会保持默认 revision"
        );
    }

    #[test]
    fn outcome_with_default_runtime_pack_uses_pack_when_item_fits() {
        let item_registry = load_item_registry().expect("item registry should load");
        let loadout = load_default_loadout(&item_registry).expect("default loadout should load");
        let mut loadout_allocator = InventoryInstanceIdAllocator::new(3_100);
        let inventory =
            instantiate_inventory_from_loadout(&loadout, &mut loadout_allocator, &item_registry)
                .expect("default loadout should instantiate");
        let runtime_pack_id = inventory
            .containers
            .iter()
            .find_map(|container| {
                container
                    .id
                    .strip_prefix("pack_")
                    .map(|_| container.id.clone())
            })
            .expect("default loadout should derive a runtime pack_<instance_id> container");

        let mut app = app_with_registry(item_registry);
        let caster = app.world_mut().spawn(inventory).id();

        app.world_mut()
            .send_event(outcome(caster, ForgeBucket::Good, Some("cai_yao_dao")));
        app.update();

        let inventory = app.world().get::<PlayerInventory>(caster).unwrap();
        let runtime_pack = inventory
            .containers
            .iter()
            .find(|container| container.id == runtime_pack_id)
            .expect("runtime pack container should still exist");
        assert!(
            runtime_pack
                .items
                .iter()
                .any(|placed| placed.instance.template_id == "cai_yao_dao"),
            "1x1 锻造工具应优先落入默认运行时背包 `{runtime_pack_id}`；当前 items={:?}",
            runtime_pack
                .items
                .iter()
                .map(|placed| placed.instance.template_id.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn outcome_with_no_free_carried_slot_does_not_mutate_inventory() {
        let mut templates = HashMap::new();
        templates.insert("cai_yao_dao".to_string(), tool_template("cai_yao_dao"));
        let mut app = app_with_templates(templates);
        let caster = app.world_mut().spawn(full_carried_inventory()).id();

        app.world_mut()
            .send_event(outcome(caster, ForgeBucket::Good, Some("cai_yao_dao")));
        app.update();

        let inventory = app.world().get::<PlayerInventory>(caster).unwrap();
        assert_eq!(
            inventory.revision,
            InventoryRevision(7),
            "没有任何随身空槽时锻造产物应 skip，不能递增 revision"
        );
        assert!(
            inventory.containers.iter().all(|container| {
                container
                    .items
                    .iter()
                    .all(|placed| placed.instance.template_id != "cai_yao_dao")
            }),
            "没有任何随身空槽时不能把锻造产物硬塞进已有格子"
        );
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
        assert!(item
            .forge_side_effects
            .iter()
            .any(|tag| tag == "brittle_edge"));
        assert!(item
            .forge_side_effects
            .iter()
            .any(|tag| tag.starts_with(crate::forge::artifact_meridian::ARTIFACT_STATE_PREFIX)));
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
    fn forged_weapon_includes_artifact_state_tag() {
        let mut templates = HashMap::new();
        templates.insert("bone_sword".to_string(), weapon_template("bone_sword"));
        let mut app = app_with_templates(templates);
        let caster = app.world_mut().spawn(empty_inventory()).id();
        let mut event = outcome(caster, ForgeBucket::Good, Some("bone_sword"));
        event.color = Some(ColorKind::Solid);

        app.world_mut().send_event(event);
        app.update();

        let inventory = app.world().get::<PlayerInventory>(caster).unwrap();
        let item = &inventory.containers[0].items[0].instance;
        assert!(
            item.forge_side_effects
                .iter()
                .any(|tag| tag.starts_with(crate::forge::artifact_meridian::ARTIFACT_STATE_PREFIX)),
            "forged weapon should carry serialized artifact state"
        );
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
