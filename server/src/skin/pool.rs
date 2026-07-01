use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use valence::client::ClientMarker;
use valence::layer::UpdateLayersPreClientSet;
use valence::prelude::{
    bevy_ecs, Added, App, Client, Component, Entity, IntoSystemConfigs, PostUpdate, Query, ResMut,
    Resource, Uuid, With, Without,
};

use super::mineskin::MineSkinClient;
use super::npc_skin_selector::{NpcSkinPoolKey, NpcVisualProfile};
use super::{packet, SignedSkin};
use crate::npc::brain::canonical_npc_id;

pub const MIN_READY_BEFORE_SPAWN: usize = 5;
const PREFETCH_TARGET_PER_POOL_KEY: usize = 8;
const REFILL_THRESHOLD: usize = 5;
const PREFETCH_TIMEOUT: Duration = Duration::from_secs(30);
const NPC_UUID_NAMESPACE: Uuid = Uuid::from_u128(0x426f_6e67_4e50_4353_6b69_6e56_3101);

#[derive(Clone, Debug, Component, PartialEq, Eq)]
pub struct NpcPlayerSkin {
    pub uuid: Uuid,
    pub name: String,
    pub skin: SignedSkin,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NpcSkinFallbackPolicy {
    #[default]
    WaitForReady,
    AllowFallback,
}

pub struct SkinPool {
    by_pool_key: HashMap<NpcSkinPoolKey, SkinBucket>,
    failover: VecDeque<SignedSkin>,
    receiver: Receiver<SkinFetchResult>,
    sender: Sender<SkinFetchResult>,
    inflight: HashSet<NpcSkinPoolKey>,
    started_prefetch: bool,
    skip_prefetch: bool,
    ready_deadline: Instant,
    request_generation: AtomicU64,
}

impl Resource for SkinPool {}

impl Default for SkinPool {
    fn default() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        let skip_prefetch = std::env::var("BONG_SKIP_SKIN_PREFETCH")
            .map(|v| v == "1")
            .unwrap_or(false);
        Self {
            by_pool_key: HashMap::new(),
            failover: VecDeque::new(),
            receiver,
            sender,
            inflight: HashSet::new(),
            started_prefetch: false,
            skip_prefetch,
            ready_deadline: Instant::now() + PREFETCH_TIMEOUT,
            request_generation: AtomicU64::new(0),
        }
    }
}

impl SkinPool {
    pub fn insert_for_key(&mut self, key: NpcSkinPoolKey, skin: SignedSkin) {
        self.by_pool_key
            .entry(key)
            .or_default()
            .skins
            .push_back(skin);
    }

    pub fn len_for_key(&self, key: NpcSkinPoolKey) -> usize {
        self.by_pool_key
            .get(&key)
            .map_or(0, |bucket| bucket.skins.len())
    }

    pub fn ready_count(&self) -> usize {
        NpcSkinPoolKey::PREFETCH_KEYS
            .into_iter()
            .map(|key| self.len_for_key(key))
            .sum()
    }

    pub fn ready_for_spawn(&self) -> bool {
        self.ready_count() >= MIN_READY_BEFORE_SPAWN
            && NpcSkinPoolKey::PREFETCH_KEYS
                .into_iter()
                .all(|key| self.len_for_key(key) > 0)
    }

    pub fn next_for_profile(&mut self, profile: NpcVisualProfile, salt: u64) -> SignedSkin {
        self.next_for_key(profile.skin_pool_key(), salt)
    }

    fn next_for_key(&mut self, key: NpcSkinPoolKey, salt: u64) -> SignedSkin {
        self.drain_ready();
        if let Some(bucket) = self.by_pool_key.get_mut(&key) {
            if let Some(skin) = bucket.next(salt) {
                return skin;
            }
        }

        if !self.failover.is_empty() {
            let index = salt as usize % self.failover.len();
            if let Some(skin) = self.failover.remove(index) {
                self.failover.push_back(skin.clone());
                return skin;
            }
        }

        tracing::error!(
            "[bong][skin] SkinPool exhausted for key {} — no skins available and no fallback",
            key.as_str()
        );
        panic!("[bong][skin] SkinPool exhausted: MINESKIN_API_KEY set? MineSkin API reachable?")
    }

