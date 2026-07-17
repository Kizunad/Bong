//! Gameplay domain events -> `play_anim` adapters for `bong:vfx_event`.
//!
//! The transport, JSON serialization, radius filtering and client delivery stay in
//! `vfx_event_emit`; this module only decides which first-party animation id should
//! represent an already-authoritative server event.

use std::collections::{HashMap, HashSet};

use valence::prelude::{Entity, EventReader, EventWriter, Local, Position, Query, Res, UniqueId};

use crate::botany::components::HarvestTerminalEvent;
use crate::botany::lifecycle::botany_quality_color;
use crate::combat::anqi_v2::{
    AnqiSkillId, ArmorPierceEvent, EchoFractalEvent, MultiShotEvent, QiInjectionEvent,
};
use crate::combat::baomai_v3::{BaomaiSkillEvent, BaomaiSkillId};
use crate::combat::body_conditioning::GuangboTicaoPracticeEvent;
use crate::combat::carrier::CarrierChargedEvent;
use crate::combat::components::WoundKind;
use crate::combat::dugu_v2::events::{
    EclipseNeedleEvent, PenetrateChainEvent, ReverseTriggeredEvent, SelfCureProgressEvent,
    ShroudActivatedEvent,
};
use crate::combat::events::{AttackIntent, AttackSource, CombatEvent, DefenseIntent};
use crate::combat::needle::QiNeedleChargedEvent;
use crate::combat::tuike_v2::{ContamTransferredEvent, DonFalseSkinEvent, FalseSkinSheddedEvent};
use crate::combat::weapon::Weapon;
use crate::combat::woliu::{VortexBackfireEvent, VortexField};
use crate::combat::woliu_v2::state::VortexV2State;
use crate::combat::woliu_v2::{VortexCastEvent, WoliuSkillId};
use crate::combat::CombatClock;
use crate::cultivation::breakthrough::BreakthroughOutcome;
use crate::cultivation::dugu::DuguObfuscationDisruptedEvent;
use crate::cultivation::technique_scroll::TechniqueLearnedEvent;
use crate::cultivation::tribulation::{TribulationAnnounce, TribulationFailed, TribulationSettled};
use crate::forge::events::TemperingHit;
use crate::forge::session::{ForgeSessions, ForgeStep};
use crate::lingtian::events::{
    DrainQiCompleted, HarvestCompleted, PlantingCompleted, ReplenishCompleted, TillCompleted,
};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::tribulation::DuXuOutcomeV1;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::sword_path::av_event::{SwordPathSkillCastEvent, SwordPathSkillId};

const ANIM_SWORD_SLASH_DOWN: &str = "bong:sword_slash_down";
const ANIM_SWORD_STAB: &str = "bong:sword_stab";
const ANIM_SWORD_CLEAVE: &str = "bong:sword_cleave";
const ANIM_SWORD_THRUST: &str = "bong:sword_thrust";
const ANIM_FIST_PUNCH_RIGHT: &str = "bong:fist_punch_right";
/// plan-skill-av-relink-v1 P1 — 空手连击左拳（与 fist_punch_right 交替，见
/// `next_fist_punch_anim`；持械分支不参与交替恒右拳）。
const ANIM_FIST_PUNCH_LEFT: &str = "bong:fist_punch_left";
const ANIM_PALM_STRIKE: &str = "bong:palm_strike";
const ANIM_PARRY_BLOCK: &str = "bong:parry_block";
const ANIM_GUARD_RAISE: &str = "bong:guard_raise";
/// plan-shield-block-v1 P1 — 持续举盾动画（isLoop:true）。与 guard_raise (FullPowerCharge) 无关。
pub const ANIM_SHIELD_RAISE: &str = "bong:shield_raise";
const ANIM_HURT_STAGGER: &str = "bong:hurt_stagger";
const ANIM_BREAKTHROUGH_YINQI: &str = "bong:breakthrough_yinqi";
const ANIM_BREAKTHROUGH_NINGMAI: &str = "bong:breakthrough_ningmai";
const ANIM_BREAKTHROUGH_GUYUAN: &str = "bong:breakthrough_guyuan";
const ANIM_BREAKTHROUGH_TONGLING: &str = "bong:breakthrough_tongling";
const ANIM_TRIBULATION_BRACE: &str = "bong:tribulation_brace";
const ANIM_HARVEST_CROUCH: &str = "bong:harvest_crouch";
/// 广播体操练习完成 → 专属完整套路动画（150tick/7.5s，5 节：伸展/扩胸/体转/体侧/下蹲）。
/// 此前复用 guard_raise（4tick 举臂格挡），真机表现"只动了一下"；改为
/// client `player_animation/guangbo_ticao.json`（client BongAnimationRegistry 自动扫描注册）。
const ANIM_GUANGBO_TICAO: &str = "bong:guangbo_ticao";
/// 广播体操练习正反馈粒子 —— 走 `bong:vfx_event` JSON 通道（与所有 gameplay 粒子一致），
/// 由客户端 `VfxRegistry` 查到 `GuangboTicaoPracticePlayer`，其内部 spawn vanilla
/// HAPPY_VILLAGER（绿色心叶星点）粒子。复用 vanilla 贴图无新资产。
///
/// 必须与 client `GuangboTicaoPracticePlayer.EVENT_ID` 精确一致，否则客户端查表 miss、
/// 粒子静默丢弃。改一处必须改另一处。
const VFX_GUANGBO_TICAO_PRACTICE: &str = "bong:guangbo_ticao_practice";
/// 广播体操动画分层 priority —— 落在 schema 合法区间 [100, 3999] 内、低于战斗招式
/// （COMBAT_PRIORITY=1000），使练习姿态让位于实际战斗动画。
const GUANGBO_TICAO_PRIORITY: u16 = 500;
const ANIM_LINGTIAN_TILL: &str = "bong:lingtian_till";
const BOTANY_HARVEST_VFX: &str = "bong:botany_harvest";
const LINGTIAN_TILL_VFX: &str = "bong:lingtian_till";
const LINGTIAN_PLANT_VFX: &str = "bong:lingtian_plant";
const LINGTIAN_REPLENISH_VFX: &str = "bong:lingtian_replenish";
const LINGTIAN_HARVEST_VFX: &str = "bong:lingtian_harvest";
const LINGTIAN_DRAIN_VFX: &str = "bong:lingtian_drain";
const WOLIU_PRIORITY: u16 = 1300;
const WOLIU_STOP_FADE_OUT_TICKS: u8 = 4;
const BAOMAI_PRIORITY: u16 = 1500;
const TUIKE_PRIORITY: u16 = 1350;
/// 剑道五招动画优先级——与 baomai 同档（剑道是高阶器修流派）。
const SWORD_PATH_PRIORITY: u16 = 1500;

// plan-sword-path-v2 P4 — 剑道五招专属 AV 资产 id（client 侧已注册）：
//   动画：BongAnimations / player_animation/*.json
//   粒子：SwordPathVfxPlayer.EVENT_IDS（bong: 命名空间）
const ANIM_SWORD_MANIFEST_CAST: &str = "bong:sword_manifest_cast";
const ANIM_SWORD_HEAVEN_GATE_CHARGE: &str = "bong:sword_heaven_gate_charge";
const ANIM_SWORD_HEAVEN_GATE_RELEASE: &str = "bong:sword_heaven_gate_release";
const VFX_SWORD_CONDENSE_EDGE: &str = "bong:sword_condense_edge";
const VFX_SWORD_QI_SLASH_PATH: &str = "bong:sword_qi_slash_path";
const VFX_SWORD_RESONANCE: &str = "bong:sword_resonance";
const VFX_SWORD_MANIFEST_SUMMON: &str = "bong:sword_manifest_summon";
const VFX_HEAVEN_GATE_CHARGE: &str = "bong:heaven_gate_charge_2s";
const VFX_HEAVEN_GATE_RELEASE: &str = "bong:heaven_gate_release";
const VFX_HEAVEN_GATE_FLASH: &str = "bong:heaven_gate_flash";

/// 暗器六招动画优先级——与剑道 / baomai 同档（暗器是高阶器修流派）。
const ANQI_PRIORITY: u16 = 1500;

// 暗器六招专属 AV 资产 id（client 侧 AnqiVfxPlayer / BongAnimations 已注册）。
// 动画全部复用现有 player_animation/*.json（windup_charge / cast_invoke /
// release_burst / sword_stab）；粒子复用现有 BongParticles sprite（无新贴图）。
const ANIM_ANQI_CHARGE: &str = "bong:windup_charge";
const ANIM_ANQI_SNIPE: &str = "bong:sword_stab";
const ANIM_ANQI_VOLLEY: &str = "bong:release_burst";
const ANIM_ANQI_INJECT: &str = "bong:cast_invoke";
const ANIM_ANQI_ECHO: &str = "bong:release_burst";
const VFX_ANQI_CHARGE_SEAL: &str = "bong:anqi_charge_seal";
const VFX_ANQI_SNIPE_BOLT: &str = "bong:anqi_snipe_bolt";
const VFX_ANQI_MULTI_VOLLEY: &str = "bong:anqi_multi_volley";
const VFX_ANQI_SOUL_INJECT: &str = "bong:anqi_soul_inject";
const VFX_ANQI_ARMOR_PIERCE: &str = "bong:anqi_armor_pierce";
const VFX_ANQI_ECHO_DECOY: &str = "bong:anqi_echo_decoy";

// 蛊道（独孤毒流）基础两招专属 AV 资产 id（client 侧 DuguNeedleVfxPlayer / BongAnimations 已注册）。
// 动画复用现有 player_animation/dugu_needle_throw.json；粒子复用既有 BongParticles sprite
// （swordQiTrail line / duguDarkGreenMist），无新贴图。
const ANIM_DUGU_NEEDLE_THROW: &str = "bong:dugu_needle_throw";
const VFX_DUGU_NEEDLE_BOLT: &str = "bong:dugu_needle_bolt";
const VFX_DUGU_POISON_INFUSE: &str = "bong:dugu_poison_infuse";

/// 绝灵涡流（woliu v1）起手式——复用 v2 涡旋站桩动画资产（client 已注册）。
const ANIM_WOLIU_V1_STANCE: &str = "bong:vortex_spiral_stance";
/// 绝灵涡流开涡吸入环。**与 client `VortexSpiralPlayer.WOLIU_V1_FIELD_OPEN` 逐字对齐。**
const VFX_WOLIU_V1_FIELD_OPEN: &str = "bong:woliu_vortex_field";
/// 绝灵涡流存续低频涡环。**与 client `VortexSpiralPlayer.WOLIU_V1_FIELD_AMBIENT` 逐字对齐。**
const VFX_WOLIU_V1_FIELD_AMBIENT: &str = "bong:woliu_vortex_field_ambient";
/// 绝灵涡流反噬断经爆裂。**与 client `VortexSpiralPlayer.WOLIU_V1_BACKFIRE` 逐字对齐。**
const VFX_WOLIU_V1_BACKFIRE: &str = "bong:woliu_vortex_backfire";
/// 存续涡环的发射周期（tick）。20tick=1s：低频到不吃 per-chunk cap，又足以标示领域仍在。
const WOLIU_V1_AMBIENT_PERIOD_TICKS: u64 = 20;
/// 蛊道两招动画优先级——与基础战斗动画同档（蛊道两招是醒灵 / 引气期入门远程招）。
const DUGU_PRIORITY: u16 = 1100;

// ── plan-skill-av-relink-v1 P1 — 孤儿动画接线 ─────────────────────────────────
// 以下 anim id 与 client `BongAnimations.java` 常量 / `player_animation/*.json`
// 资产逐字对齐（P3 由共享清单 pin 双端一致性）。stance_* 事件源 =
// `TechniqueLearnedEvent`（习得即激活，见 `technique_scroll::learn_technique_if_allowed`
// 写 `active:true`）；stance_zhenfa 无 KnownTechniques 条目驱动，维持 report-only
// （见 plan P1 表）。rune_draw / alchemy_stir / enlightenment_pose 为业务模块内联
// emit（无独立事件或事件覆盖不全，见各调用点注释），常数在此集中声明防漂移。
const ANIM_STANCE_WOLIU: &str = "bong:stance_woliu";
const ANIM_STANCE_DUGU: &str = "bong:stance_dugu";
const ANIM_STANCE_DUGU_POISON: &str = "bong:stance_dugu_poison";
const ANIM_STANCE_BAOMAI: &str = "bong:stance_baomai";
const ANIM_STANCE_ZHENMAI: &str = "bong:stance_zhenmai";
const ANIM_STANCE_TUIKE: &str = "bong:stance_tuike";
/// 淬炼抡锤动画——与 `forge::handle_tempering_hits` 的 FORGE_HAMMER_STRIKE 粒子同源
/// （TemperingHit 事件），由 `emit_forge_tempering_animation_triggers` 发射。
const ANIM_FORGE_HAMMER: &str = "bong:forge_hammer";
/// zhenfa 落阵成功 → 画符动画（`zhenfa::handle_zhenfa_place_requests` 内联 emit——
/// deploy 事件仅覆盖 4 kind + 组网成型，普通陷阱无事件，走 adapter 会漏）。
pub(crate) const ANIM_RUNE_DRAW: &str = "bong:rune_draw";
/// 炼丹干预 → 搅拌动画（`client_request_handler::handle_alchemy_intervention` 内联
/// emit，与 ALCHEMY_BREW_VAPOR 粒子同点——干预无 bevy 事件可订阅）。
pub(crate) const ANIM_ALCHEMY_STIR: &str = "bong:alchemy_stir";
/// 顿悟抉择被接受并生效 → 顿悟姿态（`insight_flow::apply_insight_chosen` 校验通过
/// 分支内联 emit——校验前发会在 stale/无效/被拒抉择上误播）。
pub(crate) const ANIM_ENLIGHTENMENT_POSE: &str = "bong:enlightenment_pose";

/// plan-skill-av-relink-v1 P3 — P1 全部 11 条新接线的 anim id 清单（**唯一真相源**，
/// 与 plan P1 表逐行同源）。共享清单
/// `client/src/test/resources/bong/anim_wiring_manifest.json` 由本表单向生成
/// （重生成入口见 `anim_wiring_manifest_test`，禁手改），双端消费：
/// - server：`network::anim_wiring_manifest_test` 断言清单与本表完全一致（无多无少
///   无漂移无字节级手改）；
/// - client：`AnimWiringManifestTest` 经 classloader 读同一份 JSON，逐项断言
///   `BongAnimationRegistry` 可注册解析 + `player_animation/<id>.json` 资产真实存在。
///
/// 新增/删除 P1 接线时必须同步改本表并重生成清单，否则双端测试各自撞红。
/// `#[cfg(test)]`：本表专供双端一致性测试消费，生产 emit 点直接用上方各常量。
#[cfg(test)]
pub(crate) const P1_WIRED_ANIM_IDS: [&str; 11] = [
    ANIM_STANCE_WOLIU,
    ANIM_STANCE_DUGU,
    ANIM_STANCE_DUGU_POISON,
    ANIM_STANCE_BAOMAI,
    ANIM_STANCE_ZHENMAI,
    ANIM_STANCE_TUIKE,
    ANIM_FORGE_HAMMER,
    ANIM_RUNE_DRAW,
    ANIM_ALCHEMY_STIR,
    ANIM_ENLIGHTENMENT_POSE,
    ANIM_FIST_PUNCH_LEFT,
];

// pub(crate)：rune_draw（zhenfa）/ alchemy_stir（client_request_handler）/
// enlightenment_pose（insight_flow）内联 emit 点复用同一优先级常量，防各自取值漂移。
pub(crate) const COMBAT_PRIORITY: u16 = 1000;
const HIT_RECOIL_PRIORITY: u16 = 2000;
pub(crate) const STORY_PRIORITY: u16 = 3000;

/// plan-skill-av-relink-v1 P1 — 空手连击超时（tick）：两次空手攻击间隔超过该值后
/// 交替态复位、右拳起手（~2s @20tps）。测试锁"超时复位"行为语义而非字面值。
const FIST_COMBO_RESET_TICKS: u64 = 40;

/// 每实体空手连击交替态（`Local<HashMap<Entity, _>>` 按 Entity 键天然玩家隔离）。
/// pub(crate)：作为 `emit_attack_animation_triggers` 系统参数类型需对注册点可见。
#[derive(Debug, Clone, Copy)]
pub(crate) struct FistComboState {
    next_is_left: bool,
    last_punch_tick: u64,
}

type PlayerAnimTargetItem<'a> = (&'a Position, &'a UniqueId);
type PlayerAnimTargetFilter = ();
type WoliuVisualStateItem<'a> = (Entity, &'a Position, &'a UniqueId, &'a VortexV2State);

/// Combat intent -> attacker action animation.
///
/// `AttackSource::BurstMeridian` is intentionally skipped: that skill already emits its
/// bespoke `bong:beng_quan` animation in `cultivation::burst_meridian`.
///
/// 剑道五招（`SwordPath*` source）同样跳过：它们的动画 / 粒子由
/// `emit_sword_path_visual_triggers` 读 `SwordPathSkillCastEvent` 独立 emit（含化形 /
/// 天门专属动画），否则会与本系统的基础剑斩动画双重触发（化形尤甚——基础 wound-kind
/// 动画会盖掉 manifest_cast 专属动画）。
/// plan-skill-av-relink-v1 P1：空手（attacker 无 `Weapon` component）且解析落拳击
/// 动画时，右/左拳按连击交替（right 起手，超时复位，见 `next_fist_punch_anim`）；
/// 持械（含 Staff/Fist 类武器）不参与交替，恒 `fist_punch_right`。
pub fn emit_attack_animation_triggers(
    mut intents: EventReader<AttackIntent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    weapons: Query<&Weapon>,
    clock: Res<CombatClock>,
    mut fist_combo: Local<HashMap<Entity, FistComboState>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for intent in intents.read() {
        if intent.source == AttackSource::BurstMeridian || is_sword_path_source(intent.source) {
            continue;
        }
        let mut anim_id = attack_anim_for_source(intent.source, intent.wound_kind);
        if anim_id == ANIM_FIST_PUNCH_RIGHT && weapons.get(intent.attacker).is_err() {
            anim_id = next_fist_punch_anim(&mut fist_combo, intent.attacker, clock.tick);
        }
        emit_play_for_entity(
            intent.attacker,
            anim_id,
            COMBAT_PRIORITY,
            Some(2),
            &players,
            &mut vfx_events,
        );
    }
}

/// 空手连击交替态机：right → left → right ...；两拳间隔超过
/// `FIST_COMBO_RESET_TICKS` 视为连击中断，复位为 right 起手。
fn next_fist_punch_anim(
    combo: &mut HashMap<Entity, FistComboState>,
    attacker: Entity,
    now_tick: u64,
) -> &'static str {
    let state = combo.entry(attacker).or_insert(FistComboState {
        next_is_left: false,
        last_punch_tick: now_tick,
    });
    if now_tick.saturating_sub(state.last_punch_tick) > FIST_COMBO_RESET_TICKS {
        state.next_is_left = false;
    }
    state.last_punch_tick = now_tick;
    let anim_id = if state.next_is_left {
        ANIM_FIST_PUNCH_LEFT
    } else {
        ANIM_FIST_PUNCH_RIGHT
    };
    state.next_is_left = !state.next_is_left;
    anim_id
}

/// plan-skill-av-relink-v1 P1 — 「激活功法」时刻 → 流派架势动画。
///
/// 事件源 = `TechniqueLearnedEvent`（`learn_technique_if_allowed` 习得即写
/// `active:true` = 激活）。生产发射路仅两条：卷轴习得（`client_request_handler`）
/// 与首击领悟（`first_hit_dash`，仅授 `movement.dash`、无架势映射）；
/// `technique_mentor::mentor_teaches_technique` 为无生产调用方的休眠 helper。
/// 现有内容卷轴仅授 `woliu.*` + `zhenmai.parry` → 今日即活 = woliu / zhenmai
/// 两族架势；dugu / dugu_poison / baomai / tuike 为接口先锁定的潜伏接线，
/// 等对应流派习得内容落地即激活。
/// 全仓不存在「流派架势切换」gameplay 事件（`stance_switch` audio recipe 的唯一
/// 发射点是 SkillXpGain 经验反馈、与架势无关），按 plan §8 #2 决议改接本时刻。
/// 无映射前缀（sword / anqi / burst_meridian / movement / morph / body / npc /
/// shield_block / sword_path）不发；dev `/technique active` 直改组件不发事件，
/// 不接线（dev-only 旁路）。
pub fn emit_technique_learned_stance_triggers(
    mut learned: EventReader<TechniqueLearnedEvent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in learned.read() {
        let Some(anim_id) = stance_anim_for_technique(event.technique_id.as_str()) else {
            continue;
        };
        emit_play_for_entity(
            event.player,
            anim_id,
            STORY_PRIORITY,
            Some(3),
            &players,
            &mut vfx_events,
        );
    }
}

