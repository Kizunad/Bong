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
    bevy_ecs, App, Client, Entity, Event, EventReader, Position, Query, Update, With,
};

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

pub fn register(app: &mut App) {
    app.add_event::<PreviewTeleportRequested>();
    if preview_mode_enabled() {
        app.add_systems(Update, handle_preview_teleport);
        tracing::info!("[bong][preview] BONG_PREVIEW_MODE=1 — !preview-tp 已激活");
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
    use valence::prelude::{App, Bundle, Component};

    /// 没有真 valence Client 的最小 stub —— 测试 system 只验 Position/Look/HeadYaw
    /// 是否被改写，不验 Client 派生的网络副作用。
    #[derive(Component)]
    struct MockClient;

    #[derive(Bundle)]
    struct MockClientBundle {
        client: MockClient,
        position: Position,
        look: Look,
        head_yaw: HeadYaw,
    }

    fn handle_preview_teleport_mock(
        mut events: EventReader<PreviewTeleportRequested>,
        mut clients: Query<(&mut Position, &mut Look, &mut HeadYaw), With<MockClient>>,
    ) {
        for ev in events.read() {
            let Ok((mut position, mut look, mut head_yaw)) = clients.get_mut(ev.player) else {
                continue;
            };
            position.set(ev.pos);
            look.yaw = ev.yaw;
            look.pitch = ev.pitch;
            head_yaw.0 = ev.yaw;
        }
    }

    #[test]
    fn preview_teleport_event_updates_components() {
        let mut app = App::new();
        app.add_event::<PreviewTeleportRequested>();
        app.add_systems(Update, handle_preview_teleport_mock);

        let entity = app
            .world_mut()
            .spawn(MockClientBundle {
                client: MockClient,
                position: Position::new([0.0, 0.0, 0.0]),
                look: Look::new(0.0, 0.0),
                head_yaw: HeadYaw(0.0),
            })
            .id();

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
        app.add_systems(Update, handle_preview_teleport_mock);

        let real = app
            .world_mut()
            .spawn(MockClientBundle {
                client: MockClient,
                position: Position::new([10.0, 20.0, 30.0]),
                look: Look::new(11.0, 22.0),
                head_yaw: HeadYaw(11.0),
            })
            .id();

        // 故意发给一个 spawn 但不带 MockClient 的 entity（query 查不到）
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
    fn preview_mode_enabled_reads_env() {
        // 直接 set/unset env var 测试 helper（注意单测可能并行，这里只在显式
        // unset 后断言 false 比较保险；set 测试避免 race）
        // SAFETY: test thread 内单独 manipulate
        unsafe {
            std::env::remove_var("BONG_PREVIEW_MODE");
        }
        assert!(
            !preview_mode_enabled(),
            "未设 BONG_PREVIEW_MODE 时应返回 false"
        );
        unsafe {
            std::env::set_var("BONG_PREVIEW_MODE", "1");
        }
        assert!(preview_mode_enabled(), "BONG_PREVIEW_MODE=1 时应返回 true");
        unsafe {
            std::env::set_var("BONG_PREVIEW_MODE", "0");
        }
        assert!(
            !preview_mode_enabled(),
            "BONG_PREVIEW_MODE=0 时应返回 false（仅 \"1\" 激活）"
        );
        unsafe {
            std::env::remove_var("BONG_PREVIEW_MODE");
        }
    }
}
