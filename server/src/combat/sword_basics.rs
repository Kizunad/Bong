use valence::prelude::{
    bevy_ecs, bevy_ecs::system::SystemParam, Component, DVec3, Entity, Events, Position, Query,
    Res, ResMut, UniqueId,
};

use crate::combat::components::{
    CastSource, Casting, SkillBarBindings, Stamina, StaminaState, StatusEffects, WoundKind,
    TICKS_PER_SECOND,
};
use crate::combat::events::{AttackIntent, AttackReach, AttackSource, StatusEffectKind};
use crate::combat::status::{has_active_status, upsert_status_effect};
use crate::combat::weapon::{Weapon, WeaponKind};
use crate::combat::CombatClock;
use crate::cultivation::components::{ColorKind, Cultivation, QiColor, Realm};
use crate::cultivation::known_techniques::KnownTechniques;
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::meridian::severed::SkillMeridianDependencies;
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::network::audio_event_emit::{
    AudioRecipient, PlaySoundRecipeRequest, AUDIO_BROADCAST_RADIUS,
};
use crate::network::cast_emit::current_unix_millis;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::qi_physics::{
    qi_excretion_loss, ContainerKind, EnvField, QiAccountId, QiTransfer, QiTransferReason,
};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

pub const SWORD_CLEAVE_SKILL_ID: &str = "sword.cleave";
pub const SWORD_THRUST_SKILL_ID: &str = "sword.thrust";
pub const SWORD_PARRY_SKILL_ID: &str = "sword.parry";
pub const SWORD_INFUSE_SKILL_ID: &str = "sword.infuse";

const SWORD_INFUSE_MIN_QI: f64 = 5.0;
const SWORD_INFUSE_MAX_FRACTION: f64 = 0.5;
const SWORD_INFUSE_HITS: f64 = 5.0;
const SWORD_INFUSE_DURATION_TICKS: u64 = 60 * TICKS_PER_SECOND;
const SWORD_QI_STORE_TICK_INTERVAL: u64 = TICKS_PER_SECOND;
pub const SWORD_PARRY_STAGGER_TICKS: u64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwordTechnique {
    Cleave,
    Thrust,
    Parry,
    Infuse,
}

impl SwordTechnique {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Cleave => SWORD_CLEAVE_SKILL_ID,
            Self::Thrust => SWORD_THRUST_SKILL_ID,
            Self::Parry => SWORD_PARRY_SKILL_ID,
            Self::Infuse => SWORD_INFUSE_SKILL_ID,
        }
    }

    const fn base_stamina_cost(self) -> f32 {
        match self {
            Self::Cleave => 8.0,
            Self::Thrust => 4.0,
            Self::Parry => 6.0,
            Self::Infuse => 3.0,
        }
    }
}

#[derive(Debug, Clone, Component, PartialEq)]
pub struct SwordQiStore {
    pub stored_qi: f64,
    pub qi_per_hit: f64,
    pub remaining_ticks: u64,
    pub infuser_color: ColorKind,
    pub weapon_instance_id: u64,
    pub container_account: QiAccountId,
    pub carrier: ContainerKind,
}

#[derive(Debug, Clone, Component, PartialEq)]
pub struct PendingSwordInfuse {
    pub amount: f64,
    pub complete_at_tick: u64,
    pub slot: u8,
    pub weapon_instance_id: u64,
    pub carrier: ContainerKind,
    pub infuser_color: ColorKind,
    pub container_account: QiAccountId,
}

