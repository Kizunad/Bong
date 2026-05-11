use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CalamityKindV1 {
    Thunder,
    PoisonMiasma,
    MeridianSeal,
    DaoxiangWave,
    HeavenlyFire,
    PressureInvert,
    AllWither,
    RealmCollapse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalamityIntentV1 {
    pub v: u8,
    pub calamity: Option<CalamityKindV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_zone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_player: Option<String>,
    pub intensity: f64,
    pub reason: String,
}
