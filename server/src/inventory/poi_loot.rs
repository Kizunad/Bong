//! plan-poi-novice-v1 — 新手 POI loot 表。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoiLootTable {
    ScrollHiddenCache,
    MutantNestBeastCoreStub,
    SpiritHerbValley,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoiLootEntry {
    pub item_id: &'static str,
    pub min_count: u8,
    pub max_count: u8,
}

pub const SCROLL_HIDDEN_CACHE: &[PoiLootEntry] = &[
    PoiLootEntry {
        item_id: "scroll_alchemy_basics",
        min_count: 1,
        max_count: 1,
    },
    PoiLootEntry {
        item_id: "scroll_forging_basics",
        min_count: 1,
        max_count: 1,
    },
    PoiLootEntry {
        item_id: "scroll_botany_basics",
        min_count: 1,
        max_count: 1,
    },
];

pub const SPIRIT_HERB_VALLEY_STARTERS: &[PoiLootEntry] = &[
    PoiLootEntry {
        item_id: "ningmai_cao",
        min_count: 1,
        max_count: 2,
    },
    PoiLootEntry {
        item_id: "yinqi_cao",
        min_count: 1,
        max_count: 3,
    },
    PoiLootEntry {
        item_id: "jiegu_rui",
        min_count: 1,
        max_count: 1,
    },
    PoiLootEntry {
        item_id: "anshen_guo",
        min_count: 1,
        max_count: 1,
    },
    PoiLootEntry {
        item_id: "qingzhuo_cao",
        min_count: 1,
        max_count: 2,
    },
];

pub fn entries_for(table: PoiLootTable) -> &'static [PoiLootEntry] {
    match table {
        PoiLootTable::ScrollHiddenCache => SCROLL_HIDDEN_CACHE,
        PoiLootTable::MutantNestBeastCoreStub => &[PoiLootEntry {
            item_id: "beast_core_stub",
            min_count: 1,
            max_count: 1,
        }],
        PoiLootTable::SpiritHerbValley => SPIRIT_HERB_VALLEY_STARTERS,
    }
}

pub fn roll_scroll_cache(seed: u64) -> Vec<&'static str> {
    let entries = entries_for(PoiLootTable::ScrollHiddenCache);
    let first = (seed as usize) % entries.len();
    let second = (first + 1 + (seed as usize / entries.len())) % entries.len();
    if first == second {
        vec![entries[first].item_id]
    } else {
        vec![entries[first].item_id, entries[second].item_id]
    }
}

pub fn log_novice_poi_loot_tables() {
    let scroll_count = entries_for(PoiLootTable::ScrollHiddenCache).len();
    let beast_core_count = entries_for(PoiLootTable::MutantNestBeastCoreStub).len();
    let herb_count = entries_for(PoiLootTable::SpiritHerbValley).len();
    let scroll_preview = roll_scroll_cache(0).join(",");
    tracing::debug!(
        "[bong][poi-novice] loot tables loaded scroll_cache={} mutant_nest={} herb_valley={} scroll_preview={}",
        scroll_count,
        beast_core_count,
        herb_count,
        scroll_preview
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_cache_rolls_one_or_two_known_scrolls() {
        let rolled = roll_scroll_cache(4);
        assert_eq!(rolled.len(), 2);
        assert!(rolled.iter().all(|id| id.starts_with("scroll_")));
    }

    #[test]
    fn spirit_herb_valley_declares_q119_five_starter_herbs() {
        let ids = entries_for(PoiLootTable::SpiritHerbValley)
            .iter()
            .map(|entry| entry.item_id)
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                "ningmai_cao",
                "yinqi_cao",
                "jiegu_rui",
                "anshen_guo",
                "qingzhuo_cao"
            ]
        );
    }
}
