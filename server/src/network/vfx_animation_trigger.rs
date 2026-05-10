//! Gameplay domain events -> `play_anim` adapters for `bong:vfx_event`.
//!
//! The transport, JSON serialization, radius filtering and client delivery stay in
//! `vfx_event_emit`; this module only decides which first-party animation id should
//! represent an already-authoritative server event.

use valence::prelude::{EventReader, EventWriter, Position, Query, UniqueId};

use crate::botany::components::HarvestTerminalEvent;
use crate::botany::lifecycle::botany_quality_color;
use crate::combat::components::WoundKind;
use crate::combat::events::{AttackIntent, AttackSource, CombatEvent, DefenseIntent};
use crate::combat::woliu_v2::{VortexCastEvent, WoliuSkillId};
use crate::cultivation::breakthrough::BreakthroughOutcome;
use crate::cultivation::tribulation::{TribulationAnnounce, TribulationFailed};
use crate::lingtian::events::{
    DrainQiCompleted, HarvestCompleted, PlantingCompleted, ReplenishCompleted, TillCompleted,
};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::vfx_event::VfxEventPayloadV1;

const ANIM_SWORD_SLASH_DOWN: &str = "bong:sword_slash_down";
const ANIM_SWORD_STAB: &str = "bong:sword_stab";
const ANIM_FIST_PUNCH_RIGHT: &str = "bong:fist_punch_right";
const ANIM_PALM_STRIKE: &str = "bong:palm_strike";
const ANIM_PARRY_BLOCK: &str = "bong:parry_block";
const ANIM_HURT_STAGGER: &str = "bong:hurt_stagger";
const ANIM_BREAKTHROUGH_YINQI: &str = "bong:breakthrough_yinqi";
const ANIM_BREAKTHROUGH_NINGMAI: &str = "bong:breakthrough_ningmai";
const ANIM_BREAKTHROUGH_GUYUAN: &str = "bong:breakthrough_guyuan";
const ANIM_BREAKTHROUGH_TONGLING: &str = "bong:breakthrough_tongling";
const ANIM_TRIBULATION_BRACE: &str = "bong:tribulation_brace";
const ANIM_HARVEST_CROUCH: &str = "bong:harvest_crouch";
const ANIM_LINGTIAN_TILL: &str = "bong:lingtian_till";
const BOTANY_HARVEST_VFX: &str = "bong:botany_harvest";
const LINGTIAN_TILL_VFX: &str = "bong:lingtian_till";
const LINGTIAN_PLANT_VFX: &str = "bong:lingtian_plant";
const LINGTIAN_REPLENISH_VFX: &str = "bong:lingtian_replenish";
const LINGTIAN_HARVEST_VFX: &str = "bong:lingtian_harvest";
const LINGTIAN_DRAIN_VFX: &str = "bong:lingtian_drain";
const WOLIU_PRIORITY: u16 = 1300;

const COMBAT_PRIORITY: u16 = 1000;
const HIT_RECOIL_PRIORITY: u16 = 2000;
const STORY_PRIORITY: u16 = 3000;

type PlayerAnimTargetItem<'a> = (&'a Position, &'a UniqueId);
type PlayerAnimTargetFilter = ();

/// Combat intent -> attacker action animation.
///
/// `AttackSource::BurstMeridian` is intentionally skipped: that skill already emits its
/// bespoke `bong:beng_quan` animation in `cultivation::burst_meridian`.
pub fn emit_attack_animation_triggers(
    mut intents: EventReader<AttackIntent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for intent in intents.read() {
        if intent.source == AttackSource::BurstMeridian {
            continue;
        }
        let anim_id = attack_anim_for_wound_kind(intent.wound_kind);
        emit_play_for_entity(
            intent.attacker,
            anim_id,
            COMBAT_PRIORITY,
            Some(2),
            &players,
            &mut vfx_events,
        );
    }
}

/// Defense intent -> guard pose animation.
pub fn emit_defense_animation_triggers(
    mut defenses: EventReader<DefenseIntent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for defense in defenses.read() {
        emit_play_for_entity(
            defense.defender,
            ANIM_PARRY_BLOCK,
            COMBAT_PRIORITY,
            Some(1),
            &players,
            &mut vfx_events,
        );
    }
}

