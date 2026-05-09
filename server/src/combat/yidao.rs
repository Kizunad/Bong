//! 医道功法 v1：5 招治疗包 + 医者身份底盘。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, DVec3, Entity, Event, Events, Position};

use crate::combat::components::{
    CastSource, Casting, Lifecycle, LifecycleState, SkillBarBindings, Wounds,
};
use crate::combat::CombatClock;
use crate::cultivation::color::PracticeLog;
use crate::cultivation::components::{
    ColorKind, Contamination, Cultivation, Karma, MeridianId, MeridianSystem, QiColor, Realm,
};
use crate::cultivation::meridian::severed::{
    check_meridian_dependencies, try_acupoint_repair, AcupointRepairOutcome,
    MeridianSeveredPermanent,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::network::{redis_bridge::RedisOutbound, RedisBridgeResource};
use crate::qi_physics::{
    contam_purge, emergency_stabilize, life_extend, mass_meridian_repair, meridian_repair,
    yidao_cast_ticks, QiAccountId, QiTransfer, QiTransferReason,
};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::schema::yidao::{
    MedicalContractStateV1, YidaoEventKindV1, YidaoEventV1, YidaoSkillIdV1,
};

const SINGLE_TARGET_RANGE_M: f64 = 5.0;
const CLOSE_TARGET_RANGE_M: f64 = 1.0;
const MASS_TARGET_RANGE_M: f64 = 5.0;
const TICK_MS: u64 = 50;

pub const MERIDIAN_REPAIR_SKILL_ID: &str = "yidao.meridian_repair";
pub const CONTAM_PURGE_SKILL_ID: &str = "yidao.contam_purge";
pub const EMERGENCY_RESUSCITATE_SKILL_ID: &str = "yidao.emergency_resuscitate";
pub const LIFE_EXTENSION_SKILL_ID: &str = "yidao.life_extension";
pub const MASS_MERIDIAN_REPAIR_SKILL_ID: &str = "yidao.mass_meridian_repair";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YidaoSkillId {
    MeridianRepair,
    ContamPurge,
    EmergencyResuscitate,
    LifeExtension,
    MassMeridianRepair,
}

impl YidaoSkillId {
    pub fn skill_id(self) -> &'static str {
        match self {
            Self::MeridianRepair => MERIDIAN_REPAIR_SKILL_ID,
            Self::ContamPurge => CONTAM_PURGE_SKILL_ID,
            Self::EmergencyResuscitate => EMERGENCY_RESUSCITATE_SKILL_ID,
            Self::LifeExtension => LIFE_EXTENSION_SKILL_ID,
            Self::MassMeridianRepair => MASS_MERIDIAN_REPAIR_SKILL_ID,
        }
    }

    pub fn wire(self) -> YidaoSkillIdV1 {
        match self {
            Self::MeridianRepair => YidaoSkillIdV1::MeridianRepair,
            Self::ContamPurge => YidaoSkillIdV1::ContamPurge,
            Self::EmergencyResuscitate => YidaoSkillIdV1::EmergencyResuscitate,
            Self::LifeExtension => YidaoSkillIdV1::LifeExtension,
            Self::MassMeridianRepair => YidaoSkillIdV1::MassMeridianRepair,
        }
    }

    fn event_kind(self) -> YidaoEventKindV1 {
        match self {
            Self::MeridianRepair => YidaoEventKindV1::MeridianHeal,
            Self::ContamPurge => YidaoEventKindV1::ContamPurge,
            Self::EmergencyResuscitate => YidaoEventKindV1::EmergencyResuscitate,
            Self::LifeExtension => YidaoEventKindV1::LifeExtension,
            Self::MassMeridianRepair => YidaoEventKindV1::MassHeal,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct YidaoSkillSpec {
    pub skill: YidaoSkillId,
    pub cast_ticks_base: u64,
    pub cooldown_ticks: u64,
    pub required_realm: Realm,
    pub range_m: f64,
    pub dependencies: &'static [MeridianId],
    pub practice_gain: f64,
    pub audio_recipe: &'static str,
    pub vfx_event_id: &'static str,
}

pub const MERIDIAN_REPAIR_DEPS: &[MeridianId] = &[MeridianId::Heart, MeridianId::Lung];
pub const CONTAM_PURGE_DEPS: &[MeridianId] = &[MeridianId::Lung, MeridianId::LargeIntestine];
pub const EMERGENCY_DEPS: &[MeridianId] = &[MeridianId::LargeIntestine];
pub const LIFE_EXTENSION_DEPS: &[MeridianId] = &[
    MeridianId::Heart,
    MeridianId::Lung,
    MeridianId::LargeIntestine,
    MeridianId::Kidney,
];
pub const MASS_REPAIR_DEPS: &[MeridianId] = &[
    MeridianId::Du,
    MeridianId::Heart,
    MeridianId::Lung,
    MeridianId::LargeIntestine,
    MeridianId::Kidney,
];

pub fn yidao_skill_spec(skill: YidaoSkillId) -> YidaoSkillSpec {
    match skill {
        YidaoSkillId::MeridianRepair => YidaoSkillSpec {
            skill,
            cast_ticks_base: 60 * 20,
            cooldown_ticks: 20 * 20,
            required_realm: Realm::Awaken,
            range_m: SINGLE_TARGET_RANGE_M,
            dependencies: MERIDIAN_REPAIR_DEPS,
            practice_gain: 50.0,
            audio_recipe: "yidao_meridian_repair",
            vfx_event_id: "bong:yidao_meridian_repair",
        },
        YidaoSkillId::ContamPurge => YidaoSkillSpec {
            skill,
            cast_ticks_base: 30 * 20,
            cooldown_ticks: 10 * 20,
            required_realm: Realm::Awaken,
            range_m: SINGLE_TARGET_RANGE_M,
            dependencies: CONTAM_PURGE_DEPS,
            practice_gain: 30.0,
            audio_recipe: "yidao_contam_purge",
            vfx_event_id: "bong:yidao_contam_purge",
        },
        YidaoSkillId::EmergencyResuscitate => YidaoSkillSpec {
            skill,
            cast_ticks_base: 5 * 20,
            cooldown_ticks: 10 * 20,
            required_realm: Realm::Awaken,
            range_m: CLOSE_TARGET_RANGE_M,
            dependencies: EMERGENCY_DEPS,
            practice_gain: 10.0,
            audio_recipe: "yidao_emergency_resuscitate",
            vfx_event_id: "bong:yidao_emergency_resuscitate",
        },
        YidaoSkillId::LifeExtension => YidaoSkillSpec {
            skill,
            cast_ticks_base: 30 * 20,
            cooldown_ticks: 3600 * 20,
            required_realm: Realm::Spirit,
            range_m: CLOSE_TARGET_RANGE_M,
            dependencies: LIFE_EXTENSION_DEPS,
            practice_gain: 200.0,
            audio_recipe: "yidao_life_extension",
            vfx_event_id: "bong:yidao_life_extension",
        },
        YidaoSkillId::MassMeridianRepair => YidaoSkillSpec {
            skill,
            cast_ticks_base: 60 * 20,
            cooldown_ticks: 3600 * 20,
            required_realm: Realm::Void,
            range_m: MASS_TARGET_RANGE_M,
            dependencies: MASS_REPAIR_DEPS,
            practice_gain: 100.0,
            audio_recipe: "yidao_mass_meridian_repair",
            vfx_event_id: "bong:yidao_mass_meridian_repair",
        },
    }
}

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct HealingMastery {
    pub meridian_repair: f64,
    pub contam_purge: f64,
    pub emergency_resuscitate: f64,
    pub life_extension: f64,
    pub mass_meridian_repair: f64,
}

impl Default for HealingMastery {
    fn default() -> Self {
        Self {
            meridian_repair: 0.0,
            contam_purge: 0.0,
            emergency_resuscitate: 0.0,
            life_extension: 0.0,
            mass_meridian_repair: 0.0,
        }
    }
}

impl HealingMastery {
    pub fn get(&self, skill: YidaoSkillId) -> f64 {
        match skill {
            YidaoSkillId::MeridianRepair => self.meridian_repair,
            YidaoSkillId::ContamPurge => self.contam_purge,
            YidaoSkillId::EmergencyResuscitate => self.emergency_resuscitate,
            YidaoSkillId::LifeExtension => self.life_extension,
            YidaoSkillId::MassMeridianRepair => self.mass_meridian_repair,
        }
    }

    pub fn add_cast_growth(&mut self, skill: YidaoSkillId) -> f64 {
        let current = self.get(skill);
        let delta = if current < 50.0 {
            0.5
        } else if current < 80.0 {
            0.2
        } else {
            0.05
        };
        let next = (current + delta).min(100.0);
        match skill {
            YidaoSkillId::MeridianRepair => self.meridian_repair = next,
            YidaoSkillId::ContamPurge => self.contam_purge = next,
            YidaoSkillId::EmergencyResuscitate => self.emergency_resuscitate = next,
            YidaoSkillId::LifeExtension => self.life_extension = next,
            YidaoSkillId::MassMeridianRepair => self.mass_meridian_repair = next,
        }
        next - current
    }
}

#[derive(Debug, Clone, Component, Default, Serialize, Deserialize, PartialEq)]
pub struct HealerProfile {
    pub reputation: i32,
    pub contracts: Vec<MedicalContract>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MedicalContract {
    pub patient_id: String,
    pub state: MedicalContractState,
    pub treatment_count: u32,
    pub first_treatment_tick: u64,
    pub last_treatment_tick: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MedicalContractState {
    Stranger,
    Patient,
    LongTermPatient,
    Bonded,
}

impl MedicalContractState {
    pub fn wire(self) -> MedicalContractStateV1 {
        match self {
            Self::Stranger => MedicalContractStateV1::Stranger,
            Self::Patient => MedicalContractStateV1::Patient,
            Self::LongTermPatient => MedicalContractStateV1::LongTermPatient,
            Self::Bonded => MedicalContractStateV1::Bonded,
        }
    }
}

#[derive(Debug, Clone, Default, Component, Serialize, Deserialize, PartialEq)]
pub struct KarmaCounter {
    pub yidao_karma: f64,
    pub tribulation_weight: f64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct YidaoEvent {
    pub payload: YidaoEventV1,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HealerNpcDecision {
    pub action: HealerNpcAction,
    pub score: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealerNpcAction {
    EmergencyResuscitate,
    LifeExtension,
    ContamPurge,
    MeridianRepair,
    Retreat,
    Idle,
}

pub fn register(app: &mut valence::prelude::App) {
    app.add_event::<YidaoEvent>();
}

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(MERIDIAN_REPAIR_SKILL_ID, resolve_meridian_repair_skill);
    registry.register(CONTAM_PURGE_SKILL_ID, resolve_contam_purge_skill);
    registry.register(
        EMERGENCY_RESUSCITATE_SKILL_ID,
        resolve_emergency_resuscitate_skill,
    );
    registry.register(LIFE_EXTENSION_SKILL_ID, resolve_life_extension_skill);
    registry.register(
        MASS_MERIDIAN_REPAIR_SKILL_ID,
        resolve_mass_meridian_repair_skill,
    );
}

pub fn resolve_meridian_repair_skill(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_yidao_skill(world, caster, slot, target, YidaoSkillId::MeridianRepair)
}

pub fn resolve_contam_purge_skill(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_yidao_skill(world, caster, slot, target, YidaoSkillId::ContamPurge)
}

pub fn resolve_emergency_resuscitate_skill(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_yidao_skill(
        world,
        caster,
        slot,
        target,
        YidaoSkillId::EmergencyResuscitate,
    )
}

pub fn resolve_life_extension_skill(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_yidao_skill(world, caster, slot, target, YidaoSkillId::LifeExtension)
}

pub fn resolve_mass_meridian_repair_skill(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_yidao_skill(
        world,
        caster,
        slot,
        target,
        YidaoSkillId::MassMeridianRepair,
    )
}

pub fn resolve_yidao_skill(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
    skill: YidaoSkillId,
) -> CastResult {
    let spec = yidao_skill_spec(skill);
    let now_tick = world
        .get_resource::<CombatClock>()
        .map(|clock| clock.tick)
        .unwrap_or_default();
    if world
        .get::<SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
    {
        return rejected(CastRejectReason::OnCooldown);
    }

    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    if !realm_at_least(cultivation.realm, spec.required_realm) {
        return rejected(CastRejectReason::RealmTooLow);
    }
    let severed = world.get::<MeridianSeveredPermanent>(caster);
    if let Err(dep) = check_meridian_dependencies(spec.dependencies, severed) {
        return rejected(CastRejectReason::MeridianSevered(Some(dep)));
    }

    let mastery = world
        .get::<HealingMastery>(caster)
        .map(|mastery| mastery.get(skill))
        .unwrap_or_default();
    let peace_color = has_peace_color(world, caster);
    let cast_ticks = match yidao_cast_ticks(spec.cast_ticks_base, mastery, peace_color) {
        Ok(ticks) => ticks,
        Err(_) => return rejected(CastRejectReason::InvalidTarget),
    };

    let patients = if skill == YidaoSkillId::MassMeridianRepair {
        let density = local_qi_density_for_mass_repair(world, caster);
        let capacity = match mass_meridian_repair(density, cultivation.realm, mastery, peace_color)
        {
            Ok(outcome) => outcome.capacity,
            Err(_) => return rejected(CastRejectReason::InvalidTarget),
        };
        let patients = collect_mass_repair_patients(world, caster, target, capacity, spec.range_m);
        if patients.is_empty() {
            return rejected(CastRejectReason::InvalidTarget);
        }
        patients
    } else {
        let Some(patient) = target else {
            return rejected(CastRejectReason::InvalidTarget);
        };
        if skill == YidaoSkillId::LifeExtension && patient == caster {
            return rejected(CastRejectReason::InvalidTarget);
        }
        if !is_patient_in_range(world, caster, patient, spec.range_m) {
            return rejected(CastRejectReason::InvalidTarget);
        }
        vec![patient]
    };

    insert_casting(world, caster, slot, spec, cast_ticks, now_tick);
    let outcome = apply_yidao_effect(
        world,
        caster,
        &patients,
        skill,
        mastery,
        peace_color,
        now_tick,
    );
    if outcome.success_count == 0 && outcome.failure_count == 0 {
        world.entity_mut(caster).remove::<Casting>();
        return rejected(CastRejectReason::InvalidTarget);
    }
    if let Some(mut bindings) = world.get_mut::<SkillBarBindings>(caster) {
        bindings.set_cooldown(slot, now_tick.saturating_add(spec.cooldown_ticks));
    }
    emit_yidao_vfx_audio(
        world,
        caster,
        patients.first().copied().unwrap_or(caster),
        spec,
    );
    CastResult::Started {
        cooldown_ticks: spec.cooldown_ticks,
        anim_duration_ticks: cast_ticks as u32,
    }
}

#[derive(Debug, Clone, PartialEq)]
struct YidaoApplyOutcome {
    patient_ids: Vec<String>,
    meridian_id: Option<MeridianId>,
    success_count: u32,
    failure_count: u32,
    qi_transferred: f64,
    contam_reduced: f64,
    hp_restored: f32,
    karma_delta: f64,
    medic_qi_max_delta: f64,
    patient_qi_max_delta: f64,
    contract_state: Option<MedicalContractState>,
    detail: String,
}

impl Default for YidaoApplyOutcome {
    fn default() -> Self {
        Self {
            patient_ids: Vec::new(),
            meridian_id: None,
            success_count: 0,
            failure_count: 0,
            qi_transferred: 0.0,
            contam_reduced: 0.0,
            hp_restored: 0.0,
            karma_delta: 0.0,
            medic_qi_max_delta: 0.0,
            patient_qi_max_delta: 0.0,
            contract_state: None,
            detail: String::new(),
        }
    }
}

fn apply_yidao_effect(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    patients: &[Entity],
    skill: YidaoSkillId,
    mastery: f64,
    peace_color: bool,
    now_tick: u64,
) -> YidaoApplyOutcome {
    let mut outcome = match skill {
        YidaoSkillId::MeridianRepair => {
            apply_meridian_repair(world, caster, patients[0], mastery, peace_color, now_tick)
        }
        YidaoSkillId::ContamPurge => {
            apply_contam_purge(world, caster, patients[0], mastery, peace_color)
        }
        YidaoSkillId::EmergencyResuscitate => {
            apply_emergency_resuscitate(world, caster, patients[0], mastery, now_tick)
        }
        YidaoSkillId::LifeExtension => {
            apply_life_extension(world, caster, patients[0], mastery, peace_color, now_tick)
        }
        YidaoSkillId::MassMeridianRepair => {
            apply_mass_meridian_repair(world, caster, patients, mastery, peace_color, now_tick)
        }
    };
    if outcome.success_count > 0 {
        outcome.contract_state = grow_healer_identity(
            world,
            caster,
            patients,
            skill,
            outcome.success_count,
            now_tick,
        );
    }
    if outcome.success_count > 0 || outcome.failure_count > 0 {
        emit_yidao_event(world, caster, skill, now_tick, &outcome);
    }
    outcome
}

fn apply_meridian_repair(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    patient: Entity,
    mastery: f64,
    peace_color: bool,
    now_tick: u64,
) -> YidaoApplyOutcome {
    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return YidaoApplyOutcome::default();
    };
    let Ok(calc) = meridian_repair(cultivation.qi_max, cultivation.realm, mastery, peace_color)
    else {
        return YidaoApplyOutcome::default();
    };
    let Some(meridian_id) = first_repairable_meridian(world, patient) else {
        return YidaoApplyOutcome::default();
    };
    if !debit_caster_qi(world, caster, calc.qi_cost) {
        return YidaoApplyOutcome::default();
    }
    let roll = deterministic_success_roll(caster, patient, meridian_id, now_tick);
    let repair_outcome = {
        let Some(mut severed) = world.get_mut::<MeridianSeveredPermanent>(patient) else {
            return YidaoApplyOutcome::default();
        };
        try_acupoint_repair(&mut severed, meridian_id, roll, calc.success_threshold)
    };
    let mut out = YidaoApplyOutcome {
        patient_ids: vec![entity_wire_id(patient)],
        meridian_id: Some(meridian_id),
        qi_transferred: calc.qi_cost,
        detail: "meridian repair".to_string(),
        ..Default::default()
    };
    match repair_outcome {
        AcupointRepairOutcome::Restored => {
            if let Some(mut meridians) = world.get_mut::<MeridianSystem>(patient) {
                let meridian = meridians.get_mut(meridian_id);
                meridian.integrity = meridian.integrity.max(0.35);
                meridian.opened = true;
            }
            credit_patient_qi(world, patient, calc.qi_cost);
            emit_qi_transfer(world, caster, patient, calc.qi_cost);
            out.success_count = 1;
        }
        AcupointRepairOutcome::Failed => {
            add_karma(world, caster, calc.medic_karma_on_failure);
            add_karma(world, patient, calc.patient_karma_on_failure);
            out.failure_count = 1;
            out.qi_transferred = 0.0;
            out.karma_delta = calc.medic_karma_on_failure + calc.patient_karma_on_failure;
            out.detail = "meridian repair failed; dead meridian recorded".to_string();
        }
        AcupointRepairOutcome::NotSevered | AcupointRepairOutcome::AlreadyDead => {}
    }
    out
}

fn apply_contam_purge(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    patient: Entity,
    mastery: f64,
    peace_color: bool,
) -> YidaoApplyOutcome {
    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return YidaoApplyOutcome::default();
    };
    let contamination_total = world
        .get::<Contamination>(patient)
        .map(contamination_total)
        .unwrap_or_default();
    let Ok(calc) = contam_purge(
        cultivation.qi_max,
        contamination_total,
        cultivation.realm,
        mastery,
        peace_color,
    ) else {
        return YidaoApplyOutcome::default();
    };
    if contamination_total <= f64::EPSILON || !debit_caster_qi(world, caster, calc.qi_cost) {
        return YidaoApplyOutcome::default();
    }
    if let Some(mut contamination) = world.get_mut::<Contamination>(patient) {
        scale_contamination(&mut contamination, calc.residual_total);
    }
    emit_qi_transfer(world, caster, patient, calc.qi_cost);
    YidaoApplyOutcome {
        patient_ids: vec![entity_wire_id(patient)],
        success_count: 1,
        qi_transferred: calc.qi_cost,
        contam_reduced: calc.purge_amount,
        detail: "contamination purged".to_string(),
        ..Default::default()
    }
}

fn apply_emergency_resuscitate(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    patient: Entity,
    mastery: f64,
    now_tick: u64,
) -> YidaoApplyOutcome {
    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return YidaoApplyOutcome::default();
    };
    let hp_max = world
        .get::<Wounds>(patient)
        .map(|wounds| wounds.health_max)
        .unwrap_or(100.0);
    let Ok(calc) = emergency_stabilize(cultivation.qi_max, hp_max, mastery) else {
        return YidaoApplyOutcome::default();
    };
    let valid_lifecycle = world.get::<Lifecycle>(patient).is_some_and(|lifecycle| {
        lifecycle.state == LifecycleState::NearDeath
            && lifecycle.last_death_tick.is_some_and(|death_tick| {
                now_tick <= death_tick.saturating_add(calc.dying_window_ticks)
            })
    });
    if !valid_lifecycle || !debit_caster_qi(world, caster, calc.qi_cost) {
        return YidaoApplyOutcome::default();
    }
    let mut restored = 0.0_f32;
    if let Some(mut wounds) = world.get_mut::<Wounds>(patient) {
        for wound in &mut wounds.entries {
            wound.bleeding_per_sec = 0.0;
        }
        let before = wounds.health_current;
        wounds.health_current = (wounds.health_current + calc.hp_restore).min(wounds.health_max);
        restored = wounds.health_current - before;
    }
    if let Some(mut lifecycle) = world.get_mut::<Lifecycle>(patient) {
        lifecycle.revive(now_tick);
    }
    emit_qi_transfer(world, caster, patient, calc.qi_cost);
    YidaoApplyOutcome {
        patient_ids: vec![entity_wire_id(patient)],
        success_count: 1,
        qi_transferred: calc.qi_cost,
        hp_restored: restored,
        detail: "emergency stabilized".to_string(),
        ..Default::default()
    }
}

fn apply_life_extension(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    patient: Entity,
    mastery: f64,
    peace_color: bool,
    now_tick: u64,
) -> YidaoApplyOutcome {
    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return YidaoApplyOutcome::default();
    };
    let Ok(calc) = life_extend(cultivation.qi_max, mastery, peace_color) else {
        return YidaoApplyOutcome::default();
    };
    let valid_lifecycle = world.get::<Lifecycle>(patient).is_some_and(|lifecycle| {
        lifecycle.state == LifecycleState::NearDeath
            && lifecycle
                .last_death_tick
                .is_some_and(|death_tick| now_tick <= death_tick.saturating_add(calc.window_ticks))
    });
    if !valid_lifecycle || !debit_caster_qi(world, caster, calc.qi_cost.min(cultivation.qi_max)) {
        return YidaoApplyOutcome::default();
    }
    apply_qi_max_loss(world, caster, calc.medic_qi_max_loss_ratio);
    apply_qi_max_loss(world, patient, calc.patient_qi_max_loss_ratio);
    add_karma(world, caster, calc.medic_karma_delta);
    if let Some(mut lifecycle) = world.get_mut::<Lifecycle>(patient) {
        lifecycle.revive(now_tick);
    }
    if let Some(mut wounds) = world.get_mut::<Wounds>(patient) {
        wounds.health_current =
            (wounds.health_max * calc.revive_hp_fraction).max(wounds.health_current);
    }
    emit_qi_transfer(world, caster, patient, calc.qi_cost.min(cultivation.qi_max));
    YidaoApplyOutcome {
        patient_ids: vec![entity_wire_id(patient)],
        success_count: 1,
        qi_transferred: calc.qi_cost.min(cultivation.qi_max),
        karma_delta: calc.medic_karma_delta,
        medic_qi_max_delta: -calc.medic_qi_max_loss_ratio,
        patient_qi_max_delta: -calc.patient_qi_max_loss_ratio,
        detail: "life extension revived patient".to_string(),
        ..Default::default()
    }
}

fn apply_mass_meridian_repair(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    patients: &[Entity],
    mastery: f64,
    peace_color: bool,
    now_tick: u64,
) -> YidaoApplyOutcome {
    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return YidaoApplyOutcome::default();
    };
    let density = local_qi_density_for_mass_repair(world, caster);
    let Ok(calc) = mass_meridian_repair(density, cultivation.realm, mastery, peace_color) else {
        return YidaoApplyOutcome::default();
    };
    if calc.capacity == 0 || !debit_caster_qi(world, caster, cultivation.qi_max) {
        return YidaoApplyOutcome::default();
    }

    let mut out = YidaoApplyOutcome {
        detail: "mass meridian repair".to_string(),
        ..Default::default()
    };
    let max_patients = usize::try_from(calc.capacity).unwrap_or(usize::MAX);
    for patient in patients.iter().copied().take(max_patients) {
        let Some(meridian_id) = first_repairable_meridian(world, patient) else {
            continue;
        };
        let repair_outcome = {
            let Some(mut severed) = world.get_mut::<MeridianSeveredPermanent>(patient) else {
                continue;
            };
            let roll = deterministic_success_roll(caster, patient, meridian_id, now_tick);
            try_acupoint_repair(&mut severed, meridian_id, roll, calc.success_threshold)
        };
        out.patient_ids.push(entity_wire_id(patient));
        out.meridian_id.get_or_insert(meridian_id);
        match repair_outcome {
            AcupointRepairOutcome::Restored => {
                if let Some(mut meridians) = world.get_mut::<MeridianSystem>(patient) {
                    let meridian = meridians.get_mut(meridian_id);
                    meridian.integrity = meridian.integrity.max(0.35);
                    meridian.opened = true;
                }
                out.success_count += 1;
                emit_qi_transfer(
                    world,
                    caster,
                    patient,
                    cultivation.qi_max / patients.len().max(1) as f64,
                );
            }
            AcupointRepairOutcome::Failed => {
                out.failure_count += 1;
            }
            AcupointRepairOutcome::NotSevered | AcupointRepairOutcome::AlreadyDead => {}
        }
    }
    if out.success_count > 0 {
        let n = f64::from(out.success_count);
        let qi_loss = calc.medic_qi_max_loss_ratio_per_patient * n;
        let karma = calc.medic_karma_delta_per_patient * n;
        apply_qi_max_loss(world, caster, qi_loss);
        add_karma(world, caster, karma);
        out.qi_transferred = cultivation.qi_max;
        out.karma_delta = karma;
        out.medic_qi_max_delta = -qi_loss;
    }
    out
}

fn grow_healer_identity(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    patients: &[Entity],
    skill: YidaoSkillId,
    success_count: u32,
    now_tick: u64,
) -> Option<MedicalContractState> {
    if world.get::<HealerProfile>(caster).is_none() {
        world.entity_mut(caster).insert(HealerProfile::default());
    }
    if world.get::<HealingMastery>(caster).is_none() {
        world.entity_mut(caster).insert(HealingMastery::default());
    }
    if world.get::<KarmaCounter>(caster).is_none() {
        world.entity_mut(caster).insert(KarmaCounter::default());
    }
    if let Some(mut mastery) = world.get_mut::<HealingMastery>(caster) {
        mastery.add_cast_growth(skill);
    }
    let mut contract_state = None;
    if let Some(mut profile) = world.get_mut::<HealerProfile>(caster) {
        let reputation_gain = i32::try_from(success_count).unwrap_or(i32::MAX);
        profile.reputation = profile.reputation.saturating_add(reputation_gain);
        for patient in patients.iter().copied().take(success_count as usize) {
            contract_state = Some(record_treatment_contract(
                &mut profile,
                entity_wire_id(patient),
                now_tick,
            ));
        }
    }
    if let Some(mut log) = world.get_mut::<PracticeLog>(caster) {
        log.add(ColorKind::Gentle, yidao_skill_spec(skill).practice_gain);
    }
    let karma_weight = world.get::<Karma>(caster).map(|karma| karma.weight);
    if let (Some(mut karma_counter), Some(karma_weight)) =
        (world.get_mut::<KarmaCounter>(caster), karma_weight)
    {
        karma_counter.yidao_karma = karma_weight;
        karma_counter.tribulation_weight = if karma_weight >= 10.0 {
            karma_weight
        } else {
            0.0
        };
    }
    contract_state
}

fn record_treatment_contract(
    profile: &mut HealerProfile,
    patient_id: String,
    now_tick: u64,
) -> MedicalContractState {
    if let Some(contract) = profile
        .contracts
        .iter_mut()
        .find(|contract| contract.patient_id == patient_id)
    {
        contract.treatment_count = contract.treatment_count.saturating_add(1);
        contract.last_treatment_tick = now_tick;
        if contract.state == MedicalContractState::Patient && contract.treatment_count >= 5 {
            contract.state = MedicalContractState::LongTermPatient;
        }
        return contract.state;
    }
    profile.contracts.push(MedicalContract {
        patient_id,
        state: MedicalContractState::Patient,
        treatment_count: 1,
        first_treatment_tick: now_tick,
        last_treatment_tick: now_tick,
    });
    MedicalContractState::Patient
}

fn emit_yidao_event(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    skill: YidaoSkillId,
    now_tick: u64,
    outcome: &YidaoApplyOutcome,
) {
    let payload = YidaoEventV1 {
        v: 1,
        kind: skill.event_kind(),
        tick: now_tick,
        medic_id: entity_wire_id(caster),
        patient_ids: outcome.patient_ids.clone(),
        skill: skill.wire(),
        meridian_id: outcome.meridian_id,
        success_count: outcome.success_count,
        failure_count: outcome.failure_count,
        qi_transferred: outcome.qi_transferred,
        contam_reduced: outcome.contam_reduced,
        hp_restored: outcome.hp_restored,
        karma_delta: outcome.karma_delta,
        medic_qi_max_delta: outcome.medic_qi_max_delta,
        patient_qi_max_delta: outcome.patient_qi_max_delta,
        contract_state: outcome.contract_state.map(MedicalContractState::wire),
        detail: outcome.detail.clone(),
    };
    if let Some(mut events) = world.get_resource_mut::<Events<YidaoEvent>>() {
        events.send(YidaoEvent {
            payload: payload.clone(),
        });
    }
    if let Some(redis) = world.get_resource::<RedisBridgeResource>() {
        let _ = redis.tx_outbound.send(RedisOutbound::YidaoEvent(payload));
    }
}

fn insert_casting(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    spec: YidaoSkillSpec,
    cast_ticks: u64,
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
        duration_ticks: cast_ticks,
        started_at_ms: current_unix_millis(),
        duration_ms: (cast_ticks.saturating_mul(TICK_MS)).min(u64::from(u32::MAX)) as u32,
        bound_instance_id: None,
        start_position,
        complete_cooldown_ticks: spec.cooldown_ticks,
        skill_id: Some(spec.skill.skill_id().to_string()),
        skill_config: None,
    });
}

