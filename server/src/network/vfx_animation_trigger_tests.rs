use super::*;
use uuid::Uuid;
use valence::prelude::{App, Events, Update};
use valence::testing::create_mock_client;

use crate::combat::components::BodyPart;
use crate::combat::events::AttackReach;
use crate::cultivation::breakthrough::{BreakthroughError, BreakthroughSuccess};
use crate::cultivation::components::Realm;

fn spawn_player(app: &mut App, name: &str, pos: [f64; 3]) -> valence::prelude::Entity {
    let (mut bundle, _helper) = create_mock_client(name);
    bundle.player.position = Position::new(pos);
    app.world_mut().spawn(bundle).id()
}

fn spawn_skinned_npc_target(app: &mut App, name: &str, pos: [f64; 3]) -> valence::prelude::Entity {
    app.world_mut()
        .spawn((
            Position::new(pos),
            UniqueId(Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes())),
        ))
        .id()
}

fn drain_vfx(app: &mut App) -> Vec<VfxEventRequest> {
    app.world_mut()
        .resource_mut::<Events<VfxEventRequest>>()
        .drain()
        .collect()
}

#[test]
fn melee_cut_attack_emits_sword_swing_for_attacker() {
    let mut app = App::new();
    app.insert_resource(CombatClock::default());
    app.add_event::<AttackIntent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_attack_animation_triggers);
    let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: None,
        issued_at_tick: 1,
        reach: AttackReach::new(1.0, 0.0),
        qi_invest: 1.0,
        wound_kind: WoundKind::Cut,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 1);
    assert_play_anim(&emitted[0], ANIM_SWORD_SLASH_DOWN, COMBAT_PRIORITY);
}

#[test]
fn sword_cleave_attack_source_emits_sword_cleave_animation() {
    let mut app = App::new();
    app.insert_resource(CombatClock::default());
    app.add_event::<AttackIntent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_attack_animation_triggers);
    let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: None,
        issued_at_tick: 1,
        reach: AttackReach::new(3.0, 0.0),
        qi_invest: 0.0,
        wound_kind: WoundKind::Cut,
        source: AttackSource::SwordCleave,
        debug_command: None,
    });

    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 1);
    assert_play_anim(&emitted[0], ANIM_SWORD_CLEAVE, COMBAT_PRIORITY);
}

#[test]
fn burst_meridian_attack_intent_does_not_duplicate_beng_quan_animation() {
    let mut app = App::new();
    app.insert_resource(CombatClock::default());
    app.add_event::<AttackIntent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_attack_animation_triggers);
    let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: None,
        issued_at_tick: 1,
        reach: AttackReach::new(1.0, 0.0),
        qi_invest: 1.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::BurstMeridian,
        debug_command: None,
    });

    app.update();

    assert!(drain_vfx(&mut app).is_empty());
}

#[test]
fn skinned_npc_with_unique_id_can_receive_action_animation() {
    let mut app = App::new();
    app.insert_resource(CombatClock::default());
    app.add_event::<AttackIntent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_attack_animation_triggers);
    let attacker = spawn_skinned_npc_target(&mut app, "npc:rogue-1", [0.0, 64.0, 0.0]);

    app.world_mut().send_event(AttackIntent {
        attacker,
        target: None,
        issued_at_tick: 1,
        reach: AttackReach::new(1.0, 0.0),
        qi_invest: 1.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });

    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 1);
    assert_play_anim(&emitted[0], ANIM_FIST_PUNCH_RIGHT, COMBAT_PRIORITY);
}

#[test]
fn combat_hit_emits_recoil_for_unique_id_target() {
    let mut app = App::new();
    app.add_event::<CombatEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_hit_recoil_animation_triggers);
    let attacker = app.world_mut().spawn_empty().id();
    let target = spawn_player(&mut app, "Bob", [1.0, 64.0, 0.0]);

    app.world_mut().send_event(CombatEvent {
        attacker,
        target,
        resolved_at_tick: 1,
        body_part: BodyPart::Chest,
        wound_kind: WoundKind::Blunt,
        source: crate::combat::events::AttackSource::Melee,
        debug_command: false,
        physical_damage: 0.0,
        damage: 0.25,
        contam_delta: 0.0,
        description: "hit".to_string(),
        defense_kind: None,
        defense_effectiveness: None,
        defense_contam_reduced: None,
        defense_wound_severity: None,
    });

    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 1);
    assert_play_anim(&emitted[0], ANIM_HURT_STAGGER, HIT_RECOIL_PRIORITY);
}

#[test]
fn physical_combat_hit_emits_recoil_for_unique_id_target() {
    let mut app = App::new();
    app.add_event::<CombatEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_hit_recoil_animation_triggers);
    let attacker = app.world_mut().spawn_empty().id();
    let target = spawn_player(&mut app, "Bob", [1.0, 64.0, 0.0]);

    app.world_mut().send_event(CombatEvent {
        attacker,
        target,
        resolved_at_tick: 1,
        body_part: BodyPart::Chest,
        wound_kind: WoundKind::Cut,
        source: crate::combat::events::AttackSource::SwordCleave,
        debug_command: false,
        physical_damage: 1.0,
        damage: 0.0,
        contam_delta: 0.0,
        description: "physical hit".to_string(),
        defense_kind: None,
        defense_effectiveness: None,
        defense_contam_reduced: None,
        defense_wound_severity: None,
    });

    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 1);
    assert_play_anim(&emitted[0], ANIM_HURT_STAGGER, HIT_RECOIL_PRIORITY);
}

#[test]
fn breakthrough_success_emits_story_animation() {
    let mut app = App::new();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_breakthrough_animation_triggers);
    let player = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

    app.world_mut().send_event(BreakthroughOutcome {
        entity: player,
        from: Realm::Awaken,
        result: Ok(BreakthroughSuccess {
            to: Realm::Induce,
            success_rate: 1.0,
            used_qi: 8.0,
        }),
    });

    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 1);
    assert_play_anim(&emitted[0], ANIM_BREAKTHROUGH_YINQI, STORY_PRIORITY);
}

#[test]
fn breakthrough_failure_does_not_play_success_animation() {
    let mut app = App::new();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_breakthrough_animation_triggers);
    let player = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

    app.world_mut().send_event(BreakthroughOutcome {
        entity: player,
        from: Realm::Awaken,
        result: Err(BreakthroughError::RolledFailure { severity: 0.4 }),
    });

    app.update();

    assert!(drain_vfx(&mut app).is_empty());
}

#[test]
fn woliu_vortex_resonance_visual_uses_field_scale_particles() {
    let mut app = App::new();
    app.add_event::<VortexCastEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_woliu_v2_visual_triggers);
    let caster = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

    app.world_mut().send_event(VortexCastEvent {
        caster,
        skill: WoliuSkillId::VortexResonance,
        tick: 10,
        center: valence::prelude::DVec3::new(1.0, 64.0, 2.0),
        lethal_radius: 0.0,
        influence_radius: 6.0,
        turbulence_radius: 6.0,
        absorbed_qi: 0.0,
        swirl_qi: 10.0,
        backfire_level: None,
        visual: crate::combat::woliu_v2::events::WoliuSkillVisual {
            animation_id: "bong:woliu_vortex_resonance",
            particle_id: "bong:woliu_vortex_resonance_field",
            sound_recipe_id: "woliu_vortex_resonance",
            hud_hint: "vortex_resonance",
            icon_texture: "bong:textures/gui/skill/woliu_heart.png",
        },
    });

    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 2);
    assert_play_anim(&emitted[0], "bong:woliu_vortex_resonance", WOLIU_PRIORITY);
    match &emitted[1].payload {
        VfxEventPayloadV1::SpawnParticle {
            event_id,
            strength,
            count,
            duration_ticks,
            ..
        } => {
            assert_eq!(event_id, "bong:woliu_vortex_resonance_field");
            assert_eq!(*strength, Some(1.0));
            assert_eq!(*count, Some(48));
            assert_eq!(*duration_ticks, Some(80));
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

#[test]
fn woliu_looping_visual_stops_when_active_window_expires() {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 10 });
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_woliu_v2_visual_stop_triggers);
    let caster = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    app.world_mut().entity_mut(caster).insert(VortexV2State {
        active_skill_kind: WoliuSkillId::VortexResonance,
        heart_passive_enabled: false,
        lethal_radius: 0.0,
        influence_radius: 6.0,
        turbulence_radius: 6.0,
        turbulence_intensity: 0.8,
        backfire_level: None,
        started_at_tick: 10,
        active_until_tick: 20,
        cooldown_until_tick: 100,
    });

    app.update();
    assert!(drain_vfx(&mut app).is_empty());

    app.world_mut().resource_mut::<CombatClock>().tick = 20;
    app.update();
    let emitted = drain_vfx(&mut app);

    assert_eq!(emitted.len(), 1);
    assert_stop_anim(
        &emitted[0],
        "bong:woliu_vortex_resonance",
        WOLIU_STOP_FADE_OUT_TICKS,
    );

    app.world_mut().resource_mut::<CombatClock>().tick = 21;
    app.update();
    assert!(
        drain_vfx(&mut app).is_empty(),
        "expected no second StopAnim because woliu lifecycle is already inactive"
    );
}

#[test]
fn woliu_stop_animation_uses_same_animation_id_as_skill_visual() {
    for skill in [
        WoliuSkillId::Hold,
        WoliuSkillId::Burst,
        WoliuSkillId::Mouth,
        WoliuSkillId::Pull,
        WoliuSkillId::Heart,
        WoliuSkillId::VacuumPalm,
        WoliuSkillId::VortexShield,
        WoliuSkillId::VacuumLock,
        WoliuSkillId::VortexResonance,
        WoliuSkillId::TurbulenceBurst,
        // plan-woliu-path-v1：虚蚀路径 5 招式
        WoliuSkillId::AmbientVortex,
        WoliuSkillId::VoidVortex,
        WoliuSkillId::SwallowingVortex,
        WoliuSkillId::VortexEcho,
        WoliuSkillId::VoidCore,
    ] {
        assert_eq!(
            woliu_anim_for_skill(skill),
            crate::combat::woliu_v2::skills::visual_for(skill).animation_id,
            "expected stop animation to match play animation for {skill:?}"
        );
    }
}

