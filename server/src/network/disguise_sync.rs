//! 伪装名单同步的视野半径过滤 —— 反作弊信息面收敛
//!
//! spider/daozhan 的 `disguise_enter` 周期全量 sync 原先把**全图**伪装实体
//! id 明文广播给所有玩家——改装客户端可直读「谁是伪装的」，拟态玩法对
//! 作弊者瞬间归零（nametag 泄漏修过之后剩下的最宽一条泄漏面）。
//!
//! 本模块把名单收敛到每个 client 的视距内：
//!
//! * 客户端**可能渲染到**的实体一定在名单里（不破坏伪装换皮渲染）；
//! * 视野外的实体 id 永不出现在发给该 client 的 payload 中（全图扫描死路）。
//!
//! 过滤用 **XZ Chebyshev 距离**——实体可见性按 chunk 方形范围判定，欧氏圆
//! 会在角上漏掉可见 chunk。半径 = (client ViewDistance chunk 数 +
//! [`SYNC_MARGIN_CHUNKS`]) × 16。视距是动态的（realm_vision 按境界 ramp、
//! preview 模式 32 chunk），所以半径必须逐 client 从 `ViewDistance` 推导，
//! 不能用固定常数。
//!
//! **空表仍每周期发送**：client 端 handler 是全量替换语义（clear+addAll），
//! 空表 keepalive 负责清掉实体离开视野/暴起事件漏收后残留的 stale 条目。

/// 视距外加的保险 margin（chunk）：实体所在 chunk 边缘与 client 所在
/// chunk 中心的取整偏差最多各占 1 chunk。
pub const SYNC_MARGIN_CHUNKS: f64 = 2.0;

/// client 视距（chunk）→ 伪装名单同步半径（block，XZ Chebyshev）。
pub fn sync_radius_blocks(view_distance_chunks: u8) -> f64 {
    (f64::from(view_distance_chunks) + SYNC_MARGIN_CHUNKS) * 16.0
}

/// 返回 `candidates` 中落在 `center` 周围 XZ Chebyshev `radius` 内的
/// entity id（保持输入顺序）。Y 轴不参与——chunk 可见性只按水平距离判定，
/// 按 Y 过滤会把头顶悬崖上可见的伪装实体错误剔除。
pub fn ids_within_radius(
    candidates: &[(i32, [f64; 3])],
    center: [f64; 3],
    radius: f64,
) -> Vec<i32> {
    candidates
        .iter()
        .filter(|(_, pos)| {
            (pos[0] - center[0]).abs() <= radius && (pos[2] - center[2]).abs() <= radius
        })
        .map(|(id, _)| *id)
        .collect()
}

