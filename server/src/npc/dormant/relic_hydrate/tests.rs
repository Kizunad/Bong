//! plan-offscreen-war-v1 P3 交付物 3+4 饱和测试：deferred-on-hydrate 遗物物化。
//!
//! 锁住的契约：
//! - 零真元（§10.1 #5 ④红线）：物化的 loot ItemInstance 一律 spirit_quality=0，即便 template
//!   的 spirit_quality_initial 非 0 也被覆写（relic_item_spirit_quality_is_zero）。
//! - deterministic：同 loot_seed 两次物化同一批 template_id + stack（relic_loot_is_deterministic_by_seed）。
//! - archetype dispatch + 脏数据：按 archetype 取不同 loot 表；未知 archetype 串 → 空（不掉默认 loot）。
//! - 模板缺失健壮：roll 出的 template_id 不在 registry → 跳过该件、不 panic。
//! - system 级：玩家在 zone → sqlite pending relic 物化进 DroppedLootRegistry + 删行 + 发 narration；
//!   无玩家 → 不物化、行保留（deferred 语义）。

use std::collections::HashMap;

use valence::prelude::{App, DVec3, Events, Position, Update};

use super::*;
use crate::inventory::{
    DroppedLootRegistry, InventoryInstanceIdAllocator, ItemCategory, ItemRarity, ItemRegistry,
    ItemTemplate,
};
use crate::npc::lifecycle::NpcArchetype;
use crate::persistence::{
    bootstrap_sqlite, load_pending_dormant_relics_for_zone, persist_pending_dormant_relic,
    PendingDormantRelicRecord, PersistenceSettings,
};
use crate::world::dimension::DimensionKind;
use crate::world::zone::{Zone, ZoneRegistry};

const TEST_PLAQUE: &str = "item.relic.engraved_plaque";

/// 造一个测试 ItemTemplate。`spirit_quality_initial` 故意可设非 0，用来证明物化路径**覆写**
/// 成 0（而非继承 template 初始品质）。
fn test_template(template_id: &str, spirit_quality_initial: f64) -> ItemTemplate {
    ItemTemplate {
        id: template_id.to_string(),
        display_name: format!("display:{template_id}"),
        category: ItemCategory::Misc,
        max_stack_count: 64,
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.2,
        rarity: ItemRarity::Common,
        spirit_quality_initial,
        description: "test relic loot template".to_string(),
        effect: None,
        cast_duration_ms: 0,
        cooldown_ms: 0,
        weapon_spec: None,
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: None,
        shelflife_profile: None,
        shelflife_track: None,
    }
}

/// 含指定 template 的最小 registry（单 template）。
fn registry_with_template(template_id: &str, spirit_quality_initial: f64) -> ItemRegistry {
    registry_with_templates(&[(template_id, spirit_quality_initial)])
}

/// 含多个 template 的 registry（每个 (id, spirit_quality_initial)）。
fn registry_with_templates(templates: &[(&str, f64)]) -> ItemRegistry {
    let mut map = HashMap::new();
    for (id, sq) in templates {
        let template = test_template(id, *sq);
        map.insert(template.id.clone(), template);
    }
    ItemRegistry::from_map(map)
}

fn relic_record(archetype: NpcArchetype, loot_seed: u64, zone: &str) -> PendingDormantRelicRecord {
    PendingDormantRelicRecord {
        relic_id: "relic-test-1".to_string(),
        char_id: "dormant:fallen:test".to_string(),
        zone: zone.to_string(),
        pos_x: 100.0,
        pos_y: 64.0,
        pos_z: -50.0,
        archetype: archetype.as_str().to_string(),
        loot_seed,
        created_tick: 7,
        created_wall: 1_000,
    }
}

// ───────────────────── 零真元（守恒红线 §10.1 #5 ④） ─────────────────────

#[test]
fn relic_item_spirit_quality_is_zero() {
    // GuardianRelic 的 engraved_plaque chance=1.0 → 必命中。registry 里该 template 的
    // spirit_quality_initial 故意设 0.9（非 0），物化后**必须**被覆写为 0.0——否则遗物会把
    // 真元凭空带进世界（§10.1 #5 ④红线）。
    let registry = registry_with_template(TEST_PLAQUE, 0.9);
    let mut allocator = InventoryInstanceIdAllocator::new(1);
    let record = relic_record(NpcArchetype::GuardianRelic, 42, "rift_valley");

    let entries =
        materialize_relic_loot(&record, DimensionKind::Overworld, &registry, &mut allocator);

    assert!(
        !entries.is_empty(),
        "GuardianRelic engraved_plaque (chance=1.0) must materialize at least one loot entry"
    );
    for entry in &entries {
        assert_eq!(
            entry.item.spirit_quality, 0.0,
            "EVERY battlefield relic loot item must have spirit_quality=0 (§10.1 #5 ④ conservation \
             redline), even when the template's spirit_quality_initial is 0.9; got {} for {}",
            entry.item.spirit_quality, entry.item.template_id
        );
    }
    // const 锚点同步锁死（防有人误改 RELIC_LOOT_SPIRIT_QUALITY 成非 0）。
    assert_eq!(
        RELIC_LOOT_SPIRIT_QUALITY, 0.0,
        "RELIC_LOOT_SPIRIT_QUALITY must stay 0.0 — it is the conservation redline anchor"
    );
}

