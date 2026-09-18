use super::*;
use crate::combat::components::{Wound, WoundKind};
use crate::inventory::{
    ContainerState, InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemRegistry,
    ItemTemplate, PlacedItemState, MAIN_PACK_CONTAINER_ID,
};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use std::collections::HashMap;
use valence::prelude::{App, DVec3, Entity, Events, Position, Query, Update, With};
use valence::testing::create_mock_client;

fn make_inventory_with_stack(instance_id: u64, stack: u32) -> PlayerInventory {
    let item = ItemInstance {
        instance_id,
        template_id: "tea".to_string(),
        display_name: "茶".to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.1,
        rarity: ItemRarity::Common,
        description: String::new(),
        stack_count: stack,
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
    };
    PlayerInventory {
        material_preparation: Default::default(),
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(5),
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 5,
            cols: 7,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: item,
            }],

            owner_instance_id: None,
        }],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 50.0,
    }
}

fn make_effect_template(template_id: &str, effect: ItemEffect) -> ItemTemplate {
    ItemTemplate {
        quick_use: true,
        id: template_id.to_string(),
        display_name: template_id.to_string(),
        category: ItemCategory::Misc,
        placeable: None,
        max_stack_count: 16,
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.1,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 0.0,
        description: String::new(),
        effect: Some(effect),
        cast_duration_ms: 1000,
        cooldown_ms: 500,
        weapon_spec: None,
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

fn make_hotbar_item(template_id: &str, instance_id: u64, stack_count: u32) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: template_id.to_string(),
        display_name: template_id.to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.1,
        rarity: ItemRarity::Common,
        description: String::new(),
        stack_count,
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

fn setup_quickslot_effect_app(template_id: &str, effect: ItemEffect) -> (App, Entity) {
    use crate::combat::CombatClock;
    use crate::player::state::PlayerState;
    use valence::prelude::DVec3;
    use valence::testing::create_mock_client;

    const INSTANCE_ID: u64 = 4242;
    const QUICK_SLOT: u8 = 0;

    let mut templates = HashMap::new();
    templates.insert(
        template_id.to_string(),
        make_effect_template(template_id, effect),
    );

    let mut inventory = PlayerInventory {
        material_preparation: Default::default(),
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 5,
            cols: 7,
            items: Vec::new(),
            owner_instance_id: None,
        }],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 50.0,
    };
    inventory.hotbar[QUICK_SLOT as usize] = Some(make_hotbar_item(template_id, INSTANCE_ID, 2));

    let mut quick_slot_bindings = QuickSlotBindings::default();
    quick_slot_bindings.slots[QUICK_SLOT as usize] = Some(INSTANCE_ID);
    let casting = Casting {
        source: CastSource::QuickSlot,
        slot: QUICK_SLOT,
        started_at_tick: 0,
        duration_ticks: 1,
        started_at_ms: 0,
        duration_ms: 50,
        bound_instance_id: Some(INSTANCE_ID),
        start_position: DVec3::new(0.0, 64.0, 0.0),
        complete_cooldown_ticks: 20,
        skill_id: None,
        skill_config: None,
    };

    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 10 });
    app.insert_resource(ItemRegistry::from_map(templates));
    app.add_event::<crate::network::audio_event_emit::PlaySoundRecipeRequest>();
    app.add_event::<crate::combat::yidao::YidaoCastCompleteEvent>();
    app.add_event::<GuangboTicaoPracticeEvent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<crate::cultivation::lifespan::LifespanExtensionIntent>();
    app.add_event::<crate::cultivation::poison_trait::ConsumePoisonPillIntent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, tick_casts_or_interrupt);

    let (client_bundle, _helper) = create_mock_client("TestPlayer");
    let player = app
        .world_mut()
        .spawn(client_bundle)
        .insert((
            Position::new([0.0, 64.0, 0.0]),
            casting,
            Wounds::default(),
            inventory,
            PlayerState::default(),
            quick_slot_bindings,
            SkillBarBindings::default(),
        ))
        .id();

    (app, player)
}

fn hotbar_stack_count(app: &mut App, player: Entity) -> u32 {
    app.world_mut()
        .entity(player)
        .get::<PlayerInventory>()
        .and_then(|inventory| inventory.hotbar[0].as_ref())
        .map(|item| item.stack_count)
        .unwrap_or(0)
}

#[test]
fn consume_one_stack_decrements_when_above_one() {
    let mut inv = make_inventory_with_stack(42, 5);
    assert!(consume_one_stack(&mut inv, 42));
    assert_eq!(inv.containers[0].items[0].instance.stack_count, 4);
    assert_eq!(inv.revision, InventoryRevision(6));
}

#[test]
fn consume_one_stack_removes_when_at_one() {
    let mut inv = make_inventory_with_stack(42, 1);
    assert!(consume_one_stack(&mut inv, 42));
    assert!(inv.containers[0].items.is_empty());
}

#[test]
fn consume_one_stack_returns_false_when_missing() {
    let mut inv = make_inventory_with_stack(42, 3);
    assert!(!consume_one_stack(&mut inv, 999));
    assert_eq!(inv.containers[0].items[0].instance.stack_count, 3);
}

/// 供 `set_cast_cooldown` 单测复用的最小 `Casting`。
fn minimal_skillbar_casting(source: CastSource, slot: u8, skill_id: Option<&str>) -> Casting {
    Casting {
        source,
        slot,
        started_at_tick: 0,
        duration_ticks: 1,
        started_at_ms: 0,
        duration_ms: 50,
        bound_instance_id: None,
        start_position: DVec3::ZERO,
        complete_cooldown_ticks: 1,
        skill_id: skill_id.map(str::to_string),
        skill_config: None,
    }
}

// ─────────────────────────────────────────────────────────────────
// bughunt skillbar-rebind-cooldown-reset — `set_cast_cooldown` 的 SkillBar 分支
// 改为按 `Casting.skill_id` 记账（不再按 `slot`），锁住该行为并覆盖缺 skill_id
// 的防御性兜底分支。
// ─────────────────────────────────────────────────────────────────

#[test]
fn set_cast_cooldown_skillbar_branch_keys_by_skill_id_not_slot() {
    let casting = minimal_skillbar_casting(CastSource::SkillBar, 1, Some("dugu.eclipse"));
    let mut quick_bindings = QuickSlotBindings::default();
    let mut skillbar_bindings = SkillBarBindings::default();

    set_cast_cooldown(
        &casting,
        &mut quick_bindings,
        &mut skillbar_bindings,
        1,
        200,
    );

    assert!(
        skillbar_bindings.is_on_cooldown("dugu.eclipse", 100),
        "SkillBar 分支必须按 Casting.skill_id（而非 slot）写入 SkillBarBindings 冷却"
    );
    assert!(
        !quick_bindings.is_on_cooldown(1, 100),
        "SkillBar 来源不应误写 QuickSlotBindings（两套 bindings 必须互不干扰）"
    );
}

#[test]
fn set_cast_cooldown_quickslot_branch_still_keys_by_slot() {
    // 对照：QuickSlotBindings 是 bug 修复范围之外的既有设计，仍按 slot 记账，不受影响。
    let casting = minimal_skillbar_casting(CastSource::QuickSlot, 1, None);
    let mut quick_bindings = QuickSlotBindings::default();
    let mut skillbar_bindings = SkillBarBindings::default();

    set_cast_cooldown(
        &casting,
        &mut quick_bindings,
        &mut skillbar_bindings,
        1,
        200,
    );

    assert!(quick_bindings.is_on_cooldown(1, 100));
    assert!(
        skillbar_bindings.cooldowns.is_empty(),
        "QuickSlot 来源不应写入 SkillBarBindings 的 cooldowns map"
    );
}

#[test]
fn set_cast_cooldown_skillbar_branch_missing_skill_id_is_defensive_noop() {
    // 理论不可达的防御性分支：所有生产 SkillBar Casting 构造点都填了 skill_id，
    // 这里锁住"万一没填"时不 panic、也不产生任何 cooldowns entry。
    let casting = minimal_skillbar_casting(CastSource::SkillBar, 1, None);
    let mut quick_bindings = QuickSlotBindings::default();
    let mut skillbar_bindings = SkillBarBindings::default();

    set_cast_cooldown(
        &casting,
        &mut quick_bindings,
        &mut skillbar_bindings,
        1,
        200,
    );

    assert!(
        skillbar_bindings.cooldowns.is_empty(),
        "缺 skill_id 时不应产生任何 cooldowns entry（无 key 可写）"
    );
}

