use valence::prelude::{bevy_ecs, DVec3, Entity, Events, Position};

use crate::combat::components::Wounds;
use crate::combat::events::{ApplyStatusEffectIntent, StatusEffectKind};
use crate::cultivation::components::{Cultivation, MeridianId};
use crate::cultivation::meridian::severed::SkillMeridianDependencies;
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::cultivation::technique_scroll::realm_rank;
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::patrol::NpcPatrol;
use crate::qi_physics::constants::{QI_EPSILON, QI_ZONE_UNIT_CAPACITY};
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason};
use crate::qi_physics::release::qi_release_to_zone;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::zone::ZoneRegistry;

// ─── AV reuse map (纯加法 cosmetic，无净新资产) ────────────────────────────────
// 三招 AV 全部复用既有 particle event_id（client 已注册）+ audio recipe（server JSON 已存在）。
//
// heal_basic → 医道平和色治疗脉络绿光（YidaoPeacePulsePlayer），与玩家「医道」回血同款语义。
// buff_speed → 解脉中和尘 + 移动冲刺音，表达「经脉加速运转」的疾行尘。
// buff_defense → 崩拳真元爆护体环 + 镇脉护盾嗡音，与 burst_meridian.ni_mai_hu_ti（同款 DamageReduction buff）一致。
const HEAL_PARTICLE_ID: &str = "bong:yidao_meridian_repair";
const HEAL_PARTICLE_COLOR: &str = "#A8E6CF";
const HEAL_AUDIO_RECIPE: &str = "yidao_meridian_repair";

const BUFF_SPEED_PARTICLE_ID: &str = "bong:jiemai_neutralize_dust";
const BUFF_SPEED_PARTICLE_COLOR: &str = "#9FD8C8";
const BUFF_SPEED_AUDIO_RECIPE: &str = "movement_dash";

const BUFF_DEFENSE_PARTICLE_ID: &str = "bong:burst_meridian_beng_quan";
const BUFF_DEFENSE_PARTICLE_COLOR: &str = "#5BA8C9";
const BUFF_DEFENSE_AUDIO_RECIPE: &str = "zhenmai_shield_hum";

/// AV 广播半径（与 audio/vfx 既有默认一致）。
const NPC_SKILL_AV_RADIUS: f64 = 64.0;
/// 护体/回血粒子原点抬高（贴近 NPC 胸口高度）。
const NPC_SKILL_AV_Y_OFFSET: f64 = 1.0;

pub const HEAL_QI_COST: f64 = 8.0;
pub const HEAL_BASE_AMOUNT: f64 = 5.0;
pub const HEAL_PER_REALM_RANK: f64 = 3.0;
pub const HEAL_COOLDOWN_TICKS: u64 = 200;
pub const BUFF_SPEED_QI_COST: f64 = 5.0;
pub const BUFF_SPEED_MAGNITUDE: f32 = 0.3;
pub const BUFF_DEFENSE_QI_COST: f64 = 6.0;
pub const BUFF_DEFENSE_MAGNITUDE: f32 = 0.2;
pub const BUFF_DURATION_TICKS: u64 = 200;
pub const BUFF_COOLDOWN_TICKS: u64 = 400;

