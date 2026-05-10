use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

use crate::cultivation::components::Realm;
use crate::npc::faction::{FactionId, FactionRank};
use crate::npc::lifecycle::{NpcArchetype, NpcLifespan};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpcSkinTier {
    Commoner,
    RogueLow,
    RogueMid,
    RogueHigh,
    DiscipleLow,
    DiscipleMid,
    DiscipleHigh,
    Other,
}

impl NpcSkinTier {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Commoner => "commoner_hemp",
            Self::RogueLow => "rogue_low_gray_robe",
            Self::RogueMid => "rogue_mid_plain_robe",
            Self::RogueHigh => "rogue_high_dharma_robe",
            Self::DiscipleLow => "disciple_low_outer",
            Self::DiscipleMid => "disciple_mid_inner",
            Self::DiscipleHigh => "disciple_high_true",
            Self::Other => "npc_other_fallback",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpcAgeBand {
    Young,
    Adult,
    Elder,
    Fading,
}

impl NpcAgeBand {
    pub fn from_ratio(age_ratio: f64) -> Self {
        let ratio = if age_ratio.is_finite() {
            age_ratio.clamp(0.0, 16.0)
        } else {
            0.0
        };
        if ratio > 0.9 {
            Self::Fading
        } else if ratio > 0.7 {
            Self::Elder
        } else if ratio < 0.3 {
            Self::Young
        } else {
            Self::Adult
        }
    }

    pub const fn is_elderly(self) -> bool {
        matches!(self, Self::Elder | Self::Fading)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NpcSkinPoolKey(pub NpcSkinTier);

impl NpcSkinPoolKey {
    pub const PREFETCH_KEYS: [Self; 7] = [
        Self(NpcSkinTier::Commoner),
        Self(NpcSkinTier::RogueLow),
        Self(NpcSkinTier::RogueMid),
        Self(NpcSkinTier::RogueHigh),
        Self(NpcSkinTier::DiscipleLow),
        Self(NpcSkinTier::DiscipleMid),
        Self(NpcSkinTier::DiscipleHigh),
    ];

    pub const fn as_str(self) -> &'static str {
        self.0.as_str()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Component)]
pub struct NpcVisualProfile {
    pub archetype: NpcArchetype,
    pub skin_tier: NpcSkinTier,
    pub skin_pool_key: NpcSkinPoolKey,
    pub age_band: NpcAgeBand,
    pub faction_id: Option<FactionId>,
    pub faction_rank: Option<FactionRank>,
}

impl NpcVisualProfile {
    pub const fn skin_pool_key(self) -> NpcSkinPoolKey {
        self.skin_pool_key
    }

    pub const fn has_high_realm_aura(self) -> bool {
        matches!(
            self.skin_tier,
            NpcSkinTier::RogueHigh | NpcSkinTier::DiscipleHigh
        )
    }
}

pub fn select_npc_visual_profile(
    archetype: NpcArchetype,
    realm: Realm,
    faction_id: Option<FactionId>,
    faction_rank: Option<FactionRank>,
    age_ratio: f64,
) -> NpcVisualProfile {
    let skin_tier = match archetype {
        NpcArchetype::Commoner => NpcSkinTier::Commoner,
        NpcArchetype::Disciple => disciple_skin_tier(realm, faction_rank),
        NpcArchetype::Rogue => rogue_skin_tier(realm),
        _ => NpcSkinTier::Other,
    };

    NpcVisualProfile {
        archetype,
        skin_tier,
        skin_pool_key: NpcSkinPoolKey(skin_tier),
        age_band: NpcAgeBand::from_ratio(age_ratio),
        faction_id,
        faction_rank,
    }
}

pub fn select_profile_from_components(
    archetype: NpcArchetype,
    realm: Realm,
    faction: Option<&crate::npc::faction::FactionMembership>,
    lifespan: Option<&NpcLifespan>,
) -> NpcVisualProfile {
    select_npc_visual_profile(
        archetype,
        realm,
        faction.map(|membership| membership.faction_id),
        faction.map(|membership| membership.rank),
        lifespan.map(NpcLifespan::age_ratio).unwrap_or_default(),
    )
}

pub fn initial_age_ratio(archetype: NpcArchetype, initial_age_ticks: f64) -> f64 {
    let max_age_ticks = archetype.default_max_age_ticks();
    if max_age_ticks <= f64::EPSILON {
        1.0
    } else {
        (initial_age_ticks.max(0.0) / max_age_ticks).clamp(0.0, 16.0)
    }
}

fn rogue_skin_tier(realm: Realm) -> NpcSkinTier {
    match realm {
        Realm::Awaken | Realm::Induce => NpcSkinTier::RogueLow,
        Realm::Condense | Realm::Solidify => NpcSkinTier::RogueMid,
        Realm::Spirit | Realm::Void => NpcSkinTier::RogueHigh,
    }
}

fn disciple_skin_tier(realm: Realm, rank: Option<FactionRank>) -> NpcSkinTier {
    if matches!(rank, Some(FactionRank::Leader)) {
        return NpcSkinTier::DiscipleHigh;
    }
    match realm {
        Realm::Awaken | Realm::Induce => NpcSkinTier::DiscipleLow,
        Realm::Condense | Realm::Solidify => NpcSkinTier::DiscipleMid,
        Realm::Spirit | Realm::Void => NpcSkinTier::DiscipleHigh,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_returns_correct_pool() {
        let commoner =
            select_npc_visual_profile(NpcArchetype::Commoner, Realm::Void, None, None, 0.2);
        assert_eq!(commoner.skin_pool_key.as_str(), "commoner_hemp");

        let low = select_npc_visual_profile(NpcArchetype::Rogue, Realm::Induce, None, None, 0.5);
        assert_eq!(low.skin_tier, NpcSkinTier::RogueLow);

        let mid = select_npc_visual_profile(NpcArchetype::Rogue, Realm::Condense, None, None, 0.5);
        assert_eq!(mid.skin_pool_key.as_str(), "rogue_mid_plain_robe");

        let high = select_npc_visual_profile(
            NpcArchetype::Disciple,
            Realm::Induce,
            Some(FactionId::Attack),
            Some(FactionRank::Leader),
            0.5,
        );
        assert_eq!(high.skin_pool_key.as_str(), "disciple_high_true");
        assert_eq!(high.faction_rank, Some(FactionRank::Leader));
    }

    #[test]
    fn age_ratio_selects_elder_variant() {
        let young = NpcAgeBand::from_ratio(0.29);
        let adult = NpcAgeBand::from_ratio(0.7);
        let elder = NpcAgeBand::from_ratio(0.71);
        let fading = NpcAgeBand::from_ratio(0.91);

        assert_eq!(young, NpcAgeBand::Young);
        assert_eq!(adult, NpcAgeBand::Adult);
        assert_eq!(elder, NpcAgeBand::Elder);
        assert_eq!(fading, NpcAgeBand::Fading);
        assert!(elder.is_elderly());
        assert!(fading.is_elderly());
    }
}
