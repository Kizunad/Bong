//! plan-lingtian-v1 P1 ECS — 把事件 / session 状态机接到 ECS 世界。
//!
//! 职责：
//!   * `handle_start_till` / `handle_start_renew` —— 收意图请求 → 验前置 → 起 session
//!   * `tick_lingtian_sessions` —— 每 Update tick 推进所有活跃 session
//!   * `apply_completed_sessions` —— Finished 的 session：spawn / reset Plot Entity，
//!     扣玩家主手锄耐久（归零则从 equipped 移除）
//!
//! 单 player 单 session：`ActiveLingtianSessions` 以玩家 Entity 为 key，
//! 进新请求时若已有活 session 直接拒。
//!
//! plot 实体：当前切片把 LingtianPlot 作为独立 Entity（`spawn(LingtianPlot, ...)`）
//! 而非真正的 valence BlockEntity（后者依 plan-persistence-v1）。Renew 通过
//! `Query<&mut LingtianPlot>` 按 BlockPos 反查匹配 plot。

use std::collections::HashMap;

use valence::prelude::{
    bevy_ecs, Commands, Entity, EventReader, EventWriter, Query, ResMut, Resource,
};

use crate::inventory::PlayerInventory;

use super::events::{RenewCompleted, StartRenewRequest, StartTillRequest, TillCompleted};
use super::hoe::HoeKind;
use super::plot::LingtianPlot;
use super::session::{RenewSession, TillSession};
use super::terrain::classify_for_till;

const MAIN_HAND_SLOT: &str = "main_hand";

#[derive(Debug)]
pub enum ActiveSession {
    Till(TillSession),
    Renew(RenewSession),
}

impl ActiveSession {
    fn tick(&mut self) {
        match self {
            ActiveSession::Till(s) => s.tick(),
            ActiveSession::Renew(s) => s.tick(),
        }
    }

    fn is_finished(&self) -> bool {
        match self {
            ActiveSession::Till(s) => s.is_finished(),
            ActiveSession::Renew(s) => s.is_finished(),
        }
    }
}

#[derive(Debug, Default, Resource)]
pub struct ActiveLingtianSessions {
    by_player: HashMap<Entity, ActiveSession>,
}

impl ActiveLingtianSessions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_session(&self, player: Entity) -> bool {
        self.by_player.contains_key(&player)
    }

    pub fn get(&self, player: Entity) -> Option<&ActiveSession> {
        self.by_player.get(&player)
    }

    pub fn len(&self) -> usize {
        self.by_player.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_player.is_empty()
    }

    /// 插入新 session。若该 player 已有则返回 false，调用方丢弃请求。
    pub fn try_insert(&mut self, player: Entity, session: ActiveSession) -> bool {
        if self.by_player.contains_key(&player) {
            return false;
        }
        self.by_player.insert(player, session);
        true
    }

    /// 清掉某 player 的 session（cancel / 完成结算后）。
    pub fn clear(&mut self, player: Entity) -> Option<ActiveSession> {
        self.by_player.remove(&player)
    }

    /// 返回所有当前已 Finished 的 (player, session) 对，并从表中移除。
    fn drain_finished(&mut self) -> Vec<(Entity, ActiveSession)> {
        let finished_players: Vec<Entity> = self
            .by_player
            .iter()
            .filter(|(_, s)| s.is_finished())
            .map(|(e, _)| *e)
            .collect();
        finished_players
            .into_iter()
            .map(|e| (e, self.by_player.remove(&e).expect("just iterated")))
            .collect()
    }

    fn tick_all(&mut self) {
        for s in self.by_player.values_mut() {
            s.tick();
        }
    }
}

// ============================================================================
// 起 session
// ============================================================================

/// 验玩家主手是否持指定档锄。
///
/// 命中策略：玩家 `equipped[main_hand]` 的 template_id 必须能反查出 `HoeKind`
/// 且与请求声明的 hoe 一致。
fn player_holds_hoe(inventory: &PlayerInventory, expected: HoeKind) -> bool {
    inventory
        .equipped
        .get(MAIN_HAND_SLOT)
        .and_then(|item| HoeKind::from_item_id(&item.template_id))
        .is_some_and(|k| k == expected)
}