#[test]
fn completed_botany_harvest_emits_leaf_burst_particle() {
    let mut app = App::new();
    app.add_event::<HarvestTerminalEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_botany_harvest_visual_triggers);
    let player = app.world_mut().spawn_empty().id();

    app.world_mut().send_event(HarvestTerminalEvent {
        client_entity: player,
        session_id: "offline:Azure".to_string(),
        target_id: "plant-1".to_string(),
        target_name: "ci_she_hao".to_string(),
        plant_kind: "ci_she_hao".to_string(),
        mode: crate::botany::components::BotanyHarvestMode::Manual,
        interrupted: false,
        completed: true,
        detail: "done".to_string(),
        target_pos: Some([10.0, 64.0, 10.0]),
        spirit_quality: 0.95,
        duration_ticks: 40,
        gathering_quality: Some(crate::gathering::quality::GatheringQuality::Perfect),
        tool_used: Some("bao_chu".to_string()),
        overflow_to_ground: false,
        bare_hand_wound: false,
        required_tool_used: false,
        required_tool_kind: None,
    });

    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 1);
    assert_spawn_particle(&emitted[0], BOTANY_HARVEST_VFX, Some(12));
    match &emitted[0].payload {
        VfxEventPayloadV1::SpawnParticle {
            color, strength, ..
        } => {
            assert_eq!(color.as_deref(), Some("#FFDD22"));
            assert_eq!(*strength, Some(0.95));
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

// ── plan-gathering-tool-bind-v1 P1（PR #1293 review 修正）：草镰专属粒子必须严格限定
// required_tool_kind == CaoLian，并验证"沿挥镰弧线"方向确实被填充 ──

fn cao_lian_harvest_terminal_event(
    client_entity: valence::prelude::Entity,
    bare_hand_wound: bool,
    required_tool_used: bool,
    required_tool_kind: Option<ToolKind>,
) -> HarvestTerminalEvent {
    HarvestTerminalEvent {
        client_entity,
        session_id: "offline:Azure".to_string(),
        target_id: "plant-1".to_string(),
        target_name: "xue_se_mai_cao".to_string(),
        plant_kind: "xue_se_mai_cao".to_string(),
        mode: crate::botany::components::BotanyHarvestMode::Manual,
        interrupted: false,
        completed: true,
        detail: "done".to_string(),
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

#[test]
fn cao_lian_harvest_swing_burst_carries_player_to_target_direction() {
    let mut app = App::new();
    app.add_event::<HarvestTerminalEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_botany_harvest_visual_triggers);
    // 玩家在 [7,64,10]，目标植物在 target_pos=[10,64,10] —— 挥砍方向应为 +X（水平）。
    let player = spawn_player(&mut app, "Azure", [7.0, 64.0, 10.0]);

    app.world_mut().send_event(cao_lian_harvest_terminal_event(
        player,
        false,
        true,
        Some(ToolKind::CaoLian),
    ));
    app.update();

    let emitted = drain_vfx(&mut app);
    // spawn_player 带完整动画目标组件，emit_play_for_entity 的采割蹲姿动画也会命中
    // （不同于其它测试用的 spawn_empty），所以是 蹲姿动画 + 基础采集 burst + 草镰
    // 专属挥砍 burst 共 3 条；只关心序列里最后一条 SpawnParticle。
    let last = emitted
        .last()
        .expect("持草镰完成采集应至少发出草镰专属挥砍 burst");
    match &last.payload {
        VfxEventPayloadV1::SpawnParticle {
            color,
            count,
            direction,
            ..
        } => {
            assert_eq!(color.as_deref(), Some(CAO_LIAN_SWING_COLOR));
            assert_eq!(*count, Some(CAO_LIAN_SWING_COUNT));
            let dir =
                direction.expect("草镰挥砍粒子必须携带方向以表达 plan P1 视听规格的'沿挥镰弧线'");
            assert!(
                dir[0] > 0.0 && dir[1] == 0.0,
                "方向应为玩家→目标的水平向量（此处应指向 +X），实际 {dir:?}"
            );
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

#[test]
fn cao_lian_bare_hand_wound_emits_red_streak_without_direction() {
    let mut app = App::new();
    app.add_event::<HarvestTerminalEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_botany_harvest_visual_triggers);
    let player = spawn_player(&mut app, "Azure", [7.0, 64.0, 10.0]);

    app.world_mut().send_event(cao_lian_harvest_terminal_event(
        player,
        true,
        false,
        Some(ToolKind::CaoLian),
    ));
    app.update();

    let emitted = drain_vfx(&mut app);
    let last = emitted
        .last()
        .expect("草镰目标草本徒手割手应至少发出红色细痕 burst");
    match &last.payload {
        VfxEventPayloadV1::SpawnParticle { color, count, .. } => {
            assert_eq!(color.as_deref(), Some(BARE_HAND_WOUND_COLOR));
            assert_eq!(*count, Some(BARE_HAND_WOUND_COUNT));
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

/// 回归：既有 DunQiJia 门槛草本持工具完成采集，不得播放草镰专属绿色挥砍粒子——
/// required_tool_used 只是"任意 required_tool 命中"的通用信号。
#[test]
fn non_cao_lian_required_tool_harvest_does_not_emit_cao_lian_burst() {
    let mut app = App::new();
    app.add_event::<HarvestTerminalEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_botany_harvest_visual_triggers);
    let player = spawn_player(&mut app, "Azure", [7.0, 64.0, 10.0]);

    app.world_mut().send_event(cao_lian_harvest_terminal_event(
        player,
        false,
        true,
        Some(ToolKind::DunQiJia),
    ));
    app.update();

    let emitted = drain_vfx(&mut app);
    // 蹲姿动画 + 基础采集 burst 共 2 条；不应再追加草镰专属绿色挥砍 burst。
    assert_eq!(
        emitted.len(),
        2,
        "既有 DunQiJia 门槛草本不属于本 plan 范围，不应追加草镰专属挥砍 burst，实际 {} 条",
        emitted.len()
    );
    let last = emitted.last().unwrap();
    match &last.payload {
        VfxEventPayloadV1::SpawnParticle { color, .. } => {
            assert_ne!(
                color.as_deref(),
                Some(CAO_LIAN_SWING_COLOR),
                "最后一条粒子不应是草镰专属挥砍色"
            );
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

/// 回归：既有 DunQiJia 门槛草本徒手割手，不得播放草镰专属红色细痕粒子。
#[test]
fn non_cao_lian_bare_hand_wound_does_not_emit_red_streak() {
    let mut app = App::new();
    app.add_event::<HarvestTerminalEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_botany_harvest_visual_triggers);
    let player = spawn_player(&mut app, "Azure", [7.0, 64.0, 10.0]);

    app.world_mut().send_event(cao_lian_harvest_terminal_event(
        player,
        true,
        false,
        Some(ToolKind::DunQiJia),
    ));
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        2,
        "既有 DunQiJia 门槛草本的徒手割手不属于本 plan 范围，不应追加草镰专属红色细痕 burst，实际 {} 条",
        emitted.len()
    );
    let last = emitted.last().unwrap();
    match &last.payload {
        VfxEventPayloadV1::SpawnParticle { color, .. } => {
            assert_ne!(
                color.as_deref(),
                Some(BARE_HAND_WOUND_COLOR),
                "最后一条粒子不应是草镰专属割手红色"
            );
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

fn add_guangbo_visual_test_system(app: &mut App) {
    app.add_event::<GuangboTicaoPracticeEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_guangbo_ticao_visual_triggers);
}

#[test]
fn guangbo_practice_emits_stretch_anim_and_happy_particle_for_skinned_player() {
    let mut app = App::new();
    add_guangbo_visual_test_system(&mut app);
    let player = spawn_skinned_npc_target(&mut app, "Stretcher", [3.0, 64.0, 9.0]);

    app.world_mut()
        .send_event(GuangboTicaoPracticeEvent { entity: player });
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        2,
        "skinned 练习者应收 PlayAnim + SpawnParticle 两条 VFX，实际 {} 条",
        emitted.len()
    );
    assert_play_anim(&emitted[0], ANIM_GUANGBO_TICAO, GUANGBO_TICAO_PRIORITY);
    assert_spawn_particle(&emitted[1], VFX_GUANGBO_TICAO_PRACTICE, Some(6));
    // 粒子在头部上方（origin.y + 1.2），颜色为温和绿。
    match &emitted[1].payload {
        VfxEventPayloadV1::SpawnParticle { origin, color, .. } => {
            assert_eq!(color.as_deref(), Some("#7FE38F"));
            assert!(
                (origin[1] - (64.0 + 1.2)).abs() < 1e-6,
                "练习粒子应在头部上方 y+1.2，实际 y={}",
                origin[1]
            );
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

#[test]
fn guangbo_ticao_uses_dedicated_anim_not_guard_raise() {
    // #4 各招专属动画：广播体操必须用专属完整套路 bong:guangbo_ticao，
    // 不得回退到此前复用的 bong:guard_raise（4tick 举臂格挡，真机"只动一下"）。
    // 防一次性修复被后续 refactor 静默还原。
    assert_eq!(
        ANIM_GUANGBO_TICAO, "bong:guangbo_ticao",
        "广播体操应发专属套路 anim id 'bong:guangbo_ticao'，实际 '{ANIM_GUANGBO_TICAO}'"
    );
    assert_ne!(
        ANIM_GUANGBO_TICAO, ANIM_GUARD_RAISE,
        "广播体操不得复用 guard_raise（举臂格挡）——那是'只动一下'的根因"
    );
}

#[test]
fn guangbo_practice_without_position_emits_nothing() {
    // caster 无 Position/UniqueId（断线）→ 动画 skip + 粒子 skip（粒子也依赖 Position）。
    let mut app = App::new();
    add_guangbo_visual_test_system(&mut app);
    let player = app.world_mut().spawn_empty().id();

    app.world_mut()
        .send_event(GuangboTicaoPracticeEvent { entity: player });
    app.update();

    let emitted = drain_vfx(&mut app);
    assert!(
        emitted.is_empty(),
        "无 Position/UniqueId 的练习者不应产生任何 VFX，实际 {} 条",
        emitted.len()
    );
}

#[test]
fn lingtian_completion_events_emit_plot_rune_particles() {
    let mut app = App::new();
    app.add_event::<TillCompleted>();
    app.add_event::<PlantingCompleted>();
    app.add_event::<HarvestCompleted>();
    app.add_event::<ReplenishCompleted>();
    app.add_event::<DrainQiCompleted>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_lingtian_visual_triggers);
    let player = app.world_mut().spawn_empty().id();
    let pos = valence::prelude::BlockPos::new(2, 65, 7);

    app.world_mut().send_event(TillCompleted {
        player,
        pos,
        hoe: crate::lingtian::hoe::HoeKind::Iron,
        hoe_instance_id: 1,
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
        plot_qi_added: 0.25,
        overflow_to_zone: 0.0,
    });
    app.world_mut().send_event(DrainQiCompleted {
        player,
        pos,
        plot_qi_drained: 0.5,
        qi_to_player: 0.4,
        qi_to_zone: 0.1,
    });

    app.update();

    let ids: Vec<_> = drain_vfx(&mut app)
        .into_iter()
        .map(|req| match req.payload {
            VfxEventPayloadV1::SpawnParticle { event_id, .. } => event_id,
            other => panic!("expected SpawnParticle, got {other:?}"),
        })
        .collect();
    assert_eq!(
        ids,
        vec![
            LINGTIAN_TILL_VFX,
            LINGTIAN_PLANT_VFX,
            LINGTIAN_HARVEST_VFX,
            LINGTIAN_REPLENISH_VFX,
            LINGTIAN_DRAIN_VFX,
        ]
    );
}

#[test]
fn tuike_v2_skill_events_emit_animation_and_particle_pairs() {
    let mut app = App::new();
    add_tuike_v2_visual_test_system(&mut app);
    let player = spawn_player(&mut app, "Azure", [1.0, 64.0, 2.0]);

    app.world_mut().send_event(DonFalseSkinEvent {
        caster: player,
        tier: crate::combat::tuike_v2::FalseSkinTier::Light,
        layers_after: 1,
        tick: 10,
        visual: tuike_visual(crate::combat::tuike_v2::TuikeSkillId::Don, false),
    });
    app.world_mut().send_event(FalseSkinSheddedEvent {
        owner: player,
        attacker: None,
        tier: crate::combat::tuike_v2::FalseSkinTier::Mid,
        damage_absorbed: 20.0,
        damage_overflow: 0.0,
        contam_load: 0.0,
        permanent_taint_load: 0.0,
        layers_after: 0,
        active: true,
        tick: 11,
        visual: tuike_visual(crate::combat::tuike_v2::TuikeSkillId::Shed, false),
    });
    app.world_mut().send_event(ContamTransferredEvent {
        caster: player,
        tier: crate::combat::tuike_v2::FalseSkinTier::Ancient,
        contam_moved_percent: 15.0,
        backflow_percent: 0.0,
        permanent_absorbed: 0.0,
        qi_cost: 105.0,
        tick: 12,
        visual: tuike_visual(crate::combat::tuike_v2::TuikeSkillId::TransferTaint, false),
    });

    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 6);
    assert_tuike_pair(
        &emitted[0..2],
        "bong:tuike_don_skin",
        "bong:false_skin_don_dust",
        Some(10),
    );
    assert_tuike_pair(
        &emitted[2..4],
        "bong:tuike_shed_burst",
        "bong:false_skin_shed_burst",
        Some(18),
    );
    assert_tuike_pair(
        &emitted[4..6],
        "bong:tuike_taint_transfer",
        "bong:false_skin_don_dust",
        Some(12),
    );
}

#[test]
fn tuike_v2_visual_trigger_ignores_non_player_entities() {
    let mut app = App::new();
    add_tuike_v2_visual_test_system(&mut app);
    let non_player = app.world_mut().spawn_empty().id();

    app.world_mut().send_event(DonFalseSkinEvent {
        caster: non_player,
        tier: crate::combat::tuike_v2::FalseSkinTier::Light,
        layers_after: 1,
        tick: 10,
        visual: tuike_visual(crate::combat::tuike_v2::TuikeSkillId::Don, false),
    });

    app.update();

    assert!(drain_vfx(&mut app).is_empty());
}

#[test]
fn tuike_v2_visual_trigger_colors_permanent_shed_and_transfer_branches() {
    let mut app = App::new();
    add_tuike_v2_visual_test_system(&mut app);
    let player = spawn_player(&mut app, "Azure", [1.0, 64.0, 2.0]);

    app.world_mut().send_event(FalseSkinSheddedEvent {
        owner: player,
        attacker: None,
        tier: crate::combat::tuike_v2::FalseSkinTier::Ancient,
        damage_absorbed: 20.0,
        damage_overflow: 0.0,
        contam_load: 0.0,
        permanent_taint_load: 0.5,
        layers_after: 0,
        active: true,
        tick: 11,
        visual: tuike_visual(crate::combat::tuike_v2::TuikeSkillId::Shed, true),
    });
    app.world_mut().send_event(ContamTransferredEvent {
        caster: player,
        tier: crate::combat::tuike_v2::FalseSkinTier::Ancient,
        contam_moved_percent: 15.0,
        backflow_percent: 0.0,
        permanent_absorbed: 0.5,
        qi_cost: 105.0,
        tick: 12,
        visual: tuike_visual(crate::combat::tuike_v2::TuikeSkillId::TransferTaint, true),
    });

    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 4);
    assert_spawn_particle_color(&emitted[1], "#BFD8FF");
    assert_spawn_particle_color(&emitted[3], "#9EC7FF");
}

/// 取 PlayAnim 的 anim_id（负向断言「不再借旧动画」回归锁用）。
fn play_anim_id(request: &VfxEventRequest) -> String {
    match &request.payload {
        VfxEventPayloadV1::PlayAnim { anim_id, .. } => anim_id.clone(),
        other => panic!("expected PlayAnim, got {other:?}"),
    }
}

/// 取 PlayAnim 的 (priority, fade_in_ticks)——P6 相位承接契约
/// （docs/player-animation-conventions.md §14.2）的 pin 用。
fn play_anim_priority_and_fade_in(request: &VfxEventRequest) -> (u16, Option<u8>) {
    match &request.payload {
        VfxEventPayloadV1::PlayAnim {
            priority,
            fade_in_ticks,
            ..
        } => (*priority, *fade_in_ticks),
        other => panic!("expected PlayAnim, got {other:?}"),
    }
}

/// 取 StopAnim 的 fade_out_ticks。
fn stop_anim_fade_out(request: &VfxEventRequest) -> Option<u8> {
    match &request.payload {
        VfxEventPayloadV1::StopAnim { fade_out_ticks, .. } => *fade_out_ticks,
        other => panic!("expected StopAnim, got {other:?}"),
    }
}

fn assert_play_anim(request: &VfxEventRequest, expected_anim: &str, expected_priority: u16) {
    match &request.payload {
        VfxEventPayloadV1::PlayAnim {
            anim_id, priority, ..
        } => {
            assert_eq!(anim_id, expected_anim);
            assert_eq!(*priority, expected_priority);
        }
        other => panic!("expected PlayAnim, got {other:?}"),
    }
}

fn assert_stop_anim(request: &VfxEventRequest, expected_anim: &str, expected_fade: u8) {
    match &request.payload {
        VfxEventPayloadV1::StopAnim {
            anim_id,
            fade_out_ticks,
            ..
        } => {
            assert_eq!(anim_id, expected_anim);
            assert_eq!(*fade_out_ticks, Some(expected_fade));
        }
        other => panic!("expected StopAnim, got {other:?}"),
    }
}

fn assert_spawn_particle(
    request: &VfxEventRequest,
    expected_event_id: &str,
    expected_count: Option<u16>,
) {
    match &request.payload {
        VfxEventPayloadV1::SpawnParticle {
            event_id, count, ..
        } => {
            assert_eq!(event_id, expected_event_id);
            assert_eq!(*count, expected_count);
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

fn add_tuike_v2_visual_test_system(app: &mut App) {
    app.add_event::<DonFalseSkinEvent>();
    app.add_event::<FalseSkinSheddedEvent>();
    app.add_event::<ContamTransferredEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_tuike_v2_visual_triggers);
}

fn tuike_visual(
    skill: crate::combat::tuike_v2::TuikeSkillId,
    ancient: bool,
) -> crate::combat::tuike_v2::events::TuikeSkillVisualPayload {
    crate::combat::tuike_v2::TuikeSkillVisual::for_skill(skill, ancient).into()
}

fn assert_tuike_pair(
    requests: &[VfxEventRequest],
    expected_anim: &str,
    expected_particle: &str,
    expected_count: Option<u16>,
) {
    assert_eq!(requests.len(), 2);
    assert_play_anim(&requests[0], expected_anim, TUIKE_PRIORITY);
    assert_spawn_particle(&requests[1], expected_particle, expected_count);
}

fn assert_spawn_particle_color(request: &VfxEventRequest, expected_color: &str) {
    match &request.payload {
        VfxEventPayloadV1::SpawnParticle { color, .. } => {
            assert_eq!(color.as_deref(), Some(expected_color));
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

// ─── plan-sword-path-v2 P4：emit_sword_path_visual_triggers ───

fn sword_path_cast(
    skill: SwordPathSkillId,
    caster: valence::prelude::Entity,
    center: valence::prelude::DVec3,
    direction: Option<valence::prelude::DVec3>,
) -> SwordPathSkillCastEvent {
    SwordPathSkillCastEvent {
        skill,
        caster,
        center,
        direction,
        tick: 10,
    }
}

fn setup_sword_path_visual_app() -> App {
    let mut app = App::new();
    app.add_event::<SwordPathSkillCastEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_sword_path_visual_triggers);
    app
}

/// 凝锋 → 专属动画（P2 前半去复用）+ 专属粒子。
#[test]
fn condense_edge_emits_dedicated_anim_and_particle() {
    let mut app = setup_sword_path_visual_app();
    let caster = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
    app.world_mut().send_event(sword_path_cast(
        SwordPathSkillId::CondenseEdge,
        caster,
        valence::prelude::DVec3::new(0.0, 64.0, 0.0),
        None,
    ));
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        2,
        "凝锋应发 1 动画 + 1 粒子，实际 {}",
        emitted.len()
    );
    assert_play_anim(
        &emitted[0],
        ANIM_SWORD_PATH_CONDENSE_EDGE,
        SWORD_PATH_PRIORITY,
    );
    assert_ne!(
        play_anim_id(&emitted[0]),
        ANIM_SWORD_CLEAVE,
        "去复用回归锁：凝锋不得再借基础横劈 bong:sword_cleave"
    );
    assert_spawn_particle(&emitted[1], VFX_SWORD_CONDENSE_EDGE, Some(10));
}

/// 剑气斩 → 专属动画（P2 前半去复用）+ 朝向 line trail 粒子（direction 透传）。
#[test]
fn qi_slash_emits_dedicated_anim_and_directional_particle() {
    let mut app = setup_sword_path_visual_app();
    let caster = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
    let dir = valence::prelude::DVec3::new(0.0, 0.0, 1.0);
    app.world_mut().send_event(sword_path_cast(
        SwordPathSkillId::QiSlash,
        caster,
        valence::prelude::DVec3::new(0.0, 64.0, 0.0),
        Some(dir),
    ));
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 2);
    assert_play_anim(&emitted[0], ANIM_SWORD_PATH_QI_SLASH, SWORD_PATH_PRIORITY);
    assert_ne!(
        play_anim_id(&emitted[0]),
        ANIM_SWORD_THRUST,
        "去复用回归锁：剑气斩不得再借基础刺击 bong:sword_thrust"
    );
    match &emitted[1].payload {
        VfxEventPayloadV1::SpawnParticle {
            event_id,
            direction,
            ..
        } => {
            assert_eq!(event_id, VFX_SWORD_QI_SLASH_PATH);
            assert_eq!(
                *direction,
                Some([0.0, 0.0, 1.0]),
                "剑气斩 line trail 必须透传 caster→target 方向，否则粒子朝向退化"
            );
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

/// 化形 → 专属 manifest_cast 动画（不是基础剑斩）+ 召唤粒子。
#[test]
fn manifest_emits_dedicated_cast_anim() {
    let mut app = setup_sword_path_visual_app();
    let caster = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
    app.world_mut().send_event(sword_path_cast(
        SwordPathSkillId::Manifest,
        caster,
        valence::prelude::DVec3::new(0.0, 64.0, 0.0),
        None,
    ));
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 2);
    assert_play_anim(&emitted[0], ANIM_SWORD_MANIFEST_CAST, SWORD_PATH_PRIORITY);
    assert_spawn_particle(&emitted[1], VFX_SWORD_MANIFEST_SUMMON, Some(14));
}

/// 天门蓄力 → charge 动画 + charge 粒子（无额外 flash）。
#[test]
fn heaven_gate_charge_emits_charge_anim_only() {
    let mut app = setup_sword_path_visual_app();
    let caster = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
    app.world_mut().send_event(sword_path_cast(
        SwordPathSkillId::HeavenGateCharge,
        caster,
        valence::prelude::DVec3::new(0.0, 64.0, 0.0),
        None,
    ));
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        2,
        "蓄力应只发 1 动画 + 1 粒子（无 flash），实际 {}",
        emitted.len()
    );
    assert_play_anim(
        &emitted[0],
        ANIM_SWORD_HEAVEN_GATE_CHARGE,
        SWORD_PATH_PRIORITY,
    );
    assert_spawn_particle(&emitted[1], VFX_HEAVEN_GATE_CHARGE, Some(12));
}

/// 天门释放 → release 动画 + release 粒子 + 额外开天 flash 粒子（三件）。
#[test]
fn heaven_gate_release_emits_release_anim_plus_flash_layer() {
    let mut app = setup_sword_path_visual_app();
    let caster = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
    app.world_mut().send_event(sword_path_cast(
        SwordPathSkillId::HeavenGateRelease,
        caster,
        valence::prelude::DVec3::new(0.0, 64.0, 0.0),
        None,
    ));
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        3,
        "释放应发 1 动画 + release 粒子 + flash 粒子，实际 {}",
        emitted.len()
    );
    assert_play_anim(
        &emitted[0],
        ANIM_SWORD_HEAVEN_GATE_RELEASE,
        SWORD_PATH_PRIORITY,
    );
    assert_spawn_particle(&emitted[1], VFX_HEAVEN_GATE_RELEASE, Some(24));
    assert_spawn_particle(&emitted[2], VFX_HEAVEN_GATE_FLASH, Some(24));
}

/// 共鸣 → 专属动画（P2 前半去复用）+ 专属粒子。
#[test]
fn resonance_emits_dedicated_anim_and_particle() {
    let mut app = setup_sword_path_visual_app();
    let caster = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
    app.world_mut().send_event(sword_path_cast(
        SwordPathSkillId::Resonance,
        caster,
        valence::prelude::DVec3::new(0.0, 64.0, 0.0),
        None,
    ));
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        2,
        "共鸣应发 1 动画 + 1 粒子，实际 {}",
        emitted.len()
    );
    assert_play_anim(&emitted[0], ANIM_SWORD_PATH_RESONANCE, SWORD_PATH_PRIORITY);
    assert_ne!(
        play_anim_id(&emitted[0]),
        ANIM_SWORD_CLEAVE,
        "去复用回归锁：共鸣不得再借基础横劈 bong:sword_cleave"
    );
    assert_spawn_particle(&emitted[1], VFX_SWORD_RESONANCE, Some(16));
}

/// caster 无 UniqueId（非 skinned）→ 动画静默 skip，但粒子仍按 center 发出。
#[test]
fn sword_path_visual_without_unique_id_still_emits_particle() {
    let mut app = setup_sword_path_visual_app();
    // spawn 一个无 UniqueId 的 caster（emit_play_for_entity 会 skip 动画）
    let caster = app.world_mut().spawn(Position::new([5.0, 64.0, 5.0])).id();
    app.world_mut().send_event(sword_path_cast(
        SwordPathSkillId::Resonance,
        caster,
        valence::prelude::DVec3::new(5.0, 64.0, 5.0),
        None,
    ));
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        1,
        "无 UniqueId 时只发粒子（动画 skip），实际 {}",
        emitted.len()
    );
    assert_spawn_particle(&emitted[0], VFX_SWORD_RESONANCE, Some(16));
}

/// 凝锋 / 剑气斩 source 的 AttackIntent 不再走 emit_attack_animation_triggers
/// （避免与 emit_sword_path_visual_triggers 双重动画）。
#[test]
fn attack_animation_skips_sword_path_sources() {
    let mut app = App::new();
    app.insert_resource(CombatClock::default());
    app.add_event::<AttackIntent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_attack_animation_triggers);
    let attacker = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);

    for source in [
        AttackSource::SwordPathCondenseEdge,
        AttackSource::SwordPathQiSlash,
        AttackSource::SwordPathResonance,
        AttackSource::SwordPathManifest,
        AttackSource::SwordPathHeavenGate,
    ] {
        app.world_mut().send_event(AttackIntent {
            attacker,
            target: None,
            issued_at_tick: 1,
            reach: crate::combat::events::AttackReach::new(3.0, 0.0),
            qi_invest: 1.0,
            wound_kind: WoundKind::Cut,
            source,
            debug_command: None,
        });
    }
    app.update();

    assert!(
        drain_vfx(&mut app).is_empty(),
        "剑道 source 的 AttackIntent 不应触发通用攻击动画——动画由 \
         emit_sword_path_visual_triggers 独立负责"
    );
}

/// 回归保护：非剑道 source（基础剑劈）仍走 emit_attack_animation_triggers。
#[test]
fn attack_animation_still_fires_for_basic_sword_cleave() {
    let mut app = App::new();
    app.insert_resource(CombatClock::default());
    app.add_event::<AttackIntent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_attack_animation_triggers);
    let attacker = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
    app.world_mut().send_event(AttackIntent {
        attacker,
        target: None,
        issued_at_tick: 1,
        reach: crate::combat::events::AttackReach::new(3.0, 0.0),
        qi_invest: 0.0,
        wound_kind: WoundKind::Cut,
        source: AttackSource::SwordCleave,
        debug_command: None,
    });
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 1, "基础剑劈 source 仍应触发通用攻击动画");
    assert_play_anim(&emitted[0], ANIM_SWORD_CLEAVE, COMBAT_PRIORITY);
}

// ─── plan-shield-block-v1 P1 CR#5：emit_shield_raise_for_entity / emit_shield_stop_for_entity ───

/// Wrapper systems so we can use add_systems(Update, ...) instead of run_system_once,
/// which is the pattern already established in this test module.
fn shield_raise_system_for(
    entity: valence::prelude::Entity,
) -> impl Fn(Query<'_, '_, (&Position, &UniqueId)>, EventWriter<'_, VfxEventRequest>) {
    move |players, mut vfx_events| {
        emit_shield_raise_for_entity(entity, &players, &mut vfx_events);
    }
}

fn shield_stop_system_for(
    entity: valence::prelude::Entity,
) -> impl Fn(Query<'_, '_, (&Position, &UniqueId)>, EventWriter<'_, VfxEventRequest>) {
    move |players, mut vfx_events| {
        emit_shield_stop_for_entity(entity, &players, &mut vfx_events);
    }
}

fn setup_shield_vfx_app_for(
    entity_fn: impl FnOnce(&mut App) -> valence::prelude::Entity,
) -> (App, valence::prelude::Entity) {
    let mut app = App::new();
    app.add_event::<VfxEventRequest>();
    let entity = entity_fn(&mut app);
    (app, entity)
}

/// CR#5 — 有效实体（携带 Position + UniqueId）触发 emit_shield_raise_for_entity：
/// 断言发出 PlayAnim，anim_id == ANIM_SHIELD_RAISE，priority == COMBAT_PRIORITY，fade_in_ticks == Some(2)。
#[test]
fn emit_shield_raise_for_valid_entity_sends_play_anim_with_combat_priority() {
    let (mut app, entity) = setup_shield_vfx_app_for(|app| {
        spawn_skinned_npc_target(app, "shield:alice", [0.0, 64.0, 0.0])
    });
    app.add_systems(Update, shield_raise_system_for(entity));
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        1,
        "emit_shield_raise_for_entity must emit exactly 1 VfxEventRequest for valid entity; actual={}",
        emitted.len()
    );
    match &emitted[0].payload {
        VfxEventPayloadV1::PlayAnim {
            anim_id,
            priority,
            fade_in_ticks,
            ..
        } => {
            assert_eq!(
                anim_id, ANIM_SHIELD_RAISE,
                "emit_shield_raise_for_entity must use ANIM_SHIELD_RAISE='{ANIM_SHIELD_RAISE}'; actual='{anim_id}'"
            );
            assert_eq!(
                *priority, COMBAT_PRIORITY,
                "emit_shield_raise_for_entity priority must equal COMBAT_PRIORITY={COMBAT_PRIORITY}; actual={priority}"
            );
            assert_eq!(
                *fade_in_ticks,
                Some(2),
                "emit_shield_raise_for_entity fade_in_ticks must be Some(2); actual={fade_in_ticks:?}"
            );
        }
        other => panic!("expected PlayAnim from emit_shield_raise_for_entity, got {other:?}"),
    }
}

/// CR#5 — emit_shield_raise_for_entity 无效实体（缺 Position/UniqueId 组件）→ 静默 skip，不 panic，不 emit。
#[test]
fn emit_shield_raise_for_missing_entity_silently_skips() {
    let (mut app, missing_entity) = setup_shield_vfx_app_for(|app| {
        // spawn_empty：无 Position / UniqueId，模拟断线后实体已移除 Client component
        app.world_mut().spawn_empty().id()
    });
    app.add_systems(Update, shield_raise_system_for(missing_entity));
    // 不应 panic；entity 缺组件时 `let Ok(...) = players.get(entity)` 走 return 分支
    app.update();

    let emitted = drain_vfx(&mut app);
    assert!(
        emitted.is_empty(),
        "emit_shield_raise_for_entity must silently skip when entity lacks Position/UniqueId; actual emitted={}",
        emitted.len()
    );
}

/// CR#5 — 有效实体触发 emit_shield_stop_for_entity：
/// 断言发出 StopAnim，anim_id == ANIM_SHIELD_RAISE，fade_out_ticks == Some(3)。
#[test]
fn emit_shield_stop_for_valid_entity_sends_stop_anim() {
    let (mut app, entity) = setup_shield_vfx_app_for(|app| {
        spawn_skinned_npc_target(app, "shield:bob", [1.0, 64.0, 0.0])
    });
    app.add_systems(Update, shield_stop_system_for(entity));
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        1,
        "emit_shield_stop_for_entity must emit exactly 1 VfxEventRequest for valid entity; actual={}",
        emitted.len()
    );
    // fade_out=3 は emit_stop_for_entity 内の固定値
    assert_stop_anim(&emitted[0], ANIM_SHIELD_RAISE, 3);
}

/// CR#5 — emit_shield_stop_for_entity 无效实体（缺 Position/UniqueId 组件）→ 静默 skip，不 panic，不 emit。
#[test]
fn emit_shield_stop_for_missing_entity_silently_skips() {
    let (mut app, missing_entity) =
        setup_shield_vfx_app_for(|app| app.world_mut().spawn_empty().id());
    app.add_systems(Update, shield_stop_system_for(missing_entity));
    app.update();

    let emitted = drain_vfx(&mut app);
    assert!(
        emitted.is_empty(),
        "emit_shield_stop_for_entity must silently skip when entity lacks Position/UniqueId; actual emitted={}",
        emitted.len()
    );
}

/// CR#5 — emit_shield_raise_for_entity anim_id 与 ANIM_SHIELD_RAISE 常量一致（不是 guard_raise 等旧常量）。
#[test]
fn emit_shield_raise_anim_id_is_distinct_from_guard_raise() {
    // 确认 ANIM_SHIELD_RAISE != ANIM_GUARD_RAISE，两者语义不同
    assert_ne!(
        ANIM_SHIELD_RAISE, ANIM_GUARD_RAISE,
        "ANIM_SHIELD_RAISE and ANIM_GUARD_RAISE must be different animation ids; \
         shield_raise is a persistent looping block anim, guard_raise is FullPowerCharge"
    );
}

// ─── AV r3-P3#3：渡劫成功 VFX ───

fn make_tribulation_settled(
    entity: valence::prelude::Entity,
    outcome: DuXuOutcomeV1,
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
fn tribulation_ascended_settled_emits_breakthrough_pillar_and_tongling_animation() {
    // Expect: outcome=Ascended → PlayAnim(breakthrough_tongling, STORY_PRIORITY) +
    //         SpawnParticle(bong:breakthrough_pillar, count=16).
    let mut app = App::new();
    app.add_event::<TribulationSettled>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_tribulation_settled_vfx_triggers);
    let player = spawn_player(&mut app, "Alice", [5.0, 64.0, 5.0]);

    app.world_mut()
        .send_event(make_tribulation_settled(player, DuXuOutcomeV1::Ascended));
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        2,
        "Ascended outcome must emit exactly 2 VFX events (PlayAnim + SpawnParticle), got {}",
        emitted.len()
    );
    assert_play_anim(&emitted[0], ANIM_BREAKTHROUGH_TONGLING, STORY_PRIORITY);
    assert_spawn_particle(&emitted[1], "bong:breakthrough_pillar", Some(16));
}

#[test]
fn tribulation_halfstep_settled_emits_breakthrough_pillar_and_guyuan_animation() {
    // Expect: outcome=HalfStep → PlayAnim(breakthrough_guyuan, STORY_PRIORITY) +
    //         SpawnParticle(bong:breakthrough_pillar, count=10, 略低于 Ascended).
    let mut app = App::new();
    app.add_event::<TribulationSettled>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_tribulation_settled_vfx_triggers);
    let player = spawn_player(&mut app, "Bob", [5.0, 64.0, 5.0]);

    app.world_mut()
        .send_event(make_tribulation_settled(player, DuXuOutcomeV1::HalfStep));
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        2,
        "HalfStep outcome must emit exactly 2 VFX events (PlayAnim + SpawnParticle), got {}",
        emitted.len()
    );
    assert_play_anim(&emitted[0], ANIM_BREAKTHROUGH_GUYUAN, STORY_PRIORITY);
    assert_spawn_particle(&emitted[1], "bong:breakthrough_pillar", Some(10));
}

