use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Commands, Component, Entity, Event, EventReader, EventWriter, ParamSet, Query, Res,
    UniqueId,
};

use crate::combat::components::{BodyPart, Lifecycle, LifecycleState};
use crate::combat::events::{AttackSource, CombatEvent};
use crate::combat::needle::{
    realm_rank, IntentSource, ShootNeedleIntent, QI_NEEDLE_COOLDOWN_TICKS, QI_NEEDLE_SKILL_ID,
};
use crate::combat::CombatClock;
use crate::cultivation::components::{Cultivation, MeridianId, MeridianSystem, Realm};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::inventory::{
    consume_item_instance_once, inventory_item_by_instance_borrow, PlayerInventory,
};
use crate::network::cast_emit::current_unix_millis;
use crate::schema::dugu::{
    AntidoteResultEventV1, AntidoteResultV1, DuguObfuscationStateV1, DuguPoisonProgressEventV1,
    DuguPoisonStateV1,
};

pub const DUGU_INFUSE_SKILL_ID: &str = "dugu.infuse_poison";
pub const DUGU_INFUSE_COST: f64 = 5.0;
pub const DUGU_INFUSION_TTL_TICKS: u64 = 20 * 60;
pub const DUGU_EXPOSURE_TICKS: u64 = 20 * 5;
pub const DUGU_POISON_TICK_INTERVAL: u64 = 20 * 60 * 5;
pub const SELF_ANTIDOTE_QI_COST: f64 = 20.0;
pub const SELF_ANTIDOTE_FAILURE_RATE: f64 = 0.30;
pub const JIEGU_RUI_ITEM_ID: &str = "jie_gu_rui";

type DuguAttackAttackerItem<'a> = (&'a PendingDuguInfusion, &'a Cultivation);
type DuguAttackTargetItem<'a> = (
    &'a mut MeridianSystem,
    &'a mut Cultivation,
    Option<&'a mut LifeRecord>,
);

