#![allow(dead_code, unused_imports)]
use super::*;
use crate::body_plan::BodyPartId;
use crate::inventory::{InventoryRevision, ItemCategory, ItemRarity, ItemTemplate, WeaponSpec};
use valence::prelude::{App, Events, Position, Update};

fn template(id: &str, name: &str, max_stack_count: u32) -> ItemTemplate {
    ItemTemplate {
        id: id.to_string(),
        display_name: name.to_string(),
        category: ItemCategory::Misc,
        placeable: None,
        max_stack_count,
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.2,
        rarity: ItemRarity::Uncommon,
        spirit_quality_initial: 1.0,
        description: name.to_string(),
        effect: None,
        cast_duration_ms: 0,
        cooldown_ms: 0,
        weapon_spec: None::<WeaponSpec>,
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

fn registry() -> ItemRegistry {
    ItemRegistry::from_map(HashMap::from([
        (
            ANQI_MATERIAL_TEMPLATE_ID.to_string(),
            template(ANQI_MATERIAL_TEMPLATE_ID, "异变兽骨", 16),
        ),
        (
            ANQI_CHARGED_TEMPLATE_ID.to_string(),
            template(ANQI_CHARGED_TEMPLATE_ID, "封元异变兽骨", 1),
        ),
    ]))
}

fn item(instance_id: u64, template_id: &str) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: template_id.to_string(),
        display_name: template_id.to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.2,
        rarity: ItemRarity::Uncommon,
        description: template_id.to_string(),
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
        alchemy: None,
        lingering_owner_qi: None,
    }
}

fn inventory_with_main_hand(template_id: &str) -> PlayerInventory {
    use crate::inventory::SlotContents;
    let mut equipped = HashMap::new();
    equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(item(7, template_id)),
    );
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers: Vec::new(),
        equipped,
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 45.0,
    }
}

fn charge_app() -> App {
    use crate::world::zone::ZoneRegistry;

    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 0 });
    app.insert_resource(registry());
    app.insert_resource(ZoneRegistry::default());
    app.add_event::<ChargeCarrierIntent>();
    app.add_event::<CarrierChargedEvent>();
    app.add_event::<CarrierChargeBeganEvent>();
    app.add_event::<CarrierChargeEndedEvent>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, (begin_charge_carrier, charge_carrier_tick));
    app
}

fn drain_charge_began(app: &mut App) -> Vec<CarrierChargeBeganEvent> {
    app.world_mut()
        .resource_mut::<Events<CarrierChargeBeganEvent>>()
        .drain()
        .collect()
}

fn drain_charge_ended(app: &mut App) -> Vec<CarrierChargeEndedEvent> {
    app.world_mut()
        .resource_mut::<Events<CarrierChargeEndedEvent>>()
        .drain()
        .collect()
}

fn spawn_charge_actor(app: &mut App) -> Entity {
    app.world_mut()
        .spawn((
            Cultivation {
                qi_current: 100.0,
                qi_max: 200.0,
                ..Default::default()
            },
            Position::new([0.0, 66.0, 0.0]),
            inventory_with_main_hand(ANQI_MATERIAL_TEMPLATE_ID),
            CarrierStore::default(),
        ))
        .id()
}

#[test]
fn transform_charged_carrier_is_non_stackable_and_bumps_revision() {
    let registry = registry();
    let mut inventory = inventory_with_main_hand(ANQI_MATERIAL_TEMPLATE_ID);

    assert!(transform_equipped_item(
        &mut inventory,
        &registry,
        CarrierSlot::MainHand,
        ANQI_CHARGED_TEMPLATE_ID
    ));

    let item = inventory
        .equipped
        .get(EQUIP_SLOT_MAIN_HAND)
        .unwrap()
        .held
        .as_ref()
        .unwrap();
    assert_eq!(item.template_id, ANQI_CHARGED_TEMPLATE_ID);
    assert_eq!(item.stack_count, 1);
    assert_eq!(inventory.revision.0, 2);
}

#[test]
fn begin_charge_channels_prepaid_qi_into_carrier_account() {
    let mut app = charge_app();
    let actor = spawn_charge_actor(&mut app);

    app.world_mut().send_event(ChargeCarrierIntent {
        carrier: actor,
        slot: Some(CarrierSlot::MainHand),
        qi_target: Some(60.0),
        issued_at_tick: 0,
    });
    app.update();

    let cultivation = app.world().get::<Cultivation>(actor).unwrap();
    assert!((cultivation.qi_current - 70.0).abs() < f64::EPSILON);

    let transfers = app.world().resource::<Events<QiTransfer>>();
    let transfer = transfers
        .iter_current_update_events()
        .find(|transfer| {
            transfer.reason == QiTransferReason::Channeling
                && transfer.to == carrier_qi_account(actor, 7)
        })
        .expect("暗器开始充能扣 prepaid_qi 后必须把真元封入 carrier container");
    assert_eq!(
        transfer.from,
        QiAccountId::player(format!("entity:{actor:?}"))
    );
    assert!((transfer.amount - 30.0).abs() < f64::EPSILON);
}

#[test]
fn interrupted_charge_releases_unsealed_prepaid_qi_to_zone() {
    let mut app = charge_app();
    let actor = spawn_charge_actor(&mut app);
    app.world_mut()
        .resource_mut::<crate::world::zone::ZoneRegistry>()
        .find_zone_mut("spawn")
        .unwrap()
        .spirit_qi = 0.0;

    app.world_mut().send_event(ChargeCarrierIntent {
        carrier: actor,
        slot: Some(CarrierSlot::MainHand),
        qi_target: Some(60.0),
        issued_at_tick: 0,
    });
    app.update();

    app.world_mut()
        .entity_mut(actor)
        .insert(Position::new([2.0, 66.0, 0.0]));
    app.world_mut().resource_mut::<CombatClock>().tick = CHARGE_DURATION_TICKS / 2;
    app.update();

    assert!(
        app.world().get::<CarrierCharging>(actor).is_none(),
        "移动中断后 CarrierCharging 必须结束"
    );
    let store = app.world().get::<CarrierStore>(actor).unwrap();
    let imprint = store
        .imprints_by_instance
        .get(&7)
        .expect("半程中断应保留已封入暗器的部分真元");
    assert!((imprint.qi_amount - 15.0).abs() < f32::EPSILON);

    let transfers = app.world().resource::<Events<QiTransfer>>();
    let transfer = transfers
        .iter_current_update_events()
        .find(|transfer| {
            transfer.reason == QiTransferReason::ReleaseToZone
                && transfer.from == carrier_qi_account(actor, 7)
        })
        .expect("移动中断时未封存的 prepaid_qi 必须释放回 zone，不能吞真元");
    assert_eq!(transfer.to, QiAccountId::zone("spawn".to_string()));
    assert!((transfer.amount - 15.0).abs() < f64::EPSILON);
}

// ── P2 后半：充能开始/结束观察事件（循环蓄力段动画停止路径 §8.1 #3）────────

