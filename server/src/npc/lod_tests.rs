#![allow(dead_code, unused_imports)]

use super::*;
use valence::prelude::{App, IntoSystemConfigs, PreUpdate};

// ECS 集成：update_npc_lod_tier_system 四档分类
// =========================================================================

#[test]
fn update_npc_lod_tier_system_assigns_tier_from_player_distance() {
    let mut app = App::new();
    app.insert_resource(NpcLodConfig::default());
    app.insert_resource(NpcLodTick(0));
    app.add_systems(
        PreUpdate,
        (tick_lod_counter, update_npc_lod_tier_system).chain(),
    );

    let npc_near = app
        .world_mut()
        .spawn((NpcMarker, Position::new([0.0, 64.0, 0.0])))
        .id();
    // dist=150 → Mid（80..256）
    let npc_mid = app
        .world_mut()
        .spawn((NpcMarker, Position::new([150.0, 64.0, 0.0])))
        .id();
    // dist=400 → Far（256..512）
    let npc_far = app
        .world_mut()
        .spawn((NpcMarker, Position::new([400.0, 64.0, 0.0])))
        .id();
    // dist=1000 → Dormant（>512）
    let npc_dormant = app
        .world_mut()
        .spawn((NpcMarker, Position::new([1000.0, 64.0, 0.0])))
        .id();
    let _ = app
        .world_mut()
        .spawn((ClientMarker, Position::new([0.0, 64.0, 0.0])))
        .id();

    // 跑 reassess_interval 次确保至少触发一轮评估
    for _ in 0..20 {
        app.update();
    }

    assert_eq!(
        app.world().get::<NpcLodTier>(npc_near).copied(),
        Some(NpcLodTier::Near),
        "期望 dist=0 → Near"
    );
    assert_eq!(
        app.world().get::<NpcLodTier>(npc_mid).copied(),
        Some(NpcLodTier::Mid),
        "期望 dist=150 → Mid（80..=256）"
    );
    assert_eq!(
        app.world().get::<NpcLodTier>(npc_far).copied(),
        Some(NpcLodTier::Far),
        "期望 dist=400 → Far（256..=512）"
    );
    assert_eq!(
        app.world().get::<NpcLodTier>(npc_dormant).copied(),
        Some(NpcLodTier::Dormant),
        "期望 dist=1000 → Dormant"
    );
}

#[test]
fn update_tier_respects_reassess_interval() {
    let mut app = App::new();
    let cfg = NpcLodConfig {
        reassess_interval: 50,
        ..Default::default()
    };
    app.insert_resource(cfg);
    app.insert_resource(NpcLodTick(0));
    app.add_systems(
        PreUpdate,
        (tick_lod_counter, update_npc_lod_tier_system).chain(),
    );

    let npc = app
        .world_mut()
        .spawn((NpcMarker, Position::new([0.0, 64.0, 0.0])))
        .id();
    let player = app
        .world_mut()
        .spawn((ClientMarker, Position::new([0.0, 64.0, 0.0])))
        .id();

    app.update();
    assert_eq!(
        app.world().get::<NpcLodTier>(npc).copied(),
        Some(NpcLodTier::Near),
        "首 tick 应先分类，避免无玩家 e2e 启动期等到 50 tick 才降载"
    );

    *app.world_mut().get_mut::<Position>(player).unwrap() = Position::new([1000.0, 64.0, 0.0]);

    for _ in 0..48 {
        app.update();
    }
    assert_eq!(
        app.world().get::<NpcLodTier>(npc).copied(),
        Some(NpcLodTier::Near),
        "期望未到 reassess_interval=50 不重算 tier，仍为 Near"
    );

    app.update();
    assert_eq!(
        app.world().get::<NpcLodTier>(npc).copied(),
        Some(NpcLodTier::Dormant),
        "期望到达 reassess_interval=50 后重算，玩家移远 → Dormant"
    );
}

