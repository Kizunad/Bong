use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Entity};

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
    pub rank: u8,
    pub loyalty: i32,
    #[serde(default)]
    pub betrayal_count: u8,
    #[serde(default)]
    pub invite_block_until_tick: Option<Tick>,
    #[serde(default)]
    pub permanently_refused: bool,
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
}
