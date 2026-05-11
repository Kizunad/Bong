use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::{CommandArg, CommandArgParseError, ParseInput};
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, Update};
use valence::protocol::packets::play::command_tree_s2c::Parser;

use crate::inventory::{clear_player_inventory, ClearScope, PlayerInventory};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClearScopeArg(pub ClearScope);

impl CommandArg for ClearScopeArg {
    fn parse_arg(input: &mut ParseInput) -> Result<Self, CommandArgParseError> {
        let raw = String::parse_arg(input)?;
        parse_clear_scope(raw.as_str()).map(Self).ok_or_else(|| {
            CommandArgParseError::InvalidArgument {
                expected: "pack|all|naked".to_string(),
                got: raw,
            }
        })
    }

    fn display() -> Parser {
        String::display()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearInvCmd {
    Clear { scope: ClearScope },
}

impl Command for ClearInvCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        let clearinv = graph
            .root()
            .literal("clearinv")
            .with_executable(|_| ClearInvCmd::Clear {
                scope: ClearScope::PackOnly,
            })
            .id();

        graph
            .at(clearinv)
            .argument("scope")
            .with_parser::<ClearScopeArg>()
            .with_executable(|input| ClearInvCmd::Clear {
                scope: ClearScopeArg::parse_arg(input).unwrap().0,
            });
    }
}

pub fn register(app: &mut App) {
    app.add_command::<ClearInvCmd>()
        .add_systems(Update, handle_clearinv);
}

pub fn handle_clearinv(
    mut events: EventReader<CommandResultEvent<ClearInvCmd>>,
    mut players: Query<(&mut PlayerInventory, &mut Client)>,
) {
    for event in events.read() {
        let Ok((mut inventory, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        let ClearInvCmd::Clear { scope } = event.result;
        clear_player_inventory(&mut inventory, scope);
        client.send_chat_message(format!(
            "[dev] clearinv {scope:?} revision={}",
            inventory.revision.0
        ));
    }
}

pub fn parse_clear_scope(raw: &str) -> Option<ClearScope> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "pack" => Some(ClearScope::PackOnly),
        "all" => Some(ClearScope::PackAndHotbar),
        "naked" => Some(ClearScope::All),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
        MAIN_PACK_CONTAINER_ID,
    };
    use std::collections::HashMap;
    use valence::prelude::Events;

    fn item(id: u64, template_id: &str) -> ItemInstance {
        ItemInstance {
            instance_id: id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: "test item".to_string(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn inventory() -> PlayerInventory {
        let mut equipped = HashMap::new();
        equipped.insert("weapon".to_string(), item(4, "sword"));
        let mut hotbar: [Option<ItemInstance>; 9] = Default::default();
        hotbar[0] = Some(item(3, "hotbar_item"));
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![
                ContainerState {
                    id: MAIN_PACK_CONTAINER_ID.to_string(),
                    name: "主背包".to_string(),
                    rows: 2,
                    cols: 2,
                    items: vec![PlacedItemState {
                        row: 0,
                        col: 0,
                        instance: item(1, "main_item"),
                    }],
                },
                ContainerState {
                    id: "side_pack".to_string(),
                    name: "侧袋".to_string(),
                    rows: 1,
                    cols: 1,
                    items: vec![PlacedItemState {
                        row: 0,
                        col: 0,
                        instance: item(2, "side_item"),
                    }],
                },
            ],
            equipped,
            hotbar,
            bone_coins: 0,
            max_weight: 99.0,
        }
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<ClearInvCmd>>();
        app.add_systems(Update, handle_clearinv);
        app
    }

    fn spawn_player(app: &mut App) -> valence::prelude::Entity {
        let player = spawn_test_client(app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut().entity_mut(player).insert(inventory());
        player
    }

    fn send(app: &mut App, player: valence::prelude::Entity, scope: ClearScope) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<ClearInvCmd>>>()
            .send(CommandResultEvent {
                result: ClearInvCmd::Clear { scope },
                executor: player,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn parse_clear_scope_accepts_three_modes() {
        assert_eq!(parse_clear_scope("pack"), Some(ClearScope::PackOnly));
        assert_eq!(parse_clear_scope("all"), Some(ClearScope::PackAndHotbar));
        assert_eq!(parse_clear_scope("naked"), Some(ClearScope::All));
        assert_eq!(parse_clear_scope("missing"), None);
    }

    #[test]
    fn clearinv_pack_clears_only_main_pack() {
        let mut app = setup_app();
        let player = spawn_player(&mut app);

        send(&mut app, player, ClearScope::PackOnly);
        run_update(&mut app);

        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(inv.containers[0].items.is_empty());
        assert_eq!(inv.containers[1].items.len(), 1);
        assert!(inv.hotbar[0].is_some());
        assert_eq!(inv.equipped.len(), 1);
    }

    #[test]
    fn clearinv_all_clears_containers_and_hotbar_but_keeps_equipment() {
        let mut app = setup_app();
        let player = spawn_player(&mut app);

        send(&mut app, player, ClearScope::PackAndHotbar);
        run_update(&mut app);

        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(inv
            .containers
            .iter()
            .all(|container| container.items.is_empty()));
        assert!(inv.hotbar.iter().all(Option::is_none));
        assert_eq!(inv.equipped.len(), 1);
    }

    #[test]
    fn clearinv_naked_clears_equipment_too() {
        let mut app = setup_app();
        let player = spawn_player(&mut app);

        send(&mut app, player, ClearScope::All);
        run_update(&mut app);

        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(inv
            .containers
            .iter()
            .all(|container| container.items.is_empty()));
        assert!(inv.hotbar.iter().all(Option::is_none));
        assert!(inv.equipped.is_empty());
        assert_eq!(inv.revision, InventoryRevision(1));
    }
}
