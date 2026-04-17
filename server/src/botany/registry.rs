use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use valence::prelude::Resource;

use crate::world::zone::{BotanyZoneTag, Zone};

pub const CI_SHE_HAO: &str = "ci_she_hao";
pub const NING_MAI_CAO: &str = "ning_mai_cao";
pub const HUI_YUAN_ZHI: &str = "hui_yuan_zhi";
pub const CHI_SUI_CAO: &str = "chi_sui_cao";
pub const GU_YUAN_GEN: &str = "gu_yuan_gen";

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
}

impl BotanyPlantId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CiSheHao => CI_SHE_HAO,
            Self::NingMaiCao => NING_MAI_CAO,
            Self::HuiYuanZhi => HUI_YUAN_ZHI,
            Self::ChiSuiCao => CHI_SUI_CAO,
            Self::GuYuanGen => GU_YUAN_GEN,
        }
    }

    pub fn from_canonical(id: &str) -> Option<Self> {
        match id {
            CI_SHE_HAO => Some(Self::CiSheHao),
            NING_MAI_CAO => Some(Self::NingMaiCao),
            HUI_YUAN_ZHI => Some(Self::HuiYuanZhi),
            CHI_SUI_CAO => Some(Self::ChiSuiCao),
            GU_YUAN_GEN => Some(Self::GuYuanGen),
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
            },
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
        ] {
            assert!(canonicalize_herb_id(id).is_ok(), "{id} should be canonical");
        }
    }
}
