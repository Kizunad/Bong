//! plan-identity-v1 P5：NPC 身份传话扩散。
//!
//! 模型保持最小可用：witness NPC 在看见 RevealedEvent 后记录一条
//! [`IdentityGossipTag`]；周期 tick 内仅向**同 zone + 同 faction** NPC 扩散。
//! 扩散概率由 [`IdentityGossipConfig`] 控制，测试可设为 1.0 锁定行为。

use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, EventReader, EventWriter, IntoSystemConfigs,
    ParamSet, Position, Query, Res, Resource, Update, With,
};

use super::events::IdentityReactionChangedEvent;
use super::reaction::{reaction_tier_of, ReactionTier};
use super::{IdentityId, PlayerIdentities, RevealedTagKind};
use crate::cultivation::components::Realm;
use crate::cultivation::dugu::DuguRevealedEvent;
use crate::npc::faction::{FactionId, FactionMembership};
use crate::npc::movement::GameTick;
use crate::npc::spawn::NpcMarker;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityGossipTag {
    pub target_player: Entity,
    pub identity_id: IdentityId,
    pub kind: RevealedTagKind,
    pub witnessed_at_tick: u64,
    pub witness_realm: Realm,
    pub permanent: bool,
}

#[derive(Debug, Clone, Default, Component, PartialEq, Eq)]
pub struct IdentityGossipMemory {
    pub known_tags: Vec<IdentityGossipTag>,
}

impl IdentityGossipMemory {
    pub fn remember(&mut self, tag: IdentityGossipTag) -> bool {
        if self.known_tags.iter().any(|known| {
            known.target_player == tag.target_player
                && known.identity_id == tag.identity_id
                && known.kind == tag.kind
        }) {
            return false;
        }
        self.known_tags.push(tag);
        true
    }
}

#[derive(Debug, Clone, Resource)]
pub struct IdentityGossipConfig {
    pub interval_ticks: u32,
    pub spread_probability: f32,
}

impl Default for IdentityGossipConfig {
    fn default() -> Self {
        Self {
            interval_ticks: 20,
            spread_probability: 0.05,
        }
    }
}

#[derive(Debug, Clone)]
struct GossipSourceSnapshot {
    source: Entity,
    zone_name: String,
    faction_id: FactionId,
    tags: Vec<IdentityGossipTag>,
}

type GossipSourceQuery<'world, 'state> = Query<
    'world,
    'state,
    (
        Entity,
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static FactionMembership,
        &'static IdentityGossipMemory,
    ),
    With<NpcMarker>,
>;

type GossipReceiverQuery<'world, 'state> = Query<
    'world,
    'state,
    (
        Entity,
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static FactionMembership,
        Option<&'static mut IdentityGossipMemory>,
    ),
    With<NpcMarker>,
>;

pub fn register(app: &mut App) {
    app.init_resource::<IdentityGossipConfig>().add_systems(
        Update,
        (
            record_dugu_gossip_witness,
            npc_identity_gossip_tick.after(record_dugu_gossip_witness),
        ),
    );
}

pub fn record_dugu_gossip_witness(
    mut commands: Commands,
    mut events: EventReader<DuguRevealedEvent>,
    players: Query<&PlayerIdentities>,
    mut memories: Query<&mut IdentityGossipMemory>,
) {
    for event in events.read() {
        let Ok(identities) = players.get(event.revealed_player) else {
            continue;
        };
        let tag = IdentityGossipTag {
            target_player: event.revealed_player,
            identity_id: identities.active_identity_id,
            kind: RevealedTagKind::DuguRevealed,
            witnessed_at_tick: event.at_tick,
            witness_realm: event.witness_realm,
            permanent: true,
        };
        if let Ok(mut memory) = memories.get_mut(event.witness) {
            memory.remember(tag);
        } else {
            let mut memory = IdentityGossipMemory::default();
            memory.remember(tag);
            commands.entity(event.witness).insert(memory);
        }
    }
}

