use super::*;
use valence::prelude::bevy_ecs::{system::SystemState, world::World};

fn fixture() -> (World, PlayerInventory) {
    let registry = load_item_registry().expect("生产物品表应能加载");
    let loadout = load_default_loadout(&registry).expect("生产初始背包应能加载");
    let inventory = instantiate_inventory_from_loadout(
        &loadout,
        &mut InventoryInstanceIdAllocator::default(),
        &registry,
    )
    .unwrap();
    let mut world = World::new();
    world.insert_resource(registry);
    world.insert_resource(DefaultLoadout(loadout));
    world.insert_resource(DevCommandPermissions::allow_user("Tester"));
    world.insert_resource(DroppedLootRegistry::default());
    (world, inventory)
}

fn reconcile(
    world: &mut World,
    username: &str,
    inventory: &PlayerInventory,
) -> io::Result<Option<PlayerInventory>> {
    let mut state = SystemState::<OperatorInventoryAccess>::new(world);
    state.get_mut(world).reconcile(
        username,
        inventory,
        [4.0, 65.0, 8.0],
        DimensionKind::default(),
    )
}

#[test]
fn permission_expands_existing_pocket_without_replacing_items_and_is_idempotent() {
    let (mut world, inventory) = fixture();
    assert!(reconcile(&mut world, "Ordinary", &inventory)
        .unwrap()
        .is_none());
    let mut expanded = reconcile(&mut world, "Tester", &inventory)
        .unwrap()
        .unwrap();
    let original = &inventory.containers[0];
    let pocket = &expanded.containers[0];
    assert_eq!((pocket.rows, pocket.cols), (OP_POCKET_ROWS, OP_POCKET_COLS));
    assert_eq!(
        pocket.items, original.items,
        "OP 扩容必须保留物品身份和位置"
    );
    assert_eq!(pocket.id, original.id, "扩容复用既有容器，不能复制库存");
    assert!(
        rebuild_containers_from_equipment(&mut expanded, world.resource::<ItemRegistry>())
            .is_empty()
    );
    assert!(expanded.max_weight >= OP_CARRY_CAPACITY);
    assert!(
        reconcile(&mut world, "Tester", &expanded)
            .unwrap()
            .is_none(),
        "常规 tick 不应增加 revision 或重复发库存"
    );
    let restored: PlayerInventory =
        serde_json::from_str(&serde_json::to_string(&expanded).unwrap()).unwrap();
    assert!(reconcile(&mut world, "Tester", &restored)
        .unwrap()
        .is_none());
}

fn fill_operator_pocket(world: &World, inventory: &mut PlayerInventory) {
    inventory
        .containers
        .retain(|container| container.id == BODY_POCKET_CONTAINER_ID);
    inventory.equipped.clear();
    inventory.containers[0].items.clear();
    let template = world
        .resource::<ItemRegistry>()
        .get("spirit_grass")
        .unwrap();
    for row in 0..OP_POCKET_ROWS {
        for col in 0..OP_POCKET_COLS {
            inventory.containers[0].items.push(PlacedItemState {
                row,
                col,
                instance: runtime_instance_from_template(
                    template,
                    1000 + u64::from(row) * 12 + u64::from(col),
                    1,
                    0,
                ),
            });
        }
    }
}

#[test]
fn revoked_operator_reconnect_preserves_every_item_in_inventory_or_durable_drops() {
    let (mut world, inventory) = fixture();
    let mut expanded = reconcile(&mut world, "Tester", &inventory)
        .unwrap()
        .unwrap();
    fill_operator_pocket(&world, &mut expanded);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("bong-op-pocket-{}-{stamp}", std::process::id()));
    let db = directory.join("bong.db");
    crate::persistence::bootstrap_sqlite(&db, "op-pocket-test").unwrap();
    world.insert_resource(PlayerStatePersistence::with_db_path(&directory, &db));
    crate::player::state::load_player_state(world.resource::<PlayerStatePersistence>(), "Tester");
    world.insert_resource(DevCommandPermissions::allow_user("SomebodyElse"));
    let reduced = reconcile(&mut world, "Tester", &expanded).unwrap().unwrap();
    assert_eq!(
        (reduced.containers[0].rows, reduced.containers[0].cols),
        (BODY_POCKET_ROWS, BODY_POCKET_COLS)
    );
    assert_eq!(reduced.max_weight, BASE_CARRY_CAPACITY);
    let settings = crate::persistence::PersistenceSettings::with_db_path(&db, "op-pocket-test");
    let drops = crate::persistence::load_durable_dropped_loot(&settings).unwrap();
    assert_eq!(&drops, &world.resource::<DroppedLootRegistry>().entries);
    let mut ids: Vec<_> = reduced.containers[0]
        .items
        .iter()
        .map(|item| item.instance.instance_id)
        .chain(drops.keys().copied())
        .collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        (1000..1144).collect::<Vec<_>>(),
        "撤权不能复制或吞掉任何库存实例"
    );
    let saved = crate::player::state::load_player_slices(
        world.resource::<PlayerStatePersistence>(),
        "Tester",
    );
    assert_eq!(
        serde_json::to_value(saved.inventory.unwrap()).unwrap(),
        serde_json::to_value(&reduced).unwrap(),
        "缩容和掉落必须同一次持久化提交"
    );
    assert!(reconcile(&mut world, "Tester", &reduced).unwrap().is_none());
}

#[test]
fn revoke_fails_without_persistence_and_does_not_publish_partial_inventory_or_drops() {
    let (mut world, inventory) = fixture();
    let mut expanded = reconcile(&mut world, "Tester", &inventory)
        .unwrap()
        .unwrap();
    fill_operator_pocket(&world, &mut expanded);
    world.insert_resource(DevCommandPermissions::allow_user("SomebodyElse"));
    let before = expanded.clone();
    assert!(reconcile(&mut world, "Tester", &expanded).is_err());
    assert_eq!(
        serde_json::to_value(expanded).unwrap(),
        serde_json::to_value(before).unwrap()
    );
    assert!(world.resource::<DroppedLootRegistry>().entries.is_empty());
}
