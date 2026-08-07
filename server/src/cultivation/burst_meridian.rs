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
    ApplyStatusEffectIntent, AttackIntent, AttackReach, AttackSource, StatusEffectKind,
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

/// 从权威 `TechniqueRegistry`（单一真源）派生经脉依赖声明（M38）。
///
/// 各招的 `required_meridians` 在 TOML 定义（loader 已验证 channel 可被
/// `parse_meridian_id` 解析、min_health ∈ (0,1]），此处逐条声明，消除「TOML 改了
/// 经脉而声明表仍锁旧常量」的双源发散。channel 解析失败（理论不可达：loader 已
/// 保证可解析）时静默跳过——声明表只防启动期重复声明，运行时门禁由 resolver 自身
/// 从 `definition.required_meridians` 读取（`npc/technique.rs` 同理）。
pub fn declare_meridian_dependencies(
    dependencies: &mut crate::cultivation::meridian::severed::SkillMeridianDependencies,
    techniques: &TechniqueRegistry,
) {
    for skill_id in [
        BENG_QUAN_SKILL_ID,
        TIE_SHAN_KAO_SKILL_ID,
        XUE_BENG_BU_SKILL_ID,
        NI_MAI_HU_TI_SKILL_ID,
    ] {
        let meridian_ids: Vec<MeridianId> = techniques
            .get(skill_id)
            .map(|definition| {
                definition
                    .required_meridians
                    .iter()
                    .filter_map(|required| parse_meridian_id(&required.channel))
                    .collect()
            })
            .unwrap_or_default();
        dependencies.declare(skill_id, meridian_ids);
    }
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
    let reach = f64::from(definition.range);
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
        reach: AttackReach::new(definition.range, 0.0),
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
        .unwrap_or_else(|| whiff_focus_point(world, caster, caster_position, definition.range));
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
    if let Some(severed) = world.get::<MeridianSeveredPermanent>(caster) {
        if let Some((id, _)) = required.iter().find(|(id, _)| severed.is_severed(*id)) {
            return Err(CastRejectReason::MeridianSevered(Some(*id)));
        }
    }
    if required.iter().any(|(id, min_health)| {
        let meridian = meridians.get(*id);
        meridian.opened && meridian.integrity >= *min_health
    }) {
        return Ok(());
    }
    Err(CastRejectReason::MeridianSevered(Some(required[0].0)))
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
mod tests {
    use super::*;
    use valence::prelude::{App, DVec3, Events, Update};

    fn spawn_caster(app: &mut App, realm: Realm, qi_current: f64, position: DVec3) -> Entity {
        let mut meridians = MeridianSystem::default();
        if let Some(registry) = app.world().get_resource::<TechniqueRegistry>() {
            for definition in registry.iter() {
                for required in &definition.required_meridians {
                    let id = parse_meridian_id(&required.channel)
                        .expect("checked-in technique meridian must parse");
                    let meridian = meridians.get_mut(id);
                    meridian.opened = true;
                    meridian.integrity = 1.0;
                    meridian.throughput_current = 1.0;
                }
            }
        }
        app.world_mut()
            .spawn((
                Cultivation {
                    realm,
                    qi_current,
                    qi_max: 100.0,
                    ..Default::default()
                },
                meridians,
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
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CombatClock { tick: 10 });
        app.add_event::<AttackIntent>();
        app.add_event::<BurstMeridianEvent>();
        app.add_event::<VfxEventRequest>();
        app
    }

    fn beng_quan_range(app: &App) -> f64 {
        f64::from(
            app.world()
                .resource::<TechniqueRegistry>()
                .get(BENG_QUAN_SKILL_ID)
                .expect("beng_quan metadata must exist")
                .range,
        )
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
    fn beng_quan_uses_overridden_runtime_metadata() {
        let mut app = app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            BENG_QUAN_SKILL_ID,
            |definition| {
                definition.qi_cost = 0.25;
                definition.range = 2.75;
                definition.cast_ticks = 13;
                definition.cooldown_ticks = 91;
            },
        ));
        let caster = spawn_caster(&mut app, Realm::Induce, 80.0, DVec3::ZERO);
        app.world_mut()
            .get_mut::<MeridianSystem>(caster)
            .unwrap()
            .get_mut(MeridianId::LargeIntestine)
            .integrity = 0.7;
        let target = spawn_target(&mut app, DVec3::new(2.5, 0.0, 0.0));

        let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

        assert_eq!(
            result,
            CastResult::Started {
                cooldown_ticks: 91,
                anim_duration_ticks: 13,
            }
        );
        assert!((qi(&app, caster) - 60.0).abs() < 1e-9);
        assert_eq!(
            app.world().get::<Casting>(caster).unwrap().duration_ticks,
            13
        );
        assert_eq!(
            app.world()
                .resource::<Events<AttackIntent>>()
                .iter_current_update_events()
                .next()
                .unwrap()
                .reach
                .max,
            2.75
        );
    }

    #[test]
    fn beng_quan_uses_overridden_runtime_race_gate_without_mutation() {
        let mut app = app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            BENG_QUAN_SKILL_ID,
            |definition| {
                definition.required_race = crate::body_plan::RaceGateOwned::Species {
                    species: vec![crate::body_plan::RaceId::new("whale")],
                };
            },
        ));
        let caster = spawn_caster(&mut app, Realm::Induce, 80.0, DVec3::ZERO);
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::RaceMismatch,
            }
        );
        assert_no_mutation(&app, caster, 80.0, 1.0);
    }

    #[test]
    fn beng_quan_rejects_when_all_meridians_miss_overridden_health_threshold() {
        let mut app = app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            BENG_QUAN_SKILL_ID,
            |definition| {
                for required in &mut definition.required_meridians {
                    required.min_health = 0.8;
                }
            },
        ));
        let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
        for id in RIGHT_ARM_MERIDIANS {
            app.world_mut()
                .get_mut::<MeridianSystem>(caster)
                .unwrap()
                .get_mut(id)
                .integrity = 0.7;
        }
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

        assert_eq!(
            result,
            rejected(CastRejectReason::MeridianSevered(Some(
                MeridianId::LargeIntestine
            )))
        );
        assert_no_mutation(&app, caster, 100.0, 0.7);
    }

    #[test]
    fn beng_quan_happy_path_mutates_atomically_and_emits_events() {
        let mut app = app();
        let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
        let range = beng_quan_range(&app);
        let target = spawn_target(&mut app, DVec3::new(range, 0.0, 0.0));

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
    fn beng_quan_rejects_out_of_range_target_without_mutation() {
        let mut app = app();
        let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
        let range = beng_quan_range(&app);
        let target = spawn_target(&mut app, DVec3::new(range + 0.01, 0.0, 0.0));

        // 锁定超距目标硬轰仍是"目标无效"的正确语义（有目标时距离门保留）。
        assert_eq!(
            resolve_beng_quan(app.world_mut(), caster, 0, Some(target)),
            rejected(CastRejectReason::InvalidTarget)
        );
        assert_no_mutation(&app, caster, 100.0, 1.0);
    }

    #[test]
    fn beng_quan_whiffs_without_target_spending_cost() {
        // Option B 去目标门禁（对齐 sword_basics 劈/刺）：无目标 = 空挥，
        // Started + 照常扣真元撕脉 + AttackIntent/BurstMeridianEvent target=None。
        let mut app = app();
        let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);

        let result = resolve_beng_quan(app.world_mut(), caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "无目标崩拳应空挥 Started（不再报 InvalidTarget），实际 {result:?}"
        );
        let qi = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        assert!(
            qi < 100.0,
            "空挥必须照常扣真元（防无目标白嫖挥拳），实际 qi={qi}"
        );
        let attack_events = app.world().resource::<Events<AttackIntent>>();
        let attack = attack_events
            .iter_current_update_events()
            .next()
            .expect("空挥仍应发 AttackIntent（target=None 由 combat resolver 跳过命中）");
        assert_eq!(attack.target, None);
        assert_eq!(attack.source, AttackSource::BurstMeridian);
        let burst_events = app.world().resource::<Events<BurstMeridianEvent>>();
        assert_eq!(
            burst_events
                .iter_current_update_events()
                .next()
                .expect("空挥仍应发 BurstMeridianEvent（AV/proto 桥照走）")
                .target,
            None
        );
        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        assert!(
            vfx_events.iter_current_update_events().count() > 0,
            "空挥应照常出动画/粒子（whiff_focus_point 沿朝向兜底）"
        );
    }

    #[test]
    fn beng_quan_whiff_particle_direction_follows_caster_look() {
        // CR #835：覆盖 whiff_focus_point 的 Look 分支——空挥粒子方向必须沿
        // 施法者视线（yaw=-90 => 朝东 +X），而非无 Look 时的 +Z 兜底。
        let mut app = app();
        let caster = spawn_caster_with_look(&mut app, Realm::Induce, 100.0, DVec3::ZERO, -90.0);

        let result = resolve_beng_quan(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));

        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        let direction = vfx_events
            .iter_current_update_events()
            .find_map(|event| match &event.payload {
                VfxEventPayloadV1::SpawnParticle { direction, .. } => *direction,
                _ => None,
            })
            .expect("空挥应发带方向的 SpawnParticle");
        assert!(
            direction[0] > 0.9 && direction[1].abs() < 1e-3 && direction[2].abs() < 1e-3,
            "yaw=-90 的空挥粒子方向应沿视线朝 +X（whiff_focus_point Look 分支），\
             实际 direction={direction:?}"
        );
    }

    #[test]
    fn beng_quan_rejects_cooldown_before_mutation() {
        let mut app = app();
        let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
        app.world_mut()
            .get_mut::<SkillBarBindings>(caster)
            .unwrap()
            .set_cooldown(BENG_QUAN_SKILL_ID, 11);
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
    fn tie_shan_kao_uses_overridden_runtime_metadata() {
        let mut app = full_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            TIE_SHAN_KAO_SKILL_ID,
            |definition| {
                definition.required_realm = "Awaken".to_string();
                definition.qi_cost = 7.5;
                definition.range = 4.0;
                definition.cast_ticks = 13;
                definition.cooldown_ticks = 91;
            },
        ));
        let caster = spawn_caster(&mut app, Realm::Awaken, 100.0, DVec3::ZERO);
        let target = spawn_target(&mut app, DVec3::new(3.5, 0.0, 0.0));

        let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

        assert_eq!(
            result,
            CastResult::Started {
                cooldown_ticks: 91,
                anim_duration_ticks: 13,
            }
        );
        assert!((qi(&app, caster) - 92.5).abs() < 1e-9);
        assert_eq!(
            app.world().get::<Casting>(caster).unwrap().duration_ticks,
            13
        );
        assert_eq!(
            app.world()
                .resource::<Events<AttackIntent>>()
                .iter_current_update_events()
                .next()
                .unwrap()
                .reach
                .max,
            4.0
        );
    }

    #[test]
    fn tie_shan_kao_uses_overridden_runtime_race_gate_without_mutation() {
        let mut app = full_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            TIE_SHAN_KAO_SKILL_ID,
            |definition| {
                definition.required_race = crate::body_plan::RaceGateOwned::Species {
                    species: vec![crate::body_plan::RaceId::new("whale")],
                };
            },
        ));
        let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

        assert_eq!(
            result,
            rejected(CastRejectReason::RaceMismatch),
            "M19: race gate must follow definition.required_race override"
        );
        assert_eq!(qi(&app, caster), 100.0, "qi must be untouched on rejection");
        assert_eq!(
            meridian_integrity(&app, caster, MeridianId::Stomach),
            1.0,
            "Stomach must be untouched on rejection"
        );
        assert!(app.world().get::<Casting>(caster).is_none());
        assert!(app
            .world()
            .resource::<Events<BurstMeridianEvent>>()
            .is_empty());
    }

    #[test]
    fn tie_shan_kao_tears_first_overridden_meridian_not_hard_coded_stomach() {
        let mut app = full_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            TIE_SHAN_KAO_SKILL_ID,
            |definition| {
                definition.required_meridians = vec![
                    TechniqueRequiredMeridian {
                        channel: "Lung".to_string(),
                        min_health: 0.5,
                    },
                    TechniqueRequiredMeridian {
                        channel: "Stomach".to_string(),
                        min_health: 0.5,
                    },
                ];
            },
        ));
        let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

        assert_eq!(
            result,
            CastResult::Started {
                cooldown_ticks: 70,
                anim_duration_ticks: 10,
            }
        );
        assert_eq!(qi(&app, caster), 65.0, "100 - 35 flat qi_cost");
        assert!(
            (meridian_integrity(&app, caster, MeridianId::Lung) - 0.8).abs() < 1e-12,
            "M19: torn meridian must follow the FIRST registry required_meridian (Lung -> 0.8)"
        );
        assert_eq!(
            meridian_integrity(&app, caster, MeridianId::Stomach),
            1.0,
            "Stomach must NOT be torn even though present in required_meridians"
        );
    }

    #[test]
    fn tie_shan_kao_rejects_closed_overridden_meridian_without_mutation() {
        let mut app = full_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            TIE_SHAN_KAO_SKILL_ID,
            |definition| {
                definition.required_meridians = vec![TechniqueRequiredMeridian {
                    channel: "Lung".to_string(),
                    min_health: 0.5,
                }];
            },
        ));
        let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
        app.world_mut()
            .get_mut::<MeridianSystem>(caster)
            .unwrap()
            .get_mut(MeridianId::Lung)
            .opened = false;
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

        assert_eq!(
            result,
            rejected(CastRejectReason::MeridianSevered(Some(MeridianId::Lung))),
            "M19: meridian gate must report the registry-overridden meridian, not the legacy default"
        );
        assert_eq!(qi(&app, caster), 100.0, "qi must be untouched on rejection");
        assert_eq!(
            meridian_integrity(&app, caster, MeridianId::Lung),
            1.0,
            "Lung must be untouched on rejection"
        );
        assert!(app.world().get::<Casting>(caster).is_none());
        assert!(app
            .world()
            .resource::<Events<BurstMeridianEvent>>()
            .is_empty());
    }

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
    fn tie_shan_kao_rejects_out_of_range_and_whiffs_without_target() {
        let mut app = full_app();
        let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
        // 超 reach.max (1.5) → InvalidTarget（有目标时距离门保留）。
        let range = f64::from(
            app.world()
                .resource::<TechniqueRegistry>()
                .get(TIE_SHAN_KAO_SKILL_ID)
                .expect("tie_shan_kao metadata must exist")
                .range,
        );
        let far = spawn_target(&mut app, DVec3::new(range + 0.5, 0.0, 0.0));
        assert_eq!(
            resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(far)),
            rejected(CastRejectReason::InvalidTarget)
        );
        assert_single_meridian_untouched(&app, caster, MeridianId::Stomach, 100.0);

        // Option B：无目标 = 空撞，Started + 扣费撕脉 + 事件 target=None。
        let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "无目标贴山靠应空撞 Started（不再报 InvalidTarget），实际 {result:?}"
        );
        let qi = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        assert!(qi < 100.0, "空撞必须照常扣真元，实际 qi={qi}");
        let attack_events = app.world().resource::<Events<AttackIntent>>();
        assert_eq!(
            attack_events
                .iter_current_update_events()
                .next()
                .expect("空撞仍应发 AttackIntent（target=None）")
                .target,
            None
        );
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
            .set_cooldown(TIE_SHAN_KAO_SKILL_ID, 11);
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

        let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

        assert_eq!(result, rejected(CastRejectReason::OnCooldown));
        assert_single_meridian_untouched(&app, caster, MeridianId::Stomach, 100.0);
    }

    // ─── 血崩步（xue_beng_bu）─ Condense / Gallbladder≥0.4 / qi 25 / dash 4 ─────────

    #[test]
    fn xue_beng_bu_uses_overridden_runtime_metadata() {
        let mut app = full_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            XUE_BENG_BU_SKILL_ID,
            |definition| {
                definition.required_realm = "Awaken".to_string();
                definition.qi_cost = 6.25;
                definition.range = 7.5;
                definition.cast_ticks = 11;
                definition.cooldown_ticks = 89;
                definition.stamina_cost = 3.0;
            },
        ));
        let caster = spawn_caster_with_look(&mut app, Realm::Awaken, 100.0, DVec3::ZERO, 0.0);
        app.world_mut().entity_mut(caster).insert(Stamina {
            current: 20.0,
            max: 20.0,
            recover_per_sec: 1.0,
            last_drain_tick: None,
            state: StaminaState::Idle,
        });

        let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            CastResult::Started {
                cooldown_ticks: 89,
                anim_duration_ticks: 11,
            }
        );
        assert!((qi(&app, caster) - 93.75).abs() < 1e-9);
        // M31：stamina_cost=3.0 已扣除（20 → 17），并打了 last_drain_tick。
        let stamina = app
            .world()
            .get::<crate::combat::components::Stamina>(caster)
            .unwrap();
        assert_eq!(stamina.current, 17.0, "M31: 必须扣 definition.stamina_cost");
        assert_eq!(stamina.last_drain_tick, Some(10), "M31: 必须打 drain tick");
        let position = app.world().get::<Position>(caster).unwrap().get();
        assert!((position.z - 7.5).abs() < 1e-9 && position.x.abs() < 1e-9);
    }

    #[test]
    fn xue_beng_bu_uses_overridden_runtime_race_gate_without_mutation() {
        let mut app = full_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            XUE_BENG_BU_SKILL_ID,
            |definition| {
                definition.required_race = crate::body_plan::RaceGateOwned::Species {
                    species: vec![crate::body_plan::RaceId::new("whale")],
                };
            },
        ));
        let caster = spawn_caster_with_look(&mut app, Realm::Condense, 100.0, DVec3::ZERO, 0.0);

        let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            rejected(CastRejectReason::RaceMismatch),
            "M19: race gate must follow definition.required_race override"
        );
        assert_eq!(qi(&app, caster), 100.0, "qi must be untouched on rejection");
        assert_eq!(
            meridian_integrity(&app, caster, MeridianId::Gallbladder),
            1.0,
            "GallBladder must be untouched on rejection"
        );
        assert_eq!(
            app.world().get::<Position>(caster).unwrap().get(),
            DVec3::ZERO,
            "no dash on rejection"
        );
        assert!(app.world().get::<Casting>(caster).is_none());
        assert!(app
            .world()
            .resource::<Events<BurstMeridianEvent>>()
            .is_empty());
    }

    #[test]
    fn xue_beng_bu_tears_first_overridden_meridian_not_hard_coded_gallbladder() {
        let mut app = full_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            XUE_BENG_BU_SKILL_ID,
            |definition| {
                definition.required_meridians = vec![
                    TechniqueRequiredMeridian {
                        channel: "Lung".to_string(),
                        min_health: 0.4,
                    },
                    TechniqueRequiredMeridian {
                        channel: "GallBladder".to_string(),
                        min_health: 0.4,
                    },
                ];
            },
        ));
        let caster = spawn_caster_with_look(&mut app, Realm::Condense, 100.0, DVec3::ZERO, 0.0);

        let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            CastResult::Started {
                cooldown_ticks: 50,
                anim_duration_ticks: 6,
            }
        );
        assert_eq!(qi(&app, caster), 75.0, "100 - 25 flat qi_cost");
        assert!(
            (meridian_integrity(&app, caster, MeridianId::Lung) - 0.75).abs() < 1e-12,
            "M19: torn meridian must follow the FIRST registry required_meridian (Lung -> 0.75)"
        );
        assert_eq!(
            meridian_integrity(&app, caster, MeridianId::Gallbladder),
            1.0,
            "GallBladder must NOT be torn even though present in required_meridians"
        );
    }

    #[test]
    fn xue_beng_bu_rejects_closed_overridden_meridian_without_mutation() {
        let mut app = full_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            XUE_BENG_BU_SKILL_ID,
            |definition| {
                definition.required_meridians = vec![TechniqueRequiredMeridian {
                    channel: "Lung".to_string(),
                    min_health: 0.4,
                }];
            },
        ));
        let caster = spawn_caster_with_look(&mut app, Realm::Condense, 100.0, DVec3::ZERO, 0.0);
        app.world_mut()
            .get_mut::<MeridianSystem>(caster)
            .unwrap()
            .get_mut(MeridianId::Lung)
            .opened = false;

        let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            rejected(CastRejectReason::MeridianSevered(Some(MeridianId::Lung))),
            "M19: meridian gate must report the registry-overridden meridian, not the legacy default"
        );
        assert_eq!(qi(&app, caster), 100.0, "qi must be untouched on rejection");
        assert_eq!(
            meridian_integrity(&app, caster, MeridianId::Lung),
            1.0,
            "Lung must be untouched on rejection"
        );
        assert_eq!(
            app.world().get::<Position>(caster).unwrap().get(),
            DVec3::ZERO,
            "no dash on rejection"
        );
        assert!(app.world().get::<Casting>(caster).is_none());
        assert!(app
            .world()
            .resource::<Events<BurstMeridianEvent>>()
            .is_empty());
    }

    #[test]
    fn xue_beng_bu_exact_stamina_cost_enters_exhausted() {
        let mut app = full_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            XUE_BENG_BU_SKILL_ID,
            |definition| {
                definition.required_realm = "Awaken".to_string();
                definition.qi_cost = 6.25;
                definition.range = 7.5;
                definition.stamina_cost = 3.0;
            },
        ));
        let caster = spawn_caster_with_look(&mut app, Realm::Awaken, 100.0, DVec3::ZERO, 0.0);
        app.world_mut().entity_mut(caster).insert(Stamina {
            current: 3.0,
            max: 20.0,
            recover_per_sec: 1.0,
            last_drain_tick: None,
            state: StaminaState::Idle,
        });

        let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

        assert!(matches!(result, CastResult::Started { .. }));
        let stamina = app.world().get::<Stamina>(caster).unwrap();
        assert_eq!(stamina.current, 0.0);
        assert_eq!(
            stamina.state,
            StaminaState::Exhausted,
            "爆脉施法扣空体力后必须进入 Exhausted，不能保留 Idle"
        );
        assert_eq!(stamina.last_drain_tick, Some(10));
    }

    #[test]
    fn xue_beng_bu_rejects_when_stamina_insufficient_without_displacement() {
        // M31 负向：stamina_cost 高于当前体力 → 拒绝且零副作用（不位移/不扣 qi/不撕裂）。
        let mut app = full_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            XUE_BENG_BU_SKILL_ID,
            |definition| {
                definition.required_realm = "Awaken".to_string();
                definition.qi_cost = 6.25;
                definition.range = 7.5;
                definition.stamina_cost = 3.0;
            },
        ));
        let caster = spawn_caster_with_look(&mut app, Realm::Awaken, 100.0, DVec3::ZERO, 0.0);
        app.world_mut().entity_mut(caster).insert(Stamina {
            current: 2.0,
            max: 20.0,
            recover_per_sec: 1.0,
            last_drain_tick: None,
            state: StaminaState::Idle,
        });

        let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

        assert_eq!(result, rejected(CastRejectReason::InRecovery));
        assert_eq!(qi(&app, caster), 100.0, "qi must be untouched on rejection");
        let position = app.world().get::<Position>(caster).unwrap().get();
        assert_eq!(position, DVec3::ZERO, "no displacement on rejection");
        let stamina = app.world().get::<Stamina>(caster).unwrap();
        assert_eq!(
            stamina.current, 2.0,
            "stamina must be untouched on rejection"
        );
    }

    #[test]
    fn xue_beng_bu_zero_stamina_cost_casts_even_when_exhausted() {
        // M33：valid zero-cost metadata（stamina_cost = 0.0）不能被体力 gate 当
        // insufficient 拒绝——Exhausted 状态下零成本招仍可施放。与 qi gate 的
        // 零成本语义（`cost <= EPSILON` 放行）对称。
        let mut app = full_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            XUE_BENG_BU_SKILL_ID,
            |definition| {
                definition.required_realm = "Awaken".to_string();
                definition.qi_cost = 6.25;
                definition.range = 7.5;
                definition.stamina_cost = 0.0;
            },
        ));
        let caster = spawn_caster_with_look(&mut app, Realm::Awaken, 100.0, DVec3::ZERO, 0.0);
        // Exhausted + 0 体力——若 stamina gate 错误拦零成本，这里会 InRecovery。
        app.world_mut().entity_mut(caster).insert(Stamina {
            current: 0.0,
            max: 20.0,
            recover_per_sec: 1.0,
            last_drain_tick: Some(5),
            state: StaminaState::Exhausted,
        });

        let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            CastResult::Started {
                cooldown_ticks: 50,
                anim_duration_ticks: 6,
            },
            "M33: zero-cost stamina must not block casting"
        );
        assert!((qi(&app, caster) - 93.75).abs() < 1e-9);
    }

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
        let expected_range = f64::from(
            app.world()
                .resource::<TechniqueRegistry>()
                .get(XUE_BENG_BU_SKILL_ID)
                .expect("xue_beng_bu metadata must exist")
                .range,
        );
        assert!(
            (pos.z - expected_range).abs() < 1e-9 && pos.x.abs() < 1e-9,
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
            .set_cooldown(XUE_BENG_BU_SKILL_ID, 11);

        let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

        assert_eq!(result, rejected(CastRejectReason::OnCooldown));
        let pos = app.world().get::<Position>(caster).unwrap().get();
        assert_eq!(pos, DVec3::ZERO);
        assert_single_meridian_untouched(&app, caster, MeridianId::Gallbladder, 100.0);
    }

    // ─── 逆脉护体（ni_mai_hu_ti）─ Solidify / Pericardium≥0.55 / qi 45 / cd 120 ─────

    #[test]
    fn ni_mai_hu_ti_uses_overridden_runtime_metadata() {
        let mut app = full_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            NI_MAI_HU_TI_SKILL_ID,
            |definition| {
                definition.required_realm = "Awaken".to_string();
                definition.qi_cost = 8.5;
                definition.cast_ticks = 15;
                definition.cooldown_ticks = 87;
            },
        ));
        let caster = spawn_caster(&mut app, Realm::Awaken, 100.0, DVec3::ZERO);

        let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            CastResult::Started {
                cooldown_ticks: 87,
                anim_duration_ticks: 15,
            }
        );
        assert!((qi(&app, caster) - 91.5).abs() < 1e-9);
        assert_eq!(
            app.world().get::<Casting>(caster).unwrap().duration_ticks,
            15
        );
    }

    #[test]
    fn ni_mai_hu_ti_uses_overridden_runtime_race_gate_without_mutation() {
        let mut app = full_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            NI_MAI_HU_TI_SKILL_ID,
            |definition| {
                definition.required_race = crate::body_plan::RaceGateOwned::Species {
                    species: vec![crate::body_plan::RaceId::new("whale")],
                };
            },
        ));
        let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);

        let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            rejected(CastRejectReason::RaceMismatch),
            "M19: race gate must follow definition.required_race override"
        );
        assert_eq!(qi(&app, caster), 100.0, "qi must be untouched on rejection");
        assert_eq!(
            meridian_integrity(&app, caster, MeridianId::Pericardium),
            1.0,
            "Pericardium must be untouched on rejection"
        );
        assert!(app.world().get::<Casting>(caster).is_none());
        assert!(app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .is_empty());
        assert!(app
            .world()
            .resource::<Events<BurstMeridianEvent>>()
            .is_empty());
    }

    #[test]
    fn ni_mai_hu_ti_tears_first_overridden_meridian_not_hard_coded_pericardium() {
        let mut app = full_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            NI_MAI_HU_TI_SKILL_ID,
            |definition| {
                definition.required_meridians = vec![
                    TechniqueRequiredMeridian {
                        channel: "Lung".to_string(),
                        min_health: 0.55,
                    },
                    TechniqueRequiredMeridian {
                        channel: "Pericardium".to_string(),
                        min_health: 0.55,
                    },
                ];
            },
        ));
        let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);

        let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            CastResult::Started {
                cooldown_ticks: 120,
                anim_duration_ticks: 12,
            }
        );
        assert_eq!(qi(&app, caster), 55.0, "100 - 45 flat qi_cost");
        assert!(
            (meridian_integrity(&app, caster, MeridianId::Lung) - 0.7).abs() < 1e-12,
            "M19: torn meridian must follow the FIRST registry required_meridian (Lung -> 0.7)"
        );
        assert_eq!(
            meridian_integrity(&app, caster, MeridianId::Pericardium),
            1.0,
            "Pericardium must NOT be torn even though present in required_meridians"
        );
    }

    #[test]
    fn ni_mai_hu_ti_rejects_closed_overridden_meridian_without_mutation() {
        let mut app = full_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            NI_MAI_HU_TI_SKILL_ID,
            |definition| {
                definition.required_meridians = vec![TechniqueRequiredMeridian {
                    channel: "Lung".to_string(),
                    min_health: 0.55,
                }];
            },
        ));
        let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);
        app.world_mut()
            .get_mut::<MeridianSystem>(caster)
            .unwrap()
            .get_mut(MeridianId::Lung)
            .opened = false;

        let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            rejected(CastRejectReason::MeridianSevered(Some(MeridianId::Lung))),
            "M19: meridian gate must report the registry-overridden meridian, not the legacy default"
        );
        assert_eq!(qi(&app, caster), 100.0, "qi must be untouched on rejection");
        assert_eq!(
            meridian_integrity(&app, caster, MeridianId::Lung),
            1.0,
            "Lung must be untouched on rejection"
        );
        assert!(app.world().get::<Casting>(caster).is_none());
        assert!(app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .is_empty());
        assert!(app
            .world()
            .resource::<Events<BurstMeridianEvent>>()
            .is_empty());
    }

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
            .set_cooldown(NI_MAI_HU_TI_SKILL_ID, 11);

        let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

        assert_eq!(result, rejected(CastRejectReason::OnCooldown));
        assert_single_meridian_untouched(&app, caster, MeridianId::Pericardium, 100.0);
        assert!(app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .is_empty());
    }

    // ─── plan-skill-anim-fidelity-v1 P3：专属动画 id pin（去复用 + 缺失补齐）────────

    /// 收集本次 update 内发出的全部 PlayAnim anim_id。
    fn emitted_play_anim_ids(app: &App) -> Vec<String> {
        app.world()
            .resource::<Events<VfxEventRequest>>()
            .iter_current_update_events()
            .filter_map(|request| match &request.payload {
                VfxEventPayloadV1::PlayAnim { anim_id, .. } => Some(anim_id.clone()),
                _ => None,
            })
            .collect()
    }

    /// 事件路径 pin：三招 resolver 各自发出**专属** PlayAnim——tie_shan_kao 靠身
    /// 撞击 / xue_beng_bu 步法突进解除崩拳借用，ni_mai_hu_ti 从 `anim_id: None`
    /// 补齐护体结印动画；任何一招回退到 `bong:beng_quan`（旧借用）立即撞红。
    /// 崩拳本尊仍合法使用 `bong:beng_quan`，见 beng_quan 专属用例。
    #[test]
    fn p3_bespoke_anim_ids_emitted_and_beng_quan_borrow_removed() {
        let cases: [(&str, &str); 3] = [
            (TIE_SHAN_KAO_SKILL_ID, TIE_SHAN_KAO_ANIM_ID),
            (XUE_BENG_BU_SKILL_ID, XUE_BENG_BU_ANIM_ID),
            (NI_MAI_HU_TI_SKILL_ID, NI_MAI_HU_TI_ANIM_ID),
        ];
        // 常量层面先锁专属 + 互异（id 拼写回退在 resolver 跑起来之前就撞红）。
        assert_eq!(TIE_SHAN_KAO_ANIM_ID, "bong:tie_shan_kao");
        assert_eq!(XUE_BENG_BU_ANIM_ID, "bong:xue_beng_bu");
        assert_eq!(NI_MAI_HU_TI_ANIM_ID, "bong:ni_mai_hu_ti");
        for (skill_id, anim_id) in cases {
            assert_ne!(
                anim_id, BENG_QUAN_ANIM_ID,
                "去复用回归锁：{skill_id} 不得回退借崩拳动画 {BENG_QUAN_ANIM_ID}"
            );
        }

        // 事件路径：tie_shan_kao（Condense，带 UniqueId 才走 PlayAnim 分支）。
        let mut app = full_app();
        let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
        app.world_mut()
            .entity_mut(caster)
            .insert(valence::prelude::UniqueId::default());
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));
        let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));
        assert!(matches!(result, CastResult::Started { .. }));
        let anims = emitted_play_anim_ids(&app);
        assert_eq!(
            anims,
            vec![TIE_SHAN_KAO_ANIM_ID.to_string()],
            "tie_shan_kao 应恰好发一条专属靠撞 PlayAnim，实际 {anims:?}"
        );

        // 事件路径：xue_beng_bu（需 Look 提供朝向）。
        let mut app = full_app();
        let caster = spawn_caster_with_look(&mut app, Realm::Condense, 100.0, DVec3::ZERO, 0.0);
        app.world_mut()
            .entity_mut(caster)
            .insert(valence::prelude::UniqueId::default());
        let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));
        let anims = emitted_play_anim_ids(&app);
        assert_eq!(
            anims,
            vec![XUE_BENG_BU_ANIM_ID.to_string()],
            "xue_beng_bu 应恰好发一条专属步法 PlayAnim，实际 {anims:?}"
        );

        // 事件路径：ni_mai_hu_ti（缺失补齐——此前 anim_id: None 完全不发）。
        let mut app = full_app();
        let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);
        app.world_mut()
            .entity_mut(caster)
            .insert(valence::prelude::UniqueId::default());
        let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));
        let anims = emitted_play_anim_ids(&app);
        assert_eq!(
            anims,
            vec![NI_MAI_HU_TI_ANIM_ID.to_string()],
            "ni_mai_hu_ti 应恰好发一条专属护体结印 PlayAnim（MISSING 缺口已补），实际 {anims:?}"
        );
    }

    /// 无 UniqueId（非玩家/无头）时 PlayAnim 分支静默跳过、粒子照发——
    /// 锁 emit_burst_av 的防御分支不因 P3 接线改变。
    #[test]
    fn p3_ni_mai_hu_ti_without_unique_id_skips_anim_keeps_particle() {
        let mut app = full_app();
        let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);
        let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));
        assert!(
            emitted_play_anim_ids(&app).is_empty(),
            "无 UniqueId 不应发 PlayAnim（防御分支）"
        );
        let particles = app
            .world()
            .resource::<Events<VfxEventRequest>>()
            .iter_current_update_events()
            .filter(|request| matches!(&request.payload, VfxEventPayloadV1::SpawnParticle { .. }))
            .count();
        assert_eq!(particles, 1, "护体粒子环应照发");
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
        let techniques = TechniqueRegistry::load_for_tests();
        declare_meridian_dependencies(&mut deps, &techniques);
        // M38：声明必须派生自权威 registry（TOML required_meridians），
        // 而非本文件锁死的旧常量（双源发散）。
        assert_eq!(
            deps.lookup(TIE_SHAN_KAO_SKILL_ID),
            &[MeridianId::Stomach],
            "tie_shan_kao 声明须来自 registry required_meridians"
        );
        assert_eq!(
            deps.lookup(XUE_BENG_BU_SKILL_ID),
            &[MeridianId::Gallbladder],
            "xue_beng_bu 声明须来自 registry required_meridians"
        );
        assert_eq!(
            deps.lookup(NI_MAI_HU_TI_SKILL_ID),
            &[MeridianId::Pericardium],
            "ni_mai_hu_ti 声明须来自 registry required_meridians"
        );
    }

    #[test]
    fn declare_meridian_dependencies_follows_registry_override() {
        // M38 双源发散回归锁：TOML 改经脉后声明表必须跟随，不能锁旧常量。
        let techniques =
            TechniqueRegistry::load_for_tests_with_override(TIE_SHAN_KAO_SKILL_ID, |definition| {
                definition.required_meridians = vec![TechniqueRequiredMeridian {
                    channel: "Liver".to_string(),
                    min_health: 0.3,
                }];
            });
        let mut deps = crate::cultivation::meridian::severed::SkillMeridianDependencies::default();
        declare_meridian_dependencies(&mut deps, &techniques);
        assert_eq!(
            deps.lookup(TIE_SHAN_KAO_SKILL_ID),
            &[MeridianId::Liver],
            "声明必须跟随 registry 覆盖后的 required_meridians"
        );
    }

    #[test]
    fn flat_qi_cost_reads_from_known_techniques_single_source() {
        // 严禁本文件硬编码重复 qi_cost：值必须来自 checked-in registry。
        let techniques = TechniqueRegistry::load_for_tests();
        assert_eq!(flat_qi_cost(&techniques, TIE_SHAN_KAO_SKILL_ID), Some(35.0));
        assert_eq!(flat_qi_cost(&techniques, XUE_BENG_BU_SKILL_ID), Some(25.0));
        assert_eq!(flat_qi_cost(&techniques, NI_MAI_HU_TI_SKILL_ID), Some(45.0));
        assert_eq!(flat_qi_cost(&techniques, "nonexistent.skill"), None);
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
        app.insert_resource(TechniqueRegistry::load_for_tests());
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
        let range = beng_quan_range(&app);
        let target = spawn_target(&mut app, DVec3::new(0.0, 70.0, range));

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
        let range = beng_quan_range(&app);
        let target = spawn_target(&mut app, DVec3::new(0.0, 70.0, range));

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

    // ─── plan-skill-anim-fidelity-v1 P5：粒子去复用回归锁 ─────────────────────────
    //
    // P3 解除了动画借用,粒子借用留到 P5：三招此前 100% 借崩拳
    // `bong:burst_meridian_beng_quan`。与真脉相反,爆脉三招**共用**识别色 #C58B3F,
    // 读招完全靠形态(GroundDecal 冲击环 / Ribbon 步法残影 / Sprite 体表逆流纹),
    // 所以这里锁的是「id 全异 + 颜色全同」这一对方向相反的性质。

    /// 收集本次 update 内发出的全部 SpawnParticle `(event_id, color)`。
    fn emitted_particles(app: &App) -> Vec<(String, Option<String>)> {
        app.world()
            .resource::<Events<VfxEventRequest>>()
            .iter_current_update_events()
            .filter_map(|request| match &request.payload {
                VfxEventPayloadV1::SpawnParticle {
                    event_id, color, ..
                } => Some((event_id.clone(), color.clone())),
                _ => None,
            })
            .collect()
    }

    /// 逐招对拍共享接线表（`network::skill_vfx_wiring`,client 按同一份表注册）。
    fn assert_emits_wired_particle(app: &App, skill_id: &str) {
        let wiring = crate::network::skill_vfx_wiring::wiring_for(skill_id)
            .unwrap_or_else(|| panic!("{skill_id} 未登记进 P5_SKILL_VFX_WIRING 接线表"));
        let particles = emitted_particles(app);
        assert_eq!(
            particles.len(),
            1,
            "{skill_id} 应恰好发 1 条 SpawnParticle,实际 {particles:?}"
        );
        assert_eq!(
            particles[0].0, wiring.event_id,
            "{skill_id} 发出的粒子 event_id 与接线表不符(client 按表注册,不符即 bridgeMiss 静默无特效)"
        );
        assert_eq!(
            particles[0].1.as_deref(),
            Some(wiring.color),
            "{skill_id} 的粒子颜色应为爆脉家族色 {}",
            wiring.color
        );
        assert_ne!(
            particles[0].0, wiring.legacy_event_id,
            "{skill_id} 回退到了 P5 之前借用的崩拳粒子 `{}`",
            wiring.legacy_event_id
        );
    }

    #[test]
    fn p5_bespoke_particle_ids_emitted_and_beng_quan_particle_borrow_removed() {
        // 常量层面先锁专属 + 互异。
        assert_eq!(TIE_SHAN_KAO_PARTICLE_ID, "bong:burst_meridian_tie_shan_kao");
        assert_eq!(XUE_BENG_BU_PARTICLE_ID, "bong:burst_meridian_xue_beng_bu");
        assert_eq!(NI_MAI_HU_TI_PARTICLE_ID, "bong:burst_meridian_ni_mai_hu_ti");
        for particle_id in [
            TIE_SHAN_KAO_PARTICLE_ID,
            XUE_BENG_BU_PARTICLE_ID,
            NI_MAI_HU_TI_PARTICLE_ID,
        ] {
            assert_ne!(
                particle_id, BENG_QUAN_PARTICLE_ID,
                "去复用回归锁：{particle_id} 不得回退借崩拳粒子 {BENG_QUAN_PARTICLE_ID}"
            );
        }

        // 事件路径：tie_shan_kao。
        let mut app = full_app();
        let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
        let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));
        let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));
        assert!(matches!(result, CastResult::Started { .. }));
        assert_emits_wired_particle(&app, TIE_SHAN_KAO_SKILL_ID);

        // 事件路径：xue_beng_bu(需 Look 提供朝向)。
        let mut app = full_app();
        let caster = spawn_caster_with_look(&mut app, Realm::Condense, 100.0, DVec3::ZERO, 0.0);
        let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));
        assert_emits_wired_particle(&app, XUE_BENG_BU_SKILL_ID);

        // 事件路径：ni_mai_hu_ti。
        let mut app = full_app();
        let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);
        let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));
        assert_emits_wired_particle(&app, NI_MAI_HU_TI_SKILL_ID);
    }

    #[test]
    fn p5_burst_family_shares_one_color_but_never_one_id() {
        // 家族设计：共用识别色 + 形态分化。两个性质都必须成立——
        // 只共用色而 id 也相同 = 回到借用；只 id 不同而颜色发散 = 失去家族识别。
        let ids = [
            BENG_QUAN_PARTICLE_ID,
            TIE_SHAN_KAO_PARTICLE_ID,
            XUE_BENG_BU_PARTICLE_ID,
            NI_MAI_HU_TI_PARTICLE_ID,
        ];
        let unique: std::collections::BTreeSet<&str> = ids.iter().copied().collect();
        assert_eq!(
            unique.len(),
            ids.len(),
            "爆脉四招粒子 id 必须两两不同,实际 {unique:?}"
        );
        for id in ids {
            assert!(
                id.starts_with("bong:burst_meridian_"),
                "{id} 不在 bong:burst_meridian_ 家族前缀下,会掉出 Important 优先级档"
            );
        }
        assert_eq!(
            BURST_MERIDIAN_FAMILY_COLOR, "#C58B3F",
            "爆脉家族识别色被改动——plan §P5.1 ② 指定为 #C58B3F"
        );
    }

    #[test]
    fn p5_rejected_burst_cast_emits_no_particle() {
        // 拒绝路径(真元不足)不得发粒子。
        let mut app = full_app();
        let caster = spawn_caster(&mut app, Realm::Condense, 0.0, DVec3::ZERO);
        let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Rejected { .. }));
        assert!(emitted_particles(&app).is_empty(), "被拒绝的施放不得发粒子");
    }

    // ─── 逆脉护体「体表逆流纹」跟随施法者 ─────────────────────────────────────────
    //
    // 交付物是「体表」逆流纹：环必须贴着身体，而不是钉在施法瞬间的世界坐标上。
    // 由于 `SpawnParticle` payload 无实体标识，跟随靠 server 在 buff 存续期内以施法者
    // 当前 `Position` 周期重发短寿命环实现，下列用例逐个状态转换锁住这条链路。

    /// `app()` 的初始 clock —— 下面所有相位算术都以它为 cast tick。
    const CAST_TICK: u64 = 10;

    /// 真跑 `ni_mai_hu_ti_aura_vfx_tick` 的取证 app。
    fn aura_app() -> App {
        let mut app = full_app();
        app.add_systems(Update, ni_mai_hu_ti_aura_vfx_tick);
        app
    }

    fn clear_vfx_events(app: &mut App) {
        app.world_mut()
            .resource_mut::<Events<VfxEventRequest>>()
            .clear();
    }

    fn move_caster(app: &mut App, caster: Entity, position: DVec3) {
        app.world_mut()
            .entity_mut(caster)
            .insert(Position::new([position.x, position.y, position.z]));
    }

    /// 本帧发出的护体环 payload（已按 event_id 过滤）。
    fn ni_mai_rings(app: &App) -> Vec<VfxEventPayloadV1> {
        app.world()
            .resource::<Events<VfxEventRequest>>()
            .iter_current_update_events()
            .filter(|request| {
                matches!(
                    &request.payload,
                    VfxEventPayloadV1::SpawnParticle { event_id, .. }
                        if event_id == NI_MAI_HU_TI_PARTICLE_ID
                )
            })
            .map(|request| request.payload.clone())
            .collect()
    }

    /// 把 clock 拨到 `tick` 跑一帧，返回本帧发出的护体环。
    fn tick_aura(app: &mut App, tick: u64) -> Vec<VfxEventPayloadV1> {
        app.world_mut().resource_mut::<CombatClock>().tick = tick;
        app.update();
        ni_mai_rings(app)
    }

    fn ring_origin(ring: &VfxEventPayloadV1) -> [f64; 3] {
        match ring {
            VfxEventPayloadV1::SpawnParticle { origin, .. } => *origin,
            other => panic!("护体环应为 SpawnParticle，实际 {other:?}"),
        }
    }

    fn ring_duration(ring: &VfxEventPayloadV1) -> Option<u16> {
        match ring {
            VfxEventPayloadV1::SpawnParticle { duration_ticks, .. } => *duration_ticks,
            other => panic!("护体环应为 SpawnParticle，实际 {other:?}"),
        }
    }

    fn cast_ni_mai_hu_ti(app: &mut App, position: DVec3) -> Entity {
        let caster = spawn_caster(app, Realm::Solidify, 100.0, position);
        let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "护体施放前置不满足，用例本身失效：{result:?}"
        );
        caster
    }

    /// 施放即装上锚点，窗口 = cast tick + 60t（buff 常量），供重发系统判存续与到期。
    #[test]
    fn p5_ni_mai_hu_ti_cast_installs_aura_anchor_matching_buff_window() {
        let mut app = aura_app();
        let caster = cast_ni_mai_hu_ti(&mut app, DVec3::ZERO);

        let aura = app
            .world()
            .get::<NiMaiHuTiAura>(caster)
            .copied()
            .expect("施放后必须挂上 NiMaiHuTiAura 锚点，否则重发系统永远找不到施法者");
        assert_eq!(aura.started_at_tick, CAST_TICK, "重发相位基准应为施法 tick");
        assert_eq!(
            aura.expires_at_tick,
            CAST_TICK + NI_MAI_HU_TI_BUFF_DURATION_TICKS,
            "锚点窗口必须与减伤 buff 时长严格同步——视觉窗口比 buff 长=护体已过纹还在转，\
             短=窗口后段裸奔无反馈"
        );
    }

    /// 首环不再用 60t 长寿命撑满窗口（那正是纹脱离身体的根因）。
    #[test]
    fn p5_ni_mai_hu_ti_cast_ring_lives_one_reemit_interval_not_whole_buff() {
        let mut app = aura_app();
        cast_ni_mai_hu_ti(&mut app, DVec3::ZERO);

        let rings = ni_mai_rings(&app);
        assert_eq!(rings.len(), 1, "施放应恰好发 1 个首环，实际 {rings:?}");
        assert_eq!(
            ring_duration(&rings[0]),
            Some(NI_MAI_HU_TI_AURA_PARTICLE_LIFETIME_TICKS),
            "首环寿命应为一个重发间隔（{NI_MAI_HU_TI_AURA_PARTICLE_LIFETIME_TICKS}t）；\
             若回到整个 buff 窗口 {NI_MAI_HU_TI_BUFF_DURATION_TICKS}t，环就会钉在施法瞬间的\
             坐标上，玩家一移动纹即脱离体表"
        );
        assert!(
            u64::from(NI_MAI_HU_TI_AURA_PARTICLE_LIFETIME_TICKS) < NI_MAI_HU_TI_BUFF_DURATION_TICKS,
            "单环寿命必须短于 buff 窗口，否则无从接力跟随"
        );
    }

    /// **核心回归锁**：窗口内施法者一路移动，每个重发环的圆心都必须落在其当时的位置。
    #[test]
    fn p5_ni_mai_hu_ti_aura_ring_follows_moving_caster() {
        let mut app = aura_app();
        let caster = cast_ni_mai_hu_ti(&mut app, DVec3::ZERO);
        clear_vfx_events(&mut app);

        // 相位落在 cast tick + 12 的整数倍上。
        let waypoints = [
            (CAST_TICK + 12, DVec3::new(3.0, 0.0, 0.0)),
            (CAST_TICK + 24, DVec3::new(3.0, 0.0, 5.0)),
            (CAST_TICK + 36, DVec3::new(-2.0, 1.0, 5.0)),
            (CAST_TICK + 48, DVec3::new(-2.0, 1.0, 9.0)),
        ];
        for (tick, position) in waypoints {
            move_caster(&mut app, caster, position);
            let rings = tick_aura(&mut app, tick);
            assert_eq!(
                rings.len(),
                1,
                "tick {tick} 应恰好重发 1 个护体环，实际 {rings:?}"
            );
            assert_eq!(
                ring_origin(&rings[0]),
                [
                    position.x,
                    position.y + BURST_AV_PARTICLE_Y_LIFT,
                    position.z
                ],
                "护体环圆心必须锚在施法者 tick {tick} 的**当前**位置（plan §P5.1 ② 「体表」\
                 逆流纹是字面交付物），而不是 cast 瞬间的 origin"
            );
            clear_vfx_events(&mut app);
        }
    }

    /// 重发相位固定，且「首环 + 各重发环」严丝合缝铺满 buff 窗口：不叠环、不留空窗。
    #[test]
    fn p5_ni_mai_hu_ti_aura_cadence_tiles_buff_window_exactly() {
        let mut app = aura_app();
        cast_ni_mai_hu_ti(&mut app, DVec3::ZERO);
        clear_vfx_events(&mut app);

        let mut emitted_at = Vec::new();
        for tick in CAST_TICK..=CAST_TICK + NI_MAI_HU_TI_BUFF_DURATION_TICKS + 20 {
            if !tick_aura(&mut app, tick).is_empty() {
                emitted_at.push(tick);
            }
            clear_vfx_events(&mut app);
        }

        assert_eq!(
            emitted_at,
            vec![
                CAST_TICK + 12,
                CAST_TICK + 24,
                CAST_TICK + 36,
                CAST_TICK + 48
            ],
            "重发相位应严格是 cast tick 之后每 {NI_MAI_HU_TI_AURA_REEMIT_INTERVAL_TICKS}t 一次\
             且不越过 buff 到期，实际 {emitted_at:?}"
        );
        assert_eq!(
            (emitted_at.len() as u64 + 1) * NI_MAI_HU_TI_AURA_REEMIT_INTERVAL_TICKS,
            NI_MAI_HU_TI_BUFF_DURATION_TICKS,
            "首环 + {} 次重发，每环活 {}t，应严格铺满 {}t buff 窗口：\
             不足则窗口中途断纹，超出则护体已过而纹还在转",
            emitted_at.len(),
            NI_MAI_HU_TI_AURA_REEMIT_INTERVAL_TICKS,
            NI_MAI_HU_TI_BUFF_DURATION_TICKS
        );
    }

    /// cast 同帧不得补环（`emit_burst_av` 已发首环），非相位 tick 同样静默。
    #[test]
    fn p5_ni_mai_hu_ti_aura_stays_silent_on_cast_tick_and_off_phase_ticks() {
        let mut app = aura_app();
        cast_ni_mai_hu_ti(&mut app, DVec3::ZERO);
        clear_vfx_events(&mut app);

        assert!(
            tick_aura(&mut app, CAST_TICK).is_empty(),
            "cast 同帧系统不得再补一环——emit_burst_av 已发首环，重复即叠成双圈"
        );
        clear_vfx_events(&mut app);
        for offset in [1_u64, 5, 11, 13, 23, 47, 59] {
            let tick = CAST_TICK + offset;
            assert!(
                tick_aura(&mut app, tick).is_empty(),
                "非重发相位 tick {tick}（cast 后第 {offset} tick）不得发环"
            );
            clear_vfx_events(&mut app);
        }
    }

    /// buff 到期 → 摘锚点 + 永久停发（否则纹会一直转下去）。
    #[test]
    fn p5_ni_mai_hu_ti_aura_anchor_removed_and_silent_after_buff_expiry() {
        let mut app = aura_app();
        let caster = cast_ni_mai_hu_ti(&mut app, DVec3::ZERO);
        clear_vfx_events(&mut app);

        let expiry = CAST_TICK + NI_MAI_HU_TI_BUFF_DURATION_TICKS;
        for tick in CAST_TICK..=expiry {
            tick_aura(&mut app, tick);
            clear_vfx_events(&mut app);
        }

        assert!(
            app.world().get::<NiMaiHuTiAura>(caster).is_none(),
            "buff 到期（tick {expiry}）后必须摘除锚点，否则重发会永久持续"
        );
        for tick in expiry + 1..=expiry + 60 {
            assert!(
                tick_aura(&mut app, tick).is_empty(),
                "buff 到期后 tick {tick} 仍在发护体环"
            );
            clear_vfx_events(&mut app);
        }
    }

    /// 首环与重发环除 origin 外逐字段同形 —— 两条发射路径发散会让窗口中途的环突然换模样。
    #[test]
    fn p5_ni_mai_hu_ti_cast_ring_and_reemit_ring_share_one_form_spec() {
        let mut app = aura_app();
        let caster = cast_ni_mai_hu_ti(&mut app, DVec3::ZERO);
        let cast_ring = ni_mai_rings(&app).into_iter().next().expect("施放应发首环");
        clear_vfx_events(&mut app);

        let moved = DVec3::new(7.0, 2.0, -4.0);
        move_caster(&mut app, caster, moved);
        let reemit_ring = tick_aura(&mut app, CAST_TICK + 12)
            .into_iter()
            .next()
            .expect("存续期应重发护体环");

        assert_ne!(
            ring_origin(&cast_ring),
            ring_origin(&reemit_ring),
            "施法者已移动，重发环必须换到新位置"
        );

        // 把首环的 origin 换成重发环的，其余字段应完全相等。
        let VfxEventPayloadV1::SpawnParticle {
            event_id,
            direction,
            color,
            strength,
            count,
            duration_ticks,
            ..
        } = cast_ring.clone()
        else {
            panic!("首环应为 SpawnParticle");
        };
        let normalized = VfxEventPayloadV1::SpawnParticle {
            event_id,
            origin: ring_origin(&reemit_ring),
            direction,
            color,
            strength,
            count,
            duration_ticks,
        };
        assert_eq!(
            normalized, reemit_ring,
            "首环与重发环只允许 origin 不同；其余字段（颜色/强度/颗数/寿命）发散会让\
             护体窗口中途的环突然换个模样"
        );
    }

    /// 冷却后重新施放 → 窗口整体后移，锚点被覆盖而不是叠出第二套环。
    #[test]
    fn p5_ni_mai_hu_ti_recast_resets_aura_window_without_stacking() {
        let mut app = aura_app();
        let caster = cast_ni_mai_hu_ti(&mut app, DVec3::ZERO);
        clear_vfx_events(&mut app);

        // 冷却 120t 之后再来一发。
        let recast_tick = CAST_TICK + 200;
        app.world_mut().resource_mut::<CombatClock>().tick = recast_tick;
        let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "冷却已过应可重放：{result:?}"
        );
        clear_vfx_events(&mut app);

        let aura = app
            .world()
            .get::<NiMaiHuTiAura>(caster)
            .copied()
            .expect("重放后锚点仍在");
        assert_eq!(
            (aura.started_at_tick, aura.expires_at_tick),
            (recast_tick, recast_tick + NI_MAI_HU_TI_BUFF_DURATION_TICKS),
            "重放应把窗口整体后移（insert 覆盖语义），而不是保留旧窗口"
        );

        let rings = tick_aura(&mut app, recast_tick + 12);
        assert_eq!(
            rings.len(),
            1,
            "重放后每个相位 tick 仍只发 1 个环，不得叠出两套，实际 {rings:?}"
        );
    }
}