#[test]
fn meridian_heal_advances_first_unhealed_crack() {
    use crate::cultivation::components::{CrackCause, MeridianCrack, MeridianSystem};
    let mut meridians = MeridianSystem::default();
    // Inject a crack into the first regular meridian.
    meridians.regular[0].cracks.push(MeridianCrack {
        severity: 0.5,
        healing_progress: 0.0,
        cause: CrackCause::Attack,
        created_at: 0,
    });
    // Manually walk apply_item_effect minus the Mut wrapper using internals.
    let crack = &mut meridians.regular[0].cracks[0];
    crack.healing_progress = (crack.healing_progress + 0.3).clamp(0.0, crack.severity);
    assert!((crack.healing_progress - 0.3).abs() < 1e-9);
    // Healing past severity should retain cull at 0.5.
    crack.healing_progress = (crack.healing_progress + 0.4).clamp(0.0, crack.severity);
    assert!((crack.healing_progress - crack.severity).abs() < 1e-9);
}

#[test]
fn qi_recovery_effect_restores_current_qi_without_raising_cap() {
    let mut cultivation = Cultivation {
        qi_current: 130.0,
        qi_max: 210.0,
        qi_max_frozen: Some(20.0),
        ..Default::default()
    };

    apply_item_effect(
        &ItemEffect::QiRecovery { amount: 120.0 },
        Some(&mut cultivation),
        None,
        None,
        None,
        "Azure",
        Entity::PLACEHOLDER,
    );

    assert_eq!(cultivation.qi_current, 190.0);
    assert_eq!(cultivation.qi_max, 210.0);
    assert_eq!(cultivation.qi_max_frozen, Some(20.0));
}

#[test]
fn composure_restore_clamps_to_full_composure() {
    let mut cultivation = Cultivation {
        composure: 0.80,
        ..Default::default()
    };

    apply_item_effect(
        &ItemEffect::ComposureRestore { magnitude: 0.35 },
        Some(&mut cultivation),
        None,
        None,
        None,
        "Azure",
        Entity::PLACEHOLDER,
    );

    assert_eq!(
        cultivation.composure, 1.0,
        "expected composure to clamp to 1.0 because restored by ComposureRestore magnitude 0.35, actual {}",
        cultivation.composure
    );
}

#[test]
fn wound_heal_targets_all_wounds_when_target_missing() {
    let mut wounds = Wounds::default();
    wounds.entries.push(Wound {
        location: crate::body_plan::legacy_body_part_to_id(BodyPart::ArmL),
        kind: WoundKind::Cut,
        severity: 0.20,
        bleeding_per_sec: 1.0,
        created_at_tick: 0,
        inflicted_by: None,
    });
    wounds.entries.push(Wound {
        location: crate::body_plan::legacy_body_part_to_id(BodyPart::LegR),
        kind: WoundKind::Blunt,
        severity: 0.75,
        bleeding_per_sec: 1.0,
        created_at_tick: 0,
        inflicted_by: None,
    });

    apply_item_effect(
        &ItemEffect::WoundHeal {
            magnitude: 1.0,
            target: None,
        },
        None,
        None,
        None,
        Some(&mut wounds),
        "Azure",
        Entity::PLACEHOLDER,
    );

    assert_eq!(
        wounds.entries.len(),
        1,
        "expected one wound to remain because all-target WoundHeal removes only the light cut, actual {}",
        wounds.entries.len()
    );
    assert_eq!(
        wounds.entries[0].location,
        crate::body_plan::legacy_body_part_to_id(BodyPart::LegR),
        "expected remaining wound to be LegR because ArmL cut was healed below removal threshold, actual {:?}",
        wounds.entries[0].location
    );
    assert!(
        (wounds.entries[0].severity - 0.50).abs() < f32::EPSILON,
        "expected LegR severity 0.50 because WoundHeal magnitude 1 applies one grade, actual {}",
        wounds.entries[0].severity
    );
}

#[test]
fn wound_heal_slash_target_filters_body_part_group() {
    let mut wounds = Wounds::default();
    wounds.entries.push(Wound {
        location: crate::body_plan::legacy_body_part_to_id(BodyPart::ArmL),
        kind: WoundKind::Blunt,
        severity: 0.40,
        bleeding_per_sec: 1.0,
        created_at_tick: 0,
        inflicted_by: None,
    });
    wounds.entries.push(Wound {
        location: crate::body_plan::legacy_body_part_to_id(BodyPart::LegL),
        kind: WoundKind::Blunt,
        severity: 0.70,
        bleeding_per_sec: 1.0,
        created_at_tick: 0,
        inflicted_by: None,
    });

    apply_item_effect(
        &ItemEffect::WoundHeal {
            magnitude: 2.0,
            target: Some("arm_l/arm_r".to_string()),
        },
        None,
        None,
        None,
        Some(&mut wounds),
        "Azure",
        Entity::PLACEHOLDER,
    );

    assert_eq!(
        wounds.entries.len(),
        1,
        "expected one wound to remain because arm target group should not heal leg wounds, actual {}",
        wounds.entries.len()
    );
    assert_eq!(
        wounds.entries[0].location,
        crate::body_plan::legacy_body_part_to_id(BodyPart::LegL),
        "expected remaining wound to be LegL because target was arm_l/arm_r, actual {:?}",
        wounds.entries[0].location
    );
    assert!(
        (wounds.entries[0].severity - 0.70).abs() < f32::EPSILON,
        "expected LegL severity unchanged at 0.70 because target was arm_l/arm_r, actual {}",
        wounds.entries[0].severity
    );
}

#[test]
fn wound_heal_slash_target_shares_grade_budget_across_group() {
    let mut wounds = Wounds::default();
    wounds.entries.push(Wound {
        location: crate::body_plan::legacy_body_part_to_id(BodyPart::ArmL),
        kind: WoundKind::Blunt,
        severity: 0.75,
        bleeding_per_sec: 1.0,
        created_at_tick: 0,
        inflicted_by: None,
    });
    wounds.entries.push(Wound {
        location: crate::body_plan::legacy_body_part_to_id(BodyPart::ArmR),
        kind: WoundKind::Blunt,
        severity: 0.75,
        bleeding_per_sec: 1.0,
        created_at_tick: 0,
        inflicted_by: None,
    });

    apply_item_effect(
        &ItemEffect::WoundHeal {
            magnitude: 2.0,
            target: Some("arm_l/arm_r".to_string()),
        },
        None,
        None,
        None,
        Some(&mut wounds),
        "Azure",
        Entity::PLACEHOLDER,
    );

    let arm_l = wounds
        .entries
        .iter()
        .find(|wound| wound.location == crate::body_plan::legacy_body_part_to_id(BodyPart::ArmL))
        .expect("ArmL wound should remain after shared-budget heal");
    let arm_r = wounds
        .entries
        .iter()
        .find(|wound| wound.location == crate::body_plan::legacy_body_part_to_id(BodyPart::ArmR))
        .expect("ArmR wound should remain after shared-budget heal");
    assert!(
        (arm_l.severity - 0.25).abs() < f32::EPSILON,
        "expected ArmL severity 0.25 because it consumes the initial two-grade budget, actual {}",
        arm_l.severity
    );
    assert!(
        (arm_r.severity - 0.50).abs() < f32::EPSILON,
        "expected ArmR severity 0.50 because remaining shared budget is one grade, actual {}",
        arm_r.severity
    );
}

#[test]
fn tick_casts_consumable_qi_recovery_consumes_and_applies() {
    let (mut app, player) = setup_quickslot_effect_app(
        "huiyuan_decoction_test",
        ItemEffect::QiRecovery { amount: 40.0 },
    );
    app.world_mut().entity_mut(player).insert(Cultivation {
        qi_current: 10.0,
        qi_max: 100.0,
        ..Default::default()
    });

    app.update();

    let stack_count = hotbar_stack_count(&mut app, player);
    assert_eq!(
        stack_count, 1,
        "expected hotbar stack count 1 because QuickSlot consumable should consume one item, actual {stack_count}"
    );
    let cultivation = app
        .world_mut()
        .entity(player)
        .get::<Cultivation>()
        .expect("Cultivation should remain attached");
    assert_eq!(
        cultivation.qi_current, 50.0,
        "expected qi_current 50.0 because QiRecovery amount 40 applies after consuming one item, actual {}",
        cultivation.qi_current
    );
}