#[derive(SystemParam)]
pub struct SwordInfuseCompletionParams<'w, 's> {
    commands: valence::prelude::Commands<'w, 's>,
    pending: Query<'w, 's, (Entity, &'static PendingSwordInfuse)>,
    weapons: Query<'w, 's, &'static Weapon>,
    cultivations: Query<'w, 's, &'static mut Cultivation>,
    positions: Query<'w, 's, &'static Position>,
    unique_ids: Query<'w, 's, &'static UniqueId>,
    qi_transfers: Option<ResMut<'w, Events<QiTransfer>>>,
    vfx_events: Option<ResMut<'w, Events<VfxEventRequest>>>,
    audio_events: Option<ResMut<'w, Events<PlaySoundRecipeRequest>>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwordTechniqueProfile {
    pub stamina_cost: f32,
    pub cast_ticks: u32,
    pub cooldown_ticks: u64,
    pub range: f32,
    pub damage_multiplier: f32,
    pub parry_window_ticks: u64,
    pub block_ratio: f32,
}

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(SWORD_CLEAVE_SKILL_ID, cast_sword_cleave);
    registry.register(SWORD_THRUST_SKILL_ID, cast_sword_thrust);
    registry.register(SWORD_PARRY_SKILL_ID, cast_sword_parry);
    registry.register(SWORD_INFUSE_SKILL_ID, cast_sword_infuse);
}

pub fn declare_meridian_dependencies(dependencies: &mut SkillMeridianDependencies) {
    for id in [
        SWORD_CLEAVE_SKILL_ID,
        SWORD_THRUST_SKILL_ID,
        SWORD_PARRY_SKILL_ID,
        SWORD_INFUSE_SKILL_ID,
    ] {
        dependencies.declare(id, Vec::new());
    }
}

pub fn sword_profile(technique: SwordTechnique, proficiency: f32) -> SwordTechniqueProfile {
    let prof = proficiency.clamp(0.0, 1.0);
    match technique {
        SwordTechnique::Cleave => SwordTechniqueProfile {
            stamina_cost: lerp(8.0, 5.0, prof),
            cast_ticks: lerp_round(16.0, 10.0, prof),
            cooldown_ticks: u64::from(lerp_round(30.0, 22.0, prof)),
            range: 3.0,
            damage_multiplier: 1.0 + prof * 0.3,
            parry_window_ticks: 0,
            block_ratio: 0.0,
        },
        SwordTechnique::Thrust => SwordTechniqueProfile {
            stamina_cost: lerp(4.0, 2.0, prof),
            cast_ticks: lerp_round(10.0, 7.0, prof),
            cooldown_ticks: u64::from(lerp_round(20.0, 14.0, prof)),
            range: lerp(3.5, 4.0, prof),
            damage_multiplier: 0.75 + prof * 0.19,
            parry_window_ticks: 0,
            block_ratio: 0.0,
        },
        SwordTechnique::Parry => SwordTechniqueProfile {
            stamina_cost: lerp(6.0, 4.0, prof),
            cast_ticks: 4,
            cooldown_ticks: u64::from(lerp_round(40.0, 30.0, prof)),
            range: 0.0,
            damage_multiplier: 0.0,
            parry_window_ticks: 4 + (prof * 4.0).floor() as u64,
            block_ratio: 0.3 + prof * 0.3,
        },
        SwordTechnique::Infuse => SwordTechniqueProfile {
            stamina_cost: SwordTechnique::Infuse.base_stamina_cost(),
            cast_ticks: 40,
            cooldown_ticks: 100,
            range: 0.0,
            damage_multiplier: 0.0,
            parry_window_ticks: 0,
            block_ratio: 0.0,
        },
    }
}

pub fn sword_proficiency_label(proficiency: f32) -> &'static str {
    let prof = proficiency.clamp(0.0, 1.0);
    if prof < 0.20 {
        "生疏"
    } else if prof < 0.50 {
        "入门"
    } else if prof < 0.80 {
        "熟练"
    } else if prof < 0.95 {
        "精通"
    } else {
        "化境"
    }
}

pub fn sword_proficiency_gain(current: f32, successful: bool, parry_bonus: bool) -> f32 {
    let current = current.clamp(0.0, 1.0);
    let base = if successful {
        if current < 0.50 {
            0.010
        } else if current < 0.80 {
            0.005
        } else if current < 0.95 {
            0.003
        } else {
            0.001
        }
    } else {
        0.002
    };
    if parry_bonus {
        base + 0.005
    } else {
        base
    }
}