fn emit_yidao_vfx_audio(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    patient: Entity,
    spec: YidaoSkillSpec,
) {
    let origin = world
        .get::<Position>(patient)
        .or_else(|| world.get::<Position>(caster))
        .map(|position| position.get())
        .unwrap_or(DVec3::ZERO);
    if let Some(mut vfx_events) = world.get_resource_mut::<Events<VfxEventRequest>>() {
        vfx_events.send(VfxEventRequest::new(
            origin,
            VfxEventPayloadV1::SpawnParticle {
                event_id: spec.vfx_event_id.to_string(),
                origin: [origin.x, origin.y + 1.0, origin.z],
                direction: Some([0.0, 1.0, 0.0]),
                color: Some("#A8E6CF".to_string()),
                strength: Some(0.85),
                count: Some(if spec.skill == YidaoSkillId::MassMeridianRepair {
                    24
                } else {
                    8
                }),
                duration_ticks: Some(60),
            },
        ));
    }
    if let Some(mut audio_events) = world.get_resource_mut::<Events<PlaySoundRecipeRequest>>() {
        audio_events.send(PlaySoundRecipeRequest {
            recipe_id: spec.audio_recipe.to_string(),
            instance_id: 0,
            pos: Some([origin.x as i32, origin.y as i32, origin.z as i32]),
            flag: Some(spec.skill.skill_id().to_string()),
            volume_mul: 1.0,
            pitch_shift: 0.0,
            recipient: AudioRecipient::Radius {
                origin,
                radius: if spec.skill == YidaoSkillId::MassMeridianRepair {
                    16.0
                } else {
                    8.0
                },
            },
        });
    }
}

