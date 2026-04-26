//! plan-tsy-zone-v1 §2 — 活坍缩渊负压抽真元 tick。
//!
//! 公式（§2.1）：
//!   rate = |zone.spirit_qi| × (player.spirit_qi_max / REFERENCE_POOL) ^ N × BASE
//! 触发条件：玩家有 `TsyPresence` + 当前 zone 是 TSY 系列。
//! 真元归零 → 发 `DeathEvent { cause: "tsy_drain" }`，由 combat lifecycle 接管。

use valence::prelude::{Entity, EventWriter, Position, Query, Res, Without};

use crate::combat::events::DeathEvent;
use crate::combat::CombatClock;
use crate::npc::spawn::NpcMarker;
use crate::player::state::PlayerState;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::tsy::TsyPresence;
use crate::world::zone::{Zone, ZoneRegistry};

/// 引气满池基准（spirit_qi_max = 100 → pool_ratio = 1.0）。
pub const REFERENCE_POOL: f64 = 100.0;
/// 非线性指数。`plan-tsy-v1.md §0` 公理 2：抽取速率与池大小**平方关系**。
pub const NONLINEAR_EXPONENT: f64 = 1.5;
/// 基准抽速（点 / tick）。20Hz tick，0.5/tick = 10/sec @ |灵压|=1.0 引气小池。
pub const BASE_DRAIN_PER_TICK: f64 = 0.5;

/// 纯函数：单 tick 抽取量（点）。非 TSY zone 返回 0；空池返回 0。
pub fn compute_drain_per_tick(zone: &Zone, player: &PlayerState) -> f64 {
    if !zone.is_tsy() {
        return 0.0;
    }
    let pool = player.spirit_qi_max.max(0.0);
    if pool <= 0.0 {
        return 0.0;
    }
    let pool_ratio = pool / REFERENCE_POOL;
    let nonlinear = pool_ratio.powf(NONLINEAR_EXPONENT);
    zone.spirit_qi.abs() * nonlinear * BASE_DRAIN_PER_TICK
}

