//! Worldgen snapshot preview support — server-side teleport for headless screenshot
//! harness（plan-worldgen-snapshot-v1 §2.4）。
//!
//! 用途：让 client preview harness 在 multi-player 模式下做远距离 setPos 不被
//! anti-cheat reject —— client 直接 setPos ±400 blocks 必然被 server force-sync 回
//! 原位。改用 server-side authoritative teleport：client 发 `!preview-tp <x> <y>
//! <z> <yaw> <pitch>` chat 命令，chat_collector 解析后 emit
//! [`PreviewTeleportRequested`] event，本 module 的 system 消费 event 改写
//! Position + Look + HeadYaw，server 主动下发 PlayerPosLook 包同步 client。
//!
//! 仅在 `BONG_PREVIEW_MODE=1` env 下激活——避免生产环境暴露 dev backdoor。

use valence::entity::{HeadYaw, Look};
use valence::prelude::{
    bevy_ecs, Added, App, Client, Entity, Event, EventReader, IntoSystemConfigs, Position, Query,
    Update, ViewDistance, With,
};

pub mod decorations;

/// Client → Server 远距离 teleport 请求。由 chat_collector 解析 `!preview-tp` 命令
/// 后 emit；preview module 的 system 消费。
///
/// 字段语义遵循 MC 1.20.1 vanilla：
///   - `pos[1]` 是 feet Y（block 顶面）
///   - `yaw` 0 朝南，正向 +Z；90 朝西，正向 -X；180 朝北，正向 -Z；-90 朝东，正向 +X
///   - `pitch` -90 仰头朝天；+90 低头朝地；0 水平
#[derive(Event, Debug, Clone, Copy)]
pub struct PreviewTeleportRequested {
    pub player: Entity,
    pub pos: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
}

/// 是否激活 preview 模式（env 驱动）。生产环境留 false 不暴露 `!preview-tp`。
pub fn preview_mode_enabled() -> bool {
    std::env::var("BONG_PREVIEW_MODE").as_deref() == Ok("1")
}

/// fix-spec-1901-v2 §4.2 — preview teleport 直接写 `Position`，纳入统一移动
/// commit set。生产 `register()` 与回归测试共用此注册路径：测试不得在本地重建
/// set 会员，否则生产注册丢失 membership 时测试仍会绿，无法发现调度契约退化。
pub(crate) fn register_preview_teleport_commit_system(app: &mut App) {
    app.add_systems(
        Update,
        handle_preview_teleport
            .in_set(crate::world::movement_commit::AuthoritativePositionCommitSet),
    );
}

pub fn register(app: &mut App) {
    app.add_event::<PreviewTeleportRequested>();
    if preview_mode_enabled() {
        register_preview_teleport_commit_system(app);
        app.add_systems(
            Update,
            (
                boost_view_distance_for_preview,
                decorations::spawn_decorations_once_system,
            ),
        );
        tracing::info!(
            "[bong][preview] BONG_PREVIEW_MODE=1 — !preview-tp + ViewDistance(32) + \
             decorations 已激活"
        );
    }
}

/// 把新加入 client 的 ViewDistance 从 valence default 2 chunks 提到 vanilla 上限
/// 32 chunks ≈ 512 blocks，让 preview client 截图能看到远处地形（普通玩家不需要
/// 这么远，所以仅 preview mode 激活）。
fn boost_view_distance_for_preview(mut clients: Query<&mut ViewDistance, Added<Client>>) {
    for mut view_distance in &mut clients {
        view_distance.set(32);
        tracing::info!(
            "[bong][preview] boosted ViewDistance → 32 chunks (was {})",
            view_distance.get()
        );
    }
}

