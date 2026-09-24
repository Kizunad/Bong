//! 终结展示：在修炼组件清理之前冻结最终属性，每个角色只发布一次。
use valence::prelude::{bevy_ecs, Client, Commands, Component, Entity, Query, Username, With};

use crate::cultivation::components::{Cultivation, MeridianSystem};
use crate::cultivation::known_techniques::KnownTechniques;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::lifespan::LifespanComponent;
use crate::schema::cultivation::realm_to_string;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1, TerminationSummaryV1};

use super::components::{Lifecycle, LifecycleState, Wounds};
use super::lifecycle::{hide_death_screen, send_payload};

#[derive(Component)]
pub struct PublishedTermination(String);

type TerminalPlayer<'a> = (
    Entity,
    &'a Lifecycle,
    Option<&'a Username>,
    Option<&'a Cultivation>,
    Option<&'a Wounds>,
    Option<&'a MeridianSystem>,
    Option<&'a KnownTechniques>,
    Option<&'a LifespanComponent>,
    Option<&'a LifeRecord>,
    Option<&'a PublishedTermination>,
);

pub fn publish_termination(
    mut commands: Commands,
    players: Query<TerminalPlayer<'_>, With<Client>>,
    mut clients: Query<&mut Client>,
) {
    for (
        entity,
        life,
        name,
        cultivation,
        wounds,
        meridians,
        techniques,
        lifespan,
        record,
        published,
    ) in &players
    {
        if life.state != LifecycleState::Terminated {
            if published.is_some() {
                commands.entity(entity).remove::<PublishedTermination>();
            }
            continue;
        }
        if published.is_some_and(|published| published.0 == life.character_id) {
            continue;
        }
        let summary = TerminationSummaryV1 {
            character_name: name.map_or_else(String::new, |name| name.0.clone()),
            realm: cultivation.map_or_else(String::new, |value| {
                realm_to_string(value.realm).to_string()
            }),
            death_count: life.death_count,
            years_lived: lifespan.map(|value| value.years_lived),
            qi_max: cultivation.map(|value| value.qi_max),
            health_max: wounds.map(|value| value.health_max),
            meridians_open: meridians.map(|value| value.opened_count() as u32),
            techniques_learned: techniques.map(|value| value.entries.len() as u32),
        };
        let cause =
            record
                .and_then(|record| record.biography.last())
                .and_then(|entry| match entry {
                    BiographyEntry::Terminated { cause, .. } => Some(cause.as_str()),
                    _ => None,
                });
        let epilogue = match cause {
            Some("voluntary_retire") => "你选择在此停步。",
            Some("tribulation_failed") => "劫数已定，此身归尘。",
            _ => "此身止于此，余痕留在残土。",
        };
        send_payload(
            &mut clients,
            entity,
            ServerDataV1::new(ServerDataPayloadV1::TerminateScreen {
                visible: true,
                final_words: record
                    .and_then(|record| record.death_insights.last())
                    .map_or_else(String::new, |insight| insight.text.clone()),
                epilogue: epilogue.to_string(),
                archetype_suggestion: String::new(),
                summary: Some(summary),
            }),
        );
        hide_death_screen(&mut clients, entity);
        commands
            .entity(entity)
            .insert(PublishedTermination(life.character_id.clone()));
    }
}