#[test]
fn tribulation_failed_settled_does_not_emit_success_vfx() {
    // Expect: outcome=Failed → no VFX (failure handled by TribulationFailed, not TribulationSettled).
    let mut app = App::new();
    app.add_event::<TribulationSettled>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_tribulation_settled_vfx_triggers);
    let player = spawn_player(&mut app, "Charlie", [5.0, 64.0, 5.0]);

    app.world_mut()
        .send_event(make_tribulation_settled(player, DuXuOutcomeV1::Failed));
    app.update();

    let emitted = drain_vfx(&mut app);
    assert!(
        emitted.is_empty(),
        "Failed outcome must not emit success VFX (handled by TribulationFailed reader), got {:?}",
        emitted.len()
    );
}

#[test]
fn tribulation_killed_fled_settled_does_not_emit_success_vfx() {
    // Expect: outcome=Killed/Fled → no VFX (not a success).
    let mut app = App::new();
    app.add_event::<TribulationSettled>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_tribulation_settled_vfx_triggers);
    let player = spawn_player(&mut app, "Dave", [5.0, 64.0, 5.0]);

    app.world_mut()
        .send_event(make_tribulation_settled(player, DuXuOutcomeV1::Killed));
    app.world_mut()
        .send_event(make_tribulation_settled(player, DuXuOutcomeV1::Fled));
    app.update();

    let emitted = drain_vfx(&mut app);
    assert!(
        emitted.is_empty(),
        "Killed/Fled outcomes must not emit success VFX, got {}",
        emitted.len()
    );
}

