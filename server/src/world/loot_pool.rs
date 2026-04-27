//! plan-tsy-container-v1 §1.4 — TSY loot pool 加载 + 滚 loot 工厂。
//!
//! 配置文件：`server/loot_pools.json`。每个 pool 定义 `rolls` 范围 + 加权
//! `entries` 列表，每个 entry 指向 `template_id` + 数量范围。
//!
//! 特殊 sentinel `__ancient_relic_random__` 由 `roll_loot_pool` 检测到后
//! 转发给 `AncientRelicPool::sample(source_class, seed)` —— 让 RelicCore 容器
//! 直接产 P1 plan 的上古遗物（避免重复定义模板）。

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use valence::prelude::{bevy_ecs, Resource};

use crate::inventory::ancient_relics::{AncientRelicPool, AncientRelicSource};
use crate::inventory::{
    InventoryInstanceIdAllocator, ItemInstance, ItemRegistry,
};

/// loot pool 注册表 resource。启动时从 `server/loot_pools.json` 加载。
#[derive(Debug, Default, Resource)]
pub struct LootPoolRegistry {
    pools: HashMap<String, LootPool>,
}

impl LootPoolRegistry {
    pub fn from_pools(pools: HashMap<String, LootPool>) -> Self {
        Self { pools }
    }

    pub fn get(&self, id: &str) -> Option<&LootPool> {
        self.pools.get(id)
    }

    pub fn len(&self) -> usize {
        self.pools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pools.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LootPool {
    /// 每次搜刮滚多少个 entry（min/max 闭区间，min 必须 ≥ 1）。
    pub rolls: (u32, u32),
    pub entries: Vec<LootEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LootEntry {
    pub template_id: String,
    pub weight: u32,
    pub count: (u32, u32),
}

const ANCIENT_RELIC_SENTINEL: &str = "__ancient_relic_random__";

/// 默认 loot pool 配置文件名（相对 server/）。
pub const DEFAULT_LOOT_POOLS_PATH: &str = "loot_pools.json";

/// 启动期加载入口。失败抛 panic（与 inventory::load_item_registry 对齐）。
pub fn load_loot_pool_registry() -> Result<LootPoolRegistry, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_LOOT_POOLS_PATH);
    load_loot_pool_registry_from_path(path)
}

pub fn load_loot_pool_registry_from_path(
    path: impl AsRef<Path>,
) -> Result<LootPoolRegistry, String> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read loot pools {}: {e}", path.display()))?;
    let raw: LootPoolsJson = serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse loot pools {}: {e}", path.display()))?;

    let mut pools = HashMap::with_capacity(raw.pools.len());
    for (id, pool_raw) in raw.pools {
        let pool = pool_raw
            .try_into_pool(&id, path)
            .map_err(|e| format!("loot pool `{id}`: {e}"))?;
        pools.insert(id, pool);
    }

    Ok(LootPoolRegistry::from_pools(pools))
}

/// 滚一个 pool，产出 ItemInstance 列表。
/// - `seed` 决定每次 roll 的随机性（同 seed 产同结果）
/// - `source_class` 给 `__ancient_relic_random__` sentinel 用
/// - 返回空列表 = pool 配置错（unknown pool / unknown template）
pub fn roll_loot_pool(
    registry: &LootPoolRegistry,
    pool_id: &str,
    item_registry: &ItemRegistry,
    relic_pool: &AncientRelicPool,
    allocator: &mut InventoryInstanceIdAllocator,
    source_class: AncientRelicSource,
    seed: u64,
) -> Vec<ItemInstance> {
    let Some(pool) = registry.get(pool_id) else {
        tracing::warn!("[bong][loot-pool] unknown pool id `{pool_id}` — returning empty");
        return Vec::new();
    };
    if pool.entries.is_empty() {
        return Vec::new();
    }

    let mut rng = SeedRng::new(seed);
    let rolls = rng.range_u32(pool.rolls.0, pool.rolls.1);
    let total_weight: u32 = pool.entries.iter().map(|e| e.weight).sum();
    if total_weight == 0 {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(rolls as usize);
    for _ in 0..rolls {
        let pick = rng.range_u32(0, total_weight.saturating_sub(1));
        let mut cumulative = 0u32;
        let mut chosen: Option<&LootEntry> = None;
        for entry in &pool.entries {
            cumulative = cumulative.saturating_add(entry.weight);
            if pick < cumulative {
                chosen = Some(entry);
                break;
            }
        }
        let Some(entry) = chosen else {
            continue;
        };
        let count = rng.range_u32(entry.count.0, entry.count.1).max(1);

        // sentinel：从 ancient relic pool 抽样
        if entry.template_id == ANCIENT_RELIC_SENTINEL {
            let relic_seed = seed.wrapping_add(rng.next_u64());
            let Some(template) = relic_pool.sample(source_class, relic_seed) else {
                tracing::warn!(
                    "[bong][loot-pool] ancient relic pool empty for source={:?}; skipping",
                    source_class
                );
                continue;
            };
            match template.to_item_instance(allocator) {
                Ok(item) => out.push(item),
                Err(e) => {
                    tracing::warn!(
                        "[bong][loot-pool] ancient relic instantiate failed: {e}; skipping"
                    );
                }
            }
            continue;
        }

        // 普通 template：走 ItemRegistry
        let Some(template) = item_registry.get(&entry.template_id) else {
            tracing::warn!(
                "[bong][loot-pool] unknown template id `{}` in pool `{pool_id}`; skipping",
                entry.template_id
            );
            continue;
        };
        let instance_id = match allocator.next_id() {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!("[bong][loot-pool] allocator overflow: {e}; aborting roll");
                return out;
            }
        };
        out.push(ItemInstance {
            instance_id,
            template_id: template.id.clone(),
            display_name: template.display_name.clone(),
            grid_w: template.grid_w,
            grid_h: template.grid_h,
            weight: template.base_weight,
            rarity: template.rarity,
            description: template.description.clone(),
            stack_count: count,
            spirit_quality: template.spirit_quality_initial,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
        });
    }

    out
}

