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
    ResMut, With,
};

use crate::combat::components::{CombatState, Wounds};
use crate::combat::CombatClock;
use crate::inventory::ancient_relics::AncientRelicPool;
use crate::inventory::InventoryInstanceIdAllocator;
use crate::inventory::{
    bump_revision, consume_item_instance_once, ItemInstance, ItemRegistry, PlacedItemState,
    PlayerInventory, MAIN_PACK_CONTAINER_ID,
};
use crate::world::loot_pool::{roll_loot_pool, LootPoolRegistry};
use crate::world::tsy_container::{
    item_as_container_key, KeyKind, LootContainer, SearchProgress, SEARCH_INTERACT_RANGE_M,
    SEARCH_MOVE_INTERRUPT_THRESHOLD_M,
};
use crate::world::tsy_container_spawn::relic_source_for_family;

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

pub fn register(app: &mut valence::prelude::App) {
    use valence::prelude::{IntoSystemConfigs, Update};
    app.add_event::<StartSearchRequest>()
        .add_event::<StartSearchResult>()
        .add_event::<SearchCompleted>()
        .add_event::<SearchAborted>()
        .add_event::<RelicExtracted>()
        .add_event::<TsyZoneInitialized>()
        .add_event::<CancelSearchRequest>();
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

#[allow(clippy::too_many_arguments)]
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
        ),
        With<valence::prelude::Client>,
    >,
    clock: Res<CombatClock>,
    mut commands: Commands,
) {
    for req in requests.read() {
        let Ok((p_pos, p_inv, p_combat, p_progress)) = players.get(req.player) else {
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
        results.send(StartSearchResult::Started {
            player: req.player,
            container: req.container,
            required_ticks,
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub fn tick_search_progress(
    mut players: Query<
        (
            Entity,
            &Position,
            &CombatState,
            &Wounds,
            &mut SearchProgress,
        ),
        With<valence::prelude::Client>,
    >,
    mut commands: Commands,
    mut completed: EventWriter<SearchCompleted>,
    mut aborted: EventWriter<SearchAborted>,
    mut containers: Query<&mut LootContainer>,
    clock: Res<CombatClock>,
    item_registry: Res<ItemRegistry>,
    loot_pools: Res<LootPoolRegistry>,
    relic_pool: Res<AncientRelicPool>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut inventories: Query<&mut PlayerInventory>,
    mut relic_extracted: EventWriter<RelicExtracted>,
) {
    let mut to_clear: Vec<(Entity, Entity, Option<SearchAbortReason>)> = Vec::new();
    let mut completions: Vec<(Entity, Entity, Option<u64>)> = Vec::new();

    for (player_ent, pos, combat, wounds, mut progress) in players.iter_mut() {
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
            ));
        }
    }

    for (player_ent, container_ent, reason) in to_clear {
        commands
            .entity(player_ent)
            .remove::<SearchProgress>()
            .remove::<IsSearching>();
        if let Ok(mut c) = containers.get_mut(container_ent) {
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

    for (player_ent, container_ent, key_id) in completions {
        // 必须有容器
        let Ok(mut container) = containers.get_mut(container_ent) else {
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
        let loot = roll_loot_pool(
            &loot_pools,
            &container.loot_pool_id,
            &item_registry,
            &relic_pool,
            &mut allocator,
            source,
            seed,
        );

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
        container.searched_by = None;
        container.depleted = true;

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
}
