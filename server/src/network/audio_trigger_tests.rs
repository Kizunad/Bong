#![allow(dead_code, unused_imports)]
use super::*;
use crate::combat::components::{BodyPart, WoundKind, Wounds};
use crate::combat::events::CombatEvent;
use crate::forge::session::{ForgeSession, ForgeSessionId};
use valence::prelude::{App, Events, Update};
use valence::testing::{create_mock_client, MockClientHelper};

#[test]
fn jiemai_combat_event_emits_parry_recipe() {
    let mut app = App::new();
    app.add_event::<CombatEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_combat_audio_triggers);
    let attacker = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    let target = app.world_mut().spawn(Position::new([1.0, 64.0, 0.0])).id();
    app.world_mut().send_event(CombatEvent {
        attacker,
        target,
        resolved_at_tick: 1,
        body_part: BodyPart::Chest,
        wound_kind: WoundKind::Blunt,
        source: crate::combat::events::AttackSource::Melee,
        debug_command: false,
        physical_damage: 0.0,
        damage: 0.4,
        contam_delta: 0.0,
        description: "test jiemai=true".to_string(),
        defense_kind: Some(DefenseKind::JieMai),
        defense_effectiveness: Some(0.9),
        defense_contam_reduced: None,
        defense_wound_severity: None,
    });

    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].recipe_id, "parry_perfect");
}

#[test]
fn combat_hit_event_emits_tiered_recipe_and_wound() {
    let mut app = App::new();
    app.init_resource::<AudioImplementationDedup>();
    app.add_event::<CombatEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_combat_audio_triggers);
    let attacker = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    let target = app.world_mut().spawn(Position::new([1.0, 64.0, 0.0])).id();
    for _ in 0..2 {
        app.world_mut().send_event(CombatEvent {
            attacker,
            target,
            resolved_at_tick: 5,
            body_part: BodyPart::Chest,
            wound_kind: WoundKind::Blunt,
            source: crate::combat::events::AttackSource::Melee,
            debug_command: false,
            physical_damage: 0.0,
            damage: 12.0,
            contam_delta: 0.0,
            description: "test hit tier".to_string(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        });
    }

    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    let recipes: Vec<_> = emitted.into_iter().map(|event| event.recipe_id).collect();
    assert_eq!(recipes, vec!["hit_heavy", "wound_inflict"]);
}

/// plan-combat-hit-location-v1 P3 — 部位差异视听反馈：头部命中即便伤害轻微（低于
/// hit_heavy 的 10.0 分级线）也要走专属 combat_hit_head_crit，而非退化成 hit_light。
#[test]
fn head_hit_event_emits_head_crit_recipe_regardless_of_damage_tier() {
    let mut app = App::new();
    app.init_resource::<AudioImplementationDedup>();
    app.add_event::<CombatEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_combat_audio_triggers);
    let attacker = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    let target = app.world_mut().spawn(Position::new([1.0, 64.0, 0.0])).id();
    app.world_mut().send_event(CombatEvent {
        attacker,
        target,
        resolved_at_tick: 5,
        body_part: BodyPart::Head,
        wound_kind: WoundKind::Blunt,
        source: crate::combat::events::AttackSource::Melee,
        debug_command: false,
        physical_damage: 0.0,
        // 故意选一个低于 hit_heavy(10.0)/hit_critical(24.0) 分级线的伤害，
        // 证明部位路由优先于伤害分级——不这样测就无法区分"恰好碰上高伤害"的假阳性。
        damage: 2.0,
        contam_delta: 0.0,
        description: "test head crit routes before damage tier".to_string(),
        defense_kind: None,
        defense_effectiveness: None,
        defense_contam_reduced: None,
        defense_wound_severity: None,
    });

    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    let recipes: Vec<_> = emitted.into_iter().map(|event| event.recipe_id).collect();
    assert_eq!(
        recipes,
        vec!["combat_hit_head_crit"],
        "轻伤头部命中应仍走专属 combat_hit_head_crit，而不是按伤害分级落回 hit_light \
             （命中要害的反馈不该被伤害数值淹没）"
    );
}

/// plan-combat-hit-location-v1 P3 — 四肢命中应统一走更闷的 combat_hit_limb，
/// 四个部位变体（ArmL/ArmR/LegL/LegR）都要命中同一条 recipe。
#[test]
fn limb_hit_events_emit_limb_recipe_for_all_four_variants() {
    for limb in [
        BodyPart::ArmL,
        BodyPart::ArmR,
        BodyPart::LegL,
        BodyPart::LegR,
    ] {
        let mut app = App::new();
        app.init_resource::<AudioImplementationDedup>();
        app.add_event::<CombatEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_combat_audio_triggers);
        let attacker = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        let target = app.world_mut().spawn(Position::new([1.0, 64.0, 0.0])).id();
        app.world_mut().send_event(CombatEvent {
            attacker,
            target,
            resolved_at_tick: 5,
            body_part: limb,
            wound_kind: WoundKind::Blunt,
            source: crate::combat::events::AttackSource::Melee,
            debug_command: false,
            physical_damage: 0.0,
            damage: 12.0,
            contam_delta: 0.0,
            description: format!("test limb hit routes to combat_hit_limb for {limb:?}"),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        });

        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        let recipes: Vec<_> = emitted.into_iter().map(|event| event.recipe_id).collect();
        assert_eq!(
            recipes,
            vec!["combat_hit_limb", "wound_inflict"],
            "四肢部位 {limb:?} 命中应路由到 combat_hit_limb（damage=12.0 仍越过 \
                 wound_inflict 的 8.0 阈值，两条 recipe 都应出现）"
        );
    }
}

/// 胸/腹/背命中不应受本次部位差异改动影响，维持既有伤害分级 recipe 选择。
#[test]
fn torso_hits_still_use_damage_tier_recipe_unaffected_by_body_part_routing() {
    for (part, damage, expected) in [
        (BodyPart::Chest, 3.0, "hit_light"),
        (BodyPart::Abdomen, 12.0, "hit_heavy"),
        (BodyPart::Back, 30.0, "hit_critical"),
    ] {
        let mut app = App::new();
        app.init_resource::<AudioImplementationDedup>();
        app.add_event::<CombatEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_combat_audio_triggers);
        let attacker = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        let target = app.world_mut().spawn(Position::new([1.0, 64.0, 0.0])).id();
        app.world_mut().send_event(CombatEvent {
            attacker,
            target,
            resolved_at_tick: 5,
            body_part: part,
            wound_kind: WoundKind::Blunt,
            source: crate::combat::events::AttackSource::Melee,
            debug_command: false,
            physical_damage: 0.0,
            damage,
            contam_delta: 0.0,
            description: format!("test torso hit {part:?} keeps damage tier recipe"),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        });

        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert_eq!(
            emitted[0].recipe_id, expected,
            "{part:?} 命中 damage={damage} 应维持既有分级 recipe {expected}，不受部位差异改动影响"
        );
    }
}

#[test]
fn npc_death_audio_waits_for_terminal_commit_and_attributes_kill() {
    let mut app = App::new();
    app.add_event::<NpcTerminalSettlementSucceeded>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_npc_death_audio_triggers);
    let npc = app
        .world_mut()
        .spawn((NpcMarker, Position::new([1.0, 64.0, 0.0])))
        .id();
    let attacker = app.world_mut().spawn_empty().id();

    app.update();
    assert!(
        app.world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .next()
            .is_none(),
        "terminal commit 前不得播放 NPC 死亡音效"
    );

    let life_record = crate::cultivation::life_record::LifeRecord::new("npc:audio:terminal");
    app.world_mut().send_event(NpcTerminalSettlementSucceeded {
        entity: npc,
        at_tick: 1,
        cause: "combat".to_string(),
        reason: crate::npc::lifecycle::NpcDeathReason::Combat,
        attacker: Some(attacker),
        attacker_player_id: Some("offline:attacker".to_string()),
        authorize_loot: true,
        actor_qi_identity: crate::cultivation::components::ActorQiIdentity::from_life_record(
            &life_record,
            crate::cultivation::components::ActorQiKind::Npc,
        )
        .expect("audio terminal fixture must use canonical NPC identity"),
    });

    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert_eq!(emitted.len(), 2);
    assert_eq!(emitted[0].recipe_id, "npc_death");
    assert_eq!(emitted[1].recipe_id, "kill_confirm");
    assert!(matches!(emitted[1].recipient, AudioRecipient::Single(entity) if entity == attacker));
}

#[test]
fn skill_lv_up_emits_player_local_recipe() {
    let mut app = App::new();
    app.add_event::<SkillXpGain>();
    app.add_event::<SkillLvUp>();
    app.add_event::<SkillScrollUsed>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_skill_audio_triggers);
    let player = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    app.world_mut().send_event(SkillLvUp {
        char_entity: player,
        skill: crate::skill::components::SkillId::Herbalism,
        new_lv: 2,
    });

    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].recipe_id, "skill_lv_up");
    assert!(matches!(emitted[0].recipient, AudioRecipient::Single(entity) if entity == player));
}

#[test]
fn blood_burn_audio_is_player_local() {
    use crate::schema::audio::AudioAttenuation;

    let registry = SoundRecipeRegistry::load_default().expect("default recipes should load");
    assert_eq!(
        registry
            .get("blood_burn_sizzle")
            .expect("blood burn recipe exists")
            .attenuation,
        AudioAttenuation::PlayerLocal,
    );
}

fn make_settled(
    entity: valence::prelude::Entity,
    outcome: crate::schema::tribulation::DuXuOutcomeV1,
) -> TribulationSettled {
    use crate::cultivation::tribulation::TribulationKind;
    use crate::schema::tribulation::DuXuResultV1;
    TribulationSettled {
        entity,
        kind: TribulationKind::DuXu,
        source: None,
        result: DuXuResultV1 {
            char_id: "test_char".to_string(),
            outcome,
            killer: None,
            waves_survived: 3,
            reason: None,
        },
    }
}

#[test]
fn tribulation_ascended_settled_emits_success_recipe() {
    // Expect: outcome=Ascended → emit_tribulation_audio_triggers emits "tribulation_ascend_success".
    let mut app = App::new();
    app.add_event::<TribulationAnnounce>();
    app.add_event::<JueBiTriggeredEvent>();
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<TribulationFailed>();
    app.add_event::<TribulationSettled>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_tribulation_audio_triggers);
    let entity = app
        .world_mut()
        .spawn(Position::new([10.0, 64.0, 10.0]))
        .id();

    app.world_mut().send_event(make_settled(
        entity,
        crate::schema::tribulation::DuXuOutcomeV1::Ascended,
    ));
    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert_eq!(
        emitted.len(),
        1,
        "expected exactly one PlaySoundRecipeRequest for Ascended outcome, got {}",
        emitted.len()
    );
    assert_eq!(
        emitted[0].recipe_id, "tribulation_ascend_success",
        "Ascended outcome must emit tribulation_ascend_success recipe, got {:?}",
        emitted[0].recipe_id
    );
}

