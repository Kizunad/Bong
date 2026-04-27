//! plan-tsy-container-v1 §1.5 / §4 — TSY 容器 spawn 配置加载 + origin modifier。
//!
//! 配置文件：`server/tsy_containers.json`。每个 family × depth 列出要 spawn 的
//! 容器 spec（kind / count / loot_pool）。`/tsy-spawn` 命令读这个 registry，
//! 按 origin（family_id 前缀）应用 count 乘数，然后在 zone AABB 内随机撒点
//! （避开 `Zone.blocked_tiles`）。
//!
//! 本模块只管"加载 + 决议出 spawn 列表"，实际 entity spawn 在
//! `tsy_dev_command.rs` / 后续 worldgen 自动 spawn 的消费侧。

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use valence::math::DVec3;
use valence::prelude::{bevy_ecs, Resource};

use crate::inventory::ancient_relics::AncientRelicSource;
use crate::world::tsy_container::ContainerKind;
use crate::world::zone::TsyDepth;

/// TSY 容器 spawn 配置 resource。启动时从 `server/tsy_containers.json` 加载。
#[derive(Debug, Default, Resource)]
pub struct TsyContainerSpawnRegistry {
    families: HashMap<String, TsyFamilyContainers>,
}

impl TsyContainerSpawnRegistry {
    pub fn from_families(families: HashMap<String, TsyFamilyContainers>) -> Self {
        Self { families }
    }

    pub fn get(&self, family_id: &str) -> Option<&TsyFamilyContainers> {
        self.families.get(family_id)
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.families.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.families.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TsyFamilyContainers {
    pub shallow: Vec<ContainerSpec>,
    pub mid: Vec<ContainerSpec>,
    pub deep: Vec<ContainerSpec>,
}

impl TsyFamilyContainers {
    pub fn for_depth(&self, depth: TsyDepth) -> &[ContainerSpec] {
        match depth {
            TsyDepth::Shallow => &self.shallow,
            TsyDepth::Mid => &self.mid,
            TsyDepth::Deep => &self.deep,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContainerSpec {
    pub kind: ContainerKind,
    pub count: u32,
    pub loot_pool_id: String,
}

pub const DEFAULT_TSY_CONTAINERS_PATH: &str = "tsy_containers.json";

pub fn load_tsy_container_spawn_registry() -> Result<TsyContainerSpawnRegistry, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_TSY_CONTAINERS_PATH);
    load_tsy_container_spawn_registry_from_path(path)
}

pub fn load_tsy_container_spawn_registry_from_path(
    path: impl AsRef<Path>,
) -> Result<TsyContainerSpawnRegistry, String> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read tsy_containers {}: {e}", path.display()))?;
    let raw: TsyContainersJson = serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse tsy_containers {}: {e}", path.display()))?;

    let mut families = HashMap::with_capacity(raw.families.len());
    for (family_id, raw_fam) in raw.families {
        let fam = raw_fam
            .try_into_family(&family_id, path)
            .map_err(|e| format!("family `{family_id}`: {e}"))?;
        families.insert(family_id, fam);
    }

    Ok(TsyContainerSpawnRegistry::from_families(families))
}

/// plan §4.2 — 起源 → 容器数量乘数表。
///
/// origin 由 family_id 前缀决定：`tsy_<origin>_<id>`（`zongmen_*` / `zhanchang_*`
/// / `gaoshou_*` 等）。匹配规则按"最长前缀"，未匹配 → 全部 1.0。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OriginMultiplier {
    pub dry_corpse_x: f32,
    pub storage_pouch_x: f32,
    pub stone_casket_x: f32,
    pub relic_core_x: f32,
}

impl OriginMultiplier {
    /// 任何容器类型 ×1.0 的中性乘数。
    pub const NEUTRAL: Self = Self {
        dry_corpse_x: 1.0,
        storage_pouch_x: 1.0,
        stone_casket_x: 1.0,
        relic_core_x: 1.0,
    };