#[test]
fn existing_tribulation_failure_animation_unaffected_by_settled_system() {
    // Regression guard: emit_tribulation_animation_triggers still fires hurt_stagger for
    // TribulationFailed events after adding the separate settled system.
    let mut app = App::new();
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationFailed>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_tribulation_animation_triggers);
    let player = spawn_player(&mut app, "Eve", [5.0, 64.0, 5.0]);

    use crate::cultivation::tribulation::TribulationFailed;
    app.world_mut().send_event(TribulationFailed {
        entity: player,
        wave: 2,
    });
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        2,
        "TribulationFailed must emit StopAnim(tribulation_brace) + PlayAnim(hurt_stagger), got {}",
        emitted.len()
    );
    assert_stop_anim(
        &emitted[0],
        ANIM_TRIBULATION_BRACE,
        TRIBULATION_BRACE_STOP_FADE_OUT_TICKS,
    );
    assert_play_anim(&emitted[1], ANIM_HURT_STAGGER, HIT_RECOIL_PRIORITY);
}

#[test]
fn tribulation_failed_stops_stuck_brace_full_body_loop() {
    // 渡劫失败必须显式 StopAnim(brace) 收掉 FULL_BODY 循环（根因见
    // TRIBULATION_BRACE_STOP_FADE_OUT_TICKS 注释）。Arrange: Announce 起播 brace →
    // Act: Failed → Assert: 恰好 StopAnim(brace) + PlayAnim(hurt_stagger)。
    let mut app = App::new();
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationFailed>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_tribulation_animation_triggers);
    let player = spawn_player(&mut app, "Frank", [5.0, 64.0, 5.0]);

    // 1. TribulationAnnounce 起播 brace 循环（FULL_BODY，STORY_PRIORITY）。
    app.world_mut().send_event(TribulationAnnounce {
        entity: player,
        char_id: "frank".to_string(),
        actor_name: "Frank".to_string(),
        epicenter: [5.0, 64.0, 5.0],
        waves_total: 3,
        started_tick: 0,
    });
    app.update();
    let announce_emitted = drain_vfx(&mut app);
    assert_eq!(announce_emitted.len(), 1);
    assert_play_anim(&announce_emitted[0], ANIM_TRIBULATION_BRACE, STORY_PRIORITY);

    // 2. 渡劫失败：必须显式 StopAnim(brace) 收掉 FULL_BODY 循环，否则永久卡姿势。
    app.world_mut().send_event(TribulationFailed {
        entity: player,
        wave: 2,
    });
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        2,
        "TribulationFailed must emit StopAnim(tribulation_brace) to release the FULL_BODY \
         channel plus PlayAnim(hurt_stagger); got {} — without the StopAnim the player is \
         stuck in the brace pose forever",
        emitted.len()
    );
    assert_stop_anim(
        &emitted[0],
        ANIM_TRIBULATION_BRACE,
        TRIBULATION_BRACE_STOP_FADE_OUT_TICKS,
    );
    assert_play_anim(&emitted[1], ANIM_HURT_STAGGER, HIT_RECOIL_PRIORITY);
}

#[test]
fn tribulation_settled_success_outcomes_do_not_regress_with_explicit_brace_stop() {
    // 成功结局（Ascended/HalfStep）靠 STORY_PRIORITY 的 breakthrough PlayAnim 覆盖 brace 循环，
    // 故**不**额外发 StopAnim（brace 修复只针对存活卡姿的 TribulationFailed）。断言具体事件契约
    // 而非仅个数，让 spurious StopAnim 或错 anim/particle 判红。
    for (outcome, expected_anim, expected_count) in [
        (DuXuOutcomeV1::Ascended, ANIM_BREAKTHROUGH_TONGLING, 16u16),
        (DuXuOutcomeV1::HalfStep, ANIM_BREAKTHROUGH_GUYUAN, 10u16),
    ] {
        let mut app = App::new();
        app.add_event::<TribulationSettled>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_tribulation_settled_vfx_triggers);
        let player = spawn_player(&mut app, "Grace", [5.0, 64.0, 5.0]);

        app.world_mut()
            .send_event(make_tribulation_settled(player, outcome));
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            2,
            "success settlement ({outcome:?}) must emit exactly PlayAnim(breakthrough) + \
             SpawnParticle(pillar); got {}",
            emitted.len()
        );
        // event[0]: breakthrough anim on the FULL_BODY channel at STORY_PRIORITY — this is what
        // overwrites the brace loop, so its id + priority are the load-bearing contract.
        assert_play_anim(&emitted[0], expected_anim, STORY_PRIORITY);
        // event[1]: breakthrough pillar particle, outcome-specific density.
        assert_spawn_particle(
            &emitted[1],
            "bong:breakthrough_pillar",
            Some(expected_count),
        );
        // The contract this test's name claims: NO StopAnim on the success path. A StopAnim here
        // (e.g. copy-pasted from the TribulationFailed fix) would still keep count at 2 if it
        // displaced the particle, so the count check alone can't catch it — assert absence.
        assert!(
            !emitted
                .iter()
                .any(|e| matches!(e.payload, VfxEventPayloadV1::StopAnim { .. })),
            "success settlement ({outcome:?}) must NOT emit StopAnim — the breakthrough anim \
             already overwrites the FULL_BODY brace loop; a StopAnim here means the \
             failure-path fix leaked into the success path. emitted={emitted:?}"
        );
    }
}

// ─── 暗器六招：emit_anqi_visual_triggers ──────────────────────

fn setup_anqi_visual_app() -> App {
    let mut app = App::new();
    app.add_event::<CarrierChargedEvent>();
    app.add_event::<CarrierChargeBeganEvent>();
    app.add_event::<CarrierChargeEndedEvent>();
    app.add_event::<QiInjectionEvent>();
    app.add_event::<MultiShotEvent>();
    app.add_event::<ArmorPierceEvent>();
    app.add_event::<EchoFractalEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_anqi_visual_triggers);
    app
}

fn anqi_injection_outcome() -> crate::qi_physics::HighDensityInjectionOutcome {
    crate::qi_physics::HighDensityInjectionOutcome {
        payload_qi: 50.0,
        wound_qi: 40.0,
        contamination_qi: 5.0,
        overload_ratio: 0.5,
        triggers_overload_tear: false,
    }
}

/// 封骨充能开始（P2 后半两段式）：循环蓄力段 anqi_charge_carrier_loop 起播，
/// 负向锁不再借通用蓄力 windup_charge。
#[test]
fn anqi_charge_began_plays_carrier_loop_anim() {
    let mut app = setup_anqi_visual_app();
    let caster = spawn_player(&mut app, "Carry", [0.0, 64.0, 0.0]);
    app.world_mut().send_event(CarrierChargeBeganEvent {
        carrier: caster,
        tick: 10,
    });
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 1, "充能开始应恰 emit 1 条循环蓄力段动画");
    assert_play_anim(&emitted[0], ANIM_ANQI_CHARGE, ANQI_PRIORITY);
    assert_ne!(
        play_anim_id(&emitted[0]),
        "bong:windup_charge",
        "去复用回归锁：封骨蓄力段不得再借通用蓄力 bong:windup_charge"
    );
}

/// 封骨充能自然完成：StopAnim(循环段) + release 收势 PlayAnim（两段式接力）。
#[test]
fn anqi_charge_ended_full_stops_loop_and_plays_release() {
    let mut app = setup_anqi_visual_app();
    let caster = spawn_player(&mut app, "CarryDone", [0.0, 64.0, 0.0]);
    app.world_mut().send_event(CarrierChargeEndedEvent {
        carrier: caster,
        full_charge: true,
        tick: 410,
    });
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        2,
        "自然完成应 emit StopAnim(循环) + PlayAnim(release)"
    );
    match &emitted[0].payload {
        VfxEventPayloadV1::StopAnim {
            anim_id,
            fade_out_ticks,
            ..
        } => {
            assert_eq!(
                anim_id, ANIM_ANQI_CHARGE,
                "停止的必须是循环蓄力段（§8.1 #3 停止路径红线）"
            );
            assert_eq!(fade_out_ticks, &Some(ANQI_CHARGE_STOP_FADE_OUT_TICKS));
        }
        other => panic!("expected StopAnim, got {other:?}"),
    }
    assert_play_anim(&emitted[1], ANIM_ANQI_CHARGE_RELEASE, ANQI_PRIORITY);
}

/// P6 相位承接契约 pin（conventions §14.2）——封骨两段式交接的 fade 形状。
///
/// 契约要害（读 PlayerAnimator 源码得出，靠猜必错）：同 channel 换 animId 时，
/// 旧层带 `fade_out` 留栈并**位于新层之上**向其混合下去，混合源是蓄力段
/// **当前相位**的姿态——所以引导窗停在任意相位都能平滑承接。三个前提缺一不可：
///
/// 1. `fade_out_ticks >= 2`：`fade_out = 0` 会立即摘层、混合源消失，交接瞬间
///    姿态塌回 vanilla 中立（实测，见 client `TwoStageHandoffBlendTest`
///    `zeroFadeOutBreaksContinuityAndIsThereforeForbiddenForTwoStage`）；
/// 2. release 的 `fade_in <= fade_out`：release 必须在淡出窗口内尽快成为完整的
///    混合目标，否则混合目标自身还是半 vanilla 状态，造成二次塌陷；
/// 3. 两段**同 priority**：跨优先级会打破层序前提，混合源随之失效。
#[test]
fn anqi_two_stage_handoff_satisfies_phase_handoff_contract() {
    let mut app = setup_anqi_visual_app();
    let caster = spawn_player(&mut app, "CarryFade", [0.0, 64.0, 0.0]);
    app.world_mut().send_event(CarrierChargeBeganEvent {
        carrier: caster,
        tick: 10,
    });
    app.update();
    let began = drain_vfx(&mut app);
    let (loop_priority, _) = play_anim_priority_and_fade_in(&began[0]);

    app.world_mut().send_event(CarrierChargeEndedEvent {
        carrier: caster,
        full_charge: true,
        tick: 410,
    });
    app.update();
    let ended = drain_vfx(&mut app);
    assert_eq!(
        ended.len(),
        2,
        "自然完成应 StopAnim(循环) + PlayAnim(release)"
    );

    let fade_out = stop_anim_fade_out(&ended[0])
        .expect("两段式循环段停止必须显式带 fade_out——None 会走客户端默认值，契约不可依赖默认");
    let (release_priority, release_fade_in) = play_anim_priority_and_fade_in(&ended[1]);
    let fade_in =
        release_fade_in.expect("release 段必须显式带 fade_in——交接混合的时间预算不可留给默认值");

    assert!(
        fade_out >= 2,
        "循环段停止 fade_out={fade_out} < 2：fade_out=0/1 会让混合源过早消失，\
         交接瞬间姿态塌回 vanilla（conventions §14.2 硬约束 1）"
    );
    assert!(
        fade_in <= fade_out,
        "release fade_in={fade_in} > 循环段 fade_out={fade_out}：release 必须在淡出\
         窗口内尽快成为完整混合目标，否则混合目标自身仍是半 vanilla 状态\
         （conventions §14.2 硬约束 3）"
    );
    assert_eq!(
        loop_priority, release_priority,
        "两段式两段必须同 priority——跨优先级会打破「淡出层在上、向下层混合」的\
         层序前提，混合源随之失效（conventions §14.2 硬约束 2）"
    );
}

