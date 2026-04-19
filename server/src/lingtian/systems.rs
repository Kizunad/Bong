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
    bevy_ecs, Commands, Entity, EventReader, EventWriter, Query, Res, ResMut, Resource,
};

use crate::botany::PlantKindRegistry;
use crate::inventory::PlayerInventory;

use super::events::{RenewCompleted, StartRenewRequest, StartTillRequest, TillCompleted};
use super::growth::advance_one_lingtian_tick;
use super::hoe::HoeKind;
use super::plot::LingtianPlot;
use super::qi_account::{LingtianTickAccumulator, ZoneQiAccount, DEFAULT_ZONE};
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
// 生长 tick（plan §1.3 / §4 LingtianTick）
// ============================================================================

/// 每 Bevy tick 累一次；满 1200 触发一 lingtian-tick：迭代所有 plot，按
/// `botany::PlantKindRegistry` 查 PlantKind，调 `advance_one_lingtian_tick`
/// 推进 growth + plot_qi + zone qi。
///
/// zone 解析当前简化为 `DEFAULT_ZONE`（plan §1.3 注释：world::zone 真挂接
/// 留 P3+，与 plan-zhenfa-v1 / WorldQiAccount 整合）。
pub fn lingtian_growth_tick(
    mut accumulator: ResMut<LingtianTickAccumulator>,
    mut zone_qi: ResMut<ZoneQiAccount>,
    registry: Res<PlantKindRegistry>,
    mut plots: Query<&mut LingtianPlot>,
) {
    if !accumulator.step() {
        return;
    }
    for mut plot in plots.iter_mut() {
        advance_plot_one_lingtian_tick(&mut plot, &registry, &mut zone_qi);
    }
}

