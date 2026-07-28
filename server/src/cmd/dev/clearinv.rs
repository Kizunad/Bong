use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::{CommandArg, CommandArgParseError, ParseInput};
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, Res, Update};
use valence::protocol::packets::play::command_tree_s2c::Parser;

use crate::inventory::{clear_player_inventory, ClearScope, ItemRegistry, PlayerInventory};

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
    registry: Res<ItemRegistry>,
) {
    for event in events.read() {
        let Ok((mut inventory, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        let ClearInvCmd::Clear { scope } = event.result;
        clear_player_inventory(&mut inventory, scope, &registry);
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
        container_id_for_worn_pack, instantiate_inventory_from_loadout, load_default_loadout,
        load_item_registry, worn_pack_instance_from_container_id, ContainerState,
        InventoryInstanceIdAllocator, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
        BASE_CARRY_CAPACITY, BODY_POCKET_CONTAINER_ID, EQUIP_SLOT_CHEST, MAIN_PACK_CONTAINER_ID,
    };
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
        let registry = load_item_registry().expect("real item registry should load");
        let loadout = load_default_loadout(&registry).expect("default loadout should load");
        let mut allocator = InventoryInstanceIdAllocator::default();
        let mut inventory = instantiate_inventory_from_loadout(&loadout, &mut allocator, &registry)
            .expect("default inventory should instantiate");

        inventory
            .containers
            .iter_mut()
            .find(|container| container.id == BODY_POCKET_CONTAINER_ID)
            .expect("default inventory should contain body_pocket")
            .items
            .push(PlacedItemState {
                row: 0,
                col: 0,
                instance: item(9_001, "body_sentinel"),
            });
        inventory.hotbar[8] = Some(item(9_002, "hotbar_sentinel"));
        inventory.containers.push(ContainerState {
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "legacy main pack".to_string(),
            rows: 1,
            cols: 1,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: item(9_003, "legacy_sentinel"),
            }],
            owner_instance_id: None,
            quick_access: false,
        });
        inventory
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<ClearInvCmd>>();
        app.insert_resource(load_item_registry().expect("real item registry should load"));
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

    fn worn_pack_identity(inventory: &PlayerInventory) -> (u64, String) {
        let pack = inventory
            .equipped
            .get(EQUIP_SLOT_CHEST)
            .and_then(|slot| {
                slot.worn
                    .iter()
                    .find(|item| item.template_id == "worn_grass_pouch")
            })
            .expect("default inventory should wear worn_grass_pouch");
        (
            pack.instance_id,
            container_id_for_worn_pack(pack.instance_id),
        )
    }

    #[test]
    fn parse_clear_scope_accepts_three_modes() {
        assert_eq!(parse_clear_scope("pack"), Some(ClearScope::PackOnly));
        assert_eq!(parse_clear_scope("all"), Some(ClearScope::PackAndHotbar));
        assert_eq!(parse_clear_scope("naked"), Some(ClearScope::All));
        assert_eq!(parse_clear_scope("missing"), None);
    }

    #[test]
    fn clearinv_pack_clears_legacy_and_dynamic_pack_only() {
        let mut app = setup_app();
        let player = spawn_player(&mut app);
        let before = app.world().get::<PlayerInventory>(player).unwrap();
        let previous_revision = before.revision;
        let (pack_instance_id, pack_container_id) = worn_pack_identity(before);
        assert!(
            before
                .containers
                .iter()
                .find(|container| container.id == pack_container_id)
                .is_some_and(|container| !container.items.is_empty()),
            "default dynamic pack must start non-empty so the clear assertion is meaningful"
        );

        send(&mut app, player, ClearScope::PackOnly);
        run_update(&mut app);

        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(
            inventory
                .containers
                .iter()
                .filter(|container| {
                    container.id == MAIN_PACK_CONTAINER_ID
                        || worn_pack_instance_from_container_id(&container.id).is_some()
                })
                .all(|container| container.items.is_empty()),
            "clearinv pack must clear legacy main_pack and every dynamic pack_<instance_id>"
        );
        assert!(
            inventory
                .containers
                .iter()
                .find(|container| container.id == BODY_POCKET_CONTAINER_ID)
                .expect("body_pocket must remain")
                .items
                .iter()
                .any(|placed| placed.instance.instance_id == 9_001),
            "clearinv pack must preserve body_pocket sentinel instance=9001"
        );
        assert_eq!(
            inventory.hotbar[8].as_ref().map(|item| item.instance_id),
            Some(9_002),
            "clearinv pack must preserve hotbar"
        );
        let dynamic_pack = inventory
            .containers
            .iter()
            .find(|container| container.id == pack_container_id)
            .expect("worn pack dynamic container must remain");
        assert_eq!(dynamic_pack.owner_instance_id, Some(pack_instance_id));
        assert_eq!(
            inventory.revision,
            InventoryRevision(previous_revision.0 + 1),
            "one clear command must bump revision exactly once"
        );
    }

    #[test]
    fn clearinv_all_clears_carried_items_but_preserves_pack_topology() {
        let mut app = setup_app();
        let player = spawn_player(&mut app);
        let before = app.world().get::<PlayerInventory>(player).unwrap();
        let previous_revision = before.revision;
        let (pack_instance_id, pack_container_id) = worn_pack_identity(before);

        send(&mut app, player, ClearScope::PackAndHotbar);
        run_update(&mut app);

        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(inventory
            .containers
            .iter()
            .all(|container| container.items.is_empty()));
        assert!(inventory.hotbar.iter().all(Option::is_none));
        let (actual_pack_id, _) = worn_pack_identity(inventory);
        assert_eq!(
            actual_pack_id, pack_instance_id,
            "worn pack instance must survive"
        );
        let dynamic_pack = inventory
            .containers
            .iter()
            .find(|container| container.id == pack_container_id)
            .expect("worn pack dynamic container must remain");
        assert_eq!(dynamic_pack.owner_instance_id, Some(pack_instance_id));
        assert!(
            (inventory.max_weight - 23.0).abs() < f64::EPSILON,
            "worn starter pack must retain BASE 15 + 8 capacity, got {}",
            inventory.max_weight
        );
        assert_eq!(
            inventory.revision,
            InventoryRevision(previous_revision.0 + 1),
            "one clear command must bump revision exactly once"
        );
    }

    #[test]
    fn clearinv_naked_removes_orphan_pack_and_restores_base_capacity() {
        let mut app = setup_app();
        let player = spawn_player(&mut app);
        let previous_revision = app.world().get::<PlayerInventory>(player).unwrap().revision;

        send(&mut app, player, ClearScope::All);
        run_update(&mut app);

        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(inventory
            .containers
            .iter()
            .all(|container| container.items.is_empty()));
        assert!(inventory.hotbar.iter().all(Option::is_none));
        assert!(inventory.equipped.is_empty());
        assert!(
            inventory
                .containers
                .iter()
                .all(|container| worn_pack_instance_from_container_id(&container.id).is_none()),
            "clearinv naked must remove dynamic pack containers after clearing equipment"
        );
        assert!(
            inventory
                .containers
                .iter()
                .any(|container| container.id == BODY_POCKET_CONTAINER_ID),
            "clearinv naked must preserve authoritative body_pocket topology"
        );
        assert!(
            (inventory.max_weight - BASE_CARRY_CAPACITY).abs() < f64::EPSILON,
            "naked capacity must return to BASE {BASE_CARRY_CAPACITY}, got {}",
            inventory.max_weight
        );
        assert_eq!(
            inventory.revision,
            InventoryRevision(previous_revision.0 + 1),
            "one clear command must bump revision exactly once"
        );
    }
}
