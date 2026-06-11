use std::collections::HashMap;

use valence::message::SendMessage;
use valence::prelude::{
    Added, Client, Entity, EventReader, Query, RemovedComponents, Res, ResMut, Resource, Text,
};
use valence::protocol::packets::play::ResourcePackStatusC2s;
use valence::resource_pack::ResourcePackStatusEvent;

pub const RESOURCE_PACK_ENABLED_ENV: &str = "BONG_RESOURCE_PACK_ENABLED";
pub const RESOURCE_PACK_URL_ENV: &str = "BONG_RESOURCE_PACK_URL";
pub const RESOURCE_PACK_SHA1_ENV: &str = "BONG_RESOURCE_PACK_SHA1";
pub const RESOURCE_PACK_FORCED_ENV: &str = "BONG_RESOURCE_PACK_FORCED";

pub const DEFAULT_RESOURCE_PACK_URL: &str =
    "https://github.com/Kizunad/Bong/releases/download/resourcepack-v1/bong-full-v1.zip";
pub const DEFAULT_RESOURCE_PACK_SHA1: &str = DEFAULT_RESOURCE_PACK_MANIFEST.sha1;
const DEFAULT_RESOURCE_PACK_PROMPT: &str =
    "Bong 全量资源包；拒绝后仍可游玩，但模型、音效、贴图会降级为 vanilla 表现。";