pub fn is_sword_attack_source(source: AttackSource) -> bool {
    matches!(
        source,
        AttackSource::SwordCleave
            | AttackSource::SwordThrust
            | AttackSource::SwordPathCondenseEdge
            | AttackSource::SwordPathQiSlash
            | AttackSource::SwordPathResonance
            | AttackSource::SwordPathManifest
            | AttackSource::SwordPathHeavenGate
    )
}

pub fn source_to_technique(source: AttackSource) -> Option<SwordTechnique> {
    match source {
        AttackSource::SwordCleave => Some(SwordTechnique::Cleave),
        AttackSource::SwordThrust => Some(SwordTechnique::Thrust),
        _ => None,
    }
}

pub fn record_sword_parry_success(world: &mut bevy_ecs::world::World, defender: Entity) {
    apply_known_gain(world, defender, SwordTechnique::Parry, true, true);
}

pub fn track_sword_proficiency_from_hits(
    mut events: valence::prelude::EventReader<crate::combat::events::CombatEvent>,
    mut players: Query<&mut KnownTechniques>,
) {
    for event in events.read() {
        let Some(technique) = source_to_technique(event.source) else {
            continue;
        };
        if event.damage <= 0.0 && event.physical_damage <= 0.0 {
            continue;
        }
        let Ok(mut known) = players.get_mut(event.attacker) else {
            continue;
        };
        let Some(entry) = known
            .entries
            .iter_mut()
            .find(|entry| entry.id == technique.id())
        else {
            continue;
        };
        let gain = sword_proficiency_gain(entry.proficiency, true, false);
        entry.proficiency = (entry.proficiency + gain).clamp(0.0, 1.0);
    }
}

pub fn sword_qi_store_tick(
    clock: Res<CombatClock>,
    mut commands: valence::prelude::Commands,
    mut stores: Query<(Entity, &mut SwordQiStore)>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
) {
    if !clock.tick.is_multiple_of(SWORD_QI_STORE_TICK_INTERVAL) {
        return;
    }
    for (entity, mut store) in &mut stores {
        if store.remaining_ticks == 0 || store.stored_qi <= f64::EPSILON {
            commands.entity(entity).remove::<SwordQiStore>();
            continue;
        }
        let elapsed_secs = SWORD_QI_STORE_TICK_INTERVAL as f64 / TICKS_PER_SECOND as f64;
        let loss = qi_excretion_loss(
            store.stored_qi,
            store.carrier,
            elapsed_secs,
            EnvField::new(0.0),
        )
        .min(store.stored_qi);
        if loss > f64::EPSILON {
            store.stored_qi = (store.stored_qi - loss).max(0.0);
            emit_qi_transfer(
                qi_transfers.as_deref_mut(),
                store.container_account.clone(),
                QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
                loss,
                QiTransferReason::Excretion,
            );
        }
        store.remaining_ticks = store
            .remaining_ticks
            .saturating_sub(SWORD_QI_STORE_TICK_INTERVAL);
        if store.remaining_ticks == 0 || store.stored_qi <= f64::EPSILON {
            commands.entity(entity).remove::<SwordQiStore>();
        }
    }
}

