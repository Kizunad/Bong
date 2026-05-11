//! plan-gathering-ux-v1 — 采集工具、进度 session 与品质判定。

pub mod feedback;
pub mod quality;
pub mod session;
pub mod tools;

use valence::prelude::{
    bevy_ecs, App, EventReader, IntoSystemConfigs, IntoSystemSetConfigs, Query, SystemSet, Update,
    With,
};

use feedback::emit_gathering_feedback;
use session::{
    apply_gathering_tool_durability, enforce_gathering_session_constraints,
    tick_gathering_sessions, GatheringCompleteEvent, GatheringProgressFrame, GatheringSessionStore,
};
use tools::GatheringTargetKind;

use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{
    GatheringQualityHintV1, GatheringTargetTypeV1, ServerDataPayloadV1, ServerDataV1,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum GatheringSystemSet {
    Produce,
    Emit,
}

pub fn register(app: &mut App) {
    app.insert_resource(GatheringSessionStore::default());
    app.add_event::<GatheringProgressFrame>();
    app.add_event::<GatheringCompleteEvent>();
    app.configure_sets(
        Update,
        (GatheringSystemSet::Produce, GatheringSystemSet::Emit).chain(),
    );
    app.add_systems(
        Update,
        (
            enforce_gathering_session_constraints,
            tick_gathering_sessions,
        )
            .chain()
            .in_set(GatheringSystemSet::Produce),
    );
    app.add_systems(
        Update,
        (
            apply_gathering_tool_durability,
            emit_gathering_feedback,
            emit_gathering_progress,
        )
            .chain()
            .in_set(GatheringSystemSet::Emit),
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
        let Some(payload) = gathering_payload_from_frame(frame) else {
            tracing::warn!(
                target: "bong::gathering",
                session_id = %frame.session_id,
                quality_hint = %frame.quality_hint,
                "skipping gathering progress frame with unknown quality_hint"
            );
            continue;
        };
        let payload_type = payload_type_label(payload.payload_type());
        match serialize_server_data_payload(&payload) {
            Ok(bytes) => send_server_data_payload(&mut client, bytes.as_slice()),
            Err(error) => log_payload_build_error(payload_type, &error),
        }
    }
}

fn gathering_payload_from_frame(frame: &GatheringProgressFrame) -> Option<ServerDataV1> {
    let quality_hint = quality_hint_wire(frame.quality_hint.as_str())?;
    Some(ServerDataV1::new(ServerDataPayloadV1::GatheringSession {
        session_id: frame.session_id.clone(),
        progress_ticks: frame.progress_ticks,
        total_ticks: frame.total_ticks,
        target_name: frame.target_name.clone(),
        target_type: target_type_wire(frame.target_type),
        quality_hint,
        tool_used: frame.tool_used.clone(),
        interrupted: frame.interrupted,
        completed: frame.completed,
    }))
}

pub fn target_type_wire(target: GatheringTargetKind) -> GatheringTargetTypeV1 {
    match target {
        GatheringTargetKind::Herb => GatheringTargetTypeV1::Herb,
        GatheringTargetKind::Ore => GatheringTargetTypeV1::Ore,
        GatheringTargetKind::Wood => GatheringTargetTypeV1::Wood,
    }
}

fn quality_hint_wire(hint: &str) -> Option<GatheringQualityHintV1> {
    match hint {
        "normal" => Some(GatheringQualityHintV1::Normal),
        "fine_likely" => Some(GatheringQualityHintV1::FineLikely),
        "perfect_possible" => Some(GatheringQualityHintV1::PerfectPossible),
        "fine" => Some(GatheringQualityHintV1::Fine),
        "perfect" => Some(GatheringQualityHintV1::Perfect),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{Entity, Events};

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

    #[test]
    fn gathering_wire_enums_match_shared_schema_values() {
        assert_eq!(
            target_type_wire(GatheringTargetKind::Herb),
            GatheringTargetTypeV1::Herb
        );
        assert_eq!(
            target_type_wire(GatheringTargetKind::Ore),
            GatheringTargetTypeV1::Ore
        );
        assert_eq!(
            target_type_wire(GatheringTargetKind::Wood),
            GatheringTargetTypeV1::Wood
        );
        assert_eq!(
            quality_hint_wire("fine_likely"),
            Some(GatheringQualityHintV1::FineLikely)
        );
        assert_eq!(
            quality_hint_wire("perfect_possible"),
            Some(GatheringQualityHintV1::PerfectPossible)
        );
        assert_eq!(
            quality_hint_wire("normal"),
            Some(GatheringQualityHintV1::Normal)
        );
        assert_eq!(
            quality_hint_wire("fine"),
            Some(GatheringQualityHintV1::Fine)
        );
        assert_eq!(
            quality_hint_wire("perfect"),
            Some(GatheringQualityHintV1::Perfect)
        );
        assert_eq!(quality_hint_wire("unexpected"), None);
    }

    #[test]
    fn gathering_progress_frame_builds_typed_server_data_payload() {
        let cases = [
            (
                GatheringTargetKind::Herb,
                "fine_likely",
                GatheringTargetTypeV1::Herb,
                GatheringQualityHintV1::FineLikely,
            ),
            (
                GatheringTargetKind::Ore,
                "perfect",
                GatheringTargetTypeV1::Ore,
                GatheringQualityHintV1::Perfect,
            ),
            (
                GatheringTargetKind::Wood,
                "normal",
                GatheringTargetTypeV1::Wood,
                GatheringQualityHintV1::Normal,
            ),
        ];

        for (target, hint, expected_target, expected_hint) in cases {
            let frame = GatheringProgressFrame {
                player: Entity::from_raw(7),
                session_id: format!("session:{hint}"),
                origin_position: [1.5, 64.0, 2.5],
                progress_ticks: 12,
                total_ticks: 40,
                target_name: "测试采集物".to_string(),
                target_type: target,
                quality_hint: hint.to_string(),
                tool_used: Some("pickaxe_iron".to_string()),
                interrupted: false,
                completed: false,
            };

            let payload = gathering_payload_from_frame(&frame)
                .expect("known quality_hint should build a server-data payload");
            match payload.payload {
                ServerDataPayloadV1::GatheringSession {
                    session_id,
                    progress_ticks,
                    total_ticks,
                    target_name,
                    target_type,
                    quality_hint,
                    tool_used,
                    interrupted,
                    completed,
                } => {
                    assert_eq!(session_id, frame.session_id);
                    assert_eq!(progress_ticks, frame.progress_ticks);
                    assert_eq!(total_ticks, frame.total_ticks);
                    assert_eq!(target_name, frame.target_name);
                    assert_eq!(target_type, expected_target);
                    assert_eq!(quality_hint, expected_hint);
                    assert_eq!(tool_used.as_deref(), Some("pickaxe_iron"));
                    assert!(!interrupted);
                    assert!(!completed);
                }
                other => panic!("expected GatheringSession payload, got {other:?}"),
            }
        }
    }

    #[test]
    fn gathering_progress_frame_rejects_unknown_quality_hint() {
        let frame = GatheringProgressFrame {
            player: Entity::from_raw(7),
            session_id: "session:bad".to_string(),
            origin_position: [1.5, 64.0, 2.5],
            progress_ticks: 12,
            total_ticks: 40,
            target_name: "测试采集物".to_string(),
            target_type: GatheringTargetKind::Herb,
            quality_hint: "legendary".to_string(),
            tool_used: None,
            interrupted: false,
            completed: false,
        };

        assert!(
            gathering_payload_from_frame(&frame).is_none(),
            "unknown quality_hint must be rejected before serializing gathering_session payload"
        );
    }
}
