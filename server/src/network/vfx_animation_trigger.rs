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
use crate::combat::carrier::CarrierChargedEvent;
use crate::combat::components::WoundKind;
use crate::combat::events::{AttackIntent, AttackSource, CombatEvent, DefenseIntent};
use crate::combat::tuike_v2::{ContamTransferredEvent, DonFalseSkinEvent, FalseSkinSheddedEvent};
use crate::combat::woliu_v2::state::VortexV2State;
use crate::combat::woliu_v2::{VortexCastEvent, WoliuSkillId};
use crate::combat::CombatClock;
use crate::cultivation::breakthrough::BreakthroughOutcome;
use crate::cultivation::tribulation::{TribulationAnnounce, TribulationFailed, TribulationSettled};
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

const COMBAT_PRIORITY: u16 = 1000;
const HIT_RECOIL_PRIORITY: u16 = 2000;
const STORY_PRIORITY: u16 = 3000;

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
pub fn emit_attack_animation_triggers(
    mut intents: EventReader<AttackIntent>,
    players: Query<PlayerAnimTargetItem<'_>, PlayerAnimTargetFilter>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for intent in intents.read() {
        if intent.source == AttackSource::BurstMeridian || is_sword_path_source(intent.source) {
            continue;
        }
        let anim_id = attack_anim_for_source(intent.source, intent.wound_kind);
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
}