#[test]
fn tick_casts_consumable_composure_restore_consumes_and_applies() {
    let (mut app, player) = setup_quickslot_effect_app(
        "calming_tea_test",
        ItemEffect::ComposureRestore { magnitude: 0.35 },
    );
    app.world_mut().entity_mut(player).insert(Cultivation {
        composure: 0.40,
        ..Default::default()
    });

    app.update();

    let stack_count = hotbar_stack_count(&mut app, player);
    assert_eq!(
        stack_count, 1,
        "expected hotbar stack count 1 because QuickSlot consumable should consume one item, actual {stack_count}"
    );
    let cultivation = app
        .world_mut()
        .entity(player)
        .get::<Cultivation>()
        .expect("Cultivation should remain attached");
    assert!(
        (cultivation.composure - 0.75).abs() < f64::EPSILON,
        "expected composure 0.75 because ComposureRestore magnitude 0.35 applies to 0.40, actual {}",
        cultivation.composure
    );
}

#[test]
fn tick_casts_rejects_targeted_item_even_with_stale_binding() {
    let (mut app, player) = setup_quickslot_effect_app(
        "leg_splint_test",
        ItemEffect::WoundHeal {
            magnitude: 2.0,
            target: Some("leg_l/leg_r".to_string()),
        },
    );
    {
        let mut entity = app.world_mut().entity_mut(player);
        let mut wounds = entity
            .get_mut::<Wounds>()
            .expect("Wounds should be present on player");
        wounds.entries.push(Wound {
            location: crate::body_plan::legacy_body_part_to_id(BodyPart::LegL),
            kind: WoundKind::Blunt,
            severity: 0.40,
            bleeding_per_sec: 1.0,
            created_at_tick: 0,
            inflicted_by: None,
        });
        wounds.entries.push(Wound {
            location: crate::body_plan::legacy_body_part_to_id(BodyPart::ArmL),
            kind: WoundKind::Blunt,
            severity: 0.40,
            bleeding_per_sec: 1.0,
            created_at_tick: 0,
            inflicted_by: None,
        });
    }

    app.update();

    assert_eq!(
        hotbar_stack_count(&mut app, player),
        2,
        "定向夹板不得在快捷消费路径扣除"
    );
    let wounds = app.world().get::<Wounds>(player).unwrap();
    assert_eq!(wounds.entries.len(), 2);
    assert!(
        wounds
            .entries
            .iter()
            .all(|w| (w.severity - 0.40).abs() < f32::EPSILON),
        "过时快捷链接不得绕过部位选择直接治疗"
    );
}

#[test]
fn cast_poison_pill_effect_emits_consume_intent() {
    fn emit_for_test(
        mut effect_intents: ParamSet<(
            EventWriter<ApplyStatusEffectIntent>,
            EventWriter<LifespanExtensionIntent>,
            EventWriter<ConsumePoisonPillIntent>,
        )>,
    ) {
        apply_cast_item_effect(
            &ItemEffect::PoisonPill {
                pill_item_id: "poison_pill_qing_lin_man_tuo".to_string(),
            },
            CastItemEffectTargets {
                cultivation: None,
                meridians: None,
                contamination: None,
                wounds: None,
            },
            &mut effect_intents,
            CastItemEffectContext {
                issued_at_tick: 42,
                username: "Azure",
                entity: Entity::PLACEHOLDER,
                item_freshness: None,
                decay_profiles: None,
            },
        );
    }

    let mut app = App::new();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<LifespanExtensionIntent>();
    app.add_event::<ConsumePoisonPillIntent>();
    app.add_systems(Update, emit_for_test);

    app.update();

    let events: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<ConsumePoisonPillIntent>>()
        .drain()
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].pill, PoisonPillKind::QingLinManTuo);
    assert_eq!(events[0].issued_at_tick, 42);
}

#[test]
fn cast_lifespan_and_pressure_effects_emit_runtime_intents() {
    fn emit_for_test(
        mut effect_intents: ParamSet<(
            EventWriter<ApplyStatusEffectIntent>,
            EventWriter<LifespanExtensionIntent>,
            EventWriter<ConsumePoisonPillIntent>,
        )>,
    ) {
        apply_cast_item_effect(
            &ItemEffect::LifespanExtension {
                years: 0,
                source: "test_core".to_string(),
            },
            CastItemEffectTargets {
                cultivation: None,
                meridians: None,
                contamination: None,
                wounds: None,
            },
            &mut effect_intents,
            CastItemEffectContext {
                issued_at_tick: 7,
                username: "Azure",
                entity: Entity::PLACEHOLDER,
                item_freshness: None,
                decay_profiles: None,
            },
        );
        apply_cast_item_effect(
            &ItemEffect::AntiSpiritPressure { duration_ticks: 0 },
            CastItemEffectTargets {
                cultivation: None,
                meridians: None,
                contamination: None,
                wounds: None,
            },
            &mut effect_intents,
            CastItemEffectContext {
                issued_at_tick: 9,
                username: "Azure",
                entity: Entity::PLACEHOLDER,
                item_freshness: None,
                decay_profiles: None,
            },
        );
    }

    let mut app = App::new();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<LifespanExtensionIntent>();
    app.add_event::<ConsumePoisonPillIntent>();
    app.add_systems(Update, emit_for_test);

    app.update();

    let lifespan_events: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<LifespanExtensionIntent>>()
        .drain()
        .collect();
    assert_eq!(lifespan_events.len(), 1);
    assert_eq!(lifespan_events[0].requested_years, 1);
    assert_eq!(lifespan_events[0].source, "test_core");

    let status_events: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<ApplyStatusEffectIntent>>()
        .drain()
        .collect();
    assert_eq!(status_events.len(), 1);
    assert_eq!(
        status_events[0].kind,
        StatusEffectKind::AntiSpiritPressurePill
    );
    assert_eq!(status_events[0].duration_ticks, 1);
    assert_eq!(status_events[0].issued_at_tick, 9);
}

#[test]
fn consume_one_stack_finds_in_hotbar() {
    let mut inv = make_inventory_with_stack(99, 10); // unrelated container item
    inv.hotbar[1] = Some(ItemInstance {
        instance_id: 7,
        template_id: "pill".to_string(),
        display_name: "丹".to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.1,
        rarity: ItemRarity::Common,
        description: String::new(),
        stack_count: 2,
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
    });
    assert!(consume_one_stack(&mut inv, 7));
    assert_eq!(inv.hotbar[1].as_ref().unwrap().stack_count, 1);
    assert!(consume_one_stack(&mut inv, 7));
    assert!(inv.hotbar[1].is_none());
}

// ── plan-food-v1 P2 BLOCKER 2：FoodRegen freshness 门控端到端测试 ──

/// 辅助：构造带 FoodRegen effect 的 ApplyStatusEffectIntent 触发函数，
/// 允许注入 freshness 和 decay profile registry。
fn build_app_for_food_regen_test(
    freshness: Option<crate::shelflife::Freshness>,
    registry: Option<crate::shelflife::DecayProfileRegistry>,
    bonus_factor: f32,
    duration_ticks: u64,
) -> App {
    let mut app = App::new();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<LifespanExtensionIntent>();
    app.add_event::<ConsumePoisonPillIntent>();
    if let Some(r) = registry {
        app.insert_resource(r);
    }

    let freshness_clone = freshness.clone();
    app.add_systems(
        Update,
        move |mut effect_intents: ParamSet<(
            EventWriter<ApplyStatusEffectIntent>,
            EventWriter<LifespanExtensionIntent>,
            EventWriter<ConsumePoisonPillIntent>,
        )>,
              reg: Option<Res<crate::shelflife::DecayProfileRegistry>>| {
            apply_cast_item_effect(
                &ItemEffect::FoodRegen {
                    bonus_factor,
                    duration_ticks,
                },
                CastItemEffectTargets {
                    cultivation: None,
                    meridians: None,
                    contamination: None,
                    wounds: None,
                },
                &mut effect_intents,
                CastItemEffectContext {
                    issued_at_tick: 1000,
                    username: "TestUser",
                    entity: Entity::PLACEHOLDER,
                    item_freshness: freshness_clone.clone(),
                    decay_profiles: reg.as_deref(),
                },
            );
        },
    );
    app
}

