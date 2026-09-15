#![allow(dead_code, unused_imports)]

use bong_server::combat::components::Lifecycle;
use bong_server::cultivation::components::Realm;
use bong_server::cultivation::death_hooks::PlayerTerminated;
use bong_server::cultivation::epitaph::*;
use bong_server::cultivation::life_record::{BiographyEntry, LifeRecord, SkillMilestone};
use bong_server::npc::spawn::NpcMarker;
use bong_server::persistence::{bootstrap_sqlite, PersistenceSettings};
use bong_server::player::state::canonical_player_id;
use bong_server::skill::components::SkillId;
use std::path::PathBuf;
use valence::prelude::*;

// ─── 测试辅助 ───────────────────────────────────────────────────────────

fn temp_persistence(test_name: &str) -> (PersistenceSettings, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "bong-epitaph-{test_name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos(),
    ));
    let db_path = root.join("data").join("bong.db");
    let settings = PersistenceSettings::with_db_path(&db_path, format!("epitaph-{test_name}"));
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");
    (settings, root)
}

fn make_life_record(char_id: &str) -> LifeRecord {
    LifeRecord::new(canonical_player_id(char_id))
}

// ─── LifeRecordSummary 字段提取 ─────────────────────────────────────────

#[test]
fn summarize_empty_biography_returns_awaken_zero_kills_no_skills_empty_zone() {
    let lr = make_life_record("Alice");
    let summary = summarize(&lr, 0);
    assert_eq!(
        summary.peak_realm,
        Realm::Awaken,
        "期望无突破记录时 peak_realm=Awaken，实际={:?}",
        summary.peak_realm
    );
    assert_eq!(
        summary.total_kills, 0,
        "期望空 biography 击杀数=0，实际={}",
        summary.total_kills
    );
    assert_eq!(
        summary.total_deaths, 0,
        "期望 death_count=0 → total_deaths=0，实际={}",
        summary.total_deaths
    );
    assert!(
        summary.signature_skill_ids.is_empty(),
        "期望无 skill_milestones 时 signature_skill_ids 为空，实际={:?}",
        summary.signature_skill_ids
    );
    assert!(
        summary.final_zone.is_empty(),
        "期望无 zone 条目时 final_zone 为空，实际={}",
        summary.final_zone
    );
}

#[test]
fn summarize_single_breakthrough_returns_correct_peak_realm() {
    let mut lr = make_life_record("Alice");
    lr.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Induce,
        tick: 100,
    });
    let summary = summarize(&lr, 0);
    assert_eq!(
        summary.peak_realm,
        Realm::Induce,
        "期望单次突破到引气时 peak_realm=Induce，实际={:?}",
        summary.peak_realm
    );
}

#[test]
fn summarize_multiple_breakthroughs_picks_highest_realm() {
    let mut lr = make_life_record("Alice");
    lr.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Induce,
        tick: 100,
    });
    lr.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Condense,
        tick: 500,
    });
    lr.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Spirit,
        tick: 2000,
    });
    let summary = summarize(&lr, 5);
    assert_eq!(
        summary.peak_realm,
        Realm::Spirit,
        "期望多次突破取最高境界 Spirit，实际={:?}",
        summary.peak_realm
    );
    assert_eq!(
        summary.total_deaths, 5,
        "期望 death_count=5 如实传入，实际={}",
        summary.total_deaths
    );
}

#[test]
fn summarize_peak_realm_takes_max_not_last_breakthrough() {
    // 判别性 case：高境界先 push（tick=100），低境界后 push（tick=500）。
    // .last() / .min_by_key() 均会选出 Induce；只有 .max_by_key(rank) 才返回 Spirit。
    // 这把方向性 mutation 锁住。
    let mut lr = make_life_record("Alice");
    lr.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Spirit, // 高境界在前
        tick: 100,
    });
    lr.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Induce, // 低境界在后（更晚 tick）
        tick: 500,
    });
    let summary = summarize(&lr, 0);
    assert_eq!(
        summary.peak_realm,
        Realm::Spirit,
        "期望 peak_realm=Spirit（最高 rank），而非末项 Induce；\
         若取 .last() 或 .min() 此断言会红——实际={:?}",
        summary.peak_realm
    );
}

#[test]
fn summarize_pvp_encounter_death_fight_does_not_count_as_kill() {
    // PvpEncounter{outcome="death_fight"} 是对称记录：被杀的输家和击杀的赢家双方各收一条。
    // 无方向区分，直接计数会给输家凭空 +1 击杀（幻影战绩）。
    // P0 修正：death_fight 完全不计入 total_kills，等 P1 补 winner/survived 字段后再接入。
    let mut lr = make_life_record("Alice");
    // 两条 death_fight（alice 可能是被杀方，不该有幻影击杀）
    lr.push(BiographyEntry::PvpEncounter {
        counterparty_id: "offline:Bob".to_string(),
        outcome: "death_fight".to_string(),
        zone: "spawn".to_string(),
        context: "wilderness".to_string(),
        observed_style: None,
        appearance_hint: None,
        qi_color_hint: None,
        tick: 200,
    });
    lr.push(BiographyEntry::PvpEncounter {
        counterparty_id: "offline:Charlie".to_string(),
        outcome: "death_fight".to_string(),
        zone: "qingyun_peaks".to_string(),
        context: "resource_point".to_string(),
        observed_style: None,
        appearance_hint: None,
        qi_color_hint: None,
        tick: 400,
    });
    // 其他非击杀 outcome 也不计：betrayal/probe_fight/bypass
    lr.push(BiographyEntry::PvpEncounter {
        counterparty_id: "offline:Dave".to_string(),
        outcome: "betrayal".to_string(),
        zone: "blood_valley".to_string(),
        context: "tsy_extract".to_string(),
        observed_style: None,
        appearance_hint: None,
        qi_color_hint: None,
        tick: 600,
    });
    lr.push(BiographyEntry::PvpEncounter {
        counterparty_id: "offline:Eve".to_string(),
        outcome: "probe_fight".to_string(),
        zone: "spawn".to_string(),
        context: "wilderness".to_string(),
        observed_style: None,
        appearance_hint: None,
        qi_color_hint: None,
        tick: 800,
    });
    lr.push(BiographyEntry::PvpEncounter {
        counterparty_id: "offline:Frank".to_string(),
        outcome: "bypass".to_string(),
        zone: "spawn".to_string(),
        context: "wilderness".to_string(),
        observed_style: None,
        appearance_hint: None,
        qi_color_hint: None,
        tick: 1000,
    });
    let summary = summarize(&lr, 0);
    assert_eq!(
        summary.total_kills, 0,
        "期望 PvpEncounter(death_fight) 不计入 total_kills（对称记录，无方向区分，幻影战绩）\
         —— 无论多少条 death_fight，total_kills 必须为 0；非 death_fight outcome 同样不计。实际={}",
        summary.total_kills
    );
}

