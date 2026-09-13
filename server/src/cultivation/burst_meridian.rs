use valence::entity::Look;
use valence::prelude::{
    bevy_ecs, Commands, Component, DVec3, Despawned, Entity, Event, EventWriter, Events, Position,
    Query, Res, UniqueId, Without,
};

use crate::body_plan::intrinsic_is_humanoid_from_world;
use crate::combat::components::{
    CastSource, Casting, SkillBarBindings, Stamina, StaminaState, WoundKind,
};
use crate::combat::events::{
    ApplyStatusEffectIntent, AttackIntent, AttackReach, AttackSource, StatusEffectKind, FIST_REACH,
};
use crate::combat::CombatClock;
use crate::cultivation::color::{record_style_practice, PracticeLog};
use crate::cultivation::components::{
    ColorKind, Cultivation, MeridianId, MeridianSystem, QiColor, Realm,
};
#[cfg(test)]
use crate::cultivation::known_techniques::TechniqueRequiredMeridian;
use crate::cultivation::known_techniques::{TechniqueDefinition, TechniqueRegistry};
use crate::cultivation::meridian::severed::{
    check_meridian_runtime_integrity, check_player_skill_meridian_gate, MeridianSeveredPermanent,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::cultivation::technique_scroll::parse_meridian_id;
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
pub(crate) const BENG_QUAN_PARTICLE_ID: &str = "bong:burst_meridian_beng_quan";

/// 爆脉家族统一识别色（plan §P5.1 ②）。四招共用同一色系，读招**完全靠形态**
/// 分化——崩拳 Line 爆发 / 靠撞 GroundDecal 冲击环 / 血崩步 Ribbon 残影 /
/// 逆脉护体 Sprite 体表环绕。
pub(crate) const BURST_MERIDIAN_FAMILY_COLOR: &str = "#C58B3F";

pub const BENG_QUAN_SKILL_ID: &str = "burst_meridian.beng_quan";
pub const BENG_QUAN_EVENT_SKILL: &str = "beng_quan";
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
/// plan-skill-anim-fidelity-v1 P3 —— 专属靠身撞击动画（借用解除：原借崩拳出拳
/// `bong:beng_quan`，肩胯靠撞与出拳姿态语义完全不同）。
const TIE_SHAN_KAO_ANIM_ID: &str = "bong:tie_shan_kao";
/// plan-skill-anim-fidelity-v1 P5 —— 专属撞击冲击环粒子（借用解除：原借崩拳
/// `bong:burst_meridian_beng_quan`，与崩拳同粒子则旁观者无从分辨靠撞与出拳）。
/// 形态 = 地面 `BongGroundDecalParticle` 冲击环，见 plan §P5.1 ②。
pub(crate) const TIE_SHAN_KAO_PARTICLE_ID: &str = "bong:burst_meridian_tie_shan_kao";
const TIE_SHAN_KAO_AUDIO_RECIPE: &str = "hit_heavy";

// ─── 血崩步（xue_beng_bu）─ 腿经裂响换短距突进，抢入战圈 ──────────────────────────
pub const XUE_BENG_BU_SKILL_ID: &str = "burst_meridian.xue_beng_bu";
pub const XUE_BENG_BU_EVENT_SKILL: &str = "xue_beng_bu";
/// 撕裂腿经（GallBladder）后的 integrity 残留比例。
pub const XUE_BENG_BU_INTEGRITY_MULTIPLIER: f64 = 0.75;
/// 突进距离（格），与 known_techniques.xue_beng_bu.range 对齐。
/// plan-skill-anim-fidelity-v1 P3 —— 专属步法突进动画（借用解除：原借崩拳出拳
/// `bong:beng_quan`，位移招播出拳属姿态语义错位）。
const XUE_BENG_BU_ANIM_ID: &str = "bong:xue_beng_bu";
/// plan-skill-anim-fidelity-v1 P5 —— 专属步法残影粒子（借用解除同上）。
/// 形态 = `BongRibbonParticle` 反向拖尾短残影，见 plan §P5.1 ②。
pub(crate) const XUE_BENG_BU_PARTICLE_ID: &str = "bong:burst_meridian_xue_beng_bu";
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
/// plan-skill-anim-fidelity-v1 P3 —— 专属护体结印动画（缺失补齐：原 `anim_id: None`
/// 完全不发 PlayAnim，护体招只有粒子+嗡音、玩家无姿态反馈）。
const NI_MAI_HU_TI_ANIM_ID: &str = "bong:ni_mai_hu_ti";
/// plan-skill-anim-fidelity-v1 P5 —— 专属体表逆流纹粒子（借用解除同上）。
/// 形态 = `BongSpriteParticle` 双高度体表环绕，见 plan §P5.1 ②。
pub(crate) const NI_MAI_HU_TI_PARTICLE_ID: &str = "bong:burst_meridian_ni_mai_hu_ti";
const NI_MAI_HU_TI_AUDIO_RECIPE: &str = "zhenmai_shield_hum";
/// 护体环重发间隔（tick）—— plan §P5.1 ② 「体表逆流纹」的锚定实现。
///
/// `SpawnParticle` payload 只有世界坐标 `origin`、没有实体标识，一次性发一个 60t 长寿命
/// 环等于把纹钉死在施法瞬间的坐标上——玩家一移动就把「体表」纹留在原地。故改由 server 在
/// buff 存续期内按本间隔、以施法者**当前** `Position` 重发短寿命环。
///
/// `60 = 5 × 12` 整除，于是恰好 5 次发射（cast 首环 + 存续期重发 4 次）铺满 buff 窗口：
/// 最后一环在 buff 到期的同一 tick 结束，不会出现「护体已过而纹还在转」的尾巴。
pub const NI_MAI_HU_TI_AURA_REEMIT_INTERVAL_TICKS: u64 = 12;
/// 单环寿命 == 重发间隔：老环恰在新环生成的同 tick 消失，既不叠环也不留空窗。
pub const NI_MAI_HU_TI_AURA_PARTICLE_LIFETIME_TICKS: u16 =
    NI_MAI_HU_TI_AURA_REEMIT_INTERVAL_TICKS as u16;
/// 护体环形态参数 —— cast 首环（`emit_burst_av`）与存续期重发共用同一组常量，
/// 两条路径发散会让窗口中途的环突然换个模样。
const NI_MAI_HU_TI_PARTICLE_STRENGTH: f32 = 0.7;
const NI_MAI_HU_TI_PARTICLE_COUNT: u16 = 14;

/// 逆脉护体存续期的体表逆流纹锚点：存在即「护体窗口内」。
///
/// 由 `resolve_ni_mai_hu_ti` 插入、`ni_mai_hu_ti_aura_vfx_tick` 到期移除。
///
/// **为什么不复用 `StatusEffects`**：护体的减伤走 `StatusEffectKind::DamageReduction`，而这是
/// 一个**共享 kind**——渡劫丹、NPC `buff_defense` 等也写它，且 `upsert_status_effect` 只按 kind
/// 合并取 max。读 `StatusEffects` 无从分辨「护体开着」还是「别的减伤开着」，会给嗑了渡劫丹的
/// 玩家凭空挂上爆脉护体环。
#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct NiMaiHuTiAura {
    /// 施法 tick。重发相位以此为基准，而非对全局 clock 取模——否则 cast 落在相位边界时
    /// 首个重发环会紧跟着首环发出，间隔不足。
    pub started_at_tick: u64,
    /// buff 到期 tick = `started_at_tick + NI_MAI_HU_TI_BUFF_DURATION_TICKS`。
    pub expires_at_tick: u64,
}