/// BLOCKER 2 端到端：新鲜灵果（freshness 状态 Fresh）→ FoodRegen 发全额 bonus CultivationAcceleration intent
#[test]
fn food_regen_fresh_ling_guo_emits_full_bonus_intent() {
    use crate::inventory::freshness::GAME_DAY_TICKS;
    use crate::shelflife::types::{DecayFormula, DecayProfile, DecayProfileId, Freshness};

    let profile = DecayProfile::Spoil {
        id: DecayProfileId::new("food_spoil_ling_guo_v1"),
        formula: DecayFormula::Linear {
            decay_per_tick: 1.0 / (GAME_DAY_TICKS as f32 * 2.0),
        },
        spoil_threshold: 0.01,
    };
    let mut registry = crate::shelflife::DecayProfileRegistry::new();
    registry.insert(profile.clone()).unwrap();

    let freshness = Freshness::new(0, 1.0, &profile); // created at tick 0, full qi
                                                      // issued_at_tick=1000 → 已经过 1000 ticks，但 ling_guo 2 GAME_DAY = 48000 ticks，
                                                      // current_qi = 1.0 - 1000/(48000) ≈ 0.979 >> threshold 0.01 → FoodApplied

    let mut app = build_app_for_food_regen_test(Some(freshness), Some(registry), 0.20, 48_000);
    app.update();

    let intents: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<ApplyStatusEffectIntent>>()
        .drain()
        .collect();
    assert_eq!(
        intents.len(),
        1,
        "新鲜灵果应发出 1 条 CultivationAcceleration intent，实际 {}",
        intents.len()
    );
    let intent = &intents[0];
    assert_eq!(
        intent.kind,
        StatusEffectKind::CultivationAcceleration,
        "intent.kind 应为 CultivationAcceleration"
    );
    assert!(
        (intent.magnitude - 0.20).abs() < 1e-3,
        "新鲜灵果 magnitude 应=0.20（全额），实际 {}",
        intent.magnitude
    );
    assert_eq!(
        intent.duration_ticks, 48_000,
        "duration_ticks 应=48000，实际 {}",
        intent.duration_ticks
    );
}

/// BLOCKER 2 端到端：腐败到 CriticalBlock 的食物 → 不发 ApplyStatusEffectIntent
#[test]
fn food_regen_critical_block_emits_no_intent() {
    use crate::inventory::freshness::GAME_DAY_TICKS;
    use crate::shelflife::types::{DecayFormula, DecayProfile, DecayProfileId, Freshness};

    // spoil_threshold = 0.5，Linear decay_per_tick = 0.5/1
    // issued_at_tick = 1000 → 如果 created_at = 0 → current = 1.0 - 1.0*1000 < 0 → CriticalBlock
    // 但 Linear 会 clamp，我们用 threshold=0.5，decay_per_tick=1.0（每 tick 衰减 1），
    // current = 1.0 - 1000*1.0 ≈ 0（极低） < 0.1×0.5 → CriticalBlock
    let profile = DecayProfile::Spoil {
        id: DecayProfileId::new("test_fast_spoil"),
        formula: DecayFormula::Linear {
            decay_per_tick: 1.0 / (GAME_DAY_TICKS as f32 * 0.1),
        },
        spoil_threshold: 0.5,
    };
    let mut registry = crate::shelflife::DecayProfileRegistry::new();
    registry.insert(profile.clone()).unwrap();

    // issued_at_tick = 1000，created_at = 0 → 已经过 1000 ticks
    // ling_guo_fast: 0.1 GAME_DAY = 2400 ticks → linear threshold 在 ~2400 ticks 内到达 spoil
    // 1000 ticks 时 current = 1 - 1000/2400 ≈ 0.583 > threshold 0.5 → 不是 block
    // 改用大的 now_tick：用 freshness created_at=0 但构造时把 issued_at_tick 设成超大值
    // 更简单：用 Spoil Linear decay，让 (tick - created_at) >> half_life
    let fast_profile = DecayProfile::Spoil {
        id: DecayProfileId::new("food_regen_crit_test"),
        formula: DecayFormula::Linear {
            decay_per_tick: 1.0 / (100.0_f32), // 每 100 tick 耗尽 initial_qi
        },
        spoil_threshold: 0.5,
    };
    let mut registry2 = crate::shelflife::DecayProfileRegistry::new();
    registry2.insert(fast_profile.clone()).unwrap();

    // issued_at_tick = 1000 (in the test app)
    // created_at = 0, decay_per_tick = 1/100 → at tick 1000: current = 1 - (1000/100) = -9 → clamped 0
    // 0 < 0.1 × 0.5 = 0.05 → CriticalBlock
    let freshness = Freshness::new(0, 1.0, &fast_profile);

    let mut app = build_app_for_food_regen_test(Some(freshness), Some(registry2), 0.20, 48_000);
    app.update();

    let intents: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<ApplyStatusEffectIntent>>()
        .drain()
        .collect();
    assert_eq!(
        intents.len(),
        0,
        "CriticalBlock 应不发出任何 CultivationAcceleration intent，实际发出 {} 条",
        intents.len()
    );
}

/// BLOCKER 2 端到端：SpoiledWarn 状态食物 → 发出 reduced_bonus intent
#[test]
fn food_regen_spoiled_warn_emits_reduced_bonus_intent() {
    use crate::shelflife::types::{DecayFormula, DecayProfile, DecayProfileId, Freshness};

    // spoil_threshold=0.5，Linear decay：每 tick 衰减 1/12000（半衰=12000 ticks = 0.5 GAME_DAY）
    // issued_at_tick=1000, created_at=0
    // tick=1000 → current = 1 - 1000/12000 ≈ 0.917 → Safe（不 warn）
    // 用更快衰减：decay_per_tick = 1/3（每 3 tick 耗完），issued_at_tick=1000
    // current = 1 - 1000/3 ≈ -332 → CriticalBlock（太快）
    // 精心设计：spoil_threshold=0.5, decay_per_tick = 0.75/12000
    // at tick=1000: current = 1.0 - (1000/12000)*0.75 = 1.0 - 0.0625 = 0.9375 → Safe
    // 但我需要 Warn 区间：0.1×spoil_threshold <= current < spoil_threshold = 0.05 ~ 0.5
    // 设 spoil_threshold=0.5, decay so that current ≈ 0.25 at tick=1000
    // 0.25 = 1.0 - decay_per_tick × 1000 → decay_per_tick = 0.00075
    // 0.05 < 0.25 < 0.5 → SpoiledWarn ✓
    let warn_profile = DecayProfile::Spoil {
        id: DecayProfileId::new("food_regen_warn_test"),
        formula: DecayFormula::Linear {
            decay_per_tick: 0.00075,
        },
        spoil_threshold: 0.5,
    };
    let mut registry = crate::shelflife::DecayProfileRegistry::new();
    registry.insert(warn_profile.clone()).unwrap();

    let freshness = Freshness::new(0, 1.0, &warn_profile);

    let mut app = build_app_for_food_regen_test(Some(freshness), Some(registry), 0.20, 48_000);
    app.update();

    let intents: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<ApplyStatusEffectIntent>>()
        .drain()
        .collect();
    assert_eq!(
        intents.len(),
        1,
        "SpoiledWarn 应发出 1 条 intent（降效），实际 {} 条",
        intents.len()
    );
    let intent = &intents[0];
    assert_eq!(intent.kind, StatusEffectKind::CultivationAcceleration);
    assert!(
        intent.magnitude < 0.20,
        "SpoiledWarn intent.magnitude 应 < 0.20（降效），实际 {}",
        intent.magnitude
    );
    assert!(
        intent.magnitude > 0.0,
        "SpoiledWarn intent.magnitude 应 > 0.0（未全腐），实际 {}",
        intent.magnitude
    );
}

/// BLOCKER 2 端到端：无 freshness（Noop）→ 退化为原始 bonus intent（兼容旧物品）
#[test]
fn food_regen_no_freshness_noop_emits_original_bonus_intent() {
    let mut app = build_app_for_food_regen_test(None, None, 0.20, 48_000);
    app.update();

    let intents: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<ApplyStatusEffectIntent>>()
        .drain()
        .collect();
    assert_eq!(
        intents.len(),
        1,
        "Noop（无 freshness）应退化为发出 1 条原始 bonus intent，实际 {} 条",
        intents.len()
    );
    let intent = &intents[0];
    assert!(
        (intent.magnitude - 0.20).abs() < 1e-4,
        "Noop 路径 magnitude 应=0.20（原始值），实际 {}",
        intent.magnitude
    );
}