/// P6 相位承接契约 pin——天门两段式（定长相位充能型，conventions §14.1）
/// 两段同 priority。该招无 StopAnim（充能段非循环、自然播完），交接靠客户端
/// 同 channel 换 id 的默认淡出，故此处只锁 priority 一致性这一前提。
#[test]
fn heaven_gate_two_stage_uses_same_priority_for_both_phases() {
    let mut app = setup_sword_path_visual_app();
    let caster = spawn_player(&mut app, "HeavenGate", [0.0, 64.0, 0.0]);

    app.world_mut().send_event(sword_path_cast(
        SwordPathSkillId::HeavenGateCharge,
        caster,
        valence::prelude::DVec3::new(0.0, 64.0, 0.0),
        None,
    ));
    app.update();
    let charge = drain_vfx(&mut app).remove(0);

    app.world_mut().send_event(sword_path_cast(
        SwordPathSkillId::HeavenGateRelease,
        caster,
        valence::prelude::DVec3::new(0.0, 64.0, 0.0),
        None,
    ));
    app.update();
    let release = drain_vfx(&mut app).remove(0);

    let (charge_priority, _) = play_anim_priority_and_fade_in(&charge);
    let (release_priority, _) = play_anim_priority_and_fade_in(&release);
    assert_eq!(
        charge_priority, release_priority,
        "天门充能段与 release 段必须同 priority（conventions §14.1/§14.2）"
    );
    assert_eq!(
        play_anim_id(&charge),
        ANIM_SWORD_HEAVEN_GATE_CHARGE,
        "充能段动画 id 漂移"
    );
    assert_eq!(
        play_anim_id(&release),
        ANIM_SWORD_HEAVEN_GATE_RELEASE,
        "release 段动画 id 漂移"
    );
}

/// 封骨充能移动打断：仅 StopAnim(循环段)，**无** release 收势（打断不奖励收势）。
#[test]
fn anqi_charge_ended_interrupted_stops_loop_without_release() {
    let mut app = setup_anqi_visual_app();
    let caster = spawn_player(&mut app, "CarryMove", [0.0, 64.0, 0.0]);
    app.world_mut().send_event(CarrierChargeEndedEvent {
        carrier: caster,
        full_charge: false,
        tick: 200,
    });
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        1,
        "移动打断应只 emit StopAnim（打断不奖励收势），实际 {} 条",
        emitted.len()
    );
    match &emitted[0].payload {
        VfxEventPayloadV1::StopAnim { anim_id, .. } => {
            assert_eq!(anim_id, ANIM_ANQI_CHARGE);
        }
        other => panic!("expected StopAnim, got {other:?}"),
    }
}

/// 封骨密封落定（CarrierChargedEvent）：仅密封粒子——动画已由 Began/Ended
/// 两段式接管，本事件不得再播任何 PlayAnim（负向回归锁）。
#[test]
fn anqi_charged_event_emits_seal_particle_only() {
    use crate::cultivation::components::ColorKind;
    let mut app = setup_anqi_visual_app();
    let caster = spawn_player(&mut app, "Carry", [0.0, 64.0, 0.0]);
    app.world_mut().send_event(CarrierChargedEvent {
        carrier: caster,
        instance_id: 1,
        qi_amount: 25.0,
        qi_color: ColorKind::Solid,
        full_charge: true,
        tick: 10,
    });
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        1,
        "CarrierChargedEvent 应只 emit 密封粒子（动画归 Began/Ended），实际 {} 条",
        emitted.len()
    );
    assert_spawn_particle(&emitted[0], VFX_ANQI_CHARGE_SEAL, Some(12));
}

/// 单射狙击：anqi_single_snipe 专属动画（P2 前半去复用）+ 弹道粒子
/// （caster→target 方向 +Z）。
#[test]
fn anqi_snipe_emits_directional_bolt() {
    use crate::combat::anqi_v2::AnqiSkillId;
    use crate::combat::carrier::CarrierKind;
    let mut app = setup_anqi_visual_app();
    let caster = spawn_player(&mut app, "Sniper", [0.0, 64.0, 0.0]);
    let target = spawn_skinned_npc_target(&mut app, "Mark", [0.0, 64.0, 5.0]);
    app.world_mut().send_event(QiInjectionEvent {
        caster,
        target: Some(target),
        skill: AnqiSkillId::SingleSnipe,
        carrier_kind: CarrierKind::YibianShougu,
        outcome: anqi_injection_outcome(),
        tick: 11,
    });
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 2, "单射应 emit 1 动画 + 1 粒子");
    assert_play_anim(&emitted[0], ANIM_ANQI_SNIPE, ANQI_PRIORITY);
    assert_ne!(
        play_anim_id(&emitted[0]),
        "bong:sword_stab",
        "去复用回归锁：单射狙击不得再借剑刺 bong:sword_stab"
    );
    match &emitted[1].payload {
        VfxEventPayloadV1::SpawnParticle {
            event_id,
            direction,
            ..
        } => {
            assert_eq!(event_id, VFX_ANQI_SNIPE_BOLT);
            let dir = direction.expect("单射弹道必须带 caster→target 方向");
            assert!(
                (dir[2] - 1.0).abs() < 1e-6 && dir[0].abs() < 1e-6,
                "方向应为 +Z 单位向量（caster→target），实际 {dir:?}"
            );
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

/// 凝魂注射：anqi_soul_inject 专属动画（P2 前半去复用）+ 魂注紫雾（无方向）。
#[test]
fn anqi_soul_inject_emits_dedicated_anim_and_mist() {
    use crate::combat::anqi_v2::AnqiSkillId;
    use crate::combat::carrier::CarrierKind;
    let mut app = setup_anqi_visual_app();
    let caster = spawn_player(&mut app, "Soul", [0.0, 64.0, 0.0]);
    app.world_mut().send_event(QiInjectionEvent {
        caster,
        target: None,
        skill: AnqiSkillId::SoulInject,
        carrier_kind: CarrierKind::DyedBone,
        outcome: anqi_injection_outcome(),
        tick: 12,
    });
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 2);
    assert_play_anim(&emitted[0], ANIM_ANQI_INJECT, ANQI_PRIORITY);
    assert_ne!(
        play_anim_id(&emitted[0]),
        "bong:cast_invoke",
        "去复用回归锁：凝魂注射不得再借通用施法 bong:cast_invoke"
    );
    assert_spawn_particle(&emitted[1], VFX_ANQI_SOUL_INJECT, Some(16));
}

/// 多发齐射：anqi_multi_shot 专属动画（P2 前半去复用）+ 扇形齐射粒子
/// （粒子数随弹数缩放）。
#[test]
fn anqi_multi_shot_scales_particle_count_with_projectiles() {
    use crate::combat::carrier::CarrierKind;
    let mut app = setup_anqi_visual_app();
    let caster = spawn_player(&mut app, "Volley", [0.0, 64.0, 0.0]);
    app.world_mut().send_event(MultiShotEvent {
        caster,
        projectile_count: 5,
        carrier_kind: CarrierKind::LingmuArrow,
        shots: Vec::new(),
        tick: 13,
    });
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 2);
    assert_play_anim(&emitted[0], ANIM_ANQI_VOLLEY, ANQI_PRIORITY);
    assert_ne!(
        play_anim_id(&emitted[0]),
        "bong:release_burst",
        "去复用回归锁：多发齐射不得再借通用爆发 bong:release_burst"
    );
    // 5 发 × 4 = 20 颗（在 clamp [8,40] 内）。
    assert_spawn_particle(&emitted[1], VFX_ANQI_MULTI_VOLLEY, Some(20));
}

/// 破甲注射：anqi_armor_pierce 专属动画（P2 后半去复用）+ 破甲火花
/// （caster→target 方向）。
#[test]
fn anqi_armor_pierce_emits_directional_sparks() {
    use crate::combat::carrier::CarrierKind;
    let mut app = setup_anqi_visual_app();
    let caster = spawn_player(&mut app, "Pierce", [0.0, 64.0, 0.0]);
    let target = spawn_skinned_npc_target(&mut app, "Armor", [5.0, 64.0, 0.0]);
    app.world_mut().send_event(ArmorPierceEvent {
        caster,
        target: Some(target),
        carrier_kind: CarrierKind::FenglingheBone,
        outcome: crate::qi_physics::ArmorPenetrationOutcome {
            base_damage: 60.0,
            ignored_defense_ratio: 0.6,
            effective_damage: 70.0,
            carrier_shatter_probability: 0.2,
        },
        tick: 14,
    });
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 2);
    assert_play_anim(&emitted[0], ANIM_ANQI_ARMOR_PIERCE, ANQI_PRIORITY);
    assert_ne!(
        play_anim_id(&emitted[0]),
        "bong:cast_invoke",
        "去复用回归锁：破甲注射不得再借通用施法 bong:cast_invoke"
    );
    match &emitted[1].payload {
        VfxEventPayloadV1::SpawnParticle {
            event_id,
            direction,
            ..
        } => {
            assert_eq!(event_id, VFX_ANQI_ARMOR_PIERCE);
            let dir = direction.expect("破甲火花必须带 caster→target 方向");
            assert!(
                (dir[0] - 1.0).abs() < 1e-6,
                "方向应为 +X 单位向量（caster→target），实际 {dir:?}"
            );
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

/// 诱饵分形：anqi_echo_fractal 专属动画（P2 后半去复用）+ 分形回响涟漪
/// （粒子随分身数缩放）。
#[test]
fn anqi_echo_fractal_emits_decoy_ripple() {
    use crate::combat::carrier::CarrierKind;
    let mut app = setup_anqi_visual_app();
    let caster = spawn_player(&mut app, "Echo", [0.0, 64.0, 0.0]);
    app.world_mut().send_event(EchoFractalEvent {
        caster,
        carrier_kind: CarrierKind::ShangguBone,
        outcome: crate::qi_physics::EchoFractalOutcome {
            local_qi_density: 9.0,
            threshold: 0.3,
            echo_count: 4,
            damage_per_echo: 2.0,
        },
        tick: 15,
    });
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 2);
    assert_play_anim(&emitted[0], ANIM_ANQI_ECHO, ANQI_PRIORITY);
    assert_ne!(
        play_anim_id(&emitted[0]),
        "bong:release_burst",
        "去复用回归锁：诱饵分形不得再借通用爆发 bong:release_burst"
    );
    // 4 分身 × 5 = 20 颗（在 clamp [10,40] 内）。
    assert_spawn_particle(&emitted[1], VFX_ANQI_ECHO_DECOY, Some(20));
}

/// caster 无 Position（被拒绝/异常态）→ 封骨不 emit（守纯加法不污染）。
#[test]
fn anqi_charge_without_position_emits_nothing() {
    use crate::cultivation::components::ColorKind;
    let mut app = setup_anqi_visual_app();
    let caster = app.world_mut().spawn_empty().id();
    app.world_mut().send_event(CarrierChargedEvent {
        carrier: caster,
        instance_id: 1,
        qi_amount: 25.0,
        qi_color: ColorKind::Solid,
        full_charge: true,
        tick: 10,
    });
    app.update();
    assert!(
        drain_vfx(&mut app).is_empty(),
        "caster 无 Position 时封骨 VFX 应静默 skip（纯 cosmetic 不强行渲染）"
    );
}

// ─── 蛊道两招（凝针 / 灌毒蛊）emit_dugu_needle_visual_triggers ───

fn setup_dugu_visual_app() -> App {
    let mut app = App::new();
    app.add_event::<QiNeedleChargedEvent>();
    app.add_event::<DuguObfuscationDisruptedEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_dugu_needle_visual_triggers);
    app
}

