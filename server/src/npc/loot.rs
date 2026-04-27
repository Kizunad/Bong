//! NPC 掉落表（plan-npc-ai-v1 §6 / §7 Phase 3）。
//!
//! NPC 通常无背包；被战斗/截胡杀死时按 archetype 的 `NpcLootTable` 生成
//! 掉落。本模块只提供数据模型 + 默认表 + deterministic roll 纯函数；
//! ECS 侧"死亡 → 生成 DroppedLoot"的挂接由 plan-death-lifecycle 接入
//! （等 `CultivationDeathTrigger` 处理链补齐 loot 分支）。

#![allow(dead_code)]

use valence::prelude::{bevy_ecs, Component};

use crate::npc::lifecycle::NpcArchetype;

// plan-npc-ai-v1 scaffolding: this module is referenced by design documents
// but not yet wired into the live death/drop pipeline.
/// 单条掉落条目：模板 ID + 基础掉率（0..=1）+ 数量范围。
/// 每次 roll 独立判定：`roll < chance` 命中 → 产出 `min..=max` 堆叠数。
#[derive(Clone, Debug, PartialEq)]
pub struct NpcLootEntry {
    pub template_id: String,
    pub chance: f32,
    pub min_stack: u32,
    pub max_stack: u32,
}

impl NpcLootEntry {
    pub fn new(template_id: impl Into<String>, chance: f32) -> Self {
        Self {
            template_id: template_id.into(),
            chance: chance.clamp(0.0, 1.0),
            min_stack: 1,
            max_stack: 1,
        }
    }

    pub fn with_stack(mut self, min: u32, max: u32) -> Self {
        self.min_stack = min.max(1);
        self.max_stack = max.max(self.min_stack);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Component)]
pub struct NpcLootTable {
    pub archetype: NpcArchetype,
    pub entries: Vec<NpcLootEntry>,
}

