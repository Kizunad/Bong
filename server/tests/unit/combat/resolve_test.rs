use bong_server::combat::baomai_v4::dead_armor::{should_block_contamination, DeadMeridianArmor};
use bong_server::combat::components::{
    ActiveStatusEffect, BodyPart, CombatState, DerivedAttrs, Lifecycle, LifecycleState,
    RevivalDecision, Stamina, StatusEffects, WoundKind, Wounds,
};
use bong_server::combat::events::{
    ApplyStatusEffectIntent, AttackIntent, AttackReach, AttackSource, CombatEvent, DeathEvent,
    DefenseIntent, StatusEffectKind, FIST_REACH,
};
use bong_server::combat::knockback::{KnockbackEvent, DEFAULT_CHAIN_DEPTH};
use bong_server::combat::resolve::{apply_defense_intents, resolve_attack_intents};
use bong_server::combat::shield_block::shield_fov_check;
use bong_server::combat::weapon::{ShieldBlockHit, ShieldBroken, WeaponBroken};
use bong_server::combat::{is_damageable, CombatClock};
use bong_server::cultivation::components::{Contamination, Cultivation, MeridianSystem, Realm};
use bong_server::cultivation::life_record::LifeRecord;
use bong_server::inventory::{
    InventoryDurabilityChangedEvent, InventoryRevision, ItemInstance, ItemRarity, PlayerInventory,
    SlotContents, EQUIP_SLOT_CHEST,
};
use bong_server::npc::movement::PendingKnockback;
use bong_server::player::state::{canonical_player_id, PlayerState};
use bong_server::qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL;
use bong_server::qi_physics::{WorldQiAccount, WorldQiBudget};
use valence::entity::Look;
use valence::prelude::{
    bevy_ecs, App, DVec3, Entity, Events, GameMode, IntoSystemConfigs, Position, Query, Update,
};
use valence::testing::create_mock_client;

fn resolve_app(tick: u64) -> App {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick });
    app.insert_resource(WorldQiAccount::default());
    app.insert_resource(WorldQiBudget::from_total(DEFAULT_SPIRIT_QI_TOTAL));
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<KnockbackEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<WeaponBroken>();
    app.add_event::<ShieldBroken>();
    app.add_event::<ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);
    app
}

fn spawn_player(app: &mut App, username: &str, position: [f64; 3]) -> Entity {
    let (mut client_bundle, _helper) = create_mock_client(username);
    client_bundle.player.position = Position::new(position);
    let entity = app
        .world_mut()
        .spawn((
            client_bundle,
            Cultivation {
                realm: Realm::Induce,
                qi_current: 60.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
            PlayerState {
                karma: 0.0,
                inventory_score: 0.0,
            },
            MeridianSystem::default(),
            LifeRecord::new(canonical_player_id(username)),
            Contamination::default(),
            StatusEffects::default(),
            Wounds::default(),
            Stamina::default(),
            CombatState::default(),
            DerivedAttrs::default(),
            Lifecycle {
                character_id: canonical_player_id(username),
                ..Default::default()
            },
        ))
        .id();
    app.world_mut()
        .entity_mut(entity)
        .insert(GameMode::Survival);
    entity
}

fn send_melee(app: &mut App, attacker: Entity, target: Entity, reach: AttackReach) {
    send_melee_with_qi(app, attacker, target, reach, 0.0, 44);
}

fn send_melee_with_qi(
    app: &mut App,
    attacker: Entity,
    target: Entity,
    reach: AttackReach,
    qi_invest: f64,
    issued_at_tick: u64,
) {
    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick,
        reach,
        qi_invest: qi_invest as f32,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
}

#[test]
fn hit_emits_knockback_event_and_pending_movement() {
    let mut app = resolve_app(44);
    let attacker = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
    let target = spawn_player(&mut app, "Crimson", [1.0, 64.0, 0.0]);
    send_melee_with_qi(&mut app, attacker, target, FIST_REACH, 10.0, 44);
    app.update();

    let pending = app
        .world()
        .get::<PendingKnockback>(target)
        .expect("公开 resolver 命中后应安装 pending knockback");
    assert_eq!(pending.attacker, Some(attacker));
    assert_eq!(pending.source, AttackSource::Melee);
    assert!(pending.distance_blocks > 0.0);
    assert_eq!(pending.chain_depth, DEFAULT_CHAIN_DEPTH);

    let events: Vec<_> = app
        .world()
        .resource::<Events<KnockbackEvent>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].attacker, attacker);
    assert_eq!(events[0].target, target);
    assert_eq!(events[0].collision_damage, None);
    assert!(!events[0].block_broken);
}