/// 简易 splitmix64 PRNG。loot 滚动用，不要求加密强度。
struct SeedRng {
    state: u64,
}

impl SeedRng {
    fn new(seed: u64) -> Self {
        // 防 zero seed 卡死
        Self {
            state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15),
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// `[min, max]` 闭区间均匀采样。max < min 视为退化为 min。
    fn range_u32(&mut self, min: u32, max: u32) -> u32 {
        if max <= min {
            return min;
        }
        let span = (max - min) as u64 + 1;
        min + (self.next_u64() % span) as u32
    }
}

#[derive(Deserialize)]
struct LootPoolsJson {
    /// 文档字段（_doc 等）由 serde 自动忽略，这里只解析 pools。
    #[serde(default)]
    pools: HashMap<String, LootPoolJson>,
}

#[derive(Deserialize)]
struct LootPoolJson {
    rolls: [u32; 2],
    entries: Vec<LootEntryJson>,
}

#[derive(Deserialize)]
struct LootEntryJson {
    template_id: String,
    weight: u32,
    count: [u32; 2],
}

impl LootPoolJson {
    fn try_into_pool(self, pool_id: &str, source: &Path) -> Result<LootPool, String> {
        let [r_min, r_max] = self.rolls;
        if r_min == 0 {
            return Err(format!(
                "{} pool `{pool_id}` rolls.min must be >= 1, got 0",
                source.display()
            ));
        }
        if r_max < r_min {
            return Err(format!(
                "{} pool `{pool_id}` rolls.max {r_max} < min {r_min}",
                source.display()
            ));
        }
        let mut entries = Vec::with_capacity(self.entries.len());
        for raw in self.entries {
            if raw.weight == 0 {
                return Err(format!(
                    "{} pool `{pool_id}` entry `{}` has weight 0",
                    source.display(),
                    raw.template_id
                ));
            }
            let [c_min, c_max] = raw.count;
            if c_min == 0 || c_max < c_min {
                return Err(format!(
                    "{} pool `{pool_id}` entry `{}` invalid count [{c_min}, {c_max}]",
                    source.display(),
                    raw.template_id
                ));
            }
            entries.push(LootEntry {
                template_id: raw.template_id,
                weight: raw.weight,
                count: (c_min, c_max),
            });
        }
        if entries.is_empty() {
            return Err(format!(
                "{} pool `{pool_id}` has no entries",
                source.display()
            ));
        }
        Ok(LootPool {
            rolls: (r_min, r_max),
            entries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ItemCategory, ItemRarity, ItemTemplate};

    fn registry_with(templates: &[(&str, &str)]) -> ItemRegistry {
        let mut map = HashMap::new();
        for (id, name) in templates {
            map.insert(
                id.to_string(),
                ItemTemplate {
                    id: id.to_string(),
                    display_name: name.to_string(),
                    category: ItemCategory::Misc,
                    grid_w: 1,
                    grid_h: 1,
                    base_weight: 0.1,
                    rarity: ItemRarity::Common,
                    spirit_quality_initial: 0.0,
                    description: String::new(),
                    effect: None,
                    cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
                    cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
                    weapon_spec: None,
                },
            );
        }
        ItemRegistry::from_map(map)
    }

    /// 测试用：默认 ancient relic 池。`relic_core_deep` 等 sentinel pool 走它；
    /// 普通 pool（不含 sentinel）即便传入这个池也不会触碰 ancient relic 路径。
    fn relic_pool() -> AncientRelicPool {
        AncientRelicPool::from_seed()
    }

    #[test]
    fn loads_default_loot_pools_json() {
        let registry = load_loot_pool_registry().expect("default loot_pools.json must parse");
        assert!(registry.get("dry_corpse_shallow_common").is_some());
        assert!(registry.get("relic_core_deep").is_some());
        assert!(registry.get("nope_does_not_exist").is_none());
    }

    #[test]
    fn default_loot_pools_reference_only_known_templates() {
        // 防止 loot_pools.json 引用 ItemRegistry 不存在的 template_id（无声漂移）。
        // sentinel `__ancient_relic_random__` 由 roll_loot_pool 走 ancient relic pool，
        // 不需要在 ItemRegistry 内。
        let pools = load_loot_pool_registry().expect("loot_pools.json must parse");
        let items = crate::inventory::load_item_registry().expect("item registry must load");
        for (pool_id, pool) in &pools.pools {
            for entry in &pool.entries {
                if entry.template_id == ANCIENT_RELIC_SENTINEL {
                    continue;
                }
                assert!(
                    items.get(&entry.template_id).is_some(),
                    "loot pool `{pool_id}` references unknown template `{}`",
                    entry.template_id
                );
            }
        }
    }

    #[test]
    fn roll_loot_pool_returns_empty_for_unknown() {
        let reg = LootPoolRegistry::default();
        let item_reg = registry_with(&[]);
        let mut alloc = InventoryInstanceIdAllocator::default();
        let out = roll_loot_pool(
            &reg,
            "nope",
            &item_reg,
            &relic_pool(),
            &mut alloc,
            AncientRelicSource::SectRuins,
            1234,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn roll_loot_pool_picks_known_template() {
        let mut pools = HashMap::new();
        pools.insert(
            "single".to_string(),
            LootPool {
                rolls: (1, 1),
                entries: vec![LootEntry {
                    template_id: "iron_sword".to_string(),
                    weight: 100,
                    count: (1, 1),
                }],
            },
        );
        let reg = LootPoolRegistry::from_pools(pools);
        let item_reg = registry_with(&[("iron_sword", "凡铁剑")]);
        let mut alloc = InventoryInstanceIdAllocator::default();
        let out = roll_loot_pool(
            &reg,
            "single",
            &item_reg,
            &relic_pool(),
            &mut alloc,
            AncientRelicSource::SectRuins,
            42,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].template_id, "iron_sword");
        assert_eq!(out[0].stack_count, 1);
    }

    #[test]
    fn roll_loot_pool_resolves_ancient_relic_sentinel() {
        let mut pools = HashMap::new();
        pools.insert(
            "relic_core_deep".to_string(),
            LootPool {
                rolls: (1, 1),
                entries: vec![LootEntry {
                    template_id: ANCIENT_RELIC_SENTINEL.to_string(),
                    weight: 100,
                    count: (1, 1),
                }],
            },
        );
        let reg = LootPoolRegistry::from_pools(pools);
        let item_reg = registry_with(&[]);
        let relics = relic_pool();
        let mut alloc = InventoryInstanceIdAllocator::default();
        let out = roll_loot_pool(
            &reg,
            "relic_core_deep",
            &item_reg,
            &relics,
            &mut alloc,
            AncientRelicSource::DaoLord,
            7,
        );
        assert_eq!(out.len(), 1, "RelicCore pool should yield exactly one item");
        // ancient_relics 命名一致：template_id 以 "ancient_" 开头（见 seed_ancient_relics）。
        assert!(
            out[0].template_id.starts_with("ancient_"),
            "expected ancient relic template id, got {}",
            out[0].template_id
        );
    }

    #[test]
    fn roll_loot_pool_skips_unknown_template_silently() {
        let mut pools = HashMap::new();
        pools.insert(
            "bad".to_string(),
            LootPool {
                rolls: (1, 1),
                entries: vec![LootEntry {
                    template_id: "this_template_does_not_exist".to_string(),
                    weight: 100,
                    count: (1, 1),
                }],
            },
        );
        let reg = LootPoolRegistry::from_pools(pools);
        let item_reg = registry_with(&[]);
        let mut alloc = InventoryInstanceIdAllocator::default();
        let out = roll_loot_pool(
            &reg,
            "bad",
            &item_reg,
            &relic_pool(),
            &mut alloc,
            AncientRelicSource::SectRuins,
            1,
        );
        assert!(out.is_empty(), "unknown template should be skipped");
    }

    #[test]
    fn rolls_min_must_be_at_least_one() {
        let path = Path::new("loot_pools.json");
        let bad = LootPoolJson {
            rolls: [0, 1],
            entries: vec![LootEntryJson {
                template_id: "x".to_string(),
                weight: 1,
                count: [1, 1],
            }],
        };
        assert!(bad.try_into_pool("p", path).is_err());
    }

    #[test]
    fn entries_must_have_positive_weight() {
        let path = Path::new("loot_pools.json");
        let bad = LootPoolJson {
            rolls: [1, 1],
            entries: vec![LootEntryJson {
                template_id: "x".to_string(),
                weight: 0,
                count: [1, 1],
            }],
        };
        assert!(bad.try_into_pool("p", path).is_err());
    }
}