/// 凝针：dugu_needle_throw 动画 + 朝向弹道粒子（caster→target 方向 +Z）。
#[test]
fn dugu_shoot_needle_emits_directional_bolt() {
    let mut app = setup_dugu_visual_app();
    let shooter = spawn_player(&mut app, "Needle", [0.0, 64.0, 0.0]);
    let target = spawn_skinned_npc_target(&mut app, "Victim", [0.0, 64.0, 6.0]);
    app.world_mut().send_event(QiNeedleChargedEvent {
        shooter,
        target: Some(target),
        tick: 7,
    });
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 2, "凝针应 emit 1 动画 + 1 弹道粒子");
    assert_play_anim(&emitted[0], ANIM_DUGU_NEEDLE_THROW, DUGU_PRIORITY);
    match &emitted[1].payload {
        VfxEventPayloadV1::SpawnParticle {
            event_id,
            direction,
            ..
        } => {
            assert_eq!(
                event_id, VFX_DUGU_NEEDLE_BOLT,
                "凝针弹道 event_id 必须与 client DuguNeedleVfxPlayer.DUGU_NEEDLE_BOLT 对齐"
            );
            let dir = direction.expect("凝针弹道必须带 caster→target 方向（细针直刺）");
            assert!(
                (dir[2] - 1.0).abs() < 1e-6 && dir[0].abs() < 1e-6,
                "方向应为 +Z 单位向量（caster→target），实际 {dir:?}"
            );
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

/// 凝针无目标：方向退化到 +X fallback（仍 emit，避免远程招无反馈）。
#[test]
fn dugu_shoot_needle_without_target_falls_back_to_default_direction() {
    let mut app = setup_dugu_visual_app();
    let shooter = spawn_player(&mut app, "NeedleNoTgt", [0.0, 64.0, 0.0]);
    app.world_mut().send_event(QiNeedleChargedEvent {
        shooter,
        target: None,
        tick: 8,
    });
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 2);
    assert_play_anim(&emitted[0], ANIM_DUGU_NEEDLE_THROW, DUGU_PRIORITY);
    match &emitted[1].payload {
        VfxEventPayloadV1::SpawnParticle {
            event_id,
            direction,
            ..
        } => {
            assert_eq!(event_id, VFX_DUGU_NEEDLE_BOLT);
            let dir = direction.expect("无目标仍透传方向（fallback）");
            assert!(
                (dir[0] - 1.0).abs() < 1e-6,
                "无目标方向应退化为 +X fallback，实际 {dir:?}"
            );
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

/// 灌毒蛊：P3 专属淬毒动画（去共用，原与凝针同发 throw）+ 毒绿雾（无方向，
/// 绕身散布）。
#[test]
fn dugu_infuse_poison_emits_bespoke_infuse_anim_and_poison_mist() {
    let mut app = setup_dugu_visual_app();
    let infuser = spawn_player(&mut app, "Infuser", [0.0, 64.0, 0.0]);
    app.world_mut().send_event(DuguObfuscationDisruptedEvent {
        infuser,
        until_tick: 200,
    });
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 2, "灌毒蛊应 emit 1 动画 + 1 毒雾粒子");
    assert_play_anim(&emitted[0], ANIM_DUGU_INFUSE_POISON, DUGU_PRIORITY);
    // P3 去共用回归锁：灌毒蛊不得再借凝针掷出动画（两招此前动画字符串完全
    // 相同、仅靠去重 id 区分，远观无法分辨"淬毒"与"射针"）。
    match &emitted[0].payload {
        VfxEventPayloadV1::PlayAnim { anim_id, .. } => assert_ne!(
            anim_id, ANIM_DUGU_NEEDLE_THROW,
            "去复用回归锁：灌毒蛊不得回退到共用 dugu_needle_throw"
        ),
        other => panic!("expected PlayAnim, got {other:?}"),
    }
    assert_spawn_particle(&emitted[1], VFX_DUGU_POISON_INFUSE, Some(14));
    match &emitted[1].payload {
        VfxEventPayloadV1::SpawnParticle { direction, .. } => {
            assert!(direction.is_none(), "灌毒蛊毒雾绕身散布，不应带方向");
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

/// shooter 无 player 组件（无 Position/UniqueId）→ 动画因 player 查询失败 skip，
/// 弹道粒子仍以默认 origin emit（与 anqi 弹道 unwrap_or_default 同口径，远程招不丢反馈）。
#[test]
fn dugu_shoot_needle_without_player_components_skips_anim_keeps_particle() {
    let mut app = setup_dugu_visual_app();
    let shooter = app.world_mut().spawn_empty().id();
    app.world_mut().send_event(QiNeedleChargedEvent {
        shooter,
        target: None,
        tick: 9,
    });
    app.update();
    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        1,
        "无 player 组件时动画 skip、仅保留 1 条弹道粒子，实际 {} 条",
        emitted.len()
    );
    assert_spawn_particle(&emitted[0], VFX_DUGU_NEEDLE_BOLT, Some(10));
}

// ========== 蛊道 v2 五招粒子（emit_dugu_v2_visual_triggers） ==========

fn setup_dugu_v2_visual_app() -> App {
    let mut app = App::new();
    app.add_event::<EclipseNeedleEvent>();
    app.add_event::<SelfCureProgressEvent>();
    app.add_event::<PenetrateChainEvent>();
    app.add_event::<ShroudActivatedEvent>();
    app.add_event::<ReverseTriggeredEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_dugu_v2_visual_triggers);
    app
}

fn dugu_v2_visual(
    skill: crate::combat::dugu_v2::events::DuguSkillId,
) -> crate::combat::dugu_v2::events::DuguSkillVisual {
    crate::combat::dugu_v2::skills::visual_for(skill)
}

fn assert_spawn_particle_origin(request: &VfxEventRequest, expected: [f64; 3]) {
    match &request.payload {
        VfxEventPayloadV1::SpawnParticle { origin, .. } => {
            assert_eq!(
                *origin, expected,
                "粒子 origin 应落在 {expected:?}（事件语义位置），实际 {origin:?}"
            );
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

/// 蚀针：毒渍脉冲印于受害者（target）脚下，event_id 与 client DuguV2VfxPlayer 对齐。
#[test]
fn dugu_v2_eclipse_emits_taint_pulse_at_target() {
    use crate::combat::dugu_v2::events::{DuguSkillId, TaintTier};
    let mut app = setup_dugu_v2_visual_app();
    let caster = spawn_player(&mut app, "Caster", [0.0, 64.0, 0.0]);
    let target = spawn_player(&mut app, "Victim", [5.0, 64.0, 5.0]);
    app.world_mut().send_event(EclipseNeedleEvent {
        caster,
        target,
        target_realm: Realm::Awaken,
        tier: TaintTier::Temporary,
        injected_qi: 4.0,
        hp_loss: 2.0,
        qi_loss: 3.0,
        qi_max_loss: 0.0,
        permanent_decay_rate_per_min: 0.0,
        returned_zone_qi: 3.0,
        reveal_probability: 0.1,
        tick: 7,
        visual: dugu_v2_visual(DuguSkillId::Eclipse),
    });
    app.update();
    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        1,
        "蚀针应只发 1 条粒子（anim/audio 在 skills.rs 内联）"
    );
    assert_spawn_particle(&emitted[0], "bong:dugu_taint_pulse", Some(12));
    assert_spawn_particle_origin(&emitted[0], [5.0, 64.0, 5.0]);
}

/// 蚀针受害者断 Position → 落到施法者位置（出手反馈不丢）。
#[test]
fn dugu_v2_eclipse_falls_back_to_caster_origin_when_target_positionless() {
    use crate::combat::dugu_v2::events::{DuguSkillId, TaintTier};
    let mut app = setup_dugu_v2_visual_app();
    let caster = spawn_player(&mut app, "Caster", [1.0, 64.0, 2.0]);
    let target = app.world_mut().spawn_empty().id();
    app.world_mut().send_event(EclipseNeedleEvent {
        caster,
        target,
        target_realm: Realm::Awaken,
        tier: TaintTier::Immediate,
        injected_qi: 1.0,
        hp_loss: 1.0,
        qi_loss: 1.0,
        qi_max_loss: 0.0,
        permanent_decay_rate_per_min: 0.0,
        returned_zone_qi: 1.0,
        reveal_probability: 0.1,
        tick: 8,
        visual: dugu_v2_visual(DuguSkillId::Eclipse),
    });
    app.update();
    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        1,
        "受害者断 Position 时粒子应回落到施法者位置（出手反馈不丢），实际 {} 条",
        emitted.len()
    );
    assert_spawn_particle_origin(&emitted[0], [1.0, 64.0, 2.0]);
}

/// 自蕴：稀薄深绿雾绕施法者。
#[test]
fn dugu_v2_self_cure_emits_thin_mist_at_caster() {
    use crate::combat::dugu_v2::events::DuguSkillId;
    let mut app = setup_dugu_v2_visual_app();
    let caster = spawn_player(&mut app, "Caster", [3.0, 64.0, 3.0]);
    app.world_mut().send_event(SelfCureProgressEvent {
        caster,
        hours_used: 2.5,
        daily_hours_after: 2.5,
        gain_percent: 10.0,
        insidious_color_percent: 5.0,
        morphology_percent: 1.0,
        self_revealed: false,
        tick: 9,
        visual: dugu_v2_visual(DuguSkillId::SelfCure),
    });
    app.update();
    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        1,
        "自蕴应只发 1 条雾粒子（anim/audio 在 skills.rs 内联），实际 {} 条",
        emitted.len()
    );
    assert_spawn_particle(&emitted[0], "bong:dugu_dark_green_mist", Some(14));
    assert_spawn_particle_origin(&emitted[0], [3.0, 64.0, 3.0]);
}

/// 侵染：毒渍密度随链上受害者数增长且封顶 32。
#[test]
fn dugu_v2_penetrate_particle_count_scales_with_targets_and_caps() {
    use crate::combat::dugu_v2::events::{DuguSkillId, TaintTier};
    let mut app = setup_dugu_v2_visual_app();
    let caster = spawn_player(&mut app, "Caster", [0.0, 64.0, 0.0]);
    let target = spawn_player(&mut app, "Victim", [2.0, 64.0, 0.0]);
    for (affected, expected_count) in [(2u32, 20u16), (99u32, 32u16)] {
        app.world_mut().send_event(PenetrateChainEvent {
            caster,
            target,
            taint_tier: TaintTier::Temporary,
            multiplier: 1.5,
            affected_targets: affected,
            permanent_decay_rate_per_min: 0.0,
            reveal_probability: 0.2,
            returned_zone_qi: 5.0,
            tick: 10,
            visual: dugu_v2_visual(DuguSkillId::Penetrate),
        });
        app.update();
        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "侵染（affected={affected}）应只发 1 条毒渍粒子，实际 {} 条",
            emitted.len()
        );
        assert_spawn_particle(&emitted[0], "bong:dugu_taint_pulse", Some(expected_count));
    }
}

/// 神识遮蔽：深绿雾罩绕施法者，strength 透传（钳 [0.6, 1.5]）。
#[test]
fn dugu_v2_shroud_emits_mist_with_clamped_strength() {
    use crate::combat::dugu_v2::events::DuguSkillId;
    let mut app = setup_dugu_v2_visual_app();
    let caster = spawn_player(&mut app, "Caster", [0.0, 64.0, 0.0]);
    app.world_mut().send_event(ShroudActivatedEvent {
        caster,
        strength: 9.0,
        expires_at_tick: 600,
        tick: 11,
        visual: dugu_v2_visual(DuguSkillId::Shroud),
    });
    app.update();
    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        1,
        "神识遮蔽应只发 1 条雾罩粒子，实际 {} 条",
        emitted.len()
    );
    assert_spawn_particle(&emitted[0], "bong:dugu_dark_green_mist", Some(28));
    match &emitted[0].payload {
        VfxEventPayloadV1::SpawnParticle { strength, .. } => {
            assert_eq!(
                *strength,
                Some(1.5),
                "遮蔽强度 9.0 应被钳到 1.5（视觉浓度上限），实际 {strength:?}"
            );
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

/// 倒蚀：爆发线束于事件自带爆心（不依赖 Position），count 随受害者数封顶 48。
#[test]
fn dugu_v2_reverse_emits_burst_at_event_center() {
    use crate::combat::dugu_v2::events::DuguSkillId;
    use valence::prelude::DVec3;
    let mut app = setup_dugu_v2_visual_app();
    let caster = app.world_mut().spawn_empty().id();
    app.world_mut().send_event(ReverseTriggeredEvent {
        caster,
        affected_targets: 3,
        burst_damage: 12.0,
        returned_zone_qi: 6.0,
        juebi_delay_ticks: None,
        tick: 12,
        center: DVec3::new(10.0, 65.0, -4.0),
        visual: dugu_v2_visual(DuguSkillId::Reverse),
    });
    app.update();
    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        1,
        "倒蚀无 Position 也必须出粒子（事件自带爆心）"
    );
    assert_spawn_particle(&emitted[0], "bong:dugu_reverse_burst", Some(36));
    assert_spawn_particle_origin(&emitted[0], [10.0, 65.0, -4.0]);
}

/// 五招 particle_id 全部落在 client DuguV2VfxPlayer.EVENT_IDS 的三个注册项内
///（谁改 visual_for 忘改 client 注册，此测撞红）。
#[test]
fn dugu_v2_all_skill_particle_ids_are_client_registered() {
    use crate::combat::dugu_v2::events::DuguSkillId;
    const CLIENT_REGISTERED: [&str; 3] = [
        "bong:dugu_taint_pulse",
        "bong:dugu_dark_green_mist",
        "bong:dugu_reverse_burst",
    ];
    for skill in DuguSkillId::ALL {
        let visual = dugu_v2_visual(skill);
        assert!(
            CLIENT_REGISTERED.contains(&visual.particle_id),
            "{skill:?} 的 particle_id {} 未在 client DuguV2VfxPlayer.EVENT_IDS 注册，\
             VfxRegistry 查表将 miss、粒子静默丢弃",
            visual.particle_id
        );
    }
}

// ========== 绝灵涡流 woliu v1（emit_woliu_v1_vortex_visual_triggers） ==========

fn setup_woliu_v1_visual_app(tick: u64) -> App {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick });
    app.add_event::<VortexBackfireEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_woliu_v1_vortex_visual_triggers);
    app
}

fn spawn_v1_field(app: &mut App, caster: Entity, cast_at_tick: u64) -> Entity {
    use valence::prelude::DVec3;
    let field = VortexField {
        center: DVec3::new(0.0, 64.0, 0.0),
        radius: 8.0,
        delta: 4.0,
        cast_at_tick,
        maintain_max_ticks: 1200,
        caster,
        env_qi_at_cast: 50.0,
        last_maintain_tick: cast_at_tick,
    };
    app.world_mut().entity_mut(caster).insert(field);
    caster
}

/// field 出现 → 专属开涡起手式动画 + 开涡吸入环；同 field 下一 tick 不重发。
/// plan-skill-anim-fidelity-v1 P3：`woliu.vortex` 借用解除——开涡不再复用 v2
/// 涡旋站桩 `vortex_spiral_stance`（20t 与瞬发 [6,12] 时长域不符且与
/// woliu.heart 撞形），改播专属 `bong:woliu_vortex_cast`。
#[test]
fn woliu_v1_field_appear_emits_stance_anim_and_open_burst_once() {
    let mut app = setup_woliu_v1_visual_app(101);
    let caster = spawn_player(&mut app, "Caster", [0.0, 64.0, 0.0]);
    spawn_v1_field(&mut app, caster, 101);

    app.update();
    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        2,
        "开涡应发 1 动画 + 1 粒子，实际 {} 条",
        emitted.len()
    );
    assert_play_anim(&emitted[0], ANIM_WOLIU_V1_STANCE, WOLIU_PRIORITY);
    // P3 专属 id 拼写 pin + 去复用回归锁（常量回指旧借用会静默复现撞形）。
    assert_eq!(ANIM_WOLIU_V1_STANCE, "bong:woliu_vortex_cast");
    assert_ne!(
        ANIM_WOLIU_V1_STANCE, "bong:vortex_spiral_stance",
        "去复用回归锁：woliu.vortex 开涡不得回退借 v2 涡旋站桩"
    );
    assert_spawn_particle(&emitted[1], VFX_WOLIU_V1_FIELD_OPEN, Some(24));

    // 下一 tick（非 ambient 周期）：不得重发开涡。
    app.world_mut().resource_mut::<CombatClock>().tick = 102;
    app.update();
    assert!(
        drain_vfx(&mut app).is_empty(),
        "field 存续第 2 tick（非周期点）不应重发任何粒子"
    );
}

/// 存续满一个周期（20 tick）→ 低频涡环；非周期 tick 静默。
#[test]
fn woliu_v1_field_sustain_emits_ambient_ring_on_period() {
    let mut app = setup_woliu_v1_visual_app(101);
    let caster = spawn_player(&mut app, "Caster", [0.0, 64.0, 0.0]);
    spawn_v1_field(&mut app, caster, 101);
    app.update();
    drain_vfx(&mut app); // 吃掉开涡

    app.world_mut().resource_mut::<CombatClock>().tick = 101 + WOLIU_V1_AMBIENT_PERIOD_TICKS;
    app.update();
    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 1, "存续满 20 tick 应发 1 条低频涡环");
    assert_spawn_particle(&emitted[0], VFX_WOLIU_V1_FIELD_AMBIENT, Some(12));
}

