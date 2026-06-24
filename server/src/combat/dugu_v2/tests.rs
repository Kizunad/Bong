use valence::prelude::{App, Events, Position};

use crate::combat::components::{SkillBarBindings, Wounds};
use crate::combat::dugu_v2::events::{DuguSkillId, TaintTier};
use crate::combat::dugu_v2::physics::{
    dirty_qi_collision, eclipse_effect, penetrate_spec, reveal_probability, self_cure_gain_percent,
    shroud_spec,
};
use crate::combat::dugu_v2::skills::{
    declare_meridian_dependencies, resolve_dugu_v2_skill, DUGU_ECLIPSE_SKILL_ID,
    DUGU_PENETRATE_SKILL_ID, DUGU_REVERSE_SKILL_ID, DUGU_SELF_CURE_SKILL_ID, DUGU_SHROUD_SKILL_ID,
};
use crate::combat::dugu_v2::state::{DuguState, ShroudActive, TaintMark};
use crate::combat::dugu_v2::{
    EclipseNeedleEvent, PenetrateChainEvent, PermanentQiMaxDecayApplied, ReverseTriggeredEvent,
    SelfCureProgressEvent, ShroudActivatedEvent,
};
use crate::combat::events::DeathEvent;
use crate::combat::CombatClock;
use crate::cultivation::components::{ColorKind, Cultivation, MeridianId, QiColor, Realm};
use crate::cultivation::dugu::DuguRevealedEvent;
use crate::cultivation::meridian::severed::{
    MeridianSeveredPermanent, SeveredSource, SkillMeridianDependencies,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::cultivation::tribulation::{JueBiTriggerEvent, JueBiTriggerSource};

fn setup_app() -> App {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 1 });
    app.add_event::<EclipseNeedleEvent>();
    app.add_event::<SelfCureProgressEvent>();
    app.add_event::<PenetrateChainEvent>();
    app.add_event::<ShroudActivatedEvent>();
    app.add_event::<ReverseTriggeredEvent>();
    app.add_event::<PermanentQiMaxDecayApplied>();
    app.add_event::<DuguRevealedEvent>();
    app.add_event::<JueBiTriggerEvent>();
    app.add_event::<DeathEvent>();
    app
}

fn actor(
    app: &mut App,
    realm: Realm,
    qi_current: f64,
    qi_max: f64,
    x: f64,
) -> valence::prelude::Entity {
    app.world_mut()
        .spawn((
            Cultivation {
                realm,
                qi_current,
                qi_max,
                ..Default::default()
            },
            QiColor::default(),
            SkillBarBindings::default(),
            Wounds::default(),
            Position::new([x, 64.0, 0.0]),
        ))
        .id()
}

#[test]
fn registers_five_dugu_v2_skills() {
    let mut registry = SkillRegistry::default();
    crate::combat::dugu_v2::register_skills(&mut registry);
    assert_eq!(DuguSkillId::ALL.len(), 5);
    for id in [
        DUGU_ECLIPSE_SKILL_ID,
        DUGU_SELF_CURE_SKILL_ID,
        DUGU_PENETRATE_SKILL_ID,
        DUGU_SHROUD_SKILL_ID,
        DUGU_REVERSE_SKILL_ID,
    ] {
        assert!(registry.lookup(id).is_some(), "{id} should be registered");
    }
    assert_eq!(DuguSkillId::Eclipse.as_str(), "dugu.eclipse");
}

#[test]
fn declared_liver_dependency_blocks_all_dugu_v2_skills_when_severed() {
    let mut app = setup_app();
    let mut dependencies = SkillMeridianDependencies::default();
    declare_meridian_dependencies(&mut dependencies);
    app.insert_resource(dependencies);

    let caster = actor(&mut app, Realm::Spirit, 500.0, 500.0, 0.0);
    let target = actor(&mut app, Realm::Spirit, 200.0, 200.0, 1.0);
    let mut severed = MeridianSeveredPermanent::default();
    severed.insert(MeridianId::Liver, SeveredSource::DuguDistortion, 1);
    app.world_mut().entity_mut(caster).insert(severed);

    for (skill, target) in [
        (DuguSkillId::Eclipse, Some(target)),
        (DuguSkillId::SelfCure, None),
        (DuguSkillId::Penetrate, Some(target)),
        (DuguSkillId::Shroud, None),
        (DuguSkillId::Reverse, Some(target)),
    ] {
        assert_eq!(
            resolve_dugu_v2_skill(app.world_mut(), caster, 0, target, skill),
            CastResult::Rejected {
                reason: CastRejectReason::MeridianSevered(Some(MeridianId::Liver))
            },
            "{skill:?} should respect DuguDistortion liver severing"
        );
    }
}

#[test]
fn eclipse_thresholds_follow_three_tiers() {
    assert_eq!(
        eclipse_effect(Realm::Awaken, 0.0).tier,
        TaintTier::Immediate
    );
    assert_eq!(
        eclipse_effect(Realm::Condense, 0.0).tier,
        TaintTier::Immediate
    );
    assert_eq!(
        eclipse_effect(Realm::Solidify, 0.0).tier,
        TaintTier::Temporary
    );
    assert_eq!(
        eclipse_effect(Realm::Spirit, 0.0).tier,
        TaintTier::Permanent
    );
    assert_eq!(
        eclipse_effect(Realm::Void, 0.0).permanent_decay_rate_per_min,
        0.001
    );
}

#[test]
fn self_cure_curve_caps_daily_hours_and_locks_color() {
    let gain = self_cure_gain_percent(0.0, 10.0, 4.0);
    assert!(
        (gain - 3.0).abs() < 1e-6,
        "only two hours remain under daily cap"
    );

    let mut app = setup_app();
    let caster = actor(&mut app, Realm::Awaken, 100.0, 100.0, 0.0);
    let result = resolve_dugu_v2_skill(app.world_mut(), caster, 0, None, DuguSkillId::SelfCure);
    assert!(matches!(result, CastResult::Started { .. }));
    let state = app.world().get::<DuguState>(caster).unwrap();
    assert!(state.insidious_color_percent > 0.0);
    assert!(
        eclipse_effect(Realm::Awaken, state.insidious_color_percent).hp_loss
            > eclipse_effect(Realm::Awaken, 0.0).hp_loss
    );
    let color = app.world().get::<QiColor>(caster).unwrap();
    assert_eq!(color.main, ColorKind::Insidious);
    assert!(color.is_permanently_locked(ColorKind::Insidious));
}

#[test]
fn dirty_qi_collision_uses_low_rejection_and_returns_zone_budget() {
    // 修法 ②：Eclipse.returned_zone_qi = rejected_qi × 0.99（仅排斥立即散逸部分）
    // 零距离零抵抗时：effective_hit 接近 injected，rejected_qi 极小，returned_zone_qi 也极小
    let outcome = dirty_qi_collision(100.0, 0.0, 0.0);
    assert!(outcome.effective_hit > 98.0);
    assert!(outcome.rejected_qi < 2.0);
    // returned_zone_qi = rejected_qi × 0.99（不再是 injected×ratio，避免双重入账）
    let expected_returned = outcome.rejected_qi * 0.99;
    assert!(
        (outcome.returned_zone_qi - expected_returned).abs() < 1e-5,
        "returned_zone_qi 应等于 rejected_qi×0.99={expected_returned:.6}，\
         实际={:.6}（Eclipse 只入账排斥部分，Reverse 入账剩余 effective_hit 部分）",
        outcome.returned_zone_qi
    );
}

#[test]
fn shroud_specs_match_realm_strengths() {
    assert_eq!(shroud_spec(Realm::Awaken).strength, 0.20);
    assert_eq!(shroud_spec(Realm::Spirit).strength, 0.85);
    assert!(shroud_spec(Realm::Void).permanent_until_cancelled);
}

#[test]
fn reveal_probability_respects_shroud_distance_and_victim_realm() {
    let near = reveal_probability(Realm::Awaken, 0.0, 3.0, Realm::Solidify);
    let far = reveal_probability(Realm::Awaken, 0.0, 20.0, Realm::Awaken);
    let hidden = reveal_probability(Realm::Awaken, 0.9, 3.0, Realm::Solidify);
    assert!(near > far);
    assert!(hidden < near);
}

#[test]
fn eclipse_applies_taint_mark_to_spirit_target() {
    let mut app = setup_app();
    let caster = actor(&mut app, Realm::Spirit, 100.0, 100.0, 0.0);
    let target = actor(&mut app, Realm::Spirit, 200.0, 200.0, 1.0);
    let result = resolve_dugu_v2_skill(
        app.world_mut(),
        caster,
        0,
        Some(target),
        DuguSkillId::Eclipse,
    );
    assert!(matches!(result, CastResult::Started { .. }));
    let mark = app.world().get::<TaintMark>(target).unwrap();
    assert!(mark.is_permanent());
    assert!(mark.permanent_decay_rate_per_min > 0.0);
    let events = app.world().resource::<Events<EclipseNeedleEvent>>();
    assert_eq!(events.len(), 1);
    assert!(app.world().resource::<Events<DeathEvent>>().is_empty());
}

#[test]
fn eclipse_lethal_damage_emits_death_event() {
    let mut app = setup_app();
    let caster = actor(&mut app, Realm::Spirit, 100.0, 100.0, 0.0);
    let target = actor(&mut app, Realm::Induce, 200.0, 200.0, 1.0);
    app.world_mut()
        .get_mut::<Wounds>(target)
        .unwrap()
        .health_current = 4.0;

    let result = resolve_dugu_v2_skill(
        app.world_mut(),
        caster,
        0,
        Some(target),
        DuguSkillId::Eclipse,
    );

    assert!(matches!(result, CastResult::Started { .. }));
    assert_eq!(
        app.world().get::<Wounds>(target).unwrap().health_current,
        0.0
    );
    let death_events = app.world().resource::<Events<DeathEvent>>();
    let event = death_events
        .iter_current_update_events()
        .find(|event| event.target == target)
        .expect("lethal dugu eclipse should emit DeathEvent");
    assert_eq!(event.attacker, Some(caster));
    assert_eq!(event.attacker_player_id, None);
    assert_eq!(
        event.cause,
        format!("dugu.eclipse:entity:{}", caster.to_bits())
    );
}

