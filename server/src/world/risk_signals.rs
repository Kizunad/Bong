//! plan-sou-da-che-v1 P1 — 环境风险信号层。
//!
//! P0 的 `RiskHeatmap` 只给服务端内部风险分数；P1 在其上派生可被后续
//! botany/audio/VFX/NPC consumer 直接消费的「非数字」信号，避免客户端或
//! agent 各自重算风险口径。

use std::collections::HashMap;

use valence::prelude::{bevy_ecs, App, IntoSystemConfigs, Res, ResMut, Resource, Update};

use crate::cultivation::components::Realm;
use crate::world::risk_heatmap::{
    risk_score, update_risk_heatmap, RiskAxes, RiskHeatmap, RiskScore,
};
use crate::world::zone::ZoneRegistry;

/// 低压→高压的可读风险层级。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskSignalTier {
    Quiet,
    Watchful,
    Tense,
    Dangerous,
    Critical,
}

impl RiskSignalTier {
    pub fn from_score(score: RiskScore) -> Self {
        match score.0 {
            0..=19 => Self::Quiet,
            20..=39 => Self::Watchful,
            40..=59 => Self::Tense,
            60..=79 => Self::Dangerous,
            _ => Self::Critical,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quiet => "quiet",
            Self::Watchful => "watchful",
            Self::Tense => "tense",
            Self::Dangerous => "dangerous",
            Self::Critical => "critical",
        }
    }
}

/// 植被/地表风险信号；对应 plan P1「植被风险信号」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloraRiskSignal {
    DeadAsh,
    SparseGrey,
    GiftGrowth,
    LushBright,
    InvertedMoss,
}

impl FloraRiskSignal {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeadAsh => "dead_ash",
            Self::SparseGrey => "sparse_grey",
            Self::GiftGrowth => "gift_growth",
            Self::LushBright => "lush_bright",
            Self::InvertedMoss => "inverted_moss",
        }
    }
}

/// 音频风险信号；recipe id 均来自 `server/assets/audio/recipes/*.json`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioRiskSignal {
    SafeWindAndBirds,
    MutedWilderness,
    DistantBeastLowGrowl,
    RatScreechAndBranches,
    TiandaoLowWhisper,
    NegativePressureHum,
}

impl AudioRiskSignal {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SafeWindAndBirds => "safe_wind_and_birds",
            Self::MutedWilderness => "muted_wilderness",
            Self::DistantBeastLowGrowl => "distant_beast_low_growl",
            Self::RatScreechAndBranches => "rat_screech_and_branches",
            Self::TiandaoLowWhisper => "tiandao_low_whisper",
            Self::NegativePressureHum => "negative_pressure_hum",
        }
    }

    pub fn recipe_id(self) -> &'static str {
        match self {
            Self::SafeWindAndBirds => "ambient_spawn_plain",
            Self::MutedWilderness => "ambient_wilderness",
            Self::DistantBeastLowGrowl => "fauna_hybrid_beast_ambient",
            Self::RatScreechAndBranches => "fauna_rat_squeal",
            Self::TiandaoLowWhisper => "tiandao_watch_ambient",
            Self::NegativePressureHum => "fauna_fuya_pressure_hum",
        }
    }
}

/// 粒子/VFX 风险信号；本层只声明事件意图，不直接发包。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleRiskSignal {
    None,
    PseudoVeinGlint,
    DarkSpores,
    PaleAfterimage,
    TideColorShift,
    NegativeQiDrift,
}

impl ParticleRiskSignal {
    pub fn event_id(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::PseudoVeinGlint => Some("bong:risk_pseudo_vein_glint"),
            Self::DarkSpores => Some("bong:risk_dark_spores"),
            Self::PaleAfterimage => Some("bong:risk_pale_afterimage"),
            Self::TideColorShift => Some("bong:risk_tide_color_shift"),
            Self::NegativeQiDrift => Some("bong:risk_negative_qi_drift"),
        }
    }
}

/// NPC 行为风险信号；供后续 NPC 行为树用同一口径表现「这里不对劲」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcBehaviorRiskSignal {
    Relaxed,
    CrouchNearCover,
    FaunaFleeLine,
    RelicTremor,
    AllFleeToSafety,
}

impl NpcBehaviorRiskSignal {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Relaxed => "relaxed",
            Self::CrouchNearCover => "crouch_near_cover",
            Self::FaunaFleeLine => "fauna_flee_line",
            Self::RelicTremor => "relic_tremor",
            Self::AllFleeToSafety => "all_flee_to_safety",
        }
    }
}

