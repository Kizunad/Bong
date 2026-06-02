//! Dev 命令：`/baolongwang spawn` —— 在玩家前方 spawn 暴龙王 BOSS。
//!
//! 仿 `cmd/dev/whale.rs` 范本（专属根 literal `baolongwang`，避免 brigadier
//! root 重复导致 `command_registry_contains_pinned_root_literals` 测试撞红）。
//!
//! dev-only：绕过自然遭遇触发逻辑，直接 spawn；不走 worldview 自然刷新规则。

use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{
    App, Client, Commands, DVec3, Entity, EntityLayerId, EventReader, Look, Position, Query,
    Update, With,
};

use crate::dandao::boss_spawn::spawn_baolongwang_at;
use crate::world::dimension::OverworldLayer;

/// 玩家面前生成偏移：前方 10 格（BOSS 体型大，太近会遮挡视野）。
const BOSS_SUMMON_FORWARD: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaolongwangCmd {
    Spawn,
}

impl Command for BaolongwangCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("baolongwang")
            .literal("spawn")
            .with_executable(|_| Self::Spawn);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<BaolongwangCmd>()
        .add_systems(Update, handle_baolongwang_cmd);
}

type BaolongwangPlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static Look,
        Option<&'static EntityLayerId>,
        &'static mut Client,
    ),
>;

pub fn handle_baolongwang_cmd(
    mut commands: Commands,
    mut events: EventReader<CommandResultEvent<BaolongwangCmd>>,
    mut players: BaolongwangPlayerQuery<'_, '_>,
    layers: Query<Entity, With<OverworldLayer>>,
) {
    for event in events.read() {
        tracing::info!(
            "[bong][baolongwang-cmd] /baolongwang spawn dispatched executor={:?}",
            event.executor
        );
        let Ok((position, look, player_layer, mut client)) = players.get_mut(event.executor) else {
            tracing::warn!(
                "[bong][baolongwang-cmd] executor not in player query, skipping (executor={:?})",
                event.executor
            );
            continue;
        };
        let BaolongwangCmd::Spawn = event.result;

        let Some(layer) = player_layer
            .map(|layer| layer.0)
            .or_else(|| layers.iter().next())
        else {
            tracing::warn!("[bong][baolongwang-cmd] no active layer found, abort spawn");
            client.send_chat_message("/baolongwang spawn failed: no active layer.");
            continue;
        };

        // 沿玩家水平朝向前方 BOSS_SUMMON_FORWARD 格。
        let forward = DVec3::new(
            (-look.yaw.to_radians()).sin() as f64,
            0.0,
            (look.yaw.to_radians()).cos() as f64,
        );
        let player_pos = position.get();
        let spawn_pos = DVec3::new(
            player_pos.x + forward.x * BOSS_SUMMON_FORWARD,
            player_pos.y,
            player_pos.z + forward.z * BOSS_SUMMON_FORWARD,
        );

        let boss = spawn_baolongwang_at(&mut commands, layer, spawn_pos);
        tracing::info!(
            "[bong][baolongwang-cmd] spawned baolongwang entity={:?} pos=({:.1},{:.1},{:.1}) layer={:?}",
            boss,
            spawn_pos.x,
            spawn_pos.y,
            spawn_pos.z,
            layer
        );
        client.send_chat_message(format!(
            "§4[暴龙王] §f已在 ({:.1}, {:.1}, {:.1}) 召唤",
            spawn_pos.x, spawn_pos.y, spawn_pos.z
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::dandao::boss_spawn::BaolongwangMarker;
    use valence::prelude::Events;

    #[test]
    fn baolongwang_spawn_cmd_creates_boss_entity() {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<BaolongwangCmd>>();
        app.add_systems(Update, handle_baolongwang_cmd);
        let layer = app.world_mut().spawn(OverworldLayer).id();
        let player = spawn_test_client(&mut app, "TestPlayer", [100.0, 70.0, 100.0]);
        app.world_mut()
            .entity_mut(player)
            .insert(EntityLayerId(layer));

        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<BaolongwangCmd>>>()
            .send(CommandResultEvent {
                result: BaolongwangCmd::Spawn,
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<BaolongwangMarker>>();
        let spawned: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(spawned.len(), 1, "应 spawn 恰好一个暴龙王 BOSS 实体");
    }

    #[test]
    fn baolongwang_spawn_cmd_spawns_exactly_one_boss_entity() {
        // 验证 spawn 命令仅生成一个 BOSS 实体，不会重复 spawn。
        let mut app = App::new();
        app.add_event::<CommandResultEvent<BaolongwangCmd>>();
        app.add_systems(Update, handle_baolongwang_cmd);
        let layer = app.world_mut().spawn(OverworldLayer).id();
        let player = spawn_test_client(&mut app, "Player2", [200.0, 70.0, 200.0]);
        app.world_mut()
            .entity_mut(player)
            .insert(EntityLayerId(layer));

        // 发两次命令，只 spawn 两只（命令独立）
        for _ in 0..2 {
            app.world_mut()
                .resource_mut::<Events<CommandResultEvent<BaolongwangCmd>>>()
                .send(CommandResultEvent {
                    result: BaolongwangCmd::Spawn,
                    executor: player,
                    modifiers: Default::default(),
                });
            run_update(&mut app);
        }

        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<BaolongwangMarker>>();
        assert_eq!(
            query.iter(app.world()).count(),
            2,
            "每次 /baolongwang spawn 应恰好 spawn 一只 BOSS（无批量 spawn 副作用）"
        );
    }

    #[test]
    fn baolongwang_spawn_cmd_missing_executor_does_not_panic() {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<BaolongwangCmd>>();
        app.add_systems(Update, handle_baolongwang_cmd);

        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<BaolongwangCmd>>>()
            .send(CommandResultEvent {
                result: BaolongwangCmd::Spawn,
                executor: Entity::PLACEHOLDER,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<BaolongwangMarker>>();
        assert_eq!(query.iter(app.world()).count(), 0);
    }
}
