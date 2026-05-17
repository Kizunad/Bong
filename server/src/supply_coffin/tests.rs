//! plan-supply-coffin-v1 P0/P2 单测 —— 饱和覆盖 Grade enum / Registry 状态机 / Loot 抽样。

use std::collections::{HashMap, HashSet};

use valence::prelude::{DVec3, Entity};

use super::loot::{loot_table, roll_count_range, roll_loot};
use super::{ActiveSupplyCoffin, CoffinCooldown, SupplyCoffinGrade, SupplyCoffinRegistry};

// =============================================================================
// SupplyCoffinGrade enum
// =============================================================================

#[test]
fn grade_all_lists_three_variants_in_order() {
    assert_eq!(
        SupplyCoffinGrade::ALL,
        [
            SupplyCoffinGrade::Common,
            SupplyCoffinGrade::Rare,
            SupplyCoffinGrade::Precious,
        ],
        "ALL 顺序固定，下游 dev 命令 / loop 依赖此顺序输出"
    );
}

#[test]
fn grade_max_active_matches_plan_spec_for_all_variants() {
    // plan §0 设计轴心 5：同档活跃上限 5 / 2 / 1
    assert_eq!(
        SupplyCoffinGrade::Common.max_active(),
        5,
        "Common 应允许 5 个并存：低档量大管饱"
    );
    assert_eq!(
        SupplyCoffinGrade::Rare.max_active(),
        2,
        "Rare 应允许 2 个并存：中档稀有"
    );
    assert_eq!(
        SupplyCoffinGrade::Precious.max_active(),
        1,
        "Precious 应允许 1 个并存：高档独占"
    );
}

#[test]
fn grade_cooldown_secs_matches_plan_spec_for_all_variants() {
    // plan P0.1：30min / 2h / 6h
    assert_eq!(SupplyCoffinGrade::Common.cooldown_secs(), 30 * 60);
    assert_eq!(SupplyCoffinGrade::Rare.cooldown_secs(), 2 * 60 * 60);
    assert_eq!(SupplyCoffinGrade::Precious.cooldown_secs(), 6 * 60 * 60);
}

#[test]
fn grade_cooldown_increases_with_rarity() {
    // 守门测试：未来若有人改其中一个常数，确保单调性不被破坏。
    assert!(
        SupplyCoffinGrade::Common.cooldown_secs() < SupplyCoffinGrade::Rare.cooldown_secs(),
        "Common cooldown < Rare cooldown 必须保持"
    );
    assert!(
        SupplyCoffinGrade::Rare.cooldown_secs() < SupplyCoffinGrade::Precious.cooldown_secs(),
        "Rare cooldown < Precious cooldown 必须保持"
    );
}

#[test]
fn grade_max_active_decreases_with_rarity() {
    assert!(
        SupplyCoffinGrade::Common.max_active() > SupplyCoffinGrade::Rare.max_active(),
        "Common max_active > Rare max_active 必须保持"
    );
    assert!(
        SupplyCoffinGrade::Rare.max_active() > SupplyCoffinGrade::Precious.max_active(),
        "Rare max_active > Precious max_active 必须保持"
    );
}

#[test]
fn grade_as_str_distinct_for_each_variant() {
    let names: HashSet<&'static str> = SupplyCoffinGrade::ALL.iter().map(|g| g.as_str()).collect();
    assert_eq!(names.len(), 3, "三 grade 的 as_str 必须互不相同");
}

#[test]
fn grade_from_str_round_trip_for_all_variants() {
    for g in SupplyCoffinGrade::ALL {
        assert_eq!(
            SupplyCoffinGrade::from_str(g.as_str()),
            Some(g),
            "as_str → from_str 必须 round trip：grade {:?}",
            g
        );
    }
}

#[test]
fn grade_from_str_rejects_unknown_and_empty() {
    assert_eq!(SupplyCoffinGrade::from_str(""), None);
    assert_eq!(SupplyCoffinGrade::from_str("epic"), None);
    assert_eq!(
        SupplyCoffinGrade::from_str("Common"),
        None,
        "from_str 是 snake_case，区分大小写：'Common' 应拒绝"
    );
    assert_eq!(SupplyCoffinGrade::from_str("legendary"), None);
}

// =============================================================================
// Loot table 静态校验
// =============================================================================

