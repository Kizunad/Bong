//! plan-tsy-zone-v1 §2 — 活坍缩渊负压抽真元 tick。
//!
//! 公式（§2.1）：
//!   rate = |zone.spirit_qi| × (cultivation.qi_max / QI_TSY_REFERENCE_POOL) ^
//!          QI_TSY_DRAIN_NONLINEAR_EXPONENT × QI_TSY_BASE_DRAIN_PER_TICK
//! 触发条件：玩家有 `TsyPresence` + 当前 zone 是 TSY 系列。
//! 真元归零 → 发 `DeathEvent { cause: "tsy_drain" }`，由 combat lifecycle 接管。

use valence::prelude::{Entity, EventWriter, Position, Query, Res, ResMut, With, Without};

use crate::combat::events::DeathEvent;
use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;
use crate::npc::spawn::NpcMarker;
use crate::npc::tsy_hostile::{compute_fuya_aura_drain_multiplier, FuyaAura};
use crate::qi_physics::constants::{
    QI_AMBIENT_EXCRETION_PER_SEC, QI_TSY_BASE_DRAIN_PER_TICK, QI_TSY_DRAIN_NONLINEAR_EXPONENT,
    QI_TSY_REFERENCE_POOL, QI_TSY_SEARCH_EXPOSURE_FACTOR,
};
use crate::qi_physics::{
    qi_excretion_loss, ContainerKind, EnvField, QiAccountId, QiTransfer, QiTransferReason,
    WorldQiAccount,
};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::tsy::TsyPresence;
use crate::world::tsy_container_search::IsSearching;
use crate::world::zone::{Zone, ZoneRegistry};

type TsyDrainPlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Cultivation,
        &'static Position,
        &'static TsyPresence,
        Option<&'static CurrentDimension>,
        Option<&'static IsSearching>,
    ),
    Without<NpcMarker>,
>;

/// 纯函数：单 tick 基础抽取量（点）。非 TSY zone 返回 0；空池返回 0。
///
/// **注意**：本函数不含搜刮 1.5× 乘数；调 [`compute_search_drain_multiplier`]
/// 拿乘数自己叠（`tsy_drain_tick` 已经走整合路径）。
pub fn compute_drain_per_tick(zone: &Zone, cultivation: &Cultivation) -> f64 {
    if !zone.is_tsy() {
        return 0.0;
    }
    let pool = cultivation.qi_max.max(0.0);
    if pool <= 0.0 {
        return 0.0;
    }
    let pool_ratio = pool / QI_TSY_REFERENCE_POOL;
    let nonlinear = pool_ratio.powf(QI_TSY_DRAIN_NONLINEAR_EXPONENT);
    let intensity = zone.spirit_qi.abs();
    let env = EnvField {
        local_zone_qi: 0.0,
        tsy_intensity: intensity.clamp(0.0, 1.0),
        ..EnvField::default()
    };
    let canonical_loss = qi_excretion_loss(intensity, ContainerKind::AmbientField, 1.0, env);
    let normalized_loss = (canonical_loss / QI_AMBIENT_EXCRETION_PER_SEC).max(0.0);
    normalized_loss * nonlinear * QI_TSY_BASE_DRAIN_PER_TICK
}

/// plan-tsy-container-v1 §2.3 — 搜刮中真元抽取乘数。
/// 搜刮是主动暴露行为：抽吸速率在 baseline 上 ×1.5。
pub fn compute_search_drain_multiplier(in_search: bool) -> f64 {
    if in_search {
        QI_TSY_SEARCH_EXPOSURE_FACTOR
    } else {
        1.0
    }
}

fn record_tsy_drain_transfer(
    account: Option<&mut WorldQiAccount>,
    player: Entity,
    zone_name: &str,
    amount: f64,
    before_player_qi: f64,
) {
    let Some(account) = account else {
        return;
    };
    if amount <= 0.0 {
        return;
    }
    let from = QiAccountId::player(format!("entity:{player:?}"));
    let to = QiAccountId::rift(zone_name.to_string());
    let source_balance = account.balance(&from).max(before_player_qi.max(amount));
    if account.set_balance(from.clone(), source_balance).is_err() {
        return;
    }
    let Ok(transfer) = QiTransfer::new(from, to, amount, QiTransferReason::RiftCollapse) else {
        return;
    };
    let _ = account.transfer(transfer);
}

