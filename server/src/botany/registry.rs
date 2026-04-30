//! plan-botany-v1 §1 — 双 registry：
//!  * `PlantKindRegistry` + TOML loader（lingtian / 可种植子集）
//!  * `BotanyKindRegistry` + 22 种正典静态表（野生采集 / ecology 事件触发）
//!
//! 两者独立：lingtian 走 `PlantKindRegistry`（由 `assets/botany/plants.toml` 驱动）；
//! botany 野生 lifecycle / harvest / ecology 走 `BotanyKindRegistry`（静态 22 种）。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use valence::prelude::Resource;

use super::plant_kind::{PlantId, PlantKind};
use crate::world::zone::{BotanyZoneTag, Zone};

const DEFAULT_PLANTS_PATH: &str = "assets/botany/plants.toml";

// ============================================================================
// lingtian 侧：PlantKindRegistry（TOML 驱动，cultivable 标签过滤）
// ============================================================================

#[derive(Debug, Default)]
pub struct PlantKindRegistry {
    plants: HashMap<PlantId, PlantKind>,
}

impl Resource for PlantKindRegistry {}

impl PlantKindRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, plant: PlantKind) -> Result<(), String> {
        if self.plants.contains_key(&plant.id) {
            return Err(format!("duplicate plant id: {}", plant.id));
        }
        self.plants.insert(plant.id.clone(), plant);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&PlantKind> {
        self.plants.get(id)
    }

    pub fn len(&self) -> usize {
        self.plants.len()
    }

    // TODO: plan-lingtian-v1 注册表接入完成后取消 allow（is_empty/iter 供后续 registry 消费者使用）
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.plants.is_empty()
    }

    #[allow(dead_code)]
    pub fn iter(&self) -> impl Iterator<Item = (&PlantId, &PlantKind)> {
        self.plants.iter()
    }

    /// plan-lingtian-v1 §4 — `SeedRegistry` 由可种植子集派生。
    pub fn cultivable_ids(&self) -> impl Iterator<Item = &PlantId> {
        self.plants
            .iter()
            .filter_map(|(id, kind)| kind.cultivable.then_some(id))
    }
}

#[derive(Debug, Deserialize)]
struct PlantsFile {
    #[serde(default)]
    plant: Vec<PlantKind>,
}

pub fn load_plant_kind_registry() -> Result<PlantKindRegistry, String> {
    load_plant_kind_registry_from(Path::new(DEFAULT_PLANTS_PATH))
}

pub fn load_plant_kind_registry_from(path: &Path) -> Result<PlantKindRegistry, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    let parsed: PlantsFile =
        toml::from_str(&raw).map_err(|e| format!("parse {}: {}", path.display(), e))?;
    let mut registry = PlantKindRegistry::new();
    for plant in parsed.plant {
        registry.insert(plant)?;
    }
    Ok(registry)
}

// ============================================================================
// botany 野生侧：BotanyKindRegistry（22 种正典静态表）
// ============================================================================

// 已有 6 种（MVP 初始）
pub const CI_SHE_HAO: &str = "ci_she_hao";
pub const NING_MAI_CAO: &str = "ning_mai_cao";
pub const HUI_YUAN_ZHI: &str = "hui_yuan_zhi";
pub const CHI_SUI_CAO: &str = "chi_sui_cao";
pub const GU_YUAN_GEN: &str = "gu_yuan_gen";
pub const KONG_SHOU_HEN: &str = "kong_shou_hen";

// plan §1.1 正典扩展（末法药材十七种 + 辛草试毒录 去重 22 种）
pub const JIE_GU_RUI: &str = "jie_gu_rui";
pub const YANG_JING_TAI: &str = "yang_jing_tai";
pub const QING_ZHUO_CAO: &str = "qing_zhuo_cao";
pub const AN_SHEN_GUO: &str = "an_shen_guo";
pub const SHI_MAI_GEN: &str = "shi_mai_gen";
pub const LING_YAN_SHI_ZHI: &str = "ling_yan_shi_zhi";
pub const YE_KU_TENG: &str = "ye_ku_teng";
pub const HUI_JIN_TAI: &str = "hui_jin_tai";
pub const ZHEN_JIE_ZI: &str = "zhen_jie_zi";
pub const SHAO_HOU_MAN: &str = "shao_hou_man";
pub const TIAN_NU_JIAO: &str = "tian_nu_jiao";
pub const FU_YOU_HUA: &str = "fu_you_hua";
pub const WU_YAN_GUO: &str = "wu_yan_guo";
pub const HEI_GU_JUN: &str = "hei_gu_jun";
pub const FU_CHEN_CAO: &str = "fu_chen_cao";
pub const ZHONG_YAN_TENG: &str = "zhong_yan_teng";

// plan-botany-v2 §2 — 绝地草木拾遗十七味（野生 only，不进 PlantKindRegistry/SeedRegistry）。
pub const FU_YUAN_JUE: &str = "fu_yuan_jue";
pub const BAI_YAN_PENG: &str = "bai_yan_peng";
pub const DUAN_JI_CI: &str = "duan_ji_ci";
pub const XUE_SE_MAI_CAO: &str = "xue_se_mai_cao";
pub const YUN_DING_LAN: &str = "yun_ding_lan";
pub const XUAN_GEN_WEI: &str = "xuan_gen_wei";
pub const YING_YUAN_GU: &str = "ying_yuan_gu";
pub const XUAN_RONG_TAI: &str = "xuan_rong_tai";
pub const YUAN_NI_HONG_YU: &str = "yuan_ni_hong_yu";
pub const JING_XIN_ZAO: &str = "jing_xin_zao";
pub const XUE_PO_LIAN: &str = "xue_po_lian";
pub const JIAO_MAI_TENG: &str = "jiao_mai_teng";
pub const LIE_YUAN_TAI: &str = "lie_yuan_tai";
pub const MING_GU_GU: &str = "ming_gu_gu";
pub const BEI_WEN_ZHI: &str = "bei_wen_zhi";
pub const LING_JING_XU: &str = "ling_jing_xu";
pub const MAO_XIN_WEI: &str = "mao_xin_wei";