/// Static resolver contracts. Keep these independent from TOML so startup can detect
/// metadata drift instead of deriving both sides from the same source.
pub const RIGHT_ARM_MERIDIANS: [MeridianId; 3] = [
    MeridianId::LargeIntestine,
    MeridianId::SmallIntestine,
    MeridianId::TripleEnergizer,
];
pub const TIE_SHAN_KAO_MERIDIANS: [MeridianId; 1] = [MeridianId::Stomach];
pub const XUE_BENG_BU_MERIDIANS: [MeridianId; 1] = [MeridianId::Gallbladder];
pub const NI_MAI_HU_TI_MERIDIANS: [MeridianId; 1] = [MeridianId::Pericardium];

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

/// Declare the meridian dependencies owned by the static resolvers.
///
/// TOML `required_meridians` is validated against these contracts during startup; it must not
/// be used to construct them, or a metadata/resolver mismatch would be invisible.
pub fn declare_meridian_dependencies(
    dependencies: &mut crate::cultivation::meridian::severed::SkillMeridianDependencies,
) {
    dependencies.declare(BENG_QUAN_SKILL_ID, RIGHT_ARM_MERIDIANS.to_vec());
    dependencies.declare(TIE_SHAN_KAO_SKILL_ID, TIE_SHAN_KAO_MERIDIANS.to_vec());
    dependencies.declare(XUE_BENG_BU_SKILL_ID, XUE_BENG_BU_MERIDIANS.to_vec());
    dependencies.declare(NI_MAI_HU_TI_SKILL_ID, NI_MAI_HU_TI_MERIDIANS.to_vec());
}

