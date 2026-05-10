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
    pub attacker_style: Option<String>,
    pub defender_style: Option<String>,
    pub attacker_rejection_rate: Option<f64>,
    pub defender_resistance: Option<f64>,
    pub defender_drain_affinity: Option<f64>,
    pub attacker_qi: Option<f64>,
    pub distance_blocks: Option<f64>,
    pub effective_hit: Option<f64>,
    pub defender_lost: Option<f64>,
    pub defender_absorbed: Option<f64>,
    pub cause: String,
    pub resolved_at_tick: u64,
}

#[derive(bevy_ecs::component::Component, Debug, Clone, PartialEq)]
pub struct StyleBalanceTelemetryProfile {
    pub style: String,
    pub rejection_rate: Option<f64>,
    pub resistance: Option<f64>,
    pub drain_affinity: Option<f64>,
}

type StyleTelemetryParticipantItem<'a> = (
    Option<&'a Username>,
    Option<&'a QiColor>,
    Option<&'a StyleBalanceTelemetryProfile>,
);

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
        let Ok((Some(defender_username), defender_color, defender_profile)) =
            participants.get(death.target)
        else {
            continue;
        };
        let (attacker_color, attacker_style, attacker_rejection_rate) = participants
            .get(attacker)
            .ok()
            .map(|(_, color, profile)| {
                (
                    color.map(StyleTelemetryColorSnapshotV1::from),
                    profile.map(|profile| profile.style.clone()),
                    profile.and_then(|profile| profile.rejection_rate),
                )
            })
            .unwrap_or((None, None, None));

        telemetry.send(StyleBalanceTelemetryEvent {
            attacker,
            attacker_player_id: attacker_player_id.clone(),
            defender: death.target,
            defender_player_id: canonical_player_id(defender_username.0.as_str()),
            attacker_color,
            defender_color: defender_color.map(StyleTelemetryColorSnapshotV1::from),
            attacker_style,
            defender_style: defender_profile.map(|profile| profile.style.clone()),
            attacker_rejection_rate,
            defender_resistance: defender_profile.and_then(|profile| profile.resistance),
            defender_drain_affinity: defender_profile.and_then(|profile| profile.drain_affinity),
            // DeathEvent-only collection has no collision outcome context; a
            // future combat-resolution producer should populate these fields.
            attacker_qi: None,
            distance_blocks: None,
            effective_hit: None,
            defender_lost: None,
            defender_absorbed: None,
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
            attacker_style: normalize_style(&event.attacker_style),
            defender_style: normalize_style(&event.defender_style),
            attacker_rejection_rate: normalize_ratio(event.attacker_rejection_rate),
            defender_resistance: normalize_ratio(event.defender_resistance),
            defender_drain_affinity: normalize_ratio(event.defender_drain_affinity),
            attacker_qi: normalize_quantity(event.attacker_qi),
            distance_blocks: normalize_quantity(event.distance_blocks),
            effective_hit: normalize_quantity(event.effective_hit),
            defender_lost: normalize_quantity(event.defender_lost),
            defender_absorbed: normalize_quantity(event.defender_absorbed),
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

fn normalize_style(value: &Option<String>) -> Option<String> {
    let trimmed = value.as_deref()?.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_ratio(value: Option<f64>) -> Option<f64> {
    value
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 1.0))
}

fn normalize_quantity(value: Option<f64>) -> Option<f64> {
    value
        .filter(|value| value.is_finite())
        .map(|value| value.max(0.0))
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
                StyleBalanceTelemetryProfile {
                    style: "baomai".to_string(),
                    rejection_rate: Some(0.65),
                    resistance: None,
                    drain_affinity: None,
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
                StyleBalanceTelemetryProfile {
                    style: "jiemai".to_string(),
                    rejection_rate: None,
                    resistance: Some(0.95),
                    drain_affinity: Some(0.2),
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
        assert_eq!(collected.attacker_style.as_deref(), Some("baomai"));
        assert_eq!(collected.defender_style.as_deref(), Some("jiemai"));
        assert_eq!(collected.attacker_rejection_rate, Some(0.65));
        assert_eq!(collected.defender_resistance, Some(0.95));
        assert_eq!(collected.defender_drain_affinity, Some(0.2));
    }

    #[test]
    fn telemetry_publish_normalizes_optional_physics_fields() {
        let (mut app, rx_outbound) = setup_app();
        let attacker = app.world_mut().spawn_empty().id();
        let defender = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(StyleBalanceTelemetryEvent {
            attacker,
            attacker_player_id: "offline:Killer".to_string(),
            defender,
            defender_player_id: "offline:Defender".to_string(),
            attacker_color: None,
            defender_color: None,
            attacker_style: Some("  baomai  ".to_string()),
            defender_style: Some("   ".to_string()),
            attacker_rejection_rate: Some(1.5),
            defender_resistance: Some(-0.25),
            defender_drain_affinity: Some(f64::NAN),
            attacker_qi: Some(-5.0),
            distance_blocks: Some(3.5),
            effective_hit: Some(f64::INFINITY),
            defender_lost: Some(-1.0),
            defender_absorbed: Some(0.4),
            cause: "test".to_string(),
            resolved_at_tick: 89,
        });
        app.update();

        let outbound = rx_outbound.try_recv().expect("expected telemetry outbound");
        let RedisOutbound::StyleBalanceTelemetry(collected) = outbound else {
            panic!("expected style balance telemetry outbound, got {outbound:?}");
        };
        assert_eq!(collected.attacker_style.as_deref(), Some("baomai"));
        assert_eq!(collected.defender_style, None);
        assert_eq!(collected.attacker_rejection_rate, Some(1.0));
        assert_eq!(collected.defender_resistance, Some(0.0));
        assert_eq!(collected.defender_drain_affinity, None);
        assert_eq!(collected.attacker_qi, Some(0.0));
        assert_eq!(collected.distance_blocks, Some(3.5));
        assert_eq!(collected.effective_hit, None);
        assert_eq!(collected.defender_lost, Some(0.0));
        assert_eq!(collected.defender_absorbed, Some(0.4));
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
