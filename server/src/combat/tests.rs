use crate::combat::{
    attach_combat_bundle_to_joined_clients,
    components::{Lifecycle, LifecycleState, RevivalDecision},
    is_damageable,
};
use crate::persistence::bootstrap_sqlite;
use crate::player::state::{
    load_current_character_id, player_character_id, save_player_lifecycle_slice,
    save_player_shrine_anchor_slice, save_player_state, PlayerStatePersistence,
};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use valence::prelude::{App, Entity, GameMode, Query, Res, Update, Username};
use valence::testing::create_mock_client;

fn unique_temp_dir(test_name: &str) -> PathBuf {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "bong-combat-mod-{test_name}-{}-{unique_suffix}",
        std::process::id()
    ))
}

#[derive(Clone, Copy)]
struct DamageabilityFixtures {
    no_mode: Entity,
    survival: Entity,
    creative: Entity,
    adventure: Entity,
    spectator: Entity,
}

impl valence::prelude::Resource for DamageabilityFixtures {}

fn assert_damageability(fixtures: Res<DamageabilityFixtures>, modes: Query<&GameMode>) {
    assert!(is_damageable(fixtures.no_mode, &modes));
    assert!(is_damageable(fixtures.survival, &modes));
    assert!(!is_damageable(fixtures.creative, &modes));
    assert!(!is_damageable(fixtures.adventure, &modes));
    assert!(!is_damageable(fixtures.spectator, &modes));
}

#[test]
fn damageable_gate_only_allows_survival_or_non_player_entities() {
    let mut app = App::new();
    let no_mode = app.world_mut().spawn_empty().id();
    let survival = app.world_mut().spawn(GameMode::Survival).id();
    let creative = app.world_mut().spawn(GameMode::Creative).id();
    let adventure = app.world_mut().spawn(GameMode::Adventure).id();
    let spectator = app.world_mut().spawn(GameMode::Spectator).id();
    app.insert_resource(DamageabilityFixtures {
        no_mode,
        survival,
        creative,
        adventure,
        spectator,
    });
    app.add_systems(Update, assert_damageability);

    app.update();
}

