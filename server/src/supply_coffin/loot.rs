//! plan-supply-coffin-v1 P0.3 — 剑道材料 loot 表。
//!
//! 三档物资棺的 loot 池均围绕 `plan-sword-path-v1 P1.2` 11 种剑道材料 + 灵泉水 / 灵木 /
//! 腐朽骨币等通用器修消耗品。每次开箱按权重不重复抽 `roll_count_range(grade)` 种，
//! 每种再独立 roll 数量。
//!
//! 真实物品 template 来源（`server/assets/items/`）：
//! - `sword_materials.toml`：refined_iron / meteor_iron / star_iron / sword_embryo_shard /
//!   ancient_sword_embryo / broken_sword_soul / spirit_spring_water / spirit_spring_essence /
//!   scroll_sword_path
//! - `forge.toml`：xuan_iron
//! - `materials.toml`：spirit_wood
//! - `shelflife_dead.toml`：rotten_bone_coin

use std::ops::RangeInclusive;

use super::SupplyCoffinGrade;

/// 单条 loot 表项（一种物品的 template_id + 数量区间 + 权重）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupplyCoffinLootEntry {
    /// 必须存在于 `ItemRegistry`，否则 `roll_loot` 产出后 `add_item_to_player_inventory` 会失败。
    pub template_id: &'static str,
    /// 每次 roll 到该 entry 时的最小堆数量（含）。
    pub min_count: u8,
    /// 每次 roll 到该 entry 时的最大堆数量（含）。
    pub max_count: u8,
    /// 抽样权重（相对值，归一化在 `roll_loot` 内部完成）。
    pub weight: u32,
}

/// 松木棺（Common）—— 低阶器修消耗品，量大管饱（plan P0.3 表）。
const COMMON_LOOT: &[SupplyCoffinLootEntry] = &[
    SupplyCoffinLootEntry {
        template_id: "refined_iron",
        min_count: 2,
        max_count: 4,
        weight: 30,
    },
    SupplyCoffinLootEntry {
        template_id: "xuan_iron",
        min_count: 1,
        max_count: 2,
        weight: 25,
    },
    SupplyCoffinLootEntry {
        template_id: "spirit_wood",
        min_count: 1,
        max_count: 1,
        weight: 20,
    },
    SupplyCoffinLootEntry {
        template_id: "spirit_spring_water",
        min_count: 1,
        max_count: 3,
        weight: 15,
    },
    SupplyCoffinLootEntry {
        template_id: "rotten_bone_coin",
        min_count: 3,
        max_count: 8,
        weight: 10,
    },
];

/// 漆棺（Rare）—— 中阶器修关键材料。
const RARE_LOOT: &[SupplyCoffinLootEntry] = &[
    SupplyCoffinLootEntry {
        template_id: "xuan_iron",
        min_count: 2,
        max_count: 4,
        weight: 25,
    },
    SupplyCoffinLootEntry {
        template_id: "meteor_iron",
        min_count: 1,
        max_count: 2,
        weight: 25,
    },
    SupplyCoffinLootEntry {
        template_id: "sword_embryo_shard",
        min_count: 1,
        max_count: 1,
        weight: 20,
    },
    SupplyCoffinLootEntry {
        template_id: "spirit_spring_essence",
        min_count: 1,
        max_count: 1,
        weight: 15,
    },
    SupplyCoffinLootEntry {
        template_id: "scroll_sword_path",
        min_count: 1,
        max_count: 1,
        weight: 15,
    },
];

