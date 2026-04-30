use std::collections::HashMap;

use valence::message::SendMessage;
use valence::prelude::{Added, Client, Entity, EventReader, Query, Res, ResMut, Resource, Text};
use valence::protocol::packets::play::ResourcePackStatusC2s;
use valence::resource_pack::ResourcePackStatusEvent;

pub const RESOURCE_PACK_ENABLED_ENV: &str = "BONG_RESOURCE_PACK_ENABLED";
pub const RESOURCE_PACK_URL_ENV: &str = "BONG_RESOURCE_PACK_URL";
pub const RESOURCE_PACK_SHA1_ENV: &str = "BONG_RESOURCE_PACK_SHA1";
pub const RESOURCE_PACK_FORCED_ENV: &str = "BONG_RESOURCE_PACK_FORCED";

pub const DEFAULT_RESOURCE_PACK_URL: &str =
    "https://raw.githubusercontent.com/Kizunad/Bong/6b39e905d97e5f2371e69ff5cf5daf5b54d1f255/client/resourcepack/bong-mineral-v1.zip";
pub const DEFAULT_RESOURCE_PACK_SHA1: &str = "3723e0156118023c9206d7605666bb90b23bc10d";
const DEFAULT_RESOURCE_PACK_PROMPT: &str =
    "Bong 矿物贴图资源包；拒绝后仍可游玩，但矿石显示为 vanilla 贴图。";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourcePackPromptConfig {
    pub url: String,
    pub sha1: String,
    pub forced: bool,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourcePackConfig {
    pub prompt: Option<ResourcePackPromptConfig>,
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
    statuses: HashMap<Entity, ResourcePackClientStatus>,
}

impl Resource for ResourcePackStatusStore {}

impl ResourcePackStatusStore {
    #[cfg(test)]
    pub fn get(&self, client: Entity) -> Option<ResourcePackClientStatus> {
        self.statuses.get(&client).copied()
    }

    fn record(
        &mut self,
        client: Entity,
        status: ResourcePackClientStatus,
    ) -> Option<ResourcePackClientStatus> {
        self.statuses.insert(client, status)
    }
}

pub fn prompt_resource_pack_on_join(
    config: Res<ResourcePackConfig>,
    mut joined_clients: Query<(Entity, &mut Client), Added<Client>>,
) {
    let Some(prompt) = &config.prompt else {
        return;
    };

    for (entity, mut client) in &mut joined_clients {
        client.set_resource_pack(
            prompt.url.as_str(),
            prompt.sha1.as_str(),
            prompt.forced,
            Some(Text::text(prompt.prompt.clone())),
        );
        tracing::info!(
            "[bong][resourcepack] prompted client entity {entity:?} with mineral pack {}",
            prompt.sha1,
        );
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
                "[bong][resourcepack] client {:?} accepted mineral pack",
                event.client
            ),
            ResourcePackClientStatus::SuccessfullyLoaded => tracing::info!(
                "[bong][resourcepack] client {:?} loaded mineral pack",
                event.client
            ),
            ResourcePackClientStatus::Declined => {
                tracing::warn!(
                    "[bong][resourcepack] client {:?} declined mineral pack; continuing degraded",
                    event.client
                );
                if let Ok(mut client) = clients.get_mut(event.client) {
                    client
                        .send_chat_message("已降级为 vanilla 矿石贴图；矿物 tooltip 与玩法仍正常。")
                }
            }
            ResourcePackClientStatus::FailedDownload => {
                tracing::warn!(
                    "[bong][resourcepack] client {:?} failed to download mineral pack; continuing degraded",
                    event.client
                );
                if let Ok(mut client) = clients.get_mut(event.client) {
                    client.send_chat_message("矿物资源包下载失败，已降级为 vanilla 贴图。")
                }
            }
        }
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
            prompt: None,
            disabled_reason: Some(format!("{RESOURCE_PACK_ENABLED_ENV}=false")),
        };
    }

    let url = non_empty_or_default(url, DEFAULT_RESOURCE_PACK_URL);
    if !is_http_url(url) {
        return ResourcePackConfig {
            prompt: None,
            disabled_reason: Some(format!("invalid {RESOURCE_PACK_URL_ENV}")),
        };
    }

    let sha1 = non_empty_or_default(sha1, DEFAULT_RESOURCE_PACK_SHA1);
    if !is_valid_sha1_hex(sha1) {
        return ResourcePackConfig {
            prompt: None,
            disabled_reason: Some(format!("invalid {RESOURCE_PACK_SHA1_ENV}")),
        };
    }

    ResourcePackConfig {
        prompt: Some(ResourcePackPromptConfig {
            url: url.to_string(),
            sha1: sha1.to_ascii_lowercase(),
            forced: parse_bool_or_default(forced, false),
            prompt: DEFAULT_RESOURCE_PACK_PROMPT.to_string(),
        }),
        disabled_reason: None,
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

    #[test]
    fn default_config_prompts_with_committed_sha1_and_degraded_decline_policy() {
        let config = resolve_resource_pack_config(None, None, None, None);
        let prompt = config.prompt.expect("default resource pack prompt");
        assert_eq!(prompt.url, DEFAULT_RESOURCE_PACK_URL);
        assert!(!prompt.url.contains("/main/"));
        assert_eq!(prompt.sha1, DEFAULT_RESOURCE_PACK_SHA1);
        assert!(!prompt.forced, "decline should degrade instead of kicking");
    }

    #[test]
    fn committed_sha1_sidecar_matches_default_constant() {
        let sidecar = include_str!("../../../client/resourcepack/bong-mineral-v1.zip.sha1").trim();
        assert_eq!(sidecar, DEFAULT_RESOURCE_PACK_SHA1);
    }

    #[test]
    fn disabled_env_suppresses_prompt() {
        let config = resolve_resource_pack_config(Some("false"), None, None, None);
        assert!(config.prompt.is_none());
        assert_eq!(
            config.disabled_reason.as_deref(),
            Some("BONG_RESOURCE_PACK_ENABLED=false")
        );
    }

    #[test]
    fn invalid_sha1_suppresses_prompt() {
        let config = resolve_resource_pack_config(None, None, Some("not-a-sha1"), None);
        assert!(config.prompt.is_none());
        assert_eq!(
            config.disabled_reason.as_deref(),
            Some("invalid BONG_RESOURCE_PACK_SHA1")
        );
    }

    #[test]
    fn env_can_override_url_sha1_and_forced_flag() {
        let config = resolve_resource_pack_config(
            Some("true"),
            Some("https://example.invalid/bong-mineral-v1.zip"),
            Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
            Some("true"),
        );
        let prompt = config.prompt.expect("prompt should be configured");
        assert_eq!(prompt.url, "https://example.invalid/bong-mineral-v1.zip");
        assert_eq!(prompt.sha1, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert!(prompt.forced);
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
    fn declined_status_is_recorded_without_requiring_kick_path() {
        let client = Entity::from_raw(7);
        let mut store = ResourcePackStatusStore::default();
        assert_eq!(
            store.record(client, ResourcePackClientStatus::Declined),
            None
        );
        assert_eq!(store.get(client), Some(ResourcePackClientStatus::Declined));
    }
}
