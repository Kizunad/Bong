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
use crate::combat::carrier::{
    CarrierChargeBeganEvent, CarrierChargeEndedEvent, CarrierChargedEvent,
};
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
use crate::tools::ToolKind;

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
/// 渡劫失败（`TribulationFailed`）时收掉 `ANIM_TRIBULATION_BRACE` 循环动画的淡出 tick。
/// brace 以 `STORY_PRIORITY`（FULL_BODY 通道）isLoop:true 播放；渡劫失败结局存活
/// （不进入死亡生命周期），若不显式 StopAnim，`ANIM_HURT_STAGGER`（`HIT_RECOIL_PRIORITY`，
/// UPPER_BODY 通道）顶不掉 FULL_BODY 循环，玩家会永久卡在抱臂姿势。
const TRIBULATION_BRACE_STOP_FADE_OUT_TICKS: u8 = 3;
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
// plan-skill-anim-fidelity-v1 P2 前半 —— 凝锋 / 剑气斩 / 共鸣去复用：三招各获
// 专属动画（原分别借基础 sword_cleave / sword_thrust / sword_cleave）。
const ANIM_SWORD_PATH_CONDENSE_EDGE: &str = "bong:sword_path_condense_edge";
const ANIM_SWORD_PATH_QI_SLASH: &str = "bong:sword_path_qi_slash";
const ANIM_SWORD_PATH_RESONANCE: &str = "bong:sword_path_resonance";
const VFX_SWORD_CONDENSE_EDGE: &str = "bong:sword_condense_edge";
const VFX_SWORD_QI_SLASH_PATH: &str = "bong:sword_qi_slash_path";
const VFX_SWORD_RESONANCE: &str = "bong:sword_resonance";
const VFX_SWORD_MANIFEST_SUMMON: &str = "bong:sword_manifest_summon";
const VFX_HEAVEN_GATE_CHARGE: &str = "bong:heaven_gate_charge_2s";
const VFX_HEAVEN_GATE_RELEASE: &str = "bong:heaven_gate_release";
const VFX_HEAVEN_GATE_FLASH: &str = "bong:heaven_gate_flash";

/// 暗器六招动画优先级——与剑道 / baomai 同档（暗器是高阶器修流派）。
const ANQI_PRIORITY: u16 = 1500;

// 暗器六招 AV 资产 id（client 侧 AnqiVfxPlayer / BongAnimations 已注册）。
// plan-skill-anim-fidelity-v1 P2 前半：single_snipe / multi_shot / soul_inject
// 三招去复用换专属动画；P2 后半：charge_carrier 两段式（真实 400t 通道，循环
// 蓄力段 + release 收势，CarrierChargeBegan/Ended 事件接线）+ armor_pierce /
// echo_fractal 专属单段（瞬发结算型长 cast，附录 A 决策 (b)——cast_ticks 是
// 元数据非真实引导窗，无循环段可挂）。粒子复用现有 BongParticles sprite。
/// 封骨充能循环蓄力段（isLoop 32t）——`CarrierChargeBeganEvent` 起播，任何
/// `CarrierChargeEndedEvent` 停播（§8.1 #3 停止路径红线）。
const ANIM_ANQI_CHARGE: &str = "bong:anqi_charge_carrier_loop";
/// 封骨充能完成收势（14t 非循环）——仅 `CarrierChargeEndedEvent{full_charge:true}`
/// 播出；移动打断不奖励收势。
const ANIM_ANQI_CHARGE_RELEASE: &str = "bong:anqi_charge_carrier_release";
/// 循环蓄力段 StopAnim 淡出（tick）。与 full_power_emit 的 windup 停止参数同档。
const ANQI_CHARGE_STOP_FADE_OUT_TICKS: u8 = 3;
const ANIM_ANQI_SNIPE: &str = "bong:anqi_single_snipe";
const ANIM_ANQI_VOLLEY: &str = "bong:anqi_multi_shot";
const ANIM_ANQI_INJECT: &str = "bong:anqi_soul_inject";
/// 破甲注射专属（46t 旋钻贯刺，P2 后半解除 cast_invoke 借用）。
const ANIM_ANQI_ARMOR_PIERCE: &str = "bong:anqi_armor_pierce";
/// 诱饵分形专属（66t 织网撒饵长演出，P2 后半解除 release_burst 借用）。
const ANIM_ANQI_ECHO: &str = "bong:anqi_echo_fractal";
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
/// plan-skill-anim-fidelity-v1 P3 —— 灌毒蛊专属淬毒动画（去共用：原与凝针同发
/// `dugu_needle_throw`，两招仅靠去重 id 区分、动画字符串完全相同）。举针过面 +
/// 覆手灌毒 + 腕封收势，无掷出动作。
const ANIM_DUGU_INFUSE_POISON: &str = "bong:dugu_infuse_poison";
const VFX_DUGU_NEEDLE_BOLT: &str = "bong:dugu_needle_bolt";
const VFX_DUGU_POISON_INFUSE: &str = "bong:dugu_poison_infuse";