// ───────────────────── deterministic loot ─────────────────────

#[test]
fn relic_loot_is_deterministic_by_seed() {
    // 同 (archetype, loot_seed) 两次物化 → 完全相同的 (template_id, stack) 序列。
    // 这是"同一遗骸每次掉同一批"的可复现契约（真服 e2e 锁得住）。
    let registry = registry_with_template(TEST_PLAQUE, 0.0);
    let record = relic_record(NpcArchetype::GuardianRelic, 12345, "rift_valley");

    let mut alloc_a = InventoryInstanceIdAllocator::new(1);
    let first = materialize_relic_loot(&record, DimensionKind::Overworld, &registry, &mut alloc_a);
    let mut alloc_b = InventoryInstanceIdAllocator::new(1);
    let second = materialize_relic_loot(&record, DimensionKind::Overworld, &registry, &mut alloc_b);

    let signature = |entries: &[crate::inventory::DroppedLootEntry]| {
        entries
            .iter()
            .map(|e| (e.item.template_id.clone(), e.item.stack_count))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        signature(&first),
        signature(&second),
        "identical (archetype, loot_seed) must produce an identical loot (template_id, stack) \
         sequence so a relic re-rolls the same loot on every hydrate; got {:?} vs {:?}",
        signature(&first),
        signature(&second)
    );
}

#[test]
fn relic_loot_can_vary_with_seed() {
    // 不同 loot_seed 应能产出不同 loot（确定性可控，不是全场掉一样）。GuardianRelic 表里
    // engraved_plaque 恒掉（chance=1.0），但 ancient_spark（chance=0.1）随种子时有时无——
    // 扫一批种子，断言至少一个种子的产出件数与 baseline 不同。
    let registry = registry_with_templates(&[
        ("item.relic.engraved_plaque", 0.0),
        ("item.relic.ancient_spark", 0.0),
    ]);

    let baseline_record = relic_record(NpcArchetype::GuardianRelic, 0, "rift_valley");
    let mut alloc = InventoryInstanceIdAllocator::new(1);
    let baseline_len = materialize_relic_loot(
        &baseline_record,
        DimensionKind::Overworld,
        &registry,
        &mut alloc,
    )
    .len();

    let diverged = (1..400u64).any(|seed| {
        let mut a = InventoryInstanceIdAllocator::new(1);
        let rec = relic_record(NpcArchetype::GuardianRelic, seed, "rift_valley");
        materialize_relic_loot(&rec, DimensionKind::Overworld, &registry, &mut a).len()
            != baseline_len
    });
    assert!(
        diverged,
        "varying loot_seed must be able to change the rolled loot (e.g. the 0.1-chance ancient_spark \
         appearing or not); no seed in 1..400 diverged from baseline len {baseline_len}"
    );
}

// ───────────────────── archetype dispatch + 脏数据 ─────────────────────

#[test]
fn materialize_relic_loot_dispatches_by_archetype() {
    // Disciple 表掉 sect_token，GuardianRelic 表掉 engraved_plaque——同 seed 不同 archetype
    // 必从不同 loot 表 roll（archetype 真的被用来选表，不是写死一张）。
    let registry = registry_with_templates(&[
        ("item.disciple.sect_token", 0.0),
        ("item.relic.engraved_plaque", 0.0),
    ]);
    let mut alloc = InventoryInstanceIdAllocator::new(1);

    let disciple = materialize_relic_loot(
        &relic_record(NpcArchetype::Disciple, 7, "rift_valley"),
        DimensionKind::Overworld,
        &registry,
        &mut alloc,
    );
    let guardian = materialize_relic_loot(
        &relic_record(NpcArchetype::GuardianRelic, 7, "rift_valley"),
        DimensionKind::Overworld,
        &registry,
        &mut alloc,
    );

    let has = |entries: &[crate::inventory::DroppedLootEntry], id: &str| {
        entries.iter().any(|e| e.item.template_id == id)
    };
    // GuardianRelic engraved_plaque chance=1.0 必现；它不在 Disciple 表里。
    assert!(
        has(&guardian, "item.relic.engraved_plaque"),
        "GuardianRelic relic must roll from the GuardianRelic table (engraved_plaque present)"
    );
    assert!(
        !has(&disciple, "item.relic.engraved_plaque"),
        "Disciple relic must NOT roll engraved_plaque — that's GuardianRelic's table, dispatch leaked"
    );
}

