#![allow(dead_code, unused_imports)]

use std::fs;
use std::path::PathBuf;

use bong_server::craft::events::{InsightTrigger, UnlockEventSource};
use bong_server::craft::recipe::{
    CraftCategory, CraftRecipe, CraftRequirements, RecipeId, UnlockSource,
};
use bong_server::craft::registry::CraftRegistry;
use bong_server::craft::unlock::*;
use valence::prelude::{App, AppExit, Last, Update};

fn recipe_with_sources(sources: Vec<UnlockSource>) -> CraftRecipe {
    CraftRecipe {
        id: RecipeId::new("craft.example.test"),
        category: CraftCategory::Misc,
        display_name: "测试".into(),
        materials: vec![("herb_a".into(), 1)],
        qi_cost: 1.0,
        time_ticks: 60,
        output: ("test_out".into(), 1),
        requirements: CraftRequirements::default(),
        unlock_sources: sources,
        station: None,
    }
}

/// 无显式解锁来源（材料发现路径）的配方，可指定 id + 原料清单。
fn empty_source_recipe(id: &str, materials: &[(&str, u32)]) -> CraftRecipe {
    CraftRecipe {
        id: RecipeId::new(id),
        category: CraftCategory::Tool,
        display_name: id.into(),
        materials: materials
            .iter()
            .map(|(t, c)| ((*t).to_string(), *c))
            .collect(),
        qi_cost: 0.0,
        time_ticks: 60,
        output: ("test_out".into(), 1),
        requirements: CraftRequirements::default(),
        unlock_sources: vec![],
        station: None,
    }
}

fn unique_tmp_path(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("bong-craft-recipe-unlocks-{stamp}-{name}.json"))
}

fn app_with_unlock_flush_systems(state: RecipeUnlockState) -> App {
    let mut app = App::new();
    app.add_event::<AppExit>();
    app.insert_resource(state);
    app.add_systems(Update, tick_recipe_unlock_flush);
    app.add_systems(Last, flush_recipe_unlocks_on_shutdown);
    app
}

#[test]
fn unlock_state_default_player_not_unlocked() {
    let state = RecipeUnlockState::new();
    assert!(!state.is_unlocked("offline:Alice", &RecipeId::new("anything")));
    assert_eq!(state.unlocked_count("offline:Alice"), 0);
}

// ── 基线常显豁免（BASELINE_RECIPES / is_baseline_recipe）──────────

#[test]
fn baseline_workbench_recipe_always_unlocked_for_unregistered_player() {
    let state = RecipeUnlockState::new();
    assert!(
        state.is_unlocked("offline:Alice", &RecipeId::new("craft.tool.workbench")),
        "制作台自身配方必须对从未注册的玩家恒解锁 —— 它是 workbench 配方树的入口，\
             走材料发现会被自己的原料锁死"
    );
}

#[test]
fn baseline_exemption_is_exact_id_match_not_prefix() {
    let state = RecipeUnlockState::new();
    for near_miss in [
        "craft.tool.workbench2",
        "craft.tool.workbench.fake",
        "craft.tool.workbenc",
    ] {
        assert!(
            !state.is_unlocked("offline:Alice", &RecipeId::new(near_miss)),
            "基线豁免必须精确匹配 id，近似 id `{near_miss}` 不应被误豁免"
        );
    }
}

#[test]
fn baseline_recipe_survives_clear_for_player() {
    // 死亡重生清空 unlock state 后基线配方仍常显（豁免不依赖 by_player 存储）。
    let mut state = RecipeUnlockState::new();
    state.unlock("offline:Alice", RecipeId::new("craft.example.a"));
    state.clear_for_player("offline:Alice");
    assert!(
        state.is_unlocked("offline:Alice", &RecipeId::new("craft.tool.workbench")),
        "clear_for_player 之后基线配方必须仍然解锁 —— 常显豁免不能被死亡重生清掉"
    );
    assert!(
        !state.is_unlocked("offline:Alice", &RecipeId::new("craft.example.a")),
        "非基线配方仍应被 clear_for_player 正常清空"
    );
}

