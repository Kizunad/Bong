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
use crate::qi_physics::ledger::{
    qi_flow_overflow_account, QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount,
};
use crate::qi_physics::release::qi_release_to_zone;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::zone::ZoneRegistry;

// ─── AV map (plan-skill-anim-fidelity-v1 P5：粒子去借用) ──────────────────────
// 去复用前三招粒子全是借的——heal 借医道 `bong:yidao_meridian_repair`、buff_speed
// 借真脉 `bong:jiemai_neutralize_dust`、buff_defense 借崩拳
// `bong:burst_meridian_beng_quan`。后果有二：① NPC 施法与玩家同招粒子完全同形，
// 旁观者分不清「对面 NPC 在补血」还是「有玩家在放医道」；② 借来的 id 命中
// `bong:yidao_` / `bong:jiemai_` 家族前缀，让 NPC 背景 cosmetic 误吃 Important
// 优先级、在拥挤 chunk 里挤掉玩家自己的技能反馈。
//
// 现改为 3 个 NPC 专属 id → client `NpcSkillAuraPlayer`（形态从简，见 plan §P5.1 ③），
// 且**不登记**进玩家技能优先级表 —— NPC 施法归 Normal 档是正确档位。
// audio recipe 仍复用既有条目（音效不在 P5 范围）。
pub(crate) const HEAL_PARTICLE_ID: &str = "bong:npc_heal_basic";
pub(crate) const HEAL_PARTICLE_COLOR: &str = "#A8E6CF";
const HEAL_AUDIO_RECIPE: &str = "yidao_meridian_repair";

pub(crate) const BUFF_SPEED_PARTICLE_ID: &str = "bong:npc_buff_speed";
/// 麦黄。旧值 `#9FD8C8` 与 heal 的 `#A8E6CF` 同属淡青绿、仅单通道差 ~10%，
/// 远距离不可辨——违背 plan P5「id 与颜色必须独立，保证旁观读招」。改后三招
/// 构成 绿 / 黄 / 蓝 高分离色相三元组。
pub(crate) const BUFF_SPEED_PARTICLE_COLOR: &str = "#E3C766";
const BUFF_SPEED_AUDIO_RECIPE: &str = "movement_dash";

pub(crate) const BUFF_DEFENSE_PARTICLE_ID: &str = "bong:npc_buff_defense";
pub(crate) const BUFF_DEFENSE_PARTICLE_COLOR: &str = "#5BA8C9";
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

/// M08：resolver 从权威 registry 读 qi_cost/cooldown 用的 skill id（与
/// `register_npc_skills` 的注册 id 一一对应；测试断言仍可用 HEAL_* 常量作
/// checked-in 期望值，运行时成本以 registry 为准）。
pub const NPC_HEAL_SKILL_ID: &str = "npc.heal_basic";
pub const NPC_BUFF_SPEED_SKILL_ID: &str = "npc.buff_speed";
pub const NPC_BUFF_DEFENSE_SKILL_ID: &str = "npc.buff_defense";

fn npc_qi_account(caster: Entity) -> QiAccountId {
    QiAccountId::npc(format!("npc_{}v{}", caster.index(), caster.generation()))
}

fn send_qi_transfer_event(world: &mut bevy_ecs::world::World, transfer: QiTransfer) {
    if let Some(mut events) = world.get_resource_mut::<Events<QiTransfer>>() {
        events.send(transfer);
    }
}

fn send_qi_transfer_events(world: &mut bevy_ecs::world::World, transfers: Vec<QiTransfer>) {
    if transfers.is_empty() {
        return;
    }

    if let Some(mut events) = world.get_resource_mut::<Events<QiTransfer>>() {
        for transfer in transfers {
            events.send(transfer);
        }
    }
}

fn credit_spent_qi_to_ledger(world: &mut bevy_ecs::world::World, transfer: &QiTransfer) {
    let Some(mut account) = world.get_resource_mut::<WorldQiAccount>() else {
        tracing::warn!(
            ?transfer,
            "[bong][npc_skill] WorldQiAccount missing; spent NPC qi cannot be ledgered"
        );
        return;
    };

    let previous_from_balance = account.balance(&transfer.from);
    if let Err(error) = account.set_balance(
        transfer.from.clone(),
        previous_from_balance + transfer.amount,
    ) {
        tracing::warn!(
            ?error,
            ?transfer,
            "[bong][npc_skill] failed to stage NPC skill spent qi source balance"
        );
        return;
    }

    if let Err(error) = account.transfer(transfer.clone()) {
        let _ = account.set_balance(transfer.from.clone(), previous_from_balance);
        tracing::warn!(
            ?error,
            ?transfer,
            "[bong][npc_skill] failed to ledger NPC skill spent qi"
        );
    }
}

