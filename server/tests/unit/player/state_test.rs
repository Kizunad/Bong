#![allow(dead_code, unused_imports)]

use bong_server::coffin::*;
use bong_server::combat::components::*;
use bong_server::craft::*;
use bong_server::cultivation::components::*;
use bong_server::cultivation::known_techniques::*;
use bong_server::cultivation::lifespan::*;
use bong_server::inventory::*;
use bong_server::network::agent_bridge::serialize_server_data_payload;
use bong_server::persistence::*;
use bong_server::player::spawn_selector::SpawnPurpose;
use bong_server::player::state::*;
use bong_server::qi_physics::ledger::pending_inflow_account;
use bong_server::qi_physics::*;
use bong_server::schema::server_data::*;
use bong_server::skill::components::*;
use bong_server::world::dimension::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
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

fn equipped_iron_sword_inventory(durability: f64) -> PlayerInventory {
    let mut inventory = empty_weapon_inventory();
    inventory.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        bong_server::inventory::SlotContents::held_single(iron_sword_instance(9_001, durability)),
    );
    inventory
}

fn persisted_inventory_snapshot(
    persistence: &PlayerStatePersistence,
    username: &str,
) -> serde_json::Value {
    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    let inventory_json: String = connection
        .query_row(
            "SELECT inventory_json FROM inventories WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .expect("persisted inventory row should exist");

    serde_json::from_str(&inventory_json).expect("persisted inventory JSON should decode")
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

fn only_container_item(inventory: &PlayerInventory) -> &ItemInstance {
    &inventory.containers[0].items[0].instance
}

#[test]
fn inventory_and_craft_session_roundtrip_and_clear_atomically() {
    let (persistence, data_dir) = sqlite_persistence("craft-session-roundtrip");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("active craft persistence requires an initialized player row");
    let inventory = empty_weapon_inventory();
    let session = CraftSession {
        recipe_id: bong_server::craft::RecipeId::new("craft.test.persisted"),
        started_at_tick: 10,
        remaining_ticks: 37,
        total_ticks: 40,
        owner_player_id: canonical_player_id("Azure"),
        qi_paid: 0.0,
        quantity_total: 3,
        completed_count: 1,
    };

    save_player_inventory_and_craft_session_slices(
        &persistence,
        "Azure",
        Some(&inventory),
        Some(&session),
    )
    .expect("inventory + craft session should persist in one transaction");

    let reloaded = load_player_slices(&persistence, "Azure");
    assert_eq!(
        reloaded.inventory.as_ref().map(|value| value.revision),
        Some(inventory.revision),
        "重启加载必须恢复与 session 同事务保存的预扣后 inventory"
    );
    assert_eq!(
        reloaded.craft_session.as_ref(),
        Some(&session),
        "重启加载必须恢复完整 CraftSession，避免预扣材料失去结算凭证"
    );

    let mut completed_inventory = inventory.clone();
    completed_inventory.revision = InventoryRevision(42);
    save_player_inventory_and_craft_session_slices(
        &persistence,
        "Azure",
        Some(&completed_inventory),
        None,
    )
    .expect("terminal inventory + session delete should commit atomically");
    let cleared = load_player_slices(&persistence, "Azure");
    assert_eq!(
        cleared.inventory.as_ref().map(|value| value.revision),
        Some(InventoryRevision(42)),
        "终止结算应保存最终 inventory"
    );
    assert!(
        cleared.craft_session.is_none(),
        "终止结算与 session 删除必须同事务提交，重连不能重复完成或退款"
    );

    std::fs::remove_dir_all(data_dir).ok();
}

#[test]
fn invalid_persisted_login_y_falls_back_to_spawn() {
    let (persistence, data_dir) = sqlite_persistence("invalid-login-y");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("saving PlayerState should succeed");
    save_player_slow_slice(
        &persistence,
        "Azure",
        [42.0, -26_297.0, -3.5],
        DimensionKind::default(),
    )
    .expect("saving invalid slow slice should succeed");

    let loaded = load_player_slices(&persistence, "Azure");

    assert_eq!(
        loaded.position,
        bong_server::player::spawn_position_for_seed("Azure", SpawnPurpose::InitialLogin)
    );
    assert_eq!(loaded.last_dimension, DimensionKind::default());

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn player_export_bundle_roundtrips_back_into_sqlite() {
    let (source_persistence, source_data_dir) = sqlite_persistence("export-bundle-source");
    let exported_state = PlayerState {
        karma: 0.25,
        inventory_score: 0.7,
    };
    save_player_slices(
        &source_persistence,
        "Azure",
        &exported_state,
        [64.0, 80.0, -12.0],
        DimensionKind::Tsy,
        None,
        None,
        &SkillSet::default(),
    )
    .expect("source player slices should persist");

    let bundle = export_player_bundle(&source_persistence, "Azure")
        .expect("player export bundle should load");

    let (target_persistence, target_data_dir) = sqlite_persistence("export-bundle-target");
    import_player_bundle(&target_persistence, &bundle).expect("player export bundle should import");

    let connection = Connection::open(target_persistence.db_path()).expect("sqlite db should open");
    let current_char_id: String = connection
        .query_row(
            "SELECT current_char_id FROM player_core WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .expect("player_core row should exist after import");
    let (karma, inventory_score): (f64, f64) = connection
        .query_row(
            "
            SELECT karma, inventory_score
            FROM player_core
            WHERE username = ?1
            ",
            params!["Azure"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("player_core payload should exist after import");
    let (pos_x, pos_y, pos_z, last_dimension_text): (f64, f64, f64, String) = connection
        .query_row(
            "SELECT pos_x, pos_y, pos_z, last_dimension FROM player_slow WHERE username = ?1",
            params!["Azure"],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("player_slow row should exist after import");
    let inventory_json: String = connection
        .query_row(
            "SELECT inventory_json FROM inventories WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .expect("inventories row should exist after import");
    let prefs_json: String = connection
        .query_row(
            "SELECT prefs_json FROM player_ui_prefs WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .expect("player_ui_prefs row should exist after import");

    assert_eq!(bundle.kind, "player_export_v1");
    assert_eq!(current_char_id, bundle.current_char_id);
    assert_eq!(karma, 0.25);
    assert_eq!(inventory_score, 0.7);
    assert_eq!((pos_x, pos_y, pos_z), (64.0, 80.0, -12.0));
    assert_eq!(last_dimension_text, "tsy");
    assert_eq!(bundle.last_dimension, DimensionKind::Tsy);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&inventory_json)
            .expect("inventory_json should decode"),
        serde_json::Value::Null
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&prefs_json).expect("prefs_json should decode"),
        bundle.ui_prefs
    );

    let _ = fs::remove_dir_all(&source_data_dir);
    let _ = fs::remove_dir_all(&target_data_dir);
}

#[test]
fn player_lifespan_slice_roundtrips_with_offline_pause_wall() {
    let (persistence, data_dir) = sqlite_persistence("lifespan-roundtrip");
    let player_state = PlayerState::default();
    let lifespan = LifespanComponent {
        born_at_tick: 144,
        years_lived: 12.5,
        cap_by_realm: LifespanCapTable::CONDENSE,
        offline_pause_tick: Some(120),
    };

    save_player_slices(
        &persistence,
        "Azure",
        &player_state,
        [11.0, 70.0, -2.0],
        DimensionKind::default(),
        None,
        Some(&lifespan),
        &SkillSet::default(),
    )
    .expect("lifespan slice should persist with player slices");

    let loaded = load_player_slices(&persistence, "Azure");
    let loaded_lifespan = loaded.lifespan.expect("lifespan should reload");
    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    let offline_pause_wall: i64 = connection
        .query_row(
            "SELECT offline_pause_wall FROM player_lifespan WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .expect("player_lifespan row should exist");

    assert_eq!(loaded_lifespan.born_at_tick, lifespan.born_at_tick);
    assert_eq!(loaded_lifespan.cap_by_realm, lifespan.cap_by_realm);
    assert!(loaded_lifespan.years_lived >= lifespan.years_lived);
    assert!(loaded_lifespan.years_lived < lifespan.years_lived + 0.01);
    assert!(offline_pause_wall > 0);

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn player_known_techniques_slice_roundtrips_dash_proficiency() {
    let (persistence, data_dir) = sqlite_persistence("known-techniques-roundtrip");
    let known_techniques = KnownTechniques {
        entries: vec![bong_server::cultivation::known_techniques::KnownTechnique {
            id: "movement.dash".to_string(),
            proficiency: 0.42,
            active: true,
        }],
    };

    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("baseline player state should persist");
    save_player_known_techniques_slice(&persistence, "Azure", &known_techniques)
        .expect("known techniques slice should persist");

    let loaded = load_player_slices(&persistence, "Azure");
    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    let known_techniques_json: String = connection
        .query_row(
            "SELECT known_techniques_json FROM player_known_techniques WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .expect("player_known_techniques row should exist");
    let snapshot: serde_json::Value =
        serde_json::from_str(&known_techniques_json).expect("known techniques JSON should decode");

    assert_eq!(
        loaded.known_techniques,
        LoadedKnownTechniques::Loaded(known_techniques),
        "roundtrip 后应加载出与写入一致的功法表（Loaded 变体），否则持久化链路本身已损坏"
    );
    assert_eq!(
        snapshot
            .pointer("/entries/0/id")
            .and_then(serde_json::Value::as_str),
        Some("movement.dash")
    );
    let proficiency = snapshot
        .pointer("/entries/0/proficiency")
        .and_then(serde_json::Value::as_f64)
        .expect("dash proficiency should persist");
    assert!((proficiency - 0.42).abs() < 1e-6);

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
fn player_lifecycle_slice_missing_row_returns_none_not_error() {
    // 首次登录 / pre-v39 老档：该用户名从未落过 player_lifecycle 行。调用方
    // （attach_combat_bundle_to_joined_clients）要能区分"没有存档"（Ok(None)，回退
    // Default）与"读取失败"（Err，同样回退但走告警日志），这里锁住前者。
    let (persistence, data_dir) = sqlite_persistence("lifecycle-missing-row");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("baseline player state should persist");

    let loaded = load_player_lifecycle_slice(&persistence, "Azure", 0)
        .expect("missing row should not be an I/O error");

    assert!(
        loaded.is_none(),
        "从未 save 过的用户名必须读回 None，不能凭空造出一个 Lifecycle"
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn player_lifecycle_slice_roundtrips_alive_default_fortune() {
    // A→A 状态转换 pin：健康在线玩家的常规 Lifecycle（Alive + 满运气次数）也要能
    // round-trip，保留不同裁决分支。
    let (persistence, data_dir) = sqlite_persistence("lifecycle-roundtrip-alive");
    let lifecycle = Lifecycle::default();
    assert_eq!(lifecycle.state, LifecycleState::Alive);
    assert_eq!(lifecycle.fortune_remaining, 3);

    save_player_lifecycle_slice(&persistence, "Azure", &lifecycle, 0)
        .expect("lifecycle slice should persist");
    let loaded = load_player_lifecycle_slice(&persistence, "Azure", 0)
        .expect("lifecycle slice should load")
        .expect("lifecycle row should exist after save");

    assert_eq!(loaded.state, LifecycleState::Alive);
    assert_eq!(loaded.fortune_remaining, 3);
    assert_eq!(loaded.awaiting_decision, None);

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn player_lifecycle_slice_roundtrips_all_state_variants() {
    // 不同生命周期状态均需正确持久化。
    for state in [
        LifecycleState::Alive,
        LifecycleState::AwaitingRevival,
        LifecycleState::Terminated,
    ] {
        let (persistence, data_dir) =
            sqlite_persistence(&format!("lifecycle-state-variant-{state:?}"));
        let lifecycle = Lifecycle {
            state,
            ..Lifecycle::default()
        };

        save_player_lifecycle_slice(&persistence, "Azure", &lifecycle, 0)
            .expect("lifecycle slice should persist");
        let loaded = load_player_lifecycle_slice(&persistence, "Azure", 0)
            .expect("lifecycle slice should load")
            .expect("lifecycle row should exist after save");

        assert_eq!(loaded.state, state, "LifecycleState::{state:?} 未原样往返");

        let _ = fs::remove_dir_all(&data_dir);
    }
}

#[test]
fn player_lifecycle_slice_roundtrips_both_revival_decision_variants() {
    // enum 变体 pin：RevivalDecision 的 Fortune / Tribulation 两个变体各一条专属用例
    // （两者语义天差地别：can_terminate() 只有 Tribulation 为 true）。
    for decision in [
        RevivalDecision::Fortune { chance: 0.8 },
        RevivalDecision::Tribulation { chance: 0.1 },
    ] {
        let (persistence, data_dir) =
            sqlite_persistence(&format!("lifecycle-decision-variant-{decision:?}"));
        let lifecycle = Lifecycle {
            state: LifecycleState::AwaitingRevival,
            awaiting_decision: Some(decision),
            revival_decision_deadline_tick: Some(42),
            ..Lifecycle::default()
        };

        save_player_lifecycle_slice(&persistence, "Azure", &lifecycle, 0)
            .expect("lifecycle slice should persist");
        let loaded = load_player_lifecycle_slice(&persistence, "Azure", 0)
            .expect("lifecycle slice should load")
            .expect("lifecycle row should exist after save");

        assert_eq!(
            loaded.awaiting_decision,
            Some(decision),
            "RevivalDecision::{decision:?} 未原样往返"
        );
        assert_eq!(
            loaded.awaiting_decision.map(|d| d.can_terminate()),
            Some(decision.can_terminate()),
            "RevivalDecision::{decision:?} 的 can_terminate() 语义（是否携带永久终结\
             风险）在往返后必须保持一致"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }
}

#[test]
fn player_lifecycle_slice_overwrite_replaces_previous_row_not_duplicates() {
    // ON CONFLICT(username) DO UPDATE：同一用户名二次 save 必须覆盖而非新增/保留旧值，
    // 否则重连读到的是"某一次历史断线"而不是"最后一次断线"的状态。
    let (persistence, data_dir) = sqlite_persistence("lifecycle-overwrite");

    let first = Lifecycle {
        state: LifecycleState::AwaitingRevival,
        fortune_remaining: 2,
        ..Lifecycle::default()
    };
    save_player_lifecycle_slice(&persistence, "Azure", &first, 0)
        .expect("first lifecycle slice should persist");

    let second = sample_lifecycle_awaiting_revival_zero_fortune();
    save_player_lifecycle_slice(&persistence, "Azure", &second, 0)
        .expect("second lifecycle slice should overwrite");

    let loaded = load_player_lifecycle_slice(&persistence, "Azure", 0)
        .expect("lifecycle slice should load")
        .expect("lifecycle row should exist after save");

    assert_eq!(
        loaded.state,
        LifecycleState::AwaitingRevival,
        "第二次 save 必须覆盖旧裁决与运数"
    );
    assert_eq!(loaded.fortune_remaining, 0);

    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    let row_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM player_lifecycle WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .expect("row count query should succeed");
    assert_eq!(row_count, 1, "覆盖保存不能在 player_lifecycle 里堆出重复行");

    let _ = fs::remove_dir_all(&data_dir);
}

fn seed_dash_known_techniques_row(persistence: &PlayerStatePersistence) -> KnownTechniques {
    let known_techniques = KnownTechniques {
        entries: vec![bong_server::cultivation::known_techniques::KnownTechnique {
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

fn corrupt_known_techniques_row(persistence: &PlayerStatePersistence) {
    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    connection
        .execute(
            "UPDATE player_known_techniques SET known_techniques_json = '{not json' WHERE username = ?1",
            params!["Azure"],
        )
        .expect("corrupting known techniques row should succeed");
}

#[test]
fn known_techniques_load_returns_load_failed_on_corrupt_json_without_touching_row() {
    let (persistence, data_dir) = sqlite_persistence("known-techniques-corrupt-json");
    seed_dash_known_techniques_row(&persistence);
    corrupt_known_techniques_row(&persistence);

    let loaded = load_player_slices(&persistence, "Azure");

    assert_eq!(
        loaded.known_techniques,
        LoadedKnownTechniques::LoadFailed,
        "行存在但 JSON 损坏时必须返回 LoadFailed（禁止兜底 default），\
         否则 join 后的 Changed flush 会用空表覆盖真实存档"
    );
    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    let raw_json: String = connection
        .query_row(
            "SELECT known_techniques_json FROM player_known_techniques WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .expect("player_known_techniques row should still exist");
    assert_eq!(
        raw_json, "{not json",
        "load 是只读操作，失败路径不得改写/删除已持久化的行"
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn known_techniques_load_returns_load_failed_on_sqlite_error() {
    let (persistence, data_dir) = sqlite_persistence("known-techniques-sqlite-error");
    seed_dash_known_techniques_row(&persistence);
    // 注入 sqlite 层错误（非 JSON 解析错误）：表被删后 SELECT 直接报错，
    // 模拟 DB 结构损坏 / 迁移中途等「行状态不可知」场景。
    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    connection
        .execute_batch("DROP TABLE player_known_techniques;")
        .expect("dropping table should succeed");

    let loaded = load_player_slices(&persistence, "Azure");

    assert_eq!(
        loaded.known_techniques,
        LoadedKnownTechniques::LoadFailed,
        "sqlite 层错误（表缺失/锁竞争等）必须返回 LoadFailed 而非 default，\
         因为无法区分「无数据」与「有数据但读不到」"
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn known_techniques_load_returns_load_failed_when_connection_cannot_open() {
    // 锁定 load_player_slices 的连接失败早退分支（open_player_connection 报错）：
    // 把 db_path 指向一个目录，sqlite 打开必定 SQLITE_CANTOPEN，稳定跨平台复现，
    // 不依赖文件权限行为。此时行状态完全不可知，必须 LoadFailed 而非「新玩家」。
    let data_dir = std::env::temp_dir().join(format!(
        "bong-known-techniques-cantopen-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let dir_as_db = data_dir.join("bong.db");
    fs::create_dir_all(&dir_as_db).expect("creating directory placeholder should succeed");
    let persistence = PlayerStatePersistence::with_db_path(&data_dir, &dir_as_db);

    let loaded = load_player_slices(&persistence, "Azure");

    assert_eq!(
        loaded.known_techniques,
        LoadedKnownTechniques::LoadFailed,
        "连接都打不开时行状态不可知，必须返回 LoadFailed 触发写保护，\
         退化成 Loaded(default) 就会重新打开「空表覆盖真实存档」的丢档路径"
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn known_techniques_load_defaults_for_new_player_without_row() {
    let (persistence, data_dir) = sqlite_persistence("known-techniques-new-player");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("baseline player state should persist");

    let loaded = load_player_slices(&persistence, "Azure");

    assert_eq!(
        loaded.known_techniques,
        LoadedKnownTechniques::Loaded(KnownTechniques::default()),
        "DB 无行 = 真新玩家，应返回 Loaded(default) 并允许后续正常落盘，\
         不得与「有行但读取失败」混为一谈"
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn known_techniques_load_recovers_after_corrupt_row_is_repaired() {
    let (persistence, data_dir) = sqlite_persistence("known-techniques-repair-recovery");
    let original = seed_dash_known_techniques_row(&persistence);
    corrupt_known_techniques_row(&persistence);
    assert_eq!(
        load_player_slices(&persistence, "Azure").known_techniques,
        LoadedKnownTechniques::LoadFailed,
        "前置条件：损坏行应先观察到 LoadFailed"
    );

    // 恢复路径：损坏行被合法写路径修复（如人工修复 / 另一次健康会话的落盘）后，
    // 下一次加载应完整恢复，不残留任何失败状态。
    save_player_known_techniques_slice(&persistence, "Azure", &original)
        .expect("repairing the row via the legit save path should succeed");

    assert_eq!(
        load_player_slices(&persistence, "Azure").known_techniques,
        LoadedKnownTechniques::Loaded(original),
        "行修复后重新加载应回到 Loaded(原数据)——失败状态只属于单次会话，不粘滞"
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn export_player_bundle_refuses_when_known_techniques_load_fails() {
    let (persistence, data_dir) = sqlite_persistence("known-techniques-export-refusal");
    seed_dash_known_techniques_row(&persistence);
    corrupt_known_techniques_row(&persistence);

    let error = export_player_bundle(&persistence, "Azure")
        .expect_err("行损坏时导出必须失败，而不是静默导出一张 default 空表");
    assert!(
        error.to_string().contains("refusing to export"),
        "导出失败原因应指明是功法行读取失败的写保护拒绝，实际：{error}"
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn equipped_weapon_persists_across_player_reload() {
    let (persistence, data_dir) = sqlite_persistence("equipped-weapon-reload");
    let inventory = equipped_iron_sword_inventory(0.87);

    persist_player_with_inventory(&persistence, "Azure", &inventory);

    let loaded = load_player_slices(&persistence, "Azure");
    let loaded_inventory = loaded.inventory.expect("inventory should reload");
    let main_hand_slot = loaded_inventory
        .equipped
        .get(EQUIP_SLOT_MAIN_HAND)
        .expect("main_hand iron_sword should reload from sqlite");
    let main_hand = main_hand_slot
        .held
        .as_ref()
        .expect("main_hand slot should have held iron_sword");
    let snapshot = persisted_inventory_snapshot(&persistence, "Azure");

    assert_eq!(main_hand.instance_id, 9_001);
    assert_eq!(main_hand.template_id, "iron_sword");
    approx_eq(main_hand.durability, 0.87);
    assert_eq!(
        snapshot
            .pointer("/equipped/main_hand/held/template_id")
            .and_then(serde_json::Value::as_str),
        Some("iron_sword")
    );
    println!(
        "weapon_persistence_snapshot equipped={}",
        serde_json::to_string(&snapshot).expect("snapshot should serialize")
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn unequipped_weapon_persists_empty_main_hand_across_reload() {
    let (persistence, data_dir) = sqlite_persistence("unequipped-weapon-reload");
    let mut inventory = equipped_iron_sword_inventory(0.62);
    move_equipped_item_to_first_container_slot(&mut inventory, 9_001)
        .expect("equipped sword should move back into the main pack");

    persist_player_with_inventory(&persistence, "Azure", &inventory);

    let loaded = load_player_slices(&persistence, "Azure");
    let loaded_inventory = loaded.inventory.expect("inventory should reload");
    let packed_sword = only_container_item(&loaded_inventory);
    let snapshot = persisted_inventory_snapshot(&persistence, "Azure");

    assert!(!loaded_inventory.equipped.contains_key(EQUIP_SLOT_MAIN_HAND));
    assert_eq!(packed_sword.instance_id, 9_001);
    assert_eq!(packed_sword.template_id, "iron_sword");
    approx_eq(packed_sword.durability, 0.62);
    assert!(snapshot.pointer("/equipped/main_hand").is_none());
    assert_eq!(
        snapshot
            .pointer("/containers/0/items/0/instance/template_id")
            .and_then(serde_json::Value::as_str),
        Some("iron_sword")
    );
    println!(
        "weapon_persistence_snapshot unequipped={}",
        serde_json::to_string(&snapshot).expect("snapshot should serialize")
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn broken_weapon_state_persists_after_inventory_slice_flush() {
    let (persistence, data_dir) = sqlite_persistence("broken-weapon-reload");
    let mut inventory = equipped_iron_sword_inventory(1.0);
    persist_player_with_inventory(&persistence, "Azure", &inventory);

    set_item_instance_durability(&mut inventory, 9_001, 0.0)
        .expect("weapon durability should update to broken");
    move_equipped_item_to_first_container_slot(&mut inventory, 9_001)
        .expect("broken weapon should move back into the main pack");
    save_player_inventory_slice(&persistence, "Azure", Some(&inventory))
        .expect("changed inventory slice should persist");

    let loaded = load_player_slices(&persistence, "Azure");
    let loaded_inventory = loaded.inventory.expect("inventory should reload");
    let broken_sword = only_container_item(&loaded_inventory);
    let snapshot = persisted_inventory_snapshot(&persistence, "Azure");

    assert!(!loaded_inventory.equipped.contains_key(EQUIP_SLOT_MAIN_HAND));
    assert_eq!(broken_sword.instance_id, 9_001);
    assert_eq!(broken_sword.template_id, "iron_sword");
    approx_eq(broken_sword.durability, 0.0);
    assert_eq!(
        snapshot
            .pointer("/containers/0/items/0/instance/durability")
            .and_then(serde_json::Value::as_f64),
        Some(0.0)
    );
    println!(
        "weapon_persistence_snapshot broken={}",
        serde_json::to_string(&snapshot).expect("snapshot should serialize")
    );

    let _ = fs::remove_dir_all(&data_dir);
}

/// plan-tarkov-backpack-v1 P2 e2e（交付物 #1 / 测试清单）— 拖入穿戴中 pack 后跨重载持久化。
///
/// 经 `apply_inventory_move` 真路径把物品拖入穿戴中的 `pack_<id>` 容器（穿戴态门控放行）→
/// `save_player_slices` 落盘 → `load_player_slices` 重载 → 物品仍在 `pack_<id>` 内。
/// 锁住「拖入持久化无额外入口、经 flush 自动落盘、重载不丢」契约；任何把 pack_<id> 容器
/// 内含物从持久化序列中摘掉的回归立即撞红。
#[test]
fn e2e_drag_item_into_pack_persists_across_reload() {
    use bong_server::inventory::{
        apply_inventory_move, container_id_for_worn_pack, rebuild_containers_from_equipment,
        ContainerSpec, ItemCategory, ItemRegistry, ItemTemplate, PlacedItemState, SlotContents,
        EQUIP_SLOT_CHEST,
    };
    use bong_server::schema::inventory::InventoryLocationV1;

    let (persistence, data_dir) = sqlite_persistence("drag-into-pack-reload");

    // 合成 registry：一个 worn pack 模板（chest，3×3）+ 一个 1×1 misc 可移动物品。
    let pack_template = ItemTemplate {
        id: "e2e_chest_pack".to_string(),
        display_name: "胸前套包".to_string(),
        category: ItemCategory::Container,
        placeable: None,
        max_stack_count: 1,
        grid_w: 2,
        grid_h: 2,
        base_weight: 0.5,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 1.0,
        description: "e2e pack".to_string(),
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
        wearer_race: bong_server::body_plan::types::RaceGateOwned::default(),
    };
    let mut dust = pack_template.clone();
    dust.id = "e2e_dust".to_string();
    dust.display_name = "灵尘".to_string();
    dust.category = ItemCategory::Misc;
    dust.container_spec = None;
    dust.grid_w = 1;
    dust.grid_h = 1;
    let registry = ItemRegistry::from_map(HashMap::from([
        ("e2e_chest_pack".to_string(), pack_template),
        ("e2e_dust".to_string(), dust),
    ]));

    // 穿戴 chest pack（instance 8801）→ rebuild 建 pack_8801 容器 + 回填 owner。
    let mut inventory = empty_weapon_inventory();
    let mut pack_item = iron_sword_instance(8_801, 1.0);
    pack_item.template_id = "e2e_chest_pack".to_string();
    pack_item.grid_w = 2;
    pack_item.grid_h = 2;
    inventory.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(pack_item),
    );
    let _ = rebuild_containers_from_equipment(&mut inventory, &registry);

    // main_pack 放一件 dust（8802），准备拖入 pack_8801。
    let mut dust_item = iron_sword_instance(8_802, 1.0);
    dust_item.template_id = "e2e_dust".to_string();
    dust_item.grid_w = 1;
    dust_item.grid_h = 1;
    let main = inventory
        .containers
        .iter_mut()
        .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
        .expect("main_pack 存在");
    main.items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: dust_item,
    });

    // 拖入穿戴中的 pack_8801（穿戴态门控放行 + 落位）。
    let pack_id = container_id_for_worn_pack(8_801);
    let from = InventoryLocationV1::Container {
        container_id: MAIN_PACK_CONTAINER_ID.to_string(),
        row: 0,
        col: 0,
    };
    let to = InventoryLocationV1::Container {
        container_id: pack_id.clone(),
        row: 1,
        col: 2,
    };
    apply_inventory_move(&mut inventory, &registry, 8_802, &from, &to, false)
        .expect("拖入穿戴中的 pack_8801 应成功");

    // 落盘。
    persist_player_with_inventory(&persistence, "PackReload", &inventory);

    // 重载。
    let loaded = load_player_slices(&persistence, "PackReload");
    let loaded_inventory = loaded.inventory.expect("inventory should reload");

    // pack_8801 容器仍存在，dust(8802) 仍在其中（位置守恒 row=1,col=2）。
    let pack = loaded_inventory
        .containers
        .iter()
        .find(|c| c.id == pack_id)
        .unwrap_or_else(|| panic!("重载后 `{pack_id}` 容器应仍存在"));
    let placed = pack
        .items
        .iter()
        .find(|p| p.instance.instance_id == 8_802)
        .unwrap_or_else(|| {
            panic!("重载后 dust(8802) 应仍在 `{pack_id}` 内（拖入持久化经 flush 自动落盘，不丢）")
        });
    assert_eq!(
        (placed.row, placed.col),
        (1, 2),
        "重载后 dust 落位坐标应守恒 (1,2)；实际 ({},{})",
        placed.row,
        placed.col
    );
    assert_eq!(
        placed.instance.template_id, "e2e_dust",
        "重载后 instance 8802 模板应保持 e2e_dust"
    );
    // dust 不应残留在 main_pack。
    let main = loaded_inventory
        .containers
        .iter()
        .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
        .expect("main_pack 重载存在");
    assert!(
        !main.items.iter().any(|p| p.instance.instance_id == 8_802),
        "拖入 pack 后 dust(8802) 不应再残留在 main_pack（move 而非 copy）"
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn import_player_bundle_rejects_invalid_current_char_id() {
    let (persistence, data_dir) = sqlite_persistence("import-invalid-char-id");
    let bundle = PlayerExportBundle {
        kind: "player_export_v1".to_string(),
        username: "Azure".to_string(),
        current_char_id: "not-a-uuid".to_string(),
        state: PlayerState {
            karma: 0.25,
            inventory_score: 0.7,
        },
        position: [64.0, 80.0, -12.0],
        last_dimension: DimensionKind::default(),
        inventory: None,
        skill_set: SkillSet::default(),
        known_techniques: KnownTechniques::default(),
        ui_prefs: serde_json::json!({
            "quick_slots": [null, null]
        }),
    };

    let error = import_player_bundle(&persistence, &bundle)
        .expect_err("invalid current_char_id should be rejected");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);

    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    let player_core_exists: Option<String> = connection
        .query_row(
            "SELECT username FROM player_core WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .optional()
        .expect("player_core query should succeed");
    let player_slow_exists: Option<String> = connection
        .query_row(
            "SELECT username FROM player_slow WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .optional()
        .expect("player_slow query should succeed");
    let inventories_exists: Option<String> = connection
        .query_row(
            "SELECT username FROM inventories WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .optional()
        .expect("inventories query should succeed");
    let prefs_exists: Option<String> = connection
        .query_row(
            "SELECT username FROM player_ui_prefs WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .optional()
        .expect("player_ui_prefs query should succeed");

    assert!(player_core_exists.is_none());
    assert!(player_slow_exists.is_none());
    assert!(inventories_exists.is_none());
    assert!(prefs_exists.is_none());

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn import_player_bundle_rejects_invalid_ui_prefs() {
    let (persistence, data_dir) = sqlite_persistence("import-invalid-ui-prefs");
    let bundle = PlayerExportBundle {
        kind: "player_export_v1".to_string(),
        username: "Azure".to_string(),
        current_char_id: Uuid::now_v7().to_string(),
        state: PlayerState {
            karma: 0.25,
            inventory_score: 0.7,
        },
        position: [64.0, 80.0, -12.0],
        last_dimension: DimensionKind::default(),
        inventory: None,
        skill_set: SkillSet::default(),
        known_techniques: KnownTechniques::default(),
        ui_prefs: serde_json::json!({
            "quick_slots": [0, 1, 2]
        }),
    };

    let error = import_player_bundle(&persistence, &bundle)
        .expect_err("invalid ui_prefs should be rejected");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);

    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    let player_core_exists: Option<String> = connection
        .query_row(
            "SELECT username FROM player_core WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .optional()
        .expect("player_core query should succeed");
    let player_slow_exists: Option<String> = connection
        .query_row(
            "SELECT username FROM player_slow WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .optional()
        .expect("player_slow query should succeed");
    let inventories_exists: Option<String> = connection
        .query_row(
            "SELECT username FROM inventories WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .optional()
        .expect("inventories query should succeed");
    let prefs_exists: Option<String> = connection
        .query_row(
            "SELECT username FROM player_ui_prefs WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .optional()
        .expect("player_ui_prefs query should succeed");

    assert!(player_core_exists.is_none());
    assert!(player_slow_exists.is_none());
    assert!(inventories_exists.is_none());
    assert!(prefs_exists.is_none());

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn computes_composite_power() {
    let state = PlayerState {
        karma: 0.25,
        inventory_score: 0.4,
    };

    let cultivation = Cultivation {
        realm: Realm::Induce,
        qi_current: 60.0,
        qi_max: 100.0,
        ..Cultivation::default()
    };

    let breakdown = state.power_breakdown(&cultivation);
    approx_eq(breakdown.combat, 0.39);
    approx_eq(breakdown.wealth, 0.4);
    approx_eq(breakdown.social, 0.4);
    approx_eq(breakdown.karma, 0.25);
    approx_eq(breakdown.territory, 0.325);
    approx_eq(state.composite_power(&cultivation), 0.36225);
}

#[test]
fn concurrent_player_core_slice_writers_serialize_under_sqlite_busy_timeout() {
    let (persistence, data_dir) = sqlite_persistence("core-slice-concurrency");
    let writer_count = 50usize;
    let baseline_state = PlayerState {
        karma: 0.1,
        inventory_score: 0.2,
    };

    for index in 0..writer_count {
        save_player_state(
            &persistence,
            format!("Player{index}").as_str(),
            &baseline_state,
        )
        .expect("baseline player state should persist");
    }

    let persistence = Arc::new(persistence);
    let barrier = Arc::new(Barrier::new(writer_count + 1));
    let handles = (0..writer_count)
        .map(|index| {
            let persistence = Arc::clone(&persistence);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let username = format!("Player{index}");
                let updated_state = PlayerState {
                    karma: ((index as f64 / 25.0) - 1.0).clamp(-1.0, 1.0),
                    inventory_score: (index as f64 / writer_count as f64).clamp(0.0, 1.0),
                };

                barrier.wait();
                save_player_core_slice(persistence.as_ref(), username.as_str(), &updated_state)
            })
        })
        .collect::<Vec<_>>();

    barrier.wait();
    let errors = handles
        .into_iter()
        .map(|handle| handle.join().expect("writer thread should not panic"))
        .filter_map(Result::err)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "all concurrent player core slice writers should succeed: {errors:?}"
    );

    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    let row_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM player_core", [], |row| row.get(0))
        .expect("player_core row count should be readable");
    assert_eq!(row_count, writer_count as i64);

    for index in 0..writer_count {
        let username = format!("Player{index}");
        let (karma, inventory_score): (f64, f64) = connection
            .query_row(
                "
                SELECT karma, inventory_score
                FROM player_core
                WHERE username = ?1
                ",
                params![username.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("updated player_core row should exist");

        assert_eq!(karma, ((index as f64 / 25.0) - 1.0).clamp(-1.0, 1.0));
        assert_eq!(
            inventory_score,
            (index as f64 / writer_count as f64).clamp(0.0, 1.0)
        );
    }

    let _ = fs::remove_dir_all(&data_dir);
}

// ─── plan-coffin-tiers-v1 P0 charge #2/#13 ──────────────────────────
// save_player_lifespan_slice（无棺上下文）不能把已存的 Jade/Stone/Bronze grade 洗成 Mundane

#[test]
fn save_player_lifespan_slice_preserves_jade_grade_on_no_coffin_context_save() {
    let (persistence, data_dir) = sqlite_persistence("lifespan-jade-grade-preserve");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("baseline player state should persist");

    // 先存一个 Jade 棺玩家
    let lifespan = bong_server::cultivation::lifespan::LifespanComponent {
        born_at_tick: 0,
        years_lived: 10.0,
        cap_by_realm: 100,
        offline_pause_tick: None,
    };
    save_player_lifespan_slice_with_coffin(
        &persistence,
        "Azure",
        &lifespan,
        Some(CoffinGrade::Jade),
    )
    .expect("save with jade coffin should succeed");

    // 验证 DB 里 grade=jade
    {
        let conn = Connection::open(persistence.db_path()).expect("db should open");
        let grade: String = conn
            .query_row(
                "SELECT coffin_grade FROM player_lifespan WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("grade row should exist");
        assert_eq!(grade, "jade", "grade should be jade after save_with_coffin");
    }

    // 触发无棺上下文保存（模拟悟道延寿路径）
    save_player_lifespan_slice(&persistence, "Azure", &lifespan)
        .expect("save_player_lifespan_slice should succeed");

    // 验证 grade 没有被洗成 mundane
    {
        let conn = Connection::open(persistence.db_path()).expect("db should open");
        let grade: String = conn
            .query_row(
                "SELECT coffin_grade FROM player_lifespan WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("grade row should exist after no-coffin save");
        assert_eq!(
            grade, "jade",
            "save_player_lifespan_slice (无棺上下文) 不应把 jade 洗成 mundane，\
             期望 jade，实际 {grade}"
        );
    }

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn save_player_lifespan_slice_preserves_stone_grade() {
    let (persistence, data_dir) = sqlite_persistence("lifespan-stone-grade-preserve");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("baseline player state should persist");

    let lifespan = bong_server::cultivation::lifespan::LifespanComponent {
        born_at_tick: 0,
        years_lived: 5.0,
        cap_by_realm: 100,
        offline_pause_tick: None,
    };
    save_player_lifespan_slice_with_coffin(
        &persistence,
        "Azure",
        &lifespan,
        Some(CoffinGrade::Stone),
    )
    .expect("save with stone coffin should succeed");
    save_player_lifespan_slice(&persistence, "Azure", &lifespan)
        .expect("save without coffin context should succeed");

    let conn = Connection::open(persistence.db_path()).expect("db should open");
    let grade: String = conn
        .query_row(
            "SELECT coffin_grade FROM player_lifespan WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .expect("grade row should exist");
    assert_eq!(
        grade, "stone",
        "save_player_lifespan_slice 不应洗掉 stone grade，期望 stone，实际 {grade}"
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn save_player_lifespan_slice_preserves_bronze_grade() {
    let (persistence, data_dir) = sqlite_persistence("lifespan-bronze-grade-preserve");
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("baseline player state should persist");

    let lifespan = bong_server::cultivation::lifespan::LifespanComponent {
        born_at_tick: 0,
        years_lived: 5.0,
        cap_by_realm: 100,
        offline_pause_tick: None,
    };
    save_player_lifespan_slice_with_coffin(
        &persistence,
        "Azure",
        &lifespan,
        Some(CoffinGrade::Bronze),
    )
    .expect("save with bronze coffin should succeed");
    save_player_lifespan_slice(&persistence, "Azure", &lifespan)
        .expect("save without coffin context should succeed");

    let conn = Connection::open(persistence.db_path()).expect("db should open");
    let grade: String = conn
        .query_row(
            "SELECT coffin_grade FROM player_lifespan WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .expect("grade row should exist");
    assert_eq!(
        grade, "bronze",
        "save_player_lifespan_slice 不应洗掉 bronze grade，期望 bronze，实际 {grade}"
    );

    let _ = fs::remove_dir_all(&data_dir);
}

// ─── F21 — clear_coffin_flag_for_username (断连时无 LifespanComponent 兜底) ───

#[test]
fn clear_coffin_flag_for_username_zeroes_in_coffin_and_resets_grade() {
    let (persistence, data_dir) = sqlite_persistence("clear-coffin-flag-happy-path");
    let lifespan = bong_server::cultivation::lifespan::LifespanComponent {
        born_at_tick: 0,
        years_lived: 3.0,
        cap_by_realm: 100,
        offline_pause_tick: None,
    };
    save_player_lifespan_slice_with_coffin(
        &persistence,
        "Azure",
        &lifespan,
        Some(CoffinGrade::Bronze),
    )
    .expect("seeding an in-coffin row should succeed");

    {
        let conn = Connection::open(persistence.db_path()).expect("db should open");
        let (in_coffin, grade): (i64, String) = conn
            .query_row(
                "SELECT in_coffin, coffin_grade FROM player_lifespan WHERE username = ?1",
                params!["Azure"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("seed row should exist");
        assert_eq!(
            in_coffin, 1,
            "sanity check: seed row must start in_coffin=1"
        );
        assert_eq!(
            grade, "bronze",
            "sanity check: seed row must start grade=bronze"
        );
    }

    clear_coffin_flag_for_username(&persistence, "Azure")
        .expect("clearing an existing row should succeed");

    let conn = Connection::open(persistence.db_path()).expect("db should open");
    let (in_coffin, grade): (i64, String) = conn
        .query_row(
            "SELECT in_coffin, coffin_grade FROM player_lifespan WHERE username = ?1",
            params!["Azure"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("row should still exist after clearing");
    assert_eq!(
        in_coffin, 0,
        "F21: in_coffin must be zeroed so the join-time re-pin check \
         (`persisted.in_coffin`) does not fire on a coffin that may no longer exist"
    );
    assert_eq!(
        grade,
        CoffinGrade::default().as_db_str(),
        "coffin_grade must reset to the NOT NULL column default ('mundane'), not NULL — the \
         column has no NULL representation (NOT NULL DEFAULT 'mundane')"
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn clear_coffin_flag_for_username_is_a_noop_when_no_row_exists() {
    let (persistence, data_dir) = sqlite_persistence("clear-coffin-flag-noop-no-row");

    // 没有先 save_player_lifespan_slice_with_coffin 播种任何行。
    clear_coffin_flag_for_username(&persistence, "GhostUser")
        .expect("clearing a nonexistent row must succeed (0 rows affected), not error");

    let conn = Connection::open(persistence.db_path()).expect("db should open");
    let row_exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM player_lifespan WHERE username = ?1",
            params!["GhostUser"],
            |row| row.get(0),
        )
        .optional()
        .expect("query should not error");
    assert!(
        row_exists.is_none(),
        "F21: clearing a username with no player_lifespan row must not insert a new \
         (incomplete) row — `UPDATE ... WHERE username = ?1` on 0 matching rows is a true no-op"
    );

    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn clear_coffin_flag_for_username_only_touches_the_target_username() {
    let (persistence, data_dir) = sqlite_persistence("clear-coffin-flag-isolation");
    let lifespan = bong_server::cultivation::lifespan::LifespanComponent {
        born_at_tick: 0,
        years_lived: 1.0,
        cap_by_realm: 100,
        offline_pause_tick: None,
    };
    save_player_lifespan_slice_with_coffin(
        &persistence,
        "Azure",
        &lifespan,
        Some(CoffinGrade::Jade),
    )
    .expect("seeding Azure's in-coffin row should succeed");
    save_player_lifespan_slice_with_coffin(
        &persistence,
        "Bystander",
        &lifespan,
        Some(CoffinGrade::Stone),
    )
    .expect("seeding Bystander's in-coffin row should succeed");

    clear_coffin_flag_for_username(&persistence, "Azure")
        .expect("clearing Azure's row should succeed");

    let conn = Connection::open(persistence.db_path()).expect("db should open");
    let (bystander_in_coffin, bystander_grade): (i64, String) = conn
        .query_row(
            "SELECT in_coffin, coffin_grade FROM player_lifespan WHERE username = ?1",
            params!["Bystander"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("Bystander row should still exist");
    assert_eq!(
        bystander_in_coffin, 1,
        "F21: clearing Azure's coffin flag must not touch Bystander's row"
    );
    assert_eq!(
        bystander_grade, "stone",
        "F21: clearing Azure's coffin flag must not touch Bystander's grade"
    );

    let _ = fs::remove_dir_all(&data_dir);
}