#[test]
fn summarize_juebi_killed_does_not_count_as_kill() {
    // JueBiKilled = 玩家自己在绝壁劫中殒命（"绝壁劫殁亡"），
    // 是死亡语义而非击杀对手——不应计入 total_kills。
    let mut lr = make_life_record("Alice");
    lr.push(BiographyEntry::JueBiKilled {
        source: "void_quota_exceeded".to_string(),
        tick: 100,
    });
    let summary = summarize(&lr, 0);
    assert_eq!(
        summary.total_kills, 0,
        "期望 JueBiKilled（绝壁劫殁亡=自身死亡）不计入 total_kills，实际={}",
        summary.total_kills
    );
}

#[test]
fn summarize_juebi_killed_does_not_inflate_kills_mixed_with_real_kills() {
    // JueBiKilled 不计入 kills，death_fight 也不计入（对称记录无方向），
    // 只有 TribulationIntercepted 才是真实定向击杀。
    let mut lr = make_life_record("Alice");
    lr.push(BiographyEntry::JueBiKilled {
        source: "void_quota_exceeded".to_string(),
        tick: 50,
    });
    // death_fight 是对称记录，Alice 可能是被杀方，不应计为 kill
    lr.push(BiographyEntry::PvpEncounter {
        counterparty_id: "offline:Bob".to_string(),
        outcome: "death_fight".to_string(),
        zone: "spawn".to_string(),
        context: "wilderness".to_string(),
        observed_style: None,
        appearance_hint: None,
        qi_color_hint: None,
        tick: 200,
    });
    // TribulationIntercepted 是定向记录，只写截劫方，确认是击杀
    lr.push(BiographyEntry::TribulationIntercepted {
        victim_id: "offline:Charlie".to_string(),
        tag: "戮道者 · 截劫".to_string(),
        tick: 400,
    });
    let summary = summarize(&lr, 0);
    assert_eq!(
        summary.total_kills, 1,
        "期望 JueBiKilled(0) + death_fight(0，对称不计) + TribulationIntercepted(1) = 1，实际={}",
        summary.total_kills
    );
}

#[test]
fn summarize_tribulation_intercepted_counts_as_kill() {
    let mut lr = make_life_record("Alice");
    lr.push(BiographyEntry::TribulationIntercepted {
        victim_id: "offline:Victim".to_string(),
        tag: "戮道者 · 截劫".to_string(),
        tick: 300,
    });
    let summary = summarize(&lr, 0);
    assert_eq!(
        summary.total_kills, 1,
        "期望 TribulationIntercepted 被计入击杀数，实际={}",
        summary.total_kills
    );
}

#[test]
fn summarize_skill_milestones_picks_top3_by_level() {
    let mut lr = make_life_record("Alice");
    lr.push_skill_milestone(SkillMilestone {
        skill: SkillId::Herbalism,
        new_lv: 2,
        achieved_at: 100,
        narration: "草木渐熟。".to_string(),
        total_xp_at: 120,
    });
    lr.push_skill_milestone(SkillMilestone {
        skill: SkillId::Alchemy,
        new_lv: 5,
        achieved_at: 300,
        narration: "炉火识性稍深。".to_string(),
        total_xp_at: 420,
    });
    lr.push_skill_milestone(SkillMilestone {
        skill: SkillId::Combat,
        new_lv: 4,
        achieved_at: 200,
        narration: "招式熟练。".to_string(),
        total_xp_at: 300,
    });
    lr.push_skill_milestone(SkillMilestone {
        skill: SkillId::Mineral,
        new_lv: 1,
        achieved_at: 50,
        narration: "识石初步。".to_string(),
        total_xp_at: 30,
    });
    let summary = summarize(&lr, 0);
    // 期望按 new_lv 降序：Alchemy(5) > Combat(4) > Herbalism(2) > Mineral(1)
    // 取前 3
    assert_eq!(
        summary.signature_skill_ids.len(),
        3,
        "期望取前 3 个最高 level 技能，实际={:?}",
        summary.signature_skill_ids
    );
    assert_eq!(
        summary.signature_skill_ids[0],
        SkillId::Alchemy,
        "期望第一位为最高 lv=5 的 Alchemy，实际={:?}",
        summary.signature_skill_ids[0]
    );
    assert_eq!(
        summary.signature_skill_ids[1],
        SkillId::Combat,
        "期望第二位为 lv=4 的 Combat，实际={:?}",
        summary.signature_skill_ids[1]
    );
    assert_eq!(
        summary.signature_skill_ids[2],
        SkillId::Herbalism,
        "期望第三位为 lv=2 的 Herbalism，实际={:?}",
        summary.signature_skill_ids[2]
    );
}

#[test]
fn summarize_final_zone_takes_last_zone_entry() {
    let mut lr = make_life_record("Alice");
    lr.push(BiographyEntry::PvpEncounter {
        counterparty_id: "offline:Bob".to_string(),
        outcome: "fled".to_string(),
        zone: "spawn".to_string(),
        context: "first_encounter".to_string(),
        observed_style: None,
        appearance_hint: None,
        qi_color_hint: None,
        tick: 100,
    });
    lr.push(BiographyEntry::PvpEncounter {
        counterparty_id: "offline:Charlie".to_string(),
        outcome: "killed".to_string(),
        zone: "blood_valley".to_string(),
        context: "final_battle".to_string(),
        observed_style: None,
        appearance_hint: None,
        qi_color_hint: None,
        tick: 500,
    });
    let summary = summarize(&lr, 0);
    assert_eq!(
        summary.final_zone, "blood_valley",
        "期望取最后一条 PvpEncounter 的 zone=blood_valley，实际={}",
        summary.final_zone
    );
}