fn debit_caster_qi(world: &mut bevy_ecs::world::World, caster: Entity, amount: f64) -> bool {
    let Some(mut cultivation) = world.get_mut::<Cultivation>(caster) else {
        return false;
    };
    if amount <= f64::EPSILON || cultivation.qi_current + f64::EPSILON < amount {
        return false;
    }
    cultivation.qi_current = (cultivation.qi_current - amount).max(0.0);
    true
}

fn credit_patient_qi(world: &mut bevy_ecs::world::World, patient: Entity, amount: f64) {
    if let Some(mut cultivation) = world.get_mut::<Cultivation>(patient) {
        cultivation.qi_current = (cultivation.qi_current + amount).min(cultivation.qi_max);
    }
}

fn apply_qi_max_loss(world: &mut bevy_ecs::world::World, entity: Entity, ratio: f64) {
    if let Some(mut cultivation) = world.get_mut::<Cultivation>(entity) {
        let loss = cultivation.qi_max * ratio.clamp(0.0, 1.0);
        cultivation.qi_max = (cultivation.qi_max - loss).max(1.0);
        cultivation.qi_current = cultivation.qi_current.min(cultivation.qi_max);
    }
}

fn add_karma(world: &mut bevy_ecs::world::World, entity: Entity, delta: f64) {
    if world.get::<Karma>(entity).is_none() {
        world.entity_mut(entity).insert(Karma::default());
    }
    if let Some(mut karma) = world.get_mut::<Karma>(entity) {
        karma.weight += delta;
    }
}