/// per-client 过滤入口：解构 `Position`/`ViewDistance` 后走
/// [`sync_radius_blocks`] + [`ids_within_radius`]。spider/daozhan 的
/// join/periodic 四个调用点共用，防组合逻辑漂移。
pub fn ids_visible_to_client(
    candidates: &[(i32, [f64; 3])],
    client_pos: &valence::prelude::Position,
    view_distance: &valence::prelude::ViewDistance,
) -> Vec<i32> {
    let p = client_pos.get();
    ids_within_radius(
        candidates,
        [p.x, p.y, p.z],
        sync_radius_blocks(view_distance.get()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_radius_scales_with_view_distance() {
        // valence 默认 2 chunks → (2+2)*16 = 64；vanilla 上限 32 → 544
        assert!(
            (sync_radius_blocks(2) - 64.0).abs() < 1e-9,
            "vd=2 应得 64 block（(2+2)*16），实际 {}",
            sync_radius_blocks(2)
        );
        assert!(
            (sync_radius_blocks(10) - 192.0).abs() < 1e-9,
            "vd=10 应得 192 block，实际 {}",
            sync_radius_blocks(10)
        );
        assert!(
            (sync_radius_blocks(32) - 544.0).abs() < 1e-9,
            "vd=32（preview 上限）应得 544 block，实际 {}",
            sync_radius_blocks(32)
        );
    }

    #[test]
    fn filters_out_far_entities_keeps_near() {
        let candidates = vec![
            (1, [10.0, 64.0, 10.0]),   // 近：保留
            (2, [500.0, 64.0, 0.0]),   // X 超界：剔除
            (3, [0.0, 64.0, -500.0]),  // Z 超界：剔除
            (4, [-30.0, 64.0, 30.0]),  // 近：保留
            (5, [400.0, 64.0, 400.0]), // 双轴超界：剔除
        ];
        let ids = ids_within_radius(&candidates, [0.0, 64.0, 0.0], 64.0);
        assert_eq!(
            ids,
            vec![1, 4],
            "半径 64 应只保留近处实体且维持输入顺序，全图实体不得进入名单"
        );
    }

    #[test]
    fn boundary_exactly_at_radius_is_included() {
        // 边界语义：<= radius 保留（伪装渲染宁多勿漏，多含一格无泄漏增量）
        let candidates = vec![(1, [64.0, 0.0, 0.0]), (2, [64.0 + 1e-6, 0.0, 0.0])];
        let ids = ids_within_radius(&candidates, [0.0, 0.0, 0.0], 64.0);
        assert_eq!(ids, vec![1], "恰在半径上的实体应保留（<=），刚越界的应剔除");
    }

    #[test]
    fn chebyshev_corner_is_included_where_euclidean_would_drop() {
        // 对角 (60,60)：欧氏距离 ~84.8 > 64，但 Chebyshev max(60,60)=60 <= 64。
        // chunk 可见性是方形的——欧氏过滤会在角上漏掉客户端实际能看见的实体。
        let candidates = vec![(7, [60.0, 64.0, 60.0])];
        let ids = ids_within_radius(&candidates, [0.0, 64.0, 0.0], 64.0);
        assert_eq!(
            ids,
            vec![7],
            "方形视距角上的实体必须保留，否则客户端会渲染出未伪装的本体（反向泄漏）"
        );
    }

    #[test]
    fn y_axis_is_ignored() {
        let candidates = vec![(1, [0.0, 320.0, 0.0]), (2, [0.0, -64.0, 0.0])];
        let ids = ids_within_radius(&candidates, [0.0, 64.0, 0.0], 64.0);
        assert_eq!(
            ids,
            vec![1, 2],
            "Y 轴不参与过滤——头顶/脚下垂直方向的可见实体不得被剔除"
        );
    }

    #[test]
    fn empty_candidates_yield_empty_list() {
        let ids = ids_within_radius(&[], [0.0, 0.0, 0.0], 64.0);
        assert!(
            ids.is_empty(),
            "无伪装实体时应得空名单（空表 keepalive 由调用方照常发送）"
        );
    }

    #[test]
    fn per_client_lists_differ_by_position_and_view_distance() {
        // 两个 client：A 在原点视距小，B 在远处视距大 → 名单各自独立
        let spiders = vec![(1, [50.0, 64.0, 0.0]), (2, [1000.0, 64.0, 0.0])];

        let a = ids_within_radius(&spiders, [0.0, 64.0, 0.0], sync_radius_blocks(2));
        assert_eq!(a, vec![1], "client A（vd=2, 半径64）只应看到近处蛛 1");

        let b = ids_within_radius(&spiders, [980.0, 64.0, 0.0], sync_radius_blocks(10));
        assert_eq!(b, vec![2], "client B（远处, 半径192）只应看到它附近的蛛 2");
    }

    #[test]
    fn ids_visible_to_client_wrapper_matches_manual_composition() {
        use valence::prelude::{Position, ViewDistance};

        let spiders = vec![(1, [50.0, 64.0, 0.0]), (2, [1000.0, 64.0, 0.0])];
        let pos = Position::new([0.0, 64.0, 0.0]);
        let vd = ViewDistance::new(2);

        let via_wrapper = ids_visible_to_client(&spiders, &pos, &vd);
        let manual = ids_within_radius(&spiders, [0.0, 64.0, 0.0], sync_radius_blocks(2));
        assert_eq!(
            via_wrapper, manual,
            "组合入口 ids_visible_to_client 必须与手工组合 sync_radius_blocks+\
             ids_within_radius 完全等价（四个调用点靠它防漂移）"
        );
        assert_eq!(via_wrapper, vec![1], "vd=2 半径 64 应只见近处蛛 1");
    }
}
