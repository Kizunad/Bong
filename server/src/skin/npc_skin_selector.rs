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
            age_ratio.clamp(0.0, 1.0)
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
    #[serde(default)]
    pub high_realm: bool,
    pub faction_id: Option<FactionId>,
    pub faction_rank: Option<FactionRank>,
}

impl NpcVisualProfile {
    pub const fn skin_pool_key(self) -> NpcSkinPoolKey {
        self.skin_pool_key
    }

    pub const fn has_high_realm_aura(self) -> bool {
        self.high_realm
    }
}

pub fn select_npc_visual_profile(
    archetype: NpcArchetype,
    realm: Realm,
    faction_id: Option<FactionId>,
    faction_rank: Option<FactionRank>,
    age_ratio: f64,
) -> NpcVisualProfile {
    let high_realm = is_high_realm(realm);
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
        high_realm,
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
        (initial_age_ticks.max(0.0) / max_age_ticks).clamp(0.0, 1.0)
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

const fn is_high_realm(realm: Realm) -> bool {
    matches!(realm, Realm::Spirit | Realm::Void)
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
        assert!(!high.has_high_realm_aura());
        assert_eq!(high.faction_rank, Some(FactionRank::Leader));
    }

    #[test]
    fn high_realm_aura_uses_realm_not_rank_skin_tier() {
        let low_realm_leader = select_npc_visual_profile(
            NpcArchetype::Disciple,
            Realm::Induce,
            Some(FactionId::Attack),
            Some(FactionRank::Leader),
            0.5,
        );
        assert_eq!(low_realm_leader.skin_tier, NpcSkinTier::DiscipleHigh);
        assert!(!low_realm_leader.has_high_realm_aura());

        let high_realm_disciple = select_npc_visual_profile(
            NpcArchetype::Disciple,
            Realm::Spirit,
            Some(FactionId::Attack),
            Some(FactionRank::Disciple),
            0.5,
        );
        assert!(high_realm_disciple.has_high_realm_aura());
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

    #[test]
    fn initial_age_ratio_is_normalized() {
        let max_age_ticks = NpcArchetype::Rogue.default_max_age_ticks();

        assert_eq!(initial_age_ratio(NpcArchetype::Rogue, -1.0), 0.0);
        assert_eq!(
            initial_age_ratio(NpcArchetype::Rogue, max_age_ticks * 2.0),
            1.0
        );
    }

    // === plan-npc-realm-distribution-v1 P2：realm→视觉档位映射穷举 pin ===
    //
    // 此前 `selector_returns_correct_pool` / `high_realm_aura_uses_realm_not_rank_skin_tier`
    // 只覆盖了 6 境界中的 3-4 个取样点。P0 修复「realm 被吞成醒灵」之后，视觉档位
    // 才第一次吃到真实境界；本测试穷举全部 6 境界 × {Rogue, Disciple(无 Leader rank)}，
    // 锁死 realm→tier 映射不被日后重构悄悄改动边界（Awaken/Induce→Low，
    // Condense/Solidify→Mid，Spirit/Void→High）。

    #[test]
    fn rogue_skin_tier_all_six_realms_pinned() {
        let expected = [
            (Realm::Awaken, NpcSkinTier::RogueLow),
            (Realm::Induce, NpcSkinTier::RogueLow),
            (Realm::Condense, NpcSkinTier::RogueMid),
            (Realm::Solidify, NpcSkinTier::RogueMid),
            (Realm::Spirit, NpcSkinTier::RogueHigh),
            (Realm::Void, NpcSkinTier::RogueHigh),
        ];
        for (realm, tier) in expected {
            let profile = select_npc_visual_profile(NpcArchetype::Rogue, realm, None, None, 0.5);
            assert_eq!(
                profile.skin_tier, tier,
                "Rogue realm={realm:?} 期望 skin_tier={tier:?}，实得 {:?}",
                profile.skin_tier
            );
        }
    }

    #[test]
    fn disciple_skin_tier_all_six_realms_pinned_without_leader_rank() {
        // rank=None（非 Leader）时，disciple_skin_tier 完全由 realm 决定；
        // Leader rank 会短路成 DiscipleHigh，另有专属测试覆盖（selector_returns_correct_pool）。
        let expected = [
            (Realm::Awaken, NpcSkinTier::DiscipleLow),
            (Realm::Induce, NpcSkinTier::DiscipleLow),
            (Realm::Condense, NpcSkinTier::DiscipleMid),
            (Realm::Solidify, NpcSkinTier::DiscipleMid),
            (Realm::Spirit, NpcSkinTier::DiscipleHigh),
            (Realm::Void, NpcSkinTier::DiscipleHigh),
        ];
        for (realm, tier) in expected {
            let profile = select_npc_visual_profile(NpcArchetype::Disciple, realm, None, None, 0.5);
            assert_eq!(
                profile.skin_tier, tier,
                "Disciple(no rank) realm={realm:?} 期望 skin_tier={tier:?}，实得 {:?}",
                profile.skin_tier
            );
        }
    }

    #[test]
    fn high_realm_aura_all_six_realms_pinned() {
        // is_high_realm 边界：Spirit/Void → true，其余四档 → false。
        let expected = [
            (Realm::Awaken, false),
            (Realm::Induce, false),
            (Realm::Condense, false),
            (Realm::Solidify, false),
            (Realm::Spirit, true),
            (Realm::Void, true),
        ];
        for (realm, has_aura) in expected {
            let profile = select_npc_visual_profile(NpcArchetype::Rogue, realm, None, None, 0.5);
            assert_eq!(
                profile.has_high_realm_aura(),
                has_aura,
                "realm={realm:?} 期望 has_high_realm_aura={has_aura}，实得 {}",
                profile.has_high_realm_aura()
            );
        }
    }

    #[test]
    fn commoner_and_other_archetypes_ignore_realm_but_still_carry_realm_derived_aura() {
        // Commoner 恒 skin_tier=Commoner（不随 realm 变化），但 high_realm aura 字段
        // 仍必须忠实反映传入 realm——防止「意图 realm≠视觉判定 realm」的另一种双源
        // （skin_tier 忽略 realm 是设计使然，aura 忽略 realm 则是 bug）。
        let low = select_npc_visual_profile(NpcArchetype::Commoner, Realm::Awaken, None, None, 0.5);
        let high = select_npc_visual_profile(NpcArchetype::Commoner, Realm::Void, None, None, 0.5);
        assert_eq!(low.skin_tier, NpcSkinTier::Commoner);
        assert_eq!(high.skin_tier, NpcSkinTier::Commoner);
        assert!(!low.has_high_realm_aura());
        assert!(high.has_high_realm_aura());
    }
}