#[test]
fn tribulation_halfstep_settled_emits_success_recipe() {
    // Expect: outcome=HalfStep → same recipe as Ascended (存活通过亦应有成功 AV).
    let mut app = App::new();
    app.add_event::<TribulationAnnounce>();
    app.add_event::<JueBiTriggeredEvent>();
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<TribulationFailed>();
    app.add_event::<TribulationSettled>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_tribulation_audio_triggers);
    let entity = app
        .world_mut()
        .spawn(Position::new([10.0, 64.0, 10.0]))
        .id();

    app.world_mut().send_event(make_settled(
        entity,
        crate::schema::tribulation::DuXuOutcomeV1::HalfStep,
    ));
    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert_eq!(
        emitted.len(),
        1,
        "expected exactly one PlaySoundRecipeRequest for HalfStep outcome, got {}",
        emitted.len()
    );
    assert_eq!(
        emitted[0].recipe_id, "tribulation_ascend_success",
        "HalfStep outcome must emit tribulation_ascend_success recipe, got {:?}",
        emitted[0].recipe_id
    );
}

#[test]
fn tribulation_failed_settled_does_not_emit_success_recipe() {
    // Expect: outcome=Failed → no success AV (failed path handled by TribulationFailed reader).
    let mut app = App::new();
    app.add_event::<TribulationAnnounce>();
    app.add_event::<JueBiTriggeredEvent>();
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<TribulationFailed>();
    app.add_event::<TribulationSettled>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_tribulation_audio_triggers);
    let entity = app
        .world_mut()
        .spawn(Position::new([10.0, 64.0, 10.0]))
        .id();

    app.world_mut().send_event(make_settled(
        entity,
        crate::schema::tribulation::DuXuOutcomeV1::Failed,
    ));
    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert!(
            emitted.is_empty(),
            "Failed outcome must not emit success recipe (handled by TribulationFailed reader), got {:?}",
            emitted.iter().map(|e| &e.recipe_id).collect::<Vec<_>>()
        );
}

#[test]
fn tribulation_killed_and_fled_settled_do_not_emit_success_recipe() {
    // Expect: outcome=Killed/Fled → no success AV (skipped, not gameplay-meaningful success).
    let mut app = App::new();
    app.add_event::<TribulationAnnounce>();
    app.add_event::<JueBiTriggeredEvent>();
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<TribulationFailed>();
    app.add_event::<TribulationSettled>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_tribulation_audio_triggers);
    let entity = app
        .world_mut()
        .spawn(Position::new([10.0, 64.0, 10.0]))
        .id();

    app.world_mut().send_event(make_settled(
        entity,
        crate::schema::tribulation::DuXuOutcomeV1::Killed,
    ));
    app.world_mut().send_event(make_settled(
        entity,
        crate::schema::tribulation::DuXuOutcomeV1::Fled,
    ));
    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert!(
        emitted.is_empty(),
        "Killed/Fled outcomes must not emit success recipe, got {:?}",
        emitted.iter().map(|e| &e.recipe_id).collect::<Vec<_>>()
    );
}

#[test]
fn existing_tribulation_failure_recipe_unaffected_by_settled_reader() {
    // Regression: TribulationFailed still emits realm_regression, adding settled reader
    // must not break the failure audio path.
    let mut app = App::new();
    app.add_event::<TribulationAnnounce>();
    app.add_event::<JueBiTriggeredEvent>();
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<TribulationFailed>();
    app.add_event::<TribulationSettled>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_tribulation_audio_triggers);
    let entity = app
        .world_mut()
        .spawn(Position::new([10.0, 64.0, 10.0]))
        .id();

    use crate::cultivation::tribulation::TribulationFailed;
    app.world_mut()
        .send_event(TribulationFailed { entity, wave: 1 });
    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert_eq!(
            emitted.len(),
            1,
            "TribulationFailed should still emit exactly one recipe after adding settled reader, got {}",
            emitted.len()
        );
    assert_eq!(
        emitted[0].recipe_id, "realm_regression",
        "TribulationFailed must emit realm_regression (regression guard), got {:?}",
        emitted[0].recipe_id
    );
}

#[test]
fn lingtian_actions_emit_dedicated_recipes() {
    let mut app = App::new();
    app.init_resource::<AudioImplementationDedup>();
    app.add_event::<TillCompleted>();
    app.add_event::<PlantingCompleted>();
    app.add_event::<HarvestCompleted>();
    app.add_event::<ReplenishCompleted>();
    app.add_event::<DrainQiCompleted>();
    app.add_event::<RenewCompleted>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_lingtian_audio_triggers);
    let player = app.world_mut().spawn_empty().id();
    let pos = valence::prelude::BlockPos::new(3, 64, 5);

    app.world_mut().send_event(TillCompleted {
        player,
        pos,
        hoe: crate::lingtian::hoe::HoeKind::Iron,
        hoe_instance_id: 1,
    });
    app.world_mut().send_event(TillCompleted {
        player,
        pos,
        hoe: crate::lingtian::hoe::HoeKind::Iron,
        hoe_instance_id: 2,
    });
    app.world_mut().send_event(PlantingCompleted {
        player,
        pos,
        plant_id: "ci_she_hao".to_string(),
    });
    app.world_mut().send_event(HarvestCompleted {
        player,
        pos,
        plant_id: "ci_she_hao".to_string(),
        seed_dropped: false,
    });
    app.world_mut().send_event(ReplenishCompleted {
        player,
        pos,
        source: crate::lingtian::session::ReplenishSource::Zone,
        plot_qi_added: 0.2,
        overflow_to_zone: 0.0,
    });
    app.world_mut().send_event(DrainQiCompleted {
        player,
        pos,
        plot_qi_drained: 0.3,
        qi_to_player: 0.24,
        qi_to_zone: 0.06,
    });

    app.update();

    let recipes: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .map(|request| request.recipe_id)
        .collect();
    assert_eq!(
            recipes,
            vec![
                "lingtian_till",
                "lingtian_plant_seed",
                "lingtian_harvest",
                "lingtian_replenish",
                "lingtian_drain"
            ],
            "regression: existing 5 lingtian sound recipes must not be broken by RenewCompleted addition"
        );
}

/// RenewCompleted emits lingtian_replenish audio cue (recipe reuse per scope decision).
///
/// Expectation: emit RenewCompleted → emit_lingtian_audio_triggers →
/// PlaySoundRecipeRequest with recipe_id="lingtian_replenish" at the correct pos.
#[test]
fn renew_completed_emits_lingtian_replenish_audio() {
    let mut app = App::new();
    app.init_resource::<AudioImplementationDedup>();
    app.add_event::<TillCompleted>();
    app.add_event::<PlantingCompleted>();
    app.add_event::<HarvestCompleted>();
    app.add_event::<ReplenishCompleted>();
    app.add_event::<DrainQiCompleted>();
    app.add_event::<RenewCompleted>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_lingtian_audio_triggers);

    let player = app.world_mut().spawn_empty().id();
    let pos = valence::prelude::BlockPos::new(10, 65, -3);

    app.world_mut().send_event(RenewCompleted {
        player,
        pos,
        hoe: crate::lingtian::hoe::HoeKind::Iron,
        hoe_instance_id: 77,
    });

    app.update();

    let recipes: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .map(|request| request.recipe_id)
        .collect();

    assert_eq!(
        recipes,
        vec!["lingtian_replenish"],
        "expected exactly one lingtian_replenish cue for RenewCompleted \
             (reuses replenish recipe per r2-P1 scope decision), got {recipes:?}"
    );
}

/// RenewCompleted: two distinct player entities each emit one lingtian_replenish cue.
///
/// AudioImplementationDedup deduplicates on (entity, recipe_id) per tick window.
/// Using distinct players confirms both events propagate independently.
/// (Same-player same-recipe same-tick dedup is tested implicitly by the dedup unit tests.)
#[test]
fn multiple_renew_completed_different_players_each_emit_cue() {
    let mut app = App::new();
    app.init_resource::<AudioImplementationDedup>();
    app.add_event::<TillCompleted>();
    app.add_event::<PlantingCompleted>();
    app.add_event::<HarvestCompleted>();
    app.add_event::<ReplenishCompleted>();
    app.add_event::<DrainQiCompleted>();
    app.add_event::<RenewCompleted>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_lingtian_audio_triggers);

    let player_a = app.world_mut().spawn_empty().id();
    let player_b = app.world_mut().spawn_empty().id();
    let pos = valence::prelude::BlockPos::new(0, 64, 0);

    app.world_mut().send_event(RenewCompleted {
        player: player_a,
        pos,
        hoe: crate::lingtian::hoe::HoeKind::Iron,
        hoe_instance_id: 1,
    });
    app.world_mut().send_event(RenewCompleted {
        player: player_b,
        pos,
        hoe: crate::lingtian::hoe::HoeKind::Iron,
        hoe_instance_id: 2,
    });

    app.update();

    let recipes: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .map(|request| request.recipe_id)
        .collect();

    assert_eq!(
        recipes,
        vec!["lingtian_replenish", "lingtian_replenish"],
        "expected one lingtian_replenish cue per distinct player entity, got {recipes:?}"
    );
}

#[test]
fn alchemy_events_emit_dedicated_recipes() {
    let mut app = App::new();
    app.add_event::<StartAlchemyRequest>();
    app.add_event::<AlchemyOutcomeEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_alchemy_audio_triggers);
    let furnace = app.world_mut().spawn(Position::new([3.0, 64.0, -2.0])).id();

    app.world_mut().send_event(StartAlchemyRequest {
        furnace,
        recipe_id: "hui_yuan_pill_v0".to_string(),
        caster_id: "offline:Azure".to_string(),
    });
    app.world_mut().send_event(AlchemyOutcomeEvent {
        furnace,
        caster_id: "offline:Azure".to_string(),
        recipe_id: Some("hui_yuan_pill_v0".to_string()),
        bucket: crate::alchemy::outcome::OutcomeBucket::Perfect,
        outcome: ResolvedOutcome::Pill {
            recipe_id: "hui_yuan_pill_v0".to_string(),
            pill: "hui_yuan_pill".to_string(),
            quality: 1.0,
            toxin_amount: 0.0,
            toxin_color: crate::cultivation::components::ColorKind::Mellow,
            qi_gain: Some(24.0),
            quality_tier: 3,
            effect_multiplier: 1.0,
            consecrated: true,
            side_effect: None,
            flawed_path: false,
        },
        elapsed_ticks: 120,
    });

    app.update();

    let recipes: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .map(|request| request.recipe_id)
        .collect();
    assert_eq!(recipes, vec!["alchemy_bubble", "alchemy_complete"]);
}

