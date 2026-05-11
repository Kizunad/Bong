//! plan-gathering-ux-v1 — 采集工具、进度 session 与品质判定。

pub mod feedback;
pub mod quality;
pub mod session;
pub mod tools;

use valence::prelude::{App, EventReader, IntoSystemConfigs, Query, Update, With};

use feedback::emit_gathering_feedback;
use session::{
    apply_gathering_tool_durability, enforce_gathering_session_constraints,
    tick_gathering_sessions, GatheringCompleteEvent, GatheringProgressFrame, GatheringSessionStore,
};
use tools::GatheringTargetKind;

use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

pub fn register(app: &mut App) {
    app.insert_resource(GatheringSessionStore::default());
    app.add_event::<GatheringProgressFrame>();
    app.add_event::<GatheringCompleteEvent>();
    app.add_systems(
        Update,
        (
            enforce_gathering_session_constraints,
            tick_gathering_sessions,
            apply_gathering_tool_durability,
            emit_gathering_feedback,
            emit_gathering_progress,
        )
            .chain(),
    );
}

fn emit_gathering_progress(
    mut frames: EventReader<GatheringProgressFrame>,
    mut clients: Query<&mut valence::prelude::Client, With<valence::prelude::Client>>,
) {
    use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};

    for frame in frames.read() {
        let Ok(mut client) = clients.get_mut(frame.player) else {
            continue;
        };
        let payload = ServerDataV1::new(ServerDataPayloadV1::GatheringSession {
            session_id: frame.session_id.clone(),
            progress_ticks: frame.progress_ticks,
            total_ticks: frame.total_ticks,
            target_name: frame.target_name.clone(),
            target_type: target_type_wire(frame.target_type).to_string(),
            quality_hint: frame.quality_hint.clone(),
            tool_used: frame.tool_used.clone(),
            interrupted: frame.interrupted,
            completed: frame.completed,
        });
        let payload_type = payload_type_label(payload.payload_type());
        match serialize_server_data_payload(&payload) {
            Ok(bytes) => send_server_data_payload(&mut client, bytes.as_slice()),
            Err(error) => log_payload_build_error(payload_type, &error),
        }
    }
}

pub fn target_type_wire(target: GatheringTargetKind) -> &'static str {
    match target {
        GatheringTargetKind::Herb => "herb",
        GatheringTargetKind::Ore => "ore",
        GatheringTargetKind::Wood => "wood",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::Events;

    #[test]
    fn register_installs_gathering_resources_and_events() {
        let mut app = App::new();
        register(&mut app);

        assert!(app.world().contains_resource::<GatheringSessionStore>());
        assert!(app
            .world()
            .contains_resource::<Events<GatheringProgressFrame>>());
        assert!(app
            .world()
            .contains_resource::<Events<GatheringCompleteEvent>>());
    }
}