#[test]
fn loot_tables_have_expected_entry_counts() {
    // plan P0.3：Common 5 / Rare 5 / Precious 6（broken_sword_soul 是 Precious 独占第 6 条）
    assert_eq!(loot_table(SupplyCoffinGrade::Common).len(), 5);
    assert_eq!(loot_table(SupplyCoffinGrade::Rare).len(), 5);
    assert_eq!(loot_table(SupplyCoffinGrade::Precious).len(), 6);
}

#[test]
fn loot_table_entries_have_valid_count_ranges_and_weights() {
    for g in SupplyCoffinGrade::ALL {
        for entry in loot_table(g) {
            assert!(
                entry.min_count >= 1,
                "grade {:?} entry `{}` min_count {} < 1：不允许 roll 出 0 个物品",
                g,
                entry.template_id,
                entry.min_count
            );
            assert!(
                entry.max_count >= entry.min_count,
                "grade {:?} entry `{}` max_count {} < min_count {}：区间反转",
                g,
                entry.template_id,
                entry.max_count,
                entry.min_count
            );
            assert!(
                entry.weight > 0,
                "grade {:?} entry `{}` weight = 0：权重必须 > 0 否则永远抽不到",
                g,
                entry.template_id
            );
        }
    }
}

#[test]
fn loot_tables_have_no_duplicate_template_ids_within_grade() {
    for g in SupplyCoffinGrade::ALL {
        let table = loot_table(g);
        let unique: HashSet<&str> = table.iter().map(|e| e.template_id).collect();
        assert_eq!(
            unique.len(),
            table.len(),
            "grade {:?} loot 表内有重复 template_id（无重复抽样依赖 entries 自身唯一）",
            g
        );
    }
}

#[test]
fn loot_tables_include_grade_signature_items() {
    // Precious 独占高阶物品：star_iron / ancient_sword_embryo / broken_sword_soul
    let precious_ids: HashSet<&str> = loot_table(SupplyCoffinGrade::Precious)
        .iter()
        .map(|e| e.template_id)
        .collect();
    for tid in ["star_iron", "ancient_sword_embryo", "broken_sword_soul"] {
        assert!(
            precious_ids.contains(tid),
            "Precious 必须含 `{}`（plan P0.3 高阶独占）",
            tid
        );
    }

    // Common 必有 refined_iron + xuan_iron（基础锻造）
    let common_ids: HashSet<&str> = loot_table(SupplyCoffinGrade::Common)
        .iter()
        .map(|e| e.template_id)
        .collect();
    assert!(common_ids.contains("refined_iron"));
    assert!(common_ids.contains("xuan_iron"));
    assert!(common_ids.contains("rotten_bone_coin"));
}

#[test]
fn loot_table_high_tier_items_are_precious_only() {
    let high_tier = ["star_iron", "ancient_sword_embryo", "broken_sword_soul"];
    for g in [SupplyCoffinGrade::Common, SupplyCoffinGrade::Rare] {
        let ids: HashSet<&str> = loot_table(g).iter().map(|e| e.template_id).collect();
        for tid in high_tier {
            assert!(
                !ids.contains(tid),
                "grade {:?} 不应包含高阶物品 `{}`（worldview：祭坛棺独有）",
                g,
                tid
            );
        }
    }
}

#[test]
fn roll_count_range_matches_plan_spec() {
    // plan P0.3：Common 2-3 / Rare 2-3 / Precious 2-4
    assert_eq!(*roll_count_range(SupplyCoffinGrade::Common).start(), 2);
    assert_eq!(*roll_count_range(SupplyCoffinGrade::Common).end(), 3);
    assert_eq!(*roll_count_range(SupplyCoffinGrade::Rare).start(), 2);
    assert_eq!(*roll_count_range(SupplyCoffinGrade::Rare).end(), 3);
    assert_eq!(*roll_count_range(SupplyCoffinGrade::Precious).start(), 2);
    assert_eq!(*roll_count_range(SupplyCoffinGrade::Precious).end(), 4);
}

// =============================================================================
// roll_loot 抽样行为
// =============================================================================

#[test]
fn roll_loot_item_count_within_roll_count_range_for_all_seeds() {
    for g in SupplyCoffinGrade::ALL {
        let range = roll_count_range(g);
        for seed in 0..200_u64 {
            let rolled = roll_loot(g, seed);
            assert!(
                rolled.len() >= usize::from(*range.start())
                    && rolled.len() <= usize::from(*range.end()),
                "grade {:?} seed {} 产出 {} 条，超出区间 {}..={}：{:?}",
                g,
                seed,
                rolled.len(),
                range.start(),
                range.end(),
                rolled
            );
        }
    }
}