    /// 取某 ContainerKind 的乘数（Skeleton 走 dry_corpse_x，按 plan 表
    /// "干尸/骨架"同列处理）。
    pub fn for_kind(self, kind: ContainerKind) -> f32 {
        match kind {
            ContainerKind::DryCorpse | ContainerKind::Skeleton => self.dry_corpse_x,
            ContainerKind::StoragePouch => self.storage_pouch_x,
            ContainerKind::StoneCasket => self.stone_casket_x,
            ContainerKind::RelicCore => self.relic_core_x,
        }
    }
}

/// plan §4.2 表硬编码：4 类起源前缀 × 4 类容器乘数。
///
/// 不写成 JSON 配置 —— 起源是设计常量，不应运营调（运营要调直接调
/// container count / loot pool weight）。后续如有更多起源类型再扩。
pub fn origin_multiplier_for_family(family_id: &str) -> OriginMultiplier {
    // 最长前缀优先：tankuozun → 大能陨落，zongmen → 宗门，zhanchang → 战场，gaoshou → 高手
    if family_id.starts_with("tsy_tankuozun") {
        OriginMultiplier {
            dry_corpse_x: 0.7,
            storage_pouch_x: 0.8,
            stone_casket_x: 0.5,
            relic_core_x: 1.3,
        }
    } else if family_id.starts_with("tsy_zongmen") {
        OriginMultiplier {
            dry_corpse_x: 1.0,
            storage_pouch_x: 1.2,
            stone_casket_x: 1.5,
            relic_core_x: 1.0,
        }
    } else if family_id.starts_with("tsy_zhanchang") {
        OriginMultiplier {
            dry_corpse_x: 1.3,
            storage_pouch_x: 0.5,
            stone_casket_x: 0.4,
            relic_core_x: 0.6,
        }
    } else if family_id.starts_with("tsy_gaoshou") {
        OriginMultiplier {
            dry_corpse_x: 1.0,
            storage_pouch_x: 1.0,
            stone_casket_x: 0.7,
            relic_core_x: 0.5,
        }
    } else {
        OriginMultiplier::NEUTRAL
    }
}

/// 应用乘数后的最终 count（向上取整，最少 0）。
pub fn apply_origin_multiplier(
    base_count: u32,
    kind: ContainerKind,
    mult: OriginMultiplier,
) -> u32 {
    let scaled = (base_count as f32) * mult.for_kind(kind);
    if scaled <= 0.0 {
        0
    } else {
        scaled.ceil() as u32
    }
}

/// 在给定 AABB 内随机采样一个不在 blocked tile 上的位置。
///
/// `blocked_tiles` 通常是 `(i32, i32)` 的 (x, z) 列表。`max_attempts` 默认 20，
/// 全部撞到 blocked → 返回 `None`（caller 决定 skip 还是 warn）。
pub fn sample_position_avoiding_blocks(
    bounds: (DVec3, DVec3),
    blocked_tiles: &[(i32, i32)],
    seed: u64,
    max_attempts: u32,
) -> Option<DVec3> {
    let (min, max) = bounds;
    if max.x < min.x || max.y < min.y || max.z < min.z {
        return None;
    }
    let mut rng = SpawnRng::new(seed);
    for _ in 0..max_attempts {
        let x = lerp(min.x, max.x, rng.next_f64());
        let y = lerp(min.y, max.y, rng.next_f64());
        let z = lerp(min.z, max.z, rng.next_f64());
        let tile = (x.floor() as i32, z.floor() as i32);
        if !blocked_tiles.contains(&tile) {
            return Some(DVec3::new(x, y, z));
        }
    }
    None
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

struct SpawnRng {
    state: u64,
}

impl SpawnRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_add(0xD1B5_4A32_D192_ED03),
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0xD1B5_4A32_D192_ED03);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_f64(&mut self) -> f64 {
        // 53-bit mantissa 满足 [0, 1) 均匀
        ((self.next_u64() >> 11) as f64) / ((1u64 << 53) as f64)
    }
}