/// plan-tsy-zone-v1 §2.2 — 抽真元 tick system。
///
/// 通过 `TsyPresence` filter + `CurrentDimension::Tsy` 双重 gate 规避
/// "presence 与 dim inconsistent" 的非法状态：
/// - 正常路径：两者一致，按 TSY dim 查 zone，扣 cultivation.qi_current
/// - 异常路径：玩家在 Overworld 但仍带 TsyPresence（lifecycle bug）→
///   `find_zone(Tsy, pos)` 返回 None 自然 skip，不静默错抽
///
/// 排除 NPC（`Without<NpcMarker>`）—— P0 不在 TSY 内放 NPC（§7 未决）。
#[allow(clippy::type_complexity)]
pub fn tsy_drain_tick(
    clock: Res<CombatClock>,
    zones: Res<ZoneRegistry>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
    mut deaths: EventWriter<DeathEvent>,
    mut players: TsyDrainPlayerQuery,
    fuya_auras: Query<(&Position, &FuyaAura), With<NpcMarker>>,
) {
    for (entity, mut cultivation, pos, _presence, current_dim, searching) in &mut players {
        // 跨位面前 dim 兜底：缺 CurrentDimension 视为 TSY（presence 已经隐含玩家在内）
        let dim = current_dim.map(|c| c.0).unwrap_or(DimensionKind::Tsy);
        let Some(zone) = zones.find_zone(dim, pos.0) else {
            continue;
        };
        // plan-tsy-container-v1 §2.3 — 搜刮中真元 ×1.5；非搜刮等价旧行为。
        let base = compute_drain_per_tick(zone, &cultivation);
        let drain = base
            * compute_search_drain_multiplier(searching.is_some())
            * compute_fuya_aura_drain_multiplier(pos.get(), fuya_auras.iter());
        if drain <= 0.0 {
            continue;
        }
        let was_alive = cultivation.qi_current > 0.0;
        let before_player_qi = cultivation.qi_current.max(0.0);
        let actual_drain = drain.min(before_player_qi);
        record_tsy_drain_transfer(
            qi_account.as_deref_mut(),
            entity,
            zone.name.as_str(),
            actual_drain,
            before_player_qi,
        );
        cultivation.qi_current = (cultivation.qi_current - drain).max(0.0);
        if was_alive && cultivation.qi_current <= 0.0 {
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

    fn player(qi_max: f64) -> Cultivation {
        Cultivation {
            qi_current: qi_max,
            qi_max,
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
        assert!(drain > 0.13 && drain < 0.14, "got drain={drain}");
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
        assert!(drain > 9.1 && drain < 9.4, "got drain={drain}");
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
    fn search_drain_multiplier_is_one_when_not_searching() {
        assert_eq!(compute_search_drain_multiplier(false), 1.0);
    }

    #[test]
    fn search_drain_multiplier_is_one_point_five_when_searching() {
        assert_eq!(compute_search_drain_multiplier(true), 1.5);
    }

    #[test]
    fn search_multiplier_scales_baseline_drain_one_point_five_x() {
        // baseline 与搜刮中应严格 1.5× 关系
        let z = tsy_zone("tsy_lingxu_01_mid", -0.7);
        let p = player(100.0);
        let base = compute_drain_per_tick(&z, &p);
        let with_search = base * compute_search_drain_multiplier(true);
        assert!((with_search - base * 1.5).abs() < 1e-9);
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

    #[test]
    fn transfer_records_tsy_drain_without_losing_qi() {
        let mut account = WorldQiAccount::default();
        record_tsy_drain_transfer(
            Some(&mut account),
            Entity::from_raw(7),
            "tsy_lingxu_01_deep",
            3.0,
            10.0,
        );

        let player_account = QiAccountId::player(format!("entity:{:?}", Entity::from_raw(7)));
        let rift_account = QiAccountId::rift("tsy_lingxu_01_deep");
        assert_eq!(account.balance(&player_account), 7.0);
        assert_eq!(account.balance(&rift_account), 3.0);
        assert_eq!(account.total(), 10.0);
        assert_eq!(account.transfers().len(), 1);
        assert_eq!(
            account.transfers()[0].reason,
            QiTransferReason::RiftCollapse
        );
    }
}
