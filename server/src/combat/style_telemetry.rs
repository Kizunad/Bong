use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Entity, Event, EventReader, EventWriter, Query, Username};

use crate::cultivation::components::{ColorKind, QiColor};

use super::events::DeathEvent;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleTelemetryColorSnapshot {
    pub main: ColorKind,
    pub secondary: Option<ColorKind>,
    pub is_chaotic: bool,
    pub is_hunyuan: bool,
}

#[derive(Debug, Clone, Event, PartialEq, Serialize, Deserialize)]
pub struct StyleBalanceTelemetryEvent {
    pub attacker: Entity,
    pub attacker_player_id: String,
    pub defender: Entity,
    pub defender_player_id: String,
    pub attacker_color: Option<StyleTelemetryColorSnapshot>,
    pub defender_color: Option<StyleTelemetryColorSnapshot>,
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
            .and_then(|(_, color)| color.map(StyleTelemetryColorSnapshot::from));

        telemetry.send(StyleBalanceTelemetryEvent {
            attacker,
            attacker_player_id: attacker_player_id.clone(),
            defender: death.target,
            defender_player_id: format!("offline:{}", defender_username.0),
            attacker_color,
            defender_color: defender_color.map(StyleTelemetryColorSnapshot::from),
            cause: death.cause.clone(),
            resolved_at_tick: death.at_tick,
        });
    }
}

impl From<&QiColor> for StyleTelemetryColorSnapshot {
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
    use valence::prelude::{App, Events, Update, Username};

    use super::*;
    use crate::cultivation::components::{ColorKind, QiColor};

    #[test]
    fn pvp_death_emits_hunyuan_telemetry_snapshot() {
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.add_event::<StyleBalanceTelemetryEvent>();
        app.add_systems(Update, collect_hunyuan_pvp_telemetry);

        let attacker = app
            .world_mut()
            .spawn((
                Username("Killer".into()),
                QiColor {
                    main: ColorKind::Heavy,
                    secondary: Some(ColorKind::Solid),
                    is_chaotic: false,
                    is_hunyuan: true,
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

        let events = app.world().resource::<Events<StyleBalanceTelemetryEvent>>();
        let mut reader = events.get_reader();
        let collected: Vec<_> = reader.read(events).collect();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].attacker_player_id, "offline:Killer");
        assert_eq!(collected[0].defender_player_id, "offline:Defender");
        assert_eq!(
            collected[0].attacker_color.as_ref().map(|c| c.is_hunyuan),
            Some(true)
        );
        assert_eq!(
            collected[0].defender_color.as_ref().map(|c| c.main),
            Some(ColorKind::Violent)
        );
    }

    #[test]
    fn non_pvp_death_does_not_emit_telemetry() {
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.add_event::<StyleBalanceTelemetryEvent>();
        app.add_systems(Update, collect_hunyuan_pvp_telemetry);
        let defender = app.world_mut().spawn(Username("Defender".into())).id();

        app.world_mut().send_event(DeathEvent {
            target: defender,
            cause: "bleed_out".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 88,
        });
        app.update();

        assert_eq!(
            app.world()
                .resource::<Events<StyleBalanceTelemetryEvent>>()
                .len(),
            0
        );
    }
}