pub fn sword_infuse_completion_tick(
    clock: Res<CombatClock>,
    mut params: SwordInfuseCompletionParams,
) {
    for (entity, pending) in &mut params.pending {
        if clock.tick < pending.complete_at_tick {
            continue;
        }
        let valid_weapon = params.weapons.get(entity).is_ok_and(|weapon| {
            weapon.weapon_kind == WeaponKind::Sword
                && weapon.instance_id == pending.weapon_instance_id
        });
        if !valid_weapon {
            params
                .commands
                .entity(entity)
                .remove::<PendingSwordInfuse>();
            continue;
        }
        let Ok(mut cultivation) = params.cultivations.get_mut(entity) else {
            params
                .commands
                .entity(entity)
                .remove::<PendingSwordInfuse>();
            continue;
        };
        if cultivation.qi_current + f64::EPSILON < pending.amount {
            params
                .commands
                .entity(entity)
                .remove::<PendingSwordInfuse>();
            continue;
        }
        cultivation.qi_current =
            (cultivation.qi_current - pending.amount).clamp(0.0, cultivation.qi_max);
        emit_qi_transfer(
            params.qi_transfers.as_deref_mut(),
            player_account_id_for_entity(entity, None),
            pending.container_account.clone(),
            pending.amount,
            QiTransferReason::Channeling,
        );
        params.commands.entity(entity).insert(SwordQiStore {
            stored_qi: pending.amount,
            qi_per_hit: pending.amount / SWORD_INFUSE_HITS,
            remaining_ticks: SWORD_INFUSE_DURATION_TICKS,
            infuser_color: pending.infuser_color,
            weapon_instance_id: pending.weapon_instance_id,
            container_account: pending.container_account.clone(),
            carrier: pending.carrier,
        });
        if let (Some(events), Ok(position)) = (
            params.vfx_events.as_deref_mut(),
            params.positions.get(entity),
        ) {
            emit_particle(
                events,
                position.get() + DVec3::new(0.0, 1.0, 0.0),
                "bong:sword_infuse_glow",
                color_hex(pending.infuser_color),
                0.85,
                8,
                40,
            );
            if let Ok(unique_id) = params.unique_ids.get(entity) {
                events.send(VfxEventRequest::new(
                    position.get(),
                    VfxEventPayloadV1::PlayAnim {
                        target_player: unique_id.0.to_string(),
                        anim_id: "bong:sword_infuse".to_string(),
                        priority: 1200,
                        fade_in_ticks: Some(2),
                    },
                ));
            }
        }
        if let (Some(events), Ok(position)) = (
            params.audio_events.as_deref_mut(),
            params.positions.get(entity),
        ) {
            emit_audio(events, "sword_infuse", entity, position.get());
        }
        params
            .commands
            .entity(entity)
            .remove::<PendingSwordInfuse>();
    }
}

pub fn drain_sword_qi_for_hit(world: &mut bevy_ecs::world::World, caster: Entity) -> f32 {
    let Some(mut store) = world.get_mut::<SwordQiStore>(caster) else {
        return 0.0;
    };
    if store.stored_qi <= f64::EPSILON || store.remaining_ticks == 0 {
        return 0.0;
    }
    let spent = store.qi_per_hit.min(store.stored_qi).max(0.0);
    store.stored_qi = (store.stored_qi - spent).max(0.0);
    spent as f32
}

fn cast_sword_cleave(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    cast_sword_attack(world, caster, slot, target, SwordTechnique::Cleave)
}

fn cast_sword_thrust(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    cast_sword_attack(world, caster, slot, target, SwordTechnique::Thrust)
}

fn cast_sword_attack(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
    technique: SwordTechnique,
) -> CastResult {
    let Some(target) = target else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let now_tick = current_tick(world);
    if is_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    if !has_sword(world, caster) {
        return rejected(CastRejectReason::InvalidTarget);
    }
    if exhausted(world, caster) {
        return rejected(CastRejectReason::InRecovery);
    }
    let Some(proficiency) = known_active_proficiency(world, caster, technique) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let profile = sword_profile(technique, proficiency);
    spend_stamina(world, caster, profile.stamina_cost);
    set_cooldown(
        world,
        caster,
        slot,
        now_tick.saturating_add(profile.cooldown_ticks),
    );
    let qi_invest = drain_sword_qi_for_hit(world, caster);
    world.send_event(AttackIntent {
        attacker: caster,
        target: Some(target),
        issued_at_tick: now_tick,
        reach: AttackReach::new(profile.range, 0.0),
        qi_invest,
        wound_kind: match technique {
            SwordTechnique::Cleave => WoundKind::Cut,
            SwordTechnique::Thrust => WoundKind::Pierce,
            _ => WoundKind::Cut,
        },
        source: match technique {
            SwordTechnique::Cleave => AttackSource::SwordCleave,
            SwordTechnique::Thrust => AttackSource::SwordThrust,
            _ => AttackSource::Melee,
        },
        debug_command: None,
    });
    CastResult::Started {
        cooldown_ticks: profile.cooldown_ticks,
        anim_duration_ticks: profile.cast_ticks,
    }
}