fn handle_preview_teleport(
    mut events: EventReader<PreviewTeleportRequested>,
    mut clients: Query<(&mut Position, &mut Look, &mut HeadYaw), With<Client>>,
) {
    for ev in events.read() {
        let Ok((mut position, mut look, mut head_yaw)) = clients.get_mut(ev.player) else {
            tracing::warn!(
                "[bong][preview] PreviewTeleportRequested 找不到 player entity {:?}",
                ev.player
            );
            continue;
        };
        position.set(ev.pos);
        look.yaw = ev.yaw;
        look.pitch = ev.pitch;
        head_yaw.0 = ev.yaw;
        tracing::info!(
            "[bong][preview] tp player={:?} pos=({:.1}, {:.1}, {:.1}) yaw={:.1} pitch={:.1}",
            ev.player,
            ev.pos[0],
            ev.pos[1],
            ev.pos[2],
            ev.yaw,
            ev.pitch
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::entity::{HeadYaw, Look};
    use valence::prelude::App;

    /// BONG_PREVIEW_MODE 的加锁 scoped guard：持锁（MutexGuard 存活到 Drop）到测试
    /// 结束，进程级 env 在整个测试期间不被其他 preview 测试改动（`cargo test` 默认
    /// 多线程并发跑同进程内测试，直接 set_var 会互相踩脚——dormant/mod.rs 同款模式）。
    struct ScopedPreviewMode {
        _guard: std::sync::MutexGuard<'static, ()>,
        previous: Option<std::ffi::OsString>,
    }

    static PREVIEW_MODE_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    impl ScopedPreviewMode {
        fn set() -> Self {
            let guard = PREVIEW_MODE_ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
            let previous = std::env::var_os("BONG_PREVIEW_MODE");
            // SAFETY: 锁内独占，与 preview_mode_enabled_reads_env 互斥
            unsafe {
                std::env::set_var("BONG_PREVIEW_MODE", "1");
            }
            Self {
                _guard: guard,
                previous,
            }
        }
    }

    impl Drop for ScopedPreviewMode {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(previous) => {
                    // SAFETY: 锁仍在 guard 内持有
                    unsafe {
                        std::env::set_var("BONG_PREVIEW_MODE", previous);
                    }
                }
                None => {
                    // SAFETY: 锁仍在 guard 内持有
                    unsafe {
                        std::env::remove_var("BONG_PREVIEW_MODE");
                    }
                }
            }
        }
    }

    fn register_real_handler(app: &mut App) {
        // 走生产注册入口 preview::register 的 BONG_PREVIEW_MODE=1 分支（central
        // review 1984-31447628937 finding [2]）：handler 的 set 会员由生产 register
        // 提供，测试不在本地重建——直接调 register_preview_teleport_commit_system 会让
        // 「生产 register 丢 enabled 分支注册」假绿（删掉 enabled 分支里的 helper
        // 调用后测试仍因手动注入而通过）。env 只在注册时刻读取，guard 在函数返回时
        // 释放已足够；锁保证与 preview_mode_enabled_reads_env 的写不交错。
        let _preview_mode = ScopedPreviewMode::set();
        crate::preview::register(app);
    }

    #[test]
    fn preview_teleport_event_updates_components() {
        let mut app = App::new();
        app.add_event::<PreviewTeleportRequested>();
        register_real_handler(&mut app);

        // real handler 的 query 带 `With<Client>`——必须 spawn 真 valence Client
        // （create_mock_client），自定义 stub 会被 query 跳过。
        let (mut client_bundle, _helper) = valence::testing::create_mock_client("PreviewTp");
        client_bundle.player.position = Position::new([0.0, 0.0, 0.0]);
        client_bundle.player.look = Look::new(0.0, 0.0);
        client_bundle.player.head_yaw = HeadYaw(0.0);
        let entity = app.world_mut().spawn(client_bundle).id();

        app.world_mut().send_event(PreviewTeleportRequested {
            player: entity,
            pos: [123.0, 200.0, -456.0],
            yaw: 45.0,
            pitch: 90.0,
        });

        app.update();

        let world = app.world();
        let position = world.get::<Position>(entity).unwrap();
        let look = world.get::<Look>(entity).unwrap();
        let head_yaw = world.get::<HeadYaw>(entity).unwrap();

        assert_eq!(
            position.get(),
            valence::prelude::DVec3::new(123.0, 200.0, -456.0),
            "Position 必须被 event 改写为 (123, 200, -456)"
        );
        assert!(
            (look.yaw - 45.0).abs() < f32::EPSILON,
            "Look.yaw 必须被改写为 45.0，实际 {}",
            look.yaw
        );
        assert!(
            (look.pitch - 90.0).abs() < f32::EPSILON,
            "Look.pitch 必须被改写为 90.0（朝地），实际 {}",
            look.pitch
        );
        assert!(
            (head_yaw.0 - 45.0).abs() < f32::EPSILON,
            "HeadYaw.0 必须跟 Look.yaw 同步为 45.0，实际 {}",
            head_yaw.0
        );
    }

    #[test]
    fn preview_teleport_event_unknown_player_no_op() {
        let mut app = App::new();
        app.add_event::<PreviewTeleportRequested>();
        register_real_handler(&mut app);

        let (mut client_bundle, _helper) = valence::testing::create_mock_client("PreviewTp");
        client_bundle.player.position = Position::new([10.0, 20.0, 30.0]);
        client_bundle.player.look = Look::new(11.0, 22.0);
        client_bundle.player.head_yaw = HeadYaw(11.0);
        let real = app.world_mut().spawn(client_bundle).id();

        // 故意发给一个 spawn 但不带 Client 的 entity（query 查不到）
        let dangling = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(PreviewTeleportRequested {
            player: dangling,
            pos: [999.0, 999.0, 999.0],
            yaw: 99.0,
            pitch: 99.0,
        });

        app.update();

        let world = app.world();
        let position = world.get::<Position>(real).unwrap();
        let look = world.get::<Look>(real).unwrap();
        assert_eq!(
            position.get(),
            valence::prelude::DVec3::new(10.0, 20.0, 30.0),
            "未匹配的 event 不应影响其他 player"
        );
        assert!(
            (look.yaw - 11.0).abs() < f32::EPSILON,
            "未匹配的 event 不应改 Look.yaw"
        );
    }

    #[test]
    fn preview_teleport_commits_position_before_lingtian_post_transfer_validation() {
        // fix-spec-1901-v2 #18：生产 `handle_preview_teleport` 在
        // AuthoritativePositionCommitSet 内写 Position，灵田 post-transfer validator
        // 排在 set 之后。本测试 validator 先注册、real handler 后注册（无 .after 边），
        // handler 的 set 会员由生产注册入口 preview::register 的
        // BONG_PREVIEW_MODE=1 分支提供，不在本地重建——删 enabled 分支里的 helper
        // 调用后 handler 落后 validator → 拒绝 → 红（central review
        // 1984-31447628937 finding [2]）。
        use crate::lingtian::events::{
            StartDrainQiRequest, StartHarvestRequest, StartPlantingRequest, StartRenewRequest,
            StartReplenishRequest, StartTillRequest,
        };
        use crate::lingtian::requests::{PendingLingtianRequest, PendingLingtianRequests};
        use crate::lingtian::session::SessionMode;
        use crate::lingtian::systems::validate_and_dispatch_lingtian_requests;
        use crate::world::dimension::DimensionKind;
        use crate::world::movement_commit::AuthoritativePositionCommitSet;
        use valence::prelude::{BlockPos, Events};

        let mut app = App::new();
        app.init_resource::<PendingLingtianRequests>()
            .add_event::<PreviewTeleportRequested>()
            .add_event::<StartTillRequest>()
            .add_event::<StartRenewRequest>()
            .add_event::<StartPlantingRequest>()
            .add_event::<StartHarvestRequest>()
            .add_event::<StartReplenishRequest>()
            .add_event::<StartDrainQiRequest>();
        // 走生产注册入口 preview::register 的 BONG_PREVIEW_MODE=1 分支（central
        // review 1984-31447628937 finding [2]）：handler 的 set 会员由生产 register
        // 提供，测试不在本地重建——直接调 register_preview_teleport_commit_system 会让
        // 「生产 register 丢 enabled 分支注册」假绿。guard 持锁到测试结束，env 不被
        // 并行测试改写。
        let _preview_mode = ScopedPreviewMode::set();
        crate::preview::register(&mut app);
        app.add_systems(
            Update,
            validate_and_dispatch_lingtian_requests.after(AuthoritativePositionCommitSet),
        );

        // Mock 客户端起点在远处（1000, 64.5, 1000）——若 handler 不在 commit set 内，
        // validator 会读到这个远点并拒绝请求。CurrentDimension 是 bong 自定义组件，
        // create_mock_client 不带，必须手动补（validator 的维度门禁要求）。
        let (mut client_bundle, _helper) = valence::testing::create_mock_client("PreviewTp");
        client_bundle.player.position = Position::new([1000.0, 64.5, 1000.0]);
        let player = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(player)
            .insert((crate::world::dimension::CurrentDimension(
                DimensionKind::Overworld,
            ),));

        app.world_mut()
            .resource_mut::<PendingLingtianRequests>()
            .push(PendingLingtianRequest::Till {
                actor: player,
                pos: BlockPos::new(0, 64, 0),
                hoe_instance_id: 7,
                mode: SessionMode::Manual,
            });

        // teleport 到目标旁 (2.5, 64.5, 0.5)——距 (0,64,0) 中心 2.0 < 4.5，应放行。
        app.world_mut().send_event(PreviewTeleportRequested {
            player,
            pos: [2.5, 64.5, 0.5],
            yaw: 0.0,
            pitch: 0.0,
        });

        app.update();

        assert_eq!(
            app.world().get::<Position>(player).unwrap().get(),
            valence::prelude::DVec3::new(2.5, 64.5, 0.5),
            "preview teleport 必须先于 post-transfer 验证提交新位置"
        );
        let start_events = app.world().resource::<Events<StartTillRequest>>();
        assert_eq!(
            start_events.get_reader().read(start_events).count(),
            1,
            "teleport 提交的位置必须被本 tick validator 读到（期望 1 条 StartTillRequest）；\
             handle_preview_teleport 若不在 AuthoritativePositionCommitSet 内即读到远处位置拒绝"
        );
    }

    #[test]
    fn preview_mode_enabled_reads_env() {
        // 与 ScopedPreviewMode 同一把锁：进程级 env 的写必须串行，否则并行测试交错
        // 会让本测试的 set("0")→assert false 读到别的测试刚写入的 "1"（或反之）。
        let _guard = PREVIEW_MODE_ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        // 锁内独占，set/unset 不与其他 preview 测试交错
        // SAFETY: 锁内独占
        unsafe {
            std::env::remove_var("BONG_PREVIEW_MODE");
        }
        assert!(
            !preview_mode_enabled(),
            "未设 BONG_PREVIEW_MODE 时应返回 false"
        );
        // SAFETY: 锁内独占
        unsafe {
            std::env::set_var("BONG_PREVIEW_MODE", "1");
        }
        assert!(preview_mode_enabled(), "BONG_PREVIEW_MODE=1 时应返回 true");
        // SAFETY: 锁内独占
        unsafe {
            std::env::set_var("BONG_PREVIEW_MODE", "0");
        }
        assert!(
            !preview_mode_enabled(),
            "BONG_PREVIEW_MODE=0 时应返回 false（仅 \"1\" 激活）"
        );
        // SAFETY: 锁内独占
        unsafe {
            std::env::remove_var("BONG_PREVIEW_MODE");
        }
    }
}