#[test]
fn materialize_relic_loot_skips_unknown_archetype() {
    // 脏 archetype 串（旧 schema / 手写 sqlite）→ from_str None → 空 vec（绝不静默掉默认 loot）。
    let registry = registry_with_template(TEST_PLAQUE, 0.0);
    let mut alloc = InventoryInstanceIdAllocator::new(1);
    let mut record = relic_record(NpcArchetype::GuardianRelic, 42, "rift_valley");
    record.archetype = "totally_not_an_archetype".to_string();

    let entries = materialize_relic_loot(&record, DimensionKind::Overworld, &registry, &mut alloc);
    assert!(
        entries.is_empty(),
        "an unknown archetype string must materialize NO loot (not fall back to a default table); \
         got {} entries",
        entries.len()
    );
}

#[test]
fn materialize_relic_loot_skips_missing_templates_without_panic() {
    // registry 不含 loot 表的 template_id（真实运行常态——loot 表用占位 id）→ 每件都跳过、
    // 不 panic、返回空 vec。
    let empty_registry = ItemRegistry::from_map(HashMap::new());
    let mut alloc = InventoryInstanceIdAllocator::new(1);
    let entries = materialize_relic_loot(
        &relic_record(NpcArchetype::GuardianRelic, 42, "rift_valley"),
        DimensionKind::Overworld,
        &empty_registry,
        &mut alloc,
    );
    assert!(
        entries.is_empty(),
        "missing templates must be skipped gracefully (empty vec), got {} entries",
        entries.len()
    );
}

#[test]
fn relic_loot_entry_carries_zone_dimension_and_pos() {
    // 物化的 DroppedLootEntry 必须带正确 dimension（调用方从 zone 解析后传入）+ 位置在遗骸点
    // 附近（jitter ±0.5）+ source_container_id 标明来源。
    let registry = registry_with_template(TEST_PLAQUE, 0.0);
    let mut alloc = InventoryInstanceIdAllocator::new(1);
    let record = relic_record(NpcArchetype::GuardianRelic, 42, "rift_valley");

    let entries = materialize_relic_loot(&record, DimensionKind::Tsy, &registry, &mut alloc);
    assert!(
        !entries.is_empty(),
        "a GuardianRelic with a valid template must materialize at least one loot entry (so the relic actually drops something on hydrate); got an empty vec"
    );
    let entry = &entries[0];
    assert_eq!(
        entry.dimension,
        DimensionKind::Tsy,
        "loot entry must carry the dimension passed in (resolved from the relic's zone)"
    );
    assert!(
        (entry.world_pos[0] - record.pos_x).abs() <= 0.5
            && (entry.world_pos[2] - record.pos_z).abs() <= 0.5,
        "loot must drop within ±0.5 of the relic position (jitter), got {:?} vs base ({}, {})",
        entry.world_pos,
        record.pos_x,
        record.pos_z
    );
    assert_eq!(
        entry.world_pos[1], record.pos_y,
        "loot y must stay on the relic's ground plane (no vertical jitter), got {}",
        entry.world_pos[1]
    );
    assert!(
        entry.source_container_id.contains("offscreen_relic")
            && entry.source_container_id.contains(&record.zone),
        "source_container_id must mark the offscreen-relic origin + zone, got {}",
        entry.source_container_id
    );
}

#[test]
fn relic_reveal_narration_picks_one_of_two_by_seed_parity() {
    // 两条 zone-perception narration（plan §148）按 loot_seed 奇偶确定性二选一。
    let even = relic_reveal_narration(0);
    let odd = relic_reveal_narration(1);
    assert_ne!(
        even, odd,
        "even vs odd loot_seed must select different narration lines"
    );
    assert!(
        even.contains("宗门信物"),
        "even-seed narration should be the sect-token line, got `{even}`"
    );
    assert!(
        odd.contains("残经") || odd.contains("散修"),
        "odd-seed narration should be the rogue-remnant line, got `{odd}`"
    );
    // 同 seed 稳定（不随调用变）。
    assert_eq!(
        relic_reveal_narration(42),
        relic_reveal_narration(42),
        "relic_reveal_narration must be deterministic per seed (same seed → same narration line) so the reveal text is stable across hydrate retries; two calls with seed=42 differed"
    );
}

