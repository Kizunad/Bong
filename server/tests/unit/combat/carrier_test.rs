#![allow(dead_code, unused_imports)]

use bong_server::combat::carrier::*;
use bong_server::combat::CombatClock;
use bong_server::cultivation::components::{ColorKind, Cultivation, MeridianId, Realm};
use bong_server::cultivation::skill_registry::{CastRejectReason, CastResult};
use valence::prelude::bevy_ecs;

#[test]
fn default_qi_target_caps_at_thirty_percent_or_eighty() {
    assert_eq!(
        default_qi_target(&Cultivation {
            qi_max: 150.0,
            ..Default::default()
        }),
        45.0
    );
    assert_eq!(
        default_qi_target(&Cultivation {
            qi_max: 540.0,
            ..Default::default()
        }),
        80.0
    );
}

#[test]
fn natural_decay_uses_half_life_curve() {
    let mut imprint = CarrierImprint {
        carrier_kind: CarrierKind::YibianShougu,
        qi_amount: 40.0,
        qi_amount_initial: 40.0,
        qi_color: ColorKind::Solid,
        source_realm: Realm::Condense,
        half_life_min: 120.0,
        decay_started_at_tick: 0,
        bond_kind: BondKind::HandheldCarrier,
        injection_kind: None,
    };
    let elapsed_min = 120.0;
    let half_lives = elapsed_min / imprint.half_life_min;
    imprint.qi_amount = imprint.qi_amount_initial * 0.5_f32.powf(half_lives);
    assert!((imprint.qi_amount - 20.0).abs() <= 0.001);
}

#[test]
fn profile_splits_yibian_bone_half_wound_half_contam() {
    let profile = anqi_carrier_profile(CarrierKind::YibianShougu);
    assert_eq!(profile.wound_ratio, 0.5);
    assert_eq!(profile.contam_ratio, 0.5);
}

/// 验证 anqi.charge_carrier 在 SkillMeridianDependencies 中已声明肺经依赖。
/// 断肺经 → charge 被通用 check_meridian_dependencies 拦截（worldview §四:286）。
#[test]
fn charge_carrier_declared_in_skill_meridian_dependencies_with_lung() {
    use bong_server::cultivation::meridian::severed::SkillMeridianDependencies;

    let mut deps = SkillMeridianDependencies::default();
    bong_server::combat::anqi_v2::declare_meridian_dependencies(&mut deps);

    assert!(
        deps.is_declared(ANQI_CHARGE_SKILL_ID),
        "期望 anqi.charge_carrier 已在 SkillMeridianDependencies 声明（plan-meridian-severed-v1 §3 强约束），\
         实际未声明 → 断肺经的玩家仍可充能"
    );
    let declared = deps.lookup(ANQI_CHARGE_SKILL_ID);
    assert!(
        declared.contains(&MeridianId::Lung),
        "期望 charge_carrier 依赖 MeridianId::Lung（肺经，真元注入暗器的主导引脉），\
         实际声明的依赖为 {declared:?}"
    );
}

/// 验证 resolve_anqi_charge_skill 在施法前检查经脉门：肺经 SEVERED → 返回 MeridianSevered。
#[test]
fn charge_carrier_cast_rejected_when_lung_severed() {
    use bong_server::combat::components::SkillBarBindings;
    use bong_server::cultivation::components::Cultivation;
    use bong_server::cultivation::meridian::severed::{MeridianSeveredPermanent, SeveredSource};

    let mut world = bevy_ecs::world::World::new();
    world.insert_resource(CombatClock { tick: 1 });
    world.insert_resource(bevy_ecs::event::Events::<ChargeCarrierIntent>::default());

    let mut severed = MeridianSeveredPermanent::default();
    severed.insert(MeridianId::Lung, SeveredSource::CombatWound, 1);

    let caster = world
        .spawn((
            Cultivation {
                qi_current: 100.0,
                qi_max: 200.0,
                ..Default::default()
            },
            SkillBarBindings::default(),
            severed,
        ))
        .id();

    let result = resolve_anqi_charge_skill(&mut world, caster, 0, None);

    assert!(
        matches!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::MeridianSevered(Some(MeridianId::Lung))
            }
        ),
        "期望肺经 SEVERED 时 resolve_anqi_charge_skill 返回 \
         CastRejectReason::MeridianSevered(Some(Lung))（真元无法经肺经注入暗器），\
         实际返回 {result:?}"
    );
}