/// Resolved hit -> target recoil animation.
pub fn emit_hit_recoil_animation_triggers(
    mut events: EventReader<CombatEvent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in events.read() {
        if event.damage <= 0.0 {
            continue;
        }
        emit_play_for_entity(
            event.target,
            ANIM_HURT_STAGGER,
            HIT_RECOIL_PRIORITY,
            Some(1),
            &players,
            &mut vfx_events,
        );
    }
}

/// Breakthrough success -> full-body story animation.
pub fn emit_breakthrough_animation_triggers(
    mut outcomes: EventReader<BreakthroughOutcome>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for outcome in outcomes.read() {
        if outcome.result.is_err() {
            continue;
        }
        emit_play_for_entity(
            outcome.entity,
            breakthrough_anim_for_outcome(outcome),
            STORY_PRIORITY,
            Some(3),
            &players,
            &mut vfx_events,
        );
    }
}

/// Tribulation lifecycle -> brace / recoil animations.
pub fn emit_tribulation_animation_triggers(
    mut announces: EventReader<TribulationAnnounce>,
    mut failures: EventReader<TribulationFailed>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for announce in announces.read() {
        emit_play_for_entity(
            announce.entity,
            ANIM_TRIBULATION_BRACE,
            STORY_PRIORITY,
            Some(3),
            &players,
            &mut vfx_events,
        );
    }

    for failure in failures.read() {
        emit_play_for_entity(
            failure.entity,
            ANIM_HURT_STAGGER,
            HIT_RECOIL_PRIORITY,
            Some(1),
            &players,
            &mut vfx_events,
        );
    }
}

pub fn emit_woliu_v2_visual_triggers(
    mut casts: EventReader<VortexCastEvent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in casts.read() {
        emit_play_for_entity(
            event.caster,
            event.visual.animation_id,
            WOLIU_PRIORITY,
            Some(2),
            &players,
            &mut vfx_events,
        );
        vfx_events.send(VfxEventRequest::new(
            event.center,
            VfxEventPayloadV1::SpawnParticle {
                event_id: event.visual.particle_id.to_string(),
                origin: [event.center.x, event.center.y, event.center.z],
                direction: None,
                color: Some(color_for_woliu_skill(event.skill).to_string()),
                strength: Some(event.turbulence_radius.clamp(0.0, 30.0) / 30.0),
                count: Some(12),
                duration_ticks: Some(42),
            },
        ));
    }
}

pub fn emit_botany_harvest_visual_triggers(
    mut terminal: EventReader<HarvestTerminalEvent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in terminal.read() {
        if !event.completed || event.interrupted {
            continue;
        }
        emit_play_for_entity(
            event.client_entity,
            ANIM_HARVEST_CROUCH,
            COMBAT_PRIORITY,
            Some(2),
            &players,
            &mut vfx_events,
        );
        let Some(pos) = event.target_pos else {
            continue;
        };
        emit_spawn_particle(
            &mut vfx_events,
            BOTANY_HARVEST_VFX,
            valence::prelude::DVec3::new(pos[0], pos[1] + 0.45, pos[2]),
            botany_quality_color(event.spirit_quality),
            event.spirit_quality.clamp(0.5, 1.0),
            12,
            36,
        );
    }
}

pub fn emit_lingtian_visual_triggers(
    mut tills: EventReader<TillCompleted>,
    mut plantings: EventReader<PlantingCompleted>,
    mut harvests: EventReader<HarvestCompleted>,
    mut replenishes: EventReader<ReplenishCompleted>,
    mut drains: EventReader<DrainQiCompleted>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
) {
    for event in tills.read() {
        emit_play_for_entity(
            event.player,
            ANIM_LINGTIAN_TILL,
            COMBAT_PRIORITY,
            Some(2),
            &players,
            &mut vfx_events,
        );
        emit_block_decal(
            &mut vfx_events,
            LINGTIAN_TILL_VFX,
            event.pos,
            "#44CCCC",
            0.65,
        );
    }
    for event in plantings.read() {
        emit_block_decal(
            &mut vfx_events,
            LINGTIAN_PLANT_VFX,
            event.pos,
            "#55EE88",
            0.75,
        );
    }
    for event in harvests.read() {
        emit_block_decal(
            &mut vfx_events,
            LINGTIAN_HARVEST_VFX,
            event.pos,
            "#88FF66",
            0.85,
        );
    }
    for event in replenishes.read() {
        emit_block_decal(
            &mut vfx_events,
            LINGTIAN_REPLENISH_VFX,
            event.pos,
            "#44DDCC",
            (0.55 + event.plot_qi_added).clamp(0.55, 1.0),
        );
    }
    for event in drains.read() {
        emit_block_decal(
            &mut vfx_events,
            LINGTIAN_DRAIN_VFX,
            event.pos,
            "#888888",
            0.7,
        );
    }
}