#[test]
fn roll_loot_template_ids_belong_to_grade_table() {
    for g in SupplyCoffinGrade::ALL {
        let valid: HashSet<&str> = loot_table(g).iter().map(|e| e.template_id).collect();
        for seed in 0..200_u64 {
            for (tid, _) in roll_loot(g, seed) {
                assert!(
                    valid.contains(tid.as_str()),
                    "grade {:?} seed {} rolled 未知 template_id `{}`",
                    g,
                    seed,
                    tid
                );
            }
        }
    }
}

#[test]
fn roll_loot_no_duplicate_template_ids_within_single_roll() {
    // 同一次开箱不允许两个相同 template_id —— 严格 no-replacement
    for g in SupplyCoffinGrade::ALL {
        for seed in 0..200_u64 {
            let rolled = roll_loot(g, seed);
            let ids: HashSet<String> = rolled.iter().map(|(t, _)| t.clone()).collect();
            assert_eq!(
                ids.len(),
                rolled.len(),
                "grade {:?} seed {} 同一次 roll 出现重复 template_id：{:?}",
                g,
                seed,
                rolled
            );
        }
    }
}

#[test]
fn roll_loot_per_entry_count_within_min_max() {
    // 每条 entry 的 count 必须落在 [min_count, max_count] 之间
    for g in SupplyCoffinGrade::ALL {
        let bounds: HashMap<&'static str, (u8, u8)> = loot_table(g)
            .iter()
            .map(|e| (e.template_id, (e.min_count, e.max_count)))
            .collect();
        for seed in 0..200_u64 {
            for (tid, count) in roll_loot(g, seed) {
                let &(lo, hi) = bounds
                    .get(tid.as_str())
                    .expect("已被 belong_to_grade_table 校验通过");
                assert!(
                    count >= lo && count <= hi,
                    "grade {:?} seed {} 物品 `{}` 数量 {} 越界 [{}, {}]",
                    g,
                    seed,
                    tid,
                    count,
                    lo,
                    hi
                );
                assert!(
                    count >= 1,
                    "count 必须 >= 1，否则 add_item_to_player_inventory 会 reject"
                );
            }
        }
    }
}

#[test]
fn roll_loot_is_deterministic_for_same_seed() {
    for g in SupplyCoffinGrade::ALL {
        for seed in [0, 1, 42, 100, u64::MAX / 2, u64::MAX] {
            let a = roll_loot(g, seed);
            let b = roll_loot(g, seed);
            assert_eq!(
                a, b,
                "grade {:?} seed {} 两次 roll 不一致：{:?} vs {:?}",
                g, seed, a, b
            );
        }
    }
}

#[test]
fn roll_loot_diverges_across_distinct_seeds() {
    // 跨 100 个不同 seed，应至少有 10 种不同的输出（不同 grade 都满足）。
    for g in SupplyCoffinGrade::ALL {
        let mut seen: HashSet<Vec<(String, u8)>> = HashSet::new();
        for seed in 0..100_u64 {
            seen.insert(roll_loot(g, seed));
        }
        assert!(
            seen.len() >= 10,
            "grade {:?} 100 个 seed 仅产生 {} 种不同 roll：分布过窄（应 >= 10）",
            g,
            seen.len()
        );
    }
}

#[test]
fn roll_loot_precious_eventually_includes_high_tier_items() {
    // Precious 含 3 个 weight=15-20 的高阶物品；200 个 seed 应至少命中一次 broken_sword_soul（最稀）。
    let high_tier_targets = ["star_iron", "ancient_sword_embryo", "broken_sword_soul"];
    let mut hits: HashMap<&'static str, usize> = HashMap::new();
    for seed in 0..500_u64 {
        for (tid, _) in roll_loot(SupplyCoffinGrade::Precious, seed) {
            for &target in &high_tier_targets {
                if tid == target {
                    *hits.entry(target).or_insert(0) += 1;
                }
            }
        }
    }
    for tid in high_tier_targets {
        assert!(
            hits.get(tid).copied().unwrap_or(0) >= 1,
            "Precious 500 seed 内未命中高阶物品 `{}`：分布异常",
            tid
        );
    }
}

// =============================================================================
// SupplyCoffinRegistry 状态机
// =============================================================================

