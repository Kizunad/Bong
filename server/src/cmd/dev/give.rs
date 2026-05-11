use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, Res, ResMut, Update};

use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};

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

pub fn handle_give(
    mut events: EventReader<CommandResultEvent<GiveCmd>>,
    registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut players: Query<(&mut PlayerInventory, &mut Client)>,
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
        match add_item_to_player_inventory(&mut inventory, &registry, &mut allocator, id, *count) {
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
    use valence::prelude::Events;

    fn test_template(id: &str) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: id.to_string(),
            category: ItemCategory::Misc,
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
}