/// 单个 zone 的环境风险信号快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskSignalProfile {
    pub tier: RiskSignalTier,
    pub flora: FloraRiskSignal,
    pub audio: AudioRiskSignal,
    pub particle: ParticleRiskSignal,
    pub npc_behavior: NpcBehaviorRiskSignal,
}

impl RiskSignalProfile {
    pub fn summary_tokens(&self) -> String {
        let particle = self.particle.event_id().unwrap_or("none");
        format!(
            "tier={} flora={} audio={} audio_recipe={} particle={} npc={}",
            self.tier.as_str(),
            self.flora.as_str(),
            self.audio.as_str(),
            self.audio.recipe_id(),
            particle,
            self.npc_behavior.as_str()
        )
    }
}

/// 所有 zone 的最新环境风险信号。
#[derive(Debug, Default, Resource)]
pub struct RiskSignalMap {
    pub by_zone: HashMap<String, RiskSignalProfile>,
}

impl RiskSignalMap {
    pub fn profile_for_zone(&self, zone_name: &str) -> Option<&RiskSignalProfile> {
        self.by_zone.get(zone_name)
    }
}

pub fn flora_signal_for_qi(spirit_qi: f64) -> FloraRiskSignal {
    if spirit_qi < 0.0 {
        FloraRiskSignal::InvertedMoss
    } else if spirit_qi <= 0.1 {
        FloraRiskSignal::DeadAsh
    } else if spirit_qi < 0.3 {
        FloraRiskSignal::SparseGrey
    } else if spirit_qi <= 0.5 {
        FloraRiskSignal::GiftGrowth
    } else {
        FloraRiskSignal::LushBright
    }
}

pub fn audio_signal_for(spirit_qi: f64, axes: RiskAxes, tier: RiskSignalTier) -> AudioRiskSignal {
    if spirit_qi < 0.0 {
        return AudioRiskSignal::NegativePressureHum;
    }
    if axes.fauna >= 20.0 {
        return AudioRiskSignal::RatScreechAndBranches;
    }
    if axes.npc >= 16.0 || matches!(tier, RiskSignalTier::Critical) {
        return AudioRiskSignal::TiandaoLowWhisper;
    }
    if matches!(tier, RiskSignalTier::Dangerous) {
        return AudioRiskSignal::DistantBeastLowGrowl;
    }
    if matches!(tier, RiskSignalTier::Quiet) {
        AudioRiskSignal::SafeWindAndBirds
    } else {
        AudioRiskSignal::MutedWilderness
    }
}

pub fn particle_signal_for(
    spirit_qi: f64,
    axes: RiskAxes,
    tier: RiskSignalTier,
) -> ParticleRiskSignal {
    if spirit_qi < 0.0 {
        return ParticleRiskSignal::NegativeQiDrift;
    }
    if axes.fauna >= 20.0 {
        return ParticleRiskSignal::DarkSpores;
    }
    if axes.npc >= 16.0 {
        return ParticleRiskSignal::PaleAfterimage;
    }
    if spirit_qi >= 0.6 {
        return ParticleRiskSignal::PseudoVeinGlint;
    }
    if matches!(tier, RiskSignalTier::Dangerous | RiskSignalTier::Critical) {
        ParticleRiskSignal::TideColorShift
    } else {
        ParticleRiskSignal::None
    }
}

pub fn npc_behavior_signal_for(axes: RiskAxes, tier: RiskSignalTier) -> NpcBehaviorRiskSignal {
    if matches!(tier, RiskSignalTier::Critical) {
        return NpcBehaviorRiskSignal::AllFleeToSafety;
    }
    if axes.fauna >= 20.0 {
        return NpcBehaviorRiskSignal::FaunaFleeLine;
    }
    if axes.npc >= 16.0 {
        return NpcBehaviorRiskSignal::RelicTremor;
    }
    if matches!(tier, RiskSignalTier::Tense | RiskSignalTier::Dangerous) {
        NpcBehaviorRiskSignal::CrouchNearCover
    } else {
        NpcBehaviorRiskSignal::Relaxed
    }
}

pub fn risk_signal_profile(
    spirit_qi: f64,
    axes: RiskAxes,
    player_realm: Realm,
) -> RiskSignalProfile {
    let tier = RiskSignalTier::from_score(risk_score(axes, player_realm));
    RiskSignalProfile {
        tier,
        flora: flora_signal_for_qi(spirit_qi),
        audio: audio_signal_for(spirit_qi, axes, tier),
        particle: particle_signal_for(spirit_qi, axes, tier),
        npc_behavior: npc_behavior_signal_for(axes, tier),
    }
}