#[test]
fn forge_events_emit_dedicated_recipes() {
    let mut app = App::new();
    app.add_event::<ForgeStartAccepted>();
    app.add_event::<TemperingHit>();
    app.add_event::<ForgeOutcomeEvent>();
    app.init_resource::<ForgeSessions>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_forge_audio_triggers);
    let station = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    let caster = app.world_mut().spawn(Position::new([1.0, 64.0, 0.0])).id();
    let session_id = ForgeSessionId(1);
    let mut session = ForgeSession::new(session_id, "forge_test".to_string(), station, caster);
    session.current_step = ForgeStep::Tempering;
    app.world_mut()
        .resource_mut::<ForgeSessions>()
        .insert(session);

    app.world_mut().send_event(ForgeStartAccepted {
        session: session_id,
        station,
        caster,
        blueprint: "forge_test".to_string(),
        materials: vec![],
    });
    app.world_mut().send_event(TemperingHit {
        session: session_id,
        beat: TemperBeat::Heavy,
        ticks_remaining: 2,
    });
    app.world_mut().send_event(ForgeOutcomeEvent {
        session: session_id,
        caster,
        blueprint: "forge_test".to_string(),
        bucket: ForgeBucket::Perfect,
        weapon_item: None,
        quality: 1.0,
        color: None,
        side_effects: vec![],
        achieved_tier: 3,
        consecration_qi_amount: 0.0,
    });

    app.update();

    let recipes: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .map(|request| request.recipe_id)
        .collect();
    assert_eq!(
        recipes,
        vec!["forge_consecrate", "forge_hammer_heavy", "forge_complete"]
    );
}

#[test]
fn player_state_audio_uses_twenty_percent_low_hp_threshold() {
    let mut app = App::new();
    app.init_resource::<AudioTriggerState>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_event::<StopSoundRecipeRequest>();
    app.add_systems(Update, emit_player_state_audio_triggers);
    let (mut bundle, _helper) = create_mock_client("low_hp");
    bundle.player.position = Position::new([0.0, 64.0, 0.0]);
    let player = app.world_mut().spawn(bundle).id();
    app.world_mut().entity_mut(player).insert(Wounds {
        health_current: 25.0,
        health_max: 100.0,
        ..Default::default()
    });

    app.update();
    let first: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert!(
        first.is_empty(),
        "25% HP should not trigger the audio-world heartbeat"
    );

    app.world_mut().entity_mut(player).insert(Wounds {
        health_current: 19.0,
        health_max: 100.0,
        ..Default::default()
    });
    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].recipe_id, "heartbeat_low_hp");
    assert_eq!(emitted[0].flag.as_deref(), Some("hp_below_20"));
}

// ────────────────────────────────────────────────────────────────────
// 低血心跳 loop 生命周期（重生残留受伤音修复）
//
// 实机 bug：死亡→点重生后仍每秒响一次受伤音。抓包实证根因——`heartbeat_low_hp`
// 是 loop recipe（interval 20 ticks，第二层 `minecraft:entity.player.hurt`），
// server 只在血量跌破 20% 的上沿发一次 play、**从不发 stop**；client 侧带 flag
// 的 loop 又把 flag 自注册成 sticky，while_flag 判定永真 → loop 永生。
// 下面这组用例把「谁开谁关」锁死在 server 侧。
// ────────────────────────────────────────────────────────────────────

/// 构造一个跑心跳生命周期的最小 App：两条 audio 事件通道 + 上沿/下沿系统 + 重生系统。
/// 返回的 `MockClientHelper` 必须由调用方持住（drop 会断开 mock client 连接）。
fn heartbeat_app(username: &str) -> (App, Entity, MockClientHelper) {
    let mut app = App::new();
    app.init_resource::<AudioTriggerState>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_event::<StopSoundRecipeRequest>();
    app.add_event::<PlayerRevived>();
    app.add_systems(
        Update,
        (
            emit_player_state_audio_triggers,
            stop_low_hp_heartbeat_on_revive,
        ),
    );
    let (mut bundle, helper) = create_mock_client(username);
    bundle.player.position = Position::new([0.0, 64.0, 0.0]);
    let player = app.world_mut().spawn(bundle).id();
    (app, player, helper)
}

fn set_health(app: &mut App, player: Entity, health_current: f32) {
    app.world_mut().entity_mut(player).insert(Wounds {
        health_current,
        health_max: 100.0,
        ..Default::default()
    });
}

fn drain_plays(app: &mut App) -> Vec<PlaySoundRecipeRequest> {
    app.world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect()
}

fn drain_stops(app: &mut App) -> Vec<StopSoundRecipeRequest> {
    app.world_mut()
        .resource_mut::<Events<StopSoundRecipeRequest>>()
        .drain()
        .collect()
}

/// loop 必须带稳定 instance id（否则事后无从 stop——这是原 bug 的结构性成因）。
#[test]
fn low_hp_heartbeat_play_carries_stable_stoppable_instance_id() {
    let (mut app, player, _client) = heartbeat_app("hbid");
    set_health(&mut app, player, 10.0);

    app.update();

    let plays = drain_plays(&mut app);
    assert_eq!(plays.len(), 1, "跌破 20% 应只发一次心跳 loop play");
    assert_eq!(plays[0].recipe_id, "heartbeat_low_hp");
    assert_ne!(
            plays[0].instance_id, 0,
            "期望心跳 loop 带非 0 的稳定 instance id 因为 instance_id=0 会让 server 侧 allocator 现分配、\
             之后无法用同一 id 发 stop（loop 永生 → 重生后仍响受伤音）；实际 0"
        );
    assert_eq!(
            plays[0].instance_id,
            low_hp_heartbeat_instance_id(player),
            "期望 instance id 由玩家实体派生（与 stop 侧同一函数）因为收 loop 要按同一 id 对齐；实际不一致"
        );
}

/// 下沿（血量回到阈值以上）必须发 stop，且是发给该玩家自己。
#[test]
fn low_hp_heartbeat_stops_when_health_recovers_above_threshold() {
    let (mut app, player, _client) = heartbeat_app("hbrec");
    set_health(&mut app, player, 10.0);
    app.update();
    assert_eq!(drain_plays(&mut app).len(), 1, "先起心跳");
    assert!(
        drain_stops(&mut app).is_empty(),
        "上沿不该发 stop——loop 刚起来"
    );

    set_health(&mut app, player, 25.0);
    app.update();

    let stops = drain_stops(&mut app);
    assert_eq!(
        stops.len(),
        1,
        "期望血量回到 25%（> 20% 阈值）时发 1 条 stop 因为开 loop 的一方负责关掉它；实际 {} 条",
        stops.len()
    );
    assert_eq!(stops[0].instance_id, low_hp_heartbeat_instance_id(player));
    assert_eq!(
        stops[0].fade_out_ticks, 0,
        "期望硬停（fade_out_ticks=0）因为贴耳心跳淡出会在重生后拖出受伤尾音；实际有淡出"
    );
    assert_eq!(
        stops[0].recipient,
        AudioRecipient::Single(player),
        "期望只发给该玩家因为心跳是 player_local 私有反馈；实际收件人不对"
    );
    assert!(drain_plays(&mut app).is_empty(), "血量回升不该再发 play");
}

/// 持续低血只有一条 play、零 stop（心跳不能每 tick 重开，也不能自己断掉）。
#[test]
fn low_hp_heartbeat_is_not_restarted_or_stopped_while_health_stays_low() {
    let (mut app, player, _client) = heartbeat_app("hbhold");
    set_health(&mut app, player, 10.0);
    app.update();
    assert_eq!(drain_plays(&mut app).len(), 1);
    drain_stops(&mut app);

    set_health(&mut app, player, 5.0);
    app.update();
    app.update();

    assert!(
        drain_plays(&mut app).is_empty(),
        "期望持续低血不再重发 play 因为 loop 由 client 侧自行重放（上沿触发语义）；实际重发了"
    );
    assert!(
        drain_stops(&mut app).is_empty(),
        "期望持续低血不发 stop 因为条件还成立；实际误停了心跳"
    );
}

/// 血量归零立即停，重生事件仍会幂等收尾，不依赖生命周期快照先到。
#[test]
fn revive_stops_low_hp_heartbeat_loop() {
    let (mut app, player, _client) = heartbeat_app("hbrev");
    set_health(&mut app, player, 10.0);
    app.update();
    assert_eq!(drain_plays(&mut app).len(), 1, "低血先起心跳");
    drain_stops(&mut app);

    set_health(&mut app, player, 0.0);
    app.update();
    let stops = drain_stops(&mut app);
    assert_eq!(stops.len(), 1, "血量归零时必须立即停止心跳");
    assert_eq!(stops[0].instance_id, low_hp_heartbeat_instance_id(player));
    app.update();
    assert!(drain_plays(&mut app).is_empty(), "零血量不能重开心跳");

    app.world_mut().send_event(PlayerRevived { entity: player });
    app.update();

    let stops = drain_stops(&mut app);
    assert!(
        stops
            .iter()
            .any(|stop| stop.instance_id == low_hp_heartbeat_instance_id(player)),
        "期望 PlayerRevived 后发出针对该玩家心跳 instance 的 stop 因为重生必须干净、\
             不能残留 heartbeat_low_hp 的 entity.player.hurt 层；实际 stop 列表 {:?}",
        stops
            .iter()
            .map(|stop| stop.instance_id)
            .collect::<Vec<_>>()
    );
}

/// 实机回归：离开存活状态就停心跳，即使仍有残余低血快照也不能重新开启。
#[test]
fn nonliving_player_stops_heartbeat_without_rearming() {
    for state in [LifecycleState::AwaitingRevival, LifecycleState::Terminated] {
        let (mut app, player, _client) = heartbeat_app("hbdead");
        set_health(&mut app, player, 10.0);
        app.world_mut()
            .entity_mut(player)
            .insert(Lifecycle::default());
        app.update();
        let plays = drain_plays(&mut app);
        assert_eq!(plays.len(), 1, "存活低血应正常启动心跳");

        app.world_mut().get_mut::<Lifecycle>(player).unwrap().state = state;
        app.update();
        let stops = drain_stops(&mut app);
        assert_eq!(stops.len(), 1, "进入 {state:?} 必须停止心跳");
        assert_eq!(stops[0].instance_id, plays[0].instance_id);
        assert_eq!(stops[0].fade_out_ticks, 0, "死亡裁决中的心跳必须硬停");
        assert!(matches!(stops[0].recipient, AudioRecipient::Single(entity) if entity == player));
        app.update();
        assert!(drain_plays(&mut app).is_empty(), "{state:?} 不得重开心跳");

        app.world_mut()
            .entity_mut(player)
            .insert(Lifecycle::default());
        app.update();
        assert_eq!(drain_plays(&mut app).len(), 1, "后续存活玩家仍应有低血反馈");
    }
}