#[test]
fn summarize_spirit_eye_breakthrough_zone_contributes_to_final_zone() {
    let mut lr = make_life_record("Alice");
    lr.push(BiographyEntry::SpiritEyeBreakthrough {
        eye_id: "eye_01".to_string(),
        zone: Some("qingyun_peaks".to_string()),
        tick: 200,
    });
    let summary = summarize(&lr, 0);
    assert_eq!(
        summary.final_zone, "qingyun_peaks",
        "期望 SpiritEyeBreakthrough 的 zone 被计入 final_zone，实际={}",
        summary.final_zone
    );
}

// ─── WorldEpitaphRegistry cap 测试 ─────────────────────────────────────

#[test]
fn registry_insert_under_cap_retains_all() {
    let mut registry = WorldEpitaphRegistry::default();
    for i in 0..5u32 {
        let entry = make_test_entry(&format!("offline:player_{i}"), i as u64);
        registry.insert(entry);
    }
    assert_eq!(
        registry.entries.len(),
        5,
        "期望插入 5 条后 registry 含 5 条，实际={}",
        registry.entries.len()
    );
}

#[test]
fn registry_insert_1001_evicts_oldest_from_memory() {
    let mut registry = WorldEpitaphRegistry::default();
    let mut first_id: Option<EpitaphId> = None;
    for i in 0..=1000u32 {
        let entry = make_test_entry(&format!("offline:player_{i}"), i as u64);
        if i == 0 {
            first_id = Some(entry.id.clone());
        }
        registry.insert(entry);
    }
    // 1001 条插入后 cap=1000，第 0 条被淘汰
    assert_eq!(
        registry.entries.len(),
        1000,
        "期望超 cap 1000 后 registry 只保留 1000 条，实际={}",
        registry.entries.len()
    );
    let first_id = first_id.expect("first_id should be set");
    assert!(
        !registry.entries.contains_key(&first_id),
        "期望第 0 条（最旧）被淘汰出内存 registry，但仍存在"
    );
}

#[test]
fn registry_cap_exactly_1000_does_not_evict() {
    let mut registry = WorldEpitaphRegistry::default();
    let mut ids = Vec::new();
    for i in 0..1000u32 {
        let entry = make_test_entry(&format!("offline:player_{i}"), i as u64);
        ids.push(entry.id.clone());
        registry.insert(entry);
    }
    assert_eq!(
        registry.entries.len(),
        1000,
        "期望插入恰好 1000 条后不触发淘汰，实际={}",
        registry.entries.len()
    );
    // 第 0 条仍在
    assert!(
        registry.entries.contains_key(&ids[0]),
        "期望 cap=1000 刚好不淘汰第 0 条，但它不在 registry 中"
    );
}

// ─── SQLite persist_epitaph round-trip ─────────────────────────────────