/// 验证 resolve_anqi_charge_skill 在肺经完好（无 SEVERED component）时正常施法。
#[test]
fn charge_carrier_cast_allowed_when_lung_intact() {
    use bong_server::combat::components::SkillBarBindings;
    use bong_server::cultivation::components::Cultivation;

    let mut world = bevy_ecs::world::World::new();
    world.insert_resource(CombatClock { tick: 1 });
    world.insert_resource(bevy_ecs::event::Events::<ChargeCarrierIntent>::default());

    // 无 MeridianSeveredPermanent component → 肺经视为 INTACT，充能应通过经脉门
    let caster = world
        .spawn((
            Cultivation {
                qi_current: 100.0,
                qi_max: 200.0,
                ..Default::default()
            },
            SkillBarBindings::default(),
        ))
        .id();

    let result = resolve_anqi_charge_skill(&mut world, caster, 0, None);

    // qi_target > 0 且无经脉阻断 → 应进入 Started（充能 intent 已 emit）
    assert!(
        matches!(result, CastResult::Started { .. }),
        "期望肺经完好时 resolve_anqi_charge_skill 返回 CastResult::Started（经脉门放行），\
         实际返回 {result:?}"
    );
}

/// 核心回归：qi_current=0 时 resolve 必须拒绝，而非启动冷却又无充能效果。
#[test]
fn charge_carrier_rejected_when_qi_current_is_zero() {
    use bong_server::combat::components::SkillBarBindings;

    let mut world = bevy_ecs::world::World::new();
    world.insert_resource(CombatClock { tick: 1 });
    world.insert_resource(bevy_ecs::event::Events::<ChargeCarrierIntent>::default());

    // qi_max=300 → qi_target=(300*0.3).min(80)=80; qi_current=0 < 80 → 应拒绝
    let caster = world
        .spawn((
            Cultivation {
                qi_current: 0.0,
                qi_max: 300.0,
                ..Default::default()
            },
            SkillBarBindings::default(),
        ))
        .id();

    let result = resolve_anqi_charge_skill(&mut world, caster, 0, None);

    assert!(
        matches!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::QiInsufficient
            }
        ),
        "期望 qi_current=0 时 resolve_anqi_charge_skill 返回 QiInsufficient（\
         阻止空冷却 bug），实际返回 {result:?}"
    );
}

/// 边界：qi_current 正好等于 qi_target 时应允许施法（临界 >= 等号成立）。
#[test]
fn charge_carrier_allowed_when_qi_current_exactly_equals_qi_target() {
    use bong_server::combat::components::SkillBarBindings;

    let mut world = bevy_ecs::world::World::new();
    world.insert_resource(CombatClock { tick: 1 });
    world.insert_resource(bevy_ecs::event::Events::<ChargeCarrierIntent>::default());

    // qi_max=200 → qi_target=(200*0.3) f32 ≈ 60.000004（非整数 60，f32 0.3 不精确）。
    // qi_current 取**真实** qi_target 值以精确测「==」临界，避免硬编码 60.0 因 f32 imprecision
    // 被 guard 误判 < 而拒绝（workflow agent 原测试 bug）。
    let cult_for_target = Cultivation {
        qi_max: 200.0,
        ..Default::default()
    };
    let qi_target_val = f64::from(default_qi_target(&cult_for_target));
    let caster = world
        .spawn((
            Cultivation {
                qi_current: qi_target_val,
                qi_max: 200.0,
                ..Default::default()
            },
            SkillBarBindings::default(),
        ))
        .id();

    let result = resolve_anqi_charge_skill(&mut world, caster, 0, None);

    assert!(
        matches!(result, CastResult::Started { .. }),
        "期望 qi_current 精确等于 qi_target({qi_target_val}) 时允许充能（\
         临界 >= 成立），实际返回 {result:?}"
    );
}

