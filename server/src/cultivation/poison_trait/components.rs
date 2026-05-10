use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

use crate::cultivation::components::Realm;

pub const POISON_DECAY_PER_HOUR_LIGHT: f32 = 1.0;
pub const POISON_DECAY_PER_HOUR_HEAVY: f32 = 0.5;
pub const DIGESTION_DECAY_PER_HOUR: f32 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoisonPillKind {
    WuSuiSanXin,
    ChiTuoZhiSui,
    QingLinManTuo,
    TieFuSheDan,
    FuXinXuanGui,
}

impl PoisonPillKind {
    pub const ALL: [Self; 5] = [
        Self::WuSuiSanXin,
        Self::ChiTuoZhiSui,
        Self::QingLinManTuo,
        Self::TieFuSheDan,
        Self::FuXinXuanGui,
    ];

    pub fn item_id(self) -> &'static str {
        match self {
            Self::WuSuiSanXin => "poison_pill_wu_sui_san_xin",
            Self::ChiTuoZhiSui => "poison_pill_chi_tuo_zhi_sui",
            Self::QingLinManTuo => "poison_pill_qing_lin_man_tuo",
            Self::TieFuSheDan => "poison_pill_tie_fu_she_dan",
            Self::FuXinXuanGui => "poison_pill_fu_xin_xuan_gui",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::WuSuiSanXin => "乌髓散心丹",
            Self::ChiTuoZhiSui => "赤陀蜘髓丹",
            Self::QingLinManTuo => "青鳞曼陀丹",
            Self::TieFuSheDan => "铁腹蛇胆丹",
            Self::FuXinXuanGui => "腐心玄龟丹",
        }
    }

    pub fn from_item_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|pill| pill.item_id() == id)
    }

    pub fn spec(self) -> PoisonPillSpec {
        match self {
            Self::WuSuiSanXin => PoisonPillSpec {
                kind: self,
                poison_amount: 5.0,
                digestion_load: 20.0,
                lifespan_years: 0.0,
                micro_tear_probability: 0.0,
                side_effect_tag: PoisonSideEffectTag::QiFocusDrift2h,
                bone_coin_price: 50,
            },
            Self::ChiTuoZhiSui => PoisonPillSpec {
                kind: self,
                poison_amount: 8.0,
                digestion_load: 25.0,
                lifespan_years: 1.0,
                micro_tear_probability: 0.0,
                side_effect_tag: PoisonSideEffectTag::RageBurst30m,
                bone_coin_price: 100,
            },
            Self::QingLinManTuo => PoisonPillSpec {
                kind: self,
                poison_amount: 10.0,
                digestion_load: 35.0,
                lifespan_years: 2.0,
                micro_tear_probability: 0.02,
                side_effect_tag: PoisonSideEffectTag::HallucinTint6h,
                bone_coin_price: 200,
            },
            Self::TieFuSheDan => PoisonPillSpec {
                kind: self,
                poison_amount: 12.0,
                digestion_load: 45.0,
                lifespan_years: 3.0,
                micro_tear_probability: 0.05,
                side_effect_tag: PoisonSideEffectTag::DigestLock6h,
                bone_coin_price: 350,
            },
            Self::FuXinXuanGui => PoisonPillSpec {
                kind: self,
                poison_amount: 15.0,
                digestion_load: 55.0,
                lifespan_years: 5.0,
                micro_tear_probability: 0.10,
                side_effect_tag: PoisonSideEffectTag::ToxicityTierUnlock,
                bone_coin_price: 800,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PoisonPillSpec {
    pub kind: PoisonPillKind,
    pub poison_amount: f32,
    pub digestion_load: f32,
    pub lifespan_years: f32,
    pub micro_tear_probability: f32,
    pub side_effect_tag: PoisonSideEffectTag,
    pub bone_coin_price: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoisonPowderKind {
    WuSuiSanXin,
    ChiTuoZhiSui,
    QingLinManTuo,
    TieFuSheDan,
    FuXinXuanGui,
}

impl PoisonPowderKind {
    pub const ALL: [Self; 5] = [
        Self::WuSuiSanXin,
        Self::ChiTuoZhiSui,
        Self::QingLinManTuo,
        Self::TieFuSheDan,
        Self::FuXinXuanGui,
    ];

    pub fn item_id(self) -> &'static str {
        match self {
            Self::WuSuiSanXin => "poison_powder_wu_sui_san_xin",
            Self::ChiTuoZhiSui => "poison_powder_chi_tuo_zhi_sui",
            Self::QingLinManTuo => "poison_powder_qing_lin_man_tuo",
            Self::TieFuSheDan => "poison_powder_tie_fu_she_dan",
            Self::FuXinXuanGui => "poison_powder_fu_xin_xuan_gui",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::WuSuiSanXin => "乌髓散心粉",
            Self::ChiTuoZhiSui => "赤陀蜘髓粉",
            Self::QingLinManTuo => "青鳞曼陀粉",
            Self::TieFuSheDan => "铁腹蛇胆粉",
            Self::FuXinXuanGui => "腐心玄龟粉",
        }
    }

    pub fn source_pill(self) -> PoisonPillKind {
        match self {
            Self::WuSuiSanXin => PoisonPillKind::WuSuiSanXin,
            Self::ChiTuoZhiSui => PoisonPillKind::ChiTuoZhiSui,
            Self::QingLinManTuo => PoisonPillKind::QingLinManTuo,
            Self::TieFuSheDan => PoisonPillKind::TieFuSheDan,
            Self::FuXinXuanGui => PoisonPillKind::FuXinXuanGui,
        }
    }

    pub fn spec(self) -> PoisonPowderSpec {
        match self {
            Self::WuSuiSanXin => PoisonPowderSpec {
                kind: self,
                damage_per_second: 2.0,
                duration_seconds: 3,
                victim_blur_ticks: 0,
                victim_micro_tear_probability: 0.0,
            },
            Self::ChiTuoZhiSui => PoisonPowderSpec {
                kind: self,
                damage_per_second: 2.0,
                duration_seconds: 5,
                victim_blur_ticks: 0,
                victim_micro_tear_probability: 0.0,
            },
            Self::QingLinManTuo => PoisonPowderSpec {
                kind: self,
                damage_per_second: 4.0,
                duration_seconds: 5,
                victim_blur_ticks: 20,
                victim_micro_tear_probability: 0.0,
            },
            Self::TieFuSheDan => PoisonPowderSpec {
                kind: self,
                damage_per_second: 5.0,
                duration_seconds: 6,
                victim_blur_ticks: 0,
                victim_micro_tear_probability: 0.0,
            },
            Self::FuXinXuanGui => PoisonPowderSpec {
                kind: self,
                damage_per_second: 8.0,
                duration_seconds: 8,
                victim_blur_ticks: 0,
                victim_micro_tear_probability: 0.05,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PoisonPowderSpec {
    pub kind: PoisonPowderKind,
    pub damage_per_second: f32,
    pub duration_seconds: u32,
    pub victim_blur_ticks: u64,
    pub victim_micro_tear_probability: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoisonSideEffectTag {
    #[serde(rename = "qi_focus_drift_2h")]
    QiFocusDrift2h,
    #[serde(rename = "rage_burst_30min")]
    RageBurst30m,
    #[serde(rename = "hallucin_tint_6h")]
    HallucinTint6h,
    #[serde(rename = "digest_lock_6h")]
    DigestLock6h,
    ToxicityTierUnlock,
}

impl PoisonSideEffectTag {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::QiFocusDrift2h => "qi_focus_drift_2h",
            Self::RageBurst30m => "rage_burst_30min",
            Self::HallucinTint6h => "hallucin_tint_6h",
            Self::DigestLock6h => "digest_lock_6h",
            Self::ToxicityTierUnlock => "toxicity_tier_unlock",
        }
    }

    pub fn duration_ticks(self) -> u64 {
        match self {
            Self::QiFocusDrift2h => 2 * 60 * 60 * 20,
            Self::RageBurst30m => 30 * 60 * 20,
            Self::HallucinTint6h => 6 * 60 * 60 * 20,
            Self::DigestLock6h => 6 * 60 * 60 * 20,
            Self::ToxicityTierUnlock => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoisonDoseRecord {
    pub tick: u64,
    pub dose_amount: f32,
    pub side_effect_tag: PoisonSideEffectTag,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct PoisonToxicity {
    pub level: f32,
    pub source_history: Vec<PoisonDoseRecord>,
    pub last_dose_tick: u64,
    #[serde(default)]
    pub last_decay_tick: u64,
    #[serde(default)]
    pub toxicity_tier_unlocked: bool,
}

impl Default for PoisonToxicity {
    fn default() -> Self {
        Self {
            level: 0.0,
            source_history: Vec::new(),
            last_dose_tick: 0,
            last_decay_tick: 0,
            toxicity_tier_unlocked: false,
        }
    }
}

impl PoisonToxicity {
    pub fn normalized(&self) -> Self {
        let mut next = self.clone();
        next.level = next.level.clamp(0.0, 100.0);
        next
    }
}

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct DigestionLoad {
    pub current: f32,
    pub capacity: f32,
    pub decay_rate: f32,
    #[serde(default)]
    pub digest_lock_until_tick: Option<u64>,
    #[serde(default)]
    pub last_decay_tick: u64,
}

impl Default for DigestionLoad {
    fn default() -> Self {
        Self {
            current: 0.0,
            capacity: 100.0,
            decay_rate: DIGESTION_DECAY_PER_HOUR,
            digest_lock_until_tick: None,
            last_decay_tick: 0,
        }
    }
}

impl DigestionLoad {
    pub fn for_realm(realm: Realm) -> Self {
        Self {
            capacity: digestion_capacity_for_realm(realm),
            ..Self::default()
        }
    }

    pub fn normalized(&self) -> Self {
        let mut next = self.clone();
        next.capacity = next.capacity.max(1.0);
        next.current = next.current.clamp(0.0, next.capacity);
        next.decay_rate = next.decay_rate.max(0.0);
        next
    }
}

pub fn digestion_capacity_for_realm(realm: Realm) -> f32 {
    let idx = match realm {
        Realm::Awaken => 0.0,
        Realm::Induce => 1.0,
        Realm::Condense => 2.0,
        Realm::Solidify => 3.0,
        Realm::Spirit => 4.0,
        Realm::Void => 5.0,
    };
    100.0 + idx * 20.0
}