/// technique_id → 流派架势动画映射（plan-skill-av-relink-v1 P1 设计收口清单）。
/// dugu 两条按完整 id 区分（毒蛊有独立 stance_dugu_poison）；其余按前缀。
fn stance_anim_for_technique(technique_id: &str) -> Option<&'static str> {
    match technique_id {
        "dugu.shoot_needle" => return Some(ANIM_STANCE_DUGU),
        "dugu.infuse_poison" => return Some(ANIM_STANCE_DUGU_POISON),
        _ => {}
    }
    match technique_id.split('.').next()? {
        "woliu" => Some(ANIM_STANCE_WOLIU),
        "baomai" => Some(ANIM_STANCE_BAOMAI),
        "zhenmai" => Some(ANIM_STANCE_ZHENMAI),
        "tuike" => Some(ANIM_STANCE_TUIKE),
        _ => None,
    }
}

/// plan-skill-av-relink-v1 P1 — 淬炼按键命中 → `forge_hammer` 抡锤动画。
///
/// 与 `forge::handle_tempering_hits` 的 FORGE_HAMMER_STRIKE 粒子同源
/// （`TemperingHit`，玩家 J/K/L 淬炼按键）；镜像其 `ForgeStep::Tempering` 步骤门：
/// 非淬炼步的 stale 按键不发动画。session 缺失（已结算/弃疗）同样不发。
pub fn emit_forge_tempering_animation_triggers(
    mut hits: EventReader<TemperingHit>,
    sessions: Res<ForgeSessions>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for hit in hits.read() {
        let Some(session) = sessions.get(hit.session) else {
            continue;
        };
        if session.current_step != ForgeStep::Tempering {
            continue;
        }
        emit_play_for_entity(
            session.caster,
            ANIM_FORGE_HAMMER,
            COMBAT_PRIORITY,
            Some(1),
            &players,
            &mut vfx_events,
        );
    }
}

/// Defense intent -> guard pose animation.
pub fn emit_defense_animation_triggers(
    mut defenses: EventReader<DefenseIntent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for defense in defenses.read() {
        emit_play_for_entity(
            defense.defender,
            ANIM_PARRY_BLOCK,
            COMBAT_PRIORITY,
            Some(1),
            &players,
            &mut vfx_events,
        );
    }
}

/// Resolved hit -> target recoil animation.
pub fn emit_hit_recoil_animation_triggers(
    mut events: EventReader<CombatEvent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in events.read() {
        if event.damage + event.physical_damage <= 0.0 {
            continue;
        }
        emit_play_for_entity(
            event.target,
            ANIM_HURT_STAGGER,
            HIT_RECOIL_PRIORITY,
            Some(1),
            &players,
            &mut vfx_events,
        );
    }
}

/// Breakthrough success -> full-body story animation.
pub fn emit_breakthrough_animation_triggers(
    mut outcomes: EventReader<BreakthroughOutcome>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for outcome in outcomes.read() {
        if outcome.result.is_err() {
            continue;
        }
        emit_play_for_entity(
            outcome.entity,
            breakthrough_anim_for_outcome(outcome),
            STORY_PRIORITY,
            Some(3),
            &players,
            &mut vfx_events,
        );
    }
}

/// Tribulation lifecycle -> brace / recoil animations.
pub fn emit_tribulation_animation_triggers(
    mut announces: EventReader<TribulationAnnounce>,
    mut failures: EventReader<TribulationFailed>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for announce in announces.read() {
        emit_play_for_entity(
            announce.entity,
            ANIM_TRIBULATION_BRACE,
            STORY_PRIORITY,
            Some(3),
            &players,
            &mut vfx_events,
        );
    }

    for failure in failures.read() {
        emit_play_for_entity(
            failure.entity,
            ANIM_HURT_STAGGER,
            HIT_RECOIL_PRIORITY,
            Some(1),
            &players,
            &mut vfx_events,
        );
    }
}

/// Tribulation settlement success (Ascended / HalfStep) -> breakthrough pillar particle + animation.
///
/// Independent system so it does not disturb `emit_tribulation_animation_triggers`'s
/// Bevy EventReader cursor.  Particle event_id `bong:breakthrough_pillar` is confirmed
/// registered in `VfxBootstrap.java` (BreakthroughPillarPlayer).
pub fn emit_tribulation_settled_vfx_triggers(
    mut settled: EventReader<TribulationSettled>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in settled.read() {
        let (anim_id, particle_count) = match event.result.outcome {
            DuXuOutcomeV1::Ascended => (ANIM_BREAKTHROUGH_TONGLING, 16u16),
            DuXuOutcomeV1::HalfStep => (ANIM_BREAKTHROUGH_GUYUAN, 10u16),
            _ => continue,
        };
        // 1. Play breakthrough animation.
        emit_play_for_entity(
            event.entity,
            anim_id,
            STORY_PRIORITY,
            Some(4),
            &players,
            &mut vfx_events,
        );
        // 2. Spawn pillar particle at the entity's position.
        let Ok((position, _unique_id)) = players.get(event.entity) else {
            continue;
        };
        let origin = position.get();
        vfx_events.send(VfxEventRequest::new(
            origin,
            VfxEventPayloadV1::SpawnParticle {
                event_id: "bong:breakthrough_pillar".to_string(),
                origin: [origin.x, origin.y, origin.z],
                direction: None,
                color: None,
                strength: Some(1.2),
                count: Some(particle_count),
                duration_ticks: Some(60),
            },
        ));
    }
}

pub fn emit_woliu_v2_visual_triggers(
    mut casts: EventReader<VortexCastEvent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in casts.read() {
        emit_play_for_entity(
            event.caster,
            event.visual.animation_id,
            WOLIU_PRIORITY,
            Some(2),
            &players,
            &mut vfx_events,
        );
        vfx_events.send(VfxEventRequest::new(
            event.center,
            VfxEventPayloadV1::SpawnParticle {
                event_id: event.visual.particle_id.to_string(),
                origin: [event.center.x, event.center.y, event.center.z],
                direction: None,
                color: Some(color_for_woliu_skill(event.skill).to_string()),
                strength: Some(woliu_particle_strength(event)),
                count: Some(woliu_particle_count(event.skill)),
                duration_ticks: Some(woliu_particle_duration_ticks(event.skill)),
            },
        ));
    }
}

#[derive(Clone, Copy)]
pub(crate) struct WoliuVisualLifecycle {
    skill: WoliuSkillId,
    was_active: bool,
}

pub fn emit_woliu_v2_visual_stop_triggers(
    clock: Res<CombatClock>,
    mut seen_states: Local<HashMap<Entity, WoliuVisualLifecycle>>,
    states: Query<WoliuVisualStateItem<'_>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    let mut seen_entities = HashSet::new();
    for (entity, position, unique_id, state) in &states {
        seen_entities.insert(entity);
        let active_now = clock.tick < state.active_until_tick;
        if let Some(previous) = seen_states.get(&entity) {
            if previous.was_active && !active_now {
                emit_stop_for_entity(
                    position,
                    unique_id,
                    woliu_anim_for_skill(previous.skill),
                    WOLIU_STOP_FADE_OUT_TICKS,
                    &mut vfx_events,
                );
            }
        }
        seen_states.insert(
            entity,
            WoliuVisualLifecycle {
                skill: state.active_skill_kind,
                was_active: active_now,
            },
        );
    }
    seen_states.retain(|entity, _| seen_entities.contains(entity));
}

/// 广播体操（body.guangbo_ticao）练习完成 → 动画 + 粒子（纯 cosmetic）。
///
/// 读 `GuangboTicaoPracticeEvent`（由 cast_emit::tick_casts_or_interrupt 在 cast
/// 自然完成时 emit），对练习者发：
/// 1. `PlayAnim`：专属 `guangbo_ticao` 完整套路（150tick/7.5s，5 节）。
/// 2. `SpawnParticle`：event_id `bong:guangbo_ticao_practice`，走 `bong:vfx_event` JSON 通道
///    （与所有 gameplay 粒子一致），客户端 `GuangboTicaoPracticePlayer` 据此 spawn vanilla
///    `happy_villager`（绿色心叶星点）正反馈，复用 vanilla 贴图无净新资产。
///    **不可**直接发 `minecraft:happy_villager`——那会让客户端 `VfxRegistry` 查表 miss、粒子静默丢弃。
///
/// **纯 cosmetic**：不读 / 改任何真元 / proficiency 状态（守恒由消费侧
/// `consume_guangbo_practice_events` 负责）。caster 无 `Position`/`UniqueId`（断线）时
/// 动画静默 skip，粒子仍按其 Position 发出（无 Position 则整体 skip）。
pub fn emit_guangbo_ticao_visual_triggers(
    mut practices: EventReader<GuangboTicaoPracticeEvent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in practices.read() {
        // 1. 动画——专属广播体操完整套路。caster 无 Position/UniqueId 时静默 skip。
        emit_play_for_entity(
            event.entity,
            ANIM_GUANGBO_TICAO,
            GUANGBO_TICAO_PRIORITY,
            Some(2),
            &players,
            &mut vfx_events,
        );

        // 2. 粒子——绿色心叶星点正反馈，围绕练习者头部发出。无 Position 时跳过粒子。
        let Ok((position, _unique_id)) = players.get(event.entity) else {
            continue;
        };
        let origin = position.get();
        emit_spawn_particle(
            &mut vfx_events,
            VFX_GUANGBO_TICAO_PRACTICE,
            valence::prelude::DVec3::new(origin.x, origin.y + 1.2, origin.z),
            "#7FE38F",
            0.6,
            6,
            20,
        );
    }
}

pub fn emit_botany_harvest_visual_triggers(
    mut terminal: EventReader<HarvestTerminalEvent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in terminal.read() {
        if !event.completed || event.interrupted {
            continue;
        }
        emit_play_for_entity(
            event.client_entity,
            ANIM_HARVEST_CROUCH,
            COMBAT_PRIORITY,
            Some(2),
            &players,
            &mut vfx_events,
        );
        let Some(pos) = event.target_pos else {
            continue;
        };
        emit_spawn_particle(
            &mut vfx_events,
            BOTANY_HARVEST_VFX,
            valence::prelude::DVec3::new(pos[0], pos[1] + 0.45, pos[2]),
            botany_quality_color(event.spirit_quality),
            event.spirit_quality.clamp(0.5, 1.0),
            12,
            36,
        );
    }
}

pub fn emit_baomai_v3_visual_triggers(
    mut events: EventReader<BaomaiSkillEvent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in events.read() {
        emit_play_for_entity(
            event.caster,
            baomai_anim_for_skill(event.skill),
            BAOMAI_PRIORITY,
            Some(2),
            &players,
            &mut vfx_events,
        );
    }
}

pub fn emit_lingtian_visual_triggers(
    mut tills: EventReader<TillCompleted>,
    mut plantings: EventReader<PlantingCompleted>,
    mut harvests: EventReader<HarvestCompleted>,
    mut replenishes: EventReader<ReplenishCompleted>,
    mut drains: EventReader<DrainQiCompleted>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
) {
    for event in tills.read() {
        emit_play_for_entity(
            event.player,
            ANIM_LINGTIAN_TILL,
            COMBAT_PRIORITY,
            Some(2),
            &players,
            &mut vfx_events,
        );
        emit_block_decal(
            &mut vfx_events,
            LINGTIAN_TILL_VFX,
            event.pos,
            "#44CCCC",
            0.65,
        );
    }
    for event in plantings.read() {
        emit_block_decal(
            &mut vfx_events,
            LINGTIAN_PLANT_VFX,
            event.pos,
            "#55EE88",
            0.75,
        );
    }
    for event in harvests.read() {
        emit_block_decal(
            &mut vfx_events,
            LINGTIAN_HARVEST_VFX,
            event.pos,
            "#88FF66",
            0.85,
        );
    }
    for event in replenishes.read() {
        emit_block_decal(
            &mut vfx_events,
            LINGTIAN_REPLENISH_VFX,
            event.pos,
            "#44DDCC",
            (0.55 + event.plot_qi_added).clamp(0.55, 1.0),
        );
    }
    for event in drains.read() {
        emit_block_decal(
            &mut vfx_events,
            LINGTIAN_DRAIN_VFX,
            event.pos,
            "#888888",
            0.7,
        );
    }
}

fn baomai_anim_for_skill(skill: BaomaiSkillId) -> &'static str {
    match skill {
        BaomaiSkillId::BengQuan => "bong:beng_quan",
        BaomaiSkillId::FullPowerCharge => ANIM_GUARD_RAISE,
        BaomaiSkillId::FullPowerRelease => ANIM_FIST_PUNCH_RIGHT,
        BaomaiSkillId::MountainShake => "bong:baomai_mountain_shake",
        BaomaiSkillId::BloodBurn => "bong:baomai_blood_burn",
        BaomaiSkillId::Disperse => "bong:baomai_disperse",
    }
}

pub fn emit_tuike_v2_visual_triggers(
    mut don_events: EventReader<DonFalseSkinEvent>,
    mut shed_events: EventReader<FalseSkinSheddedEvent>,
    mut transfer_events: EventReader<ContamTransferredEvent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in don_events.read() {
        emit_tuike_visual_for_entity(
            event.caster,
            &event.visual,
            "#D8C08A",
            10,
            &players,
            &mut vfx_events,
        );
    }
    for event in shed_events.read() {
        let color = if event.permanent_taint_load > 0.0 {
            "#BFD8FF"
        } else {
            "#B58B5A"
        };
        emit_tuike_visual_for_entity(
            event.owner,
            &event.visual,
            color,
            18,
            &players,
            &mut vfx_events,
        );
    }
    for event in transfer_events.read() {
        let color = if event.permanent_absorbed > 0.0 {
            "#9EC7FF"
        } else {
            "#7B5B8C"
        };
        emit_tuike_visual_for_entity(
            event.caster,
            &event.visual,
            color,
            12,
            &players,
            &mut vfx_events,
        );
    }
}

/// plan-sword-path-v2 P4 — 剑道五招 cast → 专属动画 + 专属粒子。
///
/// 读 `SwordPathSkillCastEvent`，按招式发：
/// 1. `PlayAnim`（攻击招式复用基础剑斩 / 化形 / 天门专属动画，引用 `BongAnimations`）。
/// 2. `SpawnParticle`（各招专属 event_id，引用 `SwordPathVfxPlayer.EVENT_IDS`）。
///
/// **纯 cosmetic**：只发 `VfxEventRequest`，不读 / 改任何战斗 / 真元状态。caster
/// 无 `Position`/`UniqueId`（断线 / 非 skinned）时动画静默 skip，粒子仍按 event.center
/// 发出（粒子不依赖 player target）。
pub fn emit_sword_path_visual_triggers(
    mut casts: EventReader<SwordPathSkillCastEvent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in casts.read() {
        // 1. 动画——攻击招式复用基础剑斩；化形 / 天门用专属动画。
        let anim_id = sword_path_anim_for_skill(event.skill);
        emit_play_for_entity(
            event.caster,
            anim_id,
            SWORD_PATH_PRIORITY,
            Some(2),
            &players,
            &mut vfx_events,
        );

        // 2. 粒子——各招专属 event_id。方向仅剑气斩用（line trail）。
        let particle_id = sword_path_particle_for_skill(event.skill);
        let direction = event.direction.map(|d| [d.x, d.y, d.z]);
        let color = sword_path_particle_color(event.skill);
        let (count, duration) = sword_path_particle_count_duration(event.skill);
        vfx_events.send(VfxEventRequest::new(
            event.center,
            VfxEventPayloadV1::SpawnParticle {
                event_id: particle_id.to_string(),
                origin: [event.center.x, event.center.y, event.center.z],
                direction,
                color: Some(color.to_string()),
                strength: Some(sword_path_particle_strength(event.skill)),
                count: Some(count),
                duration_ticks: Some(duration),
            },
        ));

        // 天门释放：额外叠一层开天 flash（双粒子层，区别于蓄力）。
        if event.skill == SwordPathSkillId::HeavenGateRelease {
            vfx_events.send(VfxEventRequest::new(
                event.center,
                VfxEventPayloadV1::SpawnParticle {
                    event_id: VFX_HEAVEN_GATE_FLASH.to_string(),
                    origin: [event.center.x, event.center.y + 1.0, event.center.z],
                    direction: None,
                    color: Some("#E8F0FF".to_string()),
                    strength: Some(1.0),
                    count: Some(24),
                    duration_ticks: Some(30),
                },
            ));
        }
    }
}