fn emit_qi_transfer(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    patient: Entity,
    amount: f64,
) {
    let Some(mut events) = world.get_resource_mut::<Events<QiTransfer>>() else {
        return;
    };
    if let Ok(transfer) = QiTransfer::new(
        QiAccountId::player(entity_wire_id(caster)),
        QiAccountId::player(entity_wire_id(patient)),
        amount,
        QiTransferReason::Healing,
    ) {
        events.send(transfer);
    }
}

fn first_repairable_meridian(
    world: &bevy_ecs::world::World,
    patient: Entity,
) -> Option<MeridianId> {
    let severed = world.get::<MeridianSeveredPermanent>(patient)?;
    MeridianId::ALL
        .iter()
        .copied()
        .find(|id| severed.is_severed(*id) && !severed.is_dead(*id))
}

fn contamination_total(contamination: &Contamination) -> f64 {
    contamination
        .entries
        .iter()
        .map(|entry| entry.amount.max(0.0))
        .sum()
}

fn scale_contamination(contamination: &mut Contamination, residual_total: f64) {
    let before = contamination_total(contamination);
    if before <= f64::EPSILON {
        contamination.entries.clear();
        return;
    }
    let ratio = (residual_total / before).clamp(0.0, 1.0);
    for entry in &mut contamination.entries {
        entry.amount *= ratio;
    }
    contamination
        .entries
        .retain(|entry| entry.amount > f64::EPSILON);
}