/// 重生后记账要清干净：下一次真掉血能重新起心跳（修复不能把低血反馈永久关死）。
#[test]
fn low_hp_heartbeat_rearms_after_revive_when_player_drops_low_again() {
    let (mut app, player, _client) = heartbeat_app("hbrearm");
    set_health(&mut app, player, 10.0);
    app.update();
    drain_plays(&mut app);

    app.world_mut().send_event(PlayerRevived { entity: player });
    // 重生把血量拉回 REVIVE_HEALTH_FRACTION（20%）——正好在阈值上，不该再起心跳。
    set_health(&mut app, player, 20.0);
    app.update();
    drain_stops(&mut app);
    assert!(
        drain_plays(&mut app).is_empty(),
        "期望重生血量（20% = 阈值）不重开心跳因为判据是严格小于阈值；实际重开了"
    );

    // 之后再被打到 15% —— 心跳必须回来（低血反馈没被永久关死）。
    set_health(&mut app, player, 15.0);
    app.update();

    let plays = drain_plays(&mut app);
    assert_eq!(
        plays.len(),
        1,
        "期望重生后再次跌破 20% 能重新起心跳因为重生只清记账、不禁用低血反馈；实际 {} 条 play",
        plays.len()
    );
    assert_eq!(plays[0].recipe_id, "heartbeat_low_hp");
}

/// 比常数 pin 更严的同族 pin：**照生产的 f32 算术**把重生血量算出来再过判据。
///
/// 生产链路是 `health_current = (health_max * REVIVE_HEALTH_FRACTION).max(1.0)`
/// （`combat::lifecycle::revive_lifecycle`）→ `hp_ratio = health_current / health_max.max(1.0)`
/// （本文件的心跳系统）。f32 舍入让「比例常数 >= 阈值」并不能推出「算出来的商 >= 阈值」：
/// health_max = 20.5 / 41.0 / 82.0 等取值下商会落到 0.19999999 < 0.2（实算复核过 41.0：
/// `41 × 0.2f32` 舍入成 8.19999980926，除 41 得 0.199999995），重生那一刻又自动起一条含
/// `entity.player.hurt` 层的心跳。今天玩家 health_max 恒为 `Wounds::default()` 的 100.0
/// （100 × 0.2 = 20.0，商恰好 0.2）所以安全。
///
/// **覆盖边界（别过度指望这条）**：它只盯 `Wounds::default()` 这一个来源，所以能挡住
/// 改 `DEFAULT_HEALTH_MAX` / `REVIVE_HEALTH_FRACTION`。若将来按境界/属性走**运行时赋值**
/// 改玩家 `health_max`（不动 Default），这条 pin 不会撞红——那种改法必须自己重算这个商。
#[test]
fn revive_health_ratio_computed_like_production_never_rearms_heartbeat() {
    let health_max = Wounds::default().health_max;
    let revived_health = (health_max * crate::combat::components::REVIVE_HEALTH_FRACTION).max(1.0);
    let revived_ratio = revived_health / health_max.max(1.0);
    assert!(
        !is_low_hp_for_heartbeat(revived_ratio),
        "期望按生产算术算出的重生血量比例 {revived_ratio}（health_max={health_max} → \
             health_current={revived_health}）不触发低血心跳（阈值 {LOW_HP_HEARTBEAT_RATIO}）——\
             f32 舍入一旦让商落到阈值之下，重生瞬间就会自动起一条含 entity.player.hurt 层的\
             心跳 loop；改动 health_max / REVIVE_HEALTH_FRACTION 时必须同时重设计心跳触发",
    );
}

#[test]
fn revive_health_fraction_never_rearms_low_hp_heartbeat() {
    // 对着**生产判据** is_low_hp_for_heartbeat 断言，而不是在测试里重写比较。
    let revive_hp_ratio = crate::combat::components::REVIVE_HEALTH_FRACTION;
    assert!(
        !is_low_hp_for_heartbeat(revive_hp_ratio),
        "期望重生血量比例 {revive_hp_ratio} 不触发低血心跳（阈值 {LOW_HP_HEARTBEAT_RATIO}）——\
             否则重生那一刻就会自动起一条含 entity.player.hurt 层的心跳 loop，玩家听到的就是\
             「重生就有受伤音」；要调低重生血量必须同时重设计心跳触发",
    );
    // 反向对照：略低于重生血量的 hp 必须仍被判为低血（防止有人把判据改成恒 false 让本测试蒙过）。
    assert!(
        is_low_hp_for_heartbeat(revive_hp_ratio - 0.01),
        "期望比重生血量再低一点（{}）仍被判低血，否则判据被改坏成恒 false、低血心跳整体失效",
        revive_hp_ratio - 0.01,
    );
}

/// 反向锁：修复只动 loop 生命周期，**真受伤的一次性音效照旧**（含重生之后再被打）。
#[test]
fn combat_hit_audio_still_plays_after_revive() {
    let mut app = App::new();
    app.init_resource::<AudioTriggerState>();
    app.init_resource::<AudioImplementationDedup>();
    app.add_event::<CombatEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_event::<StopSoundRecipeRequest>();
    app.add_event::<PlayerRevived>();
    app.add_systems(
        Update,
        (stop_low_hp_heartbeat_on_revive, emit_combat_audio_triggers),
    );
    let attacker = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    let target = app.world_mut().spawn(Position::new([1.0, 64.0, 0.0])).id();

    app.world_mut().send_event(PlayerRevived { entity: target });
    app.world_mut().send_event(CombatEvent {
        attacker,
        target,
        resolved_at_tick: 7,
        body_part: BodyPart::Chest,
        wound_kind: WoundKind::Blunt,
        source: crate::combat::events::AttackSource::Melee,
        debug_command: false,
        physical_damage: 0.0,
        damage: 12.0,
        contam_delta: 0.0,
        description: "post-revive hit".to_string(),
        defense_kind: None,
        defense_effectiveness: None,
        defense_contam_reduced: None,
        defense_wound_severity: None,
    });

    app.update();

    let recipes: Vec<_> = drain_plays(&mut app)
        .into_iter()
        .map(|request| request.recipe_id)
        .collect();
    assert_eq!(
            recipes,
            vec!["hit_heavy", "wound_inflict"],
            "期望重生后被打（胸部 12 伤害）仍发 hit_heavy + wound_inflict（后者含 entity.player.hurt 层）\
             因为修复只收心跳 loop、不该动真受伤反馈；实际 recipes={recipes:?}"
        );
}

#[test]
fn meridian_open_event_emits_chime_recipe() {
    let mut app = App::new();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<MeridianOpenedEvent>();
    app.add_event::<RealmRegressed>();
    app.add_event::<MeridianOverloadEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_cultivation_audio_triggers);
    let player = app.world_mut().spawn_empty().id();
    app.world_mut().send_event(MeridianOpenedEvent {
        entity: player,
        origin: DVec3::new(3.0, 64.0, -2.0),
    });

    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].recipe_id, "meridian_open");
    assert!(matches!(emitted[0].recipient, AudioRecipient::Single(entity) if entity == player));
}

#[test]
fn duo_she_warning_matches_life_record_target() {
    let mut app = App::new();
    app.add_event::<SocialPactEvent>();
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_event::<DuoSheWarningEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_social_audio_triggers);
    let target = app
        .world_mut()
        .spawn((
            Position::new([3.0, 64.0, 3.0]),
            LifeRecord::new("offline:Target"),
        ))
        .id();
    app.world_mut().send_event(DuoSheWarningEvent {
        host_id: "offline:Host".to_string(),
        target_id: "offline:Target".to_string(),
        at_tick: 1,
    });

    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].recipe_id, "exposure_name");
    assert!(matches!(emitted[0].recipient, AudioRecipient::Single(entity) if entity == target));
}

#[test]
fn social_pact_and_renown_emit_audio() {
    let mut app = App::new();
    app.insert_resource(SoundRecipeRegistry::load_default().expect("default recipes load"));
    app.add_event::<SocialPactEvent>();
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_event::<DuoSheWarningEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_social_audio_triggers);
    let target = app
        .world_mut()
        .spawn((
            Position::new([3.0, 64.0, 3.0]),
            LifeRecord::new("offline:Azure"),
        ))
        .id();

    app.world_mut().send_event(SocialPactEvent {
        left: "offline:Azure".to_string(),
        right: "offline:Night".to_string(),
        terms: "teach me the bind".to_string(),
        tick: 1,
        broken: false,
        breaker: None,
        witnesses: vec![],
    });
    app.world_mut().send_event(SocialRenownDeltaEvent {
        char_id: "offline:Azure".to_string(),
        identity_id: None,
        fame_delta: 2,
        notoriety_delta: 0,
        tags_added: vec![],
        tick: 2,
        reason: "test".to_string(),
    });

    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    let recipes: Vec<_> = emitted
        .iter()
        .map(|request| request.recipe_id.as_str())
        .collect();
    assert_eq!(recipes, vec!["pact_bind", "renown_gain"]);
    assert!(matches!(
        emitted[0].recipient,
        AudioRecipient::Radius { origin, .. } if origin == Position::new([3.0, 64.0, 3.0]).get()
    ));
    assert!(matches!(
        emitted[1].recipient,
        AudioRecipient::Single(entity) if entity == target
    ));
}

// ─── plan-sword-path-v2 P4：emit_sword_path_audio_triggers ───

fn sword_path_cast_event(
    skill: SwordPathSkillId,
    caster: Entity,
    center: DVec3,
) -> SwordPathSkillCastEvent {
    SwordPathSkillCastEvent {
        skill,
        caster,
        center,
        direction: None,
        tick: 10,
    }
}

fn setup_sword_path_audio_app() -> App {
    let mut app = App::new();
    app.init_resource::<AudioImplementationDedup>();
    app.add_event::<SwordPathSkillCastEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_sword_path_audio_triggers);
    app
}

/// 五招各 emit 其专属音效配方 + 专属 flag（按招式 dedup）。
#[test]
fn sword_path_skills_emit_dedicated_recipes() {
    let mut app = setup_sword_path_audio_app();
    let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    let center = DVec3::new(0.0, 64.0, 0.0);

    for skill in [
        SwordPathSkillId::CondenseEdge,
        SwordPathSkillId::QiSlash,
        SwordPathSkillId::Resonance,
        SwordPathSkillId::Manifest,
        SwordPathSkillId::HeavenGateCharge,
        SwordPathSkillId::HeavenGateRelease,
    ] {
        app.world_mut()
            .send_event(sword_path_cast_event(skill, caster, center));
    }
    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    let recipes: Vec<_> = emitted.iter().map(|e| e.recipe_id.as_str()).collect();
    assert_eq!(
        recipes,
        vec![
            "sword_condense_edge",
            "sword_qi_slash",
            "sword_resonance",
            "sword_manifest_summon",
            "heaven_gate_charge",
            "sword_manifest_strike",
        ],
        "六个 cast 阶段（含天门 charge/release）各应 emit 其专属配方"
    );
    // flag 也按招式区分，便于 client HUD / dedup
    let flags: Vec<_> = emitted
        .iter()
        .map(|e| e.flag.as_deref().unwrap_or(""))
        .collect();
    assert_eq!(
        flags,
        vec![
            "sword_path_condense_edge",
            "sword_path_qi_slash",
            "sword_path_resonance",
            "sword_path_manifest",
            "sword_path_heaven_gate_charge",
            "sword_path_heaven_gate_release",
        ]
    );
}