#[test]
fn update_tier_classifies_new_npc_before_next_reassess() {
    let mut app = App::new();
    let cfg = NpcLodConfig {
        reassess_interval: 50,
        ..Default::default()
    };
    app.insert_resource(cfg);
    app.insert_resource(NpcLodTick(0));
    app.add_systems(
        PreUpdate,
        (tick_lod_counter, update_npc_lod_tier_system).chain(),
    );

    let existing = app
        .world_mut()
        .spawn((NpcMarker, Position::new([0.0, 64.0, 0.0])))
        .id();
    let _ = app
        .world_mut()
        .spawn((ClientMarker, Position::new([0.0, 64.0, 0.0])))
        .id();

    app.update();
    assert_eq!(
        app.world().get::<NpcLodTier>(existing).copied(),
        Some(NpcLodTier::Near)
    );

    let new_npc = app
        .world_mut()
        .spawn((NpcMarker, Position::new([1000.0, 64.0, 0.0])))
        .id();

    app.update();
    assert_eq!(
        app.world().get::<NpcLodTier>(new_npc).copied(),
        Some(NpcLodTier::Dormant),
        "期望新 NPC 不等 reassess_interval 即刻分类（Option::None 走强制评估路径）"
    );
}

// =========================================================================
// is_dormant / is_mid helpers
// =========================================================================

/// 辅助：构建含 tier 系统的基础 App（NpcLodConfig + NpcLodTick + tick+update chain）。
fn make_lod_app() -> (App, u64) {
    let mut app = App::new();
    app.insert_resource(NpcLodConfig {
        reassess_interval: 1, // 每帧重算，使切换即时可见
        ..Default::default()
    });
    app.insert_resource(NpcLodTick(0));
    app.add_systems(
        PreUpdate,
        (tick_lod_counter, update_npc_lod_tier_system).chain(),
    );
    (app, 0)
}

#[test]
fn ecs_boundary_crossing_near_to_mid_qi_conserved() {
    // ECS 级别：玩家从 near 区移到远处 → NPC tier Near→Mid，Cultivation.qi_current 不变。
    //
    // 守恒红线：update_npc_lod_tier_system 只 insert NpcLodTier component，
    // 完全不触碰 Cultivation 组件，故 qi_current 在 tier 切换时精确不变。
    let (mut app, _) = make_lod_app();

    let qi_initial = 4.2_f64;
    let npc = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([0.0, 64.0, 0.0]),
            crate::cultivation::components::Cultivation {
                qi_current: qi_initial,
                qi_max: 10.0,
                ..Default::default()
            },
        ))
        .id();
    // 玩家初始在 NPC 旁（Near 档）
    let player = app
        .world_mut()
        .spawn((ClientMarker, Position::new([10.0, 64.0, 0.0])))
        .id();

    app.update();
    assert_eq!(
        app.world().get::<NpcLodTier>(npc).copied(),
        Some(NpcLodTier::Near),
        "期望玩家在 10 格时 NPC 为 Near"
    );

    // 玩家移到 150 格 → Mid（80..256）
    *app.world_mut().get_mut::<Position>(player).unwrap() = Position::new([150.0, 64.0, 0.0]);
    app.update();
    assert_eq!(
        app.world().get::<NpcLodTier>(npc).copied(),
        Some(NpcLodTier::Mid),
        "期望玩家在 150 格时 NPC 为 Mid（80..256）"
    );

    let qi_after = app
        .world()
        .get::<crate::cultivation::components::Cultivation>(npc)
        .expect("Cultivation 组件应存在")
        .qi_current;
    assert_eq!(
        qi_after, qi_initial,
        "守恒红线：ECS Near→Mid tier 切换后 Cultivation.qi_current 精确不变，\
             期望 {qi_initial}，实际 {qi_after}，\
             因为 update_npc_lod_tier_system 只修改 NpcLodTier 组件"
    );
}