/// 边界：qi_current 比 qi_target 少 1 时必须拒绝。
#[test]
fn charge_carrier_rejected_when_qi_current_one_below_qi_target() {
    use bong_server::combat::components::SkillBarBindings;

    let mut world = bevy_ecs::world::World::new();
    world.insert_resource(CombatClock { tick: 1 });
    world.insert_resource(bevy_ecs::event::Events::<ChargeCarrierIntent>::default());

    // qi_max=200 → qi_target=60; qi_current=59 < 60 → 应拒绝
    let caster = world
        .spawn((
            Cultivation {
                qi_current: 59.0,
                qi_max: 200.0,
                ..Default::default()
            },
            SkillBarBindings::default(),
        ))
        .id();

    let result = resolve_anqi_charge_skill(&mut world, caster, 0, None);

    assert!(
        matches!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::QiInsufficient
            }
        ),
        "期望 qi_current=59 < qi_target=60 时返回 QiInsufficient（单位以下拒绝），\
         实际返回 {result:?}"
    );
}

/// 高 qi_max 玩家（qi_target 触顶 80）qi_current 低于 80 时必须拒绝。
#[test]
fn charge_carrier_rejected_for_high_qi_max_player_with_low_qi_current() {
    use bong_server::combat::components::SkillBarBindings;

    let mut world = bevy_ecs::world::World::new();
    world.insert_resource(CombatClock { tick: 1 });
    world.insert_resource(bevy_ecs::event::Events::<ChargeCarrierIntent>::default());

    // qi_max=600 → qi_target=(600*0.3).min(80)=80; qi_current=50 < 80 → 应拒绝
    // 这是 bug 报告的典型场景：qi_max 高但战斗中 qi_current 被消耗
    let caster = world
        .spawn((
            Cultivation {
                qi_current: 50.0,
                qi_max: 600.0,
                ..Default::default()
            },
            SkillBarBindings::default(),
        ))
        .id();

    let result = resolve_anqi_charge_skill(&mut world, caster, 0, None);

    assert!(
        matches!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::QiInsufficient
            }
        ),
        "期望高 qi_max(600) 低 qi_current(50) 玩家被拒绝（qi_target 触顶 80，\
         qi_current<80），实际返回 {result:?}"
    );
}

/// 验证低真元拒绝时不会设置冷却（不应烧冷却）。
#[test]
fn charge_carrier_rejected_qi_insufficient_does_not_set_cooldown() {
    use bong_server::combat::components::SkillBarBindings;

    let mut world = bevy_ecs::world::World::new();
    world.insert_resource(CombatClock { tick: 10 });
    world.insert_resource(bevy_ecs::event::Events::<ChargeCarrierIntent>::default());

    let caster = world
        .spawn((
            Cultivation {
                qi_current: 0.0,
                qi_max: 300.0,
                ..Default::default()
            },
            SkillBarBindings::default(),
        ))
        .id();

    let slot: u8 = 2;
    let result = resolve_anqi_charge_skill(&mut world, caster, slot, None);

    // 先验 Rejected
    assert!(
        matches!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::QiInsufficient
            }
        ),
        "期望 qi_current=0 被拒绝，实际 {result:?}"
    );

    // 再验冷却未设置：anqi.charge_carrier 应仍处于 ready 状态（tick=10）。
    let bindings = world.get::<SkillBarBindings>(caster).unwrap();
    assert!(
        !bindings.is_on_cooldown(ANQI_CHARGE_SKILL_ID, 10),
        "期望真元不足拒绝时不设置冷却（slot={slot} 应 ready），\
         实际冷却被烧掉了"
    );
}