    pub fn drain_ready(&mut self) {
        while let Ok(result) = self.receiver.try_recv() {
            match result {
                SkinFetchResult::Ready { key, skins } => {
                    self.inflight.remove(&key);
                    for skin in skins {
                        self.insert_for_key(key, skin);
                    }
                }
                SkinFetchResult::Failed { key, error } => {
                    self.inflight.remove(&key);
                    tracing::error!(
                        "[bong][skin] MineSkin fetch failed for pool {} (error={error})",
                        key.as_str()
                    );
                }
            }
        }
    }

    fn start_prefetch_if_needed(&mut self) {
        if self.started_prefetch {
            return;
        }
        if self.skip_prefetch {
            tracing::warn!(
                "[bong][skin] BONG_SKIP_SKIN_PREFETCH=1 — skin prefetch disabled, NPCs will use villager fallback"
            );
            self.started_prefetch = true;
            return;
        }
        self.started_prefetch = true;
        self.ready_deadline = Instant::now() + PREFETCH_TIMEOUT;

        let client = match MineSkinClient::from_env() {
            Ok(client) => client,
            Err(error) => {
                panic!(
                    "[bong][skin] MineSkin unavailable (error={error}). Set MINESKIN_API_KEY in server/.env"
                );
            }
        };

        let keys: Vec<_> = NpcSkinPoolKey::PREFETCH_KEYS
            .into_iter()
            .filter(|key| self.inflight.insert(*key))
            .collect();
        self.spawn_fetch_concurrent(keys, PREFETCH_TARGET_PER_POOL_KEY, client);
    }

    fn maybe_mark_timeout(&mut self) {
        if self.started_prefetch
            && !self.skip_prefetch
            && self.ready_count() < MIN_READY_BEFORE_SPAWN
            && Instant::now() >= self.ready_deadline
        {
            panic!(
                "[bong][skin] MineSkin prefetch timed out before {MIN_READY_BEFORE_SPAWN} skins (got {}). Check MINESKIN_API_KEY and network.",
                self.ready_count()
            );
        }
    }

    fn maybe_refill(&mut self) {
        if self.skip_prefetch {
            return;
        }
        let keys: Vec<_> = NpcSkinPoolKey::PREFETCH_KEYS
            .into_iter()
            .filter(|key| {
                self.len_for_key(*key) <= REFILL_THRESHOLD && !self.inflight.contains(key)
            })
            .collect();
        if keys.is_empty() {
            return;
        }
        for key in &keys {
            self.inflight.insert(*key);
        }
        if let Ok(client) = MineSkinClient::from_env() {
            self.spawn_fetch_concurrent(keys, PREFETCH_TARGET_PER_POOL_KEY, client);
        } else {
            for key in &keys {
                self.inflight.remove(key);
            }
        }
    }

    /// F11 — 并发抓取每个 pool key（沿用 `redis_bridge::spawn_redis_bridge` 的
    /// 单线程 + 独立 tokio Runtime 模式）。旧实现在 runtime 内用 `for key in keys`
    /// 顺序 `block_on`，一个 key 被限速/重试会阻塞同批后续所有 key；这里改用
    /// `futures_util::future::join_all` 把所有 key 的 fetch 并发 join，仍逐 key
    /// 通过既有 crossbeam channel 发回 Ready/Failed —— 不改外部 channel 语义。
    fn spawn_fetch_concurrent(
        &mut self,
        keys: Vec<NpcSkinPoolKey>,
        count: usize,
        client: MineSkinClient,
    ) {
        if keys.is_empty() {
            return;
        }
        let sender = self.sender.clone();
        let fail_sender = self.sender.clone();
        let fail_keys = keys.clone();
        let request_id = self.request_generation.fetch_add(1, Ordering::Relaxed);
        std::thread::Builder::new()
            .name(format!("bong-skin-prefetch-{request_id}"))
            .spawn(move || {
                let runtime = match tokio::runtime::Runtime::new() {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        for key in &keys {
                            let _ = sender.send(SkinFetchResult::Failed {
                                key: *key,
                                error: format!("tokio runtime: {error}"),
                            });
                        }
                        return;
                    }
                };

                let results = runtime.block_on(async {
                    let client_ref = &client;
                    futures_util::future::join_all(
                        keys.iter()
                            .map(|key| async move { (*key, client_ref.fetch_random(count).await) }),
                    )
                    .await
                });

                for (key, result) in results {
                    match result {
                        Ok(skins) => {
                            let _ = sender.send(SkinFetchResult::Ready { key, skins });
                        }
                        Err(error) => {
                            let _ = sender.send(SkinFetchResult::Failed {
                                key,
                                error: error.to_string(),
                            });
                        }
                    }
                }
            })
            .map(std::mem::drop)
            .unwrap_or_else(|error| {
                for key in fail_keys {
                    let _ = fail_sender.send(SkinFetchResult::Failed {
                        key,
                        error: format!("thread spawn: {error}"),
                    });
                }
            });
    }
}

