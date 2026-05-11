//! plan-poi-novice-v1 — 新手 POI Bevy event → Redis outbound。

use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{Entity, EventReader, Query, Res, Username, With};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::player::state::canonical_player_id;
use crate::schema::poi_novice::{PoiNoviceKindV1, PoiSpawnedEventV1, TrespassEventV1};
use crate::world::poi_novice::{PoiNoviceKind, PoiSpawned, TrespassEvent, TRADE_REFUSAL_SECONDS};

pub fn publish_poi_spawned_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<PoiSpawned>,
) {
    for event in events.read() {
        let site = &event.site;
        let payload = PoiSpawnedEventV1 {
            v: 1,
            kind: "poi_spawned".to_string(),
            poi_id: site.id.clone(),
            poi_type: kind_to_wire(site.kind),
            zone: site.zone.clone(),
            pos: [
                f64::from(site.pos_xyz[0]),
                f64::from(site.pos_xyz[1]),
                f64::from(site.pos_xyz[2]),
            ],
            selection_strategy: site.selection_strategy.clone(),
            qi_affinity: site.qi_affinity,
            danger_bias: site.danger_bias,
        };
        if let Err(error) = redis.tx_outbound.send(RedisOutbound::PoiSpawned(payload)) {
            tracing::warn!("[bong][poi-novice-bridge] dropped PoiSpawned: {error}");
        }
    }
}

pub fn publish_trespass_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<TrespassEvent>,
    clients: Query<&Username, With<valence::prelude::Client>>,
) {
    let now = wall_clock_secs();
    for event in events.read() {
        let payload = TrespassEventV1 {
            v: 1,
            kind: "trespass".to_string(),
            village_id: event.village_id.clone(),
            player_id: player_id(event.player, &clients),
            killed_npc_count: event.killed_npc_count,
            refusal_until_wall_clock_secs: now.saturating_add(TRADE_REFUSAL_SECONDS),
        };
        if let Err(error) = redis.tx_outbound.send(RedisOutbound::PoiTrespass(payload)) {
            tracing::warn!("[bong][poi-novice-bridge] dropped TrespassEvent: {error}");
        }
    }
}

fn kind_to_wire(kind: PoiNoviceKind) -> PoiNoviceKindV1 {
    match kind {
        PoiNoviceKind::ForgeStation => PoiNoviceKindV1::ForgeStation,
        PoiNoviceKind::AlchemyFurnace => PoiNoviceKindV1::AlchemyFurnace,
        PoiNoviceKind::RogueVillage => PoiNoviceKindV1::RogueVillage,
        PoiNoviceKind::MutantNest => PoiNoviceKindV1::MutantNest,
        PoiNoviceKind::ScrollHidden => PoiNoviceKindV1::ScrollHidden,
        PoiNoviceKind::SpiritHerbValley => PoiNoviceKindV1::SpiritHerbValley,
        PoiNoviceKind::HerbPatch => PoiNoviceKindV1::HerbPatch,
        PoiNoviceKind::QiSpring => PoiNoviceKindV1::QiSpring,
        PoiNoviceKind::TradeSpot => PoiNoviceKindV1::TradeSpot,
        PoiNoviceKind::ShelterSpot => PoiNoviceKindV1::ShelterSpot,
        PoiNoviceKind::WaterSource => PoiNoviceKindV1::WaterSource,
    }
}

fn player_id(entity: Entity, clients: &Query<&Username, With<valence::prelude::Client>>) -> String {
    clients
        .get(entity)
        .map(|username| canonical_player_id(username.0.as_str()))
        .unwrap_or_else(|_| format!("entity:{entity:?}"))
}

fn wall_clock_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::RedisBridgeResource;
    use crate::world::poi_novice::{PoiNoviceSite, PoiSpawned, TrespassEvent};
    use crossbeam_channel::unbounded;
    use valence::prelude::{App, Update};

    fn setup_app() -> (App, crossbeam_channel::Receiver<RedisOutbound>) {
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();
        let mut app = App::new();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<PoiSpawned>();
        app.add_event::<TrespassEvent>();
        app.add_systems(
            Update,
            (publish_poi_spawned_events, publish_trespass_events),
        );
        (app, rx_outbound)
    }

    #[test]
    fn poi_spawned_event_queues_redis_payload() {
        let (mut app, rx) = setup_app();
        app.world_mut().send_event(PoiSpawned {
            site: PoiNoviceSite {
                id: "spawn:forge_station".to_string(),
                kind: PoiNoviceKind::ForgeStation,
                zone: "spawn".to_string(),
                name: "破败炼器台".to_string(),
                pos_xyz: [300.0, 71.0, 200.0],
                selection_strategy: "strict_radius_1500".to_string(),
                qi_affinity: 0.15,
                danger_bias: 0,
                tags: vec!["poi_novice".to_string()],
            },
        });
        app.update();
        let payload = match rx.try_recv().expect("poi spawned should publish") {
            RedisOutbound::PoiSpawned(payload) => payload,
            other => panic!("expected PoiSpawned, got {other:?}"),
        };
        assert_eq!(payload.poi_id, "spawn:forge_station");
        assert_eq!(payload.poi_type, PoiNoviceKindV1::ForgeStation);
    }

    #[test]
    fn trespass_event_queues_refusal_payload() {
        let (mut app, rx) = setup_app();
        app.world_mut().send_event(TrespassEvent {
            village_id: "spawn:rogue_village".to_string(),
            player: Entity::from_raw(9),
            killed_npc_count: 3,
        });
        app.update();
        let payload = match rx.try_recv().expect("trespass should publish") {
            RedisOutbound::PoiTrespass(payload) => payload,
            other => panic!("expected PoiTrespass, got {other:?}"),
        };
        assert_eq!(payload.village_id, "spawn:rogue_village");
        assert_eq!(payload.killed_npc_count, 3);
        assert!(payload.refusal_until_wall_clock_secs > TRADE_REFUSAL_SECONDS);
    }
}