const DEFAULT_RESOURCE_PACK_MANIFEST: ResourcePackManifest = ResourcePackManifest {
    name: "bong-full",
    version: "v1",
    file: "bong-full-v1.zip",
    sha1: "f2b8a86c4ccfb16b890b51df01df91e7232ddd29",
    size: 72_235_872,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourcePackManifest {
    pub name: &'static str,
    pub version: &'static str,
    pub file: &'static str,
    pub sha1: &'static str,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourcePackEntry {
    pub id: String,
    pub version: String,
    pub url: String,
    pub sha1: String,
    pub forced: bool,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourcePackConfig {
    pub packs: Vec<ResourcePackEntry>,
    pub disabled_reason: Option<String>,
}

impl Resource for ResourcePackConfig {}

impl Default for ResourcePackConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

impl ResourcePackConfig {
    pub fn from_env() -> Self {
        resolve_resource_pack_config(
            std::env::var(RESOURCE_PACK_ENABLED_ENV).ok().as_deref(),
            std::env::var(RESOURCE_PACK_URL_ENV).ok().as_deref(),
            std::env::var(RESOURCE_PACK_SHA1_ENV).ok().as_deref(),
            std::env::var(RESOURCE_PACK_FORCED_ENV).ok().as_deref(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourcePackClientStatus {
    Accepted,
    Declined,
    FailedDownload,
    SuccessfullyLoaded,
}

#[derive(Debug, Default)]
pub struct ResourcePackStatusStore {
    sessions: HashMap<Entity, ResourcePackSessionState>,
}

impl Resource for ResourcePackStatusStore {}

impl ResourcePackStatusStore {
    #[cfg(test)]
    pub fn get(&self, client: Entity) -> Option<ResourcePackClientStatus> {
        self.sessions
            .get(&client)
            .and_then(|session| session.last_status)
    }

    fn should_prompt(&self, client: Entity, pack: &ResourcePackEntry) -> bool {
        match self
            .sessions
            .get(&client)
            .and_then(|session| session.accepted.get(pack.id.as_str()))
        {
            Some(accepted) => !accepted.matches(pack),
            None => true,
        }
    }

    fn record_prompted(&mut self, client: Entity, pack: &ResourcePackEntry) {
        self.sessions
            .entry(client)
            .or_default()
            .prompted
            .insert(pack.id.clone(), ResourcePackVersion::from(pack));
    }

    fn remove_client(&mut self, client: Entity) -> Option<ResourcePackSessionState> {
        self.sessions.remove(&client)
    }

    fn record(
        &mut self,
        client: Entity,
        status: ResourcePackClientStatus,
    ) -> Option<ResourcePackClientStatus> {
        let session = self.sessions.entry(client).or_default();
        let previous = session.last_status.replace(status);
        match status {
            ResourcePackClientStatus::SuccessfullyLoaded => {
                session.accepted.extend(session.prompted.drain());
            }
            ResourcePackClientStatus::Declined | ResourcePackClientStatus::FailedDownload => {
                session.prompted.clear();
            }
            ResourcePackClientStatus::Accepted => {}
        }
        previous
    }

    #[cfg(test)]
    fn record_loaded_for_pack(&mut self, client: Entity, pack: &ResourcePackEntry) {
        self.sessions
            .entry(client)
            .or_default()
            .accepted
            .insert(pack.id.clone(), ResourcePackVersion::from(pack));
    }

    #[cfg(test)]
    fn accepted_version(&self, client: Entity, pack_id: &str) -> Option<&str> {
        self.sessions
            .get(&client)
            .and_then(|session| session.accepted.get(pack_id))
            .map(|accepted| accepted.version.as_str())
    }
}

#[derive(Debug, Default)]
struct ResourcePackSessionState {
    last_status: Option<ResourcePackClientStatus>,
    prompted: HashMap<String, ResourcePackVersion>,
    accepted: HashMap<String, ResourcePackVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResourcePackVersion {
    version: String,
    sha1: String,
}

impl ResourcePackVersion {
    fn matches(&self, pack: &ResourcePackEntry) -> bool {
        self.version == pack.version && self.sha1.eq_ignore_ascii_case(pack.sha1.as_str())
    }
}

impl From<&ResourcePackEntry> for ResourcePackVersion {
    fn from(pack: &ResourcePackEntry) -> Self {
        Self {
            version: pack.version.clone(),
            sha1: pack.sha1.to_ascii_lowercase(),
        }
    }
}

pub fn prompt_resource_pack_on_join(
    config: Res<ResourcePackConfig>,
    mut store: ResMut<ResourcePackStatusStore>,
    mut joined_clients: Query<(Entity, &mut Client), Added<Client>>,
) {
    if config.packs.is_empty() {
        return;
    }

    for (entity, mut client) in &mut joined_clients {
        for pack in &config.packs {
            if !store.should_prompt(entity, pack) {
                tracing::info!(
                    "[bong][resourcepack] client entity {entity:?} already accepted pack {} {}; skipping prompt",
                    pack.id,
                    pack.version,
                );
                continue;
            }
            client.set_resource_pack(
                pack.url.as_str(),
                pack.sha1.as_str(),
                pack.forced,
                Some(Text::text(pack.prompt.clone())),
            );
            store.record_prompted(entity, pack);
            tracing::info!(
                "[bong][resourcepack] prompted client entity {entity:?} with pack {} {} {}",
                pack.id,
                pack.version,
                pack.sha1,
            );
        }
    }
}

pub fn record_resource_pack_status(
    mut events: EventReader<ResourcePackStatusEvent>,
    mut store: ResMut<ResourcePackStatusStore>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let status = status_from_valence(event.status);
        store.record(event.client, status);
        match status {
            ResourcePackClientStatus::Accepted => tracing::info!(
                "[bong][resourcepack] event={} client {:?} accepted pack",
                resource_pack_event_name(status),
                event.client,
            ),
            ResourcePackClientStatus::SuccessfullyLoaded => tracing::info!(
                "[bong][resourcepack] event={} client {:?} loaded pack",
                resource_pack_event_name(status),
                event.client,
            ),
            ResourcePackClientStatus::Declined => {
                tracing::warn!(
                    "[bong][resourcepack] event={} client {:?} declined pack; continuing degraded",
                    resource_pack_event_name(status),
                    event.client,
                );
                if let Ok(mut client) = clients.get_mut(event.client) {
                    client.send_chat_message("已降级为 vanilla 资源表现；玩法逻辑仍正常。")
                }
            }
            ResourcePackClientStatus::FailedDownload => {
                tracing::warn!(
                    "[bong][resourcepack] event={} client {:?} failed to download pack; continuing degraded",
                    resource_pack_event_name(status),
                    event.client,
                );
                if let Ok(mut client) = clients.get_mut(event.client) {
                    client.send_chat_message("资源包下载失败，已降级为 vanilla 表现。")
                }
            }
        }
    }
}

pub fn cleanup_disconnected_resource_pack_sessions(
    mut store: ResMut<ResourcePackStatusStore>,
    mut removed_clients: RemovedComponents<Client>,
) {
    for client in removed_clients.read() {
        store.remove_client(client);
    }
}

pub fn resolve_resource_pack_config(
    enabled: Option<&str>,
    url: Option<&str>,
    sha1: Option<&str>,
    forced: Option<&str>,
) -> ResourcePackConfig {
    if !parse_bool_or_default(enabled, true) {
        return ResourcePackConfig {
            packs: Vec::new(),
            disabled_reason: Some(format!("{RESOURCE_PACK_ENABLED_ENV}=false")),
        };
    }

    let url = non_empty_or_default(url, DEFAULT_RESOURCE_PACK_URL);
    if !is_http_url(url) {
        return ResourcePackConfig {
            packs: Vec::new(),
            disabled_reason: Some(format!("invalid {RESOURCE_PACK_URL_ENV}")),
        };
    }

    let sha1 = non_empty_or_default(sha1, DEFAULT_RESOURCE_PACK_SHA1);
    if !is_valid_sha1_hex(sha1) {
        return ResourcePackConfig {
            packs: Vec::new(),
            disabled_reason: Some(format!("invalid {RESOURCE_PACK_SHA1_ENV}")),
        };
    }

    ResourcePackConfig {
        packs: vec![ResourcePackEntry {
            id: format!(
                "{}-{}",
                DEFAULT_RESOURCE_PACK_MANIFEST.name, DEFAULT_RESOURCE_PACK_MANIFEST.version
            ),
            version: DEFAULT_RESOURCE_PACK_MANIFEST.version.to_string(),
            url: url.to_string(),
            sha1: sha1.to_ascii_lowercase(),
            forced: parse_bool_or_default(forced, false),
            prompt: DEFAULT_RESOURCE_PACK_PROMPT.to_string(),
        }],
        disabled_reason: None,
    }
}

pub fn resource_pack_event_name(status: ResourcePackClientStatus) -> &'static str {
    match status {
        ResourcePackClientStatus::Accepted => "resource_pack_accepted",
        ResourcePackClientStatus::Declined => "resource_pack_declined",
        ResourcePackClientStatus::FailedDownload => "resource_pack_failed_download",
        ResourcePackClientStatus::SuccessfullyLoaded => "resource_pack_loaded",
    }
}

pub fn status_from_valence(status: ResourcePackStatusC2s) -> ResourcePackClientStatus {
    match status {
        ResourcePackStatusC2s::Accepted => ResourcePackClientStatus::Accepted,
        ResourcePackStatusC2s::Declined => ResourcePackClientStatus::Declined,
        ResourcePackStatusC2s::FailedDownload => ResourcePackClientStatus::FailedDownload,
        ResourcePackStatusC2s::SuccessfullyLoaded => ResourcePackClientStatus::SuccessfullyLoaded,
    }
}

fn non_empty_or_default<'a>(value: Option<&'a str>, default: &'a str) -> &'a str {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
}

fn parse_bool_or_default(value: Option<&str>, default: bool) -> bool {
    match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("1" | "true" | "yes" | "on") => true,
        Some("0" | "false" | "no" | "off") => false,
        Some(_) | None => default,
    }
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}

fn is_valid_sha1_hex(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::ResourcePackSendS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn setup_resource_pack_app(config: ResourcePackConfig) -> (App, MockClientHelper, Entity) {
        let mut app = App::new();
        app.insert_resource(config);
        app.insert_resource(ResourcePackStatusStore::default());
        app.add_systems(Update, prompt_resource_pack_on_join);
        let (bundle, helper) = create_mock_client("resourcepack-test");
        let client = app.world_mut().spawn(bundle).id();
        (app, helper, client)
    }

    fn flush_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
    }

    fn collect_resource_pack_prompts(helper: &mut MockClientHelper) -> Vec<(String, String, bool)> {
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                let packet = frame.decode::<ResourcePackSendS2c>().ok()?;
                Some((
                    packet.url.to_string(),
                    packet.hash.to_string(),
                    packet.forced,
                ))
            })
            .collect()
    }

    #[test]
    fn default_config_prompts_with_committed_sha1_and_degraded_decline_policy() {
        let config = resolve_resource_pack_config(None, None, None, None);
        let prompt = config.packs.first().expect("default resource pack prompt");
        assert_eq!(prompt.url, DEFAULT_RESOURCE_PACK_URL);
        assert!(!prompt.url.contains("/main/"));
        assert_eq!(prompt.sha1, DEFAULT_RESOURCE_PACK_SHA1);
        assert!(!prompt.forced, "decline should degrade instead of kicking");
        assert_eq!(
            config.packs.len(),
            1,
            "default should push exactly one full pack"
        );
        assert_eq!(config.packs[0].id, "bong-full-v1");
        assert_eq!(config.packs[0].version, "v1");
    }

    #[test]
    fn committed_manifest_matches_default_constants() {
        let manifest: Value =
            serde_json::from_str(include_str!("../../../client/resourcepack/manifest.json"))
                .expect("committed manifest should parse as JSON");
        assert_eq!(manifest["name"], DEFAULT_RESOURCE_PACK_MANIFEST.name);
        assert_eq!(manifest["version"], DEFAULT_RESOURCE_PACK_MANIFEST.version);
        assert_eq!(manifest["file"], DEFAULT_RESOURCE_PACK_MANIFEST.file);
        assert_eq!(manifest["sha1"], DEFAULT_RESOURCE_PACK_MANIFEST.sha1);
        assert_eq!(manifest["size"], DEFAULT_RESOURCE_PACK_MANIFEST.size);
    }

    #[test]
    fn disabled_env_suppresses_prompt() {
        let config = resolve_resource_pack_config(Some("false"), None, None, None);
        assert!(config.packs.is_empty());
        assert_eq!(
            config.disabled_reason.as_deref(),
            Some("BONG_RESOURCE_PACK_ENABLED=false")
        );
    }

    #[test]
    fn invalid_sha1_suppresses_prompt() {
        let config = resolve_resource_pack_config(None, None, Some("not-a-sha1"), None);
        assert!(config.packs.is_empty());
        assert_eq!(
            config.disabled_reason.as_deref(),
            Some("invalid BONG_RESOURCE_PACK_SHA1")
        );
    }

    #[test]
    fn env_can_override_url_sha1_and_forced_flag() {
        let config = resolve_resource_pack_config(
            Some("true"),
            Some("https://example.invalid/bong-full-v1.zip"),
            Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
            Some("true"),
        );
        let prompt = config.packs.first().expect("prompt should be configured");
        assert_eq!(prompt.url, "https://example.invalid/bong-full-v1.zip");
        assert_eq!(prompt.sha1, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert!(prompt.forced);
    }

    #[test]
    fn join_hook_sends_resource_pack_prompt_with_url_and_sha1() {
        let config = resolve_resource_pack_config(
            Some("true"),
            Some("https://example.invalid/bong-full-v1.zip"),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            Some("false"),
        );
        let (mut app, mut helper, _) = setup_resource_pack_app(config);

        app.update();
        flush_packets(&mut app);

        let prompts = collect_resource_pack_prompts(&mut helper);
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].0, "https://example.invalid/bong-full-v1.zip");
        assert_eq!(prompts[0].1, "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        assert!(!prompts[0].2, "decline should not be forced by default");
    }

    #[test]
    fn join_hook_sends_packs_in_config_order() {
        let config = ResourcePackConfig {
            packs: vec![
                ResourcePackEntry {
                    id: "first".to_string(),
                    version: "v1".to_string(),
                    url: "https://example.invalid/first.zip".to_string(),
                    sha1: "1111111111111111111111111111111111111111".to_string(),
                    forced: false,
                    prompt: "first".to_string(),
                },
                ResourcePackEntry {
                    id: "second".to_string(),
                    version: "v1".to_string(),
                    url: "https://example.invalid/second.zip".to_string(),
                    sha1: "2222222222222222222222222222222222222222".to_string(),
                    forced: true,
                    prompt: "second".to_string(),
                },
            ],
            disabled_reason: None,
        };
        let (mut app, mut helper, _) = setup_resource_pack_app(config);

        app.update();
        flush_packets(&mut app);

        let prompts = collect_resource_pack_prompts(&mut helper);
        assert_eq!(prompts.len(), 2);
        assert_eq!(prompts[0].0, "https://example.invalid/first.zip");
        assert_eq!(prompts[0].1, "1111111111111111111111111111111111111111");
        assert!(!prompts[0].2);
        assert_eq!(prompts[1].0, "https://example.invalid/second.zip");
        assert_eq!(prompts[1].1, "2222222222222222222222222222222222222222");
        assert!(prompts[1].2);
    }

    #[test]
    fn valence_status_maps_to_recorded_status() {
        assert_eq!(
            status_from_valence(ResourcePackStatusC2s::Accepted),
            ResourcePackClientStatus::Accepted
        );
        assert_eq!(
            status_from_valence(ResourcePackStatusC2s::Declined),
            ResourcePackClientStatus::Declined
        );
        assert_eq!(
            status_from_valence(ResourcePackStatusC2s::FailedDownload),
            ResourcePackClientStatus::FailedDownload
        );
        assert_eq!(
            status_from_valence(ResourcePackStatusC2s::SuccessfullyLoaded),
            ResourcePackClientStatus::SuccessfullyLoaded
        );
    }

    #[test]
    fn successfully_loaded_pack_records_accepted_version() {
        let client = Entity::from_raw(9);
        let pack = resolve_resource_pack_config(None, None, None, None)
            .packs
            .pop()
            .expect("default pack");
        let mut store = ResourcePackStatusStore::default();

        store.record_prompted(client, &pack);
        assert_eq!(
            store.record(client, ResourcePackClientStatus::SuccessfullyLoaded),
            None
        );

        assert_eq!(
            store.get(client),
            Some(ResourcePackClientStatus::SuccessfullyLoaded)
        );
        assert_eq!(store.accepted_version(client, pack.id.as_str()), Some("v1"));
        assert!(
            !store.should_prompt(client, &pack),
            "same accepted version should not be prompted again in the same server session"
        );
    }

    #[test]
    fn same_accepted_version_join_does_not_reprompt() {
        let config = resolve_resource_pack_config(None, None, None, None);
        let pack = config.packs[0].clone();
        let (mut app, mut helper, client) = setup_resource_pack_app(config);
        app.world_mut()
            .resource_mut::<ResourcePackStatusStore>()
            .record_loaded_for_pack(client, &pack);

        app.update();
        flush_packets(&mut app);

        let prompts = collect_resource_pack_prompts(&mut helper);
        assert!(
            prompts.is_empty(),
            "accepted version {} should not be re-prompted, actual prompts={prompts:?}",
            pack.version
        );
    }

    #[test]
    fn upgraded_version_prompts_even_when_previous_version_was_accepted() {
        let mut config = resolve_resource_pack_config(None, None, None, None);
        let previous = config.packs[0].clone();
        config.packs[0].id = "bong-full-v2".to_string();
        config.packs[0].version = "v2".to_string();
        config.packs[0].url = "https://example.invalid/bong-full-v2.zip".to_string();
        config.packs[0].sha1 = "3333333333333333333333333333333333333333".to_string();
        let upgraded = config.packs[0].clone();
        let (mut app, mut helper, client) = setup_resource_pack_app(config);
        app.world_mut()
            .resource_mut::<ResourcePackStatusStore>()
            .record_loaded_for_pack(client, &previous);

        app.update();
        flush_packets(&mut app);

        let prompts = collect_resource_pack_prompts(&mut helper);
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].0, upgraded.url);
        assert_eq!(prompts[0].1, upgraded.sha1);
    }

    #[test]
    fn same_id_different_version_prompts_again() {
        let mut config = resolve_resource_pack_config(None, None, None, None);
        let previous = config.packs[0].clone();
        config.packs[0].version = "v2".to_string();
        config.packs[0].url = "https://example.invalid/bong-full-v2.zip".to_string();
        config.packs[0].sha1 = "5555555555555555555555555555555555555555".to_string();
        let upgraded = config.packs[0].clone();
        let (mut app, mut helper, client) = setup_resource_pack_app(config);
        app.world_mut()
            .resource_mut::<ResourcePackStatusStore>()
            .record_loaded_for_pack(client, &previous);

        app.update();
        flush_packets(&mut app);

        let prompts = collect_resource_pack_prompts(&mut helper);
        assert_eq!(
            prompts.len(),
            1,
            "same pack id should re-prompt when version changes from {} to {}",
            previous.version,
            upgraded.version
        );
        assert_eq!(prompts[0].0, upgraded.url);
        assert_eq!(prompts[0].1, upgraded.sha1);
    }

    #[test]
    fn changed_sha1_for_same_version_prompts_again() {
        let mut config = resolve_resource_pack_config(None, None, None, None);
        let previous = config.packs[0].clone();
        config.packs[0].sha1 = "4444444444444444444444444444444444444444".to_string();
        let upgraded = config.packs[0].clone();
        let (mut app, mut helper, client) = setup_resource_pack_app(config);
        app.world_mut()
            .resource_mut::<ResourcePackStatusStore>()
            .record_loaded_for_pack(client, &previous);

        app.update();
        flush_packets(&mut app);

        let prompts = collect_resource_pack_prompts(&mut helper);
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].1, upgraded.sha1);
    }

    #[test]
    fn declined_or_failed_download_does_not_mark_version_accepted() {
        let client = Entity::from_raw(11);
        let pack = resolve_resource_pack_config(None, None, None, None)
            .packs
            .pop()
            .expect("default pack");
        let mut declined = ResourcePackStatusStore::default();
        declined.record_prompted(client, &pack);
        declined.record(client, ResourcePackClientStatus::Declined);
        assert_eq!(declined.accepted_version(client, pack.id.as_str()), None);
        assert!(declined.should_prompt(client, &pack));

        let mut failed = ResourcePackStatusStore::default();
        failed.record_prompted(client, &pack);
        failed.record(client, ResourcePackClientStatus::FailedDownload);
        assert_eq!(failed.accepted_version(client, pack.id.as_str()), None);
        assert!(failed.should_prompt(client, &pack));
    }

    #[test]
    fn disconnected_client_cleans_resource_pack_session() {
        let config = resolve_resource_pack_config(None, None, None, None);
        let pack = config.packs[0].clone();
        let mut app = App::new();
        app.insert_resource(ResourcePackStatusStore::default());
        app.add_systems(Update, cleanup_disconnected_resource_pack_sessions);
        let (bundle, _helper) = create_mock_client("resourcepack-disconnect-test");
        let client = app.world_mut().spawn(bundle).id();
        app.world_mut()
            .resource_mut::<ResourcePackStatusStore>()
            .record_loaded_for_pack(client, &pack);

        app.world_mut().despawn(client);
        app.update();

        let store = app.world().resource::<ResourcePackStatusStore>();
        assert_eq!(
            store.accepted_version(client, pack.id.as_str()),
            None,
            "resource pack session for disconnected client {client:?} should be removed"
        );
    }

    #[test]
    fn declined_status_is_recorded_without_requiring_kick_path() {
        let client = Entity::from_raw(7);
        let mut store = ResourcePackStatusStore::default();
        assert_eq!(
            store.record(client, ResourcePackClientStatus::Declined),
            None
        );
        assert_eq!(store.get(client), Some(ResourcePackClientStatus::Declined));
        assert_eq!(
            resource_pack_event_name(ResourcePackClientStatus::Declined),
            "resource_pack_declined"
        );
    }
}