#[test]
fn penetrate_requires_existing_taint_mark_and_increases_decay() {
    let mut app = setup_app();
    let caster = actor(&mut app, Realm::Spirit, 100.0, 100.0, 0.0);
    let target = actor(&mut app, Realm::Spirit, 200.0, 200.0, 1.0);
    let miss = resolve_dugu_v2_skill(
        app.world_mut(),
        caster,
        0,
        Some(target),
        DuguSkillId::Penetrate,
    );
    assert!(matches!(
        miss,
        CastResult::Rejected {
            reason: CastRejectReason::InvalidTarget
        }
    ));
    app.world_mut().entity_mut(target).insert(TaintMark {
        caster,
        intensity: 10.0,
        since_tick: 1,
        expires_at_tick: None,
        tier: TaintTier::Permanent,
        temporary_qi_max_loss: 0.0,
        permanent_decay_rate_per_min: 0.001,
        returned_zone_qi: 9.9,
    });
    let result = resolve_dugu_v2_skill(
        app.world_mut(),
        caster,
        1,
        Some(target),
        DuguSkillId::Penetrate,
    );
    assert!(matches!(result, CastResult::Started { .. }));
    assert!(
        app.world()
            .get::<TaintMark>(target)
            .unwrap()
            .permanent_decay_rate_per_min
            > 0.001
    );
}

#[test]
fn reverse_is_void_only_and_clears_permanent_marks() {
    let mut app = setup_app();
    let low = actor(&mut app, Realm::Spirit, 500.0, 500.0, 0.0);
    let victim = actor(&mut app, Realm::Spirit, 200.0, 200.0, 1.0);
    app.world_mut().entity_mut(victim).insert(TaintMark {
        caster: low,
        intensity: 5.0,
        since_tick: 1,
        expires_at_tick: None,
        tier: TaintTier::Permanent,
        temporary_qi_max_loss: 0.0,
        permanent_decay_rate_per_min: 0.001,
        returned_zone_qi: 4.95,
    });
    let rejected =
        resolve_dugu_v2_skill(app.world_mut(), low, 0, Some(victim), DuguSkillId::Reverse);
    assert!(matches!(
        rejected,
        CastResult::Rejected {
            reason: CastRejectReason::RealmTooLow
        }
    ));

    let void_caster = actor(&mut app, Realm::Void, 500.0, 500.0, 2.0);
    app.world_mut().entity_mut(victim).insert(TaintMark {
        caster: void_caster,
        intensity: 5.0,
        since_tick: 1,
        expires_at_tick: None,
        tier: TaintTier::Permanent,
        temporary_qi_max_loss: 0.0,
        permanent_decay_rate_per_min: 0.001,
        returned_zone_qi: 4.95,
    });
    let result = resolve_dugu_v2_skill(
        app.world_mut(),
        void_caster,
        0,
        Some(victim),
        DuguSkillId::Reverse,
    );
    assert!(matches!(result, CastResult::Started { .. }));
    assert!(app.world().get::<TaintMark>(victim).is_none());
    let juebi = app.world().resource::<Events<JueBiTriggerEvent>>();
    assert_eq!(juebi.len(), 1);
    assert_eq!(
        juebi.iter_current_update_events().next().unwrap().source,
        JueBiTriggerSource::DuguReverse
    );
}

#[test]
fn permanent_decay_tick_lowers_qi_max() {
    let mut app = setup_app();
    crate::combat::dugu_v2::register(&mut app);
    let caster = actor(&mut app, Realm::Void, 500.0, 500.0, 0.0);
    let target = actor(&mut app, Realm::Spirit, 100.0, 100.0, 1.0);
    app.world_mut().entity_mut(target).insert(TaintMark {
        caster,
        intensity: 10.0,
        since_tick: 1,
        expires_at_tick: None,
        tier: TaintTier::Permanent,
        temporary_qi_max_loss: 0.0,
        permanent_decay_rate_per_min: 0.1,
        returned_zone_qi: 9.9,
    });
    app.update();
    assert!(app.world().get::<Cultivation>(target).unwrap().qi_max < 100.0);
}

#[test]
fn shroud_maintain_tick_drains_qi_and_expires() {
    let mut app = setup_app();
    crate::combat::dugu_v2::register(&mut app);
    let caster = actor(&mut app, Realm::Awaken, 10.0, 10.0, 0.0);
    let result = resolve_dugu_v2_skill(app.world_mut(), caster, 0, None, DuguSkillId::Shroud);
    assert!(matches!(result, CastResult::Started { .. }));
    assert!(app.world().get::<ShroudActive>(caster).is_some());
    app.update();
    assert!(app.world().get::<Cultivation>(caster).unwrap().qi_current < 5.0);
}

#[test]
fn penetrate_spec_void_reaches_zone_scale() {
    assert!(penetrate_spec(Realm::Void).radius_blocks.is_infinite());
    assert_eq!(penetrate_spec(Realm::Awaken).multiplier, 1.5);
}

// --- Visual ID pin tests & emit helpers (CodeRabbit review item: dugu_v2) ---

#[test]
fn dugu_visual_ids_pin_eclipse() {
    use super::skills::visual_for;
    let v = visual_for(DuguSkillId::Eclipse);
    assert_eq!(v.animation_id, "bong:dugu_needle_throw");
    assert_eq!(v.particle_id, "bong:dugu_taint_pulse");
    assert_eq!(v.sound_recipe_id, "dugu_needle_hiss");
    assert_eq!(v.hud_hint, "蚀针");
    assert_eq!(v.icon_texture, "bong:textures/gui/skill/dugu_eclipse.png");
}

#[test]
fn dugu_visual_ids_pin_self_cure() {
    use super::skills::visual_for;
    let v = visual_for(DuguSkillId::SelfCure);
    assert_eq!(v.animation_id, "bong:dugu_self_cure_pose");
    assert_eq!(v.particle_id, "bong:dugu_dark_green_mist");
    assert_eq!(v.sound_recipe_id, "dugu_self_cure_drink");
    assert_eq!(v.hud_hint, "自蕴");
    assert_eq!(v.icon_texture, "bong:textures/gui/skill/dugu_self_cure.png");
}

#[test]
fn dugu_visual_ids_pin_penetrate() {
    use super::skills::visual_for;
    let v = visual_for(DuguSkillId::Penetrate);
    assert_eq!(v.animation_id, "bong:dugu_needle_throw");
    assert_eq!(v.particle_id, "bong:dugu_taint_pulse");
    assert_eq!(v.sound_recipe_id, "dugu_needle_hiss");
    assert_eq!(v.hud_hint, "侵染");
    assert_eq!(v.icon_texture, "bong:textures/gui/skill/dugu_penetrate.png");
}

#[test]
fn dugu_visual_ids_pin_shroud() {
    use super::skills::visual_for;
    let v = visual_for(DuguSkillId::Shroud);
    assert_eq!(v.animation_id, "bong:dugu_shroud_activate");
    assert_eq!(v.particle_id, "bong:dugu_dark_green_mist");
    assert_eq!(v.sound_recipe_id, "dugu_self_cure_drink");
    assert_eq!(v.hud_hint, "神识遮蔽");
    assert_eq!(v.icon_texture, "bong:textures/gui/skill/dugu_shroud.png");
}

#[test]
fn dugu_visual_ids_pin_reverse() {
    use super::skills::visual_for;
    let v = visual_for(DuguSkillId::Reverse);
    assert_eq!(v.animation_id, "bong:dugu_pointing_curse");
    assert_eq!(v.particle_id, "bong:dugu_reverse_burst");
    assert_eq!(v.sound_recipe_id, "dugu_curse_cackle");
    assert_eq!(v.hud_hint, "倒蚀");
    assert_eq!(v.icon_texture, "bong:textures/gui/skill/dugu_reverse.png");
}

#[test]
fn dugu_visual_ids_exhaustive_all_five_skills_have_unique_hud_hint() {
    use super::skills::visual_for;
    let mut hints = std::collections::HashSet::new();
    for skill in DuguSkillId::ALL {
        let v = visual_for(skill);
        assert!(
            hints.insert(v.hud_hint),
            "duplicate hud_hint '{}' for {skill:?} -- each skill must have a unique HUD hint",
            v.hud_hint
        );
    }
}

#[test]
fn dugu_emit_helpers_noop_without_event_resources() {
    use super::skills::{emit_anim, emit_audio, emit_vfx};
    use valence::prelude::{App, DVec3};

    let mut app = App::new();
    // Intentionally do NOT register VfxEventRequest or PlaySoundRecipeRequest events.
    let entity = app.world_mut().spawn_empty().id();
    // These should not panic when the event resources are absent.
    emit_vfx(
        app.world_mut(),
        DVec3::ZERO,
        "bong:test",
        "#FF0000",
        0.5,
        4,
        20,
    );
    emit_audio(app.world_mut(), "test_recipe", DVec3::ZERO);
    emit_anim(app.world_mut(), entity, "bong:test_anim");
}

#[test]
fn dugu_emit_anim_skips_without_unique_id() {
    use super::skills::emit_anim;
    use crate::network::vfx_event_emit::VfxEventRequest;
    use valence::prelude::{App, Events};

    let mut app = App::new();
    app.add_event::<VfxEventRequest>();
    // Spawn entity with Position but without UniqueId
    let entity = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();

    emit_anim(app.world_mut(), entity, "bong:dugu_needle_throw");

    // Should emit zero VfxEventRequest because UniqueId is missing
    assert_eq!(
        app.world().resource::<Events<VfxEventRequest>>().len(),
        0,
        "emit_anim should skip PlayAnim when entity has no UniqueId"
    );
}

// ── minor②: dugu is_chaotic guard 专项 case ─────────────────────────────

