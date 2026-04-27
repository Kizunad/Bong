//! plan-mineral-v1 §3 — 神识感知矿脉 resolver。

use valence::prelude::{EventReader, EventWriter, Query, Res};

use super::components::{MineralOreIndex, MineralOreNode};
use super::events::{
    MineralProbeDenialReason, MineralProbeIntent, MineralProbeResponse, MineralProbeResult,
};
use super::registry::MineralRegistry;
use crate::cultivation::components::{Cultivation, Realm};

const MIN_PROBE_REALM_RANK: u8 = 2;

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
    players: Query<&Cultivation>,
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
    players: &Query<&Cultivation>,
    nodes: &Query<&MineralOreNode>,
) -> MineralProbeResponse {
    let Ok(cultivation) = players.get(intent.player) else {
        return denied(intent, MineralProbeDenialReason::RealmTooLow);
    };
    if realm_rank(cultivation.realm) < MIN_PROBE_REALM_RANK {
        return denied(intent, MineralProbeDenialReason::RealmTooLow);
    }

    let Some(entity) = index.lookup(intent.position) else {
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
    use valence::prelude::{App, BlockPos, Events, Update};

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
            .id();
        let pos = BlockPos::new(8, 32, 8);
        let node = app
            .world_mut()
            .spawn(MineralOreNode::with_units(MineralId::SuiTie, pos, 3))
            .id();
        app.world_mut()
            .resource_mut::<MineralOreIndex>()
            .insert(pos, node);
        app.world_mut()
            .resource_mut::<Events<MineralProbeIntent>>()
            .send(MineralProbeIntent {
                player,
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

        let player = app.world_mut().spawn(Cultivation::default()).id();
        app.world_mut()
            .resource_mut::<Events<MineralProbeIntent>>()
            .send(MineralProbeIntent {
                player,
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
}