fn route_spent_qi_to_overflow(
    world: &mut bevy_ecs::world::World,
    from: QiAccountId,
    to: QiAccountId,
    amount: f64,
) {
    if amount <= QI_EPSILON {
        return;
    }

    let Ok(transfer) = QiTransfer::new(from, to, amount, QiTransferReason::ReleaseToZone) else {
        tracing::warn!(
            amount,
            "[bong][npc_skill] invalid NPC skill overflow transfer amount"
        );
        return;
    };

    credit_spent_qi_to_ledger(world, &transfer);
    send_qi_transfer_event(world, transfer);
}

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
            let from = npc_qi_account(caster);
            let to = qi_flow_overflow_account();
            route_spent_qi_to_overflow(world, from, to, amount);
            return;
        }
    };

    let from = npc_qi_account(caster);
    let to = QiAccountId::zone(home_zone.clone());

    let mut transfers = Vec::new();
    let mut ledger_transfers = Vec::new();

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
                        let overflow_to = qi_flow_overflow_account();
                        if let Ok(t) = QiTransfer::new(
                            from.clone(),
                            overflow_to,
                            outcome.overflow,
                            QiTransferReason::ReleaseToZone,
                        ) {
                            ledger_transfers.push(t.clone());
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
                    let overflow_to = qi_flow_overflow_account();
                    if let Ok(t) = QiTransfer::new(
                        from.clone(),
                        overflow_to,
                        amount,
                        QiTransferReason::ReleaseToZone,
                    ) {
                        ledger_transfers.push(t.clone());
                        transfers.push(t);
                    }
                }
            }
        } else {
            // Zone not found in registry — overflow.
            let overflow_to = qi_flow_overflow_account();
            if let Ok(t) = QiTransfer::new(
                from.clone(),
                overflow_to,
                amount,
                QiTransferReason::ReleaseToZone,
            ) {
                ledger_transfers.push(t.clone());
                transfers.push(t);
            }
        }
    } else {
        let overflow_to = qi_flow_overflow_account();
        if let Ok(t) = QiTransfer::new(
            from.clone(),
            overflow_to,
            amount,
            QiTransferReason::ReleaseToZone,
        ) {
            ledger_transfers.push(t.clone());
            transfers.push(t);
        }
    }

    for transfer in &ledger_transfers {
        credit_spent_qi_to_ledger(world, transfer);
    }
    send_qi_transfer_events(world, transfers);
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

    // M08：resolver 必须消费权威 registry 的 qi_cost/cooldown——selector 已经按
    // registry 成本选择（npc/technique.rs），executor 若继续用硬编码常量就会
    // 出现「selector 认为可负担 → resolver 拒绝」或双重扣费的分裂契约。
    let Some(definition) = world
        .get_resource::<crate::cultivation::known_techniques::TechniqueRegistry>()
        .and_then(|techniques| techniques.get(NPC_HEAL_SKILL_ID))
        .cloned()
    else {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    };
    let cost = definition.qi_cost;

    if cultivation.qi_current + f64::EPSILON < cost {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }

    let heal_amount = HEAL_BASE_AMOUNT + realm_rank(cultivation.realm) as f64 * HEAL_PER_REALM_RANK;
    let heal_grades = (heal_amount / 0.25).round().clamp(0.0, f64::from(u8::MAX)) as u8;

    if let Some(mut cult) = world.get_mut::<Cultivation>(caster) {
        cult.qi_current = (cult.qi_current - cost).max(0.0);
    }

    release_npc_qi_to_zone(world, caster, cost);

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
        cooldown_ticks: u64::from(definition.cooldown_ticks).max(1),
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

    // M08：同 npc_heal_basic——executor 必须与 selector 共用同一 registry 成本。
    let Some(definition) = world
        .get_resource::<crate::cultivation::known_techniques::TechniqueRegistry>()
        .and_then(|techniques| techniques.get(NPC_BUFF_SPEED_SKILL_ID))
        .cloned()
    else {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    };
    let cost = definition.qi_cost;

    if cultivation.qi_current + f64::EPSILON < cost {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }

    if let Some(mut cult) = world.get_mut::<Cultivation>(caster) {
        cult.qi_current = (cult.qi_current - cost).max(0.0);
    }

    release_npc_qi_to_zone(world, caster, cost);

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
        cooldown_ticks: u64::from(definition.cooldown_ticks).max(1),
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

    // M08：同 npc_heal_basic——executor 必须与 selector 共用同一 registry 成本。
    let Some(definition) = world
        .get_resource::<crate::cultivation::known_techniques::TechniqueRegistry>()
        .and_then(|techniques| techniques.get(NPC_BUFF_DEFENSE_SKILL_ID))
        .cloned()
    else {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    };
    let cost = definition.qi_cost;

    if cultivation.qi_current + f64::EPSILON < cost {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }

    if let Some(mut cult) = world.get_mut::<Cultivation>(caster) {
        cult.qi_current = (cult.qi_current - cost).max(0.0);
    }

    release_npc_qi_to_zone(world, caster, cost);

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
        cooldown_ticks: u64::from(definition.cooldown_ticks).max(1),
        anim_duration_ticks: 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::{BodyPart, Wound, WoundKind, Wounds};
    use crate::cultivation::components::{Cultivation, Realm};
    use crate::qi_physics::ledger::{
        assert_conservation, summarize_world_qi, QiAccountKind, WorldQiAccount,
    };
    use valence::prelude::DVec3;

    fn world_with_events() -> bevy_ecs::world::World {
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(Events::<ApplyStatusEffectIntent>::default());
        world.insert_resource(Events::<QiTransfer>::default());
        world.insert_resource(Events::<VfxEventRequest>::default());
        world.insert_resource(Events::<PlaySoundRecipeRequest>::default());
        // M08：resolver 从注入 registry 读成本——测试必须安装与生产一致的 registry。
        world.insert_resource(
            crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests(),
        );
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
                location: crate::body_plan::legacy_body_part_to_id(BodyPart::Chest),
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
                    location: crate::body_plan::legacy_body_part_to_id(BodyPart::Chest),
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

    fn insert_qi_ledger(world: &mut bevy_ecs::world::World) {
        world.insert_resource(WorldQiAccount::default());
    }

    fn ledger_balance(world: &bevy_ecs::world::World, account: &QiAccountId) -> f64 {
        world.resource::<WorldQiAccount>().balance(account)
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
        insert_qi_ledger(&mut world);
        let entity = world.spawn(make_cultivation(Realm::Condense, 50.0)).id();
        let overflow_account = qi_flow_overflow_account();

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
        assert!(
            (ledger_balance(&world, &overflow_account) - BUFF_SPEED_QI_COST).abs() < QI_EPSILON,
            "no-patrol overflow must be a real WorldQiAccount balance"
        );
        assert!(
            ledger_balance(&world, &npc_qi_account(entity)).abs() < QI_EPSILON,
            "temporary live NPC source balance must be drained back to zero"
        );
    }

    #[test]
    fn heal_full_home_zone_ledgers_overflow_and_preserves_snapshot_total() {
        let mut world = world_with_zone_registry();
        insert_qi_ledger(&mut world);
        world
            .resource_mut::<ZoneRegistry>()
            .find_zone_mut("spawn")
            .expect("default registry 必须含 spawn zone")
            .spirit_qi = 1.0;

        let wounds = make_wounds(50.0, 100.0, vec![]);
        let entity = world
            .spawn((
                make_cultivation(Realm::Induce, 50.0),
                wounds,
                NpcPatrol::new("spawn", DVec3::new(14.0, 66.0, 14.0)),
            ))
            .id();
        let overflow_account = qi_flow_overflow_account();
        let before = summarize_world_qi(&mut world);

        npc_heal_basic(&mut world, entity, 0, None);

        let zone_after = world
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .expect("spawn zone")
            .spirit_qi;
        assert!(
            (zone_after - 1.0).abs() < QI_EPSILON,
            "满仓 zone 不应继续增加，实际 {zone_after}"
        );
        assert!(
            (ledger_balance(&world, &overflow_account) - HEAL_QI_COST).abs() < QI_EPSILON,
            "满仓 overflow 应完整落入 WorldQiAccount"
        );
        assert!(
            ledger_balance(&world, &npc_qi_account(entity)).abs() < QI_EPSILON,
            "活体 NPC source 只临时引燃，转账后必须归零"
        );
        let after = summarize_world_qi(&mut world);
        assert_conservation(&before, &after, 0.0).expect("full-overflow path must conserve qi");
    }

    #[test]
    fn buff_defense_near_cap_ledgers_only_overflow_remainder() {
        let mut world = world_with_zone_registry();
        insert_qi_ledger(&mut world);
        world
            .resource_mut::<ZoneRegistry>()
            .find_zone_mut("spawn")
            .expect("default registry 必须含 spawn zone")
            .spirit_qi = 0.90;

        let entity = world
            .spawn((
                make_cultivation(Realm::Condense, 50.0),
                NpcPatrol::new("spawn", DVec3::new(14.0, 66.0, 14.0)),
            ))
            .id();
        let overflow_account = qi_flow_overflow_account();
        let before_total = world.get::<Cultivation>(entity).unwrap().qi_current
            + world
                .resource::<ZoneRegistry>()
                .find_zone_by_name("spawn")
                .expect("spawn zone")
                .spirit_qi
                * QI_ZONE_UNIT_CAPACITY
            + world.resource::<WorldQiAccount>().total();

        npc_buff_defense(&mut world, entity, 0, None);

        let zone_after = world
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .expect("spawn zone")
            .spirit_qi;
        let expected_overflow = 1.0;
        assert!(
            (zone_after - 1.0).abs() < QI_EPSILON,
            "0.90 zone 接收 5 点后应到满仓，实际 {zone_after}"
        );
        assert!(
            (ledger_balance(&world, &overflow_account) - expected_overflow).abs() < QI_EPSILON,
            "只有满仓剩余 1 点应落入 overflow ledger，accepted 部分不能双计"
        );
        assert!(
            (world.resource::<WorldQiAccount>().total() - expected_overflow).abs() < QI_EPSILON,
            "ledger total 只能包含 overflow remainder，不能包含 accepted zone 部分"
        );
        let after_total = world.get::<Cultivation>(entity).unwrap().qi_current
            + zone_after * QI_ZONE_UNIT_CAPACITY
            + world.resource::<WorldQiAccount>().total();
        assert!(
            (before_total - after_total).abs() < QI_EPSILON,
            "部分回灌 zone + overflow 时必须守恒：before={before_total}, after={after_total}"
        );
    }

    #[test]
    fn missing_home_zone_ledgers_full_cost_to_overflow() {
        let mut world = world_with_zone_registry();
        insert_qi_ledger(&mut world);
        let entity = world
            .spawn((
                make_cultivation(Realm::Condense, 50.0),
                NpcPatrol::new("missing_zone", DVec3::new(14.0, 66.0, 14.0)),
            ))
            .id();
        let overflow_account = qi_flow_overflow_account();

        npc_buff_speed(&mut world, entity, 0, None);

        assert!(
            (ledger_balance(&world, &overflow_account) - BUFF_SPEED_QI_COST).abs() < QI_EPSILON,
            "缺 home zone 时完整成本必须真实落入 overflow ledger"
        );
    }

    #[test]
    fn missing_zone_registry_ledgers_full_cost_to_overflow() {
        let mut world = world_with_events();
        insert_qi_ledger(&mut world);
        let entity = world
            .spawn((
                make_cultivation(Realm::Condense, 50.0),
                NpcPatrol::new("spawn", DVec3::new(14.0, 66.0, 14.0)),
            ))
            .id();
        let overflow_account = qi_flow_overflow_account();

        npc_buff_speed(&mut world, entity, 0, None);

        assert!(
            (ledger_balance(&world, &overflow_account) - BUFF_SPEED_QI_COST).abs() < QI_EPSILON,
            "缺 ZoneRegistry 时完整成本必须真实落入 overflow ledger"
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
    // 锁定：① 三招各发**专属** particle event_id + color（P5 去复用，旧借用 id 负向锁）
    //       ② 各发对应 reused audio recipe（音效不在 P5 范围）
    //       ③ AV 只在 cast 成功时发，rejection 不发 ④ 缺 Position 时静默跳过不发 AV 也不崩。

    fn make_position() -> Position {
        Position::new([14.0, 66.0, 14.0])
    }

    #[test]
    fn heal_basic_emits_bespoke_heal_particle_and_reused_audio() {
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
            "heal 应发 NPC 专属粒子 id（P5 去复用：原借医道 bong:yidao_meridian_repair，\
             借用时旁观者分不清是 NPC 补血还是有玩家在放医道）"
        );
        assert_eq!(
            particles[0].1.as_deref(),
            Some(HEAL_PARTICLE_COLOR),
            "heal 粒子应为薄荷绿（绿/黄/蓝三元组中的绿）"
        );
        assert_ne!(
            particles[0].0, "bong:yidao_meridian_repair",
            "heal 回退到了 P5 之前借用的医道粒子 id"
        );

        let audio = collected_audio(&world);
        assert_eq!(
            audio,
            vec![HEAL_AUDIO_RECIPE.to_string()],
            "heal 仍复用既有 yidao_meridian_repair 音效配方——P5 只做粒子去复用，音效不在范围内"
        );
    }

    #[test]
    fn buff_speed_emits_bespoke_speed_particle_and_reused_audio() {
        let mut world = world_with_events();
        let entity = world
            .spawn((make_cultivation(Realm::Condense, 50.0), make_position()))
            .id();

        npc_buff_speed(&mut world, entity, 0, None);

        let particles = collected_particles(&world);
        assert_eq!(particles.len(), 1, "buff_speed should emit 1 particle");
        assert_eq!(
            particles[0].0, BUFF_SPEED_PARTICLE_ID,
            "buff_speed 应发 NPC 专属粒子 id（P5 去复用：原借真脉 bong:jiemai_neutralize_dust）"
        );
        assert_eq!(
            particles[0].1.as_deref(),
            Some(BUFF_SPEED_PARTICLE_COLOR),
            "buff_speed 粒子应为麦黄——旧值 #9FD8C8 与 heal 的 #A8E6CF 同属淡青绿、远距离不可辨"
        );
        assert_ne!(
            particles[0].0, "bong:jiemai_neutralize_dust",
            "buff_speed 回退到了 P5 之前借用的真脉粒子 id"
        );

        let audio = collected_audio(&world);
        assert_eq!(
            audio,
            vec![BUFF_SPEED_AUDIO_RECIPE.to_string()],
            "buff_speed 仍复用既有 movement_dash 音效配方（P5 不动音效）"
        );
    }

    #[test]
    fn buff_defense_emits_bespoke_shield_particle_and_reused_audio() {
        let mut world = world_with_events();
        let entity = world
            .spawn((make_cultivation(Realm::Condense, 50.0), make_position()))
            .id();

        npc_buff_defense(&mut world, entity, 0, None);

        let particles = collected_particles(&world);
        assert_eq!(particles.len(), 1, "buff_defense should emit 1 particle");
        assert_eq!(
            particles[0].0, BUFF_DEFENSE_PARTICLE_ID,
            "buff_defense 应发 NPC 专属粒子 id（P5 去复用：原借崩拳 \
             bong:burst_meridian_beng_quan，与玩家爆脉招完全同形）"
        );
        assert_eq!(
            particles[0].1.as_deref(),
            Some(BUFF_DEFENSE_PARTICLE_COLOR),
            "buff_defense 粒子应为青蓝（绿/黄/蓝三元组中的蓝）"
        );
        assert_ne!(
            particles[0].0, "bong:burst_meridian_beng_quan",
            "buff_defense 回退到了 P5 之前借用的崩拳粒子 id"
        );

        let audio = collected_audio(&world);
        assert_eq!(
            audio,
            vec![BUFF_DEFENSE_AUDIO_RECIPE.to_string()],
            "buff_defense 仍复用既有 zhenmai_shield_hum 音效配方（P5 不动音效）"
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

    // ─── plan-skill-anim-fidelity-v1 P5：粒子去借用回归锁 ─────────────────────────

    /// 三招的粒子 id 不得是任何一个曾被借用的别家 id。
    ///
    /// 与上面的「三招互不相同」不可互相替代——三招彼此不同、但仍全是借来的，
    /// 正是 P5 之前的状态（heal 借医道 / speed 借真脉 / defense 借崩拳，三者确实互异）。
    #[test]
    fn p5_no_npc_skill_borrows_another_style_particle() {
        const LEGACY_BORROWED: [&str; 3] = [
            "bong:yidao_meridian_repair",
            "bong:jiemai_neutralize_dust",
            "bong:burst_meridian_beng_quan",
        ];
        for particle_id in [
            HEAL_PARTICLE_ID,
            BUFF_SPEED_PARTICLE_ID,
            BUFF_DEFENSE_PARTICLE_ID,
        ] {
            assert!(
                !LEGACY_BORROWED.contains(&particle_id),
                "{particle_id} 是 P5 之前借用的别家流派粒子——NPC 施法必须有自己的 id，\
                 否则旁观者分不清是 NPC 在放技能还是玩家在放"
            );
            assert!(
                particle_id.starts_with("bong:npc_"),
                "{particle_id} 应落在 bong:npc_ 前缀下——该前缀**有意**不在玩家技能优先级表里，\
                 NPC 背景 cosmetic 归 Normal 档，不与玩家技能反馈争拥挤 chunk 的粒子配额"
            );
        }
    }

    /// 三招接线与 `network::skill_vfx_wiring` 共享表逐项一致（client 按同一份表注册）。
    #[test]
    fn p5_npc_particles_match_shared_wiring_table() {
        for (skill_id, particle_id, color) in [
            ("npc.heal_basic", HEAL_PARTICLE_ID, HEAL_PARTICLE_COLOR),
            (
                "npc.buff_speed",
                BUFF_SPEED_PARTICLE_ID,
                BUFF_SPEED_PARTICLE_COLOR,
            ),
            (
                "npc.buff_defense",
                BUFF_DEFENSE_PARTICLE_ID,
                BUFF_DEFENSE_PARTICLE_COLOR,
            ),
        ] {
            let wiring = crate::network::skill_vfx_wiring::wiring_for(skill_id)
                .unwrap_or_else(|| panic!("{skill_id} 未登记进 P5_SKILL_VFX_WIRING 接线表"));
            assert_eq!(
                wiring.event_id, particle_id,
                "{skill_id} 的 event_id 与共享接线表不一致——client 按表注册，不符即 bridgeMiss"
            );
            assert_eq!(
                wiring.color, color,
                "{skill_id} 的粒子颜色与共享接线表不一致"
            );
        }
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

    // === M08 override：selector 到 resolver 共享同一注入 registry 成本/冷却 ===

    fn world_with_override(
        id: &str,
        override_definition: impl FnOnce(&mut crate::cultivation::known_techniques::TechniqueDefinition),
    ) -> bevy_ecs::world::World {
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(Events::<ApplyStatusEffectIntent>::default());
        world.insert_resource(Events::<QiTransfer>::default());
        world.insert_resource(Events::<VfxEventRequest>::default());
        world.insert_resource(Events::<PlaySoundRecipeRequest>::default());
        world.insert_resource(
            crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests_with_override(
                id,
                override_definition,
            ),
        );
        world
    }

    #[test]
    fn heal_resolver_consumes_overridden_registry_cost_and_cooldown() {
        // M08：override npc.heal_basic 成本到 40.0/冷却 77——qi=20（旧常量 8.0
        // 本可负担）必须被拒；qi=40 才放行且 cooldown 取 77（旧常量 200 必撞红），
        // 证明 resolver 结算读的是注入 registry 而非硬编码常量。
        let mut world = world_with_override(NPC_HEAL_SKILL_ID, |definition| {
            definition.qi_cost = 40.0;
            definition.cooldown_ticks = 77;
        });

        let entity = world
            .spawn((
                make_cultivation(Realm::Induce, 20.0),
                make_wounds(50.0, 100.0, vec![]),
            ))
            .id();
        let result = npc_heal_basic(&mut world, entity, 0, None);
        assert!(
            matches!(
                result,
                CastResult::Rejected {
                    reason: CastRejectReason::QiInsufficient
                }
            ),
            "overridden cost 40.0 must reject qi=20 (constant 8.0 would pass); got {result:?}"
        );

        let entity = world
            .spawn((
                make_cultivation(Realm::Induce, 40.0),
                make_wounds(50.0, 100.0, vec![]),
            ))
            .id();
        let result = npc_heal_basic(&mut world, entity, 0, None);
        assert!(
            matches!(
                result,
                CastResult::Started {
                    cooldown_ticks: 77,
                    anim_duration_ticks: 20
                }
            ),
            "overridden cooldown 77 must be returned (constant 200 would fail); got {result:?}"
        );
        let cult = world.get::<Cultivation>(entity).unwrap();
        assert!(
            (cult.qi_current - 0.0).abs() < f64::EPSILON,
            "qi must be charged by overridden cost 40.0, got {}",
            cult.qi_current
        );
    }

    #[test]
    fn buff_speed_resolver_consumes_overridden_registry_cost_and_cooldown() {
        // M08：override npc.buff_speed 成本到 30.0/冷却 88——qi=6（旧常量 5.0
        // 本可负担）必须被拒；qi=30 才放行、冷却取 88、且发出 SpeedBoost intent。
        let mut world = world_with_override(NPC_BUFF_SPEED_SKILL_ID, |definition| {
            definition.qi_cost = 30.0;
            definition.cooldown_ticks = 88;
        });

        let entity = world
            .spawn((make_cultivation(Realm::Condense, 6.0), make_position()))
            .id();
        let result = npc_buff_speed(&mut world, entity, 0, None);
        assert!(
            matches!(
                result,
                CastResult::Rejected {
                    reason: CastRejectReason::QiInsufficient
                }
            ),
            "overridden cost 30.0 must reject qi=6 (constant 5.0 would pass); got {result:?}"
        );

        let entity = world
            .spawn((make_cultivation(Realm::Condense, 30.0), make_position()))
            .id();
        let result = npc_buff_speed(&mut world, entity, 0, None);
        assert!(
            matches!(
                result,
                CastResult::Started {
                    cooldown_ticks: 88,
                    anim_duration_ticks: 10
                }
            ),
            "overridden cooldown 88 must be returned (constant 400 would fail); got {result:?}"
        );
        let intents = world.resource::<Events<ApplyStatusEffectIntent>>();
        let mut reader = intents.get_reader();
        let intents = reader.read(intents).collect::<Vec<_>>();
        assert_eq!(intents.len(), 1, "SpeedBoost intent must fire on success");
        assert_eq!(intents[0].kind, StatusEffectKind::SpeedBoost);
    }

    #[test]
    fn buff_defense_resolver_consumes_overridden_registry_cost_and_cooldown() {
        // M08：override npc.buff_defense 成本到 25.0/冷却 99——qi=7（旧常量 6.0
        // 本可负担）必须被拒；qi=25 才放行、冷却取 99、且发出 DamageReduction intent。
        let mut world = world_with_override(NPC_BUFF_DEFENSE_SKILL_ID, |definition| {
            definition.qi_cost = 25.0;
            definition.cooldown_ticks = 99;
        });

        let entity = world
            .spawn((make_cultivation(Realm::Condense, 7.0), make_position()))
            .id();
        let result = npc_buff_defense(&mut world, entity, 0, None);
        assert!(
            matches!(
                result,
                CastResult::Rejected {
                    reason: CastRejectReason::QiInsufficient
                }
            ),
            "overridden cost 25.0 must reject qi=7 (constant 6.0 would pass); got {result:?}"
        );

        let entity = world
            .spawn((make_cultivation(Realm::Condense, 25.0), make_position()))
            .id();
        let result = npc_buff_defense(&mut world, entity, 0, None);
        assert!(
            matches!(
                result,
                CastResult::Started {
                    cooldown_ticks: 99,
                    anim_duration_ticks: 10
                }
            ),
            "overridden cooldown 99 must be returned (constant 400 would fail); got {result:?}"
        );
        let intents = world.resource::<Events<ApplyStatusEffectIntent>>();
        let mut reader = intents.get_reader();
        let intents = reader.read(intents).collect::<Vec<_>>();
        assert_eq!(
            intents.len(),
            1,
            "DamageReduction intent must fire on success"
        );
        assert_eq!(intents[0].kind, StatusEffectKind::DamageReduction);
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