fn color_for_woliu_skill(skill: WoliuSkillId) -> &'static str {
    match skill {
        WoliuSkillId::Hold => "#244872",
        WoliuSkillId::Burst => "#4078A8",
        WoliuSkillId::Mouth => "#1E2440",
        WoliuSkillId::Pull => "#382058",
        WoliuSkillId::Heart => "#100818",
    }
}

fn emit_block_decal(
    vfx_events: &mut EventWriter<VfxEventRequest>,
    event_id: &'static str,
    pos: valence::prelude::BlockPos,
    color: &'static str,
    strength: f32,
) {
    emit_spawn_particle(
        vfx_events,
        event_id,
        valence::prelude::DVec3::new(
            f64::from(pos.x) + 0.5,
            f64::from(pos.y) + 1.01,
            f64::from(pos.z) + 0.5,
        ),
        color,
        strength,
        1,
        80,
    );
}

fn emit_spawn_particle(
    vfx_events: &mut EventWriter<VfxEventRequest>,
    event_id: &'static str,
    origin: valence::prelude::DVec3,
    color: &'static str,
    strength: f32,
    count: u16,
    duration_ticks: u16,
) {
    vfx_events.send(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id: event_id.to_string(),
            origin: [origin.x, origin.y, origin.z],
            direction: None,
            color: Some(color.to_string()),
            strength: Some(strength.clamp(0.0, 1.0)),
            count: Some(count),
            duration_ticks: Some(duration_ticks),
        },
    ));
}

fn attack_anim_for_wound_kind(kind: WoundKind) -> &'static str {
    match kind {
        WoundKind::Cut => ANIM_SWORD_SLASH_DOWN,
        WoundKind::Pierce => ANIM_SWORD_STAB,
        WoundKind::Burn => ANIM_PALM_STRIKE,
        WoundKind::Blunt | WoundKind::Concussion => ANIM_FIST_PUNCH_RIGHT,
    }
}

fn breakthrough_anim_for_outcome(outcome: &BreakthroughOutcome) -> &'static str {
    let Ok(success) = &outcome.result else {
        return ANIM_BREAKTHROUGH_YINQI;
    };
    match (outcome.from, success.to) {
        (
            crate::cultivation::components::Realm::Awaken,
            crate::cultivation::components::Realm::Induce,
        ) => ANIM_BREAKTHROUGH_YINQI,
        (
            crate::cultivation::components::Realm::Induce,
            crate::cultivation::components::Realm::Condense,
        ) => ANIM_BREAKTHROUGH_NINGMAI,
        (
            crate::cultivation::components::Realm::Condense,
            crate::cultivation::components::Realm::Solidify,
        ) => ANIM_BREAKTHROUGH_GUYUAN,
        _ => ANIM_BREAKTHROUGH_TONGLING,
    }
}

fn emit_play_for_entity(
    entity: valence::prelude::Entity,
    anim_id: &'static str,
    priority: u16,
    fade_in_ticks: Option<u8>,
    players: &Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    vfx_events: &mut EventWriter<VfxEventRequest>,
) {
    let Ok((position, unique_id)) = players.get(entity) else {
        return;
    };
    let origin = position.get();
    vfx_events.send(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::PlayAnim {
            target_player: unique_id.0.to_string(),
            anim_id: anim_id.to_string(),
            priority,
            fade_in_ticks,
        },
    ));
}

#[cfg(test)]
mod tests {
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

    fn spawn_skinned_npc_target(
        app: &mut App,
        name: &str,
        pos: [f64; 3],
    ) -> valence::prelude::Entity {
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
    fn burst_meridian_attack_intent_does_not_duplicate_beng_quan_animation() {
        let mut app = App::new();
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
}
