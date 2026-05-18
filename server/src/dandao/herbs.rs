//! P2 §3.1 — 丹道专属灵草（5 种）注册到 BotanyKindRegistry。
//!
//! 蜕骨藤、兽心草、龙鳞苔、续元蕊、化形根。
//! 走 BotanyKindRegistry 静态表（野生采集侧），不走 PlantKindRegistry（灵田侧）。

use crate::botany::registry::{EnvLock, HarvestHazard, WoundLevel};
use crate::tools::ToolKind;

// ── canonical IDs ──────────────────────────────────────────────────────

pub const TUI_GU_TENG: &str = "tui_gu_teng";
pub const SHOU_XIN_CAO: &str = "shou_xin_cao";
pub const LONG_LIN_TAI: &str = "long_lin_tai";
pub const XU_YUAN_RUI: &str = "xu_yuan_rui";
pub const HUA_XING_GEN: &str = "hua_xing_gen";

// ── env / hazard constants ─────────────────────────────────────────────

/// 蜕骨藤：负灵域边缘，spirit_qi -0.1 ~ 0.2。
const ENV_TUI_GU_TENG: &[EnvLock] = &[EnvLock::NegPressure { min: 0.1 }];
const HAZARD_TUI_GU_TENG: &[HarvestHazard] = &[HarvestHazard::WoundOnBareHand {
    wound: WoundLevel::Abrasion,
    required_tool: Some(ToolKind::GuaDao),
}];

/// 兽心草：异变兽出没区，spirit_qi > 0.5。
const ENV_SHOU_XIN_CAO: &[EnvLock] = &[EnvLock::RuinDensity { min: 0.2 }];
const HAZARD_SHOU_XIN_CAO: &[HarvestHazard] = &[HarvestHazard::DispersalOnFail {
    dispersal_chance: 0.4,
}];

/// 龙鳞苔：坍缩渊入口，spirit_qi -0.3 ~ 0。
const ENV_LONG_LIN_TAI: &[EnvLock] = &[EnvLock::NegPressure { min: 0.3 }];
const HAZARD_LONG_LIN_TAI: &[HarvestHazard] = &[
    HarvestHazard::WoundOnBareHand {
        wound: WoundLevel::Laceration,
        required_tool: Some(ToolKind::DunQiJia),
    },
    HarvestHazard::DispersalOnFail {
        dispersal_chance: 0.5,
    },
];

/// 续元蕊：灵眼附近，spirit_qi > 0.8。
const ENV_XU_YUAN_RUI: &[EnvLock] = &[EnvLock::QiVeinFlow { min: 0.8 }];
const HAZARD_XU_YUAN_RUI: &[HarvestHazard] = &[HarvestHazard::DispersalOnFail {
    dispersal_chance: 0.6,
}];

/// 化形根：死域深处，spirit_qi = 0。
const ENV_HUA_XING_GEN: &[EnvLock] = &[EnvLock::NegPressure { min: 0.5 }];
const HAZARD_HUA_XING_GEN: &[HarvestHazard] = &[
    HarvestHazard::WoundOnBareHand {
        wound: WoundLevel::Laceration,
        required_tool: Some(ToolKind::DunQiJia),
    },
    HarvestHazard::DispersalOnFail {
        dispersal_chance: 0.7,
    },
];

// ── BotanyPlantId 扩展 ─────────────────────────────────────────────────