/// 暗器六招 cast → 动画 + 粒子（纯 cosmetic，复用现有 anim/sprite 资产）。
///
/// 与剑道五招的 `emit_sword_path_visual_triggers` 同模式：读 anqi_v2 已 emit 的
/// 结果型 events，对 caster 发 `PlayAnim` + `SpawnParticle`，引用 client 已注册的
/// `AnqiVfxPlayer` 粒子 event_id 与 `BongAnimations` 动画 id。
///
/// 招式 → 事件源映射：
/// - 封骨（充能）`CarrierChargedEvent` → windup_charge 动画 + 封骨密封粒子
/// - 单射狙击 `QiInjectionEvent{SingleSnipe}` → sword_stab 动画 + 狙击弹道（caster→target 方向）
/// - 多发齐射 `MultiShotEvent` → release_burst 动画 + 扇形齐射粒子
/// - 凝魂注射 `QiInjectionEvent{SoulInject}` → cast_invoke 动画 + 魂注紫雾
/// - 破甲注射 `ArmorPierceEvent` → cast_invoke 动画 + 破甲金属火花（caster→target 方向）
/// - 诱饵分形 `EchoFractalEvent` → release_burst 动画 + 分形回响涟漪
///
/// **纯 cosmetic**：不读 / 改任何战斗数值 / qi_physics ledger / 命中结算。
#[allow(clippy::too_many_arguments)]
pub fn emit_anqi_visual_triggers(
    mut charges: EventReader<CarrierChargedEvent>,
    mut injections: EventReader<QiInjectionEvent>,
    mut multi_shots: EventReader<MultiShotEvent>,
    mut armor_pierces: EventReader<ArmorPierceEvent>,
    mut echoes: EventReader<EchoFractalEvent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    positions: Query<&Position>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    // 封骨（充能完成）：windup_charge 动画 + 封骨密封粒子（骨白）。
    for event in charges.read() {
        let Some(origin) = positions.get(event.carrier).map(|p| p.get()).ok() else {
            continue;
        };
        emit_play_for_entity(
            event.carrier,
            ANIM_ANQI_CHARGE,
            ANQI_PRIORITY,
            Some(2),
            &players,
            &mut vfx_events,
        );
        emit_anqi_particle(
            &mut vfx_events,
            VFX_ANQI_CHARGE_SEAL,
            origin,
            None,
            "#E8DCC8",
            0.85,
            12,
            24,
        );
    }

    // 单射狙击 / 凝魂注射：QiInjectionEvent 区分招式。狙击走 sword_stab + 弹道粒子，
    // 凝魂走 cast_invoke + 魂注紫雾。
    for event in injections.read() {
        let origin = positions
            .get(event.caster)
            .map(|p| p.get())
            .unwrap_or_default();
        match event.skill {
            AnqiSkillId::SingleSnipe => {
                let direction = caster_target_direction(&positions, event.caster, event.target);
                emit_play_for_entity(
                    event.caster,
                    ANIM_ANQI_SNIPE,
                    ANQI_PRIORITY,
                    Some(1),
                    &players,
                    &mut vfx_events,
                );
                emit_anqi_particle(
                    &mut vfx_events,
                    VFX_ANQI_SNIPE_BOLT,
                    origin,
                    Some(direction),
                    "#C8E0F0",
                    0.9,
                    12,
                    18,
                );
            }
            AnqiSkillId::SoulInject => {
                emit_play_for_entity(
                    event.caster,
                    ANIM_ANQI_INJECT,
                    ANQI_PRIORITY,
                    Some(2),
                    &players,
                    &mut vfx_events,
                );
                emit_anqi_particle(
                    &mut vfx_events,
                    VFX_ANQI_SOUL_INJECT,
                    origin,
                    None,
                    "#B9A7FF",
                    0.85,
                    16,
                    26,
                );
            }
            // MultiShot / ArmorPierce / EchoFractal 走各自专属 EventReader，不在此分支。
            _ => {}
        }
    }

    // 多发齐射：release_burst 动画 + 扇形齐射粒子（散射弹幕）。
    for event in multi_shots.read() {
        let origin = positions
            .get(event.caster)
            .map(|p| p.get())
            .unwrap_or_default();
        emit_play_for_entity(
            event.caster,
            ANIM_ANQI_VOLLEY,
            ANQI_PRIORITY,
            Some(1),
            &players,
            &mut vfx_events,
        );
        // 粒子数量随弹数缩放（每发 4 颗，clamp 到合理上限）。
        let count = (u16::from(event.projectile_count) * 4).clamp(8, 40);
        emit_anqi_particle(
            &mut vfx_events,
            VFX_ANQI_MULTI_VOLLEY,
            origin,
            None,
            "#D8E0B0",
            0.8,
            count,
            22,
        );
    }

    // 破甲注射：cast_invoke 动画 + 破甲金属火花（caster→target 朝向）。
    for event in armor_pierces.read() {
        let origin = positions
            .get(event.caster)
            .map(|p| p.get())
            .unwrap_or_default();
        let direction = caster_target_direction(&positions, event.caster, event.target);
        emit_play_for_entity(
            event.caster,
            ANIM_ANQI_INJECT,
            ANQI_PRIORITY,
            Some(1),
            &players,
            &mut vfx_events,
        );
        emit_anqi_particle(
            &mut vfx_events,
            VFX_ANQI_ARMOR_PIERCE,
            origin,
            Some(direction),
            "#C0C4C8",
            0.95,
            14,
            20,
        );
    }

    // 诱饵分形：release_burst 动画 + 分形回响涟漪（分身数缩放粒子）。
    for event in echoes.read() {
        let origin = positions
            .get(event.caster)
            .map(|p| p.get())
            .unwrap_or_default();
        emit_play_for_entity(
            event.caster,
            ANIM_ANQI_ECHO,
            ANQI_PRIORITY,
            Some(2),
            &players,
            &mut vfx_events,
        );
        let count = (event.outcome.echo_count as u16 * 5).clamp(10, 40);
        emit_anqi_particle(
            &mut vfx_events,
            VFX_ANQI_ECHO_DECOY,
            origin,
            None,
            "#B9A7FF",
            0.78,
            count,
            30,
        );
    }
}

/// caster → target 的单位方向；任一 Position 缺失或两点重合时落到 +X。
fn caster_target_direction(
    positions: &Query<&Position>,
    caster: Entity,
    target: Option<Entity>,
) -> [f64; 3] {
    let fallback = [1.0, 0.0, 0.0];
    let Some(target) = target else {
        return fallback;
    };
    let (Ok(from), Ok(to)) = (positions.get(caster), positions.get(target)) else {
        return fallback;
    };
    let delta = to.get() - from.get();
    // try_normalize 在近零向量（caster≈target）时返回 None，避免 normalize 出 NaN 方向。
    match delta.try_normalize() {
        Some(dir) => [dir.x, dir.y, dir.z],
        None => fallback,
    }
}

/// 发一条暗器 `SpawnParticle` VFX 请求（统一参数封装，减少重复）。
#[allow(clippy::too_many_arguments)]
fn emit_anqi_particle(
    vfx_events: &mut EventWriter<VfxEventRequest>,
    event_id: &'static str,
    origin: valence::prelude::DVec3,
    direction: Option<[f64; 3]>,
    color: &str,
    strength: f32,
    count: u16,
    duration_ticks: u16,
) {
    vfx_events.send(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id: event_id.to_string(),
            origin: [origin.x, origin.y, origin.z],
            direction,
            color: Some(color.to_string()),
            strength: Some(strength),
            count: Some(count),
            duration_ticks: Some(duration_ticks),
        },
    ));
}

/// 蛊道（独孤毒流）基础两招 cast → 动画 + 粒子（纯 cosmetic，复用现有 anim/sprite 资产）。
///
/// 与暗器 [`emit_anqi_visual_triggers`] 同模式：读各招已 emit 的领域事件，再翻译成
/// 第一方动画 id + 粒子 event_id。**不读 / 改任何战斗 / 真元状态。**
///
/// - 凝针 `QiNeedleChargedEvent` → `dugu_needle_throw` 动画 + 朝向 `dugu_needle_bolt`
///   弹道粒子（caster→target 方向；细针远距直刺，复用剑气 line trail，冷青白）。
/// - 灌毒蛊 `DuguObfuscationDisruptedEvent` → `dugu_needle_throw` 动画 + `dugu_poison_infuse`
///   失谐真元毒绿雾（覆于双手 / 飞针，复用 duguDarkGreenMist sprite）。
pub fn emit_dugu_needle_visual_triggers(
    mut needles: EventReader<QiNeedleChargedEvent>,
    mut infusions: EventReader<DuguObfuscationDisruptedEvent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    positions: Query<&Position>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    // 凝针：throw 动画 + 朝向弹道粒子（细针直刺，冷青白）。
    for event in needles.read() {
        let origin = positions
            .get(event.shooter)
            .map(|p| p.get())
            .unwrap_or_default();
        let direction = caster_target_direction(&positions, event.shooter, event.target);
        emit_play_for_entity(
            event.shooter,
            ANIM_DUGU_NEEDLE_THROW,
            DUGU_PRIORITY,
            Some(1),
            &players,
            &mut vfx_events,
        );
        emit_anqi_particle(
            &mut vfx_events,
            VFX_DUGU_NEEDLE_BOLT,
            origin,
            Some(direction),
            "#BFE3D0",
            0.9,
            10,
            16,
        );
    }

    // 灌毒蛊：throw 动画 + 失谐真元毒绿雾（覆入飞针，无方向，绕身散布）。
    for event in infusions.read() {
        let origin = positions
            .get(event.infuser)
            .map(|p| p.get())
            .unwrap_or_default();
        emit_play_for_entity(
            event.infuser,
            ANIM_DUGU_NEEDLE_THROW,
            DUGU_PRIORITY,
            Some(2),
            &players,
            &mut vfx_events,
        );
        emit_anqi_particle(
            &mut vfx_events,
            VFX_DUGU_POISON_INFUSE,
            origin,
            None,
            "#44AA44",
            0.85,
            14,
            24,
        );
    }
}

/// 蛊道 v2 五招 cast → 粒子（纯 cosmetic）。
///
/// anim + audio 已由 `combat::dugu_v2::skills` 在 cast 内联 emit（`emit_anim`/`emit_audio`），
/// 此处只补 `visual.particle_id` → `SpawnParticle` 这一段——此前 particle_id 只随
/// `dugu_v2_event_bridge` 进 Redis 叙事通道，client 从未收到 `bong:vfx_event`，导致
/// `dugu_taint_pulse` / `dugu_dark_green_mist` / `dugu_reverse_burst` 三张贴图永不可见。
///
/// **event_id 与 client `DuguV2VfxPlayer.EVENT_IDS` 逐字对齐。**
///
/// 粒子语义（各招差异化）：
/// - 蚀针 Eclipse → `dugu_taint_pulse` 毒渍脉冲印于**受害者**脚下（ground decal）
/// - 侵染 Penetrate → 同贴图但更亮更密（count 随链上受害者数增长）
/// - 神识遮蔽 Shroud → `dugu_dark_green_mist` 深绿雾绕**施法者**（count 随 strength）
/// - 自蕴 SelfCure → 同雾但更稀薄（疗毒内敛，不该像开罩一样张扬）
/// - 倒蚀 Reverse → `dugu_reverse_burst` 亮毒绿爆发线束于**爆心**
pub fn emit_dugu_v2_visual_triggers(
    mut eclipses: EventReader<EclipseNeedleEvent>,
    mut self_cures: EventReader<SelfCureProgressEvent>,
    mut penetrates: EventReader<PenetrateChainEvent>,
    mut shrouds: EventReader<ShroudActivatedEvent>,
    mut reverses: EventReader<ReverseTriggeredEvent>,
    positions: Query<&Position>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    // 蚀针：毒渍脉冲印于受害者脚下；受害者已断 Position 时落到施法者（至少能看到出手反馈）。
    for event in eclipses.read() {
        let Ok(position) = positions
            .get(event.target)
            .or_else(|_| positions.get(event.caster))
        else {
            continue;
        };
        emit_anqi_particle(
            &mut vfx_events,
            event.visual.particle_id,
            position.get(),
            None,
            "#57803A",
            1.0,
            12,
            30,
        );
    }

    // 自蕴：稀薄深绿雾绕施法者（疗毒内敛）。
    for event in self_cures.read() {
        let Ok(position) = positions.get(event.caster) else {
            continue;
        };
        emit_anqi_particle(
            &mut vfx_events,
            event.visual.particle_id,
            position.get(),
            None,
            "#3E6B4A",
            0.8,
            14,
            40,
        );
    }

    // 侵染：链式毒渍，密度随受害者数增长（封顶防拥挤 chunk 刷屏）。
    for event in penetrates.read() {
        let Ok(position) = positions
            .get(event.target)
            .or_else(|_| positions.get(event.caster))
        else {
            continue;
        };
        let count = (12 + event.affected_targets.saturating_mul(4)).min(32) as u16;
        emit_anqi_particle(
            &mut vfx_events,
            event.visual.particle_id,
            position.get(),
            None,
            "#6B9C46",
            1.15,
            count,
            36,
        );
    }

    // 神识遮蔽：深绿雾罩绕施法者，浓度随遮蔽强度。
    for event in shrouds.read() {
        let Ok(position) = positions.get(event.caster) else {
            continue;
        };
        emit_anqi_particle(
            &mut vfx_events,
            event.visual.particle_id,
            position.get(),
            None,
            "#335C41",
            event.strength.clamp(0.6, 1.5),
            28,
            60,
        );
    }

    // 倒蚀：亮毒绿爆发线束于爆心（事件自带 center，不依赖 Position）。
    for event in reverses.read() {
        let count = (18 + event.affected_targets.saturating_mul(6)).min(48) as u16;
        emit_anqi_particle(
            &mut vfx_events,
            event.visual.particle_id,
            event.center,
            None,
            "#A0E070",
            1.3,
            count,
            24,
        );
    }
}

/// 绝灵涡流（woliu v1 `woliu.vortex`）持续领域 → 动画 + 粒子（纯 cosmetic）。
///
/// v1 是长驻负灵域场（`VortexField` component），不是 v2 那种一次性 cast 事件，
/// 所以走 **lifecycle 驱动**（与 [`emit_woliu_v2_visual_stop_triggers`] 同模式）：
/// - field **出现** → caster 涡旋起手式动画 + `woliu_vortex_field` 开涡吸入环
/// - field **存续** → 每 [`WOLIU_V1_AMBIENT_PERIOD_TICKS`] tick 一次
///   `woliu_vortex_field_ambient` 低频涡环（强度随半径）
/// - **反噬** `VortexBackfireEvent` → `woliu_vortex_backfire` 断经暗红爆裂
///
/// 不读 / 改任何战斗 / 真元状态；event_id 与 client `VortexSpiralPlayer` 注册逐字对齐。
pub fn emit_woliu_v1_vortex_visual_triggers(
    clock: Res<CombatClock>,
    mut active_fields: Local<HashSet<Entity>>,
    fields: Query<(Entity, &VortexField)>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut backfires: EventReader<VortexBackfireEvent>,
    positions: Query<&Position>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    let mut seen = HashSet::new();
    for (entity, field) in &fields {
        seen.insert(entity);
        // 强度随领域半径（8 米基准），钳在肉眼可辨但不刷屏的区间。
        let strength = (field.radius / 8.0).clamp(0.8, 1.6);
        if !active_fields.contains(&entity) {
            emit_play_for_entity(
                field.caster,
                ANIM_WOLIU_V1_STANCE,
                WOLIU_PRIORITY,
                Some(2),
                &players,
                &mut vfx_events,
            );
            emit_anqi_particle(
                &mut vfx_events,
                VFX_WOLIU_V1_FIELD_OPEN,
                field.center,
                None,
                "#7FD4C8",
                strength,
                24,
                30,
            );
        } else if clock.tick.saturating_sub(field.cast_at_tick) % WOLIU_V1_AMBIENT_PERIOD_TICKS == 0
        {
            emit_anqi_particle(
                &mut vfx_events,
                VFX_WOLIU_V1_FIELD_AMBIENT,
                field.center,
                None,
                "#5FB8AC",
                strength,
                12,
                (WOLIU_V1_AMBIENT_PERIOD_TICKS + 4) as u16,
            );
        }
    }
    *active_fields = seen;

    // 反噬（久持断肺经 / 环境灵气枯竭）：断经暗红爆裂于施法者。
    // caster 断 Position（断线瞬间）时兜底到领域中心——与 audio 侧同口径，重要负反馈不静默丢。
    for event in backfires.read() {
        let Ok(origin) = positions
            .get(event.caster)
            .map(|p| p.get())
            .or_else(|_| fields.get(event.caster).map(|(_, field)| field.center))
        else {
            continue;
        };
        emit_anqi_particle(
            &mut vfx_events,
            VFX_WOLIU_V1_BACKFIRE,
            origin,
            None,
            "#B84A3F",
            1.3,
            30,
            20,
        );
    }
}

fn sword_path_anim_for_skill(skill: SwordPathSkillId) -> &'static str {
    match skill {
        // 凝锋 / 共鸣 是横向 / 范围剑势 → 基础横劈动画。
        SwordPathSkillId::CondenseEdge | SwordPathSkillId::Resonance => ANIM_SWORD_CLEAVE,
        // 剑气斩是远程突刺势 → 基础刺击动画。
        SwordPathSkillId::QiSlash => ANIM_SWORD_THRUST,
        // 化形 / 天门有专属动画。
        SwordPathSkillId::Manifest => ANIM_SWORD_MANIFEST_CAST,
        SwordPathSkillId::HeavenGateCharge => ANIM_SWORD_HEAVEN_GATE_CHARGE,
        SwordPathSkillId::HeavenGateRelease => ANIM_SWORD_HEAVEN_GATE_RELEASE,
    }
}

fn sword_path_particle_for_skill(skill: SwordPathSkillId) -> &'static str {
    match skill {
        SwordPathSkillId::CondenseEdge => VFX_SWORD_CONDENSE_EDGE,
        SwordPathSkillId::QiSlash => VFX_SWORD_QI_SLASH_PATH,
        SwordPathSkillId::Resonance => VFX_SWORD_RESONANCE,
        SwordPathSkillId::Manifest => VFX_SWORD_MANIFEST_SUMMON,
        SwordPathSkillId::HeavenGateCharge => VFX_HEAVEN_GATE_CHARGE,
        SwordPathSkillId::HeavenGateRelease => VFX_HEAVEN_GATE_RELEASE,
    }
}

fn sword_path_particle_color(skill: SwordPathSkillId) -> &'static str {
    match skill {
        // 剑意凝锋——锋锐青白。
        SwordPathSkillId::CondenseEdge => "#C8D8E8",
        // 剑气斩——凝练剑气，偏冷青。
        SwordPathSkillId::QiSlash => "#A8D0E0",
        // 共鸣剑鸣——荡漾波纹白蓝。
        SwordPathSkillId::Resonance => "#D0E0F0",
        // 化形——剑意凝实，金白。
        SwordPathSkillId::Manifest => "#E0E8D0",
        // 天门——开天破虚，圣白。
        SwordPathSkillId::HeavenGateCharge | SwordPathSkillId::HeavenGateRelease => "#E8F0FF",
    }
}

fn sword_path_particle_strength(skill: SwordPathSkillId) -> f32 {
    match skill {
        SwordPathSkillId::HeavenGateRelease => 1.0,
        SwordPathSkillId::HeavenGateCharge | SwordPathSkillId::Manifest => 0.9,
        SwordPathSkillId::Resonance => 0.75,
        _ => 0.7,
    }
}

fn sword_path_particle_count_duration(skill: SwordPathSkillId) -> (u16, u16) {
    match skill {
        SwordPathSkillId::CondenseEdge => (10, 20),
        SwordPathSkillId::QiSlash => (12, 20),
        SwordPathSkillId::Resonance => (16, 30),
        SwordPathSkillId::Manifest => (14, 24),
        SwordPathSkillId::HeavenGateCharge => (12, 40),
        SwordPathSkillId::HeavenGateRelease => (24, 30),
    }
}

fn color_for_woliu_skill(skill: WoliuSkillId) -> &'static str {
    match skill {
        WoliuSkillId::Hold => "#244872",
        WoliuSkillId::Burst => "#4078A8",
        WoliuSkillId::Mouth => "#1E2440",
        WoliuSkillId::Pull => "#382058",
        WoliuSkillId::Heart => "#100818",
        WoliuSkillId::VacuumPalm => "#9966CC",
        WoliuSkillId::VortexShield => "#B48CFF",
        WoliuSkillId::VacuumLock => "#7A4CC2",
        WoliuSkillId::VortexResonance => "#8F5BE0",
        WoliuSkillId::TurbulenceBurst => "#E8D9FF",
        // plan-woliu-path-v1：虚蚀路径——深紫黑色调
        WoliuSkillId::AmbientVortex => "#9B7DB8",
        WoliuSkillId::VoidVortex => "#2D1B4E",
        WoliuSkillId::SwallowingVortex => "#7B5EA7",
        WoliuSkillId::VortexEcho => "#9B7DB8",
        WoliuSkillId::VoidCore => "#000000",
    }
}

fn woliu_particle_strength(event: &VortexCastEvent) -> f32 {
    let radius = event
        .turbulence_radius
        .max(event.influence_radius)
        .max(event.lethal_radius);
    match event.skill {
        WoliuSkillId::VortexResonance | WoliuSkillId::TurbulenceBurst => {
            (radius / 6.0).clamp(0.35, 1.0)
        }
        WoliuSkillId::Heart => (radius / 12.0).clamp(0.45, 1.0),
        _ => (radius / 8.0).clamp(0.25, 0.85),
    }
}

fn woliu_particle_count(skill: WoliuSkillId) -> u16 {
    match skill {
        WoliuSkillId::VortexResonance => 48,
        WoliuSkillId::TurbulenceBurst => 64,
        WoliuSkillId::VortexShield | WoliuSkillId::VacuumLock => 32,
        WoliuSkillId::Heart => 40,
        // AV 差异化：基础 5 招粒子数体现语义体量
        // 持涡=稀疏维持 / 瞬涡=密集瞬爆 / 涡口=中量漏斗 / 涡引=中量拖尾
        WoliuSkillId::Hold => 12,
        WoliuSkillId::Burst => 28,
        WoliuSkillId::Mouth => 20,
        WoliuSkillId::Pull => 24,
        // plan-woliu-path-v1：虚蚀路径粒子数
        WoliuSkillId::VoidVortex => 48,
        WoliuSkillId::SwallowingVortex | WoliuSkillId::VoidCore => 32,
        WoliuSkillId::AmbientVortex => 6,
        WoliuSkillId::VortexEcho => 4,
        // VacuumPalm 及其余未列招式
        _ => 16,
    }
}

