//! plan-worldgen-v4 P5 §8.1#5 — 画廊审阅 owo 方块面板 dev-only give-block handler。
//!
//! client `BlockPickerPanel`（嵌入 InspectScreen，仅 dev/creative 显示）点击一个 vanilla
//! 方块条目后，发 C2S `ClientRequestV1::BlockPickerGive { block_id, count }`。
//! `client_request_handler` 把它翻译成 [`BlockPickerGiveIntent`] event，本模块的
//! [`handle_block_picker_give`] system 消费：
//!
//! 1. **gamemode 门控**：仅当玩家处于 `GameMode::Creative` 时受理（防生产玩家伪造包刷方块）。
//! 2. 从 [`ItemRegistry`] 取 `vanilla:<block_id>` 模板（由 `inject_vanilla_block_templates`
//!    在启动期为全部 vanilla `BlockKind` 自动注册）。
//! 3. 调 [`add_item_to_player_inventory`] 把对应 vanilla 方块物品放进背包。
//!
//! **dev-only 红线**：本入口显式绕过自然修炼/经济规则，不进生产 gameplay 路径。
//! gamemode 门控是运行时拒绝（不像挂 dev 命令树那样仅靠约定），生产玩家默认 Survival
//! 无法触发。

use valence::message::SendMessage;
use valence::prelude::{
    bevy_ecs, App, Client, Event, EventReader, GameMode, IntoSystemConfigs, Query, Res, ResMut,
    Update,
};

use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
    VANILLA_TEMPLATE_PREFIX,
};

/// 由 `client_request_handler` 在收到 `ClientRequestV1::BlockPickerGive` 后 emit。
#[derive(Event, Debug, Clone, PartialEq, Eq)]
pub struct BlockPickerGiveIntent {
    /// 发起请求的玩家 entity。
    pub player: valence::prelude::Entity,
    /// 不含 namespace 的 vanilla 方块短名（如 `stone_bricks`）。
    pub block_id: String,
    /// 给予数量（1..=64，已由 schema serde 守门；handler 仍完整复查 0 与 >64，因为本
    /// 内部 event 可绕过 wire 直接构造）。
    pub count: u32,
}

pub fn register(app: &mut App) {
    app.init_resource::<ItemRegistry>()
        .init_resource::<InventoryInstanceIdAllocator>()
        .add_event::<BlockPickerGiveIntent>()
        .add_systems(
            Update,
            handle_block_picker_give
                .after(crate::network::client_request_handler::handle_client_request_payloads),
        );
}