/// 充能成功开始 → 恰发 1 条 CarrierChargeBeganEvent（循环段 PlayAnim 信号），
/// 此刻不得有任何 Ended（循环不能未播先停）。
#[test]
fn begin_charge_emits_charge_began_event_without_ended() {
    let mut app = charge_app();
    let actor = spawn_charge_actor(&mut app);

    app.world_mut().send_event(ChargeCarrierIntent {
        carrier: actor,
        slot: Some(CarrierSlot::MainHand),
        qi_target: Some(60.0),
        issued_at_tick: 0,
    });
    app.update();

    let began = drain_charge_began(&mut app);
    assert_eq!(
        began.len(),
        1,
        "充能开始应恰发 1 条 CarrierChargeBeganEvent（循环蓄力段动画信号），实际 {} 条",
        began.len()
    );
    assert_eq!(began[0].carrier, actor, "Began 事件应指向充能者本人");
    assert!(
        drain_charge_ended(&mut app).is_empty(),
        "充能刚开始不得发 CarrierChargeEndedEvent（循环段不能未播先停）"
    );
}

/// 充能被拒（qi_target 超上限 → begin 静默 continue）→ 不发 Began：
/// 没开始的充能不能触发循环动画。
#[test]
fn rejected_begin_charge_emits_no_began_event() {
    let mut app = charge_app();
    let actor = spawn_charge_actor(&mut app);

    app.world_mut().send_event(ChargeCarrierIntent {
        carrier: actor,
        slot: Some(CarrierSlot::MainHand),
        // default_qi_target(qi_max=200)=60，超出即拒。
        qi_target: Some(90.0),
        issued_at_tick: 0,
    });
    app.update();

    assert!(
        app.world().get::<CarrierCharging>(actor).is_none(),
        "前置：超上限 qi_target 应被拒、不插 CarrierCharging"
    );
    assert!(
        drain_charge_began(&mut app).is_empty(),
        "被拒的充能不得发 CarrierChargeBeganEvent（否则循环动画凭空开播）"
    );
}

/// 自然完成 → Ended{full_charge:true}（StopAnim+release 信号）且与
/// CarrierChargedEvent(full) 同拍。
#[test]
fn full_charge_completion_emits_ended_full() {
    let mut app = charge_app();
    let actor = spawn_charge_actor(&mut app);

    app.world_mut().send_event(ChargeCarrierIntent {
        carrier: actor,
        slot: Some(CarrierSlot::MainHand),
        qi_target: Some(60.0),
        issued_at_tick: 0,
    });
    app.update();
    drain_charge_ended(&mut app);

    app.world_mut().resource_mut::<CombatClock>().tick = CHARGE_DURATION_TICKS;
    app.update();

    let ended = drain_charge_ended(&mut app);
    assert_eq!(
        ended.len(),
        1,
        "充能自然完成应恰发 1 条 CarrierChargeEndedEvent，实际 {} 条",
        ended.len()
    );
    assert!(
        ended[0].full_charge,
        "自然完成的 Ended 必须 full_charge=true（驱动 StopAnim+release 收势）"
    );
    assert_eq!(ended[0].carrier, actor);
    let charged: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<CarrierChargedEvent>>()
        .drain()
        .collect();
    assert!(
        charged.iter().any(|event| event.full_charge),
        "前置：自然完成应同拍发出 CarrierChargedEvent(full_charge=true)"
    );
}

/// 半程移动打断 → Ended{full_charge:false}（仅 StopAnim，打断不奖励收势）。
#[test]
fn movement_interrupt_emits_ended_not_full() {
    let mut app = charge_app();
    let actor = spawn_charge_actor(&mut app);

    app.world_mut().send_event(ChargeCarrierIntent {
        carrier: actor,
        slot: Some(CarrierSlot::MainHand),
        qi_target: Some(60.0),
        issued_at_tick: 0,
    });
    app.update();
    drain_charge_ended(&mut app);

    app.world_mut()
        .entity_mut(actor)
        .insert(Position::new([2.0, 66.0, 0.0]));
    app.world_mut().resource_mut::<CombatClock>().tick = CHARGE_DURATION_TICKS / 2;
    app.update();

    let ended = drain_charge_ended(&mut app);
    assert_eq!(
        ended.len(),
        1,
        "移动打断应恰发 1 条 CarrierChargeEndedEvent，实际 {} 条",
        ended.len()
    );
    assert!(
        !ended[0].full_charge,
        "移动打断的 Ended 必须 full_charge=false（仅 StopAnim，不播 release）"
    );
}

/// 零进度立即打断（progress≈0 → 密封量≈0 走 finish_charge 早退分支，无
/// CarrierChargedEvent）→ **仍必须**发 Ended：早退路径漏发 = 循环动画永卡
/// （§8.1 #3 全退出路径覆盖的关键锚点）。
#[test]
fn zero_progress_interrupt_still_emits_ended_despite_no_charged_event() {
    let mut app = charge_app();
    let actor = spawn_charge_actor(&mut app);

    app.world_mut().send_event(ChargeCarrierIntent {
        carrier: actor,
        slot: Some(CarrierSlot::MainHand),
        qi_target: Some(60.0),
        issued_at_tick: 0,
    });
    app.update();
    drain_charge_ended(&mut app);

    // clock 仍为 0：elapsed=0 → progress_ratio=0 → qi_amount=0 → 早退分支。
    app.world_mut()
        .entity_mut(actor)
        .insert(Position::new([2.0, 66.0, 0.0]));
    app.update();

    assert!(
        app.world().get::<CarrierCharging>(actor).is_none(),
        "前置：零进度移动打断也应结束 CarrierCharging"
    );
    let charged: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<CarrierChargedEvent>>()
        .drain()
        .collect();
    assert!(
        charged.is_empty(),
        "前置：零进度打断走早退分支，不应发 CarrierChargedEvent"
    );
    let ended = drain_charge_ended(&mut app);
    assert_eq!(
        ended.len(),
        1,
        "早退分支（密封量≈0）也必须发 Ended——漏发 = 循环蓄力段动画永卡在玩家身上"
    );
    assert!(
        !ended[0].full_charge,
        "零进度打断的 Ended 必须 full_charge=false"
    );
}

#[test]
fn projectile_hit_despawns_without_damage_or_impact_on_creative_target() {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 10 });
    app.add_event::<CombatEvent>();
    app.add_event::<CarrierImpactEvent>();
    app.add_event::<ProjectileDespawnedEvent>();
    app.add_systems(Update, projectile_tick_system);

    app.world_mut().spawn((
        Position::new([0.0, 65.0, 0.0]),
        QiProjectile {
            owner: None,
            qi_payload: 20.0,
        },
        AnqiProjectileFlight {
            carrier_kind: CarrierKind::BoneChip,
            qi_color: ColorKind::Sharp,
            carrier_grade: CarrierKind::BoneChip.grade(),
            spawn_pos: DVec3::new(0.0, 65.0, 0.0),
            prev_pos: DVec3::new(0.0, 65.0, 0.0),
            velocity: DVec3::new(20.0, 0.0, 0.0),
            max_distance: ANQI_PROJECTILE_MAX_DISTANCE,
            hitbox_inflation: ANQI_HITBOX_INFLATION,
        },
    ));
    let target = app
        .world_mut()
        .spawn((
            Position::new([0.5, 64.0, 0.0]),
            Wounds::default(),
            Contamination::default(),
            GameMode::Creative,
        ))
        .id();
    let before = app
        .world()
        .entity(target)
        .get::<Wounds>()
        .unwrap()
        .health_current;

    app.update();

    let wounds = app.world().entity(target).get::<Wounds>().unwrap();
    assert_eq!(wounds.health_current, before);
    assert!(wounds.entries.is_empty());
    assert!(app.world().resource::<Events<CombatEvent>>().is_empty());
    assert!(app
        .world()
        .resource::<Events<CarrierImpactEvent>>()
        .is_empty());
    assert_eq!(
        app.world()
            .resource::<Events<ProjectileDespawnedEvent>>()
            .len(),
        1
    );
}