#[test]
fn head_hit_applies_stunned_status() {
    let mut app = resolve_app(1200);
    app.add_systems(
        Update,
        bong_server::combat::status::status_effect_apply_tick.after(resolve_attack_intents),
    );
    let attacker = spawn_player(&mut app, "Azure", [0.0, 65.0, 0.0]);
    let target = spawn_player(&mut app, "Crimson", [1.0, 64.0, 0.0]);
    send_melee_with_qi(&mut app, attacker, target, FIST_REACH, 10.0, 1199);
    app.update();
    app.update();

    let target_ref = app.world().entity(target);
    assert!(
        target_ref
            .get::<Wounds>()
            .unwrap()
            .entries
            .iter()
            .any(|wound| {
                wound.location == bong_server::body_plan::legacy_body_part_to_id(BodyPart::Head)
            }),
        "公开 resolver 的高位命中应写入 Head wound"
    );
    assert!(
        target_ref
            .get::<StatusEffects>()
            .unwrap()
            .active
            .iter()
            .any(|effect| effect.kind == StatusEffectKind::Stunned),
        "公开 resolver 的 Head 命中应通过公开状态系统施加 Stunned"
    );
}

#[test]
fn shield_fov_check_exact_boundary_dot_minus_half_triggers_block() {
    // 防御者朝 +Z；攻击者方向与 +Z 的点积精确为 -0.5，锁住公开 FOV
    // 边界的 >= 语义，不需要启动完整 resolver。
    let cos120 = 120.0_f64.to_radians().cos();
    let sin120 = 120.0_f64.to_radians().sin();
    let attacker_pos = DVec3::new(-sin120 * 2.0, 0.0, cos120 * 2.0);
    let defender_pos = DVec3::ZERO;
    let look = Look {
        yaw: 0.0,
        pitch: 0.0,
    };

    assert!(
        shield_fov_check(attacker_pos, defender_pos, Some(&look)),
        "公开 shield_fov_check 在 dot=-0.5 边界应允许格挡；若改为 >，此契约会回归失败"
    );
}

#[test]
fn dead_armor_arml_immune_in_multi_region_set() {
    let mut armor = DeadMeridianArmor::default();
    armor
        .immune_regions
        .insert(bong_server::combat::components::BodyPart::Chest);
    armor
        .immune_regions
        .insert(bong_server::combat::components::BodyPart::ArmL);

    assert!(
        should_block_contamination(&armor, bong_server::combat::components::BodyPart::ArmL),
        "公开 should_block_contamination 应识别多免疫区中的 ArmL"
    );
    assert!(
        should_block_contamination(&armor, bong_server::combat::components::BodyPart::Chest),
        "公开 should_block_contamination 应保留多免疫区中的 Chest"
    );
    assert!(
        !should_block_contamination(&armor, bong_server::combat::components::BodyPart::Abdomen),
        "公开 should_block_contamination 不应把未登记的 Abdomen 误判为免疫"
    );
}