/// 祭坛棺（Precious）—— 高阶器修珍稀材料；含 star_iron / ancient_sword_embryo /
/// broken_sword_soul 等仅本档可见的项。
const PRECIOUS_LOOT: &[SupplyCoffinLootEntry] = &[
    SupplyCoffinLootEntry {
        template_id: "meteor_iron",
        min_count: 2,
        max_count: 3,
        weight: 20,
    },
    SupplyCoffinLootEntry {
        template_id: "star_iron",
        min_count: 1,
        max_count: 1,
        weight: 20,
    },
    SupplyCoffinLootEntry {
        template_id: "ancient_sword_embryo",
        min_count: 1,
        max_count: 1,
        weight: 15,
    },
    SupplyCoffinLootEntry {
        template_id: "sword_embryo_shard",
        min_count: 1,
        max_count: 2,
        weight: 15,
    },
    SupplyCoffinLootEntry {
        template_id: "scroll_sword_path",
        min_count: 1,
        max_count: 1,
        weight: 15,
    },
    SupplyCoffinLootEntry {
        template_id: "broken_sword_soul",
        min_count: 1,
        max_count: 1,
        weight: 15,
    },
];

/// 返回指定 grade 的静态 loot 表切片。
pub fn loot_table(grade: SupplyCoffinGrade) -> &'static [SupplyCoffinLootEntry] {
    match grade {
        SupplyCoffinGrade::Common => COMMON_LOOT,
        SupplyCoffinGrade::Rare => RARE_LOOT,
        SupplyCoffinGrade::Precious => PRECIOUS_LOOT,
    }
}

/// 每次开箱要抽几种物品（plan §P0.3 `roll_count`）。
///
/// 返回闭区间——具体落点由 `roll_loot` 用 seed 决定。
pub const fn roll_count_range(grade: SupplyCoffinGrade) -> RangeInclusive<u8> {
    match grade {
        SupplyCoffinGrade::Common => 2..=3,
        SupplyCoffinGrade::Rare => 2..=3,
        SupplyCoffinGrade::Precious => 2..=4,
    }
}

/// 从 `loot_table(grade)` 按权重 **不重复** 抽 `roll_count_range(grade)` 种，
/// 每种再 roll `[min_count, max_count]` 数量。
///
/// 完全 deterministic：相同 seed 永远产出相同 `Vec<(template_id, count)>`。
pub fn roll_loot(grade: SupplyCoffinGrade, seed: u64) -> Vec<(String, u8)> {
    let entries = loot_table(grade);
    if entries.is_empty() {
        return Vec::new();
    }

    let mut rng = SeedRng::new(seed);
    let range = roll_count_range(grade);
    let min_picks = usize::from(*range.start());
    let max_picks = usize::from(*range.end());
    let span = max_picks.saturating_sub(min_picks);
    let pick_count = (min_picks
        + if span == 0 {
            0
        } else {
            (rng.next() as usize) % (span + 1)
        })
    .min(entries.len());

    // 候选池 = entries 索引；每抽一个就从池中移除。
    let mut pool: Vec<usize> = (0..entries.len()).collect();
    let mut out: Vec<(String, u8)> = Vec::with_capacity(pick_count);

    for _ in 0..pick_count {
        let total_weight: u32 = pool.iter().map(|&i| entries[i].weight).sum();
        if total_weight == 0 {
            break;
        }
        let mut roll = (rng.next() % u64::from(total_weight)) as u32;
        let mut chosen_pool_idx = pool.len() - 1;
        for (i, &entry_i) in pool.iter().enumerate() {
            if roll < entries[entry_i].weight {
                chosen_pool_idx = i;
                break;
            }
            roll -= entries[entry_i].weight;
        }
        let entry_i = pool.remove(chosen_pool_idx);
        let entry = &entries[entry_i];
        let span = u64::from(entry.max_count - entry.min_count) + 1;
        let count = entry.min_count + (rng.next() % span) as u8;
        out.push((entry.template_id.to_string(), count));
    }

    out
}

/// splitmix64 单向 PRNG —— 仅 `roll_loot` 内部使用。
struct SeedRng {
    state: u64,
}

impl SeedRng {
    fn new(seed: u64) -> Self {
        let mixed = seed.wrapping_add(0xCAFE_BABE_1234_5678);
        Self {
            state: if mixed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                mixed
            },
        }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}