#[test]
fn ecs_boundary_crossing_mid_to_near_qi_conserved() {
    // ECS 级别：玩家从 mid 区靠近 → NPC tier Mid→Near，Cultivation.qi_current 不变。
    let (mut app, _) = make_lod_app();

    let qi_initial = 7.5_f64;
    let npc = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([0.0, 64.0, 0.0]),
            crate::cultivation::components::Cultivation {
                qi_current: qi_initial,
                qi_max: 10.0,
                ..Default::default()
            },
        ))
        .id();
    // 玩家初始在 150 格（Mid 档）
    let player = app
        .world_mut()
        .spawn((ClientMarker, Position::new([150.0, 64.0, 0.0])))
        .id();

    app.update();
    assert_eq!(
        app.world().get::<NpcLodTier>(npc).copied(),
        Some(NpcLodTier::Mid),
        "期望玩家在 150 格时 NPC 为 Mid"
    );

    // 玩家靠近到 20 格 → Near（<80）
    *app.world_mut().get_mut::<Position>(player).unwrap() = Position::new([20.0, 64.0, 0.0]);
    app.update();
    assert_eq!(
        app.world().get::<NpcLodTier>(npc).copied(),
        Some(NpcLodTier::Near),
        "期望玩家靠近到 20 格后 NPC 升频为 Near（<80）"
    );

    let qi_after = app
        .world()
        .get::<crate::cultivation::components::Cultivation>(npc)
        .expect("Cultivation 组件应存在")
        .qi_current;
    assert_eq!(
        qi_after, qi_initial,
        "守恒红线：ECS Mid→Near tier 切换后 Cultivation.qi_current 精确不变，\
             期望 {qi_initial}，实际 {qi_after}"
    );
}

#[test]
fn ecs_boundary_crossing_mid_to_far_qi_conserved() {
    // ECS 级别：玩家从 mid 区退到远处 → NPC tier Mid→Far，qi 不变。
    let (mut app, _) = make_lod_app();

    let qi_initial = 3.1_f64;
    let npc = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([0.0, 64.0, 0.0]),
            crate::cultivation::components::Cultivation {
                qi_current: qi_initial,
                qi_max: 10.0,
                ..Default::default()
            },
        ))
        .id();
    // 玩家在 150 格（Mid）
    let player = app
        .world_mut()
        .spawn((ClientMarker, Position::new([150.0, 64.0, 0.0])))
        .id();

    app.update();
    assert_eq!(
        app.world().get::<NpcLodTier>(npc).copied(),
        Some(NpcLodTier::Mid),
        "期望玩家在 150 格时 NPC 为 Mid"
    );

    // 玩家退到 400 格 → Far（256..512）
    *app.world_mut().get_mut::<Position>(player).unwrap() = Position::new([400.0, 64.0, 0.0]);
    app.update();
    assert_eq!(
        app.world().get::<NpcLodTier>(npc).copied(),
        Some(NpcLodTier::Far),
        "期望玩家退到 400 格后 NPC 降频为 Far（256..512）"
    );

    let qi_after = app
        .world()
        .get::<crate::cultivation::components::Cultivation>(npc)
        .expect("Cultivation 组件应存在")
        .qi_current;
    assert_eq!(
        qi_after, qi_initial,
        "守恒红线：ECS Mid→Far tier 切换后 Cultivation.qi_current 精确不变，\
             期望 {qi_initial}，实际 {qi_after}"
    );
}

#[test]
fn ecs_boundary_crossing_far_to_dormant_qi_conserved() {
    // ECS 级别：玩家从 far 区退到更远 → NPC tier Far→Dormant，qi 不变。
    let (mut app, _) = make_lod_app();

    let qi_initial = 6.0_f64;
    let npc = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([0.0, 64.0, 0.0]),
            crate::cultivation::components::Cultivation {
                qi_current: qi_initial,
                qi_max: 10.0,
                ..Default::default()
            },
        ))
        .id();
    // 玩家在 400 格（Far）
    let player = app
        .world_mut()
        .spawn((ClientMarker, Position::new([400.0, 64.0, 0.0])))
        .id();

    app.update();
    assert_eq!(
        app.world().get::<NpcLodTier>(npc).copied(),
        Some(NpcLodTier::Far),
        "期望玩家在 400 格时 NPC 为 Far"
    );

    // 玩家退到 1000 格 → Dormant（>512）
    *app.world_mut().get_mut::<Position>(player).unwrap() = Position::new([1000.0, 64.0, 0.0]);
    app.update();
    assert_eq!(
        app.world().get::<NpcLodTier>(npc).copied(),
        Some(NpcLodTier::Dormant),
        "期望玩家退到 1000 格后 NPC 为 Dormant（>512）"
    );

    let qi_after = app
        .world()
        .get::<crate::cultivation::components::Cultivation>(npc)
        .expect("Cultivation 组件应存在")
        .qi_current;
    assert_eq!(
        qi_after, qi_initial,
        "守恒红线：ECS Far→Dormant tier 切换后 Cultivation.qi_current 精确不变，\
             期望 {qi_initial}，实际 {qi_after}"
    );
}