/// 所有引用的剑道配方都必须在 server SoundRecipeRegistry 注册（否则路由 fallback 报 warn）。
#[test]
fn all_referenced_sword_path_recipes_exist_in_registry() {
    let registry = SoundRecipeRegistry::load_default().expect("default recipes should load");
    for skill in [
        SwordPathSkillId::CondenseEdge,
        SwordPathSkillId::QiSlash,
        SwordPathSkillId::Resonance,
        SwordPathSkillId::Manifest,
        SwordPathSkillId::HeavenGateCharge,
        SwordPathSkillId::HeavenGateRelease,
    ] {
        let recipe_id = super::sword_path_recipe_for_skill(skill);
        assert!(
            registry.get(recipe_id).is_some(),
            "剑道配方 `{recipe_id}`（招式 {skill:?}）必须在 server registry 注册，\
                 否则 recipient() fallback 到 Single 并 warn——server 与 client 音效资产脱节"
        );
    }
}

/// caster 无 Position → 落到 event.center，仍 emit 音效（不哑）。
#[test]
fn sword_path_audio_falls_back_to_center_without_position() {
    let mut app = setup_sword_path_audio_app();
    // caster 没有 Position component
    let caster = app.world_mut().spawn_empty().id();
    app.world_mut().send_event(sword_path_cast_event(
        SwordPathSkillId::QiSlash,
        caster,
        DVec3::new(7.0, 64.0, 7.0),
    ));
    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert_eq!(emitted.len(), 1, "无 Position 也应出招声（落到 center）");
    assert_eq!(emitted[0].recipe_id, "sword_qi_slash");
    assert_eq!(
        emitted[0].pos,
        Some([7, 64, 7]),
        "无 Position 时音源应落到 event.center"
    );
}

// ─── 崩脉签名：emit_baomai_v3_audio_triggers（运行时消费 emit-path 覆盖）───

fn setup_baomai_audio_app() -> App {
    let mut app = App::new();
    app.init_resource::<AudioImplementationDedup>();
    app.add_event::<BaomaiSkillEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_baomai_v3_audio_triggers);
    app
}

/// 崩脉签名 `full_power_release` 经**真实 emit 系统** emit `baomai_signature`。
/// 「运行时消费」emit-path 门：跑 `emit_baomai_v3_audio_triggers` 读 `BaomaiSkillEvent`
/// → `baomai_recipe_for_skill` → 发 `PlaySoundRecipeRequest`；删掉发声调用 / 改坏 skill→recipe
/// 映射都会撞红（补 `audio::each_signature_skill_*` 静态 pin 之外的 emit 断链覆盖）。
#[test]
fn baomai_full_power_release_emits_signature_recipe() {
    let mut app = setup_baomai_audio_app();
    let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    app.world_mut().send_event(BaomaiSkillEvent {
        skill: BaomaiSkillId::FullPowerRelease,
        caster,
        target: None,
        tick: 1,
        qi_invested: 0.0,
        damage: 0.0,
        radius_blocks: None,
        blood_multiplier: 0.0,
        flow_rate_multiplier: 0.0,
        meridian_dependencies: Vec::new(),
    });
    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    let recipes: Vec<_> = emitted.iter().map(|e| e.recipe_id.as_str()).collect();
    assert_eq!(
        recipes,
        vec!["baomai_signature"],
        "崩脉 full_power_release 应经 emit 系统实发 baomai_signature，实际 {recipes:?}"
    );
}

// ─── 涡流签名：emit_woliu_v2_audio_triggers（运行时消费 emit-path 覆盖）───

fn setup_woliu_audio_app() -> App {
    let mut app = App::new();
    app.init_resource::<AudioImplementationDedup>();
    app.add_event::<VortexCastEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_woliu_v2_audio_triggers);
    app
}

/// 涡流签名 `void_core` 经**真实 emit 系统** emit `woliu_void_core`（经 `event.visual.sound_recipe_id`）。
/// 「运行时消费」emit-path 门：跑 `emit_woliu_v2_audio_triggers` 读 `VortexCastEvent` → 发
/// `PlaySoundRecipeRequest`；删掉发声调用 / visual→recipe 映射漂移都会撞红。
#[test]
fn woliu_void_core_emits_signature_recipe() {
    use crate::combat::woliu_v2::skills::visual_for;
    use crate::combat::woliu_v2::WoliuSkillId;

    let mut app = setup_woliu_audio_app();
    let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    app.world_mut().send_event(VortexCastEvent {
        caster,
        skill: WoliuSkillId::VoidCore,
        tick: 1,
        center: DVec3::new(0.0, 64.0, 0.0),
        lethal_radius: 0.0,
        influence_radius: 0.0,
        turbulence_radius: 0.0,
        absorbed_qi: 0.0,
        swirl_qi: 0.0,
        backfire_level: None,
        visual: visual_for(WoliuSkillId::VoidCore),
    });
    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    let recipes: Vec<_> = emitted.iter().map(|e| e.recipe_id.as_str()).collect();
    assert_eq!(
            recipes,
            vec!["woliu_void_core"],
            "涡流 void_core 应经 emit 系统实发 woliu_void_core（event.visual.sound_recipe_id），实际 {recipes:?}"
        );
}

// ─── 蜕壳签名（被动路径）：emit_tuike_v2_audio_triggers（运行时消费 emit-path 覆盖）───

fn setup_tuike_audio_app() -> App {
    use crate::combat::tuike_v2::events::{
        ContamTransferredEvent, DonFalseSkinEvent, FalseSkinSheddedEvent,
    };
    let mut app = App::new();
    app.init_resource::<AudioImplementationDedup>();
    app.add_event::<DonFalseSkinEvent>();
    app.add_event::<FalseSkinSheddedEvent>();
    app.add_event::<ContamTransferredEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_tuike_v2_audio_triggers);
    app
}

/// tuike.shed **被动蜕壳**（`FalseSkinSheddedEvent`）经真实 emit 系统实发签名 `shed_skin_burst`
/// （经 `event.visual.sound_recipe_id`）。「运行时消费」emit-path 门（Pattern A 侧）；主动施法
/// `cast_shed`（Pattern B 内联在 cast 逻辑）待 P5 重构为 Pattern A 后补。
#[test]
fn tuike_shed_passive_emits_signature_recipe() {
    use crate::combat::tuike_v2::events::{FalseSkinSheddedEvent, TuikeSkillId, TuikeSkillVisual};
    use crate::combat::tuike_v2::FalseSkinTier;

    let mut app = setup_tuike_audio_app();
    let owner = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    app.world_mut().send_event(FalseSkinSheddedEvent {
        owner,
        attacker: None,
        tier: FalseSkinTier::Light,
        damage_absorbed: 0.0,
        damage_overflow: 0.0,
        contam_load: 0.0,
        permanent_taint_load: 0.0,
        layers_after: 0,
        active: true,
        tick: 1,
        visual: TuikeSkillVisual::for_skill(TuikeSkillId::Shed, false).into(),
    });
    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    let recipes: Vec<_> = emitted.iter().map(|e| e.recipe_id.as_str()).collect();
    assert_eq!(
            recipes,
            vec!["shed_skin_burst"],
            "tuike 被动蜕壳应经 emit 系统实发 shed_skin_burst（event.visual.sound_recipe_id），实际 {recipes:?}"
        );
}

// ─── 生产接线门禁：跑真实注册入口，不在测试里另抄系统清单 ───

/// 收集某个 App 的 `Update` 调度里所有系统的 (NodeId, 系统名)。`ScheduleGraph` 在
/// `add_systems` 时即填充，无需初始化/运行，故不受「缺 Events 资源会 panic」影响。
fn update_schedule_systems(app: &App) -> Vec<(bevy_ecs::schedule::NodeId, String)> {
    app.get_schedule(Update)
        .expect("Update 调度应存在")
        .graph()
        .systems()
        .map(|(id, system, _)| (id, system.name().to_string()))
        .collect()
}

/// 定位某个系统函数对应的 `SystemTypeSet` 节点——`.after(f)` / `.before(f)` 建的依赖边
/// 连的是该函数的匿名 type set，不是系统节点本身，故顺序断言要拿它对拍。
fn locate_system_type_set(app: &App, expected: &str) -> bevy_ecs::schedule::NodeId {
    let hits: Vec<_> = app
        .get_schedule(Update)
        .expect("Update 调度应存在")
        .graph()
        .system_sets()
        .filter(|(_, set, _)| format!("{set:?}").contains(expected))
        .map(|(id, _, _)| id)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "`{expected}` 的 SystemTypeSet 应唯一，实际 {} 个",
        hits.len()
    );
    hits[0]
}

/// 在调度里按后缀唯一定位一个系统，返回它的 `NodeId`——**恰好一次**，重复注册也撞红。
fn locate_exactly_once(
    systems: &[(bevy_ecs::schedule::NodeId, String)],
    expected: &str,
) -> bevy_ecs::schedule::NodeId {
    let hits: Vec<_> = systems
        .iter()
        .filter(|(_, name)| name.ends_with(expected))
        .collect();
    assert_eq!(
            hits.len(),
            1,
            "`{expected}` 应在 Update 调度里恰好注册 1 次（0 次 = 运行时永不触发的功能孤岛；\
             ≥2 次 = 重复注册：dedup 逻辑时钟会一帧推进多次、把 2 tick 窗口悄悄改短，emit 也会重发），\
             实际 {} 次",
            hits.len()
        );
    hits[0].0
}