fn cast_sword_parry(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = current_tick(world);
    if is_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    if !has_sword(world, caster) {
        return rejected(CastRejectReason::InvalidTarget);
    }
    if exhausted(world, caster) {
        return rejected(CastRejectReason::InRecovery);
    }
    let Some(proficiency) = known_active_proficiency(world, caster, SwordTechnique::Parry) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let profile = sword_profile(SwordTechnique::Parry, proficiency);
    spend_stamina(world, caster, profile.stamina_cost);
    set_cooldown(
        world,
        caster,
        slot,
        now_tick.saturating_add(profile.cooldown_ticks),
    );
    if let Some(mut statuses) = world.get_mut::<StatusEffects>(caster) {
        upsert_status_effect(
            &mut statuses,
            crate::combat::components::ActiveStatusEffect {
                kind: StatusEffectKind::SwordParrying,
                magnitude: profile.block_ratio,
                remaining_ticks: profile.parry_window_ticks,
            },
        );
    }
    apply_known_gain(world, caster, SwordTechnique::Parry, false, false);
    emit_self_visuals(
        world,
        caster,
        "bong:sword_parry",
        "bong:sword_parry_spark",
        "#FFD080",
        1200,
    );
    CastResult::Started {
        cooldown_ticks: profile.cooldown_ticks,
        anim_duration_ticks: profile.cast_ticks,
    }
}

fn cast_sword_infuse(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = current_tick(world);
    if is_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    if cultivation.realm == Realm::Awaken {
        return rejected(CastRejectReason::RealmTooLow);
    }
    let Some(weapon) = world
        .get::<Weapon>(caster)
        .cloned()
        .filter(|weapon| weapon.weapon_kind == WeaponKind::Sword)
    else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if exhausted(world, caster) {
        return rejected(CastRejectReason::InRecovery);
    }
    let Some(proficiency) = known_active_proficiency(world, caster, SwordTechnique::Infuse) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let profile = sword_profile(SwordTechnique::Infuse, proficiency);
    let amount = (cultivation.qi_current * SWORD_INFUSE_MAX_FRACTION)
        .max(0.0)
        .min(cultivation.qi_current);
    if amount < SWORD_INFUSE_MIN_QI {
        return rejected(CastRejectReason::QiInsufficient);
    }
    spend_stamina(world, caster, profile.stamina_cost);
    set_cooldown(
        world,
        caster,
        slot,
        now_tick.saturating_add(profile.cooldown_ticks),
    );
    insert_casting(
        world,
        caster,
        slot,
        SWORD_INFUSE_SKILL_ID,
        profile,
        now_tick,
    );
    let color = world
        .get::<QiColor>(caster)
        .map(|color| color.main)
        .unwrap_or(ColorKind::Mellow);
    world.entity_mut(caster).insert(PendingSwordInfuse {
        amount,
        complete_at_tick: now_tick.saturating_add(u64::from(profile.cast_ticks)),
        slot,
        weapon_instance_id: weapon.instance_id,
        carrier: carrier_for_quality(weapon.quality_tier),
        infuser_color: color,
        container_account: QiAccountId::container(format!(
            "sword_qi_store:{caster:?}:{}",
            weapon.instance_id
        )),
    });
    emit_self_visuals(
        world,
        caster,
        "bong:sword_infuse",
        "bong:sword_infuse_glow",
        color_hex(color),
        1200,
    );
    CastResult::Started {
        cooldown_ticks: profile.cooldown_ticks,
        anim_duration_ticks: profile.cast_ticks,
    }
}