/// field 移除后重新开涡 → 再次发开涡（lifecycle 状态正确清退）。
#[test]
fn woliu_v1_field_reopen_after_removal_emits_open_again() {
    let mut app = setup_woliu_v1_visual_app(101);
    let caster = spawn_player(&mut app, "Caster", [0.0, 64.0, 0.0]);
    spawn_v1_field(&mut app, caster, 101);
    app.update();
    drain_vfx(&mut app);

    app.world_mut().entity_mut(caster).remove::<VortexField>();
    app.world_mut().resource_mut::<CombatClock>().tick = 105;
    app.update();
    assert!(drain_vfx(&mut app).is_empty(), "关涡后不应再发粒子");

    spawn_v1_field(&mut app, caster, 110);
    app.world_mut().resource_mut::<CombatClock>().tick = 110;
    app.update();
    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 2, "重新开涡应再次发动画 + 开涡粒子");
    assert_spawn_particle(&emitted[1], VFX_WOLIU_V1_FIELD_OPEN, Some(24));
}

/// 反噬 → 断经暗红爆裂于施法者位置。
#[test]
fn woliu_v1_backfire_emits_burst_at_caster() {
    use crate::combat::woliu::BackfireCause;
    use crate::cultivation::components::MeridianId;
    let mut app = setup_woliu_v1_visual_app(200);
    let caster = spawn_player(&mut app, "Caster", [6.0, 64.0, 6.0]);
    app.world_mut().send_event(VortexBackfireEvent {
        caster,
        cause: BackfireCause::ExceedMaintainMax,
        meridian_severed: MeridianId::Lung,
        tick: 200,
        env_qi: 10.0,
        delta: 4.0,
        resisted: false,
    });
    app.update();
    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        1,
        "反噬应发 1 条爆裂粒子（断经负反馈），实际 {} 条",
        emitted.len()
    );
    assert_spawn_particle(&emitted[0], VFX_WOLIU_V1_BACKFIRE, Some(30));
    assert_spawn_particle_color(&emitted[0], "#B84A3F");
    assert_spawn_particle_origin(&emitted[0], [6.0, 64.0, 6.0]);
}

/// 自蕴施法者断 Position → 静默 skip（无位置可放雾，无回落来源）。
#[test]
fn dugu_v2_self_cure_skips_when_caster_positionless() {
    use crate::combat::dugu_v2::events::DuguSkillId;
    let mut app = setup_dugu_v2_visual_app();
    let caster = app.world_mut().spawn_empty().id();
    app.world_mut().send_event(SelfCureProgressEvent {
        caster,
        hours_used: 2.5,
        daily_hours_after: 2.5,
        gain_percent: 10.0,
        insidious_color_percent: 5.0,
        morphology_percent: 1.0,
        self_revealed: false,
        tick: 9,
        visual: dugu_v2_visual(DuguSkillId::SelfCure),
    });
    app.update();
    assert!(
        drain_vfx(&mut app).is_empty(),
        "自蕴施法者无 Position 应静默 skip，不得以默认坐标发粒子"
    );
}

/// 神识遮蔽施法者断 Position → 静默 skip。
#[test]
fn dugu_v2_shroud_skips_when_caster_positionless() {
    use crate::combat::dugu_v2::events::DuguSkillId;
    let mut app = setup_dugu_v2_visual_app();
    let caster = app.world_mut().spawn_empty().id();
    app.world_mut().send_event(ShroudActivatedEvent {
        caster,
        strength: 1.0,
        expires_at_tick: 600,
        tick: 11,
        visual: dugu_v2_visual(DuguSkillId::Shroud),
    });
    app.update();
    assert!(
        drain_vfx(&mut app).is_empty(),
        "遮蔽施法者无 Position 应静默 skip，不得以默认坐标发粒子"
    );
}

/// 侵染 target 与 caster 双双断 Position → 静默 skip（回落链穷尽）。
#[test]
fn dugu_v2_penetrate_skips_when_both_positionless() {
    use crate::combat::dugu_v2::events::{DuguSkillId, TaintTier};
    let mut app = setup_dugu_v2_visual_app();
    let caster = app.world_mut().spawn_empty().id();
    let target = app.world_mut().spawn_empty().id();
    app.world_mut().send_event(PenetrateChainEvent {
        caster,
        target,
        taint_tier: TaintTier::Temporary,
        multiplier: 1.5,
        affected_targets: 2,
        permanent_decay_rate_per_min: 0.0,
        reveal_probability: 0.2,
        returned_zone_qi: 5.0,
        tick: 10,
        visual: dugu_v2_visual(DuguSkillId::Penetrate),
    });
    app.update();
    assert!(
        drain_vfx(&mut app).is_empty(),
        "侵染 target 与 caster 均无 Position 时应静默 skip（回落链穷尽）"
    );
}

/// 反噬 caster 断 Position 但领域仍在 → 回落到 field.center（重要负反馈不静默丢）。
#[test]
fn woliu_v1_backfire_falls_back_to_field_center_when_caster_positionless() {
    use crate::combat::woliu::BackfireCause;
    use crate::cultivation::components::MeridianId;
    let mut app = setup_woliu_v1_visual_app(200);
    let caster = app.world_mut().spawn_empty().id();
    spawn_v1_field(&mut app, caster, 200);
    app.update();
    drain_vfx(&mut app); // 吃掉开涡（无 player 组件时动画 skip、粒子仍发）

    app.world_mut().send_event(VortexBackfireEvent {
        caster,
        cause: BackfireCause::ExceedMaintainMax,
        meridian_severed: MeridianId::Lung,
        tick: 201,
        env_qi: 10.0,
        delta: 4.0,
        resisted: false,
    });
    app.world_mut().resource_mut::<CombatClock>().tick = 201;
    app.update();
    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        1,
        "caster 无 Position 但领域仍在时，反噬粒子应回落 field.center，实际 {} 条",
        emitted.len()
    );
    assert_spawn_particle(&emitted[0], VFX_WOLIU_V1_BACKFIRE, Some(30));
    assert_spawn_particle_origin(&emitted[0], [0.0, 64.0, 0.0]);
}

/// 反噬 caster 无 Position 且领域已散 → 回落链穷尽，静默 skip。
#[test]
fn woliu_v1_backfire_skips_when_positionless_and_no_field() {
    use crate::combat::woliu::BackfireCause;
    use crate::cultivation::components::MeridianId;
    let mut app = setup_woliu_v1_visual_app(200);
    let caster = app.world_mut().spawn_empty().id();
    app.world_mut().send_event(VortexBackfireEvent {
        caster,
        cause: BackfireCause::EnvQiTooLow,
        meridian_severed: MeridianId::Lung,
        tick: 200,
        env_qi: 1.0,
        delta: 4.0,
        resisted: false,
    });
    app.update();
    assert!(
        drain_vfx(&mut app).is_empty(),
        "caster 无 Position 且无领域可回落时应静默 skip"
    );
}

/// server 侧三个 v1 event_id 字面值锁死——与 client VortexSpiralPlayer.WOLIU_V1_* 逐字对齐
///（对照 dugu_v2_all_skill_particle_ids_are_client_registered 的同款防线）。
#[test]
fn woliu_v1_particle_ids_match_client_registration() {
    assert_eq!(
        VFX_WOLIU_V1_FIELD_OPEN, "bong:woliu_vortex_field",
        "开涡 event_id 必须与 client VortexSpiralPlayer.WOLIU_V1_FIELD_OPEN 逐字一致，否则粒子静默丢弃"
    );
    assert_eq!(
        VFX_WOLIU_V1_FIELD_AMBIENT, "bong:woliu_vortex_field_ambient",
        "存续 event_id 必须与 client VortexSpiralPlayer.WOLIU_V1_FIELD_AMBIENT 逐字一致"
    );
    assert_eq!(
        VFX_WOLIU_V1_BACKFIRE, "bong:woliu_vortex_backfire",
        "反噬 event_id 必须与 client VortexSpiralPlayer.WOLIU_V1_BACKFIRE 逐字一致"
    );
}

// ── plan-skill-av-relink-v1 P3 —— P1 接线 emit pin：stance adapter ────────────

fn setup_stance_app() -> App {
    let mut app = App::new();
    app.add_event::<TechniqueLearnedEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_technique_learned_stance_triggers);
    app
}

fn send_technique_learned(app: &mut App, player: valence::prelude::Entity, technique_id: &str) {
    use crate::cultivation::technique_scroll::LearnSource;
    app.world_mut().send_event(TechniqueLearnedEvent {
        player,
        technique_id: technique_id.to_string(),
        source: LearnSource::Scroll {
            item_id: "technique_scroll_test".to_string(),
        },
    });
}

/// happy path：两条生产可达的 stance 接线逐条 pin——每个映射前缀习得时恰发
/// 一条携正确 anim_id 的 PlayAnim（technique_id 全部取 TechniqueRegistry
/// 真实条目，且均有真实卷轴内容可授予）。
#[test]
fn technique_learned_emits_mapped_stance_animation_for_each_wired_family() {
    let cases = [
        ("woliu.vortex", ANIM_STANCE_WOLIU),
        ("zhenmai.parry", ANIM_STANCE_ZHENMAI),
    ];
    for (technique_id, expected_anim) in cases {
        let mut app = setup_stance_app();
        let player = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        send_technique_learned(&mut app, player, technique_id);
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "习得 `{technique_id}` 应恰发一条架势动画，实际 {emitted:?}"
        );
        assert_play_anim(&emitted[0], expected_anim, STORY_PRIORITY);
    }
}

/// P6：架势亮相是**一次性单发**动画，其 fade_in 必须显式且 ≥ 3 tick。
///
/// 与两段式 release 的「热交接宜短」相反（conventions §14.2 硬约束 3），架势
/// 亮相是**从 vanilla 冷起手**——按 conventions §2.7「fade-in ≥ 3 tick」适用，
/// 差距大需要淡入铺垫。P6 把两个架势资产由 `isLoop:true` 改为一次性亮相
/// （§8.1 #2 第 4 条），发射侧本就是单发 + `Some(3)`，此前无任何用例锁住该值，
/// 本用例补上：既防 fade_in 被悄悄改小造成冷起手闪跳，也防被改成 None 回落默认。
#[test]
fn stance_reveal_play_anim_carries_explicit_cold_start_fade_in() {
    for technique_id in ["woliu.vortex", "zhenmai.parry"] {
        let mut app = setup_stance_app();
        let player = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        send_technique_learned(&mut app, player, technique_id);
        app.update();

        let emitted = drain_vfx(&mut app);
        let (_, fade_in) = play_anim_priority_and_fade_in(&emitted[0]);
        let fade_in = fade_in.expect(
            "架势亮相必须显式带 fade_in——None 会回落客户端默认值，冷起手的淡入预算不可依赖默认",
        );
        assert!(
            fade_in >= 3,
            "习得 `{technique_id}` 的架势亮相 fade_in={fade_in} < 3：\
             从 vanilla 冷起手到架势姿态差距大，淡入不足会闪跳\
             （conventions §2.7）"
        );
    }
}

/// 架势动画 target_player 必须是习得者自己的 uuid（不是发给别人播）。
#[test]
fn stance_animation_targets_the_learner_uuid() {
    let mut app = setup_stance_app();
    let player = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let expected_uuid = app
        .world()
        .get::<UniqueId>(player)
        .expect("mock client should carry UniqueId")
        .0
        .to_string();
    send_technique_learned(&mut app, player, "woliu.vortex");
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(emitted.len(), 1);
    match &emitted[0].payload {
        VfxEventPayloadV1::PlayAnim { target_player, .. } => assert_eq!(
            target_player, &expected_uuid,
            "架势动画应发给习得者本人（target_player = 习得者 uuid）"
        ),
        other => panic!("expected PlayAnim, got {other:?}"),
    }
}

/// 错误分支：无映射前缀不发——sword/movement 等非架势流派、zhenfa
/// （stance_zhenfa 维持 report-only）、以及对抗审查后降级 report-only 的
/// dugu / dugu_poison / baomai / tuike 四族（无生产可达习得路径不预置映射，
/// 见 plan P1 表）全部静默。习得内容落地重新接线时本清单同步移出对应条目。
#[test]
fn technique_learned_with_unmapped_family_does_not_emit_stance() {
    for technique_id in [
        "sword.cleave",
        "movement.dash",
        "dugu.shoot_needle",
        "dugu.infuse_poison",
        "baomai.full_power_charge",
        "tuike.don",
        "zhenfa.ward",
        "",
    ] {
        let mut app = setup_stance_app();
        let player = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        send_technique_learned(&mut app, player, technique_id);
        app.update();

        let emitted = drain_vfx(&mut app);
        assert!(
            emitted.is_empty(),
            "无映射 technique `{technique_id}` 不应发架势动画，实际 {emitted:?}"
        );
    }
}

/// 注：`"woliu"`（无 `.` 的裸前缀）走 `split('.').next()` 仍解析出 `woliu`——
/// 该形态在 TechniqueRegistry 中不存在，仅锁 split 语义不背离预期。
#[test]
fn bare_family_prefix_without_dot_still_maps_by_split_semantics() {
    assert_eq!(
        stance_anim_for_technique("woliu"),
        Some(ANIM_STANCE_WOLIU),
        "split('.').next() 对无点 id 返回整串——语义变更需同步本 pin 与映射注释"
    );
    assert_eq!(stance_anim_for_technique(""), None, "空串必须映射为 None");
}

/// 状态转换分支：习得者实体缺 Position/UniqueId（离线/已清理）时静默不发。
#[test]
fn technique_learned_for_entity_without_anim_target_does_not_emit() {
    let mut app = setup_stance_app();
    let ghost = app.world_mut().spawn_empty().id();
    send_technique_learned(&mut app, ghost, "woliu.vortex");
    app.update();

    assert!(
        drain_vfx(&mut app).is_empty(),
        "缺 Position/UniqueId 的实体不应发架势动画"
    );
}

/// 重复触发语义：adapter 对事件 1:1 发射、无隐藏去重状态——生产幂等门在上游
/// （`learn_technique_if_allowed` 对已习得功法返回 AlreadyKnown、不发
/// TechniqueLearnedEvent，见 technique_scroll 测试），本测试锁 adapter 自身
/// 不做去重的分工契约。
#[test]
fn repeated_technique_learned_events_emit_one_stance_anim_each() {
    let mut app = setup_stance_app();
    let player = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    send_technique_learned(&mut app, player, "woliu.vortex");
    send_technique_learned(&mut app, player, "woliu.vortex");
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        2,
        "adapter 按事件 1:1 发射（去重责任在上游 AlreadyKnown 门），实际 {emitted:?}"
    );
    for request in &emitted {
        assert_play_anim(request, ANIM_STANCE_WOLIU, STORY_PRIORITY);
    }
}

// ── plan-skill-av-relink-v1 P3 —— P1 接线 emit pin：forge_hammer adapter ─────

