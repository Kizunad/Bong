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
    let outcome = dirty_qi_collision(100.0, 0.0, 0.0);
    assert!(outcome.effective_hit > 98.0);
    assert!(outcome.rejected_qi < 2.0);
    assert!((outcome.returned_zone_qi - 99.0).abs() < 1e-6);
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
    assert_eq!(
        v.icon_texture,
        "bong:textures/gui/skill/dugu_eclipse.png"
    );
}

#[test]
fn dugu_visual_ids_pin_self_cure() {
    use super::skills::visual_for;
    let v = visual_for(DuguSkillId::SelfCure);
    assert_eq!(v.animation_id, "bong:dugu_self_cure_pose");
    assert_eq!(v.particle_id, "bong:dugu_dark_green_mist");
    assert_eq!(v.sound_recipe_id, "dugu_self_cure_drink");
    assert_eq!(v.hud_hint, "自蕴");
    assert_eq!(
        v.icon_texture,
        "bong:textures/gui/skill/dugu_self_cure.png"
    );
}

#[test]
fn dugu_visual_ids_pin_penetrate() {
    use super::skills::visual_for;
    let v = visual_for(DuguSkillId::Penetrate);
    assert_eq!(v.animation_id, "bong:dugu_needle_throw");
    assert_eq!(v.particle_id, "bong:dugu_taint_pulse");
    assert_eq!(v.sound_recipe_id, "dugu_needle_hiss");
    assert_eq!(v.hud_hint, "侵染");
    assert_eq!(
        v.icon_texture,
        "bong:textures/gui/skill/dugu_penetrate.png"
    );
}

#[test]
fn dugu_visual_ids_pin_shroud() {
    use super::skills::visual_for;
    let v = visual_for(DuguSkillId::Shroud);
    assert_eq!(v.animation_id, "bong:dugu_shroud_activate");
    assert_eq!(v.particle_id, "bong:dugu_dark_green_mist");
    assert_eq!(v.sound_recipe_id, "dugu_self_cure_drink");
    assert_eq!(v.hud_hint, "神识遮蔽");
    assert_eq!(
        v.icon_texture,
        "bong:textures/gui/skill/dugu_shroud.png"
    );
}

#[test]
fn dugu_visual_ids_pin_reverse() {
    use super::skills::visual_for;
    let v = visual_for(DuguSkillId::Reverse);
    assert_eq!(v.animation_id, "bong:dugu_pointing_curse");
    assert_eq!(v.particle_id, "bong:dugu_reverse_burst");
    assert_eq!(v.sound_recipe_id, "dugu_curse_cackle");
    assert_eq!(v.hud_hint, "倒蚀");
    assert_eq!(
        v.icon_texture,
        "bong:textures/gui/skill/dugu_reverse.png"
    );
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
    let entity = app
        .world_mut()
        .spawn(Position::new([0.0, 64.0, 0.0]))
        .id();

    emit_anim(app.world_mut(), entity, "bong:dugu_needle_throw");

    // Should emit zero VfxEventRequest because UniqueId is missing
    assert_eq!(
        app.world().resource::<Events<VfxEventRequest>>().len(),
        0,
        "emit_anim should skip PlayAnim when entity has no UniqueId"
    );
}