fn known_active_proficiency(
    world: &bevy_ecs::world::World,
    caster: Entity,
    technique: SwordTechnique,
) -> Option<f32> {
    world
        .get::<KnownTechniques>(caster)?
        .entries
        .iter()
        .find(|entry| entry.id == technique.id() && entry.active)
        .map(|entry| entry.proficiency.clamp(0.0, 1.0))
}

fn apply_known_gain(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    technique: SwordTechnique,
    successful: bool,
    parry_bonus: bool,
) {
    let Some(mut known) = world.get_mut::<KnownTechniques>(caster) else {
        return;
    };
    let Some(entry) = known
        .entries
        .iter_mut()
        .find(|entry| entry.id == technique.id())
    else {
        return;
    };
    let gain = sword_proficiency_gain(entry.proficiency, successful, parry_bonus);
    entry.proficiency = (entry.proficiency + gain).clamp(0.0, 1.0);
}

fn has_sword(world: &bevy_ecs::world::World, caster: Entity) -> bool {
    world
        .get::<Weapon>(caster)
        .is_some_and(|weapon| weapon.weapon_kind == WeaponKind::Sword)
}

fn exhausted(world: &bevy_ecs::world::World, caster: Entity) -> bool {
    world
        .get::<Stamina>(caster)
        .is_some_and(|stamina| stamina.state == StaminaState::Exhausted || stamina.current <= 0.0)
        || world
            .get::<StatusEffects>(caster)
            .is_some_and(|statuses| has_active_status(statuses, StatusEffectKind::Stunned))
}

fn spend_stamina(world: &mut bevy_ecs::world::World, caster: Entity, amount: f32) {
    let now_tick = current_tick(world);
    let Some(mut stamina) = world.get_mut::<Stamina>(caster) else {
        return;
    };
    stamina.current = (stamina.current - amount.max(0.0)).clamp(0.0, stamina.max);
    stamina.state = if stamina.current <= 0.0 {
        StaminaState::Exhausted
    } else {
        StaminaState::Combat
    };
    stamina.last_drain_tick = Some(now_tick);
}

fn is_on_cooldown(world: &bevy_ecs::world::World, caster: Entity, slot: u8, now_tick: u64) -> bool {
    world
        .get::<SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
}

fn set_cooldown(world: &mut bevy_ecs::world::World, caster: Entity, slot: u8, until_tick: u64) {
    if let Some(mut bindings) = world.get_mut::<SkillBarBindings>(caster) {
        bindings.set_cooldown(slot, until_tick);
    }
}

fn insert_casting(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    skill_id: &str,
    profile: SwordTechniqueProfile,
    now_tick: u64,
) {
    let start_position = world
        .get::<Position>(caster)
        .map(|position| position.get())
        .unwrap_or(DVec3::ZERO);
    world.entity_mut(caster).insert(Casting {
        source: CastSource::SkillBar,
        slot,
        started_at_tick: now_tick,
        duration_ticks: u64::from(profile.cast_ticks),
        started_at_ms: current_unix_millis(),
        duration_ms: profile.cast_ticks.saturating_mul(50),
        bound_instance_id: None,
        start_position,
        complete_cooldown_ticks: profile.cooldown_ticks,
        skill_id: Some(skill_id.to_string()),
        skill_config: None,
    });
}

fn current_tick(world: &bevy_ecs::world::World) -> u64 {
    world
        .get_resource::<CombatClock>()
        .map(|clock| clock.tick)
        .unwrap_or_default()
}