#[test]
fn persist_epitaph_round_trip_reads_back_identical() {
    let (settings, root) = temp_persistence("round-trip");
    let entry = make_test_entry("offline:RoundTrip", 999);
    bong_server::persistence::persist_epitaph(&settings, &entry)
        .expect("persist_epitaph should succeed");
    // 重新开连接读回
    let loaded = bong_server::persistence::load_epitaph(&settings, entry.id.0.as_str())
        .expect("load_epitaph should succeed")
        .expect("epitaph should be present in db");
    assert_eq!(
        loaded, entry,
        "期望 SQLite round-trip 读回与写入 EpitaphEntry 完全相同，实际不同"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn persist_epitaph_idempotent_upsert() {
    let (settings, root) = temp_persistence("upsert");
    let entry = make_test_entry("offline:Upsert", 42);
    bong_server::persistence::persist_epitaph(&settings, &entry)
        .expect("first persist should succeed");
    // 再写一次（ON CONFLICT DO UPDATE）
    bong_server::persistence::persist_epitaph(&settings, &entry)
        .expect("second persist (upsert) should succeed");
    let loaded = bong_server::persistence::load_epitaph(&settings, entry.id.0.as_str())
        .expect("load_epitaph should succeed")
        .expect("epitaph should be present");
    assert_eq!(loaded, entry, "期望幂等写入后读回内容不变，实际不同");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn persist_epitaph_1001_all_present_in_sqlite_after_registry_eviction() {
    let (settings, root) = temp_persistence("cap-sqlite-retain");
    let mut registry = WorldEpitaphRegistry::default();
    let mut first_entry: Option<EpitaphEntry> = None;
    for i in 0..=1000u32 {
        let entry = make_test_entry(&format!("offline:p_{i}"), i as u64);
        if i == 0 {
            first_entry = Some(entry.clone());
        }
        bong_server::persistence::persist_epitaph(&settings, &entry)
            .expect("persist_epitaph should succeed");
        registry.insert(entry);
    }
    // 内存 registry 已淘汰第 0 条
    let first = first_entry.expect("first_entry should be set");
    assert!(
        !registry.entries.contains_key(&first.id),
        "期望第 0 条已被淘汰出内存 registry"
    );
    // 但 SQLite 仍可读回（永久保留语义）
    let in_db = bong_server::persistence::load_epitaph(&settings, first.id.0.as_str())
        .expect("load_epitaph should succeed")
        .expect("期望第 0 条在 SQLite 中永久保留，但未找到");
    assert_eq!(
        in_db.character_id, first.character_id,
        "期望 SQLite 中读回的 character_id 与原始条目一致，实际不同"
    );
    let _ = std::fs::remove_dir_all(root);
}

// ─── FinalThought 变体 pin 测试 ─────────────────────────────────────────

#[test]
fn final_thought_none_serde_roundtrip() {
    let ft = FinalThought::None;
    let json = serde_json::to_string(&ft).expect("FinalThought::None 应可序列化");
    let back: FinalThought = serde_json::from_str(&json).expect("FinalThought::None 应可反序列化");
    assert_eq!(ft, back, "期望 FinalThought::None serde round-trip 一致");
}

#[test]
fn final_thought_location_hint_serde_roundtrip() {
    let ft = FinalThought::LocationHint("血谷东侧有灵石矿脉".to_string());
    let json = serde_json::to_string(&ft).unwrap();
    let back: FinalThought = serde_json::from_str(&json).unwrap();
    assert_eq!(
        ft, back,
        "期望 FinalThought::LocationHint serde round-trip 一致"
    );
}

#[test]
fn final_thought_revenge_hint_serde_roundtrip() {
    let ft = FinalThought::RevengeHint("offline:Betrayer 从背后偷袭了我".to_string());
    let json = serde_json::to_string(&ft).unwrap();
    let back: FinalThought = serde_json::from_str(&json).unwrap();
    assert_eq!(
        ft, back,
        "期望 FinalThought::RevengeHint serde round-trip 一致"
    );
}

#[test]
fn final_thought_insight_hint_serde_roundtrip() {
    let ft = FinalThought::InsightHint("合虚归真，非逆也".to_string());
    let json = serde_json::to_string(&ft).unwrap();
    let back: FinalThought = serde_json::from_str(&json).unwrap();
    assert_eq!(
        ft, back,
        "期望 FinalThought::InsightHint serde round-trip 一致"
    );
}

// ─── PendingFinalThoughtStore 软依赖（无 store → FinalThought::None）────

#[test]
fn no_pending_store_yields_final_thought_none() {
    // P0：PendingFinalThoughtStore 不注册时，系统应生成 FinalThought::None
    // 此处直接测 summarize + entry 构建逻辑，不启动完整 Bevy App
    let lr = make_life_record("Wanderer");
    let summary = summarize(&lr, 0);
    let final_thought: FinalThought = Option::<&mut PendingFinalThoughtStore>::None
        .and_then(|store| store.take(&lr.character_id))
        .unwrap_or(FinalThought::None);
    assert_eq!(
        final_thought,
        FinalThought::None,
        "期望无 PendingFinalThoughtStore 时 final_thought=None，实际={:?}",
        final_thought
    );
    // summary 也正常
    assert_eq!(summary.peak_realm, Realm::Awaken);
}

#[test]
fn pending_store_with_entry_returns_correct_thought() {
    let mut store = PendingFinalThoughtStore::default();
    store.insert(
        canonical_player_id("Alice"),
        FinalThought::RevengeHint("offline:Bob 杀了我".to_string()),
    );
    let thought = store
        .take(&canonical_player_id("Alice"))
        .unwrap_or(FinalThought::None);
    assert_eq!(
        thought,
        FinalThought::RevengeHint("offline:Bob 杀了我".to_string()),
        "期望从 store 取出正确的 RevengeHint"
    );
    // 取后应移除
    let second = store
        .take(&canonical_player_id("Alice"))
        .unwrap_or(FinalThought::None);
    assert_eq!(
        second,
        FinalThought::None,
        "期望 take 后再次 take 返回 None（条目已移除）"
    );
}

// ─── EpitaphGenerationSystem Bevy 集成测试 ──────────────────────────────

#[test]
fn epitaph_generation_system_creates_entry_for_player_terminated() {
    let (settings, root) = temp_persistence("system-player");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    let entity = app
        .world_mut()
        .spawn((
            LifeRecord::new(canonical_player_id("Alice")),
            bong_server::combat::components::Lifecycle {
                character_id: canonical_player_id("Alice"),
                death_count: 3,
                ..Default::default()
            },
        ))
        .id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    assert_eq!(
        registry.entries.len(),
        1,
        "期望 PlayerTerminated 触发后 registry 含 1 条碑刻，实际={}",
        registry.entries.len()
    );
    let (_id, entry) = registry
        .entries
        .iter()
        .next()
        .expect("should have one entry");
    assert_eq!(
        entry.character_id,
        canonical_player_id("Alice"),
        "期望碑刻 character_id = offline:Alice，实际={}",
        entry.character_id
    );
    assert_eq!(
        entry.record_summary.total_deaths, 3,
        "期望 death_count=3 被提取到 total_deaths，实际={}",
        entry.record_summary.total_deaths
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn epitaph_generation_system_skips_npc_terminated() {
    let (settings, root) = temp_persistence("system-npc-skip");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    // NPC 实体：带 NpcMarker，Without<NpcMarker> 过滤器应排除它
    let npc_entity = app
        .world_mut()
        .spawn((
            LifeRecord::new("npc:zombie_01".to_string()),
            bong_server::combat::components::Lifecycle {
                character_id: "npc:zombie_01".to_string(),
                ..Default::default()
            },
            NpcMarker,
        ))
        .id();
    app.world_mut()
        .send_event(PlayerTerminated { entity: npc_entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    assert_eq!(
        registry.entries.len(),
        0,
        "期望 NPC 终死亡不生成碑刻（Without<NpcMarker> 过滤），实际 registry 含 {} 条",
        registry.entries.len()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn epitaph_generation_system_entity_without_life_record_is_skipped() {
    let (settings, root) = temp_persistence("system-no-life-record");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    // 实体无 LifeRecord 也无 Lifecycle
    let entity = app.world_mut().spawn_empty().id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    assert_eq!(
        registry.entries.len(),
        0,
        "期望无 LifeRecord 的实体不生成碑刻，实际 registry 含 {} 条",
        registry.entries.len()
    );
    let _ = std::fs::remove_dir_all(root);
}

// ─── death_tick 三分支专属测试 ──────────────────────────────────────────

#[test]
fn death_tick_branch_lifecycle_last_death_tick_takes_priority() {
    // 分支 1：Lifecycle.last_death_tick 有值时，优先取它
    let (settings, root) = temp_persistence("death-tick-lifecycle");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    let mut lr = LifeRecord::new(canonical_player_id("Alice"));
    // biography 里也有 Terminated，但 Lifecycle.last_death_tick 应优先
    lr.push(BiographyEntry::Terminated {
        cause: "pvp".to_string(),
        tick: 9999,
    });

    let entity = app
        .world_mut()
        .spawn((
            lr,
            bong_server::combat::components::Lifecycle {
                character_id: canonical_player_id("Alice"),
                last_death_tick: Some(42),
                death_count: 1,
                ..Default::default()
            },
        ))
        .id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    let entry = registry.entries.values().next().expect("应生成一条碑刻");
    assert_eq!(
        entry.death_tick, 42,
        "期望 Lifecycle.last_death_tick=42 优先于 biography Terminated.tick=9999，实际={}",
        entry.death_tick
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn death_tick_branch_biography_terminated_fallback_when_no_lifecycle_tick() {
    // 分支 2：Lifecycle.last_death_tick=None，fallback 取 biography 末尾 Terminated.tick
    let (settings, root) = temp_persistence("death-tick-biography");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    let mut lr = LifeRecord::new(canonical_player_id("Bob"));
    lr.push(BiographyEntry::Terminated {
        cause: "tribulation".to_string(),
        tick: 777,
    });

    let entity = app
        .world_mut()
        .spawn((
            lr,
            bong_server::combat::components::Lifecycle {
                character_id: canonical_player_id("Bob"),
                last_death_tick: None, // 无 lifecycle tick
                death_count: 1,
                ..Default::default()
            },
        ))
        .id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    let entry = registry.entries.values().next().expect("应生成一条碑刻");
    assert_eq!(
        entry.death_tick, 777,
        "期望 Lifecycle.last_death_tick=None 时回退到 biography Terminated.tick=777，实际={}",
        entry.death_tick
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn death_tick_branch_fallback_zero_when_neither_source_present() {
    // 分支 3：Lifecycle.last_death_tick=None 且 biography 无 Terminated 条目 → unwrap_or(0)
    let (settings, root) = temp_persistence("death-tick-zero");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    // biography 空，lifecycle.last_death_tick=None
    let lr = LifeRecord::new(canonical_player_id("Charlie"));

    let entity = app
        .world_mut()
        .spawn((
            lr,
            bong_server::combat::components::Lifecycle {
                character_id: canonical_player_id("Charlie"),
                last_death_tick: None,
                death_count: 0,
                ..Default::default()
            },
        ))
        .id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    let entry = registry.entries.values().next().expect("应生成一条碑刻");
    assert_eq!(
        entry.death_tick, 0,
        "期望两个 tick 来源均无时 death_tick=0（unwrap_or(0)），实际={}",
        entry.death_tick
    );
    let _ = std::fs::remove_dir_all(root);
}

// ─── niche_pos 两路径专属测试 ────────────────────────────────────────────

#[test]
fn niche_pos_set_from_position_including_floor_of_negative_coords() {
    // 有 Position 时，niche_pos 应取整数 floor 坐标（含负坐标：-5.7 → -6）
    let (settings, root) = temp_persistence("niche-pos-with-pos");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    let entity = app
        .world_mut()
        .spawn((
            LifeRecord::new(canonical_player_id("Diana")),
            bong_server::combat::components::Lifecycle {
                character_id: canonical_player_id("Diana"),
                death_count: 1,
                ..Default::default()
            },
            // 正值和负小数坐标：x=12.9 → 12, y=64.1 → 64, z=-5.7 → -6
            valence::prelude::Position(valence::prelude::DVec3::new(12.9, 64.1, -5.7)),
        ))
        .id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    let entry = registry.entries.values().next().expect("应生成一条碑刻");
    assert_eq!(
        entry.niche_pos,
        Some([12, 64, -6]),
        "期望 Position(12.9, 64.1, -5.7) floor → [12, 64, -6]，实际={:?}",
        entry.niche_pos
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn niche_pos_none_when_entity_has_no_position_component() {
    // 实体无 Position component 时 niche_pos=None
    let (settings, root) = temp_persistence("niche-pos-none");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    let entity = app
        .world_mut()
        .spawn((
            LifeRecord::new(canonical_player_id("Eric")),
            bong_server::combat::components::Lifecycle {
                character_id: canonical_player_id("Eric"),
                death_count: 1,
                ..Default::default()
            },
            // 故意不附加 Position component
        ))
        .id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    let entry = registry.entries.values().next().expect("应生成一条碑刻");
    assert_eq!(
        entry.niche_pos, None,
        "期望实体无 Position component 时 niche_pos=None，实际={:?}",
        entry.niche_pos
    );
    let _ = std::fs::remove_dir_all(root);
}

// ─── final_thought 系统层端到端装配 ─────────────────────────────────────

#[test]
fn final_thought_taken_from_pending_store_and_removed_on_player_terminated() {
    // 插入 PendingFinalThoughtStore，触发 PlayerTerminated，
    // 断言 EpitaphEntry.final_thought 取自 store 且 store 已 remove。
    let (settings, root) = temp_persistence("final-thought-store");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    // 注册 PendingFinalThoughtStore（P1 路径）
    let mut store = PendingFinalThoughtStore::default();
    store.insert(
        canonical_player_id("Fang"),
        FinalThought::RevengeHint("offline:Killer 偷袭了我".to_string()),
    );
    app.insert_resource(store);
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    let entity = app
        .world_mut()
        .spawn((
            LifeRecord::new(canonical_player_id("Fang")),
            bong_server::combat::components::Lifecycle {
                character_id: canonical_player_id("Fang"),
                death_count: 2,
                ..Default::default()
            },
        ))
        .id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    let entry = registry.entries.values().next().expect("应生成一条碑刻");
    assert_eq!(
        entry.final_thought,
        FinalThought::RevengeHint("offline:Killer 偷袭了我".to_string()),
        "期望 final_thought 取自 PendingFinalThoughtStore 的 RevengeHint，实际={:?}",
        entry.final_thought
    );

    // store 已 remove（take 后条目消失，再次 take 返回 None）
    let mut store = app.world_mut().resource_mut::<PendingFinalThoughtStore>();
    let second_take = store.take(&canonical_player_id("Fang"));
    assert!(
        second_take.is_none(),
        "期望 PlayerTerminated 触发后 PendingFinalThoughtStore 中对应条目已移除（二次 take 返回 None）"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn final_thought_none_when_no_pending_store_resource_registered() {
    // 无 PendingFinalThoughtStore resource（P0 默认场景）→ final_thought=None
    let (settings, root) = temp_persistence("final-thought-no-store");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    // 故意不 insert PendingFinalThoughtStore
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    let entity = app
        .world_mut()
        .spawn((
            LifeRecord::new(canonical_player_id("Ghost")),
            bong_server::combat::components::Lifecycle {
                character_id: canonical_player_id("Ghost"),
                death_count: 0,
                ..Default::default()
            },
        ))
        .id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    let entry = registry.entries.values().next().expect("应生成一条碑刻");
    assert_eq!(
        entry.final_thought,
        FinalThought::None,
        "期望无 PendingFinalThoughtStore 时 final_thought=FinalThought::None，实际={:?}",
        entry.final_thought
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn final_thought_none_when_store_has_no_entry_for_character() {
    // PendingFinalThoughtStore 存在但不含本角色的条目 → final_thought=None
    let (settings, root) = temp_persistence("final-thought-store-miss");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    // store 里只有别的角色
    let mut store = PendingFinalThoughtStore::default();
    store.insert(
        canonical_player_id("OtherPlayer"),
        FinalThought::LocationHint("血谷有矿".to_string()),
    );
    app.insert_resource(store);
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    let entity = app
        .world_mut()
        .spawn((
            LifeRecord::new(canonical_player_id("Heron")),
            bong_server::combat::components::Lifecycle {
                character_id: canonical_player_id("Heron"),
                death_count: 1,
                ..Default::default()
            },
        ))
        .id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    let entry = registry.entries.values().next().expect("应生成一条碑刻");
    assert_eq!(
        entry.final_thought,
        FinalThought::None,
        "期望 store 中无本角色条目时 final_thought=None，实际={:?}",
        entry.final_thought
    );
    let _ = std::fs::remove_dir_all(root);
}

// ─── 系统级集成测试：total_kills / signature_skill_ids / final_zone 端到端 ─

#[test]
fn system_total_kills_only_tribulation_intercepted_not_death_fight() {
    // 端到端：biography 有 death_fight × 2 + TribulationIntercepted × 1
    // → total_kills 必须为 1（仅截劫，death_fight 不计）
    let (settings, root) = temp_persistence("sys-kills-direction");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    let mut lr = LifeRecord::new(canonical_player_id("Iris"));
    // 两条 death_fight（Iris 可能是被杀输家，不应贡献 kill）
    lr.push(BiographyEntry::PvpEncounter {
        counterparty_id: "offline:X1".to_string(),
        outcome: "death_fight".to_string(),
        zone: "spawn".to_string(),
        context: "wilderness".to_string(),
        observed_style: None,
        appearance_hint: None,
        qi_color_hint: None,
        tick: 100,
    });
    lr.push(BiographyEntry::PvpEncounter {
        counterparty_id: "offline:X2".to_string(),
        outcome: "death_fight".to_string(),
        zone: "spawn".to_string(),
        context: "wilderness".to_string(),
        observed_style: None,
        appearance_hint: None,
        qi_color_hint: None,
        tick: 200,
    });
    // 一条截劫（定向 kill，只写截劫方）
    lr.push(BiographyEntry::TribulationIntercepted {
        victim_id: "offline:Y1".to_string(),
        tag: "戮道者 · 截劫".to_string(),
        tick: 300,
    });
    let entity = app
        .world_mut()
        .spawn((
            lr,
            bong_server::combat::components::Lifecycle {
                character_id: canonical_player_id("Iris"),
                death_count: 2,
                ..Default::default()
            },
        ))
        .id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    let entry = registry.entries.values().next().expect("应生成一条碑刻");
    assert_eq!(
        entry.record_summary.total_kills, 1,
        "期望端到端：2×death_fight(不计) + 1×TribulationIntercepted = total_kills=1，\
         death_fight 是对称记录，计入会给被杀输家造成幻影战绩。实际={}",
        entry.record_summary.total_kills
    );
    assert_eq!(
        entry.record_summary.total_deaths, 2,
        "期望 total_deaths 端到端=lifecycle.death_count=2，实际={}",
        entry.record_summary.total_deaths
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn system_signature_skill_ids_dedup_same_skill_takes_highest_level() {
    // 同一 skill 多条 milestone（不同 level），去重后只取最高 level 那条。
    let (settings, root) = temp_persistence("sys-skills-dedup");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    let mut lr = LifeRecord::new(canonical_player_id("Jade"));
    // Combat lv1 先出现
    lr.push_skill_milestone(SkillMilestone {
        skill: SkillId::Combat,
        new_lv: 1,
        achieved_at: 50,
        narration: "初识拳脚。".to_string(),
        total_xp_at: 30,
    });
    // Alchemy lv3
    lr.push_skill_milestone(SkillMilestone {
        skill: SkillId::Alchemy,
        new_lv: 3,
        achieved_at: 100,
        narration: "炉火稍熟。".to_string(),
        total_xp_at: 200,
    });
    // Combat lv5 后出现（更高级，去重后应选这条）
    lr.push_skill_milestone(SkillMilestone {
        skill: SkillId::Combat,
        new_lv: 5,
        achieved_at: 300,
        narration: "拳意初成。".to_string(),
        total_xp_at: 500,
    });
    // Herbalism lv2
    lr.push_skill_milestone(SkillMilestone {
        skill: SkillId::Herbalism,
        new_lv: 2,
        achieved_at: 150,
        narration: "识草有道。".to_string(),
        total_xp_at: 120,
    });

    let entity = app
        .world_mut()
        .spawn((
            lr,
            bong_server::combat::components::Lifecycle {
                character_id: canonical_player_id("Jade"),
                death_count: 1,
                ..Default::default()
            },
        ))
        .id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    let entry = registry.entries.values().next().expect("应生成一条碑刻");
    let skills = &entry.record_summary.signature_skill_ids;
    assert_eq!(
        skills.len(),
        3,
        "期望 4 条 milestone(Combat×2去重) → 前3技能，实际={:?}",
        skills
    );
    // 按 new_lv 降序：Combat(5) > Alchemy(3) > Herbalism(2)
    assert_eq!(
        skills[0],
        SkillId::Combat,
        "期望第一名为去重后最高 lv=5 的 Combat，实际={:?}",
        skills[0]
    );
    assert_eq!(
        skills[1],
        SkillId::Alchemy,
        "期望第二名为 lv=3 的 Alchemy，实际={:?}",
        skills[1]
    );
    // Combat 不应出现两次
    assert!(
        !skills.iter().skip(1).any(|s| *s == SkillId::Combat),
        "期望同一 skill 去重后只出现一次，Combat 不应重复出现，实际={:?}",
        skills
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn system_final_zone_end_to_end_takes_last_nonempty_zone() {
    // 端到端：biography 有多条 PvpEncounter，取最后一条非空 zone。
    let (settings, root) = temp_persistence("sys-final-zone-e2e");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    let mut lr = LifeRecord::new(canonical_player_id("Kai"));
    lr.push(BiographyEntry::PvpEncounter {
        counterparty_id: "offline:A".to_string(),
        outcome: "fled".to_string(),
        zone: "spawn".to_string(),
        context: "wilderness".to_string(),
        observed_style: None,
        appearance_hint: None,
        qi_color_hint: None,
        tick: 100,
    });
    lr.push(BiographyEntry::PvpEncounter {
        counterparty_id: "offline:B".to_string(),
        outcome: "death_fight".to_string(),
        zone: "qingyun_peaks".to_string(),
        context: "resource_point".to_string(),
        observed_style: None,
        appearance_hint: None,
        qi_color_hint: None,
        tick: 500,
    });

    let entity = app
        .world_mut()
        .spawn((
            lr,
            bong_server::combat::components::Lifecycle {
                character_id: canonical_player_id("Kai"),
                death_count: 1,
                ..Default::default()
            },
        ))
        .id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    let entry = registry.entries.values().next().expect("应生成一条碑刻");
    assert_eq!(
        entry.record_summary.final_zone, "qingyun_peaks",
        "期望 final_zone 端到端取最后一条 PvpEncounter 的 zone=qingyun_peaks，实际={}",
        entry.record_summary.final_zone
    );
    let _ = std::fs::remove_dir_all(root);
}

// ─── final_zone 边界：空串跳过 / None 跳过 / 无条目返空 ──────────────────

#[test]
fn summarize_final_zone_empty_string_zone_is_skipped() {
    // PvpEncounter 的 zone 为空串时不应被选为 final_zone（跳过逻辑：`!zone.is_empty()`）。
    //
    // 判别性布局：非空条目先 push（biography[0]），空串条目后 push（biography[1]）。
    // .rev() 从末尾开始：先遇到空串 → !zone.is_empty() 守卫拒绝 → 再遇到非空 → 选中。
    // 若删掉守卫，空串会被直接返回，断言红——mutation-proof。
    let mut lr = make_life_record("Alice");
    lr.push(BiographyEntry::PvpEncounter {
        counterparty_id: "offline:Charlie".to_string(),
        outcome: "probe_fight".to_string(),
        zone: "blood_valley".to_string(), // 先 push，biography[0]
        context: "resource_point".to_string(),
        observed_style: None,
        appearance_hint: None,
        qi_color_hint: None,
        tick: 50,
    });
    lr.push(BiographyEntry::PvpEncounter {
        counterparty_id: "offline:Bob".to_string(),
        outcome: "fled".to_string(),
        zone: "".to_string(), // 后 push（biography[1]），.rev() 先遇到，应跳过
        context: "wilderness".to_string(),
        observed_style: None,
        appearance_hint: None,
        qi_color_hint: None,
        tick: 100,
    });
    // .rev() 扫：先看 biography[1] 空串（!zone.is_empty() 拒绝），
    // 再看 biography[0] "blood_valley"（通过）→ final_zone="blood_valley"
    let summary = summarize(&lr, 0);
    assert_eq!(
        summary.final_zone, "blood_valley",
        "期望空串 zone 被 !zone.is_empty() 守卫跳过，回退到更早的非空 zone=blood_valley，实际={}",
        summary.final_zone
    );
}

#[test]
fn summarize_final_zone_spirit_eye_none_zone_is_skipped() {
    // SpiritEyeBreakthrough.zone=None 时应跳过（不贡献 final_zone）
    let mut lr = make_life_record("Alice");
    lr.push(BiographyEntry::SpiritEyeBreakthrough {
        eye_id: "eye_01".to_string(),
        zone: None, // None，应跳过
        tick: 200,
    });
    // 没有其他带 zone 的条目
    let summary = summarize(&lr, 0);
    assert!(
        summary.final_zone.is_empty(),
        "期望 SpiritEyeBreakthrough.zone=None 跳过后 final_zone 为空，实际={}",
        summary.final_zone
    );
}

#[test]
fn summarize_final_zone_spirit_eye_empty_zone_string_is_skipped() {
    // SpiritEyeBreakthrough.zone=Some("") 空串也应跳过（`!z.is_empty()` 守卫）。
    //
    // 判别性布局：非空 PvpEncounter 先 push（biography[0]），
    // 空串 SpiritEye 后 push（biography[1]）。
    // .rev() 从末尾开始：先遇到 SpiritEye 空串 → !z.is_empty() 拒绝 →
    // 再遇到 PvpEncounter rift_valley → 选中。
    // 若删掉守卫，空串 Some("") 会被展开返回 ""，断言红——mutation-proof。
    let mut lr = make_life_record("Alice");
    lr.push(BiographyEntry::PvpEncounter {
        counterparty_id: "offline:Bob".to_string(),
        outcome: "fled".to_string(),
        zone: "rift_valley".to_string(), // 先 push，biography[0]
        context: "wilderness".to_string(),
        observed_style: None,
        appearance_hint: None,
        qi_color_hint: None,
        tick: 50,
    });
    lr.push(BiographyEntry::SpiritEyeBreakthrough {
        eye_id: "eye_02".to_string(),
        zone: Some("".to_string()), // 后 push（biography[1]），.rev() 先遇到，应跳过
        tick: 150,
    });
    // .rev() 扫：先看 biography[1] SpiritEye 空串（!z.is_empty() 拒绝），
    // 再看 biography[0] PvpEncounter zone=rift_valley（通过）→ final_zone="rift_valley"
    let summary = summarize(&lr, 0);
    assert_eq!(
        summary.final_zone, "rift_valley",
        "期望 SpiritEyeBreakthrough.zone=Some(\"\") 被 !z.is_empty() 守卫跳过，\
         回退到更早的 PvpEncounter zone=rift_valley，实际={}",
        summary.final_zone
    );
}

#[test]
fn summarize_final_zone_no_zone_entries_returns_empty() {
    // biography 只有无 zone 字段的条目（BreakthroughSucceeded/Terminated 等）→ final_zone 为空
    let mut lr = make_life_record("Alice");
    lr.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Induce,
        tick: 100,
    });
    lr.push(BiographyEntry::Terminated {
        cause: "tribulation".to_string(),
        tick: 500,
    });
    let summary = summarize(&lr, 1);
    assert!(
        summary.final_zone.is_empty(),
        "期望无带 zone 条目时 final_zone 为空，实际={}",
        summary.final_zone
    );
}

// ─── player_name：Username component 优先 / fallback lifecycle.character_id ─

#[test]
fn system_player_name_uses_lifecycle_character_id_when_no_username() {
    // 无 Username component 时 player_name 回退到 lifecycle.character_id
    let (settings, root) = temp_persistence("sys-name-fallback");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    let entity = app
        .world_mut()
        .spawn((
            LifeRecord::new(canonical_player_id("Luna")),
            bong_server::combat::components::Lifecycle {
                character_id: canonical_player_id("Luna"),
                death_count: 1,
                ..Default::default()
            },
            // 故意不附加 Username component
        ))
        .id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    let entry = registry.entries.values().next().expect("应生成一条碑刻");
    assert_eq!(
        entry.player_name,
        canonical_player_id("Luna"),
        "期望无 Username component 时 player_name 回退到 lifecycle.character_id=offline:Luna，实际={}",
        entry.player_name
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn system_player_name_from_username_component_when_present() {
    // 有 Username component 时 player_name 取真实显示名
    let (settings, root) = temp_persistence("sys-name-username");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    let entity = app
        .world_mut()
        .spawn((
            LifeRecord::new(canonical_player_id("Marco")),
            bong_server::combat::components::Lifecycle {
                character_id: canonical_player_id("Marco"),
                death_count: 1,
                ..Default::default()
            },
            Username("Marco".to_string()),
        ))
        .id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    let entry = registry.entries.values().next().expect("应生成一条碑刻");
    assert_eq!(
        entry.player_name, "Marco",
        "期望有 Username component 时 player_name 取显示名 Marco（非 offline:Marco），实际={}",
        entry.player_name
    );
    let _ = std::fs::remove_dir_all(root);
}

// ─── peak_realm 字段语义（原 final_realm 已重命名）──────────────────────────

#[test]
fn system_peak_realm_reflects_highest_breakthrough_not_death_time_realm() {
    // EpitaphEntry.peak_realm 取 biography 中最高突破境界（Realm::rank() 最大），
    // 不代表死亡时境界（P1 遗留）
    let (settings, root) = temp_persistence("sys-peak-realm");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    let mut lr = LifeRecord::new(canonical_player_id("Ning"));
    lr.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Induce,
        tick: 100,
    });
    lr.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Condense,
        tick: 500,
    });
    // 假设死时因某原因境界有回退（P0 无法记录回退，peak 是 Condense）

    let entity = app
        .world_mut()
        .spawn((
            lr,
            bong_server::combat::components::Lifecycle {
                character_id: canonical_player_id("Ning"),
                death_count: 1,
                ..Default::default()
            },
        ))
        .id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    let entry = registry.entries.values().next().expect("应生成一条碑刻");
    assert_eq!(
        entry.peak_realm,
        Realm::Condense,
        "期望 peak_realm 反映有生以来最高突破境界 Condense，实际={:?}",
        entry.peak_realm
    );
    assert_eq!(
        entry.record_summary.peak_realm,
        Realm::Condense,
        "期望 record_summary.peak_realm 也为 Condense，与 EpitaphEntry.peak_realm 一致，实际={:?}",
        entry.record_summary.peak_realm
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn system_peak_realm_max_by_rank_not_last_entry() {
    // 判别性 case（系统层）：Spirit 先写入（tick=100），Induce 后写入（tick=500）。
    // 若实现是 .last() 则 peak_realm=Induce；只有 .max_by_key(rank) 才返回 Spirit。
    let (settings, root) = temp_persistence("sys-peak-realm-max");
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.init_resource::<WorldEpitaphRegistry>();
    app.add_event::<PlayerTerminated>();
    app.add_systems(valence::prelude::Update, epitaph_generation_system);

    let mut lr = LifeRecord::new(canonical_player_id("RuoXi"));
    lr.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Spirit, // 高境界先写入
        tick: 100,
    });
    lr.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Induce, // 低境界后写入（末条）
        tick: 500,
    });

    let entity = app
        .world_mut()
        .spawn((
            lr,
            bong_server::combat::components::Lifecycle {
                character_id: canonical_player_id("RuoXi"),
                death_count: 1,
                ..Default::default()
            },
        ))
        .id();
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();

    let registry = app.world().resource::<WorldEpitaphRegistry>();
    let entry = registry.entries.values().next().expect("应生成一条碑刻");
    assert_eq!(
        entry.peak_realm,
        Realm::Spirit,
        "期望 peak_realm=Spirit（rank 最高），不能取末条 Induce；\
         若实现是 .last() 此断言会红——实际={:?}",
        entry.peak_realm
    );
    assert_eq!(
        entry.record_summary.peak_realm,
        Realm::Spirit,
        "期望 record_summary.peak_realm 与 EpitaphEntry.peak_realm 一致=Spirit，实际={:?}",
        entry.record_summary.peak_realm
    );
    let _ = std::fs::remove_dir_all(root);
}

// ─── 辅助：构造测试用 EpitaphEntry ─────────────────────────────────────

fn make_test_entry(character_id: &str, death_tick: u64) -> EpitaphEntry {
    EpitaphEntry {
        id: EpitaphId::new(),
        character_id: character_id.to_string(),
        player_name: character_id.to_string(),
        peak_realm: Realm::Induce,
        death_tick,
        niche_pos: Some([10, 64, -5]),
        record_summary: LifeRecordSummary {
            peak_realm: Realm::Induce,
            total_kills: 2,
            total_deaths: 1,
            signature_skill_ids: vec![SkillId::Combat],
            final_zone: "spawn".to_string(),
        },
        final_thought: FinalThought::None,
    }
}
