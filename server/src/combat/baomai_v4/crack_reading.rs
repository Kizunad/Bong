//! plan-baomai-v4 §5 — 裂读（Crack Reading）。
//!
//! 体修近战命中时探测对方经脉损伤分布。高 ρ 排斥系数让裂纹像雷达回波一样接收
//! 异种真元信号。裂纹越多接收灵敏度越高。
//!
//! **§8.1 #4 决议**：裂读对 NPC 生效但降级为 3 档精度（Intact/Damaged/Severed）。
//! NPC 无 ActiveScarCircuits / DeadMeridianArmor → has_circuit / is_dead_armor 始终 false。

use serde::Serialize;
use valence::prelude::{bevy_ecs, Component, Entity, EventReader, EventWriter, Query, Res};

use crate::combat::baomai_v4::constants::{
    CRACK_READING_DEEP_WINDOW_TICKS, CRACK_READING_DISPLAY_TICKS, CRACK_READING_RATE_CAP,
    CRACK_READING_RATE_PER_OVERLOAD,
};
use crate::combat::baomai_v4::dead_armor::DeadMeridianArmor;
use crate::combat::baomai_v4::events::CrackReadingResultEvent;
use crate::combat::baomai_v4::scar_circuit::ActiveScarCircuits;
use crate::combat::baomai_v4::scar_history::ScarHistory;
use crate::combat::events::{AttackSource, CombatEvent};
use crate::combat::CombatClock;
use crate::cultivation::components::{MeridianId, MeridianSystem};
use crate::npc::spawn::NpcMarker;

// ── 组件 ──

/// 裂读状态跟踪——记录上次裂读 tick 和目标，用于深读窗口判定。
#[derive(Component, Debug, Clone, Default)]
pub struct CrackReadingState {
    /// 上次成功裂读的 tick。
    pub last_read_tick: Option<u64>,
    /// 上次裂读的目标。
    pub last_read_target: Option<Entity>,
}

// ── 裂读结果 ──

/// 裂读结果——发送给 client 渲染，也供 event 消费。
#[derive(Debug, Clone, Serialize)]
pub struct CrackReadingResult {
    pub target: Entity,
    pub meridian_states: Vec<MeridianReadEntry>,
    pub is_deep_read: bool,
    pub display_until_tick: u64,
}

/// 单条经脉裂读条目。
#[derive(Debug, Clone, Serialize)]
pub struct MeridianReadEntry {
    pub id: MeridianId,
    /// 浅读：对玩家目标四档，对 NPC 三档（映射为 PlayerBracket 后在序列化层区分）。
    pub integrity_bracket: IntegrityBracket,
    /// 深读才有意义。NPC 目标始终 false。
    pub has_circuit: bool,
    /// 深读才有意义。NPC 目标始终 false。
    pub is_dead_armor: bool,
}

/// 玩家目标经脉完整度四档（不泄露精确数值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum IntegrityBracket {
    /// integrity >= 0.85
    Intact,
    /// 0.50 <= integrity < 0.85
    MicroTear,
    /// 0.0 < integrity < 0.50
    Torn,
    /// integrity == 0.0（或 SEVERED）
    Severed,
}

/// NPC 目标经脉完整度三档（合并 MicroTear+Torn → Damaged）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum NpcIntegrityBracket {
    /// integrity >= 0.85
    Intact,
    /// 0.0 < integrity < 0.85
    Damaged,
    /// integrity == 0.0（或 SEVERED）
    Severed,
}

// ── 映射函数 ──

/// 玩家目标：四档精度。
pub fn bracket_for_player(integrity: f64) -> IntegrityBracket {
    if integrity <= 0.0 {
        IntegrityBracket::Severed
    } else if integrity < 0.50 {
        IntegrityBracket::Torn
    } else if integrity < 0.85 {
        IntegrityBracket::MicroTear
    } else {
        IntegrityBracket::Intact
    }
}

/// NPC 目标：三档精度（§8.1 #4 决议）。
pub fn bracket_for_npc(integrity: f64) -> NpcIntegrityBracket {
    if integrity <= 0.0 {
        NpcIntegrityBracket::Severed
    } else if integrity < 0.85 {
        NpcIntegrityBracket::Damaged
    } else {
        NpcIntegrityBracket::Intact
    }
}