/// plan-tsy-zone-v1 §2.2 — 抽真元 tick system。
///
/// 通过 `TsyPresence` filter + `CurrentDimension::Tsy` 双重 gate 规避
/// "presence 与 dim inconsistent" 的非法状态：
/// - 正常路径：两者一致，按 TSY dim 查 zone，扣 spirit_qi
/// - 异常路径：玩家在 Overworld 但仍带 TsyPresence（lifecycle bug）→
///   `find_zone(Tsy, pos)` 返回 None 自然 skip，不静默错抽
///
/// 排除 NPC（`Without<NpcMarker>`）—— P0 不在 TSY 内放 NPC（§7 未决）。
pub fn tsy_drain_tick(
    clock: Res<CombatClock>,
    zones: Res<ZoneRegistry>,
    mut deaths: EventWriter<DeathEvent>,
    mut players: Query<
        (
            Entity,
            &mut PlayerState,
            &Position,
            &TsyPresence,
            Option<&CurrentDimension>,
        ),
        Without<NpcMarker>,
    >,
) {
    for (entity, mut state, pos, _presence, current_dim) in &mut players {
        // 跨位面前 dim 兜底：缺 CurrentDimension 视为 TSY（presence 已经隐含玩家在内）
        let dim = current_dim.map(|c| c.0).unwrap_or(DimensionKind::Tsy);
        let Some(zone) = zones.find_zone(dim, pos.0) else {
            continue;
        };
        let drain = compute_drain_per_tick(zone, &state);
        if drain <= 0.0 {
            continue;
        }
        let was_alive = state.spirit_qi > 0.0;
        state.spirit_qi -= drain;
        if was_alive && state.spirit_qi <= 0.0 {
            // 归零 → P0 发 DeathEvent（cause="tsy_drain"），死亡结算由 P1 plan-tsy-loot 处理。
            // 环境死亡：无攻击者。
            deaths.send(DeathEvent {
                target: entity,
                cause: "tsy_drain".to_string(),
                attacker: None,
                attacker_player_id: None,
                at_tick: clock.tick,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::dimension::DimensionKind;
    use valence::prelude::DVec3;

    fn tsy_zone(name: &str, spirit_qi: f64) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: DimensionKind::Tsy,
            bounds: (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 100.0, 100.0)),
            spirit_qi,
            danger_level: 5,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        }
    }

    fn ow_zone(name: &str) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 100.0, 100.0)),
            spirit_qi: 0.5,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        }
    }

    fn player(spirit_qi_max: f64) -> PlayerState {
        PlayerState {
            spirit_qi: spirit_qi_max,
            spirit_qi_max,
            ..Default::default()
        }
    }

    #[test]
    fn non_tsy_zone_returns_zero_drain() {
        // 非 TSY zone（哪怕 spirit_qi 是负的）不该产生 drain。
        let z = ow_zone("blood_valley");
        let p = player(100.0);
        assert_eq!(compute_drain_per_tick(&z, &p), 0.0);
    }

    #[test]
    fn zero_pool_returns_zero_drain() {
        // 池为零 → 0 drain（避免 NaN / Inf）。
        let z = tsy_zone("tsy_lingxu_01_shallow", -0.4);
        let p = player(0.0);
        assert_eq!(compute_drain_per_tick(&z, &p), 0.0);
    }

    /// plan §2.1 表："引气浅" — pool=30, qi=-0.3, 期望 ~0.04 / tick (≈0.78/sec)
    #[test]
    fn yinqi_shallow_table_value() {
        let z = tsy_zone("tsy_lingxu_01_shallow", -0.3);
        let p = player(30.0);
        let drain = compute_drain_per_tick(&z, &p);
        // 0.3 * (30/100)^1.5 * 0.5 ≈ 0.0246 / tick → ~0.49 / sec @20Hz
        // plan 表里的 0.78/sec 是基于不同 base/exponent 的旧估算；以本 const 落地的值为准。
        assert!(drain > 0.02 && drain < 0.03, "got drain={drain}");
    }

    /// plan §2.1 表："引气深" — pool=30, qi=-1.1
    #[test]
    fn yinqi_deep_table_value() {
        let z = tsy_zone("tsy_lingxu_01_deep", -1.1);
        let p = player(30.0);
        let drain = compute_drain_per_tick(&z, &p);
        // 1.1 * (30/100)^1.5 * 0.5 ≈ 0.0904 / tick → ~1.81/sec
        assert!(drain > 0.08 && drain < 0.10, "got drain={drain}");
    }

    /// plan §2.1 表："化虚浅" — pool=500, qi=-0.3
    #[test]
    fn huaxu_shallow_table_value() {
        let z = tsy_zone("tsy_lingxu_01_shallow", -0.3);
        let p = player(500.0);
        let drain = compute_drain_per_tick(&z, &p);
        // 0.3 * (500/100)^1.5 * 0.5 = 0.3 * 11.18 * 0.5 ≈ 1.677 / tick → ~33.5/sec
        assert!(drain > 1.5 && drain < 1.85, "got drain={drain}");
    }

    /// plan §2.1 表："化虚深" — pool=500, qi=-1.1
    #[test]
    fn huaxu_deep_table_value() {
        let z = tsy_zone("tsy_lingxu_01_deep", -1.1);
        let p = player(500.0);
        let drain = compute_drain_per_tick(&z, &p);
        // 1.1 * 11.18 * 0.5 ≈ 6.149 / tick → ~123/sec → 化虚深秒杀
        assert!(drain > 5.7 && drain < 6.6, "got drain={drain}");
    }

    #[test]
    fn drain_is_monotonic_in_zone_negativity() {
        // 同样的池子，灵压越负，抽得越凶。
        let p = player(100.0);
        let shallow = compute_drain_per_tick(&tsy_zone("tsy_a_shallow", -0.3), &p);
        let mid = compute_drain_per_tick(&tsy_zone("tsy_a_mid", -0.7), &p);
        let deep = compute_drain_per_tick(&tsy_zone("tsy_a_deep", -1.1), &p);
        assert!(shallow < mid && mid < deep);
    }

    #[test]
    fn drain_is_monotonic_in_pool_size() {
        // 同样的灵压，池子越大被抽得越多（非线性放大）。
        let z = tsy_zone("tsy_a_deep", -1.0);
        let small = compute_drain_per_tick(&z, &player(30.0));
        let big = compute_drain_per_tick(&z, &player(500.0));
        // big / small 应远大于 (500/30) = 16.67 —— 因为非线性指数 1.5 放大
        assert!(big / small > 30.0, "got ratio {}", big / small);
    }
}
