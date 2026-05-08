use valence::prelude::{bevy_ecs, Entity, Event, Position, UniqueId};

use crate::combat::components::{CastSource, Casting, SkillBarBindings, WoundKind};
use crate::combat::events::{AttackIntent, AttackSource, FIST_REACH};
use crate::combat::CombatClock;
use crate::cultivation::color::{record_style_practice, PracticeLog};
use crate::cultivation::components::{ColorKind, Cultivation, MeridianId, MeridianSystem, Realm};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::network::cast_emit::current_unix_millis;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::server_data::{BurstMeridianEventV1, ServerDataPayloadV1};
use crate::schema::vfx_event::VfxEventPayloadV1;

const BENG_QUAN_ANIM_ID: &str = "bong:beng_quan";
const BENG_QUAN_PARTICLE_ID: &str = "bong:burst_meridian_beng_quan";

pub const BENG_QUAN_SKILL_ID: &str = "burst_meridian.beng_quan";
pub const BENG_QUAN_EVENT_SKILL: &str = "beng_quan";
pub const BENG_QUAN_QI_COST_RATIO: f64 = 0.4;
pub const BENG_QUAN_OVERLOAD_RATIO: f64 = 1.5;
pub const BENG_QUAN_INTEGRITY_MULTIPLIER: f64 = 0.7;
pub const BENG_QUAN_COOLDOWN_TICKS: u64 = 60;
pub const BENG_QUAN_ANIM_DURATION_TICKS: u32 = 8;

const RIGHT_ARM_MERIDIANS: [MeridianId; 3] = [
    MeridianId::LargeIntestine,
    MeridianId::SmallIntestine,
    MeridianId::TripleEnergizer,
];

#[derive(Debug, Clone, Event, PartialEq)]
pub struct BurstMeridianEvent {
    pub skill: &'static str,
    pub caster: Entity,
    pub target: Option<Entity>,
    pub tick: u64,
    pub overload_ratio: f64,
    pub integrity_snapshot: f64,
}

impl BurstMeridianEvent {
    pub fn to_payload(&self, world: &bevy_ecs::world::World) -> ServerDataPayloadV1 {
        ServerDataPayloadV1::BurstMeridianEvent(BurstMeridianEventV1 {
            skill: self.skill.to_string(),
            caster: entity_wire_id(world, self.caster),
            target: self.target.map(|target| entity_wire_id(world, target)),
            tick: self.tick,
            overload_ratio: self.overload_ratio,
            integrity_snapshot: self.integrity_snapshot,
        })
    }
}

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(BENG_QUAN_SKILL_ID, resolve_beng_quan);
}