/// **接线门禁（调度侧）**：跑**生产**装配入口，断言三条签名链的 emit 系统
/// ① 恰好注册一次、② `.after(tick_audio_dedup_clock)`、③ `.before(emit_audio_play_payloads)`
/// 三条依赖边都在图里。
///
/// 走的是生产 `network::register_app_wiring`（`network::register` = Redis bootstrap + 它），
/// 所以**连顶层那一行 `audio_trigger::register(app)` 委托被删也会撞红**——不是只调
/// `audio_trigger::register` 自欺欺人（PR #1262 review 要求，已变异验证）。
#[test]
fn production_wiring_registers_audio_trigger_systems_exactly_once_in_order() {
    let mut app = App::new();
    crate::network::register_app_wiring(&mut app);

    let systems = update_schedule_systems(&app);
    // 系统本体各注册一次（重复注册撞红）……
    locate_exactly_once(&systems, "tick_audio_dedup_clock");
    locate_exactly_once(&systems, "audio_event_emit::emit_audio_play_payloads");
    // ……顺序边连的是这两个函数的 SystemTypeSet 节点。
    let dedup_clock = locate_system_type_set(&app, "tick_audio_dedup_clock");
    let payload_sink = locate_system_type_set(&app, "emit_audio_play_payloads");
    let dependency = app
        .get_schedule(Update)
        .expect("Update 调度应存在")
        .graph()
        .dependency()
        .graph();

    for expected in [
        // P5 emit 架构统一的三条签名链
        "emit_zhenmai_v2_audio_triggers",
        "emit_dugu_v2_audio_triggers",
        "emit_tuike_v2_audio_triggers",
        // 既有 Pattern A 家族（同一注册入口，防提取时漏搬）
        "emit_sword_path_audio_triggers",
        "emit_baomai_v3_audio_triggers",
        "emit_woliu_v2_audio_triggers",
        "emit_woliu_v1_vortex_audio_triggers",
        "emit_anqi_audio_triggers",
        "emit_combat_audio_triggers",
        "emit_npc_death_audio_triggers",
        "emit_cultivation_audio_triggers",
        "emit_tribulation_audio_triggers",
        "emit_alchemy_audio_triggers",
        "emit_forge_audio_triggers",
        "emit_botany_audio_triggers",
        "emit_lingtian_audio_triggers",
        "emit_skill_audio_triggers",
        "emit_social_audio_triggers",
        "emit_player_state_audio_triggers",
    ] {
        let node = locate_exactly_once(&systems, expected);
        assert!(
            dependency.contains_edge(dedup_clock, node),
            "`{expected}` 必须 .after(tick_audio_dedup_clock)——少了这条边，emit 会读到上一帧的 \
                 dedup 逻辑 tick，2 tick 去重窗口失准"
        );
        assert!(
            dependency.contains_edge(node, payload_sink),
            "`{expected}` 必须 .before(emit_audio_play_payloads)——少了这条边，本帧发出的 \
                 PlaySoundRecipeRequest 要等下一帧才投递给客户端（本文件 register 的调度契约）"
        );
    }

    // 重生收心跳 loop（#1264）挂的是 **stop** sink，约束与上面那组不同，单独锁一遍：
    // 漏 `.after(emit_player_state_audio_triggers)` 会与低血上沿系统争 `AudioTriggerState`
    // （Bevy ambiguous order）；漏 `.before(emit_audio_stop_payloads)` 则 stop 跨帧才下发。
    let revive_stop = locate_exactly_once(&systems, "stop_low_hp_heartbeat_on_revive");
    let low_hp_state = locate_system_type_set(&app, "emit_player_state_audio_triggers");
    let stop_sink = locate_system_type_set(&app, "emit_audio_stop_payloads");
    assert!(
        dependency.contains_edge(low_hp_state, revive_stop),
        "`stop_low_hp_heartbeat_on_revive` 必须 .after(emit_player_state_audio_triggers)"
    );
    assert!(
        dependency.contains_edge(revive_stop, stop_sink),
        "`stop_low_hp_heartbeat_on_revive` 必须 .before(emit_audio_stop_payloads)"
    );
}

// ─── dedup 状态转换：P5 后三处站点首次套上 AudioImplementationDedup ───

/// 蜕壳签名的 **dedup 碰撞 + 窗口恢复**（PR #1262 review：plan 明确接受这个新状态转换，
/// 那就必须有测试锁住它，否则「接受」是空话）。
///
/// `shed_skin_burst` 被主动施法与维护 / 被动掉壳共用，dedup key = (owner, recipe)、
/// 窗口 `AUDIO_DEDUP_WINDOW_TICKS` = 2 逻辑 tick。三条状态转换逐个断言：
/// ① 同 owner 同帧两条（主动 + 被动）→ 只发一条；② 窗口内（下一帧）再来 → 仍被抑制；
/// ③ 跨过窗口边界 → 恢复发声。外加 ④ 不同 owner 互不抑制。
#[test]
fn shed_signature_dedup_collides_within_window_and_recovers_after() {
    use crate::audio::implementation::AUDIO_DEDUP_WINDOW_TICKS;
    use crate::combat::tuike_v2::events::{FalseSkinSheddedEvent, TuikeSkillId, TuikeSkillVisual};
    use crate::combat::tuike_v2::FalseSkinTier;

    fn shed_event(owner: Entity, active: bool) -> FalseSkinSheddedEvent {
        FalseSkinSheddedEvent {
            owner,
            attacker: None,
            tier: FalseSkinTier::Light,
            damage_absorbed: 0.0,
            damage_overflow: 0.0,
            contam_load: 0.0,
            permanent_taint_load: 0.0,
            layers_after: 0,
            active,
            tick: 1,
            visual: TuikeSkillVisual::for_skill(TuikeSkillId::Shed, false).into(),
        }
    }

    let mut app = App::new();
    app.init_resource::<AudioImplementationDedup>();
    app.add_event::<crate::combat::tuike_v2::DonFalseSkinEvent>();
    app.add_event::<FalseSkinSheddedEvent>();
    app.add_event::<crate::combat::tuike_v2::ContamTransferredEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    // 与生产同序：dedup 逻辑时钟先推进，emit 才读它。
    app.add_systems(
        Update,
        (tick_audio_dedup_clock, emit_tuike_v2_audio_triggers).chain(),
    );
    let owner = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    let other = app.world_mut().spawn(Position::new([40.0, 64.0, 0.0])).id();

    // ① 同 owner 同帧：主动施法 + 维护/被动掉壳各发一条事件 → 只响一次。
    //    ④ 同帧另一个 owner 的蜕壳不受牵连。
    app.world_mut().send_event(shed_event(owner, true));
    app.world_mut().send_event(shed_event(owner, false));
    app.world_mut().send_event(shed_event(other, true));
    app.update();
    let first: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .map(|event| (event.recipient, event.recipe_id))
        .collect();
    assert_eq!(
        first.len(),
        2,
        "同 owner 的两条蜕壳应被 dedup 合成一条、另一 owner 独立发一条（共 2 条），实际 {first:?}"
    );
    let recipients: std::collections::BTreeSet<_> = first
        .iter()
        .map(|(recipient, _)| format!("{recipient:?}"))
        .collect();
    assert_eq!(
            recipients.len(),
            2,
            "这 2 条应分属两个不同 owner（dedup key 含 entity，别人的蜕壳不该被我的抑制），实际 {first:?}"
        );

    // ② 窗口内（下一帧，逻辑 tick 差 1 < 2）再来一条 → 仍被抑制。
    app.world_mut().send_event(shed_event(owner, true));
    app.update();
    let within: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert!(
            within.is_empty(),
            "dedup 窗口内（差 1 tick < {AUDIO_DEDUP_WINDOW_TICKS}）的同 owner 同 recipe 应被抑制，实际 {} 条",
            within.len()
        );

    // ③ 再推一帧跨过窗口边界（差 2 tick）→ 恢复发声。
    app.world_mut().send_event(shed_event(owner, true));
    app.update();
    let recovered: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .map(|event| event.recipe_id)
        .collect();
    assert_eq!(
        recovered,
        vec![crate::combat::tuike_v2::events::SHED_SKIN_BURST_RECIPE.to_string()],
        "跨过 {AUDIO_DEDUP_WINDOW_TICKS} tick 窗口后应恢复发声，实际 {recovered:?}"
    );
}

/// 真脉共用 recipe 的两招（multipoint / harden 都映射 `zhenmai_shield_hum`）同帧连发时，
/// 同样被 dedup 合成一条；而映射到别的 recipe 的招（parry）不受影响。
///
/// 这条也是 P5 新引入的状态转换（旧内联 emit 不过 dedup），plan 里已声明接受。
#[test]
fn zhenmai_shared_recipe_skills_dedup_within_window() {
    use crate::combat::zhenmai_v2::ZhenmaiSkillId;

    assert_eq!(
        ZhenmaiSkillId::MultiPoint.audio_recipe(),
        ZhenmaiSkillId::HardenMeridian.audio_recipe(),
        "本测试的前提是这两招共用 recipe（既有映射）；若哪天各自专属了，本测试应改为断言两条都响"
    );

    let mut app = App::new();
    app.init_resource::<AudioImplementationDedup>();
    app.add_event::<ZhenmaiSkillCastEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(
        Update,
        (tick_audio_dedup_clock, emit_zhenmai_v2_audio_triggers).chain(),
    );
    let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    for skill in [
        ZhenmaiSkillId::MultiPoint,
        ZhenmaiSkillId::HardenMeridian,
        ZhenmaiSkillId::Parry,
    ] {
        app.world_mut().send_event(ZhenmaiSkillCastEvent {
            caster,
            skill,
            cast_center: DVec3::new(0.0, 64.0, 0.0),
        });
    }
    app.update();

    let mut recipes = drain_recipes(&mut app);
    recipes.sort();
    let mut expected = vec![
        ZhenmaiSkillId::MultiPoint.audio_recipe().to_string(),
        ZhenmaiSkillId::Parry.audio_recipe().to_string(),
    ];
    expected.sort();
    assert_eq!(
        recipes, expected,
        "共用 `zhenmai_shield_hum` 的 multipoint / harden 同帧连发应只响一次，parry 另发一条，\
             实际 {recipes:?}"
    );
}

// ─── 真脉五招：emit_zhenmai_v2_audio_triggers（P5 emit 架构统一）───

fn setup_zhenmai_audio_app() -> App {
    let mut app = App::new();
    app.init_resource::<AudioImplementationDedup>();
    // 装**真实** registry：recipient 路由由 recipe 的 attenuation 推出，不插 registry 会退化成
    // `Single(caster)`，路由断言就成了空转。
    app.insert_resource(
        SoundRecipeRegistry::load_default().expect("default audio recipes should load"),
    );
    app.add_event::<ZhenmaiSkillCastEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_zhenmai_v2_audio_triggers);
    app
}

fn drain_recipes(app: &mut App) -> Vec<String> {
    app.world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .map(|event| event.recipe_id)
        .collect()
}

