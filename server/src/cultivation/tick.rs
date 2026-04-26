//! QiRegenTick + ZoneQiDrainTick（plan §2 QiRegenTick / ZoneQiDrainTick）。
//!
//! 两者合并到一个 system 里执行以天然保证零和：玩家每 tick 吸纳的 qi
//! 必然等量从 zone.spirit_qi 扣除（按 `QI_PER_ZONE_UNIT` 换算）。符合
//! worldview §一"灵气零和守恒"公理。
//!
//! P1 简化：无「静坐/行动」区分，全部按被动小系数回；静坐/打坐在 P1 末
//! 加客户端指令时再接入。

use valence::prelude::{bevy_ecs, Position, Query, ResMut, Resource};

use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

use super::components::{Cultivation, MeridianSystem};

/// 全局 tick 计数器 — 用于标记 last_qi_zero_at 等时间戳。
#[derive(Debug, Default, Resource)]
pub struct CultivationClock {
    pub tick: u64,
}

/// 每 tick 真元回复的归一化系数。
pub const QI_REGEN_COEF: f64 = 0.01;
/// 1.0 单位 zone concentration 可支撑多少 qi 吸纳 — 决定 zone 枯竭速度。
/// 数值越大 zone 越耐抽。
pub const QI_PER_ZONE_UNIT: f64 = 50.0;

/// 纯函数：给定 zone 浓度、rate、可用额度（qi_max - qi_current - qi_max_frozen 等）
/// 计算本 tick 的实际 gain 与 zone 浓度变化量（均为非负）。
pub fn compute_regen(zone_qi: f64, rate: f64, avg_integrity: f64, qi_room: f64) -> (f64, f64) {
    if zone_qi <= 0.0 || rate <= 0.0 || qi_room <= 0.0 {
        return (0.0, 0.0);
    }
    let raw_gain = zone_qi * rate * avg_integrity * QI_REGEN_COEF;
    // 池容量上限
    let capped_gain = raw_gain.min(qi_room);
    // 该次 gain 对应 zone 浓度扣减
    let drain = capped_gain / QI_PER_ZONE_UNIT;
    // 若扣减将 zone 拉到负值，再等比回退 gain
    if drain > zone_qi {
        let actual_drain = zone_qi;
        let actual_gain = actual_drain * QI_PER_ZONE_UNIT;
        (actual_gain, actual_drain)
    } else {
        (capped_gain, drain)
    }
}

/// QiRegenTick + ZoneQiDrainTick 合并实现。零和：玩家 qi 增量 = zone 浓度减量 × coef。
pub fn qi_regen_and_zone_drain_tick(
    mut clock: ResMut<CultivationClock>,
    zone_registry: Option<ResMut<ZoneRegistry>>,
    mut players: Query<(
        &Position,
        Option<&CurrentDimension>,
        &MeridianSystem,
        &mut Cultivation,
    )>,
) {
    clock.tick = clock.tick.wrapping_add(1);

    let Some(mut zones) = zone_registry else {
        return;
    };

    for (pos, current_dim, meridians, mut cultivation) in players.iter_mut() {
        // 通过 pos 找到 zone 的 name（不持可变借用）；entity 缺 CurrentDimension
        // 时按 Overworld 处理（NPC 暂未跨位面）。Player 在 spawn 时一定带
        // CurrentDimension（apply_spawn_defaults / restore_player_dimension）。
        let dim = current_dim.map(|c| c.0).unwrap_or(DimensionKind::Overworld);
        let Some(zone_name) = zones.find_zone(dim, pos.0).map(|z| z.name.clone()) else {
            continue;
        };
        let Some(zone) = zones.find_zone_mut(&zone_name) else {
            continue;
        };
        if zone.spirit_qi <= 0.0 {
            continue;
        }

        let rate = {
            let sum = meridians.sum_rate();
            if sum > 0.0 {
                sum
            } else {
                0.1 // Awaken 期的「基础吸纳」
            }
        };
        let avg_integrity = {
            let total: f64 = meridians.iter().map(|m| m.integrity).sum();
            let n = meridians.iter().count() as f64;
            if n > 0.0 {
                total / n
            } else {
                1.0
            }
        };
        let effective_max = cultivation.qi_max - cultivation.qi_max_frozen.unwrap_or(0.0);
        let qi_room = (effective_max - cultivation.qi_current).max(0.0);

        let (gain, drain) = compute_regen(zone.spirit_qi, rate, avg_integrity, qi_room);
        if gain <= 0.0 {
            continue;
        }

        cultivation.qi_current += gain;
        zone.spirit_qi = (zone.spirit_qi - drain).max(0.0);

        if cultivation.qi_current > 0.0 {
            cultivation.last_qi_zero_at = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_gain_in_dead_zone() {
        let (g, d) = compute_regen(0.0, 1.0, 1.0, 100.0);
        assert_eq!(g, 0.0);
        assert_eq!(d, 0.0);
    }

    #[test]
    fn gain_drains_zone_by_ratio() {
        let (g, d) = compute_regen(0.5, 1.0, 1.0, 100.0);
        assert!(g > 0.0);
        // gain / QI_PER_ZONE_UNIT == drain
        assert!((g - d * QI_PER_ZONE_UNIT).abs() < 1e-9);
    }

    #[test]
    fn qi_room_caps_gain() {
        let (g, d) = compute_regen(1.0, 100.0, 1.0, 0.5);
        assert!(g <= 0.5);
        // 即使被 qi_room 截断，drain 依然按 gain 换算
        assert!((g - d * QI_PER_ZONE_UNIT).abs() < 1e-9);
    }

    #[test]
    fn drain_clamped_to_zone_available() {
        // rate 巨大会把 drain 推到超过 zone_qi
        let zone_qi = 0.001;
        let (g, d) = compute_regen(zone_qi, 1e6, 1.0, 1e9);
        assert!(d <= zone_qi + 1e-12);
        assert!((g - d * QI_PER_ZONE_UNIT).abs() < 1e-6);
    }

    #[test]
    fn zero_sum_property() {
        // 多次 tick 后累积玩家 gain == 累积 zone drain × QI_PER_ZONE_UNIT
        let mut zone_qi = 0.5;
        let mut player_qi = 0.0;
        for _ in 0..50 {
            let room = 1e9_f64;
            let (g, d) = compute_regen(zone_qi, 1.0, 1.0, room);
            player_qi += g;
            zone_qi -= d;
        }
        let leaked = player_qi - (0.5 - zone_qi) * QI_PER_ZONE_UNIT;
        assert!(leaked.abs() < 1e-6);
    }

    #[test]
    fn integrity_scales_gain() {
        let (g_full, _) = compute_regen(0.5, 1.0, 1.0, 1e9);
        let (g_half, _) = compute_regen(0.5, 1.0, 0.5, 1e9);
        assert!((g_half - g_full * 0.5).abs() < 1e-9);
    }
}