#[test]
fn registry_empty_state_reports_zero_active_for_all_grades() {
    let r = make_registry();
    for g in SupplyCoffinGrade::ALL {
        assert_eq!(r.active_count(g), 0, "新建 registry grade {:?} 必须空", g);
    }
    assert!(r.cooldowns.is_empty());
}

#[test]
fn registry_insert_active_increments_grade_counter_only() {
    let mut r = make_registry();
    r.insert_active(
        Entity::from_raw(1),
        SupplyCoffinGrade::Common,
        DVec3::new(0.0, 0.0, 0.0),
        100,
    );
    assert_eq!(r.active_count(SupplyCoffinGrade::Common), 1);
    assert_eq!(r.active_count(SupplyCoffinGrade::Rare), 0);
    assert_eq!(r.active_count(SupplyCoffinGrade::Precious), 0);

    r.insert_active(
        Entity::from_raw(2),
        SupplyCoffinGrade::Rare,
        DVec3::new(10.0, 0.0, 0.0),
        100,
    );
    assert_eq!(r.active_count(SupplyCoffinGrade::Common), 1);
    assert_eq!(r.active_count(SupplyCoffinGrade::Rare), 1);
}

#[test]
fn registry_remove_active_returns_inserted_record_and_decrements() {
    let mut r = make_registry();
    let e = Entity::from_raw(7);
    let pos = DVec3::new(100.0, 65.0, 200.0);
    r.insert_active(e, SupplyCoffinGrade::Rare, pos, 999);
    let removed = r.remove_active(e).expect("插入过的 entity 必须能取出");
    assert_eq!(
        removed,
        ActiveSupplyCoffin {
            grade: SupplyCoffinGrade::Rare,
            pos,
            spawned_at_wall_secs: 999,
        }
    );
    assert_eq!(r.active_count(SupplyCoffinGrade::Rare), 0);
    assert!(
        r.remove_active(e).is_none(),
        "remove 后再 remove 应返回 None"
    );
}

#[test]
fn registry_cooldown_not_ready_before_grade_cooldown_secs() {
    let mut r = make_registry();
    r.enqueue_cooldown(SupplyCoffinGrade::Common, 1000);
    let cd = SupplyCoffinGrade::Common.cooldown_secs();
    // 边界 -1 tick：未到期
    assert!(!r.cooldowns[0].is_ready(1000 + cd - 1));
    // 精确边界：到期
    assert!(r.cooldowns[0].is_ready(1000 + cd));
    // 边界 +1 tick：到期
    assert!(r.cooldowns[0].is_ready(1000 + cd + 1));
}

#[test]
fn registry_pop_ready_cooldown_only_takes_expired_entry_of_target_grade() {
    let mut r = make_registry();
    r.enqueue_cooldown(SupplyCoffinGrade::Common, 0); // 30min cooldown → ready at 1800
    r.enqueue_cooldown(SupplyCoffinGrade::Rare, 0); //   2h cooldown   → ready at 7200

    // 31min 时，仅 Common 到期，Rare 不应被错误弹出
    let now = 31 * 60;
    assert!(
        !r.pop_ready_cooldown(SupplyCoffinGrade::Rare, now),
        "Rare 未到期不应能 pop"
    );
    assert_eq!(r.cooldowns.len(), 2);

    assert!(r.pop_ready_cooldown(SupplyCoffinGrade::Common, now));
    assert_eq!(r.cooldowns.len(), 1);
    assert_eq!(r.cooldowns[0].grade, SupplyCoffinGrade::Rare);

    // 2h+1min 后 Rare 也到期
    assert!(r.pop_ready_cooldown(SupplyCoffinGrade::Rare, (2 * 60 + 1) * 60));
    assert!(r.cooldowns.is_empty());
}

#[test]
fn registry_pop_ready_returns_false_when_no_matching_cooldown_exists() {
    let mut r = make_registry();
    // 空 cooldowns
    assert!(!r.pop_ready_cooldown(SupplyCoffinGrade::Common, 999_999));
    // 只有 Common，问 Precious 必空
    r.enqueue_cooldown(SupplyCoffinGrade::Common, 0);
    assert!(!r.pop_ready_cooldown(SupplyCoffinGrade::Precious, 999_999));
    // Common 自己还在
    assert_eq!(r.cooldowns.len(), 1);
}

