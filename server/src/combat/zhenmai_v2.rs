use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, Event, GameMode, IntoSystemConfigs,
    Position, Query, Res, UniqueId, Update, Username,
};

use crate::combat::components::{
    CastSource, Casting, SkillBarBindings, StatusEffects, Wounds, TICKS_PER_SECOND,
};
use crate::combat::events::{AttackSource, StatusEffectKind};
use crate::combat::status::has_active_status;
use crate::combat::{CombatClock, CombatSystemSet};
use crate::cultivation::color::{record_style_practice, PracticeLog};
use crate::cultivation::components::{
    ColorKind, Contamination, Cultivation, MeridianId, MeridianSystem, Realm,
};
use crate::cultivation::meridian::severed::{
    check_meridian_dependencies, enforce_severed_state, MeridianSeveredEvent,
    MeridianSeveredPermanent, SeveredSource, SkillMeridianDependencies,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::cast_emit::current_unix_millis;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::player::state::canonical_player_id;
use crate::qi_physics::constants::QI_DRAIN_CLAMP;
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason};
use crate::qi_physics::{multi_point_dispersion, reverse_clamp, sever_meridian, QI_ZHENMAI_BETA};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::skill::config::SkillConfigStore;
use crate::skill::events::{SkillXpGain, XpGainSource};

pub const PLAN_ID: &str = "zhenmai-v2";
pub const PARRY_SKILL_ID: &str = "zhenmai.parry";
pub const NEUTRALIZE_SKILL_ID: &str = "zhenmai.neutralize";
pub const MULTIPOINT_SKILL_ID: &str = "zhenmai.multipoint";
pub const HARDEN_SKILL_ID: &str = "zhenmai.harden";
pub const SEVER_CHAIN_SKILL_ID: &str = "zhenmai.sever_chain";

pub const BACKFIRE_AMPLIFICATION_TICKS: u64 = 60 * TICKS_PER_SECOND;
pub const SEVER_CHAIN_COOLDOWN_TICKS: u64 = 20 * 60 * TICKS_PER_SECOND;
pub const PARRY_QI_COST: f64 = 8.0;
pub const NORMAL_DRAIN_CLAMP: f64 = QI_DRAIN_CLAMP;

const PARRY_ANIM_ID: &str = "bong:zhenmai_parry";
const NEUTRALIZE_ANIM_ID: &str = "bong:zhenmai_neutralize";
const MULTIPOINT_ANIM_ID: &str = "bong:zhenmai_multipoint";
const HARDEN_ANIM_ID: &str = "bong:zhenmai_harden";
const SEVER_CHAIN_ANIM_ID: &str = "bong:zhenmai_sever_chain";

const PARRY_PARTICLE_ID: &str = "bong:jiemai_burst_blood";
const NEUTRALIZE_PARTICLE_ID: &str = "bong:jiemai_neutralize_dust";
const SEVER_FLASH_PARTICLE_ID: &str = "bong:jiemai_sever_flash";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhenmaiSkillId {
    Parry,
    Neutralize,
    MultiPoint,
    HardenMeridian,
    SeverChain,
}

impl ZhenmaiSkillId {
    pub fn xp_amount(self) -> u32 {
        match self {
            Self::Parry | Self::Neutralize | Self::HardenMeridian => 1,
            Self::MultiPoint => 2,
            Self::SeverChain => 5,
        }
    }

