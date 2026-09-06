use super::*;
use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
use crate::qi_physics::ledger::{assert_conservation, QiAccountId, QiTransfer};
use crate::qi_physics::{summarize_world_qi, WorldQiAccount, WorldQiBudget};
use crate::schema::common::SPIRIT_QI_TOTAL;

fn qi_test_app() -> App {
    let mut app = App::new();
    app.insert_resource(WorldQiAccount::default());
    app.insert_resource(WorldQiBudget::from_total(SPIRIT_QI_TOTAL));
    app
}

fn assert_full_qi_conservation(
    before: &crate::qi_physics::WorldQiSnapshot,
    after: &crate::qi_physics::WorldQiSnapshot,
    context: &str,
) {
    assert_eq!(
        before.budget_initial_total, SPIRIT_QI_TOTAL,
        "{context}: before snapshot must use the SPIRIT_QI_TOTAL budget anchor"
    );
    assert_eq!(
        after.budget_initial_total, SPIRIT_QI_TOTAL,
        "{context}: after snapshot must use the SPIRIT_QI_TOTAL budget anchor"
    );
    assert_conservation(before, after, 0.0).unwrap_or_else(|error| {
        panic!(
            "{context}: player/zone/container/ledger total must remain conserved; before={before:?}, after={after:?}, error={error:?}"
        )
    });
}

// ─────────────────── plan-race-system-v1 P0b: body_part_multipliers ───────────────────

mod body_part_multipliers_tests {
    use super::*;
    use crate::body_plan::race_registry::RaceEntry;
    use crate::body_plan::types::{
        BodyPartDef, HeightBand, HeightBandAssignment, HitGeometry, PartConsequence,
        StandingAabbSpec,
    };
    use std::collections::HashMap;

    const ALL_LEGACY_PARTS: [BodyPart; 8] = [
        BodyPart::Head,
        BodyPart::Chest,
        BodyPart::Back,
        BodyPart::Abdomen,
        BodyPart::ArmL,
        BodyPart::ArmR,
        BodyPart::LegL,
        BodyPart::LegR,
    ];