/// 从 known_techniques 读招式的 flat qi_cost（单一真值源，严禁在本文件硬编码重复）。
fn flat_qi_cost(techniques: &TechniqueRegistry, skill_id: &str) -> Option<f64> {
    techniques.get(skill_id).map(|def| def.qi_cost)
}

/// 空挥（无锁定目标）时的 AV 朝向点：沿施法者视线前推一臂；无 Look 组件时
/// 退化为正前方 +Z（只影响粒子方向，无任何战斗判定）。
fn whiff_focus_point(
    world: &bevy_ecs::world::World,
    caster: Entity,
    caster_position: valence::prelude::DVec3,
    reach: f32,
) -> valence::prelude::DVec3 {
    let dir = world
        .get::<Look>(caster)
        .map(|look| look.vec().as_dvec3())
        .unwrap_or(valence::prelude::DVec3::Z);
    caster_position + dir * f64::from(reach)
}

pub fn resolve_beng_quan(
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
        .get::<SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(BENG_QUAN_SKILL_ID, now_tick))
    {
        return rejected(CastRejectReason::OnCooldown);
    }

    let Some(caster_position) = world.get::<Position>(caster).map(|position| position.get()) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let definition = world
        .get_resource::<TechniqueRegistry>()
        .and_then(|techniques| techniques.get(BENG_QUAN_SKILL_ID))
        .cloned();
    let Some(definition) = definition else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    // `definition.range` 保留旧 metadata 展示值 1.3；resolver 的有效距离和衰减基准
    // 历来是 FIST_REACH = base 2.0 + step bonus 0.6，不能用展示值把 2.6 射程砍半。
    let effective_reach = f64::from(FIST_REACH.max);
    // Option B 去目标门禁（对齐 sword_basics 劈/刺）：崩拳是近战直拳，准星没对准
    // 实体也照常轰出（动画/粒子/扣费/撕脉/冷却照走），无目标 = 空挥。
    // 有目标时仍校验存在与射程——锁着超距目标硬轰属"目标无效"的正确语义。
    let target_position = match target {
        Some(target) => {
            let Some(target_position) =
                world.get::<Position>(target).map(|position| position.get())
            else {
                return rejected(CastRejectReason::InvalidTarget);
            };
            if caster_position.distance(target_position) > effective_reach + f64::from(f32::EPSILON)
            {
                return rejected(CastRejectReason::InvalidTarget);
            }
            Some(target_position)
        }
        None => None,
    };

    if let Some(reason) = check_race_gate(world, caster, &definition) {
        return rejected(reason);
    }
    if let Some(reason) = check_realm_gate(world, caster, definition.required_realm_value()) {
        return rejected(reason);
    }
    let cost = world
        .get::<Cultivation>(caster)
        .map(|cultivation| cultivation.qi_current * definition.qi_cost)
        .unwrap_or(0.0);
    if let Some(reason) = check_qi_gate(world, caster, cost) {
        return rejected(reason);
    }
    // M31：体力门。零成本放行（M33），Exhausted / 不足拒绝。
    if let Some(reason) = check_stamina_gate(world, caster, definition.stamina_cost) {
        return rejected(reason);
    }
    if let Err(reason) = check_beng_quan_meridian_gate(world, caster, &definition) {
        return rejected(reason);
    }
    let Some(integrity_snapshot) =
        right_arm_integrity_snapshot_from_definition(world, caster, &definition)
    else {
        return rejected(CastRejectReason::MeridianSevered(None));
    };
    let cast_ticks = definition.cast_ticks.max(1);
    let cooldown_ticks = u64::from(definition.cooldown_ticks).max(1);

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
        skill_id: Some(BENG_QUAN_SKILL_ID.to_string()),
        skill_config: None,
    });

    spend_qi(world, caster, cost);
    apply_stamina_cost(world, caster, definition.stamina_cost, now_tick);
    if let Some(mut meridians) = world.get_mut::<MeridianSystem>(caster) {
        for required in &definition.required_meridians {
            let Some(id) = parse_meridian_id(&required.channel) else {
                continue;
            };
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
        // Option 透传：Some 命中结算，None 时 resolver 跳过 = 空挥。
        target,
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
        target,
        tick: now_tick,
        overload_ratio: BENG_QUAN_OVERLOAD_RATIO,
        integrity_snapshot,
    });
    let vfx_toward = target_position
        .unwrap_or_else(|| whiff_focus_point(world, caster, caster_position, FIST_REACH.max));
    emit_beng_quan_vfx(world, caster, caster_position, vfx_toward, cast_ticks);

    CastResult::Started {
        cooldown_ticks,
        anim_duration_ticks: cast_ticks,
    }
}