#[test]
fn baseline_recipe_not_counted_nor_persisted() {
    // 豁免走谓词而非写入 by_player：不占 unlocked_count、不进落盘文件。
    let path = unique_tmp_path("baseline_not_persisted");
    let mut state = RecipeUnlockState::default().with_path(&path);
    assert_eq!(
        state.unlocked_count("offline:Alice"),
        0,
        "基线豁免不应计入 unlocked_count（它不在 by_player 存储里）"
    );
    assert!(
        !state.is_dirty(),
        "仅查询基线配方不应标 dirty —— is_unlocked 是只读谓词"
    );
    state.unlock("offline:Alice", RecipeId::new("craft.example.a"));
    state.flush().expect("flush should succeed");
    let loaded = load_recipe_unlock_log(&path).expect("load should parse");
    assert!(
        !loaded
            .by_player
            .get("offline:Alice")
            .map(|s| s.contains(&RecipeId::new("craft.tool.workbench")))
            .unwrap_or(false),
        "基线配方不应出现在落盘文件里 —— 豁免是代码谓词，不是持久化状态"
    );
    let _ = fs::remove_file(&path);
}

#[test]
fn baseline_recipe_material_discovery_short_circuits_to_already() {
    // 材料发现渠道对基线配方应短路成 Already（不重复写 state / 不重复刷 UI），
    // 与 apply_material_discovery_unlock 的 is_unlocked 前置短路语义一致。
    let mut state = RecipeUnlockState::new();
    let recipe = empty_source_recipe("craft.tool.workbench", &[("spirit_wood", 4)]);
    assert_eq!(
        unlock_via_material(&mut state, "offline:Alice", &recipe, "spirit_wood"),
        MaterialUnlockOutcome::Already,
        "基线配方走材料发现必须返回 Already（is_unlocked 恒 true），\
             返回 Newly 会把它写进 by_player 并触发多余的列表刷新"
    );
    assert!(!state.is_dirty(), "基线配方的材料发现不应产生任何状态写入");
}

#[test]
fn scroll_unlock_reservation_blocks_duplicates_until_released_or_cleared() {
    let mut state = RecipeUnlockState::new();
    let recipe = RecipeId::new("craft.secret.test");

    assert!(state.reserve_scroll_unlock("offline:Alice", &recipe));
    assert!(!state.reserve_scroll_unlock("offline:Alice", &recipe));
    assert!(state.reserve_scroll_unlock("offline:Bob", &recipe));

    state.release_scroll_unlock_reservation("offline:Alice", &recipe);
    assert!(state.reserve_scroll_unlock("offline:Alice", &recipe));
    state.clear_for_player("offline:Alice");
    assert!(state.reserve_scroll_unlock("offline:Alice", &recipe));

    state.release_scroll_unlock_reservation("offline:Alice", &recipe);
    assert!(state.unlock("offline:Alice", recipe.clone()));
    assert!(!state.reserve_scroll_unlock("offline:Alice", &recipe));
}

#[test]
fn unlock_marks_player_specific() {
    let mut state = RecipeUnlockState::new();
    let id = RecipeId::new("a");
    assert!(state.unlock("offline:Alice", id.clone()));
    assert!(state.is_unlocked("offline:Alice", &id));
    // Bob 不受影响
    assert!(!state.is_unlocked("offline:Bob", &id));
}

#[test]
fn unlock_returns_false_when_already_present() {
    let mut state = RecipeUnlockState::new();
    let id = RecipeId::new("a");
    assert!(state.unlock("offline:Alice", id.clone()));
    assert!(!state.unlock("offline:Alice", id.clone()));
}

#[test]
fn clear_for_player_drops_only_that_player() {
    let mut state = RecipeUnlockState::new();
    state.unlock("offline:Alice", RecipeId::new("a"));
    state.unlock("offline:Bob", RecipeId::new("b"));
    state.clear_for_player("offline:Alice");
    assert_eq!(state.unlocked_count("offline:Alice"), 0);
    assert!(state.is_unlocked("offline:Bob", &RecipeId::new("b")));
}

// ===== 持久化（照抄 mineral::persistence::ExhaustedMineralsLog 测试模式）=====

#[test]
fn new_state_is_not_dirty() {
    let state = RecipeUnlockState::new();
    assert!(
        !state.is_dirty(),
        "freshly constructed state must not be dirty (nothing to flush)"
    );
}

#[test]
fn unlock_marks_dirty_when_actually_new() {
    let mut state = RecipeUnlockState::new();
    assert!(
        !state.is_dirty(),
        "expected a freshly constructed state to be clean because nothing was unlocked yet"
    );
    state.unlock("offline:Alice", RecipeId::new("a"));
    assert!(
        state.is_dirty(),
        "unlock() inserting a new recipe must mark state dirty so flush picks it up"
    );
}

#[test]
fn flush_no_op_when_clean() {
    let path = unique_tmp_path("flush_clean");
    let mut state = RecipeUnlockState::default().with_path(&path);
    // 没 unlock 过 → 不应写文件
    state.flush().expect("clean flush ok");
    assert!(!path.exists(), "clean flush should not create a file");
}