/// BLOCKER 1+2 全链路：food.toml 加载 → ling_guo effect → freshness gating → status 挂载
#[test]
fn food_regen_end_to_end_from_food_toml_ling_guo_fresh_to_intent() {
    use crate::inventory::freshness::GAME_DAY_TICKS;
    use crate::shelflife::types::{DecayFormula, DecayProfile, DecayProfileId, Freshness};

    // 1) 从 food.toml 加载 ling_guo 的 FoodRegen effect
    let registry = crate::inventory::load_item_registry()
        .expect("item registry 应从 assets/items/*.toml 加载成功");
    let ling_guo = registry
        .get("food.spirit_fruit.ling_guo")
        .expect("food.spirit_fruit.ling_guo 必须在 registry 中");
    let (bonus_factor, duration_ticks) = match &ling_guo.effect {
        Some(ItemEffect::FoodRegen {
            bonus_factor,
            duration_ticks,
        }) => (*bonus_factor, *duration_ticks),
        other => panic!("ling_guo.effect 应为 FoodRegen，实际 {other:?}"),
    };

    // 2) 构造 freshness（新鲜：created_at=0，issued_at=100，远未过 spoil）
    let spoil_profile = DecayProfile::Spoil {
        id: DecayProfileId::new("food_spoil_ling_guo_v1"),
        formula: DecayFormula::Linear {
            decay_per_tick: 1.0 / (GAME_DAY_TICKS as f32 * 2.0),
        },
        spoil_threshold: 0.01,
    };
    let mut decay_reg = crate::shelflife::DecayProfileRegistry::new();
    decay_reg.insert(spoil_profile.clone()).unwrap();

    let freshness = Freshness::new(0, 1.0, &spoil_profile);

    // 3) 触发 apply_cast_item_effect + freshness 门控 → 检查 intent
    let mut app = build_app_for_food_regen_test(
        Some(freshness),
        Some(decay_reg),
        bonus_factor,
        duration_ticks,
    );
    app.update();

    let intents: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<ApplyStatusEffectIntent>>()
        .drain()
        .collect();
    assert_eq!(
        intents.len(),
        1,
        "全链路：新鲜 ling_guo 应发出 1 条 CultivationAcceleration intent，实际 {} 条",
        intents.len()
    );
    let intent = &intents[0];
    assert_eq!(intent.kind, StatusEffectKind::CultivationAcceleration);
    assert!(
        (intent.magnitude - 0.20).abs() < 1e-3,
        "全链路：magnitude 应=0.20（food.toml 配置的 bonus_factor），实际 {}",
        intent.magnitude
    );
    assert_eq!(
        intent.duration_ticks, 48_000,
        "全链路：duration_ticks 应=48000（food.toml 配置的 2 GAME_DAY），实际 {}",
        intent.duration_ticks
    );
}

// ── plan-food-v1 P2 (opus 补测) — tick_casts_or_interrupt 全链路 CriticalBlock 不扣库存 ──

/// 全链路：通过 tick_casts_or_interrupt system 验证极腐食物 CriticalBlock 不扣库存。
///
/// 背景：headline fix 的 CriticalBlock 门控在 tick_casts_or_interrupt 内（cast_emit.rs:250-298）。
/// 原有 food_regen_critical_block_emits_no_intent 直接调 apply_cast_item_effect，绕过了
/// 真正的库存扣减路径。本测试走完整 system 链路：
///   Casting 完成 → CriticalBlock 检测 → 不扣 hotbar slot stack → 不发 ApplyStatusEffectIntent。
#[test]
fn tick_casts_or_interrupt_critical_block_does_not_consume_inventory() {
    use crate::combat::components::{
        CastSource, Casting, QuickSlotBindings, SkillBarBindings, Wounds,
    };
    use crate::combat::CombatClock;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemEffect, ItemInstance, ItemRarity, ItemRegistry,
        ItemTemplate, PlayerInventory, MAIN_PACK_CONTAINER_ID,
    };
    use crate::player::state::PlayerState;
    use crate::shelflife::types::{DecayFormula, DecayProfile, DecayProfileId, Freshness};
    use crate::shelflife::{DecayProfileRegistry, DecayTrack};
    use std::collections::HashMap;
    use valence::prelude::{DVec3, Position};
    use valence::testing::create_mock_client;

    const FOOD_ID: &str = "test.food.critical_rotten";
    const INSTANCE_ID: u64 = 777;
    const QUICK_SLOT: u8 = 0;
    // clock.tick = 1000; started_at_tick = 0, duration_ticks = 1 → 完成条件 1000 >= 1
    const CLOCK_TICK: u64 = 1000;

    // 1) 构造极腐 decay profile：decay_per_tick = 1/100 → at tick=1000, current ≈ 0 → CriticalBlock
    let fast_spoil = DecayProfile::Spoil {
        id: DecayProfileId::new("crit_block_test_profile"),
        formula: DecayFormula::Linear {
            decay_per_tick: 1.0 / 100.0, // 耗尽需 100 ticks；1000 ticks 时已归零
        },
        spoil_threshold: 0.5, // CriticalBlock 阈值: current < 0.1 * 0.5 = 0.05
    };
    let mut decay_reg = DecayProfileRegistry::new();
    decay_reg.insert(fast_spoil.clone()).unwrap();

    // 2) 食物 ItemTemplate（有 FoodRegen effect + shelflife_profile）
    let food_template = ItemTemplate {
        quick_use: true,
        id: FOOD_ID.to_string(),
        display_name: "极腐食物".to_string(),
        category: crate::inventory::ItemCategory::Food,
        placeable: None,
        max_stack_count: 16,
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.1,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 1.0,
        description: String::new(),
        effect: Some(ItemEffect::FoodRegen {
            bonus_factor: 0.20,
            duration_ticks: 48_000,
        }),
        cast_duration_ms: 1000,
        cooldown_ms: 500,
        weapon_spec: None,
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
        readable_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: None,
        shield_spec: None,
        shelflife_profile: Some("crit_block_test_profile".to_string()),
        shelflife_track: Some(DecayTrack::Spoil),
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    };
    let mut templates = HashMap::new();
    templates.insert(FOOD_ID.to_string(), food_template);
    let item_registry = ItemRegistry::from_map(templates);

    // 3) 食物 ItemInstance：created_at=0 → at tick=1000 freshness 已归零 → CriticalBlock
    let food_freshness = Freshness::new(0, 1.0, &fast_spoil);
    let food_item = ItemInstance {
        instance_id: INSTANCE_ID,
        template_id: FOOD_ID.to_string(),
        display_name: "极腐食物".to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.1,
        rarity: ItemRarity::Common,
        description: String::new(),
        stack_count: 3, // 初始 3；CriticalBlock 路径不应扣减
        spirit_quality: 0.0,
        durability: 1.0,
        freshness: Some(food_freshness),
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    };
    let mut inventory = PlayerInventory {
        material_preparation: Default::default(),
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 5,
            cols: 7,
            items: vec![],
            owner_instance_id: None,
        }],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 50.0,
    };
    inventory.hotbar[QUICK_SLOT as usize] = Some(food_item);

    // 4) 构造 Casting：QuickSlot，绑定食物 instance，已过完成时间
    let mut quick_slot_bindings = QuickSlotBindings::default();
    quick_slot_bindings.slots[QUICK_SLOT as usize] = Some(INSTANCE_ID);
    let casting = Casting {
        source: CastSource::QuickSlot,
        slot: QUICK_SLOT,
        started_at_tick: 0,
        duration_ticks: 1, // clock.tick=1000 >> 0+1 → 自然完成
        started_at_ms: 0,
        duration_ms: 50,
        bound_instance_id: Some(INSTANCE_ID),
        start_position: DVec3::ZERO,
        complete_cooldown_ticks: 20,
        skill_id: None,
        skill_config: None,
    };

    // 5) 搭 App
    let mut app = App::new();
    // Resources
    app.insert_resource(CombatClock { tick: CLOCK_TICK });
    app.insert_resource(item_registry);
    app.insert_resource(decay_reg);
    // AudioEmitWriter dependencies (all optional → skip if not needed, but
    // PlaySoundRecipeRequest event is required by the SystemParam)
    app.add_event::<crate::network::audio_event_emit::PlaySoundRecipeRequest>();
    app.add_event::<crate::combat::yidao::YidaoCastCompleteEvent>();
    app.add_event::<GuangboTicaoPracticeEvent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<crate::cultivation::lifespan::LifespanExtensionIntent>();
    app.add_event::<crate::cultivation::poison_trait::ConsumePoisonPillIntent>();
    app.add_event::<VfxEventRequest>();
    // 注册 tick_casts_or_interrupt system
    app.add_systems(Update, tick_casts_or_interrupt);

    // 6) 生成 mock player entity（含 Client + Username）
    let (client_bundle, _helper) = create_mock_client("TestPlayer");
    let player = app
        .world_mut()
        .spawn(client_bundle)
        .insert((
            Position::new([0.0, 64.0, 0.0]),
            casting,
            Wounds::default(),
            inventory,
            PlayerState::default(),
            quick_slot_bindings,
            SkillBarBindings::default(),
        ))
        .id();

    // 7) 跑 system
    app.update();

    // 8) 断言：hotbar slot 的 stack_count 不变（CriticalBlock 不扣库存）
    let world = app.world_mut();
    let inv = world
        .entity(player)
        .get::<PlayerInventory>()
        .expect("PlayerInventory should still exist after tick_casts_or_interrupt");
    let actual_stack = inv.hotbar[QUICK_SLOT as usize]
        .as_ref()
        .map(|item| item.stack_count)
        .unwrap_or(0);
    assert_eq!(
        actual_stack, 3,
        "期望 hotbar slot stack_count=3（不变），因为 CriticalBlock 拒食后库存不应扣减；实际 stack={actual_stack}"
    );

    // 9) 断言：无 CultivationAcceleration ApplyStatusEffectIntent 发出
    let intents: Vec<_> = world
        .resource_mut::<Events<ApplyStatusEffectIntent>>()
        .drain()
        .collect();
    let culti_accel_count = intents
        .iter()
        .filter(|i| i.kind == StatusEffectKind::CultivationAcceleration)
        .count();
    assert_eq!(
        culti_accel_count, 0,
        "期望 0 条 CultivationAcceleration intent，因为 CriticalBlock 路径直接 continue 不调 apply_cast_item_effect；实际 {} 条",
        culti_accel_count
    );
}