// ══════════════════════════════════════════════════════════════════════════
// plan-combat-hit-location-v1 P2（决议 §8.1 旁路桶 #2）— 投射命中部位几何化 pin
// ══════════════════════════════════════════════════════════════════════════

/// 构造一发沿 X 轴飞行、经过给定绝对 Y 高度的暗器投射，命中站在原点的目标。
/// `flight_y` 决定投射穿过目标 hitbox 时的高度，从而驱动 `classify_body_part`
/// 落到不同部位——用来证明命中部位不再恒为 `BodyPart::Chest`。
fn projectile_hit_body_part_at_height(flight_y: f64) -> crate::body_plan::BodyPartId {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 10 });
    app.add_event::<CombatEvent>();
    app.add_event::<CarrierImpactEvent>();
    app.add_event::<ProjectileDespawnedEvent>();
    app.add_systems(Update, projectile_tick_system);

    app.world_mut().spawn((
        Position::new([-1.0, flight_y, 0.0]),
        QiProjectile {
            owner: None,
            qi_payload: 20.0,
        },
        AnqiProjectileFlight {
            carrier_kind: CarrierKind::BoneChip,
            qi_color: ColorKind::Sharp,
            carrier_grade: CarrierKind::BoneChip.grade(),
            spawn_pos: DVec3::new(-1.0, flight_y, 0.0),
            prev_pos: DVec3::new(-1.0, flight_y, 0.0),
            velocity: DVec3::new(20.0, 0.0, 0.0),
            max_distance: ANQI_PROJECTILE_MAX_DISTANCE,
            hitbox_inflation: ANQI_HITBOX_INFLATION,
        },
    ));
    // 目标 `Position` 是脚底坐标（`classify_body_part` 的 `target_feet_position`
    // 约定，见 `raycast.rs::standing_humanoid_aabb`）；无 `GameMode` 组件即视为
    // 可被伤害（`is_damageable` 默认 true）。
    let target = app
        .world_mut()
        .spawn((
            Position::new([0.0, 0.0, 0.0]),
            Wounds::default(),
            Contamination::default(),
        ))
        .id();

    app.update();

    let wounds = app.world().entity(target).get::<Wounds>().unwrap();
    assert_eq!(
        wounds.entries.len(),
        1,
        "flight_y={flight_y} 应命中目标产生恰好一条 Wound，实测 {} 条 —— \
         若为 0 说明本次高度没有几何相交，测试几何参数需要调整",
        wounds.entries.len()
    );
    let combat_events: Vec<_> = app
        .world()
        .resource::<Events<CombatEvent>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(combat_events.len(), 1);
    // `Wound.location`（`BodyPartId`）与 `CombatEvent.body_part`（legacy `BodyPart`，
    // 边界①转换）必须是同一次 `classify_body_part` 调用结果——humanoid 部位全部能
    // 干净转换回 legacy，转换失败（非人形，本测试不涉及）会走 Chest 占位而非本断言
    // 覆盖的路径。
    assert_eq!(
        wounds.entries[0].location,
        crate::body_plan::legacy_body_part_to_id(combat_events[0].body_part),
        "Wound.location 与 CombatEvent.body_part 必须是同一个 classify_body_part \
         调用结果，实测 Wound={:?} CombatEvent={:?} 不一致",
        wounds.entries[0].location,
        combat_events[0].body_part
    );
    wounds.entries[0].location.clone()
}

#[test]
fn projectile_hit_at_head_height_classifies_head_not_chest() {
    // 目标脚底 y=0，头部阈值 rel_y>0.88 → y>1.584；投射沿 y=1.65 平飞穿过目标中心线
    // （命中判定半径 0.3+0.4=0.7，|1.65-1.0|=0.65 留够浮点误差余量）。
    let part = projectile_hit_body_part_at_height(1.65);
    assert_eq!(
        part,
        BodyPartId::new("head"),
        "投射沿头部高度（y=1.65，脚底 y=0）飞行应命中 head，实测 {part:?} —— \
         若又是 chest 说明 P2 旁路清理被回退成硬编胸口了"
    );
}

#[test]
fn projectile_hit_at_leg_height_classifies_leg_not_chest() {
    // 腿部阈值 rel_y<=0.53 → y<=0.954；投射沿 y=0.5 平飞穿过目标中心线
    // （|0.5-1.0|=0.5，同样留够命中半径 0.7 的浮点误差余量）。
    let part = projectile_hit_body_part_at_height(0.5);
    assert!(
        part == BodyPartId::new("leg_l") || part == BodyPartId::new("leg_r"),
        "投射沿腿部高度（y=0.5，脚底 y=0）飞行应命中 leg_l/leg_r，实测 {part:?} —— \
         若是 chest 说明命中部位仍是恒定胸口而非按弹道几何算出"
    );
}

#[test]
fn projectile_hit_at_chest_height_still_classifies_chest() {
    // 对照组：胸口高度（rel_y≈0.556，在 0.55~0.88 之间且 lateral 落在阈值内）仍应判 Chest，
    // 证明这不是"再也不会出现 Chest"而是"部位随几何真实变化，胸口只是其中一种可能"。
    let part = projectile_hit_body_part_at_height(1.0);
    assert_eq!(
        part,
        BodyPartId::new("chest"),
        "投射沿胸口高度（y=1.0，脚底 y=0）飞行应命中 chest，实测 {part:?}"
    );
}

// ══════════════════════════════════════════════════════════════════════════
// plan-race-system-v1 P0 review r3（blocker+major 收口）—— carrier 投射物对
// PartBoxes 目标改用弹道线段真求交（`body_plan::geometry::raycast_part_boxes`），
// 取代此前"已知命中点 + classify_part_boxes_point 就近回退"的语义缺陷（盒间空隙
// 会被伪造成有效命中）。以下测试全部走真实 `projectile_tick_system`
// 生产系统（不直接调用几何纯函数），合成非人形 PartBoxes 构型 + 真实
// BodyPlanRegistry/RaceRegistry，覆盖：①前后部位遮挡（近盒挡远盒）②平移后仍
// 正确命中 ③射线穿过空隙时跳过 Wound 构造但伤害/事件仍照常结算。
// ══════════════════════════════════════════════════════════════════════════
mod partboxes_carrier_production_integration_tests {
    use super::*;
    use crate::body_plan::race_registry::RaceEntry;
    use crate::body_plan::types::{BodyPartDef, HitGeometry, PartBox, PartConsequence};
    use crate::body_plan::{BodyPlanRegistry, RaceRegistry};
    use crate::cultivation::components::Cultivation;
    use std::collections::HashMap as StdHashMap;

