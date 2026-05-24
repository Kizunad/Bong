//! NPC interaction memory state used by world-space memory bubbles.

use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, App, Client, Commands, Component, Entity, EventReader, Query, Update, With, Without,
};

use crate::combat::components::Lifecycle;
use crate::combat::events::CombatEvent;
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::spawn::NpcMarker;

pub const MAX_NPC_MEMORY_ENTRIES: usize = 16;

#[derive(Component, Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpcMemoryComponent {
    pub interactions: Vec<NpcMemoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpcMemoryEntry {
    pub player_uuid: String,
    pub interaction_type: NpcInteractionType,
    pub timestamp: u64,
    pub outcome: NpcInteractionOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpcInteractionType {
    Trade,
    Attack,
    Theft,
    Help,
    TradeRefused,
    Ambushed,
    FledFrom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpcInteractionOutcome {
    Friendly,
    Deceived,
    Harmed,
    Helped,
}

/// Returns true if the interaction type is pinned (resistant to eviction).
///
/// Pinned types: Attack, Theft, Ambushed — these are dangerous encounters
/// that the NPC should remember for a long time.
pub fn is_pinned(interaction_type: NpcInteractionType) -> bool {
    matches!(
        interaction_type,
        NpcInteractionType::Attack | NpcInteractionType::Theft | NpcInteractionType::Ambushed
    )
}

impl NpcMemoryComponent {
    pub fn remember(&mut self, entry: NpcMemoryEntry) {
        self.interactions.push(entry);
        self.trim_to_limit();
    }

    /// Weighted eviction: pinned entries (Attack/Theft/Ambushed) resist eviction.
    /// Unpinned entries are evicted oldest-first. If all slots are pinned,
    /// the oldest pinned entry is downgraded (effectively evicted).
    fn trim_to_limit(&mut self) {
        while self.interactions.len() > MAX_NPC_MEMORY_ENTRIES {
            // Find oldest unpinned entry
            if let Some(idx) = self
                .interactions
                .iter()
                .position(|e| !is_pinned(e.interaction_type))
            {
                self.interactions.remove(idx);
            } else {
                // All pinned: evict the oldest (first) entry
                self.interactions.remove(0);
            }
        }
    }

    /// Check if this NPC has been attacked by a specific player.
    #[allow(dead_code)]
    pub fn has_been_attacked_by(&self, player_uuid: &str) -> bool {
        self.interactions.iter().any(|e| {
            e.player_uuid == player_uuid && e.interaction_type == NpcInteractionType::Attack
        })
    }

    /// Check if this NPC has successfully traded with a specific player.
    #[allow(dead_code)]
    pub fn has_traded_with(&self, player_uuid: &str) -> bool {
        self.interactions.iter().any(|e| {
            e.player_uuid == player_uuid && e.interaction_type == NpcInteractionType::Trade
        })
    }

    /// Check if this NPC has been robbed by a specific player.
    #[allow(dead_code)]
    pub fn has_been_robbed_by(&self, player_uuid: &str) -> bool {
        self.interactions.iter().any(|e| {
            e.player_uuid == player_uuid && e.interaction_type == NpcInteractionType::Theft
        })
    }
}

pub fn register(app: &mut App) {
    app.add_systems(
        Update,
        (
            attach_npc_memory_components,
            record_attack_memories,
            trim_npc_memory_components,
        ),
    );
}

fn attach_npc_memory_components(
    mut commands: Commands,
    npcs: Query<Entity, (With<NpcMarker>, Without<NpcMemoryComponent>)>,
) {
    for npc in &npcs {
        commands.entity(npc).insert(NpcMemoryComponent::default());
    }
}

fn trim_npc_memory_components(mut memories: Query<&mut NpcMemoryComponent>) {
    for mut memory in &mut memories {
        memory.trim_to_limit();
    }
}

fn record_attack_memories(
    mut events: EventReader<CombatEvent>,
    mut memories: Query<&mut NpcMemoryComponent, With<NpcMarker>>,
    lifecycles: Query<&Lifecycle>,
    clients: Query<(), With<Client>>,
) {
    for event in events.read() {
        if event.damage <= 0.0 || clients.get(event.attacker).is_err() {
            continue;
        }
        record_player_npc_interaction(
            &mut memories,
            &lifecycles,
            event.target,
            event.attacker,
            NpcInteractionType::Attack,
            NpcInteractionOutcome::Harmed,
            event.resolved_at_tick,
        );
    }
}

pub fn record_player_npc_interaction(
    memories: &mut Query<&mut NpcMemoryComponent, With<NpcMarker>>,
    lifecycles: &Query<&Lifecycle>,
    npc: Entity,
    player: Entity,
    interaction_type: NpcInteractionType,
    outcome: NpcInteractionOutcome,
    timestamp: u64,
) {
    let Ok(mut memory) = memories.get_mut(npc) else {
        return;
    };
    let player_uuid = lifecycles
        .get(player)
        .map(|lifecycle| lifecycle.character_id.clone())
        .unwrap_or_else(|_| format!("entity:{}", player.to_bits()));
    memory.remember(NpcMemoryEntry {
        player_uuid,
        interaction_type,
        timestamp,
        outcome,
    });
}

pub fn memory_bubble_text(archetype: NpcArchetype, entry: &NpcMemoryEntry) -> &'static str {
    match (archetype, entry.interaction_type, entry.outcome) {
        (NpcArchetype::Rogue, NpcInteractionType::Trade, NpcInteractionOutcome::Deceived) => {
            "...你。上次的骨币，成色不对。"
        }
        (NpcArchetype::Rogue, NpcInteractionType::Trade, NpcInteractionOutcome::Friendly) => {
            "道友，还有灵草出让吗？"
        }
        (NpcArchetype::Commoner, NpcInteractionType::Attack, _) => "大仙饶命！小人再不敢了...",
        (NpcArchetype::GuardianRelic, _, _) => "...又来？",
        (_, NpcInteractionType::Help, _) => "上次的人情，我记着。",
        _ => "...",
    }
}

pub fn should_emit_memory_bubble(
    npc_id: &str,
    player_uuid: &str,
    interaction_count: usize,
) -> bool {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    npc_id.hash(&mut hasher);
    player_uuid.hash(&mut hasher);
    interaction_count.hash(&mut hasher);
    hasher.finish() % 2 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(i: u64) -> NpcMemoryEntry {
        NpcMemoryEntry {
            player_uuid: "offline:Azure".to_string(),
            interaction_type: if i % 2 == 0 {
                NpcInteractionType::Trade
            } else {
                NpcInteractionType::Attack
            },
            timestamp: i,
            outcome: NpcInteractionOutcome::Friendly,
        }
    }

    fn entry_with_type(i: u64, ty: NpcInteractionType) -> NpcMemoryEntry {
        NpcMemoryEntry {
            player_uuid: "offline:Azure".to_string(),
            interaction_type: ty,
            timestamp: i,
            outcome: NpcInteractionOutcome::Friendly,
        }
    }

    fn entry_for_player(i: u64, player: &str, ty: NpcInteractionType) -> NpcMemoryEntry {
        NpcMemoryEntry {
            player_uuid: player.to_string(),
            interaction_type: ty,
            timestamp: i,
            outcome: NpcInteractionOutcome::Friendly,
        }
    }

    // -----------------------------------------------------------------------
    // Slot expansion
    // -----------------------------------------------------------------------

    #[test]
    fn slot_expansion_16() {
        assert_eq!(
            MAX_NPC_MEMORY_ENTRIES, 16,
            "MAX_NPC_MEMORY_ENTRIES should be 16 (expanded from 8)"
        );
    }

    #[test]
    fn memory_component_trims_to_16() {
        let mut memory = NpcMemoryComponent::default();
        for i in 0..20 {
            memory.remember(entry_with_type(i, NpcInteractionType::Trade));
        }

        assert_eq!(
            memory.interactions.len(),
            16,
            "should trim to 16 entries, got {}",
            memory.interactions.len()
        );
        // Oldest unpinned (Trade) entries should be evicted first
        assert_eq!(
            memory.interactions[0].timestamp, 4,
            "oldest remaining entry should be timestamp 4 (first 4 evicted)"
        );
    }

    // -----------------------------------------------------------------------
    // Pinned eviction
    // -----------------------------------------------------------------------

    #[test]
    fn pinned_not_evicted() {
        let mut memory = NpcMemoryComponent::default();
        // Fill with 14 Trade entries (unpinned) and 2 Attack entries (pinned)
        for i in 0..14 {
            memory.remember(entry_with_type(i, NpcInteractionType::Trade));
        }
        memory.remember(entry_with_type(100, NpcInteractionType::Attack));
        memory.remember(entry_with_type(101, NpcInteractionType::Theft));
        assert_eq!(memory.interactions.len(), 16);

        // Add 2 more unpinned -> should evict oldest unpinned, not pinned
        memory.remember(entry_with_type(200, NpcInteractionType::Help));
        memory.remember(entry_with_type(201, NpcInteractionType::Help));

        assert_eq!(memory.interactions.len(), 16);
        // Pinned entries should still be present
        assert!(
            memory
                .interactions
                .iter()
                .any(|e| e.timestamp == 100 && e.interaction_type == NpcInteractionType::Attack),
            "Attack (pinned) entry at t=100 should survive eviction"
        );
        assert!(
            memory
                .interactions
                .iter()
                .any(|e| e.timestamp == 101 && e.interaction_type == NpcInteractionType::Theft),
            "Theft (pinned) entry at t=101 should survive eviction"
        );
    }

    #[test]
    fn oldest_pinned_downgrades_when_full() {
        let mut memory = NpcMemoryComponent::default();
        // Fill all 16 slots with pinned entries (Attack)
        for i in 0..16 {
            memory.remember(entry_with_type(i, NpcInteractionType::Attack));
        }
        assert_eq!(memory.interactions.len(), 16);

        // Add one more -> no unpinned to evict, so oldest pinned (t=0) is evicted
        memory.remember(entry_with_type(100, NpcInteractionType::Attack));
        assert_eq!(
            memory.interactions.len(),
            16,
            "should still be 16 after adding to full pinned list"
        );
        assert!(
            !memory.interactions.iter().any(|e| e.timestamp == 0),
            "oldest pinned entry (t=0) should be evicted when all slots are pinned"
        );
        assert!(
            memory.interactions.iter().any(|e| e.timestamp == 100),
            "newly added entry (t=100) should be present"
        );
    }

    #[test]
    fn ambushed_is_pinned() {
        assert!(
            is_pinned(NpcInteractionType::Ambushed),
            "Ambushed should be a pinned interaction type"
        );
    }

    #[test]
    fn trade_help_fled_not_pinned() {
        assert!(
            !is_pinned(NpcInteractionType::Trade),
            "Trade should not be pinned"
        );
        assert!(
            !is_pinned(NpcInteractionType::Help),
            "Help should not be pinned"
        );
        assert!(
            !is_pinned(NpcInteractionType::FledFrom),
            "FledFrom should not be pinned"
        );
        assert!(
            !is_pinned(NpcInteractionType::TradeRefused),
            "TradeRefused should not be pinned"
        );
    }

    // -----------------------------------------------------------------------
    // New variant serde roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn new_variants_serde_roundtrip() {
        for ty in [
            NpcInteractionType::TradeRefused,
            NpcInteractionType::Ambushed,
            NpcInteractionType::FledFrom,
        ] {
            let entry = entry_with_type(42, ty);
            let json = serde_json::to_string(&entry)
                .unwrap_or_else(|e| panic!("serialize {:?} failed: {}", ty, e));
            let deserialized: NpcMemoryEntry = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("deserialize {:?} failed: {}", ty, e));
            assert_eq!(
                deserialized.interaction_type, ty,
                "serde roundtrip for {:?} should preserve interaction_type",
                ty
            );
            assert_eq!(deserialized.timestamp, 42);
        }
    }

    // -----------------------------------------------------------------------
    // Helper method tests
    // -----------------------------------------------------------------------

    #[test]
    fn has_been_attacked_by_returns_true_for_matching_player() {
        let mut memory = NpcMemoryComponent::default();
        memory.remember(entry_for_player(
            1,
            "player:abc",
            NpcInteractionType::Attack,
        ));
        memory.remember(entry_for_player(2, "player:def", NpcInteractionType::Trade));

        assert!(
            memory.has_been_attacked_by("player:abc"),
            "should find attack by player:abc"
        );
        assert!(
            !memory.has_been_attacked_by("player:def"),
            "player:def traded, not attacked"
        );
        assert!(
            !memory.has_been_attacked_by("player:nonexistent"),
            "nonexistent player should return false"
        );
    }

    #[test]
    fn has_traded_with_returns_true_for_matching_player() {
        let mut memory = NpcMemoryComponent::default();
        memory.remember(entry_for_player(1, "player:abc", NpcInteractionType::Trade));

        assert!(
            memory.has_traded_with("player:abc"),
            "should find trade with player:abc"
        );
        assert!(
            !memory.has_traded_with("player:def"),
            "player:def has no trade record"
        );
    }

    #[test]
    fn has_been_robbed_by_returns_true_for_matching_player() {
        let mut memory = NpcMemoryComponent::default();
        memory.remember(entry_for_player(
            1,
            "player:thief",
            NpcInteractionType::Theft,
        ));

        assert!(
            memory.has_been_robbed_by("player:thief"),
            "should find theft by player:thief"
        );
        assert!(
            !memory.has_been_robbed_by("player:innocent"),
            "innocent player should return false"
        );
    }

    // -----------------------------------------------------------------------
    // Existing tests (updated for MAX=16)
    // -----------------------------------------------------------------------

    #[test]
    fn memory_bubble_probability() {
        let npc = "npc:rogue:42";
        let player = "offline:Azure";
        let mut true_count = 0;
        for count in 0..64 {
            if should_emit_memory_bubble(npc, player, count) {
                true_count += 1;
            }
        }

        assert!(
            (20..=44).contains(&true_count),
            "deterministic 50% gate drifted: {true_count}"
        );
    }

    #[test]
    fn memory_bubble_text_matches_archetype_context() {
        let mut cheated = entry(1);
        cheated.interaction_type = NpcInteractionType::Trade;
        cheated.outcome = NpcInteractionOutcome::Deceived;
        assert_eq!(
            memory_bubble_text(NpcArchetype::Rogue, &cheated),
            "...你。上次的骨币，成色不对。"
        );
    }

    #[test]
    fn remember_trims_to_max_limit() {
        let mut memory = NpcMemoryComponent::default();
        // With mixed pinned/unpinned, only unpinned (Trade) get evicted
        for i in 0..20 {
            memory.remember(entry(i)); // alternates Trade (even) and Attack (odd, pinned)
        }

        assert_eq!(
            memory.interactions.len(),
            MAX_NPC_MEMORY_ENTRIES,
            "should trim to MAX_NPC_MEMORY_ENTRIES={}",
            MAX_NPC_MEMORY_ENTRIES
        );
    }
}