pub fn npc_identity_gossip_tick(
    mut commands: Commands,
    mut query_set: ParamSet<(GossipSourceQuery<'_, '_>, GossipReceiverQuery<'_, '_>)>,
    zone_registry: Option<Res<ZoneRegistry>>,
    config: Option<Res<IdentityGossipConfig>>,
    game_tick: Option<Res<GameTick>>,
    players: Query<&PlayerIdentities>,
    mut reactions: EventWriter<IdentityReactionChangedEvent>,
) {
    let tick = game_tick.as_deref().map(|tick| tick.0).unwrap_or(0);
    let cfg = config.as_deref().cloned().unwrap_or_default();
    if cfg.interval_ticks == 0 || tick % cfg.interval_ticks != 0 {
        return;
    }
    let Some(zone_registry) = zone_registry.as_deref() else {
        return;
    };

    let sources = query_set
        .p0()
        .iter()
        .filter_map(|(source, position, dimension, membership, memory)| {
            if memory.known_tags.is_empty() {
                return None;
            }
            zone_name_for(zone_registry, dimension_kind(dimension), position.get()).map(
                |zone_name| GossipSourceSnapshot {
                    source,
                    zone_name,
                    faction_id: membership.faction_id,
                    tags: memory.known_tags.clone(),
                },
            )
        })
        .collect::<Vec<_>>();

    if sources.is_empty() {
        return;
    }

    for (receiver, position, dimension, membership, memory) in query_set.p1().iter_mut() {
        let Some(receiver_zone) =
            zone_name_for(zone_registry, dimension_kind(dimension), position.get())
        else {
            continue;
        };
        let mut newly_learned = Vec::new();
        for source in &sources {
            if source.source == receiver
                || source.zone_name != receiver_zone
                || source.faction_id != membership.faction_id
                || !should_spread(source.source, receiver, tick, cfg.spread_probability)
            {
                continue;
            }
            newly_learned.extend(source.tags.iter().cloned());
        }
        if newly_learned.is_empty() {
            continue;
        }

        match memory {
            Some(mut existing) => {
                for tag in newly_learned {
                    if existing.remember(tag.clone()) {
                        emit_reaction_for_new_gossip(&players, &mut reactions, tag, tick);
                    }
                }
            }
            None => {
                let mut inserted = IdentityGossipMemory::default();
                for tag in newly_learned {
                    if inserted.remember(tag.clone()) {
                        emit_reaction_for_new_gossip(&players, &mut reactions, tag, tick);
                    }
                }
                commands.entity(receiver).insert(inserted);
            }
        }
    }
}

fn emit_reaction_for_new_gossip(
    players: &Query<&PlayerIdentities>,
    reactions: &mut EventWriter<IdentityReactionChangedEvent>,
    tag: IdentityGossipTag,
    tick: u32,
) {
    let Ok(identities) = players.get(tag.target_player) else {
        return;
    };
    if identities.active_identity_id != tag.identity_id {
        return;
    }
    let Some(active) = identities.active() else {
        return;
    };
    let tier = reaction_tier_of(active);
    if tier == ReactionTier::Normal {
        return;
    }
    reactions.send(IdentityReactionChangedEvent {
        player: tag.target_player,
        identity_id: tag.identity_id,
        from_tier: ReactionTier::Normal,
        to_tier: tier,
        at_tick: u64::from(tick),
    });
}

fn zone_name_for(
    registry: &ZoneRegistry,
    dimension: DimensionKind,
    position: DVec3,
) -> Option<String> {
    registry
        .find_zone(dimension, position)
        .map(|zone| zone.name.clone())
}

fn dimension_kind(dimension: Option<&CurrentDimension>) -> DimensionKind {
    dimension.map(|dimension| dimension.0).unwrap_or_default()
}