pub fn resolve_beng_quan(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    let Some(target) = target else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let Some(clock) = world.get_resource::<CombatClock>() else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let now_tick = clock.tick;

    if world
        .get::<SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
    {
        return rejected(CastRejectReason::OnCooldown);
    }

    let Some(caster_position) = world.get::<Position>(caster).map(|position| position.get()) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let Some(target_position) = world.get::<Position>(target).map(|position| position.get()) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if caster_position.distance(target_position) > f64::from(FIST_REACH.max) + f64::EPSILON {
        return rejected(CastRejectReason::InvalidTarget);
    }

    let Some(cultivation) = world.get::<Cultivation>(caster) else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    if realm_rank(cultivation.realm) < realm_rank(Realm::Induce) {
        return rejected(CastRejectReason::RealmTooLow);
    }

    let cost = cultivation.qi_current * BENG_QUAN_QI_COST_RATIO;
    if cultivation.qi_current + f64::EPSILON < cost || cost <= f64::EPSILON {
        return rejected(CastRejectReason::QiInsufficient);
    }

    let Some(meridians) = world.get::<MeridianSystem>(caster) else {
        return rejected(CastRejectReason::MeridianSevered);
    };
    let integrity_snapshot = right_arm_integrity_snapshot(meridians);
    if !has_usable_right_arm_meridian(meridians) {
        return rejected(CastRejectReason::MeridianSevered);
    }

    let started_at_ms = current_unix_millis();
    world.entity_mut(caster).insert(Casting {
        source: CastSource::SkillBar,
        slot,
        started_at_tick: now_tick,
        duration_ticks: u64::from(BENG_QUAN_ANIM_DURATION_TICKS),
        started_at_ms,
        duration_ms: BENG_QUAN_ANIM_DURATION_TICKS.saturating_mul(50),
        bound_instance_id: None,
        start_position: caster_position,
        complete_cooldown_ticks: BENG_QUAN_COOLDOWN_TICKS,
        skill_id: Some(BENG_QUAN_SKILL_ID.to_string()),
        skill_config: None,
    });

    if let Some(mut cultivation) = world.get_mut::<Cultivation>(caster) {
        cultivation.qi_current = (cultivation.qi_current - cost).clamp(0.0, cultivation.qi_max);
    }
    if let Some(mut meridians) = world.get_mut::<MeridianSystem>(caster) {
        for id in RIGHT_ARM_MERIDIANS {
            let meridian = meridians.get_mut(id);
            meridian.integrity =
                (meridian.integrity * BENG_QUAN_INTEGRITY_MULTIPLIER).clamp(0.0, 1.0);
        }
    }
    if let Some(mut practice_log) = world.get_mut::<PracticeLog>(caster) {
        record_style_practice(&mut practice_log, ColorKind::Heavy);
    }

    world.send_event(AttackIntent {
        attacker: caster,
        target: Some(target),
        issued_at_tick: now_tick,
        reach: FIST_REACH,
        qi_invest: (cost * BENG_QUAN_OVERLOAD_RATIO) as f32,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::BurstMeridian,
        debug_command: None,
    });
    world.send_event(BurstMeridianEvent {
        skill: BENG_QUAN_EVENT_SKILL,
        caster,
        target: Some(target),
        tick: now_tick,
        overload_ratio: BENG_QUAN_OVERLOAD_RATIO,
        integrity_snapshot,
    });
    emit_beng_quan_vfx(world, caster, caster_position, target_position);

    CastResult::Started {
        cooldown_ticks: BENG_QUAN_COOLDOWN_TICKS,
        anim_duration_ticks: BENG_QUAN_ANIM_DURATION_TICKS,
    }
}

fn emit_beng_quan_vfx(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    caster_position: valence::prelude::DVec3,
    target_position: valence::prelude::DVec3,
) {
    if let Some(unique_id) = world.get::<UniqueId>(caster).copied() {
        world.send_event(VfxEventRequest::new(
            caster_position,
            VfxEventPayloadV1::PlayAnim {
                target_player: unique_id.0.to_string(),
                anim_id: BENG_QUAN_ANIM_ID.to_string(),
                priority: 1500,
                fade_in_ticks: Some(2),
            },
        ));
    }

    let direction = target_position - caster_position;
    world.send_event(VfxEventRequest::new(
        caster_position,
        VfxEventPayloadV1::SpawnParticle {
            event_id: BENG_QUAN_PARTICLE_ID.to_string(),
            origin: [
                caster_position.x,
                caster_position.y + 1.0,
                caster_position.z,
            ],
            direction: Some([direction.x, direction.y, direction.z]),
            color: Some("#C58B3F".to_string()),
            strength: Some(0.9),
            count: Some(8),
            duration_ticks: Some(BENG_QUAN_ANIM_DURATION_TICKS as u16),
        },
    ));
}

fn entity_wire_id(world: &bevy_ecs::world::World, entity: Entity) -> String {
    world
        .get::<UniqueId>(entity)
        .map(|unique_id| format!("player:{}", unique_id.0))
        .unwrap_or_else(|| format!("entity:{}", entity.to_bits()))
}

fn rejected(reason: CastRejectReason) -> CastResult {
    CastResult::Rejected { reason }
}