#[test]
fn apply_defense_intent_ignored_while_stunned() {
    let mut app = App::new();
    app.add_event::<DefenseIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_systems(Update, apply_defense_intents);

    let defender = app
        .world_mut()
        .spawn((
            CombatState::default(),
            Cultivation {
                realm: Realm::Induce,
                qi_current: 10.0,
                qi_max: 10.0,
                ..Cultivation::default()
            },
            StatusEffects {
                active: vec![bong_server::combat::components::ActiveStatusEffect {
                    kind: bong_server::combat::events::StatusEffectKind::Stunned,
                    magnitude: 1.0,
                    remaining_ticks: 20,
                    source_pill: None,
                }],
            },
        ))
        .id();

    app.world_mut().send_event(DefenseIntent {
        defender,
        issued_at_tick: 10,
    });
    app.update();

    assert!(
        app.world()
            .entity(defender)
            .get::<CombatState>()
            .unwrap()
            .incoming_window
            .is_none(),
        "公开 apply_defense_intents 应拒绝眩晕状态下的防御意图"
    );
}

#[test]
fn apply_defense_intent_uses_realm_armor_and_adds_parry_recovery() {
    let mut app = App::new();
    app.add_event::<DefenseIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_systems(
        Update,
        (
            apply_defense_intents,
            bong_server::combat::status::status_effect_apply_tick.after(apply_defense_intents),
        ),
    );

    let defender = app
        .world_mut()
        .spawn((
            CombatState::default(),
            Cultivation {
                realm: Realm::Condense,
                qi_current: 12.0,
                qi_max: 20.0,
                ..Cultivation::default()
            },
            PlayerInventory {
                triggered_treasures: Vec::new(),
                revision: InventoryRevision(0),
                containers: Vec::new(),
                equipped: std::collections::HashMap::from([(
                    EQUIP_SLOT_CHEST.to_string(),
                    SlotContents::worn_single(ItemInstance {
                        instance_id: 90,
                        template_id: "heavy_armor".to_string(),
                        display_name: "heavy_armor".to_string(),
                        grid_w: 2,
                        grid_h: 2,
                        weight: 7.0,
                        rarity: ItemRarity::Common,
                        description: String::new(),
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
                    }),
                )]),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 50.0,
            },
            StatusEffects::default(),
        ))
        .id();

    app.world_mut().send_event(DefenseIntent {
        defender,
        issued_at_tick: 10,
    });
    app.update();

    let entity = app.world().entity(defender);
    let window = entity
        .get::<CombatState>()
        .unwrap()
        .incoming_window
        .as_ref()
        .expect("公开 apply_defense_intents 应打开截脉窗口");
    assert_eq!(window.duration_ms, 600);
    assert!(
        entity
            .get::<StatusEffects>()
            .unwrap()
            .active
            .iter()
            .any(|effect| effect.kind == StatusEffectKind::ParryRecovery),
        "公开防御入口应为成功的截脉准备添加 ParryRecovery"
    );
}

#[test]
fn resolve_public_game_mode_gate_respects_current_target_state() {
    let mut app = App::new();
    let (survival, creative) = {
        let world = app.world_mut();
        (
            world.spawn(GameMode::Survival).id(),
            world.spawn(GameMode::Creative).id(),
        )
    };

    let world = app.world_mut();
    let mut state = bevy_ecs::system::SystemState::<Query<&GameMode>>::new(world);
    let game_modes = state.get(world);

    assert!(
        is_damageable(survival, &game_modes),
        "Survival target must remain damageable through the public resolver gate"
    );
    assert!(
        !is_damageable(creative, &game_modes),
        "Creative target must be rejected by the public resolver gate"
    );
}

