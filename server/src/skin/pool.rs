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
    fallback_mode: bool,
    ready_deadline: Instant,
    request_generation: AtomicU64,
}

impl Resource for SkinPool {}

impl Default for SkinPool {
    fn default() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self {
            by_pool_key: HashMap::new(),
            failover: VecDeque::new(),
            receiver,
            sender,
            inflight: HashSet::new(),
            started_prefetch: false,
            fallback_mode: false,
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
        self.fallback_mode || self.ready_count() >= MIN_READY_BEFORE_SPAWN
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

        SignedSkin::fallback()
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
                    self.fallback_mode = true;
                    tracing::warn!(
                        "[bong][skin] MineSkin unavailable for pool {} (error={error}), falling back to vanilla entity kinds",
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
        self.started_prefetch = true;
        self.ready_deadline = Instant::now() + PREFETCH_TIMEOUT;

        let client = match MineSkinClient::from_env() {
            Ok(client) => client,
            Err(error) => {
                self.fallback_mode = true;
                tracing::warn!(
                    "[bong][skin] MineSkin unavailable (error={error}), falling back to vanilla entity kinds for 100 rogues"
                );
                return;
            }
        };

        for key in NpcSkinPoolKey::PREFETCH_KEYS {
            self.spawn_fetch(key, PREFETCH_TARGET_PER_POOL_KEY, client.clone());
        }
    }

    fn maybe_mark_timeout(&mut self) {
        if !self.fallback_mode
            && self.started_prefetch
            && self.ready_count() < MIN_READY_BEFORE_SPAWN
            && Instant::now() >= self.ready_deadline
        {
            self.fallback_mode = true;
            tracing::warn!(
                "[bong][skin] MineSkin prefetch timed out before {MIN_READY_BEFORE_SPAWN} skins, falling back to vanilla entity kinds for 100 rogues"
            );
        }
    }

    fn maybe_refill(&mut self) {
        if self.fallback_mode {
            return;
        }
        for key in NpcSkinPoolKey::PREFETCH_KEYS {
            if self.len_for_key(key) <= REFILL_THRESHOLD && !self.inflight.contains(&key) {
                if let Ok(client) = MineSkinClient::from_env() {
                    self.spawn_fetch(key, PREFETCH_TARGET_PER_POOL_KEY, client);
                }
            }
        }
    }

    fn spawn_fetch(&mut self, key: NpcSkinPoolKey, count: usize, client: MineSkinClient) {
        if !self.inflight.insert(key) {
            return;
        }
        let sender = self.sender.clone();
        let request_id = self.request_generation.fetch_add(1, Ordering::Relaxed);
        std::thread::Builder::new()
            .name(format!("bong-skin-prefetch-{request_id}"))
            .spawn(move || {
                let runtime = match tokio::runtime::Runtime::new() {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = sender.send(SkinFetchResult::Failed {
                            key,
                            error: format!("tokio runtime: {error}"),
                        });
                        return;
                    }
                };

                let result = runtime.block_on(async move { client.fetch_random(count).await });
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
            })
            .map(std::mem::drop)
            .unwrap_or_else(|error| {
                let _ = self.sender.send(SkinFetchResult::Failed {
                    key,
                    error: format!("thread spawn: {error}"),
                });
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
    fn next_for_empty_pool_returns_fallback() {
        let mut pool = SkinPool::default();
        let skin = pool.next_for_profile(profile(NpcSkinPoolKey(NpcSkinTier::RogueLow)), 0);

        assert!(skin.is_fallback());
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
    fn npc_uuid_is_stable_for_same_entity_bits() {
        let entity = Entity::from_bits(0x0000_0004_0000_002a);

        assert_eq!(npc_uuid(entity), npc_uuid(entity));
        assert_ne!(npc_uuid(entity), Uuid::nil());
    }
}