/// 绝灵涡流（woliu v1 `woliu.vortex`）开涡起手式。
/// plan-skill-anim-fidelity-v1 P3 —— 专属双臂开涡动画（借用解除：原复用 v2
/// 涡旋站桩 `vortex_spiral_stance` 20t，瞬发招 [6,12] 时长域不符且与
/// woliu.heart 撞形）。field 出现即播（lifecycle 驱动，见
/// `emit_woliu_v1_vortex_visual_triggers`），非循环无需 StopAnim。
const ANIM_WOLIU_V1_STANCE: &str = "bong:woliu_vortex_cast";
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
// 写 `active:true`）；仅接生产可达的 woliu / zhenmai 两族——现有内容卷轴仅授
// `woliu.*` + `zhenmai.parry`，dugu / dugu_poison / baomai / tuike 四族无生产可达
// 习得路径（mentor helper 无生产调用方），与 stance_zhenfa 同为 report-only（见
// plan P1 表），待习得内容落地时连映射 + 共享清单 + 测试一起接入，不作接口预置。
// rune_draw / alchemy_stir / enlightenment_pose 为业务模块内联
// emit（无独立事件或事件覆盖不全，见各调用点注释），常数在此集中声明防漂移。
const ANIM_STANCE_WOLIU: &str = "bong:stance_woliu";
const ANIM_STANCE_ZHENMAI: &str = "bong:stance_zhenmai";
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

/// plan-skill-av-relink-v1 P3 — P1 全部 7 条新接线的 anim id 清单（**唯一真相源**，
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
pub(crate) const P1_WIRED_ANIM_IDS: [&str; 7] = [
    ANIM_STANCE_WOLIU,
    ANIM_STANCE_ZHENMAI,
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
    prune_stale_fist_combo(&mut fist_combo, clock.tick);
}