#[test]
fn ecs_boundary_crossing_dormant_to_near_qi_conserved() {
    // ECS 级别：玩家从远处靠近 Dormant NPC → Near，qi 不变（E5/E6 live↔dormant 之外的直跳）。
    let (mut app, _) = make_lod_app();

    let qi_initial = 9.0_f64;
    let npc = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([0.0, 64.0, 0.0]),
            crate::cultivation::components::Cultivation {
                qi_current: qi_initial,
                qi_max: 10.0,
                ..Default::default()
            },
        ))
        .id();
    // 无玩家 → Dormant（首帧分类）
    app.update();
    assert_eq!(
        app.world().get::<NpcLodTier>(npc).copied(),
        Some(NpcLodTier::Dormant),
        "期望无玩家时 NPC 为 Dormant"
    );

    // 玩家出现在 10 格 → Near
    let _player = app
        .world_mut()
        .spawn((ClientMarker, Position::new([10.0, 64.0, 0.0])))
        .id();
    app.update();
    assert_eq!(
        app.world().get::<NpcLodTier>(npc).copied(),
        Some(NpcLodTier::Near),
        "期望玩家靠近后 NPC 从 Dormant 直升 Near（<80）"
    );

    let qi_after = app
        .world()
        .get::<crate::cultivation::components::Cultivation>(npc)
        .expect("Cultivation 组件应存在")
        .qi_current;
    assert_eq!(
        qi_after, qi_initial,
        "守恒红线：ECS Dormant→Near tier 切换后 Cultivation.qi_current 精确不变，\
             期望 {qi_initial}，实际 {qi_after}"
    );
}

