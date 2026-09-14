use super::*;

fn unique_tmp_path(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("bong-craft-recipe-unlocks-{stamp}-{name}.json"))
}

#[test]
fn is_baseline_recipe_matches_registered_workbench_self_recipe() {
    // 常量与真实注册表对拍：BASELINE_RECIPES 里的每个 id 必须真实存在于
    // registry，防止配方改名后豁免名单静默漂移成空挂。
    let mut registry = super::super::registry::CraftRegistry::new();
    crate::craft::register_workbench_recipes(&mut registry).unwrap();
    for id in BASELINE_RECIPES {
        let rid = RecipeId::new(*id);
        assert!(is_baseline_recipe(&rid));
        let recipe = registry.get(&rid).unwrap_or_else(|| {
            panic!("BASELINE_RECIPES 含 `{id}` 但 registry 查无此配方 —— 配方改名后豁免名单未同步")
        });
        assert!(
            recipe.unlock_sources.is_empty(),
            "基线配方 `{id}` 不应同时声明显式解锁渠道（残卷/师承/顿悟），\
                 否则秘传门控被常显豁免绕过"
        );
    }
}

#[test]
fn repeated_unlock_does_not_redundantly_mark_dirty() {
    let mut state = RecipeUnlockState::new();
    state.unlock("offline:Alice", RecipeId::new("a"));
    state.flush_clock = 0; // irrelevant to this assertion, just documenting isolation
                           // manually clear dirty to simulate "already flushed"
    state.dirty = false;
    // unlocking the same recipe again is a noop — must NOT re-dirty the state,
    // otherwise every duplicate unlock attempt would force an unnecessary disk write.
    let inserted = state.unlock("offline:Alice", RecipeId::new("a"));
    assert!(!inserted, "duplicate unlock must report no insertion");
    assert!(
        !state.is_dirty(),
        "duplicate unlock must not re-mark a clean state dirty"
    );
}

#[test]
fn clear_for_player_marks_dirty_only_when_something_removed() {
    let mut state = RecipeUnlockState::new();
    state.unlock("offline:Alice", RecipeId::new("a"));
    state.dirty = false; // simulate "already flushed"
    state.clear_for_player("offline:Alice");
    assert!(
        state.is_dirty(),
        "clearing a player that actually had unlocks must mark state dirty"
    );

    state.dirty = false;
    state.clear_for_player("offline:Alice"); // already empty — noop
    assert!(
        !state.is_dirty(),
        "clearing an already-empty player must not spuriously mark dirty"
    );
}

