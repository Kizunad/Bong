//! plan-forge-leftovers-v1 §2.3 — forge outcome 写回玩家背包。

use valence::prelude::{EventReader, EventWriter, Position, Query, Res, ResMut};

use super::artifact_meridian::{artifact_state_for_outcome, write_artifact_state_to_item};
use super::events::{ForgeBucket, ForgeOutcomeEvent};
use super::session::ForgeSessions;
use super::station::WeaponForgeStation;
use crate::combat::CombatClock;
use crate::inventory::{
    add_customized_item_to_player_inventory, add_item_to_player_inventory_or_ground,
    spawn_template_dropped_loot, DroppedLootEntry, DroppedLootRegistry, GrantOrGroundOutcome,
    InventoryInstanceIdAllocator, ItemInstance, ItemRegistry, PlayerInventory,
    TemplateDroppedLootRequest,
};
use crate::mineral::MineralFeedbackEvent;
use crate::player::state::{persist_dropped_loot_entries, PlayerStatePersistence};
use crate::world::dimension::{CurrentDimension, DimensionKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForgeDropSource {
    Player,
    ForgeStation,
}

#[derive(Debug, Clone, Copy)]
struct ForgeDropTarget {
    position: [f64; 3],
    dimension: DimensionKind,
    source: ForgeDropSource,
}

#[allow(clippy::too_many_arguments)]
pub fn forge_outcome_to_inventory(
    mut events: EventReader<ForgeOutcomeEvent>,
    registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut inventories: Query<&mut PlayerInventory>,
    mut dropped_loot: ResMut<DroppedLootRegistry>,
    mut feedback: EventWriter<MineralFeedbackEvent>,
    player_positions: Query<(&Position, Option<&CurrentDimension>)>,
    combat_clock: Option<Res<CombatClock>>,
    sessions: Option<Res<ForgeSessions>>,
    stations: Query<&WeaponForgeStation>,
    persistence: Option<Res<PlayerStatePersistence>>,
) {
    for event in events.read().cloned() {
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

        let forge_quality = event.quality.clamp(0.0, 1.0);
        let forge_color = event.color;
        let forge_side_effects = event.side_effects.clone();
        let artifact_state = if template.weapon_spec.is_some()
            || crate::combat::carrier::CarrierKind::from_template_id(template_id).is_some()
        {
            Some(artifact_state_for_outcome(
                template_id,
                achieved_tier,
                event.quality,
                event.color,
                event.consecration_qi_amount,
                0,
            ))
        } else {
            None
        };
        let customize = |instance: &mut ItemInstance| {
            instance.forge_quality = Some(forge_quality);
            instance.forge_color = forge_color;
            instance.forge_side_effects = forge_side_effects.clone();
            instance.forge_achieved_tier = Some(achieved_tier);
            if let Some(state) = artifact_state.as_ref() {
                write_artifact_state_to_item(instance, state);
            }
        };
        let current_tick = combat_clock.as_ref().map_or(0, |clock| clock.tick);

        let drop_target =
            forge_drop_target(&event, &player_positions, sessions.as_deref(), &stations);
        let grant_result = if let Ok(mut inventory) = inventories.get_mut(event.caster) {
            if let Some(drop_target) = drop_target {
                add_item_to_player_inventory_or_ground(
                    &mut inventory,
                    &registry,
                    &mut allocator,
                    Some(&mut dropped_loot),
                    template_id,
                    1,
                    current_tick,
                    drop_target.position,
                    drop_target.dimension,
                    Some(&customize),
                )
            } else {
                add_customized_item_to_player_inventory(
                    &mut inventory,
                    &registry,
                    &mut allocator,
                    template_id,
                    1,
                    current_tick,
                    customize,
                )
                .map(GrantOrGroundOutcome::Granted)
            }
        } else {
            let Some(drop_target) = drop_target else {
                tracing::error!(
                    "[bong][forge] outcome for session {:?} caster {:?} has no inventory and no player or forge-station location; outcome cannot be materialized",
                    event.session,
                    event.caster
                );
                continue;
            };
            spawn_forge_outcome_dropped_loot(
                &mut dropped_loot,
                &registry,
                &mut allocator,
                template_id,
                current_tick,
                drop_target,
                &customize,
            )
            .map(|entry| GrantOrGroundOutcome::DroppedToGround(Box::new(entry)))
        };

        match grant_result {
            Ok(GrantOrGroundOutcome::Granted(_)) => {}
            Ok(GrantOrGroundOutcome::DroppedToGround(entry)) => {
                tracing::info!(
                    "[bong][forge] outcome for session {:?} caster {:?} full inventory; `{}` dropped at {:?}",
                    event.session,
                    event.caster,
                    template_id,
                    entry.world_pos
                );
                if let Some(persistence) = persistence.as_deref() {
                    if let Err(error) = persist_dropped_loot_entries(
                        persistence,
                        std::slice::from_ref(entry.as_ref()),
                    ) {
                        tracing::error!(
                            "[bong][forge] durable dropped-loot persistence failed for session {:?}, instance {}: {error}",
                            event.session,
                            entry.instance_id
                        );
                    }
                }
                match drop_target.map(|target| target.source) {
                    Some(ForgeDropSource::ForgeStation) => {
                        feedback.send(MineralFeedbackEvent::forge_outcome_dropped_near_station(
                            event.caster,
                            &template.display_name,
                        ));
                    }
                    _ => {
                        feedback.send(MineralFeedbackEvent::forge_outcome_dropped(
                            event.caster,
                            &template.display_name,
                        ));
                    }
                }
            }
            Err(err) => {
                tracing::error!(
                    "[bong][forge] outcome for session {:?} could not grant `{}` to inventory or ground: {err}",
                    event.session,
                    template_id
                );
            }
        }
    }
}

fn forge_drop_target(
    event: &ForgeOutcomeEvent,
    player_positions: &Query<(&Position, Option<&CurrentDimension>)>,
    sessions: Option<&ForgeSessions>,
    stations: &Query<&WeaponForgeStation>,
) -> Option<ForgeDropTarget> {
    if let Ok((position, Some(dimension))) = player_positions.get(event.caster) {
        let coordinates = [position.0.x, position.0.y, position.0.z];
        if coordinates.iter().all(|coordinate| coordinate.is_finite()) {
            return Some(ForgeDropTarget {
                position: coordinates,
                dimension: dimension.0,
                source: ForgeDropSource::Player,
            });
        }
    }

    // 用会话绑定的锻炉位置/维度作为缺失玩家位置/维度组件时的可靠落点，
    // 避免伪造固定世界坐标；站点实体仍在时优先读取其最新维度。
    let session = sessions.and_then(|sessions| sessions.get(event.session))?;
    let station = stations.get(session.station).ok();
    let station_pos = station
        .and_then(|station| station.pos)
        .or(session.station_pos)?;
    let station_dimension = station
        .map(|station| station.dimension)
        .unwrap_or(session.station_dimension);
    let (x, y, z) = station_pos;
    Some(ForgeDropTarget {
        position: [f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5],
        dimension: station_dimension,
        source: ForgeDropSource::ForgeStation,
    })
}

fn spawn_forge_outcome_dropped_loot(
    dropped_loot: &mut DroppedLootRegistry,
    registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
    template_id: &str,
    current_tick: u64,
    target: ForgeDropTarget,
    customize: &dyn Fn(&mut ItemInstance),
) -> Result<DroppedLootEntry, String> {
    let mut entry = spawn_template_dropped_loot(
        dropped_loot,
        registry,
        allocator,
        TemplateDroppedLootRequest {
            template_id,
            stack_count: 1,
            world_pos: target.position,
            dimension: target.dimension,
            current_tick,
        },
    )?;
    customize(&mut entry.item);
    entry.source_container_id = format!("forge_outcome:{template_id}");
    dropped_loot
        .entries
        .insert(entry.instance_id, entry.clone());
    Ok(entry)
}

fn valid_achieved_tier(value: u8) -> Option<u8> {
    (1..=4).contains(&value).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::ColorKind;
    use crate::forge::blueprint::BlueprintId;
    use crate::forge::session::{ForgeSession, ForgeSessionId, ForgeSessions};
    use crate::forge::station::WeaponForgeStation;
    use crate::inventory::{
        find_free_slot, instantiate_inventory_from_loadout, load_default_loadout,
        load_item_registry, ContainerState, DroppedLootRegistry, InventoryRevision, ItemCategory,
        ItemRarity, ItemTemplate, PlacedItemState, WeaponSpec, BODY_POCKET_CONTAINER_ID,
        MAIN_PACK_CONTAINER_ID,
    };
    use crate::mineral::{events::MSG_FORGE_OUTCOME_DROPPED, MineralFeedbackEvent};
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use std::collections::HashMap;
    use valence::prelude::{App, BlockPos, Entity, Events, Position, Update};

    fn weapon_template(id: &str) -> ItemTemplate {
        ItemTemplate {
            quick_use: false,
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
            readable_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shield_spec: None,
            shelflife_track: None,
            wearer_race: crate::body_plan::types::RaceGateOwned::default(),
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
            material_preparation: Default::default(),
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(7),
            containers: vec![ContainerState {
                quick_access: false,
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
            quick_access: false,
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
        app.insert_resource(DroppedLootRegistry::default());
        app.add_event::<ForgeOutcomeEvent>();
        app.add_event::<MineralFeedbackEvent>();
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
        let output_template = item_registry
            .get("cai_yao_dao")
            .expect("cai_yao_dao should exist in item registry");
        let runtime_pack_before = inventory
            .containers
            .iter()
            .find(|container| container.id == runtime_pack_id)
            .expect("runtime pack container should exist before forge outcome");
        let occupied_before = runtime_pack_before
            .items
            .iter()
            .map(|placed| (placed.row, placed.col))
            .collect::<Vec<_>>();
        let expected_slot = find_free_slot(
            runtime_pack_before,
            output_template.grid_w,
            output_template.grid_h,
        )
        .expect("default runtime pack should have a real free slot for 1x1 forged tool");
        assert!(
            !occupied_before.contains(&expected_slot),
            "测试前置条件错误：预期空槽 {expected_slot:?} 已被运行时背包 `{runtime_pack_id}` 占用"
        );

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
        let placed_tool = runtime_pack
            .items
            .iter()
            .find(|placed| placed.instance.template_id == "cai_yao_dao")
            .expect("1x1 锻造工具应优先落入默认运行时背包");
        assert_eq!(
            (placed_tool.row, placed_tool.col),
            expected_slot,
            "锻造工具应写入 find_free_slot 给出的真实空槽，不能固定覆盖 `(0, 0)`；\
             runtime_pack `{runtime_pack_id}` items={:?}",
            runtime_pack
                .items
                .iter()
                .map(|placed| (placed.instance.template_id.as_str(), placed.row, placed.col))
                .collect::<Vec<_>>()
        );
        assert!(
            !occupied_before.contains(&(placed_tool.row, placed_tool.col)),
            "锻造工具坐标不能与发放前已有物品重叠；发放前占用={occupied_before:?}，发放后 items={:?}",
            runtime_pack
                .items
                .iter()
                .map(|placed| (
                    placed.instance.template_id.as_str(),
                    placed.row,
                    placed.col
                ))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn outcome_with_no_free_carried_slot_drops_item_and_notifies_player() {
        let mut templates = HashMap::new();
        templates.insert("cai_yao_dao".to_string(), tool_template("cai_yao_dao"));
        let mut app = app_with_templates(templates);
        let caster = app
            .world_mut()
            .spawn((
                full_carried_inventory(),
                Position::new([12.0, 66.0, -3.0]),
                CurrentDimension(DimensionKind::Tsy),
            ))
            .id();

        app.world_mut()
            .send_event(outcome(caster, ForgeBucket::Good, Some("cai_yao_dao")));
        app.update();

        let inventory = app.world().get::<PlayerInventory>(caster).unwrap();
        assert_eq!(
            inventory.revision,
            InventoryRevision(7),
            "背包无空槽时产物应落地，不能伪造入包并递增 revision"
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

        let dropped = app.world().resource::<DroppedLootRegistry>();
        let entry = dropped
            .entries
            .values()
            .find(|entry| entry.item.template_id == "cai_yao_dao")
            .expect("背包满时锻造成品必须进入世界掉落注册表，不能静默丢失");
        assert_eq!(entry.world_pos, [12.0, 66.0, -3.0]);
        assert_eq!(entry.dimension, DimensionKind::Tsy);
        assert_eq!(entry.item.forge_quality, Some(0.93));
        assert_eq!(entry.item.forge_achieved_tier, Some(2));

        let feedback = app.world().resource::<Events<MineralFeedbackEvent>>();
        let messages: Vec<_> = feedback
            .iter_current_update_events()
            .filter(|event| event.player == caster)
            .collect();
        assert_eq!(messages.len(), 1, "背包满掉地时应给玩家一条可见反馈");
        assert_eq!(messages[0].message_id, MSG_FORGE_OUTCOME_DROPPED);
        assert!(messages[0].text.contains("cai_yao_dao"));
    }

    #[test]
    fn outcome_without_player_context_uses_forge_station_location() {
        let mut templates = HashMap::new();
        templates.insert("cai_yao_dao".to_string(), tool_template("cai_yao_dao"));
        let mut app = app_with_templates(templates);
        let caster = app.world_mut().spawn(full_carried_inventory()).id();
        let station = app
            .world_mut()
            .spawn(WeaponForgeStation::placed(
                BlockPos::new(20, 70, -5),
                2,
                caster,
            ))
            .id();
        let mut sessions = ForgeSessions::new();
        sessions.insert(ForgeSession::new(
            ForgeSessionId(9),
            BlueprintId::from("ling_feng_v0"),
            station,
            caster,
        ));
        app.world_mut().insert_resource(sessions);

        app.world_mut()
            .send_event(outcome(caster, ForgeBucket::Good, Some("cai_yao_dao")));
        app.update();

        let dropped = app.world().resource::<DroppedLootRegistry>();
        let entry = dropped
            .entries
            .values()
            .find(|entry| entry.item.template_id == "cai_yao_dao")
            .expect("无玩家位置时锻造成品应落在已知锻炉位置");
        assert_eq!(entry.world_pos, [20.5, 70.0, -4.5]);
        assert_eq!(entry.dimension, DimensionKind::Overworld);
    }

    #[test]
    fn outcome_without_inventory_uses_forge_station_location_without_pending_state() {
        let mut templates = HashMap::new();
        templates.insert("cai_yao_dao".to_string(), tool_template("cai_yao_dao"));
        let mut app = app_with_templates(templates);
        let caster = app.world_mut().spawn_empty().id();
        let station = app
            .world_mut()
            .spawn(WeaponForgeStation::placed(
                BlockPos::new(20, 70, -5),
                2,
                caster,
            ))
            .id();
        let mut sessions = ForgeSessions::new();
        sessions.insert(ForgeSession::new(
            ForgeSessionId(9),
            BlueprintId::from("ling_feng_v0"),
            station,
            caster,
        ));
        app.world_mut().insert_resource(sessions);

        app.world_mut()
            .send_event(outcome(caster, ForgeBucket::Good, Some("cai_yao_dao")));
        app.update();

        let dropped = app.world().resource::<DroppedLootRegistry>();
        let entry = dropped
            .entries
            .values()
            .find(|entry| entry.item.template_id == "cai_yao_dao")
            .expect("caster entity 已销毁时仍应在锻炉旁登记锻造成品");
        assert_eq!(entry.world_pos, [20.5, 70.0, -4.5]);
        assert_eq!(entry.dimension, DimensionKind::Overworld);
        assert_eq!(entry.source_container_id, "forge_outcome:cai_yao_dao");
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