pub fn handle_start_till(
    mut events: EventReader<StartTillRequest>,
    mut sessions: ResMut<ActiveLingtianSessions>,
    inventories: Query<&PlayerInventory>,
) {
    for req in events.read() {
        if sessions.has_session(req.player) {
            tracing::warn!(
                "[bong][lingtian] StartTillRequest rejected: player={:?} already has active session",
                req.player
            );
            continue;
        }
        let Ok(inv) = inventories.get(req.player) else {
            tracing::warn!(
                "[bong][lingtian] StartTillRequest rejected: player={:?} has no PlayerInventory",
                req.player
            );
            continue;
        };
        if !player_holds_hoe(inv, req.hoe) {
            tracing::warn!(
                "[bong][lingtian] StartTillRequest rejected: player={:?} not holding {} in main hand",
                req.player,
                req.hoe
            );
            continue;
        }
        if let Err(reason) = classify_for_till(req.terrain) {
            tracing::warn!(
                "[bong][lingtian] StartTillRequest rejected: terrain={:?} reason={:?}",
                req.terrain,
                reason
            );
            continue;
        }
        let session = TillSession::new(req.pos, req.hoe, req.mode);
        sessions.try_insert(req.player, ActiveSession::Till(session));
    }
}

pub fn handle_start_renew(
    mut events: EventReader<StartRenewRequest>,
    mut sessions: ResMut<ActiveLingtianSessions>,
    inventories: Query<&PlayerInventory>,
    plots: Query<&LingtianPlot>,
) {
    for req in events.read() {
        if sessions.has_session(req.player) {
            tracing::warn!(
                "[bong][lingtian] StartRenewRequest rejected: player={:?} already has active session",
                req.player
            );
            continue;
        }
        let Ok(inv) = inventories.get(req.player) else {
            continue;
        };
        if !player_holds_hoe(inv, req.hoe) {
            tracing::warn!(
                "[bong][lingtian] StartRenewRequest rejected: player={:?} not holding {} in main hand",
                req.player,
                req.hoe
            );
            continue;
        }
        // 必须有处于"贫瘠"状态的 plot
        let barren = plots.iter().any(|p| p.pos == req.pos && p.is_barren());
        if !barren {
            tracing::warn!(
                "[bong][lingtian] StartRenewRequest rejected: no barren plot at {:?}",
                req.pos
            );
            continue;
        }
        let session = RenewSession::new(req.pos, req.hoe);
        sessions.try_insert(req.player, ActiveSession::Renew(session));
    }
}

// ============================================================================
// tick + 结算
// ============================================================================

pub fn tick_lingtian_sessions(mut sessions: ResMut<ActiveLingtianSessions>) {
    sessions.tick_all();
}

pub fn apply_completed_sessions(
    mut commands: Commands,
    mut sessions: ResMut<ActiveLingtianSessions>,
    mut inventories: Query<&mut PlayerInventory>,
    mut plots: Query<(Entity, &mut LingtianPlot)>,
    mut till_completed: EventWriter<TillCompleted>,
    mut renew_completed: EventWriter<RenewCompleted>,
) {
    for (player, finished) in sessions.drain_finished() {
        let (pos, hoe) = match &finished {
            ActiveSession::Till(s) => (s.pos, s.hoe),
            ActiveSession::Renew(s) => (s.pos, s.hoe),
        };

        // 扣锄耐久（归零则从 equipped 移除）
        if let Ok(mut inv) = inventories.get_mut(player) {
            wear_main_hand_hoe(&mut inv, hoe);
        }

        match finished {
            ActiveSession::Till(_) => {
                commands.spawn(LingtianPlot::new(pos, Some(player)));
                till_completed.send(TillCompleted { player, pos, hoe });
            }
            ActiveSession::Renew(_) => {
                if let Some((_e, mut plot)) = plots.iter_mut().find(|(_, p)| p.pos == pos) {
                    plot.renew();
                    renew_completed.send(RenewCompleted { player, pos, hoe });
                } else {
                    tracing::warn!(
                        "[bong][lingtian] RenewSession finished but plot at {:?} vanished",
                        pos
                    );
                }
            }
        }
    }
}

