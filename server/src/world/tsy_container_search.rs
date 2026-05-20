//! plan-tsy-container-v1 §2 — TSY 容器搜刮 system + event 接口。
//!
//! 三个 system:
//! - `start_search_container` 消费 `StartSearchRequest`，做距离/钥匙/互斥/战斗
//!   等校验，成功 → 给玩家挂 `SearchProgress` + 容器 `searched_by` 锁定
//! - `tick_search_progress` 推进 elapsed_ticks + 检查移动/战斗/受击中断
//! - `handle_search_completed` 滚 loot 入背包 + 扣钥匙 + 标 depleted +
//!   RelicCore 发 RelicExtracted
//!
//! 中断条件：
//! - 玩家位置偏移 > [`SEARCH_MOVE_INTERRUPT_THRESHOLD_M`]
//! - 进入战斗（`CombatState.in_combat_until_tick > clock.tick`）
//! - 本 tick 受击（`Wounds.entries[*].created_at_tick == clock.tick`）
//!
//! 文件级 `#[allow(dead_code)]`：StartSearchResult / SearchCompleted /
//! SearchAborted / TsyZoneInitialized / RelicExtracted 字段是 IPC bridge /
//! agent narration 消费侧，本 plan 落实事件总线，client 端接入留 client plan。

#![allow(dead_code)]

use valence::prelude::{
    bevy_ecs, Commands, Component, Entity, Event, EventReader, EventWriter, Position, Query, Res,
    ResMut, Username, With,
};

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::combat::components::{CombatState, Wounds};
use crate::combat::CombatClock;
use crate::inventory::ancient_relics::AncientRelicPool;
use crate::inventory::spirit_treasure::{
    maybe_spawn_jizhaojing_from_relic_core, SpiritTreasureRegistry,
};
use crate::inventory::InventoryInstanceIdAllocator;
use crate::inventory::{
    bump_revision, consume_item_instance_once, ItemInstance, ItemRegistry, PlacedItemState,
    PlayerInventory, MAIN_PACK_CONTAINER_ID,
};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::player::state::canonical_player_id;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::loot_pool::{roll_loot_pool, LootPoolRegistry};
use crate::world::tsy_container::{
    item_as_container_key, KeyKind, LootContainer, SearchProgress, SEARCH_INTERACT_RANGE_M,
    SEARCH_MOVE_INTERRUPT_THRESHOLD_M,
};
use crate::world::tsy_container_spawn::relic_source_for_family;

const TSY_SEARCH_DUST_VFX: &str = "bong:tsy_search_dust";
const TSY_SEARCH_LOOT_POP_VFX: &str = "bong:tsy_search_loot_pop";
const TSY_SEARCH_SCRAPE_AUDIO: &str = "tsy_search_scrape";
const TSY_SEARCH_AUDIO_RADIUS: f64 = 24.0;

/// plan §2.1 — 玩家请求开始搜刮。
#[derive(Event, Debug, Clone)]
pub struct StartSearchRequest {
    pub player: Entity,
    pub container: Entity,
}

