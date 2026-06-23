use valence::entity::Look;
use valence::prelude::{bevy_ecs, DVec3, Entity, Event, Events, Position, UniqueId};

use crate::combat::components::{CastSource, Casting, SkillBarBindings, WoundKind};
use crate::combat::events::{
    ApplyStatusEffectIntent, AttackIntent, AttackReach, AttackSource, StatusEffectKind, FIST_REACH,
};
use crate::combat::CombatClock;
use crate::cultivation::color::{record_style_practice, PracticeLog};
use crate::cultivation::components::{
    ColorKind, Cultivation, MeridianId, MeridianSystem, QiColor, Realm,
};
use crate::cultivation::known_techniques::technique_definition;
use crate::cultivation::meridian::severed::{
    check_meridian_runtime_integrity, MeridianSeveredPermanent,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::network::audio_event_emit::{
    AudioRecipient, PlaySoundRecipeRequest, AUDIO_BROADCAST_RADIUS,
};
use crate::network::cast_emit::current_unix_millis;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::qi_physics::constants::{QI_EPSILON, QI_ZONE_UNIT_CAPACITY};
use crate::qi_physics::{
    qi_release_to_zone, MediumKind, QiAccountId, QiTransfer, QiTransferReason, StyleAttack,
};
use crate::schema::server_data::{BurstMeridianEventV1, ServerDataPayloadV1};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

const BENG_QUAN_ANIM_ID: &str = "bong:beng_quan";
const BENG_QUAN_PARTICLE_ID: &str = "bong:burst_meridian_beng_quan";

pub const BENG_QUAN_SKILL_ID: &str = "burst_meridian.beng_quan";
pub const BENG_QUAN_EVENT_SKILL: &str = "beng_quan";
pub const BENG_QUAN_QI_COST_RATIO: f64 = 0.4;
pub const BENG_QUAN_OVERLOAD_RATIO: f64 = 1.5;
pub const BENG_QUAN_INTEGRITY_MULTIPLIER: f64 = 0.7;
pub const BENG_QUAN_COOLDOWN_TICKS: u64 = 60;
pub const BENG_QUAN_ANIM_DURATION_TICKS: u32 = 8;

// ─── 贴山靠（tie_shan_kao）─ 沉肩压步，躯干经脉短爆撞开近身敌 ───────────────────────
pub const TIE_SHAN_KAO_SKILL_ID: &str = "burst_meridian.tie_shan_kao";
pub const TIE_SHAN_KAO_EVENT_SKILL: &str = "tie_shan_kao";
/// 撞身打击的过载击退倍率（沉肩肩撞，比崩拳更重的躯干爆发）。
pub const TIE_SHAN_KAO_OVERLOAD_RATIO: f64 = 1.6;
/// 撕裂躯干经脉（Stomach）后的 integrity 残留比例 —— 比崩拳略缓（短爆而非零距灌入）。
pub const TIE_SHAN_KAO_INTEGRITY_MULTIPLIER: f64 = 0.8;
const TIE_SHAN_KAO_MERIDIANS: [MeridianId; 1] = [MeridianId::Stomach];
const TIE_SHAN_KAO_ANIM_ID: &str = "bong:beng_quan";
const TIE_SHAN_KAO_PARTICLE_ID: &str = "bong:burst_meridian_beng_quan";
const TIE_SHAN_KAO_AUDIO_RECIPE: &str = "hit_heavy";
/// 与 known_techniques.tie_shan_kao.range 对齐（近身撞）。
const TIE_SHAN_KAO_REACH: AttackReach = AttackReach::new(1.0, 0.5);

// ─── 血崩步（xue_beng_bu）─ 腿经裂响换短距突进，抢入战圈 ──────────────────────────
pub const XUE_BENG_BU_SKILL_ID: &str = "burst_meridian.xue_beng_bu";
pub const XUE_BENG_BU_EVENT_SKILL: &str = "xue_beng_bu";
/// 撕裂腿经（GallBladder）后的 integrity 残留比例。
pub const XUE_BENG_BU_INTEGRITY_MULTIPLIER: f64 = 0.75;
/// 突进距离（格），与 known_techniques.xue_beng_bu.range 对齐。
pub const XUE_BENG_BU_DASH_BLOCKS: f64 = 4.0;
const XUE_BENG_BU_MERIDIANS: [MeridianId; 1] = [MeridianId::Gallbladder];
const XUE_BENG_BU_ANIM_ID: &str = "bong:beng_quan";
const XUE_BENG_BU_PARTICLE_ID: &str = "bong:burst_meridian_beng_quan";
const XUE_BENG_BU_AUDIO_RECIPE: &str = "movement_dash";

// ─── 逆脉护体（ni_mai_hu_ti）─ 逆转真元护要害，短时压住外伤冲击 ────────────────────
pub const NI_MAI_HU_TI_SKILL_ID: &str = "burst_meridian.ni_mai_hu_ti";
pub const NI_MAI_HU_TI_EVENT_SKILL: &str = "ni_mai_hu_ti";
/// 逆转真元护脉（Pericardium）后的 integrity 残留比例 —— 逆行最伤本脉，残留最低。
pub const NI_MAI_HU_TI_INTEGRITY_MULTIPLIER: f64 = 0.7;
/// 护体减伤幅度（受击伤害 ×(1-N)），玄阶短时强减伤但非无敌。
pub const NI_MAI_HU_TI_DAMAGE_REDUCTION: f32 = 0.35;
/// 护体持续时间（tick）—— 与 known_techniques 的 cooldown 120 形成短窗高冷却节奏。
pub const NI_MAI_HU_TI_BUFF_DURATION_TICKS: u64 = 60;
const NI_MAI_HU_TI_MERIDIANS: [MeridianId; 1] = [MeridianId::Pericardium];
const NI_MAI_HU_TI_PARTICLE_ID: &str = "bong:burst_meridian_beng_quan";
const NI_MAI_HU_TI_AUDIO_RECIPE: &str = "zhenmai_shield_hum";

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BengQuanStyleAttack {
    pub qi_invest: f64,
    pub integrity_snapshot: f64,
}

impl StyleAttack for BengQuanStyleAttack {
    fn style_color(&self) -> ColorKind {
        ColorKind::Heavy
    }

    fn injected_qi(&self) -> f64 {
        self.qi_invest.max(0.0)
    }

    fn purity(&self) -> f64 {
        self.integrity_snapshot.clamp(0.0, 1.0)
    }

    fn rejection_rate(&self) -> f64 {
        0.65
    }

    fn medium(&self) -> MediumKind {
        MediumKind::bare(ColorKind::Heavy)
    }
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
    registry.register(TIE_SHAN_KAO_SKILL_ID, resolve_tie_shan_kao);
    registry.register(XUE_BENG_BU_SKILL_ID, resolve_xue_beng_bu);
    registry.register(NI_MAI_HU_TI_SKILL_ID, resolve_ni_mai_hu_ti);
}

pub fn declare_meridian_dependencies(
    dependencies: &mut crate::cultivation::meridian::severed::SkillMeridianDependencies,
) {
    dependencies.declare(BENG_QUAN_SKILL_ID, RIGHT_ARM_MERIDIANS.to_vec());
    dependencies.declare(TIE_SHAN_KAO_SKILL_ID, TIE_SHAN_KAO_MERIDIANS.to_vec());
    dependencies.declare(XUE_BENG_BU_SKILL_ID, XUE_BENG_BU_MERIDIANS.to_vec());
    dependencies.declare(NI_MAI_HU_TI_SKILL_ID, NI_MAI_HU_TI_MERIDIANS.to_vec());
}

/// 从 known_techniques 读招式的 flat qi_cost（单一真值源，严禁在本文件硬编码重复）。
fn flat_qi_cost(skill_id: &str) -> Option<f64> {
    technique_definition(skill_id).map(|def| f64::from(def.qi_cost))
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
        return rejected(CastRejectReason::MERIDIAN_SEVERED);
    };
    let integrity_snapshot = right_arm_integrity_snapshot(meridians);
    let severed = world.get::<MeridianSeveredPermanent>(caster);
    if let Err(blocking) =
        check_meridian_runtime_integrity(&RIGHT_ARM_MERIDIANS, meridians, severed)
    {
        return rejected(CastRejectReason::MeridianSevered(Some(blocking)));
    }

    let started_at_ms = current_unix_millis();
    world.entity_mut(caster).insert(Casting {
        source: CastSource::SkillBar,
        slot,
        started_at_tick: now_tick,
        duration_ticks: u64::from(BENG_QUAN_ANIM_DURATION_TICKS),
        started_at_ms,
        duration_ms: BENG_QUAN_ANIM_DURATION_TICKS
            .saturating_mul(crate::time::MILLIS_PER_TICK as u32),
        bound_instance_id: None,
        start_position: caster_position,
        complete_cooldown_ticks: BENG_QUAN_COOLDOWN_TICKS,
        skill_id: Some(BENG_QUAN_SKILL_ID.to_string()),
        skill_config: None,
    });

    // bughunt r3 — beng_quan 此前内联扣 qi_current 却不归还 zone（绕过 spend_qi）→ 真元蒸发、守恒破。
    // 改走 spend_qi（与铁山靠/血崩步/逆脉护体同一路径）：扣费的同时把消耗回灌到 caster 所在 zone。
    spend_qi(world, caster, cost);
    if let Some(mut meridians) = world.get_mut::<MeridianSystem>(caster) {
        for id in RIGHT_ARM_MERIDIANS {
            let meridian = meridians.get_mut(id);
            meridian.integrity =
                (meridian.integrity * BENG_QUAN_INTEGRITY_MULTIPLIER).clamp(0.0, 1.0);
        }
    }
    let qi_color = world.get::<QiColor>(caster).cloned();
    if let Some(mut practice_log) = world.get_mut::<PracticeLog>(caster) {
        record_style_practice(&mut practice_log, ColorKind::Heavy, qi_color.as_ref());
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

// ─── 贴山靠（tie_shan_kao）resolver ──────────────────────────────────────────────
//
// 与崩拳同模式：近身命中目标 → 撕裂单条躯干经脉（Stomach）+ 扣 flat qi_cost(35)
// → 发 AttackIntent（BurstMeridian 来源自带 ×2.5 击退）+ BurstMeridianEvent（复用现有
// proto 桥）+ AV。差异：肩撞而非短拳，所以 reach 更短、过载倍率略高、撕脉残留较缓。
pub fn resolve_tie_shan_kao(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    let Some(target) = target else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let Some(now_tick) = combat_now_tick(world) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if is_slot_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    let Some(caster_position) = world.get::<Position>(caster).map(|p| p.get()) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let Some(target_position) = world.get::<Position>(target).map(|p| p.get()) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if caster_position.distance(target_position) > f64::from(TIE_SHAN_KAO_REACH.max) + f64::EPSILON
    {
        return rejected(CastRejectReason::InvalidTarget);
    }

    if let Some(reason) = check_realm_gate(world, caster, Realm::Condense) {
        return rejected(reason);
    }
    let Some(cost) = flat_qi_cost(TIE_SHAN_KAO_SKILL_ID) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if let Some(reason) = check_qi_gate(world, caster, cost) {
        return rejected(reason);
    }
    let Some(integrity_snapshot) =
        check_single_meridian_gate(world, caster, TIE_SHAN_KAO_MERIDIANS[0])
    else {
        return rejected(CastRejectReason::MeridianSevered(Some(
            TIE_SHAN_KAO_MERIDIANS[0],
        )));
    };

    let definition = technique_definition(TIE_SHAN_KAO_SKILL_ID);
    let (cast_ticks, cooldown_ticks) = cast_timing(definition, 10, 70);
    insert_casting(
        world,
        caster,
        slot,
        now_tick,
        cast_ticks,
        cooldown_ticks,
        caster_position,
        TIE_SHAN_KAO_SKILL_ID,
    );

    // ── 守恒：扣 flat qi + 撕裂躯干经脉（系列代价）───────────────────────────────
    spend_qi(world, caster, cost);
    tear_meridian(
        world,
        caster,
        TIE_SHAN_KAO_MERIDIANS[0],
        TIE_SHAN_KAO_INTEGRITY_MULTIPLIER,
    );
    record_heavy_practice(world, caster);

    world.send_event(AttackIntent {
        attacker: caster,
        target: Some(target),
        issued_at_tick: now_tick,
        reach: TIE_SHAN_KAO_REACH,
        qi_invest: (cost * TIE_SHAN_KAO_OVERLOAD_RATIO) as f32,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::BurstMeridian,
        debug_command: None,
    });
    world.send_event(BurstMeridianEvent {
        skill: TIE_SHAN_KAO_EVENT_SKILL,
        caster,
        target: Some(target),
        tick: now_tick,
        overload_ratio: TIE_SHAN_KAO_OVERLOAD_RATIO,
        integrity_snapshot,
    });
    emit_burst_av(
        world,
        caster,
        caster_position,
        Some(target_position - caster_position),
        BurstAv {
            anim_id: Some(TIE_SHAN_KAO_ANIM_ID),
            particle_id: TIE_SHAN_KAO_PARTICLE_ID,
            color: "#B8763A",
            strength: 0.95,
            count: 10,
            duration_ticks: cast_ticks as u16,
            audio_recipe: TIE_SHAN_KAO_AUDIO_RECIPE,
        },
    );

    CastResult::Started {
        cooldown_ticks,
        anim_duration_ticks: cast_ticks,
    }
}

// ─── 血崩步（xue_beng_bu）resolver ───────────────────────────────────────────────
//
// 爆发位移突进：扣 flat qi_cost(25) + 撕裂腿经（GallBladder）→ 沿朝向把自身 Position
// 前推 dash 距离（4 格，服务器权威，valence 自动下发客户端）。无攻击判定。
pub fn resolve_xue_beng_bu(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let Some(now_tick) = combat_now_tick(world) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if is_slot_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    let Some(caster_position) = world.get::<Position>(caster).map(|p| p.get()) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    // 朝向缺失（无 Look）→ 无法决定突进方向，拒绝（不凭空位移）。
    let Some(facing) = world
        .get::<Look>(caster)
        .and_then(horizontal_facing_from_look)
    else {
        return rejected(CastRejectReason::InvalidTarget);
    };

    if let Some(reason) = check_realm_gate(world, caster, Realm::Condense) {
        return rejected(reason);
    }
    let Some(cost) = flat_qi_cost(XUE_BENG_BU_SKILL_ID) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if let Some(reason) = check_qi_gate(world, caster, cost) {
        return rejected(reason);
    }
    let Some(integrity_snapshot) =
        check_single_meridian_gate(world, caster, XUE_BENG_BU_MERIDIANS[0])
    else {
        return rejected(CastRejectReason::MeridianSevered(Some(
            XUE_BENG_BU_MERIDIANS[0],
        )));
    };

    let definition = technique_definition(XUE_BENG_BU_SKILL_ID);
    let (cast_ticks, cooldown_ticks) = cast_timing(definition, 6, 50);
    insert_casting(
        world,
        caster,
        slot,
        now_tick,
        cast_ticks,
        cooldown_ticks,
        caster_position,
        XUE_BENG_BU_SKILL_ID,
    );

    // ── 守恒：扣 flat qi + 撕裂腿经（系列代价）+ 服务器权威位移 ──────────────────
    spend_qi(world, caster, cost);
    tear_meridian(
        world,
        caster,
        XUE_BENG_BU_MERIDIANS[0],
        XUE_BENG_BU_INTEGRITY_MULTIPLIER,
    );
    record_heavy_practice(world, caster);
    let dash_target = caster_position + facing * XUE_BENG_BU_DASH_BLOCKS;
    if let Some(mut position) = world.get_mut::<Position>(caster) {
        position.set(dash_target);
    }

    world.send_event(BurstMeridianEvent {
        skill: XUE_BENG_BU_EVENT_SKILL,
        caster,
        target: None,
        tick: now_tick,
        // 位移招无攻击过载，记 1.0 表"无放大"，integrity_snapshot 记腿经损前快照。
        overload_ratio: 1.0,
        integrity_snapshot,
    });
    emit_burst_av(
        world,
        caster,
        caster_position,
        Some(facing),
        BurstAv {
            anim_id: Some(XUE_BENG_BU_ANIM_ID),
            particle_id: XUE_BENG_BU_PARTICLE_ID,
            color: "#A23A3A",
            strength: 0.85,
            count: 12,
            duration_ticks: cast_ticks as u16,
            audio_recipe: XUE_BENG_BU_AUDIO_RECIPE,
        },
    );

    CastResult::Started {
        cooldown_ticks,
        anim_duration_ticks: cast_ticks,
    }
}

// ─── 逆脉护体（ni_mai_hu_ti）resolver ────────────────────────────────────────────
//
// 短时减伤护体 buff：扣 flat qi_cost(45) + 逆转真元最伤本脉（Pericardium）→ 发
// ApplyStatusEffectIntent(DamageReduction, magnitude 0.35) 给自身。自身招，无需 target。
pub fn resolve_ni_mai_hu_ti(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let Some(now_tick) = combat_now_tick(world) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if is_slot_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    let Some(caster_position) = world.get::<Position>(caster).map(|p| p.get()) else {
        return rejected(CastRejectReason::InvalidTarget);
    };

    if let Some(reason) = check_realm_gate(world, caster, Realm::Solidify) {
        return rejected(reason);
    }
    let Some(cost) = flat_qi_cost(NI_MAI_HU_TI_SKILL_ID) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if let Some(reason) = check_qi_gate(world, caster, cost) {
        return rejected(reason);
    }
    let Some(integrity_snapshot) =
        check_single_meridian_gate(world, caster, NI_MAI_HU_TI_MERIDIANS[0])
    else {
        return rejected(CastRejectReason::MeridianSevered(Some(
            NI_MAI_HU_TI_MERIDIANS[0],
        )));
    };

    let definition = technique_definition(NI_MAI_HU_TI_SKILL_ID);
    let (cast_ticks, cooldown_ticks) = cast_timing(definition, 12, 120);
    insert_casting(
        world,
        caster,
        slot,
        now_tick,
        cast_ticks,
        cooldown_ticks,
        caster_position,
        NI_MAI_HU_TI_SKILL_ID,
    );

    // ── 守恒：扣 flat qi + 逆转真元撕本脉（系列代价）+ 短时减伤 buff ──────────────
    spend_qi(world, caster, cost);
    tear_meridian(
        world,
        caster,
        NI_MAI_HU_TI_MERIDIANS[0],
        NI_MAI_HU_TI_INTEGRITY_MULTIPLIER,
    );
    record_heavy_practice(world, caster);
    world.send_event(ApplyStatusEffectIntent {
        target: caster,
        kind: StatusEffectKind::DamageReduction,
        magnitude: NI_MAI_HU_TI_DAMAGE_REDUCTION,
        duration_ticks: NI_MAI_HU_TI_BUFF_DURATION_TICKS,
        issued_at_tick: now_tick,
    });

    world.send_event(BurstMeridianEvent {
        skill: NI_MAI_HU_TI_EVENT_SKILL,
        caster,
        target: None,
        tick: now_tick,
        overload_ratio: 1.0,
        integrity_snapshot,
    });
    emit_burst_av(
        world,
        caster,
        caster_position,
        None,
        BurstAv {
            // 护体非攻击姿态，不复用崩拳出拳 anim（避免姿态语义错位）；仅护体粒子环 + 嗡音。
            anim_id: None,
            particle_id: NI_MAI_HU_TI_PARTICLE_ID,
            color: "#5BA8C9",
            strength: 0.7,
            count: 14,
            duration_ticks: NI_MAI_HU_TI_BUFF_DURATION_TICKS.min(u16::MAX as u64) as u16,
            audio_recipe: NI_MAI_HU_TI_AUDIO_RECIPE,
        },
    );

    CastResult::Started {
        cooldown_ticks,
        anim_duration_ticks: cast_ticks,
    }
}

// ─── 共享 cast 辅助（与崩拳路径同语义，抽出供 3 招复用）─────────────────────────────

fn combat_now_tick(world: &bevy_ecs::world::World) -> Option<u64> {
    world.get_resource::<CombatClock>().map(|clock| clock.tick)
}

fn is_slot_on_cooldown(
    world: &bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    now_tick: u64,
) -> bool {
    world
        .get::<SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
}

/// 境界门：低于要求境界返回 `Some(RealmTooLow)`，缺 Cultivation 也视作不达标。
fn check_realm_gate(
    world: &bevy_ecs::world::World,
    caster: Entity,
    required: Realm,
) -> Option<CastRejectReason> {
    match world.get::<Cultivation>(caster) {
        Some(cultivation) if realm_rank(cultivation.realm) >= realm_rank(required) => None,
        _ => Some(CastRejectReason::RealmTooLow),
    }
}

/// 真元门：flat cost。`cost <= ε`（误配 0 成本招）或 `qi_current < cost` 均拒绝，
/// 不在此处扣减（扣减由 `spend_qi` 在所有门通过后执行，保证拒绝路径零突变）。
fn check_qi_gate(
    world: &bevy_ecs::world::World,
    caster: Entity,
    cost: f64,
) -> Option<CastRejectReason> {
    let Some(cultivation) = world.get::<Cultivation>(caster) else {
        return Some(CastRejectReason::RealmTooLow);
    };
    if cost <= f64::EPSILON || cultivation.qi_current + f64::EPSILON < cost {
        return Some(CastRejectReason::QiInsufficient);
    }
    None
}

/// 单经脉门：检查永久 SEVERED + 当前 integrity 可用。通过返回该经脉损前 integrity 快照
/// （供 BurstMeridianEvent 记账）；不通过返回 None（调用方映射 MeridianSevered）。
fn check_single_meridian_gate(
    world: &bevy_ecs::world::World,
    caster: Entity,
    meridian: MeridianId,
) -> Option<f64> {
    let meridians = world.get::<MeridianSystem>(caster)?;
    let snapshot = meridians.get(meridian).integrity.clamp(0.0, 1.0);
    let severed = world.get::<MeridianSeveredPermanent>(caster);
    check_meridian_runtime_integrity(&[meridian], meridians, severed)
        .ok()
        .map(|_| snapshot)
}

#[allow(clippy::too_many_arguments)]
fn insert_casting(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    now_tick: u64,
    cast_ticks: u32,
    cooldown_ticks: u64,
    caster_position: DVec3,
    skill_id: &str,
) {
    let started_at_ms = current_unix_millis();
    world.entity_mut(caster).insert(Casting {
        source: CastSource::SkillBar,
        slot,
        started_at_tick: now_tick,
        duration_ticks: u64::from(cast_ticks),
        started_at_ms,
        duration_ms: cast_ticks.saturating_mul(crate::time::MILLIS_PER_TICK as u32),
        bound_instance_id: None,
        start_position: caster_position,
        complete_cooldown_ticks: cooldown_ticks,
        skill_id: Some(skill_id.to_string()),
        skill_config: None,
    });
}

/// 从 known_techniques 读 cast/cooldown，缺定义时退化为传入兜底（保证 ≥1）。
fn cast_timing(
    definition: Option<&'static crate::cultivation::known_techniques::TechniqueDefinition>,
    fallback_cast: u32,
    fallback_cooldown: u64,
) -> (u32, u64) {
    match definition {
        Some(def) => (def.cast_ticks.max(1), u64::from(def.cooldown_ticks).max(1)),
        None => (fallback_cast.max(1), fallback_cooldown.max(1)),
    }
}

/// 扣真元（玩家私有池）并将消耗的真元释放回区域灵气池，维持 qi_physics 守恒。
/// 与 baomai_v3 / tuike_v2 的 spend_qi + emit_spent_qi_release 模式对齐。
fn spend_qi(world: &mut bevy_ecs::world::World, caster: Entity, cost: f64) {
    if cost <= f64::EPSILON {
        return;
    }
    if !cost.is_finite() {
        return;
    }
    if let Some(mut cultivation) = world.get_mut::<Cultivation>(caster) {
        cultivation.qi_current = (cultivation.qi_current - cost).clamp(0.0, cultivation.qi_max);
    }
    emit_spent_qi_release(world, caster, cost, "burst_meridian:spend_qi");
}

/// 把消耗的真元释放回 ZoneRegistry，走 qi_release_to_zone 守恒路径；区域满时路由至 overflow。
fn emit_spent_qi_release(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    amount: f64,
    sink: &'static str,
) {
    if amount <= QI_EPSILON {
        return;
    }
    let from = QiAccountId::player(format!("entity:{}", caster.to_bits()));
    let position = world.get::<Position>(caster).map(|p| p.get());
    let dimension = world
        .get::<CurrentDimension>(caster)
        .map(|d| d.0)
        .unwrap_or(DimensionKind::Overworld);

    let mut transfers = Vec::new();
    if let (Some(position), Some(mut zones)) = (position, world.get_resource_mut::<ZoneRegistry>())
    {
        let zone_name = zones
            .find_zone(dimension, position)
            .map(|zone| zone.name.clone());
        if let Some(zone_name) = zone_name {
            if let Some(zone) = zones.find_zone_mut(zone_name.as_str()) {
                let to = QiAccountId::zone(zone.name.clone());
                let zone_current = zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY;
                match qi_release_to_zone(
                    amount,
                    from.clone(),
                    to,
                    zone_current,
                    QI_ZONE_UNIT_CAPACITY,
                ) {
                    Ok(outcome) => {
                        zone.spirit_qi =
                            (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
                        if let Some(transfer) = outcome.transfer {
                            transfers.push(transfer);
                        }
                        if outcome.overflow > QI_EPSILON {
                            push_spent_qi_overflow(
                                &mut transfers,
                                from.clone(),
                                outcome.overflow,
                                sink,
                                caster,
                            );
                        }
                    }
                    Err(error) => {
                        tracing::warn!(
                            ?error,
                            "[bong][burst_meridian] invalid spent qi release for {:?}; route to overflow",
                            caster
                        );
                        push_spent_qi_overflow(&mut transfers, from.clone(), amount, sink, caster);
                    }
                }
            } else {
                push_spent_qi_overflow(&mut transfers, from.clone(), amount, sink, caster);
            }
        } else {
            push_spent_qi_overflow(&mut transfers, from.clone(), amount, sink, caster);
        }
    } else {
        push_spent_qi_overflow(&mut transfers, from.clone(), amount, sink, caster);
    }

    for transfer in transfers {
        if let Some(mut events) = world.get_resource_mut::<Events<QiTransfer>>() {
            events.send(transfer);
        }
    }
}

fn push_spent_qi_overflow(
    transfers: &mut Vec<QiTransfer>,
    from: QiAccountId,
    amount: f64,
    sink: &'static str,
    caster: Entity,
) {
    if amount <= QI_EPSILON {
        return;
    }
    match QiTransfer::new(
        from,
        QiAccountId::overflow(format!("{sink}:{}", caster.to_bits())),
        amount,
        QiTransferReason::ReleaseToZone,
    ) {
        Ok(transfer) => transfers.push(transfer),
        Err(error) => tracing::warn!(
            ?error,
            sink,
            ?caster,
            amount,
            "[bong][burst_meridian] failed to build spent qi overflow transfer"
        ),
    }
}

/// 撕裂单条经脉（系列代价）—— integrity ×= multiplier，clamp 进 [0,1]。
fn tear_meridian(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    meridian: MeridianId,
    multiplier: f64,
) {
    if let Some(mut meridians) = world.get_mut::<MeridianSystem>(caster) {
        let m = meridians.get_mut(meridian);
        m.integrity = (m.integrity * multiplier).clamp(0.0, 1.0);
    }
}

fn record_heavy_practice(world: &mut bevy_ecs::world::World, caster: Entity) {
    let qi_color = world.get::<QiColor>(caster).cloned();
    if let Some(mut practice_log) = world.get_mut::<PracticeLog>(caster) {
        record_style_practice(&mut practice_log, ColorKind::Heavy, qi_color.as_ref());
    }
}

fn horizontal_facing_from_look(look: &Look) -> Option<DVec3> {
    let yaw = f64::from(look.yaw).to_radians();
    let facing = DVec3::new(-yaw.sin(), 0.0, yaw.cos());
    facing.is_finite().then_some(facing)
}

/// AV 描述：纯加法 cosmetic，复用既有 anim/particle/audio recipe（无净新资产）。
struct BurstAv {
    anim_id: Option<&'static str>,
    particle_id: &'static str,
    color: &'static str,
    strength: f32,
    count: u16,
    duration_ticks: u16,
    audio_recipe: &'static str,
}

fn emit_burst_av(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    caster_position: DVec3,
    direction: Option<DVec3>,
    av: BurstAv,
) {
    if let Some(anim_id) = av.anim_id {
        if let Some(unique_id) = world.get::<UniqueId>(caster).copied() {
            world.send_event(VfxEventRequest::new(
                caster_position,
                VfxEventPayloadV1::PlayAnim {
                    target_player: unique_id.0.to_string(),
                    anim_id: anim_id.to_string(),
                    priority: 1500,
                    fade_in_ticks: Some(2),
                },
            ));
        }
    }
    world.send_event(VfxEventRequest::new(
        caster_position,
        VfxEventPayloadV1::SpawnParticle {
            event_id: av.particle_id.to_string(),
            origin: [
                caster_position.x,
                caster_position.y + 1.0,
                caster_position.z,
            ],
            direction: direction.map(|d| [d.x, d.y, d.z]),
            color: Some(av.color.to_string()),
            strength: Some(av.strength.clamp(0.0, 1.0)),
            count: Some(av.count),
            duration_ticks: Some(av.duration_ticks),
        },
    ));
    if let Some(mut events) = world.get_resource_mut::<Events<PlaySoundRecipeRequest>>() {
        events.send(PlaySoundRecipeRequest {
            recipe_id: av.audio_recipe.to_string(),
            instance_id: 0,
            pos: None,
            flag: None,
            volume_mul: 1.0,
            pitch_shift: 0.0,
            recipient: AudioRecipient::Radius {
                origin: caster_position,
                radius: AUDIO_BROADCAST_RADIUS,
            },
        });
    }
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

        // 全断时退化为 "声明顺序首条" —— RIGHT_ARM_MERIDIANS[0] = LargeIntestine
        assert_eq!(
            result,
            rejected(CastRejectReason::MeridianSevered(Some(
                MeridianId::LargeIntestine
            )))
        );
        assert_no_mutation(&app, caster, 100.0, 0.0);
    }

    #[test]
    fn beng_quan_rejects_when_severed_component_marks_first_dep() {
        // SEVERED component 标 LargeIntestine → 即使 Meridian.integrity 仍 1.0 也拒绝
        let mut app = app();
        let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
        app.world_mut()
            .entity_mut(caster)
            .insert(MeridianSeveredPermanent::default());
        {
            let mut sev = app
                .world_mut()
                .get_mut::<MeridianSeveredPermanent>(caster)
                .unwrap();
            sev.insert(
                MeridianId::LargeIntestine,
                crate::cultivation::meridian::severed::SeveredSource::CombatWound,
                0,
            );
        }
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

        // check_meridian_runtime_integrity 先查 SEVERED component → 返回首个 SEVERED
        assert_eq!(
            result,
            rejected(CastRejectReason::MeridianSevered(Some(
                MeridianId::LargeIntestine
            )))
        );
        assert_no_mutation(&app, caster, 100.0, 1.0);
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

    #[test]
    fn beng_quan_style_attack_uses_heavy_color_and_integrity_purity() {
        let attack = BengQuanStyleAttack {
            qi_invest: 12.0,
            integrity_snapshot: 0.7,
        };

        assert_eq!(attack.style_color(), ColorKind::Heavy);
        assert_eq!(attack.injected_qi(), 12.0);
        assert_eq!(attack.purity(), 0.7);
        assert_eq!(attack.rejection_rate(), 0.65);
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // 贴山靠 / 血崩步 / 逆脉护体 —— 实装 skeleton 招的饱和测试。
    //   覆盖：① happy path（守恒 + 事件 + AV）② 拒绝分支（realm/qi/cooldown/meridian/
    //   target/facing）③ 守恒（拒绝路径零突变、qi/integrity 精确数值）④ 状态转换
    //   （Casting 插入、Position 位移、DamageReduction buff）。
    // ─────────────────────────────────────────────────────────────────────────────

    use valence::entity::Look;

    /// 注册 3 招会用到的全部 event 资源（含 burst 共用的 ApplyStatusEffectIntent /
    /// PlaySoundRecipeRequest）。
    fn full_app() -> App {
        let mut app = app();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app
    }

    /// 带 Look 的 caster（血崩步突进需要朝向）。yaw=0 → facing = (0,0,1)。
    fn spawn_caster_with_look(
        app: &mut App,
        realm: Realm,
        qi_current: f64,
        position: DVec3,
        yaw: f32,
    ) -> Entity {
        let caster = spawn_caster(app, realm, qi_current, position);
        app.world_mut()
            .entity_mut(caster)
            .insert(Look::new(yaw, 0.0));
        caster
    }

    fn meridian_integrity(app: &App, caster: Entity, id: MeridianId) -> f64 {
        app.world()
            .get::<MeridianSystem>(caster)
            .unwrap()
            .get(id)
            .integrity
    }

    fn qi(app: &App, caster: Entity) -> f64 {
        app.world().get::<Cultivation>(caster).unwrap().qi_current
    }

    fn sever_meridian(app: &mut App, caster: Entity, id: MeridianId) {
        app.world_mut()
            .entity_mut(caster)
            .insert(MeridianSeveredPermanent::default());
        let mut sev = app
            .world_mut()
            .get_mut::<MeridianSeveredPermanent>(caster)
            .unwrap();
        sev.insert(
            id,
            crate::cultivation::meridian::severed::SeveredSource::CombatWound,
            0,
        );
    }

    fn last_burst_skill(app: &App) -> &'static str {
        app.world()
            .resource::<Events<BurstMeridianEvent>>()
            .iter_current_update_events()
            .next()
            .expect("a BurstMeridianEvent should be emitted")
            .skill
    }

    fn emitted_audio_recipe(app: &App) -> Option<String> {
        app.world()
            .resource::<Events<PlaySoundRecipeRequest>>()
            .iter_current_update_events()
            .next()
            .map(|req| req.recipe_id.clone())
    }

    fn assert_single_meridian_untouched(app: &App, caster: Entity, id: MeridianId, qi_val: f64) {
        assert_eq!(qi(app, caster), qi_val, "qi must be untouched on rejection");
        assert_eq!(
            meridian_integrity(app, caster, id),
            1.0,
            "meridian must be untouched on rejection"
        );
        assert!(
            app.world().get::<Casting>(caster).is_none(),
            "no Casting must be inserted on rejection"
        );
        assert!(
            app.world()
                .resource::<Events<BurstMeridianEvent>>()
                .is_empty(),
            "no BurstMeridianEvent on rejection"
        );
    }

    // ─── 贴山靠（tie_shan_kao）─ Condense / Stomach≥0.5 / qi 35 / cd 70 ─────────────

    #[test]
    fn tie_shan_kao_happy_path_spends_qi_tears_stomach_and_strikes() {
        let mut app = full_app();
        let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

        assert_eq!(
            result,
            CastResult::Started {
                cooldown_ticks: 70,
                anim_duration_ticks: 10,
            },
            "cd/cast must come from known_techniques.tie_shan_kao"
        );
        // 守恒：扣 flat 35（非 ratio），Stomach ×0.8。
        assert_eq!(qi(&app, caster), 65.0, "100 - 35 flat qi_cost");
        assert!(
            (meridian_integrity(&app, caster, MeridianId::Stomach) - 0.8).abs() < 1e-12,
            "Stomach integrity torn 1.0 -> 0.8"
        );
        assert_eq!(
            app.world().get::<Casting>(caster).unwrap().duration_ticks,
            10
        );

        // 攻击：BurstMeridian 来源（自带 ×2.5 击退），qi_invest = cost * overload。
        let attack = app
            .world()
            .resource::<Events<AttackIntent>>()
            .iter_current_update_events()
            .next()
            .expect("AttackIntent emitted");
        assert_eq!(attack.target, Some(target));
        assert_eq!(attack.source, AttackSource::BurstMeridian);
        assert_eq!(attack.wound_kind, WoundKind::Blunt);
        assert!(
            (attack.qi_invest - (35.0 * TIE_SHAN_KAO_OVERLOAD_RATIO) as f32).abs() < 1e-3,
            "qi_invest = cost * overload_ratio"
        );

        assert_eq!(last_burst_skill(&app), TIE_SHAN_KAO_EVENT_SKILL);
        assert_eq!(emitted_audio_recipe(&app).as_deref(), Some("hit_heavy"));
    }

    #[test]
    fn tie_shan_kao_rejects_low_realm_without_mutation() {
        let mut app = full_app();
        // Induce < Condense → 拒绝。
        let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

        assert_eq!(result, rejected(CastRejectReason::RealmTooLow));
        assert_single_meridian_untouched(&app, caster, MeridianId::Stomach, 100.0);
    }

    #[test]
    fn tie_shan_kao_rejects_insufficient_qi_without_mutation() {
        let mut app = full_app();
        // 30 < 35 flat cost → QiInsufficient。
        let caster = spawn_caster(&mut app, Realm::Condense, 30.0, DVec3::ZERO);
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

        assert_eq!(result, rejected(CastRejectReason::QiInsufficient));
        assert_single_meridian_untouched(&app, caster, MeridianId::Stomach, 30.0);
    }

    #[test]
    fn tie_shan_kao_rejects_out_of_range_and_missing_target() {
        let mut app = full_app();
        let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
        // 超 reach.max (1.5) → InvalidTarget。
        let far = spawn_target(
            &mut app,
            DVec3::new(f64::from(TIE_SHAN_KAO_REACH.max) + 0.5, 0.0, 0.0),
        );

        assert_eq!(
            resolve_tie_shan_kao(app.world_mut(), caster, 0, None),
            rejected(CastRejectReason::InvalidTarget)
        );
        assert_eq!(
            resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(far)),
            rejected(CastRejectReason::InvalidTarget)
        );
        assert_single_meridian_untouched(&app, caster, MeridianId::Stomach, 100.0);
    }

    #[test]
    fn tie_shan_kao_rejects_when_stomach_severed() {
        let mut app = full_app();
        let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
        sever_meridian(&mut app, caster, MeridianId::Stomach);
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

        assert_eq!(
            result,
            rejected(CastRejectReason::MeridianSevered(Some(MeridianId::Stomach)))
        );
        // SEVERED component 把 Stomach 钳到 0；只断言 qi 未动 + 无副作用。
        assert_eq!(qi(&app, caster), 100.0);
        assert!(app.world().get::<Casting>(caster).is_none());
        assert!(app.world().resource::<Events<AttackIntent>>().is_empty());
    }

    #[test]
    fn tie_shan_kao_rejects_on_cooldown_before_mutation() {
        let mut app = full_app();
        let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
        app.world_mut()
            .get_mut::<SkillBarBindings>(caster)
            .unwrap()
            .set_cooldown(0, 11);
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

        assert_eq!(result, rejected(CastRejectReason::OnCooldown));
        assert_single_meridian_untouched(&app, caster, MeridianId::Stomach, 100.0);
    }

    // ─── 血崩步（xue_beng_bu）─ Condense / Gallbladder≥0.4 / qi 25 / dash 4 ─────────

    #[test]
    fn xue_beng_bu_happy_path_dashes_forward_spends_qi_tears_leg() {
        let mut app = full_app();
        // yaw=0 → facing (0,0,1)；起点原点。
        let caster = spawn_caster_with_look(&mut app, Realm::Condense, 100.0, DVec3::ZERO, 0.0);

        let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            CastResult::Started {
                cooldown_ticks: 50,
                anim_duration_ticks: 6,
            }
        );
        // 守恒：扣 flat 25，Gallbladder ×0.75。
        assert_eq!(qi(&app, caster), 75.0, "100 - 25 flat qi_cost");
        assert!(
            (meridian_integrity(&app, caster, MeridianId::Gallbladder) - 0.75).abs() < 1e-12,
            "Gallbladder torn 1.0 -> 0.75"
        );
        // 位移：沿 +Z 推 4.0 格（服务器权威 Position.set）。
        let pos = app.world().get::<Position>(caster).unwrap().get();
        assert!(
            (pos.z - XUE_BENG_BU_DASH_BLOCKS).abs() < 1e-9 && pos.x.abs() < 1e-9,
            "caster should dash +Z by 4.0 blocks, got {pos:?}"
        );
        // 无攻击事件（纯位移招）。
        assert!(
            app.world().resource::<Events<AttackIntent>>().is_empty(),
            "xue_beng_bu must not emit AttackIntent"
        );
        assert_eq!(last_burst_skill(&app), XUE_BENG_BU_EVENT_SKILL);
        assert_eq!(emitted_audio_recipe(&app).as_deref(), Some("movement_dash"));
    }

    #[test]
    fn xue_beng_bu_rejects_when_no_look_no_displacement() {
        let mut app = full_app();
        // 无 Look component → 无朝向 → 拒绝，绝不凭空位移。
        let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);

        let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

        assert_eq!(result, rejected(CastRejectReason::InvalidTarget));
        let pos = app.world().get::<Position>(caster).unwrap().get();
        assert_eq!(pos, DVec3::ZERO, "no displacement on rejection");
        assert_single_meridian_untouched(&app, caster, MeridianId::Gallbladder, 100.0);
    }

    #[test]
    fn xue_beng_bu_rejects_low_realm_without_mutation() {
        let mut app = full_app();
        let caster = spawn_caster_with_look(&mut app, Realm::Induce, 100.0, DVec3::ZERO, 0.0);

        let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

        assert_eq!(result, rejected(CastRejectReason::RealmTooLow));
        let pos = app.world().get::<Position>(caster).unwrap().get();
        assert_eq!(pos, DVec3::ZERO);
        assert_single_meridian_untouched(&app, caster, MeridianId::Gallbladder, 100.0);
    }

    #[test]
    fn xue_beng_bu_rejects_insufficient_qi_without_mutation() {
        let mut app = full_app();
        let caster = spawn_caster_with_look(&mut app, Realm::Condense, 20.0, DVec3::ZERO, 0.0);

        let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

        assert_eq!(result, rejected(CastRejectReason::QiInsufficient));
        let pos = app.world().get::<Position>(caster).unwrap().get();
        assert_eq!(pos, DVec3::ZERO);
        assert_single_meridian_untouched(&app, caster, MeridianId::Gallbladder, 20.0);
    }

    #[test]
    fn xue_beng_bu_rejects_when_gallbladder_severed() {
        let mut app = full_app();
        let caster = spawn_caster_with_look(&mut app, Realm::Condense, 100.0, DVec3::ZERO, 0.0);
        sever_meridian(&mut app, caster, MeridianId::Gallbladder);

        let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            rejected(CastRejectReason::MeridianSevered(Some(
                MeridianId::Gallbladder
            )))
        );
        assert_eq!(qi(&app, caster), 100.0);
        let pos = app.world().get::<Position>(caster).unwrap().get();
        assert_eq!(pos, DVec3::ZERO);
    }

    #[test]
    fn xue_beng_bu_rejects_on_cooldown() {
        let mut app = full_app();
        let caster = spawn_caster_with_look(&mut app, Realm::Condense, 100.0, DVec3::ZERO, 0.0);
        app.world_mut()
            .get_mut::<SkillBarBindings>(caster)
            .unwrap()
            .set_cooldown(0, 11);

        let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

        assert_eq!(result, rejected(CastRejectReason::OnCooldown));
        let pos = app.world().get::<Position>(caster).unwrap().get();
        assert_eq!(pos, DVec3::ZERO);
        assert_single_meridian_untouched(&app, caster, MeridianId::Gallbladder, 100.0);
    }

    // ─── 逆脉护体（ni_mai_hu_ti）─ Solidify / Pericardium≥0.55 / qi 45 / cd 120 ─────

    #[test]
    fn ni_mai_hu_ti_happy_path_spends_qi_tears_pericardium_applies_buff() {
        let mut app = full_app();
        let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);

        let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            CastResult::Started {
                cooldown_ticks: 120,
                anim_duration_ticks: 12,
            }
        );
        // 守恒：扣 flat 45，Pericardium ×0.7。
        assert_eq!(qi(&app, caster), 55.0, "100 - 45 flat qi_cost");
        assert!(
            (meridian_integrity(&app, caster, MeridianId::Pericardium) - 0.7).abs() < 1e-12,
            "Pericardium torn 1.0 -> 0.7"
        );
        // buff：DamageReduction(0.35, 60t) 给自身。
        let buff = app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .iter_current_update_events()
            .next()
            .expect("ApplyStatusEffectIntent emitted");
        assert_eq!(buff.target, caster);
        assert_eq!(buff.kind, StatusEffectKind::DamageReduction);
        assert!((buff.magnitude - NI_MAI_HU_TI_DAMAGE_REDUCTION).abs() < 1e-6);
        assert_eq!(buff.duration_ticks, NI_MAI_HU_TI_BUFF_DURATION_TICKS);
        // 自身招：无攻击。
        assert!(app.world().resource::<Events<AttackIntent>>().is_empty());
        assert_eq!(last_burst_skill(&app), NI_MAI_HU_TI_EVENT_SKILL);
        assert_eq!(
            emitted_audio_recipe(&app).as_deref(),
            Some("zhenmai_shield_hum")
        );
    }

    #[test]
    fn ni_mai_hu_ti_rejects_low_realm_without_mutation() {
        let mut app = full_app();
        // Condense < Solidify → 拒绝。
        let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);

        let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

        assert_eq!(result, rejected(CastRejectReason::RealmTooLow));
        assert_single_meridian_untouched(&app, caster, MeridianId::Pericardium, 100.0);
        assert!(app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .is_empty());
    }

    #[test]
    fn ni_mai_hu_ti_rejects_insufficient_qi_without_mutation() {
        let mut app = full_app();
        let caster = spawn_caster(&mut app, Realm::Solidify, 40.0, DVec3::ZERO);

        let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

        assert_eq!(result, rejected(CastRejectReason::QiInsufficient));
        assert_single_meridian_untouched(&app, caster, MeridianId::Pericardium, 40.0);
        assert!(app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .is_empty());
    }

    #[test]
    fn ni_mai_hu_ti_rejects_when_pericardium_severed() {
        let mut app = full_app();
        let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);
        sever_meridian(&mut app, caster, MeridianId::Pericardium);

        let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            rejected(CastRejectReason::MeridianSevered(Some(
                MeridianId::Pericardium
            )))
        );
        assert_eq!(qi(&app, caster), 100.0);
        assert!(app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .is_empty());
    }

    #[test]
    fn ni_mai_hu_ti_rejects_on_cooldown() {
        let mut app = full_app();
        let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);
        app.world_mut()
            .get_mut::<SkillBarBindings>(caster)
            .unwrap()
            .set_cooldown(0, 11);

        let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

        assert_eq!(result, rejected(CastRejectReason::OnCooldown));
        assert_single_meridian_untouched(&app, caster, MeridianId::Pericardium, 100.0);
        assert!(app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .is_empty());
    }

    // ─── 跨招守恒 / 精度 / 边界 ───────────────────────────────────────────────────

    #[test]
    fn tie_shan_kao_float_precision_preserved() {
        let mut app = full_app();
        let caster = spawn_caster(&mut app, Realm::Condense, 99.9, DVec3::ZERO);
        // 把 Stomach 设到刚好满足 min_health=0.5 边界附近（runtime gate 只看 >ε）。
        app.world_mut()
            .get_mut::<MeridianSystem>(caster)
            .unwrap()
            .get_mut(MeridianId::Stomach)
            .integrity = 0.6;
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

        assert!(matches!(result, CastResult::Started { .. }));
        assert!((qi(&app, caster) - 64.9).abs() < 1e-9, "99.9 - 35 = 64.9");
        assert!(
            (meridian_integrity(&app, caster, MeridianId::Stomach) - 0.48).abs() < 1e-12,
            "0.6 * 0.8 = 0.48"
        );
        // integrity_snapshot 记的是损前快照 0.6。
        let snap = app
            .world()
            .resource::<Events<BurstMeridianEvent>>()
            .iter_current_update_events()
            .next()
            .unwrap()
            .integrity_snapshot;
        assert!((snap - 0.6).abs() < 1e-12);
    }

    #[test]
    fn all_three_skills_register_and_declare_so_audit_invariant_holds() {
        // 注册 + declare 配对：缺一即 skill_registry 审计不变量赤。
        let mut registry = SkillRegistry::default();
        register_skills(&mut registry);
        assert!(registry.lookup(TIE_SHAN_KAO_SKILL_ID).is_some());
        assert!(registry.lookup(XUE_BENG_BU_SKILL_ID).is_some());
        assert!(registry.lookup(NI_MAI_HU_TI_SKILL_ID).is_some());

        let mut deps = crate::cultivation::meridian::severed::SkillMeridianDependencies::default();
        declare_meridian_dependencies(&mut deps);
        assert_eq!(deps.lookup(TIE_SHAN_KAO_SKILL_ID), &[MeridianId::Stomach]);
        assert_eq!(
            deps.lookup(XUE_BENG_BU_SKILL_ID),
            &[MeridianId::Gallbladder]
        );
        assert_eq!(
            deps.lookup(NI_MAI_HU_TI_SKILL_ID),
            &[MeridianId::Pericardium]
        );
    }

    #[test]
    fn flat_qi_cost_reads_from_known_techniques_single_source() {
        // 严禁本文件硬编码重复 qi_cost：值必须来自 known_techniques。
        assert_eq!(flat_qi_cost(TIE_SHAN_KAO_SKILL_ID), Some(35.0));
        assert_eq!(flat_qi_cost(XUE_BENG_BU_SKILL_ID), Some(25.0));
        assert_eq!(flat_qi_cost(NI_MAI_HU_TI_SKILL_ID), Some(45.0));
        assert_eq!(flat_qi_cost("nonexistent.skill"), None);
    }

    // ─── 真元守恒：burst_meridian 招式扣 qi 后必须释放回区域 ────────────────────────
    // 对齐 baomai_v3 / tuike_v2 的 emit_spent_qi_release 模式。
    // resolve.rs 把 BurstMeridian 加入 source_uses_prepaid_qi 白名单，
    // 意味着 combat resolver 不会补做任何区域释放——必须在 spend_qi 本地完成。
    // ─────────────────────────────────────────────────────────────────────────────

    use crate::qi_physics::{QiTransfer, QiTransferReason};
    use crate::world::zone::ZoneRegistry;

    /// 带 ZoneRegistry（fallback spawn zone，覆盖 y=70 层面）和 QiTransfer event 的测试 App。
    /// caster 位置应使用 DVec3::new(0.0, 70.0, 0.0) 才落在 spawn zone AABB 内。
    fn app_with_zone() -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 10 });
        app.add_event::<AttackIntent>();
        app.add_event::<BurstMeridianEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_event::<QiTransfer>();
        app.insert_resource(ZoneRegistry::fallback());
        app
    }

    fn zone_spirit_qi(app: &App) -> f64 {
        app.world()
            .resource::<ZoneRegistry>()
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(0.0, 70.0, 0.0),
            )
            .expect("spawn zone must cover (0,70,0)")
            .spirit_qi
    }

    fn qi_transfer_count(app: &App) -> usize {
        app.world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .count()
    }

    /// beng_quan 施法后区域 spirit_qi 必须升高（消耗的真元返还区域）。
    #[test]
    fn beng_quan_spend_qi_releases_to_zone() {
        let mut app = app_with_zone();
        // 在 spawn zone AABB 内（y=70）生成施法者。
        let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::new(0.0, 70.0, 0.0));
        let target = spawn_target(&mut app, DVec3::new(0.0, 70.0, f64::from(FIST_REACH.max)));

        let initial_spirit_qi = zone_spirit_qi(&app);

        let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

        assert!(
            matches!(result, CastResult::Started { .. }),
            "beng_quan must succeed; got {result:?}"
        );
        assert!(
            zone_spirit_qi(&app) > initial_spirit_qi,
            "zone spirit_qi must rise after beng_quan spend_qi; \
             before={initial_spirit_qi}, after={}",
            zone_spirit_qi(&app)
        );
        assert!(
            qi_transfer_count(&app) > 0,
            "a QiTransfer must be emitted for the zone release"
        );
    }

    /// tie_shan_kao 施法后区域 spirit_qi 必须升高（守恒同 beng_quan）。
    #[test]
    fn tie_shan_kao_spend_qi_releases_to_zone() {
        let mut app = app_with_zone();
        let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::new(0.0, 70.0, 0.0));
        let target = spawn_target(&mut app, DVec3::new(0.0, 70.0, 1.0));

        let initial_spirit_qi = zone_spirit_qi(&app);

        let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

        assert!(
            matches!(result, CastResult::Started { .. }),
            "tie_shan_kao must succeed; got {result:?}"
        );
        assert!(
            zone_spirit_qi(&app) > initial_spirit_qi,
            "zone spirit_qi must rise after tie_shan_kao spend_qi; \
             before={initial_spirit_qi}, after={}",
            zone_spirit_qi(&app)
        );
    }

    /// xue_beng_bu 施法后区域 spirit_qi 必须升高。
    #[test]
    fn xue_beng_bu_spend_qi_releases_to_zone() {
        let mut app = app_with_zone();
        // yaw=0 → facing = -z 方向（sin(0)=0, cos(0)=1），用 Look::new(0.0, 0.0)。
        let caster = spawn_caster_with_look(
            &mut app,
            Realm::Solidify,
            100.0,
            DVec3::new(0.0, 70.0, 0.0),
            0.0,
        );

        let initial_spirit_qi = zone_spirit_qi(&app);

        let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

        assert!(
            matches!(result, CastResult::Started { .. }),
            "xue_beng_bu must succeed; got {result:?}"
        );
        assert!(
            zone_spirit_qi(&app) > initial_spirit_qi,
            "zone spirit_qi must rise after xue_beng_bu spend_qi; \
             before={initial_spirit_qi}, after={}",
            zone_spirit_qi(&app)
        );
    }

    /// ni_mai_hu_ti 施法后区域 spirit_qi 必须升高。
    #[test]
    fn ni_mai_hu_ti_spend_qi_releases_to_zone() {
        let mut app = app_with_zone();
        let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::new(0.0, 70.0, 0.0));

        let initial_spirit_qi = zone_spirit_qi(&app);

        let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

        assert!(
            matches!(result, CastResult::Started { .. }),
            "ni_mai_hu_ti must succeed; got {result:?}"
        );
        assert!(
            zone_spirit_qi(&app) > initial_spirit_qi,
            "zone spirit_qi must rise after ni_mai_hu_ti spend_qi; \
             before={initial_spirit_qi}, after={}",
            zone_spirit_qi(&app)
        );
    }

    /// 拒绝路径（如真元不足）不触发任何区域释放——守恒不能凭空注入灵气。
    #[test]
    fn rejected_cast_does_not_release_qi_to_zone() {
        let mut app = app_with_zone();
        let caster = spawn_caster(&mut app, Realm::Awaken, 0.0, DVec3::new(0.0, 70.0, 0.0));
        let target = spawn_target(&mut app, DVec3::new(0.0, 70.0, 1.0));

        let initial_spirit_qi = zone_spirit_qi(&app);

        let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

        assert!(
            matches!(result, CastResult::Rejected { .. }),
            "must be rejected; got {result:?}"
        );
        assert_eq!(
            zone_spirit_qi(&app),
            initial_spirit_qi,
            "zone spirit_qi must be unchanged when cast is rejected — \
             no qi was consumed so none can be released"
        );
        assert_eq!(
            qi_transfer_count(&app),
            0,
            "no QiTransfer must be emitted on rejected cast"
        );
    }

    /// 区域已满（spirit_qi = 1.0）时，消耗的真元应路由至 overflow，zone 不超限。
    #[test]
    fn beng_quan_routes_overflow_when_zone_is_full() {
        let mut app = app_with_zone();
        // 把 spawn zone 灵气填满。
        app.world_mut()
            .resource_mut::<ZoneRegistry>()
            .find_zone_mut("spawn")
            .expect("spawn zone must exist")
            .spirit_qi = 1.0;

        let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::new(0.0, 70.0, 0.0));
        let target = spawn_target(&mut app, DVec3::new(0.0, 70.0, f64::from(FIST_REACH.max)));

        let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

        assert!(
            matches!(result, CastResult::Started { .. }),
            "beng_quan must succeed even when zone is full; got {result:?}"
        );
        assert_eq!(
            zone_spirit_qi(&app),
            1.0,
            "zone spirit_qi must not exceed 1.0 when zone is full"
        );
        // overflow 路径：仍发 QiTransfer（overflow 账户）。
        assert!(
            qi_transfer_count(&app) > 0,
            "a QiTransfer (overflow) must be emitted even when zone is full"
        );
        // 拿到 overflow transfer，验证其目的账户种类。
        let transfer = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .next()
            .expect("at least one QiTransfer must be emitted");
        assert_eq!(
            transfer.reason,
            QiTransferReason::ReleaseToZone,
            "overflow transfer reason must be ReleaseToZone"
        );
    }
}
