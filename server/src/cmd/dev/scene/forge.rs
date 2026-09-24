//! 放置真实炼器砧的开发场景；开窗、起炉与结算仍走生产链路。

use std::collections::HashMap;

use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{
    bevy_ecs, BlockPos, BlockState, ChunkLayer, Commands, Entity, Query, Res, ResMut, Resource,
    With,
};

use crate::forge::learned::LearnedBlueprints;
use crate::forge::session::ForgeSessions;
use crate::forge::station::WeaponForgeStation;
use crate::world::dimension::{CurrentDimension, DimensionKind, OverworldLayer};

#[derive(Default, Resource)]
pub(super) struct ForgeSceneState {
    stations: HashMap<Entity, (Entity, BlockPos)>,
}

#[derive(SystemParam)]
pub(super) struct ForgeSceneContext<'w, 's> {
    commands: Commands<'w, 's>,
    state: ResMut<'w, ForgeSceneState>,
    layers: Query<'w, 's, &'static mut ChunkLayer, With<OverworldLayer>>,
    dimensions: Query<'w, 's, &'static CurrentDimension>,
    stations: Query<'w, 's, &'static WeaponForgeStation>,
    learned: Query<'w, 's, &'static mut LearnedBlueprints>,
    sessions: Option<Res<'w, ForgeSessions>>,
}

impl ForgeSceneContext<'_, '_> {
    pub fn start(&mut self, player: Entity, origin: BlockPos) -> Result<BlockPos, &'static str> {
        if !self
            .dimensions
            .get(player)
            .is_ok_and(|value| value.0 == DimensionKind::Overworld)
        {
            return Err("请在主世界加载锻造场景。");
        }
        if let Some((_, pos)) = self.state.stations.get(&player) {
            return Ok(*pos);
        }
        let mut layer = self
            .layers
            .get_single_mut()
            .map_err(|_| "主世界尚未就绪。")?;
        let pos = [(2, 0), (-2, 0), (0, 2), (0, -2)]
            .into_iter()
            .map(|(x, z)| BlockPos::new(origin.x + x, origin.y, origin.z + z))
            .find(|pos| {
                layer.block(*pos).is_some_and(|block| block.state.is_air())
                    && layer
                        .block(BlockPos::new(pos.x, pos.y - 1, pos.z))
                        .is_some_and(|block| !block.state.is_air())
                    && !self
                        .stations
                        .iter()
                        .any(|station| station.block_pos() == Some(*pos))
            })
            .ok_or("身边没有安全的空位，请移到平地再试。")?;
        let station = self
            .commands
            .spawn(WeaponForgeStation::placed(pos, 2, player))
            .id();
        layer.set_block(pos, BlockState::ANVIL);
        self.state.stations.insert(player, (station, pos));
        if let Ok(mut learned) = self.learned.get_mut(player) {
            learned.learn("iron_sword_v0".into());
            learned.learn("qing_feng_v0".into());
        } else {
            let mut learned = LearnedBlueprints::new();
            learned.learn("iron_sword_v0".into());
            learned.learn("qing_feng_v0".into());
            self.commands.entity(player).insert(learned);
        }
        Ok(pos)
    }

    pub fn clear(&mut self, player: Entity) -> Result<(), &'static str> {
        let Some(&(entity, pos)) = self.state.stations.get(&player) else {
            return Ok(());
        };
        if self.stations.get(entity).is_ok_and(|station| {
            station
                .session
                .and_then(|id| self.sessions.as_deref()?.get(id))
                .is_some_and(|session| !session.is_done())
        }) {
            return Err("测试砧仍在锻造，请结束当前炉次后再清理或切换场景。");
        }
        if let Ok(mut layer) = self.layers.get_single_mut() {
            if layer
                .block(pos)
                .is_some_and(|block| block.state == BlockState::ANVIL)
            {
                layer.set_block(pos, BlockState::AIR);
            }
        }
        self.commands.entity(entity).despawn();
        self.state.stations.remove(&player);
        Ok(())
    }
}