/// 将 NpcIntegrityBracket 映射为 IntegrityBracket（用于统一 MeridianReadEntry）。
///
/// Damaged 映射为 MicroTear（保守：只告知"有损"不区分轻重）。
fn npc_bracket_to_player(b: NpcIntegrityBracket) -> IntegrityBracket {
    match b {
        NpcIntegrityBracket::Intact => IntegrityBracket::Intact,
        NpcIntegrityBracket::Damaged => IntegrityBracket::MicroTear,
        NpcIntegrityBracket::Severed => IntegrityBracket::Severed,
    }
}

// ── 概率计算 ──

/// 计算裂读成功率：min(total_overloads * 0.004, 0.50)。
pub fn crack_reading_success_rate(total_overloads: u32) -> f64 {
    let rate = total_overloads as f64 * CRACK_READING_RATE_PER_OVERLOAD;
    rate.min(CRACK_READING_RATE_CAP)
}

// ── 近战判定 ──

/// 判断攻击来源是否为近战（裂读要求肢体接触才能传回信号）。
pub fn is_melee_source(source: AttackSource) -> bool {
    matches!(
        source,
        AttackSource::Melee
            | AttackSource::BurstMeridian
            | AttackSource::FullPower
            | AttackSource::SwordCleave
            | AttackSource::SwordThrust
            | AttackSource::SwordPathCondenseEdge
    )
}

// ── 构建裂读结果 ──

/// 构建裂读结果。
///
/// `is_npc` 为 true 时使用 3 档精度，且 has_circuit / is_dead_armor 始终 false。
/// `is_deep` 为 true 时才填充 has_circuit / is_dead_armor。
pub fn build_reading_result(
    target: Entity,
    meridians: &MeridianSystem,
    circuits: Option<&ActiveScarCircuits>,
    armor: Option<&DeadMeridianArmor>,
    is_npc: bool,
    is_deep: bool,
    current_tick: u64,
) -> CrackReadingResult {
    let mut entries = Vec::with_capacity(20);

    for meridian in meridians.iter() {
        let bracket = if is_npc {
            npc_bracket_to_player(bracket_for_npc(meridian.integrity))
        } else {
            bracket_for_player(meridian.integrity)
        };

        let has_circuit = if is_deep && !is_npc {
            circuits.is_some_and(|c| c.circuits.iter().any(|kind| kind.involves(meridian.id)))
        } else {
            false
        };

        let is_dead_armor = if is_deep && !is_npc {
            armor.is_some_and(|a| {
                crate::combat::baomai_v4::dead_armor::meridian_to_body_part(meridian.id)
                    .is_some_and(|bp| a.is_immune(bp))
            })
        } else {
            false
        };

        entries.push(MeridianReadEntry {
            id: meridian.id,
            integrity_bracket: bracket,
            has_circuit,
            is_dead_armor,
        });
    }

    CrackReadingResult {
        target,
        meridian_states: entries,
        is_deep_read: is_deep,
        display_until_tick: current_tick.saturating_add(CRACK_READING_DISPLAY_TICKS),
    }
}

// ── Client payload 序列化 ──

