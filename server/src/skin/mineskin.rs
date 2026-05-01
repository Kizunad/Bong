use std::time::Duration;

use reqwest::{header::USER_AGENT, StatusCode};
use serde::Deserialize;
use tokio::time::sleep;

use super::{SignedSkin, SkinSource};

const DEFAULT_BASE_URL: &str = "https://api.mineskin.org";
const DEFAULT_RETRY_COUNT: usize = 3;
const MINESKIN_USER_AGENT: &str = concat!("Bong/", env!("CARGO_PKG_VERSION"));

#[derive(Clone, Debug)]
pub struct MineSkinClient {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl MineSkinClient {
    pub fn from_env() -> Result<Self, MineSkinUnavailable> {
        let api_key = std::env::var("MINESKIN_API_KEY")
            .ok()
            .filter(|key| !key.trim().is_empty());
        match api_key {
            Some(api_key) => Ok(Self::new(DEFAULT_BASE_URL, Some(api_key))),
            None => Err(MineSkinUnavailable::MissingApiKey),
        }
    }

    pub fn new(base_url: impl Into<String>, api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key,
        }
    }

    pub async fn fetch_random(&self, count: usize) -> Result<Vec<SignedSkin>, MineSkinError> {
        self.fetch_random_with_retry(count, DEFAULT_RETRY_COUNT)
            .await
    }

    async fn fetch_random_with_retry(
        &self,
        count: usize,
        attempts: usize,
    ) -> Result<Vec<SignedSkin>, MineSkinError> {
        if count == 0 {
            return Ok(Vec::new());
        }

        let attempts = attempts.max(1);
        let mut last_error = None;

        for attempt in 0..attempts {
            match self.fetch_random_once(count).await {
                Ok(skins) => return Ok(skins),
                Err(error) => {
                    last_error = Some(error);
                    if attempt + 1 < attempts {
                        sleep(backoff_delay(attempt)).await;
                    }
                }
            }
        }

        Err(last_error.expect("at least one attempt must run"))
    }

    async fn fetch_random_once(&self, count: usize) -> Result<Vec<SignedSkin>, MineSkinError> {
        let size = count.min(100);
        let response = self
            .get(format!("{}/v2/skins", self.base_url))
            .query(&[("size", size.to_string())])
            .send()
            .await
            .map_err(MineSkinError::Http)?;
        let status = response.status();
        if !status.is_success() {
            return Err(MineSkinError::Status(status));
        }

        let payload: MineSkinListResponse = response.json().await.map_err(MineSkinError::Http)?;
        let mut detail_refs = Vec::new();
        let mut skins = Vec::new();
        for entry in payload.skins {
            let lookup_id = entry.lookup_id();
            match entry.into_signed_skin() {
                Some(skin) => skins.push(skin),
                None => {
                    if let Some(lookup_id) = lookup_id {
                        detail_refs.push(lookup_id);
                    }
                }
            }
        }

        let mut last_error = None;
        for lookup_id in detail_refs {
            match self.fetch_skin_detail(lookup_id.as_str()).await {
                Ok(Some(skin)) => skins.push(skin),
                Ok(None) => {}
                Err(error) => last_error = Some(error),
            }
        }

        if skins.is_empty() {
            if let Some(error) = last_error {
                return Err(error);
            }
        }

        Ok(skins)
    }

    async fn fetch_skin_detail(
        &self,
        lookup_id: &str,
    ) -> Result<Option<SignedSkin>, MineSkinError> {
        let response = self
            .get(format!("{}/v2/skins/{lookup_id}", self.base_url))
            .send()
            .await
            .map_err(MineSkinError::Http)?;
        let status = response.status();
        if !status.is_success() {
            return Err(MineSkinError::Status(status));
        }

        let payload: MineSkinDetailResponse = response.json().await.map_err(MineSkinError::Http)?;
        Ok(payload.into_signed_skin())
    }

    fn get(&self, url: String) -> reqwest::RequestBuilder {
        let request = self.client.get(url).header(USER_AGENT, MINESKIN_USER_AGENT);
        if let Some(api_key) = &self.api_key {
            request.bearer_auth(api_key)
        } else {
            request
        }
    }
}

fn backoff_delay(attempt: usize) -> Duration {
    let millis = 200 + (attempt as u64 * 137) + (attempt as u64 * attempt as u64 * 83);
    Duration::from_millis(millis)
}

#[derive(Debug)]
pub enum MineSkinUnavailable {
    MissingApiKey,
}

impl std::fmt::Display for MineSkinUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingApiKey => write!(f, "MINESKIN_API_KEY missing"),
        }
    }
}

impl std::error::Error for MineSkinUnavailable {}

#[derive(Debug)]
pub enum MineSkinError {
    Http(reqwest::Error),
    Status(StatusCode),
}

impl std::fmt::Display for MineSkinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(error) => write!(f, "{error}"),
            Self::Status(status) => write!(f, "MineSkin returned HTTP {status}"),
        }
    }
}

impl std::error::Error for MineSkinError {}

#[derive(Debug, Deserialize)]
struct MineSkinListResponse {
    #[serde(default, alias = "data", alias = "items")]
    skins: Vec<MineSkinEntry>,
}

#[derive(Debug, Deserialize)]
struct MineSkinDetailResponse {
    #[serde(default)]
    skin: Option<MineSkinEntry>,
    #[serde(flatten)]
    entry: MineSkinEntry,
}

