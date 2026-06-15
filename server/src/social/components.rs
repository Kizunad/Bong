use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Entity};

use crate::npc::faction::NamedFactionId;
use crate::schema::social::RenownTagV1;

pub type CharId = String;
pub type Tick = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianKind {
    Puppet,
    ZhenfaTrap,
    BondedDaoxiang,
}

impl GuardianKind {
    pub fn max_instances(self) -> usize {
        match self {
            GuardianKind::Puppet | GuardianKind::BondedDaoxiang => 1,
            GuardianKind::ZhenfaTrap => 5,
        }
    }

    pub fn default_charges(self) -> u8 {
        match self {
            GuardianKind::Puppet => 5,
            GuardianKind::ZhenfaTrap => 1,
            GuardianKind::BondedDaoxiang => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhenfaTrapTier {
    Basic,
    Middle,
    Advanced,
}

impl Default for ZhenfaTrapTier {
    fn default() -> Self {
        Self::Basic
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HouseGuardian {
    pub id: u64,
    pub kind: GuardianKind,
    pub charges_remaining: u8,
    pub decay_at: Tick,
    pub owner: CharId,
    pub pos: [i32; 3],
    #[serde(default)]
    pub authorized_chars: Vec<CharId>,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub trap_tier: ZhenfaTrapTier,
}

impl HouseGuardian {
    pub fn new(id: u64, kind: GuardianKind, owner: CharId, pos: [i32; 3], now_tick: Tick) -> Self {
        Self {
            id,
            kind,
            charges_remaining: kind.default_charges(),
            decay_at: now_tick.saturating_add(guardian_decay_ticks(kind)),
            owner,
            pos,
            authorized_chars: Vec::new(),
            active: true,
            trap_tier: ZhenfaTrapTier::default(),
        }
    }

    pub fn is_decayed(&self, now_tick: Tick) -> bool {
        !self.active || self.charges_remaining == 0 || now_tick >= self.decay_at
    }

    pub fn can_trigger_for(&self, char_id: &str, now_tick: Tick) -> bool {
        !self.is_decayed(now_tick)
            && self.owner != char_id
            && !self
                .authorized_chars
                .iter()
                .any(|authorized| authorized == char_id)
    }

    pub fn consume_charge(&mut self) -> bool {
        if self.charges_remaining == 0 {
            return false;
        }
        self.charges_remaining -= 1;
        if self.charges_remaining == 0 {
            self.active = false;
        }
        true
    }
}

pub fn guardian_decay_ticks(kind: GuardianKind) -> Tick {
    const TICKS_PER_HOUR: Tick = 20 * 60 * 60;
    match kind {
        GuardianKind::Puppet => 24 * TICKS_PER_HOUR,
        GuardianKind::ZhenfaTrap => 6 * TICKS_PER_HOUR,
        GuardianKind::BondedDaoxiang => 30 * 24 * TICKS_PER_HOUR,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntrusionRecord {
    pub intruder: Entity,
    pub intruder_char_id: CharId,
    pub owner: CharId,
    pub time: Tick,
    pub niche_pos: [i32; 3],
    #[serde(default)]
    pub items_taken: Vec<u64>,
    #[serde(default)]
    pub guardian_kinds_triggered: Vec<GuardianKind>,
}

#[derive(Debug, Clone, Default, Component, Serialize, Deserialize, PartialEq, Eq)]
pub struct Anonymity {
    pub displayed_name: Option<String>,
    #[serde(default)]
    pub exposed_to: HashSet<CharId>,
}

impl Anonymity {
    pub fn expose_to<I>(&mut self, witnesses: I) -> usize
    where
        I: IntoIterator<Item = CharId>,
    {
        let before = self.exposed_to.len();
        self.exposed_to.extend(witnesses);
        self.exposed_to.len().saturating_sub(before)
    }

    pub fn is_exposed_to(&self, witness: &str) -> bool {
        self.exposed_to.contains(witness)
    }
}

#[derive(Debug, Clone, Default, Component, Serialize, Deserialize, PartialEq)]
pub struct Renown {
    pub fame: i32,
    pub notoriety: i32,
    #[serde(default)]
    pub tags: Vec<RenownTagV1>,
}

impl Renown {
    pub fn apply_delta(&mut self, fame_delta: i32, notoriety_delta: i32, tags: Vec<RenownTagV1>) {
        self.fame = self.fame.saturating_add(fame_delta);
        self.notoriety = self.notoriety.saturating_add(notoriety_delta);
        for tag in tags {
            self.upsert_tag(tag);
        }
    }

    pub fn top_tags(&self, now_tick: Tick, limit: usize) -> Vec<RenownTagV1> {
        let mut tags = self.tags.clone();
        tags.sort_by(|left, right| {
            tag_visible_score(right, now_tick)
                .total_cmp(&tag_visible_score(left, now_tick))
                .then_with(|| left.tag.cmp(&right.tag))
        });
        tags.truncate(limit);
        tags
    }

    fn upsert_tag(&mut self, tag: RenownTagV1) {
        if let Some(existing) = self.tags.iter_mut().find(|entry| entry.tag == tag.tag) {
            // Keep a fresh positive report from being dampened by historical negative weight.
            existing.weight = (existing.weight + tag.weight).max(tag.weight);
            existing.last_seen_tick = existing.last_seen_tick.max(tag.last_seen_tick);
            existing.permanent |= tag.permanent;
            return;
        }
        self.tags.push(tag);
    }
}

fn tag_visible_score(tag: &RenownTagV1, now_tick: Tick) -> f64 {
    if tag.permanent {
        return tag.weight;
    }
    let age_hours = now_tick.saturating_sub(tag.last_seen_tick) as f64 / (20.0 * 60.0 * 60.0);
    tag.weight / (1.0 + age_hours / 24.0)
}

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq, Eq)]
pub struct Relationship {
    pub kind: crate::schema::social::RelationshipKindV1,
    pub peer: CharId,
    pub since_tick: Tick,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Default, Component, Serialize, Deserialize, PartialEq, Eq)]
pub struct Relationships {
    #[serde(default)]
    pub edges: Vec<Relationship>,
}

impl Relationships {
    pub fn upsert(&mut self, relationship: Relationship) {
        if let Some(existing) = self
            .edges
            .iter_mut()
            .find(|edge| edge.kind == relationship.kind && edge.peer == relationship.peer)
        {
            *existing = relationship;
            return;
        }
        self.edges.push(relationship);
    }
}

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpiritNiche {
    pub owner: CharId,
    pub pos: [i32; 3],
    pub placed_at_tick: Tick,
    pub revealed: bool,
    pub revealed_by: Option<CharId>,
    #[serde(default)]
    pub is_damaged: bool,
    #[serde(default)]
    pub guardians: Vec<HouseGuardian>,
}

#[derive(Debug, Clone, Default, Component, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExposureLog(pub Vec<ExposureEvent>);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExposureEvent {
    pub tick: Tick,
    pub kind: crate::schema::social::ExposureKindV1,
    pub witnesses: Vec<CharId>,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq, Eq)]
pub struct FactionMembership {
    pub faction: crate::npc::faction::FactionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub named_faction: Option<NamedFactionId>,
    pub rank: u8,
    pub loyalty: i32,
    #[serde(default)]
    pub betrayal_count: u8,
    #[serde(default)]
    pub invite_block_until_tick: Option<Tick>,
    #[serde(default)]
    pub permanently_refused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactionReputationTier {
    High,
    Medium,
    Normal,
    Low,
    Wanted,
}

#[derive(Debug, Clone, Default, Component, Serialize, Deserialize, PartialEq, Eq)]
pub struct FactionReputation {
    #[serde(default)]
    pub per_faction: HashMap<NamedFactionId, i32>,
}

impl FactionReputation {
    pub const MIN_SCORE: i32 = -100;
    pub const MAX_SCORE: i32 = 100;

    pub fn score(&self, faction: NamedFactionId) -> i32 {
        self.per_faction.get(&faction).copied().unwrap_or_default()
    }

    pub fn apply_delta(&mut self, faction: NamedFactionId, delta: i32) -> i32 {
        let next = self
            .score(faction)
            .saturating_add(delta)
            .clamp(Self::MIN_SCORE, Self::MAX_SCORE);
        self.per_faction.insert(faction, next);
        next
    }

    pub fn tier(&self, faction: NamedFactionId) -> FactionReputationTier {
        tier_for_score(self.score(faction))
    }

    pub fn tier_for_zone(&self, zone: &str) -> FactionReputationTier {
        faction_for_zone(zone)
            .map(|faction| self.tier(faction))
            .unwrap_or(FactionReputationTier::Normal)
    }
}

pub fn faction_for_zone(zone: &str) -> Option<NamedFactionId> {
    NamedFactionId::all()
        .into_iter()
        .find(|faction| faction.zone_anchor() == zone)
}

pub fn tier_for_score(score: i32) -> FactionReputationTier {
    if score > 50 {
        FactionReputationTier::High
    } else if score >= 10 {
        FactionReputationTier::Medium
    } else if score < -50 {
        FactionReputationTier::Wanted
    } else if score < -10 {
        FactionReputationTier::Low
    } else {
        FactionReputationTier::Normal
    }
}

#[derive(Debug, Clone, Component, PartialEq, Eq)]
pub struct SparringState {
    pub partner: Entity,
    pub invite_id: String,
    pub started_at_tick: Tick,
    pub expires_at_tick: Tick,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anonymity_exposure_is_append_only_for_unique_witnesses() {
        let mut anonymity = Anonymity::default();
        assert_eq!(
            anonymity.expose_to(["char:bob".to_string(), "char:bob".to_string()]),
            1
        );
        assert!(anonymity.is_exposed_to("char:bob"));
    }

    #[test]
    fn renown_top_tags_decay_but_keep_permanent_tags() {
        let mut renown = Renown::default();
        renown.apply_delta(
            0,
            0,
            vec![
                RenownTagV1 {
                    tag: "旧闻".to_string(),
                    weight: 100.0,
                    last_seen_tick: 0,
                    permanent: false,
                },
                RenownTagV1 {
                    tag: "三叛之人".to_string(),
                    weight: 20.0,
                    last_seen_tick: 1,
                    permanent: true,
                },
            ],
        );

        let top = renown.top_tags(20 * 60 * 60 * 100, 1);
        assert_eq!(top[0].tag, "三叛之人");
    }

    #[test]
    fn house_guardian_tracks_charges_and_owner_immunity() {
        let mut guardian = HouseGuardian::new(
            1,
            GuardianKind::Puppet,
            "char:owner".to_string(),
            [10, 64, 10],
            100,
        );
        assert!(!guardian.can_trigger_for("char:owner", 101));
        assert!(guardian.can_trigger_for("char:intruder", 101));
        for _ in 0..GuardianKind::Puppet.default_charges() {
            assert!(guardian.consume_charge());
        }
        assert!(guardian.is_decayed(101));
        assert!(!guardian.consume_charge());
    }

    #[test]
    fn faction_reputation_delta_clamps_to_plan_range() {
        let mut reputation = FactionReputation::default();
        let high_score = reputation.apply_delta(NamedFactionId::QingyunHunters, 250);
        assert_eq!(
            high_score,
            FactionReputation::MAX_SCORE,
            "expected max score because positive deltas clamp to plan range, actual {high_score}"
        );
        let low_score = reputation.apply_delta(NamedFactionId::QingyunHunters, -500);
        assert_eq!(
            low_score,
            FactionReputation::MIN_SCORE,
            "expected min score because negative deltas clamp to plan range, actual {low_score}"
        );
    }

    #[test]
    fn faction_reputation_tiers_follow_plan_thresholds() {
        for (score, expected) in [
            (51, FactionReputationTier::High),
            (50, FactionReputationTier::Medium),
            (10, FactionReputationTier::Medium),
            (0, FactionReputationTier::Normal),
            (-10, FactionReputationTier::Normal),
            (-11, FactionReputationTier::Low),
            (-50, FactionReputationTier::Low),
            (-51, FactionReputationTier::Wanted),
        ] {
            let actual = tier_for_score(score);
            assert_eq!(
                actual, expected,
                "expected {expected:?} because score {score} maps to plan P3 threshold, actual {actual:?}"
            );
        }
    }

    #[test]
    fn faction_for_zone_maps_named_anchors() {
        for (zone, expected) in [
            ("qingyun_peaks", Some(NamedFactionId::QingyunHunters)),
            ("blood_valley", Some(NamedFactionId::CangyuanMerchants)),
            ("north_wastes", Some(NamedFactionId::NorthWasteDrifters)),
            ("spawn", None),
        ] {
            let actual = faction_for_zone(zone);
            assert_eq!(
                actual, expected,
                "expected {expected:?} because zone {zone} has fixed named faction anchor, actual {actual:?}"
            );
        }
    }

    #[test]
    fn faction_reputation_tier_for_zone_uses_zone_anchor() {
        let mut reputation = FactionReputation::default();
        reputation.apply_delta(NamedFactionId::QingyunHunters, 60);
        reputation.apply_delta(NamedFactionId::CangyuanMerchants, -60);

        let qingyun = reputation.tier_for_zone("qingyun_peaks");
        assert_eq!(
            qingyun,
            FactionReputationTier::High,
            "expected High because QingyunHunters score is 60, actual {qingyun:?}"
        );
        let cangyuan = reputation.tier_for_zone("blood_valley");
        assert_eq!(
            cangyuan,
            FactionReputationTier::Wanted,
            "expected Wanted because CangyuanMerchants score is -60, actual {cangyuan:?}"
        );
        let spawn = reputation.tier_for_zone("spawn");
        assert_eq!(
            spawn,
            FactionReputationTier::Normal,
            "expected Normal because spawn has no named faction anchor, actual {spawn:?}"
        );
    }

    #[test]
    fn faction_membership_serde_keeps_named_faction_backward_compatible() {
        let legacy = serde_json::json!({
            "faction": "attack",
            "rank": 1,
            "loyalty": 12
        });
        let membership: FactionMembership =
            serde_json::from_value(legacy).expect("legacy membership should deserialize");
        assert_eq!(
            membership.named_faction, None,
            "expected None because old saves omit named_faction, actual {:?}",
            membership.named_faction
        );

        let json = serde_json::to_value(FactionMembership {
            faction: crate::npc::faction::FactionId::Attack,
            named_faction: None,
            rank: 1,
            loyalty: 12,
            betrayal_count: 0,
            invite_block_until_tick: None,
            permanently_refused: false,
        })
        .expect("membership should serialize");
        assert!(
            json.get("named_faction").is_none(),
            "expected named_faction omitted because None preserves old save shape, actual {json}"
        );
    }
}