/// 剪除超出连击窗口的陈旧连击态：陈旧条目下次被使用时本就复位为右拳起手，
/// 剪除后行为完全等价——仅把表规模界定在最近活跃的攻击者内（Entity 键从不
/// 主动驱逐会随刷怪/despawn 复用索引缓慢泄漏，长跑服务端可观测）。
fn prune_stale_fist_combo(combo: &mut HashMap<Entity, FistComboState>, now_tick: u64) {
    combo.retain(|_, state| {
        now_tick.saturating_sub(state.last_punch_tick) <= FIST_COMBO_RESET_TICKS
    });
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
/// `active:true` = 激活）。卷轴习得走 `client_request_handler`；
/// `technique_mentor::mentor_teaches_technique` 为无生产调用方的休眠 helper。
/// 现有内容卷轴仅授 `woliu.*` + `zhenmai.parry` → 映射仅收录生产可达的
/// woliu / zhenmai 两族；dugu / dugu_poison / baomai / tuike 无生产可达习得
/// 路径，按断链原则不预置映射、降为 report-only（plan P1 表），待对应流派
/// 习得内容落地时连映射 + 清单 + 测试一起接入。
/// 全仓不存在「流派架势切换」gameplay 事件（`stance_switch` audio recipe 的唯一
/// 发射点是 SkillXpGain 经验反馈、与架势无关），按 plan §8 #2 决议改接本时刻。
/// 无映射前缀（sword / anqi / burst_meridian / movement / morph / body / npc /
/// shield_block / sword_path / dugu / baomai / tuike / zhenfa）不发；dev
/// `/technique active` 直改组件不发事件，不接线（dev-only 旁路）。
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
/// 仅收录生产可达的 woliu / zhenmai 两族（按前缀）；dugu / dugu_poison /
/// baomai / tuike 见 plan P1 表 report-only 行——无生产可达习得路径不预置
/// 映射，习得内容落地时在此补条目（同步扩 `P1_WIRED_ANIM_IDS` + 重生成清单）。
fn stance_anim_for_technique(technique_id: &str) -> Option<&'static str> {
    match technique_id.split('.').next()? {
        "woliu" => Some(ANIM_STANCE_WOLIU),
        "zhenmai" => Some(ANIM_STANCE_ZHENMAI),
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
        // 存活结局，无死亡动画清通道：显式 StopAnim 收掉 FULL_BODY brace 循环
        // （根因见 TRIBULATION_BRACE_STOP_FADE_OUT_TICKS 注释）。
        if let Ok((position, unique_id)) = players.get(failure.entity) {
            emit_stop_for_entity(
                position,
                unique_id,
                ANIM_TRIBULATION_BRACE,
                TRIBULATION_BRACE_STOP_FADE_OUT_TICKS,
                &mut vfx_events,
            );
        }
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

/// plan-gathering-tool-bind-v1 P1：草镰挥砍的草绿碎叶（`#7FA86A`，8 颗，8t）——
/// 复用既有 `BOTANY_HARVEST_VFX`（client `BotanyHarvestBurstPlayer` 已注册，纯 payload
/// 驱动 color/count/duration，无需新 event_id / 新 Java 类）。
const CAO_LIAN_SWING_COLOR: &str = "#7FA86A";
const CAO_LIAN_SWING_COUNT: u16 = 8;
const CAO_LIAN_SWING_DURATION_TICKS: u16 = 8;

/// plan-gathering-tool-bind-v1 P1：徒手割手的细红痕（`#C04848`，3 颗，8t）——同上复用
/// `BOTANY_HARVEST_VFX`，仅参数不同。
const BARE_HAND_WOUND_COLOR: &str = "#C04848";
const BARE_HAND_WOUND_COUNT: u16 = 3;
const BARE_HAND_WOUND_DURATION_TICKS: u16 = 8;

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
        let origin = valence::prelude::DVec3::new(pos[0], pos[1] + 0.45, pos[2]);
        emit_spawn_particle(
            &mut vfx_events,
            BOTANY_HARVEST_VFX,
            origin,
            botany_quality_color(event.spirit_quality),
            event.spirit_quality.clamp(0.5, 1.0),
            12,
            36,
        );
        // PR #1293 review 修正：required_tool_used/bare_hand_wound 是任意 required_tool
        // 草本的通用信号，仓库里已有 DunQiJia/GuaDao/BingJiaShouTao 三类既有 required_tool
        // 草本——必须额外限定 required_tool_kind == CaoLian，否则这些既有草本也会错误播放
        // 草镰专属粒子。
        let is_cao_lian = event.required_tool_kind == Some(ToolKind::CaoLian);
        if event.bare_hand_wound && is_cao_lian {
            emit_spawn_particle(
                &mut vfx_events,
                BOTANY_HARVEST_VFX,
                origin,
                BARE_HAND_WOUND_COLOR,
                0.9,
                BARE_HAND_WOUND_COUNT,
                BARE_HAND_WOUND_DURATION_TICKS,
            );
        } else if event.required_tool_used && is_cao_lian {
            // plan-gathering-tool-bind-v1 P1 视听规格"沿挥镰弧线"：方向取"玩家→采集目标"
            // 的水平向量（挥镰动作正是朝目标挥出），client BotanyHarvestBurstPlayer 消费
            // 后把粒子约束到该方向的扇形范围内，而非全向 360° 随机 burst。
            let direction =
                players
                    .get(event.client_entity)
                    .ok()
                    .and_then(|(player_position, _)| {
                        let player_pos = player_position.get();
                        let dx = origin.x - player_pos.x;
                        let dz = origin.z - player_pos.z;
                        (dx * dx + dz * dz > 1e-6).then_some([dx, 0.0, dz])
                    });
            emit_spawn_particle_with_direction(
                &mut vfx_events,
                BOTANY_HARVEST_VFX,
                origin,
                direction,
                CAO_LIAN_SWING_COLOR,
                0.85,
                CAO_LIAN_SWING_COUNT,
                CAO_LIAN_SWING_DURATION_TICKS,
            );
        }
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
/// 1. `PlayAnim`（P2 前半起五招全部专属动画，引用 `BongAnimations`）。
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
        // 1. 动画——五招各自专属动画（P2 前半起攻击三招不再借基础剑技）。
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

/// 暗器六招 cast → 动画 + 粒子（纯 cosmetic）。
///
/// 与剑道五招的 `emit_sword_path_visual_triggers` 同模式：读 anqi_v2 已 emit 的
/// 结果型 events，对 caster 发 `PlayAnim` + `SpawnParticle`，引用 client 已注册的
/// `AnqiVfxPlayer` 粒子 event_id 与 `BongAnimations` 动画 id。
///
/// 招式 → 事件源映射（P2 后半：封骨两段式 + 破甲/诱饵专属化）：
/// - 封骨开始 `CarrierChargeBeganEvent` → anqi_charge_carrier_loop 循环蓄力段起播
/// - 封骨结束 `CarrierChargeEndedEvent` → StopAnim(循环段)；`full_charge=true`
///   追加 anqi_charge_carrier_release 收势（打断不奖励收势，§8.1 #3）
/// - 封骨密封落定 `CarrierChargedEvent` → 仅封骨密封粒子（动画已由 Began/Ended
///   两段式接管，本事件不再播 windup_charge）
/// - 单射狙击 `QiInjectionEvent{SingleSnipe}` → anqi_single_snipe 专属动画 + 狙击弹道（caster→target 方向）
/// - 多发齐射 `MultiShotEvent` → anqi_multi_shot 专属动画 + 扇形齐射粒子
/// - 凝魂注射 `QiInjectionEvent{SoulInject}` → anqi_soul_inject 专属动画 + 魂注紫雾
/// - 破甲注射 `ArmorPierceEvent` → anqi_armor_pierce 专属动画 + 破甲金属火花（caster→target 方向）
/// - 诱饵分形 `EchoFractalEvent` → anqi_echo_fractal 专属动画 + 分形回响涟漪
///
/// **纯 cosmetic**：不读 / 改任何战斗数值 / qi_physics ledger / 命中结算。
#[allow(clippy::too_many_arguments)]
pub fn emit_anqi_visual_triggers(
    mut charge_begins: EventReader<CarrierChargeBeganEvent>,
    mut charge_ends: EventReader<CarrierChargeEndedEvent>,
    mut charges: EventReader<CarrierChargedEvent>,
    mut injections: EventReader<QiInjectionEvent>,
    mut multi_shots: EventReader<MultiShotEvent>,
    mut armor_pierces: EventReader<ArmorPierceEvent>,
    mut echoes: EventReader<EchoFractalEvent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    positions: Query<&Position>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    // 封骨充能开始：循环蓄力段起播（结印灌注微循环）。
    for event in charge_begins.read() {
        emit_play_for_entity(
            event.carrier,
            ANIM_ANQI_CHARGE,
            ANQI_PRIORITY,
            Some(2),
            &players,
            &mut vfx_events,
        );
    }

    // 封骨充能结束：任何退出路径都停循环段（§8.1 #3 停止路径红线）；
    // 仅自然完成（full_charge）奖励 release 收势。
    for event in charge_ends.read() {
        let Ok((position, unique_id)) = players.get(event.carrier) else {
            continue;
        };
        emit_stop_for_entity(
            position,
            unique_id,
            ANIM_ANQI_CHARGE,
            ANQI_CHARGE_STOP_FADE_OUT_TICKS,
            &mut vfx_events,
        );
        if event.full_charge {
            emit_play_for_entity(
                event.carrier,
                ANIM_ANQI_CHARGE_RELEASE,
                ANQI_PRIORITY,
                Some(1),
                &players,
                &mut vfx_events,
            );
        }
    }

    // 封骨密封落定：仅密封粒子（骨白）——动画归 Began/Ended 两段式，负向锁
    // 「本事件不再播 windup_charge」见测试。
    for event in charges.read() {
        let Some(origin) = positions.get(event.carrier).map(|p| p.get()).ok() else {
            continue;
        };
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

    // 单射狙击 / 凝魂注射：QiInjectionEvent 区分招式。狙击走 anqi_single_snipe
    // 专属动画 + 弹道粒子，凝魂走 anqi_soul_inject 专属动画 + 魂注紫雾。
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

    // 多发齐射：anqi_multi_shot 专属动画 + 扇形齐射粒子（散射弹幕）。
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

    // 破甲注射：anqi_armor_pierce 专属动画（P2 后半解除 cast_invoke 借用）
    // + 破甲金属火花（caster→target 朝向）。
    for event in armor_pierces.read() {
        let origin = positions
            .get(event.caster)
            .map(|p| p.get())
            .unwrap_or_default();
        let direction = caster_target_direction(&positions, event.caster, event.target);
        emit_play_for_entity(
            event.caster,
            ANIM_ANQI_ARMOR_PIERCE,
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

    // 诱饵分形：anqi_echo_fractal 专属动画（P2 后半解除 release_burst 借用）
    // + 分形回响涟漪（分身数缩放粒子）。
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

    // 灌毒蛊：专属淬毒动画（P3 去共用，原与凝针同发 throw）+ 失谐真元毒绿雾
    // （覆入飞针，无方向，绕身散布）。
    for event in infusions.read() {
        let origin = positions
            .get(event.infuser)
            .map(|p| p.get())
            .unwrap_or_default();
        emit_play_for_entity(
            event.infuser,
            ANIM_DUGU_INFUSE_POISON,
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
        // P2 前半去复用：凝锋（收剑入鞘式蓄意→拔剑亮刃）/ 剑气斩（回环蓄势→
        // 大斩送远）/ 共鸣（颤鸣蓄振→振荡外放）各自专属动画。
        SwordPathSkillId::CondenseEdge => ANIM_SWORD_PATH_CONDENSE_EDGE,
        SwordPathSkillId::QiSlash => ANIM_SWORD_PATH_QI_SLASH,
        SwordPathSkillId::Resonance => ANIM_SWORD_PATH_RESONANCE,
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
    emit_spawn_particle_with_direction(
        vfx_events,
        event_id,
        origin,
        None,
        color,
        strength,
        count,
        duration_ticks,
    );
}

/// 同 `emit_spawn_particle`，但可携带方向向量（未归一，客户端按需处理）——
/// plan-gathering-tool-bind-v1 P1：草镰挥砍粒子借此表达"沿挥镰弧线"（client
/// `BotanyHarvestBurstPlayer` 消费 direction 时把粒子约束到该方向的扇形范围内，
/// 而非全向 360° 随机 burst）。
#[allow(clippy::too_many_arguments)]
fn emit_spawn_particle_with_direction(
    vfx_events: &mut EventWriter<VfxEventRequest>,
    event_id: &'static str,
    origin: valence::prelude::DVec3,
    direction: Option<[f64; 3]>,
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
            direction,
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
#[path = "vfx_animation_trigger_tests.rs"]
mod tests;