pub fn update_risk_signals(
    mut signal_map: ResMut<RiskSignalMap>,
    zones: Option<Res<ZoneRegistry>>,
    heatmap: Option<Res<RiskHeatmap>>,
) {
    signal_map.by_zone.clear();

    let (Some(zones), Some(heatmap)) = (zones, heatmap) else {
        return;
    };

    for zone in &zones.zones {
        let axes = heatmap.axes_for_zone(&zone.name);
        let profile = risk_signal_profile(zone.spirit_qi, axes, Realm::Condense);
        signal_map.by_zone.insert(zone.name.clone(), profile);
    }
}

pub fn register(app: &mut App) {
    app.insert_resource(RiskSignalMap::default())
        .add_systems(Update, update_risk_signals.after(update_risk_heatmap));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::SoundRecipeRegistry;

    #[test]
    fn flora_signal_by_qi_covers_dead_sparse_gift_lush_and_negative() {
        assert_eq!(flora_signal_for_qi(-0.1), FloraRiskSignal::InvertedMoss);
        assert_eq!(flora_signal_for_qi(0.0), FloraRiskSignal::DeadAsh);
        assert_eq!(flora_signal_for_qi(0.2), FloraRiskSignal::SparseGrey);
        assert_eq!(flora_signal_for_qi(0.4), FloraRiskSignal::GiftGrowth);
        assert_eq!(flora_signal_for_qi(0.6), FloraRiskSignal::LushBright);
    }

    #[test]
    fn audio_signal_prefers_negative_pressure_over_other_axes() {
        let axes = RiskAxes {
            qi: 30.0,
            fauna: 30.0,
            npc: 25.0,
            player: 15.0,
        };
        assert_eq!(
            audio_signal_for(-0.2, axes, RiskSignalTier::Critical),
            AudioRiskSignal::NegativePressureHum,
            "负灵域边缘音频必须优先表达真元流失压力，而不是普通兽群或天道低语"
        );
    }

    #[test]
    fn fauna_pressure_maps_to_dark_spores_and_flee_line() {
        let axes = RiskAxes {
            qi: 10.0,
            fauna: 25.0,
            npc: 0.0,
            player: 0.0,
        };
        assert_eq!(
            particle_signal_for(0.4, axes, RiskSignalTier::Tense),
            ParticleRiskSignal::DarkSpores
        );
        assert_eq!(
            npc_behavior_signal_for(axes, RiskSignalTier::Tense),
            NpcBehaviorRiskSignal::FaunaFleeLine
        );
    }

    #[test]
    fn high_npc_pressure_maps_to_afterimage_and_relic_tremor() {
        let axes = RiskAxes {
            qi: 10.0,
            fauna: 0.0,
            npc: 20.0,
            player: 0.0,
        };
        assert_eq!(
            particle_signal_for(0.4, axes, RiskSignalTier::Tense),
            ParticleRiskSignal::PaleAfterimage
        );
        assert_eq!(
            npc_behavior_signal_for(axes, RiskSignalTier::Tense),
            NpcBehaviorRiskSignal::RelicTremor
        );
    }

    #[test]
    fn profile_summary_uses_signals_not_numeric_panel_values() {
        let profile = RiskSignalProfile {
            tier: RiskSignalTier::Dangerous,
            flora: FloraRiskSignal::LushBright,
            audio: AudioRiskSignal::DistantBeastLowGrowl,
            particle: ParticleRiskSignal::DarkSpores,
            npc_behavior: NpcBehaviorRiskSignal::FaunaFleeLine,
        };
        let summary = profile.summary_tokens();
        assert!(summary.contains("flora=lush_bright"));
        assert!(summary.contains("audio=distant_beast_low_growl"));
        assert!(summary.contains("particle=bong:risk_dark_spores"));
        assert!(summary.contains("npc=fauna_flee_line"));
        assert!(
            !summary.contains("RiskScore="),
            "P1 环境信号层不能退化成数字面板，summary={summary}"
        );
    }

    #[test]
    fn all_audio_signal_recipe_ids_exist_in_registry() {
        let registry =
            SoundRecipeRegistry::load_default().expect("default audio recipes should load");
        for signal in [
            AudioRiskSignal::SafeWindAndBirds,
            AudioRiskSignal::MutedWilderness,
            AudioRiskSignal::DistantBeastLowGrowl,
            AudioRiskSignal::RatScreechAndBranches,
            AudioRiskSignal::TiandaoLowWhisper,
            AudioRiskSignal::NegativePressureHum,
        ] {
            assert!(
                registry.get(signal.recipe_id()).is_some(),
                "AudioRiskSignal::{signal:?} recipe `{}` must exist",
                signal.recipe_id()
            );
        }
    }
}