const HAZARD_NONE: &[HarvestHazard] = &[];
const ENV_FU_YUAN_JUE: &[EnvLock] = &[EnvLock::NegPressure { min: 0.3 }];
const HAZARD_FU_YUAN_JUE: &[HarvestHazard] = &[HarvestHazard::QiDrainOnApproach {
    radius_blocks: 5,
    drain_per_sec: 0.4,
}];
const ENV_BAI_YAN_PENG: &[EnvLock] = &[];
const HAZARD_BAI_YAN_PENG: &[HarvestHazard] = &[
    HarvestHazard::DispersalOnFail {
        dispersal_chance: 0.6,
    },
    HarvestHazard::AttractsMobs {
        mob_kind: FaunaKind::SpiritMice,
        min_count: 2,
        max_count: 5,
    },
];
const ENV_DUAN_JI_CI: &[EnvLock] = &[
    EnvLock::RuinDensity { min: 0.3 },
    EnvLock::AdjacentDecoration {
        kind: DecorationLock::Any(&["broken_spear_tree", "war_banner_post"]),
        radius: 1,
    },
];
const HAZARD_DUAN_JI_CI: &[HarvestHazard] = &[HarvestHazard::ResonanceVision {
    duration_secs: 3,
    composure_loss: 0.05,
}];
const ENV_XUE_SE_MAI_CAO: &[EnvLock] = &[EnvLock::RuinDensity { min: 0.2 }];
const HAZARD_XUE_SE_MAI_CAO: &[HarvestHazard] = &[HarvestHazard::DispersalOnFail {
    dispersal_chance: 0.4,
}];
const ENV_YUN_DING_LAN: &[EnvLock] = &[EnvLock::SkyIslandMask {
    min: 0.2,
    surface: SkyIsleSurface::Top,
}];
const HAZARD_YUN_DING_LAN: &[HarvestHazard] = &[HarvestHazard::DispersalOnFail {
    dispersal_chance: 0.7,
}];
const ENV_XUAN_GEN_WEI: &[EnvLock] = &[EnvLock::SkyIslandMask {
    min: 0.2,
    surface: SkyIsleSurface::Bottom,
}];
const HAZARD_XUAN_GEN_WEI: &[HarvestHazard] = &[HarvestHazard::WoundOnBareHand {
    wound: WoundLevel::Laceration,
    required_tool: None,
}];
const ENV_YING_YUAN_GU: &[EnvLock] = &[
    EnvLock::UndergroundTier { tier: 1 },
    EnvLock::AdjacentLightBlock { radius: 2 },
];
const HAZARD_YING_YUAN_GU: &[HarvestHazard] = &[
    HarvestHazard::DispersalOnFail {
        dispersal_chance: 0.3,
    },
    HarvestHazard::AttractsMobs {
        mob_kind: FaunaKind::MimicSpider,
        min_count: 1,
        max_count: 2,
    },
];
const ENV_XUAN_RONG_TAI: &[EnvLock] = &[EnvLock::UndergroundTier { tier: 2 }];
const HAZARD_XUAN_RONG_TAI: &[HarvestHazard] = &[HarvestHazard::WoundOnBareHand {
    wound: WoundLevel::Abrasion,
    required_tool: None,
}];
const ENV_YUAN_NI_HONG_YU: &[EnvLock] = &[
    EnvLock::UndergroundTier { tier: 3 },
    EnvLock::AdjacentDecoration {
        kind: DecorationLock::One("yuan_ni_ebony"),
        radius: 5,
    },
    EnvLock::QiVeinFlow { min: 0.5 },
];
const HAZARD_YUAN_NI_HONG_YU: &[HarvestHazard] = &[HarvestHazard::DispersalOnFail {
    dispersal_chance: 0.5,
}];
const ENV_JING_XIN_ZAO: &[EnvLock] = &[
    EnvLock::QiVeinFlow { min: 0.6 },
    EnvLock::AdjacentDecoration {
        kind: DecorationLock::Any(&["ling_yun_mangrove", "spirit_willow"]),
        radius: 8,
    },
    EnvLock::TimePhase(WaterPulsePhase::Open),
];
const HAZARD_JING_XIN_ZAO: &[HarvestHazard] = &[HarvestHazard::SeasonRequired {
    phase: WaterPulsePhase::Open,
}];
const ENV_XUE_PO_LIAN: &[EnvLock] = &[EnvLock::SnowSurface, EnvLock::QiVeinFlow { min: 0.3 }];
const HAZARD_XUE_PO_LIAN: &[HarvestHazard] = &[HarvestHazard::WoundOnBareHand {
    wound: WoundLevel::Laceration,
    required_tool: None,
}];
const ENV_JIAO_MAI_TENG: &[EnvLock] = &[
    EnvLock::FractureMask { min: 0.4 },
    EnvLock::AdjacentDecoration {
        kind: DecorationLock::One("fire_vein_cactus"),
        radius: 3,
    },
];
const HAZARD_JIAO_MAI_TENG: &[HarvestHazard] = &[HarvestHazard::WoundOnBareHand {
    wound: WoundLevel::Laceration,
    required_tool: None,
}];
const ENV_LIE_YUAN_TAI: &[EnvLock] = &[EnvLock::PortalRiftActive];
const HAZARD_LIE_YUAN_TAI: &[HarvestHazard] = &[HarvestHazard::DispersalOnFail {
    dispersal_chance: 0.4,
}];
const ENV_MING_GU_GU: &[EnvLock] = &[
    EnvLock::RuinDensity { min: 0.4 },
    EnvLock::AdjacentDecoration {
        kind: DecorationLock::One("bone_mountain"),
        radius: 3,
    },
];
const HAZARD_MING_GU_GU: &[HarvestHazard] = &[
    HarvestHazard::ResonanceVision {
        duration_secs: 5,
        composure_loss: 0.08,
    },
    HarvestHazard::AttractsMobs {
        mob_kind: FaunaKind::MimicSpider,
        min_count: 1,
        max_count: 3,
    },
];
const ENV_BEI_WEN_ZHI: &[EnvLock] = &[
    EnvLock::RuinDensity { min: 0.3 },
    EnvLock::AdjacentDecoration {
        kind: DecorationLock::One("array_disc_remnant"),
        radius: 2,
    },
];
const HAZARD_BEI_WEN_ZHI: &[HarvestHazard] = &[HarvestHazard::DispersalOnFail {
    dispersal_chance: 0.5,
}];
const ENV_LING_JING_XU: &[EnvLock] = &[
    EnvLock::AdjacentDecoration {
        kind: DecorationLock::One("qi_crystal_pillar"),
        radius: 3,
    },
    EnvLock::QiVeinFlow { min: 0.5 },
];
const HAZARD_LING_JING_XU: &[HarvestHazard] = &[
    HarvestHazard::WoundOnBareHand {
        wound: WoundLevel::Abrasion,
        required_tool: None,
    },
    HarvestHazard::DispersalOnFail {
        dispersal_chance: 0.6,
    },
];
const ENV_MAO_XIN_WEI: &[EnvLock] = &[EnvLock::AdjacentDecoration {
    kind: DecorationLock::Any(&[
        "thatched_hermitage",
        "lone_grave_mound",
        "daily_artifact_cache",
    ]),
    radius: 2,
}];