impl MineSkinDetailResponse {
    fn into_signed_skin(self) -> Option<SignedSkin> {
        self.skin
            .and_then(MineSkinEntry::into_signed_skin)
            .or_else(|| self.entry.into_signed_skin())
    }
}

#[derive(Debug, Deserialize)]
struct MineSkinEntry {
    #[serde(default)]
    uuid: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default, alias = "shortId")]
    short_id: Option<String>,
    #[serde(default)]
    hash: Option<String>,
    #[serde(default)]
    timestamp: Option<u64>,
    #[serde(default)]
    texture: Option<MineSkinTexture>,
    #[serde(default)]
    data: Option<MineSkinEntryData>,
}

impl MineSkinEntry {
    fn lookup_id(&self) -> Option<String> {
        self.uuid
            .clone()
            .or_else(|| self.short_id.clone())
            .or_else(|| self.id.clone())
    }

    fn into_signed_skin(self) -> Option<SignedSkin> {
        let texture_hash = self.texture.as_ref().and_then(MineSkinTexture::skin_hash);
        let property = self
            .texture
            .and_then(MineSkinTexture::into_data)
            .or_else(|| {
                self.data
                    .and_then(|data| data.texture)
                    .and_then(|texture| texture.value_signature())
            })?;

        Some(SignedSkin {
            value: property.value,
            signature: property.signature.unwrap_or_default(),
            source: SkinSource::MineSkinRandom {
                hash: texture_hash
                    .or(self.hash)
                    .or(self.id)
                    .or(self.short_id)
                    .or(self.uuid)
                    .unwrap_or_else(|| self.timestamp.unwrap_or_default().to_string()),
            },
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MineSkinTexture {
    Hash(String),
    Detail {
        #[serde(default)]
        data: Option<MineSkinProperty>,
        #[serde(default)]
        hash: Option<MineSkinTextureHash>,
    },
}

impl MineSkinTexture {
    fn skin_hash(&self) -> Option<String> {
        match self {
            Self::Hash(hash) => Some(hash.clone()),
            Self::Detail { hash, .. } => hash.as_ref().and_then(|hash| hash.skin.clone()),
        }
    }

    fn into_data(self) -> Option<MineSkinProperty> {
        match self {
            Self::Hash(_) => None,
            Self::Detail { data, .. } => data,
        }
    }
}

#[derive(Debug, Deserialize)]
struct MineSkinTextureHash {
    #[serde(default)]
    skin: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MineSkinEntryData {
    #[serde(default)]
    texture: Option<MineSkinTextureData>,
}

#[derive(Debug, Deserialize)]
struct MineSkinTextureData {
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    signature: Option<String>,
}

impl MineSkinTextureData {
    fn value_signature(self) -> Option<MineSkinProperty> {
        Some(MineSkinProperty {
            value: self.value?,
            signature: self.signature,
        })
    }
}

#[derive(Debug, Deserialize)]
struct MineSkinProperty {
    value: String,
    #[serde(default)]
    signature: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn fetch_random_maps_mineskin_payload_to_signed_skin() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/skins"))
            .and(query_param("size", "2"))
            .and(header("authorization", "Bearer test-key"))
            .and(header("user-agent", MINESKIN_USER_AGENT))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "skins": [
                    {"uuid": "skin-a", "shortId": "a", "texture": "hash-a", "timestamp": 1},
                    {"uuid": "skin-b", "shortId": "b", "texture": "hash-b", "timestamp": 2}
                ]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/skins/skin-a"))
            .and(header("authorization", "Bearer test-key"))
            .and(header("user-agent", MINESKIN_USER_AGENT))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "skin": {
                    "uuid": "skin-a",
                    "texture": {
                        "data": {"value": "value-a", "signature": "sig-a"},
                        "hash": {"skin": "hash-a"}
                    }
                }
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/skins/skin-b"))
            .and(header("authorization", "Bearer test-key"))
            .and(header("user-agent", MINESKIN_USER_AGENT))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "skin": {
                    "uuid": "skin-b",
                    "data": {"texture": {"value": "value-b", "signature": "sig-b"}}
                }
            })))
            .mount(&server)
            .await;

        let client = MineSkinClient::new(server.uri(), Some("test-key".to_string()));
        let skins = client.fetch_random_with_retry(2, 1).await.unwrap();

        assert_eq!(skins.len(), 2);
        assert_eq!(skins[0].value, "value-a");
        assert_eq!(skins[0].signature, "sig-a");
        assert_eq!(
            skins[0].source,
            SkinSource::MineSkinRandom {
                hash: "hash-a".into()
            }
        );
        assert_eq!(skins[1].value, "value-b");
        assert_eq!(skins[1].signature, "sig-b");
    }

    #[tokio::test]
    async fn fetch_random_retries_transient_status() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/skins"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/skins"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "skins": [{"uuid": "skin-ok", "texture": "hash-ok"}]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/skins/skin-ok"))
            .and(header("user-agent", MINESKIN_USER_AGENT))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "skin": {"uuid": "skin-ok", "texture": {"data": {"value": "value-ok", "signature": "sig-ok"}}}
            })))
            .mount(&server)
            .await;

        let client = MineSkinClient::new(server.uri(), None);
        let skins = client.fetch_random_with_retry(1, 2).await.unwrap();

        assert_eq!(skins.len(), 1);
        assert_eq!(skins[0].value, "value-ok");
    }
}