fn carrier_for_quality(quality_tier: u8) -> ContainerKind {
    match quality_tier {
        0 => ContainerKind::WieldedInWeapon,
        1 => ContainerKind::SealedInBone,
        _ => ContainerKind::SealedAncientRelic,
    }
}

fn player_account_id_for_entity(entity: Entity, life_record: Option<&LifeRecord>) -> QiAccountId {
    if let Some(life_record) = life_record {
        return QiAccountId::player(life_record.character_id.clone());
    }
    QiAccountId::player(format!("entity:{entity:?}"))
}

fn emit_qi_transfer(
    events: Option<&mut Events<QiTransfer>>,
    from: QiAccountId,
    to: QiAccountId,
    amount: f64,
    reason: QiTransferReason,
) {
    let Some(events) = events else {
        return;
    };
    if let Ok(transfer) = QiTransfer::new(from, to, amount, reason) {
        events.send(transfer);
    }
}

fn emit_self_visuals(
    world: &mut bevy_ecs::world::World,
    entity: Entity,
    anim_id: &str,
    particle_id: &str,
    color: &str,
    priority: u16,
) {
    let origin = world
        .get::<Position>(entity)
        .map(|position| position.get())
        .unwrap_or(DVec3::ZERO);
    let unique_id = world.get::<UniqueId>(entity).map(|id| id.0.to_string());
    if let Some(mut events) = world.get_resource_mut::<Events<VfxEventRequest>>() {
        if let Some(target_player) = unique_id {
            events.send(VfxEventRequest::new(
                origin,
                VfxEventPayloadV1::PlayAnim {
                    target_player,
                    anim_id: anim_id.to_string(),
                    priority,
                    fade_in_ticks: Some(2),
                },
            ));
        }
        emit_particle(
            &mut events,
            origin + DVec3::new(0.0, 1.0, 0.0),
            particle_id,
            color,
            0.8,
            8,
            24,
        );
    }
    if let Some(mut events) = world.get_resource_mut::<Events<PlaySoundRecipeRequest>>() {
        let recipe = match particle_id {
            "bong:sword_parry_spark" => "sword_parry",
            "bong:sword_infuse_glow" => "sword_infuse",
            _ => return,
        };
        emit_audio(&mut events, recipe, entity, origin);
    }
}

