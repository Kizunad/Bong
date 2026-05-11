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

pub const MAX_NPC_MEMORY_ENTRIES: usize = 8;

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpcInteractionOutcome {
    Friendly,
    Deceived,
    Harmed,
    Helped,
}

impl NpcMemoryComponent {
    pub fn remember(&mut self, entry: NpcMemoryEntry) {
        self.interactions.push(entry);
        self.trim_to_limit();
    }

    fn trim_to_limit(&mut self) {
        if self.interactions.len() > MAX_NPC_MEMORY_ENTRIES {
            let overflow = self.interactions.len() - MAX_NPC_MEMORY_ENTRIES;
            self.interactions.drain(0..overflow);
        }
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

    #[test]
    fn memory_component_fifo_8() {
        let mut memory = NpcMemoryComponent::default();
        for i in 0..10 {
            memory.interactions.push(entry(i));
            memory.trim_to_limit();
        }

        assert_eq!(memory.interactions.len(), 8);
        assert_eq!(memory.interactions[0].timestamp, 2);
        assert_eq!(memory.interactions[7].timestamp, 9);
    }

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
    fn remember_trims_to_fifo_limit() {
        let mut memory = NpcMemoryComponent::default();
        for i in 0..10 {
            memory.remember(entry(i));
        }

        assert_eq!(memory.interactions.len(), MAX_NPC_MEMORY_ENTRIES);
        assert_eq!(memory.interactions[0].timestamp, 2);
        assert_eq!(memory.interactions[7].timestamp, 9);
    }
}