/// plan §2.1 — 开搜结果（成功 / 拒绝 + 原因）。
#[derive(Event, Debug, Clone)]
pub enum StartSearchResult {
    Started {
        player: Entity,
        container: Entity,
        required_ticks: u32,
    },
    Rejected {
        player: Entity,
        container: Entity,
        reason: SearchRejectionReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchRejectionReason {
    /// 容器已搜空
    Depleted,
    /// 已被其他玩家占用
    OccupiedByOther,
    /// 需要钥匙但 inventory 没有
    MissingKey(KeyKind),
    /// 玩家正在搜别的容器
    AlreadySearching,
    /// 距离超出
    OutOfRange,
    /// 战斗中
    InCombat,
    /// 散修遗缴每 24h 限额用尽
    DailyLimitExceeded,
}

/// plan §2.4 — 搜刮成功完成（loot 已发放）。
#[derive(Event, Debug, Clone)]
pub struct SearchCompleted {
    pub player: Entity,
    pub container: Entity,
    pub family_id: String,
    /// 这一搜出的 loot 拷贝（IPC 转 LootPreview 用）。
    pub loot: Vec<ItemInstance>,
}

/// plan §2.2 — 搜刮中断。
#[derive(Event, Debug, Clone)]
pub struct SearchAborted {
    pub player: Entity,
    pub container: Entity,
    pub reason: SearchAbortReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchAbortReason {
    Moved,
    Combat,
    Damaged,
    /// 玩家主动取消（点 ESC / 切武器等）
    Cancelled,
}

/// plan §0.6 / §6.2 — RelicCore 容器搜空时发；P2 lifecycle 可消费（当前为
/// informational：lifecycle 已通过 source_container_id 路径自给自足）。
#[derive(Event, Debug, Clone)]
pub struct RelicExtracted {
    pub family_id: String,
    pub at_tick: u64,
}

/// plan §6.2 — TSY family 容器一次性 spawn 完成时发；schema bridge 可消费。
#[derive(Event, Debug, Clone)]
pub struct TsyZoneInitialized {
    pub family_id: String,
    pub relic_count: u32,
    pub at_tick: u64,
}

/// 玩家主动取消搜刮（HUD/网络层翻译 ESC 键 → 此事件）。
#[derive(Event, Debug, Clone)]
pub struct CancelSearchRequest {
    pub player: Entity,
}

/// 玩家是否在搜刮中（plan §2.3 真元加速 hook 用 marker query）。
/// 与 `SearchProgress` Component 配套挂载，方便 query filter `With<IsSearching>`。
#[derive(Component, Debug, Default)]
pub struct IsSearching;

/// per-player per-poi 每 real-time 24h 只产出 3 次限频（plan-onboarding-loop-v1 P0.1 §6）。
/// key = (poi_id, player_uuid_string)，value = 今日已搜次数。
/// 24h 重置基于 wall-clock。
use valence::prelude::Resource;

/// 每个散修遗缴对每个玩家每 24h 允许的搜索次数上限。
pub const SURFACE_STASH_DAILY_LIMIT: u8 = 3;
/// 24 小时的秒数。
const SURFACE_STASH_RESET_INTERVAL_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Default, Resource)]
pub struct SurfaceStashPlayerLimit {
    /// (poi_id, canonical_player_id) → search_count_today
    pub limits: HashMap<(String, String), u8>,
    pub last_reset_wall_clock: u64,
}

impl SurfaceStashPlayerLimit {
    /// 检查该玩家对该 poi 是否还有搜索配额。若当前 wall-clock 距上次重置 ≥ 24h
    /// 则先重置所有计数器。
    pub fn can_search(&mut self, poi_id: &str, player_id: &str, now_secs: u64) -> bool {
        self.maybe_reset(now_secs);
        let key = (poi_id.to_string(), player_id.to_string());
        let count = self.limits.get(&key).copied().unwrap_or(0);
        count < SURFACE_STASH_DAILY_LIMIT
    }

    /// 记录一次搜索。返回搜索后的计数。
    pub fn record_search(&mut self, poi_id: &str, player_id: &str, now_secs: u64) -> u8 {
        self.maybe_reset(now_secs);
        let key = (poi_id.to_string(), player_id.to_string());
        let count = self.limits.entry(key).or_insert(0);
        *count = count.saturating_add(1);
        *count
    }

    fn maybe_reset(&mut self, now_secs: u64) {
        if now_secs.saturating_sub(self.last_reset_wall_clock) >= SURFACE_STASH_RESET_INTERVAL_SECS
        {
            self.limits.clear();
            self.last_reset_wall_clock = now_secs;
        }
    }