fn collect_mass_repair_patients(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    explicit_target: Option<Entity>,
    capacity: u32,
    range_m: f64,
) -> Vec<Entity> {
    if capacity == 0 {
        return Vec::new();
    }
    let mut patients = Vec::new();
    if let Some(target) = explicit_target {
        if target != caster
            && first_repairable_meridian(world, target).is_some()
            && is_patient_in_range(world, caster, target, range_m)
        {
            patients.push(target);
        }
    }
    let mut query = world.query::<(Entity, &MeridianSeveredPermanent)>();
    for (entity, severed) in query.iter(world) {
        if entity == caster || patients.contains(&entity) {
            continue;
        }
        if !MeridianId::ALL
            .iter()
            .any(|id| severed.is_severed(*id) && !severed.is_dead(*id))
        {
            continue;
        }
        if is_patient_in_range(world, caster, entity, range_m) {
            patients.push(entity);
        }
        if patients.len() >= capacity as usize {
            break;
        }
    }
    patients.sort_by_key(|entity| entity.to_bits());
    patients.truncate(capacity as usize);
    patients
}

fn local_qi_density_for_mass_repair(world: &bevy_ecs::world::World, caster: Entity) -> f64 {
    world
        .get::<Cultivation>(caster)
        .map(|cultivation| (cultivation.qi_current / cultivation.qi_max.max(1.0)) * 9.0)
        .unwrap_or(0.0)
}