#[derive(Default)]
struct SkinBucket {
    skins: VecDeque<SignedSkin>,
    cursor: usize,
}

impl SkinBucket {
    fn next(&mut self, salt: u64) -> Option<SignedSkin> {
        if self.skins.is_empty() {
            return None;
        }
        let index = (self.cursor + salt as usize) % self.skins.len();
        self.cursor = (self.cursor + 1) % self.skins.len();
        self.skins.get(index).cloned()
    }
}

#[derive(Debug)]
enum SkinFetchResult {
    Ready {
        key: NpcSkinPoolKey,
        skins: Vec<SignedSkin>,
    },
    Failed {
        key: NpcSkinPoolKey,
        error: String,
    },
}

pub fn npc_uuid(entity: Entity) -> Uuid {
    Uuid::new_v5(&NPC_UUID_NAMESPACE, canonical_npc_id(entity).as_bytes())
}

pub fn register(app: &mut App) {
    app.insert_resource(SkinPool::default()).add_systems(
        PostUpdate,
        (
            maintain_skin_pool,
            send_skin_catchup_to_new_client,
            broadcast_skin_add_for_new_npcs,
            broadcast_skin_remove_for_despawned_npcs,
        )
            .before(UpdateLayersPreClientSet),
    );
}

fn maintain_skin_pool(mut pool: ResMut<SkinPool>) {
    pool.start_prefetch_if_needed();
    pool.drain_ready();
    pool.maybe_mark_timeout();
    pool.maybe_refill();
}

fn send_skin_catchup_to_new_client(
    mut clients: Query<&mut Client, Added<ClientMarker>>,
    npcs: Query<&NpcPlayerSkin, Without<ClientMarker>>,
) {
    for mut client in &mut clients {
        for npc_skin in &npcs {
            packet::send_add_player(
                &mut client,
                npc_skin.uuid,
                npc_skin.name.as_str(),
                &npc_skin.skin,
            );
        }
    }
}

fn broadcast_skin_add_for_new_npcs(
    new_npcs: Query<&NpcPlayerSkin, Added<NpcPlayerSkin>>,
    mut clients: Query<&mut Client, With<ClientMarker>>,
) {
    for npc_skin in &new_npcs {
        packet::broadcast_add_player(
            clients.iter_mut(),
            npc_skin.uuid,
            npc_skin.name.as_str(),
            &npc_skin.skin,
        );
    }
}