    /// `damage_mul` 与 `legacy_body_part_multipliers` 逐部位一致但数值刻意区分开
    /// （20/15/15 而非旧表 2.0/1.5/1.5），用于证明结果确实来自这份自定义 registry
    /// 而非硬编码回退表——如果测试断言命中的是旧表数值，说明 wiring 没生效。
    fn distinctive_plan() -> crate::body_plan::BodyPlan {
        crate::body_plan::BodyPlan {
            id: "distinctive_test_plan".into(),
            display_name: "测试专用构型".to_string(),
            // plan-race-system-v1 P1a：validate_body_plan 现在要求 is_humanoid==true
            // 必须提供 meridian_profile；本 fixture 只测倍率 wiring，与经脉语义
            // 无关，设 false 避免每处都补一份 profile 数据。
            is_humanoid: false,
            parts: vec![
                BodyPartDef {
                    id: "head".into(),
                    damage_mul: 20.0,
                    contam_mul: 15.0,
                    bleed_mul: 15.0,
                    consequence: PartConsequence::Sensory,
                },
                BodyPartDef {
                    id: "chest".into(),
                    damage_mul: 10.0,
                    contam_mul: 10.0,
                    bleed_mul: 10.0,
                    consequence: PartConsequence::Core,
                },
            ],
            hit_geometry: HitGeometry::HeightBands {
                aabb: StandingAabbSpec {
                    half_width: 0.3,
                    height: 1.8,
                },
                bands: vec![
                    HeightBand {
                        min_rel_y: 0.5,
                        assignment: HeightBandAssignment::Single {
                            part: "head".into(),
                        },
                    },
                    HeightBand {
                        min_rel_y: -1.0,
                        assignment: HeightBandAssignment::Single {
                            part: "chest".into(),
                        },
                    },
                ],
                lateral_threshold: 0.19,
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: HashMap::new(),
        }
    }

    fn registries_with_distinctive_human_plan() -> (BodyPlanRegistry, RaceRegistry) {
        let body_plans = BodyPlanRegistry::from_plans(vec![distinctive_plan()])
            .expect("distinctive_test_plan must validate");
        let races = RaceRegistry::from_parts_for_test(
            vec![RaceEntry {
                id: crate::body_plan::RaceId::new("human"),
                display_name: "人族".to_string(),
                body_plan_id: "distinctive_test_plan".into(),
                beast_kinds: vec![],
            }],
            vec![],
            &body_plans,
        )
        .expect("races.json fixture must validate");
        (body_plans, races)
    }

    fn uses_registry_resolved_plan_when_present_not_hardcoded_legacy_table() {
        let (body_plans, races) = registries_with_distinctive_human_plan();
        let cultivation = Cultivation::default(); // race defaults to "human"
        let (damage_mul, contam_mul, bleed_mul) = body_part_multipliers(
            Entity::PLACEHOLDER,
            Some(&cultivation),
            Some(&body_plans),
            Some(&races),
            &crate::body_plan::legacy_body_part_to_id(BodyPart::Head),
        );
        assert_eq!(
            (damage_mul, contam_mul, bleed_mul),
            (20.0, 15.0, 15.0),
            "当 BodyPlanRegistry/RaceRegistry 都存在时，必须使用其解析出的 BodyPlan 数据\
                 （20.0/15.0/15.0），而不是硬编码回退表（2.0/1.5/1.5）——命中旧表数值说明\
                 wiring 没有真正生效"
        );
    }

    fn falls_back_to_humanoid_static_when_registries_missing() {
        // 大量既有单测（本文件其余 ~48 处 resolve_attack_intents 系统测试）未插入
        // 这两个资源——退化路径必须与 legacy 硬编码表 bit-for-bit 一致，否则会让
        // 那些既有测试全部回归红。
        for part in ALL_LEGACY_PARTS {
            assert_eq!(
                body_part_multipliers(
                    Entity::PLACEHOLDER,
                    None,
                    None,
                    None,
                    &crate::body_plan::legacy_body_part_to_id(part)
                ),
                legacy_body_part_multipliers(part),
                "part={part:?}: 资源缺失时的退化路径必须与旧硬编码表完全一致"
            );
        }
    }

    fn falls_back_to_humanoid_static_when_only_body_plans_present() {
        let (body_plans, _races) = registries_with_distinctive_human_plan();
        let cultivation = Cultivation::default();
        assert_eq!(
            body_part_multipliers(
                Entity::PLACEHOLDER,
                Some(&cultivation),
                Some(&body_plans),
                None,
                &crate::body_plan::legacy_body_part_to_id(BodyPart::Head),
            ),
            legacy_body_part_multipliers(BodyPart::Head),
            "只有 body_plans 没有 races 时（二者必须同时存在才走数据驱动路径），\
                 必须退化到 humanoid_plan_static 而不是 panic 或误用 body_plans"
        );
    }

    fn falls_back_to_humanoid_static_when_only_races_present() {
        let (_body_plans, races) = registries_with_distinctive_human_plan();
        let cultivation = Cultivation::default();
        assert_eq!(
            body_part_multipliers(
                Entity::PLACEHOLDER,
                Some(&cultivation),
                None,
                Some(&races),
                &crate::body_plan::legacy_body_part_to_id(BodyPart::Head),
            ),
            legacy_body_part_multipliers(BodyPart::Head),
            "只有 races 没有 body_plans 时同样必须退化到 humanoid_plan_static"
        );
    }

    fn unknown_player_race_falls_back_to_humanoid_static_not_panic() {
        let (body_plans, races) = registries_with_distinctive_human_plan();
        let cultivation = Cultivation {
            race: crate::body_plan::RaceId::new("does_not_exist"),
            ..Default::default()
        };
        for part in ALL_LEGACY_PARTS {
            assert_eq!(
                body_part_multipliers(
                    Entity::PLACEHOLDER,
                    Some(&cultivation),
                    Some(&body_plans),
                    Some(&races),
                    &crate::body_plan::legacy_body_part_to_id(part),
                ),
                legacy_body_part_multipliers(part),
                "part={part:?}: 未知 race 解析失败必须优雅退化到 humanoid_plan_static，\
                     而不是 panic 或返回中性 1.0 倍率"
            );
        }
    }

    fn no_cultivation_component_falls_back_to_humanoid_default_via_resolve() {
        // 目标实体既非玩家（无 Cultivation）也非 BeastKind——resolve_body_plan Tier3
        // 兜底 humanoid_default()，registries_with_distinctive_human_plan() 的
        // registry 里没有注册 "humanoid" 这个 plan id，只注册了
        // "distinctive_test_plan"，所以这条路径必须走 body_plan_multipliers 自身的
        // Err/None 退化（因为 resolve_body_plan 会因 humanoid_default() panic 缺失
        // 而无法直接调用）——改用真实 humanoid registry 验证 Tier3 兜底真的取到
        // humanoid plan 数据。
        let body_plans = BodyPlanRegistry::from_plans(vec![real_humanoid_plan_copy()])
            .expect("humanoid plan must validate");
        let races = RaceRegistry::from_parts_for_test(
            vec![RaceEntry {
                id: crate::body_plan::RaceId::new("human"),
                display_name: "人族".to_string(),
                body_plan_id: "humanoid".into(),
                beast_kinds: vec![],
            }],
            vec![],
            &body_plans,
        )
        .expect("races.json fixture must validate");

        for part in ALL_LEGACY_PARTS {
            assert_eq!(
                body_part_multipliers(
                    Entity::PLACEHOLDER,
                    None, // no Cultivation component → Tier3 fallback inside resolve_body_plan
                    Some(&body_plans),
                    Some(&races),
                    &crate::body_plan::legacy_body_part_to_id(part),
                ),
                legacy_body_part_multipliers(part),
                "part={part:?}: 无 Cultivation 组件的实体经 resolve_body_plan Tier3 兜底 \
                     humanoid_default()，数值必须与旧表一致"
            );
        }
    }

    /// `humanoid_plan_static()` 的真实数据副本（同 P0a `resolve.rs` 测试模块的
    /// `humanoid_plan()` fixture 手法）——本测试文件不便直接依赖磁盘路径解析。
    fn real_humanoid_plan_copy() -> crate::body_plan::BodyPlan {
        crate::body_plan::humanoid_plan_static().clone()
    }

    #[test]
    fn body_part_multiplier_contract_matrix() {
        let cases: [(&str, fn()); 6] = [
            (
                "uses_registry_resolved_plan_when_present_not_hardcoded_legacy_table",
                uses_registry_resolved_plan_when_present_not_hardcoded_legacy_table,
            ),
            (
                "falls_back_to_humanoid_static_when_registries_missing",
                falls_back_to_humanoid_static_when_registries_missing,
            ),
            (
                "falls_back_to_humanoid_static_when_only_body_plans_present",
                falls_back_to_humanoid_static_when_only_body_plans_present,
            ),
            (
                "falls_back_to_humanoid_static_when_only_races_present",
                falls_back_to_humanoid_static_when_only_races_present,
            ),
            (
                "unknown_player_race_falls_back_to_humanoid_static_not_panic",
                unknown_player_race_falls_back_to_humanoid_static_not_panic,
            ),
            (
                "no_cultivation_component_falls_back_to_humanoid_default_via_resolve",
                no_cultivation_component_falls_back_to_humanoid_default_via_resolve,
            ),
        ];
        for (name, case) in cases {
            let result = std::panic::catch_unwind(case);
            assert!(result.is_ok(), "body_part multiplier case '{name}' failed");
        }
    }
}

/// bug: Daoxiang TSY NPCs emit AttackIntent{qi_invest:25.0, source:Melee} but
/// NpcRuntimeBundle sets Cultivation{qi_current:0.0, qi_max:10.0}. TSY zone
/// spirit_qi < -0.4 means the regen branch never fires, so qi_current stays 0.0
/// permanently. The qi gate (qi_current + EPSILON < qi_invest → 0 < 25 → true)
/// blocks every single Daoxiang attack intent, making these NPCs deal zero damage.
///
/// Fix: AttackSource::NpcMelee is added to the source_uses_prepaid_qi whitelist,
/// decoupling NPC combat from the player qi conservation model. NPC attacks are
/// server-side-authoritative and need no qi accounting.
fn npc_melee_bypasses_qi_gate_preventing_daoxiang_zero_damage() {
    // Happy path: NpcMelee must bypass the qi gate so TSY NPC attacks resolve.
    assert!(
        source_uses_prepaid_qi(AttackSource::NpcMelee),
        "NpcMelee must bypass the qi gate: NPC qi_current=0.0 by default, \
             qi_invest values (8–30) always exceed it → every attack silently dropped"
    );
    // Negative anchor: player Melee still goes through the qi gate (anti-cheat).
    assert!(
        !source_uses_prepaid_qi(AttackSource::Melee),
        "player Melee must NOT bypass the qi gate — anti-cheat must still apply"
    );
    // NpcMelee and Melee are distinct variants (different resolver paths).
    assert_ne!(
        AttackSource::NpcMelee,
        AttackSource::Melee,
        "NpcMelee and Melee must be distinct — they have different anti-cheat semantics"
    );
    // Boundary: qi_invest=25.0 (Daoxiang) and qi_max=10.0 (NpcRuntimeBundle default)
    // would be permanently blocked without the whitelist (25.0 > 10.0).
    let daoxiang_qi_invest = 25.0_f64;
    let npc_qi_current = 0.0_f64;
    let gate_would_fire = npc_qi_current + f64::EPSILON < daoxiang_qi_invest;
    assert!(
        gate_would_fire,
        "gate condition must evaluate to true for Daoxiang params \
             (qi_current={npc_qi_current}, qi_invest={daoxiang_qi_invest}) — \
             confirms NpcMelee whitelist is required, not redundant"
    );
}

/// bug-hunt-1: QiNeedle 在 cast 阶段（needle.rs:87）已预扣 QI_NEEDLE_QI_COST(=1.0) 自
/// qi_current，并 emit AttackIntent{qi_invest:1.0, source:QiNeedle}。若 QiNeedle 不在
/// prepaid 白名单，resolver 会按 resolve.rs:447 再扣一次 qi_invest → 每发命中净扣 2.0
/// 真元（double-spend）。本测试锁定 QiNeedle 与 BurstMeridian（同样 cast 阶段预扣）一致
/// 归类为 prepaid，防回归。
fn qi_needle_is_prepaid_source_preventing_double_spend() {
    assert!(
        source_uses_prepaid_qi(AttackSource::QiNeedle),
        "QiNeedle 必须是 prepaid 源：cast 阶段已预扣真元，否则 resolver 二次扣 → double-spend"
    );
    assert_eq!(
        source_uses_prepaid_qi(AttackSource::QiNeedle),
        source_uses_prepaid_qi(AttackSource::BurstMeridian),
        "QiNeedle 与 BurstMeridian 同为 cast 阶段预扣，prepaid 分类必须一致"
    );
    // 负锚点：普通近战不预扣，仍走 resolver 扣费，确保白名单非恒真
    assert!(
        !source_uses_prepaid_qi(AttackSource::Melee),
        "Melee 非 cast 预扣源，不应被误判为 prepaid（否则白名单恒真无意义）"
    );
}

#[test]
fn prepaid_attack_source_contract_matrix() {
    let cases: [(&str, fn()); 2] = [
        (
            "npc_melee_bypasses_qi_gate_preventing_daoxiang_zero_damage",
            npc_melee_bypasses_qi_gate_preventing_daoxiang_zero_damage,
        ),
        (
            "qi_needle_is_prepaid_source_preventing_double_spend",
            qi_needle_is_prepaid_source_preventing_double_spend,
        ),
    ];
    for (name, case) in cases {
        let result = std::panic::catch_unwind(case);
        assert!(result.is_ok(), "prepaid attack source case '{name}' failed");
    }
}

use crate::combat::anticheat::AntiCheatCounter;
use crate::combat::armor::{ArmorProfile, ArmorProfileRegistry};
use crate::combat::components::{
    ActiveStatusEffect, BodyPart, CombatState, DefenseWindow, DerivedAttrs, Lifecycle,
    StatusEffects, WoundKind, Wounds,
};
use crate::combat::events::{
    ApplyStatusEffectIntent, AttackIntent, AttackReach, AttackSource, DefenseKind,
    StatusEffectKind, FIST_REACH, SPEAR_REACH,
};
use crate::combat::jiemai::jiemai_contam_multiplier_for_effectiveness;
use crate::cultivation::components::{
    Contamination, CrackCause, Cultivation, MeridianId, MeridianSystem, Realm,
};
use crate::cultivation::known_techniques::KnownTechnique;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::inventory::{
    ContainerState, InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemRegistry,
    ItemTemplate, PlayerInventory, WeaponSpec, EQUIP_SLOT_OFF_HAND,
};
use crate::npc::brain::canonical_npc_id;
use crate::npc::spawn::NpcMeleeProfile;
use crate::npc::spawn::{spawn_test_npc_runtime_shape, NpcMarker};
use crate::player::state::PlayerState;
use crate::social::components::SparringState;
use valence::prelude::{
    bevy_ecs, App, Entity, Events, GameMode, IntoSystemConfigs, Position, Resource, Update,
};
use valence::testing::create_mock_client;

#[derive(Clone, Copy, Resource)]
struct TestLayer(Entity);

fn setup_test_layer(mut commands: valence::prelude::Commands) {
    let layer = commands.spawn_empty().id();
    commands.insert_resource(TestLayer(layer));
}

fn spawn_runtime_npc(
    mut commands: valence::prelude::Commands,
    layer: valence::prelude::Res<TestLayer>,
) {
    spawn_test_npc_runtime_shape(&mut commands, layer.0);
}

fn spawn_player(
    app: &mut App,
    username: &str,
    position: [f64; 3],
    wounds: Wounds,
    stamina: Stamina,
) -> Entity {
    let (mut client_bundle, _helper) = create_mock_client(username);
    client_bundle.player.position = Position::new(position);
    let entity = app
        .world_mut()
        .spawn((
            client_bundle,
            Cultivation {
                realm: crate::cultivation::components::Realm::Induce,
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
            wounds,
            stamina,
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

fn weapon_test_registry() -> ItemRegistry {
    ItemRegistry::from_map(std::collections::HashMap::from([
        (
            "iron_sword".to_string(),
            ItemTemplate {
                id: "iron_sword".to_string(),
                display_name: "铁剑".to_string(),
                category: ItemCategory::Weapon,
                placeable: None,
                max_stack_count: 1,
                grid_w: 1,
                grid_h: 2,
                base_weight: 1.2,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 1.0,
                description: String::new(),
                effect: None,
                cast_duration_ms: 0,
                cooldown_ms: 0,
                weapon_spec: Some(WeaponSpec {
                    weapon_kind: crate::combat::weapon::WeaponKind::Sword,
                    base_attack: 12.0,
                    quality_tier: 0,
                    durability_max: 200.0,
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
            },
        ),
        (
            "strong_sword".to_string(),
            ItemTemplate {
                id: "strong_sword".to_string(),
                display_name: "强剑".to_string(),
                category: ItemCategory::Weapon,
                placeable: None,
                max_stack_count: 1,
                grid_w: 1,
                grid_h: 2,
                base_weight: 1.0,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 1.0,
                description: String::new(),
                effect: None,
                cast_duration_ms: 0,
                cooldown_ms: 0,
                weapon_spec: Some(WeaponSpec {
                    weapon_kind: crate::combat::weapon::WeaponKind::Sword,
                    base_attack: 20.0,
                    quality_tier: 0,
                    durability_max: 200.0,
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
            },
        ),
        (
            "glass_sword".to_string(),
            ItemTemplate {
                id: "glass_sword".to_string(),
                display_name: "玻璃剑".to_string(),
                category: ItemCategory::Weapon,
                placeable: None,
                max_stack_count: 1,
                grid_w: 1,
                grid_h: 2,
                base_weight: 1.0,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 1.0,
                description: String::new(),
                effect: None,
                cast_duration_ms: 0,
                cooldown_ms: 0,
                weapon_spec: Some(WeaponSpec {
                    weapon_kind: crate::combat::weapon::WeaponKind::Sword,
                    base_attack: 10.0,
                    quality_tier: 0,
                    durability_max: 10.0,
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
            },
        ),
    ]))
}

#[test]
fn armor_hit_scales_contamination_and_ticks_item_durability() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1500 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<SkillXpGain>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();

    app.insert_resource(crate::inventory::ItemRegistry::default());
    app.insert_resource(ArmorProfileRegistry::from_map(
        std::collections::HashMap::from([(
            "fake_spirit_hide".to_string(),
            ArmorProfile {
                slot: EquipSlotV1::Chest,
                body_coverage: vec![BodyPart::Chest],
                kind_mitigation: std::collections::HashMap::from([(WoundKind::Blunt, 0.5)]),
                durability_max: 100,
                broken_multiplier: 0.3,
            },
        )]),
    ));

    app.add_systems(
        Update,
        (
            crate::combat::status::attribute_aggregate_tick,
            crate::combat::weapon::sync_weapon_component_from_equipped,
            crate::combat::armor_sync::sync_armor_to_derived_attrs,
            resolve_attack_intents,
        ),
    );

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "Crimson",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    // 给 target 装一件胸甲，初始耐久比例 1.0。
    app.world_mut().entity_mut(target).insert(PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers: vec![ContainerState {
            quick_access: false,
            id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 5,
            cols: 7,
            items: vec![],
            owner_instance_id: None,
        }],
        equipped: std::collections::HashMap::from([(
            crate::inventory::EQUIP_SLOT_CHEST.to_string(),
            crate::inventory::SlotContents::worn_single(ItemInstance {
                instance_id: 88,
                template_id: "fake_spirit_hide".to_string(),
                display_name: "假灵兽皮胸甲".to_string(),
                grid_w: 2,
                grid_h: 2,
                weight: 5.0,
                rarity: crate::inventory::ItemRarity::Common,
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
    });

    app.update();

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 1499,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let combat_events = app.world().resource::<Events<CombatEvent>>();
    let event = combat_events
        .iter_current_update_events()
        .next()
        .expect("combat event should emit");
    // event.damage 是 mitigation 之后的 wound_severity（已乘 1-m）。
    // emitted_contam_delta = init_damage * 0.25 * 1 * 0.8 * (1-m) * MULTIPLIER
    //                       = event.damage * 0.25 * 1 * 0.8 * MULTIPLIER。
    let expected_contam =
        f64::from(event.damage) * 0.25 * 1.0 * 0.8 * ARMOR_HIT_CONTAMINATION_MULTIPLIER;
    assert_eq!(event.contam_delta, expected_contam);

    let inventory = app.world().entity(target).get::<PlayerInventory>().unwrap();
    assert!(
        inventory.equipped[crate::inventory::EQUIP_SLOT_CHEST].worn[0].durability < 1.0,
        "armor hit should tick down durability"
    );
}

#[test]
fn armor_break_emits_durability_event_and_radius_audio() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1501 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<SkillXpGain>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_event::<PlaySoundRecipeRequest>();

    app.insert_resource(crate::inventory::ItemRegistry::default());
    app.insert_resource(ArmorProfileRegistry::from_map(
        std::collections::HashMap::from([(
            "fake_spirit_hide".to_string(),
            ArmorProfile {
                slot: EquipSlotV1::Chest,
                body_coverage: vec![BodyPart::Chest],
                kind_mitigation: std::collections::HashMap::from([(WoundKind::Blunt, 0.5)]),
                durability_max: 1,
                broken_multiplier: 0.3,
            },
        )]),
    ));

    app.add_systems(
        Update,
        (
            crate::combat::status::attribute_aggregate_tick,
            crate::combat::weapon::sync_weapon_component_from_equipped,
            crate::combat::armor_sync::sync_armor_to_derived_attrs,
            resolve_attack_intents,
        ),
    );

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "Crimson",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut().entity_mut(target).insert(PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers: vec![ContainerState {
            quick_access: false,
            id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 5,
            cols: 7,
            items: vec![],
            owner_instance_id: None,
        }],
        equipped: std::collections::HashMap::from([(
            crate::inventory::EQUIP_SLOT_CHEST.to_string(),
            crate::inventory::SlotContents::worn_single(ItemInstance {
                instance_id: 89,
                template_id: "fake_spirit_hide".to_string(),
                display_name: "假灵兽皮胸甲".to_string(),
                grid_w: 2,
                grid_h: 2,
                weight: 5.0,
                rarity: crate::inventory::ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 1.0,
                durability: 0.25,
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
    });

    app.update();

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 1500,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let durability_events = app
        .world()
        .resource::<Events<InventoryDurabilityChangedEvent>>();
    let events: Vec<_> = durability_events.iter_current_update_events().collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].entity, target);
    assert_eq!(events[0].instance_id, 89);
    assert_eq!(events[0].durability, 0.0);

    let audio_events = app.world().resource::<Events<PlaySoundRecipeRequest>>();
    let events: Vec<_> = audio_events.iter_current_update_events().collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].recipe_id, "armor_break");
    match &events[0].recipient {
        AudioRecipient::Radius { origin, radius } => {
            assert_eq!(*origin, valence::prelude::DVec3::new(1.0, 64.0, 0.0));
            assert_eq!(*radius, AUDIO_BROADCAST_RADIUS);
        }
        other => panic!("armor_break should use radius recipient, got {other:?}"),
    }
}

// ══════════════════════════════════════════════════════════════════════════
// plan-race-system-v1 P4 opus verifier MAJOR — armor coverage 折算真实伤害差
// 集成测试：`legacy_part_for_wound_with_morph`/`apply_armor_mitigation` 此前
// 只有纯函数逻辑却零测试断言真实伤害数值差。走真实 ECS 全链路
// （`attribute_aggregate_tick` → `sync_armor_to_derived_attrs` →
// `resolve_attack_intents`），合成 whale intrinsic（本体部位 `tail_fin`，无
// legacy 对应物）+ human form `MorphState`（`RaceRegistry.morph_pairs` 声明
// `chest`(human/to 部位) → `tail_fin`(whale/from 部位) 映射），穿人形胸甲：
// - 有 `MorphState` 时命中经 part_mapping 逆查折算回 `chest` legacy 部位，
//   armor 减免真实生效（伤势/contam 被压低）；
// - 对照组同一件甲、同一本体部位，缺 `MorphState` 时 `tail_fin` 没有 legacy
//   对应物，`apply_armor_mitigation` 提前 `None`，伤害不被减免（severity 更高）。
mod morph_armor_coverage_integration_tests {
    use super::*;
    use crate::body_plan::race_registry::{MorphPairDef, RaceEntry};
    use crate::body_plan::types::{BodyPartDef, HitGeometry, PartBox, PartConsequence};
    use crate::body_plan::{BodyPartId, BodyPlanId, MorphState, RaceId};
    use crate::combat::armor::{ArmorProfile, ArmorProfileRegistry};
    use std::collections::HashMap;

    /// whale 本体构型——唯一部位 `tail_fin`，几何与
    /// `non_humanoid_consequence_integration_tests::single_part_plan` 同款已核验
    /// 命中盒（攻方 feet=[-2,64,0]、目标 feet=[0,64,0]、FIST_REACH 必命中）。
    fn whale_intrinsic_plan() -> crate::body_plan::BodyPlan {
        crate::body_plan::BodyPlan {
            id: BodyPlanId::new("test_whale_intrinsic_plan"),
            display_name: "测试飞鲸本体构型".to_string(),
            is_humanoid: false,
            parts: vec![BodyPartDef {
                id: BodyPartId::new("tail_fin"),
                damage_mul: 1.0,
                contam_mul: 1.0,
                bleed_mul: 1.0,
                consequence: PartConsequence::Core,
            }],
            hit_geometry: HitGeometry::PartBoxes {
                boxes: vec![PartBox {
                    part_id: BodyPartId::new("tail_fin"),
                    offset: [-1.0, 1.2, 0.0],
                    half_extents: [0.45, 0.45, 0.45],
                    priority: 0,
                }],
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: HashMap::new(),
        }
    }

    /// human form 构型——只需存在 `chest` 部位供 `part_mapping` 校验命中，几何
    /// 本身不参与本测试（`BodyPlanPurpose::Intrinsic` 恒读 whale 本体几何，
    /// `resolve_body_plan_for_target` 在本测试路径不会解析 form 构型）。
    fn human_form_plan() -> crate::body_plan::BodyPlan {
        crate::body_plan::BodyPlan {
            id: BodyPlanId::new("test_human_form_plan"),
            display_name: "测试人形形态构型".to_string(),
            is_humanoid: false,
            parts: vec![BodyPartDef {
                id: BodyPartId::new("chest"),
                damage_mul: 1.0,
                contam_mul: 1.0,
                bleed_mul: 1.0,
                consequence: PartConsequence::Core,
            }],
            hit_geometry: HitGeometry::PartBoxes {
                boxes: vec![PartBox {
                    part_id: BodyPartId::new("chest"),
                    offset: [-1.0, 1.2, 0.0],
                    half_extents: [0.45, 0.45, 0.45],
                    priority: 0,
                }],
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: HashMap::new(),
        }
    }

    /// whale↔human morph_pair 注册：`part_mapping` 方向 = form_part(to=human
    /// 的 `chest`) → intrinsic_part(from=whale 的 `tail_fin`)。
    fn whale_human_registries() -> (BodyPlanRegistry, RaceRegistry) {
        let body_plans =
            BodyPlanRegistry::from_plans(vec![whale_intrinsic_plan(), human_form_plan()])
                .expect("whale+human test plans must validate");
        let mut part_mapping = HashMap::new();
        part_mapping.insert(BodyPartId::new("chest"), BodyPartId::new("tail_fin"));
        let races = RaceRegistry::from_parts_for_test(
            vec![
                RaceEntry {
                    id: RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                    display_name: "人族".to_string(),
                    body_plan_id: BodyPlanId::new("test_human_form_plan"),
                    beast_kinds: vec![],
                },
                RaceEntry {
                    id: RaceId::new("whale"),
                    display_name: "飞鲸".to_string(),
                    body_plan_id: BodyPlanId::new("test_whale_intrinsic_plan"),
                    beast_kinds: vec![],
                },
            ],
            vec![MorphPairDef {
                from: RaceId::new("whale"),
                to: RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                part_mapping,
            }],
            &body_plans,
        )
        .expect("whale<->human morph pair fixture must validate");
        (body_plans, races)
    }

    /// 组装最小 App：真实 `attribute_aggregate_tick` → `sync_armor_to_derived_attrs`
    /// → `resolve_attack_intents` 链路 + whale/human registries + 人形胸甲
    /// `ArmorProfileRegistry`。`with_morph=true` 时给 target 挂 `MorphState{form:
    /// human}`；`false` 时不挂（对照组，`tail_fin` 无 legacy 对应物，armor 减免
    /// 提前 `None`）。
    fn setup_morph_armor_app(with_morph: bool) -> (App, Entity, Entity) {
        let (body_plans, races) = whale_human_registries();
        let mut app = qi_test_app();
        app.insert_resource(CombatClock { tick: 2000 });
        app.insert_resource(body_plans);
        app.insert_resource(races);
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<crate::combat::weapon::ShieldBlockHit>();
        app.add_event::<InventoryDurabilityChangedEvent>();

        app.insert_resource(crate::inventory::ItemRegistry::default());
        app.insert_resource(ArmorProfileRegistry::from_map(
            std::collections::HashMap::from([(
                "test_whale_form_chestplate".to_string(),
                ArmorProfile {
                    slot: EquipSlotV1::Chest,
                    body_coverage: vec![BodyPart::Chest],
                    kind_mitigation: std::collections::HashMap::from([(WoundKind::Blunt, 0.5)]),
                    durability_max: 100,
                    broken_multiplier: 0.3,
                },
            )]),
        ));

        app.add_systems(
            Update,
            (
                crate::combat::status::attribute_aggregate_tick,
                crate::combat::weapon::sync_weapon_component_from_equipped,
                crate::combat::armor_sync::sync_armor_to_derived_attrs,
                resolve_attack_intents,
            ),
        );

        let attacker = spawn_player(
            &mut app,
            "MorphArmorAttacker",
            [-2.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "MorphArmorTarget",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        // target 本体种族 = whale（intrinsic），覆盖 spawn_player 默认的 "human"。
        app.world_mut().entity_mut(target).insert(Cultivation {
            realm: crate::cultivation::components::Realm::Induce,
            qi_current: 60.0,
            qi_max: 100.0,
            race: RaceId::new("whale"),
            ..Cultivation::default()
        });
        if with_morph {
            app.world_mut().entity_mut(target).insert(MorphState::new(
                RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                0,
                0,
            ));
        }
        app.world_mut().entity_mut(target).insert(PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                quick_access: false,
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![],
                owner_instance_id: None,
            }],
            equipped: std::collections::HashMap::from([(
                crate::inventory::EQUIP_SLOT_CHEST.to_string(),
                crate::inventory::SlotContents::worn_single(ItemInstance {
                    instance_id: 9001,
                    template_id: "test_whale_form_chestplate".to_string(),
                    display_name: "测试人形形态胸甲".to_string(),
                    grid_w: 2,
                    grid_h: 2,
                    weight: 5.0,
                    rarity: crate::inventory::ItemRarity::Common,
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
        });

        app.update();
        (app, attacker, target)
    }

    fn send_morph_armor_attack(app: &mut App, attacker: Entity, target: Entity) {
        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 1999,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();
    }

    #[test]
    fn morphed_target_gets_real_armor_mitigation_via_part_mapping_fold_back() {
        let (mut app, attacker, target) = setup_morph_armor_app(true);
        send_morph_armor_attack(&mut app, attacker, target);

        let wounds = app.world().entity(target).get::<Wounds>().unwrap();
        assert_eq!(wounds.entries.len(), 1, "应恰好写入一条本体 tail_fin 伤口");
        assert_eq!(
            wounds.entries[0].location,
            BodyPartId::new("tail_fin"),
            "命中几何解析恒用 BodyPlanPurpose::Intrinsic（whale 本体），\
                 MorphState 不改变命中落点，只影响护甲折算"
        );

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let event = combat_events
            .iter_current_update_events()
            .next()
            .expect("combat event should emit");
        assert!(
            event.contam_delta < f64::from(event.damage) * 0.25,
            "有 MorphState 时 tail_fin 命中经 part_mapping 折算回 chest legacy \
                 部位，应命中 defense_profile 条目，护甲把 contam 压到 \
                 ARMOR_HIT_CONTAMINATION_MULTIPLIER(0.1) 量级（远低于无甲基线 \
                 0.25），实测 contam_delta={} event.damage={}",
            event.contam_delta,
            event.damage
        );

        let inventory = app.world().entity(target).get::<PlayerInventory>().unwrap();
        assert!(
            inventory.equipped[crate::inventory::EQUIP_SLOT_CHEST].worn[0].durability < 1.0,
            "护甲真实生效时应扣减耐久——耐久掉落是护甲折算命中的外部可观察副作用"
        );
    }

    #[test]
    fn unmorphed_target_same_armor_same_intrinsic_part_gets_no_mitigation() {
        let (mut app, attacker, target) = setup_morph_armor_app(false);
        send_morph_armor_attack(&mut app, attacker, target);

        let wounds = app.world().entity(target).get::<Wounds>().unwrap();
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(wounds.entries[0].location, BodyPartId::new("tail_fin"));

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let event = combat_events
            .iter_current_update_events()
            .next()
            .expect("combat event should emit");
        // 无 MorphState 时 legacy_part_for_wound_with_morph 直接
        // id_to_legacy_body_part("tail_fin") = None（非人形 8 段字符串），
        // apply_armor_mitigation 提前 `?` 返回 None——armor 完全不生效，
        // contam_delta 应落在无甲基线 damage * 0.25 * 1.0 * 0.8（截脉系数）。
        let expected_contam_no_mitigation = f64::from(event.damage) * 0.25 * 1.0 * 0.8;
        assert!(
            (event.contam_delta - expected_contam_no_mitigation).abs() < 1e-9,
            "同一件甲、同一本体部位（tail_fin），缺 MorphState 时应完全不生效\
                 （contam_delta 落在无甲基线），实测 contam_delta={} 期望={}",
            event.contam_delta,
            expected_contam_no_mitigation
        );

        let inventory = app.world().entity(target).get::<PlayerInventory>().unwrap();
        assert_eq!(
            inventory.equipped[crate::inventory::EQUIP_SLOT_CHEST].worn[0].durability,
            1.0,
            "armor 未生效（未命中折算）不应扣减耐久"
        );
    }

    #[test]
    fn morphed_target_takes_less_severity_than_unmorphed_control() {
        // 同一 qi_invest/wound_kind/攻防几何，唯一变量是 MorphState 在/不在——
        // 直接断言两组的 wound.severity 数值差，锁住"armor 真的减免了伤害"这条
        // 最终外部可观察后果（不只是 contam 侧信号）。
        let (mut app_with_morph, attacker_a, target_a) = setup_morph_armor_app(true);
        send_morph_armor_attack(&mut app_with_morph, attacker_a, target_a);
        let severity_with_morph = app_with_morph
            .world()
            .entity(target_a)
            .get::<Wounds>()
            .unwrap()
            .entries[0]
            .severity;

        let (mut app_without_morph, attacker_b, target_b) = setup_morph_armor_app(false);
        send_morph_armor_attack(&mut app_without_morph, attacker_b, target_b);
        let severity_without_morph = app_without_morph
            .world()
            .entity(target_b)
            .get::<Wounds>()
            .unwrap()
            .entries[0]
            .severity;

        assert!(
            severity_with_morph < severity_without_morph,
            "有 MorphState（armor 折算生效）的伤势应严格低于无 MorphState 的对照组\
                （armor 折算不生效），实测 with_morph={severity_with_morph} \
                 without_morph={severity_without_morph}"
        );
    }
}

fn spawn_npc(app: &mut App, position: [f64; 3], wounds: Wounds, stamina: Stamina) -> Entity {
    let entity = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new(position),
            Cultivation {
                qi_current: 60.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
            MeridianSystem::default(),
            LifeRecord::default(),
            Contamination::default(),
            StatusEffects::default(),
            wounds,
            stamina,
            CombatState::default(),
            DerivedAttrs::default(),
        ))
        .id();
    let canonical = canonical_npc_id(entity);
    app.world_mut().entity_mut(entity).insert((
        Lifecycle {
            character_id: canonical.clone(),
            ..Default::default()
        },
        LifeRecord::new(canonical),
    ));
    entity
}

/// 独立复算 NPC 攻方本次 `AttackIntent` 应该命中的真实部位——用于把"恒 Chest"断言
/// 替换为"由 raycast::npc_aim_direction 的确定性 jitter 决定"的断言（决议 §8.1 #1/#3）。
/// 直接调用与 `resolve_attack_intents` 相同的 `raycast` 公开函数（同一 seed 来源：
/// `attacker_id` + `intent.issued_at_tick`），验证的是"确实接了真实瞄准链路"而非重新
/// 拍脑袋断言一个固定枚举值。
fn expected_npc_hit_body_part(
    attacker_feet: [f64; 3],
    attacker_canonical_id: &str,
    target_feet: [f64; 3],
    issued_at_tick: u64,
    reach: AttackReach,
) -> crate::body_plan::BodyPartId {
    let origin = DVec3::new(
        attacker_feet[0],
        attacker_feet[1] + ATTACKER_EYE_HEIGHT,
        attacker_feet[2],
    );
    let target = DVec3::new(target_feet[0], target_feet[1], target_feet[2]);
    let seed = raycast::npc_aim_seed(attacker_canonical_id, issued_at_tick);
    let sigma_scale = raycast::weapon_aim_jitter_scale(reach);
    let aim_direction = raycast::npc_aim_direction(origin, target, seed, sigma_scale);
    // `resolve_attack_intents` 的目标是 humanoid（本文件测试全用人形 fixture），
    // 显式传入 `humanoid_plan_static()` 与生产资源齐全时的解析结果 bit-for-bit 一致。
    // yaw=0.0：HeightBands 分支忽略该参数，与生产调用点行为一致。
    raycast_humanoid(
        humanoid_plan_static(),
        origin,
        target,
        0.0,
        f64::from(reach.max),
        aim_direction,
    )
    .expect("expected npc aim direction to stay within reach and hit target AABB")
    .part_id
}

#[test]
fn hit_emits_direction_vfx() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 44 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "Crimson",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 44,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
    let emitted = vfx_events
        .iter_current_update_events()
        .find(|event| {
            matches!(
                &event.payload,
                crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. }
                    if event_id == gameplay_vfx::COMBAT_HIT
            )
        })
        .expect("resolved hit should emit combat_hit vfx");
    match &emitted.payload {
        crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle {
            event_id,
            direction,
            ..
        } => {
            assert_eq!(event_id, gameplay_vfx::COMBAT_HIT);
            assert!(direction.is_some(), "combat_hit should carry hit direction");
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

/// 在固定攻防几何下，扫描确定性 NPC jitter tick，找到第一个命中 `wanted` 部位的
/// `issued_at_tick`——不硬编魔数 tick，而是用生产同款 `expected_npc_hit_body_part`
/// 复算，保证测试与实现共用同一套确定性瞄准公式（plan-combat-hit-location-v1 P3）。
fn find_npc_tick_hitting(
    attacker_feet: [f64; 3],
    attacker_canonical_id: &str,
    target_feet: [f64; 3],
    reach: AttackReach,
    wanted: BodyPart,
) -> u64 {
    let wanted_id = crate::body_plan::legacy_body_part_to_id(wanted);
    (0..2000u64)
        .find(|&tick| {
            expected_npc_hit_body_part(
                attacker_feet,
                attacker_canonical_id,
                target_feet,
                tick,
                reach,
            ) == wanted_id
        })
        .unwrap_or_else(|| {
            panic!("未能在 0..2000 tick 内为部位 {wanted:?} 找到确定性 jitter 命中样本")
        })
}

#[test]
fn head_hit_emits_head_crit_vfx_not_generic_combat_hit() {
    let mut app = qi_test_app();
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker_feet = [0.0, 64.0, 0.0];
    let target_feet = [1.0, 64.0, 0.0];
    let npc_attacker = spawn_npc(
        &mut app,
        attacker_feet,
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "HeadTarget",
        target_feet,
        Wounds::default(),
        Stamina::default(),
    );
    let canonical = canonical_npc_id(npc_attacker);
    let tick = find_npc_tick_hitting(
        attacker_feet,
        &canonical,
        target_feet,
        FIST_REACH,
        BodyPart::Head,
    );
    app.insert_resource(CombatClock { tick });

    app.world_mut().send_event(AttackIntent {
        attacker: npc_attacker,
        target: Some(target),
        issued_at_tick: tick,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::NpcMelee,
        debug_command: None,
    });
    app.update();

    let target_ref = app.world().entity(target);
    let wounds = target_ref
        .get::<Wounds>()
        .expect("target should keep wounds");
    assert_eq!(
        wounds.entries[0].location,
        crate::body_plan::legacy_body_part_to_id(BodyPart::Head),
        "找到的 tick 应确实产出 Head 命中（否则测试自身校准漂移）"
    );

    let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
    let payloads: Vec<_> = vfx_events.iter_current_update_events().collect();
    let head_event = payloads
        .iter()
        .find(|event| {
            matches!(
                &event.payload,
                crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. }
                    if event_id == gameplay_vfx::COMBAT_HIT_HEAD_CRIT
            )
        })
        .expect("头部命中应 emit bong:combat_hit_head_crit（暴击星形 burst），而非通用 combat_hit");
    match &head_event.payload {
        crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle {
            color,
            count,
            duration_ticks,
            ..
        } => {
            assert_eq!(
                color.as_deref(),
                Some("#FFE9A0"),
                "头部暴击 burst 应为白金色 #FFE9A0"
            );
            assert_eq!(*count, Some(6), "头部暴击 burst 应为 ×6");
            assert_eq!(*duration_ticks, Some(8), "头部暴击 burst lifetime 应为 8t");
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
    assert!(
        !payloads.iter().any(|event| matches!(
            &event.payload,
            crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. }
                if event_id == gameplay_vfx::COMBAT_HIT
        )),
        "头部命中不应再退化 emit 通用 bong:combat_hit（应二选一，不叠加旧事件）"
    );
}

#[test]
fn limb_hit_emits_limb_vfx_distinct_from_head_and_torso() {
    let mut app = qi_test_app();
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker_feet = [0.0, 64.0, 0.0];
    let target_feet = [1.0, 64.0, 0.0];
    let npc_attacker = spawn_npc(
        &mut app,
        attacker_feet,
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "LimbTarget",
        target_feet,
        Wounds::default(),
        Stamina::default(),
    );
    let canonical = canonical_npc_id(npc_attacker);
    // ArmL/ArmR/LegL/LegR 均应路由到同一个 COMBAT_HIT_LIMB——扫第一个命中任意四肢的 tick。
    let tick = (0..2000u64)
        .find(|&tick| {
            let hit_part_id = expected_npc_hit_body_part(
                attacker_feet,
                &canonical,
                target_feet,
                tick,
                FIST_REACH,
            );
            matches!(
                crate::body_plan::id_to_legacy_body_part(&hit_part_id),
                Some(BodyPart::ArmL | BodyPart::ArmR | BodyPart::LegL | BodyPart::LegR)
            )
        })
        .expect("未能在 0..2000 tick 内找到任意四肢命中样本");
    app.insert_resource(CombatClock { tick });

    app.world_mut().send_event(AttackIntent {
        attacker: npc_attacker,
        target: Some(target),
        issued_at_tick: tick,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::NpcMelee,
        debug_command: None,
    });
    app.update();

    let target_ref = app.world().entity(target);
    let wounds = target_ref
        .get::<Wounds>()
        .expect("target should keep wounds");
    assert!(
        matches!(
            crate::body_plan::id_to_legacy_body_part(&wounds.entries[0].location),
            Some(BodyPart::ArmL | BodyPart::ArmR | BodyPart::LegL | BodyPart::LegR)
        ),
        "找到的 tick 应确实产出四肢命中，实际 {:?}",
        wounds.entries[0].location
    );

    let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
    let payloads: Vec<_> = vfx_events.iter_current_update_events().collect();
    let limb_event = payloads
        .iter()
        .find(|event| {
            matches!(
                &event.payload,
                crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. }
                    if event_id == gameplay_vfx::COMBAT_HIT_LIMB
            )
        })
        .expect("四肢命中应 emit bong:combat_hit_limb（血色三线），而非通用 combat_hit");
    match &limb_event.payload {
        crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle {
            color,
            count,
            duration_ticks,
            direction,
            ..
        } => {
            assert_eq!(
                color.as_deref(),
                Some("#8C1F1F"),
                "四肢命中血色应为 #8C1F1F"
            );
            assert_eq!(*count, Some(3), "四肢命中应为 ×3（沿命中法线三线）");
            assert_eq!(*duration_ticks, Some(6), "四肢命中 lifetime 应为 6t");
            assert!(direction.is_some(), "四肢命中应携带命中法线方向");
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
    assert!(
        !payloads.iter().any(|event| matches!(
            &event.payload,
            crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. }
                if event_id == gameplay_vfx::COMBAT_HIT_HEAD_CRIT
                    || event_id == gameplay_vfx::COMBAT_HIT
        )),
        "四肢命中不应叠加 emit 头部或通用 combat_hit 事件"
    );
}

#[test]
fn leg_wound_slowdown_emits_ground_blood_decal() {
    let mut app = qi_test_app();
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker_feet = [0.0, 64.0, 0.0];
    let target_feet = [1.0, 64.0, 0.0];
    let npc_attacker = spawn_npc(
        &mut app,
        attacker_feet,
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "LegTarget",
        target_feet,
        Wounds::default(),
        Stamina::default(),
    );
    let canonical = canonical_npc_id(npc_attacker);
    // 用 SPEAR_REACH（伤害衰减系数更宽松）+ 更高 qi_invest，方便稳定越过
    // LEG_SLOWED_SEVERITY_THRESHOLD 触发减速与血渍 decal。
    let tick = (0..2000u64)
        .find(|&tick| {
            let hit_part_id = expected_npc_hit_body_part(
                attacker_feet,
                &canonical,
                target_feet,
                tick,
                SPEAR_REACH,
            );
            matches!(
                crate::body_plan::id_to_legacy_body_part(&hit_part_id),
                Some(BodyPart::LegL | BodyPart::LegR)
            )
        })
        .expect("未能在 0..2000 tick 内找到腿部命中样本");
    app.insert_resource(CombatClock { tick });

    app.world_mut().send_event(AttackIntent {
        attacker: npc_attacker,
        target: Some(target),
        issued_at_tick: tick,
        reach: SPEAR_REACH,
        qi_invest: 60.0,
        wound_kind: WoundKind::Pierce,
        source: AttackSource::NpcMelee,
        debug_command: None,
    });
    app.update();

    let target_ref = app.world().entity(target);
    let wounds = target_ref
        .get::<Wounds>()
        .expect("target should keep wounds");
    let wound_severity = wounds.entries[0].severity;
    assert!(
        matches!(
            crate::body_plan::id_to_legacy_body_part(&wounds.entries[0].location),
            Some(BodyPart::LegL | BodyPart::LegR)
        ),
        "找到的 tick 应确实产出腿部命中，实际 {:?}",
        wounds.entries[0].location
    );
    assert!(
        wound_severity >= LEG_SLOWED_SEVERITY_THRESHOLD,
        "本测试要求腿伤严重度 {wound_severity} 越过减速阈值 {LEG_SLOWED_SEVERITY_THRESHOLD}\
             才能触发血渍 decal，否则测试自身前置条件不成立"
    );

    let status_events = app.world().resource::<Events<ApplyStatusEffectIntent>>();
    assert!(
        status_events
            .iter_current_update_events()
            .any(|event| event.kind == StatusEffectKind::Slowed),
        "腿伤越过阈值应确实触发 Slowed 减速（本测试的前置条件）"
    );

    let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
    let decal_event = vfx_events
        .iter_current_update_events()
        .find(|event| {
            matches!(
                &event.payload,
                crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. }
                    if event_id == gameplay_vfx::COMBAT_LEG_WOUND_DECAL
            )
        })
        .expect("腿伤减速触发时应 emit bong:combat_leg_wound_decal 血渍 decal");
    match &decal_event.payload {
        crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle {
            duration_ticks,
            direction,
            ..
        } => {
            assert_eq!(
                *duration_ticks,
                Some(100),
                "血渍 decal lifetime 应为 100t（区别于命中粒子的 6-12t）"
            );
            assert!(
                direction.is_none(),
                "地面 decal 不携带命中方向（水平贴地，无需法线）"
            );
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

#[test]
fn sparring_lethal_hit_ends_without_death_event() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 44 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(
        Update,
        (
            resolve_attack_intents,
            crate::combat::status::status_effect_apply_tick,
        ),
    );
    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "Crimson",
        [1.0, 64.0, 0.0],
        Wounds {
            health_current: 8.0,
            health_max: 100.0,
            entries: Vec::new(),
        },
        Stamina::default(),
    );
    app.world_mut().entity_mut(attacker).insert(SparringState {
        partner: target,
        invite_id: "sparring:1:a:b".to_string(),
        started_at_tick: 40,
        expires_at_tick: 6000,
    });
    app.world_mut().entity_mut(target).insert(SparringState {
        partner: attacker,
        invite_id: "sparring:1:a:b".to_string(),
        started_at_tick: 40,
        expires_at_tick: 6000,
    });

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 44,
        reach: FIST_REACH,
        qi_invest: 40.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();
    app.update();

    assert!(app.world().get::<SparringState>(attacker).is_none());
    assert!(app.world().get::<SparringState>(target).is_none());
    assert!(app.world().resource::<Events<DeathEvent>>().is_empty());
    let wounds = app.world().get::<Wounds>(target).unwrap();
    assert!(wounds.health_current > 0.0);
    let statuses = app.world().get::<StatusEffects>(target).unwrap();
    assert!(statuses
        .active
        .iter()
        .any(|effect| effect.kind == StatusEffectKind::Humility));
}

#[test]
fn resolve_debug_attack_applies_damage_contamination_throughput_and_death() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 12 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(
        Update,
        (
            resolve_attack_intents,
            crate::combat::status::status_effect_apply_tick,
        ),
    );

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let mut target_meridians = MeridianSystem::default();
    target_meridians.get_mut(MeridianId::Lung).opened = true;
    let target = spawn_player(
        &mut app,
        "Crimson",
        [1.0, 64.0, 0.0],
        Wounds {
            health_current: 8.0,
            health_max: 100.0,
            entries: Vec::new(),
        },
        Stamina::default(),
    );
    app.world_mut().entity_mut(target).insert(target_meridians);

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: None,
        issued_at_tick: 11,
        reach: FIST_REACH,
        qi_invest: 40.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: Some(crate::player::gameplay::CombatAction {
            target: "Crimson".to_string(),
            qi_invest: 40.0,
        }),
    });

    app.update();
    app.update();
    app.update();

    let target_ref = app.world().entity(target);
    let attacker_ref = app.world().entity(attacker);
    let attacker_cultivation = attacker_ref
        .get::<Cultivation>()
        .expect("attacker should keep cultivation");
    let attacker_meridians = attacker_ref
        .get::<MeridianSystem>()
        .expect("attacker should keep meridians");
    let wounds = target_ref
        .get::<Wounds>()
        .expect("target should keep wounds");
    let stamina = target_ref
        .get::<Stamina>()
        .expect("target should keep stamina");
    let contamination = target_ref
        .get::<Contamination>()
        .expect("target should keep contamination");
    let status_effects = target_ref
        .get::<StatusEffects>()
        .expect("target should keep status effects");
    let meridians = target_ref
        .get::<MeridianSystem>()
        .expect("target should keep meridians");
    let life = target_ref
        .get::<LifeRecord>()
        .expect("target should keep life record");

    assert!(
        wounds.health_current <= 0.0,
        "damage should reduce health to zero"
    );
    assert_eq!(wounds.entries.len(), 1, "damage should record one wound");
    assert_eq!(
        wounds.entries[0].location,
        crate::body_plan::legacy_body_part_to_id(BodyPart::Chest)
    );
    assert_eq!(wounds.entries[0].kind, WoundKind::Blunt);
    assert!(
        stamina.current < stamina.max,
        "damage should consume stamina"
    );
    assert_eq!(stamina.state, StaminaState::Combat);
    assert_eq!(
        contamination.entries.len(),
        1,
        "valid attack should write contamination"
    );
    assert_eq!(
        contamination.entries[0].attacker_id.as_deref(),
        Some("offline:Azure")
    );
    assert!(status_effects
        .active
        .iter()
        .any(|effect| effect.kind == StatusEffectKind::Bleeding && effect.magnitude > 0.0));
    assert_eq!(attacker_cultivation.qi_current, 20.0);
    assert!(
        attacker_meridians.get(MeridianId::Lung).throughput_current > 0.0,
        "attack should add attacker meridian throughput"
    );
    assert!(
        meridians.get(MeridianId::Lung).throughput_current > 0.0,
        "valid attack should add meridian throughput"
    );
    assert!(matches!(
        meridians.get(MeridianId::Lung).cracks.last(),
        Some(crack) if crack.cause == CrackCause::Attack
    ));
    // plan-race-system-v1 P0 review r2（BLOCKING-2 收口）—— `BiographyEntry::CombatHit.
    // body_part` 现在直接来自 `hit_probe.part_id` 的 Display（`BodyPartId` 原始字符串，
    // 如 "chest"），不再是 legacy `BodyPart` 的 Debug 格式（如 "Chest"）——见
    // resolve_attack_intents 的 life_record.push 调用点注释。
    assert!(matches!(
        life.biography.last(),
        Some(BiographyEntry::CombatHit { attacker_id, body_part, wound_kind, .. })
            if attacker_id == "offline:Azure"
                && body_part == "chest"
                && wound_kind == "Blunt"
    ));
}

#[test]
fn invalid_debug_attacks_have_no_side_effects() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 3 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<SkillXpGain>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "Crimson",
        [20.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    for action in [
        crate::player::gameplay::CombatAction {
            target: "".to_string(),
            qi_invest: 20.0,
        },
        crate::player::gameplay::CombatAction {
            target: "Crimson".to_string(),
            qi_invest: 0.0,
        },
        crate::player::gameplay::CombatAction {
            target: "Crimson".to_string(),
            qi_invest: 20.0,
        },
    ] {
        app.world_mut().send_event(AttackIntent {
            attacker,
            target: None,
            issued_at_tick: 2,
            reach: FIST_REACH,
            qi_invest: action.qi_invest as f32,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: Some(action),
        });
        app.update();
    }

    let target_ref = app.world().entity(target);
    let wounds = target_ref
        .get::<Wounds>()
        .expect("target should keep wounds");
    let stamina = target_ref
        .get::<Stamina>()
        .expect("target should keep stamina");
    let contamination = target_ref
        .get::<Contamination>()
        .expect("target should keep contamination");
    let meridians = target_ref
        .get::<MeridianSystem>()
        .expect("target should keep meridians");

    assert_eq!(wounds.health_current, wounds.health_max);
    assert!(
        wounds.entries.is_empty(),
        "invalid attacks must not create wounds"
    );
    assert_eq!(stamina.current, stamina.max);
    assert!(
        contamination.entries.is_empty(),
        "invalid attacks must not contaminate"
    );
    assert_eq!(meridians.get(MeridianId::Lung).throughput_current, 0.0);

    let combat_events = app.world().resource::<Events<CombatEvent>>();
    let death_events = app.world().resource::<Events<DeathEvent>>();
    assert!(
        combat_events.is_empty(),
        "invalid attacks must not emit CombatEvent"
    );
    assert!(
        death_events.is_empty(),
        "invalid attacks must not emit DeathEvent"
    );
}

#[test]
fn npc_entity_target_attack_intent_flows_through_shared_resolver() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 44 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<SkillXpGain>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let npc_attacker = spawn_npc(
        &mut app,
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "Crimson",
        [1.0, 64.0, 0.0],
        Wounds {
            health_current: 5.0,
            health_max: 100.0,
            entries: Vec::new(),
        },
        Stamina::default(),
    );

    app.world_mut().send_event(AttackIntent {
        attacker: npc_attacker,
        target: Some(target),
        issued_at_tick: 43,
        reach: NpcMeleeProfile::spear().reach,
        qi_invest: 10.0,
        wound_kind: NpcMeleeProfile::spear().wound_kind,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();
    app.update();

    let target_ref = app.world().entity(target);
    let wounds = target_ref
        .get::<Wounds>()
        .expect("target should keep wounds");
    let contamination = target_ref
        .get::<Contamination>()
        .expect("target should keep contamination");

    assert!(
        wounds.health_current <= 0.0,
        "npc entity-target intent should apply lethal damage"
    );
    assert_eq!(
        wounds.entries.len(),
        1,
        "resolver should append exactly one wound"
    );
    // §8.1 #1/#3 决议：NPC 攻方走"目标几何中心 + 确定性 jitter"，不再恒为 Chest——
    // 用同一套 raycast 公开函数独立复算期望部位，验证真实接了瞄准链路（而非硬编枚举）。
    let expected_body_part = expected_npc_hit_body_part(
        [0.0, 64.0, 0.0],
        &canonical_npc_id(npc_attacker),
        [1.0, 64.0, 0.0],
        43,
        NpcMeleeProfile::spear().reach,
    );
    assert_eq!(
        wounds.entries[0].location, expected_body_part,
        "npc 攻击命中部位应由 raycast::npc_aim_direction 的确定性 jitter 决定，\
             不再恒为 Chest（§8.1 #1）"
    );
    assert_eq!(wounds.entries[0].kind, WoundKind::Pierce);
    assert_eq!(
        contamination.entries[0].attacker_id.as_deref(),
        Some(canonical_npc_id(npc_attacker).as_str()),
        "npc attacker identity should use canonical_npc_id"
    );

    let combat_events = app.world().resource::<Events<CombatEvent>>();
    let death_events = app.world().resource::<Events<DeathEvent>>();
    assert!(
        !combat_events.is_empty(),
        "npc entity-target intent should still emit CombatEvent via shared resolver"
    );
    assert!(
        !death_events.is_empty(),
        "npc entity-target intent should emit DeathEvent when lethal"
    );
    assert!(
        app.world().resource::<Events<SkillXpGain>>().is_empty(),
        "NPC attackers should not earn player skill XP"
    );
}

#[test]
fn juebi_law_disruption_reduces_hit_and_backfires_attacker() {
    fn run_once(disrupted: bool) -> (f32, f32, f64) {
        let mut app = qi_test_app();
        app.insert_resource(CombatClock { tick: 12 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<crate::combat::weapon::ShieldBlockHit>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [0.25, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        if disrupted {
            app.world_mut()
                .entity_mut(attacker)
                .insert(JueBiLawDisruption {
                    epicenter: valence::prelude::BlockPos::new(0, 64, 0),
                    distance: 0.0,
                    seed: 11,
                });
        }

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 11,
            reach: FIST_REACH,
            qi_invest: 20.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();
        app.update();

        let attacker_wounds = app.world().get::<Wounds>(attacker).unwrap();
        let target_wounds = app.world().get::<Wounds>(target).unwrap();
        let attacker_meridians = app.world().get::<MeridianSystem>(attacker).unwrap();
        (
            target_wounds.health_max - target_wounds.health_current,
            attacker_wounds.health_max - attacker_wounds.health_current,
            attacker_meridians.get(MeridianId::Lung).throughput_current,
        )
    }

    let (normal_damage, normal_backfire, normal_throughput) = run_once(false);
    let (disrupted_damage, disrupted_backfire, disrupted_throughput) = run_once(true);

    assert!(normal_damage > 1.0);
    assert_eq!(normal_backfire, 0.0);
    assert!(disrupted_damage < normal_damage);
    assert!(disrupted_backfire > 0.0);
    assert!(disrupted_throughput > normal_throughput);
}

#[test]
fn player_to_npc_and_npc_to_player_share_same_resolver_path() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 91 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<SkillXpGain>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let player = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let npc = spawn_npc(
        &mut app,
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut().send_event(AttackIntent {
        attacker: player,
        target: Some(npc),
        issued_at_tick: 90,
        reach: FIST_REACH,
        qi_invest: 12.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.world_mut().send_event(AttackIntent {
        attacker: npc,
        target: Some(player),
        issued_at_tick: 90,
        reach: NpcMeleeProfile::spear().reach,
        qi_invest: 10.0,
        wound_kind: NpcMeleeProfile::spear().wound_kind,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let player_ref = app.world().entity(player);
    let npc_ref = app.world().entity(npc);
    let player_wounds = player_ref
        .get::<Wounds>()
        .expect("player target should keep wounds");
    let npc_wounds = npc_ref
        .get::<Wounds>()
        .expect("npc target should keep wounds");
    let player_contamination = player_ref
        .get::<Contamination>()
        .expect("player target should keep contamination");
    let npc_contamination = npc_ref
        .get::<Contamination>()
        .expect("npc target should keep contamination");

    assert_eq!(
        player_wounds.entries.len(),
        1,
        "npc->player should resolve exactly one wound"
    );
    // §8.1 #1/#3 决议：NPC 攻方(npc->player)走"目标几何中心 + 确定性 jitter"，
    // 不再恒为 Chest——用同一套 raycast 公开函数独立复算期望部位。
    let expected_npc_to_player_body_part = expected_npc_hit_body_part(
        [1.0, 64.0, 0.0],
        &canonical_npc_id(npc),
        [0.0, 64.0, 0.0],
        90,
        NpcMeleeProfile::spear().reach,
    );
    assert_eq!(
        player_wounds.entries[0].location, expected_npc_to_player_body_part,
        "npc 攻击命中部位应由 raycast::npc_aim_direction 的确定性 jitter 决定，\
             不再恒为 Chest（§8.1 #1）"
    );
    assert_eq!(player_wounds.entries[0].kind, WoundKind::Pierce);
    // player->npc：玩家攻手 Look 未显式设置(默认哨兵值)，走几何中心 fallback，
    // 与旧实现的恒定 Chest 结果一致（§P0 决议：默认 Look 视同缺失瞄准数据）。
    assert_eq!(
        npc_wounds.entries.len(),
        1,
        "player->npc should resolve exactly one wound"
    );
    assert_eq!(
        npc_wounds.entries[0].location,
        crate::body_plan::legacy_body_part_to_id(BodyPart::Chest)
    );
    assert_eq!(npc_wounds.entries[0].kind, WoundKind::Blunt);
    assert_eq!(
        player_contamination.entries[0].attacker_id.as_deref(),
        Some(canonical_npc_id(npc).as_str())
    );
    assert_eq!(
        npc_contamination.entries[0].attacker_id.as_deref(),
        Some("offline:Azure")
    );

    let combat_events = app.world().resource::<Events<CombatEvent>>();
    assert!(
        !combat_events.is_empty(),
        "both directions should emit CombatEvent through the same resolver event family"
    );
}

#[test]
fn zero_qi_npc_mundane_melee_damages_survival_player() {
    use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};

    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 93 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let player = spawn_player(
        &mut app,
        "Azure",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let npc = app
        .world_mut()
        .spawn((NpcMarker, Position::new([0.0, 64.0, 0.0])))
        .id();
    let runtime = npc_runtime_bundle(npc, NpcArchetype::Zombie, Realm::Awaken);
    assert_eq!(runtime.cultivation.qi_current, 0.0);
    app.world_mut().entity_mut(npc).insert(runtime);

    let before = app
        .world()
        .entity(player)
        .get::<Wounds>()
        .unwrap()
        .health_current;
    app.world_mut().send_event(AttackIntent {
        attacker: npc,
        target: Some(player),
        issued_at_tick: 92,
        reach: NpcMeleeProfile::fist().reach,
        qi_invest: 0.0,
        wound_kind: NpcMeleeProfile::fist().wound_kind,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let player_wounds = app.world().entity(player).get::<Wounds>().unwrap();
    assert!(
        player_wounds.health_current < before,
        "mundane NPC melee must damage Survival players without requiring qi"
    );
    let events: Vec<_> = app
        .world()
        .resource::<Events<CombatEvent>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].damage, 0.0);
    assert!(
        events[0].physical_damage > 0.0,
        "mundane NPC melee should surface as physical damage"
    );
}

#[test]
fn player_killing_npc_emits_combat_skill_xp() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 92 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<SkillXpGain>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let player = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let npc = spawn_npc(
        &mut app,
        [1.0, 64.0, 0.0],
        Wounds {
            health_current: 3.0,
            health_max: 100.0,
            entries: Vec::new(),
        },
        Stamina::default(),
    );

    app.world_mut().send_event(AttackIntent {
        attacker: player,
        target: Some(npc),
        issued_at_tick: 91,
        reach: FIST_REACH,
        qi_invest: 12.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let xp_events = app.world().resource::<Events<SkillXpGain>>();
    let xp = xp_events
        .iter_current_update_events()
        .next()
        .expect("lethal player->npc hit should award combat xp");
    assert_eq!(xp.char_entity, player);
    assert_eq!(xp.skill, SkillId::Combat);
    assert_eq!(xp.amount, 4);
}

#[test]
fn player_to_runtime_spawned_zombie_npc_target_resolves_without_dropping_intent() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 128 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_systems(
        valence::prelude::Startup,
        (setup_test_layer, spawn_runtime_npc.after(setup_test_layer)),
    );
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    app.update();
    app.update();

    let npc = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query
            .iter(world)
            .next()
            .expect("runtime zombie NPC should be spawned for resolver coverage test")
    };

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [13.0, 66.0, 14.0],
        Wounds::default(),
        Stamina::default(),
    );
    // §8.1 #1 决议 + §8.1 #4 "玩家垂直视角自然涌现"：显式给玩家设置真实 Look
    // （非默认哨兵值），证明命中部位由真实瞄准方向决定，而非恒定 fallback 胸口。
    // 玩家眼高(66+1.62=67.62)贴近僵尸头部阈值(rel_y>0.88≈67.584)，水平看向僵尸
    // (yaw=-90 正东，与 x+1 的僵尸位置同向) 即会命中 Head。
    app.world_mut()
        .entity_mut(attacker)
        .insert(Look::new(-90.0, 0.0));

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(npc),
        issued_at_tick: 127,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let npc_ref = app.world().entity(npc);
    let npc_wounds = npc_ref
        .get::<Wounds>()
        .expect("runtime zombie NPC should carry Wounds for shared resolver");
    let npc_contamination = npc_ref
        .get::<Contamination>()
        .expect("runtime zombie NPC should carry Contamination for shared resolver");

    assert_eq!(
        npc_wounds.entries.len(),
        1,
        "player->runtime-zombie intent should apply one wound"
    );
    assert_eq!(
        npc_wounds.entries[0].location,
        crate::body_plan::legacy_body_part_to_id(BodyPart::Head),
        "玩家显式设置真实 Look 水平看向僵尸，眼高贴近头部阈值应命中 Head，\
             而非旧实现恒定 fallback 的 Chest（§8.1 #1/#4）"
    );
    assert_eq!(npc_wounds.entries[0].kind, WoundKind::Blunt);
    assert_eq!(
        npc_contamination.entries[0].attacker_id.as_deref(),
        Some("offline:Azure"),
        "shared resolver should attribute player attacker on runtime zombie target"
    );

    let combat_events = app.world().resource::<Events<CombatEvent>>();
    assert!(
        !combat_events.is_empty(),
        "player->runtime-zombie intent should emit CombatEvent instead of dropping"
    );
}

#[test]
fn repeated_hits_on_dead_target_emit_single_death_event() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 300 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "Crimson",
        [1.0, 64.0, 0.0],
        Wounds {
            health_current: 1.0,
            health_max: 100.0,
            entries: Vec::new(),
        },
        Stamina::default(),
    );

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 299,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 300,
        reach: NpcMeleeProfile::spear().reach,
        qi_invest: 10.0,
        wound_kind: NpcMeleeProfile::spear().wound_kind,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let death_events = app.world().resource::<Events<DeathEvent>>();
    assert_eq!(
        death_events.len(),
        1,
        "DeathEvent should only emit on alive->dead transition, not repeated corpse hits"
    );
}

#[test]
fn debug_attack_resolves_canonical_npc_target_without_client_query_match() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 512 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let npc_target = spawn_npc(
        &mut app,
        [1.0, 64.0, 0.0],
        Wounds {
            health_current: 8.0,
            health_max: 100.0,
            entries: Vec::new(),
        },
        Stamina::default(),
    );
    let npc_id = canonical_npc_id(npc_target);

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: None,
        issued_at_tick: 511,
        reach: FIST_REACH,
        qi_invest: 40.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: Some(crate::player::gameplay::CombatAction {
            target: npc_id.clone(),
            qi_invest: 40.0,
        }),
    });

    app.update();

    let npc_ref = app.world().entity(npc_target);
    let wounds = npc_ref
        .get::<Wounds>()
        .expect("npc debug target should keep wounds");
    let contamination = npc_ref
        .get::<Contamination>()
        .expect("npc debug target should keep contamination");

    assert!(
        wounds.health_current <= 0.0,
        "debug npc target should receive resolver damage"
    );
    assert_eq!(
        contamination.entries[0].attacker_id.as_deref(),
        Some("offline:Azure"),
        "debug npc target should preserve canonical player attacker identity"
    );

    let combat_events = app.world().resource::<Events<CombatEvent>>();
    let death_events = app.world().resource::<Events<DeathEvent>>();
    assert!(
        !combat_events.is_empty(),
        "debug npc target should emit CombatEvent through shared resolver"
    );
    assert!(
        !death_events.is_empty(),
        "lethal debug npc target should emit DeathEvent"
    );
}

// ── 回归测试：黑武士近战 qi 闸门漏洞（plan-sword-path-v2 P3 review 发现）────────────────
//
// 修复前根因：`heiwushi_melee_slash_action_system` 发 `qi_invest = base_attack * phase_mult`
// （Phase1 = 35.0），但 `npc_runtime_bundle` 给 boss 的 `Cultivation::default()` 只有
// `qi_current = 0.0`。resolver §362-377 的 qi 闸门：
//   `qi_invest > EPSILON && !prepaid && qi_current < qi_invest`
// → `35.0 > ε && Melee 不在 prepaid 白名单 && 0.0 < 35.0` → continue（intent 被丢弃）。
// 结果：所有 Phase 的近战斩击永远打不到人。
//
// 修复后：`qi_invest = 0.0` 触发物理路径（is_physical_hit=true），
// 相位缩放伤害通过 `DerivedAttrs.attack_power` 传递，闸门不触发。
//
// 本测试专门覆盖 NPC 攻击者 qi_current=0 + qi_invest=0.0 能成功落伤的完整路径，
// 若将 qi_invest 改回 35.0（pre-fix），断言会失败（target wounds 不变）。
#[test]
fn heiwushi_melee_physical_path_lands_with_zero_qi_attacker() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 200 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    // Boss attacker: qi_current=0（npc_runtime_bundle default）+ attack_power=35.0
    // 这是修复前触发 qi 闸门的精确条件。
    let boss = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([0.0, 64.0, 0.0]),
            Cultivation {
                qi_current: 0.0,
                qi_max: 10.0,
                ..Cultivation::default()
            },
            MeridianSystem::default(),
            Contamination::default(),
            StatusEffects::default(),
            Wounds::default(),
            crate::combat::components::Stamina::default(),
            CombatState::default(),
            DerivedAttrs {
                attack_power: 35.0,
                ..DerivedAttrs::default()
            },
        ))
        .id();
    let canonical = canonical_npc_id(boss);
    app.world_mut().entity_mut(boss).insert((
        Lifecycle {
            character_id: canonical.clone(),
            ..Default::default()
        },
        LifeRecord::new(canonical),
    ));

    // Player target — 近战距离内 (1 block away)
    let target = spawn_player(
        &mut app,
        "Crimson",
        [1.0, 64.0, 0.0],
        Wounds {
            health_current: 100.0,
            health_max: 100.0,
            entries: Vec::new(),
        },
        crate::combat::components::Stamina::default(),
    );

    // 物理命中路径：qi_invest=0.0（修复后的黑武士近战斩击）
    app.world_mut().send_event(AttackIntent {
        attacker: boss,
        target: Some(target),
        issued_at_tick: 199,
        reach: AttackReach::new(3.0, 0.5),
        qi_invest: 0.0,
        wound_kind: WoundKind::Cut,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let target_wounds = app.world().entity(target).get::<Wounds>().unwrap();
    assert!(
        !target_wounds.entries.is_empty(),
        "黑武士近战（qi_invest=0, qi_current=0）必须落至少一道伤，实际伤口数={}。\
            若此断言失败，说明 qi 闸门仍在拦截物理近战。",
        target_wounds.entries.len()
    );
    assert!(
        target_wounds.health_current < 100.0,
        "黑武士近战必须扣血，修复前 qi_invest=35.0 被闸门拒绝导致此处不降；\
            实际 health_current={:.2}，期望 < 100.0",
        target_wounds.health_current
    );

    let combat_events = app.world().resource::<Events<CombatEvent>>();
    assert!(
        !combat_events.is_empty(),
        "黑武士近战命中必须 emit CombatEvent（resolver 未拒绝才会到达此处）"
    );
}

#[test]
fn insufficient_qi_prevents_attack_side_effects() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 901 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_npc(
        &mut app,
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut().entity_mut(attacker).insert(Cultivation {
        qi_current: 5.0,
        qi_max: 100.0,
        ..Cultivation::default()
    });

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 900,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let attacker_ref = app.world().entity(attacker);
    let target_ref = app.world().entity(target);
    let attacker_cultivation = attacker_ref.get::<Cultivation>().unwrap();
    let target_wounds = target_ref.get::<Wounds>().unwrap();
    let target_contamination = target_ref.get::<Contamination>().unwrap();
    let combat_events = app.world().resource::<Events<CombatEvent>>();
    let death_events = app.world().resource::<Events<DeathEvent>>();

    assert_eq!(attacker_cultivation.qi_current, 5.0);
    assert_eq!(target_wounds.health_current, target_wounds.health_max);
    assert!(target_wounds.entries.is_empty());
    assert!(target_contamination.entries.is_empty());
    assert!(combat_events.is_empty());
    assert!(death_events.is_empty());
}

#[test]
fn anticheat_qi_invest_violation_counts_without_changing_rejection() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 903 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_npc(
        &mut app,
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut().entity_mut(attacker).insert((
        Cultivation {
            qi_current: 5.0,
            qi_max: 100.0,
            ..Cultivation::default()
        },
        AntiCheatCounter::default(),
    ));

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 902,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let counter = app
        .world()
        .entity(attacker)
        .get::<AntiCheatCounter>()
        .unwrap();
    assert_eq!(counter.qi_invest_violations, 1);
    let target_ref = app.world().entity(target);
    assert!(
        target_ref.get::<Wounds>().unwrap().entries.is_empty(),
        "insufficient qi behavior should remain rejection"
    );
    assert!(
        app.world().resource::<Events<CombatEvent>>().is_empty(),
        "qi violation counting must not emit combat side effects"
    );
}

#[test]
fn anticheat_reach_violation_counts_without_changing_miss() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 904 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_npc(
        &mut app,
        [4.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut()
        .entity_mut(attacker)
        .insert(AntiCheatCounter::default());

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 903,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let counter = app
        .world()
        .entity(attacker)
        .get::<AntiCheatCounter>()
        .unwrap();
    assert_eq!(counter.reach_violations, 1);
    let target_ref = app.world().entity(target);
    assert_eq!(
        target_ref.get::<Wounds>().unwrap().health_current,
        target_ref.get::<Wounds>().unwrap().health_max
    );
    assert!(target_ref.get::<Wounds>().unwrap().entries.is_empty());
    assert_eq!(
        app.world()
            .entity(attacker)
            .get::<Cultivation>()
            .unwrap()
            .qi_current,
        60.0,
        "超距请求在合法性校验前必须零 qi mutation；被拒后 qi_current 应保持初始值"
    );
    assert_eq!(
        app.world()
            .resource::<WorldQiAccount>()
            .balance(&crate::qi_physics::qi_flow_overflow_account()),
        0.0,
        "超距请求被拒后不得向 qi_flow_overflow 账户写入 qi_invest"
    );
    assert_eq!(
        app.world().resource::<WorldQiAccount>().transfers().len(),
        0,
        "超距请求被拒后不得产生任何 ledger transfer"
    );
}

#[test]
fn anticheat_cooldown_violation_counts_without_blocking_hit() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 905 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_npc(
        &mut app,
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut().entity_mut(attacker).insert((
        AntiCheatCounter::default(),
        CombatState {
            last_attack_at_tick: Some(904),
            ..CombatState::default()
        },
    ));

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 904,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let counter = app
        .world()
        .entity(attacker)
        .get::<AntiCheatCounter>()
        .unwrap();
    assert_eq!(counter.cooldown_violations, 1);
    assert!(
        !app.world()
            .entity(target)
            .get::<Wounds>()
            .unwrap()
            .entries
            .is_empty(),
        "cooldown violation reporting must not change current hit resolution"
    );
    assert!(
        !app.world().resource::<Events<CombatEvent>>().is_empty(),
        "hit should still emit CombatEvent"
    );
}

#[test]
fn debug_target_selection_does_not_change_damage_when_qi_invest_matches() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 902 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target_a = spawn_player(
        &mut app,
        "Crimson",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target_b = spawn_player(
        &mut app,
        "Sable",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: None,
        issued_at_tick: 901,
        reach: FIST_REACH,
        qi_invest: 18.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: Some(crate::player::gameplay::CombatAction {
            target: "Crimson".to_string(),
            qi_invest: 18.0,
        }),
    });
    app.update();

    let first_damage = app
        .world()
        .entity(target_a)
        .get::<Wounds>()
        .unwrap()
        .entries
        .last()
        .expect("first debug hit should create wound")
        .severity;

    app.world_mut().entity_mut(attacker).insert(Cultivation {
        qi_current: 60.0,
        qi_max: 100.0,
        ..Cultivation::default()
    });

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: None,
        issued_at_tick: 902,
        reach: FIST_REACH,
        qi_invest: 18.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: Some(crate::player::gameplay::CombatAction {
            target: "Sable".to_string(),
            qi_invest: 999.0,
        }),
    });
    app.update();

    let second_damage = app
        .world()
        .entity(target_b)
        .get::<Wounds>()
        .unwrap()
        .entries
        .last()
        .expect("second debug hit should create wound")
        .severity;

    assert_eq!(first_damage, second_damage);
}

#[test]
fn jiemai_window_spends_qi_reduces_contam_and_adds_concussion() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1000 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "Crimson",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut().entity_mut(target).insert((
        CombatState {
            incoming_window: Some(DefenseWindow {
                opened_at_tick: 999,
                duration_ms: 200,
            }),
            ..CombatState::default()
        },
        Cultivation {
            realm: Realm::Induce,
            qi_current: 20.0,
            qi_max: 100.0,
            ..Cultivation::default()
        },
        PracticeLog::default(),
    ));

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 999,
        reach: FIST_REACH,
        qi_invest: 20.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let target_ref = app.world().entity(target);
    let wounds = target_ref.get::<Wounds>().unwrap();
    let contamination = target_ref.get::<Contamination>().unwrap();
    let cultivation = target_ref.get::<Cultivation>().unwrap();
    let state = target_ref.get::<CombatState>().unwrap();
    let combat_events = app.world().resource::<Events<CombatEvent>>();
    let event = combat_events
        .iter_current_update_events()
        .next()
        .expect("combat event should emit");

    let expected_effectiveness = event
        .defense_effectiveness
        .expect("jiemai success should report effectiveness");
    assert!((expected_effectiveness - 0.3).abs() < 1e-6);
    assert_eq!(
        cultivation.qi_current,
        20.0 - zhenmai_v2::parry_qi_cost_for_realm(Realm::Induce).unwrap()
    );
    assert!(state.incoming_window.is_none());
    assert_eq!(wounds.entries.len(), 2);
    assert!(wounds
        .entries
        .iter()
        .any(|w| w.kind == WoundKind::Concussion));
    let base_contam = f64::from(event.damage) * 0.25 * 0.8;
    assert_eq!(
        event.contam_delta,
        base_contam * jiemai_contam_multiplier_for_effectiveness(expected_effectiveness)
    );
    assert_eq!(event.defense_kind, Some(DefenseKind::JieMai));
    assert_eq!(event.defense_wound_severity, Some(1.0));
    assert_eq!(contamination.entries.len(), 1);
    assert_eq!(contamination.entries[0].amount, event.contam_delta);
    let life = target_ref.get::<LifeRecord>().unwrap();
    assert!(matches!(
        life.biography.last(),
        Some(BiographyEntry::JiemaiParry {
            attacker_id,
            effectiveness,
            tick,
        }) if attacker_id == "offline:Azure"
            && (*effectiveness - expected_effectiveness).abs() < 1e-6
            && *tick == 1000
    ));
    assert_eq!(
        target_ref
            .get::<PracticeLog>()
            .unwrap()
            .weights
            .get(&ColorKind::Violent)
            .copied(),
        Some(crate::cultivation::color::STYLE_PRACTICE_AMOUNT)
    );
}

// ── bughunt r2 QP-003 — jiemai 格挡真元守恒：扣除 qi_cost 后必须回灌到 zone ──

/// 完整 happy path：成功格挡后 QiTransfer 事件携带正确金额和 ReleaseToZone reason。
///
/// 守恒不变式：`parry_qi_cost == qi_transfer.amount`（不凭空消失）。
/// 验证目标：防守方 qi_current 减少 PARRY_QI_COST；同 tick 内 QiTransfer 发出；
///            amount == PARRY_QI_COST；reason == ReleaseToZone；to == zone:spawn（玩家在 spawn 区）。
#[test]
fn jiemai_parry_emits_qi_transfer_for_conservation() {
    use crate::qi_physics::ledger::QiTransferReason;
    use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1000 });
    app.insert_resource(ZoneRegistry::fallback());
    // fallback() 的 spawn zone 默认接近满（spirit_qi≈0.9），余量不足以吸收整份格挡费用 →
    // 会拆成 zone 部分 + overflow 部分。清空 spawn zone 让整份 qi_cost 落入 zone，
    // 锁住「完好回灌」happy path（守恒两条路都成立，此处验全额入 zone 的契约）。
    if let Some(zone) = app
        .world_mut()
        .resource_mut::<ZoneRegistry>()
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
    {
        zone.spirit_qi = 0.0;
    }
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "Attacker",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "Defender",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    let initial_qi = 20.0;
    let parry_cost = crate::combat::zhenmai_v2::PARRY_QI_COST;
    app.world_mut().entity_mut(target).insert((
        CombatState {
            incoming_window: Some(DefenseWindow {
                opened_at_tick: 999,
                duration_ms: 200,
            }),
            ..CombatState::default()
        },
        Cultivation {
            realm: Realm::Induce,
            qi_current: initial_qi,
            qi_max: 100.0,
            ..Cultivation::default()
        },
        // 真实玩家出生即带 CurrentDimension（player/mod.rs），生产路径 find_zone 能定位 zone；
        // 测试需显式补上，否则 defender_dim=None → release_qi_amount_to_zone 回退 Overflow 账户，
        // 守恒回灌断言（to=Zone:spawn）撞红。
        crate::world::dimension::CurrentDimension::default(),
    ));

    // 该用例只验证防守方的 canonical 截脉转账；攻击方使用已预付的
    // BurstMeridian，避免把基线 resolver 的普通 qi_invest 未入 ledger 缺陷混入守恒对拍。
    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 999,
        reach: FIST_REACH,
        qi_invest: 20.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::BurstMeridian,
        debug_command: None,
    });

    let before = summarize_world_qi(app.world_mut());
    app.update();
    let after = summarize_world_qi(app.world_mut());
    assert_full_qi_conservation(&before, &after, "截脉格挡成功");
    assert!(
        ((after.zone_qi - before.zone_qi) - parry_cost).abs() <= f64::EPSILON * QI_ZONE_UNIT_CAPACITY,
        "截脉格挡成功后扣除的 {parry_cost} 真元必须实际进入 zone 物理账户；before zone_qi={}, after zone_qi={}",
        before.zone_qi,
        after.zone_qi,
    );

    // 验证：防守方真元扣减正确。
    let cultivation = app.world().entity(target).get::<Cultivation>().unwrap();
    assert_eq!(
        cultivation.qi_current,
        initial_qi - parry_cost,
        "守恒前置：防守方 qi_current 应减少 PARRY_QI_COST={parry_cost}，\
             实际 qi_current={:.3}（初始={initial_qi}）",
        cultivation.qi_current,
    );

    // 主守恒断言：格挡消耗的真元必须回灌到所在 zone（ReleaseToZone reason）。
    let transfers: Vec<_> = app
        .world()
        .resource::<Events<QiTransfer>>()
        .iter_current_update_events()
        .cloned()
        .collect();
    let expected_transfer = QiTransfer {
        from: QiAccountId::player(canonical_player_id("Defender")),
        to: crate::qi_physics::QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME.to_string()),
        amount: parry_cost,
        reason: QiTransferReason::ReleaseToZone,
    };
    let parry_transfer = transfers
        .iter()
        .find(|transfer| **transfer == expected_transfer);
    assert!(
        parry_transfer.is_some(),
        "守恒红线：jiemai 格挡消耗 {parry_cost} 真元，必须 emit QiTransfer(reason=ReleaseToZone, \
             amount={parry_cost})；实际 transfers={:?}",
        transfers,
    );
    let t = parry_transfer.unwrap();
    assert_eq!(
        t, &expected_transfer,
        "格挡路径必须传播完整 QiTransfer {{ from, to, amount, reason }}；实际={t:?}"
    );
    assert!(
        app.world()
            .resource::<WorldQiAccount>()
            .transfers()
            .iter()
            .any(|ledger_transfer| ledger_transfer == &expected_transfer),
        "截脉格挡的完整 QiTransfer 必须进入 WorldQiAccount 审计轨迹，不能只发 Event"
    );
}

/// 边界：防守方没有足够真元时格挡失败，不应 emit 任何 jiemai QiTransfer。
///
/// 守恒不变式：无格挡发生 → 无真元被扣 → 无 QiTransfer 回灌（不凭空创造转账）。
#[test]
fn jiemai_parry_no_qi_transfer_when_insufficient_qi() {
    use crate::qi_physics::ledger::QiTransferReason;
    use crate::world::zone::ZoneRegistry;

    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1000 });
    app.insert_resource(ZoneRegistry::fallback());
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "Attacker",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "Defender",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    // 防守方真元不足（1.0 < PARRY_QI_COST=8.0），格挡条件不满足。
    app.world_mut().entity_mut(target).insert((
        CombatState {
            incoming_window: Some(DefenseWindow {
                opened_at_tick: 999,
                duration_ms: 200,
            }),
            ..CombatState::default()
        },
        Cultivation {
            realm: Realm::Induce,
            qi_current: 1.0, // < PARRY_QI_COST
            qi_max: 100.0,
            ..Cultivation::default()
        },
    ));

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 999,
        reach: FIST_REACH,
        qi_invest: 20.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    // 格挡未触发：qi_current 保持不变。
    let cultivation = app.world().entity(target).get::<Cultivation>().unwrap();
    assert_eq!(
        cultivation.qi_current, 1.0,
        "真元不足时格挡失败，qi_current 不应变动；实际 qi_current={:.3}",
        cultivation.qi_current
    );

    // 守恒不变式：无格挡 → 不产生 ReleaseToZone QiTransfer。
    let transfers: Vec<_> = app
        .world()
        .resource::<Events<QiTransfer>>()
        .iter_current_update_events()
        .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
        .collect();
    assert!(
        transfers.is_empty(),
        "格挡条件不满足时不应有 ReleaseToZone 转账（避免凭空回灌）；\
             实际 transfers={:?}",
        transfers,
    );
}

#[test]
fn jiemai_without_qi_falls_back_to_normal_settlement() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1001 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "Crimson",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut().entity_mut(target).insert((
        CombatState {
            incoming_window: Some(DefenseWindow {
                opened_at_tick: 1000,
                duration_ms: 200,
            }),
            ..CombatState::default()
        },
        Cultivation {
            realm: Realm::Induce,
            qi_current: 1.0,
            qi_max: 100.0,
            ..Cultivation::default()
        },
    ));

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 1000,
        reach: FIST_REACH,
        qi_invest: 20.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let target_ref = app.world().entity(target);
    let wounds = target_ref.get::<Wounds>().unwrap();
    let contamination = target_ref.get::<Contamination>().unwrap();
    let cultivation = target_ref.get::<Cultivation>().unwrap();
    let state = target_ref.get::<CombatState>().unwrap();
    let combat_events = app.world().resource::<Events<CombatEvent>>();
    let event = combat_events
        .iter_current_update_events()
        .next()
        .expect("combat event should emit");

    assert_eq!(cultivation.qi_current, 1.0);
    assert!(state.incoming_window.is_none());
    assert_eq!(wounds.entries.len(), 1);
    assert!(!wounds
        .entries
        .iter()
        .any(|w| w.kind == WoundKind::Concussion));
    let base_contam = f64::from(event.damage) * 0.25 * 0.8;
    assert_eq!(event.contam_delta, base_contam);
    assert_eq!(contamination.entries[0].amount, base_contam);
}

#[test]
fn expired_jiemai_window_does_not_mitigate() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1006 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "Crimson",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut().entity_mut(target).insert((
        CombatState {
            incoming_window: Some(DefenseWindow {
                opened_at_tick: 1000,
                duration_ms: 200,
            }),
            ..CombatState::default()
        },
        Cultivation {
            realm: Realm::Induce,
            qi_current: 20.0,
            qi_max: 100.0,
            ..Cultivation::default()
        },
    ));

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 1005,
        reach: FIST_REACH,
        qi_invest: 20.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let target_ref = app.world().entity(target);
    let wounds = target_ref.get::<Wounds>().unwrap();
    let contamination = target_ref.get::<Contamination>().unwrap();
    let cultivation = target_ref.get::<Cultivation>().unwrap();
    let state = target_ref.get::<CombatState>().unwrap();
    let combat_events = app.world().resource::<Events<CombatEvent>>();
    let event = combat_events
        .iter_current_update_events()
        .next()
        .expect("combat event should emit");

    assert_eq!(cultivation.qi_current, 20.0);
    assert!(state.incoming_window.is_none());
    assert_eq!(wounds.entries.len(), 1);
    assert_eq!(contamination.entries[0].amount, event.contam_delta);
}

#[test]
fn resolver_uses_attack_power_for_outgoing_damage() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1300 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<WeaponBroken>();
    app.add_event::<ShieldBroken>();
    app.add_event::<ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(
        Update,
        (
            crate::combat::status::attribute_aggregate_tick,
            resolve_attack_intents,
        ),
    );

    let baseline_attacker = spawn_player(
        &mut app,
        "AzureBase",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let amp_attacker = spawn_player(
        &mut app,
        "AzureAmp",
        [0.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );
    let baseline_target = spawn_player(
        &mut app,
        "CrimsonBase",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let amp_target = spawn_player(
        &mut app,
        "CrimsonAmp",
        [1.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut()
        .entity_mut(amp_attacker)
        .insert(StatusEffects {
            active: vec![crate::combat::components::ActiveStatusEffect {
                kind: StatusEffectKind::DamageAmp,
                magnitude: 0.25,
                remaining_ticks: 20,
                source_pill: None,
            }],
        });

    app.update();

    app.world_mut().send_event(AttackIntent {
        attacker: baseline_attacker,
        target: Some(baseline_target),
        issued_at_tick: 1299,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.world_mut().send_event(AttackIntent {
        attacker: amp_attacker,
        target: Some(amp_target),
        issued_at_tick: 1299,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let baseline_target_ref = app.world().entity(baseline_target);
    let amp_target_ref = app.world().entity(amp_target);
    let baseline_wounds = baseline_target_ref.get::<Wounds>().unwrap();
    let amp_wounds = amp_target_ref.get::<Wounds>().unwrap();
    let combat_events = app.world().resource::<Events<CombatEvent>>();
    let events: Vec<_> = combat_events.iter_current_update_events().collect();

    assert_eq!(events.len(), 2);
    let baseline_damage = events[0].damage;
    let amp_damage = events[1].damage;

    assert!(amp_damage > baseline_damage);
    assert!(
        (baseline_wounds.health_current - (baseline_wounds.health_max - baseline_damage)).abs()
            < 0.001
    );
    assert!((amp_wounds.health_current - (amp_wounds.health_max - amp_damage)).abs() < 0.001);
}

#[test]
fn resolver_applies_defense_power_to_incoming_damage() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1350 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<WeaponBroken>();
    app.add_event::<ShieldBroken>();
    app.add_event::<ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(
        Update,
        (
            crate::combat::status::attribute_aggregate_tick,
            resolve_attack_intents,
        ),
    );

    let baseline_attacker = spawn_player(
        &mut app,
        "AzureBaseDef",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let reduced_attacker = spawn_player(
        &mut app,
        "AzureRedDef",
        [0.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );
    let baseline_target = spawn_player(
        &mut app,
        "CrimsonBaseDef",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let reduced_target = spawn_player(
        &mut app,
        "CrimsonRedDef",
        [1.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut()
        .entity_mut(reduced_target)
        .insert(StatusEffects {
            active: vec![crate::combat::components::ActiveStatusEffect {
                kind: StatusEffectKind::DamageReduction,
                magnitude: 0.25,
                remaining_ticks: 20,
                source_pill: None,
            }],
        });

    app.update();

    app.world_mut().send_event(AttackIntent {
        attacker: baseline_attacker,
        target: Some(baseline_target),
        issued_at_tick: 1349,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.world_mut().send_event(AttackIntent {
        attacker: reduced_attacker,
        target: Some(reduced_target),
        issued_at_tick: 1349,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let baseline_target_ref = app.world().entity(baseline_target);
    let reduced_target_ref = app.world().entity(reduced_target);
    let baseline_wounds = baseline_target_ref.get::<Wounds>().unwrap();
    let reduced_wounds = reduced_target_ref.get::<Wounds>().unwrap();
    let combat_events = app.world().resource::<Events<CombatEvent>>();
    let events: Vec<_> = combat_events.iter_current_update_events().collect();

    assert_eq!(events.len(), 2);
    let baseline_damage = events[0].damage;
    let reduced_damage = events[1].damage;

    assert!(reduced_damage < baseline_damage);
    assert!(
        (baseline_wounds.health_current - (baseline_wounds.health_max - baseline_damage)).abs()
            < 0.001
    );
    assert!(
        (reduced_wounds.health_current - (reduced_wounds.health_max - reduced_damage)).abs()
            < 0.001
    );
}

#[test]
fn resolver_applies_tuike_naked_window_damage_penalty() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1370 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<WeaponBroken>();
    app.add_event::<ShieldBroken>();
    app.add_event::<ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let baseline_attacker = spawn_player(
        &mut app,
        "AzureBaseNaked",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let naked_attacker = spawn_player(
        &mut app,
        "AzureNaked",
        [0.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );
    let baseline_target = spawn_player(
        &mut app,
        "CrimsonBaseNaked",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let naked_target = spawn_player(
        &mut app,
        "CrimsonNaked",
        [1.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut()
        .entity_mut(naked_target)
        .insert(StackedFalseSkins {
            naked_until_tick: 1400,
            ..Default::default()
        });

    app.world_mut().send_event(AttackIntent {
        attacker: baseline_attacker,
        target: Some(baseline_target),
        issued_at_tick: 1369,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.world_mut().send_event(AttackIntent {
        attacker: naked_attacker,
        target: Some(naked_target),
        issued_at_tick: 1369,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let combat_events = app.world().resource::<Events<CombatEvent>>();
    let events: Vec<_> = combat_events.iter_current_update_events().collect();
    assert_eq!(events.len(), 2);
    assert!(
        events[1].damage > events[0].damage * 1.49,
        "裸壳期应把本次承伤放大到约 1.5 倍"
    );
}

#[test]
fn resolver_applies_backfire_amplification_to_defender_incoming_damage() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1360 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<WeaponBroken>();
    app.add_event::<ShieldBroken>();
    app.add_event::<ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(
        Update,
        (
            crate::combat::status::attribute_aggregate_tick,
            resolve_attack_intents,
        ),
    );

    let baseline_attacker = spawn_player(
        &mut app,
        "AzureBaseBackfire",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let amplified_attacker = spawn_player(
        &mut app,
        "AzureAmpBackfire",
        [0.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );
    let baseline_target = spawn_player(
        &mut app,
        "CrimsonBaseBackfire",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let amplified_target = spawn_player(
        &mut app,
        "CrimsonAmpBackfire",
        [1.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut()
        .entity_mut(amplified_target)
        .insert(BackfireAmplification {
            meridian_id: MeridianId::Du,
            attack_kind: crate::combat::zhenmai_v2::ZhenmaiAttackKind::RealYuan,
            started_at_tick: 1300,
            expires_at_tick: 1400,
            k_drain: 1.5,
            incoming_damage_multiplier: 0.5,
        });

    app.update();

    app.world_mut().send_event(AttackIntent {
        attacker: baseline_attacker,
        target: Some(baseline_target),
        issued_at_tick: 1359,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.world_mut().send_event(AttackIntent {
        attacker: amplified_attacker,
        target: Some(amplified_target),
        issued_at_tick: 1359,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let combat_events = app.world().resource::<Events<CombatEvent>>();
    let events: Vec<_> = combat_events.iter_current_update_events().collect();
    assert_eq!(events.len(), 2);
    let baseline_damage = events[0].damage;
    let amplified_damage = events[1].damage;

    assert!(
        amplified_damage < baseline_damage,
        "backfire amplification should reduce only the holder's incoming damage"
    );
    assert!(
        amplified_damage >= 1.0,
        "backfire amplification is not immunity; main hit still lands"
    );
}

// plan-weapon-v1 §6：武器加成 + 耐久扣减 + WeaponBroken 事件。
#[test]
fn weapon_increases_outgoing_damage_versus_unarmed() {
    use crate::combat::weapon::{Weapon, WeaponKind};
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1400 });
    app.insert_resource(weapon_test_registry());
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<WeaponBroken>();
    app.add_event::<ShieldBroken>();
    app.add_event::<ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(
        Update,
        (
            crate::combat::status::attribute_aggregate_tick,
            resolve_attack_intents,
        ),
    );

    let unarmed = spawn_player(
        &mut app,
        "Unarmed",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let armed = spawn_player(
        &mut app,
        "Swordsman",
        [0.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut().entity_mut(armed).insert(PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers: vec![ContainerState {
            quick_access: false,
            id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 5,
            cols: 7,
            items: vec![],
            owner_instance_id: None,
        }],
        equipped: std::collections::HashMap::from([(
            crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
            crate::inventory::SlotContents::held_single(ItemInstance {
                instance_id: 1,
                template_id: "strong_sword".to_string(),
                display_name: "强剑".to_string(),
                grid_w: 1,
                grid_h: 2,
                weight: 1.0,
                rarity: crate::inventory::ItemRarity::Common,
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
    });
    let t1 = spawn_player(
        &mut app,
        "T1",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let t2 = spawn_player(
        &mut app,
        "T2",
        [1.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );

    // armed 手持强攻武器:attack_mul 2.0 × quality 1.0 × durability 1.0 = 2.0
    app.world_mut().entity_mut(armed).insert(Weapon {
        slot: crate::combat::weapon::EquipSlot::MainHand,
        instance_id: 1,
        template_id: "strong_sword".to_string(),
        weapon_kind: WeaponKind::Sword,
        base_attack: 20.0, // attack_multiplier = 2.0
        quality_tier: 0,
        durability: 200.0,
        durability_max: 200.0,
    });

    app.update();

    app.world_mut().send_event(AttackIntent {
        attacker: unarmed,
        target: Some(t1),
        issued_at_tick: 1399,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.world_mut().send_event(AttackIntent {
        attacker: armed,
        target: Some(t2),
        issued_at_tick: 1399,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let combat_events = app.world().resource::<Events<CombatEvent>>();
    let events: Vec<_> = combat_events.iter_current_update_events().collect();
    assert_eq!(events.len(), 2);
    let unarmed_damage = events[0].damage;
    let armed_damage = events[1].damage;
    assert!(
        armed_damage > unarmed_damage * 1.5,
        "armed {armed_damage} should exceed unarmed {unarmed_damage} × 1.5"
    );

    // 命中后 armed attacker 的武器应有:durability ↓。
    let weapon = app.world().entity(armed).get::<Weapon>().unwrap();
    assert!(weapon.durability < 200.0, "durability ticked down");
    let inventory = app.world().entity(armed).get::<PlayerInventory>().unwrap();
    assert!(
        inventory.equipped[crate::inventory::EQUIP_SLOT_MAIN_HAND]
            .held
            .as_ref()
            .unwrap()
            .durability
            < 1.0,
        "inventory durability should persist the runtime wear"
    );
}

#[test]
fn iron_sword_increases_damage_by_at_least_20_percent_vs_unarmed() {
    use crate::combat::weapon::{EquipSlot, Weapon, WeaponKind};

    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1420 });
    app.insert_resource(weapon_test_registry());
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<WeaponBroken>();
    app.add_event::<ShieldBroken>();
    app.add_event::<ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(
        Update,
        (
            crate::combat::status::attribute_aggregate_tick,
            resolve_attack_intents,
        ),
    );

    let unarmed = spawn_player(
        &mut app,
        "UnarmedIronBaseline",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let armed = spawn_player(
        &mut app,
        "IronSwordUser",
        [0.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut().entity_mut(armed).insert(PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers: vec![ContainerState {
            quick_access: false,
            id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 5,
            cols: 7,
            items: vec![],
            owner_instance_id: None,
        }],
        equipped: std::collections::HashMap::from([(
            crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
            crate::inventory::SlotContents::held_single(ItemInstance {
                instance_id: 120,
                template_id: "iron_sword".to_string(),
                display_name: "铁剑".to_string(),
                grid_w: 1,
                grid_h: 2,
                weight: 1.2,
                rarity: crate::inventory::ItemRarity::Common,
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
    });
    app.world_mut().entity_mut(armed).insert(Weapon {
        slot: EquipSlot::MainHand,
        instance_id: 120,
        template_id: "iron_sword".to_string(),
        weapon_kind: WeaponKind::Sword,
        base_attack: 12.0,
        quality_tier: 0,
        durability: 200.0,
        durability_max: 200.0,
    });
    let unarmed_target = spawn_player(
        &mut app,
        "IronBaselineTarget",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let armed_target = spawn_player(
        &mut app,
        "IronSwordTarget",
        [1.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.update();

    app.world_mut().send_event(AttackIntent {
        attacker: unarmed,
        target: Some(unarmed_target),
        issued_at_tick: 1419,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.world_mut().send_event(AttackIntent {
        attacker: armed,
        target: Some(armed_target),
        issued_at_tick: 1419,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let combat_events = app.world().resource::<Events<CombatEvent>>();
    let events: Vec<_> = combat_events.iter_current_update_events().collect();
    assert_eq!(events.len(), 2);
    let unarmed_damage = events[0].damage;
    let iron_sword_damage = events[1].damage;
    let ratio = iron_sword_damage / unarmed_damage;
    println!(
            "iron_sword_damage_check unarmed={unarmed_damage:.3} iron_sword={iron_sword_damage:.3} ratio={ratio:.3}"
        );
    assert!(
            ratio >= 1.2,
            "iron_sword damage {iron_sword_damage} should be >= unarmed {unarmed_damage} x 1.2; ratio={ratio}"
        );
    assert!(
        (iron_sword_damage - unarmed_damage * 1.2).abs() < 0.001,
        "expected full-durability iron_sword to land exactly at 1.2x baseline"
    );
}

#[test]
fn tool_main_hand_deals_low_damage_above_unarmed_below_entry_sword() {
    for (index, tool_kind) in crate::tools::ALL_TOOL_KINDS.into_iter().enumerate() {
        let mut app = qi_test_app();
        app.insert_resource(CombatClock { tick: 1430 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<WeaponBroken>();
        app.add_event::<ShieldBroken>();
        app.add_event::<ShieldBlockHit>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(
            Update,
            (
                crate::combat::status::attribute_aggregate_tick,
                resolve_attack_intents,
            ),
        );

        let z = (index as f64) * 3.0;
        let unarmed = spawn_player(
            &mut app,
            "BareHandBaseline",
            [0.0, 64.0, z],
            Wounds::default(),
            Stamina::default(),
        );
        let tool_user = spawn_player(
            &mut app,
            "ToolUser",
            [0.0, 64.0, z + 1.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut()
            .entity_mut(tool_user)
            .insert(PlayerInventory {
                triggered_treasures: Vec::new(),
                revision: InventoryRevision(1),
                containers: vec![ContainerState {
                    quick_access: false,
                    id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                    name: "主背包".to_string(),
                    rows: 5,
                    cols: 7,
                    items: vec![],
                    owner_instance_id: None,
                }],
                equipped: std::collections::HashMap::from([(
                    crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
                    crate::inventory::SlotContents::held_single(ItemInstance {
                        instance_id: 130 + index as u64,
                        template_id: tool_kind.item_id().to_string(),
                        display_name: tool_kind.display_name().to_string(),
                        grid_w: 1,
                        grid_h: 2,
                        weight: 0.9,
                        rarity: crate::inventory::ItemRarity::Common,
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
                    }),
                )]),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 50.0,
            });
        let unarmed_target = spawn_player(
            &mut app,
            "BareHandTarget",
            [1.0, 64.0, z],
            Wounds::default(),
            Stamina::default(),
        );
        let tool_target = spawn_player(
            &mut app,
            "ToolTarget",
            [1.0, 64.0, z + 1.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.update();

        app.world_mut().send_event(AttackIntent {
            attacker: unarmed,
            target: Some(unarmed_target),
            issued_at_tick: 1429,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: tool_user,
            target: Some(tool_target),
            issued_at_tick: 1429,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let events: Vec<_> = combat_events.iter_current_update_events().collect();
        assert_eq!(events.len(), 2, "{tool_kind:?} should emit two hits");
        let unarmed_damage = events[0].damage;
        let tool_damage = events[1].damage;
        assert!(
            tool_damage > unarmed_damage,
            "{tool_kind:?} should beat bare hands"
        );
        assert!(
            tool_damage < unarmed_damage * 1.2,
            "{tool_kind:?} should stay below entry iron sword"
        );
        assert!(
            (tool_damage - unarmed_damage * tool_kind.combat_damage_multiplier()).abs() < 0.001,
            "{tool_kind:?} should use its ToolKind multiplier"
        );

        let inventory = app.world().get::<PlayerInventory>(tool_user).unwrap();
        assert_eq!(
            inventory
                .equipped
                .get(crate::inventory::EQUIP_SLOT_MAIN_HAND)
                .and_then(|s| s.held.as_ref())
                .unwrap()
                .durability,
            0.99,
            "{tool_kind:?} hit should tick durability"
        );
        let durability_events = app
            .world()
            .resource::<Events<InventoryDurabilityChangedEvent>>();
        let events: Vec<_> = durability_events.iter_current_update_events().collect();
        assert_eq!(
            events.len(),
            1,
            "{tool_kind:?} should emit one durability event"
        );
        assert_eq!(events[0].entity, tool_user);
        assert_eq!(events[0].instance_id, 130 + index as u64);
        assert_eq!(events[0].durability, 0.99);
    }
}

#[test]
fn broken_tool_main_hand_uses_unarmed_baseline() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1431 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<WeaponBroken>();
    app.add_event::<ShieldBroken>();
    app.add_event::<ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(
        Update,
        (
            crate::combat::status::attribute_aggregate_tick,
            resolve_attack_intents,
        ),
    );

    let broken_tool_user = spawn_player(
        &mut app,
        "BrokenToolUser",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut()
        .entity_mut(broken_tool_user)
        .insert(PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                quick_access: false,
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![],
                owner_instance_id: None,
            }],
            equipped: std::collections::HashMap::from([(
                crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
                crate::inventory::SlotContents::held_single(ItemInstance {
                    instance_id: 131,
                    template_id: "cao_lian".to_string(),
                    display_name: "草镰".to_string(),
                    grid_w: 1,
                    grid_h: 2,
                    weight: 0.9,
                    rarity: crate::inventory::ItemRarity::Common,
                    description: String::new(),
                    stack_count: 1,
                    spirit_quality: 0.0,
                    durability: 0.0,
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
        });
    let unarmed = spawn_player(
        &mut app,
        "UnarmedPeer",
        [0.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );
    let broken_tool_target = spawn_player(
        &mut app,
        "BrokenToolTarget",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let unarmed_target = spawn_player(
        &mut app,
        "UnarmedPeerTarget",
        [1.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.update();

    app.world_mut().send_event(AttackIntent {
        attacker: broken_tool_user,
        target: Some(broken_tool_target),
        issued_at_tick: 1430,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.world_mut().send_event(AttackIntent {
        attacker: unarmed,
        target: Some(unarmed_target),
        issued_at_tick: 1430,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let combat_events = app.world().resource::<Events<CombatEvent>>();
    let events: Vec<_> = combat_events.iter_current_update_events().collect();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].damage, events[1].damage);

    let durability_events = app
        .world()
        .resource::<Events<InventoryDurabilityChangedEvent>>();
    assert_eq!(durability_events.iter_current_update_events().count(), 0);
}

// 耐久归零后 Weapon component 被移除 + WeaponBroken 事件发出。
#[test]
fn weapon_breaks_after_durability_depleted() {
    use crate::combat::weapon::{Weapon, WeaponKind};
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1500 });
    app.insert_resource(weapon_test_registry());
    app.insert_resource(DroppedLootRegistry::default());
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<WeaponBroken>();
    app.add_event::<ShieldBroken>();
    app.add_event::<ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(
        Update,
        (
            crate::combat::status::attribute_aggregate_tick,
            resolve_attack_intents,
        ),
    );

    let attacker = spawn_player(
        &mut app,
        "FragileSwordsman",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut()
        .entity_mut(attacker)
        .insert(PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                quick_access: false,
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![],
                owner_instance_id: None,
            }],
            equipped: std::collections::HashMap::from([(
                crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
                crate::inventory::SlotContents::held_single(ItemInstance {
                    instance_id: 42,
                    template_id: "glass_sword".to_string(),
                    display_name: "玻璃剑".to_string(),
                    grid_w: 1,
                    grid_h: 2,
                    weight: 1.0,
                    rarity: crate::inventory::ItemRarity::Common,
                    description: String::new(),
                    stack_count: 1,
                    spirit_quality: 1.0,
                    durability: 0.04,
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
        });
    let target = spawn_player(
        &mut app,
        "Dummy",
        [1.0, 64.0, 0.0],
        Wounds {
            health_current: 1000.0, // 防止先死
            health_max: 1000.0,
            ..Wounds::default()
        },
        Stamina::default(),
    );
    // 脆武器:只剩 0.4 耐久,一击即破(HIT_DURABILITY_COST = 0.5)
    app.world_mut().entity_mut(attacker).insert(Weapon {
        slot: crate::combat::weapon::EquipSlot::MainHand,
        instance_id: 42,
        template_id: "glass_sword".to_string(),
        weapon_kind: WeaponKind::Sword,
        base_attack: 10.0,
        quality_tier: 0,
        durability: 0.4,
        durability_max: 10.0,
    });

    app.update();

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 1499,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    // Weapon component 已被移除
    assert!(
        app.world().entity(attacker).get::<Weapon>().is_none(),
        "Weapon removed after durability depleted"
    );
    // WeaponBroken event 发出
    let broken_events = app.world().resource::<Events<WeaponBroken>>();
    let events: Vec<_> = broken_events.iter_current_update_events().collect();
    assert_eq!(events.len(), 1, "one WeaponBroken emitted");
    assert_eq!(events[0].instance_id, 42);
    assert_eq!(events[0].template_id, "glass_sword");

    let inventory = app
        .world()
        .entity(attacker)
        .get::<PlayerInventory>()
        .unwrap();
    assert!(
        inventory
            .equipped
            .get(crate::inventory::EQUIP_SLOT_MAIN_HAND)
            .and_then(|s| s.held.as_ref())
            .is_none(),
        "broken weapon should leave the equip slot"
    );
    assert_eq!(inventory.containers[0].items.len(), 1);
    assert_eq!(inventory.containers[0].items[0].instance.instance_id, 42);
    assert_eq!(inventory.containers[0].items[0].instance.durability, 0.0);
}

#[test]
fn broken_weapon_drops_when_no_container_slot_is_available() {
    use crate::combat::weapon::{Weapon, WeaponKind};
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1600 });
    app.insert_resource(weapon_test_registry());
    app.insert_resource(DroppedLootRegistry::default());
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<WeaponBroken>();
    app.add_event::<ShieldBroken>();
    app.add_event::<ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(
        Update,
        (
            crate::combat::status::attribute_aggregate_tick,
            resolve_attack_intents,
        ),
    );

    let attacker = spawn_player(
        &mut app,
        "PackedSwordsman",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut()
        .entity_mut(attacker)
        .insert(PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                quick_access: false,
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 1,
                cols: 1,
                items: vec![crate::inventory::PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: ItemInstance {
                        instance_id: 7,
                        template_id: "junk_stone".to_string(),
                        display_name: "碎石".to_string(),
                        grid_w: 1,
                        grid_h: 1,
                        weight: 1.0,
                        rarity: crate::inventory::ItemRarity::Common,
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
                    },
                }],

                owner_instance_id: None,
            }],
            equipped: std::collections::HashMap::from([(
                crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
                crate::inventory::SlotContents::held_single(ItemInstance {
                    instance_id: 42,
                    template_id: "glass_sword".to_string(),
                    display_name: "玻璃剑".to_string(),
                    grid_w: 1,
                    grid_h: 2,
                    weight: 1.0,
                    rarity: crate::inventory::ItemRarity::Common,
                    description: String::new(),
                    stack_count: 1,
                    spirit_quality: 1.0,
                    durability: 0.04,
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
        });
    let target = spawn_player(
        &mut app,
        "PackedDummy",
        [1.0, 64.0, 0.0],
        Wounds {
            health_current: 1000.0,
            health_max: 1000.0,
            ..Wounds::default()
        },
        Stamina::default(),
    );
    app.world_mut().entity_mut(attacker).insert(Weapon {
        slot: crate::combat::weapon::EquipSlot::MainHand,
        instance_id: 42,
        template_id: "glass_sword".to_string(),
        weapon_kind: WeaponKind::Sword,
        base_attack: 10.0,
        quality_tier: 0,
        durability: 0.4,
        durability_max: 10.0,
    });

    app.update();

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 1599,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    assert!(
        app.world().entity(attacker).get::<Weapon>().is_none(),
        "Weapon removed after broken weapon falls back to dropped loot"
    );
    let inventory = app
        .world()
        .entity(attacker)
        .get::<PlayerInventory>()
        .unwrap();
    assert!(
        inventory
            .equipped
            .get(crate::inventory::EQUIP_SLOT_MAIN_HAND)
            .and_then(|s| s.held.as_ref())
            .is_none(),
        "broken weapon should leave the equip slot even when bag is full"
    );
    assert_eq!(inventory.containers[0].items.len(), 1);

    let dropped_registry = app.world().resource::<DroppedLootRegistry>();
    let dropped = dropped_registry
        .entries
        .get(&42)
        .expect("broken weapon should be registered as dropped loot");
    assert_eq!(dropped.instance_id, 42);
    assert_eq!(dropped.item.durability, 0.0);
}

#[test]
fn cut_and_blunt_hits_produce_different_bleed_and_crack_outputs() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1400 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let cut_attacker = spawn_player(
        &mut app,
        "CutUser",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let blunt_attacker = spawn_player(
        &mut app,
        "BluntUser",
        [0.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );
    let cut_target = spawn_player(
        &mut app,
        "CutTarget",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let blunt_target = spawn_player(
        &mut app,
        "BluntTarget",
        [1.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut().send_event(AttackIntent {
        attacker: cut_attacker,
        target: Some(cut_target),
        issued_at_tick: 1399,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Cut,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.world_mut().send_event(AttackIntent {
        attacker: blunt_attacker,
        target: Some(blunt_target),
        issued_at_tick: 1399,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let cut_target_ref = app.world().entity(cut_target);
    let blunt_target_ref = app.world().entity(blunt_target);
    let cut_wound = cut_target_ref
        .get::<Wounds>()
        .unwrap()
        .entries
        .last()
        .unwrap()
        .clone();
    let blunt_wound = blunt_target_ref
        .get::<Wounds>()
        .unwrap()
        .entries
        .last()
        .unwrap()
        .clone();
    let cut_crack = cut_target_ref
        .get::<MeridianSystem>()
        .unwrap()
        .get(MeridianId::Lung)
        .cracks
        .last()
        .unwrap()
        .clone();
    let blunt_crack = blunt_target_ref
        .get::<MeridianSystem>()
        .unwrap()
        .get(MeridianId::Lung)
        .cracks
        .last()
        .unwrap()
        .clone();

    assert!(cut_wound.bleeding_per_sec > blunt_wound.bleeding_per_sec);
    assert!(blunt_crack.severity > cut_crack.severity);
}

#[test]
fn pierce_hit_changes_contamination_output_against_blunt_baseline() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1500 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let pierce_attacker = spawn_player(
        &mut app,
        "PierceUser",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let blunt_attacker = spawn_player(
        &mut app,
        "BluntUser2",
        [0.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );
    let pierce_target = spawn_player(
        &mut app,
        "PierceTarget",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let blunt_target = spawn_player(
        &mut app,
        "BluntTarget2",
        [1.0, 64.0, 2.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut().send_event(AttackIntent {
        attacker: pierce_attacker,
        target: Some(pierce_target),
        issued_at_tick: 1499,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Pierce,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.world_mut().send_event(AttackIntent {
        attacker: blunt_attacker,
        target: Some(blunt_target),
        issued_at_tick: 1499,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let pierce_contam = app
        .world()
        .entity(pierce_target)
        .get::<Contamination>()
        .unwrap()
        .entries
        .last()
        .unwrap()
        .amount;
    let blunt_contam = app
        .world()
        .entity(blunt_target)
        .get::<Contamination>()
        .unwrap()
        .entries
        .last()
        .unwrap()
        .amount;

    assert!(pierce_contam > blunt_contam);
}

#[test]
fn zero_qi_sword_hit_resolves_physical_damage_without_contamination_or_meridian_crack() {
    use crate::combat::weapon::{EquipSlot, Weapon, WeaponKind};

    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1540 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<WeaponBroken>();
    app.add_event::<ShieldBroken>();
    app.add_event::<ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "ZeroQiSword",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut().entity_mut(attacker).insert((
        Weapon {
            slot: EquipSlot::MainHand,
            instance_id: 1540,
            template_id: "iron_sword".to_string(),
            weapon_kind: WeaponKind::Sword,
            base_attack: 12.0,
            quality_tier: 0,
            durability: 200.0,
            durability_max: 200.0,
        },
        KnownTechniques {
            entries: vec![KnownTechnique {
                id: sword_basics::SWORD_CLEAVE_SKILL_ID.to_string(),
                proficiency: 0.5,
                active: true,
            }],
        },
    ));
    let mut target_meridians = MeridianSystem::default();
    target_meridians.get_mut(MeridianId::Lung).opened = true;
    let target = spawn_player(
        &mut app,
        "ZeroQiTarget",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut().entity_mut(target).insert(target_meridians);

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 1539,
        reach: AttackReach::new(3.0, 0.0),
        qi_invest: 0.0,
        wound_kind: WoundKind::Cut,
        source: AttackSource::SwordCleave,
        debug_command: None,
    });

    app.update();

    let events: Vec<_> = app
        .world()
        .resource::<Events<CombatEvent>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].damage, 0.0);
    assert!(
        events[0].physical_damage > 0.0,
        "zero-qi sword hit should still land physical damage"
    );
    assert_eq!(events[0].contam_delta, 0.0);

    let target_ref = app.world().entity(target);
    assert!(
        target_ref
            .get::<Contamination>()
            .expect("target contamination")
            .entries
            .is_empty(),
        "physical branch must not introduce contamination"
    );
    let meridian = target_ref
        .get::<MeridianSystem>()
        .expect("target meridians")
        .get(MeridianId::Lung);
    assert_eq!(meridian.throughput_current, 0.0);
    assert!(
        meridian.cracks.is_empty(),
        "physical branch must not crack meridians"
    );
}

#[test]
fn sword_parry_blocks_physical_damage_reflects_and_staggers_attacker() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1541 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<WeaponBroken>();
    app.add_event::<ShieldBroken>();
    app.add_event::<ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "ParryAttacker",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let defender = spawn_player(
        &mut app,
        "ParryDefender",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut().entity_mut(defender).insert((
        StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::SwordParrying,
                magnitude: 0.5,
                remaining_ticks: 4,
                source_pill: None,
            }],
        },
        KnownTechniques {
            entries: vec![KnownTechnique {
                id: sword_basics::SWORD_PARRY_SKILL_ID.to_string(),
                proficiency: 0.0,
                active: true,
            }],
        },
    ));

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(defender),
        issued_at_tick: 1540,
        reach: FIST_REACH,
        qi_invest: 0.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let events: Vec<_> = app
        .world()
        .resource::<Events<CombatEvent>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].defense_kind, Some(DefenseKind::SwordParry));
    assert_eq!(events[0].defense_effectiveness, Some(0.5));
    assert!(
        (events[0].physical_damage - 0.5).abs() < 0.001,
        "50% sword parry should halve the 1.0 unarmed physical hit"
    );

    let status_intents: Vec<_> = app
        .world()
        .resource::<Events<ApplyStatusEffectIntent>>()
        .iter_current_update_events()
        .collect();
    assert!(status_intents
        .iter()
        .any(|intent| intent.target == attacker && intent.kind == StatusEffectKind::Staggered));

    let attacker_wounds = app.world().entity(attacker).get::<Wounds>().unwrap();
    assert_eq!(attacker_wounds.entries.len(), 1);
    assert!(
        (attacker_wounds.entries[0].severity - 0.075).abs() < 0.001,
        "reflected physical damage should be 15% of blocked damage"
    );
    // plan-combat-hit-location-v1 P2（决议 §8.1 旁路桶 #1）—— 剑招招架反伤应命中
    // 攻方持械臂（MAIN_ARM = ArmR），而非旧实现里硬编的恒定 Chest。
    assert_eq!(
        attacker_wounds.entries[0].location,
        crate::body_plan::legacy_body_part_to_id(crate::combat::arm_wound::MAIN_ARM),
        "招架反伤应命中攻方持械臂（ArmR），实测 {:?}；若这里变回 Chest 说明 P2 \
             反伤旁路清理被回退了",
        attacker_wounds.entries[0].location
    );

    let known = app
        .world()
        .entity(defender)
        .get::<KnownTechniques>()
        .unwrap();
    assert!(
        known.entries[0].proficiency > 0.0,
        "successful parry should raise sword.parry proficiency"
    );
}

// ══════════════════════════════════════════════════════════════════════════
// plan-combat-hit-location-v1 P1（决议 §8.1 #2）— 臂伤消费端集成测试
// ══════════════════════════════════════════════════════════════════════════

fn make_arm_wound_app() -> App {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 9000 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<WeaponBroken>();
    app.add_event::<ShieldBroken>();
    app.add_event::<ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);
    app
}

fn wound(location: BodyPart, severity: f32) -> Wound {
    Wound {
        location: crate::body_plan::legacy_body_part_to_id(location),
        kind: WoundKind::Blunt,
        severity,
        bleeding_per_sec: 0.0,
        created_at_tick: 0,
        inflicted_by: None,
    }
}

fn attacker_weapon() -> Weapon {
    use crate::combat::weapon::{EquipSlot, WeaponKind};
    Weapon {
        slot: EquipSlot::MainHand,
        instance_id: 7001,
        template_id: "iron_sword".to_string(),
        weapon_kind: WeaponKind::Sword,
        base_attack: 20.0,
        quality_tier: 0,
        durability: 200.0,
        durability_max: 200.0,
    }
}

// ── 断臂脱手落地：命中几何 helper ────────────────────────────────────────────
//
// `resolve_attack_intents` 的命中部位完全由 `raycast_humanoid` + 攻方真实瞄准
// 方向决定（决议 §8.1 #1，无恒瞄 fallback），要在集成测试里稳定命中 ArmL/ArmR
// 就必须真算一条精确打在目标 AABB 侧面臂区（`classify_body_part`：
// rel_y ∈ (0.55, 0.88]、|lateral| > 0.19）的射线，而非依赖概率 jitter。
// 下面两个 helper 把这条射线的目标点固定在 target_feet=(0,64,2.0) 正前方
// AABB 的 z_min 侧面（半宽 0.3，取 x=±0.29 贴近边缘换取最大 lateral 裕度，
// y=65.3 → rel_y=1.3/1.8≈0.72 落在臂/胸带正中），再用 `Look::set_vec` 精确
// 反推攻方 yaw/pitch——不经手工 yaw/pitch 三角函数换算，规避 `Look` 与内部
// `direction_to_yaw_pitch` 约定不一致的坑（见 raycast.rs 该函数注释）。
fn arm_hit_target_feet() -> DVec3 {
    DVec3::new(0.0, 64.0, 2.0)
}

/// `is_main_arm_side`：`true` 对应 `arm_wound::MAIN_ARM`(ArmR，lateral>0，
/// x 取负号，见 `classify_body_part` 叉乘推导)；`false` 对应 `arm_wound::OFF_ARM`
/// (ArmL，lateral<0，x 取正号)。两侧共享同一 y/z，只有 x 符号相反。
fn arm_hit_point(is_main_arm_side: bool) -> DVec3 {
    let x = if is_main_arm_side { -0.29 } else { 0.29 };
    DVec3::new(x, 65.3, 1.7)
}

/// 攻方脚下坐标固定在 `(0,64,0)`，与 `arm_hit_target_feet()` 沿 +Z 相距 2 格
/// （在任意 reach≥2 的攻击类型下都能命中，含 FIST_REACH=2.6）。
fn arm_hit_attacker_feet() -> [f64; 3] {
    [0.0, 64.0, 0.0]
}

/// 由攻方脚下坐标 + 目标命中点反推一个恰好瞄准该点的 `Look`（yaw/pitch 由
/// `Look::set_vec` 从归一化方向向量反解，非手工三角函数）。
fn aim_look_at_point(attacker_feet: DVec3, hit_point: DVec3) -> Look {
    let eye = attacker_feet + DVec3::new(0.0, ATTACKER_EYE_HEIGHT, 0.0);
    let dir = (hit_point - eye).normalize();
    let mut look = Look::default();
    look.set_vec(valence::prelude::Vec3::new(dir.x as f32, dir.y as f32, dir.z as f32).normalize());
    look
}

/// 决议 §8.1 #2：攻方主手臂（ArmR）Fracture 伤势应把自身物理攻击伤害削减到
/// 健康臂的 0.60 倍（`arm_wound::attack_damage_multiplier(Fracture)`）。
/// 用 fist 攻击（base=1.0）会撞 `damage.max(1.0)` 下限而看不出差异，
/// 因此用高基础伤害武器（base_attack=20.0）跑两遍攻击对比比例。
#[test]
fn main_arm_fracture_reduces_attacker_physical_damage_by_decision_table_ratio() {
    let mut app = make_arm_wound_app();

    let healthy_attacker = spawn_player(
        &mut app,
        "ArmHealthyAtk",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut()
        .entity_mut(healthy_attacker)
        .insert(attacker_weapon());
    let healthy_target = spawn_player(
        &mut app,
        "ArmHealthyTarget",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut().send_event(AttackIntent {
        attacker: healthy_attacker,
        target: Some(healthy_target),
        issued_at_tick: 8999,
        reach: AttackReach::new(3.0, 0.0),
        qi_invest: 0.0,
        wound_kind: WoundKind::Cut,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let healthy_damage = app
        .world()
        .resource::<Events<CombatEvent>>()
        .iter_current_update_events()
        .next()
        .expect("healthy attacker hit should resolve")
        .physical_damage;
    assert!(
            healthy_damage > 1.0,
            "测试前提：健康臂伤害必须高于 damage.max(1.0) 下限才能观测到削减比例，实际 {healthy_damage}"
        );

    let mut app2 = make_arm_wound_app();
    let wounded_attacker = spawn_player(
        &mut app2,
        "ArmFracturedAtk",
        [0.0, 64.0, 0.0],
        Wounds {
            entries: vec![wound(arm_wound::MAIN_ARM, 40.0)], // Fracture
            ..Default::default()
        },
        Stamina::default(),
    );
    app2.world_mut()
        .entity_mut(wounded_attacker)
        .insert(attacker_weapon());
    let wounded_target = spawn_player(
        &mut app2,
        "ArmFracturedTarget",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    app2.world_mut().send_event(AttackIntent {
        attacker: wounded_attacker,
        target: Some(wounded_target),
        issued_at_tick: 8999,
        reach: AttackReach::new(3.0, 0.0),
        qi_invest: 0.0,
        wound_kind: WoundKind::Cut,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app2.update();

    let wounded_damage = app2
        .world()
        .resource::<Events<CombatEvent>>()
        .iter_current_update_events()
        .next()
        .expect("wounded attacker hit should resolve")
        .physical_damage;

    let ratio = wounded_damage / healthy_damage;
    assert!(
        (ratio - 0.60).abs() < 0.01,
        "主手臂 Fracture 应把攻击伤害削减到健康臂的 0.60 倍，实际比例 {ratio:.4}\
             （healthy={healthy_damage}, wounded={wounded_damage}）"
    );
}

/// 决议 §8.1 #2：攻方副手臂（ArmL）受伤不应影响自身攻击伤害（该维度只读主手臂）。
#[test]
fn off_arm_wound_does_not_affect_attacker_physical_damage() {
    let mut app = make_arm_wound_app();
    let attacker = spawn_player(
        &mut app,
        "OffArmWoundedAtk",
        [0.0, 64.0, 0.0],
        Wounds {
            entries: vec![wound(arm_wound::OFF_ARM, 80.0)], // Severed on OFF_ARM
            ..Default::default()
        },
        Stamina::default(),
    );
    app.world_mut()
        .entity_mut(attacker)
        .insert(attacker_weapon());
    let target = spawn_player(
        &mut app,
        "OffArmWoundedTarget",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 8999,
        reach: AttackReach::new(3.0, 0.0),
        qi_invest: 0.0,
        wound_kind: WoundKind::Cut,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let damage = app
        .world()
        .resource::<Events<CombatEvent>>()
        .iter_current_update_events()
        .next()
        .expect("attack should resolve")
        .physical_damage;
    // 副手臂 Severed 不应削减攻击伤害：20.0(base) * 1.0(chest) * 1.0(attacker attrs,
    // 无 arm 惩罚) * 1.0(defender) * weapon.damage_multiplier() * 1.0(wound_profile) * 1.0(sword)。
    let expected = 20.0 * attacker_weapon().damage_multiplier();
    assert!(
        (damage - expected).abs() < 0.01,
        "副手臂受伤不应影响攻击伤害维度，期望≈{expected}，实际 {damage}"
    );
}

/// 决议 §8.1 #2：防御方副手臂（ArmL，持盾侧）Fracture 伤势应把招架（SwordParrying）
/// 减伤效果打到 0.60 倍（`arm_wound::block_multiplier(Fracture)`）——
/// base block_ratio=0.5 → 实际生效 0.5*0.60=0.30。
#[test]
fn sword_parry_off_arm_fracture_reduces_defense_effectiveness() {
    let mut app = make_arm_wound_app();

    let attacker = spawn_player(
        &mut app,
        "ParryAtkP1",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let defender = spawn_player(
        &mut app,
        "ParryDefP1",
        [1.0, 64.0, 0.0],
        Wounds {
            entries: vec![wound(arm_wound::OFF_ARM, 40.0)], // Fracture on OFF_ARM
            ..Default::default()
        },
        Stamina::default(),
    );
    app.world_mut().entity_mut(defender).insert((
        StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::SwordParrying,
                magnitude: 0.5,
                remaining_ticks: 4,
                source_pill: None,
            }],
        },
        KnownTechniques {
            entries: vec![KnownTechnique {
                id: sword_basics::SWORD_PARRY_SKILL_ID.to_string(),
                proficiency: 0.0,
                active: true,
            }],
        },
    ));

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(defender),
        issued_at_tick: 8999,
        reach: FIST_REACH,
        qi_invest: 0.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let events: Vec<_> = app
        .world()
        .resource::<Events<CombatEvent>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(events.len(), 1);
    assert!(
        events[0]
            .defense_effectiveness
            .is_some_and(|e| (e - 0.30).abs() < 0.001),
        "副手臂 Fracture 应把 0.5 base block_ratio 打到 0.30（×0.60），实际 {:?}",
        events[0].defense_effectiveness
    );
    assert!(
        (events[0].physical_damage - 0.70).abs() < 0.01,
        "1.0 unarmed hit 经打折后的招架（0.30 减伤）应剩 0.70，实际 {}",
        events[0].physical_damage
    );
}

/// 决议 §8.1 #2：防御方副手臂（ArmL，持盾侧）Fracture 伤势应把盾牌格挡减伤效果
/// 打到 0.60 倍——bone_shield base block_ratio=0.65 → 实际生效 0.65*0.60=0.39。
#[test]
fn shield_block_off_arm_fracture_reduces_defense_effectiveness() {
    let mut app = make_arm_wound_app();

    let attacker = spawn_player(
        &mut app,
        "ShieldAtkP1",
        [0.0, 64.0, 3.0],
        Wounds::default(),
        Stamina::default(),
    );
    let defender = spawn_player(
        &mut app,
        "ShieldDefP1",
        [0.0, 64.0, 1.0],
        Wounds {
            entries: vec![wound(arm_wound::OFF_ARM, 40.0)], // Fracture on OFF_ARM
            ..Default::default()
        },
        Stamina::default(),
    );
    app.world_mut().entity_mut(defender).insert((
        StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::ShieldBlocking,
                magnitude: 0.65,
                remaining_ticks: crate::combat::shield_block::SHIELD_BLOCKING_DURATION_TICKS,
                source_pill: None,
            }],
        },
        crate::combat::shield_block::ShieldBlock {
            template_id: "bone_shield".to_string(),
        },
        Look {
            yaw: 0.0,
            pitch: 0.0,
        },
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(1),
            containers: vec![],
            equipped: std::collections::HashMap::from([(
                EQUIP_SLOT_OFF_HAND.to_string(),
                crate::inventory::SlotContents::held_single(ItemInstance {
                    instance_id: 9101,
                    template_id: "bone_shield".to_string(),
                    display_name: "骨盾".to_string(),
                    grid_w: 1,
                    grid_h: 2,
                    weight: 2.5,
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
                }),
            )]),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        },
    ));

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(defender),
        issued_at_tick: 8999,
        reach: FIST_REACH,
        qi_invest: 0.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let events: Vec<_> = app
        .world()
        .resource::<Events<CombatEvent>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(events.len(), 1);
    assert!(
        events[0]
            .defense_effectiveness
            .is_some_and(|e| (e - 0.39).abs() < 0.001),
        "副手臂 Fracture 应把 bone_shield 0.65 base block_ratio 打到 0.39（×0.60），实际 {:?}",
        events[0].defense_effectiveness
    );
}

/// 决议 §8.1 #2：双臂皆伤时格挡惩罚只读副手臂，不因主手臂也受伤而叠乘更狠
/// （与攻击伤害维度只读主手臂互相独立，二者不交叉污染）。
#[test]
fn both_arms_wounded_block_penalty_reads_only_off_arm_not_multiplied() {
    let mut app = make_arm_wound_app();
    let attacker = spawn_player(
        &mut app,
        "BothArmsAtk",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let defender = spawn_player(
        &mut app,
        "BothArmsDef",
        [1.0, 64.0, 0.0],
        Wounds {
            entries: vec![
                wound(arm_wound::MAIN_ARM, 40.0), // Fracture on MAIN_ARM
                wound(arm_wound::OFF_ARM, 40.0),  // Fracture on OFF_ARM
            ],
            ..Default::default()
        },
        Stamina::default(),
    );
    app.world_mut().entity_mut(defender).insert((
        StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::SwordParrying,
                magnitude: 0.5,
                remaining_ticks: 4,
                source_pill: None,
            }],
        },
        KnownTechniques {
            entries: vec![KnownTechnique {
                id: sword_basics::SWORD_PARRY_SKILL_ID.to_string(),
                proficiency: 0.0,
                active: true,
            }],
        },
    ));

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(defender),
        issued_at_tick: 8999,
        reach: FIST_REACH,
        qi_invest: 0.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let events: Vec<_> = app
        .world()
        .resource::<Events<CombatEvent>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(events.len(), 1);
    assert!(
            events[0]
                .defense_effectiveness
                .is_some_and(|e| (e - 0.30).abs() < 0.001),
            "双臂皆 Fracture 时格挡效果应仍是 0.5*0.60=0.30（只读副手臂），不应叠乘为 0.5*0.60*0.60=0.18，\
             实际 {:?}",
            events[0].defense_effectiveness
        );
}

// ══════════════════════════════════════════════════════════════════════════
// plan-combat-hit-location-v1 P4（决议 §8.1 #2 Severed 行为级后果 #1）——
// 断臂脱手落地 pin 测试：消除 `ArmWoundFactors.main_arm_severed` 零消费孤岛。
// ══════════════════════════════════════════════════════════════════════════

/// 造一把挂在 `target` main_hand 槽（inventory 侧 + `Weapon` runtime component
/// 双侧一致）的武器，供断臂脱手测试断言"武器消失于持械槽、出现于世界掉落"。
fn equip_main_hand_weapon(app: &mut App, target: Entity, instance_id: u64) {
    use crate::combat::weapon::{EquipSlot, WeaponKind};
    app.world_mut().entity_mut(target).insert((
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(1),
            containers: vec![],
            equipped: std::collections::HashMap::from([(
                crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
                crate::inventory::SlotContents::held_single(ItemInstance {
                    instance_id,
                    template_id: "iron_sword".to_string(),
                    display_name: "铁剑".to_string(),
                    grid_w: 1,
                    grid_h: 2,
                    weight: 1.2,
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
        Weapon {
            slot: EquipSlot::MainHand,
            instance_id,
            template_id: "iron_sword".to_string(),
            weapon_kind: WeaponKind::Sword,
            base_attack: 20.0,
            quality_tier: 0,
            durability: 200.0,
            durability_max: 200.0,
        },
    ));
}

/// 决议 §8.1 #2 行为级后果 ①：主手臂(MAIN_ARM=ArmR)单次命中直接判定 Severed
/// （severity>=70）→ 该侧持械立即脱手，走 `DroppedLootRegistry` 世界掉落。
/// 三重断言对应任务书原文三点：脱手落地事件（registry 新增条目）/ 持械槽清空 /
/// 掉落物守恒（instance_id 与武器本体完整进入世界，不丢失也不复制）。
#[test]
fn main_arm_severed_hit_drops_weapon_into_world_and_clears_equip_slot() {
    let mut app = make_arm_wound_app();
    app.insert_resource(weapon_test_registry());
    app.insert_resource(DroppedLootRegistry::default());

    let attacker = spawn_player(
        &mut app,
        "SeveredAtk",
        arm_hit_attacker_feet(),
        Wounds::default(),
        Stamina::default(),
    );
    // attack_power=150 * body_part_multiplier(ArmR)=0.7 → base_damage=105，
    // 稳超 Severed 阈值 70.0（留足浮点/geometry 裕度）。
    app.world_mut().entity_mut(attacker).insert(DerivedAttrs {
        attack_power: 150.0,
        ..DerivedAttrs::default()
    });
    let target_feet = arm_hit_target_feet();
    let target = spawn_player(
        &mut app,
        "SeveredDef",
        [target_feet.x, target_feet.y, target_feet.z],
        Wounds {
            health_current: 1000.0,
            health_max: 1000.0,
            ..Wounds::default()
        },
        Stamina::default(),
    );
    equip_main_hand_weapon(&mut app, target, 9301);

    let look = aim_look_at_point(
        DVec3::new(
            arm_hit_attacker_feet()[0],
            arm_hit_attacker_feet()[1],
            arm_hit_attacker_feet()[2],
        ),
        arm_hit_point(true),
    );
    app.world_mut().entity_mut(attacker).insert(look);

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 9000,
        reach: AttackReach::new(4.0, 0.0),
        qi_invest: 0.0, // physical path：伤害不经 qi 闸门/距离 decay，只看武器/属性
        wound_kind: WoundKind::Cut,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let target_wounds = app.world().entity(target).get::<Wounds>().unwrap();
    assert_eq!(
        target_wounds.entries.last().map(|w| w.location.clone()),
        Some(crate::body_plan::legacy_body_part_to_id(
            arm_wound::MAIN_ARM
        )),
        "geometry helper 应命中 MAIN_ARM(ArmR)，实际 {:?}；若此断言先撞红说明射线\
             geometry 算错了，后续脱手断言无意义",
        target_wounds.entries.last().map(|w| w.location.clone())
    );
    assert_eq!(
        arm_wound::worst_wound_grade(
            target_wounds,
            &crate::body_plan::legacy_body_part_to_id(arm_wound::MAIN_ARM)
        ),
        arm_wound::ArmWoundGrade::Severed,
        "本次命中伤势应达到 Severed 分级，实际最重分级 {:?}（severity={:?}）；\
             断臂脱手的前置条件未达成",
        arm_wound::worst_wound_grade(
            target_wounds,
            &crate::body_plan::legacy_body_part_to_id(arm_wound::MAIN_ARM)
        ),
        target_wounds.entries.last().map(|w| w.severity)
    );

    // ① 持械槽清空：Weapon runtime component 已移除。
    assert!(
        app.world().entity(target).get::<Weapon>().is_none(),
        "主手臂 Severed 后 target 的 Weapon component 应被 remove（脱手）"
    );
    // ① 持械槽清空：inventory 侧 main_hand.held 同步清空（不是只删 runtime 影子）。
    let inventory = app.world().entity(target).get::<PlayerInventory>().unwrap();
    assert!(
        inventory
            .equipped
            .get(crate::inventory::EQUIP_SLOT_MAIN_HAND)
            .and_then(|s| s.held.as_ref())
            .is_none(),
        "断臂脱手应清空 inventory 侧 main_hand 持握槽，而不仅仅移除 runtime Weapon component"
    );
    // ① 掉落物守恒：武器整体（instance_id 不变）进入世界 DroppedLootRegistry，
    // 既不凭空消失也不复制。
    let dropped_registry = app.world().resource::<DroppedLootRegistry>();
    let dropped = dropped_registry.entries.get(&9301).expect(
        "断臂脱手的武器 instance 应出现在 DroppedLootRegistry（世界掉落），\
                     而不是被静默丢弃",
    );
    assert_eq!(dropped.instance_id, 9301);
    assert_eq!(
        dropped.item.template_id, "iron_sword",
        "掉落物应是原封不动的同一把剑，template_id 不应在脱手过程中被篡改"
    );
}

/// 决议 §8.1 #2：副手臂(OFF_ARM=ArmL) Severed **不**触发脱手——脱手落地只针对
/// 持械侧（主手臂）。若这条误连到 OFF_ARM，副手完好的持械手会被无理由缴械。
#[test]
fn off_arm_severed_hit_does_not_drop_main_hand_weapon() {
    let mut app = make_arm_wound_app();
    app.insert_resource(weapon_test_registry());
    app.insert_resource(DroppedLootRegistry::default());

    let attacker = spawn_player(
        &mut app,
        "OffSeveredAtk",
        arm_hit_attacker_feet(),
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut().entity_mut(attacker).insert(DerivedAttrs {
        attack_power: 150.0,
        ..DerivedAttrs::default()
    });
    let target_feet = arm_hit_target_feet();
    let target = spawn_player(
        &mut app,
        "OffSeveredDef",
        [target_feet.x, target_feet.y, target_feet.z],
        Wounds {
            health_current: 1000.0,
            health_max: 1000.0,
            ..Wounds::default()
        },
        Stamina::default(),
    );
    equip_main_hand_weapon(&mut app, target, 9302);

    let look = aim_look_at_point(
        DVec3::new(
            arm_hit_attacker_feet()[0],
            arm_hit_attacker_feet()[1],
            arm_hit_attacker_feet()[2],
        ),
        arm_hit_point(false), // false = OFF_ARM(ArmL) 侧
    );
    app.world_mut().entity_mut(attacker).insert(look);

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 9001,
        reach: AttackReach::new(4.0, 0.0),
        qi_invest: 0.0,
        wound_kind: WoundKind::Cut,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let target_wounds = app.world().entity(target).get::<Wounds>().unwrap();
    assert_eq!(
        target_wounds.entries.last().map(|w| w.location.clone()),
        Some(crate::body_plan::legacy_body_part_to_id(arm_wound::OFF_ARM)),
        "geometry helper 应命中 OFF_ARM(ArmL)，实际 {:?}",
        target_wounds.entries.last().map(|w| w.location.clone())
    );
    assert_eq!(
        arm_wound::worst_wound_grade(
            target_wounds,
            &crate::body_plan::legacy_body_part_to_id(arm_wound::OFF_ARM)
        ),
        arm_wound::ArmWoundGrade::Severed,
        "本次命中应达到 Severed 分级才能验证副手断裂不触发脱手这件事，实际 {:?}",
        arm_wound::worst_wound_grade(
            target_wounds,
            &crate::body_plan::legacy_body_part_to_id(arm_wound::OFF_ARM)
        )
    );

    assert!(
        app.world().entity(target).get::<Weapon>().is_some(),
        "副手臂(OFF_ARM) Severed 不应移除主手武器 Weapon component"
    );
    let inventory = app.world().entity(target).get::<PlayerInventory>().unwrap();
    assert!(
        inventory
            .equipped
            .get(crate::inventory::EQUIP_SLOT_MAIN_HAND)
            .and_then(|s| s.held.as_ref())
            .is_some(),
        "副手臂 Severed 时主手武器应仍留在装备槽（脱手只认主手臂）"
    );
    assert!(
        !app.world()
            .resource::<DroppedLootRegistry>()
            .entries
            .contains_key(&9302),
        "副手臂断裂不应把主手武器送进世界掉落"
    );
}

/// 边界：主手臂命中但伤势只到 Fracture（未达 Severed 阈值 70.0）不应触发脱手——
/// 锁住"只有 Severed 才脱手"这条边界，防止阈值判定被误改成 `>=` Fracture 起就掉。
#[test]
fn main_arm_fracture_grade_hit_does_not_trigger_weapon_drop() {
    let mut app = make_arm_wound_app();
    app.insert_resource(weapon_test_registry());
    app.insert_resource(DroppedLootRegistry::default());

    let attacker = spawn_player(
        &mut app,
        "FractureAtk",
        arm_hit_attacker_feet(),
        Wounds::default(),
        Stamina::default(),
    );
    // attack_power=20 * body_part_multiplier(ArmR)=0.7 → base_damage=14.0，
    // 落在 Fracture 区间 [35,70) 之外偏低（Laceration 区间 [15,35)），
    // 无论如何都远低于 Severed 阈值 70.0，且不会意外撞进 Severed。
    app.world_mut().entity_mut(attacker).insert(DerivedAttrs {
        attack_power: 20.0,
        ..DerivedAttrs::default()
    });
    let target_feet = arm_hit_target_feet();
    let target = spawn_player(
        &mut app,
        "FractureDef",
        [target_feet.x, target_feet.y, target_feet.z],
        Wounds {
            health_current: 1000.0,
            health_max: 1000.0,
            ..Wounds::default()
        },
        Stamina::default(),
    );
    equip_main_hand_weapon(&mut app, target, 9303);

    let look = aim_look_at_point(
        DVec3::new(
            arm_hit_attacker_feet()[0],
            arm_hit_attacker_feet()[1],
            arm_hit_attacker_feet()[2],
        ),
        arm_hit_point(true),
    );
    app.world_mut().entity_mut(attacker).insert(look);

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 9002,
        reach: AttackReach::new(4.0, 0.0),
        qi_invest: 0.0,
        wound_kind: WoundKind::Cut,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let target_wounds = app.world().entity(target).get::<Wounds>().unwrap();
    assert_eq!(
        target_wounds.entries.last().map(|w| w.location.clone()),
        Some(crate::body_plan::legacy_body_part_to_id(
            arm_wound::MAIN_ARM
        ))
    );
    assert_ne!(
        arm_wound::worst_wound_grade(
            target_wounds,
            &crate::body_plan::legacy_body_part_to_id(arm_wound::MAIN_ARM)
        ),
        arm_wound::ArmWoundGrade::Severed,
        "本测试要验证的是 Severed 以下分级不脱手，前置条件要求本次命中不能是 Severed；\
             实际却是 Severed，说明伤害计算改动了，该测试需要重新校准 attack_power"
    );

    assert!(
        app.world().entity(target).get::<Weapon>().is_some(),
        "未达 Severed 分级不应移除 Weapon component"
    );
    assert!(
        !app.world()
            .resource::<DroppedLootRegistry>()
            .entries
            .contains_key(&9303),
        "未达 Severed 分级不应把武器送进世界掉落"
    );
}

#[test]
fn burst_meridian_attack_source_uses_prepaid_qi_without_second_spend() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1550 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "BurstUser",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut().entity_mut(attacker).insert(Cultivation {
        qi_current: 60.0,
        qi_max: 100.0,
        ..Cultivation::default()
    });
    let target = spawn_player(
        &mut app,
        "BurstTarget",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 1549,
        reach: FIST_REACH,
        qi_invest: 80.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::BurstMeridian,
        debug_command: None,
    });

    app.update();

    assert_eq!(
        app.world()
            .entity(attacker)
            .get::<Cultivation>()
            .unwrap()
            .qi_current,
        60.0,
        "BurstMeridian source is already paid by skill resolver and must not spend qi again"
    );
    assert!(
        !app.world().resource::<Events<CombatEvent>>().is_empty(),
        "prepaid burst attack should still resolve even when qi_invest exceeds remaining qi"
    );
}

#[test]
fn full_power_attack_source_uses_prepaid_qi_without_second_spend() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1550 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "FullPowerUser",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut().entity_mut(attacker).insert(Cultivation {
        qi_current: 60.0,
        qi_max: 100.0,
        ..Cultivation::default()
    });
    let target = spawn_player(
        &mut app,
        "FullPowerTarget",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 1549,
        reach: FIST_REACH,
        qi_invest: 80.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::FullPower,
        debug_command: None,
    });

    app.update();

    assert_eq!(
        app.world()
            .entity(attacker)
            .get::<Cultivation>()
            .unwrap()
            .qi_current,
        60.0,
        "FullPower source is already paid by release handler and must not spend qi again"
    );
    assert!(
        !app.world().resource::<Events<CombatEvent>>().is_empty(),
        "prepaid full power attack should still resolve when qi_invest exceeds remaining qi"
    );
}

/// 端到端验证 NPC↔NPC 互殴走 shared resolver：使用 `npc_runtime_bundle`
/// 的真实形态（**无 LifeRecord**）双方交叉 `AttackIntent`，断言 Wounds
/// 写入 + 致命伤触发 DeathEvent。既有测试用 test-only helper 挂了
/// LifeRecord，未代表生产形态；本测试补齐。
#[test]
fn npc_to_npc_duel_via_runtime_bundle_resolves_damage_and_death() {
    use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};

    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 200 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    // 两个 NPC 用真实生产 bundle，无 LifeRecord。
    let npc_a = app
        .world_mut()
        .spawn((NpcMarker, Position::new([0.0, 64.0, 0.0])))
        .id();
    let mut bundle_a = npc_runtime_bundle(npc_a, NpcArchetype::Rogue, Realm::Awaken);
    // 让 A 血量濒死以便单击致命；qi 注满以过 resolver 的 qi_invest 检查。
    bundle_a.wounds = Wounds {
        health_current: 3.0,
        health_max: 100.0,
        entries: Vec::new(),
    };
    bundle_a.cultivation.qi_current = 80.0;
    bundle_a.cultivation.qi_max = 100.0;
    app.world_mut().entity_mut(npc_a).insert(bundle_a);

    let npc_b = app
        .world_mut()
        .spawn((NpcMarker, Position::new([1.0, 64.0, 0.0])))
        .id();
    let mut bundle_b = npc_runtime_bundle(npc_b, NpcArchetype::Zombie, Realm::Awaken);
    bundle_b.cultivation.qi_current = 80.0;
    bundle_b.cultivation.qi_max = 100.0;
    app.world_mut().entity_mut(npc_b).insert(bundle_b);

    // 双向 AttackIntent：A 打 B 一下（非致命），B 打 A 一下（致命）。
    app.world_mut().send_event(AttackIntent {
        attacker: npc_a,
        target: Some(npc_b),
        issued_at_tick: 199,
        reach: FIST_REACH,
        qi_invest: 8.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.world_mut().send_event(AttackIntent {
        attacker: npc_b,
        target: Some(npc_a),
        issued_at_tick: 199,
        reach: NpcMeleeProfile::spear().reach,
        qi_invest: 12.0,
        wound_kind: WoundKind::Pierce,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let a_wounds = app.world().entity(npc_a).get::<Wounds>().unwrap();
    let b_wounds = app.world().entity(npc_b).get::<Wounds>().unwrap();

    assert_eq!(
        a_wounds.entries.len(),
        1,
        "A should take exactly one wound from B's pierce"
    );
    assert_eq!(a_wounds.entries[0].kind, WoundKind::Pierce);
    assert!(
        a_wounds.health_current <= 0.0,
        "A was 3hp + pierce should be lethal, got {}",
        a_wounds.health_current
    );

    assert_eq!(
        b_wounds.entries.len(),
        1,
        "B should take exactly one wound from A's blunt"
    );
    assert_eq!(b_wounds.entries[0].kind, WoundKind::Blunt);
    assert!(
        b_wounds.health_current > 0.0,
        "B full-hp should survive one blunt, got {}",
        b_wounds.health_current
    );

    // Contamination 同样被写（双向都有 attacker_id = canonical_npc_id）。
    let a_contam = app.world().entity(npc_a).get::<Contamination>().unwrap();
    let b_contam = app.world().entity(npc_b).get::<Contamination>().unwrap();
    assert_eq!(
        a_contam.entries[0].attacker_id.as_deref(),
        Some(canonical_npc_id(npc_b).as_str())
    );
    assert_eq!(
        b_contam.entries[0].attacker_id.as_deref(),
        Some(canonical_npc_id(npc_a).as_str())
    );

    // DeathEvent 应该恰为 A 触发（B 未致命）。
    let deaths: Vec<_> = app
        .world()
        .resource::<Events<DeathEvent>>()
        .get_reader()
        .read(app.world().resource::<Events<DeathEvent>>())
        .cloned()
        .collect();
    assert_eq!(deaths.len(), 1);
    assert_eq!(deaths[0].target, npc_a);
}

// ────────────────────────────────────────────────────────
// plan-woliu-path-v1: VoidCoreActive 回归测试
// ────────────────────────────────────────────────────────

#[test]
fn void_core_active_attacker_cannot_deal_damage() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1100 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "Crimson",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut().entity_mut(attacker).insert(StatusEffects {
        active: vec![ActiveStatusEffect {
            kind: StatusEffectKind::VoidCoreActive,
            magnitude: 1.0,
            remaining_ticks: 60,
            source_pill: None,
        }],
    });

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 1099,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let wounds = app.world().entity(target).get::<Wounds>().unwrap();
    assert_eq!(
        wounds.health_current, wounds.health_max,
        "VoidCoreActive attacker should not deal damage; target health should be unchanged"
    );
    assert!(
        wounds.entries.is_empty(),
        "VoidCoreActive attacker should produce no wound entries"
    );
    assert!(
        app.world().resource::<Events<CombatEvent>>().is_empty(),
        "no CombatEvent should be emitted when attacker has VoidCoreActive"
    );
}

#[test]
fn void_core_active_target_cannot_be_hit() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1100 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);

    let attacker = spawn_player(
        &mut app,
        "Azure",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "Crimson",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.world_mut().entity_mut(target).insert(StatusEffects {
        active: vec![ActiveStatusEffect {
            kind: StatusEffectKind::VoidCoreActive,
            magnitude: 1.0,
            remaining_ticks: 60,
            source_pill: None,
        }],
    });

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 1099,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let wounds = app.world().entity(target).get::<Wounds>().unwrap();
    assert_eq!(
        wounds.health_current, wounds.health_max,
        "VoidCoreActive target should be immune to hits; health should be unchanged"
    );
    assert!(
        wounds.entries.is_empty(),
        "VoidCoreActive target should produce no wound entries"
    );
    assert!(
        app.world().resource::<Events<CombatEvent>>().is_empty(),
        "no CombatEvent should be emitted when target has VoidCoreActive"
    );
}

#[test]
fn void_core_active_defender_cannot_produce_defense_event() {
    let mut app = qi_test_app();
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
                active: vec![ActiveStatusEffect {
                    kind: StatusEffectKind::VoidCoreActive,
                    magnitude: 1.0,
                    remaining_ticks: 60,
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

    let state = app.world().entity(defender).get::<CombatState>().unwrap();
    assert!(
        state.incoming_window.is_none(),
        "VoidCoreActive defender should not produce a defense window"
    );
    let intent_count = app
        .world()
        .resource::<Events<ApplyStatusEffectIntent>>()
        .iter_current_update_events()
        .count();
    assert_eq!(
            intent_count, 0,
            "VoidCoreActive defender should not emit ApplyStatusEffectIntent (e.g. ParryRecovery), got {intent_count}"
        );
}

// ══════════════════════════════════════════════════════════════════════════
// plan-shield-block-v1 P2 — resolve_attack_intents 减伤分支集成测试
// ══════════════════════════════════════════════════════════════════════════

fn make_shield_block_app() -> App {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 5000 });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);
    app
}

// ── happy path：正面命中 + ShieldBlocking → wound.severity / bleeding / contam 按比例削减 ──
#[test]
fn shield_block_front_face_reduces_severity_bleeding_and_contam() {
    let mut app = make_shield_block_app();
    // 攻击者在 [0,64,0]，防御者在 [1,64,0] 朝 +Z（yaw=0）
    // 防御者朝 -X(facing=-sin(0)=0, cos(0)=1 → facing=(0,0,1))
    // to_attacker = 0-1=-1 in X → dot 需计算
    // 为简化：defender 在 z=1，attacker 在 z=3 → to_attacker=(0,0,2) → dot with (0,0,1)=1.0 > -0.5 → 正面
    let attacker = spawn_player(
        &mut app,
        "ShieldAtk",
        [0.0, 64.0, 3.0],
        Wounds::default(),
        Stamina::default(),
    );
    let defender = spawn_player(
        &mut app,
        "ShieldDef",
        [0.0, 64.0, 1.0],
        Wounds::default(),
        Stamina::default(),
    );
    // defender 朝 +Z（yaw=0），attacker 在 z=3（防御者前方）→ dot=1.0 > -0.5 → 正面
    // plan-shield-block-v1 P4: block_ratio 来自 shield_block_profile("bone_shield", proficiency)
    // 骨盾 proficiency=0.0 → block_ratio=0.65（P4 spec）。
    // off_hand 中的骨盾提供 template_id 用于 profile 查找；KnownTechniques 中 proficiency=0.0。
    app.world_mut().entity_mut(defender).insert((
        StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::ShieldBlocking,
                magnitude: 0.65, // P4 snapshot：magnitude 用于记录当前格挡态，block_ratio 通过 profile 计算
                remaining_ticks: crate::combat::shield_block::SHIELD_BLOCKING_DURATION_TICKS,
                source_pill: None,
            }],
        },
        crate::combat::shield_block::ShieldBlock {
            template_id: "bone_shield".to_string(),
        },
        Look {
            yaw: 0.0,
            pitch: 0.0,
        }, // 朝 +Z
        // P4: off_hand 骨盾供 profile 查找（template_id → block_ratio=0.65）
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(1),
            containers: vec![],
            equipped: std::collections::HashMap::from([(
                EQUIP_SLOT_OFF_HAND.to_string(),
                crate::inventory::SlotContents::held_single(ItemInstance {
                    instance_id: 91,
                    template_id: "bone_shield".to_string(),
                    display_name: "骨盾".to_string(),
                    grid_w: 1,
                    grid_h: 2,
                    weight: 2.5,
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
                }),
            )]),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        },
    ));

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(defender),
        issued_at_tick: 4999,
        reach: FIST_REACH,
        qi_invest: 0.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    // 1. CombatEvent 发出 defense_kind = ShieldBlock
    let events: Vec<_> = app
        .world()
        .resource::<Events<CombatEvent>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(events.len(), 1, "应有且仅有一个 CombatEvent");
    assert_eq!(
        events[0].defense_kind,
        Some(DefenseKind::ShieldBlock),
        "正面命中 ShieldBlocking 应发出 defense_kind=ShieldBlock；actual: {:?}",
        events[0].defense_kind
    );
    // 2. defense_effectiveness == block_ratio（plan-shield-block-v1 P4）
    // bone_shield proficiency=0.0 → shield_block_profile("bone_shield", 0.0).block_ratio = 0.65
    assert!(
            events[0]
                .defense_effectiveness
                .is_some_and(|e| (e - 0.65).abs() < 0.01),
            "defense_effectiveness 应等于 bone_shield 基础 block_ratio=0.65（P4 profile，proficiency=0.0），\
             actual: {:?}",
            events[0].defense_effectiveness
        );
    // 3. wound 减伤确认（physical_damage 应小于无盾时的 1.0 unarmed）
    let phys_dmg = events[0].physical_damage;
    assert!(
        phys_dmg < 0.5,
        "65% 盾格挡后 physical_damage 应小于 0.5（减伤比例正确），actual={phys_dmg}"
    );
    // 4. 攻击者无 reflected_damage（盾格挡无反伤，对比 SwordParry 有 0.15 反伤）
    let attacker_wounds = app.world().entity(attacker).get::<Wounds>().unwrap();
    assert!(
        attacker_wounds.entries.is_empty(),
        "盾格挡后攻击者不应有 reflected_damage（无反伤语义），\
             actual entries: {:?}",
        attacker_wounds.entries
    );
}

// ── 背面命中（dot < -0.5）→ 盾格挡无效 ─────────────────────────────────
#[test]
fn shield_block_back_face_no_reduction() {
    let mut app = make_shield_block_app();
    // 防御者在 [0,64,0] 朝 +Z（yaw=0），攻击者在 [0,64,-2]（背后 dot=-1.0 < -0.5）
    let attacker = spawn_player(
        &mut app,
        "BackAtk",
        [0.0, 64.0, -2.0],
        Wounds::default(),
        Stamina::default(),
    );
    let defender = spawn_player(
        &mut app,
        "BackDef",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut().entity_mut(defender).insert((
        StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::ShieldBlocking,
                magnitude: 0.6,
                remaining_ticks: crate::combat::shield_block::SHIELD_BLOCKING_DURATION_TICKS,
                source_pill: None,
            }],
        },
        crate::combat::shield_block::ShieldBlock {
            template_id: "bone_shield".to_string(),
        },
        Look {
            yaw: 0.0,
            pitch: 0.0,
        }, // 朝 +Z，攻击者在 -Z（背后）
    ));

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(defender),
        issued_at_tick: 4999,
        reach: FIST_REACH,
        qi_invest: 0.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    // 背面命中：defense_kind != ShieldBlock（盾不生效）
    let events: Vec<_> = app
        .world()
        .resource::<Events<CombatEvent>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(events.len(), 1);
    assert_ne!(
        events[0].defense_kind,
        Some(DefenseKind::ShieldBlock),
        "背面命中（dot=-1.0 < -0.5）盾格挡不应生效，defense_kind 不应为 ShieldBlock；\
             actual: {:?}",
        events[0].defense_kind
    );
    // 背面命中不减伤：物理伤害不被削减
    assert!(
        events[0].physical_damage >= 0.9,
        "背面命中不减伤，physical_damage 应接近无盾时的基础值（约 1.0 unarmed）；\
             actual: {}",
        events[0].physical_damage
    );
}

// ── 无反伤专属断言：盾格挡后攻击者无 reflected_damage ───────────────────
// 对比 SwordParry（有 0.15 反伤），盾格挡应无反伤。
#[test]
fn shield_block_has_no_reflected_damage_unlike_sword_parry() {
    let mut app = make_shield_block_app();
    let attacker = spawn_player(
        &mut app,
        "NoReflectAtk",
        [0.0, 64.0, 3.0],
        Wounds::default(),
        Stamina::default(),
    );
    let defender = spawn_player(
        &mut app,
        "NoReflectDef",
        [0.0, 64.0, 1.0],
        Wounds::default(),
        Stamina::default(),
    );
    app.world_mut().entity_mut(defender).insert((
        StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::ShieldBlocking,
                magnitude: 0.5,
                remaining_ticks: crate::combat::shield_block::SHIELD_BLOCKING_DURATION_TICKS,
                source_pill: None,
            }],
        },
        crate::combat::shield_block::ShieldBlock {
            template_id: "bone_shield".to_string(),
        },
        Look {
            yaw: 0.0,
            pitch: 0.0,
        },
    ));

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(defender),
        issued_at_tick: 4999,
        reach: FIST_REACH,
        qi_invest: 0.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    let attacker_wounds = app.world().entity(attacker).get::<Wounds>().unwrap();
    assert!(
        attacker_wounds.entries.is_empty(),
        "盾格挡不应产生 reflected_damage（无反伤），\
             对比 SwordParry 有 0.15*blocked 反伤。\
             actual attacker wound entries: {:?}",
        attacker_wounds.entries
    );
}

// ── apply_defense_intents with ShieldBlocking → 无 jiemai 窗口，无 per-block ParryRecovery ──
// plan-shield-block-v1 P2 Issue3 验证：盾格挡 emit 的 DefenseIntent 不应开 jiemai 窗口，
// 也不应施加 per-block ParryRecovery（只有真截脉才有）。
#[test]
fn apply_defense_intent_shield_blocking_no_jiemai_window_no_parry_recovery() {
    let mut app = qi_test_app();
    app.add_event::<DefenseIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_systems(
        Update,
        (
            apply_defense_intents,
            crate::combat::status::status_effect_apply_tick.after(apply_defense_intents),
        ),
    );

    let defender = app
        .world_mut()
        .spawn((
            CombatState::default(),
            Cultivation {
                realm: Realm::Condense, // 修士境界（parry_qi_cost_for_realm 返回 Some）
                qi_current: 12.0,
                qi_max: 20.0,
                ..Cultivation::default()
            },
            StatusEffects {
                active: vec![ActiveStatusEffect {
                    kind: StatusEffectKind::ShieldBlocking,
                    magnitude: 0.6,
                    remaining_ticks: crate::combat::shield_block::SHIELD_BLOCKING_DURATION_TICKS,
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

    // 断言1：盾格挡时不开 jiemai incoming_window
    let state = app.world().entity(defender).get::<CombatState>().unwrap();
    assert!(
        state.incoming_window.is_none(),
        "盾格挡下 apply_defense_intents 不应开 jiemai incoming_window（盾不耦合截脉）；\
             actual incoming_window: {:?}",
        state.incoming_window
    );
    // 断言2：不发 per-block ParryRecovery intent
    let intents: Vec<_> = app
        .world()
        .resource::<Events<ApplyStatusEffectIntent>>()
        .iter_current_update_events()
        .collect();
    let has_parry_recovery = intents
        .iter()
        .any(|i| i.kind == StatusEffectKind::ParryRecovery && i.target == defender);
    assert!(
        !has_parry_recovery,
        "盾格挡下 apply_defense_intents 不应施加 per-block ParryRecovery（锁 0.5s）；\
             actual intents: {intents:?}"
    );
}

// ══════════════════════════════════════════════════════════════════════════
// plan-shield-block-v1 P3 — 耐久扣减 / ShieldBroken 事件 / 互斥裁定
// ══════════════════════════════════════════════════════════════════════════

/// 构建带 ItemRegistry（木盾/骨盾 ShieldSpec）的 App，适合 P3 耐久测试。
fn make_shield_durability_app() -> App {
    use crate::inventory::{ItemCategory, ItemTemplate, ShieldSpec};

    let registry = ItemRegistry::from_map(std::collections::HashMap::from([
        (
            "wooden_shield".to_string(),
            ItemTemplate {
                id: "wooden_shield".to_string(),
                display_name: "木盾".to_string(),
                category: ItemCategory::Shield,
                placeable: None,
                max_stack_count: 1,
                grid_w: 1,
                grid_h: 2,
                base_weight: 3.0,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 0.0,
                description: String::new(),
                effect: None,
                cast_duration_ms: 0,
                cooldown_ms: 0,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                readable_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shelflife_profile: None,
                shield_spec: Some(ShieldSpec {
                    block_ratio: 0.5,
                    durability_max: 40.0,
                    stamina_drain_per_s: 3.0,
                }),
                shelflife_track: None,
                wearer_race: crate::body_plan::types::RaceGateOwned::default(),
            },
        ),
        (
            "bone_shield".to_string(),
            ItemTemplate {
                id: "bone_shield".to_string(),
                display_name: "骨盾".to_string(),
                category: ItemCategory::Shield,
                placeable: None,
                max_stack_count: 1,
                grid_w: 1,
                grid_h: 2,
                base_weight: 4.5,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 0.0,
                description: String::new(),
                effect: None,
                cast_duration_ms: 0,
                cooldown_ms: 0,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                readable_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shelflife_profile: None,
                shield_spec: Some(ShieldSpec {
                    block_ratio: 0.65,
                    durability_max: 80.0,
                    stamina_drain_per_s: 3.0,
                }),
                shelflife_track: None,
                wearer_race: crate::body_plan::types::RaceGateOwned::default(),
            },
        ),
    ]));

    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 8000 });
    app.insert_resource(registry);
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_systems(Update, resolve_attack_intents);
    app
}

/// 给 entity 装上 off_hand 盾（初始耐久 ratio）。
fn equip_shield_off_hand(
    app: &mut App,
    entity: Entity,
    template_id: &str,
    instance_id: u64,
    durability: f64,
) {
    let inv = PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers: vec![ContainerState {
            quick_access: false,
            id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 5,
            cols: 7,
            items: vec![],
            owner_instance_id: None,
        }],
        equipped: std::collections::HashMap::from([(
            EQUIP_SLOT_OFF_HAND.to_string(),
            crate::inventory::SlotContents::held_single(ItemInstance {
                instance_id,
                template_id: template_id.to_string(),
                display_name: template_id.to_string(),
                grid_w: 1,
                grid_h: 2,
                weight: 3.0,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 0.0,
                durability,
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
    };
    app.world_mut().entity_mut(entity).insert(inv);
}

/// 给 entity 插入 ShieldBlocking status（front-face 格挡）。
fn insert_shield_blocking(app: &mut App, entity: Entity, block_ratio: f32) {
    use valence::entity::Look;
    app.world_mut().entity_mut(entity).insert((
        StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::ShieldBlocking,
                magnitude: block_ratio,
                remaining_ticks: crate::combat::shield_block::SHIELD_BLOCKING_DURATION_TICKS,
                source_pill: None,
            }],
        },
        crate::combat::shield_block::ShieldBlock {
            template_id: "wooden_shield".to_string(),
        },
        Look {
            yaw: 0.0,
            pitch: 0.0,
        },
    ));
}

/// 发送物理攻击 intent。severity ≈ damage，以 WoundKind::Blunt 物理攻击。
fn send_physical_attack(app: &mut App, attacker: Entity, target: Entity) {
    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: 7999,
        reach: FIST_REACH,
        qi_invest: 0.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
}

// ── P3: 盾耐久公式锁定测试 ───────────────────────────────────────────────
//
// durability_max 表示满伤格挡次数；实际每次扣减量是本次被格挡的 blocked severity，
// 因此半伤格挡需要更多次命中才能归零。
#[test]
fn shield_durability_formula_uses_blocked_damage_as_cost() {
    let durability_max = 40.0_f64;
    let blocked_per_hit = 0.5_f64;
    let expected_hits_to_break = (durability_max / blocked_per_hit).ceil() as u32;

    let mut cur_ratio = 1.0_f64;
    let mut prev_ratio = 2.0_f64;
    let mut broke_at = None;

    for hit in 1..=expected_hits_to_break + 5 {
        let cur_abs = cur_ratio * durability_max;
        let next_abs = (cur_abs - blocked_per_hit).max(0.0);
        let next_ratio = (next_abs / durability_max).clamp(0.0, 1.0);
        if cur_ratio > 0.0 {
            assert!(
                next_ratio <= cur_ratio,
                "盾耐久应单调不增：hit={hit}，cur_ratio={cur_ratio:.6}，next_ratio={next_ratio:.6}"
            );
        }
        if next_ratio > 0.0 {
            assert!(cur_ratio > 0.0, "hit={hit}：耐久比例前置状态不应已归零");
        }
        prev_ratio = cur_ratio;
        cur_ratio = next_ratio;
        if cur_ratio <= 0.0 && broke_at.is_none() {
            broke_at = Some(hit);
        }
    }

    assert_eq!(
        broke_at,
        Some(expected_hits_to_break),
        "blocked_per_hit={blocked_per_hit:.2}、durability_max={durability_max} 时应在第 {expected_hits_to_break} 次归零"
    );
    let _ = prev_ratio;
}

#[test]
fn shield_durability_breaks_at_durability_max_full_damage_hits() {
    let durability_max = 40.0_f64;
    let blocked_full = 1.0_f64;
    let expected_hits = durability_max as u32;

    let mut cur_ratio = 1.0_f64;
    for hit in 1..=(expected_hits + 1) {
        let cur_abs = cur_ratio * durability_max;
        let next_abs = (cur_abs - blocked_full).max(0.0);
        cur_ratio = (next_abs / durability_max).clamp(0.0, 1.0);
        if hit == expected_hits - 1 {
            assert!(
                cur_ratio > 0.0,
                "第 {hit} 次满伤格挡后应仍有耐久，actual={cur_ratio:.6}"
            );
        }
    }
    assert!(
        cur_ratio <= 0.0,
        "满伤格挡 {expected_hits} 次后耐久应归零，actual={cur_ratio:.6}"
    );
}

// ── P3: ECS 集成 — 盾耐久扣减 + 归零 → ShieldBroken 事件 + inventory 移除 ──
#[test]
fn shield_broken_event_emitted_exactly_once_when_durability_reaches_zero() {
    let mut app = make_shield_durability_app();

    // 攻击者在 [0,64,3]，防御者（持盾者）在 [0,64,1]，正面正对
    let attacker = spawn_player(
        &mut app,
        "ShieldBrkAtk",
        [0.0, 64.0, 3.0],
        Wounds::default(),
        Stamina::default(),
    );
    let defender = spawn_player(
        &mut app,
        "ShieldBrkDef",
        [0.0, 64.0, 1.0],
        Wounds::default(),
        Stamina::default(),
    );

    // 给防御者装木盾，初始耐久极低（ratio = 0.001，cur_abs ≈ 0.04 < 任何格挡值）
    // 原因：一次拳击 block_ratio=0.5 产生 blocked ≈ 0.5 单位，
    // 若 cur_abs=1.0 则 next_abs=0.5 > 0，不触发 ShieldBroken；
    // cur_abs=0.04 < 0.5，确保一击清零。
    let last_tick_ratio = 0.001_f64; // 极低耐久 — 任何格挡都会清空
    equip_shield_off_hand(&mut app, defender, "wooden_shield", 77, last_tick_ratio);
    insert_shield_blocking(&mut app, defender, 0.5);

    // 初始时 inventory 有盾
    {
        let inv = app
            .world()
            .entity(defender)
            .get::<PlayerInventory>()
            .unwrap();
        assert!(
            inv.equipped
                .get(EQUIP_SLOT_OFF_HAND)
                .and_then(|s| s.held.as_ref())
                .is_some(),
            "前提：off_hand 槽应有盾牌 instance_id=77"
        );
    }

    send_physical_attack(&mut app, attacker, defender);
    app.update();

    // 断言1：ShieldBroken 事件恰好发出一次
    let broken_events: Vec<_> = app
        .world()
        .resource::<Events<ShieldBroken>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(
        broken_events.len(),
        1,
        "耐久归零时 ShieldBroken 事件应恰好发出一次；\
             期望 1 因为只有一次攻击且耐久归零；实际 {} 次。events: {broken_events:?}",
        broken_events.len()
    );
    assert_eq!(
        broken_events[0].instance_id, 77,
        "ShieldBroken.instance_id 应为 77；实际 {}",
        broken_events[0].instance_id
    );
    assert_eq!(
        broken_events[0].template_id, "wooden_shield",
        "ShieldBroken.template_id 应为 'wooden_shield'；实际 {:?}",
        broken_events[0].template_id
    );

    // 断言2：inventory 中盾已被移除（盾销毁）
    let inv = app
        .world()
        .entity(defender)
        .get::<PlayerInventory>()
        .unwrap();
    assert!(
        inv.equipped
            .get(EQUIP_SLOT_OFF_HAND)
            .and_then(|s| s.held.as_ref())
            .is_none(),
        "盾耐久归零后 off_hand 槽应为空（盾销毁）；实际仍存在 held: {:?}",
        inv.equipped
            .get(EQUIP_SLOT_OFF_HAND)
            .and_then(|s| s.held.as_ref())
    );
}

// ── P3: 耐久未归零 → 不 emit ShieldBroken，物品保留 ──────────────────────
#[test]
fn no_shield_broken_event_when_durability_remains_above_zero() {
    let mut app = make_shield_durability_app();

    let attacker = spawn_player(
        &mut app,
        "ShieldPartAtk",
        [0.0, 64.0, 3.0],
        Wounds::default(),
        Stamina::default(),
    );
    let defender = spawn_player(
        &mut app,
        "ShieldPartDef",
        [0.0, 64.0, 1.0],
        Wounds::default(),
        Stamina::default(),
    );

    // 初始 ratio = 1.0（满耐久）
    equip_shield_off_hand(&mut app, defender, "wooden_shield", 55, 1.0);
    insert_shield_blocking(&mut app, defender, 0.5);

    send_physical_attack(&mut app, attacker, defender);
    app.update();

    // 未归零 → 不发 ShieldBroken
    let broken_events: Vec<_> = app
        .world()
        .resource::<Events<ShieldBroken>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(
        broken_events.len(),
        0,
        "耐久未归零时 ShieldBroken 不应发出；\
             期望 0 因为木盾 durability_max=40 一次格挡仅扣 1/40；实际 {} 次",
        broken_events.len()
    );

    // 物品保留
    let inv = app
        .world()
        .entity(defender)
        .get::<PlayerInventory>()
        .unwrap();
    assert!(
        inv.equipped
            .get(EQUIP_SLOT_OFF_HAND)
            .and_then(|s| s.held.as_ref())
            .is_some(),
        "耐久未归零时 off_hand 盾应保留；实际 off_hand held 为 None"
    );
}

// ── P3: §10.5 qi_invest>0 减伤 pin 测试 — 盾对真元伤也减伤 ──────────────
// worldview.md:432「防御本质=处理物理冲击 AND 真元污染」：
// 盾的 FOV check + ShieldBlocking 对 qi 攻击同样削减 contam/qi severity。
// P3 lock：qi_invest>0 攻击时，ShieldBlocking active 的防御者 contam_delta 被 block_ratio 削减。
#[test]
fn shield_block_reduces_qi_contamination_when_qi_invest_positive() {
    let mut app = make_shield_durability_app();

    let attacker = spawn_player(
        &mut app,
        "QiAtk",
        [0.0, 64.0, 3.0],
        Wounds::default(),
        Stamina::default(),
    );
    let defender = spawn_player(
        &mut app,
        "QiDef",
        [0.0, 64.0, 1.0],
        Wounds::default(),
        Stamina::default(),
    );

    equip_shield_off_hand(&mut app, defender, "wooden_shield", 99, 1.0);
    insert_shield_blocking(&mut app, defender, 0.5);

    // qi 攻击（qi_invest > 0）
    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(defender),
        issued_at_tick: 7999,
        reach: FIST_REACH,
        qi_invest: 10.0, // 真元投入 > 0
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();

    // 盾格挡对 qi 攻击的效果：CombatEvent 应有 shield_contam_reduced > 0
    let events: Vec<_> = app
        .world()
        .resource::<Events<CombatEvent>>()
        .iter_current_update_events()
        .collect();
    let has_shield_block = events
        .iter()
        .any(|e| e.defense_kind == Some(DefenseKind::ShieldBlock));
    assert!(
        has_shield_block,
        "qi_invest>0 攻击时，ShieldBlocking active 应触发 ShieldBlock defense_kind；\
             期望盾对真元伤也减伤（worldview §432）；actual events: {events:?}"
    );
    let shield_contam_reduced = events
        .iter()
        .find(|e| e.defense_kind == Some(DefenseKind::ShieldBlock))
        .and_then(|e| e.defense_contam_reduced);
    assert!(
        shield_contam_reduced.is_some_and(|r| r > 0.0),
        "qi 攻击 ShieldBlocking 应削减 contam（defense_contam_reduced > 0）；\
             期望 > 0 因为 qi_invest=10 产生 contam，block_ratio=0.5 削减一半；\
             actual defense_contam_reduced: {shield_contam_reduced:?}"
    );
}

// ── P3: §10.5 SwordParrying+ShieldBlocking 同帧互斥裁定（防回归 pin）────────
// 互斥裁定结论：两者各自独立串行削减，无算术叠加失衡，不施加互斥守护。
// 设计意图：SwordParrying 减境界加成伤害（真元），ShieldBlocking 减物理伤。
// 同帧双激活极罕见（需同 tick 截脉窗口+举盾），串行削减符合设计意图。
//
// 本测试断言：同帧双 active 时，最终 physical_damage < 未格挡时的数值，
// 且 ShieldBroken 不意外触发（双减伤不导致算术失衡/过扣）。
#[test]
fn swordparry_and_shieldblock_both_active_does_not_double_zero_damage_or_break_shield_unexpectedly()
{
    // SwordParrying 和 ShieldBlocking 同帧双 active 时：
    // 1. wound.severity 经 SwordParrying 削减后，ShieldBlocking 再从残差继续削减 → 串行，合理
    // 2. 不会出现伤害归负 / 异常数值（两个 clamp 保证）
    // 3. 木盾满耐久一次双减伤格挡仍不会立即破盾（单次攻击 blocked_damage < durability 绝对值）

    let mut app = make_shield_durability_app();

    let attacker = spawn_player(
        &mut app,
        "DualDefAtk",
        [0.0, 64.0, 3.0],
        Wounds::default(),
        Stamina::default(),
    );
    let defender = spawn_player(
        &mut app,
        "DualDefDef",
        [0.0, 64.0, 1.0],
        Wounds::default(),
        Stamina::default(),
    );

    // 同时插入 SwordParrying（0.5）和 ShieldBlocking（0.5）两个 status
    use valence::entity::Look;
    app.world_mut().entity_mut(defender).insert((
        StatusEffects {
            active: vec![
                ActiveStatusEffect {
                    kind: StatusEffectKind::SwordParrying,
                    magnitude: 0.5,
                    remaining_ticks: 100,
                    source_pill: None,
                },
                ActiveStatusEffect {
                    kind: StatusEffectKind::ShieldBlocking,
                    magnitude: 0.5,
                    remaining_ticks: crate::combat::shield_block::SHIELD_BLOCKING_DURATION_TICKS,
                    source_pill: None,
                },
            ],
        },
        crate::combat::shield_block::ShieldBlock {
            template_id: "wooden_shield".to_string(),
        },
        Look {
            yaw: 0.0,
            pitch: 0.0,
        },
    ));
    equip_shield_off_hand(&mut app, defender, "wooden_shield", 200, 1.0);

    send_physical_attack(&mut app, attacker, defender);
    app.update();

    // 断言1：physical_damage < 1.0（不满格挡时的基础伤害，双减伤有效）
    let events: Vec<_> = app
        .world()
        .resource::<Events<CombatEvent>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(events.len(), 1, "应有一个 CombatEvent");
    let phys = events[0].physical_damage;
    assert!(
        phys >= 0.0,
        "双减伤后 physical_damage 不得为负数（clamp 保证）；实际 {phys:.4}"
    );
    // 期望 physical_damage < 1.0（无减伤时约 1.0，两层 0.5 减伤后应显著更小）
    assert!(
        phys < 1.0,
        "SwordParrying(0.5)+ShieldBlocking(0.5) 双激活后 physical_damage 应 < 1.0（双减伤有效）；\
             实际 {phys:.4}；若 ≥ 1.0 则减伤管道失效"
    );

    // 断言2：满耐久木盾不因一次双减伤格挡立即破盾
    let broken_events: Vec<_> = app
        .world()
        .resource::<Events<ShieldBroken>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(
        broken_events.len(),
        0,
        "满耐久木盾（durability_max=40）一次双减伤格挡后不应立即破盾；\
             实际 {} 个 ShieldBroken 事件（期望 0，因为 blocked_damage < 40.0）",
        broken_events.len()
    );

    // 断言3（互斥裁定 pin）：双减伤结果在合法范围内（无算术失衡）
    // 期望 physical_damage = base * (1 - 0.5) * (1 - 0.5) = base * 0.25
    // 不要求精确值，但应在 [0, base] 区间内
    let inv = app
        .world()
        .entity(defender)
        .get::<PlayerInventory>()
        .unwrap();
    assert!(
        inv.equipped
            .get(EQUIP_SLOT_OFF_HAND)
            .and_then(|s| s.held.as_ref())
            .is_some(),
        "满耐久木盾一次格挡后 off_hand 盾应保留（未破盾）；实际 off_hand held 为 None"
    );
}

// ── 死脉甲污染豁免接线（plan-baomai-v4 P0）集成测试 ──

/// 构建最小 app：注册 resolve_attack_intents 所需的全部事件，不引入额外系统。
fn setup_dead_armor_app(tick: u64) -> App {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick });
    app.add_event::<AttackIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<WeaponBroken>();
    app.add_event::<ShieldBroken>();
    app.add_event::<ShieldBlockHit>();
    app.add_event::<InventoryDurabilityChangedEvent>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, resolve_attack_intents);
    app
}

/// 给目标实体插入 DeadMeridianArmor，免疫 immune_part。
fn attach_dead_armor(app: &mut App, entity: Entity, immune_part: BodyPart) {
    use crate::combat::baomai_v4::dead_armor::DeadMeridianArmor;
    let mut armor = DeadMeridianArmor::default();
    armor.immune_regions.insert(immune_part);
    app.world_mut().entity_mut(entity).insert(armor);
}

/// 发出一次 qi 攻击（非物理），命中特定 body_part。
/// 注意：raycast_humanoid 按距离决定 body_part——
/// Chest/Back 在 [0.3, 0.9]y 偏移、Head 在 [1.4, 1.8]y、
/// ArmL/ArmR 在侧面。此处用正面 (z+1) 近距离攻击，
/// raycast 结果为 Chest，便于测试。
fn send_qi_attack(app: &mut App, attacker: Entity, target: Entity, tick: u64) {
    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: tick,
        reach: FIST_REACH,
        qi_invest: 10.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
}

/// 死脉甲免疫区命中：contamination.entries 不应包含任何源。
///
/// 设置：Ren 经脉 VoluntarySever → Chest 免疫 → 攻击命中 Chest（正面近距离）。
/// 期望：攻击后 contamination.entries 为空（免疫区被拦截，delta 直接 DROP，
///        无 zone release——守恒决议 drop_no_release）。
#[test]
fn dead_armor_immune_region_blocks_contamination() {
    let mut app = setup_dead_armor_app(2000);

    let attacker = spawn_player(
        &mut app,
        "AttackerImm",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "TargetImm",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    // 给 target 插入 DeadMeridianArmor，Chest 免疫（Ren 经脉绝脉后的映射）。
    attach_dead_armor(&mut app, target, BodyPart::Chest);

    // 首次 update（初始化系统）。
    app.update();

    // 发出 qi 攻击（qi_invest=10 > 0，产生污染路径）。
    send_qi_attack(&mut app, attacker, target, 1999);
    app.update();

    // 验证：CombatEvent 应已发出（攻击落地）。
    let events: Vec<_> = app
        .world()
        .resource::<Events<CombatEvent>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(
        events.len(),
        1,
        "期望 1 个 CombatEvent，因为攻击落地了；实际 {} 个",
        events.len()
    );

    // 主断言：contamination.entries 为空——免疫区拦截后 DROP，无注入。
    let contam = app.world().entity(target).get::<Contamination>().unwrap();
    assert!(
        contam.entries.is_empty(),
        "期望 contamination.entries 为空，因为命中部位（Chest）是 dead_armor 免疫区，\
             被拦截的 delta 直接 DROP（drop_no_release），不应写入任何 ContamSource；\
             实际 entries={:?}",
        contam.entries
    );

    // 守恒断言：事件 contam_delta 也应为 0（反映 DROP 后的值）。
    assert_eq!(
        events[0].contam_delta, 0.0,
        "期望 CombatEvent.contam_delta=0.0，因为死脉甲免疫区拦截后直接 DROP；\
             实际 contam_delta={:.4}",
        events[0].contam_delta
    );
}

/// 非免疫部位照常写入污染：dead_armor 仅拦截免疫区，不影响其他部位。
///
/// 设置：Chest 免疫，攻击命中 Chest（先验证免疫生效），
/// 然后换一个无免疫的 target（无 DeadMeridianArmor），攻击命中相同 body_part，
/// 验证 entries 非空。
#[test]
fn non_immune_region_still_contaminated() {
    let mut app = setup_dead_armor_app(2010);

    let attacker = spawn_player(
        &mut app,
        "AttackerNonImm",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    // target_no_armor：没有 DeadMeridianArmor，任何部位都不免疫。
    let target_no_armor = spawn_player(
        &mut app,
        "TargetNoArmor",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    app.update();

    send_qi_attack(&mut app, attacker, target_no_armor, 2009);
    app.update();

    let contam = app
        .world()
        .entity(target_no_armor)
        .get::<Contamination>()
        .unwrap();
    assert!(
        !contam.entries.is_empty(),
        "期望 contamination.entries 非空，因为 target_no_armor 没有 DeadMeridianArmor，\
             qi 攻击应正常写入污染；实际 entries 为空（说明过滤逻辑误判为免疫）"
    );
    // 进一步：entries[0].amount > 0
    assert!(
        contam.entries[0].amount > 0.0,
        "期望 entries[0].amount > 0，因为 qi 攻击 qi_invest=10.0 会产生非零污染；\
             实际 amount={:.4}",
        contam.entries[0].amount
    );
}

/// 无 DeadMeridianArmor 组件时照常污染（组件缺失不 panic，不误判免疫）。
#[test]
fn no_dead_armor_component_contaminates_normally() {
    let mut app = setup_dead_armor_app(2020);

    let attacker = spawn_player(
        &mut app,
        "AttackerNoDMA",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "TargetNoDMA",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    // target 故意不 insert DeadMeridianArmor。

    app.update();

    send_qi_attack(&mut app, attacker, target, 2019);
    app.update();

    let contam = app.world().entity(target).get::<Contamination>().unwrap();
    assert!(
        !contam.entries.is_empty(),
        "期望 contamination.entries 非空：target 无 DeadMeridianArmor，\
             Option<&DeadMeridianArmor>=None 时不应触发免疫逻辑；\
             实际 entries 为空（说明 flatten/is_some_and 错误地挡住了 None 分支）"
    );
}

/// 守恒断言（drop_no_release）：免疫区命中后 CombatEvent.contam_delta=0。
///
/// 两个独立 app 分别验证：
///   sub-case A：有免疫 → contam_delta=0（DROP）。
///   sub-case B：无免疫 → contam_delta>0（正常路径）。
/// 对比确认"拦截=丢弃，非 release_to_zone"。
#[test]
fn dead_armor_block_is_drop_not_release() {
    // ── sub-case A：有免疫区，contam_delta 应为 0 ──
    {
        let mut app = setup_dead_armor_app(2030);
        let attacker = spawn_player(
            &mut app,
            "AttackerDropA",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target_immune = spawn_player(
            &mut app,
            "TargetImmuneA",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        attach_dead_armor(&mut app, target_immune, BodyPart::Chest);
        app.update();

        send_qi_attack(&mut app, attacker, target_immune, 2029);
        app.update();

        let events: Vec<_> = app
            .world()
            .resource::<Events<CombatEvent>>()
            .iter_current_update_events()
            .collect();
        assert_eq!(
            events.len(),
            1,
            "期望免疫攻击产生 1 个 CombatEvent；实际 {}",
            events.len()
        );
        assert_eq!(
            events[0].contam_delta, 0.0,
            "期望免疫区命中后 CombatEvent.contam_delta=0.0（DROP，非 release_to_zone）；\
                 实际={:.4}，若非 0 则说明拦截后仍向 zone 注入通胀",
            events[0].contam_delta
        );
        // 守恒补强：DROP 路径不应 emit 任何 QiTransfer（release_to_zone 会产生 QiTransfer）。
        let qi_transfers: Vec<_> = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .collect();
        assert!(
            qi_transfers.is_empty(),
            "期望死脉甲免疫只 DROP contamination，不 emit QiTransfer/release_to_zone；\
                 实际 qi_transfers.len()={} — 若非空说明实现错误地走了 release_to_zone 导致通胀",
            qi_transfers.len()
        );
    }

    // ── sub-case B：无免疫区，contam_delta 应 > 0（正常路径验证过滤器不误杀）──
    {
        let mut app = setup_dead_armor_app(2031);
        let attacker = spawn_player(
            &mut app,
            "AttackerDropB",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target_normal = spawn_player(
            &mut app,
            "TargetNormalB",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        // target_normal 无 DeadMeridianArmor。
        app.update();

        send_qi_attack(&mut app, attacker, target_normal, 2030);
        app.update();

        let events: Vec<_> = app
            .world()
            .resource::<Events<CombatEvent>>()
            .iter_current_update_events()
            .collect();
        assert_eq!(
            events.len(),
            1,
            "期望无免疫攻击产生 1 个 CombatEvent；实际 {}",
            events.len()
        );
        assert!(
            events[0].contam_delta > 0.0,
            "期望无免疫 target 的 contam_delta > 0（正常污染路径）；\
                 实际={:.4}，若为 0 则过滤误杀了正常路径",
            events[0].contam_delta
        );
    }
}

/// 多免疫区集合包含 Chest 的拦截验证（端到端集成）。
///
/// 注意（plan-combat-hit-location-v1 §P0 更新，2026-07）：`raycast_humanoid` 已不再内置
/// 恒定胸心 fallback——命中部位现由调用方传入的瞄准方向决定。本测试的攻方是玩家且未
/// 显式设置 `Look`（等同缺失瞄准数据，见 resolve_attack_intents 的 fallback 分支），
/// 因此仍会退化为几何中心瞄准、稳定命中 Chest；这是该 fallback 分支的既定行为，
/// 不再是"raycast 无法命中其他部位"的系统性限制。
/// 本测试端到端验证"Chest 在多免疫区集合中 → DROP"，
/// ArmL 的集合成员有效性通过 `dead_armor_arml_immune_in_multi_region_set` 单元测试覆盖
/// （该测试不经过 raycast，直接验证 `should_block_contamination` 的集合查询逻辑）。
#[test]
fn dead_armor_multi_region_set_chest_is_blocked() {
    use crate::combat::baomai_v4::dead_armor::DeadMeridianArmor;
    let mut app = setup_dead_armor_app(2040);

    let attacker = spawn_player(
        &mut app,
        "AttackerMulti",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "TargetMulti",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    // 给 target 插入多免疫区（Chest + ArmL）。
    {
        let mut armor = DeadMeridianArmor::default();
        armor.immune_regions.insert(BodyPart::Chest);
        armor.immune_regions.insert(BodyPart::ArmL);
        app.world_mut().entity_mut(target).insert(armor);
    }

    app.update();

    // 攻击命中 Chest（默认正面近距离命中）。
    send_qi_attack(&mut app, attacker, target, 2039);
    app.update();

    // 先锁定命中部位为 Chest，防止几何变更导致假通过。
    let events: Vec<_> = app
        .world()
        .resource::<Events<CombatEvent>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(
        events.len(),
        1,
        "期望 Chest 攻击产生 1 个 CombatEvent；实际 {}",
        events.len()
    );
    assert_eq!(
        events[0].body_part,
        BodyPart::Chest,
        "期望正面近距离攻击命中 Chest，因为 raycast 默认瞄准 target 中心；实际命中 {:?}",
        events[0].body_part
    );

    let contam = app.world().entity(target).get::<Contamination>().unwrap();
    assert!(
        contam.entries.is_empty(),
        "期望 contamination.entries 为空：Chest 在多免疫区集合内（Chest+ArmL），应被 DROP；\
             实际 entries={:?}",
        contam.entries
    );
}

/// 边界：target 有 DeadMeridianArmor 但命中部位（Chest）不在免疫区内，照常污染。
///
/// immune_regions 只包含 ArmL，不含 Chest——正面攻击命中 Chest 应穿透免疫。
#[test]
fn dead_armor_non_immune_part_still_contaminates() {
    use crate::combat::baomai_v4::dead_armor::DeadMeridianArmor;
    let mut app = setup_dead_armor_app(2050);

    let attacker = spawn_player(
        &mut app,
        "AttackerPartial",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "TargetPartial",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );

    // 只免疫 ArmL（非攻击命中部位），Chest 未免疫。
    // 注意：BodyPart::Back 是死路免疫（classify_body_part 永不产出）；
    // 此测试改用 ArmL，确保非命中部位免疫不误拦截 Chest 攻击。
    {
        let mut armor = DeadMeridianArmor::default();
        armor.immune_regions.insert(BodyPart::ArmL);
        app.world_mut().entity_mut(target).insert(armor);
    }

    app.update();

    // 正面攻击命中 Chest（非免疫区）。
    send_qi_attack(&mut app, attacker, target, 2049);
    app.update();

    let contam = app.world().entity(target).get::<Contamination>().unwrap();
    assert!(
        !contam.entries.is_empty(),
        "期望 contamination.entries 非空：Chest 不在免疫区（仅 ArmL 免疫），\
             应正常写入污染；实际 entries 为空（partial 豁免误拦截了非免疫部位）"
    );
}

/// Head 和 Abdomen 是刻意设计的弱点区——死脉甲对其无保护。
///
/// `meridian_to_body_part` 无任何经脉映射到 Head 或 Abdomen，
/// 因此即使 target 拥有全经脉死脉甲免疫，Head/Abdomen 命中仍产生污染。
///
/// 攻击几何（依据 classify_body_part 阈值，STANDING_HEIGHT=1.8）：
/// - Head（rel_y > 0.88）：攻方 y=65.0，高于目标 → 击中顶面（rel_y=1.0→Head）
/// - Abdomen（0.35 < rel_y ≤ 0.55）：攻方 y=62.0，低于目标 → 击中中低部（rel_y≈0.40→Abdomen）
#[test]
fn dead_armor_head_and_abdomen_are_always_vulnerable() {
    use crate::combat::baomai_v4::dead_armor::DeadMeridianArmor;

    // ── Head 命中：target 满免疫区仍被污染 ──
    {
        let mut app = setup_dead_armor_app(2060);

        // 攻方高于目标（y=65.0），raycast 向下命中 Head（顶面 rel_y=1.0→Head）。
        let attacker = spawn_player(
            &mut app,
            "AttackerHead",
            [0.0, 65.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "TargetHead",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        // 给 target 插入所有可命中区域的免疫（Chest/ArmL/ArmR/LegL/LegR），
        // 但无 Head 映射——Head 是永久弱点区。
        {
            let mut armor = DeadMeridianArmor::default();
            armor.immune_regions.insert(BodyPart::Chest);
            armor.immune_regions.insert(BodyPart::ArmL);
            armor.immune_regions.insert(BodyPart::ArmR);
            armor.immune_regions.insert(BodyPart::LegL);
            armor.immune_regions.insert(BodyPart::LegR);
            app.world_mut().entity_mut(target).insert(armor);
        }

        app.update();
        send_qi_attack(&mut app, attacker, target, 2059);
        app.update();

        // 先锁定命中部位为 Head，防止几何变更后命中其他部位导致假通过。
        let head_events: Vec<_> = app
            .world()
            .resource::<Events<CombatEvent>>()
            .iter_current_update_events()
            .collect();
        assert_eq!(
            head_events.len(),
            1,
            "期望 Head 弱点子用例恰好产生 1 个 CombatEvent；实际 {} 个",
            head_events.len()
        );
        assert_eq!(
            head_events[0].body_part,
            BodyPart::Head,
            "期望 Head 弱点子用例实际命中 Head（攻方 y=65.0 高于目标 y=64.0，向下命中顶面）；\
                 实际命中 {:?} — 若此断言失败说明 classify_body_part 阈值变更，需同步调整攻方位置",
            head_events[0].body_part
        );

        let contam = app.world().entity(target).get::<Contamination>().unwrap();
        assert!(
            !contam.entries.is_empty(),
            "期望 contamination.entries 非空：Head 无死脉甲映射（刻意弱点区），\
                 即使 target 持有所有其他区域免疫，Head 命中仍应写入污染；\
                 实际 entries 为空（说明 Head 命中被误判为免疫）"
        );
    }

    // ── Abdomen 命中：target 满免疫区仍被污染 ──
    {
        let mut app = setup_dead_armor_app(2070);

        // 攻方低于目标（y=62.8），raycast 向上命中 Abdomen。
        // plan-combat-hit-location-v1 P1 校准把 LEG_ABDOMEN_BOUNDARY 从 0.35 上调到
        // 0.53（见 raycast.rs 该常量注释），把 Abdomen 命中带收窄成 rel_y∈(0.53,0.55]
        // 的窄带——原 y=62.0（rel_y≈0.40）在新阈值下已跌入 Leg，实测扫描 y∈[62.76,62.88]
        // 仍稳定落在新 Abdomen 窄带内，取中间值 62.8 留出双向余量。
        let attacker = spawn_player(
            &mut app,
            "AttackerAbd",
            [0.0, 62.8, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "TargetAbd",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        // 给 target 插入所有可命中区域的免疫，但无 Abdomen 映射。
        {
            let mut armor = DeadMeridianArmor::default();
            armor.immune_regions.insert(BodyPart::Chest);
            armor.immune_regions.insert(BodyPart::ArmL);
            armor.immune_regions.insert(BodyPart::ArmR);
            armor.immune_regions.insert(BodyPart::LegL);
            armor.immune_regions.insert(BodyPart::LegR);
            app.world_mut().entity_mut(target).insert(armor);
        }

        app.update();
        send_qi_attack(&mut app, attacker, target, 2069);
        app.update();

        // 先锁定命中部位为 Abdomen，防止几何变更后命中其他部位导致假通过。
        let abd_events: Vec<_> = app
            .world()
            .resource::<Events<CombatEvent>>()
            .iter_current_update_events()
            .collect();
        assert_eq!(
            abd_events.len(),
            1,
            "期望 Abdomen 弱点子用例恰好产生 1 个 CombatEvent；实际 {} 个",
            abd_events.len()
        );
        assert_eq!(
                abd_events[0].body_part,
                BodyPart::Abdomen,
                "期望 Abdomen 弱点子用例实际命中 Abdomen（攻方 y=62.8 低于目标 y=64.0，向上命中中低部）；\
                 实际命中 {:?} — 若此断言失败说明 classify_body_part 阈值变更，需同步调整攻方位置",
                abd_events[0].body_part
            );

        let contam = app.world().entity(target).get::<Contamination>().unwrap();
        assert!(
            !contam.entries.is_empty(),
            "期望 contamination.entries 非空：Abdomen 无死脉甲映射（刻意弱点区），\
                 即使 target 持有所有其他区域免疫，Abdomen 命中仍应写入污染；\
                 实际 entries 为空（说明 Abdomen 命中被误判为免疫）"
        );
    }
}

// ── plan-race-system-v1 P0 review 修复（BLOCKING-2）：dandao 变异部位减伤真实
// 消费链端到端测试——不止单元测试锁住 `mutation_damage_multiplier_for_part` 这个
// 孤立函数，而是证明 `resolve_attack_intents` 真的按 `mutation_slot_mapping` 解析
// 出的部位对命中伤害生效 ─────────────────────────────────────────────────────

/// 攻方在 `[0,64,0]`、目标在 `[1,64,0]`、无自定义 Look 时 raycast 默认正面近距离
/// 命中 Chest（`dead_armor_multi_region_set_chest_is_blocked` 等既有测试已锁定
/// 这条几何前提），故本测试把变异挂在 `BodySlot::Torso`（humanoid.json 映射到
/// legacy `Chest`）以获得确定性命中部位。
fn run_chest_hit_and_read_severity(
    mutation_state: Option<crate::dandao::mutation::MutationState>,
) -> f32 {
    let mut app = make_arm_wound_app();
    let attacker = spawn_player(
        &mut app,
        "MutDmgAtk",
        [0.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    let target = spawn_player(
        &mut app,
        "MutDmgTarget",
        [1.0, 64.0, 0.0],
        Wounds::default(),
        Stamina::default(),
    );
    if let Some(state) = mutation_state {
        app.world_mut().entity_mut(target).insert(state);
    }

    send_qi_attack(&mut app, attacker, target, 7700);
    app.update();

    let events: Vec<_> = app
        .world()
        .resource::<Events<CombatEvent>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(
        events.len(),
        1,
        "期望本次攻击恰好产生 1 个 CombatEvent；实际 {}",
        events.len()
    );
    assert_eq!(
        events[0].body_part,
        BodyPart::Chest,
        "测试前提：正面近距离攻击必须命中 Chest（否则本用例的 BodySlot::Torso 挂载点\
             对不上命中部位，断言会失去意义）；实际命中 {:?}",
        events[0].body_part
    );

    let target_ref = app.world().entity(target);
    let wounds = target_ref
        .get::<Wounds>()
        .expect("target must retain Wounds component");
    wounds
        .entries
        .iter()
        .find(|w| w.location == crate::body_plan::legacy_body_part_to_id(BodyPart::Chest))
        .expect("命中 Chest 必须写入一条 Chest 位置的 Wound")
        .severity
}

#[test]
fn mutation_damage_reduction_reduces_matching_hit_body_part_severity_end_to_end() {
    use crate::dandao::components::MutationStage;
    use crate::dandao::mutation::{ActiveMutation, MutationKind, MutationState};

    let baseline_severity = run_chest_hit_and_read_severity(None);
    assert!(
        baseline_severity > 0.0,
        "测试前提：无变异时命中 Chest 必须造成非零伤害才能观测折算比例，实际 {baseline_severity}"
    );

    let mutated_state = MutationState {
        stage: MutationStage::Heavy,
        slots: vec![ActiveMutation {
            kind: MutationKind::SpineSpurs, // effect() = DamageReduction { reduction_pct: 0.20, .. }
            slot: crate::dandao::mutation::BodySlot::Torso, // humanoid.json 映射到 chest
            level: 1,
            acquired_tick: 0,
        }],
        meridian_penalty: 0.0,
    };
    let mutated_severity = run_chest_hit_and_read_severity(Some(mutated_state));

    let ratio = mutated_severity / baseline_severity;
    assert!(
        (ratio - 0.80).abs() < 0.01,
        "目标挂载 DamageReduction(20%) 变异（BodySlot::Torso 经 mutation_slot_mapping \
             解析为 legacy Chest）后，命中 Chest 的伤害应打八折，实际比例 {ratio:.4}\
             （baseline={baseline_severity}, mutated={mutated_severity}）——若失败说明 \
             `resolve_attack_intents` 未真正消费 mutation_slot_mapping 解析结果"
    );
}

#[test]
fn mutation_damage_reduction_does_not_affect_non_matching_hit_body_part_end_to_end() {
    use crate::dandao::components::MutationStage;
    use crate::dandao::mutation::{ActiveMutation, MutationKind, MutationState};

    // 变异挂在 BodySlot::Back（映射到 legacy Back），但本测试的攻击几何恒定命中
    // Chest——命中部位与变异挂载部位不一致时，伤害不应受任何影响。
    let baseline_severity = run_chest_hit_and_read_severity(None);
    let mismatched_state = MutationState {
        stage: MutationStage::Heavy,
        slots: vec![ActiveMutation {
            kind: MutationKind::SpineSpurs,
            slot: crate::dandao::mutation::BodySlot::Back,
            level: 1,
            acquired_tick: 0,
        }],
        meridian_penalty: 0.0,
    };
    let mismatched_severity = run_chest_hit_and_read_severity(Some(mismatched_state));

    assert!(
        (mismatched_severity - baseline_severity).abs() < 1e-4,
        "变异挂载部位（Back）与命中部位（Chest）不一致时不应产生任何折算，\
             baseline={baseline_severity} mismatched={mismatched_severity}"
    );
}

// ══════════════════════════════════════════════════════════════════════════
// plan-race-system-v1 P0 review r2（BLOCKING-2 收口）—— dispatch_part_consequence
// 决策纯函数饱和测试：happy path × 4 个 PartConsequence 变体 × 阈值上下边界 +
// 未知 part id（这条分支在真实几何管线下不可达，见函数文档，只能在这个层级直接
// 单测锁死）。
// ══════════════════════════════════════════════════════════════════════════
mod dispatch_part_consequence_tests {
    use super::*;
    use crate::body_plan::types::{BodyPartDef, HitGeometry, PartConsequence, StandingAabbSpec};
    use crate::body_plan::BodyPartId;
    use std::collections::HashMap;

    /// 四类 `PartConsequence` 各一个部位的合成非人形构型（`hit_geometry` 本身在
    /// 本测试模块不参与求交，随便给一个合法值即可满足 `validate_body_plan`）。
    fn four_consequence_plan() -> crate::body_plan::BodyPlan {
        crate::body_plan::BodyPlan {
            id: "test_dispatch_consequence".into(),
            display_name: "测试用四类后果构型".to_string(),
            is_humanoid: false,
            parts: vec![
                BodyPartDef {
                    id: "claw".into(),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Manipulator { main_hand: true },
                },
                BodyPartDef {
                    id: "off_claw".into(),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Manipulator { main_hand: false },
                },
                BodyPartDef {
                    id: "fin".into(),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Locomotion,
                },
                BodyPartDef {
                    id: "eye".into(),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Sensory,
                },
                BodyPartDef {
                    id: "shell".into(),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Core,
                },
            ],
            hit_geometry: HitGeometry::HeightBands {
                aabb: StandingAabbSpec {
                    half_width: 0.3,
                    height: 1.8,
                },
                bands: vec![crate::body_plan::HeightBand {
                    min_rel_y: -1.0,
                    assignment: crate::body_plan::HeightBandAssignment::Single {
                        part: "shell".into(),
                    },
                }],
                lateral_threshold: 0.19,
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: HashMap::new(),
        }
    }

    fn manipulator_main_hand_severed_dispatches_sever_outcome() {
        let plan = four_consequence_plan();
        assert_eq!(
            dispatch_part_consequence(&plan, &BodyPartId::new("claw"), 70.0),
            PartConsequenceOutcome::SeverMainHandManipulator,
            "主手 Manipulator 命中且 severity 达到 Severed 分级（70.0）必须脱手"
        );
    }

    fn manipulator_main_hand_below_severed_threshold_is_no_consequence() {
        let plan = four_consequence_plan();
        assert_eq!(
            dispatch_part_consequence(&plan, &BodyPartId::new("claw"), 69.999),
            PartConsequenceOutcome::NoConsequence,
            "主手 Manipulator 命中但未到 Severed 分级（<70.0）不应脱手"
        );
    }

    fn manipulator_off_hand_never_severs_regardless_of_severity() {
        let plan = four_consequence_plan();
        assert_eq!(
            dispatch_part_consequence(&plan, &BodyPartId::new("off_claw"), 999.0),
            PartConsequenceOutcome::NoConsequence,
            "副手 Manipulator（main_hand:false）即便伤势极高也不应触发脱手——\
                 脱手判定只认主手"
        );
    }

    fn locomotion_at_or_above_slow_threshold_dispatches_leg_slow() {
        let plan = four_consequence_plan();
        assert_eq!(
            dispatch_part_consequence(
                &plan,
                &BodyPartId::new("fin"),
                LEG_SLOWED_SEVERITY_THRESHOLD
            ),
            PartConsequenceOutcome::ApplyLegSlow,
            "Locomotion 命中且 severity 恰好等于阈值（闭区间）应触发减速"
        );
    }

    fn locomotion_below_slow_threshold_is_no_consequence() {
        let plan = four_consequence_plan();
        assert_eq!(
            dispatch_part_consequence(
                &plan,
                &BodyPartId::new("fin"),
                LEG_SLOWED_SEVERITY_THRESHOLD - 0.001
            ),
            PartConsequenceOutcome::NoConsequence,
            "Locomotion 命中但 severity 未达阈值不应触发减速"
        );
    }

    fn sensory_at_or_above_stun_threshold_dispatches_head_stun() {
        let plan = four_consequence_plan();
        assert_eq!(
            dispatch_part_consequence(&plan, &BodyPartId::new("eye"), HEAD_STUN_SEVERITY_THRESHOLD),
            PartConsequenceOutcome::ApplyHeadStun,
            "Sensory 命中且 severity 恰好等于阈值（闭区间）应触发眩晕"
        );
    }

    fn sensory_below_stun_threshold_is_no_consequence() {
        let plan = four_consequence_plan();
        assert_eq!(
            dispatch_part_consequence(
                &plan,
                &BodyPartId::new("eye"),
                HEAD_STUN_SEVERITY_THRESHOLD - 0.001
            ),
            PartConsequenceOutcome::NoConsequence,
            "Sensory 命中但 severity 未达阈值不应触发眩晕"
        );
    }

    fn core_never_dispatches_any_limb_consequence_regardless_of_severity() {
        let plan = four_consequence_plan();
        for severity in [0.0_f32, 0.3, 0.5, 70.0, 9999.0] {
            assert_eq!(
                dispatch_part_consequence(&plan, &BodyPartId::new("shell"), severity),
                PartConsequenceOutcome::NoConsequence,
                "Core 命中在任意 severity（{severity}）下都不应触发脱手/减速/眩晕"
            );
        }
    }

    fn unknown_part_id_dispatches_explicit_unknown_outcome() {
        // 命中几何与部位定义理论上同出一份 BodyPlan，这条分支在真实攻击管线里
        // 不可达（见 dispatch_part_consequence 文档）——但决策函数本身必须对
        // "根本不认识的部位 id" 显式返回 UnknownPart，而不是默默当 NoConsequence
        // 处理掉（两者调用方后续行为不同：UnknownPart 要 warn）。
        let plan = four_consequence_plan();
        assert_eq!(
            dispatch_part_consequence(&plan, &BodyPartId::new("does_not_exist"), 999.0),
            PartConsequenceOutcome::UnknownPart,
            "未知 part id 必须显式返回 UnknownPart，不能被悄悄归为 NoConsequence"
        );
    }

    #[test]
    fn dispatch_part_consequence_contract_matrix() {
        let cases: [(&str, fn()); 9] = [
            (
                "manipulator_main_hand_severed_dispatches_sever_outcome",
                manipulator_main_hand_severed_dispatches_sever_outcome,
            ),
            (
                "manipulator_main_hand_below_severed_threshold_is_no_consequence",
                manipulator_main_hand_below_severed_threshold_is_no_consequence,
            ),
            (
                "manipulator_off_hand_never_severs_regardless_of_severity",
                manipulator_off_hand_never_severs_regardless_of_severity,
            ),
            (
                "locomotion_at_or_above_slow_threshold_dispatches_leg_slow",
                locomotion_at_or_above_slow_threshold_dispatches_leg_slow,
            ),
            (
                "locomotion_below_slow_threshold_is_no_consequence",
                locomotion_below_slow_threshold_is_no_consequence,
            ),
            (
                "sensory_at_or_above_stun_threshold_dispatches_head_stun",
                sensory_at_or_above_stun_threshold_dispatches_head_stun,
            ),
            (
                "sensory_below_stun_threshold_is_no_consequence",
                sensory_below_stun_threshold_is_no_consequence,
            ),
            (
                "core_never_dispatches_any_limb_consequence_regardless_of_severity",
                core_never_dispatches_any_limb_consequence_regardless_of_severity,
            ),
            (
                "unknown_part_id_dispatches_explicit_unknown_outcome",
                unknown_part_id_dispatches_explicit_unknown_outcome,
            ),
        ];
        for (name, case) in cases {
            let result = std::panic::catch_unwind(case);
            assert!(
                result.is_ok(),
                "dispatch_part_consequence case '{name}' failed"
            );
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════
// plan-race-system-v1 P0 review r2（BLOCKING-2 收口）—— 非人形构型伤残后果状态
// 转换集成测试：合成非人形 BodyPlan 走真实 resolve_attack_intents 全链路，验证
// 四类 PartConsequence 各自的**外部可观察**后果（status effect 事件 / 装备被移除），
// 而不是只测 dispatch_part_consequence 这个决策函数本身。
// ══════════════════════════════════════════════════════════════════════════
mod non_humanoid_consequence_integration_tests {
    use super::*;
    use crate::body_plan::race_registry::RaceEntry;
    use crate::body_plan::types::{BodyPartDef, HitGeometry, PartBox, PartConsequence};
    use crate::body_plan::BodyPartId;
    use std::collections::HashMap;

    /// 单部位合成构型：复用 plan-race-system-v1 PartBoxes 集成测试同款已核验几何
    /// （攻方 feet=[-2,64,0] 无 Look 回落 chest_aim_direction、目标 feet=[0,64,0]
    /// 默认朝向 yaw=0、局部盒偏移 [-1,1.2,0]、reach=FIST_REACH.max=2.0、求交距离
    /// 0.5619966636911647 blocks），只替换 `consequence`/`damage_mul`，几何行为
    /// bit-for-bit 与 `partboxes_production_integration_tests` 一致。
    fn single_part_plan(
        part_id: &str,
        consequence: PartConsequence,
        damage_mul: f32,
    ) -> crate::body_plan::BodyPlan {
        crate::body_plan::BodyPlan {
            id: format!("test_single_part_{part_id}").into(),
            display_name: "测试单部位构型".to_string(),
            is_humanoid: false,
            parts: vec![BodyPartDef {
                id: part_id.into(),
                damage_mul,
                contam_mul: 1.0,
                bleed_mul: 1.0,
                consequence,
            }],
            hit_geometry: HitGeometry::PartBoxes {
                boxes: vec![PartBox {
                    part_id: part_id.into(),
                    offset: [-1.0, 1.2, 0.0],
                    half_extents: [0.45, 0.45, 0.45],
                    priority: 0,
                }],
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: HashMap::new(),
        }
    }

    fn single_part_registries(
        plan: crate::body_plan::BodyPlan,
    ) -> (BodyPlanRegistry, RaceRegistry) {
        let plan_id = plan.id.clone();
        let body_plans =
            BodyPlanRegistry::from_plans(vec![plan]).expect("single-part plan must validate");
        let races = RaceRegistry::from_parts_for_test(
            vec![RaceEntry {
                id: crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                display_name: "单部位测试替身".to_string(),
                body_plan_id: plan_id,
                beast_kinds: vec![],
            }],
            vec![],
            &body_plans,
        )
        .expect("races fixture must validate");
        (body_plans, races)
    }

    /// 组装最小 App：合成 registries + `resolve_attack_intents` + 攻防双方玩家
    /// （几何与 `partboxes_production_integration_tests::setup_alien_carrier_app`
    /// 完全相同）。调用方负责 `send_event(AttackIntent)` + `app.update()`。
    fn setup_single_part_app(plan: crate::body_plan::BodyPlan) -> (App, Entity, Entity) {
        let (body_plans, races) = single_part_registries(plan);
        let mut app = qi_test_app();
        app.insert_resource(CombatClock { tick: 700 });
        app.insert_resource(body_plans);
        app.insert_resource(races);
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<crate::combat::weapon::ShieldBlockHit>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "SinglePartAttacker",
            [-2.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "SinglePartTarget",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        (app, attacker, target)
    }

    fn send_single_part_attack(app: &mut App, attacker: Entity, qi_invest: f32) {
        app.world_mut().send_event(AttackIntent {
            attacker,
            target: None,
            issued_at_tick: 699,
            reach: FIST_REACH,
            qi_invest,
            wound_kind: WoundKind::Cut,
            source: AttackSource::Melee,
            debug_command: Some(crate::player::gameplay::CombatAction {
                target: "SinglePartTarget".to_string(),
                qi_invest: f64::from(qi_invest),
            }),
        });
        app.update();
    }

    #[test]
    fn locomotion_hit_applies_slowed_status_effect() {
        let plan = single_part_plan("fin", PartConsequence::Locomotion, 1.0);
        let (mut app, attacker, target) = setup_single_part_app(plan);
        send_single_part_attack(&mut app, attacker, 10.0);

        let wounds = app.world().entity(target).get::<Wounds>().unwrap();
        assert_eq!(wounds.entries.len(), 1, "应恰好写入一条 Wound");
        assert_eq!(wounds.entries[0].location, BodyPartId::new("fin"));

        let slow_intents: Vec<_> = app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .iter_current_update_events()
            .filter(|intent| intent.target == target)
            .collect();
        assert!(
            slow_intents
                .iter()
                .any(|intent| intent.kind == StatusEffectKind::Slowed),
            "命中 Locomotion 部位且伤势达阈值应对目标施加 Slowed 状态效果（外部可\
                 观察后果），实测意图列表：{slow_intents:?}"
        );
    }

    #[test]
    fn sensory_hit_applies_stunned_status_effect() {
        let plan = single_part_plan("eye", PartConsequence::Sensory, 1.0);
        let (mut app, attacker, target) = setup_single_part_app(plan);
        send_single_part_attack(&mut app, attacker, 10.0);

        let wounds = app.world().entity(target).get::<Wounds>().unwrap();
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(wounds.entries[0].location, BodyPartId::new("eye"));

        let stun_intents: Vec<_> = app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .iter_current_update_events()
            .filter(|intent| intent.target == target)
            .collect();
        assert!(
            stun_intents
                .iter()
                .any(|intent| intent.kind == StatusEffectKind::Stunned),
            "命中 Sensory 部位且伤势达阈值应对目标施加 Stunned 状态效果（外部可\
                 观察后果），实测意图列表：{stun_intents:?}"
        );
    }

    #[test]
    fn core_hit_applies_neither_slowed_nor_stunned() {
        let plan = single_part_plan("shell", PartConsequence::Core, 1.0);
        let (mut app, attacker, target) = setup_single_part_app(plan);
        send_single_part_attack(&mut app, attacker, 10.0);

        let wounds = app.world().entity(target).get::<Wounds>().unwrap();
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(wounds.entries[0].location, BodyPartId::new("shell"));

        let limb_intents: Vec<_> = app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .iter_current_update_events()
            .filter(|intent| intent.target == target)
            .collect();
        assert!(
            !limb_intents.iter().any(|intent| matches!(
                intent.kind,
                StatusEffectKind::Slowed | StatusEffectKind::Stunned
            )),
            "命中 Core 部位不应触发减速/眩晕这两条肢体功能性后果，实测意图列表：\
                 {limb_intents:?}"
        );
    }

    #[test]
    fn manipulator_main_hand_severed_drops_weapon_for_non_humanoid_plan() {
        // damage_mul=10.0 + qi_invest 接近 spawn_player 给的 60.0 全额真元预算，
        // 确保伤势稳超 Severed 分级阈值（70.0）——留足浮点/常量裕度。
        let plan = single_part_plan(
            "claw",
            PartConsequence::Manipulator { main_hand: true },
            10.0,
        );
        let (mut app, attacker, target) = setup_single_part_app(plan);
        app.insert_resource(weapon_test_registry());
        app.insert_resource(DroppedLootRegistry::default());
        equip_main_hand_weapon(&mut app, target, 70601);

        send_single_part_attack(&mut app, attacker, 60.0);

        let wounds = app.world().entity(target).get::<Wounds>().unwrap();
        assert_eq!(wounds.entries.len(), 1, "应恰好写入一条 Wound");
        assert_eq!(wounds.entries[0].location, BodyPartId::new("claw"));
        assert_eq!(
            arm_wound::wound_severity_to_grade(wounds.entries[0].severity),
            arm_wound::ArmWoundGrade::Severed,
            "本次命中伤势应达到 Severed 分级（前置条件），实测 severity={}",
            wounds.entries[0].severity
        );

        assert!(
            app.world().entity(target).get::<Weapon>().is_none(),
            "非人形构型的 Manipulator{{main_hand:true}} 部位 Severed 后，Weapon \
                 runtime component 应被 remove（脱手）——外部可观察后果，不局限于\
                 legacy ArmR/humanoid 才生效"
        );
        let dropped_registry = app.world().resource::<DroppedLootRegistry>();
        let dropped = dropped_registry.entries.get(&70601).expect(
            "断臂脱手的武器 instance 应出现在 DroppedLootRegistry（世界掉落），\
                 而不是被静默丢弃",
        );
        assert_eq!(dropped.instance_id, 70601);
    }
}

// ══════════════════════════════════════════════════════════════════════════
// plan-race-system-v1 P0 review r2（BLOCKING-1 收口）—— PartBoxes 命中几何
// 生产集成测试：合成非人形构型（is_humanoid=false，hit_geometry=PartBoxes，
// 部位 id 均非 legacy 8 段字符串）走真实 resolve_attack_intents 全链路
// （AttackIntent → resolve_body_plan_for_target → raycast_humanoid 的
// PartBoxes 分支 → Wound 写入 → body_part_multipliers 伤害倍率），而非直接
// 单元调用几何函数——覆盖旋转（target Look yaw 90/180）、最近命中、无命中
// 三分支，外加"命中结果真的驱动了该 plan 的 BodyPartDef 伤害倍率"。
// 场景数值（攻方 feet=[-2,64,0]、目标 feet=[0,64,0]、瞄准点=目标胸高
// 回落 [0,64+1.2,0]、reach=FIST_REACH.max=2.0）已用独立 Python 复刻本文件
// 同款 slab/rotate 数学离线核验，见 commit 说明。
mod partboxes_production_integration_tests {
    use super::*;
    use crate::body_plan::race_registry::RaceEntry;
    use crate::body_plan::types::{BodyPartDef, HitGeometry, PartBox, PartConsequence};
    use crate::body_plan::BodyPartId;
    use std::collections::HashMap;

    /// 合成外星构型："左钳"/"右钳"/"尾鳍"三个局部盒沿目标局部系左/右/后分布，
    /// 部位 id 全部是非 legacy 8 段字符串（不能反压 `combat::components::BodyPart`）。
    /// `damage_mul` 刻意拉开 25 倍差距（5.0 vs 0.2），使"命中部位驱动伤害倍率"这条
    /// 断言即便撞上 `damage.max(1.0)` 下限也仍能看出方向性差异。
    fn alien_carrier_plan() -> crate::body_plan::BodyPlan {
        crate::body_plan::BodyPlan {
            id: "test_alien_carrier".into(),
            display_name: "测试用外星载具构型".to_string(),
            is_humanoid: false,
            parts: vec![
                BodyPartDef {
                    id: "left_pincer".into(),
                    damage_mul: 5.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Sensory,
                },
                BodyPartDef {
                    id: "right_pincer".into(),
                    damage_mul: 0.2,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Core,
                },
                BodyPartDef {
                    id: "tail_fin".into(),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Locomotion,
                },
            ],
            hit_geometry: HitGeometry::PartBoxes {
                boxes: vec![
                    PartBox {
                        part_id: "left_pincer".into(),
                        offset: [-1.0, 1.2, 0.0],
                        half_extents: [0.45, 0.45, 0.45],
                        priority: 0,
                    },
                    PartBox {
                        part_id: "right_pincer".into(),
                        offset: [1.0, 1.2, 0.0],
                        half_extents: [0.45, 0.45, 0.45],
                        priority: 0,
                    },
                    PartBox {
                        part_id: "tail_fin".into(),
                        offset: [0.0, 1.2, -1.0],
                        half_extents: [0.45, 0.45, 0.45],
                        priority: 0,
                    },
                ],
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: HashMap::new(),
        }
    }

    /// 最近命中专用构型：两个局部盒同心轴线上前后排列（`far_shoulder` 更靠近攻方
    /// 出发点、`near_edge` 更远——见下方 `near_edge`/`far_shoulder` 偏移量注释），
    /// 用于证明 `PartBoxes` 求交在多个候选命中时选**距离更近**的那个。
    fn alien_carrier_nearest_plan() -> crate::body_plan::BodyPlan {
        crate::body_plan::BodyPlan {
            id: "test_alien_carrier_nearest".into(),
            display_name: "测试用最近命中构型".to_string(),
            is_humanoid: false,
            parts: vec![
                BodyPartDef {
                    id: "far_shoulder".into(),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Core,
                },
                BodyPartDef {
                    id: "near_edge".into(),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Core,
                },
            ],
            hit_geometry: HitGeometry::PartBoxes {
                boxes: vec![
                    // 攻方沿局部 -X 方向逼近（见测试内攻方/目标坐标），此盒偏移量
                    // 绝对值更小 → 局部系里离目标中心更近、但离攻方出发点更远，
                    // 求交距离更大（1.12 blocks，独立 Python 核验）。
                    PartBox {
                        part_id: "far_shoulder".into(),
                        offset: [-0.6, 1.2, 0.0],
                        half_extents: [0.2, 0.4, 0.4],
                        priority: 0,
                    },
                    // 偏移量绝对值更大 → 离攻方出发点更近，求交距离更小
                    // （0.51 blocks）——必须是这个盒赢，而不是数组声明顺序在后的
                    // `far_shoulder` 或先声明的顺序假象。
                    PartBox {
                        part_id: "near_edge".into(),
                        offset: [-1.3, 1.2, 0.0],
                        half_extents: [0.2, 0.4, 0.4],
                        priority: 0,
                    },
                ],
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: HashMap::new(),
        }
    }

    /// races.json fixture：把 `HUMAN_RACE_ID`（`Cultivation::default().race` 恒指向
    /// 此 id）改写指向传入的合成 `plan`——`spawn_player` 构造的目标实体因此在本测试
    /// 范围内解析出该合成 `BodyPlan`，不需要额外的种族/组件改动。
    fn alien_carrier_registries(
        plan: crate::body_plan::BodyPlan,
    ) -> (BodyPlanRegistry, RaceRegistry) {
        let plan_id = plan.id.clone();
        let body_plans =
            BodyPlanRegistry::from_plans(vec![plan]).expect("alien carrier plan must validate");
        let races = RaceRegistry::from_parts_for_test(
            vec![RaceEntry {
                id: crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                display_name: "外星人族测试替身".to_string(),
                body_plan_id: plan_id,
                beast_kinds: vec![],
            }],
            vec![],
            &body_plans,
        )
        .expect("races fixture must validate");
        (body_plans, races)
    }

    /// plan-race-system-v1 bughunt major-1 修复后的 reach：这组测试的目的是纯粹
    /// 验证"目标 yaw 旋转是否真的驱动 `PartBoxes` 局部系变换"，不是验证战斗 reach
    /// 数值本身——按修复前错误符号约定凑出来的 `FIST_REACH.max=2.0` 只够在旧
    /// （错误）局部系下命中，修复符号后同一条世界系射线在正确局部系里到 `tail_fin`
    /// 的距离变长（约 2.6~3.5，见下方 yaw=90°/270° 测试注释的推导），必须放宽
    /// reach 才能继续验出"转向确实改变命中部位"这个目标行为，而不是被无关的 reach
    /// 上限提前截断成假阴性。6.6 留有充分余量覆盖全部 4 个象限。
    const ALIEN_CARRIER_TEST_REACH: AttackReach = AttackReach::new(6.0, 0.6);

    /// 攻方 feet=[-2,64,0]（无 Look，回落 chest_aim_direction）、目标 feet=[0,64,0]
    /// （按 `target_look_yaw_degrees` 显式设置朝向）。`resolve_attack_intents` 用
    /// `debug_command` 直接按用户名定向目标（跳过近战朝向锥搜索），但命中几何
    /// 仍走真实 `raycast_humanoid` + `intent.reach`（`ALIEN_CARRIER_TEST_REACH`，
    /// 见上方常量注释）。
    fn setup_alien_carrier_app(
        plan: crate::body_plan::BodyPlan,
        target_look_yaw_degrees: f32,
    ) -> (App, Entity) {
        let (body_plans, races) = alien_carrier_registries(plan);
        let mut app = qi_test_app();
        app.insert_resource(CombatClock { tick: 500 });
        app.insert_resource(body_plans);
        app.insert_resource(races);
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<crate::combat::weapon::ShieldBlockHit>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "AlienAttacker",
            [-2.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "AlienTarget",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut().entity_mut(target).insert(Look {
            yaw: target_look_yaw_degrees,
            pitch: 0.0,
        });

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: None,
            issued_at_tick: 499,
            reach: ALIEN_CARRIER_TEST_REACH,
            qi_invest: 0.0,
            wound_kind: WoundKind::Cut,
            source: AttackSource::Melee,
            debug_command: Some(crate::player::gameplay::CombatAction {
                target: "AlienTarget".to_string(),
                qi_invest: 0.0,
            }),
        });
        app.update();

        (app, target)
    }

    #[test]
    fn partboxes_hit_at_target_yaw_zero_resolves_left_pincer_and_flows_into_damage() {
        let (app, target) = setup_alien_carrier_app(alien_carrier_plan(), 0.0);
        let wounds = app
            .world()
            .entity(target)
            .get::<Wounds>()
            .expect("target should keep wounds");
        assert_eq!(
            wounds.entries.len(),
            1,
            "PartBoxes 命中应写入恰好一条 Wound（生产入口未接线时这里会是 0 条）"
        );
        assert_eq!(
            wounds.entries[0].location,
            BodyPartId::new("left_pincer"),
            "yaw=0 时攻方沿目标局部 -X 逼近应命中 left_pincer，实测 {:?}",
            wounds.entries[0].location
        );
    }

    #[test]
    fn partboxes_hit_rotates_with_target_yaw_90_degrees() {
        // plan-race-system-v1 bughunt major-1：与 valence `Look::to_vec()`
        // 约定对齐后，yaw=90° 时攻方这条固定世界系射线在目标局部系里到 `tail_fin`
        // 的实际距离约 2.6~3.5（推导见 `ALIEN_CARRIER_TEST_REACH` 注释），比修复前
        // 错误符号约定下的 ~0.56~1.48 更远——`tail_fin` 仍是唯一在射线路径上的盒
        // （left/right pincer 的局部 x 范围此时被旋转到射线轨迹之外），只是需要
        // 放宽 reach 才能验出来。
        let (app, target) = setup_alien_carrier_app(alien_carrier_plan(), 90.0);
        let wounds = app
            .world()
            .entity(target)
            .get::<Wounds>()
            .expect("target should keep wounds");
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(
            wounds.entries[0].location,
            BodyPartId::new("tail_fin"),
            "目标转 yaw=90° 后，同一条世界系攻击射线应改命中 tail_fin（局部盒随目标\
                 朝向旋转），实测 {:?}——若仍是 left_pincer 说明 target 朝向没有真正接入\
                 PartBoxes 分派",
            wounds.entries[0].location
        );
    }

    #[test]
    fn partboxes_hit_rotates_with_target_yaw_270_degrees() {
        // 第四象限 pin（配合 yaw=0/90/180 补齐四象限覆盖）：yaw=270° 下目标局部系
        // 相对攻方射线的朝向与"yaw=90° 但符号取反"等价，命中距离落回近距离
        // （~0.56~1.48），同样应命中 tail_fin——与 yaw=90° 对照，证明两个象限
        // 不是靠同一套符号巧合各自蒙对。
        let (app, target) = setup_alien_carrier_app(alien_carrier_plan(), 270.0);
        let wounds = app
            .world()
            .entity(target)
            .get::<Wounds>()
            .expect("target should keep wounds");
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(
            wounds.entries[0].location,
            BodyPartId::new("tail_fin"),
            "目标转 yaw=270° 后应命中 tail_fin，实测 {:?}",
            wounds.entries[0].location
        );
    }

    #[test]
    fn partboxes_hit_rotates_with_target_yaw_180_degrees() {
        let (app, target) = setup_alien_carrier_app(alien_carrier_plan(), 180.0);
        let wounds = app
            .world()
            .entity(target)
            .get::<Wounds>()
            .expect("target should keep wounds");
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(
            wounds.entries[0].location,
            BodyPartId::new("right_pincer"),
            "目标转 yaw=180° 后应改命中 right_pincer（与 yaw=0 的 left_pincer 相对），\
                 实测 {:?}",
            wounds.entries[0].location
        );
    }

    #[test]
    fn partboxes_no_hit_when_target_yaw_rotates_all_boxes_off_the_ray() {
        // yaw=45° 时三个盒相对本测试固定的攻击射线全部落空（独立 Python 核验）——
        // 命中入口必须显式返回 None、不产生 Wound，而不是兜底命中任意部位。
        let (app, target) = setup_alien_carrier_app(alien_carrier_plan(), 45.0);
        let wounds = app
            .world()
            .entity(target)
            .get::<Wounds>()
            .expect("target should keep wounds");
        assert!(
            wounds.entries.is_empty(),
            "yaw=45° 时三个 PartBox 均应落空，不应凭空产生 Wound，实测 {:?}",
            wounds.entries
        );
    }

    #[test]
    fn partboxes_raycast_picks_nearest_of_two_candidate_boxes() {
        let (app, target) = setup_alien_carrier_app(alien_carrier_nearest_plan(), 0.0);
        let wounds = app
            .world()
            .entity(target)
            .get::<Wounds>()
            .expect("target should keep wounds");
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(
            wounds.entries[0].location,
            BodyPartId::new("near_edge"),
            "两个候选盒都在攻击射线上时必须选距离更近的 near_edge，而非声明顺序\
                 更靠前的 far_shoulder，实测 {:?}",
            wounds.entries[0].location
        );
    }

    #[test]
    fn partboxes_hit_part_damage_multiplier_flows_from_target_plan_not_a_global_constant() {
        // 同一套攻方/目标/reach/qi_invest 设置，唯一变量是"命中哪个部位"（靠
        // target yaw 0° vs 180° 切换 left_pincer(damage_mul=5.0) / right_pincer
        // (damage_mul=0.2)）——伤害应随命中部位的 BodyPartDef.damage_mul 变化，
        // 而不是恒定倍率（25 倍差距刻意拉大，即便撞 `damage.max(1.0)` 下限也能
        // 看出方向性：更高倍率部位的伤害必须严格更高）。
        let (app_left, target_left) = setup_alien_carrier_app(alien_carrier_plan(), 0.0);
        let severity_left = app_left
            .world()
            .entity(target_left)
            .get::<Wounds>()
            .expect("target should keep wounds")
            .entries[0]
            .severity;

        let (app_right, target_right) = setup_alien_carrier_app(alien_carrier_plan(), 180.0);
        let severity_right = app_right
            .world()
            .entity(target_right)
            .get::<Wounds>()
            .expect("target should keep wounds")
            .entries[0]
            .severity;

        assert!(
            severity_left > severity_right,
            "left_pincer（damage_mul=5.0）命中的伤害应严格高于 right_pincer\
                 （damage_mul=0.2）命中的伤害，实测 left={severity_left} right={severity_right}\
                 —— 若两者相等说明命中部位没有真正驱动 body_part_multipliers 查询该\
                 合成 plan 的 BodyPartDef 数据"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════
// plan-race-system-v1 P6b review major-5 收口——production 级集成测试：真实
// `resolve_attack_intents` 全链路（`AttackIntent` → raycast → `dugu_injection_channel`
// → `ContamSource` 写入）命中一个**真实声明了 `dugu_injection` 映射**的非人形
// `BodyPlan`，断言目标 `Contamination.entries` 里落地的 `meridian_id` 是该构型
// 自己的专属 channel（不是 `None`，也不是被压回某条 humanoid 经脉）——不像
// `dugu_contam_meridian_routing_tests` 那样只单元调用 `dugu_injection_channel`
// 本身，而是走完整生产链路（连同 `resolve_body_plan_for_target` 解析 + 命中几何
// + `ContamSource` 构造 + component 写回）。
mod dugu_contam_meridian_routing_production_integration_tests {
    use super::*;
    use crate::body_plan::race_registry::RaceEntry;
    use crate::body_plan::types::{
        BodyPartDef, ChannelDef, HitGeometry, MeridianFamily, MeridianProfile, PartBox,
        PartConsequence, RealmMeridianReq,
    };
    use crate::cultivation::components::MeridianChannelId;
    use std::collections::HashMap;

    /// 非人形合成构型："body" 单一部位（几何复用
    /// `partboxes_production_integration_tests::alien_carrier_plan` 已核验的
    /// `left_pincer` 命中盒/坐标），`meridian_profile.dugu_injection` 真实声明
    /// `body -> tail_core`（非 humanoid 20 经之一的专属 channel）。
    fn synthetic_beast_plan_with_dugu_mapping() -> crate::body_plan::BodyPlan {
        crate::body_plan::BodyPlan {
            id: "test_dugu_prod_synthetic_beast".into(),
            display_name: "测试用带 dugu 映射的合成兽形构型".to_string(),
            is_humanoid: false,
            parts: vec![BodyPartDef {
                id: "body".into(),
                damage_mul: 1.0,
                contam_mul: 1.0,
                bleed_mul: 1.0,
                consequence: PartConsequence::Core,
            }],
            hit_geometry: HitGeometry::PartBoxes {
                boxes: vec![PartBox {
                    part_id: "body".into(),
                    offset: [-1.0, 1.2, 0.0],
                    half_extents: [0.45, 0.45, 0.45],
                    priority: 0,
                }],
            },
            equip_slots: vec![],
            meridian_profile: Some(MeridianProfile {
                channels: vec![ChannelDef {
                    id: "tail_core".into(),
                    family: MeridianFamily::Extraordinary,
                    body_part: None,
                    roles: vec![],
                }],
                topology_edges: vec![],
                realm_requirements: [RealmMeridianReq {
                    total: 1,
                    regular_min: 0,
                    extraordinary_min: 0,
                }; 6],
                dugu_injection: vec![crate::body_plan::types::DuguInjectionEntry {
                    body_part: "body".into(),
                    channel: MeridianChannelId::new("tail_core"),
                }],
            }),
            mutation_slot_mapping: HashMap::new(),
        }
    }

    fn dugu_prod_registries(plan: crate::body_plan::BodyPlan) -> (BodyPlanRegistry, RaceRegistry) {
        let plan_id = plan.id.clone();
        let body_plans = BodyPlanRegistry::from_plans(vec![plan])
            .expect("dugu production test plan must validate");
        let races = RaceRegistry::from_parts_for_test(
            vec![RaceEntry {
                id: crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                display_name: "测试非人形替身种族".to_string(),
                body_plan_id: plan_id,
                beast_kinds: vec![],
            }],
            vec![],
            &body_plans,
        )
        .expect("dugu production test races fixture must validate");
        (body_plans, races)
    }

    #[test]
    fn resolve_attack_intents_routes_contamination_to_non_humanoid_target_own_channel() {
        let (body_plans, races) = dugu_prod_registries(synthetic_beast_plan_with_dugu_mapping());
        let mut app = qi_test_app();
        app.insert_resource(CombatClock { tick: 500 });
        app.insert_resource(body_plans);
        app.insert_resource(races);
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<crate::combat::weapon::ShieldBlockHit>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "DuguProdAttacker",
            [-2.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "DuguProdTarget",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        // target 本体种族解析出上面的合成非人形 plan（`RaceEntry` 把
        // `HUMAN_RACE_ID` 指向该 plan，`Cultivation::default().race` 恒等于
        // `HUMAN_RACE_ID`，`spawn_player` 不需要额外改动）。
        app.world_mut().entity_mut(target).insert(Look {
            yaw: 0.0,
            pitch: 0.0,
        });

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: None,
            issued_at_tick: 499,
            reach: AttackReach::new(6.0, 0.6),
            // 蛊毒污染只在**非物理**打击写入（resolve.rs `emitted_contam_delta =
            // if is_physical_hit { 0.0 }`，is_physical_hit = qi_invest <= EPSILON）。
            // 故这里必须走 qi 投入的非物理攻击，物理近战恒 0 污染、无法验证 channel 路由。
            qi_invest: 5.0,
            wound_kind: WoundKind::Cut,
            source: AttackSource::Melee,
            debug_command: Some(crate::player::gameplay::CombatAction {
                target: "DuguProdTarget".to_string(),
                qi_invest: 5.0,
            }),
        });
        app.update();

        let contamination = app
            .world()
            .entity(target)
            .get::<Contamination>()
            .expect("target should keep contamination after a valid attack");
        assert_eq!(
                contamination.entries.len(),
                1,
                "a valid qi (non-physical) hit on the synthetic beast's declared body part should write \
                 exactly one contamination entry"
            );
        assert_eq!(
            contamination.entries[0].meridian_id,
            Some(MeridianChannelId::new("tail_core")),
            "resolve_attack_intents must route the contamination entry to the target's own \
                 non-humanoid dugu_injection channel (tail_core) end-to-end through production \
                 wiring (resolve_body_plan_for_target → dugu_injection_channel → ContamSource), \
                 not silently drop it to None — actual: {:?}",
            contamination.entries[0].meridian_id
        );
    }
}

// ───────── plan-race-system-v1 P6b review BLOCKER 收口：ContamSource.meridian_id 经脉污染路由 ─────────
//
// `ContamSource.meridian_id` 构造已从 `id_to_legacy_body_part(...).map(dugu::body_part_to_meridian)`
// 换轨为直接持有 `dugu_injection_channel(target_body_plan, &hit_probe.part_id)`
// 的结果（`ContamSource.meridian_id` 本身已是 `MeridianChannelId`，见其字段文档；
// 不再经 `to_meridian_id()` 把非 humanoid 专属 channel 强制压回 legacy 枚举丢成
// `None`——那是本轮修的 BLOCKER）。本组测试直接锁死这条表达式本身的行为（不搭建
// 完整 ECS/combat 判定链路），覆盖 ①人形目标 8 部位 bit-for-bit 不变 ②非人形目标
// 无 dugu_injection 映射时显式 None ③非人形目标**确实声明**映射时路由到自己的
// 专属 channel（换轨前这里错误断言 None，把断链固化成了契约——已改为断言真实
// 路由结果）。
mod dugu_contam_meridian_routing_tests {
    use crate::body_plan::dugu_injection_channel;
    use crate::body_plan::types::{
        BodyPartDef, BodyPlan, BodyPlanId, ChannelDef, HeightBand, HeightBandAssignment,
        HitGeometry, MeridianFamily, MeridianProfile, PartConsequence, RealmMeridianReq,
        StandingAabbSpec,
    };
    use crate::cultivation::components::{MeridianChannelId, MeridianId};

    /// 换轨后的表达式——与 `resolve_attack_intents` 内 `ContamSource.meridian_id`
    /// 构造逐字符一致，测试直接复用而不是另起一套等价但可能悄悄漂移的逻辑。
    fn routed_meridian_id(
        plan: &BodyPlan,
        part_id: &crate::body_plan::BodyPartId,
    ) -> Option<MeridianChannelId> {
        dugu_injection_channel(plan, part_id)
    }

    /// 非人形合成 fixture（单一 "body" 部位 + 单 channel meridian_profile，
    /// `dugu_injection` 默认空 vec ——非人形构型未接入 dugu 玩法的合法状态）。
    /// 与 `cultivation::non_humanoid_meridian_synthetic_chain_test` 的鲸 fixture
    /// 同款风格，本模块独立持有一份以避免跨 `#[cfg(test)]` 私有模块可见性问题。
    fn synthetic_whale_plan_for_dugu_routing_test() -> BodyPlan {
        BodyPlan {
            id: BodyPlanId::new("synthetic_test_whale_dugu_routing"),
            display_name: "合成测试鲸（dugu 路由）".to_string(),
            is_humanoid: false,
            parts: vec![BodyPartDef {
                id: "body".into(),
                damage_mul: 1.0,
                contam_mul: 1.0,
                bleed_mul: 1.0,
                consequence: PartConsequence::Core,
            }],
            hit_geometry: HitGeometry::HeightBands {
                aabb: StandingAabbSpec {
                    half_width: 2.0,
                    height: 3.0,
                },
                bands: vec![HeightBand {
                    min_rel_y: -1.0,
                    assignment: HeightBandAssignment::Single {
                        part: "body".into(),
                    },
                }],
                lateral_threshold: 0.5,
            },
            equip_slots: vec![],
            meridian_profile: Some(MeridianProfile {
                channels: vec![ChannelDef {
                    id: "tail_core".into(),
                    family: MeridianFamily::Extraordinary,
                    body_part: None,
                    roles: vec![],
                }],
                topology_edges: vec![],
                realm_requirements: [RealmMeridianReq {
                    total: 1,
                    regular_min: 0,
                    extraordinary_min: 0,
                }; 6],
                dugu_injection: vec![],
            }),
            mutation_slot_mapping: Default::default(),
        }
    }

    #[test]
    fn humanoid_target_all_eight_parts_route_to_bit_for_bit_unchanged_meridian_id() {
        // 换轨前 `dugu::body_part_to_meridian` 对这 8 个 legacy BodyPart 的输出
        // （见 `cultivation::dugu::tests` 同款断言）——humanoid 行为回归 pin：
        // 任何一项漂移都说明新调用点破坏了既有真人玩家的 dugu 污染路由。
        let plan = crate::body_plan::humanoid_plan_static();
        let expected: [(&str, MeridianId); 8] = [
            ("head", MeridianId::Du),
            ("chest", MeridianId::Heart),
            ("back", MeridianId::Du),
            ("abdomen", MeridianId::Spleen),
            ("arm_l", MeridianId::LargeIntestine),
            ("arm_r", MeridianId::LargeIntestine),
            ("leg_l", MeridianId::Bladder),
            ("leg_r", MeridianId::Bladder),
        ];
        for (body_part, expected_id) in expected {
            let part_id = crate::body_plan::BodyPartId::new(body_part);
            assert_eq!(
                routed_meridian_id(plan, &part_id),
                Some(expected_id.channel_id()),
                "humanoid body_part={body_part} 换轨后必须仍解析出 {expected_id:?}\
                     （与换轨前 dugu::body_part_to_meridian 逐项 bit-for-bit 一致，\
                     以 snake_case channel id 表达）"
            );
        }
    }

    #[test]
    fn non_humanoid_target_without_dugu_injection_mapping_routes_to_explicit_none() {
        // 复用 P1 对抗审查合成鲸 fixture（`meridian_profile.dugu_injection` 为空
        // vec——非人形构型未接入 dugu 玩法时的合法状态，见 `DuguInjectionEntry`
        // 文档）。换轨前 `id_to_legacy_body_part` 对这类非 legacy 部位 id 恒返回
        // `None`，短路到同样的 `None` 结果——本测试锁死换轨后仍是显式 `None`
        // （污染量仍计入总量，只是不挂靠某条经脉），而不是 panic 或误挂到某条
        // humanoid 经脉上。
        let plan = synthetic_whale_plan_for_dugu_routing_test();
        let part_id = crate::body_plan::BodyPartId::new("body");
        assert_eq!(
            routed_meridian_id(&plan, &part_id),
            None,
            "非人形 body plan（无 dugu_injection 映射）命中应显式路由到 None，\
                 不能 panic 也不能误挂到某条 humanoid 经脉上"
        );
    }

    #[test]
    fn non_humanoid_target_with_declared_dugu_injection_mapping_routes_to_its_own_channel() {
        // review BLOCKER 收口：若某非人形 plan **确实**声明了 dugu_injection 映射
        // （哪怕映射目标 channel 不在 humanoid 20 经之列），换轨后必须真实路由到
        // 该专属 channel——不再被 `to_meridian_id()` 强制压回 legacy 枚举、因无
        // 对应物而丢成 `None`。换轨前这里错误断言 `None`，把"非人形专属 channel
        // 实际不可消费"这条断链固化成了测试契约（测试名字说"路由到自身 channel"，
        // 断言却要求 None）——现在断言真实路由结果，让 `tail_core` 真的能挂靠、
        // 被 `contamination_tick`/`resolve_crack_target` 消费。
        let mut plan = synthetic_whale_plan_for_dugu_routing_test();
        plan.meridian_profile.as_mut().unwrap().dugu_injection =
            vec![crate::body_plan::types::DuguInjectionEntry {
                body_part: crate::body_plan::BodyPartId::new("body"),
                channel: crate::cultivation::components::MeridianChannelId::new("tail_core"),
            }];
        let part_id = crate::body_plan::BodyPartId::new("body");
        assert_eq!(
            routed_meridian_id(&plan, &part_id),
            Some(MeridianChannelId::new("tail_core")),
            "非人形专属 channel（tail_core）没有 legacy MeridianId 对应物，但它是一个\
                 真实声明的 channel——换轨后必须路由到它自己，而不是被强制丢成 None"
        );
    }
}