/// 杂色玩家施放 Eclipse 时，insidious_color_percent 加成必须强制清零（worldview §六.2）。
/// Eclipse 的 self_cure_percent 参数在杂色时应传 0.0，不管 DuguState 里记了多少。
#[test]
fn eclipse_chaotic_caster_zeroes_insidious_color_bonus() {
    let mut app = setup_app();
    let caster = actor(&mut app, Realm::Spirit, 100.0, 100.0, 0.0);
    let target = actor(&mut app, Realm::Spirit, 200.0, 200.0, 1.0);

    // 给 caster 挂上杂色 + 高 insidious_color_percent
    app.world_mut().entity_mut(caster).insert(QiColor {
        main: crate::cultivation::components::ColorKind::Insidious,
        is_chaotic: true,
        ..Default::default()
    });
    let state_with_bonus = super::state::DuguState {
        insidious_color_percent: 50.0,
        morphology_percent: 50.0,
        ..Default::default()
    };
    app.world_mut().entity_mut(caster).insert(state_with_bonus);

    // 先记录无加成时的 hp_loss（杂色玩家应等同于 insidious_color_percent=0 的效果）
    let effect_no_bonus = super::physics::eclipse_effect(Realm::Spirit, 0.0);
    let effect_with_bonus = super::physics::eclipse_effect(Realm::Spirit, 50.0);
    // 确认两条效果确实有差异（验证 color_percent 确实影响 eclipse_effect 输出）
    assert_ne!(
        effect_no_bonus.hp_loss, effect_with_bonus.hp_loss,
        "前提：insidious_color_percent 0 vs 50 应产生不同 hp_loss，否则 guard 测试无意义"
    );

    let result = resolve_dugu_v2_skill(
        app.world_mut(),
        caster,
        0,
        Some(target),
        DuguSkillId::Eclipse,
    );
    assert!(
        matches!(result, CastResult::Started { .. }),
        "杂色 caster Eclipse 应正常完成 cast（guard 只清零加成，不拒绝 cast），实际 result={result:?}"
    );

    // 蚀针事件里 qi_loss 应反映「无加成」路径
    let events_res = app.world().resource::<Events<EclipseNeedleEvent>>();
    let event = events_res
        .iter_current_update_events()
        .next()
        .expect("杂色 Eclipse 应发 EclipseNeedleEvent");
    assert!(
        (event.hp_loss - effect_no_bonus.hp_loss).abs() < 1e-3,
        "杂色 caster Eclipse hp_loss 应等于 insidious_color_percent=0 时的效果（{}），\
         实际 {}（杂色时必须强制清零 insidious_color_percent 加成，守 worldview §六.2）",
        effect_no_bonus.hp_loss,
        event.hp_loss
    );
}

/// 非杂色 caster 的 insidious_color_percent 加成应正常传入 eclipse_effect。
#[test]
fn eclipse_non_chaotic_caster_uses_insidious_color_bonus() {
    let mut app = setup_app();
    let caster = actor(&mut app, Realm::Spirit, 100.0, 100.0, 0.0);
    let target = actor(&mut app, Realm::Spirit, 200.0, 200.0, 1.0);

    // 非杂色但有 insidious_color_percent
    app.world_mut().entity_mut(caster).insert(QiColor {
        main: crate::cultivation::components::ColorKind::Insidious,
        is_chaotic: false,
        ..Default::default()
    });
    app.world_mut()
        .entity_mut(caster)
        .insert(super::state::DuguState {
            insidious_color_percent: 30.0,
            morphology_percent: 30.0,
            ..Default::default()
        });

    let effect_with_bonus = super::physics::eclipse_effect(Realm::Spirit, 30.0);
    let result = resolve_dugu_v2_skill(
        app.world_mut(),
        caster,
        0,
        Some(target),
        DuguSkillId::Eclipse,
    );
    assert!(matches!(result, CastResult::Started { .. }));

    let event = app
        .world()
        .resource::<Events<EclipseNeedleEvent>>()
        .iter_current_update_events()
        .next()
        .expect("非杂色 Eclipse 应发 EclipseNeedleEvent");
    assert!(
        (event.hp_loss - effect_with_bonus.hp_loss).abs() < 1e-3,
        "非杂色 caster 的 insidious_color_percent=30 加成应生效：期望 hp_loss {}，\
         实际 {} (保证非杂色路径不被误清零)",
        effect_with_bonus.hp_loss,
        event.hp_loss
    );
}

/// 杂色 caster 施放其他 dugu 技能（SelfCure / Penetrate / Shroud / Reverse）也不应
/// 因杂色被拒绝（guard 只清加成，不影响 cast 资格本身）。
#[test]
fn dugu_chaotic_caster_cast_not_rejected_for_non_eclipse_skills() {
    let mut app = setup_app();
    // Reverse 需要目标有 TaintMark，这里只测 SelfCure/Shroud
    let caster = actor(&mut app, Realm::Spirit, 100.0, 100.0, 0.0);
    app.world_mut().entity_mut(caster).insert(QiColor {
        is_chaotic: true,
        ..Default::default()
    });

    // SelfCure 不需目标
    let result_self_cure =
        resolve_dugu_v2_skill(app.world_mut(), caster, 0, None, DuguSkillId::SelfCure);
    assert!(
        matches!(result_self_cure, CastResult::Started { .. }),
        "杂色时 SelfCure 不应被拒绝（杂色 guard 只清加成）：实际 result={result_self_cure:?}"
    );

    // Shroud 不需目标
    let result_shroud =
        resolve_dugu_v2_skill(app.world_mut(), caster, 1, None, DuguSkillId::Shroud);
    assert!(
        matches!(result_shroud, CastResult::Started { .. }),
        "杂色时 Shroud 不应被拒绝：实际 result={result_shroud:?}"
    );
}

// ── plan-qi-conservation-leaks-v1 P4 — dugu v2 returned_zone_qi 守恒测试 ────────

/// 辅助：构建带 ZoneRegistry + WorldQiAccount 的最小 App（含所有 dugu v2 系统）。
fn setup_zone_credit_app(zone_spirit_qi_before: f64) -> App {
    use crate::qi_physics::ledger::WorldQiAccount;
    use crate::world::dimension::DimensionKind;
    use crate::world::zone::{Zone, ZoneRegistry};
    use valence::prelude::DVec3;

    let mut app = setup_app();
    crate::combat::dugu_v2::register(&mut app);

    // 注册一个覆盖玩家坐标 [0,64,0] 的 overworld spawn zone
    let zone = Zone {
        name: "spawn".to_string(),
        dimension: DimensionKind::Overworld,
        bounds: (
            DVec3::new(-100.0, 0.0, -100.0),
            DVec3::new(100.0, 200.0, 100.0),
        ),
        spirit_qi: zone_spirit_qi_before,
        danger_level: 0,
        active_events: Vec::new(),
        patrol_anchors: Vec::new(),
        blocked_tiles: Vec::new(),
    };
    app.insert_resource(ZoneRegistry { zones: vec![zone] });
    app.insert_resource(WorldQiAccount::default());
    app
}