    /// 当前 wall-clock 秒（helper，测试可绕过）。
    pub fn current_wall_clock_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

pub fn register(app: &mut valence::prelude::App) {
    use valence::prelude::{IntoSystemConfigs, Update};
    app.init_resource::<SurfaceStashPlayerLimit>()
        .add_event::<StartSearchRequest>()
        .add_event::<StartSearchResult>()
        .add_event::<SearchCompleted>()
        .add_event::<SearchAborted>()
        .add_event::<RelicExtracted>()
        .add_event::<TsyZoneInitialized>()
        .add_event::<CancelSearchRequest>()
        .add_event::<VfxEventRequest>()
        .add_event::<PlaySoundRecipeRequest>();
    app.add_systems(
        Update,
        (
            start_search_container,
            tick_search_progress,
            handle_cancel_search,
        )
            .chain(),
    );
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn start_search_container(
    mut requests: EventReader<StartSearchRequest>,
    mut results: EventWriter<StartSearchResult>,
    mut containers: Query<(&mut LootContainer, &Position)>,
    players: Query<
        (
            &Position,
            &PlayerInventory,
            &CombatState,
            Option<&SearchProgress>,
            &Username,
        ),
        With<valence::prelude::Client>,
    >,
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
    mut stash_limit: ResMut<SurfaceStashPlayerLimit>,
) {
    for req in requests.read() {
        let Ok((p_pos, p_inv, p_combat, p_progress, p_username)) = players.get(req.player) else {
            continue;
        };
        let Ok((mut container, c_pos)) = containers.get_mut(req.container) else {
            continue;
        };

        if p_progress.is_some() {
            results.send(StartSearchResult::Rejected {
                player: req.player,
                container: req.container,
                reason: SearchRejectionReason::AlreadySearching,
            });
            continue;
        }
        if container.depleted {
            results.send(StartSearchResult::Rejected {
                player: req.player,
                container: req.container,
                reason: SearchRejectionReason::Depleted,
            });
            continue;
        }
        if let Some(other) = container.searched_by {
            if other != req.player {
                results.send(StartSearchResult::Rejected {
                    player: req.player,
                    container: req.container,
                    reason: SearchRejectionReason::OccupiedByOther,
                });
                continue;
            }
        }
        if p_pos.0.distance(c_pos.0) > SEARCH_INTERACT_RANGE_M {
            results.send(StartSearchResult::Rejected {
                player: req.player,
                container: req.container,
                reason: SearchRejectionReason::OutOfRange,
            });
            continue;
        }
        if is_in_combat(p_combat, clock.tick) {
            results.send(StartSearchResult::Rejected {
                player: req.player,
                container: req.container,
                reason: SearchRejectionReason::InCombat,
            });
            continue;
        }

        // 散修遗缴每 24h 限额检查
        if container.kind == crate::world::tsy_container::ContainerKind::SurfaceStash {
            let player_id = canonical_player_id(p_username.0.as_str());
            let now = SurfaceStashPlayerLimit::current_wall_clock_secs();
            if !stash_limit.can_search(&container.family_id, &player_id, now) {
                results.send(StartSearchResult::Rejected {
                    player: req.player,
                    container: req.container,
                    reason: SearchRejectionReason::DailyLimitExceeded,
                });
                continue;
            }
        }

        // 钥匙检查
        let key_id = match container.kind.required_key() {
            Some(kk) => match find_key_in_inventory(p_inv, kk) {
                Some(id) => Some(id),
                None => {
                    results.send(StartSearchResult::Rejected {
                        player: req.player,
                        container: req.container,
                        reason: SearchRejectionReason::MissingKey(kk),
                    });
                    continue;
                }
            },
            None => None,
        };

        // 通过校验 → 写状态
        container.searched_by = Some(req.player);
        let required_ticks = container.kind.base_search_ticks();
        commands.entity(req.player).insert((
            SearchProgress {
                container: req.container,
                required_ticks,
                elapsed_ticks: 0,
                started_at_tick: clock.tick,
                started_pos: [p_pos.0.x, p_pos.0.y, p_pos.0.z],
                key_item_instance_id: key_id,
            },
            IsSearching,
        ));
        vfx_events.send(spawn_particle_vfx(
            TSY_SEARCH_DUST_VFX,
            c_pos.0,
            Some("#9A8974"),
            Some(0.5),
            Some(8),
            Some(34),
            None,
        ));
        audio_events.send(world_audio_request(
            TSY_SEARCH_SCRAPE_AUDIO,
            c_pos.0,
            Some("tsy_search".to_string()),
        ));
        results.send(StartSearchResult::Started {
            player: req.player,
            container: req.container,
            required_ticks,
        });
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn tick_search_progress(
    mut players: Query<
        (
            Entity,
            &Position,
            &CombatState,
            &Wounds,
            &mut SearchProgress,
            &Username,
        ),
        With<valence::prelude::Client>,
    >,
    mut commands: Commands,
    mut completed: EventWriter<SearchCompleted>,
    mut aborted: EventWriter<SearchAborted>,
    mut containers: Query<(&mut LootContainer, &Position)>,
    clock: Res<CombatClock>,
    item_registry: Res<ItemRegistry>,
    loot_pools: Res<LootPoolRegistry>,
    relic_pool: Res<AncientRelicPool>,
    mut spirit_treasure_registry: ResMut<SpiritTreasureRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut inventories: Query<&mut PlayerInventory>,
    mut relic_extracted: EventWriter<RelicExtracted>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
    mut stash_limit: ResMut<SurfaceStashPlayerLimit>,
) {
    let mut to_clear: Vec<(Entity, Entity, Option<SearchAbortReason>)> = Vec::new();
    let mut completions: Vec<(Entity, Entity, Option<u64>, valence::prelude::DVec3, String)> =
        Vec::new();

    for (player_ent, pos, combat, wounds, mut progress, username) in players.iter_mut() {
        let dist = pos.0.distance(valence::math::DVec3::new(
            progress.started_pos[0],
            progress.started_pos[1],
            progress.started_pos[2],
        ));
        if dist > SEARCH_MOVE_INTERRUPT_THRESHOLD_M {
            to_clear.push((
                player_ent,
                progress.container,
                Some(SearchAbortReason::Moved),
            ));
            continue;
        }
        if is_in_combat(combat, clock.tick) {
            to_clear.push((
                player_ent,
                progress.container,
                Some(SearchAbortReason::Combat),
            ));
            continue;
        }
        if damaged_this_tick(wounds, clock.tick) {
            to_clear.push((
                player_ent,
                progress.container,
                Some(SearchAbortReason::Damaged),
            ));
            continue;
        }

        progress.elapsed_ticks = progress.elapsed_ticks.saturating_add(1);
        if progress.elapsed_ticks >= progress.required_ticks {
            completions.push((
                player_ent,
                progress.container,
                progress.key_item_instance_id,
                pos.0,
                canonical_player_id(username.0.as_str()),
            ));
        }
    }

    for (player_ent, container_ent, reason) in to_clear {
        commands
            .entity(player_ent)
            .remove::<SearchProgress>()
            .remove::<IsSearching>();
        if let Ok((mut c, _)) = containers.get_mut(container_ent) {
            if c.searched_by == Some(player_ent) {
                c.searched_by = None;
            }
        }
        if let Some(r) = reason {
            aborted.send(SearchAborted {
                player: player_ent,
                container: container_ent,
                reason: r,
            });
        }
    }

    for (player_ent, container_ent, key_id, player_pos, player_id) in completions {
        // 必须有容器
        let Ok((mut container, container_pos)) = containers.get_mut(container_ent) else {
            commands
                .entity(player_ent)
                .remove::<SearchProgress>()
                .remove::<IsSearching>();
            continue;
        };

        // 滚 loot
        let source = relic_source_for_family(&container.family_id);
        let seed = clock
            .tick
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(player_ent.to_bits());
        let mut loot = roll_loot_pool(
            &loot_pools,
            &container.loot_pool_id,
            &item_registry,
            &relic_pool,
            &mut allocator,
            source,
            seed,
        );
        if let Some(spirit_treasure) = maybe_spawn_jizhaojing_from_relic_core(
            &mut spirit_treasure_registry,
            source,
            container.depth,
            container_pos.0,
            seed,
            &mut allocator,
        ) {
            loot.push(spirit_treasure);
        }

        // 入背包（无空间则丢失，告 warn —— P3 demo 简化，不做 ownerless drop）
        if let Ok(mut inv) = inventories.get_mut(player_ent) {
            for item in &loot {
                place_item_in_main_pack(&mut inv, item.clone());
            }
            // 扣钥匙
            if let Some(kid) = key_id {
                if let Err(e) = consume_item_instance_once(&mut inv, kid) {
                    tracing::warn!(
                        "[bong][tsy-container] key consume failed for instance {kid}: {e}"
                    );
                }
            }
        }

        let family_id = container.family_id.clone();
        let is_skeleton = container.kind.is_skeleton();
        let is_surface_stash =
            container.kind == crate::world::tsy_container::ContainerKind::SurfaceStash;
        container.searched_by = None;
        container.depleted = true;

        // 散修遗缴完成搜索时记录限额
        if is_surface_stash {
            let now = SurfaceStashPlayerLimit::current_wall_clock_secs();
            stash_limit.record_search(&family_id, &player_id, now);
        }

        commands
            .entity(player_ent)
            .remove::<SearchProgress>()
            .remove::<IsSearching>();
        completed.send(SearchCompleted {
            player: player_ent,
            container: container_ent,
            family_id: family_id.clone(),
            loot,
        });
        vfx_events.send(spawn_particle_vfx(
            TSY_SEARCH_LOOT_POP_VFX,
            container_pos.0,
            Some("#FFD060"),
            Some(0.75),
            Some(8),
            Some(32),
            Some([player_pos.x, player_pos.y, player_pos.z]),
        ));
        audio_events.send(world_audio_request(
            TSY_SEARCH_SCRAPE_AUDIO,
            container_pos.0,
            None,
        ));

        if is_skeleton {
            relic_extracted.send(RelicExtracted {
                family_id,
                at_tick: clock.tick,
            });
        }
    }
}

pub fn handle_cancel_search(
    mut requests: EventReader<CancelSearchRequest>,
    mut commands: Commands,
    mut containers: Query<&mut LootContainer>,
    progress_q: Query<&SearchProgress>,
    mut aborted: EventWriter<SearchAborted>,
) {
    for req in requests.read() {
        let Ok(progress) = progress_q.get(req.player) else {
            continue;
        };
        let container_ent = progress.container;
        commands
            .entity(req.player)
            .remove::<SearchProgress>()
            .remove::<IsSearching>();
        if let Ok(mut c) = containers.get_mut(container_ent) {
            if c.searched_by == Some(req.player) {
                c.searched_by = None;
            }
        }
        aborted.send(SearchAborted {
            player: req.player,
            container: container_ent,
            reason: SearchAbortReason::Cancelled,
        });
    }
}

fn is_in_combat(state: &CombatState, current_tick: u64) -> bool {
    matches!(state.in_combat_until_tick, Some(t) if t > current_tick)
}

fn damaged_this_tick(wounds: &Wounds, current_tick: u64) -> bool {
    wounds
        .entries
        .iter()
        .any(|w| w.created_at_tick == current_tick)
}

pub(crate) fn find_key_in_inventory(inv: &PlayerInventory, kind: KeyKind) -> Option<u64> {
    let target = kind.template_id();
    for container in &inv.containers {
        for placed in &container.items {
            if placed.instance.template_id == target {
                return Some(placed.instance.instance_id);
            }
        }
    }
    // hotbar 也扫
    for slot in inv.hotbar.iter().flatten() {
        if let Some(k) = item_as_container_key(slot) {
            if k == kind {
                return Some(slot.instance_id);
            }
        }
    }
    None
}

fn place_item_in_main_pack(inv: &mut PlayerInventory, instance: ItemInstance) {
    let Some(main_pack) = inv
        .containers
        .iter_mut()
        .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
    else {
        tracing::warn!(
            "[bong][tsy-container] inventory 缺 `{MAIN_PACK_CONTAINER_ID}` 容器，loot 丢失"
        );
        return;
    };
    main_pack.items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance,
    });
    bump_revision(inv);
}

fn spawn_particle_vfx(
    event_id: &str,
    origin: valence::prelude::DVec3,
    color: Option<&str>,
    strength: Option<f32>,
    count: Option<u16>,
    duration_ticks: Option<u16>,
    direction: Option<[f64; 3]>,
) -> VfxEventRequest {
    VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id: event_id.to_string(),
            origin: [origin.x, origin.y, origin.z],
            direction,
            color: color.map(str::to_string),
            strength,
            count,
            duration_ticks,
        },
    )
}

fn world_audio_request(
    recipe_id: &str,
    origin: valence::prelude::DVec3,
    flag: Option<String>,
) -> PlaySoundRecipeRequest {
    PlaySoundRecipeRequest {
        recipe_id: recipe_id.to_string(),
        instance_id: 0,
        pos: Some([
            origin.x.floor() as i32,
            origin.y.floor() as i32,
            origin.z.floor() as i32,
        ]),
        flag,
        volume_mul: 1.0,
        pitch_shift: 0.0,
        recipient: AudioRecipient::Radius {
            origin,
            radius: TSY_SEARCH_AUDIO_RADIUS,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::{Wound, WoundKind};
    use crate::inventory::ContainerState;

    fn make_inv() -> PlayerInventory {
        PlayerInventory {
            revision: crate::inventory::InventoryRevision(0),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 4,
                cols: 5,
                items: Vec::new(),
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        }
    }

    fn key_item(template: &str, instance_id: u64) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template.to_string(),
            display_name: "key".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: crate::inventory::ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    #[test]
    fn find_key_in_inventory_main_pack() {
        let mut inv = make_inv();
        inv.containers[0].items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: key_item("key_stone_casket", 42),
        });
        assert_eq!(
            find_key_in_inventory(&inv, KeyKind::StoneCasketKey),
            Some(42)
        );
        assert_eq!(find_key_in_inventory(&inv, KeyKind::JadeCoffinSeal), None);
    }

    #[test]
    fn find_key_in_inventory_hotbar() {
        let mut inv = make_inv();
        inv.hotbar[0] = Some(key_item("key_array_core", 7));
        assert_eq!(
            find_key_in_inventory(&inv, KeyKind::ArrayCoreSigil),
            Some(7)
        );
    }

    #[test]
    fn find_key_in_inventory_none() {
        let inv = make_inv();
        assert_eq!(find_key_in_inventory(&inv, KeyKind::StoneCasketKey), None);
    }

    #[test]
    fn is_in_combat_recognises_active_window() {
        let mut s = CombatState::default();
        assert!(!is_in_combat(&s, 100));
        s.in_combat_until_tick = Some(150);
        assert!(is_in_combat(&s, 100));
        assert!(!is_in_combat(&s, 150)); // 等于不算（in_combat_until_tick > tick）
        assert!(!is_in_combat(&s, 200));
    }

    #[test]
    fn damaged_this_tick_match() {
        let mut w = Wounds::default();
        assert!(!damaged_this_tick(&w, 50));
        w.entries.push(Wound {
            location: crate::combat::components::BodyPart::Chest,
            kind: WoundKind::Blunt,
            severity: 0.1,
            bleeding_per_sec: 0.0,
            created_at_tick: 50,
            inflicted_by: None,
        });
        assert!(damaged_this_tick(&w, 50));
        assert!(!damaged_this_tick(&w, 51));
    }

    #[test]
    fn place_item_in_main_pack_works() {
        let mut inv = make_inv();
        let item = key_item("iron_sword", 99);
        place_item_in_main_pack(&mut inv, item);
        assert_eq!(inv.containers[0].items.len(), 1);
        assert_eq!(inv.containers[0].items[0].instance.instance_id, 99);
        assert_eq!(inv.revision.0, 1);
    }

    #[test]
    fn place_item_warns_without_main_pack() {
        let mut inv = make_inv();
        inv.containers.clear();
        // 不应 panic，仅警告
        place_item_in_main_pack(&mut inv, key_item("x", 1));
        assert!(inv.containers.is_empty());
    }

    // ——— plan-onboarding-loop-v1 P0: SurfaceStashPlayerLimit 测试 ———

    #[test]
    fn surface_stash_player_limit_allows_3_per_day() {
        let mut limit = SurfaceStashPlayerLimit::default();
        let now = 1_000_000u64;
        for i in 0..SURFACE_STASH_DAILY_LIMIT {
            assert!(
                limit.can_search("stash_0", "player_a", now),
                "第 {} 次搜索应被允许（上限 {}），但被拒绝",
                i + 1,
                SURFACE_STASH_DAILY_LIMIT
            );
            limit.record_search("stash_0", "player_a", now);
        }
    }

    #[test]
    fn surface_stash_player_limit_blocks_4th_search() {
        let mut limit = SurfaceStashPlayerLimit::default();
        let now = 1_000_000u64;
        for _ in 0..SURFACE_STASH_DAILY_LIMIT {
            limit.record_search("stash_0", "player_a", now);
        }
        assert!(
            !limit.can_search("stash_0", "player_a", now),
            "第 {} 次搜索应被拒绝，但被允许",
            SURFACE_STASH_DAILY_LIMIT + 1
        );
    }

    #[test]
    fn surface_stash_player_limit_resets_after_24h() {
        let mut limit = SurfaceStashPlayerLimit::default();
        let now = 1_000_000u64;
        for _ in 0..SURFACE_STASH_DAILY_LIMIT {
            limit.record_search("stash_0", "player_a", now);
        }
        assert!(!limit.can_search("stash_0", "player_a", now));

        // 24h 后重置
        let after_24h = now + 24 * 60 * 60;
        assert!(
            limit.can_search("stash_0", "player_a", after_24h),
            "24h 后限额应重置，但搜索仍被拒绝"
        );
    }

    #[test]
    fn surface_stash_limit_24h_minus_1s_does_not_reset() {
        let mut limit = SurfaceStashPlayerLimit::default();
        let now = 1_000_000u64;
        for _ in 0..SURFACE_STASH_DAILY_LIMIT {
            limit.record_search("stash_0", "player_a", now);
        }
        // 24h - 1s：不应重置
        let almost_24h = now + 24 * 60 * 60 - 1;
        assert!(
            !limit.can_search("stash_0", "player_a", almost_24h),
            "24h-1s 时限额不应重置（off-by-one），但 can_search 返回了 true"
        );
    }

    #[test]
    fn surface_stash_limit_poi_isolation() {
        let mut limit = SurfaceStashPlayerLimit::default();
        let now = 1_000_000u64;
        // 在 stash_0 用完配额
        for _ in 0..SURFACE_STASH_DAILY_LIMIT {
            limit.record_search("stash_0", "player_a", now);
        }
        assert!(
            !limit.can_search("stash_0", "player_a", now),
            "stash_0 配额用尽后应拒绝"
        );
        // stash_1 应独立计数，仍然可搜
        assert!(
            limit.can_search("stash_1", "player_a", now),
            "不同 poi（stash_1）的配额应独立于 stash_0，但被拒绝"
        );
    }

    #[test]
    fn surface_stash_limit_player_isolation() {
        let mut limit = SurfaceStashPlayerLimit::default();
        let now = 1_000_000u64;
        // player_a 用完配额
        for _ in 0..SURFACE_STASH_DAILY_LIMIT {
            limit.record_search("stash_0", "player_a", now);
        }
        assert!(
            !limit.can_search("stash_0", "player_a", now),
            "player_a 配额用尽后应拒绝"
        );
        // player_b 应独立计数，仍然可搜
        assert!(
            limit.can_search("stash_0", "player_b", now),
            "不同玩家（player_b）的配额应独立于 player_a，但被拒绝"
        );
    }

    #[test]
    fn surface_stash_limit_empty_state_allows_search() {
        let mut limit = SurfaceStashPlayerLimit::default();
        let now = 1_000_000u64;
        // 初始无记录时 can_search 应返回 true
        assert!(
            limit.can_search("stash_0", "player_a", now),
            "初始无记录时 can_search 应返回 true，但返回了 false"
        );
    }
}