/// Emit QiTransfer events to return spent qi to the NPC's home zone (qi conservation).
/// Follows the same pattern as `tuike_v2::skills::release_spent_qi_to_zone`.
fn release_npc_qi_to_zone(world: &mut bevy_ecs::world::World, caster: Entity, amount: f64) {
    if amount <= QI_EPSILON {
        return;
    }

    let home_zone = match world.get::<NpcPatrol>(caster) {
        Some(patrol) => patrol.home_zone.clone(),
        None => {
            // No patrol component — route to overflow so qi is not lost.
            let from = QiAccountId::npc(format!("npc_{}v{}", caster.index(), caster.generation()));
            let to = QiAccountId::overflow(format!("npc_skill_no_zone:{}", caster.to_bits()));
            if let Ok(transfer) = QiTransfer::new(from, to, amount, QiTransferReason::ReleaseToZone)
            {
                if let Some(mut events) = world.get_resource_mut::<Events<QiTransfer>>() {
                    events.send(transfer);
                }
            }
            return;
        }
    };

    let from = QiAccountId::npc(format!("npc_{}v{}", caster.index(), caster.generation()));
    let to = QiAccountId::zone(home_zone.clone());

    let mut transfers = Vec::new();

    if let Some(mut zones) = world.get_resource_mut::<ZoneRegistry>() {
        if let Some(zone) = zones.find_zone_mut(&home_zone) {
            // 不 .max(0.0)：负灵域（spirit_qi<0）当 0 会抹掉负缺口、凭空多 credit、破坏守恒
            // （#681/#696/#698 同类）。与规范 helper death_hooks::release_qi_amount_to_zone 一致用裸值。
            let zone_current = zone.spirit_qi * QI_ZONE_UNIT_CAPACITY;
            match qi_release_to_zone(
                amount,
                from.clone(),
                to,
                zone_current,
                QI_ZONE_UNIT_CAPACITY,
            ) {
                Ok(outcome) => {
                    zone.spirit_qi = (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
                    if let Some(transfer) = outcome.transfer {
                        transfers.push(transfer);
                    }
                    if outcome.overflow > QI_EPSILON {
                        let overflow_to = QiAccountId::overflow(format!(
                            "npc_skill_overflow:{}",
                            caster.to_bits()
                        ));
                        if let Ok(t) = QiTransfer::new(
                            from.clone(),
                            overflow_to,
                            outcome.overflow,
                            QiTransferReason::ReleaseToZone,
                        ) {
                            transfers.push(t);
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        ?error,
                        "[bong][npc_skill] invalid qi release for {:?}; route to overflow",
                        caster
                    );
                    let overflow_to =
                        QiAccountId::overflow(format!("npc_skill_overflow:{}", caster.to_bits()));
                    if let Ok(t) = QiTransfer::new(
                        from.clone(),
                        overflow_to,
                        amount,
                        QiTransferReason::ReleaseToZone,
                    ) {
                        transfers.push(t);
                    }
                }
            }
        } else {
            // Zone not found in registry — overflow.
            let overflow_to =
                QiAccountId::overflow(format!("npc_skill_no_zone:{}", caster.to_bits()));
            if let Ok(t) = QiTransfer::new(
                from.clone(),
                overflow_to,
                amount,
                QiTransferReason::ReleaseToZone,
            ) {
                transfers.push(t);
            }
        }
    }

    if !transfers.is_empty() {
        if let Some(mut events) = world.get_resource_mut::<Events<QiTransfer>>() {
            for t in transfers {
                events.send(t);
            }
        }
    }
}

/// 复用既有 particle + audio recipe 发射招式 AV（纯加法 cosmetic，无净新资产）。
///
/// NPC 是 MC mob 而非玩家实体，`PlayAnim`（玩家骨架动画）不适用，因此只发
/// `SpawnParticle`（世界坐标广播，distance-filter 给附近玩家）+ `PlaySoundRecipeRequest`
/// （`AudioRecipient::Radius` 半径广播）。与 brain_spider / tsy_hostile 等敌对 NPC 的
/// AV 路径一致。缺 `Position` 或缺事件资源时静默跳过，绝不影响招式逻辑/守恒。
fn emit_npc_skill_av(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    particle_id: &str,
    particle_color: &str,
    audio_recipe: &str,
) {
    let Some(position) = world.get::<Position>(caster).map(|p| p.0) else {
        return;
    };
    let origin = DVec3::new(position.x, position.y + NPC_SKILL_AV_Y_OFFSET, position.z);

    world.send_event(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id: particle_id.to_string(),
            origin: [origin.x, origin.y, origin.z],
            direction: None,
            color: Some(particle_color.to_string()),
            strength: Some(0.75),
            count: Some(12),
            duration_ticks: Some(40),
        },
    ));

    if let Some(mut events) = world.get_resource_mut::<Events<PlaySoundRecipeRequest>>() {
        events.send(PlaySoundRecipeRequest {
            recipe_id: audio_recipe.to_string(),
            instance_id: 0,
            pos: None,
            flag: None,
            volume_mul: 1.0,
            pitch_shift: 0.0,
            recipient: AudioRecipient::Radius {
                origin,
                radius: NPC_SKILL_AV_RADIUS,
            },
        });
    }
}

pub fn register_npc_skills(registry: &mut SkillRegistry) {
    registry.register("npc.heal_basic", npc_heal_basic);
    registry.register("npc.buff_speed", npc_buff_speed);
    registry.register("npc.buff_defense", npc_buff_defense);
}

pub fn declare_npc_skill_meridian_deps(deps: &mut SkillMeridianDependencies) {
    deps.declare(
        "npc.heal_basic",
        vec![MeridianId::Spleen, MeridianId::Kidney],
    );
    deps.declare(
        "npc.buff_speed",
        vec![MeridianId::Stomach, MeridianId::Bladder],
    );
    deps.declare(
        "npc.buff_defense",
        vec![MeridianId::Lung, MeridianId::Heart],
    );
}