fn broadcast_skin_remove_for_despawned_npcs(
    despawned_npcs: Query<
        &NpcPlayerSkin,
        (With<valence::prelude::Despawned>, Without<ClientMarker>),
    >,
    mut clients: Query<&mut Client, With<ClientMarker>>,
) {
    for npc_skin in &despawned_npcs {
        packet::broadcast_remove_player(clients.iter_mut(), npc_skin.uuid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::npc::lifecycle::NpcArchetype;
    use crate::skin::npc_skin_selector::{NpcSkinTier, NpcVisualProfile};
    use crate::skin::{SignedSkin, SkinSource};
    use std::collections::HashSet;
    use std::time::{Duration, Instant};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn skin(value: &str) -> SignedSkin {
        SignedSkin {
            value: value.to_string(),
            signature: format!("sig-{value}"),
            source: SkinSource::MineSkinRandom { hash: value.into() },
        }
    }

    fn profile(key: NpcSkinPoolKey) -> NpcVisualProfile {
        NpcVisualProfile {
            archetype: NpcArchetype::Rogue,
            skin_tier: key.0,
            skin_pool_key: key,
            age_band: crate::skin::npc_skin_selector::NpcAgeBand::Adult,
            high_realm: matches!(key.0, NpcSkinTier::RogueHigh | NpcSkinTier::DiscipleHigh),
            faction_id: None,
            faction_rank: None,
        }
    }

    #[test]
    #[should_panic(expected = "SkinPool exhausted")]
    fn next_for_empty_pool_panics() {
        let mut pool = SkinPool::default();
        let _skin = pool.next_for_profile(profile(NpcSkinPoolKey(NpcSkinTier::RogueLow)), 0);
    }

    #[test]
    fn next_for_round_robins_bucket_with_salt() {
        let mut pool = SkinPool::default();
        let key = NpcSkinPoolKey(NpcSkinTier::RogueLow);
        pool.insert_for_key(key, skin("a"));
        pool.insert_for_key(key, skin("b"));
        pool.insert_for_key(key, skin("c"));

        assert_eq!(pool.next_for_profile(profile(key), 0).value, "a");
        assert_eq!(pool.next_for_profile(profile(key), 0).value, "b");
        assert_eq!(pool.next_for_profile(profile(key), 1).value, "a");
        assert_eq!(pool.len_for_key(key), 3);
    }

    #[test]
    fn skip_prefetch_keeps_pool_unready() {
        let mut pool = SkinPool {
            skip_prefetch: true,
            ..Default::default()
        };
        pool.start_prefetch_if_needed();
        assert!(
            pool.started_prefetch,
            "started_prefetch should be set even when skipping"
        );
        assert!(
            !pool.ready_for_spawn(),
            "pool should not be ready when prefetch is skipped"
        );
    }

    #[test]
    fn npc_uuid_is_stable_for_same_entity_bits() {
        let entity = Entity::from_bits(0x0000_0004_0000_002a);

        assert_eq!(npc_uuid(entity), npc_uuid(entity));
        assert_ne!(npc_uuid(entity), Uuid::nil());
    }

    // F11 — `spawn_fetch_concurrent` pin 测试。MineSkin 的 `/v2/skins` 端点本身
    // 不区分 pool key（key 只是我们内部的分桶概念），所以这些测试锁"整批完成"的
    // 聚合契约（每个 key 都有且仅有一个结果、失败不吞掉其它 key、并发而非串行）
    // 而非某个具体 key 的结果内容。

    fn success_body() -> serde_json::Value {
        serde_json::json!({
            "skins": [{
                "uuid": "skin-ok",
                "texture": {
                    "data": { "value": "value-ok", "signature": "sig-ok" },
                    "hash": { "skin": "hash-ok" }
                }
            }]
        })
    }

    /// 非阻塞轮询 crossbeam channel 直到收到 `want` 条结果或超时（用
    /// `tokio::time::sleep` 让出控制权，不会像 `recv_timeout` 那样卡住整个
    /// 单线程 runtime、饿死 wiremock 的后台 mock server task）。
    async fn drain_until(
        pool: &mut SkinPool,
        want: usize,
        timeout: Duration,
    ) -> Vec<SkinFetchResult> {
        let deadline = Instant::now() + timeout;
        let mut collected = Vec::new();
        while collected.len() < want && Instant::now() < deadline {
            match pool.receiver.try_recv() {
                Ok(result) => collected.push(result),
                Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
            }
        }
        collected
    }

    #[tokio::test]
    async fn spawn_fetch_concurrent_delivers_ready_for_every_key() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/skins"))
            .respond_with(ResponseTemplate::new(200).set_body_json(success_body()))
            .mount(&server)
            .await;
        let client = MineSkinClient::new(server.uri(), None);
        let keys = vec![
            NpcSkinPoolKey(NpcSkinTier::Commoner),
            NpcSkinPoolKey(NpcSkinTier::RogueLow),
            NpcSkinPoolKey(NpcSkinTier::RogueMid),
        ];

        let mut pool = SkinPool::default();
        pool.spawn_fetch_concurrent(keys.clone(), 1, client);

        let results = drain_until(&mut pool, keys.len(), Duration::from_secs(5)).await;
        assert_eq!(
            results.len(),
            keys.len(),
            "expected exactly one result per key (got {results:?}) because a healthy endpoint \
             must not drop any key from the batch"
        );
        let ready_keys: HashSet<_> = results
            .iter()
            .map(|r| match r {
                SkinFetchResult::Ready { key, skins } => {
                    assert!(
                        !skins.is_empty(),
                        "Ready result for {key:?} must carry the fetched skins, got empty vec"
                    );
                    *key
                }
                SkinFetchResult::Failed { key, error } => {
                    panic!("key {key:?} unexpectedly failed against a healthy mock: {error}")
                }
            })
            .collect();
        assert_eq!(
            ready_keys,
            keys.iter().copied().collect::<HashSet<_>>(),
            "every requested key must appear exactly once among the Ready results"
        );
    }

    #[tokio::test]
    async fn spawn_fetch_concurrent_delivers_failed_for_every_key_without_dropping_any() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/skins"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let client = MineSkinClient::new(server.uri(), None);
        let keys = vec![
            NpcSkinPoolKey(NpcSkinTier::Commoner),
            NpcSkinPoolKey(NpcSkinTier::RogueLow),
        ];

        let mut pool = SkinPool::default();
        pool.spawn_fetch_concurrent(keys.clone(), 1, client);

        // 每个 key 内部会走 3 次重试 + backoff（累计 ~700ms+），超时留够余量。
        let results = drain_until(&mut pool, keys.len(), Duration::from_secs(10)).await;
        assert_eq!(
            results.len(),
            keys.len(),
            "a persistently-500 endpoint must still report a result for every key (got \
             {results:?}); fewer than {} results would mean one key's exhausted retries \
             aborted the whole join_all instead of only failing that key",
            keys.len()
        );
        assert!(
            results
                .iter()
                .all(|r| matches!(r, SkinFetchResult::Failed { .. })),
            "expected only Failed results when the endpoint always 500s, got {results:?}"
        );
    }

    #[tokio::test]
    async fn spawn_fetch_concurrent_runs_keys_in_parallel_not_serially() {
        let server = MockServer::start().await;
        const PER_KEY_DELAY_MS: u64 = 300;
        Mock::given(method("GET"))
            .and(path("/v2/skins"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(success_body())
                    .set_delay(Duration::from_millis(PER_KEY_DELAY_MS)),
            )
            .mount(&server)
            .await;
        let client = MineSkinClient::new(server.uri(), None);
        let keys = vec![
            NpcSkinPoolKey(NpcSkinTier::Commoner),
            NpcSkinPoolKey(NpcSkinTier::RogueLow),
            NpcSkinPoolKey(NpcSkinTier::RogueMid),
        ];
        let key_count = keys.len() as u64;

        let mut pool = SkinPool::default();
        let started = Instant::now();
        pool.spawn_fetch_concurrent(keys.clone(), 1, client);

        let results = drain_until(&mut pool, keys.len(), Duration::from_secs(5)).await;
        let elapsed = started.elapsed();

        assert_eq!(
            results.len(),
            keys.len(),
            "all {} keys must complete, got {results:?}",
            keys.len()
        );
        let serial_lower_bound = Duration::from_millis(PER_KEY_DELAY_MS * key_count);
        assert!(
            elapsed < serial_lower_bound,
            "F11 regression: {} keys against a {PER_KEY_DELAY_MS}ms-delayed endpoint took \
             {elapsed:?}, which is >= the serial lower bound {serial_lower_bound:?} (= {} keys * \
             {PER_KEY_DELAY_MS}ms). A concurrent join_all should complete in ~{PER_KEY_DELAY_MS}ms \
             regardless of key count; this bound only trips if fetches are still being awaited \
             one-at-a-time like the pre-F11 `for key in keys {{ block_on(..) }}` loop.",
            keys.len(),
            keys.len()
        );
    }

    #[tokio::test]
    async fn spawn_fetch_concurrent_empty_keys_is_a_noop() {
        let server = MockServer::start().await;
        // 故意不挂任何 mock：一旦误发请求，wiremock 对未匹配请求默认 500 +
        // panic-on-drop（`MockServer` verify），足以暴露"空 keys 却仍发起网络调用"
        // 的回归。
        let client = MineSkinClient::new(server.uri(), None);

        let mut pool = SkinPool::default();
        pool.spawn_fetch_concurrent(Vec::new(), 1, client);

        let results = drain_until(&mut pool, 1, Duration::from_millis(300)).await;
        assert!(
            results.is_empty(),
            "empty keys must not spawn any fetch thread or send any channel result, got {results:?}"
        );
    }
}