/// 真脉五招逐招经**真实 emit 系统**实发自己的 recipe（期望值调生产映射
/// `ZhenmaiSkillId::audio_recipe` 得到，测试内不另抄表）。
///
/// 「运行时消费」emit-path 门：跑 `emit_zhenmai_v2_audio_triggers` 读 `ZhenmaiSkillCastEvent`
/// → 发 `PlaySoundRecipeRequest`。锁的是「事件被吃掉 / 串到别招的 recipe / 多发漏发」；
/// **映射表本身写错**（如 sever_chain 指向别的 recipe）不由本测试锁——期望值与生产读同一张表，
/// 那条由 `audio::each_signature_skill_actually_emitted_recipe_swaps_l0_to_its_bong_event`
/// 的 registry 内容 pin 覆盖。含签名招 `sever_chain`（`zhenmai_sever_crack`）。
#[test]
fn zhenmai_skills_emit_their_mapped_recipes() {
    use crate::combat::zhenmai_v2::ZhenmaiSkillId;

    for skill in [
        ZhenmaiSkillId::Parry,
        ZhenmaiSkillId::Neutralize,
        ZhenmaiSkillId::MultiPoint,
        ZhenmaiSkillId::HardenMeridian,
        ZhenmaiSkillId::SeverChain,
    ] {
        let mut app = setup_zhenmai_audio_app();
        let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        app.world_mut().send_event(ZhenmaiSkillCastEvent {
            caster,
            skill,
            cast_center: DVec3::new(0.0, 64.0, 0.0),
        });
        app.update();

        let recipes = drain_recipes(&mut app);
        assert_eq!(
                recipes,
                vec![skill.audio_recipe().to_string()],
                "真脉 {skill:?} 应经 emit 系统实发 `{}`（生产映射 ZhenmaiSkillId::audio_recipe），实际 {recipes:?}",
                skill.audio_recipe()
            );
    }
}

/// 真脉音源锁 **cast-time 语义**：音源恒为事件自带的 `center`（施法当时的位置），与重构前
/// 内联 emit 一致——不受「事件跨帧才被读到」「施法后玩家移动 / 传送」影响，也不依赖未声明的
/// ECS 生产者-消费者顺序（PR #1262 review 指出的行为回归，本测试即其回归门）。
///
/// 两条边界一起锁：① 事件发出后 caster 传送到远处，音源仍是施法点；
/// ② caster 根本没有 `Position`（断线 / 未同步）时照样发声、音源仍是事件 center。
#[test]
fn zhenmai_audio_uses_cast_time_center_not_live_position() {
    use crate::combat::zhenmai_v2::ZhenmaiSkillId;

    let mut app = setup_zhenmai_audio_app();
    let moved_caster = app.world_mut().spawn(Position::new([7.5, 64.0, -3.5])).id();
    let positionless_caster = app.world_mut().spawn(()).id();
    app.world_mut().send_event(ZhenmaiSkillCastEvent {
        caster: moved_caster,
        skill: ZhenmaiSkillId::SeverChain,
        cast_center: DVec3::new(7.5, 64.0, -3.5),
    });
    app.world_mut().send_event(ZhenmaiSkillCastEvent {
        caster: positionless_caster,
        skill: ZhenmaiSkillId::Parry,
        cast_center: DVec3::new(-20.5, 70.0, 11.5),
    });
    // 施法之后、音效系统跑之前，caster 传送到很远处（跨帧消费 / 玩家继续跑动的模型）。
    app.world_mut()
        .entity_mut(moved_caster)
        .insert(Position::new([900.0, 12.0, -900.0]));
    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert_eq!(emitted.len(), 2, "两条 cast 事件应各发一条音效");
    assert_eq!(
        emitted[0].recipe_id,
        ZhenmaiSkillId::SeverChain.audio_recipe()
    );
    assert_eq!(
        emitted[0].pos,
        Some([7, 64, -4]),
        "音源必须锁 cast-time center（施法点），不得跟着 caster 移动后的实时 Position 漂移"
    );
    assert_eq!(emitted[1].recipe_id, ZhenmaiSkillId::Parry.audio_recipe());
    assert_eq!(
        emitted[1].pos,
        Some([-21, 70, 11]),
        "caster 无 Position 时同样用 event.cast_center，不静默丢招式声"
    );
    // 路由锁：收听范围由 recipe 声明的 attenuation（真脉五招全是 world_3d）推出 = 64 格广播，
    // 圆心同为 cast-time center。重构前是内联硬编码的 32 格；这条已在 plan 披露为「只放宽收包」，
    // 此处钉住它——若哪天被改成听者锚点或 MELEE 8 格（dugu 踩过的坑）立刻撞红。
    assert_eq!(
        emitted[0].recipient,
        AudioRecipient::Radius {
            origin: DVec3::new(7.5, 64.0, -3.5),
            radius: AUDIO_BROADCAST_RADIUS,
        },
        "真脉音效收听范围应是以 cast-time center 为圆心的 world_3d 64 格广播"
    );
}

// ─── 蛊道倒蚀签名：emit_dugu_v2_audio_triggers（P5 emit 架构统一）───

fn setup_dugu_audio_app() -> App {
    let mut app = App::new();
    app.init_resource::<AudioImplementationDedup>();
    app.add_event::<QiNeedleChargedEvent>();
    app.add_event::<DuguObfuscationDisruptedEvent>();
    app.add_event::<ReverseTriggeredEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_dugu_v2_audio_triggers);
    app
}

fn reverse_event(caster: Entity, center: DVec3) -> ReverseTriggeredEvent {
    use crate::combat::dugu_v2::events::DuguSkillId;
    use crate::combat::dugu_v2::skills::visual_for;

    ReverseTriggeredEvent {
        caster,
        affected_targets: 1,
        burst_damage: 12.0,
        returned_zone_qi: 1.0,
        juebi_delay_ticks: None,
        tick: 1,
        center,
        visual: visual_for(DuguSkillId::Reverse),
    }
}

/// 蛊道签名（倒蚀 `ReverseTriggeredEvent`）经**真实 emit 系统**实发 `dugu_poison_signature`
/// （recipe id 引生产 const `DUGU_POISON_SIGNATURE_RECIPE` 单一真源）。
///
/// 「运行时消费」emit-path 门：原先内联在 `apply_reverse`（Pattern B）无法独立驱动，
/// P5 改为本系统读 cast 事件后可锁——删掉发声调用 / 系统没接线都撞红。
#[test]
fn dugu_reverse_emits_signature_recipe() {
    let mut app = setup_dugu_audio_app();
    let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    app.world_mut()
        .send_event(reverse_event(caster, DVec3::new(3.0, 64.0, 4.0)));
    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    let recipes: Vec<_> = emitted.iter().map(|e| e.recipe_id.as_str()).collect();
    assert_eq!(
        recipes,
        vec![DUGU_POISON_SIGNATURE_RECIPE],
        "蛊道倒蚀应经 emit 系统实发 {DUGU_POISON_SIGNATURE_RECIPE}，实际 {recipes:?}"
    );
    // 路由锁（PR #1262 review）：重构前该站点是「听者位置发声 + 以爆发中心为圆心的 64 格广播」。
    // 若哪天「顺手统一」成 emit_play，recipe 声明的 MELEE 会把收听范围砍到 8 格、再叠上世界锚点
    // 的距离衰减（L0 volume 0.24）→ 实机几乎听不见，本断言即撞红。
    assert_eq!(
        emitted[0].pos, None,
        "倒蚀签名应 pos=None（听者位置、无空间衰减），与重构前内联 emit 一致"
    );
    assert_eq!(
        emitted[0].recipient,
        AudioRecipient::Radius {
            origin: DVec3::new(3.0, 64.0, 4.0),
            radius: AUDIO_BROADCAST_RADIUS,
        },
        "倒蚀签名收听范围应是以爆发中心为圆心的 64 格广播（重构前语义），\
             不得退化成 recipe attenuation 的 MELEE 8 格"
    );
}

/// 蛊道三条 reader（凝针 / 灌毒蛊 / 倒蚀签名）同帧全触发时互不串味：各发各的 recipe，
/// 不多不少三条。
#[test]
fn dugu_three_readers_do_not_cross_talk() {
    let mut app = setup_dugu_audio_app();
    let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    app.world_mut().send_event(QiNeedleChargedEvent {
        shooter: caster,
        target: None,
        tick: 1,
    });
    app.world_mut().send_event(DuguObfuscationDisruptedEvent {
        infuser: caster,
        until_tick: 40,
    });
    app.world_mut()
        .send_event(reverse_event(caster, DVec3::new(0.0, 64.0, 0.0)));
    app.update();

    let mut recipes = drain_recipes(&mut app);
    recipes.sort();
    let mut expected = vec![
        "dugu_cast".to_string(),
        "dugu_poison_cast".to_string(),
        DUGU_POISON_SIGNATURE_RECIPE.to_string(),
    ];
    expected.sort();
    assert_eq!(
        recipes, expected,
        "凝针 → dugu_cast、灌毒蛊 → dugu_poison_cast、倒蚀 → 签名 \
             {DUGU_POISON_SIGNATURE_RECIPE}，三条各一不缺不重（顺序非契约，按 recipe 排序对拍），\
             实际 {recipes:?}"
    );
}

// ─── 暗器六招：emit_anqi_audio_triggers ───────────────────────

fn setup_anqi_audio_app() -> App {
    use crate::combat::anqi_v2::{ArmorPierceEvent, MultiShotEvent, QiInjectionEvent};
    use crate::combat::carrier::CarrierChargedEvent;

    let mut app = App::new();
    app.init_resource::<AudioImplementationDedup>();
    app.add_event::<CarrierChargedEvent>();
    app.add_event::<QiInjectionEvent>();
    app.add_event::<MultiShotEvent>();
    app.add_event::<ArmorPierceEvent>();
    app.add_event::<crate::combat::anqi_v2::DecoyDeployEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_anqi_audio_triggers);
    app
}

fn injection_outcome() -> crate::qi_physics::HighDensityInjectionOutcome {
    crate::qi_physics::HighDensityInjectionOutcome {
        payload_qi: 50.0,
        wound_qi: 40.0,
        contamination_qi: 5.0,
        overload_ratio: 0.5,
        triggers_overload_tear: false,
    }
}

fn armor_outcome() -> crate::qi_physics::ArmorPenetrationOutcome {
    crate::qi_physics::ArmorPenetrationOutcome {
        base_damage: 60.0,
        ignored_defense_ratio: 0.6,
        effective_damage: 70.0,
        carrier_shatter_probability: 0.2,
    }
}

