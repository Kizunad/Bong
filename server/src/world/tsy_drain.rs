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
) {
    let Some(account) = account else {
        return;
    };
    if amount <= 0.0 {
        return;
    }
    // 审计模式（同 BossDrain）：玩家真元已在 ECS Cultivation.qi_current 扣减，
    // ledger 只记 rift 账户增量 + audit trail，不触碰玩家账户余额。
    // 这样 summarize_world_qi 的 total_observed = player_qi(ECS) + ledger_qi(rift)，
    // 守恒不双计。
    let from = QiAccountId::player(format!("entity:{player:?}"));
    let to = QiAccountId::rift(zone_name.to_string());
    // 确保 rift 账户存在
    if !account.has_account(&to) {
        let _ = account.set_balance(to.clone(), 0.0);
    }
    // rift 账户增 amount
    let rift_balance = account.balance(&to);
    let _ = account.set_balance(to.clone(), rift_balance + amount);
    // 仅推审计轨迹，不调 transfer()（后者会检查 from 余额并拒绝）
    account.push_transfer_audit(QiTransfer {
        from,
        to,
        amount,
        reason: QiTransferReason::RiftCollapse,
    });
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
    use valence::prelude::{App, DVec3};

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
    fn transfer_records_tsy_drain_audit_only_no_player_balance() {
        // 修复后：玩家账户不写入 ledger（ECS Cultivation 是真元的唯一来源）。
        // ledger 只记 rift 账户增量 + audit trail。
        let mut account = WorldQiAccount::default();
        record_tsy_drain_transfer(
            Some(&mut account),
            Entity::from_raw(7),
            "tsy_lingxu_01_deep",
            3.0,
        );

        let player_account = QiAccountId::player(format!("entity:{:?}", Entity::from_raw(7)));
        let rift_account = QiAccountId::rift("tsy_lingxu_01_deep");

        // 玩家账户余额不应存在于 ledger（balance 返回缺省 0.0，has_account 为 false）
        assert!(
            !account.has_account(&player_account),
            "player 账户不应写入 ledger（ECS 是真元的唯一来源），has_account 应为 false"
        );
        assert_eq!(
            account.balance(&player_account),
            0.0,
            "player 不在 ledger 时 balance() 应返回缺省 0.0"
        );

        // rift 账户增 amount
        assert_eq!(
            account.balance(&rift_account),
            3.0,
            "rift 账户应增加 drain 量（3.0），期望 3.0，实际 {}",
            account.balance(&rift_account)
        );

        // ledger.total() == 仅 rift 侧（不含虚拟 player 余额）
        assert_eq!(
            account.total(),
            3.0,
            "ledger total 应等于 rift drain 量（3.0），不应虚增为 player 全池。期望 3.0，实际 {}",
            account.total()
        );

        // audit trail 仍存在一条 RiftCollapse 记录
        assert_eq!(
            account.transfers().len(),
            1,
            "应有恰好 1 条审计记录，实际 {}",
            account.transfers().len()
        );
        assert_eq!(
            account.transfers()[0].reason,
            QiTransferReason::RiftCollapse,
            "审计记录 reason 应为 RiftCollapse"
        );
    }

    #[test]
    fn transfer_no_player_ledger_entry_after_drain() {
        // 补充：drain 后 ledger 中绝对没有 player 账户条目。
        let mut account = WorldQiAccount::default();
        record_tsy_drain_transfer(Some(&mut account), Entity::from_raw(42), "tsy_zone_a", 5.0);
        let player_account = QiAccountId::player(format!("entity:{:?}", Entity::from_raw(42)));
        assert!(
            !account.has_account(&player_account),
            "drain 后 ledger 不应存在 player 账户条目"
        );
    }

    #[test]
    fn transfer_rift_balance_accumulates_across_multiple_drains() {
        // rift balance 应跨多次 drain 累积（不覆盖）。
        let mut account = WorldQiAccount::default();
        record_tsy_drain_transfer(Some(&mut account), Entity::from_raw(1), "tsy_zone_b", 2.0);
        record_tsy_drain_transfer(Some(&mut account), Entity::from_raw(2), "tsy_zone_b", 3.5);
        let rift_account = QiAccountId::rift("tsy_zone_b");
        assert_eq!(
            account.balance(&rift_account),
            5.5,
            "rift 余额应累积两次 drain，期望 5.5，实际 {}",
            account.balance(&rift_account)
        );
        assert_eq!(
            account.total(),
            5.5,
            "ledger total 应等于累计 drain 量（5.5），期望 5.5，实际 {}",
            account.total()
        );
        assert_eq!(
            account.transfers().len(),
            2,
            "应有 2 条审计记录，实际 {}",
            account.transfers().len()
        );
    }

    // ── 守恒系统级测试（system-level，驱动真实 tsy_drain_tick ECS tick） ─────────────

    /// helper：构建最小 App，并 insert 一个 TSY zone（覆盖玩家坐标 [50,64,50]）。
    fn make_drain_app_with_zone(zone_spirit_qi: f64) -> App {
        use crate::combat::events::DeathEvent;
        use valence::prelude::Update;

        let mut app = App::new();

        // 资源
        app.insert_resource(crate::combat::CombatClock { tick: 1 });
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![Zone {
                name: "tsy_zone_sys_test".to_string(),
                dimension: DimensionKind::Tsy,
                bounds: (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 100.0, 100.0)),
                spirit_qi: zone_spirit_qi,
                danger_level: 5,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
            }],
        });

        // EventWriter<DeathEvent> 必须注册
        app.add_event::<DeathEvent>();

        // 被测 system
        app.add_systems(Update, tsy_drain_tick);

        app
    }

    /// 正常 drain < pool：ECS 扣减量 == rift ledger 增量（守恒）。
    ///
    /// 使用 spirit_qi=-0.3、qi_max=100 → drain ≈ 0.043/tick（远小于 pool），
    /// 确认：delta_ecs == rift_balance（真实 system 路径）。
    #[test]
    fn system_conservation_normal_drain_ecs_delta_equals_rift_balance() {
        use crate::world::dimension::{CurrentDimension, DimensionKind};
        use crate::world::tsy::{DimensionAnchor, TsyPresence};
        use valence::prelude::{DVec3, Position};

        let mut app = make_drain_app_with_zone(-0.3);

        let qi_start = 100.0_f64;
        let player = app
            .world_mut()
            .spawn((
                Cultivation {
                    qi_current: qi_start,
                    qi_max: qi_start,
                    ..Default::default()
                },
                Position::new([50.0, 64.0, 50.0]),
                TsyPresence {
                    family_id: "tsy_zone_sys_test".to_string(),
                    entered_at_tick: 0,
                    entry_inventory_snapshot: Vec::new(),
                    return_to: DimensionAnchor {
                        dimension: DimensionKind::Overworld,
                        pos: DVec3::ZERO,
                    },
                },
                CurrentDimension(DimensionKind::Tsy),
            ))
            .id();

        app.update();

        let qi_after = app
            .world()
            .get::<Cultivation>(player)
            .expect("Cultivation should exist")
            .qi_current;

        let delta_ecs = qi_start - qi_after;

        let rift_account = QiAccountId::rift("tsy_zone_sys_test");
        let rift_balance = app
            .world()
            .resource::<WorldQiAccount>()
            .balance(&rift_account);

        assert!(
            delta_ecs > 0.0,
            "ECS 应扣减真元（spirit_qi=-0.3, pool=100），实际 delta={}",
            delta_ecs
        );
        assert!(
            (delta_ecs - rift_balance).abs() < 1e-9,
            "守恒失败：ECS 扣减量({delta_ecs}) != rift ledger 增量({rift_balance})。\
             若 record_tsy_drain_transfer 传入的是未 clamp 的 drain 而 ECS 用的是 actual_drain，\
             两者将不等（回归防御）"
        );
    }

    /// overdraft 路径：drain > before_player_qi（小池 / 大灵压）。
    ///
    /// 构造 qi_current=2.0、spirit_qi=-1.1（大强度，drain >> 2.0）。
    /// 断言：
    ///   - ECS qi_current 扣到 0（不能为负）
    ///   - rift_balance == before_player_qi（== ECS 实际损失量），而非 unclamped drain
    ///   - 两者严格相等（守恒不变式）
    #[test]
    fn system_conservation_overdraft_ecs_clamps_to_zero_rift_equals_actual_loss() {
        use crate::world::dimension::{CurrentDimension, DimensionKind};
        use crate::world::tsy::{DimensionAnchor, TsyPresence};
        use valence::prelude::{DVec3, Position};

        // spirit_qi=-1.1, qi_max=500 → drain 约 9.2/tick >> qi_current=2.0
        let mut app = make_drain_app_with_zone(-1.1);

        // 使用大 qi_max 以让 drain 放大（非线性 pool 比），但 qi_current 只给 2.0
        let qi_start = 2.0_f64;
        let qi_max = 500.0_f64;
        let player = app
            .world_mut()
            .spawn((
                Cultivation {
                    qi_current: qi_start,
                    qi_max,
                    ..Default::default()
                },
                Position::new([50.0, 64.0, 50.0]),
                TsyPresence {
                    family_id: "tsy_zone_sys_test".to_string(),
                    entered_at_tick: 0,
                    entry_inventory_snapshot: Vec::new(),
                    return_to: DimensionAnchor {
                        dimension: DimensionKind::Overworld,
                        pos: DVec3::ZERO,
                    },
                },
                CurrentDimension(DimensionKind::Tsy),
            ))
            .id();

        app.update();

        let qi_after = app
            .world()
            .get::<Cultivation>(player)
            .expect("Cultivation should exist")
            .qi_current;

        let delta_ecs = qi_start - qi_after; // 应 == qi_start（扣到 0）

        let rift_account = QiAccountId::rift("tsy_zone_sys_test");
        let rift_balance = app
            .world()
            .resource::<WorldQiAccount>()
            .balance(&rift_account);

        assert_eq!(
            qi_after, 0.0,
            "overdraft：ECS qi_current 应扣到 0（实际 {}）。\
             若此 assert 失败说明 .max(0.0) clamp 未生效",
            qi_after
        );
        assert!(
            rift_balance <= qi_start,
            "rift_balance({rift_balance}) 不应超过 before_player_qi({qi_start})：\
             ledger 记录的应是 actual_drain（min(drain, pool)），而非 unclamped drain"
        );
        assert!(
            (delta_ecs - rift_balance).abs() < 1e-9,
            "overdraft 守恒失败：ECS 扣减量({delta_ecs}) != rift ledger 增量({rift_balance})。\
             若 record_tsy_drain_transfer 传入 unclamped drain 而 ECS 用 .max(0) clamp，\
             rift_balance 将 > delta_ecs，等式不成立（此为原 bug 类回归的直接断言）"
        );
    }

    #[test]
    fn transfer_zero_amount_is_noop() {
        // amount=0 时不应写 rift 账户、不应留审计记录。
        let mut account = WorldQiAccount::default();
        record_tsy_drain_transfer(Some(&mut account), Entity::from_raw(9), "tsy_zone_d", 0.0);
        let rift_account = QiAccountId::rift("tsy_zone_d");
        assert_eq!(
            account.balance(&rift_account),
            0.0,
            "amount=0 不应写 rift 账户"
        );
        assert_eq!(account.transfers().len(), 0, "amount=0 不应留审计记录");
    }

    #[test]
    fn transfer_none_account_is_noop() {
        // account=None 时不应 panic，函数静默返回。
        record_tsy_drain_transfer(None, Entity::from_raw(11), "tsy_zone_e", 1.5);
        // 能走到这里不 panic 即通过
    }
}
