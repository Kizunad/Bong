//! 放置真实炼丹炉的开发场景；打开窗口仍走生产炼丹链路。

use std::collections::HashMap;

use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{bevy_ecs, BlockPos, Commands, DVec3, Entity, Query, Res, ResMut, Resource};

use crate::alchemy::{AlchemyFurnace, LearnedRecipes, MIN_ZONE_QI_TO_ALCHEMY};
use crate::combat::CombatClock;
use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

// 留出余量，避免场景创建后一次自然灵气消耗就跌回起炉门槛。
const TEST_ZONE_QI_MARGIN: f64 = 0.5;

#[derive(Default, Resource)]
pub(super) struct AlchemySceneState {
    furnaces: HashMap<Entity, (Entity, BlockPos)>,
}

#[derive(SystemParam)]
pub(super) struct AlchemySceneContext<'w, 's> {
    commands: Commands<'w, 's>,
    state: ResMut<'w, AlchemySceneState>,
    dimensions: Query<'w, 's, &'static CurrentDimension>,
    furnaces: Query<'w, 's, &'static AlchemyFurnace>,
    learned: Query<'w, 's, &'static mut LearnedRecipes>,
    inventories: Query<'w, 's, &'static mut PlayerInventory>,
    items: Option<Res<'w, ItemRegistry>>,
    allocator: Option<ResMut<'w, InventoryInstanceIdAllocator>>,
    clock: Option<Res<'w, CombatClock>>,
    zones: Option<ResMut<'w, ZoneRegistry>>,
}

impl AlchemySceneContext<'_, '_> {
    pub fn start(
        &mut self,
        player: Entity,
        owner: &str,
        origin: BlockPos,
    ) -> Result<BlockPos, &'static str> {
        if !self
            .dimensions
            .get(player)
            .is_ok_and(|value| value.0 == DimensionKind::Overworld)
        {
            return Err("请在主世界加载炼丹场景。");
        }
        let existing = self.state.furnaces.get(&player).copied();
        let pos = existing
            .map(|(_, pos)| pos)
            .or_else(|| {
                [(2, 0), (-2, 0), (0, 2), (0, -2)]
                    .into_iter()
                    .map(|(x, z)| BlockPos::new(origin.x + x, origin.y, origin.z + z))
                    .find(|pos| {
                        !self
                            .furnaces
                            .iter()
                            .any(|furnace| furnace.block_pos() == Some(*pos))
                    })
            })
            .ok_or("身边没有安全的空位，请移到平地再试。")?;
        let zones = self.zones.as_deref_mut().ok_or("区域尚未就绪。")?;
        // 与真实起炉检查一致：按炉位查找最具体区域，区域外使用 spawn 兜底。
        // 不能按玩家脚下或固定 spawn 加灵气，否则跨边界摆炉仍会被门槛拒绝。
        let zone_name = zones
            .find_zone(
                DimensionKind::Overworld,
                DVec3::new(pos.x as f64, pos.y as f64, pos.z as f64),
            )
            .or_else(|| zones.find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME))
            .map(|zone| zone.name.clone())
            .ok_or("此处没有可用的炼丹区域。")?;
        let zone = zones.find_zone_mut(&zone_name).ok_or("区域尚未就绪。")?;
        let items = self.items.as_deref().ok_or("物品尚未就绪。")?;
        let allocator = self.allocator.as_deref_mut().ok_or("背包尚未就绪。")?;
        let clock = self.clock.as_deref().ok_or("世界尚未就绪。")?;
        // 开发场景给出真实库存，后续投料仍走生产扣料链路。
        let mut inventory = self
            .inventories
            .get_mut(player)
            .map_err(|_| "尚未找到背包。")?;
        let mut prepared = inventory.clone();
        for (material, count) in [
            ("spirit_grass", 24),
            ("incense_plain", 4),
            ("dried_grass", 4),
            ("wood_plank", 2),
        ] {
            add_item_to_player_inventory(
                &mut prepared,
                items,
                allocator,
                material,
                count,
                clock.tick,
            )
            .map_err(|_| "请腾出背包空间，再领取炼丹材料。")?;
        }
        *inventory = prepared;
        // 仅 OP 测试场景直写，与 /zone_qi set 一样显式绕过 ledger；生产起炉仍正常校验。
        let before = zone.spirit_qi;
        zone.spirit_qi = before.max(MIN_ZONE_QI_TO_ALCHEMY + TEST_ZONE_QI_MARGIN);
        tracing::warn!(
            "[dev-cmd] alchemy scene bypass ledger: zone `{zone_name}` {before:.3} -> {:.3}",
            zone.spirit_qi
        );
        if let Ok(mut learned) = self.learned.get_mut(player) {
            learned.learn("ling_xi_wan_v1".into());
            if let Some(index) = learned.ids.iter().position(|id| id == "ling_xi_wan_v1") {
                learned.current_index = index;
            }
        }
        if existing.is_some() {
            return Ok(pos);
        }

        let furnace = self
            .commands
            .spawn({
                let mut furnace = AlchemyFurnace::placed(pos, 1);
                furnace.owner = Some(owner.to_owned());
                furnace
            })
            .id();
        self.state.furnaces.insert(player, (furnace, pos));

        Ok(pos)
    }

    pub fn clear(&mut self, player: Entity) -> Result<(), &'static str> {
        let Some(&(entity, _)) = self.state.furnaces.get(&player) else {
            return Ok(());
        };
        if self.furnaces.get(entity).is_ok_and(AlchemyFurnace::is_busy) {
            return Err("测试炉仍在炼制，请收取当前炉次后再清理或切换场景。");
        }
        self.commands.entity(entity).despawn();
        self.state.furnaces.remove(&player);
        Ok(())
    }
}