fn npc_heal_basic(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    _slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let cultivation = match world.get::<Cultivation>(caster) {
        Some(c) => c.clone(),
        None => {
            return CastResult::Rejected {
                reason: CastRejectReason::QiInsufficient,
            }
        }
    };

    if cultivation.qi_current < HEAL_QI_COST {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }

    let heal_amount = HEAL_BASE_AMOUNT + realm_rank(cultivation.realm) as f64 * HEAL_PER_REALM_RANK;
    let heal_grades = (heal_amount / 0.25).round().clamp(0.0, f64::from(u8::MAX)) as u8;

    if let Some(mut cult) = world.get_mut::<Cultivation>(caster) {
        cult.qi_current = (cult.qi_current - HEAL_QI_COST).max(0.0);
    }

    release_npc_qi_to_zone(world, caster, HEAL_QI_COST);

    if let Some(mut wounds) = world.get_mut::<Wounds>(caster) {
        crate::alchemy::pill::apply_wound_heal(&mut wounds, None, heal_grades);
    }

    emit_npc_skill_av(
        world,
        caster,
        HEAL_PARTICLE_ID,
        HEAL_PARTICLE_COLOR,
        HEAL_AUDIO_RECIPE,
    );

    CastResult::Started {
        cooldown_ticks: HEAL_COOLDOWN_TICKS,
        anim_duration_ticks: 20,
    }
}

fn npc_buff_speed(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    _slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let cultivation = match world.get::<Cultivation>(caster) {
        Some(c) => c.clone(),
        None => {
            return CastResult::Rejected {
                reason: CastRejectReason::QiInsufficient,
            }
        }
    };

    if cultivation.qi_current < BUFF_SPEED_QI_COST {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }

    if let Some(mut cult) = world.get_mut::<Cultivation>(caster) {
        cult.qi_current = (cult.qi_current - BUFF_SPEED_QI_COST).max(0.0);
    }

    release_npc_qi_to_zone(world, caster, BUFF_SPEED_QI_COST);

    let clock = world
        .get_resource::<crate::cultivation::tick::CultivationClock>()
        .map(|c| c.tick)
        .unwrap_or(0);

    if let Some(mut events) = world.get_resource_mut::<Events<ApplyStatusEffectIntent>>() {
        events.send(ApplyStatusEffectIntent {
            target: caster,
            kind: StatusEffectKind::SpeedBoost,
            magnitude: BUFF_SPEED_MAGNITUDE,
            duration_ticks: BUFF_DURATION_TICKS,
            issued_at_tick: clock,
        });
    }

    emit_npc_skill_av(
        world,
        caster,
        BUFF_SPEED_PARTICLE_ID,
        BUFF_SPEED_PARTICLE_COLOR,
        BUFF_SPEED_AUDIO_RECIPE,
    );

    CastResult::Started {
        cooldown_ticks: BUFF_COOLDOWN_TICKS,
        anim_duration_ticks: 10,
    }
}