    /// 双盒合成构型：`near_part`/`far_part` 沿局部 +X（=世界 +X，target yaw=0 时
    /// 局部系与世界系重合）前后排列，`near_part` 更靠近射线起点。
    fn two_box_plan(near_part: &str, far_part: &str) -> crate::body_plan::BodyPlan {
        crate::body_plan::BodyPlan {
            id: format!("test_carrier_two_box_{near_part}_{far_part}").into(),
            display_name: "测试用 carrier 双盒构型".to_string(),
            is_humanoid: false,
            parts: vec![
                BodyPartDef {
                    id: near_part.into(),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Core,
                },
                BodyPartDef {
                    id: far_part.into(),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Core,
                },
            ],
            hit_geometry: HitGeometry::PartBoxes {
                boxes: vec![
                    // 局部 offset y=1.0 对齐粗筛 capsule 判定用的 target_center
                    // （`target_pos + (0,1,0)`），确保粗筛与精细求交在同一高度。
                    PartBox {
                        part_id: near_part.into(),
                        offset: [0.0, 1.0, 0.0],
                        half_extents: [0.3, 0.3, 0.3],
                        priority: 0,
                    },
                    PartBox {
                        part_id: far_part.into(),
                        offset: [1.5, 1.0, 0.0],
                        half_extents: [0.3, 0.3, 0.3],
                        priority: 0,
                    },
                ],
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: StdHashMap::new(),
        }
    }

    /// 空隙构型：唯一的盒偏移远离射线路径（局部 x=5.0，射线只走到 x≈2.0 就结束），
    /// 粗筛 capsule（只判定到 target_center 点的距离，与盒位置无关）仍会命中，
    /// 但精细 PartBoxes 求交必须落空。
    fn gap_plan(part_id: &str) -> crate::body_plan::BodyPlan {
        crate::body_plan::BodyPlan {
            id: format!("test_carrier_gap_{part_id}").into(),
            display_name: "测试用 carrier 空隙构型".to_string(),
            is_humanoid: false,
            parts: vec![BodyPartDef {
                id: part_id.into(),
                damage_mul: 1.0,
                contam_mul: 1.0,
                bleed_mul: 1.0,
                consequence: PartConsequence::Core,
            }],
            hit_geometry: HitGeometry::PartBoxes {
                boxes: vec![PartBox {
                    part_id: part_id.into(),
                    offset: [5.0, 1.0, 0.0],
                    half_extents: [0.3, 0.3, 0.3],
                    priority: 0,
                }],
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: StdHashMap::new(),
        }
    }

    fn registries_for(plan: crate::body_plan::BodyPlan) -> (BodyPlanRegistry, RaceRegistry) {
        let plan_id = plan.id.clone();
        let body_plans =
            BodyPlanRegistry::from_plans(vec![plan]).expect("synthetic carrier plan must validate");
        let races = RaceRegistry::from_parts_for_test(
            vec![RaceEntry {
                id: crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                display_name: "carrier PartBoxes 测试替身".to_string(),
                body_plan_id: plan_id,
                beast_kinds: vec![],
            }],
            vec![],
            &body_plans,
        )
        .expect("races fixture must validate");
        (body_plans, races)
    }

    /// 组装最小 App：合成 registries + `projectile_tick_system` + 一发沿世界 +X
    /// 飞行的投射物 + 一个携带 `Cultivation::default()`（race 解析到合成 plan）的
    /// 目标。`target_feet` 允许任意平移，验证生产链路的世界→局部变换真的用了
    /// 目标的实际位置，而不是隐式假设原点。
    fn run_projectile_at_target(
        plan: crate::body_plan::BodyPlan,
        target_feet: DVec3,
    ) -> (Wounds, Vec<CombatEvent>, Vec<ProjectileDespawnedEvent>) {
        let (body_plans, races) = registries_for(plan);
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 900 });
        app.insert_resource(body_plans);
        app.insert_resource(races);
        app.add_event::<CombatEvent>();
        app.add_event::<CarrierImpactEvent>();
        app.add_event::<ProjectileDespawnedEvent>();
        app.add_systems(Update, projectile_tick_system);

        // 射线沿世界 +X：spawn 于 target 局部 x=-3（射线起点），一 tick 内飞抵
        // 局部 x=+2（速度 100，dt=1/20s，单 tick 位移 5.0 blocks），覆盖两盒
        // 构型的 near(x∈[-0.3,0.3])/far(x∈[1.2,1.8]) 与空隙构型的射线终点(x=2)
        // 均落在 gap 盒(x∈[4.7,5.3])之外。
        let spawn_pos = target_feet + DVec3::new(-3.0, 1.0, 0.0);
        app.world_mut().spawn((
            Position::new([spawn_pos.x, spawn_pos.y, spawn_pos.z]),
            QiProjectile {
                owner: None,
                qi_payload: 20.0,
            },
            AnqiProjectileFlight {
                carrier_kind: CarrierKind::BoneChip,
                qi_color: ColorKind::Sharp,
                carrier_grade: CarrierKind::BoneChip.grade(),
                spawn_pos,
                prev_pos: spawn_pos,
                velocity: DVec3::new(100.0, 0.0, 0.0),
                max_distance: ANQI_PROJECTILE_MAX_DISTANCE,
                hitbox_inflation: ANQI_HITBOX_INFLATION,
            },
        ));
        let target = app
            .world_mut()
            .spawn((
                Position::new([target_feet.x, target_feet.y, target_feet.z]),
                Wounds::default(),
                Contamination::default(),
                Cultivation::default(),
            ))
            .id();

        app.update();

        let wounds = app.world().entity(target).get::<Wounds>().unwrap().clone();
        let combat_events: Vec<CombatEvent> = app
            .world()
            .resource::<Events<CombatEvent>>()
            .iter_current_update_events()
            .cloned()
            .collect();
        let despawns: Vec<ProjectileDespawnedEvent> = app
            .world()
            .resource::<Events<ProjectileDespawnedEvent>>()
            .iter_current_update_events()
            .cloned()
            .collect();
        (wounds, combat_events, despawns)
    }

    #[test]
    fn near_box_occludes_far_box_at_origin() {
        let plan = two_box_plan("near_plate", "far_plate");
        let (wounds, _events, _despawns) =
            run_projectile_at_target(plan, DVec3::new(0.0, 64.0, 0.0));
        assert_eq!(
            wounds.entries.len(),
            1,
            "PartBoxes 真求交应恰好命中一个部位，实测 {:?}",
            wounds.entries
        );
        assert_eq!(
            wounds.entries[0].location,
            BodyPartId::new("near_plate"),
            "两盒都在射线路径上时，near_plate（离投射起点更近）必须遮挡 far_plate，\
             实测命中 {:?}",
            wounds.entries[0].location
        );
    }

    #[test]
    fn near_box_occludes_far_box_after_target_translation() {
        // 与上一测试几何完全相同，唯一变量是目标整体平移到远离原点的坐标——
        // 证明生产链路的世界→局部变换用的是目标实际位置，不是隐式硬编码原点。
        let plan = two_box_plan("near_plate", "far_plate");
        let (wounds, _events, _despawns) =
            run_projectile_at_target(plan, DVec3::new(437.0, 64.0, -812.0));
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(
            wounds.entries[0].location,
            BodyPartId::new("near_plate"),
            "平移后仍应命中 near_plate（局部系不变性在生产链路中成立），实测 {:?}",
            wounds.entries[0].location
        );
    }