/// plan §6 —— family origin 推到 ancient relic source class。仅供 RelicCore
/// 容器 roll loot 时确定上古遗物 source 用。
///
/// 与 `inventory::tsy_loot_spawn::source_class_from_family_id` 保持同语义；
/// 这里是为容器 spawn 路径独立暴露（避免 spawn 模块 import inventory 造成耦合）。
pub fn relic_source_for_family(family_id: &str) -> AncientRelicSource {
    if family_id.starts_with("tsy_tankuozun") {
        AncientRelicSource::DaoLord
    } else if family_id.starts_with("tsy_zongmen") {
        AncientRelicSource::SectRuins
    } else if family_id.starts_with("tsy_zhanchang") {
        AncientRelicSource::BattleSediment
    } else {
        // 其他（含 lingxu / gaoshou / 未分类）默认归为 SectRuins
        AncientRelicSource::SectRuins
    }
}

#[derive(Deserialize)]
struct TsyContainersJson {
    #[serde(default)]
    families: HashMap<String, TsyFamilyContainersJson>,
}

#[derive(Deserialize)]
struct TsyFamilyContainersJson {
    #[serde(default)]
    shallow: Vec<ContainerSpecJson>,
    #[serde(default)]
    mid: Vec<ContainerSpecJson>,
    #[serde(default)]
    deep: Vec<ContainerSpecJson>,
}

#[derive(Deserialize)]
struct ContainerSpecJson {
    kind: String,
    count: u32,
    loot_pool: String,
}