impl NpcLootTable {
    pub fn new(archetype: NpcArchetype, entries: Vec<NpcLootEntry>) -> Self {
        Self { archetype, entries }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// 默认 archetype → 掉落表。内容取自 plan §7 Phase 3（散修→丹药/法器残片 /
/// 弟子→派系信物 / 僵尸→素材 等）。模板 ID 可后续与 item_registry 校准。
pub fn default_loot_for_archetype(archetype: NpcArchetype) -> NpcLootTable {
    let entries = match archetype {
        NpcArchetype::Zombie => vec![
            NpcLootEntry::new("item.zombie.rotten_flesh", 0.8).with_stack(1, 3),
            NpcLootEntry::new("item.zombie.bone_fragment", 0.4).with_stack(1, 2),
            NpcLootEntry::new("item.bone_coin", 0.15).with_stack(1, 4),
        ],
        NpcArchetype::Commoner => vec![
            NpcLootEntry::new("item.bone_coin", 0.6).with_stack(1, 6),
            NpcLootEntry::new("item.commoner.hemp_cloth", 0.3),
        ],
        NpcArchetype::Rogue => vec![
            NpcLootEntry::new("item.rogue.pill_residue", 0.25),
            NpcLootEntry::new("item.rogue.talisman_fragment", 0.2),
            NpcLootEntry::new("item.bone_coin", 0.5).with_stack(3, 12),
        ],
        NpcArchetype::Beast => vec![
            NpcLootEntry::new("item.beast.hide", 0.7),
            NpcLootEntry::new("item.beast.fang", 0.4).with_stack(1, 2),
            NpcLootEntry::new("item.beast.spirit_core", 0.05),
        ],
        NpcArchetype::Disciple => vec![
            NpcLootEntry::new("item.disciple.sect_token", 0.8),
            NpcLootEntry::new("item.bone_coin", 0.4).with_stack(2, 10),
            NpcLootEntry::new("item.disciple.sect_scroll", 0.15),
        ],
        NpcArchetype::GuardianRelic => vec![
            NpcLootEntry::new("item.relic.engraved_plaque", 1.0),
            NpcLootEntry::new("item.relic.ancient_spark", 0.1),
        ],
        // plan-tsy-lifecycle-v1 §5.4 — 道伥 MVP loot：
        // 一件破旧凡物 + 偶尔残卷。完整 loot 继承（从原 corpse 反查 instance）推 P3。
        NpcArchetype::Daoxiang => vec![
            NpcLootEntry::new("item.daoxiang.rusty_blade", 0.5),
            NpcLootEntry::new("item.daoxiang.tattered_scroll", 0.1),
            NpcLootEntry::new("item.bone_coin", 0.4).with_stack(1, 6),
        ],
    };
    NpcLootTable::new(archetype, entries)
}

/// 单个 u64 seed → 0..=1 的 deterministic f32（测试友好，不依赖系统 rng）。
/// 使用 splitmix64 变体 + mantissa 归一化；足以给 drop roll 用。
fn splitmix64_unit(seed: u64) -> f32 {
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    // 取高 24 位做 [0,1) f32
    let bits = ((x >> 40) & 0x00FF_FFFF) as u32;
    bits as f32 / (1u32 << 24) as f32
}

/// 掉落 roll 的结果：每个命中条目 → 堆叠数量。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RolledLoot {
    pub template_id: String,
    pub stack: u32,
}

/// 基于 seed 对 `table` 逐条 roll；每条独立用 (seed ^ index_salt) 派生子种子。
/// 返回命中条目（顺序与 table.entries 一致）。Deterministic。
pub fn roll_loot(table: &NpcLootTable, seed: u64) -> Vec<RolledLoot> {
    let mut out = Vec::new();
    for (idx, entry) in table.entries.iter().enumerate() {
        let idx_u64 = idx as u64;
        let chance_seed = seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(idx_u64.wrapping_mul(0xBF58_476D_1CE4_E5B9));
        let roll = splitmix64_unit(chance_seed);
        if roll >= entry.chance {
            continue;
        }
        let stack_seed = seed
            .wrapping_mul(0x94D0_49BB_1331_11EB)
            .wrapping_add(idx_u64.wrapping_mul(0x1234_5678_ABCD_EF01));
        let span = entry.max_stack.saturating_sub(entry.min_stack) + 1;
        let offset = (splitmix64_unit(stack_seed) * span as f32) as u32;
        let stack = entry.min_stack + offset.min(span - 1);
        out.push(RolledLoot {
            template_id: entry.template_id.clone(),
            stack,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- NpcLootEntry ---

    #[test]
    fn loot_entry_clamps_chance_to_unit_range() {
        assert_eq!(NpcLootEntry::new("x", -1.0).chance, 0.0);
        assert_eq!(NpcLootEntry::new("x", 2.0).chance, 1.0);
    }

    #[test]
    fn loot_entry_with_stack_clamps_to_at_least_one() {
        let e = NpcLootEntry::new("x", 0.5).with_stack(0, 0);
        assert_eq!(e.min_stack, 1);
        assert_eq!(e.max_stack, 1);
    }

    #[test]
    fn loot_entry_with_stack_forces_max_ge_min() {
        let e = NpcLootEntry::new("x", 0.5).with_stack(5, 2);
        assert_eq!(e.min_stack, 5);
        assert_eq!(e.max_stack, 5);
    }

    // --- default tables ---

    #[test]
    fn default_tables_defined_for_every_archetype() {
        for arch in [
            NpcArchetype::Zombie,
            NpcArchetype::Commoner,
            NpcArchetype::Rogue,
            NpcArchetype::Beast,
            NpcArchetype::Disciple,
            NpcArchetype::GuardianRelic,
        ] {
            let table = default_loot_for_archetype(arch);
            assert_eq!(table.archetype, arch);
            assert!(!table.is_empty(), "{arch:?} loot table 不应为空");
        }
    }

    #[test]
    fn disciple_default_includes_sect_token() {
        let table = default_loot_for_archetype(NpcArchetype::Disciple);
        assert!(table
            .entries
            .iter()
            .any(|e| e.template_id == "item.disciple.sect_token"));
    }

    #[test]
    fn rogue_default_includes_pill_or_talisman_fragment() {
        let table = default_loot_for_archetype(NpcArchetype::Rogue);
        assert!(table
            .entries
            .iter()
            .any(|e| e.template_id.starts_with("item.rogue")));
    }

    #[test]
    fn guardian_relic_plaque_drops_always() {
        let table = default_loot_for_archetype(NpcArchetype::GuardianRelic);
        let plaque = table
            .entries
            .iter()
            .find(|e| e.template_id == "item.relic.engraved_plaque")
            .unwrap();
        assert_eq!(plaque.chance, 1.0);
    }

    // --- splitmix64_unit ---

    #[test]
    fn splitmix64_unit_returns_value_in_unit_interval() {
        for s in 0..1000 {
            let u = splitmix64_unit(s);
            assert!((0.0..1.0).contains(&u), "seed={s} got {u}");
        }
    }

    #[test]
    fn splitmix64_unit_is_deterministic() {
        assert_eq!(splitmix64_unit(42), splitmix64_unit(42));
        assert_ne!(splitmix64_unit(42), splitmix64_unit(43));
    }

    // --- roll_loot ---

    #[test]
    fn roll_loot_deterministic_same_seed() {
        let table = default_loot_for_archetype(NpcArchetype::Commoner);
        let a = roll_loot(&table, 12345);
        let b = roll_loot(&table, 12345);
        assert_eq!(a, b);
    }

    #[test]
    fn roll_loot_varies_with_seed() {
        let table = default_loot_for_archetype(NpcArchetype::Disciple);
        let mut seen = std::collections::HashSet::new();
        for s in 0..50u64 {
            seen.insert(roll_loot(&table, s * 1000));
        }
        assert!(
            seen.len() > 1,
            "不同 seed 应产生不同命中组合，实际 {} 组",
            seen.len()
        );
    }

    #[test]
    fn roll_loot_honors_chance_1_always_drops() {
        // GuardianRelic 的 engraved_plaque chance=1.0
        let table = default_loot_for_archetype(NpcArchetype::GuardianRelic);
        for s in 0..100u64 {
            let out = roll_loot(&table, s);
            assert!(
                out.iter()
                    .any(|r| r.template_id == "item.relic.engraved_plaque"),
                "chance=1.0 的条目每次都必中"
            );
        }
    }

    #[test]
    fn roll_loot_empty_table_returns_empty() {
        let table = NpcLootTable::new(NpcArchetype::Zombie, Vec::new());
        assert!(roll_loot(&table, 99).is_empty());
    }

    #[test]
    fn roll_loot_stack_within_range() {
        // Zombie rotten_flesh 1..=3
        let table = default_loot_for_archetype(NpcArchetype::Zombie);
        for s in 0..500u64 {
            for r in roll_loot(&table, s) {
                if r.template_id == "item.zombie.rotten_flesh" {
                    assert!(r.stack >= 1 && r.stack <= 3, "stack {} 越界", r.stack);
                }
            }
        }
    }

    #[test]
    fn roll_loot_chance_distribution_roughly_matches() {
        // Commoner bone_coin chance 0.6 — 500 rolls 命中率应在 ±15% 内
        let table = default_loot_for_archetype(NpcArchetype::Commoner);
        let mut hits = 0usize;
        let trials = 500;
        for s in 0..trials as u64 {
            if roll_loot(&table, s * 7919)
                .iter()
                .any(|r| r.template_id == "item.bone_coin")
            {
                hits += 1;
            }
        }
        let rate = hits as f32 / trials as f32;
        assert!(
            (rate - 0.6).abs() < 0.15,
            "骨币命中率偏离过大：实际 {rate:.3}"
        );
    }
}