fn setup_forge_anim_app(
    current_step: crate::forge::session::ForgeStep,
    caster_has_anim_target: bool,
) -> (App, crate::forge::session::ForgeSessionId) {
    use crate::forge::session::ForgeSession;
    let mut app = App::new();
    app.add_event::<TemperingHit>();
    app.add_event::<VfxEventRequest>();
    let caster = if caster_has_anim_target {
        spawn_player(&mut app, "Smith", [0.0, 64.0, 0.0])
    } else {
        app.world_mut().spawn_empty().id()
    };
    let station = app.world_mut().spawn_empty().id();
    let mut sessions = ForgeSessions::default();
    let session_id = sessions.allocate_id();
    let mut session = ForgeSession::new(session_id, "test_blueprint".to_string(), station, caster);
    session.current_step = current_step;
    sessions.insert(session);
    app.insert_resource(sessions);
    app.add_systems(Update, emit_forge_tempering_animation_triggers);
    (app, session_id)
}

fn send_tempering_hit(app: &mut App, session: crate::forge::session::ForgeSessionId) {
    use crate::forge::blueprint::TemperBeat;
    app.world_mut().send_event(TemperingHit {
        session,
        beat: TemperBeat::Light,
        ticks_remaining: 3,
    });
}

/// happy path：Tempering 步的按键命中 → 恰发一条 forge_hammer 抡锤动画。
#[test]
fn tempering_hit_in_tempering_step_emits_forge_hammer() {
    let (mut app, session_id) = setup_forge_anim_app(ForgeStep::Tempering, true);
    send_tempering_hit(&mut app, session_id);
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        1,
        "Tempering 步命中应恰发一条抡锤动画，实际 {emitted:?}"
    );
    assert_play_anim(&emitted[0], ANIM_FORGE_HAMMER, COMBAT_PRIORITY);
}

/// 重复触发语义：J/K/L 每次命中都是独立抡锤——两击两动画（1:1，无去重）。
#[test]
fn each_tempering_hit_emits_its_own_forge_hammer_swing() {
    let (mut app, session_id) = setup_forge_anim_app(ForgeStep::Tempering, true);
    send_tempering_hit(&mut app, session_id);
    send_tempering_hit(&mut app, session_id);
    app.update();

    let emitted = drain_vfx(&mut app);
    assert_eq!(
        emitted.len(),
        2,
        "每次淬炼命中各配一次抡锤动画，实际 {emitted:?}"
    );
    for request in &emitted {
        assert_play_anim(request, ANIM_FORGE_HAMMER, COMBAT_PRIORITY);
    }
}

/// 错误分支：非 Tempering 步的 stale 按键不发动画（镜像 forge 模块步骤门）。
#[test]
fn tempering_hit_outside_tempering_step_does_not_emit() {
    for step in [
        ForgeStep::Billet,
        ForgeStep::Inscription,
        ForgeStep::Consecration,
        ForgeStep::Done,
    ] {
        let (mut app, session_id) = setup_forge_anim_app(step, true);
        send_tempering_hit(&mut app, session_id);
        app.update();

        let emitted = drain_vfx(&mut app);
        assert!(
            emitted.is_empty(),
            "{step:?} 步的 stale 淬炼按键不应发抡锤动画，实际 {emitted:?}"
        );
    }
}

/// 错误分支：session 已结算/弃疗（表中不存在）时不发。
#[test]
fn tempering_hit_for_missing_session_does_not_emit() {
    use crate::forge::session::ForgeSessionId;
    let (mut app, _live_session) = setup_forge_anim_app(ForgeStep::Tempering, true);
    send_tempering_hit(&mut app, ForgeSessionId(9999));
    app.update();

    assert!(
        drain_vfx(&mut app).is_empty(),
        "不存在的 session 的命中不应发抡锤动画"
    );
}

/// 状态转换分支：caster 实体缺 Position/UniqueId（离线/已清理）时静默不发。
#[test]
fn tempering_hit_for_caster_without_anim_target_does_not_emit() {
    let (mut app, session_id) = setup_forge_anim_app(ForgeStep::Tempering, false);
    send_tempering_hit(&mut app, session_id);
    app.update();

    assert!(
        drain_vfx(&mut app).is_empty(),
        "caster 缺 Position/UniqueId 时不应发抡锤动画"
    );
}

// ── plan-skill-av-relink-v1 P3 —— fist_punch_left 连击交替序列 ────────────────

fn setup_fist_combo_app() -> App {
    let mut app = App::new();
    app.insert_resource(CombatClock::default());
    app.add_event::<AttackIntent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_attack_animation_triggers);
    app
}

/// 在 `tick` 时刻发一记拳（默认 Blunt/Melee），断言恰发一条 PlayAnim 并返回其 anim_id。
fn punch_anim_at_tick(
    app: &mut App,
    attacker: valence::prelude::Entity,
    tick: u64,
    wound_kind: WoundKind,
) -> String {
    app.world_mut().resource_mut::<CombatClock>().tick = tick;
    app.world_mut().send_event(AttackIntent {
        attacker,
        target: None,
        issued_at_tick: tick,
        reach: AttackReach::new(1.0, 0.0),
        qi_invest: 0.0,
        wound_kind,
        source: AttackSource::Melee,
        debug_command: None,
    });
    app.update();
    let emitted = drain_vfx(app);
    assert_eq!(
        emitted.len(),
        1,
        "tick {tick} 的攻击应恰发一条动画，实际 {emitted:?}"
    );
    match &emitted[0].payload {
        VfxEventPayloadV1::PlayAnim { anim_id, .. } => anim_id.clone(),
        other => panic!("expected PlayAnim, got {other:?}"),
    }
}

/// happy path：连续空手攻击 right → left → right 交替。
#[test]
fn unarmed_punches_alternate_right_left_right() {
    let mut app = setup_fist_combo_app();
    let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

    assert_eq!(
        punch_anim_at_tick(&mut app, attacker, 10, WoundKind::Blunt),
        ANIM_FIST_PUNCH_RIGHT,
        "空手连击必须右拳起手"
    );
    assert_eq!(
        punch_anim_at_tick(&mut app, attacker, 11, WoundKind::Blunt),
        ANIM_FIST_PUNCH_LEFT,
        "第二拳应交替为左拳"
    );
    assert_eq!(
        punch_anim_at_tick(&mut app, attacker, 12, WoundKind::Blunt),
        ANIM_FIST_PUNCH_RIGHT,
        "第三拳应交替回右拳"
    );
}

/// Concussion 与 Blunt 同属拳击解析（`attack_anim_for_wound_kind`），
/// 同一连击链内交替不因 wound_kind 在两者间切换而中断。
#[test]
fn concussion_wound_kind_participates_in_same_fist_combo() {
    let mut app = setup_fist_combo_app();
    let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

    assert_eq!(
        punch_anim_at_tick(&mut app, attacker, 10, WoundKind::Blunt),
        ANIM_FIST_PUNCH_RIGHT
    );
    assert_eq!(
        punch_anim_at_tick(&mut app, attacker, 11, WoundKind::Concussion),
        ANIM_FIST_PUNCH_LEFT,
        "Concussion 拳与 Blunt 拳共享同一交替链"
    );
}

/// 边界（off-by-one）：两拳间隔恰等于 FIST_COMBO_RESET_TICKS 时连击**未**中断，
/// 仍交替；超过一个 tick 才复位（行为语义引用常数，不锁字面值）。
#[test]
fn fist_combo_continues_at_reset_boundary_and_resets_past_it() {
    let mut app = setup_fist_combo_app();
    let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

    let start = 100_u64;
    assert_eq!(
        punch_anim_at_tick(&mut app, attacker, start, WoundKind::Blunt),
        ANIM_FIST_PUNCH_RIGHT
    );
    let boundary = start + FIST_COMBO_RESET_TICKS;
    assert_eq!(
        punch_anim_at_tick(&mut app, attacker, boundary, WoundKind::Blunt),
        ANIM_FIST_PUNCH_LEFT,
        "间隔恰为 FIST_COMBO_RESET_TICKS 时连击应延续（> 才算超时）"
    );

    // 此刻交替态为"下一拳右"——若超时复位不生效，下一拳同样是右拳，无从分辨。
    // 因此先补一拳把交替态推到"下一拳左"，再验证超时后回到右拳起手。
    assert_eq!(
        punch_anim_at_tick(&mut app, attacker, boundary + 1, WoundKind::Blunt),
        ANIM_FIST_PUNCH_RIGHT
    );
    let past_timeout = boundary + 1 + FIST_COMBO_RESET_TICKS + 1;
    assert_eq!(
        punch_anim_at_tick(&mut app, attacker, past_timeout, WoundKind::Blunt),
        ANIM_FIST_PUNCH_RIGHT,
        "间隔超过 FIST_COMBO_RESET_TICKS 后连击中断，必须右拳重新起手（否则应为左拳）"
    );
}

/// 持械分支：携 Weapon component（含 Staff/Fist 类武器）不参与交替，恒右拳。
#[test]
fn armed_attacker_never_alternates_to_left_punch() {
    use crate::combat::weapon::{EquipSlot, WeaponKind};
    for weapon_kind in [WeaponKind::Staff, WeaponKind::Fist] {
        let mut app = setup_fist_combo_app();
        let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        app.world_mut().entity_mut(attacker).insert(Weapon {
            slot: EquipSlot::MainHand,
            instance_id: 1,
            template_id: "test_blunt_weapon".to_string(),
            weapon_kind,
            base_attack: 1.0,
            quality_tier: 0,
            durability: 10.0,
            durability_max: 10.0,
        });

        for tick in 10..13 {
            assert_eq!(
                punch_anim_at_tick(&mut app, attacker, tick, WoundKind::Blunt),
                ANIM_FIST_PUNCH_RIGHT,
                "持械（{weapon_kind:?}）连续攻击不参与交替，恒右拳"
            );
        }
    }
}

/// 状态转换：持械攻击既不推进也不复位空手连击链——右拳后持械打两下、
/// 超时窗口内卸下再空手，链条视同延续出左拳（armed 分支不触碰交替态）。
#[test]
fn armed_attacks_do_not_touch_unarmed_combo_state() {
    use crate::combat::weapon::{EquipSlot, WeaponKind};
    let mut app = setup_fist_combo_app();
    let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

    assert_eq!(
        punch_anim_at_tick(&mut app, attacker, 10, WoundKind::Blunt),
        ANIM_FIST_PUNCH_RIGHT,
        "空手首击右拳起手"
    );

    app.world_mut().entity_mut(attacker).insert(Weapon {
        slot: EquipSlot::MainHand,
        instance_id: 1,
        template_id: "test_blunt_weapon".to_string(),
        weapon_kind: WeaponKind::Staff,
        base_attack: 1.0,
        quality_tier: 0,
        durability: 10.0,
        durability_max: 10.0,
    });
    for tick in [12, 14] {
        assert_eq!(
            punch_anim_at_tick(&mut app, attacker, tick, WoundKind::Blunt),
            ANIM_FIST_PUNCH_RIGHT,
            "持械攻击恒右拳（tick {tick}）"
        );
    }

    app.world_mut().entity_mut(attacker).remove::<Weapon>();
    assert_eq!(
        punch_anim_at_tick(&mut app, attacker, 16, WoundKind::Blunt),
        ANIM_FIST_PUNCH_LEFT,
        "超时窗口内卸下武器再空手：持械期间不触碰交替态，链条延续应出左拳"
    );
}

/// 玩家隔离：两名玩家交错互不干扰，各自独立 right → left 交替。
#[test]
fn fist_combo_state_is_isolated_per_player() {
    let mut app = setup_fist_combo_app();
    let alice = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let bob = spawn_player(&mut app, "Bob", [4.0, 64.0, 0.0]);

    assert_eq!(
        punch_anim_at_tick(&mut app, alice, 10, WoundKind::Blunt),
        ANIM_FIST_PUNCH_RIGHT,
        "Alice 首拳右起手"
    );
    assert_eq!(
        punch_anim_at_tick(&mut app, bob, 11, WoundKind::Blunt),
        ANIM_FIST_PUNCH_RIGHT,
        "Bob 首拳不受 Alice 交替态影响，同样右起手"
    );
    assert_eq!(
        punch_anim_at_tick(&mut app, alice, 12, WoundKind::Blunt),
        ANIM_FIST_PUNCH_LEFT,
        "Alice 第二拳交替左拳"
    );
    assert_eq!(
        punch_anim_at_tick(&mut app, bob, 13, WoundKind::Blunt),
        ANIM_FIST_PUNCH_LEFT,
        "Bob 第二拳按自己的链交替左拳"
    );
}

/// 陈旧连击态剪枝（`Local` map 无法从 App 外观测，函数级锁语义）：超出连击
/// 窗口的条目被驱逐、窗口内条目保留，map 不随历史攻击者无界增长；被剪条目
/// 再出拳右拳起手——与未剪时的超时复位行为完全等价。
#[test]
fn stale_fist_combo_entries_are_pruned_and_pruning_preserves_reset_semantics() {
    let mut combo: HashMap<Entity, FistComboState> = HashMap::new();
    let stale = Entity::from_raw(1);
    let boundary = Entity::from_raw(2);
    let active = Entity::from_raw(3);
    let start = 100_u64;
    let now = start + FIST_COMBO_RESET_TICKS + 1;
    next_fist_punch_anim(&mut combo, stale, start);
    next_fist_punch_anim(&mut combo, boundary, start + 1);
    next_fist_punch_anim(&mut combo, active, now);

    prune_stale_fist_combo(&mut combo, now);

    assert!(
        !combo.contains_key(&stale),
        "间隔超过 FIST_COMBO_RESET_TICKS 的陈旧条目应被剪除\
         （否则 map 随历史攻击者无界增长慢泄漏）"
    );
    assert!(
        combo.contains_key(&boundary),
        "间隔恰为 FIST_COMBO_RESET_TICKS 的条目仍在连击窗口内\
         （剪枝 <= 保留须与超时判定 > 对齐），不得误剪活链"
    );
    assert!(combo.contains_key(&active), "刚出拳的活跃条目必须保留");
    assert_eq!(combo.len(), 2, "剪枝后 map 只含窗口内攻击者");

    // 行为等价：被剪条目再次出拳与"未剪但超时复位"一致——右拳起手。
    assert_eq!(
        next_fist_punch_anim(&mut combo, stale, now + 1),
        ANIM_FIST_PUNCH_RIGHT,
        "被剪条目重建后应右拳起手，与超时复位语义一致（剪枝不得改变可观察行为）"
    );
}

/// 非拳击 wound kind（Cut/Pierce/Burn）不落入交替逻辑，空手也不产出左拳。
#[test]
fn non_fist_wound_kinds_never_emit_left_punch() {
    let mut app = setup_fist_combo_app();
    let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

    for (tick, wound_kind, expected) in [
        (10_u64, WoundKind::Cut, ANIM_SWORD_SLASH_DOWN),
        (11, WoundKind::Pierce, ANIM_SWORD_STAB),
        (12, WoundKind::Burn, ANIM_PALM_STRIKE),
    ] {
        assert_eq!(
            punch_anim_at_tick(&mut app, attacker, tick, wound_kind),
            expected,
            "{wound_kind:?} 应走原 wound-kind 动画映射，与拳击交替无关"
        );
    }
}