pub const KAI_MAI_CAO_ALIAS: &str = "kai_mai_cao";
pub const XUE_CAO_ALIAS: &str = "xue_cao";
pub const BAI_CAO_ALIAS: &str = "bai_cao";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BotanyPlantId {
    CiSheHao,
    NingMaiCao,
    HuiYuanZhi,
    ChiSuiCao,
    GuYuanGen,
    KongShouHen,
    JieGuRui,
    YangJingTai,
    QingZhuoCao,
    AnShenGuo,
    ShiMaiGen,
    LingYanShiZhi,
    YeKuTeng,
    HuiJinTai,
    ZhenJieZi,
    ShaoHouMan,
    TianNuJiao,
    FuYouHua,
    WuYanGuo,
    HeiGuJun,
    FuChenCao,
    ZhongYanTeng,
    FuYuanJue,
    BaiYanPeng,
    DuanJiCi,
    XueSeMaiCao,
    YunDingLan,
    XuanGenWei,
    YingYuanGu,
    XuanRongTai,
    YuanNiHongYu,
    JingXinZao,
    XuePoLian,
    JiaoMaiTeng,
    LieYuanTai,
    MingGuGu,
    BeiWenZhi,
    LingJingXu,
    MaoXinWei,
}

impl BotanyPlantId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CiSheHao => CI_SHE_HAO,
            Self::NingMaiCao => NING_MAI_CAO,
            Self::HuiYuanZhi => HUI_YUAN_ZHI,
            Self::ChiSuiCao => CHI_SUI_CAO,
            Self::GuYuanGen => GU_YUAN_GEN,
            Self::KongShouHen => KONG_SHOU_HEN,
            Self::JieGuRui => JIE_GU_RUI,
            Self::YangJingTai => YANG_JING_TAI,
            Self::QingZhuoCao => QING_ZHUO_CAO,
            Self::AnShenGuo => AN_SHEN_GUO,
            Self::ShiMaiGen => SHI_MAI_GEN,
            Self::LingYanShiZhi => LING_YAN_SHI_ZHI,
            Self::YeKuTeng => YE_KU_TENG,
            Self::HuiJinTai => HUI_JIN_TAI,
            Self::ZhenJieZi => ZHEN_JIE_ZI,
            Self::ShaoHouMan => SHAO_HOU_MAN,
            Self::TianNuJiao => TIAN_NU_JIAO,
            Self::FuYouHua => FU_YOU_HUA,
            Self::WuYanGuo => WU_YAN_GUO,
            Self::HeiGuJun => HEI_GU_JUN,
            Self::FuChenCao => FU_CHEN_CAO,
            Self::ZhongYanTeng => ZHONG_YAN_TENG,
            Self::FuYuanJue => FU_YUAN_JUE,
            Self::BaiYanPeng => BAI_YAN_PENG,
            Self::DuanJiCi => DUAN_JI_CI,
            Self::XueSeMaiCao => XUE_SE_MAI_CAO,
            Self::YunDingLan => YUN_DING_LAN,
            Self::XuanGenWei => XUAN_GEN_WEI,
            Self::YingYuanGu => YING_YUAN_GU,
            Self::XuanRongTai => XUAN_RONG_TAI,
            Self::YuanNiHongYu => YUAN_NI_HONG_YU,
            Self::JingXinZao => JING_XIN_ZAO,
            Self::XuePoLian => XUE_PO_LIAN,
            Self::JiaoMaiTeng => JIAO_MAI_TENG,
            Self::LieYuanTai => LIE_YUAN_TAI,
            Self::MingGuGu => MING_GU_GU,
            Self::BeiWenZhi => BEI_WEN_ZHI,
            Self::LingJingXu => LING_JING_XU,
            Self::MaoXinWei => MAO_XIN_WEI,
        }
    }

    pub fn from_canonical(id: &str) -> Option<Self> {
        match id {
            CI_SHE_HAO => Some(Self::CiSheHao),
            NING_MAI_CAO => Some(Self::NingMaiCao),
            HUI_YUAN_ZHI => Some(Self::HuiYuanZhi),
            CHI_SUI_CAO => Some(Self::ChiSuiCao),
            GU_YUAN_GEN => Some(Self::GuYuanGen),
            KONG_SHOU_HEN => Some(Self::KongShouHen),
            JIE_GU_RUI => Some(Self::JieGuRui),
            YANG_JING_TAI => Some(Self::YangJingTai),
            QING_ZHUO_CAO => Some(Self::QingZhuoCao),
            AN_SHEN_GUO => Some(Self::AnShenGuo),
            SHI_MAI_GEN => Some(Self::ShiMaiGen),
            LING_YAN_SHI_ZHI => Some(Self::LingYanShiZhi),
            YE_KU_TENG => Some(Self::YeKuTeng),
            HUI_JIN_TAI => Some(Self::HuiJinTai),
            ZHEN_JIE_ZI => Some(Self::ZhenJieZi),
            SHAO_HOU_MAN => Some(Self::ShaoHouMan),
            TIAN_NU_JIAO => Some(Self::TianNuJiao),
            FU_YOU_HUA => Some(Self::FuYouHua),
            WU_YAN_GUO => Some(Self::WuYanGuo),
            HEI_GU_JUN => Some(Self::HeiGuJun),
            FU_CHEN_CAO => Some(Self::FuChenCao),
            ZHONG_YAN_TENG => Some(Self::ZhongYanTeng),
            FU_YUAN_JUE => Some(Self::FuYuanJue),
            BAI_YAN_PENG => Some(Self::BaiYanPeng),
            DUAN_JI_CI => Some(Self::DuanJiCi),
            XUE_SE_MAI_CAO => Some(Self::XueSeMaiCao),
            YUN_DING_LAN => Some(Self::YunDingLan),
            XUAN_GEN_WEI => Some(Self::XuanGenWei),
            YING_YUAN_GU => Some(Self::YingYuanGu),
            XUAN_RONG_TAI => Some(Self::XuanRongTai),
            YUAN_NI_HONG_YU => Some(Self::YuanNiHongYu),
            JING_XIN_ZAO => Some(Self::JingXinZao),
            XUE_PO_LIAN => Some(Self::XuePoLian),
            JIAO_MAI_TENG => Some(Self::JiaoMaiTeng),
            LIE_YUAN_TAI => Some(Self::LieYuanTai),
            MING_GU_GU => Some(Self::MingGuGu),
            BEI_WEN_ZHI => Some(Self::BeiWenZhi),
            LING_JING_XU => Some(Self::LingJingXu),
            MAO_XIN_WEI => Some(Self::MaoXinWei),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotanyHerbAlias {
    KaiMai,
    Xue,
    Bai,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotanySpawnMode {
    ZoneRefresh,
    StaticPoint,
    /// plan §1.2.3 事件触发（兽死 / 残灰 / 天劫余波 / 负灵域裂缝）。
    /// 不参与 ZoneRefresh / StaticPoint spawn loop；不检查 zone 支持性与 spirit_qi 下限。
    EventTriggered,
}

/// plan §7 TODO 植物变异：特殊 zone 环境下的稀有变种。
/// - `Thunder` 天劫余波 zone（active_events 含 thunder / tribulation）
/// - `Tainted` 负灵域 / spirit_qi 负值 zone
///
/// 变种影响 drop 品质、XP、显示名前缀（"雷 · " / "黑 · "），但共用同一 canonical plant_kind。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlantVariant {
    #[default]
    None,
    Thunder,
    Tainted,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SurvivalMode {
    QiAbsorb,
    NegPressureFeed,
    PressureDifferential,
    SpiritCrystallize,
    RuinResonance,
    ThermalConvection,
    PortalSiphon,
    DualMetabolism,
    PhotoLuminance,
    WaterPulse,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SkyIsleSurface {
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WaterPulsePhase {
    Open,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DecorationLock {
    One(&'static str),
    Any(&'static [&'static str]),
}

impl DecorationLock {
    pub fn names(self) -> Vec<&'static str> {
        match self {
            Self::One(expected) => vec![expected],
            Self::Any(expected) => expected.to_vec(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnvLock {
    NegPressure { min: f32 },
    QiVeinFlow { min: f32 },
    FractureMask { min: f32 },
    RuinDensity { min: f32 },
    SkyIslandMask { min: f32, surface: SkyIsleSurface },
    UndergroundTier { tier: u8 },
    PortalRiftActive,
    AdjacentDecoration { kind: DecorationLock, radius: u8 },
    AdjacentLightBlock { radius: u8 },
    SnowSurface,
    TimePhase(WaterPulsePhase),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WoundLevel {
    Abrasion,
    Laceration,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FaunaKind {
    SpiritMice,
    MimicSpider,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HarvestHazard {
    QiDrainOnApproach {
        radius_blocks: u8,
        drain_per_sec: f32,
    },
    WoundOnBareHand {
        wound: WoundLevel,
        required_tool: Option<&'static str>,
    },
    DispersalOnFail {
        dispersal_chance: f32,
    },
    ResonanceVision {
        duration_secs: u8,
        composure_loss: f32,
    },
    SeasonRequired {
        phase: WaterPulsePhase,
    },
    AttractsMobs {
        mob_kind: FaunaKind,
        min_count: u8,
        max_count: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModelOverlay {
    None,
    Emissive,
    DualPhase,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BotanyV2Spec {
    pub survival_mode: SurvivalMode,
    pub env_locks: &'static [EnvLock],
    pub harvest_hazards: &'static [HarvestHazard],
    pub base_mesh_ref: &'static str,
    pub tint_rgb: u32,
    pub tint_rgb_secondary: Option<u32>,
    pub model_overlay: ModelOverlay,
    pub icon_prompt: &'static str,
}

impl PlantVariant {
    pub fn display_prefix(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Thunder => Some("雷"),
            Self::Tainted => Some("黑"),
        }
    }

    /// 加到 ItemInstance.spirit_quality 上（最终 clamp 到 [0,1]）。
    pub fn quality_modifier(self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Thunder => 0.10,
            Self::Tainted => -0.15,
        }
    }

    /// 加到 harvest XP 上：两种变种都给 bonus（均为稀有事件），品相差别只反映在 quality 上。
    /// Thunder 额外高 2，Tainted +1（挑战系数：需进负灵域采）。
    pub fn xp_delta(self) -> i64 {
        match self {
            Self::None => 0,
            Self::Thunder => 2,
            Self::Tainted => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BotanyPlantKind {
    pub id: BotanyPlantId,
    pub item_id: &'static str,
    pub zone_tags: &'static [BotanyZoneTag],
    pub density_factor: f32,
    pub growth_cost: f32,
    pub survive_threshold: f32,
    pub max_age_ticks: u64,
    pub regen_ticks: u64,
    pub spawn_mode: BotanySpawnMode,
    pub restore_ratio: f32,
    pub v2: Option<BotanyV2Spec>,
}

#[derive(Debug, Clone)]
pub struct BotanyKindRegistry {
    by_id: HashMap<BotanyPlantId, BotanyPlantKind>,
}

impl Resource for BotanyKindRegistry {}

impl Default for BotanyKindRegistry {
    fn default() -> Self {
        let kinds = [
            BotanyPlantKind {
                id: BotanyPlantId::CiSheHao,
                item_id: CI_SHE_HAO,
                zone_tags: &[BotanyZoneTag::Plains],
                density_factor: 4.0,
                growth_cost: 0.002,
                survive_threshold: 0.2,
                max_age_ticks: 6_000,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::ZoneRefresh,
                restore_ratio: 0.8,
                v2: None,
            },
            BotanyPlantKind {
                id: BotanyPlantId::NingMaiCao,
                item_id: NING_MAI_CAO,
                zone_tags: &[BotanyZoneTag::Plains],
                density_factor: 2.0,
                growth_cost: 0.003,
                survive_threshold: 0.4,
                max_age_ticks: 7_200,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::ZoneRefresh,
                restore_ratio: 0.8,
                v2: None,
            },
            BotanyPlantKind {
                id: BotanyPlantId::HuiYuanZhi,
                item_id: HUI_YUAN_ZHI,
                zone_tags: &[BotanyZoneTag::Marsh],
                density_factor: 1.5,
                growth_cost: 0.003,
                survive_threshold: 0.35,
                max_age_ticks: 8_000,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::ZoneRefresh,
                restore_ratio: 0.8,
                v2: None,
            },
            BotanyPlantKind {
                id: BotanyPlantId::ChiSuiCao,
                item_id: CHI_SUI_CAO,
                zone_tags: &[BotanyZoneTag::BloodValley],
                density_factor: 1.0,
                growth_cost: 0.005,
                survive_threshold: 0.25,
                max_age_ticks: 8_500,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::ZoneRefresh,
                restore_ratio: 0.8,
                v2: None,
            },
            BotanyPlantKind {
                id: BotanyPlantId::GuYuanGen,
                item_id: GU_YUAN_GEN,
                zone_tags: &[BotanyZoneTag::Marsh, BotanyZoneTag::Cave],
                density_factor: 0.3,
                growth_cost: 0.01,
                survive_threshold: 0.6,
                max_age_ticks: 10_000,
                regen_ticks: 7_200,
                spawn_mode: BotanySpawnMode::StaticPoint,
                restore_ratio: 0.8,
                v2: None,
            },
            // plan §1.2.3：异变兽死亡 → 尸旁生成空兽痕（library 正典）。
            // 不扣 zone spirit_qi，不受 biome 过滤，长寿命单次结实植物。
            BotanyPlantKind {
                id: BotanyPlantId::KongShouHen,
                item_id: KONG_SHOU_HEN,
                zone_tags: &[],
                density_factor: 0.0,
                growth_cost: 0.0,
                survive_threshold: -1.0,
                max_age_ticks: 20_000,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::EventTriggered,
                restore_ratio: 0.0,
                v2: None,
            },
            // ===== 常用七味 剩 4 种（plan §1.1 / 末法药材十七种）=====
            BotanyPlantKind {
                id: BotanyPlantId::JieGuRui,
                item_id: JIE_GU_RUI,
                zone_tags: &[BotanyZoneTag::Cave],
                density_factor: 0.8,
                growth_cost: 0.004,
                survive_threshold: 0.4,
                max_age_ticks: 7_500,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::ZoneRefresh,
                restore_ratio: 0.8,
                v2: None,
            },
            BotanyPlantKind {
                id: BotanyPlantId::YangJingTai,
                item_id: YANG_JING_TAI,
                // 死域边缘——plan §1.2.3 特殊生境，事件触发 + 不扣灵气；
                // DeathEdge 是动态边界不是 zone tag，故此处留空；spawn 依赖未来的死域事件系统
                zone_tags: &[],
                density_factor: 0.0,
                growth_cost: 0.0,
                survive_threshold: -1.0,
                max_age_ticks: 15_000,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::EventTriggered,
                restore_ratio: 0.0,
                v2: None,
            },
            BotanyPlantKind {
                id: BotanyPlantId::QingZhuoCao,
                item_id: QING_ZHUO_CAO,
                zone_tags: &[BotanyZoneTag::Plains, BotanyZoneTag::NegativeField],
                density_factor: 1.2,
                growth_cost: 0.003,
                survive_threshold: 0.3,
                max_age_ticks: 7_000,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::ZoneRefresh,
                restore_ratio: 0.8,
                v2: None,
            },
            BotanyPlantKind {
                id: BotanyPlantId::AnShenGuo,
                item_id: AN_SHEN_GUO,
                zone_tags: &[BotanyZoneTag::Mountain],
                density_factor: 1.0,
                growth_cost: 0.003,
                survive_threshold: 0.3,
                max_age_ticks: 9_000,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::ZoneRefresh,
                restore_ratio: 0.8,
                v2: None,
            },
            // ===== 稀见五味 剩 3 种 =====
            BotanyPlantKind {
                id: BotanyPlantId::ShiMaiGen,
                item_id: SHI_MAI_GEN,
                zone_tags: &[BotanyZoneTag::NegativeField],
                density_factor: 0.0,
                growth_cost: 0.0,
                survive_threshold: -1.0,
                max_age_ticks: 25_000,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::EventTriggered,
                restore_ratio: 0.0,
                v2: None,
            },
            // 灵眼未实装 → MVP 禁用生成（EventTriggered 占位，永不 spawn）
            BotanyPlantKind {
                id: BotanyPlantId::LingYanShiZhi,
                item_id: LING_YAN_SHI_ZHI,
                zone_tags: &[],
                density_factor: 0.0,
                growth_cost: 0.0,
                survive_threshold: -1.0,
                max_age_ticks: 30_000,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::EventTriggered,
                restore_ratio: 0.0,
                v2: None,
            },
            BotanyPlantKind {
                id: BotanyPlantId::YeKuTeng,
                item_id: YE_KU_TENG,
                zone_tags: &[BotanyZoneTag::Cave],
                density_factor: 0.2,
                growth_cost: 0.01,
                survive_threshold: 0.5,
                max_age_ticks: 12_000,
                regen_ticks: 9_000,
                spawn_mode: BotanySpawnMode::StaticPoint,
                restore_ratio: 0.8,
                v2: None,
            },
            // ===== 辛草剩 3 种（辛草试毒录）=====
            // 残灰方块未实装，挂 ResidueAsh tag 占位；EventTriggered 不自动 spawn
            BotanyPlantKind {
                id: BotanyPlantId::HuiJinTai,
                item_id: HUI_JIN_TAI,
                // ResidueAsh 是 block 级属性（残灰方块表面），非 zone tag；
                // spawn 依赖未来的残灰 block 事件系统（plan-residue 待立）
                zone_tags: &[],
                density_factor: 0.0,
                growth_cost: 0.0,
                survive_threshold: -1.0,
                max_age_ticks: 10_000,
                regen_ticks: 2_400, // 40 min @ 1t/30s 近似
                spawn_mode: BotanySpawnMode::EventTriggered,
                restore_ratio: 0.0,
                v2: None,
            },
            BotanyPlantKind {
                id: BotanyPlantId::ZhenJieZi,
                item_id: ZHEN_JIE_ZI,
                zone_tags: &[BotanyZoneTag::Mountain, BotanyZoneTag::Marsh],
                density_factor: 0.6,
                growth_cost: 0.005,
                survive_threshold: 0.3,
                max_age_ticks: 8_000,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::ZoneRefresh,
                restore_ratio: 0.8,
                v2: None,
            },
            BotanyPlantKind {
                id: BotanyPlantId::ShaoHouMan,
                item_id: SHAO_HOU_MAN,
                zone_tags: &[BotanyZoneTag::Cave],
                density_factor: 0.4,
                growth_cost: 0.008,
                survive_threshold: 0.4,
                max_age_ticks: 9_000,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::ZoneRefresh,
                restore_ratio: 0.8,
                v2: None,
            },
            // 伪灵脉焦土（天道陷阱，事件触发稍纵即逝）
            // FakeVeinBurn 是事件级临时状态，非 zone tag；spawn 依赖 plan-tribulation 的伪灵脉消散事件
            BotanyPlantKind {
                id: BotanyPlantId::TianNuJiao,
                item_id: TIAN_NU_JIAO,
                zone_tags: &[],
                density_factor: 0.0,
                growth_cost: 0.0,
                survive_threshold: -1.0,
                max_age_ticks: 1_200, // 稍纵即逝：1 分钟窗口
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::EventTriggered,
                restore_ratio: 0.0,
                v2: None,
            },
            // ===== 毒性五味（可采不可炼）=====
            BotanyPlantKind {
                id: BotanyPlantId::FuYouHua,
                item_id: FU_YOU_HUA,
                zone_tags: &[BotanyZoneTag::Plains],
                density_factor: 0.3,
                growth_cost: 0.002,
                survive_threshold: 0.2,
                max_age_ticks: 6_000,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::ZoneRefresh,
                restore_ratio: 0.8,
                v2: None,
            },
            BotanyPlantKind {
                id: BotanyPlantId::WuYanGuo,
                item_id: WU_YAN_GUO,
                zone_tags: &[BotanyZoneTag::Mountain],
                density_factor: 0.3,
                growth_cost: 0.002,
                survive_threshold: 0.25,
                max_age_ticks: 8_000,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::ZoneRefresh,
                restore_ratio: 0.8,
                v2: None,
            },
            // 生于死人之骨缝——死域事件触发；DeathEdge 非 zone tag 故此处留空
            BotanyPlantKind {
                id: BotanyPlantId::HeiGuJun,
                item_id: HEI_GU_JUN,
                zone_tags: &[],
                density_factor: 0.0,
                growth_cost: 0.0,
                survive_threshold: -1.0,
                max_age_ticks: 20_000,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::EventTriggered,
                restore_ratio: 0.0,
                v2: None,
            },
            BotanyPlantKind {
                id: BotanyPlantId::FuChenCao,
                item_id: FU_CHEN_CAO,
                zone_tags: &[BotanyZoneTag::NegativeField, BotanyZoneTag::Wastes],
                density_factor: 0.2,
                growth_cost: 0.002,
                survive_threshold: 0.0,
                max_age_ticks: 10_000,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::ZoneRefresh,
                restore_ratio: 0.8,
                v2: None,
            },
            // 毒蛊师终极原料——事件触发，极稀
            BotanyPlantKind {
                id: BotanyPlantId::ZhongYanTeng,
                item_id: ZHONG_YAN_TENG,
                zone_tags: &[BotanyZoneTag::NegativeField],
                density_factor: 0.0,
                growth_cost: 0.0,
                survive_threshold: -1.0,
                max_age_ticks: 30_000,
                regen_ticks: 0,
                spawn_mode: BotanySpawnMode::EventTriggered,
                restore_ratio: 0.0,
                v2: None,
            },
            // ===== plan-botany-v2：绝地草木拾遗十七味（v2 走 EnvLock，不走 zone tag）=====
            botany_v2_kind(
                BotanyPlantId::FuYuanJue,
                FU_YUAN_JUE,
                0.004,
                14_400,
                BotanyV2Spec {
                    survival_mode: SurvivalMode::NegPressureFeed,
                    env_locks: ENV_FU_YUAN_JUE,
                    harvest_hazards: HAZARD_FU_YUAN_JUE,
                    base_mesh_ref: "large_fern",
                    tint_rgb: 0x4A2E5A,
                    tint_rgb_secondary: None,
                    model_overlay: ModelOverlay::None,
                    icon_prompt: "dark purple reverse-breathing fern, salt-dry wasteland herb icon",
                },
            ),
            botany_v2_kind(
                BotanyPlantId::BaiYanPeng,
                BAI_YAN_PENG,
                0.002,
                8_000,
                BotanyV2Spec {
                    survival_mode: SurvivalMode::SpiritCrystallize,
                    env_locks: ENV_BAI_YAN_PENG,
                    harvest_hazards: HAZARD_BAI_YAN_PENG,
                    base_mesh_ref: "dead_bush",
                    tint_rgb: 0xF8F8E8,
                    tint_rgb_secondary: None,
                    model_overlay: ModelOverlay::None,
                    icon_prompt: "white saltbush with tiny spirit salt crystals, dry wasteland herb icon",
                },
            ),
            botany_v2_kind(
                BotanyPlantId::DuanJiCi,
                DUAN_JI_CI,
                0.004,
                12_000,
                BotanyV2Spec {
                    survival_mode: SurvivalMode::RuinResonance,
                    env_locks: ENV_DUAN_JI_CI,
                    harvest_hazards: HAZARD_DUAN_JI_CI,
                    base_mesh_ref: "sweet_berry_bush",
                    tint_rgb: 0x5C1E0F,
                    tint_rgb_secondary: None,
                    model_overlay: ModelOverlay::None,
                    icon_prompt: "blood-dark thorn growing around broken spear fragments, battlefield herb icon",
                },
            ),
            botany_v2_kind(
                BotanyPlantId::XueSeMaiCao,
                XUE_SE_MAI_CAO,
                0.003,
                10_000,
                BotanyV2Spec {
                    survival_mode: SurvivalMode::DualMetabolism,
                    env_locks: ENV_XUE_SE_MAI_CAO,
                    harvest_hazards: HAZARD_XUE_SE_MAI_CAO,
                    base_mesh_ref: "tall_grass",
                    tint_rgb: 0xC03020,
                    tint_rgb_secondary: Some(0x205040),
                    model_overlay: ModelOverlay::DualPhase,
                    icon_prompt: "two-phase vein grass, red day leaf and blue-green night leaf, toxic battlefield herb icon",
                },
            ),
            botany_v2_kind(
                BotanyPlantId::YunDingLan,
                YUN_DING_LAN,
                0.003,
                9_000,
                BotanyV2Spec {
                    survival_mode: SurvivalMode::QiAbsorb,
                    env_locks: ENV_YUN_DING_LAN,
                    harvest_hazards: HAZARD_YUN_DING_LAN,
                    base_mesh_ref: "lily_of_the_valley",
                    tint_rgb: 0xE8F4FF,
                    tint_rgb_secondary: None,
                    model_overlay: ModelOverlay::None,
                    icon_prompt: "silver-white orchid from floating cloud peak, light airy herb icon",
                },
            ),
            botany_v2_kind(
                BotanyPlantId::XuanGenWei,
                XUAN_GEN_WEI,
                0.004,
                11_000,
                BotanyV2Spec {
                    survival_mode: SurvivalMode::PressureDifferential,
                    env_locks: ENV_XUAN_GEN_WEI,
                    harvest_hazards: HAZARD_XUAN_GEN_WEI,
                    base_mesh_ref: "vine",
                    tint_rgb: 0x60D080,
                    tint_rgb_secondary: None,
                    model_overlay: ModelOverlay::None,
                    icon_prompt: "hanging green root vine with crystal tips, floating island underside herb icon",
                },
            ),
            botany_v2_kind(
                BotanyPlantId::YingYuanGu,
                YING_YUAN_GU,
                0.003,
                10_000,
                BotanyV2Spec {
                    survival_mode: SurvivalMode::PhotoLuminance,
                    env_locks: ENV_YING_YUAN_GU,
                    harvest_hazards: HAZARD_YING_YUAN_GU,
                    base_mesh_ref: "red_mushroom",
                    tint_rgb: 0xFFA040,
                    tint_rgb_secondary: None,
                    model_overlay: ModelOverlay::Emissive,
                    icon_prompt: "warm orange glowing abyss mushroom, emissive cave herb icon",
                },
            ),
            botany_v2_kind(
                BotanyPlantId::XuanRongTai,
                XUAN_RONG_TAI,
                0.004,
                12_000,
                BotanyV2Spec {
                    survival_mode: SurvivalMode::ThermalConvection,
                    env_locks: ENV_XUAN_RONG_TAI,
                    harvest_hazards: HAZARD_XUAN_RONG_TAI,
                    base_mesh_ref: "moss_carpet",
                    tint_rgb: 0x101015,
                    tint_rgb_secondary: None,
                    model_overlay: ModelOverlay::Emissive,
                    icon_prompt: "black velvet moss with faint silver glow, abyss middle-tier herb icon",
                },
            ),
            botany_v2_kind(
                BotanyPlantId::YuanNiHongYu,
                YUAN_NI_HONG_YU,
                0.008,
                18_000,
                BotanyV2Spec {
                    survival_mode: SurvivalMode::PressureDifferential,
                    env_locks: ENV_YUAN_NI_HONG_YU,
                    harvest_hazards: HAZARD_YUAN_NI_HONG_YU,
                    base_mesh_ref: "large_fern",
                    tint_rgb: 0xC02040,
                    tint_rgb_secondary: None,
                    model_overlay: ModelOverlay::None,
                    icon_prompt: "red jade fern under black abyss mud tree, precious herb icon",
                },
            ),
            botany_v2_kind(
                BotanyPlantId::JingXinZao,
                JING_XIN_ZAO,
                0.005,
                14_000,
                BotanyV2Spec {
                    survival_mode: SurvivalMode::WaterPulse,
                    env_locks: ENV_JING_XIN_ZAO,
                    harvest_hazards: HAZARD_JING_XIN_ZAO,
                    base_mesh_ref: "seagrass",
                    tint_rgb: 0x40A0A0,
                    tint_rgb_secondary: None,
                    model_overlay: ModelOverlay::Emissive,
                    icon_prompt: "cyan algae from spirit well heart, soft luminous water herb icon",
                },
            ),
            botany_v2_kind(
                BotanyPlantId::XuePoLian,
                XUE_PO_LIAN,
                0.006,
                16_000,
                BotanyV2Spec {
                    survival_mode: SurvivalMode::SpiritCrystallize,
                    env_locks: ENV_XUE_PO_LIAN,
                    harvest_hazards: HAZARD_XUE_PO_LIAN,
                    base_mesh_ref: "lily_of_the_valley",
                    tint_rgb: 0xF0F8FF,
                    tint_rgb_secondary: None,
                    model_overlay: ModelOverlay::None,
                    icon_prompt: "snow-white lotus with frost-blue rim, high snowline herb icon",
                },
            ),
            botany_v2_kind(
                BotanyPlantId::JiaoMaiTeng,
                JIAO_MAI_TENG,
                0.004,
                12_000,
                BotanyV2Spec {
                    survival_mode: SurvivalMode::PressureDifferential,
                    env_locks: ENV_JIAO_MAI_TENG,
                    harvest_hazards: HAZARD_JIAO_MAI_TENG,
                    base_mesh_ref: "weeping_vines",
                    tint_rgb: 0x301010,
                    tint_rgb_secondary: None,
                    model_overlay: ModelOverlay::Emissive,
                    icon_prompt: "charred vein vine with orange ember core, rift valley herb icon",
                },
            ),
            botany_v2_kind(
                BotanyPlantId::LieYuanTai,
                LIE_YUAN_TAI,
                0.003,
                9_000,
                BotanyV2Spec {
                    survival_mode: SurvivalMode::PortalSiphon,
                    env_locks: ENV_LIE_YUAN_TAI,
                    harvest_hazards: HAZARD_LIE_YUAN_TAI,
                    base_mesh_ref: "glow_lichen",
                    tint_rgb: 0x402060,
                    tint_rgb_secondary: None,
                    model_overlay: ModelOverlay::Emissive,
                    icon_prompt: "purple-black moss clinging to a dimensional rift, abyss portal herb icon",
                },
            ),
            botany_v2_kind(
                BotanyPlantId::MingGuGu,
                MING_GU_GU,
                0.004,
                14_000,
                BotanyV2Spec {
                    survival_mode: SurvivalMode::RuinResonance,
                    env_locks: ENV_MING_GU_GU,
                    harvest_hazards: HAZARD_MING_GU_GU,
                    base_mesh_ref: "brown_mushroom",
                    tint_rgb: 0xE8E0D0,
                    tint_rgb_secondary: None,
                    model_overlay: ModelOverlay::None,
                    icon_prompt: "bone-white mushroom growing from silent battlefield bones, TSY herb icon",
                },
            ),
            botany_v2_kind(
                BotanyPlantId::BeiWenZhi,
                BEI_WEN_ZHI,
                0.004,
                13_000,
                BotanyV2Spec {
                    survival_mode: SurvivalMode::RuinResonance,
                    env_locks: ENV_BEI_WEN_ZHI,
                    harvest_hazards: HAZARD_BEI_WEN_ZHI,
                    base_mesh_ref: "red_mushroom",
                    tint_rgb: 0x808890,
                    tint_rgb_secondary: Some(0x6020A0),
                    model_overlay: ModelOverlay::None,
                    icon_prompt: "gray-blue ganoderma with purple inscription veins, ruined sect herb icon",
                },
            ),
            botany_v2_kind(
                BotanyPlantId::LingJingXu,
                LING_JING_XU,
                0.004,
                13_000,
                BotanyV2Spec {
                    survival_mode: SurvivalMode::PressureDifferential,
                    env_locks: ENV_LING_JING_XU,
                    harvest_hazards: HAZARD_LING_JING_XU,
                    base_mesh_ref: "twisting_vines",
                    tint_rgb: 0xA060FF,
                    tint_rgb_secondary: None,
                    model_overlay: ModelOverlay::Emissive,
                    icon_prompt: "purple crystal tendrils around ancient qi pillar, TSY crater herb icon",
                },
            ),
            botany_v2_kind(
                BotanyPlantId::MaoXinWei,
                MAO_XIN_WEI,
                0.002,
                9_000,
                BotanyV2Spec {
                    survival_mode: SurvivalMode::RuinResonance,
                    env_locks: ENV_MAO_XIN_WEI,
                    harvest_hazards: HAZARD_NONE,
                    base_mesh_ref: "wheat",
                    tint_rgb: 0xE8C040,
                    tint_rgb_secondary: None,
                    model_overlay: ModelOverlay::None,
                    icon_prompt: "warm yellow thatch-heart vetch, hermitage remnant herb icon",
                },
            ),
        ];

        Self {
            by_id: kinds.into_iter().map(|kind| (kind.id, kind)).collect(),
        }
    }
}

impl BotanyKindRegistry {
    pub fn get(&self, id: BotanyPlantId) -> Option<&BotanyPlantKind> {
        self.by_id.get(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &BotanyPlantKind> {
        self.by_id.values()
    }

    #[allow(dead_code)]
    pub fn canonicalize(&self, raw: &str) -> Result<BotanyPlantId, String> {
        canonicalize_herb_id(raw)
    }
}

fn botany_v2_kind(
    id: BotanyPlantId,
    item_id: &'static str,
    growth_cost: f32,
    max_age_ticks: u64,
    v2: BotanyV2Spec,
) -> BotanyPlantKind {
    BotanyPlantKind {
        id,
        item_id,
        zone_tags: &[],
        density_factor: 1.0,
        growth_cost,
        survive_threshold: -1.0,
        max_age_ticks,
        regen_ticks: 0,
        spawn_mode: BotanySpawnMode::ZoneRefresh,
        restore_ratio: 0.8,
        v2: Some(v2),
    }
}

impl BotanyPlantKind {
    pub fn is_v2(&self) -> bool {
        self.v2.is_some()
    }

    pub fn v2_spec(&self) -> Option<BotanyV2Spec> {
        self.v2
    }
}

#[allow(dead_code)]
pub fn canonical_herb_id(id: BotanyPlantId) -> &'static str {
    id.as_str()
}

pub fn canonicalize_herb_id(raw: &str) -> Result<BotanyPlantId, String> {
    let normalized = raw.trim().to_ascii_lowercase();
    if let Some(id) = BotanyPlantId::from_canonical(normalized.as_str()) {
        return Ok(id);
    }

    match normalized.as_str() {
        KAI_MAI_CAO_ALIAS => Ok(BotanyPlantId::NingMaiCao),
        XUE_CAO_ALIAS => Ok(BotanyPlantId::ChiSuiCao),
        BAI_CAO_ALIAS => Ok(BotanyPlantId::HuiYuanZhi),
        other => Err(format!("non-canonical herb id `{other}` is not allowed")),
    }
}

pub fn alias_of(raw: &str) -> Option<BotanyHerbAlias> {
    match raw.trim().to_ascii_lowercase().as_str() {
        KAI_MAI_CAO_ALIAS => Some(BotanyHerbAlias::KaiMai),
        XUE_CAO_ALIAS => Some(BotanyHerbAlias::Xue),
        BAI_CAO_ALIAS => Some(BotanyHerbAlias::Bai),
        _ => None,
    }
}

pub fn zone_supports(kind: &BotanyPlantKind, zone: &Zone) -> bool {
    kind.zone_tags
        .iter()
        .any(|tag| zone.supports_botany_tag(*tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_default_assets_with_test_trio() {
        let registry = load_plant_kind_registry().expect("default plants.toml should load");
        assert!(
            registry.get("ci_she_hao").is_some(),
            "ci_she_hao 是 §3.1 测试三作物之一"
        );
        assert!(registry.get("ning_mai_cao").is_some());
        assert!(registry.get("ling_mu_miao").is_some());
    }

    #[test]
    fn cultivable_filter_excludes_wild_only() {
        let registry = load_plant_kind_registry().unwrap();
        let cultivable: Vec<_> = registry.cultivable_ids().cloned().collect();
        assert!(cultivable.iter().any(|id| id == "ci_she_hao"));
        if let Some(plant) = registry.get("shi_mai_gen") {
            assert!(!plant.cultivable, "shi_mai_gen 必须 cultivable=false");
        }
    }

    #[test]
    fn canonical_registry_rejects_non_canonical_ids() {
        assert!(canonicalize_herb_id("shi_xin_hua").is_err());
    }

    #[test]
    fn canonical_registry_accepts_aliases_via_explicit_mapping() {
        assert_eq!(
            canonicalize_herb_id("kai_mai_cao").unwrap(),
            BotanyPlantId::NingMaiCao
        );
        assert_eq!(
            canonicalize_herb_id("xue_cao").unwrap(),
            BotanyPlantId::ChiSuiCao
        );
        assert_eq!(
            canonicalize_herb_id("bai_cao").unwrap(),
            BotanyPlantId::HuiYuanZhi
        );
    }

    #[test]
    fn canonical_registry_accepts_known_ids() {
        for id in [
            CI_SHE_HAO,
            NING_MAI_CAO,
            HUI_YUAN_ZHI,
            CHI_SUI_CAO,
            GU_YUAN_GEN,
            KONG_SHOU_HEN,
            JIE_GU_RUI,
            YANG_JING_TAI,
            QING_ZHUO_CAO,
            AN_SHEN_GUO,
            SHI_MAI_GEN,
            LING_YAN_SHI_ZHI,
            YE_KU_TENG,
            HUI_JIN_TAI,
            ZHEN_JIE_ZI,
            SHAO_HOU_MAN,
            TIAN_NU_JIAO,
            FU_YOU_HUA,
            WU_YAN_GUO,
            HEI_GU_JUN,
            FU_CHEN_CAO,
            ZHONG_YAN_TENG,
            FU_YUAN_JUE,
            BAI_YAN_PENG,
            DUAN_JI_CI,
            XUE_SE_MAI_CAO,
            YUN_DING_LAN,
            XUAN_GEN_WEI,
            YING_YUAN_GU,
            XUAN_RONG_TAI,
            YUAN_NI_HONG_YU,
            JING_XIN_ZAO,
            XUE_PO_LIAN,
            JIAO_MAI_TENG,
            LIE_YUAN_TAI,
            MING_GU_GU,
            BEI_WEN_ZHI,
            LING_JING_XU,
            MAO_XIN_WEI,
        ] {
            assert!(canonicalize_herb_id(id).is_ok(), "{id} should be canonical");
        }
    }

    #[test]
    fn plant_variant_accessors_are_consistent() {
        assert_eq!(PlantVariant::None.display_prefix(), None);
        assert_eq!(PlantVariant::Thunder.display_prefix(), Some("雷"));
        assert_eq!(PlantVariant::Tainted.display_prefix(), Some("黑"));

        assert!(PlantVariant::None.quality_modifier().abs() < f64::EPSILON);
        assert!(PlantVariant::Thunder.quality_modifier() > 0.0);
        assert!(PlantVariant::Tainted.quality_modifier() < 0.0);

        assert_eq!(PlantVariant::None.xp_delta(), 0);
        assert!(PlantVariant::Thunder.xp_delta() > 0);
        // Tainted 也给 +XP：采到稀有变种即奖励，品质差别通过 quality_modifier 单独体现
        assert!(PlantVariant::Tainted.xp_delta() > 0);
        assert!(PlantVariant::Thunder.xp_delta() > PlantVariant::Tainted.xp_delta());
    }

    #[test]
    fn default_registry_contains_22_v1_and_17_v2_canonical_kinds() {
        // plan-botany-v1 22 种 + plan-botany-v2 绝地草木拾遗 17 种。
        let registry = BotanyKindRegistry::default();
        let count = registry.iter().count();
        assert_eq!(
            count, 39,
            "BotanyKindRegistry should register exactly 39 canonical kinds, got {count}"
        );
        assert_eq!(registry.iter().filter(|kind| kind.is_v2()).count(), 17);
        assert!(registry.iter().filter(|kind| kind.is_v2()).all(|kind| {
            kind.zone_tags.is_empty() && kind.spawn_mode == BotanySpawnMode::ZoneRefresh
        }));
    }
}