    #[test]
    fn ray_through_partboxes_gap_skips_wound_but_still_applies_damage() {
        let plan = gap_plan("shell");
        let (wounds, combat_events, despawns) =
            run_projectile_at_target(plan, DVec3::new(0.0, 64.0, 0.0));

        assert!(
            wounds.entries.is_empty(),
            "弹道穿过 PartBoxes 空隙必须跳过 Wound 构造，不伪造命中部位，实测 {:?}",
            wounds.entries
        );
        assert!(
            wounds.health_current < Wounds::default().health_max,
            "即便跳过 Wound 构造，粗筛已确认的真实物理接触仍应照常结算伤害（health_current \
             应低于满血），实测 {}",
            wounds.health_current
        );
        assert_eq!(
            combat_events.len(),
            1,
            "空隙命中仍应发出恰好一条 CombatEvent（伤害/事件照常结算），实测 {combat_events:?}"
        );
        assert_eq!(
            combat_events[0].body_part,
            crate::combat::components::BodyPart::Chest,
            "空隙命中的 CombatEvent.body_part 应落回 Chest 占位（显式 fallback，非静默默认）"
        );
        assert_eq!(
            despawns.len(),
            1,
            "空隙命中仍应作为 HitTarget 消耗投射物（despawn 恰好一次），实测 {despawns:?}"
        );
        assert_eq!(despawns[0].reason, ProjectileDespawnReason::HitTarget);
    }

    // ══════════════════════════════════════════════════════════════════════
    // plan-race-system-v1 P5/PR-6c —— 粗筛半径按目标 body_plan 动态派生，用真实
    // whale.json 目标验证：换轨前写死的 `0.3+ANQI_HITBOX_INFLATION=0.7` 判定不到
    // 的横向偏移，换轨后（whale 半径 ≈5.74）必须能命中；humanoid 目标半径不回归。
    // ══════════════════════════════════════════════════════════════════════

    /// 加载真实磁盘 `assets/body_plans/plans/*.json` + `races.json`（而非合成
    /// fixture）——本组测试要断言的是真实落盘 whale.json 数据驱动出的粗筛半径，
    /// 不是任意手搓的 PartBoxes 构型。
    fn real_registries() -> (BodyPlanRegistry, RaceRegistry) {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let plans_dir = manifest_dir.join(crate::body_plan::registry::DEFAULT_BODY_PLANS_DIR);
        let races_path = manifest_dir.join(crate::body_plan::race_registry::DEFAULT_RACES_PATH);
        let body_plans = BodyPlanRegistry::load_dir(&plans_dir).expect("real plans/ should load");
        let races =
            RaceRegistry::load_file(&races_path, &body_plans).expect("real races.json should load");
        (body_plans, races)
    }

    /// 组装一发沿世界 +X 飞行、在 `lateral_z_offset` 处经过目标的投射，目标携带
    /// 给定 `race` 的 `Cultivation`（走真实 registries 解析 body_plan）。
    fn run_lateral_projectile_at_real_target(
        race: crate::body_plan::RaceId,
        lateral_z_offset: f64,
    ) -> (Wounds, Vec<CombatEvent>, Vec<ProjectileDespawnedEvent>) {
        let (body_plans, races) = real_registries();
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 900 });
        app.insert_resource(body_plans);
        app.insert_resource(races);
        app.add_event::<CombatEvent>();
        app.add_event::<CarrierImpactEvent>();
        app.add_event::<ProjectileDespawnedEvent>();
        app.add_systems(Update, projectile_tick_system);

        let target_feet = DVec3::new(0.0, 64.0, 0.0);
        // 粗筛 capsule 的固定参照点是 `target_pos + (0,1,0)`，横向偏移全部落在 z
        // 轴（射线沿世界 +X 飞行，不与该参照点在 x 上重合，只在 z 上偏移）。
        let spawn_pos = DVec3::new(-3.0, target_feet.y + 1.0, lateral_z_offset);
        app.world_mut().spawn((
            Position::new([spawn_pos.x, spawn_pos.y, spawn_pos.z]),
            QiProjectile {
                owner: None,
                qi_payload: 20.0,
            },
            AnqiProjectileFlight {
                carrier_kind: CarrierKind::BoneChip,
                qi_color: ColorKind::Sharp,
                carrier_grade: CarrierKind::BoneChip.grade(),
                spawn_pos,
                prev_pos: spawn_pos,
                velocity: DVec3::new(100.0, 0.0, 0.0),
                max_distance: ANQI_PROJECTILE_MAX_DISTANCE,
                hitbox_inflation: ANQI_HITBOX_INFLATION,
            },
        ));
        let target = app
            .world_mut()
            .spawn((
                Position::new([target_feet.x, target_feet.y, target_feet.z]),
                Wounds::default(),
                Contamination::default(),
                Cultivation {
                    race,
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        let wounds = app.world().entity(target).get::<Wounds>().unwrap().clone();
        let combat_events: Vec<CombatEvent> = app
            .world()
            .resource::<Events<CombatEvent>>()
            .iter_current_update_events()
            .cloned()
            .collect();
        let despawns: Vec<ProjectileDespawnedEvent> = app
            .world()
            .resource::<Events<ProjectileDespawnedEvent>>()
            .iter_current_update_events()
            .cloned()
            .collect();
        (wounds, combat_events, despawns)
    }

    #[test]
    fn humanoid_target_bounding_radius_does_not_regress_beyond_legacy_fixed_value() {
        // 换轨前写死的判定半径是 0.3（STANDING_HALF_WIDTH 字面量）+
        // ANQI_HITBOX_INFLATION(0.4) = 0.7。z 偏移 0.6 应命中（<0.7），0.8 应未命中
        // （>0.7）——humanoid 目标必须与换轨前 bit-for-bit 一致的判定边界。
        let (_, _, despawns_hit) = run_lateral_projectile_at_real_target(
            crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID),
            0.6,
        );
        assert_eq!(
            despawns_hit.first().map(|d| d.reason),
            Some(ProjectileDespawnReason::HitTarget),
            "humanoid 目标 z 偏移 0.6（< 换轨前固定阈值 0.7）必须命中，实测 {despawns_hit:?}"
        );

        let (_, _, despawns_miss) = run_lateral_projectile_at_real_target(
            crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID),
            0.8,
        );
        assert!(
            despawns_miss
                .iter()
                .all(|d| d.reason != ProjectileDespawnReason::HitTarget),
            "humanoid 目标 z 偏移 0.8（> 换轨前固定阈值 0.7）单 tick 内不应产生 HitTarget \
             despawn，实测 {despawns_miss:?}"
        );
    }

    #[test]
    fn whale_target_bounding_radius_catches_offsets_that_would_miss_the_legacy_fixed_value() {
        // whale.json 粗筛半径 ≈5.74（tail_fin 局部 z=-3.74±half 2.0）远大于换轨前
        // 写死的 0.3——z 偏移 2.0 远超换轨前固定阈值 0.7（必定会被误判为未命中），
        // 换轨后 whale 目标必须能命中。
        let (wounds, combat_events, despawns) =
            run_lateral_projectile_at_real_target(crate::body_plan::RaceId::new("whale"), 2.0);
        assert_eq!(
            despawns.first().map(|d| d.reason),
            Some(ProjectileDespawnReason::HitTarget),
            "whale 目标 z 偏移 2.0（换轨前固定阈值 0.7 判不到，换轨后动态半径应判到）必须命中，\
             实测 {despawns:?}"
        );
        assert_eq!(
            combat_events.len(),
            1,
            "whale 目标命中仍应发出恰好一条 CombatEvent，实测 {combat_events:?}"
        );
        assert!(
            wounds.health_current < Wounds::default().health_max,
            "whale 目标命中应造成伤害（health_current 低于满血），实测 {}",
            wounds.health_current
        );
    }
}

#[test]
fn carrier_charge_qi_uses_artifact_resonance_efficiency() {
    assert_eq!(carrier_sealed_qi_amount(50.0, None), 50.0);
    assert!((carrier_sealed_qi_amount(50.0, Some(0.0)) - 40.0).abs() <= 0.001);
    assert!((carrier_sealed_qi_amount(50.0, Some(1.0)) - 60.0).abs() <= 0.001);
}

