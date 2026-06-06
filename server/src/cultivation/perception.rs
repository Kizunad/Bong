//! 神视感知 — 真元色被动范围感知（plan-color-v1 P4）。
//!
//! 通灵+ 境界玩家对范围内其他玩家的 QiColor 进行被动感知扫描，
//! 每 `PASSIVE_COLOR_SCAN_INTERVAL_TICKS` 触发一次，
//! 向 `QiColorInspectRequest` 事件队列推送 inspect 请求，
//! 由已有的 `emit_qi_color_observed_payloads` 系统负责 realm_diff 过滤和发送。
//!
//! 两境界范围：
//!   * 通灵（Spirit）→ 32 格
//!   * 化虚（Void）→ 128 格
//!   * 其他境界 → None（不感知）

use valence::prelude::{Entity, EventWriter, Position, Query, Res, With};

use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::tick::CultivationClock;
use crate::network::qi_color_observed_emit::QiColorInspectRequest;

/// 被动色感知扫描间隔：每 60 ticks（= 3 秒 @20TPS）触发一次。
pub const PASSIVE_COLOR_SCAN_INTERVAL_TICKS: u64 = 60;

/// 按境界返回被动神视感知的感知半径（方块数）。
///
/// - `None` 表示该境界不具备被动真元色感知能力
/// - 通灵=32，化虚=128；固元及以下返回 `None`
pub fn remote_color_sense_range(realm: Realm) -> Option<u32> {
    match realm {
        Realm::Awaken | Realm::Induce | Realm::Condense | Realm::Solidify => None,
        Realm::Spirit => Some(32),
        Realm::Void => Some(128),
    }
}