#[test]
fn relic_decal_color_matches_narration_variant_by_seed_parity() {
    // 视觉叙事一致性（round 3）：贴花色与 narration 必须同源——偶 seed=骨堆灰白+宗门信物，
    // 奇 seed=残卷暗黄+散修残经，视觉与文字讲同一个故事。
    // 偶 seed → 骨堆灰白 + 宗门信物文案。
    assert_eq!(
        relic_decal_color(0),
        RELIC_DECAL_COLOR_BONE,
        "even loot_seed must use the bone-heap decal color"
    );
    assert!(
        relic_reveal_narration(0).contains("宗门信物"),
        "even-seed narration must be the sect-token line so it matches the bone-heap decal"
    );
    // 奇 seed → 残卷暗黄 + 散修残经文案。
    assert_eq!(
        relic_decal_color(1),
        RELIC_DECAL_COLOR_SCROLL,
        "odd loot_seed must use the scroll-remnant decal color"
    );
    assert!(
        relic_reveal_narration(1).contains("残经") || relic_reveal_narration(1).contains("散修"),
        "odd-seed narration must be the rogue-remnant line so it matches the scroll-remnant decal"
    );
    // 两色不同（否则"两种叙事"在视觉上无区分）。
    assert_ne!(
        RELIC_DECAL_COLOR_BONE, RELIC_DECAL_COLOR_SCROLL,
        "bone vs scroll decal colors must differ to visually distinguish the two narration variants"
    );
}

// ───────────────────── system 级（deferred-on-hydrate 触发语义） ─────────────────────

fn test_zone(name: &str, dim: DimensionKind) -> Zone {
    // bounds 取一个含原点的大盒（[-100,100]³），玩家放原点即在 zone 内。
    Zone {
        name: name.to_string(),
        dimension: dim,
        bounds: (
            DVec3::new(-100.0, 0.0, -100.0),
            DVec3::new(100.0, 128.0, 100.0),
        ),
        spirit_qi: 0.3,
        danger_level: 0,
        active_events: Vec::new(),
        patrol_anchors: Vec::new(),
        blocked_tiles: Vec::new(),
    }
}

/// 起一个含 sqlite + ZoneRegistry + ItemRegistry + DroppedLootRegistry + Events + 玩家的最小 App。
/// 返回 (app, settings, root) 供断言。`with_player` 控制是否 spawn 一个在 zone 内的假 client。
fn relic_hydrate_app(
    test_name: &str,
    zone_name: &str,
    with_player: bool,
) -> (App, PersistenceSettings, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "bong-relic-hydrate-{test_name}-{}",
        std::process::id()
    ));
    let db_path = root.join("data").join("bong.db");
    let deceased = root.join("deceased");
    let settings =
        PersistenceSettings::with_paths(&db_path, &deceased, format!("relic-{test_name}"));
    bootstrap_sqlite(settings.db_path(), settings.server_run_id()).expect("bootstrap sqlite");

    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.insert_resource(crate::npc::dormant::NpcVirtualizationConfig {
        transition_interval_ticks: 1,
        ..Default::default()
    });
    app.insert_resource(ZoneRegistry {
        zones: vec![test_zone(zone_name, DimensionKind::Overworld)],
    });
    // registry 含 engraved_plaque（GuardianRelic chance=1.0 必命中）。
    app.insert_resource(registry_with_template(TEST_PLAQUE, 0.0));
    app.insert_resource(InventoryInstanceIdAllocator::new(1));
    app.insert_resource(DroppedLootRegistry::default());
    app.insert_resource(Events::<crate::network::vfx_event_emit::VfxEventRequest>::default());
    app.insert_resource(Events::<
        crate::network::audio_event_emit::PlaySoundRecipeRequest,
    >::default());
    app.insert_resource(crate::player::gameplay::PendingGameplayNarrations::default());

    if with_player {
        // 假 client：带 ClientMarker（valence 内部）+ Position 在 zone 内（原点）。Zone::default
        // 的 bounds 含原点（spawn 默认盒），玩家落在 zone 里。
        app.world_mut().spawn((
            valence::client::ClientMarker,
            Position::new(DVec3::new(0.0, 64.0, 0.0)),
        ));
    }

    app.add_systems(Update, hydrate_pending_dormant_relics_system);
    (app, settings, root)
}

