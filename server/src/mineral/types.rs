//! plan-mineral-v1 §1 / §7 — `MineralId` 正典 + 品阶 / 范畴枚举。
//!
//! 15 个 mineral_id 与 docs/library/ecology/矿物录.json 1:1 对齐。
//! 字符串域同时是 inventory item NBT `mineral_id` 字段、forge / alchemy 配方
//! `material` 字段以及 server↔agent IPC 的 wire 表示。
//!
//! 命名遵循 worldview §三 末法命名原则（禁玄/陨/星/仙词头）。

use std::fmt;

use serde::{Deserialize, Serialize};

/// 矿物品阶 — plan-mineral-v1 §0.2。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MineralRarity {
    /// 凡（1）— 最浅常见，凡铁炉可炼。
    Fan,
    /// 灵（2）— 灵铁炉可炼。
    Ling,
    /// 稀（3）— 稀铁炉可炼，触发劫气标记起点。
    Xi,
    /// 遗（4）— 上古遗物级，event-only。
    Yi,
}

impl MineralRarity {
    pub fn tier(self) -> u8 {
        match self {
            Self::Fan => 1,
            Self::Ling => 2,
            Self::Xi => 3,
            Self::Yi => 4,
        }
    }
}

/// 矿物范畴 — plan §1 四表对应分组。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MineralCategory {
    Metal,
    Crystal,
    AlchemyAux,
    LingShi,
}

/// 15 个正典 mineral_id —— 与矿物录.json §金属/灵晶/丹砂辅料/灵石 一一对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MineralId {
    // 金属系
    FanTie,
    CuTie,
    ZaGang,
    LingTie,
    SuiTie,
    CanTie,
    KuJin,
    // 灵晶系
    LingJing,
    YuSui,
    WuYao,
    // 丹砂辅料
    DanSha,
    ZhuSha,
    XiongHuang,
    XiePhen,
    // 灵石燃料层
    LingShiFan,
    LingShiZhong,
    LingShiShang,
    LingShiYi,
}

impl MineralId {
    /// 全部正典 id，按 plan §1 表列出顺序 — 用于 registry seed / 单测 enumeration。
    pub const ALL: &'static [Self] = &[
        Self::FanTie,
        Self::CuTie,
        Self::ZaGang,
        Self::LingTie,
        Self::SuiTie,
        Self::CanTie,
        Self::KuJin,
        Self::LingJing,
        Self::YuSui,
        Self::WuYao,
        Self::DanSha,
        Self::ZhuSha,
        Self::XiongHuang,
        Self::XiePhen,
        Self::LingShiFan,
        Self::LingShiZhong,
        Self::LingShiShang,
        Self::LingShiYi,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::FanTie => "fan_tie",
            Self::CuTie => "cu_tie",
            Self::ZaGang => "za_gang",
            Self::LingTie => "ling_tie",
            Self::SuiTie => "sui_tie",
            Self::CanTie => "can_tie",
            Self::KuJin => "ku_jin",
            Self::LingJing => "ling_jing",
            Self::YuSui => "yu_sui",
            Self::WuYao => "wu_yao",
            Self::DanSha => "dan_sha",
            Self::ZhuSha => "zhu_sha",
            Self::XiongHuang => "xiong_huang",
            Self::XiePhen => "xie_fen",
            Self::LingShiFan => "ling_shi_fan",
            Self::LingShiZhong => "ling_shi_zhong",
            Self::LingShiShang => "ling_shi_shang",
            Self::LingShiYi => "ling_shi_yi",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|id| id.as_str() == s)
    }

    pub fn rarity(self) -> MineralRarity {
        match self {
            Self::FanTie | Self::CuTie | Self::DanSha | Self::LingShiFan => MineralRarity::Fan,
            Self::ZaGang
            | Self::LingTie
            | Self::LingJing
            | Self::YuSui
            | Self::ZhuSha
            | Self::XiongHuang
            | Self::LingShiZhong => MineralRarity::Ling,
            Self::SuiTie | Self::CanTie | Self::WuYao | Self::XiePhen | Self::LingShiShang => {
                MineralRarity::Xi
            }
            Self::KuJin | Self::LingShiYi => MineralRarity::Yi,
        }
    }

    pub fn category(self) -> MineralCategory {
        match self {
            Self::FanTie
            | Self::CuTie
            | Self::ZaGang
            | Self::LingTie
            | Self::SuiTie
            | Self::CanTie
            | Self::KuJin => MineralCategory::Metal,
            Self::LingJing | Self::YuSui | Self::WuYao => MineralCategory::Crystal,
            Self::DanSha | Self::ZhuSha | Self::XiongHuang | Self::XiePhen => {
                MineralCategory::AlchemyAux
            }
            Self::LingShiFan | Self::LingShiZhong | Self::LingShiShang | Self::LingShiYi => {
                MineralCategory::LingShi
            }
        }
    }

    /// vanilla 1.20.1 block 名（无 `minecraft:` 前缀）— plan §4.1 改色映射 source。
    /// 同 vanilla block 被多 mineral 占用时由 server 按 biome/zone 区分（§4.2）。
    pub fn vanilla_block(self) -> &'static str {
        match self {
            Self::FanTie => "iron_ore",
            Self::CuTie => "deepslate_iron_ore",
            Self::ZaGang => "copper_ore",
            Self::LingTie => "redstone_ore",
            Self::SuiTie => "ancient_debris",
            Self::CanTie => "obsidian",
            Self::KuJin => "gold_ore",
            Self::LingJing => "emerald_ore",
            Self::YuSui => "lapis_ore",
            Self::WuYao => "coal_ore",
            Self::DanSha => "redstone_ore",
            Self::ZhuSha => "nether_gold_ore",
            Self::XiongHuang => "nether_gold_ore",
            Self::XiePhen => "nether_quartz_ore",
            Self::LingShiFan => "diamond_ore",
            Self::LingShiZhong => "diamond_ore",
            Self::LingShiShang => "diamond_ore",
            Self::LingShiYi => "diamond_ore",
        }
    }

    /// 凡铁炉=1，灵铁炉=2，稀铁炉=3。0 表示该 mineral 不入炉（如丹砂、灵石、灵晶）。
    pub fn forge_tier_min(self) -> u8 {
        match self.category() {
            MineralCategory::Metal => self.rarity().tier(),
            _ => 0,
        }
    }
}

