use valence::prelude::{Client, EventReader, Local, Query, Res, With};

use crate::cultivation::tribulation::{
    ascension_quota_limit, AscensionQuotaOccupied, AscensionQuotaOpened,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::persistence::{load_ascension_quota, PersistenceSettings};
use crate::schema::server_data::{AscensionQuotaV1, ServerDataPayloadV1, ServerDataV1};

pub fn emit_ascension_quota_payloads(
    settings: Res<PersistenceSettings>,
    mut opened: EventReader<AscensionQuotaOpened>,
    mut occupied: EventReader<AscensionQuotaOccupied>,
    mut clients: Query<&mut Client, With<Client>>,
    mut last_client_count: Local<Option<usize>>,
) {
    let joined_count = clients.iter_mut().count();
    let count_changed = last_client_count.replace(joined_count) != Some(joined_count);
    let mut should_broadcast = false;
    let mut latest_occupied_slots = None;

    for ev in opened.read() {
        should_broadcast = true;
        latest_occupied_slots = Some(ev.occupied_slots);
    }
    for ev in occupied.read() {
        should_broadcast = true;
        latest_occupied_slots = Some(ev.occupied_slots);
    }

    if !should_broadcast && !count_changed {
        return;
    }

    if joined_count == 0 {
        return;
    }

    let data = build_ascension_quota_payload(&settings, joined_count, latest_occupied_slots);
    for mut client in &mut clients {
        send_ascension_quota_to_client(&mut client, data);
    }
}

fn build_ascension_quota_payload(
    settings: &PersistenceSettings,
    joined_count: usize,
    occupied_override: Option<u32>,
) -> AscensionQuotaV1 {
    let occupied_slots = occupied_override.unwrap_or_else(|| {
        load_ascension_quota(settings)
            .map(|quota| quota.occupied_slots)
            .unwrap_or_else(|error| {
                tracing::warn!("[bong][network] failed to load ascension quota: {error}");
                0
            })
    });
    AscensionQuotaV1::new(occupied_slots, ascension_quota_limit(joined_count))
}

fn send_ascension_quota_to_client(client: &mut Client, data: AscensionQuotaV1) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::AscensionQuota(data));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(payload) => payload,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(client, payload_bytes.as_slice());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use crate::persistence::{bootstrap_sqlite, complete_tribulation_ascension};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use valence::prelude::{App, Events, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn unique_temp_dir(test_name: &str) -> PathBuf {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "bong-ascension-quota-{test_name}-{}-{unique_suffix}",
            std::process::id()
        ))
    }

    fn persistence_settings(test_name: &str) -> (PersistenceSettings, PathBuf) {
        let root = unique_temp_dir(test_name);
        let db_path = root.join("data").join("bong.db");
        let deceased_dir = root.join("library-web").join("public").join("deceased");
        bootstrap_sqlite(&db_path, &format!("ascension-quota-{test_name}"))
            .expect("sqlite bootstrap should succeed");
        (
            PersistenceSettings::with_paths(
                &db_path,
                &deceased_dir,
                format!("ascension-quota-{test_name}"),
            ),
            root,
        )
    }

    fn spawn_mock_client(app: &mut App, name: &str) -> MockClientHelper {
        let (bundle, helper) = create_mock_client(name);
        app.world_mut().spawn(bundle);
        helper
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    fn collect_ascension_quota_payloads(helper: &mut MockClientHelper) -> Vec<AscensionQuotaV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("server data payload should decode");
            if let ServerDataPayloadV1::AscensionQuota(data) = payload.payload {
                payloads.push(data);
            }
        }
        payloads
    }

    #[test]
    fn joined_client_receives_current_ascension_quota() {
        let (settings, root) = persistence_settings("join-snapshot");
        complete_tribulation_ascension(&settings, "offline:VoidWalker")
            .expect("quota setup should succeed");
        let mut app = App::new();
        app.insert_resource(settings);
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<AscensionQuotaOccupied>();
        app.add_systems(Update, emit_ascension_quota_payloads);
        let mut helper = spawn_mock_client(&mut app, "Azure");

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_ascension_quota_payloads(&mut helper);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0], AscensionQuotaV1::new(1, 1));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn quota_events_broadcast_latest_snapshot() {
        let (settings, root) = persistence_settings("broadcast-events");
        let mut app = App::new();
        app.insert_resource(settings);
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<AscensionQuotaOccupied>();
        app.add_systems(Update, emit_ascension_quota_payloads);
        let mut first = spawn_mock_client(&mut app, "Azure");
        let mut second = spawn_mock_client(&mut app, "Beryl");
        app.update();
        flush_all_client_packets(&mut app);
        let _ = collect_ascension_quota_payloads(&mut first);
        let _ = collect_ascension_quota_payloads(&mut second);

        app.world_mut()
            .resource_mut::<Events<AscensionQuotaOccupied>>()
            .send(AscensionQuotaOccupied { occupied_slots: 1 });
        app.update();
        flush_all_client_packets(&mut app);

        let first_payloads = collect_ascension_quota_payloads(&mut first);
        let second_payloads = collect_ascension_quota_payloads(&mut second);
        assert_eq!(first_payloads, vec![AscensionQuotaV1::new(1, 1)]);
        assert_eq!(second_payloads, vec![AscensionQuotaV1::new(1, 1)]);

        let _ = std::fs::remove_dir_all(root);
    }
}