#[test]
fn ecs_boundary_crossing_batch_multi_npc_all_tiers_qi_conserved() {
    // ECS 级别批量穿越：4 个 NPC 分别在各边界附近，玩家移动后 tier 全部切换，
    // 所有 NPC 的 qi_current 精确不变（零和不变量，LOD 不搬运真元）。
    //
    // 布局：
    //   npc_near  @ [0,  64, 0]，玩家初始 @ [10, 64, 0] → Near；  玩家移远后 → Dormant
    //   npc_mid   @ [150,64, 0]，同上 → Mid；  玩家移远后 → Dormant
    //   npc_far   @ [400,64, 0]，同上 → Far；  玩家移远后 → Dormant
    //   npc_dorm  @ [800,64, 0]，同上 → Dormant；  玩家移近后 → Near (if ≤80)
    let (mut app, _) = make_lod_app();

    let qis = [1.1_f64, 2.2, 3.3, 4.4];
    let positions = [
        [0.0_f64, 64.0, 0.0],
        [150.0, 64.0, 0.0],
        [400.0, 64.0, 0.0],
        [800.0, 64.0, 0.0],
    ];

    let mut npcs = Vec::new();
    for (i, (&qi, pos)) in qis.iter().zip(positions.iter()).enumerate() {
        let id = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new(*pos),
                crate::cultivation::components::Cultivation {
                    qi_current: qi,
                    qi_max: 10.0,
                    ..Default::default()
                },
            ))
            .id();
        npcs.push((id, qi, i));
    }

    // 玩家在 npc_near 旁边（10 格）
    let player = app
        .world_mut()
        .spawn((ClientMarker, Position::new([10.0, 64.0, 0.0])))
        .id();

    // 首帧分类：npc_near=Near, npc_mid=Mid, npc_far=Far, npc_dorm=Dormant
    app.update();
    let expected_initial = [
        NpcLodTier::Near,
        NpcLodTier::Mid,
        NpcLodTier::Far,
        NpcLodTier::Dormant,
    ];
    for (&(id, _, i), expected) in npcs.iter().zip(expected_initial.iter()) {
        assert_eq!(
            app.world().get::<NpcLodTier>(id).copied(),
            Some(*expected),
            "期望首帧 npc[{i}] tier={expected:?}"
        );
    }

    // 验证初始 qi 不变
    for &(id, qi_init, i) in &npcs {
        let qi = app
            .world()
            .get::<crate::cultivation::components::Cultivation>(id)
            .expect("Cultivation 存在")
            .qi_current;
        assert_eq!(
            qi, qi_init,
            "守恒：首帧分类后 npc[{i}] qi 不变，期望 {qi_init}，实际 {qi}"
        );
    }

    // 玩家移到 2000 格（全部 NPC → Dormant）
    *app.world_mut().get_mut::<Position>(player).unwrap() = Position::new([2000.0, 64.0, 0.0]);
    app.update();

    for (&(id, _, i), _) in npcs.iter().zip(expected_initial.iter()) {
        assert_eq!(
            app.world().get::<NpcLodTier>(id).copied(),
            Some(NpcLodTier::Dormant),
            "期望玩家移到 2000 格后 npc[{i}] 全部 → Dormant"
        );
    }

    // 穿越到 Dormant 后 qi 仍精确不变
    let qi_sum_after: f64 = npcs
        .iter()
        .map(|&(id, _, _)| {
            app.world()
                .get::<crate::cultivation::components::Cultivation>(id)
                .expect("Cultivation 存在")
                .qi_current
        })
        .sum();
    let qi_sum_init: f64 = qis.iter().sum();
    assert!(
        (qi_sum_after - qi_sum_init).abs() < 1e-12,
        "守恒红线：4 个 NPC 批量穿越到 Dormant 后 qi 总和不变，\
             期望 {qi_sum_init}，实际 {qi_sum_after}，\
             漂移 {}",
        (qi_sum_after - qi_sum_init).abs()
    );
}

#[test]
fn ecs_boundary_crossing_repeated_oscillation_qi_conserved() {
    // ECS 级别反复振荡：NPC 在 Near↔Mid↔Far↔Dormant 反复穿越 N 轮，
    // qi_current 始终精确不变。
    //
    // 这锁住"多轮 tier 切换不积累 qi 漂移"这条不变量——
    // 任何意外的 Cultivation 写操作（哪怕很小）都会被此测试捕捉。
    let (mut app, _) = make_lod_app();

    let qi_initial = 5.5_f64;
    let npc = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([0.0, 64.0, 0.0]),
            crate::cultivation::components::Cultivation {
                qi_current: qi_initial,
                qi_max: 10.0,
                ..Default::default()
            },
        ))
        .id();

    let player = app
        .world_mut()
        .spawn((ClientMarker, Position::new([0.0, 64.0, 0.0])))
        .id();

    // 振荡序列：近 → 中 → 远 → 休眠 → 近 → ... 共 5 轮
    let oscillation_positions: &[f64] = &[
        10.0, 150.0, 400.0, 1000.0, 10.0, 150.0, 400.0, 1000.0, 10.0, 150.0,
    ];
    for &dist in oscillation_positions {
        *app.world_mut().get_mut::<Position>(player).unwrap() = Position::new([dist, 64.0, 0.0]);
        app.update();

        let qi = app
            .world()
            .get::<crate::cultivation::components::Cultivation>(npc)
            .expect("Cultivation 存在")
            .qi_current;
        assert_eq!(
            qi, qi_initial,
            "守恒红线：NPC 在 dist={dist} 时 tier 切换后 qi_current 精确不变，\
                 期望 {qi_initial}，实际 {qi}"
        );
    }
}