fn npc_buff_defense(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    _slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let cultivation = match world.get::<Cultivation>(caster) {
        Some(c) => c.clone(),
        None => {
            return CastResult::Rejected {
                reason: CastRejectReason::QiInsufficient,
            }
        }
    };

    if cultivation.qi_current < BUFF_DEFENSE_QI_COST {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }

    if let Some(mut cult) = world.get_mut::<Cultivation>(caster) {
        cult.qi_current = (cult.qi_current - BUFF_DEFENSE_QI_COST).max(0.0);
    }

    release_npc_qi_to_zone(world, caster, BUFF_DEFENSE_QI_COST);

    let clock = world
        .get_resource::<crate::cultivation::tick::CultivationClock>()
        .map(|c| c.tick)
        .unwrap_or(0);

    if let Some(mut events) = world.get_resource_mut::<Events<ApplyStatusEffectIntent>>() {
        events.send(ApplyStatusEffectIntent {
            target: caster,
            kind: StatusEffectKind::DamageReduction,
            magnitude: BUFF_DEFENSE_MAGNITUDE,
            duration_ticks: BUFF_DURATION_TICKS,
            issued_at_tick: clock,
        });
    }

    emit_npc_skill_av(
        world,
        caster,
        BUFF_DEFENSE_PARTICLE_ID,
        BUFF_DEFENSE_PARTICLE_COLOR,
        BUFF_DEFENSE_AUDIO_RECIPE,
    );

    CastResult::Started {
        cooldown_ticks: BUFF_COOLDOWN_TICKS,
        anim_duration_ticks: 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::{BodyPart, Wound, WoundKind, Wounds};
    use crate::cultivation::components::{Cultivation, Realm};
    use crate::qi_physics::ledger::QiAccountKind;
    use valence::prelude::DVec3;

    fn world_with_events() -> bevy_ecs::world::World {
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(Events::<ApplyStatusEffectIntent>::default());
        world.insert_resource(Events::<QiTransfer>::default());
        world.insert_resource(Events::<VfxEventRequest>::default());
        world.insert_resource(Events::<PlaySoundRecipeRequest>::default());
        world
    }

    /// 收集本 tick 发出的 SpawnParticle (event_id, color)。
    fn collected_particles(world: &bevy_ecs::world::World) -> Vec<(String, Option<String>)> {
        let events = world.resource::<Events<VfxEventRequest>>();
        let mut reader = events.get_reader();
        reader
            .read(events)
            .filter_map(|req| match &req.payload {
                VfxEventPayloadV1::SpawnParticle {
                    event_id, color, ..
                } => Some((event_id.clone(), color.clone())),
                _ => None,
            })
            .collect()
    }

    /// 收集本 tick 发出的 audio recipe_id。
    fn collected_audio(world: &bevy_ecs::world::World) -> Vec<String> {
        let events = world.resource::<Events<PlaySoundRecipeRequest>>();
        let mut reader = events.get_reader();
        reader.read(events).map(|r| r.recipe_id.clone()).collect()
    }

    fn make_cultivation(realm: Realm, qi: f64) -> Cultivation {
        Cultivation {
            realm,
            qi_current: qi,
            qi_max: 100.0,
            ..Default::default()
        }
    }

    fn make_wounds(health_current: f32, health_max: f32, entries: Vec<Wound>) -> Wounds {
        Wounds {
            health_current,
            health_max,
            entries,
        }
    }

    // === npc_heal_basic ===

    #[test]
    fn heal_basic_reduces_wound_severity_and_restores_hp() {
        let mut world = world_with_events();
        let wounds = make_wounds(
            50.0,
            100.0,
            vec![Wound {
                location: BodyPart::Chest,
                kind: WoundKind::Cut,
                severity: 0.5,
                bleeding_per_sec: 0.0,
                created_at_tick: 0,
                inflicted_by: None,
            }],
        );
        let entity = world
            .spawn((make_cultivation(Realm::Induce, 50.0), wounds))
            .id();

        let result = npc_heal_basic(&mut world, entity, 0, None);

        assert!(
            matches!(
                result,
                CastResult::Started {
                    cooldown_ticks,
                    anim_duration_ticks: 20
                } if cooldown_ticks == HEAL_COOLDOWN_TICKS
            ),
            "heal should succeed with started result, got {result:?}"
        );

        let wounds = world.get::<Wounds>(entity).unwrap();
        assert!(
            wounds.health_current > 50.0,
            "HP should increase after heal, got {}",
            wounds.health_current
        );

        let cult = world.get::<Cultivation>(entity).unwrap();
        let expected_qi = 50.0 - HEAL_QI_COST;
        assert!(
            (cult.qi_current - expected_qi).abs() < f64::EPSILON,
            "qi should decrease by {}, got {}",
            HEAL_QI_COST,
            cult.qi_current
        );
    }

    #[test]
    fn heal_basic_scales_with_realm_rank() {
        let mut world_induce = world_with_events();
        let mut world_void = world_with_events();

        let wounds = || {
            make_wounds(
                20.0,
                100.0,
                vec![Wound {
                    location: BodyPart::Chest,
                    kind: WoundKind::Cut,
                    severity: 0.6,
                    bleeding_per_sec: 0.0,
                    created_at_tick: 0,
                    inflicted_by: None,
                }],
            )
        };

        let induce = world_induce
            .spawn((make_cultivation(Realm::Induce, 50.0), wounds()))
            .id();
        let void = world_void
            .spawn((make_cultivation(Realm::Void, 50.0), wounds()))
            .id();

        npc_heal_basic(&mut world_induce, induce, 0, None);
        npc_heal_basic(&mut world_void, void, 0, None);

        let hp_induce = world_induce.get::<Wounds>(induce).unwrap().health_current;
        let hp_void = world_void.get::<Wounds>(void).unwrap().health_current;

        assert!(
            hp_void > hp_induce,
            "higher realm should heal more: void={hp_void} should > induce={hp_induce}"
        );
    }

    #[test]
    fn heal_basic_rejected_when_qi_insufficient() {
        let mut world = world_with_events();
        let wounds = make_wounds(50.0, 100.0, vec![]);
        let entity = world
            .spawn((make_cultivation(Realm::Induce, 3.0), wounds))
            .id();

        let result = npc_heal_basic(&mut world, entity, 0, None);

        assert!(
            matches!(
                result,
                CastResult::Rejected {
                    reason: CastRejectReason::QiInsufficient,
                }
            ),
            "should reject when qi < {}, got {result:?}",
            HEAL_QI_COST
        );

        let cult = world.get::<Cultivation>(entity).unwrap();
        assert!(
            (cult.qi_current - 3.0).abs() < f64::EPSILON,
            "qi should not change on rejection"
        );
    }

    #[test]
    fn heal_basic_no_wounds_still_succeeds() {
        let mut world = world_with_events();
        let wounds = make_wounds(100.0, 100.0, vec![]);
        let entity = world
            .spawn((make_cultivation(Realm::Induce, 50.0), wounds))
            .id();

        let result = npc_heal_basic(&mut world, entity, 0, None);

        assert!(
            matches!(result, CastResult::Started { .. }),
            "heal with no wounds should still succeed (qi spent)"
        );

        let cult = world.get::<Cultivation>(entity).unwrap();
        let expected_qi = 50.0 - HEAL_QI_COST;
        assert!(
            (cult.qi_current - expected_qi).abs() < f64::EPSILON,
            "qi should still be consumed"
        );
    }

    #[test]
    fn heal_basic_rejected_without_cultivation() {
        let mut world = world_with_events();
        let entity = world.spawn_empty().id();

        let result = npc_heal_basic(&mut world, entity, 0, None);

        assert!(
            matches!(result, CastResult::Rejected { .. }),
            "should reject without cultivation component"
        );
    }

    #[test]
    fn heal_basic_qi_exactly_equal_to_cost() {
        let mut world = world_with_events();
        let wounds = make_wounds(50.0, 100.0, vec![]);
        let entity = world
            .spawn((make_cultivation(Realm::Induce, HEAL_QI_COST), wounds))
            .id();

        let result = npc_heal_basic(&mut world, entity, 0, None);

        assert!(
            matches!(result, CastResult::Started { .. }),
            "should succeed when qi == cost exactly"
        );

        let cult = world.get::<Cultivation>(entity).unwrap();
        assert!(
            cult.qi_current.abs() < f64::EPSILON,
            "qi should be exactly 0 after spending {}",
            HEAL_QI_COST
        );
    }

    // === npc_buff_speed ===

    #[test]
    fn buff_speed_sends_speed_boost_intent() {
        let mut world = world_with_events();
        let entity = world.spawn(make_cultivation(Realm::Condense, 50.0)).id();

        let result = npc_buff_speed(&mut world, entity, 0, None);

        assert!(
            matches!(
                result,
                CastResult::Started {
                    cooldown_ticks,
                    anim_duration_ticks: 10
                } if cooldown_ticks == BUFF_COOLDOWN_TICKS
            ),
            "buff_speed should succeed, got {result:?}"
        );

        let events = world.resource::<Events<ApplyStatusEffectIntent>>();
        let mut reader = events.get_reader();
        let intents: Vec<_> = reader.read(events).collect();
        assert_eq!(intents.len(), 1, "should send exactly 1 intent");
        assert_eq!(intents[0].target, entity);
        assert_eq!(intents[0].kind, StatusEffectKind::SpeedBoost);
        assert!((intents[0].magnitude - BUFF_SPEED_MAGNITUDE).abs() < f32::EPSILON);
        assert_eq!(intents[0].duration_ticks, BUFF_DURATION_TICKS);

        let cult = world.get::<Cultivation>(entity).unwrap();
        let expected_qi = 50.0 - BUFF_SPEED_QI_COST;
        assert!(
            (cult.qi_current - expected_qi).abs() < f64::EPSILON,
            "qi should decrease by {}",
            BUFF_SPEED_QI_COST
        );
    }

    #[test]
    fn buff_speed_rejected_when_qi_insufficient() {
        let mut world = world_with_events();
        let entity = world.spawn(make_cultivation(Realm::Condense, 2.0)).id();

        let result = npc_buff_speed(&mut world, entity, 0, None);

        assert!(
            matches!(result, CastResult::Rejected { .. }),
            "should reject when qi < {}",
            BUFF_SPEED_QI_COST
        );

        let events = world.resource::<Events<ApplyStatusEffectIntent>>();
        let mut reader = events.get_reader();
        assert_eq!(
            reader.read(events).count(),
            0,
            "no intent should be sent on rejection"
        );
    }

    #[test]
    fn buff_speed_rejected_without_cultivation() {
        let mut world = world_with_events();
        let entity = world.spawn_empty().id();

        let result = npc_buff_speed(&mut world, entity, 0, None);

        assert!(
            matches!(result, CastResult::Rejected { .. }),
            "should reject without cultivation"
        );
    }

    // === npc_buff_defense ===

    #[test]
    fn buff_defense_sends_damage_reduction_intent() {
        let mut world = world_with_events();
        let entity = world.spawn(make_cultivation(Realm::Condense, 50.0)).id();

        let result = npc_buff_defense(&mut world, entity, 0, None);

        assert!(
            matches!(
                result,
                CastResult::Started {
                    cooldown_ticks,
                    anim_duration_ticks: 10
                } if cooldown_ticks == BUFF_COOLDOWN_TICKS
            ),
            "buff_defense should succeed, got {result:?}"
        );

        let events = world.resource::<Events<ApplyStatusEffectIntent>>();
        let mut reader = events.get_reader();
        let intents: Vec<_> = reader.read(events).collect();
        assert_eq!(intents.len(), 1, "should send exactly 1 intent");
        assert_eq!(intents[0].target, entity);
        assert_eq!(intents[0].kind, StatusEffectKind::DamageReduction);
        assert!((intents[0].magnitude - BUFF_DEFENSE_MAGNITUDE).abs() < f32::EPSILON);
        assert_eq!(intents[0].duration_ticks, BUFF_DURATION_TICKS);

        let cult = world.get::<Cultivation>(entity).unwrap();
        let expected_qi = 50.0 - BUFF_DEFENSE_QI_COST;
        assert!(
            (cult.qi_current - expected_qi).abs() < f64::EPSILON,
            "qi should decrease by {}",
            BUFF_DEFENSE_QI_COST
        );
    }

    #[test]
    fn buff_defense_rejected_when_qi_insufficient() {
        let mut world = world_with_events();
        let entity = world.spawn(make_cultivation(Realm::Condense, 3.0)).id();

        let result = npc_buff_defense(&mut world, entity, 0, None);

        assert!(
            matches!(result, CastResult::Rejected { .. }),
            "should reject when qi < {}",
            BUFF_DEFENSE_QI_COST
        );
    }

    #[test]
    fn buff_defense_rejected_without_cultivation() {
        let mut world = world_with_events();
        let entity = world.spawn_empty().id();

        let result = npc_buff_defense(&mut world, entity, 0, None);

        assert!(
            matches!(result, CastResult::Rejected { .. }),
            "should reject without cultivation"
        );
    }

    #[test]
    fn buff_defense_qi_exactly_equal_to_cost() {
        let mut world = world_with_events();
        let entity = world
            .spawn(make_cultivation(Realm::Condense, BUFF_DEFENSE_QI_COST))
            .id();

        let result = npc_buff_defense(&mut world, entity, 0, None);

        assert!(
            matches!(result, CastResult::Started { .. }),
            "should succeed when qi == cost exactly"
        );

        let cult = world.get::<Cultivation>(entity).unwrap();
        assert!(
            cult.qi_current.abs() < f64::EPSILON,
            "qi should be exactly 0"
        );
    }

    // === qi conservation: QiTransfer events ===

    fn world_with_zone_registry() -> bevy_ecs::world::World {
        let mut world = world_with_events();
        world.insert_resource(ZoneRegistry::default());
        world
    }

    #[test]
    fn heal_basic_emits_qi_transfer_to_zone() {
        let mut world = world_with_zone_registry();
        let wounds = make_wounds(50.0, 100.0, vec![]);
        let entity = world
            .spawn((
                make_cultivation(Realm::Induce, 50.0),
                wounds,
                NpcPatrol::new("spawn", DVec3::new(14.0, 66.0, 14.0)),
            ))
            .id();

        npc_heal_basic(&mut world, entity, 0, None);

        let events = world.resource::<Events<QiTransfer>>();
        let mut reader = events.get_reader();
        let transfers: Vec<_> = reader.read(events).collect();
        assert!(
            !transfers.is_empty(),
            "heal_basic should emit QiTransfer event for qi conservation"
        );
        let total: f64 = transfers.iter().map(|t| t.amount).sum();
        assert!(
            (total - HEAL_QI_COST).abs() < f64::EPSILON,
            "total transfer amount should equal qi cost {}, got {} across {} transfers",
            HEAL_QI_COST,
            total,
            transfers.len()
        );
        assert!(
            transfers
                .iter()
                .all(|t| t.reason == QiTransferReason::ReleaseToZone),
            "all transfers should have reason ReleaseToZone"
        );
    }

    /// 负灵域守恒（#681/#696/#698 同类）：home_zone spirit_qi=-0.5 时不得 .max(0.0) 当 0——否则
    /// 抹掉 -25 负缺口、凭空多 25 qi。修复后 zone_current=-25、heal cost 8 全额吸收（room=75），
    /// zone_after=(-25+8)/50=-0.34（而非 bug 的 (0+8)/50=0.16）。
    #[test]
    fn heal_negative_home_zone_no_phantom_credit() {
        let mut world = world_with_zone_registry();
        let initial_spirit_qi = -0.5;
        world
            .resource_mut::<ZoneRegistry>()
            .find_zone_mut("spawn")
            .expect("default registry 必须含 spawn zone")
            .spirit_qi = initial_spirit_qi;

        let wounds = make_wounds(50.0, 100.0, vec![]);
        let entity = world
            .spawn((
                make_cultivation(Realm::Induce, 50.0),
                wounds,
                NpcPatrol::new("spawn", DVec3::new(14.0, 66.0, 14.0)),
            ))
            .id();

        npc_heal_basic(&mut world, entity, 0, None);

        let zone_after = world
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .expect("spawn zone")
            .spirit_qi;
        // 裸 spirit_qi*CAP：zone_current=-25, room=75 > cost 8 → 全额吸收, zone_after=(-25+8)/50
        let expected =
            (initial_spirit_qi * QI_ZONE_UNIT_CAPACITY + HEAL_QI_COST) / QI_ZONE_UNIT_CAPACITY;
        let buggy_clamped = HEAL_QI_COST / QI_ZONE_UNIT_CAPACITY; // .max(0.0) 当 0 时的值 ≈0.16
        assert!(
            (zone_after - expected).abs() < 1e-9,
            "负灵域应按裸 spirit_qi*CAP 计：zone_after={zone_after:.4} 应={expected:.4}；\
             若 ≈{buggy_clamped:.4} 说明 .max(0.0) 抹掉了负缺口、凭空多出 qi（#681 同类）"
        );
    }

    #[test]
    fn buff_speed_emits_qi_transfer_to_zone() {
        let mut world = world_with_zone_registry();
        let entity = world
            .spawn((
                make_cultivation(Realm::Condense, 50.0),
                NpcPatrol::new("spawn", DVec3::new(14.0, 66.0, 14.0)),
            ))
            .id();

        npc_buff_speed(&mut world, entity, 0, None);

        let events = world.resource::<Events<QiTransfer>>();
        let mut reader = events.get_reader();
        let transfers: Vec<_> = reader.read(events).collect();
        assert!(
            !transfers.is_empty(),
            "buff_speed should emit QiTransfer event for qi conservation"
        );
        let total: f64 = transfers.iter().map(|t| t.amount).sum();
        assert!(
            (total - BUFF_SPEED_QI_COST).abs() < f64::EPSILON,
            "total transfer amount should equal qi cost {}, got {} across {} transfers",
            BUFF_SPEED_QI_COST,
            total,
            transfers.len()
        );
    }

    #[test]
    fn buff_defense_emits_qi_transfer_to_zone() {
        let mut world = world_with_zone_registry();
        let entity = world
            .spawn((
                make_cultivation(Realm::Condense, 50.0),
                NpcPatrol::new("spawn", DVec3::new(14.0, 66.0, 14.0)),
            ))
            .id();

        npc_buff_defense(&mut world, entity, 0, None);

        let events = world.resource::<Events<QiTransfer>>();
        let mut reader = events.get_reader();
        let transfers: Vec<_> = reader.read(events).collect();
        assert!(
            !transfers.is_empty(),
            "buff_defense should emit QiTransfer event for qi conservation"
        );
        let total: f64 = transfers.iter().map(|t| t.amount).sum();
        assert!(
            (total - BUFF_DEFENSE_QI_COST).abs() < f64::EPSILON,
            "total transfer amount should equal qi cost {}, got {} across {} transfers",
            BUFF_DEFENSE_QI_COST,
            total,
            transfers.len()
        );
    }

    #[test]
    fn skill_without_patrol_routes_to_overflow() {
        let mut world = world_with_zone_registry();
        let entity = world.spawn(make_cultivation(Realm::Condense, 50.0)).id();

        npc_buff_speed(&mut world, entity, 0, None);

        let events = world.resource::<Events<QiTransfer>>();
        let mut reader = events.get_reader();
        let transfers: Vec<_> = reader.read(events).collect();
        assert!(
            !transfers.is_empty(),
            "skill without NpcPatrol should still emit overflow QiTransfer"
        );
        assert_eq!(
            transfers[0].to.kind,
            QiAccountKind::Overflow,
            "should route to overflow account, got {:?}",
            transfers[0].to
        );
    }

    #[test]
    fn rejected_skill_does_not_emit_qi_transfer() {
        let mut world = world_with_zone_registry();
        let entity = world
            .spawn((
                make_cultivation(Realm::Induce, 1.0),
                NpcPatrol::new("spawn", DVec3::new(14.0, 66.0, 14.0)),
            ))
            .id();

        npc_heal_basic(&mut world, entity, 0, None);

        let events = world.resource::<Events<QiTransfer>>();
        let mut reader = events.get_reader();
        let transfers: Vec<_> = reader.read(events).collect();
        assert!(
            transfers.is_empty(),
            "rejected skill (qi insufficient) should not emit QiTransfer, got {} events",
            transfers.len()
        );
    }

    // === AV (particle + audio) emission ===
    //
    // 锁定：① 三招各发对应 reused particle event_id + color ② 各发对应 reused audio recipe
    //       ③ AV 只在 cast 成功时发，rejection 不发 ④ 缺 Position 时静默跳过不发 AV 也不崩。

    fn make_position() -> Position {
        Position::new([14.0, 66.0, 14.0])
    }

    #[test]
    fn heal_basic_emits_reused_heal_particle_and_audio() {
        let mut world = world_with_events();
        let wounds = make_wounds(50.0, 100.0, vec![]);
        let entity = world
            .spawn((
                make_cultivation(Realm::Induce, 50.0),
                wounds,
                make_position(),
            ))
            .id();

        npc_heal_basic(&mut world, entity, 0, None);

        let particles = collected_particles(&world);
        assert_eq!(
            particles.len(),
            1,
            "heal should emit exactly 1 particle, got {particles:?}"
        );
        assert_eq!(
            particles[0].0, HEAL_PARTICLE_ID,
            "heal must reuse 医道治疗 particle id (no net-new asset)"
        );
        assert_eq!(
            particles[0].1.as_deref(),
            Some(HEAL_PARTICLE_COLOR),
            "heal particle color should be the green heal tint"
        );

        let audio = collected_audio(&world);
        assert_eq!(
            audio,
            vec![HEAL_AUDIO_RECIPE.to_string()],
            "heal must reuse existing yidao_meridian_repair audio recipe"
        );
    }

    #[test]
    fn buff_speed_emits_reused_speed_particle_and_audio() {
        let mut world = world_with_events();
        let entity = world
            .spawn((make_cultivation(Realm::Condense, 50.0), make_position()))
            .id();

        npc_buff_speed(&mut world, entity, 0, None);

        let particles = collected_particles(&world);
        assert_eq!(particles.len(), 1, "buff_speed should emit 1 particle");
        assert_eq!(
            particles[0].0, BUFF_SPEED_PARTICLE_ID,
            "buff_speed must reuse 解脉中和尘 particle id"
        );
        assert_eq!(particles[0].1.as_deref(), Some(BUFF_SPEED_PARTICLE_COLOR));

        let audio = collected_audio(&world);
        assert_eq!(
            audio,
            vec![BUFF_SPEED_AUDIO_RECIPE.to_string()],
            "buff_speed must reuse existing movement_dash audio recipe"
        );
    }

    #[test]
    fn buff_defense_emits_reused_shield_particle_and_audio() {
        let mut world = world_with_events();
        let entity = world
            .spawn((make_cultivation(Realm::Condense, 50.0), make_position()))
            .id();

        npc_buff_defense(&mut world, entity, 0, None);

        let particles = collected_particles(&world);
        assert_eq!(particles.len(), 1, "buff_defense should emit 1 particle");
        assert_eq!(
            particles[0].0, BUFF_DEFENSE_PARTICLE_ID,
            "buff_defense must reuse 崩拳护体环 particle id (same as ni_mai_hu_ti)"
        );
        assert_eq!(particles[0].1.as_deref(), Some(BUFF_DEFENSE_PARTICLE_COLOR));

        let audio = collected_audio(&world);
        assert_eq!(
            audio,
            vec![BUFF_DEFENSE_AUDIO_RECIPE.to_string()],
            "buff_defense must reuse existing zhenmai_shield_hum audio recipe"
        );
    }

    #[test]
    fn three_skills_emit_distinct_particle_ids() {
        // 招式差异化硬约束：三招 particle/audio 各不相同。
        assert_ne!(HEAL_PARTICLE_ID, BUFF_SPEED_PARTICLE_ID);
        assert_ne!(HEAL_PARTICLE_ID, BUFF_DEFENSE_PARTICLE_ID);
        assert_ne!(BUFF_SPEED_PARTICLE_ID, BUFF_DEFENSE_PARTICLE_ID);
        assert_ne!(HEAL_AUDIO_RECIPE, BUFF_SPEED_AUDIO_RECIPE);
        assert_ne!(HEAL_AUDIO_RECIPE, BUFF_DEFENSE_AUDIO_RECIPE);
        assert_ne!(BUFF_SPEED_AUDIO_RECIPE, BUFF_DEFENSE_AUDIO_RECIPE);
    }

    #[test]
    fn rejected_skill_does_not_emit_av() {
        let mut world = world_with_events();
        // qi insufficient → rejected → no AV.
        let entity = world
            .spawn((make_cultivation(Realm::Induce, 1.0), make_position()))
            .id();

        let result = npc_heal_basic(&mut world, entity, 0, None);
        assert!(matches!(result, CastResult::Rejected { .. }));

        assert!(
            collected_particles(&world).is_empty(),
            "rejected cast must not emit particle AV"
        );
        assert!(
            collected_audio(&world).is_empty(),
            "rejected cast must not emit audio AV"
        );
    }

    #[test]
    fn av_skipped_without_position_but_cast_still_succeeds() {
        // NPC 缺 Position（罕见，但不能 panic）：招式逻辑照常成功，仅 particle 跳过。
        let mut world = world_with_events();
        let entity = world.spawn(make_cultivation(Realm::Condense, 50.0)).id();

        let result = npc_buff_defense(&mut world, entity, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "cast should still succeed without Position"
        );

        assert!(
            collected_particles(&world).is_empty(),
            "no particle when Position absent"
        );
        // status effect intent 仍应发出（逻辑不受 AV 影响）。
        let intents = world.resource::<Events<ApplyStatusEffectIntent>>();
        let mut reader = intents.get_reader();
        assert_eq!(
            reader.read(intents).count(),
            1,
            "buff intent must fire regardless of AV path"
        );
    }
}