/// plan §1.2.1 / §1.6 — 主手锄扣 1 次耐久。归一化 [0, 1]。归零移除装备。
fn wear_main_hand_hoe(inventory: &mut PlayerInventory, expected: HoeKind) {
    let cost = expected.use_durability_cost();
    let entry = inventory.equipped.get_mut(MAIN_HAND_SLOT);
    let Some(item) = entry else {
        return;
    };
    if HoeKind::from_item_id(&item.template_id) != Some(expected) {
        return;
    }
    item.durability = (item.durability - cost).max(0.0);
    if item.durability <= 0.0 {
        inventory.equipped.remove(MAIN_HAND_SLOT);
    }
}

/// 取消某 player 的 session（外部如 quit / 离线 / 主动取消调用）。
#[allow(dead_code)]
pub fn cancel_player_session(
    sessions: &mut ActiveLingtianSessions,
    player: Entity,
) -> Option<ActiveSession> {
    sessions.clear(player)
}

// ============================================================================
// 端到端集成测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{InventoryRevision, ItemInstance, ItemRarity, PlayerInventory};
    use std::collections::HashMap;
    use valence::prelude::{App, BlockPos, IntoSystemConfigs, Update};

    use super::super::events::{
        RenewCompleted, StartRenewRequest, StartTillRequest, TillCompleted,
    };
    use super::super::hoe::HoeKind;
    use super::super::session::{SessionMode, RENEW_TICKS, TILL_MANUAL_TICKS};
    use super::super::terrain::TerrainKind;

    fn make_hoe_instance(kind: HoeKind, durability: f64) -> ItemInstance {
        ItemInstance {
            instance_id: 1,
            template_id: kind.item_id().to_string(),
            display_name: kind.item_id().to_string(),
            grid_w: 1,
            grid_h: 2,
            weight: 1.5,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability,
        }
    }

    fn make_inventory_with_hoe(kind: HoeKind, durability: f64) -> PlayerInventory {
        let mut equipped = HashMap::new();
        equipped.insert(
            MAIN_HAND_SLOT.to_string(),
            make_hoe_instance(kind, durability),
        );
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![],
            equipped,
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    fn build_app() -> App {
        let mut app = App::new();
        app.insert_resource(ActiveLingtianSessions::new())
            .add_event::<StartTillRequest>()
            .add_event::<TillCompleted>()
            .add_event::<StartRenewRequest>()
            .add_event::<RenewCompleted>()
            .add_systems(
                Update,
                (
                    handle_start_till,
                    handle_start_renew,
                    tick_lingtian_sessions,
                    apply_completed_sessions,
                )
                    .chain(),
            );
        app
    }

    #[test]
    fn till_e2e_spawns_plot_and_decrements_durability() {
        let mut app = build_app();
        let player = app
            .world_mut()
            .spawn(make_inventory_with_hoe(HoeKind::Iron, 1.0))
            .id();
        let pos = BlockPos::new(10, 64, 10);
        app.world_mut().send_event(StartTillRequest {
            player,
            pos,
            hoe: HoeKind::Iron,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Grass,
        });

        // 第 1 次 update：handle_start_till 起 session + tick_lingtian_sessions 推 1
        app.update();
        assert_eq!(app.world().resource::<ActiveLingtianSessions>().len(), 1);

        // 再 TILL_MANUAL_TICKS - 1 次 update（共 TILL_MANUAL_TICKS tick 满）
        for _ in 0..TILL_MANUAL_TICKS - 1 {
            app.update();
        }

        // session 应当 finished + plot spawn 完成 + 锄扣 1 次（durability -= 0.05）
        assert!(
            app.world().resource::<ActiveLingtianSessions>().is_empty(),
            "session 完成后应清出表"
        );
        let plots: Vec<_> = app
            .world_mut()
            .query::<&LingtianPlot>()
            .iter(app.world())
            .collect();
        assert_eq!(plots.len(), 1);
        assert_eq!(plots[0].pos, pos);
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        let dur = inv.equipped.get(MAIN_HAND_SLOT).unwrap().durability;
        assert!((dur - 0.95).abs() < 1e-9, "Iron 锄一次扣 0.05；实得 {dur}");
    }

    #[test]
    fn till_rejected_when_not_holding_hoe() {
        let mut app = build_app();
        // 玩家手里啥都没有
        let player = app
            .world_mut()
            .spawn(PlayerInventory {
                revision: InventoryRevision(0),
                containers: vec![],
                equipped: HashMap::new(),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 45.0,
            })
            .id();
        app.world_mut().send_event(StartTillRequest {
            player,
            pos: BlockPos::new(0, 64, 0),
            hoe: HoeKind::Iron,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Grass,
        });
        app.update();
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    }

    #[test]
    fn till_rejected_on_blocked_terrain() {
        let mut app = build_app();
        let player = app
            .world_mut()
            .spawn(make_inventory_with_hoe(HoeKind::Iron, 1.0))
            .id();
        app.world_mut().send_event(StartTillRequest {
            player,
            pos: BlockPos::new(0, 64, 0),
            hoe: HoeKind::Iron,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Stone,
        });
        app.update();
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    }

    #[test]
    fn second_till_during_active_session_is_rejected() {
        let mut app = build_app();
        let player = app
            .world_mut()
            .spawn(make_inventory_with_hoe(HoeKind::Iron, 1.0))
            .id();
        app.world_mut().send_event(StartTillRequest {
            player,
            pos: BlockPos::new(0, 64, 0),
            hoe: HoeKind::Iron,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Grass,
        });
        app.update();
        assert_eq!(app.world().resource::<ActiveLingtianSessions>().len(), 1);
        // 第二请求应被拒
        app.world_mut().send_event(StartTillRequest {
            player,
            pos: BlockPos::new(1, 64, 0),
            hoe: HoeKind::Iron,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Dirt,
        });
        app.update();
        assert_eq!(
            app.world().resource::<ActiveLingtianSessions>().len(),
            1,
            "重复请求不应叠 session"
        );
    }

    #[test]
    fn renew_e2e_resets_barren_plot() {
        let mut app = build_app();
        let player = app
            .world_mut()
            .spawn(make_inventory_with_hoe(HoeKind::Xuantie, 1.0))
            .id();
        let pos = BlockPos::new(5, 64, 5);
        // 直接 spawn 一个贫瘠 plot
        let mut plot = LingtianPlot::new(pos, Some(player));
        plot.harvest_count = super::super::plot::N_RENEW;
        app.world_mut().spawn(plot);

        app.world_mut().send_event(StartRenewRequest {
            player,
            pos,
            hoe: HoeKind::Xuantie,
        });
        for _ in 0..RENEW_TICKS {
            app.update();
        }
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
        let plot = app
            .world_mut()
            .query::<&LingtianPlot>()
            .iter(app.world())
            .next()
            .unwrap();
        assert_eq!(plot.harvest_count, 0, "翻新应重置 harvest_count");
        assert!(!plot.is_barren());
        // Xuantie 一次扣 0.01
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        let dur = inv.equipped.get(MAIN_HAND_SLOT).unwrap().durability;
        assert!((dur - 0.99).abs() < 1e-9, "Xuantie 一次扣 0.01；实得 {dur}");
    }

    #[test]
    fn renew_rejected_when_plot_not_barren() {
        let mut app = build_app();
        let player = app
            .world_mut()
            .spawn(make_inventory_with_hoe(HoeKind::Iron, 1.0))
            .id();
        let pos = BlockPos::new(0, 64, 0);
        // 新 plot，未贫瘠
        app.world_mut().spawn(LingtianPlot::new(pos, None));
        app.world_mut().send_event(StartRenewRequest {
            player,
            pos,
            hoe: HoeKind::Iron,
        });
        app.update();
        assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    }

    #[test]
    fn hoe_breaks_at_zero_durability() {
        let mut app = build_app();
        // Iron 锄剩 0.05 → 一次操作就归零（uses_max=20，cost=0.05）
        let player = app
            .world_mut()
            .spawn(make_inventory_with_hoe(HoeKind::Iron, 0.05))
            .id();
        let pos = BlockPos::new(0, 64, 0);
        app.world_mut().send_event(StartTillRequest {
            player,
            pos,
            hoe: HoeKind::Iron,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Grass,
        });
        for _ in 0..TILL_MANUAL_TICKS {
            app.update();
        }
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(
            !inv.equipped.contains_key(MAIN_HAND_SLOT),
            "锄归零应从 equipped 移除"
        );
    }
}