#[test]
fn hydrated_restores_state_from_prior_flush() {
    let path = unique_tmp_path("hydrate_restore");
    // 先 flush 一份到磁盘（模拟上次关服）
    let mut prior = RecipeUnlockState::default().with_path(&path);
    prior.unlock("offline:Alice", RecipeId::new("craft.example.a"));
    prior.unlock("offline:Alice", RecipeId::new("craft.example.b"));
    prior.unlock("offline:Bob", RecipeId::new("craft.example.c"));
    prior.flush().expect("flush should succeed");

    // hydrate 回来
    let restored = RecipeUnlockState::hydrated_from_path(&path);
    assert_eq!(
        restored.unlocked_count("offline:Alice"),
        2,
        "Alice should have both recipes restored after restart"
    );
    assert!(
        restored.is_unlocked("offline:Alice", &RecipeId::new("craft.example.a")),
        "Alice's first flushed recipe (craft.example.a) must survive the flush/hydrate \
             round-trip"
    );
    assert!(
        restored.is_unlocked("offline:Alice", &RecipeId::new("craft.example.b")),
        "Alice's second flushed recipe (craft.example.b) must survive the flush/hydrate \
             round-trip — a single-recipe roundtrip alone would not catch a HashSet truncated to \
             one entry"
    );
    assert!(
        restored.is_unlocked("offline:Bob", &RecipeId::new("craft.example.c")),
        "multi-player state must round-trip independently — Bob's unlock is separate from Alice's"
    );
    assert!(
            !restored.is_unlocked("offline:Bob", &RecipeId::new("craft.example.a")),
            "Bob must not inherit Alice's unlocks after hydrate — per-player isolation must survive persistence"
        );
    assert!(
        !restored.is_dirty(),
        "hydrated state must not be dirty on startup"
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn hydrated_falls_back_when_file_corrupt() {
    let path = unique_tmp_path("hydrate_corrupt");
    fs::write(&path, "corrupted json {{{").unwrap();
    let state = RecipeUnlockState::hydrated_from_path(&path);
    // 坏文件 → 空 state（warn log 已发，启动不阻塞）
    assert_eq!(
        state.player_count(),
        0,
        "expected corrupt JSON to fall back to an empty state (0 players) because startup \
             must not be blocked by a bad log file, actual={}",
        state.player_count()
    );
    assert!(
        !state.is_dirty(),
        "corrupt-file fallback must not be marked dirty, otherwise the next flush would \
             immediately overwrite the corrupt file with an empty one before an operator can \
             inspect it"
    );
    let _ = fs::remove_file(&path);
}

#[test]
fn load_recipe_unlock_log_rejects_invalid_json() {
    let path = unique_tmp_path("invalid_json");
    fs::write(&path, "not valid json").unwrap();
    assert!(
        load_recipe_unlock_log(&path).is_err(),
        "expected load_recipe_unlock_log to return Err for non-JSON content because the \
             loader must fail loudly instead of silently returning a default value"
    );
    let _ = fs::remove_file(&path);
}

#[test]
fn multi_player_flush_hydrate_keeps_each_player_isolated() {
    let path = unique_tmp_path("multi_player_isolation");
    let mut state = RecipeUnlockState::default().with_path(&path);
    state.unlock("offline:Alice", RecipeId::new("craft.example.a"));
    state.unlock("offline:Bob", RecipeId::new("craft.example.b"));
    state.unlock("offline:Carol", RecipeId::new("craft.example.c"));
    state.flush().expect("flush should succeed");

    let restored = RecipeUnlockState::hydrated_from_path(&path);
    assert_eq!(
        restored.player_count(),
        3,
        "expected 3 players (Alice/Bob/Carol) restored because each unlocked exactly one \
             distinct recipe before flush, actual={}",
        restored.player_count()
    );
    assert!(
        restored.is_unlocked("offline:Alice", &RecipeId::new("craft.example.a")),
        "Alice must keep her own unlock (craft.example.a) after multi-player flush/hydrate"
    );
    assert!(
        !restored.is_unlocked("offline:Alice", &RecipeId::new("craft.example.b")),
        "Alice must not inherit Bob's recipe (craft.example.b) — per-player isolation must \
             survive persistence, not just leak-free unlock()"
    );
    assert!(
        restored.is_unlocked("offline:Bob", &RecipeId::new("craft.example.b")),
        "Bob must keep his own unlock (craft.example.b) after multi-player flush/hydrate"
    );
    assert!(
        !restored.is_unlocked("offline:Bob", &RecipeId::new("craft.example.c")),
        "Bob must not inherit Carol's recipe (craft.example.c)"
    );
    assert!(
        restored.is_unlocked("offline:Carol", &RecipeId::new("craft.example.c")),
        "Carol must keep her own unlock (craft.example.c) after multi-player flush/hydrate"
    );
    assert!(
        !restored.is_unlocked("offline:Carol", &RecipeId::new("craft.example.a")),
        "Carol must not inherit Alice's recipe (craft.example.a)"
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn tick_recipe_unlock_flush_only_writes_after_interval_when_dirty() {
    let path = unique_tmp_path("tick_flush_interval");
    let mut app = valence::prelude::App::new();
    let state = RecipeUnlockState::default()
        .with_path(&path)
        .with_flush_interval(3);
    app.insert_resource(state);
    app.add_systems(valence::prelude::Update, tick_recipe_unlock_flush);

    // dirty=false, interval 未到 → 多次 tick 都不应写文件
    app.update();
    app.update();
    assert!(
        !path.exists(),
        "flush system must not write to disk while state is clean"
    );

    // 标记 dirty，但节流窗口还没到（第 3 次 tick 才到期）
    app.world_mut()
        .resource_mut::<RecipeUnlockState>()
        .unlock("offline:Alice", RecipeId::new("craft.example.a"));
    app.update(); // tick 3 of interval-3 window (clock started counting from app.update() calls above)

    // 继续 tick 直到节流窗口耗尽，确保最终确实落盘
    for _ in 0..5 {
        app.update();
        if path.exists() {
            break;
        }
    }
    assert!(
        path.exists(),
        "flush system must eventually persist dirty state once flush_interval_ticks elapses"
    );
    let loaded = load_recipe_unlock_log(&path).expect("load should parse");
    assert!(
        loaded
            .by_player
            .get("offline:Alice")
            .map(|s| s.contains(&RecipeId::new("craft.example.a")))
            .unwrap_or(false),
        "expected the throttled flush to persist Alice's unlock (craft.example.a) to disk \
             because the flush_interval_ticks window elapsed, actual by_player={:?}",
        loaded.by_player
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn tick_recipe_unlock_flush_clears_dirty_after_writing() {
    let path = unique_tmp_path("tick_flush_clears_dirty");
    let mut app = valence::prelude::App::new();
    let state = RecipeUnlockState::default()
        .with_path(&path)
        .with_flush_interval(1);
    app.insert_resource(state);
    app.add_systems(valence::prelude::Update, tick_recipe_unlock_flush);

    app.world_mut()
        .resource_mut::<RecipeUnlockState>()
        .unlock("offline:Alice", RecipeId::new("craft.example.a"));
    app.update();

    let resource = app.world().resource::<RecipeUnlockState>();
    assert!(
        !resource.is_dirty(),
        "after a successful throttled flush the dirty flag must be cleared"
    );

    let _ = fs::remove_file(&path);
}

// ── version pinning (CodeRabbit: schema version must not be silently misread) ──

#[test]
fn load_recipe_unlock_log_rejects_missing_version_field() {
    // No "version" key at all. `RecipeUnlockFile::version` has no `#[serde(default)]`,
    // so this must fail at parse time rather than silently defaulting to 0.
    let path = unique_tmp_path("version_pin_missing_field");
    fs::write(
        &path,
        r#"{"by_player":{"offline:Alice":["craft.example.a"]}}"#,
    )
    .unwrap();
    let result = load_recipe_unlock_log(&path);
    assert!(
        result.is_err(),
        "expected Err when the `version` field is entirely absent because \
             RecipeUnlockFile::version must not silently become 0, actual={result:?}"
    );
    let _ = fs::remove_file(&path);
}

// ── write-failure branch (CodeRabbit: atomic flush must not corrupt existing file) ──

#[test]
fn flush_returns_err_when_parent_directory_cannot_be_created() {
    // Block the parent directory: create a *regular file* at the path flush() would
    // otherwise `create_dir_all` into. mkdir-over-an-existing-file fails regardless of
    // user permissions (even as root), so this reliably forces the write-failure
    // branch across CI environments (unlike chmod-based read-only tricks).
    let blocker = unique_tmp_path("flush_fail_blocker_dir");
    fs::write(&blocker, "i am a file blocking a directory").unwrap();
    let path = blocker.join("nested").join("recipe_unlocks.json");

    let mut state = RecipeUnlockState::default().with_path(&path);
    state.unlock("offline:Alice", RecipeId::new("craft.example.a"));
    let result = state.flush();
    assert!(
        result.is_err(),
        "expected flush() to return Err because its parent directory cannot be created \
             (a regular file occupies that path component), actual={result:?}"
    );
    assert!(
        state.is_dirty(),
        "a failed flush must leave the state dirty so a later retry can still persist the \
             unlock — silently clearing dirty here would permanently lose Alice's unlock"
    );

    let _ = fs::remove_file(&blocker);
}

#[test]
fn flush_does_not_corrupt_existing_file_when_tmp_write_fails() {
    let path = unique_tmp_path("flush_fail_atomic");
    let mut state = RecipeUnlockState::default().with_path(&path);

    // 1) A successful first flush establishes a valid on-disk file.
    state.unlock("offline:Alice", RecipeId::new("craft.example.a"));
    state.flush().expect("first flush should succeed");
    let original_bytes = fs::read_to_string(&path).expect("file must exist after first flush");

    // 2) Force the *next* flush's tmp-file write to fail by pre-creating a directory
    // at the `.tmp` path flush() writes to (fs::write on a directory path always
    // errors with EISDIR, regardless of permissions/root).
    let tmp_path = path.with_extension("tmp");
    fs::create_dir_all(&tmp_path).expect("setup: create blocking dir at tmp path");

    state.unlock("offline:Bob", RecipeId::new("craft.example.b"));
    let result = state.flush();
    assert!(
        result.is_err(),
        "expected the second flush to fail because its tmp path is occupied by a \
             directory, actual={result:?}"
    );

    // 3) Atomic-write contract: a failed write must NEVER touch the final path — only
    // a successful `fs::write` + `fs::rename` pair may replace it.
    let bytes_after_failed_flush =
        fs::read_to_string(&path).expect("original file must still exist after failed flush");
    assert_eq!(
        bytes_after_failed_flush, original_bytes,
        "expected the final file to be byte-for-byte unchanged after a failed flush \
             because flush() must write a tmp file then rename, never write the final path \
             directly — a truncated/half-written final file would lose Alice's \
             already-persisted unlock"
    );
    assert!(
        state.is_dirty(),
        "a failed flush must leave dirty=true because Bob's unlock is not yet safely on \
             disk"
    );

    let _ = fs::remove_dir_all(&tmp_path);
    let _ = fs::remove_file(&path);
}

// ── shutdown flush: externally observable persistence contract ──────────

#[test]
fn app_exit_flushes_dirty_recipe_unlocks_without_waiting_interval() {
    let path = unique_tmp_path("shutdown_dirty_immediate");
    let mut app = app_with_unlock_flush_systems(
        RecipeUnlockState::default()
            .with_path(&path)
            .with_flush_interval(600),
    );
    let recipe = RecipeId::new("craft.example.shutdown_only");
    app.world_mut()
        .resource_mut::<RecipeUnlockState>()
        .unlock("offline:Alice", recipe.clone());

    app.world_mut().send_event(AppExit::Success);
    app.update();

    assert!(
            path.exists(),
            "AppExit must persist a dirty recipe unlock immediately even though the 600-tick \n             runtime throttle has not elapsed"
        );
    let restored = RecipeUnlockState::hydrated_from_path(&path);
    assert!(
        restored.is_unlocked("offline:Alice", &recipe),
        "a restart after AppExit must hydrate Alice's newly unlocked recipe from disk"
    );
    assert!(
        !app.world().resource::<RecipeUnlockState>().is_dirty(),
        "a successful shutdown flush must leave no unpersisted recipe unlock state"
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn app_exit_flush_noops_when_recipe_unlock_state_clean() {
    let path = unique_tmp_path("shutdown_clean_noop");
    let mut app = app_with_unlock_flush_systems(
        RecipeUnlockState::default()
            .with_path(&path)
            .with_flush_interval(600),
    );

    app.world_mut().send_event(AppExit::Success);
    app.update();

    assert!(
        !path.exists(),
        "AppExit with a clean unlock state must not create an empty persistence file"
    );
    assert!(
        !app.world().resource::<RecipeUnlockState>().is_dirty(),
        "a clean shutdown no-op must leave the state clean"
    );
}

#[test]
fn dirty_state_without_app_exit_is_not_flushed_before_throttle_boundary() {
    let path = unique_tmp_path("shutdown_absent_throttled");
    let mut app = app_with_unlock_flush_systems(
        RecipeUnlockState::default()
            .with_path(&path)
            .with_flush_interval(600),
    );
    let recipe = RecipeId::new("craft.example.not_yet_due");
    app.world_mut()
        .resource_mut::<RecipeUnlockState>()
        .unlock("offline:Alice", recipe);

    app.update();

    assert!(
            !path.exists(),
            "without AppExit, a dirty state at tick 1 of 600 must remain throttled instead of \n             being flushed by the shutdown hook"
        );
    assert!(
            app.world().resource::<RecipeUnlockState>().is_dirty(),
            "the still-throttled unlock must remain dirty so a later runtime or shutdown flush \n             can persist it"
        );
}

#[test]
fn app_exit_flush_failure_keeps_dirty_and_preserves_existing_file() {
    let path = unique_tmp_path("shutdown_failure_atomic");
    let old_recipe = RecipeId::new("craft.example.already_persisted");
    let new_recipe = RecipeId::new("craft.example.pending_at_shutdown");

    let mut state = RecipeUnlockState::default()
        .with_path(&path)
        .with_flush_interval(600);
    state.unlock("offline:Alice", old_recipe.clone());
    state.flush().expect("setup: baseline flush should succeed");
    let original_bytes = fs::read(&path).expect("setup: baseline file must exist");

    let tmp_path = path.with_extension("tmp");
    fs::create_dir_all(&tmp_path).expect("setup: directory must block the shutdown tmp write");
    state.unlock("offline:Alice", new_recipe.clone());

    let mut app = app_with_unlock_flush_systems(state);
    app.world_mut().send_event(AppExit::Success);
    app.update();

    assert_eq!(
        fs::read(&path).expect("the last valid unlock file must survive a failed shutdown flush"),
        original_bytes,
        "a shutdown write failure must preserve the previous recipe_unlocks.json bytes exactly"
    );
    let restored = RecipeUnlockState::hydrated_from_path(&path);
    assert!(
        restored.is_unlocked("offline:Alice", &old_recipe),
        "hydrate after the failed shutdown flush must still recover the prior valid unlock"
    );
    assert!(
        !restored.is_unlocked("offline:Alice", &new_recipe),
        "hydrate must not invent the pending unlock when its tmp write failed"
    );
    let live_state = app.world().resource::<RecipeUnlockState>();
    assert!(
        live_state.is_unlocked("offline:Alice", &new_recipe),
        "the pending unlock must remain available in memory after the failed flush"
    );
    assert!(
        live_state.is_dirty(),
        "a failed AppExit flush must keep dirty=true so the loss is not reported as persisted"
    );

    let _ = fs::remove_dir_all(&tmp_path);
    let _ = fs::remove_file(&path);
}

// ── throttle off-by-one (CodeRabbit: interval-1 must not write, interval must) ──

#[test]
fn tick_recipe_unlock_flush_off_by_one_interval_boundary() {
    let path = unique_tmp_path("tick_flush_off_by_one");
    let mut app = valence::prelude::App::new();
    let state = RecipeUnlockState::default()
        .with_path(&path)
        .with_flush_interval(5);
    app.insert_resource(state);
    app.add_systems(valence::prelude::Update, tick_recipe_unlock_flush);

    app.world_mut()
        .resource_mut::<RecipeUnlockState>()
        .unlock("offline:Alice", RecipeId::new("craft.example.a"));

    // flush_clock is incremented *before* the threshold check each Update, so it
    // reaches `flush_interval_ticks` (5) exactly on the 5th call. Ticks 1..=4
    // (interval-1 and below) must NOT write yet — this pins the off-by-one boundary
    // that the previous test only checked loosely (via a `break`-on-exists retry loop).
    for i in 1..=4 {
        app.update();
        assert!(
            !path.exists(),
            "expected no flush at tick {i} of 5 (< flush_interval_ticks) because the \
                 throttle window has not elapsed yet, but the file was written early"
        );
    }

    // 5th tick == flush_interval_ticks → must write now.
    app.update();
    assert!(
        path.exists(),
        "expected a flush exactly at tick 5 (== flush_interval_ticks=5) because the \
             throttle check uses `>=`, but no file was written"
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn scroll_unlock_succeeds_when_template_matches() {
    let mut state = RecipeUnlockState::new();
    let recipe = recipe_with_sources(vec![UnlockSource::Scroll {
        item_template: "scroll_eclipse".into(),
    }]);
    let outcome = unlock_via_scroll(&mut state, "offline:Alice", &recipe, "scroll_eclipse");
    match outcome {
        UnlockOutcome::Newly {
            source: UnlockEventSource::Scroll { item_template },
        } => assert_eq!(item_template, "scroll_eclipse"),
        other => panic!("expected Newly Scroll, got {other:?}"),
    }
    assert!(state.is_unlocked("offline:Alice", &recipe.id));
}

#[test]
fn scroll_unlock_already_when_repeated() {
    let mut state = RecipeUnlockState::new();
    let recipe = recipe_with_sources(vec![UnlockSource::Scroll {
        item_template: "scroll_a".into(),
    }]);
    unlock_via_scroll(&mut state, "offline:Alice", &recipe, "scroll_a");
    let again = unlock_via_scroll(&mut state, "offline:Alice", &recipe, "scroll_a");
    assert_eq!(again, UnlockOutcome::Already);
}

#[test]
fn scroll_unlock_source_mismatch_when_template_differs() {
    let mut state = RecipeUnlockState::new();
    let recipe = recipe_with_sources(vec![UnlockSource::Scroll {
        item_template: "scroll_a".into(),
    }]);
    let outcome = unlock_via_scroll(&mut state, "offline:Alice", &recipe, "scroll_wrong");
    assert_eq!(outcome, UnlockOutcome::SourceMismatch);
    assert!(!state.is_unlocked("offline:Alice", &recipe.id));
}

#[test]
fn scroll_unlock_source_mismatch_when_recipe_only_supports_mentor() {
    let mut state = RecipeUnlockState::new();
    let recipe = recipe_with_sources(vec![UnlockSource::Mentor {
        npc_archetype: "poison_master".into(),
    }]);
    let outcome = unlock_via_scroll(&mut state, "offline:Alice", &recipe, "scroll_a");
    assert_eq!(outcome, UnlockOutcome::SourceMismatch);
}

#[test]
fn mentor_unlock_succeeds_when_archetype_matches() {
    let mut state = RecipeUnlockState::new();
    let recipe = recipe_with_sources(vec![UnlockSource::Mentor {
        npc_archetype: "poison_master".into(),
    }]);
    let outcome = unlock_via_mentor(&mut state, "offline:Alice", &recipe, "poison_master");
    assert!(matches!(outcome, UnlockOutcome::Newly { .. }));
    assert!(state.is_unlocked("offline:Alice", &recipe.id));
}

#[test]
fn mentor_unlock_source_mismatch_for_unrelated_archetype() {
    let mut state = RecipeUnlockState::new();
    let recipe = recipe_with_sources(vec![UnlockSource::Mentor {
        npc_archetype: "poison_master".into(),
    }]);
    let outcome = unlock_via_mentor(&mut state, "offline:Alice", &recipe, "blacksmith");
    assert_eq!(outcome, UnlockOutcome::SourceMismatch);
}

#[test]
fn insight_unlock_matches_specific_trigger() {
    let mut state = RecipeUnlockState::new();
    let recipe = recipe_with_sources(vec![UnlockSource::Insight {
        trigger: InsightTrigger::Breakthrough,
    }]);
    let ok = unlock_via_insight(
        &mut state,
        "offline:Alice",
        &recipe,
        InsightTrigger::Breakthrough,
    );
    assert!(matches!(ok, UnlockOutcome::Newly { .. }));

    // 不同 trigger 不应解锁另一个新玩家
    let other = unlock_via_insight(
        &mut state,
        "offline:Bob",
        &recipe,
        InsightTrigger::NearDeath,
    );
    assert_eq!(other, UnlockOutcome::SourceMismatch);
}

#[test]
fn multi_source_recipe_can_unlock_via_either_path() {
    let mut state = RecipeUnlockState::new();
    let recipe = recipe_with_sources(vec![
        UnlockSource::Scroll {
            item_template: "scroll_a".into(),
        },
        UnlockSource::Mentor {
            npc_archetype: "poison_master".into(),
        },
    ]);
    // Alice 走 scroll
    let alice = unlock_via_scroll(&mut state, "offline:Alice", &recipe, "scroll_a");
    assert!(matches!(alice, UnlockOutcome::Newly { .. }));
    // Bob 走 mentor
    let bob = unlock_via_mentor(&mut state, "offline:Bob", &recipe, "poison_master");
    assert!(matches!(bob, UnlockOutcome::Newly { .. }));
}

// ── plan-craft-v1 P3 — find_recipes_unlockable_by_* ─────────────

#[test]
fn material_unlock_succeeds_when_ingredient_present_and_empty_source() {
    let mut state = RecipeUnlockState::new();
    let recipe = empty_source_recipe("craft.tool.knife", &[("fan_tie", 2)]);
    let outcome = unlock_via_material(&mut state, "offline:Alice", &recipe, "fan_tie");
    assert_eq!(
        outcome,
        MaterialUnlockOutcome::Newly,
        "持有空源配方的原料应触发新解锁"
    );
    assert!(state.is_unlocked("offline:Alice", &recipe.id));
}

#[test]
fn material_unlock_unlocks_via_any_one_of_multiple_ingredients() {
    // 多原料配方：持有其中任一即解锁（不要求集齐）
    let recipe = empty_source_recipe("craft.tool.multi", &[("fan_tie", 2), ("crude_wood", 1)]);
    for trigger in ["fan_tie", "crude_wood"] {
        let mut state = RecipeUnlockState::new();
        assert_eq!(
            unlock_via_material(&mut state, "offline:Alice", &recipe, trigger),
            MaterialUnlockOutcome::Newly,
            "持有原料 `{trigger}` 应足以解锁多原料配方"
        );
    }
}

#[test]
fn material_unlock_not_applicable_for_recipe_with_explicit_source() {
    // 秘传配方（有 Scroll 源）即使持有其原料也不解锁 —— worldview §九 信息差
    let mut state = RecipeUnlockState::new();
    let mut recipe = empty_source_recipe("craft.secret.poison", &[("herb_a", 1)]);
    recipe.unlock_sources = vec![UnlockSource::Scroll {
        item_template: "scroll_secret".into(),
    }];
    let outcome = unlock_via_material(&mut state, "offline:Alice", &recipe, "herb_a");
    assert_eq!(
        outcome,
        MaterialUnlockOutcome::NotApplicable,
        "有显式解锁来源的秘传配方不应被材料发现解锁"
    );
    assert!(!state.is_unlocked("offline:Alice", &recipe.id));
}

#[test]
fn material_unlock_not_applicable_when_template_not_ingredient() {
    let mut state = RecipeUnlockState::new();
    let recipe = empty_source_recipe("craft.tool.knife", &[("fan_tie", 2)]);
    let outcome = unlock_via_material(&mut state, "offline:Alice", &recipe, "zhu_pi");
    assert_eq!(
        outcome,
        MaterialUnlockOutcome::NotApplicable,
        "非该配方原料的物品不应解锁该配方"
    );
    assert!(!state.is_unlocked("offline:Alice", &recipe.id));
}

#[test]
fn material_unlock_already_when_repeated() {
    let mut state = RecipeUnlockState::new();
    let recipe = empty_source_recipe("craft.tool.knife", &[("fan_tie", 2)]);
    unlock_via_material(&mut state, "offline:Alice", &recipe, "fan_tie");
    let again = unlock_via_material(&mut state, "offline:Alice", &recipe, "fan_tie");
    assert_eq!(
        again,
        MaterialUnlockOutcome::Already,
        "已解锁配方重复发现应返回 Already（noop，避免重复刷 UI）"
    );
}

#[test]
fn material_unlock_is_player_scoped() {
    let mut state = RecipeUnlockState::new();
    let recipe = empty_source_recipe("craft.tool.knife", &[("fan_tie", 2)]);
    unlock_via_material(&mut state, "offline:Alice", &recipe, "fan_tie");
    assert!(state.is_unlocked("offline:Alice", &recipe.id));
    assert!(
        !state.is_unlocked("offline:Bob", &recipe.id),
        "材料发现解锁应 per-player，不应泄露给其他玩家"
    );
}

// ── find_recipes_unlockable_by_material ──────────────────────────

#[test]
fn find_by_material_returns_only_empty_source_with_matching_ingredient() {
    let mut registry = CraftRegistry::new();
    registry
        .register(empty_source_recipe("craft.tool.a", &[("fan_tie", 1)]))
        .unwrap();
    registry
        .register(empty_source_recipe("craft.tool.b", &[("zhu_pi", 1)]))
        .unwrap();
    // 秘传配方：原料含 fan_tie，但有 Scroll 源 → 不应出现
    let mut secret = empty_source_recipe("craft.secret.c", &[("fan_tie", 1)]);
    secret.unlock_sources = vec![UnlockSource::Scroll {
        item_template: "scroll_c".into(),
    }];
    registry.register(secret).unwrap();

    let matches = find_recipes_unlockable_by_material(&registry, "fan_tie");
    assert_eq!(
        matches.len(),
        1,
        "fan_tie 只应匹配空源的 craft.tool.a，秘传 craft.secret.c 被排除，实际={matches:?}"
    );
    assert_eq!(matches[0].id.as_str(), "craft.tool.a");
}

#[test]
fn find_by_material_returns_empty_when_no_recipe_uses_it() {
    let mut registry = CraftRegistry::new();
    registry
        .register(empty_source_recipe("craft.tool.a", &[("fan_tie", 1)]))
        .unwrap();
    assert!(find_recipes_unlockable_by_material(&registry, "unobtainium").is_empty());
}