// ── qc-P0 守恒测试：projectile_miss_qi_release_system ──────────────────────────

/// 辅助：构建带 ZoneRegistry + QiTransfer 事件的 App 并注册 miss-release 系统。
fn miss_release_app() -> App {
    use crate::qi_physics::ledger::QiTransfer;
    use crate::world::zone::ZoneRegistry;

    let mut app = App::new();
    app.add_event::<ProjectileDespawnedEvent>();
    app.add_event::<QiTransfer>();
    app.insert_resource(ZoneRegistry::default()); // 含默认 spawn zone
    app.add_systems(Update, projectile_miss_qi_release_system);
    app
}

fn spawn_entity(app: &mut App) -> Entity {
    app.world_mut().spawn_empty().id()
}

fn make_despawn_event(
    projectile: Entity,
    owner: Option<Entity>,
    residual_qi: f32,
    reason: ProjectileDespawnReason,
) -> ProjectileDespawnedEvent {
    // spawn zone 在 DEFAULT_SPAWN_BOUNDS_MIN = [-128, 64, -128] 到 [128, 80, 128]
    // 落点 [0, 66, 0] 在 spawn zone 内。
    ProjectileDespawnedEvent {
        owner,
        projectile,
        reason,
        distance: 5.0,
        qi_evaporated: 0.7 * residual_qi / 0.3,
        residual_qi,
        pos: [0.0, 66.0, 0.0],
        tick: 10,
    }
}

#[test]
fn miss_despawn_residual_goes_to_zone_qi_increases() {
    // 期望：OutOfRange despawn，residual_qi=3.0 → spawn zone.spirit_qi 上升，
    // 因为真元从投射物归还到 zone（player cast 时已扣，此处归还 zone）。
    let mut app = miss_release_app();
    let projectile = spawn_entity(&mut app);

    let zone_before = app
        .world()
        .resource::<crate::world::zone::ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;

    app.world_mut().send_event(make_despawn_event(
        projectile,
        None,
        3.0, // residual_qi
        ProjectileDespawnReason::OutOfRange,
    ));
    app.update();

    let zone_after = app
        .world()
        .resource::<crate::world::zone::ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;

    assert!(
        zone_after > zone_before,
        "期望 miss despawn 后 spawn zone.spirit_qi 上升（真元归还 zone），\
         实际 before={zone_before:.6} after={zone_after:.6}"
    );
}

#[test]
fn hit_target_despawn_does_not_release_to_zone() {
    // 期望：HitTarget despawn residual_qi=0.0（由 carrier.rs:924 保证）→
    // miss-release 系统门控 ε 后不触发 zone 更新。
    let mut app = miss_release_app();
    let projectile = spawn_entity(&mut app);

    let zone_before = app
        .world()
        .resource::<crate::world::zone::ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;

    app.world_mut().send_event(make_despawn_event(
        projectile,
        None,
        0.0, // HitTarget 已置 residual_qi=0.0
        ProjectileDespawnReason::HitTarget,
    ));
    app.update();

    let zone_after = app
        .world()
        .resource::<crate::world::zone::ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;

    assert_eq!(
        zone_before, zone_after,
        "期望 HitTarget despawn 不改变 zone.spirit_qi（residual=0，无双重释放），\
         实际 before={zone_before:.6} after={zone_after:.6}"
    );
}

#[test]
fn zero_residual_is_noop() {
    // 期望：residual_qi=0 → 不更新 zone，不 emit QiTransfer。
    use crate::qi_physics::ledger::QiTransfer;

    let mut app = miss_release_app();
    let projectile = spawn_entity(&mut app);

    let zone_before = app
        .world()
        .resource::<crate::world::zone::ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;

    app.world_mut().send_event(make_despawn_event(
        projectile,
        None,
        0.0,
        ProjectileDespawnReason::NaturalDecay,
    ));
    app.update();

    let zone_after = app
        .world()
        .resource::<crate::world::zone::ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;

    assert_eq!(
        zone_before, zone_after,
        "residual=0 时 zone.spirit_qi 应不变（期望 noop），实际改变了"
    );

    let transfers = app.world().resource::<Events<QiTransfer>>();
    assert!(
        transfers.is_empty(),
        "residual=0 时不应 emit QiTransfer，实际 emit 了 {} 条",
        transfers.len()
    );
}

#[test]
fn no_zone_at_position_routes_to_overflow_transfer() {
    // 期望：落点在 spawn zone 范围外（无 zone 覆盖）→
    // 仍 emit QiTransfer（overflow 路径），真元不蒸发。
    use crate::qi_physics::ledger::QiTransfer;

    let mut app = miss_release_app();
    let projectile = spawn_entity(&mut app);

    // 落点 [9999, 66, 9999] 不在任何注册 zone 内
    app.world_mut().send_event(ProjectileDespawnedEvent {
        owner: None,
        projectile,
        reason: ProjectileDespawnReason::OutOfRange,
        distance: 80.0,
        qi_evaporated: 7.0,
        residual_qi: 3.0,
        pos: [9999.0, 66.0, 9999.0],
        tick: 10,
    });
    app.update();

    let transfers = app.world().resource::<Events<QiTransfer>>();
    assert!(
        !transfers.is_empty(),
        "落点无 zone 时仍须 emit overflow QiTransfer（真元不蒸发），实际无 transfer"
    );
}

#[test]
fn conservation_invariant_residual_equals_transfer_total() {
    // 期望：residual_qi = Σ transfer.amount（守恒等式）。
    // zone 有足够容量吸收全部 residual。
    use crate::qi_physics::ledger::QiTransfer;

    let mut app = miss_release_app();
    let projectile = spawn_entity(&mut app);
    let residual: f32 = 5.0;

    app.world_mut().send_event(make_despawn_event(
        projectile,
        None,
        residual,
        ProjectileDespawnReason::HitBlock,
    ));
    app.update();

    let events = app.world().resource::<Events<QiTransfer>>();
    let mut reader = events.get_reader();
    let total: f64 = reader.read(events).map(|t| t.amount).sum();

    assert!(
        (total - f64::from(residual)).abs() < 1e-9,
        "守恒不变式：transfer 总量应等于 residual_qi（期望 {residual}），实际 {total}"
    );
}

// ── 经脉门测试：charge_carrier meridian gate ─────────────────────────────────────

// ── qi 门测试：resolve_anqi_charge_skill 真元不足时提前拒绝 ────────────────────

// ══════════════════════════════════════════════════════════════════════════
// bughunt-20260726 carrier-throw-dir-nan-leak — client `dir_unit` 溢出
// f32::INFINITY 时 normalize() 产生 (NaN,0,0)，旧的
// `dir.length_squared() <= f64::EPSILON` 零向量守卫因 IEEE-754 NaN 比较
// 恒假而被绕过（不是触发），NaN 流入 velocity/spawn_pos 产生永生投射物。
// ══════════════════════════════════════════════════════════════════════════

/// 组装最小 `throw_carrier_intents` App：一个 `CombatClock` + 事件通道 + 系统。
fn throw_app() -> App {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 0 });
    app.init_resource::<GuardLogDedup>();
    app.add_event::<ThrowCarrierIntent>();
    app.add_systems(Update, throw_carrier_intents);
    app
}