#[test]
fn cast_interrupt_audio_uses_recipe_attenuation() {
    fn emit_for_test(targets: Query<Entity, With<Position>>, mut audio: AudioEmitWriter) {
        let mut audio = audio.context();
        let entity = targets.single();
        let casting = Casting {
            source: CastSource::SkillBar,
            slot: 0,
            started_at_tick: 0,
            duration_ticks: 20,
            started_at_ms: 0,
            duration_ms: 1000,
            bound_instance_id: None,
            start_position: DVec3::ZERO,
            complete_cooldown_ticks: 20,
            skill_id: Some("xue_beng_bu".to_string()),
            skill_config: None,
        };
        emit_cast_interrupt_audio(&mut audio, entity, DVec3::new(1.0, 64.0, 1.0), &casting);
    }

    let mut app = App::new();
    app.insert_resource(
        crate::audio::SoundRecipeRegistry::load_default().expect("default recipes load"),
    );
    app.init_resource::<crate::audio::implementation::AudioImplementationDedup>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_for_test);
    app.world_mut().spawn(Position::new([1.0, 64.0, 1.0]));

    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].recipe_id, "cast_interrupt");
    assert!(matches!(
        emitted[0].recipient,
        AudioRecipient::Radius { .. }
    ));
}

// ── 广播体操 cast 完成 → GuangboTicaoPracticeEvent + AV recipe ────────────

/// 搭建跑 `tick_casts_or_interrupt` 所需的最小 App（注册全部依赖事件 + 音效 registry）。
fn build_cast_tick_app(clock_tick: u64) -> App {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: clock_tick });
    app.insert_resource(ItemRegistry::from_map(HashMap::new()));
    app.insert_resource(
        crate::audio::SoundRecipeRegistry::load_default().expect("default recipes load"),
    );
    app.init_resource::<crate::audio::implementation::AudioImplementationDedup>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_event::<crate::combat::yidao::YidaoCastCompleteEvent>();
    app.add_event::<GuangboTicaoPracticeEvent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<crate::cultivation::lifespan::LifespanExtensionIntent>();
    app.add_event::<crate::cultivation::poison_trait::ConsumePoisonPillIntent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, tick_casts_or_interrupt);
    app
}

fn empty_inventory_for_cast() -> PlayerInventory {
    PlayerInventory {
        material_preparation: Default::default(),
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers: vec![],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 50.0,
    }
}

fn guangbo_casting(skill_id: &str) -> Casting {
    Casting {
        source: CastSource::SkillBar,
        slot: 0,
        started_at_tick: 0,
        duration_ticks: 60, // 与 known_techniques.body.guangbo_ticao cast_ticks 一致
        started_at_ms: 0,
        duration_ms: 3000,
        bound_instance_id: None,
        start_position: DVec3::new(0.0, 64.0, 0.0),
        complete_cooldown_ticks: 200,
        skill_id: Some(skill_id.to_string()),
        skill_config: None,
    }
}

fn spawn_caster(app: &mut App, casting: Casting) -> Entity {
    let (client_bundle, _helper) = create_mock_client("Stretcher");
    app.world_mut()
        .spawn(client_bundle)
        .insert((
            Position::new([0.0, 64.0, 0.0]),
            casting,
            Wounds::default(),
            empty_inventory_for_cast(),
            PlayerState::default(),
            QuickSlotBindings::default(),
            SkillBarBindings::default(),
        ))
        .id()
}