fn woliu_particle_duration_ticks(skill: WoliuSkillId) -> u16 {
    match skill {
        WoliuSkillId::VortexResonance => 80,
        WoliuSkillId::TurbulenceBurst => 44,
        WoliuSkillId::VortexShield | WoliuSkillId::VacuumLock => 60,
        WoliuSkillId::Heart => 100,
        // AV 差异化：基础 5 招粒子时长体现节奏
        // 持涡=长驻维持 / 瞬涡=极短瞬发(200ms≈4tick 的爆发感) / 涡口=中等 / 涡引=偏短拖拽
        WoliuSkillId::Hold => 70,
        WoliuSkillId::Burst => 14,
        WoliuSkillId::Mouth => 48,
        WoliuSkillId::Pull => 30,
        // plan-woliu-path-v1：虚蚀路径粒子持续
        WoliuSkillId::VoidVortex => 80,
        WoliuSkillId::VoidCore => 60,
        WoliuSkillId::AmbientVortex => 40,
        WoliuSkillId::SwallowingVortex => 32,
        WoliuSkillId::VortexEcho => 20,
        // VacuumPalm 及其余未列招式
        _ => 42,
    }
}

fn woliu_anim_for_skill(skill: WoliuSkillId) -> &'static str {
    crate::combat::woliu_v2::skills::visual_for(skill).animation_id
}

fn emit_stop_for_entity(
    position: &Position,
    unique_id: &UniqueId,
    anim_id: &'static str,
    fade_out_ticks: u8,
    vfx_events: &mut EventWriter<VfxEventRequest>,
) {
    let origin = position.get();
    vfx_events.send(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::StopAnim {
            target_player: unique_id.0.to_string(),
            anim_id: anim_id.to_string(),
            fade_out_ticks: Some(fade_out_ticks),
        },
    ));
}

fn emit_block_decal(
    vfx_events: &mut EventWriter<VfxEventRequest>,
    event_id: &'static str,
    pos: valence::prelude::BlockPos,
    color: &'static str,
    strength: f32,
) {
    emit_spawn_particle(
        vfx_events,
        event_id,
        valence::prelude::DVec3::new(
            f64::from(pos.x) + 0.5,
            f64::from(pos.y) + 1.01,
            f64::from(pos.z) + 0.5,
        ),
        color,
        strength,
        1,
        80,
    );
}

fn emit_spawn_particle(
    vfx_events: &mut EventWriter<VfxEventRequest>,
    event_id: &'static str,
    origin: valence::prelude::DVec3,
    color: &'static str,
    strength: f32,
    count: u16,
    duration_ticks: u16,
) {
    vfx_events.send(VfxEventRequest::new(
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

/// 剑道五招的 AttackIntent source —— 动画走 `emit_sword_path_visual_triggers`，
/// 本系统跳过避免双重动画。
fn is_sword_path_source(source: AttackSource) -> bool {
    matches!(
        source,
        AttackSource::SwordPathCondenseEdge
            | AttackSource::SwordPathQiSlash
            | AttackSource::SwordPathResonance
            | AttackSource::SwordPathManifest
            | AttackSource::SwordPathHeavenGate
    )
}

fn attack_anim_for_source(source: AttackSource, kind: WoundKind) -> &'static str {
    match source {
        AttackSource::SwordCleave => ANIM_SWORD_CLEAVE,
        AttackSource::SwordThrust => ANIM_SWORD_THRUST,
        _ => attack_anim_for_wound_kind(kind),
    }
}

fn attack_anim_for_wound_kind(kind: WoundKind) -> &'static str {
    match kind {
        WoundKind::Cut => ANIM_SWORD_SLASH_DOWN,
        WoundKind::Pierce => ANIM_SWORD_STAB,
        WoundKind::Burn => ANIM_PALM_STRIKE,
        WoundKind::Blunt | WoundKind::Concussion => ANIM_FIST_PUNCH_RIGHT,
    }
}

fn breakthrough_anim_for_outcome(outcome: &BreakthroughOutcome) -> &'static str {
    let Ok(success) = &outcome.result else {
        return ANIM_BREAKTHROUGH_YINQI;
    };
    match (outcome.from, success.to) {
        (
            crate::cultivation::components::Realm::Awaken,
            crate::cultivation::components::Realm::Induce,
        ) => ANIM_BREAKTHROUGH_YINQI,
        (
            crate::cultivation::components::Realm::Induce,
            crate::cultivation::components::Realm::Condense,
        ) => ANIM_BREAKTHROUGH_NINGMAI,
        (
            crate::cultivation::components::Realm::Condense,
            crate::cultivation::components::Realm::Solidify,
        ) => ANIM_BREAKTHROUGH_GUYUAN,
        _ => ANIM_BREAKTHROUGH_TONGLING,
    }
}

fn emit_tuike_visual_for_entity(
    entity: valence::prelude::Entity,
    visual: &crate::combat::tuike_v2::events::TuikeSkillVisualPayload,
    color: &str,
    count: u16,
    players: &Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    vfx_events: &mut EventWriter<VfxEventRequest>,
) {
    let Ok((position, unique_id)) = players.get(entity) else {
        return;
    };
    let origin = position.get();
    vfx_events.send(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::PlayAnim {
            target_player: unique_id.0.to_string(),
            anim_id: visual.animation_id.clone(),
            priority: TUIKE_PRIORITY,
            fade_in_ticks: Some(2),
        },
    ));
    vfx_events.send(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id: visual.particle_id.clone(),
            origin: [origin.x, origin.y + 1.0, origin.z],
            direction: None,
            color: Some(color.to_string()),
            strength: Some(0.8),
            count: Some(count),
            duration_ticks: Some(36),
        },
    ));
}

fn emit_play_for_entity(
    entity: valence::prelude::Entity,
    anim_id: &'static str,
    priority: u16,
    fade_in_ticks: Option<u8>,
    players: &Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    vfx_events: &mut EventWriter<VfxEventRequest>,
) {
    let Ok((position, unique_id)) = players.get(entity) else {
        return;
    };
    let origin = position.get();
    vfx_events.send(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::PlayAnim {
            target_player: unique_id.0.to_string(),
            anim_id: anim_id.to_string(),
            priority,
            fade_in_ticks,
        },
    ));
}

/// plan-shield-block-v1 P1 — 向 entity 发送 `bong:shield_raise` 持续举盾动画。
/// 由 `combat::shield_block::raise_shield_handler` 调用；isLoop:true，优先级 = 战斗层中段。
pub fn emit_shield_raise_for_entity(
    entity: valence::prelude::Entity,
    players: &Query<(&valence::prelude::Position, &valence::prelude::UniqueId)>,
    vfx_events: &mut EventWriter<VfxEventRequest>,
) {
    let Ok((position, unique_id)) = players.get(entity) else {
        return;
    };
    let origin = position.get();
    vfx_events.send(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::PlayAnim {
            target_player: unique_id.0.to_string(),
            anim_id: ANIM_SHIELD_RAISE.to_string(),
            priority: COMBAT_PRIORITY,
            fade_in_ticks: Some(2),
        },
    ));
}

/// plan-shield-block-v1 P1 — 向 entity 发送 `bong:shield_raise` 停止信号，结束循环举盾动画。
/// 由 `combat::shield_block::lower_shield_handler` 和 `cleanup_shield_on_death` 调用。
/// entity 若没有 Position/UniqueId 则静默 skip（断线后实体可能已移除 Client component）。
pub fn emit_shield_stop_for_entity(
    entity: valence::prelude::Entity,
    players: &Query<(&valence::prelude::Position, &valence::prelude::UniqueId)>,
    vfx_events: &mut EventWriter<VfxEventRequest>,
) {
    let Ok((position, unique_id)) = players.get(entity) else {
        return;
    };
    emit_stop_for_entity(position, unique_id, ANIM_SHIELD_RAISE, 3, vfx_events);
}

/// plan-scroll-reading-v1 P2 — 阅读循环动画停止信号 fade_out ticks（阖卷收势，比盾牌略缓）。
/// `pub(crate)`：`client_request_handler` 的 `ScrollReadClosed` 分支复用同一常量，
/// 避免与本模块 `emit_scroll_read_stop_for_entity` 内部值各自取一份产生漂移。
pub(crate) const SCROLL_READ_ANIM_FADE_OUT_TICKS: u8 = 4;