impl TsyFamilyContainersJson {
    fn try_into_family(
        self,
        family_id: &str,
        source: &Path,
    ) -> Result<TsyFamilyContainers, String> {
        let convert =
            |raw: Vec<ContainerSpecJson>, layer_name: &str| -> Result<Vec<ContainerSpec>, String> {
                let mut out = Vec::with_capacity(raw.len());
                for spec in raw {
                    let kind = ContainerKind::from_str(&spec.kind).ok_or_else(|| {
                        format!(
                        "{} family `{family_id}` layer `{layer_name}` unknown container kind `{}`",
                        source.display(),
                        spec.kind
                    )
                    })?;
                    if spec.loot_pool.is_empty() {
                        return Err(format!(
                        "{} family `{family_id}` layer `{layer_name}` kind `{}` empty loot_pool",
                        source.display(),
                        spec.kind
                    ));
                    }
                    out.push(ContainerSpec {
                        kind,
                        count: spec.count,
                        loot_pool_id: spec.loot_pool,
                    });
                }
                Ok(out)
            };
        Ok(TsyFamilyContainers {
            shallow: convert(self.shallow, "shallow")?,
            mid: convert(self.mid, "mid")?,
            deep: convert(self.deep, "deep")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_default_tsy_containers_json() {
        let reg =
            load_tsy_container_spawn_registry().expect("default tsy_containers.json must parse");
        let fam = reg
            .get("tsy_lingxu_01")
            .expect("lingxu_01 family must exist");
        assert!(!fam.shallow.is_empty());
        assert!(!fam.mid.is_empty());
        assert!(!fam.deep.is_empty());

        // plan §4.3 — deep 层 relic_core 数量决定 lifecycle 骨架数；
        // 跟 P2 的 initial relics_remaining 对齐到 3
        let relic_count: u32 = fam
            .for_depth(TsyDepth::Deep)
            .iter()
            .filter(|s| s.kind == ContainerKind::RelicCore)
            .map(|s| s.count)
            .sum();
        assert_eq!(
            relic_count, 3,
            "deep 层 relic_core 总数应为 3（与 P2 对齐）"
        );
    }

    #[test]
    fn origin_multiplier_picks_correct_table() {
        let m = origin_multiplier_for_family("tsy_tankuozun_xiyou_01");
        assert_eq!(m.relic_core_x, 1.3);

        let m = origin_multiplier_for_family("tsy_zongmen_qingyun_03");
        assert_eq!(m.stone_casket_x, 1.5);

        let m = origin_multiplier_for_family("tsy_zhanchang_north_07");
        assert_eq!(m.dry_corpse_x, 1.3);

        let m = origin_multiplier_for_family("tsy_gaoshou_xianglong_02");
        assert_eq!(m.relic_core_x, 0.5);

        let m = origin_multiplier_for_family("tsy_lingxu_01");
        assert_eq!(m, OriginMultiplier::NEUTRAL);
    }

    #[test]
    fn apply_origin_multiplier_ceils_and_clamps() {
        let m = OriginMultiplier {
            dry_corpse_x: 1.3,
            storage_pouch_x: 0.5,
            stone_casket_x: 0.4,
            relic_core_x: 0.6,
        };
        // 12 * 1.3 = 15.6 → 16
        assert_eq!(apply_origin_multiplier(12, ContainerKind::DryCorpse, m), 16);
        // 4 * 0.5 = 2.0 → 2
        assert_eq!(
            apply_origin_multiplier(4, ContainerKind::StoragePouch, m),
            2
        );
        // 1 * 0.4 = 0.4 → 1（ceil）
        assert_eq!(apply_origin_multiplier(1, ContainerKind::StoneCasket, m), 1);
        // 0 → 0
        assert_eq!(apply_origin_multiplier(0, ContainerKind::RelicCore, m), 0);
    }

    #[test]
    fn skeleton_uses_dry_corpse_multiplier() {
        let m = OriginMultiplier {
            dry_corpse_x: 1.3,
            storage_pouch_x: 0.5,
            stone_casket_x: 0.4,
            relic_core_x: 0.6,
        };
        assert_eq!(m.for_kind(ContainerKind::Skeleton), 1.3);
    }

    #[test]
    fn sample_position_returns_some_when_aabb_open() {
        let bounds = (DVec3::new(0.0, 64.0, 0.0), DVec3::new(10.0, 65.0, 10.0));
        let pos = sample_position_avoiding_blocks(bounds, &[], 1, 5);
        let p = pos.expect("应能采到一个位置");
        assert!(p.x >= 0.0 && p.x <= 10.0);
        assert!(p.z >= 0.0 && p.z <= 10.0);
    }

    #[test]
    fn sample_position_returns_none_when_all_tiles_blocked() {
        // 1x1 AABB，唯一一个 tile (0, 0) 被屏蔽
        let bounds = (DVec3::new(0.0, 64.0, 0.0), DVec3::new(0.99, 65.0, 0.99));
        let pos = sample_position_avoiding_blocks(bounds, &[(0, 0)], 1, 20);
        assert!(pos.is_none(), "全部 tile 被屏蔽应返回 None");
    }

    #[test]
    fn relic_source_for_family_dispatches() {
        assert_eq!(
            relic_source_for_family("tsy_tankuozun_a_01"),
            AncientRelicSource::DaoLord
        );
        assert_eq!(
            relic_source_for_family("tsy_zongmen_a_01"),
            AncientRelicSource::SectRuins
        );
        assert_eq!(
            relic_source_for_family("tsy_zhanchang_a_01"),
            AncientRelicSource::BattleSediment
        );
        // unmapped → SectRuins fallback
        assert_eq!(
            relic_source_for_family("tsy_lingxu_01"),
            AncientRelicSource::SectRuins
        );
    }

    #[test]
    fn unknown_kind_in_json_is_rejected() {
        let bad: TsyContainersJson = serde_json::from_str(
            r#"{"families":{"tsy_x_01":{"shallow":[{"kind":"INVALID","count":1,"loot_pool":"p"}]}}}"#,
        )
        .unwrap();
        let path = Path::new("test.json");
        for (fid, raw) in bad.families {
            assert!(raw.try_into_family(&fid, path).is_err());
        }
    }
}