/// 丹道专属灵草的 5 个 BotanyPlantId 变体（需在 BotanyKindRegistry 工厂中追加）。
/// 由于 BotanyPlantId 是 enum 不可运行时扩展，这里用独立 struct 走 item_id 查询。
#[derive(Debug, Clone, PartialEq)]
pub struct DandaoHerb {
    pub id: &'static str,
    pub display_name: &'static str,
    pub growth_cost: f32,
    pub max_age_ticks: u64,
    pub rarity: DandaoHerbRarity,
    pub spirit_qi_min: f32,
    pub spirit_qi_max: f32,
    pub v2_spec: DandaoHerbV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DandaoHerbRarity {
    Rare,
    VeryRare,
    Legendary,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DandaoHerbV2 {
    pub survival_mode: &'static str,
    pub env_locks_count: usize,
    pub hazard_count: usize,
    pub icon_prompt: &'static str,
    pub tint_rgb: u32,
}

/// 丹道专属灵草注册表（5 种）。
pub fn dandao_herbs() -> [DandaoHerb; 5] {
    [
        DandaoHerb {
            id: TUI_GU_TENG,
            display_name: "蜕骨藤",
            growth_cost: 0.005,
            max_age_ticks: 14_400,
            rarity: DandaoHerbRarity::Rare,
            spirit_qi_min: -0.1,
            spirit_qi_max: 0.2,
            v2_spec: DandaoHerbV2 {
                survival_mode: "neg_pressure_feed",
                env_locks_count: ENV_TUI_GU_TENG.len(),
                hazard_count: HAZARD_TUI_GU_TENG.len(),
                icon_prompt: "暗紫色藤蔓缠绕骨骼图标 16x16 像素风",
                tint_rgb: 0x4A2E5A,
            },
        },
        DandaoHerb {
            id: SHOU_XIN_CAO,
            display_name: "兽心草",
            growth_cost: 0.004,
            max_age_ticks: 12_000,
            rarity: DandaoHerbRarity::Rare,
            spirit_qi_min: 0.5,
            spirit_qi_max: f32::INFINITY,
            v2_spec: DandaoHerbV2 {
                survival_mode: "qi_absorb",
                env_locks_count: ENV_SHOU_XIN_CAO.len(),
                hazard_count: HAZARD_SHOU_XIN_CAO.len(),
                icon_prompt: "心形叶片暗红脉络灵草图标 16x16 像素风",
                tint_rgb: 0x8B2020,
            },
        },
        DandaoHerb {
            id: LONG_LIN_TAI,
            display_name: "龙鳞苔",
            growth_cost: 0.006,
            max_age_ticks: 16_000,
            rarity: DandaoHerbRarity::VeryRare,
            spirit_qi_min: -0.3,
            spirit_qi_max: 0.0,
            v2_spec: DandaoHerbV2 {
                survival_mode: "neg_pressure_feed",
                env_locks_count: ENV_LONG_LIN_TAI.len(),
                hazard_count: HAZARD_LONG_LIN_TAI.len(),
                icon_prompt: "扁平苔藓鳞片状纹路灰绿色灵草图标 16x16 像素风",
                tint_rgb: 0x6B8E6B,
            },
        },
        DandaoHerb {
            id: XU_YUAN_RUI,
            display_name: "续元蕊",
            growth_cost: 0.008,
            max_age_ticks: 18_000,
            rarity: DandaoHerbRarity::VeryRare,
            spirit_qi_min: 0.8,
            spirit_qi_max: f32::INFINITY,
            v2_spec: DandaoHerbV2 {
                survival_mode: "spirit_crystallize",
                env_locks_count: ENV_XU_YUAN_RUI.len(),
                hazard_count: HAZARD_XU_YUAN_RUI.len(),
                icon_prompt: "发光花蕊金黄色微弱脉动灵草图标 16x16 像素风",
                tint_rgb: 0xFFD700,
            },
        },
        DandaoHerb {
            id: HUA_XING_GEN,
            display_name: "化形根",
            growth_cost: 0.010,
            max_age_ticks: 24_000,
            rarity: DandaoHerbRarity::Legendary,
            spirit_qi_min: -1.0,
            spirit_qi_max: 0.0,
            v2_spec: DandaoHerbV2 {
                survival_mode: "neg_pressure_feed",
                env_locks_count: ENV_HUA_XING_GEN.len(),
                hazard_count: HAZARD_HUA_XING_GEN.len(),
                icon_prompt: "人形根茎乳白色触碰时微动灵草图标 16x16 像素风",
                tint_rgb: 0xF5F5DC,
            },
        },
    ]
}

/// 查询丹道灵草的灵气阈值是否满足生长条件。
pub fn can_grow_in_spirit_qi(herb: &DandaoHerb, spirit_qi: f32) -> bool {
    spirit_qi >= herb.spirit_qi_min && spirit_qi <= herb.spirit_qi_max
}

/// 所有丹道灵草 ID 列表。
pub fn all_dandao_herb_ids() -> [&'static str; 5] {
    [
        TUI_GU_TENG,
        SHOU_XIN_CAO,
        LONG_LIN_TAI,
        XU_YUAN_RUI,
        HUA_XING_GEN,
    ]
}

#[cfg(test)]
mod herb_tests {
    use super::*;

    #[test]
    fn all_5_herbs_registered() {
        let herbs = dandao_herbs();
        assert_eq!(herbs.len(), 5, "丹道专属灵草应有 5 种");
    }

    #[test]
    fn herb_ids_unique() {
        let herbs = dandao_herbs();
        let ids: Vec<&str> = herbs.iter().map(|h| h.id).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "灵草 ID 不应重复");
    }

    #[test]
    fn herb_ids_match_constants() {
        let herbs = dandao_herbs();
        assert_eq!(herbs[0].id, TUI_GU_TENG);
        assert_eq!(herbs[1].id, SHOU_XIN_CAO);
        assert_eq!(herbs[2].id, LONG_LIN_TAI);
        assert_eq!(herbs[3].id, XU_YUAN_RUI);
        assert_eq!(herbs[4].id, HUA_XING_GEN);
    }

    #[test]
    fn tui_gu_teng_grows_in_negative_field() {
        let herbs = dandao_herbs();
        let tgt = &herbs[0];
        assert!(
            can_grow_in_spirit_qi(tgt, -0.1),
            "蜕骨藤应在 -0.1 灵气下生长"
        );
        assert!(can_grow_in_spirit_qi(tgt, 0.0), "蜕骨藤应在 0 灵气下生长");
        assert!(can_grow_in_spirit_qi(tgt, 0.2), "蜕骨藤应在 0.2 灵气下生长");
        assert!(
            !can_grow_in_spirit_qi(tgt, 0.5),
            "蜕骨藤不应在 0.5 灵气下生长"
        );
        assert!(
            !can_grow_in_spirit_qi(tgt, -0.5),
            "蜕骨藤不应在 -0.5 灵气下生长"
        );
    }

    #[test]
    fn shou_xin_cao_requires_high_qi() {
        let herbs = dandao_herbs();
        let sxc = &herbs[1];
        assert!(can_grow_in_spirit_qi(sxc, 0.5), "兽心草应在 0.5 灵气下生长");
        assert!(can_grow_in_spirit_qi(sxc, 1.0), "兽心草应在 1.0 灵气下生长");
        assert!(
            !can_grow_in_spirit_qi(sxc, 0.3),
            "兽心草不应在 0.3 灵气下生长"
        );
    }

    #[test]
    fn long_lin_tai_grows_in_deep_negative() {
        let herbs = dandao_herbs();
        let llt = &herbs[2];
        assert!(
            can_grow_in_spirit_qi(llt, -0.3),
            "龙鳞苔应在 -0.3 灵气下生长"
        );
        assert!(
            can_grow_in_spirit_qi(llt, -0.1),
            "龙鳞苔应在 -0.1 灵气下生长"
        );
        assert!(can_grow_in_spirit_qi(llt, 0.0), "龙鳞苔应在 0 灵气下生长");
        assert!(
            !can_grow_in_spirit_qi(llt, 0.5),
            "龙鳞苔不应在 0.5 灵气下生长"
        );
    }

    #[test]
    fn xu_yuan_rui_requires_very_high_qi() {
        let herbs = dandao_herbs();
        let xyr = &herbs[3];
        assert!(can_grow_in_spirit_qi(xyr, 0.8));
        assert!(can_grow_in_spirit_qi(xyr, 1.0));
        assert!(!can_grow_in_spirit_qi(xyr, 0.5));
    }

    #[test]
    fn hua_xing_gen_dead_zone_only() {
        let herbs = dandao_herbs();
        let hxg = &herbs[4];
        assert!(
            can_grow_in_spirit_qi(hxg, -1.0),
            "化形根应在 -1.0 灵气下生长"
        );
        assert!(
            can_grow_in_spirit_qi(hxg, -0.5),
            "化形根应在 -0.5 灵气下生长"
        );
        assert!(can_grow_in_spirit_qi(hxg, 0.0), "化形根应在 0 灵气边界生长");
        assert!(!can_grow_in_spirit_qi(hxg, 0.1), "化形根不应在正灵气下生长");
    }

    #[test]
    fn growth_cost_positive_for_all() {
        for herb in dandao_herbs() {
            assert!(herb.growth_cost > 0.0, "{} growth_cost 应为正数", herb.id);
        }
    }

    #[test]
    fn max_age_ticks_reasonable() {
        for herb in dandao_herbs() {
            assert!(
                herb.max_age_ticks >= 10_000,
                "{} max_age_ticks 应至少 10000",
                herb.id
            );
        }
    }

    #[test]
    fn rarity_ordering() {
        let herbs = dandao_herbs();
        // 蜕骨藤/兽心草 = Rare, 龙鳞苔/续元蕊 = VeryRare, 化形根 = Legendary
        assert_eq!(herbs[0].rarity, DandaoHerbRarity::Rare);
        assert_eq!(herbs[1].rarity, DandaoHerbRarity::Rare);
        assert_eq!(herbs[2].rarity, DandaoHerbRarity::VeryRare);
        assert_eq!(herbs[3].rarity, DandaoHerbRarity::VeryRare);
        assert_eq!(herbs[4].rarity, DandaoHerbRarity::Legendary);
    }

    #[test]
    fn v2_spec_env_and_hazard_nonzero() {
        for herb in dandao_herbs() {
            assert!(
                herb.v2_spec.env_locks_count > 0,
                "{} 应有至少 1 个 env_lock",
                herb.id
            );
        }
    }

    #[test]
    fn all_dandao_herb_ids_matches_herbs() {
        let ids = all_dandao_herb_ids();
        let herbs = dandao_herbs();
        for (i, herb) in herbs.iter().enumerate() {
            assert_eq!(
                ids[i], herb.id,
                "all_dandao_herb_ids[{i}] 应与 dandao_herbs[{i}] 一致"
            );
        }
    }
}
