#![allow(dead_code, unused_imports)]

use super::*;
use crate::combat::components::{Lifecycle, LifecycleState, RevivalDecision, TICKS_PER_SECOND};
use crate::cultivation::lifespan::LifespanCapTable;
use crate::inventory::{
    move_equipped_item_to_first_container_slot, set_item_instance_durability, ContainerState,
    DroppedLootEntry, InventoryRevision, ItemInstance, ItemRarity, PlayerInventory,
    EQUIP_SLOT_MAIN_HAND, MAIN_PACK_CONTAINER_ID,
};
use crate::network::agent_bridge::serialize_server_data_payload;
use crate::persistence::bootstrap_sqlite;
use crate::qi_physics::ledger::pending_inflow_account;
use crate::schema::server_data::{ServerDataPayloadV1, SERVER_DATA_VERSION};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn unique_temp_dir(test_name: &str) -> PathBuf {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "bong-player-state-{test_name}-{}-{unique_suffix}",
        std::process::id()
    ))
}

fn approx_eq(left: f64, right: f64) {
    assert!(
        (left - right).abs() < 1e-9,
        "expected {left} to be approximately equal to {right}"
    );
}

fn sqlite_persistence(test_name: &str) -> (PlayerStatePersistence, PathBuf) {
    let data_dir = unique_temp_dir(test_name);
    let db_path = data_dir.join("bong.db");
    bootstrap_sqlite(&db_path, &format!("player-state-{test_name}"))
        .expect("sqlite bootstrap should succeed");
    (
        PlayerStatePersistence::with_db_path(&data_dir, &db_path),
        data_dir,
    )
}

fn iron_sword_instance(instance_id: u64, durability: f64) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: "iron_sword".to_string(),
        display_name: "Iron Sword".to_string(),
        grid_w: 1,
        grid_h: 2,
        weight: 1.2,
        rarity: ItemRarity::Common,
        description: "weapon persistence fixture".to_string(),
        stack_count: 1,
        spirit_quality: 1.0,
        durability,
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

fn empty_weapon_inventory() -> PlayerInventory {
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(41),
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "Main Pack".to_string(),
            rows: 5,
            cols: 7,
            items: Vec::new(),
            owner_instance_id: None,
        }],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 17,
        max_weight: 45.0,
    }
}

/// 构造一个 v1 形态的 inventory JSON（每装备槽单件 object），仅含 equipped 段供 migrate 测试。
fn v1_inventory_json_with_equipped(equipped: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "revision": 1,
        "containers": [],
        "equipped": equipped,
        "hotbar": [null, null],
        "bone_coins": 0,
        "max_weight": 50.0
    })
}

fn v1_treasure_item(instance_id: u64, template: &str) -> serde_json::Value {
    serde_json::json!({
        "instance_id": instance_id,
        "template_id": template,
        "display_name": template,
        "grid_w": 1, "grid_h": 1, "weight": 0.2,
        "rarity": "Uncommon", "description": "", "stack_count": 1,
        "spirit_quality": 0.5, "durability": 1.0
    })
}

// plan-layered-equip-v1 P4（决议 #8）— 旧 treasure_belt_* 槽迁入触发位 triggered_treasures，
// 按 belt 槽序排列；不进装备槽 worn。
#[test]
fn migrate_v1_treasure_belt_lands_in_trigger_slots_in_order() {
    let mut value = v1_inventory_json_with_equipped(serde_json::json!({
        "treasure_belt_0": v1_treasure_item(10, "talisman_a"),
        "treasure_belt_2": v1_treasure_item(12, "talisman_c"),
        "treasure_belt_1": v1_treasure_item(11, "talisman_b"),
    }));

    migrate_equipped_v1_to_v2(&mut value);

    // 触发位顺序应按 belt_0,belt_1,belt_2（BTreeMap 槽名升序）。
    let triggered = value
        .get("triggered_treasures")
        .and_then(|v| v.as_array())
        .expect("triggered_treasures array present after migration");
    let ids: Vec<u64> = triggered
        .iter()
        .map(|item| item.get("instance_id").and_then(|v| v.as_u64()).unwrap())
        .collect();
    assert_eq!(
        ids,
        vec![10, 11, 12],
        "treasure_belt_0/1/2 should map to trigger slots in belt order"
    );

    // 装备结构里不应留 treasure_belt 槽（也不应进 worn）。
    let equipped = value.get("equipped").and_then(|v| v.as_object()).unwrap();
    assert!(
        !equipped.contains_key("treasure_belt_0")
            && !equipped.contains_key("treasure_belt_1")
            && !equipped.contains_key("treasure_belt_2"),
        "no treasure_belt slot key should survive migration"
    );
}

// 反序列化迁移产物为 PlayerInventory，确认 triggered_treasures 真正落进结构。
#[test]
fn migrate_v1_treasure_belt_deserializes_into_triggered_treasures_field() {
    let mut value = v1_inventory_json_with_equipped(serde_json::json!({
        "treasure_belt_0": v1_treasure_item(20, "talisman_x"),
    }));
    migrate_equipped_v1_to_v2(&mut value);

    let inventory: PlayerInventory =
        serde_json::from_value(value).expect("migrated v2 json deserializes");
    assert_eq!(inventory.triggered_treasures.len(), 1);
    assert_eq!(inventory.triggered_treasures[0].instance_id, 20);
    assert_eq!(inventory.triggered_treasures[0].template_id, "talisman_x");
}

// 无 treasure_belt 的旧档迁移后不应凭空生出 triggered_treasures 字段（serde default 空）。
#[test]
fn migrate_v1_without_treasure_belt_leaves_trigger_slot_empty() {
    let mut value = v1_inventory_json_with_equipped(serde_json::json!({
        "main_hand": v1_treasure_item(30, "iron_sword"),
    }));
    migrate_equipped_v1_to_v2(&mut value);
    assert!(
        value.get("triggered_treasures").is_none(),
        "no treasure_belt → migration must not inject triggered_treasures"
    );
    let inventory: PlayerInventory =
        serde_json::from_value(value).expect("deserializes with serde default empty trigger");
    assert!(inventory.triggered_treasures.is_empty());
}

/// 构造一个 v1 单件装备 object（带 instance_id），供 equipped 槽 / 容器 item 迁移测试复用。
fn v1_equip_item(instance_id: u64, template: &str) -> serde_json::Value {
    serde_json::json!({
        "instance_id": instance_id,
        "template_id": template,
        "display_name": template,
        "grid_w": 2, "grid_h": 2, "weight": 0.5,
        "rarity": "Common", "description": "", "stack_count": 1,
        "spirit_quality": 0.5, "durability": 0.5
    })
}

// Bug1（真机回归）— 旧 default.toml 形态：chest=fake_spirit_hide、main_hand=iron_sword、
// back_pack=worn_grass_pouch。迁移后 equipped 必须非空且正确：
// chest.worn == [worn_grass_pouch, fake_spirit_hide]（栈底背包件、栈顶伪皮，与 fresh 实例化一致），
// main_hand.held == iron_sword。绝不允许迁空 / 错置 / 把 equipped 件丢进容器。
#[test]
fn migrate_v1_legacy_default_loadout_keeps_equipped_correct() {
    let mut value = v1_inventory_json_with_equipped(serde_json::json!({
        "chest": v1_equip_item(1, "fake_spirit_hide"),
        "main_hand": v1_equip_item(2, "iron_sword"),
        "back_pack": v1_equip_item(3, "worn_grass_pouch"),
    }));
    migrate_equipped_v1_to_v2(&mut value);
    let inventory: PlayerInventory =
        serde_json::from_value(value).expect("migrated v2 json deserializes");

    let chest = inventory
        .equipped
        .get(crate::inventory::EQUIP_SLOT_CHEST)
        .expect("chest slot must exist after migration (equipped 不得迁空)");
    let chest_worn: Vec<&str> = chest.worn.iter().map(|i| i.template_id.as_str()).collect();
    assert_eq!(
        chest_worn,
        vec!["worn_grass_pouch", "fake_spirit_hide"],
        "迁移后 chest.worn 应为 [背包件, 伪皮]（栈底→栈顶），与 default.toml fresh 实例化一致；实际 {chest_worn:?}"
    );
    assert!(
        chest.held.is_none(),
        "身体槽 chest 不应有 held 件；实际 {:?}",
        chest.held.as_ref().map(|i| &i.template_id)
    );

    let main_hand = inventory
        .equipped
        .get(crate::inventory::EQUIP_SLOT_MAIN_HAND)
        .expect("main_hand slot must exist after migration");
    assert_eq!(
        main_hand.held.as_ref().map(|i| i.template_id.as_str()),
        Some("iron_sword"),
        "武器应迁入 main_hand.held（而非 worn / 容器）"
    );
    assert!(
        main_hand.worn.is_empty(),
        "手槽 main_hand 不应有 worn 件；实际 {:?}",
        main_hand.worn
    );

    // 不得残留旧背包专属槽 key。
    assert!(
        !inventory.equipped.contains_key("back_pack"),
        "旧 back_pack 装备槽 key 不应在 v2 equipped 中存活"
    );
}

