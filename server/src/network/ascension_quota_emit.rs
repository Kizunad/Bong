use valence::prelude::{Client, EventReader, Local, Query, Res, With};

use crate::cultivation::tribulation::{
    check_void_quota, AscensionQuotaOccupied, AscensionQuotaOpened, VoidQuotaConfig,
    VOID_QUOTA_BASIS,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::persistence::{
    load_active_tribulation_count, load_ascension_quota, PersistenceSettings,
};
use crate::qi_physics::WorldQiBudget;
use crate::schema::server_data::{AscensionQuotaV1, ServerDataPayloadV1, ServerDataV1};

#[derive(Default)]
pub(crate) struct AscensionQuotaEmitState {
    last_client_count: Option<usize>,
    last_payload: Option<AscensionQuotaV1>,
}

pub fn emit_ascension_quota_payloads(
    settings: Res<PersistenceSettings>,
    budget: Res<WorldQiBudget>,
    void_quota: Res<VoidQuotaConfig>,
    mut opened: EventReader<AscensionQuotaOpened>,
    mut occupied: EventReader<AscensionQuotaOccupied>,
    mut clients: Query<&mut Client, With<Client>>,
    mut emit_state: Local<AscensionQuotaEmitState>,
) {
    let joined_count = clients.iter_mut().count();
    let count_changed = emit_state.last_client_count != Some(joined_count);
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

    if joined_count == 0 {
        emit_state.last_client_count = Some(0);
        return;
    }

    let Some(data) =
        build_ascension_quota_payload(&settings, &budget, &void_quota, latest_occupied_slots)
    else {
        return;
    };
    let payload_changed = emit_state.last_payload.as_ref() != Some(&data);
    if !should_broadcast && !count_changed && !payload_changed {
        return;
    }
    emit_state.last_client_count = Some(joined_count);
    emit_state.last_payload = Some(data.clone());

    for mut client in &mut clients {
        send_ascension_quota_to_client(&mut client, data.clone());
    }
}

fn build_ascension_quota_payload(
    settings: &PersistenceSettings,
    budget: &WorldQiBudget,
    void_quota: &VoidQuotaConfig,
    occupied_override: Option<u32>,
) -> Option<AscensionQuotaV1> {
    let occupied_slots = match occupied_override {
        Some(slots) => slots,
        None => match load_ascension_quota(settings) {
            Ok(quota) => quota.occupied_slots,
            Err(error) => {
                tracing::warn!("[bong][network] failed to load ascension quota: {error}");
                return None;
            }
        },
    };
    let active_du_xu_slots = match load_active_tribulation_count(settings) {
        Ok(slots) => slots,
        Err(error) => {
            tracing::warn!("[bong][network] failed to load active tribulation count: {error}");
            return None;
        }
    };
    let quota = check_void_quota(
        occupied_slots.saturating_add(active_du_xu_slots),
        budget,
        void_quota,
    );
    Some(AscensionQuotaV1::with_world_qi(
        quota.occupied_slots,
        quota.quota_limit,
        quota.total_world_qi,
        quota.quota_k,
        VOID_QUOTA_BASIS,
    ))
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
    use crate::cultivation::tribulation::DEFAULT_VOID_QUOTA_K;
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use crate::persistence::{
        bootstrap_sqlite, complete_tribulation_ascension, persist_active_tribulation,
        ActiveTribulationRecord,
    };
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
        app.insert_resource(WorldQiBudget::from_total(100.0));
        app.insert_resource(VoidQuotaConfig::default());
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<AscensionQuotaOccupied>();
        app.add_systems(Update, emit_ascension_quota_payloads);
        let mut helper = spawn_mock_client(&mut app, "Azure");

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_ascension_quota_payloads(&mut helper);
        assert_eq!(payloads.len(), 1);
        assert_eq!(
            payloads[0],
            AscensionQuotaV1::with_world_qi(1, 2, 100.0, DEFAULT_VOID_QUOTA_K, VOID_QUOTA_BASIS)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn joined_client_snapshot_counts_in_flight_tribulation_slots() {
        let (settings, root) = persistence_settings("join-in-flight");
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: "offline:Azure".to_string(),
                wave_current: 1,
                waves_total: 3,
                started_tick: 100,
            },
        )
        .expect("active tribulation should persist");
        let mut app = App::new();
        app.insert_resource(settings);
        app.insert_resource(WorldQiBudget::from_total(50.0));
        app.insert_resource(VoidQuotaConfig::default());
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<AscensionQuotaOccupied>();
        app.add_systems(Update, emit_ascension_quota_payloads);
        let mut helper = spawn_mock_client(&mut app, "Beryl");

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_ascension_quota_payloads(&mut helper);
        assert_eq!(
            payloads,
            vec![AscensionQuotaV1::with_world_qi(
                1,
                1,
                50.0,
                DEFAULT_VOID_QUOTA_K,
                VOID_QUOTA_BASIS
            )]
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn world_qi_budget_change_broadcasts_quota_refresh() {
        let (settings, root) = persistence_settings("budget-refresh");
        let mut app = App::new();
        app.insert_resource(settings);
        app.insert_resource(WorldQiBudget::from_total(100.0));
        app.insert_resource(VoidQuotaConfig::default());
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<AscensionQuotaOccupied>();
        app.add_systems(Update, emit_ascension_quota_payloads);
        let mut helper = spawn_mock_client(&mut app, "Azure");

        app.update();
        flush_all_client_packets(&mut app);
        let _ = collect_ascension_quota_payloads(&mut helper);

        app.world_mut()
            .resource_mut::<WorldQiBudget>()
            .current_total = 50.0;
        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_ascension_quota_payloads(&mut helper);
        assert_eq!(
            payloads,
            vec![AscensionQuotaV1::with_world_qi(
                0,
                1,
                50.0,
                DEFAULT_VOID_QUOTA_K,
                VOID_QUOTA_BASIS
            )]
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn persistence_read_failure_skips_quota_broadcast() {
        let root = unique_temp_dir("missing-db");
        let settings = PersistenceSettings::with_paths(
            root.join("missing").join("bong.db"),
            root.join("library-web").join("public").join("deceased"),
            "ascension-quota-missing-db",
        );
        let mut app = App::new();
        app.insert_resource(settings);
        app.insert_resource(WorldQiBudget::from_total(100.0));
        app.insert_resource(VoidQuotaConfig::default());
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<AscensionQuotaOccupied>();
        app.add_systems(Update, emit_ascension_quota_payloads);
        let mut helper = spawn_mock_client(&mut app, "Azure");

        app.update();
        flush_all_client_packets(&mut app);

        assert!(collect_ascension_quota_payloads(&mut helper).is_empty());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn quota_events_broadcast_latest_snapshot() {
        let (settings, root) = persistence_settings("broadcast-events");
        let mut app = App::new();
        app.insert_resource(settings);
        app.insert_resource(WorldQiBudget::from_total(100.0));
        app.insert_resource(VoidQuotaConfig::default());
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
        let expected =
            AscensionQuotaV1::with_world_qi(1, 2, 100.0, DEFAULT_VOID_QUOTA_K, VOID_QUOTA_BASIS);
        assert_eq!(first_payloads, vec![expected.clone()]);
        assert_eq!(second_payloads, vec![expected]);

        let _ = std::fs::remove_dir_all(root);
    }
}