#[test]
fn flush_writes_json_and_roundtrips() {
    let path = unique_tmp_path("flush_writes");
    let mut state = RecipeUnlockState::default().with_path(&path);
    state.unlock("offline:Alice", RecipeId::new("craft.example.a"));
    state.flush().expect("flush should succeed");

    let loaded = load_recipe_unlock_log(&path).expect("load should parse");
    assert_eq!(
        loaded.version, RECIPE_UNLOCK_VERSION,
        "expected flush() to stamp version={RECIPE_UNLOCK_VERSION} (RECIPE_UNLOCK_VERSION) \
             because writer and loader must agree on schema version, actual={}",
        loaded.version
    );
    assert_eq!(
        loaded
            .by_player
            .get("offline:Alice")
            .map(|s| s.contains(&RecipeId::new("craft.example.a"))),
        Some(true),
        "flushed file must contain the unlocked recipe for offline:Alice"
    );
    assert!(
        !state.is_dirty(),
        "flush must clear the dirty flag on success"
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn hydrated_from_missing_path_returns_empty_state() {
    let path = unique_tmp_path("hydrate_missing");
    assert!(
        !path.exists(),
        "test precondition: unique_tmp_path must not collide with an existing file, \
             otherwise this test would exercise the hydrate-from-existing-file path instead"
    );
    let state = RecipeUnlockState::hydrated_from_path(&path);
    assert_eq!(
        state.player_count(),
        0,
        "expected 0 players because hydrating from a nonexistent path is first-boot state, \
             actual={}",
        state.player_count()
    );
    assert!(!state.is_dirty(), "fresh startup state must not be dirty");
    assert_eq!(
        state.file_path,
        path,
        "hydrated_from_path must remember the path it was given so later flush() writes back \
             to the same location, expected={}, actual={}",
        path.display(),
        state.file_path.display()
    );
}

#[test]
fn load_recipe_unlock_log_accepts_current_version() {
    let path = unique_tmp_path("version_pin_current");
    fs::write(
            &path,
            format!(
                r#"{{"version":{RECIPE_UNLOCK_VERSION},"by_player":{{"offline:Alice":["craft.example.a"]}}}}"#
            ),
        )
        .unwrap();
    let file = load_recipe_unlock_log(&path).expect(
        "loader must accept a file whose version matches RECIPE_UNLOCK_VERSION — positive \
             pin for the schema version contract",
    );
    assert_eq!(
        file.version, RECIPE_UNLOCK_VERSION,
        "expected the parsed version to equal RECIPE_UNLOCK_VERSION={RECIPE_UNLOCK_VERSION} \
             because that's what was written to disk, actual={}",
        file.version
    );
    let _ = fs::remove_file(&path);
}

#[test]
fn load_recipe_unlock_log_rejects_unsupported_version() {
    // version=999 is deliberately far from RECIPE_UNLOCK_VERSION so this test stays
    // meaningful even if the schema is bumped in the future.
    let path = unique_tmp_path("version_pin_unsupported");
    fs::write(
        &path,
        r#"{"version":999,"by_player":{"offline:Alice":["craft.example.a"]}}"#,
    )
    .unwrap();
    let result = load_recipe_unlock_log(&path);
    assert!(
        result.is_err(),
        "expected Err for version=999 (!= RECIPE_UNLOCK_VERSION={RECIPE_UNLOCK_VERSION}) \
             because silently accepting a mismatched schema version risks misinterpreting \
             by_player as current-shape data, actual={result:?}"
    );
    let _ = fs::remove_file(&path);
}

#[test]
fn load_recipe_unlock_log_rejects_stale_default_version_zero() {
    // version=0 was the pre-fix `RecipeUnlockFile::default()` value (the bug CodeRabbit
    // flagged: default() disagreed with the writer's version=1). Pin that a file
    // carrying that stale value is rejected, not silently treated as current.
    let path = unique_tmp_path("version_pin_zero");
    fs::write(&path, r#"{"version":0,"by_player":{}}"#).unwrap();
    let result = load_recipe_unlock_log(&path);
    assert!(
        result.is_err(),
        "expected Err for version=0 because it no longer matches \
             RECIPE_UNLOCK_VERSION={RECIPE_UNLOCK_VERSION} now that default() is fixed, \
             actual={result:?}"
    );
    let _ = fs::remove_file(&path);
}

#[test]
fn find_by_scroll_returns_only_matching_recipes() {
    let mut registry = super::super::registry::CraftRegistry::new();
    crate::craft::register_examples(&mut registry).unwrap();
    let matches = find_recipes_unlockable_by_scroll(&registry, "scroll_herb_knife_iron");
    assert_eq!(
            matches.len(),
            1,
            "expected one recipe because scroll_herb_knife_iron maps to one example, actual matches={matches:?}"
        );
    assert_eq!(
        matches[0].id.as_str(),
        "craft.example.herb_knife.iron",
        "expected herb knife recipe for scroll_herb_knife_iron, actual id={}",
        matches[0].id.as_str()
    );
}

#[test]
fn find_by_scroll_returns_empty_when_template_unknown() {
    let mut registry = super::super::registry::CraftRegistry::new();
    crate::craft::register_examples(&mut registry).unwrap();
    let matches = find_recipes_unlockable_by_scroll(&registry, "scroll_unknown");
    assert!(matches.is_empty());
}

#[test]
fn find_by_mentor_returns_all_recipes_with_matching_archetype() {
    let mut registry = super::super::registry::CraftRegistry::new();
    crate::craft::register_examples(&mut registry).unwrap();
    let matches = find_recipes_unlockable_by_mentor(&registry, "array_scribe");
    assert_eq!(
        matches.len(),
        1,
        "expected one recipe because array_scribe teaches one example, actual matches={matches:?}"
    );
    assert_eq!(
        matches[0].id.as_str(),
        "craft.example.zhenfa_trap.iron",
        "expected zhenfa trap recipe for array_scribe, actual id={}",
        matches[0].id.as_str()
    );
}

#[test]
fn find_by_insight_matches_only_specific_trigger() {
    let mut registry = super::super::registry::CraftRegistry::new();
    crate::craft::register_examples(&mut registry).unwrap();
    // 伪灵皮 light 注册了 Insight::NearDeath
    let near_death = find_recipes_unlockable_by_insight(&registry, InsightTrigger::NearDeath);
    assert_eq!(near_death.len(), 1);
    assert_eq!(near_death[0].id.as_str(), "craft.example.fake_skin.light");
    // breakthrough 没人注册 → 空
    let bt = find_recipes_unlockable_by_insight(&registry, InsightTrigger::Breakthrough);
    assert!(bt.is_empty());
}