// Bug3（真机回归）— 旧档背包件在 back_pack 装备槽，且有同名 `back_pack` 容器装着物品。
// 迁移后该容器必须改名到 pack_<背包件 instance_id>，否则 rebuild_containers_from_equipment
// 会新建空 pack_*、把旧 back_pack 容器留成无主孤儿（物品取不出）。
#[test]
fn migrate_v1_renames_legacy_backpack_container_to_pack_instance_namespace() {
    let mut value = serde_json::json!({
        "revision": 1,
        "containers": [
            {
                "id": "body_pocket", "name": "暗袋", "rows": 2, "cols": 3,
                "items": [{
                    "row": 0, "col": 0,
                    "instance": v1_equip_item(50, "fengling_bone_coin")
                }]
            },
            {
                "id": "back_pack", "name": "破草包", "rows": 3, "cols": 3,
                "items": [{
                    "row": 0, "col": 0,
                    "instance": v1_equip_item(51, "spirit_grass")
                }]
            }
        ],
        "equipped": {
            "back_pack": v1_equip_item(42, "worn_grass_pouch"),
        },
        "hotbar": [null, null],
        "bone_coins": 7,
        "max_weight": 23.0
    });
    migrate_equipped_v1_to_v2(&mut value);
    let inventory: PlayerInventory =
        serde_json::from_value(value).expect("migrated v2 json deserializes");

    // 背包件迁到 chest.worn。
    let chest = inventory
        .equipped
        .get(crate::inventory::EQUIP_SLOT_CHEST)
        .expect("chest slot present");
    assert_eq!(
        chest
            .worn
            .iter()
            .map(|i| i.template_id.as_str())
            .collect::<Vec<_>>(),
        vec!["worn_grass_pouch"],
        "worn_grass_pouch 应迁到 chest.worn"
    );
    let pack_instance_id = chest.worn[0].instance_id;
    assert_eq!(pack_instance_id, 42, "迁移保留原 instance_id");

    // 旧 back_pack 容器应改名到 pack_42，且内含物品原样保留。
    let expected_id = crate::inventory::container_id_for_worn_pack(pack_instance_id);
    let renamed = inventory
        .containers
        .iter()
        .find(|c| c.id == expected_id)
        .unwrap_or_else(|| {
            panic!(
                "应存在改名后的容器 `{expected_id}`；实际容器 ids = {:?}",
                inventory
                    .containers
                    .iter()
                    .map(|c| &c.id)
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(
        renamed.items.len(),
        1,
        "改名后容器内物品必须保留（不丢数据）"
    );
    assert_eq!(renamed.items[0].instance.template_id, "spirit_grass");

    // 旧 back_pack id 不应再存在（已被改名，不留孤儿）。
    assert!(
        !inventory.containers.iter().any(|c| c.id == "back_pack"),
        "旧 back_pack 容器 id 应已改名消失，不留无主孤儿"
    );
    // body_pocket 不动。
    assert!(
        inventory.containers.iter().any(|c| c.id == "body_pocket"),
        "body_pocket 容器应原样保留"
    );
}

/// 把任意 inventory_json 以指定 schema_version 落进 sqlite，再走 load_player_inventory_from_sqlite。
/// 复现真机 join → 加载链路（DEFAULT_INVENTORY_JSON / orphan-pack / 正常 v2 / v1 迁移分流全覆盖）。
fn load_inventory_row(
    schema_version: i32,
    inventory_json: &str,
) -> (Option<PlayerInventory>, PathBuf) {
    let (persistence, data_dir) = sqlite_persistence("load-inventory-row");
    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    connection
        .execute(
            "INSERT INTO inventories (username, inventory_json, schema_version, last_updated_wall)
             VALUES (?1, ?2, ?3, 0)",
            params!["LoadProbe", inventory_json, schema_version],
        )
        .expect("insert inventory row");
    let loaded = load_player_inventory_from_sqlite(&connection, "LoadProbe")
        .expect("load_player_inventory_from_sqlite should not error");
    (loaded, data_dir)
}

// Bug A（真机回归）— 真机污染存档：#736 旧迁移 bug 把伪皮冲进 body_pocket、清空 equipped、
// 丢 iron_sword/worn_grass_pouch，只剩孤儿 pack_<id> 容器，且已落盘为 schema_version=2。
// 这是 Kizun3Desu 实测行内 JSON（worn_grass_pouch instance_id=11 派生 pack_11，但 equipped 空）。
// 加载时必须识别为污染、丢弃存档、回落默认 loadout（返回 None），否则玩家 join 后 equipped 永久空。
#[test]
fn corrupt_v2_with_orphan_pack_container_is_discarded_to_default_loadout() {
    // 真机 Kizun3Desu v2 污染行的最小忠实复刻：equipped 空 + pack_11 孤儿容器 + body_pocket。
    let corrupt_v2 = serde_json::json!({
        "revision": 8,
        "containers": [
            {
                "id": "body_pocket", "name": "贴身口袋", "rows": 2, "cols": 3,
                "items": [
                    { "row": 0, "col": 0, "instance": v1_equip_item(2, "ningmai_powder") },
                    // 伪皮被旧迁移 bug 冲进 body_pocket（真机症状）。
                    { "row": 0, "col": 1, "instance": v1_equip_item(12, "fake_spirit_hide") }
                ]
            },
            {
                // 孤儿 pack_11：派生自 worn_grass_pouch(instance_id=11)，但 equipped 里已无该件。
                "id": "pack_11", "name": "破草包", "rows": 3, "cols": 3,
                "items": [
                    { "row": 0, "col": 0, "instance": v1_equip_item(4, "spirit_grass") }
                ]
            }
        ],
        "equipped": {},
        "hotbar": [null, null],
        "bone_coins": 7,
        "max_weight": 23.0,
        "triggered_treasures": []
    });
    let (loaded, data_dir) = load_inventory_row(2, &corrupt_v2.to_string());
    assert!(
        loaded.is_none(),
        "孤儿 pack_<id> 容器（equipped 无对应 worn 背包件）= #736 污染指纹，必须丢弃回落默认 loadout（返回 None），\
         否则 attach_player_state 会把空 equipped 存档插上、抑制默认 loadout，玩家 join 后 equipped 永久空；实际 loaded.is_some()={}",
        loaded.is_some()
    );
    let _ = fs::remove_dir_all(&data_dir);
}

// Bug A（防误伤）— 健康 v2 存档：equipped 有 chest.worn 背包件 + 与之自洽的 pack_<id> 容器。
// 这不是污染（容器有 backing worn 件），必须原样保留，绝不能被自愈逻辑误丢。
#[test]
fn healthy_v2_with_backed_pack_container_is_preserved() {
    let pack_id = crate::inventory::container_id_for_worn_pack(11);
    let healthy_v2 = serde_json::json!({
        "revision": 3,
        "containers": [
            { "id": "body_pocket", "name": "贴身口袋", "rows": 2, "cols": 3, "items": [] },
            {
                "id": pack_id, "name": "破草包", "rows": 3, "cols": 3,
                "items": [ { "row": 0, "col": 0, "instance": v1_equip_item(4, "spirit_grass") } ]
            }
        ],
        // worn_grass_pouch instance_id=11，与 pack_11 自洽 ⇒ 非孤儿。
        "equipped": {
            "chest": { "worn": [ v1_equip_item(11, "worn_grass_pouch") ], "held": null }
        },
        "hotbar": [null, null],
        "bone_coins": 7,
        "max_weight": 23.0,
        "triggered_treasures": []
    });
    let (loaded, data_dir) = load_inventory_row(2, &healthy_v2.to_string());
    let inventory =
        loaded.expect("健康 v2 存档（pack_<id> 有 backing worn 件）必须原样保留，不得被自愈误丢");
    let chest = inventory
        .equipped
        .get(crate::inventory::EQUIP_SLOT_CHEST)
        .expect("chest 槽应保留");
    assert_eq!(
        chest.worn.iter().map(|i| i.instance_id).collect::<Vec<_>>(),
        vec![11],
        "chest.worn 背包件 instance_id 应原样保留"
    );
    assert!(
        inventory.containers.iter().any(|c| c.id == pack_id),
        "自洽 pack_<id> 容器应原样保留"
    );
    let _ = fs::remove_dir_all(&data_dir);
}

// Bug A（防误伤）— 合法裸装玩家：equipped 空且无任何 pack_<id> 容器（卸背包时容器随之清掉）。
// 不是污染（没有孤儿容器），必须原样保留空 equipped，不能被误判为 #736 污染而重置。
#[test]
fn naked_v2_without_pack_container_is_preserved_not_reset() {
    let naked_v2 = serde_json::json!({
        "revision": 5,
        "containers": [
            { "id": "body_pocket", "name": "贴身口袋", "rows": 2, "cols": 3, "items": [] }
        ],
        "equipped": {},
        "hotbar": [null, null],
        "bone_coins": 0,
        "max_weight": 23.0,
        "triggered_treasures": []
    });
    let (loaded, data_dir) = load_inventory_row(2, &naked_v2.to_string());
    let inventory = loaded.expect("合法裸装存档（无 pack_<id> 容器）必须保留，不得误判为污染重置");
    assert!(
        inventory.equipped.is_empty(),
        "裸装玩家 equipped 应保持空（保留其存档原貌），实际 {:?}",
        inventory.equipped.keys().collect::<Vec<_>>()
    );
    let _ = fs::remove_dir_all(&data_dir);
}

// plan-tarkov-backpack-v1 P0（交付物 #6 / 测试清单）— 旧存档（无 owner_instance_id 字段）
// 加载后，`pack_<id>` 容器的 owner_instance_id 应由前缀解析回填，且回填发生在孤儿检测前
// （断言旧 pack_<id> 容器未被误删 + owner 正确）。
#[test]
fn load_backfills_owner_instance_id_for_legacy_pack_container() {
    let pack_id = crate::inventory::container_id_for_worn_pack(11);
    // 旧格式：containers 里 pack_11 无 owner_instance_id 字段（serde default → None）。
    // equipped 有自洽 worn 件 ⇒ 非孤儿，应保留。
    let legacy_v2 = serde_json::json!({
        "revision": 3,
        "containers": [
            { "id": "body_pocket", "name": "贴身口袋", "rows": 2, "cols": 3, "items": [] },
            {
                "id": pack_id, "name": "破草包", "rows": 3, "cols": 3,
                "items": [ { "row": 0, "col": 0, "instance": v1_equip_item(4, "spirit_grass") } ]
            }
        ],
        "equipped": {
            "chest": { "worn": [ v1_equip_item(11, "worn_grass_pouch") ], "held": null }
        },
        "hotbar": [null, null],
        "bone_coins": 7,
        "max_weight": 23.0,
        "triggered_treasures": []
    });
    let (loaded, data_dir) = load_inventory_row(2, &legacy_v2.to_string());
    let inventory = loaded.expect(
        "旧存档（pack_<id> 有 backing worn 件、无 owner 字段）加载后必须保留——\
         owner_instance_id 已回填，孤儿检测在回填后运行不会误判",
    );
    let pack = inventory
        .containers
        .iter()
        .find(|c| c.id == pack_id)
        .expect("旧 pack_<id> 容器必须未被误删（回填先于孤儿检测）");
    assert_eq!(
        pack.owner_instance_id,
        Some(11),
        "因为 backfill_owner_instance_ids 必须按 `pack_<id>` 前缀把 owner_instance_id 回填为 11，\
         实际 = {:?}",
        pack.owner_instance_id
    );
    // body_pocket（非 pack 容器）不应被回填。
    let body = inventory
        .containers
        .iter()
        .find(|c| c.id == "body_pocket")
        .expect("body_pocket 应保留");
    assert_eq!(
        body.owner_instance_id, None,
        "非 pack 容器（body_pocket）不应被回填 owner_instance_id"
    );
    let _ = fs::remove_dir_all(&data_dir);
}

// plan-tarkov-backpack-v1 P0（交付物 #7 / 测试清单）— 孤儿检测在回填后运行：
// 合法新格式容器（owner 已回填、与 equipped worn 件自洽）不被误判孤儿、不被丢弃。
#[test]
fn orphan_detection_runs_after_backfill_no_false_positive() {
    // 直测：手工构造一个旧格式 inventory（pack_<id> owner=None 但有 backing worn 件），
    // 先回填、再孤儿检测——回填后判定为合法（前缀路径与 owner 路径一致），不应误判孤儿。
    let pack_id = crate::inventory::container_id_for_worn_pack(77);
    let mut inventory = PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: crate::inventory::InventoryRevision(1),
        containers: vec![
            crate::inventory::ContainerState {
                quick_access: false,
                id: "body_pocket".to_string(),
                name: "贴身口袋".to_string(),
                rows: 2,
                cols: 3,
                items: Vec::new(),
                owner_instance_id: None,
            },
            crate::inventory::ContainerState {
                quick_access: false,
                id: pack_id.clone(),
                name: "破草包".to_string(),
                rows: 3,
                cols: 3,
                items: Vec::new(),
                // 旧格式：owner 字段缺省。
                owner_instance_id: None,
            },
        ],
        equipped: {
            let mut e = std::collections::HashMap::new();
            // worn 件 instance_id=77 与 pack_77 自洽（template 不影响孤儿判定，仅看 instance_id）。
            e.insert(
                crate::inventory::EQUIP_SLOT_CHEST.to_string(),
                crate::inventory::SlotContents::worn_single(iron_sword_instance(77, 1.0)),
            );
            e
        },
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 23.0,
    };

    // 回填前：pack_77 owner=None，但前缀解析可得 77（已有 backing worn 件 77）。
    backfill_owner_instance_ids(&mut inventory);
    assert_eq!(
        inventory
            .containers
            .iter()
            .find(|c| c.id == pack_id)
            .and_then(|c| c.owner_instance_id),
        Some(77),
        "回填后 pack_77 owner 应为 77"
    );
    // 回填后孤儿检测：pack_77 有 backing worn 件 77 ⇒ 合法、非孤儿。
    assert!(
        !inventory_has_orphan_pack_container(&inventory),
        "回填后合法新格式容器（owner 与 worn 件自洽）绝不应被误判孤儿（防 #736 污染误删）"
    );
}

// ─── plan-tarkov-backpack-v1 套包修复 §6 — orphan 检测扩到「任意位置」（防 #736 复发）───

/// 构造一个仅含指定 containers + equipped + hotbar 的最小 inventory（孤儿检测只看 instance_id）。
fn orphan_test_inventory(
    containers: Vec<crate::inventory::ContainerState>,
    equipped: std::collections::HashMap<String, crate::inventory::SlotContents>,
    hotbar: [Option<ItemInstance>; crate::schema::inventory::HOTBAR_SLOT_COUNT],
) -> PlayerInventory {
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: crate::inventory::InventoryRevision(1),
        containers,
        equipped,
        hotbar,
        bone_coins: 0,
        max_weight: 23.0,
    }
}

fn pack_container(instance_id: u64) -> crate::inventory::ContainerState {
    crate::inventory::ContainerState {
        quick_access: false,
        id: crate::inventory::container_id_for_worn_pack(instance_id),
        name: "包".to_string(),
        rows: 3,
        cols: 3,
        items: Vec::new(),
        owner_instance_id: Some(instance_id),
    }
}

#[test]
fn orphan_detection_false_for_pack_in_body_pocket() {
    // 背包件躺在 body_pocket 容器内（合法 retention），其 pack_<id> 容器不应误判孤儿。
    let bp = crate::inventory::ContainerState {
        quick_access: false,
        id: "body_pocket".to_string(),
        name: "暗袋".to_string(),
        rows: 2,
        cols: 3,
        items: vec![crate::inventory::PlacedItemState {
            row: 0,
            col: 0,
            instance: iron_sword_instance(55, 1.0),
        }],
        owner_instance_id: None,
    };
    let inventory = orphan_test_inventory(
        vec![bp, pack_container(55)],
        std::collections::HashMap::new(),
        Default::default(),
    );
    assert!(
        !inventory_has_orphan_pack_container(&inventory),
        "背包件在 body_pocket（任意位置存活判据）时 pack_55 不应被判孤儿（防 #736 误删存档）"
    );
}

#[test]
fn orphan_detection_false_for_pack_in_hotbar() {
    let mut hotbar: [Option<ItemInstance>; crate::schema::inventory::HOTBAR_SLOT_COUNT] =
        Default::default();
    hotbar[1] = Some(iron_sword_instance(66, 1.0));
    let inventory = orphan_test_inventory(
        vec![pack_container(66)],
        std::collections::HashMap::new(),
        hotbar,
    );
    assert!(
        !inventory_has_orphan_pack_container(&inventory),
        "背包件在 hotbar 时 pack_66 不应被判孤儿"
    );
}

#[test]
fn orphan_detection_false_for_pack_held() {
    let mut equipped = std::collections::HashMap::new();
    equipped.insert(
        crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
        crate::inventory::SlotContents {
            worn: Vec::new(),
            held: Some(iron_sword_instance(77, 1.0)),
        },
    );
    let inventory = orphan_test_inventory(vec![pack_container(77)], equipped, Default::default());
    assert!(
        !inventory_has_orphan_pack_container(&inventory),
        "背包件 held 时 pack_77 不应被判孤儿"
    );
}

#[test]
fn orphan_detection_true_for_truly_orphan_container() {
    // pack_<id> 容器但该 instance 全 inventory 任意位置查无 ⇒ 真孤儿 ⇒ #736 污染指纹。
    let inventory = orphan_test_inventory(
        vec![pack_container(99)],
        std::collections::HashMap::new(),
        Default::default(),
    );
    assert!(
        inventory_has_orphan_pack_container(&inventory),
        "无任何 backing 背包件的 pack_99 容器应判为真孤儿（保留对真污染的检测）"
    );
}

#[test]
fn orphan_detection_true_for_pack_container_when_owner_nested_in_pack_grid() {
    // P5「2 层封顶」镜像：背包件 2 仅作为 host pack 的 grid 内货物时不属携带面，rebuild
    // 永不为它建 pack_2 容器；若存档残留 pack_2 容器，则它确为孤儿（与 rebuild 镜像一致）。
    let host = crate::inventory::ContainerState {
        quick_access: false,
        id: crate::inventory::container_id_for_worn_pack(1),
        name: "host".to_string(),
        rows: 4,
        cols: 4,
        items: vec![crate::inventory::PlacedItemState {
            row: 0,
            col: 0,
            instance: iron_sword_instance(2, 1.0),
        }],
        owner_instance_id: Some(1),
    };
    let mut equipped = std::collections::HashMap::new();
    equipped.insert(
        crate::inventory::EQUIP_SLOT_CHEST.to_string(),
        crate::inventory::SlotContents::worn_single(iron_sword_instance(1, 1.0)),
    );
    let inventory =
        orphan_test_inventory(vec![host, pack_container(2)], equipped, Default::default());
    assert!(
        inventory_has_orphan_pack_container(&inventory),
        "grid 内货物背包件 2 不属携带面（2 层封顶），其残留 pack_2 容器应判为孤儿（与 rebuild 镜像）"
    );
}

// Bug A（真机回归核心）— 真机 v1 旧档（旧 default.toml 形态：chest=fake_spirit_hide、
// main_hand=iron_sword、back_pack=worn_grass_pouch + 同名 back_pack 容器装 7 件），
// 走完整 sqlite 加载链路（schema_version=1 → migrate → 反序列化）。
// 必须：equipped 非空 + chest.worn==[worn_grass_pouch, fake_spirit_hide] + main_hand.held==iron_sword
// + back_pack 容器改名到 pack_<worn_grass_pouch instance_id> 且 7 件原样保留 + body_pocket 不动。
// 这把真机 join 加载路径整条锁死，任何回归（迁空 / 错置 / 丢件 / 孤儿容器）立即撞红。
#[test]
fn real_v1_legacy_loadout_loads_with_equipped_populated_via_full_path() {
    let v1_row = serde_json::json!({
        "revision": 1,
        "containers": [
            {
                "id": "body_pocket", "name": "贴身口袋", "rows": 2, "cols": 3,
                "items": [
                    { "row": 0, "col": 0, "instance": v1_equip_item(2, "ningmai_powder") },
                    { "row": 0, "col": 1, "instance": v1_equip_item(3, "fengling_bone_coin") }
                ]
            },
            {
                "id": "back_pack", "name": "破草包", "rows": 3, "cols": 3,
                "items": [
                    { "row": 0, "col": 0, "instance": v1_equip_item(4, "spirit_grass") },
                    { "row": 0, "col": 1, "instance": v1_equip_item(5, "ningmai_powder") },
                    { "row": 0, "col": 2, "instance": v1_equip_item(6, "guyuan_pill") },
                    { "row": 1, "col": 0, "instance": v1_equip_item(7, "bone_spike") },
                    { "row": 1, "col": 1, "instance": v1_equip_item(8, "ash_spider_silk") },
                    { "row": 2, "col": 1, "instance": v1_equip_item(9, "ci_she_hao_seed") },
                    { "row": 2, "col": 2, "instance": v1_equip_item(10, "ning_mai_cao_seed") }
                ]
            }
        ],
        "equipped": {
            "chest": v1_equip_item(11, "fake_spirit_hide"),
            "main_hand": v1_equip_item(12, "iron_sword"),
            "back_pack": v1_equip_item(13, "worn_grass_pouch")
        },
        "hotbar": [null, null],
        "bone_coins": 7,
        "max_weight": 23.0
    });
    let (loaded, data_dir) = load_inventory_row(1, &v1_row.to_string());
    let inventory = loaded.expect("v1 旧档加载后 inventory 必须存在（不得迁空、不得误判污染）");

    assert!(
        !inventory.equipped.is_empty(),
        "真机 join 加载后 equipped 绝不能为空（Bug A 核心症状）"
    );
    let chest = inventory
        .equipped
        .get(crate::inventory::EQUIP_SLOT_CHEST)
        .expect("chest 槽必须存在");
    assert_eq!(
        chest
            .worn
            .iter()
            .map(|i| i.template_id.as_str())
            .collect::<Vec<_>>(),
        vec!["worn_grass_pouch", "fake_spirit_hide"],
        "chest.worn 应为 [背包件, 伪皮]（栈底→栈顶），与 default.toml fresh 实例化一致"
    );
    let main_hand = inventory
        .equipped
        .get(crate::inventory::EQUIP_SLOT_MAIN_HAND)
        .expect("main_hand 槽必须存在（iron_sword 不得丢失）");
    assert_eq!(
        main_hand.held.as_ref().map(|i| i.template_id.as_str()),
        Some("iron_sword"),
        "iron_sword 必须迁入 main_hand.held（真机数据丢失尤其严重，必锁死）"
    );

    // back_pack 容器改名到 pack_<worn_grass_pouch instance_id=13>，7 件原样保留。
    let expected_pack_id = crate::inventory::container_id_for_worn_pack(13);
    let pack = inventory
        .containers
        .iter()
        .find(|c| c.id == expected_pack_id)
        .unwrap_or_else(|| {
            panic!(
                "back_pack 容器应改名到 `{expected_pack_id}`；实际容器 ids = {:?}",
                inventory
                    .containers
                    .iter()
                    .map(|c| &c.id)
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(
        pack.items.len(),
        7,
        "改名后 pack 容器内 7 件原样保留（不丢数据）"
    );
    assert!(
        !inventory.containers.iter().any(|c| c.id == "back_pack"),
        "旧 back_pack 容器 id 不应残留（已改名，否则成无主孤儿 = 取不出）"
    );
    assert!(
        inventory.containers.iter().any(|c| c.id == "body_pocket"),
        "body_pocket 容器应原样保留"
    );
    // 关键：加载产物自身不得触发 orphan 判定（自洽，pack_13 有 backing worn 件）。
    assert!(
        !inventory_has_orphan_pack_container(&inventory),
        "v1 迁移产物必须自洽：pack_<id> 容器与 chest.worn 背包件 instance_id 对齐，不得被误判孤儿"
    );
    let _ = fs::remove_dir_all(&data_dir);
}

// Bug A（fresh join 路径）— 全新玩家（无 sqlite 行）应回落默认 loadout（instantiate_inventory_from_loadout），
// equipped 正确填充：chest.worn==[worn_grass_pouch, fake_spirit_hide]、main_hand.held==iron_sword。
// 这把 default.toml → try_into_loadout → instantiate 的 fresh 实例化结构锁死。
#[test]
fn fresh_instantiate_from_default_loadout_populates_equipped() {
    use crate::inventory::{
        instantiate_inventory_from_loadout, load_default_loadout, load_item_registry,
        InventoryInstanceIdAllocator,
    };
    // 真机 fresh join 路径：真实 ItemRegistry（assets/items）+ 真实 default.toml → instantiate。
    let registry = load_item_registry().expect("load item registry from assets/items");
    let loadout = load_default_loadout(&registry).expect("default loadout should load");
    let mut allocator = InventoryInstanceIdAllocator::default();
    let inventory = instantiate_inventory_from_loadout(&loadout, &mut allocator, &registry)
        .expect("instantiate default loadout should succeed");

    assert!(
        !inventory.equipped.is_empty(),
        "fresh 实例化后 equipped 绝不能为空"
    );
    let chest = inventory
        .equipped
        .get(crate::inventory::EQUIP_SLOT_CHEST)
        .expect("chest 槽必须存在");
    assert_eq!(
        chest
            .worn
            .iter()
            .map(|i| i.template_id.as_str())
            .collect::<Vec<_>>(),
        vec!["worn_grass_pouch", "fake_spirit_hide"],
        "fresh chest.worn 应为 [破草包, 伪皮]（两条 [[equip]] slot=chest 聚合到 worn 栈）"
    );
    let main_hand = inventory
        .equipped
        .get(crate::inventory::EQUIP_SLOT_MAIN_HAND)
        .expect("main_hand 槽必须存在");
    assert_eq!(
        main_hand.held.as_ref().map(|i| i.template_id.as_str()),
        Some("iron_sword"),
        "fresh main_hand.held 应为 iron_sword（[[equip]] slot=main_hand）"
    );
    assert!(
        !inventory_has_orphan_pack_container(&inventory),
        "fresh 实例化产物自洽：pack_<id> 与 chest.worn 背包件对齐，不得被误判孤儿"
    );
}

fn persist_player_with_inventory(
    persistence: &PlayerStatePersistence,
    username: &str,
    inventory: &PlayerInventory,
) {
    save_player_slices(
        persistence,
        username,
        &PlayerState::default(),
        [11.0, 70.0, -2.0],
        DimensionKind::default(),
        Some(inventory),
        None,
        &SkillSet::default(),
    )
    .expect("player slices with inventory should persist");
}

#[test]
fn loads_and_saves_player_state_in_sqlite() {
    let (persistence, data_dir) = sqlite_persistence("sqlite-load-save");
    let autosave_interval_ticks = 60 * TICKS_PER_SECOND;

    let persisted = PlayerState {
        karma: 0.2,
        inventory_score: 0.4,
    };

    let save_path = save_player_state(&persistence, "Azure", &persisted)
        .expect("saving PlayerState should succeed");
    let reloaded = load_player_state(&persistence, "Azure");
    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    let current_char_id: String = connection
        .query_row(
            "SELECT current_char_id FROM player_core WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .expect("player_core row should exist");
    let (pos_x, pos_y, pos_z): (f64, f64, f64) = connection
        .query_row(
            "SELECT pos_x, pos_y, pos_z FROM player_slow WHERE username = ?1",
            params!["Azure"],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("player_slow row should exist");
    let inventory_json: String = connection
        .query_row(
            "SELECT inventory_json FROM inventories WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .expect("inventories row should exist");
    let prefs_json: String = connection
        .query_row(
            "SELECT prefs_json FROM player_ui_prefs WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .expect("player_ui_prefs row should exist");
    let prefs: PlayerUiPrefs = serde_json::from_str(&prefs_json).expect("prefs_json should decode");
    let current_char_uuid =
        Uuid::parse_str(&current_char_id).expect("current_char_id should be a UUID");
    let [spawn_x, spawn_y, spawn_z] =
        crate::player::spawn_position_for_seed("Azure", SpawnPurpose::InitialLogin);

    assert_eq!(save_path, persistence.db_path().to_path_buf());
    assert_eq!(reloaded, persisted.normalized());
    assert_eq!(autosave_interval_ticks, 1_200);
    assert_eq!(current_char_uuid.get_version_num(), 7);
    assert_eq!((pos_x, pos_y, pos_z), (spawn_x, spawn_y, spawn_z));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&inventory_json)
            .expect("inventory_json should decode"),
        serde_json::Value::Null
    );
    assert_eq!(prefs, PlayerUiPrefs::default());

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn craft_checkpoint_rolls_back_every_slice_when_durable_drop_write_fails() {
    let (persistence, data_dir) = sqlite_persistence("craft-checkpoint-rollback");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("test player should initialize");
    let baseline_inventory = empty_weapon_inventory();
    save_player_inventory_and_craft_session_slices(
        &persistence,
        "Azure",
        Some(&baseline_inventory),
        None,
    )
    .expect("baseline inventory should persist");
    let connection = Connection::open(persistence.db_path()).expect("sqlite should open");
    connection
        .execute_batch(
            "
            CREATE TRIGGER fail_craft_drop_insert
            BEFORE INSERT ON dropped_loot
            BEGIN
                SELECT RAISE(FAIL, 'forced durable drop failure');
            END;
            ",
        )
        .expect("failure trigger should install");

    let mut staged_inventory = baseline_inventory.clone();
    staged_inventory.revision = InventoryRevision(42);
    let session = CraftSession {
        recipe_id: crate::craft::RecipeId::new("craft.test.rollback"),
        started_at_tick: 10,
        remaining_ticks: 30,
        total_ticks: 40,
        owner_player_id: canonical_player_id("Azure"),
        qi_paid: 3.0,
        quantity_total: 1,
        completed_count: 0,
    };
    let cultivation = Cultivation {
        qi_current: 7.0,
        ..Default::default()
    };
    let mut qi_ledger = WorldQiAccount::default();
    qi_ledger
        .set_balance(pending_inflow_account(), 3.0)
        .expect("pending inflow fixture should be valid");
    let drop = DroppedLootEntry {
        instance_id: 9_000,
        source_container_id: "craft_refund".to_string(),
        source_row: 0,
        source_col: 0,
        world_pos: [1.0, 64.0, 2.0],
        dimension: DimensionKind::Overworld,
        item: iron_sword_instance(9_000, 1.0),
    };

    let error = save_player_craft_checkpoint(
        &persistence,
        "Azure",
        Some(&staged_inventory),
        Some(&session),
        Some(&cultivation),
        Some(&qi_ledger),
        &[drop],
    )
    .expect_err("last transaction step must fail through the trigger");
    assert!(
        error.to_string().contains("forced durable drop failure"),
        "failure should retain the injected repair hint, actual={error}"
    );

    let reloaded = load_player_slices(&persistence, "Azure");
    assert_eq!(
        reloaded.inventory.as_ref().map(|value| value.revision),
        Some(InventoryRevision(41)),
        "failed checkpoint must roll inventory back to the complete pre-state"
    );
    assert!(
        reloaded.craft_session.is_none(),
        "failed checkpoint must not leave an active session beside the old inventory"
    );
    let settings = crate::persistence::PersistenceSettings::with_db_path(
        persistence.db_path(),
        "craft-checkpoint-rollback",
    );
    assert!(
        crate::persistence::load_player_cultivation_bundle(&settings, "Azure")
            .expect("cultivation lookup should succeed")
            .is_none(),
        "failed checkpoint must roll back cultivation upsert"
    );
    assert!(
        crate::persistence::load_durable_dropped_loot(&settings)
            .expect("durable drop lookup should succeed")
            .is_empty(),
        "failed checkpoint must not leak a partially durable drop"
    );
    assert_eq!(
        crate::persistence::load_pending_inflow_balance(&settings)
            .expect("pending inflow lookup should succeed"),
        0.0,
        "failed checkpoint must roll back the durable qi receiver"
    );
    std::fs::remove_dir_all(data_dir).ok();
}

#[test]
fn durable_craft_drop_roundtrips_seeds_allocator_and_stays_deleted_after_pickup() {
    let (persistence, data_dir) = sqlite_persistence("durable-craft-drop-roundtrip");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("test player should initialize");
    let inventory = empty_weapon_inventory();
    let drop = DroppedLootEntry {
        instance_id: 9_100,
        source_container_id: "craft_refund".to_string(),
        source_row: 0,
        source_col: 0,
        world_pos: [3.0, 65.0, 4.0],
        dimension: DimensionKind::Overworld,
        item: iron_sword_instance(9_100, 1.0),
    };
    save_player_craft_checkpoint(
        &persistence,
        "Azure",
        Some(&inventory),
        None,
        None,
        None,
        std::slice::from_ref(&drop),
    )
    .expect("craft drop should commit with terminal inventory");
    let settings = crate::persistence::PersistenceSettings::with_db_path(
        persistence.db_path(),
        "durable-craft-drop-roundtrip",
    );
    let first_hydrate = crate::persistence::load_durable_dropped_loot(&settings)
        .expect("first restart hydrate should succeed");
    let second_hydrate = crate::persistence::load_durable_dropped_loot(&settings)
        .expect("repeated restart hydrate should be idempotent");
    assert_eq!(first_hydrate, second_hydrate);
    assert_eq!(first_hydrate.get(&9_100), Some(&drop));
    assert_eq!(
        crate::persistence::persisted_inventory_instance_id_high_water(&settings)
            .expect("allocator high-water scan should succeed"),
        Some(9_100),
        "durable ground IDs must advance the post-restart allocator"
    );

    let mut picked_inventory = inventory.clone();
    picked_inventory.revision = InventoryRevision(42);
    save_player_inventory_and_delete_dropped_loot(
        &persistence,
        "Azure",
        &picked_inventory,
        9_100,
        None,
    )
    .expect("pickup inventory + durable delete should commit atomically");
    assert!(
        crate::persistence::load_durable_dropped_loot(&settings)
            .expect("post-pickup restart hydrate should succeed")
            .is_empty(),
        "a committed pickup must not resurrect its durable ground row"
    );
    assert_eq!(
        load_player_slices(&persistence, "Azure")
            .inventory
            .map(|value| value.revision),
        Some(InventoryRevision(42)),
        "the same pickup transaction must retain the receiving inventory"
    );
    std::fs::remove_dir_all(data_dir).ok();
}

#[test]
fn pickup_checkpoint_rolls_back_inventory_drop_and_zone_together() {
    let (persistence, data_dir) = sqlite_persistence("pickup-checkpoint-zone-rollback");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("test player should initialize");
    let baseline_inventory = empty_weapon_inventory();
    let drop = DroppedLootEntry {
        instance_id: 9_101,
        source_container_id: "craft_refund".to_string(),
        source_row: 0,
        source_col: 0,
        world_pos: [3.0, 65.0, 4.0],
        dimension: DimensionKind::Overworld,
        item: iron_sword_instance(9_101, 1.0),
    };
    save_player_craft_checkpoint(
        &persistence,
        "Azure",
        Some(&baseline_inventory),
        None,
        None,
        None,
        std::slice::from_ref(&drop),
    )
    .expect("baseline inventory and drop should persist");
    let connection = Connection::open(persistence.db_path()).expect("sqlite should open");
    connection
        .execute_batch(
            "
            CREATE TRIGGER fail_pickup_zone_update
            BEFORE INSERT ON zones_runtime
            BEGIN
                SELECT RAISE(FAIL, 'forced pickup zone failure');
            END;
            ",
        )
        .expect("failure trigger should install");

    let mut picked_inventory = baseline_inventory.clone();
    picked_inventory.revision = InventoryRevision(42);
    let zone_runtime = ZoneRuntimeRecord {
        zone_id: "spawn".to_string(),
        spirit_qi: 7.5,
        danger_level: 1,
    };
    let error = save_player_inventory_and_delete_dropped_loot(
        &persistence,
        "Azure",
        &picked_inventory,
        9_101,
        Some(&zone_runtime),
    )
    .expect_err("zone write failure must abort the entire pickup transaction");
    assert!(
        error.to_string().contains("forced pickup zone failure"),
        "failure should retain the injected repair hint, actual={error}"
    );

    let settings = crate::persistence::PersistenceSettings::with_db_path(
        persistence.db_path(),
        "pickup-checkpoint-zone-rollback",
    );
    assert_eq!(
        load_player_slices(&persistence, "Azure")
            .inventory
            .map(|value| value.revision),
        Some(InventoryRevision(41)),
        "failed pickup checkpoint must not commit the receiving inventory"
    );
    assert_eq!(
        crate::persistence::load_durable_dropped_loot(&settings)
            .expect("durable drop lookup should succeed")
            .get(&9_101),
        Some(&drop),
        "failed pickup checkpoint must retain the durable ground row for retry"
    );
    let zone_rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM zones_runtime", [], |row| row.get(0))
        .expect("zone row count should be readable");
    assert_eq!(
        zone_rows, 0,
        "failed pickup checkpoint must not leak a partial zone update"
    );

    std::fs::remove_dir_all(data_dir).ok();
}

#[test]
fn serializes_player_state_payload() {
    let state = PlayerState {
        karma: 0.2,
        inventory_score: 0.4,
    };

    let cultivation = Cultivation {
        realm: Realm::Induce,
        qi_current: 78.0,
        // qi_max≠qi_current 且≠100 fallback：锁住 HUD 真元条分母 = 真实 qi_max（非 current、非 100）。
        qi_max: 150.0,
        ..Cultivation::default()
    };

    let payload = state.server_payload_with_social_and_local_pressure(
        &cultivation,
        Some(canonical_player_id("Steve")),
        "blood_valley",
        None,
        None,
        None,
    );
    let bytes =
        serialize_server_data_payload(&payload).expect("PlayerState payload should serialize");
    let json: serde_json::Value =
        serde_json::from_slice(&bytes).expect("serialized payload should decode as JSON value");

    assert_eq!(json.get("v"), Some(&serde_json::json!(SERVER_DATA_VERSION)));
    assert_eq!(json.get("type"), Some(&serde_json::json!("player_state")));
    assert_eq!(
        json.get("player"),
        Some(&serde_json::json!("offline:Steve"))
    );
    assert_eq!(json.get("realm"), Some(&serde_json::json!("Induce")));
    assert_eq!(json.get("spirit_qi"), Some(&serde_json::json!(78.0)));
    // P0 HUD fix：下发真元上限，client 才能算正确分母；缺失 payload 会被拒收。
    assert_eq!(json.get("spirit_qi_max"), Some(&serde_json::json!(150.0)));
    assert_eq!(json.get("karma"), Some(&serde_json::json!(0.2)));
    assert_eq!(json.get("zone"), Some(&serde_json::json!("blood_valley")));

    match payload.payload {
        ServerDataPayloadV1::PlayerState {
            spirit_qi_max,
            composite_power,
            breakdown,
            ..
        } => {
            approx_eq(spirit_qi_max, cultivation.qi_max);
            approx_eq(composite_power, state.composite_power(&cultivation));
            approx_eq(breakdown.combat, state.power_breakdown(&cultivation).combat);
            approx_eq(breakdown.wealth, state.power_breakdown(&cultivation).wealth);
            approx_eq(breakdown.social, state.power_breakdown(&cultivation).social);
            approx_eq(breakdown.karma, state.power_breakdown(&cultivation).karma);
            approx_eq(
                breakdown.territory,
                state.power_breakdown(&cultivation).territory,
            );
        }
        other => panic!("expected PlayerState payload, got {other:?}"),
    }
}

#[test]
fn persisted_login_y_above_runtime_world_falls_back_to_spawn() {
    let (persistence, data_dir) = sqlite_persistence("too-high-login-y");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("saving PlayerState should succeed");
    save_player_slow_slice(
        &persistence,
        "Azure",
        [42.0, MAX_SAFE_PLAYER_Y + 1.0, -3.5],
        DimensionKind::default(),
    )
    .expect("saving too-high slow slice should succeed");

    let loaded = load_player_slices(&persistence, "Azure");

    assert_eq!(
        loaded.position,
        crate::player::spawn_position_for_seed("Azure", SpawnPurpose::InitialLogin)
    );
    assert_eq!(loaded.last_dimension, DimensionKind::default());

    let _ = fs::remove_dir_all(&data_dir);
}

fn sample_lifecycle_awaiting_revival_zero_fortune() -> Lifecycle {
    Lifecycle {
        character_id: "offline:Azure:char-1".to_string(),
        death_count: 4,
        fortune_remaining: 0,
        last_death_tick: Some(1_000),
        last_revive_tick: Some(500),
        spawn_anchor: Some([9.0, 64.0, -3.0]),
        spawn_anchor_damaged: true,
        awaiting_decision: Some(RevivalDecision::Tribulation { chance: 0.2 }),
        revival_decision_deadline_tick: Some(1_600),
        revival_roll_survived: Some(false),
        weakened_until_tick: None,
        state: LifecycleState::AwaitingRevival,
    }
}

#[test]
fn player_lifecycle_slice_roundtrips_awaiting_revival_with_zero_fortune() {
    // 核心回归锁：断线时 fortune_remaining=0 且有一个待决策的 Tribulation（可能永久
    // 终结角色），重连后必须原样读回同一状态，而不是被写/读路径的任何一端悄悄
    // 补回默认值 3 / Alive。
    let (persistence, data_dir) = sqlite_persistence("lifecycle-roundtrip-awaiting");
    let lifecycle = sample_lifecycle_awaiting_revival_zero_fortune();

    save_player_lifecycle_slice(&persistence, "Azure", &lifecycle, 0)
        .expect("lifecycle slice should persist");
    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    let persisted_last_updated_wall: i64 = connection
        .query_row(
            "SELECT last_updated_wall FROM player_lifecycle WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .expect("saved lifecycle row should expose its persistence timestamp");
    let loaded = load_player_lifecycle_slice(&persistence, "Azure", 0)
        .expect("lifecycle slice should load")
        .expect("lifecycle row should exist after save");
    let after_load_wall = current_unix_seconds();
    let max_elapsed_ticks = after_load_wall
        .saturating_sub(persisted_last_updated_wall)
        .max(0) as u64
        * crate::combat::components::TICKS_PER_SECOND;

    assert_eq!(loaded.character_id, lifecycle.character_id);
    assert_eq!(loaded.death_count, lifecycle.death_count);
    assert_eq!(
        loaded.fortune_remaining, 0,
        "fortune_remaining=0（运气已耗尽）是这个 bug 最关键的边界，必须原样往返"
    );
    assert_eq!(loaded.last_death_tick, lifecycle.last_death_tick);
    assert_eq!(loaded.last_revive_tick, lifecycle.last_revive_tick);
    assert_eq!(loaded.spawn_anchor, lifecycle.spawn_anchor);
    assert_eq!(loaded.spawn_anchor_damaged, lifecycle.spawn_anchor_damaged);
    assert_eq!(
        loaded.awaiting_decision,
        Some(RevivalDecision::Tribulation { chance: 0.2 }),
        "待决策的渡劫结果（含永久终结风险）必须原样往返"
    );
    let expected_deadline_at_save = lifecycle
        .revival_decision_deadline_tick
        .expect("sample awaiting revival lifecycle should have a deadline");
    let earliest_valid_deadline = expected_deadline_at_save.saturating_sub(max_elapsed_ticks);
    let loaded_deadline = loaded
        .revival_decision_deadline_tick
        .expect("awaiting revival deadline should survive the round trip");
    assert!(
        (earliest_valid_deadline..=expected_deadline_at_save).contains(&loaded_deadline),
        "deadline 应保留落盘时的剩余窗口并扣除测试期间真实流逝的墙钟时间；实际 {loaded_deadline}，有效区间 {earliest_valid_deadline}..={expected_deadline_at_save}",
    );
    assert_eq!(loaded.weakened_until_tick, lifecycle.weakened_until_tick);
    assert_eq!(loaded.state, LifecycleState::AwaitingRevival);

    let _ = fs::remove_dir_all(&data_dir);
}

// ── bughunt player-lifecycle-relog-death-consequence-wipe（OPUS 返工要求 1）──
//
// CombatClock 每次进程重启都从 0 重新计数，而
// revival_decision_deadline_tick / weakened_until_tick 都是落盘时刻算出的"绝对 tick"。
// 跨重启直接复用会把早已过期的 deadline 错当成几十小时后的未来事件，玩家会无限期卡在
// AwaitingRevival（无敌但无 UI）。下面的纯函数用例锁住
// `translate_lifecycle_deadline_tick_across_restart` 的折算数学，sqlite 集成用例锁住
// 端到端的读档行为。

#[test]
fn translate_lifecycle_deadline_tick_across_restart_passes_through_none() {
    assert_eq!(
        translate_lifecycle_deadline_tick_across_restart(None, 500_000, 1_000, 1_000, 0),
        None,
        "无 deadline（None）必须原样返回 None，不能凭空造出一个 tick"
    );
}

#[test]
fn translate_lifecycle_deadline_tick_across_restart_is_identity_with_zero_elapsed_wall_time() {
    // 同一进程内瞬时往返（wall 时间未流逝、CombatClock 未重启）：折算结果必须与直接
    // 复用旧绝对值完全一致，不能引入任何偏差。
    let deadline = translate_lifecycle_deadline_tick_across_restart(
        Some(9_999),
        0,     // combat_clock_tick_at_save
        1_000, // last_updated_wall
        1_000, // now_wall（未流逝）
        0,     // current_combat_clock_tick（未重启）
    );
    assert_eq!(
        deadline,
        Some(9_999),
        "wall 时间零流逝时折算必须是恒等映射，实际 {deadline:?}"
    );
}

#[test]
fn translate_lifecycle_deadline_tick_across_restart_settles_deadline_that_expired_during_downtime()
{
    // 跨重启核心场景：落盘时 CombatClock.tick=500_000，60 秒决策窗口
    // deadline=501_200，但进程重启期间墙钟流逝了 300 秒（远超 60 秒窗口）——折算结果
    // 必须落在新 tick 空间的"已过期"区间（<= current_combat_clock_tick），而不是被
    // 误当成还剩 501_200 tick（~7 小时）。
    let deadline = translate_lifecycle_deadline_tick_across_restart(
        Some(501_200),
        500_000,
        1_000,       // last_updated_wall
        1_000 + 300, // now_wall：重启耗时 300 秒
        0,           // current_combat_clock_tick：新进程刚启动
    );
    assert_eq!(
        deadline,
        Some(0),
        "决策窗口早已在墙钟层面过期，折算结果必须落在 current_combat_clock_tick(0)，\
         不能残留几十万 tick 的旧绝对值让玩家无限期卡在 AwaitingRevival；实际 {deadline:?}"
    );
}

#[test]
fn translate_lifecycle_deadline_tick_across_restart_preserves_remaining_window_when_still_valid() {
    // 落盘时刻还剩 30 秒(600 tick)才到期，重启只耗时 5 秒(100 tick)——折算后应保留约
    // 25 秒(500 tick)的真实剩余窗口，而不是无条件清零。
    let deadline = translate_lifecycle_deadline_tick_across_restart(
        Some(500_600), // combat_clock_tick_at_save(500_000) + 600
        500_000,
        1_000,     // last_updated_wall
        1_000 + 5, // now_wall：重启耗时 5 秒 = 100 tick
        0,         // current_combat_clock_tick：新进程刚启动
    );
    assert_eq!(
        deadline,
        Some(500), // 600 - 100 = 500 剩余 tick，叠加到新 tick 空间的 0 上
        "重启耗时仅 5 秒，60 秒窗口原本还剩 30 秒，折算后应保留约 25 秒(500 tick)的\
         剩余窗口；实际 {deadline:?}"
    );
}

#[test]
fn translate_lifecycle_deadline_tick_across_restart_clamps_wall_clock_rewind_to_zero_elapsed() {
    // 系统时钟回拨（now_wall < last_updated_wall）防御：不能因为负数流逝时间反而
    // "倒推"出比落盘时更长的剩余窗口。
    let deadline = translate_lifecycle_deadline_tick_across_restart(
        Some(500_600),
        500_000,
        2_000, // last_updated_wall（比 now_wall 还晚，模拟时钟回拨）
        1_000, // now_wall
        0,
    );
    assert_eq!(
        deadline,
        Some(600),
        "时钟回拨应按 0 流逝处理（remaining_at_save 原样保留），不能因为负数差值产生\
         异常膨胀的剩余窗口；实际 {deadline:?}"
    );
}

#[test]
fn translate_lifecycle_deadline_tick_across_restart_anchors_to_current_tick_within_same_session() {
    // 同进程断线重连（未重启）：CombatClock 没有归零，current_combat_clock_tick 已经
    // 推进到与墙钟流逝匹配的新值——折算逻辑必须叠加到这个新值上，而不是继续锚定在 0。
    let deadline = translate_lifecycle_deadline_tick_across_restart(
        Some(1_200), // combat_clock_tick_at_save(1_000) + 200
        1_000,
        1_000,     // last_updated_wall
        1_000 + 5, // now_wall：5 秒后重连 = 100 tick
        1_100,     // current_combat_clock_tick：同进程内已经走到 1_100（未重启）
    );
    // 剩余 = 200 - 100 = 100；叠加到 current_combat_clock_tick(1_100) 上 = 1_200，
    // 与"不折算直接复用旧绝对值 1_200"几乎一致（同进程内 wall 流逝与 tick 流逝同步时
    // 应收敛到相近结果）。
    assert_eq!(
        deadline,
        Some(1_200),
        "同进程内断线重连必须锚定在当前 CombatClock tick 上折算，实际 {deadline:?}"
    );
}

#[test]
fn player_lifecycle_slice_settles_expired_deadline_across_process_restart() {
    // 端到端集成锁：sqlite 里模拟"服务器重启前落盘、重启后读档"的完整链路。断线前
    // CombatClock.tick=500_000，60 秒决策窗口 deadline=501_200；落盘时刻是 5 分钟前
    // （远超 60 秒窗口），模拟"重启耗时够长，决策窗口早已到期"。修复前：
    // attach_combat_bundle_to_joined_clients 会原样复用 501_200 这个绝对值，重启后
    // CombatClock 从 0 计数，玩家要等 501_200 tick（~7 小时）才会被
    // auto_confirm_revival_decisions 结算，期间卡在 AwaitingRevival 无敌状态。
    let (persistence, data_dir) = sqlite_persistence("lifecycle-cross-restart-expired");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("baseline player state should persist");

    let combat_clock_tick_at_save = 500_000_u64;
    let revival_decision_deadline_tick = combat_clock_tick_at_save + 1_200; // 60s 窗口
    let last_updated_wall = current_unix_seconds() - 300;
    let lifecycle = Lifecycle {
        state: LifecycleState::AwaitingRevival,
        awaiting_decision: Some(RevivalDecision::Tribulation { chance: 0.3 }),
        revival_decision_deadline_tick: Some(revival_decision_deadline_tick),
        ..Lifecycle::default()
    };
    let lifecycle_json = serde_json::to_string(&lifecycle).expect("lifecycle should serialize");
    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    connection
        .execute(
            "
            INSERT INTO player_lifecycle (
                username, lifecycle_json, schema_version, last_updated_wall,
                combat_clock_tick_at_save
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![
                "Azure",
                lifecycle_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall,
                combat_clock_tick_at_save,
            ],
        )
        .expect("lifecycle fixture row should insert");

    // 模拟进程重启：新 CombatClock 从 0 开始。
    let loaded = load_player_lifecycle_slice(&persistence, "Azure", 0)
        .expect("lifecycle slice should load")
        .expect("lifecycle row should exist");

    assert_eq!(
        loaded.state,
        LifecycleState::AwaitingRevival,
        "跨重启不应该丢失 AwaitingRevival 状态本身，只应该折算 deadline"
    );
    assert_eq!(
        loaded.revival_decision_deadline_tick,
        Some(0),
        "决策窗口早已在墙钟层面过期，折算后的 deadline 必须落在重启后的\
         current_combat_clock_tick(0) 上，实际 {:?}——否则 auto_confirm_revival_decisions \
         会把它当成几万 tick 之后才到期，玩家无限期卡在 AwaitingRevival",
        loaded.revival_decision_deadline_tick
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn player_lifecycle_slice_preserves_partial_decision_window_across_short_restart() {
    // 短暂重启（比如几秒的滚动重启）不应该把仍然有效的决策窗口错误地清零——折算逻辑
    // 要保留按真实流逝时间计算出的剩余窗口。
    let (persistence, data_dir) = sqlite_persistence("lifecycle-cross-restart-partial");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("baseline player state should persist");

    let combat_clock_tick_at_save = 500_000_u64;
    // 60 秒决策窗口，落盘时刻 tick=500_000，窗口在 tick=501_200 到期。
    let revival_decision_deadline_tick = combat_clock_tick_at_save + 1_200;
    // 落盘 10 秒前——决策窗口还剩约 50 秒（1_000 tick）。
    let last_updated_wall = current_unix_seconds() - 10;
    let lifecycle = Lifecycle {
        state: LifecycleState::AwaitingRevival,
        awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
        revival_decision_deadline_tick: Some(revival_decision_deadline_tick),
        ..Lifecycle::default()
    };
    let lifecycle_json = serde_json::to_string(&lifecycle).expect("lifecycle should serialize");
    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    connection
        .execute(
            "
            INSERT INTO player_lifecycle (
                username, lifecycle_json, schema_version, last_updated_wall,
                combat_clock_tick_at_save
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![
                "Azure",
                lifecycle_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall,
                combat_clock_tick_at_save,
            ],
        )
        .expect("lifecycle fixture row should insert");

    // 模拟短暂重启：新进程 CombatClock 刚启动 20 tick（约 1 秒）。
    let current_combat_clock_tick = 20_u64;
    let loaded = load_player_lifecycle_slice(&persistence, "Azure", current_combat_clock_tick)
        .expect("lifecycle slice should load")
        .expect("lifecycle row should exist");

    let translated = loaded
        .revival_decision_deadline_tick
        .expect("decision deadline should still be Some after a short restart");
    let remaining_ticks = translated.saturating_sub(current_combat_clock_tick);
    // 预期剩余约 1_000 tick（50 秒），容忍 ±TICKS_PER_SECOND（1 秒）的墙钟取整抖动。
    let lower = 1_000_u64.saturating_sub(TICKS_PER_SECOND);
    let upper = 1_000_u64.saturating_add(TICKS_PER_SECOND);
    assert!(
        (lower..=upper).contains(&remaining_ticks),
        "60 秒决策窗口在落盘 10 秒后重启，应仍剩约 50 秒(1000 tick)的有效决策时间；\
         实际剩余 {remaining_ticks} tick（容忍区间 {lower}..={upper}）——过短说明误把\
         还有效的窗口判定成过期，过长说明没有正确扣减已流逝的时间",
    );

    let _ = fs::remove_dir_all(&data_dir);
}

fn seed_dash_known_techniques_row(persistence: &PlayerStatePersistence) -> KnownTechniques {
    let known_techniques = KnownTechniques {
        entries: vec![crate::cultivation::known_techniques::KnownTechnique {
            id: "movement.dash".to_string(),
            proficiency: 0.42,
            active: true,
        }],
    };
    save_player_state(persistence, "Azure", &PlayerState::default())
        .expect("baseline player state should persist");
    save_player_known_techniques_slice(persistence, "Azure", &known_techniques)
        .expect("known techniques slice should persist");
    known_techniques
}

#[test]
fn load_player_known_techniques_slice_returns_io_error_when_connection_cannot_open_and_recovers() {
    // 直接命中 canonical slice loader，而不是只测 load_player_slices 的早退分支：先
    // 写入一条真实功法行，再把 db_path 临时替换成目录模拟 SQLITE_CANTOPEN。这样若
    // loader 把 outage 错当成 Ok(None)，测试会明确失败；恢复路径验证重新建立连接后
    // 仍能读回原 durable row。
    let (persistence, data_dir) = sqlite_persistence("known-techniques-slice-cantopen");
    let expected = seed_dash_known_techniques_row(&persistence);
    let db_path = persistence.db_path().to_path_buf();
    let backup_path = data_dir.join("bong.db.outage-backup");

    fs::rename(&db_path, &backup_path).expect("database file should be movable for outage");
    fs::create_dir(&db_path).expect("directory placeholder should simulate unavailable DB");
    let outage_result = load_player_known_techniques_slice(&persistence, "Azure");

    fs::remove_dir(&db_path).expect("outage directory should be removable");
    fs::rename(&backup_path, &db_path).expect("database file should be restored after outage");

    let error = outage_result.expect_err(
        "canonical known-techniques loader must return io::Error during DB outage, not Ok(None)",
    );
    assert_eq!(
        error.kind(),
        io::ErrorKind::Other,
        "open_player_connection maps SQLite CANTOPEN to io::ErrorKind::Other; actual={error}"
    );

    let recovered = load_player_known_techniques_slice(&persistence, "Azure")
        .expect("canonical loader should reconnect after the database path is restored");
    assert_eq!(
        recovered,
        Some(expected),
        "reconnect must recover the existing durable row rather than treating the outage as a missing row"
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn canonical_techniques_load_skips_known_techniques_without_fabricating_default() {
    let (persistence, data_dir) = sqlite_persistence("known-techniques-canonical-skip");
    let expected = seed_dash_known_techniques_row(&persistence);

    let loaded = load_player_slices_for_canonical_techniques(&persistence, "Azure");

    assert_eq!(
        loaded.known_techniques,
        LoadedKnownTechniques::NotLoaded,
        "the aggregate loader must leave canonical known-techniques ownership to its slice"
    );
    assert_eq!(
        load_player_known_techniques_slice(&persistence, "Azure")
            .expect("canonical loader should still read the durable row"),
        Some(expected),
        "skipping the aggregate read must not remove or alter the durable techniques row"
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn player_lifespan_load_applies_offline_delta_from_pause_wall() {
    let (persistence, data_dir) = sqlite_persistence("lifespan-offline-delta");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("baseline player state should persist");

    let offline_pause_wall = current_unix_seconds()
        - (crate::cultivation::lifespan::LIFESPAN_SECONDS_PER_YEAR as i64 * 10);
    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    connection
        .execute(
            "
            INSERT INTO player_lifespan (
                username,
                born_at_tick,
                years_lived,
                cap_by_realm,
                offline_pause_wall,
                in_coffin,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                "Azure",
                0_u64,
                6.0_f64,
                LifespanCapTable::AWAKEN,
                offline_pause_wall,
                0_i64,
                PLAYER_ROW_SCHEMA_VERSION,
                offline_pause_wall,
            ],
        )
        .expect("lifespan fixture should insert");

    let loaded = load_player_slices(&persistence, "Azure");
    let loaded_lifespan = loaded.lifespan.expect("lifespan should reload");

    assert!(
        (6.99..=7.01).contains(&loaded_lifespan.years_lived),
        "expected ten offline real hours at x0.1 to add about one year, got {}",
        loaded_lifespan.years_lived
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn player_lifespan_load_applies_coffin_offline_multiplier() {
    let (persistence, data_dir) = sqlite_persistence("lifespan-coffin-offline-delta");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("baseline player state should persist");

    let offline_pause_wall = current_unix_seconds()
        - (crate::cultivation::lifespan::LIFESPAN_SECONDS_PER_YEAR as i64 * 10);
    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    connection
        .execute(
            "
            INSERT INTO player_lifespan (
                username,
                born_at_tick,
                years_lived,
                cap_by_realm,
                offline_pause_wall,
                in_coffin,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                "Azure",
                0_u64,
                6.0_f64,
                LifespanCapTable::AWAKEN,
                offline_pause_wall,
                1_i64,
                PLAYER_ROW_SCHEMA_VERSION,
                offline_pause_wall,
            ],
        )
        .expect("lifespan fixture should insert");

    let loaded = load_player_slices(&persistence, "Azure");
    let loaded_lifespan = loaded.lifespan.expect("lifespan should reload");

    assert!(loaded.in_coffin);
    // in_coffin=true 且无 coffin_grade → 默认凡木档 0.09
    assert_eq!(
        loaded.coffin_grade,
        Some(CoffinGrade::Mundane),
        "in_coffin=true + no explicit grade should load as Some(Mundane)"
    );
    assert!(
        (offline_lifespan_multiplier(Some(CoffinGrade::Mundane)) - 0.09).abs() < 1e-9,
        "mundane offline multiplier should be 0.09, got {}",
        offline_lifespan_multiplier(Some(CoffinGrade::Mundane))
    );
    assert!(
        (6.89..=6.91).contains(&loaded_lifespan.years_lived),
        "expected ten offline real hours in coffin at x0.09 to add about 0.9 years, got {}",
        loaded_lifespan.years_lived
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn offline_lifespan_multiplier_all_grades() {
    // 四档离线倍率 = OFFLINE(0.1) × lifespan_factor
    assert!(
        (offline_lifespan_multiplier(None) - 0.1).abs() < 1e-9,
        "None → 0.1"
    );
    assert!(
        (offline_lifespan_multiplier(Some(CoffinGrade::Mundane)) - 0.09).abs() < 1e-9,
        "Mundane → 0.09"
    );
    assert!(
        (offline_lifespan_multiplier(Some(CoffinGrade::Jade)) - 0.07).abs() < 1e-9,
        "Jade → 0.07"
    );
    assert!(
        (offline_lifespan_multiplier(Some(CoffinGrade::Stone)) - 0.05).abs() < 1e-9,
        "Stone → 0.05"
    );
    assert!(
        (offline_lifespan_multiplier(Some(CoffinGrade::Bronze)) - 0.03).abs() < 1e-9,
        "Bronze → 0.03"
    );
}

#[test]
fn player_lifespan_load_treats_zero_pause_wall_as_no_offline_delta() {
    let (persistence, data_dir) = sqlite_persistence("lifespan-zero-pause-wall");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("baseline player state should persist");

    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    connection
        .execute(
            "
            INSERT INTO player_lifespan (
                username,
                born_at_tick,
                years_lived,
                cap_by_realm,
                offline_pause_wall,
                in_coffin,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                "Azure",
                0_u64,
                12.0_f64,
                LifespanCapTable::AWAKEN,
                0_i64,
                0_i64,
                PLAYER_ROW_SCHEMA_VERSION,
                0_i64,
            ],
        )
        .expect("legacy zero-pause lifespan fixture should insert");

    let loaded = load_player_slices(&persistence, "Azure");
    let loaded_lifespan = loaded.lifespan.expect("lifespan should reload");

    assert_eq!(loaded_lifespan.years_lived, 12.0);

    let _ = fs::remove_dir_all(&data_dir);
}

/// plan-bughunt-inventory-transfer-orphan-pack-v1 P0 — 截劫夺包全量转移后不得残留孤儿
/// `pack_<id>` 容器脏档。
///
/// 复现链路：受害者穿戴一个 chest pack（rebuild 建 `pack_<id>` 容器 + 内含物）→
/// `transfer_all_inventory_contents` 把受害者 `equipped`（含 worn pack 本体）+ 所有容器
/// 内含物 + hotbar 全部搬给击杀者 → 受害者随即落盘。修复前：`from.containers` 里的
/// `pack_<id>` 容器壳原样残留但已无 backing worn 件，`load_player_slices` 重载时命中
/// `inventory_has_orphan_pack_container` → 整份 inventory 被丢弃回落默认 loadout。
/// 修复后：`transfer_all_inventory_contents` 内部收口 `rebuild_containers_from_equipment`，
/// 孤儿容器壳被同步清掉，重载必须拿回受害者的（空）inventory，而不是 `None`。
#[test]
fn transfer_all_inventory_contents_does_not_leave_orphan_pack_after_reload() {
    use crate::inventory::{
        container_id_for_worn_pack, transfer_all_inventory_contents, ContainerSpec, ItemCategory,
        ItemRegistry, ItemTemplate, PlacedItemState, SlotContents, EQUIP_SLOT_CHEST,
    };

    let (persistence, data_dir) = sqlite_persistence("transfer-orphan-pack-reload");

    // 合成 registry：一个 worn chest pack 模板 + 一个可移动 misc 内含物模板。
    let pack_template = ItemTemplate {
        id: "tribulation_chest_pack".to_string(),
        display_name: "夺魂背包".to_string(),
        category: ItemCategory::Container,
        placeable: None,
        max_stack_count: 1,
        grid_w: 2,
        grid_h: 2,
        base_weight: 0.5,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 1.0,
        description: "tribulation loot pack".to_string(),
        effect: None,
        cast_duration_ms: 0,
        cooldown_ms: 0,
        weapon_spec: None,
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
        readable_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: Some(ContainerSpec {
            quick_access: false,
            rows: 3,
            cols: 3,
            weight_capacity: 10.0,
            equip_slot: EQUIP_SLOT_CHEST.to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: false,
            accept_filter: None,
        }),
        shield_spec: None,
        shelflife_profile: None,
        shelflife_track: None,
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    };
    let mut loot = pack_template.clone();
    loot.id = "tribulation_loot".to_string();
    loot.display_name = "劫灰".to_string();
    loot.category = ItemCategory::Misc;
    loot.container_spec = None;
    loot.grid_w = 1;
    loot.grid_h = 1;
    let registry = ItemRegistry::from_map(HashMap::from([
        ("tribulation_chest_pack".to_string(), pack_template),
        ("tribulation_loot".to_string(), loot),
    ]));

    // 受害者：穿戴 chest pack（instance 9101）→ rebuild 建 pack_9101 容器 + 1 件内含物。
    let mut victim = empty_weapon_inventory();
    let mut pack_item = iron_sword_instance(9_101, 1.0);
    pack_item.template_id = "tribulation_chest_pack".to_string();
    pack_item.grid_w = 2;
    pack_item.grid_h = 2;
    victim.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(pack_item),
    );
    let _ = crate::inventory::rebuild_containers_from_equipment(&mut victim, &registry);

    let pack_id = container_id_for_worn_pack(9_101);
    let mut loot_item = iron_sword_instance(9_102, 1.0);
    loot_item.template_id = "tribulation_loot".to_string();
    loot_item.grid_w = 1;
    loot_item.grid_h = 1;
    {
        let pack = victim
            .containers
            .iter_mut()
            .find(|c| c.id == pack_id)
            .expect("rebuild should have created pack_9101");
        pack.items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: loot_item,
        });
    }
    assert!(
        victim.containers.iter().any(|c| c.id == pack_id),
        "前置条件：受害者身上应存在 live pack_9101 容器"
    );

    let mut killer = empty_weapon_inventory();

    // 截劫夺包：全量转移受害者 inventory 给击杀者。
    let outcome = transfer_all_inventory_contents(&mut victim, &mut killer, &registry);
    assert_eq!(
        outcome.items_moved, 2,
        "应转移 2 件（pack 本体 + 其内含物 loot）"
    );

    // 核心断言①：转移后受害者身上不得残留任何 pack_<id> 容器壳（孤儿）。
    assert!(
        !victim.containers.iter().any(|c| c.id == pack_id),
        "transfer 后受害者不得残留孤儿 `{pack_id}` 容器；实际容器列表={:?}",
        victim.containers.iter().map(|c| &c.id).collect::<Vec<_>>()
    );
    // 核心断言②：转移产物自身不得触发孤儿判定（与 loader 侧检测镜像一致）。
    assert!(
        !inventory_has_orphan_pack_container(&victim),
        "transfer_all_inventory_contents 产物必须自洽，不得被 loader 判为 #736 污染档"
    );

    // 核心断言③：落盘 + 重载，受害者 inventory 必须能拿回（不得被 loader 丢弃回落默认 loadout）。
    persist_player_with_inventory(&persistence, "TribulationVictim", &victim);
    let loaded = load_player_slices(&persistence, "TribulationVictim");
    assert!(
        loaded.inventory.is_some(),
        "受害者被截劫夺包后重登，inventory 不应被 loader 判为污染档丢弃回落默认 loadout"
    );
    let reloaded_victim = loaded.inventory.unwrap();
    assert!(
        !reloaded_victim.containers.iter().any(|c| c.id == pack_id),
        "重载后仍不应出现孤儿 `{pack_id}` 容器"
    );

    // 击杀者应完整收到 pack 本体 + loot（原容器内含物），战利品不丢。
    // 注：transfer 走 force_attach_item_to_inventory，全部塞进击杀者的容器格
    // （包括夺来的 pack 本体——作为物品转移，不自动穿戴）。
    assert!(
        killer
            .containers
            .iter()
            .flat_map(|c| c.items.iter())
            .any(|p| p.instance.instance_id == 9_101),
        "击杀者应在容器格收到夺来的 pack 本体（instance 9101），不得在转移中丢件"
    );
    assert!(
        killer
            .containers
            .iter()
            .flat_map(|c| c.items.iter())
            .any(|p| p.instance.instance_id == 9_102),
        "击杀者应收到 pack 内含物 loot（instance 9102），不得在转移中丢件"
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn ui_prefs_accepts_legacy_payload_without_skill_bar() {
    let prefs: PlayerUiPrefs = serde_json::from_value(serde_json::json!({
        "quick_slots": ["tea", null]
    }))
    .expect("legacy prefs should decode with default skill_bar");

    assert_eq!(prefs.quick_slots[0], Some("tea".to_string()));
    assert!(prefs.skill_configs.is_empty());
    assert!(prefs
        .skill_bar
        .iter()
        .all(|slot| matches!(slot, SkillSlotPersist::Empty)));
}

#[test]
fn ui_prefs_accepts_legacy_payload_without_skill_configs() {
    let prefs: PlayerUiPrefs = serde_json::from_value(serde_json::json!({
        "quick_slots": [null, null],
        "skill_bar": [
            {"kind":"empty"},
            {"kind":"empty"}
        ]
    }))
    .expect("legacy prefs should decode without skill_configs");

    assert!(prefs.skill_configs.is_empty());
}

#[test]
fn ui_prefs_sanitizes_legacy_dedicated_input_bindings() {
    let registry = TechniqueRegistry::load_for_tests();
    for invalid_id in ["movement.dash", "shield_block", "legacy.removed"] {
        let mut prefs: PlayerUiPrefs = serde_json::from_value(serde_json::json!({
            "skill_bar": [
                {"kind":"skill","skill_id":invalid_id},
                {"kind":"skill","skill_id":"burst_meridian.beng_quan"}
            ]
        }))
        .expect("legacy skill-bar prefs should decode");

        assert!(
            prefs.sanitize_skill_bar_bindings(&registry),
            "known dedicated-input and unknown legacy bindings must be repaired: {invalid_id}"
        );
        assert!(
            !prefs.sanitize_skill_bar_bindings(&registry),
            "sanitizing an already repaired skill bar must be idempotent"
        );

        let bindings = prefs.skill_bar_bindings(None, Some(&registry));
        assert!(matches!(bindings.slots[0], SkillSlot::Empty));
        assert!(matches!(
            &bindings.slots[1],
            SkillSlot::Skill { skill_id } if skill_id == "burst_meridian.beng_quan"
        ));
    }
}

#[test]
fn ui_prefs_rehydrates_quick_and_skill_bindings_from_inventory() {
    let prefs: PlayerUiPrefs = serde_json::from_value(serde_json::json!({
        "quick_slots": ["tea", null],
        "skill_bar": [
            {"kind":"skill","skill_id":"burst_meridian.beng_quan"},
            {"kind":"item","template_id":"tea"}
        ]
    }))
    .expect("prefs should decode");
    let mut inventory = PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: crate::inventory::InventoryRevision(0),
        containers: vec![crate::inventory::ContainerState {
            quick_access: false,
            id: "main".to_string(),
            name: "main".to_string(),
            rows: 5,
            cols: 7,
            items: vec![crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: crate::inventory::ItemInstance {
                    instance_id: 42,
                    template_id: "tea".to_string(),
                    display_name: "tea".to_string(),
                    grid_w: 1,
                    grid_h: 1,
                    weight: 0.1,
                    rarity: crate::inventory::ItemRarity::Common,
                    description: String::new(),
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
                },
            }],

            owner_instance_id: None,
        }],
        equipped: Default::default(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 50.0,
    };

    let quick = prefs.quick_slot_bindings(Some(&inventory));
    let skill_bar = prefs.skill_bar_bindings(Some(&inventory), None);

    assert_eq!(quick.slots[0], Some(42));
    assert!(matches!(
        &skill_bar.slots[0],
        SkillSlot::Skill { skill_id } if skill_id == "burst_meridian.beng_quan"
    ));
    assert_eq!(skill_bar.slots[1], SkillSlot::Item { instance_id: 42 });

    inventory.containers[0].items.clear();
    assert_eq!(prefs.quick_slot_bindings(Some(&inventory)).slots[0], None);
    assert_eq!(
        prefs.skill_bar_bindings(Some(&inventory), None).slots[1],
        SkillSlot::Empty,
        "重连时已不存在的物品必须恢复为空槽"
    );
}

#[test]
fn migrate_legacy_player_json_to_sqlite_once() {
    let (persistence, data_dir) = sqlite_persistence("legacy-migrate");

    #[derive(serde::Serialize)]
    struct LegacyPlayerStateV0 {
        realm: String,
        spirit_qi: f64,
        spirit_qi_max: f64,
        karma: f64,
        experience: u64,
        inventory_score: f64,
    }

    let legacy_state = LegacyPlayerStateV0 {
        realm: "Induce".to_string(),
        spirit_qi: 78.0,
        spirit_qi_max: 100.0,
        karma: 0.2,
        experience: 1_200,
        inventory_score: 0.4,
    };
    let expected_state = PlayerState {
        karma: 0.2,
        inventory_score: 0.4,
    };
    let save_path = persistence.path_for_username("CorruptCultivator");
    let migrated_path = persistence.migrated_path_for_username("CorruptCultivator");

    fs::create_dir_all(persistence.data_dir()).expect("test data dir should be creatable");
    fs::write(
        &save_path,
        serde_json::to_vec_pretty(&legacy_state).expect("legacy state should serialize"),
    )
    .expect("legacy PlayerState fixture should be writable");

    let migrated = load_player_state(&persistence, "CorruptCultivator");
    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    let first_char_id: String = connection
        .query_row(
            "SELECT current_char_id FROM player_core WHERE username = ?1",
            params!["CorruptCultivator"],
            |row| row.get(0),
        )
        .expect("migrated player_core row should exist");
    let reloaded = load_player_state(&persistence, "CorruptCultivator");
    let second_char_id: String = connection
        .query_row(
            "SELECT current_char_id FROM player_core WHERE username = ?1",
            params!["CorruptCultivator"],
            |row| row.get(0),
        )
        .expect("reloaded player_core row should exist");

    assert_eq!(migrated, expected_state.normalized());
    assert_eq!(reloaded, expected_state.normalized());
    assert!(
        !save_path.exists(),
        "legacy json should be renamed after migration"
    );
    assert!(
        migrated_path.exists(),
        "migrated legacy json should be preserved"
    );
    assert_eq!(first_char_id, second_char_id);
    assert_eq!(
        Uuid::parse_str(&first_char_id)
            .expect("current_char_id should be a UUID")
            .get_version_num(),
        7
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn corrupt_legacy_player_json_falls_back_without_affecting_other_players() {
    let (persistence, data_dir) = sqlite_persistence("corrupt-json-isolation");
    let corrupted_username = "CorruptCultivator";
    let healthy_username = "StableCultivator";
    let corrupted_path = persistence.path_for_username(corrupted_username);
    let corrupted_migrated_path = persistence.migrated_path_for_username(corrupted_username);
    let healthy_state = PlayerState {
        karma: -0.3,
        inventory_score: 0.55,
    };

    save_player_state(&persistence, healthy_username, &healthy_state)
        .expect("healthy player state should persist");

    fs::create_dir_all(persistence.data_dir()).expect("test data dir should be creatable");
    fs::write(&corrupted_path, br#"{"realm":"broken""#)
        .expect("corrupted legacy fixture should be writable");

    let corrupted_loaded = load_player_state(&persistence, corrupted_username);
    let healthy_loaded = load_player_state(&persistence, healthy_username);

    assert_eq!(corrupted_loaded, PlayerState::default());
    assert_eq!(healthy_loaded, healthy_state.normalized());
    assert!(
        corrupted_path.exists(),
        "corrupted legacy json should remain in place after failed migration"
    );
    assert!(
        !corrupted_migrated_path.exists(),
        "corrupted legacy json should not be marked as migrated"
    );

    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    let corrupted_row: Option<(f64, f64)> = connection
        .query_row(
            "
            SELECT karma, inventory_score
            FROM player_core
            WHERE username = ?1
            ",
            params![corrupted_username],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .expect("corrupted player_core row query should succeed");
    let healthy_row: (f64, f64) = connection
        .query_row(
            "
            SELECT karma, inventory_score
            FROM player_core
            WHERE username = ?1
            ",
            params![healthy_username],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("healthy player_core row should exist");

    assert_eq!(
        corrupted_row,
        Some((
            PlayerState::default().karma,
            PlayerState::default().inventory_score,
        ))
    );
    assert_eq!(
        healthy_row,
        (
            healthy_state.normalized().karma,
            healthy_state.normalized().inventory_score,
        )
    );

    let _ = fs::remove_dir_all(&data_dir);
}

// ─── plan-coffin-tiers-v1 P0 charge #6 — 非 mundane DB 全链路 ─────────
// save_player_lifespan_slice_with_coffin(Jade/Stone/Bronze) → DB → load → offline 回算正确

#[test]
fn db_full_chain_jade_coffin_offline_multiplier() {
    let (persistence, data_dir) = sqlite_persistence("db-full-chain-jade");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("baseline player state should persist");

    // 玩家在 Jade 棺内，离线 10 年等效真实秒
    let offline_seconds = crate::cultivation::lifespan::LIFESPAN_SECONDS_PER_YEAR as i64 * 10;
    let offline_pause_wall = current_unix_seconds() - offline_seconds;

    let conn = Connection::open(persistence.db_path()).expect("db should open");
    conn.execute(
        "INSERT INTO player_lifespan (
            username, born_at_tick, years_lived, cap_by_realm,
            offline_pause_wall, in_coffin, coffin_grade, schema_version, last_updated_wall
         ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 'jade', ?6, ?7)",
        params![
            "Azure",
            0_u64,
            6.0_f64,
            100_u32,
            offline_pause_wall,
            PLAYER_ROW_SCHEMA_VERSION,
            offline_pause_wall
        ],
    )
    .expect("jade lifespan fixture should insert");
    drop(conn);

    let loaded = load_player_slices(&persistence, "Azure");
    let loaded_lifespan = loaded.lifespan.expect("lifespan should reload");

    assert!(loaded.in_coffin, "should be in_coffin");
    assert_eq!(
        loaded.coffin_grade,
        Some(CoffinGrade::Jade),
        "loaded grade should be Some(Jade), got {:?}",
        loaded.coffin_grade
    );
    // jade 倍率 0.07 → 10 年 × 0.07 = 0.7 年
    assert!(
        (offline_lifespan_multiplier(Some(CoffinGrade::Jade)) - 0.07).abs() < 1e-9,
        "jade offline multiplier should be 0.07"
    );
    assert!(
        (6.69..=6.71).contains(&loaded_lifespan.years_lived),
        "expected 10 offline years in jade coffin at x0.07 to add ~0.7 years, \
         started at 6.0, got {}",
        loaded_lifespan.years_lived
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn db_full_chain_stone_coffin_offline_multiplier() {
    let (persistence, data_dir) = sqlite_persistence("db-full-chain-stone");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("baseline player state should persist");

    let offline_seconds = crate::cultivation::lifespan::LIFESPAN_SECONDS_PER_YEAR as i64 * 10;
    let offline_pause_wall = current_unix_seconds() - offline_seconds;

    let conn = Connection::open(persistence.db_path()).expect("db should open");
    conn.execute(
        "INSERT INTO player_lifespan (
            username, born_at_tick, years_lived, cap_by_realm,
            offline_pause_wall, in_coffin, coffin_grade, schema_version, last_updated_wall
         ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 'stone', ?6, ?7)",
        params![
            "Azure",
            0_u64,
            6.0_f64,
            100_u32,
            offline_pause_wall,
            PLAYER_ROW_SCHEMA_VERSION,
            offline_pause_wall
        ],
    )
    .expect("stone lifespan fixture should insert");
    drop(conn);

    let loaded = load_player_slices(&persistence, "Azure");
    let loaded_lifespan = loaded.lifespan.expect("lifespan should reload");

    assert!(loaded.in_coffin, "should be in_coffin");
    assert_eq!(
        loaded.coffin_grade,
        Some(CoffinGrade::Stone),
        "loaded grade should be Some(Stone), got {:?}",
        loaded.coffin_grade
    );
    // stone 倍率 0.05 → 10 年 × 0.05 = 0.5 年
    assert!(
        (offline_lifespan_multiplier(Some(CoffinGrade::Stone)) - 0.05).abs() < 1e-9,
        "stone offline multiplier should be 0.05"
    );
    assert!(
        (6.49..=6.51).contains(&loaded_lifespan.years_lived),
        "expected 10 offline years in stone coffin at x0.05 to add ~0.5 years, \
         started at 6.0, got {}",
        loaded_lifespan.years_lived
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn db_full_chain_bronze_coffin_offline_multiplier() {
    let (persistence, data_dir) = sqlite_persistence("db-full-chain-bronze");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("baseline player state should persist");

    let offline_seconds = crate::cultivation::lifespan::LIFESPAN_SECONDS_PER_YEAR as i64 * 10;
    let offline_pause_wall = current_unix_seconds() - offline_seconds;

    let conn = Connection::open(persistence.db_path()).expect("db should open");
    conn.execute(
        "INSERT INTO player_lifespan (
            username, born_at_tick, years_lived, cap_by_realm,
            offline_pause_wall, in_coffin, coffin_grade, schema_version, last_updated_wall
         ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 'bronze', ?6, ?7)",
        params![
            "Azure",
            0_u64,
            6.0_f64,
            100_u32,
            offline_pause_wall,
            PLAYER_ROW_SCHEMA_VERSION,
            offline_pause_wall
        ],
    )
    .expect("bronze lifespan fixture should insert");
    drop(conn);

    let loaded = load_player_slices(&persistence, "Azure");
    let loaded_lifespan = loaded.lifespan.expect("lifespan should reload");

    assert!(loaded.in_coffin, "should be in_coffin");
    assert_eq!(
        loaded.coffin_grade,
        Some(CoffinGrade::Bronze),
        "loaded grade should be Some(Bronze), got {:?}",
        loaded.coffin_grade
    );
    // bronze 倍率 0.03 → 10 年 × 0.03 = 0.3 年
    assert!(
        (offline_lifespan_multiplier(Some(CoffinGrade::Bronze)) - 0.03).abs() < 1e-9,
        "bronze offline multiplier should be 0.03"
    );
    assert!(
        (6.29..=6.31).contains(&loaded_lifespan.years_lived),
        "expected 10 offline years in bronze coffin at x0.03 to add ~0.3 years, \
         started at 6.0, got {}",
        loaded_lifespan.years_lived
    );

    let _ = fs::remove_dir_all(&data_dir);
}