/// 暗器六招各 emit 其专属 recipe（封骨/狙击/齐射/魂注/破甲/分形）。
#[test]
fn anqi_skills_emit_dedicated_recipes() {
    use crate::combat::anqi_v2::{
        AnqiSkillId, ArmorPierceEvent, DecoyDeployEvent, MultiShotEvent, QiInjectionEvent,
    };
    use crate::combat::carrier::{CarrierChargedEvent, CarrierKind};
    use crate::cultivation::components::ColorKind;

    let mut app = setup_anqi_audio_app();
    let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();

    app.world_mut().send_event(CarrierChargedEvent {
        carrier: caster,
        instance_id: 1,
        qi_amount: 25.0,
        qi_color: ColorKind::Solid,
        full_charge: true,
        tick: 10,
    });
    app.world_mut().send_event(QiInjectionEvent {
        caster,
        target: None,
        skill: AnqiSkillId::SingleSnipe,
        carrier_kind: CarrierKind::YibianShougu,
        outcome: injection_outcome(),
        tick: 11,
    });
    app.world_mut().send_event(QiInjectionEvent {
        caster,
        target: None,
        skill: AnqiSkillId::SoulInject,
        carrier_kind: CarrierKind::DyedBone,
        outcome: injection_outcome(),
        tick: 12,
    });
    app.world_mut().send_event(MultiShotEvent {
        caster,
        projectile_count: 5,
        carrier_kind: CarrierKind::LingmuArrow,
        shots: Vec::new(),
        tick: 13,
    });
    app.world_mut().send_event(ArmorPierceEvent {
        caster,
        target: None,
        carrier_kind: CarrierKind::FenglingheBone,
        outcome: armor_outcome(),
        tick: 14,
    });
    app.world_mut().send_event(DecoyDeployEvent {
        caster,
        echo_count: 3,
        tick: 15,
    });
    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    let mut recipes: Vec<_> = emitted.iter().map(|e| e.recipe_id.as_str()).collect();
    recipes.sort_unstable();
    let mut expected = vec![
        "anqi_charge_seal",
        "anqi_single_snipe",
        "anqi_soul_inject",
        "anqi_multi_shot",
        "anqi_armor_pierce",
        "anqi_echo_fractal",
    ];
    expected.sort_unstable();
    assert_eq!(
        recipes, expected,
        "暗器六招各应 emit 其专属 recipe，实际 {recipes:?}"
    );
}

/// QiInjectionEvent 的 MultiShot/ArmorPierce/EchoFractal 分支不发声（走各自 EventReader）。
#[test]
fn anqi_injection_only_handles_snipe_and_soul() {
    use crate::combat::anqi_v2::{AnqiSkillId, QiInjectionEvent};
    use crate::combat::carrier::CarrierKind;

    let mut app = setup_anqi_audio_app();
    let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    // 故意发一个 MultiShot 标签的 QiInjectionEvent（实际生产不会，但守语义边界）
    app.world_mut().send_event(QiInjectionEvent {
        caster,
        target: None,
        skill: AnqiSkillId::MultiShot,
        carrier_kind: CarrierKind::LingmuArrow,
        outcome: injection_outcome(),
        tick: 11,
    });
    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    assert!(
            emitted.is_empty(),
            "QiInjectionEvent 的非 Snipe/Soul 分支不应在 audio 系统发声（走 MultiShot/ArmorPierce 专属 EventReader），实际 {} 条",
            emitted.len()
        );
}

// ========== 绝灵涡流 woliu v1（emit_woliu_v1_vortex_audio_triggers） ==========

fn setup_woliu_v1_audio_app() -> App {
    let mut app = App::new();
    app.add_event::<VortexBackfireEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_woliu_v1_vortex_audio_triggers);
    app
}

fn drain_audio(app: &mut App) -> Vec<PlaySoundRecipeRequest> {
    app.world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect()
}

/// field 出现 → woliu_cast 一次；存续 tick 不重复发声。
#[test]
fn woliu_v1_field_appear_plays_cast_recipe_once() {
    use crate::combat::woliu::VortexField;
    use valence::prelude::DVec3;
    let mut app = setup_woliu_v1_audio_app();
    let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    app.world_mut().entity_mut(caster).insert(VortexField {
        center: DVec3::new(0.0, 64.0, 0.0),
        radius: 8.0,
        delta: 4.0,
        cast_at_tick: 100,
        maintain_max_ticks: 1200,
        caster,
        env_qi_at_cast: 50.0,
        last_maintain_tick: 100,
    });

    app.update();
    let emitted = drain_audio(&mut app);
    assert_eq!(
        emitted.len(),
        1,
        "开涡应发 1 条音效，实际 {} 条",
        emitted.len()
    );
    assert_eq!(
        emitted[0].recipe_id, "woliu_cast",
        "开涡应复用 woliu_cast recipe（零新资产），实际 {}",
        emitted[0].recipe_id
    );

    app.update();
    assert!(
        drain_audio(&mut app).is_empty(),
        "field 存续期间不应重复发开涡音效"
    );
}

/// 反噬 → woliu_burst_pop 爆裂声。
#[test]
fn woliu_v1_backfire_plays_burst_pop() {
    use crate::combat::woliu::{BackfireCause, VortexBackfireEvent};
    let mut app = setup_woliu_v1_audio_app();
    let caster = app.world_mut().spawn(Position::new([2.0, 64.0, 2.0])).id();
    app.world_mut().send_event(VortexBackfireEvent {
        caster,
        cause: BackfireCause::EnvQiTooLow,
        meridian_severed: crate::cultivation::components::MeridianId::Lung,
        tick: 300,
        env_qi: 1.0,
        delta: 4.0,
        resisted: false,
    });
    app.update();
    let emitted = drain_audio(&mut app);
    assert_eq!(
        emitted.len(),
        1,
        "反噬应发 1 条爆裂声，实际 {} 条",
        emitted.len()
    );
    assert_eq!(
        emitted[0].recipe_id, "woliu_burst_pop",
        "反噬应发爆裂声 recipe，实际 {}",
        emitted[0].recipe_id
    );
}

/// 反噬 caster 断 Position 但领域仍在 → 回落 field.center 仍发声（重要负反馈不静默丢）。
#[test]
fn woliu_v1_backfire_falls_back_to_field_center_when_caster_positionless() {
    use crate::combat::woliu::{BackfireCause, VortexBackfireEvent, VortexField};
    use valence::prelude::DVec3;
    let mut app = setup_woliu_v1_audio_app();
    let caster = app.world_mut().spawn_empty().id();
    app.world_mut().entity_mut(caster).insert(VortexField {
        center: DVec3::new(4.0, 64.0, -2.0),
        radius: 8.0,
        delta: 4.0,
        cast_at_tick: 100,
        maintain_max_ticks: 1200,
        caster,
        env_qi_at_cast: 50.0,
        last_maintain_tick: 100,
    });
    app.update();
    drain_audio(&mut app); // 吃掉开涡音效

    app.world_mut().send_event(VortexBackfireEvent {
        caster,
        cause: BackfireCause::ExceedMaintainMax,
        meridian_severed: crate::cultivation::components::MeridianId::Lung,
        tick: 120,
        env_qi: 10.0,
        delta: 4.0,
        resisted: false,
    });
    app.update();
    let emitted = drain_audio(&mut app);
    assert_eq!(
        emitted.len(),
        1,
        "caster 无 Position 但领域仍在时，反噬音效应回落 field.center 发出，实际 {} 条",
        emitted.len()
    );
    assert_eq!(
        emitted[0].recipe_id, "woliu_burst_pop",
        "回落路径也必须是爆裂声 recipe，实际 {}",
        emitted[0].recipe_id
    );
}

// ── plan-gathering-tool-bind-v1 P1（PR #1293 review 修正）：草镰专属 SFX 必须严格限定
// required_tool_kind == CaoLian，不能对任意 required_tool 草本一律播放 ──

fn botany_harvest_terminal_event(
    client_entity: Entity,
    bare_hand_wound: bool,
    required_tool_used: bool,
    required_tool_kind: Option<ToolKind>,
) -> HarvestTerminalEvent {
    HarvestTerminalEvent {
        client_entity,
        session_id: "offline:Azure".to_string(),
        target_id: "plant-1".to_string(),
        target_name: "test_plant".to_string(),
        plant_kind: "test_plant".to_string(),
        mode: crate::botany::components::BotanyHarvestMode::Manual,
        interrupted: false,
        completed: true,
        detail: "采得 1 株".to_string(),
        target_pos: Some([10.0, 64.0, 10.0]),
        spirit_quality: 0.9,
        duration_ticks: 40,
        gathering_quality: None,
        tool_used: None,
        overflow_to_ground: false,
        bare_hand_wound,
        required_tool_used,
        required_tool_kind,
    }
}

fn botany_audio_test_app() -> App {
    let mut app = App::new();
    app.add_event::<HarvestTerminalEvent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, emit_botany_audio_triggers);
    app
}

#[test]
fn cao_lian_harvest_swing_emits_after_cao_lian_tool_used() {
    let mut app = botany_audio_test_app();
    let player = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    app.world_mut().send_event(botany_harvest_terminal_event(
        player,
        false,
        true,
        Some(ToolKind::CaoLian),
    ));
    app.update();

    let recipes: Vec<_> = drain_audio(&mut app)
        .into_iter()
        .map(|e| e.recipe_id)
        .collect();
    assert_eq!(
        recipes,
        vec!["harvest_pluck", "cao_lian_harvest_swing"],
        "持草镰完成采集应在基础采集声之后追加草镰挥砍声"
    );
}

#[test]
fn botany_bare_hand_wound_emits_after_cao_lian_bare_hand_hit() {
    let mut app = botany_audio_test_app();
    let player = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    app.world_mut().send_event(botany_harvest_terminal_event(
        player,
        true,
        false,
        Some(ToolKind::CaoLian),
    ));
    app.update();

    let recipes: Vec<_> = drain_audio(&mut app)
        .into_iter()
        .map(|e| e.recipe_id)
        .collect();
    assert_eq!(
        recipes,
        vec!["harvest_pluck", "botany_bare_hand_wound"],
        "草镰目标草本徒手割手应在基础采集声之后追加割手痛呼声"
    );
}

/// 回归：既有 DunQiJia 门槛草本（`XuanGenWei` 等）持工具完成采集，不得播放
/// 草镰专属的 `cao_lian_harvest_swing`——required_tool_used 只是"任意 required_tool
/// 命中"的通用信号，必须靠 required_tool_kind 甄别。
#[test]
fn non_cao_lian_required_tool_harvest_does_not_emit_cao_lian_swing() {
    let mut app = botany_audio_test_app();
    let player = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    app.world_mut().send_event(botany_harvest_terminal_event(
        player,
        false,
        true,
        Some(ToolKind::DunQiJia),
    ));
    app.update();

    let recipes: Vec<_> = drain_audio(&mut app)
        .into_iter()
        .map(|e| e.recipe_id)
        .collect();
    assert_eq!(
        recipes,
        vec!["harvest_pluck"],
        "既有 DunQiJia 门槛草本不属于本 plan 范围，不应播放草镰专属挥砍声"
    );
}

/// 回归：既有 DunQiJia 门槛草本徒手割手，不得播放草镰专属的 `botany_bare_hand_wound`。
#[test]
fn non_cao_lian_bare_hand_wound_does_not_emit_botany_bare_hand_wound_recipe() {
    let mut app = botany_audio_test_app();
    let player = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
    app.world_mut().send_event(botany_harvest_terminal_event(
        player,
        true,
        false,
        Some(ToolKind::DunQiJia),
    ));
    app.update();

    let recipes: Vec<_> = drain_audio(&mut app)
        .into_iter()
        .map(|e| e.recipe_id)
        .collect();
    assert_eq!(
        recipes,
        vec!["harvest_pluck"],
        "既有 DunQiJia 门槛草本的徒手割手不属于本 plan 范围，不应播放草镰专属割手声"
    );
}