    fn action(self) -> &'static str {
        match self {
            Self::Parry => "parry",
            Self::Neutralize => "neutralize",
            Self::MultiPoint => "multipoint",
            Self::HardenMeridian => "harden",
            Self::SeverChain => "sever_chain",
        }
    }

    fn audio_recipe(self) -> &'static str {
        match self {
            Self::Parry => "zhenmai_parry_thud",
            Self::Neutralize => "zhenmai_neutralize_hiss",
            Self::MultiPoint => "zhenmai_shield_hum",
            Self::HardenMeridian => "zhenmai_shield_hum",
            Self::SeverChain => "zhenmai_sever_crack",
        }
    }

    fn anim_id(self) -> &'static str {
        match self {
            Self::Parry => PARRY_ANIM_ID,
            Self::Neutralize => NEUTRALIZE_ANIM_ID,
            Self::MultiPoint => MULTIPOINT_ANIM_ID,
            Self::HardenMeridian => HARDEN_ANIM_ID,
            Self::SeverChain => SEVER_CHAIN_ANIM_ID,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhenmaiAttackKind {
    RealYuan,
    PhysicalCarrier,
    TaintedYuan,
    Array,
}

impl ZhenmaiAttackKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "real_yuan" => Some(Self::RealYuan),
            "physical_carrier" => Some(Self::PhysicalCarrier),
            "tainted_yuan" => Some(Self::TaintedYuan),
            "array" => Some(Self::Array),
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RealYuan => "real_yuan",
            Self::PhysicalCarrier => "physical_carrier",
            Self::TaintedYuan => "tainted_yuan",
            Self::Array => "array",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParryProfile {
    pub k_drain: f64,
    pub self_damage: f32,
    pub window_ms: u32,
    pub cooldown_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NeutralizeProfile {
    pub qi_per_contam_percent: f64,
    pub max_percent: f64,
    pub cooldown_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MultiPointProfile {
    pub points: u8,
    pub duration_ticks: u64,
    pub start_qi: f64,
    pub qi_per_second: f64,
    pub k_drain: f64,
    pub cooldown_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HardenProfile {
    pub damage_multiplier: f32,
    pub duration_ticks: u64,
    pub start_qi: f64,
    pub qi_per_second: f64,
    pub max_meridians: u8,
    pub cooldown_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeverChainProfile {
    pub k_drain: f64,
    pub incoming_damage_multiplier: f32,
    pub qi_cost: f64,
    pub grants_amplification: bool,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct MultiPointActive {
    pub started_at_tick: u64,
    pub expires_at_tick: u64,
    pub points: u8,
    pub k_drain: f64,
    pub qi_per_second: f64,
    pub contact_count: u32,
    pub self_damage_per_contact: f32,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct MeridianHardenActive {
    pub started_at_tick: u64,
    pub expires_at_tick: u64,
    pub meridians: Vec<MeridianId>,
    pub damage_multiplier: f32,
    pub qi_per_second: f64,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct BackfireAmplification {
    pub meridian_id: MeridianId,
    pub attack_kind: ZhenmaiAttackKind,
    pub started_at_tick: u64,
    pub expires_at_tick: u64,
    pub k_drain: f64,
    #[serde(alias = "self_damage_multiplier")]
    pub incoming_damage_multiplier: f32,
}

impl BackfireAmplification {
    pub fn active_for(&self, kind: ZhenmaiAttackKind, tick: u64) -> bool {
        self.attack_kind == kind && tick < self.expires_at_tick
    }
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct LocalNeutralizeEvent {
    pub caster: Entity,
    pub meridian_id: MeridianId,
    pub contam_removed: f64,
    pub qi_spent: f64,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct MultiPointBackfireEvent {
    pub defender: Entity,
    pub attacker: Option<Entity>,
    pub attack_kind: ZhenmaiAttackKind,
    pub contact_index: u32,
    pub reflected_qi: f64,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct MeridianHardenEvent {
    pub caster: Entity,
    pub meridian_ids: Vec<MeridianId>,
    pub damage_multiplier: f32,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct MeridianSeveredVoluntaryEvent {
    pub caster: Entity,
    pub meridian_id: MeridianId,
    pub attack_kind: ZhenmaiAttackKind,
    pub grants_amplification: bool,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct BackfireAmplificationActiveEvent {
    pub caster: Entity,
    pub meridian_id: MeridianId,
    pub attack_kind: ZhenmaiAttackKind,
    pub k_drain: f64,
    pub self_damage_multiplier: f32,
    pub expires_at_tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct JiemaiBackfireBloodSpray {
    pub defender: Entity,
    pub origin: [f64; 3],
    pub intensity: f32,
    pub tick: u64,
}

pub fn register(app: &mut App) {
    if let Some(mut dependencies) = app
        .world_mut()
        .get_resource_mut::<SkillMeridianDependencies>()
    {
        declare_meridian_dependencies(&mut dependencies);
    } else {
        let mut dependencies = SkillMeridianDependencies::default();
        declare_meridian_dependencies(&mut dependencies);
        app.insert_resource(dependencies);
    }
    app.add_event::<LocalNeutralizeEvent>();
    app.add_event::<MultiPointBackfireEvent>();
    app.add_event::<MeridianHardenEvent>();
    app.add_event::<MeridianSeveredVoluntaryEvent>();
    app.add_event::<BackfireAmplificationActiveEvent>();
    app.add_event::<JiemaiBackfireBloodSpray>();
    app.add_event::<QiTransfer>();
    app.add_systems(
        Update,
        (
            multipoint_duration_tick,
            harden_duration_tick,
            amplification_duration_tick,
        )
            .in_set(CombatSystemSet::Physics),
    );
}

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(PARRY_SKILL_ID, resolve_parry);
    registry.register(NEUTRALIZE_SKILL_ID, resolve_neutralize);
    registry.register(MULTIPOINT_SKILL_ID, resolve_multipoint);
    registry.register(HARDEN_SKILL_ID, resolve_harden);
    registry.register(SEVER_CHAIN_SKILL_ID, resolve_sever_chain);
}

pub fn declare_meridian_dependencies(dependencies: &mut SkillMeridianDependencies) {
    dependencies.declare(PARRY_SKILL_ID, vec![MeridianId::Lung]);
    dependencies.declare(NEUTRALIZE_SKILL_ID, vec![MeridianId::Lung]);
    dependencies.declare(MULTIPOINT_SKILL_ID, vec![MeridianId::Lung]);
    dependencies.declare(HARDEN_SKILL_ID, vec![MeridianId::Lung]);
    // 绝脉断链还会检查 SkillConfig 选定的动态经脉；这里注册空依赖用于显式声明。
    dependencies.declare(SEVER_CHAIN_SKILL_ID, Vec::new());
}

pub fn parry_profile(realm: Realm, skill_lv: u8) -> ParryProfile {
    ParryProfile {
        k_drain: parry_k_drain_for_realm(realm),
        self_damage: parry_self_damage_for_realm(realm),
        window_ms: parry_window_ms(skill_lv),
        cooldown_ticks: cooldown_ticks_by_skill(30.0, 5.0, skill_lv),
    }
}

pub fn parry_qi_cost_for_realm(_realm: Realm) -> Option<f64> {
    Some(PARRY_QI_COST)
}

pub fn parry_k_drain_for_realm(realm: Realm) -> f64 {
    match realm {
        Realm::Awaken => 0.05,
        Realm::Induce => 0.15,
        Realm::Condense => 0.30,
        Realm::Solidify => 0.40,
        Realm::Spirit | Realm::Void => NORMAL_DRAIN_CLAMP,
    }
}

pub fn parry_self_damage_for_realm(realm: Realm) -> f32 {
    match realm {
        Realm::Awaken | Realm::Induce => 8.0,
        Realm::Condense => 6.0,
        Realm::Solidify => 5.0,
        Realm::Spirit => 4.0,
        Realm::Void => 3.0,
    }
}

pub fn parry_window_ms(skill_lv: u8) -> u32 {
    (100.0 + 150.0 * skill_factor(skill_lv)).round() as u32
}

pub fn neutralize_profile(realm: Realm, skill_lv: u8) -> NeutralizeProfile {
    let (qi_per_contam_percent, max_percent) = match realm {
        Realm::Awaken => (18.0, 1.0),
        Realm::Induce => (16.0, 2.0),
        Realm::Condense => (14.0, 4.0),
        Realm::Solidify => (12.0, 7.0),
        Realm::Spirit => (10.0, 10.0),
        Realm::Void => (8.0, 15.0),
    };
    NeutralizeProfile {
        qi_per_contam_percent,
        max_percent,
        cooldown_ticks: cooldown_ticks_by_skill(10.0, 3.0, skill_lv),
    }
}

pub fn multipoint_profile(realm: Realm, skill_lv: u8) -> MultiPointProfile {
    let (points, duration_seconds, start_qi, qi_per_second, k_drain) = match realm {
        Realm::Awaken => (3, 3, 12.0, 1.0, 0.05),
        Realm::Induce => (4, 4, 12.0, 1.0, 0.10),
        Realm::Condense => (5, 5, 15.0, 1.5, 0.20),
        Realm::Solidify => (6, 7, 18.0, 2.0, 0.30),
        Realm::Spirit => (7, 9, 22.0, 2.5, 0.35),
        Realm::Void => (8, 12, 28.0, 3.5, 0.35),
    };
    MultiPointProfile {
        points,
        duration_ticks: duration_seconds * TICKS_PER_SECOND,
        start_qi,
        qi_per_second,
        k_drain,
        cooldown_ticks: cooldown_ticks_by_skill(30.0, 8.0, skill_lv),
    }
}

pub fn harden_profile(realm: Realm, skill_lv: u8) -> HardenProfile {
    let (damage_multiplier, duration_seconds, start_qi, qi_per_second, max_meridians) = match realm
    {
        Realm::Awaken => (0.85, 10, 8.0, 0.5, 1),
        Realm::Induce => (0.80, 15, 8.0, 0.5, 1),
        Realm::Condense => (0.65, 20, 10.0, 0.7, 1),
        Realm::Solidify => (0.50, 30, 12.0, 1.0, 1),
        Realm::Spirit => (0.35, 45, 15.0, 1.5, 1),
        Realm::Void => (0.20, 90, 22.0, 2.5, 2),
    };
    HardenProfile {
        damage_multiplier,
        duration_ticks: duration_seconds * TICKS_PER_SECOND,
        start_qi,
        qi_per_second,
        max_meridians,
        cooldown_ticks: cooldown_ticks_by_skill(15.0, 5.0, skill_lv),
    }
}

pub fn sever_chain_profile(realm: Realm) -> SeverChainProfile {
    match realm {
        Realm::Void => SeverChainProfile {
            k_drain: sever_meridian(NORMAL_DRAIN_CLAMP, 3.0),
            incoming_damage_multiplier: 0.5,
            qi_cost: 50.0,
            grants_amplification: true,
        },
        Realm::Spirit => SeverChainProfile {
            k_drain: sever_meridian(NORMAL_DRAIN_CLAMP, 2.4),
            incoming_damage_multiplier: 0.7,
            qi_cost: 60.0,
            grants_amplification: true,
        },
        _ => SeverChainProfile {
            k_drain: 0.0,
            incoming_damage_multiplier: 1.0,
            qi_cost: 50.0,
            grants_amplification: false,
        },
    }
}

pub fn style_weight(kind: ZhenmaiAttackKind) -> f64 {
    match kind {
        ZhenmaiAttackKind::RealYuan => 0.5,
        ZhenmaiAttackKind::PhysicalCarrier => 0.7,
        ZhenmaiAttackKind::Array => 0.2,
        ZhenmaiAttackKind::TaintedYuan => 0.0,
    }
}

pub fn attack_kind_for_source(
    source: AttackSource,
    wound_kind: crate::combat::components::WoundKind,
) -> ZhenmaiAttackKind {
    match source {
        AttackSource::QiNeedle => ZhenmaiAttackKind::TaintedYuan,
        AttackSource::BurstMeridian | AttackSource::FullPower => ZhenmaiAttackKind::RealYuan,
        AttackSource::SwordCleave | AttackSource::SwordThrust => ZhenmaiAttackKind::PhysicalCarrier,
        // plan-sword-path-v2 §P1.6 — 凝锋附魔走物理（依然是金属剑刃斩击）；
        // 剑气斩/剑鸣/化形是凝实/锋锐色真元成形，按 RealYuan 走截脉反馈。
        AttackSource::SwordPathCondenseEdge => ZhenmaiAttackKind::PhysicalCarrier,
        AttackSource::SwordPathQiSlash
        | AttackSource::SwordPathResonance
        | AttackSource::SwordPathManifest => ZhenmaiAttackKind::RealYuan,
        AttackSource::Melee if wound_kind == crate::combat::components::WoundKind::Pierce => {
            ZhenmaiAttackKind::PhysicalCarrier
        }
        AttackSource::Melee => ZhenmaiAttackKind::RealYuan,
    }
}

pub fn reflected_qi(hit_qi: f64, k_drain: f64, kind: ZhenmaiAttackKind) -> f64 {
    reverse_clamp(hit_qi, k_drain, style_weight(kind), QI_ZHENMAI_BETA)
}

pub fn backfire_transfer(
    attacker_id: QiAccountId,
    defender_id: QiAccountId,
    amount: f64,
) -> Option<QiTransfer> {
    QiTransfer::new(
        attacker_id,
        defender_id,
        amount,
        QiTransferReason::Collision,
    )
    .ok()
}

fn resolve_parry(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = now_tick(world);
    if skill_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    if is_control_locked(world, caster) {
        return rejected(CastRejectReason::InRecovery);
    }
    if let Err(reason) = check_static_meridian_dependencies(world, caster, PARRY_SKILL_ID) {
        return rejected(reason);
    }
    let Some(realm) = world.get::<Cultivation>(caster).map(|c| c.realm) else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    let profile = parry_profile(realm, skill_lv_0_to_100(world, caster));
    if !spend_qi(world, caster, PARRY_QI_COST) {
        return rejected(CastRejectReason::QiInsufficient);
    }
    world.send_event(crate::combat::events::DefenseIntent {
        defender: caster,
        issued_at_tick: now_tick,
    });
    insert_casting_snapshot(
        world,
        caster,
        slot,
        PARRY_SKILL_ID,
        1,
        profile.cooldown_ticks,
        now_tick,
    );
    set_skill_cooldown(
        world,
        caster,
        slot,
        now_tick.saturating_add(profile.cooldown_ticks),
    );
    record_practice(world, caster, ZhenmaiSkillId::Parry);
    emit_skill_feedback(
        world,
        caster,
        ZhenmaiSkillId::Parry,
        PARRY_PARTICLE_ID,
        "#B6172F",
        0.8,
        8,
    );
    CastResult::Started {
        cooldown_ticks: profile.cooldown_ticks,
        anim_duration_ticks: 1,
    }
}

fn resolve_neutralize(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = now_tick(world);
    if skill_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    if let Err(reason) = check_static_meridian_dependencies(world, caster, NEUTRALIZE_SKILL_ID) {
        return rejected(reason);
    }
    let Some(realm) = world.get::<Cultivation>(caster).map(|c| c.realm) else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    let profile = neutralize_profile(realm, skill_lv_0_to_100(world, caster));
    let meridian_id = configured_meridian(world, caster, NEUTRALIZE_SKILL_ID)
        .or_else(|| first_open_meridian(world, caster))
        .unwrap_or(MeridianId::Lung);
    if is_meridian_severed(world, caster, meridian_id) {
        return rejected(CastRejectReason::MeridianSevered(Some(meridian_id)));
    }
    let removable = contamination_for_meridian(world, caster, meridian_id).min(profile.max_percent);
    if removable <= f64::EPSILON {
        return rejected(CastRejectReason::InvalidTarget);
    }
    let qi_cost = removable * profile.qi_per_contam_percent;
    if !spend_qi(world, caster, qi_cost) {
        return rejected(CastRejectReason::QiInsufficient);
    }
    let removed = reduce_contamination_for_meridian(world, caster, meridian_id, removable);
    world.send_event(LocalNeutralizeEvent {
        caster,
        meridian_id,
        contam_removed: removed,
        qi_spent: qi_cost,
        tick: now_tick,
    });
    insert_casting_snapshot(
        world,
        caster,
        slot,
        NEUTRALIZE_SKILL_ID,
        4,
        profile.cooldown_ticks,
        now_tick,
    );
    set_skill_cooldown(
        world,
        caster,
        slot,
        now_tick.saturating_add(profile.cooldown_ticks),
    );
    record_practice(world, caster, ZhenmaiSkillId::Neutralize);
    emit_skill_feedback(
        world,
        caster,
        ZhenmaiSkillId::Neutralize,
        NEUTRALIZE_PARTICLE_ID,
        "#9CA3AF",
        0.55,
        10,
    );
    CastResult::Started {
        cooldown_ticks: profile.cooldown_ticks,
        anim_duration_ticks: 4,
    }
}

fn resolve_multipoint(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = now_tick(world);
    if skill_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    if world.get::<MultiPointActive>(caster).is_some() {
        return rejected(CastRejectReason::InRecovery);
    }
    if let Err(reason) = check_static_meridian_dependencies(world, caster, MULTIPOINT_SKILL_ID) {
        return rejected(reason);
    }
    let Some(realm) = world.get::<Cultivation>(caster).map(|c| c.realm) else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    let profile = multipoint_profile(realm, skill_lv_0_to_100(world, caster));
    if !spend_qi(world, caster, profile.start_qi) {
        return rejected(CastRejectReason::QiInsufficient);
    }
    world.entity_mut(caster).insert(MultiPointActive {
        started_at_tick: now_tick,
        expires_at_tick: now_tick.saturating_add(profile.duration_ticks),
        points: profile.points,
        k_drain: profile.k_drain,
        qi_per_second: profile.qi_per_second,
        contact_count: 0,
        self_damage_per_contact: 1.0,
    });
    insert_casting_snapshot(
        world,
        caster,
        slot,
        MULTIPOINT_SKILL_ID,
        6,
        profile.cooldown_ticks,
        now_tick,
    );
    set_skill_cooldown(
        world,
        caster,
        slot,
        now_tick.saturating_add(profile.cooldown_ticks),
    );
    record_practice(world, caster, ZhenmaiSkillId::MultiPoint);
    emit_skill_feedback(
        world,
        caster,
        ZhenmaiSkillId::MultiPoint,
        PARRY_PARTICLE_ID,
        "#9B1C31",
        0.7,
        16,
    );
    CastResult::Started {
        cooldown_ticks: profile.cooldown_ticks,
        anim_duration_ticks: 6,
    }
}

fn resolve_harden(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = now_tick(world);
    if skill_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    if let Err(reason) = check_static_meridian_dependencies(world, caster, HARDEN_SKILL_ID) {
        return rejected(reason);
    }
    let Some(realm) = world.get::<Cultivation>(caster).map(|c| c.realm) else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    let profile = harden_profile(realm, skill_lv_0_to_100(world, caster));
    let configured = configured_meridian(world, caster, HARDEN_SKILL_ID);
    let mut meridians = configured.into_iter().collect::<Vec<_>>();
    if meridians.is_empty() {
        meridians.extend(
            open_meridians(world, caster)
                .into_iter()
                .take(profile.max_meridians as usize),
        );
    }
    if meridians.is_empty() {
        meridians.push(MeridianId::Lung);
    }
    meridians.truncate(profile.max_meridians as usize);
    if let Some(blocking) = meridians
        .iter()
        .copied()
        .find(|id| is_meridian_severed(world, caster, *id))
    {
        return rejected(CastRejectReason::MeridianSevered(Some(blocking)));
    }
    if !spend_qi(world, caster, profile.start_qi) {
        return rejected(CastRejectReason::QiInsufficient);
    }
    world.entity_mut(caster).insert(MeridianHardenActive {
        started_at_tick: now_tick,
        expires_at_tick: now_tick.saturating_add(profile.duration_ticks),
        meridians: meridians.clone(),
        damage_multiplier: profile.damage_multiplier,
        qi_per_second: profile.qi_per_second,
    });
    world.send_event(MeridianHardenEvent {
        caster,
        meridian_ids: meridians,
        damage_multiplier: profile.damage_multiplier,
        tick: now_tick,
    });
    insert_casting_snapshot(
        world,
        caster,
        slot,
        HARDEN_SKILL_ID,
        5,
        profile.cooldown_ticks,
        now_tick,
    );
    set_skill_cooldown(
        world,
        caster,
        slot,
        now_tick.saturating_add(profile.cooldown_ticks),
    );
    record_practice(world, caster, ZhenmaiSkillId::HardenMeridian);
    emit_skill_feedback(
        world,
        caster,
        ZhenmaiSkillId::HardenMeridian,
        NEUTRALIZE_PARTICLE_ID,
        "#C7A94B",
        0.45,
        8,
    );
    CastResult::Started {
        cooldown_ticks: profile.cooldown_ticks,
        anim_duration_ticks: 5,
    }
}

fn resolve_sever_chain(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = now_tick(world);
    if skill_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    if let Err(reason) = check_static_meridian_dependencies(world, caster, SEVER_CHAIN_SKILL_ID) {
        return rejected(reason);
    }
    let Some(realm) = world.get::<Cultivation>(caster).map(|c| c.realm) else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    let Some((meridian_id, attack_kind)) = configured_sever_chain(world, caster) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if is_meridian_severed(world, caster, meridian_id) {
        return rejected(CastRejectReason::MeridianSevered(Some(meridian_id)));
    }
    let profile = sever_chain_profile(realm);
    if profile.qi_cost > 0.0 && !spend_qi(world, caster, profile.qi_cost) {
        return rejected(CastRejectReason::QiInsufficient);
    }
    if let Some(mut meridians) = world.get_mut::<MeridianSystem>(caster) {
        enforce_severed_state(&mut meridians, meridian_id);
    }
    if let Some(mut permanent) = world.get_mut::<MeridianSeveredPermanent>(caster) {
        permanent.insert(meridian_id, SeveredSource::VoluntarySever, now_tick);
    } else {
        world
            .entity_mut(caster)
            .insert(MeridianSeveredPermanent::default());
        if let Some(mut permanent) = world.get_mut::<MeridianSeveredPermanent>(caster) {
            permanent.insert(meridian_id, SeveredSource::VoluntarySever, now_tick);
        }
    }
    world.send_event(MeridianSeveredEvent {
        entity: caster,
        meridian_id,
        source: SeveredSource::VoluntarySever,
        at_tick: now_tick,
    });
    world.send_event(MeridianSeveredVoluntaryEvent {
        caster,
        meridian_id,
        attack_kind,
        grants_amplification: profile.grants_amplification,
        tick: now_tick,
    });
    if profile.grants_amplification {
        let amplification = BackfireAmplification {
            meridian_id,
            attack_kind,
            started_at_tick: now_tick,
            expires_at_tick: now_tick.saturating_add(BACKFIRE_AMPLIFICATION_TICKS),
            k_drain: profile.k_drain,
            incoming_damage_multiplier: profile.incoming_damage_multiplier,
        };
        world.entity_mut(caster).insert(amplification.clone());
        world.send_event(BackfireAmplificationActiveEvent {
            caster,
            meridian_id,
            attack_kind,
            k_drain: amplification.k_drain,
            self_damage_multiplier: amplification.incoming_damage_multiplier,
            expires_at_tick: amplification.expires_at_tick,
        });
    }
    insert_casting_snapshot(
        world,
        caster,
        slot,
        SEVER_CHAIN_SKILL_ID,
        8,
        SEVER_CHAIN_COOLDOWN_TICKS,
        now_tick,
    );
    set_skill_cooldown(
        world,
        caster,
        slot,
        now_tick.saturating_add(SEVER_CHAIN_COOLDOWN_TICKS),
    );
    record_practice(world, caster, ZhenmaiSkillId::SeverChain);
    emit_skill_feedback(
        world,
        caster,
        ZhenmaiSkillId::SeverChain,
        SEVER_FLASH_PARTICLE_ID,
        "#F4C542",
        1.0,
        18,
    );
    CastResult::Started {
        cooldown_ticks: SEVER_CHAIN_COOLDOWN_TICKS,
        anim_duration_ticks: 8,
    }
}

pub fn apply_reflected_qi(
    world: &mut bevy_ecs::world::World,
    attacker: Entity,
    amount: f64,
) -> f64 {
    if amount <= f64::EPSILON {
        return 0.0;
    }
    let Some(mut cultivation) = world.get_mut::<Cultivation>(attacker) else {
        return 0.0;
    };
    let before = cultivation.qi_current;
    cultivation.qi_current = (cultivation.qi_current - amount).clamp(0.0, cultivation.qi_max);
    before - cultivation.qi_current
}

pub fn apply_self_damage(wounds: &mut Wounds, amount: f32) -> f32 {
    if amount <= f32::EPSILON {
        return 0.0;
    }
    let before = wounds.health_current;
    wounds.health_current = (wounds.health_current - amount).clamp(0.0, wounds.health_max);
    before - wounds.health_current
}

pub fn apply_self_damage_to_entity(
    world: &mut bevy_ecs::world::World,
    entity: Entity,
    amount: f32,
) -> f32 {
    if world
        .get::<GameMode>(entity)
        .is_some_and(|game_mode| *game_mode != GameMode::Survival)
    {
        return 0.0;
    }
    let Some(mut wounds) = world.get_mut::<Wounds>(entity) else {
        return 0.0;
    };
    apply_self_damage(&mut wounds, amount)
}

pub fn multipoint_contact(
    active: &mut MultiPointActive,
    hit_qi: f64,
    kind: ZhenmaiAttackKind,
) -> f64 {
    active.contact_count = active.contact_count.saturating_add(1);
    multi_point_dispersion(
        hit_qi,
        active.k_drain,
        style_weight(kind),
        QI_ZHENMAI_BETA,
        active.points,
    )
}

fn multipoint_duration_tick(
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut query: Query<(Entity, &MultiPointActive, Option<&mut Cultivation>)>,
) {
    for (entity, active, cultivation) in &mut query {
        if clock.tick >= active.expires_at_tick {
            commands.entity(entity).remove::<MultiPointActive>();
            continue;
        }
        if clock.tick > active.started_at_tick
            && (clock.tick - active.started_at_tick) % TICKS_PER_SECOND == 0
        {
            if let Some(mut cultivation) = cultivation {
                cultivation.qi_current =
                    (cultivation.qi_current - active.qi_per_second).clamp(0.0, cultivation.qi_max);
                if cultivation.qi_current <= f64::EPSILON {
                    commands.entity(entity).remove::<MultiPointActive>();
                }
            }
        }
    }
}

fn harden_duration_tick(
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut query: Query<(Entity, &MeridianHardenActive, Option<&mut Cultivation>)>,
) {
    for (entity, active, cultivation) in &mut query {
        if clock.tick >= active.expires_at_tick {
            commands.entity(entity).remove::<MeridianHardenActive>();
            continue;
        }
        if clock.tick > active.started_at_tick
            && (clock.tick - active.started_at_tick) % TICKS_PER_SECOND == 0
        {
            if let Some(mut cultivation) = cultivation {
                cultivation.qi_current =
                    (cultivation.qi_current - active.qi_per_second).clamp(0.0, cultivation.qi_max);
                if cultivation.qi_current <= f64::EPSILON {
                    commands.entity(entity).remove::<MeridianHardenActive>();
                }
            }
        }
    }
}

fn amplification_duration_tick(
    clock: Res<CombatClock>,
    mut commands: Commands,
    query: Query<(Entity, &BackfireAmplification)>,
) {
    for (entity, active) in &query {
        if clock.tick >= active.expires_at_tick {
            commands.entity(entity).remove::<BackfireAmplification>();
        }
    }
}

fn cooldown_ticks_by_skill(base_seconds: f32, min_seconds: f32, skill_lv: u8) -> u64 {
    ((base_seconds + (min_seconds - base_seconds) * skill_factor(skill_lv))
        * TICKS_PER_SECOND as f32)
        .round()
        .max(1.0) as u64
}

fn skill_factor(skill_lv: u8) -> f32 {
    f32::from(skill_lv.min(100)) / 100.0
}

fn skill_lv_0_to_100(world: &bevy_ecs::world::World, caster: Entity) -> u8 {
    let Some(set) = world.get::<crate::skill::components::SkillSet>(caster) else {
        return 0;
    };
    set.skills
        .get(&crate::skill::components::SkillId::Combat)
        .map(|entry| entry.lv.saturating_mul(10).min(100))
        .unwrap_or(0)
}

fn now_tick(world: &bevy_ecs::world::World) -> u64 {
    world
        .get_resource::<CombatClock>()
        .map(|clock| clock.tick)
        .unwrap_or_default()
}

fn skill_on_cooldown(
    world: &bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    now_tick: u64,
) -> bool {
    world
        .get::<SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
}

fn set_skill_cooldown(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    until_tick: u64,
) {
    if let Some(mut bindings) = world.get_mut::<SkillBarBindings>(caster) {
        bindings.set_cooldown(slot, until_tick);
    }
}

fn insert_casting_snapshot(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    skill_id: &str,
    duration_ticks: u32,
    cooldown_ticks: u64,
    now_tick: u64,
) {
    let duration_ticks = duration_ticks.max(1);
    let started_at_ms = current_unix_millis();
    let start_position = world
        .get::<Position>(caster)
        .map(|position| position.get())
        .unwrap_or(DVec3::ZERO);
    let skill_config = skill_config_snapshot(world, caster, skill_id);
    world.entity_mut(caster).insert(Casting {
        source: CastSource::SkillBar,
        slot,
        started_at_tick: now_tick,
        duration_ticks: u64::from(duration_ticks),
        started_at_ms,
        duration_ms: duration_ticks.saturating_mul(50),
        bound_instance_id: None,
        start_position,
        complete_cooldown_ticks: cooldown_ticks,
        skill_id: Some(skill_id.to_string()),
        skill_config,
    });
}

fn skill_config_snapshot(
    world: &bevy_ecs::world::World,
    caster: Entity,
    skill_id: &str,
) -> Option<crate::skill::config::SkillConfig> {
    let username = world.get::<Username>(caster)?;
    let player_id = canonical_player_id(username.0.as_str());
    let store = world.get_resource::<SkillConfigStore>()?;
    store.config_for(player_id.as_str(), skill_id).cloned()
}

fn check_static_meridian_dependencies(
    world: &bevy_ecs::world::World,
    caster: Entity,
    skill_id: &str,
) -> Result<(), CastRejectReason> {
    let Some(table) = world.get_resource::<SkillMeridianDependencies>() else {
        return Ok(());
    };
    check_meridian_dependencies(
        table.lookup(skill_id),
        world.get::<MeridianSeveredPermanent>(caster),
    )
    .map_err(|id| CastRejectReason::MeridianSevered(Some(id)))
}

fn is_control_locked(world: &bevy_ecs::world::World, caster: Entity) -> bool {
    world.get::<StatusEffects>(caster).is_some_and(|statuses| {
        has_active_status(statuses, StatusEffectKind::Stunned)
            || has_active_status(statuses, StatusEffectKind::VortexCasting)
            || has_active_status(statuses, StatusEffectKind::ParryRecovery)
    })
}

fn spend_qi(world: &mut bevy_ecs::world::World, caster: Entity, amount: f64) -> bool {
    if amount <= f64::EPSILON {
        return true;
    }
    let Some(mut cultivation) = world.get_mut::<Cultivation>(caster) else {
        return false;
    };
    if cultivation.qi_current + f64::EPSILON < amount {
        return false;
    }
    cultivation.qi_current = (cultivation.qi_current - amount).clamp(0.0, cultivation.qi_max);
    true
}

fn contamination_for_meridian(
    world: &bevy_ecs::world::World,
    caster: Entity,
    meridian_id: MeridianId,
) -> f64 {
    world
        .get::<Contamination>(caster)
        .map(|c| {
            c.entries
                .iter()
                .filter(|entry| entry.meridian_id == Some(meridian_id))
                .map(|entry| entry.amount.max(0.0))
                .sum()
        })
        .unwrap_or(0.0)
}

fn reduce_contamination_for_meridian(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    meridian_id: MeridianId,
    amount: f64,
) -> f64 {
    let Some(mut contamination) = world.get_mut::<Contamination>(caster) else {
        return 0.0;
    };
    let mut remaining = amount.max(0.0);
    let mut removed = 0.0;
    for entry in &mut contamination.entries {
        if entry.meridian_id != Some(meridian_id) {
            continue;
        }
        if remaining <= f64::EPSILON {
            break;
        }
        let take = entry.amount.min(remaining).max(0.0);
        entry.amount -= take;
        remaining -= take;
        removed += take;
    }
    contamination
        .entries
        .retain(|entry| entry.amount > f64::EPSILON);
    removed
}

fn is_meridian_severed(
    world: &bevy_ecs::world::World,
    caster: Entity,
    meridian_id: MeridianId,
) -> bool {
    world
        .get::<MeridianSeveredPermanent>(caster)
        .is_some_and(|severed| severed.is_severed(meridian_id))
}

fn first_open_meridian(world: &bevy_ecs::world::World, caster: Entity) -> Option<MeridianId> {
    world.get::<MeridianSystem>(caster).and_then(|meridians| {
        meridians
            .iter()
            .find(|meridian| meridian.opened && meridian.integrity > f64::EPSILON)
            .map(|meridian| meridian.id)
    })
}

fn open_meridians(world: &bevy_ecs::world::World, caster: Entity) -> Vec<MeridianId> {
    world
        .get::<MeridianSystem>(caster)
        .map(|meridians| {
            meridians
                .iter()
                .filter(|meridian| meridian.opened && meridian.integrity > f64::EPSILON)
                .map(|meridian| meridian.id)
                .collect()
        })
        .unwrap_or_default()
}

fn configured_meridian(
    world: &bevy_ecs::world::World,
    caster: Entity,
    skill_id: &str,
) -> Option<MeridianId> {
    configured_fields(world, caster, skill_id)
        .and_then(|fields| parse_meridian_id(fields.get("meridian_id")?))
}

fn configured_sever_chain(
    world: &bevy_ecs::world::World,
    caster: Entity,
) -> Option<(MeridianId, ZhenmaiAttackKind)> {
    let fields = configured_fields(world, caster, SEVER_CHAIN_SKILL_ID)?;
    let meridian_id = parse_meridian_id(fields.get("meridian_id")?)?;
    let kind = fields
        .get("backfire_kind")
        .and_then(Value::as_str)
        .and_then(ZhenmaiAttackKind::parse)?;
    Some((meridian_id, kind))
}

fn configured_fields<'a>(
    world: &'a bevy_ecs::world::World,
    caster: Entity,
    skill_id: &str,
) -> Option<&'a BTreeMap<String, Value>> {
    let username = world.get::<Username>(caster)?;
    let player_id = canonical_player_id(username.0.as_str());
    let store = world.get_resource::<SkillConfigStore>()?;
    Some(&store.config_for(player_id.as_str(), skill_id)?.fields)
}

fn parse_meridian_id(value: &Value) -> Option<MeridianId> {
    serde_json::from_value::<MeridianId>(value.clone()).ok()
}

fn record_practice(world: &mut bevy_ecs::world::World, caster: Entity, skill: ZhenmaiSkillId) {
    if let Some(mut practice_log) = world.get_mut::<PracticeLog>(caster) {
        record_style_practice(&mut practice_log, ColorKind::Violent);
    }
    world.send_event(SkillXpGain {
        char_entity: caster,
        skill: crate::skill::components::SkillId::Combat,
        amount: skill.xp_amount(),
        source: XpGainSource::Action {
            plan_id: PLAN_ID,
            action: skill.action(),
        },
    });
}

fn emit_skill_feedback(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    skill: ZhenmaiSkillId,
    particle_id: &str,
    color: &str,
    strength: f32,
    count: u16,
) {
    let origin = world
        .get::<Position>(caster)
        .map(|position| position.get())
        .unwrap_or(DVec3::ZERO);
    if let Some(unique_id) = world.get::<UniqueId>(caster).copied() {
        world.send_event(VfxEventRequest::new(
            origin,
            VfxEventPayloadV1::PlayAnim {
                target_player: unique_id.0.to_string(),
                anim_id: skill.anim_id().to_string(),
                priority: if skill == ZhenmaiSkillId::SeverChain {
                    1800
                } else {
                    1300
                },
                fade_in_ticks: Some(2),
            },
        ));
    }
    world.send_event(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id: particle_id.to_string(),
            origin: [origin.x, origin.y + 1.0, origin.z],
            direction: Some([0.0, 0.1, 0.0]),
            color: Some(color.to_string()),
            strength: Some(strength),
            count: Some(count),
            duration_ticks: Some(20),
        },
    ));
    world.send_event(PlaySoundRecipeRequest {
        recipe_id: skill.audio_recipe().to_string(),
        instance_id: 0,
        pos: Some([origin.x as i32, origin.y as i32, origin.z as i32]),
        flag: None,
        volume_mul: 1.0,
        pitch_shift: 0.0,
        recipient: AudioRecipient::Radius {
            origin,
            radius: 32.0,
        },
    });
}

fn rejected(reason: CastRejectReason) -> CastResult {
    CastResult::Rejected { reason }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::{WoundKind, Wounds};
    use crate::cultivation::components::{ContamSource, MeridianSystem};
    use crate::skill::config::SkillConfig;
    use valence::prelude::{App, Events, GameMode};

    fn all_realms() -> [Realm; 6] {
        [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ]
    }

    fn app_with_events() -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 100 });
        let mut dependencies = SkillMeridianDependencies::default();
        declare_meridian_dependencies(&mut dependencies);
        app.insert_resource(dependencies);
        app.add_event::<crate::combat::events::DefenseIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_event::<LocalNeutralizeEvent>();
        app.add_event::<MeridianSeveredEvent>();
        app.add_event::<MeridianSeveredVoluntaryEvent>();
        app.add_event::<BackfireAmplificationActiveEvent>();
        app
    }

    fn caster(app: &mut App, realm: Realm, qi: f64) -> Entity {
        let mut meridians = MeridianSystem::default();
        for id in MeridianId::ALL {
            meridians.get_mut(id).opened = true;
        }
        app.world_mut()
            .spawn((
                Username("Azure".to_string()),
                Cultivation {
                    realm,
                    qi_current: qi,
                    qi_max: qi.max(100.0),
                    ..Default::default()
                },
                meridians,
                Wounds::default(),
                Contamination::default(),
                PracticeLog::default(),
                SkillBarBindings::default(),
                MeridianSeveredPermanent::default(),
            ))
            .id()
    }

    fn configure_sever_chain(
        app: &mut App,
        entity: Entity,
        meridian: MeridianId,
        kind: ZhenmaiAttackKind,
    ) {
        let mut store = SkillConfigStore::default();
        store.set_config(
            canonical_player_id("Azure").as_str(),
            SEVER_CHAIN_SKILL_ID,
            SkillConfig::new(BTreeMap::from([
                ("meridian_id".to_string(), serde_json::json!(meridian)),
                (
                    "backfire_kind".to_string(),
                    serde_json::json!(kind.as_str()),
                ),
            ])),
        );
        app.world_mut().insert_resource(store);
        assert_eq!(
            configured_sever_chain(app.world(), entity),
            Some((meridian, kind))
        );
    }

    fn mark_severed(app: &mut App, entity: Entity, meridian: MeridianId) {
        app.world_mut()
            .get_mut::<MeridianSeveredPermanent>(entity)
            .unwrap()
            .insert(meridian, SeveredSource::VoluntarySever, 99);
    }

    #[test]
    fn declare_meridian_dependencies_registers_all_five_skills() {
        let mut dependencies = SkillMeridianDependencies::default();
        declare_meridian_dependencies(&mut dependencies);

        assert!(dependencies.is_declared(PARRY_SKILL_ID));
        assert!(dependencies.is_declared(NEUTRALIZE_SKILL_ID));
        assert!(dependencies.is_declared(MULTIPOINT_SKILL_ID));
        assert!(dependencies.is_declared(HARDEN_SKILL_ID));
        assert!(dependencies.is_declared(SEVER_CHAIN_SKILL_ID));
        assert_eq!(dependencies.lookup(PARRY_SKILL_ID), &[MeridianId::Lung]);
    }

    #[test]
    fn parry_profile_awaken_matches_low_realm_cost() {
        let p = parry_profile(Realm::Awaken, 0);
        assert_eq!(p.k_drain, 0.05);
        assert_eq!(p.self_damage, 8.0);
    }

    #[test]
    fn parry_profile_void_keeps_clamp_and_low_self_damage() {
        let p = parry_profile(Realm::Void, 100);
        assert_eq!(p.k_drain, 0.5);
        assert_eq!(p.self_damage, 3.0);
        assert_eq!(p.window_ms, 250);
    }

    #[test]
    fn parry_window_scales_linearly() {
        assert_eq!(parry_window_ms(0), 100);
        assert_eq!(parry_window_ms(50), 175);
        assert_eq!(parry_window_ms(100), 250);
    }

    #[test]
    fn parry_qi_cost_has_no_realm_gate() {
        for realm in all_realms() {
            assert_eq!(parry_qi_cost_for_realm(realm), Some(PARRY_QI_COST));
        }
    }

    #[test]
    fn neutralize_profile_realm_table_matches_plan() {
        assert_eq!(
            neutralize_profile(Realm::Awaken, 0).qi_per_contam_percent,
            18.0
        );
        assert_eq!(neutralize_profile(Realm::Induce, 0).max_percent, 2.0);
        assert_eq!(neutralize_profile(Realm::Condense, 0).max_percent, 4.0);
        assert_eq!(
            neutralize_profile(Realm::Solidify, 0).qi_per_contam_percent,
            12.0
        );
        assert_eq!(neutralize_profile(Realm::Spirit, 0).max_percent, 10.0);
        assert_eq!(
            neutralize_profile(Realm::Void, 0).qi_per_contam_percent,
            8.0
        );
    }

    #[test]
    fn multipoint_profile_realm_table_matches_plan() {
        assert_eq!(multipoint_profile(Realm::Awaken, 0).points, 3);
        assert_eq!(multipoint_profile(Realm::Induce, 0).points, 4);
        assert_eq!(multipoint_profile(Realm::Condense, 0).points, 5);
        assert_eq!(multipoint_profile(Realm::Solidify, 0).points, 6);
        assert_eq!(multipoint_profile(Realm::Spirit, 0).points, 7);
        assert_eq!(multipoint_profile(Realm::Void, 0).points, 8);
    }

    #[test]
    fn harden_profile_void_allows_two_meridians() {
        let profile = harden_profile(Realm::Void, 0);
        assert_eq!(profile.max_meridians, 2);
        assert_eq!(profile.damage_multiplier, 0.20);
    }

    #[test]
    fn sever_chain_only_spirit_and_void_gain_amplification() {
        assert!(!sever_chain_profile(Realm::Awaken).grants_amplification);
        assert!(sever_chain_profile(Realm::Spirit).grants_amplification);
        assert!(sever_chain_profile(Realm::Void).grants_amplification);
    }

    #[test]
    fn sever_chain_void_breaks_normal_drain_clamp() {
        let profile = sever_chain_profile(Realm::Void);
        assert_eq!(profile.k_drain, 1.5);
        assert!(profile.k_drain > NORMAL_DRAIN_CLAMP);
    }

    #[test]
    fn style_weight_matrix_matches_zhenmai_axis() {
        assert_eq!(style_weight(ZhenmaiAttackKind::RealYuan), 0.5);
        assert_eq!(style_weight(ZhenmaiAttackKind::PhysicalCarrier), 0.7);
        assert_eq!(style_weight(ZhenmaiAttackKind::Array), 0.2);
        assert_eq!(style_weight(ZhenmaiAttackKind::TaintedYuan), 0.0);
    }

    #[test]
    fn tainted_yuan_reflection_is_zero_without_immunity() {
        assert_eq!(
            reflected_qi(100.0, 1.5, ZhenmaiAttackKind::TaintedYuan),
            0.0
        );
    }

    #[test]
    fn reflected_qi_uses_beta_and_weight() {
        assert!((reflected_qi(100.0, 0.5, ZhenmaiAttackKind::PhysicalCarrier) - 21.0).abs() < 1e-6);
    }

    #[test]
    fn backfire_transfer_uses_collision_reason() {
        let transfer = backfire_transfer(
            QiAccountId::player("attacker"),
            QiAccountId::player("defender"),
            12.0,
        )
        .unwrap();
        assert_eq!(transfer.amount, 12.0);
        assert_eq!(transfer.reason, QiTransferReason::Collision);
    }

    #[test]
    fn attack_kind_maps_qi_needle_to_tainted_yuan() {
        assert_eq!(
            attack_kind_for_source(AttackSource::QiNeedle, WoundKind::Pierce),
            ZhenmaiAttackKind::TaintedYuan
        );
    }

    #[test]
    fn attack_kind_maps_piercing_melee_to_physical_carrier() {
        assert_eq!(
            attack_kind_for_source(AttackSource::Melee, WoundKind::Pierce),
            ZhenmaiAttackKind::PhysicalCarrier
        );
    }

    #[test]
    fn multipoint_contact_increments_count_and_reflects() {
        let mut active = MultiPointActive {
            started_at_tick: 1,
            expires_at_tick: 10,
            points: 5,
            k_drain: 0.2,
            qi_per_second: 1.0,
            contact_count: 0,
            self_damage_per_contact: 1.0,
        };
        let reflected = multipoint_contact(&mut active, 50.0, ZhenmaiAttackKind::RealYuan);
        assert_eq!(active.contact_count, 1);
        assert!((reflected - 3.0).abs() < 1e-6);
    }

    #[test]
    fn amplification_active_requires_kind_and_tick() {
        let active = BackfireAmplification {
            meridian_id: MeridianId::Lung,
            attack_kind: ZhenmaiAttackKind::Array,
            started_at_tick: 10,
            expires_at_tick: 30,
            k_drain: 1.5,
            incoming_damage_multiplier: 0.5,
        };
        assert!(active.active_for(ZhenmaiAttackKind::Array, 29));
        assert!(!active.active_for(ZhenmaiAttackKind::Array, 30));
        assert!(!active.active_for(ZhenmaiAttackKind::RealYuan, 20));
    }

    #[test]
    fn apply_reflected_qi_drains_attacker_pool() {
        let mut app = app_with_events();
        let entity = caster(&mut app, Realm::Void, 100.0);
        let drained = apply_reflected_qi(app.world_mut(), entity, 30.0);
        assert_eq!(drained, 30.0);
        assert_eq!(
            app.world().get::<Cultivation>(entity).unwrap().qi_current,
            70.0
        );
    }

    #[test]
    fn apply_reflected_qi_clamps_at_zero() {
        let mut app = app_with_events();
        let entity = caster(&mut app, Realm::Void, 12.0);
        let drained = apply_reflected_qi(app.world_mut(), entity, 30.0);
        assert_eq!(drained, 12.0);
        assert_eq!(
            app.world().get::<Cultivation>(entity).unwrap().qi_current,
            0.0
        );
    }

    #[test]
    fn apply_self_damage_clamps_health() {
        let mut wounds = Wounds {
            health_current: 5.0,
            ..Default::default()
        };
        let applied = apply_self_damage(&mut wounds, 8.0);
        assert_eq!(applied, 5.0);
        assert_eq!(wounds.health_current, 0.0);
    }

    #[test]
    fn apply_self_damage_to_entity_skips_creative_mode() {
        let mut app = app_with_events();
        let entity = caster(&mut app, Realm::Void, 100.0);
        app.world_mut().entity_mut(entity).insert((
            GameMode::Creative,
            Wounds {
                health_current: 12.0,
                ..Default::default()
            },
        ));

        let applied = apply_self_damage_to_entity(app.world_mut(), entity, 8.0);

        assert_eq!(applied, 0.0);
        assert_eq!(
            app.world().get::<Wounds>(entity).unwrap().health_current,
            12.0
        );
    }

    #[test]
    fn resolve_parry_spends_qi_opens_defense_and_records_xp() {
        let mut app = app_with_events();
        let entity = caster(&mut app, Realm::Induce, 20.0);
        assert!(matches!(
            resolve_parry(app.world_mut(), entity, 0, None),
            CastResult::Started { .. }
        ));
        assert_eq!(
            app.world().get::<Cultivation>(entity).unwrap().qi_current,
            12.0
        );
        assert!(!app
            .world()
            .resource::<Events<crate::combat::events::DefenseIntent>>()
            .is_empty());
        assert!(!app.world().resource::<Events<SkillXpGain>>().is_empty());
    }

    #[test]
    fn resolve_parry_rejects_insufficient_qi() {
        let mut app = app_with_events();
        let entity = caster(&mut app, Realm::Induce, 3.0);
        assert_eq!(
            resolve_parry(app.world_mut(), entity, 0, None),
            CastResult::Rejected {
                reason: CastRejectReason::QiInsufficient
            }
        );
    }

    #[test]
    fn resolve_parry_rejects_declared_severed_meridian() {
        let mut app = app_with_events();
        let entity = caster(&mut app, Realm::Induce, 20.0);
        mark_severed(&mut app, entity, MeridianId::Lung);

        assert_eq!(
            resolve_parry(app.world_mut(), entity, 0, None),
            CastResult::Rejected {
                reason: CastRejectReason::MeridianSevered(Some(MeridianId::Lung))
            }
        );
    }

    #[test]
    fn resolve_neutralize_removes_contam_with_realm_cap() {
        let mut app = app_with_events();
        let entity = caster(&mut app, Realm::Condense, 100.0);
        app.world_mut().entity_mut(entity).insert(Contamination {
            entries: vec![ContamSource {
                amount: 10.0,
                color: ColorKind::Insidious,
                meridian_id: Some(MeridianId::Lung),
                attacker_id: None,
                introduced_at: 1,
            }],
        });
        assert!(matches!(
            resolve_neutralize(app.world_mut(), entity, 0, None),
            CastResult::Started { .. }
        ));
        let contam = app.world().get::<Contamination>(entity).unwrap();
        assert_eq!(contam.entries[0].amount, 6.0);
    }

    #[test]
    fn resolve_neutralize_keeps_other_meridian_contamination() {
        let mut app = app_with_events();
        let entity = caster(&mut app, Realm::Condense, 100.0);
        app.world_mut().entity_mut(entity).insert(Contamination {
            entries: vec![
                ContamSource {
                    amount: 6.0,
                    color: ColorKind::Insidious,
                    meridian_id: Some(MeridianId::Lung),
                    attacker_id: None,
                    introduced_at: 1,
                },
                ContamSource {
                    amount: 5.0,
                    color: ColorKind::Turbid,
                    meridian_id: Some(MeridianId::Heart),
                    attacker_id: None,
                    introduced_at: 1,
                },
            ],
        });

        assert!(matches!(
            resolve_neutralize(app.world_mut(), entity, 0, None),
            CastResult::Started { .. }
        ));
        let contam = app.world().get::<Contamination>(entity).unwrap();
        assert!(contam
            .entries
            .iter()
            .any(|entry| entry.meridian_id == Some(MeridianId::Heart) && entry.amount == 5.0));
    }

    #[test]
    fn resolve_multipoint_inserts_active_component() {
        let mut app = app_with_events();
        let entity = caster(&mut app, Realm::Solidify, 100.0);
        assert!(matches!(
            resolve_multipoint(app.world_mut(), entity, 0, None),
            CastResult::Started { .. }
        ));
        let active = app.world().get::<MultiPointActive>(entity).unwrap();
        assert_eq!(active.points, 6);
    }

    #[test]
    fn resolve_multipoint_rejects_declared_severed_meridian() {
        let mut app = app_with_events();
        let entity = caster(&mut app, Realm::Solidify, 100.0);
        mark_severed(&mut app, entity, MeridianId::Lung);

        assert_eq!(
            resolve_multipoint(app.world_mut(), entity, 0, None),
            CastResult::Rejected {
                reason: CastRejectReason::MeridianSevered(Some(MeridianId::Lung))
            }
        );
    }

    #[test]
    fn resolve_harden_inserts_selected_meridian_component() {
        let mut app = app_with_events();
        let entity = caster(&mut app, Realm::Void, 100.0);
        assert!(matches!(
            resolve_harden(app.world_mut(), entity, 0, None),
            CastResult::Started { .. }
        ));
        let active = app.world().get::<MeridianHardenActive>(entity).unwrap();
        assert_eq!(active.meridians.len(), 2);
    }

    #[test]
    fn resolve_sever_chain_writes_permanent_severed_and_amplification() {
        let mut app = app_with_events();
        let entity = caster(&mut app, Realm::Void, 200.0);
        configure_sever_chain(
            &mut app,
            entity,
            MeridianId::Du,
            ZhenmaiAttackKind::PhysicalCarrier,
        );
        assert_eq!(
            resolve_sever_chain(app.world_mut(), entity, 0, None),
            CastResult::Started {
                cooldown_ticks: SEVER_CHAIN_COOLDOWN_TICKS,
                anim_duration_ticks: 8
            }
        );
        assert!(app
            .world()
            .get::<MeridianSeveredPermanent>(entity)
            .unwrap()
            .is_severed(MeridianId::Du));
        assert!(app.world().get::<BackfireAmplification>(entity).is_some());
    }

    #[test]
    fn resolve_sever_chain_below_spirit_still_severs_without_amplification() {
        let mut app = app_with_events();
        let entity = caster(&mut app, Realm::Condense, 50.0);
        configure_sever_chain(
            &mut app,
            entity,
            MeridianId::Ren,
            ZhenmaiAttackKind::RealYuan,
        );
        assert!(matches!(
            resolve_sever_chain(app.world_mut(), entity, 0, None),
            CastResult::Started { .. }
        ));
        assert_eq!(
            app.world().get::<Cultivation>(entity).unwrap().qi_current,
            0.0
        );
        assert!(app.world().get::<BackfireAmplification>(entity).is_none());
    }

    #[test]
    fn resolve_sever_chain_below_spirit_still_requires_qi_cost() {
        let mut app = app_with_events();
        let entity = caster(&mut app, Realm::Condense, 49.0);
        configure_sever_chain(
            &mut app,
            entity,
            MeridianId::Ren,
            ZhenmaiAttackKind::RealYuan,
        );

        assert_eq!(
            resolve_sever_chain(app.world_mut(), entity, 0, None),
            CastResult::Rejected {
                reason: CastRejectReason::QiInsufficient
            }
        );
        assert!(!app
            .world()
            .get::<MeridianSeveredPermanent>(entity)
            .unwrap()
            .is_severed(MeridianId::Ren));
    }
}