fn should_spread(source: Entity, receiver: Entity, tick: u32, probability: f32) -> bool {
    if probability <= 0.0 {
        return false;
    }
    if probability >= 1.0 {
        return true;
    }
    let mut hash = source.to_bits();
    hash ^= receiver.to_bits().rotate_left(17);
    hash ^= u64::from(tick).rotate_left(31);
    let bucket = (hash % 10_000) as f32 / 10_000.0;
    bucket < probability
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{IdentityProfile, RevealedTag};
    use crate::social::components::Renown;
    use crate::world::dimension::DimensionKind;
    use crate::world::zone::Zone;
    use valence::prelude::{App, Events};

    fn faction(faction_id: FactionId) -> FactionMembership {
        FactionMembership {
            faction_id,
            rank: crate::npc::faction::FactionRank::Disciple,
            reputation: crate::npc::faction::Reputation::default(),
            lineage: None,
            mission_queue: crate::npc::faction::MissionQueue::default(),
        }
    }

    fn zone(name: &str, min_x: f64, max_x: f64) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (
                DVec3::new(min_x, 0.0, -50.0),
                DVec3::new(max_x, 100.0, 50.0),
            ),
            spirit_qi: 0.0,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        }
    }

    fn wanted_player_identities() -> PlayerIdentities {
        let mut profile = IdentityProfile::new(IdentityId::DEFAULT, "毒蛊师", 0);
        profile.renown = Renown {
            fame: 0,
            notoriety: 30,
            tags: Vec::new(),
        };
        profile.revealed_tags.push(RevealedTag {
            kind: RevealedTagKind::DuguRevealed,
            witnessed_at_tick: 5,
            witness_realm: Realm::Spirit,
            permanent: true,
        });
        PlayerIdentities {
            identities: vec![profile],
            active_identity_id: IdentityId::DEFAULT,
            last_switch_tick: 0,
        }
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("same", 0.0, 50.0), zone("far", 100.0, 150.0)],
        });
        app.insert_resource(IdentityGossipConfig {
            interval_ticks: 1,
            spread_probability: 1.0,
        });
        app.insert_resource(GameTick(20));
        app.add_event::<DuguRevealedEvent>();
        app.add_event::<IdentityReactionChangedEvent>();
        app.add_systems(
            Update,
            (
                record_dugu_gossip_witness,
                npc_identity_gossip_tick.after(record_dugu_gossip_witness),
            ),
        );
        app
    }

    #[test]
    fn gossip_spreads_revealed_tag_to_same_zone_same_faction_npcs() {
        let mut app = setup_app();
        let player = app.world_mut().spawn(wanted_player_identities()).id();
        let witness = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([10.0, 64.0, 0.0]),
                faction(FactionId::Attack),
            ))
            .id();
        let receiver = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([12.0, 64.0, 0.0]),
                faction(FactionId::Attack),
            ))
            .id();
        app.world_mut().send_event(DuguRevealedEvent {
            revealed_player: player,
            witness,
            witness_realm: Realm::Spirit,
            at_position: [10.0, 64.0, 0.0],
            at_tick: 20,
        });

        app.update();

        let memory = app
            .world()
            .get::<IdentityGossipMemory>(receiver)
            .expect("receiver should learn same-zone same-faction gossip");
        assert_eq!(memory.known_tags.len(), 1);
        assert_eq!(memory.known_tags[0].target_player, player);
        assert_eq!(memory.known_tags[0].kind, RevealedTagKind::DuguRevealed);

        let events = app
            .world()
            .resource::<Events<IdentityReactionChangedEvent>>();
        let mut reader = events.get_reader();
        let collected = reader.read(events).cloned().collect::<Vec<_>>();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].player, player);
        assert_eq!(collected[0].to_tier, ReactionTier::Wanted);
    }

    #[test]
    fn gossip_does_not_spread_across_zones_or_factions() {
        let mut app = setup_app();
        let player = app.world_mut().spawn(wanted_player_identities()).id();
        let witness = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([10.0, 64.0, 0.0]),
                faction(FactionId::Attack),
            ))
            .id();
        let far_same_faction = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([120.0, 64.0, 0.0]),
                faction(FactionId::Attack),
            ))
            .id();
        let same_zone_other_faction = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([14.0, 64.0, 0.0]),
                faction(FactionId::Defend),
            ))
            .id();
        app.world_mut().send_event(DuguRevealedEvent {
            revealed_player: player,
            witness,
            witness_realm: Realm::Spirit,
            at_position: [10.0, 64.0, 0.0],
            at_tick: 20,
        });

        app.update();

        assert!(app
            .world()
            .get::<IdentityGossipMemory>(far_same_faction)
            .is_none());
        assert!(app
            .world()
            .get::<IdentityGossipMemory>(same_zone_other_faction)
            .is_none());
    }
}