/// 消费 [`BlockPickerGiveIntent`]：gamemode 门控 + ItemRegistry 查 `vanilla:<id>` + 入背包。
pub fn handle_block_picker_give(
    mut events: EventReader<BlockPickerGiveIntent>,
    registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut players: Query<(&GameMode, &mut PlayerInventory, &mut Client)>,
) {
    for event in events.read() {
        let Ok((game_mode, mut inventory, mut client)) = players.get_mut(event.player) else {
            continue;
        };

        // ① gamemode 门控：仅 Creative 受理。生产玩家默认 Survival → 直接拒绝。
        if *game_mode != GameMode::Creative {
            client.send_chat_message(
                "§c[dev] block picker rejected: requires Creative mode (dev-only)",
            );
            continue;
        }

        // ② count 防御（schema serde 已锁 1..=64，但 BlockPickerGiveIntent 是内部 event，
        //    可绕过 wire 直接构造，故 handler 完整复查上下界——纵深防御）。
        if event.count == 0 {
            client.send_chat_message("§c[dev] block picker rejected: count must be >= 1");
            continue;
        }
        if event.count > crate::schema::client_request::MAX_BLOCK_PICKER_COUNT_V1 {
            client.send_chat_message(format!(
                "§c[dev] block picker rejected: count must be <= {} (one stack)",
                crate::schema::client_request::MAX_BLOCK_PICKER_COUNT_V1
            ));
            continue;
        }

        // ③ 拼 vanilla:<block_id> 查 registry → 入背包。
        let template_id = format!("{VANILLA_TEMPLATE_PREFIX}{}", event.block_id);
        match add_item_to_player_inventory(
            &mut inventory,
            &registry,
            &mut allocator,
            &template_id,
            event.count,
            0,
        ) {
            Ok(receipt) => {
                client.send_chat_message(format!(
                    "§a[dev] gave {} x{} (revision={})",
                    receipt.template_id, receipt.stack_count, receipt.revision.0
                ));
            }
            Err(error) => {
                client.send_chat_message(format!(
                    "§c[dev] block picker `{}` failed: {error}",
                    event.block_id
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemRegistry, MAIN_PACK_CONTAINER_ID,
    };
    use std::collections::HashMap;
    use valence::prelude::{Entity, Events, GameMode};

    /// 真 registry：含全部 vanilla:<id> 模板（与生产启动期一致），不 mock。
    fn registry_with_vanilla() -> ItemRegistry {
        crate::inventory::load_item_registry().expect("load item registry (incl. vanilla blocks)")
    }

    fn inventory(rows: u8, cols: u8) -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows,
                cols,
                items: Vec::new(),
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 999.0,
        }
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.insert_resource(registry_with_vanilla());
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.add_event::<BlockPickerGiveIntent>();
        app.add_systems(Update, handle_block_picker_give);
        app
    }

    fn spawn_player(app: &mut App, gamemode: GameMode, inv: PlayerInventory) -> Entity {
        let player = spawn_test_client(app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut().entity_mut(player).insert((gamemode, inv));
        player
    }

    fn send(app: &mut App, player: Entity, block_id: &str, count: u32) {
        app.world_mut()
            .resource_mut::<Events<BlockPickerGiveIntent>>()
            .send(BlockPickerGiveIntent {
                player,
                block_id: block_id.to_string(),
                count,
            });
    }

    fn stack_count_of(app: &App, player: Entity, template_id: &str) -> u32 {
        app.world()
            .get::<PlayerInventory>(player)
            .unwrap()
            .containers[0]
            .items
            .iter()
            .filter(|it| it.instance.template_id == template_id)
            .map(|it| it.instance.stack_count)
            .sum()
    }

    /// give-block e2e（核心链路）：Creative 玩家 BlockPickerGive(stone_bricks, 8)
    /// → handler → ItemRegistry vanilla:stone_bricks → 真进 inventory（非僵尸）。
    #[test]
    fn creative_player_receives_vanilla_block_in_inventory() {
        let mut app = setup_app();
        let player = spawn_player(&mut app, GameMode::Creative, inventory(4, 9));

        send(&mut app, player, "stone_bricks", 8);
        run_update(&mut app);

        assert_eq!(
            stack_count_of(&app, player, "vanilla:stone_bricks"),
            8,
            "Creative 玩家请求 8 块 stone_bricks 后，背包应有 vanilla:stone_bricks x8"
        );
    }

    /// gamemode 门控：Survival 玩家请求被拒，背包无任何变动（revision 不变）。
    #[test]
    fn survival_player_request_is_rejected_without_mutation() {
        let mut app = setup_app();
        let player = spawn_player(&mut app, GameMode::Survival, inventory(4, 9));

        send(&mut app, player, "stone_bricks", 8);
        run_update(&mut app);

        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(
            inv.containers[0].items.is_empty(),
            "Survival 玩家的 block picker 请求必须被 gamemode 门控拒绝，背包应为空"
        );
        assert_eq!(
            inv.revision,
            InventoryRevision(0),
            "被拒绝的请求不得改动 inventory revision"
        );
    }

    /// 多个不同 vanilla 方块各自落槽，状态独立。
    #[test]
    fn multiple_distinct_blocks_each_land() {
        let mut app = setup_app();
        let player = spawn_player(&mut app, GameMode::Creative, inventory(4, 9));

        send(&mut app, player, "polished_blackstone", 16);
        send(&mut app, player, "stone_bricks", 4);
        run_update(&mut app);

        assert_eq!(
            stack_count_of(&app, player, "vanilla:polished_blackstone"),
            16,
            "polished_blackstone x16 应入背包"
        );
        assert_eq!(
            stack_count_of(&app, player, "vanilla:stone_bricks"),
            4,
            "stone_bricks x4 应入背包"
        );
    }

    /// 边界：count=0 被防御性拒绝，背包无变动。
    #[test]
    fn zero_count_is_rejected() {
        let mut app = setup_app();
        let player = spawn_player(&mut app, GameMode::Creative, inventory(4, 9));

        send(&mut app, player, "stone_bricks", 0);
        run_update(&mut app);

        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(
            inv.containers[0].items.is_empty(),
            "count=0 必须被拒绝，背包应为空"
        );
    }

    /// 边界：count=64（上界，一组）被受理，真进背包 64 个。
    #[test]
    fn max_count_64_is_accepted() {
        let mut app = setup_app();
        let player = spawn_player(&mut app, GameMode::Creative, inventory(4, 9));

        send(&mut app, player, "stone_bricks", 64);
        run_update(&mut app);

        assert_eq!(
            stack_count_of(&app, player, "vanilla:stone_bricks"),
            64,
            "count=64 是合法上界（一组），应真进背包 64 个 stone_bricks"
        );
    }

    /// 边界 + 错误分支：count=65（上界 +1）被 handler 防御性拒绝，背包无变动、revision 不变。
    /// 锁住「内部 event 绕过 schema serde 直接构造越界 count」时 handler 仍守门。
    #[test]
    fn over_max_count_65_is_rejected_without_mutation() {
        let mut app = setup_app();
        let player = spawn_player(&mut app, GameMode::Creative, inventory(4, 9));

        send(&mut app, player, "stone_bricks", 65);
        run_update(&mut app);

        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(
            inv.containers[0].items.is_empty(),
            "count=65 超一组上限，handler 必须拒绝，背包应为空"
        );
        assert_eq!(
            inv.revision,
            InventoryRevision(0),
            "被拒绝的越界请求不得改动 inventory revision"
        );
    }

    /// 错误分支：未知 block_id（无对应 vanilla:<id> 模板）被拒，背包无变动。
    #[test]
    fn unknown_block_id_is_rejected_without_mutation() {
        let mut app = setup_app();
        let player = spawn_player(&mut app, GameMode::Creative, inventory(4, 9));

        send(&mut app, player, "totally_not_a_real_block", 1);
        run_update(&mut app);

        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(
            inv.containers[0].items.is_empty(),
            "未知 block_id 无 vanilla:<id> 模板，应被 registry 查不到而拒绝，背包应为空"
        );
    }

    /// give-block → 放置链路对齐：vanilla:<id> 模板能经 block_item_to_state 解析为 BlockState。
    /// 锁住「能给到物品 ⇒ 也能放置」的端到端契约，防孤岛（给得到却放不下）。
    #[test]
    fn granted_vanilla_block_lowers_to_block_state_for_placement() {
        use crate::world::block_place::block_item_to_state;
        use crate::zhenfa::trap_content::TrapTargetFace;

        let state = block_item_to_state("vanilla:stone_bricks", TrapTargetFace::Top);
        assert!(
            state.is_some(),
            "vanilla:stone_bricks 必须能经 block_item_to_state 解析为 BlockState（给得到必放得下）"
        );
    }
}