impl fmt::Display for MineralId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_has_15_plus_3_extra_lingshi_tiers() {
        // §1 表面上 15 mineral：metal 7 + crystal 3 + alchemy 4 + lingshi 1 (按"灵石"概念)。
        // 但灵石分四档独立 mineral_id，所以实际 enum variant 数 = 18。
        assert_eq!(MineralId::ALL.len(), 18);
    }

    #[test]
    fn as_str_roundtrip() {
        for &id in MineralId::ALL {
            assert_eq!(MineralId::from_str(id.as_str()), Some(id));
        }
    }

    #[test]
    fn from_str_rejects_unknown() {
        assert_eq!(MineralId::from_str("xuan_tie"), None);
        assert_eq!(MineralId::from_str("yi_beast_bone"), None);
        assert_eq!(MineralId::from_str(""), None);
    }

    #[test]
    fn rarity_categories_consistent() {
        // 灵石按品阶 1/2/3/4 各一档
        assert_eq!(MineralId::LingShiFan.rarity().tier(), 1);
        assert_eq!(MineralId::LingShiZhong.rarity().tier(), 2);
        assert_eq!(MineralId::LingShiShang.rarity().tier(), 3);
        assert_eq!(MineralId::LingShiYi.rarity().tier(), 4);
        // 金属按 plan §1.1 表
        assert_eq!(MineralId::FanTie.rarity().tier(), 1);
        assert_eq!(MineralId::SuiTie.rarity().tier(), 3);
        assert_eq!(MineralId::KuJin.rarity().tier(), 4);
    }

    #[test]
    fn forge_tier_min_only_for_metals() {
        assert_eq!(MineralId::FanTie.forge_tier_min(), 1);
        assert_eq!(MineralId::ZaGang.forge_tier_min(), 2);
        assert_eq!(MineralId::SuiTie.forge_tier_min(), 3);
        assert_eq!(MineralId::KuJin.forge_tier_min(), 4);
        // 非金属不入炉
        assert_eq!(MineralId::DanSha.forge_tier_min(), 0);
        assert_eq!(MineralId::LingShiFan.forge_tier_min(), 0);
        assert_eq!(MineralId::LingJing.forge_tier_min(), 0);
    }

    #[test]
    fn vanilla_block_mapping_matches_plan_4_1() {
        assert_eq!(MineralId::FanTie.vanilla_block(), "iron_ore");
        assert_eq!(MineralId::CuTie.vanilla_block(), "deepslate_iron_ore");
        assert_eq!(MineralId::SuiTie.vanilla_block(), "ancient_debris");
        assert_eq!(MineralId::WuYao.vanilla_block(), "coal_ore");
        // ling_tie 与 dan_sha 共占 redstone_ore（§4.2 biome 隔离）
        assert_eq!(MineralId::LingTie.vanilla_block(), "redstone_ore");
        assert_eq!(MineralId::DanSha.vanilla_block(), "redstone_ore");
        // 灵石四档共占 diamond_ore（§1.4 — 单 block 多档由 NBT 区分）
        assert_eq!(MineralId::LingShiFan.vanilla_block(), "diamond_ore");
        assert_eq!(MineralId::LingShiYi.vanilla_block(), "diamond_ore");
    }

    #[test]
    fn display_shows_canonical_id() {
        assert_eq!(format!("{}", MineralId::SuiTie), "sui_tie");
    }
}