fn emit_beng_quan_vfx(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    caster_position: valence::prelude::DVec3,
    target_position: valence::prelude::DVec3,
    cast_ticks: u32,
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
            color: Some(BURST_MERIDIAN_FAMILY_COLOR.to_string()),
            strength: Some(0.9),
            count: Some(8),
            duration_ticks: Some(cast_ticks as u16),
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
    let Some(now_tick) = combat_now_tick(world) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if is_slot_on_cooldown(world, caster, TIE_SHAN_KAO_SKILL_ID, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    let Some(caster_position) = world.get::<Position>(caster).map(|p| p.get()) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    // Option B 去目标门禁（与崩拳同批）：肩撞照常撞出，无目标 = 空撞。
    let definition = world
        .get_resource::<TechniqueRegistry>()
        .and_then(|techniques| techniques.get(TIE_SHAN_KAO_SKILL_ID))
        .cloned();
    let Some(definition) = definition else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let reach = f64::from(definition.range);
    let target_position = match target {
        Some(target) => {
            let Some(target_position) = world.get::<Position>(target).map(|p| p.get()) else {
                return rejected(CastRejectReason::InvalidTarget);
            };
            if caster_position.distance(target_position) > reach + f64::EPSILON {
                return rejected(CastRejectReason::InvalidTarget);
            }
            Some(target_position)
        }
        None => None,
    };

    if let Some(reason) = check_race_gate(world, caster, &definition) {
        return rejected(reason);
    }
    if let Some(reason) = check_realm_gate(world, caster, definition.required_realm_value()) {
        return rejected(reason);
    }
    let (cost, cast_ticks, cooldown_ticks) = (
        definition.qi_cost,
        definition.cast_ticks.max(1),
        u64::from(definition.cooldown_ticks).max(1),
    );
    if let Some(reason) = check_qi_gate(world, caster, cost) {
        return rejected(reason);
    }
    // M31：体力门。零成本放行（M33），Exhausted / 不足拒绝。
    if let Some(reason) = check_stamina_gate(world, caster, definition.stamina_cost) {
        return rejected(reason);
    }
    let Some(primary_meridian) = definition
        .required_meridians
        .first()
        .and_then(|required| parse_meridian_id(&required.channel))
    else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if let Err(reason) = check_definition_meridian_gate(world, caster, &definition) {
        return rejected(reason);
    }
    let Some(integrity_snapshot) = check_single_meridian_gate(world, caster, primary_meridian)
    else {
        return rejected(CastRejectReason::MeridianSevered(Some(primary_meridian)));
    };

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

    // ── 守恒：扣 flat qi + 体力 + 撕裂躯干经脉（系列代价）────────────────────────
    spend_qi(world, caster, cost);
    apply_stamina_cost(world, caster, definition.stamina_cost, now_tick);
    tear_meridian(
        world,
        caster,
        primary_meridian,
        TIE_SHAN_KAO_INTEGRITY_MULTIPLIER,
    );
    record_heavy_practice(world, caster);

    world.send_event(AttackIntent {
        attacker: caster,
        // Option 透传：Some 命中结算，None 时 resolver 跳过 = 空撞。
        target,
        issued_at_tick: now_tick,
        reach: AttackReach::new(definition.range, 0.0),
        qi_invest: (cost * TIE_SHAN_KAO_OVERLOAD_RATIO) as f32,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::BurstMeridian,
        debug_command: None,
    });
    world.send_event(BurstMeridianEvent {
        skill: TIE_SHAN_KAO_EVENT_SKILL,
        caster,
        target,
        tick: now_tick,
        overload_ratio: TIE_SHAN_KAO_OVERLOAD_RATIO,
        integrity_snapshot,
    });
    let av_toward = target_position
        .unwrap_or_else(|| whiff_focus_point(world, caster, caster_position, definition.range));
    emit_burst_av(
        world,
        caster,
        caster_position,
        Some(av_toward - caster_position),
        BurstAv {
            anim_id: Some(TIE_SHAN_KAO_ANIM_ID),
            particle_id: TIE_SHAN_KAO_PARTICLE_ID,
            color: BURST_MERIDIAN_FAMILY_COLOR,
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
    if is_slot_on_cooldown(world, caster, XUE_BENG_BU_SKILL_ID, now_tick) {
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

    let definition = world
        .get_resource::<TechniqueRegistry>()
        .and_then(|techniques| techniques.get(XUE_BENG_BU_SKILL_ID))
        .cloned();
    let Some(definition) = definition else {
        return rejected(CastRejectReason::InvalidTarget);
    };

    if let Some(reason) = check_race_gate(world, caster, &definition) {
        return rejected(reason);
    }
    if let Some(reason) = check_realm_gate(world, caster, definition.required_realm_value()) {
        return rejected(reason);
    }
    let (cost, cast_ticks, cooldown_ticks) = (
        definition.qi_cost,
        definition.cast_ticks.max(1),
        u64::from(definition.cooldown_ticks).max(1),
    );
    if let Some(reason) = check_qi_gate(world, caster, cost) {
        return rejected(reason);
    }
    // M31：体力门。零成本放行（M33），Exhausted / 不足拒绝。
    if let Some(reason) = check_stamina_gate(world, caster, definition.stamina_cost) {
        return rejected(reason);
    }
    let Some(primary_meridian) = definition
        .required_meridians
        .first()
        .and_then(|required| parse_meridian_id(&required.channel))
    else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if let Err(reason) = check_definition_meridian_gate(world, caster, &definition) {
        return rejected(reason);
    }
    let Some(integrity_snapshot) = check_single_meridian_gate(world, caster, primary_meridian)
    else {
        return rejected(CastRejectReason::MeridianSevered(Some(primary_meridian)));
    };

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

    // ── 守恒：扣 flat qi + 体力 + 撕裂腿经（系列代价）+ 服务器权威位移 ────────────
    spend_qi(world, caster, cost);
    apply_stamina_cost(world, caster, definition.stamina_cost, now_tick);
    tear_meridian(
        world,
        caster,
        primary_meridian,
        XUE_BENG_BU_INTEGRITY_MULTIPLIER,
    );
    record_heavy_practice(world, caster);
    let dash_target = caster_position + facing * f64::from(definition.range);
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
            color: BURST_MERIDIAN_FAMILY_COLOR,
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
    if is_slot_on_cooldown(world, caster, NI_MAI_HU_TI_SKILL_ID, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    let Some(caster_position) = world.get::<Position>(caster).map(|p| p.get()) else {
        return rejected(CastRejectReason::InvalidTarget);
    };

    let definition = world
        .get_resource::<TechniqueRegistry>()
        .and_then(|techniques| techniques.get(NI_MAI_HU_TI_SKILL_ID))
        .cloned();
    let Some(definition) = definition else {
        return rejected(CastRejectReason::InvalidTarget);
    };

    if let Some(reason) = check_race_gate(world, caster, &definition) {
        return rejected(reason);
    }
    if let Some(reason) = check_realm_gate(world, caster, definition.required_realm_value()) {
        return rejected(reason);
    }
    let (cost, cast_ticks, cooldown_ticks) = (
        definition.qi_cost,
        definition.cast_ticks.max(1),
        u64::from(definition.cooldown_ticks).max(1),
    );
    if let Some(reason) = check_qi_gate(world, caster, cost) {
        return rejected(reason);
    }
    // M31：体力门。零成本放行（M33），Exhausted / 不足拒绝。
    if let Some(reason) = check_stamina_gate(world, caster, definition.stamina_cost) {
        return rejected(reason);
    }
    let Some(primary_meridian) = definition
        .required_meridians
        .first()
        .and_then(|required| parse_meridian_id(&required.channel))
    else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if let Err(reason) = check_definition_meridian_gate(world, caster, &definition) {
        return rejected(reason);
    }
    let Some(integrity_snapshot) = check_single_meridian_gate(world, caster, primary_meridian)
    else {
        return rejected(CastRejectReason::MeridianSevered(Some(primary_meridian)));
    };

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

    // ── 守恒：扣 flat qi + 体力 + 逆转真元撕本脉（系列代价）+ 短时减伤 buff ────────
    spend_qi(world, caster, cost);
    apply_stamina_cost(world, caster, definition.stamina_cost, now_tick);
    tear_meridian(
        world,
        caster,
        primary_meridian,
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
    // 逆流纹锚点：让 `ni_mai_hu_ti_aura_vfx_tick` 在 buff 存续期内跟着施法者重发护体环。
    // 重复施放（冷却结束后再来一发）走 insert 覆盖语义，窗口整体后移，不会叠出两套环。
    world.entity_mut(caster).insert(NiMaiHuTiAura {
        started_at_tick: now_tick,
        expires_at_tick: now_tick + NI_MAI_HU_TI_BUFF_DURATION_TICKS,
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
            // P3 缺失补齐：专属护体结印动画（此前 None 不发 PlayAnim——护体非攻击
            // 姿态不借崩拳出拳是对的，但缺动画=玩家零姿态反馈，MISSING allowlist 条目）。
            anim_id: Some(NI_MAI_HU_TI_ANIM_ID),
            particle_id: NI_MAI_HU_TI_PARTICLE_ID,
            color: BURST_MERIDIAN_FAMILY_COLOR,
            strength: NI_MAI_HU_TI_PARTICLE_STRENGTH,
            count: NI_MAI_HU_TI_PARTICLE_COUNT,
            // 首环也只活一个重发间隔——整个 60t 窗口由 `ni_mai_hu_ti_aura_vfx_tick`
            // 逐环接力，而不是靠一个长寿命环撑满（那样纹会脱离移动中的身体）。
            duration_ticks: NI_MAI_HU_TI_AURA_PARTICLE_LIFETIME_TICKS,
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
    skill_id: &str,
    now_tick: u64,
) -> bool {
    world
        .get::<SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(skill_id, now_tick))
}

fn check_race_gate(
    world: &bevy_ecs::world::World,
    caster: Entity,
    definition: &TechniqueDefinition,
) -> Option<CastRejectReason> {
    let race = world
        .get::<Cultivation>(caster)
        .map(|cultivation| cultivation.race.clone())
        .unwrap_or_else(|| crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID));
    (!definition
        .required_race
        .allows(&race, intrinsic_is_humanoid_from_world(world, caster)))
    .then_some(CastRejectReason::RaceMismatch)
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

/// 体力门（M31）：Exhausted / current ≤ 0 / current < stamina_cost 均拒绝。
/// 零成本（`stamina_cost <= 0`）直接放行——与 qi gate 的零成本语义对称（M33：
/// valid zero-cost metadata 不能被当作 insufficient）。
/// 不在此处扣减（扣减由 `apply_stamina_cost` 在所有门通过后执行）。
fn check_stamina_gate(
    world: &bevy_ecs::world::World,
    caster: Entity,
    stamina_cost: f32,
) -> Option<CastRejectReason> {
    if stamina_cost <= 0.0 {
        return None;
    }
    let Some(stamina) = world.get::<Stamina>(caster) else {
        // 无 Stamina 组件 = 无体力系统实体（如纯测试 spawn），不拦。
        return None;
    };
    if stamina.state == StaminaState::Exhausted
        || stamina.current <= 0.0
        || stamina.current < stamina_cost
    {
        return Some(CastRejectReason::InRecovery);
    }
    None
}

/// 扣除体力（M31）：与 `morph.yixing` 同一模式——clamp 到 [0, max] 并打
/// `last_drain_tick` 时间戳。仅在所有门通过后调用。
fn apply_stamina_cost(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    stamina_cost: f32,
    now_tick: u64,
) {
    if stamina_cost <= 0.0 {
        return;
    }
    if let Some(mut stamina) = world.get_mut::<Stamina>(caster) {
        stamina.current = (stamina.current - stamina_cost).clamp(0.0, stamina.max);
        stamina.state = if stamina.current <= 0.0 {
            StaminaState::Exhausted
        } else {
            StaminaState::Combat
        };
        stamina.last_drain_tick = Some(now_tick);
    }
}

fn check_beng_quan_meridian_gate(
    world: &bevy_ecs::world::World,
    caster: Entity,
    definition: &TechniqueDefinition,
) -> Result<(), CastRejectReason> {
    let Some(meridians) = world.get::<MeridianSystem>(caster) else {
        return Err(CastRejectReason::MeridianSevered(None));
    };
    let required = definition
        .required_meridians
        .iter()
        .filter_map(|required| {
            parse_meridian_id(&required.channel).map(|id| (id, f64::from(required.min_health)))
        })
        .collect::<Vec<_>>();
    if required.is_empty() {
        return Err(CastRejectReason::MeridianSevered(None));
    }
    let severed = world.get::<MeridianSeveredPermanent>(caster);
    for (id, min_health) in required {
        let meridian = meridians.get(id);
        if severed.is_some_and(|severed| severed.is_severed(id))
            || !meridian.opened
            || meridian.integrity < min_health
        {
            return Err(CastRejectReason::MeridianSevered(Some(id)));
        }
    }
    Ok(())
}

fn check_definition_meridian_gate(
    world: &bevy_ecs::world::World,
    caster: Entity,
    definition: &TechniqueDefinition,
) -> Result<(), CastRejectReason> {
    let Some(meridians) = world.get::<MeridianSystem>(caster) else {
        return Err(CastRejectReason::MeridianSevered(None));
    };
    let severed = world.get::<MeridianSeveredPermanent>(caster);
    let dependencies =
        world.get_resource::<crate::cultivation::meridian::severed::SkillMeridianDependencies>();
    check_player_skill_meridian_gate(
        &definition.id,
        &definition.required_meridians,
        meridians,
        severed,
        dependencies,
    )
    .map_err(|blocked| CastRejectReason::MeridianSevered(Some(blocked)))
}

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
    definition: Option<&TechniqueDefinition>,
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

/// 爆脉粒子相对脚底坐标的抬升（胸口高度）。护体环的存续期重发路径共用同一抬升，
/// 保证首环与后续环处在同一身高、窗口中途不会「跳一格」。
const BURST_AV_PARTICLE_Y_LIFT: f64 = 1.0;

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
                caster_position.y + BURST_AV_PARTICLE_Y_LIFT,
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

/// 护体逆流纹环的 `SpawnParticle` 请求（存续期重发路径）。
///
/// 形态参数与 cast 首环取自同一组常量，两条路径只差 `origin` —— 首环用施法瞬间的位置，
/// 重发用施法者**当前**位置，接力起来就是一圈始终贴着身体的纹。
fn ni_mai_hu_ti_aura_request(caster_position: DVec3) -> VfxEventRequest {
    VfxEventRequest::new(
        caster_position,
        VfxEventPayloadV1::SpawnParticle {
            event_id: NI_MAI_HU_TI_PARTICLE_ID.to_string(),
            origin: [
                caster_position.x,
                caster_position.y + BURST_AV_PARTICLE_Y_LIFT,
                caster_position.z,
            ],
            direction: None,
            color: Some(BURST_MERIDIAN_FAMILY_COLOR.to_string()),
            strength: Some(NI_MAI_HU_TI_PARTICLE_STRENGTH),
            count: Some(NI_MAI_HU_TI_PARTICLE_COUNT),
            duration_ticks: Some(NI_MAI_HU_TI_AURA_PARTICLE_LIFETIME_TICKS),
        },
    )
}

/// 逆脉护体存续期内，按 `NI_MAI_HU_TI_AURA_REEMIT_INTERVAL_TICKS` 以施法者当前位置重发护体环；
/// buff 到期即摘掉锚点、停止发射。
///
/// 必须排在 `vfx_event_emit::emit_vfx_event_payloads` 之前，否则本 tick 发的请求要等下一帧才投递。
pub fn ni_mai_hu_ti_aura_vfx_tick(
    clock: Res<CombatClock>,
    auras: Query<(Entity, &Position, &NiMaiHuTiAura), Without<Despawned>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut commands: Commands,
) {
    for (entity, position, aura) in &auras {
        if clock.tick >= aura.expires_at_tick {
            commands.entity(entity).remove::<NiMaiHuTiAura>();
            continue;
        }
        let elapsed = clock.tick.saturating_sub(aura.started_at_tick);
        // elapsed == 0 是 cast 同帧：`emit_burst_av` 已发过首环，这里再发就叠成两圈。
        if elapsed == 0 || !elapsed.is_multiple_of(NI_MAI_HU_TI_AURA_REEMIT_INTERVAL_TICKS) {
            continue;
        }
        vfx_events.send(ni_mai_hu_ti_aura_request(position.get()));
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

fn right_arm_integrity_snapshot_from_definition(
    world: &bevy_ecs::world::World,
    caster: Entity,
    definition: &TechniqueDefinition,
) -> Option<f64> {
    let meridians = world.get::<MeridianSystem>(caster)?;
    let required = definition
        .required_meridians
        .iter()
        .filter_map(|required| parse_meridian_id(&required.channel))
        .collect::<Vec<_>>();
    if required.is_empty() {
        return None;
    }
    Some(
        required
            .iter()
            .map(|id| meridians.get(*id).integrity.clamp(0.0, 1.0))
            .sum::<f64>()
            / required.len() as f64,
    )
}

#[cfg(test)]
#[path = "burst_meridian_tests.rs"]
mod tests;