#[test]
fn resolve_public_lifecycle_and_game_mode_matrix() {
    let cases = [
        ("creative", None, true),
        ("near_death", Some(LifecycleState::NearDeath), false),
        (
            "awaiting_revival",
            Some(LifecycleState::AwaitingRevival),
            false,
        ),
        ("terminated", Some(LifecycleState::Terminated), false),
    ];

    for (name, lifecycle_state, creative) in cases {
        let mut app = resolve_app(44);
        let attacker = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
        let target = spawn_player(&mut app, "Crimson", [1.0, 64.0, 0.0]);
        if creative {
            app.world_mut()
                .entity_mut(target)
                .insert(GameMode::Creative);
        }
        if let Some(state) = lifecycle_state {
            let mut target_entity = app.world_mut().entity_mut(target);
            let mut lifecycle = target_entity.get_mut::<Lifecycle>().unwrap();
            match state {
                LifecycleState::NearDeath => lifecycle.enter_near_death(40),
                LifecycleState::AwaitingRevival => {
                    lifecycle.enter_near_death(40);
                    lifecycle.await_revival_decision(
                        bong_server::combat::components::RevivalDecision::Fortune { chance: 1.0 },
                        120,
                    );
                }
                LifecycleState::Terminated => lifecycle.terminate(40),
                other => panic!("unexpected lifecycle case {other:?}"),
            }
        }

        let before_health = app
            .world()
            .entity(target)
            .get::<Wounds>()
            .unwrap()
            .health_current;
        send_melee(&mut app, attacker, target, FIST_REACH);
        app.update();
        let after_health = app
            .world()
            .entity(target)
            .get::<Wounds>()
            .unwrap()
            .health_current;
        assert_eq!(
            after_health, before_health,
            "公开 resolver gate case {name} must reject the hit without changing target health"
        );
        assert!(
            app.world()
                .resource::<Events<CombatEvent>>()
                .iter_current_update_events()
                .next()
                .is_none(),
            "公开 resolver gate case {name} must not emit CombatEvent"
        );
    }
}

#[test]
fn resolve_public_attacker_lifecycle_matrix() {
    let cases = [
        ("near_death", LifecycleState::NearDeath),
        ("awaiting_revival", LifecycleState::AwaitingRevival),
        ("terminated", LifecycleState::Terminated),
    ];

    for (name, state) in cases {
        let mut app = resolve_app(44);
        let attacker = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
        let target = spawn_player(&mut app, "Crimson", [1.0, 64.0, 0.0]);
        {
            let mut attacker_entity = app.world_mut().entity_mut(attacker);
            let mut lifecycle = attacker_entity.get_mut::<Lifecycle>().unwrap();
            match state {
                LifecycleState::NearDeath => lifecycle.enter_near_death(40),
                LifecycleState::AwaitingRevival => {
                    lifecycle.enter_near_death(40);
                    lifecycle.await_revival_decision(RevivalDecision::Fortune { chance: 1.0 }, 120);
                }
                LifecycleState::Terminated => lifecycle.terminate(40),
                other => panic!("unexpected lifecycle case {other:?}"),
            }
        }

        let target_before = app.world().entity(target).get::<Wounds>().unwrap().clone();
        let attacker_qi_before = app
            .world()
            .entity(attacker)
            .get::<bong_server::cultivation::components::Cultivation>()
            .unwrap()
            .qi_current;
        send_melee_with_qi(&mut app, attacker, target, FIST_REACH, 10.0, 44);
        app.update();

        let target_after = app.world().entity(target).get::<Wounds>().unwrap();
        assert_eq!(
            target_after.health_current, target_before.health_current,
            "公开 resolver attacker lifecycle case {name} must not damage the target"
        );
        assert_eq!(
            target_after.entries.len(),
            target_before.entries.len(),
            "公开 resolver attacker lifecycle case {name} must not add a wound"
        );
        assert_eq!(
            app.world()
                .entity(attacker)
                .get::<bong_server::cultivation::components::Cultivation>()
                .unwrap()
                .qi_current,
            attacker_qi_before,
            "公开 resolver attacker lifecycle case {name} must not debit qi"
        );
        assert!(
            app.world()
                .resource::<Events<CombatEvent>>()
                .iter_current_update_events()
                .next()
                .is_none(),
            "公开 resolver attacker lifecycle case {name} must not emit CombatEvent"
        );
    }

    let mut app = resolve_app(44);
    let attacker = spawn_player(&mut app, "StunnedAzure", [0.0, 64.0, 0.0]);
    let target = spawn_player(&mut app, "StunnedCrimson", [1.0, 64.0, 0.0]);
    app.world_mut().entity_mut(attacker).insert(StatusEffects {
        active: vec![ActiveStatusEffect {
            kind: StatusEffectKind::Stunned,
            magnitude: 1.0,
            remaining_ticks: 20,
            source_pill: None,
        }],
    });
    let before = app
        .world()
        .entity(target)
        .get::<Wounds>()
        .unwrap()
        .health_current;
    send_melee_with_qi(&mut app, attacker, target, FIST_REACH, 10.0, 44);
    app.update();
    assert_eq!(
        app.world()
            .entity(target)
            .get::<Wounds>()
            .unwrap()
            .health_current,
        before,
        "公开 resolver 的 Stunned attacker gate 必须拒绝攻击且不改变目标生命"
    );
    assert!(
        app.world()
            .resource::<Events<CombatEvent>>()
            .iter_current_update_events()
            .next()
            .is_none(),
        "Stunned attacker 被公开 resolver 拒绝时不应发出 CombatEvent"
    );
}