#[test]
fn registry_delay_oldest_cooldown_pushes_only_first_matching_grade() {
    let mut r = make_registry();
    r.enqueue_cooldown(SupplyCoffinGrade::Common, 0);
    r.enqueue_cooldown(SupplyCoffinGrade::Rare, 50);
    r.enqueue_cooldown(SupplyCoffinGrade::Common, 100);

    r.delay_oldest_cooldown(SupplyCoffinGrade::Common, 60);

    assert_eq!(
        r.cooldowns[0].broken_at_wall_secs, 60,
        "首个 Common 被推迟 60s"
    );
    assert_eq!(r.cooldowns[1].broken_at_wall_secs, 50, "Rare 不受影响");
    assert_eq!(
        r.cooldowns[2].broken_at_wall_secs, 100,
        "第二个 Common 不受影响"
    );
}

#[test]
fn registry_delay_oldest_cooldown_noop_for_absent_grade() {
    let mut r = make_registry();
    r.enqueue_cooldown(SupplyCoffinGrade::Common, 0);
    r.delay_oldest_cooldown(SupplyCoffinGrade::Precious, 60); // Precious 不存在
    assert_eq!(
        r.cooldowns[0].broken_at_wall_secs, 0,
        "Precious 不存在时不应误改 Common"
    );
}

#[test]
fn registry_min_distance_to_active_is_infinity_when_empty() {
    let r = make_registry();
    assert_eq!(
        r.min_distance_to_active(DVec3::new(0.0, 0.0, 0.0)),
        f64::INFINITY,
        "空 registry 距离任何点都视作无穷远（等价于无限制）"
    );
}

#[test]
fn registry_min_distance_to_active_returns_closest_distance() {
    let mut r = make_registry();
    r.insert_active(
        Entity::from_raw(1),
        SupplyCoffinGrade::Common,
        DVec3::new(10.0, 0.0, 0.0),
        0,
    );
    r.insert_active(
        Entity::from_raw(2),
        SupplyCoffinGrade::Rare,
        DVec3::new(0.0, 0.0, 30.0),
        0,
    );
    let d = r.min_distance_to_active(DVec3::new(0.0, 0.0, 0.0));
    assert!(
        (d - 10.0).abs() < 1e-6,
        "最近距离应为 10（到 entity 1），实际 {}",
        d
    );
}

#[test]
fn registry_min_distance_to_active_distinguishes_3d_distance() {
    let mut r = make_registry();
    r.insert_active(
        Entity::from_raw(1),
        SupplyCoffinGrade::Common,
        DVec3::new(0.0, 100.0, 0.0), // 100 格 y 距离
        0,
    );
    let d = r.min_distance_to_active(DVec3::new(0.0, 0.0, 0.0));
    assert!((d - 100.0).abs() < 1e-6, "3D 距离应含 y 差分量");
}

#[test]
fn rng_advances_deterministically_for_same_initial_state() {
    let mut a = make_registry();
    a.rng_state = 12345;
    let mut b = make_registry();
    b.rng_state = 12345;
    for i in 0..20 {
        assert_eq!(
            a.next_rand_u64(),
            b.next_rand_u64(),
            "step {} rng 不一致",
            i
        );
    }
}

#[test]
fn rng_diverges_for_different_seed() {
    let mut a = make_registry();
    a.rng_state = 0;
    let mut b = make_registry();
    b.rng_state = 1;
    let mut diverged = false;
    for _ in 0..5 {
        if a.next_rand_u64() != b.next_rand_u64() {
            diverged = true;
            break;
        }
    }
    assert!(diverged, "RNG state 0 vs 1 应在 5 步内出现分歧");
}

#[test]
fn cooldown_struct_is_ready_uses_saturating_add() {
    // 即便 broken_at_wall_secs 是 u64::MAX，is_ready 应不 overflow
    let cd = CoffinCooldown {
        grade: SupplyCoffinGrade::Common,
        broken_at_wall_secs: u64::MAX,
    };
    // saturating_add 让 sum = u64::MAX，is_ready 在 now = u64::MAX 时返回 true
    assert!(cd.is_ready(u64::MAX));
    // 任何更小的 now 都返回 false
    assert!(!cd.is_ready(0));
    assert!(!cd.is_ready(u64::MAX - 1));
}

fn make_registry() -> SupplyCoffinRegistry {
    SupplyCoffinRegistry::new(
        (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 0.0, 100.0)),
        65.0,
        0xDEAD_BEEF,
    )
}
