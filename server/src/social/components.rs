use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

use crate::schema::social::RenownTagV1;

pub type CharId = String;
pub type Tick = u64;
pub type DefenseModeId = String;

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
    pub defense_mode: Option<DefenseModeId>,
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
}