fn is_patient_in_range(
    world: &bevy_ecs::world::World,
    caster: Entity,
    patient: Entity,
    range_m: f64,
) -> bool {
    let (Some(caster_pos), Some(patient_pos)) = (
        world.get::<Position>(caster),
        world.get::<Position>(patient),
    ) else {
        return false;
    };
    caster_pos.get().distance_squared(patient_pos.get()) <= range_m * range_m
}

fn has_peace_color(world: &bevy_ecs::world::World, entity: Entity) -> bool {
    world.get::<QiColor>(entity).is_some_and(|color| {
        color.main == ColorKind::Gentle
            && !color.is_chaotic
            && !color.is_hunyuan
            && color.secondary.is_none()
    })
}

fn realm_at_least(actual: Realm, required: Realm) -> bool {
    realm_rank(actual) >= realm_rank(required)
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

fn deterministic_success_roll(
    caster: Entity,
    patient: Entity,
    meridian_id: MeridianId,
    tick: u64,
) -> f64 {
    let mut value = caster.to_bits() ^ patient.to_bits().rotate_left(17) ^ tick;
    value ^= (meridian_id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    (value % 10_000) as f64 / 10_000.0
}

pub fn healer_npc_decision(
    hp_percent: f32,
    severed_count: u32,
    contam_total: f64,
    near_death: bool,
    has_enemy_nearby: bool,
    can_life_extend: bool,
) -> HealerNpcDecision {
    if has_enemy_nearby {
        return HealerNpcDecision {
            action: HealerNpcAction::Retreat,
            score: 1.0,
        };
    }
    if near_death && can_life_extend {
        return HealerNpcDecision {
            action: HealerNpcAction::LifeExtension,
            score: 0.98,
        };
    }
    if hp_percent < 0.5 {
        return HealerNpcDecision {
            action: HealerNpcAction::EmergencyResuscitate,
            score: 0.8,
        };
    }
    if severed_count > 0 {
        return HealerNpcDecision {
            action: HealerNpcAction::MeridianRepair,
            score: 0.7,
        };
    }
    if contam_total >= 50.0 {
        return HealerNpcDecision {
            action: HealerNpcAction::ContamPurge,
            score: 0.65,
        };
    }
    HealerNpcDecision {
        action: HealerNpcAction::Idle,
        score: 0.1,
    }
}

pub(crate) fn entity_wire_id(entity: Entity) -> String {
    format!("entity_bits:{}", entity.to_bits())
}

fn current_unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn rejected(reason: CastRejectReason) -> CastResult {
    CastResult::Rejected { reason }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{ContamSource, MeridianSystem};
    use valence::prelude::App;

    fn app_with_yidao() -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 100 });
        app.add_event::<YidaoEvent>();
        app.add_event::<QiTransfer>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<PlaySoundRecipeRequest>();
        app
    }

    fn spawn_medic(app: &mut App, realm: Realm) -> Entity {
        app.world_mut()
            .spawn((
                Cultivation {
                    realm,
                    qi_current: 300.0,
                    qi_max: 300.0,
                    ..Default::default()
                },
                QiColor {
                    main: ColorKind::Gentle,
                    secondary: None,
                    is_chaotic: false,
                    is_hunyuan: false,
                },
                PracticeLog::default(),
                SkillBarBindings::default(),
                HealingMastery::default(),
                Position::new([0.0, 64.0, 0.0]),
            ))
            .id()
    }

    fn spawn_patient(app: &mut App) -> Entity {
        app.world_mut()
            .spawn((
                Cultivation {
                    qi_current: 0.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                Wounds {
                    health_current: 10.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle::default(),
                MeridianSystem::default(),
                Position::new([0.0, 64.0, 0.0]),
            ))
            .id()
    }

    #[test]
    fn register_skills_adds_all_five_resolvers() {
        let mut registry = SkillRegistry::default();
        register_skills(&mut registry);
        for skill in [
            YidaoSkillId::MeridianRepair,
            YidaoSkillId::ContamPurge,
            YidaoSkillId::EmergencyResuscitate,
            YidaoSkillId::LifeExtension,
            YidaoSkillId::MassMeridianRepair,
        ] {
            assert!(
                registry.lookup(skill.skill_id()).is_some(),
                "{skill:?} missing"
            );
        }
    }

    #[test]
    fn meridian_repair_restores_first_severed_meridian_and_records_identity() {
        let mut app = app_with_yidao();
        let medic = spawn_medic(&mut app, Realm::Void);
        let patient = spawn_patient(&mut app);
        let mut severed = MeridianSeveredPermanent::default();
        severed.insert(
            MeridianId::Lung,
            crate::cultivation::meridian::severed::SeveredSource::CombatWound,
            1,
        );
        app.world_mut().entity_mut(patient).insert(severed);
        app.world_mut().entity_mut(medic).insert(HealingMastery {
            meridian_repair: 100.0,
            ..Default::default()
        });

        let result = resolve_meridian_repair_skill(app.world_mut(), medic, 0, Some(patient));

        assert!(matches!(result, CastResult::Started { .. }));
        assert!(!app
            .world()
            .get::<MeridianSeveredPermanent>(patient)
            .unwrap()
            .is_severed(MeridianId::Lung));
        assert_eq!(
            app.world()
                .get::<HealerProfile>(medic)
                .unwrap()
                .contracts
                .len(),
            1
        );
        let events = app.world().resource::<Events<YidaoEvent>>();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn targeted_patient_without_position_is_rejected() {
        let mut app = app_with_yidao();
        let medic = spawn_medic(&mut app, Realm::Void);
        let patient = spawn_patient(&mut app);
        let mut severed = MeridianSeveredPermanent::default();
        severed.insert(
            MeridianId::Lung,
            crate::cultivation::meridian::severed::SeveredSource::CombatWound,
            1,
        );
        app.world_mut()
            .entity_mut(patient)
            .insert(severed)
            .remove::<Position>();

        let result = resolve_meridian_repair_skill(app.world_mut(), medic, 0, Some(patient));

        assert_eq!(result, rejected(CastRejectReason::InvalidTarget));
        assert_eq!(app.world().resource::<Events<YidaoEvent>>().len(), 0);
    }

    #[test]
    fn failed_meridian_repair_reports_no_qi_transfer() {
        let mut app = app_with_yidao();
        let medic = spawn_medic(&mut app, Realm::Awaken);
        let patient = spawn_patient(&mut app);
        let mut severed = MeridianSeveredPermanent::default();
        severed.insert(
            MeridianId::Lung,
            crate::cultivation::meridian::severed::SeveredSource::CombatWound,
            1,
        );
        app.world_mut().entity_mut(patient).insert(severed);
        let fail_tick = (0..10_000)
            .find(|tick| {
                deterministic_success_roll(medic, patient, MeridianId::Lung, *tick) >= 0.99
            })
            .expect("failure tick");

        let outcome = apply_meridian_repair(app.world_mut(), medic, patient, 0.0, true, fail_tick);

        assert_eq!(outcome.success_count, 0);
        assert_eq!(outcome.failure_count, 1);
        assert_eq!(outcome.qi_transferred, 0.0);
    }

    #[test]
    fn contam_purge_scales_all_sources_by_residual_ratio() {
        let mut app = app_with_yidao();
        let medic = spawn_medic(&mut app, Realm::Void);
        let patient = spawn_patient(&mut app);
        app.world_mut().entity_mut(patient).insert(Contamination {
            entries: vec![
                ContamSource {
                    amount: 20.0,
                    color: ColorKind::Insidious,
                    attacker_id: None,
                    introduced_at: 1,
                },
                ContamSource {
                    amount: 10.0,
                    color: ColorKind::Violent,
                    attacker_id: None,
                    introduced_at: 2,
                },
            ],
        });

        let result = resolve_contam_purge_skill(app.world_mut(), medic, 1, Some(patient));

        assert!(matches!(result, CastResult::Started { .. }));
        assert_eq!(
            app.world()
                .get::<Contamination>(patient)
                .map(contamination_total)
                .unwrap_or_default(),
            0.0
        );
    }

    #[test]
    fn emergency_resuscitate_revives_near_death_and_clears_bleeding() {
        let mut app = app_with_yidao();
        let medic = spawn_medic(&mut app, Realm::Induce);
        let patient = spawn_patient(&mut app);
        app.world_mut().entity_mut(patient).insert(Wounds {
            health_current: 0.0,
            health_max: 100.0,
            entries: vec![crate::combat::components::Wound {
                location: crate::combat::components::BodyPart::Chest,
                kind: crate::combat::components::WoundKind::Cut,
                severity: 0.8,
                bleeding_per_sec: 8.0,
                created_at_tick: 90,
                inflicted_by: None,
            }],
        });
        let mut lifecycle = Lifecycle::default();
        lifecycle.enter_near_death(90);
        app.world_mut().entity_mut(patient).insert(lifecycle);

        let result = resolve_emergency_resuscitate_skill(app.world_mut(), medic, 2, Some(patient));

        assert!(matches!(result, CastResult::Started { .. }));
        let wounds = app.world().get::<Wounds>(patient).unwrap();
        assert!(wounds.health_current > 0.0);
        assert_eq!(wounds.entries[0].bleeding_per_sec, 0.0);
        assert_eq!(
            app.world().get::<Lifecycle>(patient).unwrap().state,
            LifecycleState::Alive
        );
    }

    #[test]
    fn emergency_resuscitate_requires_active_near_death_window() {
        let mut app = app_with_yidao();
        let medic = spawn_medic(&mut app, Realm::Induce);
        let patient = spawn_patient(&mut app);
        app.world_mut().entity_mut(patient).insert(Wounds {
            health_current: 0.0,
            health_max: 100.0,
            entries: vec![crate::combat::components::Wound {
                location: crate::combat::components::BodyPart::Chest,
                kind: crate::combat::components::WoundKind::Cut,
                severity: 0.8,
                bleeding_per_sec: 8.0,
                created_at_tick: 90,
                inflicted_by: None,
            }],
        });
        app.world_mut().entity_mut(patient).insert(Lifecycle {
            state: LifecycleState::NearDeath,
            last_death_tick: None,
            ..Default::default()
        });

        let result = resolve_emergency_resuscitate_skill(app.world_mut(), medic, 2, Some(patient));

        assert_eq!(result, rejected(CastRejectReason::InvalidTarget));
        assert_eq!(
            app.world().get::<Wounds>(patient).unwrap().health_current,
            0.0
        );
        assert_eq!(
            app.world().get::<Cultivation>(medic).unwrap().qi_current,
            300.0
        );
        assert_eq!(app.world().resource::<Events<YidaoEvent>>().len(), 0);
    }

    #[test]
    fn life_extension_requires_spirit_realm_and_pays_permanent_costs() {
        let mut app = app_with_yidao();
        let medic = spawn_medic(&mut app, Realm::Spirit);
        let patient = spawn_patient(&mut app);
        let mut lifecycle = Lifecycle::default();
        lifecycle.enter_near_death(95);
        app.world_mut().entity_mut(patient).insert(lifecycle);

        let result = resolve_life_extension_skill(app.world_mut(), medic, 3, Some(patient));

        assert!(matches!(result, CastResult::Started { .. }));
        assert_eq!(
            app.world().get::<Lifecycle>(patient).unwrap().state,
            LifecycleState::Alive
        );
        assert!(app.world().get::<Cultivation>(medic).unwrap().qi_max < 300.0);
        assert!(app.world().get::<Cultivation>(patient).unwrap().qi_max < 100.0);
        assert!(app.world().get::<Karma>(medic).unwrap().weight > 0.0);
    }

    #[test]
    fn life_extension_requires_death_tick_before_emitting_event() {
        let mut app = app_with_yidao();
        let medic = spawn_medic(&mut app, Realm::Spirit);
        let patient = spawn_patient(&mut app);
        app.world_mut().entity_mut(patient).insert(Lifecycle {
            state: LifecycleState::NearDeath,
            last_death_tick: None,
            ..Default::default()
        });

        let result = resolve_life_extension_skill(app.world_mut(), medic, 3, Some(patient));

        assert_eq!(result, rejected(CastRejectReason::InvalidTarget));
        assert_eq!(
            app.world().get::<Lifecycle>(patient).unwrap().state,
            LifecycleState::NearDeath
        );
        assert_eq!(
            app.world().get::<Cultivation>(medic).unwrap().qi_current,
            300.0
        );
        assert_eq!(app.world().get::<Cultivation>(medic).unwrap().qi_max, 300.0);
        assert_eq!(app.world().resource::<Events<YidaoEvent>>().len(), 0);
    }

    #[test]
    fn life_extension_rejects_self_target() {
        let mut app = app_with_yidao();
        let medic = spawn_medic(&mut app, Realm::Spirit);
        app.world_mut().entity_mut(medic).insert(Wounds {
            health_current: 0.0,
            health_max: 100.0,
            entries: Vec::new(),
        });
        let mut lifecycle = Lifecycle::default();
        lifecycle.enter_near_death(95);
        app.world_mut().entity_mut(medic).insert(lifecycle);

        let result = resolve_life_extension_skill(app.world_mut(), medic, 3, Some(medic));

        assert_eq!(result, rejected(CastRejectReason::InvalidTarget));
        assert_eq!(
            app.world().get::<Lifecycle>(medic).unwrap().state,
            LifecycleState::NearDeath
        );
        assert_eq!(
            app.world().get::<Cultivation>(medic).unwrap().qi_current,
            300.0
        );
        assert_eq!(app.world().resource::<Events<YidaoEvent>>().len(), 0);
    }

    #[test]
    fn mass_meridian_repair_repairs_multiple_patients_and_scales_costs_by_n() {
        let mut app = app_with_yidao();
        let medic = spawn_medic(&mut app, Realm::Void);
        app.world_mut().entity_mut(medic).insert(HealingMastery {
            mass_meridian_repair: 100.0,
            ..Default::default()
        });
        let mut patients = Vec::new();
        for meridian in [MeridianId::Lung, MeridianId::Heart, MeridianId::Kidney] {
            let patient = spawn_patient(&mut app);
            let mut severed = MeridianSeveredPermanent::default();
            severed.insert(
                meridian,
                crate::cultivation::meridian::severed::SeveredSource::CombatWound,
                1,
            );
            app.world_mut().entity_mut(patient).insert(severed);
            patients.push(patient);
        }

        let result =
            resolve_mass_meridian_repair_skill(app.world_mut(), medic, 4, Some(patients[0]));

        assert!(matches!(result, CastResult::Started { .. }));
        let repaired = patients
            .iter()
            .filter(|patient| {
                app.world()
                    .get::<MeridianSeveredPermanent>(**patient)
                    .unwrap()
                    .severed_count()
                    == 0
            })
            .count();
        assert_eq!(repaired, 3);
        assert!(app.world().get::<Cultivation>(medic).unwrap().qi_max < 300.0);
    }

    #[test]
    fn healer_growth_is_per_cast_while_reputation_tracks_successful_patients() {
        let mut app = app_with_yidao();
        let medic = spawn_medic(&mut app, Realm::Void);
        let patients = vec![
            spawn_patient(&mut app),
            spawn_patient(&mut app),
            spawn_patient(&mut app),
        ];

        grow_healer_identity(
            app.world_mut(),
            medic,
            &patients,
            YidaoSkillId::MassMeridianRepair,
            3,
            120,
        );

        let mastery = app.world().get::<HealingMastery>(medic).unwrap();
        assert_eq!(mastery.mass_meridian_repair, 0.5);
        let profile = app.world().get::<HealerProfile>(medic).unwrap();
        assert_eq!(profile.reputation, 3);
        assert_eq!(profile.contracts.len(), 3);
    }

    #[test]
    fn healer_npc_decision_prioritizes_retreat_then_life_extension() {
        assert_eq!(
            healer_npc_decision(0.1, 1, 80.0, true, true, true).action,
            HealerNpcAction::Retreat
        );
        assert_eq!(
            healer_npc_decision(0.1, 1, 80.0, true, false, true).action,
            HealerNpcAction::LifeExtension
        );
    }
}
