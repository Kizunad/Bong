//! 野生植物注册表：内容来自 shared/botany/plants.json，行为由野生生态系统执行。
use std::borrow::{Borrow, Cow};
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::Resource;

use super::catalog::{valid_id, PlantCatalog};
use crate::tools::ToolKind;
use crate::world::zone::{BotanyZoneTag, Zone};

pub const KAI_MAI_CAO_ALIAS: &str = "kai_mai_cao";
pub const XUE_CAO_ALIAS: &str = "xue_cao";
pub const BAI_CAO_ALIAS: &str = "bai_cao";

// 兼容现有行为代码的物品 ID。植物注册本身仍由 shared/botany/plants.json 驱动。
pub const SPIRIT_GRASS: &str = "spirit_grass";
pub const HUI_YUAN_ZHI: &str = "hui_yuan_zhi";

/// 开放的字符串 ID。借用形式用于已有专属行为的常量，配置加载得到 owned 字符串。
/// 常量仅供代码引用，既不是完整物种清单，也不参与注册或新物种准入。
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BotanyPlantId(Cow<'static, str>);

#[allow(non_upper_case_globals)]
impl BotanyPlantId {
    pub const SpiritGrass: Self = Self(Cow::Borrowed("spirit_grass"));
    pub const CiSheHao: Self = Self(Cow::Borrowed("ci_she_hao"));
    pub const NingMaiCao: Self = Self(Cow::Borrowed("ning_mai_cao"));
    pub const HuiYuanZhi: Self = Self(Cow::Borrowed("hui_yuan_zhi"));
    pub const ChiSuiCao: Self = Self(Cow::Borrowed("chi_sui_cao"));
    pub const GuYuanGen: Self = Self(Cow::Borrowed("gu_yuan_gen"));
    pub const KongShouHen: Self = Self(Cow::Borrowed("kong_shou_hen"));
    pub const FuYuanJue: Self = Self(Cow::Borrowed("fu_yuan_jue"));
    pub const BaiYanPeng: Self = Self(Cow::Borrowed("bai_yan_peng"));
    pub const DuanJiCi: Self = Self(Cow::Borrowed("duan_ji_ci"));
    pub const XueSeMaiCao: Self = Self(Cow::Borrowed("xue_se_mai_cao"));
    pub const XuanGenWei: Self = Self(Cow::Borrowed("xuan_gen_wei"));
    pub const XuanRongTai: Self = Self(Cow::Borrowed("xuan_rong_tai"));
    pub const XuePoLian: Self = Self(Cow::Borrowed("xue_po_lian"));
    pub const JiaoMaiTeng: Self = Self(Cow::Borrowed("jiao_mai_teng"));
    pub const LieYuanTai: Self = Self(Cow::Borrowed("lie_yuan_tai"));
    pub const LingJingXu: Self = Self(Cow::Borrowed("ling_jing_xu"));
    pub const ShiLingXian: Self = Self(Cow::Borrowed("shi_ling_xian"));

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_canonical(id: &str) -> Option<Self> {
        PlantCatalog::builtin()
            .get(id)
            .map(|plant| plant.id.clone())
    }
}

impl AsRef<str> for BotanyPlantId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for BotanyPlantId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotanyHerbAlias {
    KaiMai,
    Xue,
    Bai,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BotanySpawnMode {
    ZoneRefresh,
    StaticPoint,
    /// 兽死、残灰等事件触发，不参与区域刷新或静态点刷新。
    EventTriggered,
    /// 噬灵藓独立蔓延行为；新增该行为的植物仍需专属系统支持。
    SpreadByCrawl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlantVariant {
    #[default]
    None,
    Thunder,
    Tainted,
}

impl PlantVariant {
    pub fn display_prefix(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Thunder => Some("雷"),
            Self::Tainted => Some("黑"),
        }
    }

    pub fn quality_modifier(self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Thunder => 0.10,
            Self::Tainted => -0.15,
        }
    }

    pub fn xp_delta(self) -> i64 {
        match self {
            Self::None => 0,
            Self::Thunder => 2,
            Self::Tainted => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkyIsleSurface {
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaterPulsePhase {
    Open,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DecorationLock {
    One(String),
    Any(Vec<String>),
}

impl DecorationLock {
    pub fn names(&self) -> Vec<&str> {
        match self {
            Self::One(expected) => vec![expected.as_str()],
            Self::Any(expected) => expected.iter().map(String::as_str).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WoundLevel {
    Abrasion,
    Laceration,
    Fracture,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaunaKind {
    SpiritMice,
    MimicSpider,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum HarvestHazard {
    QiDrainOnApproach {
        radius_blocks: u8,
        drain_per_sec: f32,
    },
    WoundOnBareHand {
        wound: WoundLevel,
        required_tool: Option<ToolKind>,
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelOverlay {
    None,
    Emissive,
    DualPhase,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotanyV2Spec {
    pub survival_mode: SurvivalMode,
    pub env_locks: Vec<EnvLock>,
    pub harvest_hazards: Vec<HarvestHazard>,
    pub base_mesh_ref: String,
    pub tint_rgb: u32,
    pub tint_rgb_secondary: Option<u32>,
    pub model_overlay: ModelOverlay,
    pub icon_prompt: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotanyPlantKind {
    #[serde(skip)]
    pub id: BotanyPlantId,
    pub item_id: String,
    pub zone_tags: Vec<BotanyZoneTag>,
    pub density_factor: f32,
    pub growth_cost: f32,
    pub survive_threshold: f32,
    pub max_age_ticks: u64,
    pub regen_ticks: u64,
    pub spawn_mode: BotanySpawnMode,
    pub restore_ratio: f32,
    pub v2: Option<BotanyV2Spec>,
}

impl BotanyPlantKind {
    pub fn is_v2(&self) -> bool {
        self.v2.is_some()
    }

    pub fn v2_spec(&self) -> Option<&BotanyV2Spec> {
        self.v2.as_ref()
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        let unit = |value: f32| value.is_finite() && (0.0..=1.0).contains(&value);
        if !valid_id(&self.item_id)
            || self.max_age_ticks == 0
            || self.max_age_ticks > 9_007_199_254_740_991
            || self.regen_ticks > 9_007_199_254_740_991
            || !self.density_factor.is_finite()
            || self.density_factor < 0.0
            || !self.growth_cost.is_finite()
            || self.growth_cost < 0.0
            || !(-1.0..=1.0).contains(&self.survive_threshold)
            || !unit(self.restore_ratio)
        {
            return Err(
                "wild: invalid item_id, ticks, density, qi threshold or restore ratio".into(),
            );
        }
        let mut zone_tags = std::collections::HashSet::new();
        if self.zone_tags.iter().any(|tag| !zone_tags.insert(tag)) {
            return Err("wild.zone_tags: duplicate tag".into());
        }
        if let Some(spec) = &self.v2 {
            if !valid_id(&spec.base_mesh_ref)
                || spec.tint_rgb > 0xffffff
                || spec.tint_rgb_secondary.is_some_and(|rgb| rgb > 0xffffff)
            {
                return Err("wild.v2: invalid base_mesh_ref or tint".into());
            }
            for lock in &spec.env_locks {
                let valid = match lock {
                    EnvLock::NegPressure { min }
                    | EnvLock::QiVeinFlow { min }
                    | EnvLock::FractureMask { min }
                    | EnvLock::RuinDensity { min }
                    | EnvLock::SkyIslandMask { min, .. } => unit(*min),
                    EnvLock::AdjacentDecoration { kind, .. } => {
                        let names = kind.names();
                        !names.is_empty()
                            && names.iter().all(|name| valid_id(name))
                            && names.iter().collect::<std::collections::HashSet<_>>().len()
                                == names.len()
                    }
                    _ => true,
                };
                if !valid {
                    return Err("wild.v2.env_locks: invalid threshold or decoration".into());
                }
            }
            for hazard in &spec.harvest_hazards {
                let valid = match hazard {
                    HarvestHazard::QiDrainOnApproach { drain_per_sec, .. } => {
                        drain_per_sec.is_finite() && *drain_per_sec >= 0.0
                    }
                    HarvestHazard::DispersalOnFail { dispersal_chance } => unit(*dispersal_chance),
                    HarvestHazard::ResonanceVision { composure_loss, .. } => unit(*composure_loss),
                    HarvestHazard::AttractsMobs {
                        min_count,
                        max_count,
                        ..
                    } => *min_count >= 1 && max_count >= min_count,
                    _ => true,
                };
                if !valid {
                    return Err("wild.v2.harvest_hazards: invalid amount or count range".into());
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct BotanyKindRegistry {
    by_id: HashMap<BotanyPlantId, BotanyPlantKind>,
    aliases: HashMap<String, BotanyPlantId>,
}

impl Resource for BotanyKindRegistry {}

impl Default for BotanyKindRegistry {
    fn default() -> Self {
        Self::from_catalog(PlantCatalog::builtin())
    }
}

impl BotanyKindRegistry {
    pub fn from_catalog(catalog: &PlantCatalog) -> Self {
        let mut by_id = HashMap::new();
        let mut aliases = HashMap::new();
        for plant in catalog.iter() {
            if let Some(wild) = &plant.wild {
                by_id.insert(plant.id.clone(), wild.clone());
                for alias in &plant.aliases {
                    aliases.insert(alias.clone(), plant.id.clone());
                }
            }
        }
        Self { by_id, aliases }
    }

    pub fn get(&self, id: impl AsRef<str>) -> Option<&BotanyPlantKind> {
        self.by_id.get(id.as_ref())
    }

    pub fn iter(&self) -> impl Iterator<Item = &BotanyPlantKind> {
        self.by_id.values()
    }

    pub fn canonicalize(&self, raw: &str) -> Result<BotanyPlantId, String> {
        let normalized = raw.trim().to_ascii_lowercase();
        self.by_id
            .get(normalized.as_str())
            .map(|kind| kind.id.clone())
            .or_else(|| self.aliases.get(&normalized).cloned())
            .ok_or_else(|| format!("unknown plant id `{raw}`"))
    }

    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self {
            by_id: HashMap::new(),
            aliases: HashMap::new(),
        }
    }
}

pub fn canonicalize_herb_id(raw: &str) -> Result<BotanyPlantId, String> {
    PlantCatalog::builtin()
        .resolve(&raw.trim().to_ascii_lowercase())
        .map(|plant| plant.id.clone())
        .ok_or_else(|| format!("non-canonical herb id `{raw}` is not allowed"))
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
#[path = "catalog_tests.rs"]
mod tests;