fn emit_particle(
    events: &mut Events<VfxEventRequest>,
    origin: DVec3,
    event_id: &str,
    color: &str,
    strength: f32,
    count: u16,
    duration_ticks: u16,
) {
    events.send(VfxEventRequest::new(
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

fn emit_audio(
    events: &mut Events<PlaySoundRecipeRequest>,
    recipe: &str,
    _entity: Entity,
    origin: DVec3,
) {
    events.send(PlaySoundRecipeRequest {
        recipe_id: recipe.to_string(),
        instance_id: 0,
        pos: None,
        flag: None,
        volume_mul: 1.0,
        pitch_shift: 0.0,
        recipient: AudioRecipient::Radius {
            origin,
            radius: AUDIO_BROADCAST_RADIUS,
        },
    });
}

fn color_hex(color: ColorKind) -> &'static str {
    match color {
        ColorKind::Sharp => "#C0C8E8",
        ColorKind::Heavy => "#8A6A44",
        ColorKind::Mellow => "#B0E0C0",
        ColorKind::Solid => "#B8B8B8",
        ColorKind::Light => "#E8F6FF",
        ColorKind::Intricate => "#7B63D8",
        ColorKind::Gentle => "#BDE8D0",
        ColorKind::Insidious => "#7A4AA0",
        ColorKind::Violent => "#D84830",
        ColorKind::Turbid => "#807060",
    }
}

fn lerp(start: f32, end: f32, t: f32) -> f32 {
    start + (end - start) * t.clamp(0.0, 1.0)
}

fn lerp_round(start: f32, end: f32, t: f32) -> u32 {
    lerp(start, end, t).round().max(1.0) as u32
}

fn rejected(reason: CastRejectReason) -> CastResult {
    CastResult::Rejected { reason }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::known_techniques::KnownTechnique;
    use valence::prelude::{App, Update};

    #[test]
    fn proficiency_labels_cover_five_visible_bands() {
        assert_eq!(sword_proficiency_label(0.0), "生疏");
        assert_eq!(sword_proficiency_label(0.2), "入门");
        assert_eq!(sword_proficiency_label(0.5), "熟练");
        assert_eq!(sword_proficiency_label(0.8), "精通");
        assert_eq!(sword_proficiency_label(0.95), "化境");
    }

    #[test]
    fn profiles_scale_core_sword_knobs() {
        let novice = sword_profile(SwordTechnique::Cleave, 0.0);
        let master = sword_profile(SwordTechnique::Cleave, 1.0);
        assert_eq!(novice.stamina_cost, 8.0);
        assert_eq!(master.stamina_cost, 5.0);
        assert_eq!(novice.cast_ticks, 16);
        assert_eq!(master.cast_ticks, 10);
        assert!((master.damage_multiplier - 1.3).abs() < f32::EPSILON);

        let parry = sword_profile(SwordTechnique::Parry, 1.0);
        assert_eq!(parry.parry_window_ticks, 8);
        assert!((parry.block_ratio - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn sword_qi_store_leaks_to_zone_and_expires() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: SWORD_QI_STORE_TICK_INTERVAL,
        });
        app.add_event::<QiTransfer>();
        app.add_systems(Update, sword_qi_store_tick);
        let entity = app
            .world_mut()
            .spawn(SwordQiStore {
                stored_qi: 10.0,
                qi_per_hit: 2.0,
                remaining_ticks: SWORD_QI_STORE_TICK_INTERVAL,
                infuser_color: ColorKind::Mellow,
                weapon_instance_id: 1,
                container_account: QiAccountId::container("test_sword"),
                carrier: ContainerKind::WieldedInWeapon,
            })
            .id();

        app.update();

        assert!(app.world().get::<SwordQiStore>(entity).is_none());
        let transfers = app.world().resource::<Events<QiTransfer>>();
        assert!(!transfers.is_empty());
    }

    #[test]
    fn sword_proficiency_gain_diminishes() {
        assert!(
            sword_proficiency_gain(0.0, true, false) > sword_proficiency_gain(0.9, true, false)
        );
        assert!(sword_proficiency_gain(0.1, true, true) > sword_proficiency_gain(0.1, true, false));
        assert_eq!(sword_proficiency_gain(0.1, false, false), 0.002);
    }

    fn known(id: &str, proficiency: f32) -> KnownTechniques {
        KnownTechniques {
            entries: vec![KnownTechnique {
                id: id.to_string(),
                proficiency,
                active: true,
            }],
        }
    }

    #[test]
    fn hit_events_raise_matching_sword_proficiency() {
        let mut app = App::new();
        app.add_event::<crate::combat::events::CombatEvent>();
        app.add_systems(Update, track_sword_proficiency_from_hits);
        let attacker = app
            .world_mut()
            .spawn(known(SWORD_CLEAVE_SKILL_ID, 0.0))
            .id();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut()
            .send_event(crate::combat::events::CombatEvent {
                attacker,
                target,
                resolved_at_tick: 1,
                body_part: crate::combat::components::BodyPart::Chest,
                wound_kind: WoundKind::Cut,
                source: AttackSource::SwordCleave,
                debug_command: false,
                physical_damage: 3.0,
                damage: 0.0,
                contam_delta: 0.0,
                description: "hit".to_string(),
                defense_kind: None,
                defense_effectiveness: None,
                defense_contam_reduced: None,
                defense_wound_severity: None,
            });

        app.update();

        let known = app.world().get::<KnownTechniques>(attacker).unwrap();
        assert!(known.entries[0].proficiency > 0.0);
    }
}