#[test]
fn joined_client_hydrates_shrine_anchor_from_sqlite_when_present() {
    let root = unique_temp_dir("hydrates-shrine-anchor");
    let data_dir = root.join("data");
    std::fs::create_dir_all(&data_dir).expect("data dir should create");
    let db_path = data_dir.join("bong.db");

    bootstrap_sqlite(&db_path, "combat-mod-hydrates").expect("sqlite bootstrap should succeed");
    let persistence = PlayerStatePersistence::with_db_path(&data_dir, &db_path);

    save_player_shrine_anchor_slice(&persistence, "Alice", Some([11.0, 22.0, 33.0]))
        .expect("save shrine anchor should succeed");

    let mut app = App::new();
    app.insert_resource(persistence);
    app.add_systems(Update, attach_combat_bundle_to_joined_clients);

    let (client_bundle, _helper) = create_mock_client("Alice");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.update();

    let lifecycle = app
        .world()
        .get::<Lifecycle>(entity)
        .expect("joined client should receive Lifecycle");
    assert_eq!(lifecycle.spawn_anchor, Some([11.0, 22.0, 33.0]));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn joined_client_hydrates_rotated_character_id_from_sqlite_when_present() {
    let root = unique_temp_dir("hydrates-character-id");
    let data_dir = root.join("data");
    std::fs::create_dir_all(&data_dir).expect("data dir should create");
    let db_path = data_dir.join("bong.db");

    bootstrap_sqlite(&db_path, "combat-mod-character-id").expect("sqlite bootstrap should succeed");
    let persistence = PlayerStatePersistence::with_db_path(&data_dir, &db_path);

    save_player_state(&persistence, "Alice", &Default::default())
        .expect("save player should initialize current_char_id");
    let current_char_id = crate::player::state::rotate_current_character_id(&persistence, "Alice")
        .expect("rotating current_char_id should succeed");

    let mut app = App::new();
    app.insert_resource(persistence);
    app.add_systems(Update, attach_combat_bundle_to_joined_clients);

    let (client_bundle, _helper) = create_mock_client("Alice");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.update();

    let lifecycle = app
        .world()
        .get::<Lifecycle>(entity)
        .expect("joined client should receive Lifecycle");
    assert_eq!(
        lifecycle.character_id,
        player_character_id("Alice", &current_char_id)
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn joined_client_has_no_shrine_anchor_when_missing_in_sqlite() {
    let root = unique_temp_dir("missing-shrine-anchor");
    let data_dir = root.join("data");
    std::fs::create_dir_all(&data_dir).expect("data dir should create");
    let db_path = data_dir.join("bong.db");
    bootstrap_sqlite(&db_path, "combat-mod-missing").expect("sqlite bootstrap should succeed");
    let persistence = PlayerStatePersistence::with_db_path(&data_dir, &db_path);

    let mut app = App::new();
    app.insert_resource(persistence);
    app.add_systems(Update, attach_combat_bundle_to_joined_clients);

    let (client_bundle, _helper) = create_mock_client("Bob");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.update();

    let lifecycle = app
        .world()
        .get::<Lifecycle>(entity)
        .expect("joined client should receive Lifecycle");
    assert_eq!(lifecycle.spawn_anchor, None);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn joined_client_hydrates_shrine_anchor_from_sqlite_with_optional_resource() {
    // Regression: attach_combat_bundle_to_joined_clients takes Option<Res<PlayerStatePersistence>>.
    // Ensure it still attaches the combat bundle even if persistence is missing.
    let mut app = App::new();
    app.add_systems(Update, attach_combat_bundle_to_joined_clients);

    let (client_bundle, _helper) = create_mock_client("NoDb");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.update();

    let username = app
        .world()
        .get::<Username>(entity)
        .expect("mock client should have Username");
    assert_eq!(username.0.as_str(), "NoDb");
    let lifecycle = app
        .world()
        .get::<Lifecycle>(entity)
        .expect("joined client should receive Lifecycle");
    assert_eq!(lifecycle.spawn_anchor, None);
}

// ── bughunt player-lifecycle-relog-death-consequence-wipe ──
//
// server/src/combat/mod.rs:114-131 (attach_combat_bundle_to_joined_clients) previously
// inserted `Lifecycle::default()` unconditionally for every newly-joined client, wiping
// any persisted death/revival state machine (state / fortune_remaining / awaiting_decision
// / deadline ticks) on every reconnect. These tests lock the fixed behavior: a matching
// persisted slice must be reused without resetting state, a stale/foreign character_id must NOT be
// reused, spawn_anchor must still come from the authoritative shrine table (not a stale JSON
// snapshot), deadlines must retain their remaining window after wall-clock rebasing, and the
// "never persisted" path must still fall back to a fresh default.

/// Seeds `player_core` for `username` and returns the freshly-computed "current character"
/// id exactly as `attach_combat_bundle_to_joined_clients` would compute it.
fn seed_current_character_id(persistence: &PlayerStatePersistence, username: &str) -> String {
    save_player_state(persistence, username, &Default::default())
        .expect("save player should initialize current_char_id");
    let current_char_id = load_current_character_id(persistence, username)
        .expect("load current_char_id should succeed")
        .expect("current_char_id should exist after save_player_state");
    player_character_id(username, &current_char_id)
}

#[test]
fn joined_client_hydrates_persisted_lifecycle_state_with_zero_fortune_and_pending_tribulation() {
    let root = unique_temp_dir("hydrates-lifecycle-zero-fortune");
    let data_dir = root.join("data");
    std::fs::create_dir_all(&data_dir).expect("data dir should create");
    let db_path = data_dir.join("bong.db");
    bootstrap_sqlite(&db_path, "combat-mod-lifecycle-hydrate")
        .expect("sqlite bootstrap should succeed");
    let persistence = PlayerStatePersistence::with_db_path(&data_dir, &db_path);

    let character_id = seed_current_character_id(&persistence, "Alice");
    let persisted = Lifecycle {
        character_id: character_id.clone(),
        death_count: 2,
        fortune_remaining: 0,
        last_death_tick: Some(400),
        last_revive_tick: None,
        spawn_anchor: Some([1.0, 2.0, 3.0]),
        spawn_anchor_damaged: false,
        near_death_deadline_tick: None,
        awaiting_decision: Some(RevivalDecision::Tribulation { chance: 0.15 }),
        revival_decision_deadline_tick: Some(9_999),
        weakened_until_tick: None,
        state: LifecycleState::AwaitingRevival,
    };
    let before_save_wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs() as i64;
    save_player_lifecycle_slice(&persistence, "Alice", &persisted, 0)
        .expect("save lifecycle slice should succeed");
    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    let persisted_last_updated_wall: i64 = connection
        .query_row(
            "SELECT last_updated_wall FROM player_lifecycle WHERE username = ?1",
            params!["Alice"],
            |row| row.get(0),
        )
        .expect("saved lifecycle row should expose its persistence timestamp");

    let mut app = App::new();
    app.insert_resource(persistence);
    app.add_systems(Update, attach_combat_bundle_to_joined_clients);

    let (client_bundle, _helper) = create_mock_client("Alice");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.update();

    let lifecycle = app
        .world()
        .get::<Lifecycle>(entity)
        .expect("joined client should receive Lifecycle");

    assert_eq!(lifecycle.character_id, character_id);
    assert_eq!(
        lifecycle.fortune_remaining, 0,
        "断线前已耗尽的运气次数必须原样恢复，不能被 Lifecycle::default() 洗回 3"
    );
    assert_eq!(
        lifecycle.state,
        LifecycleState::AwaitingRevival,
        "重连必须回到断线前的 AwaitingRevival 决策窗口，不能被静默复活成 Alive"
    );
    assert_eq!(
        lifecycle.awaiting_decision,
        Some(RevivalDecision::Tribulation { chance: 0.15 }),
        "待决策的渡劫结果必须原样恢复，永久终结风险不能被绕过"
    );
    let after_load_wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs() as i64;
    // 生产折算锚点是落盘行的 last_updated_wall（translate_lifecycle_deadline_tick_across_restart），
    // 断言必须与生产同锚点；保存前采样仅作 fixture 顺序不变量。
    assert!(
        before_save_wall <= persisted_last_updated_wall,
        "fixture 前置：落盘时间戳不得早于保存前采样；before_save {before_save_wall}，persisted {persisted_last_updated_wall}"
    );
    let max_elapsed_ticks = after_load_wall
        .saturating_sub(persisted_last_updated_wall)
        .max(0) as u64
        * crate::combat::components::TICKS_PER_SECOND;
    let earliest_valid_deadline = 9_999_u64.saturating_sub(max_elapsed_ticks);
    let loaded_deadline = lifecycle
        .revival_decision_deadline_tick
        .expect("awaiting revival deadline should survive hydration");
    assert!(
        (earliest_valid_deadline..=9_999).contains(&loaded_deadline),
        "重连应保留决策窗口并只扣除持久化时间戳后的真实墙钟流逝；实际 {loaded_deadline}，有效区间 {earliest_valid_deadline}..=9999",
    );
    assert_eq!(lifecycle.death_count, 2);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn joined_client_ignores_persisted_lifecycle_from_previous_life_after_character_id_rotates() {
    // 转生 / 老档场景：player_lifecycle 里遗留的 character_id 对不上刚计算出的"当前
    // 角色" id 时不能被复用——否则上一世的濒死/复活状态会错误地套到新角色身上。
    let root = unique_temp_dir("ignores-stale-lifecycle-character-id");
    let data_dir = root.join("data");
    std::fs::create_dir_all(&data_dir).expect("data dir should create");
    let db_path = data_dir.join("bong.db");
    bootstrap_sqlite(&db_path, "combat-mod-lifecycle-stale")
        .expect("sqlite bootstrap should succeed");
    let persistence = PlayerStatePersistence::with_db_path(&data_dir, &db_path);

    let current_character_id = seed_current_character_id(&persistence, "Alice");
    let stale_lifecycle = Lifecycle {
        character_id: "offline:Alice:previous-life".to_string(),
        fortune_remaining: 0,
        state: LifecycleState::AwaitingRevival,
        awaiting_decision: Some(RevivalDecision::Tribulation { chance: 0.05 }),
        revival_decision_deadline_tick: Some(1),
        ..Lifecycle::default()
    };
    assert_ne!(
        stale_lifecycle.character_id, current_character_id,
        "test fixture 前置条件：陈旧 character_id 必须确实不同于当前角色 id"
    );
    save_player_lifecycle_slice(&persistence, "Alice", &stale_lifecycle, 0)
        .expect("save stale lifecycle slice should succeed");

    let mut app = App::new();
    app.insert_resource(persistence);
    app.add_systems(Update, attach_combat_bundle_to_joined_clients);

    let (client_bundle, _helper) = create_mock_client("Alice");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.update();

    let lifecycle = app
        .world()
        .get::<Lifecycle>(entity)
        .expect("joined client should receive Lifecycle");

    assert_eq!(
        lifecycle.character_id, current_character_id,
        "新角色必须拿到刚计算出的当前 character_id，不是上一世遗留的值"
    );
    assert_eq!(
        lifecycle.state,
        LifecycleState::Alive,
        "character_id 不匹配时必须回退默认 Alive，不能继承上一世的 AwaitingRevival"
    );
    assert_eq!(
        lifecycle.fortune_remaining, 3,
        "character_id 不匹配时必须回退默认满运气次数，不能继承上一世耗尽的 0"
    );
    assert_eq!(lifecycle.awaiting_decision, None);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn joined_client_lifecycle_spawn_anchor_prefers_shrine_table_over_stale_json_snapshot() {
    // spawn_anchor 有独立的、更权威的 player_shrine 存储；Lifecycle JSON 快照里可能
    // 携带断连那一刻的旧值，不能让它反过来覆盖刚查出的权威灵龛坐标（双重真相源）。
    let root = unique_temp_dir("lifecycle-spawn-anchor-precedence");
    let data_dir = root.join("data");
    std::fs::create_dir_all(&data_dir).expect("data dir should create");
    let db_path = data_dir.join("bong.db");
    bootstrap_sqlite(&db_path, "combat-mod-lifecycle-anchor")
        .expect("sqlite bootstrap should succeed");
    let persistence = PlayerStatePersistence::with_db_path(&data_dir, &db_path);

    let character_id = seed_current_character_id(&persistence, "Alice");
    save_player_shrine_anchor_slice(&persistence, "Alice", Some([100.0, 64.0, 200.0]))
        .expect("save shrine anchor should succeed");
    let persisted = Lifecycle {
        character_id: character_id.clone(),
        spawn_anchor: Some([1.0, 1.0, 1.0]), // 断连时刻的旧快照，必须被覆盖
        ..Lifecycle::default()
    };
    save_player_lifecycle_slice(&persistence, "Alice", &persisted, 0)
        .expect("save lifecycle slice should succeed");

    let mut app = App::new();
    app.insert_resource(persistence);
    app.add_systems(Update, attach_combat_bundle_to_joined_clients);

    let (client_bundle, _helper) = create_mock_client("Alice");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.update();

    let lifecycle = app
        .world()
        .get::<Lifecycle>(entity)
        .expect("joined client should receive Lifecycle");
    assert_eq!(
        lifecycle.spawn_anchor,
        Some([100.0, 64.0, 200.0]),
        "spawn_anchor 必须以 player_shrine 表的权威值为准，不能被 Lifecycle JSON 里的\
         过期快照覆盖"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn joined_client_defaults_lifecycle_when_no_lifecycle_row_ever_persisted() {
    // 首次登录（从未死过、从未落过 player_lifecycle 行）必须仍然拿到干净的默认值——
    // 这个 bug 的修复不能反过来破坏"从没死过的新玩家"这条最常见路径。
    let root = unique_temp_dir("lifecycle-defaults-when-missing");
    let data_dir = root.join("data");
    std::fs::create_dir_all(&data_dir).expect("data dir should create");
    let db_path = data_dir.join("bong.db");
    bootstrap_sqlite(&db_path, "combat-mod-lifecycle-defaults")
        .expect("sqlite bootstrap should succeed");
    let persistence = PlayerStatePersistence::with_db_path(&data_dir, &db_path);
    let character_id = seed_current_character_id(&persistence, "Alice");

    let mut app = App::new();
    app.insert_resource(persistence);
    app.add_systems(Update, attach_combat_bundle_to_joined_clients);

    let (client_bundle, _helper) = create_mock_client("Alice");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.update();

    let lifecycle = app
        .world()
        .get::<Lifecycle>(entity)
        .expect("joined client should receive Lifecycle");
    assert_eq!(lifecycle.character_id, character_id);
    assert_eq!(lifecycle.state, LifecycleState::Alive);
    assert_eq!(lifecycle.fortune_remaining, 3);
    assert_eq!(lifecycle.awaiting_decision, None);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn joined_client_falls_back_to_default_lifecycle_when_persisted_row_is_corrupt() {
    // bughunt player-lifecycle-relog-death-consequence-wipe（OPUS 返工要求 4）：反序列化
    // 失败（损坏的 lifecycle_json）之前会被 `.ok().flatten()` 静默吞掉——与本 bug 同一失效
    // 类（濒死/待复活状态被无声抹除）。修复后必须 warn! 留痕（见
    // attach_combat_bundle_to_joined_clients 实现），但 join 流程本身绝不能 panic 或卡死，
    // 必须优雅回退到 Lifecycle::default()。
    let root = unique_temp_dir("corrupt-lifecycle-row");
    let data_dir = root.join("data");
    std::fs::create_dir_all(&data_dir).expect("data dir should create");
    let db_path = data_dir.join("bong.db");
    bootstrap_sqlite(&db_path, "combat-mod-lifecycle-corrupt")
        .expect("sqlite bootstrap should succeed");
    let persistence = PlayerStatePersistence::with_db_path(&data_dir, &db_path);
    let character_id = seed_current_character_id(&persistence, "Alice");

    // 直接写一行损坏的 lifecycle_json（不经过 save_player_lifecycle_slice，后者永远只会
    // 序列化出合法 JSON——这里模拟磁盘损坏 / 手工改坏数据的场景）。
    let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
    connection
        .execute(
            "
            INSERT INTO player_lifecycle (
                username, lifecycle_json, schema_version, last_updated_wall,
                combat_clock_tick_at_save
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params!["Alice", "{not valid json", 2_i32, 0_i64, 0_u64],
        )
        .expect("corrupt lifecycle fixture row should insert");

    let mut app = App::new();
    app.insert_resource(persistence);
    app.add_systems(Update, attach_combat_bundle_to_joined_clients);

    let (client_bundle, _helper) = create_mock_client("Alice");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.update();

    let lifecycle = app
        .world()
        .get::<Lifecycle>(entity)
        .expect("joined client should still receive a Lifecycle despite the corrupt row");
    assert_eq!(lifecycle.character_id, character_id);
    assert_eq!(
        lifecycle.state,
        LifecycleState::Alive,
        "损坏行必须回退到默认 Alive，而不是 panic 或卡住 join 流程"
    );
    assert_eq!(lifecycle.fortune_remaining, 3);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn joined_client_settles_expired_awaiting_revival_deadline_after_combat_clock_restart() {
    // bughunt player-lifecycle-relog-death-consequence-wipe（OPUS 返工要求 1）端到端锁：
    // 通过完整的 attach_combat_bundle_to_joined_clients 系统（不是直接调用
    // player::state 的私有折算函数）验证跨重启折算确实接线到了 join 路径。断线前
    // CombatClock.tick=500_000，60 秒决策窗口 deadline=501_200，落盘时刻是 5 分钟前
    // （远超 60 秒窗口）；重启后新 App 里 CombatClock.tick=0——折算后的 deadline 必须落在
    // 0 附近（已过期），而不是原样保留 501_200（~7 小时后才会被 auto_confirm_revival_
    // decisions 结算，期间玩家会卡在无 UI 的 AwaitingRevival 无敌状态）。
    let root = unique_temp_dir("combat-clock-restart-settles-deadline");
    let data_dir = root.join("data");
    std::fs::create_dir_all(&data_dir).expect("data dir should create");
    let db_path = data_dir.join("bong.db");
    bootstrap_sqlite(&db_path, "combat-mod-lifecycle-restart")
        .expect("sqlite bootstrap should succeed");
    let persistence = PlayerStatePersistence::with_db_path(&data_dir, &db_path);
    let character_id = seed_current_character_id(&persistence, "Alice");

    let persisted = Lifecycle {
        character_id: character_id.clone(),
        state: LifecycleState::AwaitingRevival,
        awaiting_decision: Some(RevivalDecision::Tribulation { chance: 0.4 }),
        revival_decision_deadline_tick: Some(501_200),
        ..Lifecycle::default()
    };
    let lifecycle_json = serde_json::to_string(&persisted).expect("lifecycle should serialize");
    let last_updated_wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs() as i64
        - 300;
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
                "Alice",
                lifecycle_json,
                2_i32,
                last_updated_wall,
                500_000_u64
            ],
        )
        .expect("lifecycle fixture row should insert");

    let mut app = App::new();
    app.insert_resource(persistence);
    // 模拟进程重启：新 App 里的 CombatClock 从 0 开始（`combat::mod::register` 的默认值）。
    app.insert_resource(crate::combat::CombatClock::default());
    app.add_systems(Update, attach_combat_bundle_to_joined_clients);

    let (client_bundle, _helper) = create_mock_client("Alice");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.update();

    let lifecycle = app
        .world()
        .get::<Lifecycle>(entity)
        .expect("joined client should receive Lifecycle");

    assert_eq!(
        lifecycle.state,
        LifecycleState::AwaitingRevival,
        "跨重启不应该丢失 AwaitingRevival 状态本身，只应该折算 deadline"
    );
    assert_eq!(
        lifecycle.revival_decision_deadline_tick,
        Some(0),
        "决策窗口早已在墙钟层面过期（落盘 5 分钟前，60 秒窗口早已结束），join 路径读回的\
         deadline 必须落在重启后的 CombatClock.tick(0) 上，实际 {:?}——否则\
         auto_confirm_revival_decisions 会把它当成 501_200 tick（~7 小时）之后才到期",
        lifecycle.revival_decision_deadline_tick
    );

    let _ = std::fs::remove_dir_all(root);
}