#[test]
fn resolve_public_game_mode_switch_is_live() {
    let mut app = resolve_app(44);
    let attacker = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
    let target = spawn_player(&mut app, "Crimson", [1.0, 64.0, 0.0]);
    app.world_mut()
        .entity_mut(target)
        .insert(GameMode::Creative);
    let before = app
        .world()
        .entity(target)
        .get::<Wounds>()
        .unwrap()
        .health_current;

    send_melee_with_qi(&mut app, attacker, target, FIST_REACH, 10.0, 44);
    app.update();
    assert_eq!(
        app.world()
            .entity(target)
            .get::<Wounds>()
            .unwrap()
            .health_current,
        before,
        "creative target must be rejected before the mode switch"
    );

    app.world_mut()
        .entity_mut(target)
        .insert(GameMode::Survival);
    send_melee_with_qi(&mut app, attacker, target, FIST_REACH, 10.0, 45);
    app.update();
    assert!(
        app.world()
            .entity(target)
            .get::<Wounds>()
            .unwrap()
            .health_current
            < before,
        "switching to Survival must make the target damageable immediately"
    );
    assert!(
        app.world()
            .resource::<Events<CombatEvent>>()
            .iter_current_update_events()
            .next()
            .is_some(),
        "Survival target should emit CombatEvent"
    );
}

#[test]
fn resolve_public_reach_boundary_matrix() {
    let cases = [
        ("outside", 4.0, false),
        ("just_outside", f64::from(FIST_REACH.max) + 0.301, false),
        ("just_inside", f64::from(FIST_REACH.max) + 0.27, true),
    ];

    for (name, target_x, should_hit) in cases {
        let mut app = resolve_app(900);
        let attacker = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
        let target = spawn_player(&mut app, "Crimson", [target_x, 64.0, 0.0]);
        let before_health = app
            .world()
            .entity(target)
            .get::<Wounds>()
            .unwrap()
            .health_current;
        send_melee(&mut app, attacker, target, FIST_REACH);
        app.update();
        let after_health = app
            .world()
            .entity(target)
            .get::<Wounds>()
            .unwrap()
            .health_current;
        assert_eq!(
            after_health < before_health,
            should_hit,
            "公开 resolver reach case {name} hit={should_hit} but health was {before_health} -> {after_health}"
        );
        assert_eq!(
            !app.world().resource::<Events<CombatEvent>>().is_empty(),
            should_hit,
            "公开 resolver reach case {name} must align CombatEvent emission with hit result"
        );
    }
}