#[test]
fn hydrate_system_materializes_loot_for_player_in_zone() {
    // 端到端 system：玩家在 zone "spawn" + sqlite 有该 zone 的 GuardianRelic pending relic →
    // 跑 system → DroppedLootRegistry 出现 loot（spirit_quality=0）+ pending 行被删 + narration 发出。
    let (mut app, settings, root) = relic_hydrate_app("materializes", "spawn", true);
    let record = relic_record(NpcArchetype::GuardianRelic, 99, "spawn");
    persist_pending_dormant_relic(&settings, &record).expect("persist pending relic");

    app.update();

    // ① DroppedLootRegistry 有 loot，且全 spirit_quality=0。
    let registry = app.world().resource::<DroppedLootRegistry>();
    assert!(
        !registry.entries.is_empty(),
        "a player standing in the relic's zone must trigger materialization into DroppedLootRegistry, \
         but the registry is empty"
    );
    for (_, entry) in registry.entries.iter() {
        assert_eq!(
            entry.item.spirit_quality, 0.0,
            "materialized ground loot must be zero-qi, got {} for {}",
            entry.item.spirit_quality, entry.item.template_id
        );
    }

    // ② pending 行被删（消费后不二次物化）。
    let remaining = load_pending_dormant_relics_for_zone(&settings, "spawn").expect("load");
    assert!(
        remaining.is_empty(),
        "the consumed pending relic must be deleted from sqlite so it never re-materializes, \
         got {} rows left",
        remaining.len()
    );

    // ③ zone narration 发出（perception scope=zone target=spawn）。
    let narrations = app
        .world_mut()
        .resource_mut::<crate::player::gameplay::PendingGameplayNarrations>()
        .drain();
    assert!(
        narrations.iter().any(|n| {
            matches!(n.scope, crate::schema::common::NarrationScope::Zone)
                && n.target.as_deref() == Some("spawn")
        }),
        "a zone-scoped reveal narration targeting 'spawn' must be emitted, got {:?}",
        narrations
    );

    // ④ VFX + audio events 发出（client 真有消费方）。
    let vfx = app
        .world_mut()
        .resource_mut::<Events<crate::network::vfx_event_emit::VfxEventRequest>>()
        .drain()
        .count();
    assert!(
        vfx >= 1,
        "a relic reveal VFX event must be emitted, got {vfx}"
    );
    let audio = app
        .world_mut()
        .resource_mut::<Events<crate::network::audio_event_emit::PlaySoundRecipeRequest>>()
        .drain()
        .collect::<Vec<_>>();
    assert!(
        audio
            .iter()
            .any(|a| a.recipe_id == OFFSCREEN_RELIC_REVEAL_AUDIO),
        "a relic reveal audio request (offscreen_relic_reveal) must be emitted, got {:?}",
        audio
            .iter()
            .map(|a| a.recipe_id.clone())
            .collect::<Vec<_>>()
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn hydrate_system_skips_zone_without_players() {
    // deferred 语义：无玩家在场 → 不物化、pending 行保留（等人来），registry 仍空。
    let (mut app, settings, root) = relic_hydrate_app("no-player", "spawn", false);
    let record = relic_record(NpcArchetype::GuardianRelic, 99, "spawn");
    persist_pending_dormant_relic(&settings, &record).expect("persist pending relic");

    app.update();

    let registry = app.world().resource::<DroppedLootRegistry>();
    assert!(
        registry.entries.is_empty(),
        "with no player present, NO loot may be materialized (deferred until someone arrives); \
         got {} entries",
        registry.entries.len()
    );
    let remaining = load_pending_dormant_relics_for_zone(&settings, "spawn").expect("load");
    assert_eq!(
        remaining.len(),
        1,
        "the pending relic must remain in sqlite when no player is in its zone (deferred), \
         got {} rows",
        remaining.len()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn hydrate_system_ignores_relic_in_other_zone() {
    // 玩家在 zone "spawn" 但遗物在 "rift_valley" → 不物化（zone 隔离），rift_valley 行保留。
    let (mut app, settings, root) = relic_hydrate_app("other-zone", "spawn", true);
    let record = relic_record(NpcArchetype::GuardianRelic, 99, "rift_valley");
    persist_pending_dormant_relic(&settings, &record).expect("persist pending relic");

    app.update();

    let registry = app.world().resource::<DroppedLootRegistry>();
    assert!(
        registry.entries.is_empty(),
        "a relic in a zone the player is NOT in must not materialize, got {} entries",
        registry.entries.len()
    );
    let remaining = load_pending_dormant_relics_for_zone(&settings, "rift_valley").expect("load");
    assert_eq!(
        remaining.len(),
        1,
        "the rift_valley relic must remain while the player is only in spawn, got {} rows",
        remaining.len()
    );
    let _ = std::fs::remove_dir_all(root);
}