#[derive(Debug, Clone, Copy, Component, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct DuguPractice {
    pub dugu_practice_level: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InfuseTarget {
    NextNeedle,
    NextMeleeAttack,
    CarrierSlot { slot: u8 },
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct InfuseDuguPoisonIntent {
    pub infuser: Entity,
    pub target_carrier: InfuseTarget,
    pub source: IntentSource,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct PendingDuguInfusion {
    pub target_carrier: InfuseTarget,
    pub infused_at_tick: u64,
    pub expires_at_tick: u64,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct DuguObfuscationDisrupted {
    pub until_tick: u64,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct DuguPoisonState {
    pub meridian_id: MeridianId,
    pub attacker: Entity,
    pub attached_at_tick: u64,
    pub poisoner_realm_tier: u8,
    pub loss_per_tick: f64,
}

#[derive(Debug, Clone, Event, PartialEq, Serialize, Deserialize)]
pub struct DuguPoisonProgressEvent {
    pub target: Entity,
    pub attacker: Entity,
    pub meridian_id: MeridianId,
    pub flow_capacity_after: f64,
    pub qi_max_after: f64,
    pub actual_loss_this_tick: f64,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq, Serialize, Deserialize)]
pub struct DuguObfuscationDisruptedEvent {
    pub infuser: Entity,
    pub until_tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq, Serialize, Deserialize)]
pub struct DuguRevealedEvent {
    pub revealed_player: Entity,
    pub witness: Entity,
    pub witness_realm: Realm,
    pub at_position: [f64; 3],
    pub at_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AntidoteResult {
    Success,
    Failed,
}

#[derive(Debug, Clone, Event)]
pub struct SelfAntidoteIntent {
    pub healer: Entity,
    pub target: Entity,
    pub antidote_instance_id: u64,
    pub source: IntentSource,
    pub roll_override: Option<f64>,
}

#[derive(Debug, Clone, Event, PartialEq, Serialize, Deserialize)]
pub struct AntidoteResultEvent {
    pub healer: Entity,
    pub target: Entity,
    pub result: AntidoteResult,
    pub meridian_id: MeridianId,
    pub qi_max_after: f64,
    pub tick: u64,
}

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(QI_NEEDLE_SKILL_ID, resolve_shoot_needle_skill);
    registry.register(DUGU_INFUSE_SKILL_ID, resolve_infuse_dugu_poison_skill);
}

pub fn resolve_infuse_dugu_poison_intents(
    mut commands: Commands,
    clock: Res<CombatClock>,
    mut intents: EventReader<InfuseDuguPoisonIntent>,
    mut actors: Query<(
        &mut Cultivation,
        &DuguPractice,
        Option<&Lifecycle>,
        Option<&PendingDuguInfusion>,
    )>,
    mut disrupted_events: EventWriter<DuguObfuscationDisruptedEvent>,
) {
    for intent in intents.read() {
        let Ok((mut cultivation, practice, lifecycle, pending)) = actors.get_mut(intent.infuser)
        else {
            continue;
        };
        if !can_infuse_dugu(&cultivation, practice, lifecycle, pending) {
            continue;
        }

        cultivation.qi_current =
            (cultivation.qi_current - DUGU_INFUSE_COST).clamp(0.0, cultivation.qi_max);
        let expires_at_tick = clock.tick.saturating_add(DUGU_INFUSION_TTL_TICKS);
        let disrupted_until = clock.tick.saturating_add(DUGU_EXPOSURE_TICKS);
        commands.entity(intent.infuser).insert((
            PendingDuguInfusion {
                target_carrier: intent.target_carrier,
                infused_at_tick: clock.tick,
                expires_at_tick,
            },
            DuguObfuscationDisrupted {
                until_tick: disrupted_until,
            },
        ));
        disrupted_events.send(DuguObfuscationDisruptedEvent {
            infuser: intent.infuser,
            until_tick: disrupted_until,
        });
    }
}

pub fn expire_dugu_state(
    mut commands: Commands,
    clock: Res<CombatClock>,
    pending: Query<(Entity, &PendingDuguInfusion)>,
    disrupted: Query<(Entity, &DuguObfuscationDisrupted)>,
) {
    for (entity, pending) in &pending {
        if clock.tick >= pending.expires_at_tick {
            commands.entity(entity).remove::<PendingDuguInfusion>();
        }
    }
    for (entity, disrupted) in &disrupted {
        if clock.tick >= disrupted.until_tick {
            commands.entity(entity).remove::<DuguObfuscationDisrupted>();
        }
    }
}

pub fn on_attack_resolved_dugu_handler(
    mut commands: Commands,
    mut combat_events: EventReader<CombatEvent>,
    mut actors: ParamSet<(
        Query<DuguAttackAttackerItem<'_>>,
        Query<DuguAttackTargetItem<'_>>,
    )>,
) {
    for event in combat_events.read() {
        let Some((target_carrier, poisoner_realm_tier)) = ({
            let attackers = actors.p0();
            let Ok((infusion, attacker_cultivation)) = attackers.get(event.attacker) else {
                continue;
            };
            Some((
                infusion.target_carrier,
                realm_rank(attacker_cultivation.realm),
            ))
        }) else {
            continue;
        };
        if !infusion_matches_attack_source(target_carrier, event.source) {
            continue;
        }
        let mut targets = actors.p1();
        let Ok((meridians, mut cultivation, life_record)) = targets.get_mut(event.target) else {
            commands
                .entity(event.attacker)
                .remove::<PendingDuguInfusion>();
            continue;
        };

        let meridian_id = body_part_to_meridian(event.body_part);
        let meridian = meridians.get(meridian_id);
        if !meridian.opened || meridian.flow_capacity <= f64::EPSILON {
            commands
                .entity(event.attacker)
                .remove::<PendingDuguInfusion>();
            continue;
        }

        let loss_per_tick = meridian.flow_capacity * 0.01;
        commands.entity(event.target).insert(DuguPoisonState {
            meridian_id,
            attacker: event.attacker,
            attached_at_tick: event.resolved_at_tick,
            poisoner_realm_tier,
            loss_per_tick,
        });
        commands
            .entity(event.attacker)
            .remove::<PendingDuguInfusion>();

        if let Some(mut life_record) = life_record {
            life_record.push(BiographyEntry::DuguPoisonInflicted {
                attacker_id: format!("entity:{}", event.attacker.to_bits()),
                target_id: format!("entity:{}", event.target.to_bits()),
                meridian_id,
                tick: event.resolved_at_tick,
            });
        }

        cultivation.qi_max = recompute_qi_max(&meridians);
        cultivation.qi_current = cultivation.qi_current.clamp(0.0, cultivation.qi_max);
    }
}

pub fn dugu_poison_tick(
    mut commands: Commands,
    clock: Res<CombatClock>,
    mut targets: Query<(
        Entity,
        &mut MeridianSystem,
        &mut Cultivation,
        &DuguPoisonState,
    )>,
    mut progress_events: EventWriter<DuguPoisonProgressEvent>,
) {
    if clock.tick == 0 {
        return;
    }

    for (entity, mut meridians, mut cultivation, poison) in &mut targets {
        let elapsed_ticks = clock.tick.saturating_sub(poison.attached_at_tick);
        if elapsed_ticks == 0 || !elapsed_ticks.is_multiple_of(DUGU_POISON_TICK_INTERVAL) {
            continue;
        }
        let meridian = meridians.get_mut(poison.meridian_id);
        if !meridian.opened || meridian.flow_capacity <= f64::EPSILON {
            commands.entity(entity).remove::<DuguPoisonState>();
            continue;
        }

        if poison.poisoner_realm_tier == 0 {
            continue;
        }
        let tier = f64::from(poison.poisoner_realm_tier);
        let scheduled_loss = (poison.loss_per_tick * tier).max(0.0);
        let actual_loss = scheduled_loss.min(meridian.flow_capacity);
        meridian.flow_capacity = (meridian.flow_capacity - actual_loss).max(0.0);
        if meridian.flow_capacity <= f64::EPSILON {
            meridian.flow_capacity = 0.0;
            meridian.opened = false;
            commands.entity(entity).remove::<DuguPoisonState>();
        }

        cultivation.qi_max = recompute_qi_max(&meridians);
        cultivation.qi_current = cultivation.qi_current.clamp(0.0, cultivation.qi_max);
        progress_events.send(DuguPoisonProgressEvent {
            target: entity,
            attacker: poison.attacker,
            meridian_id: poison.meridian_id,
            flow_capacity_after: meridians.get(poison.meridian_id).flow_capacity,
            qi_max_after: cultivation.qi_max,
            actual_loss_this_tick: actual_loss,
            tick: clock.tick,
        });
    }
}

pub fn resolve_self_antidote_intent(
    mut commands: Commands,
    clock: Res<CombatClock>,
    mut intents: EventReader<SelfAntidoteIntent>,
    mut targets: Query<(
        &mut MeridianSystem,
        &mut Cultivation,
        &DuguPoisonState,
        Option<&Lifecycle>,
    )>,
    mut inventories: Query<&mut PlayerInventory>,
    mut result_events: EventWriter<AntidoteResultEvent>,
) {
    for intent in intents.read() {
        if intent.healer != intent.target {
            continue;
        }
        let Ok((mut meridians, mut cultivation, poison, lifecycle)) =
            targets.get_mut(intent.target)
        else {
            continue;
        };
        if lifecycle.is_some_and(|lifecycle| lifecycle.state != LifecycleState::Alive) {
            continue;
        }
        if cultivation.qi_current + f64::EPSILON < SELF_ANTIDOTE_QI_COST {
            continue;
        }
        let Ok(mut inventory) = inventories.get_mut(intent.healer) else {
            continue;
        };
        let Some(item) = inventory_item_by_instance_borrow(&inventory, intent.antidote_instance_id)
        else {
            continue;
        };
        if item.template_id != JIEGU_RUI_ITEM_ID {
            continue;
        }

        if consume_item_instance_once(&mut inventory, intent.antidote_instance_id).is_err() {
            continue;
        }
        cultivation.qi_current =
            (cultivation.qi_current - SELF_ANTIDOTE_QI_COST).clamp(0.0, cultivation.qi_max);

        let roll = intent
            .roll_override
            .unwrap_or_else(|| antidote_roll(intent.healer, clock.tick, inventory.revision.0));
        let failed = roll < SELF_ANTIDOTE_FAILURE_RATE;
        let meridian_id = poison.meridian_id;
        if failed {
            let meridian = meridians.get_mut(meridian_id);
            meridian.flow_capacity = 0.0;
            meridian.opened = false;
        }
        commands.entity(intent.target).remove::<DuguPoisonState>();

        cultivation.qi_max = recompute_qi_max(&meridians);
        cultivation.qi_current = cultivation.qi_current.clamp(0.0, cultivation.qi_max);
        result_events.send(AntidoteResultEvent {
            healer: intent.healer,
            target: intent.target,
            result: if failed {
                AntidoteResult::Failed
            } else {
                AntidoteResult::Success
            },
            meridian_id,
            qi_max_after: cultivation.qi_max,
            tick: clock.tick,
        });
    }
}

pub fn can_infuse_dugu(
    cultivation: &Cultivation,
    practice: &DuguPractice,
    lifecycle: Option<&Lifecycle>,
    pending: Option<&PendingDuguInfusion>,
) -> bool {
    practice.dugu_practice_level >= 1
        && realm_rank(cultivation.realm) >= realm_rank(Realm::Induce)
        && cultivation.qi_current + f64::EPSILON >= DUGU_INFUSE_COST
        && lifecycle.is_none_or(|lifecycle| lifecycle.state == LifecycleState::Alive)
        && pending.is_none()
}

pub fn body_part_to_meridian(body_part: BodyPart) -> MeridianId {
    match body_part {
        BodyPart::Head => MeridianId::Du,
        BodyPart::Chest => MeridianId::Heart,
        BodyPart::Abdomen => MeridianId::Spleen,
        BodyPart::ArmL | BodyPart::ArmR => MeridianId::LargeIntestine,
        BodyPart::LegL | BodyPart::LegR => MeridianId::Bladder,
    }
}

pub fn recompute_qi_max(meridians: &MeridianSystem) -> f64 {
    10.0 + meridians.sum_capacity()
}

pub fn infusion_matches_attack_source(target: InfuseTarget, source: AttackSource) -> bool {
    match target {
        InfuseTarget::NextNeedle => source == AttackSource::QiNeedle,
        InfuseTarget::NextMeleeAttack => {
            matches!(source, AttackSource::Melee | AttackSource::BurstMeridian)
        }
        InfuseTarget::CarrierSlot { .. } => source == AttackSource::QiNeedle,
    }
}

pub fn antidote_roll(entity: Entity, tick: u64, revision: u64) -> f64 {
    let mut x = entity.to_bits() ^ tick.rotate_left(13) ^ revision.rotate_left(27);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
    x ^= x >> 33;
    (x % 10_000) as f64 / 10_000.0
}

pub fn poison_state_payload(
    entity_wire_id: String,
    state: Option<&DuguPoisonState>,
    meridians: Option<&MeridianSystem>,
    cultivation: Option<&Cultivation>,
    clock_tick: u64,
) -> DuguPoisonStateV1 {
    match state {
        Some(state) => DuguPoisonStateV1 {
            target: entity_wire_id,
            active: true,
            meridian_id: format!("{:?}", state.meridian_id),
            attacker: format!("entity:{}", state.attacker.to_bits()),
            attached_at_tick: state.attached_at_tick,
            poisoner_realm_tier: state.poisoner_realm_tier,
            loss_per_tick: state.loss_per_tick,
            flow_capacity_after: meridians
                .map(|meridians| meridians.get(state.meridian_id).flow_capacity)
                .unwrap_or(0.0),
            qi_max_after: cultivation
                .map(|cultivation| cultivation.qi_max)
                .unwrap_or(0.0),
            server_tick: clock_tick,
        },
        None => DuguPoisonStateV1::clear(entity_wire_id, clock_tick),
    }
}

pub fn progress_payload(
    event: &DuguPoisonProgressEvent,
    target_unique_id: Option<&UniqueId>,
    attacker_unique_id: Option<&UniqueId>,
) -> DuguPoisonProgressEventV1 {
    DuguPoisonProgressEventV1 {
        target: unique_or_entity(target_unique_id, event.target),
        attacker: unique_or_entity(attacker_unique_id, event.attacker),
        meridian_id: format!("{:?}", event.meridian_id),
        flow_capacity_after: event.flow_capacity_after,
        qi_max_after: event.qi_max_after,
        actual_loss_this_tick: event.actual_loss_this_tick,
        tick: event.tick,
    }
}

pub fn antidote_payload(
    event: &AntidoteResultEvent,
    healer_unique_id: Option<&UniqueId>,
    target_unique_id: Option<&UniqueId>,
) -> AntidoteResultEventV1 {
    AntidoteResultEventV1 {
        healer: unique_or_entity(healer_unique_id, event.healer),
        target: unique_or_entity(target_unique_id, event.target),
        result: match event.result {
            AntidoteResult::Success => AntidoteResultV1::Success,
            AntidoteResult::Failed => AntidoteResultV1::Failed,
        },
        meridian_id: format!("{:?}", event.meridian_id),
        qi_max_after: event.qi_max_after,
        tick: event.tick,
    }
}

pub fn obfuscation_payload(
    entity_wire_id: String,
    practice: Option<&DuguPractice>,
    disrupted: Option<&DuguObfuscationDisrupted>,
    clock_tick: u64,
) -> DuguObfuscationStateV1 {
    DuguObfuscationStateV1 {
        entity: entity_wire_id,
        active: practice.is_some_and(|practice| practice.dugu_practice_level >= 1),
        disrupted_until_tick: disrupted.map(|state| state.until_tick),
        server_tick: clock_tick,
    }
}

fn unique_or_entity(unique_id: Option<&UniqueId>, entity: Entity) -> String {
    unique_id
        .map(|unique_id| format!("player:{}", unique_id.0))
        .unwrap_or_else(|| format!("entity:{}", entity.to_bits()))
}

fn resolve_shoot_needle_skill(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    let Some(clock) = world.get_resource::<CombatClock>() else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let now_tick = clock.tick;
    if world
        .get::<crate::combat::components::SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
    {
        return rejected(CastRejectReason::OnCooldown);
    }
    let Some(target) = target else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let Some(cultivation) = world.get::<Cultivation>(caster) else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    let Some(stamina) = world.get::<crate::combat::components::Stamina>(caster) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let lifecycle = world.get::<Lifecycle>(caster);
    if !crate::combat::needle::can_shoot_needle(cultivation, stamina, lifecycle) {
        return rejected(CastRejectReason::QiInsufficient);
    }
    world.send_event(ShootNeedleIntent {
        shooter: caster,
        target: Some(target),
        dir_unit: [0.0, 0.0, 1.0],
        source: IntentSource::SkillBar,
    });
    insert_instant_cast(
        world,
        caster,
        slot,
        QI_NEEDLE_SKILL_ID,
        QI_NEEDLE_COOLDOWN_TICKS,
    );
    CastResult::Started {
        cooldown_ticks: QI_NEEDLE_COOLDOWN_TICKS,
        anim_duration_ticks: 1,
    }
}

fn resolve_infuse_dugu_poison_skill(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let Some(clock) = world.get_resource::<CombatClock>() else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let now_tick = clock.tick;
    if world
        .get::<crate::combat::components::SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
    {
        return rejected(CastRejectReason::OnCooldown);
    }
    let Some(cultivation) = world.get::<Cultivation>(caster) else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    let Some(practice) = world.get::<DuguPractice>(caster) else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    let lifecycle = world.get::<Lifecycle>(caster);
    let pending = world.get::<PendingDuguInfusion>(caster);
    if !can_infuse_dugu(cultivation, practice, lifecycle, pending) {
        return rejected(CastRejectReason::QiInsufficient);
    }
    world.send_event(InfuseDuguPoisonIntent {
        infuser: caster,
        target_carrier: InfuseTarget::NextNeedle,
        source: IntentSource::SkillBar,
    });
    insert_instant_cast(world, caster, slot, DUGU_INFUSE_SKILL_ID, 40);
    CastResult::Started {
        cooldown_ticks: 40,
        anim_duration_ticks: 1,
    }
}

fn insert_instant_cast(
    world: &mut bevy_ecs::world::World,
    entity: Entity,
    slot: u8,
    skill_id: &str,
    cooldown_ticks: u64,
) {
    let now_tick = world
        .get_resource::<CombatClock>()
        .map(|clock| clock.tick)
        .unwrap_or(0);
    let start_position = world
        .get::<valence::prelude::Position>(entity)
        .map(|position| position.get())
        .unwrap_or(valence::prelude::DVec3::ZERO);
    world
        .entity_mut(entity)
        .insert(crate::combat::components::Casting {
            source: crate::combat::components::CastSource::SkillBar,
            slot,
            started_at_tick: now_tick,
            duration_ticks: 1,
            started_at_ms: current_unix_millis(),
            duration_ms: 50,
            bound_instance_id: None,
            start_position,
            complete_cooldown_ticks: cooldown_ticks,
            skill_id: Some(skill_id.to_string()),
        });
}

fn rejected(reason: CastRejectReason) -> CastResult {
    CastResult::Rejected { reason }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    use valence::prelude::{App, Events, Update};

    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
        MAIN_PACK_CONTAINER_ID,
    };

    fn open(meridians: &mut MeridianSystem, id: MeridianId, cap: f64) {
        let meridian = meridians.get_mut(id);
        meridian.opened = true;
        meridian.flow_capacity = cap;
    }

    fn jie_gu_rui(instance_id: u64) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: JIEGU_RUI_ITEM_ID.to_string(),
            display_name: "解蛊蕊".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Rare,
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
        }
    }

    fn inventory_with_jie_gu_rui(instance_id: u64) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(5),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: jie_gu_rui(instance_id),
                }],
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    #[test]
    fn body_part_mapping_uses_q58_table() {
        assert_eq!(body_part_to_meridian(BodyPart::Head), MeridianId::Du);
        assert_eq!(body_part_to_meridian(BodyPart::Chest), MeridianId::Heart);
        assert_eq!(body_part_to_meridian(BodyPart::Abdomen), MeridianId::Spleen);
        assert_eq!(
            body_part_to_meridian(BodyPart::ArmL),
            MeridianId::LargeIntestine
        );
        assert_eq!(
            body_part_to_meridian(BodyPart::ArmR),
            MeridianId::LargeIntestine
        );
        assert_eq!(body_part_to_meridian(BodyPart::LegL), MeridianId::Bladder);
        assert_eq!(body_part_to_meridian(BodyPart::LegR), MeridianId::Bladder);
    }

    #[test]
    fn infuse_dugu_consumes_qi_and_opens_exposure_window() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 100 });
        app.add_event::<InfuseDuguPoisonIntent>();
        app.add_event::<DuguObfuscationDisruptedEvent>();
        app.add_systems(Update, resolve_infuse_dugu_poison_intents);
        let infuser = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Induce,
                    qi_current: 12.0,
                    qi_max: 20.0,
                    ..Cultivation::default()
                },
                DuguPractice {
                    dugu_practice_level: 1,
                },
                Lifecycle::default(),
            ))
            .id();

        app.world_mut().send_event(InfuseDuguPoisonIntent {
            infuser,
            target_carrier: InfuseTarget::NextNeedle,
            source: IntentSource::Test,
        });

        app.update();

        let cultivation = app.world().get::<Cultivation>(infuser).unwrap();
        assert_eq!(cultivation.qi_current, 7.0);
        let pending = app.world().get::<PendingDuguInfusion>(infuser).unwrap();
        assert_eq!(pending.target_carrier, InfuseTarget::NextNeedle);
        assert_eq!(pending.expires_at_tick, 100 + DUGU_INFUSION_TTL_TICKS);
        let disrupted = app
            .world()
            .get::<DuguObfuscationDisrupted>(infuser)
            .unwrap();
        assert_eq!(disrupted.until_tick, 100 + DUGU_EXPOSURE_TICKS);
        let events = app
            .world()
            .resource::<Events<DuguObfuscationDisruptedEvent>>();
        assert!(events.get_reader().read(events).any(|event| {
            event.infuser == infuser && event.until_tick == 100 + DUGU_EXPOSURE_TICKS
        }));
    }

    #[test]
    fn awaken_realm_cannot_infuse_dugu_even_with_practice_flag() {
        let cultivation = Cultivation {
            realm: Realm::Awaken,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Cultivation::default()
        };
        let practice = DuguPractice {
            dugu_practice_level: 1,
        };

        assert!(!can_infuse_dugu(&cultivation, &practice, None, None));
    }

    #[test]
    fn pending_needle_poison_attaches_state_and_records_biography() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_systems(Update, on_attack_resolved_dugu_handler);
        let attacker = app
            .world_mut()
            .spawn((
                PendingDuguInfusion {
                    target_carrier: InfuseTarget::NextNeedle,
                    infused_at_tick: 10,
                    expires_at_tick: 1_210,
                },
                Cultivation {
                    realm: Realm::Condense,
                    ..Cultivation::default()
                },
            ))
            .id();
        let mut meridians = MeridianSystem::default();
        open(&mut meridians, MeridianId::Heart, 100.0);
        let target = app
            .world_mut()
            .spawn((
                meridians,
                Cultivation {
                    qi_current: 80.0,
                    qi_max: 110.0,
                    ..Cultivation::default()
                },
                LifeRecord::default(),
            ))
            .id();
        app.world_mut().send_event(CombatEvent {
            attacker,
            target,
            resolved_at_tick: 33,
            body_part: BodyPart::Chest,
            wound_kind: crate::combat::components::WoundKind::Pierce,
            source: AttackSource::QiNeedle,
            damage: 1.0,
            contam_delta: 0.0,
            description: "qi needle hit".to_string(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        });

        app.update();

        assert!(app.world().get::<PendingDuguInfusion>(attacker).is_none());
        let state = app.world().get::<DuguPoisonState>(target).unwrap();
        assert_eq!(state.meridian_id, MeridianId::Heart);
        assert_eq!(state.attacker, attacker);
        assert_eq!(state.poisoner_realm_tier, 2);
        assert_eq!(state.loss_per_tick, 1.0);
        let life_record = app.world().get::<LifeRecord>(target).unwrap();
        assert!(matches!(
            life_record.biography.last(),
            Some(BiographyEntry::DuguPoisonInflicted {
                meridian_id: MeridianId::Heart,
                tick: 33,
                ..
            })
        ));
    }

    #[test]
    fn pending_needle_poison_does_not_attach_to_plain_melee() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_systems(Update, on_attack_resolved_dugu_handler);
        let attacker = app
            .world_mut()
            .spawn((
                PendingDuguInfusion {
                    target_carrier: InfuseTarget::NextNeedle,
                    infused_at_tick: 10,
                    expires_at_tick: 1_210,
                },
                Cultivation::default(),
            ))
            .id();
        let mut meridians = MeridianSystem::default();
        open(&mut meridians, MeridianId::Heart, 100.0);
        let target = app
            .world_mut()
            .spawn((meridians, Cultivation::default(), LifeRecord::default()))
            .id();
        app.world_mut().send_event(CombatEvent {
            attacker,
            target,
            resolved_at_tick: 34,
            body_part: BodyPart::Chest,
            wound_kind: crate::combat::components::WoundKind::Blunt,
            source: AttackSource::Melee,
            damage: 1.0,
            contam_delta: 0.0,
            description: "plain melee hit".to_string(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        });

        app.update();

        assert!(app.world().get::<DuguPoisonState>(target).is_none());
        assert!(app.world().get::<PendingDuguInfusion>(attacker).is_some());
    }

    #[test]
    fn poison_tick_permanently_reduces_flow_capacity_and_qi_max() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: DUGU_POISON_TICK_INTERVAL,
        });
        app.add_event::<DuguPoisonProgressEvent>();
        app.add_systems(Update, dugu_poison_tick);
        let attacker = app.world_mut().spawn_empty().id();
        let mut meridians = MeridianSystem::default();
        open(&mut meridians, MeridianId::Heart, 100.0);
        let target = app
            .world_mut()
            .spawn((
                meridians,
                Cultivation {
                    qi_current: 80.0,
                    qi_max: 110.0,
                    ..Cultivation::default()
                },
                DuguPoisonState {
                    meridian_id: MeridianId::Heart,
                    attacker,
                    attached_at_tick: 0,
                    poisoner_realm_tier: 2,
                    loss_per_tick: 1.0,
                },
            ))
            .id();

        app.update();

        let meridians = app.world().get::<MeridianSystem>(target).unwrap();
        assert_eq!(meridians.get(MeridianId::Heart).flow_capacity, 98.0);
        let cultivation = app.world().get::<Cultivation>(target).unwrap();
        assert_eq!(cultivation.qi_max, 108.0);
        let events = app.world().resource::<Events<DuguPoisonProgressEvent>>();
        assert!(events
            .get_reader()
            .read(events)
            .any(|event| event.actual_loss_this_tick == 2.0));
    }

    #[test]
    fn poison_tick_waits_full_interval_from_attachment_tick() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: DUGU_POISON_TICK_INTERVAL,
        });
        app.add_event::<DuguPoisonProgressEvent>();
        app.add_systems(Update, dugu_poison_tick);
        let attacker = app.world_mut().spawn_empty().id();
        let mut meridians = MeridianSystem::default();
        open(&mut meridians, MeridianId::Heart, 100.0);
        let target = app
            .world_mut()
            .spawn((
                meridians,
                Cultivation {
                    qi_current: 80.0,
                    qi_max: 110.0,
                    ..Cultivation::default()
                },
                DuguPoisonState {
                    meridian_id: MeridianId::Heart,
                    attacker,
                    attached_at_tick: DUGU_POISON_TICK_INTERVAL - 1,
                    poisoner_realm_tier: 2,
                    loss_per_tick: 1.0,
                },
            ))
            .id();

        app.update();

        assert_eq!(
            app.world()
                .get::<MeridianSystem>(target)
                .unwrap()
                .get(MeridianId::Heart)
                .flow_capacity,
            100.0
        );
        app.world_mut().resource_mut::<CombatClock>().tick =
            (DUGU_POISON_TICK_INTERVAL - 1) + DUGU_POISON_TICK_INTERVAL;

        app.update();

        assert_eq!(
            app.world()
                .get::<MeridianSystem>(target)
                .unwrap()
                .get(MeridianId::Heart)
                .flow_capacity,
            98.0
        );
    }

    #[test]
    fn poison_tick_reports_only_actual_drained_capacity() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: DUGU_POISON_TICK_INTERVAL,
        });
        app.add_event::<DuguPoisonProgressEvent>();
        app.add_systems(Update, dugu_poison_tick);
        let attacker = app.world_mut().spawn_empty().id();
        let mut meridians = MeridianSystem::default();
        open(&mut meridians, MeridianId::Heart, 0.7);
        let target = app
            .world_mut()
            .spawn((
                meridians,
                Cultivation {
                    qi_current: 10.0,
                    qi_max: 10.7,
                    ..Cultivation::default()
                },
                DuguPoisonState {
                    meridian_id: MeridianId::Heart,
                    attacker,
                    attached_at_tick: 0,
                    poisoner_realm_tier: 2,
                    loss_per_tick: 1.0,
                },
            ))
            .id();

        app.update();

        let meridians = app.world().get::<MeridianSystem>(target).unwrap();
        assert_eq!(meridians.get(MeridianId::Heart).flow_capacity, 0.0);
        let events = app.world().resource::<Events<DuguPoisonProgressEvent>>();
        assert!(events
            .get_reader()
            .read(events)
            .any(|event| event.actual_loss_this_tick == 0.7));
    }

    #[test]
    fn shoot_needle_skill_rejects_missing_target_without_casting() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 100 });
        app.add_event::<ShootNeedleIntent>();
        let caster = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Induce,
                    qi_current: 10.0,
                    qi_max: 20.0,
                    ..Cultivation::default()
                },
                crate::combat::components::Stamina::default(),
                Lifecycle::default(),
            ))
            .id();

        let result = resolve_shoot_needle_skill(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::InvalidTarget
            }
        );
        assert!(app
            .world()
            .get::<crate::combat::components::Casting>(caster)
            .is_none());
        let events = app.world().resource::<Events<ShootNeedleIntent>>();
        assert_eq!(events.get_reader().read(events).count(), 0);
    }

    #[test]
    fn antidote_success_removes_poison_without_restoring_capacity() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 30 });
        app.add_event::<SelfAntidoteIntent>();
        app.add_event::<AntidoteResultEvent>();
        app.add_systems(Update, resolve_self_antidote_intent);
        let mut meridians = MeridianSystem::default();
        open(&mut meridians, MeridianId::Heart, 70.0);
        let inventory = inventory_with_jie_gu_rui(7);
        let entity = app
            .world_mut()
            .spawn((
                meridians,
                Cultivation {
                    qi_current: 40.0,
                    qi_max: 80.0,
                    ..Cultivation::default()
                },
                DuguPoisonState {
                    meridian_id: MeridianId::Heart,
                    attacker: Entity::from_raw(99),
                    attached_at_tick: 1,
                    poisoner_realm_tier: 2,
                    loss_per_tick: 0.7,
                },
                Lifecycle::default(),
                inventory,
            ))
            .id();
        app.world_mut().send_event(SelfAntidoteIntent {
            healer: entity,
            target: entity,
            antidote_instance_id: 7,
            source: IntentSource::Test,
            roll_override: Some(0.95),
        });

        app.update();

        assert!(app.world().get::<DuguPoisonState>(entity).is_none());
        let meridians = app.world().get::<MeridianSystem>(entity).unwrap();
        assert_eq!(meridians.get(MeridianId::Heart).flow_capacity, 70.0);
        let cultivation = app.world().get::<Cultivation>(entity).unwrap();
        assert_eq!(cultivation.qi_current, 20.0);
        assert_eq!(cultivation.qi_max, 80.0);
    }

    #[test]
    fn antidote_failure_severs_meridian_without_near_death() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 31 });
        app.add_event::<SelfAntidoteIntent>();
        app.add_event::<AntidoteResultEvent>();
        app.add_systems(Update, resolve_self_antidote_intent);
        let mut meridians = MeridianSystem::default();
        open(&mut meridians, MeridianId::Heart, 70.0);
        let inventory = inventory_with_jie_gu_rui(8);
        let entity = app
            .world_mut()
            .spawn((
                meridians,
                Cultivation {
                    qi_current: 40.0,
                    qi_max: 80.0,
                    ..Cultivation::default()
                },
                DuguPoisonState {
                    meridian_id: MeridianId::Heart,
                    attacker: Entity::from_raw(99),
                    attached_at_tick: 1,
                    poisoner_realm_tier: 2,
                    loss_per_tick: 0.7,
                },
                Lifecycle::default(),
                inventory,
            ))
            .id();
        app.world_mut().send_event(SelfAntidoteIntent {
            healer: entity,
            target: entity,
            antidote_instance_id: 8,
            source: IntentSource::Test,
            roll_override: Some(0.1),
        });

        app.update();

        let meridians = app.world().get::<MeridianSystem>(entity).unwrap();
        assert!(!meridians.get(MeridianId::Heart).opened);
        assert_eq!(meridians.get(MeridianId::Heart).flow_capacity, 0.0);
        assert_eq!(
            app.world().get::<Lifecycle>(entity).unwrap().state,
            LifecycleState::Alive
        );
    }
}
