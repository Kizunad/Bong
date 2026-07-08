use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, BlockPos, Client, EventReader, Events, Query, Res, ResMut, Update};

use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::mineral::{MineralDropEvent, MineralRegistry};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GiveCmd {
    Item { id: String, count: u32 },
}

impl Command for GiveCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        let give = graph
            .root()
            .literal("give")
            .argument("id")
            .with_parser::<String>()
            .with_executable(|input| GiveCmd::Item {
                id: String::parse_arg(input).unwrap(),
                count: 1,
            })
            .id();

        graph
            .at(give)
            .argument("count")
            .with_parser::<u32>()
            .with_executable(|input| GiveCmd::Item {
                id: String::parse_arg(input).unwrap(),
                count: u32::parse_arg(input).unwrap(),
            });
    }
}

pub fn register(app: &mut App) {
    app.init_resource::<ItemRegistry>()
        .init_resource::<InventoryInstanceIdAllocator>()
        .add_command::<GiveCmd>()
        .add_systems(Update, handle_give);
}

#[allow(clippy::too_many_arguments)]
pub fn handle_give(
    mut events: EventReader<CommandResultEvent<GiveCmd>>,
    registry: Res<ItemRegistry>,
    mineral_registry: Option<Res<MineralRegistry>>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut players: Query<(&mut PlayerInventory, &mut Client)>,
    mut mineral_drops: Option<ResMut<Events<MineralDropEvent>>>,
) {
    for event in events.read() {
        let Ok((mut inventory, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        let GiveCmd::Item { id, count } = &event.result;
        if *count == 0 {
            client.send_chat_message("[dev] give rejected: count must be >= 1");
            continue;
        }

        // dev-only fallback：id 不是 ItemRegistry 模板，但是合法的裸矿物 mineral_id
        // （如 za_gang——只能靠挖矿掉落，plan-mineral-v1 §2.2 刻意不给它建 TOML 模板，
        // 见 inventory_grant.rs 头注）。复用生产链路 `MineralDropEvent` →
        // `consume_mineral_drops_into_inventory` 批量塞，供 dev/bot 场景搭建炼器/炼丹
        // 材料，不重造矿物 ItemInstance 构造逻辑（plan-forge-session-entry-wiring-v1
        // P2 全链路 e2e 新发现缺口：qing_feng_v0 需要 za_gang，此前 dev 无法给到）。
        if registry.get(id).is_none() {
            if let Some(entry) = mineral_registry
                .as_deref()
                .and_then(|reg| reg.get_by_str(id))
            {
                let mineral_id = entry.id;
                let canonical_name = entry.canonical_name;
                let Some(tx) = mineral_drops.as_deref_mut() else {
                    client.send_chat_message(format!(
                        "[dev] give `{id}` failed: mineral drop channel unavailable"
                    ));
                    continue;
                };
                for _ in 0..*count {
                    tx.send(MineralDropEvent {
                        player: event.executor,
                        mineral_id,
                        position: BlockPos::new(0, 0, 0),
                    });
                }
                client.send_chat_message(format!("[dev] gave mineral {canonical_name} x{count}"));
                continue;
            }
        }

        match add_item_to_player_inventory(&mut inventory, &registry, &mut allocator, id, *count, 0)
        {
            Ok(receipt) => {
                client.send_chat_message(format!(
                    "[dev] gave {} x{} revision={}",
                    receipt.template_id, receipt.stack_count, receipt.revision.0
                ));
            }
            Err(error) => {
                client.send_chat_message(format!("[dev] give `{id}` failed: {error}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemCategory, ItemRarity, ItemTemplate,
        DEFAULT_CAST_DURATION_MS, DEFAULT_COOLDOWN_MS, MAIN_PACK_CONTAINER_ID,
    };
    use std::collections::HashMap;
    use valence::prelude::{Events, IntoSystemConfigs};

    fn test_template(id: &str) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: id.to_string(),
            category: ItemCategory::Misc,
            placeable: None,
            max_stack_count: 64,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.1,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 1.0,
            description: "test template".to_string(),
            effect: None,
            cast_duration_ms: DEFAULT_CAST_DURATION_MS,
            cooldown_ms: DEFAULT_COOLDOWN_MS,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            readable_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shield_spec: None,
            shelflife_track: None,
        }
    }

    fn registry(ids: &[&str]) -> ItemRegistry {
        ItemRegistry::from_map(
            ids.iter()
                .map(|id| ((*id).to_string(), test_template(id)))
                .collect::<HashMap<_, _>>(),
        )
    }

    fn inventory(rows: u8, cols: u8) -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows,
                cols,
                items: Vec::new(),
                owner_instance_id: None,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 99.0,
        }
    }

    fn setup_app(registry: ItemRegistry) -> App {
        let mut app = App::new();
        app.insert_resource(registry);
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.add_event::<CommandResultEvent<GiveCmd>>();
        app.add_systems(Update, handle_give);
        app
    }

    fn spawn_player(app: &mut App, inventory: PlayerInventory) -> valence::prelude::Entity {
        let player = spawn_test_client(app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut().entity_mut(player).insert(inventory);
        player
    }

    fn send(app: &mut App, player: valence::prelude::Entity, id: &str, count: u32) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<GiveCmd>>>()
            .send(CommandResultEvent {
                result: GiveCmd::Item {
                    id: id.to_string(),
                    count,
                },
                executor: player,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn give_defaults_to_one_and_accepts_explicit_count() {
        let mut app = setup_app(registry(&["qicao_grass"]));
        let player = spawn_player(&mut app, inventory(2, 4));

        send(&mut app, player, "qicao_grass", 1);
        send(&mut app, player, "qicao_grass", 32);
        run_update(&mut app);

        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inv.containers[0].items.len(), 1);
        assert_eq!(inv.containers[0].items[0].instance.stack_count, 33);
    }

    #[test]
    fn give_rejects_unknown_and_zero_count_without_mutation() {
        let mut app = setup_app(registry(&["qicao_grass"]));
        let player = spawn_player(&mut app, inventory(2, 4));

        send(&mut app, player, "missing", 1);
        send(&mut app, player, "qicao_grass", 0);
        run_update(&mut app);

        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(inv.containers[0].items.is_empty());
        assert_eq!(inv.revision, InventoryRevision(0));
    }

    #[test]
    fn give_reports_inventory_full_as_error() {
        let mut app = setup_app(registry(&["qicao_grass"]));
        let player = spawn_player(&mut app, inventory(1, 1));

        send(&mut app, player, "qicao_grass", 64);
        run_update(&mut app);
        send(&mut app, player, "qicao_grass", 1);
        run_update(&mut app);

        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inv.containers[0].items.len(), 1);
        assert_eq!(inv.containers[0].items[0].instance.stack_count, 64);
    }

    // ── plan-forge-session-entry-wiring-v1 P2 — 裸矿物 mineral_id fallback ──────

    fn setup_app_with_minerals(item_registry: ItemRegistry) -> App {
        let mut app = setup_app(item_registry);
        app.insert_resource(crate::mineral::build_default_registry());
        app.add_event::<MineralDropEvent>();
        app
    }

    #[test]
    fn give_mineral_only_id_queues_one_drop_event_per_count() {
        // za_gang 没有 ItemRegistry TOML 模板（plan-mineral-v1 §2.2 刻意如此）——
        // 起炉 e2e 需要它，dev fallback 必须能给。
        let mut app = setup_app_with_minerals(registry(&["qicao_grass"]));
        let player = spawn_player(&mut app, inventory(5, 7));

        send(&mut app, player, "za_gang", 3);
        run_update(&mut app);

        let drops = app.world().resource::<Events<MineralDropEvent>>();
        let sent: Vec<_> = drops.iter_current_update_events().collect();
        assert_eq!(
            sent.len(),
            3,
            "give za_gang x3 应恰好排 3 条 MineralDropEvent（1 count = 1 事件，\
             对齐挖矿单方块产一枚的等价语义），实际={}",
            sent.len()
        );
        assert!(
            sent.iter()
                .all(|e| e.player == player && e.mineral_id == crate::mineral::MineralId::ZaGang),
            "每条 MineralDropEvent 的 player/mineral_id 都应匹配 give 的执行者与目标矿物，实际={sent:?}"
        );
    }

    #[test]
    fn give_mineral_only_id_lands_in_inventory_via_real_consumer_system() {
        // 全链路：give → MineralDropEvent → 生产消费系统 consume_mineral_drops_into_inventory
        // → 真实入包。只测 handle_give 排队而不接消费者会漏掉「id 到底能不能落地」这个
        // 关键契约，故这里刻意接真系统而非只断言事件队列。
        let mut app = setup_app_with_minerals(registry(&["qicao_grass"]));
        app.insert_resource(crate::mineral::persistence::MineralTickClock::default());
        app.add_systems(
            Update,
            crate::mineral::inventory_grant::consume_mineral_drops_into_inventory
                .after(handle_give),
        );
        let player = spawn_player(&mut app, inventory(5, 7));

        send(&mut app, player, "za_gang", 2);
        run_update(&mut app);

        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        let placed = inv
            .containers
            .iter()
            .flat_map(|c| c.items.iter())
            .find(|p| p.instance.template_id == "mineral_za_gang");
        let placed = placed.expect(
            "give za_gang 应通过 MineralDropEvent 真落地为 mineral_za_gang（consume_mineral_drops_into_inventory 消费），实际背包无此 item",
        );
        assert_eq!(
            placed.instance.mineral_id.as_deref(),
            Some("za_gang"),
            "落地的 ItemInstance.mineral_id 应为 za_gang（forge 原子扣料靠这个字段匹配）"
        );
    }

    #[test]
    fn give_mineral_channel_unavailable_reports_error_without_panic() {
        // MineralRegistry 有、但 Events<MineralDropEvent> 未注册（理论边界/精简测试
        // App）——必须报错而不是 panic 或静默吞掉。
        let mut app = setup_app(registry(&["qicao_grass"]));
        app.insert_resource(crate::mineral::build_default_registry());
        // 故意不 app.add_event::<MineralDropEvent>()。
        let player = spawn_player(&mut app, inventory(5, 7));

        send(&mut app, player, "za_gang", 1);
        run_update(&mut app);

        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(
            inv.containers[0].items.is_empty(),
            "drop 通道不可用时不应有任何物品落地"
        );
    }

    #[test]
    fn give_unknown_id_still_rejected_when_mineral_registry_present() {
        // MineralRegistry 存在，但 id 既不是 ItemRegistry 模板也不是合法 mineral_id
        // ——应落回原本的「unknown item template id」拒绝路径，不应误判成矿物。
        let mut app = setup_app_with_minerals(registry(&["qicao_grass"]));
        let player = spawn_player(&mut app, inventory(5, 7));

        send(&mut app, player, "definitely_not_a_thing", 1);
        run_update(&mut app);

        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(inv.containers[0].items.is_empty());
        let drops = app.world().resource::<Events<MineralDropEvent>>();
        assert_eq!(
            drops.iter_current_update_events().count(),
            0,
            "非法 id 不应误判为矿物并排 MineralDropEvent"
        );
    }
}