/// plan-scroll-reading-v1 P2 — 向 entity 发送阅读循环动画停止信号（`StopAnim`）。
/// 由 `client_request_handler` 的 `ScrollReadClosed` 分支 + `scroll_open_emit` 的死亡兜底
/// 清理系统共用。`anim_id` 取自调用方持有的 `ScrollReading` marker component 快照
/// （非编译期 `&'static` 常量——区别于 `emit_shield_stop_for_entity` 固定的
/// `ANIM_SHIELD_RAISE`，因为未来其他可阅读残卷可能挂不同循环动画 id，见 §8.1 #6 复用清单）。
/// entity 若无 Position/UniqueId 则静默 skip（断线后实体已无 Client component）。
pub fn emit_scroll_read_stop_for_entity(
    entity: valence::prelude::Entity,
    anim_id: &str,
    positions: &Query<&valence::prelude::Position>,
    unique_ids: &Query<&valence::prelude::UniqueId>,
    vfx_events: &mut EventWriter<VfxEventRequest>,
) {
    let (Ok(position), Ok(unique_id)) = (positions.get(entity), unique_ids.get(entity)) else {
        return;
    };
    let origin = position.get();
    vfx_events.send(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::StopAnim {
            target_player: unique_id.0.to_string(),
            anim_id: anim_id.to_string(),
            fade_out_ticks: Some(SCROLL_READ_ANIM_FADE_OUT_TICKS),
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use valence::prelude::{App, Events, Update};
    use valence::testing::create_mock_client;

    use crate::combat::components::BodyPart;
    use crate::combat::events::AttackReach;
    use crate::cultivation::breakthrough::{BreakthroughError, BreakthroughSuccess};
    use crate::cultivation::components::Realm;

    fn spawn_player(app: &mut App, name: &str, pos: [f64; 3]) -> valence::prelude::Entity {
        let (mut bundle, _helper) = create_mock_client(name);
        bundle.player.position = Position::new(pos);
        app.world_mut().spawn(bundle).id()
    }

    fn spawn_skinned_npc_target(
        app: &mut App,
        name: &str,
        pos: [f64; 3],
    ) -> valence::prelude::Entity {
        app.world_mut()
            .spawn((
                Position::new(pos),
                UniqueId(Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes())),
            ))
            .id()
    }

    fn drain_vfx(app: &mut App) -> Vec<VfxEventRequest> {
        app.world_mut()
            .resource_mut::<Events<VfxEventRequest>>()
            .drain()
            .collect()
    }

    #[test]
    fn melee_cut_attack_emits_sword_swing_for_attacker() {
        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.add_event::<AttackIntent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_attack_animation_triggers);
        let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: None,
            issued_at_tick: 1,
            reach: AttackReach::new(1.0, 0.0),
            qi_invest: 1.0,
            wound_kind: WoundKind::Cut,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 1);
        assert_play_anim(&emitted[0], ANIM_SWORD_SLASH_DOWN, COMBAT_PRIORITY);
    }

    #[test]
    fn sword_cleave_attack_source_emits_sword_cleave_animation() {
        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.add_event::<AttackIntent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_attack_animation_triggers);
        let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: None,
            issued_at_tick: 1,
            reach: AttackReach::new(3.0, 0.0),
            qi_invest: 0.0,
            wound_kind: WoundKind::Cut,
            source: AttackSource::SwordCleave,
            debug_command: None,
        });

        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 1);
        assert_play_anim(&emitted[0], ANIM_SWORD_CLEAVE, COMBAT_PRIORITY);
    }

    #[test]
    fn burst_meridian_attack_intent_does_not_duplicate_beng_quan_animation() {
        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.add_event::<AttackIntent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_attack_animation_triggers);
        let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: None,
            issued_at_tick: 1,
            reach: AttackReach::new(1.0, 0.0),
            qi_invest: 1.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::BurstMeridian,
            debug_command: None,
        });

        app.update();

        assert!(drain_vfx(&mut app).is_empty());
    }

    #[test]
    fn skinned_npc_with_unique_id_can_receive_action_animation() {
        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.add_event::<AttackIntent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_attack_animation_triggers);
        let attacker = spawn_skinned_npc_target(&mut app, "npc:rogue-1", [0.0, 64.0, 0.0]);

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: None,
            issued_at_tick: 1,
            reach: AttackReach::new(1.0, 0.0),
            qi_invest: 1.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 1);
        assert_play_anim(&emitted[0], ANIM_FIST_PUNCH_RIGHT, COMBAT_PRIORITY);
    }

    #[test]
    fn combat_hit_emits_recoil_for_unique_id_target() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_hit_recoil_animation_triggers);
        let attacker = app.world_mut().spawn_empty().id();
        let target = spawn_player(&mut app, "Bob", [1.0, 64.0, 0.0]);

        app.world_mut().send_event(CombatEvent {
            attacker,
            target,
            resolved_at_tick: 1,
            body_part: BodyPart::Chest,
            wound_kind: WoundKind::Blunt,
            source: crate::combat::events::AttackSource::Melee,
            debug_command: false,
            physical_damage: 0.0,
            damage: 0.25,
            contam_delta: 0.0,
            description: "hit".to_string(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        });

        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 1);
        assert_play_anim(&emitted[0], ANIM_HURT_STAGGER, HIT_RECOIL_PRIORITY);
    }

    #[test]
    fn physical_combat_hit_emits_recoil_for_unique_id_target() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_hit_recoil_animation_triggers);
        let attacker = app.world_mut().spawn_empty().id();
        let target = spawn_player(&mut app, "Bob", [1.0, 64.0, 0.0]);

        app.world_mut().send_event(CombatEvent {
            attacker,
            target,
            resolved_at_tick: 1,
            body_part: BodyPart::Chest,
            wound_kind: WoundKind::Cut,
            source: crate::combat::events::AttackSource::SwordCleave,
            debug_command: false,
            physical_damage: 1.0,
            damage: 0.0,
            contam_delta: 0.0,
            description: "physical hit".to_string(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        });

        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 1);
        assert_play_anim(&emitted[0], ANIM_HURT_STAGGER, HIT_RECOIL_PRIORITY);
    }

    #[test]
    fn breakthrough_success_emits_story_animation() {
        let mut app = App::new();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_breakthrough_animation_triggers);
        let player = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

        app.world_mut().send_event(BreakthroughOutcome {
            entity: player,
            from: Realm::Awaken,
            result: Ok(BreakthroughSuccess {
                to: Realm::Induce,
                success_rate: 1.0,
                used_qi: 8.0,
            }),
        });

        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 1);
        assert_play_anim(&emitted[0], ANIM_BREAKTHROUGH_YINQI, STORY_PRIORITY);
    }

    #[test]
    fn breakthrough_failure_does_not_play_success_animation() {
        let mut app = App::new();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_breakthrough_animation_triggers);
        let player = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

        app.world_mut().send_event(BreakthroughOutcome {
            entity: player,
            from: Realm::Awaken,
            result: Err(BreakthroughError::RolledFailure { severity: 0.4 }),
        });

        app.update();

        assert!(drain_vfx(&mut app).is_empty());
    }

    #[test]
    fn woliu_vortex_resonance_visual_uses_field_scale_particles() {
        let mut app = App::new();
        app.add_event::<VortexCastEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_woliu_v2_visual_triggers);
        let caster = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

        app.world_mut().send_event(VortexCastEvent {
            caster,
            skill: WoliuSkillId::VortexResonance,
            tick: 10,
            center: valence::prelude::DVec3::new(1.0, 64.0, 2.0),
            lethal_radius: 0.0,
            influence_radius: 6.0,
            turbulence_radius: 6.0,
            absorbed_qi: 0.0,
            swirl_qi: 10.0,
            backfire_level: None,
            visual: crate::combat::woliu_v2::events::WoliuSkillVisual {
                animation_id: "bong:woliu_vortex_resonance",
                particle_id: "bong:woliu_vortex_resonance_field",
                sound_recipe_id: "woliu_vortex_resonance",
                hud_hint: "vortex_resonance",
                icon_texture: "bong:textures/gui/skill/woliu_heart.png",
            },
        });

        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 2);
        assert_play_anim(&emitted[0], "bong:woliu_vortex_resonance", WOLIU_PRIORITY);
        match &emitted[1].payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                strength,
                count,
                duration_ticks,
                ..
            } => {
                assert_eq!(event_id, "bong:woliu_vortex_resonance_field");
                assert_eq!(*strength, Some(1.0));
                assert_eq!(*count, Some(48));
                assert_eq!(*duration_ticks, Some(80));
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    #[test]
    fn woliu_looping_visual_stops_when_active_window_expires() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 10 });
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_woliu_v2_visual_stop_triggers);
        let caster = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        app.world_mut().entity_mut(caster).insert(VortexV2State {
            active_skill_kind: WoliuSkillId::VortexResonance,
            heart_passive_enabled: false,
            lethal_radius: 0.0,
            influence_radius: 6.0,
            turbulence_radius: 6.0,
            turbulence_intensity: 0.8,
            backfire_level: None,
            started_at_tick: 10,
            active_until_tick: 20,
            cooldown_until_tick: 100,
        });

        app.update();
        assert!(drain_vfx(&mut app).is_empty());

        app.world_mut().resource_mut::<CombatClock>().tick = 20;
        app.update();
        let emitted = drain_vfx(&mut app);

        assert_eq!(emitted.len(), 1);
        assert_stop_anim(
            &emitted[0],
            "bong:woliu_vortex_resonance",
            WOLIU_STOP_FADE_OUT_TICKS,
        );

        app.world_mut().resource_mut::<CombatClock>().tick = 21;
        app.update();
        assert!(
            drain_vfx(&mut app).is_empty(),
            "expected no second StopAnim because woliu lifecycle is already inactive"
        );
    }

    #[test]
    fn woliu_stop_animation_uses_same_animation_id_as_skill_visual() {
        for skill in [
            WoliuSkillId::Hold,
            WoliuSkillId::Burst,
            WoliuSkillId::Mouth,
            WoliuSkillId::Pull,
            WoliuSkillId::Heart,
            WoliuSkillId::VacuumPalm,
            WoliuSkillId::VortexShield,
            WoliuSkillId::VacuumLock,
            WoliuSkillId::VortexResonance,
            WoliuSkillId::TurbulenceBurst,
            // plan-woliu-path-v1：虚蚀路径 5 招式
            WoliuSkillId::AmbientVortex,
            WoliuSkillId::VoidVortex,
            WoliuSkillId::SwallowingVortex,
            WoliuSkillId::VortexEcho,
            WoliuSkillId::VoidCore,
        ] {
            assert_eq!(
                woliu_anim_for_skill(skill),
                crate::combat::woliu_v2::skills::visual_for(skill).animation_id,
                "expected stop animation to match play animation for {skill:?}"
            );
        }
    }

    #[test]
    fn completed_botany_harvest_emits_leaf_burst_particle() {
        let mut app = App::new();
        app.add_event::<HarvestTerminalEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_botany_harvest_visual_triggers);
        let player = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(HarvestTerminalEvent {
            client_entity: player,
            session_id: "offline:Azure".to_string(),
            target_id: "plant-1".to_string(),
            target_name: "ci_she_hao".to_string(),
            plant_kind: "ci_she_hao".to_string(),
            mode: crate::botany::components::BotanyHarvestMode::Manual,
            interrupted: false,
            completed: true,
            detail: "done".to_string(),
            target_pos: Some([10.0, 64.0, 10.0]),
            spirit_quality: 0.95,
            duration_ticks: 40,
            gathering_quality: Some(crate::gathering::quality::GatheringQuality::Perfect),
            tool_used: Some("bao_chu".to_string()),
            overflow_to_ground: false,
        });

        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 1);
        assert_spawn_particle(&emitted[0], BOTANY_HARVEST_VFX, Some(12));
        match &emitted[0].payload {
            VfxEventPayloadV1::SpawnParticle {
                color, strength, ..
            } => {
                assert_eq!(color.as_deref(), Some("#FFDD22"));
                assert_eq!(*strength, Some(0.95));
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    fn add_guangbo_visual_test_system(app: &mut App) {
        app.add_event::<GuangboTicaoPracticeEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_guangbo_ticao_visual_triggers);
    }

    #[test]
    fn guangbo_practice_emits_stretch_anim_and_happy_particle_for_skinned_player() {
        let mut app = App::new();
        add_guangbo_visual_test_system(&mut app);
        let player = spawn_skinned_npc_target(&mut app, "Stretcher", [3.0, 64.0, 9.0]);

        app.world_mut()
            .send_event(GuangboTicaoPracticeEvent { entity: player });
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            2,
            "skinned 练习者应收 PlayAnim + SpawnParticle 两条 VFX，实际 {} 条",
            emitted.len()
        );
        assert_play_anim(&emitted[0], ANIM_GUANGBO_TICAO, GUANGBO_TICAO_PRIORITY);
        assert_spawn_particle(&emitted[1], VFX_GUANGBO_TICAO_PRACTICE, Some(6));
        // 粒子在头部上方（origin.y + 1.2），颜色为温和绿。
        match &emitted[1].payload {
            VfxEventPayloadV1::SpawnParticle { origin, color, .. } => {
                assert_eq!(color.as_deref(), Some("#7FE38F"));
                assert!(
                    (origin[1] - (64.0 + 1.2)).abs() < 1e-6,
                    "练习粒子应在头部上方 y+1.2，实际 y={}",
                    origin[1]
                );
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    #[test]
    fn guangbo_ticao_uses_dedicated_anim_not_guard_raise() {
        // #4 各招专属动画：广播体操必须用专属完整套路 bong:guangbo_ticao，
        // 不得回退到此前复用的 bong:guard_raise（4tick 举臂格挡，真机"只动一下"）。
        // 防一次性修复被后续 refactor 静默还原。
        assert_eq!(
            ANIM_GUANGBO_TICAO, "bong:guangbo_ticao",
            "广播体操应发专属套路 anim id 'bong:guangbo_ticao'，实际 '{ANIM_GUANGBO_TICAO}'"
        );
        assert_ne!(
            ANIM_GUANGBO_TICAO, ANIM_GUARD_RAISE,
            "广播体操不得复用 guard_raise（举臂格挡）——那是'只动一下'的根因"
        );
    }

    #[test]
    fn guangbo_practice_without_position_emits_nothing() {
        // caster 无 Position/UniqueId（断线）→ 动画 skip + 粒子 skip（粒子也依赖 Position）。
        let mut app = App::new();
        add_guangbo_visual_test_system(&mut app);
        let player = app.world_mut().spawn_empty().id();

        app.world_mut()
            .send_event(GuangboTicaoPracticeEvent { entity: player });
        app.update();

        let emitted = drain_vfx(&mut app);
        assert!(
            emitted.is_empty(),
            "无 Position/UniqueId 的练习者不应产生任何 VFX，实际 {} 条",
            emitted.len()
        );
    }

    #[test]
    fn lingtian_completion_events_emit_plot_rune_particles() {
        let mut app = App::new();
        app.add_event::<TillCompleted>();
        app.add_event::<PlantingCompleted>();
        app.add_event::<HarvestCompleted>();
        app.add_event::<ReplenishCompleted>();
        app.add_event::<DrainQiCompleted>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_lingtian_visual_triggers);
        let player = app.world_mut().spawn_empty().id();
        let pos = valence::prelude::BlockPos::new(2, 65, 7);

        app.world_mut().send_event(TillCompleted {
            player,
            pos,
            hoe: crate::lingtian::hoe::HoeKind::Iron,
            hoe_instance_id: 1,
        });
        app.world_mut().send_event(PlantingCompleted {
            player,
            pos,
            plant_id: "ci_she_hao".to_string(),
        });
        app.world_mut().send_event(HarvestCompleted {
            player,
            pos,
            plant_id: "ci_she_hao".to_string(),
            seed_dropped: false,
        });
        app.world_mut().send_event(ReplenishCompleted {
            player,
            pos,
            source: crate::lingtian::session::ReplenishSource::Zone,
            plot_qi_added: 0.25,
            overflow_to_zone: 0.0,
        });
        app.world_mut().send_event(DrainQiCompleted {
            player,
            pos,
            plot_qi_drained: 0.5,
            qi_to_player: 0.4,
            qi_to_zone: 0.1,
        });

        app.update();

        let ids: Vec<_> = drain_vfx(&mut app)
            .into_iter()
            .map(|req| match req.payload {
                VfxEventPayloadV1::SpawnParticle { event_id, .. } => event_id,
                other => panic!("expected SpawnParticle, got {other:?}"),
            })
            .collect();
        assert_eq!(
            ids,
            vec![
                LINGTIAN_TILL_VFX,
                LINGTIAN_PLANT_VFX,
                LINGTIAN_HARVEST_VFX,
                LINGTIAN_REPLENISH_VFX,
                LINGTIAN_DRAIN_VFX,
            ]
        );
    }

    #[test]
    fn tuike_v2_skill_events_emit_animation_and_particle_pairs() {
        let mut app = App::new();
        add_tuike_v2_visual_test_system(&mut app);
        let player = spawn_player(&mut app, "Azure", [1.0, 64.0, 2.0]);

        app.world_mut().send_event(DonFalseSkinEvent {
            caster: player,
            tier: crate::combat::tuike_v2::FalseSkinTier::Light,
            layers_after: 1,
            tick: 10,
            visual: tuike_visual(crate::combat::tuike_v2::TuikeSkillId::Don, false),
        });
        app.world_mut().send_event(FalseSkinSheddedEvent {
            owner: player,
            attacker: None,
            tier: crate::combat::tuike_v2::FalseSkinTier::Mid,
            damage_absorbed: 20.0,
            damage_overflow: 0.0,
            contam_load: 0.0,
            permanent_taint_load: 0.0,
            layers_after: 0,
            active: true,
            tick: 11,
            visual: tuike_visual(crate::combat::tuike_v2::TuikeSkillId::Shed, false),
        });
        app.world_mut().send_event(ContamTransferredEvent {
            caster: player,
            tier: crate::combat::tuike_v2::FalseSkinTier::Ancient,
            contam_moved_percent: 15.0,
            backflow_percent: 0.0,
            permanent_absorbed: 0.0,
            qi_cost: 105.0,
            tick: 12,
            visual: tuike_visual(crate::combat::tuike_v2::TuikeSkillId::TransferTaint, false),
        });

        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 6);
        assert_tuike_pair(
            &emitted[0..2],
            "bong:tuike_don_skin",
            "bong:false_skin_don_dust",
            Some(10),
        );
        assert_tuike_pair(
            &emitted[2..4],
            "bong:tuike_shed_burst",
            "bong:false_skin_shed_burst",
            Some(18),
        );
        assert_tuike_pair(
            &emitted[4..6],
            "bong:tuike_taint_transfer",
            "bong:false_skin_don_dust",
            Some(12),
        );
    }

    #[test]
    fn tuike_v2_visual_trigger_ignores_non_player_entities() {
        let mut app = App::new();
        add_tuike_v2_visual_test_system(&mut app);
        let non_player = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(DonFalseSkinEvent {
            caster: non_player,
            tier: crate::combat::tuike_v2::FalseSkinTier::Light,
            layers_after: 1,
            tick: 10,
            visual: tuike_visual(crate::combat::tuike_v2::TuikeSkillId::Don, false),
        });

        app.update();

        assert!(drain_vfx(&mut app).is_empty());
    }

    #[test]
    fn tuike_v2_visual_trigger_colors_permanent_shed_and_transfer_branches() {
        let mut app = App::new();
        add_tuike_v2_visual_test_system(&mut app);
        let player = spawn_player(&mut app, "Azure", [1.0, 64.0, 2.0]);

        app.world_mut().send_event(FalseSkinSheddedEvent {
            owner: player,
            attacker: None,
            tier: crate::combat::tuike_v2::FalseSkinTier::Ancient,
            damage_absorbed: 20.0,
            damage_overflow: 0.0,
            contam_load: 0.0,
            permanent_taint_load: 0.5,
            layers_after: 0,
            active: true,
            tick: 11,
            visual: tuike_visual(crate::combat::tuike_v2::TuikeSkillId::Shed, true),
        });
        app.world_mut().send_event(ContamTransferredEvent {
            caster: player,
            tier: crate::combat::tuike_v2::FalseSkinTier::Ancient,
            contam_moved_percent: 15.0,
            backflow_percent: 0.0,
            permanent_absorbed: 0.5,
            qi_cost: 105.0,
            tick: 12,
            visual: tuike_visual(crate::combat::tuike_v2::TuikeSkillId::TransferTaint, true),
        });

        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 4);
        assert_spawn_particle_color(&emitted[1], "#BFD8FF");
        assert_spawn_particle_color(&emitted[3], "#9EC7FF");
    }

    fn assert_play_anim(request: &VfxEventRequest, expected_anim: &str, expected_priority: u16) {
        match &request.payload {
            VfxEventPayloadV1::PlayAnim {
                anim_id, priority, ..
            } => {
                assert_eq!(anim_id, expected_anim);
                assert_eq!(*priority, expected_priority);
            }
            other => panic!("expected PlayAnim, got {other:?}"),
        }
    }

    fn assert_stop_anim(request: &VfxEventRequest, expected_anim: &str, expected_fade: u8) {
        match &request.payload {
            VfxEventPayloadV1::StopAnim {
                anim_id,
                fade_out_ticks,
                ..
            } => {
                assert_eq!(anim_id, expected_anim);
                assert_eq!(*fade_out_ticks, Some(expected_fade));
            }
            other => panic!("expected StopAnim, got {other:?}"),
        }
    }

    fn assert_spawn_particle(
        request: &VfxEventRequest,
        expected_event_id: &str,
        expected_count: Option<u16>,
    ) {
        match &request.payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id, count, ..
            } => {
                assert_eq!(event_id, expected_event_id);
                assert_eq!(*count, expected_count);
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    fn add_tuike_v2_visual_test_system(app: &mut App) {
        app.add_event::<DonFalseSkinEvent>();
        app.add_event::<FalseSkinSheddedEvent>();
        app.add_event::<ContamTransferredEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_tuike_v2_visual_triggers);
    }

    fn tuike_visual(
        skill: crate::combat::tuike_v2::TuikeSkillId,
        ancient: bool,
    ) -> crate::combat::tuike_v2::events::TuikeSkillVisualPayload {
        crate::combat::tuike_v2::TuikeSkillVisual::for_skill(skill, ancient).into()
    }

    fn assert_tuike_pair(
        requests: &[VfxEventRequest],
        expected_anim: &str,
        expected_particle: &str,
        expected_count: Option<u16>,
    ) {
        assert_eq!(requests.len(), 2);
        assert_play_anim(&requests[0], expected_anim, TUIKE_PRIORITY);
        assert_spawn_particle(&requests[1], expected_particle, expected_count);
    }

    fn assert_spawn_particle_color(request: &VfxEventRequest, expected_color: &str) {
        match &request.payload {
            VfxEventPayloadV1::SpawnParticle { color, .. } => {
                assert_eq!(color.as_deref(), Some(expected_color));
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    // ─── plan-sword-path-v2 P4：emit_sword_path_visual_triggers ───

    fn sword_path_cast(
        skill: SwordPathSkillId,
        caster: valence::prelude::Entity,
        center: valence::prelude::DVec3,
        direction: Option<valence::prelude::DVec3>,
    ) -> SwordPathSkillCastEvent {
        SwordPathSkillCastEvent {
            skill,
            caster,
            center,
            direction,
            tick: 10,
        }
    }

    fn setup_sword_path_visual_app() -> App {
        let mut app = App::new();
        app.add_event::<SwordPathSkillCastEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_sword_path_visual_triggers);
        app
    }

    /// 凝锋 → 基础横劈动画（SWORD_PATH_PRIORITY）+ 专属粒子。
    #[test]
    fn condense_edge_emits_cleave_anim_and_dedicated_particle() {
        let mut app = setup_sword_path_visual_app();
        let caster = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
        app.world_mut().send_event(sword_path_cast(
            SwordPathSkillId::CondenseEdge,
            caster,
            valence::prelude::DVec3::new(0.0, 64.0, 0.0),
            None,
        ));
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            2,
            "凝锋应发 1 动画 + 1 粒子，实际 {}",
            emitted.len()
        );
        assert_play_anim(&emitted[0], ANIM_SWORD_CLEAVE, SWORD_PATH_PRIORITY);
        assert_spawn_particle(&emitted[1], VFX_SWORD_CONDENSE_EDGE, Some(10));
    }

    /// 剑气斩 → 基础刺击动画 + 朝向 line trail 粒子（direction 透传）。
    #[test]
    fn qi_slash_emits_thrust_anim_and_directional_particle() {
        let mut app = setup_sword_path_visual_app();
        let caster = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
        let dir = valence::prelude::DVec3::new(0.0, 0.0, 1.0);
        app.world_mut().send_event(sword_path_cast(
            SwordPathSkillId::QiSlash,
            caster,
            valence::prelude::DVec3::new(0.0, 64.0, 0.0),
            Some(dir),
        ));
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 2);
        assert_play_anim(&emitted[0], ANIM_SWORD_THRUST, SWORD_PATH_PRIORITY);
        match &emitted[1].payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                direction,
                ..
            } => {
                assert_eq!(event_id, VFX_SWORD_QI_SLASH_PATH);
                assert_eq!(
                    *direction,
                    Some([0.0, 0.0, 1.0]),
                    "剑气斩 line trail 必须透传 caster→target 方向，否则粒子朝向退化"
                );
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    /// 化形 → 专属 manifest_cast 动画（不是基础剑斩）+ 召唤粒子。
    #[test]
    fn manifest_emits_dedicated_cast_anim() {
        let mut app = setup_sword_path_visual_app();
        let caster = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
        app.world_mut().send_event(sword_path_cast(
            SwordPathSkillId::Manifest,
            caster,
            valence::prelude::DVec3::new(0.0, 64.0, 0.0),
            None,
        ));
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 2);
        assert_play_anim(&emitted[0], ANIM_SWORD_MANIFEST_CAST, SWORD_PATH_PRIORITY);
        assert_spawn_particle(&emitted[1], VFX_SWORD_MANIFEST_SUMMON, Some(14));
    }

    /// 天门蓄力 → charge 动画 + charge 粒子（无额外 flash）。
    #[test]
    fn heaven_gate_charge_emits_charge_anim_only() {
        let mut app = setup_sword_path_visual_app();
        let caster = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
        app.world_mut().send_event(sword_path_cast(
            SwordPathSkillId::HeavenGateCharge,
            caster,
            valence::prelude::DVec3::new(0.0, 64.0, 0.0),
            None,
        ));
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            2,
            "蓄力应只发 1 动画 + 1 粒子（无 flash），实际 {}",
            emitted.len()
        );
        assert_play_anim(
            &emitted[0],
            ANIM_SWORD_HEAVEN_GATE_CHARGE,
            SWORD_PATH_PRIORITY,
        );
        assert_spawn_particle(&emitted[1], VFX_HEAVEN_GATE_CHARGE, Some(12));
    }

    /// 天门释放 → release 动画 + release 粒子 + 额外开天 flash 粒子（三件）。
    #[test]
    fn heaven_gate_release_emits_release_anim_plus_flash_layer() {
        let mut app = setup_sword_path_visual_app();
        let caster = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
        app.world_mut().send_event(sword_path_cast(
            SwordPathSkillId::HeavenGateRelease,
            caster,
            valence::prelude::DVec3::new(0.0, 64.0, 0.0),
            None,
        ));
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            3,
            "释放应发 1 动画 + release 粒子 + flash 粒子，实际 {}",
            emitted.len()
        );
        assert_play_anim(
            &emitted[0],
            ANIM_SWORD_HEAVEN_GATE_RELEASE,
            SWORD_PATH_PRIORITY,
        );
        assert_spawn_particle(&emitted[1], VFX_HEAVEN_GATE_RELEASE, Some(24));
        assert_spawn_particle(&emitted[2], VFX_HEAVEN_GATE_FLASH, Some(24));
    }

    /// caster 无 UniqueId（非 skinned）→ 动画静默 skip，但粒子仍按 center 发出。
    #[test]
    fn sword_path_visual_without_unique_id_still_emits_particle() {
        let mut app = setup_sword_path_visual_app();
        // spawn 一个无 UniqueId 的 caster（emit_play_for_entity 会 skip 动画）
        let caster = app.world_mut().spawn(Position::new([5.0, 64.0, 5.0])).id();
        app.world_mut().send_event(sword_path_cast(
            SwordPathSkillId::Resonance,
            caster,
            valence::prelude::DVec3::new(5.0, 64.0, 5.0),
            None,
        ));
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "无 UniqueId 时只发粒子（动画 skip），实际 {}",
            emitted.len()
        );
        assert_spawn_particle(&emitted[0], VFX_SWORD_RESONANCE, Some(16));
    }

    /// 凝锋 / 剑气斩 source 的 AttackIntent 不再走 emit_attack_animation_triggers
    /// （避免与 emit_sword_path_visual_triggers 双重动画）。
    #[test]
    fn attack_animation_skips_sword_path_sources() {
        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.add_event::<AttackIntent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_attack_animation_triggers);
        let attacker = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);

        for source in [
            AttackSource::SwordPathCondenseEdge,
            AttackSource::SwordPathQiSlash,
            AttackSource::SwordPathResonance,
            AttackSource::SwordPathManifest,
            AttackSource::SwordPathHeavenGate,
        ] {
            app.world_mut().send_event(AttackIntent {
                attacker,
                target: None,
                issued_at_tick: 1,
                reach: crate::combat::events::AttackReach::new(3.0, 0.0),
                qi_invest: 1.0,
                wound_kind: WoundKind::Cut,
                source,
                debug_command: None,
            });
        }
        app.update();

        assert!(
            drain_vfx(&mut app).is_empty(),
            "剑道 source 的 AttackIntent 不应触发通用攻击动画——动画由 \
             emit_sword_path_visual_triggers 独立负责"
        );
    }

    /// 回归保护：非剑道 source（基础剑劈）仍走 emit_attack_animation_triggers。
    #[test]
    fn attack_animation_still_fires_for_basic_sword_cleave() {
        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.add_event::<AttackIntent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_attack_animation_triggers);
        let attacker = spawn_player(&mut app, "Azure", [0.0, 64.0, 0.0]);
        app.world_mut().send_event(AttackIntent {
            attacker,
            target: None,
            issued_at_tick: 1,
            reach: crate::combat::events::AttackReach::new(3.0, 0.0),
            qi_invest: 0.0,
            wound_kind: WoundKind::Cut,
            source: AttackSource::SwordCleave,
            debug_command: None,
        });
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 1, "基础剑劈 source 仍应触发通用攻击动画");
        assert_play_anim(&emitted[0], ANIM_SWORD_CLEAVE, COMBAT_PRIORITY);
    }

    // ─── plan-shield-block-v1 P1 CR#5：emit_shield_raise_for_entity / emit_shield_stop_for_entity ───

    /// Wrapper systems so we can use add_systems(Update, ...) instead of run_system_once,
    /// which is the pattern already established in this test module.
    fn shield_raise_system_for(
        entity: valence::prelude::Entity,
    ) -> impl Fn(Query<'_, '_, (&Position, &UniqueId)>, EventWriter<'_, VfxEventRequest>) {
        move |players, mut vfx_events| {
            emit_shield_raise_for_entity(entity, &players, &mut vfx_events);
        }
    }

    fn shield_stop_system_for(
        entity: valence::prelude::Entity,
    ) -> impl Fn(Query<'_, '_, (&Position, &UniqueId)>, EventWriter<'_, VfxEventRequest>) {
        move |players, mut vfx_events| {
            emit_shield_stop_for_entity(entity, &players, &mut vfx_events);
        }
    }

    fn setup_shield_vfx_app_for(
        entity_fn: impl FnOnce(&mut App) -> valence::prelude::Entity,
    ) -> (App, valence::prelude::Entity) {
        let mut app = App::new();
        app.add_event::<VfxEventRequest>();
        let entity = entity_fn(&mut app);
        (app, entity)
    }

    /// CR#5 — 有效实体（携带 Position + UniqueId）触发 emit_shield_raise_for_entity：
    /// 断言发出 PlayAnim，anim_id == ANIM_SHIELD_RAISE，priority == COMBAT_PRIORITY，fade_in_ticks == Some(2)。
    #[test]
    fn emit_shield_raise_for_valid_entity_sends_play_anim_with_combat_priority() {
        let (mut app, entity) = setup_shield_vfx_app_for(|app| {
            spawn_skinned_npc_target(app, "shield:alice", [0.0, 64.0, 0.0])
        });
        app.add_systems(Update, shield_raise_system_for(entity));
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "emit_shield_raise_for_entity must emit exactly 1 VfxEventRequest for valid entity; actual={}",
            emitted.len()
        );
        match &emitted[0].payload {
            VfxEventPayloadV1::PlayAnim {
                anim_id,
                priority,
                fade_in_ticks,
                ..
            } => {
                assert_eq!(
                    anim_id, ANIM_SHIELD_RAISE,
                    "emit_shield_raise_for_entity must use ANIM_SHIELD_RAISE='{ANIM_SHIELD_RAISE}'; actual='{anim_id}'"
                );
                assert_eq!(
                    *priority, COMBAT_PRIORITY,
                    "emit_shield_raise_for_entity priority must equal COMBAT_PRIORITY={COMBAT_PRIORITY}; actual={priority}"
                );
                assert_eq!(
                    *fade_in_ticks,
                    Some(2),
                    "emit_shield_raise_for_entity fade_in_ticks must be Some(2); actual={fade_in_ticks:?}"
                );
            }
            other => panic!("expected PlayAnim from emit_shield_raise_for_entity, got {other:?}"),
        }
    }

    /// CR#5 — emit_shield_raise_for_entity 无效实体（缺 Position/UniqueId 组件）→ 静默 skip，不 panic，不 emit。
    #[test]
    fn emit_shield_raise_for_missing_entity_silently_skips() {
        let (mut app, missing_entity) = setup_shield_vfx_app_for(|app| {
            // spawn_empty：无 Position / UniqueId，模拟断线后实体已移除 Client component
            app.world_mut().spawn_empty().id()
        });
        app.add_systems(Update, shield_raise_system_for(missing_entity));
        // 不应 panic；entity 缺组件时 `let Ok(...) = players.get(entity)` 走 return 分支
        app.update();

        let emitted = drain_vfx(&mut app);
        assert!(
            emitted.is_empty(),
            "emit_shield_raise_for_entity must silently skip when entity lacks Position/UniqueId; actual emitted={}",
            emitted.len()
        );
    }

    /// CR#5 — 有效实体触发 emit_shield_stop_for_entity：
    /// 断言发出 StopAnim，anim_id == ANIM_SHIELD_RAISE，fade_out_ticks == Some(3)。
    #[test]
    fn emit_shield_stop_for_valid_entity_sends_stop_anim() {
        let (mut app, entity) = setup_shield_vfx_app_for(|app| {
            spawn_skinned_npc_target(app, "shield:bob", [1.0, 64.0, 0.0])
        });
        app.add_systems(Update, shield_stop_system_for(entity));
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "emit_shield_stop_for_entity must emit exactly 1 VfxEventRequest for valid entity; actual={}",
            emitted.len()
        );
        // fade_out=3 は emit_stop_for_entity 内の固定値
        assert_stop_anim(&emitted[0], ANIM_SHIELD_RAISE, 3);
    }

    /// CR#5 — emit_shield_stop_for_entity 无效实体（缺 Position/UniqueId 组件）→ 静默 skip，不 panic，不 emit。
    #[test]
    fn emit_shield_stop_for_missing_entity_silently_skips() {
        let (mut app, missing_entity) =
            setup_shield_vfx_app_for(|app| app.world_mut().spawn_empty().id());
        app.add_systems(Update, shield_stop_system_for(missing_entity));
        app.update();

        let emitted = drain_vfx(&mut app);
        assert!(
            emitted.is_empty(),
            "emit_shield_stop_for_entity must silently skip when entity lacks Position/UniqueId; actual emitted={}",
            emitted.len()
        );
    }

    /// CR#5 — emit_shield_raise_for_entity anim_id 与 ANIM_SHIELD_RAISE 常量一致（不是 guard_raise 等旧常量）。
    #[test]
    fn emit_shield_raise_anim_id_is_distinct_from_guard_raise() {
        // 确认 ANIM_SHIELD_RAISE != ANIM_GUARD_RAISE，两者语义不同
        assert_ne!(
            ANIM_SHIELD_RAISE, ANIM_GUARD_RAISE,
            "ANIM_SHIELD_RAISE and ANIM_GUARD_RAISE must be different animation ids; \
             shield_raise is a persistent looping block anim, guard_raise is FullPowerCharge"
        );
    }

    // ─── AV r3-P3#3：渡劫成功 VFX ───

    fn make_tribulation_settled(
        entity: valence::prelude::Entity,
        outcome: DuXuOutcomeV1,
    ) -> TribulationSettled {
        use crate::cultivation::tribulation::TribulationKind;
        use crate::schema::tribulation::DuXuResultV1;
        TribulationSettled {
            entity,
            kind: TribulationKind::DuXu,
            source: None,
            result: DuXuResultV1 {
                char_id: "test_char".to_string(),
                outcome,
                killer: None,
                waves_survived: 3,
                reason: None,
            },
        }
    }

    #[test]
    fn tribulation_ascended_settled_emits_breakthrough_pillar_and_tongling_animation() {
        // Expect: outcome=Ascended → PlayAnim(breakthrough_tongling, STORY_PRIORITY) +
        //         SpawnParticle(bong:breakthrough_pillar, count=16).
        let mut app = App::new();
        app.add_event::<TribulationSettled>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_tribulation_settled_vfx_triggers);
        let player = spawn_player(&mut app, "Alice", [5.0, 64.0, 5.0]);

        app.world_mut()
            .send_event(make_tribulation_settled(player, DuXuOutcomeV1::Ascended));
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            2,
            "Ascended outcome must emit exactly 2 VFX events (PlayAnim + SpawnParticle), got {}",
            emitted.len()
        );
        assert_play_anim(&emitted[0], ANIM_BREAKTHROUGH_TONGLING, STORY_PRIORITY);
        assert_spawn_particle(&emitted[1], "bong:breakthrough_pillar", Some(16));
    }

    #[test]
    fn tribulation_halfstep_settled_emits_breakthrough_pillar_and_guyuan_animation() {
        // Expect: outcome=HalfStep → PlayAnim(breakthrough_guyuan, STORY_PRIORITY) +
        //         SpawnParticle(bong:breakthrough_pillar, count=10, 略低于 Ascended).
        let mut app = App::new();
        app.add_event::<TribulationSettled>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_tribulation_settled_vfx_triggers);
        let player = spawn_player(&mut app, "Bob", [5.0, 64.0, 5.0]);

        app.world_mut()
            .send_event(make_tribulation_settled(player, DuXuOutcomeV1::HalfStep));
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            2,
            "HalfStep outcome must emit exactly 2 VFX events (PlayAnim + SpawnParticle), got {}",
            emitted.len()
        );
        assert_play_anim(&emitted[0], ANIM_BREAKTHROUGH_GUYUAN, STORY_PRIORITY);
        assert_spawn_particle(&emitted[1], "bong:breakthrough_pillar", Some(10));
    }

    #[test]
    fn tribulation_failed_settled_does_not_emit_success_vfx() {
        // Expect: outcome=Failed → no VFX (failure handled by TribulationFailed, not TribulationSettled).
        let mut app = App::new();
        app.add_event::<TribulationSettled>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_tribulation_settled_vfx_triggers);
        let player = spawn_player(&mut app, "Charlie", [5.0, 64.0, 5.0]);

        app.world_mut()
            .send_event(make_tribulation_settled(player, DuXuOutcomeV1::Failed));
        app.update();

        let emitted = drain_vfx(&mut app);
        assert!(
            emitted.is_empty(),
            "Failed outcome must not emit success VFX (handled by TribulationFailed reader), got {:?}",
            emitted.len()
        );
    }

    #[test]
    fn tribulation_killed_fled_settled_does_not_emit_success_vfx() {
        // Expect: outcome=Killed/Fled → no VFX (not a success).
        let mut app = App::new();
        app.add_event::<TribulationSettled>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_tribulation_settled_vfx_triggers);
        let player = spawn_player(&mut app, "Dave", [5.0, 64.0, 5.0]);

        app.world_mut()
            .send_event(make_tribulation_settled(player, DuXuOutcomeV1::Killed));
        app.world_mut()
            .send_event(make_tribulation_settled(player, DuXuOutcomeV1::Fled));
        app.update();

        let emitted = drain_vfx(&mut app);
        assert!(
            emitted.is_empty(),
            "Killed/Fled outcomes must not emit success VFX, got {}",
            emitted.len()
        );
    }

    #[test]
    fn existing_tribulation_failure_animation_unaffected_by_settled_system() {
        // Regression guard: emit_tribulation_animation_triggers still fires hurt_stagger for
        // TribulationFailed events after adding the separate settled system.
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationFailed>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_tribulation_animation_triggers);
        let player = spawn_player(&mut app, "Eve", [5.0, 64.0, 5.0]);

        use crate::cultivation::tribulation::TribulationFailed;
        app.world_mut().send_event(TribulationFailed {
            entity: player,
            wave: 2,
        });
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "TribulationFailed must still emit exactly one VFX (hurt_stagger), got {}",
            emitted.len()
        );
        assert_play_anim(&emitted[0], ANIM_HURT_STAGGER, HIT_RECOIL_PRIORITY);
    }

    // ─── 暗器六招：emit_anqi_visual_triggers ──────────────────────

    fn setup_anqi_visual_app() -> App {
        let mut app = App::new();
        app.add_event::<CarrierChargedEvent>();
        app.add_event::<QiInjectionEvent>();
        app.add_event::<MultiShotEvent>();
        app.add_event::<ArmorPierceEvent>();
        app.add_event::<EchoFractalEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_anqi_visual_triggers);
        app
    }

    fn anqi_injection_outcome() -> crate::qi_physics::HighDensityInjectionOutcome {
        crate::qi_physics::HighDensityInjectionOutcome {
            payload_qi: 50.0,
            wound_qi: 40.0,
            contamination_qi: 5.0,
            overload_ratio: 0.5,
            triggers_overload_tear: false,
        }
    }

    /// 封骨充能：windup_charge 动画 + 封骨密封粒子。
    #[test]
    fn anqi_charge_emits_windup_anim_and_seal_particle() {
        use crate::cultivation::components::ColorKind;
        let mut app = setup_anqi_visual_app();
        let caster = spawn_player(&mut app, "Carry", [0.0, 64.0, 0.0]);
        app.world_mut().send_event(CarrierChargedEvent {
            carrier: caster,
            instance_id: 1,
            qi_amount: 25.0,
            qi_color: ColorKind::Solid,
            full_charge: true,
            tick: 10,
        });
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 2, "封骨应 emit 1 动画 + 1 粒子");
        assert_play_anim(&emitted[0], ANIM_ANQI_CHARGE, ANQI_PRIORITY);
        assert_spawn_particle(&emitted[1], VFX_ANQI_CHARGE_SEAL, Some(12));
    }

    /// 单射狙击：sword_stab 动画 + 弹道粒子（caster→target 方向 +Z）。
    #[test]
    fn anqi_snipe_emits_directional_bolt() {
        use crate::combat::anqi_v2::AnqiSkillId;
        use crate::combat::carrier::CarrierKind;
        let mut app = setup_anqi_visual_app();
        let caster = spawn_player(&mut app, "Sniper", [0.0, 64.0, 0.0]);
        let target = spawn_skinned_npc_target(&mut app, "Mark", [0.0, 64.0, 5.0]);
        app.world_mut().send_event(QiInjectionEvent {
            caster,
            target: Some(target),
            skill: AnqiSkillId::SingleSnipe,
            carrier_kind: CarrierKind::YibianShougu,
            outcome: anqi_injection_outcome(),
            tick: 11,
        });
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 2, "单射应 emit 1 动画 + 1 粒子");
        assert_play_anim(&emitted[0], ANIM_ANQI_SNIPE, ANQI_PRIORITY);
        match &emitted[1].payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                direction,
                ..
            } => {
                assert_eq!(event_id, VFX_ANQI_SNIPE_BOLT);
                let dir = direction.expect("单射弹道必须带 caster→target 方向");
                assert!(
                    (dir[2] - 1.0).abs() < 1e-6 && dir[0].abs() < 1e-6,
                    "方向应为 +Z 单位向量（caster→target），实际 {dir:?}"
                );
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    /// 凝魂注射：cast_invoke 动画 + 魂注紫雾（无方向）。
    #[test]
    fn anqi_soul_inject_emits_cast_invoke_and_mist() {
        use crate::combat::anqi_v2::AnqiSkillId;
        use crate::combat::carrier::CarrierKind;
        let mut app = setup_anqi_visual_app();
        let caster = spawn_player(&mut app, "Soul", [0.0, 64.0, 0.0]);
        app.world_mut().send_event(QiInjectionEvent {
            caster,
            target: None,
            skill: AnqiSkillId::SoulInject,
            carrier_kind: CarrierKind::DyedBone,
            outcome: anqi_injection_outcome(),
            tick: 12,
        });
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 2);
        assert_play_anim(&emitted[0], ANIM_ANQI_INJECT, ANQI_PRIORITY);
        assert_spawn_particle(&emitted[1], VFX_ANQI_SOUL_INJECT, Some(16));
    }

    /// 多发齐射：release_burst 动画 + 扇形齐射粒子（粒子数随弹数缩放）。
    #[test]
    fn anqi_multi_shot_scales_particle_count_with_projectiles() {
        use crate::combat::carrier::CarrierKind;
        let mut app = setup_anqi_visual_app();
        let caster = spawn_player(&mut app, "Volley", [0.0, 64.0, 0.0]);
        app.world_mut().send_event(MultiShotEvent {
            caster,
            projectile_count: 5,
            carrier_kind: CarrierKind::LingmuArrow,
            shots: Vec::new(),
            tick: 13,
        });
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 2);
        assert_play_anim(&emitted[0], ANIM_ANQI_VOLLEY, ANQI_PRIORITY);
        // 5 发 × 4 = 20 颗（在 clamp [8,40] 内）。
        assert_spawn_particle(&emitted[1], VFX_ANQI_MULTI_VOLLEY, Some(20));
    }

    /// 破甲注射：cast_invoke 动画 + 破甲火花（caster→target 方向）。
    #[test]
    fn anqi_armor_pierce_emits_directional_sparks() {
        use crate::combat::carrier::CarrierKind;
        let mut app = setup_anqi_visual_app();
        let caster = spawn_player(&mut app, "Pierce", [0.0, 64.0, 0.0]);
        let target = spawn_skinned_npc_target(&mut app, "Armor", [5.0, 64.0, 0.0]);
        app.world_mut().send_event(ArmorPierceEvent {
            caster,
            target: Some(target),
            carrier_kind: CarrierKind::FenglingheBone,
            outcome: crate::qi_physics::ArmorPenetrationOutcome {
                base_damage: 60.0,
                ignored_defense_ratio: 0.6,
                effective_damage: 70.0,
                carrier_shatter_probability: 0.2,
            },
            tick: 14,
        });
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 2);
        assert_play_anim(&emitted[0], ANIM_ANQI_INJECT, ANQI_PRIORITY);
        match &emitted[1].payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                direction,
                ..
            } => {
                assert_eq!(event_id, VFX_ANQI_ARMOR_PIERCE);
                let dir = direction.expect("破甲火花必须带 caster→target 方向");
                assert!(
                    (dir[0] - 1.0).abs() < 1e-6,
                    "方向应为 +X 单位向量（caster→target），实际 {dir:?}"
                );
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    /// 诱饵分形：release_burst 动画 + 分形回响涟漪（粒子随分身数缩放）。
    #[test]
    fn anqi_echo_fractal_emits_decoy_ripple() {
        use crate::combat::carrier::CarrierKind;
        let mut app = setup_anqi_visual_app();
        let caster = spawn_player(&mut app, "Echo", [0.0, 64.0, 0.0]);
        app.world_mut().send_event(EchoFractalEvent {
            caster,
            carrier_kind: CarrierKind::ShangguBone,
            outcome: crate::qi_physics::EchoFractalOutcome {
                local_qi_density: 9.0,
                threshold: 0.3,
                echo_count: 4,
                damage_per_echo: 2.0,
            },
            tick: 15,
        });
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 2);
        assert_play_anim(&emitted[0], ANIM_ANQI_ECHO, ANQI_PRIORITY);
        // 4 分身 × 5 = 20 颗（在 clamp [10,40] 内）。
        assert_spawn_particle(&emitted[1], VFX_ANQI_ECHO_DECOY, Some(20));
    }

    /// caster 无 Position（被拒绝/异常态）→ 封骨不 emit（守纯加法不污染）。
    #[test]
    fn anqi_charge_without_position_emits_nothing() {
        use crate::cultivation::components::ColorKind;
        let mut app = setup_anqi_visual_app();
        let caster = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(CarrierChargedEvent {
            carrier: caster,
            instance_id: 1,
            qi_amount: 25.0,
            qi_color: ColorKind::Solid,
            full_charge: true,
            tick: 10,
        });
        app.update();
        assert!(
            drain_vfx(&mut app).is_empty(),
            "caster 无 Position 时封骨 VFX 应静默 skip（纯 cosmetic 不强行渲染）"
        );
    }

    // ─── 蛊道两招（凝针 / 灌毒蛊）emit_dugu_needle_visual_triggers ───

    fn setup_dugu_visual_app() -> App {
        let mut app = App::new();
        app.add_event::<QiNeedleChargedEvent>();
        app.add_event::<DuguObfuscationDisruptedEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_dugu_needle_visual_triggers);
        app
    }

    /// 凝针：dugu_needle_throw 动画 + 朝向弹道粒子（caster→target 方向 +Z）。
    #[test]
    fn dugu_shoot_needle_emits_directional_bolt() {
        let mut app = setup_dugu_visual_app();
        let shooter = spawn_player(&mut app, "Needle", [0.0, 64.0, 0.0]);
        let target = spawn_skinned_npc_target(&mut app, "Victim", [0.0, 64.0, 6.0]);
        app.world_mut().send_event(QiNeedleChargedEvent {
            shooter,
            target: Some(target),
            tick: 7,
        });
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 2, "凝针应 emit 1 动画 + 1 弹道粒子");
        assert_play_anim(&emitted[0], ANIM_DUGU_NEEDLE_THROW, DUGU_PRIORITY);
        match &emitted[1].payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                direction,
                ..
            } => {
                assert_eq!(
                    event_id, VFX_DUGU_NEEDLE_BOLT,
                    "凝针弹道 event_id 必须与 client DuguNeedleVfxPlayer.DUGU_NEEDLE_BOLT 对齐"
                );
                let dir = direction.expect("凝针弹道必须带 caster→target 方向（细针直刺）");
                assert!(
                    (dir[2] - 1.0).abs() < 1e-6 && dir[0].abs() < 1e-6,
                    "方向应为 +Z 单位向量（caster→target），实际 {dir:?}"
                );
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    /// 凝针无目标：方向退化到 +X fallback（仍 emit，避免远程招无反馈）。
    #[test]
    fn dugu_shoot_needle_without_target_falls_back_to_default_direction() {
        let mut app = setup_dugu_visual_app();
        let shooter = spawn_player(&mut app, "NeedleNoTgt", [0.0, 64.0, 0.0]);
        app.world_mut().send_event(QiNeedleChargedEvent {
            shooter,
            target: None,
            tick: 8,
        });
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 2);
        assert_play_anim(&emitted[0], ANIM_DUGU_NEEDLE_THROW, DUGU_PRIORITY);
        match &emitted[1].payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                direction,
                ..
            } => {
                assert_eq!(event_id, VFX_DUGU_NEEDLE_BOLT);
                let dir = direction.expect("无目标仍透传方向（fallback）");
                assert!(
                    (dir[0] - 1.0).abs() < 1e-6,
                    "无目标方向应退化为 +X fallback，实际 {dir:?}"
                );
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    /// 灌毒蛊：dugu_needle_throw 动画 + 毒绿雾（无方向，绕身散布）。
    #[test]
    fn dugu_infuse_poison_emits_throw_anim_and_poison_mist() {
        let mut app = setup_dugu_visual_app();
        let infuser = spawn_player(&mut app, "Infuser", [0.0, 64.0, 0.0]);
        app.world_mut().send_event(DuguObfuscationDisruptedEvent {
            infuser,
            until_tick: 200,
        });
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 2, "灌毒蛊应 emit 1 动画 + 1 毒雾粒子");
        assert_play_anim(&emitted[0], ANIM_DUGU_NEEDLE_THROW, DUGU_PRIORITY);
        assert_spawn_particle(&emitted[1], VFX_DUGU_POISON_INFUSE, Some(14));
        match &emitted[1].payload {
            VfxEventPayloadV1::SpawnParticle { direction, .. } => {
                assert!(direction.is_none(), "灌毒蛊毒雾绕身散布，不应带方向");
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    /// shooter 无 player 组件（无 Position/UniqueId）→ 动画因 player 查询失败 skip，
    /// 弹道粒子仍以默认 origin emit（与 anqi 弹道 unwrap_or_default 同口径，远程招不丢反馈）。
    #[test]
    fn dugu_shoot_needle_without_player_components_skips_anim_keeps_particle() {
        let mut app = setup_dugu_visual_app();
        let shooter = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(QiNeedleChargedEvent {
            shooter,
            target: None,
            tick: 9,
        });
        app.update();
        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "无 player 组件时动画 skip、仅保留 1 条弹道粒子，实际 {} 条",
            emitted.len()
        );
        assert_spawn_particle(&emitted[0], VFX_DUGU_NEEDLE_BOLT, Some(10));
    }

    // ========== 蛊道 v2 五招粒子（emit_dugu_v2_visual_triggers） ==========

    fn setup_dugu_v2_visual_app() -> App {
        let mut app = App::new();
        app.add_event::<EclipseNeedleEvent>();
        app.add_event::<SelfCureProgressEvent>();
        app.add_event::<PenetrateChainEvent>();
        app.add_event::<ShroudActivatedEvent>();
        app.add_event::<ReverseTriggeredEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_dugu_v2_visual_triggers);
        app
    }

    fn dugu_v2_visual(
        skill: crate::combat::dugu_v2::events::DuguSkillId,
    ) -> crate::combat::dugu_v2::events::DuguSkillVisual {
        crate::combat::dugu_v2::skills::visual_for(skill)
    }

    fn assert_spawn_particle_origin(request: &VfxEventRequest, expected: [f64; 3]) {
        match &request.payload {
            VfxEventPayloadV1::SpawnParticle { origin, .. } => {
                assert_eq!(
                    *origin, expected,
                    "粒子 origin 应落在 {expected:?}（事件语义位置），实际 {origin:?}"
                );
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    /// 蚀针：毒渍脉冲印于受害者（target）脚下，event_id 与 client DuguV2VfxPlayer 对齐。
    #[test]
    fn dugu_v2_eclipse_emits_taint_pulse_at_target() {
        use crate::combat::dugu_v2::events::{DuguSkillId, TaintTier};
        let mut app = setup_dugu_v2_visual_app();
        let caster = spawn_player(&mut app, "Caster", [0.0, 64.0, 0.0]);
        let target = spawn_player(&mut app, "Victim", [5.0, 64.0, 5.0]);
        app.world_mut().send_event(EclipseNeedleEvent {
            caster,
            target,
            target_realm: Realm::Awaken,
            tier: TaintTier::Temporary,
            injected_qi: 4.0,
            hp_loss: 2.0,
            qi_loss: 3.0,
            qi_max_loss: 0.0,
            permanent_decay_rate_per_min: 0.0,
            returned_zone_qi: 3.0,
            reveal_probability: 0.1,
            tick: 7,
            visual: dugu_v2_visual(DuguSkillId::Eclipse),
        });
        app.update();
        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "蚀针应只发 1 条粒子（anim/audio 在 skills.rs 内联）"
        );
        assert_spawn_particle(&emitted[0], "bong:dugu_taint_pulse", Some(12));
        assert_spawn_particle_origin(&emitted[0], [5.0, 64.0, 5.0]);
    }

    /// 蚀针受害者断 Position → 落到施法者位置（出手反馈不丢）。
    #[test]
    fn dugu_v2_eclipse_falls_back_to_caster_origin_when_target_positionless() {
        use crate::combat::dugu_v2::events::{DuguSkillId, TaintTier};
        let mut app = setup_dugu_v2_visual_app();
        let caster = spawn_player(&mut app, "Caster", [1.0, 64.0, 2.0]);
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(EclipseNeedleEvent {
            caster,
            target,
            target_realm: Realm::Awaken,
            tier: TaintTier::Immediate,
            injected_qi: 1.0,
            hp_loss: 1.0,
            qi_loss: 1.0,
            qi_max_loss: 0.0,
            permanent_decay_rate_per_min: 0.0,
            returned_zone_qi: 1.0,
            reveal_probability: 0.1,
            tick: 8,
            visual: dugu_v2_visual(DuguSkillId::Eclipse),
        });
        app.update();
        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "受害者断 Position 时粒子应回落到施法者位置（出手反馈不丢），实际 {} 条",
            emitted.len()
        );
        assert_spawn_particle_origin(&emitted[0], [1.0, 64.0, 2.0]);
    }

    /// 自蕴：稀薄深绿雾绕施法者。
    #[test]
    fn dugu_v2_self_cure_emits_thin_mist_at_caster() {
        use crate::combat::dugu_v2::events::DuguSkillId;
        let mut app = setup_dugu_v2_visual_app();
        let caster = spawn_player(&mut app, "Caster", [3.0, 64.0, 3.0]);
        app.world_mut().send_event(SelfCureProgressEvent {
            caster,
            hours_used: 2.5,
            daily_hours_after: 2.5,
            gain_percent: 10.0,
            insidious_color_percent: 5.0,
            morphology_percent: 1.0,
            self_revealed: false,
            tick: 9,
            visual: dugu_v2_visual(DuguSkillId::SelfCure),
        });
        app.update();
        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "自蕴应只发 1 条雾粒子（anim/audio 在 skills.rs 内联），实际 {} 条",
            emitted.len()
        );
        assert_spawn_particle(&emitted[0], "bong:dugu_dark_green_mist", Some(14));
        assert_spawn_particle_origin(&emitted[0], [3.0, 64.0, 3.0]);
    }

    /// 侵染：毒渍密度随链上受害者数增长且封顶 32。
    #[test]
    fn dugu_v2_penetrate_particle_count_scales_with_targets_and_caps() {
        use crate::combat::dugu_v2::events::{DuguSkillId, TaintTier};
        let mut app = setup_dugu_v2_visual_app();
        let caster = spawn_player(&mut app, "Caster", [0.0, 64.0, 0.0]);
        let target = spawn_player(&mut app, "Victim", [2.0, 64.0, 0.0]);
        for (affected, expected_count) in [(2u32, 20u16), (99u32, 32u16)] {
            app.world_mut().send_event(PenetrateChainEvent {
                caster,
                target,
                taint_tier: TaintTier::Temporary,
                multiplier: 1.5,
                affected_targets: affected,
                permanent_decay_rate_per_min: 0.0,
                reveal_probability: 0.2,
                returned_zone_qi: 5.0,
                tick: 10,
                visual: dugu_v2_visual(DuguSkillId::Penetrate),
            });
            app.update();
            let emitted = drain_vfx(&mut app);
            assert_eq!(
                emitted.len(),
                1,
                "侵染（affected={affected}）应只发 1 条毒渍粒子，实际 {} 条",
                emitted.len()
            );
            assert_spawn_particle(&emitted[0], "bong:dugu_taint_pulse", Some(expected_count));
        }
    }

    /// 神识遮蔽：深绿雾罩绕施法者，strength 透传（钳 [0.6, 1.5]）。
    #[test]
    fn dugu_v2_shroud_emits_mist_with_clamped_strength() {
        use crate::combat::dugu_v2::events::DuguSkillId;
        let mut app = setup_dugu_v2_visual_app();
        let caster = spawn_player(&mut app, "Caster", [0.0, 64.0, 0.0]);
        app.world_mut().send_event(ShroudActivatedEvent {
            caster,
            strength: 9.0,
            expires_at_tick: 600,
            tick: 11,
            visual: dugu_v2_visual(DuguSkillId::Shroud),
        });
        app.update();
        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "神识遮蔽应只发 1 条雾罩粒子，实际 {} 条",
            emitted.len()
        );
        assert_spawn_particle(&emitted[0], "bong:dugu_dark_green_mist", Some(28));
        match &emitted[0].payload {
            VfxEventPayloadV1::SpawnParticle { strength, .. } => {
                assert_eq!(
                    *strength,
                    Some(1.5),
                    "遮蔽强度 9.0 应被钳到 1.5（视觉浓度上限），实际 {strength:?}"
                );
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    /// 倒蚀：爆发线束于事件自带爆心（不依赖 Position），count 随受害者数封顶 48。
    #[test]
    fn dugu_v2_reverse_emits_burst_at_event_center() {
        use crate::combat::dugu_v2::events::DuguSkillId;
        use valence::prelude::DVec3;
        let mut app = setup_dugu_v2_visual_app();
        let caster = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(ReverseTriggeredEvent {
            caster,
            affected_targets: 3,
            burst_damage: 12.0,
            returned_zone_qi: 6.0,
            juebi_delay_ticks: None,
            tick: 12,
            center: DVec3::new(10.0, 65.0, -4.0),
            visual: dugu_v2_visual(DuguSkillId::Reverse),
        });
        app.update();
        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "倒蚀无 Position 也必须出粒子（事件自带爆心）"
        );
        assert_spawn_particle(&emitted[0], "bong:dugu_reverse_burst", Some(36));
        assert_spawn_particle_origin(&emitted[0], [10.0, 65.0, -4.0]);
    }

    /// 五招 particle_id 全部落在 client DuguV2VfxPlayer.EVENT_IDS 的三个注册项内
    ///（谁改 visual_for 忘改 client 注册，此测撞红）。
    #[test]
    fn dugu_v2_all_skill_particle_ids_are_client_registered() {
        use crate::combat::dugu_v2::events::DuguSkillId;
        const CLIENT_REGISTERED: [&str; 3] = [
            "bong:dugu_taint_pulse",
            "bong:dugu_dark_green_mist",
            "bong:dugu_reverse_burst",
        ];
        for skill in DuguSkillId::ALL {
            let visual = dugu_v2_visual(skill);
            assert!(
                CLIENT_REGISTERED.contains(&visual.particle_id),
                "{skill:?} 的 particle_id {} 未在 client DuguV2VfxPlayer.EVENT_IDS 注册，\
                 VfxRegistry 查表将 miss、粒子静默丢弃",
                visual.particle_id
            );
        }
    }

    // ========== 绝灵涡流 woliu v1（emit_woliu_v1_vortex_visual_triggers） ==========

    fn setup_woliu_v1_visual_app(tick: u64) -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick });
        app.add_event::<VortexBackfireEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_woliu_v1_vortex_visual_triggers);
        app
    }

    fn spawn_v1_field(app: &mut App, caster: Entity, cast_at_tick: u64) -> Entity {
        use valence::prelude::DVec3;
        let field = VortexField {
            center: DVec3::new(0.0, 64.0, 0.0),
            radius: 8.0,
            delta: 4.0,
            cast_at_tick,
            maintain_max_ticks: 1200,
            caster,
            env_qi_at_cast: 50.0,
            last_maintain_tick: cast_at_tick,
        };
        app.world_mut().entity_mut(caster).insert(field);
        caster
    }

    /// field 出现 → 起手式动画 + 开涡吸入环；同 field 下一 tick 不重发开涡。
    #[test]
    fn woliu_v1_field_appear_emits_stance_anim_and_open_burst_once() {
        let mut app = setup_woliu_v1_visual_app(101);
        let caster = spawn_player(&mut app, "Caster", [0.0, 64.0, 0.0]);
        spawn_v1_field(&mut app, caster, 101);

        app.update();
        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            2,
            "开涡应发 1 动画 + 1 粒子，实际 {} 条",
            emitted.len()
        );
        assert_play_anim(&emitted[0], ANIM_WOLIU_V1_STANCE, WOLIU_PRIORITY);
        assert_spawn_particle(&emitted[1], VFX_WOLIU_V1_FIELD_OPEN, Some(24));

        // 下一 tick（非 ambient 周期）：不得重发开涡。
        app.world_mut().resource_mut::<CombatClock>().tick = 102;
        app.update();
        assert!(
            drain_vfx(&mut app).is_empty(),
            "field 存续第 2 tick（非周期点）不应重发任何粒子"
        );
    }

    /// 存续满一个周期（20 tick）→ 低频涡环；非周期 tick 静默。
    #[test]
    fn woliu_v1_field_sustain_emits_ambient_ring_on_period() {
        let mut app = setup_woliu_v1_visual_app(101);
        let caster = spawn_player(&mut app, "Caster", [0.0, 64.0, 0.0]);
        spawn_v1_field(&mut app, caster, 101);
        app.update();
        drain_vfx(&mut app); // 吃掉开涡

        app.world_mut().resource_mut::<CombatClock>().tick = 101 + WOLIU_V1_AMBIENT_PERIOD_TICKS;
        app.update();
        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 1, "存续满 20 tick 应发 1 条低频涡环");
        assert_spawn_particle(&emitted[0], VFX_WOLIU_V1_FIELD_AMBIENT, Some(12));
    }

    /// field 移除后重新开涡 → 再次发开涡（lifecycle 状态正确清退）。
    #[test]
    fn woliu_v1_field_reopen_after_removal_emits_open_again() {
        let mut app = setup_woliu_v1_visual_app(101);
        let caster = spawn_player(&mut app, "Caster", [0.0, 64.0, 0.0]);
        spawn_v1_field(&mut app, caster, 101);
        app.update();
        drain_vfx(&mut app);

        app.world_mut().entity_mut(caster).remove::<VortexField>();
        app.world_mut().resource_mut::<CombatClock>().tick = 105;
        app.update();
        assert!(drain_vfx(&mut app).is_empty(), "关涡后不应再发粒子");

        spawn_v1_field(&mut app, caster, 110);
        app.world_mut().resource_mut::<CombatClock>().tick = 110;
        app.update();
        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 2, "重新开涡应再次发动画 + 开涡粒子");
        assert_spawn_particle(&emitted[1], VFX_WOLIU_V1_FIELD_OPEN, Some(24));
    }

    /// 反噬 → 断经暗红爆裂于施法者位置。
    #[test]
    fn woliu_v1_backfire_emits_burst_at_caster() {
        use crate::combat::woliu::BackfireCause;
        use crate::cultivation::components::MeridianId;
        let mut app = setup_woliu_v1_visual_app(200);
        let caster = spawn_player(&mut app, "Caster", [6.0, 64.0, 6.0]);
        app.world_mut().send_event(VortexBackfireEvent {
            caster,
            cause: BackfireCause::ExceedMaintainMax,
            meridian_severed: MeridianId::Lung,
            tick: 200,
            env_qi: 10.0,
            delta: 4.0,
            resisted: false,
        });
        app.update();
        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "反噬应发 1 条爆裂粒子（断经负反馈），实际 {} 条",
            emitted.len()
        );
        assert_spawn_particle(&emitted[0], VFX_WOLIU_V1_BACKFIRE, Some(30));
        assert_spawn_particle_color(&emitted[0], "#B84A3F");
        assert_spawn_particle_origin(&emitted[0], [6.0, 64.0, 6.0]);
    }

    /// 自蕴施法者断 Position → 静默 skip（无位置可放雾，无回落来源）。
    #[test]
    fn dugu_v2_self_cure_skips_when_caster_positionless() {
        use crate::combat::dugu_v2::events::DuguSkillId;
        let mut app = setup_dugu_v2_visual_app();
        let caster = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(SelfCureProgressEvent {
            caster,
            hours_used: 2.5,
            daily_hours_after: 2.5,
            gain_percent: 10.0,
            insidious_color_percent: 5.0,
            morphology_percent: 1.0,
            self_revealed: false,
            tick: 9,
            visual: dugu_v2_visual(DuguSkillId::SelfCure),
        });
        app.update();
        assert!(
            drain_vfx(&mut app).is_empty(),
            "自蕴施法者无 Position 应静默 skip，不得以默认坐标发粒子"
        );
    }

    /// 神识遮蔽施法者断 Position → 静默 skip。
    #[test]
    fn dugu_v2_shroud_skips_when_caster_positionless() {
        use crate::combat::dugu_v2::events::DuguSkillId;
        let mut app = setup_dugu_v2_visual_app();
        let caster = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(ShroudActivatedEvent {
            caster,
            strength: 1.0,
            expires_at_tick: 600,
            tick: 11,
            visual: dugu_v2_visual(DuguSkillId::Shroud),
        });
        app.update();
        assert!(
            drain_vfx(&mut app).is_empty(),
            "遮蔽施法者无 Position 应静默 skip，不得以默认坐标发粒子"
        );
    }

    /// 侵染 target 与 caster 双双断 Position → 静默 skip（回落链穷尽）。
    #[test]
    fn dugu_v2_penetrate_skips_when_both_positionless() {
        use crate::combat::dugu_v2::events::{DuguSkillId, TaintTier};
        let mut app = setup_dugu_v2_visual_app();
        let caster = app.world_mut().spawn_empty().id();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(PenetrateChainEvent {
            caster,
            target,
            taint_tier: TaintTier::Temporary,
            multiplier: 1.5,
            affected_targets: 2,
            permanent_decay_rate_per_min: 0.0,
            reveal_probability: 0.2,
            returned_zone_qi: 5.0,
            tick: 10,
            visual: dugu_v2_visual(DuguSkillId::Penetrate),
        });
        app.update();
        assert!(
            drain_vfx(&mut app).is_empty(),
            "侵染 target 与 caster 均无 Position 时应静默 skip（回落链穷尽）"
        );
    }

    /// 反噬 caster 断 Position 但领域仍在 → 回落到 field.center（重要负反馈不静默丢）。
    #[test]
    fn woliu_v1_backfire_falls_back_to_field_center_when_caster_positionless() {
        use crate::combat::woliu::BackfireCause;
        use crate::cultivation::components::MeridianId;
        let mut app = setup_woliu_v1_visual_app(200);
        let caster = app.world_mut().spawn_empty().id();
        spawn_v1_field(&mut app, caster, 200);
        app.update();
        drain_vfx(&mut app); // 吃掉开涡（无 player 组件时动画 skip、粒子仍发）

        app.world_mut().send_event(VortexBackfireEvent {
            caster,
            cause: BackfireCause::ExceedMaintainMax,
            meridian_severed: MeridianId::Lung,
            tick: 201,
            env_qi: 10.0,
            delta: 4.0,
            resisted: false,
        });
        app.world_mut().resource_mut::<CombatClock>().tick = 201;
        app.update();
        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "caster 无 Position 但领域仍在时，反噬粒子应回落 field.center，实际 {} 条",
            emitted.len()
        );
        assert_spawn_particle(&emitted[0], VFX_WOLIU_V1_BACKFIRE, Some(30));
        assert_spawn_particle_origin(&emitted[0], [0.0, 64.0, 0.0]);
    }

    /// 反噬 caster 无 Position 且领域已散 → 回落链穷尽，静默 skip。
    #[test]
    fn woliu_v1_backfire_skips_when_positionless_and_no_field() {
        use crate::combat::woliu::BackfireCause;
        use crate::cultivation::components::MeridianId;
        let mut app = setup_woliu_v1_visual_app(200);
        let caster = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(VortexBackfireEvent {
            caster,
            cause: BackfireCause::EnvQiTooLow,
            meridian_severed: MeridianId::Lung,
            tick: 200,
            env_qi: 1.0,
            delta: 4.0,
            resisted: false,
        });
        app.update();
        assert!(
            drain_vfx(&mut app).is_empty(),
            "caster 无 Position 且无领域可回落时应静默 skip"
        );
    }

    /// server 侧三个 v1 event_id 字面值锁死——与 client VortexSpiralPlayer.WOLIU_V1_* 逐字对齐
    ///（对照 dugu_v2_all_skill_particle_ids_are_client_registered 的同款防线）。
    #[test]
    fn woliu_v1_particle_ids_match_client_registration() {
        assert_eq!(
            VFX_WOLIU_V1_FIELD_OPEN, "bong:woliu_vortex_field",
            "开涡 event_id 必须与 client VortexSpiralPlayer.WOLIU_V1_FIELD_OPEN 逐字一致，否则粒子静默丢弃"
        );
        assert_eq!(
            VFX_WOLIU_V1_FIELD_AMBIENT, "bong:woliu_vortex_field_ambient",
            "存续 event_id 必须与 client VortexSpiralPlayer.WOLIU_V1_FIELD_AMBIENT 逐字一致"
        );
        assert_eq!(
            VFX_WOLIU_V1_BACKFIRE, "bong:woliu_vortex_backfire",
            "反噬 event_id 必须与 client VortexSpiralPlayer.WOLIU_V1_BACKFIRE 逐字一致"
        );
    }

    // ── plan-skill-av-relink-v1 P3 —— P1 接线 emit pin：stance adapter ────────────

    fn setup_stance_app() -> App {
        let mut app = App::new();
        app.add_event::<TechniqueLearnedEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_technique_learned_stance_triggers);
        app
    }

    fn send_technique_learned(app: &mut App, player: valence::prelude::Entity, technique_id: &str) {
        use crate::cultivation::technique_scroll::LearnSource;
        app.world_mut().send_event(TechniqueLearnedEvent {
            player,
            technique_id: technique_id.to_string(),
            source: LearnSource::Scroll {
                item_id: "technique_scroll_test".to_string(),
            },
        });
    }

    /// happy path：六条 stance 接线逐条 pin——每个映射前缀/完整 id 习得时恰发一条
    /// 携正确 anim_id 的 PlayAnim（technique_id 全部取 TECHNIQUE_DEFINITIONS 真实条目）。
    #[test]
    fn technique_learned_emits_mapped_stance_animation_for_each_wired_family() {
        let cases = [
            ("woliu.vortex", ANIM_STANCE_WOLIU),
            ("dugu.shoot_needle", ANIM_STANCE_DUGU),
            ("dugu.infuse_poison", ANIM_STANCE_DUGU_POISON),
            ("baomai.full_power_charge", ANIM_STANCE_BAOMAI),
            ("zhenmai.parry", ANIM_STANCE_ZHENMAI),
            ("tuike.don", ANIM_STANCE_TUIKE),
        ];
        for (technique_id, expected_anim) in cases {
            let mut app = setup_stance_app();
            let player = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
            send_technique_learned(&mut app, player, technique_id);
            app.update();

            let emitted = drain_vfx(&mut app);
            assert_eq!(
                emitted.len(),
                1,
                "习得 `{technique_id}` 应恰发一条架势动画，实际 {emitted:?}"
            );
            assert_play_anim(&emitted[0], expected_anim, STORY_PRIORITY);
        }
    }

    /// 架势动画 target_player 必须是习得者自己的 uuid（不是发给别人播）。
    #[test]
    fn stance_animation_targets_the_learner_uuid() {
        let mut app = setup_stance_app();
        let player = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let expected_uuid = app
            .world()
            .get::<UniqueId>(player)
            .expect("mock client should carry UniqueId")
            .0
            .to_string();
        send_technique_learned(&mut app, player, "woliu.vortex");
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(emitted.len(), 1);
        match &emitted[0].payload {
            VfxEventPayloadV1::PlayAnim { target_player, .. } => assert_eq!(
                target_player, &expected_uuid,
                "架势动画应发给习得者本人（target_player = 习得者 uuid）"
            ),
            other => panic!("expected PlayAnim, got {other:?}"),
        }
    }

    /// 错误分支：无映射前缀不发——sword/movement 等非架势流派、dugu 前缀但非两条
    /// 映射完整 id、zhenfa（stance_zhenfa 维持 report-only，见 plan P1 表）全部静默。
    #[test]
    fn technique_learned_with_unmapped_family_does_not_emit_stance() {
        for technique_id in [
            "sword.cleave",
            "movement.dash",
            "dugu.some_future_variant",
            "zhenfa.ward",
            "",
        ] {
            let mut app = setup_stance_app();
            let player = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
            send_technique_learned(&mut app, player, technique_id);
            app.update();

            let emitted = drain_vfx(&mut app);
            assert!(
                emitted.is_empty(),
                "无映射 technique `{technique_id}` 不应发架势动画，实际 {emitted:?}"
            );
        }
    }

    /// 注：`"woliu"`（无 `.` 的裸前缀）走 `split('.').next()` 仍解析出 `woliu`——
    /// 该形态在 TECHNIQUE_DEFINITIONS 中不存在，仅锁 split 语义不背离预期。
    #[test]
    fn bare_family_prefix_without_dot_still_maps_by_split_semantics() {
        assert_eq!(
            stance_anim_for_technique("woliu"),
            Some(ANIM_STANCE_WOLIU),
            "split('.').next() 对无点 id 返回整串——语义变更需同步本 pin 与映射注释"
        );
        assert_eq!(stance_anim_for_technique(""), None, "空串必须映射为 None");
    }

    /// 状态转换分支：习得者实体缺 Position/UniqueId（离线/已清理）时静默不发。
    #[test]
    fn technique_learned_for_entity_without_anim_target_does_not_emit() {
        let mut app = setup_stance_app();
        let ghost = app.world_mut().spawn_empty().id();
        send_technique_learned(&mut app, ghost, "woliu.vortex");
        app.update();

        assert!(
            drain_vfx(&mut app).is_empty(),
            "缺 Position/UniqueId 的实体不应发架势动画"
        );
    }

    /// 重复触发语义：adapter 对事件 1:1 发射、无隐藏去重状态——生产幂等门在上游
    /// （`learn_technique_if_allowed` 对已习得功法返回 AlreadyKnown、不发
    /// TechniqueLearnedEvent，见 technique_scroll 测试），本测试锁 adapter 自身
    /// 不做去重的分工契约。
    #[test]
    fn repeated_technique_learned_events_emit_one_stance_anim_each() {
        let mut app = setup_stance_app();
        let player = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        send_technique_learned(&mut app, player, "woliu.vortex");
        send_technique_learned(&mut app, player, "woliu.vortex");
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            2,
            "adapter 按事件 1:1 发射（去重责任在上游 AlreadyKnown 门），实际 {emitted:?}"
        );
        for request in &emitted {
            assert_play_anim(request, ANIM_STANCE_WOLIU, STORY_PRIORITY);
        }
    }

    // ── plan-skill-av-relink-v1 P3 —— P1 接线 emit pin：forge_hammer adapter ─────

    fn setup_forge_anim_app(
        current_step: crate::forge::session::ForgeStep,
        caster_has_anim_target: bool,
    ) -> (App, crate::forge::session::ForgeSessionId) {
        use crate::forge::session::ForgeSession;
        let mut app = App::new();
        app.add_event::<TemperingHit>();
        app.add_event::<VfxEventRequest>();
        let caster = if caster_has_anim_target {
            spawn_player(&mut app, "Smith", [0.0, 64.0, 0.0])
        } else {
            app.world_mut().spawn_empty().id()
        };
        let station = app.world_mut().spawn_empty().id();
        let mut sessions = ForgeSessions::default();
        let session_id = sessions.allocate_id();
        let mut session =
            ForgeSession::new(session_id, "test_blueprint".to_string(), station, caster);
        session.current_step = current_step;
        sessions.insert(session);
        app.insert_resource(sessions);
        app.add_systems(Update, emit_forge_tempering_animation_triggers);
        (app, session_id)
    }

    fn send_tempering_hit(app: &mut App, session: crate::forge::session::ForgeSessionId) {
        use crate::forge::blueprint::TemperBeat;
        app.world_mut().send_event(TemperingHit {
            session,
            beat: TemperBeat::Light,
            ticks_remaining: 3,
        });
    }

    /// happy path：Tempering 步的按键命中 → 恰发一条 forge_hammer 抡锤动画。
    #[test]
    fn tempering_hit_in_tempering_step_emits_forge_hammer() {
        let (mut app, session_id) = setup_forge_anim_app(ForgeStep::Tempering, true);
        send_tempering_hit(&mut app, session_id);
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "Tempering 步命中应恰发一条抡锤动画，实际 {emitted:?}"
        );
        assert_play_anim(&emitted[0], ANIM_FORGE_HAMMER, COMBAT_PRIORITY);
    }

    /// 重复触发语义：J/K/L 每次命中都是独立抡锤——两击两动画（1:1，无去重）。
    #[test]
    fn each_tempering_hit_emits_its_own_forge_hammer_swing() {
        let (mut app, session_id) = setup_forge_anim_app(ForgeStep::Tempering, true);
        send_tempering_hit(&mut app, session_id);
        send_tempering_hit(&mut app, session_id);
        app.update();

        let emitted = drain_vfx(&mut app);
        assert_eq!(
            emitted.len(),
            2,
            "每次淬炼命中各配一次抡锤动画，实际 {emitted:?}"
        );
        for request in &emitted {
            assert_play_anim(request, ANIM_FORGE_HAMMER, COMBAT_PRIORITY);
        }
    }

    /// 错误分支：非 Tempering 步的 stale 按键不发动画（镜像 forge 模块步骤门）。
    #[test]
    fn tempering_hit_outside_tempering_step_does_not_emit() {
        for step in [
            ForgeStep::Billet,
            ForgeStep::Inscription,
            ForgeStep::Consecration,
            ForgeStep::Done,
        ] {
            let (mut app, session_id) = setup_forge_anim_app(step, true);
            send_tempering_hit(&mut app, session_id);
            app.update();

            let emitted = drain_vfx(&mut app);
            assert!(
                emitted.is_empty(),
                "{step:?} 步的 stale 淬炼按键不应发抡锤动画，实际 {emitted:?}"
            );
        }
    }

    /// 错误分支：session 已结算/弃疗（表中不存在）时不发。
    #[test]
    fn tempering_hit_for_missing_session_does_not_emit() {
        use crate::forge::session::ForgeSessionId;
        let (mut app, _live_session) = setup_forge_anim_app(ForgeStep::Tempering, true);
        send_tempering_hit(&mut app, ForgeSessionId(9999));
        app.update();

        assert!(
            drain_vfx(&mut app).is_empty(),
            "不存在的 session 的命中不应发抡锤动画"
        );
    }

    /// 状态转换分支：caster 实体缺 Position/UniqueId（离线/已清理）时静默不发。
    #[test]
    fn tempering_hit_for_caster_without_anim_target_does_not_emit() {
        let (mut app, session_id) = setup_forge_anim_app(ForgeStep::Tempering, false);
        send_tempering_hit(&mut app, session_id);
        app.update();

        assert!(
            drain_vfx(&mut app).is_empty(),
            "caster 缺 Position/UniqueId 时不应发抡锤动画"
        );
    }

    // ── plan-skill-av-relink-v1 P3 —— fist_punch_left 连击交替序列 ────────────────

    fn setup_fist_combo_app() -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.add_event::<AttackIntent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_attack_animation_triggers);
        app
    }

    /// 在 `tick` 时刻发一记拳（默认 Blunt/Melee），断言恰发一条 PlayAnim 并返回其 anim_id。
    fn punch_anim_at_tick(
        app: &mut App,
        attacker: valence::prelude::Entity,
        tick: u64,
        wound_kind: WoundKind,
    ) -> String {
        app.world_mut().resource_mut::<CombatClock>().tick = tick;
        app.world_mut().send_event(AttackIntent {
            attacker,
            target: None,
            issued_at_tick: tick,
            reach: AttackReach::new(1.0, 0.0),
            qi_invest: 0.0,
            wound_kind,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();
        let emitted = drain_vfx(app);
        assert_eq!(
            emitted.len(),
            1,
            "tick {tick} 的攻击应恰发一条动画，实际 {emitted:?}"
        );
        match &emitted[0].payload {
            VfxEventPayloadV1::PlayAnim { anim_id, .. } => anim_id.clone(),
            other => panic!("expected PlayAnim, got {other:?}"),
        }
    }

    /// happy path：连续空手攻击 right → left → right 交替。
    #[test]
    fn unarmed_punches_alternate_right_left_right() {
        let mut app = setup_fist_combo_app();
        let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

        assert_eq!(
            punch_anim_at_tick(&mut app, attacker, 10, WoundKind::Blunt),
            ANIM_FIST_PUNCH_RIGHT,
            "空手连击必须右拳起手"
        );
        assert_eq!(
            punch_anim_at_tick(&mut app, attacker, 11, WoundKind::Blunt),
            ANIM_FIST_PUNCH_LEFT,
            "第二拳应交替为左拳"
        );
        assert_eq!(
            punch_anim_at_tick(&mut app, attacker, 12, WoundKind::Blunt),
            ANIM_FIST_PUNCH_RIGHT,
            "第三拳应交替回右拳"
        );
    }

    /// Concussion 与 Blunt 同属拳击解析（`attack_anim_for_wound_kind`），
    /// 同一连击链内交替不因 wound_kind 在两者间切换而中断。
    #[test]
    fn concussion_wound_kind_participates_in_same_fist_combo() {
        let mut app = setup_fist_combo_app();
        let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

        assert_eq!(
            punch_anim_at_tick(&mut app, attacker, 10, WoundKind::Blunt),
            ANIM_FIST_PUNCH_RIGHT
        );
        assert_eq!(
            punch_anim_at_tick(&mut app, attacker, 11, WoundKind::Concussion),
            ANIM_FIST_PUNCH_LEFT,
            "Concussion 拳与 Blunt 拳共享同一交替链"
        );
    }

    /// 边界（off-by-one）：两拳间隔恰等于 FIST_COMBO_RESET_TICKS 时连击**未**中断，
    /// 仍交替；超过一个 tick 才复位（行为语义引用常数，不锁字面值）。
    #[test]
    fn fist_combo_continues_at_reset_boundary_and_resets_past_it() {
        let mut app = setup_fist_combo_app();
        let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

        let start = 100_u64;
        assert_eq!(
            punch_anim_at_tick(&mut app, attacker, start, WoundKind::Blunt),
            ANIM_FIST_PUNCH_RIGHT
        );
        let boundary = start + FIST_COMBO_RESET_TICKS;
        assert_eq!(
            punch_anim_at_tick(&mut app, attacker, boundary, WoundKind::Blunt),
            ANIM_FIST_PUNCH_LEFT,
            "间隔恰为 FIST_COMBO_RESET_TICKS 时连击应延续（> 才算超时）"
        );

        // 此刻交替态为"下一拳右"——若超时复位不生效，下一拳同样是右拳，无从分辨。
        // 因此先补一拳把交替态推到"下一拳左"，再验证超时后回到右拳起手。
        assert_eq!(
            punch_anim_at_tick(&mut app, attacker, boundary + 1, WoundKind::Blunt),
            ANIM_FIST_PUNCH_RIGHT
        );
        let past_timeout = boundary + 1 + FIST_COMBO_RESET_TICKS + 1;
        assert_eq!(
            punch_anim_at_tick(&mut app, attacker, past_timeout, WoundKind::Blunt),
            ANIM_FIST_PUNCH_RIGHT,
            "间隔超过 FIST_COMBO_RESET_TICKS 后连击中断，必须右拳重新起手（否则应为左拳）"
        );
    }

    /// 持械分支：携 Weapon component（含 Staff/Fist 类武器）不参与交替，恒右拳。
    #[test]
    fn armed_attacker_never_alternates_to_left_punch() {
        use crate::combat::weapon::{EquipSlot, WeaponKind};
        for weapon_kind in [WeaponKind::Staff, WeaponKind::Fist] {
            let mut app = setup_fist_combo_app();
            let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
            app.world_mut().entity_mut(attacker).insert(Weapon {
                slot: EquipSlot::MainHand,
                instance_id: 1,
                template_id: "test_blunt_weapon".to_string(),
                weapon_kind,
                base_attack: 1.0,
                quality_tier: 0,
                durability: 10.0,
                durability_max: 10.0,
            });

            for tick in 10..13 {
                assert_eq!(
                    punch_anim_at_tick(&mut app, attacker, tick, WoundKind::Blunt),
                    ANIM_FIST_PUNCH_RIGHT,
                    "持械（{weapon_kind:?}）连续攻击不参与交替，恒右拳"
                );
            }
        }
    }

    /// 状态转换：持械攻击既不推进也不复位空手连击链——右拳后持械打两下、
    /// 超时窗口内卸下再空手，链条视同延续出左拳（armed 分支不触碰交替态）。
    #[test]
    fn armed_attacks_do_not_touch_unarmed_combo_state() {
        use crate::combat::weapon::{EquipSlot, WeaponKind};
        let mut app = setup_fist_combo_app();
        let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

        assert_eq!(
            punch_anim_at_tick(&mut app, attacker, 10, WoundKind::Blunt),
            ANIM_FIST_PUNCH_RIGHT,
            "空手首击右拳起手"
        );

        app.world_mut().entity_mut(attacker).insert(Weapon {
            slot: EquipSlot::MainHand,
            instance_id: 1,
            template_id: "test_blunt_weapon".to_string(),
            weapon_kind: WeaponKind::Staff,
            base_attack: 1.0,
            quality_tier: 0,
            durability: 10.0,
            durability_max: 10.0,
        });
        for tick in [12, 14] {
            assert_eq!(
                punch_anim_at_tick(&mut app, attacker, tick, WoundKind::Blunt),
                ANIM_FIST_PUNCH_RIGHT,
                "持械攻击恒右拳（tick {tick}）"
            );
        }

        app.world_mut().entity_mut(attacker).remove::<Weapon>();
        assert_eq!(
            punch_anim_at_tick(&mut app, attacker, 16, WoundKind::Blunt),
            ANIM_FIST_PUNCH_LEFT,
            "超时窗口内卸下武器再空手：持械期间不触碰交替态，链条延续应出左拳"
        );
    }

    /// 玩家隔离：两名玩家交错互不干扰，各自独立 right → left 交替。
    #[test]
    fn fist_combo_state_is_isolated_per_player() {
        let mut app = setup_fist_combo_app();
        let alice = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let bob = spawn_player(&mut app, "Bob", [4.0, 64.0, 0.0]);

        assert_eq!(
            punch_anim_at_tick(&mut app, alice, 10, WoundKind::Blunt),
            ANIM_FIST_PUNCH_RIGHT,
            "Alice 首拳右起手"
        );
        assert_eq!(
            punch_anim_at_tick(&mut app, bob, 11, WoundKind::Blunt),
            ANIM_FIST_PUNCH_RIGHT,
            "Bob 首拳不受 Alice 交替态影响，同样右起手"
        );
        assert_eq!(
            punch_anim_at_tick(&mut app, alice, 12, WoundKind::Blunt),
            ANIM_FIST_PUNCH_LEFT,
            "Alice 第二拳交替左拳"
        );
        assert_eq!(
            punch_anim_at_tick(&mut app, bob, 13, WoundKind::Blunt),
            ANIM_FIST_PUNCH_LEFT,
            "Bob 第二拳按自己的链交替左拳"
        );
    }

    /// 非拳击 wound kind（Cut/Pierce/Burn）不落入交替逻辑，空手也不产出左拳。
    #[test]
    fn non_fist_wound_kinds_never_emit_left_punch() {
        let mut app = setup_fist_combo_app();
        let attacker = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

        for (tick, wound_kind, expected) in [
            (10_u64, WoundKind::Cut, ANIM_SWORD_SLASH_DOWN),
            (11, WoundKind::Pierce, ANIM_SWORD_STAB),
            (12, WoundKind::Burn, ANIM_PALM_STRIKE),
        ] {
            assert_eq!(
                punch_anim_at_tick(&mut app, attacker, tick, wound_kind),
                expected,
                "{wound_kind:?} 应走原 wound-kind 动画映射，与拳击交替无关"
            );
        }
    }
}