/// happy path: Eclipse 施放后，zone.spirit_qi 精确增加 returned_zone_qi。
#[test]
fn eclipse_zone_credit_happy_path_zone_increases_by_returned_zone_qi() {
    use crate::qi_physics::ledger::WorldQiAccount;
    use crate::world::zone::ZoneRegistry;

    let zone_qi_before = 0.1_f64;
    let mut app = setup_zone_credit_app(zone_qi_before);
    let caster = actor(&mut app, Realm::Spirit, 100.0, 100.0, 0.0);
    let target = actor(&mut app, Realm::Spirit, 200.0, 200.0, 1.0);

    // 记录 Eclipse 的 returned_zone_qi（与 dirty_qi_collision 一致，约 qi_loss×0.99）
    let result = resolve_dugu_v2_skill(
        app.world_mut(),
        caster,
        0,
        Some(target),
        DuguSkillId::Eclipse,
    );
    assert!(
        matches!(result, CastResult::Started { .. }),
        "Eclipse cast 应成功，实际={result:?}"
    );
    // 从 event 中拿出 returned_zone_qi（避免手算 physics）
    let returned = {
        let events = app.world().resource::<Events<EclipseNeedleEvent>>();
        events
            .iter_current_update_events()
            .next()
            .expect("Eclipse 应发 EclipseNeedleEvent")
            .returned_zone_qi
    };
    assert!(
        returned > 0.0,
        "Spirit 境目标 Eclipse 应产生 returned_zone_qi > 0，实际={returned}"
    );

    // app.update() 触发 eclipse_zone_credit_tick 消费事件
    app.update();

    let zone_qi_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .expect("spawn zone should exist")
        .spirit_qi;

    // MF3 fix: zone_qi 增量 = returned/QI_ZONE_UNIT_CAPACITY（绝对量→归一化），而非裸加 returned
    use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    let returned_abs = f64::from(returned);
    let zone_current_abs = zone_qi_before.max(0.0) * QI_ZONE_UNIT_CAPACITY;
    let room = (QI_ZONE_UNIT_CAPACITY - zone_current_abs).max(0.0);
    let accepted = returned_abs.min(room);
    let expected = (zone_qi_before + accepted / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
    assert!(
        (zone_qi_after - expected).abs() < 1e-9,
        "Eclipse 后 zone.spirit_qi 应为 {expected:.9}（增量=accepted({accepted:.6})/CAP），         实际={zone_qi_after:.9}（MF3 fix: absolute→normalized 转换后入账 returned={returned_abs:.6}）"
    );

    // audit trail：应有 ≥1 条 DuguReturnToZone 记录（accepted 1 条，overflow 可选 1 条）
    use crate::qi_physics::ledger::QiTransferReason;
    let account = app.world().resource::<WorldQiAccount>();
    let dugu_transfers: Vec<_> = account
        .transfers()
        .iter()
        .filter(|t| t.reason == QiTransferReason::DuguReturnToZone)
        .collect();
    assert!(
        !dugu_transfers.is_empty(),
        "Eclipse 后应至少有 1 条 DuguReturnToZone 审计记录，实际=0"
    );
    // 所有 DuguReturnToZone 审计记录的 amount 之和 == returned_abs（守恒：accepted + overflow）
    let total_audit: f64 = dugu_transfers.iter().map(|t| t.amount).sum();
    assert!(
        (total_audit - returned_abs).abs() < 1e-9,
        "DuguReturnToZone 审计记录 amount 之和({total_audit:.9}) 应等于 returned({returned_abs:.9})，         确保 qi 不蒸发（accepted + overflow = returned）"
    );
}

/// happy path: Reverse（倒蚀）施放后，zone.spirit_qi 精确增加 returned_zone_qi。
#[test]
fn reverse_zone_credit_happy_path_zone_increases_by_returned_zone_qi() {
    use crate::qi_physics::ledger::WorldQiAccount;
    use crate::world::zone::ZoneRegistry;

    let zone_qi_before = 0.2_f64;
    let mut app = setup_zone_credit_app(zone_qi_before);
    let void_caster = actor(&mut app, Realm::Void, 500.0, 500.0, 0.0);
    let victim = actor(&mut app, Realm::Spirit, 200.0, 200.0, 1.0);
    app.world_mut().entity_mut(victim).insert(TaintMark {
        caster: void_caster,
        intensity: 5.0,
        since_tick: 1,
        expires_at_tick: None,
        tier: TaintTier::Permanent,
        temporary_qi_max_loss: 0.0,
        permanent_decay_rate_per_min: 0.001,
        returned_zone_qi: 4.95,
    });

    let result = resolve_dugu_v2_skill(
        app.world_mut(),
        void_caster,
        0,
        Some(victim),
        DuguSkillId::Reverse,
    );
    assert!(
        matches!(result, CastResult::Started { .. }),
        "Reverse cast 应成功，实际={result:?}"
    );

    let returned = {
        let events = app.world().resource::<Events<ReverseTriggeredEvent>>();
        events
            .iter_current_update_events()
            .next()
            .expect("Reverse 应发 ReverseTriggeredEvent")
            .returned_zone_qi
    };
    assert!(
        returned > 0.0,
        "Reverse 应产生 returned_zone_qi > 0，实际={returned}"
    );

    app.update();

    let zone_qi_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .expect("spawn zone should exist")
        .spirit_qi;

    // MF3 fix: zone_qi 增量 = returned/QI_ZONE_UNIT_CAPACITY（绝对量→归一化），而非裸加 returned
    use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    let returned_abs = f64::from(returned);
    let zone_current_abs = zone_qi_before.max(0.0) * QI_ZONE_UNIT_CAPACITY;
    let room = (QI_ZONE_UNIT_CAPACITY - zone_current_abs).max(0.0);
    let accepted = returned_abs.min(room);
    let expected = (zone_qi_before + accepted / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
    assert!(
        (zone_qi_after - expected).abs() < 1e-9,
        "Reverse 后 zone.spirit_qi 应为 {expected:.9}（增量=accepted({accepted:.6})/CAP），         实际={zone_qi_after:.9}（MF3 fix: absolute→normalized 转换后入账 returned={returned_abs:.6}）"
    );

    use crate::qi_physics::ledger::QiTransferReason;
    let account = app.world().resource::<WorldQiAccount>();
    let dugu_transfers: Vec<_> = account
        .transfers()
        .iter()
        .filter(|t| t.reason == QiTransferReason::DuguReturnToZone)
        .collect();
    assert!(
        !dugu_transfers.is_empty(),
        "Reverse 后应至少有 1 条 DuguReturnToZone 审计记录，实际=0"
    );
    // 守恒：所有 DuguReturnToZone 审计记录 amount 之和 == returned_abs
    let total_audit: f64 = dugu_transfers.iter().map(|t| t.amount).sum();
    assert!(
        (total_audit - returned_abs).abs() < 1e-9,
        "DuguReturnToZone 审计记录 amount 之和({total_audit:.9}) 应等于 returned({returned_abs:.9})"
    );
}

/// 守恒不变式：Eclipse 前后，zone_qi 增量 == returned_zone_qi（容差内）。
///
/// 修法 ② 后 Eclipse.returned_zone_qi = rejected_qi × 0.99，仅覆盖被排斥立即散逸部分。
/// 使用低初值（-0.8）确保 zone 有足够容量容纳 rejected_qi×0.99（约 1.8 单位），
/// 消除 min(returned, room) 截断掩盖泄漏的问题。
///
/// 若改动导致 returned_zone_qi 再次膨胀（如误改回 injected_qi×ratio ~39.6），
/// zone_qi_delta < returned 差值 ~37.8 会立即暴露——不再被 min() 掩盖。
#[test]
fn eclipse_conservation_total_observed_invariant() {
    use crate::qi_physics::ledger::{summarize_world_qi, WorldQiBudget};

    // 使用低初值确保 zone 有充足容量（rejected_qi×0.99 约 1.8，需要 room > 1.8）
    // -0.8 → room = 1.8，刚好容纳；若 returned 误膨胀到 ~39.6 则 room 不足，断言暴露截断
    let zone_qi_before = -0.8_f64;
    let mut app = setup_zone_credit_app(zone_qi_before);
    app.insert_resource(WorldQiBudget::from_total(100.0));

    let caster = actor(&mut app, Realm::Spirit, 100.0, 100.0, 0.0);
    let target = actor(&mut app, Realm::Spirit, 200.0, 200.0, 1.0);

    let snap_before = summarize_world_qi(app.world_mut());

    let _ = resolve_dugu_v2_skill(
        app.world_mut(),
        caster,
        0,
        Some(target),
        DuguSkillId::Eclipse,
    );
    let returned = {
        let events = app.world().resource::<Events<EclipseNeedleEvent>>();
        events
            .iter_current_update_events()
            .next()
            .expect("Eclipse 应发 EclipseNeedleEvent")
            .returned_zone_qi
    };

    // 验证 returned 在合理范围（修法 ② 后 < 2.0；若误改回 injected×ratio 则 ~39.6）
    assert!(
        f64::from(returned) < 2.0,
        "Eclipse.returned_zone_qi({returned:.4}) 异常偏大（应 < 2.0）；\
         若此断言失败说明 returned_zone_qi 被误改回 injected_qi×ratio（双重入账通胀 bug）"
    );

    app.update();

    let snap_after = summarize_world_qi(app.world_mut());

    // MF3 fix: qi_release_to_zone 处理 absolute→normalized 换算。
    // zone_qi_before = -0.8 → zone_current_abs = max(0,-0.8)*50 = 0.0（耗尽视为空）
    // room = 50.0 - 0.0 = 50.0 >> returned(~1.76) → accepted = returned, no overflow
    // zone_after = (zone_current_abs + accepted) / CAP = (0.0+1.76)/50 = 0.0352
    // zone_qi_delta = snap_after.zone_qi - snap_before.zone_qi = 0.0352 - (-0.8) = 0.8352
    use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    let returned_abs = f64::from(returned);
    let zone_current_abs = zone_qi_before.max(0.0) * QI_ZONE_UNIT_CAPACITY; // = 0.0
    let room = (QI_ZONE_UNIT_CAPACITY - zone_current_abs).max(0.0); // = 50.0
    let accepted = returned_abs.min(room); // = returned_abs (no overflow)
                                           // Expected zone_after (normalized) = (zone_current_abs + accepted) / CAP
    let expected_zone_after = (zone_current_abs + accepted) / QI_ZONE_UNIT_CAPACITY;
    assert!(
        (snap_after.zone_qi - expected_zone_after).abs() < 1e-9,
        "守恒失败：zone.spirit_qi after({:.9}) 应精确等于 (zone_current({zone_current_abs})+accepted({accepted}))/CAP={expected_zone_after:.9}。\
         zone_before={zone_qi_before}, returned={returned_abs:.6}, room={room:.1}。\
         若 snap_after.zone_qi > expected_zone_after 说明仍在裸加 returned（MF3 bug 未修复）；\
         若 snap_after.zone_qi << expected_zone_after 说明 CAP 换算出错",
        snap_after.zone_qi
    );

    // ledger_qi 不应因 DuguReturnToZone 改变（audit-only 路径，不动 ledger balance）
    let ledger_qi_delta = snap_after.ledger_qi - snap_before.ledger_qi;
    assert!(
        ledger_qi_delta.abs() < 1e-9,
        "ledger_qi 不应因 DuguReturnToZone 改变（audit-only 路径），\
         实际 delta={ledger_qi_delta}"
    );
}

/// 边界：returned_zone_qi = 0 时不产生审计记录、zone 不变。
#[test]
fn eclipse_zero_returned_zone_qi_no_audit_and_zone_unchanged() {
    use crate::qi_physics::ledger::{QiTransferReason, WorldQiAccount};
    use crate::world::zone::ZoneRegistry;

    let zone_qi_before = 0.5_f64;
    let mut app = setup_zone_credit_app(zone_qi_before);

    // 直接向 app 发送一个 returned_zone_qi=0 的 EclipseNeedleEvent（绕过 physics 直接构造）
    let caster = app.world_mut().spawn_empty().id();
    let target = app.world_mut().spawn_empty().id();
    app.world_mut().send_event(EclipseNeedleEvent {
        caster,
        target,
        target_realm: Realm::Awaken,
        tier: TaintTier::Immediate,
        injected_qi: 0.0,
        hp_loss: 0.0,
        qi_loss: 0.0,
        qi_max_loss: 0.0,
        permanent_decay_rate_per_min: 0.0,
        returned_zone_qi: 0.0, // 边界：零返还
        reveal_probability: 0.0,
        tick: 1,
        visual: crate::combat::dugu_v2::skills::visual_for(DuguSkillId::Eclipse),
    });

    app.update();

    let zone_qi_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .expect("spawn zone should exist")
        .spirit_qi;

    assert!(
        (zone_qi_after - zone_qi_before).abs() < 1e-12,
        "returned_zone_qi=0 时 zone.spirit_qi 不应改变，实际 before={zone_qi_before} after={zone_qi_after}"
    );

    let account = app.world().resource::<WorldQiAccount>();
    let dugu_transfers: Vec<_> = account
        .transfers()
        .iter()
        .filter(|t| t.reason == QiTransferReason::DuguReturnToZone)
        .collect();
    assert!(
        dugu_transfers.is_empty(),
        "returned_zone_qi=0 时不应产生 DuguReturnToZone 审计记录，实际 len={}",
        dugu_transfers.len()
    );
}

/// MF3 锁定：zone 接近饱和时，overflow qi 入账 overflow account，不蒸发。
///
/// 这是直接锁定 MF3 bug 的核心测试：
///   旧代码: zone.spirit_qi = (zone.spirit_qi + returned).clamp(-1.0, 1.0)
///     → 大 returned 被 clamp 截断 → qi 蒸发
///   新代码: qi_release_to_zone → accepted 入 zone，overflow → overflow account
///     → zone_credit_absolute + overflow == returned（守恒）
#[test]
fn eclipse_zone_credit_overflow_no_evaporation_mf3_lock() {
    use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    use crate::qi_physics::ledger::{QiTransferReason, WorldQiAccount};
    use crate::world::zone::ZoneRegistry;

    // zone 接近饱和：spirit_qi=0.98 → zone_current_abs=49.0, room=1.0
    let zone_qi_before = 0.98_f64;
    let mut app = setup_zone_credit_app(zone_qi_before);

    // 直接注入一个 returned=10.0 的 EclipseNeedleEvent（远超 room=1.0，必然有 overflow=9.0）
    let caster = app.world_mut().spawn_empty().id();
    let target_entity = app
        .world_mut()
        .spawn((valence::prelude::Position::new([0.0, 64.0, 0.0]),))
        .id();
    app.world_mut().send_event(EclipseNeedleEvent {
        caster,
        target: target_entity,
        target_realm: Realm::Spirit,
        tier: crate::combat::dugu_v2::events::TaintTier::Permanent,
        injected_qi: 40.0,
        hp_loss: 20.0,
        qi_loss: 10.0,
        qi_max_loss: 0.0,
        permanent_decay_rate_per_min: 0.0005,
        returned_zone_qi: 10.0, // >> room(1.0)，必然溢出
        reveal_probability: 0.0,
        tick: 1,
        visual: crate::combat::dugu_v2::skills::visual_for(DuguSkillId::Eclipse),
    });

    app.update();

    let zone_qi_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .expect("spawn zone should exist")
        .spirit_qi;

    // zone.spirit_qi 不应超过 1.0（overflow 被路由到 overflow account，而非被 clamp 截断）
    assert!(
        zone_qi_after <= 1.0 + 1e-9,
        "zone.spirit_qi 不应超过 1.0，实际={zone_qi_after:.9}（overflow 必须路由出去）"
    );

    // zone 增量 = accepted/QI_ZONE_UNIT_CAPACITY = room/QI_ZONE_UNIT_CAPACITY ≈ 0.02
    let zone_current_abs = zone_qi_before.max(0.0) * QI_ZONE_UNIT_CAPACITY; // 49.0
    let room = (QI_ZONE_UNIT_CAPACITY - zone_current_abs).max(0.0); // 1.0
    let returned_abs = 10.0_f64;
    let accepted = returned_abs.min(room); // 1.0
    let overflow = returned_abs - accepted; // 9.0
    let expected_zone_after = (zone_qi_before + accepted / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
    assert!(
        (zone_qi_after - expected_zone_after).abs() < 1e-9,
        "zone 应精确增加 accepted({accepted})/CAP({QI_ZONE_UNIT_CAPACITY})={:.4}，         期望 zone_after={expected_zone_after:.9}，实际={zone_qi_after:.9}",
        accepted / QI_ZONE_UNIT_CAPACITY
    );

    // 核心守恒断言：DuguReturnToZone 审计记录 amount 之和 == returned（accepted + overflow）
    let account = app.world().resource::<WorldQiAccount>();
    let dugu_transfers: Vec<_> = account
        .transfers()
        .iter()
        .filter(|t| t.reason == QiTransferReason::DuguReturnToZone)
        .collect();
    assert!(
        !dugu_transfers.is_empty(),
        "应有 DuguReturnToZone 审计记录（至少 1 条：accepted），实际=0"
    );
    let total_audit: f64 = dugu_transfers.iter().map(|t| t.amount).sum();
    assert!(
        (total_audit - returned_abs).abs() < 1e-9,
        "MF3 守恒断言：所有 DuguReturnToZone 审计记录 amount 之和({total_audit:.9})          应 == returned_abs({returned_abs:.9})（accepted({accepted}) + overflow({overflow}) = returned）。         若 total < returned 说明 MF3 bug 仍存在（qi 蒸发）"
    );

    // overflow 应有专属记录（amount = 9.0）
    let overflow_records: Vec<_> = dugu_transfers
        .iter()
        .filter(|t| {
            // overflow account ID 含 "overflow"
            matches!(&t.to, crate::qi_physics::ledger::QiAccountId { .. } if {
                let id_str = format!("{:?}", t.to);
                id_str.contains("overflow")
            })
        })
        .collect();
    assert!(
        !overflow_records.is_empty(),
        "overflow({overflow}) 应路由到 overflow account（DuguReturnToZone 记录 to 含 overflow），         实际无 overflow 记录（旧 clamp 截断未修复）"
    );
    let total_overflow: f64 = overflow_records.iter().map(|t| t.amount).sum();
    assert!(
        (total_overflow - overflow).abs() < 1e-9,
        "overflow account 记录 amount 之和({total_overflow:.9}) 应 == overflow({overflow:.9})"
    );
}

/// 不双计：同一 EclipseNeedleEvent 只入账一次（不重复消费）。
#[test]
fn eclipse_zone_credit_no_double_accounting_single_event_single_credit() {
    use crate::qi_physics::ledger::{QiTransferReason, WorldQiAccount};
    use crate::world::zone::ZoneRegistry;

    let zone_qi_before = 0.1_f64;
    let mut app = setup_zone_credit_app(zone_qi_before);
    let caster = actor(&mut app, Realm::Spirit, 100.0, 100.0, 0.0);
    let target = actor(&mut app, Realm::Spirit, 200.0, 200.0, 1.0);

    let _ = resolve_dugu_v2_skill(
        app.world_mut(),
        caster,
        0,
        Some(target),
        DuguSkillId::Eclipse,
    );
    let returned = {
        let events = app.world().resource::<Events<EclipseNeedleEvent>>();
        events
            .iter_current_update_events()
            .next()
            .expect("Eclipse 应发 EclipseNeedleEvent")
            .returned_zone_qi
    };

    // 运行两个 update tick；事件在第一个 tick 被消费，第二个 tick 不应重复入账
    app.update();
    app.update();

    let zone_qi_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;

    // MF3 fix: expected uses normalized formula
    use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    let returned_abs = f64::from(returned);
    let zone_current_abs = zone_qi_before.max(0.0) * QI_ZONE_UNIT_CAPACITY;
    let room = (QI_ZONE_UNIT_CAPACITY - zone_current_abs).max(0.0);
    let accepted = returned_abs.min(room);
    let expected = (zone_qi_before + accepted / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
    assert!(
        (zone_qi_after - expected).abs() < 1e-9,
        "两次 update 后 zone.spirit_qi 应与单次 update 相同（不双计），         期望 {expected:.9}（增量=accepted({accepted:.6})/CAP），实际 {zone_qi_after:.9}"
    );

    // 审计记录恰好 1 条（只入账一次）
    let account = app.world().resource::<WorldQiAccount>();
    let count = account
        .transfers()
        .iter()
        .filter(|t| t.reason == QiTransferReason::DuguReturnToZone)
        .count();
    assert_eq!(
        count, 1,
        "两次 update 后应恰好有 1 条 DuguReturnToZone 记录（不双计），实际={count}"
    );
}

/// DuguReturnToZone 变体 pin 测试：存在且区别于其他变体。
#[test]
fn dugu_return_to_zone_reason_pin_test() {
    use crate::qi_physics::ledger::QiTransferReason;
    let reason = QiTransferReason::DuguReturnToZone;
    assert_ne!(
        reason,
        QiTransferReason::ReleaseToZone,
        "DuguReturnToZone 应为独立变体，区别于 ReleaseToZone（毒蛊散逸 vs 招式释放）"
    );
    assert_ne!(
        reason,
        QiTransferReason::BossDrain,
        "DuguReturnToZone 应区别于 BossDrain"
    );
    assert_ne!(
        reason,
        QiTransferReason::HalfStepBuff,
        "DuguReturnToZone 应区别于 HalfStepBuff（非 audit-only 容量标记）"
    );
    assert_eq!(reason, reason, "DuguReturnToZone 与自身应相等");
}

/// 连招守恒：Eclipse 种下标记 → Reverse 爆发，两路合计 returned_zone_qi == injected_qi × ratio，
/// 不产生双重入账通胀。
///
/// 旧代码 bug（已修复）：
///   - Eclipse.returned_zone_qi = injected_qi × 0.99（入账 ~39.6）
///   - Reverse.returned_zone_qi = mark.intensity × 0.99 ≈ effective_hit × 0.99（再入账 ~37.8）
///   - 两次入账同一团脏真元 → 合计 ~77.4（远超 injected_qi × 0.99 ≈ 39.6）
///
/// 修法 ② 后：
///   - Eclipse.returned_zone_qi = rejected_qi × 0.99（排斥立即散逸，小值 ~1.8）
///   - Reverse.returned_zone_qi = mark.intensity × 0.99 ≈ effective_hit × 0.99（延迟散逸 ~37.8）
///   - 两路之和 ≈ (rejected + effective_hit) × 0.99 = attenuated_qi × 0.99 ≈ injected × 0.99
///
/// 守恒验证在事件层面进行（两路合计 ~39.6 超出 zone 容量 2.0，不能用 zone.spirit_qi delta）。
/// 当前（旧）实现下合计 ~77.4 >> 39.6，此断言会撞红；修复后转绿。
#[test]
fn eclipse_then_reverse_chain_conservation_no_double_entry() {
    use crate::qi_physics::constants::DUGU_DIRTY_QI_ZONE_RETURN_RATIO;
    use crate::qi_physics::ledger::{QiTransferReason, WorldQiAccount};

    let zone_qi_before = 0.05_f64;
    let mut app = setup_zone_credit_app(zone_qi_before);

    // Void 境施法者（Reverse 需要 Void），Spirit 境目标（qi_loss=40，返还值有代表性）
    let caster = actor(&mut app, Realm::Void, 500.0, 500.0, 0.0);
    let target = actor(&mut app, Realm::Spirit, 200.0, 200.0, 1.0);

    // ── step 1: Eclipse ──
    let result = resolve_dugu_v2_skill(
        app.world_mut(),
        caster,
        0,
        Some(target),
        DuguSkillId::Eclipse,
    );
    assert!(
        matches!(result, CastResult::Started { .. }),
        "Eclipse cast 应成功，实际={result:?}"
    );
    let eclipse_event = {
        let events = app.world().resource::<Events<EclipseNeedleEvent>>();
        events
            .iter_current_update_events()
            .next()
            .expect("Eclipse 应发 EclipseNeedleEvent")
            .clone()
    };
    let eclipse_returned = f64::from(eclipse_event.returned_zone_qi);
    let eclipse_injected = f64::from(eclipse_event.injected_qi);

    // 修法 ② 后：Eclipse.returned 仅为 rejected_qi × ratio（小值 < 5.0）
    assert!(
        eclipse_returned < 5.0,
        "Eclipse.returned_zone_qi({eclipse_returned:.4}) 异常偏大（应 < 5.0）；\
         若此断言失败说明 returned_zone_qi 被误改回 injected×ratio（旧 bug 复发）"
    );

    app.update(); // eclipse_zone_credit_tick 消费 EclipseNeedleEvent

    // ── step 2: Reverse ──
    // 使用 slot 1（Eclipse 用的是 slot 0，两个 slot 互相独立，slot 1 无 cooldown）
    let result = resolve_dugu_v2_skill(
        app.world_mut(),
        caster,
        1,
        Some(target),
        DuguSkillId::Reverse,
    );
    assert!(
        matches!(result, CastResult::Started { .. }),
        "Reverse cast 应成功（需要 target 有 TaintMark），实际={result:?}"
    );
    let reverse_event = {
        let events = app.world().resource::<Events<ReverseTriggeredEvent>>();
        events
            .iter_current_update_events()
            .next()
            .expect("Reverse 应发 ReverseTriggeredEvent")
            .clone()
    };
    let reverse_returned = f64::from(reverse_event.returned_zone_qi);
    assert!(
        reverse_returned > 0.0,
        "Reverse 应产生 returned_zone_qi > 0（mark 被爆发），实际={reverse_returned}"
    );

    app.update(); // reverse_zone_credit_tick 消费 ReverseTriggeredEvent

    // ── 守恒验证（事件层面，不依赖 zone delta 避免 clamp 截断掩盖）──
    let total_returned = eclipse_returned + reverse_returned;
    let injected_ratio = eclipse_injected * DUGU_DIRTY_QI_ZONE_RETURN_RATIO;

    // 守恒上界：total ≤ injected×ratio（距离衰减只会让 total 更小，不会更大）
    // 旧 bug：double-entry 使 total ≈ 2×injected×ratio = 2×39.6 = 79.2
    // 修法 ②：total ≈ attenuated×ratio（<= injected×ratio，差距 = attenuation，约 4-5%）
    assert!(
        total_returned <= injected_ratio + 1e-9,
        "Eclipse→Reverse 连招通胀：\n\
         total_returned={total_returned:.6} > injected×ratio={injected_ratio:.6}\n\
         说明双重入账（旧 bug 复发；应 total ≤ injected×ratio）"
    );

    // 守恒下界：total 不应低于 injected×ratio×0.90（attenuation 不应超过 10% at dist=1）
    assert!(
        total_returned >= injected_ratio * 0.90,
        "Eclipse→Reverse 连招通缩异常：\n\
         total_returned={total_returned:.6} 低于 injected×ratio×0.90={:.6}\n\
         说明有大量 qi 在 chain 中消失（物理衰减过大或有通缩漏洞）",
        injected_ratio * 0.90
    );

    // 额外验证：Reverse.returned > Eclipse.returned（大部分散逸在 Reverse 延迟发生）
    assert!(
        reverse_returned > eclipse_returned,
        "Reverse.returned({reverse_returned:.4}) 应 > Eclipse.returned({eclipse_returned:.4})；\
         effective_hit >> rejected_qi（大部分脏真元入体延迟散逸）"
    );

    // 审计记录数：Eclipse 1 条 + Reverse 1 条 = 2 条
    let account = app.world().resource::<WorldQiAccount>();
    let dugu_count = account
        .transfers()
        .iter()
        .filter(|t| t.reason == QiTransferReason::DuguReturnToZone)
        .count();
    assert_eq!(
        dugu_count, 2,
        "Eclipse+Reverse 连招应产生恰好 2 条 DuguReturnToZone 审计记录，实际={dugu_count}"
    );
}

/// 边界：Reverse returned_zone_qi=0 时不产生审计记录。
#[test]
fn reverse_zero_returned_zone_qi_no_audit() {
    use crate::qi_physics::ledger::{QiTransferReason, WorldQiAccount};

    let mut app = setup_zone_credit_app(0.5);
    let caster = app.world_mut().spawn_empty().id();

    app.world_mut().send_event(ReverseTriggeredEvent {
        caster,
        affected_targets: 1,
        burst_damage: 10.0,
        returned_zone_qi: 0.0, // 边界：零返还
        juebi_delay_ticks: None,
        tick: 1,
        center: valence::math::DVec3::ZERO,
        visual: crate::combat::dugu_v2::skills::visual_for(DuguSkillId::Reverse),
    });

    app.update();

    let account = app.world().resource::<WorldQiAccount>();
    let count = account
        .transfers()
        .iter()
        .filter(|t| t.reason == QiTransferReason::DuguReturnToZone)
        .count();
    assert_eq!(
        count, 0,
        "returned_zone_qi=0 时 Reverse 不应产生 DuguReturnToZone 审计记录，实际={count}"
    );
}

// ── QS-DUGU2-CASTCOST: 施法成本 zone credit 守恒测试 ─────────────────────────
//
// 修复前：dugu_v2 施法扣 qi_current 后无任何 zone 回灌 → total_observed 每次施法减少。
// 修复后：release_cast_cost_to_zone 把成本走 qi_release_to_zone → zone.spirit_qi 增加
//         + emit QiTransfer(ReleaseToZone) 留审计轨迹。
//
// 关键陷阱（前三轮 CI 教训）：
//   (a) 使用 spirit_qi=0.0 的空 zone，确保 room=50 >> cost（Eclipse=13），全额入账不截断。
//   (b) caster entity 需有 CurrentDimension(Overworld)，否则 find_zone 返回 None
//       → 成本路由 overflow，zone.spirit_qi 不变，断言失败。

/// 构建用于施法成本 zone credit 测试的 App（带 ZoneRegistry + QiTransfer event）。
///
/// - zone 初始为空（spirit_qi=0.0）→ room=50，所有 cost 均全额入账，无 split。
/// - 注册 QiTransfer event，以便验证 release_cast_cost_to_zone 发出的审计事件。
fn setup_cast_cost_zone_app() -> App {
    use crate::qi_physics::ledger::QiTransfer;
    use crate::qi_physics::ledger::WorldQiAccount;
    use crate::world::dimension::DimensionKind;
    use crate::world::zone::{Zone, ZoneRegistry};
    use valence::prelude::DVec3;

    let mut app = setup_app();
    // NOTE: 不调 crate::combat::dugu_v2::register(&mut app)，
    //       避免 eclipse_zone_credit_tick 在 update() 时消费 EclipseNeedleEvent 干扰 zone delta。
    // 施法成本 zone credit 在 resolve_dugu_v2_skill() 内同步完成，不需要 tick 系统。
    app.add_event::<QiTransfer>();

    // 空 zone（spirit_qi=0.0）覆盖玩家坐标 [0,64,0]，确保 room=50 >> 任何单次施法成本
    let zone = Zone {
        name: "spawn".to_string(),
        dimension: DimensionKind::Overworld,
        bounds: (
            DVec3::new(-100.0, 0.0, -100.0),
            DVec3::new(100.0, 200.0, 100.0),
        ),
        spirit_qi: 0.0,
        danger_level: 0,
        active_events: Vec::new(),
        patrol_anchors: Vec::new(),
        blocked_tiles: Vec::new(),
    };
    app.insert_resource(ZoneRegistry { zones: vec![zone] });
    app.insert_resource(WorldQiAccount::default());
    app
}

/// 构建带 CurrentDimension 的 actor（caster 必须有此组件才能定位 zone）。
fn actor_with_dim(
    app: &mut App,
    realm: Realm,
    qi_current: f64,
    qi_max: f64,
    x: f64,
) -> valence::prelude::Entity {
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    app.world_mut()
        .spawn((
            Cultivation {
                realm,
                qi_current,
                qi_max,
                ..Default::default()
            },
            QiColor::default(),
            SkillBarBindings::default(),
            Wounds::default(),
            Position::new([x, 64.0, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
        ))
        .id()
}

/// happy path：Eclipse 施法后 zone.spirit_qi 精确增加 cast cost（13.0）/CAP（50.0）。
///
/// 守恒不变式：cast cost 从 player qi_current 取走后，必须等量入账 zone，
/// 不凭空蒸发（QS-DUGU2-CASTCOST 根因修复验证）。
#[test]
fn cast_cost_zone_credit_eclipse_happy_path() {
    use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    use crate::qi_physics::ledger::{QiTransfer, QiTransferReason};
    use crate::world::zone::ZoneRegistry;
    use valence::prelude::Events;

    let mut app = setup_cast_cost_zone_app();

    // Eclipse qi_cost=13.0；给 caster 足够的 qi
    let caster = actor_with_dim(&mut app, Realm::Spirit, 100.0, 100.0, 0.0);
    let target = actor_with_dim(&mut app, Realm::Spirit, 200.0, 200.0, 1.0);

    let zone_before = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .expect("spawn zone should exist")
        .spirit_qi;
    assert!(
        (zone_before - 0.0).abs() < 1e-12,
        "前提：zone 初始应为空，实际={zone_before}"
    );
    let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;

    let result = resolve_dugu_v2_skill(
        app.world_mut(),
        caster,
        0,
        Some(target),
        DuguSkillId::Eclipse,
    );
    assert!(
        matches!(result, CastResult::Started { .. }),
        "Eclipse cast 应成功，实际={result:?}"
    );

    let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
    let eclipse_cost = 13.0_f64;
    assert!(
        (qi_before - qi_after - eclipse_cost).abs() < 1e-9,
        "caster qi_current 应扣减 Eclipse cost({eclipse_cost})，\
         before={qi_before} after={qi_after} delta={:.9}（期望 {eclipse_cost}）",
        qi_before - qi_after
    );

    // zone.spirit_qi 应精确增加 eclipse_cost / QI_ZONE_UNIT_CAPACITY
    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .expect("spawn zone should exist")
        .spirit_qi;
    let expected_zone = zone_before + eclipse_cost / QI_ZONE_UNIT_CAPACITY;
    assert!(
        (zone_after - expected_zone).abs() < 1e-9,
        "Eclipse 施法成本({eclipse_cost})应全额入账 zone：\
         zone_before={zone_before} zone_after={zone_after:.9} expected={expected_zone:.9}\n\
         若 zone_after==zone_before，说明 QS-DUGU2-CASTCOST 修复未生效（cast cost 蒸发）"
    );

    // 应有 ReleaseToZone QiTransfer 审计事件
    let events = app.world().resource::<Events<QiTransfer>>();
    let transfers: Vec<_> = events
        .iter_current_update_events()
        .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
        .collect();
    assert!(
        !transfers.is_empty(),
        "Eclipse cast 应 emit ReleaseToZone QiTransfer；未找到 — ledger 守恒轨迹丢失"
    );
    let total: f64 = transfers.iter().map(|t| t.amount).sum();
    assert!(
        (total - eclipse_cost).abs() < 1e-9,
        "ReleaseToZone 总量({total:.9}) 应等于 Eclipse cast cost({eclipse_cost:.1})，\
         守恒：cast cost == zone credit（当 zone 未满时）"
    );
}

/// happy path：SelfCure 施法（qi_cost=10）后 zone 同样入账。
/// 验证非战斗型技能的施法成本也被正确回灌。
#[test]
fn cast_cost_zone_credit_self_cure_happy_path() {
    use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    use crate::qi_physics::ledger::QiTransfer;
    use crate::qi_physics::ledger::QiTransferReason;
    use crate::world::zone::ZoneRegistry;
    use valence::prelude::Events;

    let mut app = setup_cast_cost_zone_app();
    let caster = actor_with_dim(&mut app, Realm::Awaken, 100.0, 100.0, 0.0);

    let zone_before = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;
    let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;

    let result = resolve_dugu_v2_skill(app.world_mut(), caster, 0, None, DuguSkillId::SelfCure);
    assert!(
        matches!(result, CastResult::Started { .. }),
        "SelfCure cast 应成功，实际={result:?}"
    );

    let self_cure_cost = 10.0_f64;
    let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
    assert!(
        (qi_before - qi_after - self_cure_cost).abs() < 1e-9,
        "SelfCure 应扣 {self_cure_cost} qi，before={qi_before} after={qi_after}"
    );

    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;
    let expected_zone = zone_before + self_cure_cost / QI_ZONE_UNIT_CAPACITY;
    assert!(
        (zone_after - expected_zone).abs() < 1e-9,
        "SelfCure 施法成本({self_cure_cost})应入账 zone；\
         zone_after={zone_after:.9} expected={expected_zone:.9}"
    );

    let events = app.world().resource::<Events<QiTransfer>>();
    let has_release = events
        .iter_current_update_events()
        .any(|t| t.reason == QiTransferReason::ReleaseToZone);
    assert!(
        has_release,
        "SelfCure cast 应 emit ReleaseToZone QiTransfer event"
    );
}

/// 边界：caster 没有 Position 时（zone 无法定位），成本仍从 qi_current 扣减，
/// QiTransfer 走 overflow 账户（qi 不蒸发，zone 不变）。
/// 注：实现要求 Position **和** CurrentDimension 同时存在才解析 zone；缺任一都路由 overflow，
/// 无 Overworld 默认回退（见 release_cast_cost_to_zone 的 `dimension = get::<CurrentDimension>`）。
#[test]
fn cast_cost_without_position_routes_to_overflow() {
    use crate::qi_physics::ledger::QiTransfer;
    use crate::qi_physics::ledger::QiTransferReason;
    use crate::world::zone::ZoneRegistry;
    use valence::prelude::Events;

    let mut app = setup_cast_cost_zone_app();

    // 直接构造一个没有 Position 的 caster（技能调用里 release_cast_cost_to_zone
    // position == None → 走 overflow fallback）
    // 但 resolve_dugu_v2_skill 需要 TaintMark 等，改用 SelfCure 不需要目标
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    let caster = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            QiColor::default(),
            SkillBarBindings::default(),
            Wounds::default(),
            // 故意不插入 Position
            CurrentDimension(DimensionKind::Overworld),
        ))
        .id();

    let zone_before = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;
    let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;

    let result = resolve_dugu_v2_skill(app.world_mut(), caster, 0, None, DuguSkillId::SelfCure);
    assert!(matches!(result, CastResult::Started { .. }));

    // qi_current 仍被扣减（无论 zone 是否可达）
    let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
    let self_cure_cost = 10.0_f64;
    assert!(
        (qi_before - qi_after - self_cure_cost).abs() < 1e-9,
        "无 Position 时 qi_current 仍应扣减 {self_cure_cost}，\
         before={qi_before} after={qi_after}"
    );

    // zone.spirit_qi 不应变化（找不到 zone → overflow）
    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;
    assert!(
        (zone_after - zone_before).abs() < 1e-12,
        "无 Position 时 zone.spirit_qi 不应变化（走 overflow），\
         before={zone_before} after={zone_after}"
    );

    // 但 QiTransfer(ReleaseToZone) event 应仍被发出（走 overflow account，qi 不蒸发）
    let events = app.world().resource::<Events<QiTransfer>>();
    let has_release = events
        .iter_current_update_events()
        .any(|t| t.reason == QiTransferReason::ReleaseToZone);
    assert!(
        has_release,
        "无 Position 时仍应 emit ReleaseToZone QiTransfer（overflow 路径，qi 不蒸发）"
    );
}

/// happy path：Reverse 额外成本（extra_qi_cost = REVERSE_QI_PER_TARGET × N 个目标）
/// 也必须入账 zone，不蒸发（QS-DUGU2-CASTCOST 修复核心验证）。
///
/// 执行顺序：
///   1. apply_reverse 内：extra_cost=30 走 release_cast_cost_to_zone
///      zone: 0→0.6（30 qi 全额入账，room=50）
///   2. resolve_dugu_v2_skill 后：base_cost=50 走 release_cast_cost_to_zone
///      zone_current=30, room=20 → accepted=20 入账, overflow=30 静默丢弃（与 zhenmai_v2 一致）
///      zone: 0.6→1.0（饱和）
///
/// 守恒锁定点：
///   - qi_current 扣减 total_cost = 80（两次合计）
///   - zone 从 0 增到 1.0（zone 已满，extra+部分base入账）
///   - ReleaseToZone 事件 ≥1 条（存在审计轨迹）
#[test]
fn cast_cost_zone_credit_reverse_includes_extra_cost() {
    use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    use crate::qi_physics::ledger::QiTransfer;
    use crate::qi_physics::ledger::QiTransferReason;
    use crate::world::zone::ZoneRegistry;
    use valence::prelude::Events;

    let mut app = setup_cast_cost_zone_app();

    // Reverse 需要 Void 境 + 足够的 qi（base=50 + extra=30×1=30 → 共 80）
    let void_caster = actor_with_dim(&mut app, Realm::Void, 500.0, 500.0, 0.0);
    let victim = actor_with_dim(&mut app, Realm::Spirit, 200.0, 200.0, 1.0);
    // victim 需要 TaintMark（caster=void_caster, permanent）才能 Reverse
    app.world_mut().entity_mut(victim).insert(TaintMark {
        caster: void_caster,
        intensity: 5.0,
        since_tick: 1,
        expires_at_tick: None,
        tier: TaintTier::Permanent,
        temporary_qi_max_loss: 0.0,
        permanent_decay_rate_per_min: 0.001,
        returned_zone_qi: 4.95,
    });

    let zone_before = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;
    let qi_before = app
        .world()
        .get::<Cultivation>(void_caster)
        .unwrap()
        .qi_current;

    let result = resolve_dugu_v2_skill(
        app.world_mut(),
        void_caster,
        0,
        Some(victim),
        DuguSkillId::Reverse,
    );
    assert!(
        matches!(result, CastResult::Started { .. }),
        "Reverse cast 应成功，实际={result:?}"
    );

    let qi_after = app
        .world()
        .get::<Cultivation>(void_caster)
        .unwrap()
        .qi_current;
    let reverse_base_cost = 50.0_f64;
    let reverse_extra_cost = 30.0_f64; // REVERSE_QI_PER_TARGET × 1 target
    let total_cost = reverse_base_cost + reverse_extra_cost;
    assert!(
        (qi_before - qi_after - total_cost).abs() < 1e-9,
        "Reverse 应扣除 base({reverse_base_cost}) + extra({reverse_extra_cost}) = {total_cost} qi，\
         before={qi_before} after={qi_after} delta={:.9}（期望 {total_cost}）",
        qi_before - qi_after
    );

    // zone 从 0 开始（capacity=50 qi）：
    // extra_cost=30 先入账（zone 0→0.6），然后 base_cost=50，room 剩 20 → accepted=20（zone 0.6→1.0）
    // total_accepted = 30 + 20 = 50 → zone 饱和（spirit_qi = 1.0）
    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;

    // zone delta > 0（两次施法成本回灌）
    assert!(
        zone_after > zone_before,
        "Reverse 施法成本应入账 zone（zone_before={zone_before} zone_after={zone_after}）;\
         若 zone_after==zone_before 说明 QS-DUGU2-CASTCOST extra_cost 修复未生效"
    );

    // extra_cost(30) 先入空 zone（room=50）：accepted=30, zone: 0→0.6
    // base_cost(50) 后入：zone_current_abs=30, room=20, accepted=20, zone: 0.6→1.0
    let accepted_extra = reverse_extra_cost
        .min(QI_ZONE_UNIT_CAPACITY - zone_before.max(0.0) * QI_ZONE_UNIT_CAPACITY); // = 30
    let zone_after_extra = zone_before + accepted_extra / QI_ZONE_UNIT_CAPACITY; // = 0.6
    let room_for_base = (QI_ZONE_UNIT_CAPACITY - zone_after_extra * QI_ZONE_UNIT_CAPACITY).max(0.0);
    let accepted_base = reverse_base_cost.min(room_for_base); // = 20
    let expected_zone = (zone_after_extra + accepted_base / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
    assert!(
        (zone_after - expected_zone).abs() < 1e-9,
        "Reverse zone_after({zone_after:.9}) 应为 {expected_zone:.9}\
         （extra_accepted={accepted_extra}+base_accepted={accepted_base}入 zone）"
    );

    // 至少 1 条 ReleaseToZone 事件（extra_cost 产生一条；base_cost 若有 accepted>0 产生一条）
    let events = app.world().resource::<Events<QiTransfer>>();
    let release_events: Vec<_> = events
        .iter_current_update_events()
        .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
        .collect();
    assert!(
        !release_events.is_empty(),
        "Reverse cast 应 emit ≥1 ReleaseToZone QiTransfer（施法成本审计轨迹，QS-DUGU2-CASTCOST）"
    );
    // 守恒（#693）：扣减的 qi 全额入账，不蒸发。
    // - 入 zone 账户部分 == accepted_extra + accepted_base
    // - zone 部分饱和的 overflow（base_cost 的 30）显式路由到 overflow 账户
    // - 二者之和 == total_cost（80），ReleaseToZone 总量 == 全部扣减
    use crate::qi_physics::ledger::QiAccountKind;
    let zone_released: f64 = release_events
        .iter()
        .filter(|t| t.to.kind == QiAccountKind::Zone)
        .map(|t| t.amount)
        .sum();
    let overflow_released: f64 = release_events
        .iter()
        .filter(|t| t.to.kind == QiAccountKind::Overflow)
        .map(|t| t.amount)
        .sum();
    let expected_zone_released = accepted_extra + accepted_base; // = 50
    let expected_overflow_released = total_cost - expected_zone_released; // = 30（base 饱和溢出）
    assert!(
        (zone_released - expected_zone_released).abs() < 1e-9,
        "Reverse 入 zone 账户的 ReleaseToZone 总量({zone_released:.9}) 应 == \
         accepted_extra({accepted_extra}) + accepted_base({accepted_base}) = {expected_zone_released}"
    );
    assert!(
        (overflow_released - expected_overflow_released).abs() < 1e-9,
        "Reverse zone 饱和后的 overflow({overflow_released:.9}) 应显式路由到 overflow 账户 \
         = {expected_overflow_released}（base_cost 超出 zone room 的部分），不得静默丢弃（#693）"
    );
    let total_released: f64 = release_events.iter().map(|t| t.amount).sum();
    assert!(
        (total_released - total_cost).abs() < 1e-9,
        "守恒：ReleaseToZone 总量({total_released:.9}) 应 == 扣减的 total_cost({total_cost})，\
         zone 部分饱和时 overflow 真元必须显式入账而非蒸发（#693）"
    );
}

/// 错误分支：没有 ZoneRegistry 时，cast cost 仍扣减 qi_current，
/// QiTransfer event 路由到 overflow（zone 不可用但 qi 不蒸发）。
/// 这覆盖"旧测试不含 ZoneRegistry"场景，确保 existing tests 不被修复破坏。
#[test]
fn cast_cost_without_zone_registry_routes_to_overflow() {
    use crate::qi_physics::ledger::QiTransfer;
    use crate::qi_physics::ledger::QiTransferReason;
    use valence::prelude::Events;

    // 使用不含 ZoneRegistry 的 setup_app（模拟旧测试环境）
    let mut app = setup_app();
    app.add_event::<QiTransfer>();

    let caster = actor_with_dim(&mut app, Realm::Spirit, 100.0, 100.0, 0.0);
    let target = actor_with_dim(&mut app, Realm::Spirit, 200.0, 200.0, 1.0);

    let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;
    let result = resolve_dugu_v2_skill(
        app.world_mut(),
        caster,
        0,
        Some(target),
        DuguSkillId::Eclipse,
    );
    assert!(matches!(result, CastResult::Started { .. }));

    // qi_current 仍被扣减（ZoneRegistry 不影响 qi deduction）
    let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
    let eclipse_cost = 13.0_f64;
    assert!(
        (qi_before - qi_after - eclipse_cost).abs() < 1e-9,
        "无 ZoneRegistry 时 qi_current 仍应扣减 {eclipse_cost}"
    );

    // ReleaseToZone event 被发出（overflow 路径）
    let events = app.world().resource::<Events<QiTransfer>>();
    let has_release = events
        .iter_current_update_events()
        .any(|t| t.reason == QiTransferReason::ReleaseToZone);
    assert!(
        has_release,
        "无 ZoneRegistry 时仍应 emit ReleaseToZone QiTransfer（overflow 路径）"
    );
}

/// 边界：amount <= QI_EPSILON 时 release_cast_cost_to_zone 提前返回——
/// 不扣 qi_current、不发任何 QiTransfer、zone 不变（CodeRabbit #693 Minor 要求覆盖）。
#[test]
fn release_cast_cost_below_epsilon_is_noop() {
    use super::skills::release_cast_cost_to_zone;
    use crate::qi_physics::constants::QI_EPSILON;
    use crate::qi_physics::ledger::QiTransfer;
    use crate::world::zone::ZoneRegistry;

    let mut app = setup_cast_cost_zone_app();
    let caster = actor_with_dim(&mut app, Realm::Spirit, 100.0, 100.0, 0.0);

    let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;
    let zone_before = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;

    release_cast_cost_to_zone(app.world_mut(), caster, QI_EPSILON, "dugu_v2:test");

    let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
    assert!(
        (qi_before - qi_after).abs() < 1e-12,
        "amount<=QI_EPSILON 不应扣减 qi_current（before={qi_before} after={qi_after}）"
    );
    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;
    assert!(
        (zone_before - zone_after).abs() < 1e-12,
        "amount<=QI_EPSILON 不应改动 zone（before={zone_before} after={zone_after}）"
    );
    let count = app
        .world()
        .resource::<Events<QiTransfer>>()
        .iter_current_update_events()
        .count();
    assert_eq!(count, 0, "amount<=QI_EPSILON 不应 emit 任何 QiTransfer");
}

/// 错误分支：caster 缺少 Cultivation 时 release_cast_cost_to_zone 提前返回——
/// 没东西可扣就不能凭空向 zone/overflow 释放真元（CodeRabbit #693 Minor 要求覆盖）。
#[test]
fn release_cast_cost_no_cultivation_releases_nothing() {
    use super::skills::release_cast_cost_to_zone;
    use crate::qi_physics::ledger::QiTransfer;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use crate::world::zone::ZoneRegistry;

    let mut app = setup_cast_cost_zone_app();
    // 裸实体：有 Position + CurrentDimension 但**没有** Cultivation。
    let bare = app
        .world_mut()
        .spawn((
            Position::new([0.0, 64.0, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
        ))
        .id();

    let zone_before = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;

    release_cast_cost_to_zone(app.world_mut(), bare, 50.0, "dugu_v2:test");

    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;
    assert!(
        (zone_before - zone_after).abs() < 1e-12,
        "caster 无 Cultivation 时不应向 zone 释放真元（before={zone_before} after={zone_after}）"
    );
    let count = app
        .world()
        .resource::<Events<QiTransfer>>()
        .iter_current_update_events()
        .count();
    assert_eq!(
        count, 0,
        "caster 无 Cultivation 时不应凭空 emit QiTransfer（没东西可扣）"
    );
}

/// 防御性守恒（CodeRabbit #693 Minor）：调用方漏做余额预检、qi_current < amount 时，
/// clamp 后实际只扣减 qi_current，释放金额必须 == 实际扣减量，绝不凭空多出 amount-qi_current。
#[test]
fn release_cast_cost_caps_release_to_actual_deduction() {
    use super::skills::release_cast_cost_to_zone;
    use crate::qi_physics::ledger::{QiTransfer, QiTransferReason};

    let mut app = setup_cast_cost_zone_app();
    // qi_current=5 但要求释放 80：实际只能扣 5。
    let caster = actor_with_dim(&mut app, Realm::Spirit, 5.0, 100.0, 0.0);

    release_cast_cost_to_zone(app.world_mut(), caster, 80.0, "dugu_v2:test");

    let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
    assert!(
        qi_after.abs() < 1e-12,
        "qi_current 应被扣到 0（before=5, amount=80），实际={qi_after}"
    );
    let released: f64 = app
        .world()
        .resource::<Events<QiTransfer>>()
        .iter_current_update_events()
        .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
        .map(|t| t.amount)
        .sum();
    assert!(
        (released - 5.0).abs() < 1e-9,
        "释放总量应 == 实际扣减量 5（非请求的 80），实际={released:.9}；\
         否则凭空多出 75 真元破守恒（#693 防御性守恒）"
    );
}
