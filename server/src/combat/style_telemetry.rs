use valence::prelude::{bevy_ecs, Entity, Event, EventReader, EventWriter, Query, Res, Username};

use crate::cultivation::components::QiColor;
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::player::state::canonical_player_id;
use crate::schema::style_balance::{StyleBalanceTelemetryEventV1, StyleTelemetryColorSnapshotV1};

use super::events::DeathEvent;

#[derive(Debug, Clone, Event, PartialEq)]
pub struct StyleBalanceTelemetryEvent {
    pub attacker: Entity,
    pub attacker_player_id: String,
    pub defender: Entity,
    pub defender_player_id: String,
    pub attacker_color: Option<StyleTelemetryColorSnapshotV1>,
    pub defender_color: Option<StyleTelemetryColorSnapshotV1>,
    pub cause: String,
    pub resolved_at_tick: u64,
}

type StyleTelemetryParticipantItem<'a> = (Option<&'a Username>, Option<&'a QiColor>);

pub fn collect_hunyuan_pvp_telemetry(
    mut deaths: EventReader<DeathEvent>,
    participants: Query<StyleTelemetryParticipantItem<'_>>,
    mut telemetry: EventWriter<StyleBalanceTelemetryEvent>,
) {
    for death in deaths.read() {
        let Some(attacker) = death.attacker else {
            continue;
        };
        let Some(attacker_player_id) = death.attacker_player_id.as_ref() else {
            continue;
        };
        let Ok((Some(defender_username), defender_color)) = participants.get(death.target) else {
            continue;
        };
        let attacker_color = participants
            .get(attacker)
            .ok()
            .and_then(|(_, color)| color.map(StyleTelemetryColorSnapshotV1::from));

        telemetry.send(StyleBalanceTelemetryEvent {
            attacker,
            attacker_player_id: attacker_player_id.clone(),
            defender: death.target,
            defender_player_id: canonical_player_id(defender_username.0.as_str()),
            attacker_color,
            defender_color: defender_color.map(StyleTelemetryColorSnapshotV1::from),
            cause: death.cause.clone(),
            resolved_at_tick: death.at_tick,
        });
    }
}

pub fn publish_style_balance_telemetry_events(
    redis: Option<Res<RedisBridgeResource>>,
    mut events: EventReader<StyleBalanceTelemetryEvent>,
) {
    let Some(redis) = redis else {
        return;
    };

    for event in events.read() {
        let payload = StyleBalanceTelemetryEventV1 {
            v: 1,
            attacker_player_id: event.attacker_player_id.clone(),
            defender_player_id: event.defender_player_id.clone(),
            attacker_color: event.attacker_color.clone(),
            defender_color: event.defender_color.clone(),
            cause: event.cause.clone(),
            resolved_at_tick: event.resolved_at_tick,
        };
        if let Err(error) = redis
            .tx_outbound
            .send(RedisOutbound::StyleBalanceTelemetry(payload))
        {
            tracing::warn!(
                "[bong][combat][style-balance] failed to queue telemetry outbound: {error}"
            );
        }
    }
}

impl From<&QiColor> for StyleTelemetryColorSnapshotV1 {
    fn from(color: &QiColor) -> Self {
        Self {
            main: color.main,
            secondary: color.secondary,
            is_chaotic: color.is_chaotic,
            is_hunyuan: color.is_hunyuan,
        }
    }
}

#[cfg(test)]
mod tests {
    use valence::prelude::{App, IntoSystemConfigs, Update, Username};

    use super::*;
    use crate::cultivation::components::{ColorKind, QiColor};
    use crate::network::{redis_bridge::RedisOutbound, RedisBridgeResource};

    fn setup_app() -> (App, crossbeam_channel::Receiver<RedisOutbound>) {
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.add_event::<StyleBalanceTelemetryEvent>();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_systems(Update, collect_hunyuan_pvp_telemetry);
        app.add_systems(
            Update,
            publish_style_balance_telemetry_events.after(collect_hunyuan_pvp_telemetry),
        );
        (app, rx_outbound)
    }

    #[test]
    fn pvp_death_publishes_hunyuan_telemetry_snapshot() {
        let (mut app, rx_outbound) = setup_app();

        let attacker = app
            .world_mut()
            .spawn((
                Username("Killer".into()),
                QiColor {
                    main: ColorKind::Heavy,
                    secondary: Some(ColorKind::Solid),
                    is_chaotic: false,
                    is_hunyuan: true,
                    ..Default::default()
                },
            ))
            .id();
        let defender = app
            .world_mut()
            .spawn((
                Username("Defender".into()),
                QiColor {
                    main: ColorKind::Violent,
                    secondary: None,
                    is_chaotic: false,
                    is_hunyuan: false,
                    ..Default::default()
                },
            ))
            .id();

        app.world_mut().send_event(DeathEvent {
            target: defender,
            cause: "attack_intent:offline:Killer".to_string(),
            attacker: Some(attacker),
            attacker_player_id: Some("offline:Killer".to_string()),
            at_tick: 88,
        });
        app.update();

        let outbound = rx_outbound.try_recv().expect("expected telemetry outbound");
        let RedisOutbound::StyleBalanceTelemetry(collected) = outbound else {
            panic!("expected style balance telemetry outbound, got {outbound:?}");
        };
        assert_eq!(collected.attacker_player_id, "offline:Killer");
        assert_eq!(collected.defender_player_id, "offline:Defender");
        assert_eq!(
            collected.attacker_color.as_ref().map(|c| c.is_hunyuan),
            Some(true)
        );
        assert_eq!(
            collected.defender_color.as_ref().map(|c| c.main),
            Some(ColorKind::Violent)
        );
    }

    #[test]
    fn non_pvp_death_does_not_emit_telemetry() {
        let (mut app, rx_outbound) = setup_app();
        let defender = app.world_mut().spawn(Username("Defender".into())).id();

        app.world_mut().send_event(DeathEvent {
            target: defender,
            cause: "bleed_out".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 88,
        });
        app.update();

        assert!(rx_outbound.try_recv().is_err());
    }
}