fn realm_rank(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

fn has_usable_right_arm_meridian(meridians: &MeridianSystem) -> bool {
    RIGHT_ARM_MERIDIANS
        .iter()
        .any(|id| meridians.get(*id).integrity > f64::EPSILON)
}

fn right_arm_integrity_snapshot(meridians: &MeridianSystem) -> f64 {
    RIGHT_ARM_MERIDIANS
        .iter()
        .map(|id| meridians.get(*id).integrity.clamp(0.0, 1.0))
        .sum::<f64>()
        / RIGHT_ARM_MERIDIANS.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, DVec3, Events};

    fn spawn_caster(app: &mut App, realm: Realm, qi_current: f64, position: DVec3) -> Entity {
        app.world_mut()
            .spawn((
                Cultivation {
                    realm,
                    qi_current,
                    qi_max: 100.0,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Position::new([position.x, position.y, position.z]),
                SkillBarBindings::default(),
                PracticeLog::default(),
            ))
            .id()
    }

    fn spawn_target(app: &mut App, position: DVec3) -> Entity {
        app.world_mut()
            .spawn(Position::new([position.x, position.y, position.z]))
            .id()
    }

    fn app() -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 10 });
        app.add_event::<AttackIntent>();
        app.add_event::<BurstMeridianEvent>();
        app.add_event::<VfxEventRequest>();
        app
    }

    fn assert_no_mutation(app: &App, caster: Entity, qi: f64, integrity: f64) {
        assert_eq!(
            app.world().get::<Cultivation>(caster).unwrap().qi_current,
            qi
        );
        for id in RIGHT_ARM_MERIDIANS {
            assert_eq!(
                app.world()
                    .get::<MeridianSystem>(caster)
                    .unwrap()
                    .get(id)
                    .integrity,
                integrity
            );
        }
        assert!(app.world().get::<Casting>(caster).is_none());
        assert!(app.world().resource::<Events<AttackIntent>>().is_empty());
        assert!(app
            .world()
            .resource::<Events<BurstMeridianEvent>>()
            .is_empty());
    }

    #[test]
    fn beng_quan_happy_path_mutates_atomically_and_emits_events() {
        let mut app = app();
        let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
        let target = spawn_target(&mut app, DVec3::new(f64::from(FIST_REACH.max), 0.0, 0.0));

        let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

        assert_eq!(
            result,
            CastResult::Started {
                cooldown_ticks: 60,
                anim_duration_ticks: 8,
            }
        );
        assert_eq!(
            app.world().get::<Cultivation>(caster).unwrap().qi_current,
            60.0
        );
        for id in RIGHT_ARM_MERIDIANS {
            assert_eq!(
                app.world()
                    .get::<MeridianSystem>(caster)
                    .unwrap()
                    .get(id)
                    .integrity,
                0.7
            );
        }
        assert_eq!(
            app.world().get::<Casting>(caster).unwrap().duration_ticks,
            8
        );
        assert_eq!(
            app.world()
                .get::<PracticeLog>(caster)
                .unwrap()
                .weights
                .get(&ColorKind::Heavy)
                .copied(),
            Some(crate::cultivation::color::STYLE_PRACTICE_AMOUNT)
        );

        let attack_events = app.world().resource::<Events<AttackIntent>>();
        let attack = attack_events.iter_current_update_events().next().unwrap();
        assert_eq!(attack.target, Some(target));
        assert_eq!(attack.source, AttackSource::BurstMeridian);
        assert_eq!(attack.qi_invest, 60.0);
        assert_eq!(attack.wound_kind, WoundKind::Blunt);

        let burst_events = app.world().resource::<Events<BurstMeridianEvent>>();
        let burst = burst_events.iter_current_update_events().next().unwrap();
        assert_eq!(burst.skill, BENG_QUAN_EVENT_SKILL);
        assert_eq!(burst.target, Some(target));
        assert_eq!(burst.overload_ratio, 1.5);
        assert_eq!(burst.integrity_snapshot, 1.0);

        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        let vfx: Vec<_> = vfx_events.iter_current_update_events().collect();
        assert_eq!(vfx.len(), 1);
        match &vfx[0].payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id, color, ..
            } => {
                assert_eq!(event_id, BENG_QUAN_PARTICLE_ID);
                assert_eq!(color.as_deref(), Some("#C58B3F"));
            }
            other => panic!("expected beng_quan particle, got {other:?}"),
        }
    }

    #[test]
    fn burst_event_payload_uses_stable_entity_wire_ids() {
        let mut app = app();
        let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));
        let event = BurstMeridianEvent {
            skill: BENG_QUAN_EVENT_SKILL,
            caster,
            target: Some(target),
            tick: 10,
            overload_ratio: 1.5,
            integrity_snapshot: 1.0,
        };

        let ServerDataPayloadV1::BurstMeridianEvent(payload) = event.to_payload(app.world()) else {
            panic!("expected burst meridian payload");
        };

        assert_eq!(payload.skill, BENG_QUAN_EVENT_SKILL);
        assert_eq!(payload.caster, format!("entity:{}", caster.to_bits()));
        assert_eq!(payload.target, Some(format!("entity:{}", target.to_bits())));
    }

    #[test]
    fn beng_quan_rejects_low_realm_without_mutation() {
        let mut app = app();
        let caster = spawn_caster(&mut app, Realm::Awaken, 100.0, DVec3::ZERO);
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

        assert_eq!(result, rejected(CastRejectReason::RealmTooLow));
        assert_no_mutation(&app, caster, 100.0, 1.0);
    }

    #[test]
    fn beng_quan_rejects_all_right_arm_meridians_severed() {
        let mut app = app();
        let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
        for id in RIGHT_ARM_MERIDIANS {
            app.world_mut()
                .get_mut::<MeridianSystem>(caster)
                .unwrap()
                .get_mut(id)
                .integrity = 0.0;
        }
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

        assert_eq!(result, rejected(CastRejectReason::MeridianSevered));
        assert_no_mutation(&app, caster, 100.0, 0.0);
    }

    #[test]
    fn beng_quan_allows_one_remaining_right_arm_meridian() {
        let mut app = app();
        let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
        app.world_mut()
            .get_mut::<MeridianSystem>(caster)
            .unwrap()
            .get_mut(MeridianId::SmallIntestine)
            .integrity = 0.0;
        app.world_mut()
            .get_mut::<MeridianSystem>(caster)
            .unwrap()
            .get_mut(MeridianId::TripleEnergizer)
            .integrity = 0.0;
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

        assert!(matches!(result, CastResult::Started { .. }));
    }

    #[test]
    fn beng_quan_rejects_missing_or_out_of_range_target() {
        let mut app = app();
        let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
        let target = spawn_target(
            &mut app,
            DVec3::new(f64::from(FIST_REACH.max) + 0.01, 0.0, 0.0),
        );

        assert_eq!(
            resolve_beng_quan(app.world_mut(), caster, 0, None),
            rejected(CastRejectReason::InvalidTarget)
        );
        assert_eq!(
            resolve_beng_quan(app.world_mut(), caster, 0, Some(target)),
            rejected(CastRejectReason::InvalidTarget)
        );
        assert_no_mutation(&app, caster, 100.0, 1.0);
    }

    #[test]
    fn beng_quan_rejects_cooldown_before_mutation() {
        let mut app = app();
        let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
        app.world_mut()
            .get_mut::<SkillBarBindings>(caster)
            .unwrap()
            .set_cooldown(0, 11);
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

        assert_eq!(result, rejected(CastRejectReason::OnCooldown));
        assert_no_mutation(&app, caster, 100.0, 1.0);
    }

    #[test]
    fn beng_quan_preserves_float_precision_and_pre_mutation_snapshot() {
        let mut app = app();
        let caster = spawn_caster(&mut app, Realm::Induce, 99.9, DVec3::ZERO);
        app.world_mut()
            .get_mut::<MeridianSystem>(caster)
            .unwrap()
            .get_mut(MeridianId::LargeIntestine)
            .integrity = 0.1;
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

        assert!(matches!(result, CastResult::Started { .. }));
        let qi = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        assert!((qi - 59.94).abs() < 1e-9);
        let li = app
            .world()
            .get::<MeridianSystem>(caster)
            .unwrap()
            .get(MeridianId::LargeIntestine)
            .integrity;
        assert!((li - 0.07).abs() < 1e-12);
        let burst_events = app.world().resource::<Events<BurstMeridianEvent>>();
        let burst = burst_events.iter_current_update_events().next().unwrap();
        assert!((burst.integrity_snapshot - 0.7).abs() < 1e-12);
    }
}