/// 推一个 plot 一步：查 `PlantKind`、按 `DEFAULT_ZONE` 取 zone qi、调 growth 公式。
///
/// 把"找 kind / 找 zone / 调用 advance"封装在一处，便于：
///   * `lingtian_growth_tick` system 在 Query 迭代里调
///   * 测试代码绕开 1200 个 Bevy tick 直推
pub fn advance_plot_one_lingtian_tick(
    plot: &mut LingtianPlot,
    registry: &PlantKindRegistry,
    zone_qi: &mut ZoneQiAccount,
) {
    let kind_id = match plot.crop.as_ref().map(|c| c.kind.clone()) {
        Some(id) => id,
        None => return,
    };
    let Some(kind) = registry.get(&kind_id) else {
        tracing::warn!(
            "[bong][lingtian] plot at {:?} carries unknown plant_id={}",
            plot.pos,
            kind_id
        );
        return;
    };
    let zone_qi_ref = zone_qi.get_mut(DEFAULT_ZONE);
    advance_one_lingtian_tick(plot, kind, zone_qi_ref);
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

    // ------------------------------------------------------------------------
    // P2 生长 tick e2e
    // ------------------------------------------------------------------------

    use crate::botany::{GrowthCost, PlantKind, PlantKindRegistry, PlantRarity};
    use crate::lingtian::plot::CropInstance;
    use crate::lingtian::qi_account::BEVY_TICKS_PER_LINGTIAN_TICK;

    fn ci_she_hao_kind() -> PlantKind {
        PlantKind {
            id: "ci_she_hao".into(),
            display_name: "刺舌蒿".into(),
            cultivable: true,
            growth_cost: GrowthCost::Low,
            growth_duration_ticks: 480,
            rarity: PlantRarity::Common,
            description: String::new(),
        }
    }

    fn registry_with(kind: PlantKind) -> PlantKindRegistry {
        let mut r = PlantKindRegistry::new();
        r.insert(kind).unwrap();
        r
    }

    fn build_growth_app(zone_qi: f32) -> App {
        let mut app = App::new();
        let mut acc = ZoneQiAccount::new();
        acc.set(DEFAULT_ZONE, zone_qi);
        app.insert_resource(LingtianTickAccumulator::new())
            .insert_resource(acc)
            .insert_resource(registry_with(ci_she_hao_kind()))
            .add_systems(Update, lingtian_growth_tick);
        app
    }

    fn spawn_planted_plot(app: &mut App, plot_qi: f32) -> Entity {
        let mut p = LingtianPlot::new(BlockPos::new(0, 64, 0), None);
        p.plot_qi = plot_qi;
        p.crop = Some(CropInstance::new("ci_she_hao".into()));
        app.world_mut().spawn(p).id()
    }

    // 注：1 lingtian-tick = 1200 Bevy tick；通过 `app.update()` 走完整路径
    // 单测过慢（每 lingtian-tick ≥ 100ms）。其余生长测试改用
    // `advance_n_lingtian_ticks_direct` 直推，accumulator 路径单独由
    // `growth_tick_does_not_fire_before_1200_bevy_ticks` 守。

    #[test]
    fn growth_tick_does_not_fire_before_1200_bevy_ticks() {
        let mut app = build_growth_app(0.0);
        let plot = spawn_planted_plot(&mut app, 1000.0);
        // plot_qi_cap 默认 1.0；为做"持续 baseline mult"，本测只关心: < 1200 tick 不动
        for _ in 0..BEVY_TICKS_PER_LINGTIAN_TICK - 1 {
            app.update();
        }
        let p = app.world().get::<LingtianPlot>(plot).unwrap();
        assert_eq!(
            p.crop.as_ref().unwrap().growth,
            0.0,
            "1200 - 1 个 Bevy tick 不应触发 lingtian-tick"
        );
    }

    /// 直推 lingtian-tick，跳过 1200×N 个 Bevy update（accumulator 已有独立单测）。
    fn advance_n_lingtian_ticks_direct(app: &mut App, n: u32) {
        for _ in 0..n {
            let world = app.world_mut();
            let mut zone_qi = world.remove_resource::<ZoneQiAccount>().unwrap();
            let registry = world.remove_resource::<PlantKindRegistry>().unwrap();
            let mut state = world.query::<&mut LingtianPlot>();
            for mut plot in state.iter_mut(world) {
                advance_plot_one_lingtian_tick(&mut plot, &registry, &mut zone_qi);
            }
            world.insert_resource(zone_qi);
            world.insert_resource(registry);
        }
    }

    #[test]
    fn ci_she_hao_ripens_in_480_lingtian_ticks_at_full_qi() {
        let mut app = build_growth_app(0.0);
        // plot_qi cap=1.0；每 lingtian-tick 扣 0.002（low）→ 480 tick 扣 0.96，不会枯。
        // ratio 起始=1.0 → mult=1.5 → 应早于 480 tick 熟。
        let plot = spawn_planted_plot(&mut app, 1.0);
        advance_n_lingtian_ticks_direct(&mut app, 480);
        let p = app.world().get::<LingtianPlot>(plot).unwrap();
        let crop = p.crop.as_ref().unwrap();
        assert!(crop.is_ripe(), "growth = {}", crop.growth);
    }

    #[test]
    fn zone_leak_path_when_plot_qi_dry() {
        let mut app = build_growth_app(2.0); // zone qi 充足
        let plot = spawn_planted_plot(&mut app, 0.0);
        advance_n_lingtian_ticks_direct(&mut app, 10);
        let p = app.world().get::<LingtianPlot>(plot).unwrap();
        let g = p.crop.as_ref().unwrap().growth;
        // 漏吸 10 tick：每次 = 1/480 × 0.3 = 0.000625；累 10 = 0.00625
        let expected = 10.0 * (1.0_f32 / 480.0) * 0.3;
        assert!(
            (g - expected).abs() < 1e-5,
            "growth = {g}, expected ≈ {expected}"
        );
        let zone_left = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
        // 漏吸 10 次：每次 0.002 × 0.2 = 0.0004；累 10 = 0.004
        let zone_consumed = 10.0 * 0.002 * 0.2;
        assert!(
            (zone_left - (2.0 - zone_consumed)).abs() < 1e-5,
            "zone_left = {zone_left}"
        );
    }

    #[test]
    fn stalls_when_plot_and_zone_both_dry() {
        let mut app = build_growth_app(0.0);
        let plot = spawn_planted_plot(&mut app, 0.0);
        advance_n_lingtian_ticks_direct(&mut app, 50);
        let p = app.world().get::<LingtianPlot>(plot).unwrap();
        assert_eq!(
            p.crop.as_ref().unwrap().growth,
            0.0,
            "双干 50 tick 不应有任何生长"
        );
    }
}