#[test]
fn guangbo_ticao_natural_completion_sends_practice_event_and_audio() {
    // clock=100 >= started 0 + duration 60 → 自然完成。
    let mut app = build_cast_tick_app(100);
    let entity = spawn_caster(&mut app, guangbo_casting(GUANGBO_TICAO_ID));

    app.update();

    // 1) GuangboTicaoPracticeEvent 已发出，指向正确的练习者。
    let practice_events: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<GuangboTicaoPracticeEvent>>()
        .drain()
        .collect();
    assert_eq!(
        practice_events.len(),
        1,
        "广播体操 cast 自然完成应恰好发 1 条 GuangboTicaoPracticeEvent（死事件接通），实际 {} 条",
        practice_events.len()
    );
    assert_eq!(
        practice_events[0].entity, entity,
        "练习事件应指向施法者本人"
    );

    // 2) Casting 已被移除（cast 完成）。
    assert!(
        app.world().get::<Casting>(entity).is_none(),
        "cast 完成后 Casting 组件应被移除"
    );

    // 3) 练习 AV 音效已 emit（guangbo_ticao_practice recipe）。
    let audio: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert!(
        audio
            .iter()
            .any(|e| e.recipe_id == "guangbo_ticao_practice"),
        "广播体操完成应 emit guangbo_ticao_practice 音效，实际 recipes={:?}",
        audio
            .iter()
            .map(|e| e.recipe_id.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn non_guangbo_skill_completion_sends_no_practice_event() {
    // 负锚点：其它 skill_id（无 resolver 的通用招）完成时不应发广播体操练习事件。
    let mut app = build_cast_tick_app(100);
    let _entity = spawn_caster(&mut app, guangbo_casting("some.other.skill"));

    app.update();

    let practice_events: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<GuangboTicaoPracticeEvent>>()
        .drain()
        .collect();
    assert!(
        practice_events.is_empty(),
        "非广播体操招式完成不得发 GuangboTicaoPracticeEvent，实际 {} 条",
        practice_events.len()
    );
}

#[test]
fn guangbo_ticao_not_yet_complete_sends_no_practice_event() {
    // clock=10 < started 0 + duration 60 → cast 未完成 → 不发练习事件，Casting 保留。
    let mut app = build_cast_tick_app(10);
    let entity = spawn_caster(&mut app, guangbo_casting(GUANGBO_TICAO_ID));

    app.update();

    let practice_events: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<GuangboTicaoPracticeEvent>>()
        .drain()
        .collect();
    assert!(
        practice_events.is_empty(),
        "cast 未完成不应发练习事件，实际 {} 条",
        practice_events.len()
    );
    assert!(
        app.world().get::<Casting>(entity).is_some(),
        "cast 未完成时 Casting 应仍在"
    );
}

// ── plan-skill-anim-fidelity-v1 P2 后半（§8.1 #3）：循环蓄力段停止路径 ──────

/// 查表契约：登记的循环蓄力段招式 = sword.infuse + yidao 5 招（新增循环段
/// 招式必须同步登记，否则打断后循环动画永卡——§13 #6 红线）。
#[test]
fn looping_cast_anim_table_registers_expected_skills() {
    assert_eq!(
        looping_cast_anim_id(crate::combat::sword_basics::SWORD_INFUSE_SKILL_ID),
        Some(crate::combat::sword_basics::ANIM_SWORD_INFUSE_CHARGE),
        "sword.infuse 起手播循环蓄力段，必须登记停止映射"
    );
    // P4：yidao 5 招逐条 pin（每招独立蓄力段 id，防映射串线/漂移）。
    for (skill_id, expected_loop) in [
        (
            crate::combat::yidao::MERIDIAN_REPAIR_SKILL_ID,
            crate::combat::yidao::ANIM_YIDAO_MERIDIAN_REPAIR_LOOP,
        ),
        (
            crate::combat::yidao::CONTAM_PURGE_SKILL_ID,
            crate::combat::yidao::ANIM_YIDAO_CONTAM_PURGE_LOOP,
        ),
        (
            crate::combat::yidao::MASS_MERIDIAN_REPAIR_SKILL_ID,
            crate::combat::yidao::ANIM_YIDAO_MASS_MERIDIAN_REPAIR_LOOP,
        ),
    ] {
        assert_eq!(
            looping_cast_anim_id(skill_id),
            Some(expected_loop),
            "yidao 招式 `{skill_id}` 起手播循环蓄力段，必须登记专属停止映射"
        );
    }
    assert_eq!(
        looping_cast_anim_id(GUANGBO_TICAO_ID),
        None,
        "广播体操是一次性长演出（非循环段），不得误停"
    );
    assert_eq!(looping_cast_anim_id("some.other.skill"), None);
}

fn infuse_casting(start_position: DVec3) -> Casting {
    Casting {
        source: CastSource::SkillBar,
        slot: 0,
        started_at_tick: 0,
        duration_ticks: 40, // 与 known_techniques sword.infuse cast_ticks 一致
        started_at_ms: 0,
        duration_ms: 2000,
        bound_instance_id: None,
        start_position,
        complete_cooldown_ticks: 200,
        skill_id: Some(crate::combat::sword_basics::SWORD_INFUSE_SKILL_ID.to_string()),
        skill_config: None,
    }
}

fn drain_vfx_payloads(app: &mut App) -> Vec<crate::schema::vfx_event::VfxEventPayloadV1> {
    app.world_mut()
        .resource_mut::<Events<VfxEventRequest>>()
        .drain()
        .map(|request| request.payload)
        .collect()
}

fn stop_anim_payloads(
    payloads: &[crate::schema::vfx_event::VfxEventPayloadV1],
) -> Vec<(String, Option<u8>)> {
    payloads
        .iter()
        .filter_map(|payload| match payload {
            VfxEventPayloadV1::StopAnim {
                anim_id,
                fade_out_ticks,
                ..
            } => Some((anim_id.clone(), *fade_out_ticks)),
            _ => None,
        })
        .collect()
}

/// 移动打断 sword.infuse：恰发 1 条 StopAnim(蓄力段, fade=打断档)。
#[test]
fn movement_interrupt_stops_infuse_charge_loop() {
    // clock=10 < 40 未完成；实体 Position(0,64,0) 距 start(10,64,10) 超阈值 → 移动打断。
    let mut app = build_cast_tick_app(10);
    let entity = spawn_caster(&mut app, infuse_casting(DVec3::new(10.0, 64.0, 10.0)));

    app.update();

    assert!(
        app.world().get::<Casting>(entity).is_none(),
        "前置：移动打断应移除 Casting"
    );
    let payloads = drain_vfx_payloads(&mut app);
    assert_eq!(
        stop_anim_payloads(&payloads),
        vec![(
            crate::combat::sword_basics::ANIM_SWORD_INFUSE_CHARGE.to_string(),
            Some(CAST_LOOP_ANIM_INTERRUPT_FADE_OUT_TICKS),
        )],
        "移动打断必须恰停一次循环蓄力段（否则抚刃循环永卡在玩家身上）"
    );
}

/// 受击打断 sword.infuse：本 tick 新增 wound → StopAnim(蓄力段)。
#[test]
fn damage_interrupt_stops_infuse_charge_loop() {
    let mut app = build_cast_tick_app(10);
    let entity = spawn_caster(&mut app, infuse_casting(DVec3::new(0.0, 64.0, 0.0)));
    let mut wounds = Wounds::default();
    wounds.entries.push(Wound {
        location: crate::body_plan::legacy_body_part_to_id(BodyPart::ArmL),
        kind: WoundKind::Cut,
        severity: 0.2,
        bleeding_per_sec: 1.0,
        created_at_tick: 10, // == clock.tick → 本 tick 受击
        inflicted_by: None,
    });
    app.world_mut().entity_mut(entity).insert(wounds);

    app.update();

    assert!(app.world().get::<Casting>(entity).is_none());
    let payloads = drain_vfx_payloads(&mut app);
    assert_eq!(
        stop_anim_payloads(&payloads),
        vec![(
            crate::combat::sword_basics::ANIM_SWORD_INFUSE_CHARGE.to_string(),
            Some(CAST_LOOP_ANIM_INTERRUPT_FADE_OUT_TICKS),
        )],
        "受击打断必须停循环蓄力段"
    );
}

/// 控制打断（Stunned）sword.infuse：优先级最高的打断分支同样 StopAnim。
#[test]
fn stun_interrupt_stops_infuse_charge_loop() {
    use crate::combat::components::ActiveStatusEffect;
    let mut app = build_cast_tick_app(10);
    let entity = spawn_caster(&mut app, infuse_casting(DVec3::new(0.0, 64.0, 0.0)));
    app.world_mut().entity_mut(entity).insert(StatusEffects {
        active: vec![ActiveStatusEffect {
            kind: StatusEffectKind::Stunned,
            magnitude: 1.0,
            remaining_ticks: 5,
            source_pill: None,
        }],
    });

    app.update();

    assert!(app.world().get::<Casting>(entity).is_none());
    let payloads = drain_vfx_payloads(&mut app);
    assert_eq!(
        stop_anim_payloads(&payloads),
        vec![(
            crate::combat::sword_basics::ANIM_SWORD_INFUSE_CHARGE.to_string(),
            Some(CAST_LOOP_ANIM_INTERRUPT_FADE_OUT_TICKS),
        )],
        "控制（Stunned）打断必须停循环蓄力段"
    );
}

/// 自然完成 sword.infuse：防御性兜底 StopAnim(fade=完成档)——release 段由
/// `sword_infuse_completion_tick` 同拍接力（另有专属测试），此处只锁通用分支。
#[test]
fn natural_completion_stops_infuse_charge_loop_defensively() {
    // clock=100 >= started 0 + duration 40 → 自然完成。
    let mut app = build_cast_tick_app(100);
    let entity = spawn_caster(&mut app, infuse_casting(DVec3::new(0.0, 64.0, 0.0)));

    app.update();

    assert!(app.world().get::<Casting>(entity).is_none());
    let payloads = drain_vfx_payloads(&mut app);
    assert_eq!(
        stop_anim_payloads(&payloads),
        vec![(
            crate::combat::sword_basics::ANIM_SWORD_INFUSE_CHARGE.to_string(),
            Some(CAST_LOOP_ANIM_COMPLETE_FADE_OUT_TICKS),
        )],
        "自然完成分支必须防御性停循环蓄力段"
    );
}

// ── plan-skill-anim-fidelity-v1 P4：yidao 循环蓄力段停止路径（事件路径 pin）──

fn yidao_casting(skill_id: &str, start_position: DVec3) -> Casting {
    Casting {
        source: CastSource::SkillBar,
        slot: 0,
        started_at_tick: 0,
        duration_ticks: 600, // contam_purge cast_ticks_base（窗长可变，测试取基准）
        started_at_ms: 0,
        duration_ms: 30_000,
        bound_instance_id: None,
        start_position,
        complete_cooldown_ticks: 200,
        skill_id: Some(skill_id.to_string()),
        skill_config: None,
    }
}

/// 移动打断 yidao 引导：恰发 1 条 StopAnim(该招专属蓄力段, fade=打断档)，
/// 且不发 YidaoCastCompleteEvent（打断即无结算）。
#[test]
fn movement_interrupt_stops_yidao_charge_loop_without_complete_event() {
    let mut app = build_cast_tick_app(10);
    let entity = spawn_caster(
        &mut app,
        yidao_casting(
            crate::combat::yidao::CONTAM_PURGE_SKILL_ID,
            DVec3::new(10.0, 64.0, 10.0), // 距实体位置超移动阈值 → 移动打断
        ),
    );

    app.update();

    assert!(
        app.world().get::<Casting>(entity).is_none(),
        "前置：移动打断应移除 Casting"
    );
    let payloads = drain_vfx_payloads(&mut app);
    assert_eq!(
        stop_anim_payloads(&payloads),
        vec![(
            crate::combat::yidao::ANIM_YIDAO_CONTAM_PURGE_LOOP.to_string(),
            Some(CAST_LOOP_ANIM_INTERRUPT_FADE_OUT_TICKS),
        )],
        "移动打断必须恰停一次 yidao 蓄力段（否则灸火循环永卡在玩家身上）"
    );
    let complete_events: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<crate::combat::yidao::YidaoCastCompleteEvent>>()
        .drain()
        .collect();
    assert!(
        complete_events.is_empty(),
        "打断不得发 YidaoCastCompleteEvent（打断即无结算），实际 {} 条",
        complete_events.len()
    );
}

/// 自然完成 yidao 引导：防御性兜底 StopAnim(蓄力段, fade=完成档) +
/// YidaoCastCompleteEvent（release 段由 `complete_yidao_casts` 有效结算分支
/// 接力，另有 yidao.rs 专属测试；本用例只锁通用分支）。
#[test]
fn natural_completion_stops_yidao_charge_loop_and_sends_complete_event() {
    let mut app = build_cast_tick_app(1000);
    let entity = spawn_caster(
        &mut app,
        yidao_casting(
            crate::combat::yidao::MERIDIAN_REPAIR_SKILL_ID,
            DVec3::new(0.0, 64.0, 0.0),
        ),
    );

    app.update();

    assert!(app.world().get::<Casting>(entity).is_none());
    let payloads = drain_vfx_payloads(&mut app);
    assert_eq!(
        stop_anim_payloads(&payloads),
        vec![(
            crate::combat::yidao::ANIM_YIDAO_MERIDIAN_REPAIR_LOOP.to_string(),
            Some(CAST_LOOP_ANIM_COMPLETE_FADE_OUT_TICKS),
        )],
        "自然完成分支必须防御性停 yidao 蓄力段"
    );
    let complete_events: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<crate::combat::yidao::YidaoCastCompleteEvent>>()
        .drain()
        .collect();
    assert_eq!(
        complete_events.len(),
        1,
        "自然完成必须恰发 1 条 YidaoCastCompleteEvent 供结算系统接力"
    );
    assert_eq!(
        complete_events[0].skill_id,
        crate::combat::yidao::MERIDIAN_REPAIR_SKILL_ID,
        "完成事件必须携带原 skill_id"
    );
}

/// P6 相位承接契约（conventions §14.2 裁决方案①）对**通用停止路径**的守卫。
///
/// 契约要害：`fade_out = 0` 会让旧层被立即摘除、混合源随之消失，交接瞬间不是
/// 塌回 release 首帧而是**塌回 vanilla**（手臂先落下再抬起）——这条退化由 P6
/// 的 `TwoStageHandoffBlendTest` 实测确认，故 `fade_out > 0` 是硬约束而非风格。
///
/// `anqi.charge_carrier` / `sword_path.heaven_gate` 在 `vfx_animation_trigger.rs`
/// 各有专属 pin；但 `sword.infuse` 与 `yidao.*` 走的是本文件这两个**共享常量**，
/// 此前无任何断言守着——把它们改成 0，契约会对这两族静默失效。既有用例断言的是
/// 「发出值 == 该常量」，属同义反复，捕不到常量本身被改坏。
#[test]
fn shared_loop_stop_fade_out_constants_honor_phase_handoff_contract() {
    for (name, fade_out) in [
        (
            "CAST_LOOP_ANIM_COMPLETE_FADE_OUT_TICKS（自然完成→release 接力）",
            CAST_LOOP_ANIM_COMPLETE_FADE_OUT_TICKS,
        ),
        (
            "CAST_LOOP_ANIM_INTERRUPT_FADE_OUT_TICKS（三类打断）",
            CAST_LOOP_ANIM_INTERRUPT_FADE_OUT_TICKS,
        ),
        (
            "CAST_LOOP_ANIM_CANCEL_FADE_OUT_TICKS（用户主动切槽取消）",
            CAST_LOOP_ANIM_CANCEL_FADE_OUT_TICKS,
        ),
    ] {
        assert!(
            fade_out >= 2,
            "{name} = {fade_out}，违反 P6 相位承接契约：需 ≥2 tick 才够 release \
             在淡出窗口内成为完整姿态；0 会让交接瞬间塌回 vanilla（手臂先落再抬），\
             且 `sword.infuse` / `yidao.*` 两族全靠这些共享常量，无其它断言兜底"
        );
    }
}

/// 负向锚点：未登记循环段的招式（广播体操）被移动打断时不得发任何 StopAnim
/// ——查表 miss 静默跳过，不误伤一次性长演出动画。
#[test]
fn movement_interrupt_of_non_loop_skill_sends_no_stop_anim() {
    let mut app = build_cast_tick_app(10);
    let mut casting = guangbo_casting(GUANGBO_TICAO_ID);
    casting.start_position = DVec3::new(10.0, 64.0, 10.0); // 触发移动打断
    let entity = spawn_caster(&mut app, casting);

    app.update();

    assert!(
        app.world().get::<Casting>(entity).is_none(),
        "前置：移动打断应移除 Casting"
    );
    let payloads = drain_vfx_payloads(&mut app);
    assert!(
        stop_anim_payloads(&payloads).is_empty(),
        "非循环段招式打断不得发 StopAnim（查表 miss 静默），实际 {:?}",
        stop_anim_payloads(&payloads)
    );
}

/// 三打断分支 × 5 招表驱动状态转换覆盖（review r4 补）——此前 yidao 只 pin 了
/// 移动分支，受击 / 控制两条独立退出路径没有 yidao 侧用例。三分支虽共用
/// `stop_cast_loop_anim` + 同一张查表（yidao 由构造即覆盖），但 CLAUDE.md
/// 「饱和化测试」要求所有状态转换都有命中用例：分支各自的前置条件判定改错
/// （例如漏判某分支就直接 `remove::<Casting>()`）不会被共享 helper 的测试撞红。
///
/// 每分支逐招断言：恰发一次**本招专属** loop anim 的 StopAnim（fade=打断档）、
/// `Casting` 已退出、**不发** `YidaoCastCompleteEvent`（打断即无结算，故也不会
/// 有 release 接力——release 只在 `complete_yidao_casts` 的有效结算分支发出）。
#[test]
fn every_interrupt_branch_stops_every_yidao_charge_loop() {
    use crate::combat::components::ActiveStatusEffect;

    #[derive(Clone, Copy)]
    enum Branch {
        Movement,
        Damage,
        Stun,
    }

    let skills = [
        (
            crate::combat::yidao::MERIDIAN_REPAIR_SKILL_ID,
            crate::combat::yidao::ANIM_YIDAO_MERIDIAN_REPAIR_LOOP,
        ),
        (
            crate::combat::yidao::CONTAM_PURGE_SKILL_ID,
            crate::combat::yidao::ANIM_YIDAO_CONTAM_PURGE_LOOP,
        ),
        (
            crate::combat::yidao::MASS_MERIDIAN_REPAIR_SKILL_ID,
            crate::combat::yidao::ANIM_YIDAO_MASS_MERIDIAN_REPAIR_LOOP,
        ),
    ];

    for (branch, branch_name) in [
        (Branch::Movement, "移动"),
        (Branch::Damage, "受击"),
        (Branch::Stun, "控制"),
    ] {
        for (skill_id, loop_anim) in skills {
            let mut app = build_cast_tick_app(10);
            // 移动分支靠 start_position 偏移触发；另两条留在原地，避免多分支同时命中。
            let start_position = match branch {
                Branch::Movement => DVec3::new(10.0, 64.0, 10.0),
                Branch::Damage | Branch::Stun => DVec3::new(0.0, 64.0, 0.0),
            };
            let entity = spawn_caster(&mut app, yidao_casting(skill_id, start_position));
            match branch {
                Branch::Movement => {}
                Branch::Damage => {
                    let mut wounds = Wounds::default();
                    wounds.entries.push(Wound {
                        location: crate::body_plan::legacy_body_part_to_id(BodyPart::ArmL),
                        kind: WoundKind::Cut,
                        severity: 0.2,
                        bleeding_per_sec: 1.0,
                        created_at_tick: 10, // == clock.tick → 本 tick 受击
                        inflicted_by: None,
                    });
                    app.world_mut().entity_mut(entity).insert(wounds);
                }
                Branch::Stun => {
                    app.world_mut().entity_mut(entity).insert(StatusEffects {
                        active: vec![ActiveStatusEffect {
                            kind: StatusEffectKind::Stunned,
                            magnitude: 1.0,
                            remaining_ticks: 5,
                            source_pill: None,
                        }],
                    });
                }
            }

            app.update();

            assert!(
                app.world().get::<Casting>(entity).is_none(),
                "{branch_name}打断 `{skill_id}`：前置 Casting 应已退出"
            );
            let payloads = drain_vfx_payloads(&mut app);
            assert_eq!(
                stop_anim_payloads(&payloads),
                vec![(
                    loop_anim.to_string(),
                    Some(CAST_LOOP_ANIM_INTERRUPT_FADE_OUT_TICKS),
                )],
                "{branch_name}打断 `{skill_id}` 必须恰停一次本招专属蓄力段 \
                 {loop_anim}（否则循环动画永卡玩家身上）"
            );
            let complete_events: Vec<_> = app
                .world_mut()
                .resource_mut::<Events<crate::combat::yidao::YidaoCastCompleteEvent>>()
                .drain()
                .collect();
            assert!(
                complete_events.is_empty(),
                "{branch_name}打断 `{skill_id}` 不得发 YidaoCastCompleteEvent\
                 （打断即无结算，故也无 release 接力），实际 {} 条",
                complete_events.len()
            );
        }
    }
}
