//! 默认方块破坏 apply 系统：把 valence `DiggingEvent` 转成实际的方块状态变更。
//!
//! valence 不会自动把 `DiggingEvent` 转成方块更新。Bong 内现有 3 个模块消费
//! `DiggingEvent`（mineral / spiritwood / social niche），它们各自只做 drop / index
//! 记账，不调 `set_block(AIR)` —— 没人抹平 chunk 状态时，server 内存里的方块原样
//! 保留，client 预测被覆盖回来 → 玩家看到"挖了又复原"。
//!
//! 本系统按 vanilla 规则统一处理"破坏完成"：
//!
//! - **Creative + Start**：瞬间破坏（vanilla 协议 Creative 模式只发 `Start`）
//! - **Survival + Stop**：挖掘动画走完
//! - **Survival + Start / Abort**：还在挖 / 取消，不动方块
//! - **Adventure / Spectator**：vanilla 默认无法破坏方块
//!
//! 业务模块（mineral 等）若要附加副作用（drop / 索引清理），仍消费同一份
//! `DiggingEvent`，本系统在 Bevy `Update` 阶段统一抹平 chunk —— 与业务 system 同
//! 一帧消费同一组 events，互不阻塞。

use valence::prelude::{
    App, BlockState, ChunkLayer, Client, DiggingEvent, DiggingState, EventReader, GameMode, Query,
    ResMut, Update, VisibleChunkLayer, With,
};

use crate::world::furniture::FurnitureRegistry;

/// 决策"这次 dig 是否要把方块抹成 AIR"。纯函数，与 ECS 解耦，便于 saturate 测试。
pub fn should_apply_default_break(state: DiggingState, mode: GameMode) -> bool {
    matches!(
        (state, mode),
        (DiggingState::Start, GameMode::Creative) | (DiggingState::Stop, GameMode::Survival)
    )
}

pub fn apply_default_block_break(
    mut digs: EventReader<DiggingEvent>,
    players: Query<(&GameMode, &VisibleChunkLayer), With<Client>>,
    mut layers: Query<&mut ChunkLayer>,
    mut furniture_registry: Option<ResMut<FurnitureRegistry>>,
) {
    for event in digs.read() {
        let Ok((game_mode, visible_layer)) = players.get(event.client) else {
            continue;
        };
        if !should_apply_default_break(event.state, *game_mode) {
            continue;
        }
        let Ok(mut layer) = layers.get_mut(visible_layer.0) else {
            continue;
        };
        layer.set_block(event.position, BlockState::AIR);
        if let Some(registry) = furniture_registry.as_deref_mut() {
            registry.remove([event.position.x, event.position.y, event.position.z]);
        }
    }
}

pub fn register(app: &mut App) {
    app.add_systems(Update, apply_default_block_break);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::furniture::FurnitureKind;
    use valence::prelude::{BlockPos, UnloadedChunk};
    use valence::testing::ScenarioSingleClient;

    /// 8 种 (state, mode) 组合中只有两条要应用：Creative+Start、Survival+Stop。
    /// 其余六条（含 Survival 中途 Start/Abort、Adventure/Spectator 全部）都跳过。
    #[test]
    fn should_apply_default_break_truth_table() {
        for state in [DiggingState::Start, DiggingState::Stop, DiggingState::Abort] {
            for mode in [
                GameMode::Survival,
                GameMode::Creative,
                GameMode::Adventure,
                GameMode::Spectator,
            ] {
                let expected = matches!(
                    (state, mode),
                    (DiggingState::Start, GameMode::Creative)
                        | (DiggingState::Stop, GameMode::Survival)
                );
                assert_eq!(
                    should_apply_default_break(state, mode),
                    expected,
                    "({state:?}, {mode:?}) expected={expected}"
                );
            }
        }
    }

    #[test]
    fn creative_only_breaks_on_start_not_stop_or_abort() {
        assert!(should_apply_default_break(
            DiggingState::Start,
            GameMode::Creative
        ));
        assert!(!should_apply_default_break(
            DiggingState::Stop,
            GameMode::Creative
        ));
        assert!(!should_apply_default_break(
            DiggingState::Abort,
            GameMode::Creative
        ));
    }

    #[test]
    fn survival_only_breaks_on_stop_not_start_or_abort() {
        assert!(!should_apply_default_break(
            DiggingState::Start,
            GameMode::Survival
        ));
        assert!(should_apply_default_break(
            DiggingState::Stop,
            GameMode::Survival
        ));
        assert!(!should_apply_default_break(
            DiggingState::Abort,
            GameMode::Survival
        ));
    }

    #[test]
    fn adventure_and_spectator_never_break() {
        for state in [DiggingState::Start, DiggingState::Stop, DiggingState::Abort] {
            for mode in [GameMode::Adventure, GameMode::Spectator] {
                assert!(
                    !should_apply_default_break(state, mode),
                    "({state:?}, {mode:?}) should not break"
                );
            }
        }
    }

    #[test]
    fn default_break_removes_furniture_registry_entry() {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.add_event::<DiggingEvent>();
        app.add_systems(Update, apply_default_block_break);
        app.insert_resource(FurnitureRegistry::default());
        app.world_mut()
            .entity_mut(scenario.client)
            .insert(GameMode::Survival);
        app.world_mut()
            .get_mut::<ChunkLayer>(scenario.layer)
            .expect("test layer should carry ChunkLayer")
            .insert_chunk([0, 0], UnloadedChunk::new());
        app.world_mut()
            .get_mut::<ChunkLayer>(scenario.layer)
            .expect("test layer should carry ChunkLayer")
            .set_block(BlockPos::new(1, 64, 1), BlockState::BONG_SIMPLE_BED);
        app.world_mut()
            .resource_mut::<FurnitureRegistry>()
            .register([1, 64, 1], FurnitureKind::SimpleBed);

        app.world_mut().send_event(DiggingEvent {
            client: scenario.client,
            position: BlockPos::new(1, 64, 1),
            direction: valence::protocol::Direction::Up,
            state: DiggingState::Stop,
        });
        app.update();

        assert_eq!(
            app.world()
                .resource::<FurnitureRegistry>()
                .kind_at([1, 64, 1]),
            None,
            "breaking a placed furniture block should remove its registry coordinate"
        );
        assert_eq!(
            app.world()
                .get::<ChunkLayer>(scenario.layer)
                .and_then(|layer| layer
                    .block(BlockPos::new(1, 64, 1))
                    .map(|block| block.state)),
            Some(BlockState::AIR)
        );
    }
}
