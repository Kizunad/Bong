//! plan-mineral-v1 §3 — 神识感知矿脉 resolver。

use valence::prelude::{BlockPos, DVec3, EventReader, EventWriter, Position, Query, Res};

use super::components::{MineralOreIndex, MineralOreNode};
use super::events::{
    MineralProbeDenialReason, MineralProbeIntent, MineralProbeResponse, MineralProbeResult,
};
use super::registry::MineralRegistry;
use crate::cultivation::components::{Cultivation, Realm};

const MIN_PROBE_REALM_RANK: u8 = 2;
pub const MINERAL_PROBE_MAX_DISTANCE: f64 = 6.0;

fn realm_rank(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

pub fn resolve_mineral_probe_intents(
    mut intents: EventReader<MineralProbeIntent>,
    mut responses: EventWriter<MineralProbeResponse>,
    index: Res<MineralOreIndex>,
    registry: Res<MineralRegistry>,
    players: Query<(&Cultivation, &Position)>,
    nodes: Query<&MineralOreNode>,
) {
    for intent in intents.read() {
        responses.send(resolve_one_probe(
            intent, &index, &registry, &players, &nodes,
        ));
    }
}

fn resolve_one_probe(
    intent: &MineralProbeIntent,
    index: &MineralOreIndex,
    registry: &MineralRegistry,
    players: &Query<(&Cultivation, &Position)>,
    nodes: &Query<&MineralOreNode>,
) -> MineralProbeResponse {
    let Ok((cultivation, player_position)) = players.get(intent.player) else {
        return denied(intent, MineralProbeDenialReason::RealmTooLow);
    };
    if realm_rank(cultivation.realm) < MIN_PROBE_REALM_RANK {
        return denied(intent, MineralProbeDenialReason::RealmTooLow);
    }
    if !is_probe_target_in_range(player_position.get(), intent.position) {
        return denied(intent, MineralProbeDenialReason::OutOfRange);
    }

    let Some(entity) = index.lookup(intent.dimension, intent.position) else {
        return denied(intent, MineralProbeDenialReason::NotMineralOre);
    };
    let Ok(node) = nodes.get(entity) else {
        return denied(intent, MineralProbeDenialReason::StaleOreIndex);
    };
    let Some(entry) = registry.get(node.mineral_id) else {
        return denied(intent, MineralProbeDenialReason::MineralNotRegistered);
    };

    MineralProbeResponse {
        player: intent.player,
        position: intent.position,
        result: MineralProbeResult::Found {
            mineral_id: node.mineral_id,
            remaining_units: node.remaining_units,
            display_name_zh: entry.display_name_zh.to_string(),
        },
    }
}

pub fn is_probe_target_in_range(player_pos: DVec3, target: BlockPos) -> bool {
    let target_center = DVec3::new(
        f64::from(target.x) + 0.5,
        f64::from(target.y) + 0.5,
        f64::from(target.z) + 0.5,
    );
    player_pos.distance_squared(target_center)
        <= MINERAL_PROBE_MAX_DISTANCE * MINERAL_PROBE_MAX_DISTANCE
}

fn denied(intent: &MineralProbeIntent, reason: MineralProbeDenialReason) -> MineralProbeResponse {
    MineralProbeResponse {
        player: intent.player,
        position: intent.position,
        result: MineralProbeResult::Denied { reason },
    }
}

#[cfg(test)]
mod tests {
    use super::super::components::MineralOreIndex;
    use super::super::registry::build_default_registry;
    use super::super::types::MineralId;
    use super::*;
    use crate::world::dimension::DimensionKind;
    use valence::prelude::{App, BlockPos, DVec3, Events, Position, Update};

    fn drain_responses(app: &mut App) -> Vec<MineralProbeResponse> {
        let mut events = app
            .world_mut()
            .resource_mut::<Events<MineralProbeResponse>>();
        events.drain().collect()
    }

    #[test]
    fn condense_probe_returns_mineral_and_remaining_units() {
        let mut app = App::new();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<MineralProbeResponse>();
        app.insert_resource(MineralOreIndex::default());
        app.insert_resource(build_default_registry());
        app.add_systems(Update, resolve_mineral_probe_intents);

        let player = app
            .world_mut()
            .spawn(Cultivation {
                realm: Realm::Condense,
                ..Default::default()
            })
            .insert(Position(DVec3::new(8.5, 32.0, 8.5)))
            .id();
        let pos = BlockPos::new(8, 32, 8);
        let node = app
            .world_mut()
            .spawn(MineralOreNode::with_units(MineralId::SuiTie, pos, 3))
            .id();
        app.world_mut().resource_mut::<MineralOreIndex>().insert(
            DimensionKind::Overworld,
            pos,
            node,
        );
        app.world_mut()
            .resource_mut::<Events<MineralProbeIntent>>()
            .send(MineralProbeIntent {
                player,
                dimension: DimensionKind::Overworld,
                position: pos,
            });

        app.update();
        let responses = drain_responses(&mut app);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].player, player);
        assert_eq!(responses[0].position, pos);
        assert_eq!(
            responses[0].result,
            MineralProbeResult::Found {
                mineral_id: MineralId::SuiTie,
                remaining_units: 3,
                display_name_zh: "髓铁".into(),
            }
        );
    }

    #[test]
    fn low_realm_probe_is_denied() {
        let mut app = App::new();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<MineralProbeResponse>();
        app.insert_resource(MineralOreIndex::default());
        app.insert_resource(build_default_registry());
        app.add_systems(Update, resolve_mineral_probe_intents);

        let player = app
            .world_mut()
            .spawn((Cultivation::default(), Position(DVec3::ZERO)))
            .id();
        app.world_mut()
            .resource_mut::<Events<MineralProbeIntent>>()
            .send(MineralProbeIntent {
                player,
                dimension: DimensionKind::Overworld,
                position: BlockPos::new(0, 64, 0),
            });

        app.update();
        let responses = drain_responses(&mut app);
        assert!(matches!(
            responses[0].result,
            MineralProbeResult::Denied {
                reason: MineralProbeDenialReason::RealmTooLow
            }
        ));
    }

    #[test]
    fn out_of_range_probe_is_denied_before_index_lookup() {
        let mut app = App::new();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<MineralProbeResponse>();
        app.insert_resource(MineralOreIndex::default());
        app.insert_resource(build_default_registry());
        app.add_systems(Update, resolve_mineral_probe_intents);

        let player = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Condense,
                    ..Default::default()
                },
                Position(DVec3::ZERO),
            ))
            .id();
        let pos = BlockPos::new(128, 64, 128);
        let node = app
            .world_mut()
            .spawn(MineralOreNode::with_units(MineralId::SuiTie, pos, 3))
            .id();
        app.world_mut().resource_mut::<MineralOreIndex>().insert(
            DimensionKind::Overworld,
            pos,
            node,
        );
        app.world_mut()
            .resource_mut::<Events<MineralProbeIntent>>()
            .send(MineralProbeIntent {
                player,
                dimension: DimensionKind::Overworld,
                position: pos,
            });

        app.update();
        let responses = drain_responses(&mut app);
        assert!(matches!(
            responses[0].result,
            MineralProbeResult::Denied {
                reason: MineralProbeDenialReason::OutOfRange
            }
        ));
    }

    #[test]
    fn probe_uses_dimension_scoped_index() {
        let mut app = App::new();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<MineralProbeResponse>();
        app.insert_resource(MineralOreIndex::default());
        app.insert_resource(build_default_registry());
        app.add_systems(Update, resolve_mineral_probe_intents);

        let player = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Condense,
                    ..Default::default()
                },
                Position(DVec3::new(8.5, 32.0, 8.5)),
            ))
            .id();
        let pos = BlockPos::new(8, 32, 8);
        let node = app
            .world_mut()
            .spawn(MineralOreNode::with_units(MineralId::SuiTie, pos, 3))
            .id();
        app.world_mut().resource_mut::<MineralOreIndex>().insert(
            DimensionKind::Overworld,
            pos,
            node,
        );
        app.world_mut()
            .resource_mut::<Events<MineralProbeIntent>>()
            .send(MineralProbeIntent {
                player,
                dimension: DimensionKind::Tsy,
                position: pos,
            });

        app.update();
        let responses = drain_responses(&mut app);
        assert!(matches!(
            responses[0].result,
            MineralProbeResult::Denied {
                reason: MineralProbeDenialReason::NotMineralOre
            }
        ));
    }
}