/// `bong:crack_reading` CustomPayload 的 JSON 格式。
///
/// plan-combat-skill-feedback-bridges-v1 P1：baomai_v4_event_bridge::emit_crack_reading_payload 使用。
#[derive(Debug, Clone, Serialize)]
pub struct CrackReadingPayload {
    pub target_entity_id: u64,
    pub entries: Vec<CrackReadingPayloadEntry>,
    pub is_deep: bool,
    pub display_ticks: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrackReadingPayloadEntry {
    pub meridian: String,
    pub bracket: String,
    pub has_circuit: bool,
    pub is_dead_armor: bool,
}

/// 将 CrackReadingResult 转换为 client payload JSON 格式。
pub fn to_client_payload(result: &CrackReadingResult, current_tick: u64) -> CrackReadingPayload {
    let entries = result
        .meridian_states
        .iter()
        .map(|e| CrackReadingPayloadEntry {
            meridian: format!("{:?}", e.id),
            bracket: format!("{:?}", e.integrity_bracket),
            has_circuit: e.has_circuit,
            is_dead_armor: e.is_dead_armor,
        })
        .collect();

    CrackReadingPayload {
        target_entity_id: result.target.to_bits(),
        entries,
        is_deep: result.is_deep_read,
        display_ticks: result
            .display_until_tick
            .saturating_sub(current_tick)
            .min(CRACK_READING_DISPLAY_TICKS),
    }
}

// ── System ──

/// CombatEvent 结算后裂读判定系统。
///
/// 订阅 CombatEvent，对每次近战命中：
/// 1. 检查 attacker 是否有 ScarHistory（体修经验）
/// 2. 按 success_rate = min(total_overloads * 0.004, 0.50) 概率判定
/// 3. 浅读 → 显示所有 20 条经脉的 IntegrityBracket
/// 4. 深读 → 3s 内二次命中同一目标时额外显示 has_circuit / is_dead_armor
/// 5. emit CrackReadingResultEvent
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn crack_reading_system(
    clock: Res<CombatClock>,
    mut combat_events: EventReader<CombatEvent>,
    mut readers: Query<(&ScarHistory, &mut CrackReadingState)>,
    targets: Query<(
        &MeridianSystem,
        Option<&ActiveScarCircuits>,
        Option<&DeadMeridianArmor>,
        Option<&NpcMarker>,
    )>,
    mut result_events: EventWriter<CrackReadingResultEvent>,
) {
    for event in combat_events.read() {
        // Only trigger on melee hits.
        if !is_melee_source(event.source) {
            continue;
        }

        // Attacker must have ScarHistory and CrackReadingState.
        let Ok((history, mut reading_state)) = readers.get_mut(event.attacker) else {
            continue;
        };

        // Calculate success rate.
        let success_rate = crack_reading_success_rate(history.total_overloads);
        if success_rate <= 0.0 {
            continue;
        }

        // Probabilistic roll. Use a deterministic-friendly approach for testability:
        // In production, use rand. For now, hash-based pseudo-random seeded on tick+entity.
        let roll = pseudo_random_roll(clock.tick, event.attacker, event.target);
        if roll >= success_rate {
            continue;
        }

        // Read target's meridian system.
        let Ok((target_meridians, target_circuits, target_armor, npc_marker)) =
            targets.get(event.target)
        else {
            continue;
        };

        let is_npc = npc_marker.is_some();

        // Deep read check: same target within CRACK_READING_DEEP_WINDOW_TICKS.
        let is_deep = reading_state
            .last_read_tick
            .zip(reading_state.last_read_target)
            .is_some_and(|(last_tick, last_target)| {
                last_target == event.target
                    && clock.tick.saturating_sub(last_tick) < CRACK_READING_DEEP_WINDOW_TICKS
            });

        // Build result.
        let result = build_reading_result(
            event.target,
            target_meridians,
            target_circuits,
            target_armor,
            is_npc,
            is_deep,
            clock.tick,
        );

        // Update reading state.
        reading_state.last_read_tick = Some(clock.tick);
        reading_state.last_read_target = Some(event.target);

        // Emit event.
        result_events.send(CrackReadingResultEvent {
            reader: event.attacker,
            target: event.target,
            result,
            tick: clock.tick,
        });
    }
}

/// Deterministic pseudo-random roll for testability.
///
/// Uses a simple hash of tick + entity bits to produce a [0.0, 1.0) value.
/// In production this should be replaced with a proper RNG, but determinism
/// makes testing straightforward.
pub fn pseudo_random_roll(tick: u64, attacker: Entity, target: Entity) -> f64 {
    // FNV-1a inspired hash for reproducibility.
    let mut h: u64 = 0xcbf29ce484222325;
    for byte in tick.to_le_bytes() {
        h ^= byte as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    for byte in attacker.to_bits().to_le_bytes() {
        h ^= byte as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    for byte in target.to_bits().to_le_bytes() {
        h ^= byte as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    // Normalize to [0.0, 1.0).
    (h >> 11) as f64 / ((1u64 << 53) as f64)
}