/// 生成一个持有已充能暗器（`instance_id=7`，与 `inventory_with_main_hand`
/// 硬编码的 `item(7, ..)` 对齐）+ 对应 `CarrierStore` 印记的投掷者实体。
fn spawn_throw_actor(app: &mut App, imprint_qi: f32) -> (Entity, u64) {
    let instance_id = 7;
    let mut store = CarrierStore::default();
    store.imprints_by_instance.insert(
        instance_id,
        CarrierImprint {
            carrier_kind: CarrierKind::YibianShougu,
            qi_amount: imprint_qi,
            qi_amount_initial: imprint_qi,
            qi_color: ColorKind::Sharp,
            source_realm: Realm::Condense,
            half_life_min: 120.0,
            decay_started_at_tick: 0,
            bond_kind: BondKind::HandheldCarrier,
            injection_kind: None,
        },
    );
    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 66.0, 0.0]),
            inventory_with_main_hand(ANQI_CHARGED_TEMPLATE_ID),
            store,
        ))
        .id();
    (entity, instance_id)
}

fn projectile_count(app: &mut App) -> usize {
    let mut query = app.world_mut().query::<&AnqiProjectileFlight>();
    query.iter(app.world()).count()
}

fn send_throw(app: &mut App, thrower: Entity, dir_unit: [f32; 3]) {
    app.world_mut().send_event(ThrowCarrierIntent {
        thrower,
        slot: CarrierSlot::MainHand,
        dir_unit,
        power: 1.0,
        issued_at_tick: 0,
    });
    app.update();
}

#[test]
fn throw_with_finite_unit_dir_spawns_projectile_happy_path() {
    // happy path 前置：正常有限单位向量必须仍然正常出弹——防止本次新增
    // 的 `!dir.is_finite()` 守卫误伤合法输入。
    let mut app = throw_app();
    let (actor, instance_id) = spawn_throw_actor(&mut app, 30.0);

    send_throw(&mut app, actor, [1.0, 0.0, 0.0]);

    assert_eq!(
        projectile_count(&mut app),
        1,
        "合法有限方向向量应正常产生 1 个投射物，实测 {} 个 —— 说明本次新增的\
         is_finite() 守卫把合法输入也误判成了非法输入",
        projectile_count(&mut app)
    );
    assert!(
        !app.world()
            .get::<CarrierStore>(actor)
            .unwrap()
            .imprints_by_instance
            .contains_key(&instance_id),
        "正常投掷应消费掉印记（转移进投射物），此处只是确认 happy path 走的是\
         真正的出弹分支而非某个提前 continue"
    );
}

#[test]
fn throw_with_f32_infinity_axis_rejects_and_leaves_no_projectile() {
    // bughunt-20260726 核心回归：dir_unit 溢出 f32::INFINITY（对应恶意/被
    // 修改客户端发送 JSON `1e40` 这种合法 token，经 serde_json 反序列化
    // `[f32;3]` 时 `as f32` 饱和成的产物）不应产生任何投射物实体。旧代码
    // 里 normalize() 会把它变成 (NaN,0,0)，绕过零向量守卫，制造一个
    // 永不消亡、每 tick 写 NaN 位置的幽灵实体（projectile_tick_system 的
    // `traveled > max_distance` 和 `qi_payload <= EPSILON` 两个退出判定
    // 都因 NaN 比较恒假而失效）。
    let mut app = throw_app();
    let (actor, _instance_id) = spawn_throw_actor(&mut app, 30.0);

    send_throw(&mut app, actor, [f32::INFINITY, 0.0, 0.0]);

    assert_eq!(
        projectile_count(&mut app),
        0,
        "dir_unit=[INFINITY,0,0] normalize 后是 (NaN,0,0)，必须被 is_finite() \
         守卫拒绝、不产生任何投射物实体；实测产生了 {} 个 —— 说明 NaN 泄漏\
         守卫失效，永生 NaN 实体 bug 复发",
        projectile_count(&mut app)
    );
}

#[test]
fn throw_with_negative_infinity_axis_also_rejected() {
    // 边界：负向溢出（对应 JSON `-1e40`）同样必须被拒绝，不止正向溢出。
    let mut app = throw_app();
    let (actor, _instance_id) = spawn_throw_actor(&mut app, 30.0);

    send_throw(&mut app, actor, [0.0, f32::NEG_INFINITY, 0.0]);

    assert_eq!(
        projectile_count(&mut app),
        0,
        "dir_unit=[0,-INFINITY,0] 同样必须被拒绝，实测产生了 {} 个投射物",
        projectile_count(&mut app)
    );
}

#[test]
fn throw_with_nan_axis_directly_rejected() {
    // 边界：即便 dir_unit 里直接含 NaN（标准 JSON 语法不允许裸 NaN token，
    // 因此这条路径理论上不会被本 bug 的具体触发方式命中，但守卫本身应该
    // 对任何非有限输入生效，不能依赖"NaN 只能来自 normalize 溢出"这个假设）。
    let mut app = throw_app();
    let (actor, _instance_id) = spawn_throw_actor(&mut app, 30.0);

    send_throw(&mut app, actor, [f32::NAN, 0.0, 0.0]);

    assert_eq!(
        projectile_count(&mut app),
        0,
        "dir_unit 直接含 NaN 分量也必须被拒绝，实测产生了 {} 个投射物",
        projectile_count(&mut app)
    );
}

#[test]
fn throw_with_zero_vector_still_rejected_pre_existing_behavior_unchanged() {
    // 回归：合法的零向量守卫（本次改动之前就存在）必须继续生效——新增的
    // `!dir.is_finite() ||` 前缀不能改变这条既有 `<= EPSILON` 分支的行为
    // （0.0 是有限数，不会被新增检查拦下，仍然落到旧的零向量分支）。
    let mut app = throw_app();
    let (actor, _instance_id) = spawn_throw_actor(&mut app, 30.0);

    send_throw(&mut app, actor, [0.0, 0.0, 0.0]);

    assert_eq!(
        projectile_count(&mut app),
        0,
        "零向量必须继续被既有守卫拒绝，实测产生了 {} 个投射物 —— 说明本次\
         改动意外改变了既有零向量分支的行为",
        projectile_count(&mut app)
    );
}

#[test]
fn normalized_dir_of_overflowed_axis_is_non_finite_root_cause_pin() {
    // 锁住根因前提：normalize() 对 (inf,0,0) 算出 (NaN,0,0)，且这个 NaN
    // 分量的 length_squared() 与 EPSILON 比较恒为 false —— 这正是旧守卫
    // "被绕过而非触发" 的原因。这条测试独立于 throw_carrier_intents，
    // 直接钉死 normalized_dir 本身的行为，防止未来重构（比如换 glam 版本）
    // 悄悄改变这个前提，导致上面几条回归测试失去意义却仍然"碰巧"通过。
    let dir = normalized_dir([f32::INFINITY, 0.0, 0.0]);
    assert!(
        !dir.is_finite(),
        "根因前置条件：normalized_dir([INFINITY,0,0]) 必须产生非有限结果\
         （NaN 分量），实测 {dir:?} 是有限的 —— 说明 glam 或本函数的行为\
         变了，上面的 throw_carrier_intents 回归测试的前提假设需要重新评估"
    );
    // 有意保留 `<=` 后取反、而非改写成 `>`：本测试要钉死的正是 IEEE-754
    // NaN 比较的"全假"语义（`NaN <= x` 和 `NaN > x` 同样为 false，二者不
    // 是互补关系），先绑定到具名变量避免 clippy::neg_cmp_op_on_partial_ord
    // 误判为"可简化成 `>`"——那样会悄悄改变本测试验证的语义。
    let guard_would_bypass = dir.length_squared() <= f64::EPSILON;
    assert!(
        !guard_would_bypass,
        "根因前置条件：NaN 分量的 length_squared() 与 EPSILON 比较必须恒为\
         false（IEEE-754 NaN 比较语义），这正是旧守卫被绕过而非触发的原因；\
         实测比较结果为 true，说明前提假设不成立，本次 bug 的因果链描述有误"
    );
}