/// 被动神视感知系统 — 每 `PASSIVE_COLOR_SCAN_INTERVAL_TICKS` ticks 对所有 Spirit+
/// 玩家，向其感知半径内的所有其他玩家发送 `QiColorInspectRequest`。
///
/// 已有的 `emit_qi_color_observed_payloads` 系统会消费这些 request 并进行
/// realm_diff 过滤（realm_diff ≤ 0 时不发包），因此本 system 无需额外过滤。
pub fn passive_qi_color_scan_system(
    clock: Res<CultivationClock>,
    mut qi_inspect_tx: EventWriter<QiColorInspectRequest>,
    observer_query: Query<(Entity, &Position, &Cultivation), With<Cultivation>>,
    target_query: Query<(Entity, &Position, &Cultivation), With<Cultivation>>,
) {
    let now_tick = clock.tick;

    // 按间隔节流：不是整倍数 tick 跳过
    if now_tick % PASSIVE_COLOR_SCAN_INTERVAL_TICKS != 0 {
        return;
    }

    for (observer, observer_pos, observer_cultivation) in &observer_query {
        let Some(radius) = remote_color_sense_range(observer_cultivation.realm) else {
            continue;
        };
        let radius_sq = f64::from(radius) * f64::from(radius);
        let obs_pos = observer_pos.0;

        for (target, target_pos, _target_cultivation) in &target_query {
            if target == observer {
                continue;
            }
            let dx = obs_pos.x - target_pos.0.x;
            let dy = obs_pos.y - target_pos.0.y;
            let dz = obs_pos.z - target_pos.0.z;
            let dist_sq = dx * dx + dy * dy + dz * dz;
            if dist_sq <= radius_sq {
                qi_inspect_tx.send(QiColorInspectRequest {
                    observer,
                    observed: target,
                    requested_at_tick: now_tick,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::QiColor;
    use crate::network::qi_color_observed_emit::QiColorInspectRequest;
    use valence::math::DVec3;
    use valence::prelude::{App, Events, Position, Update};

    fn make_app() -> App {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 0 });
        app.add_event::<QiColorInspectRequest>();
        app.add_systems(Update, passive_qi_color_scan_system);
        app
    }

    fn spawn_player(app: &mut App, realm: Realm, pos: [f64; 3]) -> Entity {
        app.world_mut()
            .spawn((
                Position(DVec3::new(pos[0], pos[1], pos[2])),
                Cultivation {
                    realm,
                    ..Default::default()
                },
                QiColor::default(),
            ))
            .id()
    }

    fn drain_inspect_requests(app: &mut App) -> Vec<QiColorInspectRequest> {
        app.world_mut()
            .resource_mut::<Events<QiColorInspectRequest>>()
            .drain()
            .collect()
    }

    fn set_tick(app: &mut App, tick: u64) {
        app.world_mut().resource_mut::<CultivationClock>().tick = tick;
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // remote_color_sense_range 纯函数单测
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    #[test]
    fn awaken_returns_none() {
        assert_eq!(
            remote_color_sense_range(Realm::Awaken),
            None,
            "期望 Awaken 无被动感知（None），因为醒灵未达神视阈值"
        );
    }

    #[test]
    fn induce_returns_none() {
        assert_eq!(
            remote_color_sense_range(Realm::Induce),
            None,
            "期望 Induce 无被动感知（None），引气境未达神视阈值"
        );
    }

    #[test]
    fn condense_returns_none() {
        assert_eq!(
            remote_color_sense_range(Realm::Condense),
            None,
            "期望 Condense 无被动感知（None），凝脉境未达神视阈值"
        );
    }

    #[test]
    fn solidify_returns_none() {
        assert_eq!(
            remote_color_sense_range(Realm::Solidify),
            None,
            "期望 Solidify 无被动感知（None），固元境未达神视阈值"
        );
    }

    #[test]
    fn spirit_returns_32() {
        assert_eq!(
            remote_color_sense_range(Realm::Spirit),
            Some(32),
            "期望 Spirit 感知半径=32 格（plan §5 原文），实际值不符"
        );
    }

    #[test]
    fn void_returns_128() {
        assert_eq!(
            remote_color_sense_range(Realm::Void),
            Some(128),
            "期望 Void 感知半径=128 格（plan §5 原文），实际值不符"
        );
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // passive_qi_color_scan_system 集成测试
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    #[test]
    fn solidify_observer_does_not_trigger_scan() {
        let mut app = make_app();
        set_tick(&mut app, PASSIVE_COLOR_SCAN_INTERVAL_TICKS);
        let _observer = spawn_player(&mut app, Realm::Solidify, [0.0, 64.0, 0.0]);
        let _target = spawn_player(&mut app, Realm::Induce, [10.0, 64.0, 0.0]);
        app.update();

        let requests = drain_inspect_requests(&mut app);
        assert!(
            requests.is_empty(),
            "期望 Solidify 境界不触发被动扫描，实际发出了 {} 条 request",
            requests.len()
        );
    }

    #[test]
    fn spirit_observer_within_32_blocks_triggers_inspect() {
        let mut app = make_app();
        set_tick(&mut app, PASSIVE_COLOR_SCAN_INTERVAL_TICKS);
        let observer = spawn_player(&mut app, Realm::Spirit, [0.0, 64.0, 0.0]);
        let target = spawn_player(&mut app, Realm::Induce, [20.0, 64.0, 0.0]); // dist=20 < 32
        app.update();

        let requests = drain_inspect_requests(&mut app);
        assert_eq!(
            requests.len(),
            1,
            "期望 Spirit 境界玩家对 32 格内目标发出 1 条 QiColorInspectRequest，实际数量不符"
        );
        assert_eq!(
            requests[0].observer, observer,
            "observer 字段应为 Spirit 玩家"
        );
        assert_eq!(requests[0].observed, target, "observed 字段应为目标玩家");
        assert_eq!(
            requests[0].requested_at_tick, PASSIVE_COLOR_SCAN_INTERVAL_TICKS,
            "requested_at_tick 应与当前 clock.tick 一致"
        );
    }

    #[test]
    fn spirit_observer_beyond_32_blocks_does_not_trigger() {
        let mut app = make_app();
        set_tick(&mut app, PASSIVE_COLOR_SCAN_INTERVAL_TICKS);
        let _observer = spawn_player(&mut app, Realm::Spirit, [0.0, 64.0, 0.0]);
        let _target = spawn_player(&mut app, Realm::Induce, [33.0, 64.0, 0.0]); // dist=33 > 32
        app.update();

        let requests = drain_inspect_requests(&mut app);
        assert!(
            requests.is_empty(),
            "期望 Spirit 境界玩家对 33 格处目标不触发扫描（超出 32 格范围），实际发出 {} 条",
            requests.len()
        );
    }

    #[test]
    fn void_observer_within_128_blocks_triggers_inspect() {
        let mut app = make_app();
        set_tick(&mut app, PASSIVE_COLOR_SCAN_INTERVAL_TICKS);
        let observer = spawn_player(&mut app, Realm::Void, [0.0, 64.0, 0.0]);
        let target = spawn_player(&mut app, Realm::Solidify, [100.0, 64.0, 0.0]); // dist=100 < 128
        app.update();

        let requests = drain_inspect_requests(&mut app);
        assert_eq!(
            requests.len(),
            1,
            "期望 Void 128 格内目标触发 1 条扫描，实际数量不符"
        );
        assert_eq!(requests[0].observer, observer);
        assert_eq!(requests[0].observed, target);
    }

    #[test]
    fn void_observer_beyond_128_blocks_does_not_trigger() {
        let mut app = make_app();
        set_tick(&mut app, PASSIVE_COLOR_SCAN_INTERVAL_TICKS);
        let _observer = spawn_player(&mut app, Realm::Void, [0.0, 64.0, 0.0]);
        let _target = spawn_player(&mut app, Realm::Solidify, [129.0, 64.0, 0.0]); // dist=129 > 128
        app.update();

        let requests = drain_inspect_requests(&mut app);
        assert!(
            requests.is_empty(),
            "期望 Void 境界玩家对 129 格外目标不触发扫描，实际发出 {} 条",
            requests.len()
        );
    }

    #[test]
    fn scan_does_not_emit_self_inspect() {
        // 单个 Spirit 玩家不应向自己发出 inspect
        let mut app = make_app();
        set_tick(&mut app, PASSIVE_COLOR_SCAN_INTERVAL_TICKS);
        let _player = spawn_player(&mut app, Realm::Spirit, [0.0, 64.0, 0.0]);
        app.update();

        let requests = drain_inspect_requests(&mut app);
        assert!(
            requests.is_empty(),
            "期望单个 Spirit 玩家不向自己发出 inspect，实际发出 {} 条",
            requests.len()
        );
    }

    #[test]
    fn scan_does_not_run_on_non_interval_tick() {
        // tick=1 不是 PASSIVE_COLOR_SCAN_INTERVAL_TICKS 的整数倍，不应触发扫描
        let mut app = make_app();
        set_tick(&mut app, 1);
        let _observer = spawn_player(&mut app, Realm::Spirit, [0.0, 64.0, 0.0]);
        let _target = spawn_player(&mut app, Realm::Induce, [10.0, 64.0, 0.0]);
        app.update();

        let requests = drain_inspect_requests(&mut app);
        assert!(
            requests.is_empty(),
            "期望非整倍 tick 不触发扫描，但发出了 {} 条（节流机制故障）",
            requests.len()
        );
    }

    #[test]
    fn scan_runs_exactly_on_interval_ticks() {
        // 验证 tick=0 和 tick=60 都触发，tick=30 不触发
        let mut app = make_app();
        let _observer = spawn_player(&mut app, Realm::Spirit, [0.0, 64.0, 0.0]);
        let _target = spawn_player(&mut app, Realm::Induce, [10.0, 64.0, 0.0]);

        // tick=0 应触发
        set_tick(&mut app, 0);
        app.update();
        let r0 = drain_inspect_requests(&mut app);
        assert_eq!(r0.len(), 1, "tick=0 应触发一次扫描");

        // tick=30 不是整倍数，不触发
        set_tick(&mut app, 30);
        app.update();
        let r30 = drain_inspect_requests(&mut app);
        assert!(r30.is_empty(), "tick=30 不是整倍数，不应触发扫描");

        // tick=60 是整倍数，应触发
        set_tick(&mut app, 60);
        app.update();
        let r60 = drain_inspect_requests(&mut app);
        assert_eq!(r60.len(), 1, "tick=60 应触发一次扫描");
    }

    #[test]
    fn spirit_at_exactly_32_blocks_is_within_range() {
        // 边界：distance=32.0，radius=32，32^2 = 1024 ≤ 1024 → 应触发
        let mut app = make_app();
        set_tick(&mut app, PASSIVE_COLOR_SCAN_INTERVAL_TICKS);
        let _observer = spawn_player(&mut app, Realm::Spirit, [0.0, 64.0, 0.0]);
        let _target = spawn_player(&mut app, Realm::Awaken, [32.0, 64.0, 0.0]); // dist=32.0 exactly
        app.update();

        let requests = drain_inspect_requests(&mut app);
        assert_eq!(
            requests.len(),
            1,
            "期望恰好距离=32 格时仍在范围内（<= radius^2），实际数量不符"
        );
    }

    #[test]
    fn multiple_targets_all_within_range_all_get_scanned() {
        let mut app = make_app();
        set_tick(&mut app, PASSIVE_COLOR_SCAN_INTERVAL_TICKS);
        let _observer = spawn_player(&mut app, Realm::Void, [0.0, 64.0, 0.0]);
        let _t1 = spawn_player(&mut app, Realm::Awaken, [10.0, 64.0, 0.0]);
        let _t2 = spawn_player(&mut app, Realm::Induce, [30.0, 64.0, 0.0]);
        let _t3 = spawn_player(&mut app, Realm::Condense, [100.0, 64.0, 0.0]);
        app.update();

        let requests = drain_inspect_requests(&mut app);
        assert_eq!(
            requests.len(),
            3,
            "期望 Void 范围内 3 个目标各产生 1 条 inspect request"
        );
    }
}