// ══════════════════════════════════════════════════════════════════════════
// bughunt-20260726 carrier-throw-dir-nan-leak — projectile_tick_system 防御性
// 兜底：即便未来某个绕开 throw_carrier_intents 的生产者重新引入非有限
// velocity/position，也不能制造出永生 NaN 实体。
// ══════════════════════════════════════════════════════════════════════════

fn spawn_defensive_projectile(app: &mut App, spawn_pos: DVec3, velocity: DVec3) -> Entity {
    app.world_mut()
        .spawn((
            Position::new(spawn_pos),
            QiProjectile {
                owner: None,
                qi_payload: 20.0,
            },
            AnqiProjectileFlight {
                carrier_kind: CarrierKind::BoneChip,
                qi_color: ColorKind::Sharp,
                carrier_grade: CarrierKind::BoneChip.grade(),
                spawn_pos,
                prev_pos: spawn_pos,
                velocity,
                max_distance: ANQI_PROJECTILE_MAX_DISTANCE,
                hitbox_inflation: ANQI_HITBOX_INFLATION,
            },
        ))
        .id()
}

fn defensive_tick_app() -> App {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 5 });
    app.add_event::<CombatEvent>();
    app.add_event::<CarrierImpactEvent>();
    app.add_event::<ProjectileDespawnedEvent>();
    app.add_systems(Update, projectile_tick_system);
    app
}

fn drain_projectile_despawns(app: &mut App) -> Vec<ProjectileDespawnedEvent> {
    app.world()
        .resource::<Events<ProjectileDespawnedEvent>>()
        .iter_current_update_events()
        .cloned()
        .collect()
}

#[test]
fn projectile_tick_system_despawns_non_finite_velocity_without_writing_nan_position() {
    // 核心防御性回归：非有限 velocity（模拟"未来某个生产者重新引入 NaN"
    // 的场景，不经过 throw_carrier_intents）必须在本 tick 内立即被
    // despawn，绝不允许 Position 被写入 NaN、绝不允许 NaN 流入
    // qi_physics ledger。
    let mut app = defensive_tick_app();
    let spawn_pos = DVec3::new(1.0, 65.0, 2.0);
    let projectile =
        spawn_defensive_projectile(&mut app, spawn_pos, DVec3::new(f64::INFINITY, 0.0, 0.0));

    app.update();

    assert!(
        app.world().get_entity(projectile).is_none(),
        "非有限 velocity 必须在本 tick 内被防御性 despawn，实测实体仍存活 —— \
         说明防御性兜底没生效，永生 NaN 实体 bug 会以另一条生产路径重演"
    );

    let despawns = drain_projectile_despawns(&mut app);
    assert_eq!(
        despawns.len(),
        1,
        "应恰好 despawn 一次，实测 {} 次",
        despawns.len()
    );
    assert_eq!(
        despawns[0].reason,
        ProjectileDespawnReason::OutOfRange,
        "非有限 velocity 应走新增的防御性 OutOfRange 分支，实测 {:?}",
        despawns[0].reason
    );
    assert!(
        despawns[0].pos.iter().all(|c| c.is_finite()),
        "despawn 事件的 pos 必须全部有限——绝不能把 NaN/Infinity 序列化进 \
         ProjectileDespawnedEvent 传给下游 Redis 桥接消费者，实测 {:?}",
        despawns[0].pos
    );
    assert_eq!(
        despawns[0].pos,
        [spawn_pos.x, spawn_pos.y, spawn_pos.z],
        "本 tick 前唯一已知的有限位置就是出生点，despawn 的 pos 应回退到它\
         （而不是使用非有限的 next），实测 {:?}",
        despawns[0].pos
    );
    assert!(
        despawns[0].qi_evaporated.is_finite() && despawns[0].residual_qi.is_finite(),
        "qi_evaporated/residual_qi 必须有限——绝不能把 NaN 灌进 qi_physics \
         ledger 破坏全服灵气守恒律，实测 evaporated={} residual={}",
        despawns[0].qi_evaporated,
        despawns[0].residual_qi
    );
}

#[test]
fn projectile_tick_system_despawns_single_nan_velocity_axis() {
    // 边界：精确复现 bug 报告中的 dir 形状——只有一个轴是 NaN，其余轴是
    // 合法有限值 0.0（对应 normalize((inf,0,0)) 产生的 (NaN,0.0,0.0)）。
    // `DVec3::is_finite()` 要求全部分量有限，一个轴坏了整体就该判非有限。
    let mut app = defensive_tick_app();
    let spawn_pos = DVec3::new(0.0, 65.0, 0.0);
    let projectile =
        spawn_defensive_projectile(&mut app, spawn_pos, DVec3::new(f64::NAN, 0.0, 0.0));

    app.update();

    assert!(
        app.world().get_entity(projectile).is_none(),
        "单轴 NaN velocity（精确复现 bug 报告里的 (NaN,0,0) 形状）也必须被\
         防御性 despawn，实测实体仍存活"
    );
    let despawns = drain_projectile_despawns(&mut app);
    assert_eq!(
        despawns.first().map(|d| d.reason),
        Some(ProjectileDespawnReason::OutOfRange),
        "实测 despawns={despawns:?}"
    );
}

#[test]
fn projectile_tick_system_out_of_range_still_works_for_finite_overshoot() {
    // 回归：新增的 `!next.is_finite() || !traveled.is_finite()` 检查不能
    // 误伤合法的、纯粹因为飞太远而需要 OutOfRange despawn 的有限速度弹道
    // ——防止本次防御性改动把正常的射程判定短路掉。
    let mut app = defensive_tick_app();
    let spawn_pos = DVec3::new(0.0, 65.0, 0.0);
    // 速度足够大，一 tick 内位移就超过 ANQI_PROJECTILE_MAX_DISTANCE。
    let velocity = DVec3::new(
        f64::from(ANQI_PROJECTILE_MAX_DISTANCE) * 2.0 * TICKS_PER_SECOND as f64,
        0.0,
        0.0,
    );
    let projectile = spawn_defensive_projectile(&mut app, spawn_pos, velocity);

    app.update();

    assert!(
        app.world().get_entity(projectile).is_none(),
        "有限速度飞出射程应被既有 OutOfRange 分支 despawn，实测实体仍存活"
    );
    let despawns = drain_projectile_despawns(&mut app);
    assert_eq!(
        despawns.first().map(|d| d.reason),
        Some(ProjectileDespawnReason::OutOfRange),
        "有限速度飞出射程应走 OutOfRange，实测 {despawns:?} —— 说明本次\
         新增的 is_finite() 防御检查误伤了合法的有限速度弹道"
    );
    assert!(
        despawns[0].pos.iter().all(|c| c.is_finite()),
        "有限速度场景下 despawn pos 理应有限，实测 {:?}",
        despawns[0].pos
    );
}
